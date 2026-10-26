//! Per-document scheduling for scripts and image resources.
//!
//! The scheduler deliberately owns policy, not network or rendering. Network
//! workers remain bounded by the existing fetch pools; this type decides which
//! work is eligible, records every state transition, and applies visibility /
//! foreground pressure to low-priority work.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Script,
    Module,
    Image,
    Font,
    Stylesheet,
    Fetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourcePriority {
    Blocking,
    Visible,
    Defer,
    Async,
    Background,
    Speculative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceState {
    Discovered,
    Queued,
    Fetching,
    Ready,
    Executing,
    Completed,
    Cancelled,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RejectReason {
    CpuBudget,
    MemoryPressure,
    Timeout,
    Unsupported,
    SupersededNavigation,
    InvalidUrl,
    NetworkFailure,
    DecodeFailure,
}

#[derive(Debug, Clone)]
pub struct ResourceEvent {
    pub id: ResourceId,
    pub kind: ResourceKind,
    pub state: ResourceState,
    pub priority: ResourcePriority,
    pub url: String,
    pub reason: Option<RejectReason>,
    pub bytes: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct DocumentResourceBudget {
    /// CPU time available to non-blocking resource work in one event-loop turn.
    pub cpu_slice: Duration,
    /// Maximum non-blocking script CPU per rolling one-second window.
    pub cpu_total: Duration,
    /// V8 heap pressure threshold sampled after script execution.
    pub js_heap_bytes: usize,
    /// Decoded pixel memory target used by the image cache.
    pub decoded_image_bytes: usize,
}

impl Default for DocumentResourceBudget {
    fn default() -> Self {
        Self {
            cpu_slice: Duration::from_millis(8),
            cpu_total: Duration::from_millis(50),
            js_heap_bytes: 512 * 1024 * 1024,
            decoded_image_bytes: 24 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResourceReport {
    pub discovered: usize,
    pub queued: usize,
    pub fetching: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub rejected: usize,
    pub bytes_fetched: usize,
    pub script_cpu: Duration,
    pub peak_js_heap: usize,
    pub generation: u64,
    pub rejection_reasons: HashMap<RejectReason, usize>,
}

#[derive(Debug)]
struct ResourceRecord {
    event: ResourceEvent,
}

/// State and policy for one active document.
#[derive(Debug)]
pub struct DocumentResourceScheduler {
    budget: DocumentResourceBudget,
    generation: u64,
    next_id: u64,
    visible: bool,
    turn_started: Instant,
    total_cpu: Duration,
    cpu_window_started: Instant,
    cpu_window_used: Duration,
    current_js_heap: usize,
    report: ResourceReport,
    records: HashMap<ResourceId, ResourceRecord>,
    low_priority: VecDeque<ResourceId>,
}

impl Default for DocumentResourceScheduler {
    fn default() -> Self {
        Self::new(DocumentResourceBudget::default())
    }
}

impl DocumentResourceScheduler {
    pub fn new(budget: DocumentResourceBudget) -> Self {
        Self {
            budget,
            generation: 0,
            next_id: 1,
            visible: true,
            turn_started: Instant::now(),
            total_cpu: Duration::ZERO,
            cpu_window_started: Instant::now(),
            cpu_window_used: Duration::ZERO,
            current_js_heap: 0,
            report: ResourceReport::default(),
            records: HashMap::new(),
            low_priority: VecDeque::new(),
        }
    }

    pub fn begin_document(&mut self, generation: u64) {
        self.generation = generation;
        self.next_id = 1;
        self.turn_started = Instant::now();
        self.total_cpu = Duration::ZERO;
        self.cpu_window_started = Instant::now();
        self.cpu_window_used = Duration::ZERO;
        self.current_js_heap = 0;
        self.report = ResourceReport {
            generation,
            ..ResourceReport::default()
        };
        self.records.clear();
        self.low_priority.clear();
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn begin_turn(&mut self) {
        self.turn_started = Instant::now();
        // A CPU budget is a throttle, not a permanent resource rejection.
        // Refill the allowance after a quiet window so deferred work can
        // resume after sustained page activity.
        if self.cpu_window_started.elapsed() >= Duration::from_secs(1) {
            self.cpu_window_started = Instant::now();
            self.cpu_window_used = Duration::ZERO;
        }
    }

    /// Earliest time throttled non-blocking script work may resume.
    pub fn cpu_resume_deadline(&self) -> Option<Instant> {
        (self.cpu_window_used >= self.budget.cpu_total)
            .then_some(self.cpu_window_started + Duration::from_secs(1))
    }

    pub fn discover(
        &mut self,
        kind: ResourceKind,
        priority: ResourcePriority,
        url: impl Into<String>,
    ) -> ResourceId {
        let id = ResourceId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let event = ResourceEvent {
            id,
            kind,
            state: ResourceState::Discovered,
            priority,
            url: url.into(),
            reason: None,
            bytes: 0,
        };
        self.records.insert(id, ResourceRecord { event });
        self.report.discovered += 1;
        if priority >= ResourcePriority::Async {
            self.low_priority.push_back(id);
        }
        id
    }

    pub fn transition(&mut self, id: ResourceId, state: ResourceState) {
        if let Some(record) = self.records.get_mut(&id) {
            record.event.state = state;
            match state {
                ResourceState::Queued => self.report.queued += 1,
                ResourceState::Fetching => self.report.fetching += 1,
                ResourceState::Completed => self.report.completed += 1,
                ResourceState::Failed => self.report.failed += 1,
                ResourceState::Cancelled => self.report.cancelled += 1,
                ResourceState::Rejected => self.report.rejected += 1,
                _ => {}
            }
        }
    }

    pub fn reject(&mut self, id: ResourceId, reason: RejectReason) {
        if let Some(record) = self.records.get_mut(&id) {
            record.event.reason = Some(reason);
            record.event.state = ResourceState::Rejected;
            self.report.rejected += 1;
            *self.report.rejection_reasons.entry(reason).or_default() += 1;
        }
    }

    /// Completes one resource as failed and retains the reason on its record.
    /// Handlers report outcomes; the scheduler owns resource lifecycle state.
    pub fn fail(&mut self, id: ResourceId, reason: RejectReason) {
        if let Some(record) = self.records.get_mut(&id) {
            if matches!(
                record.event.state,
                ResourceState::Completed
                    | ResourceState::Failed
                    | ResourceState::Cancelled
                    | ResourceState::Rejected
            ) {
                return;
            }
            record.event.reason = Some(reason);
            record.event.state = ResourceState::Failed;
            self.report.failed += 1;
            *self.report.rejection_reasons.entry(reason).or_default() += 1;
        }
    }

    pub fn cancel_generation(&mut self, reason: RejectReason) {
        let ids: Vec<_> = self
            .records
            .iter()
            .filter_map(|(id, record)| {
                (!matches!(
                    record.event.state,
                    ResourceState::Completed
                        | ResourceState::Failed
                        | ResourceState::Cancelled
                        | ResourceState::Rejected
                ))
                .then_some(*id)
            })
            .collect();
        for id in ids {
            if let Some(record) = self.records.get_mut(&id) {
                record.event.state = ResourceState::Cancelled;
                record.event.reason = Some(reason);
                self.report.cancelled += 1;
                *self.report.rejection_reasons.entry(reason).or_default() += 1;
            }
        }
    }

    /// Blocking work is always eligible. Non-blocking work is throttled when
    /// hidden or when the current turn/rolling CPU budget is exhausted.
    pub fn allow_execution(&self, priority: ResourcePriority) -> bool {
        if !self.visible && priority >= ResourcePriority::Async {
            return false;
        }
        if priority != ResourcePriority::Blocking
            && (self.turn_started.elapsed() > self.budget.cpu_slice
                || self.cpu_window_used >= self.budget.cpu_total)
        {
            return false;
        }
        self.execution_block_reason(priority).is_none()
    }

    /// Returns a permanent rejection reason. CPU slice exhaustion is
    /// temporary, so it blocks admission without rejecting queued work.
    pub fn execution_block_reason(&self, priority: ResourcePriority) -> Option<RejectReason> {
        if priority == ResourcePriority::Blocking {
            return None;
        }
        if !self.visible && priority >= ResourcePriority::Async {
            return None;
        }
        if self.current_js_heap >= self.budget.js_heap_bytes {
            return Some(RejectReason::MemoryPressure);
        }
        None
    }

    pub fn record_script_cpu(&mut self, elapsed: Duration) {
        self.total_cpu = self.total_cpu.saturating_add(elapsed);
        self.cpu_window_used = self.cpu_window_used.saturating_add(elapsed);
        self.report.script_cpu = self.total_cpu;
    }

    pub fn record_bytes(&mut self, id: ResourceId, bytes: usize) {
        if let Some(record) = self.records.get_mut(&id) {
            record.event.bytes = bytes;
        }
        self.report.bytes_fetched = self.report.bytes_fetched.saturating_add(bytes);
    }

    pub fn finish_url(
        &mut self,
        url: &str,
        state: ResourceState,
        reason: Option<RejectReason>,
        bytes: usize,
    ) {
        let ids: Vec<_> = self
            .records
            .iter()
            .filter_map(|(id, record)| {
                (record.event.url == url
                    && !matches!(
                        record.event.state,
                        ResourceState::Completed
                            | ResourceState::Failed
                            | ResourceState::Cancelled
                            | ResourceState::Rejected
                    ))
                .then_some(*id)
            })
            .collect();
        for id in ids {
            self.record_bytes(id, bytes);
            if let Some(reason) = reason {
                self.reject(id, reason);
            } else {
                self.transition(id, state);
            }
        }
    }

    pub fn set_priority_url(&mut self, url: &str, priority: ResourcePriority) {
        for record in self.records.values_mut() {
            if record.event.url == url
                && !matches!(
                    record.event.state,
                    ResourceState::Completed
                        | ResourceState::Failed
                        | ResourceState::Cancelled
                        | ResourceState::Rejected
                )
            {
                record.event.priority = priority;
            }
        }
    }

    pub fn claim_url(&mut self, url: &str) -> Option<ResourceId> {
        let id = self.records.iter().find_map(|(id, record)| {
            (record.event.url == url && record.event.state == ResourceState::Queued).then_some(*id)
        })?;
        if let Some(record) = self.records.get_mut(&id) {
            // Ready is a lightweight reservation state. It prevents duplicate
            // inline URLs from claiming the same scheduler record.
            record.event.state = ResourceState::Ready;
        }
        Some(id)
    }

    pub fn record_js_heap(&mut self, bytes: usize) {
        self.current_js_heap = bytes;
        self.report.peak_js_heap = self.report.peak_js_heap.max(bytes);
    }

    pub fn budget(&self) -> DocumentResourceBudget {
        self.budget
    }

    pub fn report(&self) -> &ResourceReport {
        &self.report
    }

    pub fn events(&self) -> impl Iterator<Item = &ResourceEvent> {
        self.records.values().map(|record| &record.event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocking_work_is_not_count_limited() {
        let mut scheduler = DocumentResourceScheduler::default();
        scheduler.begin_document(7);
        for _ in 0..32 {
            let id = scheduler.discover(ResourceKind::Script, ResourcePriority::Blocking, "x.js");
            assert!(scheduler.allow_execution(ResourcePriority::Blocking));
            scheduler.transition(id, ResourceState::Completed);
        }
        assert_eq!(scheduler.report().discovered, 32);
    }

    #[test]
    fn hidden_tabs_throttle_low_priority_work() {
        let mut scheduler = DocumentResourceScheduler::default();
        scheduler.set_visible(false);
        assert!(scheduler.allow_execution(ResourcePriority::Blocking));
        assert!(!scheduler.allow_execution(ResourcePriority::Async));
    }

    #[test]
    fn rejection_has_an_explicit_reason() {
        let mut scheduler = DocumentResourceScheduler::default();
        let id = scheduler.discover(ResourceKind::Image, ResourcePriority::Speculative, "x.png");
        scheduler.reject(id, RejectReason::MemoryPressure);
        assert_eq!(scheduler.report().rejected, 1);
        assert_eq!(
            scheduler
                .report()
                .rejection_reasons
                .get(&RejectReason::MemoryPressure),
            Some(&1)
        );
    }
}
