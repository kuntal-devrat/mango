//! Integration tests for Mango's JavaScript runtime (powered by Boa) and DOM bindings.

use mango_html::parse_html;
use mango_js::JsRuntime;

#[test]
fn test_js_evaluation_and_math() {
    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var x = 10;
        var y = 25;
        var sum = x + y;
        console.log("Sum result:", sum);
    "#;
    assert!(rt.execute_script(script).is_ok());

    let logs = rt.drain_console_messages();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].text, "Sum result: 35");
}

#[test]
fn test_js_dom_manipulation_get_element() {
    let html = r#"
        <html>
        <body>
            <h1 id="heading">Original Title</h1>
            <p id="desc">Description text</p>
        </body>
        </html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var h1 = document.getElementById("heading");
        console.log("Initial heading:", h1.textContent);
        h1.textContent = "Modified by Boa JavaScript!";

        var desc = document.getElementById("desc");
        desc.textContent = "Updated description!";
    "#;
    assert!(rt.execute_script(script).is_ok());
    assert!(rt.is_dom_dirty());

    let logs = rt.drain_console_messages();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].text, "Initial heading: Original Title");

    // Verify modified DOM snapshot
    let updated_doc = rt.document_snapshot();
    let root = updated_doc.root();
    let h1_node = updated_doc.find_element_by_tag(root, "h1").unwrap();
    assert_eq!(updated_doc.text_content(h1_node), "Modified by Boa JavaScript!");

    let p_node = updated_doc.find_element_by_tag(root, "p").unwrap();
    assert_eq!(updated_doc.text_content(p_node), "Updated description!");
}

#[test]
fn test_js_dom_create_element_and_attributes() {
    let html = r#"<html><body><ul id="list"></ul></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var list = document.getElementById("list");
        var item1 = document.createElement("li");
        item1.textContent = "Item One";
        item1.setAttribute("class", "list-item active");
        list.appendChild(item1);

        var item2 = document.createElement("li");
        item2.textContent = "Item Two";
        list.appendChild(item2);
    "#;
    assert!(rt.execute_script(script).is_ok());
    assert!(rt.is_dom_dirty());

    let updated_doc = rt.document_snapshot();
    let root = updated_doc.root();
    let ul = updated_doc.find_element_by_tag(root, "ul").unwrap();
    let children: Vec<_> = updated_doc.children(ul).collect();
    assert_eq!(children.len(), 2);

    let li1 = children[0].id;
    assert_eq!(updated_doc.text_content(li1), "Item One");
    if let mango_html::dom::NodeData::Element(ref elem) = updated_doc.get(li1).unwrap().data {
        assert_eq!(elem.get_attribute("class"), Some("list-item active"));
    } else {
        panic!("Expected element node");
    }
}

#[test]
fn test_js_query_selector() {
    let html = r#"
        <html>
        <body>
            <div class="card">
                <span id="target">Found Target</span>
            </div>
        </body>
        </html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var byId = document.querySelector('#target');
        console.log("querySelector id:", byId.textContent);

        var byClass = document.querySelector('.card');
        console.log("querySelector class tagName:", byClass.tagName);
    "#;
    assert!(rt.execute_script(script).is_ok());

    let logs = rt.drain_console_messages();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].text, "querySelector id: Found Target");
    assert_eq!(logs[1].text, "querySelector class tagName: DIV");
}

#[test]
fn test_js_set_timeout_execution() {
    let html = r#"<html><body><div id="status">idle</div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        setTimeout(function() {
            var el = document.getElementById("status");
            el.textContent = "done";
        }, 15);
    "#;
    assert!(rt.execute_script(script).is_ok());
    assert!(rt.has_pending_timers());

    // Before timer expires, status is still idle
    let before = rt.document_snapshot();
    let div = before.find_element_by_tag(before.root(), "div").unwrap();
    assert_eq!(before.text_content(div), "idle");

    // Sleep to let timer expire
    std::thread::sleep(std::time::Duration::from_millis(30));
    let fired = rt.tick();
    assert!(fired);

    // After timer expires, status is updated to done
    let after = rt.document_snapshot();
    let div_after = after.find_element_by_tag(after.root(), "div").unwrap();
    assert_eq!(after.text_content(div_after), "done");
}

#[test]
fn test_js_alert_and_window_properties() {
    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new(doc, 1024.0, 768.0);

    let script = r#"
        alert("Welcome to Mango Browser!");
        console.log("Window innerWidth:", innerWidth);
        console.log("Window innerHeight:", innerHeight);
    "#;
    assert!(rt.execute_script(script).is_ok());

    assert_eq!(rt.take_status_text(), Some("Alert: Welcome to Mango Browser!".to_string()));

    let logs = rt.drain_console_messages();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].text, "Window innerWidth: 1024");
    assert_eq!(logs[1].text, "Window innerHeight: 768");
}

#[test]
fn test_js_window_self_top_parent_and_location() {
    let doc = parse_html("<html><head><title>My Test Page</title></head><body><h1 id='title'>Test</h1></body></html>");
    let mut rt = JsRuntime::new_with_url(doc, 1024.0, 768.0, "https://example.com/search?q=rust");

    let script = r#"
        console.log("window is self:", window === self);
        console.log("window is top:", window === top);
        console.log("window is parent:", window === parent);
        console.log("protocol:", location.protocol);
        console.log("host:", location.host);
        console.log("pathname:", location.pathname);
        console.log("search:", location.search);
        console.log("origin:", location.origin);
        console.log("navigator userAgent:", navigator.userAgent.indexOf("Mango") !== -1);
        console.log("screen width:", screen.width);
        console.log("storage test:", typeof localStorage.setItem === 'function');
        localStorage.setItem("mango_key", "mango_val");
        console.log("storage get:", localStorage.getItem("mango_key"));
    "#;
    assert!(rt.execute_script(script).is_ok());

    let logs = rt.drain_console_messages();
    assert!(logs.iter().any(|m| m.text == "window is self: true"));
    assert!(logs.iter().any(|m| m.text == "window is top: true"));
    assert!(logs.iter().any(|m| m.text == "window is parent: true"));
    assert!(logs.iter().any(|m| m.text == "protocol: https:"));
    assert!(logs.iter().any(|m| m.text == "host: example.com"));
    // Per the WHATWG URL spec, `pathname` excludes the query string; it lives in `search`.
    assert!(logs.iter().any(|m| m.text == "pathname: /search"));
    assert!(logs.iter().any(|m| m.text == "search: ?q=rust"));
    assert!(logs.iter().any(|m| m.text == "origin: https://example.com"));
    assert!(logs.iter().any(|m| m.text == "navigator userAgent: true"));
    assert!(logs.iter().any(|m| m.text == "storage get: mango_val"));
}

#[test]
fn test_js_dom_completeness_apis() {
    let html = r#"
        <html><body>
            <div id="host" data-role="panel" data-item-count="3">
                <p id="para">Hello</p>
            </div>
            <form id="form1"><input name="q" value="x"/></form>
            <img id="pic" src="a.png"/>
            <a id="link" href="https://example.com">Link</a>
        </body></html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var host = document.getElementById('host');
        console.log('tag:', host.tagName, 'nodeType:', host.nodeType, 'nodeName:', host.nodeName);
        console.log('dataset role:', host.dataset.role, 'count:', host.dataset.itemCount);
        host.dataset.role = 'toolbar';
        console.log('dataset write:', host.getAttribute('data-role'));
        console.log('innerHTML:', host.innerHTML.replace(/\s+/g, ''));

        // cloneNode(deep) copies structure
        var clone = host.cloneNode(true);
        console.log('clone children:', clone.children.length);

        // DocumentFragment insertion flattens
        var frag = document.createDocumentFragment();
        var a = document.createElement('span');
        a.textContent = 'A';
        var b = document.createElement('span');
        b.textContent = 'B';
        frag.appendChild(a);
        frag.appendChild(b);
        console.log('fragment nodeType:', frag.nodeType);
        host.appendChild(frag);
        console.log('host children after fragment:', host.children.length);

        // insertBefore / replaceChild / remove
        var first = document.getElementById('para');
        var replaced = document.createElement('h2');
        replaced.textContent = 'Replaced';
        host.replaceChild(replaced, first);
        console.log('has para:', document.getElementById('para') !== null);

        // collection accessors
        console.log('forms:', document.forms.length, 'images:', document.images.length, 'links:', document.links.length);

        // measurement APIs come from the layout bridge (0 without a browser layout)
        var rect = host.getBoundingClientRect();
        console.log('rect type:', typeof rect.width, 'offsetW:', typeof host.offsetWidth);

        // contains reflects ancestry
        console.log('contains self:', host.contains(host), 'contains child:', host.contains(replaced), 'unknown:', host.contains(null));
        console.log('doc contains host:', document.contains(host));
    "#;
    assert!(rt.execute_script(script).is_ok(), "DOM completeness script failed");

    let logs = rt.drain_console_messages();
    let find = |needle: &str| logs.iter().any(|m| m.text.contains(needle));

    assert!(find("tag: DIV nodeType: 1 nodeName: DIV"), "nodes expose nodeType/nodeName");
    assert!(find("dataset role: panel count: 3"), "dataset reads data-* camelCased");
    assert!(find("dataset write: toolbar"), "dataset writes back to attributes");
    // The script strips whitespace, which also removes attribute separators.
    assert!(
        find("innerHTML: <pid=\"para\">Hello</p>"),
        "innerHTML serializes markup; logs were: {:?}",
        logs.iter().map(|m| m.text.clone()).collect::<Vec<_>>()
    );
    assert!(find("clone children: 1"), "cloneNode(true) copies descendants");
    assert!(find("fragment nodeType: 11"), "createDocumentFragment returns a fragment");
    assert!(
        find("host children after fragment: 3"),
        "fragment children are flattened in (1 <p> + 2 <span>); logs were: {:?}",
        logs.iter().map(|m| m.text.clone()).collect::<Vec<_>>()
    );
    assert!(find("has para: false"), "replaceChild detaches the old child");
    assert!(find("forms: 1 images: 1 links: 1"), "document collections resolve");
    assert!(find("rect type: number offsetW: number"), "measurement APIs return numbers");
    assert!(
        find("contains self: true contains child: true unknown: false"),
        "contains walks ancestors; logs were: {:?}",
        logs.iter().map(|m| m.text.clone()).collect::<Vec<_>>()
    );
    assert!(
        find("doc contains host: true"),
        "document.contains walks to the root; logs were: {:?}",
        logs.iter().map(|m| m.text.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn test_js_innerhtml_and_outerhtml_setters() {
    let html = r#"<html><body><div id="root"></div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var root = document.getElementById('root');
        root.innerHTML = '<ul><li class="item">One</li><li>Two</li></ul>';
        console.log('items:', root.querySelectorAll('li').length);
        console.log('first class:', root.querySelector('.item').textContent);
        var li = root.querySelector('.item');
        li.outerHTML = '<li id="replaced">Changed</li>';
        console.log('replaced id:', document.querySelector('#replaced') !== null);
        console.log('old gone:', document.querySelector('.item') === null);
    "#;
    assert!(rt.execute_script(script).is_ok());

    let updated = rt.document_snapshot();
    let root = updated.root();
    let div = updated.find_element_by_id(root, "root").unwrap();
    assert_eq!(updated.children(div).count(), 1, "one <ul> child");
    let ul = updated.children(div).next().unwrap().id;
    assert_eq!(updated.children(ul).count(), 2, "two <li> children after innerHTML set");

    let logs = rt.drain_console_messages();
    let find = |needle: &str| logs.iter().any(|m| m.text.contains(needle));
    assert!(find("items: 2"), "innerHTML parses markup into real nodes");
    assert!(find("first class: One"), "parsed nodes are queryable");
    assert!(find("replaced id: true"), "outerHTML setter replaces the node");
    assert!(find("old gone: true"), "previous node is detached");
}

#[test]
fn test_js_neverssl_redirect_simulation() {
    let doc = parse_html("<html><head><script></script></head><body><h1>NeverSSL</h1></body></html>");
    let mut rt = JsRuntime::new_with_url(doc, 800.0, 600.0, "http://neverssl.com/");

    let script = r#"
        var prefix = "calmlove";
        window.location.href = 'http://' + prefix + '.neverssl.com/online';
    "#;
    assert!(rt.execute_script(script).is_ok());

    let pending = rt.take_pending_navigation();
    assert_eq!(pending, Some("http://calmlove.neverssl.com/online".to_string()));
}

#[test]
fn test_js_rich_dom_apis() {
    let html = r#"
        <html>
        <head><title>Initial Title</title></head>
        <body>
            <div id="card" class="box active">Hello</div>
        </body>
        </html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        var card = document.getElementById("card");
        console.log("card className:", card.className);
        console.log("classList contains active:", card.classList.contains("active"));
        card.classList.add("highlight");
        card.style.color = "red";
        document.title = "New Mango Title";
        console.log("document.body exists:", document.body !== null);
        console.log("document.head exists:", document.head !== null);
    "#;
    assert!(rt.execute_script(script).is_ok());

    let logs = rt.drain_console_messages();
    assert!(logs.iter().any(|m| m.text == "card className: box active"));
    assert!(logs.iter().any(|m| m.text == "classList contains active: true"));
    assert!(logs.iter().any(|m| m.text == "document.body exists: true"));
    assert!(logs.iter().any(|m| m.text == "document.head exists: true"));
}

#[test]
fn test_js_form_control_api() {
    let html = r#"
        <html><body>
            <form id="signup">
                <input id="email" type="email" required value="">
                <input id="agree" type="checkbox" required>
                <input id="age" type="number" min="18">
            </form>
        </body></html>
    "#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new_with_url(doc, 800.0, 600.0, "https://example.com/signup");

    let script = r#"
        var email = document.getElementById('email');
        var agree = document.getElementById('agree');
        var age = document.getElementById('age');

        console.log('initial valid:', email.checkValidity());
        email.value = 'not-an-email';
        console.log('bad type valid:', email.checkValidity());
        console.log('typeMismatch:', email.validity.typeMismatch);
        email.value = 'ada@example.com';
        console.log('good type valid:', email.checkValidity());
        console.log('required value:', email.value);

        console.log('checkbox valid:', agree.checkValidity());
        agree.checked = true;
        console.log('checked now:', agree.checked);
        console.log('checkbox valid after:', agree.checkValidity());

        console.log('age type:', age.type);
        age.value = 'nope';
        console.log('age invalid number:', age.checkValidity());
        age.value = '21';
        console.log('age valid number:', age.checkValidity());

        email.setCustomValidity('no thanks');
        console.log('custom error blocks:', email.checkValidity());
        email.setCustomValidity('');
        console.log('custom cleared:', email.checkValidity());

        // Domains / defaults
        console.log('form owner:', email.form && email.form.id);
        console.log('willValidate:', email.willValidate);
        console.log('validationMessage empty when valid:', email.validationMessage);
    "#;
    assert!(rt.execute_script(script).is_ok(), "form-control script runs");

    let logs = rt.drain_console_messages();
    let find = |needle: &str| logs.iter().any(|m| m.text == needle);
    assert!(find("initial valid: false"), "required+empty input is invalid");
    assert!(find("bad type valid: false"), "bad email is invalid");
    assert!(find("typeMismatch: true"));
    assert!(find("good type valid: true"));
    assert!(find("required value: ada@example.com"), "value setter sticks");
    assert!(find("checkbox valid: false"), "required checkbox unchecked");
    assert!(find("checked now: true"));
    assert!(find("checkbox valid after: true"));
    assert!(find("age type: number"));
    assert!(find("age invalid number: false"));
    assert!(find("age valid number: true"));
    assert!(find("custom error blocks: false"));
    assert!(find("custom cleared: true"));
    assert!(find("form owner: signup"), "form walks to ancestor <form>");
    assert!(find("willValidate: true"));
    assert!(find("validationMessage empty when valid: "));
}

#[test]
fn test_document_cookie_is_shared_with_jar() {
    let html = "<html><body><p>hi</p></body></html>";
    let doc = parse_html(html);
    let jar: std::sync::Arc<std::sync::Mutex<mango_net::CookieJar>> =
        std::sync::Arc::new(std::sync::Mutex::new(mango_net::CookieJar::new()));
    let storage: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

    let mut rt = JsRuntime::new_with_url_storage_and_cookies(
        doc,
        800.0,
        600.0,
        "https://example.com/app/page",
        storage,
        jar.clone(),
    );

    let script = r#"
        console.log('initial cookie:', document.cookie);
        document.cookie = 'sid=abc123; Path=/';
        document.cookie = 'theme=dark';
        console.log('after set:', document.cookie);
        document.cookie = 'sid=; Path=/; Max-Age=0';
        console.log('after delete:', document.cookie);
    "#;
    assert!(rt.execute_script(script).is_ok(), "document.cookie script runs");

    let logs = rt.drain_console_messages();
    let get = |needle: &str| {
        logs.iter()
            .find(|m| m.text.starts_with(needle))
            .map(|m| m.text.clone())
    };
    assert_eq!(
        get("initial cookie:"),
        Some("initial cookie: ".to_string()),
        "starts empty: {:?}",
        get("initial cookie:")
    );
    let after = get("after set:").expect("after set log");
    assert!(after.contains("sid=abc123"), "got {after}");
    assert!(after.contains("theme=dark"), "got {after}");
    let after_delete = get("after delete:").expect("after delete log");
    assert!(!after_delete.contains("sid=abc123"), "got {after_delete}");
    assert!(after_delete.contains("theme=dark"), "got {after_delete}");

    // The jar itself — the same handle the HTTP loader uses — saw the writes.
    // `theme=dark` was set on /app/page with no Path, so its default path is
    // "/app" (RFC 6265 §5.1.4) — it must go to /app/* and not to the root.
    let header = jar
        .lock()
        .unwrap()
        .get_cookie_header(&mango_net::Url::parse("https://example.com/app/x").unwrap())
        .unwrap_or_default();
    assert!(header.contains("theme=dark"), "loader would send {header}");
    assert!(!header.contains("sid=abc123"), "loader would send {header}");
    let root_header = jar
        .lock()
        .unwrap()
        .get_cookie_header(&mango_net::Url::parse("https://example.com/").unwrap())
        .unwrap_or_default();
    assert!(
        !root_header.contains("theme=dark"),
        "path-scoped cookie must not leak to root: {root_header}"
    );
}

#[test]
fn test_js_selection_api_on_input() {
    let html = r#"<html><body><input id="t" type="text" value="hello"></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);
    let script = r#"
        var t = document.getElementById('t');
        t.select();
        console.log('sel start:', t.selectionStart, 'end:', t.selectionEnd);
        t.setSelectionRange(1, 3, 'forward');
        console.log('narrowed:', t.selectionStart === 1 && t.selectionEnd === 3);
        console.log('direction:', t.selectionDirection);
    "#;
    assert!(rt.execute_script(script).is_ok());
    let logs = rt.drain_console_messages();
    assert!(
        logs.iter().any(|m| m.text.contains("sel start: 0") && m.text.contains("end: 5")),
        "select() covers the whole value: {:?}",
        logs.iter().map(|m| m.text.clone()).collect::<Vec<_>>()
    );
    assert!(logs.iter().any(|m| m.text == "narrowed: true"));
    assert!(logs.iter().any(|m| m.text == "direction: forward"));
}

#[test]
fn test_js_document_character_set_and_content_type() {
    let mut doc = parse_html(r#"<!DOCTYPE html><html><head><meta charset="windows-1252"></head><body><h1>Encoding</h1></body></html>"#);
    assert_eq!(doc.character_set, "windows-1252");
    doc.content_type = "text/html".to_string();

    let mut rt = JsRuntime::new(doc, 800.0, 600.0);
    let script = r#"
        console.log("characterSet:", document.characterSet);
        console.log("charset:", document.charset);
        console.log("inputEncoding:", document.inputEncoding);
        console.log("contentType:", document.contentType);
    "#;
    assert!(rt.execute_script(script).is_ok());

    let logs = rt.drain_console_messages();
    assert!(logs.iter().any(|m| m.text == "characterSet: windows-1252"));
    assert!(logs.iter().any(|m| m.text == "charset: windows-1252"));
    assert!(logs.iter().any(|m| m.text == "inputEncoding: windows-1252"));
    assert!(logs.iter().any(|m| m.text == "contentType: text/html"));
}

#[test]
fn test_local_storage_origin_isolation() {
    use mango_js::web_apis::SharedLocalStorage;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    let storage: SharedLocalStorage = Arc::new(Mutex::new(HashMap::new()));

    // Origin 1: https://site-a.com
    let doc1 = parse_html("<html><body></body></html>");
    let mut rt1 = JsRuntime::new_with_url_and_storage(
        doc1,
        800.0,
        600.0,
        "https://site-a.com/dashboard",
        storage.clone(),
    );
    let s1 = r#"
        localStorage.setItem("user", "Alice");
        localStorage.setItem("session", "A-123");
        console.log("rt1 set item done");
    "#;
    assert!(rt1.execute_script(s1).is_ok());

    // Origin 2: https://site-b.com (should NOT see site-a data)
    let doc2 = parse_html("<html><body></body></html>");
    let mut rt2 = JsRuntime::new_with_url_and_storage(
        doc2,
        800.0,
        600.0,
        "https://site-b.com/profile",
        storage.clone(),
    );
    let s2 = r#"
        console.log("rt2 user:", localStorage.getItem("user"));
        console.log("rt2 length:", localStorage.length);
        localStorage.setItem("user", "Bob");
        console.log("rt2 new user:", localStorage.getItem("user"));
    "#;
    assert!(rt2.execute_script(s2).is_ok());
    let logs2 = rt2.drain_console_messages();
    assert!(logs2.iter().any(|m| m.text == "rt2 user: null"));
    assert!(logs2.iter().any(|m| m.text == "rt2 length: 0"));
    assert!(logs2.iter().any(|m| m.text == "rt2 new user: Bob"));

    // Origin 1 again: https://site-a.com/another-page (same origin, should see Alice, not Bob)
    let doc3 = parse_html("<html><body></body></html>");
    let mut rt3 = JsRuntime::new_with_url_and_storage(
        doc3,
        800.0,
        600.0,
        "https://site-a.com/settings",
        storage.clone(),
    );
    let s3 = r#"
        console.log("rt3 user:", localStorage.getItem("user"));
        console.log("rt3 session:", localStorage.getItem("session"));
        console.log("rt3 length:", localStorage.length);
    "#;
    assert!(rt3.execute_script(s3).is_ok());
    let logs3 = rt3.drain_console_messages();
    assert!(logs3.iter().any(|m| m.text == "rt3 user: Alice"));
    assert!(logs3.iter().any(|m| m.text == "rt3 session: A-123"));
    assert!(logs3.iter().any(|m| m.text == "rt3 length: 2"));

    // Direct check of underlying storage map:
    let map = storage.lock().unwrap();
    assert_eq!(map.get("https://site-a.com\x1fuser").map(|s| s.as_str()), Some("Alice"));
    assert_eq!(map.get("https://site-b.com\x1fuser").map(|s| s.as_str()), Some("Bob"));
}

#[test]
fn test_local_storage_proxy_property_access_and_storage_event() {
    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new_with_url_and_storage(
        doc,
        800.0,
        600.0,
        "https://example.com/app",
        std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
    );

    let script = r#"
        var eventLog = [];
        window.addEventListener('storage', function(e) {
            eventLog.push({ key: e.key, oldVal: e.oldValue, newVal: e.newValue });
        });

        // 1. Direct property assignment via Proxy
        localStorage.theme = "dark";
        console.log("prop get:", localStorage.theme);
        console.log("item get:", localStorage.getItem("theme"));
        console.log("in operator:", "theme" in localStorage);

        // 2. Storage event was dispatched on set
        console.log("event on set count:", eventLog.length);
        if (eventLog.length > 0) {
            console.log("event set key:", eventLog[0].key, "new:", eventLog[0].newVal);
        }

        // 3. Property deletion via Proxy
        delete localStorage.theme;
        console.log("after delete prop:", localStorage.theme);
        console.log("after delete item:", localStorage.getItem("theme"));
        console.log("event on del count:", eventLog.length);
        if (eventLog.length > 1) {
            console.log("event del key:", eventLog[1].key, "old:", eventLog[1].oldVal);
        }
    "#;
    assert!(rt.execute_script(script).is_ok());
    let logs = rt.drain_console_messages();
    assert!(logs.iter().any(|m| m.text == "prop get: dark"));
    assert!(logs.iter().any(|m| m.text == "item get: dark"));
    assert!(logs.iter().any(|m| m.text == "in operator: true"));
    assert!(logs.iter().any(|m| m.text == "event on set count: 1"));
    assert!(logs.iter().any(|m| m.text == "event set key: theme new: dark"));
    assert!(logs.iter().any(|m| m.text == "after delete prop: undefined"));
    assert!(logs.iter().any(|m| m.text == "after delete item: null"));
    assert!(logs.iter().any(|m| m.text == "event on del count: 2"));
    assert!(logs.iter().any(|m| m.text == "event del key: theme old: dark"));
}

#[test]
fn test_indexeddb_basic_crud_and_persistence() {
    use mango_js::web_apis::SharedLocalStorage;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    let shared_storage: SharedLocalStorage = Arc::new(Mutex::new(HashMap::new()));

    // Page 1: Open database, create store in onupgradeneeded, insert record
    {
        let doc = parse_html("<html><body></body></html>");
        let mut rt = JsRuntime::new_with_url_and_storage(
            doc,
            800.0,
            600.0,
            "https://myapp.com/index.html",
            shared_storage.clone(),
        );

        let script = r#"
            var req = indexedDB.open("AppDB", 1);
            req.onupgradeneeded = function(e) {
                var db = req.result;
                var store = db.createObjectStore("items", { keyPath: "id" });
            };

            req.onsuccess = function(e) {
                var db = req.result;
                var tx = db.transaction("items", "readwrite");
                var store = tx.objectStore("items");
                store.put({ id: 101, title: "Mango Browser", fast: true });

                var getReq = store.get(101);
                getReq.onsuccess = function() {
                    retrieved = getReq.result;
                    console.log("retrieved title:", retrieved.title);
                    console.log("retrieved fast:", retrieved.fast);
                };
            };
        "#;
        assert!(rt.execute_script(script).is_ok());
        let logs = rt.drain_console_messages();
        assert!(logs.iter().any(|m| m.text == "retrieved title: Mango Browser"));
        assert!(logs.iter().any(|m| m.text == "retrieved fast: true"));
    }

    // Verify storage map directly contains the serialized IndexedDB data
    {
        let map = shared_storage.lock().unwrap();
        let idb_key = "https://myapp.com\x1f__idb__AppDB";
        assert!(map.contains_key(idb_key), "IndexedDB was serialized into backing storage");
        let payload = map.get(idb_key).unwrap();
        assert!(payload.contains("Mango Browser"), "Record found in IDB payload: {payload}");
    }

    // Page 2: Re-open the database on the same origin; data must still be there!
    {
        let doc2 = parse_html("<html><body></body></html>");
        let mut rt2 = JsRuntime::new_with_url_and_storage(
            doc2,
            800.0,
            600.0,
            "https://myapp.com/dashboard",
            shared_storage.clone(),
        );

        let script2 = r#"
            var req = indexedDB.open("AppDB", 1);
            var reloadedTitle = null;
            var allItemsCount = 0;

            req.onsuccess = function(e) {
                var db = req.result;
                var tx = db.transaction("items", "readonly");
                var store = tx.objectStore("items");

                var getReq = store.get(101);
                getReq.onsuccess = function() {
                    reloadedTitle = getReq.result ? getReq.result.title : null;
                    console.log("reloaded title:", reloadedTitle);
                };

                var getAllReq = store.getAll();
                getAllReq.onsuccess = function() {
                    allItemsCount = getAllReq.result.length;
                    console.log("getAll count:", allItemsCount);
                };
            };
        "#;
        assert!(rt2.execute_script(script2).is_ok());
        let logs2 = rt2.drain_console_messages();
        assert!(logs2.iter().any(|m| m.text == "reloaded title: Mango Browser"));
        assert!(logs2.iter().any(|m| m.text == "getAll count: 1"));
    }

    // Page 3: Different origin https://otherapp.com trying to open AppDB should have a clean, isolated DB
    {
        let doc3 = parse_html("<html><body></body></html>");
        let mut rt3 = JsRuntime::new_with_url_and_storage(
            doc3,
            800.0,
            600.0,
            "https://otherapp.com/page",
            shared_storage.clone(),
        );

        let script3 = r#"
            var req = indexedDB.open("AppDB", 1);
            req.onupgradeneeded = function() {
                console.log("otherapp needed upgrade because it is a new isolated DB");
            };
            req.onsuccess = function() {
                var db = req.result;
                console.log("otherapp objectStoreNames length:", db.objectStoreNames.length);
            };
        "#;
        assert!(rt3.execute_script(script3).is_ok());
        let logs3 = rt3.drain_console_messages();
        assert!(logs3.iter().any(|m| m.text.contains("otherapp needed upgrade")));
        assert!(logs3.iter().any(|m| m.text == "otherapp objectStoreNames length: 0"));
    }
}
