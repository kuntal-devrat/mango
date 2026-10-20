    IDBObjectStore.prototype.createIndex = function(name, keyPath, options) {
        var sData = this._db._data.stores[this.name];
        sData.indexes[name] = {
            name: name,
            keyPath: keyPath,
            unique: !!(options && options.unique),
            multiEntry: !!(options && options.multiEntry)
        };
        this._save();
        this.indexNames = new DOMStringList(Object.keys(sData.indexes));
        return new IDBIndex(name, this);
    };
    IDBObjectStore.prototype.index = function(name) {
        var sData = this._db._data.stores[this.name];
        if (!sData.indexes[name]) {
            throw new Error("NotFoundError: Index not found: " + name);
        }
        return new IDBIndex(name, this);
    };
    IDBObjectStore.prototype.deleteIndex = function(name) {
        var sData = this._db._data.stores[this.name];
        delete sData.indexes[name];
        this._save();
        this.indexNames = new DOMStringList(Object.keys(sData.indexes));
    };
    IDBObjectStore.prototype.openCursor = function(query, direction) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var records = sData ? sData.records.slice() : [];
        try {
            records.sort(function(a, b) { return IDBFactory.prototype.cmp(a.key, b.key); });
        } catch(e) {}
        if (direction === 'prev' || direction === 'prevunique') {
            records.reverse();
        }
        var matched = [];
        for (var i = 0; i < records.length; i++) {
            var k = records[i].key;
            var isMatch = true;
            if (query !== undefined && query !== null) {
                if (query instanceof IDBKeyRange) {
                    isMatch = query.includes(k);
                } else if (query.includes) {
                    isMatch = query.includes(k);
                } else {
                    try { isMatch = (IDBFactory.prototype.cmp(k, query) === 0); } catch(e) { isMatch = (k === query); }
                }
            }
            if (isMatch) {
                matched.push(records[i]);
            }
        }
        if (matched.length > 0) {
            var cursor = new IDBCursorWithValue(this, direction || 'next', matched, 0, req);
            cursor.key = matched[0].key;
            cursor.primaryKey = matched[0].key;
            cursor.value = matched[0].value;
            req.result = cursor;
        } else {
            req.result = null;
        }
        req.readyState = 'done';
        return req;
    };
    IDBObjectStore.prototype.openKeyCursor = function(query, direction) {
        var req = this.openCursor(query, direction);
        if (req.result) {
            req.result.value = undefined;
        }
        return req;
    };
    globalThis.IDBObjectStore = IDBObjectStore;

    function IDBCursor(source, direction, records, index, req) {
        this.source = source;
        this.direction = direction || 'next';
        this._records = records;
        this._index = index;
        this._req = req;
        var cur = records[index];
        this.key = cur ? cur.key : undefined;
        this.primaryKey = cur ? (cur.primaryKey !== undefined ? cur.primaryKey : cur.key) : undefined;
    }
    IDBCursor.prototype.continue = function() {
        this._index++;
        if (this._index < this._records.length) {
            var cur = this._records[this._index];
            this.key = cur.key;
            this.primaryKey = cur.primaryKey !== undefined ? cur.primaryKey : cur.key;
            if (this.value !== undefined) this.value = cur.value;
            this._req.result = this;
        } else {
            this._req.result = null;
        }
        this._req._successDispatched = false;
        if (typeof this._req._onsuccess === 'function') {
            this._req._successDispatched = true;
            try { this._req._onsuccess.call(this._req, { type: 'success', target: this._req }); } catch (e) {}
        }
        this._req.dispatchEvent({ type: 'success', target: this._req });
    };
    IDBCursor.prototype.advance = function(count) {
        this._index += (count - 1);
        this.continue();
    };
    IDBCursor.prototype.update = function(val) {
        var req = new IDBRequest();
        req.source = this;
        var store = this.source.objectStore || this.source;
        var idx = store._findRecordIndex(this.primaryKey);
        if (idx !== -1) {
            var sData = store._db._data.stores[store.name];
            sData.records[idx].value = val;
            store._save();
            this.value = val;
            req.result = this.primaryKey;
        } else {
            req.error = new Error("DataError: Record not found to update");
        }
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function' && !req.error) {
            req._onsuccess.call(req, { type: 'success', target: req });
        } else if (typeof req._onerror === 'function' && req.error) {
            req._onerror.call(req, { type: 'error', target: req });
        }
        return req;
    };
    IDBCursor.prototype.delete = function() {
        var req = new IDBRequest();
        req.source = this;
        var store = this.source.objectStore || this.source;
        var idx = store._findRecordIndex(this.primaryKey);
        if (idx !== -1) {
            var sData = store._db._data.stores[store.name];
            sData.records.splice(idx, 1);
            store._save();
            req.result = undefined;
        } else {
            req.error = new Error("DataError: Record not found to delete");
        }
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function' && !req.error) {
            req._onsuccess.call(req, { type: 'success', target: req });
        } else if (typeof req._onerror === 'function' && req.error) {
            req._onerror.call(req, { type: 'error', target: req });
        }
        return req;
    };
    globalThis.IDBCursor = IDBCursor;

    function IDBCursorWithValue(source, direction, records, index, req) {
        IDBCursor.call(this, source, direction, records, index, req);
        var cur = records[index];
        this.value = cur ? cur.value : undefined;
    }
    IDBCursorWithValue.prototype = Object.create(IDBCursor.prototype);
    globalThis.IDBCursorWithValue = IDBCursorWithValue;

    function IDBIndex(name, store) {
        this.name = name;
        this.objectStore = store;
        var idxData = store._db._data.stores[store.name].indexes[name];
        this.keyPath = idxData.keyPath;
        this.unique = idxData.unique;
        this.multiEntry = idxData.multiEntry;
    }
    IDBIndex.prototype.get = function(key) {
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var found = undefined;
        if (sData) {
            for (var i = 0; i < sData.records.length; i++) {
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                try {
                    if (IDBFactory.prototype.cmp(idxKey, key) === 0) {
                        found = rec.value;
                        break;
                    }
                } catch (e) {
                    if (idxKey === key) { found = rec.value; break; }
                }
            }
        }
        req.result = found;
        req.readyState = 'done';
        return req;
    };
    IDBIndex.prototype.getKey = function(key) {
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var foundKey = undefined;
        if (sData) {
            for (var i = 0; i < sData.records.length; i++) {
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                try {
                    if (IDBFactory.prototype.cmp(idxKey, key) === 0) {
                        foundKey = rec.key;
                        break;
                    }
                } catch (e) {
                    if (idxKey === key) { foundKey = rec.key; break; }
                }
            }
        }
        req.result = foundKey;
        req.readyState = 'done';
        return req;
    };
    IDBIndex.prototype.getAll = function(query, count) {
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var all = [];
        if (sData) {
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                var isMatch = true;
                if (query !== undefined && query !== null) {
                    if (query instanceof IDBKeyRange) {
                        isMatch = query.includes(idxKey);
                    } else if (query.includes) {
                        isMatch = query.includes(idxKey);
                    } else {
                        try { isMatch = (IDBFactory.prototype.cmp(idxKey, query) === 0); } catch (e) { isMatch = (idxKey === query); }
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
    IDBIndex.prototype.getAllKeys = function(query, count) {
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var all = [];
        if (sData) {
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                var isMatch = true;
                if (query !== undefined && query !== null) {
                    if (query instanceof IDBKeyRange) {
                        isMatch = query.includes(idxKey);
                    } else if (query.includes) {
                        isMatch = query.includes(idxKey);
                    } else {
                        try { isMatch = (IDBFactory.prototype.cmp(idxKey, query) === 0); } catch (e) { isMatch = (idxKey === query); }
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
    IDBIndex.prototype.count = function(query) {
        return this.objectStore.count(query);
    };
    IDBIndex.prototype.openCursor = function(query, direction) {
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.objectStore.transaction;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var records = sData ? sData.records.slice() : [];
        var matched = [];
        for (var i = 0; i < records.length; i++) {
            var rec = records[i];
            var idxKey = _extractIdbKey(rec.value, this.keyPath);
            if (idxKey !== undefined) {
                var isMatch = true;
                if (query !== undefined && query !== null) {
                    if (query instanceof IDBKeyRange) {
                        isMatch = query.includes(idxKey);
                    } else if (query.includes) {
                        isMatch = query.includes(idxKey);
                    } else {
                        try { isMatch = (IDBFactory.prototype.cmp(idxKey, query) === 0); } catch (e) { isMatch = (idxKey === query); }
                    }
                }
                if (isMatch) {
                    matched.push({ key: idxKey, primaryKey: rec.key, value: rec.value });
                }
            }
        }
        try {
            matched.sort(function(a, b) {
                var c = IDBFactory.prototype.cmp(a.key, b.key);
                if (c !== 0) return c;
                return IDBFactory.prototype.cmp(a.primaryKey, b.primaryKey);
            });
        } catch (e) {}
        if (direction === 'prev' || direction === 'prevunique') {
            matched.reverse();
        }
        if (matched.length > 0) {
            var cursor = new IDBCursorWithValue(this, direction || 'next', matched, 0, req);
            cursor.key = matched[0].key;
            cursor.primaryKey = matched[0].primaryKey;
            cursor.value = matched[0].value;
            req.result = cursor;
        } else {
            req.result = null;
        }
        req.readyState = 'done';
        return req;
    };
    IDBIndex.prototype.openKeyCursor = function(query, direction) {
        var req = this.openCursor(query, direction);
        if (req.result) {
            req.result.value = undefined;
        }
        return req;
    };
    globalThis.IDBIndex = IDBIndex;

    function IDBDatabase(name, version, data) {
        this.name = name;
        this.version = version;
        this._data = data;
        this.objectStoreNames = new DOMStringList(Object.keys(data.stores));
        this._listeners = {};
    }
    IDBDatabase.prototype._persist = function() {
        _mangoIdbSave(this.name, JSON.stringify(this._data));
    };
    IDBDatabase.prototype.createObjectStore = function(name, options) {
        if (this._data.stores[name]) {
            throw new Error("ConstraintError: An object store with the specified name already exists");
        }
        this._data.stores[name] = {
            name: name,
            keyPath: options && options.keyPath !== undefined ? options.keyPath : null,
            autoIncrement: !!(options && options.autoIncrement),
            indexes: {},
            records: [],
            nextId: 1
        };
        this._persist();
        this.objectStoreNames = new DOMStringList(Object.keys(this._data.stores));
        return new IDBObjectStore(name, this, null);
    };
    IDBDatabase.prototype.deleteObjectStore = function(name) {
        if (!this._data.stores[name]) {
            throw new Error("NotFoundError: The specified object store was not found");
        }
        delete this._data.stores[name];
        this._persist();
        this.objectStoreNames = new DOMStringList(Object.keys(this._data.stores));
    };
    IDBDatabase.prototype.transaction = function(storeNames, mode) {
        return new IDBTransaction(this, storeNames, mode);
    };
    IDBDatabase.prototype.close = function() {};
    IDBDatabase.prototype.addEventListener = IDBRequest.prototype.addEventListener;
    IDBDatabase.prototype.removeEventListener = IDBRequest.prototype.removeEventListener;
    IDBDatabase.prototype.dispatchEvent = IDBRequest.prototype.dispatchEvent;
    globalThis.IDBDatabase = IDBDatabase;

    function IDBFactory() {}
    IDBFactory.prototype.open = function(name, version) {
        var req = new IDBOpenDBRequest();
        var raw = _mangoIdbLoad(name);
        var dbData = null;
        if (raw) {
            try { dbData = JSON.parse(raw); } catch (e) {}
        }
        var isNew = !dbData;
        if (isNew) {
            dbData = { name: name, version: 0, stores: {} };
        }
        var currentVersion = dbData.version || 0;
        var targetVersion = version !== undefined ? version : (currentVersion || 1);
        if (targetVersion < currentVersion) {
            req.error = new Error("VersionError: Requested version is lower than current version");
            req.readyState = 'done';
            return req;
        }
        var db = new IDBDatabase(name, targetVersion, dbData);
        req.result = db;
        req.readyState = 'done';
        if (targetVersion > currentVersion) {
            var oldVersion = currentVersion;
            dbData.version = targetVersion;
            db.version = targetVersion;
            db._persist();
            var tx = new IDBTransaction(db, Object.keys(dbData.stores), 'versionchange');
            req.transaction = tx;
            var upEvt = new IDBVersionChangeEvent('upgradeneeded', {
                oldVersion: oldVersion,
                newVersion: targetVersion
            });
            upEvt.target = req;
            req._needsUpgrade = true;
            req._upgradeEvent = upEvt;
        }
        return req;
    };
    IDBFactory.prototype.deleteDatabase = function(name) {
        var req = new IDBOpenDBRequest();
        _mangoIdbDelete(name);
        req.result = undefined;
        req.readyState = 'done';
        return req;
    };
    IDBFactory.prototype.databases = function() {
        var list = _mangoIdbList() || [];
        var res = [];
        for (var i = 0; i < list.length; i++) {
            var raw = _mangoIdbLoad(list[i]);
            var ver = 1;
            if (raw) {
                try { ver = JSON.parse(raw).version || 1; } catch (e) {}
            }
            res.push({ name: list[i], version: ver });
        }
        return Promise.resolve(res);
    };
    IDBFactory.prototype.cmp = function(a, b) {
        var tA = _idbKeyType(a);
        var tB = _idbKeyType(b);
        if (tA === 0 || tB === 0) {
            throw new TypeError("DataError: The operand is not a valid key.");
        }
        if (tA !== tB) {
            return tA < tB ? -1 : 1;
        }
        if (tA === 1) {
            return a < b ? -1 : (a > b ? 1 : 0);
        }
        if (tA === 2) {
            var t1 = a.getTime(), t2 = b.getTime();
            return t1 < t2 ? -1 : (t1 > t2 ? 1 : 0);
        }
        if (tA === 3) {
            return a < b ? -1 : (a > b ? 1 : 0);
        }
        if (tA === 4) {
            var len = Math.min(a.length, b.length);
            for (var i = 0; i < len; i++) {
                var res = IDBFactory.prototype.cmp(a[i], b[i]);
                if (res !== 0) return res;
            }
            return a.length < b.length ? -1 : (a.length > b.length ? 1 : 0);
        }
        return 0;
    };
    globalThis.IDBFactory = IDBFactory;
    globalThis.IDBDatabase = IDBDatabase;
    globalThis.IDBObjectStore = IDBObjectStore;
    globalThis.IDBIndex = IDBIndex;
    globalThis.IDBCursor = IDBCursor;
    globalThis.IDBCursorWithValue = IDBCursorWithValue;
    globalThis.indexedDB = new IDBFactory();

