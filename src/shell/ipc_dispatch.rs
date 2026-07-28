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
                _ => state.toggle_profile(),
            };
            DispatchResult::Message(format!("profile:{next:?}"))
        }
        "set_interception_enabled" => {
            let enabled = payload["enabled"].as_bool().unwrap_or(false);
            if enabled {
                // M1: never enable page MitM on WebKit.
                state.tamper().set_interception_enabled(false);
                DispatchResult::Err(
                    "Page intercept requires Chromium/CDP (M2). Interception left OFF.".into(),
                )
            } else {
                state.tamper().set_interception_enabled(false);
                DispatchResult::Ok
            }
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
        }
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
            let pattern = payload["target_url_pattern"].as_str().unwrap_or("").to_string();
            let key = payload["replace_header_key"].as_str().unwrap_or("").to_string();
            let val = payload["replace_header_value"].as_str().unwrap_or("").to_string();
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
        other => {
            println!("⚡ [Thor IPC] unknown/ignored privileged cmd: {other}");
            DispatchResult::Ok
        }
    }
}
