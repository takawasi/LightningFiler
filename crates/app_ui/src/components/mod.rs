//! UI Components

pub mod file_browser;
pub mod viewer;
pub mod toolbar;
pub mod status_bar;
pub mod settings;
pub mod dialogs;
pub mod spread_viewer;
pub mod split_view;
pub mod effects;
pub mod slideshow;
pub mod folder_tree;
pub mod thumbnail_catalog;

pub use file_browser::{FileBrowser, FileItem, BrowserAction, BrowserViewMode};
pub use viewer::{ImageViewer, ViewerAction, FitMode};
pub use toolbar::{Toolbar, ToolbarAction};
pub use status_bar::{StatusBar, StatusInfo};
pub use settings::{SettingsDialog, SettingsTab, SettingsAction};
pub use dialogs::{Dialog, DialogResult, ConfirmDialog, RenameDialog, TagEditDialog};
pub use spread_viewer::{SpreadViewer, SpreadMode, SpreadLayout, PagePosition};
pub use split_view::{SplitView, SplitDirection, SplitPane, SplitViewResponse};
pub use effects::{ImageTransform, Rotation, ViewerBackground, BackgroundColor, PageTransition, TransitionType};
pub use slideshow::{Slideshow, SlideshowState, SlideshowConfig};
pub use folder_tree::{FolderTree, FolderTreeAction, FolderNode};
pub use thumbnail_catalog::{ThumbnailCatalog, ThumbnailItem, CatalogAction, NavigateDirection};
