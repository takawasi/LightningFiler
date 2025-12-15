//! Application main loop

use anyhow::Result;
use app_core::{state, AppConfig, is_supported_image};
use app_fs::{UniversalPath, FileEntry, ListOptions, list_directory, list_drives, get_parent, is_root};
use app_ui::{
    components::{FileBrowser, ImageViewer, StatusBar, StatusInfo, Toolbar, ToolbarAction, BrowserAction},
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

    // State
    show_browser: bool,
    status: StatusInfo,
    current_path: UniversalPath,
    file_entries: Vec<FileEntry>,
    selected_index: Option<usize>,
    current_texture: Option<egui::TextureHandle>,
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

                            if ui.selectable_label(is_selected, label).clicked() {
                                // Selection handled via events
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
                self.file_browser.view_mode = app_ui::components::file_browser::BrowserViewMode::Grid;
            }
            ToolbarAction::ListView => {
                self.file_browser.view_mode = app_ui::components::file_browser::BrowserViewMode::List;
            }
            ToolbarAction::Settings => {
                // TODO: Open settings
            }
            ToolbarAction::Fullscreen => {
                self.show_browser = !self.show_browser;
            }
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
                // Handle keyboard shortcuts
                if event.state == ElementState::Pressed {
                    use winit::keyboard::{Key, NamedKey};

                    match &event.logical_key {
                        Key::Named(NamedKey::ArrowRight) => {
                            self.next_image();
                        }
                        Key::Character(c) if c == "l" => {
                            self.next_image();
                        }
                        Key::Named(NamedKey::ArrowLeft) => {
                            self.prev_image();
                        }
                        Key::Character(c) if c == "h" => {
                            self.prev_image();
                        }
                        Key::Named(NamedKey::Backspace) => {
                            self.navigate_up();
                        }
                        Key::Character(c) if c == "u" => {
                            self.navigate_up();
                        }
                        Key::Named(NamedKey::Enter) => {
                            if let Some(idx) = self.selected_index {
                                self.on_open(idx);
                            }
                        }
                        Key::Named(NamedKey::Escape) => {
                            if !self.show_browser {
                                self.show_browser = true;
                            }
                        }
                        Key::Named(NamedKey::F11) => {
                            self.show_browser = !self.show_browser;
                        }
                        Key::Character(c) if c == "f" => {
                            self.show_browser = !self.show_browser;
                        }
                        Key::Character(c) if c == "q" => {
                            event_loop.exit();
                        }
                        _ => {}
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
