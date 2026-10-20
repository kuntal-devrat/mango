    globalThis.Document.prototype.createElement      = function(t) { return (typeof document !== 'undefined') ? document.createElement(t) : null; };
    globalThis.Document.prototype.createElementNS    = function(ns, t) { return (typeof document !== 'undefined' && document.createElementNS) ? document.createElementNS(ns, t) : this.createElement(t); };
    globalThis.Document.prototype.createTextNode     = function(t) { return (typeof document !== 'undefined') ? document.createTextNode(t) : null; };
    globalThis.Document.prototype.createDocumentFragment = function() { return (typeof document !== 'undefined' && document.createDocumentFragment) ? document.createDocumentFragment() : null; };
    globalThis.Document.prototype.querySelector      = function(s) { return (typeof document !== 'undefined') ? document.querySelector(s) : null; };
    globalThis.Document.prototype.querySelectorAll   = function(s) { return (typeof document !== 'undefined') ? document.querySelectorAll(s) : []; };
    globalThis.Document.prototype.getElementById     = function(i) { return (typeof document !== 'undefined') ? document.getElementById(i) : null; };
    globalThis.Document.prototype.getElementsByTagName    = function(t) { return (typeof document !== 'undefined') ? document.getElementsByTagName(t) : []; };
    globalThis.Document.prototype.getElementsByClassName  = function(c) { return (typeof document !== 'undefined') ? document.getElementsByClassName(c) : []; };
    globalThis.Document.prototype.createEvent        = function(t) { return (typeof document !== 'undefined' && document.createEvent) ? document.createEvent(t) : new globalThis.Event(t); };
    globalThis.Document.prototype.importNode         = function(n, d) { return n && n.cloneNode ? n.cloneNode(d) : n; };
    globalThis.Document.prototype.adoptNode          = function(n) { return n; };
    globalThis.Document.prototype.elementFromPoint   = function(x, y) { return (typeof document !== 'undefined' && document.elementFromPoint) ? document.elementFromPoint(x, y) : null; };
    globalThis.Document.prototype.elementsFromPoint  = function(x, y) { return (typeof document !== 'undefined' && document.elementsFromPoint) ? document.elementsFromPoint(x, y) : []; };
    globalThis.Document.prototype.caretPositionFromPoint = function() { return null; };
    globalThis.Document.prototype.createRange        = function() {
        return (typeof document !== 'undefined' && document.createRange) ? document.createRange() : new globalThis.Range();
    };
    globalThis.Document.prototype.getSelection      = function() {
        return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null;
    };
    Object.defineProperty(globalThis.Document.prototype, 'activeElement', {
        get: function() { return (typeof document !== 'undefined') ? document.activeElement : null; },
        set: function(val) { if (typeof document !== 'undefined') document.activeElement = val; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'characterSet', {
        get: function() { return (typeof document !== 'undefined' && document.characterSet) ? document.characterSet : 'UTF-8'; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'charset', {
        get: function() { return (typeof document !== 'undefined' && document.charset) ? document.charset : 'UTF-8'; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'inputEncoding', {
        get: function() { return (typeof document !== 'undefined' && document.inputEncoding) ? document.inputEncoding : 'UTF-8'; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'contentType', {
        get: function() { return (typeof document !== 'undefined' && document.contentType) ? document.contentType : 'text/html'; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'documentElement', {
        get: function() { return (typeof document !== 'undefined') ? document.documentElement : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'head', {
        get: function() { return (typeof document !== 'undefined') ? document.head : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'body', {
        get: function() { return (typeof document !== 'undefined') ? document.body : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'forms', {
        get: function() { return (typeof document !== 'undefined') ? document.forms : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'images', {
        get: function() { return (typeof document !== 'undefined') ? document.images : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'links', {
        get: function() { return (typeof document !== 'undefined') ? document.links : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'scripts', {
        get: function() { return (typeof document !== 'undefined') ? document.scripts : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'children', {
        get: function() { return (typeof document !== 'undefined') ? document.children : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'childNodes', {
        get: function() { return (typeof document !== 'undefined') ? document.childNodes : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'firstChild', {
        get: function() { return (typeof document !== 'undefined') ? document.firstChild : null; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'lastChild', {
        get: function() { return (typeof document !== 'undefined') ? document.lastChild : null; },
        configurable: true
    });
    globalThis.Document.prototype.appendChild = function(c) { return (typeof document !== 'undefined' && document.appendChild) ? document.appendChild(c) : c; };
    globalThis.Document.prototype.removeChild = function(c) { return (typeof document !== 'undefined' && document.removeChild) ? document.removeChild(c) : c; };
    globalThis.Document.prototype.insertBefore = function(n, r) { return (typeof document !== 'undefined' && document.insertBefore) ? document.insertBefore(n, r) : n; };
    globalThis.Document.prototype.replaceChild = function(n, o) { return (typeof document !== 'undefined' && document.replaceChild) ? document.replaceChild(n, o) : o; };
    Object.defineProperty(globalThis.Document.prototype, 'visibilityState', {
        get: function() { return (typeof document !== 'undefined' && document.visibilityState) ? document.visibilityState : 'visible'; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'hidden', {
        get: function() { return (typeof document !== 'undefined' && typeof document.hidden === 'boolean') ? document.hidden : false; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'prerendering', {
        get: function() { return false; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'readyState', {
        get: function() { return (typeof document !== 'undefined' && document.readyState) ? document.readyState : 'complete'; },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'timeline', {
        get: function() { return (typeof document !== 'undefined' && document.timeline) ? document.timeline : null; },
        configurable: true
    });
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
    globalThis.Document.prototype.replaceChildren = globalThis.Element.prototype.replaceChildren;

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
    globalThis.DOMImplementation = function() {};
    globalThis.CharacterData = function() {};
    globalThis.CharacterData.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Text = function() {};
    globalThis.Text.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.Comment = function() {};
    globalThis.Comment.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.CDATASection = function() {};
    globalThis.CDATASection.prototype = Object.create(globalThis.Text.prototype);
    globalThis.ProcessingInstruction = function() {};
    globalThis.ProcessingInstruction.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.DocumentType = function() {};
    globalThis.DocumentType.prototype = Object.create(globalThis.Node.prototype);
    globalThis.XMLDocument = function() {};
    globalThis.XMLDocument.prototype = Object.create(globalThis.Document.prototype);
    globalThis.Attr = function() {};
    globalThis.HTMLCollection = function() {};
    globalThis.HTMLCollection.prototype = Object.create(Object.prototype);
    globalThis.HTMLCollection.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    };
    globalThis.HTMLCollection.prototype.namedItem = function(name) {
        var str = String(name);
        for (var i = 0; i < this.length; i++) {
            var el = this[i];
            if (el && (el.id === str || (el.getAttribute && el.getAttribute('name') === str))) {
                return el;
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
    globalThis.NodeList.prototype.forEach = function(cb, thisArg) {
        for (var i = 0; i < this.length; i++) {
            cb.call(thisArg, this[i], i, this);
        }
    };
    globalThis.NodeList.prototype.entries = function() {
        var i = 0;
        var self = this;
        return {
            next: function() {
                if (i < self.length) {
                    return { value: [i, self[i++]], done: false };
                }
                return { value: undefined, done: true };
            },
            [Symbol.iterator]: function() { return this; }
        };
    };
    globalThis.NodeList.prototype.keys = function() {
        var i = 0;
        var self = this;
        return {
            next: function() {
                if (i < self.length) {
                    return { value: i++, done: false };
                }
                return { value: undefined, done: true };
            },
            [Symbol.iterator]: function() { return this; }
        };
    };
    globalThis.NodeList.prototype.values = function() {
        var i = 0;
        var self = this;
        return {
            next: function() {
                if (i < self.length) {
                    return { value: self[i++], done: false };
                }
                return { value: undefined, done: true };
            },
            [Symbol.iterator]: function() { return this; }
        };
    };
    globalThis.NodeList.prototype[Symbol.iterator] = function() {
        return this.values();
    };
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


