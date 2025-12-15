//! Application main loop
//! Integrated with Doc 3 command system

use anyhow::Result;
use app_core::{state, is_supported_image, Command, CommandId, NavigationState};
use app_db::{MetadataDb, ThumbnailCache, DbPool};
use app_fs::{UniversalPath, FileEntry, ListOptions, list_directory, get_parent, is_root, get_next_sibling, get_prev_sibling, count_files};
use app_ui::{
    components::{FileBrowser, ImageViewer, StatusBar, StatusInfo, Toolbar, ToolbarAction, BrowserAction, BrowserViewMode},
    InputHandler, Renderer, Theme,
};
use egui_wgpu::ScreenDescriptor;
use std::sync::Arc;
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
    input_handler: Option<InputHandler>,
    theme: Theme,

    // Navigation state (Doc 3 compliant)
    nav_state: NavigationState,

    // Database
    db_pool: Option<DbPool>,
    metadata_db: Option<MetadataDb>,
    thumbnail_cache: Option<ThumbnailCache>,

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
        let (db_pool, metadata_db, thumbnail_cache) = match app_db::init() {
            Ok((pool, cache)) => {
                let metadata_db = MetadataDb::new(pool.clone());
                tracing::info!("Database initialized successfully");
                (Some(pool), Some(metadata_db), Some(cache))
            }
            Err(e) => {
                tracing::warn!("Failed to initialize database: {}. Running without persistence.", e);
                (None, None, None)
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
            input_handler: None,
            theme: Theme::by_name(&config.general.theme),

            nav_state,

            db_pool,
            metadata_db,
            thumbnail_cache,

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

        self.window = Some(window);
        self.renderer = Some(renderer);
        self.egui_state = Some(egui_state);
        self.egui_renderer = Some(egui_renderer);
        self.input_handler = Some(input_handler);

        Ok(())
    }

    /// Navigate to a directory
    fn navigate_to(&mut self, path: UniversalPath) {
        match list_directory(path.as_path(), &ListOptions::default()) {
            Ok(entries) => {
                self.current_path = path.clone();
                self.file_entries = entries;
                self.selected_index = None;
                self.status.file_name = path.to_string();
                self.status.message = format!("{} items", self.file_entries.len());

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

    /// Navigate up to parent directory
    fn navigate_up(&mut self) {
        if !is_root(self.current_path.as_path()) {
            if let Some(parent) = get_parent(self.current_path.as_path()) {
                self.navigate_to(parent);
            }
        }
    }

    /// Load and display an image
    fn load_image(&mut self, entry: &FileEntry) {
        if !is_supported_image(entry.path.as_path()) {
            return;
        }

        tracing::info!("Loading image: {}", entry.path);

        // Load image data
        match image::open(entry.path.as_path()) {
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
                // Archive - could implement archive viewing later
                self.status.message = "Archive viewing not yet implemented".to_string();
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

        // Track UI actions from egui closure
        let mut clicked_index: Option<usize> = None;
        let mut double_clicked_index: Option<usize> = None;

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
                    // Image viewer mode
                    ui.centered_and_justified(|ui| {
                        ui.label("Image Viewer");
                    });
                }
            });
        });

        // Handle UI actions after egui run
        if let Some(idx) = double_clicked_index {
            self.on_open(idx);
        } else if let Some(idx) = clicked_index {
            self.on_select(idx);
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
                // Split view: browser + preview
                egui::SidePanel::left("browser_panel")
                    .resizable(true)
                    .default_width(350.0)
                    .min_width(200.0)
                    .show_inside(ui, |ui| {
                        // Path bar
                        ui.horizontal(|ui| {
                            if ui.button("⬆").on_hover_text("Up").clicked() {
                                self.navigate_up();
                            }
                            ui.label(self.current_path.to_string());
                        });
                        ui.separator();

                        // File list
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let items: Vec<app_ui::components::file_browser::FileItem> =
                                self.file_entries.iter().map(|e| {
                                    app_ui::components::file_browser::FileItem {
                                        name: e.name.clone(),
                                        path: e.path.display().to_string(),
                                        is_dir: e.is_dir,
                                        size: e.size,
                                        modified: e.modified,
                                        extension: e.extension.clone(),
                                        thumbnail: None,
                                    }
                                }).collect();

                            // Sync selection
                            self.file_browser.selected = self.selected_index;

                            if let Some(action) = self.file_browser.ui(ui, &items) {
                                match action {
                                    BrowserAction::Select(idx) => self.on_select(idx),
                                    BrowserAction::Open(idx) => self.on_open(idx),
                                    BrowserAction::ContextMenu(_) => {}
                                }
                            }
                        });
                    });

                // Preview area
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.image_viewer.ui(ui);
                });
            } else {
                // Full viewer mode
                self.image_viewer.ui(ui);
            }
        });
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
                    self.image_viewer.rotate_right();
                } else {
                    self.image_viewer.rotate_left();
                }
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
                        // Horizontal flip - would need to be added to ImageViewer
                        self.status.message = "Horizontal flip".to_string();
                    }
                    Some(FlipAxis::Vertical) | None => {
                        // Vertical flip
                        self.status.message = "Vertical flip".to_string();
                    }
                }
                true
            }
            CommandId::VIEW_SPREAD_MODE => {
                use app_core::SpreadMode;
                let msg = match cmd.params.spread {
                    Some(SpreadMode::Single) => "Single page mode",
                    Some(SpreadMode::Spread) => "Spread (2-page) mode",
                    Some(SpreadMode::Auto) | None => "Auto spread mode",
                };
                self.status.message = msg.to_string();
                // TODO: Implement actual spread mode in viewer
                true
            }
            CommandId::VIEW_SET_BACKGROUND => {
                use app_core::BackgroundColor;
                let msg = match cmd.params.color {
                    Some(BackgroundColor::Black) => "Background: Black",
                    Some(BackgroundColor::Gray) => "Background: Gray",
                    Some(BackgroundColor::White) => "Background: White",
                    Some(BackgroundColor::Check) => "Background: Checkered",
                    Some(BackgroundColor::Transparent) | None => "Background: Transparent",
                };
                self.status.message = msg.to_string();
                // TODO: Implement actual background color change
                true
            }
            CommandId::VIEW_SMART_SCROLL_DOWN => {
                // Smart scroll: scroll down, if at bottom edge go to next image
                let overlap = cmd.params.overlap.unwrap_or(50);
                // For now, just go to next image if not scrollable
                if self.image_viewer.zoom <= 1.0 {
                    self.next_image();
                } else {
                    // Would scroll, but for simplicity just notify
                    self.status.message = format!("Smart scroll down (overlap: {}px)", overlap);
                }
                true
            }
            CommandId::VIEW_SMART_SCROLL_UP => {
                let overlap = cmd.params.overlap.unwrap_or(50);
                if self.image_viewer.zoom <= 1.0 {
                    self.prev_image();
                } else {
                    self.status.message = format!("Smart scroll up (overlap: {}px)", overlap);
                }
                true
            }
            CommandId::VIEW_SLIDESHOW => {
                use app_core::SlideshowAction;
                let action_str = match cmd.params.action {
                    Some(SlideshowAction::Start) => "Slideshow started",
                    Some(SlideshowAction::Stop) => "Slideshow stopped",
                    Some(SlideshowAction::Toggle) | None => "Slideshow toggled",
                };
                self.status.message = action_str.to_string();
                // TODO: Implement actual slideshow functionality
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
                self.status.message = "Split view mode (not yet implemented)".to_string();
                true
            }
            CommandId::VIEW_SYNC_SCROLL => {
                self.status.message = "Sync scroll (not yet implemented)".to_string();
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
                    self.status.message = format!("Slideshow interval: {}ms", amount);
                }
                true
            }

            // ========================================
            // File Commands (file.*)
            // ========================================

            CommandId::FILE_COPY | CommandId::FILE_CUT => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        let path = entry.path.to_string();
                        // Use arboard for clipboard
                        #[cfg(feature = "clipboard")]
                        {
                            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                let _ = clipboard.set_text(&path);
                            }
                        }
                        let action = if cmd_id == CommandId::FILE_CUT { "Cut" } else { "Copied" };
                        self.status.message = format!("{}: {}", action, entry.name);
                    }
                }
                true
            }
            CommandId::FILE_PASTE => {
                self.status.message = "Paste (not yet implemented)".to_string();
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
                            self.status.message = format!("Delete {} (confirm required)", entry.name);
                        } else {
                            if use_trash {
                                #[cfg(feature = "trash")]
                                {
                                    match trash::delete(&entry.path.as_path()) {
                                        Ok(_) => {
                                            self.status.message = format!("Moved to trash: {}", entry.name);
                                            self.navigate_to(self.current_path.clone());
                                        }
                                        Err(e) => {
                                            self.status.message = format!("Error: {}", e);
                                        }
                                    }
                                }
                                #[cfg(not(feature = "trash"))]
                                {
                                    self.status.message = "Trash feature not enabled".to_string();
                                }
                            } else {
                                match std::fs::remove_file(&entry.path.as_path()) {
                                    Ok(_) => {
                                        self.status.message = format!("Deleted: {}", entry.name);
                                        self.navigate_to(self.current_path.clone());
                                    }
                                    Err(e) => {
                                        self.status.message = format!("Error: {}", e);
                                    }
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
                        self.status.message = format!("Rename: {} (dialog required)", entry.name);
                    }
                }
                true
            }
            CommandId::FILE_CREATE_DIR => {
                self.status.message = "Create directory (dialog required)".to_string();
                true
            }
            CommandId::FILE_COPY_TO | CommandId::FILE_MOVE_TO => {
                if let Some(target) = &cmd.params.target {
                    if let Some(idx) = self.selected_index {
                        if let Some(entry) = self.file_entries.get(idx) {
                            let action = if cmd_id == CommandId::FILE_MOVE_TO { "Move" } else { "Copy" };
                            self.status.message = format!("{} {} to {}", action, entry.name, target);
                        }
                    }
                } else {
                    self.status.message = "Target path required".to_string();
                }
                true
            }
            CommandId::FILE_OPEN_EXPLORER => {
                let path = if let Some(idx) = self.selected_index {
                    self.file_entries.get(idx).map(|e| e.path.to_string())
                } else {
                    Some(self.current_path.to_string())
                };
                if let Some(path) = path {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("explorer")
                            .arg("/select,")
                            .arg(&path)
                            .spawn();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open")
                            .arg("-R")
                            .arg(&path)
                            .spawn();
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(std::path::Path::new(&path).parent().unwrap_or(std::path::Path::new(&path)))
                            .spawn();
                    }
                    self.status.message = "Opened in file explorer".to_string();
                }
                true
            }
            CommandId::FILE_OPEN_WITH => {
                if let Some(app_id) = &cmd.params.app_id {
                    if let Some(idx) = self.selected_index {
                        if let Some(entry) = self.file_entries.get(idx) {
                            self.status.message = format!("Open {} with {}", entry.name, app_id);
                        }
                    }
                }
                true
            }
            CommandId::FILE_OPEN_EXTERNAL => {
                if let Some(idx) = self.selected_index {
                    if let Some(entry) = self.file_entries.get(idx) {
                        let _ = open::that(entry.path.as_path());
                        self.status.message = format!("Opened: {}", entry.name);
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
                self.status.message = "Edit tags (dialog required)".to_string();
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
                        // TODO: Toggle mark state in app state
                        self.status.message = format!("Toggled mark: {}", entry.name);
                    }
                }
                true
            }
            CommandId::META_SELECT_MARKED => {
                // TODO: Select all marked files
                self.status.message = "Select marked files".to_string();
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
                self.status.message = "Settings (dialog required)".to_string();
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
