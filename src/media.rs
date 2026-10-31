// SPDX-License-Identifier: MIT

//! Definición única de cada tipo de medio.
//!
//! Extensiones, proporción de portada, niveles de jerarquía y forma de abrir
//! viven aquí. Los demás módulos deben consultar esta tabla en vez de repetir
//! esas decisiones.

use crate::model::{Kind, OpenAction};

/// Definición de un tipo de medio.
#[derive(Clone, Copy, Debug)]
pub struct MediaType {
    pub kind: Kind,
    pub extensions: &'static [&'static str],
    pub cover_aspect: (u32, u32),
    /// Claves i18n de los tres niveles: nivel superior, intermedio y hojas.
    pub hierarchy: [&'static str; 3],
    pub open_action: OpenAction,
}

pub const MEDIA_TYPES: &[MediaType] = &[
    MediaType {
        kind: Kind::Music,
        extensions: &["mp3", "flac", "ogg"],
        cover_aspect: (1, 1),
        hierarchy: ["artists", "albums", "tracks"],
        open_action: OpenAction::XdgOpen,
    },
    MediaType {
        kind: Kind::Video,
        extensions: &["mkv", "mp4", "avi"],
        cover_aspect: (2, 3),
        hierarchy: ["series", "seasons", "episodes"],
        open_action: OpenAction::XdgOpen,
    },
    MediaType {
        kind: Kind::Book,
        extensions: &["epub", "pdf"],
        cover_aspect: (2, 3),
        hierarchy: ["authors", "sagas", "books"],
        open_action: OpenAction::XdgOpen,
    },
];

/// Devuelve la definición del tipo indicado.
///
/// # Panics
/// Solo si la tabla de tipos está incompleta; hay un test que lo comprueba.
#[must_use]
pub fn media_type(kind: Kind) -> &'static MediaType {
    MEDIA_TYPES
        .iter()
        .find(|media| media.kind == kind)
        .expect("todo Kind debe tener una definición en MEDIA_TYPES")
}

/// Clasifica una extensión sin punto, sin distinguir mayúsculas.
#[must_use]
pub fn classify_extension(extension: &str) -> Option<Kind> {
    let extension = extension.to_ascii_lowercase();

    MEDIA_TYPES
        .iter()
        .find(|media| media.extensions.contains(&extension.as_str()))
        .map(|media| media.kind)
}

/// Clave i18n de un nivel de jerarquía (0, 1 o 2).
#[must_use]
pub fn hierarchy_key(kind: Kind, level: usize) -> Option<&'static str> {
    media_type(kind).hierarchy.get(level).copied()
}

#[cfg(test)]
mod tests {
    use super::{MEDIA_TYPES, classify_extension, hierarchy_key, media_type};
    use crate::model::{Kind, OpenAction};

    #[test]
    fn cada_tipo_tiene_una_definicion() {
        for kind in [Kind::Music, Kind::Video, Kind::Book] {
            let media = media_type(kind);
            assert_eq!(media.kind, kind);
            assert_ne!(media.extensions.len(), 0);
            assert_eq!(media.open_action, OpenAction::XdgOpen);
        }

        assert_eq!(MEDIA_TYPES.len(), 3);
    }

    #[test]
    fn clasifica_extensiones_desde_la_definicion_unica() {
        assert_eq!(classify_extension("mp3"), Some(Kind::Music));
        assert_eq!(classify_extension("FLAC"), Some(Kind::Music));
        assert_eq!(classify_extension("ogg"), Some(Kind::Music));
        assert_eq!(classify_extension("mkv"), Some(Kind::Video));
        assert_eq!(classify_extension("MP4"), Some(Kind::Video));
        assert_eq!(classify_extension("avi"), Some(Kind::Video));
        assert_eq!(classify_extension("epub"), Some(Kind::Book));
        assert_eq!(classify_extension("PDF"), Some(Kind::Book));
        assert_eq!(classify_extension("wav"), None);
    }

    #[test]
    fn las_proporciones_y_niveles_salen_de_la_misma_tabla() {
        assert_eq!(media_type(Kind::Music).cover_aspect, (1, 1));
        assert_eq!(media_type(Kind::Video).cover_aspect, (2, 3));
        assert_eq!(media_type(Kind::Book).cover_aspect, (2, 3));

        assert_eq!(hierarchy_key(Kind::Music, 0), Some("artists"));
        assert_eq!(hierarchy_key(Kind::Video, 1), Some("seasons"));
        assert_eq!(hierarchy_key(Kind::Book, 2), Some("books"));
        assert_eq!(hierarchy_key(Kind::Music, 3), None);
    }
}
