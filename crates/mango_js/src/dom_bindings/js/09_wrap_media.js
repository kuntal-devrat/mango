            get isContentEditable() {
                var cur = this;
                while (cur) {
                    var ce = (cur.getAttribute) ? cur.getAttribute('contenteditable') : null;
                    if (ce !== null) {
                        var lower = ce.toLowerCase();
                        if (lower === '' || lower === 'true') return true;
                        if (lower === 'false') return false;
                    }
                    cur = cur.parentElement;
                }
                return false;
            },
            get draggable() {
                var v = this.getAttribute('draggable');
                return v !== null && v.toLowerCase() === 'true';
            },
            set draggable(val) {
                this.setAttribute('draggable', val ? 'true' : 'false');
            },
            // <dialog> surface
            get open() { return this.hasAttribute('open'); },
            set open(val) {
                if (val) this.setAttribute('open', '');
                else this.removeAttribute('open');
            },
            get returnValue() { return this._returnValue || ''; },
            set returnValue(val) { this._returnValue = String(val); },
            show: function() {
                this.setAttribute('open', '');
                this.removeAttribute('data-mango-modal');
            },
            showModal: function() {
                if (this.hasAttribute('open')) {
                    throw new Error("InvalidStateError: Dialog is already open");
                }
                this.setAttribute('data-mango-modal', 'true');
                this.setAttribute('open', '');
                this.focus();
            },
            close: function(retVal) {
                if (!this.hasAttribute('open')) return;
                if (retVal !== undefined) this.returnValue = String(retVal);
                this.removeAttribute('open');
                this.removeAttribute('data-mango-modal');
                this.dispatchEvent({ type: 'close', target: this });
            },
            // <datalist> & <select>
            get options() {
                if (this._customOptions !== undefined) {
                    return this._customOptions;
                }
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'datalist') {
                    return this.getElementsByTagName('option');
                }
                if (tag === 'select') {
                    var list = this.getElementsByTagName('option');
                    var self = this;
                    list.add = function(opt, before) { self.add(opt, before); };
                    list.remove = function(index) { self.remove(index); };
                    return list;
                }
                return undefined;
            },
            set options(val) {
                this._customOptions = val;
            },
            add: function(opt, before) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag !== 'select') return;
                if (before === undefined || before === null) {
                    this.appendChild(opt);
                } else if (typeof before === 'number') {
                    var ref = (this.options || [])[before];
                    if (ref) this.insertBefore(opt, ref);
                    else this.appendChild(opt);
                } else {
                    this.insertBefore(opt, before);
                }
            },
            get list() {
                if (this._customList !== undefined) {
                    return this._customList;
                }
                var listId = this.getAttribute('list');
                if (!listId || typeof document === 'undefined') return null;
                var el = document.getElementById(listId);
                return (el && el.tagName && el.tagName.toLowerCase() === 'datalist') ? el : null;
            },
            set list(val) {
                this._customList = val;
            },
            // <map> & <area>
            get areas() {
                if (this._customAreas !== undefined) {
                    return this._customAreas;
                }
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'map') {
                    return this.getElementsByTagName('area');
                }
                return undefined;
            },
            set areas(val) {
                this._customAreas = val;
            },
            get shape() { return this.getAttribute('shape') || 'rect'; },
            set shape(val) { this.setAttribute('shape', String(val)); },
            get coords() { return this.getAttribute('coords') || ''; },
            set coords(val) { this.setAttribute('coords', String(val)); },
            get alt() { return this.getAttribute('alt') || ''; },
            set alt(val) { this.setAttribute('alt', String(val)); },
            get target() {
                if (this._isPI) return this._target || '';
                return (typeof this.getAttribute === 'function') ? (this.getAttribute('target') || '') : '';
            },
            set target(val) {
                if (this._isPI) { this._target = String(val); return; }
                if (typeof this.setAttribute === 'function') this.setAttribute('target', String(val));
            },
            // <object> & <embed> and CharacterData / Comment
            get data() {
                if (this._customData !== undefined) {
                    return this._customData;
                }
                if (this.nodeType === 8 || this.nodeType === 3 || this.nodeType === 7 || this.nodeType === 4) {
                    return this.nodeValue || '';
                }
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'object') {
                    return (typeof this.getAttribute === 'function') ? (this.getAttribute('data') || '') : '';
                }
                return undefined;
            },
            set data(val) {
                if (this.nodeType === 8 || this.nodeType === 3 || this.nodeType === 7 || this.nodeType === 4) {
                    this.nodeValue = val === null ? "" : String(val);
                    return;
                }
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'object' && typeof val === 'string') {
                    if (typeof this.setAttribute === 'function') this.setAttribute('data', val);
                }
                this._customData = val;
            },
            substringData: function(offset, count) {
                if (arguments.length < 2) {
                    throw new TypeError("Failed to execute 'substringData': 2 arguments required, but only " + arguments.length + " present.");
                }
                var str = this.data || '';
                var uOffset = offset >>> 0;
                var uCount = count >>> 0;
                if (uOffset > str.length) {
                    var err = typeof DOMException !== 'undefined'
                        ? new DOMException("The index is not in the allowed range.", "IndexSizeError")
                        : new Error("IndexSizeError");
                    throw err;
                }
                return str.substring(uOffset, uOffset + uCount);
            },
            appendData: function(data) {
                if (arguments.length < 1) {
                    throw new TypeError("Failed to execute 'appendData': 1 argument required, but only " + arguments.length + " present.");
                }
                this.data = (this.data || '') + String(data);
            },
            insertData: function(offset, data) {
                if (arguments.length < 2) {
                    throw new TypeError("Failed to execute 'insertData': 2 arguments required, but only " + arguments.length + " present.");
                }
                this.replaceData(offset, 0, data);
            },
            deleteData: function(offset, count) {
                if (arguments.length < 2) {
                    throw new TypeError("Failed to execute 'deleteData': 2 arguments required, but only " + arguments.length + " present.");
                }
                this.replaceData(offset, count, '');
            },
            replaceData: function(offset, count, data) {
                if (arguments.length < 3) {
                    throw new TypeError("Failed to execute 'replaceData': 3 arguments required, but only " + arguments.length + " present.");
                }
                var str = this.data || '';
                var uOffset = offset >>> 0;
                var uCount = count >>> 0;
                if (uOffset > str.length) {
                    var err = typeof DOMException !== 'undefined'
                        ? new DOMException("The index is not in the allowed range.", "IndexSizeError")
                        : new Error("IndexSizeError");
                    throw err;
                }
                if (uOffset + uCount > str.length) {
                    uCount = str.length - uOffset;
                }
                var strData = String(data);
                this.data = str.substring(0, uOffset) + strData + str.substring(uOffset + uCount);
            },
            get htmlFor() { return this.getAttribute('for') || ''; },
            set htmlFor(val) { this.setAttribute('for', String(val)); },
            get contentDocument() {
                if (this.tagName && this.tagName.toLowerCase() === 'iframe') {
                    if (!this._contentDocument) {
                        var doc = (typeof document !== 'undefined' && document.implementation && typeof document.implementation.createHTMLDocument === 'function')
                            ? document.implementation.createHTMLDocument('')
                            : null;
                        if (!doc) {
                            var head = _wrapElement(_origCE('head'));
                            var body = _wrapElement(_origCE('body'));
                            var html = _wrapElement(_origCE('html'));
                            if (html && head) html.appendChild(head);
                            if (html && body) html.appendChild(body);
                            doc = {
                                title: '',
                                readyState: 'complete',
                                body: body,
                                head: head,
                                documentElement: html,
                                createElement: function(t) { return _wrapElement(_origCE(t)); },
                                createElementNS: function(ns, t) { return _wrapElement(_origCE(t)); },
                                createTextNode: function(t) { return _wrapElement(_origCTN(t)); },
                                createDocumentFragment: function() { return _wrapElement(_createFragment()); },
                                createComment: function(t) { return (typeof document !== 'undefined' && document.createComment) ? document.createComment(t) : null; },
                                getElementById: function(id) { return null; },
                                querySelector: function(s) { return null; },
                                querySelectorAll: function(s) { return _createNodeList(function() { return []; }); },
                                getElementsByTagName: function(t) { return _createHTMLCollection(function() { return []; }); },
                                getElementsByClassName: function(c) { return _createHTMLCollection(function() { return []; }); }
                            };
                        }
                        doc.readyState = 'complete';
                        doc.defaultView = this.contentWindow;
                        if (typeof globalThis.Document !== 'undefined') {
                            Object.setPrototypeOf(doc, globalThis.Document.prototype);
                        }
                        this._contentDocument = doc;
                    }
                    return this._contentDocument;
                }
                return null;
            },
            get contentWindow() {
                if (this.tagName && this.tagName.toLowerCase() === 'iframe') {
                    if (!this._contentWindow) {
                        var self = this;
                        var win = {
                            frameElement: self,
                            postMessage: function(msg, targetOrigin) {},
                            addEventListener: function() {},
                            removeEventListener: function() {},
                            dispatchEvent: function() { return true; }
                        };
                        Object.defineProperty(win, 'document', {
                            get: function() { return self.contentDocument; },
                            configurable: true
                        });
                        this._contentWindow = win;
                    }
                    return this._contentWindow;
                }
                return null;
            },
            appendChild: function(child) {
                return _defaultAppendChild(this, child);
            },
            removeChild: function(child) {
                return _defaultRemoveChild(this, child);
            },
            insertBefore: function(newChild, refChild) {
                return _defaultInsertBefore(this, newChild, refChild);
            },
            replaceChild: function(newChild, oldChild) {
                return _defaultReplaceChild(this, newChild, oldChild);
            },
            get __shady_native_appendChild() { return this._shady_native_appendChild || this.appendChild; },
            set __shady_native_appendChild(fn) { this._shady_native_appendChild = fn; },
            get __shady_native_removeChild() { return this._shady_native_removeChild || this.removeChild; },
            set __shady_native_removeChild(fn) { this._shady_native_removeChild = fn; },
            get __shady_native_insertBefore() { return this._shady_native_insertBefore || this.insertBefore; },
            set __shady_native_insertBefore(fn) { this._shady_native_insertBefore = fn; },
            get __shady_native_replaceChild() { return this._shady_native_replaceChild || this.replaceChild; },
            set __shady_native_replaceChild(fn) { this._shady_native_replaceChild = fn; },
            get __shady_native_cloneNode() { return this._shady_native_cloneNode || this.cloneNode; },
            set __shady_native_cloneNode(fn) { this._shady_native_cloneNode = fn; },
            get __shady_native_contains() { return this._shady_native_contains || this.contains; },
            set __shady_native_contains(fn) { this._shady_native_contains = fn; },
            get __shady_native_firstChild() { return this.firstChild; },
            get __shady_native_lastChild() { return this.lastChild; },
            get __shady_native_nextSibling() { return this.nextSibling; },
            get __shady_native_previousSibling() { return this.previousSibling; },
            get __shady_native_parentNode() { return this.parentNode; },
            get __shady_native_parentElement() { return this.parentElement; },
            get __shady_native_childNodes() { return this.childNodes; },
            insertAdjacentElement: function(position, element) {
                var pos = String(position).toLowerCase();
                var parentId = _getParent(this._nodeId);
                var parentObj = parentId !== null ? _wrapElement(parentId) : null;
                if (pos === 'beforeend') {
                    this.appendChild(element);
                } else if (pos === 'afterbegin') {
                    this.insertBefore(element, this.firstChild);
                } else if (pos === 'beforebegin') {
                    if (!parentObj) return null;
                    parentObj.insertBefore(element, this);
                } else if (pos === 'afterend') {
                    if (!parentObj) return null;
                    parentObj.insertBefore(element, this.nextSibling);
                } else {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.", "SyntaxError") : new SyntaxError("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.");
                    err.name = "SyntaxError";
                    throw err;
                }
                return element;
            },
            insertAdjacentHTML: function(position, html) {
                var pos = String(position).toLowerCase();
                if (pos !== 'beforebegin' && pos !== 'afterbegin' && pos !== 'beforeend' && pos !== 'afterend') {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.", "SyntaxError") : new SyntaxError("The value provided ('" + position + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.");
                    err.name = "SyntaxError";
                    throw err;
                }
                var container = _wrapElement(_createFragment());
                container.innerHTML = String(html);
                var parentId = _getParent(this._nodeId);
                var parentObj = parentId !== null ? _wrapElement(parentId) : null;
                if (pos === 'beforeend') {
                    this.appendChild(container);
                } else if (pos === 'afterbegin') {
                    this.insertBefore(container, this.firstChild);
                } else if (pos === 'beforebegin') {
                    if (!parentObj) return;
                    parentObj.insertBefore(container, this);
                } else if (pos === 'afterend') {
                    if (!parentObj) return;
                    parentObj.insertBefore(container, this.nextSibling);
                }
            },
            insertAdjacentText: function(position, text) {
                var node = _wrapElement(_origCTN(String(text)));
                return this.insertAdjacentElement(position, node);
            },
            setAttribute: function(name, value) {
                var oldVal = _getAttribute(this._nodeId, name);
                _setAttribute(this._nodeId, name, String(value));
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    var lower = String(name).toLowerCase();
                    if (lower === 'width' || lower === 'height') {
                        _canvasResize(this._nodeId, this.width, this.height);
                    }
                }
                _invokeAttributeChangedCallback(this, String(name), oldVal, String(value));
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'attributes',
                        target: this,
                        attributeName: String(name),
                        oldValue: oldVal
                    });
                }
            },
            getAttribute: function(name) { return _getAttribute(this._nodeId, name); },
            hasAttribute: function(name) { return _getAttribute(this._nodeId, name) !== null; },
            removeAttribute: function(name) {
                var oldVal = _getAttribute(this._nodeId, name);
                _removeAttribute(this._nodeId, name);
                if (oldVal !== null) {
                    _invokeAttributeChangedCallback(this, String(name), oldVal, null);
                }
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'attributes',
                        target: this,
                        attributeName: String(name),
                        oldValue: oldVal
                    });
                }
            },
            getAttributeNS: function(ns, name) { return this.getAttribute(name); },
            setAttributeNS: function(ns, name, value) { this.setAttribute(name, value); },
            removeAttributeNS: function(ns, name) { this.removeAttribute(name); },
            hasAttributeNS: function(ns, name) { return this.hasAttribute(name); },
            get shadowRoot() {
                if (this._shadowRoot !== undefined) return this._shadowRoot;
                var srId = (typeof _getShadowRoot === 'function') ? _getShadowRoot(this._nodeId) : null;
                if (srId) {
                    var mode = (typeof _getShadowRootMode === 'function') ? _getShadowRootMode(this._nodeId) : 'open';
                    var sr = _wrapElement(srId);
                    sr.mode = mode || 'open';
                    sr.host = this;
                    sr._adoptedStyleSheets = [];
                    Object.defineProperty(sr, 'adoptedStyleSheets', {
                        get: function() { return this._adoptedStyleSheets; },
                        set: function(sheets) {
                            if (!Array.isArray(sheets)) throw new TypeError("adoptedStyleSheets must be an Array");
                            for (var i = 0; i < sheets.length; i++) {
                                if (!(sheets[i] instanceof globalThis.CSSStyleSheet)) {
                                    throw new TypeError("Items in adoptedStyleSheets must be CSSStyleSheet instances");
                                }
                            }
                            this._adoptedStyleSheets = sheets.slice();
                        },
                        configurable: true,
                        enumerable: true
                    });
                    this._shadowRoot = (sr.mode === 'open') ? sr : null;
                    return this._shadowRoot;
                }
                return null;
            },
            set shadowRoot(val) {
                this._shadowRoot = val;
            },
            attachShadow: function(init) {
                var mode = (init && init.mode) || 'open';
                var srId = _attachShadow(this._nodeId, mode);
                var sr = (srId !== null && srId !== undefined) ? _wrapElement(srId) : _wrapElement(_createFragment());
                if (typeof globalThis.ShadowRoot === 'function' && globalThis.ShadowRoot.prototype) {
                    Object.setPrototypeOf(sr, globalThis.ShadowRoot.prototype);
                }
                sr.mode = mode;
                sr.host = this;
                this.shadowRoot = (sr.mode === 'open') ? sr : null;
                this._shadowRoot = (sr.mode === 'open') ? sr : null;
                sr._adoptedStyleSheets = [];
                Object.defineProperty(sr, 'adoptedStyleSheets', {
                    get: function() { return this._adoptedStyleSheets; },
                    set: function(sheets) {
                        if (!Array.isArray(sheets)) throw new TypeError("adoptedStyleSheets must be an Array");
                        for (var i = 0; i < sheets.length; i++) {
                            if (!(sheets[i] instanceof globalThis.CSSStyleSheet)) {
                                throw new TypeError("Items in adoptedStyleSheets must be CSSStyleSheet instances");
                            }
                        }
                        this._adoptedStyleSheets = sheets.slice();
                    },
                    configurable: true,
                    enumerable: true
                });
                return sr;
            },
            get assignedSlot() {
                var p = this.parentElement || this.parentNode;
                if (!p) return null;
                var sr = p.shadowRoot;
                if (!sr && typeof _getShadowRoot === 'function' && p._nodeId) {
                    var srId = _getShadowRoot(p._nodeId);
                    if (srId) sr = _wrapElement(srId);
                }
                if (!sr) return null;
                var slotName = this.getAttribute ? (this.getAttribute('slot') || '') : '';
                var slots = sr.querySelectorAll ? sr.querySelectorAll('slot') : [];
                for (var i = 0; i < slots.length; i++) {
                    var s = slots[i];
                    var sName = s.getAttribute('name') || '';
                    if (sName === slotName) {
                        return s;
                    }
                }
                return null;
            },
            assignedNodes: function(opts) {
                if (!this.tagName || this.tagName.toLowerCase() !== 'slot') {
                    return [];
                }
                var flatten = !!(opts && opts.flatten);
                var slotName = this.getAttribute ? (this.getAttribute('name') || '') : '';

                // Find the host element enclosing this slot
                var host = null;
                if (typeof _getShadowHost === 'function' && this._nodeId) {
                    var hid = _getShadowHost(this._nodeId);
                    if (hid) host = _wrapElement(hid);
                }
                if (!host) {
                    var curr = this.parentNode || this.parentElement;
                    while (curr) {
                        if (curr.host) { host = curr.host; break; }
                        curr = curr.parentNode || curr.parentElement;
                    }
                }

                var assigned = [];
                if (host && host.childNodes) {
                    for (var i = 0; i < host.childNodes.length; i++) {
                        var child = host.childNodes[i];
                        if (child.nodeType === 1) { // Element
                            var childSlot = (child.getAttribute && child.getAttribute('slot')) || '';
                            if (childSlot === slotName) {
                                assigned.push(child);
                            }
                        } else if (child.nodeType === 3) { // Text node
                            if (slotName === '') {
                                assigned.push(child);
                            }
                        }
                    }
                }

                if (flatten) {
                    var result = [];
                    var source = (assigned.length > 0) ? assigned : (this.childNodes ? Array.from(this.childNodes) : []);
                    for (var j = 0; j < source.length; j++) {
                        var node = source[j];
                        if (node.tagName && node.tagName.toLowerCase() === 'slot' && typeof node.assignedNodes === 'function') {
                            var nested = node.assignedNodes({ flatten: true });
                            for (var k = 0; k < nested.length; k++) {
                                result.push(nested[k]);
                            }
                        } else {
                            result.push(node);
                        }
                    }
                    return result;
                }

                return assigned;
            },
            assignedElements: function(opts) {
                return (this.assignedNodes(opts) || []).filter(function(n) { return n.nodeType === 1; });
            },
            toDataURL: function(type, quality) {
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    return _canvasToDataUrl(this._nodeId, type || 'image/png');
                }
                return "";
            },
            getContext: function(type) {
                if (type === '2d') {
                    if (!this._context2d) {
                        this._context2d = _createCanvasRenderingContext2D(this);
                    }
                    return this._context2d;
                }
                if (type === 'webgl' || type === 'experimental-webgl') {
                    if (!this._webgl) {
                        if (typeof globalThis !== 'undefined' && globalThis.WebGLRenderingContext) {
                            this._webgl = new globalThis.WebGLRenderingContext(this);
                        }
                    }
                    return this._webgl || null;
                }
                if (type === 'webgl2') {
                    if (!this._webgl2) {
                        if (typeof globalThis !== 'undefined' && globalThis.WebGL2RenderingContext) {
                            this._webgl2 = new globalThis.WebGL2RenderingContext(this);
                        }
                    }
                    return this._webgl2 || null;
                }
                if (type === 'webgpu') {
                    if (!this._webgpu) {
                        if (typeof globalThis !== 'undefined' && globalThis.GPUCanvasContext) {
                            this._webgpu = new globalThis.GPUCanvasContext(this);
                        }
                    }
                    return this._webgpu || null;
                }
                return null;
            },
            transferControlToOffscreen: function() {
                if (this._transferredToOffscreen) {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("Cannot transfer control from a canvas for more than one time.", "InvalidStateError") : new Error("Cannot transfer control from a canvas for more than one time.");
                    err.name = "InvalidStateError";
                    throw err;
                }
                if (this._context2d) {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("Cannot transfer control from a canvas that already has a context.", "InvalidStateError") : new Error("Cannot transfer control from a canvas that already has a context.");
                    err.name = "InvalidStateError";
                    throw err;
                }
                this._transferredToOffscreen = true;
                var off = new OffscreenCanvas(this.width || 300, this.height || 150);
                off._nodeId = this._nodeId;
                return off;
            },
            animate: function(keyframes, options) {
                return {
                    play: function() {},
                    pause: function() {},
                    cancel: function() {},
                    finish: function() {},
                    reverse: function() {},
                    currentTime: 0,
                    startTime: 0,
                    playbackRate: 1,
                    playState: 'running',
                    onfinish: null,
                    oncancel: null,
                    finished: (typeof Promise !== 'undefined') ? Promise.resolve(this) : null
                };
            },
            get popover() {
                var attr = this.getAttribute('popover');
                if (attr === null) return null;
                var lower = String(attr).toLowerCase();
                return (lower === 'manual') ? 'manual' : 'auto';
            },
            set popover(val) {
                if (val === null || val === undefined || val === false) {
                    this.removeAttribute('popover');
                } else if (typeof val === 'string' && val.toLowerCase() === 'manual') {
                    this.setAttribute('popover', 'manual');
                } else {
                    this.setAttribute('popover', 'auto');
                }
            },
            showPopover: function() {
                if (this.popover === null) {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("Not a popover", "InvalidStateError") : new Error("Not a popover");
                    err.name = "InvalidStateError";
                    throw err;
                }
                if (this.getAttribute('data-mango-popover-open') === 'true') {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("Popover is already open", "InvalidStateError") : new Error("Popover is already open");
                    err.name = "InvalidStateError";
                    throw err;
                }
                var evtBefore = typeof ToggleEvent !== 'undefined' ? new ToggleEvent('beforetoggle', { oldState: 'closed', newState: 'open', cancelable: false }) : new Event('beforetoggle');
                evtBefore.oldState = 'closed'; evtBefore.newState = 'open';
                this.dispatchEvent(evtBefore);

                globalThis._popoverStack = globalThis._popoverStack || [];
                if (this.popover === 'auto') {
                    var toClose = [];
                    for (var i = globalThis._popoverStack.length - 1; i >= 0; i--) {
                        var p = globalThis._popoverStack[i];
                        if (p && p.popover === 'auto' && !p.contains(this)) {
                            toClose.push(p);
                        }
                    }
                    for (var i = 0; i < toClose.length; i++) {
                        try { toClose[i].hidePopover(); } catch(e) {}
                    }
                }

                globalThis._popoverStack.push(this);
                if (typeof document !== 'undefined' && document.activeElement) {
                    this._previousFocus = document.activeElement;
                }
                this.setAttribute('data-mango-popover-open', 'true');
                this.setAttribute(':popover-open', 'true');

                var evtAfter = typeof ToggleEvent !== 'undefined' ? new ToggleEvent('toggle', { oldState: 'closed', newState: 'open' }) : new Event('toggle');
                evtAfter.oldState = 'closed'; evtAfter.newState = 'open';
                this.dispatchEvent(evtAfter);
            },
            hidePopover: function() {
                if (this.popover === null || this.getAttribute('data-mango-popover-open') !== 'true') {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("Popover is not open", "InvalidStateError") : new Error("Popover is not open");
                    err.name = "InvalidStateError";
                    throw err;
                }
                var evtBefore = typeof ToggleEvent !== 'undefined' ? new ToggleEvent('beforetoggle', { oldState: 'open', newState: 'closed', cancelable: false }) : new Event('beforetoggle');
                evtBefore.oldState = 'open'; evtBefore.newState = 'closed';
                this.dispatchEvent(evtBefore);

                this.removeAttribute('data-mango-popover-open');
                this.removeAttribute(':popover-open');

                if (globalThis._popoverStack) {
                    var idx = globalThis._popoverStack.indexOf(this);
                    if (idx !== -1) globalThis._popoverStack.splice(idx, 1);
                }

                if (this._previousFocus && typeof this._previousFocus.focus === 'function') {
                    try { this._previousFocus.focus(); } catch(e) {}
                    this._previousFocus = null;
                }

                var evtAfter = typeof ToggleEvent !== 'undefined' ? new ToggleEvent('toggle', { oldState: 'open', newState: 'closed' }) : new Event('toggle');
                evtAfter.oldState = 'open'; evtAfter.newState = 'closed';
                this.dispatchEvent(evtAfter);
            },
            togglePopover: function(force) {
                var isOpen = this.getAttribute('data-mango-popover-open') === 'true';
                var shouldOpen = force !== undefined ? !!force : !isOpen;
                if (shouldOpen) {
                    if (!isOpen) this.showPopover();
                } else {
                    if (isOpen) this.hidePopover();
                }
                return shouldOpen;
            },
