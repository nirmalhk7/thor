# Archived: Chromium Downstream Experiment

This document describes Thor’s former Chromium downstream plan. It is retained as migration history only; it is not current product architecture or a build guide.

Thor’s product browser embeds CEF in the native Rust shell. Do not run the old depot_tools, Chromium fetch, snapshot, branding, build, or launch workflows. Their scripts now exit with a message and do not download or build Chromium.

Build Thor with the pinned prebuilt CEF distribution:

```bash
cargo build --locked --release --features cef-bundle --bin Thor --bin thor-labs
```

The CEF distribution is resolved by the `cef` Rust package. Thor does not fetch or build the full Chromium source tree and does not use system Chrome as its product browser. See [the current architecture](ARCHITECTURE.md) and [README](../README.md).
