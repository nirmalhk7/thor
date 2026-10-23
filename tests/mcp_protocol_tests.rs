use serde_json::json;
use thor::browser::session::TabRef;
use thor::engine::engine_coordinator::{BrowserSnapshot, EngineCommand, EngineError, EngineHandle};
use thor::engine::tab_session::TabId;
use thor::mcp::tools::handle_message;
use tokio::sync::{mpsc, watch};

#[tokio::test]
async fn unavailable_engine_is_a_tool_failure() {
    let response = handle_message(None, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"thor_navigate","arguments":{"url":"https://app.test/"}}})).await.unwrap();
    assert_eq!(response["result"]["isError"], true);
}

#[tokio::test]
async fn malformed_json_rpc_is_rejected_before_dispatch() {
    let response = handle_message(None, json!({"id":1,"method":"initialize"}))
        .await
        .unwrap();
    assert_eq!(response["error"]["code"], -32600);
}

#[tokio::test]
async fn notifications_do_not_emit_json_rpc_responses() {
    let response = handle_message(None, json!({"jsonrpc":"2.0","method":"tools/list"})).await;
    assert!(response.is_none());
}

#[tokio::test]
async fn navigation_uses_the_selected_tab_and_propagates_engine_failure() {
    let (sender, mut commands) = mpsc::channel(1);
    let (_, snapshot_rx) = watch::channel(BrowserSnapshot {
        active_target: Some(TabRef {
            id: 42,
            generation: 7,
        }),
        tabs: vec![TabId(42)],
        ..Default::default()
    });
    let engine = EngineHandle {
        cmd_tx: sender,
        snapshot_rx,
    };
    let responder = tokio::spawn(async move {
        let Some(EngineCommand::Targeted { target, command }) = commands.recv().await else {
            panic!("navigation target was not guarded")
        };
        assert_eq!(
            target,
            TabRef {
                id: 42,
                generation: 7
            }
        );
        let EngineCommand::Navigate { tab_id, reply, .. } = *command else {
            panic!("navigation was not dispatched")
        };
        assert_eq!(tab_id, TabId(42));
        reply.send(Err(EngineError::CdpRejected)).unwrap();
    });
    let response = handle_message(Some(&engine), json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"thor_navigate","arguments":{"url":"https://app.test/"}}})).await.unwrap();
    responder.await.unwrap();
    assert_eq!(response["result"]["isError"], true);
}

#[tokio::test]
async fn evaluation_carries_navigation_generation_to_the_native_host() {
    let (sender, mut commands) = mpsc::channel(1);
    let (_, snapshot_rx) = watch::channel(BrowserSnapshot {
        active_target: Some(TabRef {
            id: 42,
            generation: 7,
        }),
        tabs: vec![TabId(42)],
        ..Default::default()
    });
    let engine = EngineHandle {
        cmd_tx: sender,
        snapshot_rx,
    };
    let responder = tokio::spawn(async move {
        let Some(EngineCommand::Targeted { target, command }) = commands.recv().await else {
            panic!("evaluation target was not guarded")
        };
        assert_eq!(
            target,
            TabRef {
                id: 42,
                generation: 7
            }
        );
        let EngineCommand::Evaluate {
            tab_id,
            generation,
            reply,
            ..
        } = *command
        else {
            panic!("evaluation was not dispatched")
        };
        assert_eq!(tab_id, TabId(42));
        assert_eq!(generation, 7);
        reply.send(Ok("2".into())).unwrap();
    });
    let response = handle_message(Some(&engine), json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"thor_eval_js","arguments":{"script":"1 + 1"}}})).await.unwrap();
    responder.await.unwrap();
    assert_eq!(response["result"]["isError"], false);
}
