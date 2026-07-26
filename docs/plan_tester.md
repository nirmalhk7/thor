# Thor Test Plan (M1 / M2)

**Scope:** Binding FS / UI / SEC requirements  
**Baseline commands:** `cargo test --lib`, `cargo test --test integration_tests`  
**Engine reality:** Live shell is WebKit/AppKit chrome; Chromium/CDP is in-tree but not the live content path until M2. Tests must assert honesty (`__thorEngineStatus`), not pretend CDP works on WebKit.

---

## 1. Test strategy

| Layer | What it owns | When it runs | Pass bar |
|-------|--------------|--------------|----------|
| **Unit (`cargo test --lib`)** | Pure engines: shield, tamper, Locust scorer/runner, profiles, decoder, tab history, AppState singleton, SSL CA gen, bookmarks, palette builders | Every PR | Green; no flake |
| **Integration (`cargo test --test integration_tests`)** | Cross-module contracts: shield×profiles, Locust×local TCP, AppState×tamper/security, palette Cmd+K vs Cmd+⇧K payloads, SSL temp-dir gen | Every PR | Green; covers FS-072 |
| **Manual (macOS shell)** | Hit-testing, shortcuts, chrome IA, visual honesty, consent UX, content-vs-chrome IPC isolation | Before M1/M2 claim; after shell/UI changes | Checklist signed |
| **Perf smoke** | Chrome ready, palette open, Locust UI throttle, frozen-tab RSS (report-only until baselines) | Milestone; not fail-CI until baselines exist | Numbers published; no sleep-as-proof |
| **Live Chromium (`live_launch_smoke`, `#[ignore]`)** | Real browser launch / navigate / freeze | M2 only; CI may skip | Documented; run locally with `THOR_CHROMIUM_PATH` |

**Principles**

- Map every §8 FS acceptance + SEC-T01–T16 to a test ID (auto or manual).
- Prefer local `TcpListener` / temp `THOR_DATA_DIR` over public hosts.
- Separate **M1 (WebKit lean DX)** from **M2 (Chromium lab)**. Fail M2 claims if intercept/freeze/viewport only work on WebKit theater.
- Security gates (consent, caps, key perms) are Rust-side; UI-only checks are insufficient.

---

## 2. Traceability matrix

Legend: `UT-*` unit, `IT-*` integration (existing or planned), `MAN-*` manual, `PERF-*` perf smoke, `SEC-T*` required security cases.

### Architecture / engine

| Req | Tests |
|-----|-------|
| FS-001 | UT-APP-01 (`test_app_state_singleton_engines`), IT-APP-01 (`test_app_state_global_tamper_and_security`) |
| FS-002 | MAN-ENG-01, UT-CR-live (`live_launch_smoke`) — **M2** |
| FS-003 | **ADD** UT/IT-ENG-STATUS-01, MAN-ENG-02 |
| FS-004 | MAN-ENG-03; **ADD** UT-FREEZE-01 (CDP path) — **M2** |
| FS-005 | **ADD** UT-HIST-GC-01 (injectable clock); MAN-PRIV-03 |
| FS-006 | UT-PREWARM / runtime config — **M2 SHOULD** |
| FS-ARCH-01–04 | MAN-ENG-01–02; gate via FS-003 |

### Browse / privacy / IPC

| Req | Tests |
|-----|-------|
| FS-010 | MAN-BRW-01..05 |
| FS-011 / UI-003–004 | MAN-HIT-01..08 |
| FS-012 / UI-020 | IT-PAL-01 (`test_cmd_k_palette_*`, `test_cmd_k_payload_json_shape`), MAN-PAL-01 |
| FS-013 / UI-021 | IT-PAL-02 (`test_cmd_k_payload_json_shape` settings excl., `test_settings_registry_excludes_bookmarks`), MAN-PAL-02 |
| FS-014 | MAN-BRW-06 |
| FS-PRIV-01–03 / UI-182 | UT-HIST-01 (`test_clear_tab_history_privacy_on_close`), IT-PAL-01 clear path, MAN-PRIV-01..02, SEC-T16 |
| FS-PRIV-04 / SEC-041 | **ADD** IT-SEC-REDACT-01 |
| FS-IPC-01 / SEC-090 | **ADD** IT-IPC-01; MAN-IPC-01; SEC-T13 |
| FS-IPC-02 | IT-LOC-01 (`test_locust_http_runner_*`, `test_app_state_locust_start_stop`) |
| FS-IPC-03 / FS-043 | MAN-TAM-WK-01; **ADD** IT-ENG-STATUS-01 — WebKit path |
| FS-070 / UI-007 | MAN-BRAND-01; grep/smoke strings |
| FS-071 / UI-180–181 | MAN-HON-01..04 |
| FS-072 | Gate: both cargo test suites green |
| FS-073 | Documented ignored smoke — **M2** |

### API workbench

| Req | Tests |
|-----|-------|
| FS-020 / UI-080–083 | **ADD** IT-API-01..03; MAN-API-01..04 |
| FS-021 / UI-086 | **ADD** IT-API-ENV-01; MAN-API-05 |
| FS-022 / UI-084 | **ADD** IT-API-PERS-01; MAN-API-06 |
| FS-023 / FS-060 | **ADD** IT-API-LOC-01; MAN-INT-01 |
| FS-061 | MAN-INT-02 |
| UI-024 | MAN-API-07 |

### Locust

| Req | Tests |
|-----|-------|
| FS-030–032 / UI-100–104 | IT-LOC-01..02, UT-LOC-scorer_*, MAN-LOC-01..05; SEC-T05 |
| FS-033 / SEC-062 | **ADD** IT-LOC-CAP-01; MAN-LOC-06; SEC-T04 |
| SEC-003–004 / SEC-060–061 | MAN-LOC-CONSENT-01..02; SEC-T04–T05 |
| SEC-063 | MAN-LOC-03; IT-LOC-stop idempotent **ADD** |
| UI-103 run pill | MAN-LOC-04 |
| UI-106 throttle | PERF-LOC-01 |

### Security / profiles / SSL / decoder

| Req | Tests |
|-----|-------|
| FS-040 / SEC-070–071 | UT-SHIELD-*, IT-shield_*; SEC-T06–T07 |
| FS-041 / SEC-010–012 | UT-PROF-01, IT-PROF-01; MAN-PROF-01; SEC-T06–T08 |
| FS-042 / SEC-020–026 | UT-TAMPER-*, IT-tamper; **ADD** consent tests SEC-T01–T03 |
| FS-043 | MAN-TAM-CR-01 (**M2**); MAN-TAM-WK-01 (**M1**) |
| FS-044 / SEC-050–052 | UT-DEC-*, IT-decoder; **ADD** UT-DEC-CRACK-01; SEC-T09–T10 |
| FS-045 / SEC-030–035 | IT-SSL-01; **ADD** UT-SSL-PERMS-01; MAN-SSL-01..02; SEC-T11–T12 |
| FS-PROF-01–02 | **ADD** IT-PROF-PERS-01; MAN-PROF-02 |
| SEC-013 | MAN-PROF-03 (**M2** partitions) |
| SEC-015 | MAN-PROF-04; SEC-T08 |
| SEC-080 / UI-181 | MAN-HON-03 (extensions Info-only; no WASM load) |
| SEC-091 / SEC-T14 | MAN-IPC-02 |
| SEC-T15 | Existing lib + integration suites |
| SEC-T16 | UT-HIST + MAN-PRIV + **ADD** GC body clear |

### UI chrome / a11y (mostly manual)

| Req | Tests |
|-----|-------|
| UI-001–005, 060–065 | MAN-VIS-01..06 |
| UI-006, 100–103 | MAN-MODE-01..03 |
| UI-022–023 | MAN-KEY-01..06 |
| UI-040–044 | MAN-SURF-01..03 |
| UI-120–123 | MAN-DEV-01..03 (**M2** for real viewport) |
| UI-140–142 | MAN-SEC-UI-01..03 |
| UI-160–164 | MAN-A11Y-01..04 |

---

## 3. Manual exploratory checklist (macOS shell)

Run with `cargo run --bin Thor` (or `./scripts/run_dev.sh`). Record backend from status chip / `__thorEngineStatus` if present.

### A. Toolbar / hit-testing (`FS-011`, `UI-003–004`)

| ID | Steps | Pass |
|----|-------|------|
| MAN-HIT-01 | Load any page; click omnibox; type URL; Enter | Focus + navigate |
| MAN-HIT-02 | Click Back / Forward / Reload / New tab / Close tab | Each works; no dead zone |
| MAN-HIT-03 | Rapid tab switch ×10 then click omnibox | Still selectable |
| MAN-HIT-04 | Drag empty chrome background → move window | Window moves |
| MAN-HIT-05 | mousedown on Back/omnibox then release | Click completes (no drag steal) |
| MAN-HIT-06 | Traffic lights vertically centered in 52px bar | Visual align ~`(14,18)` |
| MAN-HIT-07 | Confirm no HTML faux traffic lights + AppKit lights | Single light set |
| MAN-HIT-08 | Confirm no dual always-on WKWebView fighting hits | Content + native strip only |

### B. Keyboard / palettes

| ID | Steps | Pass |
|----|-------|------|
| MAN-PAL-01 | ⌘K with 2 tabs of history | Bookmarks + **active** history + open tabs only; other tab’s URLs absent |
| MAN-PAL-02 | ⌘⇧K | Settings/DX/security/tools only; no bookmarks/history dump; paths `Thor://…` |
| MAN-KEY-01 | Content focused: ⌘L, ⌘T, ⌘W, ⌘R, ⌘[/⌘] | All fire |
| MAN-KEY-02 | Esc closes top overlay only | Panel/palette dismiss; tab stays |
| MAN-KEY-03 | ⌘⇧L Load; ⌘⇧. start/stop (confirm stop) | Surfaces correct |
| MAN-API-07 | API focused: ⌘Enter Send | Request fires |
| MAN-BRW-01..05 | New/switch/close/nav/back/forward/reload from toolbar + IPC | End-to-end |
| MAN-BRW-06 | Navigate; omnibox + title update | Sync |

### C. Profiles

| ID | Steps | Pass |
|----|-------|------|
| MAN-PROF-01 | Toggle Stealth; open `…?utm_source=x&keep=1` | `utm_*` gone; notice if strip occurred; chip shows Stealth |
| MAN-PROF-02 | Quit/relaunch | Profile persisted (`FS-PROF-02`) |
| MAN-PROF-03 | DevHost then Stealth cookies (M2) | No silent identity merge |
| MAN-PROF-04 / SEC-T08 | Stealth + open IDE | Bridge inactive / refuse |
| SEC-T07 | DevHost + same utm URL | Params preserved |

### D. Locust

| ID | Steps | Pass |
|----|-------|------|
| MAN-LOC-01 | Start against local stub | Real RPS/latency; not static RNG |
| MAN-LOC-02 | Double Go | One swarm; UI idempotent |
| MAN-LOC-03 | Stop twice | Idempotent; Stop always reachable |
| MAN-LOC-04 | Browse while running | Run pill only; full metrics only on Load |
| MAN-LOC-05 | Secondary strip exclusivity | Locust then Tamper → one strip; confirm if swarm running |
| MAN-LOC-CONSENT-01 / SEC-T05 | `127.0.0.1` | Basic confirm; start OK |
| MAN-LOC-CONSENT-02 / SEC-T04 | `https://example.com` | Public ack + caps; blocked without |
| MAN-INT-02 | ⌘⇧K “Load test this origin” | Host = active tab origin |

### E. Tamper / decoder / SSL

| ID | Steps | Pass |
|----|-------|------|
| SEC-T01 | Enable intercept without scope accept | Blocked (Rust + UI) |
| SEC-T02 | Rule pattern empty / `*` | Reject or dual-confirm + audit |
| SEC-T03 | Apply rule | Audit entry; secrets redacted |
| MAN-TAM-WK-01 | Intercept ON on WebKit | Explicit “requires Chromium”; no silent drop |
| MAN-TAM-CR-01 | Intercept ON on Chromium (M2) | Pause → forward/drop |
| SEC-T09–T10 | JWT decode + crack | Header/payload shown; crack unavailable / no “verified” |
| SEC-T11 | Generate CA | Key owner-only perms |
| SEC-T12 | Install trust without typing `TRUST THOR DEV CA` | Blocked |
| MAN-SSL-02 | Uninstall/distrust path visible | Instructions or one-click |

### F. Honesty / brand / a11y / IPC negative

| ID | Steps | Pass |
|----|-------|------|
| MAN-HON-01 | Locust idle | Metrics not fabricating activity |
| MAN-HON-02 | Decoder crack UI | Unavailable, not success |
| MAN-HON-03 | Extensions in ⌘⇧K | Badge **Info**, not Active; no load |
| MAN-HON-04 | No 3D DOM / fake extension toggle | Absent |
| MAN-BRAND-01 | UI strings / routes | **Thor**, `Thor://…`, never `.thor` |
| MAN-IPC-01 / SEC-T13 | Unknown/malformed IPC | No panic; no-op |
| MAN-IPC-02 / SEC-T14 | Privileged cmd from content page | No effect |
| MAN-A11Y-01..04 | Keyboard chrome path; icon labels; contrast; Reduce Motion | Meet UI-160–164 |
| MAN-PRIV-01 | Close tab | Its history gone from ⌘K |
| MAN-PRIV-02 | ⌘⇧K clear-all wipe | Histories cleared |
| MAN-VIS-01..06 | 52px bar, glass omnibox, no purple theme, no action-button soup, flush controls | Visual |

---

## 4. Automated tests: KEEP vs ADD

### KEEP (already green — do not drop)

**Lib (`cargo test --lib`) — representative**

| Test | Asserts |
|------|---------|
| `test_app_state_singleton_engines` | One `AppState`; profiles/tamper/security/history shared |
| `test_app_state_locust_start_stop` | Real HTTP via global Locust; `total_requests > 0` |
| `test_profile_manager_defaults_and_toggle` | DevHost↔Stealth flags (strip, IDE, fingerprint) |
| `test_sanitize_url_*` / fragment | Tracker strip preserves non-trackers |
| `test_dnt_gpc_headers` | DNT/GPC present |
| `test_interceptor_toggle_and_process` / rule / drop | Tamper engine basics |
| `test_jwt_detection_and_decoding` / base64 | Decode only |
| `test_locust_metric_scoring` / `test_locust_runner_hits_local_http_server` | Real runner + scorer |
| `test_generate_ca_and_leaf_writes_pem` | CA/leaf in temp dir |
| `test_tab_history_tracker` / `test_clear_tab_history_privacy_on_close` | Tab-scoped + clear |
| `test_build_cmd_k_items_*` / settings exclude bookmarks | Palette purity helpers |
| `test_bookmark_store_*` | Persistence under data dir |
| `live_launch_smoke` (`#[ignore]`) | Keep ignored; document for M2 |

**Integration — keep all current suites**, especially:

- Shield completeness / strip / clean URL / headers  
- Cmd+K / Cmd+⇧K payload shapes + history clear  
- Locust scorer percentiles + `test_locust_http_runner_against_local_listener`  
- Tamper / profile / decoder / SSL generate / AppState global  

**Caution:** `test_wasm_plugin_microkernel_integration` and extension “Active” badge assumptions conflict with SEC-080 / UI-181. Keep registration unit behavior if registry-only; **add** assertion that UI/product cannot enable real WASM load, and extension badge is Info (manual + future IT).

### ADD (named + assertion)

| ID | Name (suggested) | Asserts |
|----|------------------|---------|
| UT-DEC-CRACK-01 | `decode_jwt_crack_returns_unavailable` | Structured `available: false` / error; never “HMAC verified” |
| UT-SSL-PERMS-01 | `ca_private_key_owner_readonly` | Mode `0o600` (or platform equiv) under temp data dir |
| UT-HIST-GC-01 | `idle_tab_gc_clears_history_with_injected_clock` | Idle ≥ TTL → clear + notify payload shape |
| UT-TAM-VALID-01 | `reject_empty_or_star_pattern_without_confirm` | SEC-021 |
| UT-LOC-CAP-01 | `locust_rejects_over_cap_users` | users > cap → error |
| UT-LOC-IDEMP-01 | `locust_double_start_single_swarm` | Second start replaces; one runner |
| UT-IPC-UNK-01 | `unknown_ipc_cmd_noop` | No panic |
| IT-ENG-STATUS-01 | `engine_status_reports_backend_and_intercept` | `webkit\|chromium`, `cdp`, `intercept` honest |
| IT-PROF-NAV-01 | `stealth_sanitize_on_navigate_path` | ProfileManager Stealth → strip on; DevHost → off via AppState |
| IT-PROF-PERS-01 | `profile_mode_persists_across_reopen` | Write/read under temp `THOR_DATA_DIR` |
| IT-API-01 | `api_send_hits_local_server` | Status/latency/body real |
| IT-API-ENV-01 | `env_var_substitution_resolves_base` | `{{BASE}}/x` hits path |
| IT-API-PERS-01 | `collection_survives_reopen` | Save → reopen store |
| IT-API-LOC-01 | `locust_from_request_shares_url_and_source` | URL match; `source=request_id` |
| IT-SEC-REDACT-01 | `secrets_absent_from_locust_stats_json` | Secret env not in metrics payload |
| IT-CONSENT-01 | `interception_blocked_without_scope` | SEC-T01 |
| IT-CONSENT-02 | `locust_public_host_requires_ack` | SEC-T04 |
| IT-IPC-MAL-01 | `malformed_ipc_payload_rejected` | SEC-096 / T13 |
| IT-AUDIT-01 | `tamper_apply_writes_audit_redacted` | SEC-T03 |
| IT-SHIELD-PROF-01 | `devhost_preserves_utm_stealth_strips` | SEC-T06/T07 at AppState layer |

**Do not add** exploit PoCs, mass scanners, or silent MitM tests against public hosts.

---

## 5. Release gate checklist

### Before claiming **M1 — Lean DX** (WebKit OK)

- [ ] `cargo test --lib` green  
- [ ] `cargo test --test integration_tests` green (FS-072)  
- [ ] MUST FS IDs: 001, 010–014, 020–023, 030–032, 040–042, 044–045, 060–061, 070–072  
- [ ] MUST UI: 001–005, 020–021, 003–004 hit-test, 100–104 Locust disclosure, 180–182 honesty  
- [ ] SEC release criteria 1–8 where applicable without Chromium Fetch  
- [ ] SEC-T01–T13, T15–T16 executed; T14 attempted (content IPC); T06–T10, T11–T12 signed  
- [ ] Intercept on WebKit: **explicit unsupported** (FS-IPC-03) — not silent  
- [ ] No stub theater (fake Locust / HMAC / extensions Active / 3D DOM)  
- [ ] Manual MAN-HIT + MAN-PAL + MAN-LOC + MAN-PROF signed on macOS  
- [ ] Paths: Thor data dir, never `~/.thor`; brand **Thor**  
- [ ] Declare milestone: **M1** in release notes / PR  

### Before claiming **M2 — Dev browser** (Chromium required)

All M1 gates, plus:

- [ ] Shell drives `ChromiumRuntime` + `TabSession` for content  
- [ ] FS-002–005, 043, engine status shows `chromium` + `cdp: true`  
- [ ] SEC-T14 verified via CDP/content inject — privileged IPC inert  
- [ ] Page intercept: pause / forward / drop (FS-043)  
- [ ] Freeze on background / unfreeze on focus measurable  
- [ ] SHOULD FS-050–051 / UI Device presets if claimed  
- [ ] `live_launch_smoke` run locally (CI may still ignore)  
- [ ] Perf baselines reported separately for Chromium (no blend with WebKit)  
- [ ] Declare milestone: **M2**  

### Shared security hard fails (any milestone)

- Intercept enabled without consent  
- Locust public target without dual confirm / caps bypassed in Rust  
- Profile label ≠ `ProfileManager` behavior  
- Fake crack success  
- World-readable CA key  
- Secrets in Locust/tech-stack logs  
- WASM/MV3 real load in v1  

---

## 6. Bug severity rubric (lean browser + lab tools)

| Severity | Definition | Examples | Release impact |
|----------|------------|----------|----------------|
| **S0 Blocker** | Data loss, machine compromise path, or security gate bypass | CA install without typed confirm; content IPC starts Locust/tamper; silent system MitM; secrets in default export/logs; crash loop on launch | **No M1/M2 claim** |
| **S1 Critical** | Core journey broken or honesty violated | Dead toolbar (movable-by-background); Locust RNG theater; Cmd+K shows other-tab history; intercept “works” on WebKit by dropping traffic; double swarm; profile toggle UI-only | Block milestone |
| **S2 Major** | Important MUST degraded; workaround exists | Omnibox sync lag; collection not persist; Stop buried; secondary strips stack; Stealth notice wrong; JWT decode fails on valid token; caps only in UI | Fix before release if MUST; else document |
| **S3 Minor** | SHOULD / polish / density | Truncation, throttle >4 Hz, comfort density missing, tech-stack shallow, palette ranking soft | Backlog |
| **S4 Cosmetic** | Visual nit without IA/a11y break | Hairline color, icon tint, 2px misalignment | Optional |

**Lab-tool bias:** Misuse of Locust/tamper/CA against unintended hosts without consent → treat as **S0/S1** even if “feature works.” Fake security success → **S1**. Hit-testing regressions after shell edits → **S1** (known Thor failure mode).

**Engine honesty:** Claiming Chromium/CDP behavior while `__thorEngineStatus` says WebKit → **S1**.

---

## Coverage gaps vs current suite (summary)

| Area | Today | Gap |
|------|-------|-----|
| Shield / Locust scorer / local Locust / profiles toggle / tamper rules / JWT decode / SSL gen / history clear / AppState | Strong | — |
| Consent scope, public Locust ack, pattern validation, audit log | Missing | SEC-T01–T04, T03 |
| API workbench IPC + env + persistence | Missing | FS-020–023 |
| Engine status / WebKit intercept messaging | Missing | FS-003, FS-043 |
| JWT crack unavailable API | Missing | SEC-T10 |
| CA key permissions + trust UX | Partial (gen only) | SEC-T11–T12 |
| Idle GC with clock inject | Missing | FS-005 |
| Profile persistence across launch | Missing | FS-PROF-02 |
| Malformed/unknown IPC + content isolation | Missing auto | SEC-T13–T14 |
| Secret redaction in Locust UI JSON | Missing | FS-PRIV-04 |
| macOS chrome / shortcuts / modes | Manual only | Expected |

---

**Definition of done for this plan:** Every MUST acceptance in FS §8 and SEC-T01–T16 maps to a test ID above; M1/M2 gates are explicit; keep-list preserves FS-072 coverage; add-list closes consent, workbench, honesty, and IPC gaps without implementing exploits.