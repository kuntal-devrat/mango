//! Style tree construction: associates DOM nodes with their computed CSS styles.
//!
//! Elements with `display: none` and non-rendered nodes (`<head>`, `<style>`, `<script>`)
//! are filtered out, producing a clean tree of rendered styled nodes.

pub mod builder;
pub mod cache;
pub mod counters;
pub mod extract;
pub mod invalidation;
pub mod node;
pub mod pseudo;

#[cfg(test)]
mod tests;

pub use builder::{
    build_style_tree, build_style_tree_with_cached_index, build_style_tree_with_env,
    build_style_tree_with_size,
};
pub use cache::PageStyleIndex;
pub use counters::CounterContext;
pub use extract::{
    collect_image_maps, extract_shadow_stylesheets, extract_style_elements, scan_all_shadow_roots,
    scan_and_register_global_svg_symbols, scan_and_register_image_maps,
};
pub use invalidation::StyleInvalidator;
pub use node::StyledNode;
pub use pseudo::collapse_whitespace;
