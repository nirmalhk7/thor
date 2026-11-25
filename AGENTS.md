# AGENTS.md — Thor

Guidance for coding agents working in this repo. Humans: see [README.md](README.md).

## Mission

Thor is a **permanently macOS-only**, tabless developer browser. Make daily browsing reliable and developer workflows usable before expanding the feature catalog. Work across seven product disciplines: **Frontend, Backend, QA, Security, Performance, AI, and Web3**. Compare performance to Chrome only after a same-Mac, repeatable benchmark has been published.

Hard constraints:

1. **User-facing name is always Thor** (not `thor`). Internals: crate `thor`, binary `Thor`, Labs binary `thor-labs`, and CEF helper subprocesses.
2. **Stock CEF only.** Ship the pinned prebuilt CEF distribution inside the native Rust shell. Preserve renderer sandbox and subprocess boundaries. Do not fork, patch, or build Chromium, launch system Chrome as the product, or add Linux or Windows product support.
3. **Runtime boundary.** `src/main.rs` enters CEF. Standalone Labs/CDP executable discovery belongs in `src/engine/browser_discovery.rs`; system Chrome discovery there requires `THOR_ALLOW_SYSTEM_CHROME=1`. Do not add alternate product launch paths.
4. **Isolation hierarchy.** A profile owns its settings, bookmarks, extensions, and other explicit data. CEF request contexts own identity storage: cookies, site storage, and sign-in state persist for persistent identities; disposable contexts are purged after the last page closes. A page owns its own Back/Forward history and unsaved captures; no global history archive.
5. **Page lifecycle.** An unpinned page can expire after 30 minutes in the background; foregrounding resets the timer. Detectable drafts, calls, and playback defer expiry, but routine background polling does not. Implemented expiry closes real CEF views and drops unsaved page-scoped history and captures. Pinning ([#7](https://github.com/nirmalhk7/thor/issues/7)) protects against automatic expiry, orders pins ahead of other pages in Sites, and starts a fresh 30 minutes when a background page is unpinned. Recovery ([#9](https://github.com/nirmalhk7/thor/issues/9)) restores surviving open-page URLs in their original identities after quit or crash, preserving pins and expiry timing and counting elapsed time while Thor is closed. Never restore manually closed or expired pages, page history, or unsaved captures. Keep the already-shipped expiry behavior distinct from planned pinning and recovery.
6. **Native, tabless UI.** Preserve the Graphite Workspace design, native macOS window controls, a single address/navigation row, and no tab strip. Cmd+K opens Sites (open pages, active page history, and bookmarks); Cmd+Shift+K opens Toolbox and Developer Control Center. Keep the omnibox and toolbar clickable and keyboard-accessible.
7. **Real workflows, explicit saves.** Use existing engines via `AppState::global()`. Report only performed operations and real measurements. Use explicit Save or Export for persistent project work; tools must not save transient browsing captures automatically.
8. **Trusted IPC.** Keep privileged commands in Rust and enforce permission and consent at the command boundary (`src/net/consent.rs`, `src/workbench/commands.rs`, CEF message routing in `src/browser/cef/host/workspace.rs`). Native affordances do not grant a page new privileges.
9. **Catalog is not product scope.** `src/features/` contains 144 registered feature IDs across six domains (Frontend, Backend, QA, Security, Performance, and AI), accessed via `AppState::global().features()`. The Web3 decoder lives under `src/dx/web3_inspector/`. Reuse existing engines and verify the browser-to-result workflow rather than counting handlers as shipped capabilities.
10. **Thor Load Engine.** `src/load/` owns the canonical `ThorScenario`, shared by the Browser Runner (DOM and Web Vitals) and Protocol Runner (HTTP and WebSocket). Report measured, not fabricated, values.
11. **No Thor product telemetry.** Distinguish stock CEF outbound traffic and site requests from anything the Thor product adds. Do not claim that Thor never makes network requests; evidence for the telemetry boundary is tracked in [#25](https://github.com/nirmalhk7/thor/issues/25).

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
    tab_session/       # Labs/CDP page target lifecycle (not the native page lifecycle)
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
2. Embed CEF page views in Thor’s native macOS shell; use isolated request contexts for identities.
3. Supervise Labs/MCP; a tool failure must not terminate browsing.

```bash
# Reserve the final product build until implementation and qlty checks finish.
```

## IPC contract (workspace → Rust)

Active workspace routing lives in `src/browser/cef/host/workspace.rs` and `src/workbench/commands.rs`. Preserve their command shapes and trust checks. Native browser operations are dispatched in `src/browser/cef/host/commands.rs` and `src/browser/cef/host/actions.rs`.

Native messaging host template: `browser/native_messaging/com.thor.labs.json`.

## Commands

Run product build and packaged smoke checks on macOS. A Linux or Windows tooling check is not product verification.

```bash
cargo check
cargo test --lib
cargo test --lib features::
cargo test --test integration_tests
./scripts/run_dev.sh --once
cargo run --bin thor-labs
make check-structure
```

Live Chromium smoke test is `#[ignore]` (`live_launch_smoke`) — needs Thor-owned browser. DevCC `from_active_tab` requires a live evaluator; without Chromium it must `Err` (never empty-success theater).

## Chromium process cleanup

- Track the Thor Chromium browser process and its helpers when running tests, smoke checks, or diagnostics. On completion or failure, close the browser through its normal shutdown path and check for leftover processes, including reparented crashpad helpers.
- Before reporting completion, send `SIGTERM` to any confirmed leftover Thor Chromium processes. Recheck after a short wait and use `SIGKILL` only for confirmed survivors. Verify the browser and helpers exited. Confirm ownership with the executable path and process tree or recorded launch PIDs. Preserve active user browser sessions and other Chromium instances.

## Edit discipline

- Match existing style; minimal diffs.
- Don’t add markdown/docs unless asked (updating this file is allowed).
- Don’t commit unless the user asks.
- After CEF host or workbench UI changes: verify omnibox + action buttons still clickable.
- Prefer real CDP/HTTP behavior over alerts and simulated metrics.
- Prefer deepen existing `FeatureId`s + capture loops over minting new IDs; keep `FeatureId::ALL.len() == 144` unless intentionally expanding the registry.
- CEF shell/UI changes: single owner per surface; preserve native omnibox/action hit-testing.
