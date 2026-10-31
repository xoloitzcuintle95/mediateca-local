# Project state — Mediateca Local

Reference commit: **`f043b7c`** (`Translate public docs to English, polish README`), no new commit yet. In this pass this document was translated to English; all in-repo documents are now English-only. It was also decided NOT to create a wiki: all content lives in README/PENDIENTES/ESTADO inside the repo. Clippy and `cargo test` (80 tests) pass.

Limit of this document: **200 lines**.

## 1. What it is and how to run it

A desktop app for COSMIC in Rust + libcosmic. It catalogs local music, video and book files; it never modifies the collection.

- Visible name: **Mediateca Local**
- Crate/binary: `mediateca-local`
- ID: `com.xoloitzcuintle95.MediatecaLocal`, defined only in `src/model.rs`
- Data: `~/.local/share/com.xoloitzcuintle95.MediatecaLocal/catalogo.db`
- Cache: `~/.cache/com.xoloitzcuintle95.MediatecaLocal/covers/`
- Configuration: `~/.config/com.xoloitzcuintle95.MediatecaLocal/` (cosmic-config; `catalog_root` empty until a folder is chosen)

Commands: `cargo check`, `cargo clippy`, `just build-release`. `just run` is executed by the user, not the agent. The GUI was not opened during this review.

## 2. Modules

| File | Lines | Responsibility |
|---|---:|---|
| `src/main.rs` | 34 | Startup, i18n and `cosmic::app::run`. |
| `src/app.rs` | 1250 | Model, messages, routes, folder picker, initial load, scanning, opening and drawers. |
| `src/config.rs` | 21 | `catalog_root` in cosmic-config, version 2; empty default. |
| `src/covers.rs` | 744 | Bars, crop/fit, JPEG thumbnails v2, cache and cover limits. |
| `src/db.rs` | 1451 | SQLite, migrations, upsert, safe deletion, aliases, music and EPUB enrich. |
| `src/epub.rs` | 302 | Read-only EPUB title, authors and cover with limits. |
| `src/i18n.rs` | 101 | Loads Fluent; `fl!` and `get(key)`. |
| `src/library.rs` | 584 | Music grouping artist → album → track. |
| `src/media.rs` | 116 | Single table of types: extensions, aspect, hierarchy, opening. |
| `src/model.rs` | 212 | `Kind`, metadata, `MediaItem`, summary and warnings. |
| `src/opener.rs` | 93 | Opens a path with the default app; knows nothing about extensions. |
| `src/scanner.rs` | 234 | Disk traversal, hidden files, mtime, size, skipping non-UTF-8 paths. |
| `src/tags.rs` | 372 | MP3 tag reading with lofty and `posix_fadvise(RANDOM)`, read-only. |
| `src/video.rs` | 426 | Series/Season/Episode parser and variants. |
| `src/ui/mod.rs` | 14 | Facade of `ui/`: declares modules and re-exports `CatalogView` and `view`. |
| `src/ui/root.rs` | 88 | `CatalogView`, empty state and per-page view dispatch. |
| `src/ui/overview.rs` | 58 | "All" view: summary and album grid. |
| `src/ui/music.rs` | 241 | Artists, albums and tracks views. |
| `src/ui/video.rs` | 466 | Series, seasons, episodes and movies views. |
| `src/ui/books.rs` | 94 | Books view with 2:3 cards and title/author search. |
| `src/ui/cards.rs` | 393 | Cards, grids, thumbnails and title cropping; includes its tests. |
| `src/ui/common.rs` | 99 | Headers, lists, labels and search helpers. |
| `src/ui/status.rs` | 51 | Notices and scan/open states. |

The **details** and **aliases** panels are not in `ui/`: they live in `src/app.rs` (`details_content` and `aliases_content`).

Total: 7444 lines in `src/`. Of those, **700 lines of `db.rs` are tests**, **149 of `epub.rs`**, **66 of `opener.rs`**, **35 of `ui/books.rs`** and **22 of `ui/cards.rs`**.

## 3. SQLite

Current schema: **version 4** (`PRAGMA user_version = 4`). `open_at` migrates only if the version is lower; on normal opens it does not repeat the schema or the migrations. Enables `journal_mode=WAL` and `busy_timeout=5000 ms`.

Table `files`: `id`, `path UNIQUE`, `name`, `kind`, `creator`, `collection_creator`, `collection`, `title`, `track_number`, `track_total`, `filename_track`, `year`, `cover_hash`, `tag_status`, `mtime`, `size`.

Table `artist_aliases`: `alias` (PK `COLLATE NOCASE`), `canonical`; seed `iamdoechii → Doechii` only in the first migration.

Indexes: `idx_files_kind(kind)`, `idx_files_collection(collection_creator, collection, year)`.

Migrations: add columns, rename `artist→creator`, `album_artist→collection_creator`, `album→collection`, replace the old index. Copies databases/caches from `com.diegomendoza.Contento` and then `com.diegomendoza.Catalogo`; never overwrites or deletes the old ones.

## 4. Flow of a scan

1. `init` launches `db::load_cached` in `spawn_blocking` only if `catalog_root` is not empty.
2. When `CachedLoaded` arrives, saved items, library, aliases and video are painted.
3. If there is a root, `ScanRequested` is emitted: the UI keeps what was saved and shows "Scanning…". If the root is empty, no scan happens and the empty state with the folder picker is shown.
4. `db_path` copies the legacy database if the new one does not exist; `open_at` migrates only if needed.
5. `readable_root` validates the root; on failure it writes nothing and returns the saved data with a warning.
6. `scanner::scan` traverses with WalkDir, prunes hidden files (`.Trash-1000` included), classifies and records mtime/size.
7. `enrich_items` distributes music and EPUB over a pool of at most 4 threads. If mtime/size did not change and `cover_ready`, it reuses metadata; otherwise it reads tags or rbook and generates a thumbnail. Every file runs inside `catch_unwind`: a panic becomes `TagStatus::Error` for that file and does not abort the scan. PDFs keep only the file name.
8. One transaction upserts by `path`; deletes missing rows only if the scan was complete, did not end up empty with prior rows under the root, and the paths are under the root.
9. Rows are reloaded, covers are marked ready and `Library` is built; the app builds `VideoLibrary`.
10. `ScanFinished` updates the UI.

`EmptyRoot` rule: if the scan finds no catalogable file and the database has rows under the root, the findings (zero) are saved but **nothing is deleted** and `ScanWarning::EmptyRoot` is returned; the UI shows it the same as `RootUnavailable`. Limitation: if the user emptied the folder on purpose, the rows stay and the warning repeats until missing-row cleanup exists.

## 5. Rules from AGENTS.md

- Start from the template; verify libcosmic APIs before using them.
- `cargo check` after every change; at close, `cargo clippy` and tests. `just run` is executed by the user, not the agent.
- Don't add dependencies without explaining why.
- The app only reads the collection; thumbnails only in the XDG cache.
- The app makes no network requests. It reads tags and covers only from local files.
- lofty read-only.
- Each media type is defined in one place (extensions, aspect, levels, opening).
- libcosmic widgets/theme, no hardcoded colors; symbolic icons.
- **MVU** (Model-View-Update): logic in `update`, non-blocking UI, SQLite in XDG.
- Performance: re-read only new files or ones with changed mtime/size; reuse thumbnails; work off the UI thread.

## 6. Decisions made

- **Read-only / no network:** the collection is never written to; lofty and rbook are used only for reading; no HTTP clients.
- **Covers:** bars are detected by comparing with the border color (tolerance 12/255). In the initial inspection, 31/31 had 276–280 px bars and a 720–728×720 interior. Music 1:1 (512×512); without bars, `fit`; video/books 2:3 (512×768). Cache v2; v1 16:9 unused. The book cover moves ~50% more pixels than the square one (393,216 vs 262,144); not optimized. All sources go through `covers::ensure_thumbnail`, which rejects more than 8 MiB or 8 MP before hashing or decoding, requires dimensions greater than zero and uses a limits-capped `ImageReader`. `local_cover` checks the size with `fs::metadata` before reading the file. Music's embedded cover was already loaded into memory by lofty; the filter avoids the second copy but cannot avoid that first load.
- **EPUB:** `rbook 0.7.12` with no features; reads title, authors and cover. Cover limited to 8 MiB and 8 MP; a corrupt or metadata-less EPUB falls back to the file name and `TagStatus::Error`/`Missing`. `catch_unwind` isolates possible panics. `creator` stores the author and `title` the title; no schema change.
- **Aliases:** editable table, case-insensitive; applied when grouping by `collection_creator`.
- **Media types:** `src/media.rs` is the single source of extensions, aspect, hierarchy and `OpenAction`.
- **Catalog folder:** single editable root from "View → Catalog folder…" or from the empty state. Uses `cosmic::dialog::file_chooser`; saved with `write_entry` and the existing `watch_config` triggers the scan. Non-UTF-8 paths, `/` and the whole home folder are rejected with `ScanWarning::InvalidCatalogRoot`. Non-UTF-8 paths inside the collection are skipped and count as traversal errors, so the scan is incomplete and deletes no rows. Changing the root does not delete rows of the previous root.
- **No root:** empty `catalog_root`; no cache load, no scan, and the view shows "Choose the folder of your catalog".
- **Picker bug fixed:** with a temporary `XDG_CONFIG_HOME`, `watch_config` uses the configuration daemon over DBus (feature `dbus-config`), which watches the real configuration; the file was saved to the temporary path but no `UpdateConfig` arrived. Not a failure for new users: it is an artifact of testing with an isolated XDG config. Now, after a successful save, the app updates the config in memory and requests the scan; an `UpdateConfig` with the same root does not duplicate it. To test the first scan, change only `XDG_DATA_HOME` and `XDG_CACHE_HOME`; with a temporary `XDG_CONFIG_HOME` the DBus daemon does not see the config.
- **Opening:** `media.rs` decides the `OpenAction`; `src/opener.rs` only runs `open::that_detached(&Path)` in `spawn_blocking`, rejects relative paths and turns failure into a notice. `LaunchUrl` only opens URLs that start with `https://` with a non-empty destination. The About repository link is omitted while `CARGO_PKG_REPOSITORY` is still the TODO.
- **Interaction:** a single click on a track, episode or book opens the details panel after 400 ms; double click opens the file without showing or leaving the panel open; the panel's Open button uses the same `OpenItem`.
- **Details panel:** it docks beside the content (`context_is_overlay = false`). The cause of the bug was that, as an overlay, libcosmic captured every click of the content and the breadcrumb never received `GoToArtists`; that is why going back was impossible with the panel open. In narrow windows the content shrinks.
- **Cards:** fixed uniform height; title 2 lines with ellipsis, creator 1, metadata 1. The tallest is not measured; iced offers no max row height. Responsive grid with fixed width and space distributed; fallback in `PENDIENTES.md`.
- **Video:** file name as the only source; `SxxEyy`; variants of the same episode grouped; movies with no separate pattern; icons, not posters.
- **Generic columns:** `creator` and `collection` replace `artist`/`album` in the model and SQLite.
- **Optimization:** `cover_ready` avoids `fs::metadata` per card; search keys are precomputed lowercase; a single connection for aliases; thumbnail pool.
- **Debt:** `ScanWarning` covers scan, opening and invalid root; the name is no longer exact and renaming it is pending.

## 7. What is NOT done

Copied from `PENDIENTES.md`:

- Visually check the responsive grid in very wide and very narrow windows. If the column split fails in some case, use the `FlexRow` with `SpaceBetween` fallback and note it here.
- Check bar cropping with new covers that have light uniform backgrounds.
- Confirm on screen that both `S29E03` variants show under the same episode with their label.
- Built-in viewers/players: will not be implemented; opening stays external.
- Review old v1 thumbnails (16:9) unused in the cache.

Also, `PENDIENTES.md` has nine points marked "unverified": local per-directory cover, `cover.jpg` change detection, TPE2 grouping, retrying failed tags, `tokio` features, movie posters, the 8-thread race, and the faster filter/JPEG. (The `rust-version`/README point was completed and removed.)

## 8. Dependencies

| Dependency | Version | Use |
|---|---|---|
| `dirs` | 7 | XDG data/cache paths. |
| `i18n-embed` | 0.16 | Fluent translations. |
| `i18n-embed-fl` | 0.10 | `fl!` macro. |
| `image` | 0.25.10 | Decode PNG/JPEG and write thumbnails. |
| `lofty` | 0.25.5 | MP3 tags and covers, read-only. |
| `rbook` | 0.7.12 (no features) | EPUB title, authors and cover, read-only. |
| `open` | 5.4.5 | Open files with the default app (`that_detached`); also the repository URL in About. |
| `rusqlite` | 0.41 (`bundled`) | Embedded SQLite. |
| `rust-embed` | 8.8.0 | Embedded translations. |
| `rustix` | 1.1.6 (`fs`, Linux only) | `posix_fadvise(RANDOM)` when reading tags; no `unsafe`. |
| `sha2` | 0.11.1 | Cover hashing. |
| `tokio` | 1.48 (`full`) | `spawn_blocking` and runtime. |
| `walkdir` | 2.5 | Traversal with pruning. |
| `libcosmic` | git master | UI, theme, Application, config. |
| `xdgen` (build) | 0.1 | `.desktop` and metainfo. |

`zip` 9 is a dev-dependency only, to generate a test EPUB.

Packaging: `build.rs` generates `target/xdgen/app.desktop` and `app.metainfo.xml`; `just install` renames them to the ID, installs the icon at `share/icons/hicolor/scalable/apps/com.xoloitzcuintle95.MediatecaLocal.svg` and the metainfo at `share/metainfo/`. `appstreamcli validate --no-net` keeps the `url-homepage-missing` alert because the repository URL is a TODO; `desktop-file-validate` passes.

## 9. Performance before/after

Measurements with **temporary database and cache**, release, 292 tracks, 3 videos and 208 unique covers. The collection lives on a **spinning disk** (`/dev/sda1`, `ROTA=1`), so seek time dominates.

| Scenario | Before | After |
|---|---:|---:|
| First scan, empty cache | **39.08 s** | **26.46 s** |
| Second scan | **17.15 ms** | **22.43 ms** |
| Tags read / reused (2nd) | 292 / 292 | 292 / 292 |
| v2 thumbnails generated | 208 | 208 |

The first-scan improvement was ~32%. The second scan is measurement noise (~ms).

**`fadvise` (Linux only):** lofty already read only the tag, but kernel *readahead* pulled ~5.6× more data: **1,088 → 195 MiB** with `POSIX_FADV_RANDOM`. Still, the wall-clock gain is moderate because random seeks persist. **User's cold measurement with 4 threads: ~22.6 s vs ~26.5 s originally (~15%).** The remaining bottleneck is disk seek time. **First-import progress is still pending.**

Incremental `cargo build --release`: ~28 s. `cargo check`, `cargo clippy --all-targets -- -D warnings` and **80 tests** pass.

User measurements with the release binary and the GUI open, **point-in-time**:

- CPU at rest: **0 ticks in 30 s**.
- RSS at open: **~209 MB** with 292 tracks and 208 covers.
- RSS after browsing: **~208 MB**. No growth observed.
- Navigation tested: music → artists → albums → tracks → back; then videos; then books; then music again.
- Rough reference: COSMIC Files uses **~148 MB**.
- It is **a single test session**: one sample after browsing, no growth observed; not a formal benchmark.

The earlier **4.40 s** measurement (200 tracks, 31 reads, 144 thumbnails) **was not a first scan**: it was an incremental scan over the real database and cache, with only 31 new/changed files and 144 thumbnails written. It is not comparable with the figures above. Measured today: 292 tracks, 3 videos, 0 books and 208 unique covers; the real number can change if the disk changes.

## Verified

- Open-failure notice when no launchers are available: it appeared and the app did not freeze.
- Real `xdg-open` opened a file with a weird name.

## Not verified

- Time from opening the GUI to seeing content: not measured. CPU at rest and RSS (at open and after browsing) were already measured by the user; see §9.
- Real/responsive appearance and the `S29E03` variants on screen.
- Folder picker, XDG portal and empty state in the GUI.
- Real writes to cosmic-config from the picker (the in-memory transition is tested).
- Real click on the breadcrumb with the details panel docked (only verified in code and with an `update` test).
- Real EPUBs with odd structures and the cover limits in the GUI.
- Icon rendered on the real desktop and the final repository URL.
- Configuration migration between IDs (only database and cache).
