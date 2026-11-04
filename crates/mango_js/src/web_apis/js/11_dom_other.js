    globalThis.Document.prototype.createElement = function(t) { return this._isXml ? (typeof _createElementXML === 'function' ? _createElementXML(this, t) : null) : ((typeof document !== 'undefined' && document.createElement) ? document.createElement(t) : null); };
    globalThis.Document.prototype.createElementNS = function(ns, t) { return (typeof _createElementNS === 'function') ? _createElementNS(this, ns, t) : ((typeof document !== 'undefined' && document.createElementNS) ? document.createElementNS(ns, t) : this.createElement(t)); };
    globalThis.Document.prototype.createTextNode     = function(t) { return (typeof document !== 'undefined') ? document.createTextNode(t) : null; };
    globalThis.Document.prototype.createComment      = function(t) { return (typeof document !== 'undefined' && document.createComment) ? document.createComment(t) : null; };
    globalThis.Document.prototype.createProcessingInstruction = function(target, data) { return (typeof document !== 'undefined' && document.createProcessingInstruction) ? document.createProcessingInstruction(target, data) : null; };
    globalThis.Document.prototype.createDocumentFragment = function() { return (typeof document !== 'undefined' && document.createDocumentFragment) ? document.createDocumentFragment() : null; };
    globalThis.Document.prototype.querySelector      = function(s) {
        if (this === document && typeof document !== 'undefined') return document.querySelector(s);
        return (typeof _defaultQuerySelector === 'function') ? _defaultQuerySelector(this, s) : null;
    };
    globalThis.Document.prototype.querySelectorAll   = function(s) {
        if (this === document && typeof document !== 'undefined') return document.querySelectorAll(s);
        return (typeof _defaultQuerySelectorAll === 'function') ? _defaultQuerySelectorAll(this, s) : _createNodeList(function() { return []; });
    };
    globalThis.Document.prototype.getElementById     = function(i) {
        if (arguments.length === 0) return null;
        var s = String(i);
        if (this === document) {
            if (typeof globalThis._origGetById === 'function') {
                var nid = globalThis._origGetById(s);
                if (nid !== -1 && nid !== undefined && nid !== 0 && nid !== null) {
                    var w = (typeof _wrapElement === 'function' ? _wrapElement(nid) : (globalThis._wrapElement ? globalThis._wrapElement(nid) : (globalThis._mangoWrap ? globalThis._mangoWrap(nid) : null)));
                    if (w) return w;
                }
            }
            if (typeof document !== 'undefined' && typeof document.getElementById === 'function' && document.getElementById !== globalThis.Document.prototype.getElementById) {
                var el = document.getElementById(s);
                if (el) return el;
            }
        }
        function findId(n) {
            if (!n) return null;
            if (n.nodeType === 1 && ((n.getAttribute && n.getAttribute('id')) || n.id) === s) return n;
            var kids = n.childNodes || n._childNodes || [];
            for (var k = 0; k < kids.length; k++) { var found = findId(kids[k]); if (found) return found; }
            return null;
        }
        return findId(this.documentElement || this);
    };
    globalThis.Document.prototype.getElementsByTagName = function(t) {
        if (this === document && typeof document !== 'undefined' && typeof document.getElementsByTagName === 'function' && document.getElementsByTagName !== globalThis.Document.prototype.getElementsByTagName) {
            return document.getElementsByTagName(t);
        }
        var target = String(t).toLowerCase();
        var all = (target === '*');
        var res = [];
        function walk(n) {
            if (!n) return;
            var kids = n.childNodes || n._childNodes || [];
            for (var k = 0; k < kids.length; k++) {
                var c = kids[k];
                if (c && c.nodeType === 1) {
                    var tag = (c.localName || c.tagName || c.nodeName || '').toLowerCase();
                    if (tag.indexOf(':') !== -1) tag = tag.split(':')[1];
                    if (all || tag === target) res.push(c);
                    walk(c);
                }
            }
        }
        walk(this.documentElement || this);
        return (typeof _createHTMLCollection === 'function') ? _createHTMLCollection(function() { return res; }) : res;
    };
    globalThis.Document.prototype.getElementsByTagNameNS = function(ns, t) {
        if (this === document && typeof document !== 'undefined' && typeof document.getElementsByTagNameNS === 'function' && document.getElementsByTagNameNS !== globalThis.Document.prototype.getElementsByTagNameNS) {
            return document.getElementsByTagNameNS(ns, t);
        }
        var target = String(t).toLowerCase();
        var targetNs = ns ? String(ns) : null;
        var all = (target === '*');
        var res = [];
        function walk(n) {
            if (!n) return;
            var kids = n.childNodes || n._childNodes || [];
            for (var k = 0; k < kids.length; k++) {
                var c = kids[k];
                if (c && c.nodeType === 1) {
                    var tag = (c.localName || c.tagName || c.nodeName || '').toLowerCase();
                    if (tag.indexOf(':') !== -1) tag = tag.split(':')[1];
                    var cNs = c.namespaceURI || null;
                    if ((all || tag === target) && (!targetNs || targetNs === '*' || cNs === targetNs)) res.push(c);
                    walk(c);
                }
            }
        }
        walk(this.documentElement || this);
        return (typeof _createHTMLCollection === 'function') ? _createHTMLCollection(function() { return res; }) : res;
    };
    globalThis.Document.prototype.getElementsByClassName = function(c) {
        if (this === document && typeof document !== 'undefined' && typeof document.getElementsByClassName === 'function' && document.getElementsByClassName !== globalThis.Document.prototype.getElementsByClassName) {
            return document.getElementsByClassName(c);
        }
        var targetCls = String(c);
        var res = [];
        function walk(n) {
            if (!n) return;
            var kids = n.childNodes || n._childNodes || [];
            for (var k = 0; k < kids.length; k++) {
                var el = kids[k];
                if (el && el.nodeType === 1) {
                    var cls = (el.getAttribute && el.getAttribute('class')) || el.className || '';
                    if (cls.split(/\s+/).indexOf(targetCls) !== -1) res.push(el);
                    walk(el);
                }
            }
        }
        walk(this.documentElement || this);
        return (typeof _createHTMLCollection === 'function') ? _createHTMLCollection(function() { return res; }) : res;
    };
    globalThis.Document.prototype.createEvent        = function(t) { return (typeof document !== 'undefined' && document.createEvent) ? document.createEvent(t) : new globalThis.Event(t); };
    globalThis.Document.prototype.importNode         = function(n, d) { return n && n.cloneNode ? n.cloneNode(d) : n; };
    globalThis.Document.prototype.adoptNode          = function(n) { return (typeof _adoptNodeInternal === 'function') ? _adoptNodeInternal(this, n) : n; };
    globalThis.Document.prototype.elementFromPoint   = function(x, y) { return (typeof document !== 'undefined' && document.elementFromPoint) ? document.elementFromPoint(x, y) : null; };
    globalThis.Document.prototype.elementsFromPoint  = function(x, y) { return (typeof document !== 'undefined' && document.elementsFromPoint) ? document.elementsFromPoint(x, y) : []; };
    globalThis.Document.prototype.caretPositionFromPoint = function() { return null; };
    globalThis.Document.prototype.createRange        = function() { return (typeof document !== 'undefined' && document.createRange) ? document.createRange() : new globalThis.Range(); };
    globalThis.Document.prototype.getSelection      = function() { return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null; };
    globalThis.Document.prototype.createNodeIterator = function(root, whatToShow, filter) {
        return (typeof document !== 'undefined' && document.createNodeIterator) ? document.createNodeIterator(root, whatToShow, filter) : null;
    };
    globalThis.Document.prototype.createTreeWalker   = function(root, whatToShow, filter) {
        return (typeof document !== 'undefined' && document.createTreeWalker) ? document.createTreeWalker(root, whatToShow, filter) : null;
    };
    globalThis.Document.prototype.write             = function() {
        if (this._opened) this._writeBuffer = (this._writeBuffer || '') + Array.prototype.join.call(arguments, '');
        else if (typeof globalThis._documentWrite === 'function') globalThis._documentWrite(this, arguments, false);
    };
    globalThis.Document.prototype.writeln           = function() {
        if (this._opened) this._writeBuffer = (this._writeBuffer || '') + Array.prototype.join.call(arguments, '') + '\n';
        else if (typeof globalThis._documentWrite === 'function') globalThis._documentWrite(this, arguments, true);
    };
    globalThis.Document.prototype.open              = function() { this._opened = true; this._writeBuffer = ''; return this; };
    globalThis.Document.prototype.close             = function() {
        if (this._opened) {
            this._opened = false;
            var text = this._writeBuffer || '';
            this._writeBuffer = '';
            if (typeof _resetDocumentWithHtml === 'function') _resetDocumentWithHtml(this, text);
        }
        return this;
    };
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
            get: function() {
                if (this === document && typeof document !== 'undefined') return document[p];
                if (this['_' + p] !== undefined) return this['_' + p];
                if (p === 'head' || p === 'body') {
                    var de = this.documentElement;
                    if (de) {
                        var kids = de.childNodes || de._childNodes || [];
                        for (var i = 0; i < kids.length; i++) {
                            var k = kids[i];
                            if (k && k.nodeType === 1) {
                                var tag = (k.tagName || k.nodeName || '').toLowerCase();
                                if (p === 'head' && tag === 'head') return k;
                                if (p === 'body' && (tag === 'body' || tag === 'frameset')) return k;
                            }
                        }
                    }
                    return null;
                }
                if (p === 'forms') return this.getElementsByTagName('form');
                if (p === 'images') return this.getElementsByTagName('img');
                if (p === 'scripts') return this.getElementsByTagName('script');
                if (p === 'links') {
                    var allA = this.getElementsByTagName('a');
                    var allArea = this.getElementsByTagName('area');
                    var res = [];
                    for (var a = 0; a < allA.length; a++) if (allA[a].hasAttribute && allA[a].hasAttribute('href')) res.push(allA[a]);
                    for (var ar = 0; ar < allArea.length; ar++) if (allArea[ar].hasAttribute && allArea[ar].hasAttribute('href')) res.push(allArea[ar]);
                    return (typeof _createHTMLCollection === 'function') ? _createHTMLCollection(function() { return res; }) : res;
                }
                return null;
            },
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
            if (typeof globalThis._scrollTo === 'function') {
                try { globalThis._scrollTo(newX, newY, false); } catch(e) {}
            }
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
        if (typeof globalThis._scrollTo === 'function') {
            try { globalThis._scrollTo(targetX, targetY, true); } catch(e) {}
        }
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
