    function _createStyleObject(nodeId, el) {
        var proto = (typeof globalThis.CSSStyleDeclaration !== 'undefined')
            ? globalThis.CSSStyleDeclaration.prototype
            : Object.prototype;
        var base = Object.create(proto);
        base._nodeId = nodeId;
        base._el = el || null;
        base._props = {};
        base._syncing = false;

        // CSSStyleDeclaration is a live view of the element's existing
        // `style` attribute.  Start with those declarations so changing one
        // property does not serialize a new attribute that drops every other
        // inline declaration (for example, `style.height = '100px'` must
        // preserve the element's width and background).
        var initial = "";
        if (el && typeof el.getAttribute === 'function') {
            initial = el.getAttribute("style") || "";
        } else if (nodeId !== undefined && nodeId !== null && typeof _getAttribute === 'function') {
            initial = _getAttribute(nodeId, "style") || "";
        }
        var initialDecls = String(initial).split(';');
        for (var initialIndex = 0; initialIndex < initialDecls.length; initialIndex++) {
            var initialDecl = initialDecls[initialIndex].trim();
            if (!initialDecl) continue;
            var initialColon = initialDecl.indexOf(':');
            if (initialColon === -1) continue;
            var initialProp = initialDecl.slice(0, initialColon).trim().toLowerCase();
            var initialValue = initialDecl.slice(initialColon + 1).trim();
            if (initialProp && initialValue) base._props[initialProp] = initialValue;
        }

        function syncToDom() {
            if (base._syncing) return;
            base._syncing = true;
            try {
                var str = "";
                for (var k in base._props) {
                    if (base._props.hasOwnProperty(k) && base._props[k] !== undefined && base._props[k] !== null && base._props[k] !== "") {
                        str += k + ": " + base._props[k] + "; ";
                    }
                }
                var trimmed = str.trim();
                if (nodeId !== undefined && nodeId !== null && typeof _setAttribute === 'function') {
                    _setAttribute(nodeId, "style", trimmed);
                }
                if (el && typeof el.setAttribute === 'function') {
                    el.setAttribute("style", trimmed);
                }
            } finally {
                base._syncing = false;
            }
        }

        var WEBKIT_ALIASES = {
            '-webkit-align-content': 'align-content',
            '-webkit-align-items': 'align-items',
            '-webkit-align-self': 'align-self',
            '-webkit-flex': 'flex',
            '-webkit-flex-basis': 'flex-basis',
            '-webkit-flex-direction': 'flex-direction',
            '-webkit-flex-flow': 'flex-flow',
            '-webkit-flex-grow': 'flex-grow',
            '-webkit-flex-shrink': 'flex-shrink',
            '-webkit-flex-wrap': 'flex-wrap',
            '-webkit-justify-content': 'justify-content',
            '-webkit-order': 'order'
        };

        base.getPropertyValue = function(p) {
            if (!p) return "";
            var kebab = String(p).trim().replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
            kebab = WEBKIT_ALIASES[kebab] || kebab;
            if (kebab === 'all') {
                return base._all || (base._props && base._props['all']) || "";
            }
            if (kebab === 'flex') {
                if (!base._props) return "";
                var grow = base._props['flex-grow'];
                var shrink = base._props['flex-shrink'];
                var basis = base._props['flex-basis'];
                if (grow !== undefined && shrink !== undefined && basis !== undefined) {
                    return grow + " " + shrink + " " + basis;
                }
                return base._props['flex'] || "";
            }
            if (kebab === 'margin') {
                if (base._props && base._props['margin-top'] !== undefined
                    && base._props['margin-right'] !== undefined
                    && base._props['margin-bottom'] !== undefined
                    && base._props['margin-left'] !== undefined) {
                    var t = base._props['margin-top'];
                    var r = base._props['margin-right'];
                    var b = base._props['margin-bottom'];
                    var l = base._props['margin-left'];
                    if (t === r && t === b && t === l) return t;
                    if (t === b && r === l) return t + " " + r;
                    if (r === l) return t + " " + r + " " + b;
                    return t + " " + r + " " + b + " " + l;
                }
                return "";
            }
            if (kebab === 'padding') {
                if (base._props && base._props['padding-top'] !== undefined
                    && base._props['padding-right'] !== undefined
                    && base._props['padding-bottom'] !== undefined
                    && base._props['padding-left'] !== undefined) {
                    var t = base._props['padding-top'];
                    var r = base._props['padding-right'];
                    var b = base._props['padding-bottom'];
                    var l = base._props['padding-left'];
                    if (t === r && t === b && t === l) return t;
                    if (t === b && r === l) return t + " " + r;
                    if (r === l) return t + " " + r + " " + b;
                    return t + " " + r + " " + b + " " + l;
                }
                return "";
            }
            if (kebab === 'overflow') {
                if (base._props) {
                    var ox = base._props['overflow-x'];
                    var oy = base._props['overflow-y'];
                    if (ox !== undefined && oy !== undefined) {
                        return ox === oy ? ox : ox + " " + oy;
                    }
                    if (ox !== undefined) return ox;
                    if (oy !== undefined) return oy;
                    return base._props['overflow'] || "";
                }
                return "";
            }
            if (kebab === 'flex-flow') {
                if (!base._props || (base._props['flex-direction'] === undefined && base._props['flex-wrap'] === undefined)) {
                    return base._props ? (base._props['flex-flow'] || "") : "";
                }
                var dir = base._props['flex-direction'] || 'row';
                var wrap = base._props['flex-wrap'] || 'nowrap';
                if (dir === 'row' && wrap === 'nowrap') return 'row';
                if (dir === 'row') return wrap;
                if (wrap === 'nowrap') return dir;
                return dir + " " + wrap;
            }
            if (kebab === 'margin-trim') {
                var mt = base._props && base._props['margin-trim'];
                if (mt === 'block-start block-end' || mt === 'block-end block-start') return 'block';
                return mt || "";
            }
            if (kebab === 'inset-block') {
                var s = base._props && base._props['inset-block-start'];
                var e = base._props && base._props['inset-block-end'];
                if (s !== undefined && e !== undefined) {
                    if (s === e) return s;
                    return s + " " + e;
                }
                return "";
            }
            if (kebab === 'inset-inline') {
                var s = base._props && base._props['inset-inline-start'];
                var e = base._props && base._props['inset-inline-end'];
                if (s !== undefined && e !== undefined) {
                    if (s === e) return s;
                    return s + " " + e;
                }
                return "";
            }
            if (kebab === 'inset') {
                var t = base._props && (base._props['top'] || base._props['inset-block-start']);
                var r = base._props && (base._props['right'] || base._props['inset-inline-end']);
                var b = base._props && (base._props['bottom'] || base._props['inset-block-end']);
                var l = base._props && (base._props['left'] || base._props['inset-inline-start']);
                if (t !== undefined && r !== undefined && b !== undefined && l !== undefined) {
                    if (t === r && t === b && t === l) return t;
                    if (t === b && r === l) return t + " " + r;
                    if (r === l) return t + " " + r + " " + b;
                    return t + " " + r + " " + b + " " + l;
                }
                return "";
            }
            if (kebab === 'text-decoration') {
                if (!base._props) return "";
                var line = base._props['text-decoration-line'];
                var thick = base._props['text-decoration-thickness'];
                var st = base._props['text-decoration-style'];
                var clr = base._props['text-decoration-color'];
                if (line === undefined && thick === undefined && st === undefined && clr === undefined) {
                    return base._props['text-decoration'] || "";
                }
                var isLineInit = !line || line === 'none';
                var isThickInit = !thick || thick === 'auto';
                var isStInit = !st || st === 'solid';
                var isClrInit = !clr || clr === 'currentcolor';
                if (isLineInit && isThickInit && isStInit && isClrInit) {
                    return "none";
                }
                var parts = [];
                if (!isLineInit) parts.push(line);
                if (!isThickInit) parts.push(thick);
                if (!isStInit) parts.push(st);
                if (!isClrInit) parts.push(clr);
                return parts.length > 0 ? parts.join(" ") : "none";
            }
            if (base._props && base._props[kebab] !== undefined) return String(base._props[kebab]);
            var camel = kebab.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
            if (base[camel] !== undefined && typeof base[camel] !== 'function') return String(base[camel]);
            if (base[kebab] !== undefined && typeof base[kebab] !== 'function') return String(base[kebab]);
            return "";
        };

        base.getPropertyPriority = function(p) {
            return "";
        };

        base.setProperty = function(p, v, priority) {
            if (!p) return;
            var kebab = String(p).trim().replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
            kebab = WEBKIT_ALIASES[kebab] || kebab;
            var camel = kebab.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
            var sVal = (v === null || v === undefined) ? "" : String(v).trim();
            if (!base._props) base._props = {};

            if (sVal === "") {
                if (kebab === 'all') {
                    base._all = "";
                } else if (kebab === 'flex') {
                    delete base._props['flex-grow'];
                    delete base._props['flex-shrink'];
                    delete base._props['flex-basis'];
                    delete base.flexGrow; delete base.flexShrink; delete base.flexBasis;
                    delete base['flex-grow']; delete base['flex-shrink']; delete base['flex-basis'];
                } else if (kebab === 'flex-flow') {
                    delete base._props['flex-direction'];
                    delete base._props['flex-wrap'];
                    delete base.flexDirection; delete base.flexWrap;
                    delete base['flex-direction']; delete base['flex-wrap'];
                } else if (kebab === 'margin') {
                    delete base._props['margin-top'];
                    delete base._props['margin-right'];
                    delete base._props['margin-bottom'];
                    delete base._props['margin-left'];
                    delete base.marginTop; delete base.marginRight; delete base.marginBottom; delete base.marginLeft;
                    delete base['margin-top']; delete base['margin-right']; delete base['margin-bottom']; delete base['margin-left'];
                } else if (kebab === 'padding') {
                    delete base._props['padding-top'];
                    delete base._props['padding-right'];
                    delete base._props['padding-bottom'];
                    delete base._props['padding-left'];
                    delete base.paddingTop; delete base.paddingRight; delete base.paddingBottom; delete base.paddingLeft;
                    delete base['padding-top']; delete base['padding-right']; delete base['padding-bottom']; delete base['padding-left'];
                } else if (kebab === 'overflow') {
                    delete base._props['overflow-x'];
                    delete base._props['overflow-y'];
                    delete base.overflowX; delete base.overflowY;
                    delete base['overflow-x']; delete base['overflow-y'];
                } else if (kebab === 'inset-block') {
                    delete base._props['inset-block-start'];
                    delete base._props['inset-block-end'];
                    delete base.insetBlockStart; delete base.insetBlockEnd;
                    delete base['inset-block-start']; delete base['inset-block-end'];
                } else if (kebab === 'inset-inline') {
                    delete base._props['inset-inline-start'];
                    delete base._props['inset-inline-end'];
                    delete base.insetInlineStart; delete base.insetInlineEnd;
                    delete base['inset-inline-start']; delete base['inset-inline-end'];
                } else if (kebab === 'inset') {
                    delete base._props['top']; delete base._props['right']; delete base._props['bottom']; delete base._props['left'];
                    delete base._props['inset-block-start']; delete base._props['inset-block-end'];
                    delete base._props['inset-inline-start']; delete base._props['inset-inline-end'];
                } else if (kebab === 'text-decoration') {
                    delete base._props['text-decoration-line'];
                    delete base._props['text-decoration-thickness'];
                    delete base._props['text-decoration-style'];
                    delete base._props['text-decoration-color'];
                    delete base.textDecorationLine; delete base.textDecorationThickness;
                    delete base.textDecorationStyle; delete base.textDecorationColor;
                    delete base['text-decoration-line']; delete base['text-decoration-thickness'];
                    delete base['text-decoration-style']; delete base['text-decoration-color'];
                }
                delete base._props[kebab];
                delete base[camel];
                delete base[kebab];
                syncToDom();
                return;
            }

            if (sVal === 'inherit' || sVal === 'initial' || sVal === 'unset' || sVal.startsWith('var(')) {
                if (kebab === 'all') {
                    base._all = sVal;
                }
                if (kebab === 'flex') {
                    delete base._props['flex-grow'];
                    delete base._props['flex-shrink'];
                    delete base._props['flex-basis'];
                }
                base._props[kebab] = sVal;
                base[camel] = sVal;
                base[kebab] = sVal;
                syncToDom();
                return;
            }

            if (typeof _parseCssDeclaration === 'function') {
                var decls = _parseCssDeclaration(kebab, sVal);
                if (!decls || decls.length === 0) {
                    // Invalid CSS declaration under W3C grammar: ignore silently
                    return;
                }
                if (kebab === 'all') {
                    base._all = sVal;
                }
                if (kebab === 'flex' || kebab === 'margin' || kebab === 'padding' || kebab === 'flex-flow' || kebab === 'inset' || kebab === 'inset-block' || kebab === 'inset-inline' || kebab === 'overflow' || kebab === 'text-decoration') {
                    delete base._props[kebab];
                    delete base[camel];
                    delete base[kebab];
                }
                for (var i = 0; i < decls.length; i++) {
                    var dName = decls[i][0];
                    var dVal = decls[i][1];
                    base._props[dName] = dVal;
                    var dCamel = dName.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                    base[dCamel] = dVal;
                    base[dName] = dVal;
                }
            } else {
                base._props[kebab] = sVal;
                base[camel] = sVal;
                base[kebab] = sVal;
            }
            syncToDom();
        };

        base.removeProperty = function(p) {
            if (!p) return "";
            var kebab = String(p).trim().replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
            kebab = WEBKIT_ALIASES[kebab] || kebab;
            var prev = base.getPropertyValue(kebab);
            base.setProperty(kebab, "");
            return prev;
        };

        base.item = function(idx) {
            var keys = base._props ? Object.keys(base._props) : [];
            var i = Number(idx);
            return (i >= 0 && i < keys.length) ? keys[i] : "";
        };

        Object.defineProperty(base, 'cssText', {
            get: function() {
                var str = "";
                if (base._props) {
                    for (var k in base._props) {
                        if (base._props.hasOwnProperty(k) && base._props[k] !== undefined && base._props[k] !== null && base._props[k] !== "") {
                            str += k + ": " + base._props[k] + "; ";
                        }
                    }
                }
                return str.trim();
            },
            set: function(val) {
                base._props = {};
                var s = (val === null || val === undefined) ? "" : String(val);
                if (s) {
                    var decls = s.split(';');
                    for (var i = 0; i < decls.length; i++) {
                        var d = decls[i].trim();
                        if (!d) continue;
                        var colon = d.indexOf(':');
                        if (colon !== -1) {
                            var k = d.slice(0, colon).trim().toLowerCase();
                            var v = d.slice(colon + 1).trim();
                            base._props[k] = v;
                            var camel = k.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                            base[camel] = v;
                            base[k] = v;
                        }
                    }
                }
                syncToDom();
            },
            configurable: true,
            enumerable: true
        });

        Object.defineProperty(base, 'length', {
            get: function() {
                return base._props ? Object.keys(base._props).length : 0;
            },
            configurable: true,
            enumerable: false
        });

        var rawStyle = null;
        if (el && typeof el.getAttribute === 'function') {
            rawStyle = el.getAttribute("style");
        }
        if ((rawStyle === null || rawStyle === undefined)
            && nodeId !== undefined && typeof _getAttribute === 'function') {
            rawStyle = _getAttribute(nodeId, "style");
        }
        if (rawStyle) {
            var decls = rawStyle.split(';');
            for (var i = 0; i < decls.length; i++) {
                var d = decls[i].trim();
                if (!d) continue;
                var colon = d.indexOf(':');
                if (colon !== -1) {
                    var prop = d.slice(0, colon).trim().toLowerCase();
                    var val = d.slice(colon + 1).trim();
                    base._props[prop] = val;
                    var camel = prop.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                    base[camel] = val;
                    base[prop] = val;
                }
            }
        }

        return new Proxy(base, {
            get: function(target, prop, receiver) {
                if (typeof prop === 'string') {
                    if (prop in target && typeof target[prop] === 'function') return target[prop];
                    var kebab = prop.replace(/([A-Z])/g, '-$1').toLowerCase();
                    var val = target.getPropertyValue(kebab);
                    if (val !== "") return val;
                    if (target._props && target._props[prop] !== undefined) return target._props[prop];
                    if (prop in target) return target[prop];
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var keys = target._props ? Object.keys(target._props) : [];
                        return keys[idx] || "";
                    }
                }
                if (prop === Symbol.iterator) {
                    return function*() {
                        var keys = target._props ? Object.keys(target._props) : [];
                        for (var i = 0; i < keys.length; i++) {
                            yield keys[i];
                        }
                    };
                }
                return target[prop];
            },
            set: function(target, prop, value, receiver) {
                if (prop === 'cssText') {
                    target.cssText = value;
                    return true;
                }
                if (typeof prop === 'string') {
                    if (prop.startsWith('_') || typeof target[prop] === 'function') {
                        target[prop] = value;
                        return true;
                    }
                    target.setProperty(prop, value);
                    return true;
                }
                target[prop] = value;
                return true;
            }
        });
    }

    function _createClassList(nodeId, el) {
        var proto = (typeof globalThis.DOMTokenList !== 'undefined') ? globalThis.DOMTokenList.prototype : Object.prototype;
        var base = Object.create(proto);

        function getClassStr() {
            if (nodeId !== undefined && nodeId !== null) {
                var s = _getAttribute(nodeId, "class");
                if (s !== null && s !== undefined) return s;
            }
            if (el && typeof el.getAttribute === 'function') {
                var s = el.getAttribute("class");
                if (s !== null && s !== undefined) return s;
            }
            return (el && el._className) || "";
        }
        function setClassStr(val) {
            if (nodeId !== undefined && nodeId !== null) {
                _setAttribute(nodeId, "class", val);
            }
            if (el && typeof el.setAttribute === 'function') {
                el.setAttribute("class", val);
            }
            if (el) el._className = val;
        }
        function getTokens() {
            var cur = getClassStr();
            return cur ? cur.split(/\s+/).filter(Boolean) : [];
        }
        function setTokens(tokens) {
            setClassStr(tokens.join(" "));
        }

        base.item = function(index) {
            var tokens = getTokens();
            var i = Number(index);
            if (isNaN(i) || i < 0 || i >= tokens.length) return null;
            return tokens[i];
        };
        base.contains = function(cls) {
            var tokens = getTokens();
            return tokens.indexOf(String(cls)) !== -1;
        };
        base.add = function() {
            var tokens = getTokens();
            var changed = false;
            for (var i = 0; i < arguments.length; i++) {
                var token = String(arguments[i]);
                if (token && tokens.indexOf(token) === -1) {
                    tokens.push(token);
                    changed = true;
                }
            }
            if (changed) setTokens(tokens);
        };
        base.remove = function() {
            var tokens = getTokens();
            var changed = false;
            for (var i = 0; i < arguments.length; i++) {
                var token = String(arguments[i]);
                var idx = tokens.indexOf(token);
                if (idx !== -1) {
                    tokens.splice(idx, 1);
                    changed = true;
                }
            }
            if (changed) setTokens(tokens);
        };
        base.toggle = function(cls, force) {
            cls = String(cls);
            var tokens = getTokens();
            var exists = tokens.indexOf(cls) !== -1;
            if (force === undefined) {
                if (exists) {
                    this.remove(cls);
                    return false;
                } else {
                    this.add(cls);
                    return true;
                }
            } else if (force) {
                if (!exists) this.add(cls);
                return true;
            } else {
                if (exists) this.remove(cls);
                return false;
            }
        };
        base.replace = function(oldToken, newToken) {
            var tokens = getTokens();
            var idx = tokens.indexOf(String(oldToken));
            if (idx === -1) return false;
            tokens[idx] = String(newToken);
            setTokens(tokens);
            return true;
        };
        base.supports = function() { return true; };
        base.toString = function() { return getClassStr(); };
        base.forEach = function(callback, thisArg) {
            var tokens = getTokens();
            for (var i = 0; i < tokens.length; i++) {
                callback.call(thisArg, tokens[i], i, this);
            }
        };
        base.entries = function() {
            var tokens = getTokens();
            var i = 0;
            return {
                next: function() {
                    if (i < tokens.length) {
                        return { value: [i, tokens[i++]], done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base.keys = function() {
            var tokens = getTokens();
            var i = 0;
            return {
                next: function() {
                    if (i < tokens.length) {
                        return { value: i++, done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base.values = function() {
            var tokens = getTokens();
            var i = 0;
            return {
                next: function() {
                    if (i < tokens.length) {
                        return { value: tokens[i++], done: false };
                    }
                    return { value: undefined, done: true };
                },
                [Symbol.iterator]: function() { return this; }
            };
        };
        base[Symbol.iterator] = function() {
            return this.values();
        };
        base[Symbol.toStringTag] = 'DOMTokenList';

        return new Proxy(base, {
            get: function(target, prop, receiver) {
                if (prop === 'length') {
                    return getTokens().length;
                }
                if (prop === 'value') {
                    return getClassStr();
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var tokens = getTokens();
                        return tokens[idx];
                    }
                }
                if (prop in target) {
                    return target[prop];
                }
                return undefined;
            },
            set: function(target, prop, value) {
                if (prop === 'value') {
                    setClassStr(String(value));
                    return true;
                }
                target[prop] = value;
                return true;
            },
            has: function(target, prop) {
                if (prop === 'length' || prop === 'value') return true;
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return idx < getTokens().length;
                    }
                }
                return prop in target;
            },
            ownKeys: function(target) {
                var keys = Reflect.ownKeys(target);
                var tokens = getTokens();
                for (var i = 0; i < tokens.length; i++) {
                    var s = String(i);
                    if (keys.indexOf(s) === -1) keys.push(s);
                }
                if (keys.indexOf('length') === -1) keys.push('length');
                if (keys.indexOf('value') === -1) keys.push('value');
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                if (prop === 'length') {
                    return { value: getTokens().length, writable: false, enumerable: false, configurable: true };
                }
                if (prop === 'value') {
                    return { value: getClassStr(), writable: true, enumerable: true, configurable: true };
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var tokens = getTokens();
                        if (idx < tokens.length) {
                            return { value: tokens[idx], writable: false, enumerable: true, configurable: true };
                        }
                    }
                }
                return Object.getOwnPropertyDescriptor(target, prop);
            }
        });
    }
