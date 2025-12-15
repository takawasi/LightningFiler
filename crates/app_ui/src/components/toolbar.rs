//! Toolbar component

use egui::Ui;

/// Toolbar component
pub struct Toolbar;

impl Toolbar {
    /// Render the toolbar
    pub fn ui(ui: &mut Ui) -> Option<ToolbarAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            // Navigation buttons
            if ui.button("⬅").on_hover_text("Previous").clicked() {
                action = Some(ToolbarAction::Previous);
            }

            if ui.button("➡").on_hover_text("Next").clicked() {
                action = Some(ToolbarAction::Next);
            }

            ui.separator();

            // View buttons
            if ui.button("⬆").on_hover_text("Up folder").clicked() {
                action = Some(ToolbarAction::UpFolder);
            }

            if ui.button("🏠").on_hover_text("Home").clicked() {
                action = Some(ToolbarAction::Home);
            }

            ui.separator();

            // Zoom controls
            if ui.button("🔍+").on_hover_text("Zoom in").clicked() {
                action = Some(ToolbarAction::ZoomIn);
            }

            if ui.button("🔍-").on_hover_text("Zoom out").clicked() {
                action = Some(ToolbarAction::ZoomOut);
            }

            if ui.button("1:1").on_hover_text("Original size").clicked() {
                action = Some(ToolbarAction::OriginalSize);
            }

            if ui.button("Fit").on_hover_text("Fit to window").clicked() {
                action = Some(ToolbarAction::FitToWindow);
            }

            ui.separator();

            // Rotation
            if ui.button("↺").on_hover_text("Rotate left").clicked() {
                action = Some(ToolbarAction::RotateLeft);
            }

            if ui.button("↻").on_hover_text("Rotate right").clicked() {
                action = Some(ToolbarAction::RotateRight);
            }

            ui.separator();

            // View mode
            if ui.button("⊞").on_hover_text("Grid view").clicked() {
                action = Some(ToolbarAction::GridView);
            }

            if ui.button("≡").on_hover_text("List view").clicked() {
                action = Some(ToolbarAction::ListView);
            }

            // Spacer
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    action = Some(ToolbarAction::Settings);
                }

                if ui.button("⛶").on_hover_text("Fullscreen").clicked() {
                    action = Some(ToolbarAction::Fullscreen);
                }
            });
        });

        action
    }
}

/// Toolbar actions
#[derive(Debug, Clone, Copy)]
pub enum ToolbarAction {
    Previous,
    Next,
    UpFolder,
    Home,
    ZoomIn,
    ZoomOut,
    OriginalSize,
    FitToWindow,
    RotateLeft,
    RotateRight,
    GridView,
    ListView,
    Settings,
    Fullscreen,
}
