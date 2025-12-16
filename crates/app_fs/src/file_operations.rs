//! File operations module
//! Provides clipboard, delete, rename, copy, move operations

use std::path::{Path, PathBuf};
use thiserror::Error;

/// File operation errors
#[derive(Debug, Error)]
pub enum FileOpError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Clipboard error: {0}")]
    #[cfg(feature = "clipboard")]
    Clipboard(String),

    #[error("Trash error: {0}")]
    #[cfg(feature = "trash-support")]
    Trash(#[from] trash::Error),

    #[error("File not found: {0}")]
    NotFound(PathBuf),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("File already exists: {0}")]
    AlreadyExists(PathBuf),
}

pub type Result<T> = std::result::Result<T, FileOpError>;

/// Clipboard operation mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardMode {
    Copy,
    Cut,
}

/// File operations trait
pub trait FileOperations: Send + Sync {
    /// Copy file paths to clipboard
    fn copy_to_clipboard(&self, paths: &[PathBuf], mode: ClipboardMode) -> Result<()>;

    /// Paste files from clipboard to target directory
    fn paste_from_clipboard(&self, target_dir: &Path, cut: bool) -> Result<Vec<PathBuf>>;

    /// Delete files (move to trash or permanent delete)
    fn delete(&self, paths: &[PathBuf], use_trash: bool) -> Result<()>;

    /// Rename a file or directory
    fn rename(&self, from: &Path, to: &Path) -> Result<()>;

    /// Copy files to target directory
    fn copy_to(&self, sources: &[PathBuf], target_dir: &Path) -> Result<Vec<PathBuf>>;

    /// Move files to target directory
    fn move_to(&self, sources: &[PathBuf], target_dir: &Path) -> Result<Vec<PathBuf>>;

    /// Create a new directory
    fn create_dir(&self, path: &Path) -> Result<()>;

    /// Open file in system file explorer (with selection)
    fn open_in_explorer(&self, path: &Path, select: bool) -> Result<()>;

    /// Open file with default application
    fn open_external(&self, path: &Path) -> Result<()>;

    /// Open file with specific application
    fn open_with(&self, path: &Path, app_id: &str, args: Option<&str>) -> Result<()>;
}

/// Default implementation of file operations
pub struct DefaultFileOperations {
    #[cfg(feature = "clipboard")]
    clipboard: parking_lot::Mutex<Option<arboard::Clipboard>>,

    #[cfg(feature = "clipboard")]
    clipboard_mode: parking_lot::Mutex<Option<ClipboardMode>>,
}

impl DefaultFileOperations {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "clipboard")]
            clipboard: parking_lot::Mutex::new(arboard::Clipboard::new().ok()),

            #[cfg(feature = "clipboard")]
            clipboard_mode: parking_lot::Mutex::new(None),
        }
    }
}

impl Default for DefaultFileOperations {
    fn default() -> Self {
        Self::new()
    }
}

impl FileOperations for DefaultFileOperations {
    #[cfg(feature = "clipboard")]
    fn copy_to_clipboard(&self, paths: &[PathBuf], mode: ClipboardMode) -> Result<()> {
        if paths.is_empty() {
            return Ok(());
        }

        // Store clipboard mode for paste operation
        *self.clipboard_mode.lock() = Some(mode);

        // On Windows, use native clipboard format (CF_HDROP) for file paths
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::ffi::OsStrExt;

            // Format: list of null-terminated wide strings, double-null terminated
            let mut data: Vec<u16> = Vec::new();
            for path in paths {
                let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
                data.extend_from_slice(&wide);
                data.push(0); // null terminator
            }
            data.push(0); // double-null terminator

            // Use clipboard text as fallback (arboard doesn't support CF_HDROP directly)
            let text = paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");

            if let Some(clipboard) = self.clipboard.lock().as_mut() {
                clipboard
                    .set_text(&text)
                    .map_err(|e| FileOpError::Clipboard(e.to_string()))?;
            }

            tracing::debug!("Copied {} files to clipboard (mode: {:?})", paths.len(), mode);
        }

        // On Unix-like systems, use text format
        #[cfg(not(target_os = "windows"))]
        {
            let text = paths
                .iter()
                .map(|p| format!("file://{}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");

            if let Some(clipboard) = self.clipboard.lock().as_mut() {
                clipboard
                    .set_text(&text)
                    .map_err(|e| FileOpError::Clipboard(e.to_string()))?;
            }

            tracing::debug!("Copied {} files to clipboard (mode: {:?})", paths.len(), mode);
        }

        Ok(())
    }

    #[cfg(not(feature = "clipboard"))]
    fn copy_to_clipboard(&self, _paths: &[PathBuf], _mode: ClipboardMode) -> Result<()> {
        Err(FileOpError::InvalidOperation(
            "Clipboard feature not enabled".to_string(),
        ))
    }

    #[cfg(feature = "clipboard")]
    fn paste_from_clipboard(&self, target_dir: &Path, cut: bool) -> Result<Vec<PathBuf>> {
        if !target_dir.exists() {
            return Err(FileOpError::NotFound(target_dir.to_path_buf()));
        }

        if !target_dir.is_dir() {
            return Err(FileOpError::InvalidOperation(
                "Target must be a directory".to_string(),
            ));
        }

        let text = if let Some(clipboard) = self.clipboard.lock().as_mut() {
            clipboard
                .get_text()
                .map_err(|e| FileOpError::Clipboard(e.to_string()))?
        } else {
            return Err(FileOpError::Clipboard("Clipboard not available".to_string()));
        };

        // Parse clipboard content as file paths
        let mut pasted_files = Vec::new();

        #[cfg(target_os = "windows")]
        let paths: Vec<PathBuf> = text.lines().map(PathBuf::from).collect();

        #[cfg(not(target_os = "windows"))]
        let paths: Vec<PathBuf> = text
            .lines()
            .filter_map(|line| {
                if let Some(path_str) = line.strip_prefix("file://") {
                    Some(PathBuf::from(path_str))
                } else {
                    Some(PathBuf::from(line))
                }
            })
            .collect();

        for source in paths {
            if !source.exists() {
                tracing::warn!("Skipping non-existent file: {}", source.display());
                continue;
            }

            let file_name = source
                .file_name()
                .ok_or_else(|| FileOpError::InvalidOperation("Invalid file name".to_string()))?;
            let target = target_dir.join(file_name);

            if cut {
                // Move operation
                std::fs::rename(&source, &target)?;
                tracing::debug!("Moved: {} -> {}", source.display(), target.display());
            } else {
                // Copy operation
                if source.is_dir() {
                    copy_dir_recursive(&source, &target)?;
                } else {
                    std::fs::copy(&source, &target)?;
                }
                tracing::debug!("Copied: {} -> {}", source.display(), target.display());
            }

            pasted_files.push(target);
        }

        Ok(pasted_files)
    }

    #[cfg(not(feature = "clipboard"))]
    fn paste_from_clipboard(&self, _target_dir: &Path, _cut: bool) -> Result<Vec<PathBuf>> {
        Err(FileOpError::InvalidOperation(
            "Clipboard feature not enabled".to_string(),
        ))
    }

    #[cfg(feature = "trash-support")]
    fn delete(&self, paths: &[PathBuf], use_trash: bool) -> Result<()> {
        for path in paths {
            if !path.exists() {
                return Err(FileOpError::NotFound(path.clone()));
            }

            if use_trash {
                // Move to trash (safe delete)
                trash::delete(path)?;
                tracing::info!("Moved to trash: {}", path.display());
            } else {
                // Permanent delete
                if path.is_dir() {
                    std::fs::remove_dir_all(path)?;
                } else {
                    std::fs::remove_file(path)?;
                }
                tracing::warn!("Permanently deleted: {}", path.display());
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "trash-support"))]
    fn delete(&self, paths: &[PathBuf], _use_trash: bool) -> Result<()> {
        // Fallback: always permanent delete
        for path in paths {
            if !path.exists() {
                return Err(FileOpError::NotFound(path.clone()));
            }

            if path.is_dir() {
                std::fs::remove_dir_all(path)?;
            } else {
                std::fs::remove_file(path)?;
            }
            tracing::warn!("Permanently deleted: {}", path.display());
        }

        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        if !from.exists() {
            return Err(FileOpError::NotFound(from.to_path_buf()));
        }

        if to.exists() {
            return Err(FileOpError::AlreadyExists(to.to_path_buf()));
        }

        std::fs::rename(from, to)?;
        tracing::info!("Renamed: {} -> {}", from.display(), to.display());

        Ok(())
    }

    fn copy_to(&self, sources: &[PathBuf], target_dir: &Path) -> Result<Vec<PathBuf>> {
        if !target_dir.exists() {
            return Err(FileOpError::NotFound(target_dir.to_path_buf()));
        }

        if !target_dir.is_dir() {
            return Err(FileOpError::InvalidOperation(
                "Target must be a directory".to_string(),
            ));
        }

        let mut copied_files = Vec::new();

        for source in sources {
            if !source.exists() {
                return Err(FileOpError::NotFound(source.clone()));
            }

            let file_name = source
                .file_name()
                .ok_or_else(|| FileOpError::InvalidOperation("Invalid file name".to_string()))?;
            let target = target_dir.join(file_name);

            if source.is_dir() {
                copy_dir_recursive(source, &target)?;
            } else {
                std::fs::copy(source, &target)?;
            }

            tracing::info!("Copied: {} -> {}", source.display(), target.display());
            copied_files.push(target);
        }

        Ok(copied_files)
    }

    fn move_to(&self, sources: &[PathBuf], target_dir: &Path) -> Result<Vec<PathBuf>> {
        if !target_dir.exists() {
            return Err(FileOpError::NotFound(target_dir.to_path_buf()));
        }

        if !target_dir.is_dir() {
            return Err(FileOpError::InvalidOperation(
                "Target must be a directory".to_string(),
            ));
        }

        let mut moved_files = Vec::new();

        for source in sources {
            if !source.exists() {
                return Err(FileOpError::NotFound(source.clone()));
            }

            let file_name = source
                .file_name()
                .ok_or_else(|| FileOpError::InvalidOperation("Invalid file name".to_string()))?;
            let target = target_dir.join(file_name);

            // Try rename first (fast, same filesystem)
            match std::fs::rename(source, &target) {
                Ok(()) => {
                    tracing::info!("Moved: {} -> {}", source.display(), target.display());
                }
                Err(e) => {
                    // Check if it's a cross-filesystem error
                    // Unix: EXDEV = 18, Windows: ERROR_NOT_SAME_DEVICE = 0x11 (17)
                    let is_cross_device = match e.raw_os_error() {
                        Some(18) => cfg!(unix),  // EXDEV on Unix
                        Some(17) => cfg!(windows),  // ERROR_NOT_SAME_DEVICE on Windows
                        _ => false,
                    };

                    if is_cross_device {
                        // Fallback: copy + delete for cross-filesystem moves
                        tracing::info!("Cross-filesystem move, using copy+delete: {} -> {}", source.display(), target.display());
                        if source.is_dir() {
                            // For directories, use recursive copy
                            copy_dir_recursive(source, &target)?;
                        } else {
                            std::fs::copy(source, &target)?;
                        }
                        // Remove original after successful copy
                        if source.is_dir() {
                            std::fs::remove_dir_all(source)?;
                        } else {
                            std::fs::remove_file(source)?;
                        }
                        tracing::info!("Moved (copy+delete): {} -> {}", source.display(), target.display());
                    } else {
                        return Err(e.into());
                    }
                }
            }
            moved_files.push(target);
        }

        Ok(moved_files)
    }

    fn create_dir(&self, path: &Path) -> Result<()> {
        if path.exists() {
            return Err(FileOpError::AlreadyExists(path.to_path_buf()));
        }

        std::fs::create_dir_all(path)?;
        tracing::info!("Created directory: {}", path.display());

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn open_in_explorer(&self, path: &Path, select: bool) -> Result<()> {
        let path_str = path.display().to_string();

        if select {
            // Open Explorer with file selected
            std::process::Command::new("explorer")
                .arg("/select,")
                .arg(&path_str)
                .spawn()
                .map_err(|e| {
                    FileOpError::InvalidOperation(format!("Failed to open Explorer: {}", e))
                })?;
        } else {
            // Open folder in Explorer
            let folder = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(path)
            };

            std::process::Command::new("explorer")
                .arg(folder.display().to_string())
                .spawn()
                .map_err(|e| {
                    FileOpError::InvalidOperation(format!("Failed to open Explorer: {}", e))
                })?;
        }

        tracing::info!("Opened in Explorer: {}", path_str);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn open_in_explorer(&self, path: &Path, select: bool) -> Result<()> {
        let path_str = path.display().to_string();

        if select {
            // Open Finder with file selected
            std::process::Command::new("open")
                .arg("-R")
                .arg(&path_str)
                .spawn()
                .map_err(|e| {
                    FileOpError::InvalidOperation(format!("Failed to open Finder: {}", e))
                })?;
        } else {
            // Open folder in Finder
            let folder = if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(path)
            };

            std::process::Command::new("open")
                .arg(folder.display().to_string())
                .spawn()
                .map_err(|e| {
                    FileOpError::InvalidOperation(format!("Failed to open Finder: {}", e))
                })?;
        }

        tracing::info!("Opened in Finder: {}", path_str);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn open_in_explorer(&self, path: &Path, _select: bool) -> Result<()> {
        // Linux doesn't have a standard "select file" feature
        // Just open the containing folder
        let folder = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };

        std::process::Command::new("xdg-open")
            .arg(folder.display().to_string())
            .spawn()
            .map_err(|e| {
                FileOpError::InvalidOperation(format!("Failed to open file manager: {}", e))
            })?;

        tracing::info!("Opened in file manager: {}", folder.display());
        Ok(())
    }

    #[cfg(feature = "open-external")]
    fn open_external(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Err(FileOpError::NotFound(path.to_path_buf()));
        }

        open::that(path).map_err(|e| {
            FileOpError::InvalidOperation(format!("Failed to open file externally: {}", e))
        })?;

        tracing::info!("Opened externally: {}", path.display());
        Ok(())
    }

    #[cfg(not(feature = "open-external"))]
    fn open_external(&self, _path: &Path) -> Result<()> {
        Err(FileOpError::InvalidOperation(
            "Open external feature not enabled".to_string(),
        ))
    }

    fn open_with(&self, path: &Path, app_id: &str, args: Option<&str>) -> Result<()> {
        if !path.exists() {
            return Err(FileOpError::NotFound(path.to_path_buf()));
        }

        let mut cmd = std::process::Command::new(app_id);
        cmd.arg(path.display().to_string());

        if let Some(args_str) = args {
            // Simple argument parsing (space-separated)
            for arg in args_str.split_whitespace() {
                cmd.arg(arg);
            }
        }

        cmd.spawn().map_err(|e| {
            FileOpError::InvalidOperation(format!("Failed to open with {}: {}", app_id, e))
        })?;

        tracing::info!("Opened with {}: {}", app_id, path.display());
        Ok(())
    }
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_dir() {
        let ops = DefaultFileOperations::new();
        let test_dir = PathBuf::from("test_create_dir");

        // Clean up if exists
        let _ = fs::remove_dir_all(&test_dir);

        // Create directory
        assert!(ops.create_dir(&test_dir).is_ok());
        assert!(test_dir.exists());

        // Clean up
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_rename() {
        let ops = DefaultFileOperations::new();
        let from = PathBuf::from("test_rename_from.txt");
        let to = PathBuf::from("test_rename_to.txt");

        // Clean up
        let _ = fs::remove_file(&from);
        let _ = fs::remove_file(&to);

        // Create test file
        fs::write(&from, b"test").unwrap();

        // Rename
        assert!(ops.rename(&from, &to).is_ok());
        assert!(!from.exists());
        assert!(to.exists());

        // Clean up
        let _ = fs::remove_file(&to);
    }
}
