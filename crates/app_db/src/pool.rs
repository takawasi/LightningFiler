//! SQLite connection pool

use crate::{DbError, Result};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;

/// Initialize the SQLite connection pool
pub fn init_pool(path: &Path) -> Result<DbPool> {
    let manager = SqliteConnectionManager::file(path).with_init(|conn| {
        // Performance tuning
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA cache_size = -64000;  -- 64MB
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            PRAGMA temp_store = MEMORY;
        ",
        )?;
        Ok(())
    });

    Pool::builder()
        .max_size(10)
        .min_idle(Some(2))
        .build(manager)
        .map_err(|e| DbError::Pool(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_pool_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let pool = init_pool(temp_file.path());
        assert!(pool.is_ok());
    }
}
