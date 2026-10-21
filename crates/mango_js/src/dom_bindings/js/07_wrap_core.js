    function _wrapElement(nodeId) {
        if (nodeId === null || nodeId === undefined) return null;
        if (_wrapperCache[nodeId]) return _wrapperCache[nodeId];
        var el = {
            _nodeId: nodeId,
            _listeners: {},
            get textContent() { return _getTextContent(this._nodeId); },
            set textContent(val) {
                var oldVal = _getTextContent(this._nodeId);
                var oldKids = this.childNodes ? Array.prototype.slice.call(this.childNodes) : [];
                for (var i = 0; i < oldKids.length; i++) {
                    _invokeDisconnectedCallback(oldKids[i]);
                    _clearSubtreeWrapperCache(oldKids[i]);
                }
                _setTextContent(this._nodeId, String(val));
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'characterData',
                        target: this,
                        oldValue: oldVal
                    });
                }
            },
            get innerText() { return this.textContent; },
            set innerText(val) { this.textContent = val; },
            get innerHTML() { return _getInnerHTML(this._nodeId); },
            set innerHTML(val) {
                var oldKids = this.childNodes ? Array.prototype.slice.call(this.childNodes) : [];
                for (var i = 0; i < oldKids.length; i++) {
                    _invokeDisconnectedCallback(oldKids[i]);
                    _clearSubtreeWrapperCache(oldKids[i]);
                }
                _setInnerHTML(this._nodeId, String(val));
                if (typeof globalThis !== 'undefined' && globalThis._upgradeTree) {
                    globalThis._upgradeTree(this, null, null);
                }
            },
            get outerHTML() { return _getOuterHTML(this._nodeId); },
            set outerHTML(val) { _setOuterHTML(this._nodeId, String(val)); },
            get nodeType() {
                if (this._isPI) return 7;
                return _getNodeType(this._nodeId);
            },
            get nodeName() {
                if (this._isPI) return this._target || '';
                return _getNodeName(this._nodeId);
            },
            get nodeValue() {
                if (this._nodeValue !== undefined) return this._nodeValue;
                var v = _getNodeValue(this._nodeId);
                if (v !== null && v !== undefined) return v;
                if (this.nodeType === 1) return _getTextContent(this._nodeId);
                return null;
            },
            set nodeValue(val) {
                var oldVal = this.nodeValue;
                var newVal = val === null ? '' : String(val);
                this._nodeValue = newVal;
                _setNodeValue(this._nodeId, newVal);
                if (this.nodeType === 1) {
                    _setTextContent(this._nodeId, newVal);
                }
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'characterData',
                        target: this,
                        oldValue: oldVal
                    });
                }
            },
            ELEMENT_NODE: 1,
            TEXT_NODE: 3,
            COMMENT_NODE: 8,
            DOCUMENT_NODE: 9,
            DOCUMENT_FRAGMENT_NODE: 11,
            get tagName() { return _getTagName(this._nodeId); },
            get localName() { var t = this.tagName; return t ? t.toLowerCase() : t; },
            get content() {
                var tag = (this.tagName || '').toUpperCase();
                if (tag === 'TEMPLATE') {
                    var fragId = _getTemplateContent(this._nodeId);
                    if (fragId !== null && fragId !== undefined) {
                        return _wrapElement(fragId);
                    }
                    return undefined;
                }
                if (tag === 'META') {
                    return this.getAttribute('content') || '';
                }
                return this._customContent;
            },
            set content(val) {
                var tag = (this.tagName || '').toUpperCase();
                if (tag === 'META') {
                    this.setAttribute('content', String(val));
                } else {
                    this._customContent = val;
                }
            },
            get dataset() {
                if (!this._dataset) {
                    var self = this;
                    var target = {};
                    var keys = _getDatasetKeys(this._nodeId) || [];
                    for (var i = 0; i < keys.length; i++) {
                        (function(k) {
                            Object.defineProperty(target, k, {
                                enumerable: true,
                                configurable: true,
                                get: function() { return _getDatasetValue(self._nodeId, k); },
                                set: function(v) { _setDatasetValue(self._nodeId, k, String(v)); }
                            });
                        })(keys[i]);
                    }
                    this._dataset = new Proxy(target, {
                        get: function(t, prop) {
                            if (typeof prop !== 'string') return t[prop];
                            if (prop in t) return t[prop];
                            return _getDatasetValue(self._nodeId, prop);
                        },
                        set: function(t, prop, value) {
                            if (typeof prop !== 'string') { t[prop] = value; return true; }
                            _setDatasetValue(self._nodeId, prop, String(value));
                            Object.defineProperty(t, prop, {
                                enumerable: true,
                                configurable: true,
                                get: function() { return _getDatasetValue(self._nodeId, prop); },
                                set: function(v) { _setDatasetValue(self._nodeId, prop, String(v)); }
                            });
                            return true;
                        },
                        deleteProperty: function(t, prop) {
                            if (typeof prop === 'string') _removeDatasetValue(self._nodeId, prop);
                            delete t[prop];
                            return true;
                        },
                        has: function(t, prop) {
                            return (prop in t) || _getDatasetValue(self._nodeId, String(prop)) !== undefined;
                        },
                        ownKeys: function(t) {
                            var keys = Reflect.ownKeys(t);
                            var dKeys = _getDatasetKeys(self._nodeId) || [];
                            for (var i = 0; i < dKeys.length; i++) {
                                if (keys.indexOf(dKeys[i]) === -1) keys.push(dKeys[i]);
                            }
                            return keys;
                        },
                        getOwnPropertyDescriptor: function(t, prop) {
                            if (typeof prop === 'string') {
                                var v = _getDatasetValue(self._nodeId, prop);
                                if (v !== undefined) {
                                    return {
                                        value: v,
                                        writable: true,
                                        enumerable: true,
                                        configurable: true
                                    };
                                }
                            }
                            return Object.getOwnPropertyDescriptor(t, prop);
                        }
                    });
                }
                return this._dataset;
            },
            get id() { return this.getAttribute("id") || ""; },
            set id(val) { this.setAttribute("id", val); },
            get className() { return this.getAttribute("class") || ""; },
            set className(val) { this.setAttribute("class", val); },
            get classList() {
                if (!this._classList) this._classList = _createClassList(this._nodeId, this);
                return this._classList;
            },
            get style() {
                if (!this._style) this._style = _createStyleObject(this._nodeId);
                return this._style;
            },
            get pathname() {
                if (this._customPathname !== undefined) return this._customPathname;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href)
                        ? window.location.href
                        : 'https://www.youtube.com/';
                    try {
                        return (new URL(raw, base)).pathname;
                    } catch(e) {
                        return raw.charAt(0) === '/' ? raw : '/' + raw;
                    }
                }
                return undefined;
            },
            set pathname(val) { this._customPathname = val; },
            get href() {
                if (this._customHref !== undefined) return this._customHref;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href)
                        ? window.location.href
                        : 'https://www.youtube.com/';
                    try {
                        return (new URL(raw, base)).href;
                    } catch(e) {
                        return raw;
                    }
                }
                return this.getAttribute ? (this.getAttribute('href') || '') : '';
            },
            set href(val) { this._customHref = val; if (this.setAttribute) this.setAttribute('href', String(val)); },
            get search() {
                if (this._customSearch !== undefined) return this._customSearch;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'https://www.youtube.com/';
                    try { return (new URL(raw, base)).search; } catch(e) { return ''; }
                }
                return undefined;
            },
            set search(val) { this._customSearch = val; },
            get hash() {
                if (this._customHash !== undefined) return this._customHash;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'https://www.youtube.com/';
                    try { return (new URL(raw, base)).hash; } catch(e) { return ''; }
                }
                return undefined;
            },
            set hash(val) { this._customHash = val; },
            get host() {
                if (this._customHost !== undefined) return this._customHost;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'https://www.youtube.com/';
                    try { return (new URL(raw, base)).host; } catch(e) { return ''; }
                }
                return this._host !== undefined ? this._host : undefined;
            },
            set host(val) { this._customHost = val; this._host = val; },
            get hostname() {
                if (this._customHostname !== undefined) return this._customHostname;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'https://www.youtube.com/';
                    try { return (new URL(raw, base)).hostname; } catch(e) { return ''; }
                }
                return undefined;
            },
            set hostname(val) { this._customHostname = val; },
            get port() {
                if (this._customPort !== undefined) return this._customPort;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'https://www.youtube.com/';
                    try { return (new URL(raw, base)).port; } catch(e) { return ''; }
                }
                return undefined;
            },
            set port(val) { this._customPort = val; },
            get protocol() {
                if (this._customProtocol !== undefined) return this._customProtocol;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'https://www.youtube.com/';
                    try { return (new URL(raw, base)).protocol; } catch(e) { return 'https:'; }
                }
                return undefined;
            },
            set protocol(val) { this._customProtocol = val; },
            get origin() {
                if (this._customOrigin !== undefined) return this._customOrigin;
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'a' || tag === 'area') {
                    var raw = this.getAttribute ? (this.getAttribute('href') || '') : '';
                    var base = (typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'https://www.youtube.com/';
                    try { return (new URL(raw, base)).origin; } catch(e) { return ''; }
                }
                return undefined;
            },
            set origin(val) { this._customOrigin = val; },
            // ── Form control surface ──
            get value() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') return this.textContent;
                if (tag === 'progress') {
                    var v = _getAttribute(this._nodeId, "value");
                    if (v === null) return -1;
                    var num = parseFloat(v);
                    return isNaN(num) ? 0 : Math.max(0, num);
                }
                if (tag === 'meter') {
                    var v = _getAttribute(this._nodeId, "value");
                    var num = parseFloat(v);
                    var min = this.min;
                    var max = this.max;
                    if (isNaN(num)) return min;
                    return Math.max(min, Math.min(max, num));
                }
                if (tag === 'select') {
                    var opts = this.options || [];
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].selected) {
                            return opts[i].value;
                        }
                    }
                    if (!this.multiple && opts.length > 0) {
                        return opts[0].value;
                    }
                    return "";
                }
                if (tag === 'option') {
                    var a = _getAttribute(this._nodeId, "value");
                    return a !== null ? a : (this.textContent || '').trim();
                }
                var v = _getAttribute(this._nodeId, "data-mango-value");
                if (v !== null) return v;
                var attr = _getAttribute(this._nodeId, "value");
                return attr === null ? "" : attr;
            },
            set value(val) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') {
                    this.textContent = (val === null || val === undefined) ? "" : String(val);
                    return;
                }
                if (tag === 'select') {
                    var targetVal = (val === null || val === undefined) ? "" : String(val);
                    var opts = this.options || [];
                    var matched = false;
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].value === targetVal) {
                            opts[i].selected = true;
                            matched = true;
                        } else if (!this.multiple) {
                            opts[i].selected = false;
                        }
                    }
                    if (!matched && !this.multiple) {
                        this.selectedIndex = -1;
                    }
                    return;
                }
                if (tag === 'option') {
                    this.setAttribute("value", (val === null || val === undefined) ? "" : String(val));
                    return;
                }
                if (_getAttribute(this._nodeId, "data-mango-default-value") === null) {
                    var initVal = _getAttribute(this._nodeId, "value");
                    _setAttribute(this._nodeId, "data-mango-default-value", initVal === null ? "" : initVal);
                }
                var str = (val === null || val === undefined) ? "" : String(val);
                _setAttribute(this._nodeId, "data-mango-value", str);
                _setAttribute(this._nodeId, "value", str);
            },
            get defaultValue() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') {
                    return this._defaultValue !== undefined ? this._defaultValue : this.textContent;
                }
                if (tag === 'textarea') {
                    var def = _getAttribute(this._nodeId, "data-mango-default-value");
                    return def !== null ? def : this.textContent;
                }
                var def = _getAttribute(this._nodeId, "data-mango-default-value");
                if (def !== null) return def;
                var a = _getAttribute(this._nodeId, "value");
                return a === null ? "" : a;
            },
            set defaultValue(val) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') {
                    this._defaultValue = String(val);
                    return;
                }
                _setAttribute(this._nodeId, "data-mango-default-value", String(val));
                _setAttribute(this._nodeId, "value", String(val));
            },
            get defaultChecked() {
                var def = this.getAttribute("data-mango-default-checked");
                if (def !== null) return def === "true";
                return this.hasAttribute("checked");
            },
            set defaultChecked(val) {
                this.setAttribute("data-mango-default-checked", val ? "true" : "false");
                if (val) this.setAttribute("checked", "");
                else this.removeAttribute("checked");
            },
            get checked() {
                var dyn = this.getAttribute("data-mango-checked");
                if (dyn !== null) return dyn === "true";
                return this.hasAttribute("checked");
            },
            set checked(val) {
                if (!this.hasAttribute("data-mango-default-checked")) {
                    this.setAttribute("data-mango-default-checked", this.hasAttribute("checked") ? "true" : "false");
                }
                this.setAttribute("data-mango-checked", val ? "true" : "false");
                if (val) this.setAttribute("checked", "");
                else this.removeAttribute("checked");
                this.dispatchEvent({ type: 'change' });
            },
            get indeterminate() { return this.getAttribute("data-mango-indeterminate") === "true"; },
            set indeterminate(val) { this.setAttribute("data-mango-indeterminate", val ? "true" : "false"); },
            get disabled() { return this.hasAttribute("disabled"); },
            set disabled(val) { if (val) this.setAttribute("disabled", ""); else this.removeAttribute("disabled"); },
            get required() { return this.hasAttribute("required"); },
            set required(val) { if (val) this.setAttribute("required", ""); else this.removeAttribute("required"); },
            get readOnly() { return this.hasAttribute("readonly"); },
            set readOnly(val) { if (val) this.setAttribute("readonly", ""); else this.removeAttribute("readonly"); },
            get placeholder() { var a = _getAttribute(this._nodeId, "placeholder"); return a === null ? "" : a; },
            set placeholder(val) { _setAttribute(this._nodeId, "placeholder", String(val)); },
            get name() { var a = _getAttribute(this._nodeId, "name"); return a === null ? "" : a; },
            set name(val) { _setAttribute(this._nodeId, "name", String(val)); },
            get pattern() { var p = this.getAttribute('pattern'); return p === null ? '' : p; },
            set pattern(val) { this.setAttribute('pattern', String(val)); },
            get type() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'output') return 'output';
                if (tag === 'fieldset') return 'fieldset';
                if (tag === 'select') return this.multiple ? 'select-multiple' : 'select-one';
                var t = _getAttribute(this._nodeId, "type");
                if (t !== null && t !== '') return String(t).toLowerCase();
                return tag === 'textarea' ? 'textarea' : 'text';
            },
            set type(val) { _setAttribute(this._nodeId, "type", String(val)); },
            get min() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'meter') {
                    var m = _getAttribute(this._nodeId, "min");
                    var num = parseFloat(m);
                    return isNaN(num) ? 0.0 : num;
                }
                var a = _getAttribute(this._nodeId, "min"); return a === null ? "" : a;
            },
            set min(val) { _setAttribute(this._nodeId, "min", String(val)); },
            get max() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'progress') {
                    var m = _getAttribute(this._nodeId, "max");
                    if (m === null) return 1.0;
                    var num = parseFloat(m);
                    return isNaN(num) || num <= 0 ? 1.0 : num;
                }
                if (tag === 'meter') {
                    var m = _getAttribute(this._nodeId, "max");
                    var num = parseFloat(m);
                    var min = this.min;
                    return isNaN(num) ? Math.max(min, 1.0) : Math.max(min, num);
                }
                var a = _getAttribute(this._nodeId, "max"); return a === null ? "" : a;
            },
            set max(val) { _setAttribute(this._nodeId, "max", String(val)); },
            get step() { var a = _getAttribute(this._nodeId, "step"); return a === null ? "" : a; },
            get low() {
                var l = _getAttribute(this._nodeId, "low");
                var num = parseFloat(l);
                var min = this.min;
                var max = this.max;
                if (isNaN(num)) return min;
                return Math.max(min, Math.min(max, num));
            },
            set low(val) { _setAttribute(this._nodeId, "low", String(val)); },
            get high() {
                var h = _getAttribute(this._nodeId, "high");
                var num = parseFloat(h);
                var min = this.min;
                var max = this.max;
                var low = this.low;
                if (isNaN(num)) return max;
                return Math.max(low, Math.min(max, num));
            },
            set high(val) { _setAttribute(this._nodeId, "high", String(val)); },
            get optimum() {
                var o = _getAttribute(this._nodeId, "optimum");
                var num = parseFloat(o);
                var min = this.min;
                var max = this.max;
                if (isNaN(num)) return min + (max - min) / 2.0;
                return Math.max(min, Math.min(max, num));
            },
            set optimum(val) { _setAttribute(this._nodeId, "optimum", String(val)); },
            get position() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'progress') {
                    var v = _getAttribute(this._nodeId, "value");
                    if (v === null) return -1;
                    var val = this.value;
                    var max = this.max;
                    return max > 0 ? Math.max(0, Math.min(1, val / max)) : -1;
                }
                return undefined;
            },
            get multiple() {
                var tag = (this.tagName || '').toLowerCase();
                return (tag === 'select' || tag === 'input') ? this.hasAttribute('multiple') : false;
            },
            set multiple(val) {
                if (val) this.setAttribute('multiple', '');
                else this.removeAttribute('multiple');
            },
            get size() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select' || tag === 'input') {
                    var s = this.getAttribute('size');
                    if (s === null) return tag === 'select' ? (this.multiple ? 4 : 1) : 20;
                    var n = parseInt(s, 10);
                    return isNaN(n) || n <= 0 ? (tag === 'select' ? 1 : 20) : n;
                }
                return undefined;
            },
            set size(val) {
                var n = parseInt(val, 10);
                this.setAttribute('size', String(isNaN(n) || n <= 0 ? 1 : n));
            },
            get selectedIndex() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select') {
                    var opts = this.options || [];
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].selected) return i;
                    }
                    if (!this.multiple && opts.length > 0) return 0;
                    return -1;
                }
                return -1;
            },
            set selectedIndex(idx) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select') {
                    var target = parseInt(idx, 10);
                    var opts = this.options || [];
                    for (var i = 0; i < opts.length; i++) {
                        opts[i].selected = (i === target);
                    }
                }
            },
            get selectedOptions() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'select') {
                    var opts = this.options || [];
                    var sel = [];
                    for (var i = 0; i < opts.length; i++) {
                        if (opts[i].selected) sel.push(opts[i]);
                    }
                    if (sel.length === 0 && !this.multiple && opts.length > 0) {
                        sel.push(opts[0]);
                    }
                    sel.item = function(i) { return sel[i] || null; };
                    return sel;
                }
                return undefined;
            },
            get text() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') return (this.textContent || '').trim();
                return this.textContent;
            },
            set text(val) {
                this.textContent = String(val);
            },
            get selected() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') {
                    var dyn = this.getAttribute('data-mango-selected');
                    if (dyn !== null) return dyn === 'true';
                    return this.hasAttribute('selected');
                }
                return false;
            },
            set selected(val) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') {
                    if (!this.hasAttribute("data-mango-default-selected")) {
                        this.setAttribute("data-mango-default-selected", this.hasAttribute("selected") ? "true" : "false");
                    }
                    var boolVal = !!val;
                    if (boolVal) {
                        this.setAttribute('data-mango-selected', 'true');
                        var p = this.parentElement;
                        while (p && (p.tagName || '').toLowerCase() !== 'select') {
                            p = p.parentElement;
                        }
                        if (p && !p.multiple) {
                            var all = p.options || [];
                            for (var i = 0; i < all.length; i++) {
                                if (all[i] !== this) {
                                    all[i].removeAttribute('selected');
                                    all[i].setAttribute('data-mango-selected', 'false');
                                }
                            }
                        }
                    } else {
                        this.removeAttribute('selected');
                        this.setAttribute('data-mango-selected', 'false');
                    }
                }
            },
