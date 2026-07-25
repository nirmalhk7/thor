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

pub fn run() -> anyhow::Result<()> {
    println!("[Thor Shell Log] Initializing native OS window event loop on main thread...");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Thor — Developer Browser ⚡")
        .with_inner_size(tao::dpi::LogicalSize::new(1400.0, 900.0))
        .build(&event_loop)?;

    let chrome_height: f64 = 90.0;
    let content_webview: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));
    let content_ref = content_webview.clone();

    // Chrome WebView — browser UI (tabs, address bar, navigation)
    let _chrome_webview = WebViewBuilder::new()
        .with_html(gui::get_chrome_html())
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
            size: dpi::LogicalSize::new(1400.0, chrome_height).into(),
        })
        .with_transparent(true)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                if let Some(ref wv) = *content_ref.borrow() {
                    match cmd {
                        "navigate" => {
                            if let Some(url) = payload["url"].as_str() {
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
                        _ => {
                            println!("[Thor IPC] Unhandled command: {}", cmd);
                        }
                    }
                }
            }
        })
        .build_as_child(&window)?;

    // Content WebView — actual web pages
    let content = WebViewBuilder::new()
        .with_url("https://www.google.com")
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, chrome_height).into(),
            size: dpi::LogicalSize::new(1400.0, 900.0 - chrome_height).into(),
        })
        .with_devtools(true)
        .build_as_child(&window)?;

    *content_webview.borrow_mut() = Some(content);

    println!("============================================================");
    println!("🚀 [Thor Shell Log] Native tao+wry webview architecture ACTIVE!");
    println!("🚀 [Thor Shell Log] Rendering Posh Developer Workspace UI...");
    println!("============================================================");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    println!("[Thor Shell Log] CloseRequested received. Exiting application...");
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(new_size) => {
                    let width = new_size.width as f64;
                    let height = new_size.height as f64;

                    let _ = _chrome_webview.set_bounds(Rect {
                        position: dpi::LogicalPosition::new(0.0, 0.0).into(),
                        size: dpi::LogicalSize::new(width, chrome_height).into(),
                    });

                    if let Some(ref wv) = *content_webview.borrow() {
                        let _ = wv.set_bounds(Rect {
                            position: dpi::LogicalPosition::new(0.0, chrome_height).into(),
                            size: dpi::LogicalSize::new(width, height - chrome_height).into(),
                        });
                    }
                }
                _ => {}
            },
            _ => (),
        }
    });
}
