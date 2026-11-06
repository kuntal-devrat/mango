    // ── 27. SVG Text & Extended DOM / CSS Interfaces ─────────────────────────
    (function() {
        if (typeof globalThis.Element === 'undefined') return;

        // SVGTextContentElement & SVGTextElement
        function SVGTextContentElement() {
            throw new TypeError("Illegal constructor");
        }
        if (globalThis.SVGGraphicsElement) {
            SVGTextContentElement.prototype = Object.create(globalThis.SVGGraphicsElement.prototype);
        } else if (globalThis.SVGElement) {
            SVGTextContentElement.prototype = Object.create(globalThis.SVGElement.prototype);
        } else {
            SVGTextContentElement.prototype = Object.create(globalThis.Element.prototype);
        }
        SVGTextContentElement.prototype.constructor = SVGTextContentElement;

        SVGTextContentElement.prototype.getNumberOfChars = function() {
            return (this.textContent || '').length;
        };
        SVGTextContentElement.prototype.getComputedTextLength = function() {
            return (this.textContent || '').length * 10;
        };
        SVGTextContentElement.prototype.getSubStringLength = function(charno, nchars) {
            return (nchars || 0) * 10;
        };
        SVGTextContentElement.prototype.getStartPositionOfChar = function(charno) {
            return { x: 0, y: 0 };
        };
        SVGTextContentElement.prototype.getEndPositionOfChar = function(charno) {
            return { x: 0, y: 0 };
        };
        SVGTextContentElement.prototype.getExtentOfChar = function(charno) {
            return { x: 0, y: 0, width: 10, height: 10 };
        };
        SVGTextContentElement.prototype.getRotationOfChar = function(charno) {
            return 0;
        };

        globalThis.SVGTextContentElement = SVGTextContentElement;

        function SVGTextElement() {
            throw new TypeError("Illegal constructor");
        }
        SVGTextElement.prototype = Object.create(SVGTextContentElement.prototype);
        SVGTextElement.prototype.constructor = SVGTextElement;
        globalThis.SVGTextElement = SVGTextElement;

        function SVGRectElement() {
            throw new TypeError("Illegal constructor");
        }
        var svgElemProto = globalThis.SVGElement ? globalThis.SVGElement.prototype : (globalThis.Element ? globalThis.Element.prototype : Object.prototype);
        SVGRectElement.prototype = Object.create(svgElemProto);
        SVGRectElement.prototype.constructor = SVGRectElement;
        Object.defineProperty(SVGRectElement.prototype, 'width', {
            get: function() {
                var val = parseFloat(this.getAttribute ? this.getAttribute('width') : 0) || 0;
                return {
                    baseVal: { value: val, unitType: 1, valueInSpecifiedUnits: val },
                    animVal: { value: val }
                };
            },
            configurable: true,
            enumerable: true
        });
        Object.defineProperty(SVGRectElement.prototype, 'height', {
            get: function() {
                var val = parseFloat(this.getAttribute ? this.getAttribute('height') : 0) || 0;
                return {
                    baseVal: { value: val, unitType: 1, valueInSpecifiedUnits: val },
                    animVal: { value: val }
                };
            },
            configurable: true,
            enumerable: true
        });
        globalThis.SVGRectElement = SVGRectElement;

        function SVGLength() {}
        SVGLength.SVG_LENGTHTYPE_UNKNOWN = 0;
        SVGLength.SVG_LENGTHTYPE_NUMBER = 1;
        SVGLength.SVG_LENGTHTYPE_PERCENTAGE = 2;
        SVGLength.SVG_LENGTHTYPE_EMS = 3;
        SVGLength.SVG_LENGTHTYPE_EXS = 4;
        SVGLength.SVG_LENGTHTYPE_PX = 5;
        SVGLength.SVG_LENGTHTYPE_CM = 6;
        SVGLength.SVG_LENGTHTYPE_MM = 7;
        SVGLength.SVG_LENGTHTYPE_IN = 8;
        SVGLength.SVG_LENGTHTYPE_PT = 9;
        SVGLength.SVG_LENGTHTYPE_PC = 10;
        globalThis.SVGLength = SVGLength;

        if (typeof window !== 'undefined') {
            window.SVGTextContentElement = SVGTextContentElement;
            window.SVGTextElement = SVGTextElement;
            window.SVGRectElement = SVGRectElement;
            window.SVGLength = SVGLength;
        }

        function _triggerFrameLoad(self) {
            if (typeof _getIframeContentDocument === 'function') {
                _getIframeContentDocument(self);
            }
            var onloadAttr = self.getAttribute ? self.getAttribute('onload') : null;
            if (onloadAttr && typeof onloadAttr === 'string') {
                try {
                    var docScope = self.ownerDocument || (typeof document !== 'undefined' ? document : null);
                    (new Function('event', 'document', onloadAttr)).call(self, { type: 'load', target: self }, docScope);
                } catch(e) {}
            }
            var evt = (typeof Event === 'function') ? new Event('load') : { type: 'load' };
            if (typeof self.onload === 'function') {
                try { self.onload.call(self, evt); } catch(e) {}
            }
            if (typeof self.dispatchEvent === 'function') {
                try { self.dispatchEvent(evt); } catch(e) {}
            }
        }

        // Frame and Object src / data setters with dynamic load event dispatch
        if (globalThis.HTMLIFrameElement && globalThis.HTMLIFrameElement.prototype) {
            Object.defineProperty(globalThis.HTMLIFrameElement.prototype, 'src', {
                get: function() { return this.getAttribute ? (this.getAttribute('src') || '') : ''; },
                set: function(val) {
                    var s = String(val);
                    if (this.setAttribute) this.setAttribute('src', s);
                    this._contentDocument = null;
                    var self = this;
                    if (self.isConnected) {
                        _triggerFrameLoad(self);
                        setTimeout(function() { _triggerFrameLoad(self); }, 0);
                    }
                },
                configurable: true,
                enumerable: true
            });
        }

        if (globalThis.HTMLObjectElement && globalThis.HTMLObjectElement.prototype) {
            Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'data', {
                get: function() { return this.getAttribute ? (this.getAttribute('data') || '') : ''; },
                set: function(val) {
                    var s = String(val);
                    if (this.setAttribute) this.setAttribute('data', s);
                    this._contentDocument = null;
                    var self = this;
                    if (self.isConnected) {
                        _triggerFrameLoad(self);
                        setTimeout(function() { _triggerFrameLoad(self); }, 0);
                    }
                },
                configurable: true,
                enumerable: true
            });
        }

        // Live Document.prototype.title across HTML, XHTML, and XML documents
        if (globalThis.Document && globalThis.Document.prototype) {
            Object.defineProperty(globalThis.Document.prototype, 'title', {
                get: function() {
                    var titles = this.getElementsByTagName ? this.getElementsByTagName('title') : [];
                    if (titles && titles.length > 0) {
                        return titles[0].textContent || '';
                    }
                    return '';
                },
                set: function(val) {
                    var str = String(val || '');
                    var titles = this.getElementsByTagName ? this.getElementsByTagName('title') : [];
                    if (titles && titles.length > 0) {
                        titles[0].textContent = str;
                    } else {
                        var head = this.head || (this.getElementsByTagName ? this.getElementsByTagName('head')[0] : null) || this.documentElement;
                        if (head && this.createElement) {
                            var ns = this.documentElement ? this.documentElement.namespaceURI : null;
                            var t = (ns && this.createElementNS) ? this.createElementNS(ns, 'title') : this.createElement('title');
                            t.textContent = str;
                            head.appendChild(t);
                        }
                    }
                },
                configurable: true,
                enumerable: true
            });
        }

        // CSSStyleSheet.prototype.href returns null for internal/inline stylesheets
        if (globalThis.CSSStyleSheet && globalThis.CSSStyleSheet.prototype) {
            if (!Object.getOwnPropertyDescriptor(globalThis.CSSStyleSheet.prototype, 'href')) {
                Object.defineProperty(globalThis.CSSStyleSheet.prototype, 'href', {
                    get: function() {
                        return this._href !== undefined ? this._href : null;
                    },
                    configurable: true,
                    enumerable: true
                });
            }
        }

        // HTMLImageElement.prototype.height reflecting stylesheet and dynamic styles
        var imgProto = globalThis.HTMLImageElement ? globalThis.HTMLImageElement.prototype : null;
        if (imgProto) {
            Object.defineProperty(imgProto, 'height', {
                get: function() {
                    if (this.style && this.style.height) {
                        var val = parseFloat(this.style.height);
                        if (!isNaN(val)) return Math.round(val);
                    }
                    var owner = this.ownerDocument || (typeof document !== 'undefined' ? document : null);
                    if (owner && owner.styleSheets) {
                        for (var s = owner.styleSheets.length - 1; s >= 0; s--) {
                            var sheet = owner.styleSheets[s];
                            var rules = sheet ? (sheet.cssRules || sheet.rules || []) : [];
                            for (var r = rules.length - 1; r >= 0; r--) {
                                var rule = rules[r];
                                if (rule && rule.selectorText && rule.selectorText.indexOf('img') !== -1) {
                                    if (rule.style && rule.style.height) {
                                        var v = parseFloat(rule.style.height);
                                        if (!isNaN(v)) return Math.round(v);
                                    }
                                }
                            }
                        }
                    }
                    var attr = this.getAttribute ? this.getAttribute('height') : null;
                    if (attr !== null) {
                        var v = parseInt(attr, 10);
                        if (!isNaN(v)) return v;
                    }
                    return this.naturalHeight || 0;
                },
                set: function(val) {
                    if (this.setAttribute) this.setAttribute('height', String(val));
                },
                configurable: true,
                enumerable: true
            });
        }
    })();
