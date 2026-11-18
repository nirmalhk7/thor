use super::{DomNode, DomSnapshotDigest, FrontendEngine};
use std::collections::BTreeMap;

impl FrontendEngine {
    pub fn dom_snapshot_capture(&self, root: &DomNode) -> DomSnapshotDigest {
        let mut snapshot = Snapshot::default();
        snapshot.walk(root, 0);
        snapshot.finish()
    }
}

#[derive(Default)]
struct Snapshot {
    tag_histogram: BTreeMap<String, usize>,
    text_bytes: usize,
    attr_bytes: usize,
    node_count: usize,
    max_depth: usize,
}

impl Snapshot {
    fn walk(&mut self, node: &DomNode, depth: usize) {
        self.node_count += 1;
        self.max_depth = self.max_depth.max(depth);
        *self
            .tag_histogram
            .entry(node.tag.to_lowercase())
            .or_default() += 1;
        if let Some(text) = &node.text {
            self.text_bytes += text.len();
        }
        for (key, value) in &node.attributes {
            self.attr_bytes += key.len() + value.len();
        }
        for child in &node.children {
            self.walk(child, depth + 1);
        }
    }

    fn finish(self) -> DomSnapshotDigest {
        let mut hash: u64 = 0xcbf29ce484222325;
        for (tag, count) in &self.tag_histogram {
            for byte in tag.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash ^= *count as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= self.node_count as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        DomSnapshotDigest {
            node_count: self.node_count,
            max_depth: self.max_depth,
            tag_histogram: self.tag_histogram,
            text_bytes: self.text_bytes,
            attr_bytes: self.attr_bytes,
            digest_hex: format!("{hash:016x}"),
        }
    }
}
