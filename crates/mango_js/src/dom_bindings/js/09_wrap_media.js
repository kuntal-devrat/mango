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
                var trans = _getTransitionSpec(this, 'overlay') || _getTransitionSpec(this, 'display');
                if (trans && trans.allowDiscrete && trans.durationMs > 0) {
                    var self = this;
                    setTimeout(function() {
                        self.removeAttribute('open');
                        self.removeAttribute('data-mango-modal');
                        self.dispatchEvent({ type: 'close', target: self });
                    }, trans.durationMs);
                    return;
                }
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
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area' || tag === 'form' || tag === 'base' || tag === 'link') {
                    return (typeof this.getAttribute === 'function') ? (this.getAttribute('target') || '') : '';
                }
                if (this._customTarget !== undefined) return this._customTarget;
                return (typeof this.getAttribute === 'function') ? (this.getAttribute('target') || '') : undefined;
            },
            set target(val) {
                if (this._isPI) { this._target = String(val); return; }
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area' || tag === 'form' || tag === 'base' || tag === 'link') {
                    if (typeof this.setAttribute === 'function') this.setAttribute('target', String(val));
                } else {
                    this._customTarget = val;
                }
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
                return (typeof _getIframeContentDocument === 'function') ? _getIframeContentDocument(this) : (this._contentDocument || null);
            },
            get contentWindow() {
                return (typeof _getIframeContentWindow === 'function') ? _getIframeContentWindow(this) : (this._contentWindow || null);
            },
            appendChild: function(child) {
                if (arguments.length < 1) throw new TypeError("Failed to execute 'appendChild' on 'Node': 1 argument required");
                return _defaultAppendChild(this, child);
            },
            removeChild: function(child) {
                return _defaultRemoveChild(this, child);
            },
            insertBefore: function(newChild, refChild) {
                if (arguments.length < 2) throw new TypeError("Failed to execute 'insertBefore' on 'Node': 2 arguments required");
                return _defaultInsertBefore(this, newChild, refChild);
            },
            replaceChild: function(newChild, oldChild) {
                if (arguments.length < 2) throw new TypeError("Failed to execute 'replaceChild' on 'Node': 2 arguments required");
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
                var lower = String(name).toLowerCase();
                if (lower === 'style' && this._style && !this._style._syncing) {
                    this._style._syncing = true;
                    try { this._style.cssText = String(value); } finally { this._style._syncing = false; }
                }
                if (lower === 'id') {
                    var idVal = String(value);
                    if (idVal && !(idVal in globalThis)) {
                        var self = this;
                        try {
                            Object.defineProperty(globalThis, idVal, {
                                get: function() { return self; },
                                configurable: true,
                                enumerable: false
                            });
                        } catch (e) {}
                    }
                }
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
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
                if (this._nsAttrs) delete this._nsAttrs[name];
                var lower = String(name).toLowerCase();
                if (lower === 'style' && this._style && !this._style._syncing) {
                    this._style._syncing = true;
                    try { this._style.cssText = ""; } finally { this._style._syncing = false; }
                }
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
            getAttributeNS: function(ns, localName) {
                var a = this.getAttributeNodeNS ? this.getAttributeNodeNS(ns, localName) : null;
                return a ? a.value : null;
            },
            setAttributeNS: function(ns, name, value) {
                if (arguments.length < 3) throw new TypeError("Failed to execute 'setAttributeNS': 3 arguments required");
                if (!this._nsAttrs) this._nsAttrs = {};
                var info = (typeof _validateAndExtractNamespace === 'function')
                    ? _validateAndExtractNamespace(ns, name)
                    : { namespaceURI: ns || null, prefix: (String(name).indexOf(':') !== -1 ? String(name).split(':')[0] : null), localName: (String(name).indexOf(':') !== -1 ? String(name).split(':')[1] : String(name)), qualifiedName: String(name) };
                this._nsAttrs[info.qualifiedName] = { namespaceURI: info.namespaceURI, prefix: info.prefix, localName: info.localName, value: String(value) };
                this.setAttribute(info.qualifiedName, value);
            },
            removeAttributeNS: function(ns, localName) {
                var a = this.getAttributeNodeNS ? this.getAttributeNodeNS(ns, localName) : null;
                if (a) {
                    if (this._nsAttrs) delete this._nsAttrs[a.name];
                    this.removeAttribute(a.name);
                }
            },
            hasAttributeNS: function(ns, localName) {
                return (this.getAttributeNodeNS ? this.getAttributeNodeNS(ns, localName) : null) !== null;
            },
            get shadowRoot() {
                if (this._shadowRootInstance !== undefined) {
                    return (this._shadowRootInstance && this._shadowRootInstance.mode === 'open') ? this._shadowRootInstance : null;
                }
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
                    this._shadowRootInstance = sr;
                    this._shadowRoot = sr;
                    return (mode === 'open') ? sr : null;
                }
                return null;
            },
            set shadowRoot(val) {
                this._shadowRootInstance = val;
                this._shadowRoot = val;
            },
            attachShadow: function(init) {
                if (!init || typeof init !== 'object') {
                    throw new TypeError("Failed to execute 'attachShadow' on 'Element': 1 argument required, but only 0 present.");
                }
                var mode = init.mode;
                if (mode !== 'open' && mode !== 'closed') {
                    throw new TypeError("Failed to execute 'attachShadow' on 'Element': Invalid shadow root mode.");
                }
                if (this._shadowRootInstance) {
                    return this._shadowRootInstance;
                }
                var srId = _attachShadow(this._nodeId, mode);
                var sr = (srId !== null && srId !== undefined) ? _wrapElement(srId) : _wrapElement(_createFragment());
                if (typeof globalThis.ShadowRoot === 'function' && globalThis.ShadowRoot.prototype) {
                    Object.setPrototypeOf(sr, globalThis.ShadowRoot.prototype);
                }
                sr.mode = mode;
                sr.host = this;
                this._shadowRootInstance = sr;
                this._shadowRoot = sr;
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

                var trans = _getTransitionSpec(this, 'overlay');
                if (trans && trans.stepEnd && trans.durationMs > 0) {
                    this._overlayTransition = 'in';
                    this.setAttribute('data-mango-popover-open', 'pending');
                    this.setAttribute(':popover-open', 'pending');
                    var self = this;
                    setTimeout(function() {
                        self._overlayTransition = 'done';
                        self.setAttribute('data-mango-popover-open', 'true');
                        self.setAttribute(':popover-open', 'true');
                        var te = (typeof TransitionEvent !== 'undefined')
                            ? new TransitionEvent('transitionend', { propertyName: 'overlay' })
                            : (typeof Event !== 'undefined' ? new Event('transitionend') : { type: 'transitionend' });
                        te.propertyName = 'overlay';
                        self.dispatchEvent(te);
                    }, trans.durationMs);
                } else {
                    this.setAttribute('data-mango-popover-open', 'true');
                    this.setAttribute(':popover-open', 'true');
                }

                var evtAfter = typeof ToggleEvent !== 'undefined' ? new ToggleEvent('toggle', { oldState: 'closed', newState: 'open' }) : new Event('toggle');
                evtAfter.oldState = 'closed'; evtAfter.newState = 'open';
                this.dispatchEvent(evtAfter);
            },
            hidePopover: function() {
                var isOpen = this.getAttribute('data-mango-popover-open') === 'true' || this.getAttribute('data-mango-popover-open') === 'pending';
                if (this.popover === null || !isOpen) {
                    var err = typeof DOMException !== 'undefined' ? new DOMException("Popover is not open", "InvalidStateError") : new Error("Popover is not open");
                    err.name = "InvalidStateError";
                    throw err;
                }
                var evtBefore = typeof ToggleEvent !== 'undefined' ? new ToggleEvent('beforetoggle', { oldState: 'open', newState: 'closed', cancelable: false }) : new Event('beforetoggle');
                evtBefore.oldState = 'open'; evtBefore.newState = 'closed';
                this.dispatchEvent(evtBefore);

                var trans = _getTransitionSpec(this, 'overlay');
                if (trans && (trans.allowDiscrete || trans.stepEnd) && trans.durationMs > 0) {
                    this._overlayTransition = 'out';
                    this.setAttribute('data-mango-popover-open', 'true');
                    this.setAttribute(':popover-open', 'true');
                    var self = this;
                    setTimeout(function() {
                        self._overlayTransition = null;
                        self.removeAttribute('data-mango-popover-open');
                        self.removeAttribute(':popover-open');
                        if (globalThis._popoverStack) {
                            var idx = globalThis._popoverStack.indexOf(self);
                            if (idx !== -1) globalThis._popoverStack.splice(idx, 1);
                        }
                    }, trans.durationMs);
                } else {
                    this.removeAttribute('data-mango-popover-open');
                    this.removeAttribute(':popover-open');
                    if (globalThis._popoverStack) {
                        var idx = globalThis._popoverStack.indexOf(this);
                        if (idx !== -1) globalThis._popoverStack.splice(idx, 1);
                    }
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
