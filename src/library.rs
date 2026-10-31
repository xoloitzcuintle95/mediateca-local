// SPDX-License-Identifier: MIT

//! Agrupación de la música en artistas, álbumes y canciones.
//!
//! La carpeta no define el álbum: se usan las etiquetas. Los alias de artista
//! permiten unir variantes como `Iamdoechii` con `Doechii`.

use std::collections::{HashMap, HashSet};

use crate::model::{Kind, MediaItem, OpenAction};

/// Mapa de alias normalizados (minúsculas) a nombre canónico.
pub type AliasMap = HashMap<String, String>;

/// Identidad estable de un álbum dentro de la biblioteca.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AlbumKey {
    pub artist: String,
    pub album: String,
    pub year: Option<u16>,
}

/// Una canción dentro de un álbum.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrackEntry {
    pub id: i64,
    pub path: String,
    pub title: String,
    /// Créditos completos, tal como vienen en la etiqueta.
    pub artist: String,
    /// Posición mostrada, desde 1.
    pub position: u32,
    pub track_number: Option<u32>,
    pub filename_track: Option<u32>,
    pub cover_hash: Option<String>,
    pub cover_ready: bool,
    /// Acción de apertura heredada del tipo de medio.
    pub open_action: OpenAction,
    /// Texto precalculado para búsquedas sin volver a asignar en cada tecla.
    pub search_key: String,
}

/// Un álbum con sus canciones ordenadas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlbumEntry {
    pub key: AlbumKey,
    pub title: String,
    pub artist: String,
    pub year: Option<u16>,
    pub cover_hash: Option<String>,
    pub cover_ready: bool,
    pub tracks: Vec<TrackEntry>,
    pub search_key: String,
}

/// Un artista con sus álbumes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtistEntry {
    pub name: String,
    pub cover_hash: Option<String>,
    pub cover_ready: bool,
    pub albums: Vec<AlbumEntry>,
    pub search_key: String,
}

/// Biblioteca completa, construida en memoria a partir de los archivos.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Library {
    pub artists: Vec<ArtistEntry>,
    pub albums: Vec<AlbumEntry>,
    pub track_count: usize,
}

impl Library {
    /// Construye la biblioteca aplicando los alias de artista.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn build(items: &[MediaItem], aliases: &AliasMap) -> Self {
        let mut builders: HashMap<AlbumKey, Vec<TrackEntry>> = HashMap::new();

        for item in items.iter().filter(|item| item.kind == Kind::Music) {
            let metadata = &item.metadata;
            let raw_artist = metadata.creator.as_deref().unwrap_or_default();
            let raw_album_artist = metadata
                .collection_creator
                .as_deref()
                .unwrap_or(raw_artist)
                .trim();
            let artist = resolve_artist(raw_album_artist, aliases);
            let album = metadata.collection.clone().unwrap_or_default();
            let key = AlbumKey {
                artist: artist.clone(),
                album,
                year: metadata.year,
            };

            let title = metadata
                .title
                .clone()
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| item.name.clone());
            let search_key = format!("{title} {raw_artist}").to_lowercase();

            builders.entry(key).or_default().push(TrackEntry {
                id: item.id,
                path: item.path.clone(),
                title,
                artist: raw_artist.to_owned(),
                position: 0,
                track_number: metadata.track_number,
                filename_track: metadata.filename_track,
                cover_hash: metadata.cover_hash.clone(),
                cover_ready: item.cover_ready,
                open_action: item.open_action,
                search_key,
            });
        }

        let mut albums: Vec<AlbumEntry> = builders
            .into_iter()
            .map(|(key, mut tracks)| {
                sort_tracks(&mut tracks);
                let cover_hash = majority_cover(&tracks);
                let cover_ready = cover_hash.as_deref().is_some_and(|hash| {
                    tracks
                        .iter()
                        .any(|track| track.cover_hash.as_deref() == Some(hash) && track.cover_ready)
                });
                let mut search_key = format!("{} {}", key.album, key.artist).to_lowercase();
                for track in &tracks {
                    search_key.push(' ');
                    search_key.push_str(&track.search_key);
                }

                AlbumEntry {
                    title: key.album.clone(),
                    artist: key.artist.clone(),
                    year: key.year,
                    cover_hash,
                    cover_ready,
                    search_key,
                    key,
                    tracks,
                }
            })
            .collect();

        albums.sort_by(|left, right| {
            artist_sort_key(&left.artist)
                .cmp(&artist_sort_key(&right.artist))
                .then_with(|| right.year.cmp(&left.year))
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        });

        let mut by_artist: HashMap<String, Vec<AlbumEntry>> = HashMap::new();
        for album in &albums {
            by_artist
                .entry(album.artist.clone())
                .or_default()
                .push(album.clone());
        }

        let mut artists: Vec<ArtistEntry> = by_artist
            .into_iter()
            .map(|(name, albums)| {
                let cover_hash = albums.iter().find_map(|album| album.cover_hash.clone());
                let cover_ready = cover_hash.as_deref().is_some_and(|hash| {
                    albums
                        .iter()
                        .any(|album| album.cover_hash.as_deref() == Some(hash) && album.cover_ready)
                });
                let mut search_key = name.to_lowercase();
                for album in &albums {
                    search_key.push(' ');
                    search_key.push_str(&album.search_key);
                }

                ArtistEntry {
                    name,
                    cover_hash,
                    cover_ready,
                    albums,
                    search_key,
                }
            })
            .collect();

        artists.sort_by_key(|artist| artist_sort_key(&artist.name));

        let track_count = albums.iter().map(|album| album.tracks.len()).sum();

        Self {
            artists,
            albums,
            track_count,
        }
    }
}

/// Aplica un alias sin distinguir mayúsculas ni minúsculas.
#[must_use]
pub fn resolve_artist(name: &str, aliases: &AliasMap) -> String {
    let name = name.trim();

    if name.is_empty() {
        return String::new();
    }

    aliases
        .get(&name.to_lowercase())
        .cloned()
        .unwrap_or_else(|| name.to_owned())
}

fn artist_sort_key(name: &str) -> (bool, String) {
    (name.is_empty(), name.to_lowercase())
}

fn sort_tracks(tracks: &mut [TrackEntry]) {
    let distinct_tracks: HashSet<u32> = tracks
        .iter()
        .filter_map(|track| track.track_number)
        .filter(|number| *number > 0)
        .collect();

    if distinct_tracks.len() > 1 {
        tracks.sort_by(|left, right| {
            left.track_number
                .unwrap_or(u32::MAX)
                .cmp(&right.track_number.unwrap_or(u32::MAX))
                .then_with(|| {
                    left.filename_track
                        .unwrap_or(u32::MAX)
                        .cmp(&right.filename_track.unwrap_or(u32::MAX))
                })
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        });
    } else {
        tracks.sort_by(|left, right| {
            left.filename_track
                .unwrap_or(u32::MAX)
                .cmp(&right.filename_track.unwrap_or(u32::MAX))
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
        });
    }

    for (index, track) in tracks.iter_mut().enumerate() {
        track.position = u32::try_from(index + 1).unwrap_or(u32::MAX);
    }
}

/// Elige la portada más repetida; en empate, la primera en el orden de pistas.
fn majority_cover(tracks: &[TrackEntry]) -> Option<String> {
    let mut counts: HashMap<&str, (usize, usize)> = HashMap::new();

    for (index, track) in tracks.iter().enumerate() {
        if let Some(hash) = track.cover_hash.as_deref() {
            counts
                .entry(hash)
                .and_modify(|(count, _)| *count += 1)
                .or_insert((1, index));
        }
    }

    counts
        .into_iter()
        .max_by(
            |(hash_a, (count_a, index_a)), (hash_b, (count_b, index_b))| {
                count_a
                    .cmp(count_b)
                    .then_with(|| index_b.cmp(index_a))
                    .then_with(|| hash_a.cmp(hash_b))
            },
        )
        .map(|(hash, _)| hash.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{AlbumKey, Library, resolve_artist};
    use crate::model::{Kind, MediaItem, TagStatus, TrackMetadata};
    use std::collections::HashMap;

    #[allow(clippy::too_many_arguments)]
    fn item(
        name: &str,
        artist: &str,
        album: &str,
        title: &str,
        track: Option<u32>,
        filename_track: Option<u32>,
        year: Option<u16>,
        cover: Option<&str>,
    ) -> MediaItem {
        MediaItem {
            id: 0,
            path: format!("/musica/{name}"),
            name: name.to_owned(),
            kind: Kind::Music,
            mtime: 0,
            size: 0,
            origin: crate::model::MediaOrigin::LocalFile,
            open_action: crate::model::OpenAction::XdgOpen,
            cover_ready: cover.is_some(),
            metadata: TrackMetadata {
                creator: Some(artist.to_owned()),
                collection_creator: Some(artist.to_owned()),
                collection: Some(album.to_owned()),
                title: Some(title.to_owned()),
                track_number: track,
                track_total: None,
                filename_track,
                year,
                cover_hash: cover.map(ToOwned::to_owned),
                status: TagStatus::Complete,
            },
        }
    }

    fn aliases() -> HashMap<String, String> {
        HashMap::from([(String::from("iamdoechii"), String::from("Doechii"))])
    }

    #[test]
    fn aplica_alias_sin_distinguir_mayusculas() {
        let aliases = aliases();
        assert_eq!(resolve_artist("IAMDOECHII", &aliases), "Doechii");
        assert_eq!(resolve_artist("Iamdoechii", &aliases), "Doechii");
        assert_eq!(resolve_artist("Doechii", &aliases), "Doechii");
        assert_eq!(resolve_artist("  Doechii  ", &aliases), "Doechii");
    }

    #[test]
    fn agrupa_por_album_y_ano_no_por_carpeta() {
        let items = vec![
            item(
                "01 - A.mp3",
                "Doechii",
                "Album Uno",
                "A",
                Some(63),
                Some(1),
                Some(2020),
                None,
            ),
            item(
                "02 - B.mp3",
                "Doechii",
                "Album Dos",
                "B",
                Some(63),
                Some(2),
                Some(2021),
                None,
            ),
            item(
                "03 - C.mp3",
                "Doechii",
                "Album Uno",
                "C",
                Some(63),
                Some(3),
                Some(2020),
                None,
            ),
        ];

        let library = Library::build(&items, &HashMap::new());

        assert_eq!(library.albums.len(), 2);
        assert_eq!(library.track_count, 3);
        let uno = library
            .albums
            .iter()
            .find(|album| album.title == "Album Uno")
            .unwrap();
        assert_eq!(uno.tracks.len(), 2);
        assert_eq!(uno.year, Some(2020));
    }

    #[test]
    fn usa_prefijo_del_nombre_cuando_el_track_no_sirve() {
        let items = vec![
            item(
                "50 - B.mp3",
                "Doechii",
                "Album",
                "B",
                Some(63),
                Some(50),
                Some(2024),
                None,
            ),
            item(
                "49 - A.mp3",
                "Doechii",
                "Album",
                "A",
                Some(63),
                Some(49),
                Some(2024),
                None,
            ),
            item(
                "51 - C.mp3",
                "Doechii",
                "Album",
                "C",
                Some(63),
                Some(51),
                Some(2024),
                None,
            ),
        ];

        let library = Library::build(&items, &HashMap::new());
        let titles: Vec<&str> = library.albums[0]
            .tracks
            .iter()
            .map(|track| track.title.as_str())
            .collect();
        let positions: Vec<u32> = library.albums[0]
            .tracks
            .iter()
            .map(|track| track.position)
            .collect();

        assert_eq!(titles, ["A", "B", "C"]);
        assert_eq!(positions, [1, 2, 3]);
    }

    #[test]
    fn usa_track_number_cuando_es_valido() {
        let items = vec![
            item(
                "02 - B.mp3",
                "Doechii",
                "Album",
                "B",
                Some(2),
                Some(2),
                Some(2024),
                None,
            ),
            item(
                "01 - A.mp3",
                "Doechii",
                "Album",
                "A",
                Some(1),
                Some(1),
                Some(2024),
                None,
            ),
        ];

        let library = Library::build(&items, &HashMap::new());
        let titles: Vec<&str> = library.albums[0]
            .tracks
            .iter()
            .map(|track| track.title.as_str())
            .collect();

        assert_eq!(titles, ["A", "B"]);
    }

    #[test]
    fn elige_la_portada_mas_repetida() {
        let items = vec![
            item(
                "01 - A.mp3",
                "Doechii",
                "Album",
                "A",
                Some(63),
                Some(1),
                Some(2024),
                Some("uno"),
            ),
            item(
                "02 - B.mp3",
                "Doechii",
                "Album",
                "B",
                Some(63),
                Some(2),
                Some(2024),
                Some("dos"),
            ),
            item(
                "03 - C.mp3",
                "Doechii",
                "Album",
                "C",
                Some(63),
                Some(3),
                Some(2024),
                Some("dos"),
            ),
            item(
                "04 - D.mp3",
                "Doechii",
                "Album",
                "D",
                Some(63),
                Some(4),
                Some(2024),
                Some("dos"),
            ),
        ];

        let library = Library::build(&items, &HashMap::new());

        assert_eq!(library.albums[0].cover_hash.as_deref(), Some("dos"));
    }

    #[test]
    fn une_iamdoechii_con_doechii() {
        let items = vec![
            item(
                "01 - A.mp3",
                "Iamdoechii",
                "Album A",
                "A",
                Some(63),
                Some(1),
                Some(2020),
                None,
            ),
            item(
                "02 - B.mp3",
                "Doechii",
                "Album B",
                "B",
                Some(63),
                Some(2),
                Some(2024),
                None,
            ),
        ];

        let library = Library::build(&items, &aliases());

        assert_eq!(library.artists.len(), 1);
        assert_eq!(library.artists[0].name, "Doechii");
        assert_eq!(library.artists[0].albums.len(), 2);
    }

    #[test]
    fn mantiene_separados_albumes_con_el_mismo_titulo_y_ano_distinto() {
        let items = vec![
            item(
                "01 - A.mp3",
                "Doechii",
                "Mismo",
                "A",
                Some(63),
                Some(1),
                Some(2020),
                None,
            ),
            item(
                "02 - B.mp3",
                "Doechii",
                "Mismo",
                "B",
                Some(63),
                Some(2),
                Some(2021),
                None,
            ),
        ];

        let library = Library::build(&items, &HashMap::new());

        assert_eq!(library.albums.len(), 2);
        assert!(library.albums.iter().any(|album| album.key
            == AlbumKey {
                artist: String::from("Doechii"),
                album: String::from("Mismo"),
                year: Some(2020),
            }));
    }
}
