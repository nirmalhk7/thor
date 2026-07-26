use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents an item type that can be found in the command palette.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteItemType {
    Bookmark,
    TabHistory,
    Setting,
    DevTools,
    Action,
    Script,
    Css,
    NetworkRequest,
    Tab,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteItem {
    pub id: String,
    pub title: String,
    pub item_type: PaletteItemType,
    pub path_or_url: String,
    pub badge: Option<String>,
    pub icon: Option<String>,
    pub category: String,
    pub action_key: Option<String>,
}

impl PaletteItem {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        item_type: PaletteItemType,
        path_or_url: impl Into<String>,
        badge: Option<String>,
        icon: Option<String>,
        category: impl Into<String>,
        action_key: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            item_type,
            path_or_url: path_or_url.into(),
            badge,
            icon,
            category: category.into(),
            action_key,
        }
    }
}

/// Entry representing a single page visit inside a tab
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabHistoryEntry {
    pub url: String,
    pub title: String,
    pub timestamp: u64,
}

/// Tracks visited pages per tab.
///
/// **Privacy model:** history is in-memory and scoped per tab. Call
/// [`TabHistoryTracker::clear_tab_history`] when a tab closes so visit data
/// does not outlive the tab. Use [`TabHistoryTracker::clear_all_history`] for
/// an explicit privacy wipe (also exposed via Cmd+Shift+K).
#[derive(Debug, Clone, Default)]
pub struct TabHistoryTracker {
    history_by_tab: HashMap<String, std::collections::VecDeque<TabHistoryEntry>>,
}

impl TabHistoryTracker {
    pub fn new() -> Self {
        Self {
            history_by_tab: HashMap::new(),
        }
    }

    pub fn record_visit(&mut self, tab_id: &str, url: &str, title: &str) {
        if url.is_empty() || url == "about:blank" {
            return;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let entries = self.history_by_tab.entry(tab_id.to_string()).or_default();
        
        // Avoid duplicate consecutive entry for same URL
        if let Some(last) = entries.back() {
            if last.url == url {
                return;
            }
        }

        let entry_title = if title.is_empty() { url.to_string() } else { title.to_string() };
        entries.push_back(TabHistoryEntry {
            url: url.to_string(),
            title: entry_title,
            timestamp: now,
        });

        // Cap per-tab history to 100 items with O(1) pop_front
        if entries.len() > 100 {
            entries.pop_front();
        }
    }

    pub fn get_history_for_tab(&self, tab_id: &str) -> Vec<TabHistoryEntry> {
        self.history_by_tab
            .get(tab_id)
            .map(|deque| deque.iter().rev().cloned().collect())
            .unwrap_or_default()
    }

    /// Drop all visit history for a tab. Intended to run on tab close (privacy).
    pub fn clear_tab_history(&mut self, tab_id: &str) {
        self.history_by_tab.remove(tab_id);
    }

    pub fn clear_all_history(&mut self) {
        self.history_by_tab.clear();
    }

    /// Build [`PaletteItem`]s for one tab's visit history (most recent first).
    pub fn palette_items_for_tab(&self, tab_id: &str) -> Vec<PaletteItem> {
        tab_history_entries_to_palette_items(&self.get_history_for_tab(tab_id), tab_id)
    }

    /// Build [`PaletteItem`]s for every tab's visit history (most recent first).
    pub fn all_palette_items(&self) -> Vec<PaletteItem> {
        let mut keyed: Vec<(u64, PaletteItem)> = Vec::new();
        for (tab_id, entries) in &self.history_by_tab {
            for (i, e) in entries.iter().rev().enumerate() {
                keyed.push((
                    e.timestamp,
                    PaletteItem::new(
                        format!("hist_{}_{}_{}", tab_id, e.timestamp, i),
                        e.title.clone(),
                        PaletteItemType::TabHistory,
                        e.url.clone(),
                        Some("Tab History".into()),
                        Some("🌐".into()),
                        "Tab History",
                        Some(format!("navigate_history_{}", tab_id)),
                    ),
                ));
            }
        }
        keyed.sort_by(|a, b| b.0.cmp(&a.0));
        keyed.into_iter().map(|(_, item)| item).collect()
    }
}

/// Convert tab history entries into Cmd+K palette items.
pub fn tab_history_entries_to_palette_items(
    entries: &[TabHistoryEntry],
    tab_id: &str,
) -> Vec<PaletteItem> {
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            PaletteItem::new(
                format!("hist_{}_{}_{}", tab_id, e.timestamp, i),
                e.title.clone(),
                PaletteItemType::TabHistory,
                e.url.clone(),
                Some("Tab History".into()),
                Some("🌐".into()),
                "Tab History",
                Some(format!("navigate_history_{}", tab_id)),
            )
        })
        .collect()
}

/// Convert currently open tabs `(id, title, url)` into Cmd+K palette items.
///
/// Uses [`PaletteItemType::Tab`]. Intended for the Cmd+K
/// [`CmdPaletteMode::BookmarksAndTabHistory`] pool (open tabs + history + bookmarks).
pub fn open_tabs_to_palette_items(tabs: &[(&str, &str, &str)]) -> Vec<PaletteItem> {
    tabs.iter()
        .map(|(id, title, url)| {
            let display = if title.is_empty() { *url } else { *title };
            PaletteItem::new(
                format!("tab_{}", id),
                display,
                PaletteItemType::Tab,
                *url,
                Some("Open Tab".into()),
                Some("📑".into()),
                "Open Tabs",
                Some(format!("focus_tab_{}", id)),
            )
        })
        .collect()
}

/// Build the full Cmd+K item list for [`CmdPaletteMode::BookmarksAndTabHistory`].
///
/// Order (ranking before fuzzy filter):
/// 1. Open tabs
/// 2. In-tab visit history
/// 3. Thor bookmarks
///
/// Bookmarks must **not** appear in [`SettingsRegistry`] (Cmd+Shift+K).
pub fn build_cmd_k_items(
    bookmarks: &[PaletteItem],
    tab_history: &[PaletteItem],
    open_tabs: &[PaletteItem],
) -> Vec<PaletteItem> {
    let mut items = Vec::with_capacity(open_tabs.len() + tab_history.len() + bookmarks.len());
    items.extend(open_tabs.iter().cloned());
    items.extend(tab_history.iter().cloned());
    items.extend(bookmarks.iter().cloned());
    items
}

/// Convenience: build Cmd+K items from raw open-tab tuples + history entries + bookmarks.
pub fn build_cmd_k_items_from_sources(
    bookmarks: &[PaletteItem],
    tab_history: &[TabHistoryEntry],
    history_tab_id: &str,
    open_tabs: &[(&str, &str, &str)],
) -> Vec<PaletteItem> {
    let tabs = open_tabs_to_palette_items(open_tabs);
    let history = tab_history_entries_to_palette_items(tab_history, history_tab_id);
    build_cmd_k_items(bookmarks, &history, &tabs)
}

/// Serialize palette items to a JSON array string for chrome UI IPC.
pub fn palette_items_to_json(items: &[PaletteItem]) -> Result<String, serde_json::Error> {
    serde_json::to_string(items)
}

/// Serialize Cmd+K payload (open tabs + history + bookmarks) as JSON.
pub fn cmd_k_payload_json(
    bookmarks: &[PaletteItem],
    tab_history: &[PaletteItem],
    open_tabs: &[PaletteItem],
) -> Result<String, serde_json::Error> {
    palette_items_to_json(&build_cmd_k_items(bookmarks, tab_history, open_tabs))
}

/// Serialize Cmd+Shift+K settings registry as JSON (no bookmarks).
pub fn settings_payload_json() -> Result<String, serde_json::Error> {
    palette_items_to_json(&SettingsRegistry::get_all_settings())
}

/// Cmd+Shift+K only: settings, IDE bridge, security, performance, and extension actions.
///
/// Do **not** put bookmarks, open tabs, or tab history here — those belong to
/// [`CmdPaletteMode::BookmarksAndTabHistory`] (Cmd+K) via [`build_cmd_k_items`].
pub struct SettingsRegistry;

impl SettingsRegistry {
    /// Honest M2 tools — dual-window Chromium + AppKit chrome (no CEF theater).
    pub fn get_all_settings() -> Vec<PaletteItem> {
        vec![
            PaletteItem::new(
                "set_devtools",
                "Toggle Chromium Developer Tools",
                PaletteItemType::Setting,
                "Thor://settings/devtools",
                Some("F12 / CDP".into()),
                Some("🛠️".into()),
                "Settings",
                Some("toggle_devtools".into()),
            ),
            PaletteItem::new(
                "set_engine",
                "Engine status (honest Chromium M2)",
                PaletteItemType::Setting,
                "Thor://settings/engine",
                Some("chromium · CDP · dual-window".into()),
                Some("ℹ️".into()),
                "Settings",
                Some("engine_status".into()),
            ),
            PaletteItem::new(
                "set_profile",
                "Toggle DevHost ↔ Stealth profile",
                PaletteItemType::Setting,
                "Thor://settings/profile",
                Some("⌘⇧P".into()),
                Some("👤".into()),
                "Security",
                Some("toggle_profile".into()),
            ),
            PaletteItem::new(
                "set_idor",
                "Run IDOR Matrix Scan on Active Target",
                PaletteItemType::Setting,
                "Thor://security/idor",
                Some("Level 2 Identity Diff".into()),
                Some("🕵️".into()),
                "Security",
                Some("run_idor_matrix".into()),
            ),
            PaletteItem::new(
                "set_locust",
                "Load-test active tab (local/private)",
                PaletteItemType::Setting,
                "Thor://load",
                Some("⌘⇧L".into()),
                Some("📊".into()),
                "Performance",
                Some("locust_active_origin".into()),
            ),
            PaletteItem::new(
                "set_vscode",
                "Open path in VS Code",
                PaletteItemType::Setting,
                "vscode://file/.",
                Some("vscode://".into()),
                Some("💻".into()),
                "IDE Bridge",
                Some("open_ide_vscode".into()),
            ),
            PaletteItem::new(
                "set_cursor",
                "Open path in Cursor",
                PaletteItemType::Setting,
                "cursor://file/.",
                Some("cursor://".into()),
                Some("⚡".into()),
                "IDE Bridge",
                Some("open_ide_cursor".into()),
            ),
            PaletteItem::new(
                "set_webstorm",
                "Open path in WebStorm",
                PaletteItemType::Setting,
                "webstorm://file/.",
                Some("webstorm://".into()),
                Some("🚀".into()),
                "IDE Bridge",
                Some("open_ide_webstorm".into()),
            ),
            PaletteItem::new(
                "set_ssl",
                "Local SSL CA (generate / install)",
                PaletteItemType::Setting,
                "Thor://settings/ssl",
                Some("ECDSA P-256".into()),
                Some("🛡️".into()),
                "Security",
                Some("show_ssl_details".into()),
            ),
            PaletteItem::new(
                "set_security_shield",
                "Tracker strip shield (utm_*, gclid, fbclid)",
                PaletteItemType::Setting,
                "Thor://settings/security_shield",
                Some("Protection".into()),
                Some("🛡️".into()),
                "Security",
                Some("toggle_security_shield".into()),
            ),
            PaletteItem::new(
                "set_clear_history",
                "Clear visited tab history",
                PaletteItemType::Setting,
                "Thor://settings/clear_history",
                Some("Privacy".into()),
                Some("🗑️".into()),
                "Privacy",
                Some("clear_history".into()),
            ),
            PaletteItem::new(
                "set_memory",
                "Compact idle tabs (≥24h)",
                PaletteItemType::Setting,
                "Thor://settings/memory",
                Some("GC".into()),
                Some("❄️".into()),
                "Performance",
                Some("trigger_memory_compaction".into()),
            ),
            PaletteItem::new(
                "set_web_vitals",
                "Core Web Vitals snapshot (LCP / CLS / INP)",
                PaletteItemType::Setting,
                "Thor://settings/web_vitals",
                Some("CWV".into()),
                Some("📊".into()),
                "Performance",
                Some("show_web_vitals".into()),
            ),
            PaletteItem::new(
                "set_sla_metrics",
                "SLA / capability readout (honest M2)",
                PaletteItemType::Setting,
                "Thor://settings/sla_metrics",
                Some("Chromium CDP".into()),
                Some("📈".into()),
                "Performance",
                Some("show_sla_metrics".into()),
            ),
        ]
    }
}

/// Parse a Chrome Extension manifest.json string into a PaletteItem
pub fn parse_chrome_extension_manifest(manifest_json: &str, ext_id: &str) -> Option<PaletteItem> {
    let parsed: serde_json::Value = serde_json::from_str(manifest_json).ok()?;
    let raw_name = parsed["name"].as_str()?;
    
    // Handle localized name strings like "__MSG_extName__"
    let name = if raw_name.starts_with("__MSG_") {
        ext_id
    } else {
        raw_name
    };
    
    let version = parsed["version"].as_str().unwrap_or("1.0");
    let title = format!("{} v{}", name, version);
    
    Some(PaletteItem::new(
        format!("ext_{}", ext_id),
        title,
        PaletteItemType::Setting,
        format!("chrome-extension://{}", ext_id),
        Some("Extension".into()),
        Some("🧩".into()),
        "Extensions",
        Some(format!("open_extension_{}", ext_id)),
    ))
}

/// Scans system Chrome extension directory to discover installed Chrome extensions
pub fn discover_installed_chrome_extensions() -> Vec<PaletteItem> {
    let home = match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    let possible_dirs = [
        format!("{}/Library/Application Support/Google/Chrome/Default/Extensions", home),
        format!("{}/.config/google-chrome/Default/Extensions", home),
        format!("{}/AppData/Local/Google/Chrome/User Data/Default/Extensions", home),
    ];

    let mut discovered = Vec::new();

    for ext_dir_path in &possible_dirs {
        let dir_path = std::path::Path::new(ext_dir_path);
        if !dir_path.exists() || !dir_path.is_dir() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let ext_id = entry.file_name().to_string_lossy().to_string();
                let ext_path = entry.path();

                if ext_path.is_dir() {
                    if let Ok(version_entries) = std::fs::read_dir(&ext_path) {
                        for v_entry in version_entries.flatten() {
                            let manifest_path = v_entry.path().join("manifest.json");
                            if manifest_path.exists() {
                                if let Ok(manifest_content) = std::fs::read_to_string(&manifest_path) {
                                    if let Some(item) = parse_chrome_extension_manifest(&manifest_content, &ext_id) {
                                        discovered.push(item);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    discovered
}

/// Discovers installed Chrome extensions or default dev extensions
pub fn read_chrome_extensions() -> Vec<PaletteItem> {
    let mut extensions = vec![
        PaletteItem::new(
            "ext_react_devtools",
            "React Developer Tools v5.2.0",
            PaletteItemType::Setting,
            "chrome-extension://fmkadmapgofadopljbjfkapdkoienihi",
            Some("Extension".into()),
            Some("🧩".into()),
            "Extensions",
            Some("open_extension_react".into()),
        ),
        PaletteItem::new(
            "ext_redux_devtools",
            "Redux DevTools v3.1.3",
            PaletteItemType::Setting,
            "chrome-extension://lmhkpmbekcpmknklioeibfkpmmfibljd",
            Some("Extension".into()),
            Some("🧩".into()),
            "Extensions",
            Some("open_extension_redux".into()),
        ),
        PaletteItem::new(
            "ext_vue_devtools",
            "Vue.js devtools v6.5.1",
            PaletteItemType::Setting,
            "chrome-extension://nhdogjmejiglipccpnnnhbfiladjaklb",
            Some("Extension".into()),
            Some("🧩".into()),
            "Extensions",
            Some("open_extension_vue".into()),
        ),
        PaletteItem::new(
            "ext_ublock",
            "uBlock Origin — Content Blocker & Privacy Guard",
            PaletteItemType::Setting,
            "chrome-extension://cjpalhdlnbpafiamejdnhcphjbkeiagm",
            Some("Extension".into()),
            Some("🧩".into()),
            "Extensions",
            Some("open_extension_ublock".into()),
        ),
        PaletteItem::new(
            "ext_tailwind",
            "Tailwind CSS DevTools Inspector",
            PaletteItemType::Setting,
            "chrome-extension://gnhhdgckkhfamcamhcppdaacflandcbb",
            Some("Extension".into()),
            Some("🧩".into()),
            "Extensions",
            Some("open_extension_tailwind".into()),
        ),
    ];

    let discovered = discover_installed_chrome_extensions();
    for item in discovered {
        if !extensions.iter().any(|e| e.id == item.id) {
            extensions.push(item);
        }
    }

    extensions
}

/// Fallback default bookmarks when Chrome Bookmarks file is not found
pub fn get_default_bookmarks() -> Vec<PaletteItem> {
    vec![
        PaletteItem::new(
            "bm_github",
            "GitHub — Where the world builds software",
            PaletteItemType::Bookmark,
            "https://github.com",
            Some("Default".into()),
            Some("🐙".into()),
            "Bookmarks",
            None,
        ),
        PaletteItem::new(
            "bm_google",
            "Google Search",
            PaletteItemType::Bookmark,
            "https://www.google.com",
            Some("Default".into()),
            Some("🔍".into()),
            "Bookmarks",
            None,
        ),
        PaletteItem::new(
            "bm_mdn",
            "MDN Web Docs — Web Developer Resources",
            PaletteItemType::Bookmark,
            "https://developer.mozilla.org",
            Some("Default".into()),
            Some("📚".into()),
            "Bookmarks",
            None,
        ),
        PaletteItem::new(
            "bm_so",
            "Stack Overflow — Developer Community",
            PaletteItemType::Bookmark,
            "https://stackoverflow.com",
            Some("Default".into()),
            Some("💬".into()),
            "Bookmarks",
            None,
        ),
        PaletteItem::new(
            "bm_localhost",
            "Localhost:3000 Development Server",
            PaletteItemType::Bookmark,
            "http://localhost:3000",
            Some("Dev".into()),
            Some("⚙️".into()),
            "Bookmarks",
            None,
        ),
        PaletteItem::new(
            "bm_vite",
            "Vite Dev Server (localhost:5173)",
            PaletteItemType::Bookmark,
            "http://localhost:5173",
            Some("Dev".into()),
            Some("⚡".into()),
            "Bookmarks",
            None,
        ),
        PaletteItem::new(
            "bm_docs_rs",
            "Rust Documentation (docs.rs)",
            PaletteItemType::Bookmark,
            "https://docs.rs",
            Some("Dev".into()),
            Some("📄".into()),
            "Bookmarks",
            None,
        ),
    ]
}

/// Reads Chrome Bookmarks JSON from user's Chrome profile on macOS/Linux/Windows
pub fn read_chrome_bookmarks_json() -> Option<String> {
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
    
    let possible_paths = [
        format!("{}/Library/Application Support/Google/Chrome/Default/Bookmarks", home),
        format!("{}/.config/google-chrome/Default/Bookmarks", home),
        format!("{}/AppData/Local/Google/Chrome/User Data/Default/Bookmarks", home),
    ];

    for path in &possible_paths {
        if std::path::Path::new(path).exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                return Some(content);
            }
        }
    }
    None
}

/// Parse raw Chrome Bookmarks JSON string into a structured list of PaletteItems
pub fn parse_chrome_bookmarks(json_str: &str) -> Result<Vec<PaletteItem>, serde_json::Error> {
    let mut items = Vec::new();
    let parsed: serde_json::Value = serde_json::from_str(json_str)?;

    fn extract_nodes(node: &serde_json::Value, folder_path: &str, items: &mut Vec<PaletteItem>) {
        let name = node["name"].as_str().unwrap_or("");
        let node_type = node["type"].as_str().unwrap_or("");

        let current_path = if !folder_path.is_empty() && !name.is_empty() && name != "bookmark_bar" && name != "other" && name != "synced" {
            format!("{} / {}", folder_path, name)
        } else if name != "bookmark_bar" && name != "other" && name != "synced" {
            name.to_string()
        } else {
            folder_path.to_string()
        };

        if node_type == "url" {
            if let Some(url) = node["url"].as_str() {
                let icon = if url.contains("github") {
                    "🐙"
                } else if url.contains("google") || url.contains("gemini") {
                    "🔍"
                } else if url.contains("chatgpt") || url.contains("perplexity") || url.contains("oneai") {
                    "🤖"
                } else if url.contains("zoom") {
                    "📹"
                } else if url.contains("slack") {
                    "💬"
                } else if url.contains("canvas") || url.contains("colorado") {
                    "🎓"
                } else {
                    "🔖"
                };

                let title = if name.is_empty() { url } else { name };
                let badge = if folder_path.is_empty() { "Chrome".to_string() } else { folder_path.to_string() };

                items.push(PaletteItem::new(
                    format!("bm_{}", items.len()),
                    title,
                    PaletteItemType::Bookmark,
                    url,
                    Some(badge),
                    Some(icon.to_string()),
                    "Bookmarks",
                    None,
                ));
            }
        } else if let Some(children) = node["children"].as_array() {
            for child in children {
                extract_nodes(child, &current_path, items);
            }
        }
    }

    if let Some(roots) = parsed.get("roots") {
        if let Some(bar) = roots.get("bookmark_bar") {
            extract_nodes(bar, "", &mut items);
        }
        if let Some(other) = roots.get("other") {
            extract_nodes(other, "Other", &mut items);
        }
        if let Some(synced) = roots.get("synced") {
            extract_nodes(synced, "Mobile", &mut items);
        }
    }

    if items.is_empty() {
        Ok(items)
    } else {
        Ok(items)
    }
}

/// Command palette mode selected by hotkey.
///
/// - [`CmdPaletteMode::BookmarksAndTabHistory`] — **Sites (Cmd+K)**: open tabs, per-tab
///   visit history, and Thor bookmarks (see [`build_cmd_k_items`]).
/// - [`CmdPaletteMode::SettingsMenu`] — **Toolbox (Cmd+Shift+K)**: settings / devtools /
///   security / extensions from [`SettingsRegistry`] only (no bookmarks).
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

        // For small lists, single-threaded path; for large lists (>= 1,000), parallel path
        if len < 1000 {
            let mut min_heap: std::collections::BinaryHeap<std::cmp::Reverse<(i32, usize)>> =
                std::collections::BinaryHeap::with_capacity(51);
            let mut min_score = 0;

            for (idx, item) in items.iter().enumerate() {
                let score = score_palette_item_prepared(
                    &prepared,
                    &item.title,
                    &item.path_or_url,
                    item.badge.as_deref(),
                );

                if score > min_score {
                    if min_heap.len() < 50 {
                        min_heap.push(std::cmp::Reverse((score, idx)));
                        if min_heap.len() == 50 {
                            min_score = min_heap.peek().unwrap().0 .0;
                        }
                    } else {
                        let mut head = min_heap.peek_mut().unwrap();
                        head.0 = (score, idx);
                        drop(head);
                        min_score = min_heap.peek().unwrap().0 .0;
                    }
                }
            }

            let mut top_scored: Vec<(i32, usize)> = min_heap.into_iter().map(|r| r.0).collect();
            top_scored.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            top_scored.into_iter().map(|(_, idx)| items[idx].clone()).collect()
        } else {
            let num_threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(8);
            let chunk_size = (len + num_threads - 1) / num_threads;

            let thread_results: Vec<Vec<(i32, usize)>> = std::thread::scope(|s| {
                let mut handles = Vec::with_capacity(num_threads);

                for (chunk_idx, chunk) in items.chunks(chunk_size).enumerate() {
                    let prepared_ref = &prepared;
                    handles.push(s.spawn(move || {
                        let mut min_heap: std::collections::BinaryHeap<std::cmp::Reverse<(i32, usize)>> =
                            std::collections::BinaryHeap::with_capacity(51);
                        let mut min_score = 0;
                        let base_idx = chunk_idx * chunk_size;

                        for (i, item) in chunk.iter().enumerate() {
                            let score = score_palette_item_prepared(
                                prepared_ref,
                                &item.title,
                                &item.path_or_url,
                                item.badge.as_deref(),
                            );

                            if score > min_score {
                                let idx = base_idx + i;
                                if min_heap.len() < 50 {
                                    min_heap.push(std::cmp::Reverse((score, idx)));
                                    if min_heap.len() == 50 {
                                        min_score = min_heap.peek().unwrap().0 .0;
                                    }
                                } else {
                                    let mut head = min_heap.peek_mut().unwrap();
                                    head.0 = (score, idx);
                                    drop(head);
                                    min_score = min_heap.peek().unwrap().0 .0;
                                }
                            }
                        }

                        min_heap.into_iter().map(|r| r.0).collect()
                    }));
                }

                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });

            let mut all_scored = Vec::with_capacity(num_threads * 50);
            for res in thread_results {
                all_scored.extend(res);
            }

            all_scored.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            all_scored.truncate(50);
            all_scored.into_iter().map(|(_, idx)| items[idx].clone()).collect()
        }
    }


}

impl Default for CmdPalette {
    fn default() -> Self {
        Self::new(CmdPaletteMode::BookmarksAndTabHistory)
    }
}

pub struct PreparedQuery<'a> {
    pub raw: &'a str,
    pub lower_bytes: Vec<u8>,
    pub is_ascii: bool,
}

impl<'a> PreparedQuery<'a> {
    pub fn new(query: &'a str) -> Self {
        let raw = query.trim();
        let is_ascii = raw.is_ascii();
        let lower_bytes = if is_ascii {
            raw.bytes().map(|b| b.to_ascii_lowercase()).collect()
        } else {
            raw.to_lowercase().into_bytes()
        };

        Self {
            raw,
            lower_bytes,
            is_ascii,
        }
    }
}

/// Calculate fuzzy search match score for a palette item (zero-allocation fast path)
pub fn score_palette_item(query: &str, title: &str, url: &str, badge: Option<&str>) -> i32 {
    let prepared = PreparedQuery::new(query);
    score_palette_item_prepared(&prepared, title, url, badge)
}

pub fn score_palette_item_prepared(
    query: &PreparedQuery,
    title: &str,
    url: &str,
    badge: Option<&str>,
) -> i32 {
    if query.raw.is_empty() {
        return 0;
    }

    if starts_with_prepared(title, query) {
        if title.len() == query.lower_bytes.len() {
            return 1000;
        }
        return 800;
    }

    if word_starts_with_prepared(title, query) {
        return 600;
    }

    if contains_prepared(title, query) {
        return 400;
    }

    if contains_prepared(url, query) {
        return 300;
    }

    if let Some(b) = badge {
        if contains_prepared(b, query) {
            return 200;
        }
    }

    if query.lower_bytes.len() <= 4 && is_subsequence_prepared(title, query) {
        return 100;
    }

    0
}

#[inline(always)]
fn to_lower_fast(b: u8) -> u8 {
    b.to_ascii_lowercase()
}

#[allow(dead_code)]
#[inline]
fn eq_prepared(s: &str, query: &PreparedQuery) -> bool {
    if s.len() != query.lower_bytes.len() {
        return false;
    }
    if s.is_ascii() && query.is_ascii {
        let s_bytes = s.as_bytes();
        let q_bytes = &query.lower_bytes[..];
        for i in 0..s_bytes.len() {
            if to_lower_fast(s_bytes[i]) != q_bytes[i] {
                return false;
            }
        }
        true
    } else {
        s.to_lowercase() == query.raw.to_lowercase()
    }
}

#[inline]
fn starts_with_prepared(s: &str, query: &PreparedQuery) -> bool {
    let q_len = query.lower_bytes.len();
    if q_len == 0 {
        return true;
    }
    if s.is_ascii() && query.is_ascii {
        if s.len() < q_len {
            return false;
        }
        let s_bytes = &s.as_bytes()[..q_len];
        let q_bytes = &query.lower_bytes[..];
        for i in 0..q_len {
            if to_lower_fast(s_bytes[i]) != q_bytes[i] {
                return false;
            }
        }
        true
    } else {
        s.to_lowercase().starts_with(&query.raw.to_lowercase())
    }
}

#[inline]
fn word_starts_with_prepared(s: &str, query: &PreparedQuery) -> bool {
    let q_len = query.lower_bytes.len();
    if q_len == 0 {
        return true;
    }
    if s.is_ascii() && query.is_ascii {
        if s.len() < q_len {
            return false;
        }
        let s_bytes = s.as_bytes();
        let q_bytes = &query.lower_bytes[..];
        let first_byte = q_bytes[0];
        let max_start = s.len() - q_len;

        for i in 0..=max_start {
            if i == 0
                || s_bytes[i - 1] == b' '
                || s_bytes[i - 1] == b'-'
                || s_bytes[i - 1] == b'_'
                || s_bytes[i - 1] == b'/'
            {
                if to_lower_fast(s_bytes[i]) == first_byte {
                    let mut is_match = true;
                    for j in 1..q_len {
                        if to_lower_fast(s_bytes[i + j]) != q_bytes[j] {
                            is_match = false;
                            break;
                        }
                    }
                    if is_match {
                        return true;
                    }
                }
            }
        }
        false
    } else {
        for word in s.split_whitespace() {
            if starts_with_prepared(word, query) {
                return true;
            }
        }
        false
    }
}


#[inline]
fn contains_prepared(haystack: &str, query: &PreparedQuery) -> bool {
    let q_len = query.lower_bytes.len();
    if q_len == 0 {
        return true;
    }
    if haystack.is_ascii() && query.is_ascii {
        if haystack.len() < q_len {
            return false;
        }
        let h_bytes = haystack.as_bytes();
        let q_bytes = &query.lower_bytes[..];
        let first_byte = q_bytes[0];
        let max_start = haystack.len() - q_len;

        for idx in 0..=max_start {
            if to_lower_fast(h_bytes[idx]) == first_byte {
                let mut is_match = true;
                for i in 1..q_len {
                    if to_lower_fast(h_bytes[idx + i]) != q_bytes[i] {
                        is_match = false;
                        break;
                    }
                }
                if is_match {
                    return true;
                }
            }
        }
        false
    } else {
        haystack.to_lowercase().contains(&query.raw.to_lowercase())
    }
}

#[inline]
fn is_subsequence_prepared(haystack: &str, query: &PreparedQuery) -> bool {
    let q_len = query.lower_bytes.len();
    if q_len == 0 {
        return true;
    }
    if haystack.is_ascii() && query.is_ascii {
        if q_len > haystack.len() {
            return false;
        }
        let h_bytes = haystack.as_bytes();
        let q_bytes = &query.lower_bytes[..];
        let mut q_idx = 0;
        let mut target = q_bytes[0];

        for &b in h_bytes {
            if to_lower_fast(b) == target {
                q_idx += 1;
                if q_idx == q_len {
                    return true;
                }
                target = q_bytes[q_idx];
            }
        }
        false
    } else {
        let sub_lower = query.raw.to_lowercase();
        let full_lower = haystack.to_lowercase();
        let mut sub_chars = sub_lower.chars();
        let mut current = sub_chars.next();
        for c in full_lower.chars() {
            if let Some(target) = current {
                if c == target {
                    current = sub_chars.next();
                }
            } else {
                return true;
            }
        }
        current.is_none()
    }
}







#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrome_bookmarks_parser_invalid_json() {
        let items = parse_chrome_bookmarks("invalid json");
        assert!(items.is_err());
    }

    #[test]
    fn test_chrome_bookmarks_parser_valid_json() {
        let json_str = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "bookmark_bar",
                    "children": [
                        { "type": "url", "name": "Rust Docs", "url": "https://doc.rust-lang.org" },
                        { "type": "url", "name": "Thor Repo", "url": "https://github.com/thor/thor" }
                    ]
                }
            }
        }"#;

        let items = parse_chrome_bookmarks(json_str).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Rust Docs");
        assert_eq!(items[1].title, "Thor Repo");
        assert_eq!(items[1].icon.as_deref(), Some("🐙"));
    }

    #[test]
    fn test_tab_history_tracker() {
        let mut tracker = TabHistoryTracker::new();
        tracker.record_visit("tab1", "https://google.com", "Google");
        tracker.record_visit("tab1", "https://github.com", "GitHub");
        tracker.record_visit("tab1", "https://github.com", "GitHub"); // Duplicate adjacent

        let history = tracker.get_history_for_tab("tab1");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].url, "https://github.com");
        assert_eq!(history[1].url, "https://google.com");
    }

    #[test]
    fn test_settings_registry() {
        let settings = SettingsRegistry::get_all_settings();
        assert!(settings.len() >= 8);
        assert!(settings.iter().any(|s| s.title.contains("Developer Tools")));
        assert!(settings.iter().any(|s| s.title.contains("VS Code")));
    }

    #[test]
    fn test_fuzzy_search_scoring() {
        let items = vec![
            PaletteItem::new("1", "GitHub", PaletteItemType::Bookmark, "https://github.com", None, None, "Bookmarks", None),
            PaletteItem::new("2", "Google Search", PaletteItemType::Bookmark, "https://google.com", None, None, "Bookmarks", None),
        ];

        let mut palette = CmdPalette::new(CmdPaletteMode::BookmarksAndTabHistory);
        palette.set_items(items);

        palette.set_query("git".to_string());
        let results = palette.search();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "GitHub");
    }

    #[test]
    fn test_parse_chrome_extension_manifest() {
        let manifest = r#"{
            "manifest_version": 3,
            "name": "Vue DevTools",
            "version": "6.5.1",
            "description": "Vue Developer Tools extension"
        }"#;
        let item = parse_chrome_extension_manifest(manifest, "nhdogjmejiglipccpnnnhbfiladjaklb").unwrap();
        assert_eq!(item.id, "ext_nhdogjmejiglipccpnnnhbfiladjaklb");
        assert_eq!(item.title, "Vue DevTools v6.5.1");
        assert_eq!(item.category, "Extensions");
    }

    #[test]
    fn test_open_tabs_to_palette_items() {
        let tabs = [
            ("1", "GitHub", "https://github.com"),
            ("2", "", "https://example.com"),
        ];
        let items = open_tabs_to_palette_items(&tabs);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].item_type, PaletteItemType::Tab);
        assert_eq!(items[0].id, "tab_1");
        assert_eq!(items[0].title, "GitHub");
        assert_eq!(items[0].action_key.as_deref(), Some("focus_tab_1"));
        assert_eq!(items[1].title, "https://example.com");
        assert_eq!(items[1].category, "Open Tabs");
    }

    #[test]
    fn test_build_cmd_k_items_ranking_and_filter() {
        let bookmarks = vec![PaletteItem::new(
            "bm_mdn",
            "MDN Web Docs",
            PaletteItemType::Bookmark,
            "https://developer.mozilla.org",
            Some("Thor".into()),
            Some("📚".into()),
            "Bookmarks",
            None,
        )];
        let history = vec![PaletteItem::new(
            "hist_1",
            "Hacker News",
            PaletteItemType::TabHistory,
            "https://news.ycombinator.com",
            Some("Tab History".into()),
            Some("🌐".into()),
            "Tab History",
            None,
        )];
        let open_tabs = open_tabs_to_palette_items(&[("t1", "GitHub Open", "https://github.com")]);

        let combined = build_cmd_k_items(&bookmarks, &history, &open_tabs);
        assert_eq!(combined.len(), 3);
        // Open tabs first, then history, then bookmarks
        assert_eq!(combined[0].item_type, PaletteItemType::Tab);
        assert_eq!(combined[1].item_type, PaletteItemType::TabHistory);
        assert_eq!(combined[2].item_type, PaletteItemType::Bookmark);

        let mut palette = CmdPalette::new(CmdPaletteMode::BookmarksAndTabHistory);
        palette.set_items(combined);
        palette.set_query("github".into());
        let results = palette.search();
        assert!(!results.is_empty());
        assert_eq!(results[0].item_type, PaletteItemType::Tab);
        assert_eq!(results[0].title, "GitHub Open");

        palette.set_query("hacker".into());
        let hist_results = palette.search();
        assert_eq!(hist_results[0].item_type, PaletteItemType::TabHistory);

        let json = cmd_k_payload_json(&bookmarks, &history, &open_tabs).unwrap();
        assert!(json.contains("\"item_type\":\"Tab\"") || json.contains("\"Tab\""));
        assert!(json.contains("GitHub Open"));
    }

    #[test]
    fn test_clear_tab_history_privacy_on_close() {
        let mut tracker = TabHistoryTracker::new();
        tracker.record_visit("tab_a", "https://a.com", "A");
        tracker.record_visit("tab_b", "https://b.com", "B");
        assert_eq!(tracker.get_history_for_tab("tab_a").len(), 1);
        tracker.clear_tab_history("tab_a");
        assert!(tracker.get_history_for_tab("tab_a").is_empty());
        assert_eq!(tracker.get_history_for_tab("tab_b").len(), 1);
    }

    #[test]
    fn test_settings_registry_excludes_bookmarks() {
        let settings = SettingsRegistry::get_all_settings();
        assert!(settings
            .iter()
            .all(|s| s.item_type != PaletteItemType::Bookmark
                && s.item_type != PaletteItemType::Tab
                && s.item_type != PaletteItemType::TabHistory));
    }
}

