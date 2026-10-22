//! CSS cascade resolution, user-agent stylesheet defaults, and priority sorting.

pub(crate) mod hints;
pub mod media;
pub mod resolver;
pub mod rule_index;
pub mod types;
pub mod user_agent;

#[cfg(test)]
mod tests;

pub use media::{
    ColorSchemePreference, ContrastPreference, HoverType, MediaEnvironment, PointerType,
    ReducedMotionPreference, current_media_environment, matches_container_query,
    matches_media_query, matches_media_query_env, matches_media_query_size, set_device_pixel_ratio,
    set_hover, set_media_environment, set_pointer, set_prefers_color_scheme, set_prefers_contrast,
    set_prefers_reduced_motion,
};
pub use resolver::{
    find_container_size_for_node, resolve_cascade, resolve_cascade_with_index,
    resolve_cascade_with_shadow_index, resolve_pseudo_element_cascade,
    resolve_pseudo_element_cascade_with_index, resolve_pseudo_element_cascade_with_shadow_index,
    user_agent_rule_index,
};
pub use rule_index::{RuleIndex, extract_subject_keys_from_is_where};
pub use types::{
    IndexedContainerRule, IndexedRule, KeySelector, MatchedDeclaration, Origin, PseudoRuleBucket,
    RegisteredKeyframes, RegisteredProperty, compare_layer_paths,
};
pub use user_agent::user_agent_stylesheet;
