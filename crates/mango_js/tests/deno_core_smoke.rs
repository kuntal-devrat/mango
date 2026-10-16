use deno_core::{extension, op2, JsRuntime, RuntimeOptions};

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

    let res = runtime.execute_script("test.js", "
        const s = Deno.core.ops.op_test_string('world');
        if (s !== 'hello world') throw new Error('string mismatch: ' + s);
        const v = Deno.core.ops.op_test_vec(['a', 'b']);
        if (v.length !== 2 || v[0] !== 'a') throw new Error('vec mismatch');
        const o = Deno.core.ops.op_test_opt(null);
        if (o !== null) throw new Error('opt null mismatch');
        const o2 = Deno.core.ops.op_test_opt('foo');
        if (o2 !== 'foo') throw new Error('opt val mismatch');
    ");
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
    let mod_id = pollster::block_on(runtime.load_main_es_module_from_code(&url, code.to_string())).unwrap();
    let eval_res = runtime.mod_evaluate(mod_id);
    pollster::block_on(eval_res).unwrap();
    
    let global_val = runtime.execute_script("check", "globalThis.testVar").unwrap();
    let context_global = runtime.main_context();
    deno_core::v8::scope!(scope, runtime.v8_isolate());
    let context_local = deno_core::v8::Local::new(scope, context_global);
    let scope = &mut deno_core::v8::ContextScope::new(scope, context_local);
    let local = deno_core::v8::Local::new(scope, global_val);
    assert_eq!(local.to_rust_string_lossy(scope), "42");
}
