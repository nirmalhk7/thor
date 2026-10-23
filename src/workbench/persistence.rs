use serde_json::{json, Value};

use crate::api::{RequestBody, RequestSpec};
use crate::projects::{DocumentKind, ProjectStore};

use super::commands::{encode, WorkbenchCommand};

pub(super) fn execute(store: &ProjectStore, command: WorkbenchCommand) -> Result<Value, String> {
    match command {
        WorkbenchCommand::ProjectList => encode(store.projects().map_err(message)?),
        WorkbenchCommand::ProjectCreate { name } => encode(store.create_project(&name).map_err(message)?),
        WorkbenchCommand::RequestList { project } => encode(store.documents(&project, Some(DocumentKind::Request)).map_err(message)?),
        WorkbenchCommand::RequestOpen { id } => {
            let document = store.document(&id).map_err(message)?;
            if document.kind != DocumentKind::Request { return Err("Document is not an API request".into()); }
            let request: RequestSpec = serde_json::from_value(document.payload["request"].clone()).map_err(|_| "Invalid saved request".to_string())?;
            Ok(json!({"id":document.id,"revision":document.revision,"project":document.project_id,"title":document.title,"request":request}))
        }
        WorkbenchCommand::RequestSave { project, title, request, id, revision } => {
            safe_to_save(&request)?;
            let payload = json!({"version":1,"request":request});
            let document = match (id, revision) {
                (None, None) => store.create_document(&project, DocumentKind::Request, &title, payload),
                (Some(id), Some(revision)) => {
                    let document = store.document(&id).map_err(message)?;
                    if document.project_id != project || document.kind != DocumentKind::Request { return Err("Request does not belong to selected project".into()); }
                    store.update_document(&id, revision, &title, payload)
                }
                _ => return Err("Updating a request requires its identifier and revision".into()),
            }.map_err(message)?;
            encode(document)
        }
        _ => Err("Unsupported project operation".into()),
    }
}

fn message(error: impl std::fmt::Display) -> String { error.to_string() }

fn sensitive(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "authorization" | "proxy-authorization" | "cookie" | "set-cookie" | "x-api-key" | "api_key" | "apikey" | "access_token" | "refresh_token" | "password" | "client_secret" | "token" | "secret")
}

fn placeholder(value: &str) -> bool {
    let value = value.trim().strip_prefix("Bearer ").unwrap_or(value.trim());
    value.starts_with("{{") && value.ends_with("}}") && value.matches("{{").count() == 1
}

fn secret_value(name: &str, value: &str) -> bool { sensitive(name) && !value.is_empty() && !placeholder(value) }

fn safe_to_save(request: &RequestSpec) -> Result<(), String> {
    let unsafe_headers = request.headers.iter().chain(&request.query).any(|header| secret_value(&header.name, &header.value));
    let url = reqwest::Url::parse(&request.url).ok();
    let unsafe_url = url.as_ref().is_some_and(|url| {
        !url.username().is_empty() || url.password().is_some() || url.query_pairs().any(|(name, value)| secret_value(&name, &value))
    });
    let unsafe_body = match &request.body {
        RequestBody::Json { value } => json_has_secret(value),
        RequestBody::Form { entries } => entries.iter().any(|header| secret_value(&header.name, &header.value)),
        RequestBody::Text { text, .. } => serde_json::from_str(text).is_ok_and(|value| json_has_secret(&value)),
        _ => false,
    };
    if unsafe_headers || unsafe_url || unsafe_body {
        return Err("Use credential references or variables for secrets before saving this request".into());
    }
    Ok(())
}

fn json_has_secret(value: &Value) -> bool {
    match value {
        Value::Object(values) => values.iter().any(|(name, value)| {
            value.as_str().is_some_and(|value| secret_value(name, value)) || json_has_secret(value)
        }),
        Value::Array(values) => values.iter().any(json_has_secret),
        _ => false,
    }
}
