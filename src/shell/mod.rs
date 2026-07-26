pub mod gui;

use std::cell::RefCell;
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

    let window = builder.build(&event_loop)?;

    let chrome_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));
    let content_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));

    let content_for_ipc = content_ref.clone();
    let chrome_for_title = chrome_ref.clone();
    let chrome_for_nav = chrome_ref.clone();
    let chrome_for_hotkeys = chrome_ref.clone();
    let content_for_hotkeys = content_ref.clone();

    // Injected script for content webview to handle hotkeys
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
                } else if (key === 'r') {
                    e.preventDefault();
                    e.stopPropagation();
                    sendIpc(JSON.stringify({ cmd: 'hotkey_reload' }));
                }
            }

            window.addEventListener('keydown', handleKey, true);
            document.addEventListener('keydown', handleKey, true);
        })();
    "#;

    // 1. Build Content WebView FIRST (native webpage renderer)
    let content = WebViewBuilder::new()
        .with_url("https://www.google.com")
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
            size: dpi::LogicalSize::new(1440.0, 900.0 - CHROME_HEIGHT).into(),
        })
        .with_devtools(true)
        .with_initialization_script(hotkey_script)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor Content IPC Rx] {}", msg);
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                if let Some(ref chrome_wv) = *chrome_for_hotkeys.borrow() {
                    match cmd {
                        "log" => {
                            if let Some(log_msg) = payload["message"].as_str() {
                                println!("⚡ [Thor Content Log] {}", log_msg);
                            }
                        }
                        "hotkey_cmd_k" => {
                            println!("⚡ [Thor Hotkey] CMD+K triggered -> Opening Command Palette");
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('features')");
                        }
                        "hotkey_cmd_shift_k" => {
                            println!("⚡ [Thor Hotkey] CMD+SHIFT+K triggered -> Opening Bookmarks");
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('bookmarks_history')");
                        }
                        "hotkey_new_tab" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("newTab()");
                        }
                        "hotkey_close_tab" => {
                            let _ = chrome_wv.evaluate_script("const ai = tabs.findIndex(t => t.active); if (ai >= 0) closeTab(ai);");
                        }
                        "hotkey_focus_url" => {
                            let _ = chrome_wv.focus();
                            let _ = chrome_wv.evaluate_script("focusUrlInput()");
                        }
                        "hotkey_reload" => {
                            if let Some(ref content_wv) = *content_for_hotkeys.borrow() {
                                let _ = content_wv.evaluate_script("location.reload()");
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
        .with_document_title_changed_handler(move |title: String| {
            if let Some(ref wv) = *chrome_for_title.borrow() {
                let escaped = serde_json::to_string(&title).unwrap_or_default();
                let _ = wv.evaluate_script(&format!("updateTitle({})", escaped));
            }
        })
        .with_navigation_handler(move |url: String| {
            if let Some(ref wv) = *chrome_for_nav.borrow() {
                let escaped = serde_json::to_string(&url).unwrap_or_default();
                let _ = wv.evaluate_script(&format!("updateUrl({})", escaped));
            }
            true
        })
        .with_new_window_req_handler(|_url: String| false)
        .build_as_child(&window)?;

    // 2. Build Chrome WebView SECOND (header toolbar, fixed size 48px)
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
                    "navigate" => {
                        if let Some(url) = payload["url"].as_str() {
                            if let Some(ref wv) = *content_for_ipc.borrow() {
                                println!("⚡ [Thor Rust] Navigating content webview to: {}", url);
                                let _ = wv.load_url(url);
                                let _ = wv.focus();
                            }
                        }
                    }
                    "back" => {
                        if let Some(ref wv) = *content_for_ipc.borrow() {
                            let _ = wv.evaluate_script("history.back()");
                            let _ = wv.focus();
                        }
                    }
                    "forward" => {
                        if let Some(ref wv) = *content_for_ipc.borrow() {
                            let _ = wv.evaluate_script("history.forward()");
                            let _ = wv.focus();
                        }
                    }
                    "reload" => {
                        if let Some(ref wv) = *content_for_ipc.borrow() {
                            let _ = wv.evaluate_script("location.reload()");
                            let _ = wv.focus();
                        }
                    }
                    "toggle_devtools" => {
                        if let Some(ref wv) = *content_for_ipc.borrow() {
                            if wv.is_devtools_open() {
                                wv.close_devtools();
                            } else {
                                wv.open_devtools();
                            }
                        }
                    }
                    _ => {}
                }
            }
        })
        .build_as_child(&window)?;

    *content_ref.borrow_mut() = Some(content);
    *chrome_ref.borrow_mut() = Some(chrome);

    let chrome_for_resize = chrome_ref.clone();
    let content_for_resize = content_ref.clone();

    println!("============================================================");
    println!("🚀 [Thor Shell] Matte Obsidian Developer Browser ACTIVE!");
    println!("🚀 [Thor Shell] Dual Native WebViews: Chrome (48px) + Content (Full).");
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

                    if let Some(ref wv) = *chrome_for_resize.borrow() {
                        let _ = wv.set_bounds(Rect {
                            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
                            size: dpi::LogicalSize::new(w, CHROME_HEIGHT).into(),
                        });
                    }
                    if let Some(ref wv) = *content_for_resize.borrow() {
                        let _ = wv.set_bounds(Rect {
                            position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
                            size: dpi::LogicalSize::new(w, h - CHROME_HEIGHT).into(),
                        });
                    }
                }
                _ => {}
            },
            _ => (),
        }
    });
}
