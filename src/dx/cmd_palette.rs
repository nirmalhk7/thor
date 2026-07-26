use std::sync::{Arc, Mutex};

/// Represents an item that can be found in the command palette.
#[derive(Debug, Clone)]
pub enum PaletteItemType {
    Script,
    Css,
    NetworkRequest,
    Tab,
}

#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub id: String,
    pub title: String,
    pub item_type: PaletteItemType,
    pub path_or_url: String,
}

/// Neovim-style Cmd+K fuzzy searching command palette
pub struct CmdPalette {
    pub is_open: bool,
    pub query: String,
    pub items: Arc<Mutex<Vec<PaletteItem>>>,
    pub selected_index: Option<usize>,
}

impl CmdPalette {
    pub fn new() -> Self {
        Self {
            is_open: false,
            query: String::new(),
            items: Arc::new(Mutex::new(Vec::new())),
            selected_index: None,
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
        if !self.is_open {
            self.query.clear();
            self.selected_index = None;
        }
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        // In a real implementation, trigger fuzzy search here
    }

    pub fn add_item(&self, item: PaletteItem) {
        if let Ok(mut items) = self.items.lock() {
            items.push(item);
        }
    }

    /// Basic mock fuzzy search implementation
pub fn search(&self) -> Vec<PaletteItem> {
        let items = self.items.lock().unwrap();
        if self.query.is_empty() {
            return items.clone();
        }
        
        let query_lower = self.query.to_lowercase();
        items.iter()
            .filter(|item| item.title.to_lowercase().contains(&query_lower) || item.path_or_url.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    }
}

impl Default for CmdPalette {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads Chrome Bookmarks JSON from user's Chrome profile on macOS/Linux
pub fn read_chrome_bookmarks_json() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{}/Library/Application Support/Google/Chrome/Default/Bookmarks", home);
    if std::path::Path::new(&path).exists() {
        std::fs::read_to_string(&path).ok()
    } else {
        None
    }
}
