// WPT Suite 7: High-Performance Computing & Media (WebCodecs, OPFS, WebTransport, SharedArrayBuffer/Atomics)

promise_test(async function() {
    assert_true(typeof VideoFrame === "function", "VideoFrame constructor should exist");
    assert_true(typeof VideoDecoder === "function", "VideoDecoder constructor should exist");

    const width = 64;
    const height = 48;
    const rawPixels = new Uint8Array(width * height * 4);
    rawPixels[0] = 200; // R
    rawPixels[1] = 100; // G

    const frame = new VideoFrame(rawPixels, {
        format: "RGBA",
        codedWidth: width,
        codedHeight: height,
        timestamp: 1000
    });

    assert_equals(frame.codedWidth, 64, "VideoFrame codedWidth");
    assert_equals(frame.codedHeight, 48, "VideoFrame codedHeight");
    assert_equals(frame.format, "RGBA", "VideoFrame format");

    const copyBuf = new Uint8Array(frame.allocationSize());
    await frame.copyTo(copyBuf);
    assert_equals(copyBuf[0], 200, "Pixel R channel preserved");
    assert_equals(copyBuf[1], 100, "Pixel G channel preserved");

    frame.close();
}, "WPT Computing/Media: WebCodecs VideoFrame allocation and pixel copying");

test(function() {
    assert_true(typeof AudioData === "function", "AudioData constructor should exist");
    const sampleRate = 48000;
    const frames = 512;
    const rawAudio = new Float32Array(frames * 2);
    rawAudio[0] = 0.75;

    const audioData = new AudioData({
        format: "f32",
        sampleRate: sampleRate,
        numberOfFrames: frames,
        numberOfChannels: 2,
        timestamp: 500,
        data: rawAudio.buffer
    });

    assert_equals(audioData.sampleRate, 48000, "AudioData sampleRate");
    assert_equals(audioData.numberOfFrames, 512, "AudioData numberOfFrames");
    assert_equals(audioData.numberOfChannels, 2, "AudioData numberOfChannels");

    const outBuf = new Float32Array(frames * 2);
    audioData.copyTo(outBuf);
    assert_approx_equals(outBuf[0], 0.75, 1e-5, "Preserved audio sample amplitude");
    audioData.close();
}, "WPT Computing/Media: WebCodecs AudioData sample buffers and channel layout");

promise_test(async function() {
    assert_true(typeof navigator.storage !== "undefined", "navigator.storage should be defined");
    assert_true(typeof navigator.storage.getDirectory === "function", "storage.getDirectory should be a function");

    const root = await navigator.storage.getDirectory();
    assert_equals(root.kind, "directory", "OPFS root handle kind");

    const fileHandle = await root.getFileHandle("wpt_fast.bin", { create: true });
    assert_equals(fileHandle.kind, "file", "OPFS file handle kind");

    const syncAccess = await fileHandle.createSyncAccessHandle();
    const dataToWrite = new Uint8Array([10, 20, 30, 40]);
    const written = syncAccess.write(dataToWrite, { at: 0 });
    assert_equals(written, 4, "Bytes written to sync access handle");
    assert_equals(syncAccess.getSize(), 4, "File size after write");

    const readTarget = new Uint8Array(4);
    const read = syncAccess.read(readTarget, { at: 0 });
    assert_equals(read, 4, "Bytes read from sync access handle");
    assert_array_equals(Array.from(readTarget), [10, 20, 30, 40], "Read content matches written content");

    syncAccess.close();
}, "WPT Computing/Media: OPFS FileSystemSyncAccessHandle synchronous fast-path");

promise_test(async function() {
    assert_true(typeof WebTransport === "function", "WebTransport constructor should exist");

    let invalidRejected = false;
    try {
        new WebTransport("http://insecure.example.com");
    } catch(e) {
        invalidRejected = true;
    }
    assert_true(invalidRejected, "WebTransport must reject non-https/wts URL");

    const transport = new WebTransport("https://transport.example.com:4433/wpt");
    assert_true(transport.datagrams !== null, "transport.datagrams exists");
    assert_equals(transport.datagrams.maxDatagramSize, 1232, "maxDatagramSize standard QUIC MTU");

    await transport.ready;
    transport.close({ closeCode: 0, reason: "WPT clean close" });
    const closedInfo = await transport.closed;
    assert_equals(closedInfo.closeCode, 0, "Closed code matches");
}, "WPT Computing/Media: WebTransport QUIC transport lifecycle and datagrams");

test(function() {
    assert_true(typeof SharedArrayBuffer === "function", "SharedArrayBuffer should be defined");
    assert_true(typeof Atomics === "object", "Atomics object should be defined");

    const sab = new SharedArrayBuffer(16);
    const i32 = new Int32Array(sab);

    Atomics.store(i32, 0, 50);
    assert_equals(Atomics.load(i32, 0), 50, "Atomics.store / load");

    const old = Atomics.add(i32, 0, 25);
    assert_equals(old, 50, "Atomics.add returns old value");
    assert_equals(Atomics.load(i32, 0), 75, "Atomics.add updates value");

    const exOld = Atomics.exchange(i32, 0, 100);
    assert_equals(exOld, 75, "Atomics.exchange old value");
    assert_equals(Atomics.load(i32, 0), 100, "Atomics.exchange new value");

    const ce = Atomics.compareExchange(i32, 0, 100, 200);
    assert_equals(ce, 100, "Atomics.compareExchange expected match");
    assert_equals(Atomics.load(i32, 0), 200, "Atomics.compareExchange replacement");

    assert_true(Atomics.isLockFree(4), "Atomics.isLockFree(4) should be true");
}, "WPT Computing/Media: SharedArrayBuffer and atomic memory operations");

test(function() {
    assert_true(typeof crossOriginIsolated !== "undefined", "crossOriginIsolated global must be present");
    // Verify COOP/COEP isolation gate
    assert_false(crossOriginIsolated, "Default environment is not cross-origin isolated");
    const isolated = globalThis.__setCrossOriginIsolationHeaders("same-origin", "require-corp");
    assert_true(isolated, "COOP same-origin + COEP require-corp yields true");
    assert_true(crossOriginIsolated, "crossOriginIsolated matches true");

    globalThis.__setCrossOriginIsolationHeaders("unsafe-none", "require-corp");
    assert_false(crossOriginIsolated, "Mismatched headers resets crossOriginIsolated to false");
}, "WPT Computing/Media: crossOriginIsolated COOP & COEP security gate");
