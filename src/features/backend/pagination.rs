use super::{ApiRequest, BackendEngine, HashMap, PageSlice, PaginationReport};

impl BackendEngine {
    pub fn pagination_auditor(&self, pages: &[PageSlice]) -> PaginationReport {
        let mut issues = Vec::new();
        let mut estimated_total = None;
        for (i, p) in pages.iter().enumerate() {
            if p.page_size == 0 {
                issues.push(format!("page {i}: zero page_size"));
            }
            if p.items > p.page_size as usize {
                issues.push(format!(
                    "page {}: items {} > page_size {}",
                    p.page, p.items, p.page_size
                ));
            }
            if let Some(t) = p.total {
                estimated_total = Some(t);
            }
            if i + 1 < pages.len() {
                let n = &pages[i + 1];
                if p.next_cursor.is_none()
                    && n.page != p.page + 1
                    && p.page_size > 0
                    && n.page != p.page + 1
                {
                    issues.push(format!("non-sequential page jump {} -> {}", p.page, n.page));
                }
            }
        }
        if let Some(last) = pages.last() {
            if last.items == last.page_size as usize
                && last.next_cursor.is_none()
                && last.total.is_none()
            {
                issues.push(
                    "last page full-sized without next cursor/total — may be truncated".into(),
                );
            }
        }
        PaginationReport {
            pages: pages.len(),
            issues,
            estimated_total,
        }
    }

    /// Fetch paginated URLs via reqwest and analyze JSON arrays / `items` fields.
    pub fn pagination_auditor_http(
        &self,
        urls: &[String],
        page_size_hint: u32,
        lab_consent: bool,
    ) -> Result<PaginationReport, String> {
        for u in urls {
            crate::net::consent::require_lab_consent(u, lab_consent)?;
        }
        let mut pages = Vec::new();
        for (i, url) in urls.iter().enumerate() {
            let resp = self.send_blocking(&ApiRequest {
                method: "GET".into(),
                url: url.clone(),
                headers: HashMap::new(),
                body: None,
            })?;
            let items = count_page_items(&resp.body);
            let total = serde_json::from_str::<serde_json::Value>(&resp.body)
                .ok()
                .and_then(|v| {
                    v.get("total")
                        .or_else(|| v.get("count"))
                        .and_then(|t| t.as_u64())
                });
            let next_cursor = serde_json::from_str::<serde_json::Value>(&resp.body)
                .ok()
                .and_then(|v| {
                    v.get("next")
                        .or_else(|| v.get("next_cursor"))
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                });
            pages.push(PageSlice {
                page: (i as u32) + 1,
                page_size: page_size_hint.max(1),
                items,
                total,
                next_cursor,
            });
        }
        Ok(self.pagination_auditor(&pages))
    }
}
fn count_page_items(body: &str) -> usize {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return 0;
    };
    if let Some(arr) = v.as_array() {
        return arr.len();
    }
    for key in ["items", "data", "results", "records"] {
        if let Some(arr) = v.get(key).and_then(|x| x.as_array()) {
            return arr.len();
        }
    }
    0
}
