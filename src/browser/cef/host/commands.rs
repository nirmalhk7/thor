use std::sync::Arc;

use ::cef::*;
use parking_lot::Mutex;
use serde_json::json;
use tokio::sync::{mpsc, oneshot, watch};

use crate::engine::engine_coordinator::{
    BrowserSnapshot, EngineCommand, EngineError, EngineHandle, IdentityContextId, ScreenshotOptions,
};
use crate::engine::tab_session::TabId;

use super::{activate, browser, close_window, devtools, new_tab, with_host};

pub(super) fn start() {
    let (sender, mut receiver) = mpsc::channel(256);
    let (snapshots, snapshot_rx) = watch::channel(BrowserSnapshot::default());
    with_host(|host| host.snapshots = Some(snapshots));
    let handle = EngineHandle {
        cmd_tx: sender,
        snapshot_rx,
    };
    crate::AppState::global().set_engine_handle(handle.clone());
    tokio::spawn(async move {
        while let Some(command) = receiver.recv().await {
            let mut task = CommandTask::new(Arc::new(Mutex::new(Some(command))));
            if post_task(ThreadId::UI, Some(&mut task)) != 1 {
                crate::thor_warn!("thor::cef", "CEF rejected a browser command");
            }
        }
    });
    tokio::spawn(async move {
        if let Err(error) = crate::mcp::server::start_mcp_server(handle).await {
            crate::thor_warn!("thor::mcp", "MCP unavailable: {error}");
        }
    });
}

pub(super) fn publish() {
    with_host(|host| {
        let Some(sender) = &host.snapshots else {
            return;
        };
        let snapshot = BrowserSnapshot {
            tab_details: host.session.summaries(),
            active_target: host
                .session
                .active_tab()
                .and_then(|id| host.session.target(id).ok()),
            active_tabs: host.views.len(),
            runtime_status: "CEF running".into(),
            tabs: host.session.tab_ids().into_iter().map(TabId).collect(),
            identities: host
                .session
                .identities()
                .into_iter()
                .map(|identity| IdentityContextId(identity.id))
                .collect(),
            ..Default::default()
        };
        sender.send_replace(snapshot);
    });
}

fn control(
    id: TabId,
    action: impl FnOnce(&Browser) -> Result<(), EngineError>,
) -> Result<(), EngineError> {
    let browser = browser(id.0).ok_or(EngineError::TabNotFound)?;
    action(&browser)
}

fn handle(command: EngineCommand) {
    match command {
        EngineCommand::Targeted { target, command } => {
            let valid = with_host(|host| host.session.validate_target(target).is_ok()).unwrap_or(false);
            if valid { handle(*command); } else { command.reject(EngineError::StaleTarget); }
            return;
        }
        EngineCommand::CreateTab {
            identity_id,
            url,
            reply,
        } => {
            let result = new_tab(
                identity_id.0,
                if url.is_empty() { "about:blank" } else { &url },
            )
            .map(TabId)
            .map_err(|_| EngineError::CdpRejected);
            let _ = reply.send(result);
        }
        EngineCommand::ActivateTab { tab_id, reply } => {
            let result = control(tab_id, |_| {
                activate(tab_id.0);
                Ok(())
            });
            let _ = reply.send(result);
        }
        EngineCommand::CloseTab { tab_id, reply } => {
            let result = control(tab_id, |browser| {
                browser
                    .host()
                    .ok_or(EngineError::ContextDisposed)?
                    .close_browser(0);
                Ok(())
            });
            let _ = reply.send(result);
        }
        EngineCommand::Navigate { tab_id, url, reply } => {
            devtools::send(
                tab_id.0,
                "Page.navigate",
                json!({ "url": url }),
                devtools::Reply::Unit(reply),
            );
        }
        EngineCommand::Back { tab_id, reply } => {
            let result = control(tab_id, |browser| {
                if browser.can_go_back() != 1 {
                    return Err(EngineError::CdpRejected);
                }
                browser.go_back();
                Ok(())
            });
            let _ = reply.send(result);
        }
        EngineCommand::Forward { tab_id, reply } => {
            let result = control(tab_id, |browser| {
                if browser.can_go_forward() != 1 {
                    return Err(EngineError::CdpRejected);
                }
                browser.go_forward();
                Ok(())
            });
            let _ = reply.send(result);
        }
        EngineCommand::Reload { tab_id, reply } => {
            devtools::send(
                tab_id.0,
                "Page.reload",
                json!({}),
                devtools::Reply::Unit(reply),
            );
        }
        EngineCommand::Evaluate {
            tab_id,
            generation,
            script,
            reply,
        } => {
            evaluate(tab_id, generation, script, reply);
        }
        EngineCommand::CaptureScreenshot {
            tab_id,
            options,
            reply,
        } => {
            screenshot(tab_id, options, reply);
        }
        EngineCommand::Freeze { tab_id, reply } => {
            let foreground =
                with_host(|host| host.session.active_tab() == Some(tab_id.0)).unwrap_or(false);
            if foreground {
                let _ = reply.send(Err(EngineError::Busy));
                return;
            }
            devtools::send(
                tab_id.0,
                "Page.setWebLifecycleState",
                json!({ "state": "frozen" }),
                devtools::Reply::Unit(reply),
            );
        }
        EngineCommand::Discard { reply, .. } | EngineCommand::MergeTabs { reply, .. } => {
            let _ = reply.send(Err(EngineError::UnsupportedOperation));
        }
        EngineCommand::Shutdown { reply } => {
            close_window();
            let _ = reply.send(Ok(()));
        }
    }
    publish();
}

fn evaluate(
    tab: TabId,
    generation: u64,
    script: String,
    reply: oneshot::Sender<Result<String, EngineError>>,
) {
    let valid = with_host(|host| {
        host.session
            .target(tab.0)
            .is_ok_and(|target| generation != 0 && target.generation == generation)
    })
    .unwrap_or(false);
    if !valid {
        let _ = reply.send(Err(EngineError::StaleTarget));
        return;
    }
    devtools::send(
        tab.0,
        "Runtime.evaluate",
        json!({ "expression": script, "returnByValue": true, "awaitPromise": true }),
        devtools::Reply::Evaluate(reply),
    );
}

fn screenshot(
    tab: TabId,
    options: ScreenshotOptions,
    reply: oneshot::Sender<Result<Vec<u8>, EngineError>>,
) {
    if !matches!(options.format.as_str(), "png" | "jpeg") || options.quality > 100 {
        let _ = reply.send(Err(EngineError::UnsupportedOperation));
        return;
    }
    let mut params = json!({ "format": options.format, "fromSurface": true });
    if options.format == "jpeg" {
        params["quality"] = json!(options.quality);
    }
    devtools::send(
        tab.0,
        "Page.captureScreenshot",
        params,
        devtools::Reply::Screenshot(reply),
    );
}

wrap_task! {
    struct CommandTask { command: Arc<Mutex<Option<EngineCommand>>> }
    impl Task {
        fn execute(&self) {
            let command = self.command.lock().take();
            if let Some(command) = command { handle(command); }
        }
    }
}
