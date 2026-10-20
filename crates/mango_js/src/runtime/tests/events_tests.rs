use mango_html::dom::Document;
use mango_html::parse_html;

use crate::runtime::{EventModifiers, JsRuntime};

#[test]
fn test_dom_events() {
    let html = r#"<!DOCTYPE html>
    <html>
    <body>
        <div id="parent">
            <button id="btn">Click me</button>
        </div>
        <form id="myform">
            <input id="myinput" type="text" value="hello" />
        </form>
    </body>
    </html>"#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let setup_script = r##"
        window.log = [];

        // Window capture & bubble
        window.addEventListener("click", function(e) {
            window.log.push("window-capture:phase=" + e.eventPhase);
        }, true);
        window.addEventListener("click", function(e) {
            window.log.push("window-bubble:phase=" + e.eventPhase);
        }, false);

        // Document capture & bubble
        document.addEventListener("click", function(e) {
            window.log.push("document-capture:phase=" + e.eventPhase);
        }, { capture: true });
        document.addEventListener("click", function(e) {
            window.log.push("document-bubble:phase=" + e.eventPhase);
        }, { capture: false });

        // Parent element capture & bubble
        var parent = document.getElementById("parent");
        parent.addEventListener("click", function(e) {
            window.log.push("parent-capture:phase=" + e.eventPhase);
        }, true);
        parent.addEventListener("click", function(e) {
            window.log.push("parent-bubble:phase=" + e.eventPhase);
        }, false);

        // Button target listener & composedPath
        var btn = document.getElementById("btn");
        btn.addEventListener("click", function(e) {
            window.log.push("btn-target:phase=" + e.eventPhase);
            var path = e.composedPath();
            if (!Array.isArray(path) || path.length < 3) {
                throw new Error("composedPath invalid: " + path.length);
            }
            if (path[0] !== btn) throw new Error("composedPath[0] is not btn");
        });

        // once: true test
        window.onceCount = 0;
        btn.addEventListener("click", function() {
            window.onceCount++;
        }, { once: true });

        // passive: true test
        window.passiveTriedPrevent = false;
        btn.addEventListener("click", function(e) {
            e.preventDefault();
            window.passiveDefaultPrevented = e.defaultPrevented;
        }, { passive: true });

        // AbortSignal test
        var ac = new AbortController();
        window.abortedFired = false;
        btn.addEventListener("click", function() {
            window.abortedFired = true;
        }, { signal: ac.signal });
        ac.abort(); // Immediately remove listener before click
    "##;

    if let Err(e) = rt.execute_script(setup_script) {
        panic!("setup_script failed: {}", e);
    }

    // Find button node id
    let btn_id = {
        let doc = rt.document_ref();
        let root = doc.root();
        doc.find_element_by_tag(root, "button")
            .expect("button node")
    };

    // Dispatch click from host
    let default_allowed = rt
        .dispatch_click(Some(btn_id), 120.0, 240.0, 0, 1, EventModifiers::NONE)
        .expect("dispatch_click");
    assert!(default_allowed);

    // Verify propagation sequence and options
    let verify_script = r##"
        // Expected 3-phase propagation order:
        // 1. window-capture (phase 1)
        // 2. document-capture (phase 1)
        // 3. parent-capture (phase 1)
        // 4. btn-target (phase 2)
        // 5. parent-bubble (phase 3)
        // 6. document-bubble (phase 3)
        // 7. window-bubble (phase 3)
        var expected = [
            "window-capture:phase=1",
            "document-capture:phase=1",
            "parent-capture:phase=1",
            "btn-target:phase=2",
            "parent-bubble:phase=3",
            "document-bubble:phase=3",
            "window-bubble:phase=3"
        ];
        if (window.log.length !== expected.length) {
            throw new Error("Log length: " + window.log.length + " vs expected " + expected.length + ": " + window.log.join(", "));
        }
        for (var i = 0; i < expected.length; i++) {
            if (window.log[i] !== expected[i]) {
                throw new Error("Step " + i + ": got " + window.log[i] + " expected " + expected[i]);
            }
        }

        // once: true check (fired once)
        if (window.onceCount !== 1) throw new Error("onceCount: " + window.onceCount);

        // passive: true check (defaultPrevented remained false)
        if (window.passiveDefaultPrevented !== false) {
            throw new Error("passive listener failed to ignore preventDefault");
        }

        // AbortSignal check (did not fire)
        if (window.abortedFired !== false) throw new Error("Aborted listener fired!");
    "##;

    if let Err(e) = rt.execute_script(verify_script) {
        panic!("verify_script failed: {}", e);
    }

    // Test second click: once listener should NOT fire again
    rt.dispatch_click(Some(btn_id), 120.0, 240.0, 0, 1, EventModifiers::NONE)
        .unwrap();
    let check_once2 = "if (window.onceCount !== 1) throw new Error('once listener fired twice');";
    rt.execute_script(check_once2).unwrap();

    // Test stopPropagation
    let stop_script = r##"
        window.stopLog = [];
        var p = document.getElementById("parent");
        p.addEventListener("custom-test", function(e) {
            window.stopLog.push("parent-capture");
            e.stopPropagation();
        }, true);
        var b = document.getElementById("btn");
        b.addEventListener("custom-test", function(e) {
            window.stopLog.push("btn-target");
        });
        var ev = new CustomEvent("custom-test", { bubbles: true, cancelable: true });
        b.dispatchEvent(ev);
        if (window.stopLog.length !== 1 || window.stopLog[0] !== "parent-capture") {
            throw new Error("stopPropagation failed: " + window.stopLog.join(", "));
        }
    "##;
    rt.execute_script(stop_script).unwrap();

    // Test stopImmediatePropagation
    let imm_script = r##"
        window.immLog = [];
        var b = document.getElementById("btn");
        b.addEventListener("imm-test", function(e) {
            window.immLog.push("l1");
            e.stopImmediatePropagation();
        });
        b.addEventListener("imm-test", function(e) {
            window.immLog.push("l2");
        });
        b.dispatchEvent(new Event("imm-test"));
        if (window.immLog.length !== 1 || window.immLog[0] !== "l1") {
            throw new Error("stopImmediatePropagation failed: " + window.immLog.join(", "));
        }
    "##;
    rt.execute_script(imm_script).unwrap();

    // Test host event dispatch methods for input, change, submit, resize, scroll, domcontentloaded, load
    let input_id = {
        let doc = rt.document_ref();
        let root = doc.root();
        doc.find_element_by_tag(root, "input").expect("input node")
    };
    let form_id = {
        let doc = rt.document_ref();
        let root = doc.root();
        doc.find_element_by_tag(root, "form").expect("form node")
    };

    let host_listener_script = r##"
        window.eventsReceived = {};
        document.getElementById("myinput").addEventListener("input", function(e) {
            window.eventsReceived["input"] = e.data;
        });
        document.getElementById("myinput").addEventListener("change", function(e) {
            window.eventsReceived["change"] = true;
        });
        document.getElementById("myform").addEventListener("submit", function(e) {
            window.eventsReceived["submit"] = true;
            e.preventDefault(); // cancel form submit
        });
        document.addEventListener("DOMContentLoaded", function(e) {
            window.eventsReceived["DOMContentLoaded"] = true;
        });
        window.addEventListener("load", function(e) {
            window.eventsReceived["load"] = true;
        });
        window.addEventListener("resize", function(e) {
            window.eventsReceived["resize"] = window.innerWidth + "x" + window.innerHeight;
        });
        window.addEventListener("scroll", function(e) {
            window.eventsReceived["scroll"] = window.scrollX + "," + window.scrollY;
        });
    "##;
    rt.execute_script(host_listener_script).unwrap();

    rt.dispatch_input(input_id, Some("world")).unwrap();
    rt.dispatch_change(input_id).unwrap();
    let submit_allowed = rt.dispatch_submit(form_id).unwrap();
    assert!(!submit_allowed, "submit was preventDefault'd");
    rt.dispatch_dom_content_loaded().unwrap();
    rt.dispatch_load().unwrap();
    rt.dispatch_window_resize(1280.0, 720.0).unwrap();
    rt.dispatch_window_scroll(0.0, 300.0).unwrap();

    let check_host_events = r##"
        if (window.eventsReceived["input"] !== "world") throw new Error("input event missing");
        if (!window.eventsReceived["change"]) throw new Error("change event missing");
        if (!window.eventsReceived["submit"]) throw new Error("submit event missing");
        if (!window.eventsReceived["DOMContentLoaded"]) throw new Error("DOMContentLoaded missing");
        if (!window.eventsReceived["load"]) throw new Error("load missing");
        if (window.eventsReceived["resize"] !== "1280x720") throw new Error("resize missing: " + window.eventsReceived["resize"]);
        if (window.eventsReceived["scroll"] !== "0,300") throw new Error("scroll missing: " + window.eventsReceived["scroll"]);
    "##;
    rt.execute_script(check_host_events).unwrap();

    // Test hover and active state management
    assert!(rt.set_hover_state(Some(btn_id)));
    assert!(rt.is_dom_dirty());
    rt.clear_dom_dirty();

    // Verify button and parent both have data-mango-hover="true"
    assert_eq!(
        rt.get_node_attribute(btn_id, "data-mango-hover"),
        Some("true".to_string())
    );
    let parent_id = {
        let doc = rt.document_ref();
        let root = doc.root();
        doc.find_element_by_tag(root, "div")
            .expect("parent div node")
    };
    assert_eq!(
        rt.get_node_attribute(parent_id, "data-mango-hover"),
        Some("true".to_string())
    );

    // Unhover
    assert!(rt.set_hover_state(None));
    assert_eq!(rt.get_node_attribute(btn_id, "data-mango-hover"), None);
    assert_eq!(rt.get_node_attribute(parent_id, "data-mango-hover"), None);

    // Test active state
    assert!(rt.set_active_state(Some(btn_id)));
    assert_eq!(
        rt.get_node_attribute(btn_id, "data-mango-active"),
        Some("true".to_string())
    );
    assert_eq!(
        rt.get_node_attribute(parent_id, "data-mango-active"),
        Some("true".to_string())
    );
    assert!(rt.set_active_state(None));
    assert_eq!(rt.get_node_attribute(btn_id, "data-mango-active"), None);
    assert_eq!(rt.get_node_attribute(parent_id, "data-mango-active"), None);
}

#[test]
fn test_event_subclass_prototype_constructors() {
    let doc = Document::new();
    let mut rt = JsRuntime::new(doc, 1024.0, 768.0);
    let script = r#"
        (function() {
            var classes = [
                [Event, new Event("test")],
                [UIEvent, new UIEvent("test")],
                [MouseEvent, new MouseEvent("click")],
                [PointerEvent, new PointerEvent("pointerdown")],
                [KeyboardEvent, new KeyboardEvent("keydown")],
                [FocusEvent, new FocusEvent("focus")],
                [InputEvent, new InputEvent("input")],
                [WheelEvent, new WheelEvent("wheel")],
                [DragEvent, new DragEvent("drag")],
                [TouchEvent, new TouchEvent("touchstart")],
                [CustomEvent, new CustomEvent("custom")],
                [HashChangeEvent, new HashChangeEvent("hashchange")],
                [PopStateEvent, new PopStateEvent("popstate")],
                [ErrorEvent, new ErrorEvent("error")],
                [StorageEvent, new StorageEvent("storage")],
                [TransitionEvent, new TransitionEvent("transitionend")],
                [AnimationEvent, new AnimationEvent("animationend")],
                [SubmitEvent, new SubmitEvent("submit")],
                [FormDataEvent, new FormDataEvent("formdata")],
                [PageTransitionEvent, new PageTransitionEvent("pagehide")],
                [MessageEvent, new MessageEvent("message")],
                [CloseEvent, new CloseEvent("close")]
            ];

            for (var i = 0; i < classes.length; i++) {
                var ctor = classes[i][0];
                var inst = classes[i][1];
                if (ctor.prototype.constructor !== ctor) {
                    throw new Error(ctor.name + ".prototype.constructor !== " + ctor.name);
                }
                if (inst.constructor !== ctor) {
                    throw new Error("instance of " + ctor.name + ".constructor !== " + ctor.name);
                }
                if (!(inst instanceof Event)) {
                    throw new Error(ctor.name + " must be instanceof Event");
                }
            }

            return "ok";
        })()
    "#;
    let res = rt.eval(script).expect("eval event constructor test");
    assert_eq!(res, "ok");
}

#[test]
fn test_input_accessor_descriptors() {
    let doc = mango_html::dom::Document::new();
    let mut rt = JsRuntime::new(doc, 1024.0, 768.0);

    let script = r#"
        (function() {
            var valueDesc = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value');
            if (!valueDesc) throw new Error("HTMLInputElement.prototype.value descriptor missing");
            if (typeof valueDesc.get !== 'function') throw new Error("value get not function");
            if (typeof valueDesc.set !== 'function') throw new Error("value set not function");

            var checkedDesc = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'checked');
            if (!checkedDesc) throw new Error("HTMLInputElement.prototype.checked descriptor missing");
            if (typeof checkedDesc.get !== 'function') throw new Error("checked get not function");
            if (typeof checkedDesc.set !== 'function') throw new Error("checked set not function");

            var textDesc = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value');
            if (!textDesc || typeof textDesc.get !== 'function' || typeof textDesc.set !== 'function') {
                throw new Error("HTMLTextAreaElement.prototype.value descriptor invalid");
            }

            var selectDesc = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value');
            if (!selectDesc || typeof selectDesc.get !== 'function' || typeof selectDesc.set !== 'function') {
                throw new Error("HTMLSelectElement.prototype.value descriptor invalid");
            }

            // Verify input element interaction via prototype descriptor (React controlled input tracker pattern)
            var input = document.createElement("input");
            valueDesc.set.call(input, "foo");
            if (input.value !== "foo") throw new Error("input.value after descriptor.set expected 'foo', got: " + input.value);
            if (valueDesc.get.call(input) !== "foo") throw new Error("descriptor.get.call expected 'foo'");

            checkedDesc.set.call(input, true);
            if (input.checked !== true) throw new Error("input.checked after descriptor.set expected true");
            if (checkedDesc.get.call(input) !== true) throw new Error("checkedDesc.get.call expected true");

            // Simulate React 18 controlled component inputValueTracker hook
            var trackedValue = input.value;
            Object.defineProperty(input, 'value', {
                configurable: true,
                get: function() {
                    return valueDesc.get.call(this);
                },
                set: function(val) {
                    trackedValue = val;
                    valueDesc.set.call(this, val);
                }
            });

            input.value = "react-controlled";
            if (input.value !== "react-controlled") throw new Error("React simulated value mismatch");
            if (trackedValue !== "react-controlled") throw new Error("React tracker value not updated");

            return "ok";
        })()
    "#;
    let res = rt
        .eval(script)
        .expect("eval input accessor descriptor test");
    assert_eq!(res, "ok");
}
