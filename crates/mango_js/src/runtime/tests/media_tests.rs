use super::*;
use std::collections::HashMap;

#[test]
fn test_canvas_2d_and_web_animations() {
    let doc = parse_html("<html><body><canvas id='c'></canvas><div id='box'></div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r##"
        var c = document.getElementById("c");
        if (!c || c.width !== 300 || c.height !== 150) throw new Error("Canvas initial dims invalid");
        if (typeof HTMLCanvasElement !== 'undefined' && !(c instanceof HTMLCanvasElement)) {
            throw new Error("c not instanceof HTMLCanvasElement");
        }

        var ctx = c.getContext("2d");
        if (!ctx || ctx.canvas !== c) throw new Error("Canvas context or back-reference missing");
        if (typeof CanvasRenderingContext2D !== 'undefined' && !(ctx instanceof CanvasRenderingContext2D)) {
            throw new Error("ctx not instanceof CanvasRenderingContext2D");
        }

        ctx.fillStyle = "#ff0000";
        ctx.fillRect(0, 0, 100, 100);

        // Verify real rasterized red pixel at (50, 50)
        var imgData = ctx.getImageData(50, 50, 1, 1);
        if (!imgData || imgData.data[0] !== 255 || imgData.data[1] !== 0 || imgData.data[2] !== 0 || imgData.data[3] !== 255) {
            throw new Error("Canvas pixel verification failed: got [" + (imgData ? Array.from(imgData.data) : null) + "]");
        }

        // Verify pixel outside filled rect is transparent
        var outside = ctx.getImageData(150, 50, 1, 1);
        if (outside.data[3] !== 0) throw new Error("Outside pixel should be transparent");

        ctx.save();
        ctx.restore();

        // Real fontdue font metrics
        var metrics = ctx.measureText("Hello");
        if (!metrics || !(metrics.width > 0)) throw new Error("Canvas measureText failed: " + JSON.stringify(metrics));

        // PNG data URL snapshot
        var url = c.toDataURL();
        if (!url || url.indexOf("data:image/png;base64,") !== 0) throw new Error("Canvas toDataURL failed");

        var box = document.getElementById("box");
        var anim = box.animate([{ opacity: 0 }, { opacity: 1 }], 1000);
        if (!anim || typeof anim.play !== "function" || typeof anim.cancel !== "function") {
            throw new Error("Web Animations API failed");
        }
    "##;
    if let Err(e) = rt.execute_script(script) {
        panic!("Canvas test failed: {}", e);
    }
}

#[test]
fn test_canvas_2d_comprehensive_api() {
    let doc = parse_html("<html><body><div id='container'></div></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        // 1. Dynamic creation via document.createElement('canvas')
        var canvas = document.createElement('canvas');
        canvas.width = 200;
        canvas.height = 100;
        if (canvas.width !== 200 || canvas.height !== 100) throw new Error("Canvas width/height setting failed");

        var ctx = canvas.getContext('2d');
        if (!ctx) throw new Error("getContext('2d') returned null");

        // 2. Clear rect & fill rect
        ctx.fillStyle = '#00ff00';
        ctx.fillRect(10, 10, 50, 50);

        var greenPixel = ctx.getImageData(20, 20, 1, 1);
        if (greenPixel.data[0] !== 0 || greenPixel.data[1] !== 255 || greenPixel.data[2] !== 0 || greenPixel.data[3] !== 255) {
            throw new Error("Green pixel verification failed: got " + Array.from(greenPixel.data));
        }

        // clearRect should clear the area to transparent
        ctx.clearRect(15, 15, 10, 10);
        var clearedPixel = ctx.getImageData(16, 16, 1, 1);
        if (clearedPixel.data[3] !== 0) throw new Error("Cleared pixel should be transparent");

        // 3. PutImageData
        var newImg = ctx.createImageData(2, 2);
        // set to blue
        newImg.data[0] = 0; newImg.data[1] = 0; newImg.data[2] = 255; newImg.data[3] = 255;
        newImg.data[4] = 0; newImg.data[5] = 0; newImg.data[6] = 255; newImg.data[7] = 255;
        newImg.data[8] = 0; newImg.data[9] = 0; newImg.data[10] = 255; newImg.data[11] = 255;
        newImg.data[12] = 0; newImg.data[13] = 0; newImg.data[14] = 255; newImg.data[15] = 255;
        ctx.putImageData(newImg, 100, 50);

        var bluePixel = ctx.getImageData(100, 50, 1, 1);
        if (bluePixel.data[0] !== 0 || bluePixel.data[1] !== 0 || bluePixel.data[2] !== 255 || bluePixel.data[3] !== 255) {
            throw new Error("putImageData blue pixel verification failed: got " + Array.from(bluePixel.data));
        }

        // 4. Path operations (arc, lineTo, fill)
        ctx.beginPath();
        ctx.arc(80, 80, 10, 0, Math.PI * 2);
        ctx.fillStyle = '#ffff00';
        ctx.fill();

        var yellowPixel = ctx.getImageData(80, 80, 1, 1);
        if (yellowPixel.data[0] !== 255 || yellowPixel.data[1] !== 255 || yellowPixel.data[2] !== 0 || yellowPixel.data[3] !== 255) {
            throw new Error("yellow arc pixel failed: got " + Array.from(yellowPixel.data));
        }

        // 5. Gradients and patterns stub compatibility
        var grad = ctx.createLinearGradient(0, 0, 100, 100);
        grad.addColorStop(0, '#fff');
        grad.addColorStop(1, '#000');
        ctx.fillStyle = grad;
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_canvas_2d_comprehensive_api failed: {}", e);
    }
}

#[test]
fn test_webgl_webgl2_and_webgpu_apis() {
    let doc =
        parse_html("<html><body><canvas id='c1' width='300' height='150'></canvas></body></html>");
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);
    let script = r#"
        var c = document.getElementById('c1');
        if (!c) throw new Error("canvas element not found");

        // WebGL 1
        var gl = c.getContext('webgl');
        if (!gl) throw new Error("getContext('webgl') returned null");
        if (gl.COLOR_BUFFER_BIT !== 0x4000) throw new Error("WebGL constant missing");
        var vs = gl.createShader(gl.VERTEX_SHADER);
        gl.shaderSource(vs, "void main() { gl_Position = vec4(0.0); }");
        gl.compileShader(vs);
        if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) throw new Error("Shader compile failed");
        var prog = gl.createProgram();
        gl.attachShader(prog, vs);
        gl.linkProgram(prog);
        if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error("Program link failed");
        gl.useProgram(prog);
        var buf = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, buf);

        // WebGL 2
        var gl2 = c.getContext('webgl2');
        if (!gl2) throw new Error("getContext('webgl2') returned null");
        if (gl2.COLOR_BUFFER_BIT !== 0x4000) throw new Error("WebGL2 constant missing");
        var vao = gl2.createVertexArray();
        gl2.bindVertexArray(vao);

        // WebGPU
        if (!navigator.gpu) throw new Error("navigator.gpu not found");
        var gpuCtx = c.getContext('webgpu');
        if (!gpuCtx) throw new Error("getContext('webgpu') returned null");
        gpuCtx.configure({ format: 'bgra8unorm' });
        var tex = gpuCtx.getCurrentTexture();
        if (!tex || tex.width !== 300) throw new Error("getCurrentTexture failed");
    "#;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_webgl_webgl2_and_webgpu_apis failed: {}", e);
    }
}

#[test]
fn test_dom_measurement_and_layout() {
    let doc = parse_html(
        r#"
        <html>
        <head></head>
        <body>
          <div id="parent" style="position: relative; width: 400px; height: 300px; border-width: 5px;">
            <div id="child" style="color: red; font-size: 16px;">Hello</div>
          </div>
          <div id="target" style="width: 200px; height: 50px;">Target</div>
        </body>
        </html>
    "#,
    );
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let parent_id = {
        let d = rt.document_ref();
        let root = d.root();
        d.find_element_by_id(root, "parent").expect("parent")
    };
    let child_id = {
        let d = rt.document_ref();
        let root = d.root();
        d.find_element_by_id(root, "child").expect("child")
    };
    let target_id = {
        let d = rt.document_ref();
        let root = d.root();
        d.find_element_by_id(root, "target").expect("target")
    };

    let mut bounds = HashMap::new();
    bounds.insert(parent_id.raw(), [20.0, 30.0, 400.0, 300.0]);
    bounds.insert(child_id.raw(), [70.0, 90.0, 100.0, 80.0]);
    bounds.insert(target_id.raw(), [10.0, 400.0, 200.0, 50.0]);
    rt.set_layout_bounds(bounds);

    let script = r##"
        var parent = document.getElementById("parent");
        var child = document.getElementById("child");
        var target = document.getElementById("target");

        // 1. element.getBoundingClientRect()
        var childRect = child.getBoundingClientRect();
        if (!(childRect instanceof DOMRect)) throw new Error("childRect should be instanceof DOMRect");
        if (!(childRect instanceof DOMRectReadOnly)) throw new Error("childRect should be instanceof DOMRectReadOnly");
        if (childRect.x !== 70 || childRect.y !== 90 || childRect.width !== 100 || childRect.height !== 80) {
            throw new Error("getBoundingClientRect coordinates incorrect: " + JSON.stringify(childRect));
        }
        if (childRect.top !== 90 || childRect.left !== 70 || childRect.right !== 170 || childRect.bottom !== 170) {
            throw new Error("getBoundingClientRect bounds incorrect: " + JSON.stringify(childRect));
        }
        var jsonRect = childRect.toJSON();
        if (jsonRect.width !== 100 || jsonRect.height !== 80 || jsonRect.top !== 90 || jsonRect.left !== 70) {
            throw new Error("DOMRect.toJSON failed: " + JSON.stringify(jsonRect));
        }
        var fromRect = DOMRect.fromRect({ x: 5, y: 15, width: 25, height: 35 });
        if (fromRect.x !== 5 || fromRect.y !== 15 || fromRect.width !== 25 || fromRect.height !== 35) {
            throw new Error("DOMRect.fromRect failed");
        }
        var fromRectRO = DOMRectReadOnly.fromRect({ x: 1, y: 2, width: 3, height: 4 });
        if (fromRectRO.x !== 1 || fromRectRO.y !== 2 || fromRectRO.width !== 3 || fromRectRO.height !== 4) {
            throw new Error("DOMRectReadOnly.fromRect failed");
        }

        // 2. element.getClientRects()
        var clientRects = child.getClientRects();
        if (!(clientRects instanceof DOMRectList)) throw new Error("clientRects should be instanceof DOMRectList");
        if (clientRects.length !== 1) throw new Error("clientRects.length should be 1");
        if (clientRects.item(0).width !== 100) throw new Error("clientRects.item(0) width mismatch");
        if (clientRects[0].height !== 80) throw new Error("clientRects[0] height mismatch");
        var rectArr = Array.from(clientRects);
        if (rectArr.length !== 1 || rectArr[0].width !== 100) throw new Error("clientRects iterable mismatch");

        // 3. element.offsetWidth / offsetHeight / offsetTop / offsetLeft / offsetParent
        if (child.offsetWidth !== 100) throw new Error("child.offsetWidth mismatch: " + child.offsetWidth);
        if (child.offsetHeight !== 80) throw new Error("child.offsetHeight mismatch: " + child.offsetHeight);
        if (child.offsetParent !== parent) throw new Error("child.offsetParent should be parent");
        if (child.offsetLeft !== 50) throw new Error("child.offsetLeft should be 50, got: " + child.offsetLeft);
        if (child.offsetTop !== 60) throw new Error("child.offsetTop should be 60, got: " + child.offsetTop);
        if (parent.offsetWidth !== 400 || parent.offsetHeight !== 300) {
            throw new Error("parent offset dimensions mismatch");
        }
        var detached = document.createElement("div");
        if (detached.offsetParent !== null) throw new Error("detached element offsetParent should be null");

        // 4. element.clientWidth / clientHeight / clientTop / clientLeft
        if (parent.clientLeft !== 5) throw new Error("parent.clientLeft mismatch: " + parent.clientLeft);
        if (parent.clientTop !== 5) throw new Error("parent.clientTop mismatch: " + parent.clientTop);
        if (parent.clientWidth !== 390) throw new Error("parent.clientWidth mismatch: " + parent.clientWidth);
        if (parent.clientHeight !== 290) throw new Error("parent.clientHeight mismatch: " + parent.clientHeight);

        if (child.clientLeft !== 0 || child.clientTop !== 0) throw new Error("child clientTop/clientLeft mismatch");
        if (child.clientWidth !== 100 || child.clientHeight !== 80) throw new Error("child client dimensions mismatch");

        // 5. element.scrollWidth / scrollHeight / scrollTop / scrollLeft (get/set)
        if (child.scrollWidth !== 100 || child.scrollHeight !== 80) {
            throw new Error("child scrollWidth/Height mismatch: " + child.scrollWidth + "x" + child.scrollHeight);
        }
        child.scrollTop = 25;
        if (child.scrollTop !== 25) throw new Error("child.scrollTop setter failed: " + child.scrollTop);
        child.scrollLeft = 35;
        if (child.scrollLeft !== 35) throw new Error("child.scrollLeft setter failed: " + child.scrollLeft);

        child.scrollTo(10, 20);
        if (child.scrollLeft !== 10 || child.scrollTop !== 20) throw new Error("child.scrollTo failed");
        child.scrollBy(5, 10);
        if (child.scrollLeft !== 15 || child.scrollTop !== 30) throw new Error("child.scrollBy failed");
        child.scroll({ left: 0, top: 0 });
        if (child.scrollLeft !== 0 || child.scrollTop !== 0) throw new Error("child.scroll failed");

        // 6. element.scrollIntoView()
        target.scrollIntoView();
        if (window.scrollY !== 400) throw new Error("scrollIntoView should update window.scrollY to 400, got: " + window.scrollY);
        target.scrollIntoView({ behavior: 'smooth', block: 'start' });

        // 7. window.getComputedStyle()
        var style = window.getComputedStyle(child);
        if (!(style instanceof CSSStyleDeclaration)) throw new Error("style should be instanceof CSSStyleDeclaration");
        if (style.color !== "red" && style.color !== "rgb(255, 0, 0)") throw new Error("style.color should be red or rgb(255, 0, 0), got: " + style.color);
        if (style.fontSize !== "16px") throw new Error("style.fontSize should be 16px, got: " + style.fontSize);
        if (style.getPropertyValue("font-size") !== "16px") throw new Error("style.getPropertyValue failed");
        if (style.getPropertyValue("color") !== "red" && style.getPropertyValue("color") !== "rgb(255, 0, 0)") throw new Error("style.getPropertyValue color failed");
        if (style.width !== "100px") throw new Error("computed width should be 100px from layout, got: " + style.width);
        if (style.height !== "80px") throw new Error("computed height should be 80px from layout, got: " + style.height);
        if (style.length === 0) throw new Error("style.length should be > 0");
        if (!style.item(0)) throw new Error("style.item(0) should be non-empty");
        var styleProps = Array.from(style);
        if (styleProps.indexOf("color") === -1) throw new Error("CSSStyleDeclaration iterable should include 'color'");

        // 8. window.scrollTo(), window.scrollBy(), window.scroll() + scroll event
        window.scrollEventCount = 0;
        window.addEventListener("scroll", function() {
            window.scrollEventCount++;
        });
        window.scrollTo(50, 100);
        if (window.scrollX !== 50 || window.scrollY !== 100) throw new Error("window.scrollTo failed");
        window.scrollBy(10, 20);
        if (window.scrollX !== 60 || window.scrollY !== 120) throw new Error("window.scrollBy failed");
        window.scroll({ left: 30, top: 70 });
        if (window.scrollX !== 30 || window.scrollY !== 70) throw new Error("window.scroll failed");
        if (window.scrollEventCount < 3) throw new Error("scroll events should have fired: " + window.scrollEventCount);

        // 9. window.pageXOffset / window.pageYOffset / window.scrollX / window.scrollY
        if (window.pageXOffset !== window.scrollX || window.pageXOffset !== 30) throw new Error("window.pageXOffset mismatch");
        if (window.pageYOffset !== window.scrollY || window.pageYOffset !== 70) throw new Error("window.pageYOffset mismatch");

        // 10. document.elementFromPoint() & document.elementsFromPoint()
        var topEl = document.elementFromPoint(75, 95);
        if (topEl !== child) throw new Error("elementFromPoint should return child, got: " + (topEl ? topEl.tagName : topEl));
        var allEls = document.elementsFromPoint(75, 95);
        if (allEls.length < 2) throw new Error("elementsFromPoint should return at least 2 elements");
        if (allEls[0] !== child) throw new Error("first elementsFromPoint element should be child");
        if (allEls[1] !== parent) throw new Error("second elementsFromPoint element should be parent");

        // 11. ResizeObserver
        window.roFired = false;
        window.roTarget = null;
        window.roWidth = 0;
        var ro = new ResizeObserver(function(entries) {
            window.roFired = true;
            if (entries.length > 0) {
                window.roTarget = entries[0].target;
                window.roWidth = entries[0].contentRect.width;
            }
        });
        ro.observe(child);

        // 12. IntersectionObserver
        window.ioFired = false;
        window.ioTarget = null;
        window.ioIntersecting = false;
        var io = new IntersectionObserver(function(entries) {
            window.ioFired = true;
            if (entries.length > 0) {
                window.ioTarget = entries[0].target;
                window.ioIntersecting = entries[0].isIntersecting;
            }
        }, { threshold: [0, 1] });
        io.observe(child);

        // 13. MutationObserver
        window.moRecords = [];
        var mo = new MutationObserver(function(records) {
            window.moRecords.push.apply(window.moRecords, records);
        });
        mo.observe(parent, { childList: true, attributes: true, subtree: true, characterData: true });

        var newSpan = document.createElement("span");
        parent.appendChild(newSpan);
        parent.setAttribute("data-test", "val");
        newSpan.textContent = "Hi";

        // Test takeRecords() synchronously
        var syncRecords = mo.takeRecords();
        if (syncRecords.length === 0) throw new Error("MutationObserver.takeRecords() should have records");
        var hasChildList = syncRecords.some(function(r) { return r.type === "childList"; });
        var hasAttributes = syncRecords.some(function(r) { return r.type === "attributes" && r.attributeName === "data-test"; });
        var hasCharData = syncRecords.some(function(r) { return r.type === "characterData"; });
        if (!hasChildList) throw new Error("Missing childList mutation record");
        if (!hasAttributes) throw new Error("Missing attributes mutation record");
        if (!hasCharData) throw new Error("Missing characterData mutation record");

        // Make one more mutation to test microtask delivery
        parent.setAttribute("data-async", "yes");

        // 14. Lazy loading with IntersectionObserver
        window.lazyImg = document.createElement("img");
        window.lazyImg.setAttribute("data-src", "https://example.com/photo.jpg");
        window.lazyImg.loading = "lazy";
        parent.appendChild(window.lazyImg);
        if (window.lazyImg.loading !== "lazy") throw new Error("lazyImg.loading should be lazy");
    "##;

    if let Err(e) = rt.execute_script(script) {
        panic!("test_dom_measurement_and_layout script failed: {}", e);
    }

    // Verify async callbacks scheduled via microtasks (run_jobs was called at the end of execute_script)
    let verify_observers = r#"
        if (!window.roFired) throw new Error("ResizeObserver callback was not called");
        if (window.roTarget !== child) throw new Error("ResizeObserver target mismatch");
        if (window.roWidth !== 100) throw new Error("ResizeObserver width mismatch: " + window.roWidth);

        if (!window.ioFired) throw new Error("IntersectionObserver callback was not called");
        if (window.ioTarget !== child) throw new Error("IntersectionObserver target mismatch");
        if (!window.ioIntersecting) throw new Error("IntersectionObserver isIntersecting should be true");

        if (window.moRecords.length === 0) throw new Error("MutationObserver async callback was not called");
        var asyncAttr = window.moRecords.some(function(r) { return r.attributeName === "data-async"; });
        if (!asyncAttr) throw new Error("MutationObserver did not deliver data-async record");

        if (!window.lazyImg._lazyLoaded) throw new Error("lazyImg._lazyLoaded should be true");
        if (window.lazyImg.getAttribute("src") !== "https://example.com/photo.jpg") throw new Error("lazyImg src should be populated from data-src");
    "#;

    if let Err(e) = rt.execute_script(verify_observers) {
        panic!(
            "test_dom_measurement_and_layout verify_observers failed: {}",
            e
        );
    }
}

#[test]
fn test_audio_context_and_offscreen_canvas() {
    let html = "<!DOCTYPE html><html><body><canvas id='c' width='200' height='100'></canvas></body></html>";
    let mut rt = JsRuntime::new(parse_html(html), 800.0, 600.0);

    let script = r#"
        (function() {
            // 1. AudioContext
            if (typeof AudioContext !== 'function') throw new Error("AudioContext missing");
            var ctx = new AudioContext({ sampleRate: 48000 });
            if (ctx.sampleRate !== 48000) throw new Error("AudioContext sampleRate mismatch: " + ctx.sampleRate);
            if (ctx.state !== 'running') throw new Error("AudioContext initial state mismatch: " + ctx.state);

            var osc = ctx.createOscillator();
            osc.type = 'sawtooth';
            osc.frequency.setValueAtTime(880, 0);
            osc.start();
            osc.stop();

            var gain = ctx.createGain();
            gain.gain.value = 0.5;
            osc.connect(gain);
            gain.connect(ctx.destination);

            var buf = ctx.createBuffer(2, 512, 48000);
            if (buf.numberOfChannels !== 2 || buf.length !== 512) {
                throw new Error("AudioBuffer dimensions mismatch");
            }
            var ch0 = buf.getChannelData(0);
            ch0[0] = 0.75;
            if (ch0[0] !== 0.75) throw new Error("Channel data write failed");

            // 2. OffscreenCanvas
            if (typeof OffscreenCanvas !== 'function') throw new Error("OffscreenCanvas missing");
            var off = new OffscreenCanvas(400, 300);
            if (off.width !== 400 || off.height !== 300) throw new Error("OffscreenCanvas size mismatch");
            var c2d = off.getContext('2d');
            if (!c2d) throw new Error("OffscreenCanvas 2d context missing");
            c2d.fillStyle = '#ff0000';
            c2d.fillRect(0, 0, 100, 100);

            var bmp = off.transferToImageBitmap();
            if (bmp.width !== 400 || bmp.height !== 300) throw new Error("ImageBitmap size mismatch");

            var canvasEl = document.getElementById('c');
            var transferred = canvasEl.transferControlToOffscreen();
            if (transferred.width !== 200 || transferred.height !== 100) {
                throw new Error("transferControlToOffscreen size mismatch");
            }

            return "ok";
        })()
    "#;

    let res = rt
        .eval(script)
        .expect("eval audio and offscreen canvas test");
    assert_eq!(res, "ok");
}
