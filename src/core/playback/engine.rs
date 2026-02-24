//! core/playback/engine.rs
//! Playback engine (rodio owner).
//!
//! Owns:
//! - OutputStream (must stay alive)
//! - Sink (per current track)
//! - command loop + periodic position ticks
//!
//! Emits PlayerEvent back via a channel.
//! No Iced imports.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use super::{PlayerCommand, PlayerEvent};

const TICK_MS: u64 = 200;

pub struct PlaybackEngine {
    // Keep this alive for the lifetime of the engine!
    stream: OutputStream,

    // Current playback
    sink: Option<Sink>,
    current_path: Option<PathBuf>,
    current_duration_ms: Option<u64>,

    // Event channel
    event_tx: Sender<PlayerEvent>,
}

impl PlaybackEngine {
    pub fn new(event_tx: Sender<PlayerEvent>) -> Self {
        // rodio 0.21.x: build/open the default output stream via OutputStreamBuilder
        let stream = OutputStreamBuilder::open_default_stream()
            .expect("failed to init default audio output");

        Self {
            stream,
            sink: None,
            current_path: None,
            current_duration_ms: None,
            event_tx,
        }
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
                if let Err(e) = self.play_file(path) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                }
            }
            PlayerCommand::Pause => {
                if let Some(sink) = &self.sink {
                    sink.pause();
                    let _ = self.event_tx.send(PlayerEvent::Paused);
                }
            }
            PlayerCommand::Resume => {
                if let Some(sink) = &self.sink {
                    sink.play();
                    let _ = self.event_tx.send(PlayerEvent::Resumed);
                }
            }
            PlayerCommand::Stop => {
                self.stop_internal();
                let _ = self.event_tx.send(PlayerEvent::Stopped);
            }
            PlayerCommand::Seek(ms) => {
                if let Some(sink) = &self.sink {
                    if sink.try_seek(Duration::from_millis(ms)).is_err() {
                        let _ = self.event_tx.send(PlayerEvent::Error(
                            "Seek failed (decoder may not support it)".into(),
                        ));
                    }
                }
            }
            PlayerCommand::SetVolume(v) => {
                if let Some(sink) = &self.sink {
                    sink.set_volume(v.clamp(0.0, 1.0));
                }
            }
            PlayerCommand::Shutdown => return true,
        }

        false
    }

    fn tick(&mut self) {
        if let Some(sink) = &self.sink {
            let position_ms = sink.get_pos().as_millis() as u64;
            let _ = self.event_tx.send(PlayerEvent::Position { position_ms });

            if sink.empty() && self.current_path.is_some() {
                let _ = self.event_tx.send(PlayerEvent::TrackEnded);
                self.stop_internal();
            }
        }
    }

    fn play_file(&mut self, path: PathBuf) -> Result<(), String> {
        self.stop_internal();

        // rodio 0.21.x: Sink is created from the stream's mixer
        let sink = Sink::connect_new(self.stream.mixer());

        let file = File::open(&path).map_err(|e| format!("Failed to open file: {e}"))?;
        let reader = BufReader::new(file);

        let decoder = Decoder::new(reader).map_err(|e| format!("Decode failed: {e}"))?;
        let duration_ms = decoder.total_duration().map(|d| d.as_millis() as u64);

        sink.append(decoder);
        sink.play();

        self.current_duration_ms = duration_ms;
        self.current_path = Some(path.clone());
        self.sink = Some(sink);

        let _ = self
            .event_tx
            .send(PlayerEvent::Started { path, duration_ms });

        Ok(())
    }

    fn stop_internal(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.current_path = None;
        self.current_duration_ms = None;
    }
}
