//! File system watcher with debouncing

use crate::UniversalPath;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// File system event types
#[derive(Debug, Clone)]
pub enum WatchEvent {
    Created(UniversalPath),
    Modified(UniversalPath),
    Deleted(UniversalPath),
    Renamed { from: UniversalPath, to: UniversalPath },
}

/// File system watcher with event debouncing
pub struct FileWatcher {
    watcher: RecommendedWatcher,
    event_rx: mpsc::Receiver<WatchEvent>,
    watched_paths: Vec<UniversalPath>,
}

impl FileWatcher {
    /// Create a new file watcher with debounce interval
    pub fn new(debounce_ms: u64) -> notify::Result<Self> {
        let (raw_tx, raw_rx) = mpsc::channel::<notify::Result<Event>>();
        let (event_tx, event_rx) = mpsc::channel::<WatchEvent>();

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = raw_tx.send(res);
            },
            Config::default(),
        )?;

        // Start debounce thread
        let debounce_duration = Duration::from_millis(debounce_ms);
        std::thread::spawn(move || {
            Self::debounce_loop(raw_rx, event_tx, debounce_duration);
        });

        Ok(Self {
            watcher,
            event_rx,
            watched_paths: Vec::new(),
        })
    }

    /// Watch a directory for changes
    pub fn watch(&mut self, path: &UniversalPath, recursive: bool) -> notify::Result<()> {
        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        self.watcher.watch(path.as_path(), mode)?;
        self.watched_paths.push(path.clone());

        tracing::info!("Watching: {}", path);
        Ok(())
    }

    /// Stop watching a directory
    pub fn unwatch(&mut self, path: &UniversalPath) -> notify::Result<()> {
        self.watcher.unwatch(path.as_path())?;
        self.watched_paths.retain(|p| p.id() != path.id());

        tracing::info!("Stopped watching: {}", path);
        Ok(())
    }

    /// Try to receive the next event (non-blocking)
    pub fn try_recv(&self) -> Option<WatchEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive the next event (blocking)
    pub fn recv(&self) -> Option<WatchEvent> {
        self.event_rx.recv().ok()
    }

    /// Receive with timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Option<WatchEvent> {
        self.event_rx.recv_timeout(timeout).ok()
    }

    /// Debounce loop - consolidates rapid events into single notifications
    fn debounce_loop(
        raw_rx: mpsc::Receiver<notify::Result<Event>>,
        event_tx: mpsc::Sender<WatchEvent>,
        debounce_duration: Duration,
    ) {
        let mut pending: HashMap<PathBuf, (notify::EventKind, Instant)> = HashMap::new();

        loop {
            // Check for new raw events
            match raw_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(event)) => {
                    for path in event.paths {
                        pending.insert(path, (event.kind, Instant::now()));
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("Watch error: {:?}", e);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }

            // Process pending events that have settled
            let now = Instant::now();
            let settled: Vec<_> = pending
                .iter()
                .filter(|(_, (_, time))| now.duration_since(*time) >= debounce_duration)
                .map(|(path, (kind, _))| (path.clone(), *kind))
                .collect();

            for (path, kind) in settled {
                pending.remove(&path);

                let universal_path = UniversalPath::new(&path);

                let event = match kind {
                    notify::EventKind::Create(_) => WatchEvent::Created(universal_path),
                    notify::EventKind::Modify(_) => WatchEvent::Modified(universal_path),
                    notify::EventKind::Remove(_) => WatchEvent::Deleted(universal_path),
                    _ => continue,
                };

                if event_tx.send(event).is_err() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_creation() {
        let watcher = FileWatcher::new(100);
        assert!(watcher.is_ok());
    }
}
