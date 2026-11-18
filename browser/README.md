# Browser integration assets

`extensions/thor-lab/` contains the browser extension assets. `native_messaging/com.thor.labs.json` is the native messaging host template for the Rust sidecar.

Thor's product runtime lives in `src/browser/cef/`. Identity isolation uses CEF request contexts. See [the architecture](../docs/ARCHITECTURE.md) for current ownership and trust boundaries.
