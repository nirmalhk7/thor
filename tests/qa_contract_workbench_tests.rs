use serde_json::{json, Value};
use thor::{
    api::HttpService,
    projects::ProjectStore,
    workbench::{command_allowed, Workbench, WorkbenchCommand},
};

async fn invoke(engine: &Workbench<'_>, payload: Value) -> Result<Value, String> {
    let command = WorkbenchCommand::FeatureInvoke {
        slug: "contract-testing".into(),
        payload,
    };
    assert!(command_allowed(
        "thor://workbench/tools#contract-testing",
        true,
        &command
    ));
    engine.execute(command).await
}

#[tokio::test]
async fn launch_workflow_identifies_correct_widgets_and_rejects_contract_regressions() {
    let path =
        std::env::temp_dir().join(format!("thor-qa-workbench-{}.sqlite3", std::process::id()));
    let store = ProjectStore::open(&path).unwrap();
    let http = HttpService::new().unwrap();
    let engine = Workbench::new(&http, &store);
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/qa_response_contract.json")).unwrap();

    let passed = invoke(&engine, fixture.clone()).await.unwrap();
    assert_eq!(
        passed["result"],
        json!({"satisfied": true, "failures": [], "verified": 1})
    );

    let mut drifted = fixture.clone();
    drifted["actuals"][0]["status"] = json!(201);
    drifted["actuals"][0]["body"]["available"] = json!("yes");
    let report = invoke(&engine, drifted).await.unwrap();
    assert_eq!(
        report["result"],
        json!({
            "satisfied": false,
            "failures": [
                "GET /widgets/42: status 201 != expected 200",
                "GET /widgets/42: $.available: expected boolean, got string"
            ],
            "verified": 1
        })
    );

    let mut missing_or_null = fixture.clone();
    missing_or_null["actuals"][0]
        .as_object_mut()
        .unwrap()
        .remove("body");
    let missing = invoke(&engine, missing_or_null.clone()).await.unwrap();
    assert_eq!(missing["result"]["satisfied"], false);
    assert_eq!(
        missing["result"]["failures"][0],
        "GET /widgets/42: observed response body is missing; expected JSON to validate"
    );

    missing_or_null["interactions"][0]["response_schema"] = json!({"type":"null"});
    let still_missing = invoke(&engine, missing_or_null.clone()).await.unwrap();
    assert_eq!(still_missing["result"]["satisfied"], false);
    missing_or_null["actuals"][0]["body"] = Value::Null;
    let null_body = invoke(&engine, missing_or_null).await.unwrap();
    assert_eq!(null_body["result"]["satisfied"], true);

    let empty = invoke(&engine, json!({"interactions": [], "actuals": []})).await;
    assert!(empty
        .unwrap_err()
        .contains("at least one expected interaction"));
    drop(store);
    let _ = std::fs::remove_file(path);
}
