//! Database schema and migrations

use crate::{DbPool, Result, DbError};

const SCHEMA_VERSION: i32 = 1;

/// Run database migrations
pub fn migrate(pool: &DbPool) -> Result<()> {
    let conn = pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

    // Check current version
    let current_version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);

    if current_version < SCHEMA_VERSION {
        tracing::info!(
            "Migrating database from version {} to {}",
            current_version,
            SCHEMA_VERSION
        );

        // Apply migrations
        if current_version < 1 {
            apply_v1(&conn)?;
        }

        // Update version
        conn.execute(&format!("PRAGMA user_version = {}", SCHEMA_VERSION), [])?;
    }

    Ok(())
}

fn apply_v1(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Files table: mirrors file system state
        CREATE TABLE IF NOT EXISTS files (
            file_id INTEGER PRIMARY KEY AUTOINCREMENT,

            -- Fast lookup (xxh3 hash of display path)
            path_hash INTEGER NOT NULL UNIQUE,

            -- UTF-8 display path (for search, may be lossy)
            path_display TEXT NOT NULL,

            -- Raw bytes for exact path reconstruction (Windows WCHAR as bytes)
            path_blob BLOB NOT NULL,

            -- Parent folder hash for hierarchy queries
            parent_hash INTEGER NOT NULL,

            -- File metadata
            file_name TEXT NOT NULL,
            extension TEXT,
            size INTEGER,
            modified_at INTEGER,
            created_at INTEGER,

            -- Cached metadata (JSON)
            metadata TEXT,

            -- Indexing timestamp
            indexed_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_files_parent ON files(parent_hash);
        CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);
        CREATE INDEX IF NOT EXISTS idx_files_modified ON files(modified_at);
        CREATE INDEX IF NOT EXISTS idx_files_name ON files(file_name);

        -- Tags table
        CREATE TABLE IF NOT EXISTS tags (
            tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            color INTEGER,  -- 0xAARRGGBB
            parent_tag_id INTEGER REFERENCES tags(tag_id) ON DELETE SET NULL,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
        CREATE INDEX IF NOT EXISTS idx_tags_parent ON tags(parent_tag_id);

        -- File-Tag mapping (many-to-many)
        CREATE TABLE IF NOT EXISTS file_tags (
            file_id INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
            tag_id INTEGER NOT NULL REFERENCES tags(tag_id) ON DELETE CASCADE,
            added_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
            PRIMARY KEY (file_id, tag_id)
        );

        CREATE INDEX IF NOT EXISTS idx_file_tags_file ON file_tags(file_id);
        CREATE INDEX IF NOT EXISTS idx_file_tags_tag ON file_tags(tag_id);

        -- Search history
        CREATE TABLE IF NOT EXISTS search_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            query TEXT NOT NULL,
            result_count INTEGER,
            searched_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_search_history_time ON search_history(searched_at);

        -- Folders table (for quick folder listing)
        CREATE TABLE IF NOT EXISTS folders (
            folder_id INTEGER PRIMARY KEY AUTOINCREMENT,
            path_hash INTEGER NOT NULL UNIQUE,
            path_display TEXT NOT NULL,
            path_blob BLOB NOT NULL,
            parent_hash INTEGER,
            file_count INTEGER DEFAULT 0,
            last_scan INTEGER,
            indexed_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_folders_parent ON folders(parent_hash);

        -- Bookmarks
        CREATE TABLE IF NOT EXISTS bookmarks (
            bookmark_id INTEGER PRIMARY KEY AUTOINCREMENT,
            path_hash INTEGER NOT NULL,
            path_display TEXT NOT NULL,
            name TEXT,
            sort_order INTEGER DEFAULT 0,
            created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );
        "#,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::init_pool;
    use tempfile::NamedTempFile;

    #[test]
    fn test_migration() {
        let temp_file = NamedTempFile::new().unwrap();
        let pool = init_pool(temp_file.path()).unwrap();
        let result = migrate(&pool);
        assert!(result.is_ok());
    }
}
