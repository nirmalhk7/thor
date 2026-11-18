use super::{dhash, MaskRegion, QaEngine, VisualDiff};

impl QaEngine {
    // 3. Visual Regression --------------------------------------------------
    /// Compare two equal-length pixel buffers (grayscale bytes) plus their dHash
    /// perceptual fingerprints. Returns pixel diff ratio, Hamming distance and a
    /// pass/fail against `threshold` (max allowed pixel diff ratio).
    pub fn visual_regression(
        &self,
        baseline: &[u8],
        candidate: &[u8],
        threshold: f64,
    ) -> VisualDiff {
        let len = baseline.len().min(candidate.len()).max(1);
        let diff_pixels = baseline
            .iter()
            .zip(candidate.iter())
            .filter(|(a, b)| a.abs_diff(**b) > 8)
            .count();
        let size_penalty = baseline.len().abs_diff(candidate.len());
        let pixel_diff_ratio = (diff_pixels + size_penalty) as f64 / len as f64;
        let perceptual_distance = (dhash(baseline) ^ dhash(candidate)).count_ones();
        VisualDiff {
            pixel_diff_ratio,
            perceptual_distance,
            passed: pixel_diff_ratio <= threshold,
            masked_pixels: 0,
        }
    }

    /// Visual regression with masked regions: pixels covered by any
    /// [`MaskRegion`] (e.g. resolved from a dynamic selector's bounding box) are
    /// excluded from the diff so volatile content — timestamps, avatars, ads —
    /// does not fail the comparison. `width` is the row stride of the grayscale
    /// buffers.
    pub fn visual_regression_masked(
        &self,
        baseline: &[u8],
        candidate: &[u8],
        width: usize,
        threshold: f64,
        mask_regions: &[MaskRegion],
    ) -> VisualDiff {
        if width == 0 || mask_regions.is_empty() {
            return self.visual_regression(baseline, candidate, threshold);
        }
        let mut masked: Vec<bool> = vec![false; baseline.len().min(candidate.len())];
        for region in mask_regions {
            for row in region.y..region.y.saturating_add(region.height) {
                let row_start = row * width;
                for col in region.x..region.x.saturating_add(region.width) {
                    let idx = row_start + col;
                    if idx < masked.len() {
                        masked[idx] = true;
                    }
                }
            }
        }
        let masked_pixels = masked.iter().filter(|m| **m).count();
        let compared = masked.len().saturating_sub(masked_pixels).max(1);
        let diff_pixels = baseline
            .iter()
            .zip(candidate.iter())
            .enumerate()
            .filter(|(i, (a, b))| !masked[*i] && a.abs_diff(**b) > 8)
            .count();
        let size_penalty = baseline.len().abs_diff(candidate.len());
        let pixel_diff_ratio = (diff_pixels + size_penalty) as f64 / compared as f64;
        let perceptual_distance = (dhash(baseline) ^ dhash(candidate)).count_ones();
        VisualDiff {
            pixel_diff_ratio,
            perceptual_distance,
            passed: pixel_diff_ratio <= threshold,
            masked_pixels,
        }
    }
}
