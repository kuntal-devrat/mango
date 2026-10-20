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

