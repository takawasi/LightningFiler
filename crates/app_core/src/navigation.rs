//! Navigation state and context management

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

/// Navigation state for the application
pub struct NavigationState {
    /// Current context
    pub context: NavigationContext,

    /// Navigation history stack
    history: Vec<NavigationContext>,

    /// Forward stack (for redo)
    forward: Vec<NavigationContext>,
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
        }
    }

    /// Navigate to a new context
    pub fn navigate_to(&mut self, context: NavigationContext) {
        // Save current to history
        let old = std::mem::replace(&mut self.context, context);
        self.history.push(old);

        // Clear forward stack
        self.forward.clear();
    }

    /// Go back in history
    pub fn go_back(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            let current = std::mem::replace(&mut self.context, prev);
            self.forward.push(current);
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
        let max = self.current_files().len().saturating_sub(1);
        let index = index.min(max);

        match &mut self.context {
            NavigationContext::PhysicalFolder { current_index, .. } => *current_index = index,
            NavigationContext::TagSearch { current_index, .. } => *current_index = index,
            NavigationContext::Timeline { current_index, .. } => *current_index = index,
            NavigationContext::Archive { current_index, .. } => *current_index = index,
            NavigationContext::Search { current_index, .. } => *current_index = index,
        }
    }

    /// Move to next item
    pub fn next(&mut self) -> bool {
        let current = self.current_index();
        let max = self.current_files().len().saturating_sub(1);

        if current < max {
            self.set_index(current + 1);
            true
        } else {
            false
        }
    }

    /// Move to previous item
    pub fn prev(&mut self) -> bool {
        let current = self.current_index();

        if current > 0 {
            self.set_index(current - 1);
            true
        } else {
            false
        }
    }

    /// Skip forward by count
    pub fn skip_forward(&mut self, count: usize) {
        let current = self.current_index();
        self.set_index(current.saturating_add(count));
    }

    /// Skip backward by count
    pub fn skip_backward(&mut self, count: usize) {
        let current = self.current_index();
        self.set_index(current.saturating_sub(count));
    }

    /// Go to first item
    pub fn first(&mut self) {
        self.set_index(0);
    }

    /// Go to last item
    pub fn last(&mut self) {
        let max = self.current_files().len().saturating_sub(1);
        self.set_index(max);
    }

    /// Get current file entry
    pub fn current_file(&self) -> Option<&FileEntry> {
        self.current_files().get(self.current_index())
    }
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::new()
    }
}
