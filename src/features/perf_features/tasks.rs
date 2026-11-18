use super::{
    LongTaskEvent, LongTaskRadarReport, LongTaskRank, MainThreadBlockReport, MainThreadBucket,
    PerfEngine, TraceCategorySlice,
};

impl PerfEngine {
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
            pct: if total > 0.0 {
                dur / total * 100.0
            } else {
                0.0
            },
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
}
