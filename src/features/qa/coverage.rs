use super::{CoverageAdjacencyReport, CoverageEdge, CoveredStep, MutationHookManifest, QaEngine};

impl QaEngine {
    /// Diff observed session edges against covered edges → untested adjacency.
    pub fn coverage_adjacency(
        &self,
        observed: &[CoverageEdge],
        covered: &[CoverageEdge],
    ) -> CoverageAdjacencyReport {
        let covered_set: std::collections::HashSet<_> = covered.iter().cloned().collect();
        let uncovered: Vec<_> = observed
            .iter()
            .filter(|e| !covered_set.contains(*e))
            .cloned()
            .collect();
        let observed_n = observed.len();
        let covered_n = observed_n.saturating_sub(uncovered.len());
        CoverageAdjacencyReport {
            observed: observed_n,
            covered: covered_n,
            coverage_ratio: if observed_n == 0 {
                1.0
            } else {
                covered_n as f64 / observed_n as f64
            },
            uncovered,
        }
    }

    /// Emit Stryker + cargo-mutants hook manifests from covered test steps.
    pub fn mutation_hook_bridge(&self, steps: &[CoveredStep]) -> MutationHookManifest {
        let mut files: Vec<String> = steps.iter().map(|s| s.file.clone()).collect();
        files.sort();
        files.dedup();
        let mut mutants: Vec<String> = steps
            .iter()
            .flat_map(|s| s.mutation_targets.iter().cloned())
            .collect();
        mutants.sort();
        mutants.dedup();
        let stryker = serde_json::json!({
            "mutate": files,
            "testRunner": "command",
            "commandRunner": { "command": "npx playwright test" },
            "coveredSteps": steps.iter().map(|s| &s.id).collect::<Vec<_>>(),
        });
        let cargo_mutants = serde_json::json!({
            "examine": mutants,
            "test_tool": "cargo test",
            "timeout_multiplier": 2.0,
            "covered_steps": steps.len(),
        });
        MutationHookManifest {
            stryker,
            cargo_mutants,
            covered_steps: steps.len(),
        }
    }
}
