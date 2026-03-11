//! core/playback/mod.rs

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

mod decoder;
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
    /// Clear current sink/queue and begin playback immediately.
    PlayFile(PathBuf),

    /// Append a file to the current logical queue.
    QueueFile(PathBuf),

    /// Clear queued upcoming files, but do not stop the currently playing file.
    ClearQueue,

    /// Compatibility aliases for older GUI code.
    SetNextFile(PathBuf),
    ClearNextFile,

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
        playback_id: u64,
        path: PathBuf,
        duration_ms: Option<u64>,
        /// Position the current track started from (0 normally, nonzero when seeking).
        start_ms: u64,
    },
    Paused {
        playback_id: u64,
    },
    Resumed {
        playback_id: u64,
    },
    Stopped {
        playback_id: u64,
    },
    Position {
        playback_id: u64,
        position_ms: u64,
    },
    /// Emitted only when the entire queue is exhausted.
    TrackEnded {
        playback_id: u64,
    },
    Error(String),
}

/// Spawns playback thread and returns:
/// - PlaybackController (store in GUI state)
/// - Receiver<PlayerEvent> (polled by GUI on a timer tick)
pub fn start_playback() -> (PlaybackController, Receiver<PlayerEvent>) {
    let (command_tx, command_rx) = mpsc::channel::<PlayerCommand>();
    let (event_tx, event_rx) = mpsc::channel::<PlayerEvent>();

    thread::spawn(move || {
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