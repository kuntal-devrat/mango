    // ── 20b. WHATWG Streams API ──────────────────────────────────────────────
    function ByteLengthQueuingStrategy(options) {
        this.highWaterMark = (options && options.highWaterMark !== undefined) ? Number(options.highWaterMark) : 0;
    }
    ByteLengthQueuingStrategy.prototype.size = function(chunk) {
        return (chunk && chunk.byteLength !== undefined) ? chunk.byteLength : 0;
    };
    globalThis.ByteLengthQueuingStrategy = ByteLengthQueuingStrategy;

    function CountQueuingStrategy(options) {
        this.highWaterMark = (options && options.highWaterMark !== undefined) ? Number(options.highWaterMark) : 0;
    }
    CountQueuingStrategy.prototype.size = function() {
        return 1;
    };
    globalThis.CountQueuingStrategy = CountQueuingStrategy;

    function ReadableStreamDefaultReader(stream) {
        if (!(stream instanceof ReadableStream)) {
            throw new TypeError("ReadableStreamDefaultReader requires a ReadableStream");
        }
        if (stream.locked) {
            throw new TypeError("ReadableStream is already locked");
        }
        this._stream = stream;
        stream._reader = this;
        this._pendingReads = [];
        var self = this;
        this.closed = new Promise(function(resolve, reject) {
            self._closedResolve = resolve;
            self._closedReject = reject;
        });
        if (stream._state === 'closed') {
            this._closedResolve();
        } else if (stream._state === 'errored') {
            this._closedReject(stream._storedError);
        }
    }
    ReadableStreamDefaultReader.prototype = {
        read: function() {
            var stream = this._stream;
            if (!stream) {
                return Promise.reject(new TypeError("Reader was released"));
            }
            stream._disturbed = true;
            if (stream._queue.length > 0) {
                var chunk = stream._queue.shift();
                return Promise.resolve({ value: chunk, done: false });
            }
            if (stream._state === 'closed') {
                return Promise.resolve({ value: undefined, done: true });
            }
            if (stream._state === 'errored') {
                return Promise.reject(stream._storedError);
            }
            var self = this;
            return new Promise(function(resolve, reject) {
                self._pendingReads.push({ resolve: resolve, reject: reject });
                if (typeof stream._underlyingSource.pull === 'function') {
                    try {
                        var p = stream._underlyingSource.pull(stream._controller);
                        if (p && typeof p.then === 'function') {
                            p.catch(function(err) { stream._controller.error(err); });
                        }
                    } catch(e) {
                        stream._controller.error(e);
                    }
                }
            });
        },
        releaseLock: function() {
            if (!this._stream) return;
            if (this._pendingReads.length > 0) {
                throw new TypeError("Cannot release lock with pending reads");
            }
            this._stream._reader = null;
            this._stream = null;
        },
        cancel: function(reason) {
            if (!this._stream) {
                return Promise.reject(new TypeError("Reader was released"));
            }
            return this._stream.cancel(reason);
        }
    };
    globalThis.ReadableStreamDefaultReader = ReadableStreamDefaultReader;

    function ReadableStream(underlyingSource, strategy) {
        underlyingSource = underlyingSource || {};
        strategy = strategy || {};
        this._state = 'readable';
        this._storedError = undefined;
        this._queue = [];
        this._reader = null;
        this._disturbed = false;

        var self = this;
        var controller = {
            get desiredSize() {
                if (self._state === 'errored') return null;
                if (self._state === 'closed') return 0;
                var hwm = (strategy && strategy.highWaterMark !== undefined) ? strategy.highWaterMark : 1;
                return Math.max(0, hwm - self._queue.length);
            },
            close: function() {
                if (self._state !== 'readable') return;
                self._state = 'closed';
                if (self._reader) {
                    while (self._reader._pendingReads.length > 0) {
                        var r = self._reader._pendingReads.shift();
                        r.resolve({ value: undefined, done: true });
                    }
                    if (self._reader._closedResolve) self._reader._closedResolve();
                }
            },
            enqueue: function(chunk) {
                if (self._state !== 'readable') {
                    throw new TypeError("Cannot enqueue chunk into non-readable stream");
                }
                if (self._reader && self._reader._pendingReads.length > 0) {
                    var r = self._reader._pendingReads.shift();
                    r.resolve({ value: chunk, done: false });
                } else {
                    self._queue.push(chunk);
                }
            },
            error: function(e) {
                if (self._state !== 'readable') return;
                self._state = 'errored';
                self._storedError = e;
                if (self._reader) {
                    while (self._reader._pendingReads.length > 0) {
                        var r = self._reader._pendingReads.shift();
                        r.reject(e);
                    }
                    if (self._reader._closedReject) self._reader._closedReject(e);
                }
            }
        };
        this._controller = controller;
        this._underlyingSource = underlyingSource;

        if (typeof underlyingSource.start === 'function') {
            try {
                var res = underlyingSource.start(controller);
                if (res && typeof res.then === 'function') {
                    res.catch(function(err) { controller.error(err); });
                }
            } catch(e) {
                controller.error(e);
            }
        }
    }
    Object.defineProperty(ReadableStream.prototype, 'locked', {
        get: function() { return this._reader !== null; },
        configurable: true,
        enumerable: true
    });
    ReadableStream.prototype.cancel = function(reason) {
        if (this._reader !== null) {
            return Promise.reject(new TypeError("Cannot cancel a locked stream"));
        }
        this._state = 'closed';
        this._storedError = reason;
        this._queue = [];
        if (typeof this._underlyingSource.cancel === 'function') {
            return Promise.resolve(this._underlyingSource.cancel(reason));
        }
        return Promise.resolve();
    };
    ReadableStream.prototype.getReader = function(options) {
        if (this._reader !== null) {
            throw new TypeError("ReadableStream is already locked");
        }
        return new ReadableStreamDefaultReader(this);
    };
    ReadableStream.prototype.pipeThrough = function(transform, options) {
        if (!transform || !transform.writable || !transform.readable) {
            throw new TypeError("transform must have writable and readable properties");
        }
        this.pipeTo(transform.writable, options);
        return transform.readable;
    };
    ReadableStream.prototype.pipeTo = function(destination, options) {
        var self = this;
        var reader = this.getReader();
        var writer = destination.getWriter();
        return new Promise(function(resolve, reject) {
            function pump() {
                reader.read().then(function(res) {
                    if (res.done) {
                        writer.close().then(resolve, reject);
                        return;
                    }
                    writer.write(res.value).then(pump, reject);
                }).catch(reject);
            }
            pump();
        });
    };
    ReadableStream.prototype.tee = function() {
        var self = this;
        var branch1Ctrl, branch2Ctrl;
        var b1 = new ReadableStream({
            start: function(c) { branch1Ctrl = c; }
        });
        var b2 = new ReadableStream({
            start: function(c) { branch2Ctrl = c; }
        });
        var reader = self.getReader();
        function pump() {
            reader.read().then(function(res) {
                if (res.done) {
                    branch1Ctrl.close();
                    branch2Ctrl.close();
                    return;
                }
                branch1Ctrl.enqueue(res.value);
                branch2Ctrl.enqueue(res.value);
                pump();
            }).catch(function(err) {
                branch1Ctrl.error(err);
                branch2Ctrl.error(err);
            });
        }
        pump();
        return [b1, b2];
    };
    if (typeof Symbol !== 'undefined' && Symbol.asyncIterator) {
        ReadableStream.prototype[Symbol.asyncIterator] = function() {
            var reader = this.getReader();
            return {
                next: function() {
                    return reader.read();
                },
                return: function(val) {
                    reader.releaseLock();
                    return Promise.resolve({ value: val, done: true });
                }
            };
        };
    }
    globalThis.ReadableStream = ReadableStream;

    function WritableStreamDefaultWriter(stream) {
        if (!(stream instanceof WritableStream)) {
            throw new TypeError("Expected WritableStream");
        }
        if (stream.locked) throw new TypeError("WritableStream is locked");
        this._stream = stream;
        stream._writer = this;
        var self = this;
        this.closed = new Promise(function(res, rej) {
            self._closedResolve = res;
            self._closedReject = rej;
        });
        this.ready = Promise.resolve();
    }
    Object.defineProperty(WritableStreamDefaultWriter.prototype, 'desiredSize', {
        get: function() { return 1; }
    });
    WritableStreamDefaultWriter.prototype.write = function(chunk) {
        if (!this._stream) return Promise.reject(new TypeError("Writer released"));
        var sink = this._stream._sink;
        if (typeof sink.write === 'function') {
            return Promise.resolve(sink.write(chunk, this._stream._controller));
        }
        return Promise.resolve();
    };
    WritableStreamDefaultWriter.prototype.close = function() {
        if (!this._stream) return Promise.reject(new TypeError("Writer released"));
        var self = this;
        return this._stream._closeInternal().then(function() {
            self._closedResolve();
        });
    };
    WritableStreamDefaultWriter.prototype.abort = function(reason) {
        if (!this._stream) return Promise.reject(new TypeError("Writer released"));
        return this._stream.abort(reason);
    };
    WritableStreamDefaultWriter.prototype.releaseLock = function() {
        if (!this._stream) return;
        this._stream._writer = null;
        this._stream = null;
    };
    globalThis.WritableStreamDefaultWriter = WritableStreamDefaultWriter;

    function WritableStream(underlyingSink, strategy) {
        underlyingSink = underlyingSink || {};
        this._sink = underlyingSink;
        this._strategy = strategy || {};
        this._state = 'writable';
        this._storedError = undefined;
        this._writer = null;
        var self = this;
        var controller = {
            error: function(e) {
                self._state = 'errored';
                self._storedError = e;
            }
        };
        this._controller = controller;
        if (typeof underlyingSink.start === 'function') {
            try { underlyingSink.start(controller); } catch(e) { controller.error(e); }
        }
    }
    Object.defineProperty(WritableStream.prototype, 'locked', {
        get: function() { return this._writer !== null; },
        configurable: true,
        enumerable: true
    });
    WritableStream.prototype.abort = function(reason) {
        this._state = 'errored';
        this._storedError = reason;
        if (typeof this._sink.abort === 'function') {
            return Promise.resolve(this._sink.abort(reason));
        }
        return Promise.resolve();
    };
    WritableStream.prototype.close = function() {
        if (this.locked) return Promise.reject(new TypeError("Stream is locked"));
        return this._closeInternal();
    };
    WritableStream.prototype._closeInternal = function() {
        this._state = 'closed';
        if (typeof this._sink.close === 'function') {
            return Promise.resolve(this._sink.close());
        }
        return Promise.resolve();
    };
    WritableStream.prototype.getWriter = function() {
        if (this.locked) throw new TypeError("WritableStream is locked");
        return new WritableStreamDefaultWriter(this);
    };
    globalThis.WritableStream = WritableStream;

    function TransformStream(transformer, writableStrategy, readableStrategy) {
        transformer = transformer || {};
        var readableCtrl;
        var self = this;
        var r = new ReadableStream({
            start: function(c) { readableCtrl = c; }
        }, readableStrategy);

        var controller = {
            get desiredSize() { return readableCtrl.desiredSize; },
            enqueue: function(chunk) { readableCtrl.enqueue(chunk); },
            error: function(reason) { readableCtrl.error(reason); },
            terminate: function() { readableCtrl.close(); }
        };

        var w = new WritableStream({
            start: function(c) {
                if (typeof transformer.start === 'function') {
                    return transformer.start(controller);
                }
            },
            write: function(chunk, c) {
                if (typeof transformer.transform === 'function') {
                    return transformer.transform(chunk, controller);
                }
                controller.enqueue(chunk);
            },
            close: function() {
                if (typeof transformer.flush === 'function') {
                    return Promise.resolve(transformer.flush(controller)).then(function() {
                        controller.terminate();
                    });
                }
                controller.terminate();
            },
            abort: function(reason) {
                controller.error(reason);
            }
        }, writableStrategy);

        this.readable = r;
        this.writable = w;
    }
    globalThis.TransformStream = TransformStream;

