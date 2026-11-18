use super::{
    cdp_result, mpsc, oneshot, tab_worker, Arc, BrowserEngine, EngineCommand, EngineError,
    TabCommand, TabId, TabRecord, DEADLINE_LAUNCH_SHUTDOWN,
};

impl BrowserEngine {
    pub(super) async fn handle_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::Targeted { command, .. } => {
                command.reject(EngineError::UnsupportedOperation)
            }
            EngineCommand::CreateTab {
                identity_id: _,
                url,
                reply,
            } => self.create_tab(url, reply).await,
            EngineCommand::CloseTab { tab_id, reply } => self.close_tab(tab_id, reply).await,
            EngineCommand::ActivateTab { reply, .. }
            | EngineCommand::Back { reply, .. }
            | EngineCommand::Forward { reply, .. }
            | EngineCommand::Reload { reply, .. }
            | EngineCommand::Freeze { reply, .. }
            | EngineCommand::Discard { reply, .. } => {
                let _ = reply.send(Ok(()));
            }
            EngineCommand::Navigate { tab_id, url, reply } => {
                self.forward_tab(tab_id, TabCommand::Navigate { url, reply })
                    .await;
            }
            EngineCommand::Evaluate {
                tab_id,
                script,
                reply,
                ..
            } => {
                self.forward_tab(tab_id, TabCommand::Evaluate { script, reply })
                    .await;
            }
            EngineCommand::CaptureScreenshot {
                tab_id,
                options,
                reply,
            } => {
                self.forward_tab(tab_id, TabCommand::CaptureScreenshot { options, reply })
                    .await;
            }
            EngineCommand::MergeTabs {
                source,
                destination,
                reply,
            } => self.merge_tabs(source, destination, reply).await,
            EngineCommand::Shutdown { reply } => {
                let runtime = Arc::clone(&self.runtime);
                let result = cdp_result(
                    DEADLINE_LAUNCH_SHUTDOWN,
                    runtime.shutdown(),
                    EngineError::CdpRejected,
                )
                .await;
                let _ = reply.send(result);
            }
        }
    }

    async fn create_tab(
        &mut self,
        url: String,
        reply: oneshot::Sender<Result<TabId, EngineError>>,
    ) {
        let runtime = Arc::clone(&self.runtime);
        let result = cdp_result(
            DEADLINE_LAUNCH_SHUTDOWN,
            runtime.new_tab(Some(&url)),
            EngineError::RuntimeUnavailable,
        )
        .await
        .map(|session| {
            let tab_id = session.id();
            let (command_tx, commands) = mpsc::channel(64);
            tokio::spawn(tab_worker::run(session, commands));
            self.tabs.insert(
                tab_id,
                TabRecord {
                    command_tx,
                    last_user_interaction: Some(std::time::Instant::now()),
                    generation: 1,
                    restore: None,
                },
            );
            tab_id
        });
        let _ = reply.send(result);
    }

    async fn close_tab(&mut self, tab_id: TabId, reply: oneshot::Sender<Result<(), EngineError>>) {
        if let Some(record) = self.tabs.remove(&tab_id) {
            let _ = record.command_tx.send(TabCommand::Close { reply }).await;
        } else {
            let _ = reply.send(Err(EngineError::TabNotFound));
        }
    }

    async fn forward_tab(&self, tab_id: TabId, command: TabCommand) {
        if let Some(record) = self.tabs.get(&tab_id) {
            let _ = record.command_tx.send(command).await;
        } else {
            command.reject_missing();
        }
    }

    async fn merge_tabs(
        &mut self,
        source: TabId,
        destination: TabId,
        reply: oneshot::Sender<Result<(), EngineError>>,
    ) {
        let source_tx = self.tabs.get(&source).map(|r| r.command_tx.clone());
        let dest_tx = self.tabs.get(&destination).map(|r| r.command_tx.clone());
        let result = async {
            let s_tx = source_tx.as_ref().ok_or(EngineError::TabNotFound)?;
            let d_tx = dest_tx.as_ref().ok_or(EngineError::TabNotFound)?;
            let (hist_tx, hist_rx) = oneshot::channel();
            s_tx.send(TabCommand::GetHistory { reply: hist_tx })
                .await
                .map_err(|_| EngineError::TabNotFound)?;
            let history = hist_rx.await.map_err(|_| EngineError::TabNotFound)?;
            let (merge_tx, merge_rx) = oneshot::channel();
            d_tx.send(TabCommand::MergeHistory {
                source_history: history,
                reply: merge_tx,
            })
            .await
            .map_err(|_| EngineError::TabNotFound)?;
            let _ = merge_rx.await;
            let (close_tx, close_rx) = oneshot::channel();
            let _ = s_tx.send(TabCommand::Close { reply: close_tx }).await;
            let _ = close_rx.await;
            self.tabs.remove(&source);
            Ok(())
        }
        .await;
        let _ = reply.send(result);
    }
}
