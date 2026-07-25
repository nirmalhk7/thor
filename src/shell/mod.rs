pub mod compositor;
pub mod crash_ui;
pub mod physics;
pub mod shaders;

use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

pub fn run() -> anyhow::Result<()> {
    println!("[Thor Shell Log] Initializing native OS window event loop on main thread...");

    #[cfg(target_os = "macos")]
    println!("[Thor Shell Log] Backend: macOS Metal GPU API");
    
    #[cfg(target_os = "windows")]
    println!("[Thor Shell Log] Backend: Windows DX12 GPU API");
    
    #[cfg(target_os = "linux")]
    println!("[Thor Shell Log] Backend: Linux Vulkan GPU API");

    let event_loop = EventLoop::new()?;
    println!("[Thor Shell Log] winit EventLoop created successfully.");

    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Thor - Ultimate Developer Browser ⚡")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
            .build(&event_loop)?,
    );

    println!("[Thor Shell Log] Native OS Window handle constructed: ID {:?}", window.id());
    window.focus_window();
    println!("[Thor Shell Log] Window focus requested from OS window manager.");

    println!("[Thor Shell Log] Initializing wgpu Compositor pipeline...");
    let mut compositor = pollster::block_on(compositor::Compositor::new(Arc::clone(&window)))?;
    println!("[Thor Shell Log] wgpu Compositor initialized. Surface size: {}x{}", compositor.size.width, compositor.size.height);

    println!("============================================================");
    println!("🚀 [Thor Shell Log] Native Obsidian Glass GPU compositor ACTIVE!");
    println!("🚀 [Thor Shell Log] Entering 120Hz Event & Render Loop...");
    println!("============================================================");

    let mut frame_count: u64 = 0;

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    println!("[Thor Shell Log] CloseRequested received. Exiting application...");
                    elwt.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    println!("[Thor Shell Log] Window Resized: {}x{}", physical_size.width, physical_size.height);
                    compositor.resize(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    frame_count += 1;
                    if frame_count <= 5 || frame_count % 300 == 0 {
                        println!("[Thor Shell Log] Render Frame #{}", frame_count);
                    }

                    match compositor.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => {
                            eprintln!("[Thor Shell Log Error] Surface lost, reconfiguring...");
                            compositor.resize(compositor.size);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            eprintln!("[Thor Shell Log Error] Surface out of memory! Exiting...");
                            elwt.exit();
                        }
                        Err(e) => {
                            eprintln!("[Thor Shell Log Error] GPU Surface error: {:?}", e);
                        }
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => (),
        }
    })?;

    Ok(())
}
