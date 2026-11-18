use super::{
    FlakeReplayReport, FlakeReport, FlakeTriage, FlakeVector, ProtocolRunner, QaEngine,
    ThorScenario, TraceFrame,
};

impl QaEngine {
    // 2. Flake Radar --------------------------------------------------------
    /// `results` is an ordered run history (true = pass).
    pub fn flake_radar(&self, test: &str, results: &[bool]) -> FlakeReport {
        let runs = results.len();
        if runs == 0 {
            return FlakeReport {
                test: test.to_string(),
                runs: 0,
                pass_rate: 0.0,
                flip_rate: 0.0,
                classification: "no-data".into(),
            };
        }
        let passes = results.iter().filter(|r| **r).count();
        let pass_rate = passes as f64 / runs as f64;
        let flips = results.windows(2).filter(|w| w[0] != w[1]).count();
        let flip_rate = if runs > 1 {
            flips as f64 / (runs - 1) as f64
        } else {
            0.0
        };
        let classification = if pass_rate == 1.0 {
            "stable-pass"
        } else if pass_rate == 0.0 {
            "consistently-failing"
        } else if flip_rate >= 0.3 {
            "flaky"
        } else {
            "intermittent"
        };
        FlakeReport {
            test: test.to_string(),
            runs,
            pass_rate,
            flip_rate,
            classification: classification.to_string(),
        }
    }

    /// Replay a load scenario `n` times through the protocol runner, treating a
    /// clean run (no budget violation / transport error) as a pass, and classify
    /// the result with the flake radar. This exercises the *active* half of the
    /// flake radar rather than accepting a caller-supplied result vector.
    pub fn flake_radar_replay(&self, scenario: &ThorScenario, n: usize) -> FlakeReplayReport {
        let runner = ProtocolRunner::new();
        let mut results = Vec::with_capacity(n);
        let mut errors = Vec::new();
        for _ in 0..n {
            match runner.run_blocking(scenario) {
                Ok(_) => results.push(true),
                Err(e) => {
                    results.push(false);
                    errors.push(e);
                }
            }
        }
        let radar = self.flake_radar(&scenario.name, &results);
        FlakeReplayReport {
            scenario: scenario.name.clone(),
            requested_runs: n,
            results,
            errors,
            radar,
        }
    }

    // ---- Wave 2 -----------------------------------------------------------

    /// Attribute flake root-cause from a signal vector plus optional time-travel
    /// frames (DOM digest churn around failing steps).
    pub fn triage_flake(
        &self,
        test: &str,
        radar: &FlakeReport,
        vectors: &FlakeVector,
        frames: &[TraceFrame],
    ) -> FlakeTriage {
        let mut candidates = vec![
            ("timing-race", vectors.timing_cv + vectors.race_score),
            ("locator-instability", vectors.locator_churn),
            (
                "network-jitter",
                (vectors.network_jitter_ms / 250.0).min(1.0),
            ),
            ("animation-layout", vectors.animation_score),
        ];
        if radar.classification == "consistently-failing" {
            candidates.push(("deterministic-failure", 0.95));
        } else if radar.classification == "stable-pass" {
            candidates.push(("no-flake", 1.0));
        }
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let (root_cause, raw) = candidates.first().copied().unwrap_or(("unknown", 0.0));
        let confidence = raw.clamp(0.0, 1.0);

        let mut evidence = Vec::new();
        if frames.len() >= 2 {
            for w in frames.windows(2) {
                if w[0].dom_digest != w[1].dom_digest {
                    evidence.push(w[1].index);
                }
            }
        }
        if evidence.is_empty() && !frames.is_empty() {
            evidence.push(frames.last().map(|f| f.index).unwrap_or(0));
        }

        let recommended_action = match root_cause {
            "locator-instability" => "re-rank locators / enable runtime-self-heal",
            "timing-race" => "replace fixed waits with web-first assertions",
            "network-jitter" => "hermetic fixture pack / HAR mocks",
            "animation-layout" => "mask volatile regions in visual regression",
            "deterministic-failure" => "quarantine until root fix lands",
            "no-flake" => "keep green; no action",
            _ => "collect more flake vectors",
        };

        FlakeTriage {
            test: test.to_string(),
            root_cause: root_cause.to_string(),
            confidence,
            evidence_frame_indexes: evidence,
            recommended_action: recommended_action.to_string(),
        }
    }
}
