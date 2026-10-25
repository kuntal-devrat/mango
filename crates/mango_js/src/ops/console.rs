use crate::console::{ConsoleBuffer, ConsoleLevel, ConsoleMessage};
use deno_core::{OpState, op2};

#[op2(fast)]
pub fn op_mango_console(state: &mut OpState, level: u32, #[string] text: String) {
    let msg_level = match level {
        0 => ConsoleLevel::Log,
        1 => ConsoleLevel::Info,
        2 => ConsoleLevel::Warn,
        3 => ConsoleLevel::Error,
        _ => ConsoleLevel::Log,
    };

    eprintln!("[JS console] {text}");
    match msg_level {
        ConsoleLevel::Log | ConsoleLevel::Info => log::info!("[JS console] {}", text),
        ConsoleLevel::Warn => log::warn!("[JS console] {}", text),
        ConsoleLevel::Error => log::error!("[JS console] {}", text),
    }

    if let Some(buffer) = state.try_borrow::<ConsoleBuffer>() {
        buffer.borrow_mut().push(ConsoleMessage {
            level: msg_level,
            text,
        });
    }
}
