# Thor Implementation Plan + Prioritized Backlog

**Target declaration:** M1 = Lean DX on WebKit (honest). M2 = Chromium content engine of record. M3 = polish/COULDs.  
**Baseline reality:** Live shell = `tao` + AppKit `NativeToolbar` (52px) + **in-window WKWebView content tabs** + content-injected ⌘K/⌘⇧K. `ChromiumRuntime`/`TabSession` exist but are **not** driven by shell. `AppState::global()` has profiles/tamper/shield/history/locust; **no** workbench, consent, audit, collections, or `__thorEngineStatus`. `gui.rs` still emits Locust/tamper IPC, but `shell/mod.rs` **does not dispatch** those cmds to Rust.

---

## 1. Conflicts / reconciliations (FS + UI + SEC)

| Conflict | Sources | Resolution (binding) |
|----------|---------|----------------------|
| **Content engine: WebKit now vs Chromium of record** | AGENTS.md + FS-ARCH-01 vs `shell/mod.rs` WebKit tabs | **M1 ships WebKit content.** M2 switches shell to drive Chromium. Never claim CDP/Fetch on WebKit. Ship `__thorEngineStatus` immediately. |
| **Dual-window Chromium (AGENTS) vs in-window WebKit (shell)** | AGENTS dual-window vs UI-004 / shell comments | Until CEF: **AppKit chrome + one content surface.** Dual Chromium window OK for M2 if documented; never dual always-on WKWebViews for chrome+content. |
| **Page intercept needs Chromium** | FS-043, SEC-020+ vs WebKit | M1: tamper UI + rules + pending API + **explicit unsupported for page traffic**. M2: CDP Fetch + scope consent. Workbench HTTP is out-of-band (no page MitM). |
| **Freeze SLAs** | FS-004 vs WebKit hide/bounds | M1: soft-fail documented; hide inactive tabs only. M2: CDP lifecycle freeze. |
| **Device/viewport** | UI-120+, FS-050 vs WebKit resize | Device **panel UI** can land in late M1 as controls-only; **emulation apply** = M2 Chromium only. |
| **Consent UX vs lean chrome** | SEC-002–004 vs UI “no action soup” | Consent = **modal/palette confirm or `Thor://` lab gate**, not permanent buttons. Enforce in **Rust** (SEC-095). |
| **Profile vs SecurityShieldToggles defaults** | SEC-010 DevHost strip=false vs `SecurityShieldToggles::new()` strip/inject **true** | On init + `toggle_profile`, sync shield toggles from `ProfileManager` (single source of truth). |
| **⌘K injected into content** | UI-040 / SEC-091 vs content-injected modal + content IPC | Prefer palette WKWebView or native overlay for tools; content pages must **not** reach privileged cmds. Hotkeys may stay in content with allowlisted cmds only. |
| **Locust/Tamper UI in `gui.rs` vs native chrome** | UI-005 / UI-100 vs legacy bars | Do **not** restore 8-button HTML chrome. Port tools to secondary strip / dock / `Thor://` using `gui.rs` as reference HTML only. |
| **Tamper `url.contains`** | SEC-022 vs current `tamper.rs` | Keep engine; document unsafe; migrate matchers before calling page intercept “done” (M2 gate). |
| **Secrets at rest** | SEC-040 vs no encryption yet | M1: mode-restricted files under `thor_data_dir()` + honest gap doc; keychain = SHOULD later. |
| **IDE from content IPC** | SEC-015/091 vs content handler `open_ide` | Gate on DevHost in Rust; reject from non-chrome origin; Stealth no-ops. |
| **Brand / paths** | FS-070, UI-007, AGENTS | Always **Thor** / `Thor://…`; never `.thor`. Update AGENTS when M1 engine truth is locked. |

---

## 2. Milestone plan (MUST IDs mapped)

### M1 — Lean DX (WebKit OK)

**Engine:** WebKit content + AppKit chrome. Chromium in-tree unused for content.

| Area | MUST IDs |
|------|----------|
| Architecture / honesty | FS-001, FS-003, FS-070–072; UI-001–005, UI-007, UI-180–182; SEC-080, SEC-090–095 (baseline) |
| Browse | FS-010–014; UI-020–023; SEC-070–071 (via profile sync) |
| Workbench | FS-020–023, FS-060–061; UI-080–084 (panel/`Thor://api`); SEC-040–042 (mask + restrict files) |
| Locust | FS-030–032; UI-100–104; SEC-003–004, SEC-060–065 |
| Security (no page MitM) | FS-040–042, FS-044–045; UI-140–142; SEC-010–015, SEC-020–021, SEC-050–054, SEC-030–035 (gen + perms; install friction) |
| Privacy | FS-PRIV-01–04; SEC-041 |

**M1 exit:** Journeys J1–J4 work with **honest** engine status; Locust/workbench real HTTP; intercept enable blocked or labeled unsupported for page traffic; consent gates on Locust (+ tamper enable); `cargo test --lib` + `integration_tests` green for AppState/history/Locust/profile/shield.

### M2 — Developer browser (Chromium required)

| Area | MUST / key SHOULD |
|------|-------------------|
| Chromium shell | FS-002, FS-004–006, FS-043, FS-ARCH-01/04; UI-004 preserved |
| Tamper + Fetch | FS-042–043; SEC-002, SEC-022–026, SEC-007 |
| DX on page | FS-046; FS-050–051 (SHOULD); UI-120–123 with real apply; UI-085 |
| Integration | FS-062–063 |
| Isolation | SEC-013 (partitions or explicit warn) |

**M2 exit:** One Chromium process, many targets; freeze/unfreeze; page intercept scoped; viewport presets apply; soft-fail paths gone for claimed CDP features.

### M3 — Polish

NG-08 browser polish; CEF exploration (NG-01); FS-025/034/047/052; UI COULDs (comfort density, pop-out); SEC SHOULD (keychain, metadata IP block, browser-only CA trust).

---

## 3. File-level work packages

| ID | Package | Owner modules | Size | Milestone |
|----|---------|---------------|------|-----------|
| WP-A | Engine status + IPC allowlist hub | `shell/mod.rs`, new `shell/ipc_dispatch.rs`, `gui.rs` hooks | M | M1 |
| WP-B | Profile ↔ shield sync + persist | `app_state.rs`, `engine/profiles.rs`, `paths.rs` | S | M1 |
| WP-C | ConsentScope + audit log (Rust) | new `net/consent.rs`, `net/audit.rs`, `app_state.rs` | M | M1 |
| WP-D | Locust caps + consent + UI pill/strip | `net/locust.rs`, `app_state.rs`, `native_toolbar.rs`, tool surface HTML | M | M1 |
| WP-E | API workbench model + IPC + store | new `dx/workbench.rs` (or `net/http_client.rs` + `dx/collections.rs`), `paths.rs` | L | M1 |
| WP-F | Workbench / Security / Load UI surfaces | `shell/gui.rs` (routes), dock/strip, `Thor://` content | L | M1 |
| WP-G | Calm chrome + palette purity | `native_toolbar.rs`, content hotkey script, `dx/cmd_palette.rs` | M | M1 |
| WP-H | Tamper rules validation + UI progressive | `net/tamper.rs`, Security panel | M | M1 |
| WP-I | Decoder honesty + SSL CA UX/perms | `dx/decoder.rs`, `net/ssl_ca.rs`, settings UI | M | M1 |
| WP-J | Tests: consent, Locust caps, workbench, IPC schema | `tests/integration_tests.rs`, unit tests | M | M1 |
| WP-K | Chromium content cutover | `shell/mod.rs`, `engine/tab_session.rs`, `chromium_runtime.rs`, `freezing.rs` | L | M2 |
| WP-L | CDP Fetch ↔ tamper + matcher migrate | `devtools/cdp_integration.rs`, `net/tamper.rs` | L | M2 |
| WP-M | Device presets + session capture | `engine/`, `dx/`, shell Device panel | M | M2 |
| WP-N | `ContentBackend` trait (optional) | `engine/` | M | M2/FS-007 |

---

## 4. Will NOT do in M1

- Chromium launch as live content / CEF single-window (NG-01, FS-002 content path)
- Page-traffic Fetch intercept / fulfill mock (FS-043 done; FS-047)
- Real CDP freeze SLAs / prewarm pool in shell (FS-004–006 as shipped behavior)
- Device metrics emulation that claims BrowserStack parity (UI apply = M2)
- Session HAR/screenshot capture (FS-051)
- Synced multi-pane / 3D DOM / canvas (NG-05, FS-052)
- MV3 / WASM load (NG-02, SEC-080)
- JWT crack crypto (NG-07, SEC-051–052)
- Google sync, global history DB, session restore default (NG-03, FS-015 COULD)
- Permanent action-button soup / dual always-on WKWebViews for chrome+content
- Postman import, GraphQL, distributed Locust, OS SIGSTOP default
- Encryption-at-rest for secrets (document gap; mode-restrict only)
- Find-in-page, zoom, downloads UI (NG-08)

---

## 5. First 10 PR-sized tasks (order)

1. **`__thorEngineStatus` + central IPC dispatch**  
   Add allowlisted dispatcher used by palette/tool surfaces; unknown cmd → log + no-op (FS-IPC-01, SEC-090). Emit `{ backend: "webkit", cdp: false, intercept: false }`. Soft-fail intercept enable with UI copy (FS-003, FS-043).  
   *Touches: `shell/mod.rs`, small chrome eval helper.*

2. **Sync `ProfileManager` ↔ `SecurityShieldToggles` + persist mode**  
   DevHost/Stealth defaults match SEC-010/011; navigate uses synced toggles; persist profile under `thor_data_dir()` (FS-PROF-01/02, FS-040–041). Gate `open_ide` on DevHost (SEC-015).  
   *Touches: `app_state.rs`, `profiles.rs`, shell navigate/IDE paths.*

3. **`ConsentScope` + Locust start/stop Rust gates**  
   Confirm dialog params; private-default; public ack; caps (users/ramp/runtime); audit start; wire `locust_start`/`stop` through dispatcher to `AppState` (SEC-003/004/060–065, FS-030–032).  
   *Touches: new `net/consent.rs`, `locust.rs`, `app_state.rs`, shell IPC.*

4. **Locust progressive UI (no permanent chrome)**  
   `⌘⇧L` / `Thor://load` + run pill when RUNNING; ≤4 Hz metrics; Stop always reachable (UI-100–104, UI-025). Reuse `push_locust_stats`; do not revive action-button soup.  
   *Touches: `native_toolbar.rs` (pill chip), tool HTML/route, shell timers.*

5. **API workbench core IPC + in-memory send**  
   `api_send` → real HTTP via Thor client; `__thorApiResult`; env `{{var}}` + secret mask in UI payloads (FS-020–021, SEC-041–042).  
   *Touches: new workbench module, `app_state` handle, IPC.*

6. **Collections/environments persistence**  
   Versioned JSON under `thor_data_dir()`; mode 0600; strip-secrets export default (FS-022, SEC-040/043). IPC: save/list/upsert/env_*.  
   *Touches: `paths.rs`, store module, tests.*

7. **Workbench UI + Locust-from-request + load-this-origin**  
   Dock or `Thor://api` three-pane; `locust_from_request`; Cmd+⇧K “Load test this origin” (FS-023, FS-060–061, UI-080–084).  
   *Touches: UI surfaces, IPC, palette settings payload.*

8. **Calm chrome + palette purity + hit-test lock**  
   Verify UI-001–005; ⌘K datasets only bookmarks/active history/open tabs; ⌘⇧K tools/settings only; AppKit local monitor for shortcuts when content focused (UI-020–023, FS-011–013). Remove privileged cmds from content IPC path (SEC-091).  
   *Touches: content hotkey script, `cmd_palette.rs`, `native_toolbar.rs`.*

9. **Tamper/Security progressive layers (WebKit-honest)**  
   Shield toggles in ⌘⇧K/`Thor://security`; tamper enable requires consent; rule validation; pending list IPC stub; page intercept shows requires Chromium (FS-042, UI-140–141, SEC-020–021). Decoder surface + crack unavailable (FS-044, SEC-050–052, UI-142).  
   *Touches: `tamper.rs` validation, decoder UI, SSL status read.*

10. **SSL CA key perms + install typed-confirm IPC + test pack**  
    Owner-only key files; block install without typed `TRUST THOR DEV CA`; integration tests for Locust consent/caps, env subst, history isolation, profile persist, engine status, crack unavailable (FS-045, SEC-030–034, FS-072, SEC-T*).  
    *Touches: `ssl_ca.rs`, settings UI, `integration_tests.rs`.*

---

## 6. Risks & sequencing

### Dual-WKWebView hit-testing
- **Risk:** Palette overlay + content both visible → dead toolbar/clicks (already documented; shell hides content for overlay path, but ⌘K currently injects into content).
- **Sequence:** Keep **single interactive content plane** + AppKit strip. Prefer one hidden palette WebView shown only while modal, or content-injected modal with content IPC **allowlist stripped to hotkeys/navigate/switch_tab**. Never reintroduce always-on chrome WKWebView sibling for the 52px strip (UI-004).
- **Gate:** Manual hit-test suite after any WebView bounds/visibility change.

### AppState singletons
- **Risk:** Per-IPC `TrafficTamperEngine::new()` / dual Locust handles / profile UI desync from Rust.
- **Sequence:** All new engines (ConsentStore, CollectionStore, HttpClient, AuditLog) hang off `AppState::global()` like locust/tamper. Locust slot already replace-on-start — keep that invariant when adding caps/consent.
- **Gate:** Unit test pointer equality + “double start leaves one swarm.”

### Consent in Rust (not chrome JS)
- **Risk:** UI confirms but `set_interception_enabled` / `locust_start` still succeed via raw IPC.
- **Sequence:** PR3 before shipping Locust/Tamper UI (PR4/9). Store `ConsentScope` in AppState; handlers check scope + caps before side effects (SEC-095). Chrome only presents UX.
- **Gate:** SEC-T01/T04/T05 automated.

### Chromium cutover (M2)
- **Risk:** Mid-M1 partial Chromium wiring creates fake CDP status.
- **Sequence:** Finish M1 honesty first. M2 is a dedicated cutover PR train (WP-K then WP-L), flipping `__thorEngineStatus` only when navigate/freeze/Fetch path is real.
- **Gate:** Ignored live smoke + intercept matrix tests.

### Content-as-IPC-attacker
- **Risk:** Malicious page posts `locust_start` / `install_ca` via wry IPC on content WebView (SEC-091).
- **Sequence:** With PR1/PR8, content handler allowlist = navigation/hotkeys only; privileged cmds only on trusted chrome/palette/native surfaces.

### Profile / shield default bug
- **Risk:** Current toggles default strip+inject **on** while `ProfileManager` defaults **DevHost** (strip off) → label vs behavior mismatch (SEC-012).
- **Sequence:** Fix in PR2 before Stealth UX demos.

### Legacy `gui.rs` gravity
- **Risk:** Porting tools by re-enabling full HTML chrome WebView.
- **Sequence:** Extract panels/routes from `gui.rs`; primary strip stays native. Secondary strip = one conditional sibling surface max (UI-006).

### Suggested critical path
```
PR1 (status/IPC) → PR2 (profile sync) → PR3 (consent/Locust Rust)
    → PR4 (Locust UI) ∥ PR5–7 (workbench)
    → PR8 (palette/IPC harden) → PR9 (security UI) → PR10 (CA + tests)
    ── M1 freeze ──
    → WP-K Chromium shell → WP-L Fetch+matchers → Device/capture
```

---

## Definition of done (agents)

- Declare **M1 / M2 / M3** before claiming requirements met.
- M1 green = mapped MUST table above + no banned stub UI + engines only via `AppState::global()` + engine status never lies.
- Do not implement Chromium content or page MitM until M1 honesty and consent gates land.