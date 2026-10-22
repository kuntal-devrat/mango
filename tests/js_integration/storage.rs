use mango_html::parse_html;
use mango_js::JsRuntime;

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
    assert_eq!(
        map.get("https://site-a.com\x1fuser").map(|s| s.as_str()),
        Some("Alice")
    );
    assert_eq!(
        map.get("https://site-b.com\x1fuser").map(|s| s.as_str()),
        Some("Bob")
    );
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
    assert!(
        logs.iter()
            .any(|m| m.text == "event set key: theme new: dark")
    );
    assert!(
        logs.iter()
            .any(|m| m.text == "after delete prop: undefined")
    );
    assert!(logs.iter().any(|m| m.text == "after delete item: null"));
    assert!(logs.iter().any(|m| m.text == "event on del count: 2"));
    assert!(
        logs.iter()
            .any(|m| m.text == "event del key: theme old: dark")
    );
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
        assert!(
            logs.iter()
                .any(|m| m.text == "retrieved title: Mango Browser")
        );
        assert!(logs.iter().any(|m| m.text == "retrieved fast: true"));
    }

    // Verify storage map directly contains the serialized IndexedDB data
    {
        let map = shared_storage.lock().unwrap();
        let idb_key = "https://myapp.com\x1f__idb__AppDB";
        assert!(
            map.contains_key(idb_key),
            "IndexedDB was serialized into backing storage"
        );
        let payload = map.get(idb_key).unwrap();
        assert!(
            payload.contains("Mango Browser"),
            "Record found in IDB payload: {payload}"
        );
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
        assert!(
            logs2
                .iter()
                .any(|m| m.text == "reloaded title: Mango Browser")
        );
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
        assert!(
            logs3
                .iter()
                .any(|m| m.text.contains("otherapp needed upgrade"))
        );
        assert!(
            logs3
                .iter()
                .any(|m| m.text == "otherapp objectStoreNames length: 0")
        );
    }
}
