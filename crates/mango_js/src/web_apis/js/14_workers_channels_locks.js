    // ── 13b. ServiceWorker API (W3C Service Workers) ──────────────────────────
    function ExtendableEvent(type, eventInitDict) {
        Event.call(this, type, eventInitDict);
        this._extendLifetimePromises = [];
    }
    ExtendableEvent.prototype = Object.create(Event.prototype);
    ExtendableEvent.prototype.waitUntil = function(p) {
        this._extendLifetimePromises.push(Promise.resolve(p));
    };
    globalThis.ExtendableEvent = ExtendableEvent;

    function FetchEvent(type, init) {
        ExtendableEvent.call(this, type, init);
        this.request = (init && init.request) ? (init.request instanceof Request ? init.request : new Request(init.request)) : null;
        this.clientId = (init && init.clientId) || '';
        this.isReload = !!(init && init.isReload);
        this.resultingClientId = (init && init.resultingClientId) || '';
        this.handled = Promise.resolve();
        this.preloadResponse = Promise.resolve(undefined);
        this._responsePromise = null;
        this._responded = false;
    }
    FetchEvent.prototype = Object.create(ExtendableEvent.prototype);
    FetchEvent.prototype.respondWith = function(r) {
        this._responded = true;
        this._responsePromise = Promise.resolve(r);
    };
    globalThis.FetchEvent = FetchEvent;

    function ExtendableMessageEvent(type, init) {
        ExtendableEvent.call(this, type, init);
        this.data = (init && init.data) || null;
        this.origin = (init && init.origin) || '';
        this.lastEventId = (init && init.lastEventId) || '';
        this.source = (init && init.source) || null;
        this.ports = (init && init.ports) || [];
    }
    ExtendableMessageEvent.prototype = Object.create(ExtendableEvent.prototype);
    globalThis.ExtendableMessageEvent = ExtendableMessageEvent;

    function ServiceWorker(scriptURL, state) {
        EventTarget.call(this);
        this.scriptURL = scriptURL || '';
        this.state = state || 'activated';
        this.onstatechange = null;
        this.onerror = null;
        this.onfetch = null;
        this.oninstall = null;
        this.onactivate = null;
        this.onmessage = null;
        this._handlingFetch = false;
    }
    ServiceWorker.prototype = Object.create(EventTarget.prototype);
    ServiceWorker.prototype._setState = function(newState) {
        this.state = newState;
        var evt = new Event('statechange');
        if (typeof this.onstatechange === 'function') {
            try { this.onstatechange.call(this, evt); } catch(e) {}
        }
        this.dispatchEvent(evt);
    };
    ServiceWorker.prototype.postMessage = function(msg, transfer) {
        var self = this;
        var evt = new ExtendableMessageEvent('message', { data: msg, source: self });
        setTimeout(function() {
            if (typeof self.onmessage === 'function') {
                try { self.onmessage.call(self, evt); } catch(e) {}
            }
            self.dispatchEvent(evt);
        }, 0);
    };
    globalThis.ServiceWorker = ServiceWorker;

    function NavigationPreloadManager() {
        this._enabled = false;
        this._header = '';
    }
    NavigationPreloadManager.prototype.enable = function() { this._enabled = true; return Promise.resolve(); };
    NavigationPreloadManager.prototype.disable = function() { this._enabled = false; return Promise.resolve(); };
    NavigationPreloadManager.prototype.setHeaderValue = function(val) { this._header = String(val); return Promise.resolve(); };
    NavigationPreloadManager.prototype.getState = function() { return Promise.resolve({ enabled: this._enabled, headerValue: this._header }); };
    globalThis.NavigationPreloadManager = NavigationPreloadManager;

    function ServiceWorkerRegistration(scope, sw) {
        EventTarget.call(this);
        this.scope = scope || '/';
        var activeWorker = sw || new ServiceWorker('', 'activated');
        this.installing = null;
        this.waiting = null;
        this.active = activeWorker;
        this.navigationPreload = new NavigationPreloadManager();
        this.onupdatefound = null;
    }
    ServiceWorkerRegistration.prototype = Object.create(EventTarget.prototype);
    ServiceWorkerRegistration.prototype.update = function() { return Promise.resolve(this); };
    ServiceWorkerRegistration.prototype.unregister = function() { return Promise.resolve(true); };
    ServiceWorkerRegistration.prototype.showNotification = function(title, options) { return Promise.resolve(); };
    ServiceWorkerRegistration.prototype.getNotifications = function() { return Promise.resolve([]); };
    globalThis.ServiceWorkerRegistration = ServiceWorkerRegistration;

    function ServiceWorkerContainer() {
        EventTarget.call(this);
        var self = this;
        var defaultActive = new ServiceWorker('/', 'activated');
        var defaultReg = new ServiceWorkerRegistration('/', defaultActive);
        this.controller = defaultActive;
        this._registrations = { '/': defaultReg };
        this.ready = Promise.resolve(defaultReg);
        this.oncontrollerchange = null;
        this.onmessage = null;
    }
    ServiceWorkerContainer.prototype = Object.create(EventTarget.prototype);
    ServiceWorkerContainer.prototype.register = function(scriptURL, options) {
        var self = this;
        var scope = (options && options.scope) ? options.scope : '/';
        var sw = new ServiceWorker(scriptURL, 'installing');
        var reg = new ServiceWorkerRegistration(scope, sw);
        reg.installing = sw;
        self._registrations[scope] = reg;
        self.ready = Promise.resolve(reg);

        // Lifecycle: dispatch install event
        var installEvt = new ExtendableEvent('install');
        if (typeof sw.oninstall === 'function') {
            try { sw.oninstall.call(sw, installEvt); } catch(e) {}
        }
        sw.dispatchEvent(installEvt);

        // Transition to installed (waiting)
        sw._setState('installed');
        reg.installing = null;
        reg.waiting = sw;

        // Transition to activating -> activated (active)
        sw._setState('activating');
        var activateEvt = new ExtendableEvent('activate');
        if (typeof sw.onactivate === 'function') {
            try { sw.onactivate.call(sw, activateEvt); } catch(e) {}
        }
        sw.dispatchEvent(activateEvt);

        reg.waiting = null;
        reg.active = sw;
        sw._setState('activated');
        self.controller = sw;

        var changeEvt = new Event('controllerchange');
        if (typeof self.oncontrollerchange === 'function') {
            try { self.oncontrollerchange.call(self, changeEvt); } catch(e) {}
        }
        self.dispatchEvent(changeEvt);

        return Promise.resolve(reg);
    };
    ServiceWorkerContainer.prototype.getRegistration = function(clientURL) {
        var url = clientURL || '/';
        var bestScope = null;
        for (var s in this._registrations) {
            if (url.indexOf(s) === 0) {
                if (!bestScope || s.length > bestScope.length) {
                    bestScope = s;
                }
            }
        }
        return Promise.resolve(bestScope ? this._registrations[bestScope] : undefined);
    };
    ServiceWorkerContainer.prototype.getRegistrations = function() {
        var list = [];
        for (var k in this._registrations) {
            list.push(this._registrations[k]);
        }
        return Promise.resolve(list);
    };
    ServiceWorkerContainer.prototype.startMessages = function() {};
    globalThis.ServiceWorkerContainer = ServiceWorkerContainer;

    if (globalThis.navigator) {
        globalThis.navigator.serviceWorker = new ServiceWorkerContainer();
    }

    function Client(id, url) {
        this.id = id || 'client-1';
        this.url = url || (globalThis.location ? globalThis.location.href : 'http://localhost/');
        this.type = 'window';
        this.frameType = 'top-level';
    }
    Client.prototype.postMessage = function(msg) {};

    function Clients() {}
    Clients.prototype.get = function(id) { return Promise.resolve(new Client(id)); };
    Clients.prototype.matchAll = function(options) { return Promise.resolve([new Client()]); };
    Clients.prototype.openWindow = function(url) { return Promise.resolve(new Client(null, url)); };
    Clients.prototype.claim = function() { return Promise.resolve(); };

    globalThis.Client = Client;
    globalThis.Clients = Clients;
    if (!globalThis.clients) {
        globalThis.clients = new Clients();
    }
    if (!globalThis.skipWaiting) {
        globalThis.skipWaiting = function() { return Promise.resolve(); };
    }

    // ── 13c. BroadcastChannel API (WHATWG HTML § 9.4) ─────────────────────────
    var _nextBroadcastId = 1;
    var _activeBroadcastChannels = {};

    function BroadcastChannel(name) {
        if (!arguments.length) {
            throw new TypeError("Failed to construct 'BroadcastChannel': 1 argument required, but only 0 present.");
        }
        EventTarget.call(this);
        this.name = String(name);
        this._closed = false;
        this._id = _nextBroadcastId++;
        this.onmessage = null;
        this.onmessageerror = null;
        this._listeners = {};

        _activeBroadcastChannels[this._id] = this;

        if (typeof _mangoBroadcastCreate === 'function') {
            _mangoBroadcastCreate(this._id, this.name);
        }
    }
    BroadcastChannel.prototype = Object.create(EventTarget.prototype);
    BroadcastChannel.prototype.constructor = BroadcastChannel;

    BroadcastChannel.prototype.addEventListener = function(type, fn) {
        if (!this._listeners[type]) this._listeners[type] = [];
        this._listeners[type].push(fn);
    };

    BroadcastChannel.prototype.removeEventListener = function(type, fn) {
        if (!this._listeners[type]) return;
        this._listeners[type] = this._listeners[type].filter(function(f) { return f !== fn; });
    };

    BroadcastChannel.prototype.dispatchEvent = function(evt) {
        var type = evt.type;
        if (type === 'message' && typeof this.onmessage === 'function') {
            try { this.onmessage.call(this, evt); } catch(e) { if (typeof console !== 'undefined') console.error(e); }
        }
        if (type === 'messageerror' && typeof this.onmessageerror === 'function') {
            try { this.onmessageerror.call(this, evt); } catch(e) { if (typeof console !== 'undefined') console.error(e); }
        }
        var list = this._listeners[type] || [];
        for (var i = 0; i < list.length; i++) {
            try { list[i].call(this, evt); } catch(e) { if (typeof console !== 'undefined') console.error(e); }
        }
        return !evt.defaultPrevented;
    };

    BroadcastChannel.prototype.postMessage = function(message) {
        if (this._closed) {
            throw new DOMException("Failed to execute 'postMessage' on 'BroadcastChannel': The channel is closed.", "InvalidStateError");
        }
        var cloned = message;
        try {
            if (typeof structuredClone === 'function') {
                cloned = structuredClone(message);
            }
        } catch(e) {}

        var self = this;
        for (var id in _activeBroadcastChannels) {
            if (Number(id) !== self._id) {
                var ch = _activeBroadcastChannels[id];
                if (ch && !ch._closed && ch.name === self.name) {
                    (function(targetCh) {
                        var defer = (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) { setTimeout(cb, 0); };
                        defer(function() {
                            if (targetCh._closed) return;
                            var ev = (typeof MessageEvent === 'function')
                                ? new MessageEvent('message', { data: cloned })
                                : { type: 'message', data: cloned };
                            targetCh.dispatchEvent(ev);
                        });
                    })(ch);
                }
            }
        }

        if (typeof _mangoBroadcastPost === 'function') {
            _mangoBroadcastPost(this._id, this.name, JSON.stringify(message));
        }
    };

    BroadcastChannel.prototype.close = function() {
        if (this._closed) return;
        this._closed = true;
        if (typeof _mangoBroadcastClose === 'function') {
            _mangoBroadcastClose(this._id, this.name);
        }
        delete _activeBroadcastChannels[this._id];
    };

    globalThis._mangoDispatchBroadcastMessage = function(channelId, jsonString) {
        var ch = _activeBroadcastChannels[channelId];
        if (ch && !ch._closed) {
            try {
                var data = JSON.parse(jsonString);
                var ev = (typeof MessageEvent === 'function')
                    ? new MessageEvent('message', { data: data })
                    : { type: 'message', data: data };
                ch.dispatchEvent(ev);
            } catch(e) {}
        }
    };

    globalThis.BroadcastChannel = BroadcastChannel;

    // ── 13d. SharedWorker API (WHATWG HTML § 10.1.2) ──────────────────────────
    var _nextSharedPortId = 1;
    var _activeSharedPorts = {};

    function SharedWorker(scriptURL, options) {
        if (!arguments.length) {
            throw new TypeError("Failed to construct 'SharedWorker': 1 argument required, but only 0 present.");
        }
        EventTarget.call(this);
        this.scriptURL = String(scriptURL || '');
        var name = '';
        if (typeof options === 'string') {
            name = options;
        } else if (options && typeof options.name === 'string') {
            name = options.name;
        }
        this._name = name;
        this.onerror = null;

        var portId = _nextSharedPortId++;
        var port = new MessagePort();
        port._sharedPortId = portId;
        _activeSharedPorts[portId] = port;

        var origPostMessage = port.postMessage;
        port.postMessage = function(data, transfer) {
            if (this._closed) return;
            if (typeof _mangoSharedWorkerPost === 'function') {
                _mangoSharedWorkerPost(portId, JSON.stringify(data));
            } else {
                origPostMessage.call(port, data, transfer);
            }
        };

        var origClose = port.close;
        port.close = function() {
            origClose.call(port);
            if (typeof _mangoSharedWorkerClosePort === 'function') {
                _mangoSharedWorkerClosePort(portId);
            }
            delete _activeSharedPorts[portId];
        };

        this.port = port;

        if (typeof _mangoSharedWorkerConnect === 'function') {
            _mangoSharedWorkerConnect(portId, this.scriptURL, this._name);
        }
    }
    SharedWorker.prototype = Object.create(EventTarget.prototype);
    SharedWorker.prototype.constructor = SharedWorker;

    globalThis._mangoDispatchSharedPortMessage = function(portId, jsonString) {
        var port = _activeSharedPorts[portId];
        if (port && !port._closed) {
            try {
                var data = JSON.parse(jsonString);
                var msg = { data: data, ports: [] };
                if (port._started || port._onmessage) {
                    port._dispatch(msg);
                } else {
                    port._queue.push(msg);
                }
            } catch(e) {}
        }
    };

    globalThis.SharedWorker = SharedWorker;

    // ── 13e. Web Locks API (W3C Web Locks) ────────────────────────────────────
    var _nextClientId = 'client-' + Math.floor(Math.random() * 1000000);
    var _activeLockCallbacks = {};

    function LockManager() {}
    LockManager.prototype.request = function(name, optionsOrCallback, maybeCallback) {
        var self = this;
        var callback = typeof optionsOrCallback === 'function' ? optionsOrCallback : maybeCallback;
        var options = typeof optionsOrCallback === 'object' && optionsOrCallback !== null ? optionsOrCallback : {};
        if (typeof callback !== 'function') {
            return Promise.reject(new TypeError("Lock callback must be a function"));
        }

        var mode = (options.mode === 'shared') ? 'shared' : 'exclusive';
        var ifAvailable = !!options.ifAvailable;
        var steal = !!options.steal;
        var signal = options.signal;

        return new Promise(function(resolve, reject) {
            if (signal && signal.aborted) {
                return reject(signal.reason || new DOMException("AbortError", "AbortError"));
            }

            if (typeof _mangoLocksRequest !== 'function') {
                var lockObj = { name: String(name), mode: mode };
                return Promise.resolve(callback(lockObj)).then(resolve, reject);
            }

            var resJson = _mangoLocksRequest(String(name), mode, ifAvailable, steal, _nextClientId);
            var res = JSON.parse(resJson);

            if (res.status === 'unavailable') {
                return Promise.resolve(callback(null)).then(resolve, reject);
            }

            var executeGrant = function(grantId, grantMode) {
                var lockObj = { name: String(name), mode: grantMode || mode };
                Promise.resolve()
                    .then(function() { return callback(lockObj); })
                    .then(function(val) {
                        if (typeof _mangoLocksRelease === 'function') {
                            var unblockedJson = _mangoLocksRelease(String(name), grantId);
                            self._processUnblocked(unblockedJson);
                        }
                        resolve(val);
                    }, function(err) {
                        if (typeof _mangoLocksRelease === 'function') {
                            var unblockedJson = _mangoLocksRelease(String(name), grantId);
                            self._processUnblocked(unblockedJson);
                        }
                        reject(err);
                    });
            };

            if (res.status === 'granted') {
                executeGrant(res.grantId, res.mode);
            } else if (res.status === 'queued') {
                var reqId = res.requestId;
                _activeLockCallbacks[reqId] = {
                    execute: executeGrant,
                    reject: reject,
                    name: String(name)
                };

                if (signal) {
                    signal.addEventListener('abort', function() {
                        delete _activeLockCallbacks[reqId];
                        if (typeof _mangoLocksAbort === 'function') {
                            _mangoLocksAbort(String(name), reqId);
                        }
                        reject(signal.reason || new DOMException("AbortError", "AbortError"));
                    }, { once: true });
                }
            }
        });
    };

    LockManager.prototype._processUnblocked = function(unblockedJson) {
        if (!unblockedJson) return;
        try {
            var items = JSON.parse(unblockedJson);
            for (var i = 0; i < items.length; i++) {
                var item = items[i];
                var cb = _activeLockCallbacks[item.requestId || item.grantId];
                if (cb) {
                    delete _activeLockCallbacks[item.requestId || item.grantId];
                    cb.execute(item.grantId, item.mode);
                }
            }
        } catch(e) {}
    };

    LockManager.prototype.query = function() {
        if (typeof _mangoLocksQuery === 'function') {
            try {
                var json = _mangoLocksQuery();
                return Promise.resolve(JSON.parse(json));
            } catch(e) {
                return Promise.reject(e);
            }
        }
        return Promise.resolve({ held: [], pending: [] });
    };

    globalThis.LockManager = LockManager;
    if (globalThis.navigator) {
        globalThis.navigator.locks = new LockManager();
    }

    // ── HTML Table DOM (WHATWG § 4.9) ──────────────────────────────────────────
    function _tableHTMLCollection(fn) {
        if (typeof globalThis._createHTMLCollection === 'function') {
            return globalThis._createHTMLCollection(fn);
        }
        var proto = (typeof globalThis.HTMLCollection !== 'undefined') ? globalThis.HTMLCollection.prototype : Object.prototype;
        var arr = fn() || [];
        var col = Object.create(proto);
        for (var i = 0; i < arr.length; i++) col[i] = arr[i];
        col.length = arr.length;
        col.item = function(idx) { return arr[idx] || null; };
        return col;
    }

    if (globalThis.HTMLTableElement && globalThis.HTMLTableElement.prototype) {
        var tp = globalThis.HTMLTableElement.prototype;
        Object.defineProperty(tp, 'caption', {
            get: function() {
                var kids = this.children || [];
                for (var i = 0; i < kids.length; i++) {
                    if ((kids[i].tagName || '').toLowerCase() === 'caption') return kids[i];
                }
                return null;
            },
            set: function(val) {
                var cur = this.caption;
                if (val && (val.tagName || '').toLowerCase() === 'caption') {
                    if (cur) this.replaceChild(val, cur);
                    else this.insertBefore(val, this.firstChild);
                } else if (!val && cur) {
                    this.removeChild(cur);
                }
            },
            configurable: true
        });
        tp.createCaption = function() {
            var cur = this.caption;
            if (cur) return cur;
            var c = (this.ownerDocument || document).createElement('caption');
            this.insertBefore(c, this.firstChild);
            return c;
        };
        tp.deleteCaption = function() {
            var cur = this.caption;
            if (cur) this.removeChild(cur);
        };

        Object.defineProperty(tp, 'tHead', {
            get: function() {
                var kids = this.children || [];
                for (var i = 0; i < kids.length; i++) {
                    if ((kids[i].tagName || '').toLowerCase() === 'thead') return kids[i];
                }
                return null;
            },
            set: function(val) {
                var cur = this.tHead;
                if (val && (val.tagName || '').toLowerCase() === 'thead') {
                    if (cur) this.replaceChild(val, cur);
                    else {
                        var ref = null;
                        var kids = this.children || [];
                        for (var i = 0; i < kids.length; i++) {
                            var tag = (kids[i].tagName || '').toLowerCase();
                            if (tag !== 'caption' && tag !== 'colgroup') { ref = kids[i]; break; }
                        }
                        this.insertBefore(val, ref);
                    }
                } else if (!val && cur) {
                    this.removeChild(cur);
                }
            },
            configurable: true
        });
        tp.createTHead = function() {
            var cur = this.tHead;
            if (cur) return cur;
            var th = (this.ownerDocument || document).createElement('thead');
            var ref = null;
            var kids = this.children || [];
            for (var i = 0; i < kids.length; i++) {
                var tag = (kids[i].tagName || '').toLowerCase();
                if (tag !== 'caption' && tag !== 'colgroup') { ref = kids[i]; break; }
            }
            this.insertBefore(th, ref);
            return th;
        };
        tp.deleteTHead = function() {
            var cur = this.tHead;
            if (cur) this.removeChild(cur);
        };

        Object.defineProperty(tp, 'tFoot', {
            get: function() {
                var kids = this.children || [];
                for (var i = 0; i < kids.length; i++) {
                    if ((kids[i].tagName || '').toLowerCase() === 'tfoot') return kids[i];
                }
                return null;
            },
            set: function(val) {
                var cur = this.tFoot;
                if (val && (val.tagName || '').toLowerCase() === 'tfoot') {
                    if (cur) this.replaceChild(val, cur);
                    else this.appendChild(val);
                } else if (!val && cur) {
                    this.removeChild(cur);
                }
            },
            configurable: true
        });
        tp.createTFoot = function() {
            var cur = this.tFoot;
            if (cur) return cur;
            var tf = (this.ownerDocument || document).createElement('tfoot');
            this.appendChild(tf);
            return tf;
        };
        tp.deleteTFoot = function() {
            var cur = this.tFoot;
            if (cur) this.removeChild(cur);
        };

        Object.defineProperty(tp, 'tBodies', {
            get: function() {
                var self = this;
                return _tableHTMLCollection(function() {
                    var list = [];
                    var kids = self.children || [];
                    for (var i = 0; i < kids.length; i++) {
                        if ((kids[i].tagName || '').toLowerCase() === 'tbody') list.push(kids[i]);
                    }
                    return list;
                });
            },
            configurable: true
        });
        tp.createTBody = function() {
            var tb = (this.ownerDocument || document).createElement('tbody');
            var lastTB = null;
            var kids = this.children || [];
            for (var i = 0; i < kids.length; i++) {
                if ((kids[i].tagName || '').toLowerCase() === 'tbody') lastTB = kids[i];
            }
            if (lastTB && lastTB.nextSibling) this.insertBefore(tb, lastTB.nextSibling);
            else this.appendChild(tb);
            return tb;
        };

        Object.defineProperty(tp, 'rows', {
            get: function() {
                var self = this;
                return _tableHTMLCollection(function() {
                    var list = [];
                    var kids = self.children || [];
                    for (var i = 0; i < kids.length; i++) {
                        if ((kids[i].tagName || '').toLowerCase() === 'thead') {
                            var trs = kids[i].children || [];
                            for (var j = 0; j < trs.length; j++) {
                                if ((trs[j].tagName || '').toLowerCase() === 'tr') list.push(trs[j]);
                            }
                        }
                    }
                    for (var i = 0; i < kids.length; i++) {
                        var tag = (kids[i].tagName || '').toLowerCase();
                        if (tag === 'tbody') {
                            var trs = kids[i].children || [];
                            for (var j = 0; j < trs.length; j++) {
                                if ((trs[j].tagName || '').toLowerCase() === 'tr') list.push(trs[j]);
                            }
                        } else if (tag === 'tr') {
                            list.push(kids[i]);
                        }
                    }
                    for (var i = 0; i < kids.length; i++) {
                        if ((kids[i].tagName || '').toLowerCase() === 'tfoot') {
                            var trs = kids[i].children || [];
                            for (var j = 0; j < trs.length; j++) {
                                if ((trs[j].tagName || '').toLowerCase() === 'tr') list.push(trs[j]);
                            }
                        }
                    }
                    return list;
                });
            },
            configurable: true
        });
        tp.insertRow = function(index) {
            var r = (this.ownerDocument || document).createElement('tr');
            var rows = this.rows;
            var len = rows.length;
            if (index === undefined || index === -1 || index === len) {
                var tbs = this.tBodies;
                if (tbs.length > 0) {
                    tbs[tbs.length - 1].appendChild(r);
                } else {
                    var tb = this.createTBody();
                    tb.appendChild(r);
                }
            } else if (index >= 0 && index < len) {
                var target = rows[index];
                if (target && target.parentNode) {
                    target.parentNode.insertBefore(r, target);
                }
            } else {
                throw new DOMException("IndexSizeError", "IndexSizeError");
            }
            return r;
        };
        tp.deleteRow = function(index) {
            var rows = this.rows;
            var len = rows.length;
            if (index === -1) index = len - 1;
            if (index >= 0 && index < len) {
                var target = rows[index];
                if (target && target.parentNode) target.parentNode.removeChild(target);
            } else {
                throw new DOMException("IndexSizeError", "IndexSizeError");
            }
        };
    }

    if (globalThis.HTMLTableSectionElement && globalThis.HTMLTableSectionElement.prototype) {
        var tsp = globalThis.HTMLTableSectionElement.prototype;
        Object.defineProperty(tsp, 'rows', {
            get: function() {
                var self = this;
                return _tableHTMLCollection(function() {
                    var list = [];
                    var kids = self.children || [];
                    for (var i = 0; i < kids.length; i++) {
                        if ((kids[i].tagName || '').toLowerCase() === 'tr') list.push(kids[i]);
                    }
                    return list;
                });
            },
            configurable: true
        });
        tsp.insertRow = function(index) {
            var r = (this.ownerDocument || document).createElement('tr');
            var rows = this.rows;
            var len = rows.length;
            if (index === undefined || index === -1 || index === len) {
                this.appendChild(r);
            } else if (index >= 0 && index < len) {
                this.insertBefore(r, rows[index]);
            } else {
                throw new DOMException("IndexSizeError", "IndexSizeError");
            }
            return r;
        };
        tsp.deleteRow = function(index) {
            var rows = this.rows;
            var len = rows.length;
            if (index === -1) index = len - 1;
            if (index >= 0 && index < len) {
                this.removeChild(rows[index]);
            } else {
                throw new DOMException("IndexSizeError", "IndexSizeError");
            }
        };
    }

    if (globalThis.HTMLTableRowElement && globalThis.HTMLTableRowElement.prototype) {
        var trp = globalThis.HTMLTableRowElement.prototype;
        Object.defineProperty(trp, 'rowIndex', {
            get: function() {
                var p = this.parentElement;
                while (p && (p.tagName || '').toLowerCase() !== 'table') p = p.parentElement;
                if (!p) return -1;
                var rows = p.rows;
                for (var i = 0; i < rows.length; i++) {
                    if (rows[i] === this) return i;
                }
                return -1;
            },
            configurable: true
        });
        Object.defineProperty(trp, 'sectionRowIndex', {
            get: function() {
                var p = this.parentElement;
                if (!p) return -1;
                var rows = (typeof p.rows !== 'undefined') ? p.rows : [];
                for (var i = 0; i < rows.length; i++) {
                    if (rows[i] === this) return i;
                }
                return -1;
            },
            configurable: true
        });
        Object.defineProperty(trp, 'cells', {
            get: function() {
                var self = this;
                return _tableHTMLCollection(function() {
                    var list = [];
                    var kids = self.children || [];
                    for (var i = 0; i < kids.length; i++) {
                        var tag = (kids[i].tagName || '').toLowerCase();
                        if (tag === 'td' || tag === 'th') list.push(kids[i]);
                    }
                    return list;
                });
            },
            configurable: true
        });
        trp.insertCell = function(index) {
            var c = (this.ownerDocument || document).createElement('td');
            var cells = this.cells;
            var len = cells.length;
            if (index === undefined || index === -1 || index === len) {
                this.appendChild(c);
            } else if (index >= 0 && index < len) {
                this.insertBefore(c, cells[index]);
            } else {
                throw new DOMException("IndexSizeError", "IndexSizeError");
            }
            return c;
        };
        trp.deleteCell = function(index) {
            var cells = this.cells;
            var len = cells.length;
            if (index === -1) index = len - 1;
            if (index >= 0 && index < len) {
                this.removeChild(cells[index]);
            } else {
                throw new DOMException("IndexSizeError", "IndexSizeError");
            }
        };
    }

    if (globalThis.HTMLTableCellElement && globalThis.HTMLTableCellElement.prototype) {
        var tcp = globalThis.HTMLTableCellElement.prototype;
        Object.defineProperty(tcp, 'cellIndex', {
            get: function() {
                var p = this.parentElement;
                if (!p || (p.tagName || '').toLowerCase() !== 'tr') return -1;
                var cells = p.cells;
                for (var i = 0; i < cells.length; i++) {
                    if (cells[i] === this) return i;
                }
                return -1;
            },
            configurable: true
        });
    }

