    // ── 22. XMLHttpRequest (legacy compatibility) ─────────────────────────────
    function XMLHttpRequestUpload() {
        EventTarget.call(this);
        this.onloadstart = null;
        this.onprogress  = null;
        this.onabort     = null;
        this.onerror     = null;
        this.onload      = null;
        this.ontimeout   = null;
        this.onloadend   = null;
    }
    XMLHttpRequestUpload.prototype = Object.create(EventTarget.prototype);
    globalThis.XMLHttpRequestUpload = XMLHttpRequestUpload;

    function XMLHttpRequest() {
        EventTarget.call(this);
        this.readyState    = 0; // UNSENT
        this.status        = 0;
        this.statusText    = '';
        this.responseText  = '';
        this.response      = '';
        this.responseType  = '';
        this.responseURL   = '';
        this.responseXML   = null;
        this.timeout       = 0;
        this.withCredentials = false;
        this.upload        = new XMLHttpRequestUpload();
        this.onreadystatechange = null;
        this.onloadstart   = null;
        this.onprogress    = null;
        this.onabort       = null;
        this.onerror       = null;
        this.onload        = null;
        this.ontimeout     = null;
        this.onloadend     = null;
        this._method       = 'GET';
        this._url          = '';
        this._async        = true;
        this._headers      = {};
        this._responseHeaders = {};
        this._aborted      = false;
    }
    XMLHttpRequest.prototype = Object.create(EventTarget.prototype);
    var _xhrConsts = { UNSENT: 0, OPENED: 1, HEADERS_RECEIVED: 2, LOADING: 3, DONE: 4 };
    Object.assign(XMLHttpRequest, _xhrConsts);
    Object.assign(XMLHttpRequest.prototype, _xhrConsts);

    XMLHttpRequest.prototype.open = function(method, url, async, user, password) {
        this._method = String(method || 'GET').toUpperCase();
        this._url    = String(url || '');
        this._async  = async !== false;
        this._aborted = false;
        this._setReadyState(1 /* OPENED */);
    };
    XMLHttpRequest.prototype.setRequestHeader = function(k, v) {
        var lk = String(k).toLowerCase();
        this._headers[lk] = this._headers[lk] ? this._headers[lk] + ', ' + String(v) : String(v);
    };
    XMLHttpRequest.prototype.getResponseHeader = function(k) {
        if (this.readyState < 2) return null;
        var lk = String(k).toLowerCase();
        return this._responseHeaders[lk] !== undefined ? this._responseHeaders[lk] : null;
    };
    XMLHttpRequest.prototype.getAllResponseHeaders = function() {
        if (this.readyState < 2) return '';
        var lines = [];
        for (var k in this._responseHeaders) {
            lines.push(k + ': ' + this._responseHeaders[k]);
        }
        return lines.join('\r\n') + (lines.length > 0 ? '\r\n' : '');
    };
    XMLHttpRequest.prototype.overrideMimeType = function(m) {
        this._overrideMime = m;
    };
    XMLHttpRequest.prototype.abort = function() {
        if (this.readyState > 0 && this.readyState < 4) {
            this._aborted = true;
            this._setReadyState(4);
            var evt = { type: 'abort', target: this, currentTarget: this };
            this.dispatchEvent(evt);
            var endEvt = { type: 'loadend', target: this, currentTarget: this };
            this.dispatchEvent(endEvt);
        }
    };
    XMLHttpRequest.prototype.send = function(body) {
        var self = this;
        if (this._aborted) return;
        this.dispatchEvent({ type: 'loadstart', target: this });
        this.upload.dispatchEvent({ type: 'loadstart', target: this.upload });
        
        var hdrs = JSON.stringify(this._headers);
        var b    = (body !== undefined && body !== null) ? String(body) : '';
        var url  = this._url;
        if (url && url.indexOf('://') === -1 && url.indexOf('//') !== 0) {
            try {
                var base = globalThis.location ? globalThis.location.origin + (globalThis.location.pathname || '/') : '';
                url = new URL(url, base).href;
            } catch(e) {}
        }
        
        setTimeout(function() {
            if (self._aborted) return;
            try {
                self.upload.dispatchEvent({ type: 'progress', lengthComputable: true, loaded: b.length, total: b.length, target: self.upload });
                self.upload.dispatchEvent({ type: 'load', target: self.upload });
                self.upload.dispatchEvent({ type: 'loadend', target: self.upload });
                
                var result = JSON.parse(_mangoFetch(url, self._method, hdrs, b));
                self._setReadyState(2 /* HEADERS_RECEIVED */);
                self.status       = result.status;
                self.statusText   = result.status === 200 ? 'OK' : String(result.status);
                self.responseURL  = url;
                if (result.contentType) {
                    self._responseHeaders['content-type'] = result.contentType;
                }
                
                self._setReadyState(3 /* LOADING */);
                var bodyText = result.body || '';
                self.responseText = bodyText;
                if (self.responseType === 'json') {
                    try { self.response = JSON.parse(bodyText); } catch(_) { self.response = null; }
                } else if (self.responseType === 'arraybuffer') {
                    self.response = new TextEncoder().encode(bodyText).buffer;
                } else if (self.responseType === 'blob') {
                    self.response = new Blob([bodyText], { type: result.contentType || 'text/plain' });
                } else {
                    self.response = bodyText;
                }
                
                self._setReadyState(4 /* DONE */);
                self.dispatchEvent({ type: 'progress', lengthComputable: true, loaded: bodyText.length, total: bodyText.length, target: self });
                if (result.ok) {
                    self.dispatchEvent({ type: 'load', target: self });
                } else {
                    self.dispatchEvent({ type: 'error', target: self });
                }
                self.dispatchEvent({ type: 'loadend', target: self });
            } catch(e) {
                self.status = 0;
                self._setReadyState(4 /* DONE */);
                self.dispatchEvent({ type: 'error', target: self, message: e.message });
                self.dispatchEvent({ type: 'loadend', target: self });
            }
        }, 0);
    };
    XMLHttpRequest.prototype._setReadyState = function(rs) {
        this.readyState = rs;
        this.dispatchEvent({ type: 'readystatechange', target: this });
    };
    globalThis.XMLHttpRequest = XMLHttpRequest;

    // ── 23. WebSocket ─────────────────────────────────────────────────────────
    var _nextWsId = 1;
    var _activeWebSockets = {};

    globalThis._mangoDispatchWsOpen = function(id, protocol) {
        var ws = _activeWebSockets[id];
        if (!ws) return;
        ws.readyState = 1; // OPEN
        ws.protocol = protocol || '';
        var evt = (typeof globalThis.Event === 'function')
            ? new globalThis.Event('open')
            : { type: 'open', target: ws };
        if (typeof ws.onopen === 'function') try { ws.onopen(evt); } catch(e) {}
        ws.dispatchEvent(evt);
    };

    globalThis._mangoDispatchWsMessage = function(id, data) {
        var ws = _activeWebSockets[id];
        if (!ws) return;
        var evt = (typeof globalThis.MessageEvent === 'function')
            ? new globalThis.MessageEvent('message', { data: data, origin: ws.url, source: ws })
            : { type: 'message', data: data, origin: ws.url, target: ws };
        if (typeof ws.onmessage === 'function') try { ws.onmessage(evt); } catch(e) {}
        ws.dispatchEvent(evt);
    };

    globalThis._mangoDispatchWsError = function(id, msg) {
        var ws = _activeWebSockets[id];
        if (!ws) return;
        var evt = (typeof globalThis.ErrorEvent === 'function')
            ? new globalThis.ErrorEvent('error', { message: msg })
            : { type: 'error', message: msg, target: ws };
        if (typeof ws.onerror === 'function') try { ws.onerror(evt); } catch(e) {}
        ws.dispatchEvent(evt);
    };

    globalThis._mangoDispatchWsClose = function(id, code, reason, wasClean) {
        var ws = _activeWebSockets[id];
        if (!ws) return;
        ws.readyState = 3; // CLOSED
        var evt = (typeof globalThis.CloseEvent === 'function')
            ? new globalThis.CloseEvent('close', { code: code, reason: reason, wasClean: wasClean })
            : { type: 'close', code: code, reason: reason, wasClean: wasClean, target: ws };
        if (typeof ws.onclose === 'function') try { ws.onclose(evt); } catch(e) {}
        ws.dispatchEvent(evt);
        delete _activeWebSockets[id];
    };

    function WebSocket(url, protocols) {
        EventTarget.call(this);
        this.url           = String(url || '');
        this.protocol      = '';
        this.extensions    = '';
        this.bufferedAmount = 0;
        this.readyState    = 0; // CONNECTING
        this.binaryType    = 'blob';
        this.onopen        = null;
        this.onclose       = null;
        this.onmessage     = null;
        this.onerror       = null;

        var id = _nextWsId++;
        this._wsId = id;
        _activeWebSockets[id] = this;

        if (typeof _mangoWsConnect === 'function') {
            _mangoWsConnect(id, this.url, String(protocols || ''));
        }
    }
    WebSocket.CONNECTING = 0;
    WebSocket.OPEN       = 1;
    WebSocket.CLOSING    = 2;
    WebSocket.CLOSED     = 3;
    WebSocket.prototype = Object.create(EventTarget.prototype);
    Object.assign(WebSocket.prototype, {
        CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3,
        send: function(data) {
            if (this.readyState !== 1) {
                throw new Error("InvalidStateError: The WebSocket connection is not in the OPEN state.");
            }
            if (typeof _mangoWsSend === 'function') {
                _mangoWsSend(this._wsId, String(data));
            }
        },
        close: function(code, reason) {
            if (this.readyState === 2 || this.readyState === 3) return;
            this.readyState = 2; // CLOSING
            if (typeof _mangoWsClose === 'function') {
                _mangoWsClose(this._wsId, code !== undefined ? code : 1000, String(reason || ''));
            }
        }
    });
    globalThis.WebSocket = WebSocket;
    if (typeof window !== 'undefined') {
        window.WebSocket = WebSocket;
    }

    // ── 24. EventSource ───────────────────────────────────────────────────────
    function EventSource(url, eventSourceInitDict) {
        EventTarget.call(this);
        this.url = String(url || '');
        this.withCredentials = !!(eventSourceInitDict && eventSourceInitDict.withCredentials);
        this.readyState = 0; // CONNECTING
        this.onopen = null;
        this.onmessage = null;
        this.onerror = null;

        var self = this;
        setTimeout(function() {
            if (self.readyState === 0) {
                self.readyState = 1; // OPEN
                var openEvt = { type: 'open', target: self };
                if (typeof self.onopen === 'function') try { self.onopen(openEvt); } catch(e) {}
                self.dispatchEvent(openEvt);
            }
        }, 0);
    }
    EventSource.CONNECTING = 0;
    EventSource.OPEN       = 1;
    EventSource.CLOSED     = 2;
    EventSource.prototype = Object.create(EventTarget.prototype);
    Object.assign(EventSource.prototype, {
        CONNECTING: 0, OPEN: 1, CLOSED: 2,
        close: function() {
            this.readyState = 2; // CLOSED
        }
    });
    globalThis.EventSource = EventSource;
    if (typeof window !== 'undefined') {
        window.EventSource = EventSource;
    }

