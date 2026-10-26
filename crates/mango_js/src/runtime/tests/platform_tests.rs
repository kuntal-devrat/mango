use super::*;

#[test]
fn test_web_platform_prototypes_and_node_constants() {
    let doc = parse_html("<html><body><div id='test'></div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // Node constants
        if (Node.ELEMENT_NODE !== 1) throw new Error("Node.ELEMENT_NODE mismatch");
        if (Node.TEXT_NODE !== 3) throw new Error("Node.TEXT_NODE mismatch");
        if (Node.DOCUMENT_FRAGMENT_NODE !== 11) throw new Error("Node.DOCUMENT_FRAGMENT_NODE mismatch");

        // Prototypes
        if (!(new HTMLTemplateElement() instanceof HTMLElement)) throw new Error("Template not HTMLElement");
        if (!(new HTMLElement() instanceof Element)) throw new Error("HTMLElement not Element");
        if (!(new Element() instanceof Node)) throw new Error("Element not Node");
        if (!(new Node() instanceof EventTarget)) throw new Error("Node not EventTarget");
        if (!(new ShadowRoot() instanceof DocumentFragment)) throw new Error("ShadowRoot not DocumentFragment");
        if (!(new CDATASection() instanceof Text)) throw new Error("CDATASection not Text");
        if (!(new CustomEvent("test") instanceof Event)) throw new Error("CustomEvent not Event");
        if (!(new MouseEvent("click") instanceof Event)) throw new Error("MouseEvent not Event");
        if (!customElements || !(customElements instanceof CustomElementRegistry)) throw new Error("CustomElements registry missing");
    "#;
    assert!(rt.execute_script(script).is_ok());
}

#[test]
fn test_worker_and_service_worker_apis() {
    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. Worker
        if (typeof Worker !== "function") throw new Error("Worker constructor missing");
        var w = new Worker("worker.js");
        if (!w || typeof w.postMessage !== "function" || typeof w.terminate !== "function") {
            throw new Error("Worker instance interface incomplete");
        }
        w.postMessage({ hello: "world" });
        w.terminate();

        // 2. ServiceWorkerContainer
        if (!navigator.serviceWorker) throw new Error("navigator.serviceWorker missing");
        if (typeof navigator.serviceWorker.register !== "function") throw new Error("navigator.serviceWorker.register missing");
        if (!navigator.serviceWorker.ready) throw new Error("navigator.serviceWorker.ready missing");

        var regPromise = navigator.serviceWorker.register("/sw.js");
        if (!(regPromise instanceof Promise)) throw new Error("register must return a Promise");
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_worker_and_service_worker_apis failed: {}", e);
    }
}

#[test]
fn test_web_apis() {
    let doc = parse_html("<html><body><div id='root'></div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        // 1. URL and URLSearchParams
        var params = new URLSearchParams("a=1&b=2&a=3");
        if (params.get("a") !== "1") throw new Error("params.get failed: " + params.get("a"));
        var allA = params.getAll("a");
        if (allA.length !== 2 || allA[0] !== "1" || allA[1] !== "3") throw new Error("params.getAll failed");
        if (!params.has("b") || params.has("c")) throw new Error("params.has failed");
        params.set("b", "4");
        if (params.get("b") !== "4") throw new Error("params.set failed");
        params.append("c", "5");
        if (params.get("c") !== "5") throw new Error("params.append failed");
        params.delete("a");
        if (params.has("a")) throw new Error("params.delete failed");
        params.set("z", "9");
        params.set("a", "1");
        params.sort();
        var sortedKeys = Array.from(params.keys());
        if (sortedKeys[0] !== "a") throw new Error("params.sort failed");

        // Full spec iterables, mutation, and constructor testing
        var testParams = new URLSearchParams("b=2&a=1");
        var initialKeys = Array.from(testParams.keys());
        if (initialKeys.length !== 2 || initialKeys[0] !== "b" || initialKeys[1] !== "a") {
            throw new Error("initialKeys mismatch: " + JSON.stringify(initialKeys));
        }
        testParams.sort();
        var afterSortKeys = Array.from(testParams.keys());
        if (afterSortKeys.length !== 2 || afterSortKeys[0] !== "a" || afterSortKeys[1] !== "b") {
            throw new Error("afterSortKeys mismatch: " + JSON.stringify(afterSortKeys));
        }
        var values = Array.from(testParams.values());
        if (values.length !== 2 || values[0] !== "1" || values[1] !== "2") {
            throw new Error("values mismatch: " + JSON.stringify(values));
        }
        var entries = Array.from(testParams.entries());
        if (entries.length !== 2 || entries[0][0] !== "a" || entries[0][1] !== "1" || entries[1][0] !== "b" || entries[1][1] !== "2") {
            throw new Error("entries mismatch: " + JSON.stringify(entries));
        }
        var spreadEntries = Array.from(testParams);
        if (spreadEntries.length !== 2 || spreadEntries[0][0] !== "a" || spreadEntries[1][0] !== "b") {
            throw new Error("spreadEntries mismatch: " + JSON.stringify(spreadEntries));
        }
        if (testParams.size !== 2) throw new Error("testParams.size mismatch: " + testParams.size);

        // Sequence and record constructor init
        var fromSeq = new URLSearchParams([["k1", "v1"], ["k2", "v2"]]);
        if (fromSeq.get("k1") !== "v1" || fromSeq.get("k2") !== "v2") throw new Error("fromSeq failed");
        var fromRecord = new URLSearchParams({ p1: "100", p2: "200" });
        if (fromRecord.get("p1") !== "100" || fromRecord.get("p2") !== "200") throw new Error("fromRecord failed");
        var fromCopy = new URLSearchParams(testParams);
        if (fromCopy.get("a") !== "1" || fromCopy.get("b") !== "2") throw new Error("fromCopy failed");

        // FormData iterators and toUrlSearchParams
        var fd = new FormData();
        fd.append("x", "10");
        fd.append("y", "20");
        var fdKeys = Array.from(fd.keys());
        if (fdKeys.length !== 2 || fdKeys[0] !== "x" || fdKeys[1] !== "y") throw new Error("fdKeys failed");
        var fdFromUsp = new URLSearchParams(fd);
        if (fdFromUsp.get("x") !== "10" || fdFromUsp.get("y") !== "20") throw new Error("fdFromUsp failed");

        // URL constructor and properties
        var url = new URL("https://example.com:8080/path/test?q=search&lang=en#heading");
        if (url.protocol !== "https:") throw new Error("url.protocol mismatch: " + url.protocol);
        if (url.hostname !== "example.com") throw new Error("url.hostname mismatch: " + url.hostname);
        if (url.port !== "8080") throw new Error("url.port mismatch: " + url.port);
        if (url.host !== "example.com:8080") throw new Error("url.host mismatch: " + url.host);
        if (url.pathname !== "/path/test") throw new Error("url.pathname mismatch: " + url.pathname);
        if (url.search !== "?q=search&lang=en") throw new Error("url.search mismatch: " + url.search);
        if (url.hash !== "#heading") throw new Error("url.hash mismatch: " + url.hash);
        if (url.origin !== "https://example.com:8080") throw new Error("url.origin mismatch: " + url.origin);
        if (url.searchParams.get("q") !== "search") throw new Error("url.searchParams mismatch");

        // URL relative resolution
        var relUrl = new URL("../sub/page", "https://example.com/path/test");
        if (relUrl.pathname !== "/sub/page") throw new Error("URL relative resolution failed: " + relUrl.pathname);

        // URL static methods
        if (!URL.canParse("https://example.com")) throw new Error("URL.canParse should be true for valid URL");
        if (URL.canParse("not a url")) throw new Error("URL.canParse should be false for invalid URL");
        var blobObj = new Blob(["test"], { type: "text/plain" });
        var blobUrl = URL.createObjectURL(blobObj);
        if (blobUrl.indexOf("blob:") !== 0) throw new Error("createObjectURL failed: " + blobUrl);
        URL.revokeObjectURL(blobUrl);

        // 2. FormData
        var fd = new FormData();
        fd.append("name", "mango");
        fd.append("item", "1");
        fd.append("item", "2");
        if (fd.get("name") !== "mango") throw new Error("fd.get failed");
        var items = fd.getAll("item");
        if (items.length !== 2 || items[0] !== "1" || items[1] !== "2") throw new Error("fd.getAll failed");
        if (!fd.has("name") || fd.has("missing")) throw new Error("fd.has failed");
        fd.set("item", "single");
        if (fd.getAll("item").length !== 1 || fd.get("item") !== "single") throw new Error("fd.set failed");
        fd.delete("name");
        if (fd.has("name")) throw new Error("fd.delete failed");

        // 3. Blob, File, FileReader
        var blob = new Blob(["Hello", " ", "World"], { type: "text/plain" });
        if (blob.size !== 11) throw new Error("blob.size mismatch: " + blob.size);
        if (blob.type !== "text/plain") throw new Error("blob.type mismatch: " + blob.type);
        var sliced = blob.slice(0, 5);
        if (sliced.size !== 5) throw new Error("blob.slice size mismatch: " + sliced.size);

        var file = new File(["test file content"], "sample.txt", { type: "text/plain", lastModified: 1700000000000 });
        if (file.name !== "sample.txt") throw new Error("file.name mismatch: " + file.name);
        if (file.size !== 17) throw new Error("file.size mismatch: " + file.size);
        if (file.lastModified !== 1700000000000) throw new Error("file.lastModified mismatch: " + file.lastModified);

        window.readerResult = null;
        var reader = new FileReader();
        reader.onload = function(e) {
            window.readerResult = e.target.result;
        };
        reader.readAsText(blob);

        // 4. AbortController & AbortSignal
        var ac = new AbortController();
        if (ac.signal.aborted) throw new Error("signal should initially not be aborted");
        window.abortEventFired = false;
        ac.signal.addEventListener("abort", function() {
            window.abortEventFired = true;
        });
        ac.abort("CustomCancel");
        if (!ac.signal.aborted) throw new Error("signal should be aborted after ac.abort()");
        if (ac.signal.reason !== "CustomCancel") throw new Error("signal.reason mismatch: " + ac.signal.reason);
        if (!window.abortEventFired) throw new Error("abort event did not fire");

        var instantAbort = AbortSignal.abort("instant");
        if (!instantAbort.aborted || instantAbort.reason !== "instant") throw new Error("AbortSignal.abort failed");

        // 5. TextEncoder & TextDecoder
        var encoder = new TextEncoder();
        if (encoder.encoding !== "utf-8") throw new Error("encoder.encoding should be utf-8");
        var encoded = encoder.encode("Hello \u2764");
        if (!(encoded instanceof Uint8Array)) throw new Error("encoded should be Uint8Array");

        var dest = new Uint8Array(10);
        var res = encoder.encodeInto("Hi", dest);
        if (res.read !== 2 || res.written !== 2 || dest[0] !== 72 || dest[1] !== 105) {
            throw new Error("encodeInto failed: " + JSON.stringify(res));
        }

        var decoder = new TextDecoder();
        if (decoder.encoding !== "utf-8") throw new Error("decoder.encoding should be utf-8");
        var decoded = decoder.decode(encoded);
        if (decoded !== "Hello \u2764") throw new Error("decoder.decode failed: " + decoded);

        // 6. crypto.getRandomValues, crypto.randomUUID, crypto.subtle
        if (!crypto || !crypto.subtle) throw new Error("crypto or crypto.subtle missing");
        var randArr = new Uint8Array(16);
        crypto.getRandomValues(randArr);
        var anyNonZero = false;
        for (var i = 0; i < randArr.length; i++) {
            if (randArr[i] !== 0) anyNonZero = true;
        }
        if (!anyNonZero) throw new Error("getRandomValues returned all zeros");

        var uuid = crypto.randomUUID();
        if (typeof uuid !== "string" || uuid.length !== 36 || uuid[14] !== '4') {
            throw new Error("randomUUID format invalid: " + uuid);
        }

        // 7. btoa & atob
        var b64 = btoa("Hello World!");
        if (b64 !== "SGVsbG8gV29ybGQh") throw new Error("btoa failed: " + b64);
        var origStr = atob(b64);
        if (origStr !== "Hello World!") throw new Error("atob failed: " + origStr);

        // 8. structuredClone
        var original = {
            num: 42,
            str: "mango",
            flag: true,
            date: new Date(1700000000000),
            regex: /test/gi,
            arr: [1, 2, { nested: "yes" }],
            u8: new Uint8Array([10, 20, 30])
        };
        // Add cycle
        original.self = original;
        var cloned = structuredClone(original);
        if (cloned.num !== 42 || cloned.str !== "mango" || cloned.flag !== true) throw new Error("cloned primitives mismatch");
        if (cloned === original) throw new Error("cloned object should not be identical reference");
        if (cloned.date.getTime() !== 1700000000000) throw new Error("cloned Date mismatch");
        if (cloned.regex.source !== "test" || !cloned.regex.global || !cloned.regex.ignoreCase) throw new Error("cloned RegExp mismatch");
        if (cloned.arr[2].nested !== "yes") throw new Error("cloned nested array mismatch");
        if (cloned.self !== cloned) throw new Error("cloned circular reference failed");
        if (cloned.u8[1] !== 20 || cloned.u8 === original.u8) throw new Error("cloned TypedArray mismatch");

        // 9. fetch, Headers, Request, Response
        var headers = new Headers({ "Content-Type": "text/html", "X-Custom": "val" });
        if (headers.get("content-type") !== "text/html") throw new Error("headers.get failed");
        headers.append("Accept", "text/plain");
        if (!headers.has("accept")) throw new Error("headers.has failed");
        var headerKeys = Array.from(headers.keys());
        if (headerKeys.indexOf("x-custom") === -1) throw new Error("headers.keys failed");

        var req = new Request("https://example.com/api", {
            method: "POST",
            headers: { "X-Api-Key": "secret" },
            body: "payload"
        });
        if (req.method !== "POST") throw new Error("req.method mismatch: " + req.method);
        if (req.url !== "https://example.com/api") throw new Error("req.url mismatch: " + req.url);
        if (req.headers.get("x-api-key") !== "secret") throw new Error("req.headers mismatch");
        if (req.mode !== "cors") throw new Error("req.mode default mismatch: " + req.mode);
        if (req.credentials !== "same-origin") throw new Error("req.credentials default mismatch: " + req.credentials);

        // SOP test via _mangoFetch: cross-origin in same-origin mode must be blocked
        var sopRes = JSON.parse(_mangoFetch("https://api.other.com/data", "GET", "{}", "", "https://example.com", "same-origin", false));
        if (!sopRes.error || sopRes.error.indexOf("Same-Origin Policy") === -1) {
            throw new Error("SOP check failed to block cross-origin request: " + JSON.stringify(sopRes));
        }

        var res = new Response("Test body", {
            status: 200,
            statusText: "OK",
            headers: { "X-Powered-By": "Mango" }
        });
        if (res.status !== 200 || !res.ok) throw new Error("res.status mismatch");
        if (res.statusText !== "OK") throw new Error("res.statusText mismatch");
        if (res.headers.get("x-powered-by") !== "Mango") throw new Error("res.headers mismatch");

        var jsonRes = Response.json({ success: true });
        if (jsonRes.status !== 200) throw new Error("Response.json status mismatch");
        if (jsonRes.headers.get("content-type") !== "application/json") throw new Error("Response.json content-type mismatch");

        // 10. XMLHttpRequest & XMLHttpRequestUpload
        var xhr = new XMLHttpRequest();
        if (!(xhr instanceof XMLHttpRequest)) throw new Error("xhr not XMLHttpRequest");
        if (!xhr.upload) throw new Error("xhr.upload missing");
        if (typeof xhr.upload.addEventListener !== "function") throw new Error("xhr.upload.addEventListener missing");
        xhr.open("GET", "/test-endpoint");
        xhr.setRequestHeader("X-Requested-With", "XMLHttpRequest");
        xhr.send();

        // 11. WebSocket
        var ws = new WebSocket("wss://echo.example.com");
        if (!(ws instanceof WebSocket)) throw new Error("ws not WebSocket");
        if (ws.readyState !== 0 /* CONNECTING */ && ws.readyState !== 3 /* CLOSED */) throw new Error("ws.readyState invalid");
        if (typeof ws.addEventListener !== "function") throw new Error("ws.addEventListener missing");
        if (typeof ws.send !== "function") throw new Error("ws.send missing");
        if (typeof ws.close !== "function") throw new Error("ws.close missing");

        // 12. Cache API & Resource Timing
        if (typeof caches === "undefined" || typeof CacheStorage === "undefined") throw new Error("caches undefined");
        caches.open("v1").then(function(c) {
            var cacheReq = new Request("https://example.com/cached-page");
            var cacheRes = new Response("Cached content", { status: 200 });
            return c.put(cacheReq, cacheRes).then(function() {
                return c.match("https://example.com/cached-page");
            }).then(function(matched) {
                if (!matched) throw new Error("cache.match failed to find entry");
                return matched.text();
            }).then(function(txt) {
                if (txt !== "Cached content") throw new Error("cache text mismatch: " + txt);
                window.__cacheApiTested = true;
            });
        });

        performance.mark("task-start");
        performance.mark("task-end");
        var measure = performance.measure("task-duration", "task-start", "task-end");
        if (measure.entryType !== "measure") throw new Error("measure entryType mismatch");
        if (performance.getEntriesByType("mark").length < 2) throw new Error("performance.getEntriesByType marks missing");
    "##;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_web_apis failed: {}", e);
    }

    let verify_reader = r#"
        if (window.readerResult !== "Hello World") throw new Error("FileReader.readAsText failed: " + window.readerResult);
        if (!window.__cacheApiTested) throw new Error("Cache API async operations failed or did not resolve");
    "#;
    if let Err(e) = rt.execute_script(verify_reader) {
        panic!("test_web_apis verify_reader failed: {}", e);
    }
}

#[test]
fn test_web_idl_hierarchy() {
    let doc = parse_html("<html><body><div id='d'></div><p id='p'></p></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. Core prototypes chain
        if (!(new EventTarget() instanceof Object)) throw new Error("EventTarget not Object");
        if (!(new Node() instanceof EventTarget)) throw new Error("Node not EventTarget");
        if (!(new Element() instanceof Node)) throw new Error("Element not Node");
        if (!(new HTMLElement() instanceof Element)) throw new Error("HTMLElement not Element");

        // 2. Concrete element prototypes
        var div = document.createElement("div");
        if (!(div instanceof HTMLDivElement)) throw new Error("div not HTMLDivElement");
        if (!(div instanceof HTMLElement)) throw new Error("div not HTMLElement");
        if (!(div instanceof Element)) throw new Error("div not Element");
        if (!(div instanceof Node)) throw new Error("div not Node");
        if (!(div instanceof EventTarget)) throw new Error("div not EventTarget");

        var p = document.createElement("p");
        if (!(p instanceof HTMLParagraphElement)) throw new Error("p not HTMLParagraphElement");
        if (!(p instanceof HTMLElement)) throw new Error("p not HTMLElement");

        var h1 = document.createElement("h1");
        if (!(h1 instanceof HTMLHeadingElement)) throw new Error("h1 not HTMLHeadingElement");

        var ul = document.createElement("ul");
        if (!(ul instanceof HTMLUListElement)) throw new Error("ul not HTMLUListElement");

        var li = document.createElement("li");
        if (!(li instanceof HTMLLIElement)) throw new Error("li not HTMLLIElement");

        var table = document.createElement("table");
        if (!(table instanceof HTMLTableElement)) throw new Error("table not HTMLTableElement");

        var tr = document.createElement("tr");
        if (!(tr instanceof HTMLTableRowElement)) throw new Error("tr not HTMLTableRowElement");

        var td = document.createElement("td");
        if (!(td instanceof HTMLTableCellElement)) throw new Error("td not HTMLTableCellElement");

        var input = document.createElement("input");
        if (!(input instanceof HTMLInputElement)) throw new Error("input not HTMLInputElement");

        var canvas = document.createElement("canvas");
        if (!(canvas instanceof HTMLCanvasElement)) throw new Error("canvas not HTMLCanvasElement");

        var video = document.createElement("video");
        if (!(video instanceof HTMLVideoElement)) throw new Error("video not HTMLVideoElement");
        if (!(video instanceof HTMLMediaElement)) throw new Error("video not HTMLMediaElement");

        var audio = document.createElement("audio");
        if (!(audio instanceof HTMLAudioElement)) throw new Error("audio not HTMLAudioElement");
        if (!(audio instanceof HTMLMediaElement)) throw new Error("audio not HTMLMediaElement");

        if (!(document.body instanceof HTMLBodyElement)) throw new Error("document.body not HTMLBodyElement");
        if (!(document.head instanceof HTMLHeadElement)) throw new Error("document.head not HTMLHeadElement");
        if (!(document.documentElement instanceof HTMLHtmlElement)) throw new Error("documentElement not HTMLHtmlElement");

        // 3. Event hierarchy
        var msgEvt = new MessageEvent("message", { data: "test" });
        if (!(msgEvt instanceof MessageEvent)) throw new Error("MessageEvent not MessageEvent");
        if (!(msgEvt instanceof Event)) throw new Error("MessageEvent not Event");
        if (msgEvt.data !== "test") throw new Error("MessageEvent.data mismatch");

        var closeEvt = new CloseEvent("close", { code: 1000, reason: "ok", wasClean: true });
        if (!(closeEvt instanceof CloseEvent)) throw new Error("CloseEvent not CloseEvent");
        if (!(closeEvt instanceof Event)) throw new Error("CloseEvent not Event");
        if (closeEvt.code !== 1000 || closeEvt.wasClean !== true) throw new Error("CloseEvent props mismatch");
    "#;

    rt.execute_script(script)
        .expect("Web IDL hierarchy test failed");
}

#[test]
fn test_threaded_web_workers() {
    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        globalThis.workerResult = null;
        var blobCode = "self.onmessage = function(e) { self.postMessage({ doubled: e.data.num * 2 }); };";
        var blob = new Blob([blobCode], { type: 'text/javascript' });
        var blobUrl = URL.createObjectURL(blob);

        var worker = new Worker(blobUrl);
        worker.onmessage = function(e) {
            globalThis.workerResult = e.data.doubled;
        };
        worker.postMessage({ num: 21 });
    "##;

    rt.execute_script(script).expect("worker script failed");

    // Wait up to 2 seconds for worker thread execution and reply
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(2000) {
        rt.tick();
        let check = rt.eval("globalThis.workerResult").unwrap_or_default();
        if check == "42" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let res = rt
        .eval("globalThis.workerResult")
        .expect("eval workerResult");
    assert_eq!(
        res, "42",
        "Threaded Web Worker should calculate 21 * 2 = 42 in background"
    );
}

#[test]
fn test_websocket_live() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let port = listener.local_addr().expect("local addr").port();

    // Spawn mock WebSocket server thread
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            if let Ok(n) = stream.read(&mut buf) {
                let req = String::from_utf8_lossy(&buf[..n]);
                let mut key = "";
                for line in req.lines() {
                    if line.to_ascii_lowercase().starts_with("sec-websocket-key:") {
                        key = line.split(':').nth(1).unwrap_or("").trim();
                    }
                }
                let accept = mango_net::websocket::compute_accept_key(key);
                let resp = format!(
                    "HTTP/1.1 101 Switching Protocols\r\n\
                     Upgrade: websocket\r\n\
                     Connection: Upgrade\r\n\
                     Sec-WebSocket-Accept: {}\r\n\r\n",
                    accept
                );
                let _ = stream.write_all(resp.as_bytes());

                // Read one client frame and echo it back unmasked (server frames are not masked)
                if let Ok((opcode, payload, _fin)) = mango_net::websocket::decode_frame(&mut stream)
                {
                    if opcode == mango_net::websocket::Opcode::Text {
                        let mut server_frame = vec![0x81, payload.len() as u8];
                        server_frame.extend_from_slice(&payload);
                        let _ = stream.write_all(&server_frame);
                        let _ = stream.flush();
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                    }
                }
            }
        }
    });

    let doc = parse_html("<html><body></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let ws_url = format!("ws://127.0.0.1:{}/ws", port);
    let script = format!(
        r##"
        var ws = new WebSocket("{}");
        globalThis.openFired = false;
        globalThis.receivedMessage = null;

        ws.onopen = function(e) {{
            globalThis.openFired = true;
            ws.send("Ping from Mango");
        }};
        ws.onmessage = function(e) {{
            globalThis.receivedMessage = e.data;
        }};
    "##,
        ws_url
    );

    rt.execute_script(&script).expect("ws script failed");

    // Wait up to 8 seconds for background thread handshake & echo (avoids CI load flakes on Windows)
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_millis(8000) {
        rt.tick();
        let check = rt.eval("globalThis.receivedMessage").unwrap_or_default();
        if check.contains("Ping from Mango") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    let open_res = rt.eval("globalThis.openFired").expect("eval openFired");
    assert_eq!(open_res, "true", "WebSocket onopen should have fired");

    let msg_res = rt
        .eval("globalThis.receivedMessage")
        .expect("eval receivedMessage");
    assert_eq!(
        msg_res, "Ping from Mango",
        "WebSocket echoed message mismatch"
    );
}

#[test]
fn test_window_open_returns_window_proxy() {
    let doc = Document::new();
    let mut rt = JsRuntime::new(doc, 1024.0, 768.0);
    let script = r#"
        (function() {
            if (typeof window.open !== 'function') throw new Error("window.open is not a function");
            if (typeof open !== 'function') throw new Error("open is not a function");

            var popup = window.open("https://example.com/login", "_blank", "width=500,height=600");
            if (!popup) throw new Error("window.open returned null");
            if (popup.closed !== false) throw new Error("popup.closed must be false initially");
            if (typeof popup.focus !== 'function') throw new Error("popup.focus is not a function");
            if (typeof popup.close !== 'function') throw new Error("popup.close is not a function");
            if (typeof popup.postMessage !== 'function') throw new Error("popup.postMessage is not a function");
            if (popup.opener !== window) throw new Error("popup.opener must be window");

            globalThis.receivedMessage = null;
            popup.addEventListener("message", function(e) {
                globalThis.receivedMessage = e.data;
            });
            popup.postMessage("hello from opener", "*");
            popup.focus();

            globalThis.popupRef = popup;
            return "ok";
        })()
    "#;
    let res = rt.eval(script).expect("eval window.open test");
    assert_eq!(res, "ok");

    // Verify pending open window was captured
    let pending = rt.take_pending_open_window();
    assert!(pending.is_some());
    let open_req = pending.unwrap();
    assert_eq!(open_req.url, "https://example.com/login");
    assert_eq!(open_req.target, "_blank");
    assert_eq!(open_req.features, "width=500,height=600");

    // Tick event loop to deliver the microtask postMessage
    rt.tick();
    let msg = rt
        .eval("globalThis.receivedMessage")
        .expect("eval receivedMessage");
    assert_eq!(msg, "hello from opener");

    // Now close the popup and verify closed is true
    let close_res = rt
        .eval(
            r#"
        (function() {
            var p = globalThis.popupRef;
            p.close();
            return p.closed ? "closed" : "open";
        })()
    "#,
        )
        .expect("close popup");
    assert_eq!(close_res, "closed");
}
