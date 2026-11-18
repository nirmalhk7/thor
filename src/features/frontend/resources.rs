use super::{
    EventListenerEntry, EventListenerReport, FontAuditReport, FontFace, FrontendEngine, ImageAsset,
    ImageDeliveryFinding, ImageDeliveryReport,
};
use std::collections::BTreeMap;

impl FrontendEngine {
    // 10. Font Auditor ------------------------------------------------------
    pub fn audit_fonts(&self, faces: &[FontFace], used_families: &[String]) -> FontAuditReport {
        let used: std::collections::HashSet<String> =
            used_families.iter().map(|f| f.to_lowercase()).collect();
        let mut unused = Vec::new();
        let mut missing_swap = Vec::new();
        let mut total_bytes = 0u64;
        for f in faces {
            total_bytes += f.bytes;
            if !used.contains(&f.family.to_lowercase()) {
                unused.push(format!("{} {}", f.family, f.weight));
            }
            let display_ok = f
                .font_display
                .as_deref()
                .map(|d| matches!(d, "swap" | "optional" | "fallback"))
                .unwrap_or(false);
            if !display_ok {
                missing_swap.push(format!("{} {}", f.family, f.weight));
            }
        }
        FontAuditReport {
            unused_families: unused,
            missing_swap,
            total_bytes,
            weight_variants: faces.len(),
        }
    }

    pub fn image_delivery_auditor(&self, images: &[ImageAsset]) -> ImageDeliveryReport {
        let mut findings = Vec::new();
        let mut oversized = 0usize;
        let mut total_bytes = 0u64;
        for img in images {
            total_bytes += img.bytes;
            if img.display_w > 0.0 && img.natural_w as f64 > img.display_w * 2.0 {
                oversized += 1;
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "oversized-dimensions".into(),
                    detail: format!(
                        "natural {}px vs display {:.0}px (>2x)",
                        img.natural_w, img.display_w
                    ),
                });
            }
            if img.bytes > 200_000 {
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "heavy-bytes".into(),
                    detail: format!("{} bytes", img.bytes),
                });
            }
            let fmt = img.format.to_lowercase();
            if matches!(fmt.as_str(), "png" | "bmp" | "tiff") && img.bytes > 50_000 {
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "format".into(),
                    detail: format!("{fmt} candidate for webp/avif"),
                });
            }
            if !img.has_srcset && img.display_w > 0.0 {
                findings.push(ImageDeliveryFinding {
                    url: img.url.clone(),
                    kind: "missing-srcset".into(),
                    detail: "no responsive srcset".into(),
                });
            }
            if img.loading != "lazy" && img.display_w > 0.0 {
                // only flag below-fold heuristic via large y would need more data; skip
            }
        }
        ImageDeliveryReport {
            findings,
            total_bytes,
            oversized_count: oversized,
        }
    }

    pub fn event_listener_map(&self, entries: &[EventListenerEntry]) -> EventListenerReport {
        let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
        let mut non_passive_scroll = Vec::new();
        let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
        for e in entries {
            *by_type.entry(e.event_type.clone()).or_default() += 1;
            *seen
                .entry((e.selector.clone(), e.event_type.clone()))
                .or_default() += 1;
            if matches!(e.event_type.as_str(), "scroll" | "touchstart" | "wheel") && !e.passive {
                non_passive_scroll.push(format!("{}@{}", e.selector, e.event_type));
            }
        }
        let duplicate_bindings: Vec<String> = seen
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .map(|((sel, ev), c)| format!("{sel}@{ev} x{c}"))
            .collect();
        EventListenerReport {
            by_type,
            non_passive_scroll,
            duplicate_bindings,
            total: entries.len(),
        }
    }
}
