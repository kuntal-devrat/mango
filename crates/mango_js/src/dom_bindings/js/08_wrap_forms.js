            get defaultSelected() {
                var def = this.getAttribute("data-mango-default-selected");
                if (def !== null) return def === "true";
                return this.hasAttribute('selected');
            },
            set defaultSelected(val) {
                this.setAttribute("data-mango-default-selected", val ? "true" : "false");
                if (val) this.setAttribute('selected', '');
                else this.removeAttribute('selected');
            },
            get index() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'option') {
                    var p = this.parentElement;
                    while (p && (p.tagName || '').toLowerCase() !== 'select') {
                        p = p.parentElement;
                    }
                    if (p) {
                        var opts = p.options || [];
                        for (var i = 0; i < opts.length; i++) {
                            if (opts[i] === this) return i;
                        }
                    }
                }
                return 0;
            },
            get label() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'optgroup' || tag === 'option' || tag === 'track') {
                    var l = this.getAttribute('label');
                    return l === null ? '' : l;
                }
                return undefined;
            },
            set label(val) {
                this.setAttribute('label', String(val));
            },
            get files() {
                if (this._files) return this._files;
                var multiple = this.hasAttribute('multiple');
                this._files = { length: 0, item: function() { return null; }, 0: null };
                if (!multiple) { this._files.item = function() { return null; }; }
                return this._files;
            },
            get form() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'legend') {
                    var p = this.parentElement;
                    while (p) {
                        var pTag = (p.tagName || '').toLowerCase();
                        if (pTag === 'fieldset') return p.form;
                        if (pTag === 'form') return p;
                        p = p.parentElement;
                    }
                    return null;
                }
                var formAttr = this.getAttribute('form');
                if (formAttr && typeof document !== 'undefined' && document.getElementById) {
                    var f = document.getElementById(formAttr);
                    if (f && (f.tagName || '').toLowerCase() === 'form') return f;
                }
                var p = this._nodeId;
                while (p !== null && p !== undefined) {
                    var parentId = _getParent(p);
                    if (parentId === null || parentId === undefined) return null;
                    var el = _wrapElement(parentId);
                    if (el && el.tagName && el.tagName.toLowerCase() === 'form') return el;
                    p = parentId;
                }
                return null;
            },
            get elements() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form' || tag === 'fieldset') {
                    var list = this.querySelectorAll('input, button, select, textarea, output, fieldset');
                    var res = [];
                    for (var i = 0; i < list.length; i++) {
                        res[i] = list[i];
                        var name = list[i].getAttribute('name');
                        if (name && !(name in res)) res[name] = list[i];
                        var id = list[i].getAttribute('id');
                        if (id && !(id in res)) res[id] = list[i];
                    }
                    res.item = function(index) { return res[index] || null; };
                    res.namedItem = function(name) { return res[name] || null; };
                    return res;
                }
                return undefined;
            },
            get length() {
                if (this.nodeType === 3 || this.nodeType === 8 || this.nodeType === 7 || this.nodeType === 4) {
                    return (this.data || '').length;
                }
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    return (this.elements || []).length;
                }
                if (tag === 'select') {
                    return (this.options || []).length;
                }
                var l = (typeof this.getAttribute === 'function') ? this.getAttribute('length') : null;
                return l === null ? 0 : parseInt(l, 10) || 0;
            },
            stepUp: function(n) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag !== 'input') return;
                var t = this.type;
                if (t !== 'number' && t !== 'range' && t !== 'date' && t !== 'time') return;
                var stepAttr = this.getAttribute('step');
                if (stepAttr && stepAttr.toLowerCase() === 'any') return;
                var step = parseFloat(stepAttr);
                if (isNaN(step) || step <= 0) step = 1.0;
                var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
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
            },
            stepDown: function(n) {
                var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
                this.stepUp(-count);
            },
            submit: function() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    this.dispatchEvent({ type: 'submit', target: this });
                }
            },
            requestSubmit: function(submitter) {
                var tag = (this.tagName || '').toLowerCase();
                if (tag !== 'form') return;
                if (submitter !== undefined && submitter !== null) {
                    var subTag = (submitter.tagName || '').toLowerCase();
                    var subType = (submitter.type || submitter.getAttribute('type') || '').toLowerCase();
                    var isSubmitter = (subTag === 'button' && (subType === 'submit' || !subType)) ||
                                      (subTag === 'input' && (subType === 'submit' || subType === 'image'));
                    if (!isSubmitter) {
                        throw new TypeError("The specified element is not a submit button");
                    }
                    if (submitter.form !== this && (!submitter.form || submitter.form._nodeId !== this._nodeId)) {
                        throw new TypeError("The specified element is not owned by this form element");
                    }
                }
                var shouldValidate = !this.noValidate;
                if (submitter && submitter.formNoValidate) {
                    shouldValidate = false;
                }
                if (shouldValidate) {
                    var valid = this.reportValidity();
                    if (!valid) return;
                }
                var evt;
                if (typeof globalThis.SubmitEvent === 'function') {
                    evt = new globalThis.SubmitEvent('submit', { bubbles: true, cancelable: true, submitter: submitter || null });
                } else if (typeof globalThis.Event === 'function') {
                    evt = new globalThis.Event('submit', { bubbles: true, cancelable: true });
                    evt.submitter = submitter || null;
                } else {
                    evt = { type: 'submit', bubbles: true, cancelable: true, submitter: submitter || null };
                }
                this.dispatchEvent(evt);
            },
            reset: function() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
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
                    this.dispatchEvent({ type: 'reset', target: this });
                }
            },
            get validity() { return this._computeValidity(); },
            get validationMessage() {
                var v = this._computeValidity();
                if (v.valid) return "";
                var custom = this.getAttribute('data-mango-custom-validity');
                if (custom) return custom;
                if (v.valueMissing) return "Please fill out this field.";
                if (v.typeMismatch) return "Please match the requested type.";
                if (v.patternMismatch) return "Please match the requested format.";
                var minLen = this.getAttribute('minlength');
                if (v.tooShort && minLen !== null) return "Please lengthen this text to " + minLen + " characters or more.";
                var maxLen = this.getAttribute('maxlength');
                if (v.tooLong && maxLen !== null) return "Please shorten this text to " + maxLen + " characters or less.";
                if (v.rangeUnderflow) return "Value must be greater than or equal to " + this.min + ".";
                if (v.rangeOverflow) return "Value must be less than or equal to " + this.max + ".";
                if (v.stepMismatch) return "Please select a valid value.";
                if (v.badInput) return "Please enter a number.";
                return "Invalid value.";
            },
            get willValidate() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag !== 'input' && tag !== 'select' && tag !== 'textarea' && tag !== 'button') return false;
                var t = this.type;
                return !(t === 'hidden' || t === 'button' || t === 'reset' || this.disabled || this.readOnly);
            },
            checkValidity: function() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    var elems = this.elements || [];
                    var allValid = true;
                    for (var i = 0; i < elems.length; i++) {
                        var el = elems[i];
                        if (el.willValidate && typeof el.checkValidity === 'function') {
                            if (!el.checkValidity()) {
                                allValid = false;
                            }
                        }
                    }
                    return allValid;
                }
                var v = this._computeValidity();
                if (!v.valid) {
                    var invalidEvt = (typeof globalThis.Event === 'function') ?
                        new globalThis.Event('invalid', { bubbles: false, cancelable: true }) :
                        { type: 'invalid', target: this, bubbles: false, cancelable: true };
                    this.dispatchEvent(invalidEvt);
                }
                return v.valid;
            },
            reportValidity: function() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    var elems = this.elements || [];
                    var allValid = true;
                    var firstInvalid = null;
                    for (var i = 0; i < elems.length; i++) {
                        var el = elems[i];
                        if (el.willValidate && typeof el.checkValidity === 'function') {
                            if (!el.checkValidity()) {
                                if (!firstInvalid) firstInvalid = el;
                                allValid = false;
                            }
                        }
                    }
                    if (!allValid && firstInvalid) {
                        if (typeof firstInvalid.focus === 'function') {
                            try { firstInvalid.focus(); } catch(e) {}
                        }
                        if (typeof firstInvalid.reportValidity === 'function') {
                            firstInvalid.reportValidity();
                        }
                    }
                    return allValid;
                }
                var v = this._computeValidity();
                if (!v.valid) {
                    var cancelableEvt = (typeof globalThis.Event === 'function') ?
                        new globalThis.Event('invalid', { bubbles: false, cancelable: true }) :
                        { type: 'invalid', target: this, bubbles: false, cancelable: true };
                    this.dispatchEvent(cancelableEvt);
                    if (typeof this.focus === 'function') {
                        try { this.focus(); } catch(e) {}
                    }
                    return false;
                }
                return true;
            },
            setCustomValidity: function(message) {
                if (message) this.setAttribute('data-mango-custom-validity', String(message));
                else this.removeAttribute('data-mango-custom-validity');
            },
            _computeValidity: function() {
                var custom = this.getAttribute('data-mango-custom-validity');
                var result = {
                    valueMissing: false,
                    typeMismatch: false,
                    patternMismatch: false,
                    tooLong: false,
                    tooShort: false,
                    rangeUnderflow: false,
                    rangeOverflow: false,
                    stepMismatch: false,
                    badInput: false,
                    customError: !!custom,
                    valid: true
                };
                if (custom) {
                    result.valid = false;
                    return (typeof globalThis.ValidityState === 'function') ? new globalThis.ValidityState(result) : result;
                }
                if (!this.willValidate) {
                    return (typeof globalThis.ValidityState === 'function') ? new globalThis.ValidityState(result) : result;
                }
                var val = this.value;
                var t = this.type;
                var tag = (this.tagName || '').toLowerCase();

                if (this.required) {
                    if (t === 'checkbox' || t === 'radio') {
                        if (!this.checked) { result.valueMissing = true; result.valid = false; }
                    } else if (tag === 'select') {
                        if (!val || !String(val).trim()) {
                            result.valueMissing = true; result.valid = false;
                        }
                    } else if (!val || !String(val).trim()) {
                        result.valueMissing = true; result.valid = false;
                    }
                }

                if (val && String(val).length > 0) {
                    var strVal = String(val);
                    if (t === 'email') {
                        var parts = this.hasAttribute('multiple') ? strVal.split(',') : [strVal];
                        for (var i = 0; i < parts.length; i++) {
                            var s = parts[i].trim();
                            var at = s.indexOf('@');
                            var domain = at === -1 ? '' : s.substring(at + 1);
                            if (at < 1 || domain.indexOf('.') < 1 || /\s/.test(domain)) {
                                result.typeMismatch = true; result.valid = false; break;
                            }
                        }
                    } else if (t === 'url') {
                        if (!/^([a-z][a-z0-9+.-]*:|\/\/)/i.test(strVal)) {
                            result.typeMismatch = true; result.valid = false;
                        }
                    } else if (t === 'number') {
                        var num = parseFloat(strVal);
                        if (isNaN(num)) {
                            result.badInput = true; result.valid = false;
                        }
                    }

                    // pattern check
                    var pattern = this.getAttribute('pattern');
                    if (pattern && (t === 'text' || t === 'search' || t === 'url' || t === 'tel' || t === 'email' || t === 'password')) {
                        try {
                            var regex = new RegExp("^(?:" + pattern + ")$");
                            if (!regex.test(strVal)) {
                                result.patternMismatch = true; result.valid = false;
                            }
                        } catch (e) {}
                    }

                    // range check (number, range, date, time)
                    if (t === 'number' || t === 'range') {
                        var num = parseFloat(strVal);
                        if (!isNaN(num)) {
                            var min = parseFloat(this.min);
                            if (!isNaN(min) && num < min) {
                                result.rangeUnderflow = true; result.valid = false;
                            }
                            var max = parseFloat(this.max);
                            if (!isNaN(max) && num > max) {
                                result.rangeOverflow = true; result.valid = false;
                            }
                            var stepAttr = this.getAttribute('step');
                            if (stepAttr && stepAttr.toLowerCase() !== 'any') {
                                var step = parseFloat(stepAttr);
                                if (!isNaN(step) && step > 0) {
                                    var base = !isNaN(min) ? min : 0;
                                    var diff = Math.abs(num - base);
                                    var rem = diff % step;
                                    if (rem > 0.0001 && Math.abs(rem - step) > 0.0001) {
                                        result.stepMismatch = true; result.valid = false;
                                    }
                                }
                            }
                        }
                    } else if (t === 'date' || t === 'time') {
                        var min = this.getAttribute('min');
                        if (min && strVal < min) {
                            result.rangeUnderflow = true; result.valid = false;
                        }
                        var max = this.getAttribute('max');
                        if (max && strVal > max) {
                            result.rangeOverflow = true; result.valid = false;
                        }
                    }
                }

                var minLen = this.getAttribute('minlength');
                if (minLen !== null && String(val).length < parseInt(minLen, 10)) { result.tooShort = true; result.valid = false; }
                var maxLen = this.getAttribute('maxlength');
                if (maxLen !== null && String(val).length > parseInt(maxLen, 10)) { result.tooLong = true; result.valid = false; }

                if (result.valueMissing || result.typeMismatch || result.patternMismatch ||
                    result.tooLong || result.tooShort || result.rangeUnderflow ||
                    result.rangeOverflow || result.stepMismatch || result.badInput || result.customError) {
                    result.valid = false;
                }
                return (typeof globalThis.ValidityState === 'function') ? new globalThis.ValidityState(result) : result;
            },
            get selectionStart() { var s = this.getAttribute('data-mango-selection-start'); return s === null ? 0 : parseInt(s, 10); },
            set selectionStart(val) { this.setAttribute('data-mango-selection-start', String(val)); },
            get selectionEnd() { var s = this.getAttribute('data-mango-selection-end'); return s === null ? 0 : parseInt(s, 10); },
            set selectionEnd(val) { this.setAttribute('data-mango-selection-end', String(val)); },
            get selectionDirection() { return ['forward', 'backward', 'none'].indexOf(this.getAttribute('data-mango-selection-direction')) >= 0 ? this.getAttribute('data-mango-selection-direction') : 'none'; },
            select: function() {
                var len = String(this.value === undefined ? '' : this.value).length;
                this.setSelectionRange(0, len);
            },
            setSelectionRange: function(start, end, direction) {
                start = start === undefined ? 0 : parseInt(start, 10) || 0;
                end = end === undefined ? start : parseInt(end, 10) || 0;
                this.setAttribute('data-mango-selection-start', String(start));
                this.setAttribute('data-mango-selection-end', String(end));
                this.setAttribute('data-mango-selection-direction', direction || 'none');
                this.dispatchEvent({ type: 'select', target: this });
            },
            get src() { return this.getAttribute("src") || ""; },
            set src(val) { this.setAttribute("src", val); },
            get currentSrc() { return this.src; },
            get poster() { return this.getAttribute("poster") || ""; },
            set poster(val) { this.setAttribute("poster", val); },
            get controls() { return this.hasAttribute("controls"); },
            set controls(val) { if (val) this.setAttribute("controls", ""); else this.removeAttribute("controls"); },
            get autoplay() { return this.hasAttribute("autoplay"); },
            set autoplay(val) { if (val) this.setAttribute("autoplay", ""); else this.removeAttribute("autoplay"); },
            get loop() { return this.hasAttribute("loop"); },
            set loop(val) { if (val) this.setAttribute("loop", ""); else this.removeAttribute("loop"); },
            get muted() { return this.hasAttribute("muted") || this.getAttribute("data-mango-muted") === "true"; },
            set muted(val) {
                if (val) { this.setAttribute("muted", ""); this.setAttribute("data-mango-muted", "true"); }
                else { this.removeAttribute("muted"); this.setAttribute("data-mango-muted", "false"); }
                this.dispatchEvent({ type: 'volumechange' });
            },
            get paused() {
                var p = this.getAttribute("data-mango-playing");
                if (p === "true") return false;
                if (p === "false") return true;
                return !this.autoplay;
            },
            get currentTime() {
                var t = parseFloat(this.getAttribute("data-mango-time") || "0");
                return isNaN(t) ? 0 : t;
            },
            set currentTime(val) {
                var num = Math.max(0, parseFloat(val) || 0);
                this.setAttribute("data-mango-time", String(num));
                this.dispatchEvent({ type: 'timeupdate' });
            },
            get duration() {
                var d = parseFloat(this.getAttribute("data-mango-duration") || "100");
                return isNaN(d) ? 100 : d;
            },
            get volume() {
                return this._volume !== undefined ? this._volume : 1.0;
            },
            set volume(val) {
                this._volume = Math.max(0, Math.min(1, parseFloat(val) || 0));
                this.dispatchEvent({ type: 'volumechange' });
            },
            get playbackRate() {
                return this._playbackRate !== undefined ? this._playbackRate : 1.0;
            },
            set playbackRate(val) {
                this._playbackRate = parseFloat(val) || 1.0;
                this.dispatchEvent({ type: 'ratechange' });
            },
            get readyState() { return 4; },
            get networkState() { return 1; },
            get ended() { return false; },
            get videoWidth() {
                var w = parseInt(this.getAttribute("width"), 10);
                return !isNaN(w) && w > 0 ? w : 640;
            },
            get videoHeight() {
                var h = parseInt(this.getAttribute("height"), 10);
                return !isNaN(h) && h > 0 ? h : 360;
            },
            play: function() {
                this.setAttribute("data-mango-playing", "true");
                var self = this;
                setTimeout(function() {
                    self.dispatchEvent({ type: 'play' });
                    self.dispatchEvent({ type: 'playing' });
                }, 0);
                return (typeof Promise !== 'undefined') ? Promise.resolve() : null;
            },
            pause: function() {
                this.setAttribute("data-mango-playing", "false");
                var self = this;
                setTimeout(function() {
                    self.dispatchEvent({ type: 'pause' });
                }, 0);
            },
            load: function() {
                var self = this;
                setTimeout(function() {
                    self.dispatchEvent({ type: 'loadstart' });
                    self.dispatchEvent({ type: 'loadedmetadata' });
                    self.dispatchEvent({ type: 'canplay' });
                }, 0);
            },
            canPlayType: function(type) {
                if (!type) return "";
                var t = String(type).toLowerCase();
                if (t.indexOf("mp4") !== -1 || t.indexOf("webm") !== -1 || t.indexOf("ogg") !== -1 ||
                    t.indexOf("audio/mpeg") !== -1 || t.indexOf("audio/mp3") !== -1 ||
                    t.indexOf("audio/wav") !== -1 || t.indexOf("audio/ogg") !== -1 || t.indexOf("audio/aac") !== -1) {
                    return "probably";
                }
                return "maybe";
            },
            get srcdoc() { return this.getAttribute("srcdoc") || ""; },
            set srcdoc(val) { this.setAttribute("srcdoc", val); },
            get width() {
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    var w = parseInt(this.getAttribute("width"), 10);
                    return (!isNaN(w) && w >= 0) ? w : 300;
                }
                return this.getAttribute("width") || "";
            },
            set width(val) {
                var num = parseInt(val, 10);
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    if (isNaN(num) || num < 0) num = 300;
                    this.setAttribute("width", String(num));
                    var h = parseInt(this.getAttribute("height"), 10);
                    if (isNaN(h) || h < 0) h = 150;
                    _canvasResize(this._nodeId, num, h);
                } else {
                    this.setAttribute("width", val);
                }
            },
            get height() {
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    var h = parseInt(this.getAttribute("height"), 10);
                    return (!isNaN(h) && h >= 0) ? h : 150;
                }
                return this.getAttribute("height") || "";
            },
            set height(val) {
                var num = parseInt(val, 10);
                if (this.tagName && this.tagName.toLowerCase() === 'canvas') {
                    if (isNaN(num) || num < 0) num = 150;
                    this.setAttribute("height", String(num));
                    var w = parseInt(this.getAttribute("width"), 10);
                    if (isNaN(w) || w < 0) w = 300;
                    _canvasResize(this._nodeId, w, num);
                } else {
                    this.setAttribute("height", val);
                }
            },
            get sandbox() {
                var self = this;
                return {
                    get value() { return self.getAttribute("sandbox") || ""; },
                    set value(v) { self.setAttribute("sandbox", v); },
                    contains: function(token) {
                        var tokens = (self.getAttribute("sandbox") || "").split(/\s+/);
                        return tokens.indexOf(token) !== -1;
                    },
                    add: function(token) {
                        var tokens = (self.getAttribute("sandbox") || "").split(/\s+/).filter(Boolean);
                        if (tokens.indexOf(token) === -1) {
                            tokens.push(token);
                            self.setAttribute("sandbox", tokens.join(" "));
                        }
                    },
                    remove: function(token) {
                        var tokens = (self.getAttribute("sandbox") || "").split(/\s+/).filter(function(t) { return t !== token && t.length > 0; });
                        self.setAttribute("sandbox", tokens.join(" "));
                    },
                    toString: function() { return self.getAttribute("sandbox") || ""; }
                };
            },
            set sandbox(val) { this.setAttribute("sandbox", val); },
            // contenteditable & draggable surface (Section 8.3)
            get contentEditable() {
                var v = this.getAttribute('contenteditable');
                if (v === null) return 'inherit';
                var lower = v.toLowerCase();
                if (lower === '' || lower === 'true') return 'true';
                if (lower === 'false') return 'false';
                return 'inherit';
            },
            set contentEditable(val) {
                var s = String(val).toLowerCase();
                if (s === 'true') {
                    this.setAttribute('contenteditable', 'true');
                } else if (s === 'false') {
                    this.setAttribute('contenteditable', 'false');
                } else if (s === 'inherit') {
                    this.setAttribute('contenteditable', 'inherit');
                } else {
                    throw new Error("SyntaxError: The value provided is not a valid contentEditable state.");
                }
            },
