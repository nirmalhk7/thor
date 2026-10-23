//! Passive Visit Recon — observe the active tab after navigation.
//!
//! Read-only: DOM inventory + document.cookie + storage keys via page evaluate.
//! Composes existing SecurityEngine / TechStackDetector analyzers.
//! Never mutates the page, never fans out HTTP to other hosts, never calls
//! consent-gated labs (fuzz / SSRF / fixation / IDOR).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::dx::stack_detector::{DetectedStack, TechStackDetector};
use crate::engine::tab_session::TabSession;
use crate::features::security::{
    CookieJarEntry, HeaderReport, SecurityEngine, StorageFinding, StorageItem, TlsInfo, TlsReport,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitReconMeta {
    pub title: String,
    pub description: Option<String>,
    pub generator: Option<String>,
    pub canonical: Option<String>,
    pub robots: Option<String>,
    pub viewport: Option<String>,
    pub author: Option<String>,
    pub theme_color: Option<String>,
    pub icons: Vec<String>,
    pub open_graph: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitDomStats {
    pub total_nodes: usize,
    pub iframes: usize,
    pub stylesheets: usize,
    pub images: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitReconForm {
    pub action: String,
    pub method: String,
    pub has_password: bool,
    pub input_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitReconThirdParty {
    pub host: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitBrowserCapabilities {
    pub has_webgl: bool,
    pub has_webgpu: bool,
    pub has_wasm: bool,
    pub has_service_worker: bool,
    pub has_web_workers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitSecurityHeadersDetail {
    pub csp_directives: BTreeMap<String, String>,
    pub cors_mode: String,
    pub hsts_max_age: Option<u64>,
    pub hsts_include_subdomains: bool,
    pub hsts_preload: bool,
    pub http_only_cookies: usize,
    pub secure_cookies: usize,
    pub same_site_strict_cookies: usize,
    pub same_site_lax_cookies: usize,
    pub same_site_none_cookies: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitStructuredData {
    pub json_ld_types: Vec<String>,
    pub twitter_cards: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisitReconReport {
    pub url: String,
    pub captured_at_ms: u64,
    pub passive: bool,
    pub stack: Option<DetectedStack>,
    pub meta: VisitReconMeta,
    pub dom_stats: Option<VisitDomStats>,
    pub capabilities: Option<VisitBrowserCapabilities>,
    pub security_details: Option<VisitSecurityHeadersDetail>,
    pub structured_data: Option<VisitStructuredData>,
    pub headers: Option<HeaderReport>,
    pub header_map: BTreeMap<String, String>,
    pub tls: Option<TlsReport>,
    pub cookie_issues: Vec<String>,
    pub session_cookies: usize,
    pub storage_findings: Vec<StorageFinding>,
    pub forms: Vec<VisitReconForm>,
    pub scripts: Vec<String>,
    pub service_workers: Vec<String>,
    pub third_parties: Vec<VisitReconThirdParty>,
    pub mixed_content: Vec<String>,
    pub findings: Vec<String>,
    pub grade: String,
}

/// Orchestrates passive capture + offline analysis for one navigation.
pub struct VisitReconEngine;

impl VisitReconEngine {
    /// JS probe: inventory only (no network fan-out, no form submit).
    pub fn capture_script() -> &'static str {
        r#"(async function() {
  const abs = (u) => { try { return new URL(u, location.href).href; } catch(_) { return u || ''; } };
  const pageHost = location.hostname;
  const meta = {
    title: document.title || '',
    description: (document.querySelector('meta[name="description"]')||{}).content || null,
    generator: (document.querySelector('meta[name="generator"]')||{}).content || null,
    canonical: (document.querySelector('link[rel="canonical"]')||{}).href || null,
    robots: (document.querySelector('meta[name="robots"]')||{}).content || null,
    viewport: (document.querySelector('meta[name="viewport"]')||{}).content || null,
    author: (document.querySelector('meta[name="author"]')||{}).content || null,
    theme_color: (document.querySelector('meta[name="theme-color"]')||{}).content || null,
    icons: Array.from(document.querySelectorAll('link[rel*="icon"]')).map(l => l.href).slice(0, 12),
    open_graph: {}
  };
  document.querySelectorAll('meta[property^="og:"]').forEach(m => {
    if (m.getAttribute('property') && m.content) meta.open_graph[m.getAttribute('property')] = m.content;
  });
  const globals = {
    react: !!(window.React || window.__REACT_DEVTOOLS_GLOBAL_HOOK__ || document.querySelector('[data-reactroot]')),
    vue: !!(window.Vue || window.__VUE__ || document.querySelector('[data-v-]')),
    next: !!(window.__NEXT_DATA__),
    nuxt: !!(window.__NUXT__),
    angular: !!(window.angular || document.querySelector('[ng-app],[ng-version]')),
    svelte: !!(window.__svelte),
    jquery: !!(window.jQuery || window.$),
    alpine: !!(window.Alpine),
    htmx: !!(window.htmx),
    ethereum: !!(window.ethereum)
  };
  const dom_stats = {
    total_nodes: document.getElementsByTagName('*').length,
    iframes: document.querySelectorAll('iframe').length,
    stylesheets: document.querySelectorAll('link[rel="stylesheet"]').length,
    images: document.images.length
  };
  const headers = {};
  // Performance resource entries (already loaded) — no extra fetches.
  const resources = (performance.getEntriesByType && performance.getEntriesByType('resource')) || [];
  const third = {};
  const mixed = [];
  const scripts = [];
  resources.forEach(r => {
    try {
      const u = new URL(r.name);
      if (u.hostname && u.hostname !== pageHost) {
        third[u.hostname] = (third[u.hostname] || 0) + 1;
      }
      if (location.protocol === 'https:' && u.protocol === 'http:') mixed.push(r.name);
      if (/\.js(\?|$)/i.test(u.pathname) || r.initiatorType === 'script') scripts.push(r.name);
    } catch(_) {}
  });
  Array.from(document.scripts).forEach(s => { if (s.src) scripts.push(abs(s.src)); });
  const forms = Array.from(document.forms).slice(0, 40).map(f => ({
    action: abs(f.getAttribute('action') || location.href),
    method: (f.method || 'get').toLowerCase(),
    has_password: !!f.querySelector('input[type="password"]'),
    input_names: Array.from(f.querySelectorAll('input[name]')).map(i => i.name).slice(0, 30)
  }));
  const localItems = [];
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      const v = localStorage.getItem(k) || '';
      localItems.push({ store: 'localStorage', key: k, value: v.slice(0, 256) });
    }
  } catch(_) {}
  const sessionItems = [];
  try {
    for (let i = 0; i < sessionStorage.length; i++) {
      const k = sessionStorage.key(i);
      const v = sessionStorage.getItem(k) || '';
      sessionItems.push({ store: 'sessionStorage', key: k, value: v.slice(0, 256) });
    }
  } catch(_) {}
  const cookies = (document.cookie || '').split(';').map(p => p.trim()).filter(Boolean).map(p => {
    const i = p.indexOf('=');
    const name = i >= 0 ? p.slice(0, i) : p;
    const value = i >= 0 ? p.slice(i + 1) : '';
    return {
      name,
      value: value.slice(0, 256),
      secure: location.protocol === 'https:',
      http_only: false,
      same_site: null,
      domain: pageHost,
      path: '/',
      max_age: null
    };
  });
  let serviceWorkers = [];
  try {
    if (navigator.serviceWorker && navigator.serviceWorker.getRegistrations) {
      const regs = await navigator.serviceWorker.getRegistrations();
      serviceWorkers = regs.map(r => (r.active && r.active.scriptURL) || (r.installing && r.installing.scriptURL) || r.scope).filter(Boolean);
    }
  } catch(_) {}
  const capabilities = {
    has_webgl: !!(window.WebGLRenderingContext || window.WebGL2RenderingContext),
    has_webgpu: !!(navigator && navigator.gpu),
    has_wasm: !!(window.WebAssembly),
    has_service_worker: !!(navigator && navigator.serviceWorker),
    has_web_workers: !!(window.Worker)
  };
  const twitter_cards = {};
  document.querySelectorAll('meta[name^="twitter:"]').forEach(m => {
    if (m.getAttribute('name') && m.content) twitter_cards[m.getAttribute('name')] = m.content;
  });
  const json_ld_types = [];
  document.querySelectorAll('script[type="application/ld+json"]').forEach(s => {
    try {
      const parsed = JSON.parse(s.textContent);
      if (parsed && parsed['@type']) {
        if (Array.isArray(parsed['@type'])) json_ld_types.push(...parsed['@type']);
        else json_ld_types.push(String(parsed['@type']));
      }
    } catch(_) {}
  });
  const structured_data = {
    json_ld_types: Array.from(new Set(json_ld_types)),
    twitter_cards
  };
  const html = (document.documentElement && document.documentElement.outerHTML)
    ? document.documentElement.outerHTML.slice(0, 120000) : '';
  const cspMeta = (document.querySelector('meta[http-equiv="Content-Security-Policy"]')||{}).content;
  if (cspMeta) headers['content-security-policy'] = cspMeta;
  return {
    url: location.href,
    html,
    meta,
    globals,
    dom_stats,
    capabilities,
    structured_data,
    headers,
    forms,
    scripts: Array.from(new Set(scripts)).slice(0, 80),
    third_parties: Object.keys(third).map(host => ({ host, count: third[host] })).sort((a,b)=>b.count-a.count).slice(0, 40),
    mixed_content: mixed.slice(0, 40),
    cookies,
    storage: localItems.concat(sessionItems).slice(0, 80),
    service_workers: serviceWorkers.slice(0, 20)
  };
})()"#
    }

    pub fn run_passive(session: &TabSession, url_hint: &str) -> VisitReconReport {
        let raw = session
            .evaluate_js_blocking(Self::capture_script())
            .unwrap_or_default();
        Self::analyze_capture(&raw, url_hint)
    }

    pub fn analyze_capture(raw: &serde_json::Value, url_hint: &str) -> VisitReconReport {
        let url = raw
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or(url_hint)
            .to_string();
        let html = raw.get("html").and_then(|v| v.as_str()).unwrap_or("");
        let mut header_map: BTreeMap<String, String> = BTreeMap::new();
        if let Some(obj) = raw.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    header_map.insert(k.to_lowercase(), s.to_string());
                }
            }
        }

        let sec = SecurityEngine::new();
        let headers = if header_map.is_empty() {
            None
        } else {
            Some(sec.analyze_headers(&header_map))
        };

        let stack = Some(TechStackDetector::detect_with_headers(
            &url,
            html,
            &header_map.clone().into_iter().collect(),
        ));

        let cookies: Vec<CookieJarEntry> = raw
            .get("cookies")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        // Redact cookie values before forensics storage in report (keep flags/names).
        let cookies_redacted: Vec<CookieJarEntry> = cookies
            .iter()
            .map(|c| {
                let mut c = c.clone();
                c.value = redact_value(&c.value);
                c
            })
            .collect();
        let cookie_rep = sec.cookie_session_forensics(&cookies_redacted);

        let mut storage: Vec<StorageItem> = raw
            .get("storage")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        for item in &mut storage {
            item.value = redact_value(&item.value);
        }
        let storage_findings = sec.audit_storage(&storage);

        let meta: VisitReconMeta = raw
            .get("meta")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let forms: Vec<VisitReconForm> = raw
            .get("forms")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let scripts: Vec<String> = raw
            .get("scripts")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let third_parties: Vec<VisitReconThirdParty> = raw
            .get("third_parties")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let mixed_content: Vec<String> = raw
            .get("mixed_content")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let service_workers: Vec<String> = raw
            .get("service_workers")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        // TLS: only if capture supplied security details (omit rather than fake).
        let tls = raw.get("tls").and_then(|v| {
            let info: TlsInfo = serde_json::from_value(v.clone()).ok()?;
            Some(sec.inspect_tls(&info))
        });

        let mut findings = Vec::new();
        if let Some(ref h) = headers {
            for f in &h.findings {
                if f.recommendation != "OK" {
                    findings.push(format!("header:{} — {}", f.header, f.recommendation));
                }
            }
        }
        findings.extend(cookie_rep.issues.iter().cloned());
        for s in &storage_findings {
            findings.push(format!("storage:{} [{}] {}", s.store, s.severity, s.detail));
        }
        if !mixed_content.is_empty() {
            findings.push(format!("mixed-content: {} http resources on https page", mixed_content.len()));
        }
        if forms.iter().any(|f| f.has_password && f.method == "get") {
            findings.push("form: password field submitted via GET".into());
        }
        if !service_workers.is_empty() {
            findings.push(format!("service-worker: {} registration(s)", service_workers.len()));
        }

        let grade = composite_grade(headers.as_ref(), &cookie_rep.issues, &storage_findings, &mixed_content);

        let dom_stats: Option<VisitDomStats> = raw
            .get("dom_stats")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let capabilities: Option<VisitBrowserCapabilities> = raw
            .get("capabilities")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let structured_data: Option<VisitStructuredData> = raw
            .get("structured_data")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        // Build security details from header_map & cookies
        let mut csp_directives = BTreeMap::new();
        if let Some(csp) = header_map.get("content-security-policy") {
            for part in csp.split(';') {
                let part = part.trim();
                if let Some(space_idx) = part.find(' ') {
                    let dir = part[..space_idx].trim().to_string();
                    let val = part[space_idx + 1..].trim().to_string();
                    csp_directives.insert(dir, val);
                } else if !part.is_empty() {
                    csp_directives.insert(part.to_string(), "".to_string());
                }
            }
        }

        let cors_mode = match header_map.get("access-control-allow-origin").map(|s| s.as_str()) {
            Some("*") => "Wildcard (*) Unlimited".into(),
            Some(o) => format!("Specific ({o})"),
            None => "Restricted (Same-Origin)".into(),
        };

        let hsts = header_map.get("strict-transport-security");
        let hsts_max_age = hsts.and_then(|h| {
            h.split(';')
                .find(|p| p.trim().starts_with("max-age="))
                .and_then(|p| p.trim()["max-age=".len()..].parse::<u64>().ok())
        });
        let hsts_include_subdomains = hsts.is_some_and(|h| h.to_lowercase().contains("includesubdomains"));
        let hsts_preload = hsts.map_or(false, |h| h.to_lowercase().contains("preload"));

        let http_only_cookies = cookies.iter().filter(|c| c.http_only).count();
        let secure_cookies = cookies.iter().filter(|c| c.secure).count();
        let same_site_strict_cookies = cookies.iter().filter(|c| c.same_site.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("strict"))).count();
        let same_site_lax_cookies = cookies.iter().filter(|c| c.same_site.as_deref().map_or(false, |s| s.eq_ignore_ascii_case("lax"))).count();
        let same_site_none_cookies = cookies.iter().filter(|c| c.same_site.as_deref().map_or(false, |s| s.eq_ignore_ascii_case("none"))).count();

        let security_details = Some(VisitSecurityHeadersDetail {
            csp_directives,
            cors_mode,
            hsts_max_age,
            hsts_include_subdomains,
            hsts_preload,
            http_only_cookies,
            secure_cookies,
            same_site_strict_cookies,
            same_site_lax_cookies,
            same_site_none_cookies,
        });

        VisitReconReport {
            url,
            captured_at_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            passive: true,
            stack,
            meta,
            dom_stats,
            capabilities,
            security_details,
            structured_data,
            headers,
            header_map,
            tls,
            cookie_issues: cookie_rep.issues,
            session_cookies: cookie_rep.session_cookies,
            storage_findings,
            forms,
            scripts,
            service_workers,
            third_parties,
            mixed_content,
            findings,
            grade,
        }
    }
}

fn composite_grade(
    headers: Option<&HeaderReport>,
    cookie_issues: &[String],
    storage: &[StorageFinding],
    mixed: &[String],
) -> String {
    let mut score = headers.map(|h| h.score as i64).unwrap_or(70);
    score -= (cookie_issues.len() as i64) * 3;
    score -= storage
        .iter()
        .filter(|s| s.severity == "high")
        .count() as i64
        * 5;
    if !mixed.is_empty() {
        score -= 15;
    }
    score = score.clamp(0, 100);
    match score {
        90..=100 => "A".into(),
        80..=89 => "B".into(),
        70..=79 => "C".into(),
        60..=69 => "D".into(),
        _ => "F".into(),
    }
}

fn redact_value(v: &str) -> String {
    let trimmed = v.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Keep shape for JWT-ish / long secrets without leaking full token.
    if trimmed.len() > 12 {
        format!("{}…[REDACTED]", &trimmed[..4.min(trimmed.len())])
    } else {
        "[REDACTED]".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_capture_from_fixture_is_passive() {
        let raw = serde_json::json!({
            "url": "https://example.com/app",
            "html": "<html><head><meta name=\"generator\" content=\"Next.js\"></head><body><div id=\"__NEXT_DATA__\"></div><form method=\"post\"><input type=\"password\" name=\"pw\"></form><script src=\"https://cdn.example.net/a.js\"></script></body></html>",
            "meta": { "title": "Example", "generator": "Next.js", "open_graph": {}, "icons": [] },
            "headers": { "content-security-policy": "default-src 'self'", "strict-transport-security": "max-age=31536000" },
            "forms": [{ "action": "https://example.com/login", "method": "post", "has_password": true, "input_names": ["pw"] }],
            "scripts": ["https://cdn.example.net/a.js"],
            "third_parties": [{ "host": "cdn.example.net", "count": 1 }],
            "mixed_content": [],
            "cookies": [{ "name": "session", "value": "abc", "secure": true, "http_only": false, "same_site": "Lax", "domain": "example.com", "path": "/", "max_age": null }],
            "storage": [{ "store": "localStorage", "key": "auth_token", "value": "secret-value" }],
            "service_workers": []
        });
        let report = VisitReconEngine::analyze_capture(&raw, "https://example.com/app");
        assert!(report.passive);
        assert_eq!(report.url, "https://example.com/app");
        assert!(report.stack.as_ref().unwrap().badges.iter().any(|b| b.name.contains("Next")));
        assert!(report.headers.is_some());
        assert!(!report.cookie_issues.is_empty()); // session missing HttpOnly
        assert!(!report.storage_findings.is_empty());
        assert!(!report.grade.is_empty());
        assert!(!report.findings.is_empty());
    }

    #[test]
    fn capture_script_mentions_passive_surfaces() {
        let s = VisitReconEngine::capture_script();
        assert!(s.contains("localStorage"));
        assert!(s.contains("serviceWorker"));
        assert!(s.contains("getEntriesByType"));
        assert!(!s.contains("fetch(")); // no fan-out
    }
}
