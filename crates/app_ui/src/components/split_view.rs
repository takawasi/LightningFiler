//! Split view for comparing two images side by side

use egui::{Rect, Pos2, Vec2};
use std::path::PathBuf;

/// Split direction
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SplitDirection {
    Horizontal,  // Top/Bottom split
    #[default]
    Vertical,    // Left/Right split
}

/// A single pane in the split view
#[derive(Clone, Default)]
pub struct SplitPane {
    pub path: Option<PathBuf>,
    pub texture_id: Option<egui::TextureId>,
    pub image_size: Option<(u32, u32)>,
    pub zoom: f32,
    pub pan: Vec2,
    pub locked: bool,
}

impl SplitPane {
    pub fn new() -> Self {
        Self {
            path: None,
            texture_id: None,
            image_size: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            locked: true,
        }
    }

    pub fn clear(&mut self) {
        self.path = None;
        self.texture_id = None;
        self.image_size = None;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
    }
}

/// Split view component for comparing images
pub struct SplitView {
    pub enabled: bool,
    pub direction: SplitDirection,
    pub ratio: f32,  // 0.0-1.0, ratio for first pane
    pub panes: [SplitPane; 2],
    pub active_pane: usize,
    pub sync_zoom: bool,
    pub sync_pan: bool,
}

impl Default for SplitView {
    fn default() -> Self {
        Self::new()
    }
}

impl SplitView {
    pub fn new() -> Self {
        Self {
            enabled: false,
            direction: SplitDirection::Vertical,
            ratio: 0.5,
            panes: [SplitPane::new(), SplitPane::new()],
            active_pane: 0,
            sync_zoom: true,
            sync_pan: true,
        }
    }

    /// Toggle split view on/off
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Set split direction
    pub fn set_direction(&mut self, dir: SplitDirection) {
        self.direction = dir;
    }

    /// Toggle split direction
    pub fn toggle_direction(&mut self) {
        self.direction = match self.direction {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        };
    }

    /// Swap panes
    pub fn swap_panes(&mut self) {
        self.panes.swap(0, 1);
    }

    /// Set active pane
    pub fn set_active(&mut self, idx: usize) {
        self.active_pane = idx.min(1);
    }

    /// Get active pane
    pub fn active_pane_mut(&mut self) -> &mut SplitPane {
        &mut self.panes[self.active_pane]
    }

    /// Get inactive pane
    pub fn inactive_pane_mut(&mut self) -> &mut SplitPane {
        &mut self.panes[1 - self.active_pane]
    }

    /// Calculate rectangles for both panes
    pub fn calculate_rects(&self, viewport: Rect) -> [Rect; 2] {
        let (w, h) = (viewport.width(), viewport.height());
        let splitter_width = 4.0;

        match self.direction {
            SplitDirection::Vertical => {
                let split_x = w * self.ratio - splitter_width / 2.0;
                [
                    Rect::from_min_size(
                        viewport.min,
                        Vec2::new(split_x, h),
                    ),
                    Rect::from_min_size(
                        Pos2::new(viewport.min.x + split_x + splitter_width, viewport.min.y),
                        Vec2::new(w - split_x - splitter_width, h),
                    ),
                ]
            }
            SplitDirection::Horizontal => {
                let split_y = h * self.ratio - splitter_width / 2.0;
                [
                    Rect::from_min_size(
                        viewport.min,
                        Vec2::new(w, split_y),
                    ),
                    Rect::from_min_size(
                        Pos2::new(viewport.min.x, viewport.min.y + split_y + splitter_width),
                        Vec2::new(w, h - split_y - splitter_width),
                    ),
                ]
            }
        }
    }

    /// Calculate splitter rectangle
    pub fn splitter_rect(&self, viewport: Rect) -> Rect {
        let (w, h) = (viewport.width(), viewport.height());
        let splitter_width = 4.0;

        match self.direction {
            SplitDirection::Vertical => {
                let split_x = w * self.ratio - splitter_width / 2.0;
                Rect::from_min_size(
                    Pos2::new(viewport.min.x + split_x, viewport.min.y),
                    Vec2::new(splitter_width, h),
                )
            }
            SplitDirection::Horizontal => {
                let split_y = h * self.ratio - splitter_width / 2.0;
                Rect::from_min_size(
                    Pos2::new(viewport.min.x, viewport.min.y + split_y),
                    Vec2::new(w, splitter_width),
                )
            }
        }
    }

    /// Apply zoom to a pane (with optional sync)
    pub fn apply_zoom(&mut self, delta: f32, pane_idx: usize) {
        self.panes[pane_idx].zoom = (self.panes[pane_idx].zoom * (1.0 + delta)).clamp(0.1, 10.0);

        if self.sync_zoom {
            let other = 1 - pane_idx;
            self.panes[other].zoom = self.panes[pane_idx].zoom;
        }
    }

    /// Apply pan to a pane (with optional sync)
    pub fn apply_pan(&mut self, delta: Vec2, pane_idx: usize) {
        self.panes[pane_idx].pan += delta;

        if self.sync_pan {
            let other = 1 - pane_idx;
            self.panes[other].pan += delta;
        }
    }

    /// Reset view for all panes
    pub fn reset_view(&mut self) {
        for pane in &mut self.panes {
            pane.zoom = 1.0;
            pane.pan = Vec2::ZERO;
        }
    }

    /// Toggle sync mode
    pub fn toggle_sync(&mut self) {
        self.sync_zoom = !self.sync_zoom;
        self.sync_pan = self.sync_zoom;
    }

    /// UI rendering and interaction
    pub fn ui(&mut self, ui: &mut egui::Ui, viewport: Rect) -> SplitViewResponse {
        let mut response = SplitViewResponse::default();

        if !self.enabled {
            return response;
        }

        let rects = self.calculate_rects(viewport);
        let splitter = self.splitter_rect(viewport);

        // Draw panes
        for (i, rect) in rects.iter().enumerate() {
            let is_active = self.active_pane == i;

            // Border
            let stroke = if is_active {
                egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE)
            } else {
                egui::Stroke::new(1.0, egui::Color32::DARK_GRAY)
            };
            ui.painter().rect_stroke(*rect, 0.0, stroke);

            // Click to activate
            let pane_response = ui.allocate_rect(*rect, egui::Sense::click_and_drag());
            if pane_response.clicked() {
                self.active_pane = i;
                response.active_changed = true;
            }

            // Drag to pan
            if pane_response.dragged() {
                self.apply_pan(pane_response.drag_delta(), i);
                response.pan_changed = true;
            }

            // Scroll to zoom
            if pane_response.hovered() {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll.abs() > 0.0 {
                    self.apply_zoom(scroll * 0.001, i);
                    response.zoom_changed = true;
                }
            }
        }

        // Splitter interaction
        let splitter_response = ui.allocate_rect(splitter, egui::Sense::drag());

        // Splitter cursor
        if splitter_response.hovered() {
            ui.ctx().set_cursor_icon(match self.direction {
                SplitDirection::Vertical => egui::CursorIcon::ResizeHorizontal,
                SplitDirection::Horizontal => egui::CursorIcon::ResizeVertical,
            });
        }

        // Drag splitter to adjust ratio
        if splitter_response.dragged() {
            let delta = splitter_response.drag_delta();
            match self.direction {
                SplitDirection::Vertical => {
                    self.ratio = (self.ratio + delta.x / viewport.width()).clamp(0.1, 0.9);
                }
                SplitDirection::Horizontal => {
                    self.ratio = (self.ratio + delta.y / viewport.height()).clamp(0.1, 0.9);
                }
            }
            response.ratio_changed = true;
        }

        // Draw splitter
        let splitter_color = if splitter_response.hovered() || splitter_response.dragged() {
            egui::Color32::from_gray(120)
        } else {
            egui::Color32::from_gray(80)
        };
        ui.painter().rect_filled(splitter, 0.0, splitter_color);

        response.rects = rects;
        response
    }

    /// Get status text for display
    pub fn status_text(&self) -> String {
        if self.enabled {
            let dir = match self.direction {
                SplitDirection::Horizontal => "H",
                SplitDirection::Vertical => "V",
            };
            let sync = if self.sync_zoom { "Sync" } else { "Async" };
            format!("Split:{} {} Active:{}", dir, sync, self.active_pane + 1)
        } else {
            String::new()
        }
    }
}

/// Response from SplitView UI
pub struct SplitViewResponse {
    pub rects: [Rect; 2],
    pub active_changed: bool,
    pub ratio_changed: bool,
    pub zoom_changed: bool,
    pub pan_changed: bool,
}

impl Default for SplitViewResponse {
    fn default() -> Self {
        Self {
            rects: [Rect::NOTHING, Rect::NOTHING],
            active_changed: false,
            ratio_changed: false,
            zoom_changed: false,
            pan_changed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_view_toggle() {
        let mut view = SplitView::new();
        assert!(!view.enabled);
        view.toggle();
        assert!(view.enabled);
        view.toggle();
        assert!(!view.enabled);
    }

    #[test]
    fn test_sync_zoom() {
        let mut view = SplitView::new();
        view.sync_zoom = true;
        view.apply_zoom(0.1, 0);
        assert!((view.panes[0].zoom - view.panes[1].zoom).abs() < 0.001);
    }

    #[test]
    fn test_calculate_rects() {
        let view = SplitView::new();
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let rects = view.calculate_rects(viewport);

        // Both rects should be within viewport
        assert!(rects[0].width() > 0.0);
        assert!(rects[1].width() > 0.0);
    }
}
