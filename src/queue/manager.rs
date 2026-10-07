//! Queue manager stub (foundation only, no playback logic yet).

use crate::models::Song;

/// In-memory play queue. API reserved for later agents.
#[derive(Debug, Default)]
pub struct QueueManager {
    songs: Vec<Song>,
    index: Option<usize>,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            songs: Vec::new(),
            index: None,
        }
    }

    /// Enqueue a song at the back.
    pub fn push(&mut self, song: Song) {
        self.songs.push(song);
    }

    /// Currently selected song, if any.
    pub fn current(&self) -> Option<&Song> {
        self.index.and_then(|i| self.songs.get(i))
    }

    /// Advance to next song, returning it.
    pub fn next(&mut self) -> Option<&Song> {
        let nxt = match self.index {
            None => 0,
            Some(i) => i.saturating_add(1),
        };
        if nxt < self.songs.len() {
            self.index = Some(nxt);
            self.songs.get(nxt)
        } else {
            None
        }
    }

    /// Go back to previous song, returning it.
    pub fn previous(&mut self) -> Option<&Song> {
        let prev = match self.index {
            None | Some(0) => 0,
            Some(i) => i.saturating_sub(1),
        };
        if prev < self.songs.len() {
            self.index = Some(prev);
            self.songs.get(prev)
        } else {
            None
        }
    }

    /// Remove song at `i`, adjusting the cursor.
    pub fn remove(&mut self, i: usize) -> Option<Song> {
        if i >= self.songs.len() {
            return None;
        }
        let removed = self.songs.remove(i);
        self.index = match self.index {
            None => None,
            Some(cur) if cur == i => None,
            Some(cur) if cur > i => Some(cur.saturating_sub(1)),
            Some(cur) => Some(cur),
        };
        Some(removed)
    }

    /// Clear queue and cursor.
    pub fn clear(&mut self) {
        self.songs.clear();
        self.index = None;
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    /// Cursor into [`songs`](Self::songs), if any.
    pub fn index(&self) -> Option<usize> {
        self.index
    }

    /// Move item at `i` up (swap with `i-1`). Returns false if out of range.
    pub fn move_up(&mut self, i: usize) -> bool {
        if i == 0 || i >= self.songs.len() {
            return false;
        }
        self.songs.swap(i, i - 1);
        self.index = match self.index {
            Some(cur) if cur == i => Some(i - 1),
            Some(cur) if cur == i - 1 => Some(i),
            other => other,
        };
        true
    }

    /// Move item at `i` down (swap with `i+1`). Returns false if out of range.
    pub fn move_down(&mut self, i: usize) -> bool {
        if i + 1 >= self.songs.len() {
            return false;
        }
        self.songs.swap(i, i + 1);
        self.index = match self.index {
            Some(cur) if cur == i => Some(i + 1),
            Some(cur) if cur == i + 1 => Some(i),
            other => other,
        };
        true
    }

    /// Select index `i` as current, returning the song.
    pub fn play_at(&mut self, i: usize) -> Option<&Song> {
        if i < self.songs.len() {
            self.index = Some(i);
            self.songs.get(i)
        } else {
            None
        }
    }
}
