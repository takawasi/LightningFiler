//! Toolbar component with navigation, path input, and file operations

use egui::{Ui, ComboBox};

/// Toolbar state for path editing
pub struct ToolbarState {
    /// Current path text for editing
    pub path_text: String,
    /// Is path being edited
    pub editing_path: bool,
    /// Current sort mode
    pub sort_mode: SortMode,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            path_text: String::new(),
            editing_path: false,
            sort_mode: SortMode::Name,
        }
    }
}

impl ToolbarState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_path(&mut self, path: &str) {
        if !self.editing_path {
            self.path_text = path.to_string();
        }
    }
}

/// Sort mode for file listing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Name,
    NameDesc,
    Size,
    SizeDesc,
    Modified,
    ModifiedDesc,
    Type,
    TypeDesc,
}

impl SortMode {
    pub fn label(&self) -> &'static str {
        match self {
            SortMode::Name => "Name ↑",
            SortMode::NameDesc => "Name ↓",
            SortMode::Size => "Size ↑",
            SortMode::SizeDesc => "Size ↓",
            SortMode::Modified => "Date ↑",
            SortMode::ModifiedDesc => "Date ↓",
            SortMode::Type => "Type ↑",
            SortMode::TypeDesc => "Type ↓",
        }
    }
}

/// Toolbar component
pub struct Toolbar;

impl Toolbar {
    /// Render the toolbar with full functionality
    pub fn ui(
        ui: &mut Ui,
        state: &mut ToolbarState,
        can_go_back: bool,
        can_go_forward: bool,
    ) -> Option<ToolbarAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            // === Navigation buttons ===
            ui.add_enabled_ui(can_go_back, |ui| {
                if ui.button("◀").on_hover_text("Back (Alt+←)").clicked() {
                    action = Some(ToolbarAction::Back);
                }
            });

            ui.add_enabled_ui(can_go_forward, |ui| {
                if ui.button("▶").on_hover_text("Forward (Alt+→)").clicked() {
                    action = Some(ToolbarAction::Forward);
                }
            });

            if ui.button("⬆").on_hover_text("Up folder").clicked() {
                action = Some(ToolbarAction::UpFolder);
            }

            if ui.button("🔄").on_hover_text("Refresh (F5)").clicked() {
                action = Some(ToolbarAction::Refresh);
            }

            ui.separator();

            // === Path input ===
            let path_response = ui.add_sized(
                [ui.available_width() - 300.0, 20.0],
                egui::TextEdit::singleline(&mut state.path_text)
                    .hint_text("Enter path...")
                    .font(egui::FontId::proportional(13.0))
            );

            if path_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                action = Some(ToolbarAction::NavigateTo(state.path_text.clone()));
                state.editing_path = false;
            }

            if path_response.gained_focus() {
                state.editing_path = true;
            }

            ui.separator();

            // === File operations ===
            if ui.button("📁+").on_hover_text("New folder").clicked() {
                action = Some(ToolbarAction::NewFolder);
            }

            if ui.button("📋").on_hover_text("Copy").clicked() {
                action = Some(ToolbarAction::Copy);
            }

            if ui.button("🗑").on_hover_text("Delete").clicked() {
                action = Some(ToolbarAction::Delete);
            }

            ui.separator();

            // === Sort dropdown ===
            ComboBox::from_id_salt("sort_combo")
                .selected_text(state.sort_mode.label())
                .width(80.0)
                .show_ui(ui, |ui| {
                    for mode in [
                        SortMode::Name, SortMode::NameDesc,
                        SortMode::Size, SortMode::SizeDesc,
                        SortMode::Modified, SortMode::ModifiedDesc,
                        SortMode::Type, SortMode::TypeDesc,
                    ] {
                        if ui.selectable_value(&mut state.sort_mode, mode, mode.label()).clicked() {
                            action = Some(ToolbarAction::Sort(mode));
                        }
                    }
                });

            // === Right-aligned buttons ===
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("⚙").on_hover_text("Settings").clicked() {
                    action = Some(ToolbarAction::Settings);
                }
            });
        });

        action
    }

    /// Simple toolbar for viewer mode (minimal)
    pub fn ui_simple(ui: &mut Ui) -> Option<ToolbarAction> {
        // Empty - viewer mode has no toolbar
        None
    }
}

/// Toolbar actions
#[derive(Debug, Clone)]
pub enum ToolbarAction {
    // Navigation
    Back,
    Forward,
    UpFolder,
    Refresh,
    NavigateTo(String),

    // File operations
    NewFolder,
    Copy,
    Delete,

    // Sort
    Sort(SortMode),

    // Settings
    Settings,

    // Legacy (for compatibility)
    Previous,
    Next,
    Home,
    ZoomIn,
    ZoomOut,
    OriginalSize,
    FitToWindow,
    RotateLeft,
    RotateRight,
    GridView,
    ListView,
    Fullscreen,
}
