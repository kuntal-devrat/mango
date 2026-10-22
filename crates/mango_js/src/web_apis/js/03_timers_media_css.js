    // ── 8. Window EventTarget ─────────────────────────────────────────────────
    globalThis.addEventListener = function(type, fn, opts) {
        if (typeof globalThis._addDOMEventListener === 'function') {
            globalThis._addDOMEventListener(globalThis, type, fn, opts);
        }
    };
    globalThis.removeEventListener = function(type, fn, opts) {
        if (typeof globalThis._removeDOMEventListener === 'function') {
            globalThis._removeDOMEventListener(globalThis, type, fn, opts);
        }
    };
    globalThis.dispatchEvent = function(evt) {
        if (typeof globalThis._dispatchDOMEvent === 'function') {
            return globalThis._dispatchDOMEvent(globalThis, evt);
        }
        return true;
    };

    // ── 9. Timers with closure preservation ──────────────────────────────────
    var _timerCallbacks = {};
    var _timerNextId = 1;
    globalThis._mangoRunTimer = function(id) {
        var item = _timerCallbacks[id];
        if (!item) return;
        try {
            if (typeof item.fn === 'function') item.fn.apply(globalThis, item.args || []);
            else if (typeof item.fn === 'string') (0, eval)(item.fn);
        } catch(e) {
            if (typeof console !== 'undefined' && console.warn) {
                console.warn("[_mangoRunTimer error]:", (e && e.stack) ? e.stack : e);
            }
        }
        if (!item.repeat) delete _timerCallbacks[id];
    };
    globalThis.setTimeout = function(fn, delay) {
        var args = Array.prototype.slice.call(arguments, 2);
        var id = _timerNextId++;
        _timerCallbacks[id] = { fn: fn, repeat: false, args: args };
        if (typeof _mangoScheduleTimer === 'function') _mangoScheduleTimer(id, Number(delay) || 0, false);
        return id;
    };
    globalThis.setInterval = function(fn, delay) {
        var args = Array.prototype.slice.call(arguments, 2);
        var id = _timerNextId++;
        _timerCallbacks[id] = { fn: fn, repeat: true, args: args };
        if (typeof _mangoScheduleTimer === 'function') _mangoScheduleTimer(id, Number(delay) || 0, true);
        return id;
    };
    globalThis.clearTimeout = function(id) {
        var numId = Number(id) || 0;
        delete _timerCallbacks[numId];
        if (typeof _mangoCancelTimer === 'function') _mangoCancelTimer(numId);
    };
    globalThis.clearInterval = globalThis.clearTimeout;

    // ── 10. requestAnimationFrame ─────────────────────────────────────────────
    globalThis.requestAnimationFrame = function(cb) {
        return globalThis.setTimeout(function() {
            try { if (typeof cb === 'function') cb(performance.now()); } catch(e) {}
        }, 16);
    };
    globalThis.cancelAnimationFrame = globalThis.clearTimeout;
    globalThis.requestIdleCallback = function(cb, opts) {
        return globalThis.setTimeout(function() {
            var start = Date.now();
            try {
                if (typeof cb === 'function') {
                    cb({
                        didTimeout: false,
                        timeRemaining: function() { return Math.max(0, 50 - (Date.now() - start)); }
                    });
                }
            } catch(e) {}
        }, 1);
    };
    globalThis.cancelIdleCallback = globalThis.clearTimeout;

    // ── 11. matchMedia & MediaQueryList ───────────────────────────────────────
    function MediaQueryList(query, matches) {
        this.media = query;
        this.matches = !!matches;
        this.onchange = null;
        this._listeners = [];
    }
    MediaQueryList.prototype = Object.create((typeof EventTarget !== 'undefined' ? EventTarget.prototype : Object.prototype));
    MediaQueryList.prototype.constructor = MediaQueryList;
    MediaQueryList.prototype.addListener = function(fn) { this.addEventListener('change', fn); };
    MediaQueryList.prototype.removeListener = function(fn) { this.removeEventListener('change', fn); };
    MediaQueryList.prototype.addEventListener = function(type, fn) {
        if (typeof fn === 'function') {
            this._listeners.push({ type: type, fn: fn });
        }
    };
    MediaQueryList.prototype.removeEventListener = function(type, fn) {
        this._listeners = this._listeners.filter(function(l) { return !(l.type === type && l.fn === fn); });
    };
    MediaQueryList.prototype.dispatchEvent = function(evt) {
        var t = (evt && evt.type) || 'change';
        if (t === 'change' && typeof this.onchange === 'function') {
            try { this.onchange.call(this, evt); } catch(e) {}
        }
        for (var i = 0; i < this._listeners.length; i++) {
            if (this._listeners[i].type === t) {
                try { this._listeners[i].fn.call(this, evt); } catch(e) {}
            }
        }
        return true;
    };
    globalThis.MediaQueryList = MediaQueryList;
    if (globalThis.window) globalThis.window.MediaQueryList = MediaQueryList;

    globalThis.matchMedia = function(q) {
        var qs = String(q || "");
        // Honour prefers-color-scheme: dark — reported as false (light mode).
        // Honour hover and pointer features for desktop.
        var matchMap = {
            "prefers-color-scheme: light": true,
            "prefers-color-scheme: dark":  false,
            "hover: hover":                true,
            "pointer: fine":               true
        };
        var matched = false;
        for (var k in matchMap) {
            if (qs.indexOf(k) !== -1) { matched = matchMap[k]; break; }
        }
        var minW = qs.match(/min-width:\s*(\d+)px/);
        var maxW = qs.match(/max-width:\s*(\d+)px/);
        if (minW || maxW) {
            var curW = (typeof window !== 'undefined' && window.innerWidth) || globalThis.innerWidth || 800;
            var ok = true;
            if (minW && curW < parseInt(minW[1], 10)) { ok = false; }
            if (maxW && curW > parseInt(maxW[1], 10)) { ok = false; }
            matched = ok;
        }
        return new MediaQueryList(qs, matched);
    };

    // ── 12. getComputedStyle & CSSStyleDeclaration ────────────────────────────
    function CSSStyleDeclaration(el) {
        this._el = el || null;
    }
    var WEBKIT_ALIASES = {
        '-webkit-align-content': 'align-content',
        '-webkit-align-items': 'align-items',
        '-webkit-align-self': 'align-self',
        '-webkit-flex': 'flex',
        '-webkit-flex-basis': 'flex-basis',
        '-webkit-flex-direction': 'flex-direction',
        '-webkit-flex-flow': 'flex-flow',
        '-webkit-flex-grow': 'flex-grow',
        '-webkit-flex-shrink': 'flex-shrink',
        '-webkit-flex-wrap': 'flex-wrap',
        '-webkit-justify-content': 'justify-content',
        '-webkit-order': 'order'
    };

    CSSStyleDeclaration.prototype.getPropertyValue = function(prop) {
        var p = String(prop || '').trim().replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
        if (!p) return "";
        p = WEBKIT_ALIASES[p] || p;
        var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
        if (this[camel] !== undefined) return String(this[camel]);
        if (this[p] !== undefined) return String(this[p]);
        if (this._props && this._props[p] !== undefined) return String(this._props[p]);
        if (this._props && this._props[camel] !== undefined) return String(this._props[camel]);
        return "";
    };
    CSSStyleDeclaration.prototype.getPropertyPriority = function(prop) {
        return "";
    };
    CSSStyleDeclaration.prototype.setProperty = function(prop, val, prio) {
        if (!this._props) this._props = {};
        var p = String(prop || '').trim().toLowerCase();
        p = WEBKIT_ALIASES[p] || p;
        this._props[p] = String(val);
    };
    CSSStyleDeclaration.prototype.removeProperty = function(prop) {
        if (!this._props) this._props = {};
        var p = String(prop || '').trim().toLowerCase();
        p = WEBKIT_ALIASES[p] || p;
        var prev = this._props[p];
        delete this._props[p];
        return prev || "";
    };
    CSSStyleDeclaration.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : "";
    };
    CSSStyleDeclaration.prototype[Symbol.iterator] = function() {
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
    CSSStyleDeclaration.prototype[Symbol.toStringTag] = 'CSSStyleDeclaration';
    globalThis.CSSStyleDeclaration = CSSStyleDeclaration;

    globalThis.getComputedStyle = function(el, pseudo) {
        var cs = new CSSStyleDeclaration(el);
        var s = (el && el.style) ? el.style : {};
        var backendProps = (el && el._nodeId !== undefined && typeof _getComputedStyle === 'function')
            ? _getComputedStyle(el._nodeId)
            : null;

        var rect = (el && typeof el.getBoundingClientRect === 'function') ? el.getBoundingClientRect() : null;
        var widthStr = (rect && rect.width > 0) ? (rect.width + 'px') : ((backendProps && backendProps.width) || s.width || '0px');
        var heightStr = (rect && rect.height > 0) ? (rect.height + 'px') : ((backendProps && backendProps.height) || s.height || '0px');

        var tag = (el && el.tagName) ? el.tagName.toUpperCase() : '';
        var defaultDisplay = 'block';
        if (tag === 'SPAN' || tag === 'A' || tag === 'B' || tag === 'I' || tag === 'EM' || tag === 'STRONG' || tag === 'LABEL') {
            defaultDisplay = 'inline';
        } else if (tag === 'BUTTON' || tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA' || tag === 'IMG') {
            defaultDisplay = 'inline-block';
        } else if (tag === 'TABLE') {
            defaultDisplay = 'table';
        } else if (tag === 'TR') {
            defaultDisplay = 'table-row';
        } else if (tag === 'TD' || tag === 'TH') {
            defaultDisplay = 'table-cell';
        } else if (tag === 'LI') {
            defaultDisplay = 'list-item';
        }

        var props = {
            display:          (backendProps && backendProps.display) || s.display          || defaultDisplay,
            visibility:       (backendProps && backendProps.visibility) || s.visibility       || 'visible',
            opacity:          (backendProps && backendProps.opacity) || s.opacity          || '1',
            width:            widthStr,
            height:           heightStr,
            color:            (backendProps && backendProps.color) || s.color            || 'rgb(0, 0, 0)',
            backgroundColor:  (backendProps && (backendProps['background-color'] || backendProps.backgroundColor)) || s.backgroundColor  || 'rgba(0, 0, 0, 0)',
            fontSize:         (backendProps && (backendProps['font-size'] || backendProps.fontSize)) || s.fontSize         || '16px',
            fontFamily:       (backendProps && (backendProps['font-family'] || backendProps.fontFamily)) || s.fontFamily       || 'sans-serif',
            fontWeight:       (backendProps && (backendProps['font-weight'] || backendProps.fontWeight)) || s.fontWeight       || 'normal',
            lineHeight:       (backendProps && (backendProps['line-height'] || backendProps.lineHeight)) || s.lineHeight       || 'normal',
            position:         (backendProps && backendProps.position) || s.position         || 'static',
            top:              s.top              || 'auto',
            left:             s.left             || 'auto',
            right:            s.right            || 'auto',
            bottom:           s.bottom           || 'auto',
            margin:           s.margin           || '0px',
            marginTop:        s.marginTop        || s.margin || '0px',
            marginRight:      s.marginRight      || s.margin || '0px',
            marginBottom:     s.marginBottom     || s.margin || '0px',
            marginLeft:       s.marginLeft       || s.margin || '0px',
            padding:          s.padding          || '0px',
            paddingTop:       s.paddingTop       || s.padding || '0px',
            paddingRight:     s.paddingRight     || s.padding || '0px',
            paddingBottom:    s.paddingBottom    || s.padding || '0px',
            paddingLeft:      s.paddingLeft      || s.padding || '0px',
            border:           s.border           || '0px none rgb(0, 0, 0)',
            borderWidth:      s.borderWidth      || '0px',
            borderTopWidth:   s.borderTopWidth   || s.borderWidth || '0px',
            borderRightWidth: s.borderRightWidth || s.borderWidth || '0px',
            borderBottomWidth:s.borderBottomWidth|| s.borderWidth || '0px',
            borderLeftWidth:  s.borderLeftWidth  || s.borderWidth || '0px',
            borderStyle:      s.borderStyle      || 'none',
            borderColor:      s.borderColor      || 'rgb(0, 0, 0)',
            boxSizing:        (backendProps && (backendProps['box-sizing'] || backendProps.boxSizing)) || s.boxSizing        || 'content-box',
            transform:        s.transform        || 'none',
            transition:       s.transition       || 'none',
            overflow:         s.overflow         || 'visible',
            overflowX:        s.overflowX        || s.overflow || 'visible',
            overflowY:        s.overflowY        || s.overflow || 'visible',
            zIndex:           s.zIndex           || 'auto',
            flexDirection:    s.flexDirection    || 'row',
            alignItems:       s.alignItems       || 'stretch',
            justifyContent:   s.justifyContent   || 'flex-start',
            cursor:           s.cursor           || 'auto'
        };

        if (backendProps) {
            for (var bp in backendProps) {
                if (Object.prototype.hasOwnProperty.call(backendProps, bp)) {
                    var val = backendProps[bp];
                    props[bp] = val;
                    var camel = bp.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                    props[camel] = val;
                }
            }
        }

        // CSSOM § 2.2: `width` / `height` must expose the *used* value, so the
        // size derived from layout wins over the backend's specified value.
        props.width = widthStr;
        props.height = heightStr;

        var propNames = Object.keys(props);
        cs.length = propNames.length;
        for (var i = 0; i < propNames.length; i++) {
            var k = propNames[i];
            var v = props[k];
            cs[k] = v;
            var kebab = k.replace(/([A-Z])/g, '-$1').toLowerCase();
            cs[kebab] = v;
            cs[i] = kebab;
        }
        return cs;
    };
    if (typeof window !== 'undefined') {
        window.getComputedStyle = globalThis.getComputedStyle;
    }

    // ── 12.1 CSS Namespace (supports & escape) ────────────────────────────────
    if (typeof globalThis.CSS === 'undefined') {
        globalThis.CSS = {};
    }
    if (!globalThis.CSS.supports) {
        globalThis.CSS.supports = function(propOrCondition, value) {
            if (typeof _cssSupports === 'function') {
                if (arguments.length >= 2) {
                    return _cssSupports(propOrCondition, value);
                }
                if (arguments.length === 1) {
                    return _cssSupports(propOrCondition, null);
                }
            }
            return false;
        };
    }
    if (!globalThis.CSS.escape) {
        globalThis.CSS.escape = function(ident) {
            return String(ident).replace(/([\x00-\x1f\x7f]|^-?\d|^-[-\d]|[^a-zA-Z0-9_\-\u0080-\uFFFF])/g, function(ch) {
                return '\\' + ch;
            });
        };
    }
    if (typeof window !== 'undefined') {
        window.CSS = globalThis.CSS;
    }

    // ── 13. URL & URLSearchParams ─────────────────────────────────────────────
    function URLSearchParams(init) {
        this._entries = [];
        var self = this;
        if (init) {
            if (typeof init === 'string') {
                var s = init.replace(/^\?/, '');
                if (s.length > 0) {
                    var pairs = s.split('&');
                    for (var i = 0; i < pairs.length; i++) {
                        var pair = pairs[i];
                        var idx = pair.indexOf('=');
                        if (idx !== -1) {
                            var k = decodeURIComponent(pair.substring(0, idx).replace(/\+/g, ' '));
                            var v = decodeURIComponent(pair.substring(idx + 1).replace(/\+/g, ' '));
                            this._entries.push([k, v]);
                        } else if (pair.length > 0) {
                            var k = decodeURIComponent(pair.replace(/\+/g, ' '));
                            this._entries.push([k, '']);
                        }
                    }
                }
            } else if (init instanceof URLSearchParams || (init._entries && Array.isArray(init._entries))) {
                init.forEach(function(v, k) { self._entries.push([String(k), String(v)]); });
            } else if (Array.isArray(init)) {
                for (var i = 0; i < init.length; i++) {
                    var item = init[i];
                    if (item && item.length >= 2) {
                        this._entries.push([String(item[0]), String(item[1])]);
                    }
                }
            } else if (typeof init === 'object') {
                var keys = Object.keys(init);
                for (var i = 0; i < keys.length; i++) {
                    var k = keys[i];
                    this._entries.push([String(k), String(init[k])]);
                }
            }
        }
    }
    URLSearchParams.prototype = {
        append: function(name, value) {
            this._entries.push([String(name), String(value)]);
            if (this._url) this._url._syncFromSearchParams();
        },
        delete: function(name, value) {
            var strName = String(name);
            if (value !== undefined) {
                var strVal = String(value);
                this._entries = this._entries.filter(function(e) { return !(e[0] === strName && e[1] === strVal); });
            } else {
                this._entries = this._entries.filter(function(e) { return e[0] !== strName; });
            }
            if (this._url) this._url._syncFromSearchParams();
        },
        get: function(name) {
            var strName = String(name);
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strName) return this._entries[i][1];
            }
            return null;
        },
        getAll: function(name) {
            var strName = String(name);
            var res = [];
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strName) res.push(this._entries[i][1]);
            }
            return res;
        },
        has: function(name, value) {
            var strName = String(name);
            if (value !== undefined) {
                var strVal = String(value);
                for (var i = 0; i < this._entries.length; i++) {
                    if (this._entries[i][0] === strName && this._entries[i][1] === strVal) return true;
                }
                return false;
            }
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strName) return true;
            }
            return false;
        },
        set: function(name, value) {
            var strName = String(name);
            var strVal = String(value);
            var found = false;
            var newEntries = [];
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strName) {
                    if (!found) {
                        newEntries.push([strName, strVal]);
                        found = true;
                    }
                } else {
                    newEntries.push(this._entries[i]);
                }
            }
            if (!found) newEntries.push([strName, strVal]);
            this._entries = newEntries;
            if (this._url) this._url._syncFromSearchParams();
        },
        sort: function() {
            this._entries.sort(function(a, b) {
                if (a[0] < b[0]) return -1;
                if (a[0] > b[0]) return 1;
                return 0;
            });
            if (this._url) this._url._syncFromSearchParams();
        },
        forEach: function(cb, thisArg) {
            for (var i = 0; i < this._entries.length; i++) {
                var e = this._entries[i];
                cb.call(thisArg, e[1], e[0], this);
            }
        },
        keys: function() {
            var i = 0, self = this;
            var it = {
                next: function() {
                    if (i < self._entries.length) return { value: self._entries[i++][0], done: false };
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        },
        values: function() {
            var i = 0, self = this;
            var it = {
                next: function() {
                    if (i < self._entries.length) return { value: self._entries[i++][1], done: false };
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        },
        entries: function() {
            var i = 0, self = this;
            var it = {
                next: function() {
                    if (i < self._entries.length) {
                        var e = self._entries[i++];
                        return { value: [e[0], e[1]], done: false };
                    }
                    return { value: undefined, done: true };
                }
            };
            it[Symbol.iterator] = function() { return this; };
            return it;
        },
        toString: function() {
            var parts = [];
            for (var i = 0; i < this._entries.length; i++) {
                var e = this._entries[i];
                parts.push(encodeURIComponent(e[0]).replace(/%20/g, '+') + '=' + encodeURIComponent(e[1]).replace(/%20/g, '+'));
            }
            return parts.join('&');
        }
    };
    URLSearchParams.prototype[Symbol.iterator] = URLSearchParams.prototype.entries;
    Object.defineProperty(URLSearchParams.prototype, 'size', {
        get: function() { return this._entries.length; }
    });
    globalThis.URLSearchParams = URLSearchParams;

    function URL(url, base) {
        if (!url && url !== '') throw new TypeError('Invalid URL: url is required');
        var rawUrl = String(url);
        var full;
        if (base === undefined || base === null || base === '') {
            if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(rawUrl)) {
                base = (typeof document !== 'undefined' && document.baseURI)
                    || (typeof location !== 'undefined' && location.href)
                    || (typeof globalThis !== 'undefined' && globalThis.location && globalThis.location.href);
            }
        }
        if (base !== undefined && base !== null && base !== '') {
            var baseStr = (typeof base === 'object' && base && base.href) ? String(base.href) : String(base);
            if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(rawUrl)) {
                full = rawUrl;
            } else {
                var bSi = baseStr.indexOf('://');
                if (bSi === -1) throw new TypeError('Invalid base URL: ' + baseStr);
                var bScheme = baseStr.substring(0, bSi + 3);
                var bRest = baseStr.substring(bSi + 3);
                var bSlash = bRest.indexOf('/');
                var bHost = bSlash !== -1 ? bRest.substring(0, bSlash) : bRest;
                var bPath = bSlash !== -1 ? bRest.substring(bSlash).split('?')[0].split('#')[0] : '/';
                if (rawUrl.indexOf('//') === 0) {
                    full = baseStr.substring(0, bSi + 1) + rawUrl;
                } else if (rawUrl.charAt(0) === '/') {
                    full = bScheme + bHost + rawUrl;
                } else if (rawUrl.charAt(0) === '?') {
                    full = bScheme + bHost + bPath + rawUrl;
                } else if (rawUrl.charAt(0) === '#') {
                    var bQuery = bRest.indexOf('?') !== -1 ? '?' + bRest.substring(bRest.indexOf('?') + 1).split('#')[0] : '';
                    full = bScheme + bHost + bPath + bQuery + rawUrl;
                } else {
                    var lastSlash = bPath.lastIndexOf('/');
                    var dir = lastSlash !== -1 ? bPath.substring(0, lastSlash + 1) : '/';
                    full = bScheme + bHost + dir + rawUrl;
                }
            }
        } else {
            if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(rawUrl)) {
                throw new TypeError('Invalid URL: ' + rawUrl);
            }
            full = rawUrl;
        }
        
        this._searchParams = null;
        this._parse(full);
    }
    URL.prototype = {
        _parse: function(full) {
            var si = full.indexOf('://');
            if (si === -1) {
                var col = full.indexOf(':');
                if (col !== -1) {
                    this.protocol = full.substring(0, col + 1).toLowerCase();
                    this.pathname = full.substring(col + 1);
                    this.host = '';
                    this.hostname = '';
                    this.port = '';
                    this.search = '';
                    this.hash = '';
                    this.origin = 'null';
                    this.href = full;
                    return;
                }
                throw new TypeError('Invalid URL: ' + full);
            }
            this.protocol = full.substring(0, si + 1).toLowerCase();
            var rest = full.substring(si + 3);
            var slash = rest.indexOf('/');
            var qm = rest.indexOf('?');
            var hash = rest.indexOf('#');
            var firstSep = Math.min(slash === -1 ? 99999 : slash, qm === -1 ? 99999 : qm, hash === -1 ? 99999 : hash);
            var authority = firstSep < 99999 ? rest.substring(0, firstSep) : rest;
            var pathAndRest = firstSep < 99999 ? rest.substring(firstSep) : '/';
            
            this.host = authority;
            var at = authority.indexOf('@');
            var hostPort = at !== -1 ? authority.substring(at + 1) : authority;
            var colon = hostPort.lastIndexOf(':');
            if (colon !== -1 && hostPort.indexOf(']') === -1) {
                this.hostname = hostPort.substring(0, colon);
                this.port = hostPort.substring(colon + 1);
            } else {
                this.hostname = hostPort;
                this.port = '';
            }
            
            var qIdx = pathAndRest.indexOf('?');
            var hIdx = pathAndRest.indexOf('#');
            if (qIdx !== -1) {
                this.pathname = pathAndRest.substring(0, qIdx) || '/';
                if (hIdx !== -1 && hIdx > qIdx) {
                    this.search = pathAndRest.substring(qIdx, hIdx);
                    this.hash = pathAndRest.substring(hIdx);
                } else {
                    this.search = pathAndRest.substring(qIdx);
                    this.hash = '';
                }
            } else if (hIdx !== -1) {
                this.pathname = pathAndRest.substring(0, hIdx) || '/';
                this.search = '';
                this.hash = pathAndRest.substring(hIdx);
            } else {
                this.pathname = pathAndRest || '/';
                this.search = '';
                this.hash = '';
            }
            var segments = this.pathname.split('/');
            var normalized = [];
            for (var si_idx = 0; si_idx < segments.length; si_idx++) {
                var seg = segments[si_idx];
                if (seg === '' && si_idx > 0 && si_idx < segments.length - 1) continue;
                if (seg === '.') continue;
                if (seg === '..') {
                    if (normalized.length > 1) {
                        normalized.pop();
                    }
                } else {
                    normalized.push(seg);
                }
            }
            this.pathname = (normalized.join('/') || '/').replace(/[^\x00-\x7F]/g, encodeURIComponent);
            if (!this.pathname.startsWith('/')) this.pathname = '/' + this.pathname;
            if (this.search) this.search = this.search.replace(/[^\x00-\x7F]/g, encodeURIComponent);
            if (this.hash) this.hash = this.hash.replace(/[^\x00-\x7F]/g, encodeURIComponent);
            this.origin = this.protocol + '//' + this.host;
            this._updateHref();
        },
        _updateHref: function() {
            this.href = this.origin + this.pathname + this.search + this.hash;
        },
        _syncFromSearchParams: function() {
            if (!this._searchParams) return;
            var s = this._searchParams.toString();
            this.search = s.length > 0 ? '?' + s : '';
            this._updateHref();
        },
        get searchParams() {
            if (!this._searchParams) {
                this._searchParams = new URLSearchParams(this.search);
                this._searchParams._url = this;
            }
            return this._searchParams;
        },
        toJSON: function() {
            return this.href;
        },
        toString: function() {
            return this.href;
        }
    };
    URL.canParse = function(url, base) {
        try {
            new URL(url, base);
            return true;
        } catch(e) {
            return false;
        }
    };
    URL.createObjectURL = function(blob) {
        var url = 'blob:mango-' + Math.random().toString(36).substring(2);
        if (blob && typeof blob.text === 'function') {
            try {
                blob.text().then(function(t) {
                    if (typeof _mangoBlobRegister === 'function') _mangoBlobRegister(url, t);
                });
            } catch(e) {}
        } else if (blob && blob._bytes && typeof _mangoBlobRegister === 'function') {
            try {
                var str = (typeof TextDecoder !== 'undefined') ? new TextDecoder().decode(blob._bytes) : String(blob._bytes);
                _mangoBlobRegister(url, str);
            } catch(e) {}
        }
        return url;
    };
    URL.revokeObjectURL = function(url) {};
    globalThis.URL = URL;

