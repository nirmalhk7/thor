use super::*;

pub(super) fn perform(action: &Action) {
    if !matches!(action, Action::Sites | Action::Toolbox) {
        palette::close();
    }
    match action {
        Action::Sites => palette::open(palette::Mode::Sites),
        Action::Toolbox => palette::open(palette::Mode::Toolbox),
        Action::OpenTool(url) => open_tool(url),
        Action::New => new_active_tab(),
        Action::NextTab | Action::PreviousTab => switch_adjacent_tab(action),
        Action::FocusAddress => focus_address(),
        Action::Home => load_home(),
        Action::Inspect => inspect(),

        _ => perform_browser_action(action),
    }
}

fn new_active_tab() {
    let identity = with_host(|host| {
        host.session
            .active_tab()
            .and_then(|id| host.session.tab_identity(id).ok())
            .unwrap_or(0)
    })
    .unwrap_or(0);
    if let Err(error) = new_tab(identity, HOME_URL) {
        crate::thor_warn!("thor::cef", "New tab failed: {error}");
    }
}

fn switch_adjacent_tab(action: &Action) {
    let target = with_host(|host| {
        let tabs = host.session.summaries();
        let active = host.session.active_tab()?;
        let current = tabs.iter().position(|tab| tab.target.id == active)?;
        let index = if matches!(action, Action::NextTab) {
            (current + 1) % tabs.len()
        } else {
            (current + tabs.len() - 1) % tabs.len()
        };
        Some(tabs[index].target.id)
    })
    .flatten();
    if let Some(id) = target {
        activate(id);
    }
}

fn focus_address() {
    if let Some(Some(address)) = with_host(|host| host.address.clone()) {
        address.request_focus();
        address.select_all(0);
    }
}

fn load_home() {
    if let Some(frame) = active_browser().and_then(|browser| browser.main_frame()) {
        frame.load_url(Some(&CefString::from(HOME_URL)));
    }
}

fn inspect() {
    if let Some(host) = active_browser().and_then(|browser| browser.host()) {
        host.show_dev_tools(None, None, None, None);
    }
}

fn perform_browser_action(action: &Action) {
    if let Some(browser) = active_browser() {
        match action {
            Action::Back => browser.go_back(),
            Action::Forward => browser.go_forward(),
            Action::Reload => browser.reload(),
            Action::Close => {
                if let Some(host) = browser.host() {
                    host.close_browser(0);
                }
            }
            _ => {}
        }
    }
}

fn open_tool(url: &str) {
    let identity = with_host(|host| {
        host.session
            .active_tab()
            .and_then(|id| host.session.tab_identity(id).ok())
    })
    .flatten()
    .unwrap_or(0);
    let base = url.split('#').next().unwrap_or(url);
    let existing = with_host(|host| {
        host.session
            .summaries()
            .into_iter()
            .find(|tab| {
                host.session.tab_identity(tab.target.id).ok() == Some(identity)
                    && tab.url.split('#').next() == Some(base)
            })
            .map(|tab| (tab.target.id, tab.url))
    })
    .flatten();
    if let Some((id, current)) = existing {
        activate(id);
        // Reusing an editor preserves unsaved API input. Fragment navigation
        // selects a feature without reloading the embedded document.
        if url.contains('#') && current != url {
            if let Some(frame) = browser(id).and_then(|browser| browser.main_frame()) {
                frame.load_url(Some(&CefString::from(url)));
            }
        }
    } else if let Err(error) = new_tab(identity, url) {
        crate::thor_warn!("thor::cef", "Open tool failed: {error}");
    }
}
