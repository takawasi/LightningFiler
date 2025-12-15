//! LightningFiler Database Layer
//!
//! Provides:
//! - SQLite for metadata storage (files, tags, history)
//! - RocksDB for KVS cache (thumbnails, hashes)

mod sqlite;
mod rocksdb_cache;
mod schema;
mod pool;

pub use sqlite::{MetadataDb, FileRecord, TagRecord, FileTagRecord};
pub use rocksdb_cache::{ThumbnailCache, CacheKey};
pub use pool::DbPool;
pub use schema::migrate;

use std::path::PathBuf;
use directories::ProjectDirs;
use thiserror::Error;

/// Database errors
#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("RocksDB error: {0}")]
    RocksDb(#[from] rocksdb::Error),

    #[error("Pool error: {0}")]
    Pool(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Record not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, DbError>;

/// Get the database directory
pub fn db_dir() -> PathBuf {
    ProjectDirs::from("com", "LightningFiler", "LightningFiler")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("./data"))
}

/// Initialize all databases
pub fn init() -> Result<(DbPool, ThumbnailCache)> {
    let db_path = db_dir();
    std::fs::create_dir_all(&db_path)?;

    let sqlite_path = db_path.join("metadata.db");
    let rocksdb_path = db_path.join("cache");

    let pool = pool::init_pool(&sqlite_path)?;
    migrate(&pool)?;

    let cache = ThumbnailCache::open(&rocksdb_path)?;

    tracing::info!("Database initialized at {:?}", db_path);
    Ok((pool, cache))
}
