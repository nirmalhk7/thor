//! Product-mode launch: Thor-owned Chromium is the browser; Rust is Labs sidecar.
//!
//! System Google Chrome is never the product surface. Legacy dual-window remains
//! behind `THOR_LEGACY_DUAL_WINDOW=1`.

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use super::chromium_runtime::THOR_CHROMIUM_PATH_ENV;
use crate::app_state::AppState;
use crate::paths::{thor_bundled_chromium_path, thor_chromium_profile_dir};
use anyhow::Context;

/// Env: require owned Chromium (do not fall back to legacy dual-window).
pub const THOR_REQUIRE_OWNED_CHROMIUM_ENV: &str = "THOR_REQUIRE_OWNED_CHROMIUM";
/// Env: allow discovering system Chrome/Chromium (labs/CI only — not product).
pub const THOR_ALLOW_SYSTEM_CHROME_ENV: &str = "THOR_ALLOW_SYSTEM_CHROME";
/// Env: fixed remote debugging port for self-CDP (default is ephemeral).
pub const THOR_REMOTE_DEBUGGING_PORT_ENV: &str = "THOR_REMOTE_DEBUGGING_PORT";

const REQUIRED_SHIELD_ASSETS: &[&str] = &[
    "manifest.json",
    "service_worker.js",
    "fingerprint.js",
    "rules.json",
    "filter_bundle.json",
    "options.html",
    "options.js",
];

/// Resolve Thor-owned Chromium executable (never system Chrome for product mode).
pub fn discover_owned_chromium_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(THOR_CHROMIUM_PATH_ENV) {
        let p = PathBuf::from(path);
        if executable_exists(&p) && has_thor_product_marker(&p) {
            return Some(p);
        }
    }

    for candidate in owned_chromium_candidates() {
        if executable_exists(&candidate) && has_thor_product_marker(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn executable_exists(p: &Path) -> bool {
    if !p.is_file() || p.is_symlink() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return p
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    true
}

/// Accept only a bundle/source output explicitly staged by Thor's build flow.
/// A stock Chromium executable in `dist/` must never become the product merely
/// because it happens to have a matching path.
fn has_thor_product_marker(executable: &Path) -> bool {
    verify_thor_product_marker(executable, None).is_ok()
}

fn product_marker_path(executable: &Path) -> anyhow::Result<PathBuf> {
    let macos = executable
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "MacOS"))
        .context("owned browser executable is not inside Contents/MacOS")?;
    let contents = macos
        .parent()
        .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
        .context("owned browser executable has no Contents bundle parent")?;
    let browser_app = contents
        .parent()
        .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
        .context("owned browser executable is not inside an application bundle")?;
    let container = browser_app
        .parent()
        .context("browser bundle has no parent")?;
    let marker = if container
        .file_name()
        .is_some_and(|name| name == "Frameworks")
    {
        container
            .parent()
            .context("packaged Frameworks directory has no Contents parent")?
            .join("Resources/ThorOwnedChromium.json")
    } else {
        container.join("ThorOwnedChromium.json")
    };
    Ok(marker)
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut block = [0_u8; 1024 * 1024];
    loop {
        let count = file.read(&mut block)?;
        if count == 0 {
            break;
        }
        digest.update(&block[..count]);
    }
    let digest = digest.finalize();
    Ok(format!("{digest:x}"))
}

fn collect_tree_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!("provenance input contains a symlink: {}", path.display());
        }
        if metadata.is_dir() {
            collect_tree_files(root, &path, files)?;
        } else if metadata.is_file() {
            path.strip_prefix(root)
                .context("Shield tree entry escaped its root")?;
            files.push(path);
        }
    }
    Ok(())
}

fn sha256_tree(root: &Path) -> anyhow::Result<String> {
    let mut files = Vec::new();
    collect_tree_files(root, root, &mut files)?;
    files.sort_by(|left, right| {
        left.strip_prefix(root)
            .unwrap_or(left)
            .cmp(right.strip_prefix(root).unwrap_or(right))
    });
    if files.is_empty() {
        anyhow::bail!("Shield tree is empty: {}", root.display());
    }
    let mut digest = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .context("Shield tree entry escaped its root")?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        digest.update(relative.as_bytes());
        digest.update([0]);
        let mut file = std::fs::File::open(&path)?;
        let mut block = [0_u8; 1024 * 1024];
        loop {
            let count = file.read(&mut block)?;
            if count == 0 {
                break;
            }
            digest.update(&block[..count]);
        }
        digest.update([0]);
    }
    let digest = digest.finalize();
    Ok(format!("{digest:x}"))
}

fn verify_browser_bundle_identity(executable: &Path) -> anyhow::Result<()> {
    let info_plist = executable
        .parent()
        .and_then(Path::parent)
        .map(|contents| contents.join("Info.plist"))
        .context("browser executable has no bundle Info.plist path")?;
    if !info_plist.is_file() {
        anyhow::bail!(
            "browser bundle Info.plist is missing: {}",
            info_plist.display()
        );
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/bin/plutil")
            .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
            .arg(&info_plist)
            .output()?;
        if !output.status.success()
            || String::from_utf8_lossy(&output.stdout).trim() != "com.thor.browser"
        {
            anyhow::bail!("browser bundle identifier is not com.thor.browser");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let contents = std::fs::read(&info_plist)?;
        if !contents
            .windows(b"com.thor.browser".len())
            .any(|window| window == b"com.thor.browser")
        {
            anyhow::bail!("browser bundle identifier is not com.thor.browser");
        }
    }
    Ok(())
}

fn verify_thor_product_marker(executable: &Path, shield: Option<&Path>) -> anyhow::Result<()> {
    if !executable_exists(executable) {
        anyhow::bail!("owned browser executable is missing, linked, or not executable");
    }
    verify_browser_bundle_identity(executable)?;
    let marker_path = product_marker_path(executable)?;
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(&marker_path)?)
        .with_context(|| format!("invalid Thor provenance marker: {}", marker_path.display()))?;
    if marker["schema_version"] != 2
        || marker["product"] != "Thor"
        || marker["source"]["repository"] != "https://chromium.googlesource.com/chromium/src.git"
        || marker["source"]["ref"] != "151.0.7922.47"
        || marker["source"]["commit"] != "c583827a354674180dc8856eb65aeb3f0afa5ee9"
        || marker["artifact"]["kind"] != "chromium_browser_executable"
    {
        anyhow::bail!("Thor provenance marker has unexpected product, source, or artifact data");
    }
    let metadata = executable.metadata()?;
    let executable_sha256 = sha256_file(executable)?;
    if marker["artifact"]["filename"].as_str()
        != executable.file_name().and_then(|name| name.to_str())
        || marker["artifact"]["size"].as_u64() != Some(metadata.len())
        || marker["artifact"]["sha256"].as_str() != Some(executable_sha256.as_str())
    {
        anyhow::bail!("Thor provenance marker is not bound to this browser executable");
    }
    if let Some(shield) = shield {
        let expected = marker["inputs"]["shield"]["tree_sha256"]
            .as_str()
            .context("Thor provenance marker lacks Shield tree digest")?;
        let actual = sha256_tree(shield)?;
        if expected != actual {
            anyhow::bail!("Thor Shield tree differs from executable provenance");
        }
    }
    Ok(())
}

/// Candidate paths for Thor-owned source builds and packaged app Frameworks.
pub fn owned_chromium_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    out.push(thor_bundled_chromium_path());

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
    out.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("dist")
            .join("Thor.app")
            .join("Contents")
            .join("Frameworks")
            .join("Chromium.app")
            .join("Contents")
            .join("MacOS")
            .join("Thor"),
    );

    out
}

fn parse_debugging_port(value: Option<&str>) -> anyhow::Result<u16> {
    match value {
        None => Ok(0),
        Some(raw) => raw.parse::<u16>().with_context(|| {
            format!("{THOR_REMOTE_DEBUGGING_PORT_ENV} must be a port from 0 to 65535, got {raw:?}")
        }),
    }
}

fn debugging_port() -> anyhow::Result<u16> {
    let value = std::env::var(THOR_REMOTE_DEBUGGING_PORT_ENV).ok();
    parse_debugging_port(value.as_deref())
}

/// Spawn Thor-owned Chromium as the product browser and attach Labs (MCP / engine).
pub fn run_owned_chromium_browser(executable: PathBuf) -> anyhow::Result<()> {
    crate::thor_info!(
        "thor::product",
        "Thor product mode — owned Chromium: {}",
        executable.display()
    );

    let port = debugging_port()?;
    let user_data = thor_chromium_profile_dir();
    std::fs::create_dir_all(&user_data)?;
    let active_port_file = user_data.join("DevToolsActivePort");
    let previous_active_port = if port == 0 {
        std::fs::read(&active_port_file).ok()
    } else {
        None
    };

    let mut child = spawn_owned_chromium(&executable, &user_data, port)?;
    crate::thor_info!(
        "thor::product",
        "Chromium pid {} — awaiting {} CDP endpoint",
        child.id(),
        if port == 0 { "ephemeral" } else { "configured" }
    );

    let http = wait_for_devtools(
        &mut child,
        &user_data,
        port,
        previous_active_port.as_deref(),
        Duration::from_secs(60),
    )?;
    crate::thor_info!("thor::product", "Self-CDP endpoint ready at {http}");

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
    let shield = thor_shield_extension_path()?;
    verify_thor_product_marker(exe, Some(&shield))
        .context("Thor browser executable/Shield provenance verification failed")?;
    let mut command = Command::new(exe);
    command
        .arg(format!("--user-data-dir={}", user_data.display()))
        .arg(format!("--remote-debugging-port={port}"))
        .arg(format!(
            "--thor-privacy-profile={}",
            AppState::global()
                .profiles()
                .get_profile()
                .mode
                .chromium_value()
        ))
        .arg("--no-first-run");
    command.args(crate::engine::ThorBrowserDefaults::chromium_runtime_args());
    command.arg(format!("--load-extension={}", shield.display()));
    let child = command
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(child)
}

fn validate_thor_shield_extension(candidate: &Path) -> anyhow::Result<()> {
    if !candidate.is_dir() {
        anyhow::bail!("Thor Shield directory is missing: {}", candidate.display());
    }
    for asset in REQUIRED_SHIELD_ASSETS {
        let path = candidate.join(asset);
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("Thor Shield asset is missing: {}", path.display()))?;
        if !metadata.is_file() || metadata.len() == 0 {
            anyhow::bail!("Thor Shield asset is invalid: {}", path.display());
        }
    }

    let manifest_path = candidate.join("manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path)?)
        .with_context(|| format!("invalid Thor Shield manifest: {}", manifest_path.display()))?;
    let has_main_world_script = manifest["content_scripts"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|script| {
            script["run_at"] == "document_start"
                && script["world"] == "MAIN"
                && script["all_frames"] == true
                && script["js"]
                    .as_array()
                    .is_some_and(|files| files.iter().any(|file| file == "fingerprint.js"))
        });
    let has_required_ruleset = manifest["declarative_net_request"]["rule_resources"]
        .as_array()
        .is_some_and(|rulesets| {
            rulesets.iter().any(|ruleset| {
                ruleset["id"] == "thor_shield_rules"
                    && ruleset["enabled"] == true
                    && ruleset["path"] == "rules.json"
            })
        });
    if manifest["manifest_version"] != 3
        || manifest["background"]["service_worker"] != "service_worker.js"
        || !has_main_world_script
        || !has_required_ruleset
    {
        anyhow::bail!(
            "Thor Shield manifest does not enforce its required MV3 worker, ruleset, and MAIN-world document_start script"
        );
    }

    let rules_path = candidate.join("rules.json");
    let rules: serde_json::Value = serde_json::from_slice(&std::fs::read(&rules_path)?)
        .with_context(|| format!("invalid Thor Shield rules: {}", rules_path.display()))?;
    if rules.as_array().is_none_or(|rules| rules.is_empty()) {
        anyhow::bail!("Thor Shield ruleset is empty: {}", rules_path.display());
    }
    crate::net::SecurityShield::verify_filter_bundle(candidate)
        .map_err(anyhow::Error::msg)
        .context("Thor Shield signed filter bundle verification failed")?;
    let fingerprint = std::fs::read_to_string(candidate.join("fingerprint.js"))?;
    if fingerprint.contains("Thor") {
        anyhow::bail!("Thor Shield fingerprint protection exposes a Thor-specific marker");
    }
    Ok(())
}

pub(crate) fn thor_shield_extension_path() -> anyhow::Result<PathBuf> {
    if let Ok(path) = std::env::var("THOR_SHIELD_EXTENSION_PATH") {
        let candidate = PathBuf::from(path);
        validate_thor_shield_extension(&candidate)?;
        return Ok(candidate);
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(contents) = executable.parent().and_then(Path::parent) {
            candidates.push(contents.join("Resources").join("ThorShield"));
        }
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("browser")
            .join("extensions")
            .join("thor-shield"),
    );

    for candidate in &candidates {
        if candidate.exists() {
            validate_thor_shield_extension(candidate)?;
            return Ok(candidate.clone());
        }
    }
    let searched = candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    anyhow::bail!("Thor Shield is mandatory but unavailable; searched: {searched}")
}

#[derive(Debug, PartialEq, Eq)]
struct DevToolsActivePort {
    port: u16,
    browser_path: String,
}

fn parse_devtools_active_port(contents: &[u8]) -> anyhow::Result<DevToolsActivePort> {
    let contents = std::str::from_utf8(contents).context("DevToolsActivePort is not UTF-8")?;
    let mut lines = contents.lines();
    let port = lines
        .next()
        .context("DevToolsActivePort is missing its port")?
        .trim()
        .parse::<u16>()
        .context("DevToolsActivePort contains an invalid port")?;
    if port == 0 {
        anyhow::bail!("DevToolsActivePort contains port 0");
    }
    let browser_path = lines
        .next()
        .context("DevToolsActivePort is missing its browser WebSocket path")?
        .trim()
        .to_string();
    if !browser_path.starts_with("/devtools/browser/") {
        anyhow::bail!("DevToolsActivePort contains an invalid browser WebSocket path");
    }
    Ok(DevToolsActivePort { port, browser_path })
}

fn devtools_candidate(
    user_data: &Path,
    requested_port: u16,
    previous_active_port: Option<&[u8]>,
) -> anyhow::Result<Option<(String, Option<String>)>> {
    if requested_port != 0 {
        return Ok(Some((format!("http://127.0.0.1:{requested_port}"), None)));
    }

    let contents = match std::fs::read(user_data.join("DevToolsActivePort")) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("could not read DevToolsActivePort"),
    };
    if previous_active_port == Some(contents.as_slice()) {
        return Ok(None);
    }
    let Ok(active) = parse_devtools_active_port(&contents) else {
        // Chromium may expose the file between truncate/write operations.
        // Treat an incomplete value as not ready and retry until the deadline.
        return Ok(None);
    };
    Ok(Some((
        format!("http://127.0.0.1:{}", active.port),
        Some(active.browser_path),
    )))
}

fn wait_for_devtools(
    child: &mut Child,
    user_data: &Path,
    requested_port: u16,
    previous_active_port: Option<&[u8]>,
    timeout: Duration,
) -> anyhow::Result<String> {
    let deadline = std::time::Instant::now() + timeout;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    while std::time::Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("Thor Chromium exited before DevTools became ready ({status})");
        }
        if let Some((http_base, expected_browser_path)) =
            devtools_candidate(user_data, requested_port, previous_active_port)?
        {
            let url = format!("{http_base}/json/version");
            let ok = rt.block_on(async {
                let Ok(resp) = reqwest::get(&url).await else {
                    return false;
                };
                if !resp.status().is_success() {
                    return false;
                }
                let Ok(version) = resp.json::<serde_json::Value>().await else {
                    return false;
                };
                match expected_browser_path.as_deref() {
                    Some(path) => version["webSocketDebuggerUrl"]
                        .as_str()
                        .is_some_and(|ws| ws.ends_with(path)),
                    None => true,
                }
            });
            if ok {
                return Ok(http_base);
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if requested_port == 0 {
        anyhow::bail!(
            "timed out waiting for a fresh DevToolsActivePort in {}",
            user_data.display()
        )
    } else {
        anyhow::bail!("timed out waiting for DevTools at http://127.0.0.1:{requested_port}")
    }
}

async fn attach_labs(debug_http: &str) -> anyhow::Result<()> {
    let runtime = crate::engine::self_cdp::connect_runtime(debug_http).await?;
    crate::thor_info!(
        "thor::product",
        "Self-CDP connected ({})",
        runtime.websocket_url()
    );
    let (engine, handle) =
        crate::engine::engine_coordinator::BrowserEngine::attach_existing(runtime)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
    AppState::global().set_engine_handle(handle.clone());
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
    fn owned_candidates_include_source_build_and_frameworks() {
        let c = owned_chromium_candidates();
        assert!(c.iter().any(|p| p.to_string_lossy().contains("out/Thor")));
        assert!(c.iter().any(|p| p.to_string_lossy().contains("Frameworks")));
        assert!(!c
            .iter()
            .any(|p| p.to_string_lossy().contains("Content Shell")));
    }

    #[test]
    fn stock_bundle_without_thor_marker_is_rejected() {
        let temp =
            std::env::temp_dir().join(format!("thor-product-marker-test-{}", std::process::id()));
        let executable = temp.join("Chromium.app/Contents/MacOS/Chromium");
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, b"not-a-real-executable").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = executable.metadata().unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&executable, permissions).unwrap();
        }
        std::fs::write(
            temp.join("Chromium.app/Contents/Info.plist"),
            r#"<?xml version="1.0"?><plist><dict><key>CFBundleIdentifier</key><string>com.thor.browser</string></dict></plist>"#,
        )
        .unwrap();
        assert!(!has_thor_product_marker(&executable));

        let marker = temp.join("ThorOwnedChromium.json");
        std::fs::write(&marker, r#"{"product": "Thor"}"#).unwrap();
        assert!(!has_thor_product_marker(&executable));

        let executable_sha256 = sha256_file(&executable).unwrap();
        std::fs::write(
            &marker,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 2,
                "product": "Thor",
                "source": {
                    "repository": "https://chromium.googlesource.com/chromium/src.git",
                    "ref": "151.0.7922.47",
                    "commit": "c583827a354674180dc8856eb65aeb3f0afa5ee9"
                },
                "artifact": {
                    "kind": "chromium_browser_executable",
                    "filename": "Chromium",
                    "size": executable.metadata().unwrap().len(),
                    "sha256": executable_sha256
                }
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(has_thor_product_marker(&executable));

        std::fs::write(&executable, b"tampered-browser").unwrap();
        assert!(!has_thor_product_marker(&executable));
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn lifecycle_hook_names_are_documented() {
        use crate::engine::freezing::ChromiumLifecycleHooks;
        assert!(ChromiumLifecycleHooks::FREEZE.contains("Frozen"));
        assert!(ChromiumLifecycleHooks::DISCARD.contains("discard"));
        assert!(ChromiumLifecycleHooks::EXPIRE.contains("Close"));
    }

    #[test]
    fn shield_is_a_thor_owned_extension() {
        let extension =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("browser/extensions/thor-shield");
        validate_thor_shield_extension(&extension).unwrap();
        let manifest = extension.join("manifest.json");
        assert!(manifest.is_file());
        let contents = std::fs::read_to_string(manifest).unwrap();
        assert!(contents.contains("\"options_ui\""));
        assert!(contents.contains("\"action\""));
        assert!(contents.contains("\"storage\""));
        assert!(contents.contains("\"browsingData\""));
        assert!(contents.contains("\"contentSettings\""));
        assert!(contents.contains("\"run_at\": \"document_start\""));
        assert!(contents.contains("\"world\": \"MAIN\""));
        assert!(extension.join("options.html").is_file());
        assert!(extension.join("options.js").is_file());
        assert!(extension.join("fingerprint.js").is_file());
        let fingerprint = std::fs::read_to_string(extension.join("fingerprint.js")).unwrap();
        assert!(!fingerprint.contains("Thor"));
        assert!(!fingerprint.contains("Thor GPU"));
        let rules = std::fs::read_to_string(extension.join("rules.json")).unwrap();
        assert!(rules.matches("\"type\":\"block\"").count() >= 35);
        assert!(rules.contains("\"type\":\"upgradeScheme\""));
        let worker = std::fs::read_to_string(extension.join("service_worker.js")).unwrap();
        assert!(worker.contains("contentSettings[name].clear"));
        assert!(!worker.contains("block ? 'block' : 'allow'"));
        assert!(!worker.contains("shieldEnabled"));
        let options = std::fs::read_to_string(extension.join("options.html")).unwrap();
        assert!(options.contains("Reset selected overrides"));
    }

    #[test]
    fn shield_validation_fails_closed_when_assets_are_missing() {
        let missing =
            std::env::temp_dir().join(format!("thor-missing-shield-test-{}", std::process::id()));
        assert!(validate_thor_shield_extension(&missing).is_err());
    }

    #[test]
    fn debugging_port_defaults_to_ephemeral_and_accepts_explicit_port() {
        assert_eq!(parse_debugging_port(None).unwrap(), 0);
        assert_eq!(parse_debugging_port(Some("9222")).unwrap(), 9222);
        assert!(parse_debugging_port(Some("not-a-port")).is_err());
    }

    #[test]
    fn devtools_active_port_requires_port_and_browser_path() {
        assert_eq!(
            parse_devtools_active_port(b"43123\n/devtools/browser/test-id\n").unwrap(),
            DevToolsActivePort {
                port: 43123,
                browser_path: "/devtools/browser/test-id".into(),
            }
        );
        assert!(parse_devtools_active_port(b"0\n/devtools/browser/test-id\n").is_err());
        assert!(parse_devtools_active_port(b"43123\n/not-devtools\n").is_err());
    }

    #[test]
    fn ephemeral_devtools_discovery_ignores_stale_and_partial_files() {
        let temp = std::env::temp_dir().join(format!(
            "thor-devtools-active-port-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        let stale = b"43123\n/devtools/browser/stale\n";
        std::fs::write(temp.join("DevToolsActivePort"), stale).unwrap();
        assert_eq!(devtools_candidate(&temp, 0, Some(stale)).unwrap(), None);

        std::fs::write(temp.join("DevToolsActivePort"), b"44234\n").unwrap();
        assert_eq!(devtools_candidate(&temp, 0, Some(stale)).unwrap(), None);

        std::fs::write(
            temp.join("DevToolsActivePort"),
            b"44234\n/devtools/browser/fresh\n",
        )
        .unwrap();
        assert_eq!(
            devtools_candidate(&temp, 0, Some(stale)).unwrap(),
            Some((
                "http://127.0.0.1:44234".into(),
                Some("/devtools/browser/fresh".into()),
            ))
        );
        let _ = std::fs::remove_dir_all(temp);
    }
}
