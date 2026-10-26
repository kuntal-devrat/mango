use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mango_html::dom::Document;
use mango_html::parse_html;

use crate::console;
use crate::runtime::JsRuntime;

#[test]
fn test_js_eval_and_console() {
    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let result = rt.execute_script("console.log('Hello from JS engine!', 42);");
    assert!(result.is_ok());

    let messages = rt.drain_console_messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].level, console::ConsoleLevel::Log);
    assert_eq!(messages[0].text, "Hello from JS engine! 42");
}

#[test]
fn test_timers_set_timeout() {
    let html = r#"<html><body><div id="counter">0</div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        setTimeout(function() {
            var el = document.getElementById("counter");
            el.textContent = "1";
        }, 10);
    "#;
    assert!(rt.execute_script(script).is_ok());
    assert!(rt.has_pending_timers());

    // Wait a bit to let the timer expire
    std::thread::sleep(std::time::Duration::from_millis(20));
    let changed = rt.tick();
    assert!(changed);

    let updated_doc = rt.document_ref();
    let root = updated_doc.root();
    let div = updated_doc.find_element_by_tag(root, "div").unwrap();
    assert_eq!(updated_doc.text_content(div), "1");
}

#[test]
fn test_core_js_engine() {
    let doc = parse_html("<!DOCTYPE html><html><body><div id='out'>initial</div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        // 1. Microtask draining & Promise jobs
        var promiseRan = false;
        Promise.resolve().then(function() {
            promiseRan = true;
            var el = document.getElementById("out");
            if (el) el.textContent = "promise-resolved";
        });

        // 2. queueMicrotask execution
        var microtaskRan = false;
        queueMicrotask(function() {
            microtaskRan = true;
        });

        // 3. Intl.NumberFormat - Standard & Compact Notation (YouTube view counts)
        var nf = new Intl.NumberFormat("en-US");
        if (nf.format(1000) !== "1,000") throw new Error("NumberFormat 1000: " + nf.format(1000));

        var compactNf = new Intl.NumberFormat("en-US", { notation: "compact", compactDisplay: "short" });
        var millionFormatted = compactNf.format(1200000);
        if (millionFormatted.indexOf("M") === -1 && millionFormatted.indexOf("1.2") === -1) {
            throw new Error("Compact notation failed: " + millionFormatted);
        }
        var thousandFormatted = compactNf.format(45000);
        if (thousandFormatted.indexOf("K") === -1 && thousandFormatted.indexOf("45") === -1) {
            throw new Error("Compact notation 45K failed: " + thousandFormatted);
        }

        // Currency formatting
        var currNf = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });
        var currFormatted = currNf.format(100);
        if (currFormatted.indexOf("$") === -1 && currFormatted.indexOf("100") === -1) {
            throw new Error("Currency formatting failed: " + currFormatted);
        }

        // Percent formatting
        var pctNf = new Intl.NumberFormat("en-US", { style: "percent" });
        if (pctNf.format(0.75).indexOf("%") === -1) {
            throw new Error("Percent formatting failed: " + pctNf.format(0.75));
        }

        // 4. Intl.DateTimeFormat
        var dtf = new Intl.DateTimeFormat("en-US");
        var dateStr = dtf.format(new Date(1700000000000));
        if (!dateStr || dateStr.length === 0) throw new Error("DateTimeFormat format failed");
        var parts = dtf.formatToParts(new Date(1700000000000));
        if (!Array.isArray(parts) || parts.length === 0) throw new Error("formatToParts failed");
        var rangeStr = dtf.formatRange(new Date(1700000000000), new Date(1700100000000));
        if (!rangeStr || rangeStr.length === 0) throw new Error("formatRange failed");

        // 5. Intl.Collator
        var collator = new Intl.Collator("en");
        if (collator.compare("a", "b") >= 0) throw new Error("Collator a vs b failed");
        if (collator.compare("b", "a") <= 0) throw new Error("Collator b vs a failed");
        if (collator.compare("a", "a") !== 0) throw new Error("Collator a vs a failed");

        // 6. Intl.PluralRules
        var pr = new Intl.PluralRules("en-US");
        if (pr.select(1) !== "one") throw new Error("PluralRules 1: " + pr.select(1));
        if (pr.select(5) !== "other") throw new Error("PluralRules 5: " + pr.select(5));

        // 7. Intl.RelativeTimeFormat
        var rtf = new Intl.RelativeTimeFormat("en");
        var rtfYesterday = rtf.format(-1, "day");
        if (rtfYesterday.indexOf("day") === -1 && rtfYesterday.indexOf("ago") === -1 && rtfYesterday.indexOf("yesterday") === -1) {
            throw new Error("RelativeTimeFormat failed: " + rtfYesterday);
        }

        // 8. Intl.DisplayNames
        var dn = new Intl.DisplayNames(["en"], { type: "language" });
        if (dn.of("en") !== "English") throw new Error("DisplayNames en: " + dn.of("en"));

        // 9. Intl.ListFormat
        var lf = new Intl.ListFormat("en", { style: "long", type: "conjunction" });
        var listFormatted = lf.format(["Motorcycle", "Bus", "Car"]);
        if (listFormatted.indexOf("Motorcycle") === -1 || listFormatted.indexOf("Car") === -1) {
            throw new Error("ListFormat failed: " + listFormatted);
        }

        // 10. Intl.Segmenter
        var seg = new Intl.Segmenter("en", { granularity: "word" });
        var segments = seg.segment("Hello World");
        if (typeof segments[Symbol.iterator] !== "function") throw new Error("Segmenter iterator missing");
    "##;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_core_js_engine failed: {}", e);
    }

    // Verify that engine microtasks drained synchronously via execute_script's context.run_jobs()
    let check_script = r#"
        if (!promiseRan) throw new Error("Promise microtask did not drain");
        if (!microtaskRan) throw new Error("queueMicrotask did not drain");
        var el = document.getElementById("out");
        if (!el || el.textContent !== "promise-resolved") throw new Error("DOM mutation from microtask missing");
    "#;
    if let Err(e) = rt.execute_script(check_script) {
        panic!("test_core_js_engine verification failed: {}", e);
    }
}

#[test]
fn test_csprng_and_storage_partitioning() {
    let doc_a = Document::new();
    let shared_storage =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut rt_a = JsRuntime::new_with_url_and_storage(
        doc_a,
        800.0,
        600.0,
        "https://origin-a.example/page",
        shared_storage.clone(),
    );

    let script_crypto = r#"
        var buf = new Uint8Array(32);
        crypto.getRandomValues(buf);
        var nonZero = false;
        for (var i = 0; i < buf.length; i++) {
            if (buf[i] !== 0) nonZero = true;
        }
        if (!nonZero) throw new Error("CSPRNG returned all zeroes");

        var quotaExceeded = false;
        try {
            var bigBuf = new Uint8Array(65537);
            crypto.getRandomValues(bigBuf);
        } catch(e) {
            if (e.name === "QuotaExceededError") quotaExceeded = true;
        }
        if (!quotaExceeded) throw new Error("Expected QuotaExceededError for length > 65536");

        var uuid = crypto.randomUUID();
        if (typeof uuid !== "string" || uuid.length !== 36) {
            throw new Error("Invalid UUID format: " + uuid);
        }
    "#;
    rt_a.execute_script(script_crypto)
        .expect("Crypto test failed");

    // Test localStorage origin partitioning
    rt_a.execute_script("localStorage.setItem('secret', 'originA_data');")
        .expect("A setItem failed");

    let storage_b = shared_storage.clone();
    std::thread::spawn(move || {
        let mut rt_b = JsRuntime::new_with_url_and_storage(
            Document::new(),
            800.0,
            600.0,
            "https://origin-b.example/page",
            storage_b,
        );
        rt_b.execute_script(
            r#"
            if (localStorage.getItem('secret') !== null) {
                throw new Error("Cross-origin key leakage: origin B read origin A's key");
            }
            localStorage.removeItem('secret');
        "#,
        )
        .expect("B read/remove failed");
    })
    .join()
    .unwrap();

    rt_a.execute_script(
        r#"
        if (localStorage.getItem('secret') !== 'originA_data') {
            throw new Error("Cross-origin deletion leakage: origin B deleted origin A's key");
        }
    "#,
    )
    .expect("A verify key remains failed");

    let storage_b2 = shared_storage.clone();
    std::thread::spawn(move || {
        let mut rt_b = JsRuntime::new_with_url_and_storage(
            Document::new(),
            800.0,
            600.0,
            "https://origin-b.example/page",
            storage_b2,
        );
        rt_b.execute_script("localStorage.clear();")
            .expect("B clear failed");
    })
    .join()
    .unwrap();

    rt_a.execute_script(
        r#"
        if (localStorage.getItem('secret') !== 'originA_data') {
            throw new Error("Cross-origin clear leakage: origin B clear() wiped origin A's key");
        }
    "#,
    )
    .expect("A verify key after clear failed");
}

#[test]
fn test_storage_and_history() {
    let temp_dir = std::env::temp_dir();
    let storage_file = temp_dir.join(format!(
        "mango_test_storage_{}.tsv",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let shared_ls = Arc::new(Mutex::new(HashMap::new()));
    let tab1_ss = Arc::new(Mutex::new(HashMap::new()));
    let cookies = Arc::new(Mutex::new(mango_net::CookieJar::new()));

    let html = "<html><head></head><body><h1>Storage and History Test</h1></body></html>";

    let mut rt_loc = JsRuntime::new_with_url_storage_session_and_cookies(
        parse_html(html),
        800.0,
        600.0,
        "https://example.com:8080/path/index.html?search=rust#section1",
        shared_ls.clone(),
        tab1_ss.clone(),
        cookies.clone(),
    );

    // 1. Test Location Object
    let loc_script = r##"
        // Location properties
        if (location.href !== "https://example.com:8080/path/index.html?search=rust#section1") {
            throw new Error("location.href mismatch: " + location.href);
        }
        if (location.protocol !== "https:") throw new Error("location.protocol mismatch: " + location.protocol);
        if (location.host !== "example.com:8080") throw new Error("location.host mismatch: " + location.host);
        if (location.hostname !== "example.com") throw new Error("location.hostname mismatch: " + location.hostname);
        if (location.port !== "8080") throw new Error("location.port mismatch: " + location.port);
        if (location.pathname !== "/path/index.html") throw new Error("location.pathname mismatch: " + location.pathname);
        if (location.search !== "?search=rust") throw new Error("location.search mismatch: " + location.search);
        if (location.hash !== "#section1") throw new Error("location.hash mismatch: " + location.hash);
        if (location.origin !== "https://example.com:8080") throw new Error("location.origin mismatch: " + location.origin);
        if (location.toString() !== location.href) throw new Error("location.toString() mismatch");

        // Location hash change event dispatch
        var hashChanged = false;
        var oldHashUrl = "";
        var newHashUrl = "";
        window.addEventListener("hashchange", function(e) {
            hashChanged = true;
            oldHashUrl = e.oldURL;
            newHashUrl = e.newURL;
        });
        location.hash = "#section2";
        if (location.hash !== "#section2") throw new Error("location.hash setter failed");
        if (!location.href.endsWith("#section2")) throw new Error("location.href not updated after hash change");
        if (!hashChanged) throw new Error("hashchange event not fired on location.hash change");

        // Location search & pathname setter
        location.search = "?page=2";
        if (location.search !== "?page=2") throw new Error("location.search setter failed");
        location.pathname = "/newpath";
        if (location.pathname !== "/newpath") throw new Error("location.pathname setter failed");

        // Location assign & replace & reload
        location.assign("/assigned");
        if (location.pathname !== "/assigned") throw new Error("location.assign failed");
        location.replace("/replaced");
        if (location.pathname !== "/replaced") throw new Error("location.replace failed");
        location.reload(); // Should execute safely
    "##;
    rt_loc
        .execute_script(loc_script)
        .expect("Location test failed");
    drop(rt_loc);

    // 2. Test History Object
    let mut rt_hist = JsRuntime::new_with_url_storage_session_and_cookies(
        parse_html(html),
        800.0,
        600.0,
        "https://example.com:8080/test",
        shared_ls.clone(),
        tab1_ss.clone(),
        cookies.clone(),
    );

    let hist_script = r##"
        if (history.state !== null) throw new Error("Initial history.state should be null");
        if (history.length !== 1) throw new Error("Initial history.length should be 1, got: " + history.length);
        if (history.scrollRestoration !== "auto") throw new Error("Initial scrollRestoration should be auto");
        history.scrollRestoration = "manual";
        if (history.scrollRestoration !== "manual") throw new Error("scrollRestoration setter failed");

        // PushState
        history.pushState({ count: 1, name: "step1" }, "Step 1", "/step1?q=1#h1");
        if (history.length !== 2) throw new Error("history.length after pushState should be 2, got: " + history.length);
        if (!history.state || history.state.count !== 1 || history.state.name !== "step1") {
            throw new Error("history.state mismatch after pushState: " + JSON.stringify(history.state));
        }
        if (location.pathname !== "/step1") throw new Error("location.pathname after pushState mismatch: " + location.pathname);
        if (location.search !== "?q=1") throw new Error("location.search after pushState mismatch: " + location.search);
        if (location.hash !== "#h1") throw new Error("location.hash after pushState mismatch: " + location.hash);

        // ReplaceState
        history.replaceState({ count: 2, name: "step2" }, "Step 2", "/step2?q=2#h2");
        if (history.length !== 2) throw new Error("history.length after replaceState should remain 2, got: " + history.length);
        if (!history.state || history.state.count !== 2 || history.state.name !== "step2") {
            throw new Error("history.state mismatch after replaceState: " + JSON.stringify(history.state));
        }
        if (location.pathname !== "/step2") throw new Error("location.pathname after replaceState mismatch: " + location.pathname);

        // Cross-origin SecurityError test
        var securityCaught = false;
        try {
            history.pushState({ evil: true }, "Evil", "https://attacker.com/evil");
        } catch (e) {
            if (e.name === "SecurityError") {
                securityCaught = true;
            }
        }
        if (!securityCaught) throw new Error("Cross-origin pushState should throw SecurityError");

        // Popstate event via back/forward/go
        history.pushState({ count: 3, name: "step3" }, "Step 3", "/step3");
        if (history.length !== 3) throw new Error("history.length should be 3");

        var popstateFired = false;
        var popstateState = null;
        window.addEventListener("popstate", function(e) {
            popstateFired = true;
            popstateState = e.state;
        });

        history.back();
        if (!popstateFired) throw new Error("popstate event did not fire on history.back()");
        if (!popstateState || popstateState.count !== 2) {
            throw new Error("popstate event.state mismatch: " + JSON.stringify(popstateState));
        }
        if (location.pathname !== "/step2") throw new Error("location not updated on back(): " + location.pathname);

        popstateFired = false;
        history.forward();
        if (!popstateFired) throw new Error("popstate event did not fire on history.forward()");
        if (!popstateState || popstateState.count !== 3) {
            throw new Error("popstate event.state mismatch on forward(): " + JSON.stringify(popstateState));
        }
        if (location.pathname !== "/step3") throw new Error("location not updated on forward(): " + location.pathname);

        popstateFired = false;
        history.go(-1);
        if (!popstateFired || !popstateState || popstateState.count !== 2) {
            throw new Error("history.go(-1) failed to fire popstate or wrong state");
        }
    "##;
    rt_hist
        .execute_script(hist_script)
        .expect("History test failed");
    drop(rt_hist);

    // 3. Test localStorage (Item methods, property proxy access, file persistence)
    let mut rt_storage = JsRuntime::new_with_url_storage_session_and_cookies(
        parse_html(html),
        800.0,
        600.0,
        "https://example.com:8080/storage_test",
        shared_ls.clone(),
        tab1_ss.clone(),
        cookies.clone(),
    );

    let ls_script = r##"
        localStorage.clear();
        if (localStorage.length !== 0) throw new Error("localStorage should be empty after clear()");
        if (localStorage.getItem("missing") !== null) throw new Error("getItem on missing should return null");

        localStorage.setItem("user", "Alice");
        localStorage.setItem("theme", "dark");
        if (localStorage.length !== 2) throw new Error("localStorage.length should be 2, got: " + localStorage.length);
        if (localStorage.getItem("user") !== "Alice") throw new Error("getItem('user') mismatch");
        if (localStorage.getItem("theme") !== "dark") throw new Error("getItem('theme') mismatch");

        // Proxy property access
        localStorage.customProp = "customValue";
        if (localStorage.getItem("customProp") !== "customValue") throw new Error("Proxy property set failed to write to storage");
        if (localStorage.customProp !== "customValue") throw new Error("Proxy property get failed");
        if (!("customProp" in localStorage)) throw new Error("'customProp' in localStorage failed");

        // key(index)
        var key0 = localStorage.key(0);
        if (key0 === null) throw new Error("localStorage.key(0) returned null");

        // removeItem & delete proxy
        localStorage.removeItem("theme");
        if (localStorage.getItem("theme") !== null) throw new Error("removeItem failed");
        delete localStorage.customProp;
        if (localStorage.getItem("customProp") !== null) throw new Error("delete localStorage.prop failed");
        if (localStorage.length !== 1) throw new Error("localStorage.length after removals should be 1, got: " + localStorage.length);
    "##;
    rt_storage
        .execute_script(ls_script)
        .expect("localStorage test failed");

    // 4. Test sessionStorage (Tab-scoped, in-memory, isolated between tabs)
    let ss_tab1_script = r##"
        sessionStorage.clear();
        if (sessionStorage.length !== 0) throw new Error("sessionStorage should be empty");
        sessionStorage.setItem("tab_token", "tab1_secret_token_123");
        sessionStorage.proxyKey = "proxyVal";
        if (sessionStorage.getItem("tab_token") !== "tab1_secret_token_123") throw new Error("sessionStorage getItem failed");
        if (sessionStorage.proxyKey !== "proxyVal") throw new Error("sessionStorage proxy get failed");
        if (sessionStorage.length !== 2) throw new Error("sessionStorage.length should be 2, got: " + sessionStorage.length);
    "##;
    rt_storage
        .execute_script(ss_tab1_script)
        .expect("sessionStorage tab 1 script failed");

    // Persist localStorage to file
    rt_storage
        .persist_local_storage(&storage_file)
        .expect("Failed to persist localStorage to file");
    assert!(
        storage_file.exists(),
        "Storage file was not written to disk"
    );
    drop(rt_storage);

    // Create a new fresh runtime simulating browser restart, and load from file
    let new_shared_ls = Arc::new(Mutex::new(HashMap::new()));
    let mut rt_restored = JsRuntime::new_with_url_storage_session_and_cookies(
        parse_html(html),
        800.0,
        600.0,
        "https://example.com:8080/restored",
        new_shared_ls.clone(),
        Arc::new(Mutex::new(HashMap::new())),
        cookies.clone(),
    );
    let restored_count = rt_restored
        .load_local_storage(&storage_file)
        .expect("Failed to load localStorage from file");
    assert!(restored_count > 0, "No entries restored from file");

    let verify_ls_restored = r##"
        if (localStorage.getItem("user") !== "Alice") {
            throw new Error("Restored localStorage missing 'user' key: " + localStorage.getItem("user"));
        }
    "##;
    rt_restored
        .execute_script(verify_ls_restored)
        .expect("Restored localStorage verification failed");
    drop(rt_restored);
    let _ = std::fs::remove_file(&storage_file);

    // Simulate page navigation in Tab 1: new runtime instance with SAME tab1_ss
    let mut rt_tab1_navigated = JsRuntime::new_with_url_storage_session_and_cookies(
        parse_html(html),
        800.0,
        600.0,
        "https://example.com:8080/navigated_in_tab1",
        shared_ls.clone(),
        tab1_ss.clone(), // Same tab-scoped session storage
        cookies.clone(),
    );
    let ss_navigated_script = r##"
        if (sessionStorage.getItem("tab_token") !== "tab1_secret_token_123") {
            throw new Error("sessionStorage did not persist across navigation in same tab: " + sessionStorage.getItem("tab_token"));
        }
        if (sessionStorage.proxyKey !== "proxyVal") {
            throw new Error("sessionStorage proxy property did not persist in same tab");
        }
    "##;
    rt_tab1_navigated
        .execute_script(ss_navigated_script)
        .expect("sessionStorage navigation in same tab failed");
    drop(rt_tab1_navigated);

    // Simulate Tab 2: new runtime instance with SEPARATE new session storage
    let tab2_ss = Arc::new(Mutex::new(HashMap::new()));
    let mut rt_tab2 = JsRuntime::new_with_url_storage_session_and_cookies(
        parse_html(html),
        800.0,
        600.0,
        "https://example.com:8080/tab2_page",
        shared_ls.clone(),
        tab2_ss.clone(), // Independent tab session storage
        cookies.clone(),
    );
    let ss_tab2_script = r##"
        if (sessionStorage.getItem("tab_token") !== null) {
            throw new Error("sessionStorage leaked from Tab 1 to Tab 2!");
        }
        if (sessionStorage.length !== 0) {
            throw new Error("Tab 2 sessionStorage should be empty, got length: " + sessionStorage.length);
        }
    "##;
    rt_tab2
        .execute_script(ss_tab2_script)
        .expect("sessionStorage Tab 2 isolation check failed");
    drop(rt_tab2);
}

#[test]
fn test_es_modules() {
    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let module_src = r#"
        globalThis.moduleVar = 100;
        export const x = 42;
        export function add(a, b) { return a + b; }
        globalThis.moduleCalc = add(x, 8);
    "#;

    rt.execute_module(module_src)
        .expect("execute_module failed");

    let calc_res = rt.eval("globalThis.moduleCalc").expect("eval moduleCalc");
    assert_eq!(calc_res, "50", "ES Module execution calculation mismatch");

    let var_res = rt.eval("globalThis.moduleVar").expect("eval moduleVar");
    assert_eq!(
        var_res, "100",
        "ES Module execution global assignment mismatch"
    );

    // Test automatic fallback when execute_script receives code with ES module syntax (export)
    let module_src_via_script = r#"
        globalThis.scriptModuleVar = 200;
        export const y = 99;
    "#;
    rt.execute_script(module_src_via_script)
        .expect("execute_script with export should fallback to ES module and succeed");
    let script_mod_res = rt
        .eval("globalThis.scriptModuleVar")
        .expect("eval scriptModuleVar");
    assert_eq!(script_mod_res, "200");
}

#[test]
fn test_watchdog_terminates_infinite_loop() {
    let doc = Document::new();
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);
    let start = std::time::Instant::now();
    let res =
        rt.execute_script_with_timeout("while (true) {}", std::time::Duration::from_millis(50));
    let elapsed = start.elapsed();
    assert!(res.is_err(), "infinite loop must fail via watchdog");
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "watchdog must terminate quickly, took {:?}",
        elapsed
    );

    // Verify isolate is still completely functional and healthy after termination
    let res2 = rt.eval("1 + 1");
    assert_eq!(res2.unwrap(), "2");
}
