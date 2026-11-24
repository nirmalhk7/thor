use serde_json::Value;

use super::commands::encode;

const MAX_REQUEST_BYTES: usize = 256 * 1024;

pub(super) fn inspect_request(body: Value) -> Result<Value, String> {
    if serde_json::to_vec(&body)
        .map_err(|_| "Invalid request JSON")?
        .len()
        > MAX_REQUEST_BYTES
    {
        return Err("Use request JSON no larger than 256 KiB.".into());
    }
    if !body
        .get("model")
        .and_then(Value::as_str)
        .is_some_and(|model| !model.trim().is_empty())
    {
        return Err("Add a non-empty model name to the request body.".into());
    }

    match (body.get("messages"), body.get("prompt")) {
        (Some(Value::Array(messages)), None)
            if !messages.is_empty() && messages.iter().all(is_text_message) => {}
        (None, Some(Value::String(prompt))) if !prompt.trim().is_empty() => {}
        _ => {
            return Err(
                "Supply either a text prompt or a non-empty messages array with a role and text content for each message."
                    .into(),
            )
        }
    }

    let summary = crate::AppState::global()
        .features()
        .ai()
        .inspect_request(&body);
    encode(summary)
}

fn is_text_message(message: &Value) -> bool {
    message
        .get("role")
        .and_then(Value::as_str)
        .is_some_and(|role| !role.trim().is_empty())
        && message.get("content").is_some_and(Value::is_string)
}
