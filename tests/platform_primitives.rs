//! Platform Primitives Test Suite.
//!
//! Validates:
//! - HTML5 Popover API and auto light-dismiss
//! - Declarative Shadow DOM (DSD) JS reflection
//! - WHATWG Streams API (ReadableStream, WritableStream, TransformStream)
//! - Modern Fetch & Response stream body integration
//! - Canvas 2D Level 2 (Path2D, SVG paths, conic gradients, filter, OffscreenCanvas)

use mango_html::parse_html;
use mango_js::JsRuntime;

#[test]
fn test_html5_popover_api_and_events() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <button id="btn" popovertarget="my-popover" popovertargetaction="toggle">Toggle</button>
                <div id="my-popover" popover="auto">
                    Popover Content
                </div>
                <div id="manual-popover" popover="manual">
                    Manual Popover Content
                </div>
            </body>
        </html>
    "#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        const popover = document.getElementById('my-popover');
        let toggled = false;
        let eventStates = [];
        popover.addEventListener('toggle', (e) => {
            toggled = true;
            eventStates.push({ oldState: e.oldState, newState: e.newState });
        });

        popover.showPopover();
        if (popover.getAttribute('data-mango-popover-open') !== 'true') throw new Error("Popover should be open");
        if (popover.popover !== 'auto') throw new Error("Popover property should be auto");

        const manual = document.getElementById('manual-popover');
        manual.showPopover();
        if (manual.getAttribute('data-mango-popover-open') !== 'true') throw new Error("Manual popover should be open");

        popover.hidePopover();
        if (popover.getAttribute('data-mango-popover-open') === 'true') throw new Error("Popover should be closed");
        if (!toggled) throw new Error("Toggle event was not fired");
        if (eventStates.length < 2) throw new Error("Expected multiple toggle events");
        if (eventStates[0].newState !== 'open') throw new Error("First toggle newState should be open");
        if (eventStates[1].newState !== 'closed') throw new Error("Second toggle newState should be closed");
    "#;

    rt.execute_script(script).expect("Popover API test failed");
}

#[test]
fn test_popover_invoker_and_escape_dismiss() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <button id="btn" popovertarget="pop" popovertargetaction="show">Open</button>
                <div id="pop" popover="auto">Content</div>
            </body>
        </html>
    "#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        const btn = document.getElementById('btn');
        const pop = document.getElementById('pop');

        if (btn.popoverTargetElement !== pop) throw new Error("popoverTargetElement should match target element");
        if (btn.popoverTargetAction !== 'show') throw new Error("popoverTargetAction should be show");

        btn.click();
        if (pop.getAttribute('data-mango-popover-open') !== 'true') throw new Error("Button click should open popover");

        // Dispatch Escape keydown to trigger auto light-dismiss
        const escEvent = new KeyboardEvent('keydown', { key: 'Escape', keyCode: 27, bubbles: true });
        document.dispatchEvent(escEvent);
        if (pop.getAttribute('data-mango-popover-open') === 'true') throw new Error("Escape key should light-dismiss popover");
    "#;

    rt.execute_script(script).expect("Popover invoker test failed");
}

#[test]
fn test_declarative_shadow_dom_js_reflection() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <host-component id="open-host">
                    <template shadowrootmode="open">
                        <span id="open-shadow-child">Open Shadow DOM Child</span>
                    </template>
                    <div id="light-child">Light DOM Child</div>
                </host-component>

                <host-component id="closed-host">
                    <template shadowrootmode="closed">
                        <span id="closed-shadow-child">Closed Shadow DOM Child</span>
                    </template>
                </host-component>
            </body>
        </html>
    "#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        const openHost = document.getElementById('open-host');
        const closedHost = document.getElementById('closed-host');

        if (!openHost.shadowRoot) throw new Error("Open host must have shadowRoot");
        if (openHost.shadowRoot.mode !== 'open') throw new Error("Open host shadowRoot mode must be 'open'");
        if (closedHost.shadowRoot !== null) throw new Error("Closed host shadowRoot must be null in JS");
    "#;

    rt.execute_script(script).expect("DSD JS reflection test failed");
}

#[test]
fn test_whatwg_streams_and_queuing_strategies() {
    let html = "<!DOCTYPE html><html><body></body></html>";
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. ReadableStream
        let received = [];
        const rs = new ReadableStream({
            start(controller) {
                controller.enqueue('first');
                controller.enqueue('second');
                controller.close();
            }
        });

        if (rs.locked) throw new Error("Stream should initially not be locked");
        const reader = rs.getReader();
        if (!rs.locked) throw new Error("Stream should be locked after getReader");

        reader.read().then(r => {
            received.push(r.value);
            return reader.read();
        }).then(r => {
            received.push(r.value);
            return reader.read();
        }).then(r => {
            if (!r.done) throw new Error("Stream should be done");
            if (received[0] !== 'first' || received[1] !== 'second') throw new Error("Stream chunks mismatch: " + JSON.stringify(received));
        });

        // 2. Queuing strategies
        const bq = new ByteLengthQueuingStrategy({ highWaterMark: 1024 });
        if (bq.highWaterMark !== 1024) throw new Error("ByteLengthQueuingStrategy highWaterMark mismatch");
        if (bq.size({ byteLength: 512 }) !== 512) throw new Error("ByteLengthQueuingStrategy size mismatch");

        const cq = new CountQueuingStrategy({ highWaterMark: 5 });
        if (cq.highWaterMark !== 5) throw new Error("CountQueuingStrategy highWaterMark mismatch");
        if (cq.size('item') !== 1) throw new Error("CountQueuingStrategy size should be 1");

        // 3. TransformStream
        const ts = new TransformStream({
            transform(chunk, controller) {
                controller.enqueue(chunk.toUpperCase());
            }
        });

        const tw = ts.writable.getWriter();
        const tr = ts.readable.getReader();

        tw.write('hello');
        tw.close();

        tr.read().then(r => {
            if (r.value !== 'HELLO') throw new Error("TransformStream transformed chunk mismatch: " + r.value);
        });
    "#;

    rt.execute_script(script).expect("Streams test failed");
}

#[test]
fn test_fetch_and_response_stream_body() {
    let html = "<!DOCTYPE html><html><body></body></html>";
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. Response.prototype.body as ReadableStream
        const res = new Response("Stream Data Payload");
        const bodyStream = res.body;
        if (!(bodyStream instanceof ReadableStream)) throw new Error("Response.body must be an instance of ReadableStream");
        if (res.bodyUsed) throw new Error("res.bodyUsed should initially be false");

        const reader = bodyStream.getReader();
        reader.read().then(({ value, done }) => {
            if (done) throw new Error("First read should not be done");
            const text = new TextDecoder().decode(value);
            if (text !== "Stream Data Payload") throw new Error("Stream chunk mismatch: " + text);
        });

        // 2. Construct Response from ReadableStream
        const sourceStream = new ReadableStream({
            start(controller) {
                controller.enqueue(new TextEncoder().encode("Piped Response Body"));
                controller.close();
            }
        });

        const resFromStream = new Response(sourceStream);
        resFromStream.text().then(txt => {
            if (txt !== "Piped Response Body") throw new Error("Response(stream).text() mismatch: " + txt);
        });

        // 3. AbortSignal timeout and any
        const sigTimeout = AbortSignal.timeout(1000);
        if (!(sigTimeout instanceof AbortSignal)) throw new Error("AbortSignal.timeout must return an AbortSignal");

        const c1 = new AbortController();
        const c2 = new AbortController();
        const anySig = AbortSignal.any([c1.signal, c2.signal]);
        if (anySig.aborted) throw new Error("anySig should initially not be aborted");
        c1.abort("reason");
        if (!anySig.aborted) throw new Error("anySig should be aborted after source controller aborts");
    "#;

    rt.execute_script(script).expect("Fetch and Response stream test failed");
}

#[test]
fn test_canvas_2d_level_2_and_offscreen_canvas() {
    let html = r#"
        <!DOCTYPE html>
        <html>
            <body>
                <canvas id="c" width="300" height="200"></canvas>
            </body>
        </html>
    "#;

    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        const canvas = document.getElementById('c');
        const ctx = canvas.getContext('2d');

        // 1. Path2D API
        const path = new Path2D();
        path.rect(10, 10, 80, 80);
        path.arc(50, 50, 20, 0, Math.PI * 2);
        ctx.fill(path);
        ctx.stroke(path);

        // SVG path parsing in Path2D
        const svgPath = new Path2D("M 10 10 L 90 90 H 120 V 150 Z");
        ctx.fill(svgPath, 'evenodd');

        // 2. Canvas 2D filter property
        if (ctx.filter !== 'none') throw new Error("Initial ctx.filter should be 'none'");
        ctx.filter = "blur(5px)";
        if (ctx.filter !== "blur(5px)") throw new Error("ctx.filter setter/getter failed: " + ctx.filter);

        // 3. createConicGradient
        const conic = ctx.createConicGradient(0, 100, 100);
        conic.addColorStop(0, '#ff0000');
        conic.addColorStop(1, '#0000ff');
        if (!conic._stops || conic._stops.length !== 2) throw new Error("Conic gradient stops missing");

        // 4. OffscreenCanvas
        const off = new OffscreenCanvas(200, 100);
        if (off.width !== 200 || off.height !== 100) throw new Error("OffscreenCanvas dimensions mismatch");
        const offCtx = off.getContext('2d');
        offCtx.fillStyle = '#00ff00';
        offCtx.fillRect(0, 0, 50, 50);

        const bitmap = off.transferToImageBitmap();
        if (bitmap.width !== 200 || bitmap.height !== 100) throw new Error("ImageBitmap dimensions mismatch");

        // 5. HTMLCanvasElement.transferControlToOffscreen
        let threwOnExistingContext = false;
        try {
            canvas.transferControlToOffscreen();
        } catch(e) {
            threwOnExistingContext = true;
        }
        if (!threwOnExistingContext) throw new Error("transferControlToOffscreen on canvas with context must throw InvalidStateError");

        const freshCanvas = document.createElement('canvas');
        freshCanvas.width = 300;
        freshCanvas.height = 200;
        const offFromElement = freshCanvas.transferControlToOffscreen();
        if (offFromElement.width !== 300 || offFromElement.height !== 200) throw new Error("transferred canvas dimensions mismatch");

        let threwSecond = false;
        try {
            freshCanvas.transferControlToOffscreen();
        } catch(e) {
            threwSecond = true;
        }
        if (!threwSecond) throw new Error("Second transferControlToOffscreen should throw InvalidStateError");
    "#;

    rt.execute_script(script).expect("Canvas Level 2 test failed");
}
