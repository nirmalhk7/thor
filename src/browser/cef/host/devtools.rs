use std::collections::HashMap;
use std::time::Duration;

use ::cef::*;
use base64::Engine;
use serde_json::Value;
use tokio::sync::oneshot;

use crate::browser::session::TabRef;
use crate::engine::engine_coordinator::EngineError;

use super::{browser, with_host};

pub(super) enum Reply {
    Evaluate(oneshot::Sender<Result<String, EngineError>>),
    Screenshot(oneshot::Sender<Result<Vec<u8>, EngineError>>),
    Unit(oneshot::Sender<Result<(), EngineError>>),
}

impl Reply {
    fn fail(self, error: EngineError) {
        match self {
            Self::Evaluate(sender) => {
                let _ = sender.send(Err(error));
            }
            Self::Screenshot(sender) => {
                let _ = sender.send(Err(error));
            }
            Self::Unit(sender) => {
                let _ = sender.send(Err(error));
            }
        }
    }

    fn complete(self, result: Value) {
        match self {
            Self::Evaluate(sender) => {
                let value = result.get("result").and_then(|value| value.get("value"));
                let reply = if result.get("exceptionDetails").is_some() {
                    Err(EngineError::CdpRejected)
                } else {
                    value
                        .map(|value| {
                            value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string())
                        })
                        .ok_or(EngineError::CdpRejected)
                };
                let _ = sender.send(reply);
            }
            Self::Screenshot(sender) => {
                let reply = result
                    .get("data")
                    .and_then(Value::as_str)
                    .and_then(|data| base64::engine::general_purpose::STANDARD.decode(data).ok())
                    .filter(|bytes| !bytes.is_empty())
                    .ok_or(EngineError::CdpRejected);
                let _ = sender.send(reply);
            }
            Self::Unit(sender) => {
                let reply = if result.get("errorText").is_some() {
                    Err(EngineError::CdpRejected)
                } else {
                    Ok(())
                };
                let _ = sender.send(reply);
            }
        }
    }
}

struct Pending {
    target: TabRef,
    reply: Reply,
}

#[derive(Default)]
pub(super) struct DevTools {
    observers: HashMap<u64, Registration>,
    pending: HashMap<i32, Pending>,
    next_id: i32,
}

pub(super) fn attach(tab: u64, browser: &Browser) {
    let Some(host) = browser.host() else {
        return;
    };
    let mut observer = Observer::new(tab);
    if let Some(registration) = host.add_dev_tools_message_observer(Some(&mut observer)) {
        with_host(|state| state.devtools.observers.insert(tab, registration));
    }
}

pub(super) fn send(tab: u64, method: &str, params: Value, reply: Reply) {
    let mut reply = Some(reply);
    let pending = with_host(|host| {
        if host.devtools.pending.len() >= 128 || !host.devtools.observers.contains_key(&tab) {
            return None;
        }
        let target = host.session.target(tab).ok()?;
        let id = host.devtools.next_id.checked_add(1)?;
        host.devtools.next_id = id;
        host.devtools.pending.insert(
            id,
            Pending {
                target,
                reply: reply.take()?,
            },
        );
        Some(id)
    })
    .flatten();
    let Some(id) = pending else {
        if let Some(reply) = reply {
            reply.fail(EngineError::Busy);
        }
        return;
    };
    let message = serde_json::json!({ "id": id, "method": method, "params": params }).to_string();
    let accepted = browser(tab)
        .and_then(|browser| browser.host())
        .is_some_and(|host| host.send_dev_tools_message(Some(message.as_bytes())) == 1);
    if !accepted {
        fail(id, EngineError::CdpRejected);
        return;
    }
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let mut task = TimeoutTask::new(id);
        post_task(ThreadId::UI, Some(&mut task));
    });
}

fn fail(id: i32, error: EngineError) {
    let pending = with_host(|host| host.devtools.pending.remove(&id)).flatten();
    if let Some(pending) = pending {
        pending.reply.fail(error);
    }
}

pub(super) fn close(tab: u64) {
    let pending = with_host(|host| {
        host.devtools.observers.remove(&tab);
        let ids: Vec<_> = host
            .devtools
            .pending
            .iter()
            .filter(|(_, pending)| pending.target.id == tab)
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| host.devtools.pending.remove(&id))
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    for pending in pending {
        pending.reply.fail(EngineError::ContextDisposed);
    }
}

wrap_dev_tools_message_observer! {
    struct Observer { tab: u64 }
    impl DevToolsMessageObserver {
        fn on_dev_tools_method_result(&self, _browser: Option<&mut Browser>, message_id: i32, success: i32, result: Option<&[u8]>) {
            let pending = with_host(|host| host.devtools.pending.remove(&message_id)).flatten();
            let Some(pending) = pending else { return; };
            let valid = with_host(|host| {
                if matches!(&pending.reply, Reply::Unit(_)) {
                    host.session.target(pending.target.id).is_ok()
                } else {
                    host.session.validate_target(pending.target).is_ok()
                }
            }).unwrap_or(false);
            if self.tab != pending.target.id || !valid { pending.reply.fail(EngineError::StaleTarget); return; }
            if success != 1 { pending.reply.fail(EngineError::CdpRejected); return; }
            match result.and_then(|bytes| serde_json::from_slice(bytes).ok()) {
                Some(result) => pending.reply.complete(result),
                None => pending.reply.fail(EngineError::CdpRejected),
            }
        }
    }
}

wrap_task! {
    struct TimeoutTask { id: i32 }
    impl Task { fn execute(&self) { fail(self.id, EngineError::Timeout); } }
}
