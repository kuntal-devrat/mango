    // ── Origin Private File System (OPFS) ──────────────────────────
    (function() {
        var _opfsRoot = {
            kind: 'directory',
            name: '',
            children: new Map(),
            parent: null
        };

        function FileSystemHandle(kind, name) {
            this.kind = kind;
            this.name = name;
        }
        FileSystemHandle.prototype.isSameEntry = function(other) {
            if (!other || !(other instanceof FileSystemHandle)) return Promise.resolve(false);
            return Promise.resolve(this._entry === other._entry);
        };
        globalThis.FileSystemHandle = FileSystemHandle;

        function FileSystemWritableFileStream(entry, initialBuffer) {
            var self = this;
            this._entry = entry;
            this._position = 0;
            this._buffer = initialBuffer ? new Uint8Array(initialBuffer) : new Uint8Array(0);
            this._closed = false;

            var sink = {
                write: function(chunk) { return self.write(chunk); },
                close: function() { return self.close(); },
                abort: function(reason) { return self.abort(reason); }
            };
            if (typeof WritableStream === 'function') {
                try {
                    WritableStream.call(this, sink);
                } catch(e) {}
            }
        }
        if (typeof WritableStream === 'function') {
            FileSystemWritableFileStream.prototype = Object.create(WritableStream.prototype);
            FileSystemWritableFileStream.prototype.constructor = FileSystemWritableFileStream;
        }
        FileSystemWritableFileStream.prototype.write = function(data) {
            if (this._closed) return Promise.reject(new DOMException("Stream is closed", "InvalidStateError"));
            var bytes = null;
            var pos = this._position;
            if (typeof data === 'string') {
                bytes = new TextEncoder().encode(data);
            } else if (data instanceof ArrayBuffer) {
                bytes = new Uint8Array(data);
            } else if (ArrayBuffer.isView(data)) {
                bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
            } else if (data && typeof data === 'object') {
                if (data.type === 'seek') {
                    return this.seek(data.position);
                } else if (data.type === 'truncate') {
                    return this.truncate(data.size);
                } else if (data.type === 'write') {
                    if (typeof data.position === 'number') {
                        pos = Math.max(0, data.position);
                    }
                    if (typeof data.data === 'string') {
                        bytes = new TextEncoder().encode(data.data);
                    } else if (data.data instanceof ArrayBuffer) {
                        bytes = new Uint8Array(data.data);
                    } else if (ArrayBuffer.isView(data.data)) {
                        bytes = new Uint8Array(data.data.buffer, data.data.byteOffset, data.data.byteLength);
                    }
                }
            }
            if (bytes) {
                var needed = pos + bytes.length;
                if (needed > this._buffer.length) {
                    var bigger = new Uint8Array(needed);
                    bigger.set(this._buffer);
                    this._buffer = bigger;
                }
                this._buffer.set(bytes, pos);
                this._position = pos + bytes.length;
            }
            return Promise.resolve();
        };
        FileSystemWritableFileStream.prototype.seek = function(position) {
            if (this._closed) return Promise.reject(new DOMException("Stream is closed", "InvalidStateError"));
            this._position = Math.max(0, parseInt(position) || 0);
            return Promise.resolve();
        };
        FileSystemWritableFileStream.prototype.truncate = function(size) {
            if (this._closed) return Promise.reject(new DOMException("Stream is closed", "InvalidStateError"));
            var newSize = Math.max(0, parseInt(size) || 0);
            var nextBuf = new Uint8Array(newSize);
            nextBuf.set(this._buffer.subarray(0, Math.min(this._buffer.length, newSize)));
            this._buffer = nextBuf;
            if (this._position > newSize) {
                this._position = newSize;
            }
            return Promise.resolve();
        };
        FileSystemWritableFileStream.prototype.close = function() {
            if (this._closed) return Promise.resolve();
            this._closed = true;
            this._entry.data = this._buffer;
            this._entry.lastModified = Date.now();
            this._entry.locked = false;
            return Promise.resolve();
        };
        FileSystemWritableFileStream.prototype.abort = function(reason) {
            this._closed = true;
            this._entry.locked = false;
            return Promise.resolve();
        };
        globalThis.FileSystemWritableFileStream = FileSystemWritableFileStream;

        function FileSystemSyncAccessHandle(entry) {
            this._entry = entry;
            this._cursor = 0;
            this._closed = false;
        }
        FileSystemSyncAccessHandle.prototype.read = function(buffer, options) {
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            if (!buffer || !buffer.buffer) throw new TypeError("buffer must be an ArrayBufferView");
            var at = (options && typeof options.at === 'number') ? options.at : this._cursor;
            var fileData = this._entry.data;
            if (at >= fileData.length) return 0;
            var available = fileData.length - at;
            var toRead = Math.min(buffer.byteLength, available);
            var destU8 = new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
            destU8.set(fileData.subarray(at, at + toRead));
            if (!options || typeof options.at !== 'number') {
                this._cursor += toRead;
            }
            return toRead;
        };
        FileSystemSyncAccessHandle.prototype.write = function(buffer, options) {
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            if (!buffer || !buffer.buffer) throw new TypeError("buffer must be an ArrayBufferView");
            var at = (options && typeof options.at === 'number') ? options.at : this._cursor;
            var srcU8 = new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
            var needed = at + srcU8.length;
            if (needed > this._entry.data.length) {
                var bigger = new Uint8Array(needed);
                bigger.set(this._entry.data);
                this._entry.data = bigger;
            }
            this._entry.data.set(srcU8, at);
            this._entry.lastModified = Date.now();
            if (!options || typeof options.at !== 'number') {
                this._cursor += srcU8.length;
            }
            return srcU8.length;
        };
        FileSystemSyncAccessHandle.prototype.truncate = function(newSize) {
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            var sz = Math.max(0, parseInt(newSize) || 0);
            var nextBuf = new Uint8Array(sz);
            nextBuf.set(this._entry.data.subarray(0, Math.min(this._entry.data.length, sz)));
            this._entry.data = nextBuf;
            this._entry.lastModified = Date.now();
            if (this._cursor > sz) this._cursor = sz;
        };
        FileSystemSyncAccessHandle.prototype.getSize = function() {
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            return this._entry.data.length;
        };
        FileSystemSyncAccessHandle.prototype.flush = function() {
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
        };
        FileSystemSyncAccessHandle.prototype.close = function() {
            if (this._closed) return;
            this._closed = true;
            this._entry.locked = false;
        };
        globalThis.FileSystemSyncAccessHandle = FileSystemSyncAccessHandle;

        function FileSystemFileHandle(entry) {
            FileSystemHandle.call(this, 'file', entry.name);
            this._entry = entry;
        }
        FileSystemFileHandle.prototype = Object.create(FileSystemHandle.prototype);
        FileSystemFileHandle.prototype.constructor = FileSystemFileHandle;
        FileSystemFileHandle.prototype.getFile = function() {
            var self = this;
            var u8 = self._entry.data ? self._entry.data.slice() : new Uint8Array(0);
            var file = new File([u8], self.name, {
                type: 'application/octet-stream',
                lastModified: self._entry.lastModified || Date.now()
            });
            return Promise.resolve(file);
        };
        FileSystemFileHandle.prototype.createWritable = function(options) {
            if (this._entry.locked) {
                return Promise.reject(new DOMException("The requested file could not be accessed because it is locked.", "NoModificationAllowedError"));
            }
            this._entry.locked = true;
            var keep = options && options.keepExistingData;
            var initBuf = keep ? this._entry.data.slice() : new Uint8Array(0);
            var stream = new FileSystemWritableFileStream(this._entry, initBuf);
            return Promise.resolve(stream);
        };
        FileSystemFileHandle.prototype.createSyncAccessHandle = function() {
            if (this._entry.locked) {
                return Promise.reject(new DOMException("The requested file could not be accessed because it is locked.", "NoModificationAllowedError"));
            }
            this._entry.locked = true;
            return Promise.resolve(new FileSystemSyncAccessHandle(this._entry));
        };
        globalThis.FileSystemFileHandle = FileSystemFileHandle;

        function FileSystemDirectoryHandle(entry) {
            FileSystemHandle.call(this, 'directory', entry.name);
            this._entry = entry;
        }
        FileSystemDirectoryHandle.prototype = Object.create(FileSystemHandle.prototype);
        FileSystemDirectoryHandle.prototype.constructor = FileSystemDirectoryHandle;
        FileSystemDirectoryHandle.prototype.getFileHandle = function(name, options) {
            if (!name || typeof name !== 'string' || name.indexOf('/') !== -1 || name === '.' || name === '..') {
                return Promise.reject(new TypeError("Invalid file name: " + name));
            }
            var create = !!(options && options.create);
            if (this._entry.children.has(name)) {
                var child = this._entry.children.get(name);
                if (child.kind === 'directory') {
                    return Promise.reject(new DOMException("The entry is a directory, not a file.", "TypeMismatchError"));
                }
                return Promise.resolve(new FileSystemFileHandle(child));
            }
            if (create) {
                var newChild = {
                    kind: 'file',
                    name: name,
                    data: new Uint8Array(0),
                    lastModified: Date.now(),
                    locked: false,
                    parent: this._entry
                };
                this._entry.children.set(name, newChild);
                return Promise.resolve(new FileSystemFileHandle(newChild));
            }
            return Promise.reject(new DOMException("A requested file or directory could not be found.", "NotFoundError"));
        };
        FileSystemDirectoryHandle.prototype.getDirectoryHandle = function(name, options) {
            if (!name || typeof name !== 'string' || name.indexOf('/') !== -1 || name === '.' || name === '..') {
                return Promise.reject(new TypeError("Invalid directory name: " + name));
            }
            var create = !!(options && options.create);
            if (this._entry.children.has(name)) {
                var child = this._entry.children.get(name);
                if (child.kind === 'file') {
                    return Promise.reject(new DOMException("The entry is a file, not a directory.", "TypeMismatchError"));
                }
                return Promise.resolve(new FileSystemDirectoryHandle(child));
            }
            if (create) {
                var newDir = {
                    kind: 'directory',
                    name: name,
                    children: new Map(),
                    parent: this._entry
                };
                this._entry.children.set(name, newDir);
                return Promise.resolve(new FileSystemDirectoryHandle(newDir));
            }
            return Promise.reject(new DOMException("A requested file or directory could not be found.", "NotFoundError"));
        };
        FileSystemDirectoryHandle.prototype.removeEntry = function(name, options) {
            if (!this._entry.children.has(name)) {
                return Promise.reject(new DOMException("Entry not found: " + name, "NotFoundError"));
            }
            var child = this._entry.children.get(name);
            var recursive = !!(options && options.recursive);
            if (child.kind === 'directory' && child.children.size > 0 && !recursive) {
                return Promise.reject(new DOMException("The entry is a non-empty directory.", "InvalidModificationError"));
            }
            this._entry.children.delete(name);
            return Promise.resolve();
        };
        FileSystemDirectoryHandle.prototype.resolve = function(possibleDescendant) {
            if (!possibleDescendant || !possibleDescendant._entry) return Promise.resolve(null);
            var curr = possibleDescendant._entry;
            var path = [];
            while (curr && curr !== this._entry) {
                path.unshift(curr.name);
                curr = curr.parent;
            }
            if (curr === this._entry) {
                return Promise.resolve(path);
            }
            return Promise.resolve(null);
        };
        function makeAsyncIterator(entries) {
            var index = 0;
            var iter = {
                next: function() {
                    if (index < entries.length) {
                        return Promise.resolve({ value: entries[index++], done: false });
                    }
                    return Promise.resolve({ value: undefined, done: true });
                }
            };
            iter[Symbol.asyncIterator] = function() { return this; };
            return iter;
        }
        FileSystemDirectoryHandle.prototype.entries = function() {
            var list = [];
            this._entry.children.forEach(function(val, key) {
                var handle = val.kind === 'directory' ? new FileSystemDirectoryHandle(val) : new FileSystemFileHandle(val);
                list.push([key, handle]);
            });
            return makeAsyncIterator(list);
        };
        FileSystemDirectoryHandle.prototype.keys = function() {
            var list = [];
            this._entry.children.forEach(function(val, key) {
                list.push(key);
            });
            return makeAsyncIterator(list);
        };
        FileSystemDirectoryHandle.prototype.values = function() {
            var list = [];
            this._entry.children.forEach(function(val) {
                var handle = val.kind === 'directory' ? new FileSystemDirectoryHandle(val) : new FileSystemFileHandle(val);
                list.push(handle);
            });
            return makeAsyncIterator(list);
        };
        FileSystemDirectoryHandle.prototype[Symbol.asyncIterator] = function() {
            return this.entries();
        };
        globalThis.FileSystemDirectoryHandle = FileSystemDirectoryHandle;

        if (globalThis.navigator) {
            if (!globalThis.navigator.storage) {
                globalThis.navigator.storage = {
                    estimate: function() { return Promise.resolve({ quota: 1000000000, usage: 0 }); },
                    persist: function() { return Promise.resolve(true); }
                };
            }
            globalThis.navigator.storage.getDirectory = function() {
                return Promise.resolve(new FileSystemDirectoryHandle(_opfsRoot));
            };
        }
    })();

    // ── WebTransport (HTTP/3 & QUIC) ────────────────────────────────
    (function() {
        function WebTransportError(message, options) {
            var err = new Error(message || "WebTransport error");
            err.name = "WebTransportError";
            err.source = (options && options.source) || "session";
            err.streamErrorCode = (options && typeof options.streamErrorCode === 'number') ? options.streamErrorCode : null;
            Object.setPrototypeOf(err, WebTransportError.prototype);
            return err;
        }
        WebTransportError.prototype = Object.create(Error.prototype);
        WebTransportError.prototype.constructor = WebTransportError;
        globalThis.WebTransportError = WebTransportError;

        function WebTransportBidirectionalStream(readable, writable) {
            this.readable = readable;
            this.writable = writable;
        }
        globalThis.WebTransportBidirectionalStream = WebTransportBidirectionalStream;

        function WebTransport(url, options) {
            if (!url) throw new TypeError("WebTransport requires a URL string");
            var parsed;
            try {
                parsed = new URL(url);
            } catch(e) {
                throw new DOMException("Invalid WebTransport URL: " + url, "SyntaxError");
            }
            if (parsed.protocol !== 'https:' && parsed.protocol !== 'wts:') {
                throw new DOMException("WebTransport URL scheme must be https or wts", "SyntaxError");
            }
            if (parsed.hash && parsed.hash.length > 0) {
                throw new DOMException("WebTransport URL must not contain a fragment identifier", "SyntaxError");
            }

            this._url = parsed.href;
            this._options = options || {};
            this.reliability = (options && options.requireUnreliable) ? 'supports-unreliable' : 'supports-unreliable';
            this.congestionControl = (options && options.congestionControl) || 'default';
            this._closed = false;

            var self = this;
            var resolveReady, rejectReady;
            this.ready = new Promise(function(res, rej) {
                resolveReady = res;
                rejectReady = rej;
            });

            var resolveClosed, rejectClosed;
            this.closed = new Promise(function(res, rej) {
                resolveClosed = res;
                rejectClosed = rej;
            });

            this._resolveClosed = resolveClosed;
            this._rejectClosed = rejectClosed;

            Promise.resolve().then(function() {
                if (!self._closed) {
                    resolveReady();
                }
            });

            var resolveDraining;
            this.draining = new Promise(function(res) {
                resolveDraining = res;
            });
            this._resolveDraining = resolveDraining;

            this.datagrams = {
                maxDatagramSize: 1232,
                incomingMaxAge: null,
                outgoingMaxAge: null,
                incomingHighWaterMark: 1,
                outgoingHighWaterMark: 1,
                readable: new ReadableStream({
                    start: function(controller) {
                        self._datagramController = controller;
                    },
                    pull: function(controller) {},
                    cancel: function(reason) {}
                }),
                writable: new WritableStream({
                    write: function(chunk) {
                        return Promise.resolve();
                    },
                    close: function() {
                        return Promise.resolve();
                    },
                    abort: function(reason) {
                        return Promise.resolve();
                    }
                })
            };

            this.incomingBidirectionalStreams = new ReadableStream({
                start: function(controller) {
                    self._incomingBidiController = controller;
                }
            });
            this.incomingUnidirectionalStreams = new ReadableStream({
                start: function(controller) {
                    self._incomingUniController = controller;
                }
            });
        }

        WebTransport.prototype.createBidirectionalStream = function(options) {
            if (this._closed) return Promise.reject(new WebTransportError("Connection closed", { source: 'session' }));
            var readCtrl;
            var readable = new ReadableStream({
                start: function(controller) { readCtrl = controller; }
            });
            var writable = new WritableStream({
                write: function(chunk) { return Promise.resolve(); },
                close: function() { return Promise.resolve(); }
            });
            var bidi = new WebTransportBidirectionalStream(readable, writable);
            return Promise.resolve(bidi);
        };

        WebTransport.prototype.createUnidirectionalStream = function(options) {
            if (this._closed) return Promise.reject(new WebTransportError("Connection closed", { source: 'session' }));
            var writable = new WritableStream({
                write: function(chunk) { return Promise.resolve(); },
                close: function() { return Promise.resolve(); }
            });
            return Promise.resolve(writable);
        };

        WebTransport.prototype.close = function(closeInfo) {
            if (this._closed) return;
            this._closed = true;
            var code = (closeInfo && typeof closeInfo.closeCode === 'number') ? closeInfo.closeCode : 0;
            var reason = (closeInfo && closeInfo.reason) || "";
            this._resolveClosed({ closeCode: code, reason: reason });
        };

        WebTransport.prototype.getStats = function() {
            return Promise.resolve({
                bytesSent: 0,
                packetsSent: 0,
                bytesReceived: 0,
                packetsReceived: 0,
                smoothedRtt: 15.0,
                rttVariation: 2.0,
                minRtt: 10.0,
                datagrams: {
                    droppedSendRate: 0,
                    expiredOutgoing: 0,
                    lostOutgoing: 0
                }
            });
        };
        globalThis.WebTransport = WebTransport;
    })();

    // ── Multi-Threading, Shared Memory & Cross-Origin Isolation ─────
    (function() {
        var _coi = false;
        var _coopHeader = "";
        var _coepHeader = "";

        function evaluateCoi() {
            var coopOk = (_coopHeader === 'same-origin');
            var coepOk = (_coepHeader === 'require-corp' || _coepHeader === 'credentialless');
            _coi = coopOk && coepOk;
            return _coi;
        }

        Object.defineProperty(globalThis, 'crossOriginIsolated', {
            get: function() { return _coi; },
            set: function(val) { _coi = !!val; },
            configurable: true,
            enumerable: true
        });

        if (typeof window !== 'undefined') {
            Object.defineProperty(window, 'crossOriginIsolated', {
                get: function() { return _coi; },
                set: function(val) { _coi = !!val; },
                configurable: true,
                enumerable: true
            });
        }

        globalThis.__setCrossOriginIsolationHeaders = function(coop, coep) {
            _coopHeader = (coop || "").trim();
            _coepHeader = (coep || "").trim();
            return evaluateCoi();
        };

        globalThis.__setCrossOriginIsolated = function(val) {
            _coi = !!val;
            return _coi;
        };
    })();

})();
