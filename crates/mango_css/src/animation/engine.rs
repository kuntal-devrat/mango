//! AnimationEngine, TransitionEngine, and combined AnimationState.

use std::collections::HashMap;

use mango_html::dom::NodeId;

use crate::computed::{ComputedStyle, apply_cascaded_properties};
use crate::parser::Stylesheet;
use crate::values::{
    Animation, AnimationDirection, AnimationFillMode, AnimationIterationCount, AnimationPlayState,
    TimingFunction,
};

use super::events::{AnimationEvent, AnimationEventKind, TransitionEvent};
use super::interpolation::interpolate_style;
use super::keyframes::{Keyframes, collect_keyframes, property_differs};
use super::timeline::{ScrollTimeline, ViewTimeline};

#[derive(Debug, Clone)]
struct ActiveAnimation {
    node: NodeId,
    spec: Animation,
    frames: Keyframes,
    /// Time in ms since this animation started (including any pending delay).
    elapsed_ms: f32,
    /// Completed iterations reported so far (for `animationiteration`).
    iterations_done: f32,
    started: bool,
    finished: bool,
}

/// Tracks and advances running CSS animations for the whole document.
#[derive(Debug, Default)]
pub struct AnimationEngine {
    active: Vec<ActiveAnimation>,
    keyframes: HashMap<String, Keyframes>,
    events: Vec<AnimationEvent>,
    pub scroll_timelines: HashMap<String, ScrollTimeline>,
    pub view_timelines: HashMap<String, ViewTimeline>,
    pub node_timelines: HashMap<NodeId, String>,
}

impl AnimationEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the known `@keyframes` definitions (call once per page load).
    pub fn set_keyframes(&mut self, keyframes: HashMap<String, Keyframes>) {
        self.keyframes = keyframes;
    }

    pub fn keyframes(&self) -> &HashMap<String, Keyframes> {
        &self.keyframes
    }

    /// Number of currently running animations.
    pub fn running_count(&self) -> usize {
        self.active.iter().filter(|a| !a.finished).count()
    }

    /// Returns true while at least one animation still needs frames.
    pub fn is_animating(&self) -> bool {
        self.active.iter().any(|a| !a.finished)
    }

    pub fn set_scroll_timeline(&mut self, timeline: ScrollTimeline) {
        self.scroll_timelines
            .insert(timeline.name.clone(), timeline);
    }

    pub fn set_view_timeline(&mut self, timeline: ViewTimeline) {
        self.view_timelines.insert(timeline.name.clone(), timeline);
    }

    pub fn update_scroll_timeline(&mut self, name: &str, current_px: f32, max_px: f32) {
        if let Some(tl) = self.scroll_timelines.get_mut(name) {
            tl.current_scroll_px = current_px;
            tl.max_scroll_px = max_px;
        }
    }

    pub fn update_view_timeline(&mut self, name: &str, entry_ratio: f32) {
        if let Some(tl) = self.view_timelines.get_mut(name) {
            tl.entry_ratio = entry_ratio;
        }
    }

    pub fn bind_node_timeline(&mut self, node: NodeId, timeline_name: &str) {
        self.node_timelines.insert(node, timeline_name.to_string());
    }

    pub fn timeline_progress(&self, name: &str) -> Option<f32> {
        if let Some(tl) = self.scroll_timelines.get(name) {
            Some(tl.progress())
        } else {
            self.view_timelines.get(name).map(|vl| vl.progress())
        }
    }

    pub fn start_with_timeline(
        &mut self,
        node: NodeId,
        anim_name: &str,
        timeline_name: &str,
    ) -> bool {
        let spec = Animation {
            name: anim_name.to_string(),
            timing: TimingFunction::Linear,
            timeline: Some(timeline_name.to_string()),
            ..Default::default()
        };
        self.bind_node_timeline(node, timeline_name);
        self.start(node, &spec)
    }

    /// Starts (or restarts) the animation described by `spec` on `node`.
    ///
    /// Returns `false` when the animation has no duration or unknown name, which
    /// matches how browsers treat non-existent keyframes (nothing animates).
    pub fn start(&mut self, node: NodeId, spec: &Animation) -> bool {
        if spec.name.is_empty() || spec.name.eq_ignore_ascii_case("none") {
            return false;
        }
        let Some(frames) = self.keyframes.get(&spec.name).cloned() else {
            return false;
        };
        if frames.frames.is_empty() {
            return false;
        }
        let infinite = matches!(spec.iteration_count, AnimationIterationCount::Infinite);
        let has_timeline = spec.timeline.is_some() || self.node_timelines.contains_key(&node);
        if !infinite && spec.duration_ms <= 0.0 && !has_timeline {
            return false;
        }
        // Restart if an identical animation already exists for this node.
        self.active
            .retain(|a| !(a.node == node && a.spec.name == spec.name));
        self.active.push(ActiveAnimation {
            node,
            spec: spec.clone(),
            frames,
            elapsed_ms: 0.0,
            iterations_done: 0.0,
            started: false,
            finished: false,
        });
        true
    }

    /// Stops a named animation on a node, emitting `animationcancel`.
    pub fn cancel(&mut self, node: NodeId, name: &str) {
        let before = self.active.len();
        self.active
            .retain(|a| !(a.node == node && a.spec.name == name));
        if self.active.len() != before {
            self.events.push(AnimationEvent {
                node,
                name: name.to_string(),
                kind: AnimationEventKind::Cancel,
            });
        }
    }

    /// Drops all animation state (e.g. on navigation).
    pub fn clear(&mut self) {
        self.active.clear();
        self.events.clear();
        self.scroll_timelines.clear();
        self.view_timelines.clear();
        self.node_timelines.clear();
    }

    /// Drops animation state for nodes that no longer exist / changed identity.
    pub fn retain_nodes(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        self.active.retain(|a| keep(a.node));
    }

    /// Advances every running animation by `dt_ms` milliseconds.
    pub fn advance(&mut self, dt_ms: f32) {
        if dt_ms <= 0.0 {
            return;
        }
        let mut new_events: Vec<AnimationEvent> = Vec::new();
        for anim in &mut self.active {
            if anim.finished {
                continue;
            }
            anim.elapsed_ms += dt_ms;

            // Delay phase: nothing has started yet.
            if anim.elapsed_ms < anim.spec.delay_ms {
                continue;
            }
            if !anim.started {
                anim.started = true;
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::Start,
                });
            }

            let duration = anim.spec.duration_ms.max(0.0);
            if duration <= 0.0 {
                anim.finished = true;
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::End,
                });
                continue;
            }

            let active_ms = anim.elapsed_ms - anim.spec.delay_ms;
            let total_iterations = active_ms / duration;
            if let AnimationIterationCount::Finite(count) = anim.spec.iteration_count
                && total_iterations >= count
            {
                anim.finished = true;
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::End,
                });
                continue;
            }
            // Emit an iteration event each time an iteration boundary is crossed.
            if total_iterations.floor() > anim.iterations_done {
                anim.iterations_done = total_iterations.floor();
                new_events.push(AnimationEvent {
                    node: anim.node,
                    name: anim.spec.name.clone(),
                    kind: AnimationEventKind::Iteration,
                });
            }
        }
        self.events.extend(new_events);
    }

    /// Computes the animation progress for a single active animation, honoring
    /// direction, iteration count, and fill mode. Returns `None` when the
    /// animation contributes nothing at this moment.
    fn progress_for(anim: &ActiveAnimation) -> Option<f32> {
        let delay = anim.spec.delay_ms;
        let duration = anim.spec.duration_ms.max(0.0001);
        let elapsed = anim.elapsed_ms;

        // Before the delay elapses, only `backwards`/`both` fill modes apply.
        if elapsed < delay {
            return match anim.spec.fill_mode {
                AnimationFillMode::Backwards | AnimationFillMode::Both => Some(0.0),
                _ => None,
            };
        }
        let active_ms = elapsed - delay;
        let iterations = active_ms / duration;

        let finished = match anim.spec.iteration_count {
            AnimationIterationCount::Infinite => false,
            AnimationIterationCount::Finite(n) => iterations >= n,
        };
        if finished {
            return match anim.spec.fill_mode {
                AnimationFillMode::Forwards | AnimationFillMode::Both => {
                    // Freeze on the final iteration boundary.
                    let last = match anim.spec.iteration_count {
                        AnimationIterationCount::Finite(n) => n.max(0.0),
                        AnimationIterationCount::Infinite => iterations,
                    };
                    Some(Self::apply_direction(
                        last.fract() == 0.0 && last > 0.0,
                        last,
                        &anim.spec,
                    ))
                }
                _ => None,
            };
        }

        let complete_iterations = iterations.floor();
        let mut t = iterations.fract();
        match anim.spec.direction {
            AnimationDirection::Normal => {}
            AnimationDirection::Reverse => t = 1.0 - t,
            AnimationDirection::Alternate => {
                if (complete_iterations as i64) % 2 != 0 {
                    t = 1.0 - t;
                }
            }
            AnimationDirection::AlternateReverse => {
                if (complete_iterations as i64) % 2 == 0 {
                    t = 1.0 - t;
                }
            }
        }
        let _ = AnimationPlayState::Running;
        Some(t.clamp(0.0, 1.0))
    }

    /// Helper that resolves the final progress for fill-forwards at an exact
    /// iteration boundary (which direction-aware fraction would otherwise erase).
    fn apply_direction(_at_boundary: bool, iterations: f32, spec: &Animation) -> f32 {
        match spec.direction {
            AnimationDirection::Normal | AnimationDirection::Alternate => 1.0,
            AnimationDirection::Reverse => 0.0,
            AnimationDirection::AlternateReverse => {
                if (iterations.max(1.0) as i64) % 2 == 0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    /// Applies all animations targeting `node` to `style`, in declaration order.
    pub fn apply(&self, node: NodeId, style: &mut ComputedStyle) {
        let timeline_override = style
            .animation_timeline
            .as_deref()
            .or_else(|| self.node_timelines.get(&node).map(|s| s.as_str()))
            .and_then(|tl_name| self.timeline_progress(tl_name));

        let ordered: Vec<&ActiveAnimation> =
            self.active.iter().filter(|a| a.node == node).collect();
        for anim in ordered {
            if anim.spec.play_state == AnimationPlayState::Paused {
                // A paused animation freezes at its current progress; the clock is
                // stopped by skipping `advance`, so just render the current frame.
            }
            let raw_progress = if let Some(p) = timeline_override {
                p
            } else if let Some(p) = anim
                .spec
                .timeline
                .as_deref()
                .and_then(|tl| self.timeline_progress(tl))
            {
                p
            } else {
                let Some(progress) = Self::progress_for(anim) else {
                    continue;
                };
                progress
            };
            let eased = anim.spec.timing.sample(raw_progress);
            let declarations = anim.frames.sample(eased);
            if declarations.is_empty() {
                continue;
            }
            apply_cascaded_properties(style, &declarations, None);
        }
    }

    /// Returns (and clears) the lifecycle events accumulated since the last call.
    pub fn drain_events(&mut self) -> Vec<AnimationEvent> {
        std::mem::take(&mut self.events)
    }
}

#[derive(Debug, Clone)]
struct RunningTransition {
    node: NodeId,
    property: String,
    from: ComputedStyle,
    to: ComputedStyle,
    start_ms: f32,
    duration_ms: f32,
    delay_ms: f32,
    timing: TimingFunction,
    finished: bool,
}

/// Drives `transition-*` declarations by diffing consecutive computed styles.
#[derive(Debug, Default)]
pub struct TransitionEngine {
    running: Vec<RunningTransition>,
    now_ms: f32,
    events: Vec<TransitionEvent>,
}

impl TransitionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_animating(&self) -> bool {
        self.running.iter().any(|t| !t.finished)
    }

    pub fn running_count(&self) -> usize {
        self.running.len()
    }

    /// Registers `new` as the target style for `node`, starting transitions for
    /// every property that changed and has a matching `transition-*` declaration.
    ///
    /// Returns `true` when at least one transition started.
    pub fn retarget(&mut self, node: NodeId, old: &ComputedStyle, new: &ComputedStyle) -> bool {
        // A transition that is itself replaced keeps its current blended value as
        // the new start point, matching browser "reversing transition" behavior.
        let current = self.current_style(node, new);
        for spec in &new.transitions {
            if spec.duration_ms <= 0.0 && spec.delay_ms <= 0.0 {
                continue;
            }
            let matches_property = |prop: &str| -> bool {
                if spec.property == "all" || spec.property.eq_ignore_ascii_case(prop) {
                    return true;
                }
                match spec.property.to_ascii_lowercase().as_str() {
                    "background" => prop == "background-color",
                    "border" => {
                        prop == "border-color" || prop == "border-width" || prop == "border-radius"
                    }
                    "border-color" => prop == "border-color",
                    "border-width" => prop == "border-width",
                    "border-radius" => prop == "border-radius",
                    "outline" => {
                        prop == "outline-width" || prop == "outline-offset" || prop == "color"
                    }
                    "margin" => prop == "margin",
                    "padding" => prop == "padding",
                    "flex" => prop == "flex-grow" || prop == "flex-shrink" || prop == "flex-basis",
                    "gap" => prop == "gap",
                    "inset" => {
                        prop == "top" || prop == "right" || prop == "bottom" || prop == "left"
                    }
                    _ => false,
                }
            };
            for prop in ANIMATABLE_PROPERTIES {
                if !matches_property(prop) || !property_differs(old, new, prop) {
                    continue;
                }
                let from = if self
                    .running
                    .iter()
                    .any(|t| t.node == node && t.property == *prop && !t.finished)
                {
                    current.clone()
                } else {
                    old.clone()
                };
                // Restart an in-flight transition for this property.
                self.running
                    .retain(|t| !(t.node == node && t.property == *prop && !t.finished));
                self.running.push(RunningTransition {
                    node,
                    property: (*prop).to_string(),
                    from,
                    to: new.clone(),
                    start_ms: self.now_ms,
                    duration_ms: spec.duration_ms,
                    delay_ms: spec.delay_ms,
                    timing: spec.timing,
                    finished: false,
                });
            }
        }
        self.running.iter().any(|t| t.node == node && !t.finished)
    }

    /// Advances the transition clock by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        if dt_ms <= 0.0 {
            return;
        }
        self.now_ms += dt_ms;
        let now = self.now_ms;
        let mut events: Vec<TransitionEvent> = Vec::new();
        for t in &mut self.running {
            if t.finished {
                continue;
            }
            let total = t.delay_ms + t.duration_ms;
            if now - t.start_ms >= total {
                t.finished = true;
                events.push(TransitionEvent {
                    node: t.node,
                    property: t.property.clone(),
                    elapsed_ms: t.duration_ms,
                });
            }
        }
        self.running
            .retain(|t| !t.finished || now - t.start_ms < t.delay_ms + t.duration_ms);
        self.events.extend(events);
    }

    fn blend_factor(&self, t: &RunningTransition, now: f32) -> f32 {
        let since = now - t.start_ms;
        if since <= t.delay_ms {
            return 0.0;
        }
        if t.duration_ms <= 0.0 {
            return 1.0;
        }
        let raw = ((since - t.delay_ms) / t.duration_ms).clamp(0.0, 1.0);
        t.timing.sample(raw)
    }

    /// Returns the interpolated style for `node` as of the current clock.
    ///
    /// `base` is the freshly computed target style; the returned style blends it
    /// with the recorded start styles of any in-flight transitions.
    pub fn current_style(&self, node: NodeId, base: &ComputedStyle) -> ComputedStyle {
        let active: Vec<&RunningTransition> = self
            .running
            .iter()
            .filter(|t| t.node == node && !t.finished)
            .collect();
        if active.is_empty() {
            return base.clone();
        }
        // Blend properties one at a time. `blend` starts from the transition's
        // "from" style so untouched properties keep their original values.
        let mut result = base.clone();
        for t in active {
            if t.to != *base {
                // The target changed since this transition started but the new
                // target has not been retargeted yet; keep animating toward `to`.
                result = interpolate_style(&t.from, &t.to, self.blend_factor(t, self.now_ms));
                let _ = &result;
            } else {
                result = interpolate_style(&t.from, &t.to, self.blend_factor(t, self.now_ms));
            }
        }
        result
    }

    /// Returns (and clears) `transitionend` events emitted since the last call.
    pub fn drain_events(&mut self) -> Vec<TransitionEvent> {
        std::mem::take(&mut self.events)
    }

    /// Drops all transition state.
    pub fn clear(&mut self) {
        self.running.clear();
        self.events.clear();
    }

    /// Drops transitions for nodes that no longer exist.
    pub fn retain_nodes(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        self.running.retain(|t| keep(t.node));
    }
}

/// Properties the transition engine is able to interpolate.
pub const ANIMATABLE_PROPERTIES: &[&str] = &[
    "opacity",
    "transform",
    "color",
    "background-color",
    "border-color",
    "width",
    "height",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "margin",
    "padding",
    "top",
    "right",
    "bottom",
    "left",
    "font-size",
    "letter-spacing",
    "word-spacing",
    "text-indent",
    "border-width",
    "border-radius",
    "box-shadow",
    "text-shadow",
    "filter",
    "flex-basis",
    "flex-grow",
    "flex-shrink",
    "gap",
    "outline-width",
    "outline-offset",
    "aspect-ratio",
];

/// The set of `@keyframes` names plus the transition/animation engines, ready to
/// be driven by a render loop.
#[derive(Debug, Default)]
pub struct AnimationState {
    pub animations: AnimationEngine,
    pub transitions: TransitionEngine,
    /// Time of the last tick in milliseconds (monotonic within the browser run).
    pub last_tick_ms: f32,
}

impl AnimationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads `@keyframes` from author stylesheets.
    pub fn load_keyframes<'a, I>(&mut self, sheets: I)
    where
        I: IntoIterator<Item = &'a Stylesheet>,
    {
        self.animations.set_keyframes(collect_keyframes(sheets));
    }

    /// Starts any animations declared by `style` that are not already running.
    pub fn sync_animations(&mut self, node: NodeId, style: &ComputedStyle) -> bool {
        let mut started = false;
        for spec in &style.animations {
            if spec.name.is_empty() || spec.name.eq_ignore_ascii_case("none") {
                continue;
            }
            if self
                .animations
                .active
                .iter()
                .any(|a| a.node == node && a.spec.name == spec.name && !a.finished)
            {
                continue;
            }
            started |= self.animations.start(node, spec);
        }
        started
    }

    /// Advances both engines by `dt_ms`.
    pub fn advance(&mut self, dt_ms: f32) {
        self.last_tick_ms += dt_ms;
        self.animations.advance(dt_ms);
        self.transitions.advance(dt_ms);
    }

    /// Returns true when a new frame is required.
    pub fn is_animating(&self) -> bool {
        self.animations.is_animating() || self.transitions.is_animating()
    }
}
