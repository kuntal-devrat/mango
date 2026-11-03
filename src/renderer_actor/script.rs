use mango_js::JsRuntime;
use mango_net::{ResourceCache, Url};

use super::types::script_timeout;
use crate::browser::types::ScriptToRun;

/// Executes a single script within the isolated JavaScript runtime.
pub fn run_single_script(
    script: &ScriptToRun,
    rt: &mut JsRuntime,
    base_url: Option<&Url>,
    cache: Option<&ResourceCache>,
) {
    let is_mod = script.is_module();
    let clear_script = "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(null);";

    if !is_mod {
        if let Some(node_id) = script.node_id() {
            let js_id = mango_html::node_id_to_js(node_id);
            let _ = rt.execute_script(&format!(
                "if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript({js_id});"
            ));
        } else if let Some(src) = script.src() {
            let script_name = base_url
                .and_then(|b| b.resolve(src).ok())
                .map(|u| u.to_string())
                .unwrap_or_else(|| src.to_string());
            if let Ok(json_name) = serde_json::to_string(&script_name) {
                let _ = rt.execute_script(&format!(
                    "if (typeof globalThis.__setCurrentScriptUrl === 'function') globalThis.__setCurrentScriptUrl({json_name});"
                ));
            }
        }
    } else {
        let _ = rt.execute_script(clear_script);
    }

    match script {
        ScriptToRun::Inline {
            code, is_module, ..
        } => {
            let to = script_timeout(code.len());
            let res = if *is_module {
                rt.execute_module_with_timeout(code, to)
            } else {
                rt.execute_script_with_timeout(code, to)
            };
            if let Err(e) = res {
                log::warn!("Isolated renderer inline script error: {e}");
            }
        }
        ScriptToRun::Preloaded {
            code,
            src,
            is_module,
            ..
        } => {
            let to = script_timeout(code.len());
            let script_name = base_url
                .and_then(|b| b.resolve(src).ok())
                .map(|u| u.to_string())
                .unwrap_or_else(|| src.clone());
            let res = if *is_module {
                rt.execute_module_with_name_and_timeout(&script_name, code, to)
            } else {
                rt.execute_script_with_name_and_timeout(&script_name, code, to)
            };
            if let Err(e) = res {
                log::warn!("Isolated renderer preloaded script error ({src}): {e}");
            }
        }
        ScriptToRun::External { src, is_module, .. } => {
            let key = base_url
                .and_then(|b| b.resolve(src).ok())
                .map(|u| u.to_string())
                .unwrap_or_else(|| src.clone());
            let script_code = cache
                .and_then(|c| c.get(&key))
                .map(|entry| String::from_utf8_lossy(&entry.body).into_owned());

            if let Some(code) = script_code {
                let to = script_timeout(code.len());
                let res = if *is_module {
                    rt.execute_module_with_name_and_timeout(&key, &code, to)
                } else {
                    rt.execute_script_with_name_and_timeout(&key, &code, to)
                };
                if let Err(e) = res {
                    log::warn!("Isolated renderer external script error ({src}): {e}");
                }
            } else if !is_mod {
                let _ = rt.execute_script(clear_script);
                return;
            }
        }
    }

    if !is_mod {
        let _ = rt.execute_script(clear_script);
    }
}
