//! CSS Flexible Box Layout Module Level 1 implementation.
//!
//! Handles flex container formatting: main-axis sizing, flex-line construction,
//! flex-grow / flex-shrink resolution, `justify-content` alignment, `align-items` / `align-self`
//! cross-axis positioning, and multi-line wrapping with gaps.

pub mod alignment;
pub mod container;
pub mod cross;
pub mod item;
pub mod lines;
pub mod metrics;
pub mod placement;
pub mod sizing;

pub use container::layout_flex;
