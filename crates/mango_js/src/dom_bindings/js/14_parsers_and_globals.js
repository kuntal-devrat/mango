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
    globalThis.XMLSerializer = XMLSerializer;

    function DOMParser() {}
    DOMParser.prototype.parseFromString = function(str, type) {
        var doc = document.implementation.createHTMLDocument("");
        if (doc && doc.body) {
            doc.body.innerHTML = String(str);
        }
        return doc;
    };
    globalThis.DOMParser = DOMParser;

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
        return _wrapElement(_origCC(String(text === undefined ? '' : text)));
    };
    document.createEvent = function(type) {
        var evt = (typeof globalThis.Event !== 'undefined') ? new globalThis.Event(type || "") : { type: type || "" };
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
        evt.initMouseEvent = function(t, b, c) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
        };
        return evt;
    };
    document.createTreeWalker = function(root, whatToShow, filter) {
        return {
            root: root,
            currentNode: root,
            whatToShow: whatToShow || -1,
            filter: filter,
            nextNode: function() { return null; },
            previousNode: function() { return null; },
            parentNode: function() { return null; },
            firstChild: function() { return null; },
            lastChild: function() { return null; },
            nextSibling: function() { return null; },
            previousSibling: function() { return null; }
        };
    };
    document.createNodeIterator = function(root, whatToShow, filter) {
        return {
            root: root,
            whatToShow: whatToShow || -1,
            filter: filter,
            nextNode: function() { return null; },
            previousNode: function() { return null; },
            detach: function() {}
        };
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
        if (!node) return null;
        var oldDoc = node.ownerDocument || document;
        if (typeof node.adoptedCallback === 'function') {
            try { node.adoptedCallback(oldDoc, document); } catch(e) {}
        }
        return node;
    };
    document.defaultView = globalThis;
    document.hasFocus = function() { return true; };
    document.implementation = {
        createHTMLDocument: function(title) {
            var head = _wrapElement(_origCE('head'));
            var body = _wrapElement(_origCE('body'));
            var html = _wrapElement(_origCE('html'));
            html.appendChild(head);
            html.appendChild(body);
            if (title) {
                var titleEl = _wrapElement(_origCE('title'));
                titleEl.textContent = String(title);
                head.appendChild(titleEl);
            }
            var doc = {
                title: title || '',
                createElement: function(t) { return _wrapElement(_origCE(t)); },
                createElementNS: function(ns, t) { return _wrapElement(_origCE(t)); },
                createTextNode: function(t) { return _wrapElement(_origCTN(t)); },
                createDocumentFragment: function() { return _wrapElement(_createFragment()); },
                createComment: function(t) { return document.createComment(t); },
                createRange: function() { return document.createRange(); },
                createTreeWalker: function(root, whatToShow, filter) { return document.createTreeWalker(root, whatToShow, filter); },
                createNodeIterator: function(root, whatToShow, filter) { return document.createNodeIterator(root, whatToShow, filter); },
                createEvent: function(t) { return document.createEvent(t); },
                importNode: function(node, deep) { return document.importNode(node, deep); },
                adoptNode: function(node) { return document.adoptNode(node); },
                querySelector: function(s) { return html.querySelector ? html.querySelector(s) : null; },
                querySelectorAll: function(s) { return html.querySelectorAll ? html.querySelectorAll(s) : _createNodeList(function() { return []; }); },
                getElementById: function(id) { return html.querySelector ? html.querySelector('#' + id) : null; },
                getElementsByTagName: function(tag) { return html.getElementsByTagName ? html.getElementsByTagName(tag) : _createHTMLCollection(function() { return []; }); },
                body: body,
                head: head,
                documentElement: html,
                defaultView: globalThis,
                implementation: document.implementation,
                nodeType: 9,
                nodeName: '#document'
            };
            if (typeof globalThis.Document !== 'undefined') {
                Object.setPrototypeOf(doc, globalThis.Document.prototype);
            }
            return doc;
        },
        hasFeature: function() { return true; },
        createDocument: function() { return document; }
    };

    Object.defineProperty(document, 'body', {
        get: function() { return document.querySelector('body'); }
    });
    Object.defineProperty(document, 'head', {
        get: function() { return document.querySelector('head'); }
    });
    Object.defineProperty(document, 'documentElement', {
        get: function() { return document.querySelector('html') || document.body; }
    });
    Object.defineProperty(document, 'title', {
        get: function() {
            var t = document.querySelector('title');
            return t ? t.textContent : (this._title || "");
        },
        set: function(val) {
            var strVal = String(val != null ? val : "");
            var t = document.querySelector('title');
            if (t) {
                t.textContent = strVal;
            } else {
                var head = document.head || document.querySelector('head');
                if (head && document.createElement) {
                    var titleEl = document.createElement('title');
                    titleEl.textContent = strVal;
                    head.appendChild(titleEl);
                }
            }
            this._title = strVal;
        }
    });
    Object.defineProperty(document, 'currentScript', {
        get: function() {
            var s = document.getElementsByTagName('script');
            return s && s.length > 0 ? s[s.length - 1] : null;
        },
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
    globalThis._createStyleObject = _createStyleObject;
    globalThis._addDOMEventListener = _addDOMEventListener;
    globalThis._removeDOMEventListener = _removeDOMEventListener;
    globalThis._dispatchDOMEvent = _dispatchDOMEvent;
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
