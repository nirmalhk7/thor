pub mod gui;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::{dpi, Rect, WebView, WebViewBuilder};

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;

const CHROME_HEIGHT: f64 = 48.0;

fn create_content_tab(
    window: &tao::window::Window,
    url: &str,
    tab_id: String,
    chrome_ref: Rc<RefCell<Option<WebView>>>,
    hotkey_script: &str,
    w: f64,
    h: f64,
) -> anyhow::Result<WebView> {
    let chrome_for_hotkeys = chrome_ref.clone();
    let chrome_for_title = chrome_ref.clone();
    let chrome_for_nav = chrome_ref.clone();
    let id_for_title = tab_id.clone();
    let id_for_nav = tab_id.clone();

    let wv = WebViewBuilder::new()
        .with_url(url)
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
            size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
        })
        .with_devtools(true)
        .with_initialization_script(hotkey_script)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor Tab IPC Rx] {}", msg);
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                if let Some(ref chrome_wv) = *chrome_for_hotkeys.borrow() {
                    match cmd {
                        "log" => {
                            if let Some(log_msg) = payload["message"].as_str() {
                                println!("⚡ [Thor Log] {}", log_msg);
                            }
                        }
                        "hotkey_cmd_k" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('features')");
                        }
                        "hotkey_cmd_shift_k" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('bookmarks_history')");
                        }
                        "hotkey_new_tab" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("newTab()");
                        }
                        "hotkey_close_tab" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("const ai = tabs.findIndex(t => t.active); if (ai >= 0) closeTab(ai);");
                        }
                        "hotkey_focus_url" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("focusUrlInput()");
                        }
                        _ => {}
                    }
                }
            }
        })
        .with_document_title_changed_handler(move |title: String| {
            if let Some(ref wv) = *chrome_for_title.borrow() {
                let escaped_title = serde_json::to_string(&title).unwrap_or_default();
                let escaped_id = serde_json::to_string(&id_for_title).unwrap_or_default();
                let _ = wv.evaluate_script(&format!("updateTabTitle({}, {})", escaped_id, escaped_title));
            }
        })
        .with_navigation_handler(move |url: String| {
            if let Some(ref wv) = *chrome_for_nav.borrow() {
                let escaped_url = serde_json::to_string(&url).unwrap_or_default();
                let escaped_id = serde_json::to_string(&id_for_nav).unwrap_or_default();
                let _ = wv.evaluate_script(&format!("updateTabUrl({}, {})", escaped_id, escaped_url));
            }
            true
        })
        .with_new_window_req_handler(|_url: String| false)
        .build_as_child(window)?;

    Ok(wv)
}

pub fn run() -> anyhow::Result<()> {
    println!("[Thor Shell] Initializing native OS window event loop on main thread...");

    let event_loop = EventLoop::new();
    let mut builder = WindowBuilder::new()
        .with_title("Thor — Developer Browser ⚡")
        .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0));

    #[cfg(target_os = "macos")]
    {
        builder = builder
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true);
    }

    let window = Rc::new(builder.build(&event_loop)?);

    let chrome_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));
    let tab_pool: Rc<RefCell<HashMap<String, WebView>>> = Rc::new(RefCell::new(HashMap::new()));
    let active_tab_id: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    let win_size: Rc<RefCell<(f64, f64)>> = Rc::new(RefCell::new((1440.0, 900.0)));

    let hotkey_script = r#"
        (function() {
            function sendIpc(msg) {
                try {
                    if (window.ipc && window.ipc.postMessage) {
                        window.ipc.postMessage(msg);
                    } else if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc) {
                        window.webkit.messageHandlers.ipc.postMessage(msg);
                    }
                } catch(e) {}
            }

            function handleKey(e) {
                const isCmdOrCtrl = e.metaKey || e.ctrlKey;
                if (!isCmdOrCtrl) return;

                const key = e.key ? e.key.toLowerCase() : '';
                const code = e.code ? e.code.toLowerCase() : '';

                if (e.shiftKey && (key === 'k' || code === 'keyk')) {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_cmd_shift_k' }));
                } else if (!e.shiftKey && (key === 'k' || code === 'keyk')) {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_cmd_k' }));
                } else if (key === 't') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_new_tab' }));
                } else if (key === 'w') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_close_tab' }));
                } else if (key === 'l') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_focus_url' }));
                }
            }

            window.addEventListener('keydown', handleKey, true);
            document.addEventListener('keydown', handleKey, true);
        })();
    "#;

    // Create Initial Default Native Tab ("default_tab")
    let initial_tab_id = "default_tab".to_string();
    let initial_wv = create_content_tab(
        &window,
        "https://www.google.com",
        initial_tab_id.clone(),
        chrome_ref.clone(),
        hotkey_script,
        1440.0,
        900.0,
    )?;

    tab_pool.borrow_mut().insert(initial_tab_id.clone(), initial_wv);
    *active_tab_id.borrow_mut() = initial_tab_id;

    // Clones for Chrome IPC
    let window_for_ipc = window.clone();
    let chrome_for_ipc = chrome_ref.clone();
    let tab_pool_for_ipc = tab_pool.clone();
    let active_tab_for_ipc = active_tab_id.clone();
    let win_size_for_ipc = win_size.clone();
    let hotkey_script_for_ipc = hotkey_script.to_string();

    // Build Chrome Toolbar WebView SECOND (fixed 48px header on top)
    let chrome = WebViewBuilder::new()
        .with_html(gui::get_chrome_html())
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
            size: dpi::LogicalSize::new(1440.0, CHROME_HEIGHT).into(),
        })
        .with_transparent(true)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor Chrome IPC Rx] {}", msg);
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                match cmd {
                    "log" => {
                        if let Some(log_msg) = payload["message"].as_str() {
                            println!("⚡ [Thor Chrome Log] {}", log_msg);
                        }
                    }
                    "new_tab" => {
                        let tab_id = payload["id"].as_str().unwrap_or("tab_new").to_string();
                        let url = payload["url"].as_str().unwrap_or("https://www.google.com");
                        println!("⚡ [Thor Rust Engine] Creating new native tab: {} ({})", tab_id, url);
                        
                        let (w, h) = *win_size_for_ipc.borrow();
                        if let Ok(new_wv) = create_content_tab(
                            &window_for_ipc,
                            url,
                            tab_id.clone(),
                            chrome_for_ipc.clone(),
                            &hotkey_script_for_ipc,
                            w,
                            h,
                        ) {
                            // Hide currently active tab
                            let cur_active = active_tab_for_ipc.borrow().clone();
                            let pool = tab_pool_for_ipc.borrow();
                            if let Some(cur_wv) = pool.get(&cur_active) {
                                let _ = cur_wv.set_bounds(Rect {
                                    position: dpi::LogicalPosition::new(-10000.0, -10000.0).into(),
                                    size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                                });
                            }
                            drop(pool);

                            // Insert & Focus new tab
                            tab_pool_for_ipc.borrow_mut().insert(tab_id.clone(), new_wv);
                            *active_tab_for_ipc.borrow_mut() = tab_id.clone();
                            if let Some(active_wv) = tab_pool_for_ipc.borrow().get(&tab_id) {
                                let _ = active_wv.focus();
                            }
                        }
                    }
                    "switch_tab" => {
                        if let Some(target_id) = payload["id"].as_str() {
                            println!("⚡ [Thor Rust Engine] Switching to native tab: {}", target_id);
                            let (w, h) = *win_size_for_ipc.borrow();
                            let pool = tab_pool_for_ipc.borrow();
                            for (id, wv) in pool.iter() {
                                if id == target_id {
                                    let _ = wv.set_bounds(Rect {
                                        position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
                                        size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                                    });
                                    let _ = wv.focus();
                                } else {
                                    let _ = wv.set_bounds(Rect {
                                        position: dpi::LogicalPosition::new(-10000.0, -10000.0).into(),
                                        size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                                    });
                                }
                            }
                            *active_tab_for_ipc.borrow_mut() = target_id.to_string();
                        }
                    }
                    "close_tab" => {
                        if let Some(target_id) = payload["id"].as_str() {
                            println!("⚡ [Thor Rust Engine] Destroying native tab: {}", target_id);
                            tab_pool_for_ipc.borrow_mut().remove(target_id);
                        }
                    }
                    "navigate" => {
                        if let Some(url) = payload["url"].as_str() {
                            let cur_active = active_tab_for_ipc.borrow().clone();
                            let pool = tab_pool_for_ipc.borrow();
                            if let Some(wv) = pool.get(&cur_active) {
                                println!("⚡ [Thor Rust Engine] Navigating native tab [{}] to: {}", cur_active, url);
                                let _ = wv.load_url(url);
                                let _ = wv.focus();
                            }
                        }
                    }
                    "back" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.evaluate_script("history.back()");
                            let _ = wv.focus();
                        }
                    }
                    "forward" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.evaluate_script("history.forward()");
                            let _ = wv.focus();
                        }
                    }
                    "reload" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            let _ = wv.evaluate_script("location.reload()");
                            let _ = wv.focus();
                        }
                    }
                    "toggle_devtools" => {
                        let cur_active = active_tab_for_ipc.borrow().clone();
                        let pool = tab_pool_for_ipc.borrow();
                        if let Some(wv) = pool.get(&cur_active) {
                            if wv.is_devtools_open() {
                                wv.close_devtools();
                            } else {
                                wv.open_devtools();
                            }
                        }
                    }
                    "open_ide" => {
                        let ide_name = payload["ide"].as_str().unwrap_or("vscode");
                        let path = payload["path"].as_str().unwrap_or(".");
                        let line = payload["line"].as_u64().map(|l| l as usize);
                        let col = payload["col"].as_u64().map(|c| c as usize);
                        let ide = match ide_name {
                            "cursor" => crate::dx::ide_bridge::Ide::Cursor,
                            "webstorm" => crate::dx::ide_bridge::Ide::WebStorm,
                            _ => crate::dx::ide_bridge::Ide::VSCode,
                        };
                        println!("⚡ [Thor Rust] Opening file in IDE ({}): {}", ide_name, path);
                        let _ = crate::dx::ide_bridge::IdeBridge::open_file(ide, path, line, col);
                    }
                    _ => {}
                }
            }
        })
        .build_as_child(&window)?;

    *chrome_ref.borrow_mut() = Some(chrome);

    let chrome_for_resize = chrome_ref.clone();
    let tab_pool_for_resize = tab_pool.clone();
    let active_tab_for_resize = active_tab_id.clone();
    let win_size_for_resize = win_size.clone();

    println!("============================================================");
    println!("🚀 [Thor Shell] Matte Obsidian Developer Browser ACTIVE!");
    println!("🚀 [Thor Shell] True Native Multi-Tab Engine LOADED.");
    println!("============================================================");

    event_loop.run(move |event, _, control_flow| {
        let _window = &window;
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(new_size) => {
                    let w = new_size.width as f64;
                    let h = new_size.height as f64;
                    *win_size_for_resize.borrow_mut() = (w, h);

                    if let Some(ref wv) = *chrome_for_resize.borrow() {
                        let _ = wv.set_bounds(Rect {
                            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
                            size: dpi::LogicalSize::new(w, CHROME_HEIGHT).into(),
                        });
                    }

                    let cur_active = active_tab_for_resize.borrow().clone();
                    let pool = tab_pool_for_resize.borrow();
                    for (id, wv) in pool.iter() {
                        if id == &cur_active {
                            let _ = wv.set_bounds(Rect {
                                position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
                                size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                            });
                        } else {
                            let _ = wv.set_bounds(Rect {
                                position: dpi::LogicalPosition::new(-10000.0, -10000.0).into(),
                                size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                            });
                        }
                    }
                }
                _ => {}
            },
            _ => (),
        }
    });
}
