//! M3U8 playlist format support.

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{PlaylistError, PlaylistResult};

/// An entry in an M3U8 playlist.
#[derive(Debug, Clone)]
pub struct M3u8Entry {
    /// Path to the audio file (may be relative or absolute)
    pub path: PathBuf,

    /// Track duration (if specified in EXTINF)
    pub duration: Option<Duration>,

    /// Track title (if specified in EXTINF)
    pub title: Option<String>,

    /// Artist name (if extracted from title)
    pub artist: Option<String>,
}

impl M3u8Entry {
    /// Create a new entry with just a path.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            duration: None,
            title: None,
            artist: None,
        }
    }

    /// Create an entry with full metadata.
    pub fn with_metadata(
        path: PathBuf,
        duration: Option<Duration>,
        title: Option<String>,
        artist: Option<String>,
    ) -> Self {
        Self {
            path,
            duration,
            title,
            artist,
        }
    }
}

/// A parsed M3U8 playlist.
#[derive(Debug, Clone)]
pub struct M3u8Playlist {
    /// Playlist name (derived from filename)
    pub name: String,

    /// Playlist entries
    pub entries: Vec<M3u8Entry>,

    /// Whether this is an extended M3U (has #EXTM3U header)
    pub extended: bool,
}

impl M3u8Playlist {
    /// Create a new empty playlist.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
            extended: true,
        }
    }

    /// Add an entry to the playlist.
    pub fn add(&mut self, entry: M3u8Entry) {
        self.entries.push(entry);
    }

    /// Add a path to the playlist.
    pub fn add_path(&mut self, path: PathBuf) {
        self.entries.push(M3u8Entry::new(path));
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the playlist is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total duration if all entries have duration info.
    pub fn total_duration(&self) -> Option<Duration> {
        let mut total = Duration::ZERO;
        for entry in &self.entries {
            total += entry.duration?;
        }
        Some(total)
    }
}

/// M3U8 playlist reader.
pub struct M3u8Reader;

impl M3u8Reader {
    /// Read a playlist from a file.
    pub fn read<P: AsRef<Path>>(path: P) -> PlaylistResult<M3u8Playlist> {
        let path = path.as_ref();
        let file = File::open(path)
            .map_err(|_| PlaylistError::FileNotFound(path.to_path_buf()))?;

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        Self::read_from_reader(BufReader::new(file), name, path.parent())
    }

    /// Read a playlist from any reader.
    pub fn read_from_reader<R: BufRead>(
        reader: R,
        name: String,
        base_dir: Option<&Path>,
    ) -> PlaylistResult<M3u8Playlist> {
        let mut playlist = M3u8Playlist::new(name);
        let mut pending_extinf: Option<(Option<Duration>, Option<String>)> = None;

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            if line.starts_with("#EXTM3U") {
                playlist.extended = true;
                continue;
            }

            if let Some(info) = line.strip_prefix("#EXTINF:") {
                // Parse EXTINF line: #EXTINF:duration,title
                let (duration, title) = Self::parse_extinf(info);
                pending_extinf = Some((duration, title));
                continue;
            }

            if line.starts_with('#') {
                // Skip other comments/directives
                continue;
            }

            // This is a file path
            let path = if Path::new(line).is_absolute() {
                PathBuf::from(line)
            } else if let Some(base) = base_dir {
                base.join(line)
            } else {
                PathBuf::from(line)
            };

            let entry = if let Some((duration, title)) = pending_extinf.take() {
                let (artist, track_title) = Self::parse_title(title.as_deref());
                M3u8Entry::with_metadata(path, duration, track_title, artist)
            } else {
                M3u8Entry::new(path)
            };

            playlist.add(entry);
        }

        Ok(playlist)
    }

    /// Parse an EXTINF line content.
    fn parse_extinf(info: &str) -> (Option<Duration>, Option<String>) {
        let parts: Vec<&str> = info.splitn(2, ',').collect();

        let duration = parts.first().and_then(|s| {
            s.trim().parse::<f64>().ok().and_then(|d| {
                // M3U8 uses -1 for unknown duration; Duration can't be negative
                if d >= 0.0 { Some(Duration::from_secs_f64(d)) } else { None }
            })
        });

        let title = parts.get(1).map(|s| s.trim().to_string());

        (duration, title)
    }

    /// Parse a title into artist and track title.
    /// Common format: "Artist - Title"
    fn parse_title(title: Option<&str>) -> (Option<String>, Option<String>) {
        match title {
            Some(t) if t.contains(" - ") => {
                let parts: Vec<&str> = t.splitn(2, " - ").collect();
                (
                    Some(parts[0].trim().to_string()),
                    Some(parts[1].trim().to_string()),
                )
            }
            Some(t) => (None, Some(t.to_string())),
            None => (None, None),
        }
    }
}

/// M3U8 playlist writer.
pub struct M3u8Writer;

impl M3u8Writer {
    /// Write a playlist to a file.
    pub fn write<P: AsRef<Path>>(playlist: &M3u8Playlist, path: P) -> PlaylistResult<()> {
        let file = File::create(path.as_ref())?;
        let writer = BufWriter::new(file);
        Self::write_to_writer(playlist, writer, path.as_ref().parent())
    }

    /// Write a playlist to any writer.
    pub fn write_to_writer<W: Write>(
        playlist: &M3u8Playlist,
        mut writer: W,
        base_dir: Option<&Path>,
    ) -> PlaylistResult<()> {
        // Write header
        if playlist.extended {
            writeln!(writer, "#EXTM3U")?;
        }

        // Write entries
        for entry in &playlist.entries {
            // Write EXTINF if we have metadata
            if playlist.extended && (entry.duration.is_some() || entry.title.is_some()) {
                let duration = entry
                    .duration
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(-1);

                let title = match (&entry.artist, &entry.title) {
                    (Some(artist), Some(title)) => format!("{} - {}", artist, title),
                    (None, Some(title)) => title.clone(),
                    (Some(artist), None) => artist.clone(),
                    (None, None) => String::new(),
                };

                writeln!(writer, "#EXTINF:{},{}", duration, title)?;
            }

            // Write path (relative if possible)
            let path_str = if let Some(base) = base_dir {
                entry
                    .path
                    .strip_prefix(base)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| entry.path.to_string_lossy().to_string())
            } else {
                entry.path.to_string_lossy().to_string()
            };

            writeln!(writer, "{}", path_str)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extinf() {
        let (dur, title) = M3u8Reader::parse_extinf("123,Artist - Song Title");
        assert_eq!(dur, Some(Duration::from_secs(123)));
        assert_eq!(title, Some("Artist - Song Title".to_string()));

        let (dur, title) = M3u8Reader::parse_extinf("-1,Unknown");
        assert_eq!(dur, None); // M3U8 uses -1 for unknown duration
        assert_eq!(title, Some("Unknown".to_string()));
    }

    #[test]
    fn test_parse_title() {
        let (artist, title) = M3u8Reader::parse_title(Some("Pink Floyd - Echoes"));
        assert_eq!(artist, Some("Pink Floyd".to_string()));
        assert_eq!(title, Some("Echoes".to_string()));

        let (artist, title) = M3u8Reader::parse_title(Some("Just a Title"));
        assert_eq!(artist, None);
        assert_eq!(title, Some("Just a Title".to_string()));
    }
}
