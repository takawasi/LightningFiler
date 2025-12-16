//! Slideshow functionality for automatic image browsing

use std::time::{Duration, Instant};

/// Slideshow state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SlideshowState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Slideshow configuration
#[derive(Clone, Debug)]
pub struct SlideshowConfig {
    pub interval: Duration,
    pub loop_mode: bool,
    pub shuffle: bool,
    pub reverse: bool,
}

impl Default for SlideshowConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            loop_mode: true,
            shuffle: false,
            reverse: false,
        }
    }
}

/// Slideshow controller
pub struct Slideshow {
    pub config: SlideshowConfig,
    pub state: SlideshowState,
    last_advance: Option<Instant>,
    shuffle_order: Vec<usize>,
    shuffle_index: usize,
}

impl Default for Slideshow {
    fn default() -> Self {
        Self::new()
    }
}

impl Slideshow {
    pub fn new() -> Self {
        Self {
            config: SlideshowConfig::default(),
            state: SlideshowState::Stopped,
            last_advance: None,
            shuffle_order: Vec::new(),
            shuffle_index: 0,
        }
    }

    /// Start slideshow
    pub fn start(&mut self, total_items: usize, current_index: usize) {
        self.state = SlideshowState::Playing;
        self.last_advance = Some(Instant::now());

        if self.config.shuffle {
            self.generate_shuffle_order(total_items, current_index);
        }
    }

    /// Stop slideshow
    pub fn stop(&mut self) {
        self.state = SlideshowState::Stopped;
        self.last_advance = None;
        self.shuffle_order.clear();
        self.shuffle_index = 0;
    }

    /// Toggle slideshow state
    pub fn toggle(&mut self, total_items: usize, current_index: usize) {
        match self.state {
            SlideshowState::Stopped => self.start(total_items, current_index),
            SlideshowState::Playing => {
                self.state = SlideshowState::Paused;
            }
            SlideshowState::Paused => {
                self.state = SlideshowState::Playing;
                self.last_advance = Some(Instant::now());
            }
        }
    }

    /// Check if slideshow is playing
    pub fn is_playing(&self) -> bool {
        self.state == SlideshowState::Playing
    }

    /// Check if slideshow is active (playing or paused)
    pub fn is_active(&self) -> bool {
        self.state != SlideshowState::Stopped
    }

    /// Generate shuffle order (simple implementation without rand)
    fn generate_shuffle_order(&mut self, total: usize, current: usize) {
        // Simple pseudo-random shuffle using current time
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as usize)
            .unwrap_or(0);

        let mut order: Vec<usize> = (0..total).collect();

        // Fisher-Yates shuffle with simple PRNG
        let mut state = seed;
        for i in (1..total).rev() {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            let j = state % (i + 1);
            order.swap(i, j);
        }

        // Move current index to front
        if let Some(pos) = order.iter().position(|&x| x == current) {
            order.swap(0, pos);
        }

        self.shuffle_order = order;
        self.shuffle_index = 0;
    }

    /// Check if it's time to advance
    pub fn should_advance(&mut self) -> bool {
        if self.state != SlideshowState::Playing {
            return false;
        }

        if let Some(last) = self.last_advance {
            if last.elapsed() >= self.config.interval {
                self.last_advance = Some(Instant::now());
                return true;
            }
        }

        false
    }

    /// Calculate next index
    pub fn next_index(&mut self, current: usize, total: usize) -> Option<usize> {
        if total == 0 {
            return None;
        }

        if self.config.shuffle && !self.shuffle_order.is_empty() {
            self.shuffle_index += 1;
            if self.shuffle_index >= self.shuffle_order.len() {
                if self.config.loop_mode {
                    self.shuffle_index = 0;
                } else {
                    self.stop();
                    return None;
                }
            }
            return Some(self.shuffle_order[self.shuffle_index]);
        }

        let next = if self.config.reverse {
            if current == 0 {
                if self.config.loop_mode {
                    total - 1
                } else {
                    self.stop();
                    return None;
                }
            } else {
                current - 1
            }
        } else {
            if current >= total - 1 {
                if self.config.loop_mode {
                    0
                } else {
                    self.stop();
                    return None;
                }
            } else {
                current + 1
            }
        };

        Some(next)
    }

    /// Get progress (0.0 - 1.0) for progress bar
    pub fn progress(&self) -> f32 {
        if let Some(last) = self.last_advance {
            let elapsed = last.elapsed().as_secs_f32();
            let total = self.config.interval.as_secs_f32();
            (elapsed / total).min(1.0)
        } else {
            0.0
        }
    }

    /// Set interval in seconds
    pub fn set_interval_secs(&mut self, secs: f32) {
        self.config.interval = Duration::from_secs_f32(secs.clamp(0.5, 60.0));
    }

    /// Increase interval
    pub fn increase_interval(&mut self) {
        let current = self.config.interval.as_secs_f32();
        self.set_interval_secs(current + 1.0);
    }

    /// Decrease interval
    pub fn decrease_interval(&mut self) {
        let current = self.config.interval.as_secs_f32();
        self.set_interval_secs(current - 1.0);
    }

    /// Render progress bar
    pub fn render_progress(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        if self.state == SlideshowState::Stopped {
            return;
        }

        let progress = self.progress();
        let bar_height = 3.0;
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.max.y - bar_height),
            egui::vec2(rect.width(), bar_height),
        );

        // Background
        ui.painter().rect_filled(bar_rect, 0.0, egui::Color32::from_gray(50));

        // Progress
        let progress_rect = egui::Rect::from_min_size(
            bar_rect.min,
            egui::vec2(bar_rect.width() * progress, bar_height),
        );
        let color = if self.state == SlideshowState::Paused {
            egui::Color32::YELLOW
        } else {
            egui::Color32::from_rgb(100, 200, 100)
        };
        ui.painter().rect_filled(progress_rect, 0.0, color);
    }

    /// Get status text
    pub fn status_text(&self) -> String {
        match self.state {
            SlideshowState::Stopped => String::new(),
            SlideshowState::Playing => {
                let interval = self.config.interval.as_secs_f32();
                let mut opts = Vec::new();
                if self.config.loop_mode { opts.push("Loop"); }
                if self.config.shuffle { opts.push("Shuffle"); }
                if self.config.reverse { opts.push("Rev"); }
                let opts_str = if opts.is_empty() { String::new() } else { format!(" [{}]", opts.join(",")) };
                format!("Slideshow {:.1}s{}", interval, opts_str)
            }
            SlideshowState::Paused => "Slideshow (Paused)".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slideshow_toggle() {
        let mut ss = Slideshow::new();
        assert_eq!(ss.state, SlideshowState::Stopped);

        ss.toggle(10, 0);
        assert_eq!(ss.state, SlideshowState::Playing);

        ss.toggle(10, 0);
        assert_eq!(ss.state, SlideshowState::Paused);

        ss.toggle(10, 0);
        assert_eq!(ss.state, SlideshowState::Playing);

        ss.stop();
        assert_eq!(ss.state, SlideshowState::Stopped);
    }

    #[test]
    fn test_next_index() {
        let mut ss = Slideshow::new();
        ss.config.loop_mode = true;

        assert_eq!(ss.next_index(0, 5), Some(1));
        assert_eq!(ss.next_index(4, 5), Some(0)); // Loop

        ss.config.reverse = true;
        assert_eq!(ss.next_index(0, 5), Some(4)); // Loop reverse
        assert_eq!(ss.next_index(3, 5), Some(2));
    }

    #[test]
    fn test_interval() {
        let mut ss = Slideshow::new();
        ss.set_interval_secs(3.0);
        assert_eq!(ss.config.interval, Duration::from_secs(3));

        ss.increase_interval();
        assert_eq!(ss.config.interval, Duration::from_secs(4));

        ss.decrease_interval();
        assert_eq!(ss.config.interval, Duration::from_secs(3));
    }
}
