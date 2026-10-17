// WPT Suite 5: Storage & Offline PWA Primitives (IndexedDB 3.0, Cache Storage, Service Worker, Web Storage)

test(function() {
    assert_true(typeof localStorage !== "undefined", "localStorage should be defined");
    localStorage.clear();
    assert_equals(localStorage.length, 0, "localStorage length after clear");

    localStorage.setItem("theme", "dark");
    localStorage.setItem("user", "alice");
    assert_equals(localStorage.length, 2, "localStorage length after 2 setItem calls");
    assert_equals(localStorage.getItem("theme"), "dark", "getItem theme");
    assert_equals(localStorage.getItem("user"), "alice", "getItem user");

    localStorage.removeItem("theme");
    assert_equals(localStorage.length, 1, "localStorage length after removeItem");
    assert_equals(localStorage.getItem("theme"), null, "getItem after remove should be null");
    localStorage.clear();
}, "WPT Storage: Web Storage API localStorage setItem, getItem, removeItem, clear");

promise_test(async function() {
    assert_true(typeof caches !== "undefined", "caches global should exist");
    assert_true(typeof caches.open === "function", "caches.open should be a function");

    const cache = await caches.open("wpt-cache-v1");
    assert_true(cache !== null, "caches.open should resolve to Cache instance");

    const req = new Request("https://example.com/api/data");
    const resp = new Response(JSON.stringify({ status: "ok" }), {
        headers: { "content-type": "application/json" }
    });

    await cache.put(req, resp);

    const matched = await cache.match("https://example.com/api/data");
    assert_true(matched !== null, "cache.match should find stored response");
    const json = await matched.json();
    assert_equals(json.status, "ok", "cached response body content");

    const deleted = await caches.delete("wpt-cache-v1");
    assert_true(deleted, "caches.delete should return true");
}, "WPT Storage: Cache Storage API open, put, match, and delete");

promise_test(async function() {
    assert_true(typeof indexedDB !== "undefined", "indexedDB should be defined");
    assert_true(typeof IDBKeyRange !== "undefined", "IDBKeyRange should be defined");

    // Test W3C IDBFactory.cmp algorithm
    assert_equals(indexedDB.cmp(10, 20), -1, "cmp(10, 20) should be -1");
    assert_equals(indexedDB.cmp("abc", "abc"), 0, "cmp('abc', 'abc') should be 0");
    assert_equals(indexedDB.cmp([1, 2], [1, 1]), 1, "cmp([1, 2], [1, 1]) should be 1");

    // IDBKeyRange bounds
    const rOnly = IDBKeyRange.only(50);
    assert_true(rOnly.includes(50), "only(50) includes 50");
    assert_false(rOnly.includes(51), "only(50) does not include 51");

    const rBound = IDBKeyRange.bound(10, 20, false, false);
    assert_true(rBound.includes(15), "bound(10, 20) includes 15");
    assert_false(rBound.includes(25), "bound(10, 20) does not include 25");
}, "WPT Storage: IndexedDB key comparison and IDBKeyRange bounds");

promise_test(async function() {
    assert_true(typeof navigator.serviceWorker !== "undefined", "navigator.serviceWorker should be defined");
    const reg = await navigator.serviceWorker.register("/sw-wpt.js", { scope: "/wpt/" });
    assert_equals(reg.scope, "/wpt/", "Service worker registration scope");
    assert_true(reg.active !== null, "Active worker present");
    assert_equals(reg.active.state, "activated", "Worker state should be activated");
}, "WPT Offline: ServiceWorker registration and lifecycle scope");
