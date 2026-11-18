use super::{BTreeMap, RaceAttempt, RaceWindowReport, SecurityEngine};

impl SecurityEngine {
    pub fn race_window_repeater(&self, attempts: &[RaceAttempt]) -> RaceWindowReport {
        let mut outcomes: BTreeMap<(u16, String), usize> = BTreeMap::new();
        let mut min_start = u64::MAX;
        let mut max_end = 0u64;
        for a in attempts {
            *outcomes.entry((a.status, a.body_hash.clone())).or_default() += 1;
            min_start = min_start.min(a.start_ms);
            max_end = max_end.max(a.end_ms);
        }
        let unique_outcomes = outcomes.len();
        let window_ms = if attempts.is_empty() {
            0
        } else {
            max_end.saturating_sub(min_start)
        };
        let inconsistency = unique_outcomes > 1;
        RaceWindowReport {
            attempts: attempts.len(),
            unique_outcomes,
            window_ms,
            inconsistency,
            detail: if inconsistency {
                format!("{unique_outcomes} distinct outcomes in {window_ms}ms window")
            } else {
                "consistent outcomes".into()
            },
        }
    }

    /// Consent-gated live parallel fetches → [`race_window_repeater`].
    pub fn race_window_live(
        &self,
        url: &str,
        count: usize,
        lab_consent: bool,
    ) -> Result<RaceWindowReport, String> {
        if url.trim().is_empty() {
            return Err("race-window live requires url".into());
        }
        crate::net::consent::require_lab_consent(url, lab_consent)?;
        let n = count.clamp(2, 32);
        let url = url.to_string();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let attempts = rt.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new());
            let start_wall = std::time::Instant::now();
            let futs: Vec<_> = (0..n)
                .map(|i| {
                    let client = client.clone();
                    let url = url.clone();
                    async move {
                        let t0 = start_wall.elapsed().as_millis() as u64;
                        let (status, body) = match client.get(&url).send().await {
                            Ok(resp) => {
                                let status = resp.status().as_u16();
                                let body = resp.text().await.unwrap_or_default();
                                (status, body)
                            }
                            Err(e) => (0, e.to_string()),
                        };
                        let t1 = start_wall.elapsed().as_millis() as u64;
                        let mut hash: u64 = 0xcbf29ce484222325;
                        for b in body.as_bytes() {
                            hash ^= *b as u64;
                            hash = hash.wrapping_mul(0x100000001b3);
                        }
                        RaceAttempt {
                            id: i as u32,
                            status,
                            body_hash: format!("{hash:016x}"),
                            start_ms: t0,
                            end_ms: t1,
                        }
                    }
                })
                .collect();
            futures::future::join_all(futs).await
        });
        Ok(self.race_window_repeater(&attempts))
    }
}
