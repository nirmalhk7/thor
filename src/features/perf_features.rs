//! Performance specialist features (12).
//!
//! Budgets, trace diff, waterfall forensics, memory-leak detection, synthetic
//! monitoring, RUM-vs-lab, HDR histogram helper, INP attribution, CDN cache
//! analysis, vitals-under-load, flame-graph folding, and third-party
//! attribution. Reuses [`crate::load::LatencyHistogram`],
//! [`crate::load::PerformanceBudgets`] and [`crate::load::BrowserMetrics`].

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::load::{BrowserMetrics, LatencyHistogram, LoadMetrics, PerformanceBudgets};

// ---------------------------------------------------------------------------
// 1. Perf Budgets
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetViolation {
    pub metric: String,
    pub actual: f64,
    pub budget: f64,
    pub over_by: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetReport {
    pub passed: bool,
    pub violations: Vec<BudgetViolation>,
    pub checked: usize,
}

// ---------------------------------------------------------------------------
// 2. Trace Diff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDelta {
    pub metric: String,
    pub before: f64,
    pub after: f64,
    pub delta_pct: f64,
    pub kind: String, // "regression" | "improvement" | "stable"
}

// ---------------------------------------------------------------------------
// 3. Waterfall Forensics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTiming {
    pub url: String,
    pub start_ms: f64,
    pub dns_ms: f64,
    pub connect_ms: f64,
    pub ttfb_ms: f64,
    pub download_ms: f64,
    #[serde(default)]
    pub render_blocking: bool,
    #[serde(default)]
    pub bytes: u64,
}

impl ResourceTiming {
    pub fn total_ms(&self) -> f64 {
        self.dns_ms + self.connect_ms + self.ttfb_ms + self.download_ms
    }
    pub fn end_ms(&self) -> f64 {
        self.start_ms + self.total_ms()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterfallReport {
    pub total_ms: f64,
    pub slowest: Vec<String>,
    pub render_blocking: Vec<String>,
    pub total_bytes: u64,
}

// ---------------------------------------------------------------------------
// 4. Memory Leak Hunter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapSample {
    pub timestamp_ms: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakReport {
    /// Growth in bytes per second (least-squares slope).
    pub slope_bytes_per_sec: f64,
    pub r_squared: f64,
    pub leaking: bool,
    pub projected_bytes_1h: f64,
}

// ---------------------------------------------------------------------------
// 5. Synthetic Monitor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticCheck {
    pub target: String,
    pub success: bool,
    pub latency_ms: f64,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticSummary {
    pub target: String,
    pub checks: usize,
    pub uptime_pct: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
}

// ---------------------------------------------------------------------------
// 6. RUM vs Lab
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RumLabDivergence {
    pub metric: String,
    pub lab: f64,
    pub rum: f64,
    pub divergence_pct: f64,
}

// ---------------------------------------------------------------------------
// 8. INP Attribution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionEvent {
    pub target: String,
    pub input_delay_ms: f64,
    pub processing_ms: f64,
    pub presentation_ms: f64,
}

impl InteractionEvent {
    pub fn total_ms(&self) -> f64 {
        self.input_delay_ms + self.processing_ms + self.presentation_ms
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InpReport {
    pub inp_ms: f64,
    pub worst_target: String,
    pub dominant_phase: String,
    pub rating: String,
}

// ---------------------------------------------------------------------------
// 9. CDN Cache
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub url: String,
    #[serde(default)]
    pub cache_control: Option<String>,
    #[serde(default)]
    pub age_secs: Option<u64>,
    #[serde(default)]
    pub x_cache: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheReport {
    pub hit_ratio: f64,
    pub uncacheable: Vec<String>,
    pub misconfigured: Vec<String>,
}

// ---------------------------------------------------------------------------
// 11. Flame Graph
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlameNode {
    pub frame: String,
    pub value: u64,
    pub children: Vec<FlameNode>,
}

// ---------------------------------------------------------------------------
// 12. Third-Party Attribution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyEntry {
    pub domain: String,
    pub requests: usize,
    pub bytes: u64,
    pub blocking_ms: f64,
}

/// Performance specialist engine.
pub struct PerfEngine {
    synthetic: Mutex<Vec<SyntheticCheck>>,
}

impl Default for PerfEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfEngine {
    pub fn new() -> Self {
        Self {
            synthetic: Mutex::new(Vec::new()),
        }
    }

    // 1. Perf Budgets -------------------------------------------------------
    pub fn check_budgets(
        &self,
        budgets: &PerformanceBudgets,
        vitals: Option<&BrowserMetrics>,
        load: Option<&LoadMetrics>,
    ) -> BudgetReport {
        let mut violations = Vec::new();
        let mut checked = 0;
        let mut check =
            |name: &str, actual: Option<f64>, budget: Option<f64>, lower_is_better: bool| {
                if let (Some(a), Some(b)) = (actual, budget) {
                    checked += 1;
                    let over = if lower_is_better { a - b } else { b - a };
                    if over > 0.0 {
                        violations.push(BudgetViolation {
                            metric: name.to_string(),
                            actual: a,
                            budget: b,
                            over_by: over,
                        });
                    }
                }
            };
        if let Some(v) = vitals {
            check("lcp_ms", Some(v.lcp_ms), budgets.max_lcp_ms, true);
            check("inp_ms", Some(v.inp_ms), budgets.max_inp_ms, true);
            check("cls", Some(v.cls), budgets.max_cls, true);
            check("tbt_ms", Some(v.tbt_ms), budgets.max_tbt_ms, true);
            check("ttfb_ms", Some(v.ttfb_ms), budgets.max_ttfb_ms, true);
        }
        if let Some(l) = load {
            check(
                "p95_latency_ms",
                Some(l.p95_latency_ms),
                budgets.max_p95_latency_ms,
                true,
            );
            check(
                "p99_latency_ms",
                Some(l.p99_latency_ms),
                budgets.max_p99_latency_ms,
                true,
            );
            let error_rate = if l.total_requests > 0 {
                l.failed_requests as f64 / l.total_requests as f64
            } else {
                0.0
            };
            check("error_rate", Some(error_rate), budgets.max_error_rate, true);
            check(
                "rps",
                budgets.min_rps.map(|_| l.rps),
                budgets.min_rps,
                false,
            );
        }
        BudgetReport {
            passed: violations.is_empty(),
            violations,
            checked,
        }
    }

    // 2. Trace Diff ---------------------------------------------------------
    pub fn diff_traces(
        &self,
        before: &BTreeMap<String, f64>,
        after: &BTreeMap<String, f64>,
        regression_threshold_pct: f64,
    ) -> Vec<MetricDelta> {
        let mut keys: Vec<&String> = before.keys().chain(after.keys()).collect();
        keys.sort();
        keys.dedup();
        let mut deltas = Vec::new();
        for k in keys {
            let b = *before.get(k).unwrap_or(&0.0);
            let a = *after.get(k).unwrap_or(&0.0);
            let delta_pct = if b.abs() > f64::EPSILON {
                (a - b) / b * 100.0
            } else if a != 0.0 {
                100.0
            } else {
                0.0
            };
            let kind = if delta_pct > regression_threshold_pct {
                "regression"
            } else if delta_pct < -regression_threshold_pct {
                "improvement"
            } else {
                "stable"
            };
            deltas.push(MetricDelta {
                metric: k.clone(),
                before: b,
                after: a,
                delta_pct,
                kind: kind.to_string(),
            });
        }
        deltas
    }

    // 3. Waterfall Forensics -----------------------------------------------
    pub fn analyze_waterfall(&self, resources: &[ResourceTiming]) -> WaterfallReport {
        let total_ms = resources.iter().map(|r| r.end_ms()).fold(0.0, f64::max);
        let mut by_dur: Vec<&ResourceTiming> = resources.iter().collect();
        by_dur.sort_by(|a, b| {
            b.total_ms()
                .partial_cmp(&a.total_ms())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let slowest = by_dur
            .iter()
            .take(5)
            .map(|r| format!("{} ({:.0}ms)", r.url, r.total_ms()))
            .collect();
        let render_blocking = resources
            .iter()
            .filter(|r| r.render_blocking)
            .map(|r| r.url.clone())
            .collect();
        let total_bytes = resources.iter().map(|r| r.bytes).sum();
        WaterfallReport {
            total_ms,
            slowest,
            render_blocking,
            total_bytes,
        }
    }

    // 4. Memory Leak Hunter -------------------------------------------------
    pub fn detect_leak(&self, samples: &[HeapSample]) -> LeakReport {
        if samples.len() < 2 {
            return LeakReport {
                slope_bytes_per_sec: 0.0,
                r_squared: 0.0,
                leaking: false,
                projected_bytes_1h: 0.0,
            };
        }
        // Least-squares linear regression of used_bytes over seconds.
        let t0 = samples[0].timestamp_ms;
        let xs: Vec<f64> = samples
            .iter()
            .map(|s| (s.timestamp_ms - t0) as f64 / 1000.0)
            .collect();
        let ys: Vec<f64> = samples.iter().map(|s| s.used_bytes as f64).collect();
        let n = xs.len() as f64;
        let sx: f64 = xs.iter().sum();
        let sy: f64 = ys.iter().sum();
        let sxx: f64 = xs.iter().map(|x| x * x).sum();
        let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| x * y).sum();
        let denom = n * sxx - sx * sx;
        let slope = if denom.abs() > f64::EPSILON {
            (n * sxy - sx * sy) / denom
        } else {
            0.0
        };
        let intercept = (sy - slope * sx) / n;

        // Coefficient of determination.
        let mean_y = sy / n;
        let ss_tot: f64 = ys.iter().map(|y| (y - mean_y).powi(2)).sum();
        let ss_res: f64 = xs
            .iter()
            .zip(&ys)
            .map(|(x, y)| (y - (slope * x + intercept)).powi(2))
            .sum();
        let r_squared = if ss_tot > f64::EPSILON {
            1.0 - ss_res / ss_tot
        } else {
            0.0
        };

        // Leak = sustained positive growth with good linear fit.
        let leaking = slope > 1024.0 && r_squared > 0.8;
        LeakReport {
            slope_bytes_per_sec: slope,
            r_squared,
            leaking,
            projected_bytes_1h: slope * 3600.0,
        }
    }

    // 5. Synthetic Monitor --------------------------------------------------
    pub fn record_synthetic(&self, check: SyntheticCheck) {
        self.synthetic.lock().push(check);
    }

    pub fn synthetic_summary(&self, target: &str) -> SyntheticSummary {
        let checks: Vec<SyntheticCheck> = self
            .synthetic
            .lock()
            .iter()
            .filter(|c| c.target == target)
            .cloned()
            .collect();
        let count = checks.len();
        if count == 0 {
            return SyntheticSummary {
                target: target.to_string(),
                checks: 0,
                uptime_pct: 0.0,
                avg_latency_ms: 0.0,
                p95_latency_ms: 0.0,
            };
        }
        let successes = checks.iter().filter(|c| c.success).count();
        let uptime_pct = successes as f64 / count as f64 * 100.0;
        let avg = checks.iter().map(|c| c.latency_ms).sum::<f64>() / count as f64;
        let mut hist = LatencyHistogram::default();
        for c in &checks {
            hist.record(c.latency_ms);
        }
        SyntheticSummary {
            target: target.to_string(),
            checks: count,
            uptime_pct,
            avg_latency_ms: avg,
            p95_latency_ms: hist.percentile(95.0),
        }
    }

    // 6. RUM vs Lab ---------------------------------------------------------
    pub fn rum_vs_lab(&self, lab: &BrowserMetrics, rum: &BrowserMetrics) -> Vec<RumLabDivergence> {
        let pairs = [
            ("lcp_ms", lab.lcp_ms, rum.lcp_ms),
            ("inp_ms", lab.inp_ms, rum.inp_ms),
            ("cls", lab.cls, rum.cls),
            ("ttfb_ms", lab.ttfb_ms, rum.ttfb_ms),
            ("fcp_ms", lab.fcp_ms, rum.fcp_ms),
        ];
        pairs
            .into_iter()
            .map(|(metric, l, r)| RumLabDivergence {
                metric: metric.to_string(),
                lab: l,
                rum: r,
                divergence_pct: if l.abs() > f64::EPSILON {
                    (r - l) / l * 100.0
                } else {
                    0.0
                },
            })
            .collect()
    }

    // 7. HDR Histogram Helper ----------------------------------------------
    pub fn histogram_from_samples(&self, samples: &[f64]) -> LatencyHistogram {
        let mut hist = LatencyHistogram::default();
        for s in samples {
            hist.record(*s);
        }
        hist
    }

    pub fn percentile_table(&self, hist: &LatencyHistogram) -> Vec<(f64, f64)> {
        [50.0, 75.0, 90.0, 95.0, 99.0, 99.9]
            .into_iter()
            .map(|q| (q, hist.percentile(q)))
            .collect()
    }

    // 8. INP Attribution ----------------------------------------------------
    pub fn attribute_inp(&self, interactions: &[InteractionEvent]) -> InpReport {
        if interactions.is_empty() {
            return InpReport {
                inp_ms: 0.0,
                worst_target: String::new(),
                dominant_phase: "none".into(),
                rating: "good".into(),
            };
        }
        // INP ≈ the worst (near-max) interaction latency.
        let worst = interactions
            .iter()
            .max_by(|a, b| {
                a.total_ms()
                    .partial_cmp(&b.total_ms())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        let inp_ms = worst.total_ms();
        let (dominant_phase, _) = [
            ("input-delay", worst.input_delay_ms),
            ("processing", worst.processing_ms),
            ("presentation", worst.presentation_ms),
        ]
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();
        let rating = if inp_ms <= 200.0 {
            "good"
        } else if inp_ms <= 500.0 {
            "needs-improvement"
        } else {
            "poor"
        };
        InpReport {
            inp_ms,
            worst_target: worst.target.clone(),
            dominant_phase: dominant_phase.to_string(),
            rating: rating.to_string(),
        }
    }

    // 9. CDN Cache ----------------------------------------------------------
    pub fn analyze_cache(&self, entries: &[CacheEntry]) -> CacheReport {
        let mut hits = 0;
        let mut hit_eligible = 0;
        let mut uncacheable = Vec::new();
        let mut misconfigured = Vec::new();
        for e in entries {
            let cc = e.cache_control.as_deref().unwrap_or("").to_lowercase();
            let is_uncacheable = cc.contains("no-store")
                || cc.contains("no-cache")
                || cc.contains("max-age=0")
                || cc.is_empty();
            if is_uncacheable {
                uncacheable.push(e.url.clone());
            }
            if let Some(xc) = &e.x_cache {
                hit_eligible += 1;
                if xc.to_uppercase().contains("HIT") {
                    hits += 1;
                }
            }
            // Served as a HIT but marked uncacheable => misconfiguration.
            if e.x_cache
                .as_deref()
                .map(|x| x.to_uppercase().contains("HIT"))
                .unwrap_or(false)
                && is_uncacheable
            {
                misconfigured.push(format!(
                    "{} served from cache but Cache-Control is non-cacheable",
                    e.url
                ));
            }
            // Age exceeds max-age.
            if let (Some(age), Some(max_age)) = (e.age_secs, parse_max_age(&cc)) {
                if age > max_age {
                    misconfigured.push(format!("{} Age {age}s exceeds max-age {max_age}s", e.url));
                }
            }
        }
        let hit_ratio = if hit_eligible > 0 {
            hits as f64 / hit_eligible as f64
        } else {
            0.0
        };
        CacheReport {
            hit_ratio,
            uncacheable,
            misconfigured,
        }
    }

    // 10. Vitals Under Load -------------------------------------------------
    /// Find the concurrency "knee" where LCP first crosses the budget as load
    /// increases. Samples are `(concurrency, lcp_ms)`.
    pub fn vitals_knee(&self, samples: &[(u32, f64)], lcp_budget_ms: f64) -> Option<u32> {
        let mut sorted: Vec<(u32, f64)> = samples.to_vec();
        sorted.sort_by_key(|(c, _)| *c);
        sorted
            .into_iter()
            .find(|(_, lcp)| *lcp > lcp_budget_ms)
            .map(|(c, _)| c)
    }

    // 11. Flame Graph -------------------------------------------------------
    /// Build a flame graph tree from folded stacks (`a;b;c value`).
    pub fn build_flame_graph(&self, folded: &[String]) -> FlameNode {
        let mut root = FlameNode {
            frame: "root".to_string(),
            value: 0,
            children: Vec::new(),
        };
        for line in folded {
            let (stack, value) = match line.rsplit_once(' ') {
                Some((s, v)) => (s, v.trim().parse::<u64>().unwrap_or(0)),
                None => continue,
            };
            root.value += value;
            let mut node = &mut root;
            for frame in stack.split(';').filter(|f| !f.is_empty()) {
                let idx = match node.children.iter().position(|c| c.frame == frame) {
                    Some(i) => i,
                    None => {
                        node.children.push(FlameNode {
                            frame: frame.to_string(),
                            value: 0,
                            children: Vec::new(),
                        });
                        node.children.len() - 1
                    }
                };
                node = &mut node.children[idx];
                node.value += value;
            }
        }
        root
    }

    // 12. Third-Party Attribution ------------------------------------------
    pub fn attribute_third_party(
        &self,
        first_party: &str,
        resources: &[ResourceTiming],
    ) -> Vec<ThirdPartyEntry> {
        let fp = registrable_domain(first_party);
        let mut map: BTreeMap<String, ThirdPartyEntry> = BTreeMap::new();
        for r in resources {
            let host = url_host(&r.url);
            let domain = registrable_domain(&host);
            if domain == fp || domain.is_empty() {
                continue;
            }
            let entry = map.entry(domain.clone()).or_insert(ThirdPartyEntry {
                domain,
                requests: 0,
                bytes: 0,
                blocking_ms: 0.0,
            });
            entry.requests += 1;
            entry.bytes += r.bytes;
            if r.render_blocking {
                entry.blocking_ms += r.total_ms();
            }
        }
        let mut out: Vec<ThirdPartyEntry> = map.into_values().collect();
        out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        out
    }
}

fn parse_max_age(cache_control: &str) -> Option<u64> {
    cache_control
        .split(',')
        .find_map(|d| d.trim().strip_prefix("max-age="))
        .and_then(|v| v.trim().parse::<u64>().ok())
}

fn url_host(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    after_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .split('@')
        .last()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Approximate registrable domain (last two labels). Good enough for
/// first-vs-third-party classification of common hosts.
fn registrable_domain(host_or_url: &str) -> String {
    let host = if host_or_url.contains("://") {
        url_host(host_or_url)
    } else {
        host_or_url.to_string()
    };
    let labels: Vec<&str> = host.split('.').filter(|s| !s.is_empty()).collect();
    if labels.len() >= 2 {
        format!("{}.{}", labels[labels.len() - 2], labels[labels.len() - 1])
    } else {
        host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vitals(lcp: f64, inp: f64, cls: f64) -> BrowserMetrics {
        BrowserMetrics {
            lcp_ms: lcp,
            inp_ms: inp,
            cls,
            ..Default::default()
        }
    }

    #[test]
    fn test_budget_check() {
        let e = PerfEngine::new();
        let budgets = PerformanceBudgets::core_web_vitals();
        let good = vitals(2000.0, 150.0, 0.05);
        assert!(e.check_budgets(&budgets, Some(&good), None).passed);
        let bad = vitals(4000.0, 300.0, 0.3);
        let r = e.check_budgets(&budgets, Some(&bad), None);
        assert!(!r.passed);
        assert!(r.violations.iter().any(|v| v.metric == "lcp_ms"));
    }

    #[test]
    fn test_trace_diff() {
        let e = PerfEngine::new();
        let mut before = BTreeMap::new();
        before.insert("lcp".to_string(), 1000.0);
        before.insert("cls".to_string(), 0.1);
        let mut after = BTreeMap::new();
        after.insert("lcp".to_string(), 1500.0);
        after.insert("cls".to_string(), 0.1);
        let deltas = e.diff_traces(&before, &after, 10.0);
        let lcp = deltas.iter().find(|d| d.metric == "lcp").unwrap();
        assert_eq!(lcp.kind, "regression");
        assert!((lcp.delta_pct - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_memory_leak_detection() {
        let e = PerfEngine::new();
        let samples: Vec<HeapSample> = (0..20)
            .map(|i| HeapSample {
                timestamp_ms: i * 1000,
                used_bytes: 10_000_000 + i * 5_000_000,
            })
            .collect();
        let r = e.detect_leak(&samples);
        assert!(r.leaking);
        assert!(r.slope_bytes_per_sec > 4_000_000.0);
        assert!(r.r_squared > 0.99);
    }

    #[test]
    fn test_no_leak_flat_memory() {
        let e = PerfEngine::new();
        let samples: Vec<HeapSample> = (0..20)
            .map(|i| HeapSample {
                timestamp_ms: i * 1000,
                used_bytes: 10_000_000,
            })
            .collect();
        assert!(!e.detect_leak(&samples).leaking);
    }

    #[test]
    fn test_synthetic_summary() {
        let e = PerfEngine::new();
        for i in 0..10 {
            e.record_synthetic(SyntheticCheck {
                target: "api".into(),
                success: i != 0,
                latency_ms: 100.0 + i as f64,
                timestamp_ms: i,
            });
        }
        let s = e.synthetic_summary("api");
        assert_eq!(s.checks, 10);
        assert!((s.uptime_pct - 90.0).abs() < 1e-6);
        assert!(s.p95_latency_ms > 0.0);
    }

    #[test]
    fn test_inp_attribution() {
        let e = PerfEngine::new();
        let events = vec![
            InteractionEvent {
                target: "#a".into(),
                input_delay_ms: 10.0,
                processing_ms: 20.0,
                presentation_ms: 5.0,
            },
            InteractionEvent {
                target: "#b".into(),
                input_delay_ms: 50.0,
                processing_ms: 480.0,
                presentation_ms: 30.0,
            },
        ];
        let r = e.attribute_inp(&events);
        assert_eq!(r.worst_target, "#b");
        assert_eq!(r.dominant_phase, "processing");
        assert_eq!(r.rating, "poor"); // 560ms > 500ms
    }

    #[test]
    fn test_cache_analysis() {
        let e = PerfEngine::new();
        let entries = vec![
            CacheEntry {
                url: "/a.js".into(),
                cache_control: Some("public, max-age=3600".into()),
                age_secs: Some(100),
                x_cache: Some("HIT".into()),
            },
            CacheEntry {
                url: "/b.js".into(),
                cache_control: Some("no-store".into()),
                age_secs: None,
                x_cache: Some("MISS".into()),
            },
        ];
        let r = e.analyze_cache(&entries);
        assert!((r.hit_ratio - 0.5).abs() < 1e-6);
        assert!(r.uncacheable.contains(&"/b.js".to_string()));
    }

    #[test]
    fn test_vitals_knee() {
        let e = PerfEngine::new();
        let samples = vec![(1, 1500.0), (10, 2000.0), (50, 2600.0), (100, 3500.0)];
        assert_eq!(e.vitals_knee(&samples, 2500.0), Some(50));
    }

    #[test]
    fn test_flame_graph() {
        let e = PerfEngine::new();
        let folded = vec![
            "main;a;b 10".to_string(),
            "main;a;c 5".to_string(),
            "main;a 3".to_string(),
        ];
        let root = e.build_flame_graph(&folded);
        assert_eq!(root.value, 18);
        let main = root.children.iter().find(|c| c.frame == "main").unwrap();
        let a = main.children.iter().find(|c| c.frame == "a").unwrap();
        assert_eq!(a.value, 18);
        assert!(a.children.iter().any(|c| c.frame == "b" && c.value == 10));
    }

    #[test]
    fn test_third_party_attribution() {
        let e = PerfEngine::new();
        let resources = vec![
            ResourceTiming {
                url: "https://cdn.mysite.com/app.js".into(),
                start_ms: 0.0,
                dns_ms: 1.0,
                connect_ms: 1.0,
                ttfb_ms: 10.0,
                download_ms: 5.0,
                render_blocking: true,
                bytes: 1000,
            },
            ResourceTiming {
                url: "https://www.google-analytics.com/ga.js".into(),
                start_ms: 0.0,
                dns_ms: 5.0,
                connect_ms: 5.0,
                ttfb_ms: 50.0,
                download_ms: 20.0,
                render_blocking: true,
                bytes: 5000,
            },
        ];
        let tp = e.attribute_third_party("https://www.mysite.com", &resources);
        assert_eq!(tp.len(), 1);
        assert_eq!(tp[0].domain, "google-analytics.com");
        assert_eq!(tp[0].bytes, 5000);
    }
}
