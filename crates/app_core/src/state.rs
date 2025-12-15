//! Application state management

use crate::{AppConfig, AppError, CommandDispatcher, NavigationState, ResourceManager};
use app_db::{DbPool, MetadataDb, ThumbnailCache};
use parking_lot::RwLock;
use std::sync::Arc;

/// Main application state
pub struct AppState {
    /// Application configuration
    pub config: RwLock<AppConfig>,

    /// Database pool
    pub db_pool: DbPool,

    /// Metadata database operations
    pub metadata_db: MetadataDb,

    /// Thumbnail cache
    pub thumbnail_cache: Arc<ThumbnailCache>,

    /// Resource manager (textures, bitmaps)
    pub resources: Arc<ResourceManager>,

    /// Navigation state
    pub navigation: RwLock<NavigationState>,

    /// Command dispatcher
    pub commands: RwLock<CommandDispatcher>,

    /// Is the application in fullscreen mode?
    pub is_fullscreen: RwLock<bool>,

    /// Current zoom level
    pub zoom: RwLock<f32>,

    /// Current pan offset (x, y)
    pub pan: RwLock<(f32, f32)>,

    /// Current rotation (degrees)
    pub rotation: RwLock<i32>,
}

impl AppState {
    /// Create a new application state
    pub fn new(config: AppConfig) -> Result<Self, AppError> {
        // Initialize databases
        let (db_pool, thumbnail_cache) = app_db::init()
            .map_err(|e| AppError::Init(e.to_string()))?;

        let metadata_db = MetadataDb::new(db_pool.clone());
        let thumbnail_cache = Arc::new(thumbnail_cache);

        // Initialize resource manager
        let resources = Arc::new(ResourceManager::new());

        Ok(Self {
            config: RwLock::new(config),
            db_pool,
            metadata_db,
            thumbnail_cache,
            resources,
            navigation: RwLock::new(NavigationState::new()),
            commands: RwLock::new(CommandDispatcher::new()),
            is_fullscreen: RwLock::new(false),
            zoom: RwLock::new(1.0),
            pan: RwLock::new((0.0, 0.0)),
            rotation: RwLock::new(0),
        })
    }

    /// Save the current configuration
    pub fn save_config(&self) -> anyhow::Result<()> {
        self.config.read().save()
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&self) -> bool {
        let mut fs = self.is_fullscreen.write();
        *fs = !*fs;
        *fs
    }

    /// Set zoom level
    pub fn set_zoom(&self, level: f32) {
        *self.zoom.write() = level.clamp(0.1, 10.0);
    }

    /// Zoom in
    pub fn zoom_in(&self) {
        let current = *self.zoom.read();
        self.set_zoom(current * 1.2);
    }

    /// Zoom out
    pub fn zoom_out(&self) {
        let current = *self.zoom.read();
        self.set_zoom(current / 1.2);
    }

    /// Reset zoom to 1.0
    pub fn zoom_reset(&self) {
        *self.zoom.write() = 1.0;
    }

    /// Set pan offset
    pub fn set_pan(&self, x: f32, y: f32) {
        *self.pan.write() = (x, y);
    }

    /// Rotate left (counterclockwise)
    pub fn rotate_left(&self) {
        let mut rot = self.rotation.write();
        *rot = (*rot - 90) % 360;
        if *rot < 0 {
            *rot += 360;
        }
    }

    /// Rotate right (clockwise)
    pub fn rotate_right(&self) {
        let mut rot = self.rotation.write();
        *rot = (*rot + 90) % 360;
    }

    /// Reset rotation
    pub fn reset_rotation(&self) {
        *self.rotation.write() = 0;
    }
}
