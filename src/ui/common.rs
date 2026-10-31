// SPDX-License-Identifier: MIT

//! Helpers compartidos de la interfaz: cabeceras, listas, etiquetas y búsqueda.

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Alignment, Length};
use cosmic::widget;

use crate::app::Message;
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
) -> widget::Column<'a, Message, cosmic::Theme, cosmic::Renderer> {
    widget::column::with_capacity(2)
        .spacing(spacing.space_s)
        .width(Length::Fill)
        .push(
            widget::row::with_capacity(3)
                .push(widget::text::title1(title))
                .push(widget::space::horizontal())
                .push(widget::text::body(count))
                .spacing(spacing.space_s)
                .align_y(Alignment::End)
                .width(Length::Fill),
        )
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

pub(super) fn album_matches(album: &AlbumEntry, query: &str) -> bool {
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
