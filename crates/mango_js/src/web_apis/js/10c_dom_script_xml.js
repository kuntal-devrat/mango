    // HTMLScriptElement prototype surface
    if (globalThis.HTMLScriptElement && globalThis.HTMLScriptElement.prototype) {
        Object.defineProperty(globalThis.HTMLScriptElement.prototype, 'src', {
            get: function() {
                var val = this.getAttribute ? this.getAttribute('src') : null;
                if (val === null || val === undefined || val === '') return '';
                try {
                    var base = (typeof document !== 'undefined' && document.baseURI) ? document.baseURI : ((globalThis.location && globalThis.location.href) ? globalThis.location.href : 'about:blank');
                    return new URL(val, base).href;
                } catch(e) {
                    return String(val);
                }
            },
            set: function(v) { if (this.setAttribute) this.setAttribute('src', String(v)); },
            configurable: true
        });
        Object.defineProperty(globalThis.HTMLScriptElement.prototype, 'type', {
            get: function() { return this.getAttribute ? (this.getAttribute('type') || '') : ''; },
            set: function(v) { if (this.setAttribute) this.setAttribute('type', String(v)); },
            configurable: true
        });
        Object.defineProperty(globalThis.HTMLScriptElement.prototype, 'async', {
            get: function() { return this.hasAttribute ? this.hasAttribute('async') : false; },
            set: function(v) {
                if (v) { if (this.setAttribute) this.setAttribute('async', ''); }
                else { if (this.removeAttribute) this.removeAttribute('async'); }
            },
            configurable: true
        });
        Object.defineProperty(globalThis.HTMLScriptElement.prototype, 'defer', {
            get: function() { return this.hasAttribute ? this.hasAttribute('defer') : false; },
            set: function(v) {
                if (v) { if (this.setAttribute) this.setAttribute('defer', ''); }
                else { if (this.removeAttribute) this.removeAttribute('defer'); }
            },
            configurable: true
        });
        Object.defineProperty(globalThis.HTMLScriptElement.prototype, 'noModule', {
            get: function() { return this.hasAttribute ? this.hasAttribute('nomodule') : false; },
            set: function(v) {
                if (v) { if (this.setAttribute) this.setAttribute('nomodule', ''); }
                else { if (this.removeAttribute) this.removeAttribute('nomodule'); }
            },
            configurable: true
        });
        Object.defineProperty(globalThis.HTMLScriptElement.prototype, 'crossOrigin', {
            get: function() { return this.getAttribute ? this.getAttribute('crossorigin') : null; },
            set: function(v) {
                if (v === null || v === undefined) { if (this.removeAttribute) this.removeAttribute('crossorigin'); }
                else if (this.setAttribute) { this.setAttribute('crossorigin', String(v)); }
            },
            configurable: true
        });
        Object.defineProperty(globalThis.HTMLScriptElement.prototype, 'text', {
            get: function() { return this.textContent || ''; },
            set: function(v) { this.textContent = String(v == null ? '' : v); },
            configurable: true
        });
    }

    globalThis.HTMLMediaElement = function() {};
    globalThis.HTMLMediaElement.prototype = Object.create(globalThis.HTMLElement.prototype);
    globalThis.HTMLMediaElement.NETWORK_EMPTY    = 0;
    globalThis.HTMLMediaElement.NETWORK_IDLE     = 1;
    globalThis.HTMLMediaElement.NETWORK_LOADING  = 2;
    globalThis.HTMLMediaElement.NETWORK_NO_SOURCE= 3;
    globalThis.HTMLMediaElement.HAVE_NOTHING     = 0;
    globalThis.HTMLMediaElement.HAVE_METADATA    = 1;
    globalThis.HTMLMediaElement.HAVE_CURRENT_DATA= 2;
    globalThis.HTMLMediaElement.HAVE_FUTURE_DATA = 3;
    globalThis.HTMLMediaElement.HAVE_ENOUGH_DATA = 4;
    Object.assign(globalThis.HTMLMediaElement.prototype, globalThis.HTMLMediaElement);
    globalThis.HTMLMediaElement.prototype.play = function() {
        this._playing = true; this._paused = false;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'true');
        var self = this;
        setTimeout(function() {
            if (self.dispatchEvent) { self.dispatchEvent({ type:'play' }); self.dispatchEvent({ type:'playing' }); }
        }, 0);
        return (typeof Promise !== 'undefined') ? Promise.resolve() : null;
    };
    globalThis.HTMLMediaElement.prototype.pause = function() {
        this._playing = false; this._paused = true;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'false');
        var self = this;
        setTimeout(function() { if (self.dispatchEvent) self.dispatchEvent({ type:'pause' }); }, 0);
    };
    globalThis.HTMLMediaElement.prototype.load = function() {
        var self = this;
        setTimeout(function() {
            if (self.dispatchEvent) {
                self.dispatchEvent({ type:'loadstart' });
                self.dispatchEvent({ type:'loadedmetadata' });
                self.dispatchEvent({ type:'canplay' });
            }
        }, 0);
    };
    globalThis.HTMLMediaElement.prototype.canPlayType = function(type) {
        if (!type) return '';
        var t = String(type).toLowerCase();
        if (t.indexOf('mp4') !== -1 || t.indexOf('webm') !== -1 || t.indexOf('ogg') !== -1 ||
            t.indexOf('audio/mpeg') !== -1 || t.indexOf('audio/mp3') !== -1 ||
            t.indexOf('audio/wav') !== -1 || t.indexOf('audio/ogg') !== -1 || t.indexOf('audio/aac') !== -1) {
            return 'probably';
        }
        return 'maybe';
    };
    Object.defineProperties(globalThis.HTMLMediaElement.prototype, {
        currentTime: {
            get: function() {
                var t = (this.getAttribute && this.getAttribute('data-mango-time')) || this._currentTime || 0;
                var num = parseFloat(t);
                return isNaN(num) ? 0 : num;
            },
            set: function(val) {
                var num = Math.max(0, parseFloat(val) || 0);
                this._currentTime = num;
                if (this.setAttribute) this.setAttribute('data-mango-time', String(num));
                if (this.dispatchEvent) this.dispatchEvent({ type: 'timeupdate' });
            },
            configurable: true
        },
        duration: {
            get: function() {
                var d = (this.getAttribute && this.getAttribute('data-mango-duration')) || this._duration || 100;
                var num = parseFloat(d);
                return isNaN(num) ? 100 : num;
            },
            configurable: true
        },
        paused: {
            get: function() {
                if (this._paused !== undefined) return this._paused;
                var p = this.getAttribute ? this.getAttribute('data-mango-playing') : null;
                if (p === 'true') return false;
                if (p === 'false') return true;
                return !this.autoplay;
            },
            configurable: true
        },
        volume: {
            get: function() { return this._volume !== undefined ? this._volume : 1.0; },
            set: function(val) {
                this._volume = Math.max(0, Math.min(1, parseFloat(val) || 0));
                if (this.dispatchEvent) this.dispatchEvent({ type: 'volumechange' });
            },
            configurable: true
        },
        muted: {
            get: function() {
                if (this._muted !== undefined) return this._muted;
                return (this.hasAttribute && this.hasAttribute('muted')) ||
                    (this.getAttribute && this.getAttribute('data-mango-muted') === 'true');
            },
            set: function(val) {
                this._muted = !!val;
                if (this.setAttribute) {
                    if (val) { this.setAttribute('muted', ''); this.setAttribute('data-mango-muted', 'true'); }
                    else { if (this.removeAttribute) this.removeAttribute('muted'); this.setAttribute('data-mango-muted', 'false'); }
                }
                if (this.dispatchEvent) this.dispatchEvent({ type: 'volumechange' });
            },
            configurable: true
        },
        playbackRate: {
            get: function() { return this._playbackRate !== undefined ? this._playbackRate : 1.0; },
            set: function(val) {
                this._playbackRate = parseFloat(val) || 1.0;
                if (this.dispatchEvent) this.dispatchEvent({ type: 'ratechange' });
            },
            configurable: true
        }
    });
    globalThis.HTMLVideoElement = function() {};
    globalThis.HTMLVideoElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLAudioElement = function() {};
    globalThis.HTMLAudioElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLCanvasElement = function() {};
    globalThis.HTMLCanvasElement.prototype = Object.create(globalThis.HTMLElement.prototype);
    globalThis.CanvasRenderingContext2D = function() {};
    globalThis.ImageData = function(dataOrWidth, widthOrHeight, heightOpt) {
        if (typeof dataOrWidth === 'number') {
            this.width  = Math.max(1, Math.floor(dataOrWidth));
            this.height = (typeof widthOrHeight === 'number') ? Math.max(1, Math.floor(widthOrHeight)) : 1;
            this.data   = new Uint8ClampedArray(this.width * this.height * 4);
        } else {
            this.data   = (dataOrWidth instanceof Uint8ClampedArray) ? dataOrWidth : new Uint8ClampedArray(dataOrWidth);
            this.width  = Math.max(1, Math.floor(widthOrHeight));
            this.height = heightOpt !== undefined ? Math.max(1, Math.floor(heightOpt)) : Math.floor(this.data.length / (this.width * 4));
        }
    };

    globalThis.Document = function() {
        this.nodeType = 9; this.nodeName = "#document"; this._isXml = true;
        this.contentType = "application/xml"; this._childNodes = []; this._documentElement = null;
        this.location = null; this._characterSet = "UTF-8"; this.characterSet = "UTF-8";
        this.charset = "UTF-8"; this.inputEncoding = "UTF-8"; this.defaultView = null; this._defaultView = null;
    };
    globalThis.Document.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Document.prototype.constructor = globalThis.Document;
    globalThis.Document.prototype.createCDATASection = function(data) {
        var DomEx = (globalThis && globalThis.DOMException) ? globalThis.DOMException : (typeof DOMException !== 'undefined' ? DOMException : Error);
        if (!this._isXml && this.contentType === 'text/html') {
            throw new DomEx("The operation is not supported.", "NotSupportedError");
        }
        var str = String(data);
        if (str.indexOf("]]>") !== -1) {
            throw new DomEx("String contains invalid characters", "InvalidCharacterError");
        }
        var s = (typeof _origCTN === 'function' && typeof _wrapElement === 'function')
            ? _wrapElement(_origCTN(str))
            : Object.create(globalThis.CDATASection ? globalThis.CDATASection.prototype : (globalThis.Text ? globalThis.Text.prototype : Object.prototype));
        if (s) {
            if (globalThis.CDATASection) Object.setPrototypeOf(s, globalThis.CDATASection.prototype);
            s._isCDATA = true;
            s._nodeType = 4;
            s.nodeType = 4;
            s._nodeName = "#cdata-section";
            s.nodeName = "#cdata-section";
            s.ownerDocument = this;
            s._ownerDocument = this;
            s.data = str;
            s._data = str;
        }
        return s;
    };
    Object.defineProperty(globalThis.Document.prototype, 'URL', {
        get: function() {
            return (this._url !== undefined) ? this._url : ((globalThis.location && globalThis.location.href) ? globalThis.location.href : 'about:blank');
        },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'documentURI', {
        get: function() { return this.URL; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'baseURI', {
        get: function() {
            var baseEl = (typeof this.querySelector === 'function') ? this.querySelector('base[href]') : null;
            if (baseEl && typeof baseEl.getAttribute === 'function') {
                var href = baseEl.getAttribute('href');
                if (href) {
                    try {
                        return new URL(href, this.URL || 'about:blank').href;
                    } catch(e) {}
                }
            }
            return this.URL || 'about:blank';
        },
        configurable: true
    });
    Object.defineProperty(globalThis.Document.prototype, 'defaultView', {
        get: function() {
            if (this._defaultView !== undefined) return this._defaultView;
            if (typeof document !== 'undefined' && this === document && typeof window !== 'undefined') return window;
            return null;
        },
        set: function(v) { this._defaultView = v; },
        configurable: true,
        enumerable: true
    });
    var _bodyProxiedEvents = [
        "onafterprint", "onbeforeprint", "onbeforeunload", "onblur", "onerror", "onfocus",
        "onhashchange", "onload", "onmessage", "onoffline", "ononline", "onpagehide", "onpageshow",
        "onpopstate", "onresize", "onscroll", "onstorage", "onunload"
    ];
    _bodyProxiedEvents.forEach(function(evt) {
        var desc = {
            get: function() {
                var doc = this.ownerDocument;
                var win = doc ? doc.defaultView : null;
                return win ? win[evt] : null;
            },
            set: function(fn) {
                var doc = this.ownerDocument;
                var win = doc ? doc.defaultView : null;
                if (win) { win[evt] = fn; }
            },
            configurable: true,
            enumerable: true
        };
        if (globalThis.HTMLBodyElement && globalThis.HTMLBodyElement.prototype) {
            Object.defineProperty(globalThis.HTMLBodyElement.prototype, evt, desc);
        }
        if (globalThis.HTMLFrameSetElement && globalThis.HTMLFrameSetElement.prototype) {
            Object.defineProperty(globalThis.HTMLFrameSetElement.prototype, evt, desc);
        }
    });
    if (globalThis.HTMLIFrameElement && globalThis.HTMLIFrameElement.prototype) {
        Object.defineProperty(globalThis.HTMLIFrameElement.prototype, 'contentWindow', {
            get: function() { return (typeof _getIframeContentWindow === 'function') ? _getIframeContentWindow(this) : (this._contentWindow || null); },
            configurable: true,
            enumerable: true
        });
        Object.defineProperty(globalThis.HTMLIFrameElement.prototype, 'contentDocument', {
            get: function() { return (typeof _getIframeContentDocument === 'function') ? _getIframeContentDocument(this) : (this._contentDocument || null); },
            configurable: true,
            enumerable: true
        });
        globalThis.HTMLIFrameElement.prototype.getSVGDocument = function() {
            return this.contentDocument || null;
        };
    }
    if (globalThis.HTMLObjectElement && globalThis.HTMLObjectElement.prototype) {
        Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'contentDocument', {
            get: function() { return (typeof _getIframeContentDocument === 'function') ? _getIframeContentDocument(this) : (this._contentDocument || null); },
            configurable: true,
            enumerable: true
        });
        globalThis.HTMLObjectElement.prototype.getSVGDocument = function() {
            return this.contentDocument || null;
        };
    }
    Object.defineProperty(globalThis.Document.prototype, 'scrollingElement', {
        get: function() {
            if (this.compatMode === 'BackCompat') {
                return this.body || null;
            }
            return this.documentElement || this.body || null;
        },
        configurable: true,
        enumerable: true
    });
    globalThis.Document.prototype.__shady_getRootNode = function(opts) { return this; };
    globalThis.Document.prototype.getRootNode = function(opts) { return this; };
    if (typeof document !== 'undefined') {
        try {
            Object.setPrototypeOf(document, globalThis.Document.prototype);
        } catch(e) {}
        document.__shady_getRootNode = function(opts) { return document; };
        document.getRootNode = function(opts) { return document; };
    }

    function _locateNamespacePrefix(el, ns) {
        if (!el || el.nodeType !== 1) return null;
        if (el.namespaceURI === ns && el.prefix !== null) return el.prefix;
        if (el.attributes) {
            for (var i = 0; i < el.attributes.length; i++) {
                var a = el.attributes[i];
                var aName = a.name || '';
                var aPrefix = a.prefix;
                var aLocal = a.localName;
                if ((aPrefix === 'xmlns' || aName.indexOf('xmlns:') === 0) && a.value === ns) {
                    return aPrefix === 'xmlns' ? aLocal : aName.substring(6);
                }
            }
        }
        var parent = el.parentElement;
        return parent ? _locateNamespacePrefix(parent, ns) : null;
    }

    function _locateNamespaceURI(el, prefix) {
        if (!el || el.nodeType !== 1) return null;
        var p = (prefix === "" || prefix === undefined) ? null : prefix;
        if (el.namespaceURI !== null && el.prefix === p) return el.namespaceURI;
        if (el.attributes) {
            for (var i = 0; i < el.attributes.length; i++) {
                var a = el.attributes[i];
                var aNs = a.namespaceURI;
                var aPrefix = a.prefix;
                var aLocal = a.localName;
                var aName = a.name || '';
                var isXmlns = (aNs === 'http://www.w3.org/2000/xmlns/' || aPrefix === 'xmlns' || (!aPrefix && aLocal === 'xmlns') || aName === 'xmlns' || aName.indexOf('xmlns:') === 0);
                if (isXmlns) {
                    var pref = aPrefix === 'xmlns' ? aLocal : (aName.indexOf('xmlns:') === 0 ? aName.substring(6) : null);
                    if (pref === p) {
                        return (a.value !== undefined && a.value !== null && a.value !== "") ? a.value : null;
                    }
                }
            }
        }
        var parent = el.parentElement;
        return parent ? _locateNamespaceURI(parent, p) : null;
    }

    if (globalThis.Node && globalThis.Node.prototype) {
        globalThis.Node.prototype.lookupPrefix = function(namespace) {
            if (!namespace) return null;
            var nt = this.nodeType;
            if (nt === 1) return _locateNamespacePrefix(this, namespace);
            if (nt === 9) return this.documentElement ? _locateNamespacePrefix(this.documentElement, namespace) : null;
            if (nt === 2) return this.ownerElement ? _locateNamespacePrefix(this.ownerElement, namespace) : null;
            if (nt === 10 || nt === 11) return null;
            return this.parentElement ? _locateNamespacePrefix(this.parentElement, namespace) : null;
        };

        globalThis.Node.prototype.lookupNamespaceURI = function(prefix) {
            var p = (prefix === "" || prefix === undefined) ? null : prefix;
            var nt = this.nodeType;
            if (nt === 1) {
                if (p === 'xml') return 'http://www.w3.org/XML/1998/namespace';
                if (p === 'xmlns') return 'http://www.w3.org/2000/xmlns/';
                return _locateNamespaceURI(this, p);
            }
            if (nt === 9) return this.documentElement ? this.documentElement.lookupNamespaceURI(p) : null;
            if (nt === 2) return this.ownerElement ? this.ownerElement.lookupNamespaceURI(p) : null;
            if (nt === 10 || nt === 11) return null;
            return this.parentElement ? this.parentElement.lookupNamespaceURI(p) : null;
        };

        globalThis.Node.prototype.isDefaultNamespace = function(namespace) {
            var ns = (namespace === "" || namespace === undefined) ? null : namespace;
            var defaultNS = this.lookupNamespaceURI(null);
            return defaultNS === ns;
        };
    }
