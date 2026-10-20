use deno_core::{OpState, op2};

use crate::web_apis::{
    PendingWindowOpen, SharedEventLoop, SharedPendingNav, SharedPendingOpenWindow, SharedStatusText,
};

// ── Timers ───────────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_schedule_timer(
    state: &mut OpState,
    id: u32,
    delay_ms: u32,
    repeats: bool,
    _is_raf: bool,
) {
    if let Some(event_loop) = state.try_borrow::<SharedEventLoop>() {
        let source = format!(
            "if (typeof _mangoRunTimer === 'function') _mangoRunTimer({});",
            id
        );
        if let Ok(mut el) = event_loop.try_borrow_mut() {
            if repeats {
                el.schedule_interval_with_id(id, source, delay_ms as u64);
            } else {
                el.schedule_timeout_with_id(id, source, delay_ms as u64);
            }
        }
    }
}

#[op2(fast)]
pub fn op_mango_cancel_timer(state: &mut OpState, id: u32) {
    if let Some(event_loop) = state.try_borrow::<SharedEventLoop>()
        && let Ok(mut el) = event_loop.try_borrow_mut()
    {
        el.cancel_timer(id);
    }
}

// ── Alert & Navigation ───────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_alert(state: &mut OpState, #[string] msg: String) {
    log::info!("[JS alert] {}", msg);
    if let Some(status_text) = state.try_borrow::<SharedStatusText>() {
        *status_text.borrow_mut() = format!("Alert: {}", msg);
    }
}

#[op2(fast)]
pub fn op_mango_navigate(state: &mut OpState, #[string] url: String) {
    if !url.is_empty() {
        log::info!("[JS navigation] Script requested navigate to: {}", url);
        if let Some(pending_nav) = state.try_borrow::<SharedPendingNav>() {
            *pending_nav.borrow_mut() = Some(url);
        }
    }
}

#[op2(fast)]
pub fn op_mango_open_window(
    state: &mut OpState,
    #[string] url: String,
    #[string] target: String,
    #[string] features: String,
) {
    log::info!(
        "[JS window.open] Script requested open window: url={}, target={}, features={}",
        url,
        target,
        features
    );
    if let Some(pending_open) = state.try_borrow::<SharedPendingOpenWindow>() {
        *pending_open.borrow_mut() = Some(PendingWindowOpen {
            url,
            target,
            features,
        });
    }
}

// ── Crypto ───────────────────────────────────────────────────────────────────

#[op2]
#[serde]
pub fn op_mango_random_bytes(len: u32) -> Vec<u8> {
    let mut buf = vec![0u8; len.min(65536) as usize];
    crate::web_apis::fill_secure_random(&mut buf);
    buf
}
