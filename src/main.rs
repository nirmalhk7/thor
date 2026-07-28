#![allow(deprecated)]
#![allow(unexpected_cfgs)]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub mod app_state;
pub mod daemon;
pub mod devtools;
pub mod dx;
pub mod engine;
pub mod ipc;
pub mod load;
pub mod net;
pub mod network;
pub mod paths;
pub mod shell;
pub mod mcp;

fn main() -> anyhow::Result<()> {
    println!("============================================================");
    println!("⚡ [Thor Main Log] Starting Thor Developer Browser...");
    println!("⚡ [Thor Main Log] Initializing Tokio background runtime...");
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = _rt.enter();
    println!("⚡ [Thor Main Log] Tokio runtime active.");
    println!("⚡ [Thor Main Log] Delegating execution to native shell on main OS thread...");
    println!("============================================================");

    shell::run()?;
    Ok(())
}
