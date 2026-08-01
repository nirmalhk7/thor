# Isolation Level → Chromium primitives

Thor’s three-level hierarchy maps onto Chromium APIs (not remote CDP-only theater).

## Level 1 — Profile

- **Thor:** settings, bookmarks, extensions, root CAs, IDE bridge. No global history DB.
- **Chromium:** `ProfileImpl` + dedicated user-data-dir
  - Default: `~/Library/Application Support/Thor/ChromiumProfile`
  - Bookmarks / prefs stay profile-scoped.
  - Thor's Chromium patch defaults `SavingBrowserHistoryDisabled` to true.
  - This is a default, not immutable policy; existing user or enterprise prefs can override it.

## Level 2 — Identity Context

- **Thor:** isolated cookies, localStorage, IndexedDB, ServiceWorkers, auth tokens (`Admin`, `Standard User`, `Anonymous`, …).
- **Chromium:**
  - Rust creates an off-the-record `Target.createBrowserContext` with `disposeOnDetach=true`.
  - `Target.createTarget.browserContextId` binds tabs to that context.
  - `NewIdentity` creates an explicitly managed context; `DestroyIdentity` disposes it and its targets.
  - `CreateDisposableTab` atomically allocates a Thor-owned context and its first tab, returning both IDs; the context is disposed when its final tab closes.
  - Compatibility `CreateTab` calls with a new nonzero identity auto-create a context that is disposed after its final tab closes.
  - Chromium's native **New Identity** action creates a unique, non-primary, memory-only OTR `Profile` and opens one normal browser window without session restore.
  - Closing that identity's final tab closes its final browser; existing `BrowserManagerService` and `ProfileDestroyer` then destroy the OTR profile after renderer teardown. Failed window creation schedules the same cleanup immediately.
- BrowserContexts isolate cookies, localStorage, IndexedDB, ServiceWorkers, cache, auth state, and site permissions in Chromium itself.
- Native identity profiles never survive process restart and cannot use disk while off the record, so no startup orphan can persist. Chromium intentionally disables extensions in non-primary OTR profiles; native privacy controls still inherit through profile prefs and enterprise policy.
- Tabs created directly in Chromium's normal chrome remain in the default profile context unless opened from **New Identity**. Hardened status must report default-profile tabs as unisolated; CDP cannot retroactively move a WebContents between contexts.

## Level 3 — Tab

- **Thor:** tab-scoped history list, scroll, trace buffer, lifecycle.
- **Chromium:** `WebContents` + `TabStripModel` entry; session restore metadata only for that tab id.
- On close / 24h GC: remove from strip, wipe tab-scoped history side-car (Rust evidence store may retain promoted artifacts only).

## Lifecycle

| Stage | Thor | Chromium hook (preferred) |
|-------|------|---------------------------|
| Active | Foreground / recent | Normal WebContents |
| Frozen | 15m–1h idle | Page lifecycle frozen / WasHidden + freeze |
| Discarded | 1h–24h | Transitional CDP fallback closes target, retains Thor URL/scroll metadata, then recreates target in the same BrowserContext on demand |
| Expired | ≥24h | Close tab + wipe L3 state |

Thor's sidecar can freeze through CDP and can preserve Level 2 identity when recreating a discarded target. CDP exposes no native `TabStripModel` discard operation, so this does not yet preserve the original WebContents/tab-strip entry. A future in-process Chromium hook must replace the transitional discard fallback.

## Privacy enforcement readback

- Chromium 151.0.7922.47 already registers Topics, Protected Audience/FLEDGE, and ad-measurement prefs as disabled. Thor's patch additionally disables Related Website Sets.
- Thor's ordered native GPC patch registers a profile pref enabled by default, exposes the managed `GlobalPrivacyControlEnabled` policy, sends `Sec-GPC: 1` on frame, worker, service-worker, and redirect request paths, and exposes `navigator.globalPrivacyControl` in Window and WorkerNavigator.
- Rust Labs interception can inject GPC/DNT headers as an explicit target-profile fallback until live native parity is verified. The bundled extension does not override native DNT/GPC.
- `PrivacyPolicy` records requested DoH, GPC, Privacy Sandbox, WebRTC, JIT, dangerous-API, download, WebGL, WebGPU, and media controls.
- `PrivacyStatus` reports native GPC only after exact-build verification. Target-profile fallback injection is `BestEffort`; an unverified native-only path is `Unavailable`. Other controls without reliable generic-CDP readback follow the same proof rule.
