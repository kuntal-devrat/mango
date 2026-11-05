
    var _origGetById = document.getElementById;
    try { globalThis._origGetById = _origGetById; } catch (e) {}
    document.getElementById = function(id) {
        if (arguments.length === 0) return null;
        var sid = String(id);
        if (sid.length === 0) return null;
        return _wrapElement(_origGetById(sid));
    };
    // Chrome-internal handle used by Rust-triggered scripts to reach the cached
    // wrapper (and therefore its listeners) for a specific node id.
    try { globalThis._mangoWrap = _wrapElement; globalThis._wrapElement = _wrapElement; } catch (e) {}

    function _syncWindowNamedProperties() {
        if (typeof _querySelectorAll === 'function') {
            try {
                var ids = _querySelectorAll(0, '[id]') || [];
                for (var i = 0; i < ids.length; i++) {
                    var el = _wrapElement(ids[i]);
                    if (!el) continue;
                    var elId = (el.getAttribute && el.getAttribute('id')) || el.id;
                    if (elId && !(elId in globalThis)) {
                        (function(targetId) {
                            try {
                                Object.defineProperty(globalThis, targetId, {
                                    get: function() { return document.getElementById(targetId); },
                                    set: function(v) {
                                        Object.defineProperty(globalThis, targetId, {
                                            value: v,
                                            writable: true,
                                            configurable: true,
                                            enumerable: true
                                        });
                                    },
                                    configurable: true,
                                    enumerable: false
                                });
                            } catch (e) {}
                        })(elId);
                    }
                }
            } catch (e) {}
        }
    }
    globalThis._syncWindowNamedProperties = _syncWindowNamedProperties;
    _syncWindowNamedProperties();
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
        return (typeof _defaultGetElementsByTagName === 'function') ? _defaultGetElementsByTagName(document, tag) : _createHTMLCollection(function() { return []; });
    };
    document.getElementsByTagNameNS = function(ns, tag) {
        return (typeof _defaultGetElementsByTagNameNS === 'function') ? _defaultGetElementsByTagNameNS(document, ns, tag) : _createHTMLCollection(function() { return []; });
    };
    document.getElementsByClassName = function(cls) {
        return _createHTMLCollection(function() {
            var ids = _querySelectorAll(0, "." + String(cls)) || [];
            return ids.map(_wrapElement);
        });
    };
    function _toAsciiLower(s) {
        return String(s).replace(/[A-Z]/g, function(c) { return String.fromCharCode(c.charCodeAt(0) + 32); });
    }
    function _toAsciiUpper(s) {
        return String(s).replace(/[a-z]/g, function(c) { return String.fromCharCode(c.charCodeAt(0) - 32); });
    }
    globalThis._toAsciiLower = _toAsciiLower;
    globalThis._toAsciiUpper = _toAsciiUpper;

    function _validateXmlName(name) {
        if (name === "" || name === null || name === undefined) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("Invalid name", "InvalidCharacterError");
        }
        var str = String(name);
        if (str.length === 0) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("Invalid name", "InvalidCharacterError");
        }
        if (/[\s\x00-\x1F\x7F]/.test(str)) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("Invalid character in name: " + str, "InvalidCharacterError");
        }
        var c0 = str.charAt(0);
        if ((c0 >= '0' && c0 <= '9') || c0 === '-' || c0 === '.' || c0 === '}' || c0 === '<' || str.slice(-1) === '>') {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("Invalid character in name: " + str, "InvalidCharacterError");
        }
    }
    globalThis._validateXmlName = _validateXmlName;

    var _origCE = document.createElement;
    try { globalThis._origCE = _origCE; } catch (e) {}
    document.createElement = function(tag, options) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'createElement' on 'Document': 1 argument required");
        var t = String(tag);
        _validateXmlName(t);
        var el = _wrapElement(_origCE(t));
        if (el) {
            var tagLower = _toAsciiLower(t);
            if (tagLower === 'script') el._dynamicallyInserted = true;
            var tagUpper = _toAsciiUpper(t);
            el._localName = tagLower;
            el.localName = tagLower;
            el._tagName = tagUpper;
            el.tagName = tagUpper;
            el._nodeName = tagUpper;
            el.nodeName = tagUpper;
            el._namespaceURI = "http://www.w3.org/1999/xhtml";
            el.namespaceURI = "http://www.w3.org/1999/xhtml";
            el._prefix = null;
            el.prefix = null;
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
        if (typeof _createElementNS === 'function') {
            return _createElementNS(document, ns, tag);
        }
        var el = document.createElement(tag, options);
        if (el) {
            el.namespaceURI = (ns === "" || ns === undefined) ? null : String(ns);
        }
        return el;
    };
    var _origCTN = document.createTextNode;
    document.createTextNode = function(text) {
        var str = arguments.length === 0 ? '' : String(text);
        var el = _wrapElement(_origCTN(str));
        if (el) el._nodeValue = str;
        return el;
    };
    var _origCC = (typeof globalThis !== 'undefined' && globalThis._origCC) ? globalThis._origCC : (document.createComment || function(t) { return 0; });
    document.createComment = function(text) {
        var str = arguments.length === 0 ? '' : String(text);
        var el = _wrapElement(_origCC(str));
        if (el) el._nodeValue = str;
        return el;
    };
    function _isValidXmlName(name) {
        if (!name || typeof name !== 'string' || name.length === 0) return false;
        function isStart(cp) {
            return (cp === 58) || (cp >= 65 && cp <= 90) || (cp === 95) || (cp >= 97 && cp <= 122)
                || (cp >= 0xC0 && cp <= 0xD6) || (cp >= 0xD8 && cp <= 0xF6) || (cp >= 0xF8 && cp <= 0x2FF)
                || (cp >= 0x370 && cp <= 0x37D) || (cp >= 0x37F && cp <= 0x1FFF) || (cp >= 0x200C && cp <= 0x200D)
                || (cp >= 0x2070 && cp <= 0x218F) || (cp >= 0x2C00 && cp <= 0x2FEF) || (cp >= 0x3001 && cp <= 0xD7FF)
                || (cp >= 0xF900 && cp <= 0xFDCF) || (cp >= 0xFDF0 && cp <= 0xFFFD);
        }
        function isChar(cp) {
            return isStart(cp) || cp === 45 || cp === 46 || (cp >= 48 && cp <= 57)
                || cp === 0xB7 || (cp >= 0x300 && cp <= 0x36F) || (cp >= 0x203F && cp <= 0x2040);
        }
        if (!isStart(name.charCodeAt(0))) return false;
        for (var i = 1; i < name.length; i++) {
            if (!isChar(name.charCodeAt(i))) return false;
        }
        return true;
    }

    document.createProcessingInstruction = function(target, data) {
        var targetStr = String(target === undefined ? '' : target);
        var dataStr = String(data === undefined ? '' : data);
        if (!_isValidXmlName(targetStr)) {
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
            el.target = targetStr;
            el._nodeType = 7;
            el.nodeType = 7;
            var doc = this || document;
            el.ownerDocument = doc;
            el._ownerDocument = doc;
            if (globalThis.ProcessingInstruction && globalThis.ProcessingInstruction.prototype) {
                Object.setPrototypeOf(el, globalThis.ProcessingInstruction.prototype);
            }
            el.data = dataStr;
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
    if (globalThis.Document && globalThis.Document.prototype) {
        Object.defineProperty(globalThis.Document.prototype, 'body', {
            get: function() {
                var de = this.documentElement;
                if (!de) return null;
                var kids = de.childNodes || de._childNodes || [];
                for (var i = 0; i < kids.length; i++) {
                    var k = kids[i];
                    if (k && k.nodeType === 1) {
                        var tag = (k.tagName || k.nodeName || '').toLowerCase();
                        if (tag === 'body' || tag === 'frameset') return k;
                    }
                }
                return null;
            },
            configurable: true,
            enumerable: true
        });
        Object.defineProperty(globalThis.Document.prototype, 'head', {
            get: function() {
                var de = this.documentElement;
                if (!de) return null;
                var kids = de.childNodes || de._childNodes || [];
                for (var i = 0; i < kids.length; i++) {
                    var k = kids[i];
                    if (k && k.nodeType === 1) {
                        var tag = (k.tagName || k.nodeName || '').toLowerCase();
                        if (tag === 'head') return k;
                    }
                }
                return null;
            },
            configurable: true,
            enumerable: true
        });
        Object.defineProperty(globalThis.Document.prototype, 'scrollingElement', {
            get: function() {
                if (this.compatMode === 'BackCompat') return this.body || null;
                return this.documentElement || this.body || null;
            },
            configurable: true,
            enumerable: true
        });
        Object.defineProperty(globalThis.Document.prototype, 'location', {
            get: function() {
                if (this._location !== undefined) return this._location;
                if (typeof document !== 'undefined' && this === document && typeof globalThis.location !== 'undefined') {
                    return globalThis.location;
                }
                var win = this.defaultView;
                if (win && win.location) return win.location;
                return null;
            },
            set: function(v) {
                if (typeof document !== 'undefined' && this === document && typeof globalThis.location !== 'undefined') {
                    globalThis.location.href = String(v);
                } else {
                    this._location = v;
                }
            },
            configurable: true
        });
    }
    try {
        document.location = globalThis.location;
    } catch(e) {}
    if (!document.URL) {
        Object.defineProperty(document, 'URL', {
            get: function() {
                return (this._url !== undefined) ? this._url : ((globalThis.location && globalThis.location.href) ? globalThis.location.href : 'about:blank');
            },
            configurable: true,
            enumerable: true
        });
        Object.defineProperty(document, 'documentURI', {
            get: function() { return this.URL; },
            configurable: true,
            enumerable: true
        });
    }
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

    function _adoptSubtree(node, doc) {
        if (!node || typeof node !== 'object') return;
        node._ownerDocument = doc;
        node.ownerDocument = doc;
        if (node._attrNodes) {
            for (var k in node._attrNodes) {
                var a = node._attrNodes[k];
                if (a) { a._ownerDocument = doc; a.ownerDocument = doc; }
            }
        }
        if (node.attributes) {
            for (var i = 0; i < node.attributes.length; i++) {
                var a = node.attributes[i];
                if (a) { a._ownerDocument = doc; a.ownerDocument = doc; }
            }
        }
        var kids = node.childNodes || node._childNodes;
        if (kids) {
            for (var j = 0; j < kids.length; j++) {
                _adoptSubtree(kids[j], doc);
            }
        }
    }
    globalThis._adoptSubtree = _adoptSubtree;

    function _adoptNodeInternal(doc, node) {
        if (!node || typeof node !== 'object') throw new TypeError("Failed to execute 'adoptNode' on 'Document': parameter 1 is not of type 'Node'.");
        if (node.nodeType === 9) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("Document nodes cannot be adopted.", "NotSupportedError");
        }
        if (node.nodeType === 11 && node.host) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("ShadowRoot cannot be adopted.", "HierarchyRequestError");
        }
        var oldDoc = node.ownerDocument || document;
        var p = node.parentNode;
        if (p) {
            if (p.removeChild) {
                try { p.removeChild(node); } catch(e) {}
            }
            if (node._nodeId !== undefined && p._nodeId !== undefined && typeof _removeChild === 'function') {
                try { _removeChild(p._nodeId, node._nodeId); } catch(e) {}
            }
            node.parentNode = null;
            node._parentNode = null;
        }
        _adoptSubtree(node, doc);
        if (typeof node.adoptedCallback === 'function') {
            try { node.adoptedCallback(oldDoc, doc); } catch(e) {}
        }
        return node;
    }
    globalThis._adoptNodeInternal = _adoptNodeInternal;

    document.adoptNode = function(node) {
        return _adoptNodeInternal(this, node);
    };

    globalThis.__subresourceMap = globalThis.__subresourceMap || {};
    globalThis.__registerSubresource = function(url, text) {
        globalThis.__subresourceMap[url] = String(text);
    };

    // (XML parsing, Frames, and subresource documents are modularized in 11b_frames.js)

    // In-page anchor jumping and element scrollIntoView integration
    if (typeof _scrollIntoView === 'function') {
        globalThis._scrollIntoView = _scrollIntoView;
    }
    if (globalThis.Element && globalThis.Element.prototype) {
        var _domElemScrollIntoView = globalThis.Element.prototype.scrollIntoView;
        globalThis.Element.prototype.scrollIntoView = function(options) {
            if (typeof _domElemScrollIntoView === 'function') {
                _domElemScrollIntoView.call(this, options);
            }
            if (this._nodeId && typeof _scrollIntoView === 'function') {
                var smooth = (typeof options === 'object' && options !== null && options.behavior === 'smooth');
                _scrollIntoView(this._nodeId, smooth);
            }
        };
    }
    if (globalThis.HTMLDetailsElement && !globalThis.HTMLDetailsElement.prototype.hasOwnProperty('open')) {
        Object.defineProperty(globalThis.HTMLDetailsElement.prototype, 'open', {
            get: function() {
                return this.hasAttribute('open');
            },
            set: function(val) {
                if (val) {
                    this.setAttribute('open', '');
                } else {
                    this.removeAttribute('open');
                }
            },
            configurable: true,
            enumerable: true
        });
    }

    if (typeof _wrapElement === 'function') {
        var _baseWrapElementForScroll = _wrapElement;
        function _scrollAwareWrapElement(nodeId) {
            var el = _baseWrapElementForScroll(nodeId);
            if (!el || el._hasScrollHook) return el;
            el._hasScrollHook = true;
            if (el.tagName && el.tagName.toLowerCase() === 'details' && !('open' in el)) {
                Object.defineProperty(el, 'open', {
                    get: function() { return this.hasAttribute('open'); },
                    set: function(val) {
                        if (val) this.setAttribute('open', '');
                        else this.removeAttribute('open');
                    },
                    configurable: true,
                    enumerable: true
                });
            }
            var origScrollIntoView = el.scrollIntoView;
            el.scrollIntoView = function(options) {
                if (typeof origScrollIntoView === 'function') {
                    origScrollIntoView.call(this, options);
                }
                if (this._nodeId && typeof _scrollIntoView === 'function') {
                    var smooth = (typeof options === 'object' && options !== null && options.behavior === 'smooth');
                    _scrollIntoView(this._nodeId, smooth);
                }
            };
            return el;
        }
        _wrapElement = _scrollAwareWrapElement;
        try { globalThis._mangoWrap = _scrollAwareWrapElement; globalThis._wrapElement = _scrollAwareWrapElement; } catch(e) {}
    }

    var _docWriteLastScript = null;
    var _docWriteLastNode = null;

    function _documentWrite(doc, args, addNewline) {
        var str = Array.prototype.join.call(args, '');
        if (addNewline) str += '\n';
        if (!str) return;
        var cur = (doc === document && typeof document !== 'undefined') ? document.currentScript : null;
        var parent = (cur && cur.parentNode) ? cur.parentNode : (doc.body || doc.documentElement || doc);
        var frag = (typeof _createFragment === 'function') ? _wrapElement(_createFragment()) : (doc.createDocumentFragment ? doc.createDocumentFragment() : null);
        if (frag) {
            frag.innerHTML = str;
            var lastChild = frag.lastChild;
            if (cur && cur.parentNode) {
                var ref;
                if (_docWriteLastScript === cur && _docWriteLastNode && _docWriteLastNode.parentNode === cur.parentNode) {
                    ref = _docWriteLastNode.nextSibling;
                } else {
                    ref = cur.nextSibling;
                }
                if (ref) {
                    cur.parentNode.insertBefore(frag, ref);
                } else {
                    cur.parentNode.appendChild(frag);
                }
                _docWriteLastScript = cur;
                _docWriteLastNode = lastChild || cur;
            } else if (parent && parent.appendChild) {
                parent.appendChild(frag);
            }
        }
        if (typeof _syncWindowNamedProperties === 'function') {
            _syncWindowNamedProperties();
        }
    }
    globalThis._documentWrite = _documentWrite;

    document.write = function() {
        if (this._opened) {
            this._writeBuffer = (this._writeBuffer || '') + Array.prototype.join.call(arguments, '');
        } else {
            _documentWrite(this, arguments, false);
        }
    };
    document.writeln = function() {
        if (this._opened) {
            this._writeBuffer = (this._writeBuffer || '') + Array.prototype.join.call(arguments, '') + '\n';
        } else {
            _documentWrite(this, arguments, true);
        }
    };
    document.open = function() {
        this._opened = true;
        this._writeBuffer = '';
        return this;
    };
    document.close = function() {
        if (this._opened) {
            this._opened = false;
            var text = this._writeBuffer || '';
            this._writeBuffer = '';
            if (typeof _resetDocumentWithHtml === 'function') {
                _resetDocumentWithHtml(this, text);
            }
        }
        return this;
    };

    if (globalThis.Document && globalThis.Document.prototype) {
        globalThis.Document.prototype.open = document.open;
        globalThis.Document.prototype.close = document.close;
        globalThis.Document.prototype.write = document.write;
        globalThis.Document.prototype.writeln = document.writeln;

        globalThis.Document.prototype.getElementById = function(id) {
            if (arguments.length === 0) return null;
            var targetId = String(id);
            if (this === document && typeof _origGetById === 'function') {
                var nid = _origGetById(targetId);
                if (nid !== -1 && nid !== undefined && nid !== 0 && nid !== null) return _wrapElement(nid);
            }
            function findId(node) {
                if (!node) return null;
                if (node.nodeType === 1) {
                    var elId = (node.getAttribute && node.getAttribute('id')) || node.id || null;
                    if (elId === targetId) return node;
                }
                var kids = node.childNodes || node._childNodes || [];
                for (var i = 0; i < kids.length; i++) {
                    var found = findId(kids[i]);
                    if (found) return found;
                }
                return null;
            }
            return findId(this.documentElement || this);
        };

        globalThis.Document.prototype.getElementsByTagName = function(tag) {
            return (typeof _defaultGetElementsByTagName === 'function') ? _defaultGetElementsByTagName(this, tag) : _createHTMLCollection(function() { return []; });
        };
        globalThis.Document.prototype.getElementsByTagNameNS = function(ns, tag) {
            return (typeof _defaultGetElementsByTagNameNS === 'function') ? _defaultGetElementsByTagNameNS(this, ns, tag) : _createHTMLCollection(function() { return []; });
        };
        globalThis.Document.prototype.getElementsByClassName = function(cls) {
            return (typeof _defaultGetElementsByClassName === 'function') ? _defaultGetElementsByClassName(this, cls) : _createHTMLCollection(function() { return []; });
        };

        Object.defineProperty(globalThis.Document.prototype, 'forms', {
            get: function() {
                if (this === document && typeof _getDocumentElements === 'function') {
                    return _createHTMLCollection(function() {
                        return (_getDocumentElements('forms') || []).map(_wrapElement);
                    });
                }
                return this.getElementsByTagName('form');
            },
            configurable: true,
            enumerable: true
        });

        Object.defineProperty(globalThis.Document.prototype, 'images', {
            get: function() {
                if (this === document && typeof _getDocumentElements === 'function') {
                    return _createHTMLCollection(function() {
                        return (_getDocumentElements('images') || []).map(_wrapElement);
                    });
                }
                return this.getElementsByTagName('img');
            },
            configurable: true,
            enumerable: true
        });

        Object.defineProperty(globalThis.Document.prototype, 'scripts', {
            get: function() {
                if (this === document && typeof _getDocumentElements === 'function') {
                    return _createHTMLCollection(function() {
                        return (_getDocumentElements('scripts') || []).map(_wrapElement);
                    });
                }
                return this.getElementsByTagName('script');
            },
            configurable: true,
            enumerable: true
        });

        Object.defineProperty(globalThis.Document.prototype, 'links', {
            get: function() {
                if (this === document && typeof _getDocumentElements === 'function') {
                    return _createHTMLCollection(function() {
                        return (_getDocumentElements('links') || []).map(_wrapElement);
                    });
                }
                var allA = this.getElementsByTagName('a');
                var allArea = this.getElementsByTagName('area');
                var res = [];
                for (var i = 0; i < allA.length; i++) if (allA[i].hasAttribute('href')) res.push(allA[i]);
                for (var j = 0; j < allArea.length; j++) if (allArea[j].hasAttribute('href')) res.push(allArea[j]);
                return _createHTMLCollection(function() { return res; });
            },
            configurable: true,
            enumerable: true
        });
    }



