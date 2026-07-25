use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub mod daemon;
pub mod devtools;
pub mod dx;
pub mod engine;
pub mod ipc;
pub mod net;
pub mod network;
pub mod shell;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("⚡ Launching Thor Developer Browser Engine & Native Shell...");
    shell::run().await?;
    Ok(())
}

