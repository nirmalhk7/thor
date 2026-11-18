mod bookmarks;
mod extensions;
mod matching;
mod search;

pub use bookmarks::{get_default_bookmarks, parse_chrome_bookmarks, read_chrome_bookmarks_json};
pub use extensions::{
    discover_installed_chrome_extensions, parse_chrome_extension_manifest, read_chrome_extensions,
};
pub use matching::{score_palette_item, score_palette_item_prepared, PreparedQuery};
pub use search::{CmdPalette, CmdPaletteMode};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    Download,
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
    // Keep the positional API stable: this constructor mirrors the public fields and is used
    // throughout the feature suite and integration tests.
    #[allow(clippy::too_many_arguments)]
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

        let entry_title = if title.is_empty() {
            url.to_string()
        } else {
            title.to_string()
        };
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
        keyed.sort_by_key(|item| std::cmp::Reverse(item.0));
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

/// Serialize the Downloads palette (same modal chrome as Cmd+K / Cmd+⇧K).
pub fn downloads_payload_json(
    downloads: &[crate::engine::browser_features::DownloadItem],
) -> Result<String, serde_json::Error> {
    palette_items_to_json(&download_items_to_palette(downloads))
}

/// Map [`DownloadItem`]s into palette rows. Empty ledger → one honest empty-state row.
pub fn download_items_to_palette(
    downloads: &[crate::engine::browser_features::DownloadItem],
) -> Vec<PaletteItem> {
    if downloads.is_empty() {
        return vec![PaletteItem::new(
            "dl_empty",
            "No downloads yet",
            PaletteItemType::Download,
            "Thor://downloads",
            Some("Empty".into()),
            Some("⬇️".into()),
            "Downloads",
            None,
        )];
    }

    downloads
        .iter()
        .map(|d| {
            let badge = d.state_badge();
            let action = match d.state {
                crate::engine::browser_features::DownloadState::Completed => {
                    Some(format!("reveal_download_{}", d.id))
                }
                crate::engine::browser_features::DownloadState::InProgress
                | crate::engine::browser_features::DownloadState::Pending => {
                    Some(format!("open_download_{}", d.id))
                }
                _ => Some(format!("reveal_download_{}", d.id)),
            };
            PaletteItem::new(
                d.id.clone(),
                d.file_name(),
                PaletteItemType::Download,
                if d.target_path.is_empty() {
                    d.url.clone()
                } else {
                    d.target_path.clone()
                },
                Some(badge),
                Some("⬇️".into()),
                "Downloads",
                action,
            )
        })
        .collect()
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
                "set_downloads",
                "Downloads",
                PaletteItemType::Setting,
                "Thor://downloads",
                Some("Files".into()),
                Some("⬇️".into()),
                "Browser",
                Some("open_downloads".into()),
            ),
            PaletteItem::new(
                "set_devtools",
                "Toggle Browser Developer Tools",
                PaletteItemType::Setting,
                "Thor://settings/devtools",
                Some("F12 / CDP".into()),
                Some("🛠️".into()),
                "Settings",
                Some("toggle_devtools".into()),
            ),
            PaletteItem::new(
                "set_pip",
                "Toggle Picture-in-Picture",
                PaletteItemType::Setting,
                "Thor://settings/pip",
                Some("video PiP".into()),
                Some("⧉".into()),
                "Browser",
                Some("toggle_pip".into()),
            ),
            PaletteItem::new(
                "set_visit_recon",
                "Run Passive Visit Recon",
                PaletteItemType::Setting,
                "Thor://security/visit-recon",
                Some("headers · cookies · storage · 3p".into()),
                Some("🛰️".into()),
                "Security",
                Some("visit_recon_run".into()),
            ),
            PaletteItem::new(
                "set_engine",
                "Engine status (CEF)",
                PaletteItemType::Setting,
                "Thor://settings/engine",
                Some("cef · embedded".into()),
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
                "set_bitwarden",
                "Bitwarden vault (unlock / autofill)",
                PaletteItemType::Setting,
                "Thor://vault/bitwarden",
                Some("Vault".into()),
                Some("🔑".into()),
                "Security",
                Some("open_bitwarden_vault".into()),
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
            PaletteItem::new(
                "1",
                "GitHub",
                PaletteItemType::Bookmark,
                "https://github.com",
                None,
                None,
                "Bookmarks",
                None,
            ),
            PaletteItem::new(
                "2",
                "Google Search",
                PaletteItemType::Bookmark,
                "https://google.com",
                None,
                None,
                "Bookmarks",
                None,
            ),
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
        let item =
            parse_chrome_extension_manifest(manifest, "nhdogjmejiglipccpnnnhbfiladjaklb").unwrap();
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
        assert!(settings
            .iter()
            .any(|s| s.action_key.as_deref() == Some("open_downloads")));
    }

    #[test]
    fn test_download_items_to_palette() {
        use crate::engine::browser_features::{DownloadItem, DownloadState};
        let empty = download_items_to_palette(&[]);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].id, "dl_empty");

        let items = vec![DownloadItem {
            id: "dl_1".into(),
            url: "https://example.com/a.zip".into(),
            target_path: "/tmp/a.zip".into(),
            total_bytes: 100,
            received_bytes: 100,
            state: DownloadState::Completed,
            error_reason: None,
        }];
        let rows = download_items_to_palette(&items);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].item_type, PaletteItemType::Download);
        assert_eq!(rows[0].title, "a.zip");
        assert_eq!(rows[0].action_key.as_deref(), Some("reveal_download_dl_1"));
        let json = downloads_payload_json(&items).unwrap();
        assert!(json.contains("a.zip"));
    }
}
