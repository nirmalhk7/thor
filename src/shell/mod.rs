// pub mod compositor;
// pub mod crash_ui;
pub mod gui;
// pub mod physics;
// pub mod shaders;

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
const MODAL_HEIGHT: f64 = 520.0;

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

    // Shared refs for cross-webview communication
    let chrome_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));
    let content_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));

    // Clones for IPC handler (chrome → content & window bounds)
    let content_for_ipc = content_ref.clone();
    let chrome_for_ipc = chrome_ref.clone();

    // Build Chrome WebView — browser UI (tabs, address bar, navigation, modals)
    let chrome = WebViewBuilder::new()
        .with_html(gui::get_chrome_html())
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
            size: dpi::LogicalSize::new(1440.0, CHROME_HEIGHT).into(),
        })
        .with_transparent(true)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                match cmd {
                    "expand_chrome" => {
                        if let Some(ref wv) = *chrome_for_ipc.borrow() {
                            let _ = wv.set_bounds(Rect {
                                position: dpi::LogicalPosition::new(0.0, 0.0).into(),
                                size: dpi::LogicalSize::new(1440.0, MODAL_HEIGHT).into(),
                            });
                        }
                    }
                    "shrink_chrome" => {
                        if let Some(ref wv) = *chrome_for_ipc.borrow() {
                            let _ = wv.set_bounds(Rect {
                                position: dpi::LogicalPosition::new(0.0, 0.0).into(),
                                size: dpi::LogicalSize::new(1440.0, CHROME_HEIGHT).into(),
                            });
                        }
                    }
                    "navigate" => {
                        if let Some(url) = payload["url"].as_str() {
                            if let Some(ref wv) = *content_for_ipc.borrow() {
                                println!("[Thor] Navigating content webview to: {}", url);
                                let _ = wv.load_url(url);
                            }
                        }
                    }
                    "back" => {
                        if let Some(ref wv) = *content_for_ipc.borrow() {
                            let _ = wv.evaluate_script("history.back()");
                        }
                    }
                    "forward" => {
                        if let Some(ref wv) = *content_for_ipc.borrow() {
                            let _ = wv.evaluate_script("history.forward()");
                        }
                    }
                    "reload" => {
                        if let Some(ref wv) = *content_for_ipc.borrow() {
                            let _ = wv.evaluate_script("location.reload()");
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

    // Clones for content handlers (content → chrome sync & hotkeys)
    let chrome_for_title = chrome_ref.clone();
    let chrome_for_nav = chrome_ref.clone();
    let chrome_for_hotkeys = chrome_ref.clone();
    let content_for_hotkeys = content_ref.clone();

    // Hotkey listener script to inject into content webview in capture phase
    let hotkey_script = r#"
        window.addEventListener('keydown', (e) => {
            const isCmdOrCtrl = e.metaKey || e.ctrlKey;
            const key = e.key ? e.key.toLowerCase() : '';
            const code = e.code ? e.code.toLowerCase() : '';

            if (!isCmdOrCtrl) return;

            if (e.shiftKey && (key === 'k' || code === 'keyk')) {
                e.preventDefault();
                e.stopPropagation();
                window.ipc.postMessage(JSON.stringify({ cmd: 'hotkey_cmd_shift_k' }));
            } else if (!e.shiftKey && (key === 'k' || code === 'keyk')) {
                e.preventDefault();
                e.stopPropagation();
                window.ipc.postMessage(JSON.stringify({ cmd: 'hotkey_cmd_k' }));
            } else if (key === 't') {
                e.preventDefault();
                window.ipc.postMessage(JSON.stringify({ cmd: 'hotkey_new_tab' }));
            } else if (key === 'w') {
                e.preventDefault();
                window.ipc.postMessage(JSON.stringify({ cmd: 'hotkey_close_tab' }));
            } else if (key === 'l') {
                e.preventDefault();
                window.ipc.postMessage(JSON.stringify({ cmd: 'hotkey_focus_url' }));
            } else if (key === 'r') {
                e.preventDefault();
                window.ipc.postMessage(JSON.stringify({ cmd: 'hotkey_reload' }));
            }
        }, true);
    "#;

    // Build Content WebView — actual web pages with high-performance WKWebView configuration
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
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                if let Some(ref chrome_wv) = *chrome_for_hotkeys.borrow() {
                    match cmd {
                        "hotkey_cmd_k" => {
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('features')");
                        }
                        "hotkey_cmd_shift_k" => {
                            let _ = chrome_wv.evaluate_script("openModalFromHotkey('bookmarks_history')");
                        }
                        "hotkey_new_tab" => {
                            let _ = chrome_wv.evaluate_script("newTab()");
                        }
                        "hotkey_close_tab" => {
                            let _ = chrome_wv.evaluate_script("const ai = tabs.findIndex(t => t.active); if (ai >= 0) closeTab(ai);");
                        }
                        "hotkey_focus_url" => {
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
            true // allow all navigations
        })
        .with_new_window_req_handler(|_url: String| {
            false
        })
        .build_as_child(&window)?;

    // Store refs for event loop and cross-webview communication
    *chrome_ref.borrow_mut() = Some(chrome);
    *content_ref.borrow_mut() = Some(content);

    // Clones for resize handler
    let chrome_for_resize = chrome_ref.clone();
    let content_for_resize = content_ref.clone();

    println!("============================================================");
    println!("🚀 [Thor Shell] Matte Obsidian Developer Browser ACTIVE!");
    println!("🚀 [Thor Shell] High Performance WKWebView loaded — ⌘K & ⌘⇧K active.");
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
