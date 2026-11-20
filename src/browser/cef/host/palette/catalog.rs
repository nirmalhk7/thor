use crate::browser::session::BrowserSession;
use crate::dx::cmd_palette::{PaletteItem, PaletteItemType};
use crate::workbench::{API_URL, HOME_URL, SECURITY_URL, TOOLS_URL};

pub(super) fn item(
    id: &str,
    title: &str,
    url: &str,
    category: &str,
    kind: PaletteItemType,
) -> PaletteItem {
    PaletteItem::new(id, title, kind, url, None, None, category, None)
}

pub(super) fn sites(session: &BrowserSession, bookmarks: Vec<PaletteItem>) -> Vec<PaletteItem> {
    let mut items: Vec<_> = session
        .summaries()
        .into_iter()
        .map(|tab| {
            item(
                &format!("tab_{}", tab.target.id),
                if tab.title.is_empty() {
                    &tab.url
                } else {
                    &tab.title
                },
                &tab.url,
                "Open tab",
                PaletteItemType::Tab,
            )
        })
        .collect();
    if let Some(active) = session.active_tab() {
        if let Ok(history) = session.history(active) {
            items.extend(history.iter().rev().enumerate().map(|(index, visit)| {
                item(
                    &format!("history_{index}"),
                    if visit.title.is_empty() {
                        &visit.url
                    } else {
                        &visit.title
                    },
                    &visit.url,
                    "This tab’s history",
                    PaletteItemType::TabHistory,
                )
            }));
        }
    }
    items.extend(bookmarks);
    items
}

pub(super) fn toolbox() -> Vec<PaletteItem> {
    let mut items: Vec<_> = [
        (
            "control_center",
            "Developer Control Center",
            TOOLS_URL,
            "144 engineer features",
        ),
        ("api", "API workspace", API_URL, "Requests and responses"),
        (
            "security",
            "Security analyzers",
            SECURITY_URL,
            "Headers, JWT, response diff",
        ),
        ("extensions", "Extensions", "", "Browser"),
        ("inspect", "Inspect page", "", "Developer Tools"),
        ("bookmark", "Bookmark current page", "", "Saved to Thor"),
        ("new_tab", "New tab", HOME_URL, "Browse"),
    ]
    .into_iter()
    .map(|(id, title, url, category)| item(id, title, url, category, PaletteItemType::Action))
    .collect();
    items.extend(
        crate::AppState::global()
            .features()
            .registry()
            .all()
            .iter()
            .map(|feature| {
                item(
                    &format!("feature_{}", feature.slug),
                    &feature.name,
                    &format!("{TOOLS_URL}#{}", feature.slug),
                    feature.category.as_str(),
                    PaletteItemType::Action,
                )
            }),
    );
    items
}

pub(super) fn security() -> Vec<PaletteItem> {
    [
        ("headers", "Security headers", "Review browser protections"),
        ("jwt", "JWT inspector", "Inspect claims and expiry"),
        ("diff", "Response diff", "Compare HTTP snapshots"),
    ]
    .into_iter()
    .map(|(id, title, detail)| {
        item(
            id,
            title,
            &format!("{SECURITY_URL}#{id}"),
            detail,
            PaletteItemType::Action,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_drawer_opens_the_matching_offline_analyzer() {
        let items = security();
        for (title, url) in [
            ("Security headers", "thor://workbench/security#headers"),
            ("JWT inspector", "thor://workbench/security#jwt"),
            ("Response diff", "thor://workbench/security#diff"),
        ] {
            assert!(items.iter().any(|item| item.title == title && item.path_or_url == url));
        }
    }
}
