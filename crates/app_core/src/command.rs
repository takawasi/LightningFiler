//! Command system for user actions
//! Based on Doc 3: Input/UX Specification

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

    // ========================================
    // Navigation Commands (nav.*)
    // ========================================

    // A. Grid/List cursor movement
    pub const NAV_MOVE_UP: &'static str = "nav.move_up";
    pub const NAV_MOVE_DOWN: &'static str = "nav.move_down";
    pub const NAV_MOVE_LEFT: &'static str = "nav.move_left";
    pub const NAV_MOVE_RIGHT: &'static str = "nav.move_right";
    pub const NAV_PAGE_UP: &'static str = "nav.page_up";
    pub const NAV_PAGE_DOWN: &'static str = "nav.page_down";
    pub const NAV_HOME: &'static str = "nav.home";
    pub const NAV_END: &'static str = "nav.end";

    // B. Logical item movement (Viewer/Browser)
    pub const NAV_NEXT_ITEM: &'static str = "nav.next_item";
    pub const NAV_PREV_ITEM: &'static str = "nav.prev_item";
    pub const NAV_NEXT_PAGE: &'static str = "nav.next_page";
    pub const NAV_PREV_PAGE: &'static str = "nav.prev_page";

    // C. Hierarchy/folder movement
    pub const NAV_ENTER: &'static str = "nav.enter";
    pub const NAV_PARENT: &'static str = "nav.parent";
    pub const NAV_NEXT_SIBLING: &'static str = "nav.next_sibling";
    pub const NAV_PREV_SIBLING: &'static str = "nav.prev_sibling";
    pub const NAV_ROOT: &'static str = "nav.root";

    // D. Scroll
    pub const NAV_SCROLL_Y: &'static str = "nav.scroll_y";
    pub const NAV_SCROLL_X: &'static str = "nav.scroll_x";

    // Legacy aliases
    pub const NAV_FIRST_ITEM: &'static str = "nav.home";
    pub const NAV_LAST_ITEM: &'static str = "nav.end";
    pub const NAV_UP_FOLDER: &'static str = "nav.parent";
    pub const NAV_ENTER_FOLDER: &'static str = "nav.enter";
    pub const NAV_SKIP_FORWARD: &'static str = "nav.next_item";
    pub const NAV_SKIP_BACKWARD: &'static str = "nav.prev_item";

    // ========================================
    // View Commands (view.*)
    // ========================================

    // A. Zoom & Scale
    pub const VIEW_ZOOM_IN: &'static str = "view.zoom_in";
    pub const VIEW_ZOOM_OUT: &'static str = "view.zoom_out";
    pub const VIEW_ZOOM_SET: &'static str = "view.zoom_set";
    pub const VIEW_ZOOM_MODE_CYCLE: &'static str = "view.zoom_mode_cycle";
    pub const VIEW_LOCK_ZOOM: &'static str = "view.lock_zoom";
    pub const VIEW_ZOOM_RESET: &'static str = "view.zoom_set"; // alias
    pub const VIEW_FIT_TO_WINDOW: &'static str = "view.zoom_set";
    pub const VIEW_ORIGINAL_SIZE: &'static str = "view.zoom_set";

    // B. Pan & Scroll
    pub const VIEW_PAN: &'static str = "view.pan";
    pub const VIEW_PAN_TO: &'static str = "view.pan_to";
    pub const VIEW_SCROLL_UP: &'static str = "view.scroll_up";
    pub const VIEW_SCROLL_DOWN: &'static str = "view.scroll_down";
    pub const VIEW_SMART_SCROLL_UP: &'static str = "view.smart_scroll_up";
    pub const VIEW_SMART_SCROLL_DOWN: &'static str = "view.smart_scroll_down";
    pub const VIEW_SCROLL_N_TYPE_UP: &'static str = "view.scroll_n_type_up";
    pub const VIEW_SCROLL_N_TYPE_DOWN: &'static str = "view.scroll_n_type_down";
    pub const VIEW_TOGGLE_SNAP: &'static str = "view.toggle_snap";

    // C. Multi-view & Compare
    pub const VIEW_SPLIT_MODE: &'static str = "view.split_mode";
    pub const VIEW_NEXT_VIEW_AREA: &'static str = "view.next_view_area";
    pub const VIEW_SYNC_SCROLL: &'static str = "view.sync_scroll";
    pub const VIEW_COPY_VIEW_STATE: &'static str = "view.copy_view_state";

    // D. Viewer navigation
    pub const VIEW_NEXT_ITEM: &'static str = "view.next_item";
    pub const VIEW_PREV_ITEM: &'static str = "view.prev_item";
    pub const VIEW_NEXT_FOLDER: &'static str = "view.next_folder";
    pub const VIEW_PREV_FOLDER: &'static str = "view.prev_folder";
    pub const VIEW_SEEK: &'static str = "view.seek";
    pub const VIEW_PARENT: &'static str = "view.parent";

    // E. Slideshow
    pub const VIEW_SLIDESHOW: &'static str = "view.slideshow";
    pub const VIEW_SLIDESHOW_INTERVAL: &'static str = "view.slideshow_interval";

    // F. Display settings
    pub const VIEW_ROTATE: &'static str = "view.rotate";
    pub const VIEW_FLIP: &'static str = "view.flip";
    pub const VIEW_SPREAD_MODE: &'static str = "view.spread_mode";
    pub const VIEW_TOGGLE_TRANSITION: &'static str = "view.toggle_transition";
    pub const VIEW_TOGGLE_INFO: &'static str = "view.toggle_info";
    pub const VIEW_TOGGLE_FULLSCREEN: &'static str = "view.toggle_fullscreen";
    pub const VIEW_TOGGLE_CHROMELESS: &'static str = "view.toggle_chromeless";
    pub const VIEW_SET_BACKGROUND: &'static str = "view.set_background";
    pub const VIEW_QUICK_LOOK: &'static str = "view.quick_look";

    // Legacy aliases
    pub const VIEW_ROTATE_LEFT: &'static str = "view.rotate";
    pub const VIEW_ROTATE_RIGHT: &'static str = "view.rotate";

    // ========================================
    // File Commands (file.*)
    // ========================================

    // A. Clipboard
    pub const FILE_COPY: &'static str = "file.copy";
    pub const FILE_CUT: &'static str = "file.cut";
    pub const FILE_PASTE: &'static str = "file.paste";
    pub const FILE_COPY_IMAGE: &'static str = "file.copy_image";
    pub const FILE_COPY_PATH: &'static str = "file.copy_path";

    // B. File system
    pub const FILE_DELETE: &'static str = "file.delete";
    pub const FILE_RENAME: &'static str = "file.rename";
    pub const FILE_CREATE_DIR: &'static str = "file.create_dir";
    pub const FILE_COPY_TO: &'static str = "file.copy_to";
    pub const FILE_MOVE_TO: &'static str = "file.move_to";

    // C. External/Shell
    pub const FILE_OPEN_EXPLORER: &'static str = "file.open_explorer";
    pub const FILE_OPEN_WITH: &'static str = "file.open_with";
    pub const FILE_OPEN_EXTERNAL: &'static str = "file.open_external";
    pub const FILE_PROPERTIES: &'static str = "file.properties";

    // ========================================
    // Metadata Commands (meta.*)
    // ========================================

    pub const META_RATE: &'static str = "meta.rate";
    pub const META_RATE_STEP: &'static str = "meta.rate_step";
    pub const META_LABEL: &'static str = "meta.label";
    pub const META_TAG_TOGGLE: &'static str = "meta.tag_toggle";
    pub const META_TAG_ADD: &'static str = "meta.tag_add";
    pub const META_TAG_REMOVE: &'static str = "meta.tag_remove";
    pub const META_EDIT_TAGS: &'static str = "meta.edit_tags";
    pub const META_COPY_META: &'static str = "meta.copy_meta";
    pub const META_EDIT_COMMENT: &'static str = "meta.edit_comment";
    pub const META_TOGGLE_MARK: &'static str = "meta.toggle_mark";
    pub const META_SELECT_MARKED: &'static str = "meta.select_marked";

    // ========================================
    // App Commands (app.*)
    // ========================================

    pub const APP_EXIT: &'static str = "app.exit";
    pub const APP_RESTART: &'static str = "app.restart";
    pub const APP_OPEN_SETTINGS: &'static str = "app.open_settings";
    pub const APP_OPEN_MANUAL: &'static str = "app.open_manual";
    pub const APP_ABOUT: &'static str = "app.about";
    pub const APP_CLEAR_CACHE: &'static str = "app.clear_cache";
    pub const APP_MINIMIZE: &'static str = "app.minimize";
    pub const APP_MAXIMIZE: &'static str = "app.maximize";
    pub const APP_TOPMOST: &'static str = "app.topmost";
    pub const APP_NEW_WINDOW: &'static str = "app.new_window";
    pub const APP_TOGGLE_PANEL: &'static str = "app.toggle_panel";
    pub const APP_FOCUS_PANEL: &'static str = "app.focus_panel";
    pub const APP_LAYOUT_SAVE: &'static str = "app.layout_save";
    pub const APP_LAYOUT_LOAD: &'static str = "app.layout_load";
    pub const APP_LAYOUT_RESET: &'static str = "app.layout_reset";
    pub const APP_SEARCH: &'static str = "app.search";

    // Legacy alias
    pub const APP_QUIT: &'static str = "app.exit";
}

/// Command with optional parameters
#[derive(Debug, Clone)]
pub struct Command {
    pub id: CommandId,
    pub params: CommandParams,
}

/// Command parameters based on Doc 3 specification
#[derive(Debug, Clone, Default)]
pub struct CommandParams {
    // Navigation parameters
    /// Movement amount (nav.move_*, nav.page_*, nav.next_item, etc.)
    pub amount: Option<i32>,
    /// Select while moving (nav.move_*, nav.page_*, nav.home, nav.end)
    pub select: Option<bool>,
    /// Wrap around at boundaries (nav.move_left/right, nav.next_item, nav.prev_item)
    pub wrap: Option<bool>,
    /// Cross folder boundary (nav.next_item, nav.prev_item)
    pub cross_folder: Option<bool>,
    /// File count threshold for nav.enter (<=threshold -> Viewer, >threshold -> Browser)
    pub threshold: Option<i32>,
    /// Skip empty folders (nav.next_sibling, nav.prev_sibling)
    pub skip_empty: Option<bool>,

    // View parameters
    /// Zoom step (view.zoom_in, view.zoom_out)
    pub step: Option<f32>,
    /// Zoom/pan center (Cursor/Center)
    pub center: Option<CenterMode>,
    /// Zoom mode (Original/FitWindow/FitWidth/FitHeight)
    pub mode: Option<ZoomMode>,
    /// Scale value (view.zoom_set)
    pub scale: Option<f32>,
    /// Toggle back to original if same mode (view.zoom_set)
    pub toggle_origin: Option<bool>,
    /// Toggle state (general)
    pub toggle: Option<bool>,
    /// Pan direction (Up/Down/Left/Right)
    pub direction: Option<Direction>,
    /// Unit for scroll/pan (Pixel/Screen/Line/Page)
    pub unit: Option<ScrollUnit>,
    /// Scroll multiplier
    pub multiplier: Option<f32>,
    /// Overlap amount for smart scroll
    pub overlap: Option<i32>,
    /// Position for pan_to (TopLeft/TopRight/BottomLeft/BottomRight/Center)
    pub position: Option<Position>,
    /// Seek position (0.0-1.0)
    pub seek_position: Option<f32>,
    /// Sync mode for multi-view
    pub sync_mode: Option<SyncMode>,

    // Slideshow parameters
    /// Slideshow action (Start/Stop/Toggle)
    pub action: Option<SlideshowAction>,
    /// Slideshow order (Normal/Reverse/Shuffle/Random)
    pub order: Option<SlideshowOrder>,
    /// Relative adjustment
    pub relative: Option<bool>,

    // Display parameters
    /// Rotation angle
    pub angle: Option<i32>,
    /// Flip axis (Horizontal/Vertical)
    pub axis: Option<FlipAxis>,
    /// Spread mode (Single/Spread/Auto)
    pub spread: Option<SpreadMode>,
    /// Background color
    pub color: Option<BackgroundColor>,
    /// Info display level
    pub level: Option<InfoLevel>,
    /// Transition mode
    pub transition: Option<TransitionMode>,

    // File parameters
    /// Use trash instead of delete
    pub trash: Option<bool>,
    /// Show confirmation dialog
    pub confirm: Option<bool>,
    /// Show dialog for rename
    pub dialog: Option<bool>,
    /// Target path for copy_to/move_to
    pub target: Option<String>,
    /// Path format (Full/Name/Dir)
    pub format: Option<PathFormat>,
    /// External app ID
    pub app_id: Option<String>,
    /// External app arguments
    pub args: Option<String>,

    // Metadata parameters
    /// Rating value (0-5)
    pub value: Option<i32>,
    /// Loop rating
    pub r#loop: Option<bool>,
    /// Label color
    pub label_color: Option<LabelColor>,
    /// Tag name
    pub name: Option<String>,
    /// Copy target (Rating/Tags/All)
    pub copy_target: Option<CopyTarget>,
    /// Panel ID
    pub panel_id: Option<String>,
    /// Layout slot
    pub slot: Option<i32>,
    /// Settings page
    pub page: Option<String>,

    // Generic
    /// Integer value (legacy)
    pub int_value: Option<i64>,
    /// String value (legacy)
    pub string_value: Option<String>,
    /// Path value (legacy)
    pub path_value: Option<String>,
}

// Enums for command parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CenterMode { Cursor, Center }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomMode { Original, FitWindow, FitWidth, FitHeight }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction { Up, Down, Left, Right }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollUnit { Pixel, Screen, Line, Page }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position { TopLeft, TopRight, BottomLeft, BottomRight, Center }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode { None, Position, Relative }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideshowAction { Start, Stop, Toggle }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideshowOrder { Normal, Reverse, Shuffle, Random }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlipAxis { Horizontal, Vertical }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadMode { Single, Spread, Auto }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundColor { Black, Gray, Check, White, Transparent }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoLevel { None, Simple, Detail }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionMode { None, Fade, Slide }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathFormat { Full, Name, Dir }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelColor { Red, Blue, Green, Yellow, Purple, None }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyTarget { Rating, Tags, All }

impl Command {
    pub fn new(id: &str) -> Self {
        Self {
            id: CommandId::new(id),
            params: CommandParams::default(),
        }
    }

    // Navigation builders
    pub fn with_amount(mut self, amount: i32) -> Self {
        self.params.amount = Some(amount);
        self
    }

    pub fn with_select(mut self, select: bool) -> Self {
        self.params.select = Some(select);
        self
    }

    pub fn with_wrap(mut self, wrap: bool) -> Self {
        self.params.wrap = Some(wrap);
        self
    }

    pub fn with_cross_folder(mut self, cross: bool) -> Self {
        self.params.cross_folder = Some(cross);
        self
    }

    pub fn with_threshold(mut self, threshold: i32) -> Self {
        self.params.threshold = Some(threshold);
        self
    }

    pub fn with_skip_empty(mut self, skip: bool) -> Self {
        self.params.skip_empty = Some(skip);
        self
    }

    // View builders
    pub fn with_step(mut self, step: f32) -> Self {
        self.params.step = Some(step);
        self
    }

    pub fn with_center(mut self, center: CenterMode) -> Self {
        self.params.center = Some(center);
        self
    }

    pub fn with_zoom_mode(mut self, mode: ZoomMode) -> Self {
        self.params.mode = Some(mode);
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.params.scale = Some(scale);
        self
    }

    pub fn with_toggle(mut self, toggle: bool) -> Self {
        self.params.toggle = Some(toggle);
        self
    }

    pub fn with_direction(mut self, dir: Direction) -> Self {
        self.params.direction = Some(dir);
        self
    }

    pub fn with_unit(mut self, unit: ScrollUnit) -> Self {
        self.params.unit = Some(unit);
        self
    }

    pub fn with_overlap(mut self, overlap: i32) -> Self {
        self.params.overlap = Some(overlap);
        self
    }

    pub fn with_position(mut self, pos: Position) -> Self {
        self.params.position = Some(pos);
        self
    }

    pub fn with_angle(mut self, angle: i32) -> Self {
        self.params.angle = Some(angle);
        self
    }

    pub fn with_axis(mut self, axis: FlipAxis) -> Self {
        self.params.axis = Some(axis);
        self
    }

    pub fn with_spread_mode(mut self, spread: SpreadMode) -> Self {
        self.params.spread = Some(spread);
        self
    }

    pub fn with_background(mut self, color: BackgroundColor) -> Self {
        self.params.color = Some(color);
        self
    }

    // File builders
    pub fn with_trash(mut self, trash: bool) -> Self {
        self.params.trash = Some(trash);
        self
    }

    pub fn with_confirm(mut self, confirm: bool) -> Self {
        self.params.confirm = Some(confirm);
        self
    }

    pub fn with_dialog(mut self, dialog: bool) -> Self {
        self.params.dialog = Some(dialog);
        self
    }

    pub fn with_target(mut self, target: &str) -> Self {
        self.params.target = Some(target.to_string());
        self
    }

    pub fn with_path_format(mut self, format: PathFormat) -> Self {
        self.params.format = Some(format);
        self
    }

    pub fn with_app_id(mut self, app_id: &str) -> Self {
        self.params.app_id = Some(app_id.to_string());
        self
    }

    // Metadata builders
    pub fn with_value(mut self, value: i32) -> Self {
        self.params.value = Some(value);
        self
    }

    pub fn with_label(mut self, color: LabelColor) -> Self {
        self.params.label_color = Some(color);
        self
    }

    pub fn with_tag_name(mut self, name: &str) -> Self {
        self.params.name = Some(name.to_string());
        self
    }

    pub fn with_panel(mut self, panel_id: &str) -> Self {
        self.params.panel_id = Some(panel_id.to_string());
        self
    }

    pub fn with_slot(mut self, slot: i32) -> Self {
        self.params.slot = Some(slot);
        self
    }

    // Legacy builders
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
