//! Product-mode launch: Thor-owned Chromium is the browser; Rust is Labs sidecar.
//!
//! System Google Chrome is never the product surface. Legacy dual-window remains
//! behind `THOR_LEGACY_DUAL_WINDOW=1`.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::chromium_runtime::THOR_CHROMIUM_PATH_ENV;
use crate::paths::{thor_bundled_chromium_path, thor_chromium_profile_dir, thor_data_dir};

/// Env: require owned Chromium (do not fall back to legacy dual-window).
pub const THOR_REQUIRE_OWNED_CHROMIUM_ENV: &str = "THOR_REQUIRE_OWNED_CHROMIUM";
/// Env: allow discovering system Chrome/Chromium (labs/CI only — not product).
pub const THOR_ALLOW_SYSTEM_CHROME_ENV: &str = "THOR_ALLOW_SYSTEM_CHROME";
/// Env: remote debugging port for self-CDP (default 9222).
pub const THOR_REMOTE_DEBUGGING_PORT_ENV: &str = "THOR_REMOTE_DEBUGGING_PORT";

/// Resolve Thor-owned Chromium executable (never system Chrome for product mode).
pub fn discover_owned_chromium_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(THOR_CHROMIUM_PATH_ENV) {
        let p = PathBuf::from(path);
        if executable_exists(&p) {
            return Some(p);
        }
    }

    for candidate in owned_chromium_candidates() {
        if executable_exists(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn executable_exists(p: &Path) -> bool {
    p.is_file()
}

/// Candidate paths for Thor-owned builds / snapshot cache / app Frameworks.
pub fn owned_chromium_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    out.push(thor_bundled_chromium_path());

    out.push(
        thor_data_dir()
            .join("ChromiumCache")
            .join("Chromium.app")
            .join("Contents")
            .join("MacOS")
            .join("Chromium"),
    );

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("THOR_CHROMIUM_SRC") {
        roots.push(PathBuf::from(p));
    }
    if let Ok(s) = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".thor_chromium_src_path"),
    ) {
        roots.push(PathBuf::from(s.trim()));
    }
    if let Ok(home) = std::env::var("HOME") {
        roots.push(PathBuf::from(home).join("thor-chromium"));
    }

    for root in roots {
        let src = if root.join("src").is_dir() {
            root.join("src")
        } else {
            root
        };
        for rel in [
            "out/Thor/Thor.app/Contents/MacOS/Thor",
            "out/Thor/Chromium.app/Contents/MacOS/Chromium",
            "out/Thor/Content Shell.app/Contents/MacOS/Content Shell",
            "out/Release/Chromium.app/Contents/MacOS/Chromium",
        ] {
            out.push(src.join(rel));
        }
    }

    out.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("dist")
            .join("Thor.app")
            .join("Contents")
            .join("Frameworks")
            .join("Chromium.app")
            .join("Contents")
            .join("MacOS")
            .join("Chromium"),
    );

    out
}

fn debugging_port() -> u16 {
    std::env::var(THOR_REMOTE_DEBUGGING_PORT_ENV)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9222)
}

/// Spawn Thor-owned Chromium as the product browser and attach Labs (MCP / engine).
pub fn run_owned_chromium_browser(executable: PathBuf) -> anyhow::Result<()> {
    crate::thor_info!(
        "thor::product",
        "Thor product mode — owned Chromium: {}",
        executable.display()
    );

    let port = debugging_port();
    let user_data = thor_chromium_profile_dir();
    std::fs::create_dir_all(&user_data)?;

    let mut child = spawn_owned_chromium(&executable, &user_data, port)?;
    crate::thor_info!(
        "thor::product",
        "Chromium pid {} — CDP http://127.0.0.1:{port}",
        child.id()
    );

    let http = format!("http://127.0.0.1:{port}");
    wait_for_devtools(&http, Duration::from_secs(60))?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = rt.enter();

    if let Err(e) = rt.block_on(attach_labs(&http)) {
        crate::thor_warn!(
            "thor::product",
            "Labs attach degraded ({e}); browser remains running as product UI"
        );
    }

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("Thor Chromium exited with {status}")
    }
}

fn spawn_owned_chromium(exe: &Path, user_data: &Path, port: u16) -> anyhow::Result<Child> {
    let child = Command::new(exe)
        .arg(format!("--user-data-dir={}", user_data.display()))
        .arg(format!("--remote-debugging-port={port}"))
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("https://example.com")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(child)
}

fn wait_for_devtools(http_base: &str, timeout: Duration) -> anyhow::Result<()> {
    let deadline = std::time::Instant::now() + timeout;
    let url = format!("{http_base}/json/version");
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    while std::time::Instant::now() < deadline {
        let ok = rt.block_on(async {
            match reqwest::get(&url).await {
                Ok(resp) if resp.status().is_success() => true,
                _ => false,
            }
        });
        if ok {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    anyhow::bail!("timed out waiting for DevTools at {url}")
}

async fn attach_labs(debug_http: &str) -> anyhow::Result<()> {
    let runtime = crate::engine::self_cdp::connect_runtime(debug_http).await?;
    crate::thor_info!(
        "thor::product",
        "Self-CDP connected ({})",
        runtime.websocket_url()
    );
    let (engine, handle) = crate::engine::engine_coordinator::BrowserEngine::new(runtime);
    tokio::spawn(engine.run());
    tokio::spawn(async move {
        crate::mcp::server::start_mcp_server(handle).await;
    });
    Ok(())
}

pub fn legacy_dual_window_requested() -> bool {
    matches!(
        std::env::var("THOR_LEGACY_DUAL_WINDOW").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

pub fn require_owned_chromium() -> bool {
    matches!(
        std::env::var(THOR_REQUIRE_OWNED_CHROMIUM_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

pub fn allow_system_chrome() -> bool {
    matches!(
        std::env::var(THOR_ALLOW_SYSTEM_CHROME_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_candidates_include_cache_and_frameworks() {
        let c = owned_chromium_candidates();
        assert!(c.iter().any(|p| p.to_string_lossy().contains("ChromiumCache")));
        assert!(c.iter().any(|p| p.to_string_lossy().contains("Frameworks")));
    }

    #[test]
    fn lifecycle_hook_names_are_documented() {
        use crate::engine::freezing::ChromiumLifecycleHooks;
        assert!(ChromiumLifecycleHooks::FREEZE.contains("Frozen"));
        assert!(ChromiumLifecycleHooks::DISCARD.contains("discard"));
        assert!(ChromiumLifecycleHooks::EXPIRE.contains("Close"));
    }
}
