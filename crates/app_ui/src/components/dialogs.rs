//! Dialog components for file operations

use egui::{Context, Window, Align2};

/// Result of dialog interaction
pub enum DialogResult<T> {
    None,           // 表示中/未決定
    Ok(T),          // 確定
    Cancel,         // キャンセル
}

/// Common dialog trait
pub trait Dialog {
    type Output;
    fn ui(&mut self, ctx: &Context) -> DialogResult<Self::Output>;
    fn is_open(&self) -> bool;
    fn close(&mut self);
}

/// Confirmation dialog
pub struct ConfirmDialog {
    pub open: bool,
    pub title: String,
    pub message: String,
    pub confirm_text: String,
    pub cancel_text: String,
    pub dangerous: bool,  // trueなら確認ボタンを赤く
}

impl ConfirmDialog {
    pub fn new_delete(file_name: &str, use_trash: bool) -> Self {
        Self {
            open: true,
            title: if use_trash { "ゴミ箱へ移動" } else { "完全に削除" }.to_string(),
            message: format!("「{}」を削除しますか？", file_name),
            confirm_text: "削除".to_string(),
            cancel_text: "キャンセル".to_string(),
            dangerous: !use_trash,
        }
    }
}

impl Dialog for ConfirmDialog {
    type Output = bool;

    fn ui(&mut self, ctx: &Context) -> DialogResult<bool> {
        if !self.open {
            return DialogResult::None;
        }

        let mut result = DialogResult::None;

        Window::new(&self.title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(&self.message);
                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    let confirm_btn = if self.dangerous {
                        ui.button(egui::RichText::new(&self.confirm_text).color(egui::Color32::RED))
                    } else {
                        ui.button(&self.confirm_text)
                    };

                    if confirm_btn.clicked() {
                        result = DialogResult::Ok(true);
                        self.open = false;
                    }

                    if ui.button(&self.cancel_text).clicked() {
                        result = DialogResult::Cancel;
                        self.open = false;
                    }
                });
            });

        result
    }

    fn is_open(&self) -> bool { self.open }
    fn close(&mut self) { self.open = false; }
}

/// Rename dialog
pub struct RenameDialog {
    pub open: bool,
    pub original_name: String,
    pub new_name: String,
    pub extension: String,
    pub select_stem_only: bool,  // 拡張子を除いて選択
}

impl RenameDialog {
    pub fn new(file_name: &str) -> Self {
        let path = std::path::Path::new(file_name);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name);
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        Self {
            open: true,
            original_name: file_name.to_string(),
            new_name: stem.to_string(),
            extension: ext.to_string(),
            select_stem_only: true,
        }
    }

    pub fn full_name(&self) -> String {
        if self.extension.is_empty() {
            self.new_name.clone()
        } else {
            format!("{}.{}", self.new_name, self.extension)
        }
    }
}

impl Dialog for RenameDialog {
    type Output = String;

    fn ui(&mut self, ctx: &Context) -> DialogResult<String> {
        if !self.open {
            return DialogResult::None;
        }

        let mut result = DialogResult::None;

        Window::new("リネーム")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!("元のファイル名: {}", self.original_name));
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("新しい名前:");
                    let response = ui.text_edit_singleline(&mut self.new_name);
                    if !self.extension.is_empty() {
                        ui.label(format!(".{}", self.extension));
                    }

                    // Enter で確定
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !self.new_name.is_empty() && self.new_name != self.original_name {
                            result = DialogResult::Ok(self.full_name());
                            self.open = false;
                        }
                    }
                });

                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        if !self.new_name.is_empty() {
                            result = DialogResult::Ok(self.full_name());
                            self.open = false;
                        }
                    }
                    if ui.button("キャンセル").clicked() {
                        result = DialogResult::Cancel;
                        self.open = false;
                    }
                });
            });

        result
    }

    fn is_open(&self) -> bool { self.open }
    fn close(&mut self) { self.open = false; }
}

/// Tag edit dialog
pub struct TagEditDialog {
    pub open: bool,
    pub current_tags: Vec<String>,
    pub all_tags: Vec<String>,      // 候補（DBから取得）
    pub input: String,
    pub filtered_suggestions: Vec<String>,
}

impl TagEditDialog {
    pub fn new(current_tags: Vec<String>, all_tags: Vec<String>) -> Self {
        Self {
            open: true,
            current_tags,
            all_tags,
            input: String::new(),
            filtered_suggestions: Vec::new(),
        }
    }

    fn update_suggestions(&mut self) {
        if self.input.is_empty() {
            self.filtered_suggestions.clear();
            return;
        }

        let input_lower = self.input.to_lowercase();
        self.filtered_suggestions = self.all_tags.iter()
            .filter(|t| t.to_lowercase().contains(&input_lower))
            .filter(|t| !self.current_tags.contains(t))
            .take(10)
            .cloned()
            .collect();
    }
}

impl Dialog for TagEditDialog {
    type Output = Vec<String>;

    fn ui(&mut self, ctx: &Context) -> DialogResult<Vec<String>> {
        if !self.open {
            return DialogResult::None;
        }

        let mut result = DialogResult::None;

        Window::new("タグ編集")
            .collapsible(false)
            .resizable(true)
            .default_size([400.0, 300.0])
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                // 現在のタグ表示
                ui.label("現在のタグ:");
                ui.horizontal_wrapped(|ui| {
                    let mut to_remove = None;
                    for (i, tag) in self.current_tags.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(tag);
                            if ui.small_button("x").clicked() {
                                to_remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = to_remove {
                        self.current_tags.remove(i);
                    }
                });

                ui.separator();

                // タグ入力
                ui.horizontal(|ui| {
                    ui.label("追加:");
                    let response = ui.text_edit_singleline(&mut self.input);
                    if response.changed() {
                        self.update_suggestions();
                    }

                    // Enterで追加
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !self.input.is_empty() && !self.current_tags.contains(&self.input) {
                            self.current_tags.push(self.input.clone());
                            self.input.clear();
                            self.filtered_suggestions.clear();
                        }
                    }
                });

                // サジェスト表示
                if !self.filtered_suggestions.is_empty() {
                    ui.group(|ui| {
                        for suggestion in &self.filtered_suggestions.clone() {
                            if ui.selectable_label(false, suggestion).clicked() {
                                self.current_tags.push(suggestion.clone());
                                self.input.clear();
                                self.filtered_suggestions.clear();
                            }
                        }
                    });
                }

                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        result = DialogResult::Ok(self.current_tags.clone());
                        self.open = false;
                    }
                    if ui.button("キャンセル").clicked() {
                        result = DialogResult::Cancel;
                        self.open = false;
                    }
                });
            });

        result
    }

    fn is_open(&self) -> bool { self.open }
    fn close(&mut self) { self.open = false; }
}
