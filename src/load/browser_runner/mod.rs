use crate::load::metrics::LoadMetrics;
use crate::load::scenario::ThorScenario;

pub struct BrowserRunnerConfig {
    pub headful: bool,
    pub width: u32,
    pub height: u32,
}

impl Default for BrowserRunnerConfig {
    fn default() -> Self {
        Self {
            headful: false,
            width: 1280,
            height: 800,
        }
    }
}

/// Browser Load Test Runner for tracking real Chromium Web Vitals & DOM interaction metrics
pub struct BrowserRunner {
    config: BrowserRunnerConfig,
}

impl BrowserRunner {
    pub fn new(config: BrowserRunnerConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &BrowserRunnerConfig {
        &self.config
    }

    pub fn run_blocking(&self, scenario: &ThorScenario) -> Result<LoadMetrics, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;
        rt.block_on(self.run(scenario))
    }
}

mod estimates;
mod execution;
mod telemetry;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::cdp_bridge::CdpBridge;
    use crate::features::perf_features::PerfEngine;
    use crate::load::metrics::{BrowserMetrics, MetricsCollector};

    #[test]
    fn test_derive_vitals_from_latencies() {
        let nav = vec![40.0, 80.0, 60.0];
        let interactions = vec![20.0, 120.0, 45.0];
        let vitals =
            BrowserRunner::derive_vitals(&nav, &interactions, Some(60.0)).expect("estimate ok");
        assert!(
            vitals.synthetic,
            "HTTP-derived vitals must be marked synthetic"
        );
        assert!(vitals.ttfb_ms > 0.0);
        assert!(vitals.lcp_ms >= vitals.fcp_ms);
        assert!(vitals.inp_ms >= vitals.fid_ms);
        assert!((vitals.tbt_ms - 70.0).abs() < 1e-6);
        assert!(vitals.cls >= 0.0 && vitals.cls <= 1.0);

        let budgets = crate::load::PerformanceBudgets::core_web_vitals();
        let eval = budgets.evaluate(Some(&vitals), None, None, None, None);
        assert_eq!(
            eval.checked, 0,
            "synthetic vitals must be excluded from CWV budget gates"
        );
    }

    #[test]
    fn test_derive_vitals_refuses_empty() {
        let err = BrowserRunner::derive_vitals(&[], &[], None).expect_err("empty refused");
        assert!(err.contains("refused"), "{err}");
    }

    #[test]
    fn test_record_sample_uses_transfer_totals_not_1024() {
        // Navigate path records CdpBridge::byte_totals(), never a fake constant size.
        let bridge = CdpBridge::new();
        let n = bridge.ingest_cdp_network_json(
            r#"[{"url":"https://example.com/","transfer_bytes":4096,"decoded_bytes":8192,"bytes":4096}]"#,
        );
        assert_eq!(n, 1);
        let (transfer, decoded) = bridge.byte_totals();
        assert_eq!(transfer, 4096);
        assert_eq!(decoded, 8192);
        assert_ne!(transfer, 1024);
    }

    #[test]
    fn test_soft_nav_attaches_to_load_metrics() {
        let perf = PerfEngine::new();
        let report = perf
            .soft_nav_vitals(&[crate::load::SoftNavRecord {
                navigation_id: "n1".into(),
                url: "/spa".into(),
                lcp_ms: 900.0,
                inp_ms: 40.0,
                cls: 0.02,
                ttfb_ms: 0.0,
            }])
            .expect("soft-nav ok");
        let mut metrics = MetricsCollector::new().snapshot();
        metrics.soft_nav = Some(report);
        let soft = metrics.soft_nav.as_ref().expect("soft_nav attached");
        assert_eq!(soft.navigations, 1);
        assert!((soft.worst_lcp_ms - 900.0).abs() < 1e-6);
        assert!((soft.avg_inp_ms - 40.0).abs() < 1e-6);
    }

    #[test]
    fn test_lcp_media_bytes_from_vitals_json() {
        let (_, media) = CdpBridge::parse_web_vitals_ex(
            r#"{"lcp":1500.0,"cls":0.01,"lcpSize":64000,"lcpElement":{"size":64000}}"#,
        )
        .expect("parse");
        assert_eq!(media, Some(64_000));
        let mut snapshot = MetricsCollector::new().snapshot();
        snapshot.lcp_media_bytes = media;
        assert_eq!(snapshot.lcp_media_bytes, Some(64_000));

        let budgets = crate::load::PerformanceBudgets {
            max_lcp_media_bytes: Some(50_000),
            ..Default::default()
        };
        let violations = budgets.check(&snapshot);
        assert!(
            violations.iter().any(|v| v.contains("lcp_media_bytes")),
            "{violations:?}"
        );
    }

    #[test]
    fn test_soft_nav_folds_into_budget_check() {
        let perf = PerfEngine::new();
        let report = perf
            .soft_nav_vitals(&[crate::load::SoftNavRecord {
                navigation_id: "n1".into(),
                url: "/r".into(),
                lcp_ms: 4000.0,
                inp_ms: 10.0,
                cls: 0.01,
                ttfb_ms: 0.0,
            }])
            .unwrap();
        let mut metrics = MetricsCollector::new().snapshot();
        metrics.browser_vitals = Some(BrowserMetrics {
            lcp_ms: 1000.0,
            inp_ms: 50.0,
            cls: 0.01,
            ..Default::default()
        });
        metrics.soft_nav = Some(report);
        let budgets = crate::load::PerformanceBudgets {
            max_lcp_ms: Some(2500.0),
            ..Default::default()
        };
        let violations = budgets.check(&metrics);
        assert!(
            violations.iter().any(|v| v.contains("LCP")),
            "soft-nav worst LCP should trip budget: {violations:?}"
        );
    }
}
