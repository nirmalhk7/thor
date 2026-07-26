# Thor Performance Plan (skeptical, measurable)

**Verdict:** Current `benches/` cannot support a “better than Chrome/Firefox” claim. Three of four Criterion benches are theater (`sleep`, dummy IPC, heap-alloc sim). Real microbenches exist only in `cmd_palette_bench`. Credible wins start at **M2** (Chromium content + freeze live in shell). M1 can only claim chrome/DX latency and Locust overhead—not browser RSS/tab-switch superiority.

---

## 1. Definition of “better than Chrome/Firefox”

### What we are allowed to claim

Thor is **not** “faster Chromium rendering.” Claim scope:

| Claim class | Allowed when | Forbidden until |
|---|---|---|
| **Dev-workflow latency** (chrome ready, Cmd+K, omnibox→commit, Locust idle tax) | Measured on same machine, same pages | Marketing without harness |
| **Idle multi-tab RSS** | M2 + CDP freeze wired in shell; vs stock Chrome/FF with same N tabs | WebKit hide/bounds “freeze” |
| **Tab switch feedback** | Chrome UI + content unfreeze timed separately | Blending shell paint with content thaw |
| **Lean defaults** | No MV3 host, no sync, tab-scoped history, 24h GC | Claiming those equal lower RSS without measuring |

**Primary competitor posture:** beat stock browsers on **developer session cost** (many idle tabs + DX tools), not Speedometer/JetStream.

### KPI table (apples-to-apples)

| KPI ID | Metric | Method | Thor target | Pass vs Chrome/FF |
|---|---|---|---|---|
| **P-COLD** | Cold start → chrome interactive | `process_start` → first chrome paint / hit-testable omnibox (instrumented marker or Accessibility hit). Exclude Chromium binary download & first profile create. | **&lt; 500 ms** chrome interactive (FS §12) | Thor chrome ≤ Chrome first-window chrome; **content** cold is separate (P-CONTENT-COLD) |
| **P-CONTENT-COLD** | Content engine ready | Chromium launch → CDP `Target.getTargets` / first blank page | Report-only M1; M2: document p50/p95 | Not required to beat Chrome (we spawn Chromium) |
| **P-NAV** | Omnibox navigate (cached warm origin) | Type URL → Enter → `Page.loadEventFired` or equivalent commit marker | p95 commit ≤ Chrome/FF on same URL ±10% | **Parity OK**; win is not the page engine |
| **P-SWITCH-UI** | Tab switch chrome feedback | `switch_tab` IPC → chrome tab strip active style applied | **&lt; 100 ms** (FS §12) | Must beat or match Chrome tab-strip feel |
| **P-SWITCH-CONTENT** | Content unfreeze | CDP freeze→unfreeze → page interactive again | **p95 &lt; 250 ms** Chromium (FS §12) | Better if freeze kept RSS down without worse thaw than Chrome discard/reload |
| **P-RSS-1/10/50** | Process-tree RSS idle | After N tabs loaded to same fixture set, idle 60s; sum RSS of Thor shell + content process tree; Chrome: all related PIDs; Firefox: main+content | Document absolute MB; **growth factor** frozen vs unfrozen | Thor **idle RSS @ 10 & 50** ≤ Chrome & FF under freeze-on policy (M2 gate) |
| **P-FREEZE-DELTA** | RSS savings from freeze | Same 20 tabs: unfrozen baseline vs frozen background | Document factor; expect material drop vs unfrozen Thor | Required evidence for “lighter” |
| **P-LOCUST-IDLE** | Locust overhead when idle | Swarm stopped: CPU% of Thor vs baseline no-Locust 60s | Metrics pipeline idle ≈ 0; no polling storm | N/A vs browsers |
| **P-LOCUST-RUN** | Metrics UI tax while running | 100 users local stub: CPU of metrics→UI path vs swarm HTTP CPU | Metrics pipeline **&lt; 5%** of swarm CPU; UI throttle **≥ 250 ms** (≤4 Hz) | N/A |
| **P-CMDK** | Cmd+K open latency | Keydown → palette visible + first results painted | p95 **&lt; 50 ms** open; search stays sub-ms on realistic N (not fake 10k UI DOM) | Beat Chrome omnibox/history UI for equivalent item count |
| **P-GC** | 24h idle GC correctness + cost | Injectable clock: idle ≥ TTL → `__thorRemoveTab`, history cleared, RSS drops | Functional MUST; GC sweep CPU negligible on 3‑min interval | Privacy+memory lever Chrome lacks by default |

### Methodology rules (non-negotiable)

1. **Same machine, quiet**, plugged-in, release builds (`--release`), screen awake, no Spotlight indexing storms.
2. **Same fixture pages** (local `fixtures/perf/` static sites: light HTML, SPA shell, media-heavy)—never live news sites for RSS claims.
3. **Same tab count and URLs** across Thor / Chrome / Firefox.
4. **Separate engines:** never blend M1 WebKit numbers with M2 Chromium; never blend chrome WKWebView RSS with content Chromium RSS without labeling.
5. **Warm vs cold labeled.** First-run profile create excluded from P-COLD.
6. **Chrome baseline:** system Google Chrome, fresh profile under `/tmp/thor-bench-chrome-profile`, extensions disabled, sync off. **Firefox:** fresh profile, same.
7. **Process accounting:** macOS `ps -o rss=` / `footprint` on process **tree** rooted at browser PID (Chrome helper processes included). Script must document which PIDs counted.
8. **No sleep theater.** Markers from real events (IPC timestamps, CDP events, `Instant` around Rust handlers).
9. **N ≥ 30** for latency KPIs; report p50/p95/max. RSS: 5 runs, median.
10. **Honesty banner:** “Thor content = Chromium; wins are freeze/GC/no-MV3/lean chrome, not Blink faster.”

---

## 2. Benchmark harness plan

### Current state (debt)

| Bench | Reality | Action |
|---|---|---|
| `cold_start_bench.rs` | `thread::sleep(100µs)` | **Delete or replace** with process spawn + marker |
| `ipc_bench.rs` | `dummy_spsc_write` | Replace with real chrome→Rust IPC round-trip or drop |
| `memory_soak_bench.rs` | Alloc 50×1MB Vec | Replace with live RSS probe script |
| `cmd_palette_bench.rs` | Real Rust microbenches | Keep; split from E2E; stop claiming “browser faster” |

### Harness layout (proposed)

```
scripts/perf/
  compare_browsers.sh      # orchestrates Thor vs Chrome vs Firefox
  measure_rss.sh           # process-tree RSS for a PID
  fixtures/                # static HTTP via python/axum
benches/
  micro/                   # criterion: shield, locust score, palette search
  e2e/                     # ignored by default; needs GUI + Chromium
tools/perf_runner/         # optional small Rust bin: emits JSON markers
```

### Local scripts (fair compare)

**A. Cold chrome (`P-COLD`)**  
- Launch `Thor` with `THOR_PERF_MARKERS=1` → stdout JSON lines: `{"event":"chrome_interactive","t_ms":…}`.  
- Chrome/FF: use AppleScript/`osascript` or Playwright only to open window; measure time-to-omnibox via Accessibility—or accept “report Thor-only until automation exists” (do not fake Chrome cold with sleep).

**B. RSS idle 1/10/50 (`P-RSS-*`)**  
1. Start fixture server.  
2. Open N tabs to fixture URLs.  
3. Sleep 60s (settle).  
4. Snapshot RSS tree.  
5. For Thor M2: assert background tabs frozen (lifecycle state / logs).  
6. Emit `perf-results/rss-{browser}-n{N}.json`.

**C. Tab switch (`P-SWITCH-*`)**  
- Thor: instrument `switch_tab` handler: `t0` IPC receive, `t1` chrome eval done, `t2` CDP unfreeze ack.  
- Chrome/FF: Playwright `page.bringToFront` / keyboard Cmd+N tabs—**UI latency only comparable with high caution**; primary gate is Thor absolute SLAs + RSS.

**D. Omnibox navigate (`P-NAV`)**  
- Local fixture with cache headers; 30 navigations; CDP `load` / WebKit equivalent.

**E. Locust idle/run (`P-LOCUST-*`)**  
- Local axum stub; `sample` CPU via `ps` on Thor PID during idle and during 100-user run; parse Locust update interval ≥250ms.

**F. Cmd+K (`P-CMDK`)**  
- Marker: key event → `__thorCmdKPayload` applied + first paint. Microbench remains for search algo.

### CI policy

- **Until baselines exist:** report-only job (`cargo bench` micro + JSON artifacts).  
- **After baselines checked in:** fail on **>10%** regression on micro KPIs; E2E RSS/latency nightly, not PR-blocking until flake &lt;5%.  
- Never fail CI on Chrome comparison variance (machine noise)—comparisons are release-evidence, not PR gates.

### Fairness checklist vs system browsers

- [ ] Fresh profiles all three  
- [ ] Same fixture URLs  
- [ ] Extensions off / MV3 host absent in Thor by design (document as product difference, not cheat)  
- [ ] Same display scale / window size  
- [ ] Dual-window Thor: **measure content window + chrome**; label dual-surface overhead until CEF  
- [ ] Do not compare Thor WebKit content RSS to Chrome and call it a win for “developer browser”

---

## 3. Architecture perf bets that must ship

These are the **only** credible structural advantages. Ship or drop the marketing line.

| Bet | Requirement | Why it wins | Risk if skipped |
|---|---|---|---|
| **CDP freeze inactive tabs** | FS-004; `also_os_suspend: false` default (NG-11) | Stops JS timers/raf on background; RSS/CPU | “Lighter” false on WebKit |
| **24h idle tab GC** | FS-005; `TAB_IDLE_TTL` + `__thorRemoveTab` + `clear_tab_history` | Hard cap on zombie tabs | RSS converges to Chrome with 50 forever-tabs |
| **Prewarm pool ≥ 2** | FS-006 | Amortize target create on `new_tab` | New-tab latency worse than Chrome |
| **One Chromium process, many targets** | FS-002 / AGENTS | Avoid Chrome-per-tab fork tax | Catastrophic RSS |
| **No MV3 host / no Google sync** | NG-02/03, SEC-080 | Idle CPU + disk + network absent | Parity bloat |
| **Tab-scoped history only** | FS-PRIV-01/02 | No global DB growth; Cmd+K stays small | Palette/history cost → Chrome-like |
| **Single `AppState::global()`** | FS-001 | No per-IPC engine construct | Latency + memory churn |
| **No dual always-on content WebViews** | UI handoff; AGENTS dual-window = chrome + one content surface | Hit-testing + compositor cost | UI-anti-pattern #4 |
| **Locust UI ≤4 Hz** | FS §12 / UI-Perf | Swarm doesn’t melt chrome | Fake “browser jank” during load test |
| **Soft-fail freeze honesty** | FS-003 | Don’t claim freeze on WebKit | Credibility failure |

**Do not bet on:** OS `SIGSTOP` of whole Chromium process for per-tab freeze (one process → stops everything). Per-tab = CDP lifecycle only.

---

## 4. Anti-regressions / budgets

### Absolute budgets (gate after baseline)

| Area | Budget |
|---|---|
| Chrome interactive cold | &lt; 500 ms |
| Tab switch UI | &lt; 100 ms |
| Content unfreeze p95 (M2) | &lt; 250 ms |
| Cmd+K open p95 | &lt; 50 ms |
| Palette search (Rust, ≤2k items realistic) | &lt; 1 ms release |
| Tracker strip / URL | keep sub-10 µs release (SEC-110 OK) |
| Locust score 10k metrics | &lt; 1 ms release |
| Locust UI push interval | ≥ 250 ms |
| Prewarm pool | ≥ 2 (config) |
| Tamper history | bounded count+bytes (SEC-024)—**not** unbounded for speed |
| Max Locust users default | SEC-062 caps—**not** raised for bench glory |

### Regression policy

- Micro: **+10%** vs `perf-baselines/micro.json` → fail.  
- RSS@50 frozen: **+15%** → fail nightly.  
- New always-on WebView / per-IPC `Engine::new` / global history DB → **hard fail** review (architecture regression).

### Instrumentation budget

Markers must be zero-cost when `THOR_PERF_MARKERS` unset (cfg or atomic flag)—no permanent tracing tax in release defaults.

---

## 5. What NOT to optimize if it breaks SEC-100–106

| SEC | Do not “optimize” by… |
|---|---|
| **SEC-100** Consent gates | Skipping confirm dialogs, caching “always allow public Locust,” pre-arming intercept on prewarm tabs |
| **SEC-101** IPC allowlist/validation | Removing schema checks, widening cmds for fewer copies, reflecting content→privileged IPC |
| **SEC-102** Locust caps/cancel | Raising users/RPS to win swarm benches; ignoring stop |
| **SEC-103** Secret redaction | Logging full metrics/headers for “faster debug”; skipping redact allocs |
| **SEC-104** CA key perms / trust UX | World-readable keys; auto-trust without typed confirm |
| **SEC-105** Profile isolation | Sharing DevHost/Stealth cookie jars for faster profile toggle |
| **SEC-106** Audit log | Dropping audit writes; unbounded silent drop; sync-skip without fail-safe |

**Allowed (SEC-110–113):** strip hot path, Locust scoring allocs, CDP Fetch latency when opt-in+scoped, prewarm **if** frozen/prewarm tabs cannot run Locust/tamper without consent.

**Also do not optimize:** fake freeze via hiding WKWebView; dual WebView chrome; MV3 “for parity”; SIGSTOP whole browser as tab freeze.

---

## 6. M1 vs M2 perf acceptance gates

### M1 — Lean DX (WebKit content OK)

**May claim:** chrome/DX snappy; Locust real and cheap when idle; no theater benches as proof of browser superiority.

| Gate | Criteria |
|---|---|
| M1-P1 | Replace sleep/dummy Criterion benches with real micro **or** remove them from `cargo bench` success path |
| M1-P2 | `cmd_palette_bench` green release; Cmd+K open instrumented or manual p95 &lt; 50 ms |
| M1-P3 | Cold **chrome** interactive &lt; 500 ms (Thor-only OK) |
| M1-P4 | Locust idle CPU ≈ baseline; run path UI ≤4 Hz; metrics pipeline &lt; 5% swarm CPU @ 100 users local |
| M1-P5 | Omnibox navigate works; no dual always-on tool WebViews |
| M1-P6 | GC + tab history clear tested (clock inject); no global history DB |
| M1-P7 | `__thorEngineStatus` honest: freeze/intercept not claimed on WebKit |
| M1-P8 | **No** public claim “lighter than Chrome on 50 tabs” |

**M1 explicitly does not gate:** P-RSS vs Chrome, P-SWITCH-CONTENT, freeze SLAs.

### M2 — Developer browser (Chromium content required)

**May claim:** idle multi-tab RSS and freeze/switch SLAs vs Chrome/Firefox with published methodology.

| Gate | Criteria |
|---|---|
| M2-P1 | Shell drives `ChromiumRuntime` + `TabSession`; one browser process |
| M2-P2 | Background tabs CDP-frozen; switch: UI &lt; 100 ms, unfreeze p95 &lt; 250 ms |
| M2-P3 | RSS methodology published; Thor idle RSS @ **10 and 50** tabs ≤ Chrome and ≤ Firefox on same fixtures (median of 5), **or** documented exception with freeze-delta still showing clear win vs unfrozen Thor |
| M2-P4 | Prewarm ≥ 2; new tab from pool measured |
| M2-P5 | 24h GC closes Chromium targets + clears history + UI hook |
| M2-P6 | No MV3 host; AppState singletons; no per-tab Chrome process |
| M2-P7 | Compare script `scripts/perf/compare_browsers.sh` produces dated JSON artifact |
| M2-P8 | SEC-100–106 still hold under perf markers / prewarm (consent not skipped) |
| M2-P9 | Dual-window overhead labeled; CEF not required to pass M2 if content RSS wins hold |

### Claim matrix

| Statement | M1 | M2 |
|---|---|---|
| “Cmd+K / chrome faster for DX” | ✅ if measured | ✅ |
| “Locust doesn’t tax idle browser” | ✅ | ✅ |
| “Lighter than Chrome with many tabs” | ❌ | ✅ only with P-RSS evidence |
| “Tab freeze saves memory” | ❌ (soft-fail only) | ✅ |
| “Faster page load than Chrome” | ❌ do not claim | ❌ do not claim (same Blink) |

---

## Execution order (perf eng)

1. **Kill theater benches** → real micro + marker stubs.  
2. **Fixture server + RSS scripts** (Thor-only first).  
3. **Instrument** switch / Cmd+K / chrome_interactive / Locust throttle.  
4. **M1 gates** green.  
5. **M2 Chromium cutover** → freeze path live → cross-browser compare.  
6. Lock baselines; enable +10% micro regression in CI.

---

## Skeptical bottom line

Thor’s credible edge is **policy + lifecycle** (freeze, GC, no sync/MV3, tab-scoped history, lean chrome, single AppState)—not a faster renderer. Until M2 freeze is in the live shell and RSS is measured against stock Chrome/Firefox on identical fixtures, “better performing developer browser” is an **architecture hypothesis**, not an evidence-backed claim. Existing Criterion cold/IPC/memory benches must not ship as proof.