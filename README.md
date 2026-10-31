# Mediateca Local

A local media catalog for the COSMIC desktop. Point it at a folder and browse
your music, videos and books — it only reads your files, never touches them.

<!-- screenshots: TODO -->

## What it does

It scans one folder, reads tags and embedded covers from your existing files,
and groups them: music by artist/album, video by series/season/episode, books
by author. Opening an item hands it to your system's default application.

What it does **not** do:

- It never writes to, moves or renames anything in your collection.
- It makes no network requests — everything runs locally.
- The repository link in the About dialog just opens your web browser.

## Supported formats

| Kind  | Formats        | Notes                                        |
| ----- | -------------- | -------------------------------------------- |
| Music | mp3, flac, ogg | tags and embedded covers                     |
| Video | mkv, mp4, avi  | identified by file name (`SxxEyy`); no posters |
| Books | epub, pdf      | epub metadata and covers; pdf listed plainly |

## Requirements

- GNU/Linux with the [COSMIC](https://github.com/pop-os/cosmic-epoch) libraries
- Rust >= 1.93 (via [rustup](https://rustup.rs/); the pinned libcosmic revision requires it)
- [`just`](https://github.com/casey/just)

No other system packages are required. The build script only reads
`resources/` and `i18n/` and writes to `target/xdgen/`.

## Build and run

```sh
just            # build (release)
just run        # build and run
just install    # install to ~/.local (untested with the current app ID)
```

## Getting good results

Mediateca Local does not fetch metadata from the internet and does not read
NFO files or movie posters. It relies on the tags and file names your files
already have. Before importing, it is worth tidying your collection with:

- **Music:** [MusicBrainz Picard](https://picard.musicbrainz.org/) — accurate
  artist/album/title tags and embedded art
- **Video:** [tinyMediaManager](https://www.tinyMediaManager.org/) —
  consistent `Series SxxEyy` file names
- **Books:** [Calibre](https://calibre-ebook.com/) — proper EPUB metadata

## Known limitations

- Books are shown as a flat list; no nested shelves.
- PDFs are catalogued without reading their metadata.
- Video is matched by file name only (`SxxEyy`); movies get icons, not posters.
- The first import can take a while on a spinning disk, with no progress display.
- Search does not normalize accents.
- The details panel misbehaves when the window is very narrow and tall.

## Tests

```sh
cargo test
```

## Built with AI assistance

This project was built with the help of AI coding models, and reviewed through
tests and daily use. See [AGENTS.md](./AGENTS.md) for the project rules.

## Contributing

Issues are welcome. [AGENTS.md](./AGENTS.md) describes the project's rules for
anyone (human or AI) working on it.

## Translations

Translations use [Fluent](https://projectfluent.org/) files in
[i18n/](./i18n) (`en` and `es`).

## Packaging

For distro packaging, vendor dependencies and build from the vendored sources:

```sh
just vendor
just build-vendored
just rootdir=/path/to/pkg prefix=/usr install
```

## License

MIT — see [LICENSE](./LICENSE).
