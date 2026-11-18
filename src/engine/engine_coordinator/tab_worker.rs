use super::{
    cdp_result, mpsc, Duration, EngineError, TabCommand, DEADLINE_LAUNCH_SHUTDOWN, DEADLINE_NAV,
    DEADLINE_SCREENSHOT, DEADLINE_STATE,
};
use crate::engine::tab_session::{TabLifecycle, TabSession};

pub(super) async fn run(session: TabSession, mut commands: mpsc::Receiver<TabCommand>) {
    let mut session = Some(session);
    while let Some(command) = commands.recv().await {
        if handle_command(&mut session, command).await {
            break;
        }
    }
}

async fn handle_command(session: &mut Option<TabSession>, command: TabCommand) -> bool {
    let Some(active) = session.as_ref() else {
        command.reject_missing();
        return false;
    };
    match command {
        TabCommand::Navigate { url, reply } => {
            let result = cdp_result(
                DEADLINE_NAV,
                active.navigate(&url),
                EngineError::CdpRejected,
            )
            .await
            .map(|_| ());
            let _ = reply.send(result);
        }
        TabCommand::Close { reply } => {
            // The session was checked above; this worker exclusively owns the option.
            let active = session.take().expect("active session checked above");
            let result = cdp_result(
                DEADLINE_LAUNCH_SHUTDOWN,
                active.close(),
                EngineError::CdpRejected,
            )
            .await;
            let _ = reply.send(result);
            return true;
        }
        TabCommand::Evaluate { script, reply } => {
            let result = cdp_result(
                DEADLINE_STATE,
                active.evaluate_js(&script),
                EngineError::CdpRejected,
            )
            .await
            .map(|value| value.to_string());
            let _ = reply.send(result);
        }
        TabCommand::CaptureScreenshot { options: _, reply } => {
            let result = cdp_result(
                DEADLINE_SCREENSHOT,
                active.capture_screenshot(),
                EngineError::CdpRejected,
            )
            .await
            .and_then(|bytes| {
                if bytes.is_empty() {
                    Err(EngineError::CdpRejected)
                } else {
                    Ok(bytes)
                }
            });
            let _ = reply.send(result);
        }
        TabCommand::GetHistory { reply } => {
            let _ = reply.send(active.nav_history());
        }
        TabCommand::MergeHistory {
            source_history,
            reply,
        } => {
            active.append_history(source_history);
            let _ = reply.send(());
        }
        TabCommand::TickLifecycle { reply } => {
            tick_lifecycle(active);
            let _ = reply.send(());
        }
    }
    false
}

fn tick_lifecycle(session: &TabSession) {
    let idle = session.last_active().elapsed();
    let current = session.lifecycle();
    if idle > Duration::from_secs(24 * 3600) && current != TabLifecycle::Expired {
        // Expired after 24h (preserve the existing coordinator's no-operation branch).
    } else if idle > Duration::from_secs(3600)
        && current != TabLifecycle::Discarded
        && current != TabLifecycle::Expired
    {
        let _ = session.discard_blocking();
    } else if idle > Duration::from_secs(15 * 60) && current == TabLifecycle::Active {
        let _ = session.freeze_blocking(false);
    }
}

impl TabCommand {
    pub(super) fn reject_missing(self) {
        match self {
            Self::Navigate { reply, .. } | Self::Close { reply } => {
                let _ = reply.send(Err(EngineError::TabNotFound));
            }
            Self::Evaluate { reply, .. } => {
                let _ = reply.send(Err(EngineError::TabNotFound));
            }
            Self::CaptureScreenshot { reply, .. } => {
                let _ = reply.send(Err(EngineError::TabNotFound));
            }
            Self::GetHistory { reply } => {
                let _ = reply.send(Vec::new());
            }
            Self::MergeHistory { reply, .. } | Self::TickLifecycle { reply } => {
                let _ = reply.send(());
            }
        }
    }
}
