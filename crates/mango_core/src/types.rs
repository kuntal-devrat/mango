//! Shared core types used across the Mango browser engine.
//!
//! Provides fundamental geometry, color, and box model primitives.
//! All types are `Send + Sync`, `Copy`, and `Clone`, suitable for use in
//! parallel layout, style, and render passes.

pub use crate::color::Color;
pub use crate::geometry::{CornerRadii, EdgeSizes, Point, Rect, Size};

// Compile-time assertions: all core types must be Send + Sync.
const _: () = {
    #[allow(dead_code)]
    fn assert_send_sync<T: Send + Sync>() {}
    #[allow(dead_code)]
    fn assertions() {
        assert_send_sync::<Point>();
        assert_send_sync::<Size>();
        assert_send_sync::<Rect>();
        assert_send_sync::<Color>();
        assert_send_sync::<CornerRadii>();
        assert_send_sync::<EdgeSizes>();
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Point>();
        assert_send_sync::<Size>();
        assert_send_sync::<Rect>();
        assert_send_sync::<Color>();
        assert_send_sync::<CornerRadii>();
        assert_send_sync::<EdgeSizes>();
    }
}
