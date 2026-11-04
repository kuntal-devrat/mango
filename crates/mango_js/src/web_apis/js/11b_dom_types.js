    // ── 24. Other DOM types ───────────────────────────────────────────────────
    globalThis.DocumentFragment = function() {};
    globalThis.DocumentFragment.prototype = Object.create(globalThis.Node.prototype);
    globalThis.DocumentFragment.prototype.append = globalThis.Element.prototype.append;
    globalThis.DocumentFragment.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.DocumentFragment.prototype.replaceChildren = globalThis.Element.prototype.replaceChildren;
    globalThis.DocumentFragment.prototype.querySelector = globalThis.Element.prototype.querySelector;
    globalThis.DocumentFragment.prototype.querySelectorAll = globalThis.Element.prototype.querySelectorAll;
    globalThis.DocumentFragment.prototype.getElementsByTagName = function(t) {
        return (typeof _defaultGetElementsByTagName === 'function') ? _defaultGetElementsByTagName(this, t) : [];
    };
    globalThis.DocumentFragment.prototype.getElementsByClassName = function(c) {
        return (typeof _defaultGetElementsByClassName === 'function') ? _defaultGetElementsByClassName(this, c) : [];
    };
    globalThis.DocumentFragment.prototype.getElementById = function(id) { return this.querySelector('#' + id); };

    globalThis.Document.prototype.append = globalThis.Element.prototype.append;
    globalThis.Document.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.Document.prototype.querySelector = globalThis.Element.prototype.querySelector;
    globalThis.Document.prototype.querySelectorAll = globalThis.Element.prototype.querySelectorAll;
    globalThis.Document.prototype.getElementsByTagName = globalThis.Element.prototype.getElementsByTagName;
    globalThis.Document.prototype.getElementsByTagNameNS = globalThis.Element.prototype.getElementsByTagNameNS;
    globalThis.Document.prototype.getElementsByClassName = globalThis.Element.prototype.getElementsByClassName;
    globalThis.Document.prototype.getElementById = function(id) {
        var sid = String(id);
        var res = this.querySelector('#' + sid);
        if (res) return res;
        var all = this.querySelectorAll ? this.querySelectorAll('*') : [];
        for (var i = 0; i < all.length; i++) {
            var el = all[i];
            if (el && (el.id === sid || (el.getAttribute && el.getAttribute('id') === sid))) return el;
        }
        return null;
    };
    globalThis.Document.prototype.replaceChildren = function() {
        if (typeof _doReplaceChildren === 'function') {
            return _doReplaceChildren(this, arguments, true);
        } else if (typeof globalThis !== 'undefined' && globalThis._doReplaceChildren) {
            return globalThis._doReplaceChildren(this, arguments, true);
        }
    };

    var _shadowRootCtor = function ShadowRoot() {};
    _shadowRootCtor.prototype = Object.create(globalThis.DocumentFragment.prototype);

    Object.defineProperty(globalThis, 'ShadowRoot', {
        get: function() { return _shadowRootCtor; },
        set: function(c) {
            _shadowRootCtor = c;
        },
        configurable: true
    });
    if (!globalThis.DOMImplementation) globalThis.DOMImplementation = function() {};
    globalThis.CharacterData = function() { throw new TypeError("Illegal constructor"); };
    globalThis.CharacterData.prototype = Object.create(globalThis.Node.prototype);
    globalThis.CharacterData.prototype.constructor = globalThis.CharacterData;
    Object.defineProperty(globalThis.CharacterData.prototype, 'data', {
        get: function() {
            if (this === globalThis.CharacterData.prototype || (globalThis.Comment && this === globalThis.Comment.prototype) || (globalThis.Text && this === globalThis.Text.prototype)) {
                throw new TypeError("Illegal invocation: 'get data' called on prototype");
            }
            if (this._data !== undefined) return this._data;
            if (this._nodeId !== undefined && typeof _getNodeValue === 'function') {
                var v = _getNodeValue(this._nodeId);
                if (v !== null && v !== undefined) return v;
            }
            return (this._nodeValue !== undefined && this._nodeValue !== null) ? this._nodeValue : '';
        },
        set: function(val) {
            var oldVal = this.data;
            var str = val === null ? '' : String(val);
            this._data = str;
            this._nodeValue = str;
            if (this._nodeId !== undefined && typeof _setNodeValue === 'function') _setNodeValue(this._nodeId, str);
            if (this.parentNode && this.parentNode._sheet) {
                this.parentNode._sheet._hasManualRules = false;
            }
            if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                globalThis._reportDOMMutation({ type: 'characterData', target: this, oldValue: oldVal });
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.CharacterData.prototype, 'nodeValue', {
        get: function() { return this.data; },
        set: function(val) { this.data = val; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.CharacterData.prototype, 'textContent', {
        get: function() { return (typeof _defaultGetTextContent === 'function') ? _defaultGetTextContent(this) : this.data; },
        set: function(val) {
            if (typeof _defaultSetTextContent === 'function') {
                _defaultSetTextContent(this, val);
            } else {
                this.data = (val === null || val === undefined) ? '' : String(val);
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.CharacterData.prototype, 'length', {
        get: function() { return (this.data || '').length; },
        configurable: true
    });
    globalThis.CharacterData.prototype.substringData = function(offset, count) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'substringData': 2 arguments required");
        var str = this.data || '', uOffset = offset >>> 0, uCount = count >>> 0;
        if (uOffset > str.length) throw new ((globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error))("The index is not in the allowed range.", "IndexSizeError");
        return str.substring(uOffset, uOffset + uCount);
    };
    globalThis.CharacterData.prototype.appendData = function(data) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'appendData': 1 argument required");
        this.data = (this.data || '') + String(data);
    };
    globalThis.CharacterData.prototype.insertData = function(offset, data) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'insertData': 2 arguments required");
        this.replaceData(offset, 0, data);
    };
    globalThis.CharacterData.prototype.deleteData = function(offset, count) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'deleteData': 2 arguments required");
        this.replaceData(offset, count, '');
    };
    globalThis.CharacterData.prototype.replaceData = function(offset, count, data) {
        if (arguments.length < 3) throw new TypeError("Failed to execute 'replaceData': 3 arguments required");
        var str = this.data || '', uOffset = offset >>> 0, uCount = count >>> 0;
        if (uOffset > str.length) throw new ((globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error))("The index is not in the allowed range.", "IndexSizeError");
        if (uOffset + uCount > str.length) uCount = str.length - uOffset;
        this.data = str.substring(0, uOffset) + String(data) + str.substring(uOffset + uCount);
    };
    globalThis.Text = function(data) {
        var t = Object.create(globalThis.Text.prototype);
        t.nodeType = 3;
        t.nodeName = "#text";
        t.ownerDocument = (typeof document !== 'undefined') ? document : null;
        t.data = data === undefined ? "" : String(data);
        return t;
    };
    globalThis.Text.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.Text.prototype.constructor = globalThis.Text;
    globalThis.Text.prototype.splitText = function(offset) {
        if (arguments.length < 1) throw new TypeError("Failed to execute 'splitText' on 'Text': 1 argument required");
        var str = this.data || '';
        var uOffset = offset >>> 0;
        if (uOffset > str.length) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The index is not in the allowed range.", "IndexSizeError");
        }
        var head = str.substring(0, uOffset);
        var tail = str.substring(uOffset);
        this.data = head;
        var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
        var newNode = (doc && typeof doc.createTextNode === 'function')
            ? doc.createTextNode(tail)
            : new globalThis.Text(tail);
        var parent = this.parentNode || this._parentNode;
        if (parent && typeof parent.insertBefore === 'function') {
            parent.insertBefore(newNode, this.nextSibling);
        }
        if (typeof globalThis !== 'undefined' && typeof globalThis._notifyRangeSplitText === 'function') {
            globalThis._notifyRangeSplitText(this, newNode, uOffset);
        }
        return newNode;
    };
    Object.defineProperty(globalThis.Text.prototype, 'wholeText', {
        get: function() {
            var start = this;
            while (start.previousSibling && start.previousSibling.nodeType === 3) {
                start = start.previousSibling;
            }
            var text = "";
            var cur = start;
            while (cur && cur.nodeType === 3) {
                text += cur.data || "";
                cur = cur.nextSibling;
            }
            return text;
        },
        configurable: true,
        enumerable: true
    });

    globalThis.Comment = function(data) {
        var c = Object.create(globalThis.Comment.prototype);
        c.nodeType = 8;
        c.nodeName = "#comment";
        c.ownerDocument = (typeof document !== 'undefined') ? document : null;
        c.data = data === undefined ? "" : String(data);
        return c;
    };
    globalThis.Comment.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.Comment.prototype.constructor = globalThis.Comment;

    globalThis.CDATASection = function() {};
    globalThis.CDATASection.prototype = Object.create(globalThis.Text.prototype);
    globalThis.ProcessingInstruction = function() {};
    globalThis.ProcessingInstruction.prototype = Object.create(globalThis.CharacterData.prototype);
    Object.defineProperty(globalThis.ProcessingInstruction.prototype, 'target', { get: function() { return this._target || ''; }, configurable: true, enumerable: true });
    globalThis.DocumentType = function() { throw new TypeError("Illegal constructor"); };
    globalThis.DocumentType.prototype = Object.create(globalThis.Node.prototype);
    globalThis.DocumentType.prototype.constructor = globalThis.DocumentType;
    Object.defineProperty(globalThis.DocumentType.prototype, 'name', { get: function() { return (this._name !== undefined) ? this._name : ((this._nodeId !== undefined && typeof _getNodeName === 'function') ? _getNodeName(this._nodeId) : ''); }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'publicId', { get: function() { return (this._publicId !== undefined) ? this._publicId : ((this._nodeId !== undefined && typeof _getDoctypePublicId === 'function') ? _getDoctypePublicId(this._nodeId) : ''); }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'systemId', { get: function() { return (this._systemId !== undefined) ? this._systemId : ((this._nodeId !== undefined && typeof _getDoctypeSystemId === 'function') ? _getDoctypeSystemId(this._nodeId) : ''); }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'nodeName', { get: function() { return this.name; }, configurable: true, enumerable: true });
    Object.defineProperty(globalThis.DocumentType.prototype, 'nodeType', { get: function() { if (this === globalThis.DocumentType.prototype) throw new TypeError("Illegal invocation: 'get nodeType' called on prototype"); return 10; }, configurable: true, enumerable: true });
    globalThis.XMLDocument = function() {};
    globalThis.XMLDocument.prototype = Object.create(globalThis.Document.prototype);
    globalThis.XMLDocument.prototype.constructor = globalThis.XMLDocument;

    if (globalThis.Document && globalThis.Document.prototype) {
        if (globalThis._docCreateAttribute) globalThis.Document.prototype.createAttribute = globalThis._docCreateAttribute;
        if (globalThis._docCreateAttributeNS) globalThis.Document.prototype.createAttributeNS = globalThis._docCreateAttributeNS;
        if (globalThis._createElementNS) {
            globalThis.Document.prototype.createElementNS = function(ns, tag) {
                return globalThis._createElementNS(this, ns, tag);
            };
        }
        if (globalThis._createElementXML) {
            globalThis.Document.prototype.createElement = function(tag) {
                if (this._isXml || (this.contentType && this.contentType.indexOf('xml') !== -1)) return globalThis._createElementXML(this, tag);
                var el = (typeof document !== 'undefined' && document.createElement) ? document.createElement(tag) : null;
                if (el) { el._ownerDocument = this; el.ownerDocument = this; }
                return el;
            };
        }
    }

    ['before', 'after', 'replaceWith', 'remove'].forEach(function(m) {
        var fn = function() {
            var parent = this.parentElement || this.parentNode;
            if (!parent) return;
            if (m === 'remove') {
                parent.removeChild(this);
                return;
            }
            var marker = document.createTextNode('');
            if (m === 'before' || m === 'replaceWith') {
                parent.insertBefore(marker, this);
            } else if (m === 'after') {
                parent.insertBefore(marker, this.nextSibling);
            }
            var frag = document.createDocumentFragment();
            for (var i = 0; i < arguments.length; i++) {
                var arg = arguments[i];
                var n = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined))
                    ? arg
                    : document.createTextNode(String(arg));
                frag.appendChild(n);
            }
            if (m === 'replaceWith' && this.parentNode === parent) {
                parent.removeChild(this);
            }
            parent.insertBefore(frag, marker);
            parent.removeChild(marker);
        };
        globalThis.CharacterData.prototype[m] = fn;
        globalThis.DocumentType.prototype[m] = fn;
    });
    globalThis.HTMLCollection = function() {};
    globalThis.HTMLCollection.prototype = Object.create(Object.prototype);
    globalThis.HTMLCollection.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    };
    globalThis.HTMLCollection.prototype.namedItem = function(name) {
        var str = String(name);
        if (str === '') return null;
        for (var i = 0; i < this.length; i++) {
            var el = this[i];
            if (el) {
                var elId = (el.getAttribute && typeof el.getAttribute === 'function') ? el.getAttribute('id') : el.id;
                if (elId === str) return el;
                var elNs = (el.namespaceURI !== undefined) ? el.namespaceURI : 'http://www.w3.org/1999/xhtml';
                if (elNs === 'http://www.w3.org/1999/xhtml' && el.getAttribute && typeof el.getAttribute === 'function' && el.getAttribute('name') === str) {
                    return el;
                }
            }
        }
        return null;
    };
    globalThis.HTMLCollection.prototype[Symbol.iterator] = function() {
        var i = 0;
        var self = this;
        return {
            next: function() {
                if (i < self.length) {
                    return { value: self[i++], done: false };
                }
                return { value: undefined, done: true };
            }
        };
    };
    globalThis.HTMLCollection.prototype[Symbol.toStringTag] = 'HTMLCollection';

    globalThis.NodeList = function() {};
    globalThis.NodeList.prototype = Object.create(Object.prototype);
    globalThis.NodeList.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    };
    globalThis.NodeList.prototype.forEach = Array.prototype.forEach;
    globalThis.NodeList.prototype.entries = Array.prototype.entries;
    globalThis.NodeList.prototype.keys = Array.prototype.keys;
    globalThis.NodeList.prototype.values = Array.prototype.values;
    globalThis.NodeList.prototype[Symbol.iterator] = Array.prototype[Symbol.iterator];
    globalThis.NodeList.prototype[Symbol.toStringTag] = 'NodeList';
    globalThis.NodeFilter = {
        FILTER_ACCEPT:1, FILTER_REJECT:2, FILTER_SKIP:3,
        SHOW_ALL:-1, SHOW_ELEMENT:1, SHOW_ATTRIBUTE:2, SHOW_TEXT:4,
        SHOW_COMMENT:128, SHOW_DOCUMENT:256, SHOW_DOCUMENT_FRAGMENT:1024
    };
    function DOMRectReadOnly(x, y, width, height) {
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }
    Object.defineProperties(DOMRectReadOnly.prototype, {
        top:    { get: function() { return this.y; }, enumerable: true },
        left:   { get: function() { return this.x; }, enumerable: true },
        right:  { get: function() { return this.x + this.width; }, enumerable: true },
        bottom: { get: function() { return this.y + this.height; }, enumerable: true }
    });
    DOMRectReadOnly.prototype.toJSON = function() {
        return { x: this.x, y: this.y, width: this.width, height: this.height, top: this.top, left: this.left, right: this.right, bottom: this.bottom };
    };
    DOMRectReadOnly.fromRect = function(other) {
        return new DOMRectReadOnly(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    };
    DOMRectReadOnly.prototype[Symbol.toStringTag] = 'DOMRectReadOnly';
    globalThis.DOMRectReadOnly = DOMRectReadOnly;

    function DOMRect(x, y, width, height) {
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }
    DOMRect.prototype = Object.create(DOMRectReadOnly.prototype);
    DOMRect.prototype.constructor = DOMRect;
    DOMRect.fromRect = function(other) {
        return new DOMRect(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    };
    DOMRect.prototype[Symbol.toStringTag] = 'DOMRect';
    globalThis.DOMRect = DOMRect;

    function DOMRectList(rects) {
        var arr = rects || [];
        this.length = arr.length;
        for (var i = 0; i < arr.length; i++) {
            this[i] = arr[i];
        }
    }
    DOMRectList.prototype.item = function(index) {
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    };
    DOMRectList.prototype[Symbol.iterator] = function() {
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
    DOMRectList.prototype[Symbol.toStringTag] = 'DOMRectList';
    globalThis.DOMRectList = DOMRectList;
    globalThis.DOMPoint = function(x,y,z,w) { this.x=x||0; this.y=y||0; this.z=z||0; this.w=w!==undefined?w:1; };
    globalThis.DOMMatrix = function() {
        this.a=1; this.b=0; this.c=0; this.d=1; this.e=0; this.f=0;
        this.is2D=true; this.isIdentity=true;
        this.transformPoint=function(p){ return p; };
        this.multiply=function(){ return this; };
        this.inverse=function(){ return this; };
    };

    if (typeof globalThis._bindElementPrototype === 'function') {
        if (globalThis.Element && globalThis.Element.prototype) globalThis._bindElementPrototype(globalThis.Element.prototype);
        if (globalThis.HTMLElement && globalThis.HTMLElement.prototype) globalThis._bindElementPrototype(globalThis.HTMLElement.prototype);
        if (globalThis.SVGElement && globalThis.SVGElement.prototype) globalThis._bindElementPrototype(globalThis.SVGElement.prototype);
    }
