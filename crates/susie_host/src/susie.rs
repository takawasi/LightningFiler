//! Susie Plugin API definitions and loading
//!
//! Susie plugins (.spi) are 32-bit Windows DLLs that implement a specific API
//! for image decoding and archive handling.
//!
//! This module is Windows-only. On non-Windows platforms, stub types are provided.

#[cfg(windows)]
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_int, c_void};
#[cfg(windows)]
use std::path::Path;

/// Susie plugin handle
#[cfg(windows)]
pub struct SusiePlugin {
    _library: Library,
    get_plugin_info: GetPluginInfo,
    is_supported: IsSupported,
    get_picture: Option<GetPicture>,
    get_archive_info: Option<GetArchiveInfo>,
    get_file: Option<GetFile>,
}

#[cfg(not(windows))]
pub struct SusiePlugin {
    _dummy: (),
}

// Susie API function types (Windows only - stdcall calling convention)
#[cfg(windows)]
type GetPluginInfo = unsafe extern "stdcall" fn(c_int, *mut c_char, c_int) -> c_int;
#[cfg(windows)]
type IsSupported = unsafe extern "stdcall" fn(*const c_char, *const c_void) -> c_int;
#[cfg(windows)]
type GetPicture = unsafe extern "stdcall" fn(*const c_char, i32, u32, *mut *mut c_void, *mut *mut c_void, ProgressCallback, i32) -> c_int;
#[cfg(windows)]
type GetArchiveInfo = unsafe extern "stdcall" fn(*const c_char, i32, u32, *mut *mut c_void) -> c_int;
#[cfg(windows)]
type GetFile = unsafe extern "stdcall" fn(*const c_char, i32, *mut c_char, u32, ProgressCallback, i32) -> c_int;
#[cfg(windows)]
type ProgressCallback = Option<extern "stdcall" fn(c_int, c_int, i32) -> c_int>;

/// Plugin info types
pub const INFO_TYPE_NAME: c_int = 0;
pub const INFO_TYPE_EXT: c_int = 1;

/// Error codes
pub const SPI_SUCCESS: c_int = 0;
pub const SPI_UNSUPPORTED: c_int = -1;
pub const SPI_ABORT: c_int = 1;
pub const SPI_ERROR: c_int = 2;

#[cfg(windows)]
impl SusiePlugin {
    /// Load a Susie plugin from a file
    pub unsafe fn load(path: &Path) -> anyhow::Result<Self> {
        let library = Library::new(path)?;

        // Required functions - extract raw function pointers first
        let get_plugin_info: Symbol<GetPluginInfo> = library.get(b"GetPluginInfo\0")?;
        let get_plugin_info_fn = *get_plugin_info;
        drop(get_plugin_info);

        let is_supported: Symbol<IsSupported> = library.get(b"IsSupported\0")?;
        let is_supported_fn = *is_supported;
        drop(is_supported);

        // Optional functions (image plugins have GetPicture, archive plugins have GetArchiveInfo/GetFile)
        let get_picture_fn = library.get::<GetPicture>(b"GetPicture\0").ok().map(|s| *s);
        let get_archive_info_fn = library.get::<GetArchiveInfo>(b"GetArchiveInfo\0").ok().map(|s| *s);
        let get_file_fn = library.get::<GetFile>(b"GetFile\0").ok().map(|s| *s);

        Ok(Self {
            _library: library,
            get_plugin_info: get_plugin_info_fn,
            is_supported: is_supported_fn,
            get_picture: get_picture_fn,
            get_archive_info: get_archive_info_fn,
            get_file: get_file_fn,
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
#[cfg(windows)]
pub struct PluginManager {
    plugins: Vec<(u32, SusiePlugin)>,
    next_id: u32,
}

#[cfg(not(windows))]
pub struct PluginManager {
    _dummy: (),
}

#[cfg(windows)]
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

#[cfg(windows)]
impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(windows))]
impl PluginManager {
    pub fn new() -> Self {
        Self { _dummy: () }
    }
}

#[cfg(not(windows))]
impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
