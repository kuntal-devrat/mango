// WPT Suite 4: WHATWG Streams API (streams/*)

promise_test(async function() {
    assert_true(typeof ReadableStream === "function", "ReadableStream should be defined");

    let pullCount = 0;
    const rs = new ReadableStream({
        start(controller) {
            controller.enqueue("first");
            controller.enqueue("second");
        },
        pull(controller) {
            pullCount++;
            if (pullCount === 1) {
                controller.enqueue("third");
                controller.close();
            }
        }
    });

    const reader = rs.getReader();
    assert_true(typeof reader.read === "function", "reader.read should be a function");

    const r1 = await reader.read();
    assert_equals(r1.value, "first", "first chunk");
    assert_false(r1.done, "first chunk not done");

    const r2 = await reader.read();
    assert_equals(r2.value, "second", "second chunk");

    const r3 = await reader.read();
    assert_equals(r3.value, "third", "third chunk from pull");

    const r4 = await reader.read();
    assert_true(r4.done, "final read should be done");
}, "WPT Streams: ReadableStream enqueue, pull, and reader iteration");

promise_test(async function() {
    assert_true(typeof WritableStream === "function", "WritableStream should be defined");

    const writtenChunks = [];
    let isClosed = false;

    const ws = new WritableStream({
        write(chunk) {
            writtenChunks.push(chunk);
            return Promise.resolve();
        },
        close() {
            isClosed = true;
            return Promise.resolve();
        }
    });

    const writer = ws.getWriter();
    await writer.write("Alpha");
    await writer.write("Beta");
    await writer.close();

    assert_equals(writtenChunks.length, 2, "written chunks count");
    assert_equals(writtenChunks[0], "Alpha", "first written chunk");
    assert_equals(writtenChunks[1], "Beta", "second written chunk");
    assert_true(isClosed, "WritableStream sink close called");
}, "WPT Streams: WritableStream getWriter, write, and close");

promise_test(async function() {
    assert_true(typeof TransformStream === "function", "TransformStream should be defined");

    const ts = new TransformStream({
        transform(chunk, controller) {
            controller.enqueue(chunk.toUpperCase());
        }
    });

    const writer = ts.writable.getWriter();
    const reader = ts.readable.getReader();

    writer.write("mango");
    writer.close();

    const result = await reader.read();
    assert_equals(result.value, "MANGO", "TransformStream uppercase transform");
}, "WPT Streams: TransformStream transform pipeline");

promise_test(async function() {
    const rs = new ReadableStream({
        start(controller) {
            controller.enqueue(10);
            controller.enqueue(20);
            controller.close();
        }
    });

    const [branch1, branch2] = rs.tee();
    assert_true(branch1 instanceof ReadableStream, "tee branch1 is ReadableStream");
    assert_true(branch2 instanceof ReadableStream, "tee branch2 is ReadableStream");

    const reader1 = branch1.getReader();
    const reader2 = branch2.getReader();

    const c1_1 = await reader1.read();
    const c2_1 = await reader2.read();
    assert_equals(c1_1.value, 10, "branch 1 first value");
    assert_equals(c2_1.value, 10, "branch 2 first value");

    const c1_2 = await reader1.read();
    const c2_2 = await reader2.read();
    assert_equals(c1_2.value, 20, "branch 1 second value");
    assert_equals(c2_2.value, 20, "branch 2 second value");
}, "WPT Streams: ReadableStream.prototype.tee branching");
