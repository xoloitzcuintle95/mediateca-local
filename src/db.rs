// SPDX-License-Identifier: MIT

//! Persistencia en SQLite.
//!
//! La aplicación solo lee los archivos del usuario. La base de datos vive en
//! el directorio de datos XDG, nunca dentro del proyecto.

use std::collections::HashMap;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use rusqlite::{Connection, Transaction, params};

use crate::covers;
use crate::epub;
use crate::library::{AliasMap, Library};
use crate::model::{
    Kind, LEGACY_APP_IDS, MediaItem, MediaOrigin, ScanOutcome, ScanSummary, ScanWarning, TagStatus,
    TrackMetadata,
};
use crate::scanner;
use crate::tags;
use crate::video_tags;

pub use crate::model::APP_ID;

const DB_FILE: &str = "catalogo.db";
const SCHEMA_VERSION: i64 = 4;

const FILES_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS files (
  id                 INTEGER PRIMARY KEY,
  path               TEXT NOT NULL UNIQUE,
  name               TEXT NOT NULL,
  kind               TEXT NOT NULL CHECK (kind IN ('video', 'music', 'book')),
  creator            TEXT,
  collection_creator TEXT,
  collection         TEXT,
  title              TEXT,
  track_number       INTEGER,
  track_total        INTEGER,
  filename_track     INTEGER,
  year               INTEGER,
  cover_hash         TEXT,
  tag_status         TEXT NOT NULL DEFAULT 'missing',
  mtime              INTEGER NOT NULL DEFAULT 0,
  size               INTEGER NOT NULL DEFAULT 0
);
";

const RENAMED_COLUMNS: &[(&str, &str)] = &[
    ("artist", "creator"),
    ("album_artist", "collection_creator"),
    ("album", "collection"),
];

const ADDED_COLUMNS: &[(&str, &str)] = &[
    ("creator", "creator TEXT"),
    ("collection_creator", "collection_creator TEXT"),
    ("collection", "collection TEXT"),
    ("title", "title TEXT"),
    ("track_number", "track_number INTEGER"),
    ("track_total", "track_total INTEGER"),
    ("filename_track", "filename_track INTEGER"),
    ("year", "year INTEGER"),
    ("cover_hash", "cover_hash TEXT"),
    ("tag_status", "tag_status TEXT NOT NULL DEFAULT 'missing'"),
    ("mtime", "mtime INTEGER NOT NULL DEFAULT 0"),
    ("size", "size INTEGER NOT NULL DEFAULT 0"),
];

const ALIAS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS artist_aliases (
  alias     TEXT PRIMARY KEY COLLATE NOCASE,
  canonical TEXT NOT NULL
);
";

const INDEX_SCHEMA: &str = "
DROP INDEX IF EXISTS idx_files_album;
CREATE INDEX IF NOT EXISTS idx_files_kind ON files(kind);
CREATE INDEX IF NOT EXISTS idx_files_collection ON files(collection_creator, collection, year);
";

/// Devuelve `~/.local/share/com.xoloitzcuintle95.MediatecaLocal`.
pub fn data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|dir| dir.join(APP_ID))
        .ok_or_else(|| "No se pudo determinar el directorio de datos del usuario".to_owned())
}

/// Ruta completa de `catalogo.db`.
pub fn db_path() -> Result<PathBuf, String> {
    let path = data_dir()?.join(DB_FILE);
    migrate_legacy_database(&path)?;
    Ok(path)
}

/// Copia el catálogo de una versión anterior si el actual todavía no existe.
fn migrate_legacy_database(new_path: &Path) -> Result<(), String> {
    let Some(base) = dirs::data_dir() else {
        return Ok(());
    };

    migrate_legacy_database_from(new_path, &base)
}

fn migrate_legacy_database_from(new_path: &Path, base: &Path) -> Result<(), String> {
    if new_path.exists() {
        return Ok(());
    }

    for legacy in LEGACY_APP_IDS {
        let old_path = base.join(legacy).join(DB_FILE);

        if old_path.exists() {
            if let Some(parent) = new_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("No se pudo crear {}: {error}", parent.display()))?;
            }

            fs::copy(&old_path, new_path).map_err(|error| {
                format!(
                    "No se pudo migrar el catálogo {} a {}: {error}",
                    old_path.display(),
                    new_path.display()
                )
            })?;

            eprintln!(
                "catálogo migrado desde {} a {}",
                old_path.display(),
                new_path.display()
            );
            return Ok(());
        }
    }

    Ok(())
}

fn open_at(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("No se pudo crear {}: {error}", parent.display()))?;
    }

    let connection = Connection::open(path)
        .map_err(|error| format!("No se pudo abrir {}: {error}", path.display()))?;

    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("No se pudo configurar el timeout de SQLite: {error}"))?;
    connection
        .execute_batch("PRAGMA journal_mode = WAL;")
        .map_err(|error| format!("No se pudo activar WAL: {error}"))?;

    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("No se pudo leer la versión de la base: {error}"))?;

    if version < SCHEMA_VERSION {
        connection
            .execute_batch(FILES_SCHEMA)
            .map_err(|error| format!("No se pudo preparar la base de datos: {error}"))?;
        migrate(&connection)?;
    }

    Ok(connection)
}

/// Migra una base de Fase 1 o crea la estructura de Fase 2.
fn migrate(connection: &Connection) -> Result<(), String> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("No se pudo leer la versión de la base: {error}"))?;

    let mut columns = column_names(connection, "files")?;

    for (old, new) in RENAMED_COLUMNS {
        if columns.iter().any(|column| column == old) && !columns.iter().any(|column| column == new)
        {
            connection
                .execute_batch(&format!("ALTER TABLE files RENAME COLUMN {old} TO {new};"))
                .map_err(|error| format!("No se pudo renombrar {old} a {new}: {error}"))?;
        }
    }

    columns = column_names(connection, "files")?;

    for (name, definition) in ADDED_COLUMNS {
        if !columns.iter().any(|column| column == name) {
            connection
                .execute_batch(&format!("ALTER TABLE files ADD COLUMN {definition};"))
                .map_err(|error| format!("No se pudo agregar la columna {name}: {error}"))?;
        }
    }

    connection
        .execute_batch(ALIAS_SCHEMA)
        .map_err(|error| format!("No se pudo crear la tabla de alias: {error}"))?;

    if version < 2 {
        connection
            .execute(
                "INSERT OR IGNORE INTO artist_aliases (alias, canonical) VALUES (?1, ?2)",
                params!["iamdoechii", "Doechii"],
            )
            .map_err(|error| format!("No se pudo precargar el alias inicial: {error}"))?;
    }

    if version < SCHEMA_VERSION {
        connection
            .execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))
            .map_err(|error| format!("No se pudo guardar la versión de la base: {error}"))?;
    }

    connection
        .execute_batch(INDEX_SCHEMA)
        .map_err(|error| format!("No se pudieron crear los índices: {error}"))
}

fn column_names(connection: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| format!("No se pudo leer el esquema: {error}"))?;

    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| format!("No se pudo leer el esquema: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("No se pudo leer el esquema: {error}"))
}

/// Crea o actualiza un alias y devuelve el mapa usando una sola conexión.
pub fn set_alias_and_load(alias: &str, canonical: &str) -> Result<AliasMap, String> {
    let connection = open_at(&db_path()?)?;
    set_alias_on(&connection, alias, canonical)?;
    load_aliases_from(&connection)
}

/// Elimina un alias y devuelve el mapa usando una sola conexión.
pub fn remove_alias_and_load(alias: &str) -> Result<AliasMap, String> {
    let connection = open_at(&db_path()?)?;
    remove_alias_on(&connection, alias)?;
    load_aliases_from(&connection)
}

fn set_alias_on(connection: &Connection, alias: &str, canonical: &str) -> Result<(), String> {
    let alias = normalize_alias(alias);
    let canonical = canonical.trim();

    if alias.is_empty() || canonical.is_empty() {
        return Err("El alias y el nombre canónico no pueden estar vacíos".to_owned());
    }

    connection
        .execute(
            "INSERT INTO artist_aliases (alias, canonical) VALUES (?1, ?2)
             ON CONFLICT(alias) DO UPDATE SET canonical = excluded.canonical",
            params![alias, canonical],
        )
        .map_err(|error| format!("No se pudo guardar el alias: {error}"))?;

    Ok(())
}

fn remove_alias_on(connection: &Connection, alias: &str) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM artist_aliases WHERE alias = ?1",
            params![normalize_alias(alias)],
        )
        .map_err(|error| format!("No se pudo borrar el alias: {error}"))?;

    Ok(())
}

fn normalize_alias(alias: &str) -> String {
    alias.trim().to_lowercase()
}

fn load_aliases_from(connection: &Connection) -> Result<AliasMap, String> {
    let mut statement = connection
        .prepare("SELECT alias, canonical FROM artist_aliases")
        .map_err(|error| format!("No se pudieron leer los alias: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("No se pudieron leer los alias: {error}"))?;

    let mut aliases = HashMap::new();
    for row in rows {
        let (alias, canonical) =
            row.map_err(|error| format!("No se pudieron leer los alias: {error}"))?;
        aliases.insert(normalize_alias(&alias), canonical.trim().to_owned());
    }

    Ok(aliases)
}

/// Siembra filas sintéticas en la base (solo desarrollo; ver `crate::seed`).
///
/// Reusa `write_scan` con `allow_delete = false`: una sola transacción para
/// todas las filas, sin borrados.
///
/// # Errors
/// Si falla abrir la base o la transacción.
#[cfg(feature = "seed")]
pub fn seed_test_catalog(items: &[MediaItem]) -> Result<(), String> {
    let mut connection = open_at(&db_path()?)?;
    write_scan(&mut connection, Path::new("/"), items, false)?;
    Ok(())
}

/// Número de filas cuyo path está bajo `raiz` (idempotencia del sembrado).
///
/// # Errors
/// Si falla abrir la base.
#[cfg(feature = "seed")]
pub fn seed_filas_bajo(raiz: &Path) -> Result<usize, String> {
    let connection = open_at(&db_path()?)?;
    let prefix = raiz.join("").to_string_lossy().into_owned();
    connection
        .query_row(
            "SELECT COUNT(*) FROM files WHERE substr(path, 1, length(?1)) = ?1",
            [&prefix],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| usize::try_from(n).unwrap_or(0))
        .map_err(|error| format!("No se pudo contar el catálogo: {error}"))
}

/// Catálogo ya guardado, para mostrarlo antes del escaneo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedCatalog {
    pub items: Vec<MediaItem>,
    pub library: Library,
    pub aliases: AliasMap,
}

/// Carga lo último guardado sin recorrer el disco.
pub fn load_cached() -> Result<CachedCatalog, String> {
    load_cached_from(&db_path()?, &covers::cache_dir()?)
}

fn load_cached_from(database: &Path, cache: &Path) -> Result<CachedCatalog, String> {
    let load_guard = perf::Timer::start("load.cached");
    let connection = open_at(database)?;
    let aliases = load_aliases_from(&connection)?;
    let mut items = load_all(&connection)?;
    mark_cover_ready(&mut items, cache);
    let library = Library::build(&items, &aliases);

    Ok(CachedCatalog {
        items,
        library,
        aliases,
    })
}

/// Escanea `root`, actualiza la base y devuelve el catálogo completo.
///
/// Si la raíz no se puede leer, la base no se modifica y se devuelve el
/// catálogo guardado con un aviso. Si el recorrido tiene errores, se guardan
/// los archivos encontrados pero no se borra ninguna fila.
pub fn sync(root: &Path) -> Result<ScanOutcome, String> {
    sync_with(root, &db_path()?, &covers::cache_dir()?)
}

fn sync_with(root: &Path, database: &Path, cache: &Path) -> Result<ScanOutcome, String> {
    let mut connection = open_at(database)?;
    let aliases = load_aliases_from(&connection)?;
    let mut cached_items = load_all(&connection)?;
    mark_cover_ready(&mut cached_items, cache);
    let cached_by_path: HashMap<String, MediaItem> = cached_items
        .iter()
        .map(|item| (item.path.clone(), item.clone()))
        .collect();

    let canonical_root = match readable_root(root) {
        Ok(root) => root,
        Err(reason) => {
            let library = Library::build(&cached_items, &aliases);

            return Ok(ScanOutcome {
                summary: ScanSummary::from_items(&cached_items, 0, 0),
                items: cached_items,
                library,
                aliases,
                tag_reads: 0,
                tag_reused: 0,
                deleted: 0,
                warning: Some(ScanWarning::RootUnavailable {
                    root: root.to_string_lossy().into_owned(),
                    reason,
                }),
            });
        }
    };

    let scan = scanner::scan(&canonical_root);
    let complete = scan.summary.errors == 0;
    let has_cached_under_root = cached_items
        .iter()
        .any(|item| Path::new(&item.path).starts_with(&canonical_root));
    let mut items = scan.items;
    let empty_root = items.is_empty() && has_cached_under_root;
    let stats = enrich_items(&mut items, cache, &cached_by_path);

    let deleted = write_scan(
        &mut connection,
        &canonical_root,
        &items,
        complete && !empty_root,
    )?;
    let mut items = load_all(&connection)?;
    mark_cover_ready(&mut items, cache);
    let library = Library::build(&items, &aliases);

    Ok(ScanOutcome {
        summary: ScanSummary::from_items(&items, scan.summary.skipped, scan.summary.errors),
        items,
        library,
        aliases,
        tag_reads: stats.reads,
        tag_reused: stats.reused,
        deleted,
        warning: if !complete {
            Some(ScanWarning::Incomplete {
                errors: scan.summary.errors,
            })
        } else if empty_root {
            Some(ScanWarning::EmptyRoot)
        } else {
            None
        },
    })
}

#[derive(Clone, Copy, Debug, Default)]
struct EnrichStats {
    reads: usize,
    reused: usize,
}

/// Lee metadatos y prepara miniaturas solo para archivos nuevos o modificados.
///
/// Nada de esto escribe en la colección: lofty y rbook solo leen, y las
/// miniaturas van a `cache`. Los PDF no se procesan: se quedan con su nombre.
fn enrich_items(
    items: &mut [MediaItem],
    cache: &Path,
    cached: &HashMap<String, MediaItem>,
) -> EnrichStats {
    let reads = AtomicUsize::new(0);
    let reused = AtomicUsize::new(0);

    let workers = std::thread::available_parallelism()
        .map_or(1, |count| count.get().saturating_sub(1))
        .clamp(1, 4)
        .min(items.len().max(1));
    let chunk_size = items.len().div_ceil(workers).max(1);

    std::thread::scope(|scope| {
        for chunk in items.chunks_mut(chunk_size) {
            scope.spawn(|| {
                for item in chunk
                    .iter_mut()
                    .filter(|item| matches!(item.kind, Kind::Music | Kind::Book | Kind::Video))
                {
                    if let Some(previous) = cached.get(&item.path)
                        && can_reuse_metadata(previous, item)
                    {
                        item.metadata = previous.metadata.clone();
                        item.cover_ready = previous.cover_ready;
                        reused.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }

                    if enrich_item(item, cache) {
                        reads.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });

    EnrichStats {
        reads: reads.load(Ordering::Relaxed),
        reused: reused.load(Ordering::Relaxed),
    }
}

/// Enriquece un archivo. Devuelve si hubo que intentar leerlo.
///
/// Un panic queda contenido en este archivo: se marca como error y el resto
/// del escaneo continúa. Nunca se propaga al hilo que coordina la tanda.
fn enrich_item(item: &mut MediaItem, cache: &Path) -> bool {
    let is_epub = item.kind == Kind::Book
        && Path::new(&item.path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("epub"));

    // Los PDF no se procesan: se quedan con su nombre.
    if item.kind == Kind::Book && !is_epub {
        return false;
    }

    let _ = guard_item(item, |item| enrich_item_inner(item, cache));
    true
}

fn enrich_item_inner(item: &mut MediaItem, cache: &Path) {
    let path = Path::new(&item.path);

    match item.kind {
        Kind::Music => {
            let tags = tags::read(path);
            let directory = path.parent().unwrap_or_else(|| Path::new("."));
            let cover_hash = covers::prepare(
                tags.picture.as_deref(),
                directory,
                cache,
                covers::CoverTarget::Music,
            );

            item.cover_ready = cover_hash.is_some();
            item.metadata = TrackMetadata {
                cover_hash,
                ..tags.metadata
            };
        }
        Kind::Book => {
            let read = epub::read(path);
            let cover_hash = read
                .cover
                .as_deref()
                .and_then(|data| covers::ensure_thumbnail(cache, data, covers::CoverTarget::Book));

            item.cover_ready = cover_hash.is_some();
            item.metadata = TrackMetadata {
                creator: read.author,
                title: read.title,
                cover_hash,
                status: read.status,
                ..TrackMetadata::default()
            };
        }
        Kind::Video => {
            let tags = video_tags::read(path);
            // Sin portadas para video: `cover_hash` queda en None.
            item.cover_ready = false;
            item.metadata = TrackMetadata {
                title: tags.title,
                creator: tags.creator,
                collection: tags.collection,
                year: tags.year,
                status: tags.status,
                ..TrackMetadata::default()
            };
        }
    }
}

/// Ejecuta el enriquecimiento de un solo archivo y captura cualquier panic.
fn guard_item(item: &mut MediaItem, action: impl FnOnce(&mut MediaItem)) -> bool {
    if catch_unwind(AssertUnwindSafe(|| action(&mut *item))).is_err() {
        mark_item_error(item);
        false
    } else {
        true
    }
}

fn mark_item_error(item: &mut MediaItem) {
    item.cover_ready = false;
    item.metadata = TrackMetadata {
        title: Some(tags::fallback_title(Path::new(&item.path))),
        status: TagStatus::Error,
        ..TrackMetadata::default()
    };
}

fn can_reuse_metadata(previous: &MediaItem, current: &MediaItem) -> bool {
    previous.mtime == current.mtime
        && previous.size == current.size
        && previous.metadata.status != TagStatus::NotApplicable
        && (previous.metadata.cover_hash.is_none() || previous.cover_ready)
}

/// Marca una sola vez por escaneo si la miniatura ya existe.
fn mark_cover_ready(items: &mut [MediaItem], cache: &Path) {
    for item in items {
        item.cover_ready = item.metadata.cover_hash.as_deref().is_some_and(|hash| {
            covers::has_thumbnail(cache, hash, covers::CoverTarget::for_kind(item.kind))
        });
    }
}

/// Comprueba que la raíz exista, sea una carpeta y se pueda abrir.
fn readable_root(root: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(root)
        .map_err(|error| format!("No se pudo acceder a {}: {error}", root.display()))?;

    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("No se pudo leer {}: {error}", canonical.display()))?;

    if !metadata.is_dir() {
        return Err(format!("{} no es una carpeta", canonical.display()));
    }

    fs::read_dir(&canonical)
        .map_err(|error| format!("No se pudo leer {}: {error}", canonical.display()))?;

    Ok(canonical)
}

/// Guarda el resultado del escaneo en una sola transacción.
///
/// Devuelve cuántas filas obsoletas se borraron. El borrado solo ocurre si
/// `allow_delete` es verdadero y únicamente afecta rutas dentro de `root`.
fn write_scan(
    connection: &mut Connection,
    root: &Path,
    items: &[MediaItem],
    allow_delete: bool,
) -> Result<usize, String> {
    let transaction = connection
        .transaction()
        .map_err(|error| format!("No se pudo iniciar la transacción: {error}"))?;

    {
        let mut statement = transaction
            .prepare(
                "INSERT INTO files (
                   path, name, kind, creator, collection_creator, collection, title,
                   track_number, track_total, filename_track, year, cover_hash,
                   tag_status, mtime, size
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(path) DO UPDATE SET
                   name = excluded.name,
                   kind = excluded.kind,
                   creator = excluded.creator,
                   collection_creator = excluded.collection_creator,
                   collection = excluded.collection,
                   title = excluded.title,
                   track_number = excluded.track_number,
                   track_total = excluded.track_total,
                   filename_track = excluded.filename_track,
                   year = excluded.year,
                   cover_hash = excluded.cover_hash,
                   tag_status = excluded.tag_status,
                   mtime = excluded.mtime,
                   size = excluded.size",
            )
            .map_err(|error| format!("No se pudo preparar la actualización: {error}"))?;

        for item in items {
            let metadata = &item.metadata;

            statement
                .execute(params![
                    item.path,
                    item.name,
                    item.kind.as_str(),
                    metadata.creator.as_deref(),
                    metadata.collection_creator.as_deref(),
                    metadata.collection.as_deref(),
                    metadata.title.as_deref(),
                    metadata.track_number.map(i64::from),
                    metadata.track_total.map(i64::from),
                    metadata.filename_track.map(i64::from),
                    metadata.year.map(i64::from),
                    metadata.cover_hash.as_deref(),
                    metadata.status.as_str(),
                    item.mtime,
                    item.size,
                ])
                .map_err(|error| format!("No se pudo guardar {}: {error}", item.path))?;
        }
    }

    let deleted = if allow_delete {
        let inside = delete_missing(&transaction, root, items)?;
        let outside = delete_missing_outside_root(&transaction, root)?;
        inside + outside
    } else {
        0
    };

    transaction
        .commit()
        .map_err(|error| format!("No se pudo confirmar la transacción: {error}"))?;

    Ok(deleted)
}

/// Borra las filas dentro de `root` que no aparecieron en el último escaneo.
fn delete_missing(
    transaction: &Transaction<'_>,
    root: &Path,
    items: &[MediaItem],
) -> Result<usize, String> {
    transaction
        .execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS scanned_paths (path TEXT PRIMARY KEY);
             DELETE FROM scanned_paths;",
        )
        .map_err(|error| format!("No se pudo preparar la lista de archivos vistos: {error}"))?;

    {
        let mut statement = transaction
            .prepare("INSERT OR IGNORE INTO scanned_paths (path) VALUES (?1)")
            .map_err(|error| format!("No se pudo guardar la lista de archivos vistos: {error}"))?;

        for item in items {
            statement
                .execute([&item.path])
                .map_err(|error| format!("No se pudo guardar {}: {error}", item.path))?;
        }
    }

    let prefix = root.join("").to_string_lossy().into_owned();

    transaction
        .execute(
            "DELETE FROM files
             WHERE substr(path, 1, length(?1)) = ?1
               AND path NOT IN (SELECT path FROM scanned_paths)",
            params![prefix],
        )
        .map_err(|error| format!("No se pudieron borrar archivos obsoletos: {error}"))
}

/// Borra filas FUERA de `root` cuyo archivo ya no existe en el disco.
///
/// Esto limpia restos de raíces abandonadas (p. ej. rutas del portal de
/// Flatpak de un escaneo anterior) sin tocar archivos que siguen existiendo:
/// un usuario que cambia de raíz y regresa no pierde su catálogo.
fn delete_missing_outside_root(
    transaction: &Transaction<'_>,
    root: &Path,
) -> Result<usize, String> {
    let prefix = root.join("").to_string_lossy().into_owned();

    let rows: Vec<String> = {
        let mut statement = transaction
            .prepare("SELECT path FROM files WHERE substr(path, 1, length(?1)) <> ?1")
            .map_err(|error| format!("No se pudo leer el catálogo: {error}"))?;

        statement
            .query_map([prefix], |row| row.get::<_, String>(0))
            .map_err(|error| format!("No se pudo leer el catálogo: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("No se pudo leer el catálogo: {error}"))?
    };

    let mut deleted = 0;
    for path in rows {
        if !Path::new(&path).exists() {
            transaction
                .execute("DELETE FROM files WHERE path = ?1", [&path])
                .map_err(|error| format!("No se pudo borrar {}: {error}", path))?;
            deleted += 1;
        }
    }

    Ok(deleted)
}

fn load_all(connection: &Connection) -> Result<Vec<MediaItem>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, path, name, kind, creator, collection_creator, collection, title,
                    track_number, track_total, filename_track, year, cover_hash,
                    tag_status, mtime, size
             FROM files
             ORDER BY name COLLATE NOCASE",
        )
        .map_err(|error| format!("No se pudo preparar la lectura del catálogo: {error}"))?;

    let rows = statement
        .query_map([], |row| {
            let stored_kind: String = row.get(3)?;
            let stored_status: Option<String> = row.get(13)?;
            // El CHECK del esquema garantiza uno de los tres valores.
            let kind = Kind::from_db(&stored_kind).unwrap_or(Kind::Book);

            Ok(MediaItem {
                id: row.get(0)?,
                path: row.get(1)?,
                name: row.get(2)?,
                kind,
                mtime: row.get(14)?,
                size: row.get(15)?,
                origin: MediaOrigin::LocalFile,
                open_action: crate::media::media_type(kind).open_action,
                cover_ready: false,
                metadata: TrackMetadata {
                    creator: row.get(4)?,
                    collection_creator: row.get(5)?,
                    collection: row.get(6)?,
                    title: row.get(7)?,
                    track_number: optional_u32(row, 8)?,
                    track_total: optional_u32(row, 9)?,
                    filename_track: optional_u32(row, 10)?,
                    year: optional_u16(row, 11)?,
                    cover_hash: row.get(12)?,
                    status: stored_status
                        .as_deref()
                        .map_or(TagStatus::NotApplicable, TagStatus::from_db),
                },
            })
        })
        .map_err(|error| format!("No se pudo leer el catálogo: {error}"))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("No se pudo leer el catálogo: {error}"))
}

fn optional_u32(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u32>> {
    let value: Option<i64> = row.get(index)?;
    Ok(value.and_then(|value| u32::try_from(value).ok()))
}

fn optional_u16(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u16>> {
    Ok(optional_u32(row, index)?.and_then(|value| u16::try_from(value).ok()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use rusqlite::Connection;

    use super::{
        column_names, guard_item, load_aliases_from, load_all, load_cached_from,
        migrate_legacy_database_from, open_at, remove_alias_on, set_alias_on, sync_with,
    };
    use crate::model::{
        Kind, MediaItem, MediaOrigin, OpenAction, ScanWarning, TagStatus, TrackMetadata,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mi-catalogo-db-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    fn sync(
        root: &Path,
        database: &Path,
        base: &Path,
    ) -> Result<crate::model::ScanOutcome, String> {
        sync_with(root, database, &base.join("cache"))
    }

    #[test]
    fn un_panic_marca_solo_ese_archivo_como_error() {
        let mut item = MediaItem {
            id: 0,
            path: String::from("/tmp/cancion.mp3"),
            name: String::from("cancion.mp3"),
            kind: Kind::Music,
            mtime: 0,
            size: 0,
            origin: MediaOrigin::LocalFile,
            open_action: OpenAction::XdgOpen,
            cover_ready: true,
            metadata: TrackMetadata {
                status: TagStatus::Complete,
                ..TrackMetadata::default()
            },
        };

        let completed = guard_item(&mut item, |_item| panic!("fallo simulado"));

        assert!(!completed);
        assert_eq!(item.metadata.status, TagStatus::Error);
        assert!(!item.cover_ready);
        assert_eq!(item.metadata.title.as_deref(), Some("cancion"));
    }

    #[test]
    fn reescanear_no_duplica_y_conserva_ids() {
        let base = temp_dir("reescaneo");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("una.mp3"));
        write_file(&root.join("dos.flac"));
        write_file(&root.join("tres.mkv"));
        write_file(&root.join("cuatro.txt"));

        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 3);
        assert_eq!(first.summary.music, 2);
        assert_eq!(first.summary.video, 1);
        assert_eq!(first.summary.book, 0);
        assert_eq!(first.summary.skipped, 1);
        assert_eq!(first.deleted, 0);
        assert!(first.warning.is_none());

        let ids: Vec<i64> = first.items.iter().map(|item| item.id).collect();

        let second = sync(&root, &database, &base).unwrap();
        assert_eq!(second.summary.total(), 3);
        assert_eq!(second.deleted, 0);
        assert_eq!(
            ids,
            second.items.iter().map(|item| item.id).collect::<Vec<_>>()
        );

        let connection = Connection::open(&database).unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn carga_lo_guardado_sin_escanear() {
        let base = temp_dir("cache-carga");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");
        let cache = base.join("cache");

        write_file(&root.join("01 - Una.mp3"));
        sync(&root, &database, &base).unwrap();

        let cached = load_cached_from(&database, &cache).unwrap();
        assert_eq!(cached.items.len(), 1);
        assert_eq!(cached.library.track_count, 1);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn reescanear_solo_relee_archivos_modificados() {
        let base = temp_dir("incremental");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("a.mp3"));
        write_file(&root.join("b.mp3"));

        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.tag_reads, 2);
        assert_eq!(first.tag_reused, 0);

        let second = sync(&root, &database, &base).unwrap();
        assert_eq!(second.tag_reads, 0);
        assert_eq!(second.tag_reused, 2);

        fs::write(root.join("b.mp3"), b"contenido distinto").unwrap();

        let third = sync(&root, &database, &base).unwrap();
        assert_eq!(third.tag_reads, 1);
        assert_eq!(third.tag_reused, 1);

        let connection = Connection::open(&database).unwrap();
        let (mtime, size): (i64, i64) = connection
            .query_row(
                "SELECT mtime, size FROM files WHERE name = 'a.mp3'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let metadata = fs::metadata(root.join("a.mp3")).unwrap();

        assert!(mtime > 0);
        assert_eq!(size, i64::try_from(metadata.len()).unwrap());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn reutiliza_solo_si_la_miniatura_esta_lista() {
        let base = temp_dir("miniatura-faltante");
        let mut previous = crate::model::MediaItem {
            id: 1,
            path: String::from("/musica/x.mp3"),
            name: String::from("x.mp3"),
            kind: crate::model::Kind::Music,
            mtime: 1,
            size: 10,
            origin: crate::model::MediaOrigin::LocalFile,
            open_action: crate::model::OpenAction::XdgOpen,
            cover_ready: false,
            metadata: crate::model::TrackMetadata {
                cover_hash: Some(String::from("abc")),
                status: crate::model::TagStatus::Complete,
                ..crate::model::TrackMetadata::default()
            },
        };
        let current = crate::model::MediaItem {
            metadata: crate::model::TrackMetadata::default(),
            ..previous.clone()
        };

        assert!(!super::can_reuse_metadata(&previous, &current));

        previous.cover_ready = true;
        assert!(super::can_reuse_metadata(&previous, &current));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn borra_solo_lo_que_ya_no_existe() {
        let base = temp_dir("borrado");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("se-queda.mp3"));
        write_file(&root.join("se-va.mp3"));

        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 2);

        fs::remove_file(root.join("se-va.mp3")).unwrap();

        let second = sync(&root, &database, &base).unwrap();
        assert_eq!(second.summary.total(), 1);
        assert_eq!(second.deleted, 1);
        assert_eq!(second.items[0].name, "se-queda.mp3");
        assert!(second.warning.is_none());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn borra_fuera_de_la_raiz_solo_si_el_archivo_ya_no_existe() {
        let base = temp_dir("fuera-huerfano");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");
        let vieja_raiz = base.join("vieja");

        // Escaneo viejo: dos archivos bajo otra raíz.
        write_file(&vieja_raiz.join("sigue-existiendo.mp3"));
        write_file(&vieja_raiz.join("ya-no-existe.mp3"));
        let primera = sync(&vieja_raiz, &database, &base).unwrap();
        assert_eq!(primera.summary.total(), 2);

        // Se elimina un archivo y se cambia de raíz.
        fs::remove_file(vieja_raiz.join("ya-no-existe.mp3")).unwrap();
        write_file(&root.join("nuevo.mp3"));
        let segunda = sync(&root, &database, &base).unwrap();

        assert_eq!(segunda.deleted, 1);
        let connection = Connection::open(&database).unwrap();
        let paths: Vec<String> = super::load_all(&connection)
            .unwrap()
            .into_iter()
            .map(|item| item.path)
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("nuevo.mp3")));
        assert!(paths.iter().any(|p| p.ends_with("sigue-existiendo.mp3")));
        assert!(!paths.iter().any(|p| p.ends_with("ya-no-existe.mp3")));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn no_borra_nada_fuera_de_la_raiz() {
        let base = temp_dir("fuera");
        let root_a = base.join("catalogo-a");
        let root_b = base.join("catalogo-b");
        let database = base.join("catalogo.db");

        write_file(&root_a.join("a.mp3"));
        write_file(&root_b.join("b.mp3"));

        let first = sync(&root_a, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 1);

        let second = sync(&root_b, &database, &base).unwrap();
        assert_eq!(second.summary.total(), 2);
        assert_eq!(second.deleted, 0);
        assert!(second.items.iter().any(|item| item.name == "a.mp3"));
        assert!(second.items.iter().any(|item| item.name == "b.mp3"));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn no_borra_si_la_raiz_no_existe() {
        let base = temp_dir("sin-raiz");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("guardado.mp3"));

        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 1);

        fs::remove_dir_all(&root).unwrap();

        let second = sync(&root, &database, &base).unwrap();
        assert_eq!(second.summary.total(), 1);
        assert_eq!(second.deleted, 0);
        assert!(matches!(
            second.warning,
            Some(ScanWarning::RootUnavailable { .. })
        ));

        let connection = Connection::open(&database).unwrap();
        assert_eq!(load_all(&connection).unwrap().len(), 1);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn no_borra_si_la_raiz_queda_vacia() {
        let base = temp_dir("raiz-vacia");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("una.mp3"));
        write_file(&root.join("dos.mkv"));

        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 2);

        fs::remove_file(root.join("una.mp3")).unwrap();
        fs::remove_file(root.join("dos.mkv")).unwrap();

        let second = sync(&root, &database, &base).unwrap();

        assert_eq!(second.summary.total(), 2);
        assert_eq!(second.deleted, 0);
        assert!(matches!(second.warning, Some(ScanWarning::EmptyRoot)));

        let connection = Connection::open(&database).unwrap();
        assert_eq!(load_all(&connection).unwrap().len(), 2);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn no_avisa_si_la_raiz_vacia_tambien_tiene_base_vacia() {
        let base = temp_dir("raiz-vacia-sin-base");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        fs::create_dir_all(&root).unwrap();

        let outcome = sync(&root, &database, &base).unwrap();

        assert_eq!(outcome.summary.total(), 0);
        assert!(outcome.warning.is_none());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn sigue_borrando_ausentes_con_archivos_presentes() {
        let base = temp_dir("borrado-parcial");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("una.mp3"));
        write_file(&root.join("dos.mp3"));
        write_file(&root.join("tres.mkv"));

        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 3);

        fs::remove_file(root.join("dos.mp3")).unwrap();

        let second = sync(&root, &database, &base).unwrap();

        assert_eq!(second.summary.total(), 2);
        assert_eq!(second.deleted, 1);
        assert!(second.warning.is_none());

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn guarda_nombres_con_acentos_y_caracteres_raros() {
        let base = temp_dir("nombres");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("17 - What's Your Name？.mp3"));
        write_file(&root.join("canción.flac"));

        let outcome = sync(&root, &database, &base).unwrap();
        assert_eq!(outcome.summary.total(), 2);
        assert!(
            outcome
                .items
                .iter()
                .any(|item| item.name == "17 - What's Your Name？.mp3")
        );
        assert!(outcome.items.iter().any(|item| item.name == "canción.flac"));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn guarda_metadatos_de_respaldo_y_construye_la_biblioteca() {
        let base = temp_dir("metadatos");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("57 - WAIT.mp3"));

        let outcome = sync(&root, &database, &base).unwrap();
        let item = outcome
            .items
            .iter()
            .find(|item| item.name == "57 - WAIT.mp3")
            .unwrap();

        assert_eq!(item.metadata.title.as_deref(), Some("WAIT"));
        assert_eq!(item.metadata.filename_track, Some(57));
        assert_eq!(item.metadata.status, crate::model::TagStatus::Error);
        assert_eq!(outcome.library.track_count, 1);
        assert_eq!(outcome.library.albums.len(), 1);
        assert_eq!(outcome.library.albums[0].tracks[0].title, "WAIT");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn migra_el_catalogo_desde_cada_id_anterior() {
        assert_eq!(crate::model::LEGACY_APP_IDS[0], "com.diegomendoza.Contento");

        for (index, legacy) in crate::model::LEGACY_APP_IDS.iter().enumerate() {
            let base = temp_dir(&format!("migracion-id-{index}"));
            let old = base.join(legacy).join("catalogo.db");
            let new = base.join(crate::model::APP_ID).join("catalogo.db");
            fs::create_dir_all(old.parent().unwrap()).unwrap();
            fs::write(&old, legacy.as_bytes()).unwrap();

            migrate_legacy_database_from(&new, &base).unwrap();

            assert_eq!(fs::read(&new).unwrap(), legacy.as_bytes());

            // No se sobrescribe un catálogo nuevo que ya exista.
            fs::write(&new, b"nuevo").unwrap();
            migrate_legacy_database_from(&new, &base).unwrap();
            assert_eq!(fs::read(&new).unwrap(), b"nuevo");

            let _ = fs::remove_dir_all(base);
        }
    }

    #[test]
    fn prefiere_el_catalogo_mas_reciente() {
        let base = temp_dir("migracion-preferencia");
        let recent = base
            .join(crate::model::LEGACY_APP_IDS[0])
            .join("catalogo.db");
        let old = base
            .join(crate::model::LEGACY_APP_IDS[1])
            .join("catalogo.db");
        let new = base.join(crate::model::APP_ID).join("catalogo.db");
        fs::create_dir_all(recent.parent().unwrap()).unwrap();
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&recent, b"reciente").unwrap();
        fs::write(&old, b"antiguo").unwrap();

        migrate_legacy_database_from(&new, &base).unwrap();

        assert_eq!(fs::read(&new).unwrap(), b"reciente");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn migra_el_catalogo_sin_perder_filas() {
        let base = temp_dir("migracion-filas");
        let old = base
            .join(crate::model::LEGACY_APP_IDS[0])
            .join("catalogo.db");
        let new = base.join(crate::model::APP_ID).join("catalogo.db");
        fs::create_dir_all(old.parent().unwrap()).unwrap();

        {
            let connection = Connection::open(&old).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE files (
                       id   INTEGER PRIMARY KEY,
                       path TEXT NOT NULL UNIQUE,
                       name TEXT NOT NULL,
                       kind TEXT NOT NULL
                     );
                     INSERT INTO files (path, name, kind) VALUES
                       ('/musica/uno.mp3', 'uno.mp3', 'music'),
                       ('/musica/dos.mp3', 'dos.mp3', 'music');",
                )
                .unwrap();
        }

        migrate_legacy_database_from(&new, &base).unwrap();

        let connection = open_at(&new).unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn renombra_columnas_sin_perder_datos() {
        let base = temp_dir("renombrado");
        let database = base.join("catalogo.db");

        {
            let connection = Connection::open(&database).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE files (
                       id INTEGER PRIMARY KEY,
                       path TEXT NOT NULL UNIQUE,
                       name TEXT NOT NULL,
                       kind TEXT NOT NULL,
                       artist TEXT,
                       album_artist TEXT,
                       album TEXT,
                       title TEXT
                     );
                     INSERT INTO files
                       (path, name, kind, artist, album_artist, album, title)
                     VALUES
                       ('/musica/uno.mp3', 'uno.mp3', 'music',
                        'Doechii, JT', 'Doechii', 'Alligator', 'Uno');
                     PRAGMA user_version = 3;",
                )
                .unwrap();
        }

        let connection = open_at(&database).unwrap();
        let columns = column_names(&connection, "files").unwrap();
        assert!(columns.iter().any(|column| column == "creator"));
        assert!(columns.iter().any(|column| column == "collection"));
        assert!(columns.iter().any(|column| column == "collection_creator"));
        assert!(!columns.iter().any(|column| column == "artist"));

        let (creator, collection_creator, collection, title): (String, String, String, String) =
            connection
                .query_row(
                    "SELECT creator, collection_creator, collection, title
                 FROM files WHERE path = '/musica/uno.mp3'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .unwrap();

        assert_eq!(creator, "Doechii, JT");
        assert_eq!(collection_creator, "Doechii");
        assert_eq!(collection, "Alligator");
        assert_eq!(title, "Uno");

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn migra_el_esquema_de_fase_1_y_conserva_las_filas() {
        let base = temp_dir("migracion");
        let database = base.join("catalogo.db");

        {
            let connection = Connection::open(&database).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE files (
                       id   INTEGER PRIMARY KEY,
                       path TEXT NOT NULL UNIQUE,
                       name TEXT NOT NULL,
                       kind TEXT NOT NULL
                     );
                     INSERT INTO files (path, name, kind)
                     VALUES ('/musica/vieja.mp3', 'vieja.mp3', 'music');",
                )
                .unwrap();
        }

        let connection = open_at(&database).unwrap();
        let columns = column_names(&connection, "files").unwrap();

        for column in [
            "creator",
            "collection_creator",
            "collection",
            "cover_hash",
            "tag_status",
        ] {
            assert!(columns.iter().any(|name| name == column));
        }

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 4);

        let aliases = load_aliases_from(&connection).unwrap();
        assert_eq!(
            aliases.get("iamdoechii").map(String::as_str),
            Some("Doechii")
        );

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn permite_editar_alias_sin_distinguir_mayusculas() {
        let base = temp_dir("alias");
        let database = base.join("catalogo.db");
        let connection = open_at(&database).unwrap();

        set_alias_on(&connection, "IAMDOECHII", "Doechii").unwrap();
        set_alias_on(&connection, "artista raro", "Artista Común").unwrap();

        let aliases = load_aliases_from(&connection).unwrap();
        assert_eq!(
            aliases.get("iamdoechii").map(String::as_str),
            Some("Doechii")
        );
        assert_eq!(
            aliases.get("artista raro").map(String::as_str),
            Some("Artista Común")
        );

        remove_alias_on(&connection, "ARTISTA RARO").unwrap();
        let aliases = load_aliases_from(&connection).unwrap();
        assert!(!aliases.contains_key("artista raro"));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn no_escribe_en_los_archivos_del_catalogo() {
        let base = temp_dir("solo-lectura");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");
        let path = root.join("57 - WAIT.mp3");
        write_file(&path);
        let before = fs::read(&path).unwrap();

        let _ = sync(&root, &database, &base).unwrap();

        assert_eq!(fs::read(&path).unwrap(), before);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn enriquece_videos_con_respaldo_y_marca_estado() {
        let base = temp_dir("video-tags");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        // Sin lector de tags para .avi: respaldo por nombre de archivo.
        write_file(&root.join("Pelicula.1998.avi"));

        let outcome = sync(&root, &database, &base).unwrap();
        let video = outcome
            .items
            .iter()
            .find(|item| item.name == "Pelicula.1998.avi")
            .unwrap();

        assert_eq!(video.metadata.year, Some(1998));
        assert_eq!(video.metadata.status, TagStatus::Missing);

        // Reescaneo: con estado real, la metadata se reutiliza.
        let second = sync(&root, &database, &base).unwrap();
        assert_eq!(second.tag_reused, 1);
        assert_eq!(second.tag_reads, 0);

        let _ = fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn no_borra_si_el_escaneo_es_incompleto() {
        use std::os::unix::fs::PermissionsExt;

        let base = temp_dir("incompleto");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("visible.mp3"));
        write_file(&root.join("se-va.mp3"));

        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 2);

        let blocked = root.join("bloqueada");
        fs::create_dir(&blocked).unwrap();
        write_file(&blocked.join("secreto.mp3"));
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o000)).unwrap();

        // Si somos root, el sistema ignora los permisos y no se puede simular.
        if fs::read_dir(&blocked).is_ok() {
            fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755)).unwrap();
            let _ = fs::remove_dir_all(base);
            return;
        }

        fs::remove_file(root.join("se-va.mp3")).unwrap();

        let second = sync(&root, &database, &base).unwrap();

        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(second.summary.errors, 1);
        assert_eq!(second.deleted, 0);
        assert_eq!(second.summary.total(), 2);
        assert!(matches!(
            second.warning,
            Some(ScanWarning::Incomplete { errors: 1 })
        ));

        let _ = fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn una_ruta_no_utf8_marca_el_escaneo_incompleto_y_no_borra() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let base = temp_dir("ruta-no-utf8");
        let root = base.join("catalogo");
        let database = base.join("catalogo.db");

        write_file(&root.join("visible.mp3"));
        write_file(&root.join("se-va.mp3"));
        let first = sync(&root, &database, &base).unwrap();
        assert_eq!(first.summary.total(), 2);

        fs::remove_file(root.join("se-va.mp3")).unwrap();
        fs::write(root.join(OsStr::from_bytes(b"rara-\xff.mp3")), b"x").unwrap();

        let second = sync(&root, &database, &base).unwrap();

        assert_eq!(second.summary.errors, 1);
        assert_eq!(second.deleted, 0);
        assert_eq!(second.summary.total(), 2);
        assert!(matches!(
            second.warning,
            Some(ScanWarning::Incomplete { errors: 1 })
        ));

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn el_alias_inicial_esta_disponible() {
        let base = temp_dir("alias-inicial");
        let database = base.join("catalogo.db");
        let connection = open_at(&database).unwrap();
        let aliases: HashMap<String, String> = load_aliases_from(&connection).unwrap();

        assert_eq!(
            aliases.get("iamdoechii").map(String::as_str),
            Some("Doechii")
        );

        let _ = fs::remove_dir_all(base);
    }
}
