// SPDX-License-Identifier: MIT

//! Recorrido del disco. Este módulo solo lee archivos; nunca los modifica.

use std::path::Path;
use std::time::UNIX_EPOCH;

use walkdir::{DirEntry, WalkDir};

use crate::media;
use crate::model::{Kind, MediaItem, MediaOrigin, ScanSummary, TrackMetadata};

/// Resultado de recorrer una carpeta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanResult {
    pub items: Vec<MediaItem>,
    pub summary: ScanSummary,
}

/// Clasifica una extensión usando la definición única de tipos de medio.
#[must_use]
pub fn classify(extension: &str) -> Option<Kind> {
    media::classify_extension(extension)
}

/// Recorre `root` sin seguir enlaces simbólicos y sin entrar en carpetas ocultas.
///
/// Los errores de una entrada (por ejemplo, permisos) no abortan el recorrido:
/// se cuentan en `summary.errors`.
#[must_use]
pub fn scan(root: &Path) -> ScanResult {
    let mut items = Vec::new();
    let mut summary = ScanSummary::default();

    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_hidden(entry));

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                summary.errors += 1;
                eprintln!("error al recorrer el catálogo: {error}");
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let Some(kind) = path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .and_then(classify)
        else {
            summary.skipped += 1;
            continue;
        };

        // La ruta se guarda como texto en SQLite. Si no es UTF-8, perdería
        // bytes al convertirla y podría colisionar con otro archivo distinto.
        // Se omite y se cuenta como error para que el escaneo sea incompleto.
        let Some(path_text) = path.to_str() else {
            summary.errors += 1;
            eprintln!(
                "se omite un archivo con ruta no UTF-8: {}",
                path.display()
            );
            continue;
        };

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                summary.errors += 1;
                eprintln!("no se pudo leer {}: {error}", path.display());
                continue;
            }
        };

        let size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| {
                i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX)
            });

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path_text)
            .to_owned();

        items.push(MediaItem {
            id: 0,
            path: path_text.to_owned(),
            name,
            kind,
            mtime,
            size,
            origin: MediaOrigin::LocalFile,
            open_action: media::media_type(kind).open_action,
            cover_ready: false,
            metadata: TrackMetadata::default(),
        });

        match kind {
            Kind::Music => summary.music += 1,
            Kind::Video => summary.video += 1,
            Kind::Book => summary.book += 1,
        }
    }

    items.sort_by_cached_key(|item| item.name.to_lowercase());

    ScanResult { items, summary }
}

/// Las carpetas ocultas se podan completas, incluida `.Trash-1000`.
fn is_hidden(entry: &DirEntry) -> bool {
    entry.depth() > 0 && entry.file_name().to_string_lossy().starts_with('.')
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{classify, scan};
    use crate::model::Kind;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mi-catalogo-scanner-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn clasifica_extensiones_sin_importar_mayusculas() {
        assert_eq!(classify("mkv"), Some(Kind::Video));
        assert_eq!(classify("MP4"), Some(Kind::Video));
        assert_eq!(classify("Avi"), Some(Kind::Video));
        assert_eq!(classify("mp3"), Some(Kind::Music));
        assert_eq!(classify("FLAC"), Some(Kind::Music));
        assert_eq!(classify("ogg"), Some(Kind::Music));
        assert_eq!(classify("epub"), Some(Kind::Book));
        assert_eq!(classify("PDF"), Some(Kind::Book));
    }

    #[test]
    fn ignora_extensiones_desconocidas() {
        assert_eq!(classify("txt"), None);
        assert_eq!(classify("jpg"), None);
        assert_eq!(classify(""), None);
        assert_eq!(classify("mp3 "), None);
    }

    #[cfg(unix)]
    #[test]
    fn omite_rutas_no_utf8_que_colisionarian_al_perder_bytes() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let dir = temp_dir("no-utf8");
        let first = dir.join(OsStr::from_bytes(b"cancion-\xff.mp3"));
        let second = dir.join(OsStr::from_bytes(b"cancion-\xfe.mp3"));

        fs::write(&first, b"x").unwrap();
        fs::write(&second, b"x").unwrap();
        fs::write(dir.join("valida.mp3"), b"x").unwrap();

        // Ambas rutas se ven idénticas si se reemplazan los bytes inválidos.
        assert_eq!(first.to_string_lossy(), second.to_string_lossy());

        let result = scan(&dir);

        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "valida.mp3");
        assert_eq!(result.items[0].path, dir.join("valida.mp3").to_str().unwrap());
        assert_eq!(result.summary.errors, 2);
        assert_eq!(result.summary.music, 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn escanea_acentos_caracteres_raros_y_omite_ocultos() {
        let dir = temp_dir("ocultos");

        fs::create_dir_all(dir.join(".Trash-1000/info")).unwrap();
        fs::write(dir.join(".Trash-1000/basura.mp3"), b"x").unwrap();
        fs::write(dir.join(".Trash-1000/info/lost+found.trashinfo"), b"x").unwrap();

        fs::write(dir.join("cancion.mp3"), b"x").unwrap();
        fs::write(dir.join("17 - What's Your Name？.mp3"), b"x").unwrap();
        fs::write(dir.join("canción.flac"), b"x").unwrap();
        fs::write(dir.join("Pelicula.MKV"), b"x").unwrap();
        fs::write(dir.join("notas.txt"), b"x").unwrap();

        let result = scan(&dir);

        assert_eq!(result.summary.music, 3);
        assert_eq!(result.summary.video, 1);
        assert_eq!(result.summary.book, 0);
        assert_eq!(result.summary.skipped, 1);
        assert_eq!(result.summary.errors, 0);
        assert_eq!(result.summary.total(), 4);

        assert!(
            result
                .items
                .iter()
                .any(|item| item.name == "17 - What's Your Name？.mp3")
        );
        assert!(result.items.iter().any(|item| item.name == "canción.flac"));
        assert!(
            result
                .items
                .iter()
                .all(|item| !item.path.contains(".Trash-1000"))
        );
        assert!(result.items.iter().all(|item| item.kind != Kind::Book));

        let _ = fs::remove_dir_all(dir);
    }
}
