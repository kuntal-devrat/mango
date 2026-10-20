    function _createStyleObject(nodeId) {
        var obj = {};
        Object.defineProperty(obj, '_props', {
            value: {},
            writable: true,
            configurable: true,
            enumerable: false
        });
        Object.defineProperties(obj, {
            getPropertyValue: {
                value: function(p) {
                    if (!this._props) this._props = {};
                    if (!p) return "";
                    var val = this._props[p];
                    if (val !== undefined) return val;
                    var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                    if (this[camel] !== undefined && typeof this[camel] !== 'function') return String(this[camel]);
                    if (this[p] !== undefined && typeof this[p] !== 'function') return String(this[p]);
                    return "";
                },
                writable: true,
                configurable: true,
                enumerable: false
            },
            setProperty: {
                value: function(p, v) {
                    if (!this._props) this._props = {};
                    if (!p) return;
                    var sVal = String(v);
                    this._props[p] = sVal;
                    var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                    this[camel] = sVal;
                    this[p] = sVal;
                },
                writable: true,
                configurable: true,
                enumerable: false
            },
            removeProperty: {
                value: function(p) {
                    if (!this._props) this._props = {};
                    if (!p) return "";
                    var prev = this.getPropertyValue(p);
                    delete this._props[p];
                    var camel = p.replace(/-([a-z])/g, function(g) { return g[1].toUpperCase(); });
                    delete this[camel];
                    delete this[p];
                    return prev;
                },
                writable: true,
                configurable: true,
                enumerable: false
            }
        });
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

