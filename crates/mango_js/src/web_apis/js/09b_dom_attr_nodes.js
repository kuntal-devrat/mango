    function _setupHyperlinkUtils(proto) {
        function _getURL(el) {
            var raw = el.getAttribute ? (el.getAttribute('href') || '') : (el._href || '');
            var base = (el && el.ownerDocument && el.ownerDocument.baseURI) || ((typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'about:blank');
            try {
                return new URL(raw, base);
            } catch(e) {
                return { href: raw, pathname: raw.charAt(0) === '/' ? raw : '/' + raw, search: '', hash: '', host: '', hostname: '', port: '', protocol: 'https:', origin: '' };
            }
        }

        Object.defineProperties(proto, {
            href: {
                get: function() { return _getURL(this).href; },
                set: function(v) { if (this.setAttribute) this.setAttribute('href', String(v)); else this._href = String(v); },
                configurable: true,
                enumerable: true
            },
            pathname: {
                get: function() { return _getURL(this).pathname; },
                set: function(v) {
                    var u = _getURL(this);
                    u.pathname = String(v);
                    this.href = u.href;
                },
                configurable: true,
                enumerable: true
            },
            search: {
                get: function() { return _getURL(this).search; },
                set: function(v) {
                    var u = _getURL(this);
                    u.search = String(v);
                    this.href = u.href;
                },
                configurable: true,
                enumerable: true
            },
            hash: {
                get: function() { return _getURL(this).hash; },
                set: function(v) {
                    var u = _getURL(this);
                    u.hash = String(v);
                    this.href = u.href;
                },
                configurable: true,
                enumerable: true
            },
            host: {
                get: function() { return _getURL(this).host; },
                configurable: true,
                enumerable: true
            },
            hostname: {
                get: function() { return _getURL(this).hostname; },
                configurable: true,
                enumerable: true
            },
            port: {
                get: function() { return _getURL(this).port; },
                configurable: true,
                enumerable: true
            },
            protocol: {
                get: function() { return _getURL(this).protocol; },
                configurable: true,
                enumerable: true
            },
            origin: {
                get: function() { return _getURL(this).origin; },
                configurable: true,
                enumerable: true
            }
        });
    }

    globalThis.Element.prototype.getAttributeNode = function(name) {
        var val = this.getAttribute(name);
        if (val === null) return null;
        if (!this._attrNodes) this._attrNodes = {};
        var n = String(name);
        if (this._attrNodes[n]) { this._attrNodes[n].value = val; return this._attrNodes[n]; }
        var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
        var attr = (doc && doc.createAttribute) ? doc.createAttribute(name) : { name: name, value: val, specified: true, nodeType: 2, nodeName: name, nodeValue: val };
        attr.value = val; attr.ownerElement = this; attr.ownerDocument = doc; attr._ownerDocument = doc;
        if (globalThis.Attr) Object.setPrototypeOf(attr, globalThis.Attr.prototype);
        this._attrNodes[n] = attr;
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
        if (attr.ownerElement && attr.ownerElement !== this) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute is in use by another element.", "InUseAttributeError");
        }
        if (!this._attrNodes) this._attrNodes = {};
        var old = this.getAttributeNode ? this.getAttributeNode(attr.name) : null;
        this.setAttribute(attr.name, attr.value);
        attr.ownerElement = this; attr.ownerDocument = this.ownerDocument; attr._ownerDocument = this.ownerDocument;
        this._attrNodes[attr.name] = attr;
        return old;
    };

    globalThis.Element.prototype.setAttributeNodeNS = function(attr) {
        if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
        if (attr.ownerElement && attr.ownerElement !== this) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute is in use by another element.", "InUseAttributeError");
        }
        if (!this._attrNodes) this._attrNodes = {};
        var old = this.getAttributeNodeNS ? this.getAttributeNodeNS(attr.namespaceURI, attr.localName) : null;
        if (this.setAttributeNS) {
            this.setAttributeNS(attr.namespaceURI, attr.name, attr.value);
        } else {
            this.setAttribute(attr.name, attr.value);
        }
        attr.ownerElement = this; attr.ownerDocument = this.ownerDocument; attr._ownerDocument = this.ownerDocument;
        var key = (attr.namespaceURI || '') + ':' + attr.localName;
        this._attrNodes[key] = attr; this._attrNodes[attr.localName] = attr; this._attrNodes[attr.name] = attr;
        return old;
    };

    globalThis.Element.prototype.removeAttributeNode = function(attr) {
        if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
        if (attr.ownerElement !== this) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute is not associated with this element.", "NotFoundError");
        }
        if (this._attrNodes) {
            delete this._attrNodes[attr.name]; delete this._attrNodes[attr.localName];
            delete this._attrNodes[(attr.namespaceURI || '') + ':' + attr.localName];
        }
        attr.ownerElement = null;
        if (attr.namespaceURI && this.removeAttributeNS) {
            this.removeAttributeNS(attr.namespaceURI, attr.localName);
        } else {
            this.removeAttribute(attr.name);
        }
        return attr;
    };
