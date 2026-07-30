use serde_json::{json, Value};
use crate::engine::engine_coordinator::{EngineHandle, EngineCommand, ScreenshotOptions};
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
                        { "name": "thor_click", "description": "Click an element by CSS selector in the active tab", "inputSchema": { "type": "object", "properties": { "selector": { "type": "string" }, "id": { "type": "number" } }, "required": ["selector"] } },
                        { "name": "thor_type", "description": "Type text into an element by CSS selector", "inputSchema": { "type": "object", "properties": { "selector": { "type": "string" }, "text": { "type": "string" }, "id": { "type": "number" } }, "required": ["selector", "text"] } },
                        { "name": "thor_get_dom", "description": "Return outerHTML of documentElement (truncated)", "inputSchema": { "type": "object", "properties": { "id": { "type": "number" }, "max_chars": { "type": "number" } } } },
                        { "name": "thor_hover", "description": "Hover an element by CSS selector", "inputSchema": { "type": "object", "properties": { "selector": { "type": "string" }, "id": { "type": "number" } }, "required": ["selector"] } },
                        { "name": "thor_scroll", "description": "Scroll window or element into view", "inputSchema": { "type": "object", "properties": { "selector": { "type": "string" }, "id": { "type": "number" }, "y": { "type": "number" } } } },
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
                            Ok(Ok(bytes)) if !bytes.is_empty() => {
                                let b64 = {
                                    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
                                    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
                                    let mut i = 0;
                                    while i + 3 <= bytes.len() {
                                        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
                                        out.push(T[((n >> 18) & 63) as usize] as char);
                                        out.push(T[((n >> 12) & 63) as usize] as char);
                                        out.push(T[((n >> 6) & 63) as usize] as char);
                                        out.push(T[(n & 63) as usize] as char);
                                        i += 3;
                                    }
                                    let rem = bytes.len() - i;
                                    if rem == 1 {
                                        let n = (bytes[i] as u32) << 16;
                                        out.push(T[((n >> 18) & 63) as usize] as char);
                                        out.push(T[((n >> 12) & 63) as usize] as char);
                                        out.push('=');
                                        out.push('=');
                                    } else if rem == 2 {
                                        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
                                        out.push(T[((n >> 18) & 63) as usize] as char);
                                        out.push(T[((n >> 12) & 63) as usize] as char);
                                        out.push(T[((n >> 6) & 63) as usize] as char);
                                        out.push('=');
                                    }
                                    out
                                };
                                json!([{
                                    "type": "text",
                                    "text": format!("Screenshot captured ({} bytes)", bytes.len())
                                }, {
                                    "type": "image",
                                    "data": b64,
                                    "mimeType": "image/png"
                                }])
                            }
                            Ok(Ok(_)) => json!([{"type": "text", "text": "Error: empty screenshot"}]),
                            _ => json!([{"type": "text", "text": "Error capturing screenshot"}])
                        }
                    }
                    "thor_tabs_list" => {
                        let snap = engine.snapshot_rx.borrow().clone();
                        let tabs: Vec<Value> = snap
                            .tabs
                            .iter()
                            .map(|t| json!({ "id": t.0 }))
                            .collect();
                        json!([{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&json!({
                                "active_tabs": snap.active_tabs,
                                "runtime_status": snap.runtime_status,
                                "tabs": tabs,
                                "load": snap.load
                            })).unwrap_or_else(|_| "{}".into())
                        }])
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
                    "thor_click" => {
                        let selector = args.get("selector").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        if selector.is_empty() {
                            json!([{"type": "text", "text": "Error: selector required"}])
                        } else {
                            let script = format!(
                                r#"(function(){{
  const el = document.querySelector({});
  if (!el) return JSON.stringify({{ok:false, error:'not-found'}});
  el.scrollIntoView({{block:'center', inline:'nearest'}});
  el.dispatchEvent(new MouseEvent('click', {{bubbles:true, cancelable:true, view:window}}));
  if (typeof el.click === 'function') el.click();
  return JSON.stringify({{ok:true, tag: el.tagName}});
}})()"#,
                                serde_json::to_string(&selector).unwrap_or_else(|_| "\"\"".into())
                            );
                            let (tx, rx) = oneshot::channel();
                            let _ = engine.send(EngineCommand::Evaluate { tab_id, generation: 0, script, reply: tx });
                            match rx.await {
                                Ok(Ok(res)) => {
                                    let s = res.to_string();
                                    if s.contains("\"ok\":true") || s.contains("\"ok\": true") {
                                        json!([{"type": "text", "text": format!("Clicked {selector}")}])
                                    } else {
                                        json!([{"type": "text", "text": format!("Click failed for {selector}: {s}")}])
                                    }
                                }
                                _ => json!([{"type": "text", "text": "Error: engine evaluate failed for click"}])
                            }
                        }
                    }
                    "thor_type" => {
                        let selector = args.get("selector").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        let text = args.get("text").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        if selector.is_empty() {
                            json!([{"type": "text", "text": "Error: selector required"}])
                        } else {
                            let script = format!(
                                r#"(function(){{
  const el = document.querySelector({});
  if (!el) return JSON.stringify({{ok:false, error:'not-found'}});
  el.focus();
  const text = {};
  if ('value' in el) {{
    el.value = text;
    el.dispatchEvent(new Event('input', {{bubbles:true}}));
    el.dispatchEvent(new Event('change', {{bubbles:true}}));
  }} else if (el.isContentEditable) {{
    el.textContent = text;
    el.dispatchEvent(new Event('input', {{bubbles:true}}));
  }} else {{
    return JSON.stringify({{ok:false, error:'not-editable'}});
  }}
  return JSON.stringify({{ok:true}});
}})()"#,
                                serde_json::to_string(&selector).unwrap_or_else(|_| "\"\"".into()),
                                serde_json::to_string(&text).unwrap_or_else(|_| "\"\"".into())
                            );
                            let (tx, rx) = oneshot::channel();
                            let _ = engine.send(EngineCommand::Evaluate { tab_id, generation: 0, script, reply: tx });
                            match rx.await {
                                Ok(Ok(res)) => {
                                    let s = res.to_string();
                                    if s.contains("\"ok\":true") || s.contains("\"ok\": true") {
                                        json!([{"type": "text", "text": format!("Typed into {selector}")}])
                                    } else {
                                        json!([{"type": "text", "text": format!("Type failed for {selector}: {s}")}])
                                    }
                                }
                                _ => json!([{"type": "text", "text": "Error: engine evaluate failed for type"}])
                            }
                        }
                    }
                    "thor_get_dom" => {
                        let max = args.get("max_chars").and_then(|v| v.as_u64()).unwrap_or(100_000) as usize;
                        let script = format!(
                            r#"(function(){{
  const html = document.documentElement ? document.documentElement.outerHTML : '';
  return html.slice(0, {});
}})()"#,
                            max
                        );
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Evaluate { tab_id, generation: 0, script, reply: tx });
                        match rx.await {
                            Ok(Ok(res)) => json!([{"type": "text", "text": res.to_string()}]),
                            _ => json!([{"type": "text", "text": "Error: get_dom failed"}])
                        }
                    }
                    "thor_hover" => {
                        let selector = args.get("selector").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        let script = format!(
                            r#"(function(){{
  const el = document.querySelector({});
  if (!el) return JSON.stringify({{ok:false, error:'not-found'}});
  el.dispatchEvent(new MouseEvent('mouseover', {{bubbles:true}}));
  el.dispatchEvent(new MouseEvent('mouseenter', {{bubbles:true}}));
  return JSON.stringify({{ok:true}});
}})()"#,
                            serde_json::to_string(&selector).unwrap_or_else(|_| "\"\"".into())
                        );
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Evaluate { tab_id, generation: 0, script, reply: tx });
                        match rx.await {
                            Ok(Ok(res)) => json!([{"type": "text", "text": format!("Hover: {res}")}]),
                            _ => json!([{"type": "text", "text": "Error: hover failed"}])
                        }
                    }
                    "thor_scroll" => {
                        let selector = args.get("selector").and_then(|u| u.as_str()).unwrap_or("").to_string();
                        let y = args.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let script = if selector.is_empty() {
                            format!("window.scrollTo(0, {y}); 'scrolled';")
                        } else {
                            format!(
                                r#"(function(){{
  const el = document.querySelector({});
  if (!el) return 'not-found';
  el.scrollIntoView({{block:'center'}});
  return 'ok';
}})()"#,
                                serde_json::to_string(&selector).unwrap_or_else(|_| "\"\"".into())
                            )
                        };
                        let (tx, rx) = oneshot::channel();
                        let _ = engine.send(EngineCommand::Evaluate { tab_id, generation: 0, script, reply: tx });
                        match rx.await {
                            Ok(Ok(res)) => json!([{"type": "text", "text": format!("Scroll: {res}")}]),
                            _ => json!([{"type": "text", "text": "Error: scroll failed"}])
                        }
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
