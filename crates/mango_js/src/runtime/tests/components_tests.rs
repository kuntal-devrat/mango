use super::*;

#[test]
fn test_view_transitions_api() {
    let doc = parse_html("<html><body><div id='content'>old</div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        if (typeof document.startViewTransition !== 'function') {
            throw new Error("document.startViewTransition should be a function");
        }
        if (typeof ViewTransition !== 'function') {
            throw new Error("ViewTransition should be a global constructor");
        }

        var callbackCalled = false;
        var vt = document.startViewTransition(function() {
            callbackCalled = true;
            document.getElementById('content').textContent = 'new';
            return "done";
        });

        if (!(vt.ready instanceof Promise)) throw new Error("vt.ready should be a Promise");
        if (!(vt.finished instanceof Promise)) throw new Error("vt.finished should be a Promise");
        if (!(vt.updateCallbackDone instanceof Promise)) throw new Error("vt.updateCallbackDone should be a Promise");

        // Level 2: types Set
        if (!(vt.types instanceof Set)) throw new Error("vt.types should be a Set");
        vt.types.add('slide-left');
        vt.types.add('fade');
        if (!vt.types.has('slide-left') || !vt.types.has('fade')) {
            throw new Error("vt.types should store added types");
        }

        if (typeof vt.skipTransition !== 'function') {
            throw new Error("vt.skipTransition should be a function");
        }
        vt.skipTransition();
    "#;
    if let Err(e) = rt.execute_script(script) {
        panic!("View Transitions test failed: {}", e);
    }
}

#[test]
fn test_web_components_and_template_slot() {
    let doc = parse_html(
        "<html><body><div id='host'><slot>default</slot></div><template id='tmpl'><p>inside template</p></template></body></html>",
    );
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. Template element .content DocumentFragment
        var tmpl = document.getElementById("tmpl");
        if (!tmpl || !(tmpl instanceof HTMLTemplateElement)) throw new Error("tmpl not HTMLTemplateElement");
        var content = tmpl.content;
        if (!content || !(content instanceof DocumentFragment)) throw new Error("tmpl.content not DocumentFragment");
        var cloned = content.cloneNode(true);
        if (!cloned) throw new Error("content.cloneNode failed");

        // 2. Element.attachShadow & ShadowRoot
        var host = document.getElementById("host");
        if (!host) throw new Error("host element not found");
        var shadow = host.attachShadow({ mode: "open" });
        if (!shadow) throw new Error("attachShadow returned falsy");
        if (host.shadowRoot !== shadow) throw new Error("host.shadowRoot mismatch");
        if (shadow.host !== host) throw new Error("shadow.host mismatch");
        if (shadow.mode !== "open") throw new Error("shadow.mode mismatch");

        // Shadow DOM content
        var p = document.createElement("p");
        p.textContent = "Shadow text";
        shadow.appendChild(p);
        if (shadow.childNodes.length !== 1) throw new Error("shadow childNodes length mismatch");

        // 3. Slot element
        var slot = document.createElement("slot");
        if (!(slot instanceof HTMLSlotElement)) throw new Error("slot not HTMLSlotElement");
        shadow.appendChild(slot);

        var span = document.createElement("span");
        span.textContent = "Light Span";
        host.appendChild(span);

        var assigned = slot.assignedNodes();
        if (!Array.isArray(assigned)) throw new Error("assignedNodes not array");
        if (!assigned.includes(span)) throw new Error("assignedNodes failed to match light span");

        var assignedElems = slot.assignedElements();
        if (!assignedElems.includes(span)) throw new Error("assignedElements failed to match light span");
        if (span.assignedSlot !== slot) throw new Error("span.assignedSlot mismatch");

        // Named slot matching
        var namedSlot = document.createElement("slot");
        namedSlot.setAttribute("name", "badge");
        shadow.appendChild(namedSlot);

        var b = document.createElement("b");
        b.setAttribute("slot", "badge");
        host.appendChild(b);

        var badgeNodes = namedSlot.assignedNodes();
        if (!badgeNodes.includes(b)) throw new Error("namedSlot assignedNodes failed");
        if (b.assignedSlot !== namedSlot) throw new Error("b.assignedSlot mismatch");

        // Fallback content with flatten
        var emptySlot = document.createElement("slot");
        emptySlot.setAttribute("name", "nonexistent");
        var fallbackText = document.createTextNode("Fallback");
        emptySlot.appendChild(fallbackText);
        shadow.appendChild(emptySlot);

        if (emptySlot.assignedNodes().length !== 0) throw new Error("unassigned slot without flatten must be empty");
        var flatNodes = emptySlot.assignedNodes({ flatten: true });
        if (flatNodes.length !== 1 || flatNodes[0] !== fallbackText) throw new Error("flatten should return fallback content");

        // 4. CustomElementRegistry Lifecycle
        var connectedCalls = 0;
        var disconnectedCalls = 0;
        var attrChanges = [];

        function MyElement() {
            HTMLElement.call(this);
        }
        MyElement.prototype = Object.create(HTMLElement.prototype);
        MyElement.observedAttributes = ["data-count", "data-name"];
        MyElement.prototype.connectedCallback = function() {
            connectedCalls++;
        };
        MyElement.prototype.disconnectedCallback = function() {
            disconnectedCalls++;
        };
        MyElement.prototype.attributeChangedCallback = function(name, oldVal, newVal) {
            attrChanges.push({ name: name, oldVal: oldVal, newVal: newVal });
        };

        // CustomElement validation
        var caught = false;
        try {
            customElements.define("invalidname", MyElement);
        } catch(e) {
            caught = true;
        }
        if (!caught) throw new Error("customElements.define should reject tag without hyphen");

        customElements.define("my-element", MyElement);
        if (customElements.get("my-element") !== MyElement) throw new Error("customElements.get failed");
        var myEl = document.createElement("my-element");
        if (!(myEl instanceof MyElement)) throw new Error("myEl not instance of MyElement");

        // Append to body -> connectedCallback
        document.body.appendChild(myEl);
        if (connectedCalls !== 1) throw new Error("connectedCallback should have been called once on appendChild, got: " + connectedCalls);

        // setAttribute -> attributeChangedCallback
        myEl.setAttribute("data-count", "42");
        if (attrChanges.length !== 1 || attrChanges[0].name !== "data-count" || attrChanges[0].newVal !== "42") {
            throw new Error("attributeChangedCallback failed on setAttribute: " + JSON.stringify(attrChanges));
        }

        // removeAttribute -> attributeChangedCallback with null newVal
        myEl.removeAttribute("data-count");
        if (attrChanges.length !== 2 || attrChanges[1].name !== "data-count" || attrChanges[1].newVal !== null) {
            throw new Error("attributeChangedCallback failed on removeAttribute: " + JSON.stringify(attrChanges));
        }

        // removeChild -> disconnectedCallback
        document.body.removeChild(myEl);
        if (disconnectedCalls !== 1) throw new Error("disconnectedCallback should have been called once on removeChild, got: " + disconnectedCalls);

        // 5. Constructable Stylesheets & adoptedStyleSheets
        if (typeof CSSStyleSheet !== "function") throw new Error("CSSStyleSheet constructor missing");
        var sheet = new CSSStyleSheet();
        sheet.replaceSync(".custom { color: purple; }");
        if (sheet.cssRules.length !== 1) throw new Error("sheet.cssRules length mismatch");
        if (sheet.cssRules[0].selectorText !== ".custom") throw new Error("sheet selectorText mismatch");

        document.adoptedStyleSheets = [sheet];
        if (document.adoptedStyleSheets.length !== 1 || document.adoptedStyleSheets[0] !== sheet) {
            throw new Error("document.adoptedStyleSheets mismatch");
        }

        shadow.adoptedStyleSheets = [sheet];
        if (shadow.adoptedStyleSheets.length !== 1 || shadow.adoptedStyleSheets[0] !== sheet) {
            throw new Error("shadow.adoptedStyleSheets mismatch");
        }
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_web_components_and_template_slot failed: {}", e);
    }
}

#[test]
fn test_react_spa_platform_primitives() {
    let doc = parse_html("<!DOCTYPE html><html><body><div id='root'></div></body></html>");
    let mut rt = JsRuntime::new(doc, 1024.0, 768.0);

    let script = r##"
        (function() {
            // 1. Node Environment shims
            if (typeof process === 'undefined' || !process.env || process.env.NODE_ENV !== 'production') {
                throw new Error("process.env.NODE_ENV !== 'production'");
            }
            if (typeof global === 'undefined' || global !== globalThis) {
                throw new Error("global !== globalThis");
            }

            // 2. Document ownerDocument and getRootNode
            if (document.ownerDocument !== null) {
                throw new Error("document.ownerDocument must be null");
            }
            if (document.getRootNode() !== document) {
                throw new Error("document.getRootNode() must return document");
            }

            var root = document.getElementById('root');
            if (!root) throw new Error("root element not found");
            if (root.ownerDocument !== document) {
                throw new Error("root.ownerDocument must be document");
            }
            if (root.getRootNode() !== document) {
                throw new Error("root.getRootNode() must be document");
            }
            if (!root.isConnected) {
                throw new Error("root.isConnected must be true");
            }

            var detached = document.createElement('div');
            if (detached.ownerDocument !== document) {
                throw new Error("detached.ownerDocument must be document");
            }
            if (detached.getRootNode() !== detached) {
                throw new Error("detached.getRootNode() must be detached element itself");
            }
            if (detached.isConnected) {
                throw new Error("detached.isConnected must be false");
            }

            // 3. Comment hydration markers
            var comment = document.createComment("");
            if (comment.nodeType !== 8) {
                throw new Error("comment.nodeType must be 8");
            }
            if (comment.nodeName !== "#comment") {
                throw new Error("comment.nodeName must be #comment");
            }
            comment.data = "react-mount-point-unstable";
            if (comment.data !== "react-mount-point-unstable") {
                throw new Error("comment.data mismatch: " + comment.data);
            }
            if (comment.ownerDocument !== document) {
                throw new Error("comment.ownerDocument must be document");
            }
            root.appendChild(comment);
            if (!comment.isConnected) {
                throw new Error("attached comment must have isConnected true");
            }

            // 4. MessageChannel & MessagePort
            if (typeof MessageChannel !== 'function' || typeof MessagePort !== 'function') {
                throw new Error("MessageChannel / MessagePort not defined");
            }
            globalThis.channel = new MessageChannel();
            globalThis.port1Received = null;
            globalThis.channel.port1.onmessage = function(ev) {
                globalThis.port1Received = ev.data;
            };
            globalThis.channel.port2.postMessage({ type: 'schedule-work', deadline: 16 });

            // 5. composedPath
            var btn = document.createElement('button');
            var detachedPath = null;
            btn.addEventListener('click', function(e) {
                detachedPath = e.composedPath();
            });
            btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
            if (!detachedPath || detachedPath.length !== 1 || detachedPath[0] !== btn) {
                throw new Error("detached composedPath must contain only the detached node");
            }

            root.appendChild(btn);
            var attachedPath = null;
            btn.addEventListener('click', function(e) {
                attachedPath = e.composedPath();
            });
            btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
            if (!attachedPath || attachedPath.indexOf(document) === -1 || attachedPath.indexOf(window) === -1) {
                throw new Error("attached composedPath must bubble up to document and window");
            }

            return "ok";
        })()
    "##;

    let res = rt.eval(script).expect("eval react spa platform primitives");
    assert_eq!(res, "ok");

    // Drain event loop / microtasks to let MessagePort deliver message
    rt.run_jobs();
    let check_channel = rt.eval("(function() { return (typeof channel !== 'undefined' && port1Received && port1Received.type === 'schedule-work') ? 'channel-ok' : 'pending'; })()");
    // Verify port communication delivered
    assert_eq!(check_channel.unwrap(), "channel-ok");
}

#[test]
fn test_web_components_v1_es6_classes() {
    let doc = parse_html("<html><body><ytd-app id='app1' theme='dark'></ytd-app></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        (function() {
            var initialAttrChange = null;
            var dynamicAttrChange = null;
            var connectedCount = 0;

            class YtdApp extends HTMLElement {
                constructor() {
                    super();
                    this.initialized = true;
                    this.__data = { version: "v1" };
                }
                static get observedAttributes() {
                    return ["theme", "active"];
                }
                attributeChangedCallback(name, oldVal, newVal, ns) {
                    if (!initialAttrChange) {
                        initialAttrChange = { name: name, oldVal: oldVal, newVal: newVal };
                    } else {
                        dynamicAttrChange = { name: name, oldVal: oldVal, newVal: newVal };
                    }
                }
                connectedCallback() {
                    connectedCount++;
                }
            }

            // 1. Upgrade existing pre-parsed <ytd-app> element
            customElements.define("ytd-app", YtdApp);

            var app1 = document.getElementById("app1");
            if (!app1) throw new Error("app1 not found in DOM");
            if (!(app1 instanceof YtdApp)) throw new Error("app1 not instance of YtdApp");
            if (!(app1 instanceof HTMLElement)) throw new Error("app1 not instance of HTMLElement");
            if (!app1.initialized || !app1.__data || app1.__data.version !== "v1") {
                throw new Error("app1 constructor body was not executed during upgrade");
            }
            if (!initialAttrChange || initialAttrChange.name !== "theme" || initialAttrChange.newVal !== "dark") {
                throw new Error("initial observedAttributes not triggered on upgrade: " + JSON.stringify(initialAttrChange));
            }
            if (connectedCount < 1) {
                throw new Error("connectedCallback should have triggered for attached element");
            }

            // 2. Dynamic attribute changes
            app1.setAttribute("active", "yes");
            if (!dynamicAttrChange || dynamicAttrChange.name !== "active" || dynamicAttrChange.newVal !== "yes") {
                throw new Error("dynamic attributeChangedCallback failed: " + JSON.stringify(dynamicAttrChange));
            }

            // 3. Direct instantiation via `new YtdApp()`
            var direct = new YtdApp();
            if (!(direct instanceof YtdApp)) throw new Error("direct not instance of YtdApp");
            if (!(direct instanceof HTMLElement)) throw new Error("direct not instance of HTMLElement");
            if (direct.tagName !== "YTD-APP") throw new Error("direct tagName mismatch: " + direct.tagName);
            if (!direct.initialized || direct.__data.version !== "v1") {
                throw new Error("direct constructor body was not executed");
            }

            // 4. document.createElement("ytd-app")
            var created = document.createElement("ytd-app");
            if (!(created instanceof YtdApp)) throw new Error("created not instance of YtdApp");
            if (!created.initialized || created.__data.version !== "v1") {
                throw new Error("created constructor body was not executed");
            }

            return "ok";
        })()
    "##;

    let res = rt
        .eval(script)
        .expect("eval test_web_components_v1_es6_classes");
    assert_eq!(res, "ok");
}

#[test]
fn test_custom_elements_phase2_full_lifecycle_and_fragments() {
    let doc = parse_html(
        "<html><body><div id='container'><ytd-rich-grid id='grid'></ytd-rich-grid></div></body></html>",
    );
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        (function() {
            var events = [];
            globalThis._testEvents = events;

            class YtdCard extends HTMLElement {
                constructor() {
                    super();
                    this.constructed = true;
                }
                static get observedAttributes() {
                    return ["video-id"];
                }
                attributeChangedCallback(name, oldVal, newVal) {
                    events.push("attr:" + name + ":" + newVal);
                }
                connectedCallback() {
                    try {
                        events.push("connected:" + (this.id || this.getAttribute("video-id")));
                    } catch (err) {
                        events.push("err:" + String(err));
                    }
                }
                disconnectedCallback() {
                    events.push("disconnected:" + (this.id || this.getAttribute("video-id")));
                }
                adoptedCallback(oldDoc, newDoc) {
                    events.push("adopted:" + (this.id || this.getAttribute("video-id")));
                }
            }

            class YtdGrid extends HTMLElement {
                constructor() {
                    super();
                    this.isGrid = true;
                }
                connectedCallback() {
                    events.push("grid-connected");
                }
            }

            // whenDefined test
            globalThis._resolvedCtor = null;
            customElements.whenDefined("ytd-card").then(function(c) { globalThis._resolvedCtor = c; });
            globalThis._rejRes = null;
            customElements.whenDefined("invalidname").catch(function(e) { globalThis._rejRes = "rejected-invalid"; });

            customElements.define("ytd-card", YtdCard);
            customElements.define("ytd-rich-grid", YtdGrid);

            if (customElements.get("ytd-card") !== YtdCard) throw new Error("customElements.get failed");
            if (customElements.getName(YtdCard) !== "ytd-card") throw new Error("customElements.getName failed");

            var grid = document.getElementById("grid");
            if (!grid || !grid.isGrid) throw new Error("pre-existing grid element was not upgraded");
            if (!events.includes("grid-connected")) throw new Error("grid-connected not fired");

            // Test DocumentFragment appending
            var frag = document.createDocumentFragment();
            var card1 = document.createElement("ytd-card");
            card1.id = "card1";
            card1.setAttribute("video-id", "vid123");
            frag.appendChild(card1);

            // At this point, card1 is not connected to document
            if (events.includes("connected:card1")) throw new Error("card1 should not be connected yet in fragment");

            grid.appendChild(frag);
            if (!events.includes("connected:card1")) throw new Error("card1 should be connected after fragment append");

            // Test cloneNode
            var cloned = card1.cloneNode(true);
            if (!cloned.constructed) throw new Error("cloned card was not upgraded");
            cloned.id = "card1-clone";
            grid.appendChild(cloned);
            if (!events.includes("connected:card1-clone")) throw new Error("cloned card not connected");

            // Test adoptNode
            document.adoptNode(card1);
            if (!events.includes("adopted:card1")) throw new Error("adoptedCallback not fired");

            // Test remove
            card1.remove();
            if (!events.includes("disconnected:card1")) throw new Error("disconnected:card1 not fired. Events so far: " + JSON.stringify(events));

            var container = document.getElementById("container");
            container.innerHTML = "<ytd-card id='card2' video-id='vid999'></ytd-card>";
            var card2 = document.getElementById("card2");
            if (!card2 || !card2.constructed) throw new Error("card2 in innerHTML was not upgraded");
            if (!events.includes("disconnected:card1-clone")) throw new Error("card1-clone should be disconnected on innerHTML replacement");
            if (!events.includes("attr:video-id:vid999")) throw new Error("card2 initial attr not observed");
            if (!events.includes("connected:card2")) throw new Error("card2 not connected");

            return "ok";
        })()
    "##;

    let res = rt
        .eval(script)
        .expect("eval test_custom_elements_phase2_full_lifecycle_and_fragments");
    assert_eq!(res, "ok");

    rt.tick();

    let check = rt.eval(r#"
        if (globalThis._resolvedCtor !== customElements.get("ytd-card")) throw new Error("whenDefined promise resolve failed: " + globalThis._resolvedCtor);
        if (globalThis._rejRes !== "rejected-invalid") throw new Error("whenDefined invalid name reject failed: " + globalThis._rejRes);
        "ok"
    "#).expect("check whenDefined results");
    assert_eq!(check, "ok");
}
