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
| Full Xcode | Required; Command Line Tools alone cannot configure Chromium |
| depot_tools | ~100 MB |
| `fetch chromium` + hooks | 50–100 GB |
| First `autoninja -C out/Thor chrome` | multi-hour |
| Incremental rebuilds | minutes |

Set `THOR_CHROMIUM_SRC` to the parent of `src/` (default: `$HOME/thor-chromium`). Keep the checkout **outside** the Rust git worktree when possible.

On macOS, launch Xcode once and complete Apple's license/setup flow before building. Thor's scripts only detect this prerequisite; they never accept the license automatically.

Tagged releases use a self-hosted macOS runner labeled `thor-chromium-builder` with at least 100 GiB free. Configure `CODESIGN_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID` secrets; release packaging fails closed if signing/notarization credentials are absent. Linux and Windows release jobs stay gated until equivalent Thor-owned Chromium downstream build/package pipelines exist; Rust-launcher-only archives are not browser releases.

## Pin

The Chromium source pin and Thor feature policy live in [`thor_browser_manifest.json`](../thor_browser_manifest.json). The build pipeline checks out Chromium directly and applies only Thor-owned branding and configuration. Stock Chromium snapshots are intentionally disabled for packages because they lack that branding and configuration.

`fetch_chromium.sh` materializes the configured tag under `refs/tags/`, checks it against the manifest's exact peeled commit, checks out that commit, and verifies that `gclient sync` did not move `HEAD`. Apply/build/package steps then require that exact commit, origin, numbered patch set, GN input, staged assets, and provenance marker. A matching `chrome/VERSION` alone is not sufficient.

`out/Thor/ThorOwnedChromium.json` is generated after source validation, then sealed to the built browser executable after Ninja completes. It records the exact repository/ref/commit, ordered SHA-256 digests for every numbered patch, `browser/gn_args.thor.gn`, the complete Shield asset tree, `rules.json`, and the executable digest/size/name. Packaging copies that validated marker—not the source manifest—into the application bundle and recomputes both input and executable digests before release.

## Commands

```bash
# Fetch Thor's pinned Chromium source (long)
./scripts/fetch_chromium.sh

# Apply Thor branding + configure out/Thor
./scripts/apply_thor_branding.sh
python3 ./scripts/privacy_regression_check.py \
  --chromium-src "$THOR_CHROMIUM_SRC/src" --patch-state applied
./scripts/build_chromium.sh          # chrome + chromedriver
./scripts/verify_thor_downstream.sh
./scripts/package_macos.sh 0.1.0
./scripts/verify_macos_package.sh 0.1.0

# Run branded browser (no system Chrome)
./scripts/run_thor_chromium.sh https://example.com

# Rust Labs sidecar (features / MCP / load) against self-CDP
THOR_SELF_CDP_URL=http://127.0.0.1:9222 cargo run --bin thor-labs
```

The browserless privacy contract can run without Chromium:

```bash
python3 ./scripts/privacy_regression_check.py
```

It validates manifest/patch/GN/release-policy invariants. It does not replace the fresh-profile runtime tests listed in [`PRIVACY_SECURITY_PLAN.md`](PRIVACY_SECURITY_PLAN.md).

Run the ignored live acceptance suite against an exact-pin Thor-owned executable:

```bash
./scripts/run_privacy_acceptance.sh /absolute/path/to/Thor
```

The wrapper verifies downstream provenance before launch, then exercises startup, privacy-signal readback, WebRTC policy, isolated browser contexts, and disposable-context cleanup. Explicit live selection fails closed when the executable or provenance is missing; ordinary `cargo test` remains browserless.

The pinned macOS release args explicitly retain Chromium official optimization, ThinLTO, PartitionAlloc-as-malloc, and BackupRefPtr support/activation. All are declared by Chromium 151 and checked by the source-aware contract. Thor does not force Chromium's CFI argument on macOS: Chromium 151's own declaration enables official CFI only for Linux x64 and ChromeOS device builds.

## Isolation mapping (Levels → Chromium)

| Thor level | Chromium primitive |
|------------|-------------------|
| L1 Profile | `Profile` + user-data-dir under Thor Application Support |
| L2 Identity Context | Separate `StoragePartition` / OTR profile / isolated browser context |
| L3 Tab | `WebContents` + tab-scoped session restore metadata |

Lifecycle: Active → Frozen (`WebContents::WasHidden` / freeze APIs) → Discarded (tab discard / memory pressure) → Expired (close + wipe tab history). Prefer Chromium primitives over remote `Page.stopLoading` theater.

## Branding overlays

Patches and resources under [`browser/`](../browser/). Applied onto `$THOR_CHROMIUM_SRC/src` by `apply_thor_branding.sh` before build.
