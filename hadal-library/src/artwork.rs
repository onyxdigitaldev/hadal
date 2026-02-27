//! Album artwork extraction and caching.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use image::DynamicImage;
use lofty::TaggedFileExt;

use crate::error::LibraryResult;

/// Common artwork filenames to look for in album directories.
const ARTWORK_FILENAMES: &[&str] = &[
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
    "album.jpg",
    "album.jpeg",
    "album.png",
];

/// Album artwork manager.
pub struct ArtworkManager {
    /// Cache directory for thumbnails
    cache_dir: PathBuf,

    /// Thumbnail size
    thumbnail_size: u32,
}

impl ArtworkManager {
    /// Create a new artwork manager.
    pub fn new(cache_dir: PathBuf, thumbnail_size: u32) -> Self {
        Self {
            cache_dir,
            thumbnail_size,
        }
    }

    /// Get artwork for a track, checking cache first.
    pub fn get_artwork(&self, track_path: &Path) -> LibraryResult<Option<PathBuf>> {
        // Generate cache key
        let cache_key = self.compute_cache_key(track_path);
        let cache_path = self.cache_dir.join(format!("{:016x}.png", cache_key));

        // Check cache
        if cache_path.exists() {
            return Ok(Some(cache_path));
        }

        // Try to extract artwork
        if let Some(image) = self.extract_artwork(track_path)? {
            // Generate thumbnail and save to cache
            let thumbnail = self.create_thumbnail(&image);
            std::fs::create_dir_all(&self.cache_dir)?;
            thumbnail.save(&cache_path)?;

            return Ok(Some(cache_path));
        }

        Ok(None)
    }

    /// Extract artwork from a track or its directory.
    pub fn extract_artwork(&self, track_path: &Path) -> LibraryResult<Option<DynamicImage>> {
        // Try embedded artwork first
        if let Some(image) = self.extract_embedded(track_path)? {
            return Ok(Some(image));
        }

        // Try folder artwork
        if let Some(image) = self.find_folder_artwork(track_path)? {
            return Ok(Some(image));
        }

        Ok(None)
    }

    /// Extract embedded artwork from a file.
    fn extract_embedded(&self, path: &Path) -> LibraryResult<Option<DynamicImage>> {
        // Use lofty to extract embedded artwork
        let tagged_file = match lofty::read_from_path(path) {
            Ok(f) => f,
            Err(e) => {
                tracing::debug!("Failed to read metadata for artwork from {}: {}", path.display(), e);
                return Ok(None);
            }
        };

        // Check primary tag first, then first available
        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

        if let Some(tag) = tag {
            for picture in tag.pictures() {
                if let Ok(image) = image::load_from_memory(picture.data()) {
                    return Ok(Some(image));
                }
            }
        }

        Ok(None)
    }

    /// Find artwork in the track's directory.
    fn find_folder_artwork(&self, track_path: &Path) -> LibraryResult<Option<DynamicImage>> {
        let dir = match track_path.parent() {
            Some(d) => d,
            None => return Ok(None),
        };

        // Read directory entries once for case-insensitive matching
        let entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => return Ok(None),
        };

        // First pass: check preferred filenames (case-insensitive)
        for filename in ARTWORK_FILENAMES {
            for entry in &entries {
                if entry.file_name().to_string_lossy().eq_ignore_ascii_case(filename) {
                    if let Ok(image) = image::open(entry.path()) {
                        return Ok(Some(image));
                    }
                }
            }
        }

        // Second pass: any image file in the directory (prefer larger files)
        let mut image_files: Vec<_> = entries.iter()
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                (name.ends_with(".jpg") || name.ends_with(".jpeg") || name.ends_with(".png"))
                    && !name.contains("mini")
                    && !name.contains("back")
            })
            .collect();
        image_files.sort_by(|a, b| {
            let sa = a.metadata().map(|m| m.len()).unwrap_or(0);
            let sb = b.metadata().map(|m| m.len()).unwrap_or(0);
            sb.cmp(&sa) // largest first
        });
        if let Some(entry) = image_files.first() {
            if let Ok(image) = image::open(entry.path()) {
                return Ok(Some(image));
            }
        }

        Ok(None)
    }

    /// Create a thumbnail from an image.
    fn create_thumbnail(&self, image: &DynamicImage) -> DynamicImage {
        image.resize(
            self.thumbnail_size,
            self.thumbnail_size,
            image::imageops::FilterType::Lanczos3,
        )
    }

    /// Compute a cache key for a track.
    fn compute_cache_key(&self, track_path: &Path) -> u64 {
        let mut hasher = DefaultHasher::new();

        // Hash the directory path (artwork is typically per-album)
        if let Some(dir) = track_path.parent() {
            dir.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Clear the artwork cache.
    pub fn clear_cache(&self) -> LibraryResult<usize> {
        let mut count = 0;

        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                if entry.path().extension().map(|e| e == "png").unwrap_or(false) {
                    std::fs::remove_file(entry.path())?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> LibraryResult<CacheStats> {
        let mut stats = CacheStats::default();

        if self.cache_dir.exists() {
            for entry in std::fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                stats.file_count += 1;
                stats.total_bytes += metadata.len();
            }
        }

        Ok(stats)
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub file_count: usize,
    pub total_bytes: u64,
}

impl CacheStats {
    /// Get total size in megabytes.
    pub fn size_mb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0)
    }
}

/// Extract artwork bytes from a file without loading as image.
pub fn extract_artwork_bytes(path: &Path) -> LibraryResult<Option<Vec<u8>>> {
    let tagged_file = match lofty::read_from_path(path) {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };

    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    if let Some(tag) = tag {
        if let Some(picture) = tag.pictures().first() {
            return Ok(Some(picture.data().to_vec()));
        }
    }

    Ok(None)
}
