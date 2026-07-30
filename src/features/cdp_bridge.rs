//! CDP capture bridge.
//!
//! Turns live browser instrumentation (CDP `Network.*` events, a
//! `PerformanceObserver` injection, and the [`TrafficTamperEngine`] history)
//! into the concrete inputs the specialist engines expect — real
//! [`BrowserMetrics`], [`ResourceTiming`]/[`InitiatedResource`] waterfalls,
//! [`ApiRequest`] captures, and replayable tamper pairs. This is the "capture"
//! half that feeds the backend / security / performance analyzers instead of
//! synthesizing proxy data.
//!
//! Prefer the shared instance from
//! `AppState::global().features().cdp()`.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::features::backend::ApiRequest;
use crate::features::frontend::{DomNode, ImageAsset};
use crate::features::perf_features::{InitiatedResource, ResourceTiming};
use crate::load::BrowserMetrics;
use crate::net::tamper::{InterceptedRequest, InterceptedResponse};

/// A single captured network exchange, as assembled from CDP `Network.*`
/// lifecycle events (`requestWillBeSent`, `responseReceived`, `loadingFinished`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkEntry {
    pub url: String,
    pub method: String,
    pub status: u16,
    pub start_ms: f64,
    #[serde(default)]
    pub dns_ms: f64,
    #[serde(default)]
    pub connect_ms: f64,
    #[serde(default)]
    pub ttfb_ms: f64,
    #[serde(default)]
    pub download_ms: f64,
    #[serde(default)]
    pub bytes: u64,
    /// Encoded/on-wire transfer size when known (CDP `encodedDataLength`).
    #[serde(default)]
    pub transfer_bytes: Option<u64>,
    /// Decoded body size when known.
    #[serde(default)]
    pub decoded_bytes: Option<u64>,
    /// CDP connectionId for pool reuse correlation.
    #[serde(default)]
    pub connection_id: Option<String>,
    /// ALPN / protocol (`h2`, `h3`, `http/1.1`).
    #[serde(default)]
    pub protocol: Option<String>,
    /// URL of the resource / script that initiated this request, if known.
    #[serde(default)]
    pub initiator: Option<String>,
    #[serde(default)]
    pub render_blocking: bool,
    #[serde(default)]
    pub from_cache: bool,
    #[serde(default)]
    pub request_headers: HashMap<String, String>,
    #[serde(default)]
    pub response_headers: HashMap<String, String>,
}

impl NetworkEntry {
    fn total_ms(&self) -> f64 {
        self.dns_ms + self.connect_ms + self.ttfb_ms + self.download_ms
    }

    /// Parse a CDP-shaped Network JSON object into a [`NetworkEntry`].
    ///
    /// Recognizes flattened capture objects and Chromium CDP envelopes
    /// (`method` + `params.response` / `params.encodedDataLength`).
    pub fn from_cdp_json(value: &serde_json::Value) -> Option<Self> {
        let params = value.get("params").unwrap_or(value);
        let response = params
            .get("response")
            .or_else(|| value.get("response"))
            .unwrap_or(params);

        let url = response
            .get("url")
            .or_else(|| params.get("url"))
            .or_else(|| value.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            return None;
        }

        let method = params
            .get("request")
            .and_then(|r| r.get("method"))
            .or_else(|| value.get("method"))
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_string();

        let status = response
            .get("status")
            .and_then(|v| v.as_u64())
            .or_else(|| value.get("status").and_then(|v| v.as_u64()))
            .unwrap_or(0) as u16;

        let connection_id = response
            .get("connectionId")
            .or_else(|| value.get("connectionId"))
            .or_else(|| value.get("connection_id"))
            .and_then(|v| match v {
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
                _ => None,
            });

        let protocol = response
            .get("protocol")
            .or_else(|| response.get("alpn"))
            .or_else(|| value.get("protocol"))
            .or_else(|| value.get("alpn"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let transfer_bytes = response
            .get("encodedDataLength")
            .or_else(|| params.get("encodedDataLength"))
            .or_else(|| value.get("encodedDataLength"))
            .or_else(|| value.get("transfer_bytes"))
            .and_then(|v| v.as_u64());

        let decoded_bytes = response
            .get("decodedBodyLength")
            .or_else(|| value.get("decodedBodyLength"))
            .or_else(|| value.get("decoded_bytes"))
            .and_then(|v| v.as_u64());

        let bytes = decoded_bytes
            .or(transfer_bytes)
            .or_else(|| value.get("bytes").and_then(|v| v.as_u64()))
            .unwrap_or(0);

        let connection_reused = response
            .get("connectionReused")
            .or_else(|| value.get("connectionReused"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let from_disk_cache = response
            .get("fromDiskCache")
            .or_else(|| value.get("fromDiskCache"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let from_cache = connection_reused
            || from_disk_cache
            || value
                .get("from_cache")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

        let timing = response.get("timing").or_else(|| value.get("timing"));
        let (dns_ms, connect_ms, ttfb_ms, download_ms, start_ms) =
            parse_cdp_timing(timing, connection_reused);

        let mut request_headers = HashMap::new();
        if let Some(h) = params
            .get("request")
            .and_then(|r| r.get("headers"))
            .or_else(|| value.get("request_headers"))
            .or_else(|| value.get("requestHeaders"))
        {
            collect_headers(h, &mut request_headers);
        }
        let mut response_headers = HashMap::new();
        if let Some(h) = response
            .get("headers")
            .or_else(|| value.get("response_headers"))
            .or_else(|| value.get("responseHeaders"))
        {
            collect_headers(h, &mut response_headers);
        }

        let initiator = params
            .get("initiator")
            .and_then(|i| i.get("url").or_else(|| i.get("stack")))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    v.get("callFrames")
                        .and_then(|cf| cf.as_array())
                        .and_then(|a| a.first())
                        .and_then(|f| f.get("url"))
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                }
            })
            .or_else(|| {
                value
                    .get("initiator")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            });

        let render_blocking = value
            .get("render_blocking")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Some(Self {
            url,
            method,
            status,
            start_ms,
            dns_ms,
            connect_ms,
            ttfb_ms,
            download_ms,
            bytes,
            transfer_bytes,
            decoded_bytes,
            connection_id,
            protocol,
            initiator,
            render_blocking,
            from_cache,
            request_headers,
            response_headers,
        })
    }
}

fn collect_headers(value: &serde_json::Value, out: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                if let Some(s) = v.as_str() {
                    out.insert(k.clone(), s.to_string());
                } else if let Some(n) = v.as_f64() {
                    out.insert(k.clone(), n.to_string());
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let (Some(k), Some(v)) = (
                    item.get("name").and_then(|x| x.as_str()),
                    item.get("value").and_then(|x| x.as_str()),
                ) {
                    out.insert(k.to_string(), v.to_string());
                }
            }
        }
        _ => {}
    }
}

/// CDP `Network.Response.timing` uses ms offsets relative to `requestTime`.
fn parse_cdp_timing(
    timing: Option<&serde_json::Value>,
    connection_reused: bool,
) -> (f64, f64, f64, f64, f64) {
    let Some(t) = timing else {
        return (0.0, if connection_reused { 0.0 } else { 0.0 }, 0.0, 0.0, 0.0);
    };
    let f = |key: &str| t.get(key).and_then(|v| v.as_f64()).unwrap_or(-1.0);
    let request_time = f("requestTime");
    let start_ms = if request_time > 0.0 {
        request_time * 1000.0
    } else {
        t.get("start_ms").and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let dns_start = f("dnsStart");
    let dns_end = f("dnsEnd");
    let connect_start = f("connectStart");
    let connect_end = f("connectEnd");
    let send_start = f("sendStart");
    let receive_headers_end = f("receiveHeadersEnd");

    let dns_ms = if dns_start >= 0.0 && dns_end >= dns_start {
        dns_end - dns_start
    } else {
        0.0
    };
    let connect_ms = if connection_reused {
        0.0
    } else if connect_start >= 0.0 && connect_end >= connect_start {
        connect_end - connect_start
    } else {
        t.get("connect_ms").and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let ttfb_ms = if receive_headers_end >= 0.0 && send_start >= 0.0 {
        (receive_headers_end - send_start).max(0.0)
    } else {
        t.get("ttfb_ms").and_then(|v| v.as_f64()).unwrap_or(0.0)
    };
    let download_ms = t.get("download_ms").and_then(|v| v.as_f64()).unwrap_or(0.0);
    (dns_ms, connect_ms, ttfb_ms, download_ms, start_ms)
}

/// Callback that evaluates JS against the active Chromium tab.
pub type ActiveEvaluator = Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>;

static ACTIVE_EVALUATOR: Mutex<Option<ActiveEvaluator>> = Mutex::new(None);

/// Process-wide capture bridge holding the current tab's network entries.
pub struct CdpBridge {
    entries: Mutex<Vec<NetworkEntry>>,
}

impl Default for CdpBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl CdpBridge {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Install (or replace) the process-wide active-tab JS evaluator.
    /// Shell/`ChromiumTabs` registers this when a live TabSession is available.
    pub fn install_active_evaluator(eval: ActiveEvaluator) {
        *ACTIVE_EVALUATOR.lock() = Some(eval);
    }

    /// Clear the active-tab evaluator (no Chromium runtime / tabs closed).
    pub fn clear_active_evaluator() {
        *ACTIVE_EVALUATOR.lock() = None;
    }

    /// Evaluate `script` on the active tab via the installed hook.
    ///
    /// Returns a clear `Err` when no Chromium runtime is bound — never a silent
    /// empty success.
    pub fn evaluate_on_active(script: &str) -> Result<String, String> {
        let guard = ACTIVE_EVALUATOR.lock();
        let Some(eval) = guard.as_ref() else {
            return Err(
                "no Chromium runtime: active-tab evaluate unavailable (from_active_tab requires a live tab)"
                    .into(),
            );
        };
        eval(script)
    }

    /// JS injected via `Page.addScriptToEvaluateOnNewDocument` that wires up
    /// PerformanceObservers for LCP, INP (Event Timing) and CLS, and exposes
    /// `window.__thorCollectWebVitals()` returning a flat JSON payload that
    /// [`CdpBridge::parse_web_vitals`] can parse. Kept dependency-free so it can
    /// run on any page without shipping the web-vitals library.
    pub fn web_vitals_injection_script() -> &'static str {
        WEB_VITALS_SCRIPT
    }

    /// One-shot shell capture: install PerformanceObservers (buffered) then
    /// post a `web_vitals_result` IPC summary. Uses the same collectors as the
    /// load BrowserRunner — no invented CWV.
    pub fn web_vitals_shell_capture_script(ipc_via_queue: bool) -> String {
        let post = if ipc_via_queue {
            r#"window.__thorIpcQueue = window.__thorIpcQueue || [];
               window.__thorIpcQueue.push(MSG);"#
        } else {
            r#"if (window.ipc && window.ipc.postMessage) window.ipc.postMessage(MSG);
               else if (window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc)
                 window.webkit.messageHandlers.ipc.postMessage(MSG);"#
        };
        let post_msg = post.replace("MSG", "msg");
        let post_msg2 = post.replace("MSG", "msg2");
        let mut out = String::from("(function(){\n  try {\n");
        out.push_str(WEB_VITALS_SCRIPT);
        out.push_str(
            r#"
    var raw = (typeof window.__thorCollectWebVitals === 'function')
      ? window.__thorCollectWebVitals() : '{}';
    var v = {};
    try { v = JSON.parse(raw); } catch (e1) { v = {}; }
    var has = (v.lcp > 0) || (v.cls > 0) || (v.inp > 0) || (v.ttfb > 0) || (v.fcp > 0) || (v.tbt > 0);
    var summary;
    if (!has) {
      summary = 'vitals unavailable (no PerformanceObserver data)';
    } else {
      summary = 'LCP=' + (v.lcp ? Math.round(v.lcp) + 'ms' : '?')
        + ' CLS=' + (v.cls != null ? Number(v.cls).toFixed(3) : '?')
        + ' INP=' + (v.inp ? Math.round(v.inp) + 'ms' : '?')
        + ' TTFB=' + (v.ttfb ? Math.round(v.ttfb) + 'ms' : '?');
    }
    var msg = JSON.stringify({cmd:'web_vitals_result', summary: summary});
"#,
        );
        out.push_str(&post_msg);
        out.push_str(
            r#"
  } catch (e) {
    var msg2 = JSON.stringify({cmd:'web_vitals_result', summary: 'vitals error'});
"#,
        );
        out.push_str(&post_msg2);
        out.push_str("\n  }\n})();");
        out
    }

    /// Soft-navigation observer + speculationrules collector.
    /// Feed collected JSON through [`CdpBridge::parse_soft_navs`] into
    /// [`crate::features::perf_features::PerfEngine::soft_nav_vitals`].
    pub fn soft_nav_and_speculation_script() -> &'static str {
        SOFT_NAV_SCRIPT
    }

    /// Parse the Core Web Vitals JSON produced by the injection script (or any
    /// `Runtime.evaluate` collection) into real [`BrowserMetrics`]. Delegates to
    /// the canonical lenient parser in `devtools::cdp_integration`.
    pub fn parse_web_vitals(json: &str) -> Option<BrowserMetrics> {
        crate::devtools::cdp_integration::parse_web_vitals(json)
    }

    /// Parse CWV JSON plus optional LCP media bytes (`lcpSize` / `lcpElement.size`).
    pub fn parse_web_vitals_ex(json: &str) -> Option<(BrowserMetrics, Option<u64>)> {
        crate::devtools::cdp_integration::parse_web_vitals_ex(json)
    }

    /// Parse `__thorCollectLongTasks()` JSON into long-task / LoAF events.
    pub fn parse_long_tasks(json: &str) -> Vec<crate::features::perf_features::LongTaskEvent> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
            return Vec::new();
        };
        let arr = match value.as_array() {
            Some(a) => a.clone(),
            None => return Vec::new(),
        };
        arr.into_iter()
            .filter_map(|v| {
                let duration_ms = v
                    .get("duration_ms")
                    .or_else(|| v.get("duration"))
                    .and_then(|x| x.as_f64())?;
                let start_ms = v
                    .get("start_ms")
                    .or_else(|| v.get("startTime"))
                    .and_then(|x| x.as_f64())
                    .unwrap_or(0.0);
                let script_url = v
                    .get("script_url")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                let name = v
                    .get("name")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string());
                let inp_overlap = v
                    .get("inp_overlap")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                Some(crate::features::perf_features::LongTaskEvent {
                    duration_ms,
                    start_ms,
                    script_url,
                    name,
                    inp_overlap,
                })
            })
            .collect()
    }

    /// Parse `__thorCollectSoftNavs()` JSON into soft-nav records.
    pub fn parse_soft_navs(json: &str) -> Vec<crate::load::SoftNavRecord> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
            return Vec::new();
        };
        let arr = match value.as_array() {
            Some(a) => a.clone(),
            None => return Vec::new(),
        };
        arr.into_iter()
            .filter_map(|v| {
                Some(crate::load::SoftNavRecord {
                    navigation_id: v
                        .get("navigation_id")
                        .and_then(|x| x.as_str())
                        .unwrap_or("1")
                        .to_string(),
                    url: v
                        .get("url")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    lcp_ms: v.get("lcp_ms").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    inp_ms: v.get("inp_ms").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    cls: v.get("cls").and_then(|x| x.as_f64()).unwrap_or(0.0),
                    ttfb_ms: v.get("ttfb_ms").and_then(|x| x.as_f64()).unwrap_or(0.0),
                })
            })
            .collect()
    }

    /// Ingest one captured network exchange (waterfall / cache / third-party).
    pub fn ingest_network_entry(&self, entry: NetworkEntry) {
        self.entries.lock().push(entry);
    }

    /// Ingest CDP-shaped Network JSON (single object or array).
    ///
    /// Accepts flattened entries and Chromium event envelopes
    /// (`Network.responseReceived` / `Network.loadingFinished` / assembled
    /// request+response objects). Sets real `connection_id`, `protocol`/ALPN,
    /// `transfer_bytes` / `decoded_bytes` when present — never invents
    /// `url:start_ms` connection ids when CDP `connectionId` exists.
    pub fn ingest_cdp_network_json(&self, json: &str) -> usize {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
            return 0;
        };
        let items: Vec<serde_json::Value> = match value {
            serde_json::Value::Array(a) => a,
            serde_json::Value::Object(ref o) => {
                if let Some(arr) = o.get("entries").and_then(|v| v.as_array()) {
                    arr.clone()
                } else {
                    vec![value]
                }
            }
            _ => return 0,
        };
        let mut n = 0;
        for item in items {
            if let Some(entry) = NetworkEntry::from_cdp_json(&item) {
                self.ingest_network_entry(entry);
                n += 1;
            }
        }
        n
    }

    /// Ingest recorded tamper-engine traffic history as network entries so the
    /// same analyzers can run over intercepted/repeated traffic.
    pub fn ingest_tamper_history(
        &self,
        history: &[(InterceptedRequest, Option<InterceptedResponse>)],
    ) {
        let mut entries = self.entries.lock();
        for (req, resp) in history {
            let (status, download_ms, response_headers, bytes) = match resp {
                Some(r) => (
                    r.status_code,
                    r.duration_ms as f64,
                    r.headers.clone(),
                    r.body.as_ref().map(|b| b.len() as u64).unwrap_or(0),
                ),
                None => (0, 0.0, HashMap::new(), 0),
            };
            entries.push(NetworkEntry {
                url: req.url.clone(),
                method: req.method.clone(),
                status,
                start_ms: req.timestamp as f64,
                ttfb_ms: download_ms,
                bytes,
                from_cache: response_headers
                    .iter()
                    .any(|(k, v)| k.eq_ignore_ascii_case("x-cache") && v.to_lowercase().contains("hit")),
                request_headers: req.headers.clone(),
                response_headers,
                ..Default::default()
            });
        }
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
    }

    pub fn entries(&self) -> Vec<NetworkEntry> {
        self.entries.lock().clone()
    }

    /// Sum measured transfer / decoded bytes across ingested entries.
    pub fn byte_totals(&self) -> (u64, u64) {
        let entries = self.entries.lock();
        let transfer = entries
            .iter()
            .map(|e| e.transfer_bytes.unwrap_or(e.bytes))
            .sum();
        let decoded = entries
            .iter()
            .map(|e| e.decoded_bytes.unwrap_or(e.bytes))
            .sum();
        (transfer, decoded)
    }

    /// JS expression that serializes Navigation + Resource Timing entries into
    /// CDP-shaped network JSON for [`Self::ingest_cdp_network_json`].
    /// Uses real `transferSize` / `decodedBodySize` — never invents sizes.
    pub fn resource_timing_network_script() -> &'static str {
        r#"(() => {
          try {
            const out = [];
            const nav = performance.getEntriesByType('navigation')[0];
            if (nav) {
              out.push({
                url: location.href,
                method: 'GET',
                status: 200,
                start_ms: 0,
                ttfb_ms: nav.responseStart || 0,
                download_ms: Math.max(0, (nav.responseEnd || 0) - (nav.responseStart || 0)),
                transfer_bytes: nav.transferSize || 0,
                decoded_bytes: nav.decodedBodySize || 0,
                bytes: nav.transferSize || nav.decodedBodySize || 0,
                encodedDataLength: nav.transferSize || 0,
                decodedBodyLength: nav.decodedBodySize || 0,
                protocol: nav.nextHopProtocol || undefined,
                from_cache: (nav.transferSize === 0 && (nav.decodedBodySize || 0) > 0)
              });
            }
            for (const e of performance.getEntriesByType('resource')) {
              out.push({
                url: e.name,
                method: 'GET',
                status: 200,
                start_ms: e.startTime || 0,
                ttfb_ms: Math.max(0, (e.responseStart || 0) - (e.requestStart || 0)),
                download_ms: Math.max(0, (e.responseEnd || 0) - (e.responseStart || 0)),
                transfer_bytes: e.transferSize || 0,
                decoded_bytes: e.decodedBodySize || 0,
                bytes: e.transferSize || e.decodedBodySize || 0,
                encodedDataLength: e.transferSize || 0,
                decodedBodyLength: e.decodedBodySize || 0,
                protocol: e.nextHopProtocol || undefined,
                from_cache: (e.transferSize === 0 && (e.decodedBodySize || 0) > 0)
              });
            }
            return JSON.stringify(out);
          } catch (err) {
            return '[]';
          }
        })()"#
    }

    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }

    /// Convert captured entries into perf waterfall timings.
    pub fn resource_timings(&self) -> Vec<ResourceTiming> {
        self.entries
            .lock()
            .iter()
            .map(|e| ResourceTiming {
                url: e.url.clone(),
                start_ms: e.start_ms,
                dns_ms: e.dns_ms,
                connect_ms: e.connect_ms,
                ttfb_ms: e.ttfb_ms,
                download_ms: e.download_ms,
                render_blocking: e.render_blocking,
                bytes: e.bytes,
            })
            .collect()
    }

    /// Convert captured entries into the initiator graph for blocking-chain
    /// reconstruction.
    pub fn initiated_resources(&self) -> Vec<InitiatedResource> {
        self.entries
            .lock()
            .iter()
            .map(|e| InitiatedResource {
                url: e.url.clone(),
                initiator: e.initiator.clone(),
                start_ms: e.start_ms,
                total_ms: e.total_ms(),
                render_blocking: e.render_blocking,
            })
            .collect()
    }

    /// Convert captured entries into replayable [`ApiRequest`]s for the API
    /// workbench / chaining / IDOR engines.
    pub fn api_requests(&self) -> Vec<ApiRequest> {
        self.entries
            .lock()
            .iter()
            .map(|e| ApiRequest {
                method: e.method.clone(),
                url: e.url.clone(),
                headers: e.request_headers.clone(),
                body: None,
            })
            .collect()
    }

    /// Convert captured entries into compression-budget inputs.
    pub fn compression_entries(&self) -> Vec<crate::features::perf_features::CompressionEntry> {
        self.entries
            .lock()
            .iter()
            .map(|e| {
                let encoding = e
                    .response_headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-encoding"))
                    .map(|(_, v)| v.clone());
                let content_type = e
                    .response_headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.clone());
                crate::features::perf_features::CompressionEntry {
                    url: e.url.clone(),
                    transfer_bytes: e.transfer_bytes.unwrap_or(e.bytes),
                    decoded_bytes: e.decoded_bytes.unwrap_or(e.bytes),
                    content_encoding: encoding,
                    content_type,
                }
            })
            .collect()
    }

    /// Convert captured entries into connection-pool inspector inputs.
    ///
    /// Reuse is derived from CDP `connectionReused` / cache flags (`from_cache`)
    /// or zero connect time — never from inventing connection ids.
    pub fn conn_pool_entries(&self) -> Vec<crate::features::perf_features::ConnPoolEntry> {
        let entries = self.entries.lock();
        let mut seen: HashMap<String, usize> = HashMap::new();
        entries
            .iter()
            .map(|e| {
                let connection_id = e.connection_id.clone().unwrap_or_else(|| {
                    // Fallback only when CDP omitted connectionId.
                    format!("{}:{}", e.url, e.start_ms as u64)
                });
                let prior = seen.get(&connection_id).copied().unwrap_or(0);
                seen.insert(connection_id.clone(), prior + 1);
                let reused = e.from_cache || e.connect_ms <= 0.0 || prior > 0;
                crate::features::perf_features::ConnPoolEntry {
                    url: e.url.clone(),
                    connection_id,
                    wait_ms: e.connect_ms.max(0.0),
                    reused,
                    protocol: e.protocol.clone(),
                }
            })
            .collect()
    }

    /// Convert captured entries into HTTP/3 path audit samples.
    pub fn http3_path_samples(&self) -> Vec<crate::features::perf_features::Http3PathSample> {
        self.entries
            .lock()
            .iter()
            .map(|e| crate::features::perf_features::Http3PathSample {
                url: e.url.clone(),
                alpn: e.protocol.clone(),
                ttfb_ms: e.ttfb_ms,
                fallback_from: None,
                fallback_cost_ms: None,
            })
            .collect()
    }

    /// Distinct third-party hosts (relative to `first_party_host`) seen in the
    /// capture, with their aggregate byte weight — feeds third-party attribution.
    pub fn third_party_bytes(&self, first_party_host: &str) -> HashMap<String, u64> {
        let mut out: HashMap<String, u64> = HashMap::new();
        for e in self.entries.lock().iter() {
            if let Some(host) = host_of(&e.url) {
                if !host.eq_ignore_ascii_case(first_party_host) && !host.ends_with(first_party_host)
                {
                    *out.entry(host).or_default() += e.bytes;
                }
            }
        }
        out
    }

    /// JS snippet that walks the live DOM and returns a compact JSON tree
    /// suitable for [`CdpBridge::parse_dom_snapshot`] → FrontendEngine analyzers.
    pub fn dom_snapshot_capture_script() -> &'static str {
        r#"(() => {
  function walk(n) {
    if (!n || n.nodeType !== 1) return null;
    const attrs = {};
    for (const a of n.attributes || []) attrs[a.name] = a.value;
    const children = [];
    for (const c of n.children || []) {
      const w = walk(c);
      if (w) children.push(w);
    }
    let text = '';
    for (const c of n.childNodes || []) {
      if (c.nodeType === 3) text += c.textContent || '';
    }
    return { tag: n.tagName.toLowerCase(), attributes: attrs, text: text.trim() || null, children };
  }
  return JSON.stringify(walk(document.documentElement));
})()"#
    }

    /// Parse CDP script / Runtime.evaluate JSON into a [`DomNode`] tree.
    /// Accepts either a JSON object or a JSON string double-encoding.
    pub fn parse_dom_snapshot(json: &str) -> Option<DomNode> {
        let trimmed = json.trim();
        if trimmed.is_empty() || trimmed == "null" {
            return None;
        }
        // Runtime.evaluate often wraps the result as a quoted JSON string.
        if let Ok(inner) = serde_json::from_str::<String>(trimmed) {
            return serde_json::from_str(&inner).ok();
        }
        serde_json::from_str(trimmed).ok()
    }

    /// Derive [`ImageAsset`]s from captured network entries (image MIME / URL).
    /// Dimensions are unknown without DOM — natural/display stay 0 so the
    /// auditor still flags heavy bytes / format issues from transfer size.
    pub fn images_from_network(&self) -> Vec<ImageAsset> {
        self.entries
            .lock()
            .iter()
            .filter_map(|e| {
                let ct = e
                    .response_headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.to_lowercase())
                    .unwrap_or_default();
                let url_l = e.url.to_lowercase();
                let is_image = ct.starts_with("image/")
                    || url_l.contains(".png")
                    || url_l.contains(".jpg")
                    || url_l.contains(".jpeg")
                    || url_l.contains(".webp")
                    || url_l.contains(".gif")
                    || url_l.contains(".avif")
                    || url_l.contains(".svg")
                    || url_l.contains(".bmp");
                if !is_image {
                    return None;
                }
                let format = if ct.starts_with("image/") {
                    ct.trim_start_matches("image/")
                        .split(';')
                        .next()
                        .unwrap_or("unknown")
                        .trim()
                        .to_string()
                } else if url_l.contains(".webp") {
                    "webp".into()
                } else if url_l.contains(".avif") {
                    "avif".into()
                } else if url_l.contains(".png") {
                    "png".into()
                } else if url_l.contains(".gif") {
                    "gif".into()
                } else if url_l.contains(".svg") {
                    "svg".into()
                } else if url_l.contains(".bmp") {
                    "bmp".into()
                } else {
                    "jpeg".into()
                };
                Some(ImageAsset {
                    url: e.url.clone(),
                    natural_w: 0,
                    natural_h: 0,
                    display_w: 0.0,
                    display_h: 0.0,
                    bytes: e.transfer_bytes.unwrap_or(e.bytes),
                    format,
                    has_srcset: false,
                    loading: "eager".into(),
                })
            })
            .collect()
    }

}

fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.split('@').next_back()?;
    let host = host.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

const WEB_VITALS_SCRIPT: &str = r#"(() => {
  if (window.__thorWebVitalsInstalled) return;
  window.__thorWebVitalsInstalled = true;
  const v = { lcp: 0, inp: 0, cls: 0, fcp: 0, tbt: 0, fid: 0 };
  const lcpElement = { url: '', type: 'unknown', size: 0, discoverTime: 0, loadTime: 0, renderTime: 0 };
  const longTasks = [];
  const inpWindows = [];
  window.__thorWebVitals = v;
  window.__thorLcpElement = lcpElement;
  window.__thorLongTasks = longTasks;
  function overlapsInp(start, dur) {
    const end = start + dur;
    for (const w of inpWindows) {
      if (start < w.end && end > w.start) return true;
    }
    return false;
  }
  function pushBlocking(entry, nameHint) {
    const dur = entry.duration || 0;
    if (dur > 50) v.tbt += (dur - 50);
    let scriptUrl = '';
    try {
      const scripts = entry.attribution || [];
      if (scripts.length) {
        scriptUrl = scripts[0].containerSrc || scripts[0].scriptURL || scripts[0].name || '';
      }
    } catch (e2) {}
    longTasks.push({
      duration_ms: dur,
      start_ms: entry.startTime || 0,
      script_url: scriptUrl || null,
      name: nameHint || entry.name || 'longtask',
      inp_overlap: overlapsInp(entry.startTime || 0, dur)
    });
  }
  try {
    new PerformanceObserver((l) => {
      const es = l.getEntries();
      const last = es[es.length - 1];
      if (last) {
        v.lcp = last.renderTime || last.loadTime || last.startTime || v.lcp;
        lcpElement.renderTime = v.lcp;
        lcpElement.loadTime = last.loadTime || last.startTime || lcpElement.loadTime;
        lcpElement.discoverTime = last.startTime || lcpElement.discoverTime;
        lcpElement.size = last.size || lcpElement.size;
        lcpElement.url = (last.url || (last.element && last.element.currentSrc) || lcpElement.url || '');
        lcpElement.type = last.element ? (last.element.tagName || 'element').toLowerCase() : (last.url ? 'image' : lcpElement.type);
      }
    }).observe({ type: 'largest-contentful-paint', buffered: true });
  } catch (e) {}
  try {
    let clsValue = 0;
    new PerformanceObserver((l) => {
      for (const entry of l.getEntries()) {
        if (!entry.hadRecentInput) clsValue += entry.value;
      }
      v.cls = clsValue;
    }).observe({ type: 'layout-shift', buffered: true });
  } catch (e) {}
  try {
    new PerformanceObserver((l) => {
      for (const entry of l.getEntries()) {
        const dur = entry.duration || 0;
        if (dur > v.inp) v.inp = dur;
        const start = entry.startTime || 0;
        inpWindows.push({ start: start, end: start + dur });
        // Retroactively mark overlapping longtasks/LoAFs.
        for (const t of longTasks) {
          if (!t.inp_overlap && t.start_ms < start + dur && t.start_ms + t.duration_ms > start) {
            t.inp_overlap = true;
          }
        }
      }
    }).observe({ type: 'event', buffered: true, durationThreshold: 16 });
  } catch (e) {}
  try {
    new PerformanceObserver((l) => {
      for (const entry of l.getEntries()) {
        if (entry.name === 'first-contentful-paint') v.fcp = entry.startTime;
      }
    }).observe({ type: 'paint', buffered: true });
  } catch (e) {}
  let loafOk = false;
  try {
    new PerformanceObserver((l) => {
      for (const entry of l.getEntries()) {
        pushBlocking(entry, 'loaf');
      }
    }).observe({ type: 'long-animation-frame', buffered: true });
    loafOk = true;
  } catch (e) {}
  if (!loafOk) {
    try {
      new PerformanceObserver((l) => {
        for (const entry of l.getEntries()) {
          pushBlocking(entry, 'longtask');
        }
      }).observe({ type: 'longtask', buffered: true });
    } catch (e) {}
  }
  window.__thorCollectWebVitals = () => {
    const nav = performance.getEntriesByType('navigation')[0] || {};
    return JSON.stringify({
      lcp: v.lcp,
      inp: v.inp,
      cls: v.cls,
      fcp: v.fcp,
      tbt: v.tbt,
      fid: v.fid,
      ttfb: nav.responseStart || 0,
      domContentLoaded: nav.domContentLoadedEventEnd || 0,
      domInteractive: nav.domInteractive || 0,
      lcpElement: lcpElement,
      lcpUrl: lcpElement.url,
      lcpType: lcpElement.type,
      lcpSize: lcpElement.size
    });
  };
  window.__thorCollectLongTasks = () => JSON.stringify(longTasks);
})();"#;

const SOFT_NAV_SCRIPT: &str = r#"(() => {
  if (window.__thorSoftNavInstalled) return;
  window.__thorSoftNavInstalled = true;
  const soft = [];
  window.__thorSoftNavs = soft;
  try {
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        const navId = String(e.navigationId || soft.length + 1);
        const rec = {
          navigation_id: navId,
          url: e.name || location.href,
          lcp_ms: 0,
          inp_ms: 0,
          cls: 0,
          ttfb_ms: 0
        };
        soft.push(rec);
        try {
          new PerformanceObserver((l) => {
            const es = l.getEntries();
            const last = es[es.length - 1];
            if (last) rec.lcp_ms = last.renderTime || last.loadTime || last.startTime || rec.lcp_ms;
          }).observe({ type: 'largest-contentful-paint', buffered: true });
        } catch (e2) {}
        try {
          new PerformanceObserver((l) => {
            for (const entry of l.getEntries()) {
              const dur = entry.duration || 0;
              if (dur > rec.inp_ms) rec.inp_ms = dur;
            }
          }).observe({ type: 'event', buffered: true, durationThreshold: 16 });
        } catch (e2) {}
        try {
          let clsValue = 0;
          new PerformanceObserver((l) => {
            for (const entry of l.getEntries()) {
              if (!entry.hadRecentInput) clsValue += entry.value;
            }
            rec.cls = clsValue;
          }).observe({ type: 'layout-shift', buffered: true });
        } catch (e2) {}
      }
    }).observe({ type: 'soft-navigation', buffered: true });
  } catch (e) {}
  window.__thorCollectSoftNavs = () => JSON.stringify(soft);
  window.__thorSpeculationRules = () => {
    const rules = [];
    document.querySelectorAll('script[type="speculationrules"]').forEach((s) => {
      try { rules.push(JSON.parse(s.textContent || '{}')); } catch (e) {}
    });
    return JSON.stringify(rules);
  };
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_script() {
        let script = CdpBridge::web_vitals_injection_script();
        assert!(script.contains("largest-contentful-paint"));
        assert!(script.contains("__thorCollectWebVitals"));
        assert!(script.contains("long-animation-frame") || script.contains("longtask"));
        assert!(script.contains("inp_overlap"));
        assert!(script.contains("lcpElement"));
        assert!(CdpBridge::soft_nav_and_speculation_script().contains("soft-navigation"));
        assert!(CdpBridge::resource_timing_network_script().contains("transferSize"));
        assert!(CdpBridge::resource_timing_network_script().contains("decodedBodySize"));
        let shell = CdpBridge::web_vitals_shell_capture_script(true);
        assert!(shell.contains("__thorCollectWebVitals"));
        assert!(shell.contains("web_vitals_result"));
        assert!(shell.contains("vitals unavailable"));
        assert!(shell.contains("__thorIpcQueue"));
        let m = CdpBridge::parse_web_vitals(r#"{"lcp": 1500.0, "cls": 0.05, "ttfb": 300.0}"#)
            .expect("vitals");
        assert!((m.lcp_ms - 1500.0).abs() < 1e-6);
        let tasks = CdpBridge::parse_long_tasks(
            r#"[{"duration_ms":120,"start_ms":10,"script_url":"https://a.js","name":"loaf","inp_overlap":true}]"#,
        );
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].inp_overlap);
    }

    #[test]
    fn ingest_cdp_network_json_sets_pool_http3_compression() {
        let b = CdpBridge::new();
        let json = r#"[
          {
            "method": "Network.responseReceived",
            "params": {
              "requestId": "1",
              "response": {
                "url": "https://cdn.example.com/app.js",
                "status": 200,
                "protocol": "h3",
                "connectionId": 42,
                "connectionReused": false,
                "encodedDataLength": 1000,
                "headers": { "content-encoding": "br", "content-type": "application/javascript" },
                "timing": {
                  "requestTime": 1.0,
                  "dnsStart": 0, "dnsEnd": 5,
                  "connectStart": 5, "connectEnd": 25,
                  "sendStart": 25, "receiveHeadersEnd": 45
                }
              }
            }
          },
          {
            "url": "https://cdn.example.com/chunk.js",
            "method": "GET",
            "status": 200,
            "protocol": "h3",
            "connectionId": 42,
            "connectionReused": true,
            "encodedDataLength": 500,
            "decodedBodyLength": 2500,
            "headers": { "content-encoding": "br" },
            "timing": { "requestTime": 2.0, "connectStart": -1, "connectEnd": -1, "sendStart": 0, "receiveHeadersEnd": 12 }
          }
        ]"#;
        assert_eq!(b.ingest_cdp_network_json(json), 2);
        let (transfer, decoded) = b.byte_totals();
        assert_eq!(transfer, 1500);
        assert_eq!(decoded, 3500); // 1000 (fallback bytes) + 2500

        let pool = b.conn_pool_entries();
        assert_eq!(pool.len(), 2);
        assert_eq!(pool[0].connection_id, "42");
        assert_eq!(pool[1].connection_id, "42");
        assert!(pool[1].reused, "second request on same connectionId must be reused");
        assert_eq!(pool[0].protocol.as_deref(), Some("h3"));

        let h3 = b.http3_path_samples();
        assert!(h3.iter().all(|s| s.alpn.as_deref() == Some("h3")));

        let comp = b.compression_entries();
        assert_eq!(comp[0].transfer_bytes, 1000);
        assert_eq!(comp[1].transfer_bytes, 500);
        assert_eq!(comp[1].decoded_bytes, 2500);
        let ratio = comp[1].decoded_bytes as f64 / comp[1].transfer_bytes as f64;
        assert!((ratio - 5.0).abs() < 1e-6);

        let pe = crate::features::perf_features::PerfEngine::new();
        let report = pe.conn_pool_inspect(&pool);
        assert!(report.reuse_pct > 0.0);
        let h3_report = pe.http3_path_audit(&h3);
        assert!((h3_report.h3_pct - 100.0).abs() < 1e-6);
        let c_report = pe.compression_budget(&comp, 4.0);
        assert_eq!(c_report.entries, 2);
        assert!(c_report.inflation_ratio > 1.0);
        // 1000+500 transfer, decoded defaults to transfer for first + 2500 for second
        // = 1500 transfer, 3500 decoded → ratio ≈ 2.33
        assert!((c_report.inflation_ratio - (3500.0 / 1500.0)).abs() < 1e-6);
    }

    #[test]
    fn parse_soft_navs_json() {
        let recs = CdpBridge::parse_soft_navs(
            r#"[{"navigation_id":"2","url":"https://app/soft","lcp_ms":900,"inp_ms":40,"cls":0.01,"ttfb_ms":0}]"#,
        );
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].navigation_id, "2");
        assert!((recs[0].lcp_ms - 900.0).abs() < 1e-6);
    }

    #[test]
    fn ingest_and_convert() {
        let b = CdpBridge::new();
        b.ingest_network_entry(NetworkEntry {
            url: "https://app.example.com/api/users".into(),
            method: "GET".into(),
            status: 200,
            start_ms: 0.0,
            ttfb_ms: 40.0,
            download_ms: 10.0,
            bytes: 512,
            initiator: Some("https://app.example.com/".into()),
            ..Default::default()
        });
        b.ingest_network_entry(NetworkEntry {
            url: "https://cdn.thirdparty.io/tag.js".into(),
            method: "GET".into(),
            status: 200,
            bytes: 8192,
            ..Default::default()
        });
        assert_eq!(b.resource_timings().len(), 2);
        assert_eq!(b.initiated_resources().len(), 2);
        assert_eq!(b.api_requests()[0].url, "https://app.example.com/api/users");
        let tp = b.third_party_bytes("app.example.com");
        assert_eq!(tp.get("cdn.thirdparty.io"), Some(&8192));
        assert!(!tp.contains_key("app.example.com"));
    }

    #[test]
    fn parse_dom_snapshot_and_images_from_network() {
        let json = r#"{"tag":"html","attributes":{},"text":null,"children":[{"tag":"body","attributes":{},"text":null,"children":[{"tag":"div","attributes":{"id":"app"},"text":"hi","children":[]}]}]}"#;
        let root = CdpBridge::parse_dom_snapshot(json).expect("dom");
        assert_eq!(root.tag, "html");
        assert_eq!(root.children[0].tag, "body");
        let wrapped = serde_json::to_string(json).unwrap();
        assert!(CdpBridge::parse_dom_snapshot(&wrapped).is_some());

        let b = CdpBridge::new();
        let mut headers = HashMap::new();
        headers.insert("content-type".into(), "image/png".into());
        b.ingest_network_entry(NetworkEntry {
            url: "https://cdn.example.com/hero.png".into(),
            method: "GET".into(),
            status: 200,
            bytes: 250_000,
            transfer_bytes: Some(250_000),
            response_headers: headers,
            ..Default::default()
        });
        let imgs = b.images_from_network();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].format, "png");
        assert_eq!(imgs[0].bytes, 250_000);
    }

    #[test]
    fn ingest_tamper_history_pairs() {
        let b = CdpBridge::new();
        let req = InterceptedRequest {
            id: "1".into(),
            timestamp: 100,
            method: "POST".into(),
            url: "https://api.example.com/login".into(),
            headers: HashMap::new(),
            body: Some("{}".into()),
            is_modified: false,
        };
        let resp = InterceptedResponse {
            request_id: "1".into(),
            timestamp: 150,
            status_code: 200,
            headers: HashMap::new(),
            body: Some("ok".into()),
            duration_ms: 50,
        };
        b.ingest_tamper_history(&[(req, Some(resp))]);
        assert_eq!(b.len(), 1);
        assert_eq!(b.api_requests()[0].method, "POST");
    }
}
