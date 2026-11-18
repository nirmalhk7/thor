use super::*;

#[test]
fn image_capture_keeps_substring_detection_mime_override_and_format_priority() {
    let bridge = CdpBridge::new();
    let entries = [
        NetworkEntry {
            url: "https://test/path?next=.PNG&format=.WEBP".into(),
            bytes: 17,
            ..Default::default()
        },
        NetworkEntry {
            url: "https://test/a.jpeg".into(),
            bytes: 20,
            response_headers: HashMap::from([(
                "Content-Type".into(),
                "IMAGE/SVG+XML; charset=utf-8".into(),
            )]),
            ..Default::default()
        },
        NetworkEntry {
            url: "https://test/plain".into(),
            bytes: 31,
            response_headers: HashMap::from([("content-type".into(), "image/avif".into())]),
            ..Default::default()
        },
        NetworkEntry {
            url: "https://test/not-image".into(),
            ..Default::default()
        },
    ];
    for entry in entries {
        bridge.ingest_network_entry(entry);
    }
    let images = bridge.images_from_network();
    assert_eq!(images.len(), 3);
    assert_eq!(
        images
            .iter()
            .map(|image| image.format.as_str())
            .collect::<Vec<_>>(),
        ["webp", "svg+xml", "avif"]
    );
    assert_eq!(
        images.iter().map(|image| image.bytes).collect::<Vec<_>>(),
        [17, 20, 31]
    );
    assert!(images
        .iter()
        .all(|image| image.natural_w == 0 && image.natural_h == 0));
}
