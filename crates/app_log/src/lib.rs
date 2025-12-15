//! LightningFiler Logging & Observability Module
//!
//! Provides structured logging, panic handling, crash reports, and deadlock detection.

mod panic_hook;
mod logging;

pub use panic_hook::init_panic_hook;
pub use logging::{init_logging, cleanup_old_logs};

use std::path::PathBuf;
use directories::ProjectDirs;

/// Get the application log directory
pub fn log_dir() -> PathBuf {
    ProjectDirs::from("com", "LightningFiler", "LightningFiler")
        .map(|dirs| dirs.data_dir().join("logs"))
        .unwrap_or_else(|| PathBuf::from("./logs"))
}

/// Initialize all observability features
pub fn init() -> anyhow::Result<()> {
    init_logging()?;
    init_panic_hook();

    #[cfg(debug_assertions)]
    init_deadlock_detector();

    Ok(())
}

#[cfg(debug_assertions)]
fn init_deadlock_detector() {
    use std::thread;
    use std::time::Duration;

    thread::spawn(|| {
        loop {
            thread::sleep(Duration::from_secs(10));
            let deadlocks = parking_lot::deadlock::check_deadlock();
            if !deadlocks.is_empty() {
                tracing::error!("Deadlock detected!");
                for (i, threads) in deadlocks.iter().enumerate() {
                    tracing::error!("Deadlock #{}", i);
                    for t in threads {
                        tracing::error!("Thread Id {:#?}", t.thread_id());
                        tracing::error!("{:#?}", t.backtrace());
                    }
                }
            }
        }
    });
}
