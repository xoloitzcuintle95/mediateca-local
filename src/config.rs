// SPDX-License-Identifier: MIT

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

/// Ajustes que persisten entre ejecuciones de la aplicación.
#[derive(Debug, Clone, Default, CosmicConfigEntry, Eq, PartialEq)]
#[version = 2]
pub struct Config {
    /// Carpeta raíz que se cataloga.
    pub catalog_root: String,
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn la_raiz_por_defecto_esta_vacia() {
        assert_eq!(Config::default().catalog_root, "");
    }
}
