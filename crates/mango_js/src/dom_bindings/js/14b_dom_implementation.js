    // ── DOMImplementation, createDocument & XML/NS Element Creation ────────────
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
    globalThis._validateAndExtractNamespace = _validateAndExtractNamespace;

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
        var win = (doc && doc.defaultView) ? doc.defaultView : globalThis;
        var ns = (doc && (doc.contentType === 'application/xhtml+xml' || (!doc._isXml && doc.contentType === 'text/html'))) ? "http://www.w3.org/1999/xhtml" : null;
        if (el && ns !== 'http://www.w3.org/1999/xhtml' && globalThis.Element && globalThis.Element.prototype) {
            Object.setPrototypeOf(el, globalThis.Element.prototype);
        }
        el._namespaceURI = ns; el.namespaceURI = ns;
        el._prefix = null; el.prefix = null;
        el._localName = t; el.localName = t;
        el._tagName = t; el.tagName = t;
        el._nodeName = t; el.nodeName = t;
        el._ownerDocument = doc; el.ownerDocument = doc;
        el.nodeType = 1; el.ELEMENT_NODE = 1;
        return el;
    }
    globalThis._createElementXML = _createElementXML;

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
        } else if (info.namespaceURI === "http://www.w3.org/2000/svg") {
            var svgTag = (info.localName || '').toLowerCase();
            var svgCtors = {
                'text': 'SVGTextElement', 'tspan': 'SVGTextContentElement', 'rect': 'SVGRectElement',
                'svg': 'SVGSVGElement', 'path': 'SVGPathElement', 'circle': 'SVGCircleElement',
                'g': 'SVGGElement', 'use': 'SVGUseElement'
            };
            var ctorName = svgCtors[svgTag] || 'SVGElement';
            var svgProto = (win[ctorName] && win[ctorName].prototype) || (globalThis[ctorName] && globalThis[ctorName].prototype) || (win.Element && win.Element.prototype) || Object.prototype;
            Object.setPrototypeOf(el, svgProto);
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
    globalThis._createElementNS = _createElementNS;

    (function() {
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
            dt._publicId = String(publicId != null ? publicId : "");
            dt.publicId = String(publicId != null ? publicId : "");
            dt._systemId = String(systemId != null ? systemId : "");
            dt.systemId = String(systemId != null ? systemId : "");
            var owner = this._document || (typeof document !== 'undefined' ? document : null);
            dt._ownerDocument = owner;
            dt.ownerDocument = owner;
            dt.nodeValue = null;
            return dt;
        };

        Object.defineProperty(document, 'doctype', {
            get: function() {
                if (this._doctype !== undefined && this._doctype !== null) return this._doctype;
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
            doc.implementation._ownerDocument = doc;
            doc._documentElement = null; doc.documentElement = null;
            doc._doctype = null; doc.doctype = null;
            doc.defaultView = null; doc._defaultView = null;

            doc.createElement = function(t) { return _createElementXML(doc, t); };
            doc.createElementNS = function(nsUri, qName) { return _createElementNS(doc, nsUri, qName); };
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

            Object.defineProperty(doc, 'childNodes', {
                get: function() {
                    var self = this;
                    return (typeof _createNodeList === 'function') ? _createNodeList(function() { return self._childNodes || []; }) : (self._childNodes || []);
                },
                configurable: true
            });
            Object.defineProperty(doc, 'firstChild', {
                get: function() { return (this._childNodes && this._childNodes.length > 0) ? this._childNodes[0] : null; },
                configurable: true
            });
            Object.defineProperty(doc, 'lastChild', {
                get: function() { return (this._childNodes && this._childNodes.length > 0) ? this._childNodes[this._childNodes.length - 1] : null; },
                configurable: true
            });

            doc.appendChild = function(c) {
                if (typeof _ensurePreInsertionValidity === 'function') _ensurePreInsertionValidity(this, c, null);
                if (!c) return c;
                if (c.nodeType === 11) {
                    var fKids = c.childNodes ? Array.prototype.slice.call(c.childNodes) : (c._childNodes ? Array.prototype.slice.call(c._childNodes) : []);
                    for (var fi = 0; fi < fKids.length; fi++) this.appendChild(fKids[fi]);
                    return c;
                }
                this._childNodes.push(c);
                c._ownerDocument = this; c.ownerDocument = this; c._parentNode = this; c.parentNode = this;
                if (c.nodeType === 1 && !this._documentElement) { this._documentElement = c; this.documentElement = c; }
                else if (c.nodeType === 10) { this._doctype = c; this.doctype = c; }
                return c;
            };
            doc.removeChild = function(c) {
                if (typeof _defaultRemoveChild === 'function') {
                    try { _defaultRemoveChild(this, c); } catch(e) {}
                }
                var idx = this._childNodes.indexOf(c);
                if (idx !== -1) { this._childNodes.splice(idx, 1); c._parentNode = null; c.parentNode = null; }
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
            var ownerDoc = this._document || this._ownerDocument;
            var creationRealm = (ownerDoc && (ownerDoc._creationRealm || ownerDoc.defaultView)) ? (ownerDoc._creationRealm || ownerDoc.defaultView) : null;
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
            var docProto = (globalThis.Document && globalThis.Document.prototype) || Object.prototype;
            var doc = Object.create(docProto);
            doc._nodeType = 9; doc.nodeType = 9; doc.DOCUMENT_NODE = 9; doc.nodeName = "#document";
            doc.nodeValue = null; doc._ownerDocument = null; doc.ownerDocument = null;
            doc.title = title !== undefined ? String(title) : '';
            doc._creationRealm = creationRealm;
            doc.contentType = 'text/html'; doc.compatMode = 'CSS1Compat'; doc._compatMode = 'CSS1Compat';
            doc.characterSet = 'UTF-8'; doc._characterSet = 'UTF-8'; doc.charset = 'UTF-8'; doc.inputEncoding = 'UTF-8';
            doc.location = null;
            doc._childNodes = [doctype, html];
            doc._doctype = doctype; doc.doctype = doctype;
            doctype._ownerDocument = doc; doctype.ownerDocument = doc;
            doctype._parentNode = doc; doctype.parentNode = doc;
            html._ownerDocument = doc; html.ownerDocument = doc;
            html._parentNode = doc; html.parentNode = doc;
            head._ownerDocument = doc; head.ownerDocument = doc;
            body._ownerDocument = doc; body.ownerDocument = doc;
            if (titleEl) { titleEl._ownerDocument = doc; titleEl.ownerDocument = doc; }
            if (titleTn) { titleTn._ownerDocument = doc; titleTn.ownerDocument = doc; }
            doc.documentElement = html; doc._documentElement = html;
            doc.head = head; doc.body = body;

            doc.implementation = Object.create(DOMImplementation.prototype);
            doc.implementation._document = doc;
            doc.implementation._ownerDocument = doc;

            doc.createElement = function(t) {
                if (doc._isXml) return _createElementXML(doc, t);
                var el = (typeof document !== 'undefined' && document.createElement) ? document.createElement(t) : (typeof _origCE === 'function' ? _wrapElement(_origCE(t)) : null);
                if (el) { el._ownerDocument = doc; el.ownerDocument = doc; if (doc._creationRealm) el._creationRealm = doc._creationRealm; }
                return el;
            };
            doc.createElementNS = function(nsUri, qName) {
                var el = _createElementNS(doc, nsUri, qName);
                if (el && doc._creationRealm) el._creationRealm = doc._creationRealm;
                return el;
            };
            doc.createTextNode = function(t) {
                var tn = _wrapElement(_origCTN(t));
                if (tn) { tn._ownerDocument = doc; tn.ownerDocument = doc; if (doc._creationRealm) tn._creationRealm = doc._creationRealm; }
                return tn;
            };
            doc.createDocumentFragment = function() {
                var f = _wrapElement(_createFragment());
                if (f) { f._ownerDocument = doc; f.ownerDocument = doc; if (doc._creationRealm) f._creationRealm = doc._creationRealm; }
                return f;
            };
            doc.createComment = function(t) {
                var c = (typeof document !== 'undefined' && document.createComment) ? document.createComment(t) : null;
                if (c) { c._ownerDocument = doc; c.ownerDocument = doc; if (doc._creationRealm) c._creationRealm = doc._creationRealm; }
                return c;
            };

            Object.defineProperty(doc, 'childNodes', {
                get: function() {
                    var self = this;
                    return (typeof _createNodeList === 'function') ? _createNodeList(function() { return self._childNodes || []; }) : (self._childNodes || []);
                },
                configurable: true
            });
            Object.defineProperty(doc, 'firstChild', {
                get: function() { return (this._childNodes && this._childNodes.length > 0) ? this._childNodes[0] : null; },
                configurable: true
            });
            Object.defineProperty(doc, 'lastChild', {
                get: function() { return (this._childNodes && this._childNodes.length > 0) ? this._childNodes[this._childNodes.length - 1] : null; },
                configurable: true
            });

            doc.appendChild = function(c) {
                if (typeof _ensurePreInsertionValidity === 'function') _ensurePreInsertionValidity(this, c, null);
                if (!c) return c;
                if (c.nodeType === 11) {
                    var fKids = c.childNodes ? Array.prototype.slice.call(c.childNodes) : (c._childNodes ? Array.prototype.slice.call(c._childNodes) : []);
                    for (var fi = 0; fi < fKids.length; fi++) this.appendChild(fKids[fi]);
                    return c;
                }
                if (!this._childNodes) this._childNodes = [];
                this._childNodes.push(c);
                c._ownerDocument = this; c.ownerDocument = this; c._parentNode = this; c.parentNode = this;
                if (c.nodeType === 1 && !this._documentElement) { this._documentElement = c; this.documentElement = c; }
                else if (c.nodeType === 10) { this._doctype = c; this.doctype = c; }
                return c;
            };
            doc.removeChild = function(c) {
                if (typeof _defaultRemoveChild === 'function') {
                    try { _defaultRemoveChild(this, c); } catch(e) {}
                }
                var idx = this._childNodes ? this._childNodes.indexOf(c) : -1;
                if (idx !== -1) { this._childNodes.splice(idx, 1); c._parentNode = null; c.parentNode = null; }
                if (this._documentElement === c || this.documentElement === c) { this._documentElement = null; this.documentElement = null; }
                if (this._doctype === c || this.doctype === c) { this._doctype = null; this.doctype = null; }
                return c;
            };
            doc.insertBefore = function(n, r) {
                if (typeof _ensurePreInsertionValidity === 'function') _ensurePreInsertionValidity(this, n, r);
                if (!r) return this.appendChild(n);
                if (!n) return n;
                if (!this._childNodes) this._childNodes = [];
                var idx = this._childNodes.indexOf(r);
                if (idx === -1) return this.appendChild(n);
                this._childNodes.splice(idx, 0, n);
                n._ownerDocument = this; n.ownerDocument = this; n._parentNode = this; n.parentNode = this;
                if (n.nodeType === 1 && !this._documentElement) { this._documentElement = n; this.documentElement = n; }
                else if (n.nodeType === 10) { this._doctype = n; this.doctype = n; }
                return n;
            };

            return doc;
        };

        Object.defineProperty(globalThis, 'DOMImplementation', {
            value: DOMImplementation,
            writable: true,
            enumerable: false,
            configurable: true
        });
        document.implementation = Object.create(DOMImplementation.prototype);
        document.implementation._document = document;
    })();

    (function() {
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
        Object.defineProperty(globalThis, 'Attr', {
            value: Attr,
            writable: true,
            enumerable: false,
            configurable: true
        });
    })();
