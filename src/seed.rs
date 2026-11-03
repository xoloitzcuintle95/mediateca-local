// SPDX-License-Identifier: MIT

//! Generador de catálogo sintético (TEMPORAL, solo desarrollo).
//!
//! Se activa con `MEDIATECA_SEED=<N>` (bin compilado con `--features seed`)
//! y opera SIEMPRE en directorios XDG redirigidos por el shell; nunca toca
//! los datos reales del usuario. Se elimina tras la medición de escala.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::model::{Kind, MediaItem, MediaOrigin, OpenAction, TagStatus, TrackMetadata};

#[cfg(test)]
mod tests {
    use super::generar_items;
    use crate::model::Kind;
    use std::collections::HashSet;

    #[test]
    fn es_determinista() {
        let a = generar_items(200, std::path::Path::new("/tmp/x"));
        let b = generar_items(200, std::path::Path::new("/tmp/x"));
        assert_eq!(a.len(), b.len());
        for (izq, der) in a.iter().zip(b.iter()) {
            assert_eq!(izq.path, der.path);
            assert_eq!(izq.metadata.creator, der.metadata.creator);
            assert_eq!(izq.metadata.title, der.metadata.title);
        }
    }

    #[test]
    fn genera_los_conteos_pedidos() {
        let items = generar_items(5_000, std::path::Path::new("/tmp/x"));

        // ~17,000 pistas: 2-6 por álbum (media 4) da entre 16k y 20k.
        assert!((16_000..=20_000).contains(&items.len()), "{}", items.len());

        // Rutas únicas.
        let rutas: HashSet<&str> = items.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(rutas.len(), items.len());

        // Variedad: acentos, emojis, títulos largos.
        assert!(items.iter().any(|i| {
            i.metadata
                .collection
                .as_deref()
                .is_some_and(|c| c.chars().any(|ch| "áéíóúñÁÉÍÓÚÑ".contains(ch)))
        }));
        assert!(items.iter().any(|i| {
            i.metadata
                .collection
                .as_deref()
                .is_some_and(|c| c.contains('🎵'))
        }));
        assert!(items.iter().any(|i| {
            i.metadata
                .title
                .as_deref()
                .is_some_and(|t| t.chars().count() > 100)
        }));

        // Artistas: ~1,500 distintos por collection_creator.
        let artistas: HashSet<&String> = items
            .iter()
            .filter_map(|i| i.metadata.collection_creator.as_ref())
            .collect();
        assert_eq!(artistas.len(), 1_500);

        // Todos son música con metadatos completos.
        assert!(items.iter().all(|i| i.kind == Kind::Music));
    }
}

const ARTISTAS_BASE: &[&str] = &[
    "Doechii",
    "Café Tacvba",
    "Ángela Aguilar",
    "Ariel Rot",
    "Ñanda Mañawi",
    "Café con Sal",
    "Sueños Líquidos",
    "El Niño 🎵",
    "Molotov Júnior",
    "Zoé Azul",
    "Cortázar Swing",
    "Bersuit Ñ",
    "La Sonora Dinamita Jr",
    "Métrica Perfecta",
    "Julieta Venegas 2",
    "Chava Flores Revival",
    "Los Ángeles Azules Nuevos",
    "Banda El Recodo del Sur",
    "Panteón Rococó Viejo",
    "Enanitos Verdes 2",
    "Ñengo Flow Étnico",
    "Café Quijano Remix",
    "Lila Downs Live",
    "Natalia Lafourcade Trío",
    "Molotov Jazz",
    "Caifanes Oscuro",
    "Santa Sabina Acústico",
    "Real de Catorce Dúo",
    "Tijuana No Electrónico",
    "Control Machete Suave",
    "Kumbia Kings Viejos",
    "Sin Bandera Nuevo",
    "Reik Acústico",
    "Camila Orquestado",
    "Belinda Ranchera",
    "Thalía Sinfónica",
    "Luis Miguel Étnico",
    "José José Bolero",
    "Juan Gabriel Cumbia",
    "Vicente Fernández Rock",
];

const PALABRAS: &[&str] = &[
    "Cumbia",
    "Café",
    "Ñandú",
    "Sueños",
    "Trueno",
    "Lluvia",
    "Canción",
    "Almohada",
    "Fuego",
    "Verde",
    "Niebla",
    "Bosque",
    "Cicatriz",
    "Tormenta",
    "Azúcar",
    "Luna",
    "Medianoche",
    "Frontera",
    "Silencio",
    "Tornado",
    "Espejo",
    "Pantano",
    "Corazón",
    "Amanecer",
    "Polvo",
    "Serpiente",
    "Invierno",
    "Camino",
    "Jardín",
    "Fantasma",
];

const EMOJIS: &[&str] = &["🎵", "🔥", "🌙", "☕", "⚡", "🌵", "🌊"];

/// Genera la lista plana de ítems sintéticos; determinista.
#[must_use]
pub fn generar_items(n_albumes: usize, raiz: &Path) -> Vec<MediaItem> {
    let mut artistas: Vec<String> = ARTISTAS_BASE
        .iter()
        .map(|nombre| (*nombre).to_owned())
        .collect();
    while artistas.len() < 1_500 {
        let idx = artistas.len();
        let base = ARTISTAS_BASE[idx % ARTISTAS_BASE.len()];
        artistas.push(format!("{base} {idx}"));
    }
    artistas[1_498] = format!(
        "Artista de Nombre Excesivamente Largo {}",
        "y más texto para pasarnos del límite razonable ".repeat(8)
    );
    artistas[1_499] = format!("Artista 🎶 {}", EMOJIS[0]);

    let mut items = Vec::new();
    for album_idx in 0..n_albumes {
        let artista = artistas[album_idx % artistas.len()].clone();
        let palabra = PALABRAS[album_idx % PALABRAS.len()];
        let emoji = if album_idx % 37 == 0 {
            format!(" {}", EMOJIS[album_idx % EMOJIS.len()])
        } else {
            String::new()
        };
        let año = u16::try_from(1960 + album_idx % 66).unwrap_or(2000);

        // Los primeros 200 álbumes prueban duplicados: pares con el mismo
        // título/artista/año (se funden en Library::build) e impares con
        // título repetido pero año distinto (se separan).
        let (album, año_album) = if album_idx < 200 && album_idx % 2 == 0 {
            (format!("{palabra}{emoji}"), Some(año))
        } else if album_idx < 200 && album_idx % 2 == 1 {
            (format!("{palabra}{emoji}"), Some(año.wrapping_add(1)))
        } else {
            (format!("{palabra} {album_idx}{emoji}"), Some(año))
        };

        let n_pistas = 2 + (album_idx * 7) % 5;
        for pista_idx in 0..n_pistas {
            let extension = ["mp3", "flac", "ogg"][pista_idx % 3];
            // El título largo vive en el TAG (los filenames reales no pueden
            // pasar de 255 bytes): el archivo usa un nombre corto.
            let titulo_largo = pista_idx == 0 && album_idx % 11 == 0;
            let titulo = if titulo_largo {
                format!(
                    "Pista de Título Realmente Largo {}",
                    "con texto extra para alargar la búsqueda ".repeat(6)
                )
            } else {
                format!(
                    "{} {}",
                    PALABRAS[(album_idx + pista_idx) % PALABRAS.len()],
                    pista_idx + 1
                )
            };
            let nombre_archivo = if titulo_largo {
                format!(
                    "{:02} - Pista de Titulo Largo {}.{}",
                    pista_idx + 1,
                    album_idx,
                    extension
                )
            } else {
                format!("{:02} - {titulo}.{extension}", pista_idx + 1)
            };
            let mut creador = artista.clone();
            if album_idx % 9 == 0 {
                creador.push_str(" feat. Invitado");
            }
            items.push(MediaItem {
                id: 0,
                path: raiz
                    .join(format!("album-{album_idx:05}"))
                    .join(&nombre_archivo)
                    .to_string_lossy()
                    .into_owned(),
                name: nombre_archivo,
                kind: Kind::Music,
                mtime: 0, // lo rellena escribir_biblioteca con el stat real
                size: 0,
                origin: MediaOrigin::LocalFile,
                open_action: OpenAction::XdgOpen,
                cover_ready: false,
                metadata: TrackMetadata {
                    creator: Some(creador),
                    collection_creator: Some(artista.clone()),
                    collection: Some(album.clone()),
                    title: Some(titulo),
                    track_number: Some(u32::try_from(pista_idx + 1).unwrap_or(1)),
                    track_total: Some(u32::try_from(n_pistas).unwrap_or(1)),
                    filename_track: Some(u32::try_from(pista_idx + 1).unwrap_or(1)),
                    year: año_album,
                    cover_hash: None, // lo rellena escribir_biblioteca
                    status: TagStatus::Complete,
                },
            });
        }
    }
    items
}

/// Escribe la biblioteca sintética en disco y ajusta cada ítem al stat real.
///
/// Por álbum: un `cover.jpg` real de 320×320 (gradiente determinista por
/// álbum) y los archivos de audio como bytes dummy de ~1 KB. Devuelve los
/// ítems con `mtime`/`size` reales (para que `can_reuse_metadata` funcione)
/// y `cover_hash` = sha256 del JPEG.
///
/// # Errors
/// Si falla crear directorios o escribir archivos.
pub fn escribir_biblioteca(items: &mut [MediaItem], raiz: &Path) -> Result<(), String> {
    use image::{Rgb, RgbImage};

    std::fs::create_dir_all(raiz)
        .map_err(|error| format!("No se pudo crear {}: {error}", raiz.display()))?;

    let mut por_album: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        let dir = Path::new(&item.path)
            .parent()
            .map_or_else(String::new, |p| p.to_string_lossy().into_owned());
        por_album.entry(dir).or_default().push(idx);
    }

    for (dir_album, indices) in por_album {
        let dir_album = PathBuf::from(&dir_album);
        std::fs::create_dir_all(&dir_album)
            .map_err(|error| format!("No se pudo crear {}: {error}", dir_album.display()))?;

        let album_idx: usize = dir_album
            .file_name()
            .and_then(|nombre| nombre.to_str())
            .and_then(|nombre| nombre.trim_start_matches("album-").parse().ok())
            .unwrap_or(0);
        let mut imagen = RgbImage::new(320, 320);
        for (x, y, pixel) in imagen.enumerate_pixels_mut() {
            let r = ((album_idx * 7 + x as usize) % 256) as u8;
            let g = ((album_idx * 13 + y as usize) % 256) as u8;
            let b = ((album_idx * 29 + x as usize + y as usize) % 256) as u8;
            *pixel = Rgb([r, g, b]);
        }
        let mut jpeg: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgb8(imagen)
            .write_to(
                &mut std::io::Cursor::new(&mut jpeg),
                image::ImageFormat::Jpeg,
            )
            .map_err(|error| format!("No se pudo codificar el JPEG: {error}"))?;

        let hash: String =
            Sha256::digest(&jpeg)
                .iter()
                .fold(String::with_capacity(64), |mut hex, byte| {
                    hex.push_str(&format!("{byte:02x}"));
                    hex
                });

        std::fs::write(dir_album.join("cover.jpg"), &jpeg)
            .map_err(|error| format!("No se pudo escribir el cover: {error}"))?;

        for idx in indices {
            let item = &mut items[idx];
            let bytes = vec![0xFF_u8; 1024];
            std::fs::write(Path::new(&item.path), &bytes)
                .map_err(|error| format!("No se pudo escribir {}: {error}", item.path))?;
            let stat = std::fs::metadata(Path::new(&item.path))
                .map_err(|error| format!("No se pudo leer el stat: {error}"))?;
            item.size = i64::try_from(stat.len()).unwrap_or(i64::MAX);
            item.mtime = stat
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .and_then(|d| i64::try_from(d.as_nanos()).ok())
                .unwrap_or(0);
            item.metadata.cover_hash = Some(hash.clone());
            item.cover_ready = true;
        }
    }
    Ok(())
}

/// Punto de entrada del modo de sembrado. Idempotente.
///
/// # Errors
/// Si falla el I/O del disco o la base de datos.
pub fn ejecutar() -> Result<(), String> {
    let n_albumes: usize = std::env::var("MEDIATECA_SEED")
        .ok()
        .and_then(|valor| valor.parse().ok())
        .unwrap_or(5_000);
    let raiz = PathBuf::from("/tmp/mediateca-scale/library");
    std::fs::create_dir_all(&raiz)
        .map_err(|error| format!("No se pudo crear la biblioteca sintética: {error}"))?;

    let existentes = crate::db::seed_filas_bajo(&raiz)?;
    if existentes > 0 {
        eprintln!("[seed] ya sembrado con {existentes} archivos; nada por hacer");
        return Ok(());
    }

    let mut items = generar_items(n_albumes, &raiz);
    escribir_biblioteca(&mut items, &raiz)?;
    crate::db::seed_test_catalog(&items)?;
    eprintln!(
        "[seed] sembrado: {} álbumes, {} archivos en {}",
        n_albumes,
        items.len(),
        raiz.display()
    );
    Ok(())
}
