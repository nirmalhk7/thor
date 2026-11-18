use super::{
    ElementBox, FrontendEngine, MediaCqReport, MediaQueryCapture, ResponsiveIssue,
    ResponsiveReport, Viewport,
};

impl FrontendEngine {
    // 2. Responsive Matrix --------------------------------------------------
    pub fn standard_viewports() -> Vec<Viewport> {
        vec![
            Viewport {
                label: "mobile".into(),
                width: 375.0,
                height: 667.0,
            },
            Viewport {
                label: "tablet".into(),
                width: 768.0,
                height: 1024.0,
            },
            Viewport {
                label: "laptop".into(),
                width: 1366.0,
                height: 768.0,
            },
            Viewport {
                label: "desktop".into(),
                width: 1920.0,
                height: 1080.0,
            },
        ]
    }

    pub fn responsive_matrix(&self, captures: &[(Viewport, Vec<ElementBox>)]) -> ResponsiveReport {
        let mut issues = Vec::new();
        for (vp, boxes) in captures {
            for b in boxes {
                if b.x + b.width > vp.width + 0.5 {
                    issues.push(ResponsiveIssue {
                        viewport: vp.label.clone(),
                        selector: b.selector.clone(),
                        kind: "horizontal-overflow".into(),
                        detail: format!(
                            "right edge {:.0}px exceeds viewport {:.0}px",
                            b.x + b.width,
                            vp.width
                        ),
                    });
                }
                if b.font_size_px > 0.0 && b.font_size_px < 12.0 && vp.width <= 480.0 {
                    issues.push(ResponsiveIssue {
                        viewport: vp.label.clone(),
                        selector: b.selector.clone(),
                        kind: "tiny-text".into(),
                        detail: format!(
                            "font-size {:.1}px below 12px on small viewport",
                            b.font_size_px
                        ),
                    });
                }
                // Touch target minimum 44x44 per WCAG 2.5.5 on mobile.
                if vp.width <= 480.0 && (b.width < 44.0 || b.height < 44.0) && b.width > 0.0 {
                    issues.push(ResponsiveIssue {
                        viewport: vp.label.clone(),
                        selector: b.selector.clone(),
                        kind: "small-touch-target".into(),
                        detail: format!(
                            "{:.0}x{:.0}px below 44x44 touch target",
                            b.width, b.height
                        ),
                    });
                }
            }
        }
        ResponsiveReport {
            issues,
            viewports_tested: captures.len(),
        }
    }

    pub fn media_cq_matrix(&self, captures: &[MediaQueryCapture]) -> MediaCqReport {
        let active_count = captures.iter().filter(|c| c.matches).count();
        let container_queries = captures
            .iter()
            .filter(|c| c.query.contains("container") || c.query.contains("cq-"))
            .count();
        let mut issues = Vec::new();
        for c in captures {
            if c.matches && c.applied_rules == 0 {
                issues.push(format!("query matches but applies 0 rules: {}", c.query));
            }
        }
        if captures.len() > 40 {
            issues.push(format!(
                "high media-query count ({}) may indicate breakpoint sprawl",
                captures.len()
            ));
        }
        MediaCqReport {
            queries: captures.to_vec(),
            active_count,
            container_queries,
            issues,
        }
    }
}
