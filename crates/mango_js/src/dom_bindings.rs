//! DOM API bindings that bridge JavaScript to Mango's arena-based DOM.

use std::cell::RefCell;
use std::rc::Rc;

use mango_html::dom::{Document, NodeData, NodeId};

/// Shared document reference accessible from JS callbacks.
pub type SharedDocument = Rc<RefCell<Document>>;

/// Flag indicating the DOM was mutated and needs re-layout.
pub type DomDirtyFlag = Rc<RefCell<bool>>;

/// Published layout geometry: DOM node id -> `[x, y, width, height]` in viewport pixels.
///
/// The browser refreshes this after every layout so that JS measurement APIs
/// (`getBoundingClientRect`, `offsetWidth`, `elementFromPoint`) return real values
/// without `mango_js` depending on `mango_layout`.
pub type LayoutBounds = Rc<RefCell<std::collections::HashMap<u32, [f32; 4]>>>;

/// Shared cookie jar backing `document.cookie`.
///
/// The browser owns the jar (it loads and saves it from the profile) and shares
/// that exact handle with the HTTP loader and this runtime, so a script write is
/// visible to the very next request with no copy step.
pub type CookieStore = std::sync::Arc<std::sync::Mutex<mango_net::CookieJar>>;

pub(crate) fn jar_lock(store: &CookieStore) -> std::sync::MutexGuard<'_, mango_net::CookieJar> {
    store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub const DOM_SHIM: &str = r#"
    function _normalizeEventOptions(options) {
        var capture = false;
        var once = false;
        var passive = false;
        var signal = null;
        if (typeof options === 'boolean') {
            capture = options;
        } else if (options && typeof options === 'object') {
            capture = !!options.capture;
            once = !!options.once;
            passive = !!options.passive;
            signal = options.signal || null;
        }
        return { capture: capture, once: once, passive: passive, signal: signal };
    }

    function _addDOMEventListener(target, type, listener, options) {
        if (!listener || !target) return;
        type = String(type);
        if (!target._listeners) target._listeners = {};
        if (!target._listeners[type]) target._listeners[type] = [];

        var opts = _normalizeEventOptions(options);
        if (opts.signal && opts.signal.aborted) return;

        for (var i = 0; i < target._listeners[type].length; i++) {
            var item = target._listeners[type][i];
            if (item.fn === listener && item.capture === opts.capture) {
                return;
            }
        }

        var entry = {
            fn: listener,
            capture: opts.capture,
            once: opts.once,
            passive: opts.passive,
            signal: opts.signal
        };
        target._listeners[type].push(entry);

        if (opts.signal && typeof opts.signal.addEventListener === 'function') {
            var onAbort = function() {
                _removeDOMEventListener(target, type, listener, { capture: opts.capture });
                if (typeof opts.signal.removeEventListener === 'function') {
                    opts.signal.removeEventListener('abort', onAbort);
                }
            };
            opts.signal.addEventListener('abort', onAbort);
        }
    }

    function _removeDOMEventListener(target, type, listener, options) {
        if (!target || !target._listeners) return;
        type = String(type);
        if (!target._listeners[type]) return;
        var capture = false;
        if (typeof options === 'boolean') {
            capture = options;
        } else if (options && typeof options === 'object') {
            capture = !!options.capture;
        }
        target._listeners[type] = target._listeners[type].filter(function(item) {
            return !(item.fn === listener && item.capture === capture);
        });
    }

    if (typeof globalThis.ToggleEvent === 'undefined') {
        function ToggleEvent(type, dict) {
            dict = dict || {};
            var evt = (typeof globalThis.Event === 'function') ? new globalThis.Event(type, dict) : { type: type };
            evt.oldState = (dict && dict.oldState !== undefined) ? String(dict.oldState) : 'closed';
            evt.newState = (dict && dict.newState !== undefined) ? String(dict.newState) : 'closed';
            return evt;
        }
        if (typeof globalThis.Event === 'function') {
            ToggleEvent.prototype = Object.create(globalThis.Event.prototype);
            ToggleEvent.prototype.constructor = ToggleEvent;
        }
        globalThis.ToggleEvent = ToggleEvent;
    }

    if (typeof globalThis.ValidityState === 'undefined') {
        function ValidityState(flags) {
            if (flags) {
                this.badInput = !!flags.badInput;
                this.customError = !!flags.customError;
                this.patternMismatch = !!flags.patternMismatch;
                this.rangeOverflow = !!flags.rangeOverflow;
                this.rangeUnderflow = !!flags.rangeUnderflow;
                this.stepMismatch = !!flags.stepMismatch;
                this.tooLong = !!flags.tooLong;
                this.tooShort = !!flags.tooShort;
                this.typeMismatch = !!flags.typeMismatch;
                this.valueMissing = !!flags.valueMissing;
                this.valid = !this.badInput && !this.customError && !this.patternMismatch &&
                             !this.rangeOverflow && !this.rangeUnderflow && !this.stepMismatch &&
                             !this.tooLong && !this.tooShort && !this.typeMismatch && !this.valueMissing;
            } else {
                this.badInput = false;
                this.customError = false;
                this.patternMismatch = false;
                this.rangeOverflow = false;
                this.rangeUnderflow = false;
                this.stepMismatch = false;
                this.tooLong = false;
                this.tooShort = false;
                this.typeMismatch = false;
                this.valueMissing = false;
                this.valid = true;
            }
        }
        ValidityState.prototype = {
            constructor: ValidityState,
            badInput: false,
            customError: false,
            patternMismatch: false,
            rangeOverflow: false,
            rangeUnderflow: false,
            stepMismatch: false,
            tooLong: false,
            tooShort: false,
            typeMismatch: false,
            valueMissing: false,
            valid: true
        };
        Object.defineProperty(ValidityState.prototype, Symbol.toStringTag, {
            value: 'ValidityState',
            configurable: true
        });
        globalThis.ValidityState = ValidityState;
    }

    function _dispatchDOMEvent(target, event) {
        if (!event || typeof event !== 'object') {
            if (typeof globalThis.Event === 'function') {
                event = new globalThis.Event(String(event));
            } else {
                event = { type: String(event), bubbles: false, cancelable: false };
            }
        }
        var type = String(event.type);
        event.target = target;
        event.isTrusted = !!event.isTrusted;
        event.defaultPrevented = !!event.defaultPrevented;
        event._stopped = false;
        event._stopImmediate = false;

        // HTML5 Popover auto light-dismiss
        if ((type === 'click' || type === 'pointerdown') && globalThis._popoverStack && globalThis._popoverStack.length > 0) {
            var toClose = [];
            for (var pi = globalThis._popoverStack.length - 1; pi >= 0; pi--) {
                var p = globalThis._popoverStack[pi];
                if (p && p.popover === 'auto') {
                    var isInside = (p === target) || (typeof p.contains === 'function' && p.contains(target));
                    var isInvoker = (target && target.popoverTargetElement === p);
                    if (!isInside && !isInvoker) {
                        toClose.push(p);
                    }
                }
            }
            for (var ci = 0; ci < toClose.length; ci++) {
                try { toClose[ci].hidePopover(); } catch(e) {}
            }
        } else if (type === 'keydown' && (event.key === 'Escape' || event.keyCode === 27) && globalThis._popoverStack && globalThis._popoverStack.length > 0) {
            for (var pi = globalThis._popoverStack.length - 1; pi >= 0; pi--) {
                var p = globalThis._popoverStack[pi];
                if (p && p.popover === 'auto') {
                    try { p.hidePopover(); } catch(e) {}
                    if (typeof event.preventDefault === 'function') event.preventDefault();
                    break;
                }
            }
        }

        var path = [];
        var cur = target;
        var guard = 0;
        while (cur && guard++ < 256) {
            path.push(cur);
            if (typeof globalThis !== 'undefined' && cur === globalThis.window) {
                break;
            } else if (cur === document) {
                if (typeof globalThis !== 'undefined' && globalThis.window && globalThis.window !== document) {
                    path.push(globalThis.window);
                }
                break;
            } else {
                var p = cur.parentNode || cur.parentElement;
                if (!p && cur._nodeId !== undefined && cur._nodeId !== null) {
                    var pid = (typeof _getParentNode === 'function') ? _getParentNode(cur._nodeId) : -1;
                    if (pid === 0) p = document;
                    else if (pid > 0) p = _wrapElement(pid);
                }
                if (p) {
                    cur = p;
                } else if (cur.host) {
                    if (event.composed !== false) {
                        cur = cur.host;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        var cleanPath = [];
        for (var p = 0; p < path.length; p++) {
            if (cleanPath.indexOf(path[p]) === -1) {
                cleanPath.push(path[p]);
            }
        }
        path = cleanPath;

        event.composedPath = function() {
            return path.slice();
        };

        var onProp = 'on' + type;

        function invokeListenersOnNode(node, phaseIsCapture) {
            if (!node) return;
            event.currentTarget = node;

            if (node._listeners && node._listeners[type]) {
                var listeners = node._listeners[type].slice();
                for (var i = 0; i < listeners.length; i++) {
                    if (event._stopImmediate) break;
                    var entry = listeners[i];
                    if (event.eventPhase === 2 /* AT_TARGET */ || entry.capture === phaseIsCapture) {
                        if (entry.once) {
                            _removeDOMEventListener(node, type, entry.fn, { capture: entry.capture });
                        }
                        event._currentPassive = !!entry.passive;
                        try {
                            if (typeof entry.fn === 'function') {
                                entry.fn.call(node, event);
                            } else if (entry.fn && typeof entry.fn.handleEvent === 'function') {
                                entry.fn.handleEvent(event);
                            }
                        } catch (e) {
                            if (typeof console !== 'undefined' && console.error) console.error(e);
                        } finally {
                            event._currentPassive = false;
                        }
                    }
                }
            }

            if (!phaseIsCapture && !event._stopImmediate) {
                if (typeof node[onProp] === 'function') {
                    try {
                        node[onProp].call(node, event);
                    } catch (e) {
                        if (typeof console !== 'undefined' && console.error) console.error(e);
                    }
                }
            }
        }

        // Phase 1: Capturing Phase (window -> document -> ancestors down to target's parent)
        event.eventPhase = 1;
        for (var j = path.length - 1; j > 0; j--) {
            if (event._stopped || event._stopImmediate) break;
            invokeListenersOnNode(path[j], true);
        }

        // Phase 2: At Target Phase
        if (!event._stopped && !event._stopImmediate) {
            event.eventPhase = 2;
            invokeListenersOnNode(target, false);
        }

        // Phase 3: Bubbling Phase (target's parent -> document -> window)
        if (event.bubbles && !event._stopped && !event._stopImmediate) {
            event.eventPhase = 3;
            for (var k = 1; k < path.length; k++) {
                if (event._stopped || event._stopImmediate) break;
                invokeListenersOnNode(path[k], false);
            }
        }

        event.eventPhase = 0;
        event.currentTarget = null;
        return !event.defaultPrevented;
    }

    function _createStyleObject(nodeId) {
        var obj = {
            _props: {},
            getPropertyValue: function(p) {
                if (!p) return "";
                var val = this._props[p];
                if (val !== undefined) return val;
                var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                if (this[camel] !== undefined && typeof this[camel] !== 'function') return String(this[camel]);
                if (this[p] !== undefined && typeof this[p] !== 'function') return String(this[p]);
                return "";
            },
            setProperty: function(p, v) {
                if (!p) return;
                var sVal = String(v);
                this._props[p] = sVal;
                var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                this[camel] = sVal;
                this[p] = sVal;
            },
            removeProperty: function(p) {
                if (!p) return "";
                var prev = this.getPropertyValue(p);
                delete this._props[p];
                var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                delete this[camel];
                delete this[p];
                return prev;
            }
        };
        if (nodeId !== undefined && typeof _getAttribute === 'function') {
            var rawStyle = _getAttribute(nodeId, "style");
            if (rawStyle) {
                var decls = rawStyle.split(';');
                for (var i = 0; i < decls.length; i++) {
                    var d = decls[i].trim();
                    if (!d) continue;
                    var colon = d.indexOf(':');
                    if (colon !== -1) {
                        var prop = d.slice(0, colon).trim();
                        var val = d.slice(colon + 1).trim();
                        obj.setProperty(prop, val);
                    }
                }
            }
        }
        return obj;
    }

    function _createClassList(nodeId) {
        return {
            add: function() {
                var cur = _getAttribute(nodeId, "class") || "";
                var classes = cur.split(/\s+/).filter(Boolean);
                for (var i = 0; i < arguments.length; i++) {
                    if (classes.indexOf(arguments[i]) === -1) classes.push(arguments[i]);
                }
                _setAttribute(nodeId, "class", classes.join(" "));
            },
            remove: function() {
                var cur = _getAttribute(nodeId, "class") || "";
                var classes = cur.split(/\s+/).filter(Boolean);
                for (var i = 0; i < arguments.length; i++) {
                    var idx = classes.indexOf(arguments[i]);
                    if (idx !== -1) classes.splice(idx, 1);
                }
                _setAttribute(nodeId, "class", classes.join(" "));
            },
            contains: function(cls) {
                var cur = _getAttribute(nodeId, "class") || "";
                return cur.split(/\s+/).indexOf(cls) !== -1;
            },
            toggle: function(cls) {
                if (this.contains(cls)) { this.remove(cls); return false; }
                else { this.add(cls); return true; }
            }
        };
    }

    function _createHTMLCollection(getItemsFn) {
        var proto = (typeof globalThis.HTMLCollection !== 'undefined') ? globalThis.HTMLCollection.prototype : Object.prototype;
        var base = Object.create(proto);
        base.item = function(index) {
            var items = getItemsFn() || [];
            var i = Number(index);
            if (isNaN(i) || i < 0 || i >= items.length) return null;
            return items[i];
        };
        base.namedItem = function(name) {
            var items = getItemsFn() || [];
            var str = String(name);
            for (var i = 0; i < items.length; i++) {
                var it = items[i];
                if (it && (it.id === str || (it.getAttribute && it.getAttribute('name') === str))) {
                    return it;
                }
            }
            return null;
        };
        base[Symbol.iterator] = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: items[i++], done: false };
                    }
                    return { value: undefined, done: true };
                }
            };
        };
        base[Symbol.toStringTag] = 'HTMLCollection';

        return new Proxy(base, {
            get: function(target, prop, receiver) {
                if (prop === 'length') {
                    return (getItemsFn() || []).length;
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var items = getItemsFn() || [];
                        return items[idx];
                    }
                }
                if (prop in target) {
                    return target[prop];
                }
                if (typeof prop === 'string') {
                    return target.namedItem(prop) || undefined;
                }
                return undefined;
            },
            has: function(target, prop) {
                if (prop === 'length') return true;
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return idx < (getItemsFn() || []).length;
                    }
                    if (target.namedItem(prop) !== null) return true;
                }
                return prop in target;
            },
            ownKeys: function(target) {
                var items = getItemsFn() || [];
                var keys = [];
                for (var i = 0; i < items.length; i++) {
                    keys.push(String(i));
                }
                keys.push('length');
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                if (prop === 'length') {
                    return { value: (getItemsFn() || []).length, writable: false, enumerable: false, configurable: true };
                }
                var idx = Number(prop);
                if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                    var items = getItemsFn() || [];
                    if (idx < items.length) {
                        return { value: items[idx], writable: false, enumerable: true, configurable: true };
                    }
                }
                return Object.getOwnPropertyDescriptor(target, prop);
            }
        });
    }

    function _createNodeList(getItemsFn) {
        var proto = (typeof globalThis.NodeList !== 'undefined') ? globalThis.NodeList.prototype : Object.prototype;
        var base = Object.create(proto);
        base.item = function(index) {
            var items = getItemsFn() || [];
            var i = Number(index);
            if (isNaN(i) || i < 0 || i >= items.length) return null;
            return items[i];
        };
        base.forEach = function(callback, thisArg) {
            var items = getItemsFn() || [];
            for (var i = 0; i < items.length; i++) {
                callback.call(thisArg, items[i], i, this);
            }
        };
        base.entries = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: [i, items[i++]], done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base.keys = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: i++, done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base.values = function() {
            var items = getItemsFn() || [];
            var i = 0;
            return {
                next: function() {
                    if (i < items.length) {
                        return { value: items[i++], done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base[Symbol.iterator] = function() {
            return this.values();
        };
        base[Symbol.toStringTag] = 'NodeList';

        return new Proxy(base, {
            get: function(target, prop, receiver) {
                if (prop === 'length') {
                    return (getItemsFn() || []).length;
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var items = getItemsFn() || [];
                        return items[idx];
                    }
                }
                if (prop in target) {
                    return target[prop];
                }
                return undefined;
            },
            has: function(target, prop) {
                if (prop === 'length') return true;
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return idx < (getItemsFn() || []).length;
                    }
                }
                return prop in target;
            },
            ownKeys: function(target) {
                var items = getItemsFn() || [];
                var keys = [];
                for (var i = 0; i < items.length; i++) {
                    keys.push(String(i));
                }
                keys.push('length');
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                if (prop === 'length') {
                    return { value: (getItemsFn() || []).length, writable: false, enumerable: false, configurable: true };
                }
                var idx = Number(prop);
                if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                    var items = getItemsFn() || [];
                    if (idx < items.length) {
                        return { value: items[idx], writable: false, enumerable: true, configurable: true };
                    }
                }
                return Object.getOwnPropertyDescriptor(target, prop);
            }
        });
    }

    // ── Path2D & Canvas 2D Level 2 ───────────────────────────────────────────────
    function _parseSvgPath(path, d) {
        var str = String(d || '').trim();
        if (!str) return;
        var re = /([a-df-z])([^a-df-z]*)/gi;
        var curX = 0, curY = 0, startX = 0, startY = 0;
        var match;
        while ((match = re.exec(str)) !== null) {
            var cmd = match[1];
            var argsStr = match[2].trim();
            var numRe = /[-+]?(?:\d*\.\d+|\d+)(?:[eE][-+]?\d+)?/g;
            var nums = [];
            var numMatch;
            while ((numMatch = numRe.exec(argsStr)) !== null) {
                nums.push(parseFloat(numMatch[0]));
            }
            var isRel = (cmd === cmd.toLowerCase());
            var c = cmd.toUpperCase();
            var i = 0;
            switch(c) {
                case 'M':
                    while (i + 1 < nums.length || (i === 0 && nums.length >= 2)) {
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        startX = x; startY = y;
                        path.moveTo(x, y);
                        c = 'L';
                    }
                    break;
                case 'L':
                    while (i + 1 < nums.length) {
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        path.lineTo(x, y);
                    }
                    break;
                case 'H':
                    while (i < nums.length) {
                        var x = nums[i++];
                        if (isRel) x += curX;
                        curX = x;
                        path.lineTo(curX, curY);
                    }
                    break;
                case 'V':
                    while (i < nums.length) {
                        var y = nums[i++];
                        if (isRel) y += curY;
                        curY = y;
                        path.lineTo(curX, curY);
                    }
                    break;
                case 'C':
                    while (i + 5 < nums.length) {
                        var x1 = nums[i++], y1 = nums[i++];
                        var x2 = nums[i++], y2 = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) {
                            x1 += curX; y1 += curY;
                            x2 += curX; y2 += curY;
                            x += curX; y += curY;
                        }
                        curX = x; curY = y;
                        path.bezierCurveTo(x1, y1, x2, y2, x, y);
                    }
                    break;
                case 'S':
                    while (i + 3 < nums.length) {
                        var x2 = nums[i++], y2 = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x2 += curX; y2 += curY; x += curX; y += curY; }
                        var x1 = curX, y1 = curY;
                        curX = x; curY = y;
                        path.bezierCurveTo(x1, y1, x2, y2, x, y);
                    }
                    break;
                case 'Q':
                    while (i + 3 < nums.length) {
                        var cx = nums[i++], cy = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { cx += curX; cy += curY; x += curX; y += curY; }
                        curX = x; curY = y;
                        path.quadraticCurveTo(cx, cy, x, y);
                    }
                    break;
                case 'T':
                    while (i + 1 < nums.length) {
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        path.quadraticCurveTo(curX, curY, x, y);
                    }
                    break;
                case 'A':
                    while (i + 6 < nums.length) {
                        var rx = nums[i++], ry = nums[i++];
                        var rot = nums[i++], largeArc = nums[i++], sweep = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        path.arcTo(curX, curY, x, y, Math.max(rx, ry));
                    }
                    break;
                case 'Z':
                    path.closePath();
                    curX = startX; curY = startY;
                    break;
            }
        }
    }

    function Path2D(pathOrSvg) {
        this._isPath2D = true;
        this._ops = [];
        if (pathOrSvg instanceof Path2D) {
            this._ops = pathOrSvg._ops.map(function(op) { return op.slice(); });
        } else if (typeof pathOrSvg === 'string') {
            _parseSvgPath(this, pathOrSvg);
        }
    }
    Path2D.prototype = {
        addPath: function(path, transform) {
            if (path && path._ops) {
                for (var i = 0; i < path._ops.length; i++) {
                    this._ops.push(path._ops[i].slice());
                }
            }
        },
        closePath: function() { this._ops.push(['closePath']); },
        moveTo: function(x, y) { this._ops.push(['moveTo', Number(x)||0, Number(y)||0]); },
        lineTo: function(x, y) { this._ops.push(['lineTo', Number(x)||0, Number(y)||0]); },
        bezierCurveTo: function(cp1x, cp1y, cp2x, cp2y, x, y) {
            this._ops.push(['bezierCurveTo', Number(cp1x)||0, Number(cp1y)||0, Number(cp2x)||0, Number(cp2y)||0, Number(x)||0, Number(y)||0]);
        },
        quadraticCurveTo: function(cpx, cpy, x, y) {
            this._ops.push(['quadraticCurveTo', Number(cpx)||0, Number(cpy)||0, Number(x)||0, Number(y)||0]);
        },
        arc: function(x, y, radius, startAngle, endAngle, counterclockwise) {
            this._ops.push(['arc', Number(x)||0, Number(y)||0, Number(radius)||0, Number(startAngle)||0, Number(endAngle)||0, !!counterclockwise]);
        },
        arcTo: function(x1, y1, x2, y2, radius) {
            this._ops.push(['arcTo', Number(x1)||0, Number(y1)||0, Number(x2)||0, Number(y2)||0, Number(radius)||0]);
        },
        ellipse: function(x, y, radiusX, radiusY, rotation, startAngle, endAngle, counterclockwise) {
            this._ops.push(['arc', Number(x)||0, Number(y)||0, Number(radiusX)||0, Number(startAngle)||0, Number(endAngle)||0, !!counterclockwise]);
        },
        rect: function(x, y, w, h) {
            this._ops.push(['rect', Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0]);
        },
        roundRect: function(x, y, w, h, radii) {
            this._ops.push(['rect', Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0]);
        }
    };
    globalThis.Path2D = Path2D;

    var _nextOffscreenId = 9000000;
    function ImageBitmap(canvasOrWidth, height) {
        if (canvasOrWidth && canvasOrWidth.width !== undefined) {
            this.width = canvasOrWidth.width;
            this.height = canvasOrWidth.height;
            this._nodeId = canvasOrWidth._nodeId;
        } else {
            this.width = Number(canvasOrWidth) || 0;
            this.height = Number(height) || 0;
        }
    }
    ImageBitmap.prototype.close = function() {
        this.width = 0;
        this.height = 0;
    };
    globalThis.ImageBitmap = ImageBitmap;

    function OffscreenCanvas(width, height) {
        this.width = Math.max(0, Math.floor(Number(width) || 300));
        this.height = Math.max(0, Math.floor(Number(height) || 150));
        this._nodeId = _nextOffscreenId++;
        this._context2d = null;
    }
    OffscreenCanvas.prototype = {
        getContext: function(type, options) {
            if (type === '2d') {
                if (!this._context2d) {
                    this._context2d = _createCanvasRenderingContext2D(this);
                }
                return this._context2d;
            }
            if (type === 'webgl') {
                if (typeof globalThis.WebGLRenderingContext === 'function') {
                    return new globalThis.WebGLRenderingContext(this);
                }
            }
            if (type === 'webgl2') {
                if (typeof globalThis.WebGL2RenderingContext === 'function') {
                    return new globalThis.WebGL2RenderingContext(this);
                }
            }
            if (type === 'webgpu') {
                if (typeof globalThis.GPUCanvasContext === 'function') {
                    return new globalThis.GPUCanvasContext(this);
                }
            }
            return null;
        },
        transferToImageBitmap: function() {
            return new ImageBitmap(this);
        },
        convertToBlob: function(options) {
            var type = (options && options.type) || 'image/png';
            var dataUrl = _canvasToDataUrl(this._nodeId, type);
            var bCtor = globalThis.Blob || function(p, opts){ return { parts: p, type: opts && opts.type }; };
            return Promise.resolve(new bCtor([dataUrl], { type: type }));
        }
    };
    globalThis.OffscreenCanvas = OffscreenCanvas;

    function _createCanvasRenderingContext2D(canvas) {
        var ctx = {
            _canvas: canvas,
            _fillStyle: '#000000',
            _strokeStyle: '#000000',
            _lineWidth: 1,
            _lineCap: 'butt',
            _lineJoin: 'miter',
            _miterLimit: 10,
            _globalAlpha: 1.0,
            _font: '10px sans-serif',
            _textAlign: 'start',
            _textBaseline: 'alphabetic',
            _filter: 'none',
            shadowBlur: 0,
            shadowColor: 'rgba(0, 0, 0, 0)',
            shadowOffsetX: 0,
            shadowOffsetY: 0,
            globalCompositeOperation: 'source-over',
            imageSmoothingEnabled: true,

            get canvas() { return this._canvas; },

            get filter() { return this._filter || 'none'; },
            set filter(val) { this._filter = String(val || 'none'); },

            get fillStyle() { return this._fillStyle; },
            set fillStyle(val) {
                if (typeof val === 'string') {
                    this._fillStyle = val;
                    _canvasSetFillStyle(this._canvas._nodeId, val);
                } else if (val && typeof val === 'object') {
                    this._fillStyle = val;
                }
            },

            get strokeStyle() { return this._strokeStyle; },
            set strokeStyle(val) {
                if (typeof val === 'string') {
                    this._strokeStyle = val;
                    _canvasSetStrokeStyle(this._canvas._nodeId, val);
                } else if (val && typeof val === 'object') {
                    this._strokeStyle = val;
                }
            },

            get lineWidth() { return this._lineWidth; },
            set lineWidth(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num > 0) {
                    this._lineWidth = num;
                    _canvasSetLineWidth(this._canvas._nodeId, num);
                }
            },

            get lineCap() { return this._lineCap; },
            set lineCap(val) {
                this._lineCap = String(val);
                _canvasSetLineCap(this._canvas._nodeId, this._lineCap);
            },

            get lineJoin() { return this._lineJoin; },
            set lineJoin(val) {
                this._lineJoin = String(val);
                _canvasSetLineJoin(this._canvas._nodeId, this._lineJoin);
            },

            get miterLimit() { return this._miterLimit; },
            set miterLimit(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num > 0) {
                    this._miterLimit = num;
                    _canvasSetMiterLimit(this._canvas._nodeId, num);
                }
            },

            get globalAlpha() { return this._globalAlpha; },
            set globalAlpha(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num >= 0 && num <= 1) {
                    this._globalAlpha = num;
                    _canvasSetGlobalAlpha(this._canvas._nodeId, num);
                }
            },

            get font() { return this._font; },
            set font(val) {
                this._font = String(val);
                _canvasSetFont(this._canvas._nodeId, this._font);
            },

            get textAlign() { return this._textAlign; },
            set textAlign(val) {
                this._textAlign = String(val);
                _canvasSetTextAlign(this._canvas._nodeId, this._textAlign);
            },

            get textBaseline() { return this._textBaseline; },
            set textBaseline(val) {
                this._textBaseline = String(val);
                _canvasSetTextBaseline(this._canvas._nodeId, this._textBaseline);
            },

            fillRect: function(x, y, w, h) {
                _canvasFillRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },
            strokeRect: function(x, y, w, h) {
                _canvasStrokeRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },
            clearRect: function(x, y, w, h) {
                _canvasClearRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },

            beginPath: function() {
                _canvasBeginPath(this._canvas._nodeId);
            },
            closePath: function() {
                _canvasClosePath(this._canvas._nodeId);
            },
            moveTo: function(x, y) {
                _canvasMoveTo(this._canvas._nodeId, Number(x)||0, Number(y)||0);
            },
            lineTo: function(x, y) {
                _canvasLineTo(this._canvas._nodeId, Number(x)||0, Number(y)||0);
            },
            rect: function(x, y, w, h) {
                var nx = Number(x) || 0;
                var ny = Number(y) || 0;
                var nw = Number(w) || 0;
                var nh = Number(h) || 0;
                this.moveTo(nx, ny);
                this.lineTo(nx + nw, ny);
                this.lineTo(nx + nw, ny + nh);
                this.lineTo(nx, ny + nh);
                this.closePath();
            },
            arc: function(x, y, radius, startAngle, endAngle, counterclockwise) {
                _canvasArc(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(radius)||0, Number(startAngle)||0, Number(endAngle)||0, !!counterclockwise);
            },
            arcTo: function(x1, y1, x2, y2, radius) {
                this.lineTo(Number(x1) || 0, Number(y1) || 0);
            },
            bezierCurveTo: function(cp1x, cp1y, cp2x, cp2y, x, y) {
                this.lineTo(Number(x) || 0, Number(y) || 0);
            },
            quadraticCurveTo: function(cpx, cpy, x, y) {
                this.lineTo(Number(x) || 0, Number(y) || 0);
            },

            _applyPath2D: function(path) {
                if (!path || !path._ops) return;
                this.beginPath();
                for (var i = 0; i < path._ops.length; i++) {
                    var op = path._ops[i];
                    var fn = this[op[0]];
                    if (typeof fn === 'function') {
                        fn.apply(this, op.slice(1));
                    }
                }
            },

            fill: function(pathOrRule, rule) {
                var fillRule = 'nonzero';
                if (pathOrRule && pathOrRule._isPath2D) {
                    this._applyPath2D(pathOrRule);
                    fillRule = rule || 'nonzero';
                } else if (typeof pathOrRule === 'string') {
                    fillRule = pathOrRule;
                }
                _canvasFill(this._canvas._nodeId, String(fillRule || 'nonzero'));
            },
            stroke: function(path) {
                if (path && path._isPath2D) {
                    this._applyPath2D(path);
                }
                _canvasStroke(this._canvas._nodeId);
            },
            clip: function(pathOrRule, rule) {
                if (pathOrRule && pathOrRule._isPath2D) {
                    this._applyPath2D(pathOrRule);
                }
            },
            isPointInPath: function(pathOrX, xOrY, yOrRule, rule) { return false; },

            save: function() {
                _canvasSave(this._canvas._nodeId);
            },
            restore: function() {
                _canvasRestore(this._canvas._nodeId);
            },
            scale: function(sx, sy) {
                _canvasScale(this._canvas._nodeId, Number(sx)||1, Number(sy)||1);
            },
            rotate: function(angle) {
                _canvasRotate(this._canvas._nodeId, Number(angle)||0);
            },
            translate: function(dx, dy) {
                _canvasTranslate(this._canvas._nodeId, Number(dx)||0, Number(dy)||0);
            },
            transform: function(a, b, c, d, e, f) {
                _canvasTransform(this._canvas._nodeId, Number(a)||1, Number(b)||0, Number(c)||0, Number(d)||1, Number(e)||0, Number(f)||0);
            },
            setTransform: function(a, b, c, d, e, f) {
                if (arguments.length === 0) {
                    _canvasResetTransform(this._canvas._nodeId);
                } else {
                    _canvasSetTransform(this._canvas._nodeId, Number(a)||1, Number(b)||0, Number(c)||0, Number(d)||1, Number(e)||0, Number(f)||0);
                }
            },
            resetTransform: function() {
                _canvasResetTransform(this._canvas._nodeId);
            },
            getTransform: function() {
                return { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
            },

            fillText: function(text, x, y, maxWidth) {
                var mw = maxWidth !== undefined ? Number(maxWidth) : null;
                _canvasFillText(this._canvas._nodeId, String(text), Number(x)||0, Number(y)||0, mw);
            },
            strokeText: function(text, x, y, maxWidth) {
                var mw = maxWidth !== undefined ? Number(maxWidth) : null;
                _canvasStrokeText(this._canvas._nodeId, String(text), Number(x)||0, Number(y)||0, mw);
            },
            measureText: function(text) {
                var str = String(text || '');
                var w = _canvasMeasureText(this._canvas._nodeId, str);
                return {
                    width: w,
                    actualBoundingBoxAscent: 10,
                    actualBoundingBoxDescent: 2,
                    fontBoundingBoxAscent: 10,
                    fontBoundingBoxDescent: 2
                };
            },

            getImageData: function(sx, sy, sw, sh) {
                var w = Math.max(1, Math.floor(sw || 1));
                var h = Math.max(1, Math.floor(sh || 1));
                var arr = _canvasGetImageData(this._canvas._nodeId, Math.floor(sx || 0), Math.floor(sy || 0), w, h);
                var u8 = new Uint8ClampedArray(arr);
                if (typeof globalThis.ImageData === 'function') {
                    return new globalThis.ImageData(u8, w, h);
                }
                return { data: u8, width: w, height: h };
            },
            putImageData: function(imgData, dx, dy, dirtyX, dirtyY, dirtyWidth, dirtyHeight) {
                if (!imgData || !imgData.data) return;
                var w = imgData.width || 0;
                var h = imgData.height || 0;
                var dX = dirtyX !== undefined ? Math.floor(dirtyX) : 0;
                var dY = dirtyY !== undefined ? Math.floor(dirtyY) : 0;
                var dW = dirtyWidth !== undefined ? Math.floor(dirtyWidth) : (w - dX);
                var dH = dirtyHeight !== undefined ? Math.floor(dirtyHeight) : (h - dY);
                var raw = [];
                for (var i = 0; i < imgData.data.length; i++) {
                    raw.push(imgData.data[i]);
                }
                _canvasPutImageData(this._canvas._nodeId, raw, Math.floor(dx || 0), Math.floor(dy || 0), dX, dY, dW, dH, w);
            },
            createImageData: function(w, h) {
                var width = Math.max(1, Math.floor(w || 1));
                var height = Math.max(1, Math.floor(h || 1));
                var u8 = new Uint8ClampedArray(width * height * 4);
                if (typeof globalThis.ImageData === 'function') {
                    return new globalThis.ImageData(u8, width, height);
                }
                return { data: u8, width: width, height: height };
            },
            createLinearGradient: function(x0, y0, x1, y1) {
                return {
                    addColorStop: function(offset, color) {}
                };
            },
            createRadialGradient: function(x0, y0, r0, x1, y1, r1) {
                return {
                    addColorStop: function(offset, color) {}
                };
            },
            createConicGradient: function(startAngle, x, y) {
                var stops = [];
                return {
                    addColorStop: function(offset, color) {
                        stops.push({ offset: Number(offset) || 0, color: String(color) });
                    },
                    _type: 'conic',
                    _startAngle: Number(startAngle) || 0,
                    _x: Number(x) || 0,
                    _y: Number(y) || 0,
                    _stops: stops
                };
            },
            createPattern: function(image, repetition) {
                return {};
            },
            drawImage: function() {}
        };

        // Initialize backing store with the canvas's width and height
        _canvasResize(canvas._nodeId, canvas.width, canvas.height);

        if (typeof globalThis.CanvasRenderingContext2D === 'function') {
            Object.setPrototypeOf(ctx, globalThis.CanvasRenderingContext2D.prototype);
        }
        return ctx;
    }
    globalThis._createCanvasRenderingContext2D = _createCanvasRenderingContext2D;

    // Per-node wrapper cache. DOM element identity requires that wrapping the
    // same node twice yields the same object (`document.querySelector('#a') ===
    // document.querySelector('#a')`), and listeners/styles registered through
    // one handle must stay reachable when that node is wrapped again — e.g.
    // when the chrome dispatches `input`/`change` events from Rust.
    var _wrapperCache = {};

    function _invokeConnectedCallback(node) {
        if (!node || typeof node !== 'object') return;
        if (typeof node.connectedCallback === 'function' && !node._isCustomConnected) {
            node._isCustomConnected = true;
            try { node.connectedCallback(); } catch(e) {}
        }
        var kids = node.childNodes;
        if (kids) {
            for (var i = 0; i < kids.length; i++) {
                _invokeConnectedCallback(kids[i]);
            }
        }
    }

    function _invokeDisconnectedCallback(node) {
        if (!node || typeof node !== 'object') return;
        if (typeof node.disconnectedCallback === 'function' && node._isCustomConnected) {
            node._isCustomConnected = false;
            try { node.disconnectedCallback(); } catch(e) {}
        }
        var kids = node.childNodes;
        if (kids) {
            for (var i = 0; i < kids.length; i++) {
                _invokeDisconnectedCallback(kids[i]);
            }
        }
    }

    function _invokeAttributeChangedCallback(el, name, oldVal, newVal) {
        if (!el || typeof el !== 'object') return;
        if (oldVal === newVal) return;
        var tag = el.tagName ? String(el.tagName).toLowerCase() : '';
        var ctor = (typeof globalThis !== 'undefined' && globalThis.customElements && globalThis.customElements.get && tag)
            ? globalThis.customElements.get(tag)
            : null;
        ctor = ctor || el.constructor;
        var obs = (ctor && ctor.observedAttributes) || el.observedAttributes;
        if (Array.isArray(obs)) {
            var matches = false;
            for (var i = 0; i < obs.length; i++) {
                if (String(obs[i]).toLowerCase() === String(name).toLowerCase()) {
                    matches = true;
                    break;
                }
            }
            if (matches && typeof el.attributeChangedCallback === 'function') {
                try {
                    el.attributeChangedCallback(name, oldVal, newVal, null);
                } catch(e) {}
            }
        }
    }

    function _wrapElement(nodeId) {
        if (nodeId === null || nodeId === undefined) return null;
        if (_wrapperCache[nodeId]) return _wrapperCache[nodeId];
        var el = {
            _nodeId: nodeId,
            _listeners: {},
            get textContent() { return _getTextContent(this._nodeId); },
            set textContent(val) {
                var oldVal = _getTextContent(this._nodeId);
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
            set innerHTML(val) { _setInnerHTML(this._nodeId, String(val)); },
            get outerHTML() { return _getOuterHTML(this._nodeId); },
            set outerHTML(val) { _setOuterHTML(this._nodeId, String(val)); },
            get nodeType() { return _getNodeType(this._nodeId); },
            get nodeName() { return _getNodeName(this._nodeId); },
            get nodeValue() {
                var v = _getNodeValue(this._nodeId);
                if (v !== null && v !== undefined) return v;
                if (this.nodeType === 1) return _getTextContent(this._nodeId);
                return null;
            },
            set nodeValue(val) {
                var oldVal = this.nodeValue;
                var newVal = val === null ? '' : String(val);
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
                }
                return undefined;
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
                            return _getDatasetKeys(self._nodeId) || [];
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
                if (!this._classList) this._classList = _createClassList(this._nodeId);
                return this._classList;
            },
            get style() {
                if (!this._style) this._style = _createStyleObject(this._nodeId);
                return this._style;
            },
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
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'form') {
                    return (this.elements || []).length;
                }
                if (tag === 'select') {
                    return (this.options || []).length;
                }
                var l = this.getAttribute('length');
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
                var listId = this.getAttribute('list');
                if (!listId || typeof document === 'undefined') return null;
                var el = document.getElementById(listId);
                return (el && el.tagName && el.tagName.toLowerCase() === 'datalist') ? el : null;
            },
            // <map> & <area>
            get areas() {
                var tag = (this.tagName || '').toLowerCase();
                if (tag === 'map') {
                    return this.getElementsByTagName('area');
                }
                return undefined;
            },
            get shape() { return this.getAttribute('shape') || 'rect'; },
            set shape(val) { this.setAttribute('shape', String(val)); },
            get coords() { return this.getAttribute('coords') || ''; },
            set coords(val) { this.setAttribute('coords', String(val)); },
            get alt() { return this.getAttribute('alt') || ''; },
            set alt(val) { this.setAttribute('alt', String(val)); },
            get target() { return this.getAttribute('target') || ''; },
            set target(val) { this.setAttribute('target', String(val)); },
            // <object> & <embed> and CharacterData / Comment
            get data() {
                if (this.nodeType === 8 || this.nodeType === 3) {
                    return this.nodeValue || '';
                }
                return this.getAttribute('data') || '';
            },
            set data(val) {
                if (this.nodeType === 8 || this.nodeType === 3) {
                    this.nodeValue = String(val);
                    return;
                }
                this.setAttribute('data', String(val));
            },
            get htmlFor() { return this.getAttribute('for') || ''; },
            set htmlFor(val) { this.setAttribute('for', String(val)); },
            get contentDocument() {
                if (this.tagName && this.tagName.toLowerCase() === 'iframe') {
                    if (!this._contentDocument) {
                        this._contentDocument = {
                            readyState: 'complete',
                            body: _wrapElement(_origCE('body')),
                            head: _wrapElement(_origCE('head')),
                            createElement: function(t) { return _wrapElement(_origCE(t)); },
                            createTextNode: function(t) { return _wrapElement(_origCTN(t)); },
                            getElementById: function(id) { return null; },
                            querySelector: function(s) { return null; },
                            querySelectorAll: function(s) { return []; }
                        };
                    }
                    return this._contentDocument;
                }
                return null;
            },
            get contentWindow() {
                if (this.tagName && this.tagName.toLowerCase() === 'iframe') {
                    if (!this._contentWindow) {
                        var doc = this.contentDocument;
                        this._contentWindow = {
                            document: doc,
                            frameElement: this,
                            postMessage: function(msg, targetOrigin) {},
                            addEventListener: function() {},
                            removeEventListener: function() {}
                        };
                    }
                    return this._contentWindow;
                }
                return null;
            },
            appendChild: function(child) {
                var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
                _appendChild(this._nodeId, childId);
                var childObj = _wrapElement(childId);
                if (childObj && childObj.isConnected) {
                    _invokeConnectedCallback(childObj);
                }
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [childObj],
                        removedNodes: []
                    });
                }
                return child;
            },
            removeChild: function(child) {
                var childId = (typeof child === 'object' && child !== null && child._nodeId !== undefined) ? child._nodeId : child;
                var childObj = _wrapElement(childId);
                _removeChild(this._nodeId, childId);
                if (childObj) {
                    _invokeDisconnectedCallback(childObj);
                }
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [],
                        removedNodes: [childObj]
                    });
                }
                return child;
            },
            insertBefore: function(newChild, refChild) {
                var childId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
                var refId = (typeof refChild === 'object' && refChild !== null && refChild._nodeId !== undefined) ? refChild._nodeId : refChild;
                _insertBefore(this._nodeId, childId, (refId === undefined || refId === null) ? 0 : refId);
                var newObj = _wrapElement(childId);
                var refObj = (refId && refId !== 0) ? _wrapElement(refId) : null;
                if (newObj && newObj.isConnected) {
                    _invokeConnectedCallback(newObj);
                }
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [newObj],
                        removedNodes: [],
                        nextSibling: refObj
                    });
                }
                return newChild;
            },
            replaceChild: function(newChild, oldChild) {
                var newId = (typeof newChild === 'object' && newChild !== null && newChild._nodeId !== undefined) ? newChild._nodeId : newChild;
                var oldId = (typeof oldChild === 'object' && oldChild !== null && oldChild._nodeId !== undefined) ? oldChild._nodeId : oldChild;
                var oldObj = _wrapElement(oldId);
                var newObj = _wrapElement(newId);
                _replaceChild(this._nodeId, newId, oldId);
                if (oldObj) {
                    _invokeDisconnectedCallback(oldObj);
                }
                if (newObj && newObj.isConnected) {
                    _invokeConnectedCallback(newObj);
                }
                if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
                    globalThis._reportDOMMutation({
                        type: 'childList',
                        target: this,
                        addedNodes: [newObj],
                        removedNodes: [oldObj]
                    });
                }
                return oldChild;
            },
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
            assignedNodes: function(opts) {
                if (this.tagName && this.tagName.toLowerCase() === 'slot') {
                    var p = this.parentElement;
                    while (p && !p.host) p = p.parentElement;
                    var src = (p && p.host) ? p.host.childNodes : this.childNodes;
                    var arr = [];
                    if (src) {
                        for (var i = 0; i < src.length; i++) arr.push(src[i]);
                    }
                    return arr;
                }
                return [];
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
                var w = r[2];
                var h = r[3];
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
                var id = _querySelector(this._nodeId, String(sel));
                return id !== null ? _wrapElement(id) : null;
            },
            querySelectorAll: function(sel) {
                var ids = _querySelectorAll(this._nodeId, String(sel)) || [];
                var list = ids.map(_wrapElement);
                return _createNodeList(function() { return list; });
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
            getAttributeNames: function() { return _getAttributeNames(this._nodeId) || []; },
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
            cloneNode: function(deep) { return _wrapElement(_cloneNode(this._nodeId, !!deep)); },
            get isConnected() {
                var cur = this;
                while (cur) {
                    if (cur === document) return true;
                    var p = cur.parentElement || cur.parentNode;
                    if (!p) break;
                    cur = p;
                }
                return false;
            },
            get ownerDocument() {
                return (this === document) ? null : document;
            },
            matches: function(sel) { return _elementMatches(this._nodeId, String(sel)); },
            closest: function(sel) {
                var matchId = _elementClosest(this._nodeId, String(sel));
                return matchId !== null ? _wrapElement(matchId) : null;
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
                            else _removeChild(pId, opt._nodeId);
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
                    } else if (globalThis.HTMLElement) {
                        Object.setPrototypeOf(el, globalThis.HTMLElement.prototype);
                    }
                }
            }
        }
        _wrapperCache[nodeId] = el;
        return el;
    }

    var _origGetById = document.getElementById;
    document.getElementById = function(id) { return _wrapElement(_origGetById(id)); };
    // Chrome-internal handle used by Rust-triggered scripts to reach the cached
    // wrapper (and therefore its listeners) for a specific node id.
    try { globalThis._mangoWrap = _wrapElement; } catch (e) {}
    document.querySelector = function(sel) {
        var id = _querySelector(0, String(sel));
        return id !== null ? _wrapElement(id) : null;
    };
    document.querySelectorAll = function(sel) {
        var ids = _querySelectorAll(0, String(sel)) || [];
        var list = ids.map(_wrapElement);
        return _createNodeList(function() { return list; });
    };
    document.getElementsByTagName = function(tag) {
        return _createHTMLCollection(function() {
            var ids = _querySelectorAll(0, String(tag)) || [];
            return ids.map(_wrapElement);
        });
    };
    document.getElementsByClassName = function(cls) {
        return _createHTMLCollection(function() {
            var ids = _querySelectorAll(0, "." + String(cls)) || [];
            return ids.map(_wrapElement);
        });
    };
    var _origCE = document.createElement;
    document.createElement = function(tag) {
        var el = _wrapElement(_origCE(tag));
        if (el) {
            var customCtor = (globalThis.customElements && globalThis.customElements.get) ? globalThis.customElements.get(String(tag).toLowerCase()) : null;
            if (customCtor && typeof customCtor === 'function' && !el._constructed) {
                el._constructed = true;
                try {
                    var constructed = Reflect.construct(customCtor, [], customCtor);
                    var props = Object.getOwnPropertyNames(constructed);
                    for (var i = 0; i < props.length; i++) {
                        var p = props[i];
                        if (p !== 'constructor') {
                            el[p] = constructed[p];
                        }
                    }
                } catch (e) {
                    try { customCtor.call(el); } catch (e2) {}
                }
            }
        }
        return el;
    };
    document.createElementNS = function(ns, tag) { return document.createElement(tag); };
    var _origCTN = document.createTextNode;
    document.createTextNode = function(text) { return _wrapElement(_origCTN(text)); };
    var _origCC = (typeof globalThis !== 'undefined' && globalThis._origCC) ? globalThis._origCC : (document.createComment || function(t) { return 0; });
    document.createComment = function(text) { return _wrapElement(_origCC(String(text === undefined ? '' : text))); };

    document.addEventListener = function(type, listener, options) {
        _addDOMEventListener(document, type, listener, options);
        if (type === 'DOMContentLoaded' || type === 'load') {
            setTimeout(function() {
                try {
                    if (typeof listener === 'function') listener.call(document, { type: type, target: document, bubbles: false });
                    else if (listener && typeof listener.handleEvent === 'function') listener.handleEvent({ type: type, target: document, bubbles: false });
                } catch(e) {}
            }, 0);
        }
    };
    document.removeEventListener = function(type, listener, options) {
        _removeDOMEventListener(document, type, listener, options);
    };
    document.dispatchEvent = function(evt) {
        return _dispatchDOMEvent(document, evt);
    };
    // `document.cookie` — a live view over the shared, persisted cookie jar.
    // HttpOnly cookies never appear here; assignments are applied to the jar.
    Object.defineProperty(document, 'cookie', {
        get: function() { return _getDocumentCookie(); },
        set: function(v) { _setDocumentCookie(String(v)); },
        configurable: true
    });
    Object.defineProperty(document, 'compatMode', {
        get: function() { return _getCompatMode(); }
    });
    Object.defineProperty(document, 'characterSet', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'charset', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'inputEncoding', {
        get: function() { return _getCharacterSet(); },
        configurable: true
    });
    Object.defineProperty(document, 'contentType', {
        get: function() { return _getContentType(); },
        configurable: true
    });
    document.readyState = "complete";
    try {
        document.location = globalThis.location;
    } catch(e) {}
    document.createDocumentFragment = function() {
        return _wrapElement(_createFragment());
    };

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
        _appendChild(0, childId);
        var childObj = _wrapElement(childId);
        if (childObj && childObj.isConnected) {
            _invokeConnectedCallback(childObj);
        }
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [childObj],
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
        _insertBefore(0, childId, (refId === undefined || refId === null) ? 0 : refId);
        var newObj = _wrapElement(childId);
        var refObj = (refId && refId !== 0) ? _wrapElement(refId) : null;
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [newObj],
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
        _replaceChild(0, newId, oldId);
        if (typeof globalThis !== 'undefined' && globalThis._reportDOMMutation) {
            globalThis._reportDOMMutation({
                type: 'childList',
                target: document,
                addedNodes: [newObj],
                removedNodes: [oldObj]
            });
        }
        return oldChild;
    };
    document.cloneNode = function(deep) {
        return _wrapElement(_cloneNode(0, !!deep));
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

    function _getNodeIndex(node) {
        if (!node || !node.parentNode) return 0;
        var siblings = node.parentNode.childNodes || [];
        for (var i = 0; i < siblings.length; i++) {
            if (siblings[i] === node || (siblings[i]._nodeId !== undefined && siblings[i]._nodeId === node._nodeId)) {
                return i;
            }
        }
        return 0;
    }

    function _getNodeLength(node) {
        if (!node) return 0;
        if (node.nodeType === 3 || node.nodeType === 8) {
            return (node.nodeValue !== null && node.nodeValue !== undefined) ? String(node.nodeValue).length : (node.textContent || '').length;
        }
        return (node.childNodes || []).length;
    }

    function _getCommonAncestor(a, b) {
        if (!a || !b) return a || b || document;
        if (a === b || (a._nodeId !== undefined && a._nodeId === b._nodeId)) return a;
        var aAncestors = [];
        var cur = a;
        while (cur) {
            aAncestors.push(cur);
            cur = cur.parentNode;
        }
        cur = b;
        while (cur) {
            for (var i = 0; i < aAncestors.length; i++) {
                if (aAncestors[i] === cur || (aAncestors[i]._nodeId !== undefined && aAncestors[i]._nodeId === cur._nodeId)) {
                    return cur;
                }
            }
            cur = cur.parentNode;
        }
        return (typeof document !== 'undefined') ? document : a;
    }

    function _isDescendantOf(node, ancestor) {
        var cur = node ? node.parentNode : null;
        while (cur) {
            if (cur === ancestor || (cur._nodeId !== undefined && cur._nodeId === ancestor._nodeId)) return true;
            cur = cur.parentNode;
        }
        return false;
    }

    function _comparePoints(nodeA, offsetA, nodeB, offsetB) {
        if (nodeA === nodeB || (nodeA && nodeB && nodeA._nodeId !== undefined && nodeA._nodeId === nodeB._nodeId)) {
            return offsetA < offsetB ? -1 : (offsetA > offsetB ? 1 : 0);
        }
        if (_isDescendantOf(nodeB, nodeA)) {
            var child = nodeB;
            while (child.parentNode && child.parentNode !== nodeA && !(child.parentNode._nodeId !== undefined && child.parentNode._nodeId === nodeA._nodeId)) {
                child = child.parentNode;
            }
            var idx = _getNodeIndex(child);
            return idx < offsetA ? 1 : -1;
        }
        if (_isDescendantOf(nodeA, nodeB)) {
            var child = nodeA;
            while (child.parentNode && child.parentNode !== nodeB && !(child.parentNode._nodeId !== undefined && child.parentNode._nodeId === nodeB._nodeId)) {
                child = child.parentNode;
            }
            var idx = _getNodeIndex(child);
            return offsetB <= idx ? 1 : -1;
        }
        var common = _getCommonAncestor(nodeA, nodeB);
        if (!common) return 0;
        var childA = nodeA;
        while (childA.parentNode && childA.parentNode !== common && !(childA.parentNode._nodeId !== undefined && childA.parentNode._nodeId === common._nodeId)) {
            childA = childA.parentNode;
        }
        var childB = nodeB;
        while (childB.parentNode && childB.parentNode !== common && !(childB.parentNode._nodeId !== undefined && childB.parentNode._nodeId === common._nodeId)) {
            childB = childB.parentNode;
        }
        var idxA = _getNodeIndex(childA);
        var idxB = _getNodeIndex(childB);
        return idxA < idxB ? -1 : 1;
    }

    function Range() {
        this.startContainer = (typeof document !== 'undefined') ? (document.body || document) : null;
        this.startOffset = 0;
        this.endContainer = this.startContainer;
        this.endOffset = 0;
        this.collapsed = true;
        this.commonAncestorContainer = this.startContainer;
    }

    Range.START_TO_START = 0;
    Range.START_TO_END = 1;
    Range.END_TO_END = 2;
    Range.END_TO_START = 3;

    Range.prototype.START_TO_START = 0;
    Range.prototype.START_TO_END = 1;
    Range.prototype.END_TO_END = 2;
    Range.prototype.END_TO_START = 3;

    Range.prototype._updateState = function() {
        this.collapsed = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId)) && (this.startOffset === this.endOffset);
        this.commonAncestorContainer = _getCommonAncestor(this.startContainer, this.endContainer);
    };

    Range.prototype.setStart = function(node, offset) {
        if (!node) throw new Error("TypeError: Node must not be null");
        offset = Math.max(0, parseInt(offset, 10) || 0);
        this.startContainer = node;
        this.startOffset = offset;
        if (!this.endContainer || _comparePoints(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
            this.endContainer = node;
            this.endOffset = offset;
        }
        this._updateState();
    };

    Range.prototype.setEnd = function(node, offset) {
        if (!node) throw new Error("TypeError: Node must not be null");
        offset = Math.max(0, parseInt(offset, 10) || 0);
        this.endContainer = node;
        this.endOffset = offset;
        if (!this.startContainer || _comparePoints(this.startContainer, this.startOffset, this.endContainer, this.endOffset) > 0) {
            this.startContainer = node;
            this.startOffset = offset;
        }
        this._updateState();
    };

    Range.prototype.setStartBefore = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setStart(node.parentNode, _getNodeIndex(node));
    };

    Range.prototype.setStartAfter = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setStart(node.parentNode, _getNodeIndex(node) + 1);
    };

    Range.prototype.setEndBefore = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setEnd(node.parentNode, _getNodeIndex(node));
    };

    Range.prototype.setEndAfter = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        this.setEnd(node.parentNode, _getNodeIndex(node) + 1);
    };

    Range.prototype.collapse = function(toStart) {
        if (toStart) {
            this.endContainer = this.startContainer;
            this.endOffset = this.startOffset;
        } else {
            this.startContainer = this.endContainer;
            this.startOffset = this.endOffset;
        }
        this._updateState();
    };

    Range.prototype.selectNode = function(node) {
        if (!node || !node.parentNode) throw new Error("InvalidNodeTypeError");
        var idx = _getNodeIndex(node);
        this.setStart(node.parentNode, idx);
        this.setEnd(node.parentNode, idx + 1);
    };

    Range.prototype.selectNodeContents = function(node) {
        if (!node) throw new Error("TypeError: Node must not be null");
        this.setStart(node, 0);
        this.setEnd(node, _getNodeLength(node));
    };

    Range.prototype.compareBoundaryPoints = function(how, sourceRange) {
        if (!sourceRange) throw new Error("TypeError: sourceRange must not be null");
        if (how === 0) {
            return _comparePoints(this.startContainer, this.startOffset, sourceRange.startContainer, sourceRange.startOffset);
        } else if (how === 1) {
            return _comparePoints(this.endContainer, this.endOffset, sourceRange.startContainer, sourceRange.startOffset);
        } else if (how === 2) {
            return _comparePoints(this.endContainer, this.endOffset, sourceRange.endContainer, sourceRange.endOffset);
        } else if (how === 3) {
            return _comparePoints(this.startContainer, this.startOffset, sourceRange.endContainer, sourceRange.endOffset);
        }
        throw new Error("NotSupportedError: Invalid comparison method");
    };

    Range.prototype.cloneRange = function() {
        var r = new Range();
        r.setStart(this.startContainer, this.startOffset);
        r.setEnd(this.endContainer, this.endOffset);
        return r;
    };

    Range.prototype.deleteContents = function() {
        if (this.collapsed) return;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = s.substring(0, this.startOffset) + s.substring(this.endOffset);
            } else if (this.startContainer.childNodes) {
                var kids = Array.prototype.slice.call(this.startContainer.childNodes);
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    this.startContainer.removeChild(kids[i]);
                }
            }
        } else {
            if (this.startContainer.nodeType === 3) {
                var s1 = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = s1.substring(0, this.startOffset);
            }
            if (this.endContainer.nodeType === 3) {
                var s2 = this.endContainer.nodeValue || '';
                this.endContainer.nodeValue = s2.substring(this.endOffset);
            }
        }
        this.collapse(true);
    };

    Range.prototype.cloneContents = function() {
        var frag = _wrapElement(_createFragment());
        if (this.collapsed) return frag;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s)));
            } else if (this.startContainer.childNodes) {
                var kids = this.startContainer.childNodes;
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    if (kids[i] && kids[i].cloneNode) frag.appendChild(kids[i].cloneNode(true));
                }
            }
        } else {
            if (this.startContainer.nodeType === 3) {
                var s1 = (this.startContainer.nodeValue || '').substring(this.startOffset);
                frag.appendChild(_wrapElement(_origCTN(s1)));
            }
            if (this.endContainer.nodeType === 3) {
                var s2 = (this.endContainer.nodeValue || '').substring(0, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s2)));
            }
        }
        return frag;
    };

    Range.prototype.extractContents = function() {
        var frag = _wrapElement(_createFragment());
        if (this.collapsed) return frag;
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                var s = (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
                frag.appendChild(_wrapElement(_origCTN(s)));
                var full = this.startContainer.nodeValue || '';
                this.startContainer.nodeValue = full.substring(0, this.startOffset) + full.substring(this.endOffset);
            } else if (this.startContainer.childNodes) {
                var kids = Array.prototype.slice.call(this.startContainer.childNodes);
                for (var i = this.startOffset; i < this.endOffset && i < kids.length; i++) {
                    if (kids[i]) frag.appendChild(kids[i]);
                }
            }
        } else {
            var fragClone = this.cloneContents();
            this.deleteContents();
            return fragClone;
        }
        this.collapse(true);
        return frag;
    };

    Range.prototype.insertNode = function(node) {
        if (!node) throw new Error("TypeError: Node must not be null");
        if (this.startContainer.nodeType === 3) {
            var full = this.startContainer.nodeValue || '';
            var before = full.substring(0, this.startOffset);
            var after = full.substring(this.startOffset);
            this.startContainer.nodeValue = before;
            var afterNode = _wrapElement(_origCTN(after));
            var p = this.startContainer.parentNode;
            if (p) {
                p.insertBefore(afterNode, this.startContainer.nextSibling);
                p.insertBefore(node, afterNode);
            }
        } else {
            var ref = (this.startContainer.childNodes && this.startContainer.childNodes[this.startOffset]) || null;
            this.startContainer.insertBefore(node, ref);
        }
    };

    Range.prototype.surroundContents = function(newParent) {
        if (!newParent) throw new Error("TypeError: newParent must not be null");
        var frag = this.extractContents();
        while (newParent.firstChild) newParent.removeChild(newParent.firstChild);
        this.insertNode(newParent);
        newParent.appendChild(frag);
        this.selectNode(newParent);
    };

    Range.prototype.createContextualFragment = function(html) {
        var frag = _wrapElement(_createFragment());
        frag.innerHTML = String(html);
        return frag;
    };

    Range.prototype.toString = function() {
        if (this.collapsed) return '';
        var isSame = (this.startContainer === this.endContainer || (this.startContainer && this.endContainer && this.startContainer._nodeId !== undefined && this.startContainer._nodeId === this.endContainer._nodeId));
        if (isSame) {
            if (this.startContainer.nodeType === 3 || this.startContainer.nodeType === 8) {
                return (this.startContainer.nodeValue || '').substring(this.startOffset, this.endOffset);
            }
            return (this.startContainer.textContent || '');
        }
        if (this.startContainer.nodeType === 3 && this.endContainer.nodeType === 3) {
            var s1 = (this.startContainer.nodeValue || '').substring(this.startOffset);
            var s2 = (this.endContainer.nodeValue || '').substring(0, this.endOffset);
            return s1 + s2;
        }
        return (this.commonAncestorContainer && this.commonAncestorContainer.textContent) ? this.commonAncestorContainer.textContent : '';
    };

    Range.prototype.isPointInRange = function(node, offset) {
        return _comparePoints(node, offset, this.startContainer, this.startOffset) >= 0 && _comparePoints(node, offset, this.endContainer, this.endOffset) <= 0;
    };

    Range.prototype.comparePoint = function(node, offset) {
        if (_comparePoints(node, offset, this.startContainer, this.startOffset) < 0) return -1;
        if (_comparePoints(node, offset, this.endContainer, this.endOffset) > 0) return 1;
        return 0;
    };

    Range.prototype.intersectsNode = function(node) {
        if (!node) return false;
        var p = node.parentNode;
        if (!p) return false;
        var idx = _getNodeIndex(node);
        return _comparePoints(p, idx, this.endContainer, this.endOffset) < 0 && _comparePoints(p, idx + 1, this.startContainer, this.startOffset) > 0;
    };

    Range.prototype.getBoundingClientRect = function() {
        var startNode = this.startContainer;
        while (startNode && startNode.nodeType !== 1) startNode = startNode.parentElement || startNode.parentNode;
        var endNode = this.endContainer;
        while (endNode && endNode.nodeType !== 1) endNode = endNode.parentElement || endNode.parentNode;

        var startRect = (startNode && typeof startNode.getBoundingClientRect === 'function') ? startNode.getBoundingClientRect() : null;
        var endRect = (endNode && typeof endNode.getBoundingClientRect === 'function') ? endNode.getBoundingClientRect() : null;

        if (startRect && endRect && startRect !== endRect) {
            var left = Math.min(startRect.left, endRect.left);
            var top = Math.min(startRect.top, endRect.top);
            var right = Math.max(startRect.right, endRect.right);
            var bottom = Math.max(startRect.bottom, endRect.bottom);
            var w = Math.max(0, right - left);
            var h = Math.max(0, bottom - top);
            return {
                x: left, y: top, left: left, top: top, right: right, bottom: bottom, width: w, height: h,
                toJSON: function() { return this; }
            };
        }
        if (startRect) return startRect;
        if (endRect) return endRect;
        return { x: 0, y: 0, top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0, toJSON: function() { return this; } };
    };

    Range.prototype.getClientRects = function() {
        var r = this.getBoundingClientRect();
        if (r && (r.width > 0 || r.height > 0)) return [r];
        return [];
    };

    Range.prototype.detach = function() {};

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

    globalThis.Range = Range;
    globalThis.Selection = Selection;

    document.createRange = function() {
        return new Range();
    };
    document.getSelection = function() {
        if (!document._selection) {
            document._selection = new Selection();
        }
        return document._selection;
    };
    if (typeof window !== 'undefined') {
        window.getSelection = function() {
            return document.getSelection();
        };
    }
    document.createComment = function(text) {
        return _wrapElement(_origCC(String(text === undefined ? '' : text)));
    };
    document.createEvent = function(type) {
        var evt = (typeof globalThis.Event !== 'undefined') ? new globalThis.Event(type || "") : { type: type || "" };
        evt.initEvent = function(t, b, c) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
        };
        evt.initCustomEvent = function(t, b, c, detail) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
            this.detail = detail;
        };
        evt.initMouseEvent = function(t, b, c) {
            this.type = String(t);
            this.bubbles = !!b;
            this.cancelable = !!c;
        };
        return evt;
    };
    document.createTreeWalker = function(root, whatToShow, filter) {
        return {
            root: root,
            currentNode: root,
            whatToShow: whatToShow || -1,
            filter: filter,
            nextNode: function() { return null; },
            previousNode: function() { return null; },
            parentNode: function() { return null; },
            firstChild: function() { return null; },
            lastChild: function() { return null; },
            nextSibling: function() { return null; },
            previousSibling: function() { return null; }
        };
    };
    document.createNodeIterator = function(root, whatToShow, filter) {
        return {
            root: root,
            whatToShow: whatToShow || -1,
            filter: filter,
            nextNode: function() { return null; },
            previousNode: function() { return null; },
            detach: function() {}
        };
    };
    document.importNode = function(node, deep) {
        return (node && node.cloneNode) ? node.cloneNode(deep) : node;
    };
    document.adoptNode = function(node) { return node; };
    document.defaultView = globalThis;
    document.hasFocus = function() { return true; };
    document.implementation = {
        createHTMLDocument: function(title) {
            return {
                title: title || '',
                createElement: function(t) { return _wrapElement(_origCE(t)); },
                createElementNS: function(ns, t) { return _wrapElement(_origCE(t)); },
                createTextNode: function(t) { return _wrapElement(_origCTN(t)); },
                createDocumentFragment: function() { return _wrapElement(_createFragment()); },
                querySelector: function() { return null; },
                querySelectorAll: function() { return _createNodeList(function() { return []; }); },
                getElementById: function() { return null; },
                getElementsByTagName: function() { return _createHTMLCollection(function() { return []; }); },
                body: _wrapElement(_origCE('body')),
                head: _wrapElement(_origCE('head'))
            };
        },
        hasFeature: function() { return true; },
        createDocument: function() { return document; }
    };

    Object.defineProperty(document, 'body', {
        get: function() { return document.querySelector('body'); }
    });
    Object.defineProperty(document, 'head', {
        get: function() { return document.querySelector('head'); }
    });
    Object.defineProperty(document, 'documentElement', {
        get: function() { return document.querySelector('html') || document.body; }
    });
    Object.defineProperty(document, 'title', {
        get: function() {
            var t = document.querySelector('title');
            return t ? t.textContent : "";
        },
        set: function(val) {
            var t = document.querySelector('title');
            if (t) t.textContent = val;
        }
    });
    Object.defineProperty(document, 'currentScript', {
        get: function() {
            var s = document.getElementsByTagName('script');
            return s && s.length > 0 ? s[s.length - 1] : null;
        },
        configurable: true
    });
    document.fonts = {
        ready: Promise.resolve(),
        check: function() { return true; },
        load: function() { return Promise.resolve([]); },
        addEventListener: function() {},
        removeEventListener: function() {},
        dispatchEvent: function() { return true; }
    };

    var _docAdopted = [];
    Object.defineProperty(document, 'adoptedStyleSheets', {
        get: function() { return _docAdopted; },
        set: function(sheets) {
            if (!Array.isArray(sheets)) throw new TypeError("adoptedStyleSheets must be an Array");
            for (var i = 0; i < sheets.length; i++) {
                if (!(sheets[i] instanceof globalThis.CSSStyleSheet)) {
                    throw new TypeError("Items in adoptedStyleSheets must be CSSStyleSheet instances");
                }
            }
            _docAdopted = sheets.slice();
        },
        configurable: true,
        enumerable: true
    });

    document._nodeId = 0;
    document.nodeType = 9;
    document.nodeName = '#document';
    if (typeof globalThis !== 'undefined' && globalThis.Document && globalThis.Document.prototype) {
        Object.defineProperty(globalThis.Document.prototype, 'adoptedStyleSheets', {
            get: function() { return this === document ? _docAdopted : (this._adoptedStyleSheets || []); },
            set: function(sheets) {
                if (!Array.isArray(sheets)) throw new TypeError("adoptedStyleSheets must be an Array");
                for (var i = 0; i < sheets.length; i++) {
                    if (!(sheets[i] instanceof globalThis.CSSStyleSheet)) {
                        throw new TypeError("Items in adoptedStyleSheets must be CSSStyleSheet instances");
                    }
                }
                if (this === document) {
                    _docAdopted = sheets.slice();
                } else {
                    this._adoptedStyleSheets = sheets.slice();
                }
            },
            configurable: true,
            enumerable: true
        });
        try { Object.setPrototypeOf(document, globalThis.Document.prototype); } catch(e) {}
    }
    globalThis._addDOMEventListener = _addDOMEventListener;
    globalThis._removeDOMEventListener = _removeDOMEventListener;
    globalThis._dispatchDOMEvent = _dispatchDOMEvent;
    globalThis._dispatchInternalEvent = function(nodeId, type, dict) {
        var target;
        if (nodeId === -1 || nodeId === 'window') {
            target = (typeof globalThis !== 'undefined' && globalThis.window) ? globalThis.window : document;
        } else if (nodeId === null || nodeId === undefined || nodeId === 0 || nodeId === 'document') {
            target = document;
        } else {
            target = _wrapElement(nodeId);
        }
        if (!target) target = document;
        dict = dict || {};
        var ctor = (typeof globalThis.Event === 'function') ? globalThis.Event : function(t, d) {
            this.type = t;
            this.bubbles = !!(d && d.bubbles);
            this.cancelable = !!(d && d.cancelable);
        };
        if (dict.eventType === 'mouse' && typeof globalThis.MouseEvent === 'function') ctor = globalThis.MouseEvent;
        else if (dict.eventType === 'keyboard' && typeof globalThis.KeyboardEvent === 'function') ctor = globalThis.KeyboardEvent;
        else if (dict.eventType === 'focus' && typeof globalThis.FocusEvent === 'function') ctor = globalThis.FocusEvent;
        else if (dict.eventType === 'input' && typeof globalThis.InputEvent === 'function') ctor = globalThis.InputEvent;
        else if (dict.eventType === 'custom' && typeof globalThis.CustomEvent === 'function') ctor = globalThis.CustomEvent;
        else if (dict.eventType === 'wheel' && typeof globalThis.WheelEvent === 'function') ctor = globalThis.WheelEvent;
        else if (dict.eventType === 'touch' && typeof globalThis.TouchEvent === 'function') ctor = globalThis.TouchEvent;
        else if (dict.eventType === 'ui' && typeof globalThis.UIEvent === 'function') ctor = globalThis.UIEvent;
        else if (dict.eventType === 'submit' && typeof globalThis.SubmitEvent === 'function') ctor = globalThis.SubmitEvent;
        var evt = new ctor(type, dict);
        return _dispatchDOMEvent(target, evt);
    };
    "#;

/// Deep- or shallow-clones a node (and its attributes) into the same document.
pub(crate) fn clone_subtree(doc: &mut Document, node_id: NodeId, deep: bool) -> Option<NodeId> {
    let data = doc.get(node_id)?.data.clone();
    let new_id = match data {
        NodeData::Element(el) => doc.create_element(&el.tag_name, el.attributes.clone()),
        NodeData::Text(t) => doc.create_text(&t),
        NodeData::Comment(c) => doc.create_comment(&c),
        NodeData::DocumentFragment => doc.create_fragment(),
        NodeData::DocumentType {
            name,
            public_id,
            system_id,
        } => doc.create_doctype(&name, &public_id, &system_id),
        NodeData::Document => return None,
    };
    if deep {
        let children: Vec<NodeId> = doc.children(node_id).map(|n| n.id).collect();
        for child in children {
            if let Some(cloned) = clone_subtree(doc, child, true) {
                doc.append_child(new_id, cloned);
            }
        }
        if let Some(NodeData::Element(orig_el)) = doc.get(node_id).map(|n| &n.data)
            && let Some(orig_frag) = orig_el.template_contents
            && let Some(new_frag) = doc.template_contents(new_id)
        {
            let frag_children: Vec<NodeId> = doc.children(orig_frag).map(|n| n.id).collect();
            for fchild in frag_children {
                if let Some(cloned) = clone_subtree(doc, fchild, true) {
                    doc.append_child(new_frag, cloned);
                }
            }
        }
    }
    Some(new_id)
}

/// Copies all element/text/comment children of `source` into `target` under `parent`.
///
/// Used by `innerHTML`/`outerHTML` setters: the fragment is parsed into a temporary
/// `Document` and then adopted into the live tree.
pub(crate) fn adopt_children(source: &Document, source_id: NodeId, target: &mut Document, parent: NodeId) {
    for child in source.children(source_id) {
        match &child.data {
            NodeData::Element(el) => {
                let new_id = target.create_element(&el.tag_name, el.attributes.clone());
                target.append_child(parent, new_id);
                adopt_children(source, child.id, target, new_id);
            }
            NodeData::Text(t) => {
                let new_id = target.create_text(t);
                target.append_child(parent, new_id);
            }
            NodeData::Comment(c) => {
                let new_id = target.create_comment(c);
                target.append_child(parent, new_id);
            }
            _ => {}
        }
    }
}

/// Copies all element/text/comment children of `source` into `target` under `parent`,
/// inserting before `before_node`.
pub(crate) fn adopt_children_before(
    source: &Document,
    source_id: NodeId,
    target: &mut Document,
    parent: NodeId,
    before_node: NodeId,
) {
    for child in source.children(source_id) {
        match &child.data {
            NodeData::Element(el) => {
                let new_id = target.create_element(&el.tag_name, el.attributes.clone());
                let _ = target.insert_before(parent, new_id, before_node);
                adopt_children(source, child.id, target, new_id);
            }
            NodeData::Text(t) => {
                let new_id = target.create_text(t);
                let _ = target.insert_before(parent, new_id, before_node);
            }
            NodeData::Comment(c) => {
                let new_id = target.create_comment(c);
                let _ = target.insert_before(parent, new_id, before_node);
            }
            _ => {}
        }
    }
}

/// Collects document-level collections (`document.forms`, `document.images`, …).
pub(crate) fn collect_elements_by_kind(doc: &Document, node_id: NodeId, kind: &str, out: &mut Vec<NodeId>) {
    for child in doc.children(node_id) {
        if let NodeData::Element(el) = &child.data {
            let tag = el.tag_name.to_ascii_lowercase();
            let matches = match kind {
                "forms" => tag == "form",
                "images" => tag == "img",
                "links" => (tag == "a" || tag == "area") && el.get_attribute("href").is_some(),
                "anchors" => tag == "a" && el.get_attribute("name").is_some(),
                "scripts" => tag == "script",
                "embeds" => tag == "embed" || tag == "object",
                "all" => true,
                _ => false,
            };
            if matches {
                out.push(child.id);
            }
        }
        collect_elements_by_kind(doc, child.id, kind, out);
    }
}

/// Recursively finds an element by its `id` attribute.
pub(crate) fn find_element_by_id_recursive(
    doc: &Document,
    node_id: NodeId,
    target_id: &str,
) -> Option<NodeId> {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
        && elem.id() == Some(target_id)
    {
        return Some(node_id);
    }
    for child in doc.children(node_id) {
        if let Some(found) = find_element_by_id_recursive(doc, child.id, target_id) {
            return Some(found);
        }
    }
    None
}

/// Simple CSS selector matching: supports `#id`, `.class`, and `tagname`.
pub(crate) fn query_selector_simple(doc: &Document, node_id: NodeId, selector: &str) -> Option<NodeId> {
    let selector = selector.trim();
    if let Some(id) = selector.strip_prefix('#') {
        return find_element_by_id_recursive(doc, node_id, id);
    }
    if let Some(class) = selector.strip_prefix('.') {
        return find_element_by_class_recursive(doc, node_id, class);
    }
    doc.find_element_by_tag(node_id, selector)
}

pub(crate) fn find_element_by_class_recursive(
    doc: &Document,
    node_id: NodeId,
    target_class: &str,
) -> Option<NodeId> {
    if let Some(node) = doc.get(node_id)
        && let NodeData::Element(ref elem) = node.data
        && elem.has_class(target_class)
    {
        return Some(node_id);
    }
    for child in doc.children(node_id) {
        if let Some(found) = find_element_by_class_recursive(doc, child.id, target_class) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn find_first_matching_element(
    doc: &Document,
    current: NodeId,
    selectors: &mango_css::SelectorList,
) -> Option<NodeId> {
    for child in doc.children(current) {
        if matches!(child.data, NodeData::Element(_)) {
            if selectors.matches(child.id, doc) {
                return Some(child.id);
            }
            if let Some(found) = find_first_matching_element(doc, child.id, selectors) {
                return Some(found);
            }
        }
    }
    None
}

pub(crate) fn collect_matching_elements(
    doc: &Document,
    current: NodeId,
    selectors: &mango_css::SelectorList,
    out: &mut Vec<NodeId>,
) {
    for child in doc.children(current) {
        if matches!(child.data, NodeData::Element(_)) {
            if selectors.matches(child.id, doc) {
                out.push(child.id);
            }
            collect_matching_elements(doc, child.id, selectors, out);
        }
    }
}
