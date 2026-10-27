---
version: 1
slug: "src-browser-cef-host-rs"
primary_target: "src/browser/cef/host.rs"
related_targets: ["src/workbench"]
---

# Browser workspace

Mode: Operate. Scope: native CEF browser shell and new-tab launch surface. API development and security investigation share web context. Preserve native action hit-testing, identity isolation, ephemeral browsing, and Rust tool boundaries. Code-first, selected by the user.

## Direction contract

THESIS: Graphite Workspace places the page, request, and evidence in one compact instrument. Content receives the space; controls receive precise grouping and visible state.

OWN-WORLD: Flat graphite #151a20, inset #101419, raised #202831, near-white #edf1f5, muted #a6b0bd, and ice blue #90b8f8. System sans for UI; monospace only for URLs, requests, and results. Fine separators, small corners, no glow.

STORY: Open a destination, work with an API, or inspect security evidence. Saved projects stay independent of browser tabs. Only implemented operations are interactive.

FIRST VIEWPORT: A native macOS title bar with red, yellow, and green window controls sits above one navigation/address row. There is no tab strip or extra toolbar. New tab has a restrained Thor heading, a URL entry, and two generous text-led workflow rows for API and security. API and Security open from the workspace. Sites and Toolbox are native searchable overlays on Cmd+K and Cmd+Shift+K; Toolbox opens the real Developer Control Center. Inspect and tab switching use native shortcuts. The Address accessible name identifies the real session identity.

FORM: Graphite Workspace, grounded candidate 1; user selected the pick after seed 92ef5f72 assigned candidate 5. One signature interaction: a focused destination row moves its arrow as its background strengthens. Motion conveys focus, 150ms, reduced-motion respected.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
