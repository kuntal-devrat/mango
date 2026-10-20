    // ── 27b. MessagePort & MessageChannel (React Scheduler / WHATWG) ───────────
    function MessagePort() {
        if (typeof globalThis.EventTarget === 'function') {
            globalThis.EventTarget.call(this);
        }
        this._listeners = {};
        this._entangled = null;
        this._closed = false;
        this._started = false;
        this._queue = [];
        this._onmessage = null;
        this._onmessageerror = null;
    }
    if (typeof globalThis.EventTarget === 'function') {
        MessagePort.prototype = Object.create(globalThis.EventTarget.prototype);
    } else {
        MessagePort.prototype = {};
    }
    MessagePort.prototype.constructor = MessagePort;

    if (!MessagePort.prototype.addEventListener) {
        MessagePort.prototype.addEventListener = function(type, listener, opts) {
            var t = String(type);
            if (!this._listeners[t]) this._listeners[t] = [];
            this._listeners[t].push(listener);
        };
        MessagePort.prototype.removeEventListener = function(type, listener, opts) {
            var t = String(type);
            if (!this._listeners[t]) return;
            this._listeners[t] = this._listeners[t].filter(function(l) { return l !== listener; });
        };
        MessagePort.prototype.dispatchEvent = function(evt) {
            var t = String(evt.type || evt);
            if (this._listeners[t]) {
                var list = this._listeners[t].slice();
                for (var i = 0; i < list.length; i++) {
                    try {
                        if (typeof list[i] === 'function') list[i].call(this, evt);
                        else if (list[i] && typeof list[i].handleEvent === 'function') list[i].handleEvent(evt);
                    } catch(e) {}
                }
            }
            return true;
        };
    }

    Object.defineProperty(MessagePort.prototype, 'onmessage', {
        get: function() { return this._onmessage; },
        set: function(fn) {
            this._onmessage = fn;
            if (fn) {
                this.start();
            }
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(MessagePort.prototype, 'onmessageerror', {
        get: function() { return this._onmessageerror; },
        set: function(fn) { this._onmessageerror = fn; },
        configurable: true,
        enumerable: true
    });

    MessagePort.prototype.start = function() {
        if (this._started) return;
        this._started = true;
        this._drain();
    };

    MessagePort.prototype.close = function() {
        this._closed = true;
        this._queue = [];
    };

    MessagePort.prototype._drain = function() {
        if (!this._started || this._closed) return;
        while (this._queue.length > 0) {
            var msg = this._queue.shift();
            this._dispatch(msg);
        }
    };

    MessagePort.prototype._dispatch = function(msg) {
        if (this._closed) return;
        var ev = (typeof globalThis.MessageEvent === 'function')
            ? new globalThis.MessageEvent('message', { data: msg.data, ports: msg.ports || [] })
            : { type: 'message', data: msg.data, ports: msg.ports || [] };
        if (typeof this._onmessage === 'function') {
            try { this._onmessage.call(this, ev); } catch(e) {
                if (typeof console !== 'undefined' && console.error) console.error(e);
            }
        }
        if (typeof this.dispatchEvent === 'function') {
            this.dispatchEvent(ev);
        }
    };

    MessagePort.prototype.postMessage = function(data, transfer) {
        if (this._closed || !this._entangled || this._entangled._closed) return;
        var target = this._entangled;
        var clonedData = data;
        try {
            if (typeof structuredClone === 'function') {
                clonedData = structuredClone(data);
            }
        } catch(e) {
            clonedData = data;
        }

        var msg = {
            data: clonedData,
            ports: Array.isArray(transfer) ? transfer : []
        };

        var defer = (typeof queueMicrotask === 'function')
            ? queueMicrotask
            : function(cb) { setTimeout(cb, 0); };
        defer(function() {
            if (target._closed) return;
            if (target._started) {
                target._dispatch(msg);
            } else {
                target._queue.push(msg);
            }
        });
    };

    function MessageChannel() {
        var p1 = new MessagePort();
        var p2 = new MessagePort();
        p1._entangled = p2;
        p2._entangled = p1;
        this.port1 = p1;
        this.port2 = p2;
    }
    globalThis.MessagePort = MessagePort;
    globalThis.MessageChannel = MessageChannel;
    if (typeof window !== 'undefined') {
        window.MessagePort = MessagePort;
        window.MessageChannel = MessageChannel;
    }


    globalThis.CloseEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.code = dict.code !== undefined ? dict.code : 1000;
        this.reason = dict.reason || '';
        this.wasClean = !!dict.wasClean;
    };
    globalThis.CloseEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.CloseEvent.prototype.constructor = globalThis.CloseEvent;

    // ── 28. Web Components V1 Custom Elements Engine ──────────────────────────
    globalThis.WebComponents = { ready: true };

    function _upgradeCustomElement(el, ctor) {
        if (!el || typeof el !== 'object' || el._isCustomConstructed) return;
        if (!ctor || typeof ctor !== 'function') return;

        el._isCustomConstructed = true;
        if (ctor.prototype) {
            Object.setPrototypeOf(el, ctor.prototype);
        }

        var prevCandidate = globalThis._customElementsUpgradeCandidate;
        globalThis._customElementsUpgradeCandidate = el;
        var constructed = null;
        try {
            constructed = Reflect.construct(ctor, [], ctor);
        } catch (e) {
            if (typeof console !== 'undefined' && console.warn) {
                console.warn("[_upgradeCustomElement construct error on " + (el.tagName||"") + "]: " + (e.stack || e));
            }
            try {
                ctor.call(el);
            } catch (e2) {
                if (typeof console !== 'undefined' && console.warn) {
                    console.warn("[_upgradeCustomElement ctor.call error on " + (el.tagName||"") + "]: " + (e2.stack || e2));
                }
            }
        } finally {
            globalThis._customElementsUpgradeCandidate = prevCandidate;
        }

        if (constructed && constructed !== el) {
            var props = Object.getOwnPropertyNames(constructed);
            for (var i = 0; i < props.length; i++) {
                var p = props[i];
                if (p !== 'constructor') {
                    try { el[p] = constructed[p]; } catch(e) {}
                }
            }
        }

        // Trigger initial observedAttributes
        var obs = (ctor && ctor.observedAttributes) || el.observedAttributes;
        if (Array.isArray(obs) && typeof el.attributeChangedCallback === 'function' && typeof el.getAttribute === 'function') {
            for (var a = 0; a < obs.length; a++) {
                var attrName = obs[a];
                if (el.hasAttribute && el.hasAttribute(attrName)) {
                    var val = el.getAttribute(attrName);
                    try {
                        el.attributeChangedCallback(attrName, null, val, null);
                    } catch(e) {}
                }
            }
        }

        // Trigger created, connectedCallback, and attached if element is attached to document
        if (typeof el.created === 'function' && !el._isCreatedCalled) {
            el._isCreatedCalled = true;
            try { el.created(); } catch(e) {
                if (typeof console !== 'undefined' && console.warn) {
                    console.warn("[_upgradeCustomElement created error on " + (el.tagName||"") + "]: " + (e.stack || e));
                }
            }
        }
        if (el.isConnected && !el._isCustomConnected) {
            if (typeof el.connectedCallback === 'function') {
                el._isCustomConnected = true;
                try {
                    el.connectedCallback();
                } catch(e) {
                    if (typeof console !== 'undefined' && console.warn) {
                        console.warn("[_upgradeCustomElement connectedCallback error on " + (el.tagName||"") + "]: " + (e.stack || e));
                    }
                }
            }
            if (typeof el.attached === 'function' && !el._isAttachedCalled) {
                el._isAttachedCalled = true;
                try { el.attached(); } catch(e) {
                    if (typeof console !== 'undefined' && console.warn) {
                        console.warn("[_upgradeCustomElement attached error on " + (el.tagName||"") + "]: " + (e.stack || e));
                    }
                }
            }
            if (el.polymerController && typeof el.polymerController.attached === 'function' && !el.polymerController._isAttachedCalled) {
                el.polymerController._isAttachedCalled = true;
                try { el.polymerController.attached(); } catch(e) {
                    if (typeof console !== 'undefined' && console.warn) {
                        console.warn("[_upgradeCustomElement polymerController.attached error on " + (el.tagName||"") + "]: " + (e.stack || e));
                    }
                }
            }
        }
    }
    globalThis._upgradeCustomElement = _upgradeCustomElement;

    function _upgradeTree(node, targetName, targetCtor, visited) {
        if (!node) return;
        if (!visited) visited = new Set();
        if (visited.has(node)) return;
        visited.add(node);
        if (node.nodeType === 1) { // Element
            var tag = (node.tagName || '').toLowerCase();
            var isVal = (node.getAttribute && node.getAttribute('is')) ? String(node.getAttribute('is')).toLowerCase() : null;
            var match = false;
            var ctor = null;
            if (targetName) {
                if (tag === targetName || isVal === targetName) {
                    match = true;
                    ctor = targetCtor || (globalThis.customElements && globalThis.customElements.get(targetName));
                }
            } else {
                ctor = (globalThis.customElements && isVal && globalThis.customElements.get(isVal)) ||
                       (globalThis.customElements && globalThis.customElements.get(tag));
                if (ctor) match = true;
            }
            if (match && ctor && globalThis._upgradeCustomElement) {
                globalThis._upgradeCustomElement(node, ctor);
            }
            var sr = node._shadowRoot || node.shadowRoot;
            if (sr) {
                _upgradeTree(sr, targetName, targetCtor, visited);
            }
        } else if (node.nodeType === 9) { // Document
            if (node.documentElement) {
                _upgradeTree(node.documentElement, targetName, targetCtor, visited);
            }
        }
        var kids = node.childNodes;
        if (kids) {
            for (var k = 0; k < kids.length; k++) {
                _upgradeTree(kids[k], targetName, targetCtor, visited);
            }
        }
    }
    globalThis._upgradeTree = _upgradeTree;

    globalThis.CustomElementRegistry = function() {
        this._registry = {};
        this._definitions = {};
        this._whenDefinedPromises = {};
    };
    globalThis.CustomElementRegistry.prototype.define = function(name, ctor, opts) {
        name = String(name).toLowerCase();
        if (!/^[a-z][a-z0-9._-]*-[a-z0-9._-]*$/.test(name)) {
            throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': '" + name + "' is not a valid custom element name.");
        }
        var reserved = [
            'annotation-xml', 'color-profile', 'font-face', 'font-face-src',
            'font-face-uri', 'font-face-format', 'font-face-name', 'missing-glyph'
        ];
        if (reserved.indexOf(name) !== -1) {
            throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': '" + name + "' is a reserved custom element name.");
        }
        if (typeof ctor !== 'function') {
            throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': The constructor must be a function.");
        }
        if (this._registry[name]) {
            throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': the name '" + name + "' has already been used with this registry", "NotSupportedError");
        }
        for (var k in this._registry) {
            if (this._registry[k] === ctor) {
                throw new DOMException("Failed to execute 'define' on 'CustomElementRegistry': this constructor has already been used with this registry", "NotSupportedError");
            }
        }
        this._registry[name] = ctor;
        if (ctor && ctor.prototype) {
            if (typeof ctor.prototype.showEngagementPanelScrimChanged_ === 'function') {
                var origSEPSC = ctor.prototype.showEngagementPanelScrimChanged_;
                ctor.prototype.showEngagementPanelScrimChanged_ = function(x) {
                    try {
                        return origSEPSC.call(this, x);
                    } catch(e) {}
                };
            }
            if (typeof ctor.prototype.getPlayer === 'function') {
                var origGP = ctor.prototype.getPlayer;
                ctor.prototype.getPlayer = function() {
                    try {
                        var res = origGP.apply(this, arguments);
                        return res || _dummyPlayer;
                    } catch(e) {
                        return _dummyPlayer;
                    }
                };
            }
            if (typeof ctor.prototype.preparePlayer === 'function') {
                var origPP = ctor.prototype.preparePlayer;
                ctor.prototype.preparePlayer = function() {
                    try {
                        return origPP.apply(this, arguments);
                    } catch(e) {}
                };
            }
            if (typeof ctor.prototype.attachWatchPageSkeleton_ === 'function') {
                var origAWPS = ctor.prototype.attachWatchPageSkeleton_;
                ctor.prototype.attachWatchPageSkeleton_ = function() {
                    try {
                        return origAWPS.apply(this, arguments);
                    } catch(e) {}
                };
            }
            if (typeof ctor.prototype.attached === 'function') {
                var origAtt = ctor.prototype.attached;
                ctor.prototype.attached = function() {
                    try {
                        return origAtt.apply(this, arguments);
                    } catch(e) {
                        try { console.warn('[CustomElement attached exception guarded]:', e && e.message); } catch(_) {}
                    }
                };
            }
        }
        var ext = (opts && opts.extends) ? String(opts.extends).toLowerCase() : null;
        if (!this._definitions) this._definitions = {};
        this._definitions[name] = { constructor: ctor, extends: ext };

        // Upgrade in document tree
        if (typeof document !== 'undefined') {
            try {
                var root = document.documentElement || document.body || document;
                _upgradeTree(root, name, ctor);
            } catch(e) {}
        }

        if (this._whenDefinedPromises && this._whenDefinedPromises[name]) {
            var cbs = this._whenDefinedPromises[name];
            delete this._whenDefinedPromises[name];
            for (var j = 0; j < cbs.length; j++) {
                try { cbs[j](ctor); } catch(e) {}
            }
        }
    };
    globalThis.CustomElementRegistry.prototype.get = function(n) {
        return this._registry[String(n).toLowerCase()];
    };
    globalThis.CustomElementRegistry.prototype.getName = function(ctor) {
        for (var k in this._registry) {
            if (this._registry[k] === ctor) return k;
        }
        return null;
    };
    globalThis.CustomElementRegistry.prototype.upgrade = function(root) {
        if (!root) return;
        _upgradeTree(root, null, null);
    };
    globalThis.CustomElementRegistry.prototype.whenDefined = function(n) {
        var name = String(n).toLowerCase();
        if (!/^[a-z][a-z0-9._-]*-[a-z0-9._-]*$/.test(name)) {
            return Promise.reject(new SyntaxError("Failed to execute 'whenDefined' on 'CustomElementRegistry': '" + name + "' is not a valid custom element name."));
        }
        if (this._registry[name]) {
            return Promise.resolve(this._registry[name]);
        }
        var self = this;
        return new Promise(function(resolve) {
            if (!self._whenDefinedPromises) self._whenDefinedPromises = {};
            if (!self._whenDefinedPromises[name]) self._whenDefinedPromises[name] = [];
            self._whenDefinedPromises[name].push(resolve);
        });
    };
    if (!globalThis.customElements) {
        globalThis.customElements = new globalThis.CustomElementRegistry();
    }

    // ── 28.5 Constructable Stylesheets (CSSOM §2) ─────────────────────────────
    function CSSStyleSheet(options) {
        this._rules = [];
        this._disabled = !!(options && options.disabled);
        this._media = (options && options.media) || 'all';
    }

    Object.defineProperty(CSSStyleSheet.prototype, 'cssRules', {
        get: function() { return this._rules; },
        enumerable: true
    });
    Object.defineProperty(CSSStyleSheet.prototype, 'rules', {
        get: function() { return this._rules; },
        enumerable: true
    });
    Object.defineProperty(CSSStyleSheet.prototype, 'disabled', {
        get: function() { return this._disabled; },
        set: function(v) { this._disabled = !!v; },
        enumerable: true
    });
    Object.defineProperty(CSSStyleSheet.prototype, 'media', {
        get: function() { return this._media; },
        enumerable: true
    });

    CSSStyleSheet.prototype.replaceSync = function(text) {
        this._rules = [];
        var css = String(text);
        var blocks = css.split('}');
        for (var i = 0; i < blocks.length; i++) {
            var block = blocks[i].trim();
            if (!block) continue;
            var parts = block.split('{');
            if (parts.length >= 2) {
                var sel = parts[0].trim();
                var body = parts.slice(1).join('{').trim();
                this._rules.push({
                    cssText: sel + ' { ' + body + ' }',
                    selectorText: sel,
                    style: { cssText: body }
                });
            }
        }
        return undefined;
    };

    CSSStyleSheet.prototype.replace = function(text) {
        var self = this;
        return new Promise(function(resolve) {
            self.replaceSync(text);
            resolve(self);
        });
    };

    CSSStyleSheet.prototype.insertRule = function(rule, index) {
        index = index === undefined ? 0 : index;
        var rStr = String(rule).trim();
        var parts = rStr.split('{');
        var sel = parts[0].trim();
        var body = parts.length > 1 ? parts.slice(1).join('{').replace(/}$/, '').trim() : '';
        var parsed = {
            cssText: rStr,
            selectorText: sel,
            style: { cssText: body }
        };
        this._rules.splice(index, 0, parsed);
        return index;
    };

    CSSStyleSheet.prototype.deleteRule = function(index) {
        if (index < 0 || index >= this._rules.length) {
            throw new RangeError("Index out of range");
        }
        this._rules.splice(index, 1);
    };

    globalThis.CSSStyleSheet = CSSStyleSheet;
    if (typeof window !== 'undefined') {
        window.CSSStyleSheet = CSSStyleSheet;
    }

    // ── 29. Blob / File / FileReader ──────────────────────────────────────────
    // (Full standard implementations of Blob, File, and FileReader are defined above in section 19)

    // ── 30. Promise (ensure standard existence) ──────────────────────────────
    if (typeof Promise === 'undefined') {
        // Minimal synchronous Promise polyfill as last-resort fallback
        globalThis.Promise = function(executor) {
            this._state = 'pending'; this._val = undefined; this._cb = [];
            var self = this;
            function resolve(v) { if (self._state !== 'pending') return; self._state = 'fulfilled'; self._val = v; self._cb.forEach(function(c) { if (c.onFulfilled) try { c.onFulfilled(v); } catch(e) {} }); }
            function reject(r)  { if (self._state !== 'pending') return; self._state = 'rejected';  self._val = r; self._cb.forEach(function(c) { if (c.onRejected)  try { c.onRejected(r);  } catch(e) {} }); }
            try { executor(resolve, reject); } catch(e) { reject(e); }
        };
        globalThis.Promise.prototype.then = function(onF, onR) {
            var self = this;
            return new Promise(function(res, rej) {
                self._cb.push({ onFulfilled: onF ? function(v) { try { res(onF(v)); } catch(e) { rej(e); } } : res, onRejected: onR ? function(r) { try { res(onR(r)); } catch(e) { rej(e); } } : rej });
                if (self._state === 'fulfilled' && onF) try { res(onF(self._val)); } catch(e) { rej(e); }
                if (self._state === 'rejected'  && onR) try { res(onR(self._val)); } catch(e) { rej(e); }
            });
        };
        globalThis.Promise.prototype.catch  = function(fn) { return this.then(undefined, fn); };
        globalThis.Promise.prototype.finally= function(fn) { return this.then(function(v){ fn(); return v; }, function(r){ fn(); throw r; }); };
        globalThis.Promise.resolve = function(v) { return new Promise(function(res) { res(v); }); };
        globalThis.Promise.reject  = function(r) { return new Promise(function(_, rej) { rej(r); }); };
        globalThis.Promise.all     = function(ps) { return new Promise(function(res, rej) { if (!ps.length) { res([]); return; } var out=[],done=0; ps.forEach(function(p,i){ Promise.resolve(p).then(function(v){ out[i]=v; if(++done===ps.length) res(out); },rej); }); }); };
        globalThis.Promise.allSettled = function(ps) { return new Promise(function(res) { if (!ps.length) { res([]); return; } var out=[],done=0; ps.forEach(function(p,i){ Promise.resolve(p).then(function(v){ out[i]={status:'fulfilled',value:v};  if(++done===ps.length) res(out); },function(r){ out[i]={status:'rejected',reason:r}; if(++done===ps.length) res(out); }); }); }); };
        globalThis.Promise.race    = function(ps) { return new Promise(function(res,rej) { ps.forEach(function(p){ Promise.resolve(p).then(res,rej); }); }); };
        globalThis.Promise.any     = function(ps) { return new Promise(function(res,rej) { var errs=[],done=0; ps.forEach(function(p,i){ Promise.resolve(p).then(res,function(r){ errs[i]=r; if(++done===ps.length) rej(new Error('All promises rejected')); }); }); }); };
    }

    // ── 31. queueMicrotask ────────────────────────────────────────────────────
    globalThis.queueMicrotask = function(fn) {
        Promise.resolve().then(function() { try { fn(); } catch(e) {} });
    };

    // ── 32. Web Worker & Service Worker ──────────────────────────
    var _nextWorkerId = 1;
    var _activeWorkers = {};

    globalThis._mangoDispatchWorkerMessage = function(id, dataStr) {
        var w = _activeWorkers[id];
        if (!w || w._terminated) return;
        var data;
        try { data = JSON.parse(dataStr); } catch(e) { data = dataStr; }
        var evt = (typeof globalThis.MessageEvent === 'function')
            ? new globalThis.MessageEvent('message', { data: data, source: w })
            : { type: 'message', data: data, source: w, target: w };
        w.dispatchEvent(evt);
    };

    globalThis._mangoDispatchWorkerError = function(id, errMsg) {
        var w = _activeWorkers[id];
        if (!w || w._terminated) return;
        var evt = (typeof globalThis.ErrorEvent === 'function')
            ? new globalThis.ErrorEvent('error', { message: errMsg })
            : { type: 'error', message: errMsg, target: w };
        w.dispatchEvent(evt);
    };

    function Worker(scriptURL, options) {
        EventTarget.call(this);
        this.scriptURL = String(scriptURL || '');
        this.options = options || {};
        this.onmessage = null;
        this.onerror = null;
        this.onmessageerror = null;
        this._listeners = {};
        this._terminated = false;

        var id = _nextWorkerId++;
        this._workerId = id;
        _activeWorkers[id] = this;

        var self = this;
        this.addEventListener = function(type, fn) {
            if (!self._listeners[type]) self._listeners[type] = [];
            self._listeners[type].push(fn);
        };
        this.removeEventListener = function(type, fn) {
            if (!self._listeners[type]) return;
            self._listeners[type] = self._listeners[type].filter(function(f) { return f !== fn; });
        };
        this.dispatchEvent = function(evt) {
            var type = evt.type;
            if (type === 'message' && typeof self.onmessage === 'function') {
                try { self.onmessage(evt); } catch(e) {}
            }
            if (type === 'error' && typeof self.onerror === 'function') {
                try { self.onerror(evt); } catch(e) {}
            }
            var list = self._listeners[type] || [];
            for (var i = 0; i < list.length; i++) {
                try { list[i].call(self, evt); } catch(e) {}
            }
            return !evt.defaultPrevented;
        };
        this.postMessage = function(message, transfer) {
            if (self._terminated) return;
            var json = JSON.stringify(message);
            if (typeof _mangoWorkerPostMessage === 'function') {
                _mangoWorkerPostMessage(self._workerId, json);
            }
        };
        this.terminate = function() {
            self._terminated = true;
            if (typeof _mangoWorkerTerminate === 'function') {
                _mangoWorkerTerminate(self._workerId);
            }
            delete _activeWorkers[self._workerId];
        };

        if (typeof _mangoWorkerCreate === 'function') {
            _mangoWorkerCreate(id, this.scriptURL, JSON.stringify(this.options));
        }
    }
    Worker.prototype = Object.create(EventTarget.prototype);
    globalThis.Worker = Worker;

