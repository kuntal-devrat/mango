    globalThis.Element.prototype.closest = function(sel) {
        var cur = this;
        while (cur && (cur.nodeType === 1 || cur.nodeType === 9)) {
            if (cur.matches && cur.matches(sel)) return cur;
            cur = cur.parentElement || cur.parentNode || cur.host;
        }
        return null;
    };
    globalThis.Element.prototype.checkVisibility = function(options) {
        var checkOpacity = options && options.checkOpacity;
        var checkVisibilityCSS = options && options.checkVisibilityCSS;
        if (typeof window !== 'undefined' && window.getComputedStyle) {
            var cs = window.getComputedStyle(this);
            if (!cs) return false;
            if (cs.display === 'none') return false;
            if (cs.contentVisibility === 'hidden') return false;
            if (checkVisibilityCSS && cs.visibility === 'hidden') return false;
            if (checkOpacity && (cs.opacity === '0' || Number(cs.opacity) === 0)) return false;
        }
        var rect = this.getBoundingClientRect();
        return rect.width > 0 || rect.height > 0 || (this.getClientRects && this.getClientRects().length > 0);
    };
    globalThis.Element.prototype.focus         = function() {
        if (this.focus && this.focus !== globalThis.Element.prototype.focus) return this.focus.apply(this, arguments);
        if (this.disabled) return;
        if (typeof document !== 'undefined') {
            var prev = document.activeElement;
            if (prev === this) return;
            document.activeElement = this;
            if (prev && prev !== this && prev.dispatchEvent) {
                if (globalThis._setFocusState && prev._nodeId) globalThis._setFocusState(prev._nodeId, false);
                prev.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('blur', { bubbles: false, relatedTarget: this }));
                prev.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusout', { bubbles: true, relatedTarget: this }));
            }
            if (globalThis._setFocusState && this._nodeId) globalThis._setFocusState(this._nodeId, true);
            this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focus', { bubbles: false, relatedTarget: prev }));
            this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusin', { bubbles: true, relatedTarget: prev }));
        }
    };
    globalThis.Element.prototype.blur          = function() {
        if (this.blur && this.blur !== globalThis.Element.prototype.blur) return this.blur.apply(this, arguments);
        if (globalThis._setFocusState && this._nodeId) globalThis._setFocusState(this._nodeId, false);
        if (typeof document !== 'undefined' && document.activeElement === this) {
            document.activeElement = document.body || null;
        }
        var next = (typeof document !== 'undefined') ? document.activeElement : null;
        this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('blur', { bubbles: false, relatedTarget: next }));
        this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusout', { bubbles: true, relatedTarget: next }));
    };
    globalThis.Element.prototype.click         = function() {
        if (this.click && this.click !== globalThis.Element.prototype.click) return this.click.apply(this, arguments);
        if (this.disabled) return;
        var evt = (typeof globalThis.MouseEvent === 'function')
            ? new globalThis.MouseEvent('click', { bubbles: true, cancelable: true, view: (typeof window !== 'undefined' ? window : null), detail: 1 })
            : { type: 'click', target: this, bubbles: true, cancelable: true, defaultPrevented: false };
        var notPrevented = this.dispatchEvent(evt);
        if (notPrevented !== false && !evt.defaultPrevented) {
            var tag = (this.tagName || '').toLowerCase();
            if (tag === 'input') {
                var itype = (this.type || (this.getAttribute && this.getAttribute('type')) || 'text').toLowerCase();
                if (itype === 'checkbox') this.checked = !this.checked;
                else if (itype === 'radio') this.checked = true;
            }
        }
    };
    globalThis.Element.prototype.scrollIntoView = function(options) {
        var rect = this.getBoundingClientRect();
        var alignToTop = (options === undefined || options === true || (typeof options === 'object' && options.block !== 'end'));
        var top = (globalThis.scrollY || 0) + (alignToTop ? rect.top : (rect.bottom - ((typeof window !== 'undefined' && window.innerHeight) || 600)));
        var left = (globalThis.scrollX || 0) + rect.left;
        var behavior = (typeof options === 'object' && options.behavior) || 'auto';
        if (typeof globalThis.scrollTo === 'function') {
            globalThis.scrollTo({ top: top, left: left, behavior: behavior });
        }
    };
    function _animateElemScroll(el, targetLeft, targetTop) {
        var startLeft = el.scrollLeft || 0;
        var startTop = el.scrollTop || 0;
        var endLeft = (targetLeft !== undefined) ? targetLeft : startLeft;
        var endTop = (targetTop !== undefined) ? targetTop : startTop;
        var startTime = Date.now();
        var duration = 300;
        function step() {
            var elapsed = Date.now() - startTime;
            var progress = Math.min(1.0, elapsed / duration);
            var ease = 1.0 - Math.pow(1.0 - progress, 3);
            el.scrollLeft = startLeft + (endLeft - startLeft) * ease;
            el.scrollTop = startTop + (endTop - startTop) * ease;
            if (progress < 1.0) {
                if (typeof globalThis.requestAnimationFrame === 'function') {
                    globalThis.requestAnimationFrame(step);
                } else if (typeof setTimeout === 'function') {
                    setTimeout(step, 16);
                }
            }
        }
        step();
    }
    globalThis.Element.prototype.scrollTo      = function(x, y) {
        if (typeof x === 'object' && x !== null) {
            if (x.behavior === 'smooth') {
                _animateElemScroll(this, x.left, x.top);
            } else {
                if (x.left !== undefined) this.scrollLeft = x.left;
                if (x.top !== undefined) this.scrollTop = x.top;
            }
        } else {
            if (x !== undefined) this.scrollLeft = x;
            if (y !== undefined) this.scrollTop = y;
        }
    };
    globalThis.Element.prototype.scrollBy      = function(x, y) {
        if (typeof x === 'object' && x !== null) {
            var curLeft = this.scrollLeft || 0;
            var curTop = this.scrollTop || 0;
            var targetLeft = (x.left !== undefined) ? curLeft + x.left : curLeft;
            var targetTop = (x.top !== undefined) ? curTop + x.top : curTop;
            if (x.behavior === 'smooth') {
                _animateElemScroll(this, targetLeft, targetTop);
            } else {
                this.scrollLeft = targetLeft;
                this.scrollTop = targetTop;
            }
        } else {
            if (x !== undefined) this.scrollLeft += x;
            if (y !== undefined) this.scrollTop += y;
        }
    };
    globalThis.Element.prototype.scroll        = globalThis.Element.prototype.scrollTo;
    globalThis.Element.prototype.prepend       = function() {
        if (this.prepend && this.prepend !== globalThis.Element.prototype.prepend) return this.prepend.apply(this, arguments);
        if (arguments.length === 0) return;
        var nodes = [];
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            nodes.push((arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg)));
        }
        var first = this.firstChild;
        if (nodes.length === 1) {
            if (first) this.insertBefore(nodes[0], first);
            else if (this.appendChild) this.appendChild(nodes[0]);
        } else {
            if (typeof _ensurePreInsertionValidity === 'function') {
                for (var i = 0; i < nodes.length; i++) _ensurePreInsertionValidity(this, nodes[i], first);
            }
            var frag = (typeof document !== 'undefined' && document.createDocumentFragment) ? document.createDocumentFragment() : null;
            if (frag) {
                for (var i = 0; i < nodes.length; i++) frag.appendChild(nodes[i]);
                if (first) this.insertBefore(frag, first);
                else if (this.appendChild) this.appendChild(frag);
            } else {
                for (var i = 0; i < nodes.length; i++) {
                    if (first) this.insertBefore(nodes[i], first);
                    else if (this.appendChild) this.appendChild(nodes[i]);
                }
            }
        }
    };
    globalThis.Element.prototype.append        = function() {
        if (this.append && this.append !== globalThis.Element.prototype.append) return this.append.apply(this, arguments);
        if (arguments.length === 0) return;
        var nodes = [];
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            nodes.push((arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg)));
        }
        if (nodes.length === 1) {
            if (this.appendChild) this.appendChild(nodes[0]);
        } else {
            if (typeof _ensurePreInsertionValidity === 'function') {
                for (var i = 0; i < nodes.length; i++) _ensurePreInsertionValidity(this, nodes[i], null);
            }
            var frag = (typeof document !== 'undefined' && document.createDocumentFragment) ? document.createDocumentFragment() : null;
            if (frag) {
                for (var i = 0; i < nodes.length; i++) frag.appendChild(nodes[i]);
                if (this.appendChild) this.appendChild(frag);
            } else {
                for (var i = 0; i < nodes.length; i++) {
                    if (this.appendChild) this.appendChild(nodes[i]);
                }
            }
        }
    };
    globalThis.Element.prototype.before        = function() {
        if (this.before && this.before !== globalThis.Element.prototype.before) return this.before.apply(this, arguments);
        var p = this.parentNode;
        if (!p || !p.insertBefore) return;
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            p.insertBefore(node, this);
        }
    };
    globalThis.Element.prototype.after         = function() {
        if (this.after && this.after !== globalThis.Element.prototype.after) return this.after.apply(this, arguments);
        var p = this.parentNode;
        if (!p || !p.insertBefore) return;
        var next = this.nextSibling;
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            p.insertBefore(node, next);
        }
    };
    globalThis.Element.prototype.replaceWith   = function() {
        if (this.replaceWith && this.replaceWith !== globalThis.Element.prototype.replaceWith) return this.replaceWith.apply(this, arguments);
        var p = this.parentNode;
        if (!p) return;
        for (var i = 0; i < arguments.length; i++) {
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            if (p.insertBefore) p.insertBefore(node, this);
        }
        if (p.removeChild) p.removeChild(this);
    };
    globalThis.Element.prototype.replaceChildren = function() {
        if (this.replaceChildren && this.replaceChildren !== globalThis.Element.prototype.replaceChildren) return this.replaceChildren.apply(this, arguments);
        if (typeof _doReplaceChildren === 'function') {
            return _doReplaceChildren(this, arguments, false);
        } else if (typeof globalThis !== 'undefined' && globalThis._doReplaceChildren) {
            return globalThis._doReplaceChildren(this, arguments, false);
        }
    };
    globalThis.Element.prototype.remove        = function() {
        if (this.remove && this.remove !== globalThis.Element.prototype.remove) return this.remove.apply(this, arguments);
        if (this.parentNode && this.parentNode.removeChild) this.parentNode.removeChild(this);
    };
    globalThis.Element.prototype.insertAdjacentHTML = function(pos, html) {
        if (this.insertAdjacentHTML && this.insertAdjacentHTML !== globalThis.Element.prototype.insertAdjacentHTML) return this.insertAdjacentHTML.apply(this, arguments);
        var p = String(pos).toLowerCase();
        if (p !== 'beforebegin' && p !== 'afterbegin' && p !== 'beforeend' && p !== 'afterend') {
            throw new Error("SyntaxError: The value provided ('" + pos + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.");
        }
        if (typeof document !== 'undefined' && document.createRange) {
            var range = document.createRange();
            var frag = range.createContextualFragment(String(html));
            if (p === 'beforebegin') {
                if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(frag, this);
            } else if (p === 'afterbegin') {
                if (this.insertBefore) this.insertBefore(frag, this.firstChild);
            } else if (p === 'beforeend') {
                if (this.appendChild) this.appendChild(frag);
            } else if (p === 'afterend') {
                if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(frag, this.nextSibling);
            }
        }
    };
    globalThis.Element.prototype.insertAdjacentElement = function(pos, el) {
        if (this.insertAdjacentElement && this.insertAdjacentElement !== globalThis.Element.prototype.insertAdjacentElement) return this.insertAdjacentElement.apply(this, arguments);
        var p = String(pos).toLowerCase();
        if (p === 'beforebegin') {
            if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(el, this);
        } else if (p === 'afterbegin') {
            if (this.insertBefore) this.insertBefore(el, this.firstChild);
        } else if (p === 'beforeend') {
            if (this.appendChild) this.appendChild(el);
        } else if (p === 'afterend') {
            if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(el, this.nextSibling);
        } else {
            throw new Error("SyntaxError: Invalid position: " + pos);
        }
        return el;
    };
    globalThis.Element.prototype.insertAdjacentText    = function(pos, text) {
        if (this.insertAdjacentText && this.insertAdjacentText !== globalThis.Element.prototype.insertAdjacentText) return this.insertAdjacentText.apply(this, arguments);
        var node = (typeof document !== 'undefined' && document.createTextNode)
            ? document.createTextNode(String(text))
            : String(text);
        return this.insertAdjacentElement(pos, node);
    };
    globalThis.Element.prototype.animate               = function() { return { finished: Promise.resolve(), cancel: function() {} }; };
    globalThis.Element.prototype.getAnimations         = function() { return []; };

    globalThis.HTMLElement = function HTMLElement() {
        var newTarget = new.target;
        if (!newTarget) {
            if (globalThis._customElementsUpgradeCandidate) {
                return globalThis._customElementsUpgradeCandidate;
            }
            if (this && (this instanceof globalThis.HTMLElement || (typeof globalThis.Element === 'function' && this instanceof globalThis.Element))) {
                return this;
            }
            throw new TypeError("DOM object constructor cannot be called as a function.");
        }
        if (newTarget === globalThis.HTMLElement) {
            return Object.create(globalThis.HTMLElement.prototype);
        }
        if (globalThis._customElementsUpgradeCandidate) {
            var cand = globalThis._customElementsUpgradeCandidate;
            if (newTarget && newTarget.prototype) {
                Object.setPrototypeOf(cand, newTarget.prototype);
            }
            return cand;
        }
        var tagName = (globalThis.customElements && globalThis.customElements.getName)
            ? globalThis.customElements.getName(newTarget)
            : null;
        if (!tagName && (globalThis.customElements && globalThis.customElements._registry)) {
            var reg = globalThis.customElements._registry;
            for (var k in reg) {
                if (reg[k] === newTarget) {
                    tagName = k;
                    break;
                }
            }
        }
        if (!tagName) {
            throw new TypeError("Illegal constructor: element not registered in customElements");
        }
        var el = (typeof globalThis._origCE === 'function' && typeof globalThis._wrapElement === 'function')
            ? globalThis._wrapElement(globalThis._origCE(tagName))
            : ((typeof document !== 'undefined' && document.createElement)
                ? document.createElement(tagName)
                : Object.create(newTarget.prototype));
        el._isCustomConstructed = true;
        if (newTarget && newTarget.prototype) {
            Object.setPrototypeOf(el, newTarget.prototype);
        }
        return el;
    };
    globalThis.HTMLElement.prototype = Object.create(globalThis.Element.prototype);
    globalThis.HTMLElement.prototype.constructor = globalThis.HTMLElement;
    Object.defineProperty(globalThis.HTMLElement.prototype, 'style', {
        get: function() {
            if (!this._style) this._style = (typeof _createStyleObject === 'function') ? _createStyleObject(this._nodeId, this) : {};
            return this._style;
        },
        set: function(val) {
            if (typeof val === 'string') {
                if (typeof this.setAttribute === 'function') this.setAttribute('style', val);
                this._style = (typeof _createStyleObject === 'function') ? _createStyleObject(this._nodeId, this) : {};
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.Element.prototype, 'style', {
        get: function() {
            if (!this._style) this._style = (typeof _createStyleObject === 'function') ? _createStyleObject(this._nodeId, this) : {};
            return this._style;
        },
        set: function(val) {
            if (typeof val === 'string') {
                if (typeof this.setAttribute === 'function') this.setAttribute('style', val);
                this._style = (typeof _createStyleObject === 'function') ? _createStyleObject(this._nodeId, this) : {};
            }
        },
        configurable: true,
        enumerable: true
    });
    globalThis.Element.prototype.getPlayer = function() {
        return this.playerElement || this.ytdPlayer || globalThis._dummyPlayer;
    };
    globalThis.HTMLElement.prototype.getPlayer = function() {
        return this.playerElement || this.ytdPlayer || globalThis._dummyPlayer;
    };
    globalThis.Element.prototype.getPlayerPromise = function() {
        var p = (typeof this.getPlayer === 'function') ? this.getPlayer() : null;
        return (p && typeof p.getPlayerPromise === 'function')
            ? p.getPlayerPromise()
            : (globalThis._dummyPlayerPromise || Promise.resolve(p));
    };
    globalThis.HTMLElement.prototype.getPlayerPromise = function() {
        var p = (typeof this.getPlayer === 'function') ? this.getPlayer() : null;
        return (p && typeof p.getPlayerPromise === 'function')
            ? p.getPlayerPromise()
            : (globalThis._dummyPlayerPromise || Promise.resolve(p));
    };
    globalThis.Element.prototype.getMainAppPlayerPromise = function() {
        var p = (typeof this.getPlayer === 'function') ? this.getPlayer() : null;
        return (p && typeof p.getMainAppPlayerPromise === 'function')
            ? p.getMainAppPlayerPromise()
            : (globalThis._dummyPlayerPromise || Promise.resolve(p));
    };
    globalThis.HTMLElement.prototype.getMainAppPlayerPromise = function() {
        var p = (typeof this.getPlayer === 'function') ? this.getPlayer() : null;
        return (p && typeof p.getMainAppPlayerPromise === 'function')
            ? p.getMainAppPlayerPromise()
            : (globalThis._dummyPlayerPromise || Promise.resolve(p));
    };
    globalThis.Element.prototype.preparePlayer = function() {};
    globalThis.HTMLElement.prototype.preparePlayer = function() {};
    globalThis.Element.prototype.showEngagementPanelScrimChanged_ = function() {};
    globalThis.HTMLElement.prototype.showEngagementPanelScrimChanged_ = function() {};
    Object.defineProperties(globalThis.HTMLElement.prototype, {
        offsetWidth: {
            get: function() { return Math.round(this.getBoundingClientRect().width); },
            configurable: true
        },
        offsetHeight: {
            get: function() { return Math.round(this.getBoundingClientRect().height); },
            configurable: true
        },
        offsetParent: {
            get: function() {
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
            configurable: true
        },
        offsetLeft: {
            get: function() {
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().left - p.getBoundingClientRect().left);
                return Math.round(this.getBoundingClientRect().left);
            },
            configurable: true
        },
        offsetTop: {
            get: function() {
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().top - p.getBoundingClientRect().top);
                return Math.round(this.getBoundingClientRect().top);
            },
            configurable: true
        },
        clientWidth: {
            get: function() {
                var bl = 0, br = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    bl = parseFloat(s.borderLeftWidth) || 0;
                    br = parseFloat(s.borderRightWidth) || 0;
                }
                return Math.max(0, Math.round(this.getBoundingClientRect().width - bl - br));
            },
            configurable: true
        },
        clientHeight: {
            get: function() {
                var bt = 0, bb = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    bt = parseFloat(s.borderTopWidth) || 0;
                    bb = parseFloat(s.borderBottomWidth) || 0;
                }
                return Math.max(0, Math.round(this.getBoundingClientRect().height - bt - bb));
            },
            configurable: true
        },
        clientTop: {
            get: function() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderTopWidth) || 0);
                }
                return 0;
            },
            configurable: true
        },
        clientLeft: {
            get: function() {
                if (typeof window !== 'undefined' && window.getComputedStyle) {
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderLeftWidth) || 0);
                }
                return 0;
            },
            configurable: true
        },
        scrollWidth: {
            get: function() { return Math.max(this.clientWidth, Math.round(this.getBoundingClientRect().width)); },
            configurable: true
        },
        scrollHeight: {
            get: function() { return Math.max(this.clientHeight, Math.round(this.getBoundingClientRect().height)); },
            configurable: true
        },
        scrollTop: {
            get: function() { return this._scrollTop || 0; },
            set: function(v) {
                this._scrollTop = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {
                    if (typeof window !== 'undefined') window.scrollY = this._scrollTop;
                }
            },
            configurable: true
        },
        scrollLeft: {
            get: function() { return this._scrollLeft || 0; },
            set: function(v) {
                this._scrollLeft = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {
                    if (typeof window !== 'undefined') window.scrollX = this._scrollLeft;
                }
            },
            configurable: true
        },
        contentEditable: {
            get: function() {
                var v = this.getAttribute ? this.getAttribute('contenteditable') : null;
                if (v === null) return 'inherit';
                var lower = v.toLowerCase();
                if (lower === '' || lower === 'true') return 'true';
                if (lower === 'false') return 'false';
                return 'inherit';
            },
            set: function(val) {
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
            configurable: true
        },
        isContentEditable: {
            get: function() {
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
            configurable: true
        },
        draggable: {
            get: function() {
                var v = this.getAttribute ? this.getAttribute('draggable') : null;
                return v !== null && v.toLowerCase() === 'true';
            },
            set: function(val) {
                if (this.setAttribute) {
                    this.setAttribute('draggable', val ? 'true' : 'false');
                }
            },
            configurable: true
        }
    });

    globalThis.SVGElement    = function() {};
    globalThis.SVGElement.prototype = Object.create(globalThis.Element.prototype);
    globalThis.SVGSVGElement = function() {};
    globalThis.SVGSVGElement.prototype = Object.create(globalThis.SVGElement.prototype);
    globalThis.SVGSVGElement.prototype.createSVGPoint = function() { return { x:0, y:0, matrixTransform: function() { return this; } }; };
    globalThis.SVGSVGElement.prototype.getScreenCTM   = function() { return { a:1, b:0, c:0, d:1, e:0, f:0, inverse: function() { return this; } }; };
    ['SVGUseElement','SVGGElement','SVGPathElement','SVGCircleElement','SVGRectElement','SVGGraphicsElement'].forEach(function(s) {
        globalThis[s] = function() {}; globalThis[s].prototype = Object.create(globalThis.SVGElement.prototype);
    });

    // Specialised HTML element subclasses
    var _htmlSubclasses = [
        'HTMLTemplateElement','HTMLSlotElement','HTMLScriptElement','HTMLStyleElement','HTMLAnchorElement',
        'HTMLInputElement','HTMLButtonElement','HTMLDivElement','HTMLSpanElement',
        'HTMLIFrameElement','HTMLFormElement','HTMLSelectElement','HTMLOptionElement',
        'HTMLTableElement','HTMLTableRowElement','HTMLTableCellElement',
        'HTMLParagraphElement','HTMLHeadingElement','HTMLImageElement',
        'HTMLLIElement','HTMLUListElement','HTMLOListElement',
        'HTMLTextAreaElement','HTMLLabelElement','HTMLLinkElement','HTMLMetaElement',
        'HTMLTitleElement','HTMLBodyElement','HTMLHtmlElement','HTMLHeadElement',
        'HTMLBaseElement','HTMLBRElement','HTMLHRElement','HTMLPreElement',
        'HTMLDetailsElement','HTMLSummaryElement','HTMLDialogElement',
        'HTMLProgressElement','HTMLMeterElement','HTMLOutputElement',
        'HTMLDataListElement','HTMLFieldSetElement','HTMLLegendElement','HTMLOptGroupElement',
        'HTMLMapElement','HTMLAreaElement','HTMLObjectElement','HTMLEmbedElement',
        'HTMLSourceElement','HTMLPictureElement','HTMLTrackElement',
        'HTMLTimeElement','HTMLDataElement','HTMLRubyElement','HTMLTableSectionElement',
        'HTMLTableColElement','HTMLTableCaptionElement','HTMLDListElement',
        'HTMLQuoteElement','HTMLModElement','HTMLUnknownElement','HTMLCanvasElement',
        'HTMLMediaElement','HTMLVideoElement','HTMLAudioElement',
        'HTMLDirectoryElement','HTMLFontElement','HTMLFrameElement','HTMLFrameSetElement','HTMLParamElement'
    ];
    _htmlSubclasses.forEach(function(name) {
        globalThis[name] = function() {};
        globalThis[name].prototype = Object.create(globalThis.HTMLElement.prototype);
        globalThis[name].prototype.constructor = globalThis[name];
    });
    if (globalThis.HTMLMediaElement && globalThis.HTMLVideoElement) {
        globalThis.HTMLVideoElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
        globalThis.HTMLVideoElement.prototype.constructor = globalThis.HTMLVideoElement;
    }
    if (globalThis.HTMLMediaElement && globalThis.HTMLAudioElement) {
        globalThis.HTMLAudioElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
        globalThis.HTMLAudioElement.prototype.constructor = globalThis.HTMLAudioElement;
    }
    if (globalThis.HTMLStyleElement) {
        Object.defineProperty(globalThis.HTMLStyleElement.prototype, 'sheet', {
            get: function() {
                if (!this._sheet) {
                    if (typeof globalThis.CSSStyleSheet === 'function') {
                        this._sheet = new globalThis.CSSStyleSheet();
                        var text = this.textContent || "";
                        if (text && typeof this._sheet.replaceSync === 'function') {
                            try { this._sheet.replaceSync(text); } catch(e) {}
                        }
                    } else {
                        this._sheet = {
                            cssRules: [],
                            rules: [],
                            insertRule: function(rule, idx) { return 0; },
                            deleteRule: function(idx) {}
                        };
                    }
                }
                return this._sheet;
            },
            configurable: true,
            enumerable: true
        });
    }

    function _setupHyperlinkUtils(proto) {
        function _getURL(el) {
            var raw = el.getAttribute ? (el.getAttribute('href') || '') : (el._href || '');
            var base = (el && el.ownerDocument && el.ownerDocument.baseURI) || ((typeof window !== 'undefined' && window.location && window.location.href) ? window.location.href : 'about:blank');
            try {
                return new URL(raw, base);
            } catch(e) {
                return { href: raw, pathname: raw.charAt(0) === '/' ? raw : '/' + raw, search: '', hash: '', host: '', hostname: '', port: '', protocol: 'https:', origin: '' };
            }
        }

        Object.defineProperties(proto, {
            href: {
                get: function() { return _getURL(this).href; },
                set: function(v) { if (this.setAttribute) this.setAttribute('href', String(v)); else this._href = String(v); },
                configurable: true,
                enumerable: true
            },
            pathname: {
                get: function() { return _getURL(this).pathname; },
                set: function(v) {
                    var u = _getURL(this);
                    u.pathname = String(v);
                    this.href = u.href;
                },
                configurable: true,
                enumerable: true
            },
            search: {
                get: function() { return _getURL(this).search; },
                set: function(v) {
                    var u = _getURL(this);
                    u.search = String(v);
                    this.href = u.href;
                },
                configurable: true,
                enumerable: true
            },
            hash: {
                get: function() { return _getURL(this).hash; },
                set: function(v) {
                    var u = _getURL(this);
                    u.hash = String(v);
                    this.href = u.href;
                },
                configurable: true,
                enumerable: true
            },
            host: {
                get: function() { return _getURL(this).host; },
                configurable: true,
                enumerable: true
            },
            hostname: {
                get: function() { return _getURL(this).hostname; },
                configurable: true,
                enumerable: true
            },
            port: {
                get: function() { return _getURL(this).port; },
                configurable: true,
                enumerable: true
            },
            protocol: {
                get: function() { return _getURL(this).protocol; },
                configurable: true,
                enumerable: true
            },
            origin: {
                get: function() { return _getURL(this).origin; },
                configurable: true,
                enumerable: true
            }
        });
    }

    globalThis.Element.prototype.getAttributeNode = function(name) {
        var val = this.getAttribute(name);
        if (val === null) return null;
        if (!this._attrNodes) this._attrNodes = {};
        var n = String(name);
        if (this._attrNodes[n]) { this._attrNodes[n].value = val; return this._attrNodes[n]; }
        var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
        var attr = (doc && doc.createAttribute) ? doc.createAttribute(name) : { name: name, value: val, specified: true, nodeType: 2, nodeName: name, nodeValue: val };
        attr.value = val; attr.ownerElement = this; attr.ownerDocument = doc; attr._ownerDocument = doc;
        if (globalThis.Attr) Object.setPrototypeOf(attr, globalThis.Attr.prototype);
        this._attrNodes[n] = attr;
        return attr;
    };

    globalThis.Element.prototype.getAttributeNodeNS = function(ns, localName) {
        var val = this.getAttributeNS ? this.getAttributeNS(ns, localName) : this.getAttribute(localName);
        if (val === null) return null;
        if (!this._attrNodes) this._attrNodes = {};
        var ln = String(localName), key = (ns || '') + ':' + ln;
        if (this._attrNodes[key]) { this._attrNodes[key].value = val; return this._attrNodes[key]; }
        var doc = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
        var attr = (doc && doc.createAttributeNS) ? doc.createAttributeNS(ns, localName) : { name: localName, localName: localName, namespaceURI: ns || null, value: val, specified: true, nodeType: 2, nodeName: localName, nodeValue: val };
        attr.value = val; attr.ownerElement = this; attr.ownerDocument = doc; attr._ownerDocument = doc;
        if (globalThis.Attr) Object.setPrototypeOf(attr, globalThis.Attr.prototype);
        this._attrNodes[key] = attr; this._attrNodes[ln] = attr;
        return attr;
    };

    globalThis.Element.prototype.setAttributeNode = function(attr) {
        if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
        if (attr.ownerElement && attr.ownerElement !== this) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute is in use by another element.", "InUseAttributeError");
        }
        if (!this._attrNodes) this._attrNodes = {};
        var old = this.getAttributeNode ? this.getAttributeNode(attr.name) : null;
        this.setAttribute(attr.name, attr.value);
        attr.ownerElement = this; attr.ownerDocument = this.ownerDocument; attr._ownerDocument = this.ownerDocument;
        this._attrNodes[attr.name] = attr;
        return old;
    };

    globalThis.Element.prototype.setAttributeNodeNS = function(attr) {
        if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
        if (attr.ownerElement && attr.ownerElement !== this) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute is in use by another element.", "InUseAttributeError");
        }
        if (!this._attrNodes) this._attrNodes = {};
        var old = this.getAttributeNodeNS ? this.getAttributeNodeNS(attr.namespaceURI, attr.localName) : null;
        if (this.setAttributeNS) {
            this.setAttributeNS(attr.namespaceURI, attr.name, attr.value);
        } else {
            this.setAttribute(attr.name, attr.value);
        }
        attr.ownerElement = this; attr.ownerDocument = this.ownerDocument; attr._ownerDocument = this.ownerDocument;
        var key = (attr.namespaceURI || '') + ':' + attr.localName;
        this._attrNodes[key] = attr; this._attrNodes[attr.localName] = attr; this._attrNodes[attr.name] = attr;
        return old;
    };

    globalThis.Element.prototype.removeAttributeNode = function(attr) {
        if (!attr || attr.nodeType !== 2) throw new TypeError("parameter 1 is not of type 'Attr'");
        if (attr.ownerElement !== this) {
            var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
            throw new DomEx("The attribute is not associated with this element.", "NotFoundError");
        }
        if (this._attrNodes) {
            delete this._attrNodes[attr.name]; delete this._attrNodes[attr.localName];
            delete this._attrNodes[(attr.namespaceURI || '') + ':' + attr.localName];
        }
        attr.ownerElement = null;
        if (attr.namespaceURI && this.removeAttributeNS) {
            this.removeAttributeNS(attr.namespaceURI, attr.localName);
        } else {
            this.removeAttribute(attr.name);
        }
        return attr;
    };
