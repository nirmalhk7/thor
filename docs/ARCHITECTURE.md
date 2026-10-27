# Thor Architecture

## Product runtime

Thor’s product is a native Rust shell with CEF embedded for page rendering. The executable dispatches CEF helper processes before initializing the main UI, then loads the bundled CEF framework and creates CEF browser views inside Thor’s window. Thor ships the pinned prebuilt CEF distribution; it does not fetch or build Chromium source.

```text
Thor.app
├── Native window and browser toolbar
├── CEF browser views and renderer/helper subprocesses
├── BrowserSession: tabs and identity metadata
├── CEF RequestContexts: identity storage boundaries
└── Rust engines: API execution, FeatureEngine, evidence, security, load
```

`src/main.rs` enters `src/browser/cef`. `src/browser/cef/mod.rs` dispatches subprocesses, loads the CEF framework, sets the Thor cache root, and configures the dedicated macOS Keychain service `Thor Safe Storage` with account `Thor`. The legacy Chromium and Wry modules are migration code; they are not Thor’s product launch path.

## Window and browser controls

`src/browser/cef/host.rs` owns the native Thor window, toolbar, tab views, CEF request contexts, and browser session. The toolbar provides navigation, reload, workspace, address entry, and the Developer control. Cmd+K opens Sites; Cmd+Shift+K opens the Toolbox. Both routes resolve to native Thor actions.

Each identity receives a CEF `RequestContext`. Persistent identities use a cache directory under `Thor/CEF/Identities/<identity-id>`; disposable identities use an empty cache path and do not persist session cookies. Tabs are created with their identity’s context.

## Embedded developer workspaces

The API workspace, Security workspace, home surface, and Developer Control Center are served as `thor://workbench/...` resources by `src/browser/cef/host/workspace.rs`. The resource handler serves compiled-in assets; the browser does not call a separate localhost workbench server.

Workspace pages communicate with the Rust `Workbench` through CEF message routing. Rust validates the trusted scheme, host, main frame, command type, persistence flag, and request size before dispatch. API execution uses the shared `HttpService`; security and feature requests route to existing Rust engines through `AppState::global()`. The pages use a restrictive CSP and do not enable general network connections.

## Data and trust boundaries

- Browser profile data lives under `thor_data_dir()/CEF`.
- Persistent identity storage is separated by CEF request context.
- Disposable identity contexts are configured without a disk cache path and with session-cookie persistence disabled.
- Workbench commands are accepted only from trusted top-level `thor://workbench` documents and only for the page’s allowed command set.
- Privileged testing operations remain subject to Rust consent checks; UI visibility alone does not grant permission.

## Build and verification

The CEF crate resolves a pinned prebuilt CEF distribution. No Chromium source checkout or depot_tools are required. Legacy scripts named `fetch_chromium.sh`, `build_chromium.sh`, `run_thor_chromium.sh`, and `bootstrap_chromium_snapshot.sh` are disabled.

```bash
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --lib
cargo test --locked --test workbench_tests
cargo test --locked --test integration_tests
```
