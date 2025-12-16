//! Spread (two-page) viewing mode for manga/comics

use super::viewer::FitMode;

/// Spread display mode
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SpreadMode {
    #[default]
    Single,       // Single page
    SpreadRTL,    // Spread (right-to-left, manga)
    SpreadLTR,    // Spread (left-to-right, western books)
    Auto,         // Auto-detect from filename/metadata
}

/// Page position in spread
/// Note: Reserved for future use in spread page placement logic
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PagePosition {
    Left,
    Right,
    Center,  // Single page display
}

/// Spread layout calculation result
#[derive(Default, Clone)]
pub struct SpreadLayout {
    pub left: Option<egui::Rect>,
    pub right: Option<egui::Rect>,
    pub scale: f32,
}

/// Spread viewer component for two-page display
pub struct SpreadViewer {
    pub mode: SpreadMode,
    pub current_spread: (Option<usize>, Option<usize>),  // (left_idx, right_idx)
    pub first_page_single: bool,  // Cover page displayed alone
    pub last_page_single: bool,   // Last page displayed alone
    pub fit_mode: FitMode,
}

impl Default for SpreadViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpreadViewer {
    pub fn new() -> Self {
        Self {
            mode: SpreadMode::Single,
            current_spread: (None, None),
            first_page_single: true,
            last_page_single: true,
            fit_mode: FitMode::FitToWindow,
        }
    }

    /// Navigate to specified index and calculate spread pair
    pub fn go_to(&mut self, index: usize, total: usize) -> (Option<usize>, Option<usize>) {
        if total == 0 {
            self.current_spread = (None, None);
            return self.current_spread;
        }

        match self.mode {
            SpreadMode::Single => {
                self.current_spread = (Some(index), None);
            }
            SpreadMode::SpreadRTL | SpreadMode::SpreadLTR | SpreadMode::Auto => {
                // Cover page alone
                if self.first_page_single && index == 0 {
                    self.current_spread = (Some(0), None);
                    return self.current_spread;
                }

                // Last page alone
                if self.last_page_single && index == total - 1 && total > 1 {
                    self.current_spread = (Some(total - 1), None);
                    return self.current_spread;
                }

                // Calculate spread pair
                let adjusted_index = if self.first_page_single {
                    index.saturating_sub(1)
                } else {
                    index
                };
                let pair_start = (adjusted_index / 2) * 2;
                let real_start = if self.first_page_single { pair_start + 1 } else { pair_start };

                let left_idx = real_start;
                let right_idx = real_start + 1;

                // Range check
                let left = if left_idx < total { Some(left_idx) } else { None };
                let right = if right_idx < total {
                    // Check if last page should be single
                    if self.last_page_single && right_idx == total - 1 {
                        None
                    } else {
                        Some(right_idx)
                    }
                } else {
                    None
                };

                // Swap left/right for RTL/LTR
                self.current_spread = match self.mode {
                    SpreadMode::SpreadRTL | SpreadMode::Auto => (right, left),
                    SpreadMode::SpreadLTR => (left, right),
                    _ => (left, right),
                };
            }
        }

        self.current_spread
    }

    /// Move to next page/spread
    pub fn next(&mut self, total: usize) -> (Option<usize>, Option<usize>) {
        let current_max = match (self.current_spread.0, self.current_spread.1) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        if let Some(idx) = current_max {
            let next_idx = (idx + 1).min(total.saturating_sub(1));
            self.go_to(next_idx, total)
        } else {
            self.go_to(0, total)
        }
    }

    /// Move to previous page/spread
    pub fn prev(&mut self, total: usize) -> (Option<usize>, Option<usize>) {
        let current_min = self.current_spread.0.or(self.current_spread.1);

        if let Some(idx) = current_min {
            let step = match self.mode {
                SpreadMode::Single => 1,
                _ => 2,
            };
            let prev_idx = idx.saturating_sub(step);
            self.go_to(prev_idx, total)
        } else {
            self.go_to(0, total)
        }
    }

    /// Check if currently in spread mode
    pub fn is_spread_mode(&self) -> bool {
        !matches!(self.mode, SpreadMode::Single)
    }

    /// Get display name for current mode
    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            SpreadMode::Single => "Single",
            SpreadMode::SpreadRTL => "Spread (RTL)",
            SpreadMode::SpreadLTR => "Spread (LTR)",
            SpreadMode::Auto => "Auto",
        }
    }

    /// Calculate layout for spread display
    pub fn calculate_layout(
        &self,
        left_size: Option<(u32, u32)>,
        right_size: Option<(u32, u32)>,
        viewport: (f32, f32),
    ) -> SpreadLayout {
        let (vw, vh) = viewport;
        let gap = 4.0; // Gap between pages

        match (left_size, right_size) {
            (Some((lw, lh)), Some((rw, rh))) => {
                // Spread display
                let total_width = (lw + rw) as f32;
                let max_height = lh.max(rh) as f32;
                let total_aspect = total_width / max_height;
                let viewport_aspect = vw / vh;

                let (scale, offset_y) = if total_aspect > viewport_aspect {
                    // Width-based scaling
                    let scale = (vw - gap) / total_width;
                    let offset_y = (vh - max_height * scale) / 2.0;
                    (scale, offset_y)
                } else {
                    // Height-based scaling
                    let scale = vh / max_height;
                    let offset_y = 0.0;
                    (scale, offset_y)
                };

                let left_width = lw as f32 * scale;
                let right_width = rw as f32 * scale;
                let total_scaled = left_width + right_width + gap;
                let start_x = (vw - total_scaled) / 2.0;

                let left_rect = egui::Rect::from_min_size(
                    egui::pos2(start_x, offset_y),
                    egui::vec2(left_width, lh as f32 * scale),
                );

                let right_rect = egui::Rect::from_min_size(
                    egui::pos2(start_x + left_width + gap, offset_y),
                    egui::vec2(right_width, rh as f32 * scale),
                );

                SpreadLayout {
                    left: Some(left_rect),
                    right: Some(right_rect),
                    scale,
                }
            }
            (Some((w, h)), None) | (None, Some((w, h))) => {
                // Single page (centered)
                let aspect = w as f32 / h as f32;
                let viewport_aspect = vw / vh;

                let (scale, rect) = if aspect > viewport_aspect {
                    let scale = vw / w as f32;
                    let height = h as f32 * scale;
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(0.0, (vh - height) / 2.0),
                        egui::vec2(vw, height),
                    );
                    (scale, rect)
                } else {
                    let scale = vh / h as f32;
                    let width = w as f32 * scale;
                    let rect = egui::Rect::from_min_size(
                        egui::pos2((vw - width) / 2.0, 0.0),
                        egui::vec2(width, vh),
                    );
                    (scale, rect)
                };

                SpreadLayout {
                    left: Some(rect),
                    right: None,
                    scale,
                }
            }
            (None, None) => SpreadLayout::default(),
        }
    }

    /// Cycle through spread modes
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            SpreadMode::Single => SpreadMode::SpreadRTL,
            SpreadMode::SpreadRTL => SpreadMode::SpreadLTR,
            SpreadMode::SpreadLTR => SpreadMode::Auto,
            SpreadMode::Auto => SpreadMode::Single,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_mode() {
        let mut viewer = SpreadViewer::new();
        viewer.mode = SpreadMode::Single;

        let spread = viewer.go_to(3, 10);
        assert_eq!(spread, (Some(3), None));
    }

    #[test]
    fn test_spread_rtl_first_page() {
        let mut viewer = SpreadViewer::new();
        viewer.mode = SpreadMode::SpreadRTL;
        viewer.first_page_single = true;

        let spread = viewer.go_to(0, 10);
        assert_eq!(spread, (Some(0), None)); // Cover alone
    }

    #[test]
    fn test_spread_rtl_pair() {
        let mut viewer = SpreadViewer::new();
        viewer.mode = SpreadMode::SpreadRTL;
        viewer.first_page_single = true;
        viewer.last_page_single = false;

        let spread = viewer.go_to(1, 10);
        // Pages 1 and 2 should be paired (RTL: right=1, left=2)
        assert_eq!(spread, (Some(2), Some(1)));
    }

    #[test]
    fn test_next_prev() {
        let mut viewer = SpreadViewer::new();
        viewer.mode = SpreadMode::Single;

        viewer.go_to(5, 10);
        viewer.next(10);
        assert_eq!(viewer.current_spread.0, Some(6));

        viewer.prev(10);
        assert_eq!(viewer.current_spread.0, Some(5));
    }
}
