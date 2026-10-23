//! Thor-native bookmark store.
//!
//! Persists to the platform application-support / data directory:
//! - macOS: `~/Library/Application Support/Thor/bookmarks.json`
//! - Linux: `~/.local/share/Thor/bookmarks.json`
//! - Windows: `%APPDATA%/Thor/bookmarks.json`
//!
//! Optional import from Chrome Bookmarks JSON via
//! [`crate::dx::cmd_palette::parse_chrome_bookmarks`].

use crate::dx::cmd_palette::{parse_chrome_bookmarks, PaletteItem, PaletteItemType};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Single Thor bookmark entry persisted in `bookmarks.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub created_at: u64,
}

impl Bookmark {
    pub fn new(title: impl Into<String>, url: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let url = url.into();
        let title = title.into();
        let id = format!("bm_{}_{}", now, simple_hash(&url));
        Self {
            id,
            title,
            url,
            folder: None,
            created_at: now,
        }
    }

    pub fn with_folder(mut self, folder: impl Into<String>) -> Self {
        self.folder = Some(folder.into());
        self
    }

    pub fn to_palette_item(&self) -> PaletteItem {
        let badge = self
            .folder
            .clone()
            .unwrap_or_else(|| "Thor".to_string());
        PaletteItem::new(
            self.id.clone(),
            self.title.clone(),
            PaletteItemType::Bookmark,
            self.url.clone(),
            Some(badge),
            Some(bookmark_icon_for_url(&self.url)),
            "Bookmarks",
            None,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct BookmarkFile {
    bookmarks: Vec<Bookmark>,
}

/// Durable Thor bookmark store backed by a JSON file.
#[derive(Debug, Clone)]
pub struct BookmarkStore {
    path: PathBuf,
    bookmarks: Vec<Bookmark>,
}

impl BookmarkStore {
    /// Platform data directory for Thor (`…/Thor`).
    pub fn platform_data_dir() -> PathBuf {
        crate::paths::thor_data_dir()
    }

    /// Default bookmarks file path under the platform data dir.
    pub fn default_path() -> PathBuf {
        Self::platform_data_dir().join("bookmarks.json")
    }

    /// Open (or create) the store at the platform default path.
    pub fn open() -> io::Result<Self> {
        Self::open_at(Self::default_path())
    }

    /// Open (or create) the store at an explicit path (useful for tests).
    pub fn open_at(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let bookmarks = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            if raw.trim().is_empty() {
                Vec::new()
            } else {
                let file: BookmarkFile = serde_json::from_str(&raw).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, e)
                })?;
                file.bookmarks
            }
        } else {
            Vec::new()
        };

        Ok(Self { path, bookmarks })
    }

    /// Open under a parent directory (writes `{dir}/bookmarks.json`).
    pub fn open_in(dir: impl AsRef<Path>) -> io::Result<Self> {
        Self::open_at(dir.as_ref().join("bookmarks.json"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn list(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    pub fn add(&mut self, title: impl Into<String>, url: impl Into<String>) -> io::Result<Bookmark> {
        let bookmark = Bookmark::new(title, url);
        // Skip exact URL duplicates — return existing.
        if let Some(existing) = self.bookmarks.iter().find(|b| b.url == bookmark.url) {
            return Ok(existing.clone());
        }
        self.bookmarks.push(bookmark.clone());
        self.persist()?;
        Ok(bookmark)
    }

    /// Add multiple bookmarks in a single transaction to prevent O(N^2) I/O overhead.
    pub fn add_batch(&mut self, bookmarks: impl IntoIterator<Item = Bookmark>) -> io::Result<usize> {
        let mut added = 0;
        for bookmark in bookmarks {
            if !self.bookmarks.iter().any(|b| b.url == bookmark.url) {
                self.bookmarks.push(bookmark);
                added += 1;
            }
        }
        if added > 0 {
            self.persist()?;
        }
        Ok(added)
    }

    pub fn add_bookmark(&mut self, bookmark: Bookmark) -> io::Result<Bookmark> {
        if let Some(existing) = self.bookmarks.iter().find(|b| b.url == bookmark.url) {
            return Ok(existing.clone());
        }
        self.bookmarks.push(bookmark.clone());
        self.persist()?;
        Ok(bookmark)
    }

    pub fn remove(&mut self, id: &str) -> io::Result<bool> {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.id != id);
        let removed = self.bookmarks.len() != before;
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    pub fn remove_by_url(&mut self, url: &str) -> io::Result<bool> {
        let before = self.bookmarks.len();
        self.bookmarks.retain(|b| b.url != url);
        let removed = self.bookmarks.len() != before;
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    /// Import from Chrome Bookmarks JSON (roots/bookmark_bar/…).
    /// Skips URLs already present. Returns count newly added.
    pub fn import_chrome_json(&mut self, chrome_json: &str) -> io::Result<usize> {
        let parsed = parse_chrome_bookmarks(chrome_json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut added = 0;
        for item in parsed {
            if item.item_type != PaletteItemType::Bookmark {
                continue;
            }
            if self.bookmarks.iter().any(|b| b.url == item.path_or_url) {
                continue;
            }
            let mut bm = Bookmark::new(item.title, item.path_or_url);
            if let Some(folder) = item.badge {
                bm.folder = Some(folder);
            }
            // Prefer stable id from palette when present.
            if item.id.starts_with("bm_") {
                bm.id = item.id;
            }
            self.bookmarks.push(bm);
            added += 1;
        }
        if added > 0 {
            self.persist()?;
        }
        Ok(added)
    }

    /// Import from the user's Chrome profile Bookmarks file when present.
    pub fn import_from_chrome_profile(&mut self) -> io::Result<usize> {
        match crate::dx::cmd_palette::read_chrome_bookmarks_json() {
            Some(json) => self.import_chrome_json(&json),
            None => Ok(0),
        }
    }

    pub fn to_palette_items(&self) -> Vec<PaletteItem> {
        self.bookmarks.iter().map(Bookmark::to_palette_item).collect()
    }

    /// Seed with Thor default bookmarks when the store is empty.
    pub fn seed_defaults_if_empty(&mut self) -> io::Result<bool> {
        if !self.bookmarks.is_empty() {
            return Ok(false);
        }
        for item in crate::dx::cmd_palette::get_default_bookmarks() {
            let mut bm = Bookmark::new(item.title, item.path_or_url);
            bm.id = item.id;
            if let Some(badge) = item.badge {
                bm.folder = Some(badge);
            }
            self.bookmarks.push(bm);
        }
        self.persist()?;
        Ok(true)
    }

    fn persist(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let lock_path = self.path.with_extension("lock");
        let mut retries = 0;
        
        loop {
            match fs::OpenOptions::new().write(true).create_new(true).open(&lock_path) {
                Ok(_) => break,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    if retries > 50 {
                        // Stale lock, break it
                        let _ = fs::remove_file(&lock_path);
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        retries += 1;
                    }
                }
                Err(e) => return Err(e),
            }
        }

        let file = BookmarkFile {
            bookmarks: self.bookmarks.clone(),
        };
        
        let write_result = (|| -> io::Result<()> {
            let json = serde_json::to_string_pretty(&file)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            
            let temp_path = self.path.with_extension("tmp");
            fs::write(&temp_path, json)?;
            fs::rename(&temp_path, &self.path)?;
            Ok(())
        })();

        let _ = fs::remove_file(&lock_path);
        write_result
    }
}

fn bookmark_icon_for_url(url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    let icon = if lower.contains("github") {
        "🐙"
    } else if lower.contains("google") || lower.contains("gemini") {
        "🔍"
    } else if lower.contains("mozilla") || lower.contains("mdn") {
        "📚"
    } else if lower.contains("stackoverflow") {
        "💬"
    } else if lower.contains("localhost") || lower.contains("127.0.0.1") {
        "⚙️"
    } else if lower.contains("docs.rs") {
        "📄"
    } else {
        "🔖"
    };
    icon.to_string()
}

fn simple_hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16777619);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    fn temp_store_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "thor_bookmarks_test_{}_{}_{}",
            std::process::id(),
            nanos,
            NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_bookmark_store_crud_persistence() {
        let dir = temp_store_dir();
        let path = dir.join("bookmarks.json");

        {
            let mut store = BookmarkStore::open_at(&path).unwrap();
            assert!(store.list().is_empty());

            let a = store.add("Rust", "https://www.rust-lang.org").unwrap();
            let b = store
                .add("Thor", "https://github.com/thor/thor")
                .unwrap();
            assert_eq!(store.list().len(), 2);
            assert!(a.id.starts_with("bm_"));
            assert_ne!(a.id, b.id);

            // Duplicate URL returns existing, does not grow.
            let dup = store.add("Rust Again", "https://www.rust-lang.org").unwrap();
            assert_eq!(dup.id, a.id);
            assert_eq!(store.list().len(), 2);
        }

        {
            let mut store = BookmarkStore::open_at(&path).unwrap();
            assert_eq!(store.list().len(), 2);
            assert!(store.remove(&store.list()[0].id.clone()).unwrap());
            assert_eq!(store.list().len(), 1);
        }

        {
            let store = BookmarkStore::open_at(&path).unwrap();
            assert_eq!(store.list().len(), 1);
            let items = store.to_palette_items();
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].item_type, PaletteItemType::Bookmark);
            assert_eq!(items[0].category, "Bookmarks");
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_bookmark_store_import_chrome_json() {
        let dir = temp_store_dir();
        let mut store = BookmarkStore::open_in(&dir).unwrap();

        let chrome = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "bookmark_bar",
                    "children": [
                        { "type": "url", "name": "Rust Docs", "url": "https://doc.rust-lang.org" },
                        { "type": "url", "name": "GitHub", "url": "https://github.com" }
                    ]
                }
            }
        }"#;

        let added = store.import_chrome_json(chrome).unwrap();
        assert_eq!(added, 2);
        assert_eq!(store.list().len(), 2);

        // Re-import should add nothing.
        let added2 = store.import_chrome_json(chrome).unwrap();
        assert_eq!(added2, 0);
        assert_eq!(store.list().len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_seed_defaults_if_empty() {
        let dir = temp_store_dir();
        let mut store = BookmarkStore::open_in(&dir).unwrap();
        assert!(store.seed_defaults_if_empty().unwrap());
        assert!(!store.list().is_empty());
        assert!(!store.seed_defaults_if_empty().unwrap());
        let _ = fs::remove_dir_all(&dir);
    }
}
