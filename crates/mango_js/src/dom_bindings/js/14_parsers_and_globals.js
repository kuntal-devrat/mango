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
        return _wrapElement(_origCC(arguments.length === 0 ? '' : String(text)));
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
        return (typeof _adoptNodeInternal === 'function') ? _adoptNodeInternal(this, node) : node;
    };
    document.defaultView = globalThis;
    document.hasFocus = function() { return true; };



    function _validateAndExtractNamespace(namespace, qualifiedName) {
        var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
        var qName = String(qualifiedName);
        if (qName.length === 0) {
            throw new DomEx("The qualified name cannot be empty.", "InvalidCharacterError");
        }
        var ns = (namespace === "" || namespace === undefined) ? null : (namespace === null ? null : String(namespace));

        for (var i = 0; i < qName.length; i++) {
            var cp = qName.charCodeAt(i);
            if (cp === 0 || cp === 0x20 || cp === 0x09 || cp === 0x0A || cp === 0x0C || cp === 0x0D || cp === 0x3E) {
                throw new DomEx("Invalid character in qualified name: " + qName, "InvalidCharacterError");
            }
        }

        var prefix = null;
        var localName = qName;
        var colonIdx = qName.indexOf(':');
        if (colonIdx !== -1) {
            prefix = qName.substring(0, colonIdx);
            localName = qName.substring(colonIdx + 1);
            if (prefix.length === 0 || localName.length === 0) {
                throw new DomEx("Invalid qualified name: " + qName, "InvalidCharacterError");
            }
        }

        var startCp = localName.charCodeAt(0);
        if (startCp < 0x80) {
            var isLetterOrUnderscoreOrColon = (startCp >= 65 && startCp <= 90) || (startCp >= 97 && startCp <= 122) || (startCp === 95) || (startCp === 58);
            if (!isLetterOrUnderscoreOrColon) {
                throw new DomEx("Invalid start character in localName: " + localName, "InvalidCharacterError");
            }
        }

        if (prefix !== null && ns === null) {
            throw new DomEx("A prefix cannot be used with a null namespace.", "NamespaceError");
        }
        if (prefix === "xml" && ns !== "http://www.w3.org/XML/1998/namespace") {
            throw new DomEx("The 'xml' prefix requires the XML namespace.", "NamespaceError");
        }
        if ((qName === "xmlns" || prefix === "xmlns") && ns !== "http://www.w3.org/2000/xmlns/") {
            throw new DomEx("The 'xmlns' prefix or qualified name requires the XMLNS namespace.", "NamespaceError");
        }
        if (ns === "http://www.w3.org/2000/xmlns/" && qName !== "xmlns" && prefix !== "xmlns") {
            throw new DomEx("The XMLNS namespace requires 'xmlns' prefix or qualified name.", "NamespaceError");
        }

        return {
            namespaceURI: ns,
            prefix: prefix,
            localName: localName,
            qualifiedName: qName
        };
    }

    function _createElementXML(doc, tagName) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'createElement': 1 argument required");
        var t = String(tagName);
        if (typeof _validateXmlName === 'function') _validateXmlName(t);
        var el = null;
        if (typeof _origCE === 'function') {
            el = _wrapElement(_origCE(t));
        }
        if (!el) {
            var proto = (globalThis.Element && globalThis.Element.prototype) || Object.prototype;
            el = Object.create(proto);
        }
        var ns = (doc && (doc.contentType === 'application/xhtml+xml' || (!doc._isXml && doc.contentType === 'text/html'))) ? "http://www.w3.org/1999/xhtml" : null;
        if (el && ns !== 'http://www.w3.org/1999/xhtml' && globalThis.Element && globalThis.Element.prototype) {
            Object.setPrototypeOf(el, globalThis.Element.prototype);
        }
        el._namespaceURI = ns; el.namespaceURI = ns;
        el._prefix = null; el.prefix = null;
        el._localName = t;
        el.localName = t;
        el._tagName = t;
        el.tagName = t;
        el._nodeName = t;
        el.nodeName = t;
        el._ownerDocument = doc;
        el.ownerDocument = doc;
        el.nodeType = 1;
        el.ELEMENT_NODE = 1;
        return el;
    }

    function _createElementNS(doc, namespace, qualifiedName) {
        if (arguments.length < 3) throw new TypeError("Failed to execute 'createElementNS': 2 arguments required");
        var info = _validateAndExtractNamespace(namespace, qualifiedName);
        var el = null;
        var isHtmlNs = info.namespaceURI === "http://www.w3.org/1999/xhtml";
        if (typeof _origCE === 'function') {
            el = _wrapElement(_origCE(isHtmlNs ? info.localName : info.qualifiedName));
        }
        if (!el) {
            var proto = (globalThis.Element && globalThis.Element.prototype) || Object.prototype;
            el = Object.create(proto);
        }
        var win = (doc && doc.defaultView) ? doc.defaultView : globalThis;
        if (isHtmlNs) {
            var isLower = (typeof _toAsciiLower === 'function') ? (info.localName === _toAsciiLower(info.localName)) : (info.localName === info.localName.toLowerCase());
            if (!isLower) {
                var unkProto = (win.HTMLUnknownElement && win.HTMLUnknownElement.prototype) || (globalThis.HTMLUnknownElement && globalThis.HTMLUnknownElement.prototype);
                if (unkProto) Object.setPrototypeOf(el, unkProto);
            }
        } else {
            var elProto = (win.Element && win.Element.prototype) || (globalThis.Element && globalThis.Element.prototype) || Object.prototype;
            Object.setPrototypeOf(el, elProto);
        }
        el._namespaceURI = info.namespaceURI; el.namespaceURI = info.namespaceURI;
        el._prefix = info.prefix; el.prefix = info.prefix;
        el._localName = info.localName; el.localName = info.localName;
        var isHtmlDoc = doc && !doc._isXml && doc.contentType !== 'application/xml' && doc.contentType !== 'application/xhtml+xml';
        var tag = (isHtmlNs && isHtmlDoc)
            ? ((typeof _toAsciiUpper === 'function') ? _toAsciiUpper(info.qualifiedName) : info.qualifiedName.toUpperCase())
            : info.qualifiedName;
        el._tagName = tag; el.tagName = tag;
        el._nodeName = tag; el.nodeName = tag;
        el._ownerDocument = doc; el.ownerDocument = doc;
        el.nodeType = 1; el.ELEMENT_NODE = 1;
        return el;
    }

    function DOMImplementation() {
        throw new TypeError("Illegal constructor");
    }
    DOMImplementation.prototype.hasFeature = function() {
        return true;
    };
    function _validateDoctypeQualifiedName(name) {
        var str = String(name);
        for (var i = 0; i < str.length; i++) {
            var cp = str.charCodeAt(i);
            if (cp === 0 || cp === 0x20 || cp === 0x09 || cp === 0x0A || cp === 0x0C || cp === 0x0D || cp === 0x3E) {
                throw new DOMException("Invalid character in doctype qualified name: " + str, "InvalidCharacterError");
            }
        }
    }

    DOMImplementation.prototype.createDocumentType = function(qualifiedName, publicId, systemId) {
        if (arguments.length < 3) {
            throw new TypeError("Failed to execute 'createDocumentType': 3 arguments required");
        }
        var qName = String(qualifiedName);
        _validateDoctypeQualifiedName(qName);
        var dt = Object.create(globalThis.DocumentType ? globalThis.DocumentType.prototype : (globalThis.Node ? globalThis.Node.prototype : Object.prototype));
        dt._nodeType = 10;
        dt.nodeType = 10;
        dt._name = qName;
        dt._nodeName = qName;
        dt.name = qName;
        dt.nodeName = qName;
        dt._publicId = String(publicId);
        dt.publicId = String(publicId);
        dt._systemId = String(systemId);
        dt.systemId = String(systemId);
        var owner = this._document || (typeof document !== 'undefined' ? document : null);
        dt._ownerDocument = owner;
        dt.ownerDocument = owner;
        dt.nodeValue = null;
        return dt;
    };

    Object.defineProperty(document, 'doctype', {
        get: function() {
            if (this._doctype !== undefined) return this._doctype;
            if (typeof _getChildNodeIds === 'function' && typeof _wrapElement === 'function') {
                var rootKids = _getChildNodeIds(0) || [];
                for (var j = 0; j < rootKids.length; j++) {
                    var k = _wrapElement(rootKids[j]);
                    if (k && k.nodeType === 10) return k;
                }
            }
            return null;
        },
        set: function(val) { this._doctype = val; },
        configurable: true,
        enumerable: true
    });
    DOMImplementation.prototype.createDocument = function(namespace, qualifiedName, doctype) {
        if (arguments.length < 2) {
            throw new TypeError("Failed to execute 'createDocument': at least 2 arguments required, but only " + arguments.length + " present.");
        }
        if (doctype !== undefined && doctype !== null) {
            if (typeof doctype !== 'object' || !doctype || doctype.nodeType !== 10) {
                throw new TypeError("Failed to execute 'createDocument': parameter 3 is not of type 'DocumentType'");
            }
        }
        var ns = (namespace === "" || namespace === undefined) ? null : (namespace === null ? null : String(namespace));
        var qName = (qualifiedName === null) ? "" : String(qualifiedName);

        var extracted = null;
        if (qName !== "") {
            extracted = _validateAndExtractNamespace(ns, qName);
        }

        var proto = (globalThis.XMLDocument && globalThis.XMLDocument.prototype) ||
                    (globalThis.Document && globalThis.Document.prototype) ||
                    Object.prototype;
        var doc = Object.create(proto);

        var docContentType = (ns === "http://www.w3.org/1999/xhtml") ? "application/xhtml+xml" :
                             (ns === "http://www.w3.org/2000/svg") ? "image/svg+xml" : "application/xml";

        doc._nodeType = 9;
        doc.nodeType = 9;
        doc.DOCUMENT_NODE = 9;
        doc.nodeName = "#document";
        doc.nodeValue = null;
        doc._ownerDocument = null;
        doc.ownerDocument = null;
        doc.location = null;
        doc._compatMode = "CSS1Compat";
        doc.compatMode = "CSS1Compat";
        doc._characterSet = "UTF-8";
        doc.characterSet = "UTF-8";
        doc.charset = "UTF-8";
        doc.inputEncoding = "UTF-8";
        doc._contentType = docContentType;
        doc.contentType = docContentType;
        doc._url = "about:blank";
        doc.URL = "about:blank";
        doc.documentURI = "about:blank";
        doc._isXml = true;
        doc._childNodes = [];
        doc.implementation = Object.create(DOMImplementation.prototype);
        doc.implementation._document = doc;
        doc._documentElement = null; doc.documentElement = null;
        doc._doctype = null; doc.doctype = null;
        doc.defaultView = null; doc._defaultView = null;

        doc.createElement = function(t) {
            return _createElementXML(doc, t);
        };
        doc.createElementNS = function(nsUri, qualifiedName) {
            return _createElementNS(doc, nsUri, qualifiedName);
        };
        doc.createTextNode = function(t) {
            var n = _wrapElement(_origCTN(t));
            if (n) { n._ownerDocument = doc; n.ownerDocument = doc; }
            return n;
        };
        doc.createDocumentFragment = function() {
            var f = _wrapElement(_createFragment());
            if (f) { f._ownerDocument = doc; f.ownerDocument = doc; }
            return f;
        };
        doc.createComment = function(t) {
            var c = (typeof document !== 'undefined' && document.createComment) ? document.createComment(t) : null;
            if (c) { c._ownerDocument = doc; c.ownerDocument = doc; }
            return c;
        };
        doc.createRange = function() {
            return (typeof document !== 'undefined' && document.createRange) ? document.createRange() : null;
        };
        doc.createProcessingInstruction = function(target, data) {
            var pi = (typeof document !== 'undefined' && document.createProcessingInstruction)
                ? document.createProcessingInstruction(target, data)
                : null;
            if (pi) { pi._ownerDocument = doc; pi.ownerDocument = doc; }
            return pi;
        };
        doc.createCDATASection = function(data) {
            return (globalThis.Document && globalThis.Document.prototype && globalThis.Document.prototype.createCDATASection)
                ? globalThis.Document.prototype.createCDATASection.call(doc, data)
                : null;
        };
        doc.createAttribute = _docCreateAttribute;
        doc.createAttributeNS = _docCreateAttributeNS;

        doc.appendChild = function(c) {
            if (typeof _ensurePreInsertionValidity === 'function') _ensurePreInsertionValidity(this, c, null);
            if (!c) return c;
            if (c.nodeType === 11) {
                var fKids = c.childNodes ? Array.prototype.slice.call(c.childNodes) : (c._childNodes ? Array.prototype.slice.call(c._childNodes) : []);
                for (var fi = 0; fi < fKids.length; fi++) this.appendChild(fKids[fi]);
                return c;
            }
            this._childNodes.push(c);
            c._ownerDocument = this; c.ownerDocument = this; c._parentNode = this;
            if (c.nodeType === 1 && !this._documentElement) { this._documentElement = c; this.documentElement = c; }
            else if (c.nodeType === 10 && !this._doctype) { this._doctype = c; this.doctype = c; }
            return c;
        };
        doc.removeChild = function(c) {
            if (typeof _defaultRemoveChild === 'function') return _defaultRemoveChild(this, c);
            var idx = this._childNodes.indexOf(c);
            if (idx !== -1) { this._childNodes.splice(idx, 1); c._parentNode = null; }
            if (this._documentElement === c || this.documentElement === c) { this._documentElement = null; this.documentElement = null; }
            if (this._doctype === c || this.doctype === c) { this._doctype = null; this.doctype = null; }
            return c;
        };

        if (doctype) {
            doc.appendChild(doctype);
        }
        if (extracted) {
            var el = doc.createElementNS(extracted.namespaceURI, extracted.qualifiedName);
            doc.appendChild(el);
        }

        return doc;
    };
    DOMImplementation.prototype.createHTMLDocument = function(title) {
        var doctype = this.createDocumentType('html', '', '');
        var head = _wrapElement(_origCE('head'));
        var body = _wrapElement(_origCE('body'));
        var html = _wrapElement(_origCE('html'));
        html.appendChild(head);
        html.appendChild(body);
        var titleEl = null, titleTn = null;
        if (title !== undefined) {
            titleEl = _wrapElement(_origCE('title'));
            titleTn = _wrapElement(_origCTN(String(title)));
            titleEl.appendChild(titleTn);
            head.appendChild(titleEl);
        }
        var doc = {
            title: title !== undefined ? String(title) : '',
            contentType: 'text/html', compatMode: 'CSS1Compat', _compatMode: 'CSS1Compat',
            characterSet: 'UTF-8', _characterSet: 'UTF-8', charset: 'UTF-8', inputEncoding: 'UTF-8', location: null,
            _childNodes: [doctype, html], _doctype: doctype, doctype: doctype,
            _documentElement: html, documentElement: html, body: body, head: head, defaultView: null, _defaultView: null,
            implementation: Object.create(DOMImplementation.prototype),
            nodeType: 9, nodeName: '#document',
            createElement: function(t) { var el = _wrapElement(_origCE(t)); if (el) { el._ownerDocument = doc; el.ownerDocument = doc; } return el; },
            createElementNS: function(ns, t) { var el = _wrapElement(_origCE(t)); if (el) { el._ownerDocument = doc; el.ownerDocument = doc; } return el; },
            createTextNode: function(t) { var tn = _wrapElement(_origCTN(t)); if (tn) { tn._ownerDocument = doc; tn.ownerDocument = doc; } return tn; },
            createDocumentFragment: function() { var f = _wrapElement(_createFragment()); if (f) { f._ownerDocument = doc; f.ownerDocument = doc; } return f; },
            createComment: function(t) { var c = document.createComment(t); if (c) { c._ownerDocument = doc; c.ownerDocument = doc; } return c; },
            createRange: function() { return document.createRange(); },
            createTreeWalker: function(root, whatToShow, filter) { return document.createTreeWalker(root, whatToShow, filter); },
            createNodeIterator: function(root, whatToShow, filter) { return document.createNodeIterator(root, whatToShow, filter); },
            createEvent: function(t) { return document.createEvent(t); },
            createAttribute: _docCreateAttribute, createAttributeNS: _docCreateAttributeNS,
            importNode: function(node, deep) { return document.importNode(node, deep); },
            adoptNode: function(node) { return (typeof _adoptNodeInternal === 'function') ? _adoptNodeInternal(doc, node) : document.adoptNode(node); },
            querySelector: function(s) { return html.querySelector ? html.querySelector(s) : null; },
            querySelectorAll: function(s) { return html.querySelectorAll ? html.querySelectorAll(s) : _createNodeList(function() { return []; }); },
            getElementById: function(id) { if (arguments.length === 0) return null; var s = String(id); return (s && html.querySelector) ? html.querySelector('#' + s) : null; },
            getElementsByTagName: function(tag) { return html.getElementsByTagName ? html.getElementsByTagName(tag) : _createHTMLCollection(function() { return []; }); }
        };
        doctype._ownerDocument = doc; doctype.ownerDocument = doc; doctype._parentNode = doc; doctype.parentNode = doc;
        html._ownerDocument = doc; html.ownerDocument = doc; html._parentNode = doc; html.parentNode = doc;
        head._ownerDocument = doc; head.ownerDocument = doc;
        body._ownerDocument = doc; body.ownerDocument = doc;
        if (titleEl) { titleEl._ownerDocument = doc; titleEl.ownerDocument = doc; }
        if (titleTn) { titleTn._ownerDocument = doc; titleTn.ownerDocument = doc; }
        doc.implementation._document = doc;
        if (typeof globalThis.Document !== 'undefined') {
            Object.setPrototypeOf(doc, globalThis.Document.prototype);
        }
        return doc;
    };
    globalThis.DOMImplementation = DOMImplementation;
    document.implementation = Object.create(DOMImplementation.prototype);
    document.implementation._document = document;

    function Attr() {
        throw new TypeError("Illegal constructor");
    }
    Attr.prototype = Object.create(globalThis.Node ? globalThis.Node.prototype : Object.prototype);
    Attr.prototype.nodeType = 2;
    Object.defineProperty(Attr.prototype, 'specified', { value: true, writable: false, enumerable: true, configurable: true });
    Object.defineProperty(Attr.prototype, 'value', {
        get: function() { return (this.ownerElement && this.ownerElement.getAttribute) ? (this.ownerElement.getAttribute(this.name) || "") : (this._value || ""); },
        set: function(v) {
            var strVal = String(v != null ? v : "");
            this._value = strVal;
            if (this.ownerElement && this.ownerElement.setAttribute) {
                this.ownerElement.setAttribute(this.name, strVal);
            }
        },
        enumerable: true,
        configurable: true
    });
    Object.defineProperty(Attr.prototype, 'nodeValue', {
        get: function() { return this.value; },
        set: function(v) { this.value = v; },
        enumerable: true,
        configurable: true
    });
    Object.defineProperty(Attr.prototype, 'textContent', {
        get: function() { return this.value; },
        set: function(v) { this.value = v; },
        enumerable: true,
        configurable: true
    });
    globalThis.Attr = Attr;

    function _docCreateAttribute(localName) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'createAttribute': 1 argument required");
        var name = String(localName);
        if (name.length === 0) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute name cannot be empty.", "InvalidCharacterError");
        }
        var isHtml = !(this && (this.contentType === 'application/xml' || this.contentType === 'application/xhtml+xml' || this.contentType === 'image/svg+xml' || this._isXml));
        var finalName = isHtml ? (typeof _toAsciiLower === 'function' ? _toAsciiLower(name) : name.toLowerCase()) : name;
        var attr = Object.create(Attr.prototype);
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
        var attr = Object.create(Attr.prototype);
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
    document.createElementNS = function(ns, tag) { return _createElementNS(document, ns, tag); };
    if (globalThis.Document && globalThis.Document.prototype) {
        globalThis.Document.prototype.createAttribute = _docCreateAttribute;
        globalThis.Document.prototype.createAttributeNS = _docCreateAttributeNS;
        globalThis.Document.prototype.createElementNS = function(ns, tag) {
            return _createElementNS(this, ns, tag);
        };
        globalThis.Document.prototype.createElement = function(tag) {
            return this._isXml ? _createElementXML(this, tag) : ((typeof document !== 'undefined' && document.createElement) ? document.createElement(tag) : null);
        };
    }

    if (globalThis.Element && globalThis.Element.prototype) {
        globalThis.Element.prototype.getAttributeNode = function(name) {
            var val = this.getAttribute(name);
            if (val === null) return null;
            var attr = document.createAttribute(name);
            attr.value = val;
            attr.ownerElement = this;
            return attr;
        };
        globalThis.Element.prototype.getAttributeNodeNS = function(ns, localName) {
            var val = this.getAttributeNS ? this.getAttributeNS(ns, localName) : this.getAttribute(localName);
            if (val === null) return null;
            var attr = document.createAttributeNS(ns, localName);
            attr.value = val;
            attr.ownerElement = this;
            return attr;
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
