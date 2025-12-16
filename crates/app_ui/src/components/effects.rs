//! Viewer effects: rotation, flip, background, transitions

use std::time::{Duration, Instant};

/// Image rotation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    None,
    Cw90,
    Cw180,
    Cw270,
}

/// Image transformation parameters
#[derive(Clone, Copy, Debug, Default)]
pub struct ImageTransform {
    pub rotation: Rotation,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl ImageTransform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rotate_cw(&mut self) {
        self.rotation = match self.rotation {
            Rotation::None => Rotation::Cw90,
            Rotation::Cw90 => Rotation::Cw180,
            Rotation::Cw180 => Rotation::Cw270,
            Rotation::Cw270 => Rotation::None,
        };
    }

    pub fn rotate_ccw(&mut self) {
        self.rotation = match self.rotation {
            Rotation::None => Rotation::Cw270,
            Rotation::Cw90 => Rotation::None,
            Rotation::Cw180 => Rotation::Cw90,
            Rotation::Cw270 => Rotation::Cw180,
        };
    }

    pub fn toggle_flip_h(&mut self) {
        self.flip_horizontal = !self.flip_horizontal;
    }

    pub fn toggle_flip_v(&mut self) {
        self.flip_vertical = !self.flip_vertical;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Get rotation angle in degrees
    pub fn rotation_degrees(&self) -> f32 {
        match self.rotation {
            Rotation::None => 0.0,
            Rotation::Cw90 => 90.0,
            Rotation::Cw180 => 180.0,
            Rotation::Cw270 => 270.0,
        }
    }

    /// Get rotation angle in radians
    pub fn rotation_radians(&self) -> f32 {
        self.rotation_degrees().to_radians()
    }

    /// Transform size based on rotation
    pub fn transform_size(&self, width: u32, height: u32) -> (u32, u32) {
        match self.rotation {
            Rotation::None | Rotation::Cw180 => (width, height),
            Rotation::Cw90 | Rotation::Cw270 => (height, width),
        }
    }

    /// Get UV coordinates for transformed image
    pub fn get_uv_rect(&self) -> egui::Rect {
        let (mut u0, mut u1) = (0.0, 1.0);
        let (mut v0, mut v1) = (0.0, 1.0);

        if self.flip_horizontal {
            std::mem::swap(&mut u0, &mut u1);
        }
        if self.flip_vertical {
            std::mem::swap(&mut v0, &mut v1);
        }

        egui::Rect::from_min_max(egui::pos2(u0, v0), egui::pos2(u1, v1))
    }

    /// Check if any transformation is applied
    pub fn is_identity(&self) -> bool {
        self.rotation == Rotation::None && !self.flip_horizontal && !self.flip_vertical
    }

    /// Get status text
    pub fn status_text(&self) -> String {
        let mut parts = Vec::new();
        if self.rotation != Rotation::None {
            parts.push(format!("{}°", self.rotation_degrees() as i32));
        }
        if self.flip_horizontal {
            parts.push("H-flip".to_string());
        }
        if self.flip_vertical {
            parts.push("V-flip".to_string());
        }
        if parts.is_empty() {
            String::new()
        } else {
            parts.join(" ")
        }
    }
}

/// Background color options
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BackgroundColor {
    #[default]
    Black,
    White,
    Gray(u8),
    Checkerboard,
    Custom(egui::Color32),
}

/// Viewer background settings
#[derive(Clone, Copy, Debug)]
pub struct ViewerBackground {
    pub color: BackgroundColor,
    pub checkerboard_size: u32,
}

impl Default for ViewerBackground {
    fn default() -> Self {
        Self {
            color: BackgroundColor::Black,
            checkerboard_size: 16,
        }
    }
}

impl ViewerBackground {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cycle(&mut self) {
        self.color = match self.color {
            BackgroundColor::Black => BackgroundColor::White,
            BackgroundColor::White => BackgroundColor::Gray(128),
            BackgroundColor::Gray(_) => BackgroundColor::Checkerboard,
            BackgroundColor::Checkerboard => BackgroundColor::Black,
            BackgroundColor::Custom(_) => BackgroundColor::Black,
        };
    }

    pub fn to_egui_color(&self) -> egui::Color32 {
        match self.color {
            BackgroundColor::Black => egui::Color32::BLACK,
            BackgroundColor::White => egui::Color32::WHITE,
            BackgroundColor::Gray(v) => egui::Color32::from_gray(v),
            BackgroundColor::Checkerboard => egui::Color32::TRANSPARENT,
            BackgroundColor::Custom(c) => c,
        }
    }

    /// Draw checkerboard pattern for transparency
    pub fn draw_checkerboard(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        if self.color != BackgroundColor::Checkerboard {
            return;
        }

        let size = self.checkerboard_size as f32;
        let colors = [
            egui::Color32::from_gray(200),
            egui::Color32::from_gray(150),
        ];

        let cols = (rect.width() / size).ceil() as i32;
        let rows = (rect.height() / size).ceil() as i32;

        for row in 0..rows {
            for col in 0..cols {
                let color = colors[((row + col) % 2) as usize];
                let tile_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        rect.min.x + col as f32 * size,
                        rect.min.y + row as f32 * size,
                    ),
                    egui::vec2(size, size),
                ).intersect(rect);

                ui.painter().rect_filled(tile_rect, 0.0, color);
            }
        }
    }

    /// Get status text
    pub fn status_text(&self) -> &'static str {
        match self.color {
            BackgroundColor::Black => "BG:Black",
            BackgroundColor::White => "BG:White",
            BackgroundColor::Gray(_) => "BG:Gray",
            BackgroundColor::Checkerboard => "BG:Check",
            BackgroundColor::Custom(_) => "BG:Custom",
        }
    }
}

/// Transition type for page changes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TransitionType {
    #[default]
    None,
    Fade,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
}

/// Page transition animation
pub struct PageTransition {
    pub transition_type: TransitionType,
    pub duration: Duration,
    start_time: Option<Instant>,
    from_texture: Option<egui::TextureId>,
    to_texture: Option<egui::TextureId>,
}

impl Default for PageTransition {
    fn default() -> Self {
        Self::new()
    }
}

impl PageTransition {
    pub fn new() -> Self {
        Self {
            transition_type: TransitionType::None,
            duration: Duration::from_millis(200),
            start_time: None,
            from_texture: None,
            to_texture: None,
        }
    }

    pub fn start(&mut self, from: Option<egui::TextureId>, to: Option<egui::TextureId>) {
        if self.transition_type == TransitionType::None {
            return;
        }
        self.from_texture = from;
        self.to_texture = to;
        self.start_time = Some(Instant::now());
    }

    pub fn is_active(&self) -> bool {
        if let Some(start) = self.start_time {
            start.elapsed() < self.duration
        } else {
            false
        }
    }

    pub fn progress(&self) -> f32 {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f32();
            let total = self.duration.as_secs_f32();
            (elapsed / total).min(1.0)
        } else {
            1.0
        }
    }

    /// Ease-out cubic function
    fn ease_out(t: f32) -> f32 {
        1.0 - (1.0 - t).powi(3)
    }

    pub fn render(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        if !self.is_active() {
            return;
        }

        let t = Self::ease_out(self.progress());

        match self.transition_type {
            TransitionType::Fade => {
                if let Some(from) = self.from_texture {
                    let alpha = ((1.0 - t) * 255.0) as u8;
                    ui.painter().image(
                        from,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                    );
                }
            }
            TransitionType::SlideLeft => {
                let offset = rect.width() * (1.0 - t);
                if let Some(from) = self.from_texture {
                    let from_rect = rect.translate(egui::vec2(-offset, 0.0));
                    ui.painter().image(
                        from,
                        from_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            TransitionType::SlideRight => {
                let offset = rect.width() * (1.0 - t);
                if let Some(from) = self.from_texture {
                    let from_rect = rect.translate(egui::vec2(offset, 0.0));
                    ui.painter().image(
                        from,
                        from_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            TransitionType::SlideUp => {
                let offset = rect.height() * (1.0 - t);
                if let Some(from) = self.from_texture {
                    let from_rect = rect.translate(egui::vec2(0.0, -offset));
                    ui.painter().image(
                        from,
                        from_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            TransitionType::SlideDown => {
                let offset = rect.height() * (1.0 - t);
                if let Some(from) = self.from_texture {
                    let from_rect = rect.translate(egui::vec2(0.0, offset));
                    ui.painter().image(
                        from,
                        from_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
            }
            TransitionType::None => {}
        }
    }

    pub fn cycle_type(&mut self) {
        self.transition_type = match self.transition_type {
            TransitionType::None => TransitionType::Fade,
            TransitionType::Fade => TransitionType::SlideLeft,
            TransitionType::SlideLeft => TransitionType::SlideRight,
            TransitionType::SlideRight => TransitionType::None,
            _ => TransitionType::None,
        };
    }

    pub fn clear(&mut self) {
        self.start_time = None;
        self.from_texture = None;
        self.to_texture = None;
    }

    /// Get status text
    pub fn status_text(&self) -> &'static str {
        match self.transition_type {
            TransitionType::None => "Trans:Off",
            TransitionType::Fade => "Trans:Fade",
            TransitionType::SlideLeft => "Trans:SlideL",
            TransitionType::SlideRight => "Trans:SlideR",
            TransitionType::SlideUp => "Trans:SlideU",
            TransitionType::SlideDown => "Trans:SlideD",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_cw() {
        let mut transform = ImageTransform::new();
        assert_eq!(transform.rotation, Rotation::None);

        transform.rotate_cw();
        assert_eq!(transform.rotation, Rotation::Cw90);

        transform.rotate_cw();
        assert_eq!(transform.rotation, Rotation::Cw180);

        transform.rotate_cw();
        assert_eq!(transform.rotation, Rotation::Cw270);

        transform.rotate_cw();
        assert_eq!(transform.rotation, Rotation::None);
    }

    #[test]
    fn test_transform_size() {
        let mut transform = ImageTransform::new();
        assert_eq!(transform.transform_size(100, 200), (100, 200));

        transform.rotation = Rotation::Cw90;
        assert_eq!(transform.transform_size(100, 200), (200, 100));
    }

    #[test]
    fn test_background_cycle() {
        let mut bg = ViewerBackground::new();
        assert_eq!(bg.color, BackgroundColor::Black);

        bg.cycle();
        assert_eq!(bg.color, BackgroundColor::White);

        bg.cycle();
        assert!(matches!(bg.color, BackgroundColor::Gray(_)));
    }
}
