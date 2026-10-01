//! Console API bindings for JavaScript.
//!
//! Implements `console.log()`, `console.warn()`, `console.error()`, and `console.info()`
//! by delegating to Rust's `log` crate and storing messages for future DevTools display.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{Context, JsValue, NativeFunction};

/// Severity level of a console message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
}

/// A single console message captured from JavaScript.
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    pub level: ConsoleLevel,
    pub text: String,
}

/// Shared console message buffer that both JS and Rust can access.
pub type ConsoleBuffer = Rc<RefCell<Vec<ConsoleMessage>>>;

/// Creates a new shared console buffer.
pub fn new_console_buffer() -> ConsoleBuffer {
    Rc::new(RefCell::new(Vec::new()))
}

/// Registers the `console` object on the global scope of the given Boa context.
pub fn register_console(context: &mut Context, buffer: ConsoleBuffer) {
    let log_fn = make_console_fn(ConsoleLevel::Log, buffer.clone());
    let warn_fn = make_console_fn(ConsoleLevel::Warn, buffer.clone());
    let error_fn = make_console_fn(ConsoleLevel::Error, buffer.clone());
    let info_fn = make_console_fn(ConsoleLevel::Info, buffer.clone());

    let console_obj = boa_engine::object::ObjectInitializer::new(context)
        .function(log_fn, boa_engine::js_string!("log"), 0)
        .function(warn_fn, boa_engine::js_string!("warn"), 0)
        .function(error_fn, boa_engine::js_string!("error"), 0)
        .function(info_fn, boa_engine::js_string!("info"), 0)
        .build();

    context
        .register_global_property(
            boa_engine::js_string!("console"),
            console_obj,
            boa_engine::property::Attribute::all(),
        )
        .expect("failed to register console");
}

/// Creates a native function that formats all arguments and stores them in the buffer.
fn make_console_fn(level: ConsoleLevel, buffer: ConsoleBuffer) -> NativeFunction {
    // SAFETY: The closure captures only Rc (thread-local) references.
    // Mango is single-threaded, so this is safe.
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let parts: Vec<String> = args
                .iter()
                .map(|arg| {
                    arg.to_string(ctx)
                        .map(|s| s.to_std_string_escaped())
                        .unwrap_or_else(|_| "[object]".to_string())
                })
                .collect();
            let text = parts.join(" ");

            match level {
                ConsoleLevel::Log | ConsoleLevel::Info => log::info!("[JS console] {}", text),
                ConsoleLevel::Warn => log::warn!("[JS console] {}", text),
                ConsoleLevel::Error => log::error!("[JS console] {}", text),
            }

            buffer.borrow_mut().push(ConsoleMessage { level, text });

            Ok(JsValue::undefined())
        })
    }
}
