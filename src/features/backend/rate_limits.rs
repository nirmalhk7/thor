use super::{ApiRequest, BackendEngine, HashMap, RateLimitReport, RateLimitSample};

impl BackendEngine {
    pub fn rate_limit_probe(&self, samples: &[RateLimitSample]) -> RateLimitReport {
        let limited: Vec<_> = samples
            .iter()
            .filter(|s| s.status == 429 || s.remaining == Some(0))
            .collect();
        let inferred_limit = samples.iter().filter_map(|s| s.limit).max();
        let retries: Vec<f64> = limited.iter().filter_map(|s| s.retry_after_secs).collect();
        let avg_retry_after = if retries.is_empty() {
            None
        } else {
            Some(retries.iter().sum::<f64>() / retries.len() as f64)
        };
        let mut issues = Vec::new();
        if !limited.is_empty() && retries.is_empty() {
            issues.push("429/exhausted without Retry-After".into());
        }
        for s in samples {
            if let (Some(rem), Some(lim)) = (s.remaining, s.limit) {
                if rem > lim {
                    issues.push(format!("remaining {rem} exceeds limit {lim}"));
                }
            }
        }
        RateLimitReport {
            limited_count: limited.len(),
            inferred_limit,
            avg_retry_after,
            issues,
        }
    }

    /// Hit `url` repeatedly via reqwest; consent required for public targets.
    pub fn rate_limit_probe_http(
        &self,
        url: &str,
        count: usize,
        lab_consent: bool,
    ) -> Result<RateLimitReport, String> {
        crate::net::consent::require_lab_consent(url, lab_consent)?;
        let n = count.clamp(1, 50);
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let resp = self.send_blocking(&ApiRequest {
                method: "GET".into(),
                url: url.to_string(),
                headers: HashMap::new(),
                body: None,
            })?;
            let remaining = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("x-ratelimit-remaining"))
                .and_then(|(_, v)| v.parse().ok());
            let limit = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("x-ratelimit-limit"))
                .and_then(|(_, v)| v.parse().ok());
            let retry_after_secs = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
                .and_then(|(_, v)| v.parse().ok());
            samples.push(RateLimitSample {
                status: resp.status,
                retry_after_secs,
                remaining,
                limit,
                timestamp_ms: i as u64,
            });
        }
        Ok(self.rate_limit_probe(&samples))
    }
}
