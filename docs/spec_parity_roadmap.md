# Mango Browser: 3-Tier Testing & Chromium/Gecko Rendering Parity Roadmap

This document outlines the systematic, three-tier testing strategy designed to elevate the Mango Browser engine from specification conformance to complete, pixel-accurate visual parity with Chromium and Gecko on real-world web components.

---

## The 3-Tier Strategy Overview

```
┌────────────────────────────────────────────────────────────────────────┐
│  Tier 1: Specification Conformance (WPT-Aligned Unit & Algorithmic)    │
│  Validates isolated W3C standards: CSS parsing, cascade, selectors,    │
│  HTML5 tokenization, entities, box model formulas, DOM APIs.           │
├────────────────────────────────────────────────────────────────────────┤
│  Tier 2: Visual Regression & Pixel Reftests (Golden Image Diff)        │
│  Renders identical HTML/CSS in Mango and Headless Chromium, dumping    │
│  bitmaps to assert pixel parity (<0.1% diff tolerance) across display  │
│  lists, 2D vector shapes, text shaping, and CSS Appendix E stacking.   │
├────────────────────────────────────────────────────────────────────────┤
│  Tier 3: Real-World Component Corpus (Design System Verification)      │
│  Validates complex modern UI components (Tailwind UI, Bootstrap 5,     │
│  Radix, GitHub, HackerNews) under real nesting, flex/grid constraints, │
│  baseline alignment, and automatic minimum sizing.                     │
└────────────────────────────────────────────────────────────────────────┘
```

---

## Tier 1: Specification Conformance (WPT-Aligned Unit Parity)

Validates individual specification algorithms in isolation to ensure strict compliance with W3C and WHATWG standards.

### Phase 1.1: HTML5 Parsing & Tree Construction (WHATWG)
- [x] HTML5 entity decoding across all 2,000+ named entities (`&copy;`, `&euro;`, `&alpha;`, `&amp;`, etc.)
- [x] Self-closing slash on non-void elements treated as open tags per HTML5 spec (`<div/>` does not self-close)
- [x] RCDATA parsing rules for `<title>` and `<textarea>` (decodes entities without creating element nodes)
- [x] Case-insensitive HTML tag and attribute matching with case-sensitive class and ID attributes
- [ ] Active formatting elements list & foster parenting for malformed tables (`<table><tr><td>` repairs)
- [ ] `template` element content document fragment isolation
- [ ] Script execution ordering (`async`, `defer`, inline module vs. classic script)

### Phase 1.2: CSS Engine Cascade, Selectors & Values (CSS 2.1 / CSS3 / CSS4)
- [x] Full cascade origin and weight resolution (User Agent < User < Author Normal < Inline Normal < Author `!important` < Inline `!important`)
- [x] Specificity calculation (`(a, b, c, d)` tuple math: inline, ID, class/pseudo-class/attribute, type)
- [x] Dropping declarations with unknown units and falling back to prior valid cascade rules
- [x] `@supports` feature query evaluation for CSS declarations and selector conditions
- [x] `@media` query evaluation for `min-width`, `max-width`, `min-height`, `max-height`, `orientation`
- [x] CSS math expressions (`calc()`, nested parentheses, basic arithmetic)
- [x] Dynamic viewport units (`vw`, `vh`, `vmin`, `vmax`) synchronized with runtime viewport resizing
- [x] CSS custom properties (`var(--name, fallback)`) with cascading resolution
- [ ] Advanced selector combinators: `:has()`, `:is()`, `:where()`, `:not()`, `:nth-child(An+B [of S])`
- [ ] Attribute selectors: `[attr^=val]`, `[attr$=val]`, `[attr*=val]`, `[attr~=val]`, `[attr|=val]`

### Phase 1.3: Layout Formatting Contexts (CSS 2.1 / Flexbox / Grid)
- [x] Idempotent box tree relayout (subsequent layout passes produce zero drift or text run duplication)
- [x] Preserving DOM whitespace semantics without synthetic inline spacing insertion
- [x] `inline-block` shrink-to-fit sizing
- [x] Inline replaced elements (`<img>`, `<svg>`, `<canvas>`) participating in inline formatting contexts
- [x] Flexbox auto margin distribution stability on dynamic relayout
- [x] CSS Grid negative line indices (`-1`, `-2`) and fractional `fr` track allocation
- [x] Table `min-content` clamping preventing compression below cell contents
- [x] `position: fixed` containing block established by the viewport rather than DOM parent
- [x] `position: absolute` containing block established by the nearest positioned ancestor padding box
- [ ] Multi-column layout (`columns`, `column-gap`, `column-rule`)
- [ ] CSS fragmentation and pagination break rules (`break-inside: avoid`, `break-before`)

### Phase 1.4: JavaScript Runtime & DOM API Surface
- [x] `window.matchMedia(query)` dynamic evaluation matching runtime resolution changes
- [x] DOM element traversal (`querySelector`, `querySelectorAll`, `getElementById`, `getElementsByTagName`)
- [x] Property reflection (`style`, `classList`, `innerHTML`, `textContent`, `setAttribute`, `getAttribute`)
- [x] Event target subscription and dispatching (`addEventListener`, `removeEventListener`, `dispatchEvent`)
- [x] Timer integration (`setTimeout`, `setInterval`, `requestAnimationFrame`)
- [x] `ResizeObserver`, `IntersectionObserver`, and `MutationObserver` callbacks (✅ IMPLEMENTED & VERIFIED in `mango_js::runtime::test_section_7_4_dom_measurement_and_layout`)
- [x] Full `Fetch` / `XMLHttpRequest` response handling with SOP & CORS validation (✅ IMPLEMENTED & VERIFIED in `mango_js::runtime::test_section_7_3_web_apis`)

---

## Tier 2: Visual Regression & Pixel Reftests (Chromium Golden Diff)

Compares Mango's software rasterization output against headless Chromium pixel buffers to guarantee visual equivalence.

### Phase 2.1: Reftest Harness Infrastructure
- [ ] Headless Chromium automation driver in `tools/reftest` to capture reference PNGs at 800x600, 1280x720, and 1920x1080
- [ ] Automated pixel diff engine comparing Mango RGBA buffers against Chromium reference images
- [ ] Configurable per-pixel color delta threshold (`ΔE < 2.0` in CI to accommodate font antialiasing variance)
- [ ] Automated HTML visual diff report generator highlighting mismatching pixels in magenta

### Phase 2.2: Subpixel Layout & Device Pixel Snapping
- [ ] Fractional layout coordinate calculation with physical pixel border snapping
- [ ] Sharp 1px borders (preventing 1px borders on fractional coordinates from blurring across 2 physical pixels)
- [ ] Adjacent box seamless tiling (preventing 0.5px white gap leaks between adjacent flex/grid cells)
- [ ] Text baseline pixel snapping to eliminate vertical jitter during scrolling or resizing

### Phase 2.3: CSS 2.1 Appendix E Stacking Contexts & Paint Layering
- [x] Strict 7-layer Appendix E painting order implementation in `mango_render::display_list`:
  - Layer 1: Background and borders of the element forming the stacking context
  - Layer 2: Descendant stacking contexts with negative `z-index` (lowest first)
  - Layer 3: In-flow, non-inline-level, non-positioned descendant boxes
  - Layer 4: Non-positioned floating descendants
  - Layer 5: In-flow, inline-level, non-positioned descendant boxes (text, inline blocks)
  - Layer 6: Descendant stacking contexts with `z-index: 0` / `auto` and positioned descendants
  - Layer 7: Descendant stacking contexts with positive `z-index` (lowest first)
- [x] Spec-compliant stacking context triggers:
  - [x] `opacity < 1.0`
  - [x] `transform != none`
  - [x] `filter != none`
  - [x] `clip-path != none`
  - [x] `perspective != none`
  - [x] `isolation: isolate`
  - [x] `contain: paint` / `contain: strict`
- [ ] Reftests verifying dropdown menus, tooltips, and modals always render above background cards

### Phase 2.4: 2D Graphics Fidelity, SVG & Effects
- [x] Inset box shadows (`box-shadow: inset ...`) clipping to inner border edge
- [x] Multi-value outer box shadows with blur radius and spread radius
- [x] Filter operations (`drop-shadow()`, `blur()`)
- [x] SVG rendering of shapes (`<rect>`, `<circle>`, `<path>`), strokes, and `fill="none"`
- [ ] Advanced SVG `<use>`, `<symbol>`, and `<defs>` cross-referencing and transform cascading
- [x] CSS Gradients: Linear, radial, and conic gradients with exact color stop interpolation and angles
- [x] `border-radius` clipping: child backgrounds and images clipped cleanly by parent rounded corners

### Phase 2.5: Typography, Font Metrics & Complex Script Shaping
- [x] Multilingual text shaping and bidirectional text layout (Arabic RTL, CJK ideographs, Latin)
- [ ] Font metric extraction: OS/2 and hhea table ascent, descent, and line-gap prioritization matching DirectWrite/FreeType
- [ ] Exact font half-leading calculation: `(line-height - (ascent + descent)) / 2`
- [x] Web font loading via `@font-face` (WOFF2, TTF, OTF) with fallback font matching
- [x] Text decoration styling: `text-decoration-line`, `text-decoration-color`, `text-decoration-style` (wavy, dashed)

---

## Tier 3: Real-World Modern Component Corpus (Design System Parity)

Tests Mango against production component architectures used across modern frontend frameworks (Tailwind, Bootstrap, Radix, Material UI).

### Phase 3.1: Inline-Block & Inline-Flex Baseline Alignment
- [ ] CSS 2.1 §10.8.1 baseline calculation for `inline-block`:
  - Last in-flow line box baseline used if children exist
  - Bottom margin edge used if `overflow != visible`
  - Bottom margin edge used if empty or containing only replaced elements
- [ ] Buttons with leading/trailing SVG icons perfectly centered with text labels
- [ ] Badges and chips aligned cleanly with adjacent headings and paragraphs
- [ ] Vertical-align keyword parity: `baseline`, `middle`, `sub`, `super`, `text-top`, `text-bottom`, `top`, `bottom`

### Phase 3.2: Comprehensive User-Agent (UA) Stylesheet
- [ ] Complete default styles for HTML form controls (`button`, `input`, `select`, `textarea`):
  - [ ] `button`: `box-sizing: border-box`, default padding `1px 6px`, border styling, centered alignment
  - [ ] `input[type="text"]`, `input[type="password"]`, `input[type="email"]`: default sizing and inset borders
  - [ ] `input[type="checkbox"]` & `input[type="radio"]`: 13px–16px standard square/round geometry with check glyphs
  - [ ] `textarea`: default monospace/sans-serif font, multi-line wrapping, scrollbar container
  - [ ] `select`: native appearance dropdown arrow and option list
- [ ] Native element display properties:
  - [ ] `table`, `thead`, `tbody`, `tfoot`, `tr`, `td`, `th` table layout roles
  - [ ] `dialog`: top layer positioning, backdrop styling (`::backdrop`)
  - [ ] `details` and `<summary>` disclosure triangles and toggle layout

### Phase 3.3: Automatic Minimum Sizing & Intrinsic Sizing
- [ ] Flexbox item `min-width: auto` / `min-height: auto` defaulting to `min-content` size (CSS Flexbox §4.5)
- [ ] Text inside flex items shrinking and soft-wrapping rather than overflowing container borders
- [ ] Two-pass intrinsic sizing:
  - Pass 1: Compute `min-content` (longest unbreakable word) and `max-content` (unconstrained line)
  - Pass 2: Allocate track or container width
  - Pass 3: Final layout pass
- [ ] `width: fit-content`, `width: max-content`, and `width: min-content` support across blocks, flex items, and grid items

### Phase 3.4: Production Component Test Suites
- [ ] **Tailwind CSS Component Suite**:
  - [ ] Button variants (primary, outline, icon-only, disabled, pill)
  - [ ] Card grid (responsive cards with images, badges, title, body, and action buttons)
  - [ ] Navbar (brand logo, links, right-aligned CTA, mobile toggle hamburger)
  - [ ] Modal dialog (backdrop blur, centered card, header, body, footer actions)
  - [ ] Dropdown menu (absolute positioned container with shadow, z-index 50, item hover states)
- [ ] **Bootstrap 5 Component Suite**:
  - [ ] 12-column responsive grid (`row`, `col-*`, `g-*` gutters)
  - [ ] Accordion collapse & expansion
  - [ ] Form floating labels (`.form-floating > .form-control`)
  - [ ] Alerts and notification banners
- [ ] **Real Web Page Snapshots**:
  - [ ] GitHub repository view (sidebar, file list, README markdown styling)
  - [ ] Hacker News frontpage (nested table layout, voting arrows, comment threads)
  - [ ] Wikipedia article (infobox table float, multi-column references, table of contents)
  - [ ] Documentation site (sticky sidebar navigation, code blocks with line numbers, search bar)

---

## Phased Implementation Roadmap & Progress Tracker

| Milestone | Target Scope | Current Status | Key Deliverables |
| :--- | :--- | :--- | :--- |
| **Phase 1: Foundation** | WPT & Phase 2 Spec Parity | **COMPLETED (100%)** | 23/23 tests in `tests/spec_parity_tests.rs` passing; HTML5 entities, cascade, calc, flex margins, grid fr, fixed CB, dynamic resolution. |
| **Phase 2: Painting & Stacking** | Stacking Contexts & Pixel Snapping | **In Progress** | CSS 2.1 Appendix E 7-layer display list sorting, opacity/filter stacking context triggers, 1px border snapping. |
| **Phase 3: Visual Reftests** | Headless Chrome Golden Diff Harness | **Planned** | `tools/reftest` runner, automated PNG diffing with `<0.1%` tolerance threshold, CI test integration. |
| **Phase 4: Component Polish** | Baseline Alignment & UA Styles | **Planned** | `inline-block` baseline quirks, flex `min-width: auto`, complete form control UA stylesheet, Tailwind button/card parity. |
| **Phase 5: Production Corpus** | Real Framework & Page Parity | **Planned** | Tailwind UI, Bootstrap 5, GitHub, Wikipedia, HackerNews automated layout verification. |

---

## Running the Parity Test Suites

To execute the Tier 1 specification parity integration tests:
```bash
# Run all standards parity integration tests
cargo test --test spec_parity_tests

# Run with verbose stdout output
cargo test --test spec_parity_tests -- --nocapture

# Run the entire workspace test suite
cargo test --workspace
```
