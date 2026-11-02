    // ── 20b. WHATWG Streams API ──────────────────────────────────────────────
    function ByteLengthQueuingStrategy(options) {
        this.highWaterMark = (options && options.highWaterMark !== undefined) ? Number(options.highWaterMark) : 0;
    }
    ByteLengthQueuingStrategy.prototype.size = function(chunk) {
        return (chunk && chunk.byteLength !== undefined) ? chunk.byteLength : 0;
    };
    ByteLengthQueuingStrategy.prototype[Symbol.toStringTag] = 'ByteLengthQueuingStrategy';
    globalThis.ByteLengthQueuingStrategy = ByteLengthQueuingStrategy;

    function CountQueuingStrategy(options) {
        this.highWaterMark = (options && options.highWaterMark !== undefined) ? Number(options.highWaterMark) : 0;
    }
    CountQueuingStrategy.prototype.size = function() {
        return 1;
    };
    CountQueuingStrategy.prototype[Symbol.toStringTag] = 'CountQueuingStrategy';
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
    ReadableStreamDefaultReader.prototype[Symbol.toStringTag] = 'ReadableStreamDefaultReader';
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
        options = options || {};
        var preventClose = !!options.preventClose;
        var preventAbort = !!options.preventAbort;
        var preventCancel = !!options.preventCancel;
        var signal = options.signal;

        if (signal && signal.aborted) {
            return Promise.reject(signal.reason || new TypeError("Aborted"));
        }

        var self = this;
        var reader = this.getReader();
        var writer = destination.getWriter();

        return new Promise(function(resolve, reject) {
            var abortHandler = null;
            if (signal) {
                abortHandler = function() {
                    var err = signal.reason || new TypeError("Aborted");
                    reader.cancel(err).catch(function() {});
                    writer.abort(err).catch(function() {});
                    cleanup();
                    reject(err);
                };
                signal.addEventListener('abort', abortHandler);
            }

            function cleanup() {
                if (signal && abortHandler && signal.removeEventListener) {
                    signal.removeEventListener('abort', abortHandler);
                }
                try { reader.releaseLock(); } catch(e) {}
                try { writer.releaseLock(); } catch(e) {}
            }

            function pump() {
                reader.read().then(function(res) {
                    if (res.done) {
                        if (!preventClose) {
                            writer.close().then(function() {
                                cleanup();
                                resolve();
                            }, function(err) {
                                cleanup();
                                reject(err);
                            });
                        } else {
                            cleanup();
                            resolve();
                        }
                        return;
                    }
                    writer.write(res.value).then(pump, function(err) {
                        if (!preventCancel) {
                            reader.cancel(err).then(function() {
                                cleanup();
                                reject(err);
                            }, function() {
                                cleanup();
                                reject(err);
                            });
                        } else {
                            cleanup();
                            reject(err);
                        }
                    });
                }).catch(function(err) {
                    if (!preventAbort) {
                        writer.abort(err).then(function() {
                            cleanup();
                            reject(err);
                        }, function() {
                            cleanup();
                            reject(err);
                        });
                    } else {
                        cleanup();
                        reject(err);
                    }
                });
            }
            pump();
        });
    };
    ReadableStream.prototype.tee = function() {
        var self = this;
        var branch1Ctrl, branch2Ctrl;
        var b1Canceled = false, b2Canceled = false;
        var b1Reason, b2Reason;
        var reader = self.getReader();

        function checkCancel() {
            if (b1Canceled && b2Canceled) {
                return reader.cancel(b1Reason !== undefined ? b1Reason : b2Reason);
            }
            return Promise.resolve();
        }

        var b1 = new ReadableStream({
            start: function(c) { branch1Ctrl = c; },
            cancel: function(reason) {
                b1Canceled = true;
                b1Reason = reason;
                return checkCancel();
            }
        });
        var b2 = new ReadableStream({
            start: function(c) { branch2Ctrl = c; },
            cancel: function(reason) {
                b2Canceled = true;
                b2Reason = reason;
                return checkCancel();
            }
        });

        function pump() {
            reader.read().then(function(res) {
                if (res.done) {
                    if (!b1Canceled) branch1Ctrl.close();
                    if (!b2Canceled) branch2Ctrl.close();
                    return;
                }
                if (!b1Canceled) branch1Ctrl.enqueue(res.value);
                if (!b2Canceled) branch2Ctrl.enqueue(res.value);
                pump();
            }).catch(function(err) {
                if (!b1Canceled) branch1Ctrl.error(err);
                if (!b2Canceled) branch2Ctrl.error(err);
            });
        }
        pump();
        return [b1, b2];
    };
    ReadableStream.from = function(iterable) {
        if (!iterable || (typeof iterable[Symbol.iterator] !== 'function' && typeof iterable[Symbol.asyncIterator] !== 'function')) {
            throw new TypeError("ReadableStream.from requires an iterable object");
        }
        var it = typeof iterable[Symbol.asyncIterator] === 'function' ? iterable[Symbol.asyncIterator]() : iterable[Symbol.iterator]();
        return new ReadableStream({
            pull: function(controller) {
                return Promise.resolve(it.next()).then(function(res) {
                    if (res.done) {
                        controller.close();
                    } else {
                        controller.enqueue(res.value);
                    }
                }, function(err) {
                    controller.error(err);
                });
            },
            cancel: function(reason) {
                if (typeof it.return === 'function') {
                    return Promise.resolve(it.return(reason));
                }
                return Promise.resolve();
            }
        });
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
    ReadableStream.prototype[Symbol.toStringTag] = 'ReadableStream';
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
    WritableStreamDefaultWriter.prototype[Symbol.toStringTag] = 'WritableStreamDefaultWriter';
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
    WritableStream.prototype[Symbol.toStringTag] = 'WritableStream';
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
    TransformStream.prototype[Symbol.toStringTag] = 'TransformStream';
    globalThis.TransformStream = TransformStream;

