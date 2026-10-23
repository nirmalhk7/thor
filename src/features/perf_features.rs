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

    /// Record soft-freeze duration for `tab_id` (paired with a later unfreeze).
    pub fn note_soft_freeze(&self, tab_id: &str, soft_freeze_ms: f64) {
        self.freeze_pending
            .lock()
            .insert(tab_id.to_string(), soft_freeze_ms);
    }

    /// Complete a freeze/unfreeze pair and push a [`FreezeTimingSample`].
    pub fn note_soft_unfreeze(&self, tab_id: &str, soft_unfreeze_ms: f64) {
        let freeze_ms = self
            .freeze_pending
            .lock()
            .remove(tab_id)
            .unwrap_or(0.0);
        self.record_freeze_sample(FreezeTimingSample {
            tab_id: tab_id.to_string(),
            soft_freeze_ms: freeze_ms,
            soft_unfreeze_ms,
            discard_ms: None,
            restore_ms: None,
        });
    }

    /// Attach discard timing to the latest sample for `tab_id`, or push a new one.
    pub fn note_discard(&self, tab_id: &str, discard_ms: f64) {
        let mut samples = self.freeze_samples.lock();
        if let Some(s) = samples.iter_mut().rev().find(|s| s.tab_id == tab_id) {
            s.discard_ms = Some(discard_ms);
        } else {
            samples.push(FreezeTimingSample {
                tab_id: tab_id.to_string(),
                soft_freeze_ms: 0.0,
                soft_unfreeze_ms: 0.0,
                discard_ms: Some(discard_ms),
                restore_ms: None,
            });
        }
    }

    /// Attach restore timing to the latest sample for `tab_id`.
    pub fn note_restore(&self, tab_id: &str, restore_ms: f64) {
        let mut samples = self.freeze_samples.lock();
        if let Some(s) = samples.iter_mut().rev().find(|s| s.tab_id == tab_id) {
            s.restore_ms = Some(restore_ms);
        } else {
            samples.push(FreezeTimingSample {
                tab_id: tab_id.to_string(),
                soft_freeze_ms: 0.0,
                soft_unfreeze_ms: 0.0,
                discard_ms: None,
                restore_ms: Some(restore_ms),
            });
        }
    }

    pub fn record_freeze_sample(&self, sample: FreezeTimingSample) {
        self.freeze_samples.lock().push(sample);
    }

    pub fn freeze_timing_samples(&self) -> Vec<FreezeTimingSample> {
        self.freeze_samples.lock().clone()
    }

    pub fn clear_freeze_timing_samples(&self) {
        self.freeze_samples.lock().clear();
        self.freeze_pending.lock().clear();
    }

    // 1. Perf Budgets -------------------------------------------------------
    pub fn check_budgets(
        &self,
        budgets: &PerformanceBudgets,
        vitals: Option<&BrowserMetrics>,
        load: Option<&LoadMetrics>,
    ) -> BudgetReport {
        self.check_budgets_with_resources(budgets, vitals, load, None)
    }

    /// Delegates to [`PerformanceBudgets::evaluate`] — single source of truth
    /// shared with [`PerformanceBudgets::check`].
    pub fn check_budgets_with_resources(
        &self,
        budgets: &PerformanceBudgets,
        vitals: Option<&BrowserMetrics>,
        load: Option<&LoadMetrics>,
        resources: Option<&ResourceBudgetInputs>,
    ) -> BudgetReport {
        let (transfer, decoded, lcp_media) = match resources {
            Some(r) => (r.transfer_bytes, r.decoded_bytes, r.lcp_media_bytes),
            None => (None, None, None),
        };
        let BudgetEvaluation {
            passed,
            violations,
            checked,
        } = budgets.evaluate(vitals, load, transfer, decoded, lcp_media);
        BudgetReport {
            passed,
            checked,
            violations: violations
                .into_iter()
                .map(|v| BudgetViolation {
                    metric: v.metric,
                    actual: v.actual,
                    budget: v.budget,
                    over_by: v.over_by,
                })
                .collect(),
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

    /// Reconstruct the request blocking-chain (initiator graph) from resources
    /// carrying an `initiator` URL, then find the longest cumulative-latency
    /// root→leaf path — the network critical path.
    pub fn blocking_chain(&self, resources: &[InitiatedResource]) -> BlockingChainReport {
        use std::collections::HashMap;
        let dur: HashMap<&str, f64> = resources
            .iter()
            .map(|r| (r.url.as_str(), r.total_ms))
            .collect();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        let known: std::collections::HashSet<&str> =
            resources.iter().map(|r| r.url.as_str()).collect();
        for r in resources {
            if let Some(init) = &r.initiator {
                edges.entry(init.clone()).or_default().push(r.url.clone());
                children
                    .entry(init.clone())
                    .or_default()
                    .push(r.url.clone());
            }
        }
        // Roots: resources whose initiator is unknown/absent.
        let roots: Vec<&InitiatedResource> = resources
            .iter()
            .filter(|r| {
                r.initiator
                    .as_deref()
                    .map(|i| !known.contains(i))
                    .unwrap_or(true)
            })
            .collect();
        let mut best: Vec<String> = Vec::new();
        let mut best_ms = 0.0f64;
        for root in roots {
            let mut path = Vec::new();
            let ms = longest_chain(&root.url, &children, &dur, &mut path);
            if ms > best_ms {
                best_ms = ms;
                best = path;
            }
        }
        BlockingChainReport {
            edges,
            longest_chain: BlockingChain {
                chain: best,
                cumulative_ms: best_ms,
            },
            critical_path_ms: best_ms,
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
                synthetic_excluded: 0,
            };
        }
        let successes = checks.iter().filter(|c| c.success).count();
        let uptime_pct = successes as f64 / count as f64 * 100.0;
        // Status-only / unmeasured probes never feed latency SLA signal.
        let measured: Vec<&SyntheticCheck> = checks.iter().filter(|c| !c.synthetic).collect();
        let synthetic_excluded = count - measured.len();
        let (avg, p95) = if measured.is_empty() {
            (0.0, 0.0)
        } else {
            let avg = measured.iter().map(|c| c.latency_ms).sum::<f64>() / measured.len() as f64;
            let mut hist = LatencyHistogram::default();
            for c in &measured {
                hist.record(c.latency_ms);
            }
            (avg, hist.percentile(95.0))
        };
        SyntheticSummary {
            target: target.to_string(),
            checks: count,
            uptime_pct,
            avg_latency_ms: avg,
            p95_latency_ms: p95,
            synthetic_excluded,
        }
    }

    /// Seed status-only monitor checks from a recorded session.
    ///
    /// [`NetworkTrace`] has no timing field — refuses invented RTT. Records
    /// `synthetic: true` / `latency_ms: 0` so uptime can still be summarized
    /// while latency avg/p95 stay excluded from SLA signal.
    pub fn synthetic_from_session(
        &self,
        session: &crate::dx::test_generator::RecordedSession,
    ) -> usize {
        let mut count = 0;
        for (i, trace) in session.traces.iter().enumerate() {
            self.record_synthetic(SyntheticCheck {
                target: trace.url.clone(),
                success: trace.status > 0 && trace.status < 400,
                latency_ms: 0.0,
                timestamp_ms: i as u64,
                synthetic: true,
            });
            count += 1;
        }
        count
    }

    /// Export a folded-stack profile as speedscope-compatible JSON (evented
    /// profile), so a flame graph can be opened directly in speedscope.app.
    pub fn promote_to_profile(&self, folded: &[String], name: &str) -> serde_json::Value {
        // Build a frame table + open/close event pairs from folded stacks.
        let mut frames: Vec<String> = Vec::new();
        let mut frame_index = std::collections::HashMap::new();
        let mut events = Vec::new();
        let mut at: u64 = 0;
        let intern = |frame: &str,
                      frames: &mut Vec<String>,
                      idx: &mut std::collections::HashMap<String, usize>|
         -> usize {
            if let Some(i) = idx.get(frame) {
                *i
            } else {
                let i = frames.len();
                frames.push(frame.to_string());
                idx.insert(frame.to_string(), i);
                i
            }
        };
        for line in folded {
            let (stack, value) = match line.rsplit_once(' ') {
                Some((s, v)) => (s, v.trim().parse::<u64>().unwrap_or(0)),
                None => continue,
            };
            let stack_frames: Vec<usize> = stack
                .split(';')
                .filter(|f| !f.is_empty())
                .map(|f| intern(f, &mut frames, &mut frame_index))
                .collect();
            for &f in &stack_frames {
                events.push(serde_json::json!({ "type": "O", "frame": f, "at": at }));
            }
            at += value;
            for &f in stack_frames.iter().rev() {
                events.push(serde_json::json!({ "type": "C", "frame": f, "at": at }));
            }
        }
        let shared_frames: Vec<serde_json::Value> = frames
            .iter()
            .map(|f| serde_json::json!({ "name": f }))
            .collect();
        serde_json::json!({
            "$schema": "https://www.speedscope.app/file-format-schema.json",
            "shared": { "frames": shared_frames },
            "profiles": [{
                "type": "evented",
                "name": name,
                "unit": "none",
                "startValue": 0,
                "endValue": at,
                "events": events,
            }],
            "name": name,
            "exporter": "Thor",
        })
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

    // 13. LCP Element Forensics ---------------------------------------------
    pub fn lcp_element_forensics(&self, payload: &LcpElementPayload) -> LcpElementReport {
        let mut url = payload.url.clone().unwrap_or_default();
        let mut element_type = payload.element_type.clone().unwrap_or_else(|| "unknown".into());
        let mut size_bytes = payload.size_bytes.unwrap_or(0);
        let mut discover_ms = payload.discover_ms.unwrap_or(0.0);
        let mut load_ms = payload.load_ms.unwrap_or(0.0);
        let mut render_ms = payload.render_ms.unwrap_or(0.0);

        if let Some(raw) = &payload.vitals_json {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
                if url.is_empty() {
                    url = v
                        .pointer("/lcpElement/url")
                        .or_else(|| v.get("lcpUrl"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                }
                if element_type == "unknown" {
                    element_type = v
                        .pointer("/lcpElement/type")
                        .or_else(|| v.get("lcpType"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                }
                if size_bytes == 0 {
                    size_bytes = v
                        .pointer("/lcpElement/size")
                        .or_else(|| v.get("lcpSize"))
                        .and_then(|x| x.as_u64())
                        .unwrap_or(0);
                }
                let num = |keys: &[&str]| -> Option<f64> {
                    for k in keys {
                        if let Some(n) = v.get(*k).and_then(|x| x.as_f64()) {
                            return Some(n);
                        }
                        if let Some(n) = v.pointer(k).and_then(|x| x.as_f64()) {
                            return Some(n);
                        }
                    }
                    None
                };
                if discover_ms <= 0.0 {
                    discover_ms = num(&["lcpDiscover", "/lcpElement/discoverTime", "discover_ms"])
                        .unwrap_or(0.0);
                }
                if load_ms <= 0.0 {
                    load_ms =
                        num(&["lcpLoad", "/lcpElement/loadTime", "load_ms"]).unwrap_or(0.0);
                }
                if render_ms <= 0.0 {
                    render_ms = num(&[
                        "lcp",
                        "lcp_ms",
                        "lcpRender",
                        "/lcpElement/renderTime",
                        "render_ms",
                    ])
                    .unwrap_or(0.0);
                }
            }
        }

        // Phase deltas: discover → load → render (clamp negatives to 0).
        let discover = discover_ms.max(0.0);
        let load_phase = (load_ms - discover).max(0.0);
        let render_phase = (render_ms - load_ms.max(discover)).max(0.0);
        let total = render_ms.max(load_ms).max(discover);
        let rating = if total <= 2500.0 {
            "good"
        } else if total <= 4000.0 {
            "needs-improvement"
        } else {
            "poor"
        };
        LcpElementReport {
            url,
            element_type,
            size_bytes,
            phases: LcpPhaseBreakdown {
                discover_ms: discover,
                load_ms: load_phase,
                render_ms: render_phase,
                total_ms: total,
            },
            rating: rating.to_string(),
        }
    }

    // 14. Long Task Radar ---------------------------------------------------
    pub fn long_task_radar(&self, tasks: &[LongTaskEvent]) -> LongTaskRadarReport {
        let mut ranked: Vec<LongTaskRank> = tasks
            .iter()
            .map(|t| {
                let blocking = (t.duration_ms - 50.0).max(0.0);
                let overlap_boost = if t.inp_overlap { 1.5 } else { 1.0 };
                LongTaskRank {
                    duration_ms: t.duration_ms,
                    script_url: t.script_url.clone().unwrap_or_else(|| "(inline)".into()),
                    name: t.name.clone().unwrap_or_else(|| "longtask".into()),
                    inp_overlap: t.inp_overlap,
                    blocking_score: blocking * overlap_boost,
                }
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.blocking_score
                .partial_cmp(&a.blocking_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let total_blocking_ms: f64 = ranked.iter().map(|r| (r.duration_ms - 50.0).max(0.0)).sum();
        let inp_overlapping = ranked.iter().filter(|r| r.inp_overlap).count();
        LongTaskRadarReport {
            ranked,
            total_blocking_ms,
            inp_overlapping,
        }
    }

    // 15. Main-Thread Block Map ---------------------------------------------
    pub fn main_thread_block_map(&self, slices: &[TraceCategorySlice]) -> MainThreadBlockReport {
        let mut script = 0.0;
        let mut style = 0.0;
        let mut layout = 0.0;
        let mut paint = 0.0;
        let mut idle = 0.0;
        let mut other = 0.0;
        for s in slices {
            let cat = s.category.to_lowercase();
            let bucket = if cat.contains("script") || cat.contains("v8") || cat.contains("js") {
                &mut script
            } else if cat.contains("style") || cat.contains("css") || cat.contains("parsehtml") {
                &mut style
            } else if cat.contains("layout") || cat.contains("reflow") {
                &mut layout
            } else if cat.contains("paint") || cat.contains("raster") || cat.contains("composite") {
                &mut paint
            } else if cat.contains("idle") || cat.contains("sleep") {
                &mut idle
            } else {
                &mut other
            };
            *bucket += s.duration_ms.max(0.0);
        }
        let total = script + style + layout + paint + idle + other;
        let mk = |name: &str, dur: f64| MainThreadBucket {
            bucket: name.to_string(),
            duration_ms: dur,
            pct: if total > 0.0 { dur / total * 100.0 } else { 0.0 },
        };
        let mut buckets = vec![
            mk("script", script),
            mk("style", style),
            mk("layout", layout),
            mk("paint", paint),
            mk("idle", idle),
            mk("other", other),
        ];
        buckets.sort_by(|a, b| {
            b.duration_ms
                .partial_cmp(&a.duration_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let dominant = buckets
            .first()
            .map(|b| b.bucket.clone())
            .unwrap_or_else(|| "none".into());
        MainThreadBlockReport {
            buckets,
            total_ms: total,
            dominant,
        }
    }

    // 16. bfcache Probe -----------------------------------------------------
    pub fn bfcache_probe(&self, samples: &[BfCacheSample]) -> BfCacheReport {
        let n = samples.len();
        if n == 0 {
            return BfCacheReport {
                samples: 0,
                restore_rate: 0.0,
                avg_restore_ms: 0.0,
                top_blockers: Vec::new(),
                rating: "unknown".into(),
            };
        }
        let restored = samples.iter().filter(|s| s.restored).count();
        let restore_rate = restored as f64 / n as f64;
        let restore_times: Vec<f64> = samples
            .iter()
            .filter_map(|s| s.restore_ms.filter(|_| s.restored))
            .collect();
        let avg_restore_ms = if restore_times.is_empty() {
            0.0
        } else {
            restore_times.iter().sum::<f64>() / restore_times.len() as f64
        };
        let mut blockers: BTreeMap<String, usize> = BTreeMap::new();
        for s in samples {
            for r in &s.not_restored_reasons {
                *blockers.entry(r.reason.clone()).or_default() += 1;
            }
        }
        let mut top_blockers: Vec<(String, usize)> = blockers.into_iter().collect();
        top_blockers.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top_blockers.truncate(5);
        let rating = if restore_rate >= 0.9 {
            "good"
        } else if restore_rate >= 0.5 {
            "needs-improvement"
        } else {
            "poor"
        };
        BfCacheReport {
            samples: n,
            restore_rate,
            avg_restore_ms,
            top_blockers,
            rating: rating.to_string(),
        }
    }

    // 17. Freeze / Resume Bench ---------------------------------------------
    pub fn freeze_resume_bench(&self, samples: &[FreezeTimingSample]) -> FreezeResumeReport {
        if samples.is_empty() {
            return FreezeResumeReport {
                samples: 0,
                avg_freeze_ms: 0.0,
                avg_unfreeze_ms: 0.0,
                avg_discard_ms: 0.0,
                avg_restore_ms: 0.0,
                p95_freeze_ms: 0.0,
                p95_unfreeze_ms: 0.0,
                slow_tabs: Vec::new(),
            };
        }
        let n = samples.len() as f64;
        let avg_freeze_ms = samples.iter().map(|s| s.soft_freeze_ms).sum::<f64>() / n;
        let avg_unfreeze_ms = samples.iter().map(|s| s.soft_unfreeze_ms).sum::<f64>() / n;
        let discards: Vec<f64> = samples.iter().filter_map(|s| s.discard_ms).collect();
        let restores: Vec<f64> = samples.iter().filter_map(|s| s.restore_ms).collect();
        let avg_discard_ms = if discards.is_empty() {
            0.0
        } else {
            discards.iter().sum::<f64>() / discards.len() as f64
        };
        let avg_restore_ms = if restores.is_empty() {
            0.0
        } else {
            restores.iter().sum::<f64>() / restores.len() as f64
        };
        let freeze_vals: Vec<f64> = samples.iter().map(|s| s.soft_freeze_ms).collect();
        let unfreeze_vals: Vec<f64> = samples.iter().map(|s| s.soft_unfreeze_ms).collect();
        let p95_freeze_ms = percentile_of(&freeze_vals, 95.0);
        let p95_unfreeze_ms = percentile_of(&unfreeze_vals, 95.0);
        let slow_threshold = p95_freeze_ms.max(50.0);
        let mut slow_tabs: Vec<String> = samples
            .iter()
            .filter(|s| s.soft_freeze_ms >= slow_threshold || s.soft_unfreeze_ms >= slow_threshold)
            .map(|s| s.tab_id.clone())
            .collect();
        slow_tabs.sort();
        slow_tabs.dedup();
        FreezeResumeReport {
            samples: samples.len(),
            avg_freeze_ms,
            avg_unfreeze_ms,
            avg_discard_ms,
            avg_restore_ms,
            p95_freeze_ms,
            p95_unfreeze_ms,
            slow_tabs,
        }
    }

    // 18. Soft-Nav Vitals ---------------------------------------------------
    /// Aggregate soft-navigation CWV records.
    ///
    /// Refuses all-zero theater: when every record has `lcp_ms = inp_ms = cls = 0`,
    /// returns `Err` instead of a fake-good report. Empty input is allowed (no
    /// soft-navs observed) and yields an empty report.
    pub fn soft_nav_vitals(
        &self,
        records: &[SoftNavRecord],
    ) -> Result<SoftNavVitalsReport, String> {
        if !records.is_empty()
            && records
                .iter()
                .all(|r| r.lcp_ms == 0.0 && r.inp_ms == 0.0 && r.cls == 0.0)
        {
            return Err(
                "soft-nav vitals refused: all records have lcp=inp=cls=0 (no real SoftNav CWV)"
                    .into(),
            );
        }
        if records.is_empty() {
            return Ok(SoftNavVitalsReport {
                navigations: 0,
                records: Vec::new(),
                avg_lcp_ms: 0.0,
                worst_lcp_ms: 0.0,
                avg_inp_ms: 0.0,
                total_cls: 0.0,
            });
        }
        let n = records.len() as f64;
        let avg_lcp_ms = records.iter().map(|r| r.lcp_ms).sum::<f64>() / n;
        let worst_lcp_ms = records
            .iter()
            .map(|r| r.lcp_ms)
            .fold(0.0_f64, f64::max);
        let avg_inp_ms = records.iter().map(|r| r.inp_ms).sum::<f64>() / n;
        let total_cls = records.iter().map(|r| r.cls).sum::<f64>();
        Ok(SoftNavVitalsReport {
            navigations: records.len(),
            records: records.to_vec(),
            avg_lcp_ms,
            worst_lcp_ms,
            avg_inp_ms,
            total_cls,
        })
    }

    // 19. Speculation Lab ---------------------------------------------------
    pub fn speculation_lab(&self, trials: &[SpeculationTrial]) -> SpeculationLabReport {
        if trials.is_empty() {
            return SpeculationLabReport {
                trials: 0,
                avg_cold_ms: 0.0,
                avg_prerender_ms: 0.0,
                avg_prefetch_ms: 0.0,
                avg_prerender_savings_ms: 0.0,
                avg_prefetch_savings_ms: 0.0,
                best_url: String::new(),
            };
        }
        let n = trials.len() as f64;
        let avg_cold_ms = trials.iter().map(|t| t.cold_nav_ms).sum::<f64>() / n;
        let prerenders: Vec<(f64, f64)> = trials
            .iter()
            .filter_map(|t| t.prerender_nav_ms.map(|p| (t.cold_nav_ms, p)))
            .collect();
        let prefetches: Vec<(f64, f64)> = trials
            .iter()
            .filter_map(|t| t.prefetch_nav_ms.map(|p| (t.cold_nav_ms, p)))
            .collect();
        let avg_prerender_ms = if prerenders.is_empty() {
            0.0
        } else {
            prerenders.iter().map(|(_, p)| *p).sum::<f64>() / prerenders.len() as f64
        };
        let avg_prefetch_ms = if prefetches.is_empty() {
            0.0
        } else {
            prefetches.iter().map(|(_, p)| *p).sum::<f64>() / prefetches.len() as f64
        };
        let avg_prerender_savings_ms = if prerenders.is_empty() {
            0.0
        } else {
            prerenders.iter().map(|(c, p)| (c - p).max(0.0)).sum::<f64>() / prerenders.len() as f64
        };
        let avg_prefetch_savings_ms = if prefetches.is_empty() {
            0.0
        } else {
            prefetches.iter().map(|(c, p)| (c - p).max(0.0)).sum::<f64>() / prefetches.len() as f64
        };
        let best_url = trials
            .iter()
            .max_by(|a, b| {
                let sa = a
                    .prerender_nav_ms
                    .map(|p| a.cold_nav_ms - p)
                    .or_else(|| a.prefetch_nav_ms.map(|p| a.cold_nav_ms - p))
                    .unwrap_or(0.0);
                let sb = b
                    .prerender_nav_ms
                    .map(|p| b.cold_nav_ms - p)
                    .or_else(|| b.prefetch_nav_ms.map(|p| b.cold_nav_ms - p))
                    .unwrap_or(0.0);
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|t| t.url.clone())
            .unwrap_or_default();
        SpeculationLabReport {
            trials: trials.len(),
            avg_cold_ms,
            avg_prerender_ms,
            avg_prefetch_ms,
            avg_prerender_savings_ms,
            avg_prefetch_savings_ms,
            best_url,
        }
    }

    // 20. Connection Pool Inspector -----------------------------------------
    pub fn conn_pool_inspect(&self, entries: &[ConnPoolEntry]) -> ConnPoolReport {
        if entries.is_empty() {
            return ConnPoolReport {
                requests: 0,
                unique_connections: 0,
                reuse_pct: 0.0,
                avg_wait_ms: 0.0,
                p95_wait_ms: 0.0,
                hottest_connection: String::new(),
                requests_on_hottest: 0,
            };
        }
        let reused = entries.iter().filter(|e| e.reused).count();
        let reuse_pct = reused as f64 / entries.len() as f64 * 100.0;
        let waits: Vec<f64> = entries.iter().map(|e| e.wait_ms).collect();
        let avg_wait_ms = waits.iter().sum::<f64>() / waits.len() as f64;
        let p95_wait_ms = percentile_of(&waits, 95.0);
        let mut by_conn: BTreeMap<String, usize> = BTreeMap::new();
        for e in entries {
            *by_conn.entry(e.connection_id.clone()).or_default() += 1;
        }
        let unique_connections = by_conn.len();
        let (hottest_connection, requests_on_hottest) = by_conn
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .unwrap_or_default();
        ConnPoolReport {
            requests: entries.len(),
            unique_connections,
            reuse_pct,
            avg_wait_ms,
            p95_wait_ms,
            hottest_connection,
            requests_on_hottest,
        }
    }

    // 21. HTTP/3 Path Audit -------------------------------------------------
    pub fn http3_path_audit(&self, samples: &[Http3PathSample]) -> Http3PathReport {
        if samples.is_empty() {
            return Http3PathReport {
                samples: 0,
                h3_pct: 0.0,
                h2_pct: 0.0,
                other_pct: 0.0,
                avg_h3_ttfb_ms: 0.0,
                avg_h2_ttfb_ms: 0.0,
                avg_fallback_cost_ms: 0.0,
                fallbacks: 0,
            };
        }
        let n = samples.len() as f64;
        let classify = |alpn: Option<&str>| -> &str {
            let a = alpn.unwrap_or("").to_lowercase();
            if a.contains("h3") || a.contains("http/3") {
                "h3"
            } else if a.contains("h2") || a.contains("http/2") {
                "h2"
            } else {
                "other"
            }
        };
        let mut h3 = 0usize;
        let mut h2 = 0usize;
        let mut other = 0usize;
        let mut h3_ttfb = Vec::new();
        let mut h2_ttfb = Vec::new();
        let mut fallback_costs = Vec::new();
        for s in samples {
            match classify(s.alpn.as_deref()) {
                "h3" => {
                    h3 += 1;
                    h3_ttfb.push(s.ttfb_ms);
                }
                "h2" => {
                    h2 += 1;
                    h2_ttfb.push(s.ttfb_ms);
                }
                _ => other += 1,
            }
            if let Some(cost) = s.fallback_cost_ms {
                fallback_costs.push(cost);
            } else if s.fallback_from.is_some() {
                // Implicit: difference vs peer h3 average left for later; count fallback.
                fallback_costs.push(0.0);
            }
        }
        let avg = |xs: &[f64]| {
            if xs.is_empty() {
                0.0
            } else {
                xs.iter().sum::<f64>() / xs.len() as f64
            }
        };
        Http3PathReport {
            samples: samples.len(),
            h3_pct: h3 as f64 / n * 100.0,
            h2_pct: h2 as f64 / n * 100.0,
            other_pct: other as f64 / n * 100.0,
            avg_h3_ttfb_ms: avg(&h3_ttfb),
            avg_h2_ttfb_ms: avg(&h2_ttfb),
            avg_fallback_cost_ms: avg(&fallback_costs),
            fallbacks: samples
                .iter()
                .filter(|s| s.fallback_from.is_some() || s.fallback_cost_ms.is_some())
                .count(),
        }
    }

    // 22. Compression Budget ------------------------------------------------
    pub fn compression_budget(
        &self,
        entries: &[CompressionEntry],
        max_inflation_ratio: f64,
    ) -> CompressionBudgetReport {
        let total_transfer_bytes: u64 = entries.iter().map(|e| e.transfer_bytes).sum();
        let total_decoded_bytes: u64 = entries.iter().map(|e| e.decoded_bytes).sum();
        let inflation_ratio = if total_transfer_bytes > 0 {
            total_decoded_bytes as f64 / total_transfer_bytes as f64
        } else {
            0.0
        };
        let mut weak_encoding = Vec::new();
        for e in entries {
            let enc = e.content_encoding.as_deref().unwrap_or("").to_lowercase();
            let ctype = e.content_type.as_deref().unwrap_or("").to_lowercase();
            let compressible = ctype.contains("text/")
                || ctype.contains("javascript")
                || ctype.contains("json")
                || ctype.contains("css")
                || ctype.contains("svg")
                || ctype.contains("xml");
            if compressible && e.transfer_bytes > 1024 {
                if enc.is_empty() || enc == "identity" {
                    weak_encoding.push(CompressionFlag {
                        url: e.url.clone(),
                        reason: "missing content-encoding on compressible asset".into(),
                    });
                } else if enc == "gzip" && e.decoded_bytes > 50_000 {
                    // Prefer brotli/zstd for large text payloads.
                    weak_encoding.push(CompressionFlag {
                        url: e.url.clone(),
                        reason: "gzip on large payload; prefer br/zstd".into(),
                    });
                }
            }
            if e.transfer_bytes > 0 {
                let ratio = e.decoded_bytes as f64 / e.transfer_bytes as f64;
                if ratio > max_inflation_ratio.max(1.0) * 1.5 && enc.is_empty() {
                    weak_encoding.push(CompressionFlag {
                        url: e.url.clone(),
                        reason: format!("decoded/transfer ratio {ratio:.1} without encoding"),
                    });
                }
            }
        }
        let passed = inflation_ratio <= max_inflation_ratio && weak_encoding.is_empty();
        CompressionBudgetReport {
            entries: entries.len(),
            total_transfer_bytes,
            total_decoded_bytes,
            inflation_ratio,
            weak_encoding,
            passed,
        }
    }

    // 23. Media Weight Gate -------------------------------------------------
    pub fn media_weight_gate(
        &self,
        resources: &[MediaResource],
        max_image_bytes: u64,
        max_video_bytes: u64,
        max_lcp_media_bytes: u64,
    ) -> MediaWeightReport {
        let total_image_bytes: u64 = resources
            .iter()
            .filter(|r| r.media_type.eq_ignore_ascii_case("image"))
            .map(|r| r.bytes)
            .sum();
        let total_video_bytes: u64 = resources
            .iter()
            .filter(|r| r.media_type.eq_ignore_ascii_case("video"))
            .map(|r| r.bytes)
            .sum();
        let lcp = resources
            .iter()
            .filter(|r| r.contributes_to_lcp)
            .max_by(|a, b| {
                a.lcp_ms
                    .unwrap_or(0.0)
                    .partial_cmp(&b.lcp_ms.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.bytes.cmp(&b.bytes))
            });
        let lcp_media_bytes = lcp.map(|r| r.bytes).unwrap_or(0);
        let lcp_url = lcp.map(|r| r.url.clone()).unwrap_or_default();
        let mut over_budget = Vec::new();
        if total_image_bytes > max_image_bytes {
            over_budget.push(format!(
                "images {total_image_bytes}B exceed budget {max_image_bytes}B"
            ));
        }
        if total_video_bytes > max_video_bytes {
            over_budget.push(format!(
                "video {total_video_bytes}B exceed budget {max_video_bytes}B"
            ));
        }
        if lcp_media_bytes > max_lcp_media_bytes {
            over_budget.push(format!(
                "LCP media {lcp_media_bytes}B ({lcp_url}) exceeds budget {max_lcp_media_bytes}B"
            ));
        }
        MediaWeightReport {
            total_image_bytes,
            total_video_bytes,
            lcp_media_bytes,
            lcp_url,
            passed: over_budget.is_empty(),
            over_budget,
        }
    }

    // 24. Continuous Profiler -----------------------------------------------
    pub fn continuous_profile(&self, samples: &[StackSample]) -> ContinuousProfileReport {
        let mut folded_map: BTreeMap<String, u64> = BTreeMap::new();
        let mut total_weight = 0u64;
        for s in samples {
            if s.stack.is_empty() {
                continue;
            }
            let key = s.stack.join(";");
            *folded_map.entry(key).or_default() += s.weight.max(1);
            total_weight += s.weight.max(1);
        }
        let folded: Vec<String> = folded_map
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect();
        let flame = self.build_flame_graph(&folded);
        let speedscope = self.promote_to_profile(&folded, "thor-continuous-profile");
        ContinuousProfileReport {
            samples: samples.len(),
            total_weight,
            folded,
            flame,
            speedscope,
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

/// Longest cumulative-latency path from `node` through the initiator graph.
fn longest_chain(
    node: &str,
    children: &std::collections::HashMap<String, Vec<String>>,
    dur: &std::collections::HashMap<&str, f64>,
    path: &mut Vec<String>,
) -> f64 {
    path.push(node.to_string());
    let self_ms = dur.get(node).copied().unwrap_or(0.0);
    let mut best_child_ms = 0.0;
    let mut best_child_path: Vec<String> = Vec::new();
    if let Some(kids) = children.get(node) {
        for k in kids {
            let mut cp = Vec::new();
            let ms = longest_chain(k, children, dur, &mut cp);
            if ms > best_child_ms {
                best_child_ms = ms;
                best_child_path = cp;
            }
        }
    }
    path.extend(best_child_path);
    self_ms + best_child_ms
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
        .next_back()
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
            total_bytes: 0,
            duration_secs: 1.0,
            rps: 100.0,
            min_latency_ms: 1.0,
            max_latency_ms: 100.0,
            avg_latency_ms: 50.0,
            p50_latency_ms: 40.0,
            p90_latency_ms: 80.0,
            p95_latency_ms: 90.0,
            p99_latency_ms: 99.0,
            status_codes: Default::default(),
            browser_vitals: Some(bad.clone()),
            transfer_bytes: None,
            decoded_bytes: None,
            lcp_media_bytes: None,
            soft_nav: None,
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
        assert!(r.buckets.iter().any(|b| b.bucket == "script" && (b.pct - 40.0).abs() < 1e-6));
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
            .soft_nav_vitals(&[
                SoftNavRecord {
                    navigation_id: "1".into(),
                    url: "/a".into(),
                    lcp_ms: 800.0,
                    inp_ms: 50.0,
                    cls: 0.01,
                    ttfb_ms: 0.0,
                },
                SoftNavRecord {
                    navigation_id: "2".into(),
                    url: "/b".into(),
                    lcp_ms: 1200.0,
                    inp_ms: 80.0,
                    cls: 0.02,
                    ttfb_ms: 0.0,
                },
            ])
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
            .soft_nav_vitals(&[
                SoftNavRecord {
                    navigation_id: "1".into(),
                    url: "/a".into(),
                    lcp_ms: 0.0,
                    inp_ms: 0.0,
                    cls: 0.0,
                    ttfb_ms: 0.0,
                },
                SoftNavRecord {
                    navigation_id: "2".into(),
                    url: "/b".into(),
                    lcp_ms: 0.0,
                    inp_ms: 0.0,
                    cls: 0.0,
                    ttfb_ms: 12.0,
                },
            ])
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
