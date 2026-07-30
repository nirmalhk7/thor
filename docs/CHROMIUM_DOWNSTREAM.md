# Thor Chromium Downstream

Thor is a **Chromium downstream browser** (same product model as Chrome / Brave / Edge): one GN/Ninja-built browser binary owns window chrome + Blink/V8. Rust (`thor` crate / Labs sidecar) owns load generation, evidence, FeatureEngine analyzers, consent, and MCP — not the window host that launches system Google Chrome.

## Product truth

| Role | Owner |
|------|--------|
| Window, tabs, omnibox, find, zoom, downloads, DevTools host | Chromium `chrome/` (Thor-branded) |
| Profile / cookie / storage partitions (isolation Levels 1–2) | Chromium Profile / StoragePartition |
| Tab freeze / discard / GC | Chromium WebContents + memory APIs |
| Protocol load, exporters, CA, consent, FeatureEngine offline | Rust Labs sidecar |
| Developer instrumentation | CDP attached to **Thor’s own** browser |

**Legacy dual-window** (AppKit shell + CDP to a separate Chromium process) is transitional. Enable with `THOR_LEGACY_DUAL_WINDOW=1`. It must never be described as the long-term product.

## Disk / time (macOS arm64)

| Step | Approx. |
|------|---------|
| depot_tools | ~100 MB |
| `fetch chromium` + hooks | 50–100 GB |
| First `autoninja -C out/Thor chrome` | multi-hour |
| Incremental rebuilds | minutes |

Set `THOR_CHROMIUM_SRC` to the parent of `src/` (default: `$HOME/thor-chromium`). Keep the checkout **outside** the Rust git worktree when possible.

## Pin

Revision and snapshot metadata live in [`chromium_manifest.json`](../chromium_manifest.json). Source builds use `chromium_src_revision` (git hash or LKGR tag). Snapshot ZIPs remain a bootstrap cache only — not the product end-state.

## Commands

```bash
# One-time tooling
./scripts/setup_depot_tools.sh

# Fetch pinned Chromium (long)
./scripts/fetch_chromium.sh

# Apply Thor branding overlays + configure out/Thor
./scripts/apply_thor_branding.sh
./scripts/build_chromium.sh          # content_shell first, then chrome

# Run branded browser (no system Chrome)
./scripts/run_thor_chromium.sh https://example.com

# Rust Labs sidecar (features / MCP / load) against self-CDP
THOR_SELF_CDP_URL=http://127.0.0.1:9222 cargo run --bin thor-labs
```

## Isolation mapping (Levels → Chromium)

| Thor level | Chromium primitive |
|------------|-------------------|
| L1 Profile | `Profile` + user-data-dir under Thor Application Support |
| L2 Identity Context | Separate `StoragePartition` / OTR profile / isolated browser context |
| L3 Tab | `WebContents` + tab-scoped session restore metadata |

Lifecycle: Active → Frozen (`WebContents::WasHidden` / freeze APIs) → Discarded (tab discard / memory pressure) → Expired (close + wipe tab history). Prefer Chromium primitives over remote `Page.stopLoading` theater.

## Branding overlays

Patches and resources under [`browser/`](../browser/). Applied onto `$THOR_CHROMIUM_SRC/src` by `apply_thor_branding.sh` before build.
