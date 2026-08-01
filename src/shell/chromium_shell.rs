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
    cmd_k_payload_json, downloads_payload_json, open_tabs_to_palette_items, settings_payload_json,
};
use crate::shell::chromium_tabs::ChromiumTabs;
use crate::shell::engine_status;

use super::{
    content_hotkey_script, normalize_omnibox_url, ToolbarCmd, UserEvent, CHROME_HEIGHT,
    TAB_GC_INTERVAL, TAB_IDLE_TTL,
};

#[cfg(not(target_os = "macos"))]
use super::toggle_profile_mode;

#[cfg(target_os = "macos")]
use super::{
    open_tool_in_omnibar_chromium, run_omnibar_tool_go_chromium, setup_macos_app_icon,
    setup_macos_app_menu,
};

#[cfg(target_os = "macos")]
use super::native_toolbar;

pub fn run_chromium_shell() -> anyhow::Result<()> {
    crate::thor_info!(
        "thor::shell",
        "[Thor Shell] Initializing Chromium dual-window shell..."
    );

    #[cfg(target_os = "macos")]
    {
        setup_macos_app_menu();
        setup_macos_app_icon();
    }

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
                .with_traffic_light_inset(LogicalPosition::new(14.0, 18.0))
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
        crate::thor_info!(
            "thor::shell",
            "⚡ [Thor] Initial Chromium tab ready (dual-window)"
        );
    }

    let runtime_for_mcp = tabs.borrow().runtime.clone();
    tokio::spawn(async move {
        let Ok((engine, handle)) =
            crate::engine::engine_coordinator::BrowserEngine::attach_existing(runtime_for_mcp)
                .await
        else {
            crate::thor_warn!(
                "thor::shell",
                "MCP engine could not adopt legacy Chromium tabs"
            );
            return;
        };
        AppState::global().set_engine_handle(handle.clone());
        tokio::spawn(engine.run());
        crate::mcp::server::start_mcp_server(handle).await;
    });

    #[cfg(target_os = "macos")]
    let toolbar_ref: Rc<RefCell<Option<native_toolbar::NativeToolbar>>> =
        Rc::new(RefCell::new(None));

    let scale_factor = window.scale_factor();
    let initial_logical = window.inner_size().to_logical::<f64>(scale_factor);
    let win_size: Rc<RefCell<(f64, f64)>> = Rc::new(RefCell::new((
        initial_logical.width,
        initial_logical.height,
    )));

    #[cfg(target_os = "macos")]
    {
        let tb = native_toolbar::NativeToolbar::attach(&window, proxy.clone());
        let initial_url = tabs.borrow().resolved_url("default_tab");
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

    crate::thor_info!(
        "thor::shell",
        "============================================================"
    );
    crate::thor_info!(
        "thor::shell",
        "🚀 [Thor Shell] AppKit chrome + Chromium content (dual-window M2)"
    );
    crate::thor_info!(
        "thor::shell",
        "🚀 [Thor Shell] ⌘K / ⌘⇧K via CDP inject · omnibox drives Chromium"
    );
    crate::thor_info!(
        "thor::shell",
        "============================================================"
    );

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
                if let Some(id) = action_key.strip_prefix("bitwarden_autofill_") {
                    match tabs_for_loop.borrow().autofill_login(id) {
                        Ok(msg) => {
                            println!("⚡ [Thor Vault] autofill ok — {msg}");
                            #[cfg(target_os = "macos")]
                            if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                                tb.set_status(&msg);
                            }
                        }
                        Err(e) => {
                            eprintln!("⚡ [Thor Vault] autofill failed: {e}");
                            #[cfg(target_os = "macos")]
                            if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                                tb.set_status(&format!("Vault: {e}"));
                            }
                        }
                    }
                } else if action_key == "bitwarden_lock" {
                    let _ = AppState::global().vault().lock();
                    #[cfg(target_os = "macos")]
                    if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                        tb.set_status("Vault locked");
                    }
                } else if action_key == "open_downloads" {
                    open_downloads_palette_chromium(&tabs_for_loop.borrow());
                } else if action_key == "toggle_interception" {
                    let on = !AppState::global().tamper().is_interception_enabled();
                    AppState::global().tamper().set_interception_enabled(on);
                    tabs_for_loop.borrow_mut().set_fetch_interception(on);
                    let msg = if on {
                        "Fetch intercept ON"
                    } else {
                        "Fetch intercept OFF"
                    };
                    println!("⚡ [Thor] {msg}");
                    #[cfg(target_os = "macos")]
                    if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                        tb.set_status(msg);
                    }
                } else if action_key == "list_intercepted" {
                    let pending = AppState::global().tamper().get_pending_requests();
                    let hist = AppState::global().tamper().get_history().len();
                    let msg = format!(
                        "intercept pending={} history={} bp={}",
                        pending.len(),
                        hist,
                        AppState::global().tamper().is_breakpoint_mode()
                    );
                    println!("⚡ [Thor] {msg}");
                    for p in pending.iter().take(5) {
                        println!("  • {} {} {}", p.method, p.url, p.id);
                    }
                    #[cfg(target_os = "macos")]
                    if let Some(tb) = toolbar_for_loop.borrow_mut().as_mut() {
                        tb.set_status(&msg);
                    }
                } else if let Some(id) = action_key.strip_prefix("reveal_download_") {
                    handle_download_action(id, true);
                } else if let Some(id) = action_key.strip_prefix("open_download_") {
                    handle_download_action(id, false);
                } else {
                    #[cfg(target_os = "macos")]
                    {
                        let tabs_slot = tabs_for_loop.borrow();
                        let mut tb = toolbar_for_loop.borrow_mut();
                        open_tool_in_omnibar_chromium(&tabs_slot, &mut tb, &action_key);
                    }
                }
            }
            Event::UserEvent(UserEvent::Navigate(url)) => {
                let url = normalize_omnibox_url(&url);
                if url.is_empty() {
                    return;
                }
                #[cfg(target_os = "macos")]
                if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                    tb.set_omnibox_loading(true);
                }
                match tabs_for_loop.borrow_mut().navigate_active(&url) {
                    Ok(clean) => {
                        println!("⚡ [Thor] navigate → {clean}");
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_url(&clean);
                            tb.set_omnibox_loading(false);
                        }
                        if let Some(report) = tabs_for_loop.borrow().run_visit_recon_active() {
                            println!(
                                "⚡ [Thor Recon] {} grade={} findings={}",
                                report.url,
                                report.grade,
                                report.findings.len()
                            );
                            let json =
                                serde_json::to_string(&report).unwrap_or_else(|_| "null".into());
                            let script = format!(
                                "window.__thorVisitRecon && window.__thorVisitRecon({json});"
                            );
                            let _ = tabs_for_loop.borrow().evaluate_active(&script);
                        }
                    }
                    Err(e) => {
                        eprintln!("⚡ [Thor] navigate failed: {e}");
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_omnibox_loading(false);
                        }
                    }
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
                    ToolbarCmd::Back => {
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_omnibox_loading(true);
                        }
                        tabs_for_loop.borrow().back_active();
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_omnibox_loading(false);
                        }
                    }
                    ToolbarCmd::Forward => {
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_omnibox_loading(true);
                        }
                        tabs_for_loop.borrow().forward_active();
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_omnibox_loading(false);
                        }
                    }
                    ToolbarCmd::Reload => {
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_omnibox_loading(true);
                        }
                        tabs_for_loop.borrow().reload_active();
                        #[cfg(target_os = "macos")]
                        if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                            tb.set_omnibox_loading(false);
                        }
                    }
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
                            if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                                tb.set_omnibox_loading(true);
                            }
                            match tabs_for_loop.borrow_mut().navigate_active(&url) {
                                Ok(clean) => {
                                    println!("⚡ [Thor] navigate → {clean}");
                                    if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                                        tb.set_url(&clean);
                                        tb.set_omnibox_loading(false);
                                    }
                                    if let Some(report) =
                                        tabs_for_loop.borrow().run_visit_recon_active()
                                    {
                                        println!(
                                            "⚡ [Thor Recon] {} grade={} findings={}",
                                            report.url,
                                            report.grade,
                                            report.findings.len()
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("⚡ [Thor] navigate failed: {e}");
                                    if let Some(tb) = toolbar_for_loop.borrow().as_ref() {
                                        tb.set_omnibox_loading(false);
                                    }
                                }
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
                    let msgs = {
                        let tabs = tabs_for_loop.borrow();
                        tabs.drain_ipc_queue()
                    };
                    for msg in msgs {
                        dispatch_chromium_ipc(
                            &msg,
                            &proxy_for_loop,
                            &mut tabs_for_loop.borrow_mut(),
                        );
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
    let mut items = match cmd_k_payload_json(&bookmarks, &history, &open_items) {
        Ok(json) => serde_json::from_str::<Vec<serde_json::Value>>(&json).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    // First-class Bitwarden vault entries (no passwords).
    let vault = AppState::global().vault();
    let status = vault.status();
    let url = tabs.active_url();
    let matches = if vault.has_session() && !url.is_empty() {
        vault.list_for_url(&url).unwrap_or_default()
    } else {
        Vec::new()
    };
    let vault_items = crate::dx::bitwarden::vault_palette_items(&status, &matches);
    if let Ok(extra) = serde_json::to_value(&vault_items) {
        if let Some(arr) = extra.as_array() {
            for v in arr {
                items.push(v.clone());
            }
        }
    }
    let payload = serde_json::to_string(&items).unwrap_or_else(|_| "[]".into());
    let status_js = engine_status::engine_status_script();
    let hotkey_script = content_hotkey_script();
    let script = format!(
        "{hotkey_script}; {status_js}; window.__thorCmdKPayload = {payload}; if (window.__thorEnsureCmdPalette) window.__thorEnsureCmdPalette(); if (window.openThorModal) window.openThorModal('nav');"
    );
    let _ = tabs.evaluate_active(&script);
    println!("⚡ [Thor] Opening ⌘K on Chromium tab {}", tabs.active);
}

fn open_cmd_shift_k_chromium(tabs: &ChromiumTabs) {
    let Ok(payload) = settings_payload_json() else {
        return;
    };
    let status = engine_status::engine_status_script();
    let hotkey_script = content_hotkey_script();
    let script = format!(
        "{hotkey_script}; {status}; window.__thorSettingsPayload = {payload}; if (window.__thorEnsureCmdPalette) window.__thorEnsureCmdPalette(); if (window.openThorModal) window.openThorModal('settings');"
    );
    let _ = tabs.evaluate_active(&script);
    println!("⚡ [Thor] Opening ⌘⇧K on Chromium tab {}", tabs.active);
}

fn open_downloads_palette_chromium(tabs: &ChromiumTabs) {
    let list = AppState::global().downloads().list_downloads();
    let Ok(payload) = downloads_payload_json(&list) else {
        return;
    };
    let status = engine_status::engine_status_script();
    let hotkey_script = content_hotkey_script();
    let script = format!(
        "{hotkey_script}; {status}; window.__thorSettingsPayload = {payload}; if (window.__thorEnsureCmdPalette) window.__thorEnsureCmdPalette(); if (window.openThorModal) window.openThorModal('settings'); if (document.getElementById('thorModalInput')) document.getElementById('thorModalInput').placeholder = 'Search downloads…';"
    );
    let _ = tabs.evaluate_active(&script);
    println!(
        "⚡ [Thor] Opening Downloads palette ({} items) on tab {}",
        list.len(),
        tabs.active
    );
}

fn handle_download_action(download_id: &str, reveal: bool) {
    let Some(item) = AppState::global().downloads().get_download(download_id) else {
        eprintln!("⚡ [Thor] download not found: {download_id}");
        return;
    };
    let path = std::path::Path::new(&item.target_path);
    if !path.exists() {
        eprintln!("⚡ [Thor] download path missing: {}", item.target_path);
        return;
    }
    if !reveal
        && item
            .security_evidence
            .as_ref()
            .map(|evidence| evidence.status)
            != Some(crate::net::DownloadSecurityStatus::Allowed)
    {
        eprintln!(
            "⚡ [Thor] refusing to open unverified or blocked download: {}",
            item.target_path
        );
        return;
    }
    #[cfg(target_os = "macos")]
    {
        let status = if reveal {
            std::process::Command::new("open")
                .args(["-R", &item.target_path])
                .status()
        } else {
            std::process::Command::new("open")
                .arg(&item.target_path)
                .status()
        };
        match status {
            Ok(s) if s.success() => {
                println!(
                    "⚡ [Thor] {} download {}",
                    if reveal { "revealed" } else { "opened" },
                    item.file_name()
                );
            }
            Ok(s) => eprintln!("⚡ [Thor] open failed (exit {s})"),
            Err(e) => eprintln!("⚡ [Thor] open failed: {e}"),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = reveal;
        println!("⚡ [Thor] download at {}", item.target_path);
    }
}

fn dispatch_chromium_ipc(msg: &str, proxy: &EventLoopProxy<UserEvent>, tabs: &mut ChromiumTabs) {
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
        "hotkey_new_tab" | "new_tab" => {
            let _ = proxy.send_event(UserEvent::Toolbar(ToolbarCmd::NewTab));
        }
        "hotkey_close_tab" | "close_tab" => {
            let _ = proxy.send_event(UserEvent::CloseActiveTab);
        }
        "hotkey_reload" | "reload" => {
            let _ = proxy.send_event(UserEvent::Toolbar(ToolbarCmd::Reload));
        }
        "hotkey_focus_url" => {
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
            let summary = payload["summary"]
                .as_str()
                .unwrap_or("no vitals")
                .to_string();
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
        "toggle_pip" => match tabs.toggle_pip_active() {
            Some(r) => println!(
                "⚡ [Thor PiP] active={} mode={} ({})",
                r.active, r.mode, r.detail
            ),
            None => eprintln!("⚡ [Thor PiP] no active tab"),
        },
        "set_auto_pip" => {
            let on = payload["enabled"].as_bool().unwrap_or(true);
            AppState::global().set_auto_pip(on);
            println!("⚡ [Thor PiP] auto_pip={on}");
            if on {
                let _ = tabs.evaluate_active(
                    crate::engine::browser_features::PictureInPicture::auto_pip_injection_script(),
                );
            }
        }
        "visit_recon_run" => {
            if let Some(report) = tabs.run_visit_recon_active() {
                let json = serde_json::to_string(&report).unwrap_or_else(|_| "null".into());
                let script = format!("window.__thorVisitRecon && window.__thorVisitRecon({json});");
                let _ = tabs.evaluate_active(&script);
                println!(
                    "⚡ [Thor Recon] {} grade={} findings={}",
                    report.url,
                    report.grade,
                    report.findings.len()
                );
            }
        }
        "visit_recon_get" => {
            if let Some(report) = AppState::global().last_visit_recon() {
                let json = serde_json::to_string(&report).unwrap_or_else(|_| "null".into());
                let script = format!("window.__thorVisitRecon && window.__thorVisitRecon({json});");
                let _ = tabs.evaluate_active(&script);
            }
        }
        "set_visit_recon_enabled" => {
            let on = payload["enabled"].as_bool().unwrap_or(true);
            AppState::global().set_visit_recon_enabled(on);
            println!("⚡ [Thor Recon] enabled={on}");
        }
        "set_interception_enabled" => {
            let enabled = payload["enabled"].as_bool().unwrap_or(false);
            let res = super::ipc_dispatch::dispatch_privileged(cmd, &payload);
            match &res {
                super::ipc_dispatch::DispatchResult::Message(m) => {
                    tabs.set_fetch_interception(enabled);
                    println!("⚡ [Thor] {m}");
                }
                super::ipc_dispatch::DispatchResult::Err(e) => {
                    eprintln!("⚡ [Thor] set_interception_enabled error: {e}");
                }
                _ => {
                    tabs.set_fetch_interception(enabled);
                }
            }
        }
        "copy_llm_clean_context" => {
            // Prefer HTML from chrome payload; else capture from active Chromium tab.
            let mut payload = payload.clone();
            if payload["html"].as_str().unwrap_or("").is_empty() {
                let html = tabs
                    .evaluate_active(
                        r#"(function(){ return document.documentElement ? document.documentElement.outerHTML : ''; })()"#,
                    )
                    .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| {
                        // chromiumoxide may return a JSON string value
                        serde_json::from_value::<String>(v.clone()).ok()
                    }));
                if let Some(h) = html {
                    payload["html"] = serde_json::Value::String(h);
                }
                if payload["url"].as_str().unwrap_or("").is_empty() {
                    payload["url"] = serde_json::Value::String(tabs.active_url());
                }
            }
            let res = super::ipc_dispatch::dispatch_privileged(cmd, &payload);
            if let super::ipc_dispatch::DispatchResult::FeatureJson(v) = &res {
                let json = serde_json::to_string(v).unwrap_or_else(|_| "null".into());
                let script = format!(
                    "window.__thorLlmCleanContext && window.__thorLlmCleanContext({json});"
                );
                let _ = tabs.evaluate_active(&script);
                println!("⚡ [Thor] LLM clean context ready ({} chars)", json.len());
            } else if let super::ipc_dispatch::DispatchResult::Err(e) = &res {
                eprintln!("⚡ [Thor] copy_llm_clean_context error: {e}");
            }
        }
        "open_ide"
        | "add_tamper_rule"
        | "add_bookmark"
        | "remove_bookmark"
        | "locust_start"
        | "locust_stop"
        | "decode_payload"
        | "decode_jwt_crack"
        | "bitwarden_status"
        | "bitwarden_lock"
        | "bitwarden_logout"
        | "bitwarden_search"
        | "bitwarden_matches"
        | "bitwarden_autofill"
        | "feature_list"
        | "feature_invoke"
        | "lens_payload"
        | "forward_intercepted"
        | "drop_intercepted"
        | "list_intercepted" => {
            // Inject active URL for matches when omitted.
            let mut payload = payload.clone();
            if cmd == "bitwarden_matches" && payload["url"].as_str().unwrap_or("").is_empty() {
                payload["url"] = serde_json::Value::String(tabs.active_url());
            }
            let res = super::ipc_dispatch::dispatch_privileged(cmd, &payload);
            if let super::ipc_dispatch::DispatchResult::Message(ref m) = res {
                if let Some(id) = m.strip_prefix("bitwarden:autofill:") {
                    match tabs.autofill_login(id) {
                        Ok(msg) => println!("⚡ [Thor Vault] autofill ok — {msg}"),
                        Err(e) => eprintln!("⚡ [Thor Vault] autofill failed: {e}"),
                    }
                    return;
                }
            }
            match &res {
                super::ipc_dispatch::DispatchResult::FeatureJson(v) => {
                    let json = serde_json::to_string(v).unwrap_or_else(|_| "null".into());
                    let hook = match cmd {
                        "bitwarden_search" | "bitwarden_matches" => "__thorBitwardenMatches",
                        "feature_list" => "__thorFeatureList",
                        "feature_invoke"
                        | "lens_payload"
                        | "list_intercepted"
                        | "forward_intercepted" => "__thorFeatureResult",
                        _ if cmd.starts_with("bitwarden_") => "__thorBitwardenStatus",
                        _ => "",
                    };
                    if !hook.is_empty() {
                        let script = format!("window.{hook} && window.{hook}({json});");
                        let _ = tabs.evaluate_active(&script);
                    }
                    println!("⚡ [Thor IPC Dispatch] {cmd} → FeatureJson");
                }
                super::ipc_dispatch::DispatchResult::Err(e) => {
                    eprintln!("⚡ [Thor IPC Dispatch] {cmd} error: {e}");
                    if cmd.starts_with("bitwarden_") {
                        let err = serde_json::json!({ "error": e });
                        let json = serde_json::to_string(&err).unwrap_or_else(|_| "null".into());
                        let script = format!(
                            "window.__thorBitwardenStatus && window.__thorBitwardenStatus({json});"
                        );
                        let _ = tabs.evaluate_active(&script);
                    } else if cmd == "feature_invoke" || cmd == "lens_payload" {
                        let err = serde_json::json!({ "error": e });
                        let json = serde_json::to_string(&err).unwrap_or_else(|_| "null".into());
                        let script = format!(
                            "window.__thorFeatureResult && window.__thorFeatureResult({json});"
                        );
                        let _ = tabs.evaluate_active(&script);
                    }
                }
                other => println!("⚡ [Thor IPC Dispatch] {cmd} → {other:?}"),
            }
        }
        // Master password must not travel through content-page IPC (SEC-091).
        "bitwarden_login" | "bitwarden_unlock" => {
            println!(
                "⚡ [Thor] rejected vault unlock/login from content IPC (use omnibar Vault tool)"
            );
        }
        "log" => {
            if let Some(m) = payload["message"].as_str() {
                println!("⚡ [Thor Chromium Log] {m}");
            }
        }
        _ => {}
    }
}

pub enum TabState {
    Active,
    Frozen,
    Discarded,
    Restoring,
    McpControlled,
    LocustRunning,
}
