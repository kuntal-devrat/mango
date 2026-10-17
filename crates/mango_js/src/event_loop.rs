//! Timer and event queue management for the JavaScript runtime.
//!
//! Manages `setTimeout`, `setInterval`, and their cancellation. The browser
//! calls `tick()` on every frame to fire expired timers and run callbacks.

use std::collections::HashMap;
use std::time::{Duration, Instant};


/// Unique identifier for a scheduled timer.
pub type TimerId = u32;

/// A scheduled timer (either one-shot or repeating).
struct Timer {
    /// The JS function source to evaluate when the timer fires.
    callback_source: String,
    /// When the timer is next due.
    fire_at: Instant,
    /// For intervals: the repeat period; None for timeouts.
    interval: Option<Duration>,
    /// Whether this timer has been cancelled.
    cancelled: bool,
}

/// The event loop manages timers, background async tasks, and queued callbacks.
pub struct EventLoop {
    timers: HashMap<TimerId, Timer>,
    next_id: TimerId,
    tasks: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

/// Maximum number of concurrent active timers allowed to prevent DoS attacks.
pub const MAX_ACTIVE_TIMERS: usize = 1000;
/// Minimum allowed interval duration in milliseconds per HTML timer spec.
pub const MIN_TIMER_INTERVAL_MS: u64 = 4;

impl EventLoop {
    pub fn new() -> Self {
        Self {
            timers: HashMap::new(),
            next_id: 1,
            tasks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Returns a thread-safe handle to enqueue tasks into this event loop from any thread.
    pub fn task_queue(&self) -> std::sync::Arc<std::sync::Mutex<Vec<String>>> {
        self.tasks.clone()
    }

    /// Enqueues a task to be executed on the next tick.
    pub fn enqueue_task(&self, source: String) {
        if let Ok(mut q) = self.tasks.lock() {
            q.push(source);
        }
    }

    /// Schedules a one-shot timeout. Returns the timer ID.
    pub fn schedule_timeout(&mut self, callback_source: String, delay_ms: u64) -> TimerId {
        let id = self.next_id;
        self.next_id += 1;
        self.schedule_timeout_with_id(id, callback_source, delay_ms);
        id
    }

    /// Schedules a one-shot timeout with a predetermined ID.
    pub fn schedule_timeout_with_id(
        &mut self,
        id: TimerId,
        callback_source: String,
        delay_ms: u64,
    ) {
        if self.timers.len() >= MAX_ACTIVE_TIMERS {
            self.timers.retain(|_, t| !t.cancelled);
        }
        if self.timers.len() >= MAX_ACTIVE_TIMERS {
            log::warn!(
                "Maximum active timers reached ({}), ignoring setTimeout",
                MAX_ACTIVE_TIMERS
            );
            return;
        }
        if id >= self.next_id {
            self.next_id = id + 1;
        }
        self.timers.insert(
            id,
            Timer {
                callback_source,
                fire_at: Instant::now() + Duration::from_millis(delay_ms),
                interval: None,
                cancelled: false,
            },
        );
        log::debug!("setTimeout registered: id={}, delay={}ms", id, delay_ms);
    }

    /// Schedules a repeating interval. Returns the timer ID.
    pub fn schedule_interval(&mut self, callback_source: String, delay_ms: u64) -> TimerId {
        let id = self.next_id;
        self.next_id += 1;
        self.schedule_interval_with_id(id, callback_source, delay_ms);
        id
    }

    /// Schedules a repeating interval with a predetermined ID.
    pub fn schedule_interval_with_id(
        &mut self,
        id: TimerId,
        callback_source: String,
        delay_ms: u64,
    ) {
        if self.timers.len() >= MAX_ACTIVE_TIMERS {
            self.timers.retain(|_, t| !t.cancelled);
        }
        if self.timers.len() >= MAX_ACTIVE_TIMERS {
            log::warn!(
                "Maximum active timers reached ({}), ignoring setInterval",
                MAX_ACTIVE_TIMERS
            );
            return;
        }
        if id >= self.next_id {
            self.next_id = id + 1;
        }
        let period = Duration::from_millis(delay_ms.max(MIN_TIMER_INTERVAL_MS));
        self.timers.insert(
            id,
            Timer {
                callback_source,
                fire_at: Instant::now() + period,
                interval: Some(period),
                cancelled: false,
            },
        );
        log::debug!("setInterval registered: id={}, delay={}ms", id, delay_ms);
    }

    /// Cancels a timer by ID.
    pub fn cancel_timer(&mut self, id: TimerId) {
        if let Some(timer) = self.timers.get_mut(&id) {
            timer.cancelled = true;
            log::debug!("Timer cancelled: id={}", id);
        }
    }

    /// Collects and reschedules/removes all due tasks and timer scripts.
    pub fn drain_due(&mut self) -> Vec<String> {
        let mut scripts = Vec::new();

        // Drain background tasks (e.g. async fetch resolutions)
        if let Ok(mut q) = self.tasks.lock() {
            scripts.extend(std::mem::take(&mut *q));
        }

        let now = Instant::now();

        // Collect expired timer IDs
        let expired: Vec<TimerId> = self
            .timers
            .iter()
            .filter(|(_, t)| !t.cancelled && t.fire_at <= now)
            .map(|(id, _)| *id)
            .collect();

        for id in expired {
            let (source, interval) = {
                let timer = match self.timers.get(&id) {
                    Some(t) if !t.cancelled => t,
                    _ => continue,
                };
                (timer.callback_source.clone(), timer.interval)
            };

            scripts.push(source);

            // Reschedule intervals, remove timeouts
            match interval {
                Some(period) => {
                    if let Some(timer) = self.timers.get_mut(&id) {
                        timer.fire_at = now + period;
                    }
                }
                None => {
                    self.timers.remove(&id);
                }
            }
        }

        // Clean up cancelled timers
        self.timers.retain(|_, t| !t.cancelled);

        scripts
    }

    /// Evaluates due tasks and timers on deno_core::JsRuntime.
    /// Returns `true` if any timer or task fired.
    pub fn tick_deno(&mut self, runtime: &mut deno_core::JsRuntime) -> bool {
        let scripts = self.drain_due();
        let mut fired = false;
        for s in scripts {
            match runtime.execute_script("<event_loop>", s) {
                Ok(_) => {
                    fired = true;
                }
                Err(e) => {
                    log::warn!("EventLoop task execution error: {e}");
                }
            }
        }
        fired
    }

    /// Returns true if there are any pending timers or background tasks.
    pub fn has_pending_timers(&self) -> bool {
        let has_tasks = self.tasks.lock().map(|q| !q.is_empty()).unwrap_or(false);
        has_tasks || self.timers.values().any(|t| !t.cancelled)
    }

    /// Returns the earliest instant at which a timer or background task needs execution.
    pub fn next_deadline(&self) -> Option<Instant> {
        let has_tasks = self.tasks.lock().map(|q| !q.is_empty()).unwrap_or(false);
        if has_tasks {
            return Some(Instant::now());
        }
        self.timers
            .values()
            .filter(|t| !t.cancelled)
            .map(|t| t.fire_at)
            .min()
    }

    /// Clears all timers and pending tasks.
    pub fn clear_all(&mut self) {
        self.timers.clear();
        if let Ok(mut q) = self.tasks.lock() {
            q.clear();
        }
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_capacity_limit_and_interval_clamping() {
        let mut el = EventLoop::new();
        // Clamping to MIN_TIMER_INTERVAL_MS
        let id1 = el.schedule_interval("console.log('hi')".to_string(), 0);
        let timer = el.timers.get(&id1).unwrap();
        assert_eq!(
            timer.interval,
            Some(Duration::from_millis(MIN_TIMER_INTERVAL_MS))
        );

        // Attempting to exceed MAX_ACTIVE_TIMERS
        for _ in 1..MAX_ACTIVE_TIMERS {
            el.schedule_timeout("1+1".to_string(), 100);
        }
        assert_eq!(el.timers.len(), MAX_ACTIVE_TIMERS);

        // Additional timers are safely rejected
        let excess_id = el.schedule_timeout("overflow()".to_string(), 100);
        assert!(!el.timers.contains_key(&excess_id));
        assert_eq!(el.timers.len(), MAX_ACTIVE_TIMERS);
    }
}
