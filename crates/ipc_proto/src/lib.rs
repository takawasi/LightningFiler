//! IPC Protocol definitions for Main <-> Susie Bridge communication
//!
//! This crate defines the shared data structures and protocol for inter-process
//! communication between the 64-bit main process and 32-bit Susie plugin bridge.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Pixel format for image data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub enum PixelFormat {
    Rgba8,
    Bgra8,
    Rgb8,
    Bgr8,
    Gray8,
    GrayAlpha8,
}

impl PixelFormat {
    /// Bytes per pixel
    pub fn bytes_per_pixel(&self) -> u32 {
        match self {
            PixelFormat::Rgba8 | PixelFormat::Bgra8 => 4,
            PixelFormat::Rgb8 | PixelFormat::Bgr8 => 3,
            PixelFormat::GrayAlpha8 => 2,
            PixelFormat::Gray8 => 1,
        }
    }
}

/// Commands sent from Main (64-bit) to Bridge (32-bit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeCommand {
    /// Load a Susie plugin
    LoadPlugin { path: String },

    /// Unload a plugin
    UnloadPlugin { plugin_id: u32 },

    /// Check if a file is supported
    IsSupported { plugin_id: u32, header: Vec<u8> },

    /// Decode an image file
    GetPicture {
        plugin_id: u32,
        file_path: String,
        /// Offset in archive (for .spi that read from archive directly)
        offset: u64,
        /// Total file size
        total_size: u64,
    },

    /// Get archive file list
    GetArchiveList { plugin_id: u32, archive_path: String },

    /// Extract a file from archive
    ExtractFile {
        plugin_id: u32,
        archive_path: String,
        inner_path: String,
        dest_path: Option<String>,
    },

    /// Health check
    Ping,

    /// Graceful shutdown
    Shutdown,
}

/// Responses from Bridge (32-bit) to Main (64-bit)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeResponse {
    /// Plugin loaded successfully
    PluginLoaded {
        plugin_id: u32,
        name: String,
        version: String,
        supported_extensions: Vec<String>,
    },

    /// Plugin unloaded
    PluginUnloaded { plugin_id: u32 },

    /// File is supported by the plugin
    Supported { supported: bool },

    /// Image ready in shared memory
    ImageReady {
        /// Shared memory handle name (e.g., "Local\\LF_IMG_{uuid}")
        shmem_handle: String,
        width: u32,
        height: u32,
        /// Row stride with 256-byte alignment for wgpu
        aligned_stride: u32,
        format: PixelFormat,
        /// Total size in bytes
        size: usize,
    },

    /// Archive contents
    ArchiveList { entries: Vec<ArchiveEntry> },

    /// File extracted to path or memory
    FileExtracted {
        /// If dest_path was provided, file is written there
        path: Option<String>,
        /// If dest_path was None, data is in shared memory
        shmem_handle: Option<String>,
        size: usize,
    },

    /// Pong response to Ping
    Pong,

    /// Error occurred
    Error { code: ErrorCode, message: String },
}

/// Archive entry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
    pub compressed_size: u64,
    pub is_directory: bool,
    pub timestamp: Option<i64>,
}

/// Error codes for IPC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    PluginNotFound,
    PluginLoadFailed,
    FileNotFound,
    FileAccessDenied,
    UnsupportedFormat,
    DecodeFailed,
    MemoryAllocationFailed,
    EncodingError,
    ArchiveCorrupted,
    Timeout,
    Unknown,
}

/// Named pipe name for IPC
pub fn pipe_name() -> String {
    format!("\\\\.\\pipe\\LightningFiler_{}", std::process::id())
}

/// Generate a shared memory name
pub fn shmem_name() -> String {
    format!("Local\\LF_IMG_{}", Uuid::new_v4())
}

/// Calculate aligned stride for wgpu (256-byte alignment)
pub fn calculate_aligned_stride(width: u32, bytes_per_pixel: u32) -> u32 {
    let original_stride = width * bytes_per_pixel;
    (original_stride + 255) & !255
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aligned_stride() {
        // 1920 * 4 = 7680, already aligned to 256
        assert_eq!(calculate_aligned_stride(1920, 4), 7680);

        // 1000 * 4 = 4000, needs padding to 4096
        assert_eq!(calculate_aligned_stride(1000, 4), 4096);

        // 100 * 4 = 400, needs padding to 512
        assert_eq!(calculate_aligned_stride(100, 4), 512);
    }

    #[test]
    fn test_serialization() {
        let cmd = BridgeCommand::GetPicture {
            plugin_id: 1,
            file_path: "test.jpg".to_string(),
            offset: 0,
            total_size: 1024,
        };

        let encoded = bincode::serialize(&cmd).unwrap();
        let decoded: BridgeCommand = bincode::deserialize(&encoded).unwrap();

        match decoded {
            BridgeCommand::GetPicture { file_path, .. } => {
                assert_eq!(file_path, "test.jpg");
            }
            _ => panic!("Wrong variant"),
        }
    }
}
