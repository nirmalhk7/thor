use std::sync::Arc;

use axum::{body::Body, http::{Request, StatusCode}, routing::any, Router};
use thor::api::{Authentication, HttpService, RequestSpec, SecretVault, Variables};

#[tokio::test]
async fn bearer_credentials_resolve_at_execution_and_never_serialize_into_request_documents() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = Router::new().route("/", any(|request: Request<Body>| async move {
            if request.headers().get("authorization").is_some_and(|value| value == "Bearer private-test-token") {
                StatusCode::NO_CONTENT
            } else { StatusCode::UNAUTHORIZED }
        }));
        axum::serve(listener, app).await.unwrap();
    });
    let vault = Arc::new(SecretVault::default());
    let token = vault.store_temporary("private-test-token").unwrap();
    let mut request = RequestSpec::new("GET", url);
    request.auth = Authentication::Bearer { token: token.clone() };
    let document = serde_json::to_string(&request).unwrap();
    assert!(!document.contains("private-test-token"));
    let request: RequestSpec = serde_json::from_str(&document).unwrap();
    let service = HttpService::with_secrets(vault.clone()).unwrap();
    assert_eq!(service.send(&request, &Variables::default()).await.unwrap().status, 204);
    vault.delete(&token).unwrap();
    assert!(service.send(&request, &Variables::default()).await.is_err());
    server.abort();
}

#[tokio::test]
async fn basic_and_query_credentials_apply_without_rewriting_saved_urls() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let app = Router::new().route("/", any(|request: Request<Body>| async move {
            let basic = request.headers().get("authorization").is_some_and(|value| value == "Basic YWxpY2U6c2VjcmV0");
            let query = request.uri().query() == Some("key=space+%26+slash%2F");
            if basic || query { StatusCode::NO_CONTENT } else { StatusCode::UNAUTHORIZED }
        }));
        axum::serve(listener, app).await.unwrap();
    });
    let vault = Arc::new(SecretVault::default());
    let password = vault.store_temporary("secret").unwrap();
    let key = vault.store_temporary("space & slash/").unwrap();
    let service = HttpService::with_secrets(vault).unwrap();
    let mut request = RequestSpec::new("GET", &url);
    request.auth = Authentication::Basic { username: "alice".into(), password };
    assert_eq!(service.send(&request, &Variables::default()).await.unwrap().status, 204);
    request.auth = Authentication::ApiKeyQuery { name: "key".into(), secret: key };
    assert_eq!(service.send(&request, &Variables::default()).await.unwrap().status, 204);
    assert_eq!(request.url, url);
    server.abort();
}

#[test]
fn secret_references_reject_foreign_or_malformed_accounts() {
    for value in ["../../account", "keychain:other-service", "temporary:short"] {
        assert!(serde_json::from_value::<thor::api::SecretRef>(serde_json::json!(value)).is_err());
    }
}
