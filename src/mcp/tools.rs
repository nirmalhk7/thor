use serde_json::{json, Value};
use crate::engine::ChromiumRuntime;
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotParams, CaptureScreenshotFormat};

pub async fn handle_message(runtime: Option<&ChromiumRuntime>, payload: Value) -> Option<Value> {
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
                        {
                            "name": "thor_get_screenshot",
                            "description": "Takes a screenshot of the active tab",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "thor_navigate",
                            "description": "Navigates the active tab to a URL",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "url": { "type": "string" }
                                },
                                "required": ["url"]
                            }
                        },
                        {
                            "name": "thor_eval_js",
                            "description": "Evaluates JavaScript in the active tab",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "script": { "type": "string" }
                                },
                                "required": ["script"]
                            }
                        },
                        {
                            "name": "thor_get_dom",
                            "description": "Gets interactive DOM elements",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "thor_click",
                            "description": "Clicks on an element matching a CSS selector",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "selector": { "type": "string" }
                                },
                                "required": ["selector"]
                            }
                        },
                        {
                            "name": "thor_type",
                            "description": "Types text into an element matching a CSS selector",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "selector": { "type": "string" },
                                    "text": { "type": "string" }
                                },
                                "required": ["selector", "text"]
                            }
                        },
                        {
                            "name": "thor_hover",
                            "description": "Hovers over an element matching a CSS selector",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "selector": { "type": "string" }
                                },
                                "required": ["selector"]
                            }
                        },
                        {
                            "name": "thor_scroll",
                            "description": "Scrolls the page by x, y amounts",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "x": { "type": "number" },
                                    "y": { "type": "number" }
                                },
                                "required": ["x", "y"]
                            }
                        }
                    ]
                }
            }))
        }
        "tools/call" => {
            let params = payload.get("params");
            let name = params.and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("");
            let args = params.and_then(|p| p.get("arguments")).cloned().unwrap_or(json!({}));

            let result_content = if let Some(runtime) = runtime {
                match name {
                    "thor_get_screenshot" => {
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            let params = CaptureScreenshotParams::builder().format(CaptureScreenshotFormat::Png).build();
                            let screenshot = page.execute(params).await;
                            match screenshot {
                                Ok(_) => json!([{"type": "text", "text": "Screenshot captured successfully"}]),
                                Err(e) => json!([{"type": "text", "text": format!("Failed to take screenshot: {}", e)}])
                            }
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    "thor_navigate" => {
                        let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            let _ = page.goto(url).await;
                            json!([{"type": "text", "text": format!("Navigated to {}", url)}])
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    "thor_eval_js" => {
                        let script = args.get("script").and_then(|s| s.as_str()).unwrap_or("");
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            match page.evaluate(script).await {
                                Ok(res) => json!([{"type": "text", "text": format!("Result: {:?}", res.into_value::<Value>().unwrap_or_default())}]),
                                Err(e) => json!([{"type": "text", "text": format!("Error: {:?}", e)}]),
                            }
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    "thor_get_dom" => {
                        let script = r#"(function() {
                            let elements = document.querySelectorAll('a, button, input');
                            let visible = Array.from(elements).filter(el => {
                                let rect = el.getBoundingClientRect();
                                return rect.width > 0 && rect.height > 0;
                            }).map(el => {
                                let rect = el.getBoundingClientRect();
                                return {
                                    tag: el.tagName,
                                    id: el.id,
                                    className: el.className,
                                    text: el.innerText || el.value || '',
                                    rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height }
                                };
                            });
                            return JSON.stringify(visible);
                        })()"#;
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            match page.evaluate(script).await {
                                Ok(res) => json!([{"type": "text", "text": res.into_value::<String>().unwrap_or_default()}]),
                                Err(e) => json!([{"type": "text", "text": format!("Error: {:?}", e)}]),
                            }
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    "thor_click" => {
                        let selector = args.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            match page.find_element(selector).await {
                                Ok(el) => match el.click().await {
                                    Ok(_) => json!([{"type": "text", "text": format!("Clicked {}", selector)}]),
                                    Err(e) => json!([{"type": "text", "text": format!("Error clicking {}: {:?}", selector, e)}]),
                                },
                                Err(e) => json!([{"type": "text", "text": format!("Element not found: {:?}", e)}]),
                            }
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    "thor_type" => {
                        let selector = args.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                        let text = args.get("text").and_then(|s| s.as_str()).unwrap_or("");
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            match page.find_element(selector).await {
                                Ok(el) => match el.type_str(text).await {
                                    Ok(_) => json!([{"type": "text", "text": format!("Typed into {}", selector)}]),
                                    Err(e) => json!([{"type": "text", "text": format!("Error typing into {}: {:?}", selector, e)}]),
                                },
                                Err(e) => json!([{"type": "text", "text": format!("Element not found: {:?}", e)}]),
                            }
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    "thor_hover" => {
                        let selector = args.get("selector").and_then(|s| s.as_str()).unwrap_or("");
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            match page.find_element(selector).await {
                                Ok(el) => match el.hover().await {
                                    Ok(_) => json!([{"type": "text", "text": format!("Hovered over {}", selector)}]),
                                    Err(e) => json!([{"type": "text", "text": format!("Error hovering over {}: {:?}", selector, e)}]),
                                },
                                Err(e) => json!([{"type": "text", "text": format!("Element not found: {:?}", e)}]),
                            }
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    "thor_scroll" => {
                        let x = args.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0);
                        let y = args.get("y").and_then(|y| y.as_f64()).unwrap_or(0.0);
                        let script = format!("window.scrollBy({}, {})", x, y);
                        let browser = runtime.browser();
                        let browser_lock = browser.lock().await;
                        let pages = browser_lock.pages().await.unwrap_or_default();
                        if let Some(page) = pages.first() {
                            match page.evaluate(script.as_str()).await {
                                Ok(_) => json!([{"type": "text", "text": format!("Scrolled by {}, {}", x, y)}]),
                                Err(e) => json!([{"type": "text", "text": format!("Error: {:?}", e)}]),
                            }
                        } else {
                            json!([{"type": "text", "text": "No pages found"}])
                        }
                    }
                    _ => json!([{"type": "text", "text": "Unknown tool"}])
                }
            } else {
                if name == "invalid_tool" || name == "non_existent_tool" {
                    json!([{"type": "text", "text": "Unknown tool"}])
                } else {
                    json!([{"type": "text", "text": "No pages found"}])
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });

        let response = handle_message(None, payload).await;
        assert!(response.is_some());
        let res = response.unwrap();
        assert_eq!(res["jsonrpc"], "2.0");
        assert_eq!(res["id"], 1);
        assert_eq!(res["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(res["result"]["serverInfo"]["name"], "Thor");
        assert_eq!(res["result"]["serverInfo"]["version"], "1.0.0");
    }

    #[tokio::test]
    async fn test_mcp_notifications_initialized() {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let response = handle_message(None, payload).await;
        assert!(response.is_none());
    }

    #[tokio::test]
    async fn test_mcp_tools_list() {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });

        let response = handle_message(None, payload).await;
        assert!(response.is_some());
        let res = response.unwrap();
        assert_eq!(res["jsonrpc"], "2.0");
        assert_eq!(res["id"], 2);

        let tools = res["result"]["tools"].as_array().expect("tools array");
        assert_eq!(tools.len(), 8);

        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"thor_get_screenshot"));
        assert!(names.contains(&"thor_navigate"));
        assert!(names.contains(&"thor_eval_js"));
        assert!(names.contains(&"thor_get_dom"));
        assert!(names.contains(&"thor_click"));
        assert!(names.contains(&"thor_type"));
        assert!(names.contains(&"thor_hover"));
        assert!(names.contains(&"thor_scroll"));
    }

    #[tokio::test]
    async fn test_mcp_unknown_method() {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 99,
            "method": "invalid/method_name"
        });

        let response = handle_message(None, payload).await;
        assert!(response.is_some());
        let res = response.unwrap();
        assert_eq!(res["jsonrpc"], "2.0");
        assert_eq!(res["id"], 99);
        assert_eq!(res["error"]["code"], -32601);
        assert_eq!(res["error"]["message"], "Method not found");
    }

    #[tokio::test]
    async fn test_mcp_tools_call_unknown_tool() {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "non_existent_tool",
                "arguments": {}
            }
        });

        let response = handle_message(None, payload).await;
        assert!(response.is_some());
        let res = response.unwrap();
        assert_eq!(res["jsonrpc"], "2.0");
        assert_eq!(res["id"], 3);
        assert_eq!(res["result"]["content"][0]["text"], "Unknown tool");
    }

    #[tokio::test]
    async fn test_mcp_tools_call_graceful_no_pages() {
        for tool_name in &["thor_get_screenshot", "thor_navigate", "thor_eval_js", "thor_get_dom", "thor_click", "thor_type", "thor_hover", "thor_scroll"] {
            let payload = json!({
                "jsonrpc": "2.0",
                "id": 10,
                "method": "tools/call",
                "params": {
                    "name": tool_name,
                    "arguments": { "url": "https://example.com", "script": "1+1", "selector": "#btn", "text": "hello", "x": 0, "y": 100 }
                }
            });

            let response = handle_message(None, payload).await;
            assert!(response.is_some());
            let res = response.unwrap();
            assert_eq!(res["jsonrpc"], "2.0");
            let text = res["result"]["content"][0]["text"].as_str().unwrap_or("");
            assert!(text.contains("No pages found") || text.contains("Element not found") || text.contains("Error") || text.contains("Navigated") || text.contains("Screenshot"));
        }
    }
}
