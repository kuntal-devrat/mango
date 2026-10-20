            get popoverTargetElement() {
                var targetId = this.getAttribute('popovertarget');
                if (!targetId || typeof document === 'undefined') return null;
                return document.getElementById(targetId);
            },
            set popoverTargetElement(el) {
                if (el && el.id) {
                    this.setAttribute('popovertarget', el.id);
                } else if (!el) {
                    this.removeAttribute('popovertarget');
                }
            },
            get popoverTargetAction() {
                var act = this.getAttribute('popovertargetaction');
                if (!act) return 'toggle';
                var lower = String(act).toLowerCase();
                return (lower === 'show' || lower === 'hide') ? lower : 'toggle';
            },
            set popoverTargetAction(val) {
                if (val) this.setAttribute('popovertargetaction', String(val).toLowerCase());
                else this.removeAttribute('popovertargetaction');
            },
            addEventListener: function(type, listener, options) {
                _addDOMEventListener(this, type, listener, options);
            },
            removeEventListener: function(type, listener, options) {
                _removeDOMEventListener(this, type, listener, options);
            },
            getBoundingClientRect: function() {
                var r = _getElementRect(this._nodeId) || [0, 0, 0, 0];
                var x = r[0];
                var y = r[1];
                var w = (r && r[2] > 0) ? r[2] : 0;
                var h = (r && r[3] > 0) ? r[3] : 0;
                if (w === 0 && h === 0 && this.isConnected) {
                    w = (this.style && this.style.width && parseFloat(this.style.width)) || (this.children && this.children.length > 0 ? 100 : 50);
                    h = (this.style && this.style.height && parseFloat(this.style.height)) || 20;
                }
                if (typeof globalThis !== 'undefined' && globalThis.DOMRect) {
                    try { return new globalThis.DOMRect(x, y, w, h); } catch(e) {}
                }
                var rect = {
                    x: x, y: y, top: y, left: x,
                    right: x + w, bottom: y + h,
                    width: w, height: h
                };
                rect.toJSON = function() {
                    return { x: this.x, y: this.y, top: this.top, left: this.left, right: this.right, bottom: this.bottom, width: this.width, height: this.height };
                };
                return rect;
            },
            getClientRects: function() {
                var r = this.getBoundingClientRect();
                if (typeof globalThis !== 'undefined' && globalThis.DOMRectList) {
                    return new globalThis.DOMRectList([r]);
                }
                return [r];
            },
            contains: function(other) {
                if (other === null || other === undefined) return false;
                var otherId = (typeof other === 'object') ? other._nodeId : other;
                if (otherId === undefined || otherId === null) return false;
                return _elementContains(this._nodeId, otherId);
            },
            compareDocumentPosition: function(other) {
                if (this === other) return 0;
                if (!other || typeof other !== 'object') {
                    throw new TypeError("Failed to execute 'compareDocumentPosition' on 'Node': parameter 1 is not of type 'Node'.");
                }
                var otherId = (other._nodeId !== undefined) ? other._nodeId : null;
                if (otherId === null) {
                    return 1 | 32 | 4;
                }
                return _compareDocumentPosition(this._nodeId, otherId);
            },
            hasChildNodes: function() { return (_getChildren(this._nodeId) || []).length > 0 || (_getChildNodeIds(this._nodeId) || []).length > 0; },
            normalize: function() {},
            getRootNode: function(options) {
                var cur = this;
                var composed = options && !!options.composed;
                while (cur) {
                    var parent = cur.parentNode || cur.parentElement;
                    if (!parent) {
                        if (composed && cur.host) {
                            cur = cur.host;
                            continue;
                        }
                        break;
                    }
                    cur = parent;
                }
                return cur || document;
            },
            querySelector: function(sel) {
                return _defaultQuerySelector(this, sel);
            },
            querySelectorAll: function(sel) {
                return _defaultQuerySelectorAll(this, sel);
            },
            getElementsByTagName: function(tag) {
                var self = this;
                return _createHTMLCollection(function() {
                    var ids = _querySelectorAll(self._nodeId, String(tag)) || [];
                    return ids.map(_wrapElement);
                });
            },
            getElementsByClassName: function(cls) {
                var self = this;
                return _createHTMLCollection(function() {
                    var ids = _querySelectorAll(self._nodeId, "." + String(cls)) || [];
                    return ids.map(_wrapElement);
                });
            },
            focus: function(options) {
                if (this.disabled) return;
                var prev = document.activeElement;
                if (prev === this) return;
                document.activeElement = this;
                _setFocusState(this._nodeId, true);
                if (prev && prev !== this) {
                    if (prev._nodeId) _setFocusState(prev._nodeId, false);
                    if (prev.dispatchEvent) {
                        var bEvt = (typeof globalThis.FocusEvent === 'function')
                            ? new globalThis.FocusEvent('blur', { bubbles: false, relatedTarget: this })
                            : { type: 'blur', target: prev, relatedTarget: this, bubbles: false };
                        prev.dispatchEvent(bEvt);
                        var foEvt = (typeof globalThis.FocusEvent === 'function')
                            ? new globalThis.FocusEvent('focusout', { bubbles: true, relatedTarget: this })
                            : { type: 'focusout', target: prev, relatedTarget: this, bubbles: true };
                        prev.dispatchEvent(foEvt);
                    }
                }
                var fEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('focus', { bubbles: false, relatedTarget: prev })
                    : { type: 'focus', target: this, relatedTarget: prev, bubbles: false };
                this.dispatchEvent(fEvt);
                var fiEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('focusin', { bubbles: true, relatedTarget: prev })
                    : { type: 'focusin', target: this, relatedTarget: prev, bubbles: true };
                this.dispatchEvent(fiEvt);
            },
            blur: function() {
                _setFocusState(this._nodeId, false);
                var wasActive = (document.activeElement === this);
                if (wasActive) {
                    document.activeElement = document.body || null;
                }
                var next = document.activeElement;
                var bEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('blur', { bubbles: false, relatedTarget: next })
                    : { type: 'blur', target: this, relatedTarget: next, bubbles: false };
                this.dispatchEvent(bEvt);
                var foEvt = (typeof globalThis.FocusEvent === 'function')
                    ? new globalThis.FocusEvent('focusout', { bubbles: true, relatedTarget: next })
                    : { type: 'focusout', target: this, relatedTarget: next, bubbles: true };
                this.dispatchEvent(foEvt);
            },
            click: function() {
                if (this.disabled) return;
                var evt;
                if (typeof globalThis.MouseEvent === 'function') {
                    try {
                        evt = new globalThis.MouseEvent('click', {
                            bubbles: true,
                            cancelable: true,
                            view: (typeof window !== 'undefined' ? window : null),
                            detail: 1
                        });
                    } catch(e) {
                        evt = { type: 'click', target: this, bubbles: true, cancelable: true, defaultPrevented: false };
                    }
                } else {
                    evt = { type: 'click', target: this, bubbles: true, cancelable: true, defaultPrevented: false };
                }
                var notPrevented = this.dispatchEvent(evt);
                if (notPrevented !== false && !evt.defaultPrevented) {
                    var tag = (this.tagName || '').toLowerCase();
                    if (tag === 'input') {
                        var itype = (this.type || this.getAttribute('type') || 'text').toLowerCase();
                        if (itype === 'checkbox') {
                            this.checked = !this.checked;
                        } else if (itype === 'radio') {
                            this.checked = true;
                        } else if (itype === 'submit') {
                            var f = this.form || this.closest('form');
                            if (f && f.submit) f.submit();
                        } else if (itype === 'reset') {
                            var f = this.form || this.closest('form');
                            if (f && f.reset) f.reset();
                        }
                    } else if (tag === 'button') {
                        var btype = (this.type || this.getAttribute('type') || 'submit').toLowerCase();
                        if (btype === 'submit') {
                            var f = this.form || this.closest('form');
                            if (f && f.submit) f.submit();
                        } else if (btype === 'reset') {
                            var f = this.form || this.closest('form');
                            if (f && f.reset) f.reset();
                        }
                    }
                    if (this.popoverTargetElement) {
                        var targetEl = this.popoverTargetElement;
                        var act = this.popoverTargetAction;
                        if (act === 'show') {
                            try { targetEl.showPopover(); } catch(e) {}
                        } else if (act === 'hide') {
                            try { targetEl.hidePopover(); } catch(e) {}
                        } else {
                            try { targetEl.togglePopover(); } catch(e) {}
                        }
                    }
                }
            },
            hasAttributes: function() { return (this.getAttributeNames ? this.getAttributeNames() : []).length > 0; },
            getAttributeNames: function() { return _getAttributeNames(this._nodeId) || []; },
            get attributes() {
                var self = this;
                var names = this.getAttributeNames();
                var list = [];
                for (var i = 0; i < names.length; i++) {
                    var n = names[i];
                    var v = self.getAttribute ? (self.getAttribute(n) || '') : '';
                    var attrNode = {
                        name: n,
                        value: v,
                        specified: true,
                        ownerElement: self,
                        nodeName: n,
                        nodeValue: v
                    };
                    list.push(attrNode);
                    list[n] = attrNode;
                }
                list.item = function(index) { return this[index] || null; };
                list.getNamedItem = function(name) {
                    return self.getAttributeNode ? self.getAttributeNode(name) : (this[name] || null);
                };
                list.setNamedItem = function(attr) {
                    if (attr && attr.name && self.setAttribute) self.setAttribute(attr.name, attr.value || '');
                };
                list.removeNamedItem = function(name) {
                    var old = self.getAttributeNode ? self.getAttributeNode(name) : null;
                    if (self.removeAttribute) self.removeAttribute(name);
                    return old;
                };
                return list;
            },
            getElementsByName: function(name) {
                var self = this;
                return _createNodeList(function() {
                    var ids = _querySelectorAll(self._nodeId, '[name="' + String(name) + '"]') || [];
                    return ids.map(_wrapElement);
                });
            },
            getAttributeNode: function(name) {
                var v = _getAttribute(this._nodeId, name);
                return v === null ? null : { name: name, value: v, specified: true };
            },
            toggleAttribute: function(name, force) {
                var has = this.hasAttribute(name);
                var shouldAdd = (force === undefined) ? !has : !!force;
                if (shouldAdd) this.setAttribute(name, '');
                else this.removeAttribute(name);
                return shouldAdd;
            },
            setPointerCapture: function() {},
            releasePointerCapture: function() {},
            get parentNode() {
                var p = (typeof _getParentNode === 'function') ? _getParentNode(this._nodeId) : -1;
                if (p === -1) return null;
                if (p === 0) return document;
                return _wrapElement(p);
            },
            get parentElement() {
                var p = (typeof _getParentNode === 'function') ? _getParentNode(this._nodeId) : -1;
                if (p <= 0) return null;
                return _wrapElement(p);
            },
            get children() {
                var self = this;
                return _createHTMLCollection(function() {
                    var arr = _getChildren(self._nodeId) || [];
                    return arr.map(_wrapElement);
                });
            },
            get childNodes() {
                var self = this;
                return _createNodeList(function() {
                    var arr = _getChildNodeIds(self._nodeId) || [];
                    return arr.map(_wrapElement);
                });
            },
            get firstChild() {
                var id = _getFirstChild(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get lastChild() {
                var id = _getLastChild(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get nextSibling() {
                var id = _getNextSibling(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get previousSibling() {
                var id = _getPreviousSibling(this._nodeId);
                return id !== null ? _wrapElement(id) : null;
            },
            get childElementCount() {
                return (_getChildren(this._nodeId) || []).length;
            },
            get firstElementChild() {
                var c = _getFirstElementChild(this._nodeId);
                return c !== null ? _wrapElement(c) : null;
            },
            get lastElementChild() {
                var c = _getLastElementChild(this._nodeId);
                return c !== null ? _wrapElement(c) : null;
            },
            get nextElementSibling() {
                var s = _getNextElementSibling(this._nodeId);
                return s !== null ? _wrapElement(s) : null;
            },
            get previousElementSibling() {
                var s = _getPreviousElementSibling(this._nodeId);
                return s !== null ? _wrapElement(s) : null;
            },
            get offsetWidth() { return Math.round(this.getBoundingClientRect().width); },
            get offsetHeight() { return Math.round(this.getBoundingClientRect().height); },
            get offsetParent() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var cs = window.getComputedStyle(this);
                    if (cs && cs.position === 'fixed') return null;
                }
                if (this.tagName === 'BODY' || this.tagName === 'HTML') return null;
                var cur = this.parentElement;
                while (cur && cur.tagName !== 'BODY' && cur.tagName !== 'HTML') {
                    if (typeof window !== 'undefined' && window.getComputedStyle) {
                        var s = window.getComputedStyle(cur);
                        if (s && s.position && s.position !== 'static') return cur;
                    }
                    cur = cur.parentElement;
                }
                if (cur && cur.tagName === 'BODY') return cur;
                return null;
            },
            get offsetLeft() {
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().left - p.getBoundingClientRect().left);
                return Math.round(this.getBoundingClientRect().left);
            },
            get offsetTop() {
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().top - p.getBoundingClientRect().top);
                return Math.round(this.getBoundingClientRect().top);
            },
            get clientWidth() {
                var bl = 0, br = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    bl = parseFloat(s.borderLeftWidth) || 0;
                    br = parseFloat(s.borderRightWidth) || 0;
                }
                return Math.max(0, Math.round(this.getBoundingClientRect().width - bl - br));
            },
            get clientHeight() {
                var bt = 0, bb = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    bt = parseFloat(s.borderTopWidth) || 0;
                    bb = parseFloat(s.borderBottomWidth) || 0;
                }
                return Math.max(0, Math.round(this.getBoundingClientRect().height - bt - bb));
            },
            get clientTop() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderTopWidth) || 0);
                }
                return 0;
            },
            get clientLeft() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderLeftWidth) || 0);
                }
                return 0;
            },
            get scrollWidth() { return Math.max(this.clientWidth, Math.round(this.getBoundingClientRect().width)); },
            get scrollHeight() { return Math.max(this.clientHeight, Math.round(this.getBoundingClientRect().height)); },
            get scrollTop() { return this._scrollTop || 0; },
            set scrollTop(v) {
                this._scrollTop = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {
                    if (typeof window !== 'undefined') window.scrollY = this._scrollTop;
                }
            },
            get scrollLeft() { return this._scrollLeft || 0; },
            set scrollLeft(v) {
                this._scrollLeft = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {
                    if (typeof window !== 'undefined') window.scrollX = this._scrollLeft;
                }
            },
            scrollTo: function(x, y) {
                if (typeof x === 'object' && x !== null) {
                    if (x.left !== undefined) this.scrollLeft = x.left;
                    if (x.top !== undefined) this.scrollTop = x.top;
                } else {
                    if (x !== undefined) this.scrollLeft = x;
                    if (y !== undefined) this.scrollTop = y;
                }
            },
            scrollBy: function(x, y) {
                if (typeof x === 'object' && x !== null) {
                    if (x.left !== undefined) this.scrollLeft += x.left;
                    if (x.top !== undefined) this.scrollTop += x.top;
                } else {
                    if (x !== undefined) this.scrollLeft += x;
                    if (y !== undefined) this.scrollTop += y;
                }
            },
            scroll: function(x, y) {
                this.scrollTo(x, y);
            },
            scrollIntoView: function(options) {
                var rect = this.getBoundingClientRect();
                var alignToTop = (options === undefined || options === true || (typeof options === 'object' && options.block !== 'end'));
                var top = (window.scrollY || 0) + (alignToTop ? rect.top : (rect.bottom - ((typeof window !== 'undefined' && window.innerHeight) || 600)));
                var left = (window.scrollX || 0) + rect.left;
                var behavior = (typeof options === 'object' && options.behavior) || 'auto';
                if (typeof window.scrollTo === 'function') {
                    window.scrollTo({ top: top, left: left, behavior: behavior });
                }
            },
            cloneNode: function(deep) {
                var cloneId = _cloneNode(this._nodeId, !!deep);
                var cloned = _wrapElement(cloneId);
                if (cloned && typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                    globalThis._upgradeTree(cloned, null, null);
                }
                return cloned;
            },
            get isConnected() {
                var cur = this;
                while (cur) {
                    if (cur === document) return true;
                    var p = cur.parentElement || cur.parentNode || cur.host;
                    if (!p) break;
                    cur = p;
                }
                return false;
            },
            get ownerDocument() {
                return (this === document) ? null : document;
            },
            get polymerController() { return this._polymerController; },
            set polymerController(val) {
                this._polymerController = val;
            },
            matches: function(sel) {
                if (this._nodeId !== undefined && typeof _elementMatches === 'function') {
                    try {
                        return Boolean(_elementMatches(this._nodeId, String(sel)));
                    } catch(e) {}
                }
                return _jsElementMatches(this, sel);
            },
            webkitMatchesSelector: function(sel) { return this.matches(sel); },
            mozMatchesSelector: function(sel) { return this.matches(sel); },
            msMatchesSelector: function(sel) { return this.matches(sel); },
            oMatchesSelector: function(sel) { return this.matches(sel); },
            closest: function(sel) {
                var cur = this;
                while (cur && (cur.nodeType === 1 || cur.nodeType === 9)) {
                    if (cur.matches && cur.matches(sel)) return cur;
                    cur = cur.parentElement || cur.parentNode || cur.host;
                }
                return null;
            },
            // DOM insertion conveniences. `append`/`prepend` accept Node or string.
            _toNode: function(item) {
                if (item !== null && typeof item === 'object' && (item._nodeId !== undefined || item.nodeType !== undefined)) {
                    return item;
                }
                return _wrapElement(_origCTN(String(item)));
            },
            append: function() {
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                this.appendChild(frag);
            },
            prepend: function() {
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                var first = this.firstChild;
                if (first) {
                    this.insertBefore(frag, first);
                } else {
                    this.appendChild(frag);
                }
            },
            before: function() {
                var parent = this.parentElement || this.parentNode;
                if (!parent || parent === document) {
                    var pId = _getParent(this._nodeId);
                    if (pId === null) return;
                    parent = _wrapElement(pId);
                }
                if (!parent) return;
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                parent.insertBefore(frag, this);
            },
            after: function() {
                var parent = this.parentElement || this.parentNode;
                if (!parent || parent === document) {
                    var pId = _getParent(this._nodeId);
                    if (pId === null) return;
                    parent = _wrapElement(pId);
                }
                if (!parent) return;
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                parent.insertBefore(frag, this.nextSibling);
            },
            replaceWith: function() {
                var parent = this.parentElement || this.parentNode;
                if (!parent || parent === document) {
                    var pId = _getParent(this._nodeId);
                    if (pId === null) return;
                    parent = _wrapElement(pId);
                }
                if (!parent) return;
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                parent.insertBefore(frag, this);
                parent.removeChild(this);
            },
            replaceChildren: function() {
                var frag = _wrapElement(_createFragment());
                for (var i = 0; i < arguments.length; i++) {
                    frag.appendChild(this._toNode(arguments[i]));
                }
                while (this.firstChild) {
                    this.removeChild(this.firstChild);
                }
                this.appendChild(frag);
            },
            remove: function(index) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select' && arguments.length > 0 && typeof index === 'number') {
                    var opts = this.options || [];
                    var opt = opts[index];
                    if (opt) {
                        var pId = _getParent(opt._nodeId);
                        if (pId !== null) {
                            var pObj = _wrapElement(pId);
                            if (pObj && pObj.removeChild) pObj.removeChild(opt);
                            else {
                                _removeChild(pId, opt._nodeId);
                                _invokeDisconnectedCallback(opt);
                            }
                        }
                    }
                    return;
                }
                var parentId = _getParent(this._nodeId);
                if (parentId !== null) {
                    var parentObj = _wrapElement(parentId);
                    if (parentObj && parentObj.removeChild) {
                        parentObj.removeChild(this);
                    } else {
                        _removeChild(parentId, this._nodeId);
                        _invokeDisconnectedCallback(this);
                    }
                }
            },
            dispatchEvent: function(evt) {
                return _dispatchDOMEvent(this, evt);
            }
        };

        var tag = el.tagName ? el.tagName.toLowerCase() : '';
        var nt = el.nodeType;
        if (typeof globalThis !== 'undefined') {
            if (nt === 11 && globalThis.DocumentFragment) {
                Object.setPrototypeOf(el, globalThis.DocumentFragment.prototype);
            } else if (nt === 9 && globalThis.Document) {
                Object.setPrototypeOf(el, globalThis.Document.prototype);
            } else if (nt === 3 && globalThis.Text) {
                Object.setPrototypeOf(el, globalThis.Text.prototype);
            } else if (nt === 8 && globalThis.Comment) {
                Object.setPrototypeOf(el, globalThis.Comment.prototype);
            } else {
                var customCtor = (globalThis.customElements && globalThis.customElements.get) ? globalThis.customElements.get(tag) : null;
                if (customCtor && customCtor.prototype) {
                    Object.setPrototypeOf(el, customCtor.prototype);
                    if (!customCtor.prototype.hasOwnProperty('constructor')) {
                        try { customCtor.prototype.constructor = customCtor; } catch(e) {}
                    }
                } else {
                    var _tagToCtorName = {
                        'template': 'HTMLTemplateElement', 'slot': 'HTMLSlotElement', 'dialog': 'HTMLDialogElement',
                        'progress': 'HTMLProgressElement', 'meter': 'HTMLMeterElement', 'output': 'HTMLOutputElement',
                        'datalist': 'HTMLDataListElement', 'map': 'HTMLMapElement', 'area': 'HTMLAreaElement',
                        'object': 'HTMLObjectElement', 'embed': 'HTMLEmbedElement', 'ruby': 'HTMLRubyElement',
                        'img': 'HTMLImageElement', 'iframe': 'HTMLIFrameElement', 'canvas': 'HTMLCanvasElement',
                        'video': 'HTMLVideoElement', 'audio': 'HTMLAudioElement', 'input': 'HTMLInputElement',
                        'select': 'HTMLSelectElement', 'option': 'HTMLOptionElement', 'optgroup': 'HTMLOptGroupElement',
                        'form': 'HTMLFormElement', 'fieldset': 'HTMLFieldSetElement', 'legend': 'HTMLLegendElement',
                        'textarea': 'HTMLTextAreaElement', 'button': 'HTMLButtonElement', 'a': 'HTMLAnchorElement',
                        'div': 'HTMLDivElement', 'span': 'HTMLSpanElement', 'p': 'HTMLParagraphElement',
                        'h1': 'HTMLHeadingElement', 'h2': 'HTMLHeadingElement', 'h3': 'HTMLHeadingElement',
                        'h4': 'HTMLHeadingElement', 'h5': 'HTMLHeadingElement', 'h6': 'HTMLHeadingElement',
                        'ul': 'HTMLUListElement', 'ol': 'HTMLOListElement', 'li': 'HTMLLIElement',
                        'dl': 'HTMLDListElement', 'table': 'HTMLTableElement', 'tr': 'HTMLTableRowElement',
                        'td': 'HTMLTableCellElement', 'th': 'HTMLTableCellElement',
                        'tbody': 'HTMLTableSectionElement', 'thead': 'HTMLTableSectionElement', 'tfoot': 'HTMLTableSectionElement',
                        'col': 'HTMLTableColElement', 'colgroup': 'HTMLTableColElement', 'caption': 'HTMLTableCaptionElement',
                        'body': 'HTMLBodyElement', 'head': 'HTMLHeadElement', 'html': 'HTMLHtmlElement',
                        'script': 'HTMLScriptElement', 'style': 'HTMLStyleElement', 'link': 'HTMLLinkElement',
                        'meta': 'HTMLMetaElement', 'title': 'HTMLTitleElement', 'label': 'HTMLLabelElement',
                        'br': 'HTMLBRElement', 'hr': 'HTMLHRElement', 'pre': 'HTMLPreElement',
                        'details': 'HTMLDetailsElement', 'summary': 'HTMLSummaryElement', 'time': 'HTMLTimeElement',
                        'data': 'HTMLDataElement', 'source': 'HTMLSourceElement', 'picture': 'HTMLPictureElement',
                        'track': 'HTMLTrackElement', 'base': 'HTMLBaseElement', 'blockquote': 'HTMLQuoteElement',
                        'q': 'HTMLQuoteElement', 'del': 'HTMLModElement', 'ins': 'HTMLModElement'
                    };
                    var ctorName = _tagToCtorName[tag];
                    var ctor = ctorName ? globalThis[ctorName] : null;
                    if (ctor && ctor.prototype) {
                        Object.setPrototypeOf(el, ctor.prototype);
                    } else {
                        var customCtor = (globalThis.customElements && globalThis.customElements.get) ? globalThis.customElements.get(tag) : null;
                        if (customCtor && typeof customCtor === 'function') {
                            if (globalThis._upgradeCustomElement && !el._isCustomConstructed) {
                                globalThis._upgradeCustomElement(el, customCtor);
                            } else if (customCtor.prototype) {
                                Object.setPrototypeOf(el, customCtor.prototype);
                            }
                        } else if (globalThis.HTMLElement) {
                            Object.setPrototypeOf(el, globalThis.HTMLElement.prototype);
                        }
                    }
                }
            }
        }
        _wrapperCache[nodeId] = el;
        return el;
    }
