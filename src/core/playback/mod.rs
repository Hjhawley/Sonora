//! core/playback/mod.rs
//! Sonora playback core module.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

mod engine;

pub use engine::PlaybackEngine;

#[derive(Clone)]
pub struct PlaybackController {
    command_tx: Sender<PlayerCommand>,
}

impl PlaybackController {
    /// Best-effort send. If the engine died, the command is dropped.
    pub fn send(&self, cmd: PlayerCommand) {
        let _ = self.command_tx.send(cmd);
    }
}

#[derive(Debug)]
pub enum PlayerCommand {
    PlayFile(PathBuf),
    Pause,
    Resume,
    Stop,
    Seek(u64),      // ms
    SetVolume(f32), // 0.0..=1.0
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Started {
        path: PathBuf,
        duration_ms: Option<u64>,
    },
    Paused,
    Resumed,
    Stopped,
    Position {
        position_ms: u64,
    },
    TrackEnded,
    Error(String),
}

/// Spawns playback thread and returns:
/// - PlaybackController (store in GUI state)
/// - Receiver<PlayerEvent> (polled by GUI on a timer tick)
pub fn start_playback() -> (PlaybackController, Receiver<PlayerEvent>) {
    let (command_tx, command_rx) = mpsc::channel::<PlayerCommand>();
    let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();

    thread::spawn(move || {
        // Keep a clone for init-failure reporting
        let event_tx_fail = event_tx.clone();

        match PlaybackEngine::new(event_tx) {
            Ok(mut engine) => engine.run(command_rx),
            Err(e) => {
                let _ = event_tx_fail.send(PlayerEvent::Error(e));
            }
        }
    });

    (PlaybackController { command_tx }, event_rx)
}
