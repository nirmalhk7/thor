# Thor browser overlays (Chromium tree)

Files here are copied/patched into `$THOR_CHROMIUM_SRC/src` by `scripts/apply_thor_branding.sh`.

```
browser/
  gn_args.thor.gn          # recommended out/Thor args
  branding/
    BRANDING               # product name strings
    thor_resources.md      # resource ID notes
  patches/
    0001-thor-product-name.patch.example
  native_messaging/
    com.thor.labs.json     # host manifest template for Rust sidecar
  isolation/
    MAPPING.md             # Level 1–3 → Profile/Partition/WebContents
```

Do not edit Chromium UI from multiple agents at once — Browser Chrome stream owns `chrome/browser` changes.
