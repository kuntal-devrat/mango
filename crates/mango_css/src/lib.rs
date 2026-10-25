//! # mango_css
//!
//! CSS tokenizer, parser, selector matching, specificity calculation,
//! cascade, and computed style resolution for the Mango browser engine.

#![allow(
    clippy::collapsible_if,
    clippy::excessive_precision,
    clippy::too_many_arguments,
    clippy::needless_range_loop
)]

pub mod animation;
pub mod cascade;
pub mod computed;
pub mod parser;
pub mod properties;
pub mod property_id;
pub mod selectors;
pub mod specificity;
pub mod tokenizer;
pub mod values;

pub use animation::{
    AnimationEngine, AnimationEvent, AnimationEventKind, AnimationState, Keyframe, Keyframes,
    TransitionEngine, TransitionEvent, collect_keyframes, interpolate_style, interpolate_value,
};
pub use cascade::{
    ColorSchemePreference, ContrastPreference, HoverType, IndexedContainerRule, IndexedRule,
    MatchedDeclaration, MediaEnvironment, Origin, PointerType, ReducedMotionPreference,
    RegisteredKeyframes, RegisteredProperty, RuleIndex, compare_layer_paths,
    current_media_environment, find_container_size_for_node, matches_container_query,
    matches_media_query, matches_media_query_env, matches_media_query_size, resolve_cascade,
    resolve_cascade_with_index, resolve_cascade_with_shadow_index,
    resolve_cascade_with_shadow_index_details, resolve_pseudo_element_cascade,
    resolve_pseudo_element_cascade_with_index, resolve_pseudo_element_cascade_with_shadow_index,
    set_device_pixel_ratio, set_hover, set_media_environment, set_pointer,
    set_prefers_color_scheme, set_prefers_contrast, set_prefers_reduced_motion,
    user_agent_rule_index, user_agent_stylesheet,
};
pub use computed::{
    ComputedStyle, compute_pseudo_style, compute_pseudo_style_with_index,
    compute_pseudo_style_with_shadow_index, compute_style, compute_style_with_index,
    compute_style_with_shadow_and_part_index, compute_style_with_shadow_and_part_indexes,
    compute_style_with_shadow_index,
};
pub use parser::{
    ContainerRule, CssParser, FontFaceRule, KeyframeRule, KeyframesRule, LayerRule, MediaRule,
    PropertyRule, Rule, StyleRule, Stylesheet, parse_declaration_list, parse_selectors,
    parse_stylesheet,
};
pub use properties::{Declaration, is_valid_property_value};
pub use property_id::{CSS_PROPERTY_COUNT, CssPropertyId, PropertyMap};

/// Evaluates a CSS `@supports` condition or property-value pair.
pub fn supports(prop_or_condition: &str, value: Option<&str>) -> bool {
    if let Some(val) = value {
        let trimmed_prop = prop_or_condition.trim();
        let trimmed_val = val.trim();
        if trimmed_prop.starts_with("--") {
            return !trimmed_val.is_empty();
        }
        let decls = parse_declaration_list(&format!("{trimmed_prop}: {trimmed_val}"));
        !decls.is_empty()
    } else {
        let cond = prop_or_condition.trim();
        let cond_with_parens = if cond.starts_with('(') {
            cond.to_string()
        } else {
            format!("({cond})")
        };
        let tokens = CssTokenizer::tokenize(&cond_with_parens);
        let parser = CssParser::new(tokens.clone());
        parser.eval_supports_condition(&tokens)
    }
}
pub use selectors::{
    AttributeOperator, Combinator, ComplexSelector, CompoundSelector, SelectorList, SimpleSelector,
    current_host_element, current_scope_root, expand_nested_selectors, set_current_host_element,
    set_current_scope_root,
};
pub use specificity::Specificity;
pub use tokenizer::{CssTokenizer, Token};
pub use values::{
    AlignContent, AlignItems, AlignSelf, Appearance, BorderCollapse, BorderStyle, BoxShadow,
    BoxSizing, CalcLength, CaptionSide, Clear, Contain, ContainerType, ContentItem,
    ContentVisibility, CounterAction, Display, Float, FontDisplay, FontFeatureSettings, FontStyle,
    FontVariationSettings, FontWeight, GridAutoFlow, GridPlacement, GridTrackSize, Hyphens,
    JustifyContent, JustifyItems, JustifySelf, Length, LineClamp, OverflowWrap, Position, Resize,
    ScrollbarWidth, TableLayout, TextAlign, TextDecoration, TextDecorationLine,
    TextDecorationStyle, TextDecorationThickness, TextEmphasisStyle, TextTransform, Value,
    WordBreak, WritingMode, get_current_viewport, set_current_viewport,
};

// ── Compile-time thread-safety assertions ──────────────────────────────
// Parallel style resolution requires all core types to be Send + Sync.
// If any type accidentally gains a non-Send/Sync field, these assertions
// will fail at compile time with a clear error.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<ComputedStyle>();
    assert_send_sync::<Stylesheet>();
    assert_send_sync::<Declaration>();
    assert_send_sync::<SelectorList>();
    assert_send_sync::<Specificity>();
    assert_send_sync::<Value>();
    assert_send_sync::<CssPropertyId>();
    assert_send_sync::<PropertyMap>();
    assert_send_sync::<MediaEnvironment>();
    assert_send_sync::<MatchedDeclaration>();
    assert_send_sync::<RuleIndex>();
    assert_send_sync::<AnimationEngine>();
    assert_send_sync::<TransitionEngine>();
    assert_send_sync::<Keyframes>();
    assert_send_sync::<crate::values::MathExpr>();
    assert_send_sync::<crate::values::BorderImage>();
};
