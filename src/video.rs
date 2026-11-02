// SPDX-License-Identifier: MIT

//! Agrupación de videos por serie, temporada y episodio.
//!
//! El nombre del archivo es la única fuente: no hay metadatos ni red.

use std::collections::BTreeMap;

use crate::model::{Kind, MediaItem};

/// Biblioteca de video agrupada.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VideoLibrary {
    pub series: Vec<SeriesEntry>,
    pub movies: Vec<MediaItem>,
}

/// Una serie con sus temporadas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesEntry {
    pub name: String,
    pub seasons: Vec<SeasonEntry>,
    pub episode_count: usize,
    pub search_key: String,
}

/// Una temporada con sus episodios.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeasonEntry {
    pub number: u32,
    pub episodes: Vec<EpisodeEntry>,
    pub search_key: String,
}

/// Un episodio, con una o más variantes del mismo archivo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpisodeEntry {
    pub number: u32,
    pub title: Option<String>,
    pub variants: Vec<VideoVariant>,
    pub search_key: String,
}

/// Un archivo concreto de un episodio.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoVariant {
    pub item: MediaItem,
    pub label: Option<String>,
}

#[derive(Debug)]
struct ParsedName {
    series: String,
    season: u32,
    episode: u32,
    title: Option<String>,
    variant: Option<String>,
}

impl VideoLibrary {
    /// Construye la biblioteca de video a partir de los elementos del catálogo.
    #[must_use]
    pub fn build(items: &[MediaItem]) -> Self {
        let mut series: BTreeMap<String, SeriesBuilder> = BTreeMap::new();
        let mut movies = Vec::new();

        for item in items.iter().filter(|item| item.kind == Kind::Video) {
            if let Some(parsed) = parse_name(&item.name) {
                let key = parsed.series.to_lowercase();
                let series_entry = series
                    .entry(key)
                    .or_insert_with(|| SeriesBuilder::new(parsed.series.clone()));

                series_entry
                    .seasons
                    .entry(parsed.season)
                    .or_default()
                    .episodes
                    .entry(parsed.episode)
                    .or_insert_with(|| EpisodeBuilder {
                        title: parsed.title.clone(),
                        variants: Vec::new(),
                    })
                    .variants
                    .push(VideoVariant {
                        item: item.clone(),
                        label: parsed.variant.clone(),
                    });
            } else {
                movies.push(item.clone());
            }
        }

        let mut result = Vec::with_capacity(series.len());

        for (_, builder) in series {
            let mut seasons: Vec<SeasonEntry> = builder
                .seasons
                .into_iter()
                .map(|(number, season)| {
                    let mut episodes: Vec<EpisodeEntry> = season
                        .episodes
                        .into_iter()
                        .map(|(number, episode)| {
                            let mut variants = episode.variants;
                            variants.sort_by(|left, right| {
                                variant_sort_key(left).cmp(&variant_sort_key(right))
                            });

                            let mut search_key =
                                episode.title.as_deref().unwrap_or_default().to_lowercase();
                            for variant in &variants {
                                search_key.push(' ');
                                search_key.push_str(&variant.item.name.to_lowercase());
                                if let Some(label) = &variant.label {
                                    search_key.push(' ');
                                    search_key.push_str(&label.to_lowercase());
                                }
                            }

                            EpisodeEntry {
                                number,
                                title: episode.title,
                                variants,
                                search_key,
                            }
                        })
                        .collect();

                    episodes.sort_by_key(|episode| episode.number);
                    let mut search_key = String::new();
                    for episode in &episodes {
                        search_key.push(' ');
                        search_key.push_str(&episode.search_key);
                    }

                    SeasonEntry {
                        number,
                        episodes,
                        search_key,
                    }
                })
                .collect();

            seasons.sort_by_key(|season| season.number);
            let episode_count = seasons.iter().map(|season| season.episodes.len()).sum();
            let mut search_key = builder.name.to_lowercase();
            for season in &seasons {
                search_key.push(' ');
                search_key.push_str(&season.search_key);
            }

            result.push(SeriesEntry {
                name: builder.name,
                seasons,
                episode_count,
                search_key,
            });
        }

        result.sort_by_key(|series| series.name.to_lowercase());
        movies.sort_by_key(|movie| movie.name.to_lowercase());

        Self {
            series: result,
            movies,
        }
    }

    /// Busca una serie por su nombre visible o clave normalizada.
    #[must_use]
    pub fn series(&self, name: &str) -> Option<&SeriesEntry> {
        self.series.iter().find(|series| series.name == name)
    }
}

#[derive(Debug)]
struct SeriesBuilder {
    name: String,
    seasons: BTreeMap<u32, SeasonBuilder>,
}

impl SeriesBuilder {
    fn new(name: String) -> Self {
        Self {
            name,
            seasons: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Default)]
struct SeasonBuilder {
    episodes: BTreeMap<u32, EpisodeBuilder>,
}

/// Datos derivados del nombre de archivo, para respaldos de facetas y de
/// `video_tags` cuando el contenedor no trae etiquetas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NameFallback {
    pub series: Option<String>,
    pub year: Option<u16>,
}

/// Serie y año derivados del nombre de archivo. Es el mismo parseo que usa
/// `VideoLibrary::build`; este respaldo nunca toca el contenido del archivo.
#[must_use]
pub fn parse_video_name(name: &str) -> NameFallback {
    NameFallback {
        series: parse_name(name).map(|parsed| parsed.series),
        year: year_in_name(name),
    }
}

/// Primer corrida de 4 dígitos entre 1900 y 2100 en el nombre.
fn year_in_name(name: &str) -> Option<u16> {
    let bytes = name.as_bytes();

    for window in bytes.windows(4) {
        if !window.iter().all(u8::is_ascii_digit) {
            continue;
        }

        let Ok(year) = std::str::from_utf8(window).unwrap().parse::<u16>() else {
            continue;
        };

        if (1900..=2100).contains(&year) {
            return Some(year);
        }
    }

    None
}

#[derive(Debug)]
struct EpisodeBuilder {
    title: Option<String>,
    variants: Vec<VideoVariant>,
}

/// Extrae serie, temporada, episodio, título y variante del nombre.
#[must_use]
fn parse_name(name: &str) -> Option<ParsedName> {
    let (start, end, season, episode) = find_season_episode(name)?;

    let series = clean_series(&name[..start]);
    let title = clean_episode_title(&name[end..]);
    let variant = variant_label(&name[end..]);

    Some(ParsedName {
        series,
        season,
        episode,
        title,
        variant,
    })
}

fn find_season_episode(name: &str) -> Option<(usize, usize, u32, u32)> {
    let bytes = name.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        if !bytes[index].eq_ignore_ascii_case(&b's') {
            index += 1;
            continue;
        }

        let Some((season, after_season)) = parse_digits(bytes, index + 1) else {
            index += 1;
            continue;
        };

        if after_season >= bytes.len() || !bytes[after_season].eq_ignore_ascii_case(&b'e') {
            index += 1;
            continue;
        }

        let Some((episode, after_episode)) = parse_digits(bytes, after_season + 1) else {
            index += 1;
            continue;
        };

        return Some((index, after_episode, season, episode));
    }

    None
}

fn parse_digits(bytes: &[u8], start: usize) -> Option<(u32, usize)> {
    let mut end = start;
    let mut value = 0u32;

    while end < bytes.len() && bytes[end].is_ascii_digit() {
        value = value
            .checked_mul(10)?
            .checked_add(u32::from(bytes[end] - b'0'))?;
        end += 1;
    }

    (end > start).then_some((value, end))
}

fn clean_series(prefix: &str) -> String {
    let cleaned = prefix
        .replace(['.', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    cleaned.trim_matches([' ', '-', '–', '—']).to_owned()
}

fn clean_episode_title(suffix: &str) -> Option<String> {
    let suffix = suffix.trim_start_matches(['.', '_', '-', ' ']);
    let mut words = Vec::new();

    for word in suffix.split(['.', '_', ' ']) {
        if word.is_empty() {
            continue;
        }

        if is_quality_token(word) {
            break;
        }

        words.push(word);
    }

    let title = words.join(" ").trim().to_owned();
    (!title.is_empty()).then_some(title)
}

fn variant_label(suffix: &str) -> Option<String> {
    let mut tokens = Vec::new();

    for word in suffix.split(['.', '_', ' ', '[', ']', '-']) {
        if is_quality_token(word) && !tokens.iter().any(|token| token == word) {
            tokens.push(word.to_owned());
        }
    }

    (!tokens.is_empty()).then(|| tokens.join(" · "))
}

fn is_quality_token(word: &str) -> bool {
    let word = word.to_ascii_lowercase();

    matches!(
        word.as_str(),
        "2160p"
            | "1080p"
            | "720p"
            | "480p"
            | "hdtv"
            | "web"
            | "webrip"
            | "web-dl"
            | "bluray"
            | "brrip"
            | "x264"
            | "x265"
            | "h264"
            | "h265"
            | "hevc"
            | "xvid"
            | "aac"
            | "ddp"
            | "dd5"
            | "dd5.1"
            | "dts"
    )
}

fn variant_sort_key(variant: &VideoVariant) -> String {
    variant
        .label
        .clone()
        .unwrap_or_else(|| variant.item.name.clone())
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{VideoLibrary, clean_episode_title, parse_name};
    use crate::model::{Kind, MediaItem, MediaOrigin, OpenAction, TrackMetadata};

    fn video(name: &str) -> MediaItem {
        MediaItem {
            id: 0,
            path: format!("/videos/{name}"),
            name: name.to_owned(),
            kind: Kind::Video,
            mtime: 0,
            size: 0,
            origin: MediaOrigin::LocalFile,
            open_action: OpenAction::XdgOpen,
            cover_ready: false,
            metadata: TrackMetadata::default(),
        }
    }

    #[test]
    fn parsea_serie_temporada_y_episodio() {
        let parsed = parse_name(
            "South.Park.S29E04.Guinea.Pig.On.A.Stick.1080p.HDTV.DD5.1.x264-Slurpuff.mkv",
        )
        .unwrap();

        assert_eq!(parsed.series, "South Park");
        assert_eq!(parsed.season, 29);
        assert_eq!(parsed.episode, 4);
        assert_eq!(parsed.title.as_deref(), Some("Guinea Pig On A Stick"));
    }

    #[test]
    fn agrupa_variantes_del_mismo_episodio() {
        let items = vec![
            video("South.Park.S29E04.Guinea.Pig.On.A.Stick.1080p.HDTV.DD5.1.x264-Slurpuff.mkv"),
            video("South.Park.S29E03.1080p.HEVC.x265-MeGusta[EZTVx.to].mkv"),
            video("south.park.s29e03.you.like.that.on.your.face.1080p.hdtv.x264-vietnam.mkv"),
        ];

        let library = VideoLibrary::build(&items);

        assert_eq!(library.series.len(), 1);
        assert_eq!(library.movies.len(), 0);
        assert_eq!(library.series[0].name, "South Park");
        assert_eq!(library.series[0].seasons.len(), 1);
        assert_eq!(library.series[0].seasons[0].episodes.len(), 2);

        let episode_3 = &library.series[0].seasons[0].episodes[0];
        assert_eq!(episode_3.number, 3);
        assert_eq!(episode_3.variants.len(), 2);
        assert!(
            episode_3
                .variants
                .iter()
                .all(|variant| variant.label.is_some())
        );

        let episode_4 = &library.series[0].seasons[0].episodes[1];
        assert_eq!(episode_4.number, 4);
        assert_eq!(episode_4.title.as_deref(), Some("Guinea Pig On A Stick"));
    }

    #[test]
    fn deja_las_peliculas_sueltas_aparte() {
        let items = vec![video("Una.Pelicula.2024.1080p.mkv")];
        let library = VideoLibrary::build(&items);

        assert_eq!(library.series.len(), 0);
        assert_eq!(library.movies.len(), 1);
    }

    #[test]
    fn limpia_titulos_de_calidad() {
        assert_eq!(clean_episode_title(".1080p.HEVC.x265-MeGusta"), None);
        assert_eq!(
            clean_episode_title(".you.like.that.on.your.face.1080p.hdtv.x264"),
            Some(String::from("you like that on your face"))
        );
    }

    #[test]
    fn extrae_el_anio_del_nombre() {
        assert_eq!(year_in_name("Movie.2009.1080p.mkv"), Some(2009));
        assert_eq!(year_in_name("South.Park.S29E03.mkv"), None);
        assert_eq!(year_in_name("pelicula-de-1975.avi"), Some(1975));
        assert_eq!(year_in_name("sin-numeros.mkv"), None);
    }

    #[test]
    fn el_respaldo_del_nombre_da_serie_y_anio() {
        let parsed = parse_video_name("South.Park.S29E03.2024.1080p.mkv");
        assert_eq!(parsed.series.as_deref(), Some("South Park"));
        assert_eq!(parsed.year, Some(2024));

        let parsed = parse_video_name("Una.Pelicula.1998.avi");
        assert_eq!(parsed.series, None);
        assert_eq!(parsed.year, Some(1998));
    }
}
