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

    // ========================================
    // Doc 2 spec: Japanese/Emoji path tests
    // ========================================

    // Windows-only tests (path separator handling)
    #[cfg(windows)]
    #[test]
    fn test_japanese_path() {
        // Japanese characters in path (日本語)
        let path = UniversalPath::new("C:\\Users\\テスト\\画像\\写真.jpg");
        assert!(path.display().contains("写真.jpg"));
        assert_eq!(path.file_name(), Some("写真.jpg"));
        assert_eq!(path.extension(), Some("jpg"));
    }

    #[test]
    fn test_japanese_hash_consistency() {
        // Same Japanese path should produce same hash
        let path1 = UniversalPath::new("C:\\ユーザー\\日本語フォルダ\\ファイル.png");
        let path2 = UniversalPath::new("C:\\ユーザー\\日本語フォルダ\\ファイル.png");
        assert_eq!(path1.id(), path2.id());
    }

    #[cfg(windows)]
    #[test]
    fn test_emoji_path() {
        // Emoji in path (絵文字)
        let path = UniversalPath::new("C:\\Users\\🎨Art\\📷Photos\\🌸sakura.jpg");
        assert!(path.display().contains("🌸sakura.jpg"));
        assert_eq!(path.file_name(), Some("🌸sakura.jpg"));
    }

    #[test]
    fn test_mixed_unicode_path() {
        // Mix of Japanese, emoji, and ASCII
        let path = UniversalPath::new("C:\\Users\\田中\\🎮ゲーム\\screenshot_2024.png");
        assert!(path.display().contains("田中"));
        assert!(path.display().contains("🎮ゲーム"));
    }

    #[cfg(windows)]
    #[test]
    fn test_space_in_path() {
        // Spaces in path (Doc 2: must handle spaces correctly)
        let path = UniversalPath::new("C:\\Users\\My Documents\\My Pictures\\photo 001.jpg");
        assert!(path.display().contains("My Documents"));
        assert!(path.display().contains("photo 001.jpg"));
        assert_eq!(path.file_name(), Some("photo 001.jpg"));
    }

    #[cfg(windows)]
    #[test]
    fn test_space_and_japanese_combined() {
        // Spaces with Japanese
        let path = UniversalPath::new("C:\\Users\\山田 太郎\\書類 フォルダ\\メモ 2024.txt");
        assert!(path.display().contains("山田 太郎"));
        assert_eq!(path.file_name(), Some("メモ 2024.txt"));
    }

    #[test]
    fn test_raw_bytes_roundtrip_japanese() {
        // Raw bytes should preserve Japanese characters perfectly
        let original = UniversalPath::new("C:\\日本語\\テスト\\ファイル名.txt");
        let bytes = original.as_raw_bytes();
        let reconstructed = UniversalPath::from_raw_bytes(bytes).expect("Should reconstruct");
        assert_eq!(original.display(), reconstructed.display());
        assert_eq!(original.id(), reconstructed.id());
    }

    #[test]
    fn test_raw_bytes_roundtrip_emoji() {
        // Raw bytes should preserve emoji perfectly
        let original = UniversalPath::new("C:\\🎮\\🎨\\🌸.png");
        let bytes = original.as_raw_bytes();
        let reconstructed = UniversalPath::from_raw_bytes(bytes).expect("Should reconstruct");
        assert_eq!(original.display(), reconstructed.display());
    }

    #[cfg(windows)]
    #[test]
    fn test_unc_prefix_added() {
        // Doc 2 spec: \\?\ prefix must be added for long path support
        let path = UniversalPath::new("C:\\Users\\test\\image.jpg");
        assert!(path.display().starts_with(r"\\?\"));
    }

    #[cfg(windows)]
    #[test]
    fn test_unc_prefix_not_duplicated() {
        // Should not duplicate \\?\ prefix
        let path = UniversalPath::new(r"\\?\C:\Users\test\image.jpg");
        let display = path.display();
        // Count occurrences of \\?\
        let count = display.matches(r"\\?\").count();
        assert_eq!(count, 1, "UNC prefix should appear exactly once");
    }

    #[cfg(windows)]
    #[test]
    fn test_unc_japanese_path() {
        // Japanese path with UNC prefix
        let path = UniversalPath::new("C:\\ユーザー\\写真\\桜.jpg");
        assert!(path.display().starts_with(r"\\?\"));
        assert!(path.display().contains("桜.jpg"));
    }

    #[cfg(windows)]
    #[test]
    fn test_parent_japanese() {
        let path = UniversalPath::new("C:\\親フォルダ\\子フォルダ\\ファイル.txt");
        let parent = path.parent().expect("Should have parent");
        assert!(parent.display().contains("子フォルダ"));
    }

    #[test]
    fn test_join_japanese() {
        let parent = UniversalPath::new("C:\\ベース");
        let child = parent.join("新しいファイル.txt");
        assert!(child.display().contains("新しいファイル.txt"));
    }

    #[test]
    fn test_long_japanese_filename() {
        // Test long Japanese filename (stress test)
        let long_name = "これは非常に長い日本語のファイル名でテストしています_".repeat(5);
        let path = UniversalPath::new(format!("C:\\{}.txt", long_name));
        assert!(path.display().contains(&long_name));
    }

    // Linux/Unix path tests
    #[cfg(not(windows))]
    #[test]
    fn test_unix_japanese_path() {
        let path = UniversalPath::new("/home/ユーザー/画像/写真.jpg");
        assert!(path.display().contains("写真.jpg"));
        assert_eq!(path.file_name(), Some("写真.jpg"));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_unix_emoji_path() {
        let path = UniversalPath::new("/home/user/🎨Art/📷Photos/🌸sakura.jpg");
        assert!(path.display().contains("🌸sakura.jpg"));
        assert_eq!(path.file_name(), Some("🌸sakura.jpg"));
    }

    #[cfg(not(windows))]
    #[test]
    fn test_unix_space_in_path() {
        let path = UniversalPath::new("/home/user/My Documents/photo 001.jpg");
        assert!(path.display().contains("My Documents"));
        assert_eq!(path.file_name(), Some("photo 001.jpg"));
    }
}
