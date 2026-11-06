    (function() {
        function XMLSerializer() {}
        XMLSerializer.prototype.serializeToString = function(node) {
            if (!node) return "";
            if (node.nodeType === 3) {
                var val = (node.nodeValue !== null && node.nodeValue !== undefined) ? String(node.nodeValue) : (node.textContent || "");
                return val.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
            }
            if (node.nodeType === 8) {
                return "<!--" + (node.nodeValue || "") + "-->";
            }
            if (node.nodeType === 11) {
                var s = "";
                if (node.childNodes) {
                    for (var i = 0; i < node.childNodes.length; i++) {
                        s += this.serializeToString(node.childNodes[i]);
                    }
                }
                return s;
            }
            if (node.nodeType === 9) {
                return (node.documentElement ? this.serializeToString(node.documentElement) : "");
            }
            if (node.nodeType === 1) {
                var tag = (node.tagName || node.nodeName || "").toLowerCase();
                var res = "<" + tag;
                if (node.attributes) {
                    for (var j = 0; j < node.attributes.length; j++) {
                        var attr = node.attributes[j];
                        if (attr && attr.name) {
                            var attrVal = String(attr.value || "").replace(/&/g, "&amp;").replace(/"/g, "&quot;");
                            res += " " + attr.name + '="' + attrVal + '"';
                        }
                    }
                }
                var voidTags = /^(area|base|br|col|embed|hr|img|input|link|meta|param|source|track|wbr)$/i;
                if (voidTags.test(tag) && (!node.childNodes || node.childNodes.length === 0)) {
                    return res + ">";
                }
                res += ">";
                if (node.childNodes && node.childNodes.length > 0) {
                    for (var k = 0; k < node.childNodes.length; k++) {
                        res += this.serializeToString(node.childNodes[k]);
                    }
                } else if (node.innerHTML !== undefined && node.innerHTML !== "") {
                    res += node.innerHTML;
                }
                res += "</" + tag + ">";
                return res;
            }
            if (node.outerHTML !== undefined) {
                return node.outerHTML;
            }
            return "";
        };
        Object.defineProperty(globalThis, 'XMLSerializer', {
            value: XMLSerializer,
            writable: true,
            enumerable: false,
            configurable: true
        });
    })();

    (function() {
        function DOMParser() {}
        DOMParser.prototype.parseFromString = function(str, type) {
            var t = String(type || 'text/html').toLowerCase();
            if (t === 'text/xml' || t === 'application/xml' || t === 'application/xhtml+xml' || t === 'image/svg+xml') {
                var doc = document.implementation.createDocument(null, "");
                doc._contentType = t;
                doc.contentType = t;
                doc._isXml = true;
                if (typeof _parseXmlToDoc === 'function') {
                    return _parseXmlToDoc(String(str), doc, t === 'application/xhtml+xml');
                }
                return doc;
            }
            var doc = document.implementation.createHTMLDocument("");
            if (doc && doc.body) {
                doc.body.innerHTML = String(str);
            }
            return doc;
        };
        Object.defineProperty(globalThis, 'DOMParser', {
            value: DOMParser,
            writable: true,
            enumerable: false,
            configurable: true
        });
    })();

    document.createRange = function() {
        return new Range();
    };
    document.getSelection = function() {
        if (!document._selection) {
            document._selection = new Selection();
        }
        return document._selection;
    };
    if (typeof window !== 'undefined') {
        window.getSelection = function() {
            return document.getSelection();
        };
    }
    document.createComment = function(text) {
        return _wrapElement(_origCC(arguments.length === 0 ? '' : String(text)));
    };
    document.createEvent = function(type) {
        var lower = String(type || "").toLowerCase();
        var evt;
        if ((lower === 'uievent' || lower === 'uievents') && typeof globalThis.UIEvent === 'function') {
            evt = new globalThis.UIEvent("");
        } else if ((lower === 'mouseevent' || lower === 'mouseevents') && typeof globalThis.MouseEvent === 'function') {
            evt = new globalThis.MouseEvent("");
        } else if ((lower === 'customevent' || lower === 'customevents') && typeof globalThis.CustomEvent === 'function') {
            evt = new globalThis.CustomEvent("");
        } else if (typeof globalThis.Event !== 'undefined') {
            evt = new globalThis.Event(type || "");
        } else {
            evt = { type: type || "" };
        }
        evt.initEvent = function(t, b, c) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
        };
        evt.initCustomEvent = function(t, b, c, detail) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
            this.detail = detail;
        };
        evt.initUIEvent = function(t, b, c, view, detail) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
            this.view = view || null;
            this.detail = (detail !== undefined) ? Number(detail) : 0;
        };
        evt.initMouseEvent = function(t, b, c, view, detail, sx, sy, cx, cy, ctrl, alt, shift, meta, button, rel) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
            this.view = view || null;
            this.detail = (detail !== undefined) ? Number(detail) : 0;
            this.screenX = sx || 0;
            this.screenY = sy || 0;
            this.clientX = cx || 0;
            this.clientY = cy || 0;
            this.ctrlKey = !!ctrl;
            this.altKey = !!alt;
            this.shiftKey = !!shift;
            this.metaKey = !!meta;
            this.button = button || 0;
            this.relatedTarget = rel || null;
        };
        return evt;
    };
    document.importNode = function(node, deep) {
        if (!node) return null;
        var cloned = (node.cloneNode) ? node.cloneNode(deep) : node;
        if (cloned && typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
            globalThis._upgradeTree(cloned, null, null);
        }
        return cloned;
    };
    document.adoptNode = function(node) {
        return (typeof _adoptNodeInternal === 'function') ? _adoptNodeInternal(this, node) : node;
    };
    document.defaultView = globalThis;
    document.hasFocus = function() { return true; };



    // (DOMImplementation, createDocument & XML/NS element creation modularized in 14b_dom_implementation.js)

    function _docCreateAttribute(localName) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'createAttribute': 1 argument required");
        var name = String(localName);
        if (name.length === 0) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute name cannot be empty.", "InvalidCharacterError");
        }
        var isHtml = !(this && (this.contentType === 'application/xml' || this.contentType === 'application/xhtml+xml' || this.contentType === 'image/svg+xml' || this._isXml));
        var finalName = isHtml ? (typeof _toAsciiLower === 'function' ? _toAsciiLower(name) : name.toLowerCase()) : name;
        var attr = Object.create(globalThis.Attr.prototype);
        attr.nodeType = 2;
        attr.name = finalName;
        attr.localName = finalName;
        attr.nodeName = finalName;
        attr._value = "";
        attr.value = "";
        attr.nodeValue = "";
        attr.textContent = "";
        attr.specified = true;
        attr.ownerElement = null;
        attr.namespaceURI = null;
        attr.prefix = null;
        attr.ownerDocument = this;
        return attr;
    }

    function _docCreateAttributeNS(namespace, qualifiedName) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'createAttributeNS': 2 arguments required");
        var info = _validateAndExtractNamespace(namespace, qualifiedName);
        var attr = Object.create(globalThis.Attr.prototype);
        attr.nodeType = 2;
        attr.name = info.qualifiedName;
        attr.localName = info.localName;
        attr.nodeName = info.qualifiedName;
        attr._value = "";
        attr.value = "";
        attr.nodeValue = "";
        attr.textContent = "";
        attr.specified = true;
        attr.ownerElement = null;
        attr.namespaceURI = info.namespaceURI;
        attr.prefix = info.prefix;
        attr.ownerDocument = this;
        return attr;
    }

    document.createAttribute = _docCreateAttribute;
    document.createAttributeNS = _docCreateAttributeNS;
    globalThis._docCreateAttribute = _docCreateAttribute;
    globalThis._docCreateAttributeNS = _docCreateAttributeNS;
    document.createElementNS = function(ns, tag) { return _createElementNS(document, ns, tag); };
    if (globalThis.Document && globalThis.Document.prototype) {
        globalThis.Document.prototype.createAttribute = _docCreateAttribute;
        globalThis.Document.prototype.createAttributeNS = _docCreateAttributeNS;
        globalThis.Document.prototype.createElementNS = function(ns, tag) {
            return _createElementNS(this, ns, tag);
        };
        globalThis.Document.prototype.createElement = function(tag) {
            if (this._isXml) return _createElementXML(this, tag);
            var el = (typeof document !== 'undefined' && document.createElement) ? document.createElement(tag) : (typeof _origCE === 'function' ? _wrapElement(_origCE(tag)) : null);
            if (el) { el._ownerDocument = this; el.ownerDocument = this; }
            return el;
        };
    }

    if (globalThis.Element && globalThis.Element.prototype) {
        globalThis.Element.prototype.getAttributeNode = function(name) {
            var val = this.getAttribute(name);
            if (val === null) return null;
            var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
            var attr = (doc && typeof doc.createAttribute === 'function') ? doc.createAttribute(name) : ((typeof document !== 'undefined' && document.createAttribute) ? document.createAttribute(name) : { name: name, value: val, nodeType: 2 });
            attr.value = val;
            attr.ownerElement = this;
            attr.ownerDocument = doc;
            attr._ownerDocument = doc;
            return attr;
        };
        globalThis.Element.prototype.getAttributeNodeNS = function(ns, localName) {
            var targetNs = (ns === "" || ns === undefined) ? null : (ns === null ? null : String(ns));
            var targetLocal = String(localName);
            var attrs = this.attributes;
            if (attrs) {
                for (var i = 0; i < attrs.length; i++) {
                    var a = attrs[i];
                    if (a && a.namespaceURI === targetNs && a.localName === targetLocal) {
                        return a;
                    }
                }
            }
            return null;
        };
        globalThis.Element.prototype.setAttributeNode = function(attr) {
            if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
            var old = this.getAttributeNode(attr.name);
            this.setAttribute(attr.name, attr.value);
            attr.ownerElement = this;
            return old;
        };
        globalThis.Element.prototype.setAttributeNodeNS = function(attr) {
            if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
            var old = this.getAttributeNodeNS(attr.namespaceURI, attr.localName);
            if (this.setAttributeNS) {
                this.setAttributeNS(attr.namespaceURI, attr.name, attr.value);
            } else {
                this.setAttribute(attr.name, attr.value);
            }
            attr.ownerElement = this;
            return old;
        };
        globalThis.Element.prototype.removeAttributeNode = function(attr) {
            if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
            if (attr.ownerElement !== this) {
                var err = typeof DOMException !== 'undefined' ? new DOMException("The attribute is not associated with this element", "NotFoundError") : new Error("NotFoundError");
                throw err;
            }
            this.removeAttribute(attr.name);
            attr.ownerElement = null;
            return attr;
        };
    }

    Object.defineProperty(document, 'body', {
        get: function() { return document.querySelector('body'); }
    });
    Object.defineProperty(document, 'head', {
        get: function() { return document.querySelector('head'); }
    });
    Object.defineProperty(document, 'documentElement', {
        get: function() { return document.querySelector('html') || document.body; }
    });
    if (globalThis.Document && globalThis.Document.prototype) {
        Object.defineProperty(globalThis.Document.prototype, 'title', {
            get: function() {
                var titles = this.getElementsByTagName ? this.getElementsByTagName('title') : [];
                if (titles && titles.length > 0) {
                    return titles[0].textContent || "";
                }
                if (this === document && this.querySelector) {
                    var t = this.querySelector('title');
                    if (t) return t.textContent || "";
                }
                return this._title || "";
            },
            set: function(val) {
                var strVal = String(val != null ? val : "");
                var titles = this.getElementsByTagName ? this.getElementsByTagName('title') : [];
                if (titles && titles.length > 0) {
                    titles[0].textContent = strVal;
                } else {
                    var head = this.head || (this.getElementsByTagName ? this.getElementsByTagName('head')[0] : null);
                    if (head && this.createElement) {
                        var titleEl = (this.createElementNS && this.documentElement && this.documentElement.namespaceURI)
                            ? this.createElementNS(this.documentElement.namespaceURI, 'title')
                            : this.createElement('title');
                        titleEl.textContent = strVal;
                        head.appendChild(titleEl);
                    }
                }
                this._title = strVal;
            },
            configurable: true,
            enumerable: true
        });
    } else {
        Object.defineProperty(document, 'title', {
            get: function() {
                var titles = document.getElementsByTagName ? document.getElementsByTagName('title') : [];
                if (titles && titles.length > 0) return titles[0].textContent || "";
                var t = document.querySelector('title');
                return t ? t.textContent : (this._title || "");
            },
            set: function(val) {
                var strVal = String(val != null ? val : "");
                var titles = document.getElementsByTagName ? document.getElementsByTagName('title') : [];
                if (titles && titles.length > 0) {
                    titles[0].textContent = strVal;
                } else {
                    var head = document.head || document.querySelector('head');
                    if (head && document.createElement) {
                        var titleEl = document.createElement('title');
                        titleEl.textContent = strVal;
                        head.appendChild(titleEl);
                    }
                }
                this._title = strVal;
            },
            configurable: true,
            enumerable: true
        });
    }
    var _currentScriptNode = null;
    globalThis.__setCurrentScript = function(nodeOrId) {
        _currentScriptNode = (nodeOrId && typeof nodeOrId === 'number') ? _wrapElement(nodeOrId) : (nodeOrId || null);
    };
    globalThis.__setCurrentScriptUrl = function(url) {
        if (!url) {
            _currentScriptNode = null;
            return;
        }
        if (typeof document !== 'undefined' && document.querySelectorAll) {
            var scripts = document.querySelectorAll('script');
            for (var i = 0; i < scripts.length; i++) {
                var s = scripts[i];
                if (s.src === url || (s.getAttribute && s.getAttribute('src') === url)) {
                    _currentScriptNode = s;
                    return;
                }
            }
        }
        var fallback = (typeof document !== 'undefined' && document.createElement) ? document.createElement('script') : {};
        if (fallback.setAttribute) fallback.setAttribute('src', url);
        fallback.src = url;
        _currentScriptNode = fallback;
    };
    Object.defineProperty(document, 'currentScript', {
        get: function() { return _currentScriptNode; },
        configurable: true
    });
    Object.defineProperty(document, 'baseURI', {
        get: function() {
            var baseEl = document.querySelector('base[href]');
            if (baseEl) {
                var href = baseEl.getAttribute('href');
                if (href) {
                    try {
                        return new URL(href, (globalThis.location && globalThis.location.href) ? globalThis.location.href : 'about:blank').href;
                    } catch(e) {}
                }
            }
            return (globalThis.location && globalThis.location.href) ? globalThis.location.href : 'about:blank';
        },
        configurable: true
    });
    document.fonts = {
        ready: Promise.resolve(),
        check: function() { return true; },
        load: function() { return Promise.resolve([]); },
        addEventListener: function() {},
        removeEventListener: function() {},
        dispatchEvent: function() { return true; }
    };

    var _docAdopted = [];
    Object.defineProperty(document, 'adoptedStyleSheets', {
        get: function() { return _docAdopted; },
        set: function(sheets) {
            if (!Array.isArray(sheets)) throw new TypeError("adoptedStyleSheets must be an Array");
            for (var i = 0; i < sheets.length; i++) {
                if (!(sheets[i] instanceof globalThis.CSSStyleSheet)) {
                    throw new TypeError("Items in adoptedStyleSheets must be CSSStyleSheet instances");
                }
            }
            _docAdopted = sheets.slice();
        },
        configurable: true,
        enumerable: true
    });

    document._nodeId = 0;
    document.nodeType = 9;
    document.nodeName = '#document';
    if (typeof globalThis !== 'undefined' && globalThis.Document && globalThis.Document.prototype) {
        Object.defineProperty(globalThis.Document.prototype, 'adoptedStyleSheets', {
            get: function() { return this === document ? _docAdopted : (this._adoptedStyleSheets || []); },
            set: function(sheets) {
                if (!Array.isArray(sheets)) throw new TypeError("adoptedStyleSheets must be an Array");
                for (var i = 0; i < sheets.length; i++) {
                    if (!(sheets[i] instanceof globalThis.CSSStyleSheet)) {
                        throw new TypeError("Items in adoptedStyleSheets must be CSSStyleSheet instances");
                    }
                }
                if (this === document) {
                    _docAdopted = sheets.slice();
                } else {
                    this._adoptedStyleSheets = sheets.slice();
                }
            },
            configurable: true,
            enumerable: true
        });
        try { Object.setPrototypeOf(document, globalThis.Document.prototype); } catch(e) {}
    }
    globalThis._createStyleObject = _createStyleObject; globalThis._addDOMEventListener = _addDOMEventListener;
    globalThis._removeDOMEventListener = _removeDOMEventListener; globalThis._dispatchDOMEvent = _dispatchDOMEvent;
    globalThis._createElementXML = _createElementXML; globalThis._createElementNS = _createElementNS;
    globalThis._dispatchInternalEvent = function(nodeId, type, dict) {
        var target;
        if (nodeId === -1 || nodeId === 'window') {
            target = (typeof globalThis !== 'undefined' && globalThis.window) ? globalThis.window : document;
        } else if (nodeId === null || nodeId === undefined || nodeId === 0 || nodeId === 'document') {
            target = document;
        } else {
            target = _wrapElement(nodeId);
        }
        if (!target) target = document;
        dict = dict || {};
        var ctor = (typeof globalThis.Event === 'function') ? globalThis.Event : function(t, d) {
            this.type = t;
            this.bubbles = !!(d && d.bubbles);
            this.cancelable = !!(d && d.cancelable);
        };
        if (dict.eventType === 'mouse' && typeof globalThis.MouseEvent === 'function') ctor = globalThis.MouseEvent;
        else if (dict.eventType === 'keyboard' && typeof globalThis.KeyboardEvent === 'function') ctor = globalThis.KeyboardEvent;
        else if (dict.eventType === 'focus' && typeof globalThis.FocusEvent === 'function') ctor = globalThis.FocusEvent;
        else if (dict.eventType === 'input' && typeof globalThis.InputEvent === 'function') ctor = globalThis.InputEvent;
        else if (dict.eventType === 'custom' && typeof globalThis.CustomEvent === 'function') ctor = globalThis.CustomEvent;
        else if (dict.eventType === 'wheel' && typeof globalThis.WheelEvent === 'function') ctor = globalThis.WheelEvent;
        else if (dict.eventType === 'touch' && typeof globalThis.TouchEvent === 'function') ctor = globalThis.TouchEvent;
        else if (dict.eventType === 'ui' && typeof globalThis.UIEvent === 'function') ctor = globalThis.UIEvent;
        else if (dict.eventType === 'submit' && typeof globalThis.SubmitEvent === 'function') ctor = globalThis.SubmitEvent;
        var evt = new ctor(type, dict);
        return _dispatchDOMEvent(target, evt);
    };
