use super::{CompressionBudgetReport, CompressionEntry, CompressionFlag, PerfEngine};

impl PerfEngine {
    // 22. Compression Budget ------------------------------------------------
    pub fn compression_budget(
        &self,
        entries: &[CompressionEntry],
        max_inflation_ratio: f64,
    ) -> CompressionBudgetReport {
        let total_transfer_bytes: u64 = entries.iter().map(|e| e.transfer_bytes).sum();
        let total_decoded_bytes: u64 = entries.iter().map(|e| e.decoded_bytes).sum();
        let inflation_ratio = if total_transfer_bytes > 0 {
            total_decoded_bytes as f64 / total_transfer_bytes as f64
        } else {
            0.0
        };
        let mut weak_encoding = Vec::new();
        for entry in entries {
            compression_flags(entry, max_inflation_ratio, &mut weak_encoding);
        }
        let passed = inflation_ratio <= max_inflation_ratio && weak_encoding.is_empty();
        CompressionBudgetReport {
            entries: entries.len(),
            total_transfer_bytes,
            total_decoded_bytes,
            inflation_ratio,
            weak_encoding,
            passed,
        }
    }
}

fn compression_flags(
    entry: &CompressionEntry,
    max_inflation_ratio: f64,
    flags: &mut Vec<CompressionFlag>,
) {
    let encoding = entry
        .content_encoding
        .as_deref()
        .unwrap_or("")
        .to_lowercase();
    let content_type = entry.content_type.as_deref().unwrap_or("").to_lowercase();
    if let Some(reason) = encoding_reason(entry, &encoding, &content_type) {
        flags.push(CompressionFlag {
            url: entry.url.clone(),
            reason: reason.into(),
        });
    }
    if entry.transfer_bytes > 0 {
        let ratio = entry.decoded_bytes as f64 / entry.transfer_bytes as f64;
        if ratio > max_inflation_ratio.max(1.0) * 1.5 && encoding.is_empty() {
            flags.push(CompressionFlag {
                url: entry.url.clone(),
                reason: format!("decoded/transfer ratio {ratio:.1} without encoding"),
            });
        }
    }
}

fn encoding_reason(
    entry: &CompressionEntry,
    encoding: &str,
    content_type: &str,
) -> Option<&'static str> {
    let compressible = ["text/", "javascript", "json", "css", "svg", "xml"]
        .iter()
        .any(|kind| content_type.contains(kind));
    if !compressible || entry.transfer_bytes <= 1024 {
        return None;
    }
    if encoding.is_empty() || encoding == "identity" {
        Some("missing content-encoding on compressible asset")
    } else if encoding == "gzip" && entry.decoded_bytes > 50_000 {
        Some("gzip on large payload; prefer br/zstd")
    } else {
        None
    }
}
