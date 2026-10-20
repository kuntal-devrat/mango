use deno_core::{OpState, op2};

use crate::web_apis::SharedEventLoop;

// ── Web Workers ──────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_worker_create(state: &mut OpState, id: u32, #[string] script_url: String) {
    let page_url = state
        .try_borrow::<crate::ops::dom::PageUrl>()
        .map(|p| p.0.clone())
        .unwrap_or_default();
    let tasks = if let Some(event_loop) = state.try_borrow::<SharedEventLoop>() {
        event_loop.borrow().task_queue()
    } else {
        return;
    };
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        wm.create_worker(id as u64, &script_url, &page_url, tasks);
    }
}

#[op2(fast)]
pub fn op_mango_worker_post_message(
    state: &mut OpState,
    worker_id: u32,
    #[string] message: String,
) {
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        let _ = wm.post_message(worker_id as u64, &message);
    }
}

#[op2(fast)]
pub fn op_mango_worker_terminate(state: &mut OpState, worker_id: u32) {
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        wm.terminate(worker_id as u64);
    }
}

#[op2(fast)]
pub fn op_mango_blob_register(
    state: &mut OpState,
    #[string] blob_id: String,
    #[string] data: String,
) {
    if let Some(wm) = state.try_borrow::<crate::worker::WorkerManager>() {
        wm.register_blob(blob_id, data);
    }
}
