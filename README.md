# Thor

**Thor** is a developer-focused Chromium browser built in Rust. It aims to feel lighter and faster than stock Chrome/Firefox/Opera by staying lean: one Chromium process, aggressive tab freeze, tab-scoped history only, 24-hour idle tab GC, and no Google sync/extension host by default.

Built for fullstack engineers and security specialists — IDE jump, traffic tamper, Locust load testing, JWT decoder, tech-stack detect, DevHost / Stealth profiles.

## Requirements

- Rust stable (edition 2021)
- A Chromium/Chrome binary on the machine (or set `THOR_CHROMIUM_PATH`)
- macOS is the primary target; Linux/Windows are partially supported

## Run

```bash
cargo run --bin Thor
```

Dev auto-reload (if `cargo-watch` is installed):

```bash
./scripts/run_dev.sh
```

### Environment

| Variable | Purpose |
|----------|---------|
| `THOR_CHROMIUM_PATH` | Absolute path to Chrome/Chromium executable |
| `THOR_DATA_DIR` | Override Thor’s data directory |

Default data dir:

- macOS: `~/Library/Application Support/Thor`
- Windows: `%APPDATA%/Thor`
- Linux: `~/.local/share/Thor` (or `$XDG_DATA_HOME/Thor`)

## Product model

| Shortcut | What it does |
|----------|----------------|
| `+` / ⌘T | New tab |
| ⌘W | Close tab |
| ⌘K | Bookmarks + **this tab’s** history + open tabs |
| ⌘⇧K | Settings / DX / security features only |
| ⌘L | Focus omnibox |
| ⌘R / ⌘[ / ⌘] | Reload / back / forward |

**History:** per-tab only. Cleared when the tab closes or is GC’d.  
**Idle GC:** tabs unused ≥ 24 hours are closed and cleared.  
**Brand:** always **Thor** in user-facing UI (crate name remains `thor`).

## Architecture (v1)

Thor currently uses **two windows**:

1. **Control chrome** — Rust + `tao`/`wry` toolbar (omnibox, tabs, Cmd+K, tool panels)
2. **Content** — one headful Chromium process driven via CDP (`chromiumoxide`), many tabs as targets

Single-window CEF embed is planned later. Performance levers today: small prewarm pool, freeze inactive targets, no global history DB, isolated profile under `Thor/ChromiumProfile`.

## Features (status)

| Area | Status |
|------|--------|
| Chromium tabs via CDP | Working |
| Cmd+K / Cmd+⇧K | Working |
| Tab freeze + 24h GC | Working |
| Tracker URL strip / Security shield | Working (navigate path) |
| Locust HTTP swarm | Real HTTP (Tokio) |
| Traffic tamper engine | Wired; CDP Fetch intercept still evolving |
| JWT / Base64 decoder | Working |
| Tech stack detector | CDP HTML + heuristics |
| Local SSL CA (`rcgen`) | Generates certs under Thor data dir |
| Find-in-page, zoom, downloads | Not done yet |
| Single-window embed | Not done yet |

## Develop

```bash
cargo check
cargo test --lib
cargo test --test integration_tests
cargo bench   # optional SLA benches
```

## License

Not specified yet — treat as private/unpublished unless a license file is added.
