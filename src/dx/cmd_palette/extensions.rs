use super::{PaletteItem, PaletteItemType};

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
        format!(
            "{}/Library/Application Support/Google/Chrome/Default/Extensions",
            home
        ),
        format!("{}/.config/google-chrome/Default/Extensions", home),
        format!(
            "{}/AppData/Local/Google/Chrome/User Data/Default/Extensions",
            home
        ),
    ];

    let mut discovered = Vec::new();
    for ext_dir_path in &possible_dirs {
        discover_extensions_in_dir(std::path::Path::new(ext_dir_path), &mut discovered);
    }
    discovered
}

fn discover_extensions_in_dir(dir_path: &std::path::Path, discovered: &mut Vec<PaletteItem>) {
    if !dir_path.exists() || !dir_path.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir_path) else {
        return;
    };
    for entry in entries.flatten() {
        let ext_id = entry.file_name().to_string_lossy().to_string();
        if let Some(item) = discover_extension(&entry.path(), &ext_id) {
            discovered.push(item);
        }
    }
}

fn discover_extension(ext_path: &std::path::Path, ext_id: &str) -> Option<PaletteItem> {
    if !ext_path.is_dir() {
        return None;
    }
    let versions = std::fs::read_dir(ext_path).ok()?;
    for version in versions.flatten() {
        if let Some(item) = read_extension_manifest(&version.path().join("manifest.json"), ext_id) {
            return Some(item);
        }
    }
    None
}

fn read_extension_manifest(manifest_path: &std::path::Path, ext_id: &str) -> Option<PaletteItem> {
    if !manifest_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(manifest_path).ok()?;
    parse_chrome_extension_manifest(&content, ext_id)
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
