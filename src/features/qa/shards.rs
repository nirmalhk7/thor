use super::{HashMap, QaEngine, ShardBucket, ShardPlan, ShardScenario};

impl QaEngine {
    /// Balance scenarios into shards by duration, keeping identity affinity.
    pub fn plan_parallel_shards(
        &self,
        scenarios: &[ShardScenario],
        shard_count: usize,
    ) -> ShardPlan {
        let n = shard_count.max(1);
        let mut shards: Vec<ShardBucket> = (0..n)
            .map(|i| ShardBucket {
                shard_id: i,
                scenarios: Vec::new(),
                total_ms: 0.0,
                identities: Vec::new(),
            })
            .collect();

        // Group by identity so affinity stays within a shard when possible.
        let mut by_identity: HashMap<String, Vec<&ShardScenario>> = HashMap::new();
        for s in scenarios {
            by_identity.entry(s.identity.clone()).or_default().push(s);
        }
        let mut groups: Vec<_> = by_identity.into_iter().collect();
        groups.sort_by(|a, b| {
            let sa: f64 = a.1.iter().map(|s| s.duration_ms).sum();
            let sb: f64 = b.1.iter().map(|s| s.duration_ms).sum();
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });

        for (identity, group) in groups {
            let mut target = 0usize;
            let mut best = f64::MAX;
            for (i, shard) in shards.iter().enumerate() {
                if shard.total_ms < best {
                    best = shard.total_ms;
                    target = i;
                }
            }
            if !identity.is_empty() {
                shards[target].identities.push(identity);
            }
            for s in group {
                shards[target].scenarios.push(s.name.clone());
                shards[target].total_ms += s.duration_ms;
            }
        }

        let plan_json = serde_json::to_string_pretty(&serde_json::json!({
            "shard_count": n,
            "shards": shards,
        }))
        .unwrap_or_else(|_| "{}".into());

        ShardPlan {
            shard_count: n,
            shards,
            plan_json,
        }
    }
}
