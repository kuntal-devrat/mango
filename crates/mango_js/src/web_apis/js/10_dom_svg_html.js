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
            if (this.dispatchEvent) {
                this.dispatchEvent({ type: 'change' });
            }
        },
        configurable: true,
        enumerable: true
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

    // HTMLFormElement prototype surface
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
        if (this.dispatchEvent) {
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

    globalThis.HTMLMediaElement = function() {};
    globalThis.HTMLMediaElement.prototype = Object.create(globalThis.HTMLElement.prototype);
    globalThis.HTMLMediaElement.NETWORK_EMPTY    = 0;
    globalThis.HTMLMediaElement.NETWORK_IDLE     = 1;
    globalThis.HTMLMediaElement.NETWORK_LOADING  = 2;
    globalThis.HTMLMediaElement.NETWORK_NO_SOURCE= 3;
    globalThis.HTMLMediaElement.HAVE_NOTHING     = 0;
    globalThis.HTMLMediaElement.HAVE_METADATA    = 1;
    globalThis.HTMLMediaElement.HAVE_CURRENT_DATA= 2;
    globalThis.HTMLMediaElement.HAVE_FUTURE_DATA = 3;
    globalThis.HTMLMediaElement.HAVE_ENOUGH_DATA = 4;
    Object.assign(globalThis.HTMLMediaElement.prototype, globalThis.HTMLMediaElement);
    globalThis.HTMLMediaElement.prototype.play = function() {
        this._playing = true; this._paused = false;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'true');
        var self = this;
        setTimeout(function() {
            if (self.dispatchEvent) { self.dispatchEvent({ type:'play' }); self.dispatchEvent({ type:'playing' }); }
        }, 0);
        return (typeof Promise !== 'undefined') ? Promise.resolve() : null;
    };
    globalThis.HTMLMediaElement.prototype.pause = function() {
        this._playing = false; this._paused = true;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'false');
        var self = this;
        setTimeout(function() { if (self.dispatchEvent) self.dispatchEvent({ type:'pause' }); }, 0);
    };
    globalThis.HTMLMediaElement.prototype.load = function() {
        var self = this;
        setTimeout(function() {
            if (self.dispatchEvent) {
                self.dispatchEvent({ type:'loadstart' });
                self.dispatchEvent({ type:'loadedmetadata' });
                self.dispatchEvent({ type:'canplay' });
            }
        }, 0);
    };
    globalThis.HTMLMediaElement.prototype.canPlayType = function(type) {
        if (!type) return '';
        var t = String(type).toLowerCase();
        if (t.indexOf('mp4') !== -1 || t.indexOf('webm') !== -1 || t.indexOf('ogg') !== -1 ||
            t.indexOf('audio/mpeg') !== -1 || t.indexOf('audio/mp3') !== -1 ||
            t.indexOf('audio/wav') !== -1 || t.indexOf('audio/ogg') !== -1 || t.indexOf('audio/aac') !== -1) {
            return 'probably';
        }
        return 'maybe';
    };
    Object.defineProperties(globalThis.HTMLMediaElement.prototype, {
        currentTime: {
            get: function() {
                var t = (this.getAttribute && this.getAttribute('data-mango-time')) || this._currentTime || 0;
                var num = parseFloat(t);
                return isNaN(num) ? 0 : num;
            },
            set: function(val) {
                var num = Math.max(0, parseFloat(val) || 0);
                this._currentTime = num;
                if (this.setAttribute) this.setAttribute('data-mango-time', String(num));
                if (this.dispatchEvent) this.dispatchEvent({ type: 'timeupdate' });
            },
            configurable: true
        },
        duration: {
            get: function() {
                var d = (this.getAttribute && this.getAttribute('data-mango-duration')) || this._duration || 100;
                var num = parseFloat(d);
                return isNaN(num) ? 100 : num;
            },
            configurable: true
        },
        paused: {
            get: function() {
                if (this._paused !== undefined) return this._paused;
                var p = this.getAttribute ? this.getAttribute('data-mango-playing') : null;
                if (p === 'true') return false;
                if (p === 'false') return true;
                return !this.autoplay;
            },
            configurable: true
        },
        volume: {
            get: function() { return this._volume !== undefined ? this._volume : 1.0; },
            set: function(val) {
                this._volume = Math.max(0, Math.min(1, parseFloat(val) || 0));
                if (this.dispatchEvent) this.dispatchEvent({ type: 'volumechange' });
            },
            configurable: true
        },
        muted: {
            get: function() {
                if (this._muted !== undefined) return this._muted;
                return (this.hasAttribute && this.hasAttribute('muted')) ||
                    (this.getAttribute && this.getAttribute('data-mango-muted') === 'true');
            },
            set: function(val) {
                this._muted = !!val;
                if (this.setAttribute) {
                    if (val) { this.setAttribute('muted', ''); this.setAttribute('data-mango-muted', 'true'); }
                    else { if (this.removeAttribute) this.removeAttribute('muted'); this.setAttribute('data-mango-muted', 'false'); }
                }
                if (this.dispatchEvent) this.dispatchEvent({ type: 'volumechange' });
            },
            configurable: true
        },
        playbackRate: {
            get: function() { return this._playbackRate !== undefined ? this._playbackRate : 1.0; },
            set: function(val) {
                this._playbackRate = parseFloat(val) || 1.0;
                if (this.dispatchEvent) this.dispatchEvent({ type: 'ratechange' });
            },
            configurable: true
        }
    });
    globalThis.HTMLVideoElement = function() {};
    globalThis.HTMLVideoElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLAudioElement = function() {};
    globalThis.HTMLAudioElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLCanvasElement = function() {};
    globalThis.HTMLCanvasElement.prototype = Object.create(globalThis.HTMLElement.prototype);
    globalThis.CanvasRenderingContext2D = function() {};
    globalThis.ImageData = function(dataOrWidth, widthOrHeight, heightOpt) {
        if (typeof dataOrWidth === 'number') {
            this.width  = Math.max(1, Math.floor(dataOrWidth));
            this.height = (typeof widthOrHeight === 'number') ? Math.max(1, Math.floor(widthOrHeight)) : 1;
            this.data   = new Uint8ClampedArray(this.width * this.height * 4);
        } else {
            this.data   = (dataOrWidth instanceof Uint8ClampedArray) ? dataOrWidth : new Uint8ClampedArray(dataOrWidth);
            this.width  = Math.max(1, Math.floor(widthOrHeight));
            this.height = heightOpt !== undefined ? Math.max(1, Math.floor(heightOpt)) : Math.floor(this.data.length / (this.width * 4));
        }
    };

    globalThis.Document = function() {
        this.nodeType = 9; this.nodeName = "#document"; this._isXml = true;
        this.contentType = "application/xml"; this._childNodes = []; this._documentElement = null;
        this.location = null; this._characterSet = "UTF-8"; this.characterSet = "UTF-8";
        this.charset = "UTF-8"; this.inputEncoding = "UTF-8"; this.defaultView = null; this._defaultView = null;
    };
    globalThis.Document.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Document.prototype.constructor = globalThis.Document;
    globalThis.Document.prototype.createCDATASection = function(data) {
        var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
        if (!this._isXml && this.contentType === 'text/html') {
            throw new DomEx("The operation is not supported.", "NotSupportedError");
        }
        var str = String(data);
        if (str.indexOf("]]>") !== -1) {
            throw new DomEx("String contains invalid characters", "InvalidCharacterError");
        }
        var s = (typeof _origCTN === 'function' && typeof _wrapElement === 'function')
            ? _wrapElement(_origCTN(str))
            : Object.create(globalThis.CDATASection ? globalThis.CDATASection.prototype : (globalThis.Text ? globalThis.Text.prototype : Object.prototype));
        if (s) {
            if (globalThis.CDATASection) Object.setPrototypeOf(s, globalThis.CDATASection.prototype);
            s._isCDATA = true;
            s._nodeType = 4;
            s.nodeType = 4;
            s._nodeName = "#cdata-section";
            s.nodeName = "#cdata-section";
            s.ownerDocument = this;
            s._ownerDocument = this;
            s.data = str;
            s._data = str;
        }
        return s;
    };
    Object.defineProperty(globalThis.Document.prototype, 'URL', {
        get: function() {
            return (this._url !== undefined) ? this._url : ((globalThis.location && globalThis.location.href) ? globalThis.location.href : 'about:blank');
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'documentURI', {
        get: function() { return this.URL; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'baseURI', {
        get: function() {
            var baseEl = (typeof this.querySelector === 'function') ? this.querySelector('base[href]') : null;
            if (baseEl && typeof baseEl.getAttribute === 'function') {
                var href = baseEl.getAttribute('href');
                if (href) {
                    try {
                        return new URL(href, this.URL || 'about:blank').href;
                    } catch(e) {}
                }
            }
            return this.URL || 'about:blank';
        },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'defaultView', {
        get: function() {
            if (this._defaultView !== undefined) return this._defaultView;
            if (typeof document !== 'undefined' && this === document && typeof window !== 'undefined') return window;
            return null;
        },
        set: function(v) { this._defaultView = v; },
        configurable: true,
        enumerable: true
    });
    var _bodyProxiedEvents = [
        "onafterprint", "onbeforeprint", "onbeforeunload", "onblur", "onerror", "onfocus",
        "onhashchange", "onload", "onmessage", "onoffline", "ononline", "onpagehide", "onpageshow",
        "onpopstate", "onresize", "onscroll", "onstorage", "onunload"
    ];
    _bodyProxiedEvents.forEach(function(evt) {
        var desc = {
            get: function() {
                var doc = this.ownerDocument;
                var win = doc ? doc.defaultView : null;
                return win ? win[evt] : null;
            },
            set: function(fn) {
                var doc = this.ownerDocument;
                var win = doc ? doc.defaultView : null;
                if (win) { win[evt] = fn; }
            },
            configurable: true,
            enumerable: true
        };
        if (globalThis.HTMLBodyElement && globalThis.HTMLBodyElement.prototype) {
            Object.defineProperty(globalThis.HTMLBodyElement.prototype, evt, desc);
        }
        if (globalThis.HTMLFrameSetElement && globalThis.HTMLFrameSetElement.prototype) {
            Object.defineProperty(globalThis.HTMLFrameSetElement.prototype, evt, desc);
        }
    });
    if (globalThis.HTMLIFrameElement && globalThis.HTMLIFrameElement.prototype) {
        Object.defineProperty(globalThis.HTMLIFrameElement.prototype, 'contentWindow', {
            get: function() { return (typeof _getIframeContentWindow === 'function') ? _getIframeContentWindow(this) : (this._contentWindow || null); },
            configurable: true,
            enumerable: true
        });
        Object.defineProperty(globalThis.HTMLIFrameElement.prototype, 'contentDocument', {
            get: function() { return (typeof _getIframeContentDocument === 'function') ? _getIframeContentDocument(this) : (this._contentDocument || null); },
            configurable: true,
            enumerable: true
        });
    }
    Object.defineProperty(globalThis.Document.prototype, 'scrollingElement', {
        get: function() {
            if (this.compatMode === 'BackCompat') {
                return this.body || null;
            }
            return this.documentElement || this.body || null;
        },
        configurable: true,
        enumerable: true
    });
    globalThis.Document.prototype.__shady_getRootNode = function(opts) { return this; };
    globalThis.Document.prototype.getRootNode = function(opts) { return this; };
    if (typeof document !== 'undefined') {
        try {
            Object.setPrototypeOf(document, globalThis.Document.prototype);
        } catch(e) {}
        document.__shady_getRootNode = function(opts) { return document; };
        document.getRootNode = function(opts) { return document; };
    }

    function _locateNamespacePrefix(el, ns) {
        if (!el || el.nodeType !== 1) return null;
        if (el.namespaceURI === ns && el.prefix !== null) return el.prefix;
        if (el.attributes) {
            for (var i = 0; i < el.attributes.length; i++) {
                var a = el.attributes[i];
                var aName = a.name || '';
                var aPrefix = a.prefix;
                var aLocal = a.localName;
                if ((aPrefix === 'xmlns' || aName.indexOf('xmlns:') === 0) && a.value === ns) {
                    return aPrefix === 'xmlns' ? aLocal : aName.substring(6);
                }
            }
        }
        var parent = el.parentElement;
        return parent ? _locateNamespacePrefix(parent, ns) : null;
    }

    function _locateNamespaceURI(el, prefix) {
        if (!el || el.nodeType !== 1) return null;
        var p = (prefix === "" || prefix === undefined) ? null : prefix;
        if (el.namespaceURI !== null && el.prefix === p) return el.namespaceURI;
        if (el.attributes) {
            for (var i = 0; i < el.attributes.length; i++) {
                var a = el.attributes[i];
                var aNs = a.namespaceURI;
                var aPrefix = a.prefix;
                var aLocal = a.localName;
                var aName = a.name || '';
                var isXmlns = (aNs === 'http://www.w3.org/2000/xmlns/' || aPrefix === 'xmlns' || (!aPrefix && aLocal === 'xmlns') || aName === 'xmlns' || aName.indexOf('xmlns:') === 0);
                if (isXmlns) {
                    var pref = aPrefix === 'xmlns' ? aLocal : (aName.indexOf('xmlns:') === 0 ? aName.substring(6) : null);
                    if (pref === p) {
                        return (a.value !== undefined && a.value !== null && a.value !== "") ? a.value : null;
                    }
                }
            }
        }
        var parent = el.parentElement;
        return parent ? _locateNamespaceURI(parent, p) : null;
    }

    if (globalThis.Node && globalThis.Node.prototype) {
        globalThis.Node.prototype.lookupPrefix = function(namespace) {
            if (!namespace) return null;
            var nt = this.nodeType;
            if (nt === 1) return _locateNamespacePrefix(this, namespace);
            if (nt === 9) return this.documentElement ? _locateNamespacePrefix(this.documentElement, namespace) : null;
            if (nt === 2) return this.ownerElement ? _locateNamespacePrefix(this.ownerElement, namespace) : null;
            if (nt === 10 || nt === 11) return null;
            return this.parentElement ? _locateNamespacePrefix(this.parentElement, namespace) : null;
        };

        globalThis.Node.prototype.lookupNamespaceURI = function(prefix) {
            var p = (prefix === "" || prefix === undefined) ? null : prefix;
            var nt = this.nodeType;
            if (nt === 1) {
                if (p === 'xml') return 'http://www.w3.org/XML/1998/namespace';
                if (p === 'xmlns') return 'http://www.w3.org/2000/xmlns/';
                return _locateNamespaceURI(this, p);
            }
            if (nt === 9) return this.documentElement ? this.documentElement.lookupNamespaceURI(p) : null;
            if (nt === 2) return this.ownerElement ? this.ownerElement.lookupNamespaceURI(p) : null;
            if (nt === 10 || nt === 11) return null;
            return this.parentElement ? this.parentElement.lookupNamespaceURI(p) : null;
        };

        globalThis.Node.prototype.isDefaultNamespace = function(namespace) {
            var ns = (namespace === "" || namespace === undefined) ? null : namespace;
            var defaultNS = this.lookupNamespaceURI(null);
            return defaultNS === ns;
        };
    }
