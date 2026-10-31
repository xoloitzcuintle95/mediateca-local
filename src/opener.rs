// SPDX-License-Identifier: MIT

//! Apertura de archivos con la aplicación predeterminada del sistema.
//!
//! Este módulo solo decide **cómo** se abre una ruta. Qué elemento se abre y
//! con qué acción (`OpenAction`) lo decide `media.rs`; aquí no se conocen
//! extensiones ni tipos de medio.

use std::path::Path;

/// Abre `path` con la aplicación predeterminada sin esperar a que termine.
///
/// Solo se aceptan rutas absolutas. `open` protege las rutas que empiezan por
/// `-`, pero una ruta relativa dependería del directorio de trabajo de la app.
///
/// # Errors
///
/// Devuelve un texto legible si la ruta no es absoluta o si ningún lanzador del
/// sistema pudo abrirla.
pub fn open_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("La ruta no es absoluta; no se abrió el archivo.".to_owned());
    }

    open::that_detached(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::sync::Mutex;

    #[cfg(unix)]
    static PATH_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn rechaza_rutas_relativas() {
        let error = super::open_path(std::path::Path::new("cancion.mp3")).unwrap_err();

        assert!(error.contains("absoluta"));
    }

    #[cfg(unix)]
    #[test]
    fn pasa_la_ruta_literal_a_xdg_open() {
        use std::os::unix::fs::PermissionsExt;

        let _guard = PATH_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let dir = std::env::temp_dir().join(format!(
            "mediateca-opener-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let args_file = dir.join("argumentos");
        let fake = dir.join("xdg-open");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
                args_file.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![dir.clone()];
        paths.extend(std::env::split_paths(&original_path));
        let test_path = std::env::join_paths(paths).unwrap();

        // SAFETY: el mutex evita que otra prueba de este módulo cambie PATH a la vez.
        unsafe {
            std::env::set_var("PATH", &test_path);
        }

        let path = dir.join("pista $(touch /tmp/pwned) \"comillas\".mp3");
        let result = super::open_path(&path);

        unsafe {
            std::env::set_var("PATH", &original_path);
        }

        result.unwrap();

        let received = std::fs::read_to_string(&args_file).unwrap();
        assert_eq!(received.trim_end_matches('\n'), path.to_string_lossy());
    }
}
