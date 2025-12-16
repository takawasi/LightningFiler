//! File system watcher with notify-debouncer-mini

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// File system event types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Removed(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// File system watcher with debouncing
pub struct FileWatcher {
    debouncer: Debouncer<RecommendedWatcher>,
    event_rx: Receiver<Result<Vec<DebouncedEvent>, notify::Error>>,
    watched_paths: Vec<PathBuf>,
}

impl FileWatcher {
    /// Create a new file watcher with 100ms debounce
    pub fn new() -> Result<Self, notify::Error> {
        let (tx, rx) = channel();

        let debouncer = new_debouncer(
            Duration::from_millis(100),  // 100ms debounce
            tx,
        )?;

        Ok(Self {
            debouncer,
            event_rx: rx,
            watched_paths: Vec::new(),
        })
    }

    /// Watch a path for changes (non-recursive)
    pub fn watch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self.debouncer.watcher().watch(path, RecursiveMode::NonRecursive)?;
        self.watched_paths.push(path.to_path_buf());
        tracing::info!("Watching: {}", path.display());
        Ok(())
    }

    /// Stop watching a path
    pub fn unwatch(&mut self, path: &Path) -> Result<(), notify::Error> {
        self.debouncer.watcher().unwatch(path)?;
        self.watched_paths.retain(|p| p != path);
        tracing::info!("Unwatched: {}", path.display());
        Ok(())
    }

    /// Poll for file system events (non-blocking)
    pub fn poll_events(&self) -> Vec<FsEvent> {
        let mut events = Vec::new();

        while let Ok(result) = self.event_rx.try_recv() {
            match result {
                Ok(debounced_events) => {
                    for event in debounced_events {
                        if let Some(fs_event) = Self::convert_event(event) {
                            events.push(fs_event);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Watcher error: {:?}", e);
                }
            }
        }

        // Deduplication: remove consecutive Modified events for the same path
        events.dedup_by(|a, b| {
            match (a, b) {
                (FsEvent::Modified(p1), FsEvent::Modified(p2)) => p1 == p2,
                _ => false,
            }
        });

        events
    }

    /// Convert debounced event to FsEvent
    fn convert_event(event: DebouncedEvent) -> Option<FsEvent> {
        use notify_debouncer_mini::DebouncedEventKind;

        match event.kind {
            DebouncedEventKind::Any => {
                // Check if path exists to determine event type
                if event.path.exists() {
                    // Try to detect if it's newly created (within 1 second)
                    if event.path.metadata()
                        .and_then(|m| m.created())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .map(|elapsed| elapsed < Duration::from_secs(1))
                        .unwrap_or(false)
                    {
                        Some(FsEvent::Created(event.path))
                    } else {
                        Some(FsEvent::Modified(event.path))
                    }
                } else {
                    Some(FsEvent::Removed(event.path))
                }
            }
            DebouncedEventKind::AnyContinuous => None,
            _ => None,
        }
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        for path in &self.watched_paths {
            let _ = self.debouncer.watcher().unwatch(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_creation() {
        let watcher = FileWatcher::new();
        assert!(watcher.is_ok());
    }
}
