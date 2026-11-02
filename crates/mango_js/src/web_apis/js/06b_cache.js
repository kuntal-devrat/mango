    // ── Cache API (caches.open, caches.match, cache.put, cache.delete, cache.keys) ──
    function Cache() {
        this._items = [];
    }
    Cache.prototype = {
        match: function(request, options) {
            var url = (request instanceof Request) ? request.url : String(request);
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = ignoreSearch ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (cmpUrl === target) {
                    return Promise.resolve(this._items[i].res.clone());
                }
            }
            return Promise.resolve(undefined);
        },
        matchAll: function(request, options) {
            var results = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = (url && ignoreSearch) ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (!target || cmpUrl === target) {
                    results.push(this._items[i].res.clone());
                }
            }
            return Promise.resolve(results);
        },
        add: function(request) {
            return this.addAll([request]);
        },
        addAll: function(requests) {
            var self = this;
            var promises = requests.map(function(r) { return fetch(r); });
            return Promise.all(promises).then(function(responses) {
                for (var i = 0; i < requests.length; i++) {
                    var req = (requests[i] instanceof Request) ? requests[i] : new Request(requests[i]);
                    self.put(req, responses[i]);
                }
            });
        },
        put: function(request, response) {
            var req = (request instanceof Request) ? request : new Request(request);
            var res = response.clone();
            for (var i = 0; i < this._items.length; i++) {
                if (this._items[i].req.url === req.url) {
                    this._items[i] = { req: req, res: res };
                    return Promise.resolve();
                }
            }
            this._items.push({ req: req, res: res });
            return Promise.resolve();
        },
        delete: function(request, options) {
            var url = (request instanceof Request) ? request.url : String(request);
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = ignoreSearch ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (cmpUrl === target) {
                    this._items.splice(i, 1);
                    return Promise.resolve(true);
                }
            }
            return Promise.resolve(false);
        },
        keys: function(request, options) {
            var keys = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = (url && ignoreSearch) ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (!target || cmpUrl === target) {
                    keys.push(this._items[i].req.clone());
                }
            }
            return Promise.resolve(keys);
        }
    };
    globalThis.Cache = Cache;

    function CacheStorage() {
        this._caches = {};
    }
    CacheStorage.prototype = {
        open: function(cacheName) {
            var name = String(cacheName);
            if (!this._caches[name]) {
                this._caches[name] = new Cache();
            }
            return Promise.resolve(this._caches[name]);
        },
        has: function(cacheName) {
            return Promise.resolve(Object.prototype.hasOwnProperty.call(this._caches, String(cacheName)));
        },
        delete: function(cacheName) {
            var name = String(cacheName);
            if (Object.prototype.hasOwnProperty.call(this._caches, name)) {
                delete this._caches[name];
                return Promise.resolve(true);
            }
            return Promise.resolve(false);
        },
        keys: function() {
            return Promise.resolve(Object.keys(this._caches));
        },
        match: function(request, options) {
            var self = this;
            if (options && options.cacheName) {
                var single = this._caches[String(options.cacheName)];
                if (single) return single.match(request, options);
                return Promise.resolve(undefined);
            }
            var keys = Object.keys(this._caches);
            function checkNext(idx) {
                if (idx >= keys.length) return Promise.resolve(undefined);
                return self._caches[keys[idx]].match(request, options).then(function(res) {
                    if (res) return res;
                    return checkNext(idx + 1);
                });
            }
            return checkNext(0);
        }
    };
    globalThis.CacheStorage = CacheStorage;
    globalThis.caches = new CacheStorage();
