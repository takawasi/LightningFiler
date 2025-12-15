//! Application error types

use thiserror::Error;

/// Main application error type
#[derive(Error, Debug)]
pub enum AppError {
    // ===== Recoverable Errors (notify user, continue) =====
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Image decode error: {0}")]
    ImageDecode(String),

    #[error("Archive error: {0}")]
    Archive(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    // ===== Recoverable (internal recovery attempt) =====
    #[error("GPU device lost")]
    GpuLost,

    #[error("Bridge process error: {0}")]
    Bridge(String),

    // ===== Fatal Errors (application termination) =====
    #[error("Database corruption: {0}")]
    DbCorruption(String),

    #[error("System resource exhaustion: {0}")]
    SystemResource(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Initialization failed: {0}")]
    Init(String),
}

impl AppError {
    /// Is this error recoverable?
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AppError::Io(_)
                | AppError::FileNotFound(_)
                | AppError::AccessDenied(_)
                | AppError::UnsupportedFormat(_)
                | AppError::ImageDecode(_)
                | AppError::Archive(_)
                | AppError::Plugin(_)
                | AppError::Encoding(_)
                | AppError::GpuLost
                | AppError::Bridge(_)
        )
    }

    /// Is this a fatal error?
    pub fn is_fatal(&self) -> bool {
        !self.is_recoverable()
    }

    /// Get a user-friendly message
    pub fn user_message(&self) -> String {
        match self {
            AppError::FileNotFound(path) => format!("File not found: {}", path),
            AppError::AccessDenied(path) => format!("Access denied: {}", path),
            AppError::UnsupportedFormat(ext) => format!("Unsupported format: {}", ext),
            AppError::ImageDecode(msg) => format!("Cannot load image: {}", msg),
            AppError::Archive(msg) => format!("Archive error: {}", msg),
            AppError::GpuLost => "Display device reset. Reloading...".to_string(),
            _ => self.to_string(),
        }
    }
}

impl From<app_fs::FsError> for AppError {
    fn from(e: app_fs::FsError) -> Self {
        match e {
            app_fs::FsError::NotFound(p) => AppError::FileNotFound(p),
            app_fs::FsError::AccessDenied(p) => AppError::AccessDenied(p),
            app_fs::FsError::Archive(msg) => AppError::Archive(msg),
            app_fs::FsError::Encoding(msg) => AppError::Encoding(msg),
            _ => AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
        }
    }
}

impl From<app_db::DbError> for AppError {
    fn from(e: app_db::DbError) -> Self {
        match e {
            app_db::DbError::NotFound(msg) => AppError::FileNotFound(msg),
            _ => AppError::DbCorruption(e.to_string()),
        }
    }
}

impl From<image::ImageError> for AppError {
    fn from(e: image::ImageError) -> Self {
        AppError::ImageDecode(e.to_string())
    }
}
