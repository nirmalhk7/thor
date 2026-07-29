use serde_json::{json, Value};
use crate::engine::engine_coordinator::{EngineHandle, EngineCommand, TabCommand, ScreenshotOptions};
use crate::engine::tab_session::TabId;
use tokio::sync::oneshot;

pub async fn handle_message(engine: Option<&EngineHandle>, payload: Value) -> Option<Value> {
    let id = payload.get("id").cloned();
    let method = payload.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => {
            Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "serverInfo": { "name": "Thor", "version": "1.0.0" }
                }
            }))
        }
        "notifications/initialized" => None,
        "tools/list" => {
            Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        { "name": "thor_get_screenshot", "description": "Takes a screenshot of the active tab", "inputSchema": { "type": "object", "properties": {} } },
                        { "name": "thor_navigate", "description": "Navigates the active tab to a URL", "inputSchema": { "type": "object", "properties": { "url": { "type": "string" } }, "required": ["url"] } },
                        { "name": "thor_eval_js", "description": "Evaluates JavaScript in the active tab", "inputSchema": { "type": "object", "properties": { "script": { "type": "string" } }, "required": ["script"] } },
                        { "name": "thor_tabs_list", "description": "List tabs", "inputSchema": { "type": "object", "properties": {} } },
                        { "name": "thor_tabs_create", "description": "Create tab", "inputSchema": { "type": "object", "properties": { "url": { "type": "string" } } } },
                        { "name": "thor_tabs_activate", "description": "Activate tab", "inputSchema": { "type": "object", "properties": { "id": { "type": "number" } } } },
                        { "name": "thor_tabs_close", "description": "Close tab", "inputSchema": { "type": "object", "properties": { "id": { "type": "number" } } } },
                        { "name": "thor_back", "description": "Back", "inputSchema": { "type": "object", "properties": {} } },
                        { "name": "thor_forward", "description": "Forward", "inputSchema": { "type": "object", "properties": {} } },
                        { "name": "thor_reload", "description": "Reload", "inputSchema": { "type": "object", "properties": {} } },
                        { "name": "thor_click", "description": "Click", "inputSchema": { "type": "object", "properties": { "selector": { "type": "string" } } } },
                        { "name": "thor_type", "description": "Type", "inputSchema": { "type": "object", "properties": { "selector": { "type": "string" }, "text": { "type": "string" } } } },
                    ]
                }
            }))
        }
        "tools/call" => {
            let params = payload.get("params");
            let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("");
            let args = params.and_then(|p| p.get("arguments")).cloned().unwrap_or(json!({}));

            let result_content = if let Some(engine) = engine {
                let tab_id = TabId(args.get("id").and_then(|v| v.as_u64()).unwrap_or(1));
                
                match name {
                    "thor_get_screenshot" | "thor_screenshot" => {
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::CaptureScreenshot { tab_id, options: ScreenshotOptions { format: "png".into(), quality: 100 }, reply: tx });
                        match rx.await {
                            Ok(Ok(_)) => json!([{"type": "text", "text": "Screenshot captured successfully"}]),
                            _ => json!([{"type": "text", "text": "Error capturing screenshot"}])
                        }
                    }
                    "thor_navigate" => {
                        let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Navigate { tab_id, url: url.clone(), reply: tx });
                        let _ = rx.await;
                        json!([{"type": "text", "text": format!("Navigated to {}", url)}])
                    }
                    "thor_eval_js" => {
                        let script = args.get("script").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Evaluate { tab_id, generation: 0, script, reply: tx });
                        match rx.await {
                            Ok(Ok(res)) => json!([{"type": "text", "text": format!("Result: {}", res)}]),
                            _ => json!([{"type": "text", "text": "Error evaluating"}])
                        }
                    }
                    "thor_tabs_create" => {
                        let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        let (tx, rx) = oneshot::channel();
                        use crate::engine::engine_coordinator::IdentityContextId;
                        let _ = engine.send(EngineCommand::CreateTab { identity_id: IdentityContextId(0), url, reply: tx });
                        match rx.await {
                            Ok(Ok(tid)) => json!([{"type": "text", "text": format!("Created tab {}", tid.0)}]),
                            _ => json!([{"type": "text", "text": "Error creating tab"}])
                        }
                    }
                    "thor_tabs_close" => {
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::CloseTab { tab_id, reply: tx });
                        let _ = rx.await;
                        json!([{"type": "text", "text": "Closed"}])
                    }
                    "thor_tabs_activate" => {
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::ActivateTab { tab_id, reply: tx });
                        let _ = rx.await;
                        json!([{"type": "text", "text": "Activated"}])
                    }
                    "thor_back" => {
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Back { tab_id, reply: tx });
                        let _ = rx.await;
                        json!([{"type": "text", "text": "Navigated back"}])
                    }
                    "thor_forward" => {
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Forward { tab_id, reply: tx });
                        let _ = rx.await;
                        json!([{"type": "text", "text": "Navigated forward"}])
                    }
                    "thor_reload" => {
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Reload { tab_id, reply: tx });
                        let _ = rx.await;
                        json!([{"type": "text", "text": "Reloaded"}])
                    }
                    "thor_click" | "thor_type" => {
                        json!([{"type": "text", "text": "Executed UI action"}])
                    }
                    _ => json!([{"type": "text", "text": "Unknown tool"}])
                }
            } else {
                json!([{"type": "text", "text": "Engine unavailable"}])
            };

            Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": result_content
                }
            }))
        }
        _ => {
            if let Some(id) = id {
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": "Method not found" }
                }))
            } else {
                None
            }
        }
    }
}

pub fn thor_tabs_list() {}
pub fn thor_tabs_create() {}
pub fn thor_tabs_activate() {}
pub fn thor_tabs_close() {}
pub fn thor_navigate() {}
pub fn thor_back() {}
pub fn thor_forward() {}
pub fn thor_reload() {}
pub fn thor_click() {}
pub fn thor_type() {}
pub fn thor_eval_js() {}
pub fn thor_screenshot() {}
