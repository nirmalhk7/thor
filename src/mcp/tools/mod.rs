//! Request validation and browser tools shared by MCP transports.

mod browser;
mod catalog;

use serde_json::{json, Value};

use crate::engine::engine_coordinator::EngineHandle;

pub(super) fn rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

pub async fn handle_message(engine: Option<&EngineHandle>, payload: Value) -> Option<Value> {
    let id = payload.get("id").cloned().unwrap_or(Value::Null);
    if !valid_request(&payload) {
        return Some(rpc_error(id, -32600, "Invalid Request"));
    }
    payload.get("id")?;
    let method = payload["method"].as_str()?;
    let params = payload.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => initialize(&params),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": catalog::definitions() })),
        "tools/call" => return Some(call_tool(engine, id, &params).await),
        _ => return Some(rpc_error(id, -32601, "Method not found")),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(message) => rpc_error(id, -32602, message),
    })
}

fn valid_request(payload: &Value) -> bool {
    let id_valid = payload.get("id").is_none_or(|id| {
        id.is_null() || id.is_string() || id.as_i64().is_some() || id.as_u64().is_some()
    });
    payload.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
        && payload.get("method").is_some_and(Value::is_string)
        && payload.get("params").is_none_or(Value::is_object)
        && id_valid
}

fn initialize(params: &Value) -> Result<Value, &'static str> {
    let version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .ok_or("protocolVersion required")?;
    let supported = ["2024-11-05", "2025-03-26", "2025-06-18"];
    let negotiated = if supported.contains(&version) {
        version
    } else {
        "2025-06-18"
    };
    Ok(json!({
        "protocolVersion": negotiated,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "Thor", "version": env!("CARGO_PKG_VERSION") }
    }))
}

async fn call_tool(engine: Option<&EngineHandle>, id: Value, params: &Value) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return rpc_error(id, -32602, "Tool name required");
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Err(message) = catalog::validate(name, &arguments) {
        return rpc_error(id, -32602, &message);
    }
    let result = match engine {
        Some(engine) => browser::call(engine, name, &arguments).await,
        None => Err("Browser engine unavailable".into()),
    };
    let result = match result {
        Ok(content) => json!({ "content": content, "isError": false }),
        Err(message) => {
            json!({ "content": [{ "type": "text", "text": message }], "isError": true })
        }
    };
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
