    // ── 14. IndexedDB API (W3C Indexed Database API) ──────────────────────────
    function DOMStringList(arr) {
        this._list = arr || [];
        for (var i = 0; i < this._list.length; i++) {
            this[i] = this._list[i];
        }
    }
    DOMStringList.prototype.contains = function(str) {
        return this._list.indexOf(str) !== -1;
    };
    DOMStringList.prototype.item = function(index) {
        return this._list[index] || null;
    };
    Object.defineProperty(DOMStringList.prototype, 'length', {
        get: function() { return this._list.length; }
    });
    globalThis.DOMStringList = DOMStringList;

    function IDBRequest() {
        this.result = undefined;
        this.error = null;
        this.source = null;
        this.transaction = null;
        this.readyState = 'pending';
        this._onsuccess = null;
        this._onerror = null;
        this._listeners = {};
        this._successDispatched = false;
        this._errorDispatched = false;
    }
    Object.defineProperty(IDBRequest.prototype, 'onsuccess', {
        get: function() { return this._onsuccess; },
        set: function(fn) {
            this._onsuccess = fn;
            if (typeof fn === 'function' && this.readyState === 'done' && !this.error) {
                if (this._needsUpgrade && !this._upgradeDispatched) {
                    if (typeof this._onupgradeneeded === 'function') {
                        this._upgradeDispatched = true;
                        try { this._onupgradeneeded.call(this, this._upgradeEvent); } catch (e) {}
                    }
                }
                if (!this._successDispatched) {
                    this._successDispatched = true;
                    try { fn.call(this, { type: 'success', target: this }); } catch (e) {}
                }
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(IDBRequest.prototype, 'onerror', {
        get: function() { return this._onerror; },
        set: function(fn) {
            this._onerror = fn;
            if (typeof fn === 'function' && this.readyState === 'done' && this.error) {
                if (!this._errorDispatched) {
                    this._errorDispatched = true;
                    try { fn.call(this, { type: 'error', target: this }); } catch (e) {}
                }
            }
        },
        configurable: true,
        enumerable: true
    });
    IDBRequest.prototype.addEventListener = function(type, fn) {
        if (!this._listeners[type]) this._listeners[type] = [];
        this._listeners[type].push(fn);
        if (type === 'upgradeneeded' && this._needsUpgrade && !this._upgradeDispatched) {
            this._upgradeDispatched = true;
            try { fn.call(this, this._upgradeEvent); } catch (e) {}
            if (typeof this._onsuccess === 'function' && !this._successDispatched) {
                this._successDispatched = true;
                try { this._onsuccess.call(this, { type: 'success', target: this }); } catch (e) {}
            }
        } else if (type === 'success' && this.readyState === 'done' && !this.error) {
            if (!this._needsUpgrade || this._upgradeDispatched) {
                try { fn.call(this, { type: 'success', target: this }); } catch (e) {}
            }
        } else if (type === 'error' && this.readyState === 'done' && this.error) {
            try { fn.call(this, { type: 'error', target: this }); } catch (e) {}
        }
    };
    IDBRequest.prototype.removeEventListener = function(type, fn) {
        if (this._listeners[type]) {
            this._listeners[type] = this._listeners[type].filter(function(f) { return f !== fn; });
        }
    };
    IDBRequest.prototype.dispatchEvent = function(evt) {
        var t = evt ? evt.type : 'success';
        if (t === 'success' && typeof this._onsuccess === 'function') {
            try { this._onsuccess.call(this, evt); } catch (e) {}
        } else if (t === 'error' && typeof this._onerror === 'function') {
            try { this._onerror.call(this, evt); } catch (e) {}
        } else if (t === 'upgradeneeded' && typeof this._onupgradeneeded === 'function') {
            try { this._onupgradeneeded.call(this, evt); } catch (e) {}
        }
        if (this._listeners[t]) {
            for (var i = 0; i < this._listeners[t].length; i++) {
                try { this._listeners[t][i].call(this, evt); } catch (e) {}
            }
        }
        return true;
    };
    globalThis.IDBRequest = IDBRequest;

    function IDBOpenDBRequest() {
        IDBRequest.call(this);
        this._onupgradeneeded = null;
        this._onblocked = null;
        this._needsUpgrade = false;
        this._upgradeDispatched = false;
        this._upgradeEvent = null;
    }
    IDBOpenDBRequest.prototype = Object.create(IDBRequest.prototype);
    IDBOpenDBRequest.prototype.constructor = IDBOpenDBRequest;

    Object.defineProperty(IDBOpenDBRequest.prototype, 'onupgradeneeded', {
        get: function() { return this._onupgradeneeded; },
        set: function(fn) {
            this._onupgradeneeded = fn;
            if (typeof fn === 'function' && this._needsUpgrade && !this._upgradeDispatched) {
                this._upgradeDispatched = true;
                try { fn.call(this, this._upgradeEvent); } catch (e) {}
                if (typeof this._onsuccess === 'function' && !this._successDispatched) {
                    this._successDispatched = true;
                    try { this._onsuccess.call(this, { type: 'success', target: this }); } catch (e) {}
                }
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(IDBOpenDBRequest.prototype, 'onblocked', {
        get: function() { return this._onblocked; },
        set: function(fn) { this._onblocked = fn; },
        configurable: true,
        enumerable: true
    });
    globalThis.IDBOpenDBRequest = IDBOpenDBRequest;

    function IDBVersionChangeEvent(type, dict) {
        this.type = type;
        this.oldVersion = (dict && dict.oldVersion !== undefined) ? dict.oldVersion : 0;
        this.newVersion = (dict && dict.newVersion !== undefined) ? dict.newVersion : null;
        this.target = null;
    }
    globalThis.IDBVersionChangeEvent = IDBVersionChangeEvent;

    function _extractIdbKey(obj, keyPath) {
        if (!obj || typeof obj !== 'object' || keyPath === null || keyPath === undefined) return undefined;
        if (Array.isArray(keyPath)) {
            var arr = [];
            for (var i = 0; i < keyPath.length; i++) {
                arr.push(obj[keyPath[i]]);
            }
            return arr;
        }
        return obj[keyPath];
    }

    function _idbKeyType(k) {
        if (typeof k === 'number') return 1;
        if (k instanceof Date) return 2;
        if (typeof k === 'string') return 3;
        if (Array.isArray(k)) return 4;
        return 0;
    }

    function IDBKeyRange(lower, upper, lowerOpen, upperOpen) {
        this.lower = lower;
        this.upper = upper;
        this.lowerOpen = !!lowerOpen;
        this.upperOpen = !!upperOpen;
    }
    IDBKeyRange.only = function(val) {
        return new IDBKeyRange(val, val, false, false);
    };
    IDBKeyRange.lowerBound = function(lower, open) {
        return new IDBKeyRange(lower, undefined, open, true);
    };
    IDBKeyRange.upperBound = function(upper, open) {
        return new IDBKeyRange(undefined, upper, true, open);
    };
    IDBKeyRange.bound = function(lower, upper, lowerOpen, upperOpen) {
        return new IDBKeyRange(lower, upper, lowerOpen, upperOpen);
    };
    IDBKeyRange.prototype.includes = function(key) {
        if (this.lower !== undefined) {
            var cLow = IDBFactory.prototype.cmp(key, this.lower);
            if (this.lowerOpen ? cLow <= 0 : cLow < 0) return false;
        }
        if (this.upper !== undefined) {
            var cUp = IDBFactory.prototype.cmp(key, this.upper);
            if (this.upperOpen ? cUp >= 0 : cUp > 0) return false;
        }
        return true;
    };
    globalThis.IDBKeyRange = IDBKeyRange;

    function IDBTransaction(db, storeNames, mode) {
        this.db = db;
        this.mode = mode || 'readonly';
        this.error = null;
        this._oncomplete = null;
        this._onerror = null;
        this._onabort = null;
        this._storeNames = Array.isArray(storeNames) ? storeNames : [storeNames];
        this._active = true;
        this._listeners = {};
        this._completeDispatched = false;
        try {
            this._snapshot = JSON.stringify(db._data);
        } catch(e) {
            this._snapshot = null;
        }
    }
    Object.defineProperty(IDBTransaction.prototype, 'oncomplete', {
        get: function() { return this._oncomplete; },
        set: function(fn) {
            this._oncomplete = fn;
            if (typeof fn === 'function' && !this._completeDispatched) {
                this._completeDispatched = true;
                try { fn.call(this, { type: 'complete', target: this }); } catch (e) {}
            }
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(IDBTransaction.prototype, 'onerror', {
        get: function() { return this._onerror; },
        set: function(fn) { this._onerror = fn; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(IDBTransaction.prototype, 'onabort', {
        get: function() { return this._onabort; },
        set: function(fn) { this._onabort = fn; },
        configurable: true,
        enumerable: true
    });
    IDBTransaction.prototype.objectStore = function(name) {
        if (!this.db._data.stores[name]) {
            throw new Error("NotFoundError: The specified object store was not found: " + name);
        }
        return new IDBObjectStore(name, this.db, this);
    };
    IDBTransaction.prototype.abort = function() {
        if (!this._active) return;
        this._active = false;
        if (this._snapshot) {
            try {
                this.db._data = JSON.parse(this._snapshot);
                this.db._persist();
            } catch(e) {}
        }
        var abortEvt = { type: 'abort', target: this };
        if (typeof this._onabort === 'function') this._onabort(abortEvt);
        this.dispatchEvent(abortEvt);
    };
    IDBTransaction.prototype.commit = function() {
        this._active = false;
        var completeEvt = { type: 'complete', target: this };
        if (typeof this._oncomplete === 'function') this._oncomplete(completeEvt);
        this.dispatchEvent(completeEvt);
    };
    IDBTransaction.prototype.addEventListener = IDBRequest.prototype.addEventListener;
    IDBTransaction.prototype.removeEventListener = IDBRequest.prototype.removeEventListener;
    IDBTransaction.prototype.dispatchEvent = IDBRequest.prototype.dispatchEvent;
    globalThis.IDBTransaction = IDBTransaction;

    function IDBObjectStore(name, db, tx) {
        this.name = name;
        this.transaction = tx;
        this._db = db;
        var sData = db._data.stores[name] || { keyPath: null, autoIncrement: false, indexes: {}, records: [], nextId: 1 };
        this.keyPath = sData.keyPath;
        this.autoIncrement = sData.autoIncrement;
        this.indexNames = new DOMStringList(Object.keys(sData.indexes || {}));
    }
    IDBObjectStore.prototype._save = function() {
        this._db._persist();
    };
    IDBObjectStore.prototype._findRecordIndex = function(key) {
        var sData = this._db._data.stores[this.name];
        if (!sData) return -1;
        for (var i = 0; i < sData.records.length; i++) {
            try {
                if (IDBFactory.prototype.cmp(sData.records[i].key, key) === 0) return i;
            } catch(e) {
                if (sData.records[i].key === key) return i;
            }
        }
        return -1;
    };
    IDBObjectStore.prototype.get = function(key) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var idx = this._findRecordIndex(key);
        var res = idx !== -1 ? this._db._data.stores[this.name].records[idx].value : undefined;
        req.result = res;
        req.readyState = 'done';
        return req;
    };
    IDBObjectStore.prototype.getAll = function(query, count) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var all = [];
        if (sData) {
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {
                var rec = sData.records[i];
                var isMatch = true;
                if (query !== undefined && query !== null) {
                    if (query instanceof IDBKeyRange) {
                        isMatch = query.includes(rec.key);
                    } else if (query.includes) {
                        isMatch = query.includes(rec.key);
                    } else {
                        try { isMatch = (IDBFactory.prototype.cmp(rec.key, query) === 0); } catch(e) { isMatch = (rec.key === query); }
                    }
                }
                if (isMatch) {
                    all.push(rec.value);
                }
            }
        }
        req.result = all;
        req.readyState = 'done';
        return req;
    };
    IDBObjectStore.prototype.getAllKeys = function(query, count) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var all = [];
        if (sData) {
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {
                var rec = sData.records[i];
                var isMatch = true;
                if (query !== undefined && query !== null) {
                    if (query instanceof IDBKeyRange) {
                        isMatch = query.includes(rec.key);
                    } else if (query.includes) {
                        isMatch = query.includes(rec.key);
                    } else {
                        try { isMatch = (IDBFactory.prototype.cmp(rec.key, query) === 0); } catch(e) { isMatch = (rec.key === query); }
                    }
                }
                if (isMatch) {
                    all.push(rec.key);
                }
            }
        }
        req.result = all;
        req.readyState = 'done';
        return req;
    };
    IDBObjectStore.prototype.put = function(value, key) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var recKey = key;
        if (recKey === undefined && this.keyPath) {
            recKey = _extractIdbKey(value, this.keyPath);
        }
        if (recKey === undefined && this.autoIncrement) {
            recKey = sData.nextId++;
            if (this.keyPath && value && typeof value === 'object' && !Array.isArray(this.keyPath)) {
                value[this.keyPath] = recKey;
            }
        }
        if (recKey === undefined) {
            req.error = new Error("DataError: No key provided");
            req.readyState = 'done';
            if (typeof req._onerror === 'function') req._onerror.call(req, { type: 'error', target: req });
            return req;
        }
        // Check unique index constraints
        if (sData.indexes) {
            for (var idxName in sData.indexes) {
                var idxDef = sData.indexes[idxName];
                if (idxDef.unique) {
                    var valToCheck = _extractIdbKey(value, idxDef.keyPath);
                    if (valToCheck !== undefined) {
                        for (var r = 0; r < sData.records.length; r++) {
                            if (sData.records[r].key !== recKey) {
                                var existingVal = _extractIdbKey(sData.records[r].value, idxDef.keyPath);
                                try {
                                    if (IDBFactory.prototype.cmp(existingVal, valToCheck) === 0) {
                                        req.error = new Error("ConstraintError: Unique index constraint violated for index " + idxName);
                                        req.readyState = 'done';
                                        if (typeof req._onerror === 'function') req._onerror.call(req, { type: 'error', target: req });
                                        return req;
                                    }
                                } catch(e) {}
                            }
                        }
                    }
                }
            }
        }
        var idx = this._findRecordIndex(recKey);
        if (idx !== -1) {
            sData.records[idx].value = value;
        } else {
            sData.records.push({ key: recKey, value: value });
        }
        this._save();
        req.result = recKey;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, { type: 'success', target: req });
        return req;
    };
    IDBObjectStore.prototype.add = function(value, key) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var recKey = key;
        if (recKey === undefined && this.keyPath) {
            recKey = _extractIdbKey(value, this.keyPath);
        }
        if (recKey === undefined && this.autoIncrement) {
            recKey = sData.nextId++;
            if (this.keyPath && value && typeof value === 'object' && !Array.isArray(this.keyPath)) {
                value[this.keyPath] = recKey;
            }
        }
        if (recKey === undefined) {
            req.error = new Error("DataError: No key provided");
            req.readyState = 'done';
            if (typeof req._onerror === 'function') req._onerror.call(req, { type: 'error', target: req });
            return req;
        }
        var idx = this._findRecordIndex(recKey);
        if (idx !== -1) {
            req.error = new Error("ConstraintError: Key already exists in object store");
            req.readyState = 'done';
            if (typeof req._onerror === 'function') req._onerror.call(req, { type: 'error', target: req });
            return req;
        }
        // Check unique index constraints
        if (sData.indexes) {
            for (var idxName in sData.indexes) {
                var idxDef = sData.indexes[idxName];
                if (idxDef.unique) {
                    var valToCheck = _extractIdbKey(value, idxDef.keyPath);
                    if (valToCheck !== undefined) {
                        for (var r = 0; r < sData.records.length; r++) {
                            var existingVal = _extractIdbKey(sData.records[r].value, idxDef.keyPath);
                            try {
                                if (IDBFactory.prototype.cmp(existingVal, valToCheck) === 0) {
                                    req.error = new Error("ConstraintError: Unique index constraint violated for index " + idxName);
                                    req.readyState = 'done';
                                    if (typeof req._onerror === 'function') req._onerror.call(req, { type: 'error', target: req });
                                    return req;
                                }
                            } catch(e) {}
                        }
                    }
                }
            }
        }
        sData.records.push({ key: recKey, value: value });
        this._save();
        req.result = recKey;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, { type: 'success', target: req });
        return req;
    };
    IDBObjectStore.prototype.delete = function(key) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var idx = this._findRecordIndex(key);
        if (idx !== -1) {
            sData.records.splice(idx, 1);
            this._save();
        }
        req.result = undefined;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, { type: 'success', target: req });
        return req;
    };
    IDBObjectStore.prototype.clear = function() {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        if (sData) {
            sData.records = [];
            this._save();
        }
        req.result = undefined;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, { type: 'success', target: req });
        return req;
    };
    IDBObjectStore.prototype.count = function(query) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var c = 0;
        if (sData) {
            if (query === undefined || query === null) {
                c = sData.records.length;
            } else {
                for (var i = 0; i < sData.records.length; i++) {
                    var k = sData.records[i].key;
                    var isMatch = (query instanceof IDBKeyRange) ? query.includes(k) : (query.includes ? query.includes(k) : (k === query));
                    if (isMatch) c++;
                }
            }
        }
        req.result = c;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, { type: 'success', target: req });
        return req;
    };
