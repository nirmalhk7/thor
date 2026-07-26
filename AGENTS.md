# AGENTS.md — Thor

Guidance for coding agents working in this repo. Humans: see [README.md](README.md).

## Mission

Thor is a **lighter/faster-than-Chrome** developer browser (fullstack + security). Prefer changes that reduce weight and latency over feature theater.

**Milestone truth (M2):** Live shell is **AppKit chrome + headful Chromium content via CDP** (dual-window). Pages open in one Chrome/Chromium window; Thor owns omnibar / ⌘K / tools. CEF single-window embed is later. Never claim Fetch intercept UI until wired — expose `__thorEngineStatus` honestly.

Hard constraints:

1. **User-facing name is always `Thor`** (not `thor`). Crate/module/binary internals: crate `thor`, binary `Thor`.
2. **M2 (live):** One Chromium browser process for content; many CDP targets/tabs. Do not spawn Chrome-per-tab. Dual-window until CEF. Legacy wry WebKit content path is unused by `shell::run()`.
3. **Tab-scoped history only** — no global visited-history DB. Clear history on tab close / 24h GC.
4. **Cmd+K** = bookmarks + active-tab history + open tabs. **Cmd+⇧K** = settings/DX/security only.
5. Do **not** reintroduce stub UI (fake Locust RNG, fake “HMAC verified”, 3D DOM / canvas sync theater, pretend extension loading).
6. Prefer wiring existing Rust engines via **`AppState::global()`** — never `TrafficTamperEngine::new()` / `ProfileManager::new()` per IPC message.
7. **Privileged lab IPC** (Locust public, intercept, CA install) enforced in Rust (`net/consent`, `shell/ipc_dispatch`) — content pages must not escalate.
8. **Lean Locust:** local/private targets by default; caps enforced; `⌘⇧L` / Load button toggles active-tab origin.

## Layout

```
src/
  main.rs              # bin Thor entry (mimalloc + shell::run)
  lib.rs               # library crate thor
  paths.rs             # thor_data_dir / ChromiumProfile paths
  app_state.rs         # process-wide singletons
  shell/
    mod.rs             # window loop, IPC → Chromium TabSession
    gui.rs             # chrome HTML/CSS/JS (toolbar + modals)
  engine/
    chromium_runtime.rs
    tab_session.rs
    freezing.rs        # OS SIGSTOP + CDP lifecycle (soft-fail)
    profiles.rs        # DevHost / StealthTarget
  dx/                  # Cmd+K, bookmarks, decoder, stack detect, IDE bridge
  net/                 # shield, tamper, locust, ssl_ca
  daemon/prewarm.rs
  devtools/cdp_integration.rs
tests/integration_tests.rs
```

## Runtime model (M2 dual-window)

- **tao + AppKit** = Thor chrome only (omnibar, traffic lights, tool modes).
- **Chromium** = headful content via `ChromiumRuntime` + `TabSession` (CDP), managed by `shell/chromium_tabs.rs`.
- Dual-window is intentional until CEF/in-process embed. Document gaps; don’t fake a single compositor.

Shell should call:

```rust
ChromiumRuntime::launch_blocking(ChromiumRuntimeConfig { headful: true, prewarm_pool_size: 2, ..Default::default() })
rt.acquire_tab_blocking()?  // prefers prewarm
tab.navigate_blocking(url)?
// on switch: freeze previous (CDP, also_os_suspend: false), unfreeze active
// on close / 24h GC: clear tab history, close TabSession
```

## IPC contract (chrome → Rust)

Preserve existing cmd shapes in `shell/gui.rs` → `shell/mod.rs`:

`new_tab`, `switch_tab`, `close_tab`, `navigate`, `back`, `forward`, `reload`, `toggle_devtools`, `open_ide`, `toggle_profile`, `set_interception_enabled`, `add_tamper_rule`, `locust_start` / `locust_stop`, `decode_payload` / `decode_jwt_crack`, `rescan_tech_stack`, `open_cmd_k` / `open_cmd_shift_k`, `drag_window`.

Rust → JS hooks (evaluate in chrome webview):

- `updateTabUrl` / `updateTabTitle`
- `__thorUpdateLocustStats`, `__thorUpdateDecodeResult`, `__thorUpdateTechStack`
- `__thorRemoveTab` (idle GC)
- `__thorCmdKPayload` / `__thorSettingsPayload` (optional inject before modal)

## macOS chrome UI gotchas

- **Never** set `with_movable_by_window_background(true)` — it steals WKWebView clicks; the whole toolbar goes dead.
- Interactive toolbar must use `-webkit-app-region: no-drag` (drag only on `.chrome-row` empty background).
- Do **not** call `drag_window` on `mousedown` — that cancels the subsequent click/input.
- Align traffic lights with `with_traffic_light_inset` to the chrome row (currently 52px bar, inset `(14, 18)`).
- Keep Locust / interceptor / decoder bars as **siblings**, not nested inside a `display:none` parent.
- Seed omnibox on load; sync URL from Rust after Chromium navigate.
- Prefer opaque chrome webview (`with_transparent(false)`) for reliable hit-testing.

## Branding & paths

- User-facing strings: **Thor**.
- Settings palette paths: `Thor://…` (display).
- Data: `paths::thor_data_dir()` → `…/Thor` (never `~/.thor`).
- Chromium profile: `paths::thor_chromium_profile_dir()`.
- Env: `THOR_CHROMIUM_PATH`, `THOR_DATA_DIR`.

## Commands

```bash
cargo check
cargo test --lib
cargo test --test integration_tests
cargo run --bin Thor
./scripts/run_dev.sh
```

Live Chromium smoke test is `#[ignore]` (`live_launch_smoke`) — needs a real browser.

## Deferred (do not expand unless asked)

- CEF / single-window compositing
- Real MV3 extension host
- Obsidian 3D DOM / multi-viewport canvas
- WASM plugin sandbox
- HMAC JWT crack (no crypto crate linked — keep honest “unavailable” messaging)
- Find-in-page, zoom, downloads UI (planned “proper browser” polish)

## Edit discipline

- Match existing style; minimal diffs.
- Don’t add markdown/docs unless asked (except when updating these files).
- Don’t commit unless the user asks.
- After shell/UI HTML changes: verify omnibox + action buttons still clickable.
- Prefer real CDP/HTTP behavior over alerts and simulated metrics.
