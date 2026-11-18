use super::{AiEngine, JsonSchemaGuardReport};

impl AiEngine {
    pub fn json_schema_guard(
        &self,
        value: &serde_json::Value,
        schema: &serde_json::Value,
    ) -> JsonSchemaGuardReport {
        let errors = validate_json_schema_min(value, schema, "$", 0);
        JsonSchemaGuardReport {
            ok: errors.is_empty(),
            errors,
        }
    }
}

fn validate_json_schema_min(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    path: &str,
    depth: usize,
) -> Vec<String> {
    const MAX_DEPTH: usize = 32;
    let mut errors = Vec::new();
    if depth > MAX_DEPTH {
        errors.push(format!("{path}: exceeded max schema depth {MAX_DEPTH}"));
        return errors;
    }
    match schema.get("type").and_then(|v| v.as_str()).unwrap_or("") {
        "object" => validate_object(value, schema, path, depth, &mut errors),
        "array" => validate_array(value, schema, path, depth, &mut errors),
        "string" if !value.is_string() => errors.push(format!("{path}: expected string")),
        "number" | "integer" if !value.is_number() => {
            errors.push(format!("{path}: expected number"))
        }
        "boolean" if !value.is_boolean() => errors.push(format!("{path}: expected boolean")),
        _ => {}
    }
    errors
}

fn validate_object(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    path: &str,
    depth: usize,
    errors: &mut Vec<String>,
) {
    let Some(object) = value.as_object() else {
        errors.push(format!("{path}: expected object"));
        return;
    };
    validate_required(object, schema, path, errors);
    let properties = schema
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    validate_properties(object, &properties, path, depth, errors);
    match schema.get("additionalProperties") {
        Some(serde_json::Value::Bool(false)) => {
            for key in object.keys() {
                if !properties.contains_key(key) {
                    errors.push(format!("{path}: additional property '{key}' not allowed"));
                }
            }
        }
        Some(sub) if sub.is_object() => {
            for (key, value) in object {
                if !properties.contains_key(key) {
                    errors.extend(validate_json_schema_min(
                        value,
                        sub,
                        &format!("{path}.{key}"),
                        depth + 1,
                    ));
                }
            }
        }
        _ => {}
    }
}

fn validate_required(
    object: &serde_json::Map<String, serde_json::Value>,
    schema: &serde_json::Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
        for item in required {
            if let Some(key) = item.as_str() {
                if object.get(key).is_none() {
                    errors.push(format!("{path}: missing required '{key}'"));
                }
            }
        }
    }
}

fn validate_properties(
    object: &serde_json::Map<String, serde_json::Value>,
    properties: &serde_json::Map<String, serde_json::Value>,
    path: &str,
    depth: usize,
    errors: &mut Vec<String>,
) {
    for (key, sub) in properties {
        if let Some(value) = object.get(key) {
            errors.extend(validate_json_schema_min(
                value,
                sub,
                &format!("{path}.{key}"),
                depth + 1,
            ));
        }
    }
}

fn validate_array(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    path: &str,
    depth: usize,
    errors: &mut Vec<String>,
) {
    let Some(array) = value.as_array() else {
        errors.push(format!("{path}: expected array"));
        return;
    };
    if let Some(items) = schema.get("items") {
        for (index, value) in array.iter().enumerate() {
            errors.extend(validate_json_schema_min(
                value,
                items,
                &format!("{path}[{index}]"),
                depth + 1,
            ));
        }
    }
}
