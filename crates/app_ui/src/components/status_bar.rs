//! Status bar component

use egui::Ui;

/// Status bar information
#[derive(Debug, Clone, Default)]
pub struct StatusInfo {
    /// Current file name
    pub file_name: String,

    /// Current index / total
    pub position: String,

    /// Image dimensions
    pub dimensions: String,

    /// File size
    pub file_size: String,

    /// Zoom level
    pub zoom: String,

    /// Additional status message
    pub message: String,
}

/// Status bar component
pub struct StatusBar;

impl StatusBar {
    /// Render the status bar
    pub fn ui(ui: &mut Ui, info: &StatusInfo) {
        ui.horizontal(|ui| {
            // File name
            ui.label(&info.file_name);

            ui.separator();

            // Position
            if !info.position.is_empty() {
                ui.label(&info.position);
                ui.separator();
            }

            // Dimensions
            if !info.dimensions.is_empty() {
                ui.label(&info.dimensions);
                ui.separator();
            }

            // File size
            if !info.file_size.is_empty() {
                ui.label(&info.file_size);
                ui.separator();
            }

            // Zoom
            if !info.zoom.is_empty() {
                ui.label(format!("Zoom: {}", info.zoom));
            }

            // Spacer
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Message on the right
                if !info.message.is_empty() {
                    ui.label(&info.message);
                }
            });
        });
    }
}
