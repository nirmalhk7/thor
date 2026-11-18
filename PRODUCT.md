# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Desktop application: Rust owns a native CEF Views browser shell; HTML is embedded browser content. This is not a mobile website.

## Users

Developers working with APIs and performing penetration testing against web applications, using the browser as their working context.

## Product Purpose

Thor's ultimate objective is an API development platform and a penetration-testing tool built into the browser. Browsing supplies the application context; API work and security investigation are central product workflows. This design pass covers the browser shell, new-tab surface, and functional Sites/Toolbox menus.

## Positioning

Each tab is an isolated experiment with ephemeral browsing history. Identity contexts retain site storage independently of a tab.

## Operating Context

API request editing, replay, response inspection, and saved requests belong alongside security investigation and evidence. Collections, environments, scenarios, findings, and explicitly saved results persist independently of browsing tabs. The shell must make the current identity and target understandable when those workflows are exposed.

## Capabilities and Constraints

- Embed prebuilt CEF in one browser window. Never download or build the full Chromium source tree.
- Preserve working navigation, tabs, identities, keyboard access, and existing tool engines.
- Closing a tab deletes its unsaved browsing history. Never invent measurements or claim unperformed operations succeeded.
- Keep controls lightweight and accessible. Preserve native omnibox and action hit-testing.
- Header: native macOS window controls above one navigation/address row; no tab strip or extra toolbar. Cmd+K opens Sites; Cmd+Shift+K opens Toolbox and Developer Control Center.
- No local deployment or installation. User approved a disposable /private/tmp app layout and isolated development run for native verification.

## Brand Commitments

The product name is Thor. The requested visual direction is lean and futuristic. The user selected a code-first workflow.

## Evidence on Hand

The CEF shell lives in `src/browser/cef/host/mod.rs`; browser session behavior lives in `src/browser/session.rs`. The existing shell uses native CEF controls. `src/workbench/` contains an API request editor and Rust commands for sending and saving requests. Security engines exist in `src/features/security/mod.rs` and `src/net/`, but their existence alone does not mean every tool is available from the CEF shell. No benchmark claims or demonstration metrics are authorized for the new-tab surface.

## Product Principles

- Keep web context, API work, and security investigation within one coherent workspace.
- Make active state and navigation legible.
- Use real actions and truthful state.
- Favor low latency and low weight.
