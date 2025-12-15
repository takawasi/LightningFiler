//! Application theming

use egui::{Color32, Style, Visuals};

/// Application theme
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub background: Color32,
    pub surface: Color32,
    pub primary: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub accent: Color32,
    pub error: Color32,
    pub warning: Color32,
    pub success: Color32,
}

impl Theme {
    /// Dark theme (default)
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            background: Color32::from_rgb(32, 32, 32),
            surface: Color32::from_rgb(48, 48, 48),
            primary: Color32::from_rgb(64, 64, 64),
            text: Color32::from_rgb(240, 240, 240),
            text_secondary: Color32::from_rgb(160, 160, 160),
            accent: Color32::from_rgb(100, 149, 237), // Cornflower blue
            error: Color32::from_rgb(220, 80, 80),
            warning: Color32::from_rgb(220, 180, 80),
            success: Color32::from_rgb(80, 200, 120),
        }
    }

    /// Light theme
    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            background: Color32::from_rgb(250, 250, 250),
            surface: Color32::from_rgb(255, 255, 255),
            primary: Color32::from_rgb(230, 230, 230),
            text: Color32::from_rgb(32, 32, 32),
            text_secondary: Color32::from_rgb(100, 100, 100),
            accent: Color32::from_rgb(59, 130, 246), // Blue
            error: Color32::from_rgb(220, 38, 38),
            warning: Color32::from_rgb(234, 179, 8),
            success: Color32::from_rgb(34, 197, 94),
        }
    }

    /// Apply theme to egui
    pub fn apply(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        let mut visuals = if self.name == "dark" {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        // Customize visuals
        visuals.panel_fill = self.surface;
        visuals.window_fill = self.surface;
        visuals.extreme_bg_color = self.background;
        visuals.faint_bg_color = self.primary;

        visuals.widgets.noninteractive.bg_fill = self.surface;
        visuals.widgets.noninteractive.fg_stroke.color = self.text;

        visuals.widgets.inactive.bg_fill = self.primary;
        visuals.widgets.inactive.fg_stroke.color = self.text;

        visuals.widgets.hovered.bg_fill = self.accent.linear_multiply(0.3);
        visuals.widgets.hovered.fg_stroke.color = self.text;

        visuals.widgets.active.bg_fill = self.accent.linear_multiply(0.5);
        visuals.widgets.active.fg_stroke.color = self.text;

        visuals.selection.bg_fill = self.accent.linear_multiply(0.3);
        visuals.selection.stroke.color = self.accent;

        style.visuals = visuals;
        ctx.set_style(style);
    }

    /// Get theme by name
    pub fn by_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "light" => Self::light(),
            _ => Self::dark(),
        }
    }

    /// Parse a hex color string
    pub fn parse_color(hex: &str) -> Option<Color32> {
        let hex = hex.trim_start_matches('#');

        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color32::from_rgb(r, g, b))
        } else if hex.len() == 8 {
            let a = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let r = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let g = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let b = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color32::from_rgba_unmultiplied(r, g, b, a))
        } else {
            None
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
