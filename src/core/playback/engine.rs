//! core/playback/engine.rs
//! playback driver

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
struct TrackBoundary {
    playback_id: u64,
    path: PathBuf,
    duration_ms: Option<u64>,
    start_ms: u64,
}

struct QueuedSource {
    source: Box<dyn Source<Item = f32> + Send>,
    boundary: TrackBoundary,
}

#[derive(Default)]
struct SharedQueue {
    current: Option<QueuedSource>,
    queued: VecDeque<QueuedSource>,
}

/// Single source appended to rodio sink. It internally walks:
/// current source -> queued source -> queued source...
/// and emits an exact boundary notification when it switches tracks.
struct EngineQueueSource {
    shared: Arc<Mutex<SharedQueue>>,
    boundary_tx: Sender<TrackBoundary>,
}

impl EngineQueueSource {
    fn new(shared: Arc<Mutex<SharedQueue>>, boundary_tx: Sender<TrackBoundary>) -> Self {
        Self {
            shared,
            boundary_tx,
        }
    }
}

impl Iterator for EngineQueueSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut pending_boundary: Option<TrackBoundary> = None;

            {
                let mut shared = self.shared.lock().ok()?;
                let current = shared.current.as_mut()?;

                if let Some(sample) = current.source.next() {
                    return Some(sample);
                }

                let next = shared.queued.pop_front();
                match next {
                    Some(next_src) => {
                        pending_boundary = Some(next_src.boundary.clone());
                        shared.current = Some(next_src);
                    }
                    None => {
                        shared.current = None;
                        return None;
                    }
                }
            }

            if let Some(boundary) = pending_boundary {
                let _ = self.boundary_tx.send(boundary);
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
    // Keep alive for lifetime of engine.
    stream: OutputStream,

    // One persistent sink per playback session.
    sink: Option<Sink>,

    // Shared logical queue consumed by EngineQueueSource.
    shared_queue: Option<Arc<Mutex<SharedQueue>>>,

    // Exact boundary notifications from EngineQueueSource.
    boundary_rx: Receiver<TrackBoundary>,
    boundary_tx: Sender<TrackBoundary>,

    // Current track metadata.
    current_path: Option<PathBuf>,
    current_duration_ms: Option<u64>,
    current_start_ms: u64,

    // Absolute sink position (ms) when the current track began.
    current_track_sink_origin_ms: u64,

    // Monotonic id for each logical track transition.
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

        let (boundary_tx, boundary_rx) = mpsc::channel::<TrackBoundary>();

        Ok(Self {
            stream,
            sink: None,
            shared_queue: None,
            boundary_rx,
            boundary_tx,
            current_path: None,
            current_duration_ms: None,
            current_start_ms: 0,
            current_track_sink_origin_ms: 0,
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
                self.clear_queue_only();
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
        let Some(sink) = &self.sink else {
            return;
        };

        let sink_pos_ms = sink.get_pos().as_millis() as u64;
        let sink_empty = sink.empty();

        // Drain exact source-boundary notifications first.
        while let Ok(boundary) = self.boundary_rx.try_recv() {
            self.current_track_sink_origin_ms = sink_pos_ms;
            self.current_path = Some(boundary.path.clone());
            self.current_duration_ms = boundary.duration_ms;
            self.current_start_ms = boundary.start_ms;
            self.current_playback_id = Some(boundary.playback_id);
            self.ended_emitted = false;

            #[cfg(debug_assertions)]
            eprintln!(
                "[ENGINE] Exact boundary playback_id={} path={} duration_ms={:?}",
                boundary.playback_id,
                boundary.path.display(),
                boundary.duration_ms
            );

            let _ = self.event_tx.send(PlayerEvent::Started {
                playback_id: boundary.playback_id,
                path: boundary.path,
                duration_ms: boundary.duration_ms,
                start_ms: boundary.start_ms,
            });
        }

        let Some(playback_id) = self.current_playback_id else {
            return;
        };

        let rel_ms = sink_pos_ms.saturating_sub(self.current_track_sink_origin_ms);
        let position_ms = self.current_start_ms + rel_ms;

        let _ = self.event_tx.send(PlayerEvent::Position {
            playback_id,
            position_ms,
        });

        // Entire queue exhausted.
        if sink_empty && self.current_path.is_some() && !self.ended_emitted {
            self.ended_emitted = true;
            let _ = self.event_tx.send(PlayerEvent::TrackEnded { playback_id });
            self.stop_internal();
        }
    }

    fn queue_file(&mut self, path: PathBuf) -> Result<(), String> {
        if self.sink.is_none() || self.shared_queue.is_none() || self.current_path.is_none() {
            return self.play_file_at(path, 0, true);
        }

        let (src, duration_ms) = open_source_at_ms(&path, 0)?;
        let playback_id = self.alloc_playback_id();

        let queued = QueuedSource {
            source: Box::new(src),
            boundary: TrackBoundary {
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

    fn play_file_at(
        &mut self,
        path: PathBuf,
        start_ms: u64,
        resume_playing: bool,
    ) -> Result<(), String> {
        self.stop_internal();

        let (src, duration_ms) = open_source_at_ms(&path, start_ms)?;
        let playback_id = self.alloc_playback_id();

        let shared = Arc::new(Mutex::new(SharedQueue {
            current: Some(QueuedSource {
                source: Box::new(src),
                boundary: TrackBoundary {
                    playback_id,
                    path: path.clone(),
                    duration_ms,
                    start_ms,
                },
            }),
            queued: VecDeque::new(),
        }));

        let sink = Sink::connect_new(self.stream.mixer());
        sink.set_volume(self.volume);
        sink.append(EngineQueueSource::new(
            shared.clone(),
            self.boundary_tx.clone(),
        ));

        if resume_playing {
            sink.play();
        } else {
            sink.pause();
        }

        self.sink = Some(sink);
        self.shared_queue = Some(shared);

        self.current_path = Some(path.clone());
        self.current_duration_ms = duration_ms;
        self.current_start_ms = start_ms;
        self.current_track_sink_origin_ms = 0;
        self.current_playback_id = Some(playback_id);
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

    fn clear_queue_only(&mut self) {
        if let Some(shared) = &self.shared_queue {
            if let Ok(mut shared) = shared.lock() {
                shared.queued.clear();
            }
        }
    }

    fn stop_internal(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }

        self.shared_queue = None;
        self.current_path = None;
        self.current_duration_ms = None;
        self.current_start_ms = 0;
        self.current_track_sink_origin_ms = 0;
        self.current_playback_id = None;
        self.ended_emitted = false;

        while self.boundary_rx.try_recv().is_ok() {}
    }
}
