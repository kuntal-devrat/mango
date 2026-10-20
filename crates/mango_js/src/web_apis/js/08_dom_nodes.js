    // ── 24. DOM constructor hierarchy ─────────────────────────────────────────

    globalThis.Node = function() {};
    globalThis.Node.prototype = Object.create(EventTarget.prototype);
    // Node type constants and Document Position bitmasks
    var _nodeConsts = {
        ELEMENT_NODE:1, ATTRIBUTE_NODE:2, TEXT_NODE:3, CDATA_SECTION_NODE:4,
        ENTITY_REFERENCE_NODE:5, ENTITY_NODE:6, PROCESSING_INSTRUCTION_NODE:7, COMMENT_NODE:8,
        DOCUMENT_NODE:9, DOCUMENT_TYPE_NODE:10, DOCUMENT_FRAGMENT_NODE:11, NOTATION_NODE:12,
        DOCUMENT_POSITION_DISCONNECTED: 1,
        DOCUMENT_POSITION_PRECEDING: 2,
        DOCUMENT_POSITION_FOLLOWING: 4,
        DOCUMENT_POSITION_CONTAINS: 8,
        DOCUMENT_POSITION_CONTAINED_BY: 16,
        DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: 32
    };
    Object.assign(globalThis.Node, _nodeConsts);
    Object.assign(globalThis.Node.prototype, _nodeConsts);
    if (!globalThis.Node.prototype.appendChild) globalThis.Node.prototype.appendChild     = function(c) { return (this && this.appendChild && this.appendChild !== globalThis.Node.prototype.appendChild) ? this.appendChild(c) : c; };
    if (!globalThis.Node.prototype.removeChild) globalThis.Node.prototype.removeChild     = function(c) { return (this && this.removeChild && this.removeChild !== globalThis.Node.prototype.removeChild) ? this.removeChild(c) : c; };
    if (!globalThis.Node.prototype.insertBefore) globalThis.Node.prototype.insertBefore    = function(n, r) { return this.appendChild ? this.appendChild(n) : n; };
    if (!globalThis.Node.prototype.replaceChild) globalThis.Node.prototype.replaceChild    = function(n, o) { if (this.removeChild) this.removeChild(o); if (this.appendChild) this.appendChild(n); return o; };
    if (!globalThis.Node.prototype.cloneNode) globalThis.Node.prototype.cloneNode       = function(deep) { return this; };
    globalThis.Node.prototype.contains        = function(o) { return o === this; };
    globalThis.Node.prototype.hasChildNodes   = function() { return (this.children && this.children.length > 0) || false; };
    globalThis.Node.prototype.normalize       = function() {};
    Object.defineProperties(globalThis.Node.prototype, {
        parentNode: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getParentNode === 'function') {
                    var p = _getParentNode(this._nodeId);
                    if (p === -1) return null;
                    if (p === 0) return document;
                    return (typeof _wrapElement === 'function') ? _wrapElement(p) : null;
                }
                return this._parentNode || null;
            },
            set: function(val) {
                this._parentNode = val;
            },
            configurable: true,
            enumerable: true
        },
        parentElement: {
            get: function() {
                var p = this.parentNode;
                return (p && p.nodeType === 1) ? p : null;
            },
            configurable: true,
            enumerable: true
        },
        firstChild: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getFirstChild === 'function') {
                    var fid = _getFirstChild(this._nodeId);
                    if (fid !== null && fid !== undefined && typeof _wrapElement === 'function') {
                        return _wrapElement(fid);
                    }
                    return null;
                }
                var kids = this.childNodes;
                return (kids && kids.length > 0) ? kids[0] : null;
            },
            configurable: true,
            enumerable: true
        },
        lastChild: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getLastChild === 'function') {
                    var lid = _getLastChild(this._nodeId);
                    if (lid !== null && lid !== undefined && typeof _wrapElement === 'function') {
                        return _wrapElement(lid);
                    }
                    return null;
                }
                var kids = this.childNodes;
                return (kids && kids.length > 0) ? kids[kids.length - 1] : null;
            },
            configurable: true,
            enumerable: true
        },
        previousSibling: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getPreviousSibling === 'function') {
                    var sid = _getPreviousSibling(this._nodeId);
                    return (sid !== null && sid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(sid) : null;
                }
                return null;
            },
            configurable: true,
            enumerable: true
        },
        nextSibling: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getNextSibling === 'function') {
                    var sid = _getNextSibling(this._nodeId);
                    return (sid !== null && sid !== undefined && typeof _wrapElement === 'function') ? _wrapElement(sid) : null;
                }
                return null;
            },
            configurable: true,
            enumerable: true
        },
        childNodes: {
            get: function() {
                var self = this;
                if (typeof _createNodeList === 'function' && typeof _getChildNodeIds === 'function' && self._nodeId !== undefined) {
                    return _createNodeList(function() {
                        var arr = _getChildNodeIds(self._nodeId) || [];
                        return arr.map(function(id) { return (typeof _wrapElement === 'function') ? _wrapElement(id) : id; });
                    });
                }
                return self._childNodes || [];
            },
            configurable: true,
            enumerable: true
        },
        isConnected: {
            get: function() {
                var cur = this;
                while (cur) {
                    if (cur === document || cur.nodeType === 9) return true;
                    if (cur.nodeType === 11 && cur.host) {
                        cur = cur.host;
                        continue;
                    }
                    cur = cur.parentNode || cur.parentElement || cur.host;
                }
                return false;
            },
            configurable: true,
            enumerable: true
        },
        nodeType: {
            get: function() {
                return (this._nodeId !== undefined && typeof _getNodeType === 'function')
                    ? _getNodeType(this._nodeId)
                    : (this._nodeType || 1);
            },
            configurable: true,
            enumerable: true
        },
        nodeName: {
            get: function() {
                if (this._nodeId !== undefined && typeof _getNodeName === 'function') {
                    return _getNodeName(this._nodeId);
                }
                return this.tagName || '#node';
            },
            configurable: true,
            enumerable: true
        },
        nodeValue: {
            get: function() {
                return (this._nodeId !== undefined && typeof _getNodeValue === 'function')
                    ? _getNodeValue(this._nodeId)
                    : (this._nodeValue || null);
            },
            set: function(v) {
                if (this._nodeId !== undefined && typeof _setNodeValue === 'function') {
                    _setNodeValue(this._nodeId, String(v));
                } else {
                    this._nodeValue = String(v);
                }
            },
            configurable: true,
            enumerable: true
        },
        textContent: {
            get: function() {
                return (this._nodeId !== undefined && typeof _getTextContent === 'function')
                    ? _getTextContent(this._nodeId)
                    : (this._textContent || '');
            },
            set: function(val) {
                if (this._nodeId !== undefined && typeof _setTextContent === 'function') {
                    _setTextContent(this._nodeId, String(val));
                } else {
                    this._textContent = String(val);
                }
            },
            configurable: true,
            enumerable: true
        }
    });
    globalThis.Node.prototype.isEqualNode     = function(o) { return this === o; };
    globalThis.Node.prototype.isSameNode      = function(o) { return this === o; };
    Object.defineProperty(globalThis.Node.prototype, 'ownerDocument', {
        get: function() {
            return (typeof document !== 'undefined' && this === document) ? null : (typeof document !== 'undefined' ? document : null);
        },
        configurable: true
    });
    Object.defineProperty(globalThis.Node.prototype, 'baseURI', {
        get: function() {
            var doc = (this.ownerDocument || (typeof document !== 'undefined' ? document : null));
            return (doc && doc.baseURI) || (typeof location !== 'undefined' ? location.href : 'about:blank');
        },
        configurable: true
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
        if (this === other) return 0;
        if (!other || typeof other !== 'object') {
            throw new TypeError("Failed to execute 'compareDocumentPosition' on 'Node': parameter 1 is not of type 'Node'.");
        }
        var otherId = (other._nodeId !== undefined) ? other._nodeId : null;
        var myId = (this._nodeId !== undefined) ? this._nodeId : null;
        if (myId === null && typeof document !== 'undefined' && this === document) {
            myId = 0;
        }
        if (otherId === null && typeof document !== 'undefined' && other === document) {
            otherId = 0;
        }
        if (typeof _compareDocumentPosition === 'function' && myId !== null && otherId !== null) {
            return _compareDocumentPosition(myId, otherId);
        }
        return 1 | 32 | 4;
    };

    globalThis.Element = function() {};
    globalThis.Element.prototype = Object.create(globalThis.Node.prototype);
    Object.defineProperties(globalThis.Element.prototype, {
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
    globalThis.NamedNodeMap = function NamedNodeMap() {};
    globalThis.Attr = function Attr() {};
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
            var self = this;
            var names = (typeof self.getAttributeNames === 'function') ? self.getAttributeNames() : [];
            var list = [];
            for (var i = 0; i < names.length; i++) {
                var n = names[i];
                var v = (typeof self.getAttribute === 'function') ? (self.getAttribute(n) || '') : '';
                var attrNode = {
                    name: n,
                    value: v,
                    specified: true,
                    ownerElement: self,
                    nodeName: n,
                    nodeValue: v
                };
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
    globalThis.Element.prototype.getElementsByClassName = function(c) { return (typeof _defaultGetElementsByClassName === 'function') ? _defaultGetElementsByClassName(this, c) : []; };
    globalThis.Element.prototype.attachShadow = function(init) {
        var mode = (init && init.mode) || 'open';
        var srId = (this._nodeId !== undefined && typeof _attachShadow === 'function')
            ? _attachShadow(this._nodeId, mode)
            : null;
        var sr = (srId !== null && srId !== undefined && typeof _wrapElement === 'function')
            ? _wrapElement(srId)
            : ((typeof _createFragment === 'function' && typeof _wrapElement === 'function')
                ? _wrapElement(_createFragment())
                : (typeof document !== 'undefined' && document.createDocumentFragment ? document.createDocumentFragment() : {}));
        if (typeof globalThis.ShadowRoot === 'function' && globalThis.ShadowRoot.prototype) {
            Object.setPrototypeOf(sr, globalThis.ShadowRoot.prototype);
        }
        sr.mode = mode;
        sr.host = this;
        this.shadowRoot = (mode === 'open') ? sr : null;
        this._shadowRoot = (mode === 'open') ? sr : null;
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
        if (!el || !sel) return false;
        var s = String(sel).trim();
        if (!s) return false;
        if (s === '*') return true;
        if (s.indexOf(',') !== -1) {
            var parts = s.split(',');
            for (var i = 0; i < parts.length; i++) {
                if (_jsElementMatches(el, parts[i].trim())) return true;
            }
            return false;
        }
        if (s.indexOf(':not(') === 0 && s.charAt(s.length - 1) === ')') {
            var inner = s.substring(5, s.length - 1);
            return !_jsElementMatches(el, inner);
        }
        var idIdx = s.indexOf('#');
        if (idIdx !== -1) {
            var rest = s.substring(idIdx + 1);
            var endId = rest.search(/[\.\[: ]/);
            var expectedId = endId === -1 ? rest : rest.substring(0, endId);
            var elId = (el.id !== undefined && el.id !== null) ? String(el.id) : (el.getAttribute ? el.getAttribute('id') : '');
            if (elId !== expectedId) return false;
        }
        var tagMatch = s.match(/^([a-zA-Z0-9_\-]+)/);
        if (tagMatch) {
            var expectedTag = tagMatch[1].toUpperCase();
            var elTag = el.tagName ? String(el.tagName).toUpperCase() : '';
            if (elTag && elTag !== expectedTag) return false;
        }
        var classRegex = /\.([a-zA-Z0-9_\-]+)/g;
        var cm;
        while ((cm = classRegex.exec(s)) !== null) {
            var cls = cm[1];
            if (el.classList && typeof el.classList.contains === 'function') {
                if (!el.classList.contains(cls)) return false;
            } else if (el.className) {
                var clist = String(el.className).split(/\s+/);
                if (clist.indexOf(cls) === -1) return false;
            } else {
                return false;
            }
        }
        var attrRegex = /\[([a-zA-Z0-9_\-]+)(?:=([^\],]+))?\]/g;
        var am;
        while ((am = attrRegex.exec(s)) !== null) {
            var attrName = am[1];
            var expectedVal = am[2];
            if (expectedVal !== undefined) {
                expectedVal = expectedVal.replace(/^['"]|['"]$/g, '');
                var actualVal = (typeof el.getAttribute === 'function') ? el.getAttribute(attrName) : el[attrName];
                if (String(actualVal) !== String(expectedVal)) return false;
            } else {
                var hasA = (typeof el.hasAttribute === 'function') ? el.hasAttribute(attrName) : (attrName in el);
                if (!hasA) return false;
            }
        }
        if (s.indexOf(':empty') !== -1) {
            var children = el.childNodes || el.children || [];
            for (var ci = 0; ci < children.length; ci++) {
                var c = children[ci];
                if (c.nodeType === 1) return false;
                if (c.nodeType === 3 && c.nodeValue && c.nodeValue.length > 0) return false;
            }
        }
        return true;
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

