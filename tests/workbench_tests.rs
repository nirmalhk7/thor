use serde_json::json;
use thor::{
    api::HttpService,
    projects::ProjectStore,
    workbench::{is_tool_document, Workbench, WorkbenchCommand},
};

#[tokio::test]
async fn control_center_uses_registered_features_and_keeps_its_command_boundary() {
    let path = std::env::temp_dir().join(format!(
        "thor-control-center-{}.sqlite3",
        std::process::id()
    ));
    let store = ProjectStore::open(&path).unwrap();
    let http = HttpService::new().unwrap();
    let workbench = Workbench::new(&http, &store);
    let list: WorkbenchCommand = serde_json::from_value(json!({"command":"feature_list"})).unwrap();
    assert!(thor::workbench::command_allowed(
        "thor://workbench/tools",
        true,
        &list
    ));
    assert!(!thor::workbench::command_allowed(
        "thor://workbench/security",
        true,
        &list
    ));
    assert!(!thor::workbench::command_allowed(
        "https://workbench/tools",
        true,
        &list
    ));
    let result = workbench.execute(list).await.unwrap();
    assert_eq!(result.as_array().unwrap().len(), 144);
    let invoke: WorkbenchCommand = serde_json::from_value(
        json!({"command":"feature_invoke","slug":"header-analyzer","payload":{"headers":{}}}),
    )
    .unwrap();
    assert!(workbench.execute(invoke).await.unwrap()["result"]["findings"].is_array());
    let send: WorkbenchCommand = serde_json::from_value(json!({"command":"project_list"})).unwrap();
    assert!(!thor::workbench::command_allowed(
        "thor://workbench/tools",
        true,
        &send
    ));
    assert!(thor::workbench::asset("thor://workbench/tools").is_some());
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn address_navigation_preserves_explicit_schemes_and_uses_http_for_loopback() {
    for (input, expected) in [
        (" localhost:8080/api ", "http://localhost:8080/api"),
        ("127.0.0.1:3000", "http://127.0.0.1:3000"),
        ("[::1]:9000/", "http://[::1]:9000/"),
        ("localhost.example.com", "https://localhost.example.com"),
        ("example.com", "https://example.com"),
        ("http://example.test", "http://example.test"),
        ("thor://workbench/api", "thor://workbench/api"),
        ("about:blank", "about:blank"),
        (" ", "thor://workbench/"),
    ] {
        assert_eq!(thor::workbench::navigation_url(input), expected);
    }
}

#[test]
fn workspace_documents_are_served_without_granting_asset_files_tool_access() {
    for url in ["thor://workbench/", "thor://workbench/security"] {
        let asset = thor::workbench::asset(url).expect("workspace document must exist");
        assert_eq!(asset.mime, "text/html");
        assert!(!asset.bytes.is_empty());
    }
    for url in [
        "thor://workbench/",
        "thor://workbench/workspace.js",
        "thor://workbench/security.js",
    ] {
        assert!(!is_tool_document(url, true), "{url}");
    }
}

#[tokio::test]
async fn security_workspace_runs_local_analysis_and_refuses_other_features() {
    let path = std::env::temp_dir().join(format!(
        "thor-security-workspace-{}.sqlite3",
        std::process::id()
    ));
    let store = ProjectStore::open(&path).unwrap();
    let service = HttpService::new().unwrap();
    let workbench = Workbench::new(&service, &store);
    let command = serde_json::from_value::<WorkbenchCommand>(json!({
        "command":"security_analyze", "slug":"header-analyzer", "payload":{"headers":{}}
    }))
    .expect("local security analyzer command");
    let result = workbench.execute(command).await.expect("header analysis");
    assert!(result["result"]["findings"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["present"] == false)));
    let command = serde_json::from_value::<WorkbenchCommand>(json!({
        "command":"security_analyze", "slug":"ssrf-lab", "payload":{}
    }))
    .unwrap();
    assert!(workbench.execute(command).await.is_err());
    drop(store);
    let _ = std::fs::remove_file(path);
}

#[test]
fn security_document_cannot_send_requests_or_write_projects() {
    for payload in [
        json!({"command":"api_send", "request":{"method":"GET","url":"http://localhost/"}}),
        json!({"command":"project_create","name":"Unexpected"}),
    ] {
        let command = serde_json::from_value::<WorkbenchCommand>(payload).unwrap();
        assert!(!thor::workbench::command_allowed(
            "thor://workbench/security",
            true,
            &command
        ));
        assert!(!thor::workbench::command_allowed(
            "https://workbench/api",
            true,
            &command
        ));
        assert!(!thor::workbench::command_allowed(
            "thor://workbench/api",
            false,
            &command
        ));
        assert!(thor::workbench::command_allowed(
            "thor://workbench/api",
            true,
            &command
        ));
    }
}

#[test]
fn privileged_tools_require_the_exact_internal_main_document() {
    assert!(is_tool_document("thor://workbench/api", true));
    for url in [
        "https://workbench/api",
        "thor://workbench.evil/api",
        "thor://user@workbench/api",
        "thor://workbench:123/api",
        "thor://workbench/workbench.js",
        "file:///api",
    ] {
        assert!(!is_tool_document(url, true), "{url}");
    }
    assert!(!is_tool_document("thor://workbench/api", false));
}

#[tokio::test]
async fn saved_api_requests_reopen_from_the_real_project_database() {
    let path = std::env::temp_dir().join(format!("thor-workbench-{}.sqlite3", std::process::id()));
    let store = ProjectStore::open(&path).unwrap();
    let service = HttpService::new().unwrap();
    let workbench = Workbench::new(&service, &store);
    let project = workbench
        .execute(
            serde_json::from_value::<WorkbenchCommand>(
                json!({"command":"project_create", "name":"Local API"}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let request = json!({"method":"POST", "url":"http://localhost:9000/api", "body":{"type":"json", "value":{"name":"Thor"}}});
    let saved = workbench.execute(serde_json::from_value(json!({"command":"request_save", "project":project["id"], "title":"Create item", "request":request})).unwrap()).await.unwrap();
    drop(store);
    let reopened = ProjectStore::open(&path).unwrap();
    let workbench = Workbench::new(&service, &reopened);
    let opened = workbench
        .execute(
            serde_json::from_value(json!({"command":"request_open", "id":saved["id"]})).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(opened["title"], "Create item");
    assert_eq!(opened["request"]["body"]["value"]["name"], "Thor");
    reopened
        .delete_project(&serde_json::from_value(project["id"].clone()).unwrap())
        .unwrap();
    drop(reopened);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn saving_a_request_refuses_plaintext_credentials() {
    let path = std::env::temp_dir().join(format!(
        "thor-workbench-secrets-{}.sqlite3",
        std::process::id()
    ));
    let store = ProjectStore::open(&path).unwrap();
    let project = store.create_project("Secrets").unwrap();
    let service = HttpService::new().unwrap();
    let workbench = Workbench::new(&service, &store);
    let command = json!({"command":"request_save", "project":project.id, "title":"Unsafe", "request":{"method":"GET", "url":"https://example.test/", "headers":[{"name":"Authorization", "value":"Bearer secret-value"}]}});
    assert!(workbench
        .execute(serde_json::from_value(command).unwrap())
        .await
        .is_err());
    assert!(store.documents(&project.id, None).unwrap().is_empty());
    store.delete_project(&project.id).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}
