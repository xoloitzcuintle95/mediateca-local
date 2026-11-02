# AGENTS.md — Perfil rust-cosmic (Mediateca Local)

## Propósito
Desarrollo de Mediateca Local: app de escritorio COSMIC (Pop!_OS 24.04) en
Rust con libcosmic que cataloga archivos locales (video, música, libros) y
los abre con la aplicación predeterminada del sistema. Código de producción
y tests. Toda la documentación del repo va en inglés.

## Stack y Comandos
- Rust + libcosmic (pinned), SQLite, Flatpak.
- After every change run `cargo check`. When a task is done, run
  `cargo clippy` and the tests. `just run` is executed by the user, not the
  agent. Never say "done" if it doesn't compile. `just build-release` also
  exists; Flatpak packaging only with explicit approval.
- Before using any libcosmic API, look it up in the template's code, in the
  libcosmic examples (downloaded under ~/.cargo/git/checkouts/) or with
  `cargo doc`. Never invent APIs from memory: libcosmic changes often.

## Reglas de Oro
- ALWAYS start from the template's code.
- If you fail 3 times with the same error, stop and explain the problem.
- Don't add dependencies without saying what they are for.
- The app only READS the catalog folders: never delete, move or modify the
  user's files. Covers and thumbnails are stored only in the user's cache
  directory (XDG_CACHE_HOME), never inside the collection.
- The app makes no network requests. It reads tags and covers only from
  local files.
- Tags are read with lofty in read-only mode; its write APIs are never used.
- Git: author `xoloitzcuintle95` + noreply, never real identity.

## Flujo de Trabajo
1. Planear: describe the change and the intended diff before touching code.
2. Pedir permiso: wait for the user's OK.
3. Ejecutar: minimal change → `cargo check` green.
4. Reportar: what changed, what was verified (check/clippy/test), what is
   left.

## Límites de dominio
- La bóveda personal es del perfil OG. El juego (Godot) es del perfil
  gamedev. El copy y Build in Public son del perfil marketing. Este perfil
  no escribe copy ni notas personales.

---

# Mediateca Local — media catalog for COSMIC (project rules)

## Media types
- Each media type is defined in one place: extensions, cover aspect ratio,
  names of its hierarchy levels, and how to open the file.

## Design (must look like an official COSMIC app)
- Use libcosmic widgets, not raw iced widgets when an equivalent exists.
- Standard COSMIC structure: header bar with search, side navigation bar
  (All / Music / Video / Books) and a context panel for details and settings.
- Colors, spacing and radii come from the theme (cosmic-theme). Zero hardcoded
  colors. Must look right in light and dark mode.
- Symbolic icons from the system theme, not emojis.
- Settings are stored with cosmic-config.

## Architecture
- App ID: `com.xoloitzcuintle95.MediatecaLocal`, defined in `src/model.rs`.
- Model-View-Update pattern: Message -> update -> view. No heavy logic in view.
- Scanning and the database run in async tasks: the UI never freezes.
- SQLite in the user's XDG data directory, not inside the project.
- Open files with `xdg-open`, as a separate process from the app.
- Modules: model, db, scanner, opener, ui/.

## Performance
- Re-scanning only re-reads tags and covers from new files, or files whose
  size or mtime changed. Everything else reuses the metadata and cover hash
  already stored in SQLite.
- Scanning, tag reading and thumbnail generation run off the UI thread.
- A cover is processed once per content; later requests reuse the cached
  thumbnail.
