# AGENTS.md — Thor

Guidance for coding agents working in this repo. Humans: see [README.md](README.md).

## Mission

Thor is a **lighter/faster-than-Chrome** developer browser (fullstack, security, perf, LLM/AI, and Web3/blockchain). Prefer changes that reduce weight and latency over feature theater.

> **Core Positioning:**  
> *"A high-performance, ephemeral developer browser where every tab is an isolated experiment and every interaction can become a test, trace, benchmark, finding, or reproducible artifact."*

> **Architectural Principle:**  
> *"Use Chromium internals for browser semantics. Use CDP for developer instrumentation. Use Rust for orchestration, event indexing, processing, and high-scale load generation."*

Hard constraints:

1. **User-facing name is always `Thor`** (not `thor`). Crate/module/binary internals: crate `thor`, binary `Thor`.
2. **M2 (live):** One Chromium browser process for content; many CDP targets/tabs. Do not spawn Chrome-per-tab. Dual-window until CEF. Legacy wry WebKit content path is unused by `shell::run()`.
3. **Three-Level Isolation Hierarchy**:
   - **Level 1 (Profile)**: Settings, bookmarks, extensions, root CAs, IDE bridge. No global history DB.
   - **Level 2 (Identity Context)**: Isolated `cookies`, `localStorage`, `IndexedDB`, `ServiceWorkers`, auth tokens (`Admin`, `Standard User`, `Anonymous`, etc.). Disposable Isolated Tabs purge ephemeral identity context on tab close.
   - **Level 3 (Tab)**: Tab-scoped history list, scroll pos, trace buffer, and lifecycle. Wiped clean on tab close / 24h GC.
4. **Four-Stage Ephemeral Tab Lifecycle**:
   - `Active` (0–15m foreground) $\rightarrow$ `Frozen` (15m–1h CDP stop) $\rightarrow$ `Discarded` (1h–24h RAM target detach & memory release) $\rightarrow$ `Expired` ($\ge$24h auto-close & tab history wipe).
5. **Cmd+K** = bookmarks + active-tab history + open tabs. **Cmd+⇧K** = Developer Control Center & 5 Specialist Lenses.
6. **No Fake/Stub UI**: Prefer wiring real Rust engines via **`AppState::global()`** — never mock RNG or pretend crypto.
7. **Privileged lab IPC** (Locust public, intercept, CA install) enforced in Rust (`net/consent`, `shell/ipc_dispatch`).
8. **Universal Evidence Engine & 5 Specialist Lenses**:
   - Top-Level Areas: `Browse`, `Inspect`, `Compare`, `Run`.
   - Lenses: `Full-Stack`, `Security` (IDOR matrix), `Performance` (Web Vitals), `AI / LLM` (`llm_studio.rs`), `Web3 / Chain` (`web3_inspector.rs`).
   - Operation: `Promote to Test` (`test_generator.rs`) exports Playwright TS, k6 JS, or cURL.
9. **Thor Load Engine (`src/load/`)**: Canonical `ThorScenario` spec with dual runners (`BrowserRunner` for Web Vitals/DOM, `ProtocolRunner` for multi-threaded Rust HTTP/WS traffic).

## Layout

```
src/
  main.rs              # bin Thor entry (mimalloc + shell::run)
  lib.rs               # library crate thor
  paths.rs             # thor_data_dir / ChromiumProfile paths
  app_state.rs         # process-wide singletons
  shell/
    mod.rs             # window loop, IPC → Chromium TabSession
    gui.rs             # chrome HTML/CSS/JS (toolbar + modals + 4 areas)
    chromium_shell.rs  # AppKit window & tab strip
    chromium_tabs.rs   # tab session manager & 3-level isolation
  engine/
    chromium_runtime.rs# CDP process launcher & target manager
    tab_session.rs     # CDP page target & 4-stage lifecycle
    browser_features.rs# WebContents::Find, HostZoomMap, DownloadManager, Favicons, CORS
    freezing.rs        # CDP page freeze & memory discard
    profiles.rs        # DevHost / StealthTarget
  dx/                  # Cmd+K, bookmarks, decoder, stack detect, IDE bridge
    llm_studio.rs      # AI/LLM token counter, API cost calc, SSE TTFT parser
    web3_inspector.rs  # EVM/Solana JSON-RPC parser, 4-byte ABI decoder, EIP-55 validator
    test_generator.rs  # 'Promote to Test' exporter (Playwright, k6, cURL)
  load/                # Thor Load Engine
    scenario.rs        # ThorScenario spec & step definitions
    compiler.rs        # Scenario compiler & network trace converter
    metrics.rs         # Metrics collector (p50/p90/p99 latency, Web Vitals)
    browser_runner.rs  # Chromium target Web Vitals runner
    protocol_runner.rs # Multi-threaded Rust HTTP/WS scale runner
  net/                 # shield, tamper, locust, ssl_ca
  daemon/prewarm.rs
  devtools/cdp_integration.rs
tests/integration_tests.rs
```

## Runtime model (M2 dual-window)

- **tao + AppKit** = Thor chrome only (omnibar, traffic lights, tool modes, posh micro-animations).
- **Chromium** = headful content via `ChromiumRuntime` + `TabSession` (CDP), managed by `shell/chromium_tabs.rs`.

Shell lifecycle execution:

```rust
ChromiumRuntime::launch_blocking(ChromiumRuntimeConfig { headful: true, prewarm_pool_size: 2, ..Default::default() })
rt.acquire_tab_blocking()?  // prefers prewarm
tab.navigate_blocking(url)?
// 15m idle: soft-freeze via CDP (Page.stopLoading)
// 1h idle: RAM offload (CDP target detach & memory release)
// 24h idle GC: close TabSession & wipe tab-scoped history
```

## IPC contract (chrome → Rust)

Preserve existing cmd shapes in `shell/gui.rs` $\rightarrow$ `shell/mod.rs`:

`new_tab`, `switch_tab`, `close_tab`, `navigate`, `back`, `forward`, `reload`, `add_bookmark`, `remove_bookmark`, `find_in_page`, `set_zoom_level`, `toggle_devtools`, `open_ide`, `toggle_profile`, `set_interception_enabled`, `add_tamper_rule`, `locust_start` / `locust_stop`, `decode_payload`, `rescan_tech_stack`, `open_cmd_k` / `open_cmd_shift_k`, `drag_window`.

Rust $\rightarrow$ JS hooks (evaluate in chrome webview):

- `updateTabUrl` / `updateTabTitle`
- `__thorUpdateLocustStats`, `__thorUpdateDecodeResult`, `__thorUpdateTechStack`
- `__thorRemoveTab` (idle GC)
- `__thorCmdKPayload` / `__thorSettingsPayload`

## Commands

```bash
cargo check
cargo test --lib
cargo test --test integration_tests
cargo run --bin Thor
./scripts/run_dev.sh
```

Live Chromium smoke test is `#[ignore]` (`live_launch_smoke`) — needs a real browser.

## Edit discipline

- Match existing style; minimal diffs.
- Don’t add markdown/docs unless asked (except when updating these files).
- Don’t commit unless the user asks.
- After shell/UI HTML changes: verify omnibox + action buttons still clickable.
- Prefer real CDP/HTTP behavior over alerts and simulated metrics.
