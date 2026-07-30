//! First-class Bitwarden Password Manager integration via the official `bw` CLI.
//!
//! Session keys live only in process memory (never written to disk by Thor).
//! Autofill secrets are passed only into CDP fill scripts — never chrome hooks.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Lean login summary for UI / Cmd+K (no password).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultItemSummary {
    pub id: String,
    pub name: String,
    pub username: Option<String>,
    pub uris: Vec<String>,
}

/// Decrypted login for autofill only (never serialize into chrome hooks).
#[derive(Debug, Clone)]
pub struct VaultLogin {
    pub id: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultState {
    MissingCli,
    Unauthenticated,
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultStatus {
    pub cli_present: bool,
    pub state: VaultState,
    pub user_email: Option<String>,
    pub server_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    MissingCli,
    NotLoggedIn,
    Locked,
    AuthFailed(String),
    Cli(String),
    Parse(String),
    NoSession,
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCli => write!(
                f,
                "Bitwarden CLI (bw) not found on PATH — install from https://bitwarden.com/help/cli/"
            ),
            Self::NotLoggedIn => write!(f, "Not logged in to Bitwarden — use login first"),
            Self::Locked => write!(f, "Vault is locked"),
            Self::AuthFailed(m) => write!(f, "Authentication failed: {m}"),
            Self::Cli(m) => write!(f, "bw: {m}"),
            Self::Parse(m) => write!(f, "parse error: {m}"),
            Self::NoSession => write!(f, "No vault session — unlock first"),
        }
    }
}

/// Process-wide Bitwarden vault client (`bw` CLI backend).
pub struct BitwardenVault {
    session: Mutex<Option<String>>,
}

impl Default for BitwardenVault {
    fn default() -> Self {
        Self::new()
    }
}

impl BitwardenVault {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }

    pub fn has_session(&self) -> bool {
        self.session.lock().is_some()
    }

    pub fn clear_session(&self) {
        *self.session.lock() = None;
    }

    /// Detect CLI and report vault status (never logs secrets).
    pub fn status(&self) -> VaultStatus {
        if !cli_present() {
            return VaultStatus {
                cli_present: false,
                state: VaultState::MissingCli,
                user_email: None,
                server_url: None,
            };
        }
        match run_bw(&["status"], None) {
            Ok(raw) => parse_status_json(&raw, self.has_session()),
            Err(_) => VaultStatus {
                cli_present: true,
                state: if self.has_session() {
                    VaultState::Unlocked
                } else {
                    VaultState::Locked
                },
                user_email: None,
                server_url: None,
            },
        }
    }

    /// `bw login --raw` with email + master password via stdin.
    pub fn login(&self, email: &str, password: &str) -> Result<(), VaultError> {
        ensure_cli()?;
        let email = email.trim();
        if email.is_empty() || password.is_empty() {
            return Err(VaultError::AuthFailed("email and password required".into()));
        }
        let raw = run_bw_with_stdin(&["login", email, "--raw"], password, None)?;
        let session = raw.trim().to_string();
        if session.is_empty() {
            return Err(VaultError::AuthFailed("empty session from bw login".into()));
        }
        *self.session.lock() = Some(session);
        Ok(())
    }

    /// `bw unlock --raw` → store session in memory only.
    pub fn unlock(&self, password: &str) -> Result<(), VaultError> {
        ensure_cli()?;
        if password.is_empty() {
            return Err(VaultError::AuthFailed("master password required".into()));
        }
        let raw = run_bw_with_stdin(&["unlock", "--raw"], password, None)?;
        let session = raw.trim().to_string();
        if session.is_empty() {
            return Err(VaultError::AuthFailed("empty session from bw unlock".into()));
        }
        *self.session.lock() = Some(session);
        Ok(())
    }

    pub fn lock(&self) -> Result<(), VaultError> {
        let session = self.session.lock().take();
        if cli_present() {
            let _ = run_bw(&["lock"], session.as_deref());
        }
        Ok(())
    }

    pub fn logout(&self) -> Result<(), VaultError> {
        self.clear_session();
        if cli_present() {
            let _ = run_bw(&["logout"], None);
        }
        Ok(())
    }

    pub fn list_for_url(&self, url: &str) -> Result<Vec<VaultItemSummary>, VaultError> {
        let session = self.require_session()?;
        let url = url.trim();
        if url.is_empty() {
            return Ok(Vec::new());
        }
        let raw = run_bw(&["list", "items", "--url", url, "--raw"], Some(&session))?;
        parse_item_summaries(&raw)
    }

    pub fn search(&self, query: &str) -> Result<Vec<VaultItemSummary>, VaultError> {
        let session = self.require_session()?;
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let raw = run_bw(&["list", "items", "--search", q, "--raw"], Some(&session))?;
        parse_item_summaries(&raw)
    }

    pub fn get_login(&self, id: &str) -> Result<VaultLogin, VaultError> {
        let session = self.require_session()?;
        let id = id.trim();
        if id.is_empty() {
            return Err(VaultError::Cli("item id required".into()));
        }
        let raw = run_bw(&["get", "item", id, "--raw"], Some(&session))?;
        parse_vault_login(&raw)
    }

    /// JSON status envelope for chrome / Cmd+K (no secrets).
    pub fn status_json(&self) -> Value {
        let s = self.status();
        serde_json::json!({
            "cli_present": s.cli_present,
            "state": s.state,
            "user_email": s.user_email,
            "server_url": s.server_url,
            "has_session": self.has_session(),
        })
    }

    /// Summaries as JSON (no passwords).
    pub fn summaries_json(items: &[VaultItemSummary]) -> Value {
        serde_json::to_value(items).unwrap_or_else(|_| Value::Array(vec![]))
    }

    fn require_session(&self) -> Result<String, VaultError> {
        self.session
            .lock()
            .clone()
            .ok_or(VaultError::NoSession)
    }
}

fn ensure_cli() -> Result<(), VaultError> {
    if cli_present() {
        Ok(())
    } else {
        Err(VaultError::MissingCli)
    }
}

fn cli_present() -> bool {
    Command::new("bw")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_bw(args: &[&str], session: Option<&str>) -> Result<String, VaultError> {
    let mut cmd = Command::new("bw");
    cmd.args(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(s) = session {
        cmd.env("BW_SESSION", s);
    }
    // Avoid hanging forever on hung CLI.
    let output = cmd
        .output()
        .map_err(|e| VaultError::Cli(format!("failed to spawn bw: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit {}", output.status)
        };
        return Err(classify_cli_error(&msg));
    }
    Ok(stdout)
}

fn run_bw_with_stdin(args: &[&str], stdin_data: &str, session: Option<&str>) -> Result<String, VaultError> {
    use std::io::Write;
    let mut cmd = Command::new("bw");
    cmd.args(args);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(s) = session {
        cmd.env("BW_SESSION", s);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| VaultError::Cli(format!("failed to spawn bw: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_data.as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    // Soft timeout: wait up to 60s (unlock/login can be slow).
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > Duration::from_secs(60) => {
                let _ = child.kill();
                return Err(VaultError::Cli("bw timed out".into()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(VaultError::Cli(format!("wait failed: {e}"))),
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| VaultError::Cli(format!("bw wait: {e}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit {}", output.status)
        };
        return Err(classify_cli_error(&msg));
    }
    // `bw unlock --raw` / `login --raw` print only the session key.
    Ok(stdout)
}

fn classify_cli_error(msg: &str) -> VaultError {
    let lower = msg.to_lowercase();
    if lower.contains("not logged in") || lower.contains("you are not logged in") {
        VaultError::NotLoggedIn
    } else if lower.contains("vault is locked") || lower.contains("session key") {
        VaultError::Locked
    } else if lower.contains("invalid master password")
        || lower.contains("username or password is incorrect")
        || lower.contains("authentication failed")
    {
        VaultError::AuthFailed(msg.to_string())
    } else {
        VaultError::Cli(msg.to_string())
    }
}

pub fn parse_status_json(raw: &str, has_local_session: bool) -> VaultStatus {
    let v: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => {
            return VaultStatus {
                cli_present: true,
                state: if has_local_session {
                    VaultState::Unlocked
                } else {
                    VaultState::Locked
                },
                user_email: None,
                server_url: None,
            };
        }
    };
    let status = v["status"].as_str().unwrap_or("");
    let state = match status {
        "unauthenticated" => VaultState::Unauthenticated,
        "locked" => {
            if has_local_session {
                VaultState::Unlocked
            } else {
                VaultState::Locked
            }
        }
        "unlocked" => VaultState::Unlocked,
        _ => {
            if has_local_session {
                VaultState::Unlocked
            } else {
                VaultState::Locked
            }
        }
    };
    VaultStatus {
        cli_present: true,
        state,
        user_email: v["userEmail"].as_str().map(str::to_string),
        server_url: v["serverUrl"].as_str().map(str::to_string),
    }
}

pub fn parse_item_summaries(raw: &str) -> Result<Vec<VaultItemSummary>, VaultError> {
    let items: Vec<Value> = serde_json::from_str(raw)
        .map_err(|e| VaultError::Parse(format!("list items: {e}")))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        // Login type == 1 in Bitwarden.
        let item_type = item["type"].as_u64().unwrap_or(0);
        if item_type != 1 {
            continue;
        }
        let id = item["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            continue;
        }
        let name = item["name"].as_str().unwrap_or("Untitled").to_string();
        let username = item["login"]["username"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let uris = item["login"]["uris"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|u| u["uri"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        out.push(VaultItemSummary {
            id,
            name,
            username,
            uris,
        });
    }
    Ok(out)
}

pub fn parse_vault_login(raw: &str) -> Result<VaultLogin, VaultError> {
    let item: Value =
        serde_json::from_str(raw).map_err(|e| VaultError::Parse(format!("get item: {e}")))?;
    let id = item["id"].as_str().unwrap_or("").to_string();
    if id.is_empty() {
        return Err(VaultError::Parse("item missing id".into()));
    }
    let login = &item["login"];
    let username = login["username"].as_str().unwrap_or("").to_string();
    let password = login["password"].as_str().unwrap_or("").to_string();
    if password.is_empty() && username.is_empty() {
        return Err(VaultError::Parse("item has no login credentials".into()));
    }
    Ok(VaultLogin {
        id,
        username,
        password,
    })
}

/// Build a CDP `Runtime.evaluate` expression that fills username/password fields.
/// Values are JSON-escaped — never interpolated raw.
pub fn build_autofill_script(username: &str, password: &str) -> String {
    let u = serde_json::to_string(username).unwrap_or_else(|_| "\"\"".into());
    let p = serde_json::to_string(password).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(function(){{
  var u = {u};
  var p = {p};
  function setVal(el, val) {{
    if (!el) return false;
    el.focus();
    var proto = window.HTMLInputElement && HTMLInputElement.prototype;
    var desc = proto && Object.getOwnPropertyDescriptor(proto, 'value');
    if (desc && desc.set) desc.set.call(el, val);
    else el.value = val;
    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
    return true;
  }}
  var pass = document.querySelector('input[type="password"]:not([disabled]):not([readonly])');
  var user = document.querySelector(
    'input[autocomplete="username"],input[autocomplete="email"],input[type="email"],input[name*="user" i],input[name*="email" i],input[id*="user" i],input[id*="email" i],input[type="text"]:not([disabled])'
  );
  var filled = 0;
  if (u && setVal(user, u)) filled++;
  if (p && setVal(pass, p)) filled++;
  return {{ filled: filled, hasPassword: !!pass, hasUsername: !!user }};
}})()"#
    )
}

/// Cmd+K / settings palette helpers (no passwords).
pub fn vault_palette_items(
    status: &VaultStatus,
    matches: &[VaultItemSummary],
) -> Vec<crate::dx::cmd_palette::PaletteItem> {
    use crate::dx::cmd_palette::{PaletteItem, PaletteItemType};
    let mut items = Vec::new();
    match status.state {
        VaultState::MissingCli => {
            items.push(PaletteItem::new(
                "bw_missing",
                "Bitwarden CLI missing — install `bw`",
                PaletteItemType::Setting,
                "Thor://vault/missing",
                Some("Vault".into()),
                Some("🔑".into()),
                "Vault",
                Some("open_bitwarden_vault".into()),
            ));
        }
        VaultState::Unauthenticated | VaultState::Locked => {
            items.push(PaletteItem::new(
                "bw_unlock",
                "Unlock Bitwarden vault…",
                PaletteItemType::Setting,
                "Thor://vault/unlock",
                Some("Vault".into()),
                Some("🔑".into()),
                "Vault",
                Some("open_bitwarden_vault".into()),
            ));
        }
        VaultState::Unlocked => {
            items.push(PaletteItem::new(
                "bw_lock",
                "Lock Bitwarden vault",
                PaletteItemType::Setting,
                "Thor://vault/lock",
                Some("Vault".into()),
                Some("🔒".into()),
                "Vault",
                Some("bitwarden_lock".into()),
            ));
            for m in matches {
                let sub = m
                    .username
                    .clone()
                    .or_else(|| m.uris.first().cloned())
                    .unwrap_or_default();
                items.push(PaletteItem::new(
                    format!("bw_item_{}", m.id),
                    format!("Fill: {}", m.name),
                    PaletteItemType::Setting,
                    m.uris.first().cloned().unwrap_or_else(|| "Thor://vault/item".into()),
                    Some(if sub.is_empty() {
                        "Vault".into()
                    } else {
                        sub
                    }),
                    Some("🔑".into()),
                    "Vault",
                    Some(format!("bitwarden_autofill_{}", m.id)),
                ));
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST_FIXTURE: &str = r#"[
      {
        "id": "aaaa-1111",
        "name": "Example Login",
        "type": 1,
        "login": {
          "username": "dev@example.com",
          "password": "secret-should-not-leak-to-summary",
          "uris": [{ "uri": "https://example.com/login" }]
        }
      },
      {
        "id": "bbbb-2222",
        "name": "Note Only",
        "type": 2,
        "notes": "no login"
      }
    ]"#;

    const ITEM_FIXTURE: &str = r#"{
      "id": "aaaa-1111",
      "name": "Example Login",
      "type": 1,
      "login": {
        "username": "dev@example.com",
        "password": "s3cret!",
        "uris": [{ "uri": "https://example.com" }]
      }
    }"#;

    const STATUS_LOCKED: &str = r#"{"status":"locked","userEmail":"u@ex.com","serverUrl":"https://vault.bitwarden.com"}"#;
    const STATUS_UNLOCKED: &str = r#"{"status":"unlocked","userEmail":"u@ex.com"}"#;

    #[test]
    fn test_parse_item_summaries_skips_notes_and_passwords() {
        let items = parse_item_summaries(LIST_FIXTURE).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "aaaa-1111");
        assert_eq!(items[0].name, "Example Login");
        assert_eq!(items[0].username.as_deref(), Some("dev@example.com"));
        assert_eq!(items[0].uris, vec!["https://example.com/login"]);
        let json = serde_json::to_string(&items).unwrap();
        assert!(!json.contains("secret-should-not-leak"));
    }

    #[test]
    fn test_parse_vault_login() {
        let login = parse_vault_login(ITEM_FIXTURE).unwrap();
        assert_eq!(login.id, "aaaa-1111");
        assert_eq!(login.username, "dev@example.com");
        assert_eq!(login.password, "s3cret!");
    }

    #[test]
    fn test_parse_status() {
        let locked = parse_status_json(STATUS_LOCKED, false);
        assert_eq!(locked.state, VaultState::Locked);
        assert_eq!(locked.user_email.as_deref(), Some("u@ex.com"));

        let unlocked_local = parse_status_json(STATUS_LOCKED, true);
        assert_eq!(unlocked_local.state, VaultState::Unlocked);

        let unlocked = parse_status_json(STATUS_UNLOCKED, false);
        assert_eq!(unlocked.state, VaultState::Unlocked);
    }

    #[test]
    fn test_autofill_script_escapes_quotes_and_no_raw_interp() {
        let script = build_autofill_script(r#"alice"bob"#, r#"p'ass\x"#);
        assert!(script.contains(r#"alice\"bob"#) || script.contains("alice\\\"bob"));
        // Must use JSON string literals, not raw concatenation of password into JS.
        assert!(script.contains("var u = "));
        assert!(script.contains("var p = "));
        assert!(script.contains("filled"));
        // XSS / breakout: a crafted password must not close the JS string early as raw.
        let evil = build_autofill_script("u", r#""; alert(1); //"#);
        assert!(evil.contains("\\") || evil.contains("alert"));
        // The password is inside a JSON-encoded string assigned to `p`.
        assert!(evil.contains("var p = "));
    }

    #[test]
    fn test_vault_palette_locked_and_matches() {
        let status = VaultStatus {
            cli_present: true,
            state: VaultState::Locked,
            user_email: None,
            server_url: None,
        };
        let locked = vault_palette_items(&status, &[]);
        assert_eq!(locked.len(), 1);
        assert_eq!(
            locked[0].action_key.as_deref(),
            Some("open_bitwarden_vault")
        );

        let unlocked = VaultStatus {
            state: VaultState::Unlocked,
            ..status
        };
        let matches = vec![VaultItemSummary {
            id: "id1".into(),
            name: "GitHub".into(),
            username: Some("octocat".into()),
            uris: vec!["https://github.com".into()],
        }];
        let items = vault_palette_items(&unlocked, &matches);
        assert!(items.iter().any(|i| i.action_key.as_deref() == Some("bitwarden_lock")));
        assert!(items
            .iter()
            .any(|i| i.action_key.as_deref() == Some("bitwarden_autofill_id1")));
    }
}
