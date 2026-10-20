    if (typeof globalThis !== 'undefined') {
        if (typeof globalThis.DOMTokenList === 'undefined') {
            globalThis.DOMTokenList = function DOMTokenList() {
                throw new TypeError("Illegal constructor");
            };
        }
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
            globalThis.DocumentFragment.prototype.getElementsByClassName = function(c) { return _defaultGetElementsByClassName(this, c); };
            globalThis.DocumentFragment.prototype.getElementById = function(id) { return this.querySelector('#' + id); };
        }
    }

