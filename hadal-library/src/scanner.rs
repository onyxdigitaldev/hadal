//! Directory scanner for audio files.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::error::LibraryResult;

/// Supported audio file extensions.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "flac", "mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "wave", "aif", "aiff", "wv",
];

/// Directory scanner for finding audio files.
pub struct Scanner {
    /// Folders to scan
    folders: Vec<PathBuf>,

    /// Whether scanning is in progress
    scanning: Arc<AtomicBool>,

    /// Number of files found
    files_found: Arc<AtomicUsize>,

    /// Cancel flag
    cancelled: Arc<AtomicBool>,
}

impl Scanner {
    /// Create a new scanner for the given folders.
    pub fn new(folders: Vec<PathBuf>) -> Self {
        Self {
            folders,
            scanning: Arc::new(AtomicBool::new(false)),
            files_found: Arc::new(AtomicUsize::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if scanning is in progress.
    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::Relaxed)
    }

    /// Get the number of files found so far.
    pub fn files_found(&self) -> usize {
        self.files_found.load(Ordering::Relaxed)
    }

    /// Cancel the current scan.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Scan all folders and return found audio files.
    pub fn scan_sync(&self) -> LibraryResult<Vec<PathBuf>> {
        self.scanning.store(true, Ordering::Relaxed);
        self.files_found.store(0, Ordering::Relaxed);
        self.cancelled.store(false, Ordering::Relaxed);

        let mut files = Vec::new();

        for folder in &self.folders {
            if self.cancelled.load(Ordering::Relaxed) {
                break;
            }

            self.scan_directory(folder, &mut files)?;
        }

        self.scanning.store(false, Ordering::Relaxed);

        tracing::info!("Scan complete: {} audio files found", files.len());

        Ok(files)
    }

    /// Scan all folders asynchronously, sending files through a channel.
    pub async fn scan_async(
        &self,
        tx: mpsc::Sender<PathBuf>,
    ) -> LibraryResult<usize> {
        self.scanning.store(true, Ordering::Relaxed);
        self.files_found.store(0, Ordering::Relaxed);
        self.cancelled.store(false, Ordering::Relaxed);

        let mut count = 0;

        for folder in &self.folders {
            if self.cancelled.load(Ordering::Relaxed) {
                break;
            }

            count += self.scan_directory_async(folder, &tx).await?;
        }

        self.scanning.store(false, Ordering::Relaxed);

        tracing::info!("Async scan complete: {} audio files found", count);

        Ok(count)
    }

    /// Recursively scan a directory.
    fn scan_directory(&self, dir: &Path, files: &mut Vec<PathBuf>) -> LibraryResult<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                tracing::warn!("Failed to read directory {}: {}", dir.display(), e);
                return Ok(());
            }
        };

        for entry in entries {
            if self.cancelled.load(Ordering::Relaxed) {
                return Ok(());
            }

            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("Failed to read entry in {}: {}", dir.display(), e);
                    continue;
                }
            };

            let path = entry.path();

            if path.is_dir() {
                // Recurse into subdirectory
                self.scan_directory(&path, files)?;
            } else if Self::is_audio_file(&path) {
                files.push(path);
                self.files_found.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    /// Recursively scan a directory asynchronously.
    async fn scan_directory_async(
        &self,
        dir: &Path,
        tx: &mpsc::Sender<PathBuf>,
    ) -> LibraryResult<usize> {
        if !dir.is_dir() {
            return Ok(0);
        }

        let mut count = 0;

        let mut read_dir = match tokio::fs::read_dir(dir).await {
            Ok(rd) => rd,
            Err(e) => {
                tracing::warn!("Failed to read directory {}: {}", dir.display(), e);
                return Ok(0);
            }
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if self.cancelled.load(Ordering::Relaxed) {
                break;
            }

            let path = entry.path();

            if path.is_dir() {
                count += Box::pin(self.scan_directory_async(&path, tx)).await?;
            } else if Self::is_audio_file(&path)
                && tx.send(path).await.is_ok()
            {
                count += 1;
                self.files_found.fetch_add(1, Ordering::Relaxed);
            }
        }

        Ok(count)
    }

    /// Check if a file is a supported audio file.
    pub fn is_audio_file(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| {
                AUDIO_EXTENSIONS
                    .iter()
                    .any(|&supported| supported.eq_ignore_ascii_case(ext))
            })
            .unwrap_or(false)
    }

    /// Get the total size of all files in the scan list.
    pub fn total_size(files: &[PathBuf]) -> u64 {
        files
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum()
    }
}

impl Default for Scanner {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_audio_file() {
        assert!(Scanner::is_audio_file(Path::new("song.flac")));
        assert!(Scanner::is_audio_file(Path::new("song.FLAC")));
        assert!(Scanner::is_audio_file(Path::new("song.mp3")));
        assert!(Scanner::is_audio_file(Path::new("song.m4a")));
        assert!(Scanner::is_audio_file(Path::new("song.ogg")));
        assert!(Scanner::is_audio_file(Path::new("song.opus")));
        assert!(Scanner::is_audio_file(Path::new("song.wav")));
        assert!(Scanner::is_audio_file(Path::new("song.aiff")));

        assert!(!Scanner::is_audio_file(Path::new("song.txt")));
        assert!(!Scanner::is_audio_file(Path::new("song.jpg")));
        assert!(!Scanner::is_audio_file(Path::new("song")));
    }
}
