    // ── 15. DOM Traversal (TreeWalker, NodeIterator, NodeFilter) ───────────────

    (function() {
    var NodeFilter = function NodeFilter() {
        throw new TypeError("Illegal constructor");
    };
    var _filterConsts = {
        FILTER_ACCEPT: 1,
        FILTER_REJECT: 2,
        FILTER_SKIP: 3,
        SHOW_ALL: 0xFFFFFFFF,
        SHOW_ELEMENT: 0x1,
        SHOW_ATTRIBUTE: 0x2,
        SHOW_TEXT: 0x4,
        SHOW_CDATA_SECTION: 0x8,
        SHOW_ENTITY_REFERENCE: 0x10,
        SHOW_ENTITY: 0x20,
        SHOW_PROCESSING_INSTRUCTION: 0x40,
        SHOW_COMMENT: 0x80,
        SHOW_DOCUMENT: 0x100,
        SHOW_DOCUMENT_TYPE: 0x200,
        SHOW_DOCUMENT_FRAGMENT: 0x400,
        SHOW_NOTATION: 0x800
    };
    Object.assign(NodeFilter, _filterConsts);
    NodeFilter.prototype = Object.create(Object.prototype);
    Object.assign(NodeFilter.prototype, _filterConsts);

    function _filterTestNode(whatToShow, filter, node) {
        if (!node) return 2; // FILTER_REJECT
        var mask = 1 << ((node.nodeType || 0) - 1);
        if ((whatToShow & mask) === 0) {
            return 3; // FILTER_SKIP
        }
        if (!filter) return 1; // FILTER_ACCEPT
        try {
            var res = 1;
            if (typeof filter === 'function') {
                res = filter(node);
            } else if (filter && typeof filter.acceptNode === 'function') {
                res = filter.acceptNode(node);
            }
            return Number(res);
        } catch(e) {
            throw e;
        }
    }

    var TreeWalker = function TreeWalker() {
        throw new TypeError("Illegal constructor");
    };
    TreeWalker.prototype = Object.create(Object.prototype);
    TreeWalker.prototype.constructor = TreeWalker;

    Object.defineProperty(TreeWalker.prototype, 'root', {
        get: function() { return this._root; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(TreeWalker.prototype, 'whatToShow', {
        get: function() { return this._whatToShow; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(TreeWalker.prototype, 'filter', {
        get: function() { return this._filter; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(TreeWalker.prototype, 'currentNode', {
        get: function() { return this._currentNode; },
        set: function(val) {
            if (!val || typeof val !== 'object') throw new TypeError("Failed to set 'currentNode': parameter 1 is not of type 'Node'");
            this._currentNode = val;
        },
        enumerable: true, configurable: true
    });

    TreeWalker.prototype.parentNode = function() {
        var node = this._currentNode;
        while (node && node !== this._root) {
            node = node.parentNode;
            if (node) {
                var f = _filterTestNode(this._whatToShow, this._filter, node);
                if (f === 1) { // FILTER_ACCEPT
                    this._currentNode = node;
                    return node;
                }
            }
        }
        return null;
    };

    function _getFirstChild(node) {
        if (!node) return null;
        if (node.firstChild) return node.firstChild;
        if (node.childNodes && node.childNodes.length > 0) return node.childNodes[0];
        return null;
    }
    function _getLastChild(node) {
        if (!node) return null;
        if (node.lastChild) return node.lastChild;
        if (node.childNodes && node.childNodes.length > 0) return node.childNodes[node.childNodes.length - 1];
        return null;
    }

    TreeWalker.prototype.firstChild = function() {
        var node = this._currentNode;
        var child = _getFirstChild(node);
        while (child) {
            var f = _filterTestNode(this._whatToShow, this._filter, child);
            if (f === 1) {
                this._currentNode = child;
                return child;
            }
            if (f === 3) { // FILTER_SKIP: try descendants
                var sub = _getFirstChild(child);
                if (sub) {
                    child = sub;
                    continue;
                }
            }
            while (child) {
                if (child.nextSibling) {
                    child = child.nextSibling;
                    break;
                }
                var p = child.parentNode;
                if (!p || p === node || p === this._root) {
                    child = null;
                    break;
                }
                child = p;
            }
        }
        return null;
    };

    TreeWalker.prototype.lastChild = function() {
        var node = this._currentNode;
        var child = _getLastChild(node);
        while (child) {
            var f = _filterTestNode(this._whatToShow, this._filter, child);
            if (f === 1) {
                this._currentNode = child;
                return child;
            }
            if (f === 3) {
                var sub = _getLastChild(child);
                if (sub) {
                    child = sub;
                    continue;
                }
            }
            while (child) {
                if (child.previousSibling) {
                    child = child.previousSibling;
                    break;
                }
                var p = child.parentNode;
                if (!p || p === node || p === this._root) {
                    child = null;
                    break;
                }
                child = p;
            }
        }
        return null;
    };

    TreeWalker.prototype.previousSibling = function() {
        var node = this._currentNode;
        if (node === this._root) return null;
        while (node) {
            var sib = node.previousSibling;
            while (sib) {
                var f = _filterTestNode(this._whatToShow, this._filter, sib);
                if (f === 1) {
                    this._currentNode = sib;
                    return sib;
                }
                var sub = (f === 3) ? _getLastChild(sib) : null;
                if (sub) {
                    sib = sub;
                } else {
                    sib = sib.previousSibling;
                }
            }
            node = node.parentNode;
            if (!node || node === this._root) return null;
            if (_filterTestNode(this._whatToShow, this._filter, node) === 1) return null;
        }
        return null;
    };

    TreeWalker.prototype.nextSibling = function() {
        var node = this._currentNode;
        if (node === this._root) return null;
        while (node) {
            var sib = node.nextSibling;
            while (sib) {
                var f = _filterTestNode(this._whatToShow, this._filter, sib);
                if (f === 1) {
                    this._currentNode = sib;
                    return sib;
                }
                var sub = (f === 3) ? _getFirstChild(sib) : null;
                if (sub) {
                    sib = sub;
                } else {
                    sib = sib.nextSibling;
                }
            }
            node = node.parentNode;
            if (!node || node === this._root) return null;
            if (_filterTestNode(this._whatToShow, this._filter, node) === 1) return null;
        }
        return null;
    };

    TreeWalker.prototype.previousNode = function() {
        var node = this._currentNode;
        while (node !== this._root) {
            var sib = node.previousSibling;
            while (sib) {
                node = sib;
                var f = _filterTestNode(this._whatToShow, this._filter, node);
                while (f !== 2 && _getLastChild(node)) {
                    node = _getLastChild(node);
                    f = _filterTestNode(this._whatToShow, this._filter, node);
                }
                if (f === 1) {
                    this._currentNode = node;
                    return node;
                }
                sib = sib.previousSibling;
            }
            if (node === this._root || !node.parentNode) return null;
            node = node.parentNode;
            if (_filterTestNode(this._whatToShow, this._filter, node) === 1) {
                this._currentNode = node;
                return node;
            }
        }
        return null;
    };

    TreeWalker.prototype.nextNode = function() {
        var node = this._currentNode;
        var result = 1;
        while (true) {
            while (result !== 2 && _getFirstChild(node)) {
                node = _getFirstChild(node);
                result = _filterTestNode(this._whatToShow, this._filter, node);
                if (result === 1) {
                    this._currentNode = node;
                    return node;
                }
            }
            var temp = node;
            var sib = null;
            while (temp) {
                if (temp === this._root) return null;
                if (temp.nextSibling) {
                    sib = temp.nextSibling;
                    break;
                }
                temp = temp.parentNode;
            }
            if (!sib) return null;
            node = sib;
            result = _filterTestNode(this._whatToShow, this._filter, node);
            if (result === 1) {
                this._currentNode = node;
                return node;
            }
        }
    };

    // NodeIterator
    var _activeNodeIterators = [];

    var NodeIterator = function NodeIterator() {
        throw new TypeError("Illegal constructor");
    };
    NodeIterator.prototype = Object.create(Object.prototype);
    NodeIterator.prototype.constructor = NodeIterator;

    Object.defineProperty(NodeIterator.prototype, 'root', {
        get: function() { return this._root; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(NodeIterator.prototype, 'referenceNode', {
        get: function() { return this._reference; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(NodeIterator.prototype, 'pointerBeforeReferenceNode', {
        get: function() { return this._pointerBeforeReference; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(NodeIterator.prototype, 'whatToShow', {
        get: function() { return this._whatToShow; },
        enumerable: true, configurable: true
    });
    Object.defineProperty(NodeIterator.prototype, 'filter', {
        get: function() { return this._filter; },
        enumerable: true, configurable: true
    });
    NodeIterator.prototype.detach = function() {};

    function _nodeFollowing(node, root) {
        var fc = _getFirstChild(node);
        if (fc) return fc;
        if (node === root) return null;
        var cur = node;
        while (cur) {
            if (cur === root) return null;
            if (cur.nextSibling) return cur.nextSibling;
            cur = cur.parentNode;
        }
        return null;
    }

    function _nodePreceding(node, root) {
        if (node === root) return null;
        var ps = node.previousSibling;
        if (ps) {
            var cur = ps;
            while (_getLastChild(cur)) {
                cur = _getLastChild(cur);
            }
            return cur;
        }
        var p = node.parentNode;
        if (!p || p === root) return p || null;
        return p;
    }

    function _traverseNodeIterator(iterator, direction) {
        iterator._candidateReference = {
            node: iterator._reference,
            pointerBefore: iterator._pointerBeforeReference
        };
        var result = null;

        while (true) {
            var candidate = iterator._candidateReference;
            if (direction === 'next') {
                if (!candidate.pointerBefore) {
                    var following = _nodeFollowing(candidate.node, iterator._root);
                    if (!following) break;
                    candidate.node = following;
                    candidate.pointerBefore = false;
                } else {
                    candidate.pointerBefore = false;
                }
            } else {
                if (candidate.pointerBefore) {
                    var preceding = _nodePreceding(candidate.node, iterator._root);
                    if (!preceding) break;
                    candidate.node = preceding;
                    candidate.pointerBefore = true;
                } else {
                    candidate.pointerBefore = true;
                }
            }

            var nodeToFilter = candidate.node;
            var testResult = _filterTestNode(iterator._whatToShow, iterator._filter, nodeToFilter);
            if (testResult === 1) {
                result = nodeToFilter;
                break;
            }
        }

        if (result !== null) {
            iterator._reference = iterator._candidateReference.node;
            iterator._pointerBeforeReference = iterator._candidateReference.pointerBefore;
        }
        iterator._candidateReference = null;
        return result;
    }

    NodeIterator.prototype.nextNode = function() {
        return _traverseNodeIterator(this, 'next');
    };

    NodeIterator.prototype.previousNode = function() {
        return _traverseNodeIterator(this, 'previous');
    };

    function _isInclusiveAncestorNode(ancestor, node) {
        if (!ancestor || !node) return false;
        var cur = node;
        while (cur) {
            if (cur === ancestor || (cur._nodeId !== undefined && ancestor._nodeId !== undefined && cur._nodeId === ancestor._nodeId)) {
                return true;
            }
            cur = cur.parentNode || cur._parentNode;
        }
        return false;
    }

    function _getNodePrevSibling(node) {
        if (!node) return null;
        if (node.previousSibling) return node.previousSibling;
        var p = node.parentNode || node._parentNode;
        if (p && p.childNodes) {
            var idx = -1;
            for (var i = 0; i < p.childNodes.length; i++) {
                if (p.childNodes[i] === node || (p.childNodes[i]._nodeId !== undefined && node._nodeId !== undefined && p.childNodes[i]._nodeId === node._nodeId)) {
                    idx = i; break;
                }
            }
            if (idx > 0) return p.childNodes[idx - 1];
        }
        return null;
    }

    function _getNodeNextSibling(node) {
        if (!node) return null;
        if (node.nextSibling) return node.nextSibling;
        var p = node.parentNode || node._parentNode;
        if (p && p.childNodes) {
            var idx = -1;
            for (var i = 0; i < p.childNodes.length; i++) {
                if (p.childNodes[i] === node || (p.childNodes[i]._nodeId !== undefined && node._nodeId !== undefined && p.childNodes[i]._nodeId === node._nodeId)) {
                    idx = i; break;
                }
            }
            if (idx !== -1 && idx + 1 < p.childNodes.length) return p.childNodes[idx + 1];
        }
        return null;
    }

    function _adjustNodePointer(pointer, iterator, toBeRemovedNode) {
        if (!_isInclusiveAncestorNode(toBeRemovedNode, pointer.node)) {
            return pointer;
        }
        if (_isInclusiveAncestorNode(toBeRemovedNode, iterator._root)) {
            return pointer;
        }

        if (pointer.pointerBefore) {
            var cur = toBeRemovedNode;
            var next = null;
            while (cur && cur !== iterator._root) {
                var sib = _getNodeNextSibling(cur);
                if (sib) {
                    if (_isInclusiveAncestorNode(iterator._root, sib)) {
                        next = sib;
                        break;
                    }
                }
                cur = cur.parentNode || cur._parentNode;
            }
            if (next) {
                return { node: next, pointerBefore: true };
            }
            var prev = null;
            var ps = _getNodePrevSibling(toBeRemovedNode);
            if (ps) {
                var c = ps;
                while (_getLastChild(c)) c = _getLastChild(c);
                prev = c;
            } else {
                prev = toBeRemovedNode.parentNode || toBeRemovedNode._parentNode;
            }
            if (!prev || !_isInclusiveAncestorNode(iterator._root, prev)) {
                prev = iterator._root;
            }
            return { node: prev, pointerBefore: false };
        } else {
            var prev = null;
            var ps = _getNodePrevSibling(toBeRemovedNode);
            if (ps) {
                var c = ps;
                while (_getLastChild(c)) c = _getLastChild(c);
                prev = c;
            } else {
                prev = toBeRemovedNode.parentNode || toBeRemovedNode._parentNode;
            }
            if (!prev || !_isInclusiveAncestorNode(iterator._root, prev)) {
                prev = iterator._root;
            }
            return { node: prev, pointerBefore: false };
        }
    }

    function _notifyNodeIteratorsPreRemove(toBeRemoved) {
        if (!toBeRemoved) return;
        for (var i = 0; i < _activeNodeIterators.length; i++) {
            var iter = _activeNodeIterators[i];
            if (!iter) continue;
            var adj = _adjustNodePointer({ node: iter._reference, pointerBefore: iter._pointerBeforeReference }, iter, toBeRemoved);
            iter._reference = adj.node;
            iter._pointerBeforeReference = adj.pointerBefore;
            if (iter._candidateReference) {
                var cAdj = _adjustNodePointer(iter._candidateReference, iter, toBeRemoved);
                iter._candidateReference.node = cAdj.node;
                iter._candidateReference.pointerBefore = cAdj.pointerBefore;
            }
        }
    }
    globalThis._notifyNodeIteratorsPreRemove = _notifyNodeIteratorsPreRemove;

    document.createTreeWalker = function(root, whatToShow, filter) {
        if (arguments.length < 1 || !root || typeof root !== 'object') {
            throw new TypeError("Failed to execute 'createTreeWalker' on 'Document': 1 argument required");
        }
        var tw = Object.create(TreeWalker.prototype);
        tw._root = root;
        tw._currentNode = root;
        tw._whatToShow = (whatToShow === undefined) ? 0xFFFFFFFF : (Number(whatToShow) >>> 0);
        tw._filter = filter || null;
        return tw;
    };

    document.createNodeIterator = function(root, whatToShow, filter) {
        if (arguments.length < 1 || !root || typeof root !== 'object') {
            throw new TypeError("Failed to execute 'createNodeIterator' on 'Document': 1 argument required");
        }
        var ni = Object.create(NodeIterator.prototype);
        ni._root = root;
        ni._reference = root;
        ni._pointerBeforeReference = true;
        ni._whatToShow = (whatToShow === undefined) ? 0xFFFFFFFF : (Number(whatToShow) >>> 0);
        ni._filter = filter || null;
        ni._candidateReference = null;
        _activeNodeIterators.push(ni);
        return ni;
    };

    if (globalThis.Document && globalThis.Document.prototype) {
        globalThis.Document.prototype.createTreeWalker = function(root, whatToShow, filter) {
            return document.createTreeWalker(root, whatToShow, filter);
        };
        globalThis.Document.prototype.createNodeIterator = function(root, whatToShow, filter) {
            return document.createNodeIterator(root, whatToShow, filter);
        };
    }

    var _domInterfaces = {
        NodeIterator: NodeIterator,
        TreeWalker: TreeWalker,
        NodeFilter: NodeFilter
    };
    for (var _k in _domInterfaces) {
        Object.defineProperty(globalThis, _k, {
            value: _domInterfaces[_k],
            writable: true,
            enumerable: false,
            configurable: true
        });
    }

    var _allDomInterfaces = [
        "Event", "CustomEvent", "EventTarget", "AbortController", "AbortSignal",
        "Node", "Document", "DOMImplementation", "DocumentFragment", "ProcessingInstruction",
        "DocumentType", "Element", "Attr", "CharacterData", "Text", "Comment",
        "NodeIterator", "TreeWalker", "NodeFilter", "NodeList", "HTMLCollection", "DOMTokenList"
    ];
    for (var _j = 0; _j < _allDomInterfaces.length; _j++) {
        var _ifName = _allDomInterfaces[_j];
        if (globalThis[_ifName] !== undefined) {
            Object.defineProperty(globalThis, _ifName, {
                value: globalThis[_ifName],
                writable: true,
                enumerable: false,
                configurable: true
            });
        }
    }
    })();
