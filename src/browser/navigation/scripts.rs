//! Pending script queuing, prioritization, and execution.

use super::super::BrowserChrome;
use super::super::types::{ScheduledScript, ScriptToRun};

impl BrowserChrome {
    pub(crate) fn enqueue_pending_script(&mut self, script: ScriptToRun) {
        let kind = if script.is_module() {
            crate::browser::resource_scheduler::ResourceKind::Module
        } else {
            crate::browser::resource_scheduler::ResourceKind::Script
        };
        let priority = if script.is_defer() {
            crate::browser::resource_scheduler::ResourcePriority::Defer
        } else if script.is_async() {
            crate::browser::resource_scheduler::ResourcePriority::Async
        } else {
            crate::browser::resource_scheduler::ResourcePriority::Blocking
        };
        let url = match &script {
            ScriptToRun::Inline { .. } => "inline://document".to_string(),
            ScriptToRun::External { src, .. } | ScriptToRun::Preloaded { src, .. } => src.clone(),
        };
        let resource_id = self.resource_scheduler.claim_url(&url).unwrap_or_else(|| {
            let id = self.resource_scheduler.discover(kind, priority, url);
            self.resource_scheduler.transition(
                id,
                crate::browser::resource_scheduler::ResourceState::Queued,
            );
            id
        });
        self.pending_scripts.push_back(ScheduledScript {
            script,
            resource_id,
        });
    }

    /// Executes a single pending deferred/async script with a watchdog timeout.
    pub(crate) fn execute_pending_script(&mut self, s: ScriptToRun) {
        let Some(ref mut rt) = self.js_runtime else {
            return;
        };
        let script_timeout = |code_len: usize| -> std::time::Duration {
            let extra_ms = (code_len / 10_000) as u64 * 100;
            std::time::Duration::from_millis(1500 + extra_ms)
                .min(std::time::Duration::from_millis(3000))
        };
        match s {
            ScriptToRun::Inline { code, is_module } => {
                let to = script_timeout(code.len());
                let res = if is_module {
                    rt.execute_module_with_timeout(&code, to)
                } else {
                    rt.execute_script_with_timeout(&code, to)
                };
                if let Err(e) = res {
                    log::warn!("Pending inline script error: {e}");
                }
            }
            ScriptToRun::Preloaded {
                code,
                src,
                is_module,
                ..
            } => {
                let to = script_timeout(code.len());
                let script_name = self
                    .base_url
                    .as_ref()
                    .and_then(|b| b.resolve(&src).ok())
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| src.clone());
                let res = if is_module {
                    rt.execute_module_with_name_and_timeout(&script_name, &code, to)
                } else {
                    rt.execute_script_with_name_and_timeout(&script_name, &code, to)
                };
                if let Err(e) = res {
                    log::warn!("Pending preloaded script error ({src}): {e}");
                }
            }
            ScriptToRun::External { src, is_module, .. } => {
                let Some(ref base) = self.base_url else {
                    return;
                };
                let Ok(resolved) = base.resolve(&src) else {
                    return;
                };
                let key = resolved.as_str();
                let script_code = if let Some(cached) = self.loader.cache().get(&key) {
                    Some(String::from_utf8_lossy(&cached.body).into_owned())
                } else {
                    log::warn!(
                        "Resource failed: pending external script unavailable in cache: {src}"
                    );
                    self.devtools.log(
                        crate::devtools::ConsoleLevel::Error,
                        format!(
                            "Resource failed: pending external script unavailable in cache: {src}"
                        ),
                    );
                    self.resource_scheduler.finish_url(
                        &src,
                        crate::browser::resource_scheduler::ResourceState::Failed,
                        Some(crate::browser::resource_scheduler::RejectReason::NetworkFailure),
                        0,
                    );
                    None
                };
                if let Some(code) = script_code {
                    let to = script_timeout(code.len());
                    let res = if is_module {
                        rt.execute_module_with_name_and_timeout(&key, &code, to)
                    } else {
                        rt.execute_script_with_name_and_timeout(&key, &code, to)
                    };
                    if let Err(e) = res {
                        log::warn!("Pending external script error ({src}): {e}");
                    }
                }
            }
        }
    }
}
