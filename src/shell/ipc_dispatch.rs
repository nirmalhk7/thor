//! Allowlisted privileged IPC handlers (SEC-090 / FS-IPC-01).
//!
//! Content pages must not reach these — call only from trusted shell surfaces
//! (native toolbar, key monitor, or a future chrome WebView).

use serde_json::Value;

use crate::app_state::AppState;
use crate::engine::profiles::ProfileMode;
use crate::net::consent::{is_private_or_local_url, LocustStartGate};
use crate::shell::engine_status;

#[derive(Debug, Clone)]
pub enum DispatchResult {
    Ok,
    Message(String),
    EngineStatus(Value),
    LocustMetrics(crate::net::locust::LocustScoredMetrics),
    /// A JSON envelope produced by the Engineer Feature Suite (feature list,
    /// invoke result, or category lens) to be pushed back to the chrome UI.
    FeatureJson(Value),
    Err(String),
}

/// Handle a privileged command. Unknown cmds → Ok (no-op) after log.
pub fn dispatch_privileged(cmd: &str, payload: &Value) -> DispatchResult {
    let state = AppState::global();
    match cmd {
        "engine_status" | "get_engine_status" => {
            DispatchResult::EngineStatus(engine_status::engine_status())
        }
        "toggle_profile" => {
            let mode = payload["mode"].as_str();
            let next = match mode {
                Some("StealthTarget") | Some("stealth") => {
                    state.set_profile_mode(ProfileMode::StealthTarget);
                    ProfileMode::StealthTarget
                }
                Some("DevHost") | Some("dev") => {
                    state.set_profile_mode(ProfileMode::DevHost);
                    ProfileMode::DevHost
                }
                Some("HardenedTarget") | Some("hardened") => {
                    state.set_profile_mode(ProfileMode::HardenedTarget);
                    ProfileMode::HardenedTarget
                }
                _ => state.toggle_profile(),
            };
            DispatchResult::Message(format!("profile:{next:?}"))
        }
        "set_interception_enabled" => {
            let enabled = payload["enabled"].as_bool().unwrap_or(false);
            state.tamper().set_interception_enabled(enabled);
            if let Some(bp) = payload["breakpoint"].as_bool() {
                state.tamper().set_breakpoint_mode(bp);
            }
            if enabled {
                DispatchResult::Message("interception:on".into())
            } else {
                DispatchResult::Message("interception:off".into())
            }
        }
        "forward_intercepted" => {
            let id = payload["request_id"].as_str().unwrap_or("");
            if id.is_empty() {
                return DispatchResult::Err("forward_intercepted requires request_id".into());
            }
            match state.tamper().forward_request(id, None) {
                Some(req) => DispatchResult::FeatureJson(serde_json::json!({
                    "forwarded": true,
                    "id": req.id,
                    "url": req.url,
                })),
                None => DispatchResult::Err(format!("no pending request {id}")),
            }
        }
        "drop_intercepted" => {
            let id = payload["request_id"].as_str().unwrap_or("");
            if id.is_empty() {
                return DispatchResult::Err("drop_intercepted requires request_id".into());
            }
            if state.tamper().drop_request(id) {
                DispatchResult::Message(format!("dropped:{id}"))
            } else {
                DispatchResult::Err(format!("no pending request {id}"))
            }
        }
        "list_intercepted" => {
            let pending = state.tamper().get_pending_requests();
            let history_len = state.tamper().get_history().len();
            DispatchResult::FeatureJson(serde_json::json!({
                "pending": pending,
                "history_len": history_len,
                "interception_enabled": state.tamper().is_interception_enabled(),
                "breakpoint_mode": state.tamper().is_breakpoint_mode(),
            }))
        }
        "locust_start" => {
            let url = payload["url"]
                .as_str()
                .or_else(|| payload["target_url"].as_str())
                .unwrap_or("")
                .to_string();
            let users = payload["users"].as_u64().unwrap_or(5) as u32;
            let ramp = payload["ramp"]
                .as_u64()
                .or_else(|| payload["ramp_up_secs"].as_u64())
                .unwrap_or(1) as u32;
            let runtime = payload["runtime"]
                .as_u64()
                .or_else(|| payload["runtime_secs"].as_u64())
                .unwrap_or(30);
            let public_ack = payload["public_ack"].as_bool().unwrap_or(false);
            match state.locust_start_gated(LocustStartGate {
                target_url: url,
                users,
                ramp_up_secs: ramp,
                runtime_secs: runtime,
                public_ack,
            }) {
                Ok(()) => DispatchResult::Message("locust:started".into()),
                Err(e) => DispatchResult::Err(e),
            }
        }
        "locust_stop" => match state.locust_stop() {
            Some(m) => DispatchResult::LocustMetrics(m),
            None => DispatchResult::Message("locust:idle".into()),
        },
        "locust_toggle_active" => {
            // URL must be supplied by Rust (active tab), not the page.
            let url = payload["url"].as_str().unwrap_or("").to_string();
            if state.locust_is_running() {
                let _ = state.locust_stop();
                return DispatchResult::Message("locust:stopped".into());
            }
            if url.is_empty() {
                return DispatchResult::Err("No active URL for Locust".into());
            }
            // Private: auto-start with lean defaults. Public: refuse until UI ack exists.
            let public_ack = false;
            if !is_private_or_local_url(&url) {
                return DispatchResult::Err(format!(
                    "Public target blocked in M1 lean mode (authorized local/private only): {url}"
                ));
            }
            match state.locust_start_gated(LocustStartGate {
                target_url: url.clone(),
                users: 5,
                ramp_up_secs: 1,
                runtime_secs: 30,
                public_ack,
            }) {
                Ok(()) => DispatchResult::Message(format!("locust:started:{url}")),
                Err(e) => DispatchResult::Err(e),
            }
        }
        "open_ide" => {
            if !state.profiles().get_profile().enable_ide_bridge {
                return DispatchResult::Err("IDE bridge disabled in Stealth profile".into());
            }
            let ide_name = payload["ide"].as_str().unwrap_or("vscode");
            let path = payload["path"].as_str().unwrap_or(".");
            let line = payload["line"].as_u64().map(|l| l as usize);
            let col = payload["col"].as_u64().map(|c| c as usize);
            let ide = match ide_name {
                "cursor" => crate::dx::ide_bridge::Ide::Cursor,
                "webstorm" => crate::dx::ide_bridge::Ide::WebStorm,
                _ => crate::dx::ide_bridge::Ide::VSCode,
            };
            match crate::dx::ide_bridge::IdeBridge::open_file(ide, path, line, col) {
                Ok(()) => DispatchResult::Ok,
                Err(e) => DispatchResult::Err(e),
            }
        }
        "decode_payload" => {
            let input = payload["input"].as_str().unwrap_or("");
            let result = crate::dx::decoder::DecoderStudio::detect_type(input);
            DispatchResult::Message(format!("decode:{result:?}"))
        }
        "decode_jwt_crack" => DispatchResult::Err(
            "JWT crack unavailable (honest). Display-only decoder is supported.".into(),
        ),
        "add_tamper_rule" => {
            let pattern = payload["target_url_pattern"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let key = payload["replace_header_key"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let val = payload["replace_header_value"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let _drop = payload["drop"].as_bool().unwrap_or(false);
            if pattern.is_empty() {
                return DispatchResult::Err("Pattern required".into());
            }
            state.tamper().add_rule(crate::net::tamper::TamperRule {
                id: format!("rule_{}", std::time::Instant::now().elapsed().as_nanos()),
                enabled: true,
                target_url_pattern: pattern,
                match_header_key: None,
                replace_header_key: key,
                replace_header_value: val,
                mock_status: None,
                mock_body: None,
                delay_ms: None,
            });
            DispatchResult::Message("tamper:rule_added".into())
        }
        "feature_list" => {
            let report = state.features().status_report();
            match serde_json::to_value(&report) {
                Ok(v) => DispatchResult::FeatureJson(v),
                Err(e) => DispatchResult::Err(format!("feature_list serialize failed: {e}")),
            }
        }
        "feature_invoke" => {
            let slug = payload["slug"].as_str().unwrap_or("").to_string();
            if slug.is_empty() {
                return DispatchResult::Err("feature_invoke requires 'slug'".into());
            }
            match crate::features::orchestrator::FeatureOrchestrator::invoke(&slug, payload) {
                Ok(v) => DispatchResult::FeatureJson(v),
                Err(e) => DispatchResult::FeatureJson(serde_json::json!({
                    "slug": slug,
                    "error": e,
                })),
            }
        }
        "lens_payload" => {
            let category = payload["category"].as_str().unwrap_or("");
            match crate::features::orchestrator::FeatureOrchestrator::lens_payload(category) {
                Ok(v) => DispatchResult::FeatureJson(v),
                Err(e) => DispatchResult::Err(e),
            }
        }
        "get_logs" => {
            let min_lvl = payload["level"].as_str().and_then(|s| s.parse::<crate::logging::LogLevel>().ok());
            let target = payload["target"].as_str();
            let limit = payload["limit"].as_u64().unwrap_or(100) as usize;
            let records = state.logs().query(min_lvl, target, limit);
            match serde_json::to_value(&records) {
                Ok(v) => DispatchResult::FeatureJson(serde_json::json!({ "logs": v })),
                Err(e) => DispatchResult::Err(format!("get_logs serialize failed: {e}")),
            }
        }
        "clear_logs" => {
            state.logs().clear();
            DispatchResult::Message("logs:cleared".into())
        }
        "copy_llm_clean_context" => {
            let html = payload["html"].as_str().unwrap_or("");
            let url = payload["url"].as_str().unwrap_or("");
            let title = payload["title"].as_str().unwrap_or("");
            if html.is_empty() {
                return DispatchResult::Err(
                    "copy_llm_clean_context requires html (shell should evaluate documentElement.outerHTML)"
                        .into(),
                );
            }
            let cleaned = crate::dx::llm_clean_context::LlmCleanContext::clean_dom(html);
            let mut md = String::new();
            if !title.is_empty() || !url.is_empty() {
                md.push_str(&format!("# {}\n\nSource: {}\n\n", title, url));
            }
            md.push_str(&cleaned.markdown);
            DispatchResult::FeatureJson(serde_json::json!({
                "markdown": md,
                "raw_html_len": cleaned.raw_html_len,
                "clean_markdown_len": cleaned.clean_markdown_len,
                "token_savings_percent": cleaned.token_savings_percent,
                "url": url,
                "title": title,
            }))
        }
        "add_bookmark" => {
            let title = payload["title"].as_str().unwrap_or("").to_string();
            let url = payload["url"].as_str().unwrap_or("").to_string();
            if url.is_empty() {
                return DispatchResult::Err("URL required".into());
            }
            if let Ok(mut store) = crate::dx::bookmarks::BookmarkStore::open() {
                let _ = store.add(title, url);
                DispatchResult::Message("bookmark:added".into())
            } else {
                DispatchResult::Err("Failed to save bookmark".into())
            }
        }
        "run_idor_matrix" => {
            let req_id = payload["request_id"].as_str();
            let req = match req_id {
                Some(id) => state.tamper().get_request_by_id(id),
                None => state.tamper().get_latest_request(),
            };
            if let Some(r) = req {
                let idor_engine = state.idor().clone();
                let rt = tokio::runtime::Handle::current();
                let result = rt.block_on(idor_engine.run_matrix(&r, vec![]));
                let json = serde_json::to_string(&result).unwrap_or_default();
                DispatchResult::Message(format!("idor:matrix_result:{}", json))
            } else {
                DispatchResult::Err("No request available to run IDOR matrix scan".into())
            }
        }
        "remove_bookmark" => {
            let url = payload["url"].as_str().unwrap_or("").to_string();
            if url.is_empty() {
                return DispatchResult::Err("URL required".into());
            }
            if let Ok(mut store) = crate::dx::bookmarks::BookmarkStore::open() {
                let _ = store.remove_by_url(&url);
                DispatchResult::Message("bookmark:removed".into())
            } else {
                DispatchResult::Err("Failed to remove bookmark".into())
            }
        }
        "bitwarden_status" => {
            DispatchResult::FeatureJson(state.vault().status_json())
        }
        "bitwarden_login" => {
            let email = payload["email"].as_str().unwrap_or("");
            let password = payload["password"].as_str().unwrap_or("");
            match state.vault().login(email, password) {
                Ok(()) => DispatchResult::FeatureJson(state.vault().status_json()),
                Err(e) => DispatchResult::Err(e.to_string()),
            }
        }
        "bitwarden_unlock" => {
            let password = payload["password"].as_str().unwrap_or("");
            match state.vault().unlock(password) {
                Ok(()) => DispatchResult::FeatureJson(state.vault().status_json()),
                Err(e) => DispatchResult::Err(e.to_string()),
            }
        }
        "bitwarden_lock" => match state.vault().lock() {
            Ok(()) => DispatchResult::FeatureJson(state.vault().status_json()),
            Err(e) => DispatchResult::Err(e.to_string()),
        }
        "bitwarden_logout" => match state.vault().logout() {
            Ok(()) => DispatchResult::FeatureJson(state.vault().status_json()),
            Err(e) => DispatchResult::Err(e.to_string()),
        }
        "bitwarden_search" => {
            let query = payload["query"].as_str().unwrap_or("");
            match state.vault().search(query) {
                Ok(items) => DispatchResult::FeatureJson(serde_json::json!({
                    "items": crate::dx::bitwarden::BitwardenVault::summaries_json(&items),
                })),
                Err(e) => DispatchResult::Err(e.to_string()),
            }
        }
        "bitwarden_matches" => {
            let url = payload["url"].as_str().unwrap_or("");
            match state.vault().list_for_url(url) {
                Ok(items) => DispatchResult::FeatureJson(serde_json::json!({
                    "url": url,
                    "items": crate::dx::bitwarden::BitwardenVault::summaries_json(&items),
                })),
                Err(e) => DispatchResult::Err(e.to_string()),
            }
        }
        // Autofill requires CDP on the active tab — shell must call
        // `bitwarden_autofill_active` after this returns Ok with item id ack.
        // Never return passwords in this envelope.
        "bitwarden_autofill" => {
            let id = payload["id"].as_str().unwrap_or("").trim();
            if id.is_empty() {
                return DispatchResult::Err("item id required".into());
            }
            if !state.vault().has_session() {
                return DispatchResult::Err(crate::dx::bitwarden::VaultError::NoSession.to_string());
            }
            // Signal shell to perform CDP fill (id only — no secrets).
            DispatchResult::Message(format!("bitwarden:autofill:{id}"))
        }
        other => {
            println!("⚡ [Thor IPC] unknown/ignored privileged cmd: {other}");
            DispatchResult::Ok
        }
    }
}
