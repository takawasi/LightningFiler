//! Resource management (textures, bitmaps, caching)

use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;

/// Resource manager for VRAM and RAM caching
pub struct ResourceManager {
    /// RAM cache: decoded images
    ram_cache: RwLock<LruCache<u64, Arc<DecodedImage>>>,

    /// Currently loading images (to prevent duplicate loads)
    loading: DashMap<u64, tokio::sync::broadcast::Sender<LoadResult>>,

    /// Memory limits
    ram_limit: usize,
    current_ram_usage: RwLock<usize>,
}

/// Decoded image in RAM
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub format: ImageFormat,
}

#[derive(Debug, Clone, Copy)]
pub enum ImageFormat {
    Rgba8,
    Rgb8,
}

type LoadResult = Result<Arc<DecodedImage>, String>;

/// Simple LRU cache
struct LruCache<K, V> {
    capacity: usize,
    entries: Vec<(K, V, usize)>, // key, value, access_order
    order_counter: usize,
}

impl<K: Eq + Clone, V> LruCache<K, V> {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Vec::with_capacity(capacity),
            order_counter: 0,
        }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        if let Some(idx) = self.entries.iter().position(|(k, _, _)| k == key) {
            self.order_counter += 1;
            self.entries[idx].2 = self.order_counter;
            Some(&self.entries[idx].1)
        } else {
            None
        }
    }

    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.order_counter += 1;

        // Check if key exists
        if let Some(idx) = self.entries.iter().position(|(k, _, _)| k == &key) {
            let old = std::mem::replace(&mut self.entries[idx].1, value);
            self.entries[idx].2 = self.order_counter;
            return Some(old);
        }

        // Evict if necessary
        if self.entries.len() >= self.capacity {
            // Find LRU entry
            if let Some((idx, _)) = self.entries
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, _, order))| order)
            {
                self.entries.remove(idx);
            }
        }

        self.entries.push((key, value, self.order_counter));
        None
    }

    fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(idx) = self.entries.iter().position(|(k, _, _)| k == key) {
            Some(self.entries.remove(idx).1)
        } else {
            None
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order_counter = 0;
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}

impl ResourceManager {
    /// Create a new resource manager with default limits
    pub fn new() -> Self {
        // Default: 512MB RAM cache
        Self::with_limits(512 * 1024 * 1024)
    }

    /// Create with specific memory limits
    pub fn with_limits(ram_limit: usize) -> Self {
        Self {
            ram_cache: RwLock::new(LruCache::new(1000)),
            loading: DashMap::new(),
            ram_limit,
            current_ram_usage: RwLock::new(0),
        }
    }

    /// Get a cached image
    pub fn get_image(&self, hash: u64) -> Option<Arc<DecodedImage>> {
        self.ram_cache.write().get(&hash).cloned()
    }

    /// Store a decoded image
    pub fn store_image(&self, hash: u64, image: DecodedImage) -> Arc<DecodedImage> {
        let size = image.data.len();
        let image = Arc::new(image);

        // Check memory pressure
        {
            let mut usage = self.current_ram_usage.write();
            *usage += size;

            // Simple eviction if over limit
            while *usage > self.ram_limit {
                // Would need proper LRU tracking here
                break;
            }
        }

        self.ram_cache.write().insert(hash, image.clone());
        image
    }

    /// Remove an image from cache
    pub fn remove_image(&self, hash: u64) {
        if let Some(image) = self.ram_cache.write().remove(&hash) {
            let mut usage = self.current_ram_usage.write();
            *usage = usage.saturating_sub(image.data.len());
        }
    }

    /// Clear all caches
    pub fn clear(&self) {
        self.ram_cache.write().clear();
        *self.current_ram_usage.write() = 0;
    }

    /// Get current RAM usage
    pub fn ram_usage(&self) -> usize {
        *self.current_ram_usage.read()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            ram_entries: self.ram_cache.read().len(),
            ram_usage: *self.current_ram_usage.read(),
            ram_limit: self.ram_limit,
        }
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub ram_entries: usize,
    pub ram_usage: usize,
    pub ram_limit: usize,
}
