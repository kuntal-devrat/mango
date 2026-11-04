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

    globalThis.Selection = Selection;
