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

    function _isHostInclusiveAncestor(node, parent) {
        if (!node || !parent) return false;
        if (node === parent) return true;
        if (node._nodeId !== undefined && parent._nodeId !== undefined && node._nodeId === parent._nodeId) return true;
        if (node.contains && node.contains(parent)) return true;
        var cur = parent;
        var depth = 0;
        while (cur && depth++ < 1000) {
            if (cur === node) return true;
            if (cur._nodeId !== undefined && node._nodeId !== undefined && cur._nodeId === node._nodeId) return true;
            cur = cur.parentElement || cur.parentNode || cur.host;
        }
        return false;
    }
    globalThis._isHostInclusiveAncestor = _isHostInclusiveAncestor;

    function _isNodeLike(obj) {
        if (!obj || (typeof obj !== 'object' && typeof obj !== 'function')) return false;
        if (typeof Node !== 'undefined' && obj instanceof Node) return true;
        return obj.nodeType !== undefined || obj._nodeId !== undefined;
    }
    globalThis._isNodeLike = _isNodeLike;

    function _isChildOf(parent, child) {
        if (!parent || !child) return false;
        if (child.parentNode === parent || child._parentNode === parent) return true;
        if (parent._childNodes && parent._childNodes.indexOf(child) !== -1) return true;
        if (parent.childNodes) {
            for (var i = 0; i < parent.childNodes.length; i++) {
                if (parent.childNodes[i] === child) return true;
                if (parent.childNodes[i] && child && parent.childNodes[i]._nodeId !== undefined && child._nodeId !== undefined && parent.childNodes[i]._nodeId === child._nodeId) return true;
            }
        }
        if (parent._nodeId !== undefined && child._nodeId !== undefined && typeof _getParentNode === 'function') {
            var pId = _getParentNode(child._nodeId);
            if (pId !== -1 && pId === parent._nodeId) return true;
        }
        return false;
    }
    globalThis._isChildOf = _isChildOf;

    function _ensurePreInsertionValidity(parent, node, child) {
        if (!parent || typeof parent !== 'object') throw new DOMException("Parent is not a node.", "HierarchyRequestError");
        var pNt = parent.nodeType;
        if (pNt !== 1 && pNt !== 9 && pNt !== 11) throw new DOMException("Node cannot have children.", "HierarchyRequestError");
        if (!node || typeof node !== 'object') throw new DOMException("Invalid child node.", "HierarchyRequestError");
        var cNt = node.nodeType;
        if (cNt === undefined && node._nodeId !== undefined && typeof _getNodeType === 'function') {
            cNt = _getNodeType(node._nodeId);
        }
        if (_isHostInclusiveAncestor(node, parent)) throw new DOMException("New child is an ancestor of parent.", "HierarchyRequestError");
        if (cNt === 11) {
            var fKids = node.childNodes || node._childNodes || [];
            for (var i = 0; i < fKids.length; i++) {
                if (_isHostInclusiveAncestor(fKids[i], parent)) throw new DOMException("Fragment child is an ancestor of parent.", "HierarchyRequestError");
            }
        }
        if (child !== null && child !== undefined) {
            if (!_isChildOf(parent, child)) {
                throw new DOMException("The node before which the new node is to be inserted is not a child of this node.", "NotFoundError");
            }
        }
        if (cNt !== 1 && cNt !== 3 && cNt !== 4 && cNt !== 7 && cNt !== 8 && cNt !== 10 && cNt !== 11) {
            throw new DOMException("Invalid child node type.", "HierarchyRequestError");
        }
        if ((cNt === 3 || cNt === 4) && pNt === 9) throw new DOMException("Text nodes cannot be children of a Document.", "HierarchyRequestError");
        if (cNt === 10 && pNt !== 9) throw new DOMException("DocumentType cannot be child of non-Document.", "HierarchyRequestError");
        if (pNt === 9) {
            var docKids = parent.childNodes ? Array.prototype.slice.call(parent.childNodes) : (parent._childNodes ? Array.prototype.slice.call(parent._childNodes) : []);
            var hasDocElement = false, hasDocType = false;
            for (var di = 0; di < docKids.length; di++) {
                var dk = docKids[di];
                if (dk !== node && (node._nodeId === undefined || dk._nodeId !== node._nodeId)) {
                    if (dk.nodeType === 1) hasDocElement = true;
                    if (dk.nodeType === 10) hasDocType = true;
                }
            }
            if (cNt === 11) {
                var elemCount = 0, hasText = false;
                var fc = node.childNodes || node._childNodes || [];
                for (var j = 0; j < fc.length; j++) {
                    if (fc[j].nodeType === 1) elemCount++;
                    if (fc[j].nodeType === 3 || fc[j].nodeType === 4) hasText = true;
                }
                if (hasText || elemCount > 1) throw new DOMException("Invalid DocumentFragment for Document.", "HierarchyRequestError");
                if (elemCount === 1) {
                    if (hasDocElement || (child && child.nodeType === 10)) {
                        throw new DOMException("Document cannot have more than one element.", "HierarchyRequestError");
                    }
                    if (child) {
                        var foundC = false;
                        for (var k = 0; k < docKids.length; k++) {
                            if (docKids[k] === child || (docKids[k]._nodeId !== undefined && child._nodeId !== undefined && docKids[k]._nodeId === child._nodeId)) foundC = true;
                            else if (foundC && docKids[k].nodeType === 10) throw new DOMException("Element cannot precede doctype.", "HierarchyRequestError");
                        }
                    }
                }
            } else if (cNt === 1) {
                if (hasDocElement || (child && child.nodeType === 10)) {
                    throw new DOMException("Document cannot have more than one element.", "HierarchyRequestError");
                }
                if (child) {
                    var foundC2 = false;
                    for (var k = 0; k < docKids.length; k++) {
                        if (docKids[k] === child || (docKids[k]._nodeId !== undefined && child._nodeId !== undefined && docKids[k]._nodeId === child._nodeId)) foundC2 = true;
                        else if (foundC2 && docKids[k].nodeType === 10) throw new DOMException("Element cannot precede doctype.", "HierarchyRequestError");
                    }
                }
            } else if (cNt === 10) {
                if (hasDocType) throw new DOMException("Document cannot have more than one doctype.", "HierarchyRequestError");
                if (child) {
                    for (var k = 0; k < docKids.length; k++) {
                        if (docKids[k] === child || (docKids[k]._nodeId !== undefined && child._nodeId !== undefined && docKids[k]._nodeId === child._nodeId)) break;
                        if (docKids[k].nodeType === 1) throw new DOMException("Doctype cannot follow element.", "HierarchyRequestError");
                    }
                } else if (hasDocElement) {
                    throw new DOMException("Doctype cannot follow element.", "HierarchyRequestError");
                }
            }
        }
    }
    globalThis._ensurePreInsertionValidity = _ensurePreInsertionValidity;

    function _defaultAppendChild(node, child) {
        if (!_isNodeLike(child)) throw new TypeError("Failed to execute 'appendChild' on 'Node': parameter 1 is not of type 'Node'.");
        if (!node) return child;
        _ensurePreInsertionValidity(node, child, null);
        var hasId = typeof child === 'object' && child !== null && child._nodeId !== undefined;
        var childId = hasId ? child._nodeId : child;
        var childObj = hasId ? _wrapElement(childId) : child;
        var isFrag = childObj && childObj.nodeType === 11;
        var fragChildren = isFrag ? (childObj.childNodes ? Array.prototype.slice.call(childObj.childNodes) : (childObj._childNodes ? Array.prototype.slice.call(childObj._childNodes) : [])) : null;
        var insertList = isFrag && fragChildren ? fragChildren : [childObj];
        for (var k = 0; k < insertList.length; k++) {
            var item = insertList[k];
            if (item && typeof item === 'object') {
                if (item.parentNode && item.parentNode !== childObj && item.parentNode.removeChild) {
                    try { item.parentNode.removeChild(item); } catch(e) {}
                }
                item._parentNode = node;
                item.parentNode = node;
                var owner = (node.nodeType === 9) ? node : (node.ownerDocument || (typeof document !== 'undefined' ? document : null));
                if (owner) {
                    if (typeof _adoptSubtree === 'function') _adoptSubtree(item, owner);
                    else { item.ownerDocument = owner; item._ownerDocument = owner; }
                }
            }
        }
        if (child && typeof child === 'object') {
            var owner = (node.nodeType === 9) ? node : (node.ownerDocument || (typeof document !== 'undefined' ? document : null));
            if (owner) {
                if (typeof _adoptSubtree === 'function') _adoptSubtree(child, owner);
                else { child.ownerDocument = owner; child._ownerDocument = owner; }
            }
        }
        if (node._nodeId === undefined) {
            if (!node._childNodes) node._childNodes = [];
            for (var k = 0; k < insertList.length; k++) {
                node._childNodes.push(insertList[k]);
                if (node.nodeType === 9) {
                    if (insertList[k].nodeType === 1 && !node.documentElement) { node.documentElement = insertList[k]; node._documentElement = insertList[k]; }
                    if (insertList[k].nodeType === 10 && !node.doctype) { node.doctype = insertList[k]; node._doctype = insertList[k]; }
                }
            }
            if (isFrag && childObj._childNodes) childObj._childNodes = [];
            return child;
        }
        if (hasId) {
            _appendChild(node._nodeId, childId);
        } else if (node._childNodes) {
            for (var k = 0; k < insertList.length; k++) node._childNodes.push(insertList[k]);
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
        } else if (childObj && childObj.isConnected) {
            if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                globalThis._upgradeTree(childObj, null, null);
            }
            _invokeConnectedCallback(childObj);
            if (childObj.tagName && childObj.tagName.toLowerCase() === 'iframe') {
                setTimeout(function() {
                    var evt = (typeof Event === 'function') ? new Event('load') : { type: 'load' };
                    if (typeof childObj.onload === 'function') {
                        try { childObj.onload.call(childObj, evt); } catch(e) {}
                    }
                    if (typeof childObj.dispatchEvent === 'function') {
                        try { childObj.dispatchEvent(evt); } catch(e) {}
                    }
                }, 0);
            }
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
        if (!child || typeof child !== 'object' || (child.nodeType === undefined && child._nodeId === undefined)) {
            throw new TypeError("Failed to execute 'removeChild' on 'Node': parameter 1 is not of type 'Node'.");
        }
        if (!node || typeof node !== 'object') {
            throw new DOMException("Node cannot have children.", "NotFoundError");
        }
        var isChild = false;
        var hasId = child._nodeId !== undefined;
        var childId = hasId ? child._nodeId : child;
        var childObj = hasId ? _wrapElement(childId) : child;
        if (node._nodeId !== undefined && hasId && typeof _getParentNode === 'function') {
            var pId = _getParentNode(childId);
            if (pId !== -1 && pId === node._nodeId) isChild = true;
        }
        if (!isChild && node._childNodes && node._childNodes.indexOf(child) !== -1) isChild = true;
        if (!isChild && (child.parentNode === node || child._parentNode === node)) isChild = true;
        if (!isChild) {
            throw new DOMException("The node to be removed is not a child of this node.", "NotFoundError");
        }
        var removeIdx = -1;
        if (node._childNodes) {
            removeIdx = node._childNodes.indexOf(child);
        }
        if (removeIdx === -1 && typeof _getNodeIndex === 'function') {
            removeIdx = _getNodeIndex(child);
        }
        child._parentNode = null;
        child.parentNode = null;
        if (node._nodeId === undefined) {
            if (node._childNodes && removeIdx !== -1) node._childNodes.splice(removeIdx, 1);
            if (node.nodeType === 9) {
                if (node._documentElement === child || node.documentElement === child) {
                    node._documentElement = null; node.documentElement = null;
                }
                if (node._doctype === child || node.doctype === child) {
                    node._doctype = null; node.doctype = null;
                }
            }
        } else {
            if (hasId) {
                _removeChild(node._nodeId, childId);
            } else if (node._childNodes && removeIdx !== -1) {
                node._childNodes.splice(removeIdx, 1);
            }
        }
        if (removeIdx !== -1 && typeof globalThis !== 'undefined' && typeof globalThis._notifyRangeNodeRemoved === 'function') {
            globalThis._notifyRangeNodeRemoved(node, child, removeIdx);
        }
        if (childObj) _invokeDisconnectedCallback(childObj);
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: node,
                addedNodes: [],
                removedNodes: [childObj || child]
            });
        }
        return child;
    }

    function _defaultInsertBefore(node, newChild, refChild) {
        if (arguments.length < 3) throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required, but only " + (arguments.length - 1) + " present.");
        if (!_isNodeLike(newChild)) throw new TypeError("Failed to execute 'insertBefore' on 'Node': parameter 1 is not of type 'Node'.");
        if (refChild !== null && refChild !== undefined && !_isNodeLike(refChild)) throw new TypeError("Failed to execute 'insertBefore' on 'Node': parameter 2 is not of type 'Node'.");
        if (!node) return newChild;
        _ensurePreInsertionValidity(node, newChild, refChild);
        if (newChild === refChild || (newChild && refChild && newChild._nodeId !== undefined && newChild._nodeId === refChild._nodeId)) return newChild;
        var hasId = typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined;
        var childId = hasId ? newChild._nodeId : newChild;
        var refId = (typeof refChild === 'object' && refChild !== null && refChild._nodeId !== undefined) ? refChild._nodeId : refChild;
        var newObj = hasId ? _wrapElement(childId) : newChild;
        var isFrag = newObj && newObj.nodeType === 11;
        var fragChildren = isFrag ? (newObj.childNodes ? Array.prototype.slice.call(newObj.childNodes) : (newObj._childNodes ? Array.prototype.slice.call(newObj._childNodes) : [])) : null;
        var insertList = isFrag && fragChildren ? fragChildren : [newObj];
        for (var k = 0; k < insertList.length; k++) {
            var item = insertList[k];
            if (item && typeof item === 'object') {
                if (item.parentNode && item.parentNode !== newObj && item.parentNode.removeChild) {
                    try { item.parentNode.removeChild(item); } catch(e) {}
                }
                item._parentNode = node; item.parentNode = node;
                var owner = (node.nodeType === 9) ? node : (node.ownerDocument || (typeof document !== 'undefined' ? document : null));
                if (owner) {
                    if (typeof _adoptSubtree === 'function') _adoptSubtree(item, owner);
                    else { item.ownerDocument = owner; item._ownerDocument = owner; }
                }
            }
        }
        if (node._nodeId === undefined) {
            if (!node._childNodes) node._childNodes = [];
            var refIdx = refChild ? node._childNodes.indexOf(refChild) : -1;
            if (refIdx === -1) {
                for (var k = 0; k < insertList.length; k++) {
                    node._childNodes.push(insertList[k]);
                    if (node.nodeType === 9) {
                        if (insertList[k].nodeType === 1 && !node.documentElement) { node.documentElement = insertList[k]; node._documentElement = insertList[k]; }
                        if (insertList[k].nodeType === 10 && !node.doctype) { node.doctype = insertList[k]; node._doctype = insertList[k]; }
                    }
                }
            } else {
                for (var k = 0; k < insertList.length; k++) {
                    node._childNodes.splice(refIdx + k, 0, insertList[k]);
                    if (node.nodeType === 9) {
                        if (insertList[k].nodeType === 1 && !node.documentElement) { node.documentElement = insertList[k]; node._documentElement = insertList[k]; }
                        if (insertList[k].nodeType === 10 && !node.doctype) { node.doctype = insertList[k]; node._doctype = insertList[k]; }
                    }
                }
            }
            if (isFrag && newObj._childNodes) newObj._childNodes = [];
            return newChild;
        }
        _insertBefore(node._nodeId, childId, (refId === undefined || refId === null) ? 0 : refId);
        var refObj = (refId && refId !== 0) ? _wrapElement(refId) : null;
        if (isFrag && fragChildren) {
            for (var i = 0; i < fragChildren.length; i++) {
                var fc = fragChildren[i];
                if (fc && fc.isConnected) {
                    if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) globalThis._upgradeTree(fc, null, null);
                    _invokeConnectedCallback(fc);
                }
            }
        } else if (newObj && newObj.isConnected) {
            if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) globalThis._upgradeTree(newObj, null, null);
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
        if (arguments.length < 3) throw new TypeError("Failed to execute 'replaceChild' on 'Node': 2 arguments required, but only " + (arguments.length - 1) + " present.");
        if (!_isNodeLike(newChild)) throw new TypeError("Failed to execute 'replaceChild' on 'Node': parameter 1 is not of type 'Node'.");
        if (!_isNodeLike(oldChild)) throw new TypeError("Failed to execute 'replaceChild' on 'Node': parameter 2 is not of type 'Node'.");
        if (!node || typeof node !== 'object') throw new DOMException("Parent is not a node.", "HierarchyRequestError");
        var pNt = node.nodeType;
        if (pNt !== 1 && pNt !== 9 && pNt !== 11) throw new DOMException("Node cannot have children.", "HierarchyRequestError");
        var cNt = newChild.nodeType;
        if (cNt === undefined && newChild._nodeId !== undefined && typeof _getNodeType === 'function') {
            cNt = _getNodeType(newChild._nodeId);
        }
        if (_isHostInclusiveAncestor(newChild, node)) throw new DOMException("New child is an ancestor of parent.", "HierarchyRequestError");
        if (cNt === 11) {
            var fKids = newChild.childNodes || newChild._childNodes || [];
            for (var i = 0; i < fKids.length; i++) {
                if (_isHostInclusiveAncestor(fKids[i], node)) throw new DOMException("Fragment child is an ancestor of parent.", "HierarchyRequestError");
            }
        }
        if (!_isChildOf(node, oldChild)) throw new DOMException("The node to be replaced is not a child of this node.", "NotFoundError");
        if (cNt !== 1 && cNt !== 3 && cNt !== 4 && cNt !== 7 && cNt !== 8 && cNt !== 10 && cNt !== 11) {
            throw new DOMException("Invalid child node type.", "HierarchyRequestError");
        }
        if ((cNt === 3 || cNt === 4) && pNt === 9) throw new DOMException("Text nodes cannot be children of a Document.", "HierarchyRequestError");
        if (cNt === 10 && pNt !== 9) throw new DOMException("DocumentType cannot be child of non-Document.", "HierarchyRequestError");
        if (pNt === 9) {
            var docKids = node.childNodes ? Array.prototype.slice.call(node.childNodes) : (node._childNodes ? Array.prototype.slice.call(node._childNodes) : []);
            var hasOtherElement = false, hasOtherDoctype = false;
            for (var di = 0; di < docKids.length; di++) {
                if (docKids[di] !== oldChild && docKids[di] !== newChild) {
                    if (docKids[di].nodeType === 1) hasOtherElement = true;
                    if (docKids[di].nodeType === 10) hasOtherDoctype = true;
                }
            }
            if (cNt === 11) {
                var elemCount = 0, hasText = false;
                var fc = newChild.childNodes || newChild._childNodes || [];
                for (var j = 0; j < fc.length; j++) {
                    if (fc[j].nodeType === 1) elemCount++;
                    if (fc[j].nodeType === 3 || fc[j].nodeType === 4) hasText = true;
                }
                if (hasText || elemCount > 1) throw new DOMException("Invalid DocumentFragment for Document.", "HierarchyRequestError");
                if (elemCount === 1) {
                    if (hasOtherElement) throw new DOMException("Document cannot have more than one element.", "HierarchyRequestError");
                    var foundOld = false;
                    for (var k = 0; k < docKids.length; k++) {
                        if (docKids[k] === oldChild || (docKids[k]._nodeId !== undefined && oldChild._nodeId !== undefined && docKids[k]._nodeId === oldChild._nodeId)) foundOld = true;
                        else if (foundOld && docKids[k].nodeType === 10) throw new DOMException("Element cannot precede doctype.", "HierarchyRequestError");
                    }
                }
            } else if (cNt === 1) {
                if (hasOtherElement) throw new DOMException("Document cannot have more than one element.", "HierarchyRequestError");
                var foundOld2 = false;
                for (var k = 0; k < docKids.length; k++) {
                    if (docKids[k] === oldChild || (docKids[k]._nodeId !== undefined && oldChild._nodeId !== undefined && docKids[k]._nodeId === oldChild._nodeId)) foundOld2 = true;
                    else if (foundOld2 && docKids[k].nodeType === 10) throw new DOMException("Element cannot precede doctype.", "HierarchyRequestError");
                }
            } else if (cNt === 10) {
                if (hasOtherDoctype) throw new DOMException("Document cannot have more than one doctype.", "HierarchyRequestError");
                for (var k = 0; k < docKids.length; k++) {
                    if (docKids[k] === oldChild || (docKids[k]._nodeId !== undefined && oldChild._nodeId !== undefined && docKids[k]._nodeId === oldChild._nodeId)) break;
                    if (docKids[k].nodeType === 1) throw new DOMException("Doctype cannot follow element.", "HierarchyRequestError");
                }
            }
        }
        if (newChild === oldChild) return oldChild;
        var hasNewId = typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined;
        var hasOldId = typeof oldChild === 'object' && oldChild !== null && oldChild._nodeId !== undefined;
        var newId = hasNewId ? newChild._nodeId : newChild;
        var oldId = hasOldId ? oldChild._nodeId : oldChild;
        var oldObj = hasOldId ? _wrapElement(oldId) : oldChild;
        var newObj = hasNewId ? _wrapElement(newId) : newChild;
        var isFrag = newObj && newObj.nodeType === 11;
        var fragChildren = isFrag ? (newObj.childNodes ? Array.prototype.slice.call(newObj.childNodes) : (newObj._childNodes ? Array.prototype.slice.call(newObj._childNodes) : [])) : null;
        var insertList = isFrag && fragChildren ? fragChildren : [newObj];
        for (var k = 0; k < insertList.length; k++) {
            var item = insertList[k];
            if (item && typeof item === 'object') {
                if (item.parentNode && item.parentNode !== newObj && item.parentNode.removeChild) {
                    try { item.parentNode.removeChild(item); } catch(e) {}
                }
                item._parentNode = node; item.parentNode = node;
                var owner = (node.nodeType === 9) ? node : (node.ownerDocument || (typeof document !== 'undefined' ? document : null));
                if (owner) { item.ownerDocument = owner; item._ownerDocument = owner; }
            }
        }
        if (node._nodeId === undefined) {
            if (!node._childNodes) node._childNodes = [];
            var idx = node._childNodes.indexOf(oldChild);
            if (idx !== -1) {
                node._childNodes.splice(idx, 1);
                for (var k = 0; k < insertList.length; k++) {
                    node._childNodes.splice(idx + k, 0, insertList[k]);
                }
            } else {
                for (var k = 0; k < insertList.length; k++) node._childNodes.push(insertList[k]);
            }
            if (node.nodeType === 9) {
                if (node._documentElement === oldChild || node.documentElement === oldChild) {
                    node._documentElement = (newObj.nodeType === 1) ? newObj : null;
                    node.documentElement = node._documentElement;
                }
                if (node._doctype === oldChild || node.doctype === oldChild) {
                    node._doctype = (newObj.nodeType === 10) ? newObj : null;
                    node.doctype = node._doctype;
                }
            }
            oldChild.parentNode = null; oldChild._parentNode = null;
            if (isFrag && newObj._childNodes) newObj._childNodes = [];
            return oldChild;
        }
        _replaceChild(node._nodeId, newId, oldId);
        if (oldChild) { oldChild.parentNode = null; oldChild._parentNode = null; }
        if (oldObj) _invokeDisconnectedCallback(oldObj);
        if (isFrag && fragChildren) {
            for (var i = 0; i < fragChildren.length; i++) {
                var fc = fragChildren[i];
                if (fc && fc.isConnected) {
                    if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) globalThis._upgradeTree(fc, null, null);
                    _invokeConnectedCallback(fc);
                }
            }
        } else if (newObj && newObj.isConnected) {
            if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) globalThis._upgradeTree(newObj, null, null);
            _invokeConnectedCallback(newObj);
        }
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: node,
                addedNodes: isFrag && fragChildren ? fragChildren : [newObj],
                removedNodes: [oldObj || oldChild]
            });
        }
        return oldChild;
    }

    function _defaultGetTextContent(node) {
        if (!node || typeof node !== 'object') return null;
        var nt = node.nodeType;
        if (nt === undefined && node._nodeId !== undefined && typeof _getNodeType === 'function') nt = _getNodeType(node._nodeId);
        if (nt === 9 || nt === 10) return null;
        if (nt === 3 || nt === 4 || nt === 7 || nt === 8) {
            if (node.data !== undefined && node.data !== null) return node.data;
            if (node._data !== undefined && node._data !== null) return node._data;
            if (node._nodeValue !== undefined && node._nodeValue !== null) return node._nodeValue;
            if (node._nodeId !== undefined && typeof _getNodeValue === 'function') {
                var v = _getNodeValue(node._nodeId);
                if (v !== null && v !== undefined) return v;
            }
            return "";
        }
        if (nt === 2) return (node.value !== undefined && node.value !== null) ? node.value : "";
        if (nt === 1 || nt === 11) {
            if (node.tagName && node.tagName.toLowerCase() === 'template') return "";
            var kids = node.childNodes || node._childNodes;
            if (kids && kids.length > 0) {
                var out = "";
                for (var i = 0; i < kids.length; i++) {
                    var k = kids[i];
                    if (!k) continue;
                    var knt = k.nodeType !== undefined ? k.nodeType : (k._nodeId !== undefined && typeof _getNodeType === 'function' ? _getNodeType(k._nodeId) : undefined);
                    if (knt === 3 || knt === 4) {
                        if (k.data !== undefined && k.data !== null) out += k.data;
                        else if (k._data !== undefined && k._data !== null) out += k._data;
                        else if (k._nodeValue !== undefined && k._nodeValue !== null) out += k._nodeValue;
                        else if (k._nodeId !== undefined && typeof _getNodeValue === 'function') {
                            var kv = _getNodeValue(k._nodeId);
                            if (kv !== null && kv !== undefined) out += kv;
                        }
                    } else if (knt === 1) {
                        out += _defaultGetTextContent(k);
                    }
                }
                return out;
            }
            if (node._nodeId !== undefined && typeof _getTextContent === 'function') {
                var txt = _getTextContent(node._nodeId);
                return (txt !== null && txt !== undefined) ? txt : "";
            }
            return "";
        }
        return null;
    }

    function _defaultSetTextContent(node, val) {
        if (!node || typeof node !== 'object') return;
        var nt = node.nodeType;
        if (nt === undefined && node._nodeId !== undefined && typeof _getNodeType === 'function') nt = _getNodeType(node._nodeId);
        if (nt === 9 || nt === 10) return;
        if (nt === 3 || nt === 4 || nt === 7 || nt === 8) {
            var strVal = (val === null || val === undefined) ? "" : String(val);
            var oldVal = _defaultGetTextContent(node);
            node.data = strVal; node._data = strVal; node._nodeValue = strVal;
            if (node._nodeId !== undefined && typeof _setNodeValue === 'function') _setNodeValue(node._nodeId, strVal);
            if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                globalThis._reportDOMMutation({ type: 'characterData', target: node, oldValue: oldVal });
            }
            return;
        }
        if (nt === 2) {
            node.value = (val === null || val === undefined) ? "" : String(val);
            return;
        }
        if (nt === 1 || nt === 11) {
            var strVal = (val === null || val === undefined) ? "" : String(val);
            var oldKids = node.childNodes ? Array.prototype.slice.call(node.childNodes) : (node._childNodes ? Array.prototype.slice.call(node._childNodes) : []);
            for (var i = 0; i < oldKids.length; i++) {
                var k = oldKids[i];
                if (k) {
                    k.parentNode = null; k._parentNode = null;
                    _invokeDisconnectedCallback(k); _clearSubtreeWrapperCache(k);
                }
            }
            if (node._childNodes) node._childNodes = [];
            if (node._nodeId !== undefined && typeof _setTextContent === 'function') _setTextContent(node._nodeId, strVal);
            if (strVal !== "" && node._nodeId === undefined) {
                var textNode = (typeof document !== 'undefined' && document.createTextNode)
                    ? document.createTextNode(strVal)
                    : { nodeType: 3, data: strVal, nodeValue: strVal, textContent: strVal, parentNode: node, _parentNode: node };
                textNode.parentNode = node; textNode._parentNode = node;
                if (!node._childNodes) node._childNodes = [];
                node._childNodes.push(textNode);
            }
            if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                globalThis._reportDOMMutation({
                    type: 'childList',
                    target: node,
                    addedNodes: (strVal !== "" && node.firstChild) ? [node.firstChild] : [],
                    removedNodes: oldKids
                });
            }
            return;
        }
    }

    globalThis._defaultAppendChild = _defaultAppendChild; globalThis._defaultRemoveChild = _defaultRemoveChild;
    globalThis._defaultInsertBefore = _defaultInsertBefore; globalThis._defaultReplaceChild = _defaultReplaceChild;
    globalThis._defaultGetTextContent = _defaultGetTextContent; globalThis._defaultSetTextContent = _defaultSetTextContent;
    if (typeof globalThis !== 'undefined' && globalThis.Node && globalThis.Node.prototype) {
        globalThis.Node.prototype.appendChild = function(child) {
            if (arguments.length < 1) throw new TypeError("Failed to execute 'appendChild' on 'Node': 1 argument required");
            return _defaultAppendChild(this, child);
        };
        globalThis.Node.prototype.removeChild = function(child) { return _defaultRemoveChild(this, child); };
        globalThis.Node.prototype.insertBefore = function(newChild, refChild) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required");
            return _defaultInsertBefore(this, newChild, refChild);
        };
        globalThis.Node.prototype.moveBefore = function(node, child) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'moveBefore' on 'Node': 2 arguments required");
            return _defaultInsertBefore(this, node, child);
        };
        globalThis.Node.prototype.replaceChild = function(newChild, oldChild) {
            if (arguments.length < 2) throw new TypeError("Failed to execute 'replaceChild' on 'Node': 2 arguments required");
            return _defaultReplaceChild(this, newChild, oldChild);
        };
        globalThis.Node.prototype.cloneNode = function(deep) {
            if (this && this._nodeId !== undefined && typeof _cloneNode === 'function') {
                var cloneId = _cloneNode(this._nodeId, !!deep); return cloneId ? _wrapElement(cloneId) : null;
            }
            return this;
        };
        ['appendChild', 'insertBefore', 'moveBefore', 'removeChild', 'replaceChild', 'cloneNode', 'contains'].forEach(function(m) {
            var np = '__shady_native_' + m;
            [globalThis.Node, globalThis.Element, globalThis.Document, globalThis.DocumentFragment].forEach(function(cls) {
                if (cls && cls.prototype && !cls.prototype[np]) {
                    Object.defineProperty(cls.prototype, np, { value: function() { return this[m].apply(this, arguments); }, writable: true, configurable: true });
                }
            });
        });
    }


