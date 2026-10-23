use serde_json::{json, Value};

fn definition(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
) -> Value {
    let mut properties = properties;
    properties["id"] = json!({ "type": "integer", "minimum": 1 });
    properties["generation"] = json!({ "type": "integer", "minimum": 1 });
    json!({
        "name": name, "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": !read_only, "openWorldHint": true }
    })
}

pub(super) fn definitions() -> Vec<Value> {
    let string = json!({ "type": "string", "minLength": 1 });
    let selector = json!({ "selector": string });
    vec![
        definition(
            "thor_tabs_list",
            "List open tabs and navigation targets",
            json!({}),
            &[],
            true,
        ),
        definition(
            "thor_tabs_create",
            "Create a tab in a selected identity",
            json!({ "url": string, "identity_id": { "type": "integer", "minimum": 0 } }),
            &[],
            false,
        ),
        definition(
            "thor_tabs_activate",
            "Activate an existing tab",
            json!({}),
            &["id"],
            false,
        ),
        definition(
            "thor_tabs_close",
            "Close a tab and erase unsaved browsing state",
            json!({}),
            &["id"],
            false,
        ),
        definition(
            "thor_navigate",
            "Start navigation in the selected tab",
            json!({ "url": string }),
            &["url"],
            false,
        ),
        definition(
            "thor_back",
            "Navigate back in the selected tab",
            json!({}),
            &[],
            false,
        ),
        definition(
            "thor_forward",
            "Navigate forward in the selected tab",
            json!({}),
            &[],
            false,
        ),
        definition(
            "thor_reload",
            "Reload the selected tab",
            json!({}),
            &[],
            false,
        ),
        definition(
            "thor_eval_js",
            "Evaluate JavaScript in a resolved navigation target",
            json!({ "script": string }),
            &["script"],
            false,
        ),
        definition(
            "thor_get_screenshot",
            "Capture the selected tab as PNG",
            json!({}),
            &[],
            true,
        ),
        definition(
            "thor_get_dom",
            "Read document HTML with a size limit",
            json!({ "max_chars": { "type": "integer", "minimum": 1, "maximum": 1_000_000 } }),
            &[],
            true,
        ),
        definition(
            "thor_click",
            "Click a document element",
            selector.clone(),
            &["selector"],
            false,
        ),
        definition(
            "thor_type",
            "Set text and dispatch input/change events",
            json!({ "selector": string, "text": { "type": "string" } }),
            &["selector", "text"],
            false,
        ),
        definition(
            "thor_hover",
            "Dispatch mouseover to an element",
            selector,
            &["selector"],
            false,
        ),
        definition(
            "thor_scroll",
            "Scroll the window or an element",
            json!({ "selector": string, "y": { "type": "number" } }),
            &[],
            false,
        ),
    ]
}

pub(super) fn validate(name: &str, arguments: &Value) -> Result<(), String> {
    let definition = definitions()
        .into_iter()
        .find(|entry| entry["name"] == name)
        .ok_or("Unknown tool")?;
    let object = arguments
        .as_object()
        .ok_or("Tool arguments must be an object")?;
    let schema = &definition["inputSchema"];
    for required in schema["required"].as_array().into_iter().flatten() {
        let key = required.as_str().ok_or("Invalid tool schema")?;
        if !object.contains_key(key) {
            return Err(format!("Missing argument: {key}"));
        }
    }
    for (key, value) in object {
        let property = schema["properties"]
            .get(key)
            .ok_or_else(|| format!("Unknown argument: {key}"))?;
        validate_property(key, value, property)?;
    }
    Ok(())
}

fn validate_property(key: &str, value: &Value, schema: &Value) -> Result<(), String> {
    let valid_type = match schema["type"].as_str() {
        Some("string") => value
            .as_str()
            .is_some_and(|text| schema.get("minLength").is_none() || !text.is_empty()),
        Some("integer") => value.as_u64().is_some(),
        Some("number") => value.is_number(),
        _ => false,
    };
    let within_minimum = schema
        .get("minimum")
        .is_none_or(|minimum| value.as_f64() >= minimum.as_f64());
    let within_maximum = schema
        .get("maximum")
        .is_none_or(|maximum| value.as_f64() <= maximum.as_f64());
    if !valid_type || !within_minimum || !within_maximum {
        return Err(format!("Invalid argument: {key}"));
    }
    Ok(())
}
