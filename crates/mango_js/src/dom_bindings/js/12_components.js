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
        var childId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
        var refId = (typeof refChild === 'object' && refChild !== null && refChild._nodeId !== undefined) ? refChild._nodeId : refChild;
        var newObj = _wrapElement(childId);
        var isFrag = newObj && newObj.nodeType === 11;
        var fragChildren = isFrag ? (newObj.childNodes ? Array.prototype.slice.call(newObj.childNodes) : []) : null;
        _insertBefore(0, childId, (refId === undefined || refId === null) ? 0 : refId);
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
                target: document,
                addedNodes: isFrag && fragChildren ? fragChildren : [newObj],
                removedNodes: [],
                nextSibling: refObj
            });
        }
        return newChild;
    };
    document.replaceChild = function(newChild, oldChild) {
        var newId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
        var oldId = (typeof oldChild === 'object' && oldChild !== null && oldChild._nodeId !== undefined) ? oldChild._nodeId : oldChild;
        var oldObj = _wrapElement(oldId);
        var newObj = _wrapElement(newId);
        var isFrag = newObj && newObj.nodeType === 11;
        var fragChildren = isFrag ? (newObj.childNodes ? Array.prototype.slice.call(newObj.childNodes) : []) : null;
        _replaceChild(0, newId, oldId);
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
                target: document,
                addedNodes: isFrag && fragChildren ? fragChildren : [newObj],
                removedNodes: [oldObj]
            });
        }
        return oldChild;
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
        var otherId = (other && other._nodeId !== undefined) ? other._nodeId : other;
        return _elementContains(0, otherId);
    };
    document.compareDocumentPosition = function(other) {
        if (this === other) return 0;
        if (!other || typeof other !== 'object') {
            throw new TypeError("Failed to execute 'compareDocumentPosition' on 'Node': parameter 1 is not of type 'Node'.");
        }
        var otherId = (other._nodeId !== undefined) ? other._nodeId : null;
        if (otherId === null) {
            return 1 | 32 | 4;
        }
        return _compareDocumentPosition(0, otherId);
    };

    document.append = function() {
        var frag = _wrapElement(_createFragment());
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined))
                ? arg : _wrapElement(_origCTN(String(arg)));
            frag.appendChild(node);
        }
        document.appendChild(frag);
    };
    document.prepend = function() {
        var frag = _wrapElement(_createFragment());
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined))
                ? arg : _wrapElement(_origCTN(String(arg)));
            frag.appendChild(node);
        }
        var first = document.firstChild;
        if (first) document.insertBefore(frag, first);
        else document.appendChild(frag);
    };
    document.replaceChildren = function() {
        var frag = _wrapElement(_createFragment());
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg !== null && typeof arg === 'object' && (arg._nodeId !== undefined || arg.nodeType !== undefined))
                ? arg : _wrapElement(_origCTN(String(arg)));
            frag.appendChild(node);
        }
        while (document.firstChild) {
            document.removeChild(document.firstChild);
        }
        document.appendChild(frag);
    };

