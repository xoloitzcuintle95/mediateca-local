// SPDX-License-Identifier: MIT

mod app;
mod config;
mod covers;
mod db;
mod epub;
mod i18n;
mod library;
mod media;
mod model;
mod opener;
mod scanner;
#[cfg(feature = "seed")]
mod seed;
mod tags;
mod ui;
mod video;
mod video_tags;

// Allocator global: mimalloc reduce la fragmentación y el RSS frente al
// malloc del sistema (glibc).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> cosmic::iced::Result {
    // Modo de desarrollo: sembrar catálogo sintético (feature `seed`).
    #[cfg(feature = "seed")]
    if std::env::var_os("MEDIATECA_SEED").is_some()
        && let Err(error) = seed::ejecutar()
    {
        eprintln!("no se pudo sembrar el catálogo de prueba: {error}");
    }

    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Settings for configuring the application window and iced runtime.
    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(360.0)
            .min_height(180.0),
    );

    // Starts the application's event loop with `()` as the application's flags.
    cosmic::app::run::<app::AppModel>(settings, ())
}
