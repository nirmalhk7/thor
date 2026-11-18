use super::{dhash, QaEngine, SnapshotLayer, SnapshotManifest};

impl QaEngine {
    /// Multi-layer snapshot manifesto (pixels / DOM / a11y / body hashes).
    pub fn snapshot_strategy_pack(
        &self,
        name: &str,
        pixels: &[u8],
        dom: &str,
        a11y_digest: &str,
        body: &str,
    ) -> SnapshotManifest {
        let layers = vec![
            SnapshotLayer {
                name: "pixels".into(),
                strategy: "grayscale-dhash".into(),
                hash: format!("{:016x}", dhash(pixels)),
            },
            SnapshotLayer {
                name: "dom".into(),
                strategy: "dom-digest".into(),
                hash: format!("{:016x}", dhash(dom.as_bytes())),
            },
            SnapshotLayer {
                name: "a11y".into(),
                strategy: "a11y-tree-hash".into(),
                hash: format!("{:016x}", dhash(a11y_digest.as_bytes())),
            },
            SnapshotLayer {
                name: "body".into(),
                strategy: "response-body-hash".into(),
                hash: format!("{:016x}", dhash(body.as_bytes())),
            },
        ];
        let manifesto = serde_json::json!({
            "name": name,
            "layers": layers,
        });
        SnapshotManifest {
            name: name.to_string(),
            layers,
            manifesto_json: serde_json::to_string_pretty(&manifesto).unwrap_or_default(),
        }
    }
}
