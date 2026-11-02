// SPDX-License-Identifier: MIT

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

/// Ajustes que persisten entre ejecuciones de la aplicación.
#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 3]
pub struct Config {
    /// Carpeta raíz que se cataloga.
    pub catalog_root: String,
    /// Modo de visualización: "grid" o "list" (ver `ViewMode::as_str`).
    pub view_mode: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            catalog_root: String::new(),
            view_mode: String::from("grid"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn la_raiz_por_defecto_esta_vacia() {
        assert_eq!(Config::default().catalog_root, "");
    }

    #[test]
    fn el_modo_por_defecto_es_grid() {
        assert_eq!(Config::default().view_mode, String::from("grid"));
    }
}
