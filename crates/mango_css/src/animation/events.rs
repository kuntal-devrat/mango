//! Animation and transition lifecycle events.

use mango_html::dom::NodeId;

/// A lifecycle event produced by the animation subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationEventKind {
    Start,
    End,
    Iteration,
    Cancel,
}

/// An event emitted by [`AnimationEngine`] (maps to CSS `animationstart`/`animationend`).
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationEvent {
    pub node: NodeId,
    pub name: String,
    pub kind: AnimationEventKind,
}

/// An event emitted by the transition engine (`transitionend` / `transitioncancel`).
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionEvent {
    pub node: NodeId,
    pub property: String,
    pub elapsed_ms: f32,
}
