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
    prefer_tasks: bool,
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
            prefer_tasks: true,
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
        mango_core::wake_event_loop();
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

    /// Returns the number of live (non-cancelled) timers.
    pub fn pending_count(&self) -> usize {
        self.timers.values().filter(|t| !t.cancelled).count()
    }

    /// Makes every outstanding timer due immediately.
    ///
    /// Used by deterministic test harnesses that advance virtual time instead
    /// of sleeping for real wall-clock delays. Returns the number of live
    /// timers that were expedited.
    pub fn expedite_timers(&mut self) -> usize {
        let now = Instant::now();
        let mut count = 0;
        for timer in self.timers.values_mut() {
            if !timer.cancelled {
                timer.fire_at = now;
                count += 1;
            }
        }
        count
    }

    /// Collects and reschedules/removes all due tasks and timer scripts.
    pub fn drain_due(&mut self) -> Vec<String> {
        self.drain_due_limited(usize::MAX)
    }

    /// Drains at most `limit` callbacks. A hostile page must not be able to
    /// monopolize the browser thread by making thousands of timers due at
    /// once; unprocessed timers remain scheduled for the next frame.
    pub fn drain_due_limited(&mut self, limit: usize) -> Vec<String> {
        if limit == 0 {
            return Vec::new();
        }
        let now = Instant::now();
        let has_tasks = self.tasks.lock().is_ok_and(|q| !q.is_empty());
        let has_due_timers = self
            .timers
            .values()
            .any(|timer| !timer.cancelled && timer.fire_at <= now);
        let task_first = match (has_tasks, has_due_timers) {
            (true, true) => {
                let first = self.prefer_tasks;
                self.prefer_tasks = !self.prefer_tasks;
                first
            }
            (true, false) => true,
            (false, true) => false,
            (false, false) => return Vec::new(),
        };

        let mut scripts = Vec::with_capacity(limit);
        if task_first {
            self.take_tasks(limit, &mut scripts);
            self.take_due_timers(now, limit.saturating_sub(scripts.len()), &mut scripts);
        } else {
            self.take_due_timers(now, limit, &mut scripts);
            self.take_tasks(limit.saturating_sub(scripts.len()), &mut scripts);
        }
        scripts
    }

    fn take_tasks(&self, limit: usize, scripts: &mut Vec<String>) {
        if limit == 0 {
            return;
        }
        if let Ok(mut q) = self.tasks.lock() {
            let take = q.len().min(limit);
            scripts.extend(q.drain(..take));
        }
    }

    fn take_due_timers(&mut self, now: Instant, limit: usize, scripts: &mut Vec<String>) {
        if limit == 0 {
            return;
        }
        // Deterministic ordering for equal/overdue deadlines: earlier deadline
        // first, then registration ID. HashMap iteration order is unspecified.
        let mut expired: Vec<(Instant, TimerId)> = self
            .timers
            .iter()
            .filter(|(_, timer)| !timer.cancelled && timer.fire_at <= now)
            .map(|(id, timer)| (timer.fire_at, *id))
            .collect();
        expired.sort_unstable();

        for (_, id) in expired.into_iter().take(limit) {
            let (source, interval) = match self.timers.get(&id) {
                Some(timer) if !timer.cancelled => (timer.callback_source.clone(), timer.interval),
                _ => continue,
            };
            scripts.push(source);
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
        self.timers.retain(|_, timer| !timer.cancelled);
    }

    /// Evaluates due tasks and timers on deno_core::JsRuntime.
    /// Returns `true` if any timer or task fired.
    pub fn tick_deno(&mut self, runtime: &mut deno_core::JsRuntime) -> bool {
        // This compatibility entry point must obey the same callback ceiling
        // as the normal runtime tick; never drain an unbounded task backlog.
        const MAX_CALLBACKS_PER_TICK: usize = 1;
        let scripts = self.drain_due_limited(MAX_CALLBACKS_PER_TICK);
        let mut fired = false;
        for s in scripts {
            // This legacy path executes directly on the caller's runtime, so
            // enforce a per-callback ceiling as well as the batch ceiling.
            let isolate = runtime.v8_isolate().thread_safe_handle();
            let (tx, rx) = std::sync::mpsc::channel();
            let watchdog_isolate = isolate.clone();
            let watchdog = std::thread::spawn(move || {
                if rx.recv_timeout(Duration::from_millis(20)).is_err() {
                    watchdog_isolate.terminate_execution();
                }
            });
            match runtime.execute_script("<event_loop>", s) {
                Ok(_) => {
                    fired = true;
                }
                Err(e) => {
                    log::warn!("EventLoop task execution error: {e}");
                }
            }
            let _ = tx.send(());
            let _ = watchdog.join();
            isolate.cancel_terminate_execution();
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
