//! UniversalPath - Safe path handling for Windows with UNC prefix support

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

/// A path wrapper that handles Windows path limitations
///
/// Features:
/// - Automatic UNC prefix (\\?\) for long path support
/// - Lossy UTF-8 display string for UI
/// - Hash-based ID for database lookups
/// - Raw bytes preservation for non-UTF8 paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalPath {
    /// Raw path for file system operations (with UNC prefix on Windows)
    #[serde(skip)]
    raw: PathBuf,

    /// UTF-8 display string (lossy conversion for UI)
    display: String,

    /// Hash-based ID for fast lookups
    id: u64,

    /// Raw bytes for database storage (preserves non-UTF8 characters)
    #[serde(with = "serde_bytes")]
    raw_bytes: Vec<u8>,
}

mod serde_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        bytes.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Vec::<u8>::deserialize(deserializer)
    }
}

impl UniversalPath {
    /// Create a new UniversalPath from any path-like type
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();

        // Normalize and add UNC prefix on Windows
        let raw = Self::normalize_path(path);

        // Create display string (lossy UTF-8)
        let display = raw.to_string_lossy().to_string();

        // Calculate hash for DB lookups
        let id = xxh3_64(display.as_bytes());

        // Store raw bytes for perfect reconstruction
        let raw_bytes = Self::path_to_bytes(&raw);

        Self {
            raw,
            display,
            id,
            raw_bytes,
        }
    }

    /// Reconstruct from database storage
    pub fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        let path = Self::bytes_to_path(bytes)?;
        Some(Self::new(path))
    }

    /// Get the raw PathBuf for file system operations
    pub fn as_path(&self) -> &Path {
        &self.raw
    }

    /// Get the raw PathBuf (owned)
    pub fn to_path_buf(&self) -> PathBuf {
        self.raw.clone()
    }

    /// Get the display string for UI
    pub fn display(&self) -> &str {
        &self.display
    }

    /// Get the hash ID for database lookups
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get raw bytes for database storage
    pub fn as_raw_bytes(&self) -> &[u8] {
        &self.raw_bytes
    }

    /// Get parent directory
    pub fn parent(&self) -> Option<Self> {
        self.raw.parent().map(Self::new)
    }

    /// Get file name
    pub fn file_name(&self) -> Option<&str> {
        self.raw.file_name()?.to_str()
    }

    /// Get file extension
    pub fn extension(&self) -> Option<&str> {
        self.raw.extension()?.to_str()
    }

    /// Check if path exists
    pub fn exists(&self) -> bool {
        self.raw.exists()
    }

    /// Check if path is a directory
    pub fn is_dir(&self) -> bool {
        self.raw.is_dir()
    }

    /// Check if path is a file
    pub fn is_file(&self) -> bool {
        self.raw.is_file()
    }

    /// Join with another path component
    pub fn join<P: AsRef<Path>>(&self, path: P) -> Self {
        Self::new(self.raw.join(path))
    }

    /// Normalize path and add UNC prefix on Windows
    #[cfg(windows)]
    fn normalize_path(path: &Path) -> PathBuf {
        use std::path::Component;

        // Convert to absolute path
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_default()
                .join(path)
        };

        // Normalize components (resolve . and ..)
        let mut normalized = PathBuf::new();
        for component in absolute.components() {
            match component {
                Component::ParentDir => {
                    normalized.pop();
                }
                Component::CurDir => {}
                _ => normalized.push(component),
            }
        }

        // Add UNC prefix if not present
        let path_str = normalized.to_string_lossy();
        if !path_str.starts_with(r"\\?\") && !path_str.starts_with(r"\\.\") {
            PathBuf::from(format!(r"\\?\{}", path_str))
        } else {
            normalized
        }
    }

    #[cfg(not(windows))]
    fn normalize_path(path: &Path) -> PathBuf {
        // On non-Windows, just canonicalize
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    /// Convert PathBuf to raw bytes for storage
    #[cfg(windows)]
    fn path_to_bytes(path: &Path) -> Vec<u8> {
        use std::os::windows::ffi::OsStrExt;

        path.as_os_str()
            .encode_wide()
            .flat_map(|c| c.to_le_bytes())
            .collect()
    }

    #[cfg(not(windows))]
    fn path_to_bytes(path: &Path) -> Vec<u8> {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }

    /// Convert raw bytes back to PathBuf
    #[cfg(windows)]
    fn bytes_to_path(bytes: &[u8]) -> Option<PathBuf> {
        use std::os::windows::ffi::OsStringExt;

        if bytes.len() % 2 != 0 {
            return None;
        }

        let wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        Some(PathBuf::from(OsString::from_wide(&wide)))
    }

    #[cfg(not(windows))]
    fn bytes_to_path(bytes: &[u8]) -> Option<PathBuf> {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(OsString::from_vec(bytes.to_vec())))
    }
}

impl AsRef<Path> for UniversalPath {
    fn as_ref(&self) -> &Path {
        &self.raw
    }
}

impl From<PathBuf> for UniversalPath {
    fn from(path: PathBuf) -> Self {
        Self::new(path)
    }
}

impl From<&Path> for UniversalPath {
    fn from(path: &Path) -> Self {
        Self::new(path)
    }
}

impl From<String> for UniversalPath {
    fn from(path: String) -> Self {
        Self::new(path)
    }
}

impl From<&str> for UniversalPath {
    fn from(path: &str) -> Self {
        Self::new(path)
    }
}

impl std::fmt::Display for UniversalPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display without UNC prefix for readability
        let display = self.display.strip_prefix(r"\\?\").unwrap_or(&self.display);
        write!(f, "{}", display)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        let path1 = UniversalPath::new("C:\\Users\\test\\image.jpg");
        let path2 = UniversalPath::new("C:\\Users\\test\\image.jpg");
        assert_eq!(path1.id(), path2.id());
    }

    #[test]
    fn test_display() {
        let path = UniversalPath::new("C:\\Users\\test\\image.jpg");
        assert!(path.display().contains("image.jpg"));
    }
}
