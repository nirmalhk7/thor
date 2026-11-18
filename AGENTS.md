# AGENTS.md — Thor

Guidance for coding agents working in this repo. Humans: see [README.md](README.md).

## Mission

Thor is a **lighter/faster-than-Chrome** developer browser (fullstack, security, perf, LLM/AI, and Web3/blockchain). Prefer changes that reduce weight and latency over feature theater.

> **Core Positioning:**  
> *"A high-performance, ephemeral developer browser where every tab is an isolated experiment and every interaction can become a test, trace, benchmark, finding, or reproducible artifact."*

> **Architectural Principle:**  
> *"Thor embeds CEF (Chromium Embedded Framework) in its own native browser shell. Ship CEF binaries; do not fetch/build the full Chromium source tree. Rust owns the shell, orchestration, evidence, FeatureEngine, and high-scale load generation. CDP instruments Thor's embedded browser."*

Hard constraints:

1. **User-facing name is always `Thor`** (not `thor`). Internals: crate `thor`, binary `Thor`, Labs binary `thor-labs`, and CEF helper subprocesses.
2. **Product (CEF):** One Thor native shell embeds CEF browsers for tabs, preserving CEF's renderer sandbox and subprocesses. **Do not** use system Google Chrome or a separately visible Chromium window as the product. Ship prebuilt CEF binaries.
3. **Browser tooling:** `src/main.rs` enters CEF. Standalone Labs/CDP executable discovery lives in `src/engine/browser_discovery.rs`; system Chrome discovery requires `THOR_ALLOW_SYSTEM_CHROME=1`. Do not add alternate product launch paths or compatibility scaffolding for the removed shell.
4. **Three-Level Isolation Hierarchy** (map onto CEF request contexts and browsers):
   - **Level 1 (Profile)**: Settings, bookmarks, extensions, root CAs, IDE bridge. No global history DB.
   - **Level 2 (Identity Context)**: Isolated `cookies`, `localStorage`, `IndexedDB`, `ServiceWorkers`, auth tokens (`Admin`, `Standard User`, `Anonymous`, etc.). Logins and site storage persist by default. Explicit disposable identities purge storage after their final tab closes.
   - **Level 3 (Tab)**: Tab-scoped history list, scroll pos, trace buffer, and lifecycle. Wiped clean on tab close / 24h GC.
5. **Four-Stage Ephemeral Tab Lifecycle**:
   - `Active` (0–15m background inactivity) $\rightarrow$ `Frozen` (15m–1h) $\rightarrow$ `Discarded` (1h–24h RAM release) $\rightarrow$ `Expired` ($\ge$24h auto-close & tab history wipe). Bind lifecycle to real CEF operations; never report an unperformed operation as successful.
   - Collections, environments, scenarios, findings, and tool results persist independently of browsing. Passive browsing captures persist only after explicit promotion. Closing a tab deletes unsaved history/captures; crash recovery never restores browsing history.
6. **Cmd+K** = Sites menu (bookmarks + active-tab history + open tabs). **Cmd+⇧K** = Toolbox menu → **Developer Control Center** (144 engineer features across 6 domain lenses) plus product specialist tools.
7. **No Fake/Stub UI**: Prefer wiring real Rust engines via **`AppState::global()`** — never mock RNG or pretend crypto. Prefer deepen/capture wiring or honest `Analyzer (offline)` naming over Lab/Workbench theater. Do not claim CEF/Electron as “built like Chrome.”
8. **Privileged lab IPC** (Locust public, intercept, CA install, SSRF/fixation mutate, live stub/SSE labs) enforced in Rust (`src/net/consent.rs`, `src/workbench/commands.rs`, feature handlers). CEF workspace IPC is validated in `src/browser/cef/host/workspace.rs`.
9. **Universal Evidence Engine, product lenses & Engineer Feature Suite**:
   - Top-Level Areas: `Browse`, `Inspect`, `Compare`, `Run`.
   - Product lenses: `Full-Stack`, `Security` (IDOR matrix), `Performance` (Web Vitals), `AI / LLM` (`src/dx/llm_studio/`), `Web3 / Chain` (`src/dx/web3_inspector/`).
   - **Engineer Feature Suite (`src/features/`)**: **144** `FeatureId`s in six domains — Frontend, Backend, Security, Performance, QA, AI — registered in `FeatureEngine` / `FeatureOrchestrator`, invoked from DevCC via `feature_list` / `feature_invoke` / `lens_payload`. Access: `AppState::global().features()`.
   - Operation: `Promote to Test` (`src/dx/test_generator/`) exports Playwright TS, k6 JS, cURL, ThorScenario, EvalHarness, Finding, ReqwestRust, GoHttp, PythonHttpx, CI Bundle (+ oracles / hermetic / quarantine compose).
10. **Thor Load Engine (`src/load/`)**: Canonical `ThorScenario` spec with dual runners (`BrowserRunner` for Web Vitals/DOM, `ProtocolRunner` for multi-threaded Rust HTTP/WS traffic). Soft-nav / LCP media / measured transfer bytes feed `PerformanceBudgets`.

## Layout

```
browser/               # extension + native messaging assets
scripts/               # packaging, release, and executable repository checks
crates/                # workspace compatibility crates
examples/              # manual diagnostics, separate from shipped binaries
src/
  main.rs              # bin Thor entry (mimalloc + browser::cef::run)
  bin/thor_labs.rs     # Labs sidecar (self-CDP → FeatureEngine / MCP)
  bin/package_thor.rs  # CEF app bundle packager
  lib.rs               # library crate thor
  paths.rs             # thor_data_dir / ChromiumProfile / cache paths
  app_state.rs         # process-wide singletons
  browser/
    session.rs         # tab and identity state
    cef/               # embedded runtime, native host, browser commands, trusted IPC
  api/                 # request models, auth, variables, and HTTP execution
  workbench/           # embedded UI assets, commands, and persistence
  projects/            # persistent project collections and evidence
  engine/
    browser_discovery.rs # executable discovery for standalone Labs/CDP tooling
    self_cdp.rs        # connect Labs to Thor’s own DevTools
    chromium_runtime/  # CDP launch / connect / target manager
    tab_session/       # CDP page target & 4-stage lifecycle
    engine_coordinator/# commands, snapshots, and dispatch
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

## Repository structure

- Modules with child files own a directory; `mod.rs` is their entry point. Leaf modules use `snake_case.rs`. Keep implementations and their behavior tests beside their owner.
- Preserve current feature behavior, feature slugs, IPC payloads, and exported formats during source moves. Retired launchers and unused compatibility APIs should be removed rather than maintained.
- Add feature implementations under the existing domain directory in `src/features/`; register handlers in `catalog.rs`. Shared orchestration belongs in `orchestrator/`. Keep the catalog at 144 entries.
- CEF owns native browser UI and browser operations; workbench owns embedded developer surfaces; engines own reusable behavior. Put a new operation with its owner instead of extending unrelated entry points.
- Run `python3 scripts/check_structure.py` after source moves. It enforces filenames and directory ownership; the Rust compiler checks module resolution. Run the checker tests with `python3 -m unittest discover -s tests -p test_repository_structure.py`.
- Current runtime references are `src/main.rs`, this layout, and `docs/ARCHITECTURE.md`. Product requirements remain in `PRODUCT.md` and `docs/requirements_*.md`.

## Runtime model

### CEF runtime

1. Load bundled CEF and dispatch helper subprocesses before initializing the main application.
2. Embed CEF tab views in Thor's native shell; use isolated request contexts for identities.
3. Supervise Labs/MCP; a tool failure must not terminate browsing.

```bash
# Reserve the final product build until implementation and qlty checks finish.
```

## IPC contract (workspace → Rust)

Active workspace routing lives in `src/browser/cef/host/workspace.rs` and `src/workbench/commands.rs`. Preserve their command shapes and trust checks. Native browser operations are dispatched in `src/browser/cef/host/commands.rs` and `src/browser/cef/host/actions.rs`.

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
make check-structure
```

Live Chromium smoke test is `#[ignore]` (`live_launch_smoke`) — needs Thor-owned browser. DevCC `from_active_tab` requires a live evaluator; without Chromium it must `Err` (never empty-success theater).

## Edit discipline

- Match existing style; minimal diffs.
- Don’t add markdown/docs unless asked (updating this file is allowed).
- Don’t commit unless the user asks.
- After CEF host or workbench UI changes: verify omnibox + action buttons still clickable.
- Prefer real CDP/HTTP behavior over alerts and simulated metrics.
- Prefer deepen existing `FeatureId`s + capture loops over minting new IDs; keep `FeatureId::ALL.len() == 144` unless intentionally expanding the registry.
- CEF shell/UI changes: single owner per surface; preserve native omnibox/action hit-testing.
