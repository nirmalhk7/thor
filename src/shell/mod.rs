pub mod compositor;
pub mod physics;
pub mod shaders;

pub async fn run() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    println!("Initializing macOS metal backend...");
    
    #[cfg(target_os = "windows")]
    println!("Initializing Windows DX12 backend...");
    
    #[cfg(target_os = "linux")]
    println!("Initializing Linux Vulkan backend...");

    let _compositor = compositor::Compositor::new().await?;
    
    println!("Shell running.");
    Ok(())
}
