// SPDX-License-Identifier: MIT

//! Provides localization support for this crate.

use i18n_embed::{
    DefaultLocalizer, LanguageLoader, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
    unic_langid::LanguageIdentifier,
};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

/// Applies the requested language(s) to requested translations from the `fl!()` macro.
pub fn init(requested_languages: &[LanguageIdentifier]) {
    if let Err(why) = localizer().select(requested_languages) {
        eprintln!("error while loading fluent localizations: {why}");
    }
}

// Get the `Localizer` to be used for localizing this library.
#[must_use]
pub fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();

    loader
        .load_fallback_language(&Localizations)
        .expect("Error while loading fallback language");

    loader
});

/// Obtiene una cadena por clave en tiempo de ejecución.
///
/// Se usa para las claves de jerarquía definidas en `media::MEDIA_TYPES`.
#[must_use]
pub fn get(key: &str) -> String {
    LANGUAGE_LOADER.get(key)
}

/// Request a localized string by ID from the i18n/ directory.
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

#[cfg(test)]
mod tests {
    use i18n_embed::unic_langid::LanguageIdentifier;

    use super::init;

    /// Fluent envuelve los números con caracteres invisibles de aislamiento bidi.
    fn sin_aislamiento(texto: &str) -> String {
        texto.replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn carga_las_traducciones_en_espanol() {
        let spanish: LanguageIdentifier = "es-MX".parse().unwrap();
        init(&[spanish]);

        assert_eq!(crate::fl!("nav-music"), "Música");
        assert_eq!(crate::fl!("nav-video"), "Video");
        assert_eq!(crate::fl!("nav-books"), "Libros");
        assert_eq!(crate::fl!("unknown-artist"), "Artista desconocido");
        assert_eq!(crate::fl!("unknown-album"), "Sin álbum");
        assert_eq!(
            sin_aislamiento(&crate::fl!(
                "summary",
                tracks = 73,
                albums = 30,
                artists = 10,
                videos = 3,
                books = 0
            )),
            "73 canciones · 30 álbumes · 10 artistas · 3 videos · 0 libros"
        );
        assert_eq!(
            sin_aislamiento(&crate::fl!("deleted", count = 1)),
            "1 entrada eliminada"
        );
        assert_eq!(
            sin_aislamiento(&crate::fl!("deleted", count = 2)),
            "2 entradas eliminadas"
        );
    }
}
