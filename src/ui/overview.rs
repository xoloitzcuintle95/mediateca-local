// SPDX-License-Identifier: MIT

//! Vista general con el resumen y la cuadrícula de álbumes.

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::Length;
use cosmic::widget;

use crate::app::{Message, PAGE_SIZE, PageTarget, total_pages};
use crate::fl;
use crate::library::AlbumEntry;
use crate::model::{Kind, ViewMode};
use crate::perf;

use super::CatalogView;
use super::cards::{album_grid, album_list};
use super::common::{
    OVERVIEW_SCROLL_ID, count_kind, empty, page_bar, page_header, view_mode_toggle,
};
use super::status::push_status;

pub(super) fn overview<'a>(
    state: CatalogView<'a>,
    _query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let _view_guard = perf::Timer::start("view.overview");

    let total = state.overview_visible.len();
    let total_paginas = total_pages(total);

    let albums: Vec<&AlbumEntry> = state
        .overview_visible
        .chunks(PAGE_SIZE)
        .nth(state.overview_page)
        .map(|pagina| {
            pagina
                .iter()
                .filter_map(|&indice| state.library.albums.get(indice))
                .collect()
        })
        .unwrap_or_default();

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
        fl!("albums-count", count = total),
        spacing,
        Some(view_mode_toggle(state.view_mode)),
    );
    content = push_status(content, &state);
    content = content.push(widget::text::body(summary));

    content = if albums.is_empty() {
        content.push(empty(fl!("empty-music")))
    } else {
        content.push(
            widget::scrollable(match state.view_mode {
                ViewMode::Grid => album_grid(albums, state.cache_dir, spacing),
                ViewMode::List => album_list(albums, state.cache_dir, spacing),
            })
            .id(OVERVIEW_SCROLL_ID.clone())
            .width(Length::Fill)
            .height(Length::Fill),
        )
    };

    if total_paginas > 1 {
        content = content.push(page_bar(
            state.overview_page,
            total_paginas,
            PageTarget::Overview,
            spacing,
        ));
    }

    content.height(Length::Fill).into()
}
