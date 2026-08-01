# Thor browser overlays (Chromium tree)

Files here are applied directly to the pinned Chromium tree by
`scripts/apply_thor_branding.sh`.

```
browser/
  gn_args.thor.gn          # Thor release-build args
  branding/
    BRANDING               # product name strings
    thor_resources.md      # resource ID notes
  patches/
    0001-thor-product-name.patch  # product branding and bundle identity
    0002-thor-privacy-defaults.patch
    0003-thor-native-gpc.patch
    0004-thor-new-identity.patch  # disposable native OTR identity action
    0005-thor-navigation-url-cleanup.patch  # conservative top-level tracking-query cleanup
    0006-thor-native-profile-policy.patch    # DevHost/Stealth/Hardened API + JIT defaults
    0007-Reduce-WebGL-debug-renderer-exposure.patch # native WebGL renderer reduction for target profiles
    0008-thor-destroy-identity.patch      # explicit native teardown request for selected disposable identity
  native_messaging/
    com.thor.labs.json     # host manifest template for Rust sidecar
  isolation/
    MAPPING.md             # Level 1–3 → Profile/Partition/WebContents
```

Do not edit Chromium UI from multiple agents at once — Browser Chrome stream owns `chrome/browser` changes.
