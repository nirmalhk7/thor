# Thor Privacy and Security Plan

Thor is a Chromium downstream. Privacy and security guarantees belong in the
owned browser product first; the Rust Labs sidecar and bundled extensions are
instrumentation and defense-in-depth, not substitutes for browser enforcement.

## Source baseline

Behavioral requirements were cross-checked against the official projects for
[Helium](https://github.com/imputnet/helium),
[Brave](https://github.com/brave/brave-core),
[Cromite](https://github.com/uazo/cromite),
[ungoogled-chromium](https://github.com/ungoogled-software/ungoogled-chromium),
[LibreWolf](https://www.librewolf.net/docs/features/),
[Mullvad Browser](https://github.com/mullvad/mullvad-browser),
[Vanadium](https://github.com/GrapheneOS/Vanadium),
[Tor Browser](https://gitlab.torproject.org/tpo/applications/tor-browser), and
[DuckDuckGo Privacy Essentials](https://github.com/duckduckgo/duckduckgo-privacy-extension).
Thor reimplements desired behavior against its pinned Chromium revision; it
does not copy third-party patch sets without provenance and license review.

## Delivery gates

### P0 — Product integrity and safe defaults

- Build only the manifest-pinned Thor-owned Chromium source and reject stock,
  mismatched, or unmarked browser bundles.
- Fail closed when mandatory Thor privacy assets are absent or invalid.
- Preserve Safe Browsing database/component updates while disabling optional
  extended reporting.
- Use a verified ephemeral DevTools endpoint by default.
- Enable native HTTPS-Only, block third-party cookies, disable global history,
  search suggestions, speculative networking, hyperlink auditing, alternate
  error pages, translation offers, and Privacy Sandbox ad APIs by default.
- Keep strict site/origin isolation and release compiler mitigations enabled.

Acceptance: a clean profile passes build provenance checks; Safe Browsing
updates and interstitials work; no global History database is populated; HTTP
navigation reaches the native HTTPS interstitial; third-party cookies fail.

### P0 — Identity and storage isolation

- Bind Thor identity contexts to real Chromium `BrowserContext` /
  `StoragePartition` ownership.
- Partition cookies, DOM storage, IndexedDB, service workers, permissions,
  cache, HSTS, blobs, and network connection state.
- Expose native disposable-tab and identity controls in Thor browser chrome.
- Dispose the partition after the final bound tab closes, on Labs detach, and
  during orphan cleanup at startup.
- Add a one-action **New Identity** operation that closes bound tabs, cancels
  downloads, destroys storage, and creates a clean context.

Acceptance: two identities cannot observe each other's state; closing the last
disposable tab removes every tested storage surface and permission grant.

### P0 — Network privacy and blocking

- Replace the bootstrap hostname list with a maintained, signed filter
  component supporting tracker/ad rules, CNAME uncloaking, script surrogates,
  URL cleanup, and per-site exceptions.
- Apply rules in Chromium's request path so they cover subresources, service
  workers, redirects, and isolated contexts.
- Keep GPC on both request (`Sec-GPC`) and JavaScript
  (`navigator.globalPrivacyControl`) surfaces.
- Provide managed DoH resolver selection, status, bootstrap behavior, and a
  strict WebRTC `disable_non_proxied_udp` mode.
- Inventory browser-originated endpoints and enforce an audited allowlist
  without disabling security updates.

Acceptance: filter regression corpus passes; DNS CNAME tracking is blocked;
isolated tabs receive identical protection; clean-profile egress matches the
documented allowlist.

### P0 — Coherent anti-fingerprinting

- Implement the Stealth profile natively across Window, Worker, Worklet,
  Canvas, WebGL/WebGPU, Audio, fonts, media devices, timers, Client Hints,
  screen geometry, and locale/time-zone surfaces.
- Use stable site/session-scoped normalization or noise. Never expose Thor-only
  vendor strings or per-document instability that increases uniqueness.
- Keep DevHost truthful for debugging and make profile boundaries explicit.
- Treat document-start MAIN-world injection as a temporary defense, not the
  final guarantee.

Acceptance: early-page and worker tests observe one coherent profile; reloads
do not create a unique instability signal; cross-site linkability is reduced;
compatibility corpus remains within its budget.

### P1 — Hardened target mode

- Add opt-in per-site JIT disable and security tiers for hostile targets.
- Gate WebBluetooth, WebSerial, WebUSB, MIDI, sensors, Idle Detection, local
  font access, and other high-entropy/dangerous APIs in Stealth/Hardened modes.
- Keep certificate transparency and modern TLS floors; scope Thor development
  CAs to explicit DevHost consent, with verified install and uninstall.
- Add download quarantine, hashing, dangerous-download readback, and optional
  scanning without weakening Chromium Safe Browsing.
- Separate DevHost and Stealth extension sets and audit native-messaging and
  host permissions.

Acceptance: each dangerous API and JIT state is observable per site; CA and
download operations fail closed with verified OS/browser state.

### P1 — Navigation and local-data minimization

- Add maintained query stripping, debouncing, unlinkable bounce storage,
  `window.name` reset, and safe referrer/auth stripping.
- Prevent omnibox/search network requests before Enter in Stealth.
- Minimize form/search history, cache persistence, favicons, closed-tab state,
  and session metadata; document password-manager behavior.

Acceptance: OAuth/payment compatibility corpus passes; bounce identities are
unlinkable; clean-profile local artifacts match the documented retention map.

## Release verification

### Browserless contract gate

`python3 scripts/privacy_regression_check.py` runs without a browser and fails
closed on manifest-pin drift, patch-set drift, missing privacy-default hunks,
release-hardening GN drift, forbidden weakening flags, Shield manifest/asset
drift, launcher egress/CDP/WebRTC defaults, release-workflow drift, or an
ungated Linux/Windows packager. With `--chromium-src` and `--patch-state`, it
also verifies the exact pinned tag commit and origin, Chromium 151 GN
declarations, and forward/reverse patch applicability.

`generate_thor_provenance.py` binds each build/package to that exact commit and
to ordered patch, GN, Shield-tree, filter-rule, and built-executable digests. Source verification,
packaging, artifact verification, and the developer launcher reject a stale or
forged marker.

This is source/package scaffolding, not proof of runtime behavior. A release
remains incomplete until the browser-backed gates below run against the signed,
bundled Thor browser.

Every supported release must run:

1. Pinned-source, patch, GN-input, branding, and bundle provenance checks.
2. Fresh-profile egress and Safe Browsing readiness tests.
3. HTTPS-only, mixed-content, DoH, WebRTC, GPC, and filter-effect tests.
4. Cross-identity leakage and final-tab purge tests.
5. Early-main-world and worker fingerprint consistency tests.
6. Site-isolation, sandbox, compiler-hardening, CA, and download checks.
7. Signed package launch tests using only the bundled Thor browser.

Linux and Windows releases remain gated until they bundle and verify the same
owned Chromium product and Shield assets as macOS.

The macOS release workflow is likewise gated on a labeled high-capacity runner,
exact source/patch checks, a successful official Chromium build, bundle
verification, signing, notarization, ZIP/DMG integrity, and artifact checksum
readback. It never accepts the Xcode license automatically.

## Intentional exclusions

- No global Safe Browsing disablement.
- No global JIT, WebGL, WebRTC, DRM, or OS/user-root disablement; these belong
  in explicit hardened profiles where appropriate.
- No casual “Tor mode” without a separate anonymity threat model and complete
  proxy-bypass analysis.
- No policy bypass intended to defeat legitimate enterprise administration.
- No individual anti-fingerprint checkbox collection that creates rare,
  uniquely identifying configurations.
