use super::{MediaResource, MediaWeightReport, PerfEngine};

impl PerfEngine {
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
}
