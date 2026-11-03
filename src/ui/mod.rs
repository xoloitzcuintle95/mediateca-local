// SPDX-License-Identifier: MIT

//! Construcción de la interfaz del catálogo.

mod books;
mod cards;
pub(crate) mod common;
mod music;
mod overview;
mod root;
mod status;
mod video;

pub use root::{CatalogView, view};
