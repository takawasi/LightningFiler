//! Application main loop

use anyhow::Result;
use app_core::{state, AppConfig};
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
}

impl App {
    fn new() -> Self {
        let config = state().map(|s| s.config.read().clone()).unwrap_or_default();

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
            status: StatusInfo::default(),
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

    fn render(&mut self) {
        let Some(window) = &self.window else { return };
        let Some(renderer) = &self.renderer else { return };
        let Some(egui_state) = &mut self.egui_state else { return };
        let Some(egui_renderer) = &mut self.egui_renderer else { return };

        // Get surface texture
        let output = match renderer.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                if let Some(r) = &mut self.renderer {
                    r.handle_device_lost();
                }
                return;
            }
            Err(e) => {
                tracing::error!("Surface error: {:?}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Run egui
        let raw_input = egui_state.take_egui_input(window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            self.ui(ctx);
        });

        egui_state.handle_platform_output(window, full_output.platform_output);

        let clipped_primitives = self.egui_ctx.tessellate(
            full_output.shapes,
            full_output.pixels_per_point,
        );

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

            egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
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
                    .default_width(300.0)
                    .show_inside(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            // Placeholder items for demo
                            let items = vec![
                                app_ui::components::file_browser::FileItem {
                                    name: "Documents".to_string(),
                                    path: "C:\\Documents".to_string(),
                                    is_dir: true,
                                    size: 0,
                                    modified: None,
                                    extension: String::new(),
                                    thumbnail: None,
                                },
                                app_ui::components::file_browser::FileItem {
                                    name: "image1.jpg".to_string(),
                                    path: "C:\\image1.jpg".to_string(),
                                    is_dir: false,
                                    size: 1024000,
                                    modified: Some(1702800000),
                                    extension: "jpg".to_string(),
                                    thumbnail: None,
                                },
                            ];

                            if let Some(action) = self.file_browser.ui(ui, &items) {
                                self.handle_browser_action(action);
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
            ToolbarAction::Previous => {
                if let Some(state) = state() {
                    state.navigation.write().prev();
                }
            }
            ToolbarAction::Next => {
                if let Some(state) = state() {
                    state.navigation.write().next();
                }
            }
            ToolbarAction::UpFolder => {
                // Navigate up
            }
            ToolbarAction::Home => {
                // Go home
            }
            ToolbarAction::ZoomIn => {
                self.image_viewer.zoom_in();
            }
            ToolbarAction::ZoomOut => {
                self.image_viewer.zoom_out();
            }
            ToolbarAction::OriginalSize => {
                self.image_viewer.set_zoom(1.0);
            }
            ToolbarAction::FitToWindow => {
                self.image_viewer.fit_mode = app_ui::components::viewer::FitMode::FitToWindow;
                self.image_viewer.reset_view();
            }
            ToolbarAction::RotateLeft => {
                self.image_viewer.rotate_left();
            }
            ToolbarAction::RotateRight => {
                self.image_viewer.rotate_right();
            }
            ToolbarAction::GridView => {
                self.file_browser.view_mode = app_ui::components::file_browser::BrowserViewMode::Grid;
            }
            ToolbarAction::ListView => {
                self.file_browser.view_mode = app_ui::components::file_browser::BrowserViewMode::List;
            }
            ToolbarAction::Settings => {
                // Open settings
            }
            ToolbarAction::Fullscreen => {
                self.show_browser = !self.show_browser;
            }
        }
    }

    fn handle_browser_action(&mut self, action: BrowserAction) {
        match action {
            BrowserAction::Select(idx) => {
                tracing::debug!("Selected item: {}", idx);
            }
            BrowserAction::Open(idx) => {
                tracing::debug!("Opened item: {}", idx);
            }
            BrowserAction::ContextMenu(idx) => {
                tracing::debug!("Context menu for item: {}", idx);
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
                if let Some(handler) = &self.input_handler {
                    if let Some(cmd) = handler.handle_key(&event) {
                        tracing::debug!("Command: {:?}", cmd);
                        // Dispatch command
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
