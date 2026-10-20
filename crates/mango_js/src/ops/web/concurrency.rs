use deno_core::{OpState, op2};

use crate::ops::web::origin::get_page_origin;
use crate::web_apis::SharedEventLoop;

// ── BroadcastChannel (WHATWG HTML § 9.4) ───────────────────────────────────

#[op2(fast)]
pub fn op_mango_broadcast_create(state: &mut OpState, channel_id: u32, #[string] name: String) {
    let origin = get_page_origin(state);
    let tasks = state
        .try_borrow::<SharedEventLoop>()
        .map(|el| el.borrow().task_queue());
    if let Some(hub) = state.try_borrow::<crate::broadcast_channel::BroadcastChannelHub>() {
        hub.subscribe(&origin, &name, channel_id as u64, tasks, None);
    }
}

#[op2(fast)]
pub fn op_mango_broadcast_post(
    state: &mut OpState,
    channel_id: u32,
    #[string] name: String,
    #[string] message: String,
) -> u32 {
    let origin = get_page_origin(state);
    if let Some(hub) = state.try_borrow::<crate::broadcast_channel::BroadcastChannelHub>() {
        hub.broadcast(&origin, &name, channel_id as u64, &message) as u32
    } else {
        0
    }
}

#[op2(fast)]
pub fn op_mango_broadcast_close(state: &mut OpState, channel_id: u32, #[string] name: String) {
    let origin = get_page_origin(state);
    if let Some(hub) = state.try_borrow::<crate::broadcast_channel::BroadcastChannelHub>() {
        hub.unsubscribe(&origin, &name, channel_id as u64);
    }
}

// ── SharedWorker (WHATWG HTML § 10.1.2) ─────────────────────────────────────

#[op2(fast)]
pub fn op_mango_shared_worker_connect(
    state: &mut OpState,
    port_id: u32,
    #[string] script_url: String,
    #[string] name: String,
) {
    let origin = get_page_origin(state);
    let tasks = if let Some(el) = state.try_borrow::<SharedEventLoop>() {
        el.borrow().task_queue()
    } else {
        return;
    };
    let blobs = if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        wm.blob_urls()
    } else {
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()))
    };

    if let Some(hub) = state.try_borrow::<crate::shared_worker::SharedWorkerHub>() {
        let key = crate::shared_worker::SharedWorkerKey::new(origin, script_url, name);
        let _ = hub.connect(key, port_id as u64, tasks, blobs);
    }
}

#[op2(fast)]
pub fn op_mango_shared_worker_post(state: &mut OpState, port_id: u32, #[string] message: String) {
    if let Some(hub) = state.try_borrow::<crate::shared_worker::SharedWorkerHub>() {
        let _ = hub.post_message(port_id as u64, &message);
    }
}

#[op2(fast)]
pub fn op_mango_shared_worker_close_port(state: &mut OpState, port_id: u32) {
    if let Some(hub) = state.try_borrow::<crate::shared_worker::SharedWorkerHub>() {
        hub.close_port(port_id as u64);
    }
}

// ── Web Locks API (W3C Web Locks) ───────────────────────────────────────────

#[op2]
#[string]
pub fn op_mango_locks_request(
    state: &mut OpState,
    #[string] name: String,
    #[string] mode: String,
    if_available: bool,
    steal: bool,
    #[string] client_id: String,
) -> String {
    let origin = get_page_origin(state);
    let lock_mode = crate::web_locks::LockMode::parse(&mode);
    if let Some(engine) = state.try_borrow::<crate::web_locks::WebLocksEngine>() {
        let res = engine.request(&origin, &name, lock_mode, if_available, steal, &client_id);
        match res {
            crate::web_locks::LockResult::Granted(g) => {
                format!(
                    r#"{{"status":"granted","grantId":{},"mode":"{}"}}"#,
                    g.grant_id,
                    g.mode.as_str()
                )
            }
            crate::web_locks::LockResult::Queued(q) => {
                format!(
                    r#"{{"status":"queued","requestId":{},"mode":"{}"}}"#,
                    q.request_id,
                    q.mode.as_str()
                )
            }
            crate::web_locks::LockResult::Unavailable => r#"{"status":"unavailable"}"#.to_string(),
        }
    } else {
        r#"{"status":"unavailable"}"#.to_string()
    }
}

#[op2]
#[string]
pub fn op_mango_locks_release(
    state: &mut OpState,
    #[string] name: String,
    grant_id: u32,
) -> String {
    let origin = get_page_origin(state);
    if let Some(engine) = state.try_borrow::<crate::web_locks::WebLocksEngine>() {
        let unblocked = engine.release(&origin, &name, grant_id as u64);
        let items: Vec<String> = unblocked
            .iter()
            .map(|g| {
                format!(
                    r#"{{"grantId":{},"clientId":"{}","mode":"{}"}}"#,
                    g.grant_id,
                    g.client_id,
                    g.mode.as_str()
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    } else {
        "[]".to_string()
    }
}

#[op2(fast)]
pub fn op_mango_locks_abort(state: &mut OpState, #[string] name: String, request_id: u32) -> bool {
    let origin = get_page_origin(state);
    if let Some(engine) = state.try_borrow::<crate::web_locks::WebLocksEngine>() {
        engine.abort_pending(&origin, &name, request_id as u64)
    } else {
        false
    }
}

#[op2]
#[string]
pub fn op_mango_locks_query(state: &mut OpState) -> String {
    let origin = get_page_origin(state);
    if let Some(engine) = state.try_borrow::<crate::web_locks::WebLocksEngine>() {
        let snapshot = engine.query(&origin);
        serde_json::to_string(&snapshot)
            .unwrap_or_else(|_| r#"{"held":[],"pending":[]}"#.to_string())
    } else {
        r#"{"held":[],"pending":[]}"#.to_string()
    }
}
