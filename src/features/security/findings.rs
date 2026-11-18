use super::{json, NetworkTrace, RecordedSession, SecurityEngine, TestGenerator};

impl SecurityEngine {
    // 5. Promote to Finding -------------------------------------------------
    /// Build a Markdown finding document from observed request/response traces,
    /// reusing the Universal Test Generator's finding exporter.
    pub fn promote_to_finding(
        &self,
        title: &str,
        target: &str,
        traces: Vec<NetworkTrace>,
    ) -> String {
        let mut session = RecordedSession::new(title, target);
        session.traces = traces;
        TestGenerator::export_finding(&session)
    }

    /// Build a full promote-to-finding bundle: a Markdown report, a HAR-like JSON
    /// entry list, and a reproducible cURL script — everything needed to file the
    /// finding and reproduce it.
    pub fn promote_to_finding_bundle(
        &self,
        title: &str,
        target: &str,
        traces: Vec<NetworkTrace>,
    ) -> serde_json::Value {
        let markdown = self.promote_to_finding(title, target, traces.clone());
        let mut session = RecordedSession::new(title, target);
        session.traces = traces.clone();
        let curl = TestGenerator::export_curl(&session);
        let entries: Vec<serde_json::Value> = traces
            .iter()
            .map(|t| {
                json!({
                    "request": {
                        "method": t.method,
                        "url": t.url,
                        "headers": t.headers,
                        "postData": t.body,
                    },
                    "response": { "status": t.status },
                })
            })
            .collect();
        let har = json!({
            "log": {
                "version": "1.2",
                "creator": { "name": "Thor", "version": "0.1.1" },
                "entries": entries,
            }
        });
        json!({
            "title": title,
            "target": target,
            "markdown": markdown,
            "curl": curl,
            "har": har,
        })
    }
}
