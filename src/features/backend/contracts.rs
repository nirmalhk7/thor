use super::BackendEngine;

impl BackendEngine {
    // 3. Contract Validation ------------------------------------------------
    /// Validate `value` against a minimal JSON schema (`type`, `required`,
    /// `properties`, `items`). Returns a list of human-readable violations.
    pub fn validate_contract(
        &self,
        value: &serde_json::Value,
        schema: &serde_json::Value,
    ) -> Vec<String> {
        let mut errs = Vec::new();
        validate_json(value, schema, "$", &mut errs);
        errs
    }
}
fn validate_json(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    path: &str,
    errs: &mut Vec<String>,
) {
    if let Some(expected) = schema.get("type").and_then(|t| t.as_str()) {
        if !matches_type(value, expected) {
            errs.push(format!(
                "{path}: expected {expected}, got {}",
                json_type_name(value)
            ));
            return;
        }
    }
    match value {
        serde_json::Value::Object(obj) => validate_object(obj, schema, path, errs),
        serde_json::Value::Array(arr) => validate_array(arr, schema, path, errs),
        _ => {}
    }
}

fn matches_type(value: &serde_json::Value, expected: &str) -> bool {
    match expected {
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        other => other == json_type_name(value),
    }
}

fn validate_object(
    obj: &serde_json::Map<String, serde_json::Value>,
    schema: &serde_json::Value,
    path: &str,
    errs: &mut Vec<String>,
) {
    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
        for key in required.iter().filter_map(|r| r.as_str()) {
            if !obj.contains_key(key) {
                errs.push(format!("{path}: missing required property '{key}'"));
            }
        }
    }
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return;
    };
    for (key, sub_schema) in props {
        if let Some(value) = obj.get(key) {
            validate_json(value, sub_schema, &format!("{path}.{key}"), errs);
        }
    }
}

fn validate_array(
    arr: &[serde_json::Value],
    schema: &serde_json::Value,
    path: &str,
    errs: &mut Vec<String>,
) {
    let Some(items) = schema.get("items") else {
        return;
    };
    for (i, value) in arr.iter().enumerate() {
        validate_json(value, items, &format!("{path}[{i}]"), errs);
    }
}

fn json_type_name(v: &serde_json::Value) -> &'static str {
    use serde_json::Value;
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
