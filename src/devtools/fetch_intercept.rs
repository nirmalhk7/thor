//! CDP Fetch domain ↔ [`TrafficTamperEngine`] bridge.
//!
//! When interception is enabled, `Fetch.enable` pauses matching requests. Each
//! paused request is run through tamper rules:
//! - [`TamperOutcome::Forward`] → `Fetch.continueRequest` (with header edits)
//! - [`TamperOutcome::Mock`] → `Fetch.fulfillRequest`
//! - [`TamperOutcome::Pause`] → queued for UI review, then auto-continued so
//!   browsing does not hang until a continue/drop UI exists

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use chromiumoxide::cdp::browser_protocol::fetch::{
    ContinueRequestParams, DisableParams, EnableParams, EventRequestPaused, FulfillRequestParams,
    HeaderEntry, RequestPattern,
};
use chromiumoxide::Page;
use futures::StreamExt;
use std::time::Duration;
use tokio::task::JoinHandle;

use crate::engine::tab_session::TabSession;
use crate::net::tamper::{
    InterceptedRequest, InterceptedResponse, TamperOutcome, TrafficTamperEngine,
};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn headers_map(value: &serde_json::Value) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            if let Some(s) = v.as_str() {
                out.insert(k.clone(), s.to_string());
            } else {
                out.insert(k.clone(), v.to_string());
            }
        }
    }
    out
}

fn to_header_entries(headers: &HashMap<String, String>) -> Vec<HeaderEntry> {
    headers
        .iter()
        .map(|(k, v)| HeaderEntry::new(k.clone(), v.clone()))
        .collect()
}

fn b64_encode(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = (bytes[i] as u32) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn request_body(event: &EventRequestPaused) -> Option<String> {
    let entries = event.request.post_data_entries.as_ref()?;
    let mut raw = String::new();
        for entry in entries {
            if let Some(b) = &entry.bytes {
                // CDP Binary is already a base64 string; keep opaque text for tamper history.
                raw.push_str(AsRef::<str>::as_ref(b));
            }
        }
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
}

fn from_event(event: &EventRequestPaused) -> InterceptedRequest {
    InterceptedRequest {
        id: event.request_id.inner().clone(),
        timestamp: now_ms(),
        method: event.request.method.clone(),
        url: event.request.url.clone(),
        headers: headers_map(event.request.headers.inner()),
        body: request_body(event),
        is_modified: false,
    }
}

/// Enable Fetch interception on a tab and spawn a listener that applies tamper rules.
pub fn attach_blocking(
    session: &TabSession,
    tamper: TrafficTamperEngine,
) -> anyhow::Result<JoinHandle<()>> {
    let runtime = session.runtime().clone();
    let handle = runtime.handle().clone();
    let page = session
        .page()
        .ok_or_else(|| anyhow::anyhow!("page not initialized"))?;
    crate::engine::chromium_runtime::block_on_handle(&handle.clone(), async move {
        let enable = EnableParams::builder()
            .pattern(
                RequestPattern::builder()
                    .url_pattern("*")
                    .request_stage(chromiumoxide::cdp::browser_protocol::fetch::RequestStage::Request)
                    .build(),
            )
            .pattern(
                RequestPattern::builder()
                    .url_pattern("*")
                    .request_stage(chromiumoxide::cdp::browser_protocol::fetch::RequestStage::Response)
                    .build(),
            )
            .handle_auth_requests(false)
            .build();
        page.execute(enable)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut events = page
            .event_listener::<EventRequestPaused>()
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let page2 = page.clone();
        let task = handle.spawn(async move {
            while let Some(event) = events.next().await {
                if let Err(e) = handle_paused(&page2, &tamper, &event).await {
                    eprintln!("⚡ [Thor Fetch] pause handler: {e}");
                }
            }
        });
        Ok(task)
    })
}

/// Disable Fetch domain on a page (listener exits when events stop).
pub fn detach_blocking(session: &TabSession) -> anyhow::Result<()> {
    let page = session
        .page()
        .ok_or_else(|| anyhow::anyhow!("page not initialized"))?;
    crate::engine::chromium_runtime::block_on_handle(session.runtime().handle(), async move {
        page.execute(DisableParams::default())
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    })
}

async fn handle_paused(
    page: &Page,
    tamper: &TrafficTamperEngine,
    event: &EventRequestPaused,
) -> anyhow::Result<()> {
    // Response-stage pauses: record status into tamper history, then continue.
    if event.response_status_code.is_some() || event.response_error_reason.is_some() {
        let req = from_event(event);
        let resp = InterceptedResponse {
            request_id: event.request_id.inner().clone(),
            timestamp: now_ms(),
            status_code: event.response_status_code.unwrap_or(0) as u16,
            headers: event
                .response_headers
                .as_ref()
                .map(|hs| {
                    hs.iter()
                        .map(|h| (h.name.clone(), h.value.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            body: None,
            duration_ms: 0,
        };
        tamper.record_history(req, Some(resp));
        page.execute(ContinueRequestParams::new(event.request_id.clone()))
            .await?;
        return Ok(());
    }

    let req = from_event(event);
    match tamper.evaluate_tamper_outcome(req) {
        TamperOutcome::Mock(resp) => {
            fulfill(page, &event.request_id, &resp).await?;
        }
        TamperOutcome::Forward(fwd) => {
            if fwd.is_modified {
                let headers = to_header_entries(&fwd.headers);
                let mut builder = ContinueRequestParams::builder()
                    .request_id(event.request_id.clone())
                    .url(fwd.url.clone())
                    .method(fwd.method.clone())
                    .headers(headers);
                if let Some(body) = fwd.body.clone() {
                    builder = builder.post_data(body);
                }
                let params = builder.build().map_err(|e| anyhow::anyhow!(e))?;
                page.execute(params).await?;
            } else {
                page.execute(ContinueRequestParams::new(event.request_id.clone()))
                    .await?;
            }
            tamper.record_history(fwd, None);
        }
        TamperOutcome::Pause => {
            // True hold: block CDP continue until UI forward/drop (or 30s timeout).
            let id = event.request_id.inner().clone();
            let release = tokio::task::spawn_blocking({
                let tamper = tamper.clone();
                let id = id.clone();
                move || tamper.wait_for_pause_release(&id, Duration::from_secs(30))
            })
            .await
            .map_err(|e| anyhow::anyhow!("pause wait join: {e}"))?;

            match release {
                crate::net::tamper::PauseRelease::Drop => {
                    use chromiumoxide::cdp::browser_protocol::fetch::FailRequestParams;
                    use chromiumoxide::cdp::browser_protocol::network::ErrorReason;
                    let params = FailRequestParams::new(
                        event.request_id.clone(),
                        ErrorReason::BlockedByClient,
                    );
                    page.execute(params).await?;
                }
                crate::net::tamper::PauseRelease::Forward(fwd) => {
                    if fwd.is_modified {
                        let headers = to_header_entries(&fwd.headers);
                        let mut builder = ContinueRequestParams::builder()
                            .request_id(event.request_id.clone())
                            .url(fwd.url.clone())
                            .method(fwd.method.clone())
                            .headers(headers);
                        if let Some(body) = fwd.body.clone() {
                            builder = builder.post_data(body);
                        }
                        let params = builder.build().map_err(|e| anyhow::anyhow!(e))?;
                        page.execute(params).await?;
                    } else {
                        page.execute(ContinueRequestParams::new(event.request_id.clone()))
                            .await?;
                    }
                    tamper.record_history(fwd, None);
                }
            }
        }
    }
    Ok(())
}

async fn fulfill(
    page: &Page,
    request_id: &chromiumoxide::cdp::browser_protocol::fetch::RequestId,
    resp: &InterceptedResponse,
) -> anyhow::Result<()> {
    let headers = to_header_entries(&resp.headers);
    let mut builder = FulfillRequestParams::builder()
        .request_id(request_id.clone())
        .response_code(resp.status_code as i64)
        .response_headers(headers);
    if let Some(body) = &resp.body {
        builder = builder.body(b64_encode(body.as_bytes()));
    }
    let params = builder.build().map_err(|e| anyhow::anyhow!(e))?;
    page.execute(params).await?;
    Ok(())
}
