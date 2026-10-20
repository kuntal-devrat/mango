//! Comprehensive integration tests for WHATWG DOM & HTML5 Standards Parity.
//!
//! Covers:
//! 1. Observers Architecture (MutationObserver, IntersectionObserver, ResizeObserver)
//! 2. HTML5 Constraint Validation & Forms Engine (ValidityState, checkValidity, reportValidity, requestSubmit)
//! 3. Selection & Range Layout Geometry (getBoundingClientRect, getClientRects)
//! 4. DOM Level 4 Event Dispatch (composedPath across Shadow DOM, passive listeners, once listeners, AbortSignal)
//! 5. Visual Validation Bubble Subsystem

use mango_html::parse_html;
use mango_js::JsRuntime;

#[test]
fn test_mutation_observer_attributes_and_character_data() {
    let html = r#"<!DOCTYPE html><html><body><div id="target" class="init-cls">Initial Text</div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var target = document.getElementById("target");
        var recordsReceived = [];

        var observer = new MutationObserver(function(records) {
            recordsReceived = records.slice();
        });

        observer.observe(target, {
            attributes: true,
            attributeFilter: ["data-test", "title"],
            attributeOldValue: true,
            characterData: true,
            characterDataOldValue: true
        });

        // Ignored attribute mutation (not in filter)
        target.setAttribute("class", "ignored-cls");
        // Observed attribute mutation
        target.setAttribute("data-test", "val-1");
        target.setAttribute("data-test", "val-2");

        // Observed characterData mutation via nodeValue
        target.nodeValue = "Updated Text";

        // takeRecords() drains without executing callback
        var earlyRecords = observer.takeRecords();
        if (earlyRecords.length !== 3) throw new Error("Expected 3 records in takeRecords(), got " + earlyRecords.length);
        if (earlyRecords[0].attributeName !== "data-test" || earlyRecords[0].oldValue !== null) throw new Error("Record 0 mismatch");
        if (earlyRecords[1].attributeName !== "data-test" || earlyRecords[1].oldValue !== "val-1") throw new Error("Record 1 mismatch");
        if (earlyRecords[2].type !== "characterData" || earlyRecords[2].oldValue !== "Initial Text") throw new Error("Record 2 mismatch");

        // Further mutation observed after takeRecords
        target.setAttribute("title", "New Title");
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("Script execution failed: {}", e);
    }
    assert!(rt.is_dom_dirty());
}

#[test]
fn test_intersection_observer_and_thresholds() {
    let html = r#"<!DOCTYPE html><html><body><div id="target" style="width: 100px; height: 100px;"></div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var target = document.getElementById("target");
        var observedEntries = null;

        var observer = new IntersectionObserver(function(entries, obs) {
            observedEntries = entries;
        }, {
            rootMargin: "10px 20px 10px 20px",
            threshold: [0, 0.5, 1.0]
        });

        if (observer.thresholds.length !== 3) throw new Error("Thresholds parsing failed");
        if (observer.rootMargin !== "10px 20px 10px 20px") throw new Error("rootMargin mismatch");

        observer.observe(target);

        var immediateRecords = observer.takeRecords();
        if (immediateRecords.length !== 1) throw new Error("Expected initial zero-delay notification");
        var entry = immediateRecords[0];
        if (entry.target !== target) throw new Error("Target mismatch");
        if (typeof entry.isIntersecting !== 'boolean') throw new Error("isIntersecting missing");
        if (typeof entry.intersectionRatio !== 'number') throw new Error("intersectionRatio missing");
        if (!entry.rootBounds || !entry.boundingClientRect) throw new Error("Rect bounds missing");
    "#;

    assert!(rt.execute_script(script).is_ok());
}

#[test]
fn test_resize_observer_box_sizes_and_loop_limit() {
    let html = r#"<!DOCTYPE html><html><body><div id="box" style="width: 200px; height: 100px;"></div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        if (typeof ResizeObserverSize !== 'function') throw new Error("ResizeObserverSize missing");
        var size = new ResizeObserverSize(200, 100);
        if (size.inlineSize !== 200 || size.blockSize !== 100) throw new Error("ResizeObserverSize properties mismatch");
        if (!Object.isFrozen(size)) throw new Error("ResizeObserverSize must be frozen");

        var box = document.getElementById("box");
        var ro = new ResizeObserver(function(entries) {});
        ro.observe(box);

        var entry = new ResizeObserverEntry(box);
        if (!Array.isArray(entry.borderBoxSize) || !Object.isFrozen(entry.borderBoxSize)) throw new Error("borderBoxSize must be frozen array");
        if (!Array.isArray(entry.contentBoxSize) || !Object.isFrozen(entry.contentBoxSize)) throw new Error("contentBoxSize must be frozen array");
        if (!Array.isArray(entry.devicePixelContentBoxSize) || !Object.isFrozen(entry.devicePixelContentBoxSize)) throw new Error("devicePixelContentBoxSize must be frozen array");
        if (!(entry.borderBoxSize[0] instanceof ResizeObserverSize)) throw new Error("borderBoxSize item must be ResizeObserverSize");

        // Loop limit protection verification (> 20 recursive scheduling triggers error event)
        var errorFired = false;
        var errorMsg = "";
        window.addEventListener("error", function(evt) {
            errorFired = true;
            errorMsg = evt.message;
        });

        // Trigger > 20 recursive scheduled deliveries
        for (var i = 0; i < 25; i++) {
            ro._scheduled = false;
            ro._scheduleDelivery();
        }
        if (!errorFired) throw new Error("ErrorEvent was not dispatched upon exceeding loop limit");
        if (errorMsg !== "ResizeObserver loop completed with undelivered notifications.") {
            throw new Error("Unexpected error message: " + errorMsg);
        }
    "#;

    assert!(rt.execute_script(script).is_ok());
}

#[test]
fn test_html5_validity_state_and_form_validation() {
    let html = r#"<!DOCTYPE html>
    <html>
    <body>
        <form id="test-form">
            <input id="req-text" type="text" required />
            <input id="email-input" type="email" value="invalid-email" />
            <input id="num-input" type="number" min="10" max="50" step="5" value="12" />
            <button id="sub-btn" type="submit">Submit</button>
        </form>
    </body>
    </html>"#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        if (typeof ValidityState !== 'function') throw new Error("globalThis.ValidityState constructor missing");

        var form = document.getElementById("test-form");
        var reqInput = document.getElementById("req-text");
        var emailInput = document.getElementById("email-input");
        var numInput = document.getElementById("num-input");
        var btn = document.getElementById("sub-btn");

        // Required input check
        var v1 = reqInput.validity;
        if (!(v1 instanceof ValidityState)) throw new Error("validity must be instance of ValidityState");
        if (!v1.valueMissing) throw new Error("valueMissing should be true for empty required text input");
        if (v1.valid) throw new Error("valid should be false for valueMissing");

        // Email typeMismatch check
        var v2 = emailInput.validity;
        if (!v2.typeMismatch) throw new Error("typeMismatch should be true for invalid email");

        // Number stepMismatch check (value 12 with min=10, step=5)
        var v3 = numInput.validity;
        if (!v3.stepMismatch) throw new Error("stepMismatch should be true for 12 with step 5");

        // Custom validity check
        numInput.setCustomValidity("Custom failure message");
        var v4 = numInput.validity;
        if (!v4.customError || v4.valid) throw new Error("customError should be true");
        if (numInput.validationMessage !== "Custom failure message") throw new Error("validationMessage mismatch");
        numInput.setCustomValidity("");

        // checkValidity() dispatches 'invalid' event on invalid input
        var invalidFired = false;
        reqInput.addEventListener("invalid", function() {
            invalidFired = true;
        });
        var checkRes = reqInput.checkValidity();
        if (checkRes !== false || !invalidFired) throw new Error("checkValidity failed to fire invalid event");

        // form.checkValidity() returns false when controls are invalid
        if (form.checkValidity() !== false) throw new Error("form.checkValidity should return false");

        // Make all controls valid
        reqInput.value = "Valid text";
        emailInput.value = "user@example.com";
        numInput.value = "15";
        if (!form.checkValidity()) throw new Error("form should be valid now");

        // form.requestSubmit() with invalid submitter throws TypeError
        var caught = false;
        try {
            form.requestSubmit(reqInput);
        } catch(e) {
            if (e instanceof TypeError) caught = true;
        }
        if (!caught) throw new Error("requestSubmit with non-submitter did not throw TypeError");

        // form.requestSubmit(btn) successfully fires submit event with submitter
        var submitFired = false;
        var submitterReceived = null;
        form.addEventListener("submit", function(evt) {
            submitFired = true;
            submitterReceived = evt.submitter;
        });
        form.requestSubmit(btn);
        if (!submitFired) throw new Error("requestSubmit failed to fire submit event");
        if (submitterReceived !== btn) throw new Error("evt.submitter was not preserved");
    "#;

    assert!(rt.execute_script(script).is_ok());
}

#[test]
fn test_form_data_serialization() {
    let html = r#"<!DOCTYPE html><html><body></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var fd = new FormData();
        fd.append("username", "alice");
        fd.append("search query", "mango browser");

        // URL encoded serialization
        var urlEncoded = fd.serializeUrlEncoded();
        if (urlEncoded !== "username=alice&search+query=mango+browser") {
            throw new Error("serializeUrlEncoded mismatch: " + urlEncoded);
        }

        // URLSearchParams interoperability
        var usp = fd.toUrlSearchParams();
        if (!usp || usp.get("username") !== "alice" || usp.get("search query") !== "mango browser") {
            throw new Error("toUrlSearchParams mismatch");
        }

        // Multipart serialization
        var boundary = "CustomBoundary123";
        var multipart = fd.serializeMultipart(boundary);
        if (multipart.boundary !== boundary) throw new Error("Boundary mismatch");
        if (multipart.contentType !== "multipart/form-data; boundary=CustomBoundary123") throw new Error("ContentType mismatch");
        if (multipart.body.indexOf("--CustomBoundary123") === -1 || multipart.body.indexOf('name="username"') === -1) {
            throw new Error("Multipart body missing expected entries");
        }
    "#;

    assert!(rt.execute_script(script).is_ok());
}

#[test]
fn test_selection_and_range_geometry() {
    let html = r#"<!DOCTYPE html>
    <html>
    <body>
        <div id="container" style="width: 300px; height: 150px;">
            <p id="p1">First Paragraph</p>
            <p id="p2">Second Paragraph</p>
        </div>
    </body>
    </html>"#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var p1 = document.getElementById("p1");
        var p2 = document.getElementById("p2");

        var range = document.createRange();
        range.setStart(p1, 0);
        range.setEnd(p2, 1);

        var rect = range.getBoundingClientRect();
        if (typeof rect.left !== 'number' || typeof rect.top !== 'number' || typeof rect.width !== 'number') {
            throw new Error("getBoundingClientRect rect properties missing");
        }

        var clientRects = range.getClientRects();
        if (!Array.isArray(clientRects)) throw new Error("getClientRects must return an array");

        // Selection API
        var sel = window.getSelection();
        sel.removeAllRanges();
        if (sel.rangeCount !== 0 || !sel.isCollapsed) throw new Error("Selection not cleared");

        sel.addRange(range);
        if (sel.rangeCount !== 1) throw new Error("Selection rangeCount mismatch");
        if (sel.anchorNode !== p1) throw new Error("Selection anchorNode mismatch");

        sel.selectAllChildren(p1);
        if (sel.rangeCount !== 1 || sel.anchorNode !== p1) throw new Error("selectAllChildren mismatch");

        sel.collapse(p1, 0);
        if (!sel.isCollapsed) throw new Error("Selection collapse mismatch");
    "#;

    assert!(rt.execute_script(script).is_ok());
}

#[test]
fn test_composed_path_across_shadow_dom_and_event_options() {
    let html = r#"<!DOCTYPE html>
    <html>
    <body>
        <div id="host"></div>
    </body>
    </html>"#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var host = document.getElementById("host");
        var shadow = host.attachShadow({ mode: "open" });

        var inner = document.createElement("button");
        inner.textContent = "Click inside shadow";
        shadow.appendChild(inner);

        // 1. Composed event traversal across Shadow boundary into host -> body -> html -> document -> window
        var pathNodes = null;
        host.addEventListener("custom-composed", function(evt) {
            pathNodes = evt.composedPath();
        });

        var compEvt = new Event("custom-composed", { bubbles: true, composed: true });
        inner.dispatchEvent(compEvt);

        if (!pathNodes) throw new Error("composed event did not reach host");
        if (pathNodes.indexOf(inner) === -1 || pathNodes.indexOf(shadow) === -1 || pathNodes.indexOf(host) === -1) {
            throw new Error("composedPath did not cross shadow boundary");
        }

        // 2. Non-composed event stops at shadow root
        var reachedHostNonComposed = false;
        host.addEventListener("custom-non-composed", function() {
            reachedHostNonComposed = true;
        });
        var nonCompEvt = new Event("custom-non-composed", { bubbles: true, composed: false });
        inner.dispatchEvent(nonCompEvt);
        if (reachedHostNonComposed) throw new Error("Non-composed event leaked past shadow boundary");

        // 3. Passive listener cannot preventDefault
        var passivePrevented = false;
        inner.addEventListener("test-passive", function(evt) {
            evt.preventDefault();
            passivePrevented = evt.defaultPrevented;
        }, { passive: true });

        var passEvt = new Event("test-passive", { cancelable: true });
        inner.dispatchEvent(passEvt);
        if (passivePrevented || passEvt.defaultPrevented) {
            throw new Error("Passive listener was able to preventDefault");
        }

        // 4. Once listener auto-removes after single invocation
        var count = 0;
        inner.addEventListener("test-once", function() {
            count++;
        }, { once: true });

        inner.dispatchEvent(new Event("test-once"));
        inner.dispatchEvent(new Event("test-once"));
        if (count !== 1) throw new Error("Once listener fired " + count + " times");

        // 5. AbortSignal listener auto-removal
        var ac = new AbortController();
        var abortCount = 0;
        inner.addEventListener("test-abort", function() {
            abortCount++;
        }, { signal: ac.signal });

        inner.dispatchEvent(new Event("test-abort"));
        ac.abort();
        inner.dispatchEvent(new Event("test-abort"));
        if (abortCount !== 1) throw new Error("Aborted listener fired after abort");
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("Script execution failed: {}", e);
    }
}

#[test]
fn test_visual_validation_bubble_subsystem() {
    use mango_browser::form_handler::ValidationBubble;

    let node_id = mango_html::dom::NodeId::from_raw(99);
    let bubble = ValidationBubble::new(
        node_id,
        150.0,
        250.0,
        "Please select an item in the list.".to_string(),
    );

    assert_eq!(bubble.target_node_id, node_id);
    assert_eq!(bubble.x, 150.0);
    assert_eq!(bubble.y, 250.0);
    assert!(bubble.width >= 160.0);
    assert_eq!(bubble.height, 36.0);
    assert_eq!(bubble.message, "Please select an item in the list.");

    // Hit-testing
    assert!(bubble.contains(160.0, 260.0));
    assert!(!bubble.contains(10.0, 10.0));

    // Render display list commands
    let commands = bubble.render();
    assert!(commands.len() >= 5);
}
