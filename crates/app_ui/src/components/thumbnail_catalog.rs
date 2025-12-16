//! Thumbnail catalog component for right panel
//! Displays image thumbnails in a grid layout

use egui::{Ui, Vec2, Rect, Response, TextureHandle};
use std::path::PathBuf;

/// Action returned from thumbnail catalog interaction
#[derive(Debug, Clone)]
pub enum CatalogAction {
    /// User selected an item (single click)
    Select(usize),
    /// User wants to open an item (double click / Enter)
    Open(usize),
    /// User wants to go to parent folder
    GoToParent,
    /// Navigation action
    Navigate(NavigateDirection),
}

#[derive(Debug, Clone, Copy)]
pub enum NavigateDirection {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

/// A thumbnail item in the catalog
#[derive(Clone)]
pub struct ThumbnailItem {
    pub path: PathBuf,
    pub name: String,
    pub texture: Option<TextureHandle>,
    pub is_folder: bool,
    pub is_image: bool,
}

impl ThumbnailItem {
    pub fn new(path: PathBuf, is_folder: bool, is_image: bool) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        Self {
            path,
            name,
            texture: None,
            is_folder,
            is_image,
        }
    }

    pub fn set_texture(&mut self, texture: TextureHandle) {
        self.texture = Some(texture);
    }
}

/// Thumbnail catalog component
pub struct ThumbnailCatalog {
    /// Thumbnail size
    pub thumbnail_size: f32,
    /// Currently selected index
    pub selected: Option<usize>,
    /// Number of columns (calculated from width)
    columns: usize,
    /// Number of visible rows
    visible_rows: usize,
}

impl Default for ThumbnailCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailCatalog {
    pub fn new() -> Self {
        Self {
            thumbnail_size: 128.0,
            selected: None,
            columns: 4,
            visible_rows: 4,
        }
    }

    /// Set thumbnail size
    pub fn set_thumbnail_size(&mut self, size: f32) {
        self.thumbnail_size = size.clamp(64.0, 512.0);
    }

    /// Increase thumbnail size
    pub fn zoom_in(&mut self) {
        self.set_thumbnail_size(self.thumbnail_size * 1.2);
    }

    /// Decrease thumbnail size
    pub fn zoom_out(&mut self) {
        self.set_thumbnail_size(self.thumbnail_size / 1.2);
    }

    /// Calculate grid dimensions
    fn calculate_grid(&mut self, available_width: f32, available_height: f32) {
        let item_width = self.thumbnail_size + 16.0; // padding
        let item_height = self.thumbnail_size + 32.0; // padding + label

        self.columns = (available_width / item_width).max(1.0) as usize;
        self.visible_rows = (available_height / item_height).max(1.0) as usize;
    }

    /// Navigate selection
    pub fn navigate(&mut self, direction: NavigateDirection, item_count: usize) -> Option<usize> {
        if item_count == 0 {
            return None;
        }

        let current = self.selected.unwrap_or(0);
        let cols = self.columns.max(1);

        let new_index = match direction {
            NavigateDirection::Up => {
                if current >= cols {
                    current - cols
                } else {
                    current
                }
            }
            NavigateDirection::Down => {
                let next = current + cols;
                if next < item_count {
                    next
                } else {
                    current
                }
            }
            NavigateDirection::Left => {
                if current > 0 {
                    current - 1
                } else {
                    current
                }
            }
            NavigateDirection::Right => {
                if current + 1 < item_count {
                    current + 1
                } else {
                    current
                }
            }
            NavigateDirection::PageUp => {
                let jump = self.visible_rows * cols;
                current.saturating_sub(jump)
            }
            NavigateDirection::PageDown => {
                let jump = self.visible_rows * cols;
                (current + jump).min(item_count - 1)
            }
            NavigateDirection::Home => 0,
            NavigateDirection::End => item_count - 1,
        };

        self.selected = Some(new_index);
        Some(new_index)
    }

    /// Render the thumbnail catalog
    pub fn ui(&mut self, ui: &mut Ui, items: &[ThumbnailItem]) -> Option<CatalogAction> {
        let mut action = None;

        // Calculate grid dimensions
        let available = ui.available_size();
        self.calculate_grid(available.x, available.y);

        // Handle keyboard navigation
        action = self.handle_keyboard(ui, items.len());

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let item_width = self.thumbnail_size + 16.0;
                let item_height = self.thumbnail_size + 32.0;

                egui::Grid::new("thumbnail_grid")
                    .num_columns(self.columns)
                    .spacing(Vec2::new(8.0, 8.0))
                    .show(ui, |ui| {
                        for (idx, item) in items.iter().enumerate() {
                            let is_selected = self.selected == Some(idx);

                            let response = self.render_thumbnail_item(ui, item, is_selected, idx);

                            // Handle clicks
                            if response.clicked() {
                                self.selected = Some(idx);
                                action = Some(CatalogAction::Select(idx));
                            }

                            if response.double_clicked() {
                                action = Some(CatalogAction::Open(idx));
                            }

                            // End row
                            if (idx + 1) % self.columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            });

        action
    }

    /// Handle keyboard input
    fn handle_keyboard(&mut self, ui: &Ui, item_count: usize) -> Option<CatalogAction> {
        if item_count == 0 {
            return None;
        }

        let input = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::PageUp),
                i.key_pressed(egui::Key::PageDown),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Backspace),
            )
        });

        if input.0 {
            self.navigate(NavigateDirection::Up, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::Up));
        }
        if input.1 {
            self.navigate(NavigateDirection::Down, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::Down));
        }
        if input.2 {
            self.navigate(NavigateDirection::Left, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::Left));
        }
        if input.3 {
            self.navigate(NavigateDirection::Right, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::Right));
        }
        if input.4 {
            self.navigate(NavigateDirection::PageUp, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::PageUp));
        }
        if input.5 {
            self.navigate(NavigateDirection::PageDown, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::PageDown));
        }
        if input.6 {
            self.navigate(NavigateDirection::Home, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::Home));
        }
        if input.7 {
            self.navigate(NavigateDirection::End, item_count);
            return Some(CatalogAction::Navigate(NavigateDirection::End));
        }
        if input.8 {
            // Enter - open selected
            if let Some(idx) = self.selected {
                return Some(CatalogAction::Open(idx));
            }
        }
        if input.9 {
            // Backspace - go to parent
            return Some(CatalogAction::GoToParent);
        }

        None
    }

    /// Render a single thumbnail item
    fn render_thumbnail_item(
        &self,
        ui: &mut Ui,
        item: &ThumbnailItem,
        is_selected: bool,
        _idx: usize,
    ) -> Response {
        let item_size = Vec2::new(self.thumbnail_size + 8.0, self.thumbnail_size + 28.0);

        let (rect, response) = ui.allocate_exact_size(item_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // Background
            let bg_color = if is_selected {
                egui::Color32::from_rgba_unmultiplied(100, 150, 255, 80)
            } else if response.hovered() {
                egui::Color32::from_rgba_unmultiplied(100, 100, 100, 40)
            } else {
                egui::Color32::TRANSPARENT
            };

            painter.rect_filled(rect, 4.0, bg_color);

            // Selection border
            if is_selected {
                painter.rect_stroke(
                    rect,
                    4.0,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 150, 255)),
                );
            }

            // Thumbnail area
            let thumb_rect = Rect::from_min_size(
                rect.min + Vec2::new(4.0, 4.0),
                Vec2::splat(self.thumbnail_size),
            );

            // Draw thumbnail or placeholder
            if let Some(texture) = &item.texture {
                // Draw actual thumbnail
                let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                painter.image(texture.id(), thumb_rect, uv, egui::Color32::WHITE);
            } else {
                // Draw placeholder
                painter.rect_filled(thumb_rect, 2.0, egui::Color32::from_gray(40));

                // Icon based on type
                let icon = if item.is_folder {
                    "📁"
                } else if item.is_image {
                    "🖼"
                } else {
                    "📄"
                };

                painter.text(
                    thumb_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    icon,
                    egui::FontId::proportional(32.0),
                    egui::Color32::GRAY,
                );
            }

            // File name label
            let label_rect = Rect::from_min_size(
                egui::pos2(rect.min.x, thumb_rect.max.y + 2.0),
                Vec2::new(item_size.x, 20.0),
            );

            // Truncate name if too long
            let max_chars = (self.thumbnail_size / 8.0) as usize;
            let display_name = if item.name.len() > max_chars {
                format!("{}...", &item.name[..max_chars.saturating_sub(3)])
            } else {
                item.name.clone()
            };

            painter.text(
                label_rect.center(),
                egui::Align2::CENTER_CENTER,
                &display_name,
                egui::FontId::proportional(11.0),
                if is_selected {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::LIGHT_GRAY
                },
            );
        }

        response
    }

    /// Get current column count
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// Get selected index
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }
}
