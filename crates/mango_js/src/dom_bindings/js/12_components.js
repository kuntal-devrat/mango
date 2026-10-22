    // W3C View Transitions API Level 1 & 2
    function ViewTransition(updateCallback) {
        var self = this;
        this.types = new Set();
        this._skipped = false;

        var updateDoneResolve, updateDoneReject;
        this.updateCallbackDone = new Promise(function(resolve, reject) {
            updateDoneResolve = resolve;
            updateDoneReject = reject;
        });

        var readyResolve, readyReject;
        this.ready = new Promise(function(resolve, reject) {
            readyResolve = resolve;
            readyReject = reject;
        });

        var finishedResolve, finishedReject;
        this.finished = new Promise(function(resolve, reject) {
            finishedResolve = resolve;
            finishedReject = reject;
        });

        this.skipTransition = function() {
            self._skipped = true;
            try {
                readyReject(new Error("Transition was skipped"));
            } catch(e) {}
            finishedResolve();
        };

        Promise.resolve().then(function() {
            if (typeof updateCallback === 'function') {
                return Promise.resolve(updateCallback());
            }
        }).then(function(result) {
            updateDoneResolve(result);
            if (!self._skipped) {
                readyResolve();
                finishedResolve();
            }
        }).catch(function(err) {
            updateDoneReject(err);
            readyReject(err);
            finishedReject(err);
        });
    }

    ViewTransition.prototype[Symbol.toStringTag] = 'ViewTransition';
    try { globalThis.ViewTransition = ViewTransition; } catch(e) {}

    document.startViewTransition = function(updateCallback) {
        return new ViewTransition(updateCallback);
    };
    Object.defineProperty(document, 'forms', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('forms') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'images', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('images') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'links', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('links') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'anchors', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('anchors') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'scripts', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('scripts') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'embeds', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getDocumentElements('embeds') || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'children', {
        get: function() {
            return _createHTMLCollection(function() {
                return (_getChildren(0) || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'childNodes', {
        get: function() {
            return _createNodeList(function() {
                return (_getChildNodeIds(0) || []).map(_wrapElement);
            });
        }
    });
    Object.defineProperty(document, 'firstChild', {
        get: function() {
            var id = _getFirstChild(0);
            return id !== null ? _wrapElement(id) : null;
        }
    });
    Object.defineProperty(document, 'lastChild', {
        get: function() {
            var id = _getLastChild(0);
            return id !== null ? _wrapElement(id) : null;
        }
    });
    Object.defineProperty(document, 'childElementCount', {
        get: function() {
            return (_getChildren(0) || []).length;
        }
    });
    document.getElementsByName = function(name) {
        return _createNodeList(function() {
            var ids = _querySelectorAll(0, '[name="' + String(name) + '"]') || [];
            return ids.map(_wrapElement);
        });
    };
    document.appendChild = function(child) {
        if (typeof _ensurePreInsertionValidity === 'function') _ensurePreInsertionValidity(document, child, null);
        var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
        var childObj = _wrapElement(childId);
        var isFrag = childObj && childObj.nodeType === 11;
        var fragChildren = isFrag ? (childObj.childNodes ? Array.prototype.slice.call(childObj.childNodes) : []) : null;
        _appendChild(0, childId);
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
                target: document,
                addedNodes: isFrag && fragChildren ? fragChildren : [childObj],
                removedNodes: []
            });
        }
        return child;
    };
    document.removeChild = function(child) {
        var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
        var childObj = _wrapElement(childId);
        _removeChild(0, childId);
        if (childObj) {
            _invokeDisconnectedCallback(childObj);
        }
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [],
                removedNodes: [childObj]
            });
        }
        return child;
    };
    document.insertBefore = function(newChild, refChild) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required");
        return _defaultInsertBefore(document, newChild, refChild);
    };
    document.replaceChild = function(newChild, oldChild) {
        if (arguments.length < 2) throw new TypeError("Failed to execute 'replaceChild' on 'Node': 2 arguments required");
        return _defaultReplaceChild(document, newChild, oldChild);
    };
    document.cloneNode = function(deep) {
        var cloneId = _cloneNode(0, !!deep);
        var cloned = _wrapElement(cloneId);
        if (cloned && typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
            globalThis._upgradeTree(cloned, null, null);
        }
        return cloned;
    };
    document.ownerDocument = null;
    document.getRootNode = function(options) {
        return document;
    };
    document.elementFromPoint = function(x, y) {
        var id = _elementFromPoint(Number(x), Number(y));
        return id === null ? null : _wrapElement(id);
    };
    document.elementsFromPoint = function(x, y) {
        if (typeof _elementsFromPoint === 'function') {
            var ids = _elementsFromPoint(Number(x), Number(y));
            if (ids && ids.length > 0) {
                return ids.map(_wrapElement);
            }
        }
        var el = document.elementFromPoint(x, y);
        return el === null ? [] : [el];
    };
    var _docActiveElement = null;
    Object.defineProperty(document, 'activeElement', {
        get: function() { return _docActiveElement || document.body || null; },
        set: function(el) { _docActiveElement = el; },
        configurable: true
    });
    document.hasChildNodes = function() { return (_getChildNodeIds(0) || []).length > 0; };
    document.contains = function(other) {
        if (!other) return false;
        if (other === this) return true;
        if (other._nodeId !== undefined && typeof _elementContains === 'function') {
            return _elementContains(0, other._nodeId);
        }
        var p = other.parentNode;
        while (p) {
            if (p === this) return true;
            p = p.parentNode;
        }
        return false;
    };
    document.compareDocumentPosition = function(other) {
        if (typeof _compareDocumentPositionNodes === 'function') {
            return _compareDocumentPositionNodes(this, other);
        }
        if (this === other) return 0;
        return 1 | 32 | 4;
    };

    document.append = function() {
        if (arguments.length === 0) return;
        var nodes = [];
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            nodes.push((arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined)) ? arg : _wrapElement(_origCTN(String(arg))));
        }
        if (nodes.length === 1) {
            document.appendChild(nodes[0]);
        } else {
            if (typeof _ensurePreInsertionValidity === 'function') {
                for (var i = 0; i < nodes.length; i++) _ensurePreInsertionValidity(document, nodes[i], null);
            }
            var frag = _wrapElement(_createFragment());
            for (var i = 0; i < nodes.length; i++) frag.appendChild(nodes[i]);
            document.appendChild(frag);
        }
    };
    document.prepend = function() {
        if (arguments.length === 0) return;
        var nodes = [];
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            nodes.push((arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined)) ? arg : _wrapElement(_origCTN(String(arg))));
        }
        var first = document.firstChild;
        if (nodes.length === 1) {
            if (first) document.insertBefore(nodes[0], first);
            else document.appendChild(nodes[0]);
        } else {
            if (typeof _ensurePreInsertionValidity === 'function') {
                for (var i = 0; i < nodes.length; i++) _ensurePreInsertionValidity(document, nodes[i], first);
            }
            var frag = _wrapElement(_createFragment());
            for (var i = 0; i < nodes.length; i++) frag.appendChild(nodes[i]);
            if (first) document.insertBefore(frag, first);
            else document.appendChild(frag);
        }
    };
    function _doReplaceChildren(target, args, isDoc) {
        if (!target) return;
        var isDocument = isDoc || target.nodeType === 9;
        var pNt = target.nodeType;
        if (!isDocument && pNt !== 1 && pNt !== 11) {
            throw new DOMException("Target cannot have children.", "HierarchyRequestError");
        }

        // 1. Convert arguments to nodes
        var rawNodes = [];
        for (var i = 0; i < args.length; i++) {
            var arg = args[i];
            if (arg !== null && typeof arg === 'object' && (arg.nodeType !== undefined || arg._nodeId !== undefined)) {
                rawNodes.push(arg);
            } else {
                var textStr = String(arg);
                var tn = (typeof _origCTN === 'function')
                    ? _wrapElement(_origCTN(textStr))
                    : ((typeof document !== 'undefined' && document.createTextNode)
                        ? document.createTextNode(textStr)
                        : { nodeType: 3, nodeValue: textStr, textContent: textStr });
                rawNodes.push(tn);
            }
        }

        // 2. Flatten fragments to determine incoming items
        var nodesToInsert = [];
        for (var i = 0; i < rawNodes.length; i++) {
            var n = rawNodes[i];
            var nt = n.nodeType;
            if (nt === undefined && n._nodeId !== undefined && typeof _getNodeType === 'function') {
                nt = _getNodeType(n._nodeId);
            }
            if (nt === 11) {
                var fKids = n.childNodes ? Array.prototype.slice.call(n.childNodes) : (n._childNodes ? Array.prototype.slice.call(n._childNodes) : []);
                for (var j = 0; j < fKids.length; j++) {
                    nodesToInsert.push(fKids[j]);
                }
            } else {
                nodesToInsert.push(n);
            }
        }

        // 3. Pre-insertion validity validation BEFORE modifying target
        var elemCount = 0;
        var doctypeCount = 0;
        var hasSeenElement = false;

        for (var i = 0; i < nodesToInsert.length; i++) {
            var node = nodesToInsert[i];
            var nt = node.nodeType;
            if (nt === undefined && node._nodeId !== undefined && typeof _getNodeType === 'function') {
                nt = _getNodeType(node._nodeId);
            }

            if (nt !== 1 && nt !== 3 && nt !== 7 && nt !== 8 && nt !== 10 && nt !== 11) {
                throw new DOMException("Invalid child node type.", "HierarchyRequestError");
            }

            if (typeof _isHostInclusiveAncestor === 'function' && _isHostInclusiveAncestor(node, target)) {
                throw new DOMException("New child is an ancestor of target.", "HierarchyRequestError");
            }

            if (isDocument) {
                if (nt === 3) {
                    throw new DOMException("Text nodes cannot be children of a Document.", "HierarchyRequestError");
                }
                if (nt === 1) {
                    elemCount++;
                    hasSeenElement = true;
                    if (elemCount > 1) {
                        throw new DOMException("Document cannot have more than one element.", "HierarchyRequestError");
                    }
                } else if (nt === 10) {
                    doctypeCount++;
                    if (doctypeCount > 1) {
                        throw new DOMException("Document cannot have more than one doctype.", "HierarchyRequestError");
                    }
                    if (hasSeenElement) {
                        throw new DOMException("Doctype cannot follow element.", "HierarchyRequestError");
                    }
                }
            } else {
                if (nt === 10) {
                    throw new DOMException("DocumentType cannot be child of non-Document.", "HierarchyRequestError");
                }
            }
        }

        if (isDocument) {
            for (var i = 0; i < rawNodes.length; i++) {
                var rn = rawNodes[i];
                if (rn.nodeType === 11) {
                    var fKids = rn.childNodes ? Array.prototype.slice.call(rn.childNodes) : (rn._childNodes ? Array.prototype.slice.call(rn._childNodes) : []);
                    for (var j = 0; j < fKids.length; j++) {
                        if (fKids[j].nodeType === 3) {
                            throw new DOMException("Text nodes cannot be children of a Document.", "HierarchyRequestError");
                        }
                    }
                }
            }
        }

        // 4. Detach incoming nodes from their previous parents
        for (var i = 0; i < rawNodes.length; i++) {
            var n = rawNodes[i];
            if (n.nodeType === 11) {
                var fKids = n.childNodes ? Array.prototype.slice.call(n.childNodes) : (n._childNodes ? Array.prototype.slice.call(n._childNodes) : []);
                for (var j = 0; j < fKids.length; j++) {
                    var fk = fKids[j];
                    var prevP = fk.parentElement || fk.parentNode || fk._parentNode;
                    if (prevP && prevP !== n && prevP !== target) {
                        if (prevP.removeChild) {
                            try { prevP.removeChild(fk); } catch(e) {}
                        }
                    }
                }
                if (n._childNodes) n._childNodes = [];
            } else {
                var prevP = n.parentElement || n.parentNode || n._parentNode;
                if (prevP && prevP !== target) {
                    if (prevP.removeChild) {
                        try { prevP.removeChild(n); } catch(e) {}
                    }
                }
            }
        }

        // 5. Collect old children of target
        var oldKids = [];
        if (target.childNodes) {
            oldKids = Array.prototype.slice.call(target.childNodes);
        } else if (target._childNodes) {
            oldKids = Array.prototype.slice.call(target._childNodes);
        }

        if (oldKids.length === 0 && nodesToInsert.length === 0) {
            return;
        }

        // 6. Clear target children (without individual childList mutation records)
        for (var i = 0; i < oldKids.length; i++) {
            var oldKid = oldKids[i];
            if (!oldKid) continue;
            if (target._nodeId !== undefined && oldKid._nodeId !== undefined && typeof _removeChild === 'function') {
                try { _removeChild(target._nodeId, oldKid._nodeId); } catch(e) {}
            }
            oldKid._parentNode = null;
            oldKid.parentNode = null;
            if (typeof _invokeDisconnectedCallback === 'function') {
                _invokeDisconnectedCallback(oldKid);
            }
        }
        if (target._childNodes) target._childNodes = [];
        if (isDocument) {
            target._documentElement = null;
            target.documentElement = null;
            target._doctype = null;
            target.doctype = null;
        }

        // 7. Insert new children into target
        var addedNodes = [];
        for (var i = 0; i < nodesToInsert.length; i++) {
            var newKid = nodesToInsert[i];
            if (!newKid) continue;
            addedNodes.push(newKid);
            if (typeof newKid === 'object') {
                newKid._parentNode = target;
                newKid.parentNode = target;
                newKid._ownerDocument = isDocument ? target : (target.ownerDocument || document);
                newKid.ownerDocument = newKid._ownerDocument;
            }
            if (target._nodeId !== undefined && newKid._nodeId !== undefined && typeof _appendChild === 'function') {
                try { _appendChild(target._nodeId, newKid._nodeId); } catch(e) {}
            }
            if (target._childNodes) {
                target._childNodes.push(newKid);
            }
            if (isDocument) {
                if (newKid.nodeType === 1 && !target.documentElement) {
                    target._documentElement = newKid;
                    target.documentElement = newKid;
                } else if (newKid.nodeType === 10 && !target.doctype) {
                    target._doctype = newKid;
                    target.doctype = newKid;
                }
            }
            if (newKid.isConnected) {
                if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                    globalThis._upgradeTree(newKid, null, null);
                }
                if (typeof _invokeConnectedCallback === 'function') {
                    _invokeConnectedCallback(newKid);
                }
            }
        }

        // 8. Queue a SINGLE childList mutation record on target
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: target,
                addedNodes: addedNodes,
                removedNodes: oldKids,
                previousSibling: null,
                nextSibling: null
            });
        }
    }
    globalThis._doReplaceChildren = _doReplaceChildren;

    document.replaceChildren = function() {
        _doReplaceChildren(document, arguments, true);
    };

