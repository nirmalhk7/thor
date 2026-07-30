# Isolation Level → Chromium primitives

Thor’s three-level hierarchy maps onto Chromium APIs (not remote CDP-only theater).

## Level 1 — Profile

- **Thor:** settings, bookmarks, extensions, root CAs, IDE bridge. No global history DB.
- **Chromium:** `ProfileImpl` + dedicated user-data-dir
  - Default: `~/Library/Application Support/Thor/ChromiumProfile`
  - Bookmarks / prefs stay profile-scoped; disable persistent global history via prefs / enterprise policy hooks where needed.

## Level 2 — Identity Context

- **Thor:** isolated cookies, localStorage, IndexedDB, ServiceWorkers, auth tokens (`Admin`, `Standard User`, `Anonymous`, …).
- **Chromium:**
  - Prefer separate `StoragePartition` per identity, or
  - Separate OTR / non-persistent profile for `Anonymous` / disposable contexts.
- Disposable Isolated Tab: create partition → bind WebContents → destroy partition on tab close.

## Level 3 — Tab

- **Thor:** tab-scoped history list, scroll, trace buffer, lifecycle.
- **Chromium:** `WebContents` + `TabStripModel` entry; session restore metadata only for that tab id.
- On close / 24h GC: remove from strip, wipe tab-scoped history side-car (Rust evidence store may retain promoted artifacts only).

## Lifecycle

| Stage | Thor | Chromium hook (preferred) |
|-------|------|---------------------------|
| Active | Foreground / recent | Normal WebContents |
| Frozen | 15m–1h idle | Page lifecycle frozen / WasHidden + freeze |
| Discarded | 1h–24h | Tab discard / unload renderer, keep URL+scroll |
| Expired | ≥24h | Close tab + wipe L3 state |

CDP `Page.stopLoading` / target detach remain **legacy dual-window** fallbacks only.
