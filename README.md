Sonora
Sonora is a desktop music player and tag editor for local MP3 libraries. It combines playback, browsing albums, batch metadata editing, and embedded artwork management in one app.

Sonora is split into two main layers:
Core: scanning, library caching, tag reading/writing, embedded artwork, playback, and SQLite-backed persistence
GUI: app state, user interaction, search/sort/selection, and rendering for the library, inspector, and playback controls
On normal startup, Sonora loads cached library data from SQLite instead of rescanning the filesystem every launch.

Features
Import and scan local music folders
Persistent library backed by SQLite
Searchable, sortable Track View
Album View with album grid and album detail screens
Audio playback with play/pause, previous/next, shuffle, repeat, seek, and volume
MP3 metadata editing
Batch editing with mixed-field handling
Embedded artwork (display, add, replace, remove, extract as .png)
Manually hide albums within library directories
Identify missing files upon rescan

Usage
Launch Sonora
Add one or more music folders
Scan the library
Browse in Track View or Album View
Select tracks to inspect or edit metadata
Use the playback controls to listen
Architecture

Downloads
Prebuilt executables are available on the Releases page.

Build from source
Requirements
Rust
Cargo
Build

```bash
cargo build –release
```

Run

```bash
cargo run –release
```

Compiled binaries are placed in target/release/.
