use super::{HeapSample, LeakReport, PerfEngine};

impl PerfEngine {
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
}
