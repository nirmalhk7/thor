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
    let chrome_for_ipc = chrome_ref.clone();
    let chrome_for_resize = chrome_ref.clone();

    // Build Single Unified WebView taking up full window bounds
    let chrome = WebViewBuilder::new()
        .with_html(gui::get_chrome_html())
        .with_bounds(Rect {
            position: dpi::LogicalPosition::new(0.0, 0.0).into(),
            size: dpi::LogicalSize::new(1440.0, 900.0).into(),
        })
        .with_devtools(true)
        .with_ipc_handler(move |request: http::Request<String>| {
            let msg = request.body();
            println!("⚡ [Thor IPC Rx] {}", msg);
            let payload: serde_json::Value = serde_json::from_str(msg).unwrap_or_default();
            if let Some(cmd) = payload["cmd"].as_str() {
                match cmd {
                    "log" => {
                        if let Some(log_msg) = payload["message"].as_str() {
                            println!("⚡ [Thor Log] {}", log_msg);
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
                    "toggle_devtools" => {
                        if let Some(ref wv) = *chrome_for_ipc.borrow() {
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

    *chrome_ref.borrow_mut() = Some(chrome);

    println!("============================================================");
    println!("🚀 [Thor Shell] Matte Obsidian Developer Browser ACTIVE!");
    println!("🚀 [Thor Shell] Single-WebView Unified Architecture LOADED.");
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
                            size: dpi::LogicalSize::new(w, h).into(),
                        });
                    }
                }
                _ => {}
            },
            _ => (),
        }
    });
}
