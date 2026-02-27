//! Play queue management.

use std::collections::VecDeque;

use hadal_common::RepeatMode;
use serde::{Deserialize, Serialize};

use crate::error::{PlaylistError, PlaylistResult};

/// An item in the play queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Track ID from the library
    pub track_id: i64,

    /// Track title (cached for display)
    pub title: String,

    /// Artist name (cached for display)
    pub artist: Option<String>,

    /// Duration in milliseconds
    pub duration_ms: i64,

    /// File path
    pub path: String,
}

impl QueueItem {
    /// Create a new queue item.
    pub fn new(
        track_id: i64,
        title: String,
        artist: Option<String>,
        duration_ms: i64,
        path: String,
    ) -> Self {
        Self {
            track_id,
            title,
            artist,
            duration_ms,
            path,
        }
    }
}

/// The play queue.
#[derive(Debug, Clone, Default)]
pub struct PlayQueue {
    /// Queue items
    items: VecDeque<QueueItem>,

    /// Current position in the queue
    position: usize,

    /// Original order (for unshuffle)
    original_order: Vec<i64>,

    /// Shuffle mode enabled
    shuffle: bool,

    /// Repeat mode
    repeat: RepeatMode,

    /// History of played tracks (for back navigation)
    history: Vec<usize>,

    /// Maximum history size
    max_history: usize,
}

impl PlayQueue {
    /// Create a new empty queue.
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            position: 0,
            original_order: Vec::new(),
            shuffle: false,
            repeat: RepeatMode::Off,
            history: Vec::new(),
            max_history: 100,
        }
    }

    /// Get the number of items in the queue.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the current position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Get the current item.
    pub fn current(&self) -> Option<&QueueItem> {
        self.items.get(self.position)
    }

    /// Get all items.
    pub fn items(&self) -> &VecDeque<QueueItem> {
        &self.items
    }

    /// Get an item by index.
    pub fn get(&self, index: usize) -> Option<&QueueItem> {
        self.items.get(index)
    }

    /// Get total duration of all items.
    pub fn total_duration_ms(&self) -> i64 {
        self.items.iter().map(|i| i.duration_ms).sum()
    }

    /// Get remaining duration from current position.
    pub fn remaining_duration_ms(&self) -> i64 {
        self.items
            .iter()
            .skip(self.position)
            .map(|i| i.duration_ms)
            .sum()
    }

    /// Add an item to the end of the queue.
    pub fn push_back(&mut self, item: QueueItem) {
        self.original_order.push(item.track_id);
        self.items.push_back(item);
    }

    /// Add an item to play next (after current).
    pub fn push_next(&mut self, item: QueueItem) {
        let insert_pos = if self.items.is_empty() {
            0
        } else {
            self.position + 1
        };

        self.original_order.insert(insert_pos, item.track_id);
        self.items.insert(insert_pos, item);
    }

    /// Add multiple items to the end.
    pub fn extend(&mut self, items: impl IntoIterator<Item = QueueItem>) {
        for item in items {
            self.push_back(item);
        }
    }

    /// Remove an item at the given index.
    pub fn remove(&mut self, index: usize) -> PlaylistResult<QueueItem> {
        if index >= self.items.len() {
            return Err(PlaylistError::InvalidIndex(index));
        }

        if index < self.position {
            self.position -= 1;
        }

        self.original_order.remove(index);
        self.items
            .remove(index)
            .ok_or(PlaylistError::InvalidIndex(index))
    }

    /// Clear the queue.
    pub fn clear(&mut self) {
        self.items.clear();
        self.original_order.clear();
        self.history.clear();
        self.position = 0;
    }

    /// Move to the next track.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&QueueItem> {
        if self.items.is_empty() {
            return None;
        }

        // Save current position to history
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(self.position);

        match self.repeat {
            RepeatMode::One => {
                // Stay on current track
            }
            RepeatMode::All => {
                self.position = (self.position + 1) % self.items.len();
            }
            RepeatMode::Off => {
                if self.position + 1 < self.items.len() {
                    self.position += 1;
                } else {
                    return None; // End of queue
                }
            }
        }

        self.current()
    }

    /// Move to the previous track.
    pub fn previous(&mut self) -> Option<&QueueItem> {
        if self.items.is_empty() {
            return None;
        }

        // Try to use history first
        if let Some(prev_pos) = self.history.pop() {
            self.position = prev_pos;
            return self.current();
        }

        // Otherwise, just go back one
        if self.position > 0 {
            self.position -= 1;
        } else if self.repeat == RepeatMode::All {
            self.position = self.items.len() - 1;
        }

        self.current()
    }

    /// Jump to a specific position.
    pub fn jump_to(&mut self, index: usize) -> PlaylistResult<&QueueItem> {
        if index >= self.items.len() {
            return Err(PlaylistError::InvalidIndex(index));
        }

        // Save current to history
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(self.position);

        self.position = index;
        self.current().ok_or(PlaylistError::QueueEmpty)
    }

    /// Set shuffle mode.
    pub fn set_shuffle(&mut self, shuffle: bool) {
        if shuffle == self.shuffle {
            return;
        }

        self.shuffle = shuffle;

        if shuffle {
            self.shuffle_queue();
        } else {
            self.unshuffle_queue();
        }
    }

    /// Toggle shuffle mode.
    pub fn toggle_shuffle(&mut self) {
        self.set_shuffle(!self.shuffle);
    }

    /// Get shuffle mode.
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Set repeat mode.
    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    /// Cycle repeat mode.
    pub fn cycle_repeat(&mut self) {
        self.repeat = self.repeat.cycle();
    }

    /// Get repeat mode.
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Shuffle the queue (keeping current track at position 0).
    fn shuffle_queue(&mut self) {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};

        if self.items.len() <= 1 {
            return;
        }

        // Move current to front
        if self.position > 0 {
            let current = self.items.remove(self.position).unwrap();
            self.items.push_front(current);
        }
        self.position = 0;

        // Fisher-Yates shuffle for remaining items
        let random = RandomState::new();
        let len = self.items.len();

        for i in (2..len).rev() {
            let mut hasher = random.build_hasher();
            hasher.write_usize(i);
            let j = (hasher.finish() as usize % i) + 1;
            self.items.swap(i, j);
        }
    }

    /// Restore original order.
    fn unshuffle_queue(&mut self) {
        if self.original_order.is_empty() {
            return;
        }

        let current_id = self.current().map(|i| i.track_id);

        // Build index map for O(1) lookups instead of O(n) position search
        let order_map: std::collections::HashMap<i64, usize> = self
            .original_order
            .iter()
            .enumerate()
            .map(|(i, &id)| (id, i))
            .collect();

        // Sort items back to original order
        let mut sorted: Vec<_> = self.items.drain(..).collect();
        sorted.sort_by_key(|item| {
            order_map.get(&item.track_id).copied().unwrap_or(usize::MAX)
        });

        self.items = sorted.into();

        // Restore position to current track
        if let Some(id) = current_id {
            self.position = self
                .items
                .iter()
                .position(|i| i.track_id == id)
                .unwrap_or(0);
        }
    }

    /// Move an item in the queue.
    pub fn move_item(&mut self, from: usize, to: usize) -> PlaylistResult<()> {
        if from >= self.items.len() || to >= self.items.len() {
            return Err(PlaylistError::InvalidIndex(from.max(to)));
        }

        if from == to {
            return Ok(());
        }

        let item = self.items.remove(from).unwrap();
        self.items.insert(to, item);

        // Update position if needed
        if self.position == from {
            self.position = to;
        } else if from < self.position && to >= self.position {
            self.position -= 1;
        } else if from > self.position && to <= self.position {
            self.position += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(id: i64) -> QueueItem {
        QueueItem::new(id, format!("Track {}", id), None, 180000, format!("/path/{}.flac", id))
    }

    #[test]
    fn test_queue_basic() {
        let mut queue = PlayQueue::new();
        assert!(queue.is_empty());

        queue.push_back(make_item(1));
        queue.push_back(make_item(2));
        queue.push_back(make_item(3));

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.current().unwrap().track_id, 1);

        queue.next();
        assert_eq!(queue.current().unwrap().track_id, 2);

        queue.next();
        assert_eq!(queue.current().unwrap().track_id, 3);
    }

    #[test]
    fn test_queue_repeat() {
        let mut queue = PlayQueue::new();
        queue.push_back(make_item(1));
        queue.push_back(make_item(2));

        queue.set_repeat(RepeatMode::All);

        queue.next();
        assert_eq!(queue.current().unwrap().track_id, 2);

        queue.next();
        assert_eq!(queue.current().unwrap().track_id, 1); // Wrapped around
    }
}
