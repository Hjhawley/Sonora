mod core;

use iced::widget::{Column, button, column, row, scrollable, text, text_input};
use iced::{Length, Task};

use iced::futures::channel::oneshot;
use std::path::{Path, PathBuf};

use crate::core::types::TrackRow;

const TEST_ROOT: &str = "test";
const LIST_HEIGHT: f32 = 420.0;
const ROOTS_HEIGHT: f32 = 120.0;

struct Sonora {
    status: String,
    scanning: bool,

    root_input: String,
    roots: Vec<PathBuf>,

    tracks: Vec<TrackRow>,
}

impl Default for Sonora {
    fn default() -> Self {
        Self {
            status: "Add a folder, then Scan.".to_string(),
            scanning: false,

            root_input: String::new(),
            roots: Vec::new(),

            tracks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    RootInputChanged(String),
    AddRootPressed,
    RemoveRoot(usize),

    ScanLibrary,
    ScanFinished(Result<(Vec<TrackRow>, usize), String>), // (rows, tag_read_failures)
}

fn main() -> iced::Result {
    iced::application(Sonora::default, update, view)
        .title("Sonora")
        .run()
}

fn update(state: &mut Sonora, message: Message) -> Task<Message> {
    match message {
        Message::RootInputChanged(s) => {
            state.root_input = s;
            Task::none()
        }

        Message::AddRootPressed => {
            let input = state.root_input.trim();
            if input.is_empty() {
                return Task::none();
            }

            let p = PathBuf::from(input);

            if !Path::new(input).is_dir() {
                state.status = format!("Not a folder: {}", p.display());
                return Task::none();
            }

            if state.roots.contains(&p) {
                state.status = format!("Already added: {}", p.display());
                state.root_input.clear();
                return Task::none();
            }

            state.roots.push(p.clone());
            state.root_input.clear();
            state.status = format!("Added folder: {}", p.display());
            Task::none()
        }

        Message::RemoveRoot(i) => {
            if i < state.roots.len() && !state.scanning {
                let removed = state.roots.remove(i);
                state.status = format!("Removed folder: {}", removed.display());
            }
            Task::none()
        }

        Message::ScanLibrary => {
            if state.scanning {
                return Task::none();
            }

            state.scanning = true;
            state.tracks.clear();
            state.status = "Scanning…".to_string();

            // If user hasn't added any roots yet, fall back to ./test so you can keep iterating fast
            let roots_to_scan = if state.roots.is_empty() {
                vec![PathBuf::from(TEST_ROOT)]
            } else {
                state.roots.clone()
            };

            Task::perform(
                async move {
                    let (tx, rx) = oneshot::channel::<Result<(Vec<TrackRow>, usize), String>>();

                    std::thread::spawn(move || {
                        let _ = tx.send(crate::core::scan_and_read_roots(roots_to_scan));
                    });

                    rx.await
                        .map_err(|_| "Scan thread dropped without returning".to_string())?
                },
                Message::ScanFinished,
            )
        }

        Message::ScanFinished(result) => {
            state.scanning = false;

            match result {
                Ok((mut rows, tag_failures)) => {
                    rows.sort_by(|a, b| a.path.cmp(&b.path));

                    state.status = if tag_failures == 0 {
                        format!("Loaded {} tracks", rows.len())
                    } else {
                        format!(
                            "Loaded {} tracks ({} tag read failures)",
                            rows.len(),
                            tag_failures
                        )
                    };

                    state.tracks = rows;
                }
                Err(e) => {
                    state.status = format!("Error: {e}");
                    state.tracks.clear();
                }
            }

            Task::none()
        }
    }
}

fn view(state: &Sonora) -> Column<'_, Message> {
    // --- Roots UI ---
    let root_input = text_input("Add folder path (ex: H:\\music)", &state.root_input)
        .on_input(Message::RootInputChanged)
        .on_submit(Message::AddRootPressed)
        .width(Length::Fill);

    let add_btn = if state.scanning {
        button("Add")
    } else {
        button("Add").on_press(Message::AddRootPressed)
    };

    let add_row = row![root_input, add_btn].spacing(8);

    let mut roots_list = column![];
    for (i, p) in state.roots.iter().enumerate() {
        let remove_btn = if state.scanning {
            button("×")
        } else {
            button("×").on_press(Message::RemoveRoot(i))
        };

        roots_list = roots_list.push(row![text(p.display().to_string()), remove_btn].spacing(8));
    }

    let roots_panel = scrollable(roots_list.spacing(6)).height(Length::Fixed(ROOTS_HEIGHT));

    // --- Track list ---
    let mut track_list = column![];
    for t in &state.tracks {
        track_list = track_list.push(text(format_track_row(t)));
    }

    let scan_btn = if state.scanning {
        button("Scanning…")
    } else {
        button("Scan Library").on_press(Message::ScanLibrary)
    };

    column![
        text("Sonora (roots + scan + tag read)"),
        text(&state.status),
        add_row,
        roots_panel,
        scan_btn,
        scrollable(track_list.spacing(10)).height(Length::Fixed(LIST_HEIGHT)),
    ]
    .spacing(12)
}

fn format_track_row(t: &TrackRow) -> String {
    let title = t.title.as_deref().unwrap_or_else(|| {
        t.path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Title")
    });

    let artist = t.artist.as_deref().unwrap_or("Unknown Artist");
    let album = t.album.as_deref().unwrap_or("Unknown Album");
    let track_no = t
        .track_no
        .map(|n| n.to_string())
        .unwrap_or_else(|| "??".to_string());

    format!(
        "#{track_no} — {artist} — {title} ({album})\n{}",
        t.path.display()
    )
}
