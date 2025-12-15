//! Structured logging setup with tracing

use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the logging system
pub fn init_logging() -> anyhow::Result<()> {
    let log_dir = super::log_dir();
    std::fs::create_dir_all(&log_dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Keep the guard alive for the lifetime of the application
    // In production, this should be stored in AppState
    std::mem::forget(_guard);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    #[cfg(debug_assertions)]
    {
        // Development: pretty console output + file
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().pretty())
            .with(fmt::layer().json().with_writer(non_blocking))
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        // Release: JSON file only
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().json().with_writer(non_blocking))
            .init();
    }

    tracing::info!("Logging initialized");
    Ok(())
}

/// Clean up log files older than specified days
pub fn cleanup_old_logs(days: u32) -> anyhow::Result<usize> {
    use std::time::{Duration, SystemTime};

    let log_dir = super::log_dir();
    if !log_dir.exists() {
        return Ok(0);
    }

    let threshold = SystemTime::now() - Duration::from_secs(days as u64 * 24 * 60 * 60);
    let mut deleted = 0;

    for entry in std::fs::read_dir(&log_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "log") {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < threshold {
                        if std::fs::remove_file(&path).is_ok() {
                            deleted += 1;
                            tracing::debug!("Deleted old log: {:?}", path);
                        }
                    }
                }
            }
        }
    }

    tracing::info!("Cleaned up {} old log files", deleted);
    Ok(deleted)
}
