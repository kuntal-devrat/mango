//! Timer and event queue management for the JavaScript runtime.
//!
//! Manages `setTimeout`, `setInterval`, and their cancellation. The browser
//! calls `tick()` on every frame to fire expired timers and run callbacks.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use boa_engine::Context;

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
    pub fn schedule_timeout_with_id(&mut self, id: TimerId, callback_source: String, delay_ms: u64) {
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
    pub fn schedule_interval_with_id(&mut self, id: TimerId, callback_source: String, delay_ms: u64) {
        if id >= self.next_id {
            self.next_id = id + 1;
        }
        let period = Duration::from_millis(delay_ms.max(4)); // Minimum 4ms like browsers
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

    /// Processes expired timers and queued background tasks. Evaluates their callback in the given Boa context.
    /// Returns `true` if any timer or task fired (indicating the page might need re-rendering).
    pub fn tick(&mut self, context: &mut Context) -> bool {
        let mut fired = false;

        // Drain background tasks (e.g. async fetch resolutions)
        let tasks = if let Ok(mut q) = self.tasks.lock() {
            std::mem::take(&mut *q)
        } else {
            Vec::new()
        };

        for source in tasks {
            match context.eval(boa_engine::Source::from_bytes(&source)) {
                Ok(_) => {
                    fired = true;
                }
                Err(e) => {
                    log::warn!("Background task callback error: {}", e);
                }
            }
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

            // Execute the callback
            match context.eval(boa_engine::Source::from_bytes(&source)) {
                Ok(_) => {
                    log::debug!("Timer {} fired successfully", id);
                    fired = true;
                }
                Err(e) => {
                    log::warn!("Timer {} callback error: {}", id, e);
                }
            }

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

        fired
    }

    /// Returns true if there are any pending timers or background tasks.
    pub fn has_pending_timers(&self) -> bool {
        let has_tasks = self.tasks.lock().map(|q| !q.is_empty()).unwrap_or(false);
        has_tasks || self.timers.values().any(|t| !t.cancelled)
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
