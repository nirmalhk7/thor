# Thor

> **A high-performance, ephemeral developer browser where every tab is an isolated experiment and every interaction can become a test, trace, benchmark, finding, or reproducible artifact.**

**Thor** is a developer-focused browser built in Rust and powered by a dual-window architecture (**AppKit native UI shell + headful Chromium CDP engine**). It delivers 100% Chromium web compatibility while operating significantly faster and lighter than Google Chrome through native UI rendering (`mimalloc` allocated), zero global history overhead, prewarmed target acquisition, aggressive tab memory offloading, and automated 24-hour idle tab garbage collection.

---

## Architecture & Ephemeral Lifecycles

```
+-------------------------------------------------------------------------------------------------------------------+
|                                            Thor Architecture & Engine                             |
+-------------------------------------------------------------------------------------------------------------------+
| POSH APPKIT / TAO NATIVE UI SHELL (Rust + mimalloc + 120 FPS GPU Micro-Animations)                               |
| - Near-zero UI overhead (<150ms cold start vs ~1200ms Chrome)                                                     |
| - 4 Top-Level Areas: Browse, Inspect, Compare, Run                                                                |
| - Unified Command Palette (Cmd+K: Tabs/Bookmarks/History | Cmd+Shift+K: Developer Control Center)                 |
| - Universal 'Promote to Test' Exporter (Playwright TS / k6 JS / cURL)                                             |
+---------------------------------------------------------+---------------------------------------------------------+
                                                          | Atomic Ring-Buffer IPC (<1ms)
                                                          v
+-------------------------------------------------------------------------------------------------------------------+
| HEADFUL CHROMIUM CDP CONTENT PROCESS (chromiumoxide)                                                              |
| - 100% Chromium Web Compatibility (V8 JS, Blink, WebGPU, WASM, WebSockets, Service Workers)                     |
| - Prewarmed CDP Target Pool (<5ms tab acquisition)                                                                |
| - Ephemeral 4-Stage Tab Lifecycles (Active -> Frozen -> Discarded -> Expired)                                     |
| - 3-Level Isolation Hierarchy (Profile -> Identity Context -> Tab)                                                |
+-------------------------------------------------------------------------------------------------------------------+
```

### 1. Three-Level Isolation Hierarchy
- **Level 1: Thor Profile**: Durable configuration (bookmarks, extensions, trusted certificates, settings, IDE integration). No permanent global history DB.
- **Level 2: Identity Context**: Owns cookies, `localStorage`, `IndexedDB`, `ServiceWorkers`, cache partition, and auth tokens. Runs `Admin`, `Standard User`, `Staging User`, `Wallet A`, and `Anonymous` sessions side-by-side.
  - **Disposable Isolated Tab**: Option to spawn a tab with its own ephemeral Identity Context, completely purged when the tab is closed.
- **Level 3: Tab**: Owns navigation history list, scroll state, trace buffer, and lifecycle deadlines.

### 2. Four-Stage Ephemeral Tab Lifecycle
1. **Active**: Live CDP target session in foreground / active use.
2. **Frozen**: Soft-suspended after 15m idle via CDP page lifecycle (`CdpPageFreezer`), pausing JS timers without releasing memory.
3. **Discarded**: After 1 hour of inactivity or memory pressure, the CDP target is detached and memory released, preserving URL, scroll position, and tab-scoped history for seamless instant reload on click.
4. **Expired**: Inactive tabs remaining dormant for $\ge$24 hours are automatically closed, purging all state and wiping tab-scoped history clean.

---

## Product Features & 5 Specialist Lenses

Thor organizes developer capabilities under **4 Top-Level Areas** (`Browse`, `Inspect`, `Compare`, `Run`) and **5 Specialist Lenses**:

1. **Full-Stack Lens**: Correlates UI clicks to backend requests and deep-links stack traces directly into VS Code (`vscode://file/...`), Cursor (`cursor://file/...`), or Zed (`zed://file/...`). Includes Tech Stack Detector.
2. **Security Lens**: Identity Matrix testing — compares endpoint responses across `Admin`, `Owner`, `Other User`, and `Anonymous` to detect IDORs instantly. Features Traffic Tamper engine, Security Shield anti-tracking, SSL CA generator, and Decoder Studio (JWT/Base64/Hex/JSON).
3. **Performance Lens**: Live Core Web Vitals telemetry (`LCP`, `CLS`, `INP`, `TTFB`, `FCP`), rolling profiler, and CDP timeline diffs.
4. **AI / LLM Telemetry Studio (`src/dx/llm_studio.rs`)**: Token counter, API cost estimator (GPT-4, Claude 3.5, Gemini 1.5, Llama 3), and `SseStreamParser` tracking TTFT (Time to First Token) & tokens/sec metrics.
5. **Web3 / Blockchain Inspector (`src/dx/web3_inspector.rs`)**: EVM & Solana JSON-RPC call logger, 4-byte ABI selector decoder (`0xa9059cbb`), and pure-Rust EIP-55 address checksum validator.
6. **Thor Load Engine (`src/load/`)**: Canonical `ThorScenario` specification powered by a **Browser Runner** (for Web Vitals/DOM rendering truth) and a **Protocol Runner** (multi-threaded Rust HTTP/WS traffic for high scale).
7. **Universal 'Promote to Test' Exporter (`src/dx/test_generator.rs`)**: Converts recorded network traces and DOM steps into Playwright TypeScript end-to-end tests, k6 load scripts, and cURL commands.

---

## Standard Browser Parity ("Normal Browser Features")

- **Bookmark Saving (`Cmd+D` & Star Button `★`)**: Persistent bookmarking to `~/Library/Application Support/Thor/bookmarks.json` with active golden star state indicator and `Cmd+K` palette integration.
- **Find in Page (`Cmd+F`)**: Floating glass search bar overlay with match count (`X of Y`), next/prev navigation (`Enter` / `Shift+Enter`), and Chromium native `WebContents::Find` wrapper.
- **Page Zoom & Scale (`Cmd++`, `Cmd+-`, `Cmd+0`)**: Chromium `HostZoomMap` wrapper for per-origin page zoom (100%, 110%, 125%) with status percentage display.
- **Downloads Manager**: Browser-owned `DownloadManager` bridge tracking active downloads across tab expiry.
- **Favicon Resolution**: Cached Favicon service resolving icons with fallback to manifest -> DOM `<link>` -> `/favicon.ico` -> monogram.
- **CORS Management & Cost Tracking**: 3-mode CORS switcher (`Strict`, `Permissive`, `Disabled`) and two-tier network instrumentation cost badge (`Lightweight` ~0.1ms vs `Deep` ~2.5ms).

---

## Shortcuts & Navigation

| Shortcut | Action |
|---|---|
| `Cmd+T` / `+` | New tab |
| `Cmd+W` | Close tab |
| `Cmd+D` | Toggle bookmark (`★`/`☆`) |
| `Cmd+F` | Find in page floating glass search bar |
| `Cmd++` / `Cmd+-` / `Cmd+0` | Zoom in / Zoom out / Reset zoom |
| `Cmd+K` | Open tabs + Tab-scoped history + Saved Bookmarks |
| `Cmd+Shift+K` | Developer Control Center & Specialist Lenses |
| `Cmd+L` | Focus Omnibox |
| `Cmd+R` / `Cmd+[` / `Cmd+]` | Reload / Back / Forward |

---

## Requirements & Build

- Rust stable (edition 2021)
- Google Chrome / Chromium installed (or set `THOR_CHROMIUM_PATH`)
- macOS (primary target; Linux/Windows partially supported)

```bash
# Build & Run
cargo run --bin Thor

# Quality & Test Verification
cargo check
cargo test --lib
cargo test --test integration_tests
```
