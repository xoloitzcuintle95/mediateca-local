// SPDX-License-Identifier: MIT

//! Tarjetas, cuadrículas y miniaturas compartidas por las vistas.

use std::path::Path;

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::widget::text::Wrapping;
use cosmic::iced::{ContentFit, Length};
use cosmic::widget;

use crate::app::{Details, Message};
use crate::covers;
use crate::fl;
use crate::library::{AlbumEntry, ArtistEntry};
use crate::model::{Kind, MediaItem};

use super::common::{album_subtitle, album_title};

pub(super) const CARD_WIDTH: f32 = 220.0;
const TITLE_HEIGHT: f32 = 40.0;
const LINE_HEIGHT: f32 = 20.0;
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn album_grid<'a>(
    albums: Vec<&'a AlbumEntry>,
    cache_dir: &'a Path,
    spacing: Spacing,
) -> Element<'a, Message> {
    widget::responsive(move |size| {
        let width = size.width.max(1.0);
        let gap = f32::from(spacing.space_m);
        let card_width = CARD_WIDTH.min(width);
        let columns =
            (((width + gap) / (card_width + gap)).floor() as usize).clamp(1, albums.len().max(1));

        let cards = albums
            .iter()
            .map(|album| album_card(album, cache_dir, spacing, card_width))
            .collect();

        rows_from_cards(cards, columns, spacing)
    })
    .height(Length::Shrink)
    .into()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn artist_grid<'a>(
    artists: Vec<&'a ArtistEntry>,
    cache_dir: &'a Path,
    spacing: Spacing,
) -> Element<'a, Message> {
    widget::responsive(move |size| {
        let width = size.width.max(1.0);
        let gap = f32::from(spacing.space_m);
        let card_width = CARD_WIDTH.min(width);
        let columns =
            (((width + gap) / (card_width + gap)).floor() as usize).clamp(1, artists.len().max(1));

        let cards = artists
            .iter()
            .map(|artist| artist_card(artist, cache_dir, spacing, card_width))
            .collect();

        rows_from_cards(cards, columns, spacing)
    })
    .height(Length::Shrink)
    .into()
}
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn book_grid<'a>(
    books: Vec<&'a MediaItem>,
    cache_dir: &'a Path,
    spacing: Spacing,
) -> Element<'a, Message> {
    widget::responsive(move |size| {
        let width = size.width.max(1.0);
        let gap = f32::from(spacing.space_m);
        let card_width = CARD_WIDTH.min(width);
        let columns =
            (((width + gap) / (card_width + gap)).floor() as usize).clamp(1, books.len().max(1));

        let cards = books
            .iter()
            .map(|book| book_card(book, cache_dir, spacing, card_width))
            .collect();

        rows_from_cards(cards, columns, spacing)
    })
    .height(Length::Shrink)
    .into()
}

pub(super) fn rows_from_cards<'a>(
    cards: Vec<Element<'a, Message>>,
    columns: usize,
    spacing: Spacing,
) -> Element<'a, Message> {
    let mut cards = cards.into_iter();
    let mut rows = widget::column::with_capacity(8)
        .spacing(spacing.space_m)
        .width(Length::Fill)
        .height(Length::Shrink);

    loop {
        let mut row_cards: Vec<Element<'a, Message>> = cards.by_ref().take(columns).collect();

        if row_cards.is_empty() {
            break;
        }

        let count = row_cards.len();
        let mut row = widget::row::with_capacity(count * 2 + 2)
            .spacing(spacing.space_m)
            .width(Length::Fill)
            .height(Length::Shrink);

        if count == 1 && columns > 1 {
            row = row.push(widget::space::horizontal());
            row = row.push(row_cards.pop().unwrap());
            row = row.push(widget::space::horizontal());
        } else {
            let mut iter = row_cards.into_iter();

            if let Some(first) = iter.next() {
                row = row.push(first);
            }

            for card in iter {
                row = row.push(widget::space::horizontal()).push(card);
            }
        }

        rows = rows.push(row);
    }

    rows.into()
}

fn album_card<'a>(
    album: &'a AlbumEntry,
    cache_dir: &'a Path,
    spacing: Spacing,
    card_width: f32,
) -> Element<'a, Message> {
    let padding = f32::from(spacing.space_xs);
    let cover_width = (card_width - 2.0 * padding).max(48.0);
    let title = ellipsize(&album_title(album), title_budget(cover_width));
    let artist = if album.artist.is_empty() {
        fl!("unknown-artist")
    } else {
        album.artist.clone()
    };
    let artist = ellipsize(&artist, line_budget(cover_width));
    let subtitle = ellipsize(&album_subtitle(album), line_budget(cover_width));

    let content = widget::column::with_capacity(4)
        .spacing(spacing.space_xxxs)
        .width(Length::Fill)
        .push(cover_thumbnail(
            album.cover_hash.as_deref(),
            cache_dir,
            covers::CoverTarget::Music,
            cover_width,
            album.cover_ready,
        ))
        .push(title_block(title))
        .push(line_block(artist))
        .push(line_block(subtitle));

    let card = widget::container(content)
        .class(cosmic::theme::Container::Card)
        .padding(spacing.space_xs)
        .width(Length::Fixed(card_width));

    widget::button::custom(card)
        .class(cosmic::theme::Button::Transparent)
        .padding(0)
        .width(Length::Fixed(card_width))
        .on_press(Message::OpenAlbum(album.key.clone()))
        .into()
}

fn book_card<'a>(
    book: &'a MediaItem,
    cache_dir: &'a Path,
    spacing: Spacing,
    card_width: f32,
) -> Element<'a, Message> {
    let padding = f32::from(spacing.space_xs);
    let cover_width = (card_width - 2.0 * padding).max(48.0);
    let display_title = book
        .metadata
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(&book.name)
        .to_owned();
    let title = ellipsize(&display_title, title_budget(cover_width));
    let display_author = book
        .metadata
        .creator
        .as_deref()
        .map(str::trim)
        .filter(|author| !author.is_empty())
        .map_or_else(|| fl!("unknown-author"), ToOwned::to_owned);
    let author = ellipsize(&display_author, line_budget(cover_width));

    let content = widget::column::with_capacity(3)
        .spacing(spacing.space_xxxs)
        .width(Length::Fill)
        .push(cover_thumbnail(
            book.metadata.cover_hash.as_deref(),
            cache_dir,
            covers::CoverTarget::Book,
            cover_width,
            book.cover_ready,
        ))
        .push(title_block(title))
        .push(line_block(author));

    let card = widget::container(content)
        .class(cosmic::theme::Container::Card)
        .padding(spacing.space_xs)
        .width(Length::Fixed(card_width));

    widget::button::custom(card)
        .class(cosmic::theme::Button::Transparent)
        .padding(0)
        .width(Length::Fixed(card_width))
        .on_press(Message::ItemClicked(Details {
            id: book.id,
            title: display_title,
            subtitle: display_author,
            path: book.path.clone(),
            cover_hash: book.metadata.cover_hash.clone(),
            cover_ready: book.cover_ready,
            kind: Kind::Book,
            open_action: book.open_action,
        }))
        .into()
}

fn artist_card<'a>(
    artist: &'a ArtistEntry,
    cache_dir: &'a Path,
    spacing: Spacing,
    card_width: f32,
) -> Element<'a, Message> {
    let padding = f32::from(spacing.space_xs);
    let cover_width = (card_width - 2.0 * padding).max(48.0);
    let name = if artist.name.is_empty() {
        fl!("unknown-artist")
    } else {
        artist.name.clone()
    };
    let name = ellipsize(&name, title_budget(cover_width));
    let album_count = ellipsize(
        &fl!("albums-count", count = artist.albums.len()),
        line_budget(cover_width),
    );

    let content = widget::column::with_capacity(3)
        .spacing(spacing.space_xxxs)
        .width(Length::Fill)
        .push(cover_thumbnail(
            artist.cover_hash.as_deref(),
            cache_dir,
            covers::CoverTarget::Music,
            cover_width,
            artist.cover_ready,
        ))
        .push(title_block(name))
        .push(line_block(album_count));

    let card = widget::container(content)
        .class(cosmic::theme::Container::Card)
        .padding(spacing.space_xs)
        .width(Length::Fixed(card_width));

    widget::button::custom(card)
        .class(cosmic::theme::Button::Transparent)
        .padding(0)
        .width(Length::Fixed(card_width))
        .on_press(Message::OpenArtist(artist.name.clone()))
        .into()
}

pub(super) fn title_block<'a>(title: String) -> Element<'a, Message> {
    widget::container(
        widget::text::body(title)
            .wrapping(Wrapping::WordOrGlyph)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fixed(TITLE_HEIGHT))
    .clip(true)
    .into()
}

pub(super) fn line_block<'a>(text: String) -> Element<'a, Message> {
    widget::container(widget::text::caption(text).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fixed(LINE_HEIGHT))
        .clip(true)
        .into()
}

pub(super) fn ellipsize(text: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(1);
    let count = text.chars().count();

    if count <= max_chars {
        return text.to_owned();
    }

    let mut shortened: String = text.chars().take(max_chars - 1).collect();
    shortened.push('…');
    shortened
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn title_budget(width: f32) -> usize {
    ((width / 8.0) as usize).saturating_mul(2).max(16)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn line_budget(width: f32) -> usize {
    ((width / 8.0) as usize).max(12)
}

pub(super) fn cover_thumbnail<'a>(
    hash: Option<&str>,
    cache_dir: &Path,
    target: covers::CoverTarget,
    width: f32,
    ready: bool,
) -> Element<'a, Message> {
    let height = target.height_for_width(width);

    if ready
        && let Some(hash) = hash
        && !cache_dir.as_os_str().is_empty()
    {
        return widget::image(cosmic::iced::widget::image::Handle::from_path(
            covers::thumbnail_path(cache_dir, hash, target),
        ))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .content_fit(ContentFit::Cover)
        .border_radius(cosmic::theme::active().cosmic().corner_radii.radius_s)
        .into();
    }

    let icon = match target {
        covers::CoverTarget::Music => "audio-x-generic-symbolic",
        covers::CoverTarget::Video => "video-x-generic-symbolic",
        covers::CoverTarget::Book => "x-office-document-symbolic",
    };

    widget::container(widget::icon::from_name(icon).size(48))
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .class(cosmic::theme::Container::Card)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::{ellipsize, line_budget, title_budget};

    #[test]
    fn recorta_titulos_respetando_utf8() {
        assert_eq!(ellipsize("Canción", 20), "Canción");

        let cut = ellipsize("17 - What's Your Name？", 12);
        assert_eq!(cut.chars().count(), 12);
        assert!(cut.ends_with('…'));
        assert!(cut.starts_with("17 - What"));
    }

    #[test]
    fn los_presupuestos_crecen_con_el_ancho() {
        assert!(title_budget(200.0) > title_budget(100.0));
        assert!(line_budget(200.0) > line_budget(100.0));
        assert!(title_budget(1.0) >= 16);
        assert!(line_budget(1.0) >= 12);
    }
}
