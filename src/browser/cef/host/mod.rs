use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use ::cef::*;

use crate::browser::session::BrowserSession;

mod actions;
mod commands;
mod devtools;
mod palette;
mod theme;
mod workspace;

use crate::workbench::HOME_URL;
use ::cef::wrapper::message_router::{BrowserSideRouter, MessageRouterBrowserSideHandlerCallbacks};
use actions::perform;
use std::sync::Arc;

struct Host {
    session: BrowserSession,
    cache_root: PathBuf,
    window: Option<Window>,
    content: Option<Panel>,
    address: Option<Textfield>,
    back: Option<LabelButton>,
    forward: Option<LabelButton>,
    tools: Option<Arc<BrowserSideRouter>>,
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
            address: None,
            back: None,
            forward: None,
            tools: None,
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
            workspace::register_context(&context)?;
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
        address.set_text(Some(&CefString::from(display_address(&url))));
    }
    refresh_controls();
    commands::publish();
}

fn display_address(url: &str) -> &str {
    if url == HOME_URL {
        ""
    } else {
        url
    }
}

fn refresh_controls() {
    let current = active_browser();
    with_host(|host| {
        if let Some(back) = &host.back {
            back.set_enabled(current.as_ref().map_or(0, |browser| browser.can_go_back()));
        }
        if let Some(forward) = &host.forward {
            forward.set_enabled(
                current
                    .as_ref()
                    .map_or(0, |browser| browser.can_go_forward()),
            );
        }
        if let Some(address) = &host.address {
            let identity = host
                .session
                .active_tab()
                .and_then(|id| host.session.tab_identity(id).ok())
                .and_then(|id| host.session.identity(id).ok())
                .map(|identity| identity.name.as_str())
                .unwrap_or("Default");
            address.set_accessible_name(Some(&CefString::from(
                format!("Address — {identity} identity").as_str(),
            )));
        }
    });
}

#[derive(Clone)]
pub(super) enum Action {
    Back,
    Forward,
    Reload,
    New,
    Close,
    Home,
    Inspect,
    FocusAddress,
    NextTab,
    PreviousTab,
    Sites,
    Toolbox,
    OpenTool(String),
}

fn add_button(panel: &Panel, title: &str, action: Action) -> Option<LabelButton> {
    let mut delegate = ControlDelegate::new(action);
    let button = label_button_create(Some(&mut delegate), Some(&CefString::from(title)))?;
    button.set_accessible_name(Some(&CefString::from(title)));
    button.set_tooltip_text(Some(&CefString::from(title)));
    button.set_font_list(Some(&CefString::from("Arial, 12px")));
    button.set_enabled_text_colors(theme::TEXT);
    button.set_text_color(ButtonState::DISABLED, theme::MUTED);
    button.set_minimum_size(Some(&Size {
        width: 44,
        height: 34,
    }));
    button.set_background_color(theme::BG);
    panel.add_child_view(Some(&mut View::from(&button)));
    Some(button)
}

fn add_icon(panel: &Panel, title: &str, icon: theme::Icon, action: Action) -> Option<LabelButton> {
    let button = add_button(panel, "", action)?;
    button.set_accessible_name(Some(&CefString::from(title)));
    button.set_tooltip_text(Some(&CefString::from(title)));
    button.set_minimum_size(Some(&Size {
        width: 32,
        height: 34,
    }));
    button.set_maximum_size(Some(&Size {
        width: 32,
        height: 34,
    }));
    for state in [
        ButtonState::NORMAL,
        ButtonState::HOVERED,
        ButtonState::PRESSED,
        ButtonState::DISABLED,
    ] {
        let color = if state == ButtonState::DISABLED {
            0xff687787
        } else {
            theme::TEXT
        };
        if let Some(mut image) = theme::icon(icon, color) {
            button.set_image(state, Some(&mut image));
        }
    }
    Some(button)
}

fn shortcut_action(event: &KeyEvent) -> Option<Action> {
    if !matches!(
        event.type_,
        KeyEventType::RAWKEYDOWN | KeyEventType::KEYDOWN
    ) {
        return None;
    }
    let modifier = if cfg!(target_os = "macos") {
        sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0
    } else {
        sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0
    };
    let control = sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;
    let shift = sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0;
    if event.windows_key_code == 9 && event.modifiers & control != 0 {
        return Some(if event.modifiers & shift != 0 {
            Action::PreviousTab
        } else {
            Action::NextTab
        });
    }
    if event.modifiers & modifier == 0 {
        return None;
    }
    match event.windows_key_code {
        75 if event.modifiers & shift != 0 => Some(Action::Toolbox),
        75 => Some(Action::Sites),
        76 => Some(Action::FocusAddress),
        219 if event.modifiers & shift != 0 => Some(Action::PreviousTab),
        221 if event.modifiers & shift != 0 => Some(Action::NextTab),
        73 if event.modifiers & sys::cef_event_flags_t::EVENTFLAG_ALT_DOWN.0 != 0 => {
            Some(Action::Inspect)
        }
        84 => Some(Action::New),
        87 => Some(Action::Close),
        82 => Some(Action::Reload),
        _ => None,
    }
}

fn shortcut(event: &KeyEvent) -> i32 {
    if palette::key(event) == 1 {
        return 1;
    }
    let Some(action) = shortcut_action(event) else {
        return 0;
    };
    // Native delegates may own the control being closed. Finish the input
    // callback before changing its view hierarchy or closing its browser.
    i32::from(queue_action(action))
}

pub(super) fn queue_action(action: Action) -> bool {
    let mut task = ShortcutTask::new(action);
    post_task(ThreadId::UI, Some(&mut task)) == 1
}

wrap_task! {
    struct ShortcutTask {action:Action}
    impl Task {fn execute(&self){perform(&self.action);}}
}

fn create_window(window: &Window) -> anyhow::Result<()> {
    theme::apply(window);
    let root = panel_create(None).ok_or_else(|| anyhow::anyhow!("CEF root panel unavailable"))?;
    root.set_background_color(theme::BG);
    let layout = root
        .set_to_box_layout(Some(&BoxLayoutSettings {
            cross_axis_alignment: AxisAlignment::STRETCH,
            ..Default::default()
        }))
        .ok_or_else(|| anyhow::anyhow!("CEF root layout unavailable"))?;
    let toolbar = panel_create(None).ok_or_else(|| anyhow::anyhow!("CEF toolbar unavailable"))?;
    toolbar.set_background_color(theme::BG);
    let toolbar_layout = toolbar
        .set_to_box_layout(Some(&BoxLayoutSettings {
            horizontal: 1,
            minimum_cross_axis_size: 46,
            between_child_spacing: 4,
            inside_border_horizontal_spacing: 10,
            inside_border_vertical_spacing: 6,
            cross_axis_alignment: AxisAlignment::CENTER,
            ..Default::default()
        }))
        .ok_or_else(|| anyhow::anyhow!("CEF toolbar layout unavailable"))?;
    let back = add_icon(&toolbar, "Back", theme::Icon::Back, Action::Back);
    let forward = add_icon(&toolbar, "Forward", theme::Icon::Forward, Action::Forward);
    add_icon(&toolbar, "Reload", theme::Icon::Reload, Action::Reload);
    add_icon(&toolbar, "Workspace", theme::Icon::Home, Action::Home);
    let mut delegate = AddressDelegate::new();
    let address = textfield_create(Some(&mut delegate))
        .ok_or_else(|| anyhow::anyhow!("CEF omnibox unavailable"))?;
    address.set_accessible_name(Some(&CefString::from("Address")));
    address.set_placeholder_text(Some(&CefString::from("Enter a URL or localhost address")));
    address.set_font_list(Some(&CefString::from("Arial, 13px")));
    address.set_background_color(theme::INSET);
    address.set_size(Some(&Size {
        width: 400,
        height: 34,
    }));
    toolbar.add_child_view(Some(&mut View::from(&address)));
    toolbar_layout.set_flex_for_view(Some(&mut View::from(&address)), 1);
    add_button(&toolbar, "Developer", Action::Toolbox);
    let content =
        panel_create(None).ok_or_else(|| anyhow::anyhow!("CEF content panel unavailable"))?;
    content.set_background_color(theme::BG);
    content.set_to_fill_layout();
    for mut view in [View::from(&toolbar), View::from(&content)] {
        root.add_child_view(Some(&mut view));
    }
    layout.set_flex_for_view(Some(&mut View::from(&content)), 1);
    window.set_to_fill_layout();
    window.add_child_view(Some(&mut View::from(&root)));
    let title = if std::env::args_os().any(|arg| arg == "--developer-mode") {
        "Thor - Developer Mode"
    } else {
        "Thor"
    };
    window.set_title(Some(&CefString::from(title)));
    with_host(|host| {
        host.window = Some(window.clone());
        host.content = Some(content);
        host.address = Some(address);
        host.back = back;
        host.forward = forward;
    });
    new_tab(0, HOME_URL)?;
    window.show();
    window.activate();
    Ok(())
}

pub(super) fn close_window() {
    palette::close();
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
        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            if let Some(registrar) = registrar { registrar.add_custom_scheme(Some(&CefString::from("thor")), (SchemeOptions::STANDARD.get_raw() | SchemeOptions::SECURE.get_raw() | SchemeOptions::DISPLAY_ISOLATED.get_raw()) as i32); }
        }
        fn render_process_handler(&self) -> Option<RenderProcessHandler> { Some(workspace::WorkspaceRenderer::new()) }
        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ProcessHandler::new())
        }
    }
}

wrap_browser_process_handler! {
    struct ProcessHandler;
    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            match workspace::start() {
                Ok(tools) => { with_host(|host| host.tools = Some(tools)); }
                Err(error) => { crate::thor_warn!("thor::cef", "Workspace unavailable: {error}"); }
            }
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
    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size { Size { width: 1280, height: 840 } }
        fn minimum_size(&self, _view: Option<&mut View>) -> Size { Size { width: 720, height: 540 } }
    }
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
        fn on_window_bounds_changed(&self, _window: Option<&mut Window>, _bounds: Option<&Rect>) { palette::refresh(); }
        fn on_key_event(&self, _window: Option<&mut Window>, event: Option<&KeyEvent>) -> i32 { event.map_or(0, shortcut) }
        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> i32 { 1 }
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
        fn request_handler(&self) -> Option<RequestHandler> { Some(WorkspaceRequest::new()) }
        fn load_handler(&self) -> Option<LoadHandler> { Some(NavigationState::new()) }
        fn keyboard_handler(&self) -> Option<KeyboardHandler> { Some(BrowserKeys::new()) }
        fn on_process_message_received(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, source_process: ProcessId, message: Option<&mut ProcessMessage>) -> i32 {
            with_host(|host| host.tools.clone()).flatten().map_or(0, |router| i32::from(router.on_process_message_received(browser.cloned(), frame.cloned(), source_process, message.cloned())))
        }
    }
}

wrap_life_span_handler! {
    struct TabLifetime { id: u64 }
    impl LifeSpanHandler {
        fn do_close(&self, _browser: Option<&mut Browser>) -> i32 {
            // A tab owns a BrowserView, not the top-level window. Tear down that
            // view once CEF has accepted the close; OnBeforeClose owns cleanup.
            if let Some((Some(panel), Some(view))) = with_host(|host| {
                (host.content.clone(), host.views.remove(&self.id))
            }) {
                panel.remove_child_view(Some(&mut View::from(&view)));
            }
            1
        }
        fn on_before_close(&self, browser: Option<&mut Browser>) {
            if let Some(router) = with_host(|host| host.tools.clone()).flatten() { router.on_before_close(browser.cloned()); }
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
            if let Some(address) = address { address.set_text(Some(&CefString::from(display_address(&url)))); }
            refresh_controls();
            commands::publish();
        }
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title { with_host(|host| {
                let _ = host.session.update_title(self.id, &title.to_string());
            }); }
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
            if shortcut(event) == 1 { return 1; }
            if !matches!(event.type_, KeyEventType::RAWKEYDOWN | KeyEventType::KEYDOWN) || event.windows_key_code != 13 { return 0; }
            let url = CefString::from(&textfield.text()).to_string();
            let url = crate::workbench::navigation_url(&url);
            if let Some(frame) = active_browser().and_then(|browser| browser.main_frame()) { frame.load_url(Some(&CefString::from(url.as_str()))); }
            1
        }
    }
}

wrap_request_handler! {
    struct WorkspaceRequest;
    impl RequestHandler {
        fn on_before_browse(&self, browser: Option<&mut Browser>, frame: Option<&mut Frame>, _request: Option<&mut Request>, _user_gesture: i32, _is_redirect: i32) -> i32 {
            if let Some(router) = with_host(|host| host.tools.clone()).flatten() { router.on_before_browse(browser.cloned(), frame.cloned()); }
            0
        }
        fn on_render_process_terminated(&self, browser: Option<&mut Browser>, _status: TerminationStatus, _error_code: i32, _error_string: Option<&CefString>) {
            if let Some(router) = with_host(|host| host.tools.clone()).flatten() { router.on_render_process_terminated(browser.cloned()); }
        }
    }
}

wrap_load_handler! {
    struct NavigationState;
    impl LoadHandler {
        fn on_loading_state_change(&self, _browser: Option<&mut Browser>, _is_loading: i32, _can_go_back: i32, _can_go_forward: i32) { refresh_controls(); }
    }
}

wrap_keyboard_handler! {
    struct BrowserKeys;
    impl KeyboardHandler {
        fn on_pre_key_event(&self, browser: Option<&mut Browser>, event: Option<&KeyEvent>, _os_event: *mut u8, _is_keyboard_shortcut: Option<&mut i32>) -> i32 {
            let Some(event)=event else { return 0; };
            if shortcut(event)==1 { return 1; }
            if cfg!(target_os="macos") && matches!(event.type_,KeyEventType::KEYDOWN|KeyEventType::RAWKEYDOWN) && event.modifiers & sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0 != 0 {
                if let Some(frame)=browser.and_then(|browser|browser.focused_frame()) {
                    match event.windows_key_code {
                        65=>frame.select_all(), 67=>frame.copy(), 86=>frame.paste(), 88=>frame.cut(),
                        90 if event.modifiers & sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0 != 0=>frame.redo(),
                        90=>frame.undo(), _=>return 0,
                    }
                    return 1;
                }
            }
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_and_renderer_keydown_events_share_browser_shortcuts() {
        let modifier = if cfg!(target_os = "macos") {
            sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0
        } else {
            sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0
        };
        for type_ in [KeyEventType::KEYDOWN, KeyEventType::RAWKEYDOWN] {
            let event = KeyEvent {
                type_,
                modifiers: modifier,
                windows_key_code: 84,
                ..Default::default()
            };
            assert!(matches!(shortcut_action(&event), Some(Action::New)));
            let event = KeyEvent {
                modifiers: modifier | sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0,
                windows_key_code: 219,
                ..event
            };
            assert!(matches!(shortcut_action(&event), Some(Action::PreviousTab)));
            let event = KeyEvent {
                modifiers: 0,
                ..event
            };
            assert!(shortcut_action(&event).is_none());
            let event = KeyEvent {
                modifiers: modifier,
                windows_key_code: 75,
                ..event
            };
            assert!(matches!(shortcut_action(&event), Some(Action::Sites)));
            let event = KeyEvent {
                modifiers: modifier | sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0,
                ..event
            };
            assert!(matches!(shortcut_action(&event), Some(Action::Toolbox)));
        }
        let event = KeyEvent {
            type_: KeyEventType::KEYUP,
            modifiers: modifier,
            windows_key_code: 84,
            ..Default::default()
        };
        assert!(shortcut_action(&event).is_none());
    }
}
