// SPDX-License-Identifier: MIT

//! Vistas de música: artistas, álbumes y canciones.

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::widget::text::Wrapping;
use cosmic::iced::{Alignment, Length};
use cosmic::widget;

use crate::app::{Details, Message};
use crate::covers;
use crate::fl;
use crate::library::{AlbumEntry, ArtistEntry, TrackEntry};
use crate::model::Kind;

use super::CatalogView;
use super::cards::{album_grid, artist_grid, cover_thumbnail};
use super::common::{
    album_matches, album_title, artist_matches, empty, level_label, page_header, scrollable,
    track_matches, year_label,
};
use super::status::push_status;

pub(super) fn artists<'a>(
    state: CatalogView<'a>,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let artists: Vec<&ArtistEntry> = state
        .library
        .artists
        .iter()
        .filter(|artist| artist_matches(artist, query))
        .collect();

    let mut content = page_header(
        fl!("nav-music"),
        fl!("artists-count", count = artists.len()),
        spacing,
    );
    content = push_status(content, &state);

    content = if artists.is_empty() {
        content.push(empty(fl!("empty-music")))
    } else {
        content.push(scrollable(artist_grid(artists, state.cache_dir, spacing)))
    };

    content.height(Length::Fill).into()
}
pub(super) fn artist_albums<'a>(
    state: CatalogView<'a>,
    artist_name: &str,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let artist = state
        .library
        .artists
        .iter()
        .find(|artist| artist.name == artist_name);

    let display_name = if artist_name.is_empty() {
        fl!("unknown-artist")
    } else {
        artist_name.to_owned()
    };

    let albums: Vec<&AlbumEntry> = artist.map_or_else(Vec::new, |artist| {
        artist
            .albums
            .iter()
            .filter(|album| album_matches(album, query))
            .collect()
    });

    let breadcrumb = widget::row::with_capacity(3)
        .push(widget::button::link(level_label(Kind::Music, 0)).on_press(Message::GoToArtists))
        .push(widget::text::caption("›"))
        .push(widget::text::body(display_name.clone()))
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center);

    let mut content = widget::column::with_capacity(6)
        .spacing(spacing.space_s)
        .width(Length::Fill)
        .height(Length::Fill)
        .push(breadcrumb)
        .push(
            widget::row::with_capacity(3)
                .push(widget::text::title1(display_name))
                .push(widget::space::horizontal())
                .push(widget::text::body(fl!(
                    "albums-count",
                    count = albums.len()
                )))
                .spacing(spacing.space_s)
                .align_y(Alignment::End)
                .width(Length::Fill),
        );

    content = push_status(content, &state);

    content = if albums.is_empty() {
        content.push(empty(fl!("empty-music")))
    } else {
        content.push(scrollable(album_grid(albums, state.cache_dir, spacing)))
    };

    content.height(Length::Fill).into()
}
#[allow(clippy::too_many_lines)]
pub(super) fn album_detail<'a>(
    state: CatalogView<'a>,
    key: &crate::library::AlbumKey,
    query: &str,
    from_overview: bool,
    spacing: Spacing,
) -> Element<'a, Message> {
    let Some(album) = state.library.albums.iter().find(|album| &album.key == key) else {
        let mut content = page_header(fl!("nav-all"), String::new(), spacing);
        content = content.push(empty(fl!("empty-tracks")));
        return content.height(Length::Fill).into();
    };

    let album_title = album_title(album);
    let artist_name = if album.artist.is_empty() {
        fl!("unknown-artist")
    } else {
        album.artist.clone()
    };

    let tracks: Vec<&TrackEntry> = album
        .tracks
        .iter()
        .filter(|track| track_matches(track, query))
        .collect();

    let breadcrumb = if from_overview {
        widget::row::with_capacity(3)
            .push(widget::button::link(fl!("nav-all")).on_press(Message::GoToOverview))
            .push(widget::text::caption("›"))
            .push(widget::text::body(album_title.clone()))
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center)
    } else {
        widget::row::with_capacity(5)
            .push(widget::button::link(level_label(Kind::Music, 0)).on_press(Message::GoToArtists))
            .push(widget::text::caption("›"))
            .push(
                widget::button::link(artist_name.clone())
                    .on_press(Message::OpenArtist(album.artist.clone())),
            )
            .push(widget::text::caption("›"))
            .push(widget::text::body(album_title.clone()))
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center)
    };

    let album_header = widget::row::with_capacity(2)
        .push(cover_thumbnail(
            album.cover_hash.as_deref(),
            state.cache_dir,
            covers::CoverTarget::Music,
            200.0,
            album.cover_ready,
        ))
        .push(
            widget::column::with_capacity(3)
                .push(widget::text::title1(album_title.clone()).wrapping(Wrapping::WordOrGlyph))
                .push(widget::text::body(artist_name))
                .push(widget::text::caption(year_label(album.year)))
                .spacing(spacing.space_xxxs)
                .width(Length::Fill),
        )
        .spacing(spacing.space_m)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    let mut content = widget::column::with_capacity(7)
        .spacing(spacing.space_s)
        .width(Length::Fill)
        .height(Length::Fill)
        .push(breadcrumb)
        .push(album_header);

    content = push_status(content, &state);
    content = content.push(widget::text::body(fl!(
        "tracks-count",
        count = tracks.len()
    )));

    if tracks.is_empty() {
        content = content.push(empty(fl!("empty-tracks")));
    } else {
        let mut list = widget::list_column();

        for track in tracks {
            let subtitle = fl!(
                "details-subtitle",
                artist = track.artist.as_str(),
                album = album_title.as_str()
            );

            list = list.add(
                widget::list::button(
                    widget::row::with_capacity(2)
                        .push(widget::text::caption(format!("{:02}", track.position)))
                        .push(
                            widget::column::with_capacity(2)
                                .push(
                                    widget::text::body(&track.title)
                                        .wrapping(Wrapping::WordOrGlyph),
                                )
                                .push(widget::text::caption(&track.artist))
                                .spacing(spacing.space_xxxs)
                                .width(Length::Fill),
                        )
                        .spacing(spacing.space_s)
                        .align_y(Alignment::Center)
                        .width(Length::Fill),
                )
                .on_press(Message::ItemClicked(Details {
                    id: track.id,
                    title: track.title.clone(),
                    subtitle,
                    path: track.path.clone(),
                    cover_hash: album.cover_hash.clone(),
                    cover_ready: album.cover_ready,
                    kind: Kind::Music,
                    open_action: track.open_action,
                })),
            );
        }

        content = content.push(scrollable(list));
    }

    content.height(Length::Fill).into()
}
