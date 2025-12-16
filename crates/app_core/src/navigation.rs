//! Navigation state and context management
//! Based on Doc 3: Input/UX Specification - Navigation Commands (nav.*)

use app_fs::UniversalPath;
use serde::{Deserialize, Serialize};

/// Navigation context determines how navigation commands behave
#[derive(Debug, Clone)]
pub enum NavigationContext {
    /// Browsing a physical folder
    PhysicalFolder {
        path: UniversalPath,
        files: Vec<FileEntry>,
        current_index: usize,
    },

    /// Viewing tag search results
    TagSearch {
        tag_ids: Vec<i64>,
        query: String,
        results: Vec<FileEntry>,
        current_index: usize,
    },

    /// Viewing timeline (date-based)
    Timeline {
        start_date: i64,
        end_date: i64,
        results: Vec<FileEntry>,
        current_index: usize,
    },

    /// Inside an archive
    Archive {
        archive_path: UniversalPath,
        inner_path: Option<String>,
        entries: Vec<FileEntry>,
        current_index: usize,
    },

    /// Search results
    Search {
        query: String,
        results: Vec<FileEntry>,
        current_index: usize,
    },
}

/// File entry in navigation list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified: Option<i64>,
    pub thumbnail_hash: Option<u64>,
}

/// Grid layout information for navigation
#[derive(Debug, Clone, Copy)]
pub struct GridLayout {
    /// Number of columns in the grid
    pub columns: usize,
    /// Number of visible rows (for page calculation)
    pub visible_rows: usize,
}

impl Default for GridLayout {
    fn default() -> Self {
        Self {
            columns: 1,
            visible_rows: 10,
        }
    }
}

/// Selection state for multi-select support
#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    /// Selected item indices
    pub selected: Vec<usize>,
    /// Anchor index for shift-select
    pub anchor: Option<usize>,
}

impl SelectionState {
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    pub fn select_single(&mut self, index: usize) {
        self.selected.clear();
        self.selected.push(index);
        self.anchor = Some(index);
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(pos) = self.selected.iter().position(|&i| i == index) {
            self.selected.remove(pos);
        } else {
            self.selected.push(index);
        }
    }

    pub fn select_range(&mut self, from: usize, to: usize) {
        let (start, end) = if from <= to { (from, to) } else { (to, from) };
        self.selected.clear();
        for i in start..=end {
            self.selected.push(i);
        }
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }
}

/// Navigation state for the application
pub struct NavigationState {
    /// Current context
    pub context: NavigationContext,

    /// Navigation history stack
    history: Vec<NavigationContext>,

    /// Forward stack (for redo)
    forward: Vec<NavigationContext>,

    /// Grid layout for browser view
    pub grid_layout: GridLayout,

    /// Selection state
    pub selection: SelectionState,

    /// Default threshold for nav.enter (files <= threshold -> Viewer mode)
    pub enter_threshold: i32,
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            context: NavigationContext::PhysicalFolder {
                path: UniversalPath::new("."),
                files: Vec::new(),
                current_index: 0,
            },
            history: Vec::new(),
            forward: Vec::new(),
            grid_layout: GridLayout::default(),
            selection: SelectionState::default(),
            enter_threshold: 5, // Default: <=5 files -> Viewer mode
        }
    }

    /// Update grid layout based on view dimensions
    pub fn update_grid_layout(&mut self, columns: usize, visible_rows: usize) {
        self.grid_layout.columns = columns.max(1);
        self.grid_layout.visible_rows = visible_rows.max(1);
    }

    /// Navigate to a new context
    pub fn navigate_to(&mut self, context: NavigationContext) {
        // Save current to history
        let old = std::mem::replace(&mut self.context, context);
        self.history.push(old);

        // Clear forward stack and selection
        self.forward.clear();
        self.selection.clear();
    }

    /// Go back in history
    pub fn go_back(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            let current = std::mem::replace(&mut self.context, prev);
            self.forward.push(current);
            self.selection.clear();
            true
        } else {
            false
        }
    }

    /// Go forward in history
    pub fn go_forward(&mut self) -> bool {
        if let Some(next) = self.forward.pop() {
            let current = std::mem::replace(&mut self.context, next);
            self.history.push(current);
            self.selection.clear();
            true
        } else {
            false
        }
    }

    /// Get current file entries
    pub fn current_files(&self) -> &[FileEntry] {
        match &self.context {
            NavigationContext::PhysicalFolder { files, .. } => files,
            NavigationContext::TagSearch { results, .. } => results,
            NavigationContext::Timeline { results, .. } => results,
            NavigationContext::Archive { entries, .. } => entries,
            NavigationContext::Search { results, .. } => results,
        }
    }

    /// Get file count
    pub fn file_count(&self) -> usize {
        self.current_files().len()
    }

    /// Get current index
    pub fn current_index(&self) -> usize {
        match &self.context {
            NavigationContext::PhysicalFolder { current_index, .. } => *current_index,
            NavigationContext::TagSearch { current_index, .. } => *current_index,
            NavigationContext::Timeline { current_index, .. } => *current_index,
            NavigationContext::Archive { current_index, .. } => *current_index,
            NavigationContext::Search { current_index, .. } => *current_index,
        }
    }

    /// Set current index
    pub fn set_index(&mut self, index: usize) {
        let count = self.current_files().len();
        if count == 0 {
            return;
        }
        let index = index.min(count - 1);

        match &mut self.context {
            NavigationContext::PhysicalFolder { current_index, .. } => *current_index = index,
            NavigationContext::TagSearch { current_index, .. } => *current_index = index,
            NavigationContext::Timeline { current_index, .. } => *current_index = index,
            NavigationContext::Archive { current_index, .. } => *current_index = index,
            NavigationContext::Search { current_index, .. } => *current_index = index,
        }
    }

    // ========================================
    // Grid Navigation (nav.move_*)
    // ========================================

    /// Move up in grid (nav.move_up)
    /// Returns true if position changed
    pub fn move_up(&mut self, amount: usize, select: bool) -> bool {
        let current = self.current_index();
        let columns = self.grid_layout.columns;
        let move_by = columns * amount;

        if current >= move_by {
            let new_index = current - move_by;
            self.set_index(new_index);
            if select {
                self.selection.select_range(self.selection.anchor.unwrap_or(current), new_index);
            } else {
                self.selection.select_single(new_index);
            }
            true
        } else if current > 0 {
            // Move to first item in column or first item
            let new_index = current % columns;
            if new_index != current {
                self.set_index(new_index);
                if select {
                    self.selection.select_range(self.selection.anchor.unwrap_or(current), new_index);
                } else {
                    self.selection.select_single(new_index);
                }
                true
            } else {
                self.set_index(0);
                if select {
                    self.selection.select_range(self.selection.anchor.unwrap_or(current), 0);
                } else {
                    self.selection.select_single(0);
                }
                true
            }
        } else {
            false
        }
    }

    /// Move down in grid (nav.move_down)
    pub fn move_down(&mut self, amount: usize, select: bool) -> bool {
        let current = self.current_index();
        let count = self.file_count();
        if count == 0 {
            return false;
        }
        let max = count - 1;
        let columns = self.grid_layout.columns;
        let move_by = columns * amount;

        let new_index = (current + move_by).min(max);
        if new_index != current {
            self.set_index(new_index);
            if select {
                self.selection.select_range(self.selection.anchor.unwrap_or(current), new_index);
            } else {
                self.selection.select_single(new_index);
            }
            true
        } else {
            false
        }
    }

    /// Move left in grid (nav.move_left)
    pub fn move_left(&mut self, amount: usize, select: bool, wrap: bool) -> bool {
        let current = self.current_index();
        let columns = self.grid_layout.columns;
        let current_col = current % columns;

        let new_index = if current_col >= amount {
            current - amount
        } else if wrap && current >= columns {
            // Wrap to end of previous row
            current - current_col + columns - 1 - columns
        } else if current > 0 {
            // Move to beginning of row or 0
            current - current_col
        } else {
            return false;
        };

        if new_index != current {
            self.set_index(new_index);
            if select {
                self.selection.select_range(self.selection.anchor.unwrap_or(current), new_index);
            } else {
                self.selection.select_single(new_index);
            }
            true
        } else {
            false
        }
    }

    /// Move right in grid (nav.move_right)
    pub fn move_right(&mut self, amount: usize, select: bool, wrap: bool) -> bool {
        let current = self.current_index();
        let count = self.file_count();
        if count == 0 {
            return false;
        }
        let max = count - 1;
        let columns = self.grid_layout.columns;
        let current_col = current % columns;
        let row_end = current - current_col + columns - 1;

        let new_index = if current_col + amount < columns && current + amount <= max {
            current + amount
        } else if wrap && current + columns <= max {
            // Wrap to start of next row
            current - current_col + columns
        } else {
            // Move to end of row or max
            row_end.min(max)
        };

        if new_index != current && new_index <= max {
            self.set_index(new_index);
            if select {
                self.selection.select_range(self.selection.anchor.unwrap_or(current), new_index);
            } else {
                self.selection.select_single(new_index);
            }
            true
        } else {
            false
        }
    }

    // ========================================
    // Page Navigation (nav.page_*)
    // ========================================

    /// Page up (nav.page_up)
    pub fn page_up(&mut self, amount: usize, select: bool) -> bool {
        let items_per_page = self.grid_layout.columns * self.grid_layout.visible_rows;
        let move_by = items_per_page * amount;
        let current = self.current_index();

        let new_index = current.saturating_sub(move_by);
        if new_index != current {
            self.set_index(new_index);
            if select {
                self.selection.select_range(self.selection.anchor.unwrap_or(current), new_index);
            } else {
                self.selection.select_single(new_index);
            }
            true
        } else {
            false
        }
    }

    /// Page down (nav.page_down)
    pub fn page_down(&mut self, amount: usize, select: bool) -> bool {
        let count = self.file_count();
        if count == 0 {
            return false;
        }
        let max = count - 1;
        let items_per_page = self.grid_layout.columns * self.grid_layout.visible_rows;
        let move_by = items_per_page * amount;
        let current = self.current_index();

        let new_index = (current + move_by).min(max);
        if new_index != current {
            self.set_index(new_index);
            if select {
                self.selection.select_range(self.selection.anchor.unwrap_or(current), new_index);
            } else {
                self.selection.select_single(new_index);
            }
            true
        } else {
            false
        }
    }

    // ========================================
    // Home/End (nav.home, nav.end)
    // ========================================

    /// Go to first item (nav.home)
    pub fn home(&mut self, select: bool) {
        let current = self.current_index();
        self.set_index(0);
        if select {
            self.selection.select_range(self.selection.anchor.unwrap_or(current), 0);
        } else {
            self.selection.select_single(0);
        }
    }

    /// Go to last item (nav.end)
    pub fn end(&mut self, select: bool) {
        let count = self.file_count();
        if count == 0 {
            return;
        }
        let current = self.current_index();
        let max = count - 1;
        self.set_index(max);
        if select {
            self.selection.select_range(self.selection.anchor.unwrap_or(current), max);
        } else {
            self.selection.select_single(max);
        }
    }

    // ========================================
    // Item Navigation (nav.next_item, nav.prev_item)
    // ========================================

    /// Move to next item (nav.next_item)
    pub fn next_item(&mut self, amount: usize, wrap: bool) -> bool {
        let current = self.current_index();
        let count = self.file_count();
        if count == 0 {
            return false;
        }
        let max = count - 1;

        let new_index = if current + amount <= max {
            current + amount
        } else if wrap {
            (current + amount) % count
        } else {
            max
        };

        if new_index != current {
            self.set_index(new_index);
            self.selection.select_single(new_index);
            true
        } else {
            false
        }
    }

    /// Move to previous item (nav.prev_item)
    pub fn prev_item(&mut self, amount: usize, wrap: bool) -> bool {
        let current = self.current_index();
        let count = self.file_count();
        if count == 0 {
            return false;
        }

        let new_index = if current >= amount {
            current - amount
        } else if wrap {
            // Fix: when (amount - current) % count == 0, result should be 0, not count
            let diff = (amount - current) % count;
            if diff == 0 { 0 } else { count - diff }
        } else {
            0
        };

        if new_index != current {
            self.set_index(new_index);
            self.selection.select_single(new_index);
            true
        } else {
            false
        }
    }

    // ========================================
    // Legacy methods (compatibility)
    // ========================================

    /// Move to next item (legacy)
    pub fn next(&mut self) -> bool {
        self.next_item(1, false)
    }

    /// Move to previous item (legacy)
    pub fn prev(&mut self) -> bool {
        self.prev_item(1, false)
    }

    /// Skip forward by count (legacy)
    pub fn skip_forward(&mut self, count: usize) {
        self.next_item(count, false);
    }

    /// Skip backward by count (legacy)
    pub fn skip_backward(&mut self, count: usize) {
        self.prev_item(count, false);
    }

    /// Go to first item (legacy)
    pub fn first(&mut self) {
        self.home(false);
    }

    /// Go to last item (legacy)
    pub fn last(&mut self) {
        self.end(false);
    }

    /// Get current file entry
    pub fn current_file(&self) -> Option<&FileEntry> {
        self.current_files().get(self.current_index())
    }

    /// Get current path (for PhysicalFolder context)
    pub fn current_path(&self) -> Option<&UniversalPath> {
        match &self.context {
            NavigationContext::PhysicalFolder { path, .. } => Some(path),
            NavigationContext::Archive { archive_path, .. } => Some(archive_path),
            _ => None,
        }
    }

    // ========================================
    // Enter logic (nav.enter)
    // ========================================

    /// Determine enter action based on threshold
    /// Returns: (should_enter_browser, should_open_viewer, file_count)
    pub fn should_enter_viewer(&self, threshold: Option<i32>) -> (bool, bool, usize) {
        let threshold = threshold.unwrap_or(self.enter_threshold) as usize;

        if let Some(entry) = self.current_file() {
            if entry.is_dir {
                // For directories, we need to check child count
                // This would need filesystem access, so return false for now
                // The actual check should be done in the app layer
                (true, false, 0)
            } else {
                // For files, always open in viewer
                (false, true, 1)
            }
        } else {
            (false, false, 0)
        }
    }

    /// Check if current selection is a directory
    pub fn is_current_dir(&self) -> bool {
        self.current_file().map(|f| f.is_dir).unwrap_or(false)
    }

    /// Check if current selection is an image
    pub fn is_current_image(&self) -> bool {
        self.current_file()
            .map(|f| {
                let ext = f.name.rsplit('.').next().unwrap_or("").to_lowercase();
                matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "tif")
            })
            .unwrap_or(false)
    }
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::new()
    }
}
