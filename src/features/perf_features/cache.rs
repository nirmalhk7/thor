use super::{CacheEntry, CacheReport, PerfEngine};

impl PerfEngine {
    // 9. CDN Cache ----------------------------------------------------------
    pub fn analyze_cache(&self, entries: &[CacheEntry]) -> CacheReport {
        let mut hits = 0;
        let mut hit_eligible = 0;
        let mut uncacheable = Vec::new();
        let mut misconfigured = Vec::new();
        for entry in entries {
            let (entry_hits, eligible) =
                analyze_cache_entry(entry, &mut uncacheable, &mut misconfigured);
            hits += entry_hits;
            hit_eligible += eligible;
        }
        let hit_ratio = if hit_eligible > 0 {
            hits as f64 / hit_eligible as f64
        } else {
            0.0
        };
        CacheReport {
            hit_ratio,
            uncacheable,
            misconfigured,
        }
    }
}
fn parse_max_age(cache_control: &str) -> Option<u64> {
    cache_control
        .split(',')
        .find_map(|d| d.trim().strip_prefix("max-age="))
        .and_then(|v| v.trim().parse::<u64>().ok())
}

fn analyze_cache_entry(
    entry: &CacheEntry,
    uncacheable: &mut Vec<String>,
    misconfigured: &mut Vec<String>,
) -> (i32, i32) {
    let cc = entry.cache_control.as_deref().unwrap_or("").to_lowercase();
    let is_uncacheable = cc.is_empty()
        || ["no-store", "no-cache", "max-age=0"]
            .iter()
            .any(|token| cc.contains(token));
    if is_uncacheable {
        uncacheable.push(entry.url.clone());
    }
    let is_hit = entry
        .x_cache
        .as_deref()
        .map(|header| header.to_uppercase().contains("HIT"))
        .unwrap_or(false);
    if is_hit && is_uncacheable {
        misconfigured.push(format!(
            "{} served from cache but Cache-Control is non-cacheable",
            entry.url
        ));
    }
    check_cache_age(entry, &cc, misconfigured);
    (i32::from(is_hit), i32::from(entry.x_cache.is_some()))
}

fn check_cache_age(entry: &CacheEntry, cache_control: &str, issues: &mut Vec<String>) {
    if let (Some(age), Some(max_age)) = (entry.age_secs, parse_max_age(cache_control)) {
        if age > max_age {
            issues.push(format!(
                "{} Age {age}s exceeds max-age {max_age}s",
                entry.url
            ));
        }
    }
}
