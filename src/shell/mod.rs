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

const CHROME_HEIGHT: f64 = 96.0;

pub fn run() -> anyhow::Result<()> {
    println!("[Thor Shell] Initializing native OS window event loop on main thread...");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Thor — Developer Browser ⚡")
        .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 900.0))
        .build(&event_loop)?;

    // Shared refs for cross-webview communication
    let chrome_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));
    let content_ref: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));

    // Clones for IPC handler (chrome → content)
    let content_for_ipc = content_ref.clone();

    // Build Chrome WebView — browser UI (tabs, address bar, navigation)
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
                if let Some(ref wv) = *content_for_ipc.borrow() {
                    match cmd {
                        "navigate" => {
                            if let Some(url) = payload["url"].as_str() {
                                println!("[Thor] Navigating to: {}", url);
                                let _ = wv.load_url(url);
                            }
                        }
                        "back" => { let _ = wv.evaluate_script("history.back()"); }
                        "forward" => { let _ = wv.evaluate_script("history.forward()"); }
                        "reload" => { let _ = wv.evaluate_script("location.reload()"); }
                        "toggle_devtools" => {
                            if wv.is_devtools_open() {
                                wv.close_devtools();
                            } else {
                                wv.open_devtools();
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
        .build_as_child(&window)?;

    // Clones for content handlers (content → chrome sync)
    let chrome_for_title = chrome_ref.clone();
    let chrome_for_nav = chrome_ref.clone();

    // Build Content WebView — actual web pages
    let content = WebViewBuilder::new()
        .with_url("https://www.google.com")
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
            size: dpi::LogicalSize::new(1440.0, 900.0 - CHROME_HEIGHT).into(),
        })
        .with_devtools(true)
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
            // Block target="_blank" from opening system browser
            // The navigation handler will catch subsequent loads
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
    println!("🚀 [Thor Shell] Functional developer browser ACTIVE!");
    println!("🚀 [Thor Shell] WKWebView engine loaded — browsing enabled.");
    println!("============================================================");

    event_loop.run(move |event, _, control_flow| {
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
