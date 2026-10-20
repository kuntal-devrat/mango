    // ── 25. Image / Audio constructors ────────────────────────────────────────
    globalThis.Image = function(width, height) {
        var img = (typeof document !== 'undefined' && document.createElement) ? document.createElement('img') : { tagName:'IMG', style:{} };
        if (width  !== undefined) img.width  = width;
        if (height !== undefined) img.height = height;
        return img;
    };
    globalThis.HTMLImageElement = globalThis.Image;
    globalThis.Audio = function(src) {
        var audio = (typeof document !== 'undefined' && document.createElement) ? document.createElement('audio') : Object.create(globalThis.HTMLAudioElement.prototype);
        if (src !== undefined) audio.src = src;
        return audio;
    };
    globalThis.Audio.prototype = globalThis.HTMLAudioElement.prototype;

    // ── 26. Observers ─────────────────────────────────────────────────────────
    function MutationRecord(type, target) {
        this.type = type;
        this.target = target;
        this.addedNodes = [];
        this.removedNodes = [];
        this.previousSibling = null;
        this.nextSibling = null;
        this.attributeName = null;
        this.attributeNamespace = null;
        this.oldValue = null;
    }
    MutationRecord.prototype[Symbol.toStringTag] = 'MutationRecord';
    globalThis.MutationRecord = MutationRecord;

    var _activeMutationObservers = [];

    function MutationObserver(cb) {
        if (typeof cb !== 'function') throw new TypeError("MutationObserver callback must be a function");
        this._cb = cb;
        this._observations = [];
        this._records = [];
        this._scheduled = false;
        _activeMutationObservers.push(this);
    }
    MutationObserver.prototype.observe = function(target, options) {
        if (!target) return;
        var opts = options || {};
        this._observations = this._observations.filter(function(o) { return o.target !== target; });
        this._observations.push({ target: target, options: opts });
    };
    MutationObserver.prototype.disconnect = function() {
        this._observations = [];
        this._records = [];
        var self = this;
        _activeMutationObservers = _activeMutationObservers.filter(function(o) { return o !== self; });
    };
    MutationObserver.prototype.takeRecords = function() {
        var recs = this._records.slice();
        this._records = [];
        return recs;
    };
    MutationObserver.prototype._queueRecord = function(record) {
        this._records.push(record);
        if (this._scheduled) return;
        this._scheduled = true;
        var self = this;
        var deliver = function() {
            self._scheduled = false;
            var records = self.takeRecords();
            if (records.length > 0) {
                try {
                    self._cb(records, self);
                } catch(e) {
                    if (typeof console !== 'undefined' && console.error) console.error(e);
                }
            }
        };
        if (typeof queueMicrotask === 'function') {
            queueMicrotask(deliver);
        } else if (typeof Promise !== 'undefined') {
            Promise.resolve().then(deliver);
        } else if (typeof setTimeout !== 'undefined') {
            setTimeout(deliver, 0);
        } else {
            deliver();
        }
    };
    MutationObserver.prototype[Symbol.toStringTag] = 'MutationObserver';
    globalThis.MutationObserver = MutationObserver;

    globalThis._reportDOMMutation = function(detail) {
        if (_activeMutationObservers.length === 0) return;
        for (var i = 0; i < _activeMutationObservers.length; i++) {
            var obs = _activeMutationObservers[i];
            for (var j = 0; j < obs._observations.length; j++) {
                var ob = obs._observations[j];
                var matchesTarget = (ob.target === detail.target);
                if (!matchesTarget && ob.options.subtree && ob.target.contains) {
                    matchesTarget = ob.target.contains(detail.target);
                }
                if (!matchesTarget) continue;

                var opt = ob.options;
                if (detail.type === 'childList' && opt.childList) {
                    var rec = new MutationRecord('childList', detail.target);
                    rec.addedNodes = detail.addedNodes || [];
                    rec.removedNodes = detail.removedNodes || [];
                    rec.previousSibling = detail.previousSibling || null;
                    rec.nextSibling = detail.nextSibling || null;
                    obs._queueRecord(rec);
                } else if (detail.type === 'attributes' && (opt.attributes || opt.attributeFilter || opt.attributeOldValue)) {
                    if (opt.attributeFilter && Array.isArray(opt.attributeFilter) && opt.attributeFilter.indexOf(detail.attributeName) === -1) {
                        continue;
                    }
                    var rec = new MutationRecord('attributes', detail.target);
                    rec.attributeName = detail.attributeName;
                    rec.attributeNamespace = detail.attributeNamespace || null;
                    if (opt.attributeOldValue) rec.oldValue = detail.oldValue !== undefined ? detail.oldValue : null;
                    obs._queueRecord(rec);
                } else if (detail.type === 'characterData' && (opt.characterData || opt.characterDataOldValue)) {
                    var rec = new MutationRecord('characterData', detail.target);
                    if (opt.characterDataOldValue) rec.oldValue = detail.oldValue !== undefined ? detail.oldValue : null;
                    obs._queueRecord(rec);
                }
            }
        }
    };

    function IntersectionObserverEntry(target, rootBounds, boundingClientRect, intersectionRect, isIntersecting, intersectionRatio, time) {
        this.target = target;
        this.rootBounds = rootBounds;
        this.boundingClientRect = boundingClientRect;
        this.intersectionRect = intersectionRect;
        this.isIntersecting = isIntersecting;
        this.intersectionRatio = intersectionRatio;
        this.time = time || (typeof performance !== 'undefined' && performance.now ? performance.now() : Date.now());
        this.isVisible = false;
    }
    IntersectionObserverEntry.prototype[Symbol.toStringTag] = 'IntersectionObserverEntry';
    globalThis.IntersectionObserverEntry = IntersectionObserverEntry;

    var _activeIntersectionObservers = [];

    function IntersectionObserver(cb, options) {
        if (typeof cb !== 'function') throw new TypeError("IntersectionObserver callback must be a function");
        this._cb = cb;
        this._options = options || {};
        this.root = this._options.root || null;
        this.rootMargin = this._options.rootMargin || "0px 0px 0px 0px";
        this.thresholds = Array.isArray(this._options.threshold) ? this._options.threshold : [this._options.threshold !== undefined ? Number(this._options.threshold) : 0];
        this._targets = [];
        this._records = [];
        this._scheduled = false;
        _activeIntersectionObservers.push(this);
    }
    IntersectionObserver.prototype.observe = function(target) {
        if (!target) return;
        if (this._targets.indexOf(target) === -1) {
            this._targets.push(target);
        }
        this._computeAndSchedule();
    };
    IntersectionObserver.prototype.unobserve = function(target) {
        this._targets = this._targets.filter(function(t) { return t !== target; });
    };
    IntersectionObserver.prototype.disconnect = function() {
        this._targets = [];
        this._records = [];
        var self = this;
        _activeIntersectionObservers = _activeIntersectionObservers.filter(function(o) { return o !== self; });
    };
    IntersectionObserver.prototype.takeRecords = function() {
        var recs = this._records.slice();
        this._records = [];
        return recs;
    };
    IntersectionObserver.prototype._computeAndSchedule = function() {
        var self = this;
        var rootRect;
        if (self.root && typeof self.root.getBoundingClientRect === 'function') {
            rootRect = self.root.getBoundingClientRect();
        } else {
            var vw = (typeof window !== 'undefined' && window.innerWidth) || 800;
            var vh = (typeof window !== 'undefined' && window.innerHeight) || 600;
            rootRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(0, 0, vw, vh) : { x:0, y:0, top:0, left:0, width:vw, height:vh, right:vw, bottom:vh };
        }

        var mTop = 0, mRight = 0, mBottom = 0, mLeft = 0;
        if (self.rootMargin) {
            var parts = String(self.rootMargin).trim().split(/\s+/).map(function(p) { return parseFloat(p) || 0; });
            if (parts.length === 1) { mTop = mRight = mBottom = mLeft = parts[0]; }
            else if (parts.length === 2) { mTop = mBottom = parts[0]; mRight = mLeft = parts[1]; }
            else if (parts.length === 4) { mTop = parts[0]; mRight = parts[1]; mBottom = parts[2]; mLeft = parts[3]; }
        }
        var rLeft = rootRect.left - mLeft;
        var rTop = rootRect.top - mTop;
        var rRight = rootRect.right + mRight;
        var rBottom = rootRect.bottom + mBottom;
        var rWidth = rRight - rLeft;
        var rHeight = rBottom - rTop;
        var expandedRootRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(rLeft, rTop, rWidth, rHeight) : { x:rLeft, y:rTop, top:rTop, left:rLeft, width:rWidth, height:rHeight, right:rRight, bottom:rBottom };

        for (var i = 0; i < self._targets.length; i++) {
            var target = self._targets[i];
            var bRect = (typeof target.getBoundingClientRect === 'function') ? target.getBoundingClientRect() : { x:0, y:0, top:0, left:0, width:0, height:0, right:0, bottom:0 };
            var iLeft = Math.max(bRect.left, rLeft);
            var iTop = Math.max(bRect.top, rTop);
            var iRight = Math.min(bRect.right, rRight);
            var iBottom = Math.min(bRect.bottom, rBottom);

            var isIntersecting = (iRight > iLeft && iBottom > iTop) ||
                (bRect.width === 0 && bRect.height === 0 && bRect.left >= rLeft && bRect.right <= rRight && bRect.top >= rTop && bRect.bottom <= rBottom);
            var iWidth = isIntersecting ? Math.max(0, iRight - iLeft) : 0;
            var iHeight = isIntersecting ? Math.max(0, iBottom - iTop) : 0;
            var intersectRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(isIntersecting ? iLeft : 0, isIntersecting ? iTop : 0, iWidth, iHeight) : { x: isIntersecting ? iLeft : 0, y: isIntersecting ? iTop : 0, top: isIntersecting ? iTop : 0, left: isIntersecting ? iLeft : 0, width: iWidth, height: iHeight, right: isIntersecting ? iRight : 0, bottom: isIntersecting ? iBottom : 0 };

            var targetArea = (bRect.width || 0) * (bRect.height || 0);
            var intersectArea = iWidth * iHeight;
            var ratio = 0;
            if (targetArea > 0) {
                ratio = intersectArea / targetArea;
            } else if (isIntersecting) {
                ratio = 1;
            }

            var entry = new IntersectionObserverEntry(target, expandedRootRect, bRect, intersectRect, isIntersecting, ratio);
            self._records.push(entry);
        }

        if (!self._scheduled) {
            self._scheduled = true;
            var deliver = function() {
                self._scheduled = false;
                var records = self.takeRecords();
                if (records.length > 0) {
                    try {
                        self._cb(records, self);
                    } catch(e) {
                        if (typeof console !== 'undefined' && console.error) console.error(e);
                    }
                }
            };
            if (typeof queueMicrotask === 'function') {
                queueMicrotask(deliver);
            } else if (typeof Promise !== 'undefined') {
                Promise.resolve().then(deliver);
            } else if (typeof setTimeout !== 'undefined') {
                setTimeout(deliver, 0);
            } else {
                deliver();
            }
        }
    };
    IntersectionObserver.prototype[Symbol.toStringTag] = 'IntersectionObserver';
    globalThis.IntersectionObserver = IntersectionObserver;

    // ── Lazy Loading with IntersectionObserver (loading="lazy") ────────────────
    var _lazyLoadObserver = null;
    function _getLazyObserver() {
        if (!_lazyLoadObserver && typeof IntersectionObserver === 'function') {
            _lazyLoadObserver = new IntersectionObserver(function(entries) {
                entries.forEach(function(entry) {
                    if (entry.isIntersecting) {
                        var el = entry.target;
                        if (el) {
                            var dataSrc = el.getAttribute ? el.getAttribute('data-src') : null;
                            if (dataSrc) {
                                if (el.setAttribute) el.setAttribute('src', dataSrc);
                                else el.src = dataSrc;
                                if (el.removeAttribute) el.removeAttribute('data-src');
                            }
                            el._lazyLoaded = true;
                            if (_lazyLoadObserver) _lazyLoadObserver.unobserve(el);
                        }
                    }
                });
            }, { rootMargin: '200px 0px' });
        }
        return _lazyLoadObserver;
    }

    function _setupLazyLoadingProperty(proto) {
        if (!proto) return;
        Object.defineProperty(proto, 'loading', {
            get: function() { return this.getAttribute ? (this.getAttribute('loading') || 'eager') : 'eager'; },
            set: function(val) {
                var s = String(val).toLowerCase();
                if (this.setAttribute) this.setAttribute('loading', s);
                if (s === 'lazy') {
                    var obs = _getLazyObserver();
                    if (obs) obs.observe(this);
                }
            },
            configurable: true
        });
    }
    _setupLazyLoadingProperty(globalThis.HTMLElement ? globalThis.HTMLElement.prototype : null);
    _setupLazyLoadingProperty(globalThis.HTMLImageElement ? globalThis.HTMLImageElement.prototype : null);
    _setupLazyLoadingProperty(globalThis.HTMLIFrameElement ? globalThis.HTMLIFrameElement.prototype : null);

    function ResizeObserverSize(inlineSize, blockSize) {
        this.inlineSize = Number(inlineSize) || 0;
        this.blockSize = Number(blockSize) || 0;
        Object.freeze(this);
    }
    ResizeObserverSize.prototype[Symbol.toStringTag] = 'ResizeObserverSize';
    globalThis.ResizeObserverSize = ResizeObserverSize;

    function ResizeObserverEntry(target) {
        this.target = target;
        var r = (typeof target.getBoundingClientRect === 'function') ? target.getBoundingClientRect() : { x:0, y:0, width:0, height:0 };
        var w = r.width || 0;
        var h = r.height || 0;
        this.contentRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(r.x, r.y, w, h) : r;
        this.borderBoxSize = Object.freeze([new ResizeObserverSize(w, h)]);
        this.contentBoxSize = Object.freeze([new ResizeObserverSize(w, h)]);
        var dpr = (typeof window !== 'undefined' && window.devicePixelRatio) || 1;
        this.devicePixelContentBoxSize = Object.freeze([new ResizeObserverSize(Math.round(w * dpr), Math.round(h * dpr))]);
    }
    ResizeObserverEntry.prototype[Symbol.toStringTag] = 'ResizeObserverEntry';
    globalThis.ResizeObserverEntry = ResizeObserverEntry;

    var _activeResizeObservers = [];
    var _resizeObserverLoopCount = 0;
    var _resizeObserverResetScheduled = false;

    function ResizeObserver(cb) {
        if (typeof cb !== 'function') throw new TypeError("ResizeObserver callback must be a function");
        this._cb = cb;
        this._targets = [];
        this._scheduled = false;
        _activeResizeObservers.push(this);
    }
    ResizeObserver.prototype.observe = function(target, options) {
        if (!target) return;
        var idx = this._targets.findIndex(function(t) { return t.target === target; });
        if (idx === -1) {
            this._targets.push({ target: target, options: options || {} });
        }
        this._scheduleDelivery();
    };
    ResizeObserver.prototype.unobserve = function(target) {
        this._targets = this._targets.filter(function(t) { return t.target !== target; });
    };
    ResizeObserver.prototype.disconnect = function() {
        this._targets = [];
        var self = this;
        _activeResizeObservers = _activeResizeObservers.filter(function(o) { return o !== self; });
    };
    ResizeObserver.prototype._scheduleDelivery = function() {
        if (this._scheduled) return;
        this._scheduled = true;
        _resizeObserverLoopCount++;
        if (_resizeObserverLoopCount > 20) {
            this._scheduled = false;
            if (typeof window !== 'undefined' && typeof window.dispatchEvent === 'function') {
                var errEvt;
                if (typeof globalThis.ErrorEvent === 'function') {
                    errEvt = new globalThis.ErrorEvent('error', {
                        message: 'ResizeObserver loop completed with undelivered notifications.'
                    });
                } else if (typeof globalThis.Event === 'function') {
                    errEvt = new globalThis.Event('error');
                    errEvt.message = 'ResizeObserver loop completed with undelivered notifications.';
                }
                if (errEvt) window.dispatchEvent(errEvt);
            }
            return;
        }
        if (!_resizeObserverResetScheduled) {
            _resizeObserverResetScheduled = true;
            var resetLoop = function() {
                _resizeObserverLoopCount = 0;
                _resizeObserverResetScheduled = false;
            };
            if (typeof setTimeout !== 'undefined') setTimeout(resetLoop, 0);
            else if (typeof queueMicrotask === 'function') queueMicrotask(resetLoop);
        }
        var self = this;
        var deliver = function() {
            self._scheduled = false;
            if (self._targets.length === 0) return;
            var entries = self._targets.map(function(t) {
                return new ResizeObserverEntry(t.target);
            });
            try {
                self._cb(entries, self);
            } catch(e) {
                if (typeof console !== 'undefined' && console.error) console.error(e);
            }
        };
        if (typeof queueMicrotask === 'function') {
            queueMicrotask(deliver);
        } else if (typeof Promise !== 'undefined') {
            Promise.resolve().then(deliver);
        } else if (typeof setTimeout !== 'undefined') {
            setTimeout(deliver, 0);
        } else {
            deliver();
        }
    };
    ResizeObserver.prototype[Symbol.toStringTag] = 'ResizeObserver';
    globalThis.ResizeObserver = ResizeObserver;
    var _supportedPerformanceEntryTypes = Object.freeze([
        'element', 'event', 'first-input', 'largest-contentful-paint',
        'layout-shift', 'long-animation-frame', 'longtask', 'mark',
        'measure', 'navigation', 'paint', 'resource', 'visibility-state',
        'soft-navigation'
    ]);
    globalThis.PerformanceObserver = function(cb) {
        this._cb = cb;
        this.observe = function() {};
        this.disconnect = function() {};
        this.takeRecords = function() { return []; };
    };
    globalThis.PerformanceObserver.supportedEntryTypes = _supportedPerformanceEntryTypes;
    globalThis.PerformanceObserver.prototype.observe = function() {};
    globalThis.PerformanceObserver.prototype.disconnect = function() {};
    globalThis.PerformanceObserver.prototype.takeRecords = function() { return []; };

    // ── 27. Event classes ─────────────────────────────────────────────────────
    globalThis.Event = function(type, dict) {
        this.type              = String(type);
        this.bubbles           = !!(dict && dict.bubbles);
        this.cancelable        = (dict && dict.cancelable !== undefined) ? !!dict.cancelable : true;
        this.composed          = !!(dict && dict.composed);
        this.defaultPrevented  = false;
        this.cancelBubble      = false;
        this.timeStamp         = Date.now();
        this.eventPhase        = 0;
        this.isTrusted         = false;
        this.target            = null;
        this.currentTarget     = null;
        this._stopped          = false;
        this._stopImmediate    = false;
        this._currentPassive   = false;
        this.preventDefault    = function() {
            if (this._currentPassive) {
                if (typeof console !== 'undefined' && console.warn) {
                    console.warn("Unable to preventDefault inside passive event listener invocation.");
                }
                return;
            }
            if (this.cancelable) {
                this.defaultPrevented = true;
            }
        };
        this.stopPropagation   = function() {
            this._stopped = true;
            this.cancelBubble = true;
        };
        this.stopImmediatePropagation = function() {
            this._stopped = true;
            this._stopImmediate = true;
            this.cancelBubble = true;
        };
        this.composedPath      = function() { return []; };
    };
    globalThis.Event.NONE = 0;
    globalThis.Event.CAPTURING_PHASE = 1;
    globalThis.Event.AT_TARGET = 2;
    globalThis.Event.BUBBLING_PHASE = 3;
    globalThis.Event.prototype.preventDefault = function() {
        if (this._currentPassive) {
            if (typeof console !== 'undefined' && console.warn) {
                console.warn("Unable to preventDefault inside passive event listener invocation.");
            }
            return;
        }
        if (this.cancelable) {
            this.defaultPrevented = true;
        }
    };
    globalThis.Event.prototype.stopPropagation = function() {
        this._stopped = true;
        this.cancelBubble = true;
    };
    globalThis.Event.prototype.stopImmediatePropagation = function() {
        this._stopped = true;
        this._stopImmediate = true;
        this.cancelBubble = true;
    };
    globalThis.Event.prototype.composedPath = function() { return []; };
    globalThis.Event.prototype.constructor = globalThis.Event;

    globalThis.UIEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        this.view   = (dict && dict.view)   || globalThis.window;
        this.detail = (dict && dict.detail) || 0;
    };
    globalThis.UIEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.UIEvent.prototype.constructor = globalThis.UIEvent;

    globalThis.MouseEvent = function(type, dict) {
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {};
        this.clientX  = dict.clientX  || 0; this.clientY  = dict.clientY  || 0;
        this.screenX  = dict.screenX  || 0; this.screenY  = dict.screenY  || 0;
        this.pageX    = dict.pageX    || 0; this.pageY    = dict.pageY    || 0;
        this.offsetX  = dict.offsetX  || 0; this.offsetY  = dict.offsetY  || 0;
        this.button   = dict.button   || 0; this.buttons  = dict.buttons  || 0;
        this.ctrlKey  = !!dict.ctrlKey;  this.shiftKey = !!dict.shiftKey;
        this.altKey   = !!dict.altKey;   this.metaKey  = !!dict.metaKey;
        this.relatedTarget = dict.relatedTarget || null;
        this.getModifierState = function(key) { return false; };
    };
    globalThis.MouseEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.MouseEvent.prototype.constructor = globalThis.MouseEvent;

    globalThis.PointerEvent = function(type, dict) {
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {};
        this.pointerId   = dict.pointerId   || 0;
        this.pointerType = dict.pointerType || 'mouse';
        this.isPrimary   = dict.isPrimary   !== false;
        this.pressure    = dict.pressure    || 0;
        this.width       = dict.width       || 1;
        this.height      = dict.height      || 1;
    };
    globalThis.PointerEvent.prototype = Object.create(globalThis.MouseEvent.prototype);
    globalThis.PointerEvent.prototype.constructor = globalThis.PointerEvent;

    globalThis.KeyboardEvent = function(type, dict) {
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {};
        this.key      = dict.key      || '';   this.code     = dict.code     || '';
        this.keyCode  = dict.keyCode  || 0;    this.charCode = dict.charCode || 0;
        this.which    = dict.which    || 0;    this.location = dict.location || 0;
        this.repeat   = !!dict.repeat;
        this.ctrlKey  = !!dict.ctrlKey;  this.shiftKey = !!dict.shiftKey;
        this.altKey   = !!dict.altKey;   this.metaKey  = !!dict.metaKey;
        this.getModifierState = function(k) { return false; };
    };
    globalThis.KeyboardEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.KeyboardEvent.prototype.constructor = globalThis.KeyboardEvent;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_STANDARD = 0;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_LEFT     = 1;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_RIGHT    = 2;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_NUMPAD   = 3;

    globalThis.FocusEvent = function(type, dict) {
        globalThis.UIEvent.call(this, type, dict);
        this.relatedTarget = (dict && dict.relatedTarget) || null;
    };
    globalThis.FocusEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.FocusEvent.prototype.constructor = globalThis.FocusEvent;

    globalThis.InputEvent = function(type, dict) {
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {};
        this.data           = dict.data           || null;
        this.inputType      = dict.inputType      || '';
        this.isComposing    = !!dict.isComposing;
        this.dataTransfer   = dict.dataTransfer   || null;
    };
    globalThis.InputEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.InputEvent.prototype.constructor = globalThis.InputEvent;

    globalThis.WheelEvent = function(type, dict) {
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {};
        this.deltaX    = dict.deltaX    || 0;
        this.deltaY    = dict.deltaY    || 0;
        this.deltaZ    = dict.deltaZ    || 0;
        this.deltaMode = dict.deltaMode || 0;
    };
    globalThis.WheelEvent.prototype = Object.create(globalThis.MouseEvent.prototype);
    globalThis.WheelEvent.prototype.constructor = globalThis.WheelEvent;
    globalThis.WheelEvent.DOM_DELTA_PIXEL = 0;
    globalThis.WheelEvent.DOM_DELTA_LINE  = 1;
    globalThis.WheelEvent.DOM_DELTA_PAGE  = 2;

    // Drag and Drop API (HTML5 §7.6 / Section 8.3)
    globalThis.DataTransfer = function() {
        this.dropEffect = 'none';
        this.effectAllowed = 'none';
        this._data = {};
        this.files = [];
        var self = this;
        this.items = {
            get length() { return Object.keys(self._data).length; },
            add: function(data, type) {
                var k = (type || 'text/plain').toLowerCase();
                self._data[k] = String(data);
                return { kind: 'string', type: k };
            },
            remove: function(idx) {
                var keys = Object.keys(self._data);
                if (idx >= 0 && idx < keys.length) {
                    delete self._data[keys[idx]];
                }
            },
            clear: function() {
                self._data = {};
            }
        };
    };
    Object.defineProperty(globalThis.DataTransfer.prototype, 'types', {
        get: function() {
            return Object.freeze(Object.keys(this._data || {}));
        },
        enumerable: true,
        configurable: true
    });
    globalThis.DataTransfer.prototype.setData = function(format, data) {
        if (!this._data) this._data = {};
        this._data[String(format).toLowerCase()] = String(data);
    };
    globalThis.DataTransfer.prototype.getData = function(format) {
        if (!this._data) return '';
        return this._data[String(format).toLowerCase()] || '';
    };
    globalThis.DataTransfer.prototype.clearData = function(format) {
        if (!this._data) return;
        if (format === undefined) {
            this._data = {};
        } else {
            delete this._data[String(format).toLowerCase()];
        }
    };
    globalThis.DataTransfer.prototype.setDragImage = function(img, x, y) {
        this._dragImage = { image: img, x: x || 0, y: y || 0 };
    };

    globalThis.DragEvent = function(type, dict) {
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {};
        this.dataTransfer = (dict && dict.dataTransfer !== undefined) ? dict.dataTransfer : new globalThis.DataTransfer();
    };
    globalThis.DragEvent.prototype = Object.create(globalThis.MouseEvent.prototype);
    globalThis.DragEvent.prototype.constructor = globalThis.DragEvent;

    globalThis.TouchEvent = function(type, dict) {
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {};
        this.touches        = dict.touches        || [];
        this.targetTouches  = dict.targetTouches  || [];
        this.changedTouches = dict.changedTouches || [];
    };
    globalThis.TouchEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.TouchEvent.prototype.constructor = globalThis.TouchEvent;

    globalThis.CustomEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        this.detail = (dict && dict.detail !== undefined) ? dict.detail : null;
        this.initCustomEvent = function(t, b, c, d) { this.type=t; this.bubbles=b; this.cancelable=c; this.detail=d; };
    };
    globalThis.CustomEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.CustomEvent.prototype.constructor = globalThis.CustomEvent;

    globalThis.ToggleEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.oldState = (dict && dict.oldState !== undefined) ? String(dict.oldState) : 'closed';
        this.newState = (dict && dict.newState !== undefined) ? String(dict.newState) : 'closed';
    };
    globalThis.ToggleEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.ToggleEvent.prototype.constructor = globalThis.ToggleEvent;

    globalThis.HashChangeEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        this.oldURL = (dict && dict.oldURL) || '';
        this.newURL = (dict && dict.newURL) || '';
    };
    globalThis.HashChangeEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.HashChangeEvent.prototype.constructor = globalThis.HashChangeEvent;

    globalThis.PopStateEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        this.state = (dict && dict.state) || null;
    };
    globalThis.PopStateEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.PopStateEvent.prototype.constructor = globalThis.PopStateEvent;

    globalThis.ErrorEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.message  = dict.message  || '';
        this.filename = dict.filename || '';
        this.lineno   = dict.lineno   || 0;
        this.colno    = dict.colno    || 0;
        this.error    = dict.error    || null;
    };
    globalThis.ErrorEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.ErrorEvent.prototype.constructor = globalThis.ErrorEvent;

    globalThis.StorageEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.key         = dict.key         || null;
        this.oldValue    = dict.oldValue    || null;
        this.newValue    = dict.newValue    || null;
        this.url         = dict.url         || '';
        this.storageArea = dict.storageArea || null;
    };
    globalThis.StorageEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.StorageEvent.prototype.constructor = globalThis.StorageEvent;

    // TransitionEvent / AnimationEvent (for CSS animation callbacks)
    globalThis.TransitionEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.propertyName = dict.propertyName || '';
        this.elapsedTime  = dict.elapsedTime  || 0;
        this.pseudoElement= dict.pseudoElement|| '';
    };
    globalThis.TransitionEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.TransitionEvent.prototype.constructor = globalThis.TransitionEvent;

    globalThis.AnimationEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.animationName = dict.animationName || '';
        this.elapsedTime   = dict.elapsedTime   || 0;
        this.pseudoElement = dict.pseudoElement || '';
    };
    globalThis.AnimationEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.AnimationEvent.prototype.constructor = globalThis.AnimationEvent;

    globalThis.SubmitEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.submitter = dict.submitter || null;
    };
    globalThis.SubmitEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.SubmitEvent.prototype.constructor = globalThis.SubmitEvent;

    globalThis.FormDataEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.formData = dict.formData || null;
    };
    globalThis.FormDataEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.FormDataEvent.prototype.constructor = globalThis.FormDataEvent;

    globalThis.PageTransitionEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.persisted = !!dict.persisted;
    };
    globalThis.PageTransitionEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.PageTransitionEvent.prototype.constructor = globalThis.PageTransitionEvent;

    globalThis.MessageEvent = function(type, dict) {
        globalThis.Event.call(this, type, dict);
        dict = dict || {};
        this.data = dict.data !== undefined ? dict.data : null;
        this.origin = dict.origin || '';
        this.lastEventId = dict.lastEventId || '';
        this.source = dict.source || null;
        this.ports = dict.ports || [];
    };
    globalThis.MessageEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.MessageEvent.prototype.constructor = globalThis.MessageEvent;

