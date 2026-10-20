# Contributing to Mango Browser 🥭

Thank you for your interest in contributing to **Mango Browser**! Mango is an open-source, experimental web engine built from scratch in pure Rust. We are actively working towards standard web compliance and high rendering fidelity through small, incremental improvements.

---

## 🤖 AI Transparency Policy

Mango is developed through open, transparent **human and AI pair-programming**. We believe AI coding assistants are powerful tools for building complex systems when paired with human judgment, rigorous specifications, and automated testing.

### Guidelines for AI-Assisted Contributions:

1. **AI-assisted contributions are welcome**: You are welcome to use Claude, Cursor, ChatGPT, GitHub Copilot, Antigravity, or any other AI assistant to help research, write, refactor, and test code. Handwritten code is equally welcome.
2. **You own the code you submit**: 
   - You must personally read, understand, and verify every single line of code in your pull request.
   - You must be able to answer questions and explain the design decisions in code review.
   - **No blind copy-paste spam**: Submitting unreviewed, unverified AI output or hallucinated code is strictly prohibited. PRs that do not compile, break the test suite, or demonstrate a lack of understanding will be closed immediately.
3. **Be transparent**: We encourage contributors to note if a PR was AI-assisted in the description (e.g., `Built with assistance from Cursor / Claude 3.5 Sonnet`).

---

## 📐 PR Size & Modularity Rules

To keep Mango maintainable, reliable, and easy to review, all pull requests must follow these core constraints:

### 1. Small, Single-Purpose PRs
* **One concern per PR**: A PR should address exactly one bug fix, one CSS property, one Web API, or one refactoring task.
* **Keep diffs small**: Small PRs (under 300 lines of diff where possible) are reviewed, tested, and merged rapidly. Avoid bundling unrelated formatting, refactoring, and feature additions into a single PR.

### 2. The $\le 800$ Lines per File Rule
* All Rust (`.rs`) and JavaScript (`.js`) source files in `crates/` and `src/` must stay **strictly under 800 lines**.
* If a module approaches 800 lines, split it into a directory with focused sub-modules (e.g. `block_flow/intrinsic.rs`, `block_flow/positioned.rs`, etc.). Do not let files grow into unmaintainable monoliths.

### 3. Zero Warnings & Full Test Coverage
* **Zero compiler / clippy warnings**: Code must compile cleanly with `cargo clippy --workspace --all-targets -- -D warnings`.
* **Zero test regressions**: The full test suite must pass 100% on `cargo test --workspace`.
* **Add tests for every change**:
  - Bug fixes must include a regression test proving the bug existed and is now resolved.
  - New features must include unit tests and/or Web Platform Tests.

---

## 🛠️ Development Setup & Quality Checks

### 1. Prerequisites

* **Rust toolchain** (1.75+):
  ```bash
  rustup update stable
  rustup component add clippy rustfmt
  ```
* **Linux packages** (for desktop windowing):
  ```bash
  sudo apt-get install -y pkg-config libx11-dev libasound2-dev libfontconfig1-dev
  ```

### 2. Verifying Locally Before Submitting

Always run the following commands locally before opening a pull request:

```bash
# 1. Format code (must produce no diffs)
cargo fmt --all -- --check

# 2. Check for clippy warnings (zero warnings allowed)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Run all workspace unit and integration tests
cargo test --workspace

# 4. Run official Web Platform Tests
cargo test --test wpt_official_runner
```

---

## 🔍 Finding & Reporting Issues

If you want to contribute but don't know where to start:

1. **Check the Starter Issues**: Browse the [GitHub Issues](https://github.com/kuntal-devrat/mango/issues) for 10 curated starter issues tagged `good first issue` across layout, CSS, rendering, and JavaScript with exact file paths and test steps.
2. **Check the Issue Tracker**: Look for issues tagged `good first issue`, `rendering-parity`, or `spec-compliance`.
3. **Find a Rendering Bug using Headless**:
   You can render any URL using Mango's headless renderer and compare it against Chromium:
   ```bash
   cargo run --release --bin headless -- "https://example.com" mango_render.png 1280 900 --page
   ```
   If you spot a visual discrepancy (misaligned button, incorrect font size, wrapping text), open an issue with:
   - The URL
   - Mango screenshot vs. Chrome screenshot
   - The element's CSS selector (use `--probe "<selector>" 1280 900` to inspect its computed style)
3. **Submit an Incremental Fix**:
   Trace the issue to the relevant crate (`mango_css` for styles, `mango_layout` for box geometry, `mango_render` for rasterization, or `mango_js` for DOM APIs), fix the logic, add a test, and open a small PR.

---

## 📝 Commit Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/):

* `fix(layout)`: Fix vertical centering on input buttons
* `feat(css)`: Add support for `aspect-ratio` property
* `test(wpt)`: Add test coverage for flexbox nested items
* `refactor(render)`: Modularize SVG path parser to obey 800 lines rule

Thank you for helping make Mango a fast, lightweight, and compliant browser engine!
