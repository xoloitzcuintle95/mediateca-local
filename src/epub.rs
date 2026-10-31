// SPDX-License-Identifier: MIT

//! Lectura de metadatos y portada de EPUB, estrictamente de solo lectura.
//!
//! Un EPUB es un ZIP no confiable: la portada se lee con un límite de bytes y
//! se comprueban sus dimensiones antes de decodificarla. Cualquier error se
//! convierte en un resultado de respaldo; nunca se propaga al escaneo.

use std::io::{self, Cursor, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use image::ImageReader;
use rbook::Epub;
use rbook::ebook::manifest::ManifestEntry;

use crate::covers::{MAX_COVER_BYTES, MAX_COVER_PIXELS};
use crate::model::TagStatus;

/// Datos leídos de un EPUB.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpubRead {
    pub title: Option<String>,
    pub author: Option<String>,
    pub cover: Option<Vec<u8>>,
    pub status: TagStatus,
}

/// Lee título, autores y portada de un EPUB.
///
/// Nunca falla: ante un EPUB corrupto, malformado o sin metadatos devuelve el
/// nombre del archivo como título y el estado correspondiente.
#[must_use]
pub fn read(path: &Path) -> EpubRead {
    catch_unwind(AssertUnwindSafe(|| read_inner(path)))
        .unwrap_or_else(|_| fallback(path, TagStatus::Error))
}

fn read_inner(path: &Path) -> EpubRead {
    let mut options = Epub::options();
    options.skip_toc(true);
    options.skip_spine(true);

    let Ok(epub) = options.open(path) else {
        return fallback(path, TagStatus::Error);
    };

    let title = epub
        .metadata()
        .title()
        .map(|title| title.value().trim().to_owned())
        .filter(|title| !title.is_empty());

    let author = {
        let authors: Vec<String> = epub
            .metadata()
            .creators()
            .map(|creator| creator.value().trim().to_owned())
            .filter(|creator| !creator.is_empty())
            .collect();

        (!authors.is_empty()).then(|| authors.join(", "))
    };

    let cover = epub
        .manifest()
        .cover_image()
        .and_then(|cover| read_cover(&cover));

    let status = match (&title, &author) {
        (Some(_), Some(_)) => TagStatus::Complete,
        (Some(_), None) | (None, Some(_)) => TagStatus::Partial,
        (None, None) => TagStatus::Missing,
    };

    EpubRead {
        title: Some(title.unwrap_or_else(|| fallback_title(path))),
        author,
        cover,
        status,
    }
}

fn fallback(path: &Path, status: TagStatus) -> EpubRead {
    EpubRead {
        title: Some(fallback_title(path)),
        author: None,
        cover: None,
        status,
    }
}

fn fallback_title(path: &Path) -> String {
    path.file_stem().map_or_else(
        || path.to_string_lossy().into_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    )
}

fn read_cover<'a>(entry: &impl ManifestEntry<'a>) -> Option<Vec<u8>> {
    let mut buffer = LimitedBuffer::new(MAX_COVER_BYTES);
    entry.copy_bytes(&mut buffer).ok()?;

    let data = buffer.into_inner();
    cover_dimensions_ok(&data).then_some(data)
}

fn cover_dimensions_ok(data: &[u8]) -> bool {
    let Ok(reader) = ImageReader::new(Cursor::new(data)).with_guessed_format() else {
        return false;
    };
    let Ok((width, height)) = reader.into_dimensions() else {
        return false;
    };

    width > 0
        && height > 0
        && u64::from(width).saturating_mul(u64::from(height)) <= MAX_COVER_PIXELS
}

struct LimitedBuffer {
    data: Vec<u8>,
    limit: usize,
}

impl LimitedBuffer {
    fn new(limit: usize) -> Self {
        Self {
            data: Vec::new(),
            limit,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.data
    }
}

impl Write for LimitedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.data.len().saturating_add(bytes.len()) > self.limit {
            return Err(io::Error::other("la portada supera el límite"));
        }

        self.data.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};

    use image::{ImageFormat, Rgb, RgbImage};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::read;
    use crate::model::TagStatus;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mediateca-epub-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png_cover() -> Vec<u8> {
        let image = RgbImage::from_pixel(4, 6, Rgb([180, 40, 40]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    fn content_opf(with_cover: bool) -> String {
        let cover_manifest = if with_cover {
            r#"<item id="cover" href="cover.png" media-type="image/png" properties="cover-image"/>"#
        } else {
            ""
        };

        format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="id">test</dc:identifier>
    <dc:title>Libro de prueba</dc:title>
    <dc:creator>Autora Prueba</dc:creator>
    <dc:language>es</dc:language>
    <meta property="dcterms:modified">2026-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="chapter" href="chapter.xhtml" media-type="application/xhtml+xml"/>
    {cover_manifest}
  </manifest>
  <spine>
    <itemref idref="chapter"/>
  </spine>
</package>"#
        )
    }

    fn write_epub(path: &Path, with_cover: bool) {
        let file = fs::File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

        zip.start_file("mimetype", options).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        zip.start_file("META-INF/container.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .unwrap();

        zip.start_file("OEBPS/content.opf", options).unwrap();
        zip.write_all(content_opf(with_cover).as_bytes()).unwrap();

        zip.start_file("OEBPS/chapter.xhtml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><body><p>Hola</p></body></html>"#,
        )
        .unwrap();

        if with_cover {
            zip.start_file("OEBPS/cover.png", options).unwrap();
            zip.write_all(&png_cover()).unwrap();
        }

        zip.finish().unwrap();
    }

    #[test]
    fn corta_la_portada_al_superar_el_tope_de_bytes() {
        let mut buffer = super::LimitedBuffer::new(super::MAX_COVER_BYTES);
        let data = vec![0u8; super::MAX_COVER_BYTES + 1];

        assert!(buffer.write_all(&data).is_err());
    }

    #[test]
    fn lee_titulo_autor_y_portada() {
        let dir = temp_dir("completo");
        let path = dir.join("libro.epub");
        write_epub(&path, true);

        let epub = read(&path);

        assert_eq!(epub.title.as_deref(), Some("Libro de prueba"));
        assert_eq!(epub.author.as_deref(), Some("Autora Prueba"));
        assert!(epub.cover.is_some());
        assert_eq!(epub.status, TagStatus::Complete);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn epub_sin_portada_conserva_metadatos() {
        let dir = temp_dir("sin-portada");
        let path = dir.join("libro.epub");
        write_epub(&path, false);

        let epub = read(&path);

        assert_eq!(epub.title.as_deref(), Some("Libro de prueba"));
        assert_eq!(epub.author.as_deref(), Some("Autora Prueba"));
        assert!(epub.cover.is_none());
        assert_eq!(epub.status, TagStatus::Complete);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn epub_corrupto_usa_nombre_y_estado_error() {
        let dir = temp_dir("corrupto");
        let path = dir.join("libro-roto.epub");
        fs::write(&path, b"esto no es un epub").unwrap();

        let epub = read(&path);

        assert_eq!(epub.title.as_deref(), Some("libro-roto"));
        assert!(epub.author.is_none());
        assert!(epub.cover.is_none());
        assert_eq!(epub.status, TagStatus::Error);

        let _ = fs::remove_dir_all(dir);
    }
}
