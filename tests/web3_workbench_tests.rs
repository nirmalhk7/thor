use serde_json::{json, Value};
use thor::{
    api::HttpService,
    projects::ProjectStore,
    workbench::{asset, command_allowed, Workbench, WorkbenchCommand, WEB3_URL},
};

const FIXTURE: &str = include_str!("fixtures/web3_balance_call.json");

async fn inspect(engine: &Workbench<'_>, request: Value) -> Result<Value, String> {
    engine
        .execute(WorkbenchCommand::Web3InspectCall { request })
        .await
}

#[tokio::test]
async fn web3_workbench_decodes_the_account_without_claiming_a_balance() {
    let path = std::env::temp_dir().join(format!(
        "thor-web3-workbench-{}.sqlite3",
        std::process::id()
    ));
    let store = ProjectStore::open(&path).unwrap();
    let service = HttpService::new().unwrap();
    let engine = Workbench::new(&service, &store);
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let command: WorkbenchCommand =
        serde_json::from_value(json!({"command": "web3_inspect_call", "request": fixture}))
            .unwrap();

    assert_eq!(asset(WEB3_URL).unwrap().mime, "text/html");
    assert_eq!(
        asset("thor://workbench/web3.js").unwrap().mime,
        "text/javascript"
    );
    assert!(command_allowed(WEB3_URL, true, &command));
    for url in [
        "https://workbench/web3",
        "thor://workbench/web3.js",
        "thor://workbench/tools",
        "thor://workbench/api",
        "thor://workbench/security",
    ] {
        assert!(!command_allowed(url, true, &command), "{url}");
    }
    assert!(!command_allowed(WEB3_URL, false, &command));
    assert!(!command_allowed(
        WEB3_URL,
        true,
        &WorkbenchCommand::ProjectList
    ));

    let report = engine.execute(command).await.unwrap();
    assert_eq!(
        report,
        json!({
            "state": "decoded",
            "rpc_method": "eth_call",
            "contract_address": "0x1111111111111111111111111111111111111111",
            "selector": "0x70a08231",
            "signature": "balanceOf(address)",
            "account_address": "0x2222222222222222222222222222222222222222"
        })
    );

    let mut compat = fixture.clone();
    let call = compat["params"][0].as_object_mut().unwrap();
    let input = call.remove("input").unwrap();
    call.insert("data".into(), input.clone());
    assert_eq!(inspect(&engine, compat.clone()).await.unwrap(), report);
    compat["params"][0]["input"] = input;
    assert_eq!(inspect(&engine, compat.clone()).await.unwrap(), report);
    compat["params"][0]["input"] = json!("0xdeadbeef");
    assert!(inspect(&engine, compat)
        .await
        .unwrap_err()
        .contains("disagree"));

    let mut other = fixture.clone();
    other["params"][0]["input"] = json!("0xDEADBEEF");
    let limited = inspect(&engine, other).await.unwrap();
    assert_eq!(limited["state"], "selector_only");
    assert_eq!(limited["selector"], "0xdeadbeef");
    assert!(limited.get("signature").is_none());
    assert!(limited.get("account_address").is_none());

    for (changed, expected) in [
        (("method", json!("eth_sign")), "JSON-RPC 2.0 eth_call"),
        (("id", json!("x".repeat(64 * 1024))), "64 KiB"),
    ] {
        let mut malformed = fixture.clone();
        malformed[changed.0] = changed.1;
        assert!(inspect(&engine, malformed)
            .await
            .unwrap_err()
            .contains(expected));
    }

    let mut malformed = fixture.clone();
    for (calldata, expected) in [
        ("0x70a082310", "four-byte selector"),
        ("0x70a08231", "one 32-byte argument"),
        (
            "0x70a082310100000000000000000000002222222222222222222222222222222222222222",
            "zero address padding",
        ),
        (
            "0x70a08231000000000000000000000000222222222222222222222222222222222222222200",
            "one 32-byte argument",
        ),
    ] {
        malformed["params"][0]["input"] = json!(calldata);
        assert!(inspect(&engine, malformed.clone())
            .await
            .unwrap_err()
            .contains(expected));
    }
    malformed["params"][0]["to"] = json!("example.eth");
    assert!(inspect(&engine, malformed)
        .await
        .unwrap_err()
        .contains("20-byte hex contract"));
    assert!(store.projects().unwrap().is_empty());
    drop(store);
    let _ = std::fs::remove_file(path);
}
