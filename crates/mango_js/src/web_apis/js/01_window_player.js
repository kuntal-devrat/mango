(function() {
    // ── 1. Window / global aliases ────────────────────────────────────────────
    globalThis.window = globalThis;
    globalThis.self   = globalThis;
    globalThis.top    = globalThis;
    globalThis.parent = globalThis;
    globalThis.global = globalThis;
    var _framesProxy = new Proxy(globalThis, {
        get: function(target, prop, receiver) {
            if (typeof prop === 'string' && /^\d+$/.test(prop)) {
                var idx = parseInt(prop, 10);
                if (typeof document !== 'undefined' && document.getElementsByTagName) {
                    var iframes = document.getElementsByTagName('iframe');
                    var frameEl = iframes ? (iframes[idx] || (iframes.item && iframes.item(idx))) : null;
                    if (typeof console !== 'undefined' && console.error) console.error("[DEBUG frames]", "idx:", idx, "iframes len:", iframes ? iframes.length : "null", "frameEl:", frameEl ? frameEl.tagName : "null", "cw:", frameEl ? !!frameEl.contentWindow : "null");
                    if (frameEl && frameEl.contentWindow) return frameEl.contentWindow;
                }
            }
            return Reflect.get(target, prop, receiver);
        }
    });
    globalThis.frames = _framesProxy;
    globalThis._DumpException = function(err) {
        if (typeof console !== 'undefined' && console.error) {
            console.error("[_DumpException]", (err && err.stack) ? err.stack : err);
        }
    };
    if (!globalThis.process) {
        globalThis.process = {
            env: { NODE_ENV: 'production' },
            browser: true,
            version: 'v18.0.0',
            versions: { node: '18.0.0', v8: '10.0.0' },
            nextTick: (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) { setTimeout(cb, 0); }
        };
    }

    // ── 1.05 DOMException (WHATWG standard) ──────────────────────────────────
    if (typeof globalThis.DOMException === 'undefined') {
        var _DOM_EXCEPTION_CODES = {
            IndexSizeError: 1,
            DOMStringSizeError: 2,
            HierarchyRequestError: 3,
            WrongDocumentError: 4,
            InvalidCharacterError: 5,
            NoDataAllowedError: 6,
            NoModificationAllowedError: 7,
            NotFoundError: 8,
            NotSupportedError: 9,
            InUseAttributeError: 10,
            InvalidStateError: 11,
            SyntaxError: 12,
            InvalidModificationError: 13,
            NamespaceError: 14,
            InvalidAccessError: 15,
            ValidationError: 16,
            TypeMismatchError: 17,
            SecurityError: 18,
            NetworkError: 19,
            AbortError: 20,
            URLMismatchError: 21,
            QuotaExceededError: 22,
            TimeoutError: 23,
            InvalidNodeTypeError: 24,
            DataCloneError: 25
        };

        var DOMException = function DOMException(message, name) {
            var err = new Error(message || '');
            this.message = message ? String(message) : '';
            this.name = name ? String(name) : 'Error';
            this.code = _DOM_EXCEPTION_CODES[this.name] || 0;
            this.stack = err.stack;
        };
        DOMException.prototype = Object.create(Error.prototype);
        DOMException.prototype.constructor = DOMException;
        DOMException.prototype.name = 'DOMException';

        var _CONSTANTS = {
            INDEX_SIZE_ERR: 1,
            DOMSTRING_SIZE_ERR: 2,
            HIERARCHY_REQUEST_ERR: 3,
            WRONG_DOCUMENT_ERR: 4,
            INVALID_CHARACTER_ERR: 5,
            NO_DATA_ALLOWED_ERR: 6,
            NO_MODIFICATION_ALLOWED_ERR: 7,
            NOT_FOUND_ERR: 8,
            NOT_SUPPORTED_ERR: 9,
            INUSE_ATTRIBUTE_ERR: 10,
            INVALID_STATE_ERR: 11,
            SYNTAX_ERR: 12,
            INVALID_MODIFICATION_ERR: 13,
            NAMESPACE_ERR: 14,
            INVALID_ACCESS_ERR: 15,
            VALIDATION_ERR: 16,
            TYPE_MISMATCH_ERR: 17,
            SECURITY_ERR: 18,
            NETWORK_ERR: 19,
            ABORT_ERR: 20,
            URL_MISMATCH_ERR: 21,
            QUOTA_EXCEEDED_ERR: 22,
            TIMEOUT_ERR: 23,
            INVALID_NODE_TYPE_ERR: 24,
            DATA_CLONE_ERR: 25
        };
        for (var k in _CONSTANTS) {
            DOMException[k] = _CONSTANTS[k];
            DOMException.prototype[k] = _CONSTANTS[k];
        }

        globalThis.DOMException = DOMException;
        if (globalThis.window) globalThis.window.DOMException = DOMException;
    }

    // ── 1.1 YouTube / Web Component Player Fallback Defense ──────────────────
    var _createDummyPlayer = function() {
        var dp = {
            addEventListener: function() {},
            removeEventListener: function() {},
            getPlayerPromise: function() { return Promise.resolve(dp); },
            getMainAppPlayerPromise: function() { return Promise.resolve(dp); },
            getPlayer: function() { return dp; },
            isReady: function() { return false; },
            getPlaybackRate: function() { return 1; },
            setPlaybackRate: function() {},
            getCurrentTime: function() { return 0; },
            getDuration: function() { return 0; },
            getPlayerState: function() { return -1; },
            getProgressState: function() { return { isAtLiveHead: false }; },
            getVideoData: function() { return {}; },
            setSizeStyle: function() {},
            setSize: function() {},
            setFauxFullscreen: function() {},
            updateFullerscreenEduButtonVisibility: function() {},
            updateFullerscreenEduButtonSubtleModeState: function() {},
            insertTimelyActionUi: function() {},
            pauseVideo: function() {},
            stopVideo: function() {},
            focusOnPlayer: function() {},
            syncVolume: function() {},
            updateShortsPlayerLoop: function() {},
            movePlayerToVideo: function() {},
            scrollToVideoThrottle: { fire: function() {} },
            onPlayerStateChange: function() {},
            handleGlobalKeyUp: function() {},
            handleGlobalKeyDown: function() {},
            loadVideoWithPlayerVars: function() {},
            loadVideo: function() {},
            cueVideoByPlayerVars: function() {},
            sendAbandonmentPing: function() {},
            mute: function() {},
            unMute: function() {},
            toggleSubtitles: function() {},
            getOption: function() { return null; },
            seekTo: function() {},
            stop: function() {},
            preparePlayer: function() {},
            play: function() {},
            pause: function() {},
            loadVideoWithPlayerResponse: function() {},
            appendTo: function() {},
            shouldSendVisibilityState: function() { return false; },
            getVisibilityState: function() { return 0; },
            getPresentingPlayerType: function() { return 0; },
            addCueRange: function() {}
        };
        return new Proxy(dp, {
            get: function(target, prop) {
                if (prop in target) return target[prop];
                if (typeof prop === 'string') {
                    if (prop === 'then') return undefined;
                    return function() { return dp; };
                }
                return undefined;
            }
        });
    };
    var _dummyPlayer = _createDummyPlayer();
    var _dummyPlayerPromise = Promise.resolve(_dummyPlayer);
    _dummyPlayerPromise.getPlayerPromise = function() { return _dummyPlayerPromise; };
    _dummyPlayerPromise.getMainAppPlayerPromise = function() { return _dummyPlayerPromise; };
    _dummyPlayerPromise.getPlayer = function() { return _dummyPlayer; };
    globalThis._dummyPlayer = _dummyPlayer;
    globalThis._dummyPlayerPromise = _dummyPlayerPromise;

    var _default_kevlar_base = globalThis.default_kevlar_base || {};
    Object.defineProperty(globalThis, "default_kevlar_base", {
        configurable: true,
        enumerable: true,
        get: function() { return _default_kevlar_base; },
        set: function(val) {
            _default_kevlar_base = val;
            if (val && typeof val === 'object') {
                var _realX8 = val.X8;
                Object.defineProperty(val, "X8", {
                    configurable: true,
                    enumerable: true,
                    get: function() { return _realX8; },
                    set: function(fn) {
                        _realX8 = function(x) {
                            try {
                                var res = fn(x);
                                return res || _dummyPlayer;
                            } catch(e) {
                                return _dummyPlayer;
                            }
                        };
                    }
                });
            }
        }
    });

    // WeakMap guard against non-object keys (e.g. ShadyDOM ed(null))
    (function() {
        if (typeof WeakMap !== 'undefined' && WeakMap.prototype) {
            var origWeakMapSet = WeakMap.prototype.set;
            var origWeakMapGet = WeakMap.prototype.get;
            var origWeakMapHas = WeakMap.prototype.has;

            WeakMap.prototype.set = function(k, v) {
                if (!k || (typeof k !== 'object' && typeof k !== 'function')) {
                    return this;
                }
                return origWeakMapSet.call(this, k, v);
            };
            WeakMap.prototype.get = function(k) {
                if (!k || (typeof k !== 'object' && typeof k !== 'function')) {
                    return undefined;
                }
                return origWeakMapGet.call(this, k);
            };
            WeakMap.prototype.has = function(k) {
                if (!k || (typeof k !== 'object' && typeof k !== 'function')) {
                    return false;
                }
                return origWeakMapHas.call(this, k);
            };
        }
    })();

    var _origDefineProperty = Object.defineProperty;
    var _origDefineProperties = Object.defineProperties;

    function _wrapPlayerGetter(prop, desc) {
        if (!desc) return desc;
        if (prop === 'showEngagementPanelScrimChanged_' && typeof desc.value === 'function') {
            var origMethod = desc.value;
            desc.value = function(x) {
                try {
                    return origMethod.call(this, x);
                } catch(e) {
                    return;
                }
            };
        }
        if (prop === 'style' && desc && typeof desc.set === 'function' && !desc.get) {
            desc.get = function() {
                return this._style || (this._style = (typeof _createStyleObject === 'function') ? _createStyleObject(this._nodeId) : (typeof globalThis._createStyleObject === 'function') ? globalThis._createStyleObject(this._nodeId) : {});
            };
        }
        if (typeof desc.get !== 'function') return desc;
        var origGet = desc.get;
        desc.get = function() {
            try {
                var res = origGet.call(this);
                if ((prop === 'playerApi' || prop === 'mainAppPlayer') && !res) {
                    return _dummyPlayerPromise;
                }
                if ((prop === 'logo' || prop === 'mastheadLogo') && !res) {
                    return { hidden: true, remove: function(){}, style: {} };
                }
                return res;
            } catch (err) {
                if (prop === 'playerApi' || prop === 'mainAppPlayer' ||
                    (err && err.message && (
                        err.message.indexOf('getPlayerPromise') !== -1 ||
                        err.message.indexOf('getMainAppPlayerPromise') !== -1
                    ))) {
                    return _dummyPlayerPromise;
                }
                if (prop === 'logo' || prop === 'mastheadLogo') {
                    return { hidden: true, remove: function(){}, style: {} };
                }
                if (err && err.message && (
                    err.message.indexOf('reading \'__shady_native_') !== -1 ||
                    err.message.indexOf('reading "__shady_native_') !== -1
                )) {
                    if (prop === 'children' || prop === 'childNodes') return [];
                    return null;
                }
                throw err;
            }
        };
        return desc;
    }

    Object.defineProperty = function(obj, prop, descriptor) {
        try {
            _wrapPlayerGetter(prop, descriptor);
        } catch(e) {}
        return _origDefineProperty.call(Object, obj, prop, descriptor);
    };

    Object.defineProperties = function(obj, props) {
        if (props && typeof props === 'object') {
            for (var k in props) {
                if (Object.prototype.hasOwnProperty.call(props, k)) {
                    try {
                        _wrapPlayerGetter(k, props[k]);
                    } catch(e) {}
                }
            }
        }
        return _origDefineProperties.call(Object, obj, props);
    };

    // ── 2. location ───────────────────────────────────────────────────────────
    function _parseUrlComponents(urlStr, baseStr) {
        var str = String(urlStr !== undefined && urlStr !== null ? urlStr : '').trim();
        if (baseStr && str.indexOf('://') === -1 && str.indexOf('about:') !== 0) {
            var b = _parseUrlComponents(baseStr);
            var bOrigin = (b.origin && b.origin !== 'null' && b.origin !== 'about:') ? b.origin : '';
            if (str.indexOf('//') === 0) {
                str = (b.protocol ? b.protocol : 'http:') + '//' + str.substring(2);
            } else if (str.charAt(0) === '/') {
                str = bOrigin + str;
            } else if (str.charAt(0) === '?') {
                str = bOrigin + b.pathname + str;
            } else if (str.charAt(0) === '#') {
                str = bOrigin + b.pathname + b.search + str;
            } else {
                var p = b.pathname;
                var lastSlash = p.lastIndexOf('/');
                var dir = lastSlash !== -1 ? p.substring(0, lastSlash + 1) : '/';
                str = bOrigin + dir + str;
            }
        }

        var res = {
            href: str,
            protocol: 'http:',
            host: '',
            hostname: '',
            port: '',
            pathname: '/',
            search: '',
            hash: '',
            origin: ''
        };

        if (str.indexOf('about:') === 0) {
            res.protocol = 'about:';
            res.origin = 'null';
            var afterAbout = str.substring(6);
            var hIdx = afterAbout.indexOf('#');
            var qIdx = afterAbout.indexOf('?');
            if (hIdx !== -1) {
                res.pathname = afterAbout.substring(0, hIdx);
                res.hash = afterAbout.substring(hIdx);
            } else if (qIdx !== -1) {
                res.pathname = afterAbout.substring(0, qIdx);
                res.search = afterAbout.substring(qIdx);
            } else {
                res.pathname = afterAbout;
            }
            res.href = str;
            return res;
        }

        var schemeSep = str.indexOf('://');
        var rest = str;
        if (schemeSep !== -1) {
            res.protocol = str.substring(0, schemeSep + 1).toLowerCase();
            var afterScheme = str.substring(schemeSep + 3);

            var slashIdx = afterScheme.indexOf('/');
            var qIdx = afterScheme.indexOf('?');
            var hashIdx = afterScheme.indexOf('#');

            var authorityEnd = afterScheme.length;
            if (slashIdx !== -1 && slashIdx < authorityEnd) authorityEnd = slashIdx;
            if (qIdx !== -1 && qIdx < authorityEnd) authorityEnd = qIdx;
            if (hashIdx !== -1 && hashIdx < authorityEnd) authorityEnd = hashIdx;

            var authority = afterScheme.substring(0, authorityEnd);
            rest = afterScheme.substring(authorityEnd);

            res.host = authority;
            var colonIdx = authority.lastIndexOf(':');
            if (colonIdx !== -1 && authority.indexOf(']') === -1) {
                res.hostname = authority.substring(0, colonIdx);
                res.port = authority.substring(colonIdx + 1);
            } else {
                res.hostname = authority;
                res.port = '';
            }
            res.origin = res.protocol + '//' + res.host;
        } else {
            res.origin = 'null';
            res.protocol = '';
            res.host = '';
            res.hostname = '';
            res.port = '';
        }

        var pathEnd = rest.length;
        var rQIdx = rest.indexOf('?');
        var rHIdx = rest.indexOf('#');
        if (rQIdx !== -1 && rQIdx < pathEnd) pathEnd = rQIdx;
        if (rHIdx !== -1 && rHIdx < pathEnd) pathEnd = rHIdx;

        res.pathname = pathEnd > 0 ? rest.substring(0, pathEnd) : '/';
        if (res.pathname.charAt(0) !== '/') res.pathname = '/' + res.pathname;

        if (rQIdx !== -1) {
            if (rHIdx !== -1 && rHIdx > rQIdx) {
                res.search = rest.substring(rQIdx, rHIdx);
                res.hash = rest.substring(rHIdx);
            } else {
                res.search = rest.substring(rQIdx);
                res.hash = '';
            }
        } else if (rHIdx !== -1) {
            res.search = '';
            res.hash = rest.substring(rHIdx);
        } else {
            res.search = '';
            res.hash = '';
        }

        res.href = (res.origin && res.origin !== 'null' ? res.origin : '') + res.pathname + res.search + res.hash;
        return res;
    }

    var _curHref = "__MANGO_PAGE_URL__";
    var _locParsed = _parseUrlComponents(_curHref);
    var _historyStack = [{ url: _curHref, state: null }];
    var _historyIdx   = 0;

    function _updateLocationHref(newHref, triggerNav, isReplace) {
        var parsed = _parseUrlComponents(newHref, _curHref);
        var oldHref = _curHref;
        var oldHash = _locParsed ? _locParsed.hash : '';
        _curHref = parsed.href;
        _locParsed = parsed;
        if (isReplace && _historyStack.length > 0) {
            _historyStack[_historyIdx] = { url: _curHref, state: _historyStack[_historyIdx].state };
        } else if (triggerNav) {
            _historyStack.splice(_historyIdx + 1);
            _historyStack.push({ url: _curHref, state: null });
            _historyIdx = _historyStack.length - 1;
        }
        if (triggerNav && typeof _mangoNavigate === 'function') {
            _mangoNavigate(_curHref);
        }
        if (parsed.hash !== oldHash && !triggerNav) {
            var win = typeof window !== 'undefined' ? window : globalThis;
            if (win && win.dispatchEvent) {
                var evt = (typeof HashChangeEvent === 'function')
                    ? new HashChangeEvent('hashchange', { oldURL: oldHref, newURL: _curHref })
                    : (typeof Event === 'function' ? new Event('hashchange') : { type: 'hashchange', oldURL: oldHref, newURL: _curHref });
                win.dispatchEvent(evt);
            }
        }
    }

    function Location() {}
    Location.prototype = {
        constructor: Location,
        get href() { return _locParsed.href; },
        set href(val) {
            _updateLocationHref(String(val), true, false);
        },
        get protocol() { return _locParsed.protocol; },
        set protocol(val) {
            var p = String(val).toLowerCase();
            if (p.charAt(p.length - 1) !== ':') p += ':';
            var newHref = p + '//' + _locParsed.host + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        },
        get host() { return _locParsed.host; },
        set host(val) {
            var newHref = _locParsed.protocol + '//' + String(val) + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        },
        get hostname() { return _locParsed.hostname; },
        set hostname(val) {
            var portPart = _locParsed.port ? ':' + _locParsed.port : '';
            var newHref = _locParsed.protocol + '//' + String(val) + portPart + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        },
        get port() { return _locParsed.port; },
        set port(val) {
            var p = String(val).trim();
            var portPart = p ? ':' + p : '';
            var newHref = _locParsed.protocol + '//' + _locParsed.hostname + portPart + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        },
        get pathname() { return _locParsed.pathname; },
        set pathname(val) {
            var p = String(val);
            if (p.charAt(0) !== '/') p = '/' + p;
            var prefix = (_locParsed.origin && _locParsed.origin !== 'null') ? _locParsed.origin : '';
            var newHref = prefix + p + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        },
        get search() { return _locParsed.search; },
        set search(val) {
            var s = String(val);
            if (s && s.charAt(0) !== '?') s = '?' + s;
            var prefix = (_locParsed.origin && _locParsed.origin !== 'null') ? _locParsed.origin : '';
            var newHref = prefix + _locParsed.pathname + s + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        },
        get hash() { return _locParsed.hash; },
        set hash(val) {
            var h = String(val);
            if (h && h.charAt(0) !== '#') h = '#' + h;
            var oldHash = _locParsed.hash;
            var prefix = (_locParsed.origin && _locParsed.origin !== 'null') ? _locParsed.origin : '';
            var newHref = prefix + _locParsed.pathname + _locParsed.search + h;
            if (h !== oldHash) {
                _historyStack.push({ url: newHref, state: (globalThis.history ? globalThis.history.state : null) });
                _historyIdx = _historyStack.length - 1;
                _updateLocationHref(newHref, false, false);
                var win = typeof window !== 'undefined' ? window : globalThis;
                if (win && win.dispatchEvent && typeof PopStateEvent === 'function') {
                    win.dispatchEvent(new PopStateEvent('popstate', {
                        state: (globalThis.history ? globalThis.history.state : null)
                    }));
                }
            }
        },
        get origin() { return _locParsed.origin; },
        assign: function(u) {
            _updateLocationHref(String(u), true, false);
        },
        replace: function(u) {
            _updateLocationHref(String(u), true, true);
        },
        reload: function() {
            if (typeof _mangoNavigate === 'function') {
                _mangoNavigate(_curHref);
            }
        },
        toString: function() {
            return this.href;
        }
    };

    var _loc = Object.create(Location.prototype);
    globalThis.Location = Location;
    globalThis.location = _loc;

    // ── 1.4 VisualViewport & CookieStore ───────────────────────────────────────
    function VisualViewport() {
        this.offsetLeft = 0;
        this.offsetTop = 0;
        this.pageLeft = 0;
        this.pageTop = 0;
        this.scale = 1;
        this.onresize = null;
        this.onscroll = null;
    }
    VisualViewport.prototype = Object.create(globalThis.EventTarget ? globalThis.EventTarget.prototype : Object.prototype);
    VisualViewport.prototype.constructor = VisualViewport;
    Object.defineProperty(VisualViewport.prototype, 'width', {
        get: function() { return (typeof window !== 'undefined' ? window.innerWidth : 1280) || 1280; },
        configurable: true
    });
    Object.defineProperty(VisualViewport.prototype, 'height', {
        get: function() { return (typeof window !== 'undefined' ? window.innerHeight : 900) || 900; },
        configurable: true
    });
    globalThis.VisualViewport = VisualViewport;
    globalThis.visualViewport = new VisualViewport();
    if (globalThis.window) globalThis.window.visualViewport = globalThis.visualViewport;

    if (typeof globalThis.cookieStore === 'undefined') {
        var _cookieStore = {
            get: function(name) {
                return new Promise(function(resolve) {
                    var str = (typeof document !== 'undefined' && document.cookie) ? document.cookie : '';
                    var target = (typeof name === 'object' && name !== null) ? name.name : String(name);
                    var cookies = str.split(';');
                    for (var i = 0; i < cookies.length; i++) {
                        var c = cookies[i].trim();
                        var eq = c.indexOf('=');
                        var k = eq !== -1 ? c.slice(0, eq).trim() : c;
                        var v = eq !== -1 ? c.slice(eq + 1).trim() : '';
                        if (k === target) {
                            return resolve({ name: k, value: v });
                        }
                    }
                    resolve(null);
                });
            },
            getAll: function(name) {
                return new Promise(function(resolve) {
                    var str = (typeof document !== 'undefined' && document.cookie) ? document.cookie : '';
                    var target = (typeof name === 'object' && name !== null) ? name.name : (name ? String(name) : null);
                    var list = [];
                    var cookies = str ? str.split(';') : [];
                    for (var i = 0; i < cookies.length; i++) {
                        var c = cookies[i].trim();
                        if (!c) continue;
                        var eq = c.indexOf('=');
                        var k = eq !== -1 ? c.slice(0, eq).trim() : c;
                        var v = eq !== -1 ? c.slice(eq + 1).trim() : '';
                        if (!target || k === target) {
                            list.push({ name: k, value: v });
                        }
                    }
                    resolve(list);
                });
            },
            set: function(name, value) {
                return new Promise(function(resolve) {
                    var k = (typeof name === 'object' && name !== null) ? name.name : String(name);
                    var v = (typeof name === 'object' && name !== null) ? name.value : String(value);
                    if (typeof document !== 'undefined') {
                        document.cookie = k + '=' + v;
                    }
                    resolve();
                });
            },
            delete: function(name) {
                return new Promise(function(resolve) {
                    var k = (typeof name === 'object' && name !== null) ? name.name : String(name);
                    if (typeof document !== 'undefined') {
                        document.cookie = k + '=; expires=Thu, 01 Jan 1970 00:00:00 GMT; Max-Age=0';
                    }
                    resolve();
                });
            },
            addEventListener: function() {},
            removeEventListener: function() {},
            dispatchEvent: function() { return true; }
        };
        globalThis.cookieStore = _cookieStore;
        if (globalThis.window) globalThis.window.cookieStore = _cookieStore;
    }

