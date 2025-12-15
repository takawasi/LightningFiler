//! LightningFiler File System Abstraction Layer
//!
//! Provides a unified interface for file system operations, including:
//! - UniversalPath: Safe path handling with UNC prefix support
//! - VFS: Virtual File System for archives
//! - Encoding detection and conversion
//! - File watching
//! - Directory browsing

mod universal_path;
mod encoding;
mod vfs;
mod watcher;
mod sanitize;
mod browser;

pub use universal_path::UniversalPath;
pub use encoding::{detect_encoding, decode_bytes, EncodingHint};
pub use vfs::{VirtualFileSystem, VfsEntry, VfsError};
pub use watcher::{FileWatcher, WatchEvent};
pub use sanitize::{sanitize_filename, SanitizeMode};
pub use browser::{FileEntry, ListOptions, SortBy, SortOrder, list_directory, list_drives, get_parent, is_root, get_siblings, get_next_sibling, get_prev_sibling, count_files};

use thiserror::Error;

/// File system errors
#[derive(Error, Debug)]
pub enum FsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path not found: {0}")]
    NotFound(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Archive error: {0}")]
    Archive(String),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Path too long: {0}")]
    PathTooLong(String),
}

pub type Result<T> = std::result::Result<T, FsError>;
