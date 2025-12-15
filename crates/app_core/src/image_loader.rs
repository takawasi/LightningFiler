//! Image loading and decoding service

use crate::AppError;
use crate::resource::ImageFormat;
use app_fs::UniversalPath;
use image::{GenericImageView, ImageReader};
use rayon::prelude::*;
use std::io::Cursor;
use std::path::Path;
use tokio::sync::mpsc;
use xxhash_rust::xxh3::xxh3_64;

/// Image loader service
pub struct ImageLoader {
    /// Channel for load requests
    request_tx: mpsc::UnboundedSender<LoadRequest>,
}

/// Load request
#[derive(Debug)]
struct LoadRequest {
    path: UniversalPath,
    target_size: Option<(u32, u32)>,
    callback: tokio::sync::oneshot::Sender<Result<LoadedImage, AppError>>,
}

/// Loaded image result
#[derive(Debug, Clone)]
pub struct LoadedImage {
    pub path: UniversalPath,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub format: ImageFormat,
    pub hash: u64,
}

impl ImageLoader {
    /// Create a new image loader
    pub fn new() -> Self {
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<LoadRequest>();

        // Spawn worker thread
        std::thread::spawn(move || {
            while let Some(request) = request_rx.blocking_recv() {
                let result = Self::load_image_sync(&request.path, request.target_size);
                let _ = request.callback.send(result);
            }
        });

        Self { request_tx }
    }

    /// Load an image asynchronously
    pub async fn load(&self, path: UniversalPath, target_size: Option<(u32, u32)>) -> Result<LoadedImage, AppError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.request_tx.send(LoadRequest {
            path,
            target_size,
            callback: tx,
        }).map_err(|_| AppError::SystemResource("Image loader channel closed".into()))?;

        rx.await.map_err(|_| AppError::SystemResource("Image loader response failed".into()))?
    }

    /// Load image synchronously (called from worker thread)
    fn load_image_sync(path: &UniversalPath, target_size: Option<(u32, u32)>) -> Result<LoadedImage, AppError> {
        tracing::debug!("Loading image: {}", path);

        // Read file
        let data = std::fs::read(path.as_path())?;
        let hash = xxh3_64(&data);

        // Decode image
        let reader = ImageReader::new(Cursor::new(&data))
            .with_guessed_format()
            .map_err(|e| AppError::ImageDecode(e.to_string()))?;

        let img = reader.decode()
            .map_err(|e| AppError::ImageDecode(e.to_string()))?;

        // Resize if needed
        let img = if let Some((max_w, max_h)) = target_size {
            let (w, h) = img.dimensions();
            if w > max_w || h > max_h {
                img.thumbnail(max_w, max_h)
            } else {
                img
            }
        } else {
            img
        };

        // Convert to RGBA8
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        Ok(LoadedImage {
            path: path.clone(),
            width,
            height,
            data: rgba.into_raw(),
            format: ImageFormat::Rgba8,
            hash,
        })
    }
}

impl Default for ImageLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Thumbnail generator
pub struct ThumbnailGenerator {
    size: u32,
}

impl ThumbnailGenerator {
    pub fn new(size: u32) -> Self {
        Self { size }
    }

    /// Generate thumbnail for an image file
    pub fn generate(&self, path: &Path) -> Result<LoadedImage, AppError> {
        let data = std::fs::read(path)?;
        let hash = xxh3_64(&data);

        let reader = ImageReader::new(Cursor::new(&data))
            .with_guessed_format()
            .map_err(|e| AppError::ImageDecode(e.to_string()))?;

        let img = reader.decode()
            .map_err(|e| AppError::ImageDecode(e.to_string()))?;

        // Generate thumbnail
        let thumb = img.thumbnail(self.size, self.size);
        let rgba = thumb.to_rgba8();
        let (width, height) = rgba.dimensions();

        Ok(LoadedImage {
            path: UniversalPath::new(path),
            width,
            height,
            data: rgba.into_raw(),
            format: ImageFormat::Rgba8,
            hash,
        })
    }

    /// Generate thumbnails for multiple files in parallel
    pub fn generate_batch(&self, paths: &[&Path]) -> Vec<Result<LoadedImage, AppError>> {
        paths.par_iter()
            .map(|path| self.generate(path))
            .collect()
    }
}

/// Get image dimensions without fully decoding
pub fn get_image_dimensions(path: &Path) -> Result<(u32, u32), AppError> {
    let reader = ImageReader::open(path)
        .map_err(|e| AppError::ImageDecode(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| AppError::ImageDecode(e.to_string()))?;

    reader.into_dimensions()
        .map_err(|e| AppError::ImageDecode(e.to_string()))
}

/// Check if a file is a supported image format
pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "ico" | "tiff" | "tif"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_image() {
        assert!(is_supported_image(Path::new("test.jpg")));
        assert!(is_supported_image(Path::new("test.PNG")));
        assert!(is_supported_image(Path::new("test.WebP")));
        assert!(!is_supported_image(Path::new("test.txt")));
        assert!(!is_supported_image(Path::new("test.mp4")));
    }
}
