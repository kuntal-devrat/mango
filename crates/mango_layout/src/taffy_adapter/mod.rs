pub mod convert;
pub mod layout;

#[cfg(test)]
mod tests;

use crate::box_tree::LayoutBox;
use crate::dimensions::Dimensions;
use crate::float::FloatContext;

/// Computes CSS Flexbox layout using the Taffy engine.
/// Returns `true` if layout was computed successfully, or `false` to fall back.
pub fn compute_taffy_flex(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) -> bool {
    layout::execute_taffy_layout(container, containing_block, float_ctx, false)
}

/// Computes CSS Grid layout using the Taffy engine.
/// Returns `true` if layout was computed successfully, or `false` to fall back.
pub fn compute_taffy_grid(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) -> bool {
    layout::execute_taffy_layout(container, containing_block, float_ctx, true)
}

/// Computes CSS Flexbox or Grid layout on a container subtree using Taffy.
pub fn compute_taffy_flex_or_grid(
    container: &mut LayoutBox,
    containing_block: &Dimensions,
    float_ctx: &mut FloatContext,
) -> bool {
    let is_grid = container
        .style
        .as_ref()
        .map(|s| {
            matches!(
                s.display,
                mango_css::values::Display::Grid | mango_css::values::Display::InlineGrid
            )
        })
        .unwrap_or(false);
    layout::execute_taffy_layout(container, containing_block, float_ctx, is_grid)
}
