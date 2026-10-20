//! 2D Geometry primitives for the Mango browser engine.
//!
//! Provides point, size, rectangle, corner radii, and box-model edge sizes
//! used across parsing, CSS style resolution, layout formatting, and rasterization.

pub mod corner_radii;
pub mod edge_sizes;
pub mod point;
pub mod rect;
pub mod size;

pub use corner_radii::CornerRadii;
pub use edge_sizes::EdgeSizes;
pub use point::Point;
pub use rect::Rect;
pub use size::Size;
