//! Chromium dual-window shell entry — replaces WebKit content path in `run()`.
//! Kept as a module so `mod.rs` helpers can migrate incrementally.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::WindowBuilder,
};

use crate::app_state::AppState;
use crate::dx::bookmarks::BookmarkStore;
use crate::dx::cmd_palette::{
    cmd_k_payload_json, open_tabs_to_palette_items, settings_payload_json,
};
use crate::shell::chromium_tabs::ChromiumTabs;
use crate::shell::engine_status;

use super::{
    content_hotkey_script, normalize_omnibox_url, toggle_profile_mode, ToolbarCmd, UserEvent,
    CHROME_HEIGHT, TAB_GC_INTERVAL, TAB_IDLE_TTL,
};

#[cfg(target_os = "macos")]
use super::{
    open_tool_in_omnibar_chromium, run_omnibar_tool_go_chromium, setup_macos_app_menu,
};

#[cfg(target_os = "macos")]
use super::native_toolbar;

pub fn run_chromium_shell() -> anyhow::Result<()> {
    println!("[Thor Shell] Initializing Chromium dual-window shell...");

    #[cfg(target_os = "macos")]
    setup_macos_app_menu();

    let _ = AppState::global();

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy: EventLoopProxy<UserEvent> = event_loop.create_proxy();

    let builder = {
        #[cfg(target_os = "macos")]
        {
            use tao::dpi::LogicalPosition;
            use tao::platform::macos::WindowBuilderExtMacOS;
            WindowBuilder::new()
                .with_title("Thor — Developer Browser")
                .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0))
                .with_decorations(true)
                .with_fullsize_content_view(true)
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_traffic_light_inset(LogicalPosition::new(14.0, (52.0 - 12.0) / 2.0))
        }
        #[cfg(not(target_os = "macos"))]
        {
            WindowBuilder::new()
                .with_title("Thor — Developer Browser")
                .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0))
                .with_decorations(true)
        }
    };

    let window = Rc::new(builder.build(&event_loop)?);

    let tabs = Rc::new(RefCell::new(ChromiumTabs::launch(content_hotkey_script())?));
    {
        let mut t = tabs.borrow_mut();
        t.create_tab("default_tab", "https://www.google.com")
            .map_err(|e| anyhow::anyhow!("initial Chromium tab: {e}"))?;
        println!("⚡ [Thor] Initial Chromium tab ready (dual-window)");
    }

    #[cfg(target_os = "macos")]
    let toolbar_ref: Rc<RefCell<Option<native_toolbar::NativeToolbar>>> =
        Rc::new(RefCell::new(None));

    let scale_factor = window.scale_factor();
    let initial_logical = window.inner_size().to_logical::<f64>(scale_factor);
    let win_size: Rc<RefCell<(f64, f64)>> =
        Rc::new(RefCell::new((initial_logical.width, initial_logical.height)));

    #[cfg(target_os = "macos")]
    {
        let tb = native_toolbar::NativeToolbar::attach(&window, proxy.clone());
        let initial_url = tabs
            .borrow()
            .resolved_url("default_tab");
        if initial_url.is_empty() {
            tb.set_url("https://www.google.com");
        } else {
            tb.set_url(&initial_url);
        }
        tb.focus_omnibox();
        *toolbar_ref.borrow_mut() = Some(tb);
    }

    let win_size_for_resize = win_size.clone();
    let tabs_for_loop = tabs.clone();
    #[cfg(target_os = "macos")]
    let toolbar_for_loop = toolbar_ref.clone();
    let window_for_events = window.clone();
    let proxy_for_loop = proxy.clone();
    let mut next_gc_at = Instant::now() + TAB_GC_INTERVAL;
    let mut next_ipc_at = Instant::now() + Duration::from_millis(250);

    println!("============================================================");
    println!("🚀 [Thor Shell] AppKit chrome + Chromium content (dual-window M2)");
    println!("🚀 [Thor Shell] ⌘K / ⌘⇧K via CDP inject · omnibox drives Chromium");
    println!("============================================================");
    let _ = CHROME_HEIGHT;

    event_loop.run(move |event, _, control_flow| {
        let wake = next_gc_at.min(next_ipc_at);
        *control_flow = ControlFlow::WaitUntil(wake);

        match event {
            Event::UserEvent(UserEvent::Exit) => {
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::OpenCmdK)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::OpenCmdK)) => {
                open_cmd_k_chromium(&tabs_for_loop.borrow());
            }
            Event::UserEvent(UserEvent::OpenCmdShiftK)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::OpenCmdShiftK)) => {
                open_cmd_shift_k_chromium(&tabs_for_loop.borrow());
            }
            Event::UserEvent(UserEvent::LocustToggle)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::LocustToggle)) => {
                #[cfg(target_os = "macos")]
                {
                    let tabs_slot = tabs_for_loop.borrow();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    open_tool_in_omnibar_chromium(&tabs_slot, &mut tb, "locust_active_origin");
                }
            }
            Event::UserEvent(UserEvent::OpenTool(action_key)) => {
                #[cfg(target_os = "macos")]
                {
                    let tabs_slot = tabs_for_loop.borrow();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    open_tool_in_omnibar_chromium(&tabs_slot, &mut tb, &action_key);
                }
            }
            Event::UserEvent(UserEvent::Navigate(url)) => {
                let url = normalize_omnibox_url(&url);
                if url.is_empty() {
                    return;
                }
                match tabs_for_loop.borrow_mut().navigate_active(&url) {
                    Ok(clean) => {
                        println!("⚡ [Thor] navigate → {clean}");
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_url(&clean);
                        }
                    }
                    Err(e) => eprintln!("⚡ [Thor] navigate failed: {e}"),
                }
            }
            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::ToolGo)) => {
                #[cfg(target_os = "macos")]
                {
                    let mut tabs_slot = tabs_for_loop.borrow_mut();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    let done = run_omnibar_tool_go_chromium(&mut tabs_slot, &mut tb);
                    if done {
                        if let Some(t) = tb.as_mut() {
                            t.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                            t.focus_omnibox();
                        }
                    }
                }
            }
            Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::ToolCancel)) => {
                #[cfg(target_os = "macos")]
                if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                    if !tb.tool_mode().is_browse() {
                        tb.set_tool_mode(native_toolbar::ChromeToolMode::Browse);
                        tb.focus_omnibox();
                    }
                }
            }
            Event::UserEvent(UserEvent::WebVitalsResult(summary)) => {
                #[cfg(target_os = "macos")]
                if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                    tb.set_status(&summary);
                    println!("⚡ [Thor Vitals] {summary}");
                }
            }
            Event::UserEvent(UserEvent::ToggleProfile)
            | Event::UserEvent(UserEvent::Toolbar(ToolbarCmd::ToggleProfile)) => {
                #[cfg(target_os = "macos")]
                {
                    let tabs_slot = tabs_for_loop.borrow();
                    let mut tb = toolbar_for_loop.borrow_mut();
                    open_tool_in_omnibar_chromium(&tabs_slot, &mut tb, "toggle_profile");
                }
                #[cfg(not(target_os = "macos"))]
                toggle_profile_mode();
            }
            Event::UserEvent(UserEvent::FocusOmnibox) => {
                #[cfg(target_os = "macos")]
                if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                    tb.focus_omnibox();
                }
            }
            Event::UserEvent(UserEvent::ClosePalette) => {}
            Event::UserEvent(UserEvent::SwitchTab(target)) => {
                let url = {
                    let mut tabs_slot = tabs_for_loop.borrow_mut();
                    tabs_slot.switch_tab(&target)
                };
                #[cfg(target_os = "macos")]
                if let Some(url) = url {
                    if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                        tb.set_url(&url);
                        tb.raise();
                    }
                }
                println!("⚡ [Thor] switch tab → {target}");
            }
            Event::UserEvent(UserEvent::CloseActiveTab) => {
                let url = {
                    let mut tabs_slot = tabs_for_loop.borrow_mut();
                    if tabs_slot.sessions.len() <= 1 {
                        tabs_slot.reload_active();
                        None
                    } else {
                        let closing = tabs_slot.active.clone();
                        println!("⚡ [Thor] close tab {closing}");
                        tabs_slot.close_tab(&closing);
                        Some(tabs_slot.active_url())
                    }
                };
                #[cfg(target_os = "macos")]
                if let Some(url) = url {
                    if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                        tb.set_url(&url);
                    }
                }
            }
            Event::UserEvent(UserEvent::Toolbar(cmd)) => {
                let (w, h) = *win_size_for_resize.borrow();
                match cmd {
                    ToolbarCmd::OpenCmdK
                    | ToolbarCmd::OpenCmdShiftK
                    | ToolbarCmd::LocustToggle
                    | ToolbarCmd::ToolGo
                    | ToolbarCmd::ToolCancel
                    | ToolbarCmd::ToggleProfile => {}
                    ToolbarCmd::Back => tabs_for_loop.borrow().back_active(),
                    ToolbarCmd::Forward => tabs_for_loop.borrow().forward_active(),
                    ToolbarCmd::Reload => tabs_for_loop.borrow().reload_active(),
                    ToolbarCmd::NewTab => {
                        let tab_id = format!("tab_{}", Instant::now().elapsed().as_nanos());
                        let url = "https://www.google.com";
                        match tabs_for_loop.borrow_mut().create_tab(&tab_id, url) {
                            Ok(()) => {
                                #[cfg(target_os = "macos")]
                                if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                                    tb.set_url(url);
                                    tb.reflow(w, h);
                                }
                                println!("⚡ [Thor] new Chromium tab {tab_id}");
                            }
                            Err(e) => eprintln!("⚡ [Thor] new_tab failed: {e}"),
                        }
                    }
                    ToolbarCmd::SubmitOmnibox => {
                        #[cfg(target_os = "macos")]
                        {
                            let raw = toolbar_for_loop
                                .borrow()
                                .as_ref()
                                .map(|t| t.read_url())
                                .unwrap_or_default();
                            let url = normalize_omnibox_url(&raw);
                            if url.is_empty() {
                                return;
                            }
                            match tabs_for_loop.borrow_mut().navigate_active(&url) {
                                Ok(clean) => {
                                    println!("⚡ [Thor] navigate → {clean}");
                                    if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                                        tb.set_url(&clean);
                                    }
                                }
                                Err(e) => eprintln!("⚡ [Thor] navigate failed: {e}"),
                            }
                        }
                    }
                }
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. })
            | Event::NewEvents(StartCause::Init) => {
                let now = Instant::now();
                if now >= next_ipc_at {
                    next_ipc_at = now + Duration::from_millis(250);
                    let msgs = tabs_for_loop.borrow().drain_ipc_queue();
                    for msg in msgs {
                        dispatch_chromium_ipc(&msg, &proxy_for_loop);
                    }
                }
                if now >= next_gc_at {
                    next_gc_at = now + TAB_GC_INTERVAL;
                    tabs_for_loop.borrow_mut().gc_idle(TAB_IDLE_TTL);
                }
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(new_size) => {
                    let scale_factor = window_for_events.scale_factor();
                    let logical_size = new_size.to_logical::<f64>(scale_factor);
                    let w = logical_size.width;
                    let h = logical_size.height;
                    *win_size_for_resize.borrow_mut() = (w, h);
                    #[cfg(target_os = "macos")]
                    if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                        tb.reflow(w, h);
                        tb.raise();
                    }
                }
                _ => {}
            },
            _ => {}
        }
    });
}

fn open_cmd_k_chromium(tabs: &ChromiumTabs) {
    let bookmarks = BookmarkStore::open()
        .map(|s| s.to_palette_items())
        .unwrap_or_default();
    let open_refs = tabs.open_tab_palette_refs();
    let open_items = open_tabs_to_palette_items(&open_refs);
    let history = AppState::global()
        .tab_history()
        .palette_items_for_tab(&tabs.active);
    let Ok(payload) = cmd_k_payload_json(&bookmarks, &history, &open_items) else {
        return;
    };
    let status = engine_status::engine_status_script();
    let script = format!(
        "{status}; window.__thorCmdKPayload = {payload}; if (window.openThorModal) window.openThorModal('nav');"
    );
    let _ = tabs.evaluate_active(&script);
    println!("⚡ [Thor] Opening ⌘K on Chromium tab {}", tabs.active);
}

fn open_cmd_shift_k_chromium(tabs: &ChromiumTabs) {
    let Ok(payload) = settings_payload_json() else {
        return;
    };
    let status = engine_status::engine_status_script();
    let script = format!(
        "{status}; window.__thorSettingsPayload = {payload}; if (window.openThorModal) window.openThorModal('settings');"
    );
    let _ = tabs.evaluate_active(&script);
    println!("⚡ [Thor] Opening ⌘⇧K on Chromium tab {}", tabs.active);
}

fn dispatch_chromium_ipc(msg: &str, proxy: &EventLoopProxy<UserEvent>) {
    let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
    let Some(cmd) = payload["cmd"].as_str() else {
        return;
    };
    match cmd {
        "hotkey_cmd_k" | "open_cmd_k" => {
            let _ = proxy.send_event(UserEvent::OpenCmdK);
        }
        "hotkey_cmd_shift_k" | "open_cmd_shift_k" => {
            let _ = proxy.send_event(UserEvent::OpenCmdShiftK);
        }
        "hotkey_locust_toggle" | "locust_toggle_active" => {
            let _ = proxy.send_event(UserEvent::LocustToggle);
        }
        "hotkey_toggle_profile" | "toggle_profile" => {
            let _ = proxy.send_event(UserEvent::ToggleProfile);
        }
        "hotkey_new_tab" => {
            let _ = proxy.send_event(UserEvent::Toolbar(ToolbarCmd::NewTab));
        }
        "hotkey_close_tab" => {
            let _ = proxy.send_event(UserEvent::CloseActiveTab);
        }
        "hotkey_reload" | "reload" => {
            let _ = proxy.send_event(UserEvent::Toolbar(ToolbarCmd::Reload));
        }
        "hotkey_focus_url" => {
            // Forward to the event loop so the AppKit toolbar can be focused
            // and text selected (Chrome-style ⌘L / click omnibox behavior).
            let _ = proxy.send_event(UserEvent::FocusOmnibox);
        }
        "switch_tab" => {
            if let Some(id) = payload["id"].as_str() {
                let _ = proxy.send_event(UserEvent::SwitchTab(id.to_string()));
            }
        }
        "open_tool" => {
            if let Some(key) = payload["action_key"].as_str() {
                let _ = proxy.send_event(UserEvent::OpenTool(key.to_string()));
            }
        }
        "navigate" => {
            if let Some(url) = payload["url"].as_str() {
                let _ = proxy.send_event(UserEvent::Navigate(url.to_string()));
            }
        }
        "web_vitals_result" => {
            let summary = payload["summary"].as_str().unwrap_or("no vitals").to_string();
            let _ = proxy.send_event(UserEvent::WebVitalsResult(summary));
        }
        "back" => {
            let _ = proxy.send_event(UserEvent::Toolbar(ToolbarCmd::Back));
        }
        "forward" => {
            let _ = proxy.send_event(UserEvent::Toolbar(ToolbarCmd::Forward));
        }
        "toggle_devtools" => {
            let _ = proxy.send_event(UserEvent::OpenTool("toggle_devtools".into()));
        }
        "log" => {
            if let Some(m) = payload["message"].as_str() {
                println!("⚡ [Thor Chromium Log] {m}");
            }
        }
        _ => {}
    }
}
