//! core/playback/engine.rs

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use rodio::{OutputStream, OutputStreamBuilder, Sink};

use super::decoder::open_source_at_ms;
use super::{PlayerCommand, PlayerEvent};

const TICK_MS: u64 = 200;

pub struct PlaybackEngine {
    // Keep alive for lifetime of engine
    stream: OutputStream,

    sink: Option<Sink>,
    current_path: Option<PathBuf>,
    current_duration_ms: Option<u64>,

    // UI position = base_position_ms + sink.get_pos()
    base_position_ms: u64,

    // Track current volume so seek/play can apply it to new sinks
    volume: f32,

    // Prevent duplicate TrackEnded events for the same track.
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
            base_position_ms: 0,
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
            PlayerCommand::Pause => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Pause");

                if let Some(sink) = &self.sink {
                    sink.pause();
                    let _ = self.event_tx.send(PlayerEvent::Paused);
                }
            }
            PlayerCommand::Resume => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Resume");

                if let Some(sink) = &self.sink {
                    sink.play();
                    let _ = self.event_tx.send(PlayerEvent::Resumed);
                }
            }
            PlayerCommand::Stop => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Stop");

                self.stop_internal();
                let _ = self.event_tx.send(PlayerEvent::Stopped);
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

                // Preserve paused/playing state across seek.
                let resume_playing = self.sink.as_ref().map(|s| !s.is_paused()).unwrap_or(true);

                if let Err(e) = self.play_file_at(path, ms, resume_playing) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                } else {
                    // Immediate UI feedback (optional; tick will also catch up).
                    let _ = self
                        .event_tx
                        .send(PlayerEvent::Position { position_ms: ms });
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
        let Some(sink) = &self.sink else {
            return;
        };

        let position_ms = self.base_position_ms + sink.get_pos().as_millis() as u64;
        let _ = self.event_tx.send(PlayerEvent::Position { position_ms });

        if sink.empty() && self.current_path.is_some() && !self.ended_emitted {
            self.ended_emitted = true;
            let _ = self.event_tx.send(PlayerEvent::TrackEnded);
            self.stop_internal();
        }
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

        self.current_duration_ms = duration_ms;
        self.current_path = Some(path.clone());
        self.sink = Some(sink);

        self.base_position_ms = start_ms;
        self.ended_emitted = false;

        #[cfg(debug_assertions)]
        eprintln!(
            "[ENGINE] Started path={} start_ms={} duration_ms={:?}",
            path.display(),
            start_ms,
            duration_ms
        );

        let _ = self.event_tx.send(PlayerEvent::Started {
            path,
            duration_ms,
            start_ms,
        });

        Ok(())
    }

    fn stop_internal(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.current_path = None;
        self.current_duration_ms = None;
        self.base_position_ms = 0;
        self.ended_emitted = false;
    }
}
