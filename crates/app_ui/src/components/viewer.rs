//! Image viewer component
//! Based on Doc 4: UI/Rendering Specification

use egui::{Ui, Vec2, Rect, Pos2, TextureId, Color32, Stroke, FontId, Align2};
use std::time::Instant;

/// Viewer action returned to parent
#[derive(Debug, Clone)]
pub enum ViewerAction {
    None,
    NextImage,
    PrevImage,
    FirstImage,
    LastImage,
    ToggleFullscreen,
    ToggleSlideshow,
    OpenSettings,
    Close,
    SeekTo(f32),  // 0.0-1.0 position
}

/// Image viewer component with Doc 4 overlay UI
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

    /// Horizontal flip
    pub flip_h: bool,

    /// Vertical flip
    pub flip_v: bool,

    /// Fit mode
    pub fit_mode: FitMode,

    /// Is dragging (panning)
    drag_start: Option<Pos2>,
    pan_start: Vec2,

    // Doc 4: Overlay UI state
    /// Show overlay (auto-hide after mouse idle)
    overlay_visible: bool,
    /// Last mouse movement time
    last_mouse_move: Instant,
    /// Overlay fade duration (ms)
    overlay_fade_ms: u64,

    // Navigation info for overlay
    /// Current file name
    pub file_name: String,
    /// Image resolution text
    pub resolution_text: String,
    /// Current position in folder (1-based)
    pub current_index: usize,
    /// Total files in folder
    pub total_files: usize,
    /// Slideshow running
    pub slideshow_active: bool,

    // Seek bar state
    seek_dragging: bool,
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
            flip_h: false,
            flip_v: false,
            fit_mode: FitMode::FitToWindow,
            drag_start: None,
            pan_start: Vec2::ZERO,
            // Overlay
            overlay_visible: true,
            last_mouse_move: Instant::now(),
            overlay_fade_ms: 3000,
            // Navigation info
            file_name: String::new(),
            resolution_text: String::new(),
            current_index: 0,
            total_files: 0,
            slideshow_active: false,
            seek_dragging: false,
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

    /// Render the viewer with Doc 4 overlay UI
    pub fn ui(&mut self, ui: &mut Ui) -> ViewerAction {
        let available = ui.available_rect_before_wrap();
        let mut action = ViewerAction::None;

        // Overlay dimensions
        let bar_height = 40.0;
        let seek_height = 24.0;

        // Check mouse movement for overlay visibility
        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        if pointer_pos.is_some() {
            let delta = ui.input(|i| i.pointer.delta());
            if delta.length() > 1.0 {
                self.last_mouse_move = Instant::now();
                self.overlay_visible = true;
            }
        }

        // Auto-hide overlay after idle time
        if self.last_mouse_move.elapsed().as_millis() > self.overlay_fade_ms as u128 {
            if !self.seek_dragging {
                self.overlay_visible = false;
            }
        }

        // Draw background
        ui.painter().rect_filled(
            available,
            0.0,
            Color32::from_rgb(32, 32, 32),
        );

        // Draw image if available
        if let Some(texture) = self.texture {
            let display_size = self.calculate_display_size(available.size());
            let image_rect = self.calculate_image_rect(available, display_size);

            // Calculate UV with flip support
            let uv = self.calculate_uv();
            ui.painter().image(texture, image_rect, uv, Color32::WHITE);
        } else {
            // No image placeholder
            ui.painter().text(
                available.center(),
                Align2::CENTER_CENTER,
                "No image",
                FontId::proportional(24.0),
                Color32::GRAY,
            );
        }

        // Always draw seek bar at bottom (even when overlay is hidden)
        if self.texture.is_some() {
            if let Some(seek_action) = self.draw_seek_bar(ui, available) {
                action = seek_action;
            }
        }

        // Draw overlay UI (Doc 4 spec) - top bar only when visible
        if self.overlay_visible && self.texture.is_some() {
            if let Some(overlay_action) = self.draw_top_bar(ui, available) {
                action = overlay_action;
            }
        }

        // Handle input for main image area (excluding overlay areas)
        // Only if no overlay/seek action was taken
        if matches!(action, ViewerAction::None) {
            // Always exclude seek bar, exclude top bar only when visible
            let image_area = if self.texture.is_some() {
                let top_offset = if self.overlay_visible { bar_height } else { 0.0 };
                Rect::from_min_max(
                    Pos2::new(available.min.x, available.min.y + top_offset),
                    Pos2::new(available.max.x, available.max.y - seek_height),
                )
            } else {
                available
            };

            if let Some(input_action) = self.handle_input(ui, image_area) {
                action = input_action;
            }
        }

        action
    }

    /// Calculate UV rect with flip support
    fn calculate_uv(&self) -> Rect {
        let (u_min, u_max) = if self.flip_h { (1.0, 0.0) } else { (0.0, 1.0) };
        let (v_min, v_max) = if self.flip_v { (1.0, 0.0) } else { (0.0, 1.0) };
        Rect::from_min_max(Pos2::new(u_min, v_min), Pos2::new(u_max, v_max))
    }

    /// Draw top control bar (Doc 4: 1.3 A)
    fn draw_top_bar(&mut self, ui: &mut Ui, rect: Rect) -> Option<ViewerAction> {
        let mut action: Option<ViewerAction> = None;
        let bar_height = 40.0;
        let bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 180);

        let top_bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), bar_height));
        ui.painter().rect_filled(top_bar, 0.0, bg_color);

        // Left: File info
        let info_text = format!("{}", self.file_name);
        ui.painter().text(
            Pos2::new(top_bar.min.x + 12.0, top_bar.center().y),
            Align2::LEFT_CENTER,
            &info_text,
            FontId::proportional(14.0),
            Color32::WHITE,
        );

        // Resolution (right of filename)
        if !self.resolution_text.is_empty() {
            ui.painter().text(
                Pos2::new(top_bar.min.x + 12.0 + info_text.len() as f32 * 8.0 + 20.0, top_bar.center().y),
                Align2::LEFT_CENTER,
                &self.resolution_text,
                FontId::proportional(12.0),
                Color32::GRAY,
            );
        }

        // Center: Navigation
        let nav_center_x = top_bar.center().x;
        let nav_y = top_bar.center().y;
        let nav_spacing = 30.0;

        // Navigation buttons: << < N/M > >> ▶
        let nav_buttons = [
            ("⏮", -2.5 * nav_spacing, ViewerAction::FirstImage),
            ("◀", -1.5 * nav_spacing, ViewerAction::PrevImage),
            ("▶", 1.5 * nav_spacing, ViewerAction::NextImage),
            ("⏭", 2.5 * nav_spacing, ViewerAction::LastImage),
        ];

        for (label, offset, btn_action) in nav_buttons {
            let btn_pos = Pos2::new(nav_center_x + offset, nav_y);
            let btn_rect = Rect::from_center_size(btn_pos, Vec2::splat(24.0));
            let response = ui.allocate_rect(btn_rect, egui::Sense::click());

            let color = if response.hovered() { Color32::WHITE } else { Color32::LIGHT_GRAY };
            ui.painter().text(btn_pos, Align2::CENTER_CENTER, label, FontId::proportional(16.0), color);

            if response.clicked() {
                action = Some(btn_action);
            }
        }

        // Position text: "N / M"
        let pos_text = format!("{} / {}", self.current_index, self.total_files);
        ui.painter().text(
            Pos2::new(nav_center_x, nav_y),
            Align2::CENTER_CENTER,
            &pos_text,
            FontId::proportional(14.0),
            Color32::WHITE,
        );

        // Slideshow button
        let slideshow_pos = Pos2::new(nav_center_x + 4.0 * nav_spacing, nav_y);
        let slideshow_rect = Rect::from_center_size(slideshow_pos, Vec2::splat(24.0));
        let slideshow_response = ui.allocate_rect(slideshow_rect, egui::Sense::click());
        let ss_label = if self.slideshow_active { "⏸" } else { "▶️" };
        let ss_color = if slideshow_response.hovered() { Color32::WHITE } else { Color32::LIGHT_GRAY };
        ui.painter().text(slideshow_pos, Align2::CENTER_CENTER, ss_label, FontId::proportional(16.0), ss_color);
        if slideshow_response.clicked() {
            action = Some(ViewerAction::ToggleSlideshow);
        }

        // Right: Settings, Fullscreen, Close
        let right_x = top_bar.max.x - 12.0;
        let right_buttons = [
            ("✕", 0.0, ViewerAction::Close),
            ("⛶", -30.0, ViewerAction::ToggleFullscreen),
            ("⚙", -60.0, ViewerAction::OpenSettings),
        ];

        for (label, offset, btn_action) in right_buttons {
            let btn_pos = Pos2::new(right_x + offset, nav_y);
            let btn_rect = Rect::from_center_size(btn_pos, Vec2::splat(24.0));
            let response = ui.allocate_rect(btn_rect, egui::Sense::click());

            let color = if response.hovered() { Color32::WHITE } else { Color32::LIGHT_GRAY };
            ui.painter().text(btn_pos, Align2::CENTER_CENTER, label, FontId::proportional(16.0), color);

            if response.clicked() {
                action = Some(btn_action);
            }
        }

        action
    }

    /// Draw bottom seek bar (Doc 4: 1.3 B) - Always visible
    fn draw_seek_bar(&mut self, ui: &mut Ui, rect: Rect) -> Option<ViewerAction> {
        let mut action: Option<ViewerAction> = None;
        let seek_height = 24.0;
        let bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 180);

        let seek_bar = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.max.y - seek_height),
            Vec2::new(rect.width(), seek_height),
        );
        ui.painter().rect_filled(seek_bar, 0.0, bg_color);

        // Seek track
        let track_margin = 20.0;
        let track_rect = Rect::from_min_max(
            Pos2::new(seek_bar.min.x + track_margin, seek_bar.center().y - 3.0),
            Pos2::new(seek_bar.max.x - track_margin, seek_bar.center().y + 3.0),
        );
        ui.painter().rect_filled(track_rect, 2.0, Color32::DARK_GRAY);

        // Seek position indicator
        if self.total_files > 0 {
            let progress = self.current_index as f32 / self.total_files as f32;
            let indicator_x = track_rect.min.x + track_rect.width() * progress;
            let indicator_rect = Rect::from_center_size(
                Pos2::new(indicator_x, seek_bar.center().y),
                Vec2::new(8.0, 16.0),
            );
            ui.painter().rect_filled(indicator_rect, 2.0, Color32::WHITE);

            // Filled portion
            let filled_rect = Rect::from_min_max(
                track_rect.min,
                Pos2::new(indicator_x, track_rect.max.y),
            );
            ui.painter().rect_filled(filled_rect, 2.0, Color32::from_rgb(100, 150, 255));

            // Seek interaction - allocate clickable area
            let seek_response = ui.allocate_rect(track_rect.expand(8.0), egui::Sense::click_and_drag());
            if seek_response.drag_started() {
                self.seek_dragging = true;
            }
            if seek_response.drag_stopped() {
                self.seek_dragging = false;
            }
            if seek_response.clicked() || seek_response.dragged() {
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let relative_x = (pos.x - track_rect.min.x) / track_rect.width();
                    let seek_pos = relative_x.clamp(0.0, 1.0);
                    action = Some(ViewerAction::SeekTo(seek_pos));
                }
            }
        }

        action
    }

    fn handle_input(&mut self, ui: &mut Ui, rect: Rect) -> Option<ViewerAction> {
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

        // Double-click to close viewer (return to browser)
        if response.double_clicked() {
            return Some(ViewerAction::Close);
        }

        None
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

    /// Check if image fits within the given available size
    pub fn image_fits_in(&self, available: Vec2) -> bool {
        if self.image_size == Vec2::ZERO {
            return true;
        }
        let display_size = self.calculate_display_size(available);
        display_size.x <= available.x && display_size.y <= available.y
    }

    /// Smart scroll down/forward
    /// Returns true if should go to next image, false if scrolled within image
    pub fn smart_scroll_down(&mut self, available: Vec2, overlap: f32) -> bool {
        if self.image_size == Vec2::ZERO {
            return true; // No image, go to next
        }

        let display_size = self.calculate_display_size(available);

        // If image fits, go to next image
        if display_size.y <= available.y {
            return true;
        }

        // Calculate max pan (how far can we scroll down)
        let max_pan_y = (display_size.y - available.y) / 2.0;

        // Check if we're at the bottom edge
        if self.pan.y <= -max_pan_y + 1.0 {
            // At bottom edge, go to next image
            self.pan.y = max_pan_y; // Reset to top for next image
            return true;
        }

        // Scroll down (pan negative Y)
        let scroll_amount = available.y - overlap;
        self.pan.y = (self.pan.y - scroll_amount).max(-max_pan_y);
        false
    }

    /// Smart scroll up/backward
    /// Returns true if should go to prev image, false if scrolled within image
    pub fn smart_scroll_up(&mut self, available: Vec2, overlap: f32) -> bool {
        if self.image_size == Vec2::ZERO {
            return true; // No image, go to prev
        }

        let display_size = self.calculate_display_size(available);

        // If image fits, go to prev image
        if display_size.y <= available.y {
            return true;
        }

        // Calculate max pan
        let max_pan_y = (display_size.y - available.y) / 2.0;

        // Check if we're at the top edge
        if self.pan.y >= max_pan_y - 1.0 {
            // At top edge, go to prev image
            self.pan.y = -max_pan_y; // Reset to bottom for prev image
            return true;
        }

        // Scroll up (pan positive Y)
        let scroll_amount = available.y - overlap;
        self.pan.y = (self.pan.y + scroll_amount).min(max_pan_y);
        false
    }

    /// Get estimated available size for smart scroll calculations
    /// This returns a reasonable default; actual size comes from UI rendering
    pub fn get_estimated_available(&self) -> Vec2 {
        // Use 1080p as default estimate; actual calculation happens in UI
        Vec2::new(1920.0, 1040.0)
    }
}

impl Default for ImageViewer {
    fn default() -> Self {
        Self::new()
    }
}
