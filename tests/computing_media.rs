//! High-Performance Computing & Media Parity Test Suite.
//!
//! Validates:
//! - WebCodecs (VideoFrame, AudioData, EncodedVideoChunk, EncodedAudioChunk, VideoDecoder, AudioDecoder, VideoEncoder, AudioEncoder)
//! - 5.2 Origin Private File System (OPFS: navigator.storage.getDirectory, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemWritableFileStream, FileSystemSyncAccessHandle in-place binary fast-path)
//! - 5.3 WebTransport (HTTP/3 & QUIC: WebTransport, ready, closed, datagrams readable/writable, bidi/uni streams, stats, error handling)
//! - 5.4 Multi-Threading & Shared Memory (SharedArrayBuffer, Atomics load/store/add/sub/and/or/xor/exchange/compareExchange/isLockFree, crossOriginIsolated COOP/COEP gate)

use mango_html::parse_html;
use mango_js::JsRuntime;

#[test]
fn test_webcodecs_video_and_audio_frame_primitives() {
    let html = r#"<!DOCTYPE html><html><body><canvas id="c" width="320" height="240"></canvas></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        globalThis.testPassed = false;

        // 1. VideoFrame from buffer
        const width = 160;
        const height = 120;
        const rgbaBuf = new Uint8Array(width * height * 4);
        for (let i = 0; i < rgbaBuf.length; i += 4) {
            rgbaBuf[i] = 255;     // R
            rgbaBuf[i + 1] = 128; // G
            rgbaBuf[i + 2] = 64;  // B
            rgbaBuf[i + 3] = 255; // A
        }

        const vFrame = new VideoFrame(rgbaBuf, {
            format: 'RGBA',
            codedWidth: width,
            codedHeight: height,
            timestamp: 1000000,
            duration: 33333
        });

        if (vFrame.format !== 'RGBA') throw new Error("vFrame.format mismatch: " + vFrame.format);
        if (vFrame.codedWidth !== 160 || vFrame.codedHeight !== 120) throw new Error("Coded dimensions mismatch");
        if (vFrame.timestamp !== 1000000) throw new Error("Timestamp mismatch: " + vFrame.timestamp);
        if (vFrame.duration !== 33333) throw new Error("Duration mismatch: " + vFrame.duration);
        if (vFrame.displayWidth !== 160 || vFrame.displayHeight !== 120) throw new Error("Display dimensions mismatch");
        if (!vFrame.colorSpace || vFrame.colorSpace.primaries !== 'bt709') throw new Error("colorSpace invalid");

        const allocSize = vFrame.allocationSize();
        if (allocSize !== width * height * 4) throw new Error("allocationSize mismatch: " + allocSize);

        const destBuf = new Uint8Array(allocSize);
        vFrame.copyTo(destBuf).then(layout => {
            if (destBuf[0] !== 255 || destBuf[1] !== 128 || destBuf[2] !== 64) {
                throw new Error("Pixel copy failed");
            }
            if (!layout || !layout[0] || layout[0].stride !== width * 4) {
                throw new Error("Layout stride mismatch");
            }
        });

        const clonedFrame = vFrame.clone();
        if (clonedFrame.codedWidth !== 160) throw new Error("Cloned frame width mismatch");
        vFrame.close();

        let threwOnClosed = false;
        try {
            vFrame.allocationSize();
        } catch (e) {
            threwOnClosed = true;
        }
        if (!threwOnClosed) throw new Error("Closed VideoFrame should throw on method call");

        // 2. VideoFrame from canvas element
        const canvas = document.getElementById('c');
        const canvasFrame = new VideoFrame(canvas, { timestamp: 500000 });
        if (canvasFrame.codedWidth !== 320 || canvasFrame.codedHeight !== 240) {
            throw new Error("Canvas VideoFrame dimensions mismatch");
        }
        canvasFrame.close();

        // 3. AudioData primitive
        const sampleRate = 48000;
        const frames = 1024;
        const channels = 2;
        const audioBuf = new Float32Array(frames * channels);
        for (let i = 0; i < audioBuf.length; i++) {
            audioBuf[i] = Math.sin(i * 0.1);
        }

        const aData = new AudioData({
            format: 'f32',
            sampleRate: sampleRate,
            numberOfFrames: frames,
            numberOfChannels: channels,
            timestamp: 2000000,
            data: audioBuf.buffer
        });

        if (aData.format !== 'f32') throw new Error("AudioData format mismatch");
        if (aData.sampleRate !== 48000) throw new Error("sampleRate mismatch");
        if (aData.numberOfFrames !== 1024) throw new Error("numberOfFrames mismatch");
        if (aData.numberOfChannels !== 2) throw new Error("numberOfChannels mismatch");
        if (aData.timestamp !== 2000000) throw new Error("AudioData timestamp mismatch");
        if (aData.duration <= 0) throw new Error("AudioData duration should be positive");

        const aAlloc = aData.allocationSize();
        if (aAlloc !== frames * channels * 4) throw new Error("Audio allocation size mismatch: " + aAlloc);

        const aDest = new Float32Array(frames * channels);
        aData.copyTo(aDest);
        if (Math.abs(aDest[0] - audioBuf[0]) > 1e-6) throw new Error("Audio copyTo data mismatch");

        const aClone = aData.clone();
        if (aClone.numberOfFrames !== 1024) throw new Error("Cloned audio data frames mismatch");
        aData.close();

        // 4. EncodedVideoChunk & EncodedAudioChunk
        const chunkData = new Uint8Array([0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0x00, 0x1f]);
        const vChunk = new EncodedVideoChunk({
            type: 'key',
            timestamp: 3000000,
            duration: 33333,
            data: chunkData
        });
        if (vChunk.type !== 'key' || vChunk.timestamp !== 3000000 || vChunk.byteLength !== 8) {
            throw new Error("EncodedVideoChunk properties mismatch");
        }
        const vChunkCopy = new Uint8Array(8);
        vChunk.copyTo(vChunkCopy);
        if (vChunkCopy[4] !== 0x67) throw new Error("EncodedVideoChunk copyTo failed");

        const aChunk = new EncodedAudioChunk({
            type: 'delta',
            timestamp: 4000000,
            duration: 21333,
            data: new Uint8Array([0xff, 0xf1, 0x50, 0x80])
        });
        if (aChunk.type !== 'delta' || aChunk.byteLength !== 4) {
            throw new Error("EncodedAudioChunk properties mismatch");
        }

        globalThis.testPassed = true;
    "#;

    rt.execute_script(script).expect("WebCodecs frame primitives test failed");
    rt.run_jobs();

    let verify = r#"
        if (!globalThis.testPassed) throw new Error("WebCodecs primitives test did not pass");
    "#;
    rt.execute_script(verify).unwrap();
}

#[test]
fn test_webcodecs_decoders_and_encoders() {
    let html = r#"<!DOCTYPE html><html><body></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        globalThis.videoDecoded = false;
        globalThis.audioDecoded = false;
        globalThis.videoEncoded = false;
        globalThis.audioEncoded = false;

        // 1. VideoDecoder
        VideoDecoder.isConfigSupported({ codec: 'vp8' }).then(res => {
            if (!res.supported) throw new Error("vp8 should be supported");
        });

        const vDec = new VideoDecoder({
            output: (frame) => {
                if (frame && frame.codedWidth === 320 && frame.codedHeight === 240) {
                    globalThis.videoDecoded = true;
                }
                frame.close();
            },
            error: (e) => { throw e; }
        });

        vDec.configure({ codec: 'vp8', codedWidth: 320, codedHeight: 240 });
        if (vDec.state !== 'configured') throw new Error("VideoDecoder state should be configured");

        const rawVideoChunk = new EncodedVideoChunk({
            type: 'key',
            timestamp: 1000,
            duration: 33333,
            data: new Uint8Array([0x00, 0x00, 0x01])
        });
        vDec.decode(rawVideoChunk);
        vDec.flush().then(() => {
            vDec.close();
            if (vDec.state !== 'closed') throw new Error("VideoDecoder should be closed");
        });

        // 2. AudioDecoder
        AudioDecoder.isConfigSupported({ codec: 'opus' }).then(res => {
            if (!res.supported) throw new Error("opus should be supported");
        });

        const aDec = new AudioDecoder({
            output: (data) => {
                if (data && data.sampleRate === 44100 && data.numberOfChannels === 2) {
                    globalThis.audioDecoded = true;
                }
                data.close();
            },
            error: (e) => { throw e; }
        });

        aDec.configure({ codec: 'opus', sampleRate: 44100, numberOfChannels: 2 });
        const rawAudioChunk = new EncodedAudioChunk({
            type: 'key',
            timestamp: 2000,
            duration: 20000,
            data: new Uint8Array([0x4f, 0x70, 0x75, 0x73])
        });
        aDec.decode(rawAudioChunk);
        aDec.flush().then(() => {
            aDec.close();
        });

        // 3. VideoEncoder
        VideoEncoder.isConfigSupported({ codec: 'avc1.42001E' }).then(res => {
            if (!res.supported) throw new Error("avc1 should be supported");
        });

        const vEnc = new VideoEncoder({
            output: (chunk, metadata) => {
                if (chunk && chunk.type === 'key') {
                    globalThis.videoEncoded = true;
                }
            },
            error: (e) => { throw e; }
        });
        vEnc.configure({ codec: 'avc1.42001E', width: 640, height: 480, bitrate: 2000000 });
        const inputFrame = new VideoFrame(new Uint8Array(640 * 480 * 4), {
            format: 'RGBA',
            codedWidth: 640,
            codedHeight: 480,
            timestamp: 3000
        });
        vEnc.encode(inputFrame, { keyFrame: true });
        inputFrame.close();
        vEnc.flush().then(() => {
            vEnc.close();
        });

        // 4. AudioEncoder
        AudioEncoder.isConfigSupported({ codec: 'mp4a.40.2' }).then(res => {
            if (!res.supported) throw new Error("aac should be supported");
        });

        const aEnc = new AudioEncoder({
            output: (chunk, metadata) => {
                if (chunk && chunk.byteLength > 0) {
                    globalThis.audioEncoded = true;
                }
            },
            error: (e) => { throw e; }
        });
        aEnc.configure({ codec: 'mp4a.40.2', sampleRate: 48000, numberOfChannels: 2 });
        const inputAudio = new AudioData({
            format: 'f32',
            sampleRate: 48000,
            numberOfFrames: 1024,
            numberOfChannels: 2,
            timestamp: 4000,
            data: new Float32Array(1024 * 2).buffer
        });
        aEnc.encode(inputAudio);
        inputAudio.close();
        aEnc.flush().then(() => {
            aEnc.close();
        });
    "#;

    rt.execute_script(script).expect("WebCodecs decoders/encoders failed");
    rt.run_jobs();

    let verify = r#"
        if (!globalThis.videoDecoded) throw new Error("VideoDecoder output was not received");
        if (!globalThis.audioDecoded) throw new Error("AudioDecoder output was not received");
        if (!globalThis.videoEncoded) throw new Error("VideoEncoder output was not received");
        if (!globalThis.audioEncoded) throw new Error("AudioEncoder output was not received");
    "#;
    rt.execute_script(verify).unwrap();
}

#[test]
fn test_opfs_directory_and_file_handles() {
    let html = r#"<!DOCTYPE html><html><body></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        globalThis.opfsSuccess = false;

        async function runOpfsTest() {
            if (!navigator.storage || typeof navigator.storage.getDirectory !== 'function') {
                throw new Error("navigator.storage.getDirectory must be defined");
            }

            const root = await navigator.storage.getDirectory();
            if (root.kind !== 'directory') throw new Error("Root kind should be directory");

            // Create subdirectory
            const subDir = await root.getDirectoryHandle('documents', { create: true });
            if (subDir.kind !== 'directory' || subDir.name !== 'documents') {
                throw new Error("Subdirectory handle invalid");
            }

            // Create file inside subdirectory
            const fileHandle = await subDir.getFileHandle('notes.txt', { create: true });
            if (fileHandle.kind !== 'file' || fileHandle.name !== 'notes.txt') {
                throw new Error("File handle invalid");
            }

            // Write into file
            const writable = await fileHandle.createWritable();
            await writable.write("Hello, Origin Private File System!");
            await writable.seek(7);
            await writable.write("OPFS");
            await writable.close();

            // Read back via getFile()
            const file = await fileHandle.getFile();
            if (file.name !== 'notes.txt') throw new Error("File name mismatch");
            const text = await file.text();
            if (text !== "Hello, OPFSin Private File System!") {
                throw new Error("Unexpected file content: " + text);
            }

            // Resolve path
            const path = await root.resolve(fileHandle);
            if (!path || path.length !== 2 || path[0] !== 'documents' || path[1] !== 'notes.txt') {
                throw new Error("resolve path failed: " + JSON.stringify(path));
            }

            // Verify async iterator entries
            const entries = [];
            for await (const [name, handle] of subDir.entries()) {
                entries.push({ name, kind: handle.kind });
            }
            if (entries.length !== 1 || entries[0].name !== 'notes.txt' || entries[0].kind !== 'file') {
                throw new Error("Async iteration mismatch: " + JSON.stringify(entries));
            }

            // Verify isSameEntry
            const sameFile = await subDir.getFileHandle('notes.txt');
            const isSame = await fileHandle.isSameEntry(sameFile);
            if (!isSame) throw new Error("isSameEntry should return true for same file");

            // Remove entry
            await subDir.removeEntry('notes.txt');
            let notFound = false;
            try {
                await subDir.getFileHandle('notes.txt', { create: false });
            } catch (e) {
                notFound = true;
            }
            if (!notFound) throw new Error("Removed file should throw NotFoundError");

            globalThis.opfsSuccess = true;
        }

        runOpfsTest().catch(e => {
            console.error("OPFS ERROR:", e && (e.stack || e.message || e));
        });
    "#;

    rt.execute_script(script).expect("OPFS test script failed");
    rt.run_jobs();

    let verify = r#"
        if (!globalThis.opfsSuccess) throw new Error("OPFS async operations did not complete successfully");
    "#;
    rt.execute_script(verify).unwrap();
}

#[test]
fn test_opfs_sync_access_handle_fast_path() {
    let html = r#"<!DOCTYPE html><html><body></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        globalThis.syncSuccess = false;

        async function runSyncTest() {
            const root = await navigator.storage.getDirectory();
            const dbHandle = await root.getFileHandle('sqlite.db', { create: true });

            // Open synchronous access handle (Wasm / SQLite in-place fast-path)
            const syncHandle = await dbHandle.createSyncAccessHandle();

            // Write 16-byte header
            const header = new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
            const written = syncHandle.write(header, { at: 0 });
            if (written !== 16) throw new Error("Bytes written mismatch: " + written);

            if (syncHandle.getSize() !== 16) throw new Error("Size mismatch: " + syncHandle.getSize());

            // In-place read
            const readBuf = new Uint8Array(16);
            const readCount = syncHandle.read(readBuf, { at: 0 });
            if (readCount !== 16) throw new Error("Read count mismatch: " + readCount);
            for (let i = 0; i < 16; i++) {
                if (readBuf[i] !== i + 1) throw new Error("Data byte mismatch at " + i);
            }

            // Truncate
            syncHandle.truncate(8);
            if (syncHandle.getSize() !== 8) throw new Error("Truncated size mismatch");

            // Mutual exclusion test: opening another handle while locked must reject
            let lockPrevented = false;
            try {
                await dbHandle.createSyncAccessHandle();
            } catch (e) {
                lockPrevented = true;
            }
            if (!lockPrevented) throw new Error("Should prevent second access handle while locked");

            syncHandle.flush();
            syncHandle.close();

            // After close, second handle should now succeed
            const secondHandle = await dbHandle.createSyncAccessHandle();
            if (secondHandle.getSize() !== 8) throw new Error("Size after reopen mismatch");
            secondHandle.close();

            globalThis.syncSuccess = true;
        }

        runSyncTest();
    "#;

    rt.execute_script(script).expect("OPFS sync access handle script failed");
    rt.run_jobs();

    let verify = r#"
        if (!globalThis.syncSuccess) throw new Error("OPFS sync access test failed");
    "#;
    rt.execute_script(verify).unwrap();
}

#[test]
fn test_webtransport_quic_lifecycle_datagrams_and_streams() {
    let html = r#"<!DOCTYPE html><html><body></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    let script = r#"
        globalThis.transportPassed = false;

        // 1. URL scheme validation
        let invalidSchemeThrown = false;
        try {
            new WebTransport("http://example.com/wt");
        } catch (e) {
            invalidSchemeThrown = true;
        }
        if (!invalidSchemeThrown) throw new Error("WebTransport must reject non-https/wts URLs");

        let fragmentThrown = false;
        try {
            new WebTransport("https://example.com/wt#fragment");
        } catch (e) {
            fragmentThrown = true;
        }
        if (!fragmentThrown) throw new Error("WebTransport must reject URL with hash fragment");

        // 2. Valid connection lifecycle
        const wt = new WebTransport("https://quic.example.com:4433/transport", {
            congestionControl: "low-latency"
        });

        if (wt.congestionControl !== "low-latency") throw new Error("congestionControl mismatch");
        if (wt.reliability !== "supports-unreliable") throw new Error("reliability mismatch");

        // 3. Datagrams
        if (!wt.datagrams || typeof wt.datagrams.maxDatagramSize !== 'number') {
            throw new Error("datagrams interface missing");
        }
        if (wt.datagrams.maxDatagramSize !== 1232) {
            throw new Error("maxDatagramSize mismatch: " + wt.datagrams.maxDatagramSize);
        }

        // Test sending datagram
        const writer = wt.datagrams.writable.getWriter();
        writer.write(new Uint8Array([1, 2, 3, 4])).then(() => {
            writer.releaseLock();
        });

        // 4. Streams creation
        wt.createBidirectionalStream().then(bidi => {
            if (!bidi.readable || !bidi.writable) throw new Error("Bidirectional stream missing endpoints");
        });

        wt.createUnidirectionalStream().then(uni => {
            if (!uni || typeof uni.getWriter !== 'function') throw new Error("Unidirectional stream invalid");
        });

        // 5. Ready and stats
        wt.ready.then(async () => {
            const stats = await wt.getStats();
            if (typeof stats.smoothedRtt !== 'number' || stats.smoothedRtt <= 0) {
                throw new Error("Stats smoothedRtt invalid");
            }

            // Close connection
            wt.close({ closeCode: 200, reason: "Session complete" });
        });

        wt.closed.then(closeInfo => {
            if (closeInfo.closeCode !== 200 || closeInfo.reason !== "Session complete") {
                throw new Error("Closed promise resolution mismatch: " + JSON.stringify(closeInfo));
            }
            globalThis.transportPassed = true;
        });
    "#;

    rt.execute_script(script).expect("WebTransport script failed");
    rt.run_jobs();

    let verify = r#"
        if (!globalThis.transportPassed) throw new Error("WebTransport lifecycle test did not pass");
    "#;
    rt.execute_script(verify).unwrap();
}

#[test]
fn test_multithreading_shared_memory_atomics_and_coi() {
    let html = r#"<!DOCTYPE html><html><body></body></html>"#;
    let doc = parse_html(html);
    let mut rt = JsRuntime::new(doc, 800.0, 600.0);

    // 1. Verify SharedArrayBuffer and full Atomics primitives
    let script = r#"
        if (typeof SharedArrayBuffer !== 'function') throw new Error("SharedArrayBuffer must be available");
        if (typeof Atomics !== 'object') throw new Error("Atomics must be available");

        const sab = new SharedArrayBuffer(32);
        if (sab.byteLength !== 32) throw new Error("SharedArrayBuffer byteLength mismatch");

        const i32 = new Int32Array(sab);

        // Atomics.store and load
        Atomics.store(i32, 0, 100);
        if (Atomics.load(i32, 0) !== 100) throw new Error("Atomics store/load mismatch");

        // Atomics.add
        const oldAdd = Atomics.add(i32, 0, 25);
        if (oldAdd !== 100 || Atomics.load(i32, 0) !== 125) throw new Error("Atomics.add failed");

        // Atomics.sub
        const oldSub = Atomics.sub(i32, 0, 15);
        if (oldSub !== 125 || Atomics.load(i32, 0) !== 110) throw new Error("Atomics.sub failed");

        // Atomics.and
        Atomics.store(i32, 1, 0b1111);
        Atomics.and(i32, 1, 0b1010);
        if (Atomics.load(i32, 1) !== 0b1010) throw new Error("Atomics.and failed");

        // Atomics.or
        Atomics.or(i32, 1, 0b0101);
        if (Atomics.load(i32, 1) !== 0b1111) throw new Error("Atomics.or failed");

        // Atomics.xor
        Atomics.xor(i32, 1, 0b1100);
        if (Atomics.load(i32, 1) !== 0b0011) throw new Error("Atomics.xor failed");

        // Atomics.exchange
        const exOld = Atomics.exchange(i32, 0, 999);
        if (exOld !== 110 || Atomics.load(i32, 0) !== 999) throw new Error("Atomics.exchange failed");

        // Atomics.compareExchange
        const ceOld1 = Atomics.compareExchange(i32, 0, 500, 777); // expected != 999, no swap
        if (ceOld1 !== 999 || Atomics.load(i32, 0) !== 999) throw new Error("Atomics.compareExchange mismatch 1");
        const ceOld2 = Atomics.compareExchange(i32, 0, 999, 777); // expected == 999, swap
        if (ceOld2 !== 999 || Atomics.load(i32, 0) !== 777) throw new Error("Atomics.compareExchange mismatch 2");

        // Atomics.isLockFree
        if (!Atomics.isLockFree(4)) throw new Error("32-bit atomic operations should be lock-free");
    "#;
    rt.execute_script(script).expect("SharedArrayBuffer and Atomics test failed");

    // 2. Cross-Origin Isolation Gate
    assert!(!rt.is_cross_origin_isolated(), "Default runtime should not be cross-origin isolated");

    // In JS check window.crossOriginIsolated
    let check_initial = r#"
        if (window.crossOriginIsolated !== false || globalThis.crossOriginIsolated !== false) {
            throw new Error("crossOriginIsolated should initially be false");
        }
    "#;
    rt.execute_script(check_initial).unwrap();

    // Set invalid / mismatched COOP/COEP headers
    rt.set_cross_origin_isolation_headers("unsafe-none", "require-corp");
    assert!(!rt.is_cross_origin_isolated(), "Mismatched COOP header should prevent isolation");

    rt.set_cross_origin_isolation_headers("same-origin", "unsafe-none");
    assert!(!rt.is_cross_origin_isolated(), "Mismatched COEP header should prevent isolation");

    // Set matching COOP: same-origin and COEP: require-corp
    rt.set_cross_origin_isolation_headers("same-origin", "require-corp");
    assert!(rt.is_cross_origin_isolated(), "COOP same-origin + COEP require-corp must isolate");

    let check_isolated = r#"
        if (window.crossOriginIsolated !== true || globalThis.crossOriginIsolated !== true) {
            throw new Error("crossOriginIsolated should be true after valid COOP/COEP headers");
        }
    "#;
    rt.execute_script(check_isolated).unwrap();

    // Test COEP: credentialless as well
    rt.set_cross_origin_isolation_headers("same-origin", "credentialless");
    assert!(rt.is_cross_origin_isolated(), "COOP same-origin + COEP credentialless must isolate");

    // Direct toggle
    rt.set_cross_origin_isolated(false);
    assert!(!rt.is_cross_origin_isolated());
    rt.set_cross_origin_isolated(true);
    assert!(rt.is_cross_origin_isolated());
}
