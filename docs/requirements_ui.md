# Thor UI/UX Requirements

**Document type:** Product / UI requirements (not implementation)  
**Audience:** Developer, Tester, Perf agent  
**Context:** Matte obsidian aesthetic; native macOS AppKit unified titlebar (52px) is the live chrome; HTML chrome in `gui.rs` is legacy reference for tool surfaces; dual always-on WKWebViews are a known hit-testing failure mode.

---

## Executive summary

- **Browse is the product; tools are modes.** Thor’s default viewport is a lean developer browser. Postman / Locust / BrowserStack-lite / Security never compete for permanent chrome real estate.
- **One chrome strip, one job.** The 52px unified bar is traffic lights + tabs + nav + glass omnibox (+ a tiny mode/status chip). Tool launchers do not live as a labeled action row.
- **Keyboard is the primary IA.** `⌘K` = navigate (bookmarks / tab history / open tabs). `⌘⇧K` = tools & settings. Dedicated shortcut families open API, Locust, Device, and Security surfaces.
- **Three presentation layers only:** ephemeral palettes, docked workbench panels, and `Thor://…` dedicated routes. No third toolbar, no permanent multi-bar stack in browse mode.
- **Honest density for power users.** Dense, monospace-friendly, progressive disclosure; real metrics only; no fake HMAC / RNG theater; a11y and hit-testing are acceptance gates, not polish.

---

## 1. Information architecture

### Principle: Mode, not dashboard

Thor has **one primary mode** (Browse) and **four tool modes**. At most **one tool mode owns the secondary surface** at a time (except an optional slim “run pill” for a background Locust swarm).

| Mode | Job | Where it lives |
|------|-----|----------------|
| **Browse** | Navigate, tabs, page content | Unified chrome + content |
| **API** | Request / response / collections | Docked Workbench or `Thor://api` |
| **Load** | Locust swarm control + live metrics | Docked Load strip or `Thor://load` |
| **Device** | Viewport / UA / geo / profile session | Compact Device panel or `Thor://device` |
| **Security** | Shield, Tamper, Decoder | Progressive: toggles in palette → panels → `Thor://security` |

### Cross-cutting rules

- Tool modes are **discoverable from `⌘⇧K` and shortcuts**, not from a permanent 6–8 button action cluster.
- **Tab context is sacred:** history stays per-tab; tool state may be session-global (Locust) or tab-scoped (Device UA) — label which, always.
- Opening a tool mode must **not** bury the omnibox or traffic lights.
- Closing a tool mode returns to Browse with **zero leftover bars** (except optional Locust run pill).

---

## 2. First-viewport / chrome composition

The “hero” of Thor chrome is **brand-light presence + navigational clarity**, not marketing copy.

### Chrome anatomy (top → bottom)

1. **Primary strip (exactly 52px, always on)**  
   System traffic lights (inset) → tab strip → `+` → back/forward/reload → **glass omnibox** → optional profile/mode chip → overflow (`⋯`) only if needed.
2. **Secondary strip (0–1, conditional)**  
   Exactly one of: Tech stack readout *or* Locust run controls *or* Tamper quick-add *or* Device viewport bar. Never two at once in Browse.
3. **Workbench dock (optional, left/right/bottom)**  
   Full API / Security studio / Device lab editors. Content area shrinks; chrome strip unchanged.
4. **Content plane**  
   Chromium page or `Thor://` route. Never covered by a second always-on WebView chrome layer for hit-testing reasons on macOS.

### Composition rules

- Brand signal: subtle **Thor** mark or lightning glyph near tabs/omnibox — present, not a headline.
- Omnibox is the **visual and interaction anchor** of the strip (glass, focus ring, monospace URL).
- Nav controls are **flush** (no chunky bezels, no pill clusters).
- Empty chrome background may be drag-region; interactive controls must be `no-drag` / AppKit-hit-testable.

---

## MUST / SHOULD / COULD

### A. Architecture & chrome

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-001** | MUST | Exactly one primary chrome strip (52px unified titlebar). No HTML faux traffic lights when AppKit system lights are used. |
| **UI-002** | MUST | Traffic lights inset to vertically center in the 52px bar (target: ~`(14, 18)` logical). Nav content leading inset clears lights (~76px). |
| **UI-003** | MUST | Never enable `movable_by_window_background`. Never call window-drag on control `mousedown`. Prefer opaque chrome surfaces for reliable hit-testing. |
| **UI-004** | MUST | Do not stack dual always-on WKWebViews for chrome+content hit targets on macOS; native AppKit toolbar (or single opaque chrome surface) owns the strip. |
| **UI-005** | MUST | Primary strip contains only: traffic lights, tabs, nav, glass omnibox, at most one mode/status chip, and overflow. No permanent Tamper/Decoder/Locust/Shield/Tech labeled buttons. |
| **UI-006** | MUST | At most one secondary strip visible in Browse mode. Opening a second tool replaces or dismisses the first (with confirm if Locust is running). |
| **UI-007** | MUST | User-facing product name is always **Thor**. Settings/system routes display as `Thor://…`. |
| **UI-008** | SHOULD | Overflow menu (`⋯`) or `⌘⇧K` lists all tool entry points; chrome stays calm under 1280px width. |
| **UI-009** | SHOULD | Tab strip truncates gracefully; active tab title remains readable; close affordance appears on hover/focus. |
| **UI-010** | COULD | Subtle Thor wordmark fade-in on cold start (≤300ms), then collapse to glyph. |

### B. Keyboard model

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-020** | MUST | `⌘K` opens **Nav palette** only: bookmarks + **active tab** history + open tabs. No settings/security items. |
| **UI-021** | MUST | `⌘⇧K` opens **Tools palette** only: settings, DX, security, tool mode launches, IDE bridges. No bookmarks/history mix. |
| **UI-022** | MUST | `⌘L` focuses omnibox and selects URL. `⌘T` / `⌘W` new/close tab. `⌘R` reload. `⌘[` / `⌘]` back/forward. `Esc` closes topmost overlay/panel step. |
| **UI-023** | MUST | Shortcuts work even when content WebView or omnibox would otherwise swallow menu key equivalents (AppKit local monitor or equivalent). |
| **UI-024** | MUST | API workbench (when focused): `⌘Enter` Send; `⌘⇧F` focus URL; `⌘⇧H` Headers; `⌘⇧B` Body; `⌘⇧P` Params; `⌘⇧C` Collections sidebar. |
| **UI-025** | MUST | Locust: `⌘⇧L` toggle Load surface; `⌘⇧.` Start/Stop swarm (confirm Stop if running); when Load focused, `⌘Enter` Start. |
| **UI-026** | SHOULD | Device: `⌘⇧D` open Device panel; Security: `⌘⇧S` open Security surface; Decoder: `⌘⇧E` (encode/decode). |
| **UI-027** | SHOULD | Palette ↑↓ + Enter + typeahead; category tabs filter only within the active palette mode. |
| **UI-028** | COULD | Chord help sheet `⌘/` listing all Thor shortcuts (not page find — find-in-page is deferred). |

### C. Panels vs modals vs `Thor://` routes

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-040** | MUST | **Palettes (modal):** ephemeral, centered, dismissible, no persistent chrome height change. Used for `⌘K` / `⌘⇧K` only. |
| **UI-041** | MUST | **Panels (docked):** persistent until closed; resize handle; one primary dock at a time for deep editors (API, Security studio). |
| **UI-042** | MUST | **`Thor://` routes:** full content-plane pages for durable configuration and large canvases: e.g. `Thor://settings`, `Thor://api`, `Thor://api/collections`, `Thor://load`, `Thor://device`, `Thor://security`, `Thor://security/decoder`. |
| **UI-043** | MUST | Choosing a palette item that is a `Thor://` URL navigates content; choosing a tool action opens panel/strip without fake navigation. |
| **UI-044** | MUST | Tool bars (Locust/Tamper/Decoder/Stack) are **siblings** of chrome, never nested under a `display:none` parent that breaks layout/hit-testing when shown. |
| **UI-045** | SHOULD | Deep links: `Thor://api?method=GET&url=…` pre-fills request editor; `Thor://load?users=100` pre-fills Locust. |
| **UI-046** | SHOULD | Panel width/height remembered per tool mode for the session. |
| **UI-047** | COULD | Pop-out Workbench to a second window once single-compositor embed exists (deferred until CEF path). |

### D. Visual system

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-060** | MUST | Matte obsidian palette: void `#0b0c0–#0b0c10`, chrome `#121318`, modal `#161820`, text `#f1f5f9` / secondary `#94a3b8` / dim `#64748b`, hairline borders `rgba(255,255,255,0.08)`. |
| **UI-061** | MUST | Accent for interaction: cool blue `#3b82f6` (focus, active tab fav, primary CTA). Semantic: green success, red danger/stop, amber warn. **Purple is not a brand accent** (decoder may use a muted secondary, not purple-on-obsidian theme). |
| **UI-062** | MUST | Glass omnibox: vibrancy/translucent fill + 1px hairline (~10% white) + 6px radius; focus strengthens border, not glow bloom. |
| **UI-063** | MUST | Flush controls: transparent/inline bezels, 6px corners, icon tint ~slate; hover = subtle fill, not elevation cards. |
| **UI-064** | MUST | Typography: UI sans for chrome labels; **JetBrains Mono / SF Mono** for URLs, headers, payloads, metrics. Avoid Inter-as-identity if shipping web fonts; prefer system + one mono. |
| **UI-065** | MUST | Motion: ≤120–180ms ease-out for chrome; palette open; panel slide. No continuous glow/pulse except Locust **RUNNING** status (subtle). |
| **UI-066** | SHOULD | Density mode default = “power”: 11–12px chrome type, tight gaps (4–6px), compact inputs. |
| **UI-067** | COULD | Optional “Comfort” density (+2px type, +4px padding) in Settings. |

### E. API workbench (Postman-like)

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-080** | MUST | Workbench layout: Collections sidebar | Request editor (method, URL, Params/Headers/Body tabs) | Response viewer (status, time, size, body + headers). |
| **UI-081** | MUST | Method selector + URL field + Send are one horizontal composition (one job: fire request). |
| **UI-082** | MUST | Response viewer shows real status/latency/size from engine; empty/error states are honest. No placeholder “200 OK” theater. |
| **UI-083** | MUST | Body editor supports raw JSON/text; pretty-print toggle; copy response. |
| **UI-084** | MUST | Collections are first-class: create folder, save request, rename, delete; open from `Thor://api/collections`. |
| **UI-085** | SHOULD | “Import from current tab URL” and “Replay last intercepted request” when Tamper/CDP Fetch available. |
| **UI-086** | SHOULD | Environment vars (`{{baseUrl}}`) with a single active environment chip — not a second dashboard. |
| **UI-087** | SHOULD | History of sent requests in-session (not global browser history). |
| **UI-088** | COULD | Sync collections to filesystem under Thor data dir for git-friendly JSON. |

### F. Locust UX

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-100** | MUST | Locust does not occupy permanent chrome. Entry: `⌘⇧L` / `⌘⇧K` / `Thor://load`. |
| **UI-101** | MUST | Controls: Users, Ramp-up (users/s), Runtime, Profile (constant/ramp/spike/wave), Go, Stop — one compact strip or page section. |
| **UI-102** | MUST | Live metrics when running: RPS, failure %, P50, P95, total requests, status badge (`READY` / `RUNNING` / `STOPPING` / `DONE` / `ERROR`). Values from real Locust engine only. |
| **UI-103** | MUST | While browsing with swarm running, collapse to a **run pill** in primary strip or secondary mini-strip: status + RPS + Stop. Full metrics only when Load surface is open. |
| **UI-104** | MUST | Start/Stop are idempotent in UI; double-Go does not spawn duplicate swarms; Stop always reachable. |
| **UI-105** | SHOULD | Target URL defaults to active tab URL; editable before Go. |
| **UI-106** | SHOULD | Metrics update throttle ≤4 Hz in UI to protect main thread; engine may sample faster. |
| **UI-107** | COULD | Sparkline for RPS/P95 on `Thor://load` only (not in Browse pill). |

### G. Device / session (BrowserStack-lite)

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-120** | MUST | Device panel controls: viewport presets (mobile/tablet/desktop custom), device pixel ratio, User-Agent string, locale, timezone, geolocation lat/lon + accuracy, online/offline. |
| **UI-121** | MUST | Changes apply to **active tab session** and are labeled as such; DevHost vs StealthTarget profile remains a separate, explicit toggle. |
| **UI-122** | MUST | Viewport emulation does not invent a multi-device card grid on the first surface — one preview context, one controls column. |
| **UI-123** | MUST | Stealth vs DevHost: show concise active effects (tracker strip, GPC/DNT, IDE bridge paused, etc.) without `alert()` spam. |
| **UI-124** | SHOULD | Preset list curated (≤12); custom width/height inputs always available. |
| **UI-125** | SHOULD | “Reset session emulation” one-click restore. |
| **UI-126** | COULD | Save named device profiles locally. |

### H. Security tooling (progressive disclosure)

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-140** | MUST | Layer 0 — Shield toggles in `⌘⇧K` / `Thor://security`: tracker sanitizer, HTTPS upgrade, cookie/fingerprint policy, ad/telemetry block, local SSL CA status. Real state only. |
| **UI-141** | MUST | Layer 1 — Tamper panel: enable intercept, URL pattern, replace key/value, add rule, rule list. No purple “studio” banner required. |
| **UI-142** | MUST | Layer 2 — Decoder: paste JWT/Base64/Hex → auto-detect type → structured result. HMAC crack only if crypto is wired; otherwise honest **Unavailable** (no fake “verified”). |
| **UI-143** | MUST | Tech stack detector is opt-in secondary strip or Security subview — not always-on pills in Browse. |
| **UI-144** | SHOULD | Interceptor OFF/ON is the loudest control; rules are secondary. |
| **UI-145** | SHOULD | Decoder result uses monospace, collapsible claims/headers/payload sections. |
| **UI-146** | COULD | “Send decoded claims to API workbench as headers” bridge. |

### I. Accessibility & density

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-160** | MUST | All chrome controls keyboard-reachable; visible focus rings (hairline accent, not thick system default if customized). |
| **UI-161** | MUST | Icon-only buttons have accessible names (`aria-label` / AppKit accessibility label). |
| **UI-162** | MUST | Contrast: text/secondary on obsidian meets WCAG AA for UI text; disabled states still distinguishable. |
| **UI-163** | MUST | Prefer reduced-motion: skip non-essential transitions when OS Reduce Motion is on. |
| **UI-164** | MUST | Hit targets in chrome ≥ 28×28 logical px (flush look ≠ tiny hit area). |
| **UI-165** | SHOULD | Screen reader announcements for Locust status changes and request Send completion. |
| **UI-166** | SHOULD | High-density tables/lists for collections, rules, metrics — zebra optional, cards discouraged. |
| **UI-167** | COULD | Full keyboard grid navigation in collections tree. |

### J. Honesty & product integrity

| ID | Level | Requirement |
|----|-------|-------------|
| **UI-180** | MUST | No stub theater: no fake Locust RNG metrics, no fake HMAC verified, no pretend extension load/toggle, no 3D DOM sync theater. |
| **UI-181** | MUST | Extension list in Tools palette is metadata/info-only until MV3 host exists — badge **Info**, not Active. |
| **UI-182** | MUST | History privacy: tab-scoped; cleared on tab close / 24h GC; Tools palette can offer explicit wipe. |

---

## Acceptance criteria (per surface)

### Browse chrome

**Visual**
- Single 52px obsidian strip; glass omnibox dominant; flush nav; traffic lights aligned; no action-button soup.
- Secondary strip absent by default.

**Interaction**
- Omnibox focus/submit, tabs, nav, `⌘K`/`⌘⇧K`/`⌘L` work with content focused.
- Clicks never dead after drag-region mistakes; no dual-WebView hit-test regression.

### Nav palette (`⌘K`)

**Visual**
- Search + category tabs (All / Bookmarks / History / Open Tabs) + list + footer hints. Obsidian modal, not card grid.

**Interaction**
- Typeahead filters; Enter navigates/switches tab; Esc closes; only nav datasets.

### Tools palette (`⌘⇧K`)

**Visual**
- Same shell, tools categories; settings paths show `Thor://…`.

**Interaction**
- Launches panels/routes; toggles reflect real engine state; info-only items do not pretend to enable.

### API workbench

**Visual**
- Three-pane (or two-pane collapsed) density layout; monospace URL/body; no marketing hero.

**Interaction**
- Send → real response; save to collection; shortcuts work when panel focused; Esc closes panel without killing tab.

### Locust

**Visual**
- Idle: compact controls. Running + Browse: pill only. Full metrics only on Load surface.

**Interaction**
- Go/Stop correct; metrics update live; cannot leave Stop unreachable; no fake numbers.

### Device

**Visual**
- One controls column + optional single preview hint; no device card marketplace.

**Interaction**
- Preset applies viewport/UA; geo fields validate; reset works; profile toggle updates chrome chip without modal alerts.

### Security

**Visual**
- Shield list → Tamper → Decoder depth; progressive, not all bars open.

**Interaction**
- Toggles persist session; tamper rules add/list; decoder honest; crack unavailable messaging when not linked.

---

## Handoff

### For Developer

- Implement IA as **Browse + one tool owner**, not restore the legacy 8-button action row.
- Keep AppKit unified titlebar as source of truth for nav/omnibox; port Locust/Tamper/Decoder/Tech as **conditional strips/panels/routes**, not a second chrome WebView.
- Honor `⌘K` / `⌘⇧K` dataset split; wire shortcut families; inject palette payloads before open when needed.
- Preserve IPC shapes where possible (`locust_start/stop`, `set_interception_enabled`, `add_tamper_rule`, `decode_payload`, `toggle_profile`, etc.) behind new surfaces.
- Respect macOS gotchas: no movable background, traffic light inset, opaque hit-testing, sibling bars.
- Ship honest empty/unavailable states; wire `AppState::global()` engines only.

### For Tester

- **Hit-test suite:** every chrome control clickable with page loaded; omnibox selectable; no dead toolbar after rapid tab switches.
- **Shortcut suite:** `⌘K` excludes settings; `⌘⇧K` excludes bookmarks/history; `⌘L`/`⌘T`/`⌘W`/`⌘R`; API Send; Locust start/stop.
- **Mode exclusivity:** open Locust then Tamper → only one secondary strip; Locust running → pill remains + Stop works from Browse.
- **Honesty:** Locust metrics change only when swarm active; decoder crack shows unavailable if unimplemented; extensions cannot “enable.”
- **Visual regression:** 52px bar, glass omnibox, no purple theme, no dual titlebars, traffic light alignment.
- **A11y smoke:** keyboard-only chrome path; labels on icon buttons; Reduce Motion respected.
- **Privacy:** close tab → history gone from `⌘K`; GC path clears as specified.

### For Perf

- Palette open P95 target: keep existing cmd-palette bench intent (no jank; avoid large DOM re-render of full page).
- Locust UI updates capped (≤4 Hz); do not reflow entire chrome each tick — update metric nodes / pill only.
- Panels must not add a second composited always-on WebView over content for chrome.
- Tool mode open/close must not leak listeners or duplicate key monitors.
- Cold start: chrome attach + traffic light inset without blocking first paint longer than necessary; no web-font FOIT for Inter if avoidable.
- Memory: dismiss panels release large response bodies from DOM when closed (or virtualize); collections lists virtualize above ~200 items.

---

## Explicit anti-patterns

1. **Action-button soup** — permanent Tamper / Decoder / Tech / Locust / Shield / ⌘K / ⌘⇧K / DevTools labeled cluster in the primary strip.
2. **Card soup** — tool home screens as grids of elevated cards, badges, and promo tiles.
3. **Dashboard first viewport** — stats, schedules, multi-widget chrome on Browse load.
4. **Dual toolbars / dual titlebars** — system lights + HTML fake lights; AppKit bar + HTML 52px twin; two always-on WKWebViews fighting hits.
5. **Purple AI defaults** — purple-to-indigo gradients, glow accents, soft “AI studio” violet decoder themes as brand.
6. **Cream / terracotta / broadsheet** — warm paper, dense newspaper columns, hairline-print layouts (wrong product).
7. **Multi-bar stack** — Locust + interceptor + decoder + tech all `active` simultaneously eating content.
8. **Modal for deep work** — editing collections or reading large responses only inside the Cmd+K modal.
9. **Theater metrics / security** — RNG RPS, fake HMAC verified, pretend extension toggles, alert() profile essays.
10. **Hero overlays on chrome** — floating promo chips, stickers, or stat callouts on the omnibox.
11. **Inset “media card” device lab** — BrowserStack parody with many device frames as the first Device UI.
12. **Global history DB UI** — treating Thor like Chrome history; history is tab-scoped.
13. **Movable-by-background / drag-on-mousedown** — known click killers.
14. **Inter as brand identity + emoji-led IA** — emoji may appear sparingly as icons, but navigation must work without emoji literacy; prefer SVG/SF Symbols in native chrome.

---

## Priority cut line (opinionated)

**Ship order for UI correctness**
1. Calm primary chrome + hit-testing (UI-001–005, UI-003–004)  
2. Palette purity `⌘K` / `⌘⇧K` (UI-020–021)  
3. Locust progressive disclosure + run pill (UI-100–104)  
4. API workbench panel + `Thor://api` (UI-080–084)  
5. Security progressive layers (UI-140–142)  
6. Device panel (UI-120–123)  

Everything marked COULD waits until the MUST surfaces are calm, honest, and keyboard-complete.