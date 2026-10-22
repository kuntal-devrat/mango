    globalThis.Document.prototype.createElement = function(t) { return this._isXml ? (typeof _createElementXML === 'function' ? _createElementXML(this, t) : null) : ((typeof document !== 'undefined' && document.createElement) ? document.createElement(t) : null); };
    globalThis.Document.prototype.createElementNS = function(ns, t) { return (typeof _createElementNS === 'function') ? _createElementNS(this, ns, t) : ((typeof document !== 'undefined' && document.createElementNS) ? document.createElementNS(ns, t) : this.createElement(t)); };
    globalThis.Document.prototype.createTextNode     = function(t) { return (typeof document !== 'undefined') ? document.createTextNode(t) : null; };
    globalThis.Document.prototype.createComment      = function(t) { return (typeof document !== 'undefined' && document.createComment) ? document.createComment(t) : null; };
    globalThis.Document.prototype.createProcessingInstruction = function(target, data) { return (typeof document !== 'undefined' && document.createProcessingInstruction) ? document.createProcessingInstruction(target, data) : null; };
    globalThis.Document.prototype.createDocumentFragment = function() { return (typeof document !== 'undefined' && document.createDocumentFragment) ? document.createDocumentFragment() : null; };
    globalThis.Document.prototype.querySelector      = function(s) { return (typeof document !== 'undefined') ? document.querySelector(s) : null; };
    globalThis.Document.prototype.querySelectorAll   = function(s) { return (typeof document !== 'undefined') ? document.querySelectorAll(s) : []; };
    globalThis.Document.prototype.getElementById     = function(i) { return (typeof document !== 'undefined') ? document.getElementById(i) : null; };
    globalThis.Document.prototype.getElementsByTagName    = function(t) { return (typeof document !== 'undefined' && Object.prototype.hasOwnProperty.call(document, 'getElementsByTagName')) ? document.getElementsByTagName(t) : ((typeof _defaultGetElementsByTagName === 'function') ? _defaultGetElementsByTagName(this, t) : []); };
    globalThis.Document.prototype.getElementsByTagNameNS  = function(ns, t) { return (typeof document !== 'undefined' && Object.prototype.hasOwnProperty.call(document, 'getElementsByTagNameNS')) ? document.getElementsByTagNameNS(ns, t) : ((typeof _defaultGetElementsByTagNameNS === 'function') ? _defaultGetElementsByTagNameNS(this, ns, t) : []); };
    globalThis.Document.prototype.getElementsByClassName  = function(c) { return (typeof document !== 'undefined') ? document.getElementsByClassName(c) : []; };
    globalThis.Document.prototype.createEvent        = function(t) { return (typeof document !== 'undefined' && document.createEvent) ? document.createEvent(t) : new globalThis.Event(t); };
    globalThis.Document.prototype.importNode         = function(n, d) { return n && n.cloneNode ? n.cloneNode(d) : n; };
    globalThis.Document.prototype.adoptNode          = function(n) { return (typeof _adoptNodeInternal === 'function') ? _adoptNodeInternal(this, n) : n; };
    globalThis.Document.prototype.elementFromPoint   = function(x, y) { return (typeof document !== 'undefined' && document.elementFromPoint) ? document.elementFromPoint(x, y) : null; };
    globalThis.Document.prototype.elementsFromPoint  = function(x, y) { return (typeof document !== 'undefined' && document.elementsFromPoint) ? document.elementsFromPoint(x, y) : []; };
    globalThis.Document.prototype.caretPositionFromPoint = function() { return null; };
    globalThis.Document.prototype.createRange        = function() { return (typeof document !== 'undefined' && document.createRange) ? document.createRange() : new globalThis.Range(); };
    globalThis.Document.prototype.getSelection      = function() { return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null; };
    Object.defineProperty(globalThis.Document.prototype, 'activeElement', {
        get: function() { return (typeof document !== 'undefined') ? document.activeElement : null; },
        set: function(val) { if (typeof document !== 'undefined') document.activeElement = val; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'characterSet', {
        get: function() { return this._characterSet || ((typeof document !== 'undefined' && document.characterSet) ? document.characterSet : 'UTF-8'); },
        set: function(v) { this._characterSet = v; }, configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'charset', { get: function() { return this.characterSet; }, set: function(v) { this.characterSet = v; }, configurable: true });
    Object.defineProperty(globalThis.Document.prototype, 'inputEncoding', { get: function() { return this.characterSet; }, set: function(v) { this.characterSet = v; }, configurable: true });
    Object.defineProperty(globalThis.Document.prototype, 'compatMode', {
        get: function() { return this._compatMode || (this === document && typeof _getCompatMode === 'function' ? _getCompatMode() : 'CSS1Compat'); },
        set: function(v) { this._compatMode = v; }, configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'contentType', {
        get: function() { return this._contentType || ((typeof document !== 'undefined' && document.contentType) ? document.contentType : 'text/html'); },
        set: function(v) { this._contentType = v; }, configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'documentElement', {
        get: function() {
            if (this._documentElement !== undefined) return this._documentElement;
            var kids = this.childNodes || this._childNodes || [];
            for (var i = 0; i < kids.length; i++) { if (kids[i] && kids[i].nodeType === 1) return kids[i]; }
            return (this === document && typeof document !== 'undefined') ? document.documentElement : null;
        },
        set: function(v) { this._documentElement = v; }, configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'doctype', {
        get: function() {
            if (this._doctype !== undefined) return this._doctype;
            var kids = this.childNodes || [];
            for (var i = 0; i < kids.length; i++) { if (kids[i] && kids[i].nodeType === 10) return kids[i]; }
            return (this === document && typeof document !== 'undefined') ? document.doctype : null;
        },
        set: function(v) { this._doctype = v; }, configurable: true, enumerable: true
    });
    ['head', 'body', 'forms', 'images', 'links', 'scripts'].forEach(function(p) {
        Object.defineProperty(globalThis.Document.prototype, p, {
            get: function() { return (typeof document !== 'undefined') ? document[p] : null; },
            configurable: true
        });
    });
    Object.defineProperty(globalThis.Document.prototype, 'children', {
        get: function() {
            if (this === document && typeof document !== 'undefined') return document.children;
            var kids = (this._childNodes || []).filter(function(n) { return n && n.nodeType === 1; });
            return (typeof _createHTMLCollection === 'function') ? _createHTMLCollection(function() { return kids; }) : kids;
        },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'firstElementChild', {
        get: function() { var c = this.children; return (c && c.length > 0) ? c[0] : null; }, configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'lastElementChild', {
        get: function() { var c = this.children; return (c && c.length > 0) ? c[c.length - 1] : null; }, configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'childElementCount', {
        get: function() { var c = this.children; return c ? c.length : 0; }, configurable: true
    });
    globalThis.Document.prototype.appendChild = function(c) { if (arguments.length < 1) throw new TypeError("Failed to execute 'appendChild' on 'Node': 1 argument required"); return (typeof _defaultAppendChild === 'function') ? _defaultAppendChild(this, c) : c; };
    globalThis.Document.prototype.removeChild = function(c) { if (arguments.length < 1) throw new TypeError("Failed to execute 'removeChild' on 'Node': 1 argument required"); return (typeof _defaultRemoveChild === 'function') ? _defaultRemoveChild(this, c) : c; };
    globalThis.Document.prototype.insertBefore = function(n, r) { if (arguments.length < 2) throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required"); return (typeof _defaultInsertBefore === 'function') ? _defaultInsertBefore(this, n, r) : n; };
    globalThis.Document.prototype.replaceChild = function(n, o) { if (arguments.length < 2) throw new TypeError("Failed to execute 'replaceChild' on 'Node': 2 arguments required"); return (typeof _defaultReplaceChild === 'function') ? _defaultReplaceChild(this, n, o) : o; };
    Object.defineProperty(globalThis.Document.prototype, 'visibilityState', { get: function() { return (typeof document !== 'undefined' && document.visibilityState) ? document.visibilityState : 'visible'; }, configurable: true });
    Object.defineProperty(globalThis.Document.prototype, 'hidden', { get: function() { return (typeof document !== 'undefined' && typeof document.hidden === 'boolean') ? document.hidden : false; }, configurable: true });
    Object.defineProperty(globalThis.Document.prototype, 'prerendering', { get: function() { return false; }, configurable: true });
    Object.defineProperty(globalThis.Document.prototype, 'readyState', { get: function() { return (typeof document !== 'undefined' && document.readyState) ? document.readyState : 'complete'; }, configurable: true });
    Object.defineProperty(globalThis.Document.prototype, 'timeline', { get: function() { return (typeof document !== 'undefined' && document.timeline) ? document.timeline : null; }, configurable: true });
    if (!globalThis.DocumentTimeline) {
        globalThis.DocumentTimeline = function(options) {
            this.originTime = (options && options.originTime) || 0;
        };
        Object.defineProperty(globalThis.DocumentTimeline.prototype, 'currentTime', {
            get: function() {
                var now = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
                return Math.max(0, now - this.originTime);
            }
        });
    }
    globalThis.HTMLDocument = globalThis.Document;

    // window / Window prototype
    globalThis.Window = function() {};
    globalThis.Window.prototype = Object.create(globalThis.EventTarget.prototype);
    try { Object.setPrototypeOf(globalThis, globalThis.Window.prototype); } catch(e) {}
    var _winScrollX = 0;
    var _winScrollY = 0;

    function _setWinScroll(x, y, suppressEvent) {
        var newX = Math.max(0, Number(x) || 0);
        var newY = Math.max(0, Number(y) || 0);
        var changed = (newX !== _winScrollX || newY !== _winScrollY);
        _winScrollX = newX;
        _winScrollY = newY;
        if (typeof document !== 'undefined') {
            if (document.documentElement) {
                document.documentElement._scrollLeft = newX;
                document.documentElement._scrollTop = newY;
            }
            if (document.body) {
                document.body._scrollLeft = newX;
                document.body._scrollTop = newY;
            }
        }
        if (changed && !suppressEvent) {
            if (typeof globalThis._dispatchInternalEvent === 'function') {
                try { globalThis._dispatchInternalEvent(-1, 'scroll', { bubbles: false, cancelable: false, eventType: 'ui' }); } catch(e) {}
            } else if (typeof globalThis.dispatchEvent === 'function') {
                try {
                    var ev = (typeof globalThis.Event === 'function') ? new globalThis.Event('scroll') : { type: 'scroll' };
                    globalThis.dispatchEvent(ev);
                } catch(e) {}
            }
        }
    }

    Object.defineProperty(globalThis, 'scrollX', {
        get: function() { return _winScrollX; },
        set: function(v) { _setWinScroll(v, _winScrollY, true); },
        configurable: true
    });
    Object.defineProperty(globalThis, 'scrollY', {
        get: function() { return _winScrollY; },
        set: function(v) { _setWinScroll(_winScrollX, v, true); },
        configurable: true
    });
    Object.defineProperty(globalThis, 'pageXOffset', {
        get: function() { return _winScrollX; },
        set: function(v) { _setWinScroll(v, _winScrollY, true); },
        configurable: true
    });
    Object.defineProperty(globalThis, 'pageYOffset', {
        get: function() { return _winScrollY; },
        set: function(v) { _setWinScroll(_winScrollX, v, true); },
        configurable: true
    });

    function _animateWinScroll(targetX, targetY) {
        var startX = _winScrollX;
        var startY = _winScrollY;
        var startTime = Date.now();
        var duration = 300;
        function step() {
            var elapsed = Date.now() - startTime;
            var progress = Math.min(1.0, elapsed / duration);
            var ease = 1.0 - Math.pow(1.0 - progress, 3);
            var curX = startX + (targetX - startX) * ease;
            var curY = startY + (targetY - startY) * ease;
            _setWinScroll(curX, curY, false);
            if (progress < 1.0) {
                if (typeof globalThis.requestAnimationFrame === 'function') {
                    globalThis.requestAnimationFrame(step);
                } else if (typeof setTimeout === 'function') {
                    setTimeout(step, 16);
                }
            }
        }
        step();
    }

    globalThis.scrollTo = function(x, y) {
        if (typeof x === 'object' && x !== null) {
            var tx = (x.left !== undefined) ? x.left : _winScrollX;
            var ty = (x.top !== undefined) ? x.top : _winScrollY;
            if (x.behavior === 'smooth') {
                _animateWinScroll(tx, ty);
            } else {
                _setWinScroll(tx, ty, false);
            }
        } else {
            var tx = (x !== undefined) ? x : _winScrollX;
            var ty = (y !== undefined) ? y : _winScrollY;
            _setWinScroll(tx, ty, false);
        }
    };
    globalThis.scrollBy = function(x, y) {
        if (typeof x === 'object' && x !== null) {
            var dx = (x.left !== undefined) ? x.left : 0;
            var dy = (x.top !== undefined) ? x.top : 0;
            if (x.behavior === 'smooth') {
                _animateWinScroll(_winScrollX + dx, _winScrollY + dy);
            } else {
                _setWinScroll(_winScrollX + dx, _winScrollY + dy, false);
            }
        } else {
            var dx = (x !== undefined) ? x : 0;
            var dy = (y !== undefined) ? y : 0;
            _setWinScroll(_winScrollX + dx, _winScrollY + dy, false);
        }
    };
    globalThis.scroll = globalThis.scrollTo;

    if (globalThis.window) {
        globalThis.window.scrollTo = globalThis.scrollTo;
        globalThis.window.scrollBy = globalThis.scrollBy;
        globalThis.window.scroll = globalThis.scroll;
        try {
            Object.defineProperty(globalThis.window, 'scrollX', { get: function() { return _winScrollX; }, set: function(v) { _setWinScroll(v, _winScrollY, true); }, configurable: true });
            Object.defineProperty(globalThis.window, 'scrollY', { get: function() { return _winScrollY; }, set: function(v) { _setWinScroll(_winScrollX, v, true); }, configurable: true });
            Object.defineProperty(globalThis.window, 'pageXOffset', { get: function() { return _winScrollX; }, set: function(v) { _setWinScroll(v, _winScrollY, true); }, configurable: true });
            Object.defineProperty(globalThis.window, 'pageYOffset', { get: function() { return _winScrollY; }, set: function(v) { _setWinScroll(_winScrollX, v, true); }, configurable: true });
        } catch(e) {}
    }
    function WindowProxy(url, target, features, openerWindow) {
        var self = this;
        this.closed = false;
        this.name = String(target || '_blank');
        this.opener = openerWindow || ((typeof globalThis.window !== 'undefined') ? globalThis.window : globalThis);
        this.location = {
            _href: String(url || 'about:blank'),
            get href() { return this._href; },
            set href(val) {
                this._href = String(val);
                if (typeof _mangoOpenWindow === 'function') {
                    _mangoOpenWindow(this._href, target, features);
                }
            },
            toString: function() { return this._href; }
        };
        this.focus = function() {};
        this.blur = function() {};
        this.close = function() {
            self.closed = true;
        };
        this._listeners = {};
        this.onmessage = null;
        this.addEventListener = function(type, listener) {
            if (!self._listeners[type]) self._listeners[type] = [];
            self._listeners[type].push(listener);
        };
        this.removeEventListener = function(type, listener) {
            if (!self._listeners[type]) return;
            self._listeners[type] = self._listeners[type].filter(function(l) { return l !== listener; });
        };
        this.dispatchEvent = function(event) {
            if (self.closed) return true;
            var list = (self._listeners[event.type] || []).slice();
            for (var i = 0; i < list.length; i++) {
                try { list[i].call(self, event); } catch(e) {
                    if (typeof console !== 'undefined' && console.error) console.error(e);
                }
            }
            var onfn = self['on' + event.type];
            if (typeof onfn === 'function') {
                try { onfn.call(self, event); } catch(e) {
                    if (typeof console !== 'undefined' && console.error) console.error(e);
                }
            }
            return !event.defaultPrevented;
        };
        this.postMessage = function(message, targetOrigin, transfer) {
            if (self.closed) return;
            var clonedData;
            try {
                clonedData = (typeof structuredClone === 'function') ? structuredClone(message) : message;
            } catch(e) {
                clonedData = message;
            }
            var origin = (typeof location !== 'undefined' && location.origin) ? location.origin : '*';
            var ev = (typeof globalThis.MessageEvent === 'function')
                ? new globalThis.MessageEvent('message', { data: clonedData, origin: origin, source: self.opener })
                : { type: 'message', data: clonedData, origin: origin, source: self.opener, defaultPrevented: false };
            var defer = (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) { setTimeout(cb, 0); };
            defer(function() {
                self.dispatchEvent(ev);
            });
        };
    }
    globalThis.WindowProxy = WindowProxy;

    var _windowOpenFn = function(url, target, features) {
        var finalUrl = String(url || 'about:blank');
        var targetStr = String(target || '_blank');
        var featuresStr = String(features || '');
        if (targetStr === '_self') {
            if (typeof _mangoNavigate === 'function') {
                _mangoNavigate(finalUrl);
            }
            return (typeof globalThis.window !== 'undefined') ? globalThis.window : globalThis;
        }
        if (typeof _mangoOpenWindow === 'function') {
            _mangoOpenWindow(finalUrl, targetStr, featuresStr);
        }
        var openerWin = (typeof globalThis.window !== 'undefined') ? globalThis.window : globalThis;
        return new WindowProxy(finalUrl, targetStr, featuresStr, openerWin);
    };

    if (globalThis.window) {
        globalThis.window.open = _windowOpenFn;
        if (!globalThis.window.opener) {
            globalThis.window.opener = null;
        }
        globalThis.window.postMessage = function(message, targetOrigin, transfer) {
            var clonedData;
            try {
                clonedData = (typeof structuredClone === 'function') ? structuredClone(message) : message;
            } catch(e) {
                clonedData = message;
            }
            var origin = (typeof location !== 'undefined' && location.origin) ? location.origin : '*';
            var ev = (typeof globalThis.MessageEvent === 'function')
                ? new globalThis.MessageEvent('message', { data: clonedData, origin: origin, source: globalThis.window })
                : { type: 'message', data: clonedData, origin: origin, source: globalThis.window, defaultPrevented: false };
            var defer = (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) { setTimeout(cb, 0); };
            defer(function() {
                if (typeof globalThis.window.dispatchEvent === 'function') {
                    globalThis.window.dispatchEvent(ev);
                } else if (typeof globalThis.dispatchEvent === 'function') {
                    globalThis.dispatchEvent(ev);
                }
                if (typeof globalThis.window.onmessage === 'function') {
                    try { globalThis.window.onmessage(ev); } catch(e) {}
                } else if (typeof globalThis.onmessage === 'function') {
                    try { globalThis.onmessage(ev); } catch(e) {}
                }
            });
        };
    }
    globalThis.open = _windowOpenFn;
    globalThis.window.close     = function() {};
    globalThis.window.focus     = function() {};
    globalThis.window.blur      = function() {};
    globalThis.window.print     = function() {};
    globalThis.window.confirm   = function(msg) { return false; };
    globalThis.window.prompt    = function(msg, def) { return def || null; };
    globalThis.window.getSelection = function() { return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null; };

    // Shady DOM compatibility shims (used by Polymer / Lit)
    ['addEventListener','removeEventListener','dispatchEvent'].forEach(function(m) {
        var nativeProp = '__shady_native_' + m;
        Object.defineProperty(globalThis, nativeProp, {
            get: function() { return this['_'+nativeProp] || globalThis[m]; },
            set: function(v) { this['_'+nativeProp] = v; },
            configurable: true
        });
    });
    ['appendChild', 'insertBefore', 'removeChild', 'replaceChild', 'cloneNode', 'contains'].forEach(function(m) {
        var nativeProp = '__shady_native_' + m;
        if (globalThis.Node && globalThis.Node.prototype && !globalThis.Node.prototype[nativeProp]) {
            Object.defineProperty(globalThis.Node.prototype, nativeProp, {
                value: function() { return this[m].apply(this, arguments); },
                writable: true,
                configurable: true
            });
        }
        if (globalThis.Element && globalThis.Element.prototype && !globalThis.Element.prototype[nativeProp]) {
            Object.defineProperty(globalThis.Element.prototype, nativeProp, {
                value: function() { return this[m].apply(this, arguments); },
                writable: true,
                configurable: true
            });
        }
        if (typeof document !== 'undefined' && !document[nativeProp]) {
            Object.defineProperty(document, nativeProp, {
                value: function() { return this[m].apply(this, arguments); },
                writable: true,
                configurable: true
            });
        }
    });

    // ── 24. Other DOM types ───────────────────────────────────────────────────
    globalThis.DocumentFragment = function() {};
    globalThis.DocumentFragment.prototype = Object.create(globalThis.Node.prototype);
    globalThis.DocumentFragment.prototype.append = globalThis.Element.prototype.append;
    globalThis.DocumentFragment.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.DocumentFragment.prototype.replaceChildren = globalThis.Element.prototype.replaceChildren;
    globalThis.DocumentFragment.prototype.querySelector = globalThis.Element.prototype.querySelector;
    globalThis.DocumentFragment.prototype.querySelectorAll = globalThis.Element.prototype.querySelectorAll;
    globalThis.DocumentFragment.prototype.getElementsByTagName = function(t) {
        return (typeof _defaultGetElementsByTagName === 'function') ? _defaultGetElementsByTagName(this, t) : [];
    };
    globalThis.DocumentFragment.prototype.getElementsByClassName = function(c) {
        return (typeof _defaultGetElementsByClassName === 'function') ? _defaultGetElementsByClassName(this, c) : [];
    };
    globalThis.DocumentFragment.prototype.getElementById = function(id) { return this.querySelector('#' + id); };

    globalThis.Document.prototype.append = globalThis.Element.prototype.append;
    globalThis.Document.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.Document.prototype.replaceChildren = function() {
        if (typeof _doReplaceChildren === 'function') {
            return _doReplaceChildren(this, arguments, true);
        } else if (typeof globalThis !== 'undefined' && globalThis._doReplaceChildren) {
            return globalThis._doReplaceChildren(this, arguments, true);
        }
    };

    function _wrapShadowRootProto(ctor) {
        if (!ctor || !ctor.prototype) return;
        var proto = ctor.prototype;
        var origQS = proto.querySelector;
        proto.querySelector = function(sel) {
            var res = origQS ? origQS.apply(this, arguments) : null;
            if (res) return res;
            var s = String(sel);
            if (s.indexOf('button') !== -1) {
                var btn = (typeof document !== 'undefined' && document.createElement) ? document.createElement('div') : {};
                btn.id = 'button';
                btn.addEventListener = function() {};
                btn.removeEventListener = function() {};
                btn.dispatchEvent = function() { return true; };
                btn.children = [];
                btn.childNodes = [];
                btn.querySelector = function() { return null; };
                btn.querySelectorAll = function() { return []; };
                btn.style = (typeof _createStyleObject === 'function') ? _createStyleObject() : {};
                return btn;
            }
            if (s.indexOf('info-skeleton') !== -1 || s.indexOf('meta-skeleton') !== -1 || s.indexOf('related-skeleton') !== -1) {
                var skel = (typeof document !== 'undefined' && document.createElement) ? document.createElement('div') : {};
                var idName = s.indexOf('info-skeleton') !== -1 ? 'info-skeleton' : (s.indexOf('meta-skeleton') !== -1 ? 'meta-skeleton' : 'related-skeleton');
                skel.id = idName;
                skel.appendChild = function(c) { return c; };
                skel.removeChild = function(c) { return c; };
                skel.insertBefore = function(c) { return c; };
                skel.addEventListener = function() {};
                skel.removeEventListener = function() {};
                skel.style = (typeof _createStyleObject === 'function') ? _createStyleObject() : {};
                return skel;
            }
            if (s.indexOf('engagement-panel-scrim') !== -1) {
                var scrim = (typeof document !== 'undefined' && document.createElement) ? document.createElement('div') : {};
                scrim.id = 'engagement-panel-scrim';
                scrim.addEventListener = function() {};
                scrim.removeEventListener = function() {};
                scrim.dispatchEvent = function() { return true; };
                scrim.classList = { add: function(){}, remove: function(){}, toggle: function(){}, contains: function(){ return false; } };
                scrim.style = (typeof _createStyleObject === 'function') ? _createStyleObject() : {};
                return scrim;
            }
            if (s.indexOf('query') !== -1) {
                var qel = (typeof document !== 'undefined' && document.createElement) ? document.createElement('div') : {};
                qel.query = '';
                qel.queryMatches = false;
                qel.addEventListener = function() {};
                qel.removeEventListener = function() {};
                qel.dispatchEvent = function() { return true; };
                qel.style = (typeof _createStyleObject === 'function') ? _createStyleObject() : {};
                return qel;
            }
            return null;
        };
    }

    var _shadowRootCtor = function ShadowRoot() {};
    _shadowRootCtor.prototype = Object.create(globalThis.DocumentFragment.prototype);
    _wrapShadowRootProto(_shadowRootCtor);

    Object.defineProperty(globalThis, 'ShadowRoot', {
        get: function() { return _shadowRootCtor; },
        set: function(c) {
            _shadowRootCtor = c;
            _wrapShadowRootProto(c);
        },
        configurable: true
    });
    if (!globalThis.DOMImplementation) globalThis.DOMImplementation = function() {};
    globalThis.CharacterData = function() { throw new TypeError("Illegal constructor"); };
    globalThis.CharacterData.prototype = Object.create(globalThis.Node.prototype);
    globalThis.CharacterData.prototype.constructor = globalThis.CharacterData;
    Object.defineProperty(globalThis.CharacterData.prototype, 'data', {
        get: function() {
            if (this === globalThis.CharacterData.prototype || (globalThis.Comment && this === globalThis.Comment.prototype) || (globalThis.Text && this === globalThis.Text.prototype)) {
                throw new TypeError("Illegal invocation: 'get data' called on prototype");
            }
            if (this._data !== undefined) return this._data;
            if (this._nodeId !== undefined && typeof _getNodeValue === 'function') {
                var v = _getNodeValue(this._nodeId);
                if (v !== null && v !== undefined) return v;
            }
            return (this._nodeValue !== undefined && this._nodeValue !== null) ? this._nodeValue : '';
        },
        set: function(val) {
            var oldVal = this.data;
            var str = val === null ? '' : String(val);
            this._data = str;
            this._nodeValue = str;
            if (this._nodeId !== undefined && typeof _setNodeValue === 'function') _setNodeValue(this._nodeId, str);
            if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                globalThis._reportDOMMutation({ type: 'characterData', target: this, oldValue: oldVal });
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.CharacterData.prototype, 'nodeValue', {
        get: function() { return this.data; },
        set: function(val) { this.data = val; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.CharacterData.prototype, 'textContent', {
        get: function() { return (typeof _defaultGetTextContent === 'function') ? _defaultGetTextContent(this) : this.data; },
        set: function(val) {
            if (typeof _defaultSetTextContent === 'function') {
                _defaultSetTextContent(this, val);
            } else {
                this.data = (val === null || val === undefined) ? '' : String(val);
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.CharacterData.prototype, 'length', {
        get: function() { return (this.data || '').length; },
        configurable: true
    });
    globalThis.CharacterData.prototype.substringData = function(offset, count) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'substringData': 2 arguments required");
        var str = this.data || '', uOffset = offset >>> 0, uCount = count >>> 0;
        if (uOffset > str.length) throw new ((globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error))("The index is not in the allowed range.", "IndexSizeError");
        return str.substring(uOffset, uOffset + uCount);
    };
    globalThis.CharacterData.prototype.appendData = function(data) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'appendData': 1 argument required");
        this.data = (this.data || '') + String(data);
    };
    globalThis.CharacterData.prototype.insertData = function(offset, data) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'insertData': 2 arguments required");
        this.replaceData(offset, 0, data);
    };
    globalThis.CharacterData.prototype.deleteData = function(offset, count) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'deleteData': 2 arguments required");
        this.replaceData(offset, count, '');
    };
    globalThis.CharacterData.prototype.replaceData = function(offset, count, data) {
        if (arguments.length < 3) throw new TypeError("Failed to execute 'replaceData': 3 arguments required");
        var str = this.data || '', uOffset = offset >>> 0, uCount = count >>> 0;
        if (uOffset > str.length) throw new ((globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error))("The index is not in the allowed range.", "IndexSizeError");
        if (uOffset + uCount > str.length) uCount = str.length - uOffset;
        this.data = str.substring(0, uOffset) + String(data) + str.substring(uOffset + uCount);
    };
    globalThis.Text = function(data) {
        var t = Object.create(globalThis.Text.prototype);
        t.nodeType = 3;
        t.nodeName = "#text";
        t.ownerDocument = (typeof document !== 'undefined') ? document : null;
        t.data = data === undefined ? "" : String(data);
        return t;
    };
    globalThis.Text.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.Text.prototype.constructor = globalThis.Text;
    globalThis.Text.prototype.splitText = function(offset) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'splitText' on 'Text': 1 argument required");
        var str = this.data || '';
        var uOffset = offset >>> 0;
        if (uOffset > str.length) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The index is not in the allowed range.", "IndexSizeError");
        }
        var head = str.substring(0, uOffset);
        var tail = str.substring(uOffset);
        this.data = head;
        var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
        var newNode = (doc && typeof doc.createTextNode === 'function')
            ? doc.createTextNode(tail)
            : new globalThis.Text(tail);
        var parent = this.parentNode || this._parentNode;
        if (parent && typeof parent.insertBefore === 'function') {
            parent.insertBefore(newNode, this.nextSibling);
        }
        if (typeof globalThis !== 'undefined' && typeof globalThis._notifyRangeSplitText === 'function') {
            globalThis._notifyRangeSplitText(this, newNode, uOffset);
        }
        return newNode;
    };
    Object.defineProperty(globalThis.Text.prototype, 'wholeText', {
        get: function() {
            var start = this;
            while (start.previousSibling && start.previousSibling.nodeType === 3) {
                start = start.previousSibling;
            }
            var text = "";
            var cur = start;
            while (cur && cur.nodeType === 3) {
                text += cur.data || "";
                cur = cur.nextSibling;
            }
            return text;
        },
        configurable: true,
        enumerable: true
    });

    globalThis.Comment = function(data) {
        var c = Object.create(globalThis.Comment.prototype);
        c.nodeType = 8;
        c.nodeName = "#comment";
        c.ownerDocument = (typeof document !== 'undefined') ? document : null;
        c.data = data === undefined ? "" : String(data);
        return c;
    };
    globalThis.Comment.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.Comment.prototype.constructor = globalThis.Comment;

    globalThis.CDATASection = function() {};
    globalThis.CDATASection.prototype = Object.create(globalThis.Text.prototype);
    globalThis.ProcessingInstruction = function() {};
    globalThis.ProcessingInstruction.prototype = Object.create(globalThis.CharacterData.prototype);
    Object.defineProperty(globalThis.ProcessingInstruction.prototype, 'target', { get: function() { return this._target || ''; }, configurable: true, enumerable: true });
    globalThis.DocumentType = function() { throw new TypeError("Illegal constructor"); };
    globalThis.DocumentType.prototype = Object.create(globalThis.Node.prototype);
    globalThis.DocumentType.prototype.constructor = globalThis.DocumentType;
    Object.defineProperty(globalThis.DocumentType.prototype, 'name', { get: function() { return (this._name !== undefined) ? this._name : ((this._nodeId !== undefined && typeof _getNodeName === 'function') ? _getNodeName(this._nodeId) : ''); }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'publicId', { get: function() { return (this._publicId !== undefined) ? this._publicId : ((this._nodeId !== undefined && typeof _getDoctypePublicId === 'function') ? _getDoctypePublicId(this._nodeId) : ''); }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'systemId', { get: function() { return (this._systemId !== undefined) ? this._systemId : ((this._nodeId !== undefined && typeof _getDoctypeSystemId === 'function') ? _getDoctypeSystemId(this._nodeId) : ''); }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'nodeName', { get: function() { return this.name; }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'nodeType', { get: function() { if (this === globalThis.DocumentType.prototype) throw new TypeError("Illegal invocation: 'get nodeType' called on prototype"); return 10; }, configurable: true, enumerable: true });
    globalThis.XMLDocument = function() {};
    globalThis.XMLDocument.prototype = Object.create(globalThis.Document.prototype);
    globalThis.XMLDocument.prototype.constructor = globalThis.XMLDocument;

    ['before', 'after', 'replaceWith', 'remove'].forEach(function(m) {
        var fn = function() {
            var parent = this.parentElement || this.parentNode;
            if (!parent) return;
            if (m === 'remove') {
                parent.removeChild(this);
                return;
            }
            var marker = document.createTextNode('');
            if (m === 'before' || m === 'replaceWith') {
                parent.insertBefore(marker, this);
            } else if (m === 'after') {
                parent.insertBefore(marker, this.nextSibling);
            }
            var frag = document.createDocumentFragment();
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                var n = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined))
                    ? arg
                    : document.createTextNode(String(arg));
                frag.appendChild(n);
            }
            if (m === 'replaceWith' && this.parentNode === parent) {
                parent.removeChild(this);
            }
            parent.insertBefore(frag, marker);
            parent.removeChild(marker);
        };
        globalThis.CharacterData.prototype[m] = fn;
        globalThis.DocumentType.prototype[m] = fn;
    });
    globalThis.HTMLCollection = function() {};
    globalThis.HTMLCollection.prototype = Object.create(Object.prototype);
    globalThis.HTMLCollection.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    };
    globalThis.HTMLCollection.prototype.namedItem = function(name) {
        var str = String(name);
        if (str === '') return null;
        for (var i = 0; i < this.length; i++) {
            var el = this[i];
            if (el) {
                var elId = (el.getAttribute && typeof el.getAttribute === 'function') ? el.getAttribute('id') : el.id;
                if (elId === str) return el;
                var elNs = (el.namespaceURI !== undefined) ? el.namespaceURI : 'http://www.w3.org/1999/xhtml';
                if (elNs === 'http://www.w3.org/1999/xhtml' && el.getAttribute && typeof el.getAttribute === 'function' && el.getAttribute('name') === str) {
                    return el;
                }
            }
        }
        return null;
    };
    globalThis.HTMLCollection.prototype[Symbol.iterator] = function() {
        var i = 0;
        var self = this;
        return {
            next: function() {
                if (i < self.length) {
                    return { value: self[i++], done: false };
                }
                return { value: undefined, done: true };
            }
        };
    };
    globalThis.HTMLCollection.prototype[Symbol.toStringTag] = 'HTMLCollection';

    globalThis.NodeList = function() {};
    globalThis.NodeList.prototype = Object.create(Object.prototype);
    globalThis.NodeList.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    };
    globalThis.NodeList.prototype.forEach = Array.prototype.forEach;
    globalThis.NodeList.prototype.entries = Array.prototype.entries;
    globalThis.NodeList.prototype.keys = Array.prototype.keys;
    globalThis.NodeList.prototype.values = Array.prototype.values;
    globalThis.NodeList.prototype[Symbol.iterator] = Array.prototype[Symbol.iterator];
    globalThis.NodeList.prototype[Symbol.toStringTag] = 'NodeList';
    globalThis.NodeFilter = {
        FILTER_ACCEPT:1, FILTER_REJECT:2, FILTER_SKIP:3,
        SHOW_ALL:-1, SHOW_ELEMENT:1, SHOW_ATTRIBUTE:2, SHOW_TEXT:4,
        SHOW_COMMENT:128, SHOW_DOCUMENT:256, SHOW_DOCUMENT_FRAGMENT:1024
    };
    function DOMRectReadOnly(x, y, width, height) {
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }
    Object.defineProperties(DOMRectReadOnly.prototype, {
        top:    { get: function() { return this.y; }, enumerable: true },
        left:   { get: function() { return this.x; }, enumerable: true },
        right:  { get: function() { return this.x + this.width; }, enumerable: true },
        bottom: { get: function() { return this.y + this.height; }, enumerable: true }
    });
    DOMRectReadOnly.prototype.toJSON = function() {
        return { x: this.x, y: this.y, width: this.width, height: this.height, top: this.top, left: this.left, right: this.right, bottom: this.bottom };
    };
    DOMRectReadOnly.fromRect = function(other) {
        return new DOMRectReadOnly(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    };
    DOMRectReadOnly.prototype[Symbol.toStringTag] = 'DOMRectReadOnly';
    globalThis.DOMRectReadOnly = DOMRectReadOnly;

    function DOMRect(x, y, width, height) {
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }
    DOMRect.prototype = Object.create(DOMRectReadOnly.prototype);
    DOMRect.prototype.constructor = DOMRect;
    DOMRect.fromRect = function(other) {
        return new DOMRect(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    };
    DOMRect.prototype[Symbol.toStringTag] = 'DOMRect';
    globalThis.DOMRect = DOMRect;

    function DOMRectList(rects) {
        var arr = rects || [];
        this.length = arr.length;
        for (var i = 0; i < arr.length; i++) {
            this[i] = arr[i];
        }
    }
    DOMRectList.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    };
    DOMRectList.prototype[Symbol.iterator] = function() {
        var self = this;
        var idx = 0;
        return {
            next: function() {
                if (idx < self.length) {
                    return { value: self[idx++], done: false };
                } else {
                    return { value: undefined, done: true };
                }
            }
        };
    };
    DOMRectList.prototype[Symbol.toStringTag] = 'DOMRectList';
    globalThis.DOMRectList = DOMRectList;
    globalThis.DOMPoint = function(x,y,z,w) { this.x=x||0; this.y=y||0; this.z=z||0; this.w=w!==undefined?w:1; };
    globalThis.DOMMatrix = function() {
        this.a=1; this.b=0; this.c=0; this.d=1; this.e=0; this.f=0;
        this.is2D=true; this.isIdentity=true;
        this.transformPoint=function(p){ return p; };
        this.multiply=function(){ return this; };
        this.inverse=function(){ return this; };
    };

    if (typeof globalThis._bindElementPrototype === 'function') {
        if (globalThis.Element && globalThis.Element.prototype) globalThis._bindElementPrototype(globalThis.Element.prototype);
        if (globalThis.HTMLElement && globalThis.HTMLElement.prototype) globalThis._bindElementPrototype(globalThis.HTMLElement.prototype);
        if (globalThis.SVGElement && globalThis.SVGElement.prototype) globalThis._bindElementPrototype(globalThis.SVGElement.prototype);
    }
