# AGENTS.md — Thor

Guidance for coding agents working in this repo. Humans: see [README.md](README.md).

## Mission

Thor is a **lighter/faster-than-Chrome** developer browser (fullstack, security, perf, LLM/AI, and Web3/blockchain). Prefer changes that reduce weight and latency over feature theater.

> **Core Positioning:**  
> *"A high-performance, ephemeral developer browser where every tab is an isolated experiment and every interaction can become a test, trace, benchmark, finding, or reproducible artifact."*

> **Architectural Principle:**  
> *"Thor is a Chromium downstream (same product model as Chrome/Brave/Edge): browser chrome + Blink/V8 in one GN/Ninja-built product. Use CDP for developer instrumentation against Thor’s own browser. Use Rust as the Labs sidecar for orchestration, evidence, FeatureEngine, and high-scale load generation."*

Hard constraints:

1. **User-facing name is always `Thor`** (not `thor`). Crate/module/binary internals: crate `thor`, binary `Thor` (product launcher / Labs) + Chromium-downstream browser binary. Sidecar binary: `thor-labs`.
2. **Product (Chromium downstream):** One Thor-owned Chromium browser process; many tabs/WebContents. **Do not** use system Google Chrome as the product surface. Build/fetch: see [docs/CHROMIUM_DOWNSTREAM.md](docs/CHROMIUM_DOWNSTREAM.md) and `scripts/{setup_depot_tools,fetch_chromium,build_chromium,run_thor_chromium,bootstrap_chromium_snapshot}.sh`. Branding overlays: `browser/`.
3. **Legacy dual-window (transitional only):** AppKit shell + CDP to a separate Chromium process (`shell/chromium_shell.rs`). Enable with `THOR_LEGACY_DUAL_WINDOW=1` (or `THOR_DUAL_WINDOW=1`). Not the long-term product. System Chrome discovery requires `THOR_ALLOW_SYSTEM_CHROME=1`.
4. **Three-Level Isolation Hierarchy** (map onto Chromium Profile / StoragePartition / WebContents — see `browser/isolation/MAPPING.md`):
   - **Level 1 (Profile)**: Settings, bookmarks, extensions, root CAs, IDE bridge. No global history DB.
   - **Level 2 (Identity Context)**: Isolated `cookies`, `localStorage`, `IndexedDB`, `ServiceWorkers`, auth tokens (`Admin`, `Standard User`, `Anonymous`, etc.). Disposable Isolated Tabs purge ephemeral identity context on tab close.
   - **Level 3 (Tab)**: Tab-scoped history list, scroll pos, trace buffer, and lifecycle. Wiped clean on tab close / 24h GC.
5. **Four-Stage Ephemeral Tab Lifecycle**:
   - `Active` (0–15m foreground) $\rightarrow$ `Frozen` (15m–1h) $\rightarrow$ `Discarded` (1h–24h RAM release) $\rightarrow$ `Expired` ($\ge$24h auto-close & tab history wipe). Prefer Chromium WebContents discard/freeze APIs over remote CDP theater.
6. **Cmd+K** = Sites menu (bookmarks + active-tab history + open tabs). **Cmd+⇧K** = Toolbox menu → **Developer Control Center** (144 engineer features across 6 domain lenses) plus product specialist tools.
7. **No Fake/Stub UI**: Prefer wiring real Rust engines via **`AppState::global()`** — never mock RNG or pretend crypto. Prefer deepen/capture wiring or honest `Analyzer (offline)` naming over Lab/Workbench theater. Do not claim CEF/Electron as “built like Chrome.”
8. **Privileged lab IPC** (Locust public, intercept, CA install, SSRF/fixation mutate, live stub/SSE labs) enforced in Rust (`net/consent`, `shell/ipc_dispatch`).
9. **Universal Evidence Engine, product lenses & Engineer Feature Suite**:
   - Top-Level Areas: `Browse`, `Inspect`, `Compare`, `Run`.
   - Product lenses: `Full-Stack`, `Security` (IDOR matrix), `Performance` (Web Vitals), `AI / LLM` (`llm_studio.rs`), `Web3 / Chain` (`web3_inspector.rs`).
   - **Engineer Feature Suite (`src/features/`)**: **144** `FeatureId`s in six domains — Frontend, Backend, Security, Performance, QA, AI — registered in `FeatureEngine` / `FeatureOrchestrator`, invoked from DevCC via `feature_list` / `feature_invoke` / `lens_payload`. Access: `AppState::global().features()`.
   - Operation: `Promote to Test` (`test_generator.rs`) exports Playwright TS, k6 JS, cURL, ThorScenario, EvalHarness, Finding, ReqwestRust, GoHttp, PythonHttpx, CI Bundle (+ oracles / hermetic / quarantine compose).
10. **Thor Load Engine (`src/load/`)**: Canonical `ThorScenario` spec with dual runners (`BrowserRunner` for Web Vitals/DOM, `ProtocolRunner` for multi-threaded Rust HTTP/WS traffic). Soft-nav / LCP media / measured transfer bytes feed `PerformanceBudgets`.

## Layout

```
browser/               # Thor Chromium branding overlays + isolation map + native messaging
docs/CHROMIUM_DOWNSTREAM.md
scripts/
  setup_depot_tools.sh
  fetch_chromium.sh
  apply_thor_branding.sh
  build_chromium.sh
  run_thor_chromium.sh
  bootstrap_chromium_snapshot.sh
chromium_manifest.json # pinned revision + snapshot URLs
src/
  main.rs              # bin Thor entry (mimalloc + shell::run → product or legacy)
  bin/thor_labs.rs     # Labs sidecar (self-CDP → FeatureEngine / MCP)
  lib.rs               # library crate thor
  paths.rs             # thor_data_dir / ChromiumProfile / cache paths
  app_state.rs         # process-wide singletons
  shell/
    mod.rs             # product launch + legacy dual-window / wry
    gui.rs             # chrome HTML/CSS/JS (toolbar + modals + 4 areas)
    chromium_shell.rs  # LEGACY AppKit + CDP dual-window
    chromium_tabs.rs   # LEGACY tab session manager
  engine/
    product.rs         # owned-Chromium discovery + product launch
    self_cdp.rs        # connect Labs to Thor’s own DevTools
    chromium_runtime.rs# CDP launch / connect / target manager
    tab_session.rs     # CDP page target & 4-stage lifecycle
    browser_features.rs
    freezing.rs
    profiles.rs
  dx/                  # Cmd+K, bookmarks, decoder, stack detect, IDE bridge
  features/            # Engineer Feature Suite (144 FeatureIds, 6 domains)
  load/                # Thor Load Engine
  net/                 # shield, tamper, locust, ssl_ca, consent
  mcp/                 # MCP server + browser tools
  daemon/prewarm.rs
  devtools/cdp_integration.rs
tests/
  integration_tests.rs
  features_*.rs
benches/feature_sla_bench.rs
```

## Runtime model

### Product (default)

1. Discover Thor-owned Chromium (`THOR_CHROMIUM_PATH`, Frameworks, `ChromiumCache`, `$THOR_CHROMIUM_SRC/src/out/Thor`).
2. Spawn that binary with Thor user-data-dir + `--remote-debugging-port` (self-CDP).
3. Attach Rust Labs (engine coordinator + MCP). UI is Chromium’s browser chrome — not a second tao content window.

```bash
./scripts/bootstrap_chromium_snapshot.sh   # owned binary without full source fetch
./scripts/run_thor_chromium.sh
cargo run --bin Thor                       # product launcher
THOR_SELF_CDP_URL=http://127.0.0.1:9222 cargo run --bin thor-labs
```

### Legacy dual-window (transitional)

```bash
THOR_LEGACY_DUAL_WINDOW=1 cargo run --bin Thor
```

Uses `ChromiumRuntime::launch_blocking` + AppKit shell. Requires owned Chromium or `THOR_ALLOW_SYSTEM_CHROME=1`.

## IPC contract (chrome → Rust)

Preserve existing cmd shapes in `shell/gui.rs` $\rightarrow$ `shell/mod.rs` for legacy UI:

`new_tab`, `switch_tab`, `close_tab`, `navigate`, `back`, `forward`, `reload`, `add_bookmark`, `remove_bookmark`, `find_in_page`, `set_zoom_level`, `toggle_devtools`, `open_ide`, `toggle_profile`, `set_interception_enabled`, `add_tamper_rule`, `locust_start` / `locust_stop`, `decode_payload`, `rescan_tech_stack`, `open_cmd_k` / `open_cmd_shift_k`, `drag_window`, `feature_list` / `feature_invoke` / `lens_payload`.

Native messaging host template: `browser/native_messaging/com.thor.labs.json`.

## Commands

```bash
cargo check
cargo test --lib
cargo test --lib features::
cargo test --test integration_tests
cargo run --bin Thor
cargo run --bin thor-labs
./scripts/run_dev.sh
./scripts/setup_depot_tools.sh && ./scripts/fetch_chromium.sh   # long
./scripts/build_chromium.sh
./scripts/run_thor_chromium.sh
```

Live Chromium smoke test is `#[ignore]` (`live_launch_smoke`) — needs Thor-owned browser. DevCC `from_active_tab` requires a live evaluator; without Chromium it must `Err` (never empty-success theater).

## Edit discipline

- Match existing style; minimal diffs.
- Don’t add markdown/docs unless asked (except when updating these files / CHROMIUM_DOWNSTREAM).
- Don’t commit unless the user asks.
- After shell/UI HTML changes: verify omnibox + action buttons still clickable.
- Prefer real CDP/HTTP behavior over alerts and simulated metrics.
- Prefer deepen existing `FeatureId`s + capture loops over minting new IDs; keep `FeatureId::ALL.len() == 144` unless intentionally expanding the registry.
- Chromium UI changes: single agent owns `chrome/browser` / `browser/` overlays — no parallel UI edits.
