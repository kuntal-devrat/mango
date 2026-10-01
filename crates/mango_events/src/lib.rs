//! # mango_events
//!
//! W3C DOM Events with capture/bubble phases, decoupled from the browser chrome (ARCH-006).
//!
//! Implements the core W3C DOM Level 3 Events model:
//! - `EventPhase`: Capture, AtTarget, Bubble
//! - `Event`: Base event with propagation control
//! - `EventTarget`: Trait for objects that can receive events
//! - `EventListener`: Registered listener with capture/once flags
//! - `EventDispatcher`: Propagates events along a target chain with proper phase ordering
//!
//! This crate is engine-agnostic: it knows nothing about DOM trees, rendering,
//! or the browser chrome. The consumer provides the **propagation path** (a list
//! of event target IDs from window → document → … → target) and this crate
//! handles capture/at-target/bubble dispatch with `stopPropagation`,
//! `stopImmediatePropagation`, `preventDefault`, and `once` semantics.
//!
//! # Example
//! ```
//! use mango_events::{Event, EventDispatcher, EventPhase, ListenerOptions};
//!
//! let mut dispatcher = EventDispatcher::new();
//!
//! // Register a bubble-phase listener on target 1
//! dispatcher.add_listener(1, "click", |evt| {
//!     assert_eq!(evt.event_type, "click");
//! }, ListenerOptions::default());
//!
//! // Dispatch along path [0, 1] (window=0, target=1)
//! let evt = Event::new("click");
//! let result = dispatcher.dispatch(evt, &[0, 1]);
//! assert!(!result.default_prevented);
//! ```

use std::collections::HashMap;

/// Unique identifier for an event target (e.g., a DOM node ID or window handle).
pub type TargetId = u32;

/// W3C DOM event propagation phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    /// The event is not currently being dispatched.
    None = 0,
    /// The event is propagating down from Window → target's parent.
    Capturing = 1,
    /// The event has reached the target itself.
    AtTarget = 2,
    /// The event is propagating up from target's parent → Window.
    Bubbling = 3,
}

impl Default for EventPhase {
    fn default() -> Self {
        Self::None
    }
}

/// A W3C DOM Event.
#[derive(Debug, Clone)]
pub struct Event {
    /// The event type name (e.g. `"click"`, `"keydown"`).
    pub event_type: String,
    /// Whether the event bubbles up through the target chain.
    pub bubbles: bool,
    /// Whether the event's default action can be cancelled.
    pub cancelable: bool,
    /// Current propagation phase.
    pub phase: EventPhase,
    /// The target this event was originally dispatched to.
    pub target: Option<TargetId>,
    /// The target currently handling the event (changes during propagation).
    pub current_target: Option<TargetId>,
    /// If true, propagation stops after the current target.
    propagation_stopped: bool,
    /// If true, no more listeners on the current target are called.
    immediate_propagation_stopped: bool,
    /// If true, the default action should be prevented.
    pub default_prevented: bool,
    /// Whether this event was created by `dispatchEvent` (vs. user interaction).
    pub is_trusted: bool,
    /// Monotonic timestamp of event creation in milliseconds.
    pub timestamp: f64,
}

impl Event {
    /// Creates a new event with the given type. Bubbles and cancelable by default.
    pub fn new(event_type: &str) -> Self {
        Self {
            event_type: event_type.to_string(),
            bubbles: true,
            cancelable: true,
            phase: EventPhase::None,
            target: None,
            current_target: None,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            default_prevented: false,
            is_trusted: true,
            timestamp: 0.0,
        }
    }

    /// Creates a non-bubbling event (e.g. `focus`, `load`).
    pub fn non_bubbling(event_type: &str) -> Self {
        let mut evt = Self::new(event_type);
        evt.bubbles = false;
        evt
    }

    /// Creates a non-cancelable event (e.g. `load`, `unload`).
    pub fn non_cancelable(event_type: &str) -> Self {
        let mut evt = Self::new(event_type);
        evt.cancelable = false;
        evt
    }

    /// Stops propagation to subsequent targets in the chain.
    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    /// Stops propagation AND prevents remaining listeners on the current target.
    pub fn stop_immediate_propagation(&mut self) {
        self.propagation_stopped = true;
        self.immediate_propagation_stopped = true;
    }

    /// Cancels the default action if the event is cancelable.
    pub fn prevent_default(&mut self) {
        if self.cancelable {
            self.default_prevented = true;
        }
    }

    /// Returns the composed path (target chain) for this event dispatch.
    pub fn is_propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }

    pub fn is_immediate_propagation_stopped(&self) -> bool {
        self.immediate_propagation_stopped
    }
}

/// Options for registering an event listener.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListenerOptions {
    /// If true, the listener runs during the capture phase.
    pub capture: bool,
    /// If true, the listener is removed after the first invocation.
    pub once: bool,
    /// If true, `preventDefault()` will never be called (performance hint).
    pub passive: bool,
}

/// A type-erased event listener callback.
pub type ListenerCallback = Box<dyn FnMut(&mut Event) + Send>;

/// A registered event listener entry.
struct RegisteredListener {
    event_type: String,
    callback: ListenerCallback,
    options: ListenerOptions,
    /// Monotonically increasing ID for stable ordering.
    insertion_order: u64,
}

/// The result of dispatching an event through a target chain.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    /// Whether `preventDefault()` was called.
    pub default_prevented: bool,
    /// Whether `stopPropagation()` was called.
    pub propagation_stopped: bool,
}

/// Central event dispatcher implementing W3C DOM Events capture/bubble model.
///
/// Listeners are registered per target ID and dispatched along a propagation
/// path provided by the caller.
pub struct EventDispatcher {
    /// Listeners keyed by target ID.
    listeners: HashMap<TargetId, Vec<RegisteredListener>>,
    /// Monotonically increasing insertion counter.
    next_id: u64,
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl EventDispatcher {
    /// Creates a new empty dispatcher.
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new(),
            next_id: 0,
        }
    }

    /// Registers an event listener on the specified target.
    pub fn add_listener<F>(
        &mut self,
        target: TargetId,
        event_type: &str,
        callback: F,
        options: ListenerOptions,
    ) where
        F: FnMut(&mut Event) + Send + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;

        self.listeners
            .entry(target)
            .or_default()
            .push(RegisteredListener {
                event_type: event_type.to_string(),
                callback: Box::new(callback),
                options,
                insertion_order: id,
            });
    }

    /// Removes all listeners matching the given target, event type, and capture flag.
    pub fn remove_listener(
        &mut self,
        target: TargetId,
        event_type: &str,
        capture: bool,
    ) {
        if let Some(list) = self.listeners.get_mut(&target) {
            list.retain(|l| !(l.event_type == event_type && l.options.capture == capture));
        }
    }

    /// Removes all listeners for a given target (used on node removal).
    pub fn remove_all_listeners(&mut self, target: TargetId) {
        self.listeners.remove(&target);
    }

    /// Returns the number of listeners registered for the given target.
    pub fn listener_count(&self, target: TargetId) -> usize {
        self.listeners.get(&target).map_or(0, |l| l.len())
    }

    /// Dispatches an event along the given propagation path.
    ///
    /// The `path` is ordered from the outermost ancestor (e.g. Window, index 0)
    /// to the deepest target (last element). The last element in `path` is
    /// treated as the event target.
    ///
    /// Dispatch phases:
    /// 1. **Capture**: Walk path[0..target) calling capture-phase listeners.
    /// 2. **At-Target**: Call both capture and bubble listeners on the target.
    /// 3. **Bubble**: Walk path[target-1..0] calling bubble-phase listeners (if `bubbles`).
    pub fn dispatch(&mut self, mut event: Event, path: &[TargetId]) -> DispatchResult {
        if path.is_empty() {
            return DispatchResult {
                default_prevented: event.default_prevented,
                propagation_stopped: false,
            };
        }

        let target_idx = path.len() - 1;
        let target_id = path[target_idx];
        event.target = Some(target_id);

        let mut fired_once_ids = std::collections::HashSet::new();

        // ── Capture phase ──
        event.phase = EventPhase::Capturing;
        for &current in &path[..target_idx] {
            if event.is_propagation_stopped() {
                break;
            }
            event.current_target = Some(current);
            self.invoke_listeners(current, &mut event, true, &mut fired_once_ids);
        }

        // ── At-Target phase ──
        if !event.is_propagation_stopped() {
            event.phase = EventPhase::AtTarget;
            event.current_target = Some(target_id);
            // At target: both capture and bubble listeners fire
            self.invoke_listeners(target_id, &mut event, true, &mut fired_once_ids);
            if !event.is_immediate_propagation_stopped() {
                self.invoke_listeners(target_id, &mut event, false, &mut fired_once_ids);
            }
        }

        // ── Bubble phase ──
        if event.bubbles && !event.is_propagation_stopped() {
            event.phase = EventPhase::Bubbling;
            for &current in path[..target_idx].iter().rev() {
                if event.is_propagation_stopped() {
                    break;
                }
                event.current_target = Some(current);
                self.invoke_listeners(current, &mut event, false, &mut fired_once_ids);
            }
        }

        event.phase = EventPhase::None;
        event.current_target = None;

        // Clean up only `once` listeners that were actually invoked (audit 4.8 / #886)
        for target in path {
            if let Some(list) = self.listeners.get_mut(target) {
                list.retain(|l| !fired_once_ids.contains(&l.insertion_order));
            }
        }

        DispatchResult {
            default_prevented: event.default_prevented,
            propagation_stopped: event.is_propagation_stopped(),
        }
    }

    /// Invokes matching listeners on the given target for the specified phase.
    fn invoke_listeners(
        &mut self,
        target: TargetId,
        event: &mut Event,
        capture_phase: bool,
        fired_once_ids: &mut std::collections::HashSet<u64>,
    ) {
        let Some(list) = self.listeners.get_mut(&target) else {
            return;
        };

        // Sort by insertion order for stable dispatch
        list.sort_by_key(|l| l.insertion_order);

        // Collect indices to invoke (we can't borrow mutably while iterating)
        let indices: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                l.event_type == event.event_type && l.options.capture == capture_phase
            })
            .map(|(i, _)| i)
            .collect();

        for idx in indices {
            if event.is_immediate_propagation_stopped() {
                break;
            }
            if idx < list.len() {
                if list[idx].options.once {
                    fired_once_ids.insert(list[idx].insertion_order);
                }
                (list[idx].callback)(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_capture_at_target_bubble_ordering() {
        let mut dispatcher = EventDispatcher::new();
        let log = Arc::new(Mutex::new(Vec::<String>::new()));

        // Window (id=0) capture listener
        let log_c = log.clone();
        dispatcher.add_listener(0, "click", move |evt| {
            log_c.lock().unwrap().push(format!("window:capture:{:?}", evt.phase));
        }, ListenerOptions { capture: true, ..Default::default() });

        // Target (id=2) bubble listener
        let log_c = log.clone();
        dispatcher.add_listener(2, "click", move |evt| {
            log_c.lock().unwrap().push(format!("target:bubble:{:?}", evt.phase));
        }, ListenerOptions::default());

        // Parent (id=1) bubble listener
        let log_c = log.clone();
        dispatcher.add_listener(1, "click", move |evt| {
            log_c.lock().unwrap().push(format!("parent:bubble:{:?}", evt.phase));
        }, ListenerOptions::default());

        let event = Event::new("click");
        let result = dispatcher.dispatch(event, &[0, 1, 2]);

        let entries = log.lock().unwrap();
        assert_eq!(entries.len(), 3, "expected 3 listener invocations, got: {:?}", *entries);
        assert!(entries[0].starts_with("window:capture"), "first should be window capture");
        assert!(entries[1].starts_with("target:bubble"), "second should be target at-target");
        assert!(entries[2].starts_with("parent:bubble"), "third should be parent bubble");
        assert!(!result.default_prevented);
    }

    #[test]
    fn test_stop_propagation() {
        let mut dispatcher = EventDispatcher::new();
        let reached_parent = Arc::new(Mutex::new(false));

        // Target stops propagation
        dispatcher.add_listener(1, "click", |evt| {
            evt.stop_propagation();
        }, ListenerOptions::default());

        // Parent should NOT be reached
        let reached = reached_parent.clone();
        dispatcher.add_listener(0, "click", move |_| {
            *reached.lock().unwrap() = true;
        }, ListenerOptions::default());

        let event = Event::new("click");
        dispatcher.dispatch(event, &[0, 1]);

        assert!(!*reached_parent.lock().unwrap(), "parent should not be reached");
    }

    #[test]
    fn test_prevent_default() {
        let mut dispatcher = EventDispatcher::new();

        dispatcher.add_listener(0, "click", |evt| {
            evt.prevent_default();
        }, ListenerOptions::default());

        let event = Event::new("click");
        let result = dispatcher.dispatch(event, &[0]);

        assert!(result.default_prevented);
    }

    #[test]
    fn test_once_listener_removed_after_dispatch() {
        let mut dispatcher = EventDispatcher::new();
        let count = Arc::new(Mutex::new(0u32));

        let count_c = count.clone();
        dispatcher.add_listener(0, "click", move |_| {
            *count_c.lock().unwrap() += 1;
        }, ListenerOptions { once: true, ..Default::default() });

        // First dispatch
        dispatcher.dispatch(Event::new("click"), &[0]);
        assert_eq!(*count.lock().unwrap(), 1);

        // Second dispatch — listener should be gone
        dispatcher.dispatch(Event::new("click"), &[0]);
        assert_eq!(*count.lock().unwrap(), 1);
    }

    #[test]
    fn test_non_bubbling_event() {
        let mut dispatcher = EventDispatcher::new();
        let reached_parent = Arc::new(Mutex::new(false));

        let reached = reached_parent.clone();
        dispatcher.add_listener(0, "focus", move |_| {
            *reached.lock().unwrap() = true;
        }, ListenerOptions::default());

        dispatcher.add_listener(1, "focus", |_| {}, ListenerOptions::default());

        let event = Event::non_bubbling("focus");
        dispatcher.dispatch(event, &[0, 1]);

        assert!(!*reached_parent.lock().unwrap(), "non-bubbling event should not reach parent");
    }

    #[test]
    fn test_remove_listener() {
        let mut dispatcher = EventDispatcher::new();
        let called = Arc::new(Mutex::new(false));

        let called_c = called.clone();
        dispatcher.add_listener(0, "click", move |_| {
            *called_c.lock().unwrap() = true;
        }, ListenerOptions::default());

        dispatcher.remove_listener(0, "click", false);
        dispatcher.dispatch(Event::new("click"), &[0]);

        assert!(!*called.lock().unwrap());
    }

    #[test]
    fn test_remove_all_listeners() {
        let mut dispatcher = EventDispatcher::new();

        dispatcher.add_listener(5, "click", |_| {}, ListenerOptions::default());
        dispatcher.add_listener(5, "keydown", |_| {}, ListenerOptions::default());
        assert_eq!(dispatcher.listener_count(5), 2);

        dispatcher.remove_all_listeners(5);
        assert_eq!(dispatcher.listener_count(5), 0);
    }

    #[test]
    fn test_stop_immediate_propagation() {
        let mut dispatcher = EventDispatcher::new();
        let second_called = Arc::new(Mutex::new(false));

        // First listener stops immediate propagation
        dispatcher.add_listener(0, "click", |evt| {
            evt.stop_immediate_propagation();
        }, ListenerOptions::default());

        // Second listener on same target should NOT fire
        let called = second_called.clone();
        dispatcher.add_listener(0, "click", move |_| {
            *called.lock().unwrap() = true;
        }, ListenerOptions::default());

        dispatcher.dispatch(Event::new("click"), &[0]);
        assert!(!*second_called.lock().unwrap());
    }
}
