//! Command system for user actions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Command identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommandId(pub String);

impl CommandId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    // Navigation commands
    pub const NAV_NEXT_ITEM: &'static str = "nav.next_item";
    pub const NAV_PREV_ITEM: &'static str = "nav.prev_item";
    pub const NAV_FIRST_ITEM: &'static str = "nav.first_item";
    pub const NAV_LAST_ITEM: &'static str = "nav.last_item";
    pub const NAV_UP_FOLDER: &'static str = "nav.up_folder";
    pub const NAV_ENTER_FOLDER: &'static str = "nav.enter_folder";
    pub const NAV_SKIP_FORWARD: &'static str = "nav.skip_forward";
    pub const NAV_SKIP_BACKWARD: &'static str = "nav.skip_backward";

    // View commands
    pub const VIEW_TOGGLE_FULLSCREEN: &'static str = "view.toggle_fullscreen";
    pub const VIEW_ZOOM_IN: &'static str = "view.zoom_in";
    pub const VIEW_ZOOM_OUT: &'static str = "view.zoom_out";
    pub const VIEW_ZOOM_RESET: &'static str = "view.zoom_reset";
    pub const VIEW_FIT_TO_WINDOW: &'static str = "view.fit_to_window";
    pub const VIEW_ORIGINAL_SIZE: &'static str = "view.original_size";
    pub const VIEW_ROTATE_LEFT: &'static str = "view.rotate_left";
    pub const VIEW_ROTATE_RIGHT: &'static str = "view.rotate_right";

    // File commands
    pub const FILE_DELETE: &'static str = "file.delete";
    pub const FILE_RENAME: &'static str = "file.rename";
    pub const FILE_COPY: &'static str = "file.copy";
    pub const FILE_CUT: &'static str = "file.cut";
    pub const FILE_PASTE: &'static str = "file.paste";
    pub const FILE_MOVE_TO: &'static str = "file.move_to";
    pub const FILE_COPY_TO: &'static str = "file.copy_to";

    // App commands
    pub const APP_OPEN_SETTINGS: &'static str = "app.open_settings";
    pub const APP_QUIT: &'static str = "app.quit";
    pub const APP_SEARCH: &'static str = "app.search";
}

/// Command with optional parameters
#[derive(Debug, Clone)]
pub struct Command {
    pub id: CommandId,
    pub params: CommandParams,
}

/// Command parameters
#[derive(Debug, Clone, Default)]
pub struct CommandParams {
    pub int_value: Option<i64>,
    pub string_value: Option<String>,
    pub path_value: Option<String>,
}

impl Command {
    pub fn new(id: &str) -> Self {
        Self {
            id: CommandId::new(id),
            params: CommandParams::default(),
        }
    }

    pub fn with_int(mut self, value: i64) -> Self {
        self.params.int_value = Some(value);
        self
    }

    pub fn with_string(mut self, value: &str) -> Self {
        self.params.string_value = Some(value.to_string());
        self
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.params.path_value = Some(path.to_string());
        self
    }
}

/// Command handler trait
pub trait CommandHandler: Send + Sync {
    fn execute(&self, cmd: &Command) -> anyhow::Result<()>;
    fn can_execute(&self, cmd: &Command) -> bool;
}

/// Command dispatcher
pub struct CommandDispatcher {
    handlers: HashMap<String, Box<dyn CommandHandler>>,
}

impl CommandDispatcher {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register<H: CommandHandler + 'static>(&mut self, command_id: &str, handler: H) {
        self.handlers.insert(command_id.to_string(), Box::new(handler));
    }

    pub fn dispatch(&self, cmd: &Command) -> anyhow::Result<()> {
        if let Some(handler) = self.handlers.get(cmd.id.as_str()) {
            if handler.can_execute(cmd) {
                handler.execute(cmd)?;
            } else {
                tracing::debug!("Command {} cannot be executed in current context", cmd.id.as_str());
            }
        } else {
            tracing::warn!("Unknown command: {}", cmd.id.as_str());
        }
        Ok(())
    }

    pub fn can_execute(&self, cmd: &Command) -> bool {
        self.handlers
            .get(cmd.id.as_str())
            .map(|h| h.can_execute(cmd))
            .unwrap_or(false)
    }
}

impl Default for CommandDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
