//! CSS shorthand expansion modules.

pub mod background;
pub mod border;
pub mod box_model;
pub mod flex_grid;
pub mod typography;

pub(crate) use background::expand_background_shorthand;
pub(crate) use border::{
    expand_border_sides, expand_column_rule_shorthand, expand_outline_shorthand,
};
pub(crate) use box_model::{
    expand_block_shorthand, expand_border_radius_shorthand, expand_four_sides,
    expand_four_sides_scalar, expand_inline_shorthand, expand_inset_block_shorthand,
    expand_inset_inline_shorthand, expand_inset_shorthand, expand_list_style_shorthand,
    expand_overflow_shorthand, expand_two_sides_scalar,
};
pub(crate) use flex_grid::{
    expand_flex_flow_shorthand, expand_flex_shorthand, expand_gap_shorthand,
    expand_grid_area_shorthand, expand_grid_placement_shorthand, expand_grid_template_shorthand,
    expand_place_shorthand,
};
pub(crate) use typography::{
    expand_columns_shorthand, expand_contain_intrinsic_size_shorthand, expand_container_shorthand,
    expand_font_shorthand, expand_mask_shorthand, expand_text_decoration_shorthand,
    expand_text_emphasis_shorthand,
};
