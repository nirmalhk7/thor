use serde_json::json;
use thor::projects::{DocumentKind, ProjectStore, StoreError};

fn database() -> std::path::PathBuf {
    let mut nonce = [0; 12];
    getrandom::fill(&mut nonce).unwrap();
    let suffix: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    std::env::temp_dir()
        .join(format!("thor-project-test-{suffix}"))
        .join("projects.sqlite3")
}

#[test]
fn authored_requests_survive_reopening_the_project_database() {
    let path = database();
    let store = ProjectStore::open(&path).unwrap();
    let project = store.create_project("Service").unwrap();
    let request = store
        .create_document(
            &project.id,
            DocumentKind::Request,
            "List users",
            json!({"method":"GET","url":"https://app.test/users"}),
        )
        .unwrap();
    drop(store);
    let store = ProjectStore::open(&path).unwrap();
    let loaded = store.document(&request.id).unwrap();
    assert_eq!(loaded.payload["url"], "https://app.test/users");
    assert_eq!(store.projects().unwrap()[0].name, "Service");
    drop(store);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn optimistic_revisions_prevent_lost_edits() {
    let path = database();
    let store = ProjectStore::open(&path).unwrap();
    let project = store.create_project("Service").unwrap();
    let document = store
        .create_document(
            &project.id,
            DocumentKind::Environment,
            "Local",
            json!({"baseUrl":"http://localhost:3000"}),
        )
        .unwrap();
    let updated = store
        .update_document(
            &document.id,
            document.revision,
            "Local",
            json!({"baseUrl":"http://localhost:4000"}),
        )
        .unwrap();
    assert_eq!(updated.revision, 2);
    assert!(matches!(
        store.update_document(&document.id, document.revision, "Old edit", json!({})),
        Err(StoreError::Conflict)
    ));
    assert_eq!(
        store.document(&document.id).unwrap().payload["baseUrl"],
        "http://localhost:4000"
    );
    drop(store);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn project_deletion_removes_owned_documents_without_touching_other_projects() {
    let path = database();
    let store = ProjectStore::open(&path).unwrap();
    let first = store.create_project("First").unwrap();
    let second = store.create_project("Second").unwrap();
    let removed = store
        .create_document(&first.id, DocumentKind::Finding, "Finding", json!({}))
        .unwrap();
    let retained = store
        .create_document(&second.id, DocumentKind::Scenario, "Scenario", json!({}))
        .unwrap();
    store.delete_project(&first.id).unwrap();
    assert!(matches!(
        store.document(&removed.id),
        Err(StoreError::NotFound)
    ));
    assert_eq!(store.document(&retained.id).unwrap().project_id, second.id);
    drop(store);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn future_database_versions_are_rejected_without_mutating_them() {
    let path = database();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 999).unwrap();
    drop(connection);
    assert!(matches!(
        ProjectStore::open(&path),
        Err(StoreError::UnsupportedVersion(999))
    ));
    let connection = rusqlite::Connection::open(&path).unwrap();
    let mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "delete");
    drop(connection);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
