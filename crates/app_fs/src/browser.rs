//! File system browser - directory listing and file operations

use crate::{FsError, Result, UniversalPath};
use std::fs;
use std::path::Path;

/// File entry with metadata
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: UniversalPath,
    pub name: String,
    pub is_dir: bool,
    pub is_hidden: bool,
    pub size: u64,
    pub modified: Option<i64>,
    pub extension: String,
}

impl FileEntry {
    /// Create a new file entry from path
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let universal = UniversalPath::new(path);

        let metadata = fs::metadata(path)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let extension = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);

        let is_hidden = is_hidden_file(path, &name);

        Ok(Self {
            path: universal,
            name,
            is_dir: metadata.is_dir(),
            is_hidden,
            size: metadata.len(),
            modified,
            extension,
        })
    }

    /// Check if this is an image file
    pub fn is_image(&self) -> bool {
        matches!(
            self.extension.as_str(),
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "ico" | "tiff" | "tif"
        )
    }

    /// Check if this is an archive file
    pub fn is_archive(&self) -> bool {
        matches!(
            self.extension.as_str(),
            "zip" | "cbz" | "rar" | "cbr" | "7z" | "cb7" | "lzh" | "tar" | "gz" | "tgz"
        )
    }
}

/// Sort order for file listing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Name,
    Size,
    Modified,
    Extension,
}

/// Sort direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Options for listing directory contents
#[derive(Debug, Clone)]
pub struct ListOptions {
    pub show_hidden: bool,
    pub show_directories: bool,
    pub show_files: bool,
    pub sort_by: SortBy,
    pub sort_order: SortOrder,
    pub filter_extensions: Option<Vec<String>>,
}

impl Default for ListOptions {
    fn default() -> Self {
        Self {
            show_hidden: false,
            show_directories: true,
            show_files: true,
            sort_by: SortBy::Name,
            sort_order: SortOrder::Ascending,
            filter_extensions: None,
        }
    }
}

impl ListOptions {
    /// Filter for images only
    pub fn images_only() -> Self {
        Self {
            filter_extensions: Some(vec![
                "jpg".into(), "jpeg".into(), "png".into(), "gif".into(),
                "webp".into(), "bmp".into(), "ico".into(), "tiff".into(), "tif".into(),
            ]),
            ..Default::default()
        }
    }

    /// Filter for archives only
    pub fn archives_only() -> Self {
        Self {
            filter_extensions: Some(vec![
                "zip".into(), "cbz".into(), "rar".into(), "cbr".into(),
                "7z".into(), "cb7".into(), "lzh".into(),
            ]),
            ..Default::default()
        }
    }
}

/// List directory contents
pub fn list_directory<P: AsRef<Path>>(path: P, options: &ListOptions) -> Result<Vec<FileEntry>> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(FsError::NotFound(path.display().to_string()));
    }

    if !path.is_dir() {
        return Err(FsError::InvalidPath(format!("Not a directory: {}", path.display())));
    }

    let mut entries = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_entry = match FileEntry::from_path(entry.path()) {
            Ok(e) => e,
            Err(_) => continue, // Skip entries we can't read
        };

        // Apply filters
        if !options.show_hidden && file_entry.is_hidden {
            continue;
        }

        if !options.show_directories && file_entry.is_dir {
            continue;
        }

        if !options.show_files && !file_entry.is_dir {
            continue;
        }

        if let Some(ref exts) = options.filter_extensions {
            if !file_entry.is_dir && !exts.contains(&file_entry.extension) {
                continue;
            }
        }

        entries.push(file_entry);
    }

    // Sort entries
    sort_entries(&mut entries, options.sort_by, options.sort_order);

    Ok(entries)
}

/// Sort file entries
fn sort_entries(entries: &mut [FileEntry], sort_by: SortBy, order: SortOrder) {
    entries.sort_by(|a, b| {
        // Directories always come first
        if a.is_dir != b.is_dir {
            return if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }

        let cmp = match sort_by {
            SortBy::Name => natural_sort_key(&a.name).cmp(&natural_sort_key(&b.name)),
            SortBy::Size => a.size.cmp(&b.size),
            SortBy::Modified => a.modified.cmp(&b.modified),
            SortBy::Extension => {
                let ext_cmp = a.extension.cmp(&b.extension);
                if ext_cmp == std::cmp::Ordering::Equal {
                    natural_sort_key(&a.name).cmp(&natural_sort_key(&b.name))
                } else {
                    ext_cmp
                }
            }
        };

        match order {
            SortOrder::Ascending => cmp,
            SortOrder::Descending => cmp.reverse(),
        }
    });
}

/// Generate a natural sort key (handles numbers correctly)
/// "image2.jpg" < "image10.jpg"
fn natural_sort_key(s: &str) -> Vec<NaturalSortPart> {
    let mut parts = Vec::new();
    let mut current_num = String::new();
    let mut current_str = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            if !current_str.is_empty() {
                parts.push(NaturalSortPart::Str(current_str.to_lowercase()));
                current_str.clear();
            }
            current_num.push(c);
        } else {
            if !current_num.is_empty() {
                if let Ok(n) = current_num.parse::<u64>() {
                    parts.push(NaturalSortPart::Num(n));
                }
                current_num.clear();
            }
            current_str.push(c);
        }
    }

    // Handle remaining parts
    if !current_num.is_empty() {
        if let Ok(n) = current_num.parse::<u64>() {
            parts.push(NaturalSortPart::Num(n));
        }
    }
    if !current_str.is_empty() {
        parts.push(NaturalSortPart::Str(current_str.to_lowercase()));
    }

    parts
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum NaturalSortPart {
    Num(u64),
    Str(String),
}

/// Check if a file is hidden
#[cfg(windows)]
fn is_hidden_file(path: &Path, _name: &str) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;

    fs::metadata(path)
        .map(|m| m.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_hidden_file(_path: &Path, name: &str) -> bool {
    name.starts_with('.')
}

/// Get parent directory
pub fn get_parent<P: AsRef<Path>>(path: P) -> Option<UniversalPath> {
    path.as_ref().parent().map(UniversalPath::new)
}

/// Check if path is a root/drive
pub fn is_root<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();

    #[cfg(windows)]
    {
        // Windows: C:\ is root
        let s = path.to_string_lossy();
        s.len() <= 3 && s.ends_with('\\')
    }

    #[cfg(not(windows))]
    {
        path.parent().is_none()
    }
}

/// List available drives (Windows) or mount points
#[cfg(windows)]
pub fn list_drives() -> Vec<UniversalPath> {
    let mut drives = Vec::new();

    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let path = Path::new(&drive);
        if path.exists() {
            drives.push(UniversalPath::new(path));
        }
    }

    drives
}

#[cfg(not(windows))]
pub fn list_drives() -> Vec<UniversalPath> {
    vec![UniversalPath::new("/")]
}

/// Get sibling folders of a given folder
/// Returns (previous_sibling, next_sibling)
pub fn get_siblings<P: AsRef<Path>>(path: P, skip_empty: bool) -> (Option<UniversalPath>, Option<UniversalPath>) {
    let path = path.as_ref();

    let parent = match path.parent() {
        Some(p) => p,
        None => return (None, None),
    };

    let current_name = match path.file_name() {
        Some(n) => n.to_string_lossy().to_string(),
        None => return (None, None),
    };

    // List sibling directories
    let options = ListOptions {
        show_hidden: false,
        show_directories: true,
        show_files: false,
        sort_by: SortBy::Name,
        sort_order: SortOrder::Ascending,
        filter_extensions: None,
    };

    let siblings = match list_directory(parent, &options) {
        Ok(entries) => entries,
        Err(_) => return (None, None),
    };

    // Filter directories only and optionally skip empty ones
    let siblings: Vec<_> = siblings
        .into_iter()
        .filter(|e| {
            if !e.is_dir {
                return false;
            }
            if skip_empty {
                // Check if directory has any files/subdirs
                list_directory(e.path.as_path(), &ListOptions::default())
                    .map(|entries| !entries.is_empty())
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    // Find current folder index
    let current_idx = siblings.iter().position(|e| e.name == current_name);

    let prev = current_idx.and_then(|idx| {
        if idx > 0 {
            siblings.get(idx - 1).map(|e| e.path.clone())
        } else {
            None
        }
    });

    let next = current_idx.and_then(|idx| {
        siblings.get(idx + 1).map(|e| e.path.clone())
    });

    (prev, next)
}

/// Get the next sibling folder (nav.next_sibling)
pub fn get_next_sibling<P: AsRef<Path>>(path: P, skip_empty: bool) -> Option<UniversalPath> {
    get_siblings(path, skip_empty).1
}

/// Get the previous sibling folder (nav.prev_sibling)
pub fn get_prev_sibling<P: AsRef<Path>>(path: P, skip_empty: bool) -> Option<UniversalPath> {
    get_siblings(path, skip_empty).0
}

/// Count files in a directory (for nav.enter threshold check)
pub fn count_files<P: AsRef<Path>>(path: P) -> Result<usize> {
    let options = ListOptions {
        show_hidden: false,
        show_directories: false,
        show_files: true,
        sort_by: SortBy::Name,
        sort_order: SortOrder::Ascending,
        filter_extensions: None,
    };

    list_directory(path, &options).map(|entries| entries.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_sort() {
        let mut names = vec!["image10.jpg", "image2.jpg", "image1.jpg", "image20.jpg"];
        names.sort_by(|a, b| natural_sort_key(a).cmp(&natural_sort_key(b)));
        assert_eq!(names, vec!["image1.jpg", "image2.jpg", "image10.jpg", "image20.jpg"]);
    }
}
