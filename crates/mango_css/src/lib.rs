//! # mango_css
//!
//! CSS tokenizer, parser, selector matching, specificity calculation,
//! cascade, and computed style resolution for the Mango browser engine.

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


pub use cascade::{
    current_media_environment, find_container_size_for_node, matches_container_query,
    matches_media_query, matches_media_query_env, matches_media_query_size, resolve_cascade,
    resolve_cascade_with_index, resolve_pseudo_element_cascade,
    resolve_pseudo_element_cascade_with_index, set_device_pixel_ratio, set_hover,
    set_media_environment, set_pointer, set_prefers_color_scheme, set_prefers_contrast,
    set_prefers_reduced_motion, user_agent_rule_index, user_agent_stylesheet,
    ColorSchemePreference, ContrastPreference, HoverType, IndexedContainerRule, IndexedRule,
    MatchedDeclaration, MediaEnvironment, Origin, PointerType, ReducedMotionPreference, RuleIndex,
};
pub use computed::{
    compute_pseudo_style, compute_pseudo_style_with_index, compute_style,
    compute_style_with_index, ComputedStyle,
};
pub use animation::{
    collect_keyframes, interpolate_style, interpolate_value, AnimationEngine, AnimationEvent,
    AnimationEventKind, AnimationState, Keyframe, Keyframes, TransitionEngine, TransitionEvent,
};
pub use parser::{
    parse_declaration_list, parse_selectors, parse_stylesheet, ContainerRule, CssParser,
    FontFaceRule, KeyframeRule, KeyframesRule, MediaRule, Rule, StyleRule, Stylesheet,
};
pub use properties::Declaration;
pub use selectors::{
    AttributeOperator, Combinator, ComplexSelector, CompoundSelector, SelectorList, SimpleSelector,
};
pub use specificity::Specificity;
pub use tokenizer::{CssTokenizer, Token};
pub use values::{
    get_current_viewport, set_current_viewport,
    AlignContent, AlignItems, AlignSelf, Appearance, BorderCollapse, BorderStyle, BoxShadow, BoxSizing, CalcLength, CaptionSide, Clear, Contain,
    ContainerType, ContentItem, ContentVisibility, CounterAction, Display, Float, FontDisplay,
    FontFeatureSettings, FontStyle, FontVariationSettings, FontWeight, GridAutoFlow, GridPlacement, GridTrackSize,
    Hyphens, Length, LineClamp, OverflowWrap, Position, Resize, ScrollbarWidth, TableLayout, TextAlign,
    TextDecoration, TextDecorationThickness, TextEmphasisStyle, TextTransform, Value, WordBreak,
    WritingMode,
};
pub use property_id::{CssPropertyId, PropertyMap, CSS_PROPERTY_COUNT};
