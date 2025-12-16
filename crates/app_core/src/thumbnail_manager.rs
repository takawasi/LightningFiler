//! Thumbnail generation and caching manager
//!
//! Integrates:
//! - RocksDB thumbnail cache
//! - Async thumbnail generation
//! - Memory-based texture cache

use crate::{AppError, ThumbnailGenerator, LoadedImage};
use app_db::{ThumbnailCache, CacheKey};
use app_fs::UniversalPath;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use xxhash_rust::xxh3::xxh3_64;

/// Thumbnail size presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThumbnailSize {
    Small,  // 128x128
    Medium, // 256x256
    Large,  // 512x512
}

impl ThumbnailSize {
    pub fn to_dimensions(self) -> (u32, u32) {
        match self {
            ThumbnailSize::Small => (128, 128),
            ThumbnailSize::Medium => (256, 256),
            ThumbnailSize::Large => (512, 512),
        }
    }

    pub fn to_u32(self) -> u32 {
        match self {
            ThumbnailSize::Small => 128,
            ThumbnailSize::Medium => 256,
            ThumbnailSize::Large => 512,
        }
    }
}

/// Thumbnail request
#[derive(Debug)]
struct ThumbnailRequest {
    path: UniversalPath,
    size: ThumbnailSize,
    callback: tokio::sync::oneshot::Sender<Result<LoadedImage, AppError>>,
}

/// Thumbnail manager handles generation and caching
#[derive(Clone)]
pub struct ThumbnailManager {
    /// RocksDB cache for persistent storage
    cache: Arc<ThumbnailCache>,

    /// In-memory cache for recently loaded thumbnails
    memory_cache: Arc<RwLock<HashMap<(u64, ThumbnailSize), Vec<u8>>>>,

    /// Channel for thumbnail generation requests
    request_tx: mpsc::UnboundedSender<ThumbnailRequest>,
}

impl ThumbnailManager {
    /// Create a new thumbnail manager
    pub fn new(cache: Arc<ThumbnailCache>) -> Self {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<ThumbnailRequest>();
        let cache_clone = cache.clone();
        let memory_cache = Arc::new(RwLock::new(HashMap::new()));
        let memory_cache_clone = memory_cache.clone();

        // Spawn worker thread for thumbnail generation
        std::thread::spawn(move || {
            while let Some(request) = request_rx.blocking_recv() {
                let result = Self::generate_thumbnail_sync(
                    &request.path,
                    request.size,
                    &cache_clone,
                    &memory_cache_clone,
                );
                let _ = request.callback.send(result);
            }
        });

        Self {
            cache,
            memory_cache,
            request_tx,
        }
    }

    /// Request a thumbnail asynchronously
    /// Returns cached thumbnail immediately if available, otherwise generates in background
    pub async fn get_thumbnail(
        &self,
        path: UniversalPath,
        size: ThumbnailSize,
    ) -> Result<LoadedImage, AppError> {
        // Calculate file hash
        let file_data = tokio::fs::read(path.as_path()).await?;
        let hash = xxh3_64(&file_data);

        // Check memory cache first
        {
            let cache_read = self.memory_cache.read().await;
            if let Some(data) = cache_read.get(&(hash, size)) {
                let (width, height) = size.to_dimensions();
                return Ok(LoadedImage {
                    path: path.clone(),
                    width,
                    height,
                    data: data.clone(),
                    format: crate::resource::ImageFormat::Rgba8,
                    hash,
                });
            }
        }

        // Check RocksDB cache
        let (width, height) = size.to_dimensions();
        let cache_key = CacheKey::new(hash, width, height);

        if let Some(cached_data) = self.cache.get(cache_key)? {
            // Store in memory cache
            let mut cache_write = self.memory_cache.write().await;
            cache_write.insert((hash, size), cached_data.clone());

            return Ok(LoadedImage {
                path: path.clone(),
                width,
                height,
                data: cached_data,
                format: crate::resource::ImageFormat::Rgba8,
                hash,
            });
        }

        // Not cached - request generation
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.request_tx.send(ThumbnailRequest {
            path,
            size,
            callback: tx,
        }).map_err(|_| AppError::SystemResource("Thumbnail manager channel closed".into()))?;

        rx.await.map_err(|_| AppError::SystemResource("Thumbnail generation failed".into()))?
    }

    /// Generate thumbnail synchronously (called from worker thread)
    fn generate_thumbnail_sync(
        path: &UniversalPath,
        size: ThumbnailSize,
        cache: &ThumbnailCache,
        memory_cache: &Arc<RwLock<HashMap<(u64, ThumbnailSize), Vec<u8>>>>,
    ) -> Result<LoadedImage, AppError> {
        tracing::debug!("Generating thumbnail: {} ({:?})", path, size);

        let generator = ThumbnailGenerator::new(size.to_u32());
        let loaded = generator.generate(path.as_path())?;

        // Store in RocksDB cache
        let (width, height) = size.to_dimensions();
        let cache_key = CacheKey::new(loaded.hash, width, height);
        cache.put(cache_key, &loaded.data)?;

        // Store in memory cache
        let mut mem_cache = memory_cache.blocking_write();
        mem_cache.insert((loaded.hash, size), loaded.data.clone());

        // Limit memory cache size (keep last 100 thumbnails)
        if mem_cache.len() > 100 {
            // Remove oldest entries (simple LRU approximation)
            let keys_to_remove: Vec<_> = mem_cache.keys().take(20).cloned().collect();
            for key in keys_to_remove {
                mem_cache.remove(&key);
            }
        }

        Ok(loaded)
    }

    /// Get thumbnail synchronously if cached, otherwise return None
    pub fn get_cached_sync(&self, path: &Path, size: ThumbnailSize) -> Option<LoadedImage> {
        let file_data = std::fs::read(path).ok()?;
        let hash = xxh3_64(&file_data);
        let (width, height) = size.to_dimensions();
        let cache_key = CacheKey::new(hash, width, height);

        // Check RocksDB cache
        let cached_data = self.cache.get(cache_key).ok()??;

        Some(LoadedImage {
            path: UniversalPath::new(path),
            width,
            height,
            data: cached_data,
            format: crate::resource::ImageFormat::Rgba8,
            hash,
        })
    }

    /// Check if a thumbnail exists in cache
    pub fn has_cached(&self, path: &Path, size: ThumbnailSize) -> Result<bool, AppError> {
        let file_data = std::fs::read(path)?;
        let hash = xxh3_64(&file_data);
        let (width, height) = size.to_dimensions();
        let cache_key = CacheKey::new(hash, width, height);

        Ok(self.cache.exists(cache_key)?)
    }

    /// Clear memory cache
    pub async fn clear_memory_cache(&self) {
        let mut cache_write = self.memory_cache.write().await;
        cache_write.clear();
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        let memory_size = self.memory_cache.read().await.len();
        let disk_size = self.cache.approximate_size();

        CacheStats {
            memory_entries: memory_size,
            disk_size_bytes: disk_size,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub memory_entries: usize,
    pub disk_size_bytes: u64,
}
