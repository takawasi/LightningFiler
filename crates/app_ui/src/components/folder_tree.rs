//! Folder tree component for left panel
//! Displays only folders in a hierarchical tree structure

use egui::{Ui, Response, Vec2};
use std::path::{Path, PathBuf};
use std::collections::HashSet;

/// Action returned from folder tree interaction
#[derive(Debug, Clone)]
pub enum FolderTreeAction {
    /// User selected a folder
    SelectFolder(PathBuf),
    /// User expanded/collapsed a folder
    ToggleExpand(PathBuf),
    /// User wants to go to parent
    GoToParent,
}

/// A node in the folder tree
#[derive(Debug, Clone)]
pub struct FolderNode {
    pub path: PathBuf,
    pub name: String,
    pub has_children: bool,
    pub depth: usize,
}

impl FolderNode {
    pub fn new(path: PathBuf, depth: usize) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        // Check if has subdirectories
        let has_children = std::fs::read_dir(&path)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            })
            .unwrap_or(false);

        Self {
            path,
            name,
            has_children,
            depth,
        }
    }
}

/// Folder tree component
pub struct FolderTree {
    /// Currently selected folder
    pub selected: Option<PathBuf>,
    /// Expanded folders
    pub expanded: HashSet<PathBuf>,
    /// Root paths (drives on Windows, / on Unix)
    pub roots: Vec<PathBuf>,
    /// Cached folder nodes
    nodes: Vec<FolderNode>,
    /// Last refreshed path
    last_root: Option<PathBuf>,
}

impl Default for FolderTree {
    fn default() -> Self {
        Self::new()
    }
}

impl FolderTree {
    pub fn new() -> Self {
        let roots = Self::get_root_paths();

        Self {
            selected: None,
            expanded: HashSet::new(),
            roots,
            nodes: Vec::new(),
            last_root: None,
        }
    }

    /// Get root paths (drives on Windows, / on Unix)
    #[cfg(windows)]
    fn get_root_paths() -> Vec<PathBuf> {
        // Get available drives
        let mut drives = Vec::new();
        for letter in b'A'..=b'Z' {
            let drive = format!("{}:\\", letter as char);
            let path = PathBuf::from(&drive);
            if path.exists() {
                drives.push(path);
            }
        }
        if drives.is_empty() {
            drives.push(PathBuf::from("C:\\"));
        }
        drives
    }

    #[cfg(not(windows))]
    fn get_root_paths() -> Vec<PathBuf> {
        vec![PathBuf::from("/")]
    }

    /// Set the current root folder to display
    pub fn set_root(&mut self, path: &Path) {
        if self.last_root.as_deref() != Some(path) {
            self.last_root = Some(path.to_path_buf());
            self.refresh_nodes(path);
        }
    }

    /// Refresh the folder tree from a root path
    fn refresh_nodes(&mut self, root: &Path) {
        self.nodes.clear();

        // Add the root itself
        self.nodes.push(FolderNode::new(root.to_path_buf(), 0));

        // Recursively add expanded folders
        self.add_children(root, 1);
    }

    /// Add children of a folder if it's expanded
    fn add_children(&mut self, parent: &Path, depth: usize) {
        if !self.expanded.contains(parent) {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(parent) {
            let mut folders: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .collect();

            // Sort alphabetically
            folders.sort_by(|a, b| {
                a.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .cmp(&b.file_name().map(|n| n.to_string_lossy().to_lowercase()))
            });

            for folder in folders {
                self.nodes.push(FolderNode::new(folder.clone(), depth));
                // Recursively add if expanded
                self.add_children(&folder, depth + 1);
            }
        }
    }

    /// Toggle expansion of a folder
    pub fn toggle_expand(&mut self, path: &Path) {
        if self.expanded.contains(path) {
            self.expanded.remove(path);
        } else {
            self.expanded.insert(path.to_path_buf());
        }

        // Refresh if we have a root
        if let Some(root) = self.last_root.clone() {
            self.refresh_nodes(&root);
        }
    }

    /// Expand to show a specific path
    pub fn expand_to(&mut self, path: &Path) {
        // Expand all ancestors
        let mut current = path.to_path_buf();
        while let Some(parent) = current.parent() {
            self.expanded.insert(parent.to_path_buf());
            current = parent.to_path_buf();
        }

        // Refresh
        if let Some(root) = self.last_root.clone() {
            self.refresh_nodes(&root);
        }
    }

    /// Render the folder tree
    pub fn ui(&mut self, ui: &mut Ui, current_path: &Path) -> Option<FolderTreeAction> {
        let mut action = None;

        // Make sure tree is updated for current path
        if let Some(root) = current_path.ancestors().last() {
            self.set_root(root);
        } else {
            self.set_root(current_path);
        }

        // Ensure current path is visible
        self.expand_to(current_path);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Parent folder button
                if current_path.parent().is_some() {
                    let parent_response = ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.label("📁");
                        ui.label("..");
                    }).response;

                    if parent_response.clicked() {
                        action = Some(FolderTreeAction::GoToParent);
                    }
                    if parent_response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }

                ui.separator();

                // Render folder nodes
                for node in &self.nodes.clone() {
                    let is_selected = self.selected.as_ref() == Some(&node.path)
                        || current_path == node.path;
                    let is_expanded = self.expanded.contains(&node.path);

                    let indent = node.depth as f32 * 16.0;

                    ui.horizontal(|ui| {
                        ui.add_space(indent + 4.0);

                        // Expand/collapse button
                        if node.has_children {
                            let arrow = if is_expanded { "▼" } else { "▶" };
                            if ui.small_button(arrow).clicked() {
                                self.toggle_expand(&node.path);
                                action = Some(FolderTreeAction::ToggleExpand(node.path.clone()));
                            }
                        } else {
                            ui.add_space(18.0); // Space for alignment
                        }

                        // Folder icon and name (truncate long names to prevent panel width changes)
                        let icon = if is_expanded { "📂" } else { "📁" };
                        let max_name_chars = 20;
                        let display_name = if node.name.chars().count() > max_name_chars {
                            let truncated: String = node.name.chars().take(max_name_chars - 2).collect();
                            format!("{}…", truncated)
                        } else {
                            node.name.clone()
                        };

                        let text = egui::RichText::new(format!("{} {}", icon, display_name));
                        let text = if is_selected {
                            text.strong().color(egui::Color32::LIGHT_BLUE)
                        } else {
                            text
                        };

                        let label_response = ui.selectable_label(is_selected, text)
                            .on_hover_text(&node.name); // Show full name on hover

                        if label_response.clicked() {
                            self.selected = Some(node.path.clone());
                            action = Some(FolderTreeAction::SelectFolder(node.path.clone()));
                        }

                        if label_response.double_clicked() {
                            self.toggle_expand(&node.path);
                        }
                    });
                }
            });

        action
    }
}
