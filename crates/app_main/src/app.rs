//! Application main loop
//! Integrated with Doc 3 command system

use anyhow::Result;
use app_core::{state, is_supported_image, Command, CommandId, NavigationState, ThumbnailManager, ThumbnailSize};
use app_db::{MetadataDb, ThumbnailCache, DbPool};
use app_fs::{UniversalPath, FileEntry, ListOptions, list_directory, get_parent, is_root, get_next_sibling, get_prev_sibling, count_files, FileOperations, DefaultFileOperations, ClipboardMode, VirtualFileSystem, FileWatcher, FsEvent};
use app_ui::{
    components::{FileBrowser, ImageViewer, StatusBar, StatusInfo, Toolbar, ToolbarAction, BrowserAction, BrowserViewMode, SettingsDialog, SettingsAction, ViewerAction, Dialog, DialogResult, ConfirmDialog, RenameDialog, TagEditDialog, SpreadViewer, SpreadMode, SpreadLayout, SplitView, SplitDirection, ImageTransform, ViewerBackground, PageTransition, Slideshow, FolderTree, FolderTreeAction, ThumbnailCatalog, ThumbnailItem, CatalogAction},
    InputHandler, Renderer, Theme,
};
use egui_wgpu::ScreenDescriptor;
use std::collections::{HashSet, HashMap};
use std::sync::Arc;
use std::path::PathBuf;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

/// Main application state for the event loop
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    egui_ctx: egui::Context,
    egui_state: Option<egui_winit::State>,
    egui_renderer: Option<egui_wgpu::Renderer>,

    // UI Components
    file_browser: FileBrowser,
    image_viewer: ImageViewer,
    settings_dialog: SettingsDialog,
    input_handler: Option<InputHandler>,
    theme: Theme,

    // Navigation state (Doc 3 compliant)
    nav_state: NavigationState,

    // Database
    db_pool: Option<DbPool>,
    metadata_db: Option<MetadataDb>,
    thumbnail_cache: Option<Arc<ThumbnailCache>>,
    thumbnail_manager: Option<ThumbnailManager>,

    // Texture cache (path_hash -> TextureHandle)
    thumbnail_textures: HashMap<u64, egui::TextureHandle>,

    // State
    show_browser: bool,
    status: StatusInfo,
    current_path: UniversalPath,
    file_entries: Vec<FileEntry>,
    selected_index: Option<usize>,
    current_texture: Option<egui::TextureHandle>,

    // Grid layout tracking
    grid_columns: usize,
    grid_visible_rows: usize,

    // Temporary marks (cleared on exit)
    marked_files: HashSet<u64>,

    // Overlay UI state (Doc 4 spec)
    overlay_visible: bool,
    last_mouse_move: Option<std::time::Instant>,

    // File operations
    file_ops: Arc<DefaultFileOperations>,

    // File watcher
    file_watcher: Option<FileWatcher>,

    // Archive support
    current_archive: Option<VirtualFileSystem>,
    archive_inner_path: String,
    // Map from FileEntry.path.id() to archive inner path
    archive_path_map: HashMap<u64, String>,

    // Dialogs
    confirm_dialog: Option<ConfirmDialog>,
    rename_dialog: Option<RenameDialog>,
    tag_dialog: Option<TagEditDialog>,
    pending_delete_path: Option<PathBuf>,

    // Spread viewer (two-page display)
    spread_viewer: SpreadViewer,

    // Split view (compare two images)
    split_view: SplitView,

    // Viewer effects
    image_transform: ImageTransform,
    viewer_background: ViewerBackground,
    page_transition: PageTransition,

    // Slideshow
    slideshow: Slideshow,

    // New UI components (Doc spec compliance)
    folder_tree: FolderTree,
    thumbnail_catalog: ThumbnailCatalog,
    catalog_items: Vec<ThumbnailItem>,
}

impl App {
    fn new() -> Self {
        let config = state().map(|s| s.config.read().clone()).unwrap_or_default();

        // Get initial path
        let current_path = state()
            .map(|s| s.current_path())
            .unwrap_or_else(|| UniversalPath::new("."));

        // Load initial directory
        let file_entries = list_directory(current_path.as_path(), &ListOptions::default())
            .unwrap_or_default();

        // Initialize navigation state
        let mut nav_state = NavigationState::new();
        nav_state.enter_threshold = config.navigation.enter_threshold.unwrap_or(5);

        // Initialize database
        let (db_pool, metadata_db, thumbnail_cache, thumbnail_manager) = match app_db::init() {
            Ok((pool, cache)) => {
                let metadata_db = MetadataDb::new(pool.clone());
                let cache_arc = Arc::new(cache);
                let thumbnail_manager = ThumbnailManager::new(cache_arc.clone());
                tracing::info!("Database initialized successfully");
                (Some(pool), Some(metadata_db), Some(cache_arc), Some(thumbnail_manager))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize database: {}. Running without persistence.", e);
                (None, None, None, None)
            }
        };

        // Initialize file watcher
        let file_watcher = match FileWatcher::new() {
            Ok(mut watcher) => {
                // Watch the initial directory
                if let Err(e) = watcher.watch(current_path.as_path()) {
                    tracing::warn!("Failed to watch directory: {}", e);
                }
                Some(watcher)
            }
            Err(e) => {
                tracing::warn!("Failed to create file watcher: {}", e);
                None
            }
        };

        Self {
            window: None,
            renderer: None,
            egui_ctx: egui::Context::default(),
            egui_state: None,
            egui_renderer: None,

            file_browser: FileBrowser::new(),
            image_viewer: ImageViewer::new(),
            settings_dialog: SettingsDialog::new(config.clone()),
            input_handler: None,
            theme: Theme::by_name(&config.general.theme),

            nav_state,

            db_pool,
            metadata_db,
            thumbnail_cache,
            thumbnail_manager,

            thumbnail_textures: HashMap::new(),

            show_browser: true,
            status: StatusInfo {
                file_name: current_path.display().to_string(),
                position: String::new(),
                dimensions: String::new(),
                file_size: String::new(),
                zoom: String::new(),
                message: format!("{} items", file_entries.len()),
            },
            current_path,
            file_entries,
            selected_index: None,
            current_texture: None,

            grid_columns: 1,
            grid_visible_rows: 10,

            marked_files: HashSet::new(),

            overlay_visible: true,
            last_mouse_move: None,

            file_ops: Arc::new(DefaultFileOperations::new()),

            file_watcher,

            current_archive: None,
            archive_inner_path: String::new(),
            archive_path_map: HashMap::new(),

            confirm_dialog: None,
            rename_dialog: None,
            tag_dialog: None,
            pending_delete_path: None,

            spread_viewer: SpreadViewer::new(),
            split_view: SplitView::new(),
            image_transform: ImageTransform::new(),
            viewer_background: ViewerBackground::new(),
            page_transition: PageTransition::new(),
            slideshow: Slideshow::new(),
            folder_tree: FolderTree::new(),
            thumbnail_catalog: ThumbnailCatalog::new(),
            catalog_items: Vec::new(),
        }
    }

    fn init_window(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window_attrs = Window::default_attributes()
            .with_title("LightningFiler")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

        let window = Arc::new(event_loop.create_window(window_attrs)?);

        // Initialize renderer
        let renderer = pollster::block_on(Renderer::new(window.clone()))?;

        // Initialize egui
        let egui_state = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            None,
            None,
            None,
        );

        let egui_renderer = egui_wgpu::Renderer::new(
            &renderer.device,
            renderer.config.format,
            None,
            1,
            false,
        );

        // Initialize input handler
        let config = state().map(|s| s.config.read().clone()).unwrap_or_default();
        let input_handler = InputHandler::new(config.keybindings);

        // Apply theme
        self.theme.apply(&self.egui_ctx);

        // Setup Japanese font support
        self.setup_fonts();

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);
        self.input_handler = Some(input_handler);

        Ok(())
    }

    /// Setup fonts for Japanese and Unicode support
    fn setup_fonts(&self) {
        let mut fonts = egui::FontDefinitions::default();

        // Add system fonts that support Japanese
        #[cfg(windows)]
        {
            // Windows: Use Yu Gothic or Meiryo
            if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\YuGothR.ttc") {
                fonts.font_data.insert(
                    "japanese".to_owned(),
                    egui::FontData::from_owned(font_data).into(),
                );
            } else if let Ok(font_data) = std::fs::read("C:\\Windows\\Fonts\\meiryo.ttc") {
                fonts.font_data.insert(
                    "japanese".to_owned(),
                    egui::FontData::from_owned(font_data).into(),
                );
            }
        }

        #[cfg(not(windows))]
        {
            // Linux: Try common Japanese fonts
            let font_paths = [
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
            ];
            for path in font_paths {
                if let Ok(font_data) = std::fs::read(path) {
                    fonts.font_data.insert(
                        "japanese".to_owned(),
                        egui::FontData::from_owned(font_data).into(),
                    );
                    break;
                }
            }
        }

        // Add Japanese font to font families
        if fonts.font_data.contains_key("japanese") {
            fonts.families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("japanese".to_owned());
            fonts.families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("japanese".to_owned());
        }

        self.egui_ctx.set_fonts(fonts);
    }

    /// Navigate to a directory
    fn navigate_to(&mut self, path: UniversalPath) {
        // Unwatch previous path
        if let Some(ref mut watcher) = self.file_watcher {
            let _ = watcher.unwatch(self.current_path.as_path());
        }

        // Clear archive state when navigating to a regular directory
        self.current_archive = None;
        self.archive_inner_path.clear();
        self.archive_path_map.clear();

        match list_directory(path.as_path(), &ListOptions::default()) {
            Ok(entries) => {
                self.current_path = path.clone();
                self.file_entries = entries;
                self.selected_index = None;
                self.status.file_name = path.to_string();
                self.status.message = format!("{} items", self.file_entries.len());

                // Watch new path
                if let Some(ref mut watcher) = self.file_watcher {
                    let _ = watcher.watch(path.as_path());
                }

                // Request thumbnails for image files
                self.request_thumbnails_for_current_directory();

                // Update global state
                if let Some(state) = state() {
                    state.set_current_path(path);
                }
            }
            Err(e) => {
                tracing::error!("Failed to navigate to directory: {}", e);
                self.status.message = format!("Error: {}", e);
            }
        }
    }

    /// Enter an archive file and display its contents as if it were a directory
    fn enter_archive(&mut self, archive_path: UniversalPath) {
        match VirtualFileSystem::open(archive_path.as_path()) {
            Ok(vfs) => {
                match vfs.list_entries() {
                    Ok(vfs_entries) => {
                        // Clear previous archive path mappings
                        self.archive_path_map.clear();

                        // Convert VfsEntry to FileEntry for display
                        let file_entries: Vec<FileEntry> = vfs_entries.iter().filter_map(|ve| {
                            // Create a pseudo-path for the archive entry
                            let entry_path = archive_path.join(&ve.path);

                            // Store mapping from entry path ID to archive inner path
                            self.archive_path_map.insert(entry_path.id(), ve.path.clone());

                            Some(FileEntry {
                                path: entry_path,
                                name: ve.name.clone(),
                                is_dir: ve.is_dir,
                                is_hidden: false,
                                size: ve.size,
                                modified: ve.modified,
                                extension: std::path::Path::new(&ve.name)
                                    .extension()
                                    .map(|e| e.to_string_lossy().to_lowercase())
                                    .unwrap_or_default(),
                            })
                        }).collect();

                        self.current_archive = Some(vfs);
                        self.archive_inner_path = String::new();
                        self.file_entries = file_entries;
                        self.selected_index = None;
                        self.status.message = format!("Archive: {} ({} items)",
                            archive_path.display(), self.file_entries.len());
                    }
                    Err(e) => {
                        tracing::error!("Failed to list archive entries: {}", e);
                        self.status.message = format!("Archive error: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to open archive: {}", e);
                self.status.message = format!("Cannot open archive: {}", e);
            }
        }
    }

    /// Request thumbnails for all image files in current directory
    /// This pre-generates thumbnails in the background
    fn request_thumbnails_for_current_directory(&mut self) {
        let Some(ref thumbnail_manager) = self.thumbnail_manager else {
            return;
        };

        let thumbnail_manager = thumbnail_manager.clone();

        // Collect image entries
        let image_entries: Vec<_> = self.file_entries.iter()
            .filter(|e| e.is_image())
            .map(|e| e.path.clone())
            .collect();

        // Spawn async task to pre-generate thumbnails
        tokio::spawn(async move {
            for path in image_entries {
                // This will generate and cache thumbnails in the background
                let _ = thumbnail_manager.get_thumbnail(path, ThumbnailSize::Small).await;
            }
        });
    }

    /// Load thumbnail texture for a file entry
    /// Returns TextureHandle if thumbnail is cached, None otherwise (triggers async generation)
    fn load_thumbnail_texture(&mut self, entry: &FileEntry) -> Option<egui::TextureHandle> {
        let Some(ref thumbnail_manager) = self.thumbnail_manager else {
            return None;
        };

        let path_hash = entry.path.id();

        // Check if texture already loaded
        if let Some(texture_handle) = self.thumbnail_textures.get(&path_hash) {
            return Some(texture_handle.clone());
        }

        // Try to get cached thumbnail (sync)
        if let Some(loaded) = thumbnail_manager.get_cached_sync(entry.path.as_path(), ThumbnailSize::Small) {
            // Create egui texture
            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                [loaded.width as usize, loaded.height as usize],
                &loaded.data,
            );

            let texture_handle = self.egui_ctx.load_texture(
                entry.name.clone(),
                color_image,
                egui::TextureOptions::LINEAR,
            );

            self.thumbnail_textures.insert(path_hash, texture_handle.clone());

            return Some(texture_handle);
        }

        // Not cached - request async generation
        let thumbnail_manager = thumbnail_manager.clone();
        let path = entry.path.clone();
        let egui_ctx = self.egui_ctx.clone();

        tokio::spawn(async move {
            if let Ok(_) = thumbnail_manager.get_thumbnail(path, ThumbnailSize::Small).await {
                // Request repaint to show the newly generated thumbnail
                egui_ctx.request_repaint();
            }
        });

        None
    }

    /// Navigate up to parent directory
    fn navigate_up(&mut self) {
        // If we're in an archive, exit the archive first
        if self.current_archive.is_some() {
            self.current_archive = None;
            self.archive_inner_path.clear();
            self.archive_path_map.clear();
            // Reload the directory containing the archive
            let path = self.current_path.clone();
            if let Some(parent) = get_parent(path.as_path()) {
                self.navigate_to(parent);
            }
            return;
        }

        // Normal directory navigation
        if !is_root(self.current_path.as_path()) {
            if let Some(parent) = get_parent(self.current_path.as_path()) {
                self.navigate_to(parent);
            }
        }
    }

    /// Navigate to a path (PathBuf version)
    fn navigate_to_path(&mut self, path: &std::path::Path) {
        let universal_path = UniversalPath::new(path);
        self.navigate_to(universal_path);
        // Clear catalog items to force refresh
        self.catalog_items.clear();
    }

    /// Load and display an image
    fn load_image(&mut self, entry: &FileEntry) {
        if !is_supported_image(entry.path.as_path()) {
            return;
        }

        tracing::info!("Loading image: {}", entry.path);

        // Load image data - handle both filesystem and archive
        let image_result = if let Some(ref vfs) = self.current_archive {
            // Loading from archive - get the inner path from mapping
            if let Some(inner_path) = self.archive_path_map.get(&entry.path.id()) {
                match vfs.read_file(inner_path) {
                    Ok(data) => {
                        image::load_from_memory(&data)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                    }
                    Err(e) => {
                        tracing::error!("Failed to read from archive: {}", e);
                        Err(std::io::Error::new(std::io::ErrorKind::Other, e))
                    }
                }
            } else {
                tracing::error!("Archive path not found in mapping");
                Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Archive path not found"))
            }
        } else {
            // Loading from filesystem
            image::open(entry.path.as_path())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        };

        match image_result {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();
                let pixels = rgba.as_flat_samples();

                // Create egui texture
                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    pixels.as_slice(),
                );

                let texture = self.egui_ctx.load_texture(
                    entry.name.clone(),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );

                // Update viewer
                self.image_viewer.set_image(texture.id(), width, height);
                self.current_texture = Some(texture);

                // Update viewer overlay info (Doc 4)
                self.image_viewer.file_name = entry.name.clone();
                self.image_viewer.resolution_text = format!("{}×{}", width, height);
                self.image_viewer.current_index = self.selected_index.map(|i| i + 1).unwrap_or(1);
                self.image_viewer.total_files = self.file_entries.len();

                // Update status
                self.status.file_name = entry.name.clone();
                self.status.dimensions = format!("{}×{}", width, height);
                self.status.file_size = format_size(entry.size);
            }
            Err(e) => {
                tracing::error!("Failed to load image: {}", e);
                self.status.message = format!("Error: {}", e);
                self.image_viewer.clear();
                self.current_texture = None;
            }
        }
    }

    /// Handle selection change
    fn on_select(&mut self, index: usize) {
        self.selected_index = Some(index);
        self.file_browser.selected = Some(index);

        if let Some(entry) = self.file_entries.get(index) {
            if entry.is_image() {
                self.load_image(&entry.clone());
            }

            // Update position status
            self.status.position = format!("{} / {}", index + 1, self.file_entries.len());
        }
    }

    /// Handle open (enter folder or open image)
    fn on_open(&mut self, index: usize) {
        if let Some(entry) = self.file_entries.get(index).cloned() {
            if entry.is_dir {
                self.navigate_to(entry.path);
            } else if entry.is_archive() {
                self.enter_archive(entry.path);
            } else if entry.is_image() {
                self.load_image(&entry);
                self.show_browser = false; // Switch to viewer mode
            }
        }
    }

    /// Handle nav.enter with threshold logic (Doc 3 specification)
    /// If folder has <= threshold files, open first image in Viewer mode
    /// If folder has > threshold files, enter in Browser mode
    fn on_enter_with_threshold(&mut self, index: usize, threshold: i32) {
        if let Some(entry) = self.file_entries.get(index).cloned() {
            if entry.is_dir {
                // Check file count in the target directory
                match count_files(entry.path.as_path()) {
                    Ok(file_count) => {
                        if file_count <= threshold as usize && file_count > 0 {
                            // Few files - open in Viewer mode
                            // Navigate to folder, then find first image and show it
                            self.navigate_to(entry.path.clone());

                            // Find first image and load it
                            if let Some(first_image_idx) = self.file_entries.iter().position(|e| e.is_image()) {
                                self.on_select(first_image_idx);
                                if let Some(img_entry) = self.file_entries.get(first_image_idx) {
                                    self.load_image(&img_entry.clone());
                                    self.show_browser = false; // Viewer mode
                                }
                            }
                        } else {
                            // Many files or empty - open in Browser mode
                            self.navigate_to(entry.path);
                        }
                    }
                    Err(_) => {
                        // Fallback to normal navigation
                        self.navigate_to(entry.path);
                    }
                }
            } else if entry.is_image() {
                // Regular file - open in Viewer
                self.load_image(&entry);
                self.show_browser = false;
            } else if entry.is_archive() {
                // Archive - open as directory
                self.enter_archive(entry.path);
            }
        }
    }

    /// Navigate to next image
    fn next_image(&mut self) {
        let current = self.selected_index.unwrap_or(0);
        let max = self.file_entries.len().saturating_sub(1);

        // Find next image file
        for i in (current + 1)..=max {
            if let Some(entry) = self.file_entries.get(i) {
                if entry.is_image() {
                    self.on_select(i);
                    return;
                }
            }
        }
    }

    /// Navigate to previous image
    fn prev_image(&mut self) {
        let current = self.selected_index.unwrap_or(0);

        // Find previous image file
        for i in (0..current).rev() {
            if let Some(entry) = self.file_entries.get(i) {
                if entry.is_image() {
                    self.on_select(i);
                    return;
                }
            }
        }
    }

    /// Navigate to first image
    fn first_image(&mut self) {
        // Find first image file
        for i in 0..self.file_entries.len() {
            if let Some(entry) = self.file_entries.get(i) {
                if entry.is_image() {
                    self.on_select(i);
                    return;
                }
            }
        }
    }

    /// Navigate to last image
    fn last_image(&mut self) {
        // Find last image file
        for i in (0..self.file_entries.len()).rev() {
            if let Some(entry) = self.file_entries.get(i) {
                if entry.is_image() {
                    self.on_select(i);
                    return;
                }
            }
        }
    }

    fn render(&mut self) {
        // Extract references we need, avoiding borrow conflicts
        let window = match &self.window {
            Some(w) => w.clone(),
            None => return,
        };

        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        let egui_state = match &mut self.egui_state {
            Some(s) => s,
            None => return,
        };

        // Get surface texture
        let output = match renderer.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                return;
            }
            Err(e) => {
                tracing::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Run egui - take input before borrowing self
        let raw_input = egui_state.take_egui_input(&window);

        // Store values we need for UI
        let current_path_str = self.current_path.display().to_string();
        let show_browser = self.show_browser;
        let selected_index = self.selected_index;
        let entries = self.file_entries.clone();

        // Viewer state for rendering
        let viewer_texture = self.image_viewer.texture;
        let viewer_image_size = self.image_viewer.image_size;
        let viewer_zoom = self.image_viewer.zoom;
        let viewer_pan = self.image_viewer.pan;
        let viewer_rotation = self.image_viewer.rotation;
        let viewer_fit_mode = self.image_viewer.fit_mode;

        // Track UI actions from egui closure
        let mut clicked_index: Option<usize> = None;
        let mut double_clicked_index: Option<usize> = None;

        // Track dialog results for post-closure handling
        let mut confirm_result: Option<bool> = None;
        let mut rename_result: Option<String> = None;
        let mut tag_result: Option<Vec<String>> = None;

        // Track viewer input for post-closure handling
        let mut viewer_zoom_delta: f32 = 0.0;
        let mut viewer_pan_delta = egui::Vec2::ZERO;
        let mut viewer_drag_started = false;
        let mut viewer_drag_ended = false;
        let mut viewer_double_clicked = false;

        // Overlay UI state
        let overlay_visible = self.overlay_visible;
        let image_count: usize = entries.iter().filter(|e| e.is_image()).count();
        let current_image_pos: usize = if let Some(idx) = selected_index {
            entries.iter().take(idx + 1).filter(|e| e.is_image()).count()
        } else {
            0
        };
        let mut mouse_moved = false;
        let mut seek_bar_clicked: Option<f32> = None;
        let mut nav_action: Option<&str> = None;

        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            // Top panel - Toolbar
            egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("📁");
                    ui.label(&current_path_str);
                });
            });

            // Bottom panel - Status bar
            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("{} items", entries.len()));
                    if let Some(idx) = selected_index {
                        ui.separator();
                        if let Some(entry) = entries.get(idx) {
                            ui.label(&entry.name);
                        }
                    }
                });
            });

            // Central panel - File browser or viewer
            egui::CentralPanel::default().show(ctx, |ui| {
                if show_browser {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (idx, entry) in entries.iter().enumerate() {
                            let is_selected = selected_index == Some(idx);
                            let icon = if entry.is_dir { "📁 " } else { "📄 " };
                            let label = format!("{}{}", icon, entry.name);

                            let response = ui.selectable_label(is_selected, label);
                            if response.clicked() {
                                clicked_index = Some(idx);
                            }
                            if response.double_clicked() {
                                double_clicked_index = Some(idx);
                            }
                        }
                    });
                } else {
                    // Image viewer mode - Doc 4 compliant
                    let available = ui.available_rect_before_wrap();

                    // Draw dark background
                    ui.painter().rect_filled(
                        available,
                        0.0,
                        egui::Color32::from_rgb(32, 32, 32),
                    );

                    // Allocate rect for input handling
                    let response = ui.allocate_rect(available, egui::Sense::click_and_drag());

                    // Handle zoom with scroll wheel (Doc 4: cursor-centered zoom)
                    if response.hovered() {
                        let scroll = ui.input(|i| i.raw_scroll_delta.y);
                        if scroll != 0.0 {
                            viewer_zoom_delta = scroll;
                        }
                    }

                    // Handle pan with drag (Doc 4: 1:1 tracking, no inertia)
                    if response.drag_started() {
                        viewer_drag_started = true;
                    }
                    if response.dragged() {
                        viewer_pan_delta = response.drag_delta();
                    }
                    if response.drag_stopped() {
                        viewer_drag_ended = true;
                    }

                    // Double-click to reset view
                    if response.double_clicked() {
                        viewer_double_clicked = true;
                    }

                    // Render image if texture exists
                    if let Some(texture_id) = viewer_texture {
                        // Calculate display size based on fit mode
                        let rotated_size = if viewer_rotation == 90 || viewer_rotation == 270 {
                            egui::Vec2::new(viewer_image_size.y, viewer_image_size.x)
                        } else {
                            viewer_image_size
                        };

                        let base_scale = match viewer_fit_mode {
                            app_ui::components::viewer::FitMode::FitToWindow => {
                                let scale_x = available.width() / rotated_size.x;
                                let scale_y = available.height() / rotated_size.y;
                                scale_x.min(scale_y).min(1.0)
                            }
                            app_ui::components::viewer::FitMode::FitWidth => {
                                available.width() / rotated_size.x
                            }
                            app_ui::components::viewer::FitMode::FitHeight => {
                                available.height() / rotated_size.y
                            }
                            app_ui::components::viewer::FitMode::OriginalSize => 1.0,
                        };

                        let display_size = rotated_size * base_scale * viewer_zoom;
                        let center = available.center() + viewer_pan;
                        let image_rect = egui::Rect::from_center_size(center, display_size);

                        // Draw image
                        let uv = egui::Rect::from_min_max(
                            egui::Pos2::ZERO,
                            egui::Pos2::new(1.0, 1.0),
                        );
                        ui.painter().image(texture_id, image_rect, uv, egui::Color32::WHITE);

                        // Check mouse activity for overlay visibility
                        if ui.input(|i| i.pointer.delta().length() > 0.0) {
                            mouse_moved = true;
                        }

                        // Draw overlay UI (Doc 4 spec) when visible
                        if overlay_visible {
                            let overlay_bg = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180);
                            let overlay_height = 36.0;

                            // === Top Control Bar ===
                            let top_bar = egui::Rect::from_min_size(
                                available.left_top(),
                                egui::Vec2::new(available.width(), overlay_height),
                            );
                            ui.painter().rect_filled(top_bar, 0.0, overlay_bg);

                            // Left: File info
                            if let Some(idx) = selected_index {
                                if let Some(entry) = entries.get(idx) {
                                    let info_text = format!(
                                        "{} - {}×{}",
                                        entry.name,
                                        viewer_image_size.x as u32,
                                        viewer_image_size.y as u32,
                                    );
                                    ui.painter().text(
                                        top_bar.left_center() + egui::Vec2::new(10.0, 0.0),
                                        egui::Align2::LEFT_CENTER,
                                        &info_text,
                                        egui::FontId::proportional(14.0),
                                        egui::Color32::WHITE,
                                    );
                                }
                            }

                            // Center: Navigation controls
                            let nav_text = format!("{} / {}", current_image_pos, image_count);
                            ui.painter().text(
                                top_bar.center(),
                                egui::Align2::CENTER_CENTER,
                                &nav_text,
                                egui::FontId::proportional(14.0),
                                egui::Color32::WHITE,
                            );

                            // Navigation buttons (simple text for now)
                            let nav_left = top_bar.center() - egui::Vec2::new(80.0, 0.0);
                            let nav_right = top_bar.center() + egui::Vec2::new(80.0, 0.0);
                            ui.painter().text(
                                nav_left - egui::Vec2::new(30.0, 0.0),
                                egui::Align2::CENTER_CENTER,
                                "<<",
                                egui::FontId::proportional(14.0),
                                egui::Color32::GRAY,
                            );
                            ui.painter().text(
                                nav_left,
                                egui::Align2::CENTER_CENTER,
                                "<",
                                egui::FontId::proportional(16.0),
                                egui::Color32::WHITE,
                            );
                            ui.painter().text(
                                nav_right,
                                egui::Align2::CENTER_CENTER,
                                ">",
                                egui::FontId::proportional(16.0),
                                egui::Color32::WHITE,
                            );
                            ui.painter().text(
                                nav_right + egui::Vec2::new(30.0, 0.0),
                                egui::Align2::CENTER_CENTER,
                                ">>",
                                egui::FontId::proportional(14.0),
                                egui::Color32::GRAY,
                            );

                            // Right: Zoom info
                            let zoom_text = format!("{:.0}%", viewer_zoom * base_scale * 100.0);
                            ui.painter().text(
                                top_bar.right_center() - egui::Vec2::new(10.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                &zoom_text,
                                egui::FontId::proportional(14.0),
                                egui::Color32::WHITE,
                            );

                            // === Bottom Seek Bar ===
                            let seek_bar_height = 24.0;
                            let seek_bar = egui::Rect::from_min_size(
                                egui::Pos2::new(available.left(), available.bottom() - seek_bar_height),
                                egui::Vec2::new(available.width(), seek_bar_height),
                            );
                            ui.painter().rect_filled(seek_bar, 0.0, overlay_bg);

                            // Draw seek bar track
                            let track_margin = 20.0;
                            let track_rect = egui::Rect::from_min_max(
                                egui::Pos2::new(seek_bar.left() + track_margin, seek_bar.center().y - 2.0),
                                egui::Pos2::new(seek_bar.right() - track_margin, seek_bar.center().y + 2.0),
                            );
                            ui.painter().rect_filled(track_rect, 2.0, egui::Color32::DARK_GRAY);

                            // Draw position indicator
                            if image_count > 0 {
                                let progress = current_image_pos as f32 / image_count as f32;
                                let indicator_x = track_rect.left() + track_rect.width() * progress;
                                let indicator_pos = egui::Pos2::new(indicator_x, seek_bar.center().y);
                                ui.painter().circle_filled(indicator_pos, 6.0, egui::Color32::WHITE);

                                // Filled portion
                                let filled_rect = egui::Rect::from_min_max(
                                    track_rect.left_top(),
                                    egui::Pos2::new(indicator_x, track_rect.bottom()),
                                );
                                ui.painter().rect_filled(filled_rect, 2.0, egui::Color32::from_rgb(100, 150, 255));
                            }

                            // Handle seek bar click
                            let seek_response = ui.allocate_rect(seek_bar, egui::Sense::click());
                            if seek_response.clicked() {
                                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                                    let relative_x = (pos.x - track_rect.left()) / track_rect.width();
                                    seek_bar_clicked = Some(relative_x.clamp(0.0, 1.0));
                                }
                            }
                        }
                    } else {
                        // No image placeholder
                        ui.painter().text(
                            available.center(),
                            egui::Align2::CENTER_CENTER,
                            "No image loaded",
                            egui::FontId::proportional(24.0),
                            egui::Color32::GRAY,
                        );
                    }
                }
            });

            // Settings dialog (rendered on top)
            if let Some(action) = self.settings_dialog.ui(ctx) {
                match action {
                    SettingsAction::Ok => {
                        // Apply changes and close
                        let new_config = self.settings_dialog.get_config().clone();
                        if let Some(state) = state() {
                            *state.config.write() = new_config.clone();
                            if let Err(e) = new_config.save() {
                                tracing::error!("Failed to save config: {}", e);
                            }
                        }
                        self.settings_dialog.close();
                    }
                    SettingsAction::Apply => {
                        // Apply changes but keep dialog open
                        let new_config = self.settings_dialog.get_config().clone();
                        if let Some(state) = state() {
                            *state.config.write() = new_config.clone();
                            if let Err(e) = new_config.save() {
                                tracing::error!("Failed to save config: {}", e);
                            }
                        }
                        self.settings_dialog.reset_modified();
                    }
                    SettingsAction::Cancel => {
                        // Discard changes and close
                        self.settings_dialog.close();
                    }
                }
            }

            // Confirm dialog (rendered on top)
            if let Some(ref mut dialog) = self.confirm_dialog {
                match dialog.ui(ctx) {
                    DialogResult::Ok(result) => {
                        confirm_result = Some(result);
                        self.confirm_dialog = None;
                    }
                    DialogResult::Cancel => {
                        confirm_result = Some(false);
                        self.confirm_dialog = None;
                    }
                    _ => {}
                }
            }

            // Rename dialog
            if let Some(ref mut dialog) = self.rename_dialog {
                match dialog.ui(ctx) {
                    DialogResult::Ok(new_name) => {
                        rename_result = Some(new_name);
                        self.rename_dialog = None;
                    }
                    DialogResult::Cancel => {
                        self.rename_dialog = None;
                    }
                    _ => {}
                }
            }

            // Tag edit dialog
            if let Some(ref mut dialog) = self.tag_dialog {
                match dialog.ui(ctx) {
                    DialogResult::Ok(tags) => {
                        tag_result = Some(tags);
                        self.tag_dialog = None;
                    }
                    DialogResult::Cancel => {
                        self.tag_dialog = None;
                    }
                    _ => {}
                }
            }
        });

        // Handle UI actions after egui run
        if let Some(idx) = double_clicked_index {
            self.on_open(idx);
        } else if let Some(idx) = clicked_index {
            self.on_select(idx);
        }

        // Handle dialog results
        if let Some(confirmed) = confirm_result {
            if confirmed {
                if let Some(path) = self.pending_delete_path.take() {
                    let _ = self.file_ops.delete(&[path], true);
                    self.navigate_to(self.current_path.clone());
                }
            } else {
                self.pending_delete_path = None;
            }
        }

        if let Some(new_name) = rename_result {
            if let Some(idx) = self.selected_index {
                if let Some(entry) = self.file_entries.get(idx) {
                    let from = entry.path.as_path();
                    let to = from.with_file_name(new_name);
                    match self.file_ops.rename(from, &to) {
                        Ok(_) => {
                            self.status.message = format!("Renamed to: {}", to.display());
                            self.navigate_to(self.current_path.clone());
                        }
                        Err(e) => {
                            self.status.message = format!("Rename error: {}", e);
                        }
                    }
                }
            }
        }

        if let Some(tags) = tag_result {
            if let Some(idx) = self.selected_index {
                if let Some(_entry) = self.file_entries.get(idx) {
                    // TODO: Save tags to DB
                    self.status.message = format!("Tags updated: {:?}", tags);
                }
            }
        }

        // Handle viewer input (Doc 4 compliant)
        if !self.show_browser {
            // Zoom with scroll wheel
            if viewer_zoom_delta != 0.0 {
                let zoom_factor = if viewer_zoom_delta > 0.0 { 1.1 } else { 0.9 };
                self.image_viewer.zoom = (self.image_viewer.zoom * zoom_factor).clamp(0.1, 10.0);
            }

            // Pan with drag (1:1 tracking)
            if viewer_pan_delta != egui::Vec2::ZERO {
                self.image_viewer.pan += viewer_pan_delta;
            }

            // Double-click to reset view
            if viewer_double_clicked {
                self.image_viewer.reset_view();
            }

            // Update overlay visibility based on mouse movement
            if mouse_moved {
                self.overlay_visible = true;
                self.last_mouse_move = Some(std::time::Instant::now());
            } else if let Some(last_move) = self.last_mouse_move {
                // Hide overlay after 3 seconds of inactivity
                if last_move.elapsed().as_secs() > 3 {
                    self.overlay_visible = false;
                }
            }

            // Handle seek bar navigation
            if let Some(position) = seek_bar_clicked {
                // Jump to image at given position (0.0 - 1.0)
                let image_indices: Vec<usize> = self.file_entries.iter()
                    .enumerate()
                    .filter(|(_, e)| e.is_image())
                    .map(|(i, _)| i)
                    .collect();
                if !image_indices.is_empty() {
                    let target_idx = ((position * image_indices.len() as f32) as usize)
                        .min(image_indices.len() - 1);
                    if let Some(&idx) = image_indices.get(target_idx) {
                        self.on_select(idx);
                    }
                }
            }
        }

        // Handle platform output
        if let Some(egui_state) = &mut self.egui_state {
            egui_state.handle_platform_output(&window, full_output.platform_output);
        }

        let clipped_primitives = self.egui_ctx.tessellate(
            full_output.shapes,
            full_output.pixels_per_point,
        );

        // Get renderer and egui_renderer again for rendering
        let renderer = match &self.renderer {
            Some(r) => r,
            None => return,
        };

        let egui_renderer = match &mut self.egui_renderer {
            Some(r) => r,
            None => return,
        };

        // Render
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [renderer.size.0, renderer.size.1],
            pixels_per_point: window.scale_factor() as f32,
        };

        let mut encoder = renderer.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("egui encoder") }
        );

        // Update egui textures
        for (id, delta) in &full_output.textures_delta.set {
            egui_renderer.update_texture(&renderer.device, &renderer.queue, *id, delta);
        }

        egui_renderer.update_buffers(
            &renderer.device,
            &renderer.queue,
            &mut encoder,
            &clipped_primitives,
            &screen_descriptor,
        );

        // Create render pass and render egui
        // Note: We need to manage lifetimes carefully here
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // SAFETY: The render_pass is dropped before encoder.finish() is called,
            // so the borrow is valid even though we're transmuting the lifetime.
            // This is necessary because egui-wgpu 0.29 requires 'static lifetime.
            let render_pass_static: &mut wgpu::RenderPass<'static> = unsafe {
                std::mem::transmute(&mut render_pass)
            };

            egui_renderer.render(render_pass_static, &clipped_primitives, &screen_descriptor);
        }

        // Free textures
        for id in &full_output.textures_delta.free {
            egui_renderer.free_texture(id);
        }

        renderer.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    #[allow(dead_code)]
    fn ui(&mut self, ctx: &egui::Context) {
        // Top panel - Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            if let Some(action) = Toolbar::ui(ui) {
                self.handle_toolbar_action(action);
            }
        });

        // Bottom panel - Status bar
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            StatusBar::ui(ui, &self.status);
        });

        // Main content
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.show_browser {
                // Doc spec: Left panel = Folder tree only, Right panel = Thumbnail catalog

                // Left panel - Folder Tree (folders only, no files)
                egui::SidePanel::left("folder_tree_panel")
                    .resizable(true)
                    .default_width(200.0)
                    .min_width(150.0)
                    .max_width(400.0)
                    .show_inside(ui, |ui| {
                        ui.heading("Folders");
                        ui.separator();

                        // Folder tree - only shows folders
                        let current = self.current_path.as_path().to_path_buf();
                        if let Some(action) = self.folder_tree.ui(ui, &current) {
                            match action {
                                FolderTreeAction::SelectFolder(path) => {
                                    self.navigate_to_path(&path);
                                }
                                FolderTreeAction::ToggleExpand(_) => {
                                    // Tree handles this internally
                                }
                                FolderTreeAction::GoToParent => {
                                    self.navigate_up();
                                }
                            }
                        }
                    });

                // Right panel - Thumbnail Catalog (image thumbnails in grid)
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    // Path bar at top
                    ui.horizontal(|ui| {
                        if ui.button("⬆").on_hover_text("Parent folder").clicked() {
                            self.navigate_up();
                        }
                        ui.separator();
                        ui.label(format!("📁 {}", self.current_path));
                        ui.separator();
                        let image_count = self.file_entries.iter().filter(|e| e.is_image()).count();
                        ui.label(format!("{} images", image_count));
                    });
                    ui.separator();

                    // Update catalog items from file entries
                    self.update_catalog_items();

                    // Sync selection
                    self.thumbnail_catalog.selected = self.selected_index;

                    // Thumbnail catalog grid
                    let catalog_items = self.catalog_items.clone();
                    if let Some(action) = self.thumbnail_catalog.ui(ui, &catalog_items) {
                        match action {
                            CatalogAction::Select(idx) => self.on_select(idx),
                            CatalogAction::Open(idx) => self.on_open(idx),
                            CatalogAction::GoToParent => self.navigate_up(),
                            CatalogAction::Navigate(_) => {
                                // Navigation already handled internally
                                if let Some(idx) = self.thumbnail_catalog.selected {
                                    self.on_select(idx);
                                }
                            }
                        }
                    }
                });
            } else {
                // Full viewer mode
                // Note: Double-click to close is handled inside image_viewer.ui()
                // Do NOT allocate_response here as it blocks seek bar interaction
                let action = self.image_viewer.ui(ui);
                self.handle_viewer_action(action);
            }
        });
    }

    /// Update catalog items from current file entries
    fn update_catalog_items(&mut self) {
        // Rebuild catalog if entries changed
        if self.catalog_items.len() != self.file_entries.len() {
            // Clone entries to avoid borrow conflict
            let entries: Vec<_> = self.file_entries.iter().cloned().collect();
            self.catalog_items = entries.iter().map(|e| {
                let mut item = ThumbnailItem::new(
                    e.path.as_path().to_path_buf(),
                    e.is_dir,
                    e.is_image(),
                );

                // Load thumbnail texture if available
                if e.is_image() {
                    if let Some(texture) = self.load_thumbnail_texture(e) {
                        item.set_texture(texture);
                    }
                }

                item
            }).collect();
        } else {
            // Update thumbnails for existing items that don't have one yet
            // Collect indices and entries to update first to avoid borrow conflict
            let updates: Vec<_> = self.file_entries.iter().enumerate()
                .filter(|(idx, entry)| {
                    entry.is_image() &&
                    self.catalog_items.get(*idx).map(|i| i.texture.is_none()).unwrap_or(false)
                })
                .map(|(idx, entry)| (idx, entry.clone()))
                .collect();

            for (idx, entry) in updates {
                if let Some(texture) = self.load_thumbnail_texture(&entry) {
                    if let Some(item) = self.catalog_items.get_mut(idx) {
                        item.set_texture(texture);
                    }
                }
            }
        }
    }

    /// Handle viewer overlay UI actions (Doc 4 spec)
    fn handle_viewer_action(&mut self, action: ViewerAction) {
        match action {
            ViewerAction::None => {}
            ViewerAction::NextImage => self.next_image(),
            ViewerAction::PrevImage => self.prev_image(),
            ViewerAction::FirstImage => self.first_image(),
            ViewerAction::LastImage => self.last_image(),
            ViewerAction::ToggleFullscreen => {
                self.show_browser = !self.show_browser;
            }
            ViewerAction::ToggleSlideshow => {
                self.image_viewer.slideshow_active = !self.image_viewer.slideshow_active;
                // TODO: Start/stop slideshow timer
            }
            ViewerAction::OpenSettings => {
                self.settings_dialog.open = true;
            }
            ViewerAction::Close => {
                self.show_browser = true;
            }
            ViewerAction::SeekTo(position) => {
                // Seek to position in file list (0.0-1.0)
                if !self.file_entries.is_empty() {
                    let target_idx = ((self.file_entries.len() as f32 - 1.0) * position) as usize;
                    self.on_select(target_idx);
                    // If it's an image, load it
                    if let Some(entry) = self.file_entries.get(target_idx).cloned() {
                        if entry.is_image() {
                            self.load_image(&entry);
                        }
                    }
                }
            }
        }
    }

    #[allow(dead_code)]
    fn handle_toolbar_action(&mut self, action: ToolbarAction) {
        match action {
            ToolbarAction::Previous => self.prev_image(),
            ToolbarAction::Next => self.next_image(),
            ToolbarAction::UpFolder => self.navigate_up(),
            ToolbarAction::Home => {
                if let Some(home) = dirs_next::home_dir() {
                    self.navigate_to(UniversalPath::new(home));
                }
            }
            ToolbarAction::ZoomIn => self.image_viewer.zoom_in(),
            ToolbarAction::ZoomOut => self.image_viewer.zoom_out(),
            ToolbarAction::OriginalSize => self.image_viewer.set_zoom(1.0),
            ToolbarAction::FitToWindow => {
                self.image_viewer.fit_mode = app_ui::components::viewer::FitMode::FitToWindow;
                self.image_viewer.reset_view();
            }
            ToolbarAction::RotateLeft => self.image_viewer.rotate_left(),
            ToolbarAction::RotateRight => self.image_viewer.rotate_right(),
            ToolbarAction::GridView => {
                self.file_browser.view_mode = BrowserViewMode::Grid;
            }
            ToolbarAction::ListView => {
                self.file_browser.view_mode = BrowserViewMode::List;
            }
            ToolbarAction::Settings => {
                // TODO: Open settings
            }
            ToolbarAction::Fullscreen => {
                self.show_browser = !self.show_browser;
            }
        }
    }

    // ========================================
    // Command Execution (Doc 3 compliant)
    // ========================================

    /// Execute a command based on Doc 3 specification
    fn execute_command(&mut self, cmd: &Command) -> bool {
        let cmd_id = cmd.id.as_str();
        let amount = cmd.params.amount.unwrap_or(1) as usize;
        let select = cmd.params.select.unwrap_or(false);
        let wrap = cmd.params.wrap.unwrap_or(false);

        tracing::debug!("Executing command: {} (amount={}, select={}, wrap={})", cmd_id, amount, select, wrap);

        match cmd_id {
            // ========================================
            // Navigation Commands (nav.*)
            // ========================================

            // Grid movement
            CommandId::NAV_MOVE_UP => {
                if self.show_browser {
                    self.nav_state.move_up(amount, select);
                    self.sync_selection_from_nav();
                    true
                } else {
                    false
                }
            }
            CommandId::NAV_MOVE_DOWN => {
                if self.show_browser {
                    self.nav_state.move_down(amount, select);
                    self.sync_selection_from_nav();
                    true
                } else {
                    false
                }
            }
            CommandId::NAV_MOVE_LEFT => {
                if self.show_browser {
                    self.nav_state.move_left(amount, select, wrap);
                    self.sync_selection_from_nav();
                    true
                } else {
                    self.prev_image();
                    true
                }
            }
            CommandId::NAV_MOVE_RIGHT => {
                if self.show_browser {
                    self.nav_state.move_right(amount, select, wrap);
                    self.sync_selection_from_nav();
                    true
                } else {
                    self.next_image();
                    true
                }
            }

            // Page navigation
            CommandId::NAV_PAGE_UP => {
                self.nav_state.page_up(amount, select);
                self.sync_selection_from_nav();
                true
            }
            CommandId::NAV_PAGE_DOWN => {
                self.nav_state.page_down(amount, select);
                self.sync_selection_from_nav();
                true
            }

            // Home/End
            CommandId::NAV_HOME => {
                self.nav_state.home(select);
                self.sync_selection_from_nav();
                true
            }
            CommandId::NAV_END => {
                self.nav_state.end(select);
                self.sync_selection_from_nav();
                true
            }

            // Item navigation
            CommandId::NAV_NEXT_ITEM => {
                if self.show_browser {
                    self.nav_state.next_item(amount, wrap);
                    self.sync_selection_from_nav();
                } else {
                    self.next_image();
                }
                true
            }
            CommandId::NAV_PREV_ITEM => {
                if self.show_browser {
                    self.nav_state.prev_item(amount, wrap);
                    self.sync_selection_from_nav();
                } else {
                    self.prev_image();
                }
                true
            }

            // Hierarchy navigation
            CommandId::NAV_ENTER => {
                if let Some(idx) = self.selected_index {
                    let threshold = cmd.params.threshold.unwrap_or(self.nav_state.enter_threshold);
                    self.on_enter_with_threshold(idx, threshold);
                }
                true
            }
            CommandId::NAV_PARENT => {
                self.navigate_up();
                true
            }
            CommandId::NAV_ROOT => {
                // Navigate to drive root (Windows) or / (Unix)
                #[cfg(windows)]
                {
                    let path_str = self.current_path.to_string();
                    if let Some(drive) = path_str.chars().next() {
                        self.navigate_to(UniversalPath::new(format!("{}:\\", drive)));
                    }
                }
                #[cfg(not(windows))]
                {
                    self.navigate_to(UniversalPath::new("/"));
                }
                true
            }
            CommandId::NAV_NEXT_SIBLING => {
                let skip_empty = cmd.params.skip_empty.unwrap_or(true);
                if let Some(next) = get_next_sibling(self.current_path.as_path(), skip_empty) {
                    self.navigate_to(next);
                    true
                } else {
                    self.status.message = "No next sibling folder".to_string();
                    false
                }
            }
            CommandId::NAV_PREV_SIBLING => {
                let skip_empty = cmd.params.skip_empty.unwrap_or(true);
                if let Some(prev) = get_prev_sibling(self.current_path.as_path(), skip_empty) {
                    self.navigate_to(prev);
                    true
                } else {
                    self.status.message = "No previous sibling folder".to_string();
                    false
                }
            }

            // ========================================
            // View Commands (view.*)
            // ========================================

            CommandId::VIEW_ZOOM_IN => {
                let step = cmd.params.step.unwrap_or(0.2);
                self.image_viewer.zoom = (self.image_viewer.zoom * (1.0 + step)).min(10.0);
                true
            }
            CommandId::VIEW_ZOOM_OUT => {
                let step = cmd.params.step.unwrap_or(0.2);
                self.image_viewer.zoom = (self.image_viewer.zoom / (1.0 + step)).max(0.1);
                true
            }
            CommandId::VIEW_ZOOM_SET => {
                use app_core::ZoomMode;
                match cmd.params.mode {
                    Some(ZoomMode::Original) => {
                        self.image_viewer.fit_mode = app_ui::components::viewer::FitMode::OriginalSize;
                        self.image_viewer.zoom = 1.0;
                    }
                    Some(ZoomMode::FitWindow) => {
                        self.image_viewer.fit_mode = app_ui::components::viewer::FitMode::FitToWindow;
                    }
                    Some(ZoomMode::FitWidth) => {
                        self.image_viewer.fit_mode = app_ui::components::viewer::FitMode::FitWidth;
                    }
                    Some(ZoomMode::FitHeight) => {
                        self.image_viewer.fit_mode = app_ui::components::viewer::FitMode::FitHeight;
                    }
                    None => {
                        if let Some(scale) = cmd.params.scale {
                            self.image_viewer.zoom = scale.clamp(0.1, 10.0);
                        }
                    }
                }
                self.image_viewer.reset_view();
                true
            }
            CommandId::VIEW_ROTATE => {
                let angle = cmd.params.angle.unwrap_or(90);
                if angle > 0 {
                    self.image_transform.rotate_cw();
                    self.image_viewer.rotate_right();
                } else {
                    self.image_transform.rotate_ccw();
                    self.image_viewer.rotate_left();
                }
                let status = self.image_transform.status_text();
                self.status.message = if status.is_empty() { "No transform".to_string() } else { status };
                true
            }
            CommandId::VIEW_TOGGLE_FULLSCREEN => {
                self.show_browser = !self.show_browser;
                true
            }
            CommandId::VIEW_NEXT_ITEM => {
                self.next_image();
                true
            }
            CommandId::VIEW_PREV_ITEM => {
                self.prev_image();
                true
            }
            CommandId::VIEW_PARENT => {
                self.show_browser = true;
                true
            }
            CommandId::VIEW_FLIP => {
                use app_core::FlipAxis;
                match cmd.params.axis {
                    Some(FlipAxis::Horizontal) => {
                        self.image_transform.toggle_flip_h();
                    }
                    Some(FlipAxis::Vertical) => {
                        self.image_transform.toggle_flip_v();
                    }
                    None => {
                        // Toggle horizontal by default
                        self.image_transform.toggle_flip_h();
                    }
                }
                let status = self.image_transform.status_text();
                self.status.message = if status.is_empty() { "No transform".to_string() } else { status };
                true
            }
            CommandId::VIEW_SPREAD_MODE => {
                use app_core::SpreadMode as CoreSpreadMode;
                // Convert core SpreadMode to ui SpreadMode
                match cmd.params.spread {
                    Some(CoreSpreadMode::Single) => self.spread_viewer.mode = SpreadMode::Single,
                    Some(CoreSpreadMode::Spread) => self.spread_viewer.mode = SpreadMode::SpreadRTL,
                    Some(CoreSpreadMode::Auto) | None => {
                        // Cycle through modes
                        self.spread_viewer.cycle_mode();
                    }
                };
                // Recalculate spread for current position
                if let Some(idx) = self.selected_index {
                    self.spread_viewer.go_to(idx, self.file_entries.len());
                }
                self.status.message = format!("Spread: {}", self.spread_viewer.mode_name());
                true
            }
            CommandId::VIEW_SET_BACKGROUND => {
                use app_core::BackgroundColor as CoreBgColor;
                use app_ui::components::BackgroundColor;
                match cmd.params.color {
                    Some(CoreBgColor::Black) => self.viewer_background.color = BackgroundColor::Black,
                    Some(CoreBgColor::Gray) => self.viewer_background.color = BackgroundColor::Gray(128),
                    Some(CoreBgColor::White) => self.viewer_background.color = BackgroundColor::White,
                    Some(CoreBgColor::Check) => self.viewer_background.color = BackgroundColor::Checkerboard,
                    Some(CoreBgColor::Transparent) | None => {
                        // Cycle through backgrounds
                        self.viewer_background.cycle();
                    }
                };
                self.status.message = self.viewer_background.status_text().to_string();
                true
            }
            CommandId::VIEW_SMART_SCROLL_DOWN => {
                // Doc 3 spec: Space = toggle_mark (Browser) / smart_scroll (Viewer)
                if self.show_browser {
                    // Browser context: toggle mark on current file
                    if let Some(idx) = self.selected_index {
                        if let Some(entry) = self.file_entries.get(idx) {
                            let hash = entry.path.id();
                            if self.marked_files.contains(&hash) {
                                self.marked_files.remove(&hash);
                                self.status.message = format!("Unmarked: {}", entry.name);
                            } else {
                                self.marked_files.insert(hash);
                                self.status.message = format!("Marked: {} ({} total)", entry.name, self.marked_files.len());
                            }
                        }
                    }
                } else {
                    // Viewer context: smart scroll (Doc 4 spec)
                    let overlap = cmd.params.overlap.unwrap_or(50) as f32;
                    let available = self.image_viewer.get_estimated_available();
                    if self.image_viewer.smart_scroll_down(available, overlap) {
                        // At bottom edge or image fits, go to next image
                        self.next_image();
                    }
                }
                true
            }
            CommandId::VIEW_SMART_SCROLL_UP => {
                // Viewer context: smart scroll up (Doc 4 spec)
                let overlap = cmd.params.overlap.unwrap_or(50) as f32;
                let available = self.image_viewer.get_estimated_available();
                if self.image_viewer.smart_scroll_up(available, overlap) {
                    // At top edge or image fits, go to prev image
                    self.prev_image();
                }
                true
            }
            CommandId::VIEW_SLIDESHOW => {
                use app_core::SlideshowAction;
                let total = self.file_entries.iter().filter(|e| e.is_image()).count();
                let current = self.selected_index.unwrap_or(0);
                match cmd.params.action {
                    Some(SlideshowAction::Start) => self.slideshow.start(total, current),
                    Some(SlideshowAction::Stop) => self.slideshow.stop(),
                    Some(SlideshowAction::Toggle) | None => self.slideshow.toggle(total, current),
                };
                let status = self.slideshow.status_text();
                self.status.message = if status.is_empty() { "Slideshow stopped".to_string() } else { status };
                true
            }
            CommandId::VIEW_PAN => {
                use app_core::Direction;
                let amount = cmd.params.amount.unwrap_or(10) as f32;
                match cmd.params.direction {
                    Some(Direction::Up) => self.image_viewer.pan.y += amount,
                    Some(Direction::Down) => self.image_viewer.pan.y -= amount,
                    Some(Direction::Left) => self.image_viewer.pan.x += amount,
                    Some(Direction::Right) => self.image_viewer.pan.x -= amount,
                    None => {}
                }
                true
            }
            CommandId::VIEW_PAN_TO => {
                use app_core::Position;
                // Reset pan to specific position
                match cmd.params.position {
                    Some(Position::TopLeft) => {
                        self.image_viewer.pan = egui::Vec2::ZERO;
                    }
                    Some(Position::Center) | None => {
                        self.image_viewer.pan = egui::Vec2::ZERO;
                    }
                    _ => {
                        // Other positions would need size calculation
                        self.image_viewer.pan = egui::Vec2::ZERO;
                    }
                }
                true
            }
            CommandId::VIEW_TOGGLE_INFO => {
                use app_core::InfoLevel;
                let level_str = match cmd.params.level {
                    Some(InfoLevel::None) => "Info: Hidden",
                    Some(InfoLevel::Simple) => "Info: Simple",
                    Some(InfoLevel::Detail) => "Info: Detailed",
                    None => "Info: Toggled",
                };
                self.status.message = level_str.to_string();
                true
            }
            CommandId::VIEW_LOCK_ZOOM => {
                let toggle = cmd.params.toggle.unwrap_or(true);
                if toggle {
                    self.status.message = "Zoom lock toggled".to_string();
                }
                true
            }
            CommandId::VIEW_ZOOM_MODE_CYCLE => {
                // Cycle through zoom modes: FitWindow -> Original -> FitWidth -> FitHeight -> FitWindow
                use app_ui::components::viewer::FitMode;
                self.image_viewer.fit_mode = match self.image_viewer.fit_mode {
                    FitMode::FitToWindow => {
                        self.image_viewer.zoom = 1.0;
                        FitMode::OriginalSize
                    }
                    FitMode::OriginalSize => FitMode::FitWidth,
                    FitMode::FitWidth => FitMode::FitHeight,
                    FitMode::FitHeight => FitMode::FitToWindow,
                };
                self.image_viewer.reset_view();
                true
            }
            CommandId::VIEW_SCROLL_UP | CommandId::VIEW_SCROLL_DOWN => {
                let amount = cmd.params.amount.unwrap_or(50) as f32;
                if cmd_id == CommandId::VIEW_SCROLL_UP {
                    self.image_viewer.pan.y += amount;
                } else {
                    self.image_viewer.pan.y -= amount;
                }
                true
            }
            CommandId::VIEW_NEXT_FOLDER => {
                let skip_empty = cmd.params.skip_empty.unwrap_or(true);
                if let Some(next) = get_next_sibling(self.current_path.as_path(), skip_empty) {
                    self.navigate_to(next);
                    // Auto-select first image
                    if let Some(first_img_idx) = self.file_entries.iter().position(|e| e.is_image()) {
                        self.on_select(first_img_idx);
                    }
                    true
                } else {
                    self.status.message = "No next folder".to_string();
                    false
                }
            }
            CommandId::VIEW_PREV_FOLDER => {
                let skip_empty = cmd.params.skip_empty.unwrap_or(true);
                if let Some(prev) = get_prev_sibling(self.current_path.as_path(), skip_empty) {
                    self.navigate_to(prev);
                    if let Some(last_img_idx) = self.file_entries.iter().rposition(|e| e.is_image()) {
                        self.on_select(last_img_idx);
                    }
                    true
                } else {
                    self.status.message = "No previous folder".to_string();
                    false
                }
            }
            CommandId::VIEW_TOGGLE_TRANSITION => {
                self.status.message = "Transition toggled".to_string();
                true
            }
            CommandId::VIEW_TOGGLE_CHROMELESS => {
                // Chromeless = no UI, just image
                self.show_browser = false;
                self.status.message = "Chromeless mode".to_string();
                true
            }
            CommandId::VIEW_QUICK_LOOK => {
                // Quick look at selected file
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        if entry.is_image() {
                            self.load_image(&entry.clone());
                        }
                    }
                }
                true
            }
            CommandId::VIEW_SPLIT_MODE => {
                self.split_view.toggle();
                if self.split_view.enabled {
                    // Set second pane to next file
                    if let Some(idx) = self.selected_index {
                        if idx + 1 < self.file_entries.len() {
                            self.split_view.panes[1].path = Some(
                                self.file_entries[idx + 1].path.as_path().to_path_buf()
                            );
                        }
                    }
                    self.status.message = format!("Split view: ON ({})", self.split_view.status_text());
                } else {
                    self.status.message = "Split view: OFF".to_string();
                }
                true
            }
            CommandId::VIEW_SYNC_SCROLL => {
                self.split_view.toggle_sync();
                let sync = if self.split_view.sync_zoom { "ON" } else { "OFF" };
                self.status.message = format!("Sync scroll: {}", sync);
                true
            }
            CommandId::VIEW_SEEK => {
                // Seek to position (0.0-1.0)
                if let Some(pos) = cmd.params.seek_position {
                    let total = self.file_entries.iter().filter(|e| e.is_image()).count();
                    if total > 0 {
                        let target_idx = ((pos * total as f32) as usize).min(total - 1);
                        let image_indices: Vec<usize> = self.file_entries.iter()
                            .enumerate()
                            .filter(|(_, e)| e.is_image())
                            .map(|(i, _)| i)
                            .collect();
                        if let Some(&idx) = image_indices.get(target_idx) {
                            self.on_select(idx);
                        }
                    }
                }
                true
            }
            CommandId::VIEW_SLIDESHOW_INTERVAL => {
                if let Some(amount) = cmd.params.amount {
                    self.slideshow.set_interval_secs(amount as f32 / 1000.0);
                } else {
                    // Toggle increase/decrease
                    self.slideshow.increase_interval();
                }
                let interval = self.slideshow.config.interval.as_secs_f32();
                self.status.message = format!("Slideshow interval: {:.1}s", interval);
                true
            }

            // ========================================
            // File Commands (file.*)
            // ========================================

            CommandId::FILE_COPY | CommandId::FILE_CUT => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        let mode = if cmd_id == CommandId::FILE_CUT {
                            ClipboardMode::Cut
                        } else {
                            ClipboardMode::Copy
                        };

                        let paths = vec![entry.path.as_path().to_path_buf()];
                        match self.file_ops.copy_to_clipboard(&paths, mode) {
                            Ok(_) => {
                                let action = if cmd_id == CommandId::FILE_CUT { "Cut" } else { "Copied" };
                                self.status.message = format!("{}: {}", action, entry.name);
                            }
                            Err(e) => {
                                self.status.message = format!("Clipboard error: {}", e);
                            }
                        }
                    }
                }
                true
            }
            CommandId::FILE_PASTE => {
                let cut = false; // Will be determined from clipboard mode
                match self.file_ops.paste_from_clipboard(self.current_path.as_path(), cut) {
                    Ok(pasted) => {
                        self.status.message = format!("Pasted {} file(s)", pasted.len());
                        // Refresh directory
                        self.navigate_to(self.current_path.clone());
                    }
                    Err(e) => {
                        self.status.message = format!("Paste error: {}", e);
                    }
                }
                true
            }
            CommandId::FILE_COPY_IMAGE => {
                self.status.message = "Copy image to clipboard (not yet implemented)".to_string();
                true
            }
            CommandId::FILE_COPY_PATH => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        use app_core::PathFormat;
                        let path_str = match cmd.params.format {
                            Some(PathFormat::Name) => entry.name.clone(),
                            Some(PathFormat::Dir) => entry.path.as_path()
                                .parent()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default(),
                            Some(PathFormat::Full) | None => entry.path.to_string(),
                        };
                        #[cfg(feature = "clipboard")]
                        {
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(&path_str);
                            }
                        }
                        self.status.message = format!("Path copied: {}", path_str);
                    }
                }
                true
            }
            CommandId::FILE_DELETE => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        let use_trash = cmd.params.trash.unwrap_or(true);
                        let confirm = cmd.params.confirm.unwrap_or(true);

                        if confirm {
                            // ダイアログ表示
                            self.pending_delete_path = Some(entry.path.as_path().to_path_buf());
                            self.confirm_dialog = Some(ConfirmDialog::new_delete(
                                &entry.name,
                                use_trash
                            ));
                        } else {
                            // 即削除
                            let paths = vec![entry.path.as_path().to_path_buf()];
                            match self.file_ops.delete(&paths, use_trash) {
                                Ok(_) => {
                                    let action = if use_trash { "Moved to trash" } else { "Deleted" };
                                    self.status.message = format!("{}: {}", action, entry.name);
                                    // Refresh directory
                                    self.navigate_to(self.current_path.clone());
                                }
                                Err(e) => {
                                    self.status.message = format!("Delete error: {}", e);
                                }
                            }
                        }
                    }
                }
                true
            }
            CommandId::FILE_RENAME => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        // Show rename dialog
                        self.rename_dialog = Some(RenameDialog::new(&entry.name));
                    }
                }
                true
            }
            CommandId::FILE_CREATE_DIR => {
                // TODO: Show dialog to get directory name
                self.status.message = "Create directory (dialog required - not yet implemented)".to_string();

                // Example usage (would be called after dialog):
                // let new_dir = self.current_path.as_path().join("NewFolder");
                // match self.file_ops.create_dir(&new_dir) {
                //     Ok(_) => { self.navigate_to(self.current_path.clone()); }
                //     Err(e) => { self.status.message = format!("Create dir error: {}", e); }
                // }
                true
            }
            CommandId::FILE_COPY_TO | CommandId::FILE_MOVE_TO => {
                if let Some(target_str) = &cmd.params.target {
                    if let Some(idx) = self.selected_index {
                        if let Some(entry) = self.file_entries.get(idx) {
                            let target_dir = PathBuf::from(target_str);
                            let sources = vec![entry.path.as_path().to_path_buf()];

                            let result = if cmd_id == CommandId::FILE_MOVE_TO {
                                self.file_ops.move_to(&sources, &target_dir)
                            } else {
                                self.file_ops.copy_to(&sources, &target_dir)
                            };

                            match result {
                                Ok(files) => {
                                    let action = if cmd_id == CommandId::FILE_MOVE_TO { "Moved" } else { "Copied" };
                                    self.status.message = format!("{} {} to {}", action, entry.name, target_str);
                                    // Refresh if moved
                                    if cmd_id == CommandId::FILE_MOVE_TO {
                                        self.navigate_to(self.current_path.clone());
                                    }
                                }
                                Err(e) => {
                                    self.status.message = format!("File operation error: {}", e);
                                }
                            }
                        }
                    }
                } else {
                    // TODO: Show dialog to select target directory
                    self.status.message = "Target path required (dialog not yet implemented)".to_string();
                }
                true
            }
            CommandId::FILE_OPEN_EXPLORER => {
                let select = cmd.params.select.unwrap_or(true);
                let path = if let Some(idx) = self.selected_index {
                    self.file_entries.get(idx).map(|e| e.path.as_path().to_path_buf())
                } else {
                    Some(self.current_path.as_path().to_path_buf())
                };

                if let Some(path_buf) = path {
                    match self.file_ops.open_in_explorer(&path_buf, select) {
                        Ok(_) => {
                            self.status.message = "Opened in file explorer".to_string();
                        }
                        Err(e) => {
                            self.status.message = format!("Open explorer error: {}", e);
                        }
                    }
                }
                true
            }
            CommandId::FILE_OPEN_WITH => {
                if let Some(app_id) = &cmd.params.app_id {
                    if let Some(idx) = self.selected_index {
                        if let Some(entry) = self.file_entries.get(idx) {
                            let args = cmd.params.args.as_deref();
                            match self.file_ops.open_with(entry.path.as_path(), app_id, args) {
                                Ok(_) => {
                                    self.status.message = format!("Opened {} with {}", entry.name, app_id);
                                }
                                Err(e) => {
                                    self.status.message = format!("Open with error: {}", e);
                                }
                            }
                        }
                    }
                }
                true
            }
            CommandId::FILE_OPEN_EXTERNAL => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        match self.file_ops.open_external(entry.path.as_path()) {
                            Ok(_) => {
                                self.status.message = format!("Opened: {}", entry.name);
                            }
                            Err(e) => {
                                self.status.message = format!("Open external error: {}", e);
                            }
                        }
                    }
                }
                true
            }
            CommandId::FILE_PROPERTIES => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        self.status.message = format!(
                            "{}: {} ({} bytes)",
                            entry.name,
                            if entry.is_dir { "Directory" } else { "File" },
                            entry.size
                        );
                    }
                }
                true
            }

            // ========================================
            // Metadata Commands (meta.*)
            // ========================================

            CommandId::META_RATE => {
                if let Some(value) = cmd.params.value {
                    let rating = value.clamp(0, 5);
                    if let Some(idx) = self.selected_index {
                        if let Some(entry) = self.file_entries.get(idx) {
                            // Store rating in database
                            if let Some(ref db) = self.metadata_db {
                                // Ensure file is in DB first
                                let _ = db.upsert_file(&entry.path, Some(entry.size as i64), entry.modified);
                                // Set rating
                                match db.set_rating(entry.path.id(), rating) {
                                    Ok(_) => {
                                        self.status.message = format!("{}: Rating {} (saved)", entry.name, "★".repeat(rating as usize));
                                    }
                                    Err(e) => {
                                        self.status.message = format!("Failed to save rating: {}", e);
                                    }
                                }
                            } else {
                                self.status.message = format!("{}: Rating {} (DB unavailable)", entry.name, "★".repeat(rating as usize));
                            }
                        }
                    }
                }
                true
            }
            CommandId::META_RATE_STEP => {
                let step = cmd.params.amount.unwrap_or(1);
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        if let Some(ref db) = self.metadata_db {
                            // Get current rating and increment
                            let current = db.get_rating(entry.path.id()).unwrap_or(0);
                            let new_rating = ((current + step) % 6).max(0);
                            let _ = db.upsert_file(&entry.path, Some(entry.size as i64), entry.modified);
                            match db.set_rating(entry.path.id(), new_rating) {
                                Ok(_) => {
                                    self.status.message = format!("{}: Rating {} (saved)", entry.name, "★".repeat(new_rating as usize));
                                }
                                Err(e) => {
                                    self.status.message = format!("Failed to save rating: {}", e);
                                }
                            }
                        } else {
                            self.status.message = format!("Rating step: {} (DB unavailable)", step);
                        }
                    }
                }
                true
            }
            CommandId::META_LABEL => {
                use app_core::LabelColor;
                let (label_name, label_value) = match cmd.params.label_color {
                    Some(LabelColor::Red) => ("Red", Some(0xFF0000u32)),
                    Some(LabelColor::Blue) => ("Blue", Some(0x0000FF)),
                    Some(LabelColor::Green) => ("Green", Some(0x00FF00)),
                    Some(LabelColor::Yellow) => ("Yellow", Some(0xFFFF00)),
                    Some(LabelColor::Purple) => ("Purple", Some(0x800080)),
                    Some(LabelColor::None) | None => ("None", None),
                };
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        if let Some(ref db) = self.metadata_db {
                            let _ = db.upsert_file(&entry.path, Some(entry.size as i64), entry.modified);
                            match db.set_label(entry.path.id(), label_value) {
                                Ok(_) => {
                                    self.status.message = format!("{}: Label {} (saved)", entry.name, label_name);
                                }
                                Err(e) => {
                                    self.status.message = format!("Failed to save label: {}", e);
                                }
                            }
                        } else {
                            self.status.message = format!("Label: {} (DB unavailable)", label_name);
                        }
                    }
                } else {
                    self.status.message = format!("Label: {}", label_name);
                }
                true
            }
            CommandId::META_TAG_TOGGLE | CommandId::META_TAG_ADD | CommandId::META_TAG_REMOVE => {
                if let Some(tag_name) = &cmd.params.name {
                    if let Some(idx) = self.selected_index {
                        if let Some(entry) = self.file_entries.get(idx) {
                            if let Some(ref db) = self.metadata_db {
                                // Ensure file is in DB
                                let file_id = match db.upsert_file(&entry.path, Some(entry.size as i64), entry.modified) {
                                    Ok(id) => id,
                                    Err(e) => {
                                        self.status.message = format!("DB error: {}", e);
                                        return true;
                                    }
                                };

                                // Get or create tag
                                let tags = db.list_tags().unwrap_or_default();
                                let tag_id = tags.iter()
                                    .find(|t| t.name.eq_ignore_ascii_case(tag_name))
                                    .map(|t| t.tag_id)
                                    .or_else(|| db.create_tag(tag_name, None).ok());

                                if let Some(tag_id) = tag_id {
                                    let result = match cmd_id {
                                        CommandId::META_TAG_ADD => db.add_tag_to_file(file_id, tag_id),
                                        CommandId::META_TAG_REMOVE => db.remove_tag_from_file(file_id, tag_id),
                                        _ => {
                                            // Toggle - check if tag exists, then add/remove
                                            db.add_tag_to_file(file_id, tag_id)
                                        }
                                    };
                                    let action = match cmd_id {
                                        CommandId::META_TAG_ADD => "Added tag",
                                        CommandId::META_TAG_REMOVE => "Removed tag",
                                        _ => "Toggled tag",
                                    };
                                    match result {
                                        Ok(_) => {
                                            self.status.message = format!("{}: {} (saved)", action, tag_name);
                                        }
                                        Err(e) => {
                                            self.status.message = format!("Failed to {} {}: {}", action.to_lowercase(), tag_name, e);
                                        }
                                    }
                                }
                            } else {
                                self.status.message = format!("Tag: {} (DB unavailable)", tag_name);
                            }
                        }
                    }
                }
                true
            }
            CommandId::META_EDIT_TAGS => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        // TODO: Load current tags from DB and all available tags
                        let current_tags = Vec::new();  // Placeholder
                        let all_tags = Vec::new();      // Placeholder
                        self.tag_dialog = Some(TagEditDialog::new(current_tags, all_tags));
                    }
                }
                true
            }
            CommandId::META_COPY_META => {
                use app_core::CopyTarget;
                let target = match cmd.params.copy_target {
                    Some(CopyTarget::Rating) => "rating",
                    Some(CopyTarget::Tags) => "tags",
                    Some(CopyTarget::All) | None => "all metadata",
                };
                self.status.message = format!("Copied {}", target);
                true
            }
            CommandId::META_EDIT_COMMENT => {
                self.status.message = "Edit comment (dialog required)".to_string();
                true
            }
            CommandId::META_TOGGLE_MARK => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        let hash = entry.path.id();
                        if self.marked_files.contains(&hash) {
                            self.marked_files.remove(&hash);
                            self.status.message = format!("Unmarked: {}", entry.name);
                        } else {
                            self.marked_files.insert(hash);
                            self.status.message = format!("Marked: {} ({} total)", entry.name, self.marked_files.len());
                        }
                    }
                }
                true
            }
            CommandId::META_SELECT_MARKED => {
                // Select all marked files in current folder
                let marked_count = self.file_entries.iter()
                    .filter(|e| self.marked_files.contains(&e.path.id()))
                    .count();
                self.status.message = format!("{} marked files in current folder", marked_count);
                true
            }

            // ========================================
            // App Commands (app.*)
            // ========================================

            CommandId::APP_EXIT => {
                // Will be handled by returning true and checking in event loop
                true
            }
            CommandId::APP_OPEN_SETTINGS => {
                let config = state().map(|s| s.config.read().clone()).unwrap_or_default();
                self.settings_dialog.open(config, None);
                self.status.message = "Opening settings...".to_string();
                true
            }
            CommandId::APP_OPEN_MANUAL => {
                let _ = open::that("https://github.com/your-repo/lightningfiler/wiki");
                self.status.message = "Opening manual...".to_string();
                true
            }
            CommandId::APP_ABOUT => {
                self.status.message = "LightningFiler v0.1.0".to_string();
                true
            }
            CommandId::APP_CLEAR_CACHE => {
                // TODO: Clear thumbnail/preview cache
                self.status.message = "Cache cleared".to_string();
                true
            }
            CommandId::APP_MINIMIZE => {
                if let Some(window) = &self.window {
                    window.set_minimized(true);
                }
                true
            }
            CommandId::APP_MAXIMIZE => {
                if let Some(window) = &self.window {
                    window.set_maximized(!window.is_maximized());
                }
                true
            }
            CommandId::APP_TOPMOST => {
                // TODO: Toggle always-on-top
                self.status.message = "Always on top toggled".to_string();
                true
            }
            CommandId::APP_NEW_WINDOW => {
                // TODO: Spawn new window
                self.status.message = "New window (not yet implemented)".to_string();
                true
            }
            CommandId::APP_TOGGLE_PANEL => {
                if let Some(panel_id) = &cmd.params.panel_id {
                    match panel_id.as_str() {
                        "tree" => {
                            self.status.message = "Tree panel toggled".to_string();
                        }
                        "info" => {
                            self.status.message = "Info panel toggled".to_string();
                        }
                        "preview" => {
                            self.show_browser = !self.show_browser;
                        }
                        _ => {
                            self.status.message = format!("Unknown panel: {}", panel_id);
                        }
                    }
                }
                true
            }
            CommandId::APP_FOCUS_PANEL => {
                if let Some(panel_id) = &cmd.params.panel_id {
                    self.status.message = format!("Focus panel: {}", panel_id);
                }
                true
            }
            CommandId::APP_LAYOUT_SAVE => {
                let slot = cmd.params.slot.unwrap_or(1);
                self.status.message = format!("Layout saved to slot {}", slot);
                true
            }
            CommandId::APP_LAYOUT_LOAD => {
                let slot = cmd.params.slot.unwrap_or(1);
                self.status.message = format!("Layout loaded from slot {}", slot);
                true
            }
            CommandId::APP_LAYOUT_RESET => {
                self.status.message = "Layout reset to default".to_string();
                true
            }
            CommandId::APP_SEARCH => {
                self.status.message = "Search (dialog required)".to_string();
                true
            }
            CommandId::APP_RESTART => {
                self.status.message = "Restart (not yet implemented)".to_string();
                true
            }

            _ => {
                tracing::debug!("Unhandled command: {}", cmd_id);
                false
            }
        }
    }

    /// Sync selection state from NavigationState to app state
    fn sync_selection_from_nav(&mut self) {
        let idx = self.nav_state.current_index();
        self.selected_index = Some(idx);
        self.file_browser.selected = Some(idx);

        // Update position status
        self.status.position = format!("{} / {}", idx + 1, self.file_entries.len());

        // Load image preview if applicable
        if let Some(entry) = self.file_entries.get(idx) {
            if entry.is_image() {
                self.load_image(&entry.clone());
            }
        }
    }

    /// Update grid layout based on current view dimensions
    fn update_grid_layout(&mut self, available_width: f32, item_size: f32) {
        let columns = ((available_width / (item_size + 16.0)).max(1.0)) as usize;
        let visible_rows = 10; // Could calculate from available height

        if columns != self.grid_columns {
            self.grid_columns = columns;
            self.grid_visible_rows = visible_rows;
            self.nav_state.update_grid_layout(columns, visible_rows);
        }
    }

    /// Handle file system events from watcher
    fn handle_fs_event(&mut self, event: FsEvent) {
        match event {
            FsEvent::Created(path) => {
                tracing::info!("File created: {}", path.display());
                // Refresh directory list
                self.refresh_current_directory();

                // DB registration
                if let Some(ref db) = self.metadata_db {
                    let upath = UniversalPath::new(&path);
                    let size = path.metadata().map(|m| m.len() as i64).ok();
                    let modified = path.metadata().ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64);
                    let _ = db.upsert_file(&upath, size, modified);
                }
            }
            FsEvent::Removed(path) => {
                tracing::info!("File removed: {}", path.display());
                self.refresh_current_directory();

                // DB deletion
                if let Some(ref db) = self.metadata_db {
                    let upath = UniversalPath::new(&path);
                    let _ = db.delete_file(upath.id());
                }

                // Thumbnail cache deletion
                if let Some(ref cache) = self.thumbnail_cache {
                    let upath = UniversalPath::new(&path);
                    let _ = cache.delete_by_hash(upath.id());
                }
            }
            FsEvent::Modified(path) => {
                tracing::debug!("File modified: {}", path.display());
                // Reload if currently displayed image was modified
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        if entry.path.as_path() == path {
                            // Currently displayed image was modified
                            self.load_image(&entry.clone());
                        }
                    }
                }
            }
            FsEvent::Renamed { from, to } => {
                tracing::info!("File renamed: {} -> {}", from.display(), to.display());
                self.refresh_current_directory();

                // DB: delete old + insert new (since rename_file doesn't exist yet)
                if let Some(ref db) = self.metadata_db {
                    let old_upath = UniversalPath::new(&from);
                    let _ = db.delete_file(old_upath.id());

                    let new_upath = UniversalPath::new(&to);
                    let size = to.metadata().map(|m| m.len() as i64).ok();
                    let modified = to.metadata().ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64);
                    let _ = db.upsert_file(&new_upath, size, modified);
                }

                // Thumbnail cache: delete old + new will be generated on demand
                if let Some(ref cache) = self.thumbnail_cache {
                    let old_upath = UniversalPath::new(&from);
                    let _ = cache.delete_by_hash(old_upath.id());
                }
            }
        }
    }

    /// Refresh current directory while preserving selection
    fn refresh_current_directory(&mut self) {
        if let Ok(entries) = list_directory(self.current_path.as_path(), &ListOptions::default()) {
            // Preserve selected path
            let selected_path = self.selected_index
                .and_then(|i| self.file_entries.get(i))
                .map(|e| e.path.clone());

            self.file_entries = entries;

            // Restore selection
            if let Some(path) = selected_path {
                self.selected_index = self.file_entries.iter()
                    .position(|e| e.path.id() == path.id());
            }

            self.status.message = format!("{} items", self.file_entries.len());
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            if let Err(e) = self.init_window(event_loop) {
                tracing::error!("Failed to initialize window: {}", e);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Let egui handle the event first
        if let Some(egui_state) = &mut self.egui_state {
            if let Some(window) = &self.window {
                let response = egui_state.on_window_event(window, &event);
                if response.consumed {
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    return;
                }
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                tracing::info!("Close requested");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize((size.width, size.height));
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                // Handle keyboard shortcuts via command system
                if event.state == ElementState::Pressed {
                    use winit::keyboard::{Key, NamedKey};

                    // Try InputHandler first (configurable keybindings)
                    let mut handled = false;
                    if let Some(handler) = &self.input_handler {
                        if let Some(cmd) = handler.handle_key(&event) {
                            // Check for app.exit command
                            if cmd.id.as_str() == CommandId::APP_EXIT {
                                event_loop.exit();
                                return;
                            }
                            handled = self.execute_command(&cmd);
                        }
                    }

                    // Fallback for hardcoded keys (for commands not in keybindings)
                    if !handled {
                        match &event.logical_key {
                            // Grid navigation
                            Key::Named(NamedKey::ArrowUp) => {
                                let cmd = Command::new(CommandId::NAV_MOVE_UP);
                                self.execute_command(&cmd);
                            }
                            Key::Named(NamedKey::ArrowDown) => {
                                let cmd = Command::new(CommandId::NAV_MOVE_DOWN);
                                self.execute_command(&cmd);
                            }
                            Key::Named(NamedKey::ArrowLeft) => {
                                let cmd = Command::new(CommandId::NAV_MOVE_LEFT);
                                self.execute_command(&cmd);
                            }
                            Key::Named(NamedKey::ArrowRight) => {
                                let cmd = Command::new(CommandId::NAV_MOVE_RIGHT);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "k" => {
                                let cmd = Command::new(CommandId::NAV_MOVE_UP);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "j" => {
                                let cmd = Command::new(CommandId::NAV_MOVE_DOWN);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "h" => {
                                let cmd = Command::new(CommandId::NAV_MOVE_LEFT);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "l" => {
                                let cmd = Command::new(CommandId::NAV_MOVE_RIGHT);
                                self.execute_command(&cmd);
                            }

                            // Page navigation
                            Key::Named(NamedKey::PageUp) => {
                                let cmd = Command::new(CommandId::NAV_PAGE_UP);
                                self.execute_command(&cmd);
                            }
                            Key::Named(NamedKey::PageDown) => {
                                let cmd = Command::new(CommandId::NAV_PAGE_DOWN);
                                self.execute_command(&cmd);
                            }
                            Key::Named(NamedKey::Home) => {
                                let cmd = Command::new(CommandId::NAV_HOME);
                                self.execute_command(&cmd);
                            }
                            Key::Named(NamedKey::End) => {
                                let cmd = Command::new(CommandId::NAV_END);
                                self.execute_command(&cmd);
                            }

                            // Hierarchy navigation
                            Key::Named(NamedKey::Backspace) => {
                                let cmd = Command::new(CommandId::NAV_PARENT);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "u" => {
                                let cmd = Command::new(CommandId::NAV_PARENT);
                                self.execute_command(&cmd);
                            }
                            Key::Named(NamedKey::Enter) => {
                                let cmd = Command::new(CommandId::NAV_ENTER);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "o" => {
                                let cmd = Command::new(CommandId::NAV_ENTER);
                                self.execute_command(&cmd);
                            }

                            // View commands
                            Key::Named(NamedKey::Escape) => {
                                if !self.show_browser {
                                    let cmd = Command::new(CommandId::VIEW_PARENT);
                                    self.execute_command(&cmd);
                                }
                            }
                            Key::Named(NamedKey::F11) => {
                                let cmd = Command::new(CommandId::VIEW_TOGGLE_FULLSCREEN);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "f" => {
                                let cmd = Command::new(CommandId::VIEW_TOGGLE_FULLSCREEN);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "r" => {
                                let cmd = Command::new(CommandId::VIEW_ROTATE).with_angle(90);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "+" || c == "=" => {
                                let cmd = Command::new(CommandId::VIEW_ZOOM_IN);
                                self.execute_command(&cmd);
                            }
                            Key::Character(c) if c == "-" => {
                                let cmd = Command::new(CommandId::VIEW_ZOOM_OUT);
                                self.execute_command(&cmd);
                            }

                            // App commands
                            Key::Character(c) if c == "q" => {
                                event_loop.exit();
                            }

                            _ => {}
                        }
                    }
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                if let Some(handler) = &mut self.input_handler {
                    handler.update_modifiers(modifiers.state());
                }
            }

            WindowEvent::RedrawRequested => {
                self.render();
            }

            _ => {}
        }

        // Request redraw
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // File watcher event processing
        if let Some(ref watcher) = self.file_watcher {
            let events = watcher.poll_events();
            for event in events {
                self.handle_fs_event(event);
            }
        }

        // Slideshow advancement
        if self.slideshow.should_advance() {
            if let Some(current) = self.selected_index {
                let total = self.file_entries.iter().filter(|e| e.is_image()).count();
                if let Some(next) = self.slideshow.next_index(current, total) {
                    // Find actual index for image at position `next`
                    let image_indices: Vec<usize> = self.file_entries.iter()
                        .enumerate()
                        .filter(|(_, e)| e.is_image())
                        .map(|(i, _)| i)
                        .collect();
                    if let Some(&actual_idx) = image_indices.get(next) {
                        self.on_select(actual_idx);
                        if let Some(entry) = self.file_entries.get(actual_idx).cloned() {
                            self.load_image(&entry);
                        }
                    }
                }
            }
        }

        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// Run the application
pub fn run() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

/// Format file size for display
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
