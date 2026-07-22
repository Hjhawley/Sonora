# Sonora

Sonora is a desktop music player and tag editor for local MP3 libraries. It combines library management, playback, metadata editing, batch editing, and embedded artwork management in one app.

Currently supports MP3 files only.

## Features

* Import and scan one or more music folders
* Persistent SQLite-backed music library
* Searchable and sortable Track View
* Album View with grid and detail layouts
* Play, pause, seek, skip, shuffle, repeat, and volume controls
* Edit MP3 metadata individually or in batches
* Handle mixed metadata across multi-track selections
* Display, replace, remove, and extract embedded artwork
* Review hidden and missing tracks for library maintenance
* Preserve library roots, volume, and preferred view mode between sessions

## Download

Prebuilt versions are available from the GitHub Releases page.

Download the archive for your operating system, extract it, and run:

* `sonora.exe` on Windows
* `sonora` on Linux or macOS

## Build from source

### Requirements

* Rust and Cargo

### Build

```
cargo build --release
```

The compiled executable will be placed in `target/release/`.

### Run

```
cargo run --release
```