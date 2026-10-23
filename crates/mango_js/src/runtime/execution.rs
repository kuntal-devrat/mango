use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::runtime::JsRuntime;

/// Maximum execution budget granted to a large script (>= 20 KB), including
/// the per-byte compilation allowance. This is intentionally a hard ceiling:
/// a runaway script must never freeze the UI for longer than this.
const MAX_LARGE_SCRIPT_TIMEOUT: Duration = Duration::from_millis(5_000);
/// Scripts below this size are considered small and use their caller-provided
/// timeout unchanged.
const LARGE_SCRIPT_THRESHOLD: usize = 20_000;

/// RAII watchdog that terminates a V8 isolate if an execution exceeds its
/// budget, then always clears the termination flag and joins the watchdog
/// thread on all exit paths (success, error, panic, or early return).
struct ExecutionWatchdog {
    tx: Option<Sender<()>>,
    handle: Option<JoinHandle<()>>,
    timed_out: std::sync::Arc<AtomicBool>,
    isolate: deno_core::v8::IsolateHandle,
}

impl ExecutionWatchdog {
    fn new(isolate: deno_core::v8::IsolateHandle, timeout: Duration, thread_name: &str) -> Self {
        let (tx, rx) = channel();
        let timed_out = std::sync::Arc::new(AtomicBool::new(false));
        let watchdog_timed_out = timed_out.clone();
        let watchdog_isolate = isolate.clone();
        let handle = std::thread::Builder::new()
            .name(thread_name.to_string())
            .spawn(move || match rx.recv_timeout(timeout) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    watchdog_timed_out.store(true, Ordering::SeqCst);
                    watchdog_isolate.terminate_execution();
                }
            })
            .ok();

        Self {
            tx: Some(tx),
            handle,
            timed_out,
            isolate,
        }
    }

    /// Stops the watchdog, joins its thread, clears any pending V8 termination,
    /// and reports whether the timeout actually fired.
    fn finish(mut self) -> bool {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        // A watchdog may have fired just as execution completed. Always clear
        // the isolate-wide termination flag so one timeout cannot poison every
        // later script in the same page.
        self.isolate.cancel_terminate_execution();
        self.timed_out.load(Ordering::SeqCst)
    }
}

impl Drop for ExecutionWatchdog {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        self.isolate.cancel_terminate_execution();
    }
}

impl JsRuntime {
    const MAX_HEAP_BYTES: usize = 512 * 1024 * 1024;

    fn enforce_heap_budget(&mut self) -> Result<(), String> {
        if self.execution_disabled {
            return Err("page JavaScript disabled after memory budget exceeded".to_string());
        }
        let used = self
            .runtime
            .v8_isolate()
            .get_heap_statistics()
            .used_heap_size();
        if used > Self::MAX_HEAP_BYTES {
            self.execution_disabled = true;
            self.clear_timers();
            log::error!(
                "Disabling page JavaScript at {} MiB (budget {} MiB)",
                used / (1024 * 1024),
                Self::MAX_HEAP_BYTES / (1024 * 1024)
            );
            return Err("page JavaScript memory budget exceeded".to_string());
        }
        Ok(())
    }

    /// Executes an ES Module script and resolves any top-level exports or promises.
    pub fn execute_module(&mut self, source: &str) -> Result<(), String> {
        self.execute_module_with_name_and_timeout(
            "<module>",
            source,
            std::time::Duration::from_secs(30),
        )
    }

    /// Executes an ES Module script with a specified timeout.
    pub fn execute_module_with_timeout(
        &mut self,
        source: &str,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        self.execute_module_with_name_and_timeout("<module>", source, timeout)
    }

    /// Executes an ES Module script with a given specifier name and timeout.
    pub fn execute_module_with_name_and_timeout(
        &mut self,
        name: &str,
        source: &str,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        self.enforce_heap_budget()?;
        let effective_timeout = Self::compute_scaled_timeout(source.len(), timeout);
        let _guard = self.enter_isolate();
        let isolate_handle = self.runtime.v8_isolate().thread_safe_handle();
        let watchdog = ExecutionWatchdog::new(
            isolate_handle,
            effective_timeout,
            "mango-js-module-watchdog",
        );

        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        static MODULE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let specifier_str = if name.starts_with("http://") || name.starts_with("https://") {
            name.to_string()
        } else {
            format!(
                "mango://module_{}.js",
                MODULE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            )
        };
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let url = deno_core::url::Url::parse(&specifier_str)
                .map_err(|e| format!("URL parse error: {e}"))?;
            let mod_id = pollster::block_on(
                self.runtime
                    .load_side_es_module_from_code(&url, source.to_string()),
            )
            .map_err(|e| {
                log::warn!("load_side_es_module_from_code error for {url}: {e:?}");
                format!("Module load error: {e}")
            })?;

            let eval_res = self.runtime.mod_evaluate(mod_id);
            pollster::block_on(eval_res).map_err(|e| {
                log::warn!("mod_evaluate error for {url}: {e:?}");
                format!("Module eval error: {e}")
            })?;
            Ok::<(), String>(())
        }));
        std::panic::set_hook(prev_hook);
        let timed_out = watchdog.finish();

        if timed_out {
            log::warn!(
                "JS module '{name}' timed out after {:?} and was safely terminated",
                effective_timeout
            );
            return Err(format!(
                "JS module execution timed out after {:?}",
                effective_timeout
            ));
        }

        match res {
            Ok(Ok(_)) => {
                let heap = self
                    .runtime
                    .v8_isolate()
                    .get_heap_statistics()
                    .used_heap_size();
                log::debug!(
                    "ES Module '{name}' executed successfully ({} bytes, {} MiB V8 heap)",
                    source.len(),
                    heap / (1024 * 1024)
                );
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
                Ok(())
            }
            Ok(Err(err_msg)) => {
                log::warn!("{err_msg}");
                Err(err_msg)
            }
            Err(panic_err) => {
                let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown JS module panic".to_string()
                };
                let err_msg = format!("JS Module Panic in '{name}' caught safely: {msg}");
                log::warn!("{err_msg}");
                Err(err_msg)
            }
        }
    }

    /// Executes a JavaScript source string (e.g., inline `<script>` content or DOM event dispatch)
    /// with an automatic watchdog timeout to prevent UI freezes and Wayland Force Quit dialogs.
    pub fn execute_script(&mut self, source: &str) -> Result<(), String> {
        self.execute_script_with_name_and_timeout(
            "<script>",
            source,
            std::time::Duration::from_millis(1500),
        )
    }

    /// Computes a scaled execution timeout based on script size.
    /// Small scripts (< 20 KB) retain their caller-provided base timeout.
    /// Large scripts (e.g. YouTube bundles of 1MB-5MB) receive an additional
    /// 2 ms per KB of source, capped at 5,000 ms. The cap is a hard upper
    /// bound: a large script can never run longer than five seconds, and no
    /// input can reduce a caller's explicit timeout.
    pub fn compute_scaled_timeout(source_len: usize, base_timeout: Duration) -> Duration {
        if source_len < LARGE_SCRIPT_THRESHOLD {
            return base_timeout;
        }
        let extra_kb = (source_len - LARGE_SCRIPT_THRESHOLD) / 1024;
        let extra_ms = (extra_kb as u64) * 2;
        let scaled = base_timeout.saturating_add(Duration::from_millis(extra_ms));
        scaled.min(MAX_LARGE_SCRIPT_TIMEOUT)
    }

    /// Executes a named JavaScript source string with a maximum execution timeout.
    /// If the script exceeds the timeout, it is safely terminated via V8 isolate handle,
    /// preventing laptop freezes and OS 'Force Quit' dialogs.
    pub fn execute_script_with_name_and_timeout(
        &mut self,
        name: &str,
        source: &str,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        self.enforce_heap_budget()?;
        let effective_timeout = Self::compute_scaled_timeout(source.len(), timeout);
        let _guard = self.enter_isolate();
        let isolate_handle = self.runtime.v8_isolate().thread_safe_handle();
        let watchdog =
            ExecutionWatchdog::new(isolate_handle, effective_timeout, "mango-js-watchdog");

        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let name_owned = name.to_string();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.runtime.execute_script(name_owned, source.to_string())
        }));
        std::panic::set_hook(prev_hook);
        let timed_out = watchdog.finish();

        if timed_out {
            log::warn!(
                "JS execution of '{name}' timed out after {:?} and was safely terminated",
                effective_timeout
            );
            return Err(format!(
                "JS execution timed out after {:?}",
                effective_timeout
            ));
        }

        match res {
            Ok(Ok(_)) => {
                let heap = self
                    .runtime
                    .v8_isolate()
                    .get_heap_statistics()
                    .used_heap_size();
                log::debug!(
                    "Script '{name}' executed successfully ({} bytes, {} MiB V8 heap)",
                    source.len(),
                    heap / (1024 * 1024)
                );
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
                Ok(())
            }
            Ok(Err(e)) => {
                let err_str = e.to_string();
                if err_str.contains("execution terminated") {
                    log::warn!("JS execution of '{name}' was terminated");
                    return Err(format!("JS execution of '{name}' was terminated"));
                }
                if err_str.contains("Cannot use import statement outside a module")
                    || err_str.contains("Unexpected token 'export'")
                {
                    log::info!("Script '{name}' contains ES module syntax, executing as ES module");
                    return self.execute_module_with_name_and_timeout(
                        name,
                        source,
                        effective_timeout,
                    );
                }
                let err_msg = format!("JS Error in '{name}': {e}");
                log::warn!("{err_msg}");
                Err(err_msg)
            }
            Err(panic_err) => {
                let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown JS panic".to_string()
                };
                let err_msg = format!("JS Panic in '{name}' caught safely: {msg}");
                log::warn!("{err_msg}");
                Err(err_msg)
            }
        }
    }

    /// Executes a JavaScript source string with a maximum execution timeout.
    /// If the script exceeds the timeout, it is safely terminated via V8 isolate handle,
    /// preventing laptop freezes and OS 'Force Quit' dialogs.
    pub fn execute_script_with_timeout(
        &mut self,
        source: &str,
        timeout: std::time::Duration,
    ) -> Result<(), String> {
        self.execute_script_with_name_and_timeout("<script>", source, timeout)
    }

    /// Evaluates a JavaScript expression and returns its string result.
    pub fn eval(&mut self, source: &str) -> Result<String, String> {
        let _guard = self.enter_isolate();
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.runtime.execute_script("<eval>", source.to_string())
        }));
        std::panic::set_hook(prev_hook);

        match res {
            Ok(Ok(global_val)) => {
                let context_global = self.runtime.main_context();
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
                let context_local = deno_core::v8::Local::new(scope, context_global);
                let scope = &mut deno_core::v8::ContextScope::new(scope, context_local);
                let local = deno_core::v8::Local::new(scope, global_val);
                let str_val = local.to_rust_string_lossy(scope);
                Ok(str_val)
            }
            Ok(Err(e)) => {
                let err_msg = format!("JS Error: {e}");
                log::warn!("{err_msg}");
                Err(err_msg)
            }
            Err(panic_err) => {
                let msg = if let Some(s) = panic_err.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_err.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "Unknown JS panic".to_string()
                };
                let err_msg = format!("JS Panic caught safely: {msg}");
                log::warn!("{err_msg}");
                Err(err_msg)
            }
        }
    }

    /// Processes any expired timers and drains promise microtasks with watchdog protection.
    /// Returns `true` if the DOM was mutated.
    pub fn tick(&mut self) -> bool {
        // Keep one page from monopolising the UI thread. Remaining callbacks
        // stay queued and are serviced on later browser frames.
        const MAX_CALLBACKS_PER_TICK: usize = 32;
        let scripts = self
            .event_loop
            .borrow_mut()
            .drain_due_limited(MAX_CALLBACKS_PER_TICK);
        let mut timer_fired = false;
        for s in scripts {
            if self
                .execute_script_with_name_and_timeout(
                    "<event_loop>",
                    &s,
                    std::time::Duration::from_millis(1500),
                )
                .is_ok()
            {
                timer_fired = true;
            }
        }

        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        {
            let isolate_ptr: *mut deno_core::v8::Isolate = &mut **self.runtime.v8_isolate();
            unsafe {
                (*isolate_ptr).enter();
            }
            let _ = self
                .runtime
                .poll_event_loop(&mut cx, deno_core::PollEventLoopOptions::default());
            unsafe {
                (*isolate_ptr).exit();
            }
        }

        let dirty = *self.dom_dirty.borrow();
        if dirty {
            *self.dom_dirty.borrow_mut() = false;
        }
        dirty || timer_fired
    }

    /// Whether any timer, animation frame, or queued task is still waiting.
    pub fn has_pending_event_loop_work(&self) -> bool {
        self.event_loop.borrow().pending_count() > 0
    }

    /// Runs exactly one animation-frame turn with pending timers expedited.
    pub fn run_animation_frame(&mut self) {
        self.event_loop.borrow_mut().expedite_timers();
        let _ = self.tick();
        self.run_jobs();
    }

    /// Runs up to `max_frames` event-loop turns, treating every pending timer
    /// (including `requestAnimationFrame` callbacks) as already due.
    ///
    /// This is the deterministic equivalent of waiting through animation frames:
    /// it exercises the same callback code without depending on wall-clock time,
    /// which keeps reftest runs fast and repeatable.
    pub fn pump_event_loop(&mut self, max_frames: usize) {
        for _ in 0..max_frames {
            let pending = self.event_loop.borrow_mut().expedite_timers();
            if pending == 0 {
                self.run_jobs();
                break;
            }
            let _ = self.tick();
            self.run_jobs();
        }
    }

    /// Drains any queued Promise microtasks, MutationObserver callbacks, or deferred jobs.
    pub fn run_jobs(&mut self) {
        {
            let _guard = self.enter_isolate();
            deno_core::v8::scope!(scope, self.runtime.v8_isolate());
            scope.perform_microtask_checkpoint();
        }

        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        {
            let isolate_ptr: *mut deno_core::v8::Isolate = &mut **self.runtime.v8_isolate();
            unsafe {
                (*isolate_ptr).enter();
            }
            let _ = self
                .runtime
                .poll_event_loop(&mut cx, deno_core::PollEventLoopOptions::default());
            unsafe {
                (*isolate_ptr).exit();
            }
        }
    }

    /// Clears all timers (called on page navigation).
    pub fn clear_timers(&mut self) {
        self.event_loop.borrow_mut().clear_all();
    }

    /// Returns true if there are pending timers.
    pub fn has_pending_timers(&self) -> bool {
        self.event_loop.borrow().has_pending_timers()
    }

    /// Returns the earliest deadline when a timer or background task needs execution.
    pub fn next_timer_deadline(&self) -> Option<std::time::Instant> {
        self.event_loop.borrow().next_deadline()
    }

    /// Returns the V8 heap usage in bytes, for tab memory accounting.
    pub fn heap_size_bytes(&mut self) -> usize {
        let _guard = self.enter_isolate();
        self.runtime
            .v8_isolate()
            .get_heap_statistics()
            .used_heap_size()
    }

    /// Forces a complete garbage collection cycle to free unreferenced JS objects.
    pub fn force_gc(&mut self) {
        self.gc();
    }

    /// Requests a V8 low memory notification garbage collection cycle.
    pub fn gc(&mut self) {
        {
            let _guard = self.enter_isolate();
            self.runtime.v8_isolate().low_memory_notification();
            self.runtime
                .v8_isolate()
                .request_garbage_collection_for_testing(deno_core::v8::GarbageCollectionType::Full);
            // FinalizationRegistry callbacks are queued as microtasks/tasks
            // after the collection. Run the checkpoint now; the event-loop
            // drain below executes the cleanup tasks V8 posts for finalized
            // wrappers.
            deno_core::v8::scope!(scope, self.runtime.v8_isolate());
            scope.perform_microtask_checkpoint();
        }
        self.run_jobs();
    }

    pub(crate) fn eval_bool_script(&mut self, script: &str) -> Result<bool, String> {
        let _guard = self.enter_isolate();
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.runtime
                .execute_script("<dispatch>", script.to_string())
        }));
        std::panic::set_hook(prev_hook);

        match res {
            Ok(Ok(global_val)) => {
                let context_global = self.runtime.main_context();
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
                let context_local = deno_core::v8::Local::new(scope, context_global);
                let scope = &mut deno_core::v8::ContextScope::new(scope, context_local);
                let local = deno_core::v8::Local::new(scope, global_val);
                Ok(local.boolean_value(scope))
            }
            Ok(Err(e)) => Err(format!("Dispatch error: {e}")),
            Err(_) => Err("Dispatch panic".to_string()),
        }
    }
}
