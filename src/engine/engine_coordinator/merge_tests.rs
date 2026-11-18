use super::test_peer::{add_tab, engine};
use super::*;
use crate::engine::tab_session::HistoryEntry;

fn history() -> Vec<HistoryEntry> {
    vec![HistoryEntry {
        sequence: 3,
        visited_at: std::time::UNIX_EPOCH,
        url: "https://example.test/source".into(),
        normalized_url: "https://example.test/source".into(),
        title: "source title".into(),
        transition: "link".into(),
    }]
}

#[tokio::test]
async fn merge_closes_source_after_destination_drops_ack_and_ignores_close_error() {
    let (mut engine, _, _peer) = engine(true).await;
    let mut source = add_tab(&mut engine, 1);
    let mut destination = add_tab(&mut engine, 2);
    let (reply, receiver) = oneshot::channel();
    let source_worker = async {
        let Some(TabCommand::GetHistory { reply }) = source.recv().await else {
            panic!("history not requested")
        };
        reply.send(history()).unwrap();
        let Some(TabCommand::Close { reply }) = source.recv().await else {
            panic!("source not closed")
        };
        let _ = reply.send(Err(EngineError::CdpRejected));
    };
    let destination_worker = async {
        let Some(TabCommand::MergeHistory {
            source_history,
            reply,
        }) = destination.recv().await
        else {
            panic!("history not merged")
        };
        assert_eq!(source_history[0].url, "https://example.test/source");
        assert_eq!(source_history[0].title, "source title");
        assert_eq!(source_history[0].sequence, 3);
        drop(reply);
    };
    timeout(Duration::from_secs(2), async {
        tokio::join!(
            engine.handle_command(EngineCommand::MergeTabs {
                source: TabId(1),
                destination: TabId(2),
                reply
            }),
            source_worker,
            destination_worker
        );
    })
    .await
    .unwrap();
    assert!(receiver.await.unwrap().is_ok());
    assert!(!engine.tabs.contains_key(&TabId(1)));
    assert!(engine.tabs.contains_key(&TabId(2)));
}

#[tokio::test]
async fn failed_history_reply_preserves_both_tabs_and_skips_merge_and_close() {
    let (mut engine, _, _peer) = engine(true).await;
    let mut source = add_tab(&mut engine, 1);
    let mut destination = add_tab(&mut engine, 2);
    let (reply, receiver) = oneshot::channel();
    let worker = async {
        let Some(TabCommand::GetHistory { reply }) = source.recv().await else {
            panic!("history not requested")
        };
        drop(reply);
    };
    tokio::join!(
        engine.handle_command(EngineCommand::MergeTabs {
            source: TabId(1),
            destination: TabId(2),
            reply
        }),
        worker
    );
    assert!(matches!(
        receiver.await.unwrap(),
        Err(EngineError::TabNotFound)
    ));
    assert_eq!(engine.tabs.len(), 2);
    assert!(matches!(
        source.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    assert!(matches!(
        destination.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn same_source_and_destination_still_merges_then_closes_and_removes_tab() {
    let (mut engine, _, _peer) = engine(true).await;
    let mut tab = add_tab(&mut engine, 1);
    let (reply, receiver) = oneshot::channel();
    let worker = async {
        let Some(TabCommand::GetHistory { reply }) = tab.recv().await else {
            panic!("history not requested")
        };
        reply.send(history()).unwrap();
        let Some(TabCommand::MergeHistory {
            source_history,
            reply,
        }) = tab.recv().await
        else {
            panic!("history not merged")
        };
        assert_eq!(source_history.len(), 1);
        let _ = reply.send(());
        let Some(TabCommand::Close { reply }) = tab.recv().await else {
            panic!("tab not closed")
        };
        let _ = reply.send(Ok(()));
    };
    timeout(Duration::from_secs(2), async {
        tokio::join!(
            engine.handle_command(EngineCommand::MergeTabs {
                source: TabId(1),
                destination: TabId(1),
                reply
            }),
            worker
        );
    })
    .await
    .unwrap();
    assert!(receiver.await.unwrap().is_ok());
    assert!(engine.tabs.is_empty());
}
