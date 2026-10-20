//! CSS transitions, `@keyframes` animations, and value interpolation.

pub mod engine;
pub mod events;
pub mod interpolation;
pub mod keyframes;
pub mod timeline;

#[cfg(test)]
mod tests;

pub use engine::{AnimationEngine, AnimationState, TransitionEngine};
pub use events::{AnimationEvent, AnimationEventKind, TransitionEvent};
pub use interpolation::{
    interpolate_color, interpolate_gradient, interpolate_length, interpolate_style,
    interpolate_transform, interpolate_value, lerp,
};
pub use keyframes::{
    Keyframe, Keyframes, animation_names, collect_keyframes, has_animations, has_transitions,
    property_differs,
};
pub use timeline::{ScrollTimeline, ViewTimeline};
