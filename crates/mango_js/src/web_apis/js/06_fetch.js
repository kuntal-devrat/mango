    // ── 21. fetch() ───────────────────────────────────────────────────────────
    function Headers(init) {
        this._map = {};
        var self = this;
        if (init) {
            if (init instanceof Headers || (init._map && typeof init._map === 'object')) {
                for (var k in init._map) {
                    this._map[k] = init._map[k];
                }
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
    }
    Headers.prototype = {
        constructor: Headers,
        append: function(k, v) {
            var lk = String(k).toLowerCase();
            this._map[lk] = this._map[lk] ? this._map[lk] + ', ' + String(v) : String(v);
        },
        delete: function(k) {
            delete this._map[String(k).toLowerCase()];
        },
        get: function(k) {
            var lk = String(k).toLowerCase();
            return this._map[lk] !== undefined ? this._map[lk] : null;
        },
        getAll: function(k) {
            var val = this.get(k);
            if (val === null) return [];
            return val.split(', ');
        },
        getSetCookie: function() {
            var c = this.get('set-cookie');
            return c ? c.split(', ') : [];
        },
        has: function(k) {
            return String(k).toLowerCase() in this._map;
        },
        set: function(k, v) {
            this._map[String(k).toLowerCase()] = String(v);
        },
        forEach: function(cb, thisArg) {
            for (var k in this._map) {
                cb.call(thisArg, this._map[k], k, this);
            }
        },
        keys: function() {
            var keys = Object.keys(this._map);
            var i = 0;
            var it = {
                next: function() {
                    if (i < keys.length) return { value: keys[i++], done: false };
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        },
        values: function() {
            var keys = Object.keys(this._map);
            var self = this;
            var i = 0;
            var it = {
                next: function() {
                    if (i < keys.length) return { value: self._map[keys[i++]], done: false };
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        },
        entries: function() {
            var keys = Object.keys(this._map);
            var self = this;
            var i = 0;
            var it = {
                next: function() {
                    if (i < keys.length) {
                        var k = keys[i++];
                        return { value: [k, self._map[k]], done: false };
                    }
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        }
    };
    Headers.prototype[Symbol.iterator] = Headers.prototype.entries;
    globalThis.Headers = Headers;

    function Request(input, init) {
        init = init || {};
        if (input instanceof Request) {
            this.url = input.url;
            this.method = init.method ? String(init.method).toUpperCase() : input.method;
            this.headers = new Headers(init.headers || input.headers);
            this.body = init.body !== undefined ? init.body : input.body;
            this.signal = init.signal || input.signal;
        } else {
            this.url = String(input || '');
            this.method = init.method ? String(init.method).toUpperCase() : 'GET';
            this.headers = new Headers(init.headers);
            this.body = init.body !== undefined ? init.body : null;
            this.signal = init.signal || null;
        }
        this.mode = init.mode || 'cors';
        this.credentials = init.credentials || 'same-origin';
        this.cache = init.cache || 'default';
        this.redirect = init.redirect || 'follow';
        this.referrer = init.referrer || 'about:client';
        this.destination = init.destination || '';
        this.bodyUsed = false;
    }
    Request.prototype = {
        clone: function() {
            return new Request(this, {});
        },
        text: function() {
            this.bodyUsed = true;
            return Promise.resolve(String(this.body || ''));
        },
        json: function() {
            this.bodyUsed = true;
            try { return Promise.resolve(JSON.parse(this.body || 'null')); }
            catch(e) { return Promise.reject(e); }
        }
    };
    globalThis.Request = Request;

    function Response(body, init) {
        if (body instanceof ReadableStream) {
            this._bodyStream = body;
            this._body = null;
        } else {
            this._body = body !== undefined && body !== null ? body : '';
            this._bodyStream = null;
        }
        this.status = (init && init.status !== undefined) ? Number(init.status) : 200;
        this.statusText = (init && init.statusText !== undefined) ? String(init.statusText) : (this.status === 200 ? 'OK' : String(this.status));
        this.ok = this.status >= 200 && this.status < 300;
        this.headers = new Headers(init && init.headers);
        this.url = (init && init.url) ? String(init.url) : '';
        this.type = (init && init.type) ? String(init.type) : 'default';
        this.redirected = !!(init && init.redirected);
        this._bodyUsed = false;
    }
    Object.defineProperty(Response.prototype, 'bodyUsed', {
        get: function() {
            return !!this._bodyUsed || (this._bodyStream ? this._bodyStream._disturbed : false);
        },
        set: function(val) {
            this._bodyUsed = !!val;
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(Response.prototype, 'body', {
        get: function() {
            if (this._bodyStream) return this._bodyStream;
            var self = this;
            var rawBody = this._body;
            var rawBase64 = this._raw && this._raw.bodyBase64;
            this._bodyStream = new ReadableStream({
                start: function(controller) {
                    var chunk = null;
                    if (rawBase64 && typeof atob === 'function') {
                        try {
                            var bin = atob(rawBase64);
                            chunk = new Uint8Array(bin.length);
                            for (var i = 0; i < bin.length; i++) chunk[i] = bin.charCodeAt(i);
                        } catch(e) {}
                    }
                    if (!chunk) {
                        if (rawBody instanceof Uint8Array) {
                            chunk = rawBody;
                        } else if (rawBody instanceof ArrayBuffer) {
                            chunk = new Uint8Array(rawBody);
                        } else if (ArrayBuffer.isView(rawBody)) {
                            chunk = new Uint8Array(rawBody.buffer, rawBody.byteOffset, rawBody.byteLength);
                        } else if (typeof rawBody === 'string' && rawBody.length > 0) {
                            chunk = new TextEncoder().encode(rawBody);
                        }
                    }
                    if (chunk && chunk.length > 0) {
                        controller.enqueue(chunk);
                    }
                    controller.close();
                }
            });
            return this._bodyStream;
        },
        configurable: true,
        enumerable: true
    });
    Response.prototype.text = function() {
        if (this.bodyUsed && !this._bodyStream) {
            return Promise.reject(new TypeError("Body already used"));
        }
        this.bodyUsed = true;
        if (this._bodyStream && this._body === null) {
            var reader = this._bodyStream.getReader();
            var chunks = [];
            function drain() {
                return reader.read().then(function(res) {
                    if (res.done) {
                        var totalLen = 0;
                        for (var i = 0; i < chunks.length; i++) totalLen += chunks[i].length;
                        var merged = new Uint8Array(totalLen);
                        var off = 0;
                        for (var i = 0; i < chunks.length; i++) {
                            merged.set(chunks[i], off);
                            off += chunks[i].length;
                        }
                        return new TextDecoder().decode(merged);
                    }
                    if (typeof res.value === 'string') {
                        chunks.push(new TextEncoder().encode(res.value));
                    } else if (res.value instanceof Uint8Array) {
                        chunks.push(res.value);
                    } else if (res.value && res.value.buffer) {
                        chunks.push(new Uint8Array(res.value.buffer));
                    }
                    return drain();
                });
            }
            return drain();
        }
        if (typeof this._body === 'string') return Promise.resolve(this._body);
        if (this._body instanceof Blob) return this._body.text();
        if (this._body instanceof ArrayBuffer || ArrayBuffer.isView(this._body)) {
            return Promise.resolve(new TextDecoder().decode(this._body));
        }
        return Promise.resolve(String(this._body));
    };
    Response.prototype.json = function() {
        var self = this;
        return this.text().then(function(t) {
            return JSON.parse(t);
        });
    };
    Response.prototype.blob = function() {
        this.bodyUsed = true;
        if (this._body instanceof Blob) return Promise.resolve(this._body);
        var ct = this.headers.get('content-type') || 'text/plain';
        return Promise.resolve(new Blob([this._body], { type: ct }));
    };
    Response.prototype.arrayBuffer = function() {
        this.bodyUsed = true;
        if (this._body instanceof ArrayBuffer) return Promise.resolve(this._body);
        if (ArrayBuffer.isView(this._body)) return Promise.resolve(this._body.buffer);
        if (this._body instanceof Blob) return this._body.arrayBuffer();
        var enc = new TextEncoder().encode(String(this._body));
        return Promise.resolve(enc.buffer);
    };
    Response.prototype.formData = function() {
        this.bodyUsed = true;
        var fd = new FormData();
        return this.text().then(function(t) {
            var params = new URLSearchParams(t);
            params.forEach(function(v, k) { fd.append(k, v); });
            return fd;
        });
    };
    Response.prototype.clone = function() {
        return new Response(this._body, {
            status: this.status,
            statusText: this.statusText,
            headers: this.headers,
            url: this.url,
            type: this.type,
            redirected: this.redirected
        });
    };
    Response.error = function() {
        var r = new Response(null, { status: 0, statusText: '' });
        r.type = 'error';
        r.ok = false;
        return r;
    };
    Response.redirect = function(url, status) {
        status = status || 302;
        if ([301, 302, 303, 307, 308].indexOf(status) === -1) {
            throw new RangeError('Invalid redirect status');
        }
        return new Response(null, { status: status, headers: { Location: String(url) } });
    };
    Response.json = function(data, init) {
        var body = JSON.stringify(data);
        var hdrs = new Headers(init && init.headers);
        if (!hdrs.has('content-type')) hdrs.set('content-type', 'application/json');
        var opts = Object.assign({}, init, { headers: hdrs });
        return new Response(body, opts);
    };
    globalThis.Response = Response;

    // ── Cache API (caches.open, caches.match, cache.put, cache.delete, cache.keys) ──
    function Cache() {
        this._items = [];
    }
    Cache.prototype = {
        match: function(request, options) {
            var url = (request instanceof Request) ? request.url : String(request);
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = ignoreSearch ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (cmpUrl === target) {
                    return Promise.resolve(this._items[i].res.clone());
                }
            }
            return Promise.resolve(undefined);
        },
        matchAll: function(request, options) {
            var results = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = (url && ignoreSearch) ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (!target || cmpUrl === target) {
                    results.push(this._items[i].res.clone());
                }
            }
            return Promise.resolve(results);
        },
        add: function(request) {
            return this.addAll([request]);
        },
        addAll: function(requests) {
            var self = this;
            var promises = requests.map(function(r) { return fetch(r); });
            return Promise.all(promises).then(function(responses) {
                for (var i = 0; i < requests.length; i++) {
                    var req = (requests[i] instanceof Request) ? requests[i] : new Request(requests[i]);
                    self.put(req, responses[i]);
                }
            });
        },
        put: function(request, response) {
            var req = (request instanceof Request) ? request : new Request(request);
            var res = response.clone();
            for (var i = 0; i < this._items.length; i++) {
                if (this._items[i].req.url === req.url) {
                    this._items[i] = { req: req, res: res };
                    return Promise.resolve();
                }
            }
            this._items.push({ req: req, res: res });
            return Promise.resolve();
        },
        delete: function(request, options) {
            var url = (request instanceof Request) ? request.url : String(request);
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = ignoreSearch ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (cmpUrl === target) {
                    this._items.splice(i, 1);
                    return Promise.resolve(true);
                }
            }
            return Promise.resolve(false);
        },
        keys: function(request, options) {
            var keys = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = (url && ignoreSearch) ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (!target || cmpUrl === target) {
                    keys.push(this._items[i].req.clone());
                }
            }
            return Promise.resolve(keys);
        }
    };
    globalThis.Cache = Cache;

    function CacheStorage() {
        this._caches = {};
    }
    CacheStorage.prototype = {
        open: function(cacheName) {
            var name = String(cacheName);
            if (!this._caches[name]) {
                this._caches[name] = new Cache();
            }
            return Promise.resolve(this._caches[name]);
        },
        has: function(cacheName) {
            return Promise.resolve(Object.prototype.hasOwnProperty.call(this._caches, String(cacheName)));
        },
        delete: function(cacheName) {
            var name = String(cacheName);
            if (Object.prototype.hasOwnProperty.call(this._caches, name)) {
                delete this._caches[name];
                return Promise.resolve(true);
            }
            return Promise.resolve(false);
        },
        keys: function() {
            return Promise.resolve(Object.keys(this._caches));
        },
        match: function(request, options) {
            var self = this;
            if (options && options.cacheName) {
                var single = this._caches[String(options.cacheName)];
                if (single) return single.match(request, options);
                return Promise.resolve(undefined);
            }
            var keys = Object.keys(this._caches);
            function checkNext(idx) {
                if (idx >= keys.length) return Promise.resolve(undefined);
                return self._caches[keys[idx]].match(request, options).then(function(res) {
                    if (res) return res;
                    return checkNext(idx + 1);
                });
            }
            return checkNext(0);
        }
    };
    globalThis.CacheStorage = CacheStorage;
    globalThis.caches = new CacheStorage();

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
                this.bodyUsed = true;
                try {
                    var bin = atob(raw.bodyBase64);
                    var buf = new Uint8Array(bin.length);
                    for (var i = 0; i < bin.length; i++) {
                        buf[i] = bin.charCodeAt(i);
                    }
                    return Promise.resolve(buf.buffer);
                } catch(e) {
                    var enc = new TextEncoder().encode(raw.body || '');
                    return Promise.resolve(enc.buffer);
                }
            };
            r.blob = function() {
                this.bodyUsed = true;
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
            return Promise.reject(new Error('The user aborted a request.'));
        }
        var url = req.url;
        if (url && url.indexOf('://') === -1 && url.indexOf('//') !== 0) {
            try {
                var base = globalThis.location ? globalThis.location.origin + (globalThis.location.pathname || '/') : '';
                url = new URL(url, base).href;
            } catch(e) {}
        }
        
        var hdrs = {};
        req.headers.forEach(function(v, k) { hdrs[k] = v; });
        var bodyStr = (req.body !== undefined && req.body !== null && !(req.body instanceof ReadableStream)) ? String(req.body) : '';
        var pageOrigin = (globalThis.location && globalThis.location.origin) ? globalThis.location.origin : '';
        var reqMode = req.mode || (init && init.mode) || 'cors';
        var reqCreds = (req.credentials === 'include' || (init && init.credentials === 'include'));
        if (init && init._swBypass) req._swBypass = true;

        // Service Worker Fetch Event Interception
        if (!req._swBypass && typeof FetchEvent !== 'undefined' && globalThis.navigator && globalThis.navigator.serviceWorker && globalThis.navigator.serviceWorker.controller) {
            var sw = globalThis.navigator.serviceWorker.controller;
            var hasFetchHandler = (sw._listeners && sw._listeners['fetch'] && sw._listeners['fetch'].length > 0) || (typeof sw.onfetch === 'function');
            if (hasFetchHandler && !sw._handlingFetch) {
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
                if (signal) {
                    signal.addEventListener('abort', function() {
                        reject(new Error('The user aborted a request.'));
                    });
                }

                var fetchStart = (typeof _p0 !== 'undefined') ? (Date.now() - _p0) : 0;
                if (typeof _mangoStartFetch === 'function') {
                    var fetchId = _nextFetchId++;
                    _pendingFetches[fetchId] = { resolve: resolve, reject: reject, url: url, startTime: fetchStart };
                    _mangoStartFetch(fetchId, url, req.method, JSON.stringify(hdrs), bodyPayload, pageOrigin, reqMode, reqCreds);
                } else if (typeof _mangoFetch === 'function') {
                    try {
                        var resultJson = _mangoFetch(url, req.method, JSON.stringify(hdrs), bodyPayload, pageOrigin, reqMode, reqCreds);
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

        if (req.body instanceof ReadableStream) {
            var reader = req.body.getReader();
            var streamChunks = [];
            function drainStream() {
                return reader.read().then(function(res) {
                    if (res.done) {
                        var totalLen = 0;
                        for (var i = 0; i < streamChunks.length; i++) totalLen += streamChunks[i].length;
                        var merged = new Uint8Array(totalLen);
                        var off = 0;
                        for (var i = 0; i < streamChunks.length; i++) {
                            merged.set(streamChunks[i], off);
                            off += streamChunks[i].length;
                        }
                        return new TextDecoder().decode(merged);
                    }
                    if (typeof res.value === 'string') {
                        streamChunks.push(new TextEncoder().encode(res.value));
                    } else if (res.value instanceof Uint8Array) {
                        streamChunks.push(res.value);
                    } else if (res.value && res.value.buffer) {
                        streamChunks.push(new Uint8Array(res.value.buffer));
                    }
                    return drainStream();
                });
            }
            return drainStream().then(function(drainedText) {
                return performFetch(drainedText);
            });
        }

        return performFetch(bodyStr);
    };

