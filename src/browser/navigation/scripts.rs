//! Pending script queuing, prioritization, and execution.

use super::super::BrowserChrome;
use super::super::types::{ScheduledScript, ScriptToRun};

pub(crate) fn script_timeout(code_len: usize) -> std::time::Duration {
    let extra_secs = (code_len / 1_000_000) as u64 * 5;
    std::time::Duration::from_secs(30 + extra_secs).min(std::time::Duration::from_secs(60))
}

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
        let is_mod = s.is_module();
        if !is_mod {
            if let Some(node_id) = s.node_id() {
                rt.set_current_script(Some(node_id));
            } else if let Some(src) = s.src() {
                let script_name = self
                    .base_url
                    .as_ref()
                    .and_then(|b| b.resolve(src).ok())
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| src.to_string());
                rt.set_current_script_url(&script_name);
            }
        } else {
            rt.set_current_script(None);
        }
        match s {
            ScriptToRun::Inline {
                code, is_module, ..
            } => {
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
                    if !is_mod {
                        let _ = rt.execute_script(
                            "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                        );
                    }
                    return;
                };
                let Ok(resolved) = base.resolve(&src) else {
                    if !is_mod {
                        let _ = rt.execute_script(
                            "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                        );
                    }
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
        if !is_mod {
            rt.set_current_script(None);
        }
    }

    /// Executes a synchronous script during initial page navigation.
    pub(crate) fn execute_sync_script(
        &mut self,
        s: ScriptToRun,
        rt: &mut mango_js::JsRuntime,
        ext_cnt: &mut usize,
    ) {
        let is_mod = s.is_module();
        let heap_mb = rt.heap_size_bytes() / (1024 * 1024);
        match &s {
            ScriptToRun::Inline { code, .. } => {
                log::info!(
                    "[SCRIPT EXEC] Inline script len={}, V8 heap={}MB",
                    code.len(),
                    heap_mb
                );
            }
            ScriptToRun::Preloaded { src, code, .. } => {
                log::info!(
                    "[SCRIPT EXEC] Preloaded script '{}' len={}, V8 heap={}MB",
                    src,
                    code.len(),
                    heap_mb
                );
            }
            ScriptToRun::External { src, .. } => {
                log::info!(
                    "[SCRIPT EXEC] External script '{}', V8 heap={}MB",
                    src,
                    heap_mb
                );
            }
        }
        if !is_mod {
            if let Some(node_id) = s.node_id() {
                rt.set_current_script(Some(node_id));
            } else if let Some(src) = s.src() {
                let script_name = self
                    .base_url
                    .as_ref()
                    .and_then(|b| b.resolve(src).ok())
                    .map(|u| u.to_string())
                    .unwrap_or_else(|| src.to_string());
                rt.set_current_script_url(&script_name);
            }
        } else {
            rt.set_current_script(None);
        }
        match s {
            ScriptToRun::Inline {
                code, is_module, ..
            } => {
                let to = script_timeout(code.len());
                let res = if is_module {
                    rt.execute_module_with_timeout(&code, to)
                } else {
                    rt.execute_script_with_timeout(&code, to)
                };
                if let Err(e) = res {
                    log::warn!("Inline script error: {e}");
                }
                self.resource_scheduler.finish_url(
                    "inline://document",
                    crate::browser::resource_scheduler::ResourceState::Completed,
                    None,
                    code.len(),
                );
            }
            ScriptToRun::Preloaded {
                code,
                src,
                is_module,
                ..
            } => {
                *ext_cnt += 1;
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
                    log::warn!("Preloaded script error ({src}): {e}");
                }
                self.resource_scheduler.finish_url(
                    &src,
                    crate::browser::resource_scheduler::ResourceState::Completed,
                    None,
                    code.len(),
                );
            }
            ScriptToRun::External { src, is_module, .. } => {
                let Some(ref base) = self.base_url else {
                    if !is_mod {
                        let _ = rt.execute_script(
                            "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                        );
                    }
                    return;
                };
                let Ok(resolved) = base.resolve(&src) else {
                    if !is_mod {
                        let _ = rt.execute_script(
                            "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);",
                        );
                    }
                    return;
                };
                let key = resolved.as_str();
                let script_code = if let Some(cached) = self.loader.cache().get(&key) {
                    Some(String::from_utf8_lossy(&cached.body).into_owned())
                } else {
                    log::warn!("Resource failed: external script unavailable in cache: {src}");
                    self.devtools.log(
                        crate::devtools::ConsoleLevel::Error,
                        format!("Resource failed: external script unavailable in cache: {src}"),
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
                    *ext_cnt += 1;
                    let to = script_timeout(code.len());
                    let res = if is_module {
                        rt.execute_module_with_name_and_timeout(&key, &code, to)
                    } else {
                        rt.execute_script_with_name_and_timeout(&key, &code, to)
                    };
                    if let Err(e) = res {
                        log::warn!("External script error ({src}): {e}");
                    }
                    self.resource_scheduler.finish_url(
                        &src,
                        crate::browser::resource_scheduler::ResourceState::Completed,
                        None,
                        code.len(),
                    );
                }
            }
        };
        if !is_mod {
            rt.set_current_script(None);
        }
    }
}

fn skip_ws_and_comments(bytes: &[u8], mut j: usize) -> usize {
    let len = bytes.len();
    while j < len {
        if bytes[j].is_ascii_whitespace() {
            j += 1;
        } else if j + 1 < len && bytes[j] == b'/' && bytes[j + 1] == b'*' {
            j += 2;
            while j + 1 < len && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            j = (j + 2).min(len);
        } else if j + 1 < len && bytes[j] == b'/' && bytes[j + 1] == b'/' {
            j += 2;
            while j < len && bytes[j] != b'\n' {
                j += 1;
            }
        } else {
            break;
        }
    }
    j
}

pub(crate) fn scan_es_module_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if i + 4 <= len && &bytes[i..i + 4] == b"from" {
            let j = skip_ws_and_comments(bytes, i + 4);
            if j < len && (bytes[j] == b'\'' || bytes[j] == b'"' || bytes[j] == b'`') {
                let quote = bytes[j];
                let start = j + 1;
                let mut end = start;
                while end < len && bytes[end] != quote && bytes[end] != b'\n' {
                    end += 1;
                }
                if end < len && bytes[end] == quote && end > start {
                    if let Ok(path) = std::str::from_utf8(&bytes[start..end]) {
                        let path = path.trim();
                        if !path.is_empty()
                            && !path.contains("${")
                            && !imports.contains(&path.to_string())
                        {
                            imports.push(path.to_string());
                        }
                    }
                }
                i = end + 1;
                continue;
            }
        } else if i + 6 <= len && &bytes[i..i + 6] == b"import" {
            let mut j = skip_ws_and_comments(bytes, i + 6);
            if j < len && bytes[j] == b'(' {
                j = skip_ws_and_comments(bytes, j + 1);
            }
            if j < len && (bytes[j] == b'\'' || bytes[j] == b'"' || bytes[j] == b'`') {
                let quote = bytes[j];
                let start = j + 1;
                let mut end = start;
                while end < len && bytes[end] != quote && bytes[end] != b'\n' {
                    end += 1;
                }
                if end < len && bytes[end] == quote && end > start {
                    if let Ok(path) = std::str::from_utf8(&bytes[start..end]) {
                        let path = path.trim();
                        if !path.is_empty()
                            && !path.contains("${")
                            && !imports.contains(&path.to_string())
                        {
                            imports.push(path.to_string());
                        }
                    }
                }
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    imports
}
