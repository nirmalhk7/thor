use serde_json::json;
use thor::{api::HttpService, projects::ProjectStore, workbench::{is_tool_document, Workbench, WorkbenchCommand}};

#[test]
fn privileged_tools_require_the_exact_internal_main_document() {
    assert!(is_tool_document("thor://workbench/api", true));
    for url in ["https://workbench/api", "thor://workbench.evil/api", "thor://user@workbench/api", "thor://workbench:123/api", "thor://workbench/workbench.js", "file:///api"] {
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
    let project = workbench.execute(serde_json::from_value::<WorkbenchCommand>(json!({"command":"project_create", "name":"Local API"})).unwrap()).await.unwrap();
    let request = json!({"method":"POST", "url":"http://localhost:9000/api", "body":{"type":"json", "value":{"name":"Thor"}}});
    let saved = workbench.execute(serde_json::from_value(json!({"command":"request_save", "project":project["id"], "title":"Create item", "request":request})).unwrap()).await.unwrap();
    drop(store);
    let reopened = ProjectStore::open(&path).unwrap();
    let workbench = Workbench::new(&service, &reopened);
    let opened = workbench.execute(serde_json::from_value(json!({"command":"request_open", "id":saved["id"]})).unwrap()).await.unwrap();
    assert_eq!(opened["title"], "Create item");
    assert_eq!(opened["request"]["body"]["value"]["name"], "Thor");
    reopened.delete_project(&serde_json::from_value(project["id"].clone()).unwrap()).unwrap();
    drop(reopened);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn saving_a_request_refuses_plaintext_credentials() {
    let path = std::env::temp_dir().join(format!("thor-workbench-secrets-{}.sqlite3", std::process::id()));
    let store = ProjectStore::open(&path).unwrap();
    let project = store.create_project("Secrets").unwrap();
    let service = HttpService::new().unwrap();
    let workbench = Workbench::new(&service, &store);
    let command = json!({"command":"request_save", "project":project.id, "title":"Unsafe", "request":{"method":"GET", "url":"https://example.test/", "headers":[{"name":"Authorization", "value":"Bearer secret-value"}]}});
    assert!(workbench.execute(serde_json::from_value(command).unwrap()).await.is_err());
    assert!(store.documents(&project.id, None).unwrap().is_empty());
    store.delete_project(&project.id).unwrap();
    drop(store);
    let _ = std::fs::remove_file(path);
}
