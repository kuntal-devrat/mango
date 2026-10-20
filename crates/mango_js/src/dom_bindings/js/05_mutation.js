    // Per-node wrapper cache. DOM element identity requires that wrapping the
    // same node twice yields the same object (`document.querySelector('#a') ===
    // document.querySelector('#a')`), and listeners/styles registered through
    // one handle must stay reachable when that node is wrapped again — e.g.
    // when the chrome dispatches `input`/`change` events from Rust.
    var _wrapperCache = {};

    function _invokeConnectedCallback(node, visited) {
        if (!node || typeof node !== 'object') return;
        if (!visited) visited = new Set();
        if (visited.has(node)) return;
        visited.add(node);

        if (typeof node.connectedCallback === 'function' && !node._isCustomConnected) {
            node._isCustomConnected = true;
            try {
                node.connectedCallback();
            } catch(e) {
                if (typeof console !== 'undefined' && console.warn) {
                    console.warn("[_invokeConnectedCallback connectedCallback error on " + (node.tagName||"") + "]:", (e && e.stack) ? e.stack : e);
                }
            }
        }
        if (typeof node.attached === 'function' && !node._isAttachedCalled) {
            node._isAttachedCalled = true;
            try {
                node.attached();
            } catch(e) {
                if (typeof console !== 'undefined' && console.warn) {
                    console.warn("[_invokeConnectedCallback attached error on " + (node.tagName||"") + "]:", (e && e.stack) ? e.stack : e);
                }
            }
        }
        if (node.usePatchedLifecycles) {
            node.lifecycleIsConnected = true;
        }
        var ctrl = node.polymerController;
        if (ctrl && typeof ctrl === 'object') {
            if (typeof ctrl.connectedCallback === 'function' && !ctrl._isCustomConnected) {
                ctrl._isCustomConnected = true;
                try {
                    ctrl.connectedCallback();
                } catch(e) {
                    if (typeof console !== 'undefined' && console.warn) {
                        console.warn("[_invokeConnectedCallback polymerController.connectedCallback error]:", (e && e.stack) ? e.stack : e);
                    }
                }
            }
            if (typeof ctrl.attached === 'function' && !ctrl._isAttachedCalled) {
                ctrl._isAttachedCalled = true;
                try {
                    ctrl.attached();
                } catch(e) {
                    if (typeof console !== 'undefined' && console.warn) {
                        console.warn("[_invokeConnectedCallback polymerController.attached error]:", (e && e.stack) ? e.stack : e);
                    }
                }
            }
        }
        if (typeof node.dispatchEvent === 'function') {
            try {
                var evt = (typeof globalThis.CustomEvent === 'function')
                    ? new globalThis.CustomEvent('attached', { bubbles: true, composed: true })
                    : { type: 'attached', target: node, bubbles: true, composed: true, defaultPrevented: false };
                node.dispatchEvent(evt);
            } catch(e) {}
        }
        if (node && (node.id === 'page-manager' || (node.tagName && String(node.tagName).toLowerCase() === 'ytd-page-manager'))) {
            try {
                var appEl = (typeof document !== 'undefined' && document.querySelector) ? document.querySelector('ytd-app') : null;
                var appCtrl = appEl && (appEl.polymerController || appEl);
                if (appCtrl && appCtrl.pageManagerAttachedPromise && typeof appCtrl.pageManagerAttachedPromise.resolve === 'function') {
                    appCtrl.pageManagerAttachedPromise.resolve();
                }
            } catch(e) {}
        }
        var nodeTag = (node.tagName || '').toLowerCase();
        if ((nodeTag === 'script' || nodeTag === 'link') && !node._hasDispatchedLoad) {
            node._hasDispatchedLoad = true;
            setTimeout(function() {
                try {
                    if (typeof node.onload === 'function') {
                        node.onload({ type: 'load', target: node });
                    }
                    if (typeof node.dispatchEvent === 'function') {
                        node.dispatchEvent(new Event('load'));
                    }
                } catch(e) {}
            }, 0);
        }
        var kids = node.childNodes;
        if (kids) {
            for (var i = 0; i < kids.length; i++) {
                _invokeConnectedCallback(kids[i], visited);
            }
        }
        if (node.shadowRoot) {
            _invokeConnectedCallback(node.shadowRoot, visited);
        } else if (node.root && node.root !== node && node.root.host === node) {
            _invokeConnectedCallback(node.root, visited);
        }
    }
    globalThis._invokeConnectedCallback = _invokeConnectedCallback;

    function _invokeDisconnectedCallback(node, visited) {
        if (!node || typeof node !== 'object') return;
        if (!visited) visited = new Set();
        if (visited.has(node)) return;
        visited.add(node);

        if (typeof node.disconnectedCallback === 'function' && node._isCustomConnected) {
            node._isCustomConnected = false;
            try { node.disconnectedCallback(); } catch(e) {}
        }
        if (typeof node.detached === 'function' && node._isAttachedCalled) {
            node._isAttachedCalled = false;
            try { node.detached(); } catch(e) {}
        }
        if (node.polymerController && typeof node.polymerController.detached === 'function' && node.polymerController._isAttachedCalled) {
            node.polymerController._isAttachedCalled = false;
            try { node.polymerController.detached(); } catch(e) {}
        }
        var kids = node.childNodes;
        if (kids) {
            for (var i = 0; i < kids.length; i++) {
                _invokeDisconnectedCallback(kids[i], visited);
            }
        }
        if (node.shadowRoot) {
            _invokeDisconnectedCallback(node.shadowRoot, visited);
        } else if (node.root && node.root !== node && node.root.host === node) {
            _invokeDisconnectedCallback(node.root, visited);
        }
    }

    function _invokeAttributeChangedCallback(el, name, oldVal, newVal) {
        if (!el || typeof el !== 'object') return;
        if (oldVal === newVal) return;
        var tag = el.tagName ? String(el.tagName).toLowerCase() : '';
        var ctor = (typeof globalThis !== 'undefined' && globalThis.customElements && globalThis.customElements.get && tag)
            ? globalThis.customElements.get(tag)
            : null;
        ctor = ctor || el.constructor;
        var obs = (ctor && ctor.observedAttributes) || el.observedAttributes;
        if (Array.isArray(obs)) {
            var matches = false;
            for (var i = 0; i < obs.length; i++) {
                if (String(obs[i]).toLowerCase() === String(name).toLowerCase()) {
                    matches = true;
                    break;
                }
            }
            if (matches && typeof el.attributeChangedCallback === 'function') {
                try {
                    el.attributeChangedCallback(name, oldVal, newVal, null);
                } catch(e) {}
            }
        }
    }

    function _clearSubtreeWrapperCache(node) {
        if (!node) return;
        if (node._nodeId !== undefined) {
            delete _wrapperCache[node._nodeId];
        }
        var k = node.childNodes;
        if (k) {
            for (var i = 0; i < k.length; i++) {
                _clearSubtreeWrapperCache(k[i]);
            }
        }
    }

    function _defaultAppendChild(node, child) {
        if (!node || node._nodeId === undefined) return child;
        var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
        var childObj = _wrapElement(childId);
        var isFrag = childObj && childObj.nodeType === 11;
        var fragChildren = isFrag ? (childObj.childNodes ? Array.prototype.slice.call(childObj.childNodes) : []) : null;
        _appendChild(node._nodeId, childId);
        if (isFrag && fragChildren) {
            for (var i = 0; i < fragChildren.length; i++) {
                var fc = fragChildren[i];
                if (fc && fc.isConnected) {
                    if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                        globalThis._upgradeTree(fc, null, null);
                    }
                    _invokeConnectedCallback(fc);
                }
            }
        } else if (childObj && childObj.isConnected) {
            if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                globalThis._upgradeTree(childObj, null, null);
            }
            _invokeConnectedCallback(childObj);
        }
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: node,
                addedNodes: isFrag && fragChildren ? fragChildren : [childObj],
                removedNodes: []
            });
        }
        return child;
    }

    function _defaultRemoveChild(node, child) {
        if (!node || node._nodeId === undefined) return child;
        var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
        var childObj = _wrapElement(childId);
        _removeChild(node._nodeId, childId);
        if (childObj) {
            _invokeDisconnectedCallback(childObj);
        }
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: node,
                addedNodes: [],
                removedNodes: [childObj]
            });
        }
        return child;
    }

    function _defaultInsertBefore(node, newChild, refChild) {
        if (!node || node._nodeId === undefined) return newChild;
        var childId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
        var refId = (typeof refChild === 'object' && refChild !== null && refChild._nodeId !== undefined) ? refChild._nodeId : refChild;
        var newObj = _wrapElement(childId);
        var isFrag = newObj && newObj.nodeType === 11;
        var fragChildren = isFrag ? (newObj.childNodes ? Array.prototype.slice.call(newObj.childNodes) : []) : null;
        _insertBefore(node._nodeId, childId, (refId === undefined || refId === null) ? 0 : refId);
        var refObj = (refId && refId !== 0) ? _wrapElement(refId) : null;
        if (isFrag && fragChildren) {
            for (var i = 0; i < fragChildren.length; i++) {
                var fc = fragChildren[i];
                if (fc && fc.isConnected) {
                    if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                        globalThis._upgradeTree(fc, null, null);
                    }
                    _invokeConnectedCallback(fc);
                }
            }
        } else if (newObj && newObj.isConnected) {
            if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                globalThis._upgradeTree(newObj, null, null);
            }
            _invokeConnectedCallback(newObj);
        }
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: node,
                addedNodes: isFrag && fragChildren ? fragChildren : [newObj],
                removedNodes: [],
                nextSibling: refObj
            });
        }
        return newChild;
    }

    function _defaultReplaceChild(node, newChild, oldChild) {
        if (!node || node._nodeId === undefined) return oldChild;
        var newId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
        var oldId = (typeof oldChild === 'object' && oldChild !== null && oldChild._nodeId !== undefined) ? oldChild._nodeId : oldChild;
        var oldObj = _wrapElement(oldId);
        var newObj = _wrapElement(newId);
        var isFrag = newObj && newObj.nodeType === 11;
        var fragChildren = isFrag ? (newObj.childNodes ? Array.prototype.slice.call(newObj.childNodes) : []) : null;
        _replaceChild(node._nodeId, newId, oldId);
        if (oldObj) {
            _invokeDisconnectedCallback(oldObj);
        }
        if (isFrag && fragChildren) {
            for (var i = 0; i < fragChildren.length; i++) {
                var fc = fragChildren[i];
                if (fc && fc.isConnected) {
                    if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                        globalThis._upgradeTree(fc, null, null);
                    }
                    _invokeConnectedCallback(fc);
                }
            }
        } else if (newObj && newObj.isConnected) {
            if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                globalThis._upgradeTree(newObj, null, null);
            }
            _invokeConnectedCallback(newObj);
        }
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: node,
                addedNodes: isFrag && fragChildren ? fragChildren : [newObj],
                removedNodes: [oldObj]
            });
        }
        return oldChild;
    }

    if (typeof globalThis !== 'undefined' && globalThis.Node && globalThis.Node.prototype) {
        globalThis.Node.prototype.appendChild = function(child) {
            return _defaultAppendChild(this, child);
        };
        globalThis.Node.prototype.removeChild = function(child) {
            return _defaultRemoveChild(this, child);
        };
        globalThis.Node.prototype.insertBefore = function(newChild, refChild) {
            return _defaultInsertBefore(this, newChild, refChild);
        };
        globalThis.Node.prototype.replaceChild = function(newChild, oldChild) {
            return _defaultReplaceChild(this, newChild, oldChild);
        };
        globalThis.Node.prototype.cloneNode = function(deep) {
            if (this && this._nodeId !== undefined && typeof _cloneNode === 'function') {
                var cloneId = _cloneNode(this._nodeId, !!deep);
                return cloneId ? _wrapElement(cloneId) : null;
            }
            return this;
        };
        ['appendChild', 'insertBefore', 'removeChild', 'replaceChild', 'cloneNode', 'contains'].forEach(function(m) {
            var nativeProp = '__shady_native_' + m;
            if (globalThis.Node && globalThis.Node.prototype && !globalThis.Node.prototype[nativeProp]) {
                Object.defineProperty(globalThis.Node.prototype, nativeProp, {
                    value: function() { return this[m].apply(this, arguments); },
                    writable: true,
                    configurable: true
                });
            }
            if (globalThis.Element && globalThis.Element.prototype && !globalThis.Element.prototype[nativeProp]) {
                Object.defineProperty(globalThis.Element.prototype, nativeProp, {
                    value: function() { return this[m].apply(this, arguments); },
                    writable: true,
                    configurable: true
                });
            }
            if (globalThis.Document && globalThis.Document.prototype && !globalThis.Document.prototype[nativeProp]) {
                Object.defineProperty(globalThis.Document.prototype, nativeProp, {
                    value: function() { return this[m].apply(this, arguments); },
                    writable: true,
                    configurable: true
                });
            }
            if (globalThis.DocumentFragment && globalThis.DocumentFragment.prototype && !globalThis.DocumentFragment.prototype[nativeProp]) {
                Object.defineProperty(globalThis.DocumentFragment.prototype, nativeProp, {
                    value: function() { return this[m].apply(this, arguments); },
                    writable: true,
                    configurable: true
                });
            }
        });
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
                nodeType: 1,
                tagName: 'DIV',
                id: 'engagement-panel-scrim',
                addEventListener: function() {},
                removeEventListener: function() {},
                dispatchEvent: function() { return true; },
                style: _createStyleObject(),
                classList: { add: function() {}, remove: function() {}, toggle: function() {}, contains: function() { return false; } },
                setAttribute: function() {},
                getAttribute: function() { return null; },
                removeAttribute: function() {}
            };
        }
        if (s.indexOf('info-skeleton') !== -1 || s.indexOf('meta-skeleton') !== -1 || s.indexOf('related-skeleton') !== -1) {
            var idName = s.indexOf('info-skeleton') !== -1 ? 'info-skeleton' : (s.indexOf('meta-skeleton') !== -1 ? 'meta-skeleton' : 'related-skeleton');
            return {
                nodeType: 1,
                tagName: 'DIV',
                id: idName,
                appendChild: function(c) { return c; },
                removeChild: function(c) { return c; },
                insertBefore: function(c) { return c; },
                addEventListener: function() {},
                removeEventListener: function() {},
                style: _createStyleObject()
            };
        }
        if (s.indexOf('button') !== -1) {
            return {
                nodeType: 1,
                tagName: 'DIV',
                id: 'button',
                children: [],
                childNodes: [],
                querySelector: function() { return null; },
                querySelectorAll: function() { return []; },
                addEventListener: function() {},
                removeEventListener: function() {},
                style: _createStyleObject()
            };
        }
        if (s === '#background' || s.indexOf('background') !== -1) {
            return {
                nodeType: 1,
                tagName: 'DIV',
                id: 'background',
                children: [],
                childNodes: [],
                querySelector: function() { return null; },
                querySelectorAll: function() { return []; },
                addEventListener: function() {},
                removeEventListener: function() {},
                dispatchEvent: function() { return true; },
                style: _createStyleObject(),
                classList: { add: function() {}, remove: function() {}, toggle: function() {}, contains: function() { return false; } },
                setAttribute: function() {},
                getAttribute: function() { return null; },
                removeAttribute: function() {}
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
                        if (typeof k.matches === 'function' ? k.matches(s) : _jsElementMatches(k, s)) {
                            list.push(k);
                        }
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
        var list = [];
        if (node._nodeId !== undefined && typeof _querySelectorAll === 'function') {
            var ids = _querySelectorAll(node._nodeId, t) || [];
            list = ids.map(_wrapElement);
        }
        if (list.length === 0) {
            var isWild = (t === '*');
            var tUpper = t.toUpperCase();
            function _searchTag(n) {
                var kids = n.childNodes || n.children || [];
                for (var i = 0; i < kids.length; i++) {
                    var k = kids[i];
                    if (k && k.nodeType === 1) {
                        if (isWild || (k.tagName && k.tagName.toUpperCase() === tUpper)) {
                            list.push(k);
                        }
                        _searchTag(k);
                    }
                }
            }
            _searchTag(node);
        }
        return _createHTMLCollection(function() { return list; });
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
                            if (!k.classList || !k.classList.contains(classes[c])) {
                                hasAll = false;
                                break;
                            }
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

    try {
        globalThis._defaultGetElementsByTagName = _defaultGetElementsByTagName;
        globalThis._defaultGetElementsByClassName = _defaultGetElementsByClassName;
        globalThis._defaultQuerySelector = _defaultQuerySelector;
        globalThis._defaultQuerySelectorAll = _defaultQuerySelectorAll;
    } catch (e) {}

