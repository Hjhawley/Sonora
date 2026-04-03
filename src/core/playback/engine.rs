//! core/playback/engine.rs
//! Playback transport and audio streaming driver.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};

use super::decoder::open_source_at_ms;
use super::{PlayerCommand, PlayerEvent};

const TICK_MS: u64 = 200;

#[derive(Debug, Clone)]
struct TrackStart {
    playback_id: u64,
    path: PathBuf,
    duration_ms: Option<u64>,
    start_ms: u64,
}

#[derive(Debug, Clone)]
struct ActiveTrack {
    playback_id: u64,
    path: PathBuf,
    start_ms: u64,
    sink_origin_ms: u64,
}

struct QueuedTrackSource {
    source: Box<dyn Source<Item = f32> + Send>,
    start: TrackStart,
}

#[derive(Default)]
struct PlaybackQueueState {
    current: Option<QueuedTrackSource>,
    queued: VecDeque<QueuedTrackSource>,
}

/// Single source appended to rodio.
/// Internally walks current source -> queued source -> queued source...
/// and emits an exact track-start notification when it advances.
struct EngineQueueSource {
    shared: Arc<Mutex<PlaybackQueueState>>,
    start_tx: Sender<TrackStart>,
}

impl EngineQueueSource {
    fn new(shared: Arc<Mutex<PlaybackQueueState>>, start_tx: Sender<TrackStart>) -> Self {
        Self { shared, start_tx }
    }
}

impl Iterator for EngineQueueSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let pending_start = {
                let mut shared = self.shared.lock().ok()?;
                let current = shared.current.as_mut()?;

                if let Some(sample) = current.source.next() {
                    return Some(sample);
                }

                match shared.queued.pop_front() {
                    Some(next_src) => {
                        let start = next_src.start.clone();
                        shared.current = Some(next_src);
                        Some(start)
                    }
                    None => {
                        shared.current = None;
                        return None;
                    }
                }
            };

            if let Some(start) = pending_start {
                let _ = self.start_tx.send(start);
            }
        }
    }
}

impl Source for EngineQueueSource {
    fn current_span_len(&self) -> Option<usize> {
        let Ok(shared) = self.shared.lock() else {
            return None;
        };

        shared
            .current
            .as_ref()
            .and_then(|q| q.source.current_span_len())
    }

    fn channels(&self) -> u16 {
        let Ok(shared) = self.shared.lock() else {
            return 2;
        };

        shared
            .current
            .as_ref()
            .map(|q| q.source.channels())
            .unwrap_or(2)
    }

    fn sample_rate(&self) -> u32 {
        let Ok(shared) = self.shared.lock() else {
            return 44_100;
        };

        shared
            .current
            .as_ref()
            .map(|q| q.source.sample_rate())
            .unwrap_or(44_100)
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub struct PlaybackEngine {
    /// Must be kept alive for rodio output to remain active.
    stream: OutputStream,

    /// Active sink for the current playback instance.
    sink: Option<Sink>,

    /// Shared queue consumed by EngineQueueSource.
    shared_queue: Option<Arc<Mutex<PlaybackQueueState>>>,

    /// Exact track-start notifications emitted when the queue source advances.
    track_start_rx: Receiver<TrackStart>,
    track_start_tx: Sender<TrackStart>,

    /// Current logical playback state.
    active: Option<ActiveTrack>,

    /// Monotonic id for each playback instance.
    /// A seek or replay creates a new playback_id even for the same file.
    next_playback_id: u64,

    /// Cached so newly created sinks inherit the latest volume.
    volume: f32,

    /// Prevent duplicate TrackEnded events for the same queue exhaustion.
    ended_emitted: bool,

    event_tx: Sender<PlayerEvent>,
}

impl PlaybackEngine {
    pub fn new(event_tx: Sender<PlayerEvent>) -> Result<Self, String> {
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| format!("Audio init failed: {e}"))?;

        let (track_start_tx, track_start_rx) = mpsc::channel::<TrackStart>();

        Ok(Self {
            stream,
            sink: None,
            shared_queue: None,
            track_start_rx,
            track_start_tx,
            active: None,
            next_playback_id: 1,
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
                    self.handle_command(cmd);

                    while let Ok(cmd) = command_rx.try_recv() {
                        self.handle_command(cmd);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            self.tick();
        }

        self.stop_and_clear_state();
    }

    fn handle_command(&mut self, cmd: PlayerCommand) {
        match cmd {
            PlayerCommand::PlayFile(path) => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] PlayFile {}", path.display());

                if let Err(e) = self.start_file_at(path, 0, true, VecDeque::new()) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                }
            }

            PlayerCommand::QueueFile(path) => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] QueueFile {}", path.display());

                if let Err(e) = self.enqueue_file(path) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                }
            }

            PlayerCommand::ClearQueue => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] ClearQueue");
                self.clear_upcoming_queue();
            }

            PlayerCommand::Pause => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Pause");

                if let (Some(sink), Some(active)) = (&self.sink, &self.active) {
                    sink.pause();
                    let _ = self.event_tx.send(PlayerEvent::Paused {
                        playback_id: active.playback_id,
                    });
                }
            }

            PlayerCommand::Resume => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Resume");

                if let (Some(sink), Some(active)) = (&self.sink, &self.active) {
                    sink.play();
                    let _ = self.event_tx.send(PlayerEvent::Resumed {
                        playback_id: active.playback_id,
                    });
                }
            }

            PlayerCommand::Stop => {
                #[cfg(debug_assertions)]
                eprintln!("[ENGINE] Stop");

                let stopped_id = self.active.as_ref().map(|a| a.playback_id);
                self.stop_and_clear_state();

                if let Some(playback_id) = stopped_id {
                    let _ = self.event_tx.send(PlayerEvent::Stopped { playback_id });
                }
            }

            PlayerCommand::Seek(ms) => {
                #[cfg(debug_assertions)]
                eprintln!(
                    "[ENGINE] Seek(ms={}) current_path={:?}",
                    ms,
                    self.active.as_ref().map(|a| a.path.display().to_string())
                );

                let Some(active) = self.active.clone() else {
                    return;
                };

                let resume_playing = self.sink.as_ref().map(|s| !s.is_paused()).unwrap_or(true);
                let upcoming = self.take_upcoming_queue();

                if let Err(e) = self.start_file_at(active.path, ms, resume_playing, upcoming) {
                    let _ = self.event_tx.send(PlayerEvent::Error(e));
                } else if let Some(active) = &self.active {
                    let _ = self.event_tx.send(PlayerEvent::Position {
                        playback_id: active.playback_id,
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
        }
    }

    fn tick(&mut self) {
        let Some(sink) = &self.sink else {
            return;
        };

        let sink_pos_ms = sink.get_pos().as_millis() as u64;
        let sink_empty = sink.empty();

        self.process_track_starts(sink_pos_ms);
        self.emit_position(sink_pos_ms);
        self.handle_queue_exhaustion(sink_empty);
    }

    fn process_track_starts(&mut self, sink_pos_ms: u64) {
        while let Ok(start) = self.track_start_rx.try_recv() {
            self.active = Some(ActiveTrack {
                playback_id: start.playback_id,
                path: start.path.clone(),
                start_ms: start.start_ms,
                sink_origin_ms: sink_pos_ms,
            });

            self.ended_emitted = false;

            #[cfg(debug_assertions)]
            eprintln!(
                "[ENGINE] Exact boundary playback_id={} path={} duration_ms={:?}",
                start.playback_id,
                start.path.display(),
                start.duration_ms
            );

            let _ = self.event_tx.send(PlayerEvent::Started {
                playback_id: start.playback_id,
                path: start.path,
                duration_ms: start.duration_ms,
                start_ms: start.start_ms,
            });
        }
    }

    fn emit_position(&mut self, sink_pos_ms: u64) {
        let Some(active) = &self.active else {
            return;
        };

        let rel_ms = sink_pos_ms.saturating_sub(active.sink_origin_ms);
        let position_ms = active.start_ms + rel_ms;

        let _ = self.event_tx.send(PlayerEvent::Position {
            playback_id: active.playback_id,
            position_ms,
        });
    }

    fn handle_queue_exhaustion(&mut self, sink_empty: bool) {
        let Some(active) = &self.active else {
            return;
        };

        if sink_empty && !self.ended_emitted {
            self.ended_emitted = true;

            let playback_id = active.playback_id;
            let _ = self.event_tx.send(PlayerEvent::TrackEnded { playback_id });

            self.stop_and_clear_state();
        }
    }

    fn enqueue_file(&mut self, path: PathBuf) -> Result<(), String> {
        if self.sink.is_none() || self.shared_queue.is_none() || self.active.is_none() {
            return self.start_file_at(path, 0, true, VecDeque::new());
        }

        let (src, duration_ms) = open_source_at_ms(&path, 0)?;
        let playback_id = self.alloc_playback_id();

        let queued = QueuedTrackSource {
            source: Box::new(src),
            start: TrackStart {
                playback_id,
                path,
                duration_ms,
                start_ms: 0,
            },
        };

        if let Some(shared) = &self.shared_queue {
            if let Ok(mut shared) = shared.lock() {
                shared.queued.push_back(queued);
            }
        }

        Ok(())
    }

    fn start_file_at(
        &mut self,
        path: PathBuf,
        start_ms: u64,
        resume_playing: bool,
        upcoming: VecDeque<QueuedTrackSource>,
    ) -> Result<(), String> {
        self.stop_and_clear_state();

        let (src, duration_ms) = open_source_at_ms(&path, start_ms)?;
        let playback_id = self.alloc_playback_id();

        let shared = Arc::new(Mutex::new(PlaybackQueueState {
            current: Some(QueuedTrackSource {
                source: Box::new(src),
                start: TrackStart {
                    playback_id,
                    path: path.clone(),
                    duration_ms,
                    start_ms,
                },
            }),
            queued: upcoming,
        }));

        let sink = Sink::connect_new(self.stream.mixer());
        sink.set_volume(self.volume);
        sink.append(EngineQueueSource::new(
            shared.clone(),
            self.track_start_tx.clone(),
        ));

        if resume_playing {
            sink.play();
        } else {
            sink.pause();
        }

        self.sink = Some(sink);
        self.shared_queue = Some(shared);
        self.active = Some(ActiveTrack {
            playback_id,
            path: path.clone(),
            start_ms,
            sink_origin_ms: 0,
        });
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

    fn take_upcoming_queue(&mut self) -> VecDeque<QueuedTrackSource> {
        if let Some(shared) = &self.shared_queue {
            if let Ok(mut shared) = shared.lock() {
                return std::mem::take(&mut shared.queued);
            }
        }

        VecDeque::new()
    }

    fn alloc_playback_id(&mut self) -> u64 {
        let id = self.next_playback_id;
        self.next_playback_id = self.next_playback_id.saturating_add(1);
        id
    }

    fn clear_upcoming_queue(&mut self) {
        if let Some(shared) = &self.shared_queue {
            if let Ok(mut shared) = shared.lock() {
                shared.queued.clear();
            }
        }
    }

    fn stop_and_clear_state(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }

        self.shared_queue = None;
        self.active = None;
        self.ended_emitted = false;

        while self.track_start_rx.try_recv().is_ok() {}
    }
}
