---
name: Thor
description: Graphite Workspace for browser, API, and security work.
colors:
  accent: "#90b8f8"
  accent-hover: "#b4d0fc"
  api-action: "#244d7e"
  api-action-hover: "#2f6097"
  api-action-border: "#396b9f"
  graphite: "#151a20"
  inset: "#101419"
  raised: "#202831"
  separator: "#34404d"
  text: "#edf1f5"
  muted: "#a6b0bd"
  error: "#f3a6a6"
typography:
  headline:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "34px"
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: "-.03em"
  title:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "15px"
    fontWeight: 500
    lineHeight: 1.5
  body:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "12px"
    lineHeight: 1.5
  mono:
    fontFamily: "ui-monospace, SFMono-Regular, Consolas, monospace"
    fontSize: "12px"
    lineHeight: 1.7
  native-control:
    fontFamily: "Arial"
    fontSize: "12px"
  native-address:
    fontFamily: "Arial"
    fontSize: "13px"
  native-menu-caption:
    fontFamily: "Arial"
    fontSize: "16px"
  native-menu-search:
    fontFamily: "Arial"
    fontSize: "14px"
  native-menu-row:
    fontFamily: "Arial"
    fontSize: "13px"
rounded:
  shortcut: "4px"
  control: "5px"
  field: "6px"
  destination: "8px"
spacing:
  compact: "4px"
  small: "8px"
  control: "12px"
  group: "16px"
  section: "24px"
  panel: "28px"
  page: "32px"
  major: "40px"
components:
  button-analyze:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.inset}"
    rounded: "{rounded.control}"
    padding: "9px 18px"
  button-analyze-hover:
    backgroundColor: "{colors.accent-hover}"
  button-api:
    backgroundColor: "{colors.api-action}"
    textColor: "{colors.text}"
    rounded: "{rounded.field}"
    padding: "8px 12px"
  button-api-hover:
    backgroundColor: "{colors.api-action-hover}"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    rounded: "{rounded.field}"
    padding: "8px 12px"
  destination:
    backgroundColor: "{colors.inset}"
    textColor: "{colors.text}"
    rounded: "{rounded.destination}"
    padding: "8px 10px 8px 18px"
  analysis-field:
    backgroundColor: "{colors.inset}"
    textColor: "{colors.text}"
    typography: "{typography.mono}"
    rounded: "{rounded.field}"
    padding: "14px"
  workspace-nav:
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "6px 12px"
  workflow-row:
    textColor: "{colors.text}"
    padding: "22px 8px"
  native-command-shell:
    backgroundColor: "{colors.inset}"
    padding: "12px 16px"
  native-command-row:
    backgroundColor: "{colors.inset}"
    textColor: "{colors.text}"
    typography: "{typography.native-menu-row}"
  native-command-row-selected:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.accent}"
  feature-search:
    backgroundColor: "{colors.inset}"
    textColor: "{colors.text}"
    rounded: "{rounded.field}"
    padding: "10px 12px"
  feature-choice:
    backgroundColor: "transparent"
    textColor: "{colors.text}"
    padding: "12px"
  feature-choice-selected:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.accent}"
---

# Design System: Thor

## Overview

**Creative North Star: "Graphite Workspace"**

Graphite Workspace is a compact, dark instrument for browser work, API requests, and security evidence. Flat graphite surfaces, pale text, restrained ice blue, and precise separators keep controls legible without competing with the content.

The implemented language is functional and text led. A standard macOS title bar sits above one native navigation/address row; embedded HTML uses system sans for interface text and monospace for request data and results. Native controls and HTML share the core graphite palette. Sites and Toolbox use native command overlays outside page DOM. This record captures source values; runtime evidence is tracked separately, and no finish approval is recorded here.

**Key Characteristics:**

- Compact controls with clear active and focus states.
- Inset fields and tonal surface changes rather than decorative elevation.
- Small corners, fine separators, and restrained outline icons.
- Short state transitions that respect reduced motion.

Source evidence: `src/browser/cef/host.rs`, `src/browser/cef/host/theme.rs`, `src/browser/cef/host/palette.rs`, `src/workbench/workspace.css`, `src/workbench/workbench.css`, the home/security/API/Control Center HTML documents, and `src/workbench/tools.js`. The surface contract lives in `.impeccable/surfaces/src-browser-cef-host-rs.md`. Tokens above are extracted from those implementations; native geometry is expressed in CEF logical units, HTML geometry in CSS pixels.

## Colors

Cool graphite neutrals carry structure; ice blue identifies focus, active navigation, and analysis actions.

### Primary

- **Ice Blue** (`accent`): native focus borders and selected command results, active HTML navigation, outline icons, carets, and analysis actions.
- **Pale Ice** (`accent-hover`): analysis button hover.

### Secondary

- **Deep Request Blue** (`api-action`): API action buttons, with the related hover and border roles retained from the API stylesheet. This is an existing component variant, not a replacement for Ice Blue.

### Tertiary

- **Soft Error Rose** (`error`): home address validation and security analysis errors. No success color is established by these sources.

### Neutral

- **Graphite** (`graphite`): shell and page canvas.
- **Inset Graphite** (`inset`): native omnibox and command overlay interior, HTML sidebars, and HTML fields. The API stylesheet names this same role `--surface`.
- **Raised Graphite** (`raised`): hovered HTML workflow rows, selected HTML navigation and feature choices, and the selected native command result. The native menu defines this color directly.
- **Steel Separator** (`separator`): HTML field borders and section dividers, plus the native command overlay boundary.
- **Near White** (`text`): primary interface text and results.
- **Muted Steel** (`muted`): descriptions, native omnibox placeholder text, disabled native label colors, and supporting HTML status text.

### Named Rules

**The Shared Graphite Rule.** Use the shared graphite, inset, text, muted, and ice blue roles across native controls and embedded content. Use raised and separator roles for HTML regions and native command menus. Preserve semantic state distinctions.

Existing drift, not canonized: API errors use `#ff9a9a` while home/security use the recorded error token. Selection backgrounds also differ (`#264c7b` in API, `#36547d` in workspace). These differences remain source facts rather than new global palette rules.

## Typography

**Body Font:** native system sans stack in HTML; Arial in CEF native controls.
**Label/Mono Font:** `ui-monospace, SFMono-Regular, Consolas, monospace` for security inputs/results; the API stack omits Consolas.

**Character:** compact interface typography with restrained headings and clear separation between interface language and technical data. No custom font assets or dedicated display family are present.

### Hierarchy

- **Headline:** home heading uses the frontmatter headline role; it reduces to (28px) at the workspace narrow breakpoint. Security headings use (25px), weight (500).
- **Title:** workspace headings use the frontmatter title role. API headings use (15px), weight (600); its sidebar heading uses (20px), weight (600).
- **Body:** base HTML text uses the body role. Home introduction and address text use (14px). Supporting descriptions are constrained to (52ch) or (65ch) where the source establishes it.
- **Label:** supporting HTML notes and status text use (12px). Identity has no visible label in the native navigation row.
- **Technical data:** security editors and results use the mono role. API editors use (12px/1.6); responses use (12px/1.65) with tabular numerals.
- **Native controls:** the icon-button factory specifies native-control as its base font, with an empty visible label; the omnibox uses native-address. Command menu captions use native-menu-caption, search uses native-menu-search, rows use native-menu-row, and footer labels use native-control. Native line height and weight are not explicitly set. The standard macOS title bar retains platform typography.

### Named Rules

**The Data Type Rule.** Use monospace for request data, technical results, and shortcut keys; retain sans serif for interface text. The current native omnibox and home address field also use sans serif.

## Layout

The native macOS title bar uses standard red, yellow, and green window controls. The CEF window enables standard window buttons and sets its title to Thor; their rendering and geometry remain platform owned. Below it, the content root stacks one navigation/address row and a flexible browser content region. The row contains Back, Forward, Reload, Workspace, and the omnibox. There is no visible tab strip, identity label, or API/Security/Inspect button group.

Native icon controls are (32 × 34). The navigation row specifies a minimum cross-axis size of (46), spacing of (4), and inside horizontal/vertical spacing of (10/6). The omnibox receives flex (1), with an initial size of (400 × 34). Window preferred size is (1280 × 840), minimum (720 × 540); these are configured native values, not measured screenshots.

The home content uses a centered column with width `min(720px, calc(100% - 64px))` and top margin `clamp(40px, 10vh, 112px)`. Workflow rows are text led, separated by rules, and at least (105px) high. Home layout uses more whitespace than the working editors; do not turn that surface composition into a global density requirement.

Security uses a (190px) tool sidebar and a flexible analysis pane, capped at (1120px), with (32px 40px) pane padding. At (720px) and below the tool list wraps above the pane, and pane padding becomes (28px 24px).

API uses a (240px) saved-request sidebar and a flexible main pane capped at (1600px). At (900px) the sidebar becomes (190px) and request/save/response rows wrap. At (600px) the layout stacks; saved requests receive a (120px) maximum height. These are embedded-content adaptations; they do not define a separate mobile product.

Control Center uses a centered two-column grid capped at (1440px), with a (280px) feature sidebar and flexible analyzer. Sidebar padding is (28px 20px). At (800px) and below the sidebar becomes (220px) and analyzer padding becomes (28px 24px). The feature list scrolls within `calc(100vh - 225px)`.

Native command overlays are centered horizontally at logical y (76). Width is `(window.width - 48).clamp(300, 680)`; height is `142 + count.clamp(1, 7) * 46`. The Steel Separator enclosure is (1) logical unit around an Inset Graphite interior with horizontal/vertical padding (16/12) and section spacing (8). Search is (40) high; results have minimum row height (44), gap (2), and at most seven visible rows. These are configured CEF values, not screenshot measurements.

The spacing tokens capture reused steps, not an enforced mathematical scale. Preserve the owning surface's existing dimensions when extending it.

## Elevation & Depth

Depth comes from tonal layering and fine borders. The inspected native and HTML source defines no shadows, gradients, blur, or glow. Inset fields recede, selected and hovered items use the raised tone, and separators mark working regions. Native command menus are CEF overlay views with a Steel Separator enclosure; no invented scrim or shadow is part of this record.

### Named Rules

**The Tonal Depth Rule.** Use inset and raised surface tones plus fine separators for structure. The inspected surfaces define no shadows or glow.

## Shapes

HTML uses small, functional corners: shortcut keys use the shortcut radius, compact navigation and analysis actions use control radius, editor fields and API buttons use field radius, and the home address container uses destination radius. Editor tabs and workflow separators remain square. Borders are generally (1px); the API selected editor tab has a (2px) bottom border. CEF native corner behavior is not explicitly specified and must not be inferred from HTML radii.

HTML outline icons use (20 × 20px), stroke width (1.6), round caps, and round joins; workflow icons use (24 × 24px). Native navigation icons are drawn from line segments into (18px) images at scales (1) and (2), preserving native control ownership.

## Components

### Buttons

Compact and purposeful. Security's analysis action uses Ice Blue with inset text, weight (600), and the recorded control radius. API actions use Deep Request Blue with a fine blue border and weight (500); quiet actions use transparent fill and a separator border. API actions have minimum height (34px); this minimum is carried in the sidecar CSS because the frontmatter component schema has no minimum-height property.

Hover changes fill. API pressed state applies `brightness(.9)`. HTML disabled buttons use opacity (.5) and default cursor. Workspace focus uses a (2px) Ice Blue outline offset by (4px); API button/field focus uses the same outline offset by (2px). Retain those existing surface differences rather than silently normalizing them.

### Inputs / Fields

Inset, bordered, and editable. The home destination container strengthens its border to Ice Blue on focus within; its input is (36px) high. The submit control is (36 × 36px), raised at rest, and uses `#303c4a` on hover. Multiline security fields use the recorded analysis-field token and minimum height (180px). API multiline editors share that minimum height with (12px) padding. API field hover changes its border to `#505c70`. API input and textarea placeholders explicitly use Muted Steel at opacity (1); the source foreground/background pair has contrast (8.42:1). Control Center search uses the same placeholder treatment, inset background, separator border, field radius, and padding (10px 12px).

### Navigation

Native navigation icons use Near White in normal, hovered, and pressed states, with disabled icons drawn in `#687787`. Back and Forward enabled states follow actual browser history. The current identity appears in the omnibox accessible name (`Address — {identity} identity`), not as a visible control. Tabs remain session state controlled by keyboard: Cmd/Ctrl+T and Cmd/Ctrl+W create and close; Ctrl+Tab and Ctrl+Shift+Tab cycle; Cmd/Ctrl+Shift+[ and Cmd/Ctrl+Shift+] also cycle. Inspect uses Cmd/Ctrl+Alt+I. Sites opens with Cmd/Ctrl+K and Toolbox with Cmd/Ctrl+Shift+K; tool destinations also remain in the HTML workspace.

HTML workspace navigation uses muted text, raised hover fill, and Ice Blue current-page text. Security tool selection follows the same raised/blue pairing. API editor tabs use near-white selected text and the accent underline. No separate mobile navigation component is implemented.

### Workflow Rows

Open, rule-separated rows rather than floating cards. Each row groups an outline icon, title, short description, and trailing arrow. Hover and keyboard focus raise the background; the arrow changes to Ice Blue and shifts (3px) right. The implemented arrow remains visible at rest; focus and hover change its color and position.

Motion is limited to state changes: workspace rows/arrows use (150ms ease-out), API fields/buttons use (120ms ease-out), both only under `prefers-reduced-motion: no-preference`. No card or chip pattern is established by the inspected surfaces. The native command overlay below is the only command-menu pattern recorded here.

### Native Command Menus

Sites and Toolbox share one native searchable overlay. Its caption and search field lead into text-only result rows and a muted footer. The selected row pairs Raised Graphite with Ice Blue text; unselected rows use Inset Graphite with Near White text. A fine Steel Separator enclosure keeps the menu distinguishable from browser content. Native title-bar and navigation-row layout stays intact underneath.

Sites searches open tabs, the active tab's history, and bookmarks. Toolbox exposes Developer Control Center, API workspace, Security analyzers, Inspect page, Bookmark current page, and New tab. Search reflects actual results; an empty list displays “No matches. Try another title or URL.” Arrow keys cycle selection, Return opens it, Escape closes, and repeating the opening shortcut toggles that menu. Existing-tab results activate the tab; tool destinations reuse an existing tool tab or open one. These controls remain outside page DOM; do not substitute a webpage modal for this native surface.

### Developer Control Center

A filterable registry list shares the analyzer/result language. Feature choices are square, full-width rows with padding (12px), a (1px) separator below, and (11px) muted metadata. Hover raises the background; selected rows add Ice Blue text. Busy rows use opacity (.65) and wait cursor. Selection, empty-list, loading, disabled, running, completed, and error states reflect the actual registry and engine response.

The page loads the current 144-feature registry, filters name/domain/slug, accepts an Input JSON object, and invokes the selected Rust feature. Missing fields and unavailable engine connections show errors; active-page evaluation is explicitly unavailable from this tool tab. It reuses the existing analysis action, multiline editor, result header, monospace output, and muted empty-result treatment rather than defining a second visual system.

## Do's and Don'ts

### Do:

- **Do** reuse the shared palette roles for native and HTML surfaces, including raised selection and separator enclosure in native command menus.
- **Do** retain explicit focus, selected, disabled, and error states from the owning surface.
- **Do** use compact sans serif interface text and monospace technical editors and results.
- **Do** keep native control geometry owned by CEF and HTML control geometry owned by its stylesheet.
- **Do** respect reduced motion when extending existing transitions.

### Don't:

- **Don't** add glow or decorative shadow to the Graphite Workspace surfaces.
- **Don't** replace native controls with visual replicas that compromise omnibox or action hit-testing.
- **Don't** turn source inconsistencies or unverified native rendering into rules for future screens.
