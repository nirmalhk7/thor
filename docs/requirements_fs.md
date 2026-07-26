# Thor — Fullstack Product & Engineering Requirements

**Version:** 0.1 (agent contract)  
**Scope:** Requirements only — no implementation.  
**Baseline reality:** macOS shell = `tao` + `wry` (WKWebView) + native AppKit toolbar; content tabs are in-window WebKit; Chromium/CDP stack exists in-tree (`ChromiumRuntime` / `TabSession`) but is **not** the live content engine. DX/net libraries partially wired via chrome IPC + `AppState::global()`.

---

## Executive summary

- Thor is a **lean developer browser**: browse + API workbench + local load test + security inspect — not Chrome++ and not a cloud device farm.
- **Architecture law:** Shell (chrome/UI) ≠ Content engine ≠ Net/DX modules. All engines hang off process-wide `AppState`; no per-IPC engine construction.
- **v1 content decision:** Goals that need real request intercept, accurate freeze, multi-viewport CDP, and Chrome-parity pages **require Chromium/CDP**. WebKit-only shell can ship browse + out-of-band HTTP tools, but **blocks** tamper-on-page-traffic and true BrowserStack-like emulation.
- **Privacy/perf defaults:** tab-scoped history only, 24h idle tab GC, freeze inactive tabs, no Google sync / no MV3 host by default; brand always **Thor**.
- Agents deliver against **MUST** IDs with testable acceptance; SHOULD/COULD are ordered deferrals that keep the product lean.

---

## 1. Architecture boundaries

| Layer | Owns | Must not own |
|-------|------|--------------|
| **Shell** (`shell/`) | Window, native toolbar, omnibox/tabs chrome, Cmd+K / Cmd+⇧K surfaces, IPC dispatch, layout of tool bars | HTTP semantics, CDP protocol details, Locust scheduling, cert crypto |
| **Content engine** (`engine/`) | Tab lifecycle, navigate/back/forward/reload, freeze/unfreeze, prewarm, profile-backed browsing context, CDP targets | Persistence format for collections; Locust swarm logic; chrome HTML |
| **Net** (`net/`) | Security shield, traffic tamper model, Locust swarm, local SSL CA | UI chrome; tab visual bounds |
| **DX** (`dx/`) | Cmd palette payloads, bookmarks, decoder, stack detect, IDE bridge stubs | Browser process lifecycle |
| **AppState** | Process-wide singletons (profiles, tamper, shield toggles, tab history, locust handle) | Per-tab WebView handles (shell-local) |
| **Persistence** (`paths` + stores) | Data under `thor_data_dir()` (`…/Thor`), Chromium profile dir | Secrets in git; global visited-history DB |

**Engine strategy (opinionated):**

- **FS-ARCH-01 (MUST):** Treat Chromium (one browser process, many targets) as the **content engine of record** for v1 “developer browser” acceptance of intercept/freeze/emulation.
- **FS-ARCH-02 (MUST):** Keep wry chrome as control surface until CEF/single-window embed; dual surface is allowed if documented and not faked as one compositor.
- **FS-ARCH-03 (MUST):** WebKit content path may remain as **fallback / smoke** only; features that claim CDP behavior must soft-fail honestly when engine ≠ Chromium.
- **FS-ARCH-04 (MUST):** One Chromium browser process max; never Chrome-per-tab.
- **FS-ARCH-05 (SHOULD):** Abstract a `ContentBackend` boundary (navigate, history, eval, freeze, fetch-intercept hooks) so shell IPC does not hard-code WebKit APIs for DX features.

---

## 2. Core user journeys

### J1 — Browse (Chrome-like, lean)

1. Launch Thor → chrome ready; content engine starts (or connects) with prewarm pool.
2. Omnibox navigate (URL or search-shaped input policy as implemented); tracker strip applied per profile/toggles.
3. Tabs: new / switch / close; back / forward / reload; title/URL sync to chrome.
4. Inactive tabs freeze; idle ≥ 24h GC closes tab and clears that tab’s history.
5. Cmd+K: bookmarks + **active tab** history + open tabs. Cmd+⇧K: settings/DX/security only.

### J2 — API workbench (Postman-like)

1. Open workbench from Cmd+⇧K or dedicated chrome panel.
2. Build request (method, URL, headers, body, auth); resolve env vars.
3. Send via **Thor HTTP client** (not necessarily page network); view status, headers, body, timing.
4. Save to collection; switch environments; optionally “Open as Locust target” or “Replay in tamper history”.

### J3 — Load test (Locust-like)

1. From workbench request or explicit URL → configure users, ramp-up, duration.
2. Start swarm (real HTTP); live metrics stream to chrome (`__thorUpdateLocustStats`).
3. Stop → scored metrics (RPS, error rate, p50/p90/p95/p99, score). No RNG theater.

### J4 — Security inspect

1. Toggle DevHost ↔ Stealth; shield strip/inject headers behave accordingly.
2. Enable interception → paused requests appear; edit / forward / drop (requires Chromium Fetch path for **page** traffic).
3. Decoder: Base64 / JWT decode; JWT crack stays honest “unavailable” until crypto is linked.
4. Optional: tech-stack rescan of active page HTML (CDP or content eval).
5. Local SSL CA: generate/trust path for HTTPS to local services under Thor data dir.

### J5 — Multi-viewport / session capture (BrowserStack-like, local)

1. Pick preset viewports (mobile/tablet/desktop); sync scroll/click **only if** backend supports multi-target or multi-emulation.
2. Session capture: export HAR-like + screenshot set for active tab(s) — local files only, no cloud farm.

---

## 3. Data models

### Tab

| Field | Notes |
|-------|--------|
| `id` | Stable string (chrome) mapped to engine `TabId` when Chromium |
| `url`, `title` | Synced from engine |
| `last_active` | For GC |
| `lifecycle` | `Active` \| `Frozen` |
| `history[]` | **Tab-scoped only**; cleared on close/GC |
| `profile_mode` | Inherits process profile unless later per-tab override (COULD) |

### TabHistoryEntry

`url`, `title`, `timestamp` — in-memory; no global DB.

### ApiRequest (workbench)

`id`, `name`, `method`, `url`, `headers[]`, `body` (raw/json/form), `auth` (none \| bearer \| basic \| header), `timeout_ms`, `collection_id?`

### Collection

`id`, `name`, `requests[]`, `created_at`, `updated_at` — persisted under Thor data dir.

### Environment

`id`, `name`, `variables: { key → value }`, `secret_keys[]` (values redacted in UI logs). Active env resolves `{{var}}` in URL/headers/body.

### LocustRun

`id`, `target_url`, `users`, `ramp_up_secs`, `runtime_secs`, `started_at`, `metrics` (`LocustScoredMetrics`), `source` (`workbench_request_id` \| `url`)

### InterceptedRequest / Response / TamperRule

As in existing tamper engine; history capped (MUST define max entries, e.g. 500).

### SessionCapture (SHOULD)

`id`, `tab_id`, `created_at`, `har_path`, `screenshot_paths[]`, `viewport_presets[]`

### Bookmark

Existing store; appears in Cmd+K only (not a second history DB).

---

## 4. IPC / event contracts

Preserve and extend chrome → Rust JSON `{ "cmd": "...", ... }` shapes. All handlers use `AppState::global()`.

### Chrome → Rust (MUST retain)

| cmd | Payload (min) | Effect |
|-----|---------------|--------|
| `new_tab` | — | Create tab + engine target |
| `switch_tab` | `id` | Unfreeze target; freeze previous |
| `close_tab` | `id` | Clear tab history; destroy target |
| `navigate` | `url` | Shield sanitize → engine navigate → record history |
| `back` / `forward` / `reload` | — | Engine |
| `toggle_devtools` | — | Engine or soft-fail |
| `open_ide` | `ide`, `path` | DX bridge when DevHost |
| `toggle_profile` | — | DevHost ↔ Stealth |
| `set_interception_enabled` | `enabled` | Tamper + enable Fetch if Chromium |
| `add_tamper_rule` | rule fields | Persist in-memory rules |
| `locust_start` | `url`, `users`, `ramp_up_secs`, `runtime_secs` | Real swarm |
| `locust_stop` | — | Stop + metrics |
| `decode_payload` / `decode_jwt_crack` | payload | Decoder; crack may return unavailable |
| `rescan_tech_stack` | — | HTML heuristics → UI |
| `open_cmd_k` / `open_cmd_shift_k` | — | Palette |
| `drag_window` | — | Shell only; never on mousedown of controls |

### Chrome → Rust (MUST add for workbench)

| cmd | Purpose |
|-----|---------|
| `api_send` | Execute ApiRequest with active env |
| `api_save_request` / `api_list_collection` / `api_upsert_collection` | Persistence |
| `env_set_active` / `env_upsert` | Environments |
| `locust_from_request` | `request_id` + swarm params |
| `tamper_forward` / `tamper_drop` / `tamper_list_pending` | Intercept UX |
| `session_capture` | SHOULD — export local capture |

### Rust → chrome (evaluate hooks)

| Hook | When |
|------|------|
| `updateTabUrl` / `updateTabTitle` | Navigation |
| `__thorUpdateLocustStats` | Live + final metrics |
| `__thorUpdateDecodeResult` | Decoder |
| `__thorUpdateTechStack` | Rescan |
| `__thorRemoveTab` | Idle GC |
| `__thorCmdKPayload` / `__thorSettingsPayload` | Before modal |
| `__thorApiResult` | Workbench response (MUST) |
| `__thorTamperPending` | Intercept queue (MUST when intercept on) |
| `__thorEngineStatus` | `{ backend: "chromium"\|"webkit", cdp: bool, intercept: bool }` (MUST) |

**Contract rules:**

- **FS-IPC-01 (MUST):** Unknown `cmd` → no-op + log; never panic.
- **FS-IPC-02 (MUST):** Locust stats must reflect real HTTP measurements only.
- **FS-IPC-03 (MUST):** If intercept enabled on WebKit backend → UI shows explicit “requires Chromium engine”.

---

## 5. Persistence & profiles

### Paths

- Data: `paths::thor_data_dir()` → `…/Thor` (override `THOR_DATA_DIR`).
- Chromium profile: `thor_chromium_profile_dir()`.
- Collections/envs: e.g. `Thor/collections.json`, `Thor/environments.json` (format versioned).
- SSL CA: under Thor data dir (existing `ssl_ca` behavior).
- No `~/.thor`.

### Profile modes

| | **DevHost** | **StealthTarget** |
|--|-------------|-------------------|
| Tracking strip | off by default | on |
| GPC/DNT inject | off | on |
| Fingerprint noise | off | on (best-effort; honesty if WebKit-limited) |
| WebGL | allowed | allowed with noise intent |
| IDE bridge | enabled | disabled |
| Intercept / Locust | allowed | allowed (user-driven) |

- **FS-PROF-01 (MUST):** Toggle updates `AppState` immediately; navigate path honors shield toggles.
- **FS-PROF-02 (MUST):** Profile choice persisted across launches.
- **FS-PROF-03 (SHOULD):** Stealth does not claim undetectable anti-bot; copy = “hardened defaults”.

### History / privacy

- **FS-PRIV-01 (MUST):** No global visited-history DB.
- **FS-PRIV-02 (MUST):** Close tab / GC → `clear_tab_history`.
- **FS-PRIV-03 (MUST):** Cmd+⇧K exposes clear-all history wipe.
- **FS-PRIV-04 (MUST):** Env secret values not written to Locust UI logs or tech-stack payloads.

---

## 6. Non-goals / deferrals (keep lean)

| ID | Deferred | Why |
|----|----------|-----|
| NG-01 | CEF / single-window compositing | Large; dual surface OK for v1 |
| NG-02 | Real MV3 extension host | Weight + sync theater |
| NG-03 | Google account sync | Conflicts with lean/privacy |
| NG-04 | Cloud device farm / BrowserStack SaaS | Out of scope; local presets only |
| NG-05 | Obsidian 3D DOM / canvas sync theater | Explicitly banned as fake UX |
| NG-06 | WASM plugin sandbox | Defer |
| NG-07 | HMAC/JWT crack crypto | Honest unavailable until crate linked |
| NG-08 | Find-in-page, zoom, downloads UI | Browser polish; after core journeys |
| NG-09 | Full Postman import/export parity (v1) | v1 = Thor-native collections; import = COULD |
| NG-10 | Distributed Locust workers | Single-process Tokio swarm only |
| NG-11 | Per-tab OS SIGSTOP by default | Soft CDP freeze first; OS suspend optional/risky |

---

## 7. MUST / SHOULD / COULD requirements

### Architecture & engine

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-001 | MUST | Shell, content engine, net/DX separated; engines only via `AppState::global()`. |
| FS-002 | MUST | One Chromium process, many tabs/targets when Chromium backend active. |
| FS-003 | MUST | Expose `__thorEngineStatus` so UI never lies about CDP/intercept capability. |
| FS-004 | MUST | Tab freeze on background + unfreeze on focus (CDP lifecycle; soft-fail documented on WebKit). |
| FS-005 | MUST | Idle tab GC ≥ 24h closes tab, clears history, notifies chrome via `__thorRemoveTab`. |
| FS-006 | SHOULD | Prewarm pool size ≥ 2 for Chromium tab acquire. |
| FS-007 | COULD | `ContentBackend` trait + runtime switch `THOR_CONTENT_BACKEND=chromium\|webkit`. |

### Browse journey

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-010 | MUST | New/switch/close/navigate/back/forward/reload work end-to-end from native toolbar + IPC. |
| FS-011 | MUST | Omnibox + action buttons remain hit-testable (macOS: no movable-by-background; `no-drag` on controls; no drag_window on mousedown). |
| FS-012 | MUST | Cmd+K = bookmarks + active-tab history + open tabs only. |
| FS-013 | MUST | Cmd+⇧K = settings/DX/security only (no browse history dump). |
| FS-014 | MUST | URL sync: engine navigation updates chrome omnibox/title. |
| FS-015 | SHOULD | Seed omnibox on load; restore last session tabs = COULD (conflicts with lean — default off). |

### API workbench

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-020 | MUST | Send HTTP(S) request with method/URL/headers/body/auth; show status, headers, body, latency. |
| FS-021 | MUST | Environments with `{{var}}` substitution; secret redaction in UI. |
| FS-022 | MUST | Persist collections under Thor data dir; survive restart. |
| FS-023 | MUST | “Send to Locust” from a saved/current request (same URL/method/headers/body as swarm template — v1 may be URL+GET/POST body only if documented). |
| FS-024 | SHOULD | Bearer/Basic auth helpers; copy-as-curl. |
| FS-025 | COULD | Postman collection import; GraphQL body mode. |

### Locust

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-030 | MUST | Real concurrent HTTP via Tokio; no simulated RNG metrics in UI. |
| FS-031 | MUST | Live snapshot metrics while running; final scored metrics on stop. |
| FS-032 | MUST | Start replaces previous swarm cleanly; stop is idempotent. |
| FS-033 | SHOULD | Cap max users (e.g. 500) with clear error to protect host. |
| FS-034 | COULD | Per-endpoint weighted tasks; CSV export. |

### Security

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-040 | MUST | Tracker query strip on navigate when enabled; toggleable. |
| FS-041 | MUST | Security/GPC-DNT header injection when enabled + Stealth defaults. |
| FS-042 | MUST | Tamper engine rules + pending queue API wired to UI. |
| FS-043 | MUST | On Chromium: Fetch intercept integrates with tamper forward/drop; on WebKit: explicit unsupported. |
| FS-044 | MUST | JWT/Base64 decode works; crack returns structured unavailable (no fake HMAC verified). |
| FS-045 | MUST | Local SSL CA can generate cert material under Thor data dir. |
| FS-046 | SHOULD | Tech-stack detect from real page HTML (CDP), not canned lists only. |
| FS-047 | COULD | Full repeater edit-body + fulfill mock responses. |

### Multi-viewport / capture

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-050 | SHOULD | ≥3 local viewport presets with device metrics override **on Chromium**. |
| FS-051 | SHOULD | Local session capture (HAR + screenshots) to Thor data dir. |
| FS-052 | COULD | Synced multi-pane scroll/click (`dx/canvas`) — only after single-pane presets work; no fake 3D DOM. |

### Integration (Postman × Locust × browser)

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-060 | MUST | Workbench request → Locust run shares target URL and records `source=request_id`. |
| FS-061 | MUST | Browser omnibox URL → “Load test this origin” from Cmd+⇧K. |
| FS-062 | SHOULD | Tamper history item → “Save as ApiRequest”. |
| FS-063 | SHOULD | Locust run summary link back to workbench request if sourced. |
| FS-064 | COULD | Capture browser cookies/headers into workbench env (DevHost only; explicit user action). |

### Quality bar

| ID | Pri | Requirement |
|----|-----|-------------|
| FS-070 | MUST | User-facing strings say **Thor**; paths never `.thor`. |
| FS-071 | MUST | No stub UI: no fake Locust, fake HMAC, fake extension load, fake 3D DOM. |
| FS-072 | MUST | `cargo test --lib` and `cargo test --test integration_tests` cover AppState, history scope, Locust start/stop against local listener, profile toggle. |
| FS-073 | SHOULD | Ignored live Chromium smoke (`live_launch_smoke`) documented; CI may skip. |

---

## 8. Acceptance criteria (testable) by area

### Browse

- Given 3 tabs, switching freezes non-active (Chromium) or documents soft-fail (WebKit).
- Closing tab removes its history from Cmd+K; other tabs unaffected.
- After simulated idle ≥ TTL (test hook or injectable clock), GC fires `__thorRemoveTab` and history gone.
- Cmd+K never lists another tab’s history entries.

### API workbench

- Save collection → quit → relaunch → collection present.
- Env `BASE=http://127.0.0.1:PORT` + URL `{{BASE}}/x` hits local server; assertion on path.
- Secret env var does not appear in Locust stats JSON pushed to UI.

### Locust

- Against local `TcpListener`/`axum` stub: `total_requests > 0`, latencies non-zero, `error_rate` matches forced 500s.
- UI callback receives running=true snapshots then final score on stop.
- Double `locust_start` does not leave two swarms running.

### Security

- `utm_*` stripped iff strip enabled.
- Intercept on + Chromium: request paused until forward/drop; on WebKit: status hook shows unsupported and no silent drop.
- Decoder JWT header/payload JSON shown; crack endpoint returns `available: false` (or equivalent) without claiming success.

### Profiles

- Toggle DevHost→Stealth persists; new navigations apply Stealth strip/header defaults.
- IDE bridge IPC no-ops or refuses in Stealth.

### Integration

- From workbench “Load test”: Locust target equals request URL after env resolve.
- From browser “Load test this origin”: swarm host matches active tab origin.

---

## 9. WebKit vs Chromium — risk register

| Risk | Impact | Product implication |
|------|--------|---------------------|
| **Page traffic tamper needs CDP Fetch** | High | WebKit shell **cannot** meet FS-043 page intercept. Ship workbench-only HTTP inspect or block on Chromium cutover. |
| **Device metrics / multi-viewport** | High | Reliable emulation is Chromium CDP; WebKit resize ≠ device emulation. FS-050 blocked on WebKit. |
| **Freeze / memory wins** | Medium | WebKit hide/bounds ≠ CDP lifecycle freeze; “lighter than Chrome” claim weak until Chromium freeze path is live in shell. |
| **Tech-stack / page HTML** | Medium | Without CDP eval, heuristics are shallow or require inject scripting into WKWebView (fragile). |
| **Dual docs drift** | Medium | README/AGENTS describe Chromium dual-window; shell comments say WebKit in-window — agents must trust **shell code + FS-003 status hook**, not stale prose. |
| **Site compatibility** | High | Devs targeting Chrome will reject WebKit-only as daily driver. |
| **CEF embed delay** | Low–Med | Two windows/surfaces OK if performance SLAs measured on content process, not chrome WKWebView. |

**Decision gate (opinionated):**  
Milestone **M1 (lean DX)** can complete on WebKit: browse basics, workbench, Locust, decoder, shield-on-navigate, profiles.  
Milestone **M2 (developer browser)** is **blocked** until shell drives `ChromiumRuntime` + `TabSession` for content: intercept, freeze SLAs, viewport presets, honest Chrome-parity browsing.

---

## 10. Handoff — For Developer

**Deliverables**

1. Engine status + backend honesty (`FS-003`, `FS-043` messaging).
2. Wire or gate Chromium content path for M2; keep WebKit usable for M1 without fake CDP features.
3. API workbench IPC + persistence (`FS-020`–`FS-023`).
4. Locust UI ↔ `AppState` real metrics only (`FS-030`–`FS-032`).
5. Tamper UI ↔ pending/forward/drop; Chromium Fetch integration or explicit unsupported.
6. Preserve macOS toolbar hit-testing invariants (`FS-011`).
7. Update agent-facing docs only when behavior changes (shell backend truth).

**Definition of done:** All MUST IDs for committed milestone green; no banned stub UI; `AppState::global()` used everywhere for engines.

---

## 11. Handoff — For Tester

**Deliverables**

1. Journey scripts/checklists for J1–J4 (and J5 if SHOULD in scope).
2. Automated tests: history isolation, GC hook (clock inject), Locust against local server, env substitution, profile persistence, intercept capability matrix (Chromium vs WebKit).
3. Manual macOS suite: omnibox focus, Cmd+K / Cmd+⇧K separation, traffic lights/toolbar clicks, palette vs content hit-testing.
4. Negative tests: fake-metric absence; JWT crack unavailable; Stealth IDE refuse; unknown IPC cmd.
5. Compatibility smoke: top N URLs only on Chromium milestone.

**Definition of done:** Every MUST acceptance criterion in §8 mapped to a test id (auto or manual) with pass/fail evidence.

---

## 12. Handoff — For Perf

**Deliverables**

1. Replace theatrical benches (e.g. sleep-based cold start) with measurable probes: chrome ready time, first navigate commit, tab switch + freeze cost, Locust RPS overhead on host, RSS with N frozen tabs.
2. SLAs (initial targets — tune with evidence):
   - Cold chrome interactive &lt; 500 ms after process start (exclude Chromium first download).
   - Tab switch (cached) &lt; 100 ms chrome feedback; content unfreeze p95 &lt; 250 ms on Chromium.
   - 20 background tabs frozen: RSS growth bounded vs unfrozen baseline (document factor).
   - Locust 100 users: metrics pipeline CPU &lt; 5% of swarm CPU; UI update throttle ≥ 250 ms.
3. Report WebKit vs Chromium separately; do not blend.
4. Fail CI bench job only on regressions beyond agreed % once baselines exist; until then report-only.

**Definition of done:** Published baseline numbers for M1 (WebKit) and M2 (Chromium) with methodology; no simulated sleeps as “proof”.

---

## 13. Milestone cut line (for agents)

| Milestone | Engine | MUST focus |
|-----------|--------|------------|
| **M1 — Lean DX** | WebKit OK | FS-001, 010–014, 020–023, 030–032, 040–042, 044–045, 060–061, 070–072 |
| **M2 — Dev browser** | Chromium required | FS-002–006, 043, 046, 050–051, 004 freeze SLAs, 062–063 |
| **M3 — Polish** | Chromium | NG-08 items, CEF exploration, FS-025/034/047/052 COULDs as pulled |

Agents must declare which milestone they target before claiming “Thor requirements met.”