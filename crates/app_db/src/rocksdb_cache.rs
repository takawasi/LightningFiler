//! RocksDB-based thumbnail and hash cache

use crate::Result;
use rocksdb::{Options, DB};
use std::path::Path;

/// Key for thumbnail cache
#[derive(Debug, Clone, Copy)]
pub struct CacheKey {
    /// File content hash (xxh3)
    pub hash: u64,
    /// Thumbnail width
    pub width: u32,
    /// Thumbnail height
    pub height: u32,
}

impl CacheKey {
    /// Create a new cache key
    pub fn new(hash: u64, width: u32, height: u32) -> Self {
        Self { hash, width, height }
    }

    /// Serialize to bytes (16 bytes total)
    pub fn to_bytes(&self) -> [u8; 16] {
        let mut key = [0u8; 16];
        key[0..8].copy_from_slice(&self.hash.to_be_bytes());
        key[8..12].copy_from_slice(&self.width.to_be_bytes());
        key[12..16].copy_from_slice(&self.height.to_be_bytes());
        key
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 16 {
            return None;
        }

        Some(Self {
            hash: u64::from_be_bytes(bytes[0..8].try_into().ok()?),
            width: u32::from_be_bytes(bytes[8..12].try_into().ok()?),
            height: u32::from_be_bytes(bytes[12..16].try_into().ok()?),
        })
    }
}

/// Thumbnail cache using RocksDB
pub struct ThumbnailCache {
    db: DB,
}

impl ThumbnailCache {
    /// Open or create the cache database
    pub fn open(path: &Path) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        opts.set_max_open_files(256);
        opts.set_write_buffer_size(64 * 1024 * 1024); // 64MB
        opts.set_max_write_buffer_number(3);
        opts.set_target_file_size_base(64 * 1024 * 1024);

        let db = DB::open(&opts, path)?;
        Ok(Self { db })
    }

    /// Store a thumbnail
    pub fn put(&self, key: CacheKey, data: &[u8]) -> Result<()> {
        self.db.put(key.to_bytes(), data)?;
        Ok(())
    }

    /// Retrieve a thumbnail
    pub fn get(&self, key: CacheKey) -> Result<Option<Vec<u8>>> {
        Ok(self.db.get(key.to_bytes())?)
    }

    /// Delete a thumbnail
    pub fn delete(&self, key: CacheKey) -> Result<()> {
        self.db.delete(key.to_bytes())?;
        Ok(())
    }

    /// Delete all thumbnails for a file hash
    pub fn delete_by_hash(&self, hash: u64) -> Result<usize> {
        let prefix = hash.to_be_bytes();
        let mut count = 0;

        let iter = self.db.prefix_iterator(&prefix);
        for item in iter {
            let (key, _) = item?;
            if key.starts_with(&prefix) {
                self.db.delete(&key)?;
                count += 1;
            } else {
                break;
            }
        }

        Ok(count)
    }

    /// Check if a thumbnail exists
    pub fn exists(&self, key: CacheKey) -> Result<bool> {
        Ok(self.db.get_pinned(key.to_bytes())?.is_some())
    }

    /// Get approximate cache size
    pub fn approximate_size(&self) -> u64 {
        self.db
            .property_int_value("rocksdb.estimate-live-data-size")
            .unwrap_or(None)
            .unwrap_or(0)
    }

    /// Compact the database
    pub fn compact(&self) {
        self.db.compact_range::<[u8; 0], [u8; 0]>(None, None);
    }

    /// Store a file content hash
    pub fn put_file_hash(&self, path_hash: u64, content_hash: u64) -> Result<()> {
        let mut key = b"hash:".to_vec();
        key.extend_from_slice(&path_hash.to_be_bytes());
        self.db.put(&key, content_hash.to_be_bytes())?;
        Ok(())
    }

    /// Get a file content hash
    pub fn get_file_hash(&self, path_hash: u64) -> Result<Option<u64>> {
        let mut key = b"hash:".to_vec();
        key.extend_from_slice(&path_hash.to_be_bytes());

        match self.db.get(&key)? {
            Some(bytes) if bytes.len() == 8 => {
                Ok(Some(u64::from_be_bytes(bytes[..8].try_into().unwrap())))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_key() {
        let key = CacheKey::new(12345, 256, 256);
        let bytes = key.to_bytes();
        let restored = CacheKey::from_bytes(&bytes).unwrap();

        assert_eq!(key.hash, restored.hash);
        assert_eq!(key.width, restored.width);
        assert_eq!(key.height, restored.height);
    }

    #[test]
    fn test_cache_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache = ThumbnailCache::open(temp_dir.path()).unwrap();

        let key = CacheKey::new(12345, 256, 256);
        let data = vec![1, 2, 3, 4, 5];

        // Put
        cache.put(key, &data).unwrap();

        // Get
        let retrieved = cache.get(key).unwrap().unwrap();
        assert_eq!(retrieved, data);

        // Exists
        assert!(cache.exists(key).unwrap());

        // Delete
        cache.delete(key).unwrap();
        assert!(!cache.exists(key).unwrap());
    }
}
