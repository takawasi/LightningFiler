//! Image viewer component

use egui::{Ui, Vec2, Rect, Pos2, TextureId};

/// Image viewer component
pub struct ImageViewer {
    /// Current texture
    pub texture: Option<TextureId>,

    /// Image dimensions
    pub image_size: Vec2,

    /// Current zoom level
    pub zoom: f32,

    /// Pan offset
    pub pan: Vec2,

    /// Rotation (degrees, 0/90/180/270)
    pub rotation: i32,

    /// Fit mode
    pub fit_mode: FitMode,

    /// Is dragging (panning)
    drag_start: Option<Pos2>,
    pan_start: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitMode {
    FitToWindow,
    FitWidth,
    FitHeight,
    OriginalSize,
}

impl ImageViewer {
    pub fn new() -> Self {
        Self {
            texture: None,
            image_size: Vec2::ZERO,
            zoom: 1.0,
            pan: Vec2::ZERO,
            rotation: 0,
            fit_mode: FitMode::FitToWindow,
            drag_start: None,
            pan_start: Vec2::ZERO,
        }
    }

    /// Set the image to display
    pub fn set_image(&mut self, texture: TextureId, width: u32, height: u32) {
        self.texture = Some(texture);
        self.image_size = Vec2::new(width as f32, height as f32);
        self.reset_view();
    }

    /// Clear the current image
    pub fn clear(&mut self) {
        self.texture = None;
        self.image_size = Vec2::ZERO;
    }

    /// Reset view to default
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.rotation = 0;
    }

    /// Render the viewer
    pub fn ui(&mut self, ui: &mut Ui) {
        let available = ui.available_rect_before_wrap();

        // Handle input
        self.handle_input(ui, available);

        // Draw background
        ui.painter().rect_filled(
            available,
            0.0,
            egui::Color32::from_rgb(32, 32, 32),
        );

        // Draw image if available
        if let Some(texture) = self.texture {
            let display_size = self.calculate_display_size(available.size());
            let image_rect = self.calculate_image_rect(available, display_size);

            // Draw image
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            ui.painter().image(texture, image_rect, uv, egui::Color32::WHITE);
        } else {
            // No image placeholder
            ui.painter().text(
                available.center(),
                egui::Align2::CENTER_CENTER,
                "No image",
                egui::FontId::proportional(24.0),
                egui::Color32::GRAY,
            );
        }
    }

    fn handle_input(&mut self, ui: &mut Ui, rect: Rect) {
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

        // Zoom with scroll
        if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let zoom_factor = if scroll > 0.0 { 1.1 } else { 0.9 };
                self.zoom = (self.zoom * zoom_factor).clamp(0.1, 10.0);
            }
        }

        // Pan with drag
        if response.drag_started() {
            self.drag_start = ui.input(|i| i.pointer.hover_pos());
            self.pan_start = self.pan;
        }

        if response.dragged() {
            if let (Some(start), Some(current)) = (self.drag_start, ui.input(|i| i.pointer.hover_pos())) {
                let delta = current - start;
                self.pan = self.pan_start + Vec2::new(delta.x, delta.y);
            }
        }

        if response.drag_stopped() {
            self.drag_start = None;
        }

        // Double-click to reset
        if response.double_clicked() {
            self.reset_view();
        }
    }

    fn calculate_display_size(&self, available: Vec2) -> Vec2 {
        if self.image_size == Vec2::ZERO {
            return Vec2::ZERO;
        }

        // Apply rotation to size
        let image_size = if self.rotation == 90 || self.rotation == 270 {
            Vec2::new(self.image_size.y, self.image_size.x)
        } else {
            self.image_size
        };

        // Calculate base size based on fit mode
        let base_size = match self.fit_mode {
            FitMode::FitToWindow => {
                let scale_x = available.x / image_size.x;
                let scale_y = available.y / image_size.y;
                let scale = scale_x.min(scale_y).min(1.0); // Don't upscale
                image_size * scale
            }
            FitMode::FitWidth => {
                let scale = available.x / image_size.x;
                image_size * scale
            }
            FitMode::FitHeight => {
                let scale = available.y / image_size.y;
                image_size * scale
            }
            FitMode::OriginalSize => image_size,
        };

        // Apply zoom
        base_size * self.zoom
    }

    fn calculate_image_rect(&self, available: Rect, display_size: Vec2) -> Rect {
        let center = available.center() + self.pan;
        Rect::from_center_size(center, display_size)
    }

    /// Zoom in
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * 1.2).min(10.0);
    }

    /// Zoom out
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / 1.2).max(0.1);
    }

    /// Set zoom level
    pub fn set_zoom(&mut self, level: f32) {
        self.zoom = level.clamp(0.1, 10.0);
    }

    /// Rotate left
    pub fn rotate_left(&mut self) {
        self.rotation = (self.rotation + 270) % 360;
    }

    /// Rotate right
    pub fn rotate_right(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
    }
}

impl Default for ImageViewer {
    fn default() -> Self {
        Self::new()
    }
}
