use super::*;

fn create_test_runtime() -> JsRuntime {
    JsRuntime::new(Document::new(), 800.0, 600.0)
}

#[test]
fn test_whatwg_headers_conformance() {
    let mut rt = create_test_runtime();
    let script = r#"
        (function() {
            // 1. Valid header name and value parsing
            var h = new Headers({
                'Content-Type': 'text/html; charset=utf-8',
                'X-Custom-Header': 'custom-val'
            });
            if (h.get('content-type') !== 'text/html; charset=utf-8') {
                throw new Error('Case-insensitive get failed for content-type: ' + h.get('content-type'));
            }
            if (h.get('Content-Type') !== 'text/html; charset=utf-8') {
                throw new Error('Case-insensitive get failed with mixed case');
            }
            if (!h.has('x-custom-header') || !h.has('X-CUSTOM-HEADER')) {
                throw new Error('Case-insensitive has failed');
            }

            // 2. Invalid header name validation
            var invalidNames = ['', 'Invalid Name', 'Content@Type', 'H:eader'];
            for (var i = 0; i < invalidNames.length; i++) {
                var threw = false;
                try {
                    h.set(invalidNames[i], 'val');
                } catch(e) {
                    if (e instanceof TypeError) threw = true;
                }
                if (!threw) throw new Error('Expected TypeError for invalid header name: ' + invalidNames[i]);
            }

            // 3. Invalid header value validation (control characters \0, \r, \n)
            var invalidValues = ['val\0ue', 'val\nue', 'val\rue'];
            for (var i = 0; i < invalidValues.length; i++) {
                var threw = false;
                try {
                    h.set('X-Valid', invalidValues[i]);
                } catch(e) {
                    if (e instanceof TypeError) threw = true;
                }
                if (!threw) throw new Error('Expected TypeError for invalid header value: ' + invalidValues[i]);
            }

            // 4. append() combines values with comma-space
            h.append('Accept', 'text/html');
            h.append('Accept', 'application/xhtml+xml');
            if (h.get('accept') !== 'text/html, application/xhtml+xml') {
                throw new Error('append() failed to combine values: ' + h.get('accept'));
            }
            var allAccept = h.getAll('accept');
            if (allAccept.length !== 2 || allAccept[0] !== 'text/html' || allAccept[1] !== 'application/xhtml+xml') {
                throw new Error('getAll() mismatch: ' + JSON.stringify(allAccept));
            }

            // 5. Set-Cookie preservation and getSetCookie()
            h.append('Set-Cookie', 'id=1; Expires=Wed, 21 Oct 2026 07:28:00 GMT; Path=/');
            h.append('Set-Cookie', 'user=alice; Path=/');
            var sc = h.getSetCookie();
            if (sc.length !== 2) throw new Error('getSetCookie() length mismatch: ' + sc.length);
            if (sc[0] !== 'id=1; Expires=Wed, 21 Oct 2026 07:28:00 GMT; Path=/') {
                throw new Error('getSetCookie() item 0 mismatch: ' + sc[0]);
            }
            if (sc[1] !== 'user=alice; Path=/') {
                throw new Error('getSetCookie() item 1 mismatch: ' + sc[1]);
            }

            // 6. Delete
            h.delete('x-custom-header');
            if (h.has('x-custom-header') || h.get('x-custom-header') !== null) {
                throw new Error('delete() failed');
            }

            // 7. Sorted iterator entries
            var h2 = new Headers();
            h2.set('z-header', 'z');
            h2.set('a-header', 'a');
            h2.set('m-header', 'm');
            var keys = [];
            for (var k of h2.keys()) {
                keys.push(k);
            }
            if (keys.join(',') !== 'a-header,m-header,z-header') {
                throw new Error('Headers keys not sorted: ' + keys.join(','));
            }

            // 8. Symbol.toStringTag
            if (Object.prototype.toString.call(h) !== '[object Headers]') {
                throw new Error('Headers toStringTag mismatch: ' + Object.prototype.toString.call(h));
            }
        })();
    "#;
    rt.execute_script(script)
        .expect("Headers conformance test failed");
}

#[test]
fn test_whatwg_request_conformance() {
    let mut rt = create_test_runtime();
    let script = r#"
        (async function() {
            // 1. Construction and default values
            var req = new Request('https://example.com/api/test', {
                method: 'post',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ key: 'value' })
            });
            if (req.method !== 'POST') throw new Error('Method not uppercased: ' + req.method);
            if (req.url !== 'https://example.com/api/test') throw new Error('URL mismatch: ' + req.url);
            if (req.mode !== 'cors') throw new Error('Default mode mismatch: ' + req.mode);
            if (req.credentials !== 'same-origin') throw new Error('Default credentials mismatch: ' + req.credentials);
            if (req.bodyUsed) throw new Error('New request should not have bodyUsed true');

            // 2. Forbidden methods
            var forbidden = ['CONNECT', 'TRACE', 'TRACK', 'connect', 'trace', 'track'];
            for (var i = 0; i < forbidden.length; i++) {
                var threw = false;
                try {
                    new Request('https://example.com', { method: forbidden[i] });
                } catch(e) {
                    if (e instanceof TypeError) threw = true;
                }
                if (!threw) throw new Error('Expected TypeError for forbidden method: ' + forbidden[i]);
            }

            // 3. GET/HEAD cannot have body
            var threwGetBody = false;
            try {
                new Request('https://example.com', { method: 'GET', body: 'illegal' });
            } catch(e) {
                if (e instanceof TypeError) threwGetBody = true;
            }
            if (!threwGetBody) throw new Error('Expected TypeError for GET request with body');

            // 4. Body mixin: text(), json(), arrayBuffer(), bytes()
            var bodyText = await req.text();
            if (bodyText !== '{"key":"value"}') throw new Error('req.text() mismatch: ' + bodyText);
            if (!req.bodyUsed) throw new Error('bodyUsed should be true after reading body');

            // 5. Subsequent read rejects with TypeError
            var threwSecondRead = false;
            try {
                await req.json();
            } catch(e) {
                if (e instanceof TypeError) threwSecondRead = true;
            }
            if (!threwSecondRead) throw new Error('Second read on consumed body should throw TypeError');

            // 6. Cloning before consumption
            var reqOriginal = new Request('https://example.com/stream', {
                method: 'PUT',
                body: 'payload-to-clone'
            });
            var reqClone = reqOriginal.clone();
            var t1 = await reqOriginal.text();
            var t2 = await reqClone.text();
            if (t1 !== 'payload-to-clone' || t2 !== 'payload-to-clone') {
                throw new Error('Cloned request body text mismatch: ' + t1 + ' vs ' + t2);
            }

            // 7. Cloning after consumption must throw TypeError
            var threwCloneDisturbed = false;
            try {
                reqOriginal.clone();
            } catch(e) {
                if (e instanceof TypeError) threwCloneDisturbed = true;
            }
            if (!threwCloneDisturbed) throw new Error('Cloning consumed request should throw TypeError');

            // 8. bytes() returns Uint8Array
            var reqBytes = new Request('https://example.com/bytes', { method: 'POST', body: 'hello' });
            var b = await reqBytes.bytes();
            if (!(b instanceof Uint8Array)) throw new Error('bytes() did not return Uint8Array');
            if (new TextDecoder().decode(b) !== 'hello') throw new Error('bytes() content mismatch');
        })();
    "#;
    rt.execute_script(script)
        .expect("Request conformance test failed");
}

#[test]
fn test_whatwg_response_conformance() {
    let mut rt = create_test_runtime();
    let script = r#"
        (async function() {
            // 1. Status range validation
            var invalidStatuses = [199, 600, -1, 999];
            for (var i = 0; i < invalidStatuses.length; i++) {
                var threw = false;
                try {
                    new Response('ok', { status: invalidStatuses[i] });
                } catch(e) {
                    if (e instanceof RangeError) threw = true;
                }
                if (!threw) throw new Error('Expected RangeError for invalid status: ' + invalidStatuses[i]);
            }

            // 2. Null body status (204, 205, 304) with body
            var nullStatuses = [204, 205, 304];
            for (var i = 0; i < nullStatuses.length; i++) {
                var threw = false;
                try {
                    new Response('non-empty-body', { status: nullStatuses[i] });
                } catch(e) {
                    if (e instanceof TypeError) threw = true;
                }
                if (!threw) throw new Error('Expected TypeError for null body status: ' + nullStatuses[i]);
            }

            // 3. Static constructors
            var errResp = Response.error();
            if (errResp.type !== 'error' || errResp.status !== 0 || errResp.ok) {
                throw new Error('Response.error() mismatch');
            }

            var redir = Response.redirect('https://example.com/new-location', 301);
            if (redir.status !== 301 || redir.headers.get('location') !== 'https://example.com/new-location') {
                throw new Error('Response.redirect() mismatch');
            }

            var jsonResp = Response.json({ success: true, count: 42 });
            if (jsonResp.status !== 200 || !jsonResp.ok) throw new Error('Response.json() status mismatch');
            if (jsonResp.headers.get('content-type') !== 'application/json') {
                throw new Error('Response.json() content-type mismatch: ' + jsonResp.headers.get('content-type'));
            }
            var parsed = await jsonResp.json();
            if (!parsed.success || parsed.count !== 42) throw new Error('Response.json() body parsing mismatch');

            // 4. clone() and body consumption
            var orig = new Response('original-body');
            var cloned = orig.clone();
            var oTxt = await orig.text();
            var cTxt = await cloned.text();
            if (oTxt !== 'original-body' || cTxt !== 'original-body') {
                throw new Error('Response clone() text mismatch');
            }

            // 5. Disturbed clone throws TypeError
            var threwDisturbed = false;
            try {
                orig.clone();
            } catch(e) {
                if (e instanceof TypeError) threwDisturbed = true;
            }
            if (!threwDisturbed) throw new Error('Cloning disturbed Response should throw TypeError');

            // 6. bytes() method
            var binResp = new Response(new Uint8Array([1, 2, 3, 4, 5]));
            var bytes = await binResp.bytes();
            if (!(bytes instanceof Uint8Array) || bytes.length !== 5 || bytes[2] !== 3) {
                throw new Error('Response.bytes() mismatch');
            }
        })();
    "#;
    rt.execute_script(script)
        .expect("Response conformance test failed");
}

#[test]
fn test_whatwg_streams_conformance() {
    let mut rt = create_test_runtime();
    let script = r#"
        (async function() {
            // 1. ReadableStream.from() sync iterable
            var numbers = [10, 20, 30];
            var streamFromSync = ReadableStream.from(numbers);
            var reader1 = streamFromSync.getReader();
            var res1 = await reader1.read();
            var res2 = await reader1.read();
            var res3 = await reader1.read();
            var res4 = await reader1.read();
            if (res1.value !== 10 || res2.value !== 20 || res3.value !== 30 || !res4.done) {
                throw new Error('ReadableStream.from(sync) read sequence failed');
            }

            // 2. ReadableStream.from() async generator
            async function* asyncGen() {
                yield 'chunk-a';
                yield 'chunk-b';
            }
            var streamFromAsync = ReadableStream.from(asyncGen());
            var chunks = [];
            for await (var chunk of streamFromAsync) {
                chunks.push(chunk);
            }
            if (chunks.join(',') !== 'chunk-a,chunk-b') {
                throw new Error('for await on ReadableStream.from(asyncGen) failed: ' + chunks.join(','));
            }

            // 3. ReadableStream tee()
            var src = new ReadableStream({
                start: function(c) {
                    c.enqueue('item1');
                    c.enqueue('item2');
                    c.close();
                }
            });
            var [branchA, branchB] = src.tee();
            var rA = branchA.getReader();
            var rB = branchB.getReader();
            var valA1 = await rA.read();
            var valB1 = await rB.read();
            var valA2 = await rA.read();
            var valB2 = await rB.read();
            if (valA1.value !== 'item1' || valB1.value !== 'item1' || valA2.value !== 'item2' || valB2.value !== 'item2') {
                throw new Error('tee() stream mismatch');
            }

            // 4. TransformStream
            var uppercaseTransform = new TransformStream({
                transform: function(chunk, controller) {
                    controller.enqueue(String(chunk).toUpperCase());
                }
            });
            var writer = uppercaseTransform.writable.getWriter();
            var reader = uppercaseTransform.readable.getReader();

            writer.write('mango');
            var transformed = await reader.read();
            if (transformed.value !== 'MANGO') {
                throw new Error('TransformStream failed: ' + transformed.value);
            }
            await writer.close();
            var doneRes = await reader.read();
            if (!doneRes.done) throw new Error('TransformStream reader should be done after writer close');

            // 5. pipeTo with AbortSignal
            var countStream = new ReadableStream({
                start: function(c) {
                    c.enqueue('x');
                    c.enqueue('y');
                }
            });
            var controller = new AbortController();
            controller.abort();
            var dummySink = new WritableStream({});
            var threwAbort = false;
            try {
                await countStream.pipeTo(dummySink, { signal: controller.signal });
            } catch(e) {
                threwAbort = true;
            }
            if (!threwAbort) throw new Error('pipeTo should reject when aborted signal provided');
        })();
    "#;
    rt.execute_script(script)
        .expect("Streams conformance test failed");
}

#[test]
fn test_whatwg_form_data_conformance() {
    let mut rt = create_test_runtime();
    let script = r#"
        (function() {
            var fd = new FormData();
            fd.append('username', 'alice');
            fd.append('role', 'admin');
            fd.append('role', 'editor');

            if (fd.get('username') !== 'alice') throw new Error('fd.get failed');
            var roles = fd.getAll('role');
            if (roles.length !== 2 || roles[0] !== 'admin' || roles[1] !== 'editor') {
                throw new Error('fd.getAll failed: ' + JSON.stringify(roles));
            }
            if (!fd.has('username') || !fd.has('role')) throw new Error('fd.has failed');

            // set replaces existing and removes duplicates
            fd.set('role', 'superuser');
            roles = fd.getAll('role');
            if (roles.length !== 1 || roles[0] !== 'superuser') {
                throw new Error('fd.set failed to replace: ' + JSON.stringify(roles));
            }

            // delete
            fd.delete('username');
            if (fd.has('username') || fd.get('username') !== null) {
                throw new Error('fd.delete failed');
            }

            // Iterator and keys
            fd.append('k1', 'v1');
            fd.append('k2', 'v2');
            var keys = [];
            for (var [k, v] of fd) {
                keys.push(k + '=' + v);
            }
            if (keys.indexOf('k1=v1') === -1 || keys.indexOf('k2=v2') === -1) {
                throw new Error('FormData iterator failed: ' + keys.join(','));
            }

            // Symbol.toStringTag
            if (Object.prototype.toString.call(fd) !== '[object FormData]') {
                throw new Error('FormData toStringTag mismatch: ' + Object.prototype.toString.call(fd));
            }
        })();
    "#;
    rt.execute_script(script)
        .expect("FormData conformance test failed");
}

#[test]
fn test_whatwg_storage_conformance() {
    let mut rt = create_test_runtime();
    let script = r#"
        (function() {
            // Clear storage
            localStorage.clear();
            sessionStorage.clear();
            if (localStorage.length !== 0 || sessionStorage.length !== 0) {
                throw new Error('Storage should be empty after clear()');
            }

            // 1. setItem and getItem
            localStorage.setItem('theme', 'dark');
            localStorage.setItem('version', '2');
            if (localStorage.getItem('theme') !== 'dark') throw new Error('getItem theme mismatch');
            if (localStorage.getItem('version') !== '2') throw new Error('getItem version mismatch');
            if (localStorage.getItem('nonexistent') !== null) throw new Error('missing key should return null');

            // 2. Property proxy getter and setter
            localStorage.user = 'bob';
            if (localStorage.getItem('user') !== 'bob') throw new Error('Proxy setter failed');
            if (localStorage.user !== 'bob') throw new Error('Proxy getter failed');
            if (!('user' in localStorage)) throw new Error("'in' operator on localStorage failed");

            // 3. length and deterministic key(index)
            if (localStorage.length !== 3) throw new Error('localStorage.length mismatch: ' + localStorage.length);
            var collectedKeys = [];
            for (var i = 0; i < localStorage.length; i++) {
                var k = localStorage.key(i);
                if (k === null) throw new Error('key(' + i + ') returned null');
                collectedKeys.push(k);
            }
            if (collectedKeys.indexOf('theme') === -1 || collectedKeys.indexOf('user') === -1 || collectedKeys.indexOf('version') === -1) {
                throw new Error('Collected keys missing items: ' + collectedKeys.join(','));
            }

            // 4. delete property and removeItem
            delete localStorage.user;
            if (localStorage.getItem('user') !== null || localStorage.user !== undefined) {
                throw new Error('delete localStorage.prop failed');
            }
            localStorage.removeItem('theme');
            if (localStorage.getItem('theme') !== null) {
                throw new Error('removeItem failed');
            }
            if (localStorage.length !== 1) {
                throw new Error('length after removals should be 1, got: ' + localStorage.length);
            }

            // 5. sessionStorage isolation
            sessionStorage.setItem('tab_data', 'isolated');
            if (localStorage.getItem('tab_data') !== null) {
                throw new Error('sessionStorage leaked to localStorage');
            }
            if (sessionStorage.getItem('tab_data') !== 'isolated') {
                throw new Error('sessionStorage getItem mismatch');
            }

            // 6. Symbol.toStringTag
            if (Object.prototype.toString.call(localStorage) !== '[object Storage]') {
                throw new Error('localStorage toStringTag mismatch: ' + Object.prototype.toString.call(localStorage));
            }
        })();
    "#;
    rt.execute_script(script)
        .expect("Storage conformance test failed");
}
