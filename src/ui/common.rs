// SPDX-License-Identifier: MIT

//! Helpers compartidos de la interfaz: cabeceras, listas, etiquetas y búsqueda.

use std::sync::LazyLock;

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{self, icon};

use crate::app::{Message, PageTarget};
use crate::fl;
use crate::library::{AlbumEntry, ArtistEntry, TrackEntry};
use crate::media;
use crate::model::{Kind, MediaItem};

pub(super) fn level_label(kind: Kind, level: usize) -> String {
    media::hierarchy_key(kind, level).map_or_else(String::new, crate::i18n::get)
}
pub(super) fn page_header<'a>(
    title: String,
    count: String,
    spacing: Spacing,
    toggle: Option<Element<'a, Message>>,
) -> widget::Column<'a, Message, cosmic::Theme, cosmic::Renderer> {
    let mut title_row = widget::row::with_capacity(3)
        .push(widget::text::title1(title))
        .push(widget::space::horizontal())
        .spacing(spacing.space_s)
        .align_y(Alignment::End)
        .width(Length::Fill);

    if let Some(toggle) = toggle {
        title_row = title_row.push(toggle);
    }

    title_row = title_row.push(widget::text::body(count));

    widget::column::with_capacity(2)
        .spacing(spacing.space_s)
        .width(Length::Fill)
        .push(title_row)
}

/// Id fijo del scrollable del overview (para el scroll al inicio al paginar).
pub(crate) static OVERVIEW_SCROLL_ID: LazyLock<cosmic::iced::core::widget::Id> =
    LazyLock::new(|| cosmic::iced::core::widget::Id::new("overview-list"));

/// Barra de paginación: Primera/Anterior/"Página X de Y"/Siguiente/Última.
///
/// Deshabilita los botones en los extremos; el llamador decide si se dibuja.
pub(super) fn page_bar<'a>(
    pagina: usize,
    total_paginas: usize,
    destino: PageTarget,
    spacing: Spacing,
) -> Element<'a, Message> {
    let en_primera = pagina == 0;
    let en_ultima = pagina + 1 >= total_paginas;
    let pagina_humana = pagina + 1;

    widget::row::with_capacity(5)
        .spacing(spacing.space_s)
        .align_y(Alignment::Center)
        .push(
            widget::button::text(fl!("pagina-primera"))
                .on_press_maybe((!en_primera).then_some(Message::FirstPage(destino))),
        )
        .push(
            widget::button::text(fl!("pagina-anterior"))
                .on_press_maybe((!en_primera).then_some(Message::PrevPage(destino))),
        )
        .push(widget::text::body(fl!(
            "pagina-posicion",
            pagina = pagina_humana,
            total = total_paginas
        )))
        .push(
            widget::button::text(fl!("pagina-siguiente"))
                .on_press_maybe((!en_ultima).then_some(Message::NextPage(destino))),
        )
        .push(
            widget::button::text(fl!("pagina-ultima"))
                .on_press_maybe((!en_ultima).then_some(Message::LastPage(destino))),
        )
        .into()
}

/// Alternador de modo de vista: cuadrícula o lista. Tema-nativo; el activo
/// usa el acento del tema y el inactivo `Button::Text` (fondo transparente
/// pero texto visible: `Button::Transparent` pinta el contenido en alpha 0).
pub(super) fn view_mode_toggle<'a>(mode: crate::model::ViewMode) -> Element<'a, Message> {
    let grid_active = mode == crate::model::ViewMode::Grid;
    let list_active = mode == crate::model::ViewMode::List;

    widget::row::with_capacity(2)
        .spacing(spacing_from_static())
        .push(
            widget::button::icon(icon::from_name("view-grid-symbolic").size(16))
                .class(if grid_active {
                    cosmic::theme::Button::Suggested
                } else {
                    cosmic::theme::Button::Text
                })
                .on_press(Message::ViewModeChanged(crate::model::ViewMode::Grid)),
        )
        .push(
            widget::button::icon(icon::from_name("view-list-symbolic").size(16))
                .class(if list_active {
                    cosmic::theme::Button::Suggested
                } else {
                    cosmic::theme::Button::Text
                })
                .on_press(Message::ViewModeChanged(crate::model::ViewMode::List)),
        )
        .into()
}

fn spacing_from_static() -> f32 {
    // El alternador se dibuja fuera del contexto de página; usa el espaciado
    // estándar del tema, que no cambia por página.
    f32::from(cosmic::theme::spacing().space_s)
}
pub(super) fn scrollable<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    widget::scrollable(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub(super) fn empty<'a>(message: String) -> Element<'a, Message> {
    widget::container(widget::text::body(message))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
}
pub(super) fn album_title(album: &AlbumEntry) -> String {
    if album.title.trim().is_empty() {
        fl!("unknown-album")
    } else {
        album.title.clone()
    }
}

pub(super) fn album_subtitle(album: &AlbumEntry) -> String {
    let count = album.tracks.len();

    if count == 1 {
        album.year.map_or_else(
            || fl!("single"),
            |year| fl!("single-with-year", year = year),
        )
    } else {
        album.year.map_or_else(
            || fl!("album-no-year", count = count),
            |year| fl!("album-subtitle", count = count, year = year),
        )
    }
}

pub(super) fn year_label(year: Option<u16>) -> String {
    year.map_or_else(|| fl!("unknown-year"), |year| year.to_string())
}

pub(super) fn count_kind(items: &[MediaItem], kind: Kind) -> usize {
    items.iter().filter(|item| item.kind == kind).count()
}

pub(crate) fn album_matches(album: &AlbumEntry, query: &str) -> bool {
    query.is_empty() || album.search_key.contains(query)
}

pub(super) fn artist_matches(artist: &ArtistEntry, query: &str) -> bool {
    query.is_empty() || artist.search_key.contains(query)
}

pub(super) fn track_matches(track: &TrackEntry, query: &str) -> bool {
    query.is_empty() || track.search_key.contains(query)
}

pub(super) fn matches_text(text: &str, query: &str) -> bool {
    query.is_empty() || text.to_lowercase().contains(query)
}
