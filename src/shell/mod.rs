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

pub async fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    println!("⚡ Initializing macOS Metal GPU backend...");
    
    #[cfg(target_os = "windows")]
    println!("⚡ Initializing Windows DX12 GPU backend...");
    
    #[cfg(target_os = "linux")]
    println!("⚡ Initializing Linux Vulkan GPU backend...");

    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Thor - Ultimate Developer Browser ⚡")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
            .build(&event_loop)?,
    );

    let mut compositor = compositor::Compositor::new(Arc::clone(&window)).await?;

    println!("🚀 Native Obsidian Glass GPU compositor active! Entering 120Hz render loop...");

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    println!("Closing Thor...");
                    elwt.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    compositor.resize(physical_size);
                }
                WindowEvent::RedrawRequested => {
                    match compositor.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => compositor.resize(compositor.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(e) => eprintln!("GPU Surface error: {:?}", e),
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
