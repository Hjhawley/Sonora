Sonora
Sonora is a desktop music player and tag editor for local MP3 libraries.

It combines playback, library browsing, metadata editing, embedded artwork management, and batch editing in one app.

Features
Import and scan local music folders
Persistent library backed by SQLite
Searchable, sortable Track View
Album View with album grid and album detail screens
Audio playback with play/pause, previous/next, shuffle, repeat, seek, and volume
MP3 metadata editing
Batch editing with mixed-field handling
Embedded artwork support:
display
replace
remove
extract
Hidden / Missing library scopes for maintenance and cleanup
Persistent app state, including saved roots, volume, and preferred view mode
Downloads
Prebuilt binaries are available on the Releases page.

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

Usage
Launch Sonora
Add one or more music folders
Scan the library
Browse in Track View or Album View
Select tracks to inspect or edit metadata
Use the playback controls to listen
Architecture
Sonora is split into two main layers:

Core: scanning, library caching, tag reading/writing, embedded artwork, playback, and SQLite-backed persistence
GUI: app state, user interaction, search/sort/selection, and rendering for the library, inspector, and playback controls
On normal startup, Sonora loads cached library data from SQLite instead of rescanning the filesystem every launch.
