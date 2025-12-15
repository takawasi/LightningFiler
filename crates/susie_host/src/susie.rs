//! Susie Plugin API definitions and loading
//!
//! Susie plugins (.spi) are 32-bit Windows DLLs that implement a specific API
//! for image decoding and archive handling.

use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void};
use std::path::Path;

/// Susie plugin handle
pub struct SusiePlugin {
    _library: Library,
    get_plugin_info: GetPluginInfo,
    is_supported: IsSupported,
    get_picture: Option<GetPicture>,
    get_archive_info: Option<GetArchiveInfo>,
    get_file: Option<GetFile>,
}

// Susie API function types
type GetPluginInfo = unsafe extern "stdcall" fn(c_int, *mut c_char, c_int) -> c_int;
type IsSupported = unsafe extern "stdcall" fn(*const c_char, *const c_void) -> c_int;
type GetPicture = unsafe extern "stdcall" fn(*const c_char, i32, u32, *mut *mut c_void, *mut *mut c_void, ProgressCallback, i32) -> c_int;
type GetArchiveInfo = unsafe extern "stdcall" fn(*const c_char, i32, u32, *mut *mut c_void) -> c_int;
type GetFile = unsafe extern "stdcall" fn(*const c_char, i32, *mut c_char, u32, ProgressCallback, i32) -> c_int;
type ProgressCallback = Option<extern "stdcall" fn(c_int, c_int, i32) -> c_int>;

/// Plugin info types
pub const INFO_TYPE_NAME: c_int = 0;
pub const INFO_TYPE_EXT: c_int = 1;

/// Error codes
pub const SPI_SUCCESS: c_int = 0;
pub const SPI_UNSUPPORTED: c_int = -1;
pub const SPI_ABORT: c_int = 1;
pub const SPI_ERROR: c_int = 2;

impl SusiePlugin {
    /// Load a Susie plugin from a file
    pub unsafe fn load(path: &Path) -> anyhow::Result<Self> {
        let library = Library::new(path)?;

        // Required functions
        let get_plugin_info: Symbol<GetPluginInfo> = library.get(b"GetPluginInfo\0")?;
        let is_supported: Symbol<IsSupported> = library.get(b"IsSupported\0")?;

        // Optional functions (image plugins have GetPicture, archive plugins have GetArchiveInfo/GetFile)
        let get_picture: Option<Symbol<GetPicture>> = library.get(b"GetPicture\0").ok();
        let get_archive_info: Option<Symbol<GetArchiveInfo>> = library.get(b"GetArchiveInfo\0").ok();
        let get_file: Option<Symbol<GetFile>> = library.get(b"GetFile\0").ok();

        Ok(Self {
            _library: library,
            get_plugin_info: *get_plugin_info,
            is_supported: *is_supported,
            get_picture: get_picture.map(|s| *s),
            get_archive_info: get_archive_info.map(|s| *s),
            get_file: get_file.map(|s| *s),
        })
    }

    /// Get plugin name
    pub fn get_name(&self) -> String {
        let mut buffer = vec![0u8; 256];
        unsafe {
            let len = (self.get_plugin_info)(INFO_TYPE_NAME, buffer.as_mut_ptr() as *mut c_char, buffer.len() as c_int);
            if len > 0 {
                buffer.truncate(len as usize);
                String::from_utf8_lossy(&buffer).to_string()
            } else {
                String::new()
            }
        }
    }

    /// Get supported extensions
    pub fn get_extensions(&self) -> Vec<String> {
        let mut extensions = Vec::new();
        let mut buffer = vec![0u8; 256];

        unsafe {
            let len = (self.get_plugin_info)(INFO_TYPE_EXT, buffer.as_mut_ptr() as *mut c_char, buffer.len() as c_int);
            if len > 0 {
                buffer.truncate(len as usize);
                let ext_str = String::from_utf8_lossy(&buffer);
                // Format: "*.jpg;*.jpeg" or "*.zip;*.lzh"
                for ext in ext_str.split(';') {
                    let ext = ext.trim().trim_start_matches("*.");
                    if !ext.is_empty() {
                        extensions.push(ext.to_lowercase());
                    }
                }
            }
        }

        extensions
    }

    /// Check if a file is supported
    pub fn is_supported(&self, path: &str, header: &[u8]) -> bool {
        let c_path = std::ffi::CString::new(path).unwrap();
        unsafe {
            let result = (self.is_supported)(c_path.as_ptr(), header.as_ptr() as *const c_void);
            result != 0
        }
    }

    /// Is this an image plugin?
    pub fn is_image_plugin(&self) -> bool {
        self.get_picture.is_some()
    }

    /// Is this an archive plugin?
    pub fn is_archive_plugin(&self) -> bool {
        self.get_archive_info.is_some() && self.get_file.is_some()
    }
}

/// Plugin manager for loading and managing multiple plugins
pub struct PluginManager {
    plugins: Vec<(u32, SusiePlugin)>,
    next_id: u32,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            next_id: 1,
        }
    }

    /// Load a plugin and return its ID
    pub fn load_plugin(&mut self, path: &Path) -> anyhow::Result<u32> {
        let plugin = unsafe { SusiePlugin::load(path)? };
        let id = self.next_id;
        self.next_id += 1;
        self.plugins.push((id, plugin));
        Ok(id)
    }

    /// Get a plugin by ID
    pub fn get_plugin(&self, id: u32) -> Option<&SusiePlugin> {
        self.plugins.iter().find(|(pid, _)| *pid == id).map(|(_, p)| p)
    }

    /// Unload a plugin
    pub fn unload_plugin(&mut self, id: u32) -> bool {
        if let Some(idx) = self.plugins.iter().position(|(pid, _)| *pid == id) {
            self.plugins.remove(idx);
            true
        } else {
            false
        }
    }

    /// Find a plugin that supports the given file
    pub fn find_supporting_plugin(&self, path: &str, header: &[u8]) -> Option<u32> {
        for (id, plugin) in &self.plugins {
            if plugin.is_supported(path, header) {
                return Some(*id);
            }
        }
        None
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
