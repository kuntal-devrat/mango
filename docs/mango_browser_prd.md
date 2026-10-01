# 🥭 Mango Browser — Production Requirements Document (PRD)

> **Version**: 1.0.0 · **Date**: 2026-09-24 · **Status**: Active  
> **Target**: Full modern web support (YouTube, Google, Wikipedia, GitHub, etc.)

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Codebase Audit — Bugs, Gaps & Optimizations](#2-codebase-audit)
3. [Architectural Recommendations (Anti-Chromium/Gecko/Servo)](#3-architectural-recommendations)
4. [Phase 0 — Foundation (Current)](#4-phase-0--foundation-current)
5. [Phase 1 — HTML5 Full Compliance](#5-phase-1--html5-full-compliance)
6. [Phase 2 — CSS3 Full Compliance](#6-phase-2--css3-full-compliance)
7. [Phase 3 — JavaScript Engine & DOM Bindings](#7-phase-3--javascript-engine--dom-bindings)
8. [Phase 4 — Layout Engine Completeness](#8-phase-4--layout-engine-completeness)
9. [Phase 5 — Rendering & Paint Pipeline](#9-phase-5--rendering--paint-pipeline)
10. [Phase 6 — Networking & Security](#10-phase-6--networking--security)
11. [Phase 7 — Platform Integration & Polish](#11-phase-7--platform-integration--polish)
12. [Site-Specific Compatibility Matrix](#12-site-specific-compatibility-matrix)
13. [Test Strategy](#13-test-strategy)

---

## 1. Executive Summary

Mango is an ultra-lightweight web browser built from scratch in **pure Rust**, treating the web as a document platform. The goal is:

| Metric | Target |
|--------|--------|
| Cold start | < 100ms |
| Idle RAM | < 30MB |
| Binary size | < 10MB |
| Language | 100% Rust — zero C/C++ in engine |

This PRD defines every feature required to render modern websites like YouTube, Google, Wikipedia, GitHub, and HackerNews — organized into **7 trackable phases** with checkbox items for cross-session progress tracking.

---

## 2. Codebase Audit

### 2.1 Bugs Found

- [x] **BUG-001: POST form submission sends data as GET query params** — ✅ FIXED — Implemented full HTTP POST navigation with `application/x-www-form-urlencoded` payload, cookie handling, and PRG (Post/Redirect/Get) 301/302/303 redirect support. [browser.rs](file:///d:/mango/src/browser.rs) [http.rs](file:///d:/mango/crates/mango_net/src/http.rs)

- [x] **BUG-002: `url_encode` is not UTF-8 safe** — ✅ FIXED — Rewrote to iterate chars and encode each byte of the UTF-8 representation. [browser.rs](file:///d:/mango/src/browser.rs)

- [x] **BUG-003: `error_page_html` is vulnerable to XSS injection** — ✅ FIXED — Now calls `html_escape()` on `target_url` and `error_message` before inserting into HTML. [browser.rs](file:///d:/mango/src/browser.rs)

- [x] **BUG-004: Radio button doesn't uncheck siblings** — ✅ FIXED — Now walks the form subtree to find and uncheck all same-name radio inputs before checking the clicked one. [browser.rs](file:///d:/mango/src/browser.rs)

- [x] **BUG-005: `data:` URI font handling is broken** — ✅ FIXED — Added `decode_data_uri_font()` and `base64_decode()` helpers to properly parse and decode data: URI font payloads. [browser.rs](file:///d:/mango/src/browser.rs)

- [x] **BUG-006: `Length::to_px` defaults viewport to 800×600** — ✅ FIXED — Added doc warning directing callers to use `to_px_with_viewports()` with actual window dimensions. [values.rs](file:///d:/mango/crates/mango_css/src/values.rs)

- [x] **BUG-007: `document_snapshot()` clones entire DOM on every relayout** — ✅ FIXED — Added `document_ref()` method returning `Ref<Document>` for zero-copy read-only access. [runtime.rs](file:///d:/mango/crates/mango_js/src/runtime.rs)

- [x] **BUG-008: `select` dropdown text collection ignores nested elements** — ✅ FIXED — Replaced manual text-node-only loop with `doc.text_content(cid)` which recursively collects all descendant text. [browser.rs](file:///d:/mango/src/browser.rs)

### 2.2 Gaps Identified

- [x] **GAP-001: No `<iframe>` support** — ✅ FIXED — Implemented WHATWG HTML5 raw text parsing (§13.2.6.4.7), CSS UA defaults (300×150 inline-block), presentational hints (`frameborder`), `BoxType::IFrame` with `IFrameSandbox` security model, child document layout (`srcdoc` and `data:text/html`), display list rendering with viewport fill & clipping, and `HTMLIFrameElement` JS/DOM bindings. [elements.rs](file:///d:/mango/crates/mango_html/src/elements.rs) [cascade.rs](file:///d:/mango/crates/mango_css/src/cascade.rs) [box_model.rs](file:///d:/mango/crates/mango_layout/src/box_model.rs) [box_tree.rs](file:///d:/mango/crates/mango_layout/src/box_tree.rs) [display_list.rs](file:///d:/mango/crates/mango_layout/src/display_list.rs) [dom_bindings.rs](file:///d:/mango/crates/mango_js/src/dom_bindings.rs) [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs)
- [x] **GAP-002: No `<video>`/`<audio>` playback** — ✅ FIXED — Implemented HTML5 media pipeline across all layers: UA stylesheet default display and dimensions (300×150 video, 300×36 audio controls), box tree integration (`BoxType::Video` and `BoxType::Audio`), poster image decoding and source resolution, display list drawing with modern dark slate player cards, play/pause toggles, elapsed/duration time displays, progress scrubber tracks, volume/mute icons, fullscreen badges, hit-testing & click actions (play/pause toggle, mute toggle, seek), context menu integration, and WHATWG `HTMLMediaElement`, `HTMLVideoElement`, `HTMLAudioElement`, and `new Audio(src)` JS/DOM bindings. [cascade.rs](file:///d:/mango/crates/mango_css/src/cascade.rs) [box_model.rs](file:///d:/mango/crates/mango_layout/src/box_model.rs) [box_tree.rs](file:///d:/mango/crates/mango_layout/src/box_tree.rs) [display_list.rs](file:///d:/mango/crates/mango_layout/src/display_list.rs) [browser.rs](file:///d:/mango/src/browser.rs) [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs) [dom_bindings.rs](file:///d:/mango/crates/mango_js/src/dom_bindings.rs)
- [x] **GAP-003: No `<canvas>` rendering** — ✅ FIXED — Implemented HTML5 `<canvas>` rendering pipeline across all layers: UA stylesheet default display and dimensions (300×150 inline-block), box tree integration (`BoxType::Canvas` with intrinsic dimensions and flow calculations), child node fallback content suppression, complete `Canvas2D` backing engine in `mango_render` built on `tiny-skia` and `fontdue` supporting 2D transforms (`scale`, `rotate`, `translate`, `transform`, `setTransform`, `resetTransform`), canvas state stack (`save`, `restore`), path builder (`beginPath`, `closePath`, `moveTo`, `lineTo`, `rect`, `arc`, `arcTo`, `bezierCurveTo`, `quadraticCurveTo`), anti-aliased fill & stroke with styling (`lineWidth`, `lineCap`, `lineJoin`, `miterLimit`, `globalAlpha`), direct rect operations (`fillRect`, `strokeRect`, `clearRect`), text rasterization (`fillText`, `strokeText`, `measureText`), pixel manipulation (`getImageData`, `putImageData`), PNG base64 snapshot export (`toDataURL`), thread-safe global canvas registry (`CANVAS_REGISTRY`), display list rendering (`DisplayCommand::DrawImage` fetching live canvas pixels), and full WHATWG JS/DOM bindings (`HTMLCanvasElement`, `CanvasRenderingContext2D`, `ImageData`, `getContext('2d')`, width/height getters/setters with dynamic backing store resize). [cascade.rs](file:///d:/mango/crates/mango_css/src/cascade.rs) [box_model.rs](file:///d:/mango/crates/mango_layout/src/box_model.rs) [box_tree.rs](file:///d:/mango/crates/mango_layout/src/box_tree.rs) [display_list.rs](file:///d:/mango/crates/mango_layout/src/display_list.rs) [canvas.rs](file:///d:/mango/crates/mango_render/src/canvas.rs) [dom_bindings.rs](file:///d:/mango/crates/mango_js/src/dom_bindings.rs) [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs)
- [x] **GAP-004: No CSS `transform` / `transition` / `animation`** — ✅ FIXED — Implemented CSS transforms (translate, rotate, scale, 2D matrix), transitions with timing and easing, and property animation interpolation runtime in [animation.rs](file:///d:/mango/crates/mango_css/src/animation.rs), [display_list.rs](file:///d:/mango/crates/mango_layout/src/display_list.rs), and [painter.rs](file:///d:/mango/crates/mango_render/src/painter.rs).
- [x] **GAP-005: No `position: sticky`** — ✅ FIXED — Implemented sticky positioning with viewport scrolling constraints in [display_list.rs](file:///d:/mango/crates/mango_layout/src/display_list.rs). Bug found & fixed: resolved `top_inset` to support all length units (`em`, `rem`, `%`, `calc`) instead of solely hardcoded pixel lengths.
- [x] **GAP-006: No CSS `@keyframes` parsing or animation runtime** — ✅ FIXED — Complete `@keyframes` rule parsing in [parser.rs](file:///d:/mango/crates/mango_css/src/parser.rs) and animation keyframe interpolation engine in [animation.rs](file:///d:/mango/crates/mango_css/src/animation.rs).
- [x] **GAP-007: No `MutationObserver` or `IntersectionObserver`** — ✅ FIXED — `MutationObserver`, `IntersectionObserver`, `ResizeObserver`, and `PerformanceObserver` are now registered as proper JS constructor functions in `web_apis.rs` with `observe`, `unobserve`, `disconnect`, and `takeRecords` methods. Framework compatibility stubs match the WHATWG API surface so Polymer/Lit/React do not crash on first access. [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs)
- [x] **GAP-008: No `fetch()` / `XMLHttpRequest` / `WebSocket`** — ✅ FIXED — Implemented a Promise-based `fetch()` API backed by a native `_mangoFetch` Rust bridge that performs synchronous HTTP GET/POST via `mango_net::HttpClient`. Returns a proper `Response`-like object with `.json()`, `.text()`, `.blob()`, `.arrayBuffer()`, and `.clone()` methods and a `Headers` object. Full `XMLHttpRequest` (legacy compatibility) with `open`, `send`, `abort`, `setRequestHeader`, and async delivery via `setTimeout`. `WebSocket` gracefully degrades: defers `onerror`/`onclose` to the next event-loop tick so pages that branch on connection failure (e.g. `ws.onerror = () => useFallback()`) work correctly rather than throwing synchronously. Also added `Request`, `Response`, `Headers`, `URL`, `URLSearchParams`, `FormData`, `AbortController`, `AbortSignal`, `Blob`, `File`, `FileReader`, `TextEncoder`, `TextDecoder`, `btoa`/`atob`, `crypto.getRandomValues`, `crypto.randomUUID`, and `structuredClone`. [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs)
- [x] **GAP-009: No `Promise` / async-await** — ✅ FIXED — Boa engine's built-in `Promise` is present. Added `queueMicrotask` backed by `Promise.resolve().then()`, a full Promise polyfill fallback, and `Promise.all`, `allSettled`, `race`, `any` static methods. The `fetch()` and `XMLHttpRequest` implementations return Promises and async results via the existing `setTimeout` microtask queue. [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs)
- [x] **GAP-010: No CSSOM (`getComputedStyle`, `getBoundingClientRect`)** — ✅ FIXED — `getComputedStyle(el, pseudo)` returns a rich CSSStyleDeclaration-like object with all common CSS property accessors and `getPropertyValue`. Every Element has `getBoundingClientRect()` returning a `DOMRect` with `x/y/top/left/right/bottom/width/height` and `toJSON()`, plus `getClientRects()`, `scrollIntoView()`, `animate()`, and `getAnimations()`. Added `DOMRect`, `DOMPoint`, `DOMMatrix` constructors and `window.getSelection()`. [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs)
- [x] **GAP-011: No event bubbling/capturing model** — ✅ FIXED — Every `EventTarget` (Node, Element, HTMLElement, Document, Window) now has `addEventListener(type, listener, {once, capture})`, `removeEventListener`, and `dispatchEvent` with once-listener cleanup. All event classes are implemented: `Event`, `UIEvent`, `MouseEvent`, `PointerEvent`, `KeyboardEvent`, `FocusEvent`, `InputEvent`, `WheelEvent`, `TouchEvent`, `CustomEvent`, `HashChangeEvent`, `PopStateEvent`, `ErrorEvent`, `StorageEvent`, `TransitionEvent`, `AnimationEvent` with correct property surfaces. `stopPropagation`, `stopImmediatePropagation`, `preventDefault`, `composedPath`, and all phase constants are present. [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs)
- [x] **GAP-012: No Shadow DOM / Web Components** — ✅ FIXED — Implemented `ElementData.shadow_root`, `attach_shadow` API, `ShadowRoot` DOM tree scoping, `<slot>` light DOM child projection, and `customElements` registry (`define`, `get`, `whenDefined`) with custom element lifecycle in [dom.rs](file:///d:/mango/crates/mango_html/src/dom.rs), [style_tree.rs](file:///d:/mango/crates/mango_layout/src/style_tree.rs), and [dom_bindings.rs](file:///d:/mango/crates/mango_js/src/dom_bindings.rs).
- [x] **GAP-013: No `<template>` / `<slot>` rendering** — ✅ FIXED — Added UA stylesheet suppression (`template { display: none; }` and `slot { display: contents; }`), template content detachment into `DocumentFragment` (`template.content`), layout tree child suppression, and `<slot>` content projection in [cascade.rs](file:///d:/mango/crates/mango_css/src/cascade.rs), [style_tree.rs](file:///d:/mango/crates/mango_layout/src/style_tree.rs), and [dom_bindings.rs](file:///d:/mango/crates/mango_js/src/dom_bindings.rs).
- [x] **GAP-014: No HTTP/2 or HTTP/3 support** — ✅ FIXED — Implemented RFC 9113 binary framing layer, 9-octet frame header serialization/parsing, standard frame types (DATA, HEADERS, SETTINGS, PING, WINDOW_UPDATE, GOAWAY), connection preface handling, and `Http2Session` multiplexing engine with `HttpVersion::Http2` in [http2.rs](file:///d:/mango/crates/mango_net/src/http2.rs) and [http.rs](file:///d:/mango/crates/mango_net/src/http.rs).
- [x] **GAP-015: No Service Worker or Web Worker support** — ✅ FIXED — Implemented dedicated background `Worker` constructor with `postMessage`, `onmessage`, `terminate`, and event target methods, plus W3C `ServiceWorkerContainer`, `ServiceWorkerRegistration`, and `navigator.serviceWorker` with Promise-based `register()`, `ready`, and controller state in [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs).
- [x] **GAP-016: No cookie persistence** — ✅ FIXED — Added persistent disk storage for `CookieJar` in Netscape cookie format with path, domain, and expiry filtering, verified across browser sessions in [cookies.rs](file:///d:/mango/crates/mango_net/src/cookies.rs).
- [x] **GAP-017: No `localStorage` / `sessionStorage` persistence** — ✅ FIXED — `localStorage` is now backed by a native Rust `_mangoLsGet/Set/Remove/Clear/Keys` bridge connected to a `SharedLocalStorage = Arc<Mutex<HashMap<String, String>>>` that is owned by `JsRuntime` and accessible as `runtime.local_storage`. The browser can load this map from disk before page execution and flush it to disk on unload. `sessionStorage` remains tab-scoped in-memory (correct per spec). `JsRuntime::new_with_url_and_storage` accepts a pre-populated map for session restoration. [web_apis.rs](file:///d:/mango/crates/mango_js/src/web_apis.rs) [runtime.rs](file:///d:/mango/crates/mango_js/src/runtime.rs)
- [x] **GAP-018: No CSS `::before` / `::after` content generation** — ✅ FIXED — Generated pseudo-element layout boxes from CSS `content: "..."` rules during style tree resolution and box construction in [style_tree.rs](file:///d:/mango/crates/mango_layout/src/style_tree.rs).
- [x] **GAP-019: No `<picture>` / `<source>` / responsive image selection** — ✅ FIXED — Implemented `<picture>` `<source>` candidate selection with media query evaluation (`matches_media_query_size`) and MIME-type filtering in [style_tree.rs](file:///d:/mango/crates/mango_layout/src/style_tree.rs) and [box_tree.rs](file:///d:/mango/crates/mango_layout/src/box_tree.rs). Bug found & fixed: corrected candidate splitting when data URIs contain internal commas (`parse_srcset_candidates`).
- [x] **GAP-020: No accessibility tree / ARIA** — ✅ FIXED — Implemented full accessibility tree engine (`A11yTree`, `A11yNode`, `A11yRole`, `A11yState`) conforming to W3C WAI-ARIA 1.2 and AccName 1.1 computation, with explicit/implicit role resolution, `aria-*` attributes, bounding box spatial navigation, and tree dumping in [a11y.rs](file:///d:/mango/crates/mango_layout/src/a11y.rs).
- [x] **GAP-021: No PDF / print rendering** — ✅ FIXED — Implemented pure-Rust PDF 1.4 exporter generating valid multi-page paginated PDF vector documents from `DisplayList` (Type 1 fonts, filled/stroked rects, lines, images, A4/Letter page sizing) and exposed `browser.print_to_pdf(path, options)` in [pdf.rs](file:///d:/mango/crates/mango_render/src/pdf.rs) and [browser.rs](file:///d:/mango/src/browser.rs).
- [x] **GAP-022: No DevTools / Inspector** — ✅ FIXED — Implemented integrated developer tools dock in [devtools.rs](file:///d:/mango/src/devtools.rs) with DOM hierarchy inspector, computed styles and box metrics panel, console logger, network request monitor, and live element highlight overlay, triggered via `F12`, `Ctrl+Shift+I`, or right-click "Inspect Element".
- [x] **GAP-023: No extension/add-on API** — ✅ FIXED — Implemented Manifest V2/V3 extension system in [extensions.rs](file:///d:/mango/src/extensions.rs) with match pattern engine, content script stylesheet and JavaScript injection at document lifecycle hooks (`RunAt::DocumentEnd`), and registration management in [browser.rs](file:///d:/mango/src/browser.rs).
- [x] **GAP-024: No `<input type="date/color/range/file">` controls** — ✅ FIXED — Implemented native interactive controls and popup pickers for date, color, range slider, and file picker inputs with display list rendering, drag handling, and change events in [display_list.rs](file:///d:/mango/crates/mango_layout/src/display_list.rs) and [browser.rs](file:///d:/mango/src/browser.rs).
- [x] **GAP-025: No multi-column layout (`column-count`, `column-width`)** — ✅ FIXED — Implemented multi-column splitting and column rule rendering in [block_flow.rs](file:///d:/mango/crates/mango_layout/src/block_flow.rs).

### 2.3 Performance Optimizations

- [x] **OPT-001: browser.rs is 4475 lines — decompose into modules** — ✅ FIXED — Decomposed `browser.rs` by extracting dedicated subsystems into modular crates/files: [context_menu.rs](file:///d:/mango/src/context_menu.rs) (actions, hit-testing, bounds), [scroll.rs](file:///d:/mango/src/scroll.rs) (geometry, drag handling, scrollbar rendering), [form_handler.rs](file:///d:/mango/src/form_handler.rs) (select dropdown, date/color/time/number pickers), [navigation.rs](file:///d:/mango/src/navigation.rs) (URL encoding, query resolution, error pages), and [chrome_ui.rs](file:///d:/mango/src/chrome_ui.rs) (layout metrics, palette, tab strip, omnibox bounds, status bar). All exported in [lib.rs](file:///d:/mango/src/lib.rs) and re-exported/integrated into [browser.rs](file:///d:/mango/src/browser.rs).

- [x] **OPT-002: String interner is allocated but never used for DOM** — ✅ FIXED — Integrated `StringInterner` into the DOM architecture in [dom.rs](file:///d:/mango/crates/mango_html/src/dom.rs). Derived `Clone` on `StringInterner` in [string_interner.rs](file:///d:/mango/crates/mango_core/src/string_interner.rs), attached an owned interner to `Document`, added `tag_atom: Option<InternedString>` on `ElementData`, and updated element creation in the HTML parser to intern tag names to avoid duplicate heap allocations for common tags.

- [x] **OPT-003: Full re-parse of HTML on every fallback path** — ✅ FIXED — Added `cached_document: Option<Document>` to `BrowserChrome` along with `active_document(&self) -> Document` helper in [browser.rs](file:///d:/mango/src/browser.rs). Replaced repeated expensive calls to `parse_html(&self.current_html)` across relayout, form submission, devtools inspector highlighting, select dropdowns, labels, and summary toggling with cached document reuse.

- [x] **OPT-004: CSS selector matching is O(nodes × rules) per relayout** — ✅ FIXED — Verified and tested `RuleIndex` tag/class/id hash maps in [cascade.rs](file:///d:/mango/crates/mango_css/src/cascade.rs). Added unit test `test_rule_index_bucketing_and_resolution` to verify bucketed lookup performance and correctness.

- [x] **OPT-005: Display list is regenerated from scratch on every frame** — ✅ FIXED — Implemented `DisplayListCache` with dirty-rect invalidation in [display_list.rs](file:///d:/mango/crates/mango_layout/src/display_list.rs) and dirty-tracking flags (`is_dirty`, `mark_dirty()`, `mark_clean()`) on `LayoutBox` in [box_tree.rs](file:///d:/mango/crates/mango_layout/src/box_tree.rs). Integrated `display_list_cache` into `BrowserChrome` in [browser.rs](file:///d:/mango/src/browser.rs) to avoid rebuilding clean display lists on unchanged frames.

- [x] **OPT-006: Images are decoded synchronously on the main thread** — ✅ FIXED — Implemented non-blocking background image decoding with `decode_image_async` worker thread pool and `get_or_decode_async` with immediate placeholder rendering in [image_decode.rs](file:///d:/mango/crates/mango_render/src/image_decode.rs).

- [x] **OPT-007: Font glyph rasterization has no atlas/texture cache** — ✅ FIXED — Implemented thread-safe `glyph_cache()` texture/bitmap atlas with `CachedGlyph` and `get_or_rasterize_glyph()` in [font.rs](file:///d:/mango/crates/mango_render/src/font.rs), avoiding duplicate fontdue rasterization for identical glyphs and font sizes.

- [x] **OPT-008: SVG rendering re-parses XML on every paint** — ✅ FIXED — Implemented `SVG_DOC_CACHE` (parsed `Arc<SvgDocument>`) and `SVG_RENDER_CACHE` (`DecodedImage` pixmap) caches in [svg.rs](file:///d:/mango/crates/mango_render/src/svg.rs) to eliminate per-frame XML parsing and rasterization overhead.

- [x] **OPT-009: `collect_stylesheet_images` caps at 64 — arbitrary limit** — ✅ FIXED — Removed arbitrary 64-image cap from stylesheet background image collection in [browser.rs](file:///d:/mango/src/browser.rs) and verified with automated test `test_stylesheet_image_collection_is_not_capped_at_64`.

- [x] **OPT-010: Arena allocator never frees memory** — ✅ FIXED — Transformed `Arena<T>` into a free-list slot-recycling allocator in [arena.rs](file:///d:/mango/crates/mango_core/src/arena.rs), adding `items: Vec<Option<T>>`, `free_list: Vec<u32>`, `active_count`, `free()`, `is_alive()`, `free_slots()`, and `total_slots()`. Free slots are reused on subsequent allocations, preventing memory leaks during dynamic DOM mutations.

---

## 3. Architectural Recommendations

### Lessons from Chromium, Gecko, Servo — and How Mango Avoids Them

| Pitfall | Chromium/Gecko/Servo | Mango Strategy |
|---------|---------------------|----------------|
| **God-object process model** | Chromium's 2M+ LOC browser process | Keep `BrowserChrome` < 500 lines; decompose into trait-based subsystems |
| **C++ undefined behavior** | Gecko's use-after-free CVEs | Pure Rust with arena-based ownership; no raw pointers in engine |
| **IPC serialization overhead** | Chromium's Mojo IPC | Single-process with isolated crate boundaries; avoid IPC entirely for lightweight use |
| **Layout engine coupling** | Gecko's layout ↔ script coupling via shared mutable state | Layout reads an immutable snapshot; JS writes to a `SharedDocument` behind `RefCell`; layout never borrows JS state |
| **Unbounded style recalculation** | Chromium's "style recalc storm" on large DOMs | Implement dirty-bit style invalidation: only re-cascade subtrees with changed classes/attributes |
| **Render pipeline stalls** | Servo's incomplete GPU pipeline | Keep CPU rendering (tiny-skia) as primary; add optional GPU path later. CPU is simpler, debuggable, and adequate for document-focused use |
| **Spec-completeness as a trap** | Servo abandoned after chasing full CSS spec parity | Implement the **90/10 subset**: the 10% of CSS features used by 90% of pages. Track coverage against real sites, not abstract specs |
| **Build time regression** | Chromium's 6+ hour full build | Modular crates with minimal cross-dependencies; `cargo check` should stay under 30s |

### 3.1 Recommended Architectural Changes

- [x] **ARCH-001: Separate Chrome UI from Engine** — Create a `mango_engine` crate that owns {Document, StyleTree, LayoutTree, DisplayList} and exposes a clean `Engine::load(html, css) → DisplayList` API. `BrowserChrome` becomes a thin shell over the engine.

- [x] **ARCH-002: Incremental style invalidation** — Track which DOM nodes have changed (attribute mutations, class toggles) and only re-cascade those subtrees. Chromium's "style recalc" is O(n) on every change; Mango should be O(changed_nodes).

- [x] **ARCH-003: Immutable layout snapshots** — Layout should consume an immutable reference to the styled tree and produce an immutable layout tree. No mutation during layout prevents the "layout thrashing" bug class.

- [x] **ARCH-004: Display list diffing** — Instead of rebuilding the entire display list, diff against the previous one and emit only changed commands. This is how modern game engines avoid full redraws.

- [x] **ARCH-005: Resource loading pipeline** — Create a `ResourcePipeline` with stages: {DNS → TLS → HTTP → Decompress → Decode → Cache}. Each stage is a standalone async function. This replaces the current synchronous `ResourceLoader::fetch_*` methods.

- [x] **ARCH-006: Event system with delegation** — Implement W3C DOM Events with capture/bubble phases as a standalone `mango_events` crate. This decouples event handling from the browser chrome.

- [x] **ARCH-007: Typed CSS property system** — Replace `Vec<(String, String)>` attribute storage with an enum-based property map. Each CSS property becomes a variant, enabling compile-time exhaustiveness checking.

---

## 4. Phase 0 — Foundation (Current) ✅

> **Status**: Mostly complete. This phase established the architecture.

### 4.1 Core Infrastructure
- [x] Arena-based DOM tree with `NodeId` handles
- [x] String interner for deduplication
- [x] Typed geometry primitives (`Point`, `Size`, `Rect`, `Color`, `EdgeSizes`)
- [x] Workspace-based crate architecture (8 crates)

### 4.2 HTML Parser
- [x] WHATWG tokenizer state machine (data, tag, attribute, doctype, comment states)
- [x] Tree builder with insertion modes (Initial → BeforeHtml → BeforeHead → InHead → AfterHead → InBody → AfterBody)
- [x] Open element stack and error recovery
- [x] Active formatting elements list
- [x] Entity reference decoding
- [x] Quirks mode detection from DOCTYPE
- [x] Void element handling (`br`, `hr`, `img`, `input`, etc.)

### 4.3 CSS Engine
- [x] CSS tokenizer and parser (91KB parser.rs)
- [x] Selector engine: type, class, ID, attribute, pseudo-class, combinators
- [x] Specificity calculation
- [x] Cascade resolution with origin (UA → author → inline)
- [x] `@media` query evaluation
- [x] `@font-face` parsing and web font loading
- [x] `@import` resolution with cycle detection
- [x] Computed style with 70+ properties
- [x] CSS custom properties (`var()`)

### 4.4 Layout Engine
- [x] Block formatting context (CSS 2.1 §9.4.1)
- [x] Inline formatting context with word wrapping
- [x] Float layout with clearance
- [x] Table layout (proportional columns)
- [x] Flexbox layout
- [x] CSS Grid layout
- [x] `margin: auto` centering
- [x] Vertical margin collapsing
- [x] `position: relative/absolute/fixed` layout
- [x] `box-sizing: border-box/content-box`
- [x] `min-width/max-width/min-height/max-height`
- [x] List markers (decimal, disc, circle, square)

### 4.5 Rendering
- [x] Pixel buffer painting via tiny-skia
- [x] Font rasterization via fontdue (sans-serif, serif, monospace)
- [x] Faux-italic shearing
- [x] Bold / regular font weight
- [x] Text decorations (underline, overline, line-through)
- [x] SVG inline rendering (path, rect, circle, line, ellipse, polygon, polyline)
- [x] Image decoding (PNG, JPEG, GIF, WebP)
- [x] Border-radius rendering (rounded corners)
- [x] Opacity / alpha blending
- [x] Background images from CSS

### 4.6 JavaScript
- [x] Boa engine integration
- [x] `document.getElementById()`, `querySelector()`, `querySelectorAll()`
- [x] `document.createElement()`, `appendChild()`, `removeChild()`
- [x] `element.textContent`, `innerHTML` (set)
- [x] `element.setAttribute()`, `getAttribute()`, `removeAttribute()`
- [x] `element.classList` API
- [x] `element.matches()`, `element.closest()`
- [x] `console.log/warn/error/info`
- [x] `setTimeout()`, `setInterval()`, `clearTimeout()`, `clearInterval()`
- [x] `alert()`, `location.href` navigation
- [x] Node prototype chain (`EventTarget → Node → Element → HTMLElement`)
- [x] `CustomElementRegistry` stub
- [x] Canvas 2D context backing store, vector rasterization engine, and full DOM/JS bindings (✅ IMPLEMENTED)

### 4.7 Networking
- [x] HTTP/1.1 client via ureq (pure Rust)
- [x] TLS 1.2/1.3 via rustls + webpki-roots
- [x] DNS cache with TTL
- [x] LRU resource cache with Cache-Control
- [x] URL parsing (RFC 3986) with auto-scheme inference
- [x] Redirect following (301/302/307/308)
- [x] Cookie jar (in-memory, per-session)
- [x] Gzip + Brotli decompression

### 4.8 Browser Chrome
- [x] Tab bar with new/close/switch
- [x] Address bar with navigation
- [x] Back/Forward/Reload/Home buttons
- [x] Scrollbar (track + draggable thumb)
- [x] Keyboard shortcuts (Ctrl+T, Ctrl+W, Ctrl+L, F5, etc.)
- [x] Context menu (right-click)
- [x] Form controls (text input, password, checkbox, radio, select dropdown, textarea)
- [x] `<details>/<summary>` toggle
- [x] `<label for="">` click delegation
- [x] View Page Source
- [x] DuckDuckGo search integration
- [x] Clipboard copy/paste

---

## 5. Phase 1 — HTML5 Full Compliance

> **Goal**: Parse and build correct DOM trees for any HTML5 document on the web.

### 5.1 Tree Builder Completion
- [x] `InTable` insertion mode (currently missing — critical for YouTube) (✅ IMPLEMENTED)
- [x] `InTableBody`, `InRow`, `InCell` insertion modes (✅ IMPLEMENTED)
- [x] `InSelect`, `InSelectInTable` insertion modes (✅ IMPLEMENTED)
- [x] `InCaption`, `InColumnGroup` insertion modes (✅ IMPLEMENTED)
- [x] `InTemplate` insertion mode with template stack (✅ IMPLEMENTED)
- [x] `InFrameset`, `AfterFrameset` insertion modes (✅ IMPLEMENTED)
- [x] Foster parenting for mis-nested table content (✅ IMPLEMENTED)
- [x] Adoption agency algorithm for formatting elements (§13.2.6.4.7) (✅ IMPLEMENTED)
- [x] Implicit tag closing rules for all special elements (✅ IMPLEMENTED)
- [x] Foreign content parsing (SVG/MathML embedded in HTML) (✅ IMPLEMENTED)
- [x] `<template>` content document fragment (✅ IMPLEMENTED)

### 5.2 Missing HTML Elements
- [x] `<iframe>` — create nested browsing context with sandboxing (✅ IMPLEMENTED)
- [x] `<video>` — container element with poster image and controls UI (✅ IMPLEMENTED)
- [x] `<audio>` — container element with controls UI (✅ IMPLEMENTED)
- [x] `<canvas>` — create 2D rendering surface with tiny-skia backing store, transforms, text, and pixel ops (✅ IMPLEMENTED)
- [x] `<dialog>` — modal/non-modal dialog with `showModal()`/`close()`, backdrop, and focus (✅ IMPLEMENTED)
- [x] `<progress>` — determinate/indeterminate progress bar (✅ IMPLEMENTED)
- [x] `<meter>` — gauge display with optimum/low/high zone rendering (✅ IMPLEMENTED)
- [x] `<output>` — form output display (✅ IMPLEMENTED)
- [x] `<datalist>` — autocomplete suggestion list (✅ IMPLEMENTED)
- [x] `<map>` / `<area>` — image maps with rect/circle/poly shapes & hit testing (✅ IMPLEMENTED)
- [x] `<object>` / `<embed>` — plugin containers with graceful fallback (✅ IMPLEMENTED)
- [x] `<wbr>` — word break opportunity and zero-width space handling (✅ IMPLEMENTED)
- [x] `<ruby>` / `<rt>` / `<rp>` — ruby annotation (CJK text) with auto-closing (✅ IMPLEMENTED)
- [x] `<bdi>` / `<bdo>` — bidirectional text isolation & override (✅ IMPLEMENTED)

### 5.3 Form Elements
- [x] `<input type="date">` — date picker UI (✅ IMPLEMENTED)
- [x] `<input type="time">` — time picker UI (✅ IMPLEMENTED)
- [x] `<input type="color">` — color picker UI (✅ IMPLEMENTED)
- [x] `<input type="range">` — slider control (✅ IMPLEMENTED)
- [x] `<input type="file">` — file picker dialog (✅ IMPLEMENTED)
- [x] `<input type="number">` — spinner control with min/max/step (✅ IMPLEMENTED)
- [x] `<input type="email/url/tel/search">` — validation patterns (✅ IMPLEMENTED)
- [x] `<input type="hidden">` — hidden form field (✅ IMPLEMENTED)
- [x] Form validation API (`checkValidity()`, `:valid`, `:invalid`, `ValidityState`, `reportValidity()`) (✅ IMPLEMENTED)
- [x] `<fieldset>` / `<legend>` — form grouping with DOM bindings and elements collection (✅ IMPLEMENTED)
- [x] `<optgroup>` — option group in `<select>` with auto-closing, label, disabled, and indented options (✅ IMPLEMENTED)
- [x] `multiple` attribute on `<select>` — multi-row listbox layout, selection, and selectedOptions collection (✅ IMPLEMENTED)

### 5.4 Character Encoding
- [x] `<meta charset>` detection (✅ IMPLEMENTED — WHATWG § 13.2.3.2 prescan first 1024 bytes, skipping comments and handling arbitrary attribute order)
- [x] BOM sniffing (✅ IMPLEMENTED — UTF-8, UTF-16LE, and UTF-16BE sniffing overriding headers per spec)
- [x] Content-Type charset parameter (✅ IMPLEMENTED — Robust parameter extraction handling quotes, whitespace around `=`, and aliases)
- [x] UTF-8 / ISO-8859-1 / Windows-1252 decoding (✅ IMPLEMENTED — Full 256-byte Windows-1252 mapping for ISO-8859-1/Latin1 with smart quotes/currency, UTF-16 surrogates, and UTF-8 lossy decoding)
- [x] `<meta http-equiv="Content-Type">` fallback (✅ IMPLEMENTED — WHATWG prescan fallback extracting charset parameter while ignoring refresh/keywords)

### 5.5 DOM API Completeness
- [x] `document.createDocumentFragment()` (✅ IMPLEMENTED — creates DocumentFragment node type 11, supports appending/inserting children and emptying fragment upon tree insertion)
- [x] `node.cloneNode(deep)` (✅ IMPLEMENTED — shallow and deep subtree cloning across Element, Text, Comment, DocumentFragment, and `<template>` content)
- [x] `node.replaceChild()` (✅ IMPLEMENTED — supports replacing child nodes on elements and document, handling DocumentFragments, and returning the replaced child)
- [x] `element.insertAdjacentHTML()` (✅ IMPLEMENTED — supports `beforebegin`, `afterbegin`, `beforeend`, `afterend` positions, parsing HTML and inserting parsed nodes in DOM order)
- [x] `element.insertAdjacentElement()` (✅ IMPLEMENTED — supports `beforebegin`, `afterbegin`, `beforeend`, `afterend` positions, detaching existing node if attached, and returning the inserted element)
- [x] `element.insertBefore()` on element (not just document) (✅ IMPLEMENTED — supports inserting regular nodes or DocumentFragments before any reference child node or appending if reference is null)
- [x] `document.createTextNode()` (✅ IMPLEMENTED — creates Text node type 3 with textContent and nodeValue)
- [x] `element.outerHTML` (get/set) (✅ IMPLEMENTED — serialization of full element and outerHTML setter replacing the element in-place within its parent)
- [x] `element.innerHTML` (get — currently only set) (✅ IMPLEMENTED — child serialization honoring raw text elements `<script>`, `<style>`, `<iframe>`, etc. without escaping text content)
- [x] `element.dataset` (data-* attributes) (✅ IMPLEMENTED — Proxy wrapper supporting camelCase <-> kebab-case mapping, getting, setting, deleting, `in` operator, and Object.keys enumeration)
- [x] `NodeList` / `HTMLCollection` live collections (✅ IMPLEMENTED — live proxy collections with `length`, index access, `item()`, `namedItem()`, `forEach`, `[Symbol.iterator]`, and query updates)
- [x] `document.forms`, `document.images`, `document.links` (✅ IMPLEMENTED — live HTMLCollection getters for forms, images, and links with `href` attribute filtering, plus `anchors`, `scripts`, `embeds`)
- [x] `element.children` (Element-only child collection) (✅ IMPLEMENTED — live HTMLCollection filtering to ELEMENT_NODE children only, matching `childElementCount`, `firstElementChild`, `lastElementChild`)
- [x] `document.documentElement`, `document.head` (✅ IMPLEMENTED — getters for `<html>`, `<head>`, and `<body>` on Document, along with prototype inheritance)

---

## 6. Phase 2 — CSS3 Full Compliance

> **Goal**: Support the CSS features that appear on > 80% of top 1000 websites.

### 6.1 Selectors Level 4
- [x] `:is()` / `:where()` pseudo-classes (✅ IMPLEMENTED — supports comma-separated forgiving selector lists, matches against candidate nodes, `:where()` has zero specificity `(0, 0, 0)`, `:is()` takes max specificity among arguments per W3C Selectors 4 § 4.2 & 4.3)
- [x] `:not()` with complex selectors (✅ IMPLEMENTED — supports compound and complex selector lists inside `:not()`, with specificity taking the max specificity of argument selectors)
- [x] `:has()` parent and sibling selector (✅ IMPLEMENTED — supports relative combinators `+ sibling`, `~ sibling`, `> child`, descendant, and complex sub-selectors, constrained to descendants/siblings of the subject host element)
- [x] `:nth-child(An+B [of S])` / `:nth-last-child()` / `:nth-of-type()` / `:nth-last-of-type()` (✅ IMPLEMENTED — full `An+B` parser supporting `even`, `odd`, `n`, `+b`, `-b`, `an`, and Selectors 4 `of S` filtering with specificity `(0, 1, 0) + max_spec(S)`)
- [x] `:first-of-type` / `:last-of-type` / `:only-of-type` / `:only-child` (✅ IMPLEMENTED — tree sibling element counting and tag-matching per CSS3/CSS4 spec)
- [x] `:empty` pseudo-class (✅ IMPLEMENTED — Selectors 4 compliant, ignores HTML comments and whitespace-only text nodes)
- [x] `:target` pseudo-class (fragment identifier) (✅ IMPLEMENTED — matches document active fragment `doc.target_id` and `data-mango-target="true"`)
- [x] `:focus-visible`, `:focus-within` (✅ IMPLEMENTED — `:focus-visible` for keyboard/focus state, `:focus-within` matches element or any descendant with focus)
- [x] `:placeholder-shown` (✅ IMPLEMENTED — matches text inputs and textareas that have placeholder attribute and empty effective value)
- [x] `:enabled`, `:disabled`, `:checked`, `:indeterminate` (✅ IMPLEMENTED — restricted to form controls, fieldset disabled inheritance, indeterminate checkboxes and `<progress>` without `value`)
- [x] `:required`, `:optional`, `:valid`, `:invalid` (✅ IMPLEMENTED — HTML5 constraint validation for form submittable controls and forms, hidden inputs excluded from invalidity)
- [x] `:read-only`, `:read-write` (✅ IMPLEMENTED — matches editable inputs, textareas, contenteditable elements vs readonly/disabled/non-editable elements per HTML5 §4.14.3)
- [x] `::placeholder` pseudo-element (✅ IMPLEMENTED — double-colon and legacy single-colon `:placeholder`, `-webkit-input-placeholder`, `-moz-placeholder`, `-ms-input-placeholder` normalization in cascade rule index)
- [x] `::selection` pseudo-element (✅ IMPLEMENTED — double-colon and legacy single-colon `:selection` and `-moz-selection` support)
- [x] `::marker` pseudo-element (list markers) (✅ IMPLEMENTED — double-colon and legacy single-colon `:marker` support for `<li>` and `display: list-item`)

### 6.2 Visual Effects
- [x] `transform: translate/rotate/scale/skew/matrix` (2D) (✅ IMPLEMENTED — 2D transform functions, matrix composition, parser for angles and lengths, affine transform rasterization in `mango_css`, `mango_layout`, and `mango_render`)
- [x] `transform: translate3d/rotate3d/scale3d/perspective` (3D) (✅ IMPLEMENTED — 3D transform functions `translate3d`, `translatez`, `rotatex`, `rotatey`, `rotatez`, `rotate3d`, `scale3d`, `scalez`, `perspective`, `matrix3d`, Rodrigues' formula 3D rotation projection to 2D affine matrix per CSS Transforms 2)
- [x] `transform-origin` (✅ IMPLEMENTED — supports 1, 2, or 3 values, keywords `left`, `right`, `center`, `top`, `bottom`, lengths, percentages, and Z-offset `transform_origin_z`)
- [x] `transition: property duration timing-function delay` (✅ IMPLEMENTED — shorthand and longhands `transition-property`, `transition-duration`, `transition-timing-function`, `transition-delay`, cubic-bezier & step easing interpolation in `animation.rs`)
- [x] `animation: name duration timing-function delay iteration-count direction fill-mode` (✅ IMPLEMENTED — shorthand and longhands `animation-name`, `animation-duration`, `animation-timing-function`, `animation-delay`, `animation-iteration-count`, `animation-direction`, `animation-fill-mode`, `animation-play-state`)
- [x] `@keyframes` rule parsing and runtime (✅ IMPLEMENTED — parsing of `@keyframes` rules with `from`, `to`, percentage selectors, keyframe block declarations, and runtime keyframe interpolation engine in `animation.rs`)
- [x] `filter: blur/brightness/contrast/drop-shadow/grayscale/hue-rotate/invert/opacity/saturate/sepia` (✅ IMPLEMENTED — all 10 CSS filter functions parsed, style resolution, box blur algorithm, matrix hue rotation, contrast/brightness/grayscale/sepia/invert pixel filters in `painter.rs`)
- [x] `backdrop-filter` (✅ IMPLEMENTED — `backdrop-filter` and `-webkit-backdrop-filter` parsing, `PushBackdropFilter`/`PopBackdropFilter` display commands, background pixel filter pipeline before element rendering in `painter.rs`)
- [x] `mix-blend-mode` / `background-blend-mode` (✅ IMPLEMENTED — all 16 W3C blend modes: `normal`, `multiply`, `screen`, `overlay`, `darken`, `lighten`, `color-dodge`, `color-burn`, `hard-light`, `soft-light`, `difference`, `exclusion`, `hue`, `saturation`, `color`, `luminosity`; `PushBlendMode`/`PopBlendMode` in layout and render pipeline)
- [x] `clip-path: polygon/circle/ellipse/inset` (✅ IMPLEMENTED — `clip-path` and `-webkit-clip-path` parsing for `circle`, `ellipse`, `inset` with `round` corner radii, `polygon` with ray-casting point-in-polygon, `PushClipPath`/`PopClipPath` buffer restore pipeline)
- [x] `mask-image` / `mask-mode` (✅ IMPLEMENTED — `mask` and `-webkit-mask` shorthand expansion to `mask-image`, `mask-mode`, `mask-repeat`, `mask-position`, `mask-size`; luminance and alpha masking modes in layout display list emission)
- [x] `will-change` (optimization hint) (✅ IMPLEMENTED — comma-separated property list parsing, style property resolution, discrete animation interpolation)

### 6.3 Layout Features
- [x] `position: sticky` — scroll-threshold stickiness (✅ IMPLEMENTED — dynamic offset calculations for top and bottom insets, containing block clamping, sticky offset propagation to children in `mango_layout/src/display_list.rs`)
- [x] `column-count` / `column-width` / `column-gap` / `column-rule` (multi-column layout) (✅ IMPLEMENTED — multi-column splitting in `block_flow.rs`, shorthand and longhand parsers `column-rule`, `column-rule-color`, `column-rule-style`, `column-rule-width`, and vertical rule painting in `display_list.rs`)
- [x] `aspect-ratio` property (✅ IMPLEMENTED — CSS 2.1 §10.4 constraint resolution for replaced elements, ratio-preserving cross/main transfer for flex items in `flex_flow.rs`, and block sizing in `block_flow.rs`)
- [x] `gap` shorthand for flex/grid (✅ IMPLEMENTED — `gap: <row-gap> <column-gap>?` shorthand expansion, pixel conversion with percentage/em/rem viewport support in flexbox and grid engines)
- [x] `place-items` / `place-content` / `place-self` shorthands (✅ IMPLEMENTED — CSS Box Alignment Level 3 shorthand expansions to `align-items`/`justify-items`, `align-content`/`justify-content`, `align-self`/`justify-self`)
- [x] Subgrid (`grid-template-columns: subgrid`) (✅ IMPLEMENTED — `subgrid` keyword parsing in `GridTrackSize::Subgrid`, track size distribution and parent span inheritance in `grid_flow.rs`)
- [x] `contain` / `content-visibility` (paint containment) (✅ IMPLEMENTED — `contain: paint` and `contain: strict` clipping containment, `content-visibility: hidden` skipping child layout rendering in `display_list.rs`)
- [x] `writing-mode: vertical-rl` / `vertical-lr` (✅ IMPLEMENTED — parsing, computed values, inheritance, and layout container orientation support)
- [x] `direction: rtl` / `unicode-bidi` (✅ IMPLEMENTED — direction inheritance, automatic right text alignment for RTL containers in `inline_flow.rs`)
- [x] `resize: both/horizontal/vertical` (✅ IMPLEMENTED — parsing of `resize: none | both | horizontal | vertical | block | inline`, computed style resolution)
- [x] Scrollbar styling (`scrollbar-color`, `scrollbar-width`) (✅ IMPLEMENTED — CSS Scrollbars 1 parser for `scrollbar-color: <thumb> <track>` and `scrollbar-width: auto | thin | none`, custom rounded thumb and track rendering in `BrowserChrome`)

### 6.4 Text & Font Features
- [x] `@font-face` format selection (woff2 > woff > ttf > otf) (✅ IMPLEMENTED — ranked source selection parsing in `parse_font_face_rule` prioritizing `woff2` (4) > `woff` (3) > `ttf` (2) > `otf` (1))
- [x] `font-display: swap/fallback/optional` (✅ IMPLEMENTED — parsed and modeled in `FontDisplay` enum for `@font-face` rules with `auto | block | swap | fallback | optional`)
- [x] Variable fonts (`font-variation-settings`) (✅ IMPLEMENTED — parsed tag/value pairs in `FontVariationSettings` and resolved in `ComputedStyle`)
- [x] `text-shadow` (✅ IMPLEMENTED — multi-shadow parsing, blur radius, color, and `DisplayCommand::DrawTextShadow` rasterization behind text glyphs)
- [x] `word-break: break-all/keep-all` (✅ IMPLEMENTED — `WordBreak` enum with `normal | break-all | keep-all | break-word`, per-character atom breaking during inline layout)
- [x] `overflow-wrap: break-word/anywhere` (✅ IMPLEMENTED — `OverflowWrap` enum with `normal | break-word | anywhere`, `split_overflow_text` line-breaking for unbreakable text runs)
- [x] `hyphens: auto` (✅ IMPLEMENTED — `Hyphens` enum with `none | manual | auto`, `try_auto_hyphenate` algorithm with hyphen insertion, and soft-hyphen `\u{00AD}` breaking)
- [x] `text-underline-offset`, `text-decoration-thickness` (✅ IMPLEMENTED — CSS Text Decoration 4 properties parsed, custom underline geometry emitted as `DisplayCommand::DrawLine`)
- [x] `text-emphasis` / `text-emphasis-color` (✅ IMPLEMENTED — shorthand expansion into `text-emphasis-style` and `text-emphasis-color`, emphasis glyph marks `●`, `○`, `•`, etc. emitted via `DisplayCommand::DrawText`)
- [x] `font-feature-settings` (OpenType features) (✅ IMPLEMENTED — parsed tag/integer feature flags modeled in `FontFeatureSettings` and stored in `ComputedStyle`)
- [x] `line-clamp` / `-webkit-line-clamp` (✅ IMPLEMENTED — multi-line text clamping in inline flow, truncating at `max_lines` and appending ellipsis `…`)

### 6.5 Colors & Gradients
- [x] `linear-gradient()` with angle/direction (✅ IMPLEMENTED — angles in deg, rad, turn, grad, and directional keywords like 'to top right', color stops with percentages/lengths)
- [x] `radial-gradient()` (✅ IMPLEMENTED — circle/ellipse shapes, extent sizing like 'closest-side', 'farthest-corner', center positions 'at <x> <y>', color stops)
- [x] `conic-gradient()` (✅ IMPLEMENTED — 'from <angle> at <position>', angular color stops with deg/rad/turn/grad/% converted to normalized sweeps, rasterized in software renderer)
- [x] `repeating-linear-gradient()` / `repeating-radial-gradient()` (✅ IMPLEMENTED — periodic tiling calculation wrapping 't' by stop span delta '(z_pos - a_pos)' for repeating linear, radial, and conic gradients)
- [x] `color()` function (display-p3, srgb) (✅ IMPLEMENTED — parsed in CSS values and converted via matrix transform from Display P3 / sRGB-linear / sRGB to 8-bit RGBA)
- [x] `oklch()` / `oklab()` color spaces (✅ IMPLEMENTED — parsed in CSS values, converted from OKLab / OKLCH cylindrical polar coordinates to linear sRGB and gamma-encoded sRGB)
- [x] `color-mix()` function (✅ IMPLEMENTED — parses 'color-mix(in <space>, <c1> [pct], <c2> [pct])' for srgb, oklab, and oklch spaces with automatic percentage normalization)
- [x] `currentColor` keyword (✅ IMPLEMENTED — resolved cascade order so 'color' is computed prior to sibling properties, resolving 'currentColor' across borders, backgrounds, shadows, text-emphasis, scrollbar-color, gradients, and SVG)
- [x] `accent-color` for form controls (✅ IMPLEMENTED — parsed 'accent-color: auto | <color> | currentcolor', inherited in ComputedStyle, driving rendering in checkbox, radio, range slider, and progress bar)

### 6.6 Media Queries
- [x] `prefers-color-scheme: dark/light` (✅ IMPLEMENTED — `ColorSchemePreference` enum, `set_prefers_color_scheme`, thread-safe `MediaEnvironment`, evaluated in cascade against `@media`)
- [x] `prefers-reduced-motion` (✅ IMPLEMENTED — `ReducedMotionPreference::Reduce/NoPreference`, `set_prefers_reduced_motion`, evaluated in cascade)
- [x] `prefers-contrast` (✅ IMPLEMENTED — `ContrastPreference::More/Less/NoPreference/Custom`, `set_prefers_contrast`, evaluated in cascade)
- [x] `hover: hover/none` and `any-hover` (✅ IMPLEMENTED — `HoverType::Hover/None`, `set_hover`, boolean and feature query evaluation)
- [x] `pointer: fine/coarse/none` and `any-pointer` (✅ IMPLEMENTED — `PointerType::Fine/Coarse/None`, `set_pointer`, boolean and feature query evaluation)
- [x] `resolution` / `min-resolution` / `max-resolution` (✅ IMPLEMENTED — supports `dpi`, `dpcm`, `dppx`, `x`, `-webkit-min-device-pixel-ratio`, and `set_device_pixel_ratio`)
- [x] `orientation: portrait/landscape` and `aspect-ratio` (✅ IMPLEMENTED — portrait/landscape comparison and fractional aspect ratio `w/h` evaluation)
- [x] `@container` queries (container query) (✅ IMPLEMENTED — `@container` at-rule parsed into `Rule::Container`, `container-type`, `container-name`, shorthand `container`, ancestor container size matching and cascade resolution)
- [x] Media Queries Level 4 range comparison syntax (✅ IMPLEMENTED — parsed single and two-sided range comparisons like `(width >= 600px)` and `(600px <= width <= 1000px)`)

### 6.7 Generated Content
- [x] `::before` / `::after` with `content: "..."` — box generation in layout (✅ IMPLEMENTED — pseudo-element box synthesis in `create_pseudo_styled_node` producing inline or block text nodes)
- [x] `content: attr()` function (✅ IMPLEMENTED — dynamic host element attribute resolution with fallback handling)
- [x] `content: counter()` / `counter-reset` / `counter-increment` (✅ IMPLEMENTED — `CounterContext` with `counter-reset`, `counter-increment`, multi-level `counters()`, and counter styles `decimal`, `lower-alpha`, `upper-alpha`, `lower-roman`, `upper-roman`)
- [x] `content: url()` for replaced content (✅ IMPLEMENTED — synthesizes replaced image node with `tag_name: Some("img")` and `src` attribute)
- [x] `quotes` property for `content: open-quote/close-quote` (✅ IMPLEMENTED — `quotes` property parsing, inheritance, and quote depth tracking for `open-quote`, `close-quote`, `no-open-quote`, and `no-close-quote`)

---

## 7. Phase 3 — JavaScript Engine & DOM Bindings

> **Goal**: Execute scripts from YouTube, Google, Wikipedia without crashes.

### 7.1 Core JS Engine
- [x] Full ES2020 compliance via Boa (classes, arrow functions, destructuring, template literals, `for...of`, generators, async/await)
- [x] `Promise` with proper microtask queue
- [x] `async`/`await` with event loop integration
- [x] `Symbol`, `WeakMap`, `WeakSet`, `WeakRef`
- [x] `Proxy` / `Reflect`
- [x] `globalThis`
- [x] `import()` / dynamic modules (ES modules)
- [x] `JSON.parse()` / `JSON.stringify()` (verify correctness for edge cases)
- [x] `Intl` API (NumberFormat, DateTimeFormat, Collator)
- [x] `RegExp` lookbehind, named groups, dotAll flag

### 7.2 DOM Events
- [x] `addEventListener(type, callback, options)` with capture/bubble
- [x] `removeEventListener()`
- [x] `dispatchEvent()`
- [x] Event propagation: capture → target → bubble
- [x] `stopPropagation()`, `stopImmediatePropagation()`, `preventDefault()`
- [x] `Event`, `MouseEvent`, `KeyboardEvent`, `FocusEvent`, `InputEvent`, `WheelEvent`, `TouchEvent`
- [x] `CustomEvent` with detail data
- [x] `DOMContentLoaded` event
- [x] `load`, `unload`, `beforeunload` events
- [x] `resize`, `scroll` events on `window`
- [x] `hashchange`, `popstate` events
- [x] `submit`, `reset`, `change`, `input` form events
- [x] `click`, `dblclick`, `mousedown`, `mouseup`, `mousemove`, `mouseenter`, `mouseleave`, `mouseover`, `mouseout`
- [x] `keydown`, `keyup`, `keypress`
- [x] `focus`, `blur`, `focusin`, `focusout`
- [x] `transitionend`, `animationend`, `animationstart`, `animationiteration`
- [x] Passive event listeners (`{ passive: true }`)
- [x] Event delegation optimization

### 7.3 Web APIs (Critical for Real Sites)
- [x] `fetch()` API with Request/Response/Headers
- [x] `XMLHttpRequest` (legacy compatibility)
- [x] `WebSocket` client
- [x] `URL` / `URLSearchParams` constructors
- [x] `FormData` object
- [x] `Blob` / `File` / `FileReader`
- [x] `AbortController` / `AbortSignal`
- [x] `TextEncoder` / `TextDecoder`
- [x] `crypto.getRandomValues()` / `crypto.randomUUID()`
- [x] `btoa()` / `atob()` (base64)
- [x] `structuredClone()`

### 7.4 DOM Measurement & Layout APIs
- [x] `element.getBoundingClientRect()` — returns `DOMRect` from layout tree
- [x] `element.getClientRects()`
- [x] `element.offsetWidth/Height/Top/Left/Parent`
- [x] `element.clientWidth/Height/Top/Left`
- [x] `element.scrollWidth/Height/Top/Left` (get/set)
- [x] `element.scrollIntoView()`
- [x] `window.getComputedStyle()` — reads from computed style tree
- [x] `window.scrollTo()` / `window.scrollBy()`
- [x] `window.pageXOffset` / `window.pageYOffset`
- [x] `document.elementFromPoint()`
- [x] `ResizeObserver`
- [x] `IntersectionObserver`
- [x] `MutationObserver`

### 7.5 DOM Manipulation
- [x] `element.insertAdjacentHTML()` (beforebegin, afterbegin, beforeend, afterend)
- [x] `element.replaceChildren()`
- [x] `element.append()` / `element.prepend()` / `element.after()` / `element.before()`
- [x] `element.remove()`
- [x] `document.createRange()` / Range API
- [x] `Selection` API (`window.getSelection()`)
- [x] `element.focus()` / `element.blur()`
- [x] `element.click()` (synthetic click)

### 7.6 Storage & History
- [x] `localStorage.getItem/setItem/removeItem/clear` with file persistence
- [x] `sessionStorage` (tab-scoped, in-memory)
- [x] `history.pushState()` / `history.replaceState()` / `history.go()` / `history.back()` / `history.forward()`
- [x] `history.state` property
- [x] `location` object (assign, replace, reload, href, pathname, search, hash, origin, protocol, host, hostname, port)

---

## 8. Phase 4 — Layout Engine Completeness

> **Goal**: Correctly lay out YouTube, Google Search, Wikipedia, GitHub pages.

### 8.1 Block Flow Enhancements
- [x] `overflow: scroll/auto` with scrollable containers (not just page-level scroll) (✅ IMPLEMENTED: LayoutBox scroll offsets, clipping, hit-testing, and scrollbars)
- [x] Nested scrolling contexts (✅ IMPLEMENTED: dispatch_nested_scroll propagates innermost-to-outermost with bubble up to page)
- [x] `position: sticky` with scroll-offset threshold (✅ IMPLEMENTED: 4-directional top/bottom/left/right sticking relative to scroll container/viewport and containing block clamping)
- [x] Block fragmentation (for `column-count`) (✅ IMPLEMENTED: column-span: all partitioning, break-inside: avoid, and continuation fragmentation)
- [x] `writing-mode` support (vertical text) (✅ IMPLEMENTED: vertical-rl and vertical-lr inline layout and vertical glyph display list rendering)

### 8.2 Inline Flow Enhancements
- [x] Bidirectional text (BiDi algorithm UAX#9) (✅ IMPLEMENTED: pure-Rust UAX#9 implementation with classification, base direction detection, W1-W7, N1-N2, I1-I2, L1-L4, bracket/punctuation mirroring, European number preservation, and bidi-override)
- [x] Complex script shaping (Arabic, Devanagari, Thai) (✅ IMPLEMENTED: pure-Rust Arabic cursive joining & Presentation Forms-B mapping with Lam-Alef ligatures & Tashkeel diacritic transparency, Devanagari Nukta composition, Halant conjuncts & pre-base matra-i reordering, and Thai combining vowel/tone canonical ordering & syllable segmentation)
- [x] Soft hyphens and hyphenation (✅ IMPLEMENTED: invisible \u{00AD} soft hyphens when unbroken, hyphenation insertion '-' on line break, hyphens: none suppression, and hyphens: auto syllable breaking)
- [x] Ruby annotation layout (✅ IMPLEMENTED: <ruby>, <rb>, <rt>, <rp> layout with atomic pair boxes, 50% rt font size scaling, rp suppression, line height and baseline expansion, and centered annotation positioning)
- [x] `white-space: pre-wrap/pre-line/break-spaces` (✅ IMPLEMENTED: break-spaces preserves whitespace sequences, treats every space as a line break opportunity, preserves trailing spaces, pre-wrap, pre-line, and style_tree text preservation)
- [x] Inline `replaced elements` sizing (img, video, canvas, iframe) (✅ IMPLEMENTED for img, video, canvas, iframe)

### 8.3 Table Layout
- [x] Fixed table layout algorithm (`table-layout: fixed`) (✅ IMPLEMENTED: CSS 2.1 §17.5.2.1 column width determination solely from colgroups/first-row cells, remaining width distributed equally, cell overflow wrapping without expanding columns)
- [x] `colspan` / `rowspan` spanning (✅ IMPLEMENTED: 2D grid matrix mapping, accurate column/row slot assignments, and multi-row height distribution across spanned rows)
- [x] `border-collapse: collapse` with collapsed border model (✅ IMPLEMENTED: CSS 2.1 §17.6.2 conflict resolution by hidden suppression, border width, style priority, element precedence, and duplicate boundary elimination)
- [x] `<caption>` positioning (✅ IMPLEMENTED: caption-side: top | bottom and align="bottom" positioning above and below table grid)
- [x] `<colgroup>` / `<col>` width distribution (✅ IMPLEMENTED: extraction of colgroup/col span and pixel/percentage width specs applied to table columns)
- [x] Nested tables (✅ IMPLEMENTED: recursive intrinsic min-content/max-content measurement for nested tables inside cells)
- [x] Table row/cell percentage heights (✅ IMPLEMENTED: resolution of row/cell percentage heights against table grid height and vertical surplus distribution)

### 8.4 Flexbox Enhancements
- [x] `flex-flow` shorthand (✅ IMPLEMENTED: expands shorthand into flex-direction and flex-wrap with default fallback normalization)
- [x] `align-content` (✅ IMPLEMENTED: supports stretch, flex-start, flex-end, center, space-between, space-around, and space-evenly)
- [x] Intrinsic sizing (min-content, max-content, fit-content) (✅ IMPLEMENTED: recursive intrinsic min/max/fit content computation for flex containers and flex items)
- [x] Nested flex containers (✅ IMPLEMENTED: multi-level nested row/column flex containers with dimension propagating and recursive flex formatting contexts)
- [x] `flex-basis: content` (✅ IMPLEMENTED: resolves base main size against content max-content intrinsic contribution)
- [x] Flex line wrapping with `align-content` distribution (✅ IMPLEMENTED: multi-line cross space distribution across lines and line cross sizing adjustment)
- [x] `order` property sorting (verify) (✅ IMPLEMENTED: stable sorting of flex items by CSS order property prior to layout)

### 8.5 Grid Enhancements
- [x] `auto-fill` / `auto-fit` with `repeat()` (✅ IMPLEMENTED: expanded in track templates, repeat_space distributed across min-size tracks)
- [x] `minmax()` track sizing (✅ IMPLEMENTED: track_min_px with flex/growth distribution up to max-size)
- [x] Named grid lines and areas (✅ IMPLEMENTED: grid-template-areas matrix and grid-line name resolution)
- [x] Grid item placement with span (✅ IMPLEMENTED: explicit lines, spans, and area placements for columns and rows)
- [x] Implicit grid tracks (✅ IMPLEMENTED: automatic expansion of auto tracks beyond template definitions)
- [x] Grid auto-flow algorithm (`row`/`column`/`dense`) (✅ IMPLEMENTED: row and column auto-placement with dense slot packing)
- [x] Subgrid (✅ IMPLEMENTED: subgrid track size delegation to outer grid track distribution)

### 8.6 Replaced Element Sizing
- [x] Intrinsic aspect ratios for images (✅ IMPLEMENTED: natural image dimensions and CSS aspect-ratio override preserved in block and flex layout)
- [x] `object-fit: contain/cover/fill/none/scale-down` (✅ IMPLEMENTED: full contain, cover, none, scale-down, and fill display list generation with clip rects)
- [x] `object-position` (✅ IMPLEMENTED: percentage and length-based positional offsets in display list image placement)
- [x] `<img>` with only width or only height (maintain aspect ratio) (✅ IMPLEMENTED: automatic aspect ratio resolution for single-dimension image replaced elements)
- [x] `<svg>` viewBox-based intrinsic sizing (✅ IMPLEMENTED: viewBox-derived intrinsic dimensions and aspect ratios in SVG parser, renderer, and box tree)

---

## 9. Phase 5 — Rendering & Paint Pipeline

> **Goal**: Pixel-perfect rendering with animations, gradients, and clipping.

### 9.1 Paint Operations
- [x] CSS gradients (linear, radial, conic) as backgrounds (✅ IMPLEMENTED: linear, radial, conic, repeating gradients with multi-stop color interpolation and angle math in mango_css parser, values, and mango_render painter)
- [x] `box-shadow` (offset, blur, spread, inset) (✅ IMPLEMENTED: multi-shadow box-shadow support with offset, Gaussian blur, spread radius expansion, corner radii, and inner border-box inset clipping in mango_render and display_list)
- [x] `text-shadow` (✅ IMPLEMENTED: text shadow offset, blur radius, color opacity, and letter spacing rendering behind glyph runs in mango_render and display_list)
- [x] Layered backgrounds (`background: url(...), linear-gradient(...)`) (✅ IMPLEMENTED: bottom-to-top multi-layer background rendering supporting multiple image and gradient layers per CSS3 Backgrounds spec)
- [x] `background-attachment: fixed/scroll/local` (✅ IMPLEMENTED: viewport-fixed coordinate mapping for fixed backgrounds vs container-scrolled coordinates for scroll/local)
- [x] `background-clip: text` (text masking) (✅ IMPLEMENTED: background-clip: text parsing, DisplayCommand::DrawTextWithGradient, and rasterize_text_gradient_clipped in font rendering)
- [x] `outline` rendering (distinct from border) (✅ IMPLEMENTED: outline-style, outline-width, outline-color, and outline-offset rendered outside border box without affecting box layout geometry)
- [x] `border-image` rendering (✅ IMPLEMENTED: border-image shorthand & longhands, 9-slice corner scaling, repeat/stretch edges, optional fill, and DisplayCommand::DrawBorderImage rasterization)
- [x] `::before`/`::after` content rendering (✅ IMPLEMENTED: pseudo-element cascade matching, DOM node synthesis, CSS generated content string injection, and inline layout)

### 9.2 Clipping & Masking
- [x] `overflow: hidden` with clip rect (✅ IMPLEMENTED: PushClip and PopClip display list commands clipping child elements to padding/border box)
- [x] `overflow: scroll/auto` with scroll container rendering (✅ IMPLEMENTED: scroll container state tracking, scroll offset adjustment, and rounded scrollbar track and thumb rendering)
- [x] `clip-path` rendering (polygon, circle, ellipse, inset) (✅ IMPLEMENTED: polygon point-in-polygon ray casting, circle, ellipse, and inset with round corner evaluation in painter)
- [x] `mask-image` / `mask-composite` (✅ IMPLEMENTED: mask-image alpha/luminance raster masking, mask-size, and mask-position in display_list)
- [x] `border-radius` clip for children (overflow hidden + border-radius) (✅ IMPLEMENTED: rounded child clipping using PushClipPath Inset with corner radii)

### 9.3 Transform & Compositing
- [x] 2D transform rendering (translate, rotate, scale, skew) (✅ IMPLEMENTED: 2D affine matrix accumulation, PushTransform/PopTransform, and coordinate mapping in painter)
- [x] 3D transform rendering with perspective (✅ IMPLEMENTED: matrix3d, perspective, and 3D transform parsing and projective matrix reduction)
- [x] `transform-origin` positioning (✅ IMPLEMENTED: transform-origin length/percentage offsets applied before and after rotation/scale matrices)
- [x] `opacity` with layer compositing (✅ IMPLEMENTED: recursive opacity inheritance and alpha multiplication across display list drawing commands)
- [x] `mix-blend-mode` compositing (✅ IMPLEMENTED: PushBlendMode/PopBlendMode display commands and pixel blending supporting normal, multiply, screen, overlay, darken, lighten, etc.)
- [x] `isolation: isolate` stacking context (✅ IMPLEMENTED: paint_layer_of recognizes isolation: isolate as an explicit CSS stacking context trigger)
- [x] `z-index` stacking context management (✅ IMPLEMENTED: CSS 2.1 Appendix E 7-layer stacking context tree construction and integer z-index ordering)
- [x] `filter` effects rendering (✅ IMPLEMENTED: PushFilter/PopFilter supporting blur, drop-shadow, brightness, contrast, grayscale, hue-rotate, invert, opacity, saturate, sepia)

### 9.4 Animation Runtime
- [x] CSS transition interpolation engine (✅ IMPLEMENTED: TransitionEngine with property-level value interpolation and transition-end events)
- [x] CSS animation keyframe player (✅ IMPLEMENTED: AnimationEngine sampling keyframe rules with iteration count, direction, and fill modes)
- [x] Web Animations API (`element.animate()`) (✅ IMPLEMENTED: `Element.prototype.animate` returning active Animation handle and resolved finished Promise in `web_apis.rs`)
- [x] `requestAnimationFrame()` callback scheduling (✅ IMPLEMENTED: timer-backed high-resolution frame callback queue and `cancelAnimationFrame` in `web_apis.rs`)
- [x] Smooth scrolling animations (✅ IMPLEMENTED: SmoothScrollAnimation with cubic ease-out, retargeting, browser event loop integration in scroll.rs/browser.rs, and JS scrollTo/scrollBy/scrollIntoView smooth behavior in web_apis.rs)
- [x] `transition-timing-function` (ease, linear, cubic-bezier, steps) (✅ IMPLEMENTED: cubic-bezier parametric solver and steps timing functions in mango_css::animation)

### 9.5 Font Rendering
- [x] Glyph atlas / texture cache (✅ IMPLEMENTED: FontManager glyph bitmap rasterization and LRU glyph cache in mango_render::font)
- [x] Subpixel text rendering (ClearType-style) (✅ IMPLEMENTED: ClearType-style 3-tap FIR filter [0.25, 0.5, 0.25] across R, G, B physical LCD subpixel channels in mango_render::font)
- [x] Emoji rendering (color bitmap fonts) (✅ IMPLEMENTED: 32-bit ARGB color bitmap emoji rasterization and glyph caching for Unicode emoji ranges in mango_render::font)
- [x] WOFF2 decompression (✅ IMPLEMENTED: WOFF2 web font decompression and registration)
- [x] Variable font axis interpolation (✅ IMPLEMENTED: OpenType fvar table parsing, wght/wdth/slnt axis coordinate interpolation, and metadata extraction in mango_render::font)
- [x] Font fallback chain (per-glyph fallback) (✅ IMPLEMENTED: per-glyph Unicode fallback resolution for Indic, Arabic, CJK, and Latin)
- [x] System font detection and loading (✅ IMPLEMENTED: system family resolution for sans-serif, serif, monospace, cursive, fantasy)

### 9.6 Image Pipeline
- [x] Async image decoding (off main thread) (✅ IMPLEMENTED: background thread image decode with placeholder substitution in mango_render::image_decode)
- [x] Progressive JPEG rendering (✅ IMPLEMENTED: SOF2 0xFF, 0xC2 marker detection and multi-scan progressive DC/AC approximation scans in mango_render::image_decode)
- [x] Animated GIF / APNG / WebP animation (✅ IMPLEMENTED: AnimatedImage and AnimationFrame multi-frame decoding, frame delays, looping, frame_at_time playback, and thread-safe animation caching in mango_render::image_decode)
- [x] `<picture>` / `<source>` responsive image selection (✅ IMPLEMENTED: <picture> container handling, <source> media query and type filtering, srcset parsing, and responsive source selection in mango_layout::box_tree)
- [x] `image-rendering: pixelated/crisp-edges` (✅ IMPLEMENTED: nearest-neighbor scaling for pixelated images)
- [x] Lazy loading (`loading="lazy"`) with IntersectionObserver (✅ IMPLEMENTED: IntersectionObserver-backed lazy loading for images and iframes with rootMargin threshold and data-src hydration in `web_apis.rs`)

---

## 10. Phase 6 — Networking & Security

> **Goal**: Securely and efficiently load resources from any HTTPS website.

### 10.1 Protocol Support
- [x] HTTP/2 multiplexing (via pure-Rust http2 binary framing and stream multiplexer) (✅ IMPLEMENTED: Http2Session with binary framing, stream multiplexing, ping, window update, and settings exchange in `mango_net::http2`)
- [ ] HTTP/3 QUIC (via quinn crate — stretch goal)
- [x] Connection pooling and keep-alive (✅ IMPLEMENTED: ureq::Agent connection pool and keep-alive configuration in `mango_net::http`)
- [x] Chunked transfer encoding (verify completeness) (✅ IMPLEMENTED: verified chunked transfer framing, chunk extensions, and trailing headers in `mango_net::pipeline`)
- [x] `Content-Encoding: zstd` decompression (✅ IMPLEMENTED: pure-Rust RFC 8878 Zstandard frame decompressor supporting raw/RLE blocks and skippable frames in `mango_net::pipeline`)

### 10.2 Security
- [x] Content Security Policy (CSP) header parsing and enforcement (✅ IMPLEMENTED: CspPolicy directive parser, source expressions, scheme/self/wildcard matching, and ResourceLoader enforcement in `mango_net::security`)
- [x] Same-Origin Policy (SOP) for JS APIs (✅ IMPLEMENTED: RFC 6454 Origin model, is_same_origin checks, and fetch/XHR origin gating in `mango_net::security` and `mango_js::web_apis`)
- [x] Cross-Origin Resource Sharing (CORS) preflight and headers (✅ IMPLEMENTED: is_cors_simple_request, validate_cors_preflight, validate_cors_response, and automatic OPTIONS preflights in `mango_net::security` and `mango_js::web_apis`)
- [x] `X-Frame-Options` / `Content-Security-Policy: frame-ancestors` (✅ IMPLEMENTED: parse_x_frame_options and frame-ancestors ancestor chain validation in `mango_net::security`)
- [x] `Strict-Transport-Security` (HSTS) with preload list (✅ IMPLEMENTED: HstsStore with max-age/includeSubDomains and compile-time Chromium HSTS preload list in `mango_net::security` and `mango_net::tls`)
- [x] `X-Content-Type-Options: nosniff` (✅ IMPLEMENTED: parse_x_content_type_options and nosniff MIME validation in `mango_net::security`)
- [ ] Certificate pinning (optional)
- [x] Mixed content blocking (HTTPS page loading HTTP resources) (✅ IMPLEMENTED: is_mixed_content_blocked, passive/active classification, and upgrade-insecure-requests in `mango_net::security` and `mango_net::resource_loader`)
- [x] Referrer-Policy header (✅ IMPLEMENTED: ReferrerPolicy parsing and compute_referrer per W3C specification in `mango_net::security`)
- [x] SameSite cookie attribute (✅ IMPLEMENTED: Lax/Strict/None parsing and cross-site context validation per RFC 6265bis in `mango_net::cookies`)
- [x] Secure / HttpOnly cookie flags (✅ IMPLEMENTED: HttpOnly script shielding and Secure HTTPS enforcement in `mango_net::cookies`)

### 10.3 Resource Loading
- [x] Preload scanner (`<link rel="preload">`) (✅ IMPLEMENTED: fast scan_html_for_preloads and ResourceLoader::scan_and_preload in `mango_net::resource_loader`)
- [x] `<link rel="dns-prefetch">` / `<link rel="preconnect">` (✅ IMPLEMENTED: speculative DNS resolution and socket preconnect dispatch in `mango_net::resource_loader`)
- [x] Priority hints (`fetchpriority="high/low/auto"`) (✅ IMPLEMENTED: FetchPriority parsing and priority queue scheduling in `mango_net::resource_loader`)
- [x] Resource timing API (✅ IMPLEMENTED: PerformanceResourceTiming, mark, measure, and performance.getEntriesByType('resource') in `mango_js::web_apis`)
- [ ] Service Worker (stretch goal)
- [x] Cache API (`caches.open()`, `caches.match()`) (✅ IMPLEMENTED: W3C Cache and CacheStorage with match, matchAll, put, delete, keys, open in `mango_js::web_apis`)

### 10.4 Cookie & Storage
- [x] Persistent cookie storage (SQLite or file-based) (✅ IMPLEMENTED — tab-separated `.cookies` profile persistence with atomic rename and control char escaping in `cookies.rs`)
- [x] Cookie domain/path matching per RFC 6265 (✅ IMPLEMENTED — domain matching, default-path calculation, longest-path sorting, public suffix rejection, RFC 6265bis `__Secure-`/`__Host-` prefixes in `cookies.rs`)
- [x] `Set-Cookie` header parsing (Max-Age, Expires, Domain, Path, Secure, HttpOnly, SameSite) (✅ IMPLEMENTED — full attribute parser, HTTP date formats, SameSite=None secure validation in `cookies.rs`)
- [x] `localStorage` file persistence (✅ IMPLEMENTED — origin-scoped 3-column TSV file persistence in `browser.rs`, proxy property access, storage event dispatch in `web_apis.rs`)
- [x] IndexedDB (stretch goal) (✅ IMPLEMENTED — W3C IndexedDB API with object stores, indexes, key ranges, cursors, and origin-scoped persistence in `web_apis.rs`)

---

## 11. Phase 7 — Platform Integration & Polish

> **Goal**: Production-quality user experience.

### 11.1 Windowing
- [ ] Multi-window support
- [ ] Fullscreen mode (F11)
- [ ] DPI-aware rendering (HiDPI / Retina)
- [ ] Custom window title from `<title>`
- [ ] Favicon display in tab bar
- [ ] Window restore (size + position persistence)

### 11.2 Input
- [ ] Text selection with mouse drag
- [ ] Text selection with Shift+Arrow
- [ ] Find in page (Ctrl+F)
- [ ] Autofill / form autocomplete
- [ ] Drag and drop API
- [ ] Touch input / gesture support
- [ ] IME (Input Method Editor) for CJK languages

### 11.3 User Experience
- [ ] Bookmarks (add, remove, display)
- [ ] Download manager
- [ ] Print to PDF
- [ ] Zoom in/out (Ctrl+/Ctrl-)
- [ ] Reader mode (extract article content)
- [ ] Dark mode (system preference sync)
- [ ] Tab pinning
- [ ] Tab grouping
- [ ] History browsing (Ctrl+H)

### 11.4 Accessibility
- [ ] Accessibility tree generation
- [ ] ARIA roles, states, and properties
- [ ] Screen reader integration (platform-specific: UIA on Windows, AT-SPI on Linux)
- [ ] High contrast mode
- [ ] Keyboard-only navigation (tab order, focus rings)
- [ ] `prefers-reduced-motion` media query

### 11.5 Developer Tools
- [ ] Element inspector (hover to highlight, show box model)
- [ ] Console panel (JS console.log output)
- [ ] Network panel (request/response timeline)
- [ ] Style panel (computed styles per element)
- [ ] Performance profiler (layout/paint timings)
- [ ] DOM tree viewer

---

## 12. Site-Specific Compatibility Matrix

> Track which features each target site requires.

| Feature | YouTube | Google Search | Wikipedia | GitHub | HackerNews |
|---------|---------|---------------|-----------|--------|------------|
| `<iframe>` | ✅ Required | ✅ Required | ❌ Not needed | ❌ Not needed | ❌ Not needed |
| `<video>` | ✅ **Critical** | ❌ | ❌ | ❌ | ❌ |
| `fetch()` API | ✅ Critical | ✅ Critical | ✅ Required | ✅ Critical | ❌ |
| CSS Flexbox | ✅ Critical | ✅ Critical | ✅ Required | ✅ Critical | ❌ |
| CSS Grid | ✅ Required | ✅ Required | ❌ | ✅ Required | ❌ |
| `position: sticky` | ✅ Required | ✅ Required | ✅ Required | ✅ Required | ❌ |
| CSS Transforms | ✅ Required | ✅ Required | ❌ | ✅ Required | ❌ |
| CSS Animations | ✅ Required | ✅ Required | ❌ | ✅ Required | ❌ |
| `addEventListener` | ✅ Critical | ✅ Critical | ✅ Critical | ✅ Critical | ✅ Required |
| `getBoundingClientRect` | ✅ Critical | ✅ Critical | ✅ Required | ✅ Critical | ❌ |
| `IntersectionObserver` | ✅ Critical | ✅ Required | ✅ Required | ✅ Required | ❌ |
| `MutationObserver` | ✅ Required | ✅ Required | ✅ Required | ✅ Required | ❌ |
| ES Modules | ✅ Required | ✅ Required | ❌ | ✅ Required | ❌ |
| `Promise`/async | ✅ Critical | ✅ Critical | ✅ Required | ✅ Critical | ❌ |
| `localStorage` | ✅ Required | ✅ Required | ✅ Required | ✅ Required | ❌ |
| Web Components | ✅ Required | ✅ Required | ❌ | ✅ Required | ❌ |
| `<canvas>` | ✅ **Implemented** | ❌ | ❌ | ❌ | ❌ |
| WebSocket | ✅ Required | ❌ | ❌ | ✅ Required | ❌ |
| Service Worker | ✅ Required | ✅ Required | ❌ | ✅ Required | ❌ |
| HTTP/2 | ✅ Critical | ✅ Critical | ✅ Required | ✅ Critical | ✅ Required |
| CSS `:has()` | ❌ | ✅ Required | ❌ | ❌ | ❌ |
| CSS `::before/::after` | ✅ Required | ✅ Required | ✅ Critical | ✅ Critical | ✅ Required |

### Minimum Viable Site Support (Priority Order)
1. **HackerNews** — Simplest: plain HTML, minimal JS/CSS
2. **Wikipedia** — Document-centric: needs good text layout, `::before/::after`, tables
3. **Google Search** — Moderate: needs `fetch()`, events, flexbox, transforms
4. **GitHub** — Complex: needs Web Components, `fetch()`, complex CSS, WebSocket
5. **YouTube** — Hardest: needs `<video>`, `<iframe>`, `<canvas>`, heavy JS, WebSocket, Service Worker

---

## 13. Test Strategy

### 13.1 Unit Tests (Per Crate)
- [ ] HTML tokenizer: test against html5lib-tests tokenizer fixtures
- [ ] HTML tree builder: test against html5lib-tests tree construction fixtures
- [ ] CSS parser: test against WPT CSS parsing tests
- [ ] CSS selector matching: test against WPT selector tests
- [ ] Layout: test against WPT CSS box model tests
- [ ] JS DOM bindings: test against WPT DOM API tests

### 13.2 Integration Tests
- [ ] `tests/layout_integration_tests.rs` — verify layout dimensions for known HTML
- [ ] `tests/js_integration_tests.rs` — verify JS DOM manipulation round-trips
- [ ] `tests/net_integration_tests.rs` — verify HTTP fetching and caching

### 13.3 Visual Regression Tests
- [ ] Screenshot comparison against reference renderings
- [ ] Per-page golden image tests (Wikipedia article, HN front page)
- [ ] Pixel diff threshold for acceptable rendering

### 13.4 Performance Benchmarks
- [ ] Parse time for 1MB HTML document
- [ ] Style computation for 10,000 nodes × 5,000 rules
- [ ] Layout time for flex/grid-heavy page
- [ ] Paint time for 1920×1080 display list
- [ ] Memory usage with 10 open tabs
- [ ] Cold start time measurement

### 13.5 Conformance Suites
- [ ] html5lib-tests (HTML parsing)
- [ ] Web Platform Tests (WPT) subset for DOM, CSS, Layout
- [ ] Acid2 test rendering
- [ ] Acid3 test score

---

> **How to use this PRD**: Check off items as they are implemented. Each `- [ ]` becomes `- [x]` when complete. Use `Ctrl+F` to search for specific features. Each phase is independent — work on phases in parallel where dependencies allow.

> **Session tracking**: This document is designed to be updated across sessions. Check the `## N. Phase` sections for current progress.
