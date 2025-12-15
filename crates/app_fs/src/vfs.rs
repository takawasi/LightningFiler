//! Virtual File System for transparent archive handling

use crate::{FsError, Result, UniversalPath, encoding};
use serde::{Deserialize, Serialize};
use std::io::Read;

/// Error type for VFS operations
#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("Archive not found: {0}")]
    ArchiveNotFound(String),

    #[error("Entry not found in archive: {0}")]
    EntryNotFound(String),

    #[error("Unsupported archive format: {0}")]
    UnsupportedFormat(String),

    #[error("Archive corrupted: {0}")]
    Corrupted(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Entry in a virtual file system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsEntry {
    /// Display name (UTF-8, potentially lossy)
    pub name: String,

    /// Full path within the archive
    pub path: String,

    /// File size in bytes
    pub size: u64,

    /// Compressed size (if applicable)
    pub compressed_size: Option<u64>,

    /// Is this a directory?
    pub is_dir: bool,

    /// Last modified timestamp (Unix epoch)
    pub modified: Option<i64>,
}

/// Virtual File System abstraction
pub struct VirtualFileSystem {
    /// Archive path
    archive_path: UniversalPath,

    /// Archive format
    format: ArchiveFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZip,
    Tar,
    TarGz,
    TarBz2,
    /// Use Susie Bridge for this format
    Susie,
}

impl VirtualFileSystem {
    /// Open an archive file
    pub fn open<P: Into<UniversalPath>>(path: P) -> Result<Self> {
        let path = path.into();

        let format = Self::detect_format(&path)?;

        Ok(Self {
            archive_path: path,
            format,
        })
    }

    /// Detect archive format from extension
    fn detect_format(path: &UniversalPath) -> Result<ArchiveFormat> {
        let ext = path
            .extension()
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "zip" | "cbz" | "epub" => Ok(ArchiveFormat::Zip),
            "7z" | "cb7" => Ok(ArchiveFormat::SevenZip),
            "tar" => Ok(ArchiveFormat::Tar),
            "gz" | "tgz" => Ok(ArchiveFormat::TarGz),
            "bz2" | "tbz" | "tbz2" => Ok(ArchiveFormat::TarBz2),
            "rar" | "cbr" | "lzh" | "lha" => Ok(ArchiveFormat::Susie),
            _ => Err(FsError::Archive(format!("Unknown archive format: {}", ext))),
        }
    }

    /// List all entries in the archive
    pub fn list_entries(&self) -> Result<Vec<VfsEntry>> {
        match self.format {
            ArchiveFormat::Zip => self.list_zip_entries(),
            ArchiveFormat::SevenZip => self.list_7z_entries(),
            ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarBz2 => {
                self.list_tar_entries()
            }
            ArchiveFormat::Susie => {
                Err(FsError::Archive("Susie archives require Bridge process".into()))
            }
        }
    }

    /// Read a file from the archive
    pub fn read_file(&self, inner_path: &str) -> Result<Vec<u8>> {
        match self.format {
            ArchiveFormat::Zip => self.read_zip_file(inner_path),
            ArchiveFormat::SevenZip => self.read_7z_file(inner_path),
            ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarBz2 => {
                self.read_tar_file(inner_path)
            }
            ArchiveFormat::Susie => {
                Err(FsError::Archive("Susie archives require Bridge process".into()))
            }
        }
    }

    // ZIP implementation
    fn list_zip_entries(&self) -> Result<Vec<VfsEntry>> {
        let file = std::fs::File::open(self.archive_path.as_path())?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| FsError::Archive(e.to_string()))?;

        let hint = encoding::system_encoding_hint();
        let mut entries = Vec::with_capacity(archive.len());

        for i in 0..archive.len() {
            let file = archive.by_index_raw(i)
                .map_err(|e| FsError::Archive(e.to_string()))?;

            // Handle filename encoding
            // Try to decode as UTF-8 first, fallback to system encoding
            let raw_name = file.name_raw();
            let name = match std::str::from_utf8(raw_name) {
                Ok(s) => s.to_string(),
                Err(_) => {
                    // Try to decode non-UTF8 filename
                    let (decoded, _) = encoding::decode_bytes(raw_name, hint);
                    decoded
                }
            };

            entries.push(VfsEntry {
                name: name.rsplit('/').next().unwrap_or(&name).to_string(),
                path: name,
                size: file.size(),
                compressed_size: Some(file.compressed_size()),
                is_dir: file.is_dir(),
                modified: file.last_modified().map(|dt| {
                    // Convert to Unix timestamp (approximate)
                    let year = dt.year() as i64;
                    let month = dt.month() as i64;
                    let day = dt.day() as i64;
                    let hour = dt.hour() as i64;
                    let minute = dt.minute() as i64;
                    let second = dt.second() as i64;

                    // Rough calculation (ignoring leap years, etc.)
                    ((year - 1970) * 365 * 24 * 3600)
                        + (month * 30 * 24 * 3600)
                        + (day * 24 * 3600)
                        + (hour * 3600)
                        + (minute * 60)
                        + second
                }),
            });
        }

        Ok(entries)
    }

    fn read_zip_file(&self, inner_path: &str) -> Result<Vec<u8>> {
        let file = std::fs::File::open(self.archive_path.as_path())?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| FsError::Archive(e.to_string()))?;

        let mut zip_file = archive.by_name(inner_path)
            .map_err(|e| FsError::Archive(e.to_string()))?;

        let mut buffer = Vec::with_capacity(zip_file.size() as usize);
        zip_file.read_to_end(&mut buffer)?;

        Ok(buffer)
    }

    // 7z implementation
    fn list_7z_entries(&self) -> Result<Vec<VfsEntry>> {
        let mut entries = Vec::new();

        sevenz_rust::decompress_file_with_extract_fn(
            self.archive_path.as_path(),
            std::path::Path::new(""),
            |entry, _, _| {
                entries.push(VfsEntry {
                    name: entry.name().rsplit('/').next().unwrap_or(entry.name()).to_string(),
                    path: entry.name().to_string(),
                    size: entry.size(),
                    compressed_size: Some(entry.compressed_size),
                    is_dir: entry.is_directory(),
                    modified: None, // 7z-rust doesn't expose timestamps easily
                });
                Ok(false) // Don't actually extract
            },
        ).map_err(|e| FsError::Archive(e.to_string()))?;

        Ok(entries)
    }

    fn read_7z_file(&self, inner_path: &str) -> Result<Vec<u8>> {
        let mut result: Option<Vec<u8>> = None;

        sevenz_rust::decompress_file_with_extract_fn(
            self.archive_path.as_path(),
            std::path::Path::new(""),
            |entry, reader, _| {
                if entry.name() == inner_path {
                    let mut buffer = Vec::new();
                    reader.read_to_end(&mut buffer)?;
                    result = Some(buffer);
                    Ok(false) // Stop extraction
                } else {
                    Ok(true) // Continue
                }
            },
        ).map_err(|e| FsError::Archive(e.to_string()))?;

        result.ok_or_else(|| FsError::Archive(format!("File not found: {}", inner_path)))
    }

    // TAR implementation (with optional compression)
    fn list_tar_entries(&self) -> Result<Vec<VfsEntry>> {
        let file = std::fs::File::open(self.archive_path.as_path())?;
        let reader: Box<dyn Read> = match self.format {
            ArchiveFormat::TarGz => Box::new(flate2::read::GzDecoder::new(file)),
            ArchiveFormat::TarBz2 => {
                // bzip2 would need another crate
                return Err(FsError::Archive("bzip2 not yet supported".into()));
            }
            _ => Box::new(file),
        };

        let mut archive = tar::Archive::new(reader);
        let mut entries = Vec::new();

        for entry in archive.entries()? {
            let entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy().to_string();

            entries.push(VfsEntry {
                name: path.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| path_str.clone()),
                path: path_str,
                size: entry.size(),
                compressed_size: None,
                is_dir: entry.header().entry_type().is_dir(),
                modified: entry.header().mtime().ok().map(|t| t as i64),
            });
        }

        Ok(entries)
    }

    fn read_tar_file(&self, inner_path: &str) -> Result<Vec<u8>> {
        let file = std::fs::File::open(self.archive_path.as_path())?;
        let reader: Box<dyn Read> = match self.format {
            ArchiveFormat::TarGz => Box::new(flate2::read::GzDecoder::new(file)),
            _ => Box::new(file),
        };

        let mut archive = tar::Archive::new(reader);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            if path.to_string_lossy() == inner_path {
                let mut buffer = Vec::with_capacity(entry.size() as usize);
                entry.read_to_end(&mut buffer)?;
                return Ok(buffer);
            }
        }

        Err(FsError::Archive(format!("File not found: {}", inner_path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_detection() {
        let path = UniversalPath::new("test.zip");
        let format = VirtualFileSystem::detect_format(&path).unwrap();
        assert_eq!(format, ArchiveFormat::Zip);

        let path = UniversalPath::new("test.cbz");
        let format = VirtualFileSystem::detect_format(&path).unwrap();
        assert_eq!(format, ArchiveFormat::Zip);

        let path = UniversalPath::new("test.7z");
        let format = VirtualFileSystem::detect_format(&path).unwrap();
        assert_eq!(format, ArchiveFormat::SevenZip);
    }
}
