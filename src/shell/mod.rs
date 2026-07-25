pub mod compositor;
pub mod crash_ui;
pub mod physics;
pub mod shaders;

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
    let window = WindowBuilder::new()
        .with_title("Thor - Ultimate Developer Browser")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)?;

    println!("🚀 Native GUI window spawned (1280x800). Entering main event loop...");

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                println!("Closing Thor...");
                elwt.exit();
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => (),
        }
    })?;

    Ok(())
}

