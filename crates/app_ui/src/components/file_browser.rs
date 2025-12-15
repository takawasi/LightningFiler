//! File browser component (grid/list view)

use egui::{Ui, Vec2, Response};

/// File browser component
pub struct FileBrowser {
    /// Thumbnail size
    pub thumbnail_size: f32,

    /// View mode
    pub view_mode: BrowserViewMode,

    /// Selected index
    pub selected: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserViewMode {
    Grid,
    List,
    Details,
}

impl FileBrowser {
    pub fn new() -> Self {
        Self {
            thumbnail_size: 128.0,
            view_mode: BrowserViewMode::Grid,
            selected: None,
        }
    }

    /// Render the file browser
    pub fn ui(&mut self, ui: &mut Ui, items: &[FileItem]) -> Option<BrowserAction> {
        let mut action = None;

        match self.view_mode {
            BrowserViewMode::Grid => {
                action = self.render_grid(ui, items);
            }
            BrowserViewMode::List => {
                action = self.render_list(ui, items);
            }
            BrowserViewMode::Details => {
                action = self.render_details(ui, items);
            }
        }

        action
    }

    fn render_grid(&mut self, ui: &mut Ui, items: &[FileItem]) -> Option<BrowserAction> {
        let mut action = None;
        let available_width = ui.available_width();
        let item_width = self.thumbnail_size + 16.0;
        let columns = (available_width / item_width).max(1.0) as usize;

        egui::Grid::new("file_grid")
            .num_columns(columns)
            .spacing(Vec2::splat(8.0))
            .show(ui, |ui| {
                for (idx, item) in items.iter().enumerate() {
                    let is_selected = self.selected == Some(idx);

                    let response = self.render_grid_item(ui, item, is_selected);

                    if response.clicked() {
                        self.selected = Some(idx);
                        action = Some(BrowserAction::Select(idx));
                    }

                    if response.double_clicked() {
                        action = Some(BrowserAction::Open(idx));
                    }

                    if (idx + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });

        action
    }

    fn render_grid_item(&self, ui: &mut Ui, item: &FileItem, selected: bool) -> Response {
        let size = Vec2::splat(self.thumbnail_size);

        ui.vertical(|ui| {
            ui.set_width(size.x + 8.0);

            // Thumbnail area
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

            if selected {
                ui.painter().rect_filled(
                    rect.expand(2.0),
                    4.0,
                    ui.visuals().selection.bg_fill,
                );
            }

            // Placeholder for thumbnail
            ui.painter().rect_filled(
                rect,
                4.0,
                if item.is_dir {
                    egui::Color32::from_rgb(100, 140, 180)
                } else {
                    egui::Color32::from_rgb(80, 80, 80)
                },
            );

            // Icon for folders
            if item.is_dir {
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "📁",
                    egui::FontId::proportional(32.0),
                    egui::Color32::WHITE,
                );
            }

            // File name (truncated)
            let name = if item.name.len() > 20 {
                format!("{}...", &item.name[..17])
            } else {
                item.name.clone()
            };

            ui.add(
                egui::Label::new(name)
                    .wrap_mode(egui::TextWrapMode::Truncate)
            );

            response
        }).inner
    }

    fn render_list(&mut self, ui: &mut Ui, items: &[FileItem]) -> Option<BrowserAction> {
        let mut action = None;

        for (idx, item) in items.iter().enumerate() {
            let is_selected = self.selected == Some(idx);

            let response = ui.horizontal(|ui| {
                if is_selected {
                    ui.visuals_mut().override_text_color = Some(ui.visuals().selection.stroke.color);
                }

                // Icon
                let icon = if item.is_dir { "📁" } else { "📄" };
                ui.label(icon);

                // Name
                let response = ui.selectable_label(is_selected, &item.name);

                response
            }).inner;

            if response.clicked() {
                self.selected = Some(idx);
                action = Some(BrowserAction::Select(idx));
            }

            if response.double_clicked() {
                action = Some(BrowserAction::Open(idx));
            }
        }

        action
    }

    fn render_details(&mut self, ui: &mut Ui, items: &[FileItem]) -> Option<BrowserAction> {
        let mut action = None;

        egui::Grid::new("details_grid")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                // Header
                ui.strong("Name");
                ui.strong("Size");
                ui.strong("Modified");
                ui.strong("Type");
                ui.end_row();

                for (idx, item) in items.iter().enumerate() {
                    let is_selected = self.selected == Some(idx);

                    let response = ui.selectable_label(is_selected, &item.name);
                    ui.label(format_size(item.size));
                    ui.label(format_date(item.modified));
                    ui.label(if item.is_dir { "Folder" } else { &item.extension });
                    ui.end_row();

                    if response.clicked() {
                        self.selected = Some(idx);
                        action = Some(BrowserAction::Select(idx));
                    }

                    if response.double_clicked() {
                        action = Some(BrowserAction::Open(idx));
                    }
                }
            });

        action
    }
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new()
    }
}

/// File item for display
#[derive(Debug, Clone)]
pub struct FileItem {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<i64>,
    pub extension: String,
    pub thumbnail: Option<egui::TextureId>,
}

/// Browser action
#[derive(Debug, Clone)]
pub enum BrowserAction {
    Select(usize),
    Open(usize),
    ContextMenu(usize),
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_date(timestamp: Option<i64>) -> String {
    timestamp
        .map(|ts| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Invalid".to_string())
        })
        .unwrap_or_else(|| "-".to_string())
}
