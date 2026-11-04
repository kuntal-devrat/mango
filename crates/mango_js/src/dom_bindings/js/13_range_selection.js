    function _getNodeIndex(node) {
        if (!node) return 0;
        var p = node.parentNode || node._parentNode;
        if (!p) return 0;
        var siblings = p.childNodes || p._childNodes || [];
        for (var i = 0; i < siblings.length; i++) {
            if (siblings[i] === node || (siblings[i]._nodeId !== undefined && node._nodeId !== undefined && siblings[i]._nodeId === node._nodeId)) {
                return i;
            }
        }
        return 0;
    }

    function _getNodeLength(node) {
        if (!node) return 0;
        if (node.nodeType === 3 || node.nodeType === 4 || node.nodeType === 8) {
            return (node.nodeValue !== null && node.nodeValue !== undefined) ? String(node.nodeValue).length : (node.data ? String(node.data).length : (node.textContent || '').length);
        }
        return (node.childNodes || node._childNodes || []).length;
    }

    function _getCommonAncestor(a, b) {
        if (!a || !b) return a || b || document;
        if (a === b || (a._nodeId !== undefined && b._nodeId !== undefined && a._nodeId === b._nodeId)) return a;
        var aAncestors = [];
        var cur = a;
        while (cur) {
            aAncestors.push(cur);
            cur = cur.parentNode || cur._parentNode;
        }
        cur = b;
        while (cur) {
            for (var i = 0; i < aAncestors.length; i++) {
                if (aAncestors[i] === cur || (aAncestors[i]._nodeId !== undefined && cur._nodeId !== undefined && aAncestors[i]._nodeId === cur._nodeId)) {
                    return cur;
                }
            }
            cur = cur.parentNode || cur._parentNode;
        }
        return (typeof document !== 'undefined') ? document : a;
    }

    function _isDescendantOf(node, ancestor) {
        var cur = node ? (node.parentNode || node._parentNode) : null;
        while (cur) {
            if (cur === ancestor || (cur._nodeId !== undefined && ancestor && ancestor._nodeId !== undefined && cur._nodeId === ancestor._nodeId)) return true;
            cur = cur.parentNode || cur._parentNode;
        }
        return false;
    }

    function _isInclusiveAncestor(ancestor, node) {
        if (!ancestor || !node) return false;
        if (ancestor === node || (ancestor._nodeId !== undefined && node._nodeId !== undefined && ancestor._nodeId === node._nodeId)) return true;
        return _isDescendantOf(node, ancestor);
    }

    function _comparePoints(nodeA, offsetA, nodeB, offsetB) {
        if (!nodeA || !nodeB) return 0;
        if (nodeA === nodeB || (nodeA._nodeId !== undefined && nodeB._nodeId !== undefined && nodeA._nodeId === nodeB._nodeId)) {
            return offsetA < offsetB ? -1 : (offsetA > offsetB ? 1 : 0);
        }
        if (_isDescendantOf(nodeB, nodeA)) {
            var child = nodeB;
            while (child && (child.parentNode || child._parentNode) !== nodeA && !(child.parentNode && child.parentNode._nodeId !== undefined && nodeA._nodeId !== undefined && child.parentNode._nodeId === nodeA._nodeId)) {
                child = child.parentNode || child._parentNode;
            }
            var idx = _getNodeIndex(child);
            return offsetA <= idx ? -1 : 1;
        }
        if (_isDescendantOf(nodeA, nodeB)) {
            var child = nodeA;
            while (child && (child.parentNode || child._parentNode) !== nodeB && !(child.parentNode && child.parentNode._nodeId !== undefined && nodeB._nodeId !== undefined && child.parentNode._nodeId === nodeB._nodeId)) {
                child = child.parentNode || child._parentNode;
            }
            var idx = _getNodeIndex(child);
            return idx < offsetB ? -1 : 1;
        }
        var common = _getCommonAncestor(nodeA, nodeB);
        if (!common) return 0;
        var childA = nodeA;
        while (childA && (childA.parentNode || childA._parentNode) !== common && !(childA.parentNode && childA.parentNode._nodeId !== undefined && common._nodeId !== undefined && childA.parentNode._nodeId === common._nodeId)) {
            childA = childA.parentNode || childA._parentNode;
        }
        var childB = nodeB;
        while (childB && (childB.parentNode || childB._parentNode) !== common && !(childB.parentNode && childB.parentNode._nodeId !== undefined && common._nodeId !== undefined && childB.parentNode._nodeId === common._nodeId)) {
            childB = childB.parentNode || childB._parentNode;
        }
        var idxA = _getNodeIndex(childA);
        var idxB = _getNodeIndex(childB);
        return idxA < idxB ? -1 : 1;
    }

    var _docPositionUid = 1;
    function _compareDocumentPositionNodes(nodeA, nodeB) {
        if (nodeA === nodeB) return 0;
        if (!nodeB || typeof nodeB !== 'object') {
            throw new TypeError("Failed to execute 'compareDocumentPosition' on 'Node': parameter 1 is not of type 'Node'.");
        }
        var rootA = nodeA;
        while (rootA.parentNode) rootA = rootA.parentNode;
        var rootB = nodeB;
        while (rootB.parentNode) rootB = rootB.parentNode;

        if (rootA !== rootB) {
            if (!nodeA._docPosUid) nodeA._docPosUid = _docPositionUid++;
            if (!nodeB._docPosUid) nodeB._docPosUid = _docPositionUid++;
            var isFollowing = nodeA._docPosUid < nodeB._docPosUid;
            return 1 | 32 | (isFollowing ? 4 : 2);
        }

        if (_isDescendantOf(nodeA, nodeB)) {
            return 8 | 2;
        }
        if (_isDescendantOf(nodeB, nodeA)) {
            return 16 | 4;
        }

        var common = _getCommonAncestor(nodeA, nodeB);
        var childA = nodeA;
        while (childA.parentNode && childA.parentNode !== common && !(childA.parentNode._nodeId !== undefined && childA.parentNode._nodeId === common._nodeId)) {
            childA = childA.parentNode;
        }
        var childB = nodeB;
        while (childB.parentNode && childB.parentNode !== common && !(childB.parentNode._nodeId !== undefined && childB.parentNode._nodeId === common._nodeId)) {
            childB = childB.parentNode;
        }
        var idxA = _getNodeIndex(childA);
        var idxB = _getNodeIndex(childB);
        return idxB < idxA ? 2 : 4;
    }
    globalThis._compareDocumentPositionNodes = _compareDocumentPositionNodes;

    var _liveRanges = new Set();
    globalThis._liveRanges = _liveRanges;

    function Range() {
        this.startContainer = (typeof document !== 'undefined') ? document : null;
        this.startOffset = 0;
        this.endContainer = this.startContainer;
        this.endOffset = 0;
        this.collapsed = true;
        this.commonAncestorContainer = this.startContainer;
        _liveRanges.add(this);
    }

    Range.START_TO_START = 0;
    Range.START_TO_END = 1;
    Range.END_TO_END = 2;
    Range.END_TO_START = 3;

    Range.prototype.START_TO_START = 0;
    Range.prototype.START_TO_END = 1;
    Range.prototype.END_TO_END = 2;
    Range.prototype.END_TO_START = 3;

    Range.prototype._updateState = function() {
        this.collapsed = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId)) && (this.startOffset === this.endOffset);
        this.commonAncestorContainer = _getCommonAncestor(this.startContainer, this.endContainer);
    };

    Range.prototype.setStart = function(node, offset) {
        if (!node) throw new TypeError("Failed to execute 'setStart' on 'Range': parameter 1 is not of type 'Node'.");
        offset = Math.max(0, parseInt(offset, 10) || 0);
        var nodeRoot = node.getRootNode ? node.getRootNode() : (node.ownerDocument || document);
        var currentRoot = this.startContainer ? (this.startContainer.getRootNode ? this.startContainer.getRootNode() : (this.startContainer.ownerDocument || document)) : null;
        if (currentRoot && nodeRoot !== currentRoot) {
            this.endContainer = node;
            this.endOffset = offset;
        }
        this.startContainer = node;
        this.startOffset = offset;
        if (!this.endContainer || _comparePoints(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
            this.endContainer = node;
            this.endOffset = offset;
        }
        this._updateState();
    };

    Range.prototype.setEnd = function(node, offset) {
        if (!node) throw new TypeError("Failed to execute 'setEnd' on 'Range': parameter 1 is not of type 'Node'.");
        offset = Math.max(0, parseInt(offset, 10) || 0);
        var nodeRoot = node.getRootNode ? node.getRootNode() : (node.ownerDocument || document);
        var currentRoot = this.startContainer ? (this.startContainer.getRootNode ? this.startContainer.getRootNode() : (this.startContainer.ownerDocument || document)) : null;
        if (currentRoot && nodeRoot !== currentRoot) {
            this.startContainer = node;
            this.startOffset = offset;
        }
        this.endContainer = node;
        this.endOffset = offset;
        if (!this.startContainer || _comparePoints(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
            this.startContainer = node;
            this.startOffset = offset;
        }
        this._updateState();
    };

    Range.prototype.setStartBefore = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setStart(node.parentNode, _getNodeIndex(node));
    };

    Range.prototype.setStartAfter = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setStart(node.parentNode, _getNodeIndex(node) + 1);
    };

    Range.prototype.setEndBefore = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setEnd(node.parentNode, _getNodeIndex(node));
    };

    Range.prototype.setEndAfter = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setEnd(node.parentNode, _getNodeIndex(node) + 1);
    };

    Range.prototype.collapse = function(toStart) {
        if (toStart) {
            this.endContainer = this.startContainer;
            this.endOffset = this.startOffset;
        } else {
            this.startContainer = this.endContainer;
            this.startOffset = this.endOffset;
        }
        this._updateState();
    };

    Range.prototype.selectNode = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        var idx = _getNodeIndex(node);
        this.setStart(node.parentNode, idx);
        this.setEnd(node.parentNode, idx + 1);
    };

    Range.prototype.selectNodeContents = function(node) {
        if (!node) throw new Error("TypeError: Node must not be null");
        this.setStart(node, 0);
        this.setEnd(node, _getNodeLength(node));
    };

    Range.prototype.compareBoundaryPoints = function(how, sourceRange) {
        if (!sourceRange) throw new Error("TypeError: sourceRange must not be null");
        if (how === 0) {
            return _comparePoints(this.startContainer, this.startOffset, sourceRange.startContainer, sourceRange.startOffset);
        } else if (how === 1) {
            return _comparePoints(this.endContainer, this.endOffset, sourceRange.startContainer, sourceRange.startOffset);
        } else if (how === 2) {
            return _comparePoints(this.endContainer, this.endOffset, sourceRange.endContainer, sourceRange.endOffset);
        } else if (how === 3) {
            return _comparePoints(this.startContainer, this.startOffset, sourceRange.endContainer, sourceRange.endOffset);
        }
        throw new Error("NotSupportedError: Invalid comparison method");
    };

    Range.prototype.cloneRange = function() {
        var r = new Range();
        r.setStart(this.startContainer, this.startOffset);
        r.setEnd(this.endContainer, this.endOffset);
        return r;
    };

    function _isContained(node, range) {
        return _comparePoints(range.startContainer, range.startOffset, node, 0) <= 0 &&
               _comparePoints(node, _getNodeLength(node), range.endContainer, range.endOffset) <= 0;
    }

    function _extractRangeContents(range, cloneOnly) {
        var sc = range.startContainer, so = range.startOffset;
        var ec = range.endContainer, eo = range.endOffset;
        var ownerDoc = sc ? (sc.ownerDocument || sc._ownerDocument || (sc.nodeType === 9 ? sc : document)) : document;
        var frag = ownerDoc.createDocumentFragment ? ownerDoc.createDocumentFragment() : _wrapElement(_createFragment());
        if (range.collapsed) return frag;

        var isSame = (sc === ec || (sc && ec && sc._nodeId !== undefined && sc._nodeId === ec._nodeId));
        if (isSame && (sc.nodeType === 3 || sc.nodeType === 4 || sc.nodeType === 8)) {
            var clone = sc.cloneNode(false);
            var str = (sc.data || sc.nodeValue || '').substring(so, eo);
            clone.nodeValue = str;
            if (clone.data !== undefined) clone.data = str;
            frag.appendChild(clone);
            if (!cloneOnly) {
                var full = sc.data || sc.nodeValue || '';
                var remaining = full.substring(0, so) + full.substring(eo);
                sc.nodeValue = remaining;
                if (sc.data !== undefined) sc.data = remaining;
            }
            return frag;
        }

        var common = range.commonAncestorContainer || _getCommonAncestor(sc, ec);
        var firstPartiallyContained = null;
        if (!_isInclusiveAncestor(sc, ec)) {
            var cur = sc;
            while (cur && (cur.parentNode || cur._parentNode) !== common && !(cur.parentNode && cur.parentNode._nodeId !== undefined && common._nodeId !== undefined && cur.parentNode._nodeId === common._nodeId)) {
                cur = cur.parentNode || cur._parentNode;
            }
            firstPartiallyContained = cur;
        }

        var lastPartiallyContained = null;
        if (!_isInclusiveAncestor(ec, sc)) {
            var cur = ec;
            while (cur && (cur.parentNode || cur._parentNode) !== common && !(cur.parentNode && cur.parentNode._nodeId !== undefined && common._nodeId !== undefined && cur.parentNode._nodeId === common._nodeId)) {
                cur = cur.parentNode || cur._parentNode;
            }
            lastPartiallyContained = cur;
        }

        var commonKids = common.childNodes ? Array.prototype.slice.call(common.childNodes) : (common._childNodes ? Array.prototype.slice.call(common._childNodes) : []);
        var containedChildren = [];
        for (var i = 0; i < commonKids.length; i++) {
            var k = commonKids[i];
            if (k && _isContained(k, range)) {
                containedChildren.push(k);
            }
        }

        var newOffset = 0;
        if (firstPartiallyContained) {
            newOffset = _getNodeIndex(firstPartiallyContained) + 1;
        } else if (containedChildren.length > 0) {
            newOffset = _getNodeIndex(containedChildren[0]);
        }

        if (firstPartiallyContained) {
            if (firstPartiallyContained.nodeType === 3 || firstPartiallyContained.nodeType === 4 || firstPartiallyContained.nodeType === 8) {
                var clone = firstPartiallyContained.cloneNode(false);
                var full = firstPartiallyContained.data || firstPartiallyContained.nodeValue || '';
                clone.nodeValue = full.substring(so);
                if (clone.data !== undefined) clone.data = clone.nodeValue;
                frag.appendChild(clone);
                if (!cloneOnly) {
                    firstPartiallyContained.nodeValue = full.substring(0, so);
                    if (firstPartiallyContained.data !== undefined) firstPartiallyContained.data = firstPartiallyContained.nodeValue;
                }
            } else {
                var clone = firstPartiallyContained.cloneNode(false);
                frag.appendChild(clone);
                var subrange = new Range();
                subrange.setStart(sc, so);
                subrange.setEnd(firstPartiallyContained, _getNodeLength(firstPartiallyContained));
                var subfrag = _extractRangeContents(subrange, cloneOnly);
                clone.appendChild(subfrag);
            }
        }

        for (var j = 0; j < containedChildren.length; j++) {
            var ck = containedChildren[j];
            if (cloneOnly) {
                frag.appendChild(ck.cloneNode(true));
            } else {
                frag.appendChild(ck);
            }
        }

        if (lastPartiallyContained) {
            if (lastPartiallyContained.nodeType === 3 || lastPartiallyContained.nodeType === 4 || lastPartiallyContained.nodeType === 8) {
                var clone = lastPartiallyContained.cloneNode(false);
                var full = lastPartiallyContained.data || lastPartiallyContained.nodeValue || '';
                clone.nodeValue = full.substring(0, eo);
                if (clone.data !== undefined) clone.data = clone.nodeValue;
                frag.appendChild(clone);
                if (!cloneOnly) {
                    lastPartiallyContained.nodeValue = full.substring(eo);
                    if (lastPartiallyContained.data !== undefined) lastPartiallyContained.data = lastPartiallyContained.nodeValue;
                }
            } else {
                var clone = lastPartiallyContained.cloneNode(false);
                frag.appendChild(clone);
                var subrange = new Range();
                subrange.setStart(lastPartiallyContained, 0);
                subrange.setEnd(ec, eo);
                var subfrag = _extractRangeContents(subrange, cloneOnly);
                clone.appendChild(subfrag);
            }
        }

        if (!cloneOnly) {
            range.setStart(common, newOffset);
            range.collapse(true);
        }
        return frag;
    }

    Range.prototype.deleteContents = function() {
        _extractRangeContents(this, false);
    };

    Range.prototype.cloneContents = function() {
        return _extractRangeContents(this, true);
    };

    Range.prototype.extractContents = function() {
        return _extractRangeContents(this, false);
    };

    Range.prototype.insertNode = function(node) {
        if (!node) throw new TypeError("Failed to execute 'insertNode' on 'Range': parameter 1 is not of type 'Node'.");
        if (this.startContainer.nodeType === 7 || this.startContainer.nodeType === 8 ||
            (this.startContainer.nodeType === 3 && !this.startContainer.parentNode) ||
            this.startContainer === node) {
            throw new DOMException("The node cannot be inserted at the specified point in the hierarchy.", "HierarchyRequestError");
        }
        var referenceNode = null;
        if (this.startContainer.nodeType === 3) {
            referenceNode = this.startContainer;
        } else {
            var scKids = this.startContainer.childNodes || this.startContainer._childNodes || [];
            referenceNode = (this.startOffset < scKids.length) ? scKids[this.startOffset] : null;
        }
        var parent = referenceNode === null ? this.startContainer : (referenceNode.parentNode || referenceNode._parentNode);

        if (this.startContainer.nodeType === 3) {
            referenceNode = this.startContainer.splitText(this.startOffset);
            parent = referenceNode.parentNode || referenceNode._parentNode;
        }
        if (node === referenceNode) {
            referenceNode = referenceNode.nextSibling;
        }
        if (node.parentNode) {
            node.parentNode.removeChild(node);
        }
        var newOffset = referenceNode === null ? _getNodeLength(parent) : _getNodeIndex(referenceNode);
        var isFrag = node.nodeType === 11;
        var nodeLen = isFrag ? (node.childNodes ? node.childNodes.length : 0) : 1;
        newOffset += nodeLen;

        parent.insertBefore(node, referenceNode);
        if (this.collapsed) {
            this.setEnd(parent, newOffset);
        }
    };

    Range.prototype.surroundContents = function(newParent) {
        if (!newParent) throw new TypeError("Failed to execute 'surroundContents' on 'Range': parameter 1 is not of type 'Node'.");
        var npType = newParent.nodeType;
        if (npType === 9 || npType === 10 || npType === 11) {
            throw new DOMException("The node provided is invalid.", "InvalidNodeTypeError");
        }
        var sc = this.startContainer, ec = this.endContainer;
        if (sc !== ec) {
            if (_isPartiallyContained(sc, this) && sc.nodeType !== 3) {
                throw new DOMException("The Range has partially contained non-Text boundary points.", "InvalidStateError");
            }
            if (_isPartiallyContained(ec, this) && ec.nodeType !== 3) {
                throw new DOMException("The Range has partially contained non-Text boundary points.", "InvalidStateError");
            }
            var common = this.commonAncestorContainer || _getCommonAncestor(sc, ec);
            var kids = common.childNodes || common._childNodes || [];
            for (var i = 0; i < kids.length; i++) {
                var k = kids[i];
                if (k && k.nodeType !== 3 && _isPartiallyContained(k, this)) {
                    throw new DOMException("The Range has partially contained non-Text nodes.", "InvalidStateError");
                }
            }
        }
        var frag = this.extractContents();
        while (newParent.firstChild) newParent.removeChild(newParent.firstChild);
        this.insertNode(newParent);
        newParent.appendChild(frag);
        this.selectNode(newParent);
    };

    Range.prototype.createContextualFragment = function(html) {
        var frag = _wrapElement(_createFragment());
        frag.innerHTML = String(html);
        return frag;
    };

    Range.prototype.toString = function() {
        if (this.collapsed) return '';
        var sc = this.startContainer, so = this.startOffset;
        var ec = this.endContainer, eo = this.endOffset;
        var isSame = (sc === ec || (sc && ec && sc._nodeId !== undefined && sc._nodeId === ec._nodeId));
        if (isSame && (sc.nodeType === 3 || sc.nodeType === 4)) {
            return (sc.data || sc.nodeValue || '').substring(so, eo);
        }
        var s = '';
        if (sc.nodeType === 3 || sc.nodeType === 4) {
            s += (sc.data || sc.nodeValue || '').substring(so);
        }
        var root = this.commonAncestorContainer || sc;
        if (root) {
            function walkText(node) {
                if (!node) return;
                if (node.nodeType === 3 || node.nodeType === 4) {
                    var sameStart = (node === sc || (sc && node._nodeId !== undefined && node._nodeId === sc._nodeId));
                    var sameEnd = (node === ec || (ec && node._nodeId !== undefined && node._nodeId === ec._nodeId));
                    if (!sameStart && !sameEnd) {
                        var len = _getNodeLength(node);
                        var afterStart = _comparePoints(sc, so, node, 0) <= 0;
                        var beforeEnd = _comparePoints(node, len, ec, eo) <= 0;
                        if (afterStart && beforeEnd) {
                            s += (node.data || node.nodeValue || '');
                        }
                    }
                } else {
                    var kids = node.childNodes || node._childNodes || [];
                    for (var i = 0; i < kids.length; i++) {
                        walkText(kids[i]);
                    }
                }
            }
            var rootKids = root.childNodes || root._childNodes || [];
            for (var i = 0; i < rootKids.length; i++) {
                walkText(rootKids[i]);
            }
        }
        if ((ec.nodeType === 3 || ec.nodeType === 4) && !isSame) {
            s += (ec.data || ec.nodeValue || '').substring(0, eo);
        }
        return s;
    };

    Range.prototype.isPointInRange = function(node, offset) {
        return _comparePoints(node, offset, this.startContainer, this.startOffset) >= 0 && _comparePoints(node, offset, this.endContainer, this.endOffset) <= 0;
    };

    Range.prototype.comparePoint = function(node, offset) {
        if (_comparePoints(node, offset, this.startContainer, this.startOffset) < 0) return -1;
        if (_comparePoints(node, offset, this.endContainer, this.endOffset) > 0) return 1;
        return 0;
    };

    Range.prototype.intersectsNode = function(node) {
        if (!node) return false;
        var p = node.parentNode;
        if (!p) return false;
        var idx = _getNodeIndex(node);
        return _comparePoints(p, idx, this.endContainer, this.endOffset) < 0 && _comparePoints(p, idx + 1, this.startContainer, this.startOffset) > 0;
    };

    Range.prototype.getBoundingClientRect = function() {
        var startNode = this.startContainer;
        while (startNode && startNode.nodeType !== 1) startNode = startNode.parentElement || startNode.parentNode;
        var endNode = this.endContainer;
        while (endNode && endNode.nodeType !== 1) endNode = endNode.parentElement || endNode.parentNode;

        var startRect = (startNode && typeof startNode.getBoundingClientRect === 'function') ? startNode.getBoundingClientRect() : null;
        var endRect = (endNode && typeof endNode.getBoundingClientRect === 'function') ? endNode.getBoundingClientRect() : null;

        if (startRect && endRect && startRect !== endRect) {
            var left = Math.min(startRect.left, endRect.left);
            var top = Math.min(startRect.top, endRect.top);
            var right = Math.max(startRect.right, endRect.right);
            var bottom = Math.max(startRect.bottom, endRect.bottom);
            var w = Math.max(0, right - left);
            var h = Math.max(0, bottom - top);
            return {
                x: left, y: top, left: left, top: top, right: right, bottom: bottom, width: w, height: h,
                toJSON: function() { return this; }
            };
        }
        if (startRect) return startRect;
        if (endRect) return endRect;
        return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, toJSON: function() { return this; } };
    };

    Range.prototype.getClientRects = function() {
        var r = this.getBoundingClientRect();
        if (r && (r.width > 0 || r.height > 0)) return [r];
        return [];
    };

    Range.prototype.detach = function() {
        if (_liveRanges) _liveRanges.delete(this);
    };

    globalThis.Range = Range;

    function _notifyRangeNodeRemoved(parent, node, index) {
        if (!_liveRanges || _liveRanges.size === 0) return;
        _liveRanges.forEach(function(range) {
            var updated = false;
            if (range.startContainer === node || _isDescendantOf(range.startContainer, node)) {
                range.startContainer = parent;
                range.startOffset = index;
                updated = true;
            } else if (range.startContainer === parent && range.startOffset > index) {
                range.startOffset--;
                updated = true;
            }
            if (range.endContainer === node || _isDescendantOf(range.endContainer, node)) {
                range.endContainer = parent;
                range.endOffset = index;
                updated = true;
            } else if (range.endContainer === parent && range.endOffset > index) {
                range.endOffset--;
                updated = true;
            }
            if (updated && range._updateState) range._updateState();
        });
    }
    globalThis._notifyRangeNodeRemoved = _notifyRangeNodeRemoved;

    function _notifyRangeSplitText(node, newNode, offset) {
        if (!_liveRanges || _liveRanges.size === 0) return;
        var parent = node.parentNode || node._parentNode;
        var nodeIndex = -1;
        if (parent && parent._childNodes) {
            nodeIndex = parent._childNodes.indexOf(node);
        }
        if (nodeIndex === -1 && typeof _getNodeIndex === 'function') {
            nodeIndex = _getNodeIndex(node);
        }
        _liveRanges.forEach(function(range) {
            var updated = false;
            if (range.startContainer === node && range.startOffset > offset) {
                range.startContainer = newNode;
                range.startOffset -= offset;
                updated = true;
            }
            if (range.endContainer === node && range.endOffset > offset) {
                range.endContainer = newNode;
                range.endOffset -= offset;
                updated = true;
            }
            if (parent && (range.startContainer === parent || (range.startContainer && parent && range.startContainer._nodeId !== undefined && range.startContainer._nodeId === parent._nodeId)) && range.startOffset === nodeIndex + 1) {
                range.startOffset++;
                updated = true;
            }
            if (parent && (range.endContainer === parent || (range.endContainer && parent && range.endContainer._nodeId !== undefined && range.endContainer._nodeId === parent._nodeId)) && range.endOffset === nodeIndex + 1) {
                range.endOffset++;
                updated = true;
            }
            if (updated && range._updateState) range._updateState();
        });
    }
    globalThis._notifyRangeSplitText = _notifyRangeSplitText;

    function _notifyRangeNormalizeMerge(node, prevLength, sibling) {
        if (!_liveRanges || _liveRanges.size === 0) return;
        var parent = sibling.parentNode || sibling._parentNode;
        var siblingIndex = -1;
        if (parent && parent._childNodes) {
            siblingIndex = parent._childNodes.indexOf(sibling);
        }
        if (siblingIndex === -1 && typeof _getNodeIndex === 'function') {
            siblingIndex = _getNodeIndex(sibling);
        }
        _liveRanges.forEach(function(range) {
            var updated = false;
            if (range.startContainer === sibling || (range.startContainer && sibling && range.startContainer._nodeId !== undefined && range.startContainer._nodeId === sibling._nodeId)) {
                range.startContainer = node;
                range.startOffset = prevLength + range.startOffset;
                updated = true;
            }
            if (range.endContainer === sibling || (range.endContainer && sibling && range.endContainer._nodeId !== undefined && range.endContainer._nodeId === sibling._nodeId)) {
                range.endContainer = node;
                range.endOffset = prevLength + range.endOffset;
                updated = true;
            }
            if (parent && (range.startContainer === parent || (range.startContainer && parent && range.startContainer._nodeId !== undefined && range.startContainer._nodeId === parent._nodeId)) && range.startOffset === siblingIndex) {
                range.startContainer = node;
                range.startOffset = prevLength;
                updated = true;
            }
            if (parent && (range.endContainer === parent || (range.endContainer && parent && range.endContainer._nodeId !== undefined && range.endContainer._nodeId === parent._nodeId)) && range.endOffset === siblingIndex) {
                range.endContainer = node;
                range.endOffset = prevLength;
                updated = true;
            }
            if (updated && range._updateState) range._updateState();
        });
    }
    globalThis._notifyRangeNormalizeMerge = _notifyRangeNormalizeMerge;

    function _isExclusiveTextNode(node) {
        if (!node) return false;
        if (node.nodeType !== 3) return false;
        if (node._isCDATA) return false;
        if (node.nodeName !== "#text") return false;
        if (typeof CDATASection !== 'undefined' && node instanceof CDATASection) return false;
        return true;
    }
    globalThis._isExclusiveTextNode = _isExclusiveTextNode;

    function _normalizeNode(root) {
        if (!root) return;
        var child = root.firstChild;
        while (child) {
            var next = child.nextSibling;
            if (_isExclusiveTextNode(child)) {
                var dataLen = (child.data !== undefined && child.data !== null) ? child.data.length : ((child.nodeValue !== undefined && child.nodeValue !== null) ? child.nodeValue.length : 0);
                if (dataLen === 0) {
                    root.removeChild(child);
                    child = next;
                    continue;
                }
                var sib = child.nextSibling;
                while (sib && _isExclusiveTextNode(sib)) {
                    var sibNext = sib.nextSibling;
                    var sibData = (sib.data !== undefined && sib.data !== null) ? sib.data : (sib.nodeValue || "");
                    var prevLen = (child.data !== undefined && child.data !== null) ? child.data.length : (child.nodeValue ? child.nodeValue.length : 0);
                    _notifyRangeNormalizeMerge(child, prevLen, sib);
                    child.data = (child.data || "") + sibData;
                    root.removeChild(sib);
                    sib = sibNext;
                }
                child = sib;
            } else {
                _normalizeNode(child);
                child = next;
            }
        }
    }
    globalThis._normalizeNode = _normalizeNode;

