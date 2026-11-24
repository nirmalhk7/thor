use super::{score_palette_item_prepared, PaletteItem, PreparedQuery};
use std::sync::{Arc, Mutex};

/// Command palette mode selected by hotkey.
///
/// - [`CmdPaletteMode::BookmarksAndTabHistory`] — **Sites (Cmd+K)**: open tabs, per-tab
///   visit history, and Thor bookmarks (see [`super::build_cmd_k_items`]).
/// - [`CmdPaletteMode::SettingsMenu`] — **Toolbox (Cmd+Shift+K)**: settings / devtools /
///   security / extensions from [`super::SettingsRegistry`] only (no bookmarks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdPaletteMode {
    /// Sites (Cmd+K): open tabs + tab history + bookmarks.
    BookmarksAndTabHistory,
    /// Toolbox (Cmd+Shift+K): settings registry only.
    SettingsMenu,
}

/// Neovim-style high-performance fuzzy searching command palette
pub struct CmdPalette {
    pub is_open: bool,
    pub mode: CmdPaletteMode,
    pub query: String,
    pub items: Arc<Mutex<Vec<PaletteItem>>>,
    pub selected_index: usize,
}

impl CmdPalette {
    pub fn new(mode: CmdPaletteMode) -> Self {
        Self {
            is_open: false,
            mode,
            query: String::new(),
            items: Arc::new(Mutex::new(Vec::new())),
            selected_index: 0,
        }
    }

    pub fn set_items(&self, new_items: Vec<PaletteItem>) {
        if let Ok(mut items) = self.items.lock() {
            *items = new_items;
        }
    }

    pub fn toggle(&mut self, mode: CmdPaletteMode) {
        if self.is_open && self.mode == mode {
            self.is_open = false;
            self.query.clear();
            self.selected_index = 0;
        } else {
            self.is_open = true;
            self.mode = mode;
            self.query.clear();
            self.selected_index = 0;
        }
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.selected_index = 0;
    }

    /// High-performance fuzzy scoring and search implementation
    pub fn search(&self) -> Vec<PaletteItem> {
        let items = self.items.lock().unwrap();
        let query_trimmed = self.query.trim();
        if query_trimmed.is_empty() {
            return items.iter().take(50).cloned().collect();
        }

        let prepared = PreparedQuery::new(query_trimmed);
        let len = items.len();
        if len == 0 {
            return Vec::new();
        }

        // Keep the same threshold and chunking for parallel search.
        let mut scored = if len < 1000 {
            top_scores(&items, &prepared, 0)
        } else {
            parallel_scores(&items, &prepared)
        };
        scored.sort_unstable_by_key(|&(score, index)| (std::cmp::Reverse(score), index));
        if len >= 1000 {
            scored.truncate(50);
        }
        scored
            .into_iter()
            .map(|(_, idx)| items[idx].clone())
            .collect()
    }
}

impl Default for CmdPalette {
    fn default() -> Self {
        Self::new(CmdPaletteMode::BookmarksAndTabHistory)
    }
}

struct TopMatches {
    heap: std::collections::BinaryHeap<std::cmp::Reverse<(i32, usize)>>,
    min_score: i32,
}

impl TopMatches {
    fn new() -> Self {
        Self {
            heap: std::collections::BinaryHeap::with_capacity(51),
            min_score: 0,
        }
    }

    fn insert(&mut self, score: i32, idx: usize) {
        if score <= self.min_score {
            return;
        }
        if self.heap.len() < 50 {
            self.heap.push(std::cmp::Reverse((score, idx)));
            if self.heap.len() == 50 {
                self.min_score = self.heap.peek().unwrap().0 .0;
            }
        } else {
            let mut head = self.heap.peek_mut().unwrap();
            head.0 = (score, idx);
            drop(head);
            self.min_score = self.heap.peek().unwrap().0 .0;
        }
    }
}

fn top_scores(items: &[PaletteItem], query: &PreparedQuery, base_idx: usize) -> Vec<(i32, usize)> {
    let mut top = TopMatches::new();
    for (i, item) in items.iter().enumerate() {
        let score = score_palette_item_prepared(
            query,
            &item.title,
            &item.path_or_url,
            item.badge.as_deref(),
        );
        top.insert(score, base_idx + i);
    }
    top.heap.into_iter().map(|r| r.0).collect()
}

fn parallel_scores(items: &[PaletteItem], query: &PreparedQuery) -> Vec<(i32, usize)> {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let chunk_size = items.len().div_ceil(num_threads);
    let thread_results: Vec<Vec<(i32, usize)>> = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(num_threads);
        for (chunk_idx, chunk) in items.chunks(chunk_size).enumerate() {
            handles.push(scope.spawn(move || top_scores(chunk, query, chunk_idx * chunk_size)));
        }
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut scored = Vec::with_capacity(num_threads * 50);
    for result in thread_results {
        scored.extend(result);
    }
    scored
}
