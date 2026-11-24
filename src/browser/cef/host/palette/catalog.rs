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
        ("api", "API workspace", API_URL, "Request editor"),
        (
            "security",
            "Security analyzers",
            SECURITY_URL,
            "Headers, JWT, response diff",
        ),
        (
            "web3_call",
            "Inspect an Ethereum call",
            crate::workbench::WEB3_URL,
            "Pasted eth_call JSON",
        ),
        ("extensions", "Extensions", "", "Browser"),
        ("inspect", "Inspect page", "", "Current page"),
        ("bookmark", "Bookmark current page", "", "Saved to Thor"),
        ("new_tab", "New tab", HOME_URL, "Browse"),
    ]
    .into_iter()
    .map(|(id, title, url, category)| {
        let mut entry = item(id, title, url, category, PaletteItemType::Action);
        if id == "api" {
            entry.badge = Some("replay request; resend request; send HTTP request".into());
        }
        if id == "web3_call" {
            entry.badge = Some("decode calldata; decode balanceof; inspect eth_call".into());
        }
        entry
    })
    .collect();
    let mut headers = item(
        "headers",
        "Security headers",
        &format!("{SECURITY_URL}#headers"),
        "Pasted headers",
        PaletteItemType::Action,
    );
    headers.badge = Some("inspect headers; analyze response headers".into());
    items.push(headers);
    items.extend(
        crate::AppState::global()
            .features()
            .registry()
            .all()
            .iter()
            .map(|feature| {
                let mut entry = item(
                    &format!("feature_{}", feature.slug),
                    &feature.name,
                    &format!("{TOOLS_URL}#{}", feature.slug),
                    &format!("{} · Pasted JSON", feature.category.as_str()),
                    PaletteItemType::Action,
                );
                if feature.slug == "perf-budgets" {
                    entry.category = "Performance · Pasted measurements".into();
                    entry.badge = Some("measure load; measure page load; check web vitals".into());
                }
                entry
            }),
    );
    items
}

pub(super) fn security() -> Vec<PaletteItem> {
    [
        ("headers", "Security headers", "Pasted headers"),
        ("jwt", "JWT inspector", "Pasted token"),
        ("diff", "Response diff", "Pasted HTTP snapshots"),
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
    fn toolbox_search_finds_tasks_and_describes_their_input() {
        use crate::dx::cmd_palette::{CmdPalette, CmdPaletteMode};

        let mut palette = CmdPalette::new(CmdPaletteMode::SettingsMenu);
        palette.set_items(toolbox());
        for (query, destination, input) in [
            ("replay request", "thor://workbench/api", "Request editor"),
            (
                "inspect headers",
                "thor://workbench/security#headers",
                "Pasted headers",
            ),
            (
                "measure load",
                "thor://workbench/tools#perf-budgets",
                "Pasted measurements",
            ),
        ] {
            palette.set_query(query.into());
            let matches = palette.search();
            let first = matches
                .first()
                .unwrap_or_else(|| panic!("No match for {query}"));
            assert_eq!(first.path_or_url, destination, "{query}");
            assert!(
                first.category.contains(input),
                "{query}: {}",
                first.category
            );
        }

        palette.set_query("decode calldata".into());
        let first = &palette.search()[0];
        assert_eq!(first.path_or_url, crate::workbench::WEB3_URL);
        assert_eq!(first.category, "Pasted eth_call JSON");

        palette.set_query("inspect page".into());
        assert!(palette.search()[0].category.contains("Current page"));
    }

    #[test]
    fn security_drawer_opens_the_matching_offline_analyzer() {
        let items = security();
        for (title, url) in [
            ("Security headers", "thor://workbench/security#headers"),
            ("JWT inspector", "thor://workbench/security#jwt"),
            ("Response diff", "thor://workbench/security#diff"),
        ] {
            assert!(items
                .iter()
                .any(|item| item.title == title && item.path_or_url == url));
        }
    }
}
