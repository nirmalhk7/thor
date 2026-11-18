//! Performance specialist features (13 + 12 wave-2).
//!
//! Budgets, trace diff, waterfall forensics, memory-leak detection, synthetic
//! monitoring, RUM-vs-lab, HDR histogram helper, INP attribution, CDN cache
//! analysis, vitals-under-load, flame-graph folding, third-party attribution,
//! plus LCP element forensics, long-task radar, main-thread block map, bfcache
//! probe, freeze/resume bench, soft-nav vitals, speculation lab, connection
//! pool inspector, HTTP/3 path audit, compression budget, media weight gate,
//! and continuous profiler. Reuses [`crate::load::LatencyHistogram`],
//! [`crate::load::PerformanceBudgets`] and [`crate::load::BrowserMetrics`].

mod bfcache;
mod budgets;
mod cache;
mod compression;
mod connections;
mod freeze;
mod lcp;
mod media;
mod memory;
mod profiles;
mod speculation;
mod synthetic;
mod tasks;
mod third_party;
mod traces;
mod vitals;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::load::{
    BrowserMetrics, BudgetEvaluation, LatencyHistogram, LoadMetrics, PerformanceBudgets,
};
pub use crate::load::{SoftNavRecord, SoftNavVitalsReport};

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

/// A resource with its initiator, for blocking-chain (request dependency)
/// reconstruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiatedResource {
    pub url: String,
    #[serde(default)]
    pub initiator: Option<String>,
    pub start_ms: f64,
    pub total_ms: f64,
    #[serde(default)]
    pub render_blocking: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingChain {
    /// Ordered URLs from a root request down to the deepest dependent.
    pub chain: Vec<String>,
    pub cumulative_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockingChainReport {
    /// initiator url -> resources it initiated.
    pub edges: BTreeMap<String, Vec<String>>,
    pub longest_chain: BlockingChain,
    pub critical_path_ms: f64,
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
    /// Status-only seed from a recorded session (no measured RTT). Excluded
    /// from latency avg/p95 used as SLA signal.
    #[serde(default)]
    pub synthetic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyntheticSummary {
    pub target: String,
    pub checks: usize,
    pub uptime_pct: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    /// How many checks were status-only / unmeasured and excluded from latency.
    #[serde(default)]
    pub synthetic_excluded: usize,
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

// ---------------------------------------------------------------------------
// 13. LCP Element Forensics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LcpElementPayload {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub element_type: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub render_ms: Option<f64>,
    #[serde(default)]
    pub load_ms: Option<f64>,
    #[serde(default)]
    pub discover_ms: Option<f64>,
    /// Optional raw CDP / PerformanceObserver vitals JSON (keys: lcp, lcpElement…).
    #[serde(default)]
    pub vitals_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcpPhaseBreakdown {
    pub discover_ms: f64,
    pub load_ms: f64,
    pub render_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcpElementReport {
    pub url: String,
    pub element_type: String,
    pub size_bytes: u64,
    pub phases: LcpPhaseBreakdown,
    pub rating: String,
}

// ---------------------------------------------------------------------------
// 14. Long Task Radar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTaskEvent {
    pub duration_ms: f64,
    pub start_ms: f64,
    #[serde(default)]
    pub script_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// True when this longtask/LoAF overlapped an INP interaction window.
    #[serde(default)]
    pub inp_overlap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTaskRank {
    pub duration_ms: f64,
    pub script_url: String,
    pub name: String,
    pub inp_overlap: bool,
    pub blocking_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LongTaskRadarReport {
    pub ranked: Vec<LongTaskRank>,
    pub total_blocking_ms: f64,
    pub inp_overlapping: usize,
}

// ---------------------------------------------------------------------------
// 15. Main-Thread Block Map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCategorySlice {
    pub category: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainThreadBucket {
    pub bucket: String,
    pub duration_ms: f64,
    pub pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainThreadBlockReport {
    pub buckets: Vec<MainThreadBucket>,
    pub total_ms: f64,
    pub dominant: String,
}

// ---------------------------------------------------------------------------
// 16. bfcache Probe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BfCacheNotRestored {
    pub reason: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BfCacheSample {
    pub restored: bool,
    #[serde(default)]
    pub restore_ms: Option<f64>,
    #[serde(default)]
    pub not_restored_reasons: Vec<BfCacheNotRestored>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BfCacheReport {
    pub samples: usize,
    pub restore_rate: f64,
    pub avg_restore_ms: f64,
    pub top_blockers: Vec<(String, usize)>,
    pub rating: String,
}

// ---------------------------------------------------------------------------
// 17. Freeze / Resume Bench
// ---------------------------------------------------------------------------

/// Timing sample for Thor tab soft-freeze / unfreeze / discard lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeTimingSample {
    pub tab_id: String,
    pub soft_freeze_ms: f64,
    pub soft_unfreeze_ms: f64,
    #[serde(default)]
    pub discard_ms: Option<f64>,
    #[serde(default)]
    pub restore_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreezeResumeReport {
    pub samples: usize,
    pub avg_freeze_ms: f64,
    pub avg_unfreeze_ms: f64,
    pub avg_discard_ms: f64,
    pub avg_restore_ms: f64,
    pub p95_freeze_ms: f64,
    pub p95_unfreeze_ms: f64,
    pub slow_tabs: Vec<String>,
}

// ---------------------------------------------------------------------------
// 18. Soft-Nav Vitals (SoftNavRecord / SoftNavVitalsReport re-exported from load)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 19. Speculation Lab
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculationTrial {
    pub url: String,
    pub cold_nav_ms: f64,
    #[serde(default)]
    pub prerender_nav_ms: Option<f64>,
    #[serde(default)]
    pub prefetch_nav_ms: Option<f64>,
    #[serde(default)]
    pub rule_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculationLabReport {
    pub trials: usize,
    pub avg_cold_ms: f64,
    pub avg_prerender_ms: f64,
    pub avg_prefetch_ms: f64,
    pub avg_prerender_savings_ms: f64,
    pub avg_prefetch_savings_ms: f64,
    pub best_url: String,
}

// ---------------------------------------------------------------------------
// 20. Connection Pool Inspector
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnPoolEntry {
    pub url: String,
    pub connection_id: String,
    pub wait_ms: f64,
    #[serde(default)]
    pub reused: bool,
    #[serde(default)]
    pub protocol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnPoolReport {
    pub requests: usize,
    pub unique_connections: usize,
    pub reuse_pct: f64,
    pub avg_wait_ms: f64,
    pub p95_wait_ms: f64,
    pub hottest_connection: String,
    pub requests_on_hottest: usize,
}

// ---------------------------------------------------------------------------
// 21. HTTP/3 Path Audit
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http3PathSample {
    pub url: String,
    #[serde(default)]
    pub alpn: Option<String>,
    pub ttfb_ms: f64,
    #[serde(default)]
    pub fallback_from: Option<String>,
    #[serde(default)]
    pub fallback_cost_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Http3PathReport {
    pub samples: usize,
    pub h3_pct: f64,
    pub h2_pct: f64,
    pub other_pct: f64,
    pub avg_h3_ttfb_ms: f64,
    pub avg_h2_ttfb_ms: f64,
    pub avg_fallback_cost_ms: f64,
    pub fallbacks: usize,
}

// ---------------------------------------------------------------------------
// 22. Compression Budget
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionEntry {
    pub url: String,
    pub transfer_bytes: u64,
    pub decoded_bytes: u64,
    #[serde(default)]
    pub content_encoding: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionFlag {
    pub url: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionBudgetReport {
    pub entries: usize,
    pub total_transfer_bytes: u64,
    pub total_decoded_bytes: u64,
    pub inflation_ratio: f64,
    pub weak_encoding: Vec<CompressionFlag>,
    pub passed: bool,
}

// ---------------------------------------------------------------------------
// 23. Media Weight Gate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaResource {
    pub url: String,
    pub bytes: u64,
    pub media_type: String, // "image" | "video" | "other"
    #[serde(default)]
    pub contributes_to_lcp: bool,
    #[serde(default)]
    pub lcp_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaWeightReport {
    pub total_image_bytes: u64,
    pub total_video_bytes: u64,
    pub lcp_media_bytes: u64,
    pub lcp_url: String,
    pub over_budget: Vec<String>,
    pub passed: bool,
}

// ---------------------------------------------------------------------------
// 24. Continuous Profiler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackSample {
    /// Frames root → leaf.
    pub stack: Vec<String>,
    #[serde(default = "default_stack_weight")]
    pub weight: u64,
}

fn default_stack_weight() -> u64 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousProfileReport {
    pub samples: usize,
    pub total_weight: u64,
    pub folded: Vec<String>,
    pub flame: FlameNode,
    pub speedscope: serde_json::Value,
}

/// Optional resource-weight inputs for budget checks (compression / media).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceBudgetInputs {
    pub transfer_bytes: Option<u64>,
    pub decoded_bytes: Option<u64>,
    pub lcp_media_bytes: Option<u64>,
}

/// Performance specialist engine.
pub struct PerfEngine {
    synthetic: Mutex<Vec<SyntheticCheck>>,
    freeze_samples: Mutex<Vec<FreezeTimingSample>>,
    /// In-flight freeze timings keyed by tab id (completed on unfreeze).
    freeze_pending: Mutex<std::collections::HashMap<String, f64>>,
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
            freeze_samples: Mutex::new(Vec::new()),
            freeze_pending: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

fn percentile_of(samples: &[f64], q: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((q / 100.0) * (sorted.len() as f64)).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[idx]
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

    fn soft_nav_pair(values: [(f64, f64, f64, f64); 2]) -> [SoftNavRecord; 2] {
        std::array::from_fn(|index| {
            let (lcp_ms, inp_ms, cls, ttfb_ms) = values[index];
            SoftNavRecord {
                navigation_id: (index + 1).to_string(),
                url: ["/a", "/b"][index].into(),
                lcp_ms,
                inp_ms,
                cls,
                ttfb_ms,
            }
        })
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
    fn test_budget_check_agrees_with_performance_budgets() {
        let e = PerfEngine::new();
        let budgets = PerformanceBudgets::core_web_vitals();
        let bad = BrowserMetrics {
            lcp_ms: 4000.0,
            inp_ms: 300.0,
            cls: 0.3,
            tbt_ms: 500.0,
            ttfb_ms: 1200.0,
            ..Default::default()
        };
        let load = LoadMetrics {
            total_requests: 100,
            successful_requests: 90,
            failed_requests: 10,
            duration_secs: 1.0,
            max_latency_ms: 100.0,
            p95_latency_ms: 90.0,
            p99_latency_ms: 99.0,
            ..crate::features::sla::budget_load_metrics(bad.clone())
        };
        let pe = e.check_budgets(&budgets, Some(&bad), Some(&load));
        let eval = budgets.evaluate(Some(&bad), Some(&load), None, None, None);
        assert_eq!(pe.passed, eval.passed);
        assert_eq!(pe.checked, eval.checked);
        let pe_metrics: Vec<_> = pe.violations.iter().map(|v| v.metric.as_str()).collect();
        let eval_metrics: Vec<_> = eval.violations.iter().map(|v| v.metric.as_str()).collect();
        assert_eq!(pe_metrics, eval_metrics);
        let msgs = budgets.check(&load);
        assert_eq!(msgs.len(), pe.violations.len());
    }

    #[test]
    fn test_freeze_timing_record_roundtrip() {
        let e = PerfEngine::new();
        e.note_soft_freeze("tab-1", 12.5);
        e.note_soft_unfreeze("tab-1", 8.0);
        e.note_discard("tab-1", 3.0);
        e.note_restore("tab-1", 40.0);
        let samples = e.freeze_timing_samples();
        assert_eq!(samples.len(), 1);
        assert!((samples[0].soft_freeze_ms - 12.5).abs() < 1e-6);
        assert!((samples[0].soft_unfreeze_ms - 8.0).abs() < 1e-6);
        assert_eq!(samples[0].discard_ms, Some(3.0));
        assert_eq!(samples[0].restore_ms, Some(40.0));
        let report = e.freeze_resume_bench(&samples);
        assert_eq!(report.samples, 1);
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
                synthetic: false,
            });
        }
        let s = e.synthetic_summary("api");
        assert_eq!(s.checks, 10);
        assert_eq!(s.synthetic_excluded, 0);
        assert!((s.uptime_pct - 90.0).abs() < 1e-6);
        assert!(s.p95_latency_ms > 0.0);
    }

    #[test]
    fn test_synthetic_from_session_marks_unmeasured() {
        use crate::dx::test_generator::{NetworkTrace, RecordedSession};
        use std::collections::HashMap;
        let e = PerfEngine::new();
        let mut session = RecordedSession::new("s", "https://example.com");
        session.add_trace(NetworkTrace {
            method: "GET".into(),
            url: "https://example.com/api".into(),
            headers: HashMap::new(),
            body: None,
            status: 200,
        });
        let n = e.synthetic_from_session(&session);
        assert_eq!(n, 1);
        let s = e.synthetic_summary("https://example.com/api");
        assert_eq!(s.checks, 1);
        assert_eq!(s.synthetic_excluded, 1);
        assert_eq!(s.avg_latency_ms, 0.0);
        assert_eq!(s.p95_latency_ms, 0.0);
        assert!((s.uptime_pct - 100.0).abs() < 1e-6);
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

    #[test]
    fn test_lcp_element_forensics_from_json() {
        let e = PerfEngine::new();
        let payload = LcpElementPayload {
            vitals_json: Some(
                r#"{"lcp":2100,"lcpElement":{"url":"https://cdn/hero.webp","type":"image","size":82000,"discoverTime":120,"loadTime":900,"renderTime":2100}}"#
                    .into(),
            ),
            ..Default::default()
        };
        let r = e.lcp_element_forensics(&payload);
        assert_eq!(r.url, "https://cdn/hero.webp");
        assert_eq!(r.element_type, "image");
        assert_eq!(r.size_bytes, 82000);
        assert!((r.phases.total_ms - 2100.0).abs() < 1e-6);
        assert_eq!(r.rating, "good");
        assert!(r.phases.load_ms > 0.0);
    }

    #[test]
    fn test_long_task_radar_ranks_inp_overlap() {
        let e = PerfEngine::new();
        let tasks = vec![
            LongTaskEvent {
                duration_ms: 120.0,
                start_ms: 0.0,
                script_url: Some("a.js".into()),
                name: Some("script".into()),
                inp_overlap: false,
            },
            LongTaskEvent {
                duration_ms: 100.0,
                start_ms: 10.0,
                script_url: Some("b.js".into()),
                name: Some("loaf".into()),
                inp_overlap: true,
            },
        ];
        let r = e.long_task_radar(&tasks);
        assert_eq!(r.ranked[0].script_url, "b.js"); // 50*1.5 > 70
        assert_eq!(r.inp_overlapping, 1);
        assert!((r.total_blocking_ms - 120.0).abs() < 1e-6);
    }

    #[test]
    fn test_main_thread_block_map() {
        let e = PerfEngine::new();
        let slices = vec![
            TraceCategorySlice {
                category: "v8.execute".into(),
                duration_ms: 400.0,
            },
            TraceCategorySlice {
                category: "blink.layout".into(),
                duration_ms: 100.0,
            },
            TraceCategorySlice {
                category: "devtools.timeline:Paint".into(),
                duration_ms: 50.0,
            },
            TraceCategorySlice {
                category: "idle".into(),
                duration_ms: 450.0,
            },
        ];
        let r = e.main_thread_block_map(&slices);
        assert!((r.total_ms - 1000.0).abs() < 1e-6);
        assert!(r.dominant == "idle" || r.dominant == "script");
        assert!(r
            .buckets
            .iter()
            .any(|b| b.bucket == "script" && (b.pct - 40.0).abs() < 1e-6));
    }

    #[test]
    fn test_bfcache_probe() {
        let e = PerfEngine::new();
        let samples = vec![
            BfCacheSample {
                restored: true,
                restore_ms: Some(12.0),
                not_restored_reasons: vec![],
            },
            BfCacheSample {
                restored: false,
                restore_ms: None,
                not_restored_reasons: vec![BfCacheNotRestored {
                    reason: "UnloadHandler".into(),
                    source: Some("document".into()),
                }],
            },
            BfCacheSample {
                restored: true,
                restore_ms: Some(8.0),
                not_restored_reasons: vec![],
            },
        ];
        let r = e.bfcache_probe(&samples);
        assert!((r.restore_rate - 2.0 / 3.0).abs() < 1e-6);
        assert!((r.avg_restore_ms - 10.0).abs() < 1e-6);
        assert_eq!(r.top_blockers[0].0, "UnloadHandler");
    }

    #[test]
    fn test_freeze_resume_bench() {
        let e = PerfEngine::new();
        let samples = vec![
            FreezeTimingSample {
                tab_id: "t1".into(),
                soft_freeze_ms: 5.0,
                soft_unfreeze_ms: 8.0,
                discard_ms: Some(20.0),
                restore_ms: Some(30.0),
            },
            FreezeTimingSample {
                tab_id: "t2".into(),
                soft_freeze_ms: 40.0,
                soft_unfreeze_ms: 12.0,
                discard_ms: None,
                restore_ms: None,
            },
        ];
        let r = e.freeze_resume_bench(&samples);
        assert_eq!(r.samples, 2);
        assert!((r.avg_freeze_ms - 22.5).abs() < 1e-6);
        assert!(r.p95_freeze_ms >= 40.0);
    }

    #[test]
    fn test_soft_nav_and_speculation() {
        let e = PerfEngine::new();
        let soft = e
            .soft_nav_vitals(&soft_nav_pair([
                (800.0, 50.0, 0.01, 0.0),
                (1200.0, 80.0, 0.02, 0.0),
            ]))
            .expect("real soft-nav vitals");
        assert_eq!(soft.navigations, 2);
        assert!((soft.avg_lcp_ms - 1000.0).abs() < 1e-6);
        assert!((soft.worst_lcp_ms - 1200.0).abs() < 1e-6);

        let lab = e.speculation_lab(&[SpeculationTrial {
            url: "/home".into(),
            cold_nav_ms: 1000.0,
            prerender_nav_ms: Some(200.0),
            prefetch_nav_ms: Some(600.0),
            rule_type: Some("prerender".into()),
        }]);
        assert!((lab.avg_prerender_savings_ms - 800.0).abs() < 1e-6);
        assert_eq!(lab.best_url, "/home");
    }

    #[test]
    fn test_soft_nav_refuses_all_zero() {
        let e = PerfEngine::new();
        let err = e
            .soft_nav_vitals(&soft_nav_pair([
                (0.0, 0.0, 0.0, 0.0),
                (0.0, 0.0, 0.0, 12.0),
            ]))
            .expect_err("all-zero soft-nav must be refused");
        assert!(
            err.contains("refused") && err.contains("lcp=inp=cls=0"),
            "err={err}"
        );
        // Empty observe path stays Ok (no soft-nav events).
        let empty = e.soft_nav_vitals(&[]).expect("empty ok");
        assert_eq!(empty.navigations, 0);
    }

    #[test]
    fn test_conn_pool_http3_compression_media_profiler() {
        let e = PerfEngine::new();
        let pool = e.conn_pool_inspect(&[
            ConnPoolEntry {
                url: "/a".into(),
                connection_id: "c1".into(),
                wait_ms: 2.0,
                reused: false,
                protocol: Some("h2".into()),
            },
            ConnPoolEntry {
                url: "/b".into(),
                connection_id: "c1".into(),
                wait_ms: 10.0,
                reused: true,
                protocol: Some("h2".into()),
            },
            ConnPoolEntry {
                url: "/c".into(),
                connection_id: "c2".into(),
                wait_ms: 1.0,
                reused: false,
                protocol: Some("h3".into()),
            },
        ]);
        assert!((pool.reuse_pct - 100.0 / 3.0).abs() < 1e-6);
        assert_eq!(pool.hottest_connection, "c1");
        assert_eq!(pool.requests_on_hottest, 2);

        let h3 = e.http3_path_audit(&[
            Http3PathSample {
                url: "/".into(),
                alpn: Some("h3".into()),
                ttfb_ms: 40.0,
                fallback_from: None,
                fallback_cost_ms: None,
            },
            Http3PathSample {
                url: "/x".into(),
                alpn: Some("h2".into()),
                ttfb_ms: 80.0,
                fallback_from: Some("h3".into()),
                fallback_cost_ms: Some(25.0),
            },
        ]);
        assert!((h3.h3_pct - 50.0).abs() < 1e-6);
        assert_eq!(h3.fallbacks, 1);
        assert!((h3.avg_fallback_cost_ms - 25.0).abs() < 1e-6);

        let comp = e.compression_budget(
            &[CompressionEntry {
                url: "/app.js".into(),
                transfer_bytes: 100_000,
                decoded_bytes: 400_000,
                content_encoding: None,
                content_type: Some("application/javascript".into()),
            }],
            3.0,
        );
        assert!(!comp.passed);
        assert!(!comp.weak_encoding.is_empty());

        let media = e.media_weight_gate(
            &[MediaResource {
                url: "hero.webp".into(),
                bytes: 600_000,
                media_type: "image".into(),
                contributes_to_lcp: true,
                lcp_ms: Some(2200.0),
            }],
            500_000,
            2_000_000,
            300_000,
        );
        assert!(!media.passed);
        assert_eq!(media.lcp_url, "hero.webp");

        let profile = e.continuous_profile(&[
            StackSample {
                stack: vec!["main".into(), "render".into()],
                weight: 5,
            },
            StackSample {
                stack: vec!["main".into(), "render".into()],
                weight: 3,
            },
            StackSample {
                stack: vec!["main".into(), "fetch".into()],
                weight: 2,
            },
        ]);
        assert_eq!(profile.total_weight, 10);
        assert_eq!(profile.flame.value, 10);
        assert!(profile.speedscope.get("$schema").is_some());
    }
}

#[cfg(test)]
mod behavior_tests;
