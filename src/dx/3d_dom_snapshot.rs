use serde::{Deserialize, Serialize};

/// 3D DOM Layer representation for Obsidian Glass rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer3D {
    pub id: String,
    pub tag_name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub z_depth: f32,
    pub color_rgba: [f32; 4],
}

pub struct DomSnapshot3D {
    pub is_active: bool,
    pub camera_position: [f32; 3],
    pub zoom_level: f32,
    pub layers: Vec<Layer3D>,
}

impl DomSnapshot3D {
    pub fn new() -> Self {
        Self {
            is_active: false,
            camera_position: [0.0, 0.0, 5.0],
            zoom_level: 1.0,
            layers: Vec::new(),
        }
    }

    pub fn toggle_glass_mode(&mut self) {
        self.is_active = !self.is_active;
    }

    pub fn update_camera(&mut self, dx: f32, dy: f32, dz: f32) {
        self.camera_position[0] += dx;
        self.camera_position[1] += dy;
        self.camera_position[2] += dz;
    }

    /// Extract 3D layout layers from raw DOM tree JSON or snapshot
    pub fn parse_dom_layers(&mut self, html_content: &str) -> usize {
        let mut layers = Vec::new();
        let tags = ["div", "section", "article", "header", "main", "footer", "button", "input", "a", "p"];

        let mut depth_counter: f32 = 0.0;
        for token in html_content.split('<') {
            let trimmed = token.trim();
            if trimmed.is_empty() || trimmed.starts_with('!') || trimmed.starts_with('?') {
                continue;
            }
            if trimmed.starts_with('/') {
                depth_counter = (depth_counter - 1.0).max(0.0);
            } else {
                depth_counter += 1.0;
                let tag_name = trimmed.split_whitespace().next().unwrap_or("").trim_end_matches('>');
                let clean_tag = tag_name.to_lowercase();
                for tag in &tags {
                    if clean_tag == *tag || clean_tag.starts_with(&format!("{}>", tag)) {
                        layers.push(Layer3D {
                            id: format!("node_{}", layers.len()),
                            tag_name: tag.to_string(),
                            x: (layers.len() as f32 * 15.0) % 800.0,
                            y: (layers.len() as f32 * 20.0) % 600.0,
                            width: 300.0 / (depth_counter * 0.2 + 1.0),
                            height: 150.0 / (depth_counter * 0.2 + 1.0),
                            z_depth: depth_counter * 10.0,
                            color_rgba: [0.2, 0.5, 0.9, 0.85],
                        });
                        break;
                    }
                }
            }
        }

        self.layers = layers;
        self.layers.len()
    }

    /// Renders the 3D layout tree frame
    pub fn render_frame(&self) {
        if !self.is_active {
        }
        // wgpu render pass executes over self.layers
    }
}

impl Default for DomSnapshot3D {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dom_layer_extraction() {
        let html = r#"
            <main>
                <header><button>Click</button></header>
                <section><article><p>Text</p></article></section>
            </main>
        "#;
        let mut snapshot = DomSnapshot3D::new();
        let count = snapshot.parse_dom_layers(html);
        assert!(count >= 5);
        assert!(snapshot.layers.iter().any(|l| l.tag_name == "button"));
        assert!(snapshot.layers.iter().any(|l| l.z_depth > 0.0));
    }
}
