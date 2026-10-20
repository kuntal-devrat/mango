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
                var keys = Reflect.ownKeys(target);
                var items = getItemsFn() || [];
                for (var i = 0; i < items.length; i++) {
                    var s = String(i);
                    if (keys.indexOf(s) === -1) keys.push(s);
                }
                if (keys.indexOf('length') === -1) keys.push('length');
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
                var keys = Reflect.ownKeys(target);
                var items = getItemsFn() || [];
                for (var i = 0; i < items.length; i++) {
                    var s = String(i);
                    if (keys.indexOf(s) === -1) keys.push(s);
                }
                if (keys.indexOf('length') === -1) keys.push('length');
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

