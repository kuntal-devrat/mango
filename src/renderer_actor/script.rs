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

    if !is_mod {
        if let Some(node_id) = script.node_id() {
            rt.set_current_script(Some(node_id));
        } else if let Some(src) = script.src() {
            let script_name = base_url
                .and_then(|b| b.resolve(src).ok())
                .map(|u| u.to_string())
                .unwrap_or_else(|| src.to_string());
            rt.set_current_script_url(&script_name);
        }
    } else {
        rt.set_current_script(None);
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
                rt.set_current_script(None);
                return;
            }
        }
    }

    if !is_mod {
        rt.set_current_script(None);
    }
}
