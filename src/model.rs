// SPDX-License-Identifier: MIT

//! Datos compartidos entre el escáner, la base de datos y la interfaz.

/// Identificador de la aplicación, usado para config, datos y caché.
/// Este es el único lugar del código donde se define el ID.
pub const APP_ID: &str = "com.xoloitzcuintle95.MediatecaLocal";

/// IDs anteriores que se migran la primera vez que se abre la app.
/// El orden va del más reciente al más antiguo: se usa el primero que exista.
pub const LEGACY_APP_IDS: &[&str] = &["com.diegomendoza.Contento", "com.diegomendoza.Catalogo"];

/// Tipo de archivo que el catálogo reconoce.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Video,
    Music,
    Book,
}

impl Kind {
    /// Valor que se guarda en la columna `kind` de SQLite.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Music => "music",
            Self::Book => "book",
        }
    }

    /// Convierte el valor guardado en SQLite a `Kind`.
    #[must_use]
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "video" => Some(Self::Video),
            "music" => Some(Self::Music),
            "book" => Some(Self::Book),
            _ => None,
        }
    }
}

/// Estado de las etiquetas de un archivo de música.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TagStatus {
    /// Artista, álbum y título están presentes.
    Complete,
    /// Hay algunas etiquetas, pero faltan otras.
    Partial,
    /// El archivo se pudo leer, pero no tiene etiquetas útiles.
    Missing,
    /// No se pudo leer el archivo o sus etiquetas.
    Error,
    /// El archivo no es de música; sus etiquetas no aplican.
    #[default]
    NotApplicable,
}

impl TagStatus {
    /// Valor que se guarda en la columna `tag_status` de SQLite.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Missing => "missing",
            Self::Error => "error",
            Self::NotApplicable => "n/a",
        }
    }

    /// Convierte el valor guardado en SQLite a `TagStatus`.
    #[must_use]
    pub fn from_db(value: &str) -> Self {
        match value {
            "complete" => Self::Complete,
            "partial" => Self::Partial,
            "missing" => Self::Missing,
            "error" => Self::Error,
            _ => Self::NotApplicable,
        }
    }
}

/// De dónde salió el elemento del catálogo.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum MediaOrigin {
    /// Archivo local dentro de una carpeta del catálogo.
    #[default]
    LocalFile,
}

/// Cómo se abrirá un elemento con la aplicación predeterminada del sistema.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OpenAction {
    /// Abrir con el lanzador del sistema (`xdg-open` y respaldos).
    #[default]
    XdgOpen,
}

/// Etiquetas leídas de un archivo. Son solo datos; nunca se escriben al archivo.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrackMetadata {
    /// Artista de la pista o autor del libro. Es genérico para todos los medios.
    pub creator: Option<String>,
    /// Creador principal de la colección (álbum, serie o saga).
    pub collection_creator: Option<String>,
    /// Álbum, serie o saga a la que pertenece el elemento.
    pub collection: Option<String>,
    pub title: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub filename_track: Option<u32>,
    pub year: Option<u16>,
    /// Hash del contenido de la portada, usado para localizar la miniatura.
    pub cover_hash: Option<String>,
    pub status: TagStatus,
}

/// Un archivo del catálogo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaItem {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub kind: Kind,
    /// Fecha de modificación en nanosegundos desde la época Unix.
    pub mtime: i64,
    /// Tamaño del archivo en bytes.
    pub size: i64,
    /// Origen del elemento.
    pub origin: MediaOrigin,
    /// Acción de apertura asociada a su tipo de medio.
    pub open_action: OpenAction,
    /// Indica si la miniatura ya existe en caché; evita consultar el disco en la vista.
    pub cover_ready: bool,
    pub metadata: TrackMetadata,
}

/// Resumen de un escaneo.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScanSummary {
    pub music: usize,
    pub video: usize,
    pub book: usize,
    pub skipped: usize,
    pub errors: usize,
}

impl ScanSummary {
    /// Cuenta los elementos por tipo y conserva los descartes y errores del escaneo.
    #[must_use]
    pub fn from_items(items: &[MediaItem], skipped: usize, errors: usize) -> Self {
        let mut summary = Self {
            skipped,
            errors,
            ..Self::default()
        };

        for item in items {
            match item.kind {
                Kind::Music => summary.music += 1,
                Kind::Video => summary.video += 1,
                Kind::Book => summary.book += 1,
            }
        }

        summary
    }

    /// Total de archivos catalogados.
    #[must_use]
    #[allow(dead_code)] // Se usa en pruebas y en la vista resumen.
    pub const fn total(&self) -> usize {
        self.music + self.video + self.book
    }
}

/// Aviso no fatal producido durante la sincronización o al abrir un archivo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScanWarning {
    /// La carpeta raíz no existe, no es una carpeta o no se pudo leer.
    RootUnavailable { root: String, reason: String },
    /// Hubo errores al recorrer algunos archivos o carpetas.
    Incomplete { errors: usize },
    /// La raíz quedó vacía pero la base tenía filas; no se borró nada.
    EmptyRoot,
    /// La carpeta elegida o guardada no se pudo usar.
    InvalidCatalogRoot { reason: String },
    /// No se pudo abrir un archivo con la aplicación predeterminada.
    OpenFailed { reason: String },
}

/// Modo de visualización de la biblioteca. Es solo presentación.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMode {
    /// Tarjetas en cuadrícula.
    #[default]
    Grid,
    /// Filas compactas.
    List,
}

impl ViewMode {
    /// Valor que se guarda en la configuración.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::List => "list",
        }
    }

    /// Convierte el valor guardado en la configuración; inválido → None.
    #[must_use]
    pub fn from_config(value: &str) -> Option<Self> {
        match value {
            "grid" => Some(Self::Grid),
            "list" => Some(Self::List),
            _ => None,
        }
    }
}

/// Resultado de sincronizar el disco con la base de datos.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScanOutcome {
    pub items: Vec<MediaItem>,
    /// Biblioteca agrupada, lista para la interfaz.
    pub library: crate::library::Library,
    /// Alias de artista vigentes al terminar el escaneo.
    pub aliases: crate::library::AliasMap,
    pub summary: ScanSummary,
    /// Archivos a los que se leyeron etiquetas en este escaneo.
    #[allow(dead_code)]
    pub tag_reads: usize,
    /// Archivos cuya metadata se reutilizó por no haber cambiado.
    #[allow(dead_code)]
    pub tag_reused: usize,
    pub deleted: usize,
    pub warning: Option<ScanWarning>,
}

#[cfg(test)]
mod tests {
    use super::ViewMode;

    #[test]
    fn view_mode_persiste_y_recupera() {
        assert_eq!(ViewMode::default().as_str(), "grid");
        assert_eq!(ViewMode::from_config("grid"), Some(ViewMode::Grid));
        assert_eq!(ViewMode::from_config("list"), Some(ViewMode::List));
        assert_eq!(ViewMode::from_config("otra-cosa"), None);
        assert_eq!(ViewMode::from_config(""), None);
    }
}
