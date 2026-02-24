//! core/playback/engine.rs

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use super::{PlayerCommand, PlayerEvent};

const TICK_MS: u64 = 200;

pub struct PlaybackEngine {
    // Keep alive for lifetime of engine
    stream: OutputStream,

    sink: Option<Sink>,
    current_path: Option<PathBuf>,
    current_duration_ms: Option<u64>,

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
            PlayerCommand::Seek(_ms) => {
                // rodio 0.21.1 Sink::try_seek exists in some setups but not all decoders;
                // to keep MVP stable, treat seek as unsupported for now.
                let _ = self
                    .event_tx
                    .send(PlayerEvent::Error("Seeking not supported yet.".into()));
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
        let Some(sink) = &self.sink else {
            return;
        };

        let position_ms = sink.get_pos().as_millis() as u64;
        let _ = self.event_tx.send(PlayerEvent::Position { position_ms });

        if sink.empty() && self.current_path.is_some() && !self.ended_emitted {
            self.ended_emitted = true;
            let _ = self.event_tx.send(PlayerEvent::TrackEnded);
            self.stop_internal();
        }
    }

    fn play_file(&mut self, path: PathBuf) -> Result<(), String> {
        self.stop_internal();

        // rodio 0.21.x: create sink from the stream's mixer
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
        self.ended_emitted = false;

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
        self.ended_emitted = false;
    }
}
