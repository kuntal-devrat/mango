    // ── 24. DOM constructor hierarchy ─────────────────────────────────────────

    globalThis.Node = function() {};
    globalThis.Node.prototype = Object.create(EventTarget.prototype);
    globalThis.Node.prototype.constructor = globalThis.Node;
    // Node type constants and Document Position bitmasks
    var _nodeConsts = {
        ELEMENT_NODE:1, ATTRIBUTE_NODE:2, TEXT_NODE:3, CDATA_SECTION_NODE:4, ENTITY_REFERENCE_NODE:5, ENTITY_NODE:6,
        PROCESSING_INSTRUCTION_NODE:7, COMMENT_NODE:8, DOCUMENT_NODE:9, DOCUMENT_TYPE_NODE:10, DOCUMENT_FRAGMENT_NODE:11, NOTATION_NODE:12,
        DOCUMENT_POSITION_DISCONNECTED:1, DOCUMENT_POSITION_PRECEDING:2, DOCUMENT_POSITION_FOLLOWING:4, DOCUMENT_POSITION_CONTAINS:8,
        DOCUMENT_POSITION_CONTAINED_BY:16, DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC:32
    };
    Object.assign(globalThis.Node, _nodeConsts);
    Object.assign(globalThis.Node.prototype, _nodeConsts);
    if (globalThis.Attr && globalThis.Attr.prototype) Object.setPrototypeOf(globalThis.Attr.prototype, globalThis.Node.prototype);
    globalThis.Node.prototype.appendChild  = function(c) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'appendChild' on 'Node': 1 argument required");
        return (typeof _defaultAppendChild === 'function') ? _defaultAppendChild(this, c) : c;
    };
    globalThis.Node.prototype.removeChild  = function(c) { return (typeof _defaultRemoveChild === 'function') ? _defaultRemoveChild(this, c) : c; };
    globalThis.Node.prototype.insertBefore = function(n, r) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required");
        return (typeof _defaultInsertBefore === 'function') ? _defaultInsertBefore(this, n, r) : n;
    };
    globalThis.Node.prototype.replaceChild = function(n, o) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'replaceChild' on 'Node': 2 arguments required");
        return (typeof _defaultReplaceChild === 'function') ? _defaultReplaceChild(this, n, o) : o;
    };
    globalThis.Node.prototype.moveBefore   = function(n, r) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'moveBefore' on 'Node': 2 arguments required");
        return (typeof _defaultInsertBefore === 'function') ? _defaultInsertBefore(this, n, r) : n;
    };
    globalThis.Node.prototype.cloneNode    = function(deep) {
        if (this && this._nodeId !== undefined && typeof _cloneNode === 'function') {
            var cloneId = _cloneNode(this._nodeId, !!deep);
            var clObj = (cloneId && typeof _wrapElement === 'function') ? _wrapElement(cloneId) : null;
            if (clObj) {
                if (this._prefix !== undefined) { clObj._prefix = this._prefix; clObj.prefix = this._prefix; }
                var curNs = this.namespaceURI; if (curNs) { clObj._namespaceURI = curNs; clObj.namespaceURI = curNs; }
                if (this._localName !== undefined) { clObj._localName = this._localName; clObj.localName = this._localName; }
                if (this._tagName !== undefined) { clObj._tagName = this._tagName; clObj.tagName = this._tagName; }
                if (this._nodeName !== undefined) { clObj._nodeName = this._nodeName; clObj.nodeName = this._nodeName; }
                if (this._isPI) { clObj._isPI = true; clObj._target = this._target; }
            }
            return clObj;
        }
        if (this && this.nodeType === 10) {
            var impl = (this.ownerDocument && this.ownerDocument.implementation) || (typeof document !== 'undefined' && document.implementation);
            if (impl && impl.createDocumentType) {
                var c = impl.createDocumentType(this.name || '', this.publicId || '', this.systemId || '');
                c._ownerDocument = this._ownerDocument || this.ownerDocument;
                return c;
            }
        }
        var copy = Object.create(Object.getPrototypeOf(this));
        for (var k in this) { if (Object.prototype.hasOwnProperty.call(this, k)) copy[k] = this[k]; }
        copy._parentNode = null; copy.parentNode = null;
        if (copy._childNodes) {
            copy._childNodes = deep ? (this._childNodes || []).map(function(c) { var cl = c && c.cloneNode ? c.cloneNode(true) : c; if (cl) cl._parentNode = copy; return cl; }) : [];
            if (!deep && copy.nodeType === 9) {
                copy._documentElement = null; copy.documentElement = null;
                copy._doctype = null; copy.doctype = null;
                copy.title = ""; copy._title = "";
            }
        }
        return copy;
    };
    globalThis.Node.prototype.contains     = function(o) {
        if (!o) return false;
        if (o === this) return true;
        if (this._nodeId !== undefined && o._nodeId !== undefined && typeof _elementContains === 'function') {
            return _elementContains(this._nodeId, o._nodeId);
        }
        var p = o.parentNode;
        while (p) {
            if (p === this) return true;
            p = p.parentNode;
        }
        return false;
    };
    globalThis.Node.prototype.hasChildNodes = function() { return (this.children && this.children.length > 0) || (this.childNodes && this.childNodes.length > 0) || false; };
    globalThis.Node.prototype.normalize    = function() { if (typeof _normalizeNode === 'function') return _normalizeNode(this); };
    Object.defineProperties(globalThis.Node.prototype, {
        parentNode: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getParentNode === 'function') {
                    var p = _getParentNode(this._nodeId);
                    if (p === -1) return this._parentNode || null;
                    if (p === 0) return document;
                    return (typeof _wrapElement === 'function') ? _wrapElement(p) : null;
                }
                return this._parentNode || null;
            },
            set: function(val) { this._parentNode = val; },
            configurable: true, enumerable: true
        },
        parentElement: {
            get: function() { var p = this.parentNode; return (p && p.nodeType === 1) ? p : null; },
            configurable: true, enumerable: true
        },
        firstChild: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getFirstChild === 'function') {
                    var fid = _getFirstChild(this._nodeId);
                    return (fid !== null && fid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(fid) : null;
                }
                var kids = this.childNodes; return (kids && kids.length > 0) ? kids[0] : null;
            },
            configurable: true, enumerable: true
        },
        lastChild: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getLastChild === 'function') {
                    var lid = _getLastChild(this._nodeId);
                    return (lid !== null && lid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(lid) : null;
                }
                var kids = this.childNodes; return (kids && kids.length > 0) ? kids[kids.length - 1] : null;
            },
            configurable: true, enumerable: true
        },
        previousSibling: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getPreviousSibling === 'function') {
                    var sid = _getPreviousSibling(this._nodeId);
                    if (sid !== null && sid !== undefined && sid !== -1 && typeof _wrapElement === 'function') return _wrapElement(sid);
                }
                var p = this.parentNode || this._parentNode;
                if (p) {
                    var k = p.childNodes || p._childNodes;
                    if (k) {
                        for (var i = 0; i < k.length; i++) {
                            if (k[i] === this || (k[i] && k[i]._nodeId !== undefined && k[i]._nodeId === this._nodeId)) return i > 0 ? k[i - 1] : null;
                        }
                    }
                }
                return null;
            },
            configurable: true, enumerable: true
        },
        nextSibling: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getNextSibling === 'function') {
                    var sid = _getNextSibling(this._nodeId);
                    if (sid !== null && sid !== undefined && sid !== -1 && typeof _wrapElement === 'function') return _wrapElement(sid);
                }
                var p = this.parentNode || this._parentNode;
                if (p) {
                    var k = p.childNodes || p._childNodes;
                    if (k) {
                        for (var i = 0; i < k.length; i++) {
                            if (k[i] === this || (k[i] && k[i]._nodeId !== undefined && k[i]._nodeId === this._nodeId)) return i + 1 < k.length ? k[i + 1] : null;
                        }
                    }
                }
                return null;
            },
            configurable: true, enumerable: true
        },
        childNodes: {
            get: function() {
                if (this._cachedChildNodes) return this._cachedChildNodes;
                var self = this;
                if (typeof _createNodeList === 'function' && typeof _getChildNodeIds === 'function' && self._nodeId !== undefined) {
                    this._cachedChildNodes = _createNodeList(function() {
                        var arr = _getChildNodeIds(self._nodeId) || [];
                        return arr.map(function(id) { return (typeof _wrapElement === 'function') ? _wrapElement(id) : id; });
                    });
                    return this._cachedChildNodes;
                }
                if (typeof _createNodeList === 'function') {
                    this._cachedChildNodes = _createNodeList(function() { return self._childNodes || []; });
                    return this._cachedChildNodes;
                }
                return self._childNodes || [];
            },
            configurable: true,
            enumerable: true
        },
        isConnected: {
            get: function() {
                var cur = this, depth = 0;
                while (cur && depth++ < 1000) {
                    if (cur === document || cur.nodeType === 9) return true;
                    if (cur.nodeType === 11 && cur.host) { cur = cur.host; continue; }
                    var p = cur.parentNode || cur.parentElement || cur.host;
                    if (!p || p === cur) break;
                    cur = p;
                }
                return false;
            },
            configurable: true,
            enumerable: true
        },
        nodeType: {
            get: function() {
                if (this === globalThis.Node.prototype ||
                    (globalThis.CharacterData && this === globalThis.CharacterData.prototype) ||
                    (globalThis.Comment && this === globalThis.Comment.prototype) ||
                    (globalThis.Text && this === globalThis.Text.prototype) ||
                    (globalThis.Element && this === globalThis.Element.prototype) ||
                    (globalThis.Document && this === globalThis.Document.prototype) ||
                    (globalThis.XMLDocument && this === globalThis.XMLDocument.prototype) ||
                    (globalThis.DocumentFragment && this === globalThis.DocumentFragment.prototype) ||
                    (globalThis.DocumentType && this === globalThis.DocumentType.prototype) ||
                    (globalThis.Attr && this === globalThis.Attr.prototype) ||
                    (this.constructor && this.constructor.prototype === this)) {
                    throw new TypeError("Illegal invocation: 'get nodeType' called on prototype");
                }
                if (this._nodeType !== undefined) return this._nodeType;
                return (this._nodeId !== undefined && typeof _getNodeType === 'function')
                    ? _getNodeType(this._nodeId)
                    : 1;
            },
            set: function(val) { this._nodeType = val; }, configurable: true, enumerable: true
        },
        nodeName: {
            get: function() {
                if (this._nodeName !== undefined) return this._nodeName;
                return (this._nodeId !== undefined && typeof _getNodeName === 'function') ? _getNodeName(this._nodeId) : (this.tagName || '#node');
            },
            set: function(v) { this._nodeName = v; }, configurable: true, enumerable: true
        },
        nodeValue: {
            get: function() {
                var nt = this.nodeType;
                if (nt === 2) return this.value;
                if (nt === 3 || nt === 4 || nt === 7 || nt === 8) {
                    if (this._data !== undefined) return this._data;
                    if (this._nodeId !== undefined && typeof _getNodeValue === 'function') {
                        var v = _getNodeValue(this._nodeId);
                        if (v !== null && v !== undefined) return v;
                    }
                    return (this._nodeValue !== undefined) ? this._nodeValue : '';
                }
                return null;
            },
            set: function(v) {
                var nt = this.nodeType;
                if (nt === 2) {
                    this.value = v;
                    return;
                }
                if (nt === 3 || nt === 4 || nt === 7 || nt === 8) {
                    var str = (v === null || v === undefined) ? '' : String(v);
                    this.data = str;
                    return;
                }
            },
            configurable: true,
            enumerable: true
        },
        textContent: {
            get: function() {
                return (typeof _defaultGetTextContent === 'function') ? _defaultGetTextContent(this) : null;
            },
            set: function(val) {
                if (typeof _defaultSetTextContent === 'function') {
                    _defaultSetTextContent(this, val);
                }
            },
            configurable: true,
            enumerable: true
        }
    });
    globalThis.Node.prototype.isEqualNode = function(other) {
        if (this === other) return true;
        if (!other || typeof other !== 'object' || this.nodeType !== other.nodeType) return false;
        var nt = this.nodeType;
        if (nt === 10) {
            if (this.name !== other.name || this.publicId !== other.publicId || this.systemId !== other.systemId) return false;
        } else if (nt === 1) {
            if (this.namespaceURI !== other.namespaceURI || this.prefix !== other.prefix || this.localName !== other.localName) return false;
            var aList = this.attributes ? Array.from(this.attributes) : [], bList = other.attributes ? Array.from(other.attributes) : [];
            if (aList.length !== bList.length) return false;
            for (var i = 0; i < aList.length; i++) {
                var a = aList[i], found = false;
                for (var j = 0; j < bList.length; j++) {
                    var b = bList[j];
                    if (a.namespaceURI === b.namespaceURI && a.localName === b.localName && a.value === b.value) { found = true; break; }
                }
                if (!found) return false;
            }
        } else if (nt === 2) {
            if (this.namespaceURI !== other.namespaceURI || this.localName !== other.localName || this.value !== other.value) return false;
        } else if (nt === 7) {
            if (this.target !== other.target || this.data !== other.data) return false;
        } else if (nt === 3 || nt === 8) {
            if (this.data !== other.data) return false;
        }
        var aKids = this.childNodes || [], bKids = other.childNodes || [];
        if (aKids.length !== bKids.length) return false;
        for (var k = 0; k < aKids.length; k++) {
            if (!aKids[k].isEqualNode(bKids[k])) return false;
        }
        return true;
    };
    globalThis.Node.prototype.isSameNode = function(o) { return this === o; };
    Object.defineProperty(globalThis.Node.prototype, 'ownerDocument', {
        get: function() {
            if (this.nodeType === 9 || (typeof document !== 'undefined' && this === document)) return null;
            if (this.nodeType === 2 && this.ownerElement && this.ownerElement.ownerDocument) {
                return this.ownerElement.ownerDocument;
            }
            return (this._ownerDocument !== undefined) ? this._ownerDocument : (typeof document !== 'undefined' ? document : null);
        },
        set: function(val) { this._ownerDocument = val; }, configurable: true, enumerable: true
    });
    Object.defineProperty(globalThis.Node.prototype, 'baseURI', {
        get: function() {
            if (this.nodeType === 2 && this.ownerElement && this.ownerElement.baseURI) {
                return this.ownerElement.baseURI;
            }
            var doc = (this.nodeType === 9) ? this : (this.ownerDocument || (typeof document !== 'undefined' ? document : null));
            return (doc && doc.baseURI) || (typeof location !== 'undefined' ? location.href : 'about:blank');
        },
        configurable: true,
        enumerable: true
    });
    globalThis.Node.prototype.getRootNode     = function(options) {
        var cur = this;
        while (cur) {
            var parent = cur.parentNode || cur.parentElement;
            if (!parent) break;
            cur = parent;
        }
        return cur || (typeof document !== 'undefined' ? document : this);
    };
    globalThis.Node.prototype.compareDocumentPosition = function(other) {
        if (typeof _compareDocumentPositionNodes === 'function') {
            return _compareDocumentPositionNodes(this, other);
        }
        if (this === other) return 0;
        return 1 | 32 | 4;
    };

    globalThis.Element = function() {};
    globalThis.Element.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Element.prototype.constructor = globalThis.Element;
    Object.defineProperties(globalThis.Element.prototype, {
        prefix: {
            get: function() {
                return (this._prefix !== undefined) ? this._prefix : null;
            },
            configurable: true,
            enumerable: true
        },
        localName: {
            get: function() {
                if (this._localName !== undefined) return this._localName;
                var t = this.tagName;
                return t ? t.toLowerCase() : null;
            },
            configurable: true,
            enumerable: true
        },
        namespaceURI: {
            get: function() {
                if (this._namespaceURI !== undefined) return this._namespaceURI;
                var cur = this;
                while (cur && cur.nodeType === 1) {
                    var t = (cur._tagName || (cur._nodeId !== undefined && typeof _getTagName === 'function' ? _getTagName(cur._nodeId) : '') || '').toLowerCase();
                    if (t === 'svg' || t === 'use' || t === 'g' || t === 'path' || t === 'circle' || t === 'rect') return 'http://www.w3.org/2000/svg';
                    cur = cur._parentNode || cur.parentNode;
                }
                var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
                if (doc && doc._isXml && doc.contentType !== 'application/xhtml+xml') return null;
                return 'http://www.w3.org/1999/xhtml';
            },
            configurable: true,
            enumerable: true
        },
        tagName: {
            get: function() {
                if (this._tagName !== undefined) return this._tagName;
                if (this._nodeId !== undefined && typeof _getTagName === 'function') {
                    var t = _getTagName(this._nodeId);
                    if (this.namespaceURI !== 'http://www.w3.org/1999/xhtml') return t ? t.toLowerCase() : t;
                    return t;
                }
                return '';
            },
            configurable: true,
            enumerable: true
        },
        id: {
            get: function() { return this.getAttribute ? (this.getAttribute('id') || '') : ''; },
            set: function(v) { if (this.setAttribute) this.setAttribute('id', v); },
            configurable: true,
            enumerable: true
        },
        className: {
            get: function() { return this.getAttribute ? (this.getAttribute('class') || '') : ''; },
            set: function(v) { if (this.setAttribute) this.setAttribute('class', v); },
            configurable: true,
            enumerable: true
        },
        slot: {
            get: function() { return this.getAttribute ? (this.getAttribute('slot') || '') : ''; },
            set: function(v) { if (this.setAttribute) this.setAttribute('slot', v); },
            configurable: true,
            enumerable: true
        },
        style: {
            get: function() {
                if (!this._style) {
                    var creator = (typeof _createStyleObject === 'function')
                        ? _createStyleObject
                        : (typeof globalThis._createStyleObject === 'function')
                            ? globalThis._createStyleObject
                            : null;
                    if (creator) {
                        this._style = creator(this._nodeId);
                    } else {
                        this._style = {
                            _props: {},
                            getPropertyValue: function(p) {
                                if (!this._props) this._props = {};
                                return this._props[p] || '';
                            },
                            setProperty: function(p, v) {
                                if (!this._props) this._props = {};
                                this._props[p] = String(v);
                            },
                            removeProperty: function(p) {
                                if (!this._props) this._props = {};
                                var r = this._props[p];
                                delete this._props[p];
                                return r || '';
                            }
                        };
                    }
                }
                return this._style;
            },
            set: function(val) {
                if (typeof val === 'string' && this.setAttribute) {
                    this.setAttribute('style', val);
                }
            },
            configurable: true,
            enumerable: true
        },
        firstElementChild: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getFirstElementChild === 'function') {
                    var eid = _getFirstElementChild(this._nodeId);
                    return (eid !== null && eid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(eid) : null;
                }
                var kids = this.children;
                return (kids && kids.length > 0) ? kids[0] : null;
            },
            configurable: true,
            enumerable: true
        },
        lastElementChild: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getLastElementChild === 'function') {
                    var eid = _getLastElementChild(this._nodeId);
                    return (eid !== null && eid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(eid) : null;
                }
                var kids = this.children;
                return (kids && kids.length > 0) ? kids[kids.length - 1] : null;
            },
            configurable: true,
            enumerable: true
        },
        previousElementSibling: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getPreviousElementSibling === 'function') {
                    var eid = _getPreviousElementSibling(this._nodeId);
                    return (eid !== null && eid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(eid) : null;
                }
                var cur = this.previousSibling;
                while (cur) {
                    if (cur.nodeType === 1) return cur;
                    cur = cur.previousSibling;
                }
                return null;
            },
            configurable: true,
            enumerable: true
        },
        nextElementSibling: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getNextElementSibling === 'function') {
                    var eid = _getNextElementSibling(this._nodeId);
                    return (eid !== null && eid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(eid) : null;
                }
                var cur = this.nextSibling;
                while (cur) {
                    if (cur.nodeType === 1) return cur;
                    cur = cur.nextSibling;
                }
                return null;
            },
            configurable: true,
            enumerable: true
        },
        childElementCount: {
            get: function() {
                var kids = this.children;
                return kids ? kids.length : 0;
            },
            configurable: true,
            enumerable: true
        },
        children: {
            get: function() {
                var self = this;
                if (typeof _createHTMLCollection === 'function' && typeof _getChildren === 'function' && self._nodeId !== undefined) {
                    return _createHTMLCollection(function() {
                        var arr = _getChildren(self._nodeId) || [];
                        return arr.map(function(id) { return (typeof _wrapElement === 'function') ? _wrapElement(id) : id; });
                    });
                }
                return self._children || [];
            },
            configurable: true,
            enumerable: true
        }
    });
    globalThis.Element.prototype.getAttribute       = function(n) { return null; };
    globalThis.Element.prototype.setAttribute       = function(n, v) {};
    globalThis.Element.prototype.removeAttribute    = function(n) {};
    globalThis.Element.prototype.hasAttribute       = function(n) { return false; };
    globalThis.Element.prototype.toggleAttribute    = function(n, f) { var has = this.hasAttribute(n); if (f === undefined ? !has : f) this.setAttribute(n,''); else this.removeAttribute(n); return this.hasAttribute(n); };
    globalThis.Element.prototype.getAttributeNS     = function(ns, n) { return this.getAttribute(n); };
    globalThis.Element.prototype.setAttributeNS     = function(ns, n, v) { this.setAttribute(n, v); };
    globalThis.Element.prototype.removeAttributeNS  = function(ns, n) { this.removeAttribute(n); };
    globalThis.Element.prototype.hasAttributeNS     = function(ns, n) { return this.hasAttribute(n); };
    globalThis.Element.prototype.getAttributeNames  = function() { return []; };
    globalThis.Element.prototype.hasAttributes      = function() { return (this.getAttributeNames ? this.getAttributeNames() : []).length > 0; };
    if (!globalThis.NamedNodeMap) globalThis.NamedNodeMap = function NamedNodeMap() {};
    if (!globalThis.Attr) {
        globalThis.Attr = function Attr() { throw new TypeError("Illegal constructor"); };
    }
    globalThis.DOMTokenList = function DOMTokenList() {
        throw new TypeError("Illegal constructor");
    };
    Object.defineProperty(globalThis.Element.prototype, 'classList', {
        get: function() {
            if (!this._classList) {
                var self = this;
                var proto = (typeof globalThis.DOMTokenList !== 'undefined') ? globalThis.DOMTokenList.prototype : Object.prototype;
                var base = Object.create(proto);
                function getTokens() {
                    var s = (typeof self.getAttribute === 'function') ? (self.getAttribute('class') || '') : (self._className || '');
                    return s ? s.split(/\s+/).filter(Boolean) : [];
                }
                function setTokens(tokens) {
                    var s = tokens.join(' ');
                    if (typeof self.setAttribute === 'function') self.setAttribute('class', s);
                    self._className = s;
                }
                base.item = function(index) {
                    var tokens = getTokens();
                    var i = Number(index);
                    return (isNaN(i) || i < 0 || i >= tokens.length) ? null : tokens[i];
                };
                base.contains = function(cls) { return getTokens().indexOf(String(cls)) !== -1; };
                base.add = function() {
                    var tokens = getTokens(), changed = false;
                    for (var i = 0; i < arguments.length; i++) {
                        var t = String(arguments[i]);
                        if (t && tokens.indexOf(t) === -1) { tokens.push(t); changed = true; }
                    }
                    if (changed) setTokens(tokens);
                };
                base.remove = function() {
                    var tokens = getTokens(), changed = false;
                    for (var i = 0; i < arguments.length; i++) {
                        var t = String(arguments[i]), idx = tokens.indexOf(t);
                        if (idx !== -1) { tokens.splice(idx, 1); changed = true; }
                    }
                    if (changed) setTokens(tokens);
                };
                base.toggle = function(cls, force) {
                    cls = String(cls);
                    var exists = this.contains(cls);
                    if (force === undefined) {
                        if (exists) { this.remove(cls); return false; }
                        else { this.add(cls); return true; }
                    } else if (force) {
                        if (!exists) this.add(cls);
                        return true;
                    } else {
                        if (exists) this.remove(cls);
                        return false;
                    }
                };
                base.replace = function(oldToken, newToken) {
                    var tokens = getTokens(), idx = tokens.indexOf(String(oldToken));
                    if (idx === -1) return false;
                    tokens[idx] = String(newToken);
                    setTokens(tokens);
                    return true;
                };
                base.supports = function() { return true; };
                base.toString = function() {
                    return (typeof self.getAttribute === 'function') ? (self.getAttribute('class') || '') : (self._className || '');
                };
                base.forEach = function(cb, thisArg) {
                    var tokens = getTokens();
                    for (var i = 0; i < tokens.length; i++) cb.call(thisArg, tokens[i], i, this);
                };
                base.values = function() { return getTokens().values(); };
                base.keys = function() { return getTokens().keys(); };
                base.entries = function() { return getTokens().entries(); };
                base[Symbol.iterator] = function() { return getTokens()[Symbol.iterator](); };
                base[Symbol.toStringTag] = 'DOMTokenList';

                self._classList = new Proxy(base, {
                    get: function(target, prop, receiver) {
                        if (prop === 'length') return getTokens().length;
                        if (prop === 'value') return base.toString();
                        if (typeof prop === 'string') {
                            var idx = Number(prop);
                            if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                                var tokens = getTokens();
                                return tokens[idx];
                            }
                        }
                        if (prop in target) return target[prop];
                        return undefined;
                    },
                    set: function(target, prop, value) {
                        if (prop === 'value') {
                            if (typeof self.setAttribute === 'function') self.setAttribute('class', String(value));
                            self._className = String(value);
                            return true;
                        }
                        target[prop] = value;
                        return true;
                    },
                    has: function(target, prop) {
                        if (prop === 'length' || prop === 'value') return true;
                        if (typeof prop === 'string') {
                            var idx = Number(prop);
                            if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                                return idx < getTokens().length;
                            }
                        }
                        return prop in target;
                    },
                    ownKeys: function(target) {
                        var tokens = getTokens(), keys = [];
                        for (var i = 0; i < tokens.length; i++) keys.push(String(i));
                        keys.push('length');
                        keys.push('value');
                        return keys;
                    },
                    getOwnPropertyDescriptor: function(target, prop) {
                        if (prop === 'length') return { value: getTokens().length, writable: false, enumerable: false, configurable: true };
                        if (prop === 'value') return { value: base.toString(), writable: true, enumerable: true, configurable: true };
                        if (typeof prop === 'string') {
                            var idx = Number(prop);
                            if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                                var tokens = getTokens();
                                if (idx < tokens.length) return { value: tokens[idx], writable: false, enumerable: true, configurable: true };
                            }
                        }
                        return Object.getOwnPropertyDescriptor(target, prop);
                    }
                });
            }
            return this._classList;
        },
        configurable: true
    });
    Object.defineProperty(globalThis.Element.prototype, 'attributes', {
        get: function() {
            var self = this, names = (typeof self.getAttributeNames === 'function') ? self.getAttributeNames() : [], list = [];
            for (var i = 0; i < names.length; i++) {
                var n = names[i], v = (typeof self.getAttribute === 'function') ? (self.getAttribute(n) || '') : '';
                var nsInfo = (self._nsAttrs && self._nsAttrs[n]) || null;
                var colon = String(n).indexOf(':');
                var prefix = nsInfo ? nsInfo.prefix : (colon !== -1 ? String(n).substring(0, colon) : null);
                var localName = nsInfo ? nsInfo.localName : (colon !== -1 ? String(n).substring(colon + 1) : n);
                var nsUri = nsInfo ? nsInfo.namespaceURI : null;
                if (!nsUri) {
                    if (prefix === 'xmlns' || n === 'xmlns') nsUri = 'http://www.w3.org/2000/xmlns/';
                    else if (prefix === 'xlink') nsUri = 'http://www.w3.org/1999/xlink';
                    else if (prefix === 'xml') nsUri = 'http://www.w3.org/XML/1998/namespace';
                }
                var attrNode = {
                    name: n, value: v, specified: true, ownerElement: self, nodeName: n, nodeValue: v,
                    namespaceURI: nsUri, prefix: prefix, localName: localName
                };
                if (globalThis.Attr) Object.setPrototypeOf(attrNode, globalThis.Attr.prototype);
                list.push(attrNode);
                list[n] = attrNode;
            }
            list.item = function(index) { return this[index] || null; };
            list.getNamedItem = function(name) {
                return (typeof self.getAttributeNode === 'function') ? self.getAttributeNode(name) : (this[name] || null);
            };
            list.setNamedItem = function(attr) {
                if (attr && attr.name && typeof self.setAttribute === 'function') self.setAttribute(attr.name, attr.value || '');
            };
            list.removeNamedItem = function(name) {
                var old = (typeof self.getAttributeNode === 'function') ? self.getAttributeNode(name) : null;
                if (typeof self.removeAttribute === 'function') self.removeAttribute(name);
                return old;
            };
            list[Symbol.iterator] = function() { return Array.prototype[Symbol.iterator].call(this); };
            return list;
        },
        configurable: true
    });
    globalThis.Element.prototype.querySelector        = function(s) { return (typeof _defaultQuerySelector === 'function') ? _defaultQuerySelector(this, s) : null; };
    globalThis.Element.prototype.querySelectorAll     = function(s) { return (typeof _defaultQuerySelectorAll === 'function') ? _defaultQuerySelectorAll(this, s) : []; };
    globalThis.Element.prototype.getElementsByTagName = function(t) { return (typeof _defaultGetElementsByTagName === 'function') ? _defaultGetElementsByTagName(this, t) : []; };
    globalThis.Element.prototype.getElementsByTagNameNS = function(ns, t) { return (typeof _defaultGetElementsByTagNameNS === 'function') ? _defaultGetElementsByTagNameNS(this, ns, t) : []; };
    globalThis.Element.prototype.getElementsByClassName = function(c) { return (typeof _defaultGetElementsByClassName === 'function') ? _defaultGetElementsByClassName(this, c) : []; };
    globalThis.Element.prototype.attachShadow = function(init) {
        if (!init || typeof init !== 'object') throw new TypeError("Failed to execute 'attachShadow' on 'Element': 1 argument required.");
        var mode = init.mode;
        if (mode !== 'open' && mode !== 'closed') throw new TypeError("Failed to execute 'attachShadow': Invalid shadow root mode.");
        if (this._shadowRootInstance) return this._shadowRootInstance;
        var srId = (this._nodeId !== undefined && typeof _attachShadow === 'function') ? _attachShadow(this._nodeId, mode) : null;
        var sr = (srId !== null && srId !== undefined && typeof _wrapElement === 'function')
            ? _wrapElement(srId)
            : ((typeof _createFragment === 'function' && typeof _wrapElement === 'function') ? _wrapElement(_createFragment()) : (typeof document !== 'undefined' && document.createDocumentFragment ? document.createDocumentFragment() : {}));
        if (typeof globalThis.ShadowRoot === 'function' && globalThis.ShadowRoot.prototype) Object.setPrototypeOf(sr, globalThis.ShadowRoot.prototype);
        sr.mode = mode; sr.host = this;
        this._shadowRoot = sr; this._shadowRootInstance = sr;
        Object.defineProperty(this, 'shadowRoot', { get: function() { return (mode === 'open') ? sr : null; }, configurable: true });
        if (!sr.adoptedStyleSheets) sr._adoptedStyleSheets = [];
        return sr;
    };
    globalThis.Element.prototype.getBoundingClientRect = function() {
        var r = (typeof _getElementRect === 'function' && this._nodeId !== undefined) ? _getElementRect(this._nodeId) : null;
        var x = (r && r[0]) || 0;
        var y = (r && r[1]) || 0;
        var w = (r && r[2] > 0) ? r[2] : (this.offsetWidth || 0);
        var h = (r && r[3] > 0) ? r[3] : (this.offsetHeight || 0);
        if (w === 0 && h === 0 && this.isConnected) {
            w = (this.style && this.style.width && parseFloat(this.style.width)) || (this.children && this.children.length > 0 ? 100 : 50);
            h = (this.style && this.style.height && parseFloat(this.style.height)) || 20;
        }
        if (globalThis.DOMRect) return new globalThis.DOMRect(x, y, w, h);
        return {
            x: x, y: y, left: x, top: y, width: w, height: h, right: x + w, bottom: y + h,
            toJSON: function() { return this; }
        };
    };
    globalThis.Element.prototype.getClientRects = function() {
        var r = this.getBoundingClientRect();
        if (globalThis.DOMRectList) return new globalThis.DOMRectList([r]);
        return [r];
    };
    function _jsElementMatches(el, sel) {
        if (typeof globalThis !== 'undefined' && typeof globalThis._jsElementMatches === 'function') {
            return globalThis._jsElementMatches(el, sel);
        }
        return false;
    }

    var _matchFn = function(sel) {
        if (!this) return false;
        if (this._nodeId !== undefined && typeof globalThis._elementMatches === 'function') {
            try {
                return Boolean(globalThis._elementMatches(this._nodeId, String(sel)));
            } catch(e) {}
        }
        return _jsElementMatches(this, sel);
    };

    globalThis.Element.prototype.matches               = _matchFn;
    globalThis.Element.prototype.webkitMatchesSelector = _matchFn;
    globalThis.Element.prototype.mozMatchesSelector    = _matchFn;
    globalThis.Element.prototype.msMatchesSelector     = _matchFn;
    globalThis.Element.prototype.oMatchesSelector      = _matchFn;

