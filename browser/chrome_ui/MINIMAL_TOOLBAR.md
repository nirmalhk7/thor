# Minimal Thor chrome (Phase 2 — in Chromium tree)

Target: single window with Thor-branded toolbar inside Chromium Views/Cocoa — not tao.

## Must-have controls

1. Tab strip — new / close / switch
2. Omnibox — navigate on Enter; show URL / security chip
3. Back / forward / reload
4. Window title / menu: “Thor”
5. Optional: Cmd+K / Cmd+⇧K host that forwards to Labs via native messaging

## Implementation notes (once `$THOR_CHROMIUM_SRC/src` exists)

- Branding strings: `chrome/app/theme/thor/BRANDING` (copied by `apply_thor_branding.sh`)
- Start from Chromium `BrowserView` + `ToolbarView`; replace product name resources
- Do **not** spawn a second OS window for content
- Remote debugging stays enabled for Labs (`--remote-debugging-port`)

## Acceptance

- One visible app window
- Navigate / new tab / reload without system Google Chrome
- Activity Monitor process name is Thor/Chromium from `out/Thor`, not “Google Chrome”
