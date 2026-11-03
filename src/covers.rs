// SPDX-License-Identifier: MIT

//! Miniaturas de portadas en el directorio de caché del usuario.
//!
//! Los archivos de la colección solo se leen. Las miniaturas se escriben
//! exclusivamente en `XDG_CACHE_HOME/com.xoloitzcuintle95.MediatecaLocal/covers`.

use std::fmt::Write as _;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageReader, Limits, Rgb, RgbImage};
use sha2::{Digest, Sha256};

use crate::media;
use crate::model::{APP_ID, Kind, LEGACY_APP_IDS};

/// Versión del algoritmo de miniaturas. Cambiarla invalida el caché anterior.
pub const THUMBNAIL_VERSION: &str = "v3";
/// Ancho base de todas las miniaturas. 320 px basta: las tarjetas se dibujan
/// a 220 px y la lista a 48 px; a 512 el caché RGBA de iced consumía ~1 MB
/// por portada y con ~90 álbumes el RSS se iba a 100+ MB.
pub const THUMBNAIL_WIDTH: u32 = 320;

/// Tope de bytes de una portada antes de leerla, decodificarla o hashearla.
pub const MAX_COVER_BYTES: usize = 8 * 1024 * 1024;
/// Tope de píxeles de una portada antes de decodificarla.
pub const MAX_COVER_PIXELS: u64 = 8_000_000;
/// Tope de ancho o alto individual de una portada.
const MAX_COVER_SIDE: u32 = 8_000_000;
/// Presupuesto de memoria por decodificación de portada.
const MAX_COVER_ALLOC: u64 = MAX_COVER_PIXELS.saturating_mul(8);

/// Tipo de portada que se quiere generar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverTarget {
    Music,
    // La usa la tanda de video.
    #[allow(dead_code)]
    Video,
    Book,
}

impl CoverTarget {
    /// Portada que corresponde a un tipo de medio.
    #[must_use]
    pub const fn for_kind(kind: Kind) -> Self {
        match kind {
            Kind::Music => Self::Music,
            Kind::Video => Self::Video,
            Kind::Book => Self::Book,
        }
    }

    /// Tipo de medio al que pertenece esta portada.
    #[must_use]
    pub const fn kind(self) -> Kind {
        match self {
            Self::Music => Kind::Music,
            Self::Video => Kind::Video,
            Self::Book => Kind::Book,
        }
    }

    /// Proporción (ancho, alto) definida en la tabla única de medios.
    #[must_use]
    pub fn aspect(self) -> (u32, u32) {
        media::media_type(self.kind()).cover_aspect
    }

    /// Alto de la miniatura a partir de su ancho.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn height_for_width(self, width: f32) -> f32 {
        let (aspect_width, aspect_height) = self.aspect();
        width * aspect_height as f32 / aspect_width as f32
    }

    /// Alto de la miniatura estándar.
    #[must_use]
    pub fn height(self) -> u32 {
        THUMBNAIL_WIDTH * self.aspect().1 / self.aspect().0
    }
}

const JPEG_QUALITY: u8 = 85;
const BAR_TOLERANCE: u8 = 12;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const LOCAL_COVERS: &[&str] = &[
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
];

/// Directorio de caché de miniaturas.
pub fn cache_dir() -> Result<PathBuf, String> {
    let dir = dirs::cache_dir()
        .map(|dir| dir.join(APP_ID).join("covers"))
        .ok_or_else(|| "No se pudo determinar el directorio de caché del usuario".to_owned())?;

    migrate_legacy_cache(&dir);
    Ok(dir)
}

/// Copia las miniaturas de una versión anterior si aún no hay caché nuevo.
fn migrate_legacy_cache(new_cache: &Path) {
    let Some(base) = dirs::cache_dir() else {
        return;
    };

    migrate_legacy_cache_from(new_cache, &base);
}

fn migrate_legacy_cache_from(new_cache: &Path, base: &Path) {
    if new_cache.exists() {
        return;
    }

    for legacy in LEGACY_APP_IDS {
        let old_cache = base.join(legacy).join("covers");

        let Ok(entries) = fs::read_dir(&old_cache) else {
            continue;
        };

        if fs::create_dir_all(new_cache).is_err() {
            return;
        }

        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                let _ = fs::copy(entry.path(), new_cache.join(entry.file_name()));
            }
        }

        return;
    }
}

/// Genera (si hace falta) la miniatura y devuelve el hash de la imagen original.
///
/// El hash es el nombre estable; el tipo de medio forma parte del nombre para
/// que una misma imagen pueda tener una miniatura 1:1 y otra 2:3.
pub fn ensure_thumbnail(cache: &Path, data: &[u8], target: CoverTarget) -> Option<String> {
    // Los límites se aplican antes de hashear o decodificar: una portada
    // adversaria no debe llegar al decodificador ni gastar memoria de más.
    if data.is_empty() || data.len() > MAX_COVER_BYTES {
        return None;
    }

    let (width, height) = image_dimensions(data)?;

    if !dimensions_allowed(width, height) {
        return None;
    }

    let hash = hex_sha256(data);

    if has_thumbnail(cache, &hash, target) {
        return Some(hash);
    }

    if let Err(error) = fs::create_dir_all(cache) {
        eprintln!("no se pudo crear el caché {}: {error}", cache.display());
        return None;
    }

    let image = decode_cover(data)?;
    let thumbnail = render_cover(image, target);
    let path = thumbnail_path(cache, &hash, target);
    let temporary = path.with_extension(format!(
        "jpg.{}.tmp",
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let result = write_jpeg(&temporary, &thumbnail).and_then(|()| {
        fs::rename(&temporary, &path)
            .map_err(|error| format!("no se pudo mover la miniatura: {error}"))
    });

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }

    match result {
        Ok(()) => Some(hash),
        Err(error) => {
            eprintln!("{error}");
            None
        }
    }
}

/// Límites estrictos aplicados por `image` al leer o decodificar una portada.
fn cover_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_COVER_SIDE);
    limits.max_image_height = Some(MAX_COVER_SIDE);
    limits.max_alloc = Some(MAX_COVER_ALLOC);
    limits
}

fn cover_reader(data: &[u8]) -> Option<ImageReader<Cursor<&[u8]>>> {
    let mut reader = ImageReader::new(Cursor::new(data));
    reader.limits(cover_limits());
    reader.with_guessed_format().ok()
}

fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    cover_reader(data)?.into_dimensions().ok()
}

fn decode_cover(data: &[u8]) -> Option<DynamicImage> {
    match cover_reader(data)?.decode() {
        Ok(image) => Some(image),
        Err(error) => {
            eprintln!("no se pudo decodificar una portada: {error}");
            None
        }
    }
}

fn dimensions_allowed(width: u32, height: u32) -> bool {
    width > 0
        && height > 0
        && u64::from(width).saturating_mul(u64::from(height)) <= MAX_COVER_PIXELS
}

fn render_cover(image: DynamicImage, target: CoverTarget) -> DynamicImage {
    let (image, cropped_bars) = match detect_bars(&image) {
        Some((left, right))
            if target != CoverTarget::Music || is_squareish(&image, left, right) =>
        {
            let (width, height) = image.dimensions();
            (image.crop_imm(left, 0, width - left - right, height), true)
        }
        _ => (image, false),
    };

    if target == CoverTarget::Music && cropped_bars {
        image.resize_to_fill(THUMBNAIL_WIDTH, target.height(), FilterType::Lanczos3)
    } else {
        fit_cover(&image, THUMBNAIL_WIDTH, target.height())
    }
}

/// Detecta barras laterales comparando cada columna con el color del borde.
///
/// No basta con que una columna sea uniforme: una carátula con fondo liso
/// también lo sería. Por eso se compara contra el color exacto del borde.
fn detect_bars(image: &DynamicImage) -> Option<(u32, u32)> {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();

    if width < 16 || height < 16 {
        return None;
    }

    let rows: Vec<u32> = (0..=32)
        .map(|index| (index * height / 32).min(height - 1))
        .collect();

    let left_color = *rgb.get_pixel(0, rows[0]);
    let mut left = 0;
    while left < width / 2 && column_matches(&rgb, left, &rows, left_color) {
        left += 1;
    }

    let right_color = *rgb.get_pixel(width - 1, rows[0]);
    let mut right = 0;
    while right < width / 2 && column_matches(&rgb, width - 1 - right, &rows, right_color) {
        right += 1;
    }

    let minimum_bar = (width / 20).max(2);

    if left < minimum_bar || right < minimum_bar || left + right >= width {
        return None;
    }

    Some((left, right))
}

fn column_matches(image: &RgbImage, x: u32, rows: &[u32], color: Rgb<u8>) -> bool {
    rows.iter()
        .all(|&y| pixels_close(*image.get_pixel(x, y), color))
}

fn pixels_close(left: Rgb<u8>, right: Rgb<u8>) -> bool {
    left.0
        .iter()
        .zip(right.0.iter())
        .all(|(left, right)| left.abs_diff(*right) <= BAR_TOLERANCE)
}

fn is_squareish(image: &DynamicImage, left: u32, right: u32) -> bool {
    let (width, height) = image.dimensions();
    let interior = f32::from(u16::try_from(width - left - right).unwrap_or(u16::MAX));
    let height = f32::from(u16::try_from(height).unwrap_or(u16::MAX));

    (interior / height - 1.0).abs() <= 0.05
}

/// Encaja la imagen completa dentro del lienzo, sin recortar.
fn fit_cover(image: &DynamicImage, width: u32, height: u32) -> DynamicImage {
    let background = border_color(image);
    let fitted = image.resize(width, height, FilterType::Lanczos3).to_rgb8();
    let mut canvas = RgbImage::from_pixel(width, height, background);

    let x = (width.saturating_sub(fitted.width())) / 2;
    let y = (height.saturating_sub(fitted.height())) / 2;
    image::imageops::overlay(&mut canvas, &fitted, i64::from(x), i64::from(y));

    DynamicImage::ImageRgb8(canvas)
}

fn border_color(image: &DynamicImage) -> Rgb<u8> {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();

    if width == 0 || height == 0 {
        return Rgb([0, 0, 0]);
    }

    let corners = [
        *rgb.get_pixel(0, 0),
        *rgb.get_pixel(width - 1, 0),
        *rgb.get_pixel(0, height - 1),
        *rgb.get_pixel(width - 1, height - 1),
    ];

    let mut red = 0u32;
    let mut green = 0u32;
    let mut blue = 0u32;

    for color in corners {
        red += u32::from(color.0[0]);
        green += u32::from(color.0[1]);
        blue += u32::from(color.0[2]);
    }

    Rgb([
        u8::try_from(red / 4).unwrap_or(0),
        u8::try_from(green / 4).unwrap_or(0),
        u8::try_from(blue / 4).unwrap_or(0),
    ])
}

fn write_jpeg(path: &Path, image: &DynamicImage) -> Result<(), String> {
    let mut file = fs::File::create(path)
        .map_err(|error| format!("no se pudo crear {}: {error}", path.display()))?;

    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, JPEG_QUALITY)
        .encode_image(image)
        .map_err(|error| format!("no se pudo escribir {}: {error}", path.display()))
}

/// Busca una portada local en la carpeta del archivo.
#[must_use]
pub fn local_cover(directory: &Path) -> Option<Vec<u8>> {
    let entries = fs::read_dir(directory).ok()?;
    let names: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_lowercase(),
                entry.path(),
            )
        })
        .collect();

    LOCAL_COVERS.iter().find_map(|candidate| {
        names
            .iter()
            .find(|(name, _)| name == candidate)
            .and_then(|(_, path)| read_local_cover(path))
    })
}

/// Lee una portada local sin cargar más de `MAX_COVER_BYTES` en memoria.
fn read_local_cover(path: &Path) -> Option<Vec<u8>> {
    let metadata = fs::metadata(path).ok()?;
    let limit = u64::try_from(MAX_COVER_BYTES).ok()?;

    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }

    let file = fs::File::open(path).ok()?;
    let mut data = Vec::with_capacity(usize::try_from(metadata.len()).ok()?);

    file.take(limit.saturating_add(1))
        .read_to_end(&mut data)
        .ok()?;

    (data.len() <= MAX_COVER_BYTES).then_some(data)
}

/// Prefiere la portada incrustada; si no hay, busca una local.
pub fn prepare(
    picture: Option<&[u8]>,
    directory: &Path,
    cache: &Path,
    target: CoverTarget,
) -> Option<String> {
    picture
        .and_then(|data| ensure_thumbnail(cache, data, target))
        .or_else(|| local_cover(directory).and_then(|data| ensure_thumbnail(cache, &data, target)))
}

/// Ruta de la miniatura para un hash y tipo de contenido.
#[must_use]
pub fn thumbnail_path(cache: &Path, hash: &str, target: CoverTarget) -> PathBuf {
    cache.join(format!(
        "{hash}-{THUMBNAIL_VERSION}-{}x{}.jpg",
        THUMBNAIL_WIDTH,
        target.height()
    ))
}

/// Indica si la miniatura ya existe y no está vacía.
#[must_use]
pub fn has_thumbnail(cache: &Path, hash: &str, target: CoverTarget) -> bool {
    thumbnail_path(cache, hash, target)
        .metadata()
        .is_ok_and(|metadata| metadata.len() > 0)
}

fn hex_sha256(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut hash = String::with_capacity(digest.len() * 2);

    for byte in digest {
        let _ = write!(hash, "{byte:02x}");
    }

    hash
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};

    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

    use super::{
        CoverTarget, MAX_COVER_BYTES, THUMBNAIL_WIDTH, border_color, dimensions_allowed,
        ensure_thumbnail, local_cover, migrate_legacy_cache_from, prepare, thumbnail_path,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mi-catalogo-covers-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png(width: u32, height: u32, color: Rgb<u8>) -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, color));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn jpeg(width: u32, height: u32, color: Rgb<u8>) -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, color));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .unwrap();
        bytes
    }

    fn with_bars(width: u32, height: u32, bar: Rgb<u8>, interior: Rgb<u8>) -> Vec<u8> {
        let mut image = RgbImage::from_pixel(width, height, bar);

        for y in 0..height {
            for x in (width / 2 - height / 2)..(width / 2 + height / 2) {
                image.put_pixel(x, y, interior);
            }
        }

        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn thumbnail_count(cache: &Path) -> usize {
        fs::read_dir(cache).map_or(0, |entries| entries.filter_map(Result::ok).count())
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;

        for byte in data {
            crc ^= u32::from(*byte);

            for _ in 0..8 {
                let mask = 0u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }

        !crc
    }

    /// PNG mínimo con solo la cabecera IHDR y el final IEND.
    ///
    /// Sirve para probar el límite de píxeles sin reservar el búfer de la
    /// imagen: el decodificador solo lee las dimensiones declaradas.
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::with_capacity(17);
        ihdr.extend_from_slice(b"IHDR");
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(&ihdr);
        png.extend_from_slice(&crc32(&ihdr).to_be_bytes());
        png.extend_from_slice(&0u32.to_be_bytes());
        png.extend_from_slice(b"IEND");
        png.extend_from_slice(&crc32(b"IEND").to_be_bytes());
        png
    }

    #[test]
    fn genera_miniatura_cuadrada_y_recorta_las_barras() {
        let dir = temp_dir("miniatura-cuadrada");
        let cache = dir.join("cache");
        let bar = Rgb([200, 20, 20]);
        let interior = Rgb([20, 80, 200]);
        let data = with_bars(1280, 720, bar, interior);

        let hash = ensure_thumbnail(&cache, &data, CoverTarget::Music).unwrap();
        let path = thumbnail_path(&cache, &hash, CoverTarget::Music);
        let image = image::open(&path).unwrap();

        assert_eq!(image.width(), THUMBNAIL_WIDTH);
        assert_eq!(image.height(), THUMBNAIL_WIDTH);
        let center = image
            .to_rgb8()
            .get_pixel(THUMBNAIL_WIDTH / 2, THUMBNAIL_WIDTH / 2)
            .0;
        assert!(center[2] > center[0], "el centro debe ser el interior azul");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn genera_miniaturas_2_3_para_video_y_libro() {
        let dir = temp_dir("miniatura-2-3");
        let cache = dir.join("cache");
        let data = png(1280, 720, Rgb([90, 90, 90]));

        for target in [CoverTarget::Video, CoverTarget::Book] {
            let hash = ensure_thumbnail(&cache, &data, target).unwrap();
            let image = image::open(thumbnail_path(&cache, &hash, target)).unwrap();
            assert_eq!(image.width(), THUMBNAIL_WIDTH);
            assert_eq!(image.height(), THUMBNAIL_WIDTH * 3 / 2);
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sin_barras_usa_encaje_fit() {
        let dir = temp_dir("sin-barras");
        let cache = dir.join("cache");
        let data = png(1280, 720, Rgb([30, 120, 60]));

        let hash = ensure_thumbnail(&cache, &data, CoverTarget::Music).unwrap();
        let image = image::open(thumbnail_path(&cache, &hash, CoverTarget::Music)).unwrap();

        assert_eq!(image.width(), THUMBNAIL_WIDTH);
        assert_eq!(image.height(), THUMBNAIL_WIDTH);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn deduplica_portadas_identicas_por_tipo() {
        let dir = temp_dir("dedupe");
        let cache = dir.join("cache");
        let data = with_bars(1280, 720, Rgb([1, 2, 3]), Rgb([4, 5, 6]));

        let first = ensure_thumbnail(&cache, &data, CoverTarget::Music).unwrap();
        let second = ensure_thumbnail(&cache, &data, CoverTarget::Music).unwrap();

        assert_eq!(first, second);
        assert_eq!(thumbnail_count(&cache), 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ignora_imagenes_invalidas() {
        let dir = temp_dir("invalida");
        let cache = dir.join("cache");

        assert_eq!(
            ensure_thumbnail(&cache, b"no es una imagen", CoverTarget::Music),
            None
        );
        assert_eq!(thumbnail_count(&cache), 0);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rechaza_portadas_que_superan_el_tope_de_bytes() {
        let dir = temp_dir("limite-bytes");
        let cache = dir.join("cache");
        let data = vec![0u8; MAX_COVER_BYTES + 1];

        assert_eq!(ensure_thumbnail(&cache, &data, CoverTarget::Music), None);
        assert_eq!(thumbnail_count(&cache), 0);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rechaza_demasiados_pixeles_sin_decodificar_la_imagen() {
        let dir = temp_dir("limite-pixeles");
        let cache = dir.join("cache");
        // 100 000 × 100 000 = 10 000 000 000 píxeles. Solo se escribe la
        // cabecera PNG; nunca se asigna ese búfer en la prueba.
        let data = png_header(100_000, 100_000);

        assert_eq!(ensure_thumbnail(&cache, &data, CoverTarget::Music), None);
        assert_eq!(thumbnail_count(&cache), 0);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn valida_dimensiones_positivas_y_el_tope_de_pixeles() {
        assert!(!dimensions_allowed(0, 0));
        assert!(!dimensions_allowed(0, 1));
        assert!(!dimensions_allowed(1, 0));
        assert!(dimensions_allowed(4000, 2000));
        assert!(!dimensions_allowed(4000, 2001));
    }

    #[test]
    fn border_color_no_paniquea_con_imagen_vacia() {
        let empty = DynamicImage::ImageRgb8(RgbImage::new(0, 0));

        assert_eq!(border_color(&empty), Rgb([0, 0, 0]));
    }

    #[test]
    fn ignora_cover_local_demasiado_grande_sin_leerlo() {
        let dir = temp_dir("local-grande");
        let album = dir.join("album");
        fs::create_dir_all(&album).unwrap();
        fs::write(album.join("cover.jpg"), vec![0u8; MAX_COVER_BYTES + 1]).unwrap();

        assert_eq!(local_cover(&album), None);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn usa_portada_local_solo_si_no_hay_incrustada() {
        let dir = temp_dir("local");
        let album = dir.join("album");
        let cache = dir.join("cache");
        fs::create_dir_all(&album).unwrap();
        let local = jpeg(640, 360, Rgb([200, 100, 50]));
        fs::write(album.join("cover.jpg"), &local).unwrap();

        let local_hash = prepare(None, &album, &cache, CoverTarget::Music).unwrap();
        assert!(thumbnail_path(&cache, &local_hash, CoverTarget::Music).exists());
        assert_eq!(local_cover(&album).unwrap(), local);

        let embedded = png(1280, 720, Rgb([10, 20, 30]));
        let embedded_hash = prepare(Some(&embedded), &album, &cache, CoverTarget::Music).unwrap();
        assert_ne!(local_hash, embedded_hash);
        assert_eq!(thumbnail_count(&cache), 2);
        assert_eq!(fs::read(album.join("cover.jpg")).unwrap(), local);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn migra_el_cache_desde_cada_id_anterior() {
        assert_eq!(crate::model::LEGACY_APP_IDS[0], "com.diegomendoza.Contento");

        for (index, legacy) in crate::model::LEGACY_APP_IDS.iter().enumerate() {
            let base = temp_dir(&format!("cache-migracion-{index}"));
            let old = base.join(legacy).join("covers");
            fs::create_dir_all(&old).unwrap();
            fs::write(old.join("portada.jpg"), legacy.as_bytes()).unwrap();

            let new = base.join("nuevo/covers");
            migrate_legacy_cache_from(&new, &base);

            assert_eq!(
                fs::read(new.join("portada.jpg")).unwrap(),
                legacy.as_bytes()
            );

            let _ = fs::remove_dir_all(base);
        }
    }

    #[test]
    fn prefiere_el_cache_mas_reciente() {
        let base = temp_dir("cache-preferencia");
        let recent = base.join(crate::model::LEGACY_APP_IDS[0]).join("covers");
        let old = base.join(crate::model::LEGACY_APP_IDS[1]).join("covers");
        fs::create_dir_all(&recent).unwrap();
        fs::create_dir_all(&old).unwrap();
        fs::write(recent.join("portada.jpg"), b"reciente").unwrap();
        fs::write(old.join("portada.jpg"), b"antigua").unwrap();

        let new = base.join("nuevo/covers");
        migrate_legacy_cache_from(&new, &base);

        assert_eq!(fs::read(new.join("portada.jpg")).unwrap(), b"reciente");

        let _ = fs::remove_dir_all(base);
    }
}
