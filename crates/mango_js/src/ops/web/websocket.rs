use deno_core::{OpState, op2};

use crate::web_apis::SharedEventLoop;

// ── WebSockets ───────────────────────────────────────────────────────────────

#[op2(fast)]
pub fn op_mango_ws_connect(
    state: &mut OpState,
    id: u32,
    #[string] url: String,
    #[string] protocols: String,
) {
    let tasks = if let Some(event_loop) = state.try_borrow::<SharedEventLoop>() {
        event_loop.borrow().task_queue()
    } else {
        return;
    };
    if let Some(wsm) = state.try_borrow::<mango_net::WebSocketManager>() {
        let ws = wsm.clone();
        ws.connect(id as u64, &url, &protocols, move |event| {
            let cmd = match event {
                mango_net::WebSocketEvent::Open { protocol } => {
                    let p = protocol.replace('\\', "\\\\").replace('"', "\\\"");
                    format!(
                        "if (typeof _mangoDispatchWsOpen === 'function') _mangoDispatchWsOpen({}, \"{}\");",
                        id, p
                    )
                }
                mango_net::WebSocketEvent::Message { data } => {
                    let escaped = data
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r");
                    format!(
                        "if (typeof _mangoDispatchWsMessage === 'function') _mangoDispatchWsMessage({}, \"{}\");",
                        id, escaped
                    )
                }
                mango_net::WebSocketEvent::BinaryMessage { data: _ } => {
                    format!(
                        "if (typeof _mangoDispatchWsMessage === 'function') _mangoDispatchWsMessage({}, \"\");",
                        id
                    )
                }
                mango_net::WebSocketEvent::Error { message } => {
                    let escaped = message
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r");
                    format!(
                        "if (typeof _mangoDispatchWsError === 'function') _mangoDispatchWsError({}, \"{}\");",
                        id, escaped
                    )
                }
                mango_net::WebSocketEvent::Close {
                    code,
                    reason,
                    was_clean,
                } => {
                    let escaped = reason
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r");
                    format!(
                        "if (typeof _mangoDispatchWsClose === 'function') _mangoDispatchWsClose({}, {}, \"{}\", {});",
                        id, code, escaped, was_clean
                    )
                }
            };
            if let Ok(mut q) = tasks.lock() {
                q.push(cmd);
            }
        });
    }
}

#[op2(fast)]
pub fn op_mango_ws_send(state: &mut OpState, ws_id: u32, #[string] data: String) {
    if let Some(wsm) = state.try_borrow::<mango_net::WebSocketManager>() {
        let _ = wsm.send_text(ws_id as u64, data);
    }
}

#[op2(fast)]
pub fn op_mango_ws_close(state: &mut OpState, ws_id: u32, code: u32, #[string] reason: String) {
    if let Some(wsm) = state.try_borrow::<mango_net::WebSocketManager>() {
        let _ = wsm.close(ws_id as u64, code as u16, reason);
    }
}
