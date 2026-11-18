use super::test_peer::{add_tab, engine};
use super::*;

async fn request_tab(reject_create: bool) -> Result<TabId, EngineError> {
    let (mut engine, _, _peer) = engine(reject_create).await;
    let (reply, receiver) = oneshot::channel();
    engine
        .handle_command(EngineCommand::CreateTab {
            identity_id: IdentityContextId(3),
            url: "about:blank".into(),
            reply,
        })
        .await;
    assert!(engine.tabs.is_empty());
    receiver.await.unwrap()
}

#[tokio::test]
async fn create_maps_protocol_rejection_to_runtime_unavailable_without_adding_tab() {
    assert!(matches!(
        request_tab(true).await,
        Err(EngineError::RuntimeUnavailable)
    ));
}

#[tokio::test]
async fn create_maps_launch_deadline_to_timeout_without_adding_tab() {
    assert!(matches!(
        request_tab(false).await,
        Err(EngineError::Timeout)
    ));
}

#[tokio::test]
async fn targeted_command_returns_unsupported_through_nested_reply() {
    let (mut engine, _, _peer) = engine(true).await;
    let (reply, receiver) = oneshot::channel();
    let command = EngineCommand::Navigate {
        tab_id: TabId(99),
        url: "about:blank".into(),
        reply,
    };
    let target = crate::browser::session::TabRef {
        id: 99,
        generation: 2,
    };
    engine
        .handle_command(EngineCommand::Targeted {
            target,
            command: Box::new(command),
        })
        .await;
    assert!(matches!(
        receiver.await.unwrap(),
        Err(EngineError::UnsupportedOperation)
    ));
}

#[tokio::test]
async fn navigate_routes_url_and_preserves_tab_reply() {
    let (mut engine, _, _peer) = engine(true).await;
    let mut tab = add_tab(&mut engine, 41);
    let (reply, receiver) = oneshot::channel();
    engine
        .handle_command(EngineCommand::Navigate {
            tab_id: TabId(41),
            url: "https://example.test/path".into(),
            reply,
        })
        .await;
    let Some(TabCommand::Navigate { url, reply }) = tab.recv().await else {
        panic!("navigate not forwarded")
    };
    assert_eq!(url, "https://example.test/path");
    reply.send(Err(EngineError::CdpRejected)).unwrap();
    assert!(matches!(
        receiver.await.unwrap(),
        Err(EngineError::CdpRejected)
    ));
}

#[tokio::test]
async fn evaluate_and_screenshot_preserve_payloads_and_missing_errors() {
    let (mut engine, _, _peer) = engine(true).await;
    let mut tab = add_tab(&mut engine, 41);
    let (reply, receiver) = oneshot::channel();
    engine
        .handle_command(EngineCommand::Evaluate {
            tab_id: TabId(41),
            generation: 7,
            script: "({answer:42})".into(),
            reply,
        })
        .await;
    let Some(TabCommand::Evaluate { script, reply }) = tab.recv().await else {
        panic!("evaluate not forwarded")
    };
    assert_eq!(script, "({answer:42})");
    reply.send(Ok("{\"answer\":42}".into())).unwrap();
    assert_eq!(receiver.await.unwrap().unwrap(), "{\"answer\":42}");
    let (reply, receiver) = oneshot::channel();
    engine
        .handle_command(EngineCommand::CaptureScreenshot {
            tab_id: TabId(41),
            options: ScreenshotOptions {
                format: "jpeg".into(),
                quality: 73,
            },
            reply,
        })
        .await;
    let Some(TabCommand::CaptureScreenshot { options, reply }) = tab.recv().await else {
        panic!("screenshot not forwarded")
    };
    assert_eq!((options.format.as_str(), options.quality), ("jpeg", 73));
    reply.send(Ok(vec![1, 2, 3])).unwrap();
    assert_eq!(receiver.await.unwrap().unwrap(), [1, 2, 3]);
    let (reply, receiver) = oneshot::channel();
    engine
        .handle_command(EngineCommand::CaptureScreenshot {
            tab_id: TabId(99),
            options: ScreenshotOptions {
                format: "png".into(),
                quality: 0,
            },
            reply,
        })
        .await;
    assert!(matches!(
        receiver.await.unwrap(),
        Err(EngineError::TabNotFound)
    ));
}

#[tokio::test]
async fn close_removes_tab_before_forwarding_and_closed_channel_drops_reply() {
    let (mut engine, _, _peer) = engine(true).await;
    let mut tab = add_tab(&mut engine, 41);
    let (reply, receiver) = oneshot::channel();
    engine
        .handle_command(EngineCommand::CloseTab {
            tab_id: TabId(41),
            reply,
        })
        .await;
    assert!(!engine.tabs.contains_key(&TabId(41)));
    let Some(TabCommand::Close { reply }) = tab.recv().await else {
        panic!("close not forwarded")
    };
    reply.send(Ok(())).unwrap();
    assert!(receiver.await.unwrap().is_ok());
    drop(add_tab(&mut engine, 42));
    let (reply, receiver) = oneshot::channel();
    engine
        .handle_command(EngineCommand::Navigate {
            tab_id: TabId(42),
            url: "about:blank".into(),
            reply,
        })
        .await;
    assert!(receiver.await.is_err());
    assert!(engine.tabs.contains_key(&TabId(42)));
}
