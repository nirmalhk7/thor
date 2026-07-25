pub mod compositor;
pub mod crash_ui;
pub mod gui;
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

    let event_loop = EventLoop::new()?;
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Thor - Ultimate Developer Browser ⚡")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0))
            .build(&event_loop)?,
    );

    window.focus_window();

    let mut compositor = pollster::block_on(compositor::Compositor::new(Arc::clone(&window)))?;
    let egui_ctx = egui::Context::default();
    let mut egui_state = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &window,
        Some(window.scale_factor() as f32),
        None,
    );

    let mut gui_state = gui::ThorGuiState::new();

    println!("============================================================");
    println!("🚀 [Thor Shell Log] Native Obsidian Glass GPU compositor ACTIVE!");
    println!("🚀 [Thor Shell Log] Rendering Posh Developer Workspace UI...");
    println!("============================================================");

    event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent { event, .. } => {
                let response = egui_state.on_window_event(&window, &event);
                if response.repaint {
                    window.request_redraw();
                }

                match event {
                    WindowEvent::CloseRequested => {
                        println!("[Thor Shell Log] CloseRequested received. Exiting application...");
                        elwt.exit();
                    }
                    WindowEvent::Resized(physical_size) => {
                        compositor.resize(physical_size);
                    }
                    WindowEvent::RedrawRequested => {
                        let raw_input = egui_state.take_egui_input(&window);
                        let full_output = egui_ctx.run(raw_input, |ctx| {
                            gui::render_thor_ui(ctx, &mut gui_state);
                        });

                        egui_state.handle_platform_output(&window, full_output.platform_output.clone());

                        match compositor.render_gui(&egui_ctx, full_output, &window) {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost) => compositor.resize(compositor.size),
                            Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                            Err(e) => eprintln!("[Thor Shell Log Error] GPU Surface error: {:?}", e),
                        }
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => (),
        }
    })?;

    Ok(())
}
