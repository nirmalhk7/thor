# Thor

> **A high-performance, ephemeral developer browser where every tab is an isolated experiment and every interaction can become a test, trace, benchmark, finding, or reproducible artifact.**

**Thor** is a native Rust browser shell with the Chromium Embedded Framework (CEF) embedded for web content. Thor bundles the pinned prebuilt CEF distribution; it does not fetch or build the full Chromium source tree and does not use system Chrome as its product browser.

API development, security testing, and the Engineer Feature Suite open inside Thor from the Developer control in the browser toolbar or **Cmd+Shift+K**. The workbenches are served by Thor's in-process CEF resource handler and call Rust engines directly; they do not start a separate web server.

---

## Architecture & Ephemeral Lifecycles

```
┌────────────────────────── Thor.app ──────────────────────────┐
│ Native window: macOS controls, tabs, address bar, actions     │
│ ┌──────────────────────────────────────────────────────────┐ │
│ │ Embedded CEF browser                                     │ │
│ │ Web pages · API workspace · Security workspace · DevCC   │ │
│ └──────────────────────────────┬───────────────────────────┘ │
│ Rust shell, IPC, API engine, FeatureEngine, evidence, tools  │
└─────────────────────────────────────────────────────────────┘
                     │
                     └── CEF renderer/helper subprocesses
```

Thor stores browser data under its application data directory and configures CEF with the Keychain service **Thor Safe Storage** and account **Thor**. Existing cookies encrypted with the former shared Chromium Safe Storage key require a fresh sign-in. CEF request contexts are the boundary for identity storage; tab lifecycle actions must report success only after the corresponding CEF operation completes.

---

## Product Features & 5 Specialist Lenses

Thor organizes developer capabilities under **4 Top-Level Areas** (`Browse`, `Inspect`, `Compare`, `Run`) and **5 Specialist Lenses**:

1. **Full-Stack Lens**: Correlates UI clicks to backend requests and deep-links stack traces directly into VS Code (`vscode://file/...`), Cursor (`cursor://file/...`), or Zed (`zed://file/...`). Includes Tech Stack Detector and CSS Layout Overlay (`css_overlay.rs`).
2. **Security Lens**: Identity Matrix testing — compares endpoint responses across `Admin`, `Owner`, `Other User`, and `Anonymous` to detect IDORs using structural JSON body key hashing (`compute_json_structure_hash`). Features Traffic Tamper engine with response mocking (`mock_status`, `mock_body`, `delay_ms`), secret header redaction (`SEC-027`), Security Shield anti-tracking, SSL CA generator, and Decoder Studio (JWT/Base64/Hex/JSON).
3. **Performance Lens**: Live Core Web Vitals telemetry (`LCP`, `CLS`, `INP`, `TBT`, `TTFB`, `FCP`), lock-free atomic histogram metrics ($O(1)$ zero-lock latency tracking up to 10,000+ RPS), rolling profiler, and CDP timeline diffs.
4. **AI / LLM Telemetry Studio (`src/dx/llm_studio/mod.rs` & `src/dx/llm_clean_context.rs`)**: Token counter, 2026 model pricing table with prompt caching (`claude-3.7-sonnet`, `deepseek-r1`, `gpt-4o`, `gemini-2.0-flash`), multi-provider SSE parser, local Ollama (`localhost:11434`) / vLLM (`localhost:8000`) auto-detection, and `Cmd+Shift+C` Copy Page as LLM Clean Context.
5. **Web3 / Blockchain Inspector (`src/dx/web3_inspector/mod.rs`)**: EVM & Solana JSON-RPC call logger, 4-byte ABI selector decoder (`0xa9059cbb`), and pure-Rust EIP-55 address checksum validator.
6. **Thor Load Engine (`src/load/`)**: Canonical `ThorScenario` specification powered by a **Browser Runner** (for Web Vitals/DOM rendering truth) and a **Protocol Runner** (multi-threaded Rust HTTP/WS traffic for high scale).
7. **Universal 'Promote to Test' Polyglot Exporter (`src/dx/test_generator/mod.rs`)**: Converts recorded network traces into Rust `reqwest`, Go `net/http`, Python `httpx`, Playwright TypeScript end-to-end tests, k6 load scripts, cURL, ThorScenario, EvalHarness, Finding bundles, and composed CI manifests (hermetic fixtures + oracles + a11y specs + quarantine filters).

---

## Engineer Feature Suite (144 features · 6 lenses)

Accessible from **Cmd+⇧K → Developer Control Center**. The chrome calls `feature_list` / `feature_invoke` / `lens_payload`; Rust routes through `FeatureOrchestrator` into domain engines under `src/features/`. Optional **`from_active_tab`** evaluates CDP probe scripts on the live `TabSession`.

| Domain | Count | Highlights |
|---|---:|---|
| **Frontend** | 23 | Hydration diff, CSS coverage, focus order, DOM snapshot, image delivery, layout thrash |
| **Backend** | 24 | HMAC webhook vault, live stub / SSE labs, rate-limit & pagination HTTP probes, OpenAPI diff |
| **Security** | 24 | CORS/CSP forensics, consent-gated fixation/SSRF, GraphQL authz, race analyzer, SRI |
| **Performance** | 25 | LCP element forensics, long-task radar, freeze–resume bench, soft-nav vitals, H3 / conn-pool |
| **QA** | 24 | Flake triager, quarantine policy, hermetic Playwright export, CI bundle compose, oracles |
| **AI** | 24 | Tool-call forensics, cache-hit $, JSON Schema guard, MCP traffic lens, streaming SLO gates |

Registry source of truth: `FeatureId::ALL` in `src/features/mod.rs` (`AppState::global().features()`). See [docs/FEATURES.md](docs/FEATURES.md) for the full matrix.

---

## Standard Browser Parity ("Normal Browser Features")

- **Bookmark Saving (`Cmd+D` & Star Button `★`)**: Persistent bookmarking to `~/Library/Application Support/Thor/bookmarks.json` with active golden star state indicator and `Cmd+K` palette integration.
- **Find in Page (`Cmd+F`)**: Search the active CEF page with match count and next/previous navigation.
- **Page Zoom & Scale (`Cmd++`, `Cmd+-`, `Cmd+0`)**: Adjust the active CEF browser’s page zoom.
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
| `Cmd+K` | Sites Menu (Open tabs + Tab-scoped history + Saved Bookmarks) |
| `Cmd+Shift+K` | Toolbox Menu → **Developer Control Center** (144 engineer features · 6 lenses) + specialist tools |
| `Cmd+L` | Focus Omnibox |
| `Cmd+Shift+C` | Copy Page as LLM Clean Context (Markdown/DOM) |
| `Cmd+/` | Keyboard Shortcuts Cheat Sheet |
| `Cmd+R` / `Cmd+[` / `Cmd+]` | Reload / Back / Forward |

---

## Requirements & Build

- Rust stable (edition 2021)
- macOS (primary target; Linux/Windows partially supported)
- The pinned prebuilt CEF distribution is resolved by the `cef` Rust package. No full Chromium checkout is needed.

```bash
# Build Thor, Labs, and the CEF app packager
cargo build --locked --release --features cef-bundle --bin Thor --bin thor-labs --bin thor-package

# Stage a macOS app bundle, then launch it
cargo run --locked --release --features cef-bundle --bin thor-package -- dist/cef
open dist/cef/Thor.app

# Quality & Test Verification
cargo check
cargo test --lib
cargo test --lib features::
cargo test --test integration_tests
cargo test --test features_tests
cargo test --test features_v2_tests
cargo test --test features_wave2_tests
cargo test --test features_wave4_tests
cargo test --test features_wave5_tests
```

Repository conventions are checked with `make check-structure` and in CI. Current architecture: [ARCHITECTURE](docs/ARCHITECTURE.md) · [FEATURES](docs/FEATURES.md) · [TESTING_GUIDE](docs/TESTING_GUIDE.md) · [AGENTS.md](AGENTS.md) (coding agents).
