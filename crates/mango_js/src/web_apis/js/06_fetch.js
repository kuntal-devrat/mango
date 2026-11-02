    // ── 21. WHATWG Fetch (Headers, Request, Response, fetch) ───────────────────
    var _HEADER_NAME_REGEX = /^[a-zA-Z0-9!#$%&'*+\-.^_`|~]+$/;

    function _validateHeaderName(name) {
        var str = String(name);
        if (!str || !_HEADER_NAME_REGEX.test(str)) {
            throw new TypeError("Invalid header name: '" + str + "'");
        }
        return str;
    }

    function _validateHeaderValue(val) {
        var str = String(val);
        for (var i = 0; i < str.length; i++) {
            var code = str.charCodeAt(i);
            if (code === 0x00 || code === 0x0a || code === 0x0d) {
                throw new TypeError("Invalid header value character: 0x" + code.toString(16));
            }
        }
        // Trim leading and trailing HTTP whitespace (\x20, \x09)
        return str.replace(/^[\x20\x09]+|[\x20\x09]+$/g, '');
    }

    function Headers(init) {
        this._map = Object.create(null);
        this._setCookies = [];
        if (!init) return;

        if (init instanceof Headers) {
            var self = this;
            init.forEach(function(v, k) { self.append(k, v); });
            var sc = init.getSetCookie();
            for (var i = 0; i < sc.length; i++) this.append('set-cookie', sc[i]);
        } else if (Array.isArray(init)) {
            for (var i = 0; i < init.length; i++) {
                var p = init[i];
                if (p && p.length >= 2) this.append(p[0], p[1]);
            }
        } else if (typeof init === 'object') {
            var keys = Object.keys(init);
            for (var i = 0; i < keys.length; i++) {
                var k = keys[i];
                this.append(k, init[k]);
            }
        }
    }

    Headers.prototype = {
        constructor: Headers,
        append: function(name, val) {
            var n = _validateHeaderName(name);
            var v = _validateHeaderValue(val);
            var ln = n.toLowerCase();
            if (ln === 'set-cookie') {
                this._setCookies.push(v);
            } else {
                this._map[ln] = this._map[ln] !== undefined ? this._map[ln] + ', ' + v : v;
            }
        },
        delete: function(name) {
            var n = _validateHeaderName(name);
            var ln = n.toLowerCase();
            if (ln === 'set-cookie') {
                this._setCookies = [];
            } else {
                delete this._map[ln];
            }
        },
        get: function(name) {
            var n = _validateHeaderName(name);
            var ln = n.toLowerCase();
            if (ln === 'set-cookie') {
                return this._setCookies.length > 0 ? this._setCookies.join(', ') : null;
            }
            return this._map[ln] !== undefined ? this._map[ln] : null;
        },
        getSetCookie: function() {
            return this._setCookies.slice();
        },
        getAll: function(name) {
            var n = _validateHeaderName(name);
            var ln = n.toLowerCase();
            if (ln === 'set-cookie') return this._setCookies.slice();
            var val = this.get(n);
            return val !== null ? val.split(', ') : [];
        },
        has: function(name) {
            var n = _validateHeaderName(name);
            var ln = n.toLowerCase();
            if (ln === 'set-cookie') return this._setCookies.length > 0;
            return ln in this._map;
        },
        set: function(name, val) {
            var n = _validateHeaderName(name);
            var v = _validateHeaderValue(val);
            var ln = n.toLowerCase();
            if (ln === 'set-cookie') {
                this._setCookies = [v];
            } else {
                this._map[ln] = v;
            }
        },
        forEach: function(cb, thisArg) {
            var entries = this._sortedEntries();
            for (var i = 0; i < entries.length; i++) {
                cb.call(thisArg, entries[i][1], entries[i][0], this);
            }
        },
        _sortedEntries: function() {
            var list = [];
            for (var k in this._map) {
                list.push([k, this._map[k]]);
            }
            for (var i = 0; i < this._setCookies.length; i++) {
                list.push(['set-cookie', this._setCookies[i]]);
            }
            list.sort(function(a, b) {
                return a[0] < b[0] ? -1 : (a[0] > b[0] ? 1 : 0);
            });
            return list;
        },
        keys: function() {
            var entries = this._sortedEntries();
            var i = 0;
            var it = {
                next: function() {
                    if (i < entries.length) return { value: entries[i++][0], done: false };
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        },
        values: function() {
            var entries = this._sortedEntries();
            var i = 0;
            var it = {
                next: function() {
                    if (i < entries.length) return { value: entries[i++][1], done: false };
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        },
        entries: function() {
            var entries = this._sortedEntries();
            var i = 0;
            var it = {
                next: function() {
                    if (i < entries.length) return { value: entries[i++], done: false };
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        }
    };
    Headers.prototype[Symbol.iterator] = Headers.prototype.entries;
    Headers.prototype[Symbol.toStringTag] = 'Headers';
    globalThis.Headers = Headers;

    // Helper: convert body inputs to a ReadableStream and deduce Content-Type
    function _consumeBodyPayload(body) {
        if (body === null || body === undefined) return { stream: null, text: '' };
        if (body instanceof ReadableStream) return { stream: body, text: null };

        var u8 = null;
        var text = null;
        var contentType = null;

        if (typeof body === 'string') {
            text = body;
            u8 = new TextEncoder().encode(body);
            contentType = 'text/plain;charset=UTF-8';
        } else if (body instanceof Uint8Array) {
            u8 = body;
        } else if (body instanceof ArrayBuffer) {
            u8 = new Uint8Array(body);
        } else if (ArrayBuffer.isView(body)) {
            u8 = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
        } else if (typeof Blob !== 'undefined' && body instanceof Blob) {
            if (body.type) contentType = body.type;
            if (body._buffer) u8 = new Uint8Array(body._buffer);
            else if (body._chunks) {
                var total = 0;
                for (var ci = 0; ci < body._chunks.length; ci++) total += body._chunks[ci].length;
                u8 = new Uint8Array(total);
                var off = 0;
                for (var ci = 0; ci < body._chunks.length; ci++) {
                    u8.set(body._chunks[ci], off);
                    off += body._chunks[ci].length;
                }
            }
        } else if (typeof FormData !== 'undefined' && body instanceof FormData) {
            text = body.serializeUrlEncoded ? body.serializeUrlEncoded() : '';
            u8 = new TextEncoder().encode(text);
            contentType = 'application/x-www-form-urlencoded;charset=UTF-8';
        } else if (typeof URLSearchParams !== 'undefined' && body instanceof URLSearchParams) {
            text = body.toString();
            u8 = new TextEncoder().encode(text);
            contentType = 'application/x-www-form-urlencoded;charset=UTF-8';
        } else {
            text = String(body);
            u8 = new TextEncoder().encode(text);
            contentType = 'text/plain;charset=UTF-8';
        }

        var stream = new ReadableStream({
            start: function(controller) {
                if (u8 && u8.length > 0) controller.enqueue(u8);
                controller.close();
            }
        });
        return { stream: stream, text: text, u8: u8, contentType: contentType };
    }

    function _readAllChunksFromStream(stream) {
        var reader = stream.getReader();
        var chunks = [];
        function pump() {
            return reader.read().then(function(res) {
                if (res.done) {
                    var total = 0;
                    for (var i = 0; i < chunks.length; i++) total += chunks[i].length;
                    var merged = new Uint8Array(total);
                    var off = 0;
                    for (var i = 0; i < chunks.length; i++) {
                        merged.set(chunks[i], off);
                        off += chunks[i].length;
                    }
                    return merged;
                }
                if (typeof res.value === 'string') {
                    chunks.push(new TextEncoder().encode(res.value));
                } else if (res.value instanceof Uint8Array) {
                    chunks.push(res.value);
                } else if (res.value && res.value.buffer) {
                    chunks.push(new Uint8Array(res.value.buffer));
                }
                return pump();
            });
        }
        return pump();
    }

    function Request(input, init) {
        init = init || {};
        var method = init.method;
        var headersInit = init.headers;
        var bodyInit = init.body;
        var signal = init.signal;

        if (input instanceof Request) {
            if (input.bodyUsed) {
                throw new TypeError("Cannot construct Request from a disturbed Request");
            }
            this.url = input.url;
            this.method = method ? String(method).toUpperCase() : input.method;
            this.headers = new Headers(headersInit || input.headers);
            this.signal = signal || input.signal;
            this.mode = init.mode || input.mode;
            this.credentials = init.credentials || input.credentials;
            this.cache = init.cache || input.cache;
            this.redirect = init.redirect || input.redirect;
            this.referrer = init.referrer || input.referrer;
            this.referrerPolicy = init.referrerPolicy || input.referrerPolicy;
            this.destination = init.destination || input.destination;
            this.integrity = init.integrity || input.integrity;
            this.keepalive = init.keepalive !== undefined ? !!init.keepalive : input.keepalive;

            if (bodyInit !== undefined) {
                var extracted = _consumeBodyPayload(bodyInit);
                this._bodyStream = extracted.stream;
                this._rawText = extracted.text;
                if (extracted.contentType && !this.headers.has('content-type')) {
                    this.headers.set('content-type', extracted.contentType);
                }
            } else if (input._bodyStream) {
                var branches = input._bodyStream.tee();
                input._bodyStream = branches[0];
                this._bodyStream = branches[1];
                this._rawText = input._rawText;
            } else {
                this._bodyStream = null;
                this._rawText = null;
            }
        } else {
            var urlStr = String(input || '');
            if (urlStr && urlStr.indexOf('://') === -1 && urlStr.indexOf('//') !== 0) {
                try {
                    var base = globalThis.location ? (globalThis.location.origin + (globalThis.location.pathname || '/')) : 'http://localhost/';
                    urlStr = new URL(urlStr, base).href;
                } catch(e) {}
            }
            this.url = urlStr;
            this.method = method ? String(method).toUpperCase() : 'GET';
            this.headers = new Headers(headersInit);
            this.signal = signal || (typeof AbortController !== 'undefined' ? new AbortController().signal : null);
            this.mode = init.mode || 'cors';
            this.credentials = init.credentials || 'same-origin';
            this.cache = init.cache || 'default';
            this.redirect = init.redirect || 'follow';
            this.referrer = init.referrer || 'about:client';
            this.referrerPolicy = init.referrerPolicy || '';
            this.destination = init.destination || '';
            this.integrity = init.integrity || '';
            this.keepalive = !!init.keepalive;

            if (bodyInit !== undefined && bodyInit !== null) {
                var extracted = _consumeBodyPayload(bodyInit);
                this._bodyStream = extracted.stream;
                this._rawText = extracted.text;
                if (extracted.contentType && !this.headers.has('content-type')) {
                    this.headers.set('content-type', extracted.contentType);
                }
            } else {
                this._bodyStream = null;
                this._rawText = null;
            }
        }

        // Validate forbidden methods
        var upperM = this.method.toUpperCase();
        if (upperM === 'CONNECT' || upperM === 'TRACE' || upperM === 'TRACK') {
            throw new TypeError("Forbidden method: " + upperM);
        }
        if ((upperM === 'GET' || upperM === 'HEAD') && this._bodyStream !== null) {
            throw new TypeError("Request with GET/HEAD method cannot have body");
        }

        this._bodyUsed = false;
        this.isReloadNavigation = false;
        this.isHistoryNavigation = false;
    }

    Object.defineProperty(Request.prototype, 'bodyUsed', {
        get: function() {
            return !!this._bodyUsed || (this._bodyStream ? this._bodyStream._disturbed : false);
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(Request.prototype, 'body', {
        get: function() { return this._bodyStream; },
        configurable: true,
        enumerable: true
    });

    Request.prototype.clone = function() {
        if (this.bodyUsed) {
            throw new TypeError("Cannot clone a disturbed Request");
        }
        return new Request(this);
    };
    Request.prototype.text = function() {
        if (this.bodyUsed) return Promise.reject(new TypeError("Body has already been consumed"));
        this._bodyUsed = true;
        if (this._rawText !== null && this._rawText !== undefined) {
            return Promise.resolve(this._rawText);
        }
        if (!this._bodyStream) return Promise.resolve('');
        return _readAllChunksFromStream(this._bodyStream).then(function(u8) {
            return new TextDecoder().decode(u8);
        });
    };
    Request.prototype.json = function() {
        return this.text().then(function(t) { return JSON.parse(t); });
    };
    Request.prototype.arrayBuffer = function() {
        if (this.bodyUsed) return Promise.reject(new TypeError("Body has already been consumed"));
        this._bodyUsed = true;
        if (!this._bodyStream) return Promise.resolve(new ArrayBuffer(0));
        return _readAllChunksFromStream(this._bodyStream).then(function(u8) {
            return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
        });
    };
    Request.prototype.bytes = function() {
        if (this.bodyUsed) return Promise.reject(new TypeError("Body has already been consumed"));
        this._bodyUsed = true;
        if (!this._bodyStream) return Promise.resolve(new Uint8Array(0));
        return _readAllChunksFromStream(this._bodyStream);
    };
    Request.prototype.blob = function() {
        var ct = this.headers.get('content-type') || '';
        return this.arrayBuffer().then(function(buf) {
            return new Blob([buf], { type: ct });
        });
    };
    Request.prototype.formData = function() {
        var self = this;
        return this.text().then(function(t) {
            var fd = new FormData();
            var params = new URLSearchParams(t);
            params.forEach(function(v, k) { fd.append(k, v); });
            return fd;
        });
    };
    Request.prototype[Symbol.toStringTag] = 'Request';
    globalThis.Request = Request;

    function Response(body, init) {
        init = init || {};
        var status = (init.status !== undefined) ? Number(init.status) : 200;
        if (status < 200 || status > 599) {
            if (init.type !== 'error' && status !== 0) {
                throw new RangeError("The status provided (" + status + ") is outside the range [200, 599]");
            }
        }
        if ((status === 204 || status === 205 || status === 304) && body !== null && body !== undefined && body !== '') {
            throw new TypeError("Response with null body status cannot have body");
        }

        this.status = status;
        this.statusText = (init.statusText !== undefined) ? String(init.statusText) : (status === 200 ? 'OK' : '');
        this.ok = status >= 200 && status < 300;
        this.headers = new Headers(init.headers);
        this.url = init.url ? String(init.url) : '';
        this.type = init.type ? String(init.type) : 'default';
        this.redirected = !!init.redirected;

        var extracted = _consumeBodyPayload(body);
        this._bodyStream = extracted.stream;
        this._rawText = extracted.text;
        if (extracted.contentType && !this.headers.has('content-type')) {
            this.headers.set('content-type', extracted.contentType);
        }
        this._bodyUsed = false;
    }

    Object.defineProperty(Response.prototype, 'bodyUsed', {
        get: function() {
            return !!this._bodyUsed || (this._bodyStream ? this._bodyStream._disturbed : false);
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(Response.prototype, 'body', {
        get: function() { return this._bodyStream; },
        configurable: true,
        enumerable: true
    });

    Response.prototype.clone = function() {
        if (this.bodyUsed) {
            throw new TypeError("Cannot clone a disturbed Response");
        }
        var newBody = null;
        if (this._bodyStream) {
            var branches = this._bodyStream.tee();
            this._bodyStream = branches[0];
            newBody = branches[1];
        }
        var r = new Response(newBody, {
            status: this.status,
            statusText: this.statusText,
            headers: this.headers,
            url: this.url,
            type: this.type,
            redirected: this.redirected
        });
        r._rawText = this._rawText;
        return r;
    };
    Response.prototype.text = function() {
        if (this.bodyUsed) return Promise.reject(new TypeError("Body has already been consumed"));
        this._bodyUsed = true;
        if (this._rawText !== null && this._rawText !== undefined) {
            return Promise.resolve(this._rawText);
        }
        if (!this._bodyStream) return Promise.resolve('');
        return _readAllChunksFromStream(this._bodyStream).then(function(u8) {
            return new TextDecoder().decode(u8);
        });
    };
    Response.prototype.json = function() {
        return this.text().then(function(t) { return JSON.parse(t); });
    };
    Response.prototype.arrayBuffer = function() {
        if (this.bodyUsed) return Promise.reject(new TypeError("Body has already been consumed"));
        this._bodyUsed = true;
        if (!this._bodyStream) return Promise.resolve(new ArrayBuffer(0));
        return _readAllChunksFromStream(this._bodyStream).then(function(u8) {
            return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
        });
    };
    Response.prototype.bytes = function() {
        if (this.bodyUsed) return Promise.reject(new TypeError("Body has already been consumed"));
        this._bodyUsed = true;
        if (!this._bodyStream) return Promise.resolve(new Uint8Array(0));
        return _readAllChunksFromStream(this._bodyStream);
    };
    Response.prototype.blob = function() {
        var ct = this.headers.get('content-type') || '';
        return this.arrayBuffer().then(function(buf) {
            return new Blob([buf], { type: ct });
        });
    };
    Response.prototype.formData = function() {
        return this.text().then(function(t) {
            var fd = new FormData();
            var params = new URLSearchParams(t);
            params.forEach(function(v, k) { fd.append(k, v); });
            return fd;
        });
    };

    Response.error = function() {
        var r = new Response(null, { status: 0, statusText: '', type: 'error' });
        r.ok = false;
        return r;
    };
    Response.redirect = function(url, status) {
        status = status || 302;
        if ([301, 302, 303, 307, 308].indexOf(status) === -1) {
            throw new RangeError("Invalid redirect status: " + status);
        }
        var r = new Response(null, {
            status: status,
            headers: { Location: String(url) }
        });
        return r;
    };
    Response.json = function(data, init) {
        var body = JSON.stringify(data);
        var hdrs = new Headers(init && init.headers);
        if (!hdrs.has('content-type')) hdrs.set('content-type', 'application/json');
        var opts = Object.assign({}, init, { headers: hdrs });
        return new Response(body, opts);
    };
    Response.prototype[Symbol.toStringTag] = 'Response';
    globalThis.Response = Response;

    // ── fetch() Execution Engine ───────────────────────────────────────────────
    var _nextFetchId = 1;
    var _pendingFetches = {};

    globalThis._mangoResolveFetch = function(id, rawJson) {
        var entry = _pendingFetches[id];
        if (!entry) return;
        delete _pendingFetches[id];

        var raw;
        try {
            raw = typeof rawJson === 'string' ? JSON.parse(rawJson) : rawJson;
        } catch(e) {
            entry.reject(new TypeError('fetch: invalid response from native bridge: ' + e));
            return;
        }

        if (raw.error) {
            entry.reject(new TypeError('fetch failed: ' + raw.error));
        } else {
            var res = _makeResponse(raw);
            if (typeof PerformanceResourceTiming !== 'undefined' && typeof _perfEntries !== 'undefined' && typeof _p0 !== 'undefined') {
                var dur = Math.max(0, (Date.now() - _p0) - (entry.startTime || 0));
                _perfEntries.push(new PerformanceResourceTiming(entry.url || '', 'fetch', entry.startTime || 0, dur, raw.status || 200));
            }
            entry.resolve(res);
        }
    };

    function _makeResponse(raw) {
        var hdrs = new Headers();
        if (raw.headers) {
            for (var k in raw.headers) {
                hdrs.set(k, raw.headers[k]);
            }
        }
        if (!hdrs.has('content-type') && raw.contentType) {
            hdrs.set('content-type', raw.contentType);
        }

        var r = new Response(raw.body || '', {
            status: raw.status,
            statusText: raw.statusText || (raw.status === 200 ? 'OK' : String(raw.status)),
            headers: hdrs
        });
        r.ok = raw.ok;
        r.type = 'basic';
        r._raw = raw;

        if (raw.bodyBase64 && typeof atob === 'function') {
            r.arrayBuffer = function() {
                if (this.bodyUsed) return Promise.reject(new TypeError("Body has already been consumed"));
                this._bodyUsed = true;
                try {
                    var bin = atob(raw.bodyBase64);
                    var buf = new Uint8Array(bin.length);
                    for (var i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i);
                    return Promise.resolve(buf.buffer);
                } catch(e) {
                    var enc = new TextEncoder().encode(raw.body || '');
                    return Promise.resolve(enc.buffer);
                }
            };
            r.blob = function() {
                return this.arrayBuffer().then(function(buf) {
                    return new Blob([buf], { type: raw.contentType || 'application/octet-stream' });
                });
            };
        }
        return r;
    }

    globalThis.fetch = function(input, init) {
        var req;
        if (input instanceof Request) {
            req = init ? new Request(input, init) : input;
        } else {
            req = new Request(input, init);
        }
        var signal = req.signal;
        if (signal && signal.aborted) {
            return Promise.reject(signal.reason || new TypeError('The user aborted a request.'));
        }
        var url = req.url;
        var hdrs = {};
        req.headers.forEach(function(v, k) { hdrs[k] = v; });
        var pageOrigin = (globalThis.location && globalThis.location.origin) ? globalThis.location.origin : '';
        var reqMode = req.mode || (init && init.mode) || 'cors';
        var reqCreds = (req.credentials === 'include' || (init && init.credentials === 'include'));

        // Service Worker Fetch Event Interception
        if (globalThis.navigator && globalThis.navigator.serviceWorker && globalThis.navigator.serviceWorker.controller) {
            var sw = globalThis.navigator.serviceWorker.controller;
            if (!sw._handlingFetch && typeof FetchEvent === 'function') {
                try {
                    sw._handlingFetch = true;
                    var fe = new FetchEvent('fetch', { request: req });
                    sw.dispatchEvent(fe);
                    if (fe._responded && fe._responsePromise) {
                        return fe._responsePromise;
                    }
                } finally {
                    sw._handlingFetch = false;
                }
            }
        }

        function performFetch(bodyPayload) {
            return new Promise(function(resolve, reject) {
                var abortHandler = null;
                if (signal) {
                    abortHandler = function() {
                        reject(signal.reason || new TypeError('The user aborted a request.'));
                    };
                    signal.addEventListener('abort', abortHandler);
                }

                var fetchStart = (typeof _p0 !== 'undefined') ? (Date.now() - _p0) : 0;
                if (typeof _mangoStartFetch === 'function') {
                    var fetchId = _nextFetchId++;
                    _pendingFetches[fetchId] = {
                        resolve: function(res) {
                            if (signal && abortHandler && signal.removeEventListener) {
                                signal.removeEventListener('abort', abortHandler);
                            }
                            resolve(res);
                        },
                        reject: function(err) {
                            if (signal && abortHandler && signal.removeEventListener) {
                                signal.removeEventListener('abort', abortHandler);
                            }
                            reject(err);
                        },
                        url: url,
                        startTime: fetchStart
                    };
                    _mangoStartFetch(fetchId, url, req.method, JSON.stringify(hdrs), bodyPayload || '', pageOrigin, reqMode, reqCreds);
                } else if (typeof _mangoFetch === 'function') {
                    try {
                        var resultJson = _mangoFetch(url, req.method, JSON.stringify(hdrs), bodyPayload || '', pageOrigin, reqMode, reqCreds);
                        var raw = JSON.parse(resultJson);
                        if (raw.error) reject(new TypeError('fetch failed: ' + raw.error));
                        else {
                            var res = _makeResponse(raw);
                            if (typeof PerformanceResourceTiming !== 'undefined' && typeof _perfEntries !== 'undefined' && typeof _p0 !== 'undefined') {
                                var dur = Math.max(0, (Date.now() - _p0) - fetchStart);
                                _perfEntries.push(new PerformanceResourceTiming(url, 'fetch', fetchStart, dur, raw.status || 200));
                            }
                            resolve(res);
                        }
                    } catch(e) {
                        reject(new TypeError('fetch error: ' + (e.message || e)));
                    }
                } else {
                    reject(new TypeError('fetch: native bridge unavailable'));
                }
            });
        }

        if (req._bodyStream) {
            return _readAllChunksFromStream(req._bodyStream).then(function(u8) {
                var text = new TextDecoder().decode(u8);
                return performFetch(text);
            });
        }

        return performFetch(req._rawText || '');
    };
