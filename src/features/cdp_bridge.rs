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

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::features::backend::ApiRequest;
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
}

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

    /// JS injected via `Page.addScriptToEvaluateOnNewDocument` that wires up
    /// PerformanceObservers for LCP, INP (Event Timing) and CLS, and exposes
    /// `window.__thorCollectWebVitals()` returning a flat JSON payload that
    /// [`CdpBridge::parse_web_vitals`] can parse. Kept dependency-free so it can
    /// run on any page without shipping the web-vitals library.
    pub fn web_vitals_injection_script() -> &'static str {
        WEB_VITALS_SCRIPT
    }

    /// Parse the Core Web Vitals JSON produced by the injection script (or any
    /// `Runtime.evaluate` collection) into real [`BrowserMetrics`]. Delegates to
    /// the canonical lenient parser in `devtools::cdp_integration`.
    pub fn parse_web_vitals(json: &str) -> Option<BrowserMetrics> {
        crate::devtools::cdp_integration::parse_web_vitals(json)
    }

    /// Ingest one captured network exchange (waterfall / cache / third-party).
    pub fn ingest_network_entry(&self, entry: NetworkEntry) {
        self.entries.lock().push(entry);
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
  const v = { lcp: 0, inp: 0, cls: 0, fcp: 0 };
  window.__thorWebVitals = v;
  try {
    new PerformanceObserver((l) => {
      const es = l.getEntries();
      const last = es[es.length - 1];
      if (last) v.lcp = last.renderTime || last.loadTime || last.startTime || v.lcp;
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
  window.__thorCollectWebVitals = () => {
    const nav = performance.getEntriesByType('navigation')[0] || {};
    return JSON.stringify({
      lcp: v.lcp,
      inp: v.inp,
      cls: v.cls,
      fcp: v.fcp,
      ttfb: nav.responseStart || 0,
      domContentLoaded: nav.domContentLoadedEventEnd || 0,
      domInteractive: nav.domInteractive || 0
    });
  };
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_script() {
        assert!(CdpBridge::web_vitals_injection_script().contains("largest-contentful-paint"));
        assert!(CdpBridge::web_vitals_injection_script().contains("__thorCollectWebVitals"));
        let m = CdpBridge::parse_web_vitals(r#"{"lcp": 1500.0, "cls": 0.05, "ttfb": 300.0}"#)
            .expect("vitals");
        assert!((m.lcp_ms - 1500.0).abs() < 1e-6);
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
