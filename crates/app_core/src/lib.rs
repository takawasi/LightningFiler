//! LightningFiler Core Domain Logic
//!
//! This crate contains:
//! - Application state management
//! - Command system
//! - Configuration
//! - Error types
//! - Navigation context
//! - Resource management
//! - Image loading

pub mod state;
pub mod config;
pub mod command;
pub mod error;
pub mod navigation;
pub mod resource;
pub mod i18n;
pub mod image_loader;
pub mod thumbnail_manager;

pub use state::AppState;
pub use config::{
    AppConfig, GeneralConfig, ViewerConfig, FilerConfig, NavigationConfig,
    FitMode, Interpolation, SpreadMode, ReadingDirection,
    SortBy, SortOrder, ViewMode,
};
pub use command::{
    Command, CommandId, CommandDispatcher, CommandParams, CommandHandler,
    // Enums
    CenterMode, ZoomMode, Direction, ScrollUnit, Position, SyncMode,
    SlideshowAction, SlideshowOrder, FlipAxis, BackgroundColor,
    InfoLevel, TransitionMode, PathFormat, LabelColor, CopyTarget,
};
// Note: SpreadMode is exported from config module
pub use error::AppError;
pub use navigation::{NavigationContext, NavigationState, GridLayout, SelectionState, FileEntry as NavFileEntry};
pub use resource::ResourceManager;
pub use image_loader::{ImageLoader, LoadedImage, ThumbnailGenerator, is_supported_image, get_image_dimensions};
pub use thumbnail_manager::{ThumbnailManager, ThumbnailSize, CacheStats};

use once_cell::sync::OnceCell;

/// Global application state (for UI access)
static APP_STATE: OnceCell<AppState> = OnceCell::new();

/// Initialize global application state
pub fn init(config: AppConfig) -> anyhow::Result<&'static AppState> {
    let state = AppState::new(config)?;
    APP_STATE.set(state).map_err(|_| anyhow::anyhow!("AppState already initialized"))?;
    Ok(APP_STATE.get().unwrap())
}

/// Get global application state
pub fn state() -> Option<&'static AppState> {
    APP_STATE.get()
}
