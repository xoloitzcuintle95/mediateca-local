// SPDX-License-Identifier: MIT

//! Vista plana de libros en tarjetas.

use cosmic::Element;
use cosmic::cosmic_theme::Spacing;
use cosmic::iced::Length;

use crate::app::Message;
use crate::fl;
use crate::model::{Kind, MediaItem, ViewMode};

use super::CatalogView;
use super::cards::{book_grid, book_list};
use super::common::{empty, page_header, scrollable, view_mode_toggle};
use super::status::push_status;

pub(super) fn books_view<'a>(
    state: CatalogView<'a>,
    query: &str,
    spacing: Spacing,
) -> Element<'a, Message> {
    let books: Vec<&MediaItem> = state
        .items
        .iter()
        .filter(|item| item.kind == Kind::Book && book_matches(item, query))
        .collect();

    let mut content = page_header(
        fl!("nav-books"),
        fl!("books-count", count = books.len()),
        spacing,
        Some(view_mode_toggle(state.view_mode)),
    );
    content = push_status(content, &state);

    content = if books.is_empty() {
        content.push(empty(fl!("empty-generic")))
    } else {
        content.push(scrollable(match state.view_mode {
            ViewMode::Grid => book_grid(books, state.cache_dir, spacing),
            ViewMode::List => book_list(books, state.cache_dir, spacing),
        }))
    };

    content.height(Length::Fill).into()
}

fn book_matches(book: &MediaItem, query: &str) -> bool {
    query.is_empty()
        || book.name.to_lowercase().contains(query)
        || book
            .metadata
            .title
            .as_deref()
            .is_some_and(|title| title.to_lowercase().contains(query))
        || book
            .metadata
            .creator
            .as_deref()
            .is_some_and(|author| author.to_lowercase().contains(query))
}

#[cfg(test)]
mod tests {
    use super::book_matches;
    use crate::model::{Kind, MediaItem, MediaOrigin, OpenAction, TagStatus, TrackMetadata};

    fn book(title: &str, author: &str) -> MediaItem {
        MediaItem {
            id: 1,
            path: String::from("/libros/libro.epub"),
            name: String::from("libro.epub"),
            kind: Kind::Book,
            mtime: 0,
            size: 0,
            origin: MediaOrigin::LocalFile,
            open_action: OpenAction::XdgOpen,
            cover_ready: false,
            metadata: TrackMetadata {
                title: Some(String::from(title)),
                creator: Some(String::from(author)),
                status: TagStatus::Complete,
                ..TrackMetadata::default()
            },
        }
    }

    #[test]
    fn busca_por_nombre_titulo_y_autor() {
        let book = book("El libro", "Autora Prueba");

        assert!(book_matches(&book, "libro.epub"));
        assert!(book_matches(&book, "el libro"));
        assert!(book_matches(&book, "autora"));
        assert!(!book_matches(&book, "otro"));
    }
}
