//! Settings dialog component

use app_core::AppConfig;
use egui::{Color32, ComboBox, Slider, Ui};

/// Settings dialog state
pub struct SettingsDialog {
    /// Currently open
    pub open: bool,
    /// Current tab
    pub current_tab: SettingsTab,
    /// Working copy of config (for Apply/Cancel functionality)
    pub working_config: AppConfig,
    /// Whether any changes have been made
    pub modified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Viewer,
    Navigation,
    Keybinds,
}

/// Actions from settings dialog
pub enum SettingsAction {
    /// Apply and save changes
    Apply,
    /// Apply and close
    Ok,
    /// Cancel and discard changes
    Cancel,
}

impl SettingsDialog {
    pub fn new(config: AppConfig) -> Self {
        Self {
            open: false,
            current_tab: SettingsTab::General,
            working_config: config,
            modified: false,
        }
    }

    /// Open the settings dialog with a specific tab
    pub fn open(&mut self, config: AppConfig, tab: Option<SettingsTab>) {
        self.open = true;
        self.working_config = config;
        self.modified = false;
        if let Some(tab) = tab {
            self.current_tab = tab;
        }
    }

    /// Close the dialog
    pub fn close(&mut self) {
        self.open = false;
        self.modified = false;
    }

    /// Render the settings dialog
    /// Returns Some(action) if a button was clicked
    pub fn ui(&mut self, ctx: &egui::Context) -> Option<SettingsAction> {
        if !self.open {
            return None;
        }

        let mut action = None;
        let mut window_open = true;

        egui::Window::new("Settings")
            .open(&mut window_open)
            .resizable(true)
            .default_size([600.0, 500.0])
            .collapsible(false)
            .show(ctx, |ui| {
                // Tab bar
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.current_tab, SettingsTab::General, "General");
                    ui.selectable_value(&mut self.current_tab, SettingsTab::Viewer, "Viewer");
                    ui.selectable_value(&mut self.current_tab, SettingsTab::Navigation, "Navigation");
                    ui.selectable_value(&mut self.current_tab, SettingsTab::Keybinds, "Keybinds");
                });

                ui.separator();

                // Tab content
                egui::ScrollArea::vertical().show(ui, |ui| {
                    match self.current_tab {
                        SettingsTab::General => self.ui_general_tab(ui),
                        SettingsTab::Viewer => self.ui_viewer_tab(ui),
                        SettingsTab::Navigation => self.ui_navigation_tab(ui),
                        SettingsTab::Keybinds => self.ui_keybinds_tab(ui),
                    }
                });

                ui.separator();

                // Bottom buttons
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        action = Some(SettingsAction::Ok);
                    }
                    if ui.button("Apply").clicked() {
                        action = Some(SettingsAction::Apply);
                    }
                    if ui.button("Cancel").clicked() {
                        action = Some(SettingsAction::Cancel);
                    }

                    // Show modified indicator
                    if self.modified {
                        ui.label(
                            egui::RichText::new("(Modified)")
                                .color(Color32::YELLOW)
                                .italics(),
                        );
                    }
                });
            });

        // If window was closed by clicking X, treat as Cancel
        if !window_open && action.is_none() {
            action = Some(SettingsAction::Cancel);
        }

        // Update self.open based on window_open
        self.open = window_open;

        action
    }

    fn ui_general_tab(&mut self, ui: &mut Ui) {
        ui.heading("General Settings");
        ui.add_space(10.0);

        egui::Grid::new("general_grid")
            .num_columns(2)
            .spacing([40.0, 10.0])
            .show(ui, |ui| {
                // Language
                ui.label("Language:");
                let current_lang = self.working_config.general.language.clone();
                ComboBox::from_id_salt("language")
                    .selected_text(&current_lang)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut self.working_config.general.language, "ja".to_string(), "Japanese").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.general.language, "en".to_string(), "English").clicked() {
                            self.modified = true;
                        }
                    });
                ui.end_row();

                // Theme
                ui.label("Theme:");
                let current_theme = self.working_config.general.theme.clone();
                ComboBox::from_id_salt("theme")
                    .selected_text(&current_theme)
                    .show_ui(ui, |ui| {
                        if ui.selectable_value(&mut self.working_config.general.theme, "dark".to_string(), "Dark").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.general.theme, "light".to_string(), "Light").clicked() {
                            self.modified = true;
                        }
                    });
                ui.end_row();

                // Start Maximized
                ui.label("Start Maximized:");
                if ui.checkbox(&mut self.working_config.general.start_maximized, "").changed() {
                    self.modified = true;
                }
                ui.end_row();

                // Remember Window State
                ui.label("Remember Window State:");
                if ui.checkbox(&mut self.working_config.general.remember_window_state, "").changed() {
                    self.modified = true;
                }
                ui.end_row();

                // Check Updates
                ui.label("Check for Updates:");
                if ui.checkbox(&mut self.working_config.general.check_updates, "").changed() {
                    self.modified = true;
                }
                ui.end_row();
            });
    }

    fn ui_viewer_tab(&mut self, ui: &mut Ui) {
        ui.heading("Viewer Settings");
        ui.add_space(10.0);

        egui::Grid::new("viewer_grid")
            .num_columns(2)
            .spacing([40.0, 10.0])
            .show(ui, |ui| {
                // Background Color
                ui.label("Background Color:");
                ui.horizontal(|ui| {
                    // Parse current color
                    let mut color = parse_hex_color(&self.working_config.viewer.background_color);

                    if ui.color_edit_button_srgba(&mut color).changed() {
                        self.working_config.viewer.background_color = format!(
                            "#{:02X}{:02X}{:02X}",
                            color.r(),
                            color.g(),
                            color.b()
                        );
                        self.modified = true;
                    }

                    ui.label(&self.working_config.viewer.background_color);
                });
                ui.end_row();

                // Fit Mode
                ui.label("Fit Mode:");
                let current_fit = format!("{:?}", self.working_config.viewer.fit_mode);
                ComboBox::from_id_salt("fit_mode")
                    .selected_text(&current_fit)
                    .show_ui(ui, |ui| {
                        use app_core::FitMode;
                        if ui.selectable_value(&mut self.working_config.viewer.fit_mode, FitMode::FitToWindow, "Fit to Window").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.fit_mode, FitMode::FitWidth, "Fit Width").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.fit_mode, FitMode::FitHeight, "Fit Height").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.fit_mode, FitMode::OriginalSize, "Original Size").clicked() {
                            self.modified = true;
                        }
                    });
                ui.end_row();

                // Interpolation
                ui.label("Interpolation:");
                let current_interp = format!("{:?}", self.working_config.viewer.interpolation);
                ComboBox::from_id_salt("interpolation")
                    .selected_text(&current_interp)
                    .show_ui(ui, |ui| {
                        use app_core::Interpolation;
                        if ui.selectable_value(&mut self.working_config.viewer.interpolation, Interpolation::Nearest, "Nearest (Fast)").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.interpolation, Interpolation::Bilinear, "Bilinear (Balanced)").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.interpolation, Interpolation::Lanczos3, "Lanczos3 (High Quality)").clicked() {
                            self.modified = true;
                        }
                    });
                ui.end_row();

                // Spread Mode
                ui.label("Spread Mode:");
                let current_spread = format!("{:?}", self.working_config.viewer.spread_mode);
                ComboBox::from_id_salt("spread_mode")
                    .selected_text(&current_spread)
                    .show_ui(ui, |ui| {
                        use app_core::SpreadMode;
                        if ui.selectable_value(&mut self.working_config.viewer.spread_mode, SpreadMode::Single, "Single Page").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.spread_mode, SpreadMode::Spread, "Spread (2 Pages)").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.spread_mode, SpreadMode::Auto, "Auto").clicked() {
                            self.modified = true;
                        }
                    });
                ui.end_row();

                // Reading Direction
                ui.label("Reading Direction:");
                let current_dir = match self.working_config.viewer.reading_direction {
                    app_core::ReadingDirection::LeftToRight => "Left to Right",
                    app_core::ReadingDirection::RightToLeft => "Right to Left",
                };
                ComboBox::from_id_salt("reading_direction")
                    .selected_text(current_dir)
                    .show_ui(ui, |ui| {
                        use app_core::ReadingDirection;
                        if ui.selectable_value(&mut self.working_config.viewer.reading_direction, ReadingDirection::LeftToRight, "Left to Right").clicked() {
                            self.modified = true;
                        }
                        if ui.selectable_value(&mut self.working_config.viewer.reading_direction, ReadingDirection::RightToLeft, "Right to Left").clicked() {
                            self.modified = true;
                        }
                    });
                ui.end_row();

                // Slideshow Interval
                ui.label("Slideshow Interval (ms):");
                let mut interval = self.working_config.viewer.slideshow_interval_ms as f64;
                if ui.add(Slider::new(&mut interval, 500.0..=10000.0).step_by(100.0)).changed() {
                    self.working_config.viewer.slideshow_interval_ms = interval as u64;
                    self.modified = true;
                }
                ui.end_row();

                // Enable Animation
                ui.label("Enable Animation:");
                if ui.checkbox(&mut self.working_config.viewer.enable_animation, "").changed() {
                    self.modified = true;
                }
                ui.end_row();

                // Preload Count
                ui.label("Preload Count:");
                let mut preload = self.working_config.viewer.preload_count as f64;
                if ui.add(Slider::new(&mut preload, 0.0..=10.0).step_by(1.0)).changed() {
                    self.working_config.viewer.preload_count = preload as usize;
                    self.modified = true;
                }
                ui.end_row();
            });
    }

    fn ui_navigation_tab(&mut self, ui: &mut Ui) {
        ui.heading("Navigation Settings");
        ui.add_space(10.0);

        egui::Grid::new("navigation_grid")
            .num_columns(2)
            .spacing([40.0, 10.0])
            .show(ui, |ui| {
                // Enter Threshold
                ui.label("Enter Threshold:");
                ui.horizontal(|ui| {
                    let mut threshold = self.working_config.navigation.enter_threshold.unwrap_or(5) as f64;
                    if ui.add(Slider::new(&mut threshold, 1.0..=20.0).step_by(1.0)).changed() {
                        self.working_config.navigation.enter_threshold = Some(threshold as i32);
                        self.modified = true;
                    }
                    ui.label("files");
                });
                ui.end_row();

                ui.label("");
                ui.label("(≤ threshold: Viewer mode, > threshold: Browser mode)")
                    .on_hover_text("When entering a folder with few files, automatically switch to Viewer mode");
                ui.end_row();

                // Skip Empty Folders
                ui.label("Skip Empty Folders:");
                if ui.checkbox(&mut self.working_config.navigation.skip_empty_folders, "")
                    .on_hover_text("Skip empty folders when navigating siblings")
                    .changed()
                {
                    self.modified = true;
                }
                ui.end_row();

                // Cross-Folder Navigation
                ui.label("Cross-Folder Navigation:");
                if ui.checkbox(&mut self.working_config.navigation.cross_folder_navigation, "")
                    .on_hover_text("Automatically advance to next/previous folder when reaching end of current folder")
                    .changed()
                {
                    self.modified = true;
                }
                ui.end_row();

                // Wrap Navigation
                ui.label("Wrap Navigation:");
                if ui.checkbox(&mut self.working_config.navigation.wrap_navigation, "")
                    .on_hover_text("Wrap around when reaching the end of a folder")
                    .changed()
                {
                    self.modified = true;
                }
                ui.end_row();
            });
    }

    fn ui_keybinds_tab(&mut self, ui: &mut Ui) {
        ui.heading("Keybind Settings");
        ui.add_space(10.0);

        ui.label("Command → Key Bindings");
        ui.separator();

        // Group keybindings by category
        let categories = [
            ("Navigation", "nav."),
            ("View", "view."),
            ("File", "file."),
            ("Meta", "meta."),
            ("App", "app."),
        ];

        for (category_name, prefix) in categories {
            ui.collapsing(category_name, |ui| {
                egui::Grid::new(format!("keybinds_{}", prefix))
                    .num_columns(2)
                    .spacing([20.0, 5.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Get sorted keys for this category
                        let mut keys: Vec<_> = self.working_config.keybindings
                            .keys()
                            .filter(|k| k.starts_with(prefix))
                            .cloned()
                            .collect();
                        keys.sort();

                        for key in keys {
                            ui.label(&key);

                            if let Some(bindings) = self.working_config.keybindings.get_mut(&key) {
                                let binding_text = bindings.join(", ");
                                let mut new_text = binding_text.clone();

                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut new_text)
                                        .desired_width(200.0)
                                        .hint_text("e.g., Ctrl+N, Down")
                                );

                                if response.changed() {
                                    // Parse the new bindings
                                    let new_bindings: Vec<String> = new_text
                                        .split(',')
                                        .map(|s| s.trim().to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    *bindings = new_bindings;
                                    self.modified = true;
                                }

                                if response.on_hover_text("Separate multiple keys with commas").changed() {
                                    // Already handled above
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        }

        ui.add_space(10.0);
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Reset to Defaults").clicked() {
                self.working_config.keybindings = app_core::AppConfig::default().keybindings;
                self.modified = true;
            }
        });
    }

    /// Get the current working config
    pub fn get_config(&self) -> &AppConfig {
        &self.working_config
    }

    /// Reset modifications flag
    pub fn reset_modified(&mut self) {
        self.modified = false;
    }
}

/// Parse hex color string to Color32
fn parse_hex_color(hex: &str) -> Color32 {
    let hex = hex.trim_start_matches('#');

    if hex.len() == 6 {
        if let Ok(r) = u8::from_str_radix(&hex[0..2], 16) {
            if let Ok(g) = u8::from_str_radix(&hex[2..4], 16) {
                if let Ok(b) = u8::from_str_radix(&hex[4..6], 16) {
                    return Color32::from_rgb(r, g, b);
                }
            }
        }
    }

    // Fallback to dark gray
    Color32::from_rgb(32, 32, 32)
}
