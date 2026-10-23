    function _getNamedMap(items) {
        var map = {};
        var names = [];
        for (var i = 0; i < items.length; i++) {
            var it = items[i];
            if (!it) continue;
            var id = (it.getAttribute && typeof it.getAttribute === 'function') ? it.getAttribute('id') : it.id;
            if (id && !map.hasOwnProperty(id)) {
                map[id] = it;
                names.push(id);
            }
            var itNs = it.namespaceURI !== undefined ? it.namespaceURI : 'http://www.w3.org/1999/xhtml';
            if (itNs === 'http://www.w3.org/1999/xhtml' && it.getAttribute && typeof it.getAttribute === 'function') {
                var nm = it.getAttribute('name');
                if (nm && !map.hasOwnProperty(nm)) {
                    map[nm] = it;
                    names.push(nm);
                }
            }
        }
        return { map: map, names: names };
    }

    function _createHTMLCollection(getItemsFn) {
        var proto = (typeof globalThis.HTMLCollection !== 'undefined') ? globalThis.HTMLCollection.prototype : Object.prototype;
        var base = Object.create(proto);
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
                var desc = Object.getOwnPropertyDescriptor(target, prop);
                if (desc) {
                    if (desc.get) return desc.get.call(receiver);
                    if ('value' in desc) return desc.value;
                }
                if (prop === 'length') {
                    return (getItemsFn() || []).length;
                }
                if (prop === Symbol.iterator) {
                    return function() {
                        var items = getItemsFn() || [];
                        return items[Symbol.iterator]();
                    };
                }
                var items = getItemsFn() || [];
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return items[idx];
                    }
                }
                if (prop in target) {
                    return target[prop];
                }
                if (typeof prop === 'string') {
                    var named = _getNamedMap(items);
                    if (named.map.hasOwnProperty(prop)) return named.map[prop];
                }
                return undefined;
            },
            set: function(target, prop, value, receiver) {
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return false;
                    }
                }
                target[prop] = value;
                return true;
            },
            has: function(target, prop) {
                var items = getItemsFn() || [];
                if (prop === 'length') return true;
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        return idx < items.length;
                    }
                    var named = _getNamedMap(items);
                    if (named.map.hasOwnProperty(prop)) return true;
                }
                return prop in target;
            },
            ownKeys: function(target) {
                var items = getItemsFn() || [];
                var keys = [];
                for (var i = 0; i < items.length; i++) {
                    keys.push(String(i));
                }
                var named = _getNamedMap(items);
                for (var n = 0; n < named.names.length; n++) {
                    if (keys.indexOf(named.names[n]) === -1) keys.push(named.names[n]);
                }
                var targetKeys = Object.getOwnPropertyNames(target);
                for (var t = 0; t < targetKeys.length; t++) {
                    if (keys.indexOf(targetKeys[t]) === -1) keys.push(targetKeys[t]);
                }
                return keys;
            },
            getOwnPropertyDescriptor: function(target, prop) {
                var own = Object.getOwnPropertyDescriptor(target, prop);
                if (own) return own;
                var items = getItemsFn() || [];
                if (prop === 'length') {
                    return { value: items.length, writable: false, enumerable: false, configurable: true };
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        if (idx < items.length) {
                            return { value: items[idx], writable: false, enumerable: true, configurable: true };
                        }
                        return undefined;
                    }
                    var named = _getNamedMap(items);
                    if (named.map.hasOwnProperty(prop)) {
                        return { value: named.map[prop], writable: false, enumerable: false, configurable: true };
                    }
                }
                return Object.getOwnPropertyDescriptor(target, prop);
            }
        });
    }

    function _createNodeList(getItemsFn) {
        var proto = (typeof globalThis.NodeList !== 'undefined') ? globalThis.NodeList.prototype : Object.prototype;
        var base = Object.create(proto);

        return new Proxy(base, {
            get: function(target, prop, receiver) {
                var desc = Object.getOwnPropertyDescriptor(target, prop);
                if (desc) {
                    if (desc.get) return desc.get.call(receiver);
                    if ('value' in desc) return desc.value;
                }
                if (prop === 'length') {
                    return (getItemsFn() || []).length;
                }
                if (prop === Symbol.iterator) {
                    return function() {
                        var items = getItemsFn() || [];
                        return items[Symbol.iterator]();
                    };
                }
                if (typeof prop === 'string') {
                    var idx = Number(prop);
                    if (Number.isInteger(idx) && idx >= 0 && String(idx) === prop) {
                        var items = getItemsFn() || [];
                        return items[idx];
                    }
                }
                if (prop === 'item') {
                    return function(index) {
                        var items = getItemsFn() || [];
                        var i = Number(index);
                        if (isNaN(i) || i < 0 || i >= items.length) return null;
                        return items[i];
                    };
                }
                if (prop === 'forEach') {
                    return function(callback, thisArg) {
                        var items = getItemsFn() || [];
                        for (var i = 0; i < items.length; i++) {
                            callback.call(thisArg, items[i], i, receiver);
                        }
                    };
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
                var own = Object.getOwnPropertyDescriptor(target, prop);
                if (own) return own;
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

