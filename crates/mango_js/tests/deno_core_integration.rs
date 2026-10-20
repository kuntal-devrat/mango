use deno_core::{JsRuntime, RuntimeOptions, extension, op2};

#[op2]
#[string]
fn op_test_string(#[string] input: String) -> String {
    format!("hello {}", input)
}

#[op2]
#[serde]
fn op_test_vec(#[serde] input: Vec<String>) -> Vec<String> {
    input
}

#[op2]
#[string]
fn op_test_opt(#[string] input: Option<String>) -> Option<String> {
    input
}

extension!(
    mango_smoke,
    ops = [op_test_string, op_test_vec, op_test_opt],
);

#[test]
fn test_deno_core_types() {
    let mut runtime = JsRuntime::new(RuntimeOptions {
        extensions: vec![mango_smoke::init()],
        ..Default::default()
    });

    let res = runtime.execute_script(
        "test.js",
        "
        const s = Deno.core.ops.op_test_string('world');
        if (s !== 'hello world') throw new Error('string mismatch: ' + s);
        const v = Deno.core.ops.op_test_vec(['a', 'b']);
        if (v.length !== 2 || v[0] !== 'a') throw new Error('vec mismatch');
        const o = Deno.core.ops.op_test_opt(null);
        if (o !== null) throw new Error('opt null mismatch');
        const o2 = Deno.core.ops.op_test_opt('foo');
        if (o2 !== 'foo') throw new Error('opt val mismatch');
    ",
    );
    assert!(res.is_ok(), "test failed: {:?}", res);
    let global_val = runtime.execute_script("eval_test", "40 + 2").unwrap();
    let context_global = runtime.main_context();
    deno_core::v8::scope!(scope, runtime.v8_isolate());
    let context_local = deno_core::v8::Local::new(scope, context_global);
    let scope = &mut deno_core::v8::ContextScope::new(scope, context_local);
    let local = deno_core::v8::Local::new(scope, global_val);
    let str_val = local.to_rust_string_lossy(scope);
    assert_eq!(str_val, "42");
}

#[test]
fn test_deno_core_module() {
    let mut runtime = JsRuntime::new(RuntimeOptions::default());
    let url = deno_core::url::Url::parse("mango://test_mod.js").unwrap();
    let code = "
        globalThis.testVar = 42;
        export const x = 10;
    ";
    let mod_id =
        pollster::block_on(runtime.load_main_es_module_from_code(&url, code.to_string())).unwrap();
    let eval_res = runtime.mod_evaluate(mod_id);
    pollster::block_on(eval_res).unwrap();

    let global_val = runtime
        .execute_script("check", "globalThis.testVar")
        .unwrap();
    let context_global = runtime.main_context();
    deno_core::v8::scope!(scope, runtime.v8_isolate());
    let context_local = deno_core::v8::Local::new(scope, context_global);
    let scope = &mut deno_core::v8::ContextScope::new(scope, context_local);
    let local = deno_core::v8::Local::new(scope, global_val);
    assert_eq!(local.to_rust_string_lossy(scope), "42");
}

struct IsolateScopeGuard(*mut deno_core::v8::Isolate);
impl IsolateScopeGuard {
    fn new(isolate: &mut deno_core::v8::Isolate) -> Self {
        unsafe {
            isolate.enter();
        }
        Self(isolate as *mut _)
    }
}
impl Drop for IsolateScopeGuard {
    fn drop(&mut self) {
        unsafe {
            (*self.0).exit();
        }
    }
}

struct WrappedRuntime(JsRuntime);
impl WrappedRuntime {
    fn new() -> Self {
        let mut rt = JsRuntime::new(RuntimeOptions::default());
        unsafe {
            rt.v8_isolate().exit();
        }
        Self(rt)
    }
    fn eval(&mut self, code: &str) -> String {
        let _guard = IsolateScopeGuard::new(self.0.v8_isolate());
        let val = self.0.execute_script("test", code.to_string()).unwrap();
        let context = self.0.main_context();
        deno_core::v8::scope!(scope, self.0.v8_isolate());
        let context = deno_core::v8::Local::new(scope, context);
        let scope = &mut deno_core::v8::ContextScope::new(scope, context);
        let local = deno_core::v8::Local::new(scope, val);
        local.to_rust_string_lossy(scope)
    }
}
impl Drop for WrappedRuntime {
    fn drop(&mut self) {
        unsafe {
            self.0.v8_isolate().enter();
        }
    }
}

#[test]
fn test_arbitrary_runtimes_drop_order() {
    let mut rt1 = WrappedRuntime::new();
    let mut rt2 = WrappedRuntime::new();
    let mut rt3 = WrappedRuntime::new();

    assert_eq!(rt1.eval("1 + 1"), "2");
    assert_eq!(rt2.eval("2 + 2"), "4");
    assert_eq!(rt3.eval("3 + 3"), "6");

    // Drop rt2 first!
    drop(rt2);

    assert_eq!(rt1.eval("10 + 10"), "20");
    assert_eq!(rt3.eval("30 + 30"), "60");

    // Drop rt1 next!
    drop(rt1);

    assert_eq!(rt3.eval("100 + 100"), "200");

    // Drop rt3 last!
    drop(rt3);
}
