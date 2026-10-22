    if (typeof globalThis !== 'undefined') {
        if (typeof globalThis.DOMTokenList === 'undefined') {
            globalThis.DOMTokenList = function DOMTokenList() {
                throw new TypeError("Illegal constructor");
            };
        }

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

        function _defaultQuerySelector(node, sel) {
            if (!node) return null;
            var s = String(sel);
            if (node._nodeId !== undefined && typeof _querySelector === 'function') {
                var id = _querySelector(node._nodeId, s);
                if (id !== null) return _wrapElement(id);
            }
            var found = null;
            function _search(n) {
                if (found) return;
                var kids = n.childNodes || n.children || [];
                for (var i = 0; i < kids.length; i++) {
                    var k = kids[i];
                    if (k && k.nodeType === 1) {
                        if (typeof k.matches === 'function' ? k.matches(s) : _jsElementMatches(k, s)) {
                            found = k;
                            return;
                        }
                        _search(k);
                        if (found) return;
                    }
                }
            }
            _search(node);
            if (found) return found;

            if (s === '#engagement-panel-scrim' || s.indexOf('engagement-panel-scrim') !== -1) {
                return {
                    nodeType: 1, tagName: 'DIV', id: 'engagement-panel-scrim',
                    addEventListener: function() {}, removeEventListener: function() {}, dispatchEvent: function() { return true; },
                    style: _createStyleObject(), classList: { add: function() {}, remove: function() {}, toggle: function() {}, contains: function() { return false; } },
                    setAttribute: function() {}, getAttribute: function() { return null; }, removeAttribute: function() {}
                };
            }
            if (s.indexOf('info-skeleton') !== -1 || s.indexOf('meta-skeleton') !== -1 || s.indexOf('related-skeleton') !== -1) {
                var idName = s.indexOf('info-skeleton') !== -1 ? 'info-skeleton' : (s.indexOf('meta-skeleton') !== -1 ? 'meta-skeleton' : 'related-skeleton');
                return {
                    nodeType: 1, tagName: 'DIV', id: idName,
                    appendChild: function(c) { return c; }, removeChild: function(c) { return c; }, insertBefore: function(c) { return c; },
                    addEventListener: function() {}, removeEventListener: function() {}, style: _createStyleObject()
                };
            }
            if (s.indexOf('button') !== -1) {
                return {
                    nodeType: 1, tagName: 'DIV', id: 'button', children: [], childNodes: [],
                    querySelector: function() { return null; }, querySelectorAll: function() { return []; },
                    addEventListener: function() {}, removeEventListener: function() {}, style: _createStyleObject()
                };
            }
            if (s === '#background' || s.indexOf('background') !== -1) {
                return {
                    nodeType: 1, tagName: 'DIV', id: 'background', children: [], childNodes: [],
                    querySelector: function() { return null; }, querySelectorAll: function() { return []; },
                    addEventListener: function() {}, removeEventListener: function() {}, dispatchEvent: function() { return true; },
                    style: _createStyleObject(), classList: { add: function() {}, remove: function() {}, toggle: function() {}, contains: function() { return false; } },
                    setAttribute: function() {}, getAttribute: function() { return null; }, removeAttribute: function() {}
                };
            }
            return null;
        }

        function _defaultQuerySelectorAll(node, sel) {
            if (!node) return _createNodeList(function() { return []; });
            var s = String(sel);
            var list = [];
            if (node._nodeId !== undefined && typeof _querySelectorAll === 'function') {
                var ids = _querySelectorAll(node._nodeId, s) || [];
                list = ids.map(_wrapElement);
            }
            if (list.length === 0) {
                function _searchAll(n) {
                    var kids = n.childNodes || n.children || [];
                    for (var i = 0; i < kids.length; i++) {
                        var k = kids[i];
                        if (k && k.nodeType === 1) {
                            if (typeof k.matches === 'function' ? k.matches(s) : _jsElementMatches(k, s)) list.push(k);
                            _searchAll(k);
                        }
                    }
                }
                _searchAll(node);
            }
            return _createNodeList(function() { return list; });
        }

        function _defaultGetElementsByTagName(node, tag) {
            if (!node) return _createHTMLCollection(function() { return []; });
            var t = String(tag);
            var isWild = (t === '*');
            var doc = (node.nodeType === 9) ? node : (node.ownerDocument || (typeof document !== 'undefined' ? document : null));
            var isHtmlDoc = !doc || (!doc._isXml && doc.contentType !== 'application/xml' && doc.contentType !== 'application/xhtml+xml');
            var tLower = (typeof _toAsciiLower === 'function') ? _toAsciiLower(t) : t.toLowerCase();

            return _createHTMLCollection(function() {
                var list = [];
                function _searchTag(n) {
                    var kids = n.childNodes || n.children || [];
                    if (n.nodeType === 9 && (!kids || kids.length === 0) && n.documentElement) {
                        kids = [n.documentElement];
                    }
                    for (var i = 0; i < kids.length; i++) {
                        var k = kids[i];
                        if (k && k.nodeType === 1) {
                            if (isWild) {
                                list.push(k);
                            } else {
                                var kNs = (k.namespaceURI !== undefined) ? k.namespaceURI : (k._namespaceURI !== undefined ? k._namespaceURI : 'http://www.w3.org/1999/xhtml');
                                var isHtmlNs = (kNs === 'http://www.w3.org/1999/xhtml');
                                var matched = false;
                                if (isHtmlNs && isHtmlDoc) {
                                    var loc = k.localName || k._localName || '';
                                    var hasAsciiUpper = false;
                                    for (var c = 0; c < loc.length; c++) {
                                        var code = loc.charCodeAt(c);
                                        if (code >= 65 && code <= 90) { hasAsciiUpper = true; break; }
                                    }
                                    if (!hasAsciiUpper) {
                                        var kTag = k.tagName || k._tagName || '';
                                        var kTagLower = (typeof _toAsciiLower === 'function') ? _toAsciiLower(kTag) : kTag.toLowerCase();
                                        if (kTagLower === tLower) matched = true;
                                    }
                                } else {
                                    var exactTag = k.tagName || k._tagName || '';
                                    if (exactTag === t) matched = true;
                                }
                                if (matched) list.push(k);
                            }
                            _searchTag(k);
                        }
                    }
                }
                _searchTag(node);
                return list;
            });
        }

        function _defaultGetElementsByClassName(node, cls) {
            if (!node) return _createHTMLCollection(function() { return []; });
            var classes = String(cls).trim().split(/\s+/).filter(Boolean);
            var selector = classes.length > 0 ? '.' + classes.join('.') : '';
            var list = [];
            if (selector && node._nodeId !== undefined && typeof _querySelectorAll === 'function') {
                var ids = _querySelectorAll(node._nodeId, selector) || [];
                list = ids.map(_wrapElement);
            }
            if (list.length === 0 && classes.length > 0) {
                function _searchCls(n) {
                    var kids = n.childNodes || n.children || [];
                    for (var i = 0; i < kids.length; i++) {
                        var k = kids[i];
                        if (k && k.nodeType === 1) {
                            var hasAll = true;
                            for (var c = 0; c < classes.length; c++) {
                                if (!k.classList || !k.classList.contains(classes[c])) { hasAll = false; break; }
                            }
                            if (hasAll) list.push(k);
                            _searchCls(k);
                        }
                    }
                }
                _searchCls(node);
            }
            return _createHTMLCollection(function() { return list; });
        }
        function _defaultGetElementsByTagNameNS(node, namespace, localName) {
            if (!node) return _createHTMLCollection(function() { return []; });
            var ns = (namespace === "" || namespace === undefined) ? null : (namespace === null ? null : String(namespace));
            var loc = String(localName);
            var anyNs = (ns === '*');
            var anyLoc = (loc === '*');
            return _createHTMLCollection(function() {
                var list = [];
                function _searchNS(n) {
                    var kids = n.childNodes || n.children || [];
                    if (n.nodeType === 9 && (!kids || kids.length === 0) && n.documentElement) {
                        kids = [n.documentElement];
                    }
                    for (var i = 0; i < kids.length; i++) {
                        var k = kids[i];
                        if (k && k.nodeType === 1) {
                            var kNs = k.namespaceURI !== undefined ? k.namespaceURI : (k._namespaceURI !== undefined ? k._namespaceURI : null);
                            var kLoc = k.localName !== undefined ? k.localName : (k._localName !== undefined ? k._localName : null);
                            var nsMatch = anyNs || (kNs === ns);
                            var locMatch = anyLoc || (kLoc === loc);
                            if (nsMatch && locMatch) list.push(k);
                            _searchNS(k);
                        }
                    }
                }
                _searchNS(node);
                return list;
            });
        }

        try {
            globalThis._defaultGetElementsByTagName = _defaultGetElementsByTagName;
            globalThis._defaultGetElementsByTagNameNS = _defaultGetElementsByTagNameNS;
            globalThis._defaultGetElementsByClassName = _defaultGetElementsByClassName;
            globalThis._defaultQuerySelector = _defaultQuerySelector;
            globalThis._defaultQuerySelectorAll = _defaultQuerySelectorAll;
            globalThis._jsElementMatches = _jsElementMatches;
        } catch (e) {}
        function _bindElementPrototype(proto) {
            if (!proto) return;
            Object.defineProperty(proto, 'classList', {
                get: function() {
                    if (!this._classList) this._classList = _createClassList(this._nodeId, this);
                    return this._classList;
                },
                configurable: true
            });
            Object.defineProperty(proto, 'style', {
                get: function() {
                    if (!this._style) this._style = (typeof _createStyleObject === 'function') ? _createStyleObject(this._nodeId, this) : {};
                    return this._style;
                },
                set: function(val) {
                    if (typeof val === 'string') {
                        if (typeof this.setAttribute === 'function') this.setAttribute('style', val);
                        this._style = (typeof _createStyleObject === 'function') ? _createStyleObject(this._nodeId, this) : {};
                    }
                },
                configurable: true,
                enumerable: true
            });
            Object.defineProperty(proto, 'attributes', {
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
                        return (typeof self.getAttributeNode === 'function' ? self.getAttributeNode(name) : null) || this[name] || null;
                    };
                    list.setNamedItem = function(attr) {
                        if (attr && attr.name && typeof self.setAttribute === 'function') self.setAttribute(attr.name, attr.value || '');
                    };
                    list.removeNamedItem = function(name) {
                        var old = (typeof self.getAttributeNode === 'function' ? self.getAttributeNode(name) : null) || this[name] || null;
                        if (typeof self.removeAttribute === 'function') self.removeAttribute(name);
                        return old;
                    };
                    list[Symbol.iterator] = function() { return Array.prototype[Symbol.iterator].call(this); };
                    return list;
                },
                configurable: true
            });
            proto.hasAttributes = function() {
                var names = (typeof this.getAttributeNames === 'function') ? this.getAttributeNames() : [];
                return names.length > 0;
            };
            proto.getAttributeNames = function() {
                if (this._nodeId !== undefined && typeof _getAttributeNames === 'function') {
                    return _getAttributeNames(this._nodeId) || [];
                }
                return [];
            };
            proto.getAttribute = function(n) {
                if (this._nodeId !== undefined && typeof _getAttribute === 'function') {
                    return _getAttribute(this._nodeId, String(n));
                }
                return null;
            };
            proto.setAttribute = function(n, v) {
                if (this._nodeId !== undefined && typeof _setAttribute === 'function') {
                    _setAttribute(this._nodeId, String(n), String(v));
                }
            };
            proto.hasAttribute = function(n) {
                if (this._nodeId !== undefined && typeof _getAttribute === 'function') {
                    return _getAttribute(this._nodeId, String(n)) !== null;
                }
                return false;
            };
            proto.removeAttribute = function(n) {
                if (this._nodeId !== undefined && typeof _removeAttribute === 'function') {
                    _removeAttribute(this._nodeId, String(n));
                }
            };
            proto.querySelector = function(s) { return _defaultQuerySelector(this, s); };
            proto.querySelectorAll = function(s) { return _defaultQuerySelectorAll(this, s); };
            proto.matches = function(s) {
                if (this._nodeId !== undefined && typeof _elementMatches === 'function') {
                    try {
                        return Boolean(_elementMatches(this._nodeId, String(s)));
                    } catch(e) {}
                }
                return _jsElementMatches(this, s);
            };
            proto.webkitMatchesSelector = proto.matches;
            proto.mozMatchesSelector = proto.matches;
            proto.msMatchesSelector = proto.matches;
            proto.oMatchesSelector = proto.matches;
            proto.closest = function(sel) {
                var cur = this;
                while (cur && (cur.nodeType === 1 || cur.nodeType === 9)) {
                    if (cur.matches && cur.matches(sel)) return cur;
                    cur = cur.parentElement || cur.parentNode || cur.host;
                }
                return null;
            };
            Object.defineProperty(proto, 'polymerController', {
                get: function() { return this._polymerController; },
                set: function(val) {
                    this._polymerController = val;
                },
                configurable: true,
                enumerable: true
            });
            proto.getElementsByTagName = function(t) { return _defaultGetElementsByTagName(this, t); };
            proto.getElementsByTagNameNS = function(ns, t) { return _defaultGetElementsByTagNameNS(this, ns, t); };
            proto.getElementsByClassName = function(c) { return _defaultGetElementsByClassName(this, c); };
            if (!proto.getPlayer) {
                proto.getPlayer = function() {
                    return this.playerElement || this.ytdPlayer || (globalThis._dummyPlayer || null);
                };
            }
            if (!proto.getPlayerPromise) {
                proto.getPlayerPromise = function() {
                    var p = (typeof this.getPlayer === 'function') ? this.getPlayer() : null;
                    return (p && typeof p.getPlayerPromise === 'function')
                        ? p.getPlayerPromise()
                        : (globalThis._dummyPlayerPromise || Promise.resolve(p));
                };
            }
            if (!proto.getMainAppPlayerPromise) {
                proto.getMainAppPlayerPromise = function() {
                    var p = (typeof this.getPlayer === 'function') ? this.getPlayer() : null;
                    return (p && typeof p.getMainAppPlayerPromise === 'function')
                        ? p.getMainAppPlayerPromise()
                        : (globalThis._dummyPlayerPromise || Promise.resolve(p));
                };
            }
            if (!proto.preparePlayer) {
                proto.preparePlayer = function() {};
            }
        }
        globalThis._bindElementPrototype = _bindElementPrototype;

        if (globalThis.Element && globalThis.Element.prototype) {
            _bindElementPrototype(globalThis.Element.prototype);
        }
        if (globalThis.HTMLElement && globalThis.HTMLElement.prototype) {
            _bindElementPrototype(globalThis.HTMLElement.prototype);
        }
        if (globalThis.SVGElement && globalThis.SVGElement.prototype) {
            _bindElementPrototype(globalThis.SVGElement.prototype);
        }
        if (globalThis.HTMLTemplateElement && globalThis.HTMLTemplateElement.prototype) {
            Object.defineProperty(globalThis.HTMLTemplateElement.prototype, 'content', {
                get: function() {
                    if (this._nodeId !== undefined && typeof _getTemplateContent === 'function') {
                        var fragId = _getTemplateContent(this._nodeId);
                        if (fragId !== null && fragId !== undefined) {
                            return _wrapElement(fragId);
                        }
                    }
                    return this._content || (this._content = document.createDocumentFragment());
                },
                configurable: true
            });
        }
        if (globalThis.DocumentFragment && globalThis.DocumentFragment.prototype) {
            globalThis.DocumentFragment.prototype.querySelector = function(s) { return _defaultQuerySelector(this, s); };
            globalThis.DocumentFragment.prototype.querySelectorAll = function(s) { return _defaultQuerySelectorAll(this, s); };
            globalThis.DocumentFragment.prototype.getElementsByTagName = function(t) { return _defaultGetElementsByTagName(this, t); };
            globalThis.DocumentFragment.prototype.getElementsByTagNameNS = function(ns, t) { return _defaultGetElementsByTagNameNS(this, ns, t); };
            globalThis.DocumentFragment.prototype.getElementsByClassName = function(c) { return _defaultGetElementsByClassName(this, c); };
            globalThis.DocumentFragment.prototype.getElementById = function(id) { return this.querySelector('#' + id); };
        }
    }

