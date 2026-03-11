//! core/playback/engine.rs
//! playback driver

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use rodio::{OutputStream, OutputStreamBuilder, Sink};

use super::decoder::open_source_at_ms;
use super::{PlayerCommand, PlayerEvent};

const TICK_MS: u64 = 200;

#[derive(Debug, Clone)]
struct QueuedTrack {
    path: PathBuf,
    duration_ms: Option<u64>,
    start_ms: u64,
}

pub struct PlaybackEngine {
    // Keep alive for lifetime of engine.
    stream: OutputStream,

    // One persistent sink per playback session.
    sink: Option<Sink>,

    // Current track metadata.
    current_path: Option<PathBuf>,
    current_duration_ms: Option<u64>,
    current_start_ms: u64,

    // Absolute sink position (ms) when the current track began.
    current_track_sink_origin_ms: u64,

    // How long the current queued segment actually plays inside the sink.
    // For a seeked track, this is duration - start_ms.
    current_track_playback_len_ms: Option<u64>,

    // Upcoming appended tracks.
    queued_tracks: VecDeque<QueuedTrack>,

    // Monotonic id for each logical track/session transition.
    next_playback_id: u64,
    current_playback_id: Option<u64>,

    // Track current volume so seek/play can apply it to the sink.
    volume: f32,

    // Prevent duplicate TrackEnded events for the same queue exhaustion.
    ended_emitted: bool,

    event_tx: Sender<PlayerEvent>,
}

impl PlaybackEngine {
    pub fn new(event_tx: Sender<PlayerEvent>) -> Result<Self, String> {
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| format!("Audio init failed: {e}"))?;

        Ok(Self {
            stream,
            sink: None,
            current_path: None,
            current_duration_ms: None,
            current_start_ms: 0,
            current_track_sink_origin_ms: 0,
            current_track_playback_len_ms: None,
            queued_tracks: VecDeque::new(),
            next_playback_id: 1,
            current_playback_id: None,
            volume: 1.0,
            ended_emitted: false,
            event_tx,
        })
    }

    pub fn run(&mut self, command_rx: Receiver<PlayerCommand>) {
        let tick = Duration::from_millis(TICK_MS);

        loop {
            match command_rx.recv_timeout(tick) {
                Ok(cmd) => {
                    if self.handle_command(cmd) {
                        break;
                    }
                    while let Ok(cmd) = command_rx.try_recv() {
                        if self.handle_command(cmd) {
                            return;
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            self.tick();
        }

        self.stop_internal();
    }

    fn handle_command(&mut self, cmd: PlayerCommand) -> bool {
        match cmd {
            PlayerCommand::PlayFile(path) => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] PlayFile {}", path.display());

                if let Err(e) = self.play_file_at(path, 0, true) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                }
            }

            PlayerCommand::QueueFile(path) | PlayerCommand::SetNextFile(path) => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] QueueFile {}", path.display());

                if let Err(e) = self.queue_file(path) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                }
            }

            PlayerCommand::ClearQueue | PlayerCommand::ClearNextFile => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] ClearQueue");
                self.queued_tracks.clear();
            }

            PlayerCommand::Pause => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Pause");

                if let (Some(sink), Some(playback_id)) = (&self.sink, self.current_playback_id) {
                    sink.pause();
                    let _ = self.event_tx.send(PlayerEvent::Paused { playback_id });
                }
            }

            PlayerCommand::Resume => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Resume");

                if let (Some(sink), Some(playback_id)) = (&self.sink, self.current_playback_id) {
                    sink.play();
                    let _ = self.event_tx.send(PlayerEvent::Resumed { playback_id });
                }
            }

            PlayerCommand::Stop => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Stop");

                let stopped_id = self.current_playback_id;
                self.stop_internal();

                if let Some(playback_id) = stopped_id {
                    let _ = self.event_tx.send(PlayerEvent::Stopped { playback_id });
                }
            }

            PlayerCommand::Seek(ms) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[ENGINE] Seek(ms={}) current_path={:?}",
                    ms,
                    self.current_path.as_ref().map(|p| p.display().to_string())
                );

                let Some(path) = self.current_path.clone() else {
                    return false;
                };

                let resume_playing = self.sink.as_ref().map(|s| !s.is_paused()).unwrap_or(true);

                if let Err(e) = self.play_file_at(path, ms, resume_playing) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                } else if let Some(playback_id) = self.current_playback_id {
                    let _ = self.event_tx.send(PlayerEvent::Position {
                        playback_id,
                        position_ms: ms,
                    });
                }
            }

            PlayerCommand::SetVolume(v) => {
                self.volume = v.clamp(0.0, 1.0);

                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] SetVolume {}", self.volume);

                if let Some(sink) = &self.sink {
                    sink.set_volume(self.volume);
                }
            }

            PlayerCommand::Shutdown => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Shutdown");
                return true;
            }
        }

        false
    }

    fn tick(&mut self) {
        let Some(playback_id) = self.current_playback_id else {
            return;
        };

        let (sink_pos_ms, sink_empty) = {
            let Some(sink) = &self.sink else {
                return;
            };
            (sink.get_pos().as_millis() as u64, sink.empty())
        };

        let rel_ms = sink_pos_ms.saturating_sub(self.current_track_sink_origin_ms);
        let position_ms = self.current_start_ms + rel_ms;

        let _ = self.event_tx.send(PlayerEvent::Position {
            playback_id,
            position_ms,
        });

        // Advance logical track metadata if the sink has already crossed into queued sources.
        while let Some(cur_playback_len) = self.current_track_playback_len_ms {
            let rel_ms = sink_pos_ms.saturating_sub(self.current_track_sink_origin_ms);

            if rel_ms < cur_playback_len {
                break;
            }

            let Some(next_track) = self.queued_tracks.pop_front() else {
                break;
            };

            self.current_track_sink_origin_ms = self
                .current_track_sink_origin_ms
                .saturating_add(cur_playback_len);

            self.current_path = Some(next_track.path.clone());
            self.current_duration_ms = next_track.duration_ms;
            self.current_start_ms = next_track.start_ms;
            self.current_track_playback_len_ms = next_track
                .duration_ms
                .map(|d| d.saturating_sub(next_track.start_ms));

            let next_playback_id = self.alloc_playback_id();
            self.current_playback_id = Some(next_playback_id);

            #[cfg(debug_assertions)]
            eprintln!(
                "[ENGINE] Advanced to queued track playback_id={} path={} duration_ms={:?}",
                next_playback_id,
                next_track.path.display(),
                next_track.duration_ms
            );

            let _ = self.event_tx.send(PlayerEvent::Started {
                playback_id: next_playback_id,
                path: next_track.path,
                duration_ms: next_track.duration_ms,
                start_ms: next_track.start_ms,
            });
        }

        // Entire sink exhausted = whole queue finished.
        if sink_empty && self.current_path.is_some() && !self.ended_emitted {
            self.ended_emitted = true;
            let final_id = self.current_playback_id.unwrap_or(playback_id);
            let _ = self.event_tx.send(PlayerEvent::TrackEnded {
                playback_id: final_id,
            });
            self.stop_internal();
        }
    }

    fn queue_file(&mut self, path: PathBuf) -> Result<(), String> {
        // If nothing is currently playing, QueueFile acts like PlayFile.
        if self.sink.is_none() || self.current_path.is_none() {
            return self.play_file_at(path, 0, true);
        }

        let Some(sink) = &self.sink else {
            return self.play_file_at(path, 0, true);
        };

        let (src, duration_ms) = open_source_at_ms(&path, 0)?;
        sink.append(src);

        self.queued_tracks.push_back(QueuedTrack {
            path,
            duration_ms,
            start_ms: 0,
        });

        Ok(())
    }

    fn play_file_at(
        &mut self,
        path: PathBuf,
        start_ms: u64,
        resume_playing: bool,
    ) -> Result<(), String> {
        self.stop_internal();

        let sink = Sink::connect_new(self.stream.mixer());
        sink.set_volume(self.volume);

        // decoder is responsible for seek + any fallback skipping.
        let (src, duration_ms) = open_source_at_ms(&path, start_ms)?;
        sink.append(src);

        if resume_playing {
            sink.play();
        } else {
            sink.pause();
        }

        let playback_id = self.alloc_playback_id();

        self.sink = Some(sink);
        self.current_path = Some(path.clone());
        self.current_duration_ms = duration_ms;
        self.current_start_ms = start_ms;
        self.current_track_sink_origin_ms = 0;
        self.current_track_playback_len_ms = duration_ms.map(|d| d.saturating_sub(start_ms));
        self.current_playback_id = Some(playback_id);
        self.queued_tracks.clear();
        self.ended_emitted = false;

        #[cfg(debug_assertions)]
        eprintln!(
            "[ENGINE] Started playback_id={} path={} start_ms={} duration_ms={:?}",
            playback_id,
            path.display(),
            start_ms,
            duration_ms
        );

        let _ = self.event_tx.send(PlayerEvent::Started {
            playback_id,
            path,
            duration_ms,
            start_ms,
        });

        Ok(())
    }

    fn alloc_playback_id(&mut self) -> u64 {
        let id = self.next_playback_id;
        self.next_playback_id = self.next_playback_id.saturating_add(1);
        id
    }

    fn stop_internal(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }

        self.current_path = None;
        self.current_duration_ms = None;
        self.current_start_ms = 0;
        self.current_track_sink_origin_ms = 0;
        self.current_track_playback_len_ms = None;
        self.current_playback_id = None;
        self.queued_tracks.clear();
        self.ended_emitted = false;
    }
}
