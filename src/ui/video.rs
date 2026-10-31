// SPDX-License-Identifier: MIT

//! Vistas de video: series, temporadas, episodios y películas.

use std::path::Path;

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::{Alignment, Length};
use cosmic::widget;

use crate::app::{Details, Message, VideoRoute};
use crate::covers;
use crate::fl;
use crate::model::{Kind, MediaItem};
use crate::video::{EpisodeEntry, SeasonEntry, SeriesEntry};

use super::CatalogView;
use super::cards::{
    CARD_WIDTH, cover_thumbnail, ellipsize, line_block, line_budget, rows_from_cards, title_block,
    title_budget,
};
use super::common::{empty, level_label, matches_text, page_header, scrollable};
use super::status::push_status;

pub(super) fn video_view<'a>(
    state: CatalogView<'a>,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    match state.video_route {
        VideoRoute::Series => video_series_root(state, query, spacing),
        VideoRoute::Seasons { series } => video_seasons(state, series, query, spacing),
        VideoRoute::Episodes { series, season } => {
            video_episodes(state, series, *season, query, spacing)
        }
    }
}

fn video_series_root<'a>(
    state: CatalogView<'a>,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let series: Vec<&SeriesEntry> = state
        .video_library
        .series
        .iter()
        .filter(|series| series_matches(series, query))
        .collect();
    let movies: Vec<&MediaItem> = state
        .video_library
        .movies
        .iter()
        .filter(|movie| matches_text(&movie.name, query))
        .collect();

    let mut content = page_header(
        fl!("nav-video"),
        fl!(
            "video-summary",
            series = series.len(),
            movies = movies.len()
        ),
        spacing,
    );
    content = push_status(content, &state);

    if series.is_empty() && movies.is_empty() {
        content = content.push(empty(fl!("empty-generic")));
    } else {
        let mut body = widget::column::with_capacity(4)
            .spacing(spacing.space_m)
            .width(Length::Fill);

        if !series.is_empty() {
            body = body.push(series_grid(series, state.cache_dir, spacing));
        }

        if !movies.is_empty() {
            body = body
                .push(widget::text::title3(fl!("movies")))
                .push(movie_list(movies, spacing));
        }

        content = content.push(scrollable(body));
    }

    content.height(Length::Fill).into()
}

fn video_seasons<'a>(
    state: CatalogView<'a>,
    series_name: &str,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let Some(series) = state.video_library.series(series_name) else {
        return page_header(fl!("nav-video"), String::new(), spacing)
            .push(empty(fl!("empty-generic")))
            .height(Length::Fill)
            .into();
    };

    let seasons: Vec<&SeasonEntry> = series
        .seasons
        .iter()
        .filter(|season| season_matches(season, query))
        .collect();

    let breadcrumb = widget::row::with_capacity(3)
        .push(widget::button::link(level_label(Kind::Video, 0)).on_press(Message::GoToSeries))
        .push(widget::text::caption("›"))
        .push(widget::text::body(series_display_name(&series.name)))
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center);

    let mut content = widget::column::with_capacity(5)
        .spacing(spacing.space_s)
        .width(Length::Fill)
        .height(Length::Fill)
        .push(breadcrumb)
        .push(
            widget::row::with_capacity(3)
                .push(widget::text::title1(series_display_name(&series.name)))
                .push(widget::space::horizontal())
                .push(widget::text::body(fl!(
                    "seasons-count",
                    count = seasons.len()
                )))
                .spacing(spacing.space_s)
                .align_y(Alignment::End)
                .width(Length::Fill),
        );
    content = push_status(content, &state);

    content = if seasons.is_empty() {
        content.push(empty(fl!("empty-generic")))
    } else {
        content.push(scrollable(season_grid(
            seasons,
            &series.name,
            state.cache_dir,
            spacing,
        )))
    };

    content.height(Length::Fill).into()
}

#[allow(clippy::too_many_lines)]
fn video_episodes<'a>(
    state: CatalogView<'a>,
    series_name: &str,
    season_number: u32,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let Some(series) = state.video_library.series(series_name) else {
        return page_header(fl!("nav-video"), String::new(), spacing)
            .push(empty(fl!("empty-generic")))
            .height(Length::Fill)
            .into();
    };
    let Some(season) = series
        .seasons
        .iter()
        .find(|season| season.number == season_number)
    else {
        return page_header(fl!("nav-video"), String::new(), spacing)
            .push(empty(fl!("empty-generic")))
            .height(Length::Fill)
            .into();
    };

    let episodes: Vec<&EpisodeEntry> = season
        .episodes
        .iter()
        .filter(|episode| episode_matches(episode, query))
        .collect();

    let season_label = fl!("season-label", number = season.number);
    let breadcrumb = widget::row::with_capacity(5)
        .push(widget::button::link(level_label(Kind::Video, 0)).on_press(Message::GoToSeries))
        .push(widget::text::caption("›"))
        .push(
            widget::button::link(series_display_name(&series.name))
                .on_press(Message::GoToSeasons(series.name.clone())),
        )
        .push(widget::text::caption("›"))
        .push(widget::text::body(season_label.clone()))
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center);

    let mut content = widget::column::with_capacity(5)
        .spacing(spacing.space_s)
        .width(Length::Fill)
        .height(Length::Fill)
        .push(breadcrumb)
        .push(
            widget::row::with_capacity(3)
                .push(widget::text::title1(season_label.clone()))
                .push(widget::space::horizontal())
                .push(widget::text::body(fl!(
                    "episodes-count",
                    count = episodes.len()
                )))
                .spacing(spacing.space_s)
                .align_y(Alignment::End)
                .width(Length::Fill),
        );
    content = push_status(content, &state);

    if episodes.is_empty() {
        content = content.push(empty(fl!("empty-generic")));
    } else {
        let display_series = series_display_name(series_name);
        let mut list = widget::list_column();

        for episode in episodes {
            let episode_title = episode_display_title(episode);

            for variant in &episode.variants {
                let variant_label = variant.label.clone().unwrap_or_else(|| fl!("file"));
                let subtitle = fl!(
                    "video-details-subtitle",
                    series = display_series.as_str(),
                    season = season_label.as_str(),
                    variant = variant_label.as_str()
                );

                list = list.add(
                    widget::list::button(
                        widget::row::with_capacity(2)
                            .push(widget::icon::from_name("video-x-generic-symbolic").size(20))
                            .push(
                                widget::column::with_capacity(2)
                                    .push(widget::text::body(episode_title.clone()))
                                    .push(widget::text::caption(variant_label))
                                    .spacing(spacing.space_xxxs)
                                    .width(Length::Fill),
                            )
                            .spacing(spacing.space_s)
                            .align_y(Alignment::Center)
                            .width(Length::Fill),
                    )
                    .on_press(Message::ItemClicked(Details {
                        id: variant.item.id,
                        title: episode_title.clone(),
                        subtitle,
                        path: variant.item.path.clone(),
                        cover_hash: None,
                        cover_ready: false,
                        kind: Kind::Video,
                        open_action: variant.item.open_action,
                    })),
                );
            }
        }

        content = content.push(scrollable(list));
    }

    content.height(Length::Fill).into()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn series_grid<'a>(
    series: Vec<&'a SeriesEntry>,
    cache_dir: &'a Path,
    spacing: Spacing,
) -> Element<'a, Message> {
    widget::responsive(move |size| {
        let width = size.width.max(1.0);
        let gap = f32::from(spacing.space_m);
        let card_width = CARD_WIDTH.min(width);
        let columns =
            (((width + gap) / (card_width + gap)).floor() as usize).clamp(1, series.len().max(1));

        let cards = series
            .iter()
            .map(|series| series_card(series, cache_dir, spacing, card_width))
            .collect();

        rows_from_cards(cards, columns, spacing)
    })
    .height(Length::Shrink)
    .into()
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn season_grid<'a>(
    seasons: Vec<&'a SeasonEntry>,
    series_name: &'a str,
    cache_dir: &'a Path,
    spacing: Spacing,
) -> Element<'a, Message> {
    widget::responsive(move |size| {
        let width = size.width.max(1.0);
        let gap = f32::from(spacing.space_m);
        let card_width = CARD_WIDTH.min(width);
        let columns =
            (((width + gap) / (card_width + gap)).floor() as usize).clamp(1, seasons.len().max(1));

        let cards = seasons
            .iter()
            .map(|season| season_card(season, series_name, cache_dir, spacing, card_width))
            .collect();

        rows_from_cards(cards, columns, spacing)
    })
    .height(Length::Shrink)
    .into()
}

fn series_card<'a>(
    series: &'a SeriesEntry,
    cache_dir: &'a Path,
    spacing: Spacing,
    card_width: f32,
) -> Element<'a, Message> {
    let padding = f32::from(spacing.space_xs);
    let cover_width = (card_width - 2.0 * padding).max(48.0);
    let title = ellipsize(
        &series_display_name(&series.name),
        title_budget(cover_width),
    );
    let subtitle = ellipsize(
        &fl!(
            "series-subtitle",
            seasons = series.seasons.len(),
            episodes = series.episode_count
        ),
        line_budget(cover_width),
    );

    let content = widget::column::with_capacity(3)
        .spacing(spacing.space_xxxs)
        .width(Length::Fill)
        .push(cover_thumbnail(
            None,
            cache_dir,
            covers::CoverTarget::Video,
            cover_width,
            false,
        ))
        .push(title_block(title))
        .push(line_block(subtitle));

    let card = widget::container(content)
        .class(cosmic::theme::Container::Card)
        .padding(spacing.space_xs)
        .width(Length::Fixed(card_width));

    widget::button::custom(card)
        .class(cosmic::theme::Button::Transparent)
        .padding(0)
        .width(Length::Fixed(card_width))
        .on_press(Message::OpenSeries(series.name.clone()))
        .into()
}

fn season_card<'a>(
    season: &'a SeasonEntry,
    series_name: &'a str,
    cache_dir: &'a Path,
    spacing: Spacing,
    card_width: f32,
) -> Element<'a, Message> {
    let padding = f32::from(spacing.space_xs);
    let cover_width = (card_width - 2.0 * padding).max(48.0);
    let title = ellipsize(
        &fl!("season-label", number = season.number),
        title_budget(cover_width),
    );
    let subtitle = ellipsize(
        &fl!("episodes-count", count = season.episodes.len()),
        line_budget(cover_width),
    );

    let content = widget::column::with_capacity(3)
        .spacing(spacing.space_xxxs)
        .width(Length::Fill)
        .push(cover_thumbnail(
            None,
            cache_dir,
            covers::CoverTarget::Video,
            cover_width,
            false,
        ))
        .push(title_block(title))
        .push(line_block(subtitle));

    let card = widget::container(content)
        .class(cosmic::theme::Container::Card)
        .padding(spacing.space_xs)
        .width(Length::Fixed(card_width));

    widget::button::custom(card)
        .class(cosmic::theme::Button::Transparent)
        .padding(0)
        .width(Length::Fixed(card_width))
        .on_press(Message::OpenSeason {
            series: series_name.to_owned(),
            season: season.number,
        })
        .into()
}

fn movie_list<'a>(movies: Vec<&MediaItem>, spacing: Spacing) -> Element<'a, Message> {
    let mut list = widget::list_column();

    for movie in movies {
        list = list.add(
            widget::list::button(
                widget::row::with_capacity(2)
                    .push(widget::icon::from_name("video-x-generic-symbolic").size(20))
                    .push(widget::text::body(movie.name.clone()))
                    .spacing(spacing.space_s)
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
            )
            .on_press(Message::ItemClicked(Details {
                id: movie.id,
                title: movie.name.clone(),
                subtitle: fl!("movie"),
                path: movie.path.clone(),
                cover_hash: None,
                cover_ready: false,
                kind: Kind::Video,
                open_action: movie.open_action,
            })),
        );
    }

    list.into()
}

fn series_display_name(name: &str) -> String {
    if name.trim().is_empty() {
        fl!("unknown-series")
    } else {
        name.to_owned()
    }
}

fn episode_display_title(episode: &EpisodeEntry) -> String {
    let label = fl!("episode-label", number = episode.number);

    episode
        .title
        .as_ref()
        .map_or(label.clone(), |title| format!("{label} · {title}"))
}

fn series_matches(series: &SeriesEntry, query: &str) -> bool {
    query.is_empty() || series.search_key.contains(query)
}

fn season_matches(season: &SeasonEntry, query: &str) -> bool {
    query.is_empty() || season.search_key.contains(query)
}

fn episode_matches(episode: &EpisodeEntry, query: &str) -> bool {
    query.is_empty() || episode.search_key.contains(query)
}
