# Thor Security Requirements

**Document type:** Security requirements (not implementation)  
**Audience:** Developer / Tester / Perf agents  
**Scope grounding:** `src/net` (tamper, ssl_ca, security_shield, locust), `src/dx` (decoder, wasm_plugin), `src/engine/profiles.rs`, chrome↔Rust IPC in `shell/`  
**Status:** Binding for v1 product behavior; some surfaces (WASM/MV3, full CDP Fetch MITM) are partially deferred but still constrained here.

---

## Executive summary

- Thor **protects the developer’s machine and local identity**, while **deliberately enabling authorized traffic inspection/tamper and load testing** against targets the user owns or has explicit permission to test.
- **DevHost** maximizes DX (IDE bridge, full WebGL, local SSL); **StealthTarget** maximizes privacy/anti-tracking (strip trackers, GPC/DNT, fingerprint noise, IDE bridge off). Profiles must not silently mix storage/identity.
- **Tamper, Locust, and Dev CA** are high-risk tools: they require **scope consent, safe defaults (localhost/private), validation, confirmations, and auditability** — never silent or drive-by MitM of arbitrary hosts.
- **Decoder/JWT is display-and-inspect only**; no cracking, stuffing, or mass credential abuse. HMAC crack remains unavailable/honest, not fake.
- **Perf must not bypass security gates** (consent, rate caps, IPC allowlists, secret redaction, CA key permissions). Speed wins only after those invariants hold.

---

## 1. Threat model

### Assets Thor protects

| Asset | Why it matters |
|---|---|
| OS trust store / keychain | Dev CA install can enable system-wide TLS interception |
| Local secrets (API keys, collection vars, CA private keys) | Leak → account takeover / MitM capability |
| Chromium profile cookies / storage | Cross-site identity bleed between DevHost and Stealth |
| Chrome IPC surface (wry webview → Rust) | Untrusted JS must not escalate to arbitrary process actions |
| User’s authorized target systems | Tools can DoS or corrupt if misconfigured |
| Audit trail of tamper/load actions | Accountability for dual-use tooling |

### Adversaries / misuse cases

1. **Malicious or compromised page** in Chromium content process trying to abuse chrome IPC or local APIs.
2. **User error / over-scope** — load test or MitM pointed at third-party production without authorization.
3. **Local malware / shared machine** reading Thor data dir (`certs/`, collections, history buffers).
4. **Supply-chain / plugin** (future WASM/MV3) exfiltrating tokens or enabling stealth MitM.
5. **Social engineering** — Thor marketed as “security lab” used as a botnet / credential-stuffing kit.

### What Thor deliberately enables (authorized testing only)

- Local/dev TLS with **Thor Dev CA** for `localhost` / declared dev hosts.
- Request/response **header (and later body) tamper** via Fetch interception for **scoped** URLs.
- **Locust-like** concurrent HTTP load against **consented** targets.
- JWT/Base64/Hex/JSON **decode & inspect**.
- Tracker stripping, GPC/DNT, fingerprint noise under Stealth.
- IDE bridge under DevHost only.

### What Thor does **not** claim to protect

- The remote application’s server-side security (Thor is a client tool).
- Users who intentionally ignore consent dialogs (product still must gate and log).
- Full anonymity against sophisticated fingerprinting (Stealth is best-effort hardening, not Tor).

---

## 2. Hard boundaries (consent & scope)

### MUST

| ID | Requirement |
|---|---|
| **SEC-001** | Thor MUST NOT provide features whose primary purpose is attacking third parties without authorization (credential stuffing, botnet orchestration, mass scanning, drive-by MitM of arbitrary public hosts). |
| **SEC-002** | Before enabling **traffic interception/tamper**, Thor MUST require an explicit **scope declaration**: hostname/URL pattern allowlist + confirmation that the user owns the target or has written authorization. |
| **SEC-003** | Before starting a **load test**, Thor MUST require confirmation of target URL, users, duration, and estimated request volume; confirmation MUST restate that unauthorized load testing may be unlawful. |
| **SEC-004** | Tamper and Locust MUST default to **localhost / loopback / RFC1918 / `.localhost` / `.test` / `dev.local`**; targeting public Internet hosts MUST require an additional “public target” acknowledgment (not a single checkbox buried in settings). |
| **SEC-005** | Product copy, README, and in-app alerts MUST state that security-lab features are for **authorized testing of the user’s own systems** only. |
| **SEC-006** | Agents (human or AI) implementing Thor MUST refuse requests to add exploit PoCs, mass abuse tooling, or silent MitM of hosts outside declared scope. |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-007** | Thor SHOULD persist a per-session **ConsentScope** record (patterns, timestamp, profile mode) and refuse tamper/load actions outside it until re-confirmed. |
| **SEC-008** | Thor SHOULD warn when scope patterns are overly broad (`*`, empty string, bare `.com`, etc.). |

---

## 3. Profile model (DevHost vs StealthTarget)

Aligned with `ProfileMode` / `ProfileConfig` in `src/engine/profiles.rs`.

### MUST

| ID | Requirement |
|---|---|
| **SEC-010** | **DevHost** MUST: `strip_tracking_params=false`, `inject_gpc_dnt_headers=false`, `randomize_fingerprints=false`, `allow_webgl=true`, `enable_ide_bridge=true` (defaults match current `dev_host_default`). |
| **SEC-011** | **StealthTarget** MUST: `strip_tracking_params=true`, `inject_gpc_dnt_headers=true`, `randomize_fingerprints=true`, `enable_ide_bridge=false`. |
| **SEC-012** | Profile switch MUST apply **consistently** to navigation sanitization, header injection, fingerprint behavior, and IDE bridge — not UI-label-only (current chrome alerts must match Rust `ProfileManager` state via `AppState::global()`). |
| **SEC-013** | Cookie jars / local storage / Chromium profile partitions MUST NOT silently merge DevHost and Stealth identities. Prefer separate storage partitions or an explicit “shared storage” warning before any shared profile path. |
| **SEC-014** | User-Agent / Client Hints under Stealth MUST remain **plausible Chromium-compatible** (no unique “Thor fingerprint” that increases trackability beyond necessary). DevHost MAY identify as Thor in Sec-CH-UA for DX honesty. |
| **SEC-015** | Stealth MUST pause/disable IDE bridge IPC that can leak local filesystem paths or project names to web content. |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-016** | Profile mode SHOULD be visible in chrome chrome UI at all times and reflected in any exported HAR/audit logs. |
| **SEC-017** | Per-tab profile override SHOULD be considered; if deferred, document that profile is process-global and switching affects all tabs. |

### COULD

| ID | Requirement |
|---|---|
| **SEC-018** | COULD offer a “Lab” profile that enables tamper+Locust with mandatory scope, separate from daily Stealth browsing. |

---

## 4. Traffic tamper rules

Grounded in `TrafficTamperEngine` / `TamperRule` (`src/net/tamper.rs`). Interception defaults off today — keep that.

### MUST

| ID | Requirement |
|---|---|
| **SEC-020** | Interception MUST default to **disabled**. Enabling requires SEC-002 consent. |
| **SEC-021** | Rules MUST validate: non-empty pattern; pattern length cap; reject patterns that match all URLs unless user confirms “global rule”; header names MUST match HTTP token grammar; values MUST have size caps. |
| **SEC-022** | Substring `url.contains(pattern)` matching (current behavior) MUST be documented as unsafe-if-naive; MUST migrate to **explicit** matchers (host suffix, path prefix, or safe glob) before shipping as “production lab.” |
| **SEC-023** | Every applied rule (auto or manual forward) MUST append an **audit log** entry: timestamp, rule id, request id, URL (redacted query secrets), header keys changed (not necessarily secret values), profile mode, scope id. |
| **SEC-024** | Tamper history buffers MUST be **bounded** (count + bytes); clearing history MUST be available; tab close / 24h GC policy MUST cover sensitive bodies. |
| **SEC-025** | Body rewrite / `Fetch.fulfillRequest` mocking MUST NOT be enabled without the same scope consent and MUST never inject executable payloads into third-party origins outside scope. |
| **SEC-026** | Dropping or holding requests MUST surface in UI; silent indefinite park of all traffic when interception is on MUST have a fail-safe (timeout → fail closed with user notice). |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-027** | SHOULD redact `Authorization`, `Cookie`, `Set-Cookie`, and known API-key headers in on-screen history by default (reveal requires explicit click). |
| **SEC-028** | SHOULD prevent exporting full raw history to disk without confirmation. |

---

## 5. Certificate / local CA trust UX

Grounded in `SslManager` (`src/net/ssl_ca.rs`): ECDSA P-256 CA + leaf for `*.localhost` / `localhost` / `*.test` / `dev.local`; `install_ca_trust` touches **system** trust stores.

### MUST

| ID | Requirement |
|---|---|
| **SEC-030** | CA private key (`Thor-dev-ca.key.pem`) MUST be stored under Thor data dir with **restrictive filesystem permissions** (owner read-only); MUST NOT be world-readable. |
| **SEC-031** | Installing CA to OS trust store MUST require **explicit, high-friction UX**: explain that a trusted root can MitM TLS for software that trusts that store; show cert fingerprint; require typed confirmation (e.g. type `TRUST THOR DEV CA`). |
| **SEC-032** | Leaf SANs MUST remain limited to **local/dev** names unless user explicitly requests additional SANs and acknowledges risk; MUST NOT auto-issue leaves for arbitrary public DNS names without separate “dynamic leaf for scoped host” flow gated by consent. |
| **SEC-033** | Thor MUST provide **uninstall / distrust** instructions and, where OS APIs allow, a one-click remove-trust path. |
| **SEC-034** | System keychain install (macOS `security add-trusted-cert` to System.keychain) MUST be treated as **admin-privileged**; failure MUST surface clearly; Thor MUST NOT silently retry with elevated privileges without OS auth UI. |
| **SEC-035** | CA/leaf generation for tests MUST never write keys into the repo workspace or world-writable temp without cleanup (tests already use temp dirs — keep that). |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-036** | Prefer **user-keychain / browser-only trust** over system root when technically sufficient for Chromium profile used by Thor. |
| **SEC-037** | SHOULD show “Dev CA trusted: yes/no” status in settings with fingerprint. |

### COULD

| ID | Requirement |
|---|---|
| **SEC-038** | COULD support ephemeral CA per project with auto-expiry (short TTL). |

---

## 6. Secrets handling

### MUST

| ID | Requirement |
|---|---|
| **SEC-040** | API keys / tokens in Postman-like collections and env vars MUST be stored encrypted at rest or in OS keychain; plaintext files in Thor data dir MUST be mode-restricted if encryption is not yet shipped (document gap honestly). |
| **SEC-041** | Secrets MUST NOT be written to logs, crash reports, Cmd+K history, tech-stack scan output, or Locust metrics payloads. |
| **SEC-042** | UI MUST mask secrets by default; copy-to-clipboard MUST require explicit action. |
| **SEC-043** | Export of collections MUST support a **strip secrets** mode as default; “include secrets” requires confirmation. |
| **SEC-044** | CA private keys and collection secrets MUST be excluded from any future sync/cloud feature unless end-to-end encrypted (out of scope = do not sync). |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-045** | SHOULD integrate OS keychain for high-value secrets (macOS Keychain, Windows Credential Locker, libsecret). |
| **SEC-046** | SHOULD detect accidental paste of secrets into tamper header values and warn. |

---

## 7. JWT / decoder

Grounded in `DecoderStudio`; AGENTS.md defers HMAC crack — keep honest.

### MUST

| ID | Requirement |
|---|---|
| **SEC-050** | Decoder MUST be **display/inspect only**: decode JWT header/payload, Base64, Hex, JSON. |
| **SEC-051** | Thor MUST NOT implement credential stuffing, password spraying, token brute-force farms, or wordlist-driven JWT cracking as a product feature. |
| **SEC-052** | `decode_jwt_crack` MUST remain **unavailable** (or clearly “not linked / not supported”) until a deliberate crypto design ships; MUST NOT show fake “HMAC verified” success theater. |
| **SEC-053** | Decoded output MAY warn on `alg=none` / dangerous alg confusion for educational value; MUST NOT auto-“fix” tokens against remote IdPs. |
| **SEC-054** | Pasted JWTs MUST be treatable as secrets: optional clear-on-blur; avoid persisting full tokens in unbounded history. |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-055** | SHOULD offer claim highlighting (exp, iss, aud) without sending tokens off-device. |

### COULD

| ID | Requirement |
|---|---|
| **SEC-056** | COULD verify signature **locally** when user supplies a public key/JWKS file they provide — still not remote stuffing. |

---

## 8. Load testing ethics & guardrails

Grounded in `LocustHandle` / `LocustConfig` — currently no target allowlist or rate caps beyond `users.max(1)`.

### MUST

| ID | Requirement |
|---|---|
| **SEC-060** | Locust start MUST enforce SEC-003 / SEC-004 consent and scope. |
| **SEC-061** | Default target MUST be `http://127.0.0.1` / `localhost` (or empty with blocked start); MUST reject empty URL (already). |
| **SEC-062** | MUST enforce hard caps unless “advanced” unlock: e.g. max users ≤ 100 (default ≤ 10), ramp ≤ 60s, runtime ≤ 300s, max RPS budget derived from users. Public targets: stricter defaults. |
| **SEC-063** | MUST support immediate **stop** that cancels in-flight work (`stop` flag already exists — wire reliably in UI and verify). |
| **SEC-064** | MUST NOT follow open redirects to out-of-scope hosts during a run. |
| **SEC-065** | MUST log start parameters (URL host, users, duration) to audit log; MUST NOT log Authorization headers from responses. |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-066** | SHOULD block known cloud metadata IPs (`169.254.169.254`, etc.) and link-local abuse targets. |
| **SEC-067** | SHOULD show live estimated total requests before start. |

### COULD

| ID | Requirement |
|---|---|
| **SEC-068** | COULD import Locust-like scripts later; any scripting host inherits WASM/plugin threat model (SEC-080+). |

---

## 9. Tracker stripping / privacy defaults

Grounded in `SecurityShield`.

### MUST

| ID | Requirement |
|---|---|
| **SEC-070** | Stealth MUST strip known tracking params (utm_*, gclid, fbclid, …) per `SecurityShield`; DevHost MUST leave them intact by default. |
| **SEC-071** | Stripping MUST preserve non-tracking query params and fragments; MUST NOT break URL structure (covered by existing tests — keep green). |
| **SEC-072** | When stripping causes site breakage, Thor MUST offer **per-site bypass** or one-click “reload with original URL” without disabling Stealth globally forever. |
| **SEC-073** | DNT/GPC injection in Stealth MUST not claim legal compliance guarantees beyond sending the headers. |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-074** | SHOULD show the existing “Trackers Stripped” notice only when strip actually occurred. |
| **SEC-075** | SHOULD allow user-editable allow/deny lists for param names. |

---

## 10. Extension / WASM plugin surface

`WasmPluginMicrokernel` is registry-only today; AGENTS.md defers real WASM sandbox and MV3 host.

### MUST (even while deferred)

| ID | Requirement |
|---|---|
| **SEC-080** | Until a real sandbox ships, Thor MUST NOT load untrusted WASM/MV3 code in-process or claim extension support in UI (no stub theater). |
| **SEC-081** | When implemented, plugins MUST declare capabilities (network, filesystem, CDP, secrets); default deny; user grant per capability. |
| **SEC-082** | Plugins MUST NOT gain CA private key access or OS trust-store install rights. |
| **SEC-083** | Plugins MUST NOT bypass Locust/tamper consent gates. |

### DEFERRED threat model (document now, implement later)

- Malicious plugin exfiltrates cookies via CDP.
- Prototype pollution / memory unsafety in host FFI.
- Privilege confusion between chrome webview and plugin worker.

**Status:** Explicitly deferred; SEC-080 is the binding v1 control.

---

## 11. Secure IPC (chrome ↔ content / Rust)

Chrome is wry webview HTML/JS; content is Chromium; Rust handles privileged actions (`shell/mod.rs` IPC allowlist).

### MUST

| ID | Requirement |
|---|---|
| **SEC-090** | IPC command surface MUST be an **allowlist** only (existing cmds). Unknown cmds MUST no-op and SHOULD log. |
| **SEC-091** | Chromium **content** pages MUST NOT be able to invoke Thor privileged IPC (`locust_start`, `add_tamper_rule`, `install_ca`, `open_ide`, etc.). Only the Thor chrome webview (or Rust UI) may. |
| **SEC-092** | IPC payloads MUST be schema-validated (types, ranges, string lengths); reject oversized bodies. |
| **SEC-093** | `open_ide` path arguments MUST be constrained (no arbitrary shell); prefer allowlisted IDE schemes and sanitized paths. |
| **SEC-094** | Rust → JS evaluation hooks (`__thorUpdate*`) MUST treat injected strings as data; prevent script breakout via unescaped content in evaluated JS. |
| **SEC-095** | Privileged security actions (enable interception, install CA, locust public target) MUST be enforceable in **Rust**, not only in chrome JS. |

### SHOULD

| ID | Requirement |
|---|---|
| **SEC-096** | SHOULD add integration tests that simulate malformed IPC and assert rejection. |

---

## 12. Acceptance criteria & security test cases (Tester)

### Acceptance criteria (release gate)

1. Interception off by default; cannot enable without consent UI + Rust-side flag tied to scope.
2. Locust refuses empty/public targets without dual confirmation; caps enforced in Rust.
3. Profile toggle changes real `ProfileManager` behavior (strip/headers/IDE), not label alone.
4. Decoder never reports fake crack success; crack path honest-unavailable.
5. No WASM/MV3 load in v1 builds.
6. CA install requires typed confirmation; key files not world-readable.
7. Secrets not present in default logs or exported collections.
8. Content process cannot call privileged IPC (negative test).

### Required test cases (Tester MUST run)

| ID | Case | Expected |
|---|---|---|
| **SEC-T01** | Enable interception without accepting scope | Blocked |
| **SEC-T02** | Add tamper rule with empty/`*` pattern | Rejected or dual-confirm + audit |
| **SEC-T03** | Apply rule; inspect audit log | Entry present; secrets redacted |
| **SEC-T04** | Locust → `https://example.com` with defaults | Public confirmation required; caps applied |
| **SEC-T05** | Locust → `http://127.0.0.1:PORT` | Allowed after basic confirm; stop works |
| **SEC-T06** | Toggle Stealth; navigate URL with `utm_source` | Param stripped; notice shown |
| **SEC-T07** | Toggle DevHost; same URL | Param preserved |
| **SEC-T08** | Stealth + IDE bridge attempt | Bridge inactive |
| **SEC-T09** | Paste JWT; decode | Header/payload shown; no crack success |
| **SEC-T10** | Call `decode_jwt_crack` | Honest unavailable / error |
| **SEC-T11** | Generate CA; check key perms | Owner-only |
| **SEC-T12** | Install CA trust without typing confirm | Blocked |
| **SEC-T13** | Malformed IPC from chrome | No panic; rejected |
| **SEC-T14** | Inject privileged IPC from content page (manual/CDP) | No effect |
| **SEC-T15** | Existing `integration_tests` shield/tamper/profile/ssl suites | Pass |
| **SEC-T16** | History/GC: sensitive bodies after tab close / 24h | Cleared or inaccessible |

---

## 13. Perf–security tradeoffs (Perf agent)

### MUST NOT sacrifice for speed

| ID | Requirement |
|---|---|
| **SEC-100** | Consent / confirmation gates. |
| **SEC-101** | IPC allowlist + payload validation. |
| **SEC-102** | Locust rate/user caps and cancelation. |
| **SEC-103** | Secret redaction in logs/UI defaults. |
| **SEC-104** | CA key permissions and trust UX friction. |
| **SEC-105** | Profile isolation guarantees (no “share cookies for faster switch” without warning). |
| **SEC-106** | Audit log writes for tamper/load (async OK; drop-on-full must fail safe, not silent). |

### MAY optimize

| ID | Requirement |
|---|---|
| **SEC-110** | Tracker strip hot path (already HashSet) — OK to optimize allocations. |
| **SEC-111** | Locust metric scoring (zero-alloc goals) — OK if metrics exclude secrets. |
| **SEC-112** | CDP Fetch pipeline latency — OK if interception remains opt-in and scoped. |
| **SEC-113** | Prewarm Chromium pool — OK if frozen tabs cannot run Locust/tamper without active consent. |

---

## Requirement priority index

### MUST (ship blockers)

SEC-001–006, 010–015, 020–026, 030–035, 040–044, 050–054, 060–065, 070–073, 080–083, 090–095, 100–106.

### SHOULD

SEC-007–008, 016–017, 027–028, 036–037, 045–046, 055, 066–067, 074–075, 096.

### COULD

SEC-018, 038, 056, 068.

---

## Handoff

### For Developer

- Wire **Rust-enforced** consent scope for `set_interception_enabled`, `add_tamper_rule`, `locust_start`, and any CA install IPC — chrome JS is not a security boundary.
- Replace naive `url.contains` matching with explicit host/path matchers before calling CDP Fetch “done.”
- Keep `AppState::global()` singletons; no per-message engines.
- Persist audit log; bound history; redact secrets.
- Harden key file modes; redesign CA trust UX (typed confirm, distrust path).
- Keep JWT crack unavailable/honest; remove UI that implies cracking works.
- Do not ship WASM/MV3 loaders; leave stubs non-functional.
- Partition or warn on DevHost vs Stealth storage; disable IDE bridge in Stealth for real.
- Schema-validate all IPC; ensure content cannot reach privileged cmds.

### For Tester

- Execute SEC-T01–T16 every milestone that touches net/profiles/ssl/decoder/IPC.
- Treat “UI alert says Stealth” without Rust state change as **fail**.
- Verify Locust metrics are from real HTTP, not RNG theater.
- Negative-test public target and empty pattern paths.
- Confirm `cargo test --lib` and `cargo test --test integration_tests` stay green for shield/tamper/profile/ssl.

### For Perf

- Benchmark strip, Locust scoring, CDP continue path **after** gates exist.
- Do not remove caps, redaction, or audit I/O to win benches.
- Accept async audit / bounded queues; never unbounded intercept history for speed.
- Prewarm/freeze must not skip consent when enabling lab features on a tab.

---

## Explicit NON-GOALS

- **No botnet / DDoS-as-a-service** features, peer swarming, or anonymous attack relays.
- **No credential stuffing**, password spraying, account-checking, or bulk login automation.
- **No drive-by MitM** of arbitrary Internet hosts; no silent system CA install; no auto-trust without UX.
- **No fake security theater** (fake HMAC verified, fake Locust RNG, pretend extension loading).
- **No off-device JWT/secret exfiltration** for “cloud decode.”
- **No Tor/anonymity guarantee**; Stealth ≠ unlinkable browsing.
- **No weaponized exploit generation** or PoC payloads in-tree for attacking third parties.
- **No real WASM/MV3 host in v1** until capability sandbox exists (SEC-080).

---

*End of security requirements.*