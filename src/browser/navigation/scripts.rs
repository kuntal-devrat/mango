//! Script scheduling and execution handler.
//!
//! The document scheduler owns admission and lifecycle state. This module owns
//! script payload resolution and execution, then reports an outcome for the
//! scheduler to record against the script's stable resource ID.

use super::super::BrowserChrome;
use super::super::resource_scheduler::{
    RejectReason, ResourceKind, ResourcePriority, ResourceState,
};
use super::super::types::{ScheduledScript, ScriptToRun};

fn resource_url(script: &ScriptToRun) -> String {
    match script {
        ScriptToRun::Inline { .. } => "inline://document".to_string(),
        ScriptToRun::External { src, .. } | ScriptToRun::Preloaded { src, .. } => src.clone(),
    }
}

fn resource_priority(script: &ScriptToRun) -> ResourcePriority {
    if script.is_defer() {
        ResourcePriority::Defer
    } else if script.is_async() {
        ResourcePriority::Async
    } else {
        ResourcePriority::Blocking
    }
}

fn script_timeout(code_len: usize) -> std::time::Duration {
    let extra_ms = (code_len / 10_000) as u64 * 100;
    std::time::Duration::from_millis(1500 + extra_ms).min(std::time::Duration::from_millis(3000))
}

impl BrowserChrome {
    /// Creates one queued resource record and binds it to this script payload.
    pub(crate) fn schedule_script(&mut self, script: ScriptToRun) -> ScheduledScript {
        let kind = if script.is_module() {
            ResourceKind::Module
        } else {
            ResourceKind::Script
        };
        let id = self.resource_scheduler.discover(
            kind,
            resource_priority(&script),
            resource_url(&script),
        );
        self.resource_scheduler
            .transition(id, ResourceState::Queued);
        ScheduledScript {
            script,
            resource_id: id,
        }
    }

    pub(crate) fn enqueue_pending_script(&mut self, script: ScriptToRun) {
        let scheduled = self.schedule_script(script);
        self.enqueue_scheduled_script(scheduled);
    }

    pub(crate) fn enqueue_scheduled_script(&mut self, script: ScheduledScript) {
        self.pending_scripts.push_back(script);
    }

    /// Runs one script resource and records exactly one terminal transition.
    /// `runtime` is supplied by the caller so synchronous navigation and the
    /// incremental event-loop path share this same execution handler.
    pub(crate) fn execute_script_resource(
        &mut self,
        scheduled: ScheduledScript,
        runtime: &mut mango_js::JsRuntime,
    ) -> bool {
        let id = scheduled.resource_id;
        let priority = resource_priority(&scheduled.script);
        if !self
            .resource_scheduler
            .transition(id, ResourceState::Executing)
        {
            return false;
        }

        let started = std::time::Instant::now();
        let outcome = self.execute_script_payload(scheduled.script, runtime);
        if priority != ResourcePriority::Blocking {
            self.resource_scheduler.record_script_cpu(started.elapsed());
        }
        match outcome {
            Ok(bytes) => self.resource_scheduler.complete(id, bytes),
            Err(reason) => self.resource_scheduler.fail(id, reason),
        }
    }

    /// Runs the selected async/deferred resource using the active JS runtime.
    pub(crate) fn execute_pending_script(&mut self, scheduled: ScheduledScript) -> bool {
        let id = scheduled.resource_id;
        let Some(mut runtime) = self.js_runtime.take() else {
            self.resource_scheduler.fail(id, RejectReason::Unsupported);
            return false;
        };
        let completed = self.execute_script_resource(scheduled, &mut runtime);
        self.js_runtime = Some(runtime);
        completed
    }

    fn execute_script_payload(
        &mut self,
        script: ScriptToRun,
        runtime: &mut mango_js::JsRuntime,
    ) -> Result<usize, RejectReason> {
        match script {
            ScriptToRun::Inline { code, is_module } => {
                let timeout = script_timeout(code.len());
                let result = if is_module {
                    runtime.execute_module_with_timeout(&code, timeout)
                } else {
                    runtime.execute_script_with_timeout(&code, timeout)
                };
                if let Err(error) = result {
                    log::warn!("Inline script error: {error}");
                }
                Ok(code.len())
            }
            ScriptToRun::Preloaded {
                src,
                code,
                is_module,
                ..
            } => {
                let name = self
                    .base_url
                    .as_ref()
                    .and_then(|base| base.resolve(&src).ok())
                    .map(|url| url.to_string())
                    .unwrap_or_else(|| src.clone());
                let timeout = script_timeout(code.len());
                let result = if is_module {
                    runtime.execute_module_with_name_and_timeout(&name, &code, timeout)
                } else {
                    runtime.execute_script_with_name_and_timeout(&name, &code, timeout)
                };
                if let Err(error) = result {
                    log::warn!("Preloaded script error ({src}): {error}");
                }
                Ok(code.len())
            }
            ScriptToRun::External { src, is_module, .. } => {
                let base = self.base_url.as_ref().ok_or(RejectReason::InvalidUrl)?;
                let resolved = base.resolve(&src).map_err(|_| RejectReason::InvalidUrl)?;
                let key = resolved.as_str();
                let cached = self.loader.cache().get(&key).ok_or_else(|| {
                    log::warn!("Resource failed: external script unavailable in cache: {src}");
                    self.devtools.log(
                        crate::devtools::ConsoleLevel::Error,
                        format!("Resource failed: external script unavailable in cache: {src}"),
                    );
                    RejectReason::NetworkFailure
                })?;
                let code = String::from_utf8_lossy(&cached.body).into_owned();
                let timeout = script_timeout(code.len());
                let result = if is_module {
                    runtime.execute_module_with_name_and_timeout(&key, &code, timeout)
                } else {
                    runtime.execute_script_with_name_and_timeout(&key, &code, timeout)
                };
                if let Err(error) = result {
                    log::warn!("External script error ({src}): {error}");
                }
                Ok(code.len())
            }
        }
    }
}
