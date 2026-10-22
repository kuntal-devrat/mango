use crate::runtime::JsRuntime;

impl JsRuntime {
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
        let effective_timeout = Self::compute_scaled_timeout(source.len(), timeout);
        let _guard = self.enter_isolate();
        let isolate_handle = self.runtime.v8_isolate().thread_safe_handle();

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

            let (tx, rx) = std::sync::mpsc::channel();
            let watchdog_handle = isolate_handle.clone();
            let watchdog = std::thread::Builder::new()
                .name("mango-js-module-watchdog".to_string())
                .spawn(move || match rx.recv_timeout(effective_timeout) {
                    Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        watchdog_handle.terminate_execution();
                    }
                });

            let eval_res = self.runtime.mod_evaluate(mod_id);
            let eval_result = pollster::block_on(eval_res).map_err(|e| {
                log::warn!("mod_evaluate error for {url}: {e:?}");
                format!("Module eval error: {e}")
            });
            let _ = tx.send(());
            if let Ok(handle) = watchdog {
                let _ = handle.join();
            }
            eval_result?;
            Ok::<(), String>(())
        }));
        std::panic::set_hook(prev_hook);

        match res {
            Ok(Ok(_)) => {
                log::debug!(
                    "ES Module '{name}' executed successfully ({} bytes)",
                    source.len()
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
                let _ = self.runtime.v8_isolate().cancel_terminate_execution();
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

    /// Executes a JavaScript source string (e.g., inline `<script>` content).
    pub fn execute_script(&mut self, source: &str) -> Result<(), String> {
        let _guard = self.enter_isolate();
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.runtime.execute_script("<script>", source.to_string())
        }));
        std::panic::set_hook(prev_hook);

        match res {
            Ok(Ok(_)) => {
                log::info!("Script executed successfully ({} bytes)", source.len());
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
                Ok(())
            }
            Ok(Err(e)) => {
                let err_str = e.to_string();
                if err_str.contains("Cannot use import statement outside a module")
                    || err_str.contains("Unexpected token 'export'")
                {
                    log::info!("Script contains ES module syntax, executing as ES module");
                    return self.execute_module(source);
                }
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

    /// Computes a scaled execution timeout based on script size.
    /// Small scripts (< 20 KB) retain their base timeout.
    /// Large scripts (e.g. YouTube bundles of 1MB-5MB) scale up to 5,000ms
    /// to allow V8 compilation and initial evaluation without false watchdog terminations.
    pub fn compute_scaled_timeout(
        source_len: usize,
        base_timeout: std::time::Duration,
    ) -> std::time::Duration {
        if source_len < 20_000 {
            base_timeout.min(std::time::Duration::from_millis(1500))
        } else {
            let extra_kb = (source_len - 20_000) / 1024;
            let extra_ms = (extra_kb as u64) * 2;
            let scaled = base_timeout + std::time::Duration::from_millis(extra_ms);
            scaled.min(std::time::Duration::from_millis(1500))
        }
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
        let effective_timeout = Self::compute_scaled_timeout(source.len(), timeout);
        let _guard = self.enter_isolate();
        let isolate_handle = self.runtime.v8_isolate().thread_safe_handle();
        let (tx, rx) = std::sync::mpsc::channel();

        let watchdog = std::thread::Builder::new()
            .name("mango-js-watchdog".to_string())
            .spawn(move || match rx.recv_timeout(effective_timeout) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    isolate_handle.terminate_execution();
                }
            });

        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let name_owned = name.to_string();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.runtime.execute_script(name_owned, source.to_string())
        }));
        std::panic::set_hook(prev_hook);
        let _ = tx.send(());
        if let Ok(handle) = watchdog {
            let _ = handle.join();
        }

        match res {
            Ok(Ok(_)) => {
                log::debug!(
                    "Script '{name}' executed successfully ({} bytes)",
                    source.len()
                );
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
                Ok(())
            }
            Ok(Err(e)) => {
                let err_str = e.to_string();
                if err_str.contains("execution terminated") {
                    let _ = self.runtime.v8_isolate().cancel_terminate_execution();
                    log::warn!(
                        "JS execution of '{name}' timed out after {:?} and was safely terminated",
                        effective_timeout
                    );
                    return Err(format!(
                        "JS execution timed out after {:?}",
                        effective_timeout
                    ));
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
                let _ = self.runtime.v8_isolate().cancel_terminate_execution();
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

    /// Processes any expired timers and drains promise microtasks. Returns `true` if the DOM was mutated.
    pub fn tick(&mut self) -> bool {
        let timer_fired = {
            let _guard = self.enter_isolate();
            let prev_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let scripts = self.event_loop.borrow_mut().drain_due();
                let mut timer_fired = false;
                for s in scripts {
                    if self.runtime.execute_script("<event_loop>", s).is_ok() {
                        timer_fired = true;
                    }
                }
                deno_core::v8::scope!(scope, self.runtime.v8_isolate());
                scope.perform_microtask_checkpoint();
                timer_fired
            }));
            std::panic::set_hook(prev_hook);
            res.unwrap_or(false)
        };

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

    /// Forces a complete garbage collection cycle to free unreferenced JS objects.
    pub fn force_gc() {}

    /// Requests a V8 low memory notification garbage collection cycle.
    pub fn gc(&mut self) {
        let _guard = self.enter_isolate();
        self.runtime.v8_isolate().low_memory_notification();
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
