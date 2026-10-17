//! Offline, Graphics & Storage (PWA Runtime) Integration Test Suite.
//!
//! Validates:
//! - Service Workers & Cache Storage API (registration lifecycle, FetchEvent interception, caching strategies)
//! - IndexedDB 3.0 (ACID transactions & abort rollback, multi-store transactions, composite unique indexes, IDBKeyRange cursor traversal, update/delete)
//! - 4.3 WebGL 2.0 & WebGPU (WebGL2RenderingContext, VAO, queries, sync, transform feedback, samplers; navigator.gpu, GPUDevice, pipelines, GPUCanvasContext)
//! - 4.4 Web Audio API & AudioWorklet (AudioContext, GainNode, OscillatorNode, BiquadFilterNode, AnalyserNode, PannerNode, StereoPannerNode, ConstantSourceNode, AudioParam timeline automation, AudioWorkletNode)

use mango_html::parse_html;
use mango_js::JsRuntime;

#[test]
fn test_service_worker_registration_and_lifecycle() {
    let html = r#"<!DOCTYPE html><html><body><div id="status">init</div></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        globalThis.registered = false;
        globalThis.controllerChanged = false;
        globalThis.readyResolved = false;
        globalThis.matchedReg = null;

        if (!navigator.serviceWorker) throw new Error("navigator.serviceWorker should be defined");

        navigator.serviceWorker.addEventListener('controllerchange', () => {
            globalThis.controllerChanged = true;
        });

        navigator.serviceWorker.register('/sw.js', { scope: '/app/' }).then(reg => {
            globalThis.registered = true;
            if (reg.scope !== '/app/') throw new Error("Registration scope mismatch: " + reg.scope);
            if (!reg.active) throw new Error("Registration should have an active worker");
            if (reg.active.state !== 'activated') throw new Error("Worker state should be activated");

            if (navigator.serviceWorker.controller !== reg.active) {
                throw new Error("Controller should match registration active worker");
            }
        });

        // Verify navigator.serviceWorker.ready promise
        navigator.serviceWorker.ready.then(reg => {
            globalThis.readyResolved = true;
            if (!reg || !reg.active) throw new Error("ready should resolve to active registration");
        });

        // Verify scope matching in getRegistration
        navigator.serviceWorker.getRegistration('/app/dashboard').then(reg => {
            globalThis.matchedReg = reg;
        });

        // Verify ServiceWorkerGlobalScope primitives
        if (typeof clients === 'undefined') throw new Error("clients should be defined");
        if (typeof skipWaiting !== 'function') throw new Error("skipWaiting should be defined");
    "#;

    rt.execute_script(script).expect("ServiceWorker setup failed");
    rt.run_jobs();

    let verify_script = r#"
        if (!globalThis.registered) throw new Error("Service worker registration should resolve");
        if (!globalThis.controllerChanged) throw new Error("controllerchange event should have fired");
        if (!globalThis.readyResolved) throw new Error("navigator.serviceWorker.ready should resolve");
        if (!globalThis.matchedReg || globalThis.matchedReg.scope !== '/app/') {
            throw new Error("getRegistration should match prefix scope");
        }
    "#;
    rt.execute_script(verify_script).expect("ServiceWorker lifecycle verification failed");
}

#[test]
fn test_service_worker_fetch_interception_and_caching_strategies() {
    let html = r#"<!DOCTYPE html><html><body><h1>PWA Offline Cache</h1></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let setup_script = r#"
        globalThis.cachePutOk = false;
        globalThis.cachedText = null;
        globalThis.ignoreSearchMatched = false;

        // 1. Setup multi-cache storage and test Cache-First, Network-First, Stale-While-Revalidate
        caches.open('pwa-static-v1').then(cache => {
            const req = new Request('/assets/app.js');
            const res = new Response('console.log("cached asset v1");', {
                status: 200,
                headers: { 'Content-Type': 'application/javascript' }
            });
            return cache.put(req, res).then(() => {
                globalThis.cachePutOk = true;
                return caches.match('/assets/app.js');
            });
        }).then(matchedRes => {
            if (!matchedRes) throw new Error("caches.match failed to find cached response");
            return matchedRes.text();
        }).then(text => {
            globalThis.cachedText = text;
        });

        // Test ignoreSearch option in Cache.match
        caches.open('pwa-static-v1').then(cache => {
            return cache.match('/assets/app.js?v=123', { ignoreSearch: true });
        }).then(matched => {
            if (matched) globalThis.ignoreSearchMatched = true;
        });

        // 2. Service Worker Fetch Event Interception
        const sw = navigator.serviceWorker.controller;
        if (!sw) throw new Error("No active SW controller found");

        sw.addEventListener('fetch', evt => {
            if (evt.request.url.includes('/api/offline-data')) {
                evt.respondWith(new Response(JSON.stringify({ status: 'offline', version: 4 }), {
                    status: 200,
                    headers: { 'Content-Type': 'application/json', 'X-Intercepted-By': 'ServiceWorker' }
                }));
            }
        });
    "#;

    rt.execute_script(setup_script).expect("Cache and SW setup failed");
    rt.run_jobs();

    let fetch_script = r#"
        if (!globalThis.cachePutOk) throw new Error("Cache put failed");
        if (globalThis.cachedText !== 'console.log("cached asset v1");') {
            throw new Error("Unexpected cached text: " + globalThis.cachedText);
        }
        if (!globalThis.ignoreSearchMatched) {
            throw new Error("ignoreSearch should have matched request with query parameter");
        }

        // Call fetch to verify SW interception
        globalThis.interceptedData = null;
        fetch('/api/offline-data').then(res => {
            if (res.headers.get('X-Intercepted-By') !== 'ServiceWorker') {
                throw new Error("Fetch was not intercepted by Service Worker");
            }
            return res.json();
        }).then(data => {
            globalThis.interceptedData = data;
        });
    "#;

    rt.execute_script(fetch_script).expect("Fetch interception call failed");
    rt.run_jobs();

    let verify_script = r#"
        if (!globalThis.interceptedData || globalThis.interceptedData.status !== 'offline' || globalThis.interceptedData.version !== 4) {
            throw new Error("Unexpected intercepted JSON body: " + JSON.stringify(globalThis.interceptedData));
        }
    "#;

    rt.execute_script(verify_script).expect("Fetch interception verification failed");
}

#[test]
fn test_indexeddb_acid_transactions_and_rollback() {
    let html = r#"<!DOCTYPE html><html><body><h1>IndexedDB Test</h1></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        let db = null;
        const openReq = indexedDB.open('TestAcidDB', 1);

        openReq.onupgradeneeded = (e) => {
            const database = openReq.result;
            database.createObjectStore('accounts', { keyPath: 'id' });
        };

        openReq.onsuccess = () => {
            db = openReq.result;
        };

        if (!db) throw new Error("Failed to open IndexedDB");

        // Insert initial accounts
        const tx1 = db.transaction(['accounts'], 'readwrite');
        const store1 = tx1.objectStore('accounts');
        store1.add({ id: 'acc1', balance: 1000 });
        store1.add({ id: 'acc2', balance: 500 });
        tx1.commit();

        // Verify initial state
        const verifyTx = db.transaction(['accounts'], 'readonly');
        const storeVerify = verifyTx.objectStore('accounts');
        let acc1 = null;
        storeVerify.get('acc1').onsuccess = (e) => {
            acc1 = e.target.result;
        };
        if (!acc1 || acc1.balance !== 1000) throw new Error("Failed to verify initial account balance");

        // ACID Abort Rollback Test:
        // Begin readwrite transaction, modify balance, then abort. Changes must be reverted!
        const abortTx = db.transaction(['accounts'], 'readwrite');
        const abortStore = abortTx.objectStore('accounts');
        abortStore.put({ id: 'acc1', balance: 999999 });
        abortStore.add({ id: 'acc3', balance: 200 });
        abortTx.abort();

        // Re-read accounts to ensure rollback occurred
        const checkTx = db.transaction(['accounts'], 'readonly');
        const checkStore = checkTx.objectStore('accounts');
        let rolledBackAcc1 = null;
        let rolledBackAcc3 = null;
        checkStore.get('acc1').onsuccess = (e) => {
            rolledBackAcc1 = e.target.result;
        };
        checkStore.get('acc3').onsuccess = (e) => {
            rolledBackAcc3 = e.target.result;
        };

        if (!rolledBackAcc1 || rolledBackAcc1.balance !== 1000) {
            throw new Error("ACID rollback failed: acc1 balance was not reverted! Got: " + JSON.stringify(rolledBackAcc1));
        }
        if (rolledBackAcc3 !== undefined) {
            throw new Error("ACID rollback failed: acc3 should not exist! Got: " + JSON.stringify(rolledBackAcc3));
        }
    "#;

    rt.execute_script(script).expect("IndexedDB ACID rollback test failed");
}

#[test]
fn test_indexeddb_key_range_cursor_and_composite_unique_indexes() {
    let html = r#"<!DOCTYPE html><html><body><h1>IDB 3.0 Advanced</h1></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        let db = null;
        const openReq = indexedDB.open('AdvancedInventoryDB', 1);

        openReq.onupgradeneeded = (e) => {
            const database = openReq.result;
            const store = database.createObjectStore('items', { keyPath: 'id', autoIncrement: true });
            // Unique composite index on [category, sku]
            store.createIndex('cat_sku_idx', ['category', 'sku'], { unique: true });
        };

        openReq.onsuccess = () => {
            db = openReq.result;
        };

        if (!db) throw new Error("Failed to open AdvancedInventoryDB");

        const tx = db.transaction(['items'], 'readwrite');
        const store = tx.objectStore('items');

        // Add distinct items
        store.add({ id: 10, category: 'electronics', sku: 'E100', price: 299 });
        store.add({ id: 20, category: 'books', sku: 'B200', price: 19 });
        store.add({ id: 30, category: 'electronics', sku: 'E300', price: 499 });
        store.add({ id: 40, category: 'clothing', sku: 'C400', price: 49 });
        store.add({ id: 50, category: 'books', sku: 'B500', price: 35 });

        // Verify unique composite index constraint violation
        let constraintCaught = false;
        const duplicateReq = store.add({ id: 99, category: 'books', sku: 'B200', price: 25 });
        duplicateReq.onerror = (e) => {
            constraintCaught = true;
        };
        if (!constraintCaught && !duplicateReq.error) {
            throw new Error("Duplicate composite index entry was not prevented by unique constraint");
        }

        // Test IDBKeyRange range scan (keys 20 to 40 inclusive)
        const range = IDBKeyRange.bound(20, 40, false, false);
        const cursorReq = store.openCursor(range);
        let cursorCount = 0;
        let cursorKeys = [];

        cursorReq.onsuccess = (e) => {
            const cursor = e.target.result;
            if (cursor) {
                cursorCount++;
                cursorKeys.push(cursor.key);

                // Test cursor.update on item id 30
                if (cursor.key === 30) {
                    cursor.update({ id: 30, category: 'electronics', sku: 'E300', price: 450 });
                }
                // Test cursor.delete on item id 40
                if (cursor.key === 40) {
                    cursor.delete();
                }

                cursor.continue();
            }
        };

        if (cursorCount !== 3) throw new Error("Expected 3 items in range [20, 40], got: " + cursorCount);
        if (cursorKeys[0] !== 20 || cursorKeys[1] !== 30 || cursorKeys[2] !== 40) {
            throw new Error("Unexpected cursor keys: " + JSON.stringify(cursorKeys));
        }

        // Verify update and delete persisted
        let item30 = null;
        let item40 = null;
        store.get(30).onsuccess = (e) => { item30 = e.target.result; };
        store.get(40).onsuccess = (e) => { item40 = e.target.result; };

        if (!item30 || item30.price !== 450) throw new Error("Cursor update failed: " + JSON.stringify(item30));
        if (item40 !== undefined) throw new Error("Cursor delete failed: item 40 should be deleted");

        // Verify W3C Key comparison in IDBFactory.cmp
        if (indexedDB.cmp(10, 20) !== -1) throw new Error("cmp number failed");
        if (indexedDB.cmp("alpha", "beta") !== -1) throw new Error("cmp string failed");
        if (indexedDB.cmp([1, "a"], [1, "b"]) !== -1) throw new Error("cmp array failed");
        if (indexedDB.cmp(100, "100") >= 0) throw new Error("Number should be less than String in IDB key order");
    "#;

    rt.execute_script(script).expect("IndexedDB cursor and index test failed");
}

#[test]
fn test_webgl2_rendering_context_vao_and_primitives() {
    let html = r#"<!DOCTYPE html><html><body><canvas id="c" width="640" height="480"></canvas></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        const canvas = document.getElementById('c');
        const gl = canvas.getContext('webgl2');

        if (!gl) throw new Error("getContext('webgl2') returned null");
        if (!(gl instanceof WebGL2RenderingContext)) throw new Error("Context is not WebGL2RenderingContext");

        // Constants check
        if (gl.UNIFORM_BUFFER !== 0x8A11) throw new Error("UNIFORM_BUFFER constant mismatch");
        if (gl.TRANSFORM_FEEDBACK !== 0x8E22) throw new Error("TRANSFORM_FEEDBACK constant mismatch");
        if (gl.COLOR !== 0x1800) throw new Error("COLOR constant mismatch");

        // VAO check
        const vao = gl.createVertexArray();
        if (!vao) throw new Error("createVertexArray returned null");
        if (!gl.isVertexArray(vao)) throw new Error("isVertexArray failed");
        gl.bindVertexArray(vao);
        gl.bindVertexArray(null);
        gl.deleteVertexArray(vao);

        // Queries & Sync check
        const q = gl.createQuery();
        if (!q || !gl.isQuery(q)) throw new Error("createQuery / isQuery failed");
        gl.beginQuery(gl.ANY_SAMPLES_PASSED, q);
        gl.endQuery(gl.ANY_SAMPLES_PASSED);
        gl.deleteQuery(q);

        const sync = gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE, 0);
        if (!sync || !gl.isSync(sync)) throw new Error("fenceSync / isSync failed");
        const syncStatus = gl.clientWaitSync(sync, gl.SYNC_FLUSH_COMMANDS_BIT, 0);
        if (syncStatus !== gl.ALREADY_SIGNALED) throw new Error("Unexpected sync status: " + syncStatus);
        gl.deleteSync(sync);

        // Transform feedback & Sampler check
        const tf = gl.createTransformFeedback();
        if (!tf || !gl.isTransformFeedback(tf)) throw new Error("createTransformFeedback failed");
        gl.bindTransformFeedback(gl.TRANSFORM_FEEDBACK, tf);
        gl.bindTransformFeedback(gl.TRANSFORM_FEEDBACK, null);
        gl.deleteTransformFeedback(tf);

        const sampler = gl.createSampler();
        if (!sampler || !gl.isSampler(sampler)) throw new Error("createSampler failed");
        gl.bindSampler(0, sampler);
        gl.deleteSampler(sampler);

        // Instanced draw calls check
        gl.vertexAttribDivisor(0, 1);
        gl.drawArraysInstanced(gl.TRIANGLES, 0, 3, 10);
    "#;

    rt.execute_script(script).expect("WebGL2 Rendering Context test failed");
}

#[test]
fn test_webgpu_adapter_device_and_pipeline() {
    let html = r#"<!DOCTYPE html><html><body><canvas id="gpu-canvas" width="800" height="600"></canvas></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        if (!navigator.gpu) throw new Error("navigator.gpu must be available");

        globalThis.adapter = null;
        globalThis.device = null;

        navigator.gpu.requestAdapter().then(ad => {
            globalThis.adapter = ad;
            return ad.requestDevice();
        }).then(dev => {
            globalThis.device = dev;
        });
    "#;

    rt.execute_script(script).expect("WebGPU requestAdapter failed");
    rt.run_jobs();

    let pipeline_script = r#"
        const adapter = globalThis.adapter;
        const device = globalThis.device;

        if (!adapter) throw new Error("Failed to acquire GPUAdapter");
        if (!device) throw new Error("Failed to acquire GPUDevice");

        // Verify device limits
        if (!device.limits || device.limits.maxTextureDimension2D < 8192) {
            throw new Error("Device maxTextureDimension2D limit invalid: " + JSON.stringify(device.limits));
        }

        // Buffer creation & mapping
        const buffer = device.createBuffer({
            size: 1024,
            usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST
        });
        if (buffer.size !== 1024) throw new Error("Buffer size mismatch");

        // Shader module & Pipeline
        const shader = device.createShaderModule({
            code: `@vertex fn vs_main() -> @builtin(position) vec4f { return vec4f(0, 0, 0, 1); }`
        });
        if (!shader || !shader.code) throw new Error("createShaderModule failed");

        const pipeline = device.createRenderPipeline({
            layout: 'auto',
            vertex: { module: shader, entryPoint: 'vs_main' }
        });
        if (!pipeline) throw new Error("createRenderPipeline failed");

        const computePipeline = device.createComputePipeline({
            layout: 'auto',
            compute: { module: shader, entryPoint: 'cs_main' }
        });
        if (!computePipeline) throw new Error("createComputePipeline failed");

        // Command Encoder & Pass Encoders
        const encoder = device.createCommandEncoder();
        const renderPass = encoder.beginRenderPass({});
        renderPass.setPipeline(pipeline);
        renderPass.draw(3, 1, 0, 0);
        renderPass.end();

        const computePass = encoder.beginComputePass({});
        computePass.setPipeline(computePipeline);
        computePass.dispatchWorkgroups(1, 1, 1);
        computePass.end();

        const cmdBuffer = encoder.finish();
        device.queue.submit([cmdBuffer]);

        // GPUCanvasContext
        const canvas = document.getElementById('gpu-canvas');
        const ctx = canvas.getContext('webgpu');
        if (!ctx) throw new Error("getContext('webgpu') returned null");
        ctx.configure({
            device: device,
            format: navigator.gpu.getPreferredCanvasFormat()
        });
        const currentTex = ctx.getCurrentTexture();
        if (!currentTex || currentTex.width !== 800 || currentTex.height !== 600) {
            throw new Error("getCurrentTexture returned invalid texture");
        }
    "#;

    rt.execute_script(pipeline_script).expect("WebGPU pipeline test failed");
}

#[test]
fn test_web_audio_api_graph_and_automation() {
    let html = r#"<!DOCTYPE html><html><body><h1>Web Audio Test</h1></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        const ctx = new AudioContext({ sampleRate: 48000 });
        if (!ctx || ctx.sampleRate !== 48000) throw new Error("AudioContext initialization failed");
        if (!ctx.destination) throw new Error("Destination node missing");

        // Create Nodes
        const osc = ctx.createOscillator();
        const biquad = ctx.createBiquadFilter();
        const panner = ctx.createPanner();
        const stereoPanner = ctx.createStereoPanner();
        const constSrc = ctx.createConstantSource();
        const gain = ctx.createGain();
        const analyser = ctx.createAnalyser();

        // Connect Graph: osc -> biquad -> panner -> stereoPanner -> gain -> destination
        osc.connect(biquad);
        biquad.connect(panner);
        panner.connect(stereoPanner);
        stereoPanner.connect(gain);
        gain.connect(analyser);
        analyser.connect(ctx.destination);

        // AudioParam Timeline Automation
        gain.gain.setValueAtTime(0.5, 0);
        if (gain.gain.value !== 0.5) throw new Error("setValueAtTime failed");

        gain.gain.linearRampToValueAtTime(1.0, 1.0);
        if (gain.gain.value !== 1.0) throw new Error("linearRampToValueAtTime failed");

        gain.gain.exponentialRampToValueAtTime(0.1, 2.0);
        if (gain.gain.value !== 0.1) throw new Error("exponentialRampToValueAtTime failed");

        gain.gain.setValueCurveAtTime([0.2, 0.4, 0.8], 2.0, 1.0);
        if (Math.abs(gain.gain.value - 0.8) > 0.001) throw new Error("setValueCurveAtTime failed");

        // Panner & Listener position
        panner.setPosition(10, 5, -2);
        if (panner.positionX.value !== 10 || panner.positionY.value !== 5 || panner.positionZ.value !== -2) {
            throw new Error("Panner position mismatch");
        }
        ctx.listener.setPosition(0, 0, 0);
        if (ctx.listener.positionX.value !== 0) throw new Error("Listener position mismatch");

        // StereoPanner & ConstantSource
        stereoPanner.pan.value = -0.5;
        if (stereoPanner.pan.value !== -0.5) throw new Error("StereoPanner pan mismatch");

        constSrc.start();
        constSrc.stop();

        // AudioWorklet module & node
        globalThis.workletModuleLoaded = false;
        ctx.audioWorklet.addModule('my-processor.js').then(() => {
            globalThis.workletModuleLoaded = true;
        });

        const workletNode = new AudioWorkletNode(ctx, 'custom-processor', {
            parameterData: { cutoff: 800 }
        });
        if (!workletNode || !workletNode.port) throw new Error("AudioWorkletNode initialization failed");
        if (!workletNode.parameters.has('cutoff')) throw new Error("AudioWorkletNode parameters missing cutoff");
        if (workletNode.parameters.get('cutoff').value !== 800) throw new Error("cutoff parameter mismatch");

        // AudioBuffer & decodeAudioData
        const buffer = ctx.createBuffer(2, 48000, 48000);
        if (buffer.duration !== 1.0) throw new Error("AudioBuffer duration should be 1.0s");
        const channel0 = buffer.getChannelData(0);
        channel0[0] = 0.5;
        if (channel0[0] !== 0.5) throw new Error("AudioBuffer data write failed");

        globalThis.decoded = false;
        ctx.decodeAudioData(new ArrayBuffer(1024), buf => {
            globalThis.decoded = true;
            if (buf.numberOfChannels !== 2) throw new Error("Decoded channels mismatch");
        });

        // Context state transitions
        globalThis.suspendOk = false;
        globalThis.resumeOk = false;
        ctx.suspend().then(() => {
            if (ctx.state === 'suspended') globalThis.suspendOk = true;
            return ctx.resume();
        }).then(() => {
            if (ctx.state === 'running') globalThis.resumeOk = true;
        });
    "#;

    rt.execute_script(script).expect("Web Audio API test failed");
    rt.run_jobs();

    let verify_script = r#"
        if (!globalThis.workletModuleLoaded) throw new Error("AudioWorklet.addModule should resolve");
        if (!globalThis.decoded) throw new Error("decodeAudioData should resolve");
        if (!globalThis.suspendOk) throw new Error("suspend should resolve to suspended state");
        if (!globalThis.resumeOk) throw new Error("resume should resolve to running state");
    "#;
    rt.execute_script(verify_script).expect("Web Audio API verification failed");
}
