    function _getNodeIndex(node) {
        if (!node || !node.parentNode) return 0;
        var siblings = node.parentNode.childNodes || [];
        for (var i = 0; i < siblings.length; i++) {
            if (siblings[i] === node || (siblings[i]._nodeId !== undefined && siblings[i]._nodeId === node._nodeId)) {
                return i;
            }
        }
        return 0;
    }

    function _getNodeLength(node) {
        if (!node) return 0;
        if (node.nodeType === 3 || node.nodeType === 8) {
            return (node.nodeValue !== null && node.nodeValue !== undefined) ? String(node.nodeValue).length : (node.textContent || '').length;
        }
        return (node.childNodes || []).length;
    }

    function _getCommonAncestor(a, b) {
        if (!a || !b) return a || b || document;
        if (a === b || (a._nodeId !== undefined && a._nodeId === b._nodeId)) return a;
        var aAncestors = [];
        var cur = a;
        while (cur) {
            aAncestors.push(cur);
            cur = cur.parentNode;
        }
        cur = b;
        while (cur) {
            for (var i = 0; i < aAncestors.length; i++) {
                if (aAncestors[i] === cur || (aAncestors[i]._nodeId !== undefined && aAncestors[i]._nodeId === cur._nodeId)) {
                    return cur;
                }
            }
            cur = cur.parentNode;
        }
        return (typeof document !== 'undefined') ? document : a;
    }

    function _isDescendantOf(node, ancestor) {
        var cur = node ? node.parentNode : null;
        while (cur) {
            if (cur === ancestor || (cur._nodeId !== undefined && cur._nodeId === ancestor._nodeId)) return true;
            cur = cur.parentNode;
        }
        return false;
    }

    function _comparePoints(nodeA, offsetA, nodeB, offsetB) {
        if (nodeA === nodeB || (nodeA && nodeB && nodeA._nodeId !== undefined && nodeA._nodeId === nodeB._nodeId)) {
            return offsetA < offsetB ? -1 : (offsetA > offsetB ? 1 : 0);
        }
        if (_isDescendantOf(nodeB, nodeA)) {
            var child = nodeB;
            while (child.parentNode && child.parentNode !== nodeA && !(child.parentNode._nodeId !== undefined && child.parentNode._nodeId === nodeA._nodeId)) {
                child = child.parentNode;
            }
            var idx = _getNodeIndex(child);
            return idx < offsetA ? 1 : -1;
        }
        if (_isDescendantOf(nodeA, nodeB)) {
            var child = nodeA;
            while (child.parentNode && child.parentNode !== nodeB && !(child.parentNode._nodeId !== undefined && child.parentNode._nodeId === nodeB._nodeId)) {
                child = child.parentNode;
            }
            var idx = _getNodeIndex(child);
            return offsetB <= idx ? 1 : -1;
        }
        var common = _getCommonAncestor(nodeA, nodeB);
        if (!common) return 0;
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
        return idxA < idxB ? -1 : 1;
    }

    function Range() {
        this.startContainer = (typeof document !== 'undefined') ? (document.body || document) : null;
        this.startOffset = 0;
        this.endContainer = this.startContainer;
        this.endOffset = 0;
        this.collapsed = true;
        this.commonAncestorContainer = this.startContainer;
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
        if (!node) throw new Error("TypeError: Node must not be null");
        offset = Math.max(0, parseInt(offset, 10) || 0);
        this.startContainer = node;
        this.startOffset = offset;
        if (!this.endContainer || _comparePoints(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
            this.endContainer = node;
            this.endOffset = offset;
        }
        this._updateState();
    };

    Range.prototype.setEnd = function(node, offset) {
        if (!node) throw new Error("TypeError: Node must not be null");
        offset = Math.max(0, parseInt(offset, 10) || 0);
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

    Range.prototype.deleteContents = function() {
        if (this.collapsed) return;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = s.substring(0, this.startOffset) + s.substring(this.endOffset);
            } else if (this.startContainer.childNodes) {
                var kids = Array.prototype.slice.call(this.startContainer.childNodes);
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    this.startContainer.removeChild(kids[i]);
                }
            }
        } else {
            if (this.startContainer.nodeType === 3) {
                var s1 = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = s1.substring(0, this.startOffset);
            }
            if (this.endContainer.nodeType === 3) {
                var s2 = this.endContainer.nodeValue || '';
                this.endContainer.nodeValue = s2.substring(this.endOffset);
            }
        }
        this.collapse(true);
    };

    Range.prototype.cloneContents = function() {
        var frag = _wrapElement(_createFragment());
        if (this.collapsed) return frag;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s)));
            } else if (this.startContainer.childNodes) {
                var kids = this.startContainer.childNodes;
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    if (kids[i] && kids[i].cloneNode) frag.appendChild(kids[i].cloneNode(true));
                }
            }
        } else {
            if (this.startContainer.nodeType === 3) {
                var s1 = (this.startContainer.nodeValue || '').substring(this.startOffset);
                frag.appendChild(_wrapElement(_origCTN(s1)));
            }
            if (this.endContainer.nodeType === 3) {
                var s2 = (this.endContainer.nodeValue || '').substring(0, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s2)));
            }
        }
        return frag;
    };

    Range.prototype.extractContents = function() {
        var frag = _wrapElement(_createFragment());
        if (this.collapsed) return frag;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s)));
                var full = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = full.substring(0, this.startOffset) + full.substring(this.endOffset);
            } else if (this.startContainer.childNodes) {
                var kids = Array.prototype.slice.call(this.startContainer.childNodes);
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    if (kids[i]) frag.appendChild(kids[i]);
                }
            }
        } else {
            var fragClone = this.cloneContents();
            this.deleteContents();
            return fragClone;
        }
        this.collapse(true);
        return frag;
    };

    Range.prototype.insertNode = function(node) {
        if (!node) throw new Error("TypeError: Node must not be null");
        if (this.startContainer.nodeType === 3) {
            var full = this.startContainer.nodeValue || '';
            var before = full.substring(0, this.startOffset);
            var after = full.substring(this.startOffset);
            this.startContainer.nodeValue = before;
            var afterNode = _wrapElement(_origCTN(after));
            var p = this.startContainer.parentNode;
            if (p) {
                p.insertBefore(afterNode, this.startContainer.nextSibling);
                p.insertBefore(node, afterNode);
            }
        } else {
            var ref = (this.startContainer.childNodes && this.startContainer.childNodes[this.startOffset]) || null;
            this.startContainer.insertBefore(node, ref);
        }
    };

    Range.prototype.surroundContents = function(newParent) {
        if (!newParent) throw new Error("TypeError: newParent must not be null");
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
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                return (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
            }
            return (this.startContainer.textContent || '');
        }
        if (this.startContainer.nodeType === 3 && this.endContainer.nodeType === 3) {
            var s1 = (this.startContainer.nodeValue || '').substring(this.startOffset);
            var s2 = (this.endContainer.nodeValue || '').substring(0, this.endOffset);
            return s1 + s2;
        }
        return (this.commonAncestorContainer && this.commonAncestorContainer.textContent) ? this.commonAncestorContainer.textContent : '';
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

    Range.prototype.detach = function() {};

    function Selection() {
        this._range = null;
        this.anchorNode = null;
        this.anchorOffset = 0;
        this.focusNode = null;
        this.focusOffset = 0;
        this.isCollapsed = true;
        this.rangeCount = 0;
        this.type = 'None';
    }

    Selection.prototype._sync = function() {
        if (this._range) {
            this.anchorNode = this._range.startContainer;
            this.anchorOffset = this._range.startOffset;
            this.focusNode = this._range.endContainer;
            this.focusOffset = this._range.endOffset;
            this.isCollapsed = this._range.collapsed;
            this.rangeCount = 1;
            this.type = this._range.collapsed ? 'Caret' : 'Range';
        } else {
            this.anchorNode = null;
            this.anchorOffset = 0;
            this.focusNode = null;
            this.focusOffset = 0;
            this.isCollapsed = true;
            this.rangeCount = 0;
            this.type = 'None';
        }
    };

    Selection.prototype.getRangeAt = function(index) {
        if (Number(index) !== 0 || !this._range) {
            throw new Error("IndexSizeError: Selection does not contain a range at index " + index);
        }
        return this._range;
    };

    Selection.prototype.addRange = function(range) {
        if (!range) return;
        this._range = range;
        this._sync();
    };

    Selection.prototype.removeRange = function(range) {
        if (this._range === range) {
            this.removeAllRanges();
        }
    };

    Selection.prototype.removeAllRanges = function() {
        this._range = null;
        this._sync();
    };

    Selection.prototype.empty = function() {
        this.removeAllRanges();
    };

    Selection.prototype.collapse = function(node, offset) {
        var r = new Range();
        r.setStart(node, offset || 0);
        r.collapse(true);
        this.addRange(r);
    };

    Selection.prototype.collapseToStart = function() {
        if (this._range) {
            this._range.collapse(true);
            this._sync();
        }
    };

    Selection.prototype.collapseToEnd = function() {
        if (this._range) {
            this._range.collapse(false);
            this._sync();
        }
    };

    Selection.prototype.extend = function(node, offset) {
        if (this._range) {
            this._range.setEnd(node, offset || 0);
            this._sync();
        }
    };

    Selection.prototype.setBaseAndExtent = function(anchorNode, anchorOffset, focusNode, focusOffset) {
        var r = new Range();
        r.setStart(anchorNode, anchorOffset || 0);
        r.setEnd(focusNode, focusOffset || 0);
        this.addRange(r);
    };

    Selection.prototype.selectAllChildren = function(node) {
        var r = new Range();
        r.selectNodeContents(node);
        this.addRange(r);
    };

    Selection.prototype.deleteFromDocument = function() {
        if (this._range) {
            this._range.deleteContents();
            this._sync();
        }
    };

    Selection.prototype.containsNode = function(node, partlyContained) {
        if (!this._range) return false;
        return this._range.intersectsNode(node);
    };

    Selection.prototype.toString = function() {
        return this._range ? this._range.toString() : '';
    };

    globalThis.Range = Range;
    globalThis.Selection = Selection;

