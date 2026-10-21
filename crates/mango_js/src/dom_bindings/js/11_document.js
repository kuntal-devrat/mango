
    var _origGetById = document.getElementById;
    document.getElementById = function(id) { return _wrapElement(_origGetById(id)); };
    // Chrome-internal handle used by Rust-triggered scripts to reach the cached
    // wrapper (and therefore its listeners) for a specific node id.
    try { globalThis._mangoWrap = _wrapElement; globalThis._wrapElement = _wrapElement; } catch (e) {}
    document.querySelector = function(sel) {
        var id = _querySelector(0, String(sel));
        if (id !== null) return _wrapElement(id);
        var s = String(sel);
        if (s === '#engagement-panel-scrim' || s.indexOf('engagement-panel-scrim') !== -1) {
            return {
                nodeType: 1,
                tagName: 'DIV',
                id: 'engagement-panel-scrim',
                addEventListener: function() {},
                removeEventListener: function() {},
                dispatchEvent: function() { return true; },
                style: _createStyleObject(),
                classList: { add: function() {}, remove: function() {}, toggle: function() {}, contains: function() { return false; } },
                setAttribute: function() {},
                getAttribute: function() { return null; },
                removeAttribute: function() {}
            };
        }
        if (s === '#info-skeleton' || s === '#meta-skeleton' || s === '#related-skeleton') {
            return {
                nodeType: 1,
                tagName: 'DIV',
                id: s.substring(1),
                appendChild: function(c) { return c; },
                removeChild: function(c) { return c; },
                insertBefore: function(c) { return c; },
                addEventListener: function() {},
                removeEventListener: function() {},
                style: _createStyleObject()
            };
        }
        return null;
    };
    document.querySelectorAll = function(sel) {
        var ids = _querySelectorAll(0, String(sel)) || [];
        var list = ids.map(_wrapElement);
        return _createNodeList(function() { return list; });
    };
    document.getElementsByTagName = function(tag) {
        return _createHTMLCollection(function() {
            var ids = _querySelectorAll(0, String(tag)) || [];
            return ids.map(_wrapElement);
        });
    };
    document.getElementsByClassName = function(cls) {
        return _createHTMLCollection(function() {
            var ids = _querySelectorAll(0, "." + String(cls)) || [];
            return ids.map(_wrapElement);
        });
    };
    var _origCE = document.createElement;
    try { globalThis._origCE = _origCE; } catch (e) {}
    document.createElement = function(tag, options) {
        var el = _wrapElement(_origCE(tag));
        if (el) {
            var tagLower = String(tag).toLowerCase();
            var isVal = (options && typeof options === 'object' && options.is) ? String(options.is).toLowerCase() : null;
            var customCtor = null;
            if (isVal && globalThis.customElements && globalThis.customElements.get) {
                customCtor = globalThis.customElements.get(isVal);
                if (customCtor) {
                    try { el.setAttribute('is', isVal); } catch(e) {}
                }
            }
            if (!customCtor && globalThis.customElements && globalThis.customElements.get) {
                customCtor = globalThis.customElements.get(tagLower);
            }
            if (customCtor && typeof customCtor === 'function' && !el._isCustomConstructed) {
                if (globalThis._upgradeCustomElement) {
                    globalThis._upgradeCustomElement(el, customCtor);
                }
            }
        }
        return el;
    };
    document.createElementNS = function(ns, tag, options) {
        var el = document.createElement(tag, options);
        if (el) {
            el.namespaceURI = String(ns || '');
        }
        return el;
    };
    var _origCTN = document.createTextNode;
    document.createTextNode = function(text) {
        var str = String(text === undefined ? '' : text);
        var el = _wrapElement(_origCTN(str));
        if (el) el._nodeValue = str;
        return el;
    };
    var _origCC = (typeof globalThis !== 'undefined' && globalThis._origCC) ? globalThis._origCC : (document.createComment || function(t) { return 0; });
    document.createComment = function(text) {
        var str = String(text === undefined ? '' : text);
        var el = _wrapElement(_origCC(str));
        if (el) el._nodeValue = str;
        return el;
    };
    document.createProcessingInstruction = function(target, data) {
        var targetStr = String(target === undefined ? '' : target);
        var dataStr = String(data === undefined ? '' : data);
        if (!targetStr || /[\s<>]/.test(targetStr)) {
            var err = typeof DOMException !== 'undefined'
                ? new DOMException("The provided name is not a valid name.", "InvalidCharacterError")
                : new Error("InvalidCharacterError");
            throw err;
        }
        if (dataStr.indexOf("?>") !== -1) {
            var err = typeof DOMException !== 'undefined'
                ? new DOMException("The data contains invalid characters.", "InvalidCharacterError")
                : new Error("InvalidCharacterError");
            throw err;
        }
        var piId = _origCC(dataStr);
        var el = _wrapElement(piId);
        if (el) {
            el._isPI = true;
            el._target = targetStr;
            el._nodeValue = dataStr;
            if (globalThis.ProcessingInstruction && globalThis.ProcessingInstruction.prototype) {
                Object.setPrototypeOf(el, globalThis.ProcessingInstruction.prototype);
            }
        }
        return el;
    };

    document.addEventListener = function(type, listener, options) {
        _addDOMEventListener(document, type, listener, options);
        if (type === 'DOMContentLoaded' || type === 'load') {
            setTimeout(function() {
                try {
                    if (typeof listener === 'function') listener.call(document, { type: type, target: document, bubbles: false });
                    else if (listener && typeof listener.handleEvent === 'function') listener.handleEvent({ type: type, target: document, bubbles: false });
                } catch(e) {}
            }, 0);
        }
    };
    document.removeEventListener = function(type, listener, options) {
        _removeDOMEventListener(document, type, listener, options);
    };
    document.dispatchEvent = function(evt) {
        return _dispatchDOMEvent(document, evt);
    };
    // `document.cookie` — a live view over the shared, persisted cookie jar.
    // HttpOnly cookies never appear here; assignments are applied to the jar.
    Object.defineProperty(document, 'cookie', {
        get: function() { return _getDocumentCookie(); },
        set: function(v) { _setDocumentCookie(String(v)); },
        configurable: true
    });
    Object.defineProperty(document, 'compatMode', {
        get: function() { return _getCompatMode(); }
    });
    Object.defineProperty(document, 'characterSet', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'charset', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'inputEncoding', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'contentType', {
        get: function() { return _getContentType(); },
        configurable: true
    });
    document.readyState = "complete";
    Object.defineProperty(document, 'visibilityState', {
        get: function() { return 'visible'; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(document, 'hidden', {
        get: function() { return false; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(document, 'prerendering', {
        get: function() { return false; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(document, 'timeline', {
        get: function() {
            return {
                get currentTime() {
                    return (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
                }
            };
        },
        configurable: true,
        enumerable: true
    });
    try {
        document.location = globalThis.location;
    } catch(e) {}
    document.createDocumentFragment = function() {
        return _wrapElement(_createFragment());
    };
    if (!document._adoptedStyleSheets) document._adoptedStyleSheets = [];
    Object.defineProperty(document, 'adoptedStyleSheets', {
        get: function() { return this._adoptedStyleSheets || []; },
        set: function(sheets) {
            if (!Array.isArray(sheets)) throw new TypeError("adoptedStyleSheets must be an Array");
            this._adoptedStyleSheets = sheets.slice();
        },
        configurable: true,
        enumerable: true
    });
    if (!document.startViewTransition) {
        document.startViewTransition = function(callback) {
            var updateCallbackDone = Promise.resolve().then(function() {
                if (typeof callback === 'function') return callback();
            });
            var ready = Promise.resolve();
            var finished = updateCallbackDone;
            return {
                ready: ready,
                finished: finished,
                updateCallbackDone: updateCallbackDone,
                skipTransition: function() {}
            };
        };
    }

