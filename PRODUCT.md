# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The design tooling uses `web` for the embedded HTML surfaces. Thor itself is permanently macOS-only: a native Rust and CEF Views browser, not a hosted website or a product for Linux, Windows, or mobile.

## Users

Developers who want everyday browsing and their work tools in one browser. Thor covers seven disciplines: Frontend, Backend, QA, Security, Performance, AI, and Web3.

## Product Purpose

Make a useful, tabless daily-driver browser for developers. A workflow starts with a page or supplied evidence, does a real job, and produces a result the developer can inspect and, where supported, save or export. The product scope has seven disciplines; the current 144-entry FeatureEngine registry covers six, and the Web3 decoder is separate. Do not treat an engine entry as a shipped browser workflow.

## Positioning

Pages are temporary working context, not a browsing-history archive. Identity contexts isolate site storage and preserve sign-in state independently of page history. Explicitly saved project work has its own lifetime.

## Operating Context

- **Expiry:** A page can expire after 30 minutes in the background. Foregrounding it resets the clock; detectable drafts, calls, and playback defer expiry. Routine polling is not a reason to keep a page alive.
- **Pinning ([#7](https://github.com/nirmalhk7/thor/issues/7)):** Users pin or unpin live pages from Sites. Pins never expire automatically and appear first, followed by other open pages in recent-use order. Unpinning a background page gives it a fresh 30 minutes.
- **Recovery ([#9](https://github.com/nirmalhk7/thor/issues/9)):** Restore only surviving page URLs in the original identity, with pin state and expiry timing intact, after normal quit or crash. Time while Thor is closed counts toward existing background deadlines. Do not restore closed or expired pages, page history, or unsaved captures.
- Expiry ships separately from pinning and recovery. Projects, saved requests, and explicitly saved tool results are independent of browsing pages.

## Capabilities and Constraints

- Bundle the pinned stock CEF distribution, with CEF’s native process and sandbox boundaries. Do not fork or build Chromium, or substitute system Chrome for the product.
- Keep the native macOS title bar and a single address/navigation row. No visible tab strip or extra toolbar. Cmd+K opens Sites; Cmd+Shift+K opens Toolbox and the Developer Control Center.
- Preserve working navigation, identity isolation, keyboard access, and omnibox and toolbar hit-testing. Any feature that uses browser context must keep the originating page and identity clear.
- Report only real actions and results. Do not claim Thor outperforms Chrome until a fair same-Mac benchmark is published. Thor does not add product telemetry; distinguish stock CEF and site traffic and verify the boundary in [#25](https://github.com/nirmalhk7/thor/issues/25).

## Brand Commitments

The product name is Thor. Keep the lean Graphite Workspace design and native controls; prefer a focused, code-first implementation over adding more chrome.

## Evidence on Hand

`src/browser/cef/host/mod.rs` owns the CEF shell; `src/browser/session.rs` owns page and identity state. `src/workbench/` contains the embedded workspaces and Rust command routing. The current native toolbox launches standalone workbench actions; browser-context workflows must be verified separately. [DESIGN.md](DESIGN.md) records the visual baseline, not runtime evidence.

## Product Principles

- Make the daily browsing path reliable before expanding the tool catalog.
- Show a clear source, action, and result. Keep only the evidence that was explicitly saved.
- Preserve native navigation, readable states, and steady keyboard focus.
