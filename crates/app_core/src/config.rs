//! Application configuration

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub viewer: ViewerConfig,
    pub filer: FilerConfig,
    pub keybindings: HashMap<String, Vec<String>>,
    pub recent_folders: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            viewer: ViewerConfig::default(),
            filer: FilerConfig::default(),
            keybindings: default_keybindings(),
            recent_folders: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub language: String,
    pub theme: String,
    pub start_maximized: bool,
    pub remember_window_state: bool,
    pub check_updates: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: "ja".to_string(),
            theme: "dark".to_string(),
            start_maximized: false,
            remember_window_state: true,
            check_updates: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewerConfig {
    pub background_color: String,
    pub fit_mode: FitMode,
    pub interpolation: Interpolation,
    pub spread_mode: SpreadMode,
    pub reading_direction: ReadingDirection,
    pub slideshow_interval_ms: u64,
    pub enable_animation: bool,
    pub preload_count: usize,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            background_color: "#202020".to_string(),
            fit_mode: FitMode::FitToWindow,
            interpolation: Interpolation::Lanczos3,
            spread_mode: SpreadMode::Single,
            reading_direction: ReadingDirection::LeftToRight,
            slideshow_interval_ms: 3000,
            enable_animation: true,
            preload_count: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FilerConfig {
    pub show_hidden_files: bool,
    pub sort_by: SortBy,
    pub sort_order: SortOrder,
    pub thumbnail_size: u32,
    pub view_mode: ViewMode,
    pub confirm_delete: bool,
    pub use_recycle_bin: bool,
}

impl Default for FilerConfig {
    fn default() -> Self {
        Self {
            show_hidden_files: false,
            sort_by: SortBy::Name,
            sort_order: SortOrder::Ascending,
            thumbnail_size: 128,
            view_mode: ViewMode::Grid,
            confirm_delete: true,
            use_recycle_bin: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitMode {
    #[serde(rename = "fit")]
    FitToWindow,
    #[serde(rename = "width")]
    FitWidth,
    #[serde(rename = "height")]
    FitHeight,
    #[serde(rename = "original")]
    OriginalSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interpolation {
    #[serde(rename = "nearest")]
    Nearest,
    #[serde(rename = "bilinear")]
    Bilinear,
    #[serde(rename = "lanczos3")]
    Lanczos3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpreadMode {
    #[serde(rename = "single")]
    Single,
    #[serde(rename = "spread")]
    Spread,
    #[serde(rename = "auto")]
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadingDirection {
    #[serde(rename = "ltr")]
    LeftToRight,
    #[serde(rename = "rtl")]
    RightToLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortBy {
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "size")]
    Size,
    #[serde(rename = "modified")]
    Modified,
    #[serde(rename = "type")]
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    #[serde(rename = "asc")]
    Ascending,
    #[serde(rename = "desc")]
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewMode {
    #[serde(rename = "grid")]
    Grid,
    #[serde(rename = "list")]
    List,
    #[serde(rename = "details")]
    Details,
}

impl AppConfig {
    /// Load configuration from file
    pub fn load() -> anyhow::Result<Self> {
        let config_path = Self::config_path();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Self = toml::from_str(&content)?;
            tracing::info!("Configuration loaded from {:?}", config_path);
            Ok(config)
        } else {
            tracing::info!("Using default configuration");
            Ok(Self::default())
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> anyhow::Result<()> {
        let config_path = Self::config_path();

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

        tracing::info!("Configuration saved to {:?}", config_path);
        Ok(())
    }

    /// Get the configuration file path
    pub fn config_path() -> PathBuf {
        ProjectDirs::from("com", "LightningFiler", "LightningFiler")
            .map(|dirs| dirs.config_dir().join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("./config.toml"))
    }
}

fn default_keybindings() -> HashMap<String, Vec<String>> {
    let mut kb = HashMap::new();

    // Navigation
    kb.insert("nav.next_item".into(), vec!["Right".into(), "l".into(), "Space".into()]);
    kb.insert("nav.prev_item".into(), vec!["Left".into(), "h".into(), "Shift+Space".into()]);
    kb.insert("nav.first_item".into(), vec!["Home".into()]);
    kb.insert("nav.last_item".into(), vec!["End".into()]);
    kb.insert("nav.up_folder".into(), vec!["Backspace".into(), "u".into()]);
    kb.insert("nav.enter_folder".into(), vec!["Return".into(), "o".into()]);
    kb.insert("nav.skip_forward".into(), vec!["Ctrl+Right".into()]);
    kb.insert("nav.skip_backward".into(), vec!["Ctrl+Left".into()]);

    // View
    kb.insert("view.toggle_fullscreen".into(), vec!["F11".into(), "f".into()]);
    kb.insert("view.zoom_in".into(), vec!["Plus".into(), "Ctrl+Up".into()]);
    kb.insert("view.zoom_out".into(), vec!["Minus".into(), "Ctrl+Down".into()]);
    kb.insert("view.zoom_reset".into(), vec!["0".into()]);
    kb.insert("view.fit_to_window".into(), vec!["Ctrl+0".into()]);
    kb.insert("view.original_size".into(), vec!["1".into()]);
    kb.insert("view.rotate_left".into(), vec!["Ctrl+Left".into()]);
    kb.insert("view.rotate_right".into(), vec!["Ctrl+Right".into()]);

    // File
    kb.insert("file.delete".into(), vec!["Delete".into()]);
    kb.insert("file.rename".into(), vec!["F2".into()]);
    kb.insert("file.copy".into(), vec!["Ctrl+c".into()]);
    kb.insert("file.cut".into(), vec!["Ctrl+x".into()]);
    kb.insert("file.paste".into(), vec!["Ctrl+v".into()]);

    // App
    kb.insert("app.open_settings".into(), vec!["Ctrl+Comma".into()]);
    kb.insert("app.quit".into(), vec!["Alt+F4".into(), "q".into()]);
    kb.insert("app.search".into(), vec!["Ctrl+f".into(), "Slash".into()]);

    kb
}
