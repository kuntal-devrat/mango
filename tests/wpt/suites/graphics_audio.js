// WPT Suite 6: Graphics & Audio (OffscreenCanvas, WebGL2, WebGPU, Web Audio)

promise_test(async function() {
    assert_true(typeof OffscreenCanvas === "function", "OffscreenCanvas constructor should be defined");
    const osc = new OffscreenCanvas(400, 300);
    assert_equals(osc.width, 400, "OffscreenCanvas width");
    assert_equals(osc.height, 300, "OffscreenCanvas height");

    const ctx = osc.getContext("2d");
    assert_true(ctx !== null, "getContext('2d') should return 2D context");
    assert_equals(ctx.canvas, osc, "Context canvas reference");

    const blob = await osc.convertToBlob({ type: "image/png" });
    assert_true(blob !== null, "convertToBlob should return Blob");
    assert_equals(blob.type, "image/png", "Blob mime type");

    const bitmap = osc.transferToImageBitmap();
    assert_equals(bitmap.width, 400, "ImageBitmap width");
    assert_equals(bitmap.height, 300, "ImageBitmap height");
}, "WPT Graphics: OffscreenCanvas 2D context, convertToBlob, and transferToImageBitmap");

test(function() {
    assert_true(typeof WebGL2RenderingContext === "function", "WebGL2RenderingContext should be defined");
    const osc = new OffscreenCanvas(300, 150);
    const gl = osc.getContext("webgl2");
    assert_true(gl !== null, "getContext('webgl2') should return WebGL2RenderingContext");

    const vao = gl.createVertexArray();
    assert_true(vao !== null, "createVertexArray should succeed");
    gl.bindVertexArray(vao);
    assert_true(gl.isVertexArray(vao), "isVertexArray should return true");
    gl.bindVertexArray(null);
}, "WPT Graphics: WebGL 2.0 Vertex Array Objects (VAO)");

promise_test(async function() {
    assert_true(typeof navigator.gpu !== "undefined", "navigator.gpu should be defined");
    assert_true(typeof navigator.gpu.requestAdapter === "function", "requestAdapter should be a function");

    const adapter = await navigator.gpu.requestAdapter();
    assert_true(adapter !== null, "requestAdapter should resolve to GPUAdapter");

    const device = await adapter.requestDevice();
    assert_true(device !== null, "requestDevice should resolve to GPUDevice");

    const shader = device.createShaderModule({
        code: `@vertex fn vs() -> @builtin(position) vec4f { return vec4f(0.0); }`
    });
    assert_true(shader !== null, "createShaderModule should succeed");
}, "WPT Graphics: WebGPU adapter, device, and shader module creation");

test(function() {
    assert_true(typeof AudioContext === "function", "AudioContext should be defined");
    const ctx = new AudioContext({ sampleRate: 48000 });
    assert_equals(ctx.sampleRate, 48000, "AudioContext sampleRate");
    assert_true(ctx.destination !== null, "AudioDestinationNode exists");

    const gain = ctx.createGain();
    assert_true(gain.gain !== null, "GainNode AudioParam exists");
    assert_equals(gain.gain.value, 1.0, "Default gain value");
    gain.gain.setValueAtTime(0.5, 0.1);
    assert_approx_equals(gain.gain.value, 0.5, 1e-4, "Automated gain value");

    const osc = ctx.createOscillator();
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start();
    osc.stop();
}, "WPT Audio: AudioContext graph creation, AudioParam automation, and node connections");
