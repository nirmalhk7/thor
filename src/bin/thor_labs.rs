//! Thor MCP stdio bridge. Connects to the CEF browser already running in Thor.
//!
//! Usage:
//!   thor-labs --stdio
//!   THOR_SELF_CDP_URL=http://127.0.0.1:9222 thor-labs --self-cdp (legacy)
//!
//! Native messaging host: browser/native_messaging/com.thor.labs.json

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    if args.is_empty() || args == ["--stdio"] {
        return rt.block_on(thor::mcp::stdio::run());
    }
    if args != ["--self-cdp"] {
        anyhow::bail!("Usage: thor-labs [--stdio | --self-cdp]");
    }
    let _logger = thor::logging::init_logging();
    let url = std::env::var("THOR_SELF_CDP_URL")
        .map_err(|_| anyhow::anyhow!("--self-cdp requires THOR_SELF_CDP_URL"))?;

    thor::thor_info!("thor::labs", "Thor Labs sidecar attaching to {url}");

    rt.block_on(async move {
        let runtime = thor::engine::self_cdp::connect_runtime(url).await?;
        thor::thor_info!("thor::labs", "Connected — ws {}", runtime.websocket_url());

        let _ = thor::AppState::global();
        let features = thor::AppState::global().features();
        thor::thor_info!(
            "thor::labs",
            "FeatureEngine ready ({} ids)",
            thor::FeatureId::ALL.len()
        );
        let _ = features;

        let (engine, handle) = thor::engine::engine_coordinator::BrowserEngine::new(runtime);
        tokio::spawn(engine.run());
        thor::mcp::server::start_mcp_server(handle).await?;
        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}
