//! SQLite metadata operations

use crate::{DbError, DbPool, Result};
use app_fs::UniversalPath;
use serde::{Deserialize, Serialize};

/// File record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub file_id: i64,
    pub path_hash: i64,
    pub path_display: String,
    pub path_blob: Vec<u8>,
    pub parent_hash: i64,
    pub file_name: String,
    pub extension: Option<String>,
    pub size: Option<i64>,
    pub modified_at: Option<i64>,
    pub created_at: Option<i64>,
    pub metadata: Option<String>,
    pub indexed_at: i64,
}

/// Tag record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRecord {
    pub tag_id: i64,
    pub name: String,
    pub color: Option<u32>,
    pub parent_tag_id: Option<i64>,
}

/// File-Tag mapping
#[derive(Debug, Clone)]
pub struct FileTagRecord {
    pub file_id: i64,
    pub tag_id: i64,
    pub added_at: i64,
}

/// Metadata database operations
pub struct MetadataDb {
    pool: DbPool,
}

impl MetadataDb {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ===== File Operations =====

    /// Insert or update a file record
    pub fn upsert_file(&self, path: &UniversalPath, size: Option<i64>, modified_at: Option<i64>) -> Result<i64> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let path_hash = path.id() as i64;
        let parent_hash = path.parent().map(|p| p.id() as i64).unwrap_or(0);
        let file_name = path.file_name().unwrap_or("").to_string();
        let extension = path.extension().map(|s| s.to_lowercase());

        conn.execute(
            r#"
            INSERT INTO files (path_hash, path_display, path_blob, parent_hash, file_name, extension, size, modified_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(path_hash) DO UPDATE SET
                path_display = excluded.path_display,
                size = excluded.size,
                modified_at = excluded.modified_at,
                indexed_at = strftime('%s', 'now')
            "#,
            rusqlite::params![
                path_hash,
                path.display(),
                path.as_raw_bytes(),
                parent_hash,
                file_name,
                extension,
                size,
                modified_at,
            ],
        )?;

        let file_id = conn.last_insert_rowid();
        Ok(file_id)
    }

    /// Get a file by path hash
    pub fn get_file_by_hash(&self, path_hash: u64) -> Result<Option<FileRecord>> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT file_id, path_hash, path_display, path_blob, parent_hash, file_name, extension, size, modified_at, created_at, metadata, indexed_at
             FROM files WHERE path_hash = ?1"
        )?;

        let result = stmt.query_row([path_hash as i64], |row| {
            Ok(FileRecord {
                file_id: row.get(0)?,
                path_hash: row.get(1)?,
                path_display: row.get(2)?,
                path_blob: row.get(3)?,
                parent_hash: row.get(4)?,
                file_name: row.get(5)?,
                extension: row.get(6)?,
                size: row.get(7)?,
                modified_at: row.get(8)?,
                created_at: row.get(9)?,
                metadata: row.get(10)?,
                indexed_at: row.get(11)?,
            })
        });

        match result {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List files in a folder
    pub fn list_files_in_folder(&self, parent_hash: u64, offset: usize, limit: usize) -> Result<Vec<FileRecord>> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT file_id, path_hash, path_display, path_blob, parent_hash, file_name, extension, size, modified_at, created_at, metadata, indexed_at
             FROM files WHERE parent_hash = ?1
             ORDER BY file_name COLLATE NOCASE
             LIMIT ?2 OFFSET ?3"
        )?;

        let rows = stmt.query_map([parent_hash as i64, limit as i64, offset as i64], |row| {
            Ok(FileRecord {
                file_id: row.get(0)?,
                path_hash: row.get(1)?,
                path_display: row.get(2)?,
                path_blob: row.get(3)?,
                parent_hash: row.get(4)?,
                file_name: row.get(5)?,
                extension: row.get(6)?,
                size: row.get(7)?,
                modified_at: row.get(8)?,
                created_at: row.get(9)?,
                metadata: row.get(10)?,
                indexed_at: row.get(11)?,
            })
        })?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }

        Ok(files)
    }

    /// Delete a file record
    pub fn delete_file(&self, path_hash: u64) -> Result<bool> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let rows = conn.execute("DELETE FROM files WHERE path_hash = ?1", [path_hash as i64])?;
        Ok(rows > 0)
    }

    /// Search files by name pattern
    pub fn search_files(&self, pattern: &str, limit: usize) -> Result<Vec<FileRecord>> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let search_pattern = format!("%{}%", pattern);

        let mut stmt = conn.prepare(
            "SELECT file_id, path_hash, path_display, path_blob, parent_hash, file_name, extension, size, modified_at, created_at, metadata, indexed_at
             FROM files WHERE file_name LIKE ?1 OR path_display LIKE ?1
             ORDER BY file_name COLLATE NOCASE
             LIMIT ?2"
        )?;

        let rows = stmt.query_map([&search_pattern, &limit.to_string()], |row| {
            Ok(FileRecord {
                file_id: row.get(0)?,
                path_hash: row.get(1)?,
                path_display: row.get(2)?,
                path_blob: row.get(3)?,
                parent_hash: row.get(4)?,
                file_name: row.get(5)?,
                extension: row.get(6)?,
                size: row.get(7)?,
                modified_at: row.get(8)?,
                created_at: row.get(9)?,
                metadata: row.get(10)?,
                indexed_at: row.get(11)?,
            })
        })?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }

        Ok(files)
    }

    // ===== Tag Operations =====

    /// Create a new tag
    pub fn create_tag(&self, name: &str, color: Option<u32>) -> Result<i64> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        conn.execute(
            "INSERT INTO tags (name, color) VALUES (?1, ?2)",
            rusqlite::params![name, color],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get all tags
    pub fn list_tags(&self) -> Result<Vec<TagRecord>> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let mut stmt = conn.prepare("SELECT tag_id, name, color, parent_tag_id FROM tags ORDER BY name")?;

        let rows = stmt.query_map([], |row| {
            Ok(TagRecord {
                tag_id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                parent_tag_id: row.get(3)?,
            })
        })?;

        let mut tags = Vec::new();
        for row in rows {
            tags.push(row?);
        }

        Ok(tags)
    }

    /// Add a tag to a file
    pub fn add_tag_to_file(&self, file_id: i64, tag_id: i64) -> Result<()> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        conn.execute(
            "INSERT OR IGNORE INTO file_tags (file_id, tag_id) VALUES (?1, ?2)",
            [file_id, tag_id],
        )?;

        Ok(())
    }

    /// Remove a tag from a file
    pub fn remove_tag_from_file(&self, file_id: i64, tag_id: i64) -> Result<()> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        conn.execute(
            "DELETE FROM file_tags WHERE file_id = ?1 AND tag_id = ?2",
            [file_id, tag_id],
        )?;

        Ok(())
    }

    /// Get files with a specific tag
    pub fn get_files_by_tag(&self, tag_id: i64, limit: usize) -> Result<Vec<FileRecord>> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let mut stmt = conn.prepare(
            "SELECT f.file_id, f.path_hash, f.path_display, f.path_blob, f.parent_hash, f.file_name, f.extension, f.size, f.modified_at, f.created_at, f.metadata, f.indexed_at
             FROM files f
             INNER JOIN file_tags ft ON f.file_id = ft.file_id
             WHERE ft.tag_id = ?1
             ORDER BY f.file_name COLLATE NOCASE
             LIMIT ?2"
        )?;

        let rows = stmt.query_map([tag_id, limit as i64], |row| {
            Ok(FileRecord {
                file_id: row.get(0)?,
                path_hash: row.get(1)?,
                path_display: row.get(2)?,
                path_blob: row.get(3)?,
                parent_hash: row.get(4)?,
                file_name: row.get(5)?,
                extension: row.get(6)?,
                size: row.get(7)?,
                modified_at: row.get(8)?,
                created_at: row.get(9)?,
                metadata: row.get(10)?,
                indexed_at: row.get(11)?,
            })
        })?;

        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }

        Ok(files)
    }

    // ===== Rating Operations =====

    /// Set rating for a file (0-5)
    pub fn set_rating(&self, path_hash: u64, rating: i32) -> Result<()> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        // Get current metadata JSON and update rating
        let current_metadata: Option<String> = conn.query_row(
            "SELECT metadata FROM files WHERE path_hash = ?1",
            [path_hash as i64],
            |row| row.get(0),
        ).ok().flatten();

        let new_metadata = match current_metadata {
            Some(json_str) => {
                if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    json["rating"] = serde_json::json!(rating);
                    serde_json::to_string(&json).unwrap_or_else(|_| format!(r#"{{"rating":{}}}"#, rating))
                } else {
                    format!(r#"{{"rating":{}}}"#, rating)
                }
            }
            None => format!(r#"{{"rating":{}}}"#, rating),
        };

        conn.execute(
            "UPDATE files SET metadata = ?1 WHERE path_hash = ?2",
            rusqlite::params![new_metadata, path_hash as i64],
        )?;

        Ok(())
    }

    /// Get rating for a file (returns 0 if not set)
    pub fn get_rating(&self, path_hash: u64) -> Result<i32> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let metadata: Option<String> = conn.query_row(
            "SELECT metadata FROM files WHERE path_hash = ?1",
            [path_hash as i64],
            |row| row.get(0),
        ).ok().flatten();

        let rating = metadata
            .and_then(|json_str| serde_json::from_str::<serde_json::Value>(&json_str).ok())
            .and_then(|json| json["rating"].as_i64())
            .map(|r| r as i32)
            .unwrap_or(0);

        Ok(rating)
    }

    /// Set label color for a file
    pub fn set_label(&self, path_hash: u64, label: Option<u32>) -> Result<()> {
        let conn = self.pool.get().map_err(|e| DbError::Pool(e.to_string()))?;

        let current_metadata: Option<String> = conn.query_row(
            "SELECT metadata FROM files WHERE path_hash = ?1",
            [path_hash as i64],
            |row| row.get(0),
        ).ok().flatten();

        let new_metadata = match current_metadata {
            Some(json_str) => {
                if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    json["label"] = serde_json::json!(label);
                    serde_json::to_string(&json).unwrap_or_else(|_| {
                        // Fallback: use proper JSON serialization
                        serde_json::json!({"label": label}).to_string()
                    })
                } else {
                    // Invalid existing JSON: create new properly formatted JSON
                    serde_json::json!({"label": label}).to_string()
                }
            }
            None => serde_json::json!({"label": label}).to_string(),
        };

        conn.execute(
            "UPDATE files SET metadata = ?1 WHERE path_hash = ?2",
            rusqlite::params![new_metadata, path_hash as i64],
        )?;

        Ok(())
    }
}
