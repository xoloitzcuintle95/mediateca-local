// SPDX-License-Identifier: MIT

//! Entrada de la interfaz: estado de la vista y despacho por página.

use std::path::Path;

use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget;

use crate::app::{Message, MusicRoute, Page, ScanState, VideoRoute};
use crate::fl;
use crate::model::{MediaItem, ScanWarning};
use crate::video::VideoLibrary;

use super::{books, music, overview, status, video};

/// Datos que la vista necesita del modelo de la aplicación.
#[derive(Clone, Copy)]
pub struct CatalogView<'a> {
    pub page: Page,
    pub music_route: &'a MusicRoute,
    pub overview_album: Option<&'a crate::library::AlbumKey>,
    pub library: &'a crate::library::Library,
    pub video_library: &'a VideoLibrary,
    pub video_route: &'a VideoRoute,
    pub items: &'a [MediaItem],
    pub search: &'a str,
    pub scan_state: &'a ScanState,
    pub deleted: usize,
    pub warning: Option<&'a ScanWarning>,
    pub cache_dir: &'a Path,
    pub catalog_root: &'a str,
}

/// Dibuja la página activa.
pub fn view(state: CatalogView<'_>) -> Element<'_, Message> {
    if state.catalog_root.trim().is_empty() {
        return empty_catalog(state);
    }

    let spacing = cosmic::theme::spacing();
    let query = state.search.trim().to_lowercase();

    let content = match state.page {
        Page::All => match state.overview_album {
            Some(key) => music::album_detail(state, key, &query, true, spacing),
            None => overview::overview(state, &query, spacing),
        },
        Page::Music => match state.music_route {
            MusicRoute::Artists => music::artists(state, &query, spacing),
            MusicRoute::Albums { artist } => music::artist_albums(state, artist, &query, spacing),
            MusicRoute::Album { key } => music::album_detail(state, key, &query, false, spacing),
        },
        Page::Video => video::video_view(state, &query, spacing),
        Page::Books => books::books_view(state, &query, spacing),
    };

    widget::container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([spacing.space_s, spacing.space_m])
        .into()
}

fn empty_catalog(state: CatalogView<'_>) -> Element<'_, Message> {
    let spacing = cosmic::theme::spacing();
    let message = widget::container(
        widget::column::with_capacity(2)
            .spacing(spacing.space_m)
            .push(widget::text::title3(fl!("choose-catalog-folder")))
            .push(
                widget::button::suggested(fl!("choose-folder"))
                    .on_press(Message::ChooseCatalogFolder),
            ),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center);

    let mut content = widget::column::with_capacity(2)
        .width(Length::Fill)
        .height(Length::Fill);
    content = status::push_status(content, &state);
    content = content.push(message);
    content.into()
}
