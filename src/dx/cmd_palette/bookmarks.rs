use super::{PaletteItem, PaletteItemType};

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
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;

    let possible_paths = [
        format!(
            "{}/Library/Application Support/Google/Chrome/Default/Bookmarks",
            home
        ),
        format!("{}/.config/google-chrome/Default/Bookmarks", home),
        format!(
            "{}/AppData/Local/Google/Chrome/User Data/Default/Bookmarks",
            home
        ),
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

    Ok(items)
}

fn extract_nodes(node: &serde_json::Value, folder_path: &str, items: &mut Vec<PaletteItem>) {
    let name = node["name"].as_str().unwrap_or("");
    let node_type = node["type"].as_str().unwrap_or("");
    let current_path = bookmark_folder_path(folder_path, name);
    if node_type == "url" {
        append_bookmark(node, name, folder_path, items);
    } else if let Some(children) = node["children"].as_array() {
        for child in children {
            extract_nodes(child, &current_path, items);
        }
    }
}

fn append_bookmark(
    node: &serde_json::Value,
    name: &str,
    folder_path: &str,
    items: &mut Vec<PaletteItem>,
) {
    if let Some(url) = node["url"].as_str() {
        let title = if name.is_empty() { url } else { name };
        let badge = if folder_path.is_empty() {
            "Chrome"
        } else {
            folder_path
        };
        items.push(PaletteItem::new(
            format!("bm_{}", items.len()),
            title,
            PaletteItemType::Bookmark,
            url,
            Some(badge.to_string()),
            Some(bookmark_icon(url).to_string()),
            "Bookmarks",
            None,
        ));
    }
}

fn bookmark_icon(url: &str) -> &'static str {
    const ICONS: &[(&[&str], &str)] = &[
        (&["github"], "🐙"),
        (&["google", "gemini"], "🔍"),
        (&["chatgpt", "perplexity", "oneai"], "🤖"),
        (&["zoom"], "📹"),
        (&["slack"], "💬"),
        (&["canvas", "colorado"], "🎓"),
    ];
    ICONS
        .iter()
        .find(|(keywords, _)| keywords.iter().any(|keyword| url.contains(keyword)))
        .map(|(_, icon)| *icon)
        .unwrap_or("🔖")
}

fn bookmark_folder_path(folder_path: &str, name: &str) -> String {
    let visible_name = !matches!(name, "bookmark_bar" | "other" | "synced");
    match (folder_path.is_empty(), name.is_empty(), visible_name) {
        (false, false, true) => format!("{folder_path} / {name}"),
        (_, false, true) => name.to_string(),
        _ => folder_path.to_string(),
    }
}
