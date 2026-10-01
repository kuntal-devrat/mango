# Contributing to Mango Browser 🥭

Thank you for your interest in contributing to **Mango Browser**! Mango is built from the ground up in pure Rust to deliver an ultra-fast, lightweight, memory-efficient web engine that treats the web as a document platform.

---

## Code of Conduct

All contributors and participants are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please report unacceptable behavior through GitHub moderation or private reporting.

---

## Development Setup

### Prerequisites

- **Rust**: Latest stable Rust toolchain (1.75+ recommended). Install via [rustup](https://rustup.rs/):
  ```bash
  rustup update stable
  rustup component add clippy rustfmt
  ```
- **OS Dependencies** (only needed for GUI windowing on Linux):
  ```bash
  # Debian / Ubuntu
  sudo apt-get install -y pkg-config libx11-dev libasound2-dev libfontconfig1-dev
  ```

---

## Workspace Structure

Mango is structured as a Cargo workspace partitioned into 10 focused crates:

| Crate | Responsibilities |
|---|---|
| [`mango_core`](crates/mango_core) | Fundamental types (`Color`, `Rect`, `Point`, `Size`), memory arena allocators, string interning |
| [`mango_html`](crates/mango_html) | WHATWG HTML5 tokenization, tree building, entity decoding, DOM tree representation |
| [`mango_css`](crates/mango_css) | CSS3 tokenizer, parser, selectors (Level 4), specificity, cascade, media/container queries |
| [`mango_layout`](crates/mango_layout) | Box tree generation, block/inline flow, flexbox, CSS grid, table layout, accessibility tree |
| [`mango_render`](crates/mango_render) | Display list commands, 2D software rasterizer, font shaping/fallback, SVG renderer, Canvas 2D, PDF export |
| [`mango_net`](crates/mango_net) | HTTP/1.1 & HTTP/2 client, TLS, DNS resolver, cookie store, CSP/HSTS/CORS security, resource cache |
| [`mango_js`](crates/mango_js) | JavaScript engine (powered by Boa), DOM bindings, Web Platform APIs, microtask event loop |
| [`mango_events`](crates/mango_events) | W3C DOM Event dispatch, event target tree, capture and bubbling propagation phases |
| [`mango_engine`](crates/mango_engine) | Engine orchestration, document lifecycle, incremental restyle, relayout scheduling |
| [`mango_platform`](crates/mango_platform) | Platform abstractions: windowing, input events, clipboard integration |

Binaries:
- `mango`: Interactive desktop web browser with tabs, navigation bar, dev tools, and history.
- `headless`: Headless webpage renderer and screenshot tool for CLI automation and visual regression testing.

---

## Building and Running

### Build

```bash
# Debug build
cargo build

# Optimized release build
cargo build --release
```

### Running Interactive Browser

```bash
cargo run --bin mango
```

### Running Headless Renderer

```bash
# Render a webpage directly to an image file (e.g., 1280x800 page viewport)
cargo run --bin headless -- "https://www.rust-lang.org" rust.png 1280 800 --page
```

---

## Testing & Quality Assurance

### Running Tests

We maintain a 100% test pass rate across all crates:

```bash
# Run all workspace unit, integration, and spec parity tests
cargo test --workspace

# Run layout integration tests
cargo test -p mango_layout

# Run JavaScript integration tests
cargo test --test js_integration_tests

# Run Web Platform spec parity tests
cargo test --test spec_parity_tests
```

### Formatting & Linting

Before opening a pull request, ensure code adheres to standard Rust style:

```bash
# Check code formatting
cargo fmt --all -- --check

# Check clippy lints
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Pull Request Guidelines

1. **Create a Topic Branch**: Fork the repo and create a branch like `feature/css-subgrid` or `fix/flex-stretch`.
2. **Atomic Commits**: Write descriptive commit messages explaining *why* a change was made.
3. **Add Tests**: Every bug fix or new feature must include a corresponding unit or integration test.
4. **Zero Regressions**: Ensure `cargo test --workspace` passes cleanly.
5. **Open a PR**: Fill out the provided [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md).
