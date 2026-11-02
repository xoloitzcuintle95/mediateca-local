// SPDX-License-Identifier: MIT

//! Lectura de etiquetas de contenedores de video. Solo lectura, nunca escritura.
//!
//! Despacho por extensión: MP4/M4A/MOV con `mp4ameta` (estilo iTunes),
//! MKV/WebM con `matroska` (segmento Info y Tags). Los contenedores sin
//! lector caen a un respaldo derivado del nombre de archivo con el mismo
//! parseo que usa la vista de video.

use std::path::Path;

use matroska::TagValue;
use crate::model::TagStatus;

/// Etiquetas leídas de un video. Son solo datos; nunca se escriben al archivo.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VideoTags {
    pub title: Option<String>,
    pub creator: Option<String>,
    /// Serie o colección, si el etiquetador la escribió.
    pub collection: Option<String>,
    pub year: Option<u16>,
    pub status: TagStatus,
}

/// Lee las etiquetas del contenedor según su extensión.
pub fn read(path: &Path) -> VideoTags {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    let tags = match ext.as_str() {
        "mp4" | "m4v" | "m4a" | "mov" => read_mp4(path),
        "mkv" | "webm" => read_mkv(path),
        // Contenedor sin lector: respaldo por nombre de archivo.
        _ => return fallback_de_nombre(path),
    };

    match tags {
        Some(tags) => {
            let status = match (&tags.title, &tags.creator, &tags.year) {
                (Some(_), Some(_), Some(_)) => TagStatus::Complete,
                (None, None, None) => TagStatus::Missing,
                _ => TagStatus::Partial,
            };
            VideoTags { status, ..tags }
        }
        // Hay lector para esta extensión pero el archivo no se pudo leer.
        None => VideoTags {
            status: TagStatus::Error,
            ..VideoTags::default()
        },
    }
}

/// Año civil desde segundos Unix, sin dependencias extra (algoritmo de
/// Hinnant completo, con la corrección de enero y febrero porque el año del
/// algoritmo empieza en marzo). Devuelve None fuera de rango de u16.
fn year_de_unix(unix_secs: i64) -> Option<u16> {
    let z = unix_secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    // Día del año dentro del año-marzo y mes para la corrección.
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };

    u16::try_from(y + i64::from(month <= 2)).ok()
}

fn read_mp4(path: &Path) -> Option<VideoTags> {
    let tag = mp4ameta::Tag::read_from_path(path).ok()?;

    let year = tag
        .year()
        .and_then(|text| text.get(..4))
        .and_then(|text| text.parse::<u16>().ok());

    Some(VideoTags {
        title: tag.title().map(String::from),
        creator: tag.artist().map(String::from),
        collection: tag.album().map(String::from),
        year,
        ..VideoTags::default()
    })
}

fn read_mkv(path: &Path) -> Option<VideoTags> {
    let doc = matroska::open(path).ok()?;

    // ffmpeg suele escribir el título en el segmento Info, no en Tags.
    let mut title = doc.info.title.clone();
    let mut creator = None;
    let mut year = doc
        .info
        .date_utc
        .map(i64::from)
        .map(|ns| 978_307_200 + ns / 1_000_000_000)
        .and_then(year_de_unix);

    for tag in &doc.tags {
        for simple in &tag.simple {
            let Some(tag_value) = simple.value.as_ref() else {
                continue;
            };
            let TagValue::String(value) = tag_value else {
                continue;
            };

            match simple.name.to_ascii_uppercase().as_str() {
                "TITLE" if title.is_none() => title = Some(value.to_owned()),
                "ARTIST" | "PERFORMEDBY" if creator.is_none() => {
                    creator = Some(value.to_owned())
                }
                "DATE_RECORDED" | "DATE" if year.is_none() => {
                    year = value.get(..4).and_then(|s| s.parse().ok());
                }
                _ => {}
            }
        }
    }

    Some(VideoTags {
        title,
        creator,
        year,
        ..VideoTags::default()
    })
}

fn fallback_de_nombre(path: &Path) -> VideoTags {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    let parsed = crate::video::parse_video_name(name);

    VideoTags {
        collection: parsed.series,
        year: parsed.year,
        status: TagStatus::Missing,
        ..VideoTags::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mi-catalogo-videotags-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn un_archivo_invalido_da_error_y_no_panea() {
        let dir = temp_dir("roto");
        let path = dir.join("roto.mp4");
        std::fs::write(&path, b"no-es-un-mp4").unwrap();

        let tags = read(&path);
        assert_eq!(tags.status, TagStatus::Error);
        assert_eq!(tags.title, None);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn un_mkv_invalido_da_error() {
        let dir = temp_dir("roto-mkv");
        let path = dir.join("roto.mkv");
        std::fs::write(&path, b"no-es-un-mkv").unwrap();

        assert_eq!(read(&path).status, TagStatus::Error);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn contenedores_sin_lector_caen_al_respaldo() {
        let tags = read(Path::new("/m/Pelicula.1998.avi"));
        assert_eq!(tags.status, TagStatus::Missing);
        assert_eq!(tags.year, Some(1998));
        assert_eq!(tags.title, None);
    }

    #[test]
    fn el_respaldo_reusa_el_parseo_de_series() {
        let tags = read(Path::new("/m/South.Park.S29E03.1998.avi"));
        assert_eq!(tags.status, TagStatus::Missing);
        assert_eq!(tags.collection, Some(String::from("South Park")));
        assert_eq!(tags.year, Some(1998));
    }

    #[test]
    fn calcula_el_anio_desde_el_epoch_de_mkv() {
        // 2001-01-01 UTC son 978307200 segundos Unix.
        assert_eq!(year_de_unix(978_307_200), Some(2001));
        // Un año después, redondeado por días.
        assert_eq!(year_de_unix(978_307_200 + 365 * 86_400), Some(2002));
        assert_eq!(year_de_unix(0), Some(1970));
    }
}
