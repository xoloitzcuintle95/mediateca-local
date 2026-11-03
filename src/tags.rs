// SPDX-License-Identifier: MIT

//! Lectura de etiquetas con `lofty`.
//!
//! Es estrictamente de solo lectura: este módulo no guarda ni modifica el
//! archivo original.

use std::fs;
use std::io::BufReader;
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::error::FileParseError;
use lofty::file::{FileType, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag};

use crate::covers::MAX_COVER_BYTES;
use crate::model::{TagStatus, TrackMetadata};

/// Etiquetas leídas de un archivo, incluida la portada incrustada.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TagRead {
    pub metadata: TrackMetadata,
    pub picture: Option<Vec<u8>>,
}

/// Lee las etiquetas de `path`. Nunca falla: ante un error devuelve
/// `TagStatus::Error` y los datos de respaldo del nombre del archivo.
#[must_use]
pub fn read(path: &Path) -> TagRead {
    read_with_advice(path, true)
}

/// `use_fadvise` existe para la prueba A/B; en producción siempre es `true`.
fn read_with_advice(path: &Path, use_fadvise: bool) -> TagRead {
    let filename_track = filename_track(path);
    let fallback = fallback_title(path);

    // Sin `read_properties`: no necesitamos duración y así lofty no recorre
    // todo el audio, solo las etiquetas.
    let parsed = open_probe(path, use_fadvise).and_then(|probe| {
        probe
            .options(ParseOptions::new().read_properties(false))
            .read()
    });

    match parsed {
        Ok(file) => read_tags(&file, filename_track, fallback),
        Err(error) => {
            eprintln!(
                "no se pudieron leer las etiquetas de {}: {error}",
                path.display()
            );
            TagRead {
                metadata: TrackMetadata {
                    title: Some(fallback),
                    filename_track,
                    status: TagStatus::Error,
                    ..TrackMetadata::default()
                },
                picture: None,
            }
        }
    }
}

/// Abre el archivo nosotros para poder dar la pista de I/O antes de que lofty
/// lo lea. El tipo se resuelve igual que `Probe::open`: por la extensión.
fn open_probe(
    path: &Path,
    use_fadvise: bool,
) -> Result<Probe<BufReader<fs::File>>, FileParseError> {
    let file = fs::File::open(path)?;

    if use_fadvise {
        advise_random(&file);
    }

    let reader = BufReader::new(file);

    Ok(match FileType::from_path(path) {
        Some(file_type) => Probe::with_file_type(reader, file_type),
        None => Probe::new(reader),
    })
}

/// `POSIX_FADV_RANDOM`: el tag está al principio y no hace falta que el kernel
/// lea por adelantado el audio que viene detrás. Si falla, se ignora.
#[cfg(target_os = "linux")]
fn advise_random(file: &fs::File) {
    let _ = rustix::fs::fadvise(file, 0, None, rustix::fs::Advice::Random);
}

#[cfg(not(target_os = "linux"))]
fn advise_random(_file: &fs::File) {}

fn read_tags(
    file: &lofty::file::TaggedFile,
    filename_track: Option<u32>,
    fallback_title: String,
) -> TagRead {
    let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) else {
        return TagRead {
            metadata: TrackMetadata {
                title: Some(fallback_title),
                filename_track,
                status: TagStatus::Missing,
                ..TrackMetadata::default()
            },
            picture: None,
        };
    };

    let creator = clean(tag.artist().as_deref());
    // El álbum se agrupa por su artista real (TPE2), no por los créditos del
    // track: "Doechii feat. X" no debe crear un artista falso. Si el tagger
    // no escribió TPE2, se recurre al primer artista del track, como antes.
    let collection_creator = clean(tag.get_string(ItemKey::AlbumArtist))
        .or_else(|| creator.as_deref().and_then(first_artist))
        .map(|artist| principal_artist(&artist));
    let collection = clean(tag.album().as_deref());
    let title = clean(tag.title().as_deref());
    let has_any = creator.is_some() || collection.is_some() || title.is_some();
    let complete = creator.is_some() && collection.is_some() && title.is_some();

    let metadata = TrackMetadata {
        creator: creator.clone(),
        collection_creator,
        collection,
        title: title.or(Some(fallback_title)),
        track_number: tag.track(),
        track_total: tag.track_total(),
        filename_track,
        year: tag.date().map(|date| date.year),
        cover_hash: None,
        status: if complete {
            TagStatus::Complete
        } else if has_any {
            TagStatus::Partial
        } else {
            TagStatus::Missing
        },
    };

    TagRead {
        metadata,
        picture: cover_picture(tag),
    }
}

fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// El artista principal para agrupar: quita el sufijo de colaboraciones
/// ("feat. X", "ft. X", "featuring X") que muchos taggers escriben dentro del
/// mismo campo. No toca nombres acreditados con `&` ("Simon & Garfunkel") ni
/// comas: eso forma parte del nombre, no separa créditos.
#[must_use]
pub fn principal_artist(artist: &str) -> String {
    const SUFFIXES: [&str; 4] = [" feat.", " featuring", " ft.", " ft "];
    let lower = artist.to_lowercase();
    let cut = SUFFIXES
        .iter()
        .filter_map(|suffix| lower.find(suffix))
        .min();

    match cut {
        Some(index) if lower.len() == artist.len() => artist[..index].trim().to_owned(),
        _ => artist.trim().to_owned(),
    }
}

/// El primer artista de una lista separada por comas, tal como viene en TPE1.
#[must_use]
pub fn first_artist(artist: &str) -> Option<String> {
    artist
        .split(',')
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// Extrae el número inicial del nombre: `57 - WAIT.mp3` -> `Some(57)`.
///
/// Se limita a 1-3 dígitos y exige un separador después para no confundir
/// años como `2024 - Canción.mp3` con números de pista.
#[must_use]
pub fn filename_track(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_string_lossy();
    let digits: String = stem.chars().take_while(char::is_ascii_digit).collect();

    if digits.is_empty() || digits.len() > 3 {
        return None;
    }

    let rest = stem.get(digits.len()..)?;
    let valid_separator = rest
        .chars()
        .next()
        .is_none_or(|character| character.is_whitespace() || matches!(character, '-' | '_' | '.'));

    valid_separator.then(|| digits.parse().ok()).flatten()
}

/// Título de respaldo: nombre del archivo sin el prefijo numérico.
#[must_use]
pub fn fallback_title(path: &Path) -> String {
    let stem = path.file_stem().map_or_else(
        || path.to_string_lossy().into_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );

    let digits = stem.chars().take_while(char::is_ascii_digit).count();

    if digits > 0
        && digits <= 3
        && let Some(rest) = stem.get(digits..)
    {
        let title = rest.trim_start_matches(|character: char| {
            character.is_whitespace() || matches!(character, '-' | '_' | '.')
        });

        if !title.is_empty() {
            return title.to_owned();
        }
    }

    stem
}

fn cover_picture(tag: &Tag) -> Option<Vec<u8>> {
    let pictures = tag.pictures();

    pictures
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.first())
        .filter(|picture| !picture.data().is_empty())
        // Lofty ya cargó la imagen en memoria; este filtro evita una segunda
        // copia enorme, pero no puede evitar la memoria que ya usó lofty.
        .filter(|picture| picture.data().len() <= MAX_COVER_BYTES)
        .map(|picture: &Picture| picture.data().to_vec())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use sha2::{Digest, Sha256};

    use super::{
        fallback_title, filename_track, first_artist, principal_artist, read, read_with_advice,
    };
    use crate::model::TagStatus;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mi-catalogo-tags-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn synchsafe(size: usize) -> [u8; 4] {
        let mut size = size;
        let mut bytes = [0u8; 4];

        for byte in bytes.iter_mut().rev() {
            *byte = u8::try_from(size & 0x7F).unwrap();
            size >>= 7;
        }

        bytes
    }

    fn id3_frame(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(10 + payload.len());
        frame.extend_from_slice(id);
        frame.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(payload);
        frame
    }

    fn mp3_de_prueba() -> Vec<u8> {
        let apic = b"\x00image/jpeg\0\x03\x00\xFF\xD8portada-falsa\xFF\xD9";
        let mut frames = Vec::new();
        frames.extend(id3_frame(b"TPE2", b"\x00Album Artista\0"));
        frames.extend(id3_frame(b"TPE1", b"\x00Artista\0"));
        frames.extend(id3_frame(b"TALB", b"\x00Album\0"));
        frames.extend(id3_frame(b"TIT2", b"\x00Cancion\0"));
        frames.extend(id3_frame(b"TRCK", b"\x007/12\0"));
        frames.extend(id3_frame(b"TYER", b"\x002020\0"));
        frames.extend(id3_frame(b"APIC", apic));

        let mut file = Vec::new();
        file.extend_from_slice(b"ID3");
        file.extend_from_slice(&[3, 0, 0]);
        file.extend_from_slice(&synchsafe(frames.len()));
        file.extend_from_slice(&frames);
        file
    }

    fn sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn el_fadvise_no_cambia_las_etiquetas_ni_la_portada() {
        let dir = temp_dir("fadvise");
        let path = dir.join("07 - Cancion.mp3");
        fs::write(&path, mp3_de_prueba()).unwrap();

        let con = read_with_advice(&path, true);
        let sin = read_with_advice(&path, false);

        assert_eq!(con.metadata.status, TagStatus::Complete);
        assert_eq!(con.metadata.creator.as_deref(), Some("Artista"));
        assert_eq!(
            con.metadata.collection_creator.as_deref(),
            Some("Album Artista")
        );
        assert_eq!(con.metadata.collection.as_deref(), Some("Album"));
        assert_eq!(con.metadata.title.as_deref(), Some("Cancion"));
        assert_eq!(con.metadata.track_number, Some(7));
        assert_eq!(con.metadata.track_total, Some(12));
        assert_eq!(con.metadata.year, Some(2020));
        assert_eq!(con.metadata, sin.metadata);
        assert_eq!(con.picture, sin.picture);

        let picture = con.picture.as_deref().expect("el MP3 debe tener portada");
        assert_eq!(sha256(picture), sha256(sin.picture.as_deref().unwrap()));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn extrae_numero_de_pista_del_nombre() {
        assert_eq!(filename_track(Path::new("57 - WAIT.mp3")), Some(57));
        assert_eq!(filename_track(Path::new("01 - Girls.mp3")), Some(1));
        assert_eq!(filename_track(Path::new("01.Girls.mp3")), Some(1));
        assert_eq!(filename_track(Path::new("01_Girls.mp3")), Some(1));
        assert_eq!(filename_track(Path::new("2024 - Canción.mp3")), None);
        assert_eq!(filename_track(Path::new("Girls.mp3")), None);
    }

    #[test]
    fn quita_el_prefijo_del_titulo_de_respaldo() {
        assert_eq!(fallback_title(Path::new("57 - WAIT.mp3")), "WAIT");
        assert_eq!(
            fallback_title(Path::new("17 - What's Your Name？.mp3")),
            "What's Your Name？"
        );
        assert_eq!(fallback_title(Path::new("canción.flac")), "canción");
        assert_eq!(
            fallback_title(Path::new("2024 - Canción.mp3")),
            "2024 - Canción"
        );
        assert_eq!(fallback_title(Path::new("01.mp3")), "01");
    }

    #[test]
    fn el_artista_principal_quita_el_feat_y_conserva_los_nombres_con_ampersand() {
        assert_eq!(principal_artist("Doechii feat. Leo Conoza"), "Doechii");
        assert_eq!(principal_artist("Doechii ft. Mike Escanor"), "Doechii");
        assert_eq!(principal_artist("Doechii featuring Jstray"), "Doechii");
        assert_eq!(principal_artist("Simon & Garfunkel"), "Simon & Garfunkel");
        assert_eq!(principal_artist("Doechii"), "Doechii");
        assert_eq!(principal_artist(""), "");
    }

    #[test]
    fn toma_el_primer_artista_de_la_lista() {
        assert_eq!(first_artist("Doechii, JT"), Some(String::from("Doechii")));
        assert_eq!(
            first_artist("Jayprob, Doechii"),
            Some(String::from("Jayprob"))
        );
        assert_eq!(first_artist(""), None);
        assert_eq!(first_artist("  ,  "), None);
    }

    #[test]
    fn un_archivo_sin_etiquetas_usa_el_nombre_y_no_se_modifica() {
        let dir = temp_dir("sin-etiquetas");
        let path = dir.join("57 - WAIT.mp3");
        fs::write(&path, b"no es un mp3 real").unwrap();
        let before = fs::read(&path).unwrap();

        let tags = read(&path);

        assert_eq!(tags.metadata.status, TagStatus::Error);
        assert_eq!(tags.metadata.title.as_deref(), Some("WAIT"));
        assert_eq!(tags.metadata.filename_track, Some(57));
        assert_eq!(tags.picture, None);
        assert_eq!(fs::read(&path).unwrap(), before);

        let _ = fs::remove_dir_all(dir);
    }
}
