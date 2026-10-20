    function ClipboardItem(items, options) {
        this._items = items || {};
        this.types = Object.keys(this._items);
        this.presentationStyle = (options && options.presentationStyle) || 'unspecified';
    }
    ClipboardItem.prototype.getType = function(type) {
        var item = this._items[type];
        if (item) {
            return Promise.resolve(item);
        }
        if (typeof DOMException !== 'undefined') {
            return Promise.reject(new DOMException('Type not found', 'NotFoundError'));
        }
        return Promise.reject(new Error('Type not found'));
    };
    globalThis.ClipboardItem = ClipboardItem;
    if (globalThis.window) globalThis.window.ClipboardItem = ClipboardItem;

    var _clipText = "";
    var _clipItems = [];

    // ── 3. navigator ─────────────────────────────────────────────────────────
    globalThis.navigator = {
        userAgent:          "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Mango/0.3.0",
        appName:            "Netscape",
        appVersion:         "5.0 (Windows NT 10.0; Win64; x64)",
        platform:           "Win32",
        language:           "en-US",
        languages:          ["en-US", "en"],
        onLine:             true,
        cookieEnabled:      true,
        vendor:             "Google Inc.",
        hardwareConcurrency: 4,
        maxTouchPoints:     0,
        sendBeacon:         function() { return false; },
        clipboard: {
            readText: function() {
                return Promise.resolve(_clipText);
            },
            writeText: function(text) {
                _clipText = String(text != null ? text : "");
                return Promise.resolve();
            },
            read: function() {
                return Promise.resolve(_clipItems.slice());
            },
            write: function(items) {
                if (Array.isArray(items)) {
                    _clipItems = items.slice();
                    for (var i = 0; i < items.length; i++) {
                        if (items[i] && items[i].types && items[i].types.indexOf('text/plain') !== -1) {
                            items[i].getType('text/plain').then(function(blob) {
                                if (blob && typeof blob.text === 'function') {
                                    blob.text().then(function(t) { _clipText = t; });
                                }
                            });
                        }
                    }
                }
                return Promise.resolve();
            }
        },
        userAgentData: {
            brands: [
                { brand: "Not-A.Brand", version: "24" },
                { brand: "Chromium", version: "124" },
                { brand: "Google Chrome", version: "124" }
            ],
            mobile: false,
            platform: "Windows",
            getHighEntropyValues: function(hints) {
                return Promise.resolve({
                    architecture: "x86",
                    bitness: "64",
                    brands: this.brands,
                    mobile: false,
                    model: "",
                    platform: "Windows",
                    platformVersion: "15.0.0",
                    uaFullVersion: "124.0.0.0"
                });
            },
            toJSON: function() {
                return { brands: this.brands, mobile: this.mobile, platform: this.platform };
            }
        },
        connection: {
            effectiveType: "4g",
            rtt: 50,
            downlink: 10,
            saveData: false,
            addEventListener: function() {},
            removeEventListener: function() {}
        },
        permissions: {
            query: function(desc) {
                return Promise.resolve({ state: "granted", addEventListener: function() {}, removeEventListener: function() {} });
            }
        },
        mediaCapabilities: {
            decodingInfo: function() { return Promise.resolve({ supported: true, smooth: true, powerEfficient: true }); },
            encodingInfo: function() { return Promise.resolve({ supported: true, smooth: true, powerEfficient: true }); }
        },
        mediaDevices: {
            getUserMedia: function() { return Promise.reject(new Error("NotSupportedError")); },
            enumerateDevices: function() { return Promise.resolve([]); },
            getDisplayMedia: function() { return Promise.reject(new Error("NotSupportedError")); },
            addEventListener: function() {},
            removeEventListener: function() {},
            dispatchEvent: function() { return true; }
        },
        storage: {
            estimate: function() { return Promise.resolve({ quota: 1000000000, usage: 0 }); },
            persist:  function() { return Promise.resolve(true); }
        }
    };

    // ── 4. screen ────────────────────────────────────────────────────────────
    globalThis.screen = {
        width:       1920,
        height:      1080,
        availWidth:  1920,
        availHeight: 1040,
        colorDepth:  24,
        pixelDepth:  24
    };

    // ── 5. history ───────────────────────────────────────────────────────────
    function History() {}
    History.prototype = {
        constructor: History,
        get length() { return _historyStack.length; },
        get state() { return _historyStack[_historyIdx] ? _historyStack[_historyIdx].state : null; },
        get scrollRestoration() { return this._scrollRestoration || 'auto'; },
        set scrollRestoration(v) { this._scrollRestoration = (v === 'manual') ? 'manual' : 'auto'; },
        back: function() { this.go(-1); },
        forward: function() { this.go(1); },
        go: function(n) {
            var delta = parseInt(n, 10) || 0;
            if (delta === 0) {
                location.reload();
                return;
            }
            var targetIdx = _historyIdx + delta;
            if (targetIdx >= 0 && targetIdx < _historyStack.length && targetIdx !== _historyIdx) {
                var oldHref = _curHref;
                var oldHash = _locParsed.hash;
                _historyIdx = targetIdx;
                var entry = _historyStack[_historyIdx];
                _updateLocationHref(entry.url, false, false);
                if (typeof window !== 'undefined' && window.dispatchEvent && typeof PopStateEvent === 'function') {
                    window.dispatchEvent(new PopStateEvent('popstate', {
                        state: entry.state
                    }));
                }
            }
        },
        pushState: function(state, title, url) {
            var newUrl = _curHref;
            if (url !== undefined && url !== null && url !== '') {
                var resolved = _parseUrlComponents(url, _curHref);
                if (_locParsed.origin !== 'null' && resolved.origin !== 'null' && resolved.origin !== _locParsed.origin) {
                    var err = typeof DOMException !== 'undefined' 
                        ? new DOMException("Failed to execute 'pushState' on 'History': A history state object cannot be added with URL of different origin.", "SecurityError")
                        : new Error("SecurityError: Failed to execute 'pushState' on 'History': A history state object cannot be added with URL of different origin.");
                    err.name = "SecurityError";
                    throw err;
                }
                newUrl = resolved.href;
            }
            var clonedState = null;
            if (state !== undefined && state !== null) {
                try {
                    clonedState = JSON.parse(JSON.stringify(state));
                } catch(e) {
                    clonedState = state;
                }
            }
            _historyStack.splice(_historyIdx + 1);
            _historyStack.push({ url: newUrl, state: clonedState });
            _historyIdx = _historyStack.length - 1;
            _updateLocationHref(newUrl, false, false);
        },
        replaceState: function(state, title, url) {
            var newUrl = _curHref;
            if (url !== undefined && url !== null && url !== '') {
                var resolved = _parseUrlComponents(url, _curHref);
                if (_locParsed.origin !== 'null' && resolved.origin !== 'null' && resolved.origin !== _locParsed.origin) {
                    var err = typeof DOMException !== 'undefined' 
                        ? new DOMException("Failed to execute 'replaceState' on 'History': A history state object cannot be added with URL of different origin.", "SecurityError")
                        : new Error("SecurityError: Failed to execute 'replaceState' on 'History': A history state object cannot be added with URL of different origin.");
                    err.name = "SecurityError";
                    throw err;
                }
                newUrl = resolved.href;
            }
            var clonedState = null;
            if (state !== undefined && state !== null) {
                try {
                    clonedState = JSON.parse(JSON.stringify(state));
                } catch(e) {
                    clonedState = state;
                }
            }
            _historyStack[_historyIdx] = { url: newUrl, state: clonedState };
            _updateLocationHref(newUrl, false, false);
        }
    };

    var _hist = Object.create(History.prototype);
    _hist._scrollRestoration = 'auto';
    globalThis.History = History;
    globalThis.history = _hist;

    // ── 6. performance ───────────────────────────────────────────────────────
    var _p0 = Date.now();
    var _perfEntries = [];
    function PerformanceEntry(name, entryType, startTime, duration) {
        this.name = String(name || '');
        this.entryType = String(entryType || '');
        this.startTime = Number(startTime || 0);
        this.duration = Number(duration || 0);
    }
    PerformanceEntry.prototype.toJSON = function() {
        return {
            name: this.name,
            entryType: this.entryType,
            startTime: this.startTime,
            duration: this.duration
        };
    };
    globalThis.PerformanceEntry = PerformanceEntry;

    function PerformanceResourceTiming(name, initiatorType, startTime, duration, responseStatus) {
        PerformanceEntry.call(this, name, 'resource', startTime, duration);
        this.initiatorType = initiatorType || 'fetch';
        this.responseStatus = responseStatus || 200;
        this.transferSize = 0;
        this.encodedBodySize = 0;
        this.decodedBodySize = 0;
        this.fetchStart = startTime;
        this.responseEnd = startTime + duration;
    }
    PerformanceResourceTiming.prototype = Object.create(PerformanceEntry.prototype);
    PerformanceResourceTiming.prototype.constructor = PerformanceResourceTiming;
    globalThis.PerformanceResourceTiming = PerformanceResourceTiming;

    globalThis.performance = {
        now:         function() { return Date.now() - _p0; },
        timeOrigin:  _p0,
        timing:      { navigationStart: _p0, loadEventEnd: _p0 + 50 },
        mark:        function(name) {
            var entry = new PerformanceEntry(name, 'mark', Date.now() - _p0, 0);
            _perfEntries.push(entry);
            return entry;
        },
        measure:     function(name, startMark, endMark) {
            var start = 0;
            var end = Date.now() - _p0;
            if (startMark) {
                for (var i = _perfEntries.length - 1; i >= 0; i--) {
                    if (_perfEntries[i].name === startMark) { start = _perfEntries[i].startTime; break; }
                }
            }
            if (endMark) {
                for (var i = _perfEntries.length - 1; i >= 0; i--) {
                    if (_perfEntries[i].name === endMark) { end = _perfEntries[i].startTime; break; }
                }
            }
            var entry = new PerformanceEntry(name, 'measure', start, Math.max(0, end - start));
            _perfEntries.push(entry);
            return entry;
        },
        clearMarks:  function(name) {
            _perfEntries = _perfEntries.filter(function(e) {
                return e.entryType !== 'mark' || (name && e.name !== name);
            });
        },
        clearMeasures: function(name) {
            _perfEntries = _perfEntries.filter(function(e) {
                return e.entryType !== 'measure' || (name && e.name !== name);
            });
        },
        clearResourceTimings: function() {
            _perfEntries = _perfEntries.filter(function(e) { return e.entryType !== 'resource'; });
        },
        getEntries:  function() { return _perfEntries.slice(); },
        getEntriesByName: function(name, type) {
            return _perfEntries.filter(function(e) {
                return e.name === name && (!type || e.entryType === type);
            });
        },
        getEntriesByType: function(type) {
            return _perfEntries.filter(function(e) { return e.entryType === type; });
        }
    };

    // ── 7. localStorage (backed by native Rust HashMap) ───────────────────────
    //
    // Reads/writes go through _mangoLs* native functions so the browser can
    // persist the map to disk across page loads.
    function Storage() {}
    var _storageProto = {
        getItem: function(k) { return _mangoLsGet(String(k)); },
        setItem: function(k, v) {
            var old = _mangoLsGet(String(k));
            _mangoLsSet(String(k), String(v));
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {
                window.dispatchEvent(new StorageEvent('storage', {
                    key: String(k),
                    oldValue: old,
                    newValue: String(v),
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }));
            }
        },
        removeItem: function(k) {
            var old = _mangoLsGet(String(k));
            _mangoLsRemove(String(k));
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {
                window.dispatchEvent(new StorageEvent('storage', {
                    key: String(k),
                    oldValue: old,
                    newValue: null,
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }));
            }
        },
        clear: function() {
            _mangoLsClear();
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {
                window.dispatchEvent(new StorageEvent('storage', {
                    key: null,
                    oldValue: null,
                    newValue: null,
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }));
            }
        },
        get length() {
            var keys = _mangoLsKeys();
            return keys ? keys.length : 0;
        },
        key: function(i) {
            var keys = _mangoLsKeys();
            return (keys && i >= 0 && i < keys.length) ? keys[i] : null;
        }
    };
    Storage.prototype = _storageProto;
    globalThis.Storage = Storage;

    var _lsBase = Object.create(Storage.prototype);
    if (typeof Proxy !== 'undefined') {
        globalThis.localStorage = new Proxy(_lsBase, {
            get: function(target, prop) {
                if (prop in target || typeof prop === 'symbol') {
                    return target[prop];
                }
                var val = target.getItem(String(prop));
                return val !== null ? val : undefined;
            },
            set: function(target, prop, value) {
                if (typeof prop === 'symbol' || prop in Storage.prototype) {
                    target[prop] = value;
                } else {
                    target.setItem(String(prop), String(value));
                }
                return true;
            },
            deleteProperty: function(target, prop) {
                if (typeof prop === 'symbol' || prop in Storage.prototype) {
                    delete target[prop];
                } else {
                    target.removeItem(String(prop));
                }
                return true;
            },
            has: function(target, prop) {
                if (prop in target) {
                    return true;
                }
                return target.getItem(String(prop)) !== null;
            },
            ownKeys: function(target) {
                var keys = Reflect.ownKeys(target);
                var len = 0;
                try { len = target.length; } catch (e) {}
                for (var i = 0; i < len; i++) {
                    var k = target.key(i);
                    if (k !== null && keys.indexOf(k) === -1) keys.push(k);
                }
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                if (prop in target || typeof prop === 'symbol') {
                    return Object.getOwnPropertyDescriptor(target, prop) || {
                        value: target[prop],
                        writable: true,
                        enumerable: false,
                        configurable: true
                    };
                }
                var val = target.getItem(String(prop));
                if (val !== null) {
                    return {
                        value: val,
                        writable: true,
                        enumerable: true,
                        configurable: true
                    };
                }
                return undefined;
            }
        });
    } else {
        globalThis.localStorage = _lsBase;
    }

    // sessionStorage: tab-scoped in-memory, partitioned per origin.
    var _ssBase = Object.create(Storage.prototype);
    _ssBase.getItem = function(k) { return _mangoSsGet(String(k)); };
    _ssBase.setItem = function(k, v) {
        var old = _mangoSsGet(String(k));
        _mangoSsSet(String(k), String(v));
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {
            window.dispatchEvent(new StorageEvent('storage', {
                key: String(k),
                oldValue: old,
                newValue: String(v),
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }));
        }
    };
    _ssBase.removeItem = function(k) {
        var old = _mangoSsGet(String(k));
        _mangoSsRemove(String(k));
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {
            window.dispatchEvent(new StorageEvent('storage', {
                key: String(k),
                oldValue: old,
                newValue: null,
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }));
        }
    };
    _ssBase.clear = function() {
        _mangoSsClear();
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {
            window.dispatchEvent(new StorageEvent('storage', {
                key: null,
                oldValue: null,
                newValue: null,
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }));
        }
    };
    Object.defineProperty(_ssBase, 'length', {
        get: function() {
            var keys = _mangoSsKeys();
            return keys ? keys.length : 0;
        },
        configurable: true
    });
    _ssBase.key = function(i) {
        var keys = _mangoSsKeys();
        return (keys && i >= 0 && i < keys.length) ? keys[i] : null;
    };

    if (typeof Proxy !== 'undefined') {
        globalThis.sessionStorage = new Proxy(_ssBase, {
            get: function(target, prop) {
                if (prop in target || typeof prop === 'symbol') {
                    return target[prop];
                }
                var val = target.getItem(String(prop));
                return val !== null ? val : undefined;
            },
            set: function(target, prop, value) {
                if (typeof prop === 'symbol' || prop in Storage.prototype) {
                    target[prop] = value;
                } else {
                    target.setItem(String(prop), String(value));
                }
                return true;
            },
            deleteProperty: function(target, prop) {
                if (typeof prop === 'symbol' || prop in Storage.prototype) {
                    delete target[prop];
                } else {
                    target.removeItem(String(prop));
                }
                return true;
            },
            has: function(target, prop) {
                if (prop in target) {
                    return true;
                }
                return target.getItem(String(prop)) !== null;
            },
            ownKeys: function(target) {
                var keys = Reflect.ownKeys(target);
                var len = 0;
                try { len = target.length; } catch (e) {}
                for (var i = 0; i < len; i++) {
                    var k = target.key(i);
                    if (k !== null && keys.indexOf(k) === -1) keys.push(k);
                }
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                if (prop in target || typeof prop === 'symbol') {
                    return Object.getOwnPropertyDescriptor(target, prop) || {
                        value: target[prop],
                        writable: true,
                        enumerable: false,
                        configurable: true
                    };
                }
                var val = target.getItem(String(prop));
                if (val !== null) {
                    return {
                        value: val,
                        writable: true,
                        enumerable: true,
                        configurable: true
                    };
                }
                return undefined;
            }
        });
    } else {
        globalThis.sessionStorage = _ssBase;
    }

    // ── 7b. EventTarget base class ───────────────────────────────────────────
    function EventTarget() {
        this._listeners = {};
    }
    EventTarget.prototype = {
        addEventListener: function(type, listener, opts) {
            if (typeof globalThis._addDOMEventListener === 'function') {
                globalThis._addDOMEventListener(this, type, listener, opts);
            } else {
                if (!listener) return;
                var t = String(type);
                if (!this._listeners) this._listeners = {};
                if (!this._listeners[t]) this._listeners[t] = [];
                var cap = false, onc = false;
                if (typeof opts === 'boolean') cap = opts;
                else if (opts && typeof opts === 'object') { cap = !!opts.capture; onc = !!opts.once; }
                for (var i = 0; i < this._listeners[t].length; i++) {
                    if (this._listeners[t][i].fn === listener && this._listeners[t][i].capture === cap) return;
                }
                this._listeners[t].push({ fn: listener, capture: cap, once: onc });
            }
        },
        removeEventListener: function(type, listener, opts) {
            if (typeof globalThis._removeDOMEventListener === 'function') {
                globalThis._removeDOMEventListener(this, type, listener, opts);
            } else {
                if (!this._listeners) return;
                var t = String(type);
                if (!this._listeners[t]) return;
                var cap = false;
                if (typeof opts === 'boolean') cap = opts;
                else if (opts && typeof opts === 'object') cap = !!opts.capture;
                this._listeners[t] = this._listeners[t].filter(function(item) {
                    return !(item.fn === listener && item.capture === cap);
                });
            }
        },
        dispatchEvent: function(evt) {
            if (typeof globalThis._dispatchDOMEvent === 'function') {
                return globalThis._dispatchDOMEvent(this, evt);
            } else {
                if (!evt) return true;
                var t = String(evt.type || evt);
                var handler = 'on' + t;
                if (typeof this[handler] === 'function') {
                    try { this[handler].call(this, evt); } catch(e) {}
                }
                if (!this._listeners || !this._listeners[t]) return true;
                var list = this._listeners[t].slice();
                for (var i = 0; i < list.length; i++) {
                    var item = list[i];
                    if (item.once) this.removeEventListener(t, item.fn, { capture: item.capture });
                    try {
                        if (typeof item.fn === 'function') item.fn.call(this, evt);
                        else if (item.fn && typeof item.fn.handleEvent === 'function') item.fn.handleEvent(evt);
                    } catch(e) {}
                }
                return true;
            }
        }
    };
    globalThis.EventTarget = EventTarget;

