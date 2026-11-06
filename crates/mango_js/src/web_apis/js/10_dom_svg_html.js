    if (globalThis.HTMLAnchorElement) _setupHyperlinkUtils(globalThis.HTMLAnchorElement.prototype);
    if (globalThis.HTMLAreaElement) _setupHyperlinkUtils(globalThis.HTMLAreaElement.prototype);

    // HTMLSelectElement prototype surface
    globalThis.HTMLSelectElement.prototype.checkValidity = function() {
        return typeof this._computeValidity === 'function' ? this._computeValidity().valid : true;
    };
    globalThis.HTMLSelectElement.prototype.reportValidity = function() {
        return this.checkValidity();
    };
    globalThis.HTMLSelectElement.prototype.setCustomValidity = function(message) {
        if (message) {
            if (this.setAttribute) this.setAttribute('data-mango-custom-validity', String(message));
        } else {
            if (this.removeAttribute) this.removeAttribute('data-mango-custom-validity');
        }
    };
    globalThis.HTMLSelectElement.prototype.add = function(opt, before) {
        if (before === undefined || before === null) {
            this.appendChild(opt);
        } else if (typeof before === 'number') {
            var ref = (this.options || [])[before];
            if (ref) this.insertBefore(opt, ref);
            else this.appendChild(opt);
        } else {
            this.insertBefore(opt, before);
        }
    };
    globalThis.HTMLSelectElement.prototype.remove = function(index) {
        var opts = this.options || [];
        var opt = opts[index];
        if (opt && opt.parentNode) opt.parentNode.removeChild(opt);
    };
    Object.defineProperty(globalThis.HTMLSelectElement.prototype, 'value', {
        get: function() {
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
        },
        set: function(val) {
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
        },
        configurable: true,
        enumerable: true
    });

    // HTMLTextAreaElement prototype surface
    Object.defineProperty(globalThis.HTMLTextAreaElement.prototype, 'value', {
        get: function() {
            var v = this.getAttribute ? this.getAttribute("data-mango-value") : null;
            if (v !== null) return v;
            return this.textContent || "";
        },
        set: function(val) {
            var str = (val === null || val === undefined) ? "" : String(val);
            if (this.setAttribute) {
                this.setAttribute("data-mango-value", str);
            }
            this.textContent = str;
        },
        configurable: true,
        enumerable: true
    });

    // HTMLInputElement prototype surface
    Object.defineProperty(globalThis.HTMLInputElement.prototype, 'value', {
        get: function() {
            var v = this.getAttribute ? this.getAttribute("data-mango-value") : null;
            if (v !== null) return v;
            var attr = this.getAttribute ? this.getAttribute("value") : null;
            return attr === null ? "" : attr;
        },
        set: function(val) {
            if (this.getAttribute && this.getAttribute("data-mango-default-value") === null) {
                var initVal = this.getAttribute("value");
                if (this.setAttribute) this.setAttribute("data-mango-default-value", initVal === null ? "" : initVal);
            }
            var str = (val === null || val === undefined) ? "" : String(val);
            if (this.setAttribute) {
                this.setAttribute("data-mango-value", str);
                this.setAttribute("value", str);
            }
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(globalThis.HTMLInputElement.prototype, 'checked', {
        get: function() {
            var dyn = this.getAttribute ? this.getAttribute("data-mango-checked") : null;
            if (dyn !== null) return dyn === "true";
            return this.hasAttribute ? this.hasAttribute("checked") : false;
        },
        set: function(val) {
            if (this.hasAttribute && !this.hasAttribute("data-mango-default-checked")) {
                this.setAttribute("data-mango-default-checked", this.hasAttribute("checked") ? "true" : "false");
            }
            if (this.setAttribute) {
                this.setAttribute("data-mango-checked", val ? "true" : "false");
                if (val) this.setAttribute("checked", "");
                else if (this.removeAttribute) this.removeAttribute("checked");
            }
            if (val && (this.type || '').toLowerCase() === 'radio' && this.name) {
                var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
                if (doc && doc.querySelectorAll) {
                    var group = doc.querySelectorAll('input[type="radio"][name="' + this.name + '"]');
                    for (var gi = 0; gi < group.length; gi++) {
                        var other = group[gi];
                        if (other !== this) {
                            if (other.setAttribute) other.setAttribute("data-mango-checked", "false");
                            if (other.removeAttribute) other.removeAttribute("checked");
                        }
                    }
                }
            }
            if (this.dispatchEvent) {
                this.dispatchEvent({ type: 'change' });
            }
        },
        configurable: true,
        enumerable: true
    });

    ['HTMLInputElement', 'HTMLButtonElement', 'HTMLSelectElement', 'HTMLTextAreaElement', 'HTMLOptionElement', 'HTMLOptGroupElement', 'HTMLFieldSetElement'].forEach(function(cls) {
        if (globalThis[cls] && globalThis[cls].prototype) {
            Object.defineProperty(globalThis[cls].prototype, 'disabled', {
                get: function() { return this.hasAttribute ? this.hasAttribute('disabled') : false; },
                set: function(val) {
                    if (val) {
                        if (this.setAttribute) this.setAttribute('disabled', '');
                    } else {
                        if (this.removeAttribute) this.removeAttribute('disabled');
                    }
                },
                configurable: true,
                enumerable: true
            });
        }
    });
    globalThis.HTMLInputElement.prototype.stepUp = function(n) {
        var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
        var stepAttr = this.getAttribute ? this.getAttribute('step') : null;
        if (stepAttr && stepAttr.toLowerCase() === 'any') return;
        var step = parseFloat(stepAttr);
        if (isNaN(step) || step <= 0) step = 1.0;
        var cur = parseFloat(this.value);
        if (isNaN(cur)) {
            var minVal = parseFloat(this.min);
            cur = isNaN(minVal) ? 0.0 : minVal;
        }
        var next = cur + count * step;
        var min = parseFloat(this.min);
        if (!isNaN(min) && next < min) next = min;
        var max = parseFloat(this.max);
        if (!isNaN(max) && next > max) next = max;
        var decimals = 0;
        var stepStr = String(step);
        var dotIdx = stepStr.indexOf('.');
        if (dotIdx >= 0) decimals = stepStr.length - dotIdx - 1;
        this.value = decimals > 0 ? next.toFixed(decimals) : String(Math.round(next));
    };
    globalThis.HTMLInputElement.prototype.stepDown = function(n) {
        var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
        this.stepUp(-count);
    };
    globalThis.HTMLInputElement.prototype.checkValidity = function() {
        var v = typeof this._computeValidity === 'function' ? this._computeValidity() : { valid: true };
        if (!v.valid && this.dispatchEvent) {
            this.dispatchEvent({ type: 'invalid', target: this });
        }
        return v.valid;
    };
    globalThis.HTMLInputElement.prototype.reportValidity = function() {
        return this.checkValidity();
    };
    globalThis.HTMLInputElement.prototype.setCustomValidity = function(message) {
        if (message) {
            if (this.setAttribute) this.setAttribute('data-mango-custom-validity', String(message));
        } else {
            if (this.removeAttribute) this.removeAttribute('data-mango-custom-validity');
        }
    };

    // HTMLFormElement prototype surface & HTMLFormControlsCollection
    function _getOwnerForm(el) {
        if (!el) return null;
        if (el.getAttribute) {
            var formId = el.getAttribute('form');
            if (formId && typeof document !== 'undefined' && document.getElementById) {
                var f = document.getElementById(formId);
                if (f && f.tagName && f.tagName.toLowerCase() === 'form') return f;
            }
        }
        var cur = el.parentElement;
        while (cur) {
            if (cur.tagName && cur.tagName.toLowerCase() === 'form') return cur;
            cur = cur.parentElement;
        }
        return null;
    }

    var _formPropDesc = {
        get: function() { return _getOwnerForm(this); },
        configurable: true,
        enumerable: true
    };
    if (globalThis.HTMLInputElement && globalThis.HTMLInputElement.prototype) {
        Object.defineProperty(globalThis.HTMLInputElement.prototype, 'form', _formPropDesc);
    }
    if (globalThis.HTMLSelectElement && globalThis.HTMLSelectElement.prototype) {
        Object.defineProperty(globalThis.HTMLSelectElement.prototype, 'form', _formPropDesc);
    }
    if (globalThis.HTMLTextAreaElement && globalThis.HTMLTextAreaElement.prototype) {
        Object.defineProperty(globalThis.HTMLTextAreaElement.prototype, 'form', _formPropDesc);
    }
    if (globalThis.HTMLButtonElement && globalThis.HTMLButtonElement.prototype) {
        Object.defineProperty(globalThis.HTMLButtonElement.prototype, 'form', _formPropDesc);
    }

    function HTMLFormControlsCollection(elements) {
        for (var i = 0; i < elements.length; i++) {
            this[i] = elements[i];
            var name = elements[i].getAttribute ? (elements[i].getAttribute('name') || elements[i].getAttribute('id')) : (elements[i].name || elements[i].id);
            if (name && !(name in this)) {
                this[name] = elements[i];
            }
        }
        this.length = elements.length;
    }
    HTMLFormControlsCollection.prototype.item = function(index) {
        return this[index] || null;
    };
    HTMLFormControlsCollection.prototype.namedItem = function(name) {
        return this[name] || null;
    };
    if (typeof Symbol !== 'undefined' && Symbol.iterator) {
        HTMLFormControlsCollection.prototype[Symbol.iterator] = Array.prototype[Symbol.iterator];
    }
    globalThis.HTMLFormControlsCollection = HTMLFormControlsCollection;

    Object.defineProperty(globalThis.HTMLFormElement.prototype, 'elements', {
        get: function() {
            var controls = [];
            if (this.querySelectorAll) {
                var list = this.querySelectorAll('button, input, select, textarea, fieldset, output');
                for (var i = 0; i < list.length; i++) {
                    controls.push(list[i]);
                }
            }
            return new HTMLFormControlsCollection(controls);
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(globalThis.HTMLFormElement.prototype, 'length', {
        get: function() {
            return this.elements.length;
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(globalThis.HTMLFormElement.prototype, 'action', {
        get: function() {
            var a = this.getAttribute ? this.getAttribute('action') : '';
            if (a) {
                try {
                    return new URL(a, (typeof location !== 'undefined' && location.href) ? location.href : 'about:blank').href;
                } catch(e) {
                    return String(a);
                }
            }
            return (typeof location !== 'undefined' && location.href) ? location.href : '';
        },
        set: function(v) {
            if (this.setAttribute) this.setAttribute('action', String(v));
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(globalThis.HTMLFormElement.prototype, 'method', {
        get: function() {
            var m = this.getAttribute ? this.getAttribute('method') : '';
            return (m || 'get').toLowerCase();
        },
        set: function(v) {
            if (this.setAttribute) this.setAttribute('method', String(v));
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(globalThis.HTMLFormElement.prototype, 'name', {
        get: function() {
            return this.getAttribute ? (this.getAttribute('name') || '') : '';
        },
        set: function(v) {
            if (this.setAttribute) this.setAttribute('name', String(v));
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(globalThis.HTMLFormElement.prototype, 'target', {
        get: function() {
            return this.getAttribute ? (this.getAttribute('target') || '') : '';
        },
        set: function(v) {
            if (this.setAttribute) this.setAttribute('target', String(v));
        },
        configurable: true,
        enumerable: true
    });

    globalThis.HTMLFormElement.prototype.checkValidity = function() {
        var elems = this.elements || [];
        var allValid = true;
        var firstInvalid = null;
        for (var i = 0; i < elems.length; i++) {
            var el = elems[i];
            if (el.willValidate && typeof el.checkValidity === 'function' && !el.checkValidity()) {
                if (!firstInvalid) firstInvalid = el;
                allValid = false;
            }
        }
        return allValid;
    };
    globalThis.HTMLFormElement.prototype.reportValidity = function() {
        return this.checkValidity();
    };
    globalThis.HTMLFormElement.prototype.submit = function() {
        var action = this.action || ((typeof location !== 'undefined') ? location.href : '');
        var method = (this.method || 'get').toLowerCase();
        if (method === 'get' && action) {
            try {
                var url = new URL(action);
                if (typeof FormData !== 'undefined') {
                    var fd = new FormData(this);
                    for (var pair of fd.entries()) {
                        if (typeof pair[1] === 'string') {
                            url.searchParams.append(pair[0], pair[1]);
                        }
                    }
                }
                if (typeof globalThis._mangoNavigate === 'function') {
                    globalThis._mangoNavigate(url.href);
                } else if (typeof location !== 'undefined') {
                    location.href = url.href;
                }
            } catch(e) {
                if (typeof location !== 'undefined') location.href = action;
            }
        } else if (this.dispatchEvent) {
            this.dispatchEvent({ type: 'submit', target: this });
        }
    };
    globalThis.HTMLFormElement.prototype.reset = function() {
        var elems = this.elements || [];
        for (var i = 0; i < elems.length; i++) {
            var el = elems[i];
            var elTag = (el.tagName || '').toLowerCase();
            if (elTag === 'input') {
                var t = el.type;
                if (t === 'checkbox' || t === 'radio') {
                    el.checked = el.defaultChecked;
                } else {
                    el.value = el.defaultValue;
                }
            } else if (elTag === 'textarea') {
                el.value = el.defaultValue;
            } else if (elTag === 'select') {
                var opts = el.options || [];
                for (var j = 0; j < opts.length; j++) {
                    opts[j].selected = opts[j].defaultSelected;
                }
            }
        }
        if (this.dispatchEvent) {
            this.dispatchEvent({ type: 'reset', target: this });
        }
    };

    // HTMLDialogElement prototype surface
    globalThis.HTMLDialogElement.prototype.show = function() {
        if (this.setAttribute) this.setAttribute('open', '');
        if (this.removeAttribute) this.removeAttribute('data-mango-modal');
    };
    globalThis.HTMLDialogElement.prototype.showModal = function() {
        if (this.hasAttribute && this.hasAttribute('open')) {
            throw new Error('InvalidStateError: Dialog is already open');
        }
        if (this.setAttribute) {
            this.setAttribute('data-mango-modal', 'true');
            this.setAttribute('open', '');
        }
        if (this.focus) this.focus();
    };
    globalThis.HTMLDialogElement.prototype.close = function(retVal) {
        if (this.hasAttribute && !this.hasAttribute('open')) return;
        if (retVal !== undefined) this.returnValue = String(retVal);
        var trans = (typeof _getTransitionSpec === 'function') ? (_getTransitionSpec(this, 'overlay') || _getTransitionSpec(this, 'display')) : null;
        if (trans && trans.allowDiscrete && trans.durationMs > 0) {
            var self = this;
            setTimeout(function() {
                if (self.removeAttribute) {
                    self.removeAttribute('open');
                    self.removeAttribute('data-mango-modal');
                }
                if (self.dispatchEvent) self.dispatchEvent({ type: 'close', target: self });
            }, trans.durationMs);
            return;
        }
        if (this.removeAttribute) {
            this.removeAttribute('open');
            this.removeAttribute('data-mango-modal');
        }
        if (this.dispatchEvent) this.dispatchEvent({ type: 'close', target: this });
    };
    Object.defineProperty(globalThis.HTMLDialogElement.prototype, 'open', {
        get: function() { return this.hasAttribute ? this.hasAttribute('open') : false; },
        set: function(v) { if (v) { if (this.setAttribute) this.setAttribute('open', ''); } else { if (this.removeAttribute) this.removeAttribute('open'); } },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLDialogElement.prototype, 'returnValue', {
        get: function() { return this._returnValue || ''; },
        set: function(v) { this._returnValue = String(v); },
        configurable: true
    });

    // HTMLProgressElement prototype surface
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'value', {
        get: function() {
            var v = this.getAttribute ? this.getAttribute('value') : null;
            if (v === null) return -1;
            var n = parseFloat(v);
            return isNaN(n) ? 0 : Math.max(0, n);
        },
        set: function(v) { if (this.setAttribute) this.setAttribute('value', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'max', {
        get: function() {
            var m = this.getAttribute ? this.getAttribute('max') : null;
            if (m === null) return 1.0;
            var n = parseFloat(m);
            return isNaN(n) || n <= 0 ? 1.0 : n;
        },
        set: function(v) { if (this.setAttribute) this.setAttribute('max', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'position', {
        get: function() {
            var v = this.getAttribute ? this.getAttribute('value') : null;
            if (v === null) return -1;
            var val = this.value;
            var max = this.max;
            return max > 0 ? Math.max(0, Math.min(1, val / max)) : -1;
        },
        configurable: true
    });

    // HTMLMeterElement prototype surface
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'min', {
        get: function() { var m = this.getAttribute ? this.getAttribute('min') : null; var n = parseFloat(m); return isNaN(n) ? 0.0 : n; },
        set: function(v) { if (this.setAttribute) this.setAttribute('min', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'max', {
        get: function() { var m = this.getAttribute ? this.getAttribute('max') : null; var n = parseFloat(m); var min = this.min; return isNaN(n) ? Math.max(min, 1.0) : Math.max(min, n); },
        set: function(v) { if (this.setAttribute) this.setAttribute('max', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'value', {
        get: function() { var v = this.getAttribute ? this.getAttribute('value') : null; var n = parseFloat(v); var min = this.min; var max = this.max; if (isNaN(n)) return min; return Math.max(min, Math.min(max, n)); },
        set: function(v) { if (this.setAttribute) this.setAttribute('value', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'low', {
        get: function() { var l = this.getAttribute ? this.getAttribute('low') : null; var n = parseFloat(l); var min = this.min; var max = this.max; if (isNaN(n)) return min; return Math.max(min, Math.min(max, n)); },
        set: function(v) { if (this.setAttribute) this.setAttribute('low', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'high', {
        get: function() { var h = this.getAttribute ? this.getAttribute('high') : null; var n = parseFloat(h); var min = this.min; var max = this.max; var low = this.low; if (isNaN(n)) return max; return Math.max(low, Math.min(max, n)); },
        set: function(v) { if (this.setAttribute) this.setAttribute('high', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'optimum', {
        get: function() { var o = this.getAttribute ? this.getAttribute('optimum') : null; var n = parseFloat(o); var min = this.min; var max = this.max; if (isNaN(n)) return min + (max - min) / 2.0; return Math.max(min, Math.min(max, n)); },
        set: function(v) { if (this.setAttribute) this.setAttribute('optimum', String(v)); },
        configurable: true
    });

    // HTMLOutputElement prototype surface
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'value', {
        get: function() { return this.textContent || ''; },
        set: function(v) { this.textContent = String(v); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'defaultValue', {
        get: function() { return this._defaultValue !== undefined ? this._defaultValue : (this.textContent || ''); },
        set: function(v) { this._defaultValue = String(v); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'type', {
        get: function() { return 'output'; },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'name', {
        get: function() { return this.getAttribute ? (this.getAttribute('name') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('name', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'htmlFor', {
        get: function() { return this.getAttribute ? (this.getAttribute('for') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('for', String(v)); },
        configurable: true
    });

    // HTMLDataListElement prototype surface
    Object.defineProperty(globalThis.HTMLDataListElement.prototype, 'options', {
        get: function() { return this.getElementsByTagName ? this.getElementsByTagName('option') : []; },
        configurable: true
    });

    // HTMLInputElement.list surface
    Object.defineProperty(globalThis.HTMLInputElement.prototype, 'list', {
        get: function() {
            var listId = this.getAttribute ? this.getAttribute('list') : null;
            if (!listId || typeof document === 'undefined') return null;
            var el = document.getElementById(listId);
            return (el && el.tagName && el.tagName.toLowerCase() === 'datalist') ? el : null;
        },
        configurable: true
    });

    // HTMLMapElement prototype surface
    Object.defineProperty(globalThis.HTMLMapElement.prototype, 'name', {
        get: function() { return this.getAttribute ? (this.getAttribute('name') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('name', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLMapElement.prototype, 'areas', {
        get: function() { return this.getElementsByTagName ? this.getElementsByTagName('area') : []; },
        configurable: true
    });

    // HTMLAreaElement prototype surface
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'shape', {
        get: function() { return this.getAttribute ? (this.getAttribute('shape') || 'rect') : 'rect'; },
        set: function(v) { if (this.setAttribute) this.setAttribute('shape', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'coords', {
        get: function() { return this.getAttribute ? (this.getAttribute('coords') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('coords', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'alt', {
        get: function() { return this.getAttribute ? (this.getAttribute('alt') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('alt', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'target', {
        get: function() { return this.getAttribute ? (this.getAttribute('target') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('target', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'href', {
        get: function() { return this.getAttribute ? (this.getAttribute('href') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('href', String(v)); },
        configurable: true
    });

    // HTMLObjectElement prototype surface
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'data', {
        get: function() { return this.getAttribute ? (this.getAttribute('data') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('data', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'type', {
        get: function() { return this.getAttribute ? (this.getAttribute('type') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('type', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'name', {
        get: function() { return this.getAttribute ? (this.getAttribute('name') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('name', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'width', {
        get: function() { return this.getAttribute ? (this.getAttribute('width') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('width', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'height', {
        get: function() { return this.getAttribute ? (this.getAttribute('height') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('height', String(v)); },
        configurable: true
    });

    // HTMLEmbedElement prototype surface
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'src', {
        get: function() { return this.getAttribute ? (this.getAttribute('src') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('src', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'type', {
        get: function() { return this.getAttribute ? (this.getAttribute('type') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('type', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'width', {
        get: function() { return this.getAttribute ? (this.getAttribute('width') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('width', String(v)); },
        configurable: true
    });
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'height', {
        get: function() { return this.getAttribute ? (this.getAttribute('height') || '') : ''; },
        set: function(v) { if (this.setAttribute) this.setAttribute('height', String(v)); },
        configurable: true
    });

