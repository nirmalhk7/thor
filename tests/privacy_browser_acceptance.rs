//! Live, fail-closed acceptance checks for Thor's browser privacy boundary.
//!
//! This test is ignored during ordinary `cargo test` because it launches a real
//! Chromium-downstream browser. Selecting the ignored test without a verified
//! Thor-owned source/package binary is an error, never a skip.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use chromiumoxide::cdp::browser_protocol::browser::{
    GetBrowserCommandLineParams, PermissionSetting, SetPermissionParams,
};
use serde_json::Value;
use thor::engine::browser_defaults::{ThorBrowserDefaults, WebRtcIpPolicy};
use thor::engine::chromium_runtime::ChromiumRuntime;
use thor::engine::engine_coordinator::{BrowserEngine, EngineCommand};
use thor::engine::tab_session::TabSession;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

const BINARY_ENV: &str = "THOR_PRIVACY_ACCEPTANCE_BINARY";
const WEBRTC_POLICY_ARG: &str = "--force-webrtc-ip-handling-policy=disable_non_proxied_udp";
const PRIVACY_PROFILE_ARG: &str = "--thor-privacy-profile=stealth";
const CDP_START_TIMEOUT: Duration = Duration::from_secs(30);
const UNVERIFIED_IDENTITY_BOUNDARIES: &[(&str, &str)] = &[
    (
        "HSTS state",
        "the current loopback/CDP harness has no trusted HTTPS origin or HSTS-state API",
    ),
    (
        "HTTP authentication cache",
        "Thor's public engine API does not expose per-context auth-cache state",
    ),
    (
        "socket and network connection pools",
        "CDP does not expose pool ownership strongly enough to prove non-sharing",
    ),
    (
        "startup orphan BrowserContext cleanup",
        "attach_existing intentionally adopts unknown contexts as unowned and exposes no ownership-safe cleanup API",
    ),
];
const UNVERIFIED_RELEASE_BOUNDARIES: &[(&str, &str)] = &[
    (
        "Safe Browsing and component readiness",
        "requires update-service fixtures and browser-process network observation",
    ),
    (
        "HTTPS-Only and mixed-content interstitials",
        "loopback HTTP is exempt from normal upgrade behavior and no trusted HTTPS fixture exists",
    ),
    (
        "third-party cookies and persistent history",
        "the harness currently probes first-party ephemeral contexts and removes its profile",
    ),
    (
        "DoH health and browser-process egress",
        "command-line readback proves requested configuration, not resolver health or all browser traffic",
    ),
    (
        "filter effect and CNAME uncloaking",
        "the bundled extension is loaded but the harness has no signed filter corpus or DNS fixture",
    ),
    (
        "native identity browser-chrome commands",
        "headless CDP context tests do not exercise the New Identity or Destroy Identity menu commands",
    ),
    (
        "profile API/JIT and WebGL policy",
        "the harness has not yet observed DevHost, Stealth, and Hardened renderer surfaces",
    ),
    (
        "site isolation, compiler hardening, CA, and downloads",
        "these require process, build-metadata, OS trust, and download fixtures not present here",
    ),
];

#[derive(Debug)]
enum OwnedLayout {
    Source {
        chromium_src: PathBuf,
        chromium_root: PathBuf,
        out_dir: PathBuf,
    },
    Package {
        outer_app: PathBuf,
    },
}

#[derive(Debug)]
struct OwnedBuild {
    executable: PathBuf,
    browser_app: PathBuf,
    marker: PathBuf,
    shield: PathBuf,
    layout: OwnedLayout,
}

impl OwnedBuild {
    fn from_environment() -> Result<Self> {
        let raw = std::env::var_os(BINARY_ENV).ok_or_else(|| {
            anyhow!(
                "{BINARY_ENV} is required when this ignored acceptance test is selected; \
                 use scripts/run_privacy_acceptance.sh"
            )
        })?;
        let supplied = PathBuf::from(raw);
        let metadata = fs::symlink_metadata(&supplied).with_context(|| {
            format!(
                "Thor privacy acceptance binary is missing: {}",
                supplied.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            bail!(
                "Thor privacy acceptance binary must not be supplied through a symlink: {}",
                supplied.display()
            );
        }
        if !metadata.is_file() {
            bail!(
                "Thor privacy acceptance binary is not a file: {}",
                supplied.display()
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                bail!(
                    "Thor privacy acceptance binary is not executable: {}",
                    supplied.display()
                );
            }
        }

        let executable = supplied
            .canonicalize()
            .with_context(|| format!("cannot canonicalize Thor binary: {}", supplied.display()))?;
        let macos_dir = executable
            .parent()
            .filter(|path| path.file_name().is_some_and(|name| name == "MacOS"))
            .ok_or_else(|| anyhow!("binary is not inside a macOS Chromium application bundle"))?;
        let contents_dir = macos_dir
            .parent()
            .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
            .ok_or_else(|| {
                anyhow!("binary is not inside an application Contents/MacOS directory")
            })?;
        let inner_app = contents_dir
            .parent()
            .ok_or_else(|| anyhow!("browser application bundle has no parent"))?;
        if !matches!(
            inner_app.file_name().and_then(|name| name.to_str()),
            Some("Thor.app" | "Chromium.app")
        ) {
            bail!(
                "browser bundle must be Thor.app or Chromium.app, found {}",
                inner_app.display()
            );
        }

        let marker_dir = inner_app
            .parent()
            .ok_or_else(|| anyhow!("browser bundle has no output directory"))?;
        let source_marker = marker_dir.join("ThorOwnedChromium.json");
        let source_shield = marker_dir.join("ThorShield");
        if source_marker.is_file() && source_shield.is_dir() {
            let chromium_src = marker_dir
                .ancestors()
                .find(|path| path.join(".git").exists() && path.file_name().is_some_and(|n| n == "src"))
                .map(Path::to_path_buf)
                .ok_or_else(|| {
                    anyhow!(
                        "source browser marker exists, but no containing Chromium src Git checkout was found"
                    )
                })?;
            let chromium_root = chromium_src
                .parent()
                .ok_or_else(|| anyhow!("Chromium src checkout has no parent"))?
                .to_path_buf();
            let out_dir = marker_dir
                .strip_prefix(&chromium_src)
                .context("source browser output is outside its Chromium src checkout")?
                .to_path_buf();
            let build = Self {
                executable: executable.clone(),
                browser_app: inner_app.to_path_buf(),
                marker: source_marker,
                shield: source_shield,
                layout: OwnedLayout::Source {
                    chromium_src,
                    chromium_root,
                    out_dir,
                },
            };
            build.verify()?;
            return Ok(build);
        }

        let frameworks_dir = marker_dir;
        if frameworks_dir
            .file_name()
            .is_none_or(|name| name != "Frameworks")
        {
            bail!(
                "binary has neither source-build provenance nor packaged Frameworks layout: {}",
                executable.display()
            );
        }
        let outer_contents = frameworks_dir
            .parent()
            .filter(|path| path.file_name().is_some_and(|name| name == "Contents"))
            .ok_or_else(|| {
                anyhow!("packaged Chromium is not under Thor.app/Contents/Frameworks")
            })?;
        let outer_app = outer_contents
            .parent()
            .filter(|path| path.file_name().is_some_and(|name| name == "Thor.app"))
            .ok_or_else(|| anyhow!("packaged Chromium outer bundle must be named Thor.app"))?
            .to_path_buf();
        let resources = outer_contents.join("Resources");
        let build = Self {
            executable: executable.clone(),
            browser_app: inner_app.to_path_buf(),
            marker: resources.join("ThorOwnedChromium.json"),
            shield: resources.join("ThorShield"),
            layout: OwnedLayout::Package { outer_app },
        };
        build.verify()?;
        Ok(build)
    }

    fn verify(&self) -> Result<()> {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let browser_plist = self.browser_app.join("Contents/Info.plist");
        if plist_value(&browser_plist, "CFBundleIdentifier")? != "com.thor.browser" {
            bail!(
                "selected Chromium bundle is not branded com.thor.browser: {}",
                self.browser_app.display()
            );
        }
        let bundle_executable = plist_value(&browser_plist, "CFBundleExecutable")?;
        let declared_executable = self
            .browser_app
            .join("Contents/MacOS")
            .join(bundle_executable)
            .canonicalize()
            .context("selected browser's declared CFBundleExecutable is missing")?;
        if declared_executable != self.executable {
            bail!(
                "selected binary differs from the browser bundle's CFBundleExecutable: {}",
                self.executable.display()
            );
        }
        if !self.marker.is_file() {
            bail!(
                "Thor-owned provenance marker is missing: {}",
                self.marker.display()
            );
        }
        for asset in [
            "manifest.json",
            "service_worker.js",
            "fingerprint.js",
            "rules.json",
            "filter_bundle.json",
            "options.html",
            "options.js",
        ] {
            let path = self.shield.join(asset);
            if !path.is_file() || path.metadata()?.len() == 0 {
                bail!(
                    "verified Thor Shield asset is missing or empty: {}",
                    path.display()
                );
            }
        }

        let mut provenance = StdCommand::new("python3");
        provenance
            .arg(repo.join("scripts/generate_thor_provenance.py"))
            .arg("--shield-dir")
            .arg(&self.shield)
            .arg("--browser-executable")
            .arg(&self.executable)
            .arg("--check")
            .arg(&self.marker);
        if let OwnedLayout::Source { chromium_src, .. } = &self.layout {
            provenance.arg("--chromium-src").arg(chromium_src);
        }
        require_success(provenance, "Thor provenance verification")?;

        match &self.layout {
            OwnedLayout::Source {
                chromium_root,
                out_dir,
                ..
            } => {
                let mut verifier = StdCommand::new(repo.join("scripts/verify_thor_downstream.sh"));
                verifier
                    .env("THOR_CHROMIUM_SRC", chromium_root)
                    .env("THOR_CHROMIUM_OUT", out_dir);
                require_success(verifier, "Thor source downstream verification")?;
            }
            OwnedLayout::Package { outer_app } => {
                if plist_value(&outer_app.join("Contents/Info.plist"), "CFBundleIdentifier")?
                    != "com.thor.browser"
                {
                    bail!(
                        "packaged outer application is not com.thor.browser: {}",
                        outer_app.display()
                    );
                }
            }
        }
        Ok(())
    }
}

fn plist_value(plist: &Path, key: &str) -> Result<String> {
    if !plist.is_file() {
        bail!(
            "Thor application Info.plist is missing: {}",
            plist.display()
        );
    }
    let output = StdCommand::new("plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(plist)
        .output()
        .with_context(|| format!("failed to inspect {key} in {}", plist.display()))?;
    if !output.status.success() {
        bail!(
            "failed to inspect {key} in {}: {}",
            plist.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)
        .context("plutil returned non-UTF-8 application metadata")?
        .trim()
        .to_string())
}

fn require_success(mut command: StdCommand, label: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("failed to execute {label}"))?;
    if !output.status.success() {
        bail!(
            "{label} failed ({}):\n{}{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

struct LiveBrowser {
    child: Option<Child>,
    profile: PathBuf,
}

impl LiveBrowser {
    async fn launch(build: &OwnedBuild) -> Result<(Self, String)> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before Unix epoch")?
            .as_nanos();
        let profile = std::env::temp_dir().join(format!(
            "thor-privacy-acceptance-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&profile)
            .with_context(|| format!("failed to create isolated profile: {}", profile.display()))?;

        let mut command = Command::new(&build.executable);
        command
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--remote-debugging-address=127.0.0.1")
            .arg("--remote-debugging-port=0")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--no-pings")
            .arg(PRIVACY_PROFILE_ARG)
            .arg(WEBRTC_POLICY_ARG)
            .arg("--dns-over-https-mode=secure")
            .arg("--dns-over-https-templates=https://cloudflare-dns.com/dns-query")
            .arg(format!("--load-extension={}", build.shield.display()))
            .arg("--enable-automation")
            .arg("--headless=new")
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let child = command.spawn().with_context(|| {
            format!(
                "failed to launch Thor-owned browser: {}",
                build.executable.display()
            )
        })?;
        let mut live = Self {
            child: Some(child),
            profile,
        };
        let debug_url = live.wait_for_debug_endpoint().await?;
        Ok((live, debug_url))
    }

    async fn wait_for_debug_endpoint(&mut self) -> Result<String> {
        let active_port = self.profile.join("DevToolsActivePort");
        timeout(CDP_START_TIMEOUT, async {
            loop {
                if let Some(status) = self
                    .child
                    .as_mut()
                    .ok_or_else(|| anyhow!("Thor browser child is unavailable"))?
                    .try_wait()
                    .context("failed checking Thor browser status")?
                {
                    bail!("Thor browser exited before CDP became ready: {status}");
                }
                if let Ok(contents) = fs::read_to_string(&active_port) {
                    let mut lines = contents.lines();
                    let port = lines
                        .next()
                        .ok_or_else(|| anyhow!("DevToolsActivePort has no port"))?;
                    let browser_path = lines.next().ok_or_else(|| {
                        anyhow!("DevToolsActivePort has no browser WebSocket path")
                    })?;
                    port.parse::<u16>()
                        .context("DevToolsActivePort contains an invalid port")?;
                    if !browser_path.starts_with("/devtools/browser/") {
                        bail!("DevToolsActivePort contains an unexpected WebSocket path");
                    }
                    return Ok(format!("ws://127.0.0.1:{port}{browser_path}"));
                }
                sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .map_err(|_| anyhow!("Thor browser did not expose CDP within {CDP_START_TIMEOUT:?}"))?
    }

    async fn stop(mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            if child.try_wait()?.is_none() {
                child.kill().await.context("failed to stop Thor browser")?;
            }
            let _ = child.wait().await;
        }
        remove_acceptance_profile(&self.profile)?;
        self.profile.clear();
        Ok(())
    }
}

impl Drop for LiveBrowser {
    fn drop(&mut self) {
        if let Some(child) = self.child.as_mut() {
            let _ = child.start_kill();
        }
        if !self.profile.as_os_str().is_empty() {
            let _ = remove_acceptance_profile(&self.profile);
        }
    }
}

fn remove_acceptance_profile(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if path.parent() != Some(std::env::temp_dir().as_path())
        || !name.starts_with("thor-privacy-acceptance-")
    {
        bail!(
            "refusing to remove unexpected acceptance profile path: {}",
            path.display()
        );
    }
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to remove acceptance profile: {}", path.display())),
    }
}

#[derive(Clone, Debug)]
struct RequestObservation {
    path: String,
    headers: BTreeMap<String, String>,
}

struct LoopbackServer {
    address: SocketAddr,
    observations: Arc<Mutex<Vec<RequestObservation>>>,
    task: JoinHandle<()>,
}

impl LoopbackServer {
    async fn start() -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .context("failed to bind privacy acceptance loopback server")?;
        let address = listener.local_addr()?;
        let observations = Arc::new(Mutex::new(Vec::new()));
        let server_observations = Arc::clone(&observations);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let observations = Arc::clone(&server_observations);
                tokio::spawn(async move {
                    let _ = serve_connection(stream, observations).await;
                });
            }
        });
        Ok(Self {
            address,
            observations,
            task,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    fn observation(&self, path: &str) -> Result<RequestObservation> {
        self.observations
            .lock()
            .map_err(|_| anyhow!("loopback observation lock was poisoned"))?
            .iter()
            .find(|request| request.path == path)
            .cloned()
            .ok_or_else(|| anyhow!("loopback server did not observe request {path}"))
    }
}

impl Drop for LoopbackServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_connection(
    mut stream: TcpStream,
    observations: Arc<Mutex<Vec<RequestObservation>>>,
) -> Result<()> {
    let mut request = Vec::with_capacity(4096);
    let mut buffer = [0_u8; 2048];
    while request.len() < 64 * 1024 && !request.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = timeout(Duration::from_secs(5), stream.read(&mut buffer))
            .await
            .context("loopback request read timed out")??;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
    }
    let request = String::from_utf8(request).context("loopback request was not UTF-8 HTTP/1")?;
    let mut lines = request.split("\r\n");
    let request_line = lines.next().ok_or_else(|| anyhow!("empty HTTP request"))?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow!("malformed HTTP request line"))?
        .to_string();
    let mut headers = BTreeMap::new();
    for line in lines.take_while(|line| !line.is_empty()) {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    observations
        .lock()
        .map_err(|_| anyhow!("loopback observation lock was poisoned"))?
        .push(RequestObservation {
            path: path.clone(),
            headers,
        });

    let (status, content_type, body, extra_headers) = if path
        .starts_with("/gpc-dedicated-worker.js")
    {
        (
            "200 OK",
            "application/javascript",
            r#"const token = new URL(self.location.href).searchParams.get('token');
fetch(`/observe-dedicated-worker?token=${encodeURIComponent(token)}`, {cache: 'no-store'})
  .then(response => postMessage({globalPrivacyControl: navigator.globalPrivacyControl, fetchOk: response.ok}));"#,
            "",
        )
    } else if path.starts_with("/gpc-shared-worker.js") {
        (
            "200 OK",
            "application/javascript",
            r#"const token = new URL(self.location.href).searchParams.get('token');
self.onconnect = event => {
  const port = event.ports[0];
  fetch(`/observe-shared-worker?token=${encodeURIComponent(token)}`, {cache: 'no-store'})
    .then(response => port.postMessage({globalPrivacyControl: navigator.globalPrivacyControl, fetchOk: response.ok}));
};"#,
            "",
        )
    } else if path.starts_with("/gpc-service-worker.js") {
        (
            "200 OK",
            "application/javascript",
            r#"self.addEventListener('message', event => {
  const token = event.data.token;
  event.waitUntil(
    fetch(`/observe-service-worker?token=${encodeURIComponent(token)}`, {cache: 'no-store'})
      .then(response => event.source.postMessage({
        type: 'thor-service-worker-gpc',
        token,
        globalPrivacyControl: navigator.globalPrivacyControl,
        fetchOk: response.ok
      }))
  );
});"#,
            "Service-Worker-Allowed: /\r\n",
        )
    } else if path.starts_with("/sw.js") {
        (
            "200 OK",
            "application/javascript",
            "self.addEventListener('fetch', () => {});",
            "Service-Worker-Allowed: /\r\n",
        )
    } else if path.starts_with("/observe") {
        ("200 OK", "application/json", "{}", "")
    } else {
        (
                "200 OK",
                "text/html; charset=utf-8",
                "<!doctype html><meta charset=utf-8><title>Thor privacy acceptance</title>",
                "Content-Security-Policy: default-src 'self'; img-src 'none'; object-src 'none'; base-uri 'none'\r\n",
            )
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\n{extra_headers}Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}

async fn evaluate(tab: &TabSession, expression: &str, label: &str) -> Result<Value> {
    timeout(Duration::from_secs(15), tab.evaluate_js(expression))
        .await
        .with_context(|| format!("browser timed out during {label} JavaScript"))?
        .with_context(|| format!("browser rejected {label} JavaScript"))
}

async fn wait_for_document_ready(tab: &TabSession, label: &str) -> Result<()> {
    timeout(Duration::from_secs(15), async {
        loop {
            if evaluate(tab, "document.readyState === 'complete'", label).await?
                == Value::Bool(true)
            {
                return Ok(());
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .with_context(|| format!("{label} document did not reach readyState=complete"))?
}

async fn initialize_identity(tab: &TabSession, label: &str) -> Result<Value> {
    let script = format!(
        r#"(async () => {{
          const label = {label:?};
          document.cookie = `thor_identity=${{label}}; Path=/; SameSite=Lax`;
          localStorage.setItem('thor_identity', label);
          sessionStorage.setItem('thor_identity', label);
          await new Promise((resolve, reject) => {{
            const request = indexedDB.open('thor_identity_db', 1);
            request.onupgradeneeded = () => request.result.createObjectStore('state');
            request.onerror = () => reject(request.error);
            request.onsuccess = () => {{
              const db = request.result;
              const transaction = db.transaction('state', 'readwrite');
              transaction.objectStore('state').put(label, 'owner');
              transaction.onerror = () => reject(transaction.error);
              transaction.oncomplete = () => {{ db.close(); resolve(); }};
            }};
          }});
          const cache = await caches.open('thor_identity_cache');
          await cache.put('/identity-cache-entry', new Response(label));
          const registration = await navigator.serviceWorker.register(
            `/sw.js?owner=${{encodeURIComponent(label)}}`,
            {{ scope: `/identity-${{label.toLowerCase()}}/` }}
          );
          await navigator.serviceWorker.ready;
          const blobUrl = URL.createObjectURL(
            new Blob([label], {{ type: 'text/plain' }})
          );
          return {{ registeredScope: registration.scope, blobUrl }};
        }})()"#
    );
    evaluate(tab, &script, &format!("identity {label} initialization")).await
}

async fn identity_state(tab: &TabSession) -> Result<Value> {
    evaluate(
        tab,
        r#"(async () => {
          const cacheNames = (await caches.keys()).sort();
          let cacheValue = null;
          if (cacheNames.includes('thor_identity_cache')) {
            const cached = await (await caches.open('thor_identity_cache'))
              .match('/identity-cache-entry');
            if (cached) cacheValue = await cached.text();
          }
          return {
            cookie: document.cookie,
            localStorage: localStorage.getItem('thor_identity'),
            sessionStorage: sessionStorage.getItem('thor_identity'),
            databases: (await indexedDB.databases()).map(database => database.name).sort(),
            cacheNames,
            cacheValue,
            permission: (await navigator.permissions.query({name: 'geolocation'})).state,
            serviceWorkers: (await navigator.serviceWorker.getRegistrations())
              .map(registration => registration.scope).sort()
          };
        })()"#,
        "identity state inspection",
    )
    .await
}

fn require_identity_state(
    state: &Value,
    label: &str,
    server: &LoopbackServer,
    expected_permission: &str,
) -> Result<()> {
    let cookie = state["cookie"]
        .as_str()
        .ok_or_else(|| anyhow!("identity state cookie is not a string: {state}"))?;
    if !cookie
        .split(';')
        .any(|part| part.trim() == format!("thor_identity={label}"))
    {
        bail!("identity {label} cookie is absent or contaminated: {cookie}");
    }
    if state["localStorage"] != Value::String(label.to_string()) {
        bail!("identity {label} localStorage mismatch: {state}");
    }
    if state["sessionStorage"] != Value::String(label.to_string()) {
        bail!("identity {label} sessionStorage mismatch: {state}");
    }
    let databases = state["databases"]
        .as_array()
        .ok_or_else(|| anyhow!("identity databases result is not an array: {state}"))?;
    if databases != &[Value::String("thor_identity_db".into())] {
        bail!("identity {label} IndexedDB visibility mismatch: {databases:?}");
    }
    if state["cacheNames"] != Value::Array(vec![Value::String("thor_identity_cache".into())])
        || state["cacheValue"] != Value::String(label.to_string())
    {
        bail!("identity {label} CacheStorage visibility mismatch: {state}");
    }
    if state["permission"] != Value::String(expected_permission.to_string()) {
        bail!("identity {label} permission state mismatch: {state}");
    }
    let workers = state["serviceWorkers"]
        .as_array()
        .ok_or_else(|| anyhow!("identity service-worker result is not an array: {state}"))?;
    let expected_scope = format!(
        "http://{}/identity-{}/",
        server.address,
        label.to_ascii_lowercase()
    );
    if workers != &[Value::String(expected_scope)] {
        bail!("identity {label} service-worker visibility mismatch: {workers:?}");
    }
    Ok(())
}

fn require_empty_identity_state(state: &Value, expected_permission: &str) -> Result<()> {
    if state["cookie"] != ""
        || !state["localStorage"].is_null()
        || !state["sessionStorage"].is_null()
        || state["databases"] != Value::Array(Vec::new())
        || state["cacheNames"] != Value::Array(Vec::new())
        || !state["cacheValue"].is_null()
        || state["permission"] != Value::String(expected_permission.to_string())
        || state["serviceWorkers"] != Value::Array(Vec::new())
    {
        bail!("fresh identity inherited another BrowserContext's state: {state}");
    }
    Ok(())
}

async fn check_startup_and_process_policy(
    runtime: &Arc<ChromiumRuntime>,
    build: &OwnedBuild,
) -> Result<TabSession> {
    let mut final_startup_tabs = None;
    for observation in 0..20 {
        let startup_tabs = runtime.existing_tabs().await?;
        if startup_tabs.is_empty() {
            bail!("Thor startup produced no about:blank page target");
        }
        for tab in &startup_tabs {
            let url = tab
                .url()
                .await?
                .ok_or_else(|| anyhow!("startup page target has no URL"))?;
            if url != "about:blank" {
                bail!("Thor startup created an unsolicited page target: {url}");
            }
        }
        final_startup_tabs = Some(startup_tabs);
        if observation != 19 {
            sleep(Duration::from_millis(100)).await;
        }
    }
    let startup_tabs = final_startup_tabs
        .expect("startup stabilization loop always performs at least one observation");

    let browser = runtime.browser();
    let command_line = browser
        .lock()
        .await
        .execute(GetBrowserCommandLineParams::default())
        .await
        .context("Browser.getBrowserCommandLine was unavailable")?
        .result
        .arguments;
    for required in [
        WEBRTC_POLICY_ARG.to_string(),
        PRIVACY_PROFILE_ARG.to_string(),
        "--dns-over-https-mode=secure".to_string(),
        format!("--load-extension={}", build.shield.display()),
    ] {
        if !command_line.iter().any(|argument| argument == &required) {
            bail!("live Thor browser command line is missing required policy: {required}");
        }
    }
    if command_line
        .iter()
        .any(|argument| argument == "--no-sandbox")
    {
        bail!("privacy acceptance browser unexpectedly disabled Chromium sandbox");
    }

    Ok(startup_tabs
        .into_iter()
        .next()
        .expect("startup tab vector was checked as nonempty"))
}

async fn check_gpc_dnt_and_webrtc(tab: &TabSession, server: &LoopbackServer) -> Result<()> {
    let token = format!("{}", std::process::id());
    tab.navigate(&server.url(&format!("/signals?token={token}")))
        .await?;
    let signals = evaluate(
        tab,
        r#"({
          globalPrivacyControl: navigator.globalPrivacyControl,
          doNotTrack: navigator.doNotTrack
        })"#,
        "GPC and DNT observation",
    )
    .await?;
    if signals["globalPrivacyControl"] != Value::Bool(true) {
        bail!("navigator.globalPrivacyControl is not true: {signals}");
    }
    if signals["doNotTrack"] != Value::String("1".into()) {
        bail!("navigator.doNotTrack is not \"1\": {signals}");
    }

    let observation_path = format!("/observe?token={token}");
    let fetch_script = format!(
        r#"fetch({observation_path:?}, {{ cache: 'no-store', credentials: 'include' }})
             .then(response => response.ok)"#
    );
    if evaluate(tab, &fetch_script, "GPC/DNT request header observation").await?
        != Value::Bool(true)
    {
        bail!("loopback GPC/DNT observation request failed");
    }
    let observation = server.observation(&observation_path)?;
    if observation.headers.get("sec-gpc").map(String::as_str) != Some("1") {
        bail!("live request did not carry Sec-GPC: 1: {observation:?}");
    }
    if observation.headers.get("dnt").map(String::as_str) != Some("1") {
        bail!("live request did not carry DNT: 1: {observation:?}");
    }

    check_native_query_cleanup(tab, server).await?;

    check_native_worker_gpc(tab, server, &token).await?;

    let candidates = evaluate(
        tab,
        r#"new Promise(async (resolve, reject) => {
          const peer = new RTCPeerConnection({ iceServers: [] });
          const candidates = [];
          const timer = setTimeout(() => { peer.close(); resolve(candidates); }, 5000);
          peer.onicecandidate = event => {
            if (event.candidate) {
              candidates.push(event.candidate.candidate);
            } else {
              clearTimeout(timer);
              peer.close();
              resolve(candidates);
            }
          };
          try {
            peer.createDataChannel('thor-privacy-acceptance');
            await peer.setLocalDescription(await peer.createOffer());
          } catch (error) {
            clearTimeout(timer);
            peer.close();
            reject(error);
          }
        })"#,
        "WebRTC candidate policy observation",
    )
    .await?;
    let candidates = candidates
        .as_array()
        .ok_or_else(|| anyhow!("WebRTC observation did not return a candidate array"))?;
    if let Some(candidate) = candidates
        .iter()
        .filter_map(Value::as_str)
        .find(|candidate| {
            candidate
                .split_whitespace()
                .nth(2)
                .is_some_and(|protocol| protocol.eq_ignore_ascii_case("udp"))
        })
    {
        bail!("WebRTC exposed a non-proxied UDP candidate under strict policy: {candidate}");
    }
    Ok(())
}

async fn check_native_query_cleanup(tab: &TabSession, server: &LoopbackServer) -> Result<()> {
    let dirty_path = "/cleanup?utm_source=acceptance&gclid=discard&keep=1";
    let clean_path = "/cleanup?keep=1";
    tab.navigate(&server.url(dirty_path))
        .await
        .context("native query-cleanup navigation failed")?;
    if server.observation(clean_path).is_err() {
        bail!(
            "live navigation did not remove known tracking parameters while preserving the query: {}",
            clean_path
        );
    }
    if server.observation(dirty_path).is_ok() {
        bail!("live navigation sent the uncleaned tracking query: {dirty_path}");
    }
    Ok(())
}

async fn check_native_worker_gpc(
    tab: &TabSession,
    server: &LoopbackServer,
    token: &str,
) -> Result<()> {
    let script = format!(
        r#"(async () => {{
          const token = {token:?};
          const dedicated = new Promise((resolve, reject) => {{
            const worker = new Worker(
              `/gpc-dedicated-worker.js?token=${{encodeURIComponent(token)}}`
            );
            const timer = setTimeout(() => {{
              worker.terminate();
              reject(new Error('dedicated worker GPC timeout'));
            }}, 5000);
            worker.onmessage = event => {{
              clearTimeout(timer);
              worker.terminate();
              resolve(event.data);
            }};
            worker.onerror = error => {{ clearTimeout(timer); reject(error); }};
          }});

          const shared = new Promise((resolve, reject) => {{
            const worker = new SharedWorker(
              `/gpc-shared-worker.js?token=${{encodeURIComponent(token)}}`
            );
            const timer = setTimeout(() => {{
              worker.port.close();
              reject(new Error('shared worker GPC timeout'));
            }}, 5000);
            worker.port.onmessage = event => {{
              clearTimeout(timer);
              worker.port.close();
              resolve(event.data);
            }};
            worker.onerror = error => {{ clearTimeout(timer); reject(error); }};
            worker.port.start();
          }});

          const service = (async () => {{
            const registration = await navigator.serviceWorker.register(
              `/gpc-service-worker.js?token=${{encodeURIComponent(token)}}`,
              {{ scope: '/' }}
            );
            await navigator.serviceWorker.ready;
            try {{
              return await new Promise((resolve, reject) => {{
                const timer = setTimeout(
                  () => reject(new Error('service worker GPC timeout')),
                  5000
                );
                const onMessage = event => {{
                  if (event.data?.type !== 'thor-service-worker-gpc' ||
                      event.data?.token !== token) return;
                  clearTimeout(timer);
                  navigator.serviceWorker.removeEventListener('message', onMessage);
                  resolve(event.data);
                }};
                navigator.serviceWorker.addEventListener('message', onMessage);
                registration.active.postMessage({{ token }});
              }});
            }} finally {{
              await registration.unregister();
            }}
          }})();

          const [dedicatedResult, sharedResult, serviceResult] =
            await Promise.all([dedicated, shared, service]);
          return {{
            dedicated: dedicatedResult,
            shared: sharedResult,
            service: serviceResult
          }};
        }})()"#
    );
    let results = evaluate(tab, &script, "native worker GPC observation").await?;
    for worker in ["dedicated", "shared", "service"] {
        if results[worker]["globalPrivacyControl"] != Value::Bool(true)
            || results[worker]["fetchOk"] != Value::Bool(true)
        {
            bail!("{worker} worker lacks native GPC parity: {results}");
        }
    }

    for path in [
        format!("/gpc-dedicated-worker.js?token={token}"),
        format!("/observe-dedicated-worker?token={token}"),
        format!("/gpc-shared-worker.js?token={token}"),
        format!("/observe-shared-worker?token={token}"),
        format!("/gpc-service-worker.js?token={token}"),
        format!("/observe-service-worker?token={token}"),
    ] {
        let observation = server.observation(&path)?;
        if observation.headers.get("sec-gpc").map(String::as_str) != Some("1") {
            bail!("worker request did not carry Sec-GPC: 1: {observation:?}");
        }
    }
    Ok(())
}

async fn set_context_permission(
    runtime: &Arc<ChromiumRuntime>,
    browser_context_id: &str,
    origin: &str,
    setting: PermissionSetting,
) -> Result<()> {
    let params = SetPermissionParams::builder()
        .permission("geolocation")
        .setting(setting)
        .origin(origin)
        .browser_context_id(browser_context_id.to_owned())
        .build()
        .map_err(|error| anyhow!("invalid CDP permission override: {error}"))?;
    runtime
        .browser()
        .lock()
        .await
        .execute(params)
        .await
        .context("Browser.setPermission rejected identity-scoped override")?;
    Ok(())
}

fn required_string<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| anyhow!("{label} result has no string {key}: {value}"))
}

async fn require_blob_visibility(
    owner: &TabSession,
    other_identity: &TabSession,
    blob_url: &str,
    label: &str,
) -> Result<()> {
    let encoded_url = serde_json::to_string(blob_url)?;
    let fetch_script = format!(
        r#"(async () => {{
          try {{
            const response = await fetch({encoded_url});
            return {{ accessible: true, ok: response.ok, text: await response.text() }};
          }} catch (error) {{
            return {{ accessible: false, error: error.name }};
          }}
        }})()"#
    );
    let owner_result = evaluate(owner, &fetch_script, "own identity blob fetch").await?;
    if owner_result["accessible"] != Value::Bool(true)
        || owner_result["ok"] != Value::Bool(true)
        || owner_result["text"] != Value::String(label.to_string())
    {
        bail!("identity {label} cannot read its own object URL: {owner_result}");
    }
    let other_result = evaluate(other_identity, &fetch_script, "cross-identity blob fetch").await?;
    if other_result["accessible"] != Value::Bool(false) {
        bail!("identity {label} object URL leaked across BrowserContexts: {other_result}");
    }
    evaluate(
        owner,
        &format!("URL.revokeObjectURL({encoded_url})"),
        "object URL revocation",
    )
    .await?;
    Ok(())
}

async fn check_identity_isolation(
    runtime: &Arc<ChromiumRuntime>,
    server: &LoopbackServer,
) -> Result<()> {
    let context_a = runtime.create_identity_context().await?;
    let context_b = runtime.create_identity_context().await?;
    if context_a == context_b {
        bail!("Chromium returned duplicate identity BrowserContext IDs");
    }
    let contexts: BTreeSet<_> = runtime.identity_contexts().await?.into_iter().collect();
    if !contexts.contains(&context_a) || !contexts.contains(&context_b) {
        bail!("created identity BrowserContexts are not both live: {contexts:?}");
    }

    let tab_a = runtime
        .new_tab_in_context(Some(&server.url("/identity-a/page")), Some(&context_a))
        .await?;
    let tab_b = runtime
        .new_tab_in_context(Some(&server.url("/identity-b/page")), Some(&context_b))
        .await?;
    wait_for_document_ready(&tab_a, "identity A").await?;
    wait_for_document_ready(&tab_b, "identity B").await?;
    set_context_permission(
        runtime,
        &context_a,
        &server.origin(),
        PermissionSetting::Granted,
    )
    .await?;
    set_context_permission(
        runtime,
        &context_b,
        &server.origin(),
        PermissionSetting::Denied,
    )
    .await?;

    let initialized_a = initialize_identity(&tab_a, "A").await?;
    let blob_a =
        required_string(&initialized_a, "blobUrl", "identity A initialization")?.to_string();
    require_empty_identity_state(&identity_state(&tab_b).await?, "denied")?;
    require_blob_visibility(&tab_a, &tab_b, &blob_a, "A").await?;

    let initialized_b = initialize_identity(&tab_b, "B").await?;
    let blob_b =
        required_string(&initialized_b, "blobUrl", "identity B initialization")?.to_string();
    require_blob_visibility(&tab_b, &tab_a, &blob_b, "B").await?;
    require_identity_state(&identity_state(&tab_a).await?, "A", server, "granted")?;
    require_identity_state(&identity_state(&tab_b).await?, "B", server, "denied")?;

    tab_a.close().await?;
    tab_b.close().await?;
    runtime.dispose_identity_context(&context_a).await?;
    runtime.dispose_identity_context(&context_b).await?;
    Ok(())
}

async fn require_context_disposed(
    runtime: &Arc<ChromiumRuntime>,
    browser_context_id: &str,
    label: &str,
) -> Result<()> {
    if let Ok(tab) = runtime
        .new_tab_in_context(Some("about:blank"), Some(browser_context_id))
        .await
    {
        let _ = tab.close().await;
        bail!("{label} BrowserContext still accepted a page target after disposal");
    }
    Ok(())
}

async fn check_disposable_final_tab(runtime: &Arc<ChromiumRuntime>) -> Result<()> {
    let baseline: BTreeSet<_> = runtime.identity_contexts().await?.into_iter().collect();
    let baseline_targets: BTreeSet<_> = runtime
        .existing_tabs()
        .await?
        .into_iter()
        .filter_map(|tab| tab.target_id().map(|target| target.inner().clone()))
        .collect();
    let (engine, handle) = BrowserEngine::new(Arc::clone(runtime));
    let engine_task = tokio::spawn(engine.run());

    if let Ok(unexpected) = handle.create_disposable_tab("http://[::1".into()).await {
        let (close_reply, close_response) = oneshot::channel();
        handle
            .cmd_tx
            .send(EngineCommand::CloseTab {
                tab_id: unexpected.tab_id,
                reply: close_reply,
            })
            .await
            .context("failed to clean up unexpectedly successful invalid navigation")?;
        let _ = timeout(Duration::from_secs(30), close_response).await;
        bail!("disposable tab unexpectedly accepted invalid navigation URL");
    }
    let after_failed_navigation: BTreeSet<_> =
        runtime.identity_contexts().await?.into_iter().collect();
    if after_failed_navigation != baseline {
        bail!("failed disposable navigation leaked a BrowserContext: {after_failed_navigation:?}");
    }
    let targets_after_failure: BTreeSet<_> = runtime
        .existing_tabs()
        .await?
        .into_iter()
        .filter_map(|tab| tab.target_id().map(|target| target.inner().clone()))
        .collect();
    if targets_after_failure != baseline_targets {
        bail!("failed disposable navigation leaked a page target: {targets_after_failure:?}");
    }

    let first = handle
        .create_disposable_tab("about:blank".into())
        .await
        .map_err(|error| anyhow!("first disposable tab creation failed: {error}"))?;
    let second = handle
        .create_disposable_tab("about:blank".into())
        .await
        .map_err(|error| anyhow!("second disposable tab creation failed: {error}"))?;
    if first.identity_id == second.identity_id || first.tab_id == second.tab_id {
        bail!("disposable-tab API returned colliding tab or identity IDs");
    }
    let during: BTreeSet<_> = runtime.identity_contexts().await?.into_iter().collect();
    if during.len() != baseline.len() + 2 || !baseline.is_subset(&during) {
        bail!("two disposable tabs did not create distinct BrowserContexts: {during:?}");
    }
    let created_contexts: BTreeSet<_> = during.difference(&baseline).cloned().collect();

    let (close_reply, close_response) = oneshot::channel();
    handle
        .cmd_tx
        .send(EngineCommand::CloseTab {
            tab_id: first.tab_id,
            reply: close_reply,
        })
        .await
        .context("Thor coordinator stopped before disposable tab close")?;
    timeout(Duration::from_secs(30), close_response)
        .await
        .context("disposable final-tab close timed out")??
        .map_err(|error| anyhow!("disposable final-tab close failed: {error}"))?;
    let after_first: BTreeSet<_> = runtime.identity_contexts().await?.into_iter().collect();
    if after_first.len() != baseline.len() + 1 || !baseline.is_subset(&after_first) {
        bail!("closing first disposable final tab removed wrong BrowserContexts: {after_first:?}");
    }
    let disposed_after_first: Vec<_> = created_contexts.difference(&after_first).cloned().collect();
    if disposed_after_first.len() != 1 {
        bail!("first final-tab close did not identify exactly one disposed context");
    }
    require_context_disposed(runtime, &disposed_after_first[0], "first disposable").await?;

    let (close_reply, close_response) = oneshot::channel();
    handle
        .cmd_tx
        .send(EngineCommand::CloseTab {
            tab_id: second.tab_id,
            reply: close_reply,
        })
        .await
        .context("Thor coordinator stopped before second disposable tab close")?;
    timeout(Duration::from_secs(30), close_response)
        .await
        .context("second disposable final-tab close timed out")??
        .map_err(|error| anyhow!("second disposable final-tab close failed: {error}"))?;
    let after: BTreeSet<_> = runtime.identity_contexts().await?.into_iter().collect();
    if after != baseline {
        bail!("closing disposable final tab did not dispose its BrowserContext: {after:?}");
    }
    for context_id in &created_contexts {
        require_context_disposed(runtime, context_id, "final disposable").await?;
    }

    handle
        .create_disposable_tab("about:blank".into())
        .await
        .map_err(|error| anyhow!("detach-cleanup disposable tab creation failed: {error}"))?;
    let before_detach: BTreeSet<_> = runtime.identity_contexts().await?.into_iter().collect();
    let detach_contexts: Vec<_> = before_detach.difference(&baseline).cloned().collect();
    if detach_contexts.len() != 1 {
        bail!("coordinator detach setup did not create exactly one ephemeral BrowserContext");
    }
    drop(handle);
    timeout(Duration::from_secs(30), engine_task)
        .await
        .context("Thor coordinator did not stop after its handle was dropped")??;
    let after_detach: BTreeSet<_> = runtime.identity_contexts().await?.into_iter().collect();
    if after_detach != baseline {
        bail!("coordinator detach leaked an owned ephemeral BrowserContext: {after_detach:?}");
    }
    require_context_disposed(runtime, &detach_contexts[0], "coordinator detach").await?;
    Ok(())
}

#[test]
fn browserless_webrtc_default_is_fail_private() {
    assert_eq!(
        ThorBrowserDefaults::default_webrtc_ip_handling_policy(),
        WebRtcIpPolicy::DisableNonProxiedUdp
    );
    assert_eq!(
        ThorBrowserDefaults::new().webrtc_ip_policy,
        WebRtcIpPolicy::DisableNonProxiedUdp
    );
}

#[test]
fn unverified_identity_boundaries_are_explicit() {
    assert_eq!(
        UNVERIFIED_IDENTITY_BOUNDARIES
            .iter()
            .map(|(boundary, _)| *boundary)
            .collect::<Vec<_>>(),
        vec![
            "HSTS state",
            "HTTP authentication cache",
            "socket and network connection pools",
            "startup orphan BrowserContext cleanup",
        ]
    );
    assert!(UNVERIFIED_IDENTITY_BOUNDARIES
        .iter()
        .all(|(_, reason)| !reason.is_empty()));
}

#[test]
fn unverified_release_boundaries_are_explicit() {
    assert_eq!(UNVERIFIED_RELEASE_BOUNDARIES.len(), 8);
    assert!(UNVERIFIED_RELEASE_BOUNDARIES
        .iter()
        .all(|(boundary, reason)| !boundary.is_empty() && !reason.is_empty()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires a live provenance-verified Thor-owned Chromium source/package binary"]
async fn thor_browser_privacy_acceptance() -> Result<()> {
    let build = OwnedBuild::from_environment()?;
    let server = LoopbackServer::start().await?;
    let (live_browser, debug_url) = LiveBrowser::launch(&build).await?;
    let runtime = ChromiumRuntime::connect(debug_url)
        .await
        .context("Thor engine could not attach to the launched owned browser")?;

    let startup_tab = check_startup_and_process_policy(&runtime, &build).await?;
    check_gpc_dnt_and_webrtc(&startup_tab, &server).await?;
    check_identity_isolation(&runtime, &server).await?;
    check_disposable_final_tab(&runtime).await?;

    Arc::clone(&runtime)
        .shutdown()
        .await
        .context("Thor engine failed to shut down acceptance browser")?;
    live_browser.stop().await?;
    Ok(())
}
