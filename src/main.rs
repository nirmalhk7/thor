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
pub mod features;
pub mod ipc;
pub mod load;
pub mod net;
pub mod network;
pub mod paths;
pub mod shell;
pub mod mcp;
pub mod logging;

fn main() -> anyhow::Result<()> {
    let _logger = thor::logging::init_logging();
    thor::thor_info!("thor::main", "============================================================");
    thor::thor_info!("thor::main", "⚡ [Thor Main Log] Starting Thor Developer Browser...");
    thor::thor_info!("thor::main", "⚡ [Thor Main Log] Initializing Tokio background runtime...");
    let _rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = _rt.enter();
    thor::thor_info!("thor::main", "⚡ [Thor Main Log] Tokio runtime active.");
    thor::thor_info!("thor::main", "⚡ [Thor Main Log] Delegating execution to native shell on main OS thread...");
    thor::thor_info!("thor::main", "============================================================");

    shell::run()?;
    Ok(())
}
