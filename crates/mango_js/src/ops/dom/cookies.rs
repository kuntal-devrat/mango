use deno_core::{OpState, op2};

use crate::dom_bindings::{CookieStore, jar_lock};
use crate::ops::dom::types::PageUrl;

#[op2]
#[string]
pub fn op_mango_cookie_get(state: &mut OpState) -> String {
    let page_url = state
        .try_borrow::<PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_else(|| "about:blank".to_string());
    if let Some(cookie_jar) = state.try_borrow::<CookieStore>() {
        let jar = jar_lock(cookie_jar);
        if let Ok(url) = mango_net::Url::parse(&page_url) {
            return jar.script_cookie_string(&url);
        }
    }
    String::new()
}

#[op2(fast)]
pub fn op_mango_cookie_set(state: &mut OpState, #[string] cookie_str: String) {
    let page_url = state
        .try_borrow::<PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_else(|| "about:blank".to_string());
    if let Some(cookie_jar) = state.try_borrow::<CookieStore>() {
        let mut jar = jar_lock(cookie_jar);
        if let Ok(url) = mango_net::Url::parse(&page_url) {
            jar.set_from_script(&cookie_str, &url);
            jar.prune_expired();
        }
    }
}
