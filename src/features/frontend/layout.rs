use super::{
    ClsHeatmapReport, ContainCandidate, ContainmentAdvice, ContainmentReport, FrontendEngine,
    LayoutOp, LayoutShiftEntry, LayoutShiftRect, LayoutThrashReport, PaintHeatmapRegion,
    PaintHeatmapReport, StackingElement, TokenUsageF, ZIndexReport,
};
use std::collections::{BTreeMap, HashMap};

impl FrontendEngine {
    // 7. CLS Heatmap --------------------------------------------------------
    pub fn cls_heatmap(&self, entries: &[LayoutShiftEntry]) -> ClsHeatmapReport {
        let mut per_node: HashMap<String, f64> = HashMap::new();
        let mut total = 0.0;
        for e in entries {
            if e.had_recent_input {
                continue; // Excluded from CLS per spec.
            }
            total += e.score;
            *per_node.entry(e.node.clone()).or_default() += e.score;
        }
        let mut contributors: Vec<TokenUsageF> = per_node
            .into_iter()
            .map(|(node, score)| TokenUsageF {
                node,
                score,
                percent: if total > 0.0 {
                    score / total * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        contributors.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let rating = if total <= 0.1 {
            "good"
        } else if total <= 0.25 {
            "needs-improvement"
        } else {
            "poor"
        };
        ClsHeatmapReport {
            total_cls: total,
            contributors,
            rating: rating.to_string(),
        }
    }

    /// Bucket layout shifts into a 3×3 spatial grid of the viewport, producing a
    /// paint heatmap that shows *where* on the page instability concentrates.
    pub fn paint_heatmap(
        &self,
        shifts: &[LayoutShiftRect],
        viewport_w: f64,
        viewport_h: f64,
    ) -> PaintHeatmapReport {
        let vw = viewport_w.max(1.0);
        let vh = viewport_h.max(1.0);
        let col_labels = ["left", "center", "right"];
        let row_labels = ["top", "middle", "bottom"];
        let mut region_score: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        let mut total = 0.0;
        for s in shifts {
            if s.had_recent_input {
                continue;
            }
            let cx = s.x + s.width / 2.0;
            let cy = s.y + s.height / 2.0;
            let col = ((cx / vw) * 3.0).floor().clamp(0.0, 2.0) as usize;
            let row = ((cy / vh) * 3.0).floor().clamp(0.0, 2.0) as usize;
            let label = format!("{}-{}", row_labels[row], col_labels[col]);
            let e = region_score.entry(label).or_insert((0.0, 0));
            e.0 += s.score;
            e.1 += 1;
            total += s.score;
        }
        let mut regions: Vec<PaintHeatmapRegion> = region_score
            .into_iter()
            .map(|(region, (score, count))| PaintHeatmapRegion {
                region,
                score,
                count,
            })
            .collect();
        regions.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let hottest_region = regions
            .first()
            .map(|r| r.region.clone())
            .unwrap_or_default();
        PaintHeatmapReport {
            regions,
            total_score: total,
            hottest_region,
        }
    }

    // 11. Z-Index Visualizer ------------------------------------------------
    pub fn visualize_zindex(&self, elements: &[StackingElement]) -> ZIndexReport {
        let mut ordered: Vec<&StackingElement> = elements.iter().collect();
        ordered.sort_by_key(|a| a.z_index.unwrap_or(0));
        let paint_order = ordered.iter().map(|e| e.selector.clone()).collect();

        // Conflicts: same z-index among sibling-level stacking contexts.
        let mut by_z: BTreeMap<i64, Vec<String>> = BTreeMap::new();
        for e in elements {
            if let Some(z) = e.z_index {
                by_z.entry(z).or_default().push(e.selector.clone());
            }
        }
        let conflicts = by_z
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(z, v)| format!("z-index {z} shared by {}", v.join(", ")))
            .collect();

        // z-index has no effect without positioning / stacking context.
        let excessive = elements
            .iter()
            .filter(|e| {
                e.z_index.map(|z| z >= 1000).unwrap_or(false)
                    || (e.z_index.is_some()
                        && e.position == "static"
                        && !e.creates_stacking_context)
            })
            .map(|e| e.selector.clone())
            .collect();

        let max_z = elements.iter().filter_map(|e| e.z_index).max().unwrap_or(0);
        ZIndexReport {
            paint_order,
            conflicts,
            excessive,
            max_z,
        }
    }

    pub fn layout_thrash_detector(&self, ops: &[LayoutOp]) -> LayoutThrashReport {
        // Thrash = read (measure) immediately followed by write (mutate) in a tight window.
        let mut thrash_windows = 0usize;
        let mut worst_burst = 0usize;
        let mut burst = 0usize;
        let mut offending: BTreeMap<String, usize> = BTreeMap::new();
        for w in ops.windows(2) {
            let a = &w[0];
            let b = &w[1];
            let read_write = (a.kind == "measure" && b.kind == "mutate")
                || (a.kind == "mutate" && b.kind == "measure");
            let tight = (b.timestamp_ms - a.timestamp_ms).abs() <= 16.0;
            if read_write && tight {
                thrash_windows += 1;
                burst += 1;
                worst_burst = worst_burst.max(burst);
                *offending.entry(a.selector.clone()).or_default() += 1;
                *offending.entry(b.selector.clone()).or_default() += 1;
            } else {
                burst = 0;
            }
        }
        let mut offending_selectors: Vec<(String, usize)> = offending.into_iter().collect();
        offending_selectors.sort_by_key(|item| std::cmp::Reverse(item.1));
        let offending_selectors: Vec<String> = offending_selectors
            .into_iter()
            .take(10)
            .map(|(s, _)| s)
            .collect();
        let score = (thrash_windows as f64) * 10.0 + (worst_burst as f64) * 5.0;
        LayoutThrashReport {
            thrash_windows,
            worst_burst,
            offending_selectors,
            score,
        }
    }

    pub fn css_containment_advisor(&self, nodes: &[ContainCandidate]) -> ContainmentReport {
        let mut advice = Vec::new();
        for n in nodes {
            if n.size_known && n.paints_independently && !n.has_layout_isolation {
                advice.push(ContainmentAdvice {
                    selector: n.selector.clone(),
                    recommend: "contain: layout paint".into(),
                    reason: "independent paint subtree with known size".into(),
                });
            } else if n.size_known && !n.has_layout_isolation {
                advice.push(ContainmentAdvice {
                    selector: n.selector.clone(),
                    recommend: "contain: layout".into(),
                    reason: "known size but layout not isolated".into(),
                });
            } else if n.paints_independently && !n.has_layout_isolation {
                advice.push(ContainmentAdvice {
                    selector: n.selector.clone(),
                    recommend: "contain: paint".into(),
                    reason: "independent paint without containment".into(),
                });
            }
        }
        ContainmentReport { advice }
    }
}
