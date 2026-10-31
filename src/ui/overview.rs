// SPDX-License-Identifier: MIT

//! Vista general con el resumen y la cuadrícula de álbumes.

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::Length;
use cosmic::widget;

use crate::app::Message;
use crate::fl;
use crate::library::AlbumEntry;
use crate::model::Kind;

use super::CatalogView;
use super::cards::album_grid;
use super::common::{album_matches, count_kind, empty, page_header, scrollable};
use super::status::push_status;

pub(super) fn overview<'a>(
    state: CatalogView<'a>,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let albums: Vec<&AlbumEntry> = state
        .library
        .albums
        .iter()
        .filter(|album| album_matches(album, query))
        .collect();

    let videos = count_kind(state.items, Kind::Video);
    let books = count_kind(state.items, Kind::Book);
    let summary = fl!(
        "summary",
        tracks = state.library.track_count,
        albums = state.library.albums.len(),
        artists = state.library.artists.len(),
        videos = videos,
        books = books
    );

    let mut content = page_header(
        fl!("nav-all"),
        fl!("albums-count", count = albums.len()),
        spacing,
    );
    content = push_status(content, &state);
    content = content.push(widget::text::body(summary));

    content = if albums.is_empty() {
        content.push(empty(fl!("empty-music")))
    } else {
        content.push(scrollable(album_grid(albums, state.cache_dir, spacing)))
    };

    content.height(Length::Fill).into()
}
