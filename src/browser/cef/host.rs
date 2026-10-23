use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use ::cef::*;

use crate::browser::session::BrowserSession;

mod commands;
mod devtools;

struct Host {
    session: BrowserSession,
    cache_root: PathBuf,
    window: Option<Window>,
    content: Option<Panel>,
    tabs: Option<Panel>,
    address: Option<Textfield>,
    contexts: HashMap<u64, RequestContext>,
    views: HashMap<u64, BrowserView>,
    closing: bool,
    snapshots:
        Option<tokio::sync::watch::Sender<crate::engine::engine_coordinator::BrowserSnapshot>>,
    devtools: devtools::DevTools,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

pub(super) fn prepare(cache_root: PathBuf) {
    HOST.with(|cell| {
        *cell.borrow_mut() = Some(Host {
            session: BrowserSession::default(),
            cache_root,
            window: None,
            content: None,
            tabs: None,
            address: None,
            contexts: HashMap::new(),
            views: HashMap::new(),
            closing: false,
            snapshots: None,
            devtools: devtools::DevTools::default(),
        })
    });
}

pub(super) fn release() {
    let host = HOST.with(|cell| cell.borrow_mut().take());
    drop(host);
    crate::AppState::global().tab_history().clear_all_history();
}

fn with_host<T>(f: impl FnOnce(&mut Host) -> T) -> Option<T> {
    HOST.with(|cell| cell.borrow_mut().as_mut().map(f))
}

fn browser(id: u64) -> Option<Browser> {
    with_host(|host| host.views.get(&id).and_then(|view| view.browser())).flatten()
}

fn active_browser() -> Option<Browser> {
    with_host(|host| host.session.active_tab())
        .flatten()
        .and_then(browser)
}

fn new_tab(identity: u64, url: &str) -> anyhow::Result<u64> {
    let (id, mut context, content) = with_host(|host| -> anyhow::Result<_> {
        let disposable = host.session.identity(identity)?.disposable;
        if !host.contexts.contains_key(&identity) {
            let cache = if disposable {
                String::new()
            } else {
                host.cache_root
                    .join("Identities")
                    .join(identity.to_string())
                    .to_string_lossy()
                    .into_owned()
            };
            let settings = RequestContextSettings {
                cache_path: CefString::from(cache.as_str()),
                persist_session_cookies: i32::from(!disposable),
                ..Default::default()
            };
            let context = request_context_create_context(Some(&settings), None)
                .ok_or_else(|| anyhow::anyhow!("CEF identity context creation failed"))?;
            host.contexts.insert(identity, context);
        }
        let content = host
            .content
            .clone()
            .ok_or_else(|| anyhow::anyhow!("CEF window unavailable"))?;
        let id = host.session.open_tab(identity)?;
        Ok((id, host.contexts[&identity].clone(), content))
    })
    .ok_or_else(|| anyhow::anyhow!("CEF host unavailable"))??;
    let mut client = TabClient::new(id);
    let mut delegate = TabViewDelegate::new(id);
    let view = browser_view_create(
        Some(&mut client),
        Some(&CefString::from(url)),
        Some(&BrowserSettings::default()),
        None,
        Some(&mut context),
        Some(&mut delegate),
    );
    let Some(view) = view else {
        with_host(|host| host.session.close_tab(id));
        anyhow::bail!("CEF browser creation failed");
    };
    with_host(|host| host.views.insert(id, view.clone()));
    content.add_child_view(Some(&mut View::from(&view)));
    activate(id);
    Ok(id)
}

fn activate(id: u64) {
    let Some(Some((views, address, url))) = with_host(|host| {
        host.session.activate(id).ok()?;
        let views: Vec<_> = host
            .views
            .iter()
            .map(|(key, view)| (*key, view.clone()))
            .collect();
        let url = host
            .session
            .history(id)
            .ok()?
            .back()
            .map(|entry| entry.url.clone())
            .unwrap_or_default();
        Some((views, host.address.clone(), url))
    }) else {
        return;
    };
    for (key, view) in views {
        view.set_visible(i32::from(key == id));
        if key == id {
            view.request_focus();
        }
    }
    if let Some(address) = address {
        address.set_text(Some(&CefString::from(url.as_str())));
    }
    refresh_tabs();
    commands::publish();
}

fn refresh_tabs() {
    let Some(Some((panel, tabs))) = with_host(|host| {
        let panel = host.tabs.clone()?;
        let tabs: Vec<_> = host
            .session
            .tab_ids()
            .into_iter()
            .map(|id| {
                let label = host
                    .session
                    .history(id)
                    .ok()
                    .and_then(|history| history.back())
                    .map(|entry| entry.title.clone())
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| "New tab".into());
                (id, label)
            })
            .collect();
        Some((panel, tabs))
    }) else {
        return;
    };
    panel.remove_all_child_views();
    for (id, title) in tabs {
        add_button(&panel, &title, Action::Select(id));
    }
    add_button(&panel, "New tab", Action::New);
    panel.layout();
}

#[derive(Clone)]
enum Action {
    Back,
    Forward,
    Reload,
    New,
    Close,
    Select(u64),
}

fn perform(action: &Action) {
    match action {
        Action::New => {
            if let Err(error) = new_tab(0, "about:blank") {
                crate::thor_warn!("thor::cef", "New tab failed: {error}");
            }
        }
        Action::Select(id) => activate(*id),
        _ => {
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
    }
}

fn add_button(panel: &Panel, title: &str, action: Action) {
    let mut delegate = ControlDelegate::new(action);
    if let Some(button) = label_button_create(Some(&mut delegate), Some(&CefString::from(title))) {
        button.set_accessible_name(Some(&CefString::from(title)));
        panel.add_child_view(Some(&mut View::from(&button)));
    }
}

fn create_window(window: &Window) -> anyhow::Result<()> {
    let root = panel_create(None).ok_or_else(|| anyhow::anyhow!("CEF root panel unavailable"))?;
    let layout = root
        .set_to_box_layout(Some(&BoxLayoutSettings::default()))
        .ok_or_else(|| anyhow::anyhow!("CEF root layout unavailable"))?;
    let tabs = panel_create(None).ok_or_else(|| anyhow::anyhow!("CEF tab strip unavailable"))?;
    tabs.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        minimum_cross_axis_size: 28,
        ..Default::default()
    }));
    let toolbar = panel_create(None).ok_or_else(|| anyhow::anyhow!("CEF toolbar unavailable"))?;
    let toolbar_layout = toolbar
        .set_to_box_layout(Some(&BoxLayoutSettings {
            horizontal: 1,
            minimum_cross_axis_size: 44,
            between_child_spacing: 6,
            inside_border_horizontal_spacing: 8,
            ..Default::default()
        }))
        .ok_or_else(|| anyhow::anyhow!("CEF toolbar layout unavailable"))?;
    for (label, action) in [
        ("Back", Action::Back),
        ("Forward", Action::Forward),
        ("Reload", Action::Reload),
    ] {
        add_button(&toolbar, label, action);
    }
    let mut delegate = AddressDelegate::new();
    let address = textfield_create(Some(&mut delegate))
        .ok_or_else(|| anyhow::anyhow!("CEF omnibox unavailable"))?;
    address.set_accessible_name(Some(&CefString::from("Address")));
    address.set_placeholder_text(Some(&CefString::from("Enter URL")));
    toolbar.add_child_view(Some(&mut View::from(&address)));
    toolbar_layout.set_flex_for_view(Some(&mut View::from(&address)), 1);
    add_button(&toolbar, "Close tab", Action::Close);
    let content =
        panel_create(None).ok_or_else(|| anyhow::anyhow!("CEF content panel unavailable"))?;
    content.set_to_fill_layout();
    for panel in [&tabs, &toolbar, &content] {
        root.add_child_view(Some(&mut View::from(panel)));
    }
    layout.set_flex_for_view(Some(&mut View::from(&content)), 1);
    window.set_to_fill_layout();
    window.add_child_view(Some(&mut View::from(&root)));
    window.set_title(Some(&CefString::from("Thor")));
    with_host(|host| {
        host.window = Some(window.clone());
        host.content = Some(content);
        host.tabs = Some(tabs);
        host.address = Some(address);
    });
    new_tab(0, "about:blank")?;
    window.show();
    Ok(())
}

pub(super) fn close_window() {
    let browsers = with_host(|host| {
        host.closing = true;
        host.views
            .values()
            .filter_map(|view| view.browser())
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    if browsers.is_empty() {
        quit_message_loop();
    }
    for browser in browsers {
        if let Some(host) = browser.host() {
            host.close_browser(0);
        }
    }
}

wrap_app! {
    pub(super) struct ThorApp;
    impl App {
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ProcessHandler::new())
        }
    }
}

wrap_browser_process_handler! {
    struct ProcessHandler;
    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            commands::start();
            let mut delegate = ThorWindowDelegate::new();
            if window_create_top_level(Some(&mut delegate)).is_none() {
                crate::thor_warn!("thor::cef", "CEF window creation failed");
                quit_message_loop();
            }
        }
    }
}

wrap_window_delegate! {
    struct ThorWindowDelegate {}
    impl ViewDelegate { fn preferred_size(&self, _view: Option<&mut View>) -> Size { Size { width: 1280, height: 840 } } }
    impl PanelDelegate {}
    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            if let Some(window) = window {
                if let Err(error) = create_window(window) {
                    crate::thor_warn!("thor::cef", "Window setup failed: {error}"); close_window();
                }
            }
        }
        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            if with_host(|host| host.views.is_empty()).unwrap_or(true) { return 1; }
            close_window(); 0
        }
        fn on_window_destroyed(&self, _window: Option<&mut Window>) { quit_message_loop(); }
        fn can_resize(&self, _window: Option<&mut Window>) -> i32 { 1 }
        fn can_maximize(&self, _window: Option<&mut Window>) -> i32 { 1 }
        fn can_minimize(&self, _window: Option<&mut Window>) -> i32 { 1 }
        fn window_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
    }
}

wrap_browser_view_delegate! {
    struct TabViewDelegate { id: u64 }
    impl ViewDelegate {}
    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle { RuntimeStyle::ALLOY }
        fn on_browser_created(&self, _view: Option<&mut BrowserView>, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                devtools::attach(self.id, browser);
            }
        }
    }
}

wrap_client! {
    struct TabClient { id: u64 }
    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> { Some(TabLifetime::new(self.id)) }
        fn display_handler(&self) -> Option<DisplayHandler> { Some(TabDisplay::new(self.id)) }
    }
}

wrap_life_span_handler! {
    struct TabLifetime { id: u64 }
    impl LifeSpanHandler {
        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            devtools::close(self.id);
            let removed = with_host(|host| {
                let view = host.views.remove(&self.id);
                if let Ok(Some(identity)) = host.session.close_tab(self.id) { host.contexts.remove(&identity); }
                (host.content.clone(), view, host.session.active_tab(), host.window.clone())
            });
            crate::AppState::global().tab_history().clear_tab_history(&self.id.to_string());
            if let Some((panel, view, active, window)) = removed {
                if let (Some(panel), Some(view)) = (panel, view) { panel.remove_child_view(Some(&mut View::from(&view))); }
                if let Some(active) = active { activate(active); } else if let Some(window) = window { window.close(); }
            }
        }
    }
}

wrap_display_handler! {
    struct TabDisplay { id: u64 }
    impl DisplayHandler {
        fn on_address_change(&self, _browser: Option<&mut Browser>, frame: Option<&mut Frame>, url: Option<&CefString>) {
            if frame.is_none_or(|frame| frame.is_main() == 0) { return; }
            let Some(url) = url else { return; };
            let url = url.to_string();
            let address = with_host(|host| {
                let _ = host.session.record_navigation(self.id, &url, "");
                (host.session.active_tab() == Some(self.id)).then(|| host.address.clone()).flatten()
            }).flatten();
            if let Some(address) = address { address.set_text(Some(&CefString::from(url.as_str()))); }
            commands::publish();
        }
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title { with_host(|host| {
                let _ = host.session.update_title(self.id, &title.to_string());
            }); refresh_tabs(); }
        }
    }
}

wrap_button_delegate! {
    struct ControlDelegate { action: Action }
    impl ViewDelegate {}
    impl ButtonDelegate { fn on_button_pressed(&self, _button: Option<&mut Button>) { perform(&self.action); } }
}

wrap_textfield_delegate! {
    struct AddressDelegate {}
    impl ViewDelegate {}
    impl TextfieldDelegate {
        fn on_key_event(&self, textfield: Option<&mut Textfield>, event: Option<&KeyEvent>) -> i32 {
            let (Some(textfield), Some(event)) = (textfield, event) else { return 0; };
            if event.type_ != KeyEventType::RAWKEYDOWN || event.windows_key_code != 13 { return 0; }
            let url = CefString::from(&textfield.text()).to_string();
            let url = if url.contains("://") || url.starts_with("about:") { url } else { format!("https://{url}") };
            if let Some(frame) = active_browser().and_then(|browser| browser.main_frame()) { frame.load_url(Some(&CefString::from(url.as_str()))); }
            1
        }
    }
}
