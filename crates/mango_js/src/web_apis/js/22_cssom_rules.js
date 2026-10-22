    // ── CSSOM § 4 CSSRule & Subclasses ───────────────────────────────────────
    function CSSRule() {
        this.parentRule = null;
        this.parentStyleSheet = null;
    }

    CSSRule.STYLE_RULE = 1;
    CSSRule.CHARSET_RULE = 2;
    CSSRule.IMPORT_RULE = 3;
    CSSRule.MEDIA_RULE = 4;
    CSSRule.FONT_FACE_RULE = 5;
    CSSRule.PAGE_RULE = 6;
    CSSRule.KEYFRAMES_RULE = 7;
    CSSRule.KEYFRAME_RULE = 8;
    CSSRule.MARGIN_RULE = 9;
    CSSRule.NAMESPACE_RULE = 10;
    CSSRule.COUNTER_STYLE_RULE = 11;
    CSSRule.SUPPORTS_RULE = 12;
    CSSRule.DOCUMENT_RULE = 13;
    CSSRule.FONT_FEATURE_VALUES_RULE = 14;
    CSSRule.VIEWPORT_RULE = 15;
    CSSRule.REGION_STYLE_RULE = 16;

    for (var k in CSSRule) {
        if (typeof CSSRule[k] === 'number') {
            CSSRule.prototype[k] = CSSRule[k];
        }
    }
    CSSRule.prototype.type = 0;

    CSSRule.prototype._notifyMutate = function() {
        if (this.parentRule && typeof this.parentRule._notifyMutate === 'function') {
            this.parentRule._notifyMutate();
        } else if (this.parentStyleSheet && typeof this.parentStyleSheet._onMutate === 'function') {
            this.parentStyleSheet._onMutate();
        }
    };

    // ── CSSRuleList ──────────────────────────────────────────────────────────
    function CSSRuleList() {
        var arr = [];
        Object.setPrototypeOf(arr, CSSRuleList.prototype);
        return arr;
    }
    CSSRuleList.prototype = Object.create(Array.prototype);
    CSSRuleList.prototype.constructor = CSSRuleList;
    CSSRuleList.prototype.item = function(index) {
        index = Number(index);
        return (index >= 0 && index < this.length) ? this[index] : null;
    };
    Object.defineProperty(CSSRuleList.prototype, Symbol.toStringTag, {
        value: 'CSSRuleList',
        configurable: true
    });

    // ── MediaList ────────────────────────────────────────────────────────────
    function _createMediaList(mediaText) {
        var raw = String(mediaText || '').trim();
        var list = raw ? raw.split(',').map(function(s) { return s.trim(); }).filter(Boolean) : [];
        return {
            length: list.length,
            mediaText: list.join(', '),
            item: function(i) { return list[i] || null; },
            appendMedium: function(m) {
                list.push(String(m).trim());
                this.length = list.length;
                this.mediaText = list.join(', ');
            },
            deleteMedium: function(m) {
                var idx = list.indexOf(String(m).trim());
                if (idx !== -1) {
                    list.splice(idx, 1);
                    this.length = list.length;
                    this.mediaText = list.join(', ');
                }
            },
            toString: function() { return this.mediaText; }
        };
    }

    // ── CSSGroupingRule ──────────────────────────────────────────────────────
    function CSSGroupingRule() {
        CSSRule.call(this);
        this._rules = new CSSRuleList();
    }
    CSSGroupingRule.prototype = Object.create(CSSRule.prototype);
    CSSGroupingRule.prototype.constructor = CSSGroupingRule;

    Object.defineProperty(CSSGroupingRule.prototype, 'cssRules', {
        get: function() { return this._rules; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(CSSGroupingRule.prototype, 'rules', {
        get: function() { return this._rules; },
        configurable: true,
        enumerable: true
    });

    // ── CSSConditionRule ─────────────────────────────────────────────────────
    function CSSConditionRule(condition) {
        CSSGroupingRule.call(this);
        this._conditionText = String(condition || '').trim();
    }
    CSSConditionRule.prototype = Object.create(CSSGroupingRule.prototype);
    CSSConditionRule.prototype.constructor = CSSConditionRule;

    Object.defineProperty(CSSConditionRule.prototype, 'conditionText', {
        get: function() { return this._conditionText; },
        configurable: true,
        enumerable: true
    });

    // ── CSSMediaRule ─────────────────────────────────────────────────────────
    function CSSMediaRule(query) {
        CSSConditionRule.call(this, query);
        this.type = CSSRule.MEDIA_RULE;
        this.media = _createMediaList(query);
    }
    CSSMediaRule.prototype = Object.create(CSSConditionRule.prototype);
    CSSMediaRule.prototype.constructor = CSSMediaRule;

    CSSMediaRule.prototype._serialize = function() {
        var header = '@media ' + this.media.mediaText;
        if (!this._rules || this._rules.length === 0) return header + ' {\n}';
        var inner = this._rules.map(function(r) {
            var txt = (typeof r._serialize === 'function') ? r._serialize() : (r.cssText || '');
            return '  ' + txt.replace(/\n/g, '\n  ');
        }).join('\n');
        return header + ' {\n' + inner + '\n}';
    };

    Object.defineProperty(CSSMediaRule.prototype, 'cssText', {
        get: function() { return this._serialize(); },
        configurable: true,
        enumerable: true
    });

    // ── CSSSupportsRule ──────────────────────────────────────────────────────
    function CSSSupportsRule(condition) {
        CSSConditionRule.call(this, condition);
        this.type = CSSRule.SUPPORTS_RULE;
    }
    CSSSupportsRule.prototype = Object.create(CSSConditionRule.prototype);
    CSSSupportsRule.prototype.constructor = CSSSupportsRule;

    CSSSupportsRule.prototype._serialize = function() {
        var header = '@supports ' + this._conditionText;
        if (!this._rules || this._rules.length === 0) return header + ' {\n}';
        var inner = this._rules.map(function(r) {
            var txt = (typeof r._serialize === 'function') ? r._serialize() : (r.cssText || '');
            return '  ' + txt.replace(/\n/g, '\n  ');
        }).join('\n');
        return header + ' {\n' + inner + '\n}';
    };

    Object.defineProperty(CSSSupportsRule.prototype, 'cssText', {
        get: function() { return this._serialize(); },
        configurable: true,
        enumerable: true
    });

    // ── CSSLayerBlockRule ────────────────────────────────────────────────────
    function CSSLayerBlockRule(name) {
        CSSGroupingRule.call(this);
        this.name = String(name || '').trim();
    }
    CSSLayerBlockRule.prototype = Object.create(CSSGroupingRule.prototype);
    CSSLayerBlockRule.prototype.constructor = CSSLayerBlockRule;

    CSSLayerBlockRule.prototype._serialize = function() {
        var header = this.name ? '@layer ' + this.name : '@layer';
        if (!this._rules || this._rules.length === 0) return header + ' {\n}';
        var inner = this._rules.map(function(r) {
            var txt = (typeof r._serialize === 'function') ? r._serialize() : (r.cssText || '');
            return '  ' + txt.replace(/\n/g, '\n  ');
        }).join('\n');
        return header + ' {\n' + inner + '\n}';
    };

    Object.defineProperty(CSSLayerBlockRule.prototype, 'cssText', {
        get: function() { return this._serialize(); },
        configurable: true,
        enumerable: true
    });

    // ── CSSLayerStatementRule ────────────────────────────────────────────────
    function CSSLayerStatementRule(names) {
        CSSRule.call(this);
        this.nameList = Array.isArray(names) ? names.slice() : [];
    }
    CSSLayerStatementRule.prototype = Object.create(CSSRule.prototype);
    CSSLayerStatementRule.prototype.constructor = CSSLayerStatementRule;

    CSSLayerStatementRule.prototype._serialize = function() {
        return '@layer ' + this.nameList.join(', ') + ';';
    };

    Object.defineProperty(CSSLayerStatementRule.prototype, 'cssText', {
        get: function() { return this._serialize(); },
        configurable: true,
        enumerable: true
    });

    // ── CSSScopeRule ─────────────────────────────────────────────────────────
    function CSSScopeRule(start, end) {
        CSSGroupingRule.call(this);
        this._start = start !== undefined ? start : null;
        this._end = end !== undefined ? end : null;
    }
    CSSScopeRule.prototype = Object.create(CSSGroupingRule.prototype);
    CSSScopeRule.prototype.constructor = CSSScopeRule;

    Object.defineProperty(CSSScopeRule.prototype, 'start', {
        get: function() { return this._start; },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(CSSScopeRule.prototype, 'end', {
        get: function() { return this._end; },
        configurable: true,
        enumerable: true
    });

    CSSScopeRule.prototype._serialize = function() {
        var p = "";
        if (this._start !== null) p += '(' + this._start + ')';
        if (this._end !== null) {
            if (p) p += ' ';
            p += 'to (' + this._end + ')';
        }
        var header = p ? '@scope ' + p : '@scope';
        if (!this._rules || this._rules.length === 0) {
            return header + ' {\n}';
        }
        var inner = [];
        for (var i = 0; i < this._rules.length; i++) {
            var r = this._rules[i];
            var txt = (typeof r._serialize === 'function') ? r._serialize() : (r.cssText || '');
            inner.push('  ' + txt.replace(/\n/g, '\n  '));
        }
        return header + ' {\n' + inner.join('\n') + '\n}';
    };

    Object.defineProperty(CSSScopeRule.prototype, 'cssText', {
        get: function() { return this._serialize(); },
        configurable: true,
        enumerable: true
    });

    // ── CSSNestedDeclarations ────────────────────────────────────────────────
    function CSSNestedDeclarations(rawText) {
        CSSRule.call(this);
        this._style = (typeof globalThis.CSSStyleDeclaration === 'function')
            ? new globalThis.CSSStyleDeclaration()
            : { _props: {}, setProperty: function(p, v) { this._props[p] = v; } };
        this._style._rule = this;
        if (rawText) {
            this._setRaw(rawText);
        }
    }
    CSSNestedDeclarations.prototype = Object.create(CSSRule.prototype);
    CSSNestedDeclarations.prototype.constructor = CSSNestedDeclarations;

    CSSNestedDeclarations.prototype._setRaw = function(raw) {
        this._style._props = {};
        var decls = String(raw).split(';');
        for (var i = 0; i < decls.length; i++) {
            var d = decls[i].trim();
            if (!d) continue;
            var colon = d.indexOf(':');
            if (colon !== -1) {
                var prop = d.slice(0, colon).trim();
                var val = d.slice(colon + 1).trim();
                if (typeof this._style.setProperty === 'function') {
                    this._style.setProperty(prop, val);
                } else {
                    this._style._props[prop] = val;
                }
            }
        }
    };

    CSSNestedDeclarations.prototype._serialize = function() {
        var parts = [];
        var props = this._style._props || {};
        for (var k in props) {
            if (Object.prototype.hasOwnProperty.call(props, k) && props[k] !== undefined && props[k] !== "") {
                parts.push(k + ': ' + props[k] + ';');
            }
        }
        return parts.join(' ');
    };

    Object.defineProperty(CSSNestedDeclarations.prototype, 'style', {
        get: function() { return this._style; },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(CSSNestedDeclarations.prototype, 'cssText', {
        get: function() { return this._serialize(); },
        set: function(val) {
            this._setRaw(val);
            this._notifyMutate();
        },
        configurable: true,
        enumerable: true
    });

    // ── CSSStyleRule ─────────────────────────────────────────────────────────
    function CSSStyleRule(sel, bodyText) {
        CSSGroupingRule.call(this);
        this.type = CSSRule.STYLE_RULE;
        this._selectorText = String(sel || '').trim();
        this._style = (typeof globalThis.CSSStyleDeclaration === 'function')
            ? new globalThis.CSSStyleDeclaration()
            : { _props: {}, setProperty: function(p, v) { this._props[p] = v; } };
        this._style._rule = this;
        if (bodyText) {
            this._parseBody(bodyText);
        }
    }
    CSSStyleRule.prototype = Object.create(CSSGroupingRule.prototype);
    CSSStyleRule.prototype.constructor = CSSStyleRule;

    CSSStyleRule.prototype._serialize = function() {
        var parts = [];
        var props = this._style._props || {};
        for (var k in props) {
            if (Object.prototype.hasOwnProperty.call(props, k) && props[k] !== undefined && props[k] !== "") {
                parts.push(k + ': ' + props[k] + ';');
            }
        }
        var declsText = parts.join(' ');
        var inner = [];
        if (declsText) inner.push(declsText);
        if (this._rules && this._rules.length > 0) {
            for (var i = 0; i < this._rules.length; i++) {
                var r = this._rules[i];
                var txt = (typeof r._serialize === 'function') ? r._serialize() : (r.cssText || '');
                inner.push(txt);
            }
        }
        if (inner.length === 0) {
            return this._selectorText + ' { }';
        }
        if (this._rules && this._rules.length > 0) {
            return this._selectorText + ' {\n  ' + inner.join('\n  ') + '\n}';
        }
        return this._selectorText + ' { ' + inner.join(' ') + ' }';
    };

    Object.defineProperty(CSSStyleRule.prototype, 'selectorText', {
        get: function() { return this._selectorText; },
        set: function(val) {
            this._selectorText = String(val).trim();
            this._notifyMutate();
        },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(CSSStyleRule.prototype, 'style', {
        get: function() { return this._style; },
        configurable: true,
        enumerable: true
    });

    Object.defineProperty(CSSStyleRule.prototype, 'cssText', {
        get: function() { return this._serialize(); },
        set: function(val) {
            this._notifyMutate();
        },
        configurable: true,
        enumerable: true
    });

    // ── CSSPositionTryRule (CSS Anchor Positioning § 5) ─────────────────────
    var POSITION_TRY_ALLOWED = {
        'top': 1, 'left': 1, 'right': 1, 'bottom': 1, 'inset': 1,
        'inset-block': 1, 'inset-inline': 1, 'inset-block-start': 1, 'inset-block-end': 1,
        'inset-inline-start': 1, 'inset-inline-end': 1,
        'width': 1, 'height': 1, 'min-width': 1, 'min-height': 1, 'max-width': 1, 'max-height': 1,
        'block-size': 1, 'inline-size': 1, 'min-block-size': 1, 'min-inline-size': 1, 'max-block-size': 1, 'max-inline-size': 1,
        'margin': 1, 'margin-top': 1, 'margin-left': 1, 'margin-right': 1, 'margin-bottom': 1,
        'margin-block': 1, 'margin-inline': 1, 'margin-block-start': 1, 'margin-block-end': 1,
        'margin-inline-start': 1, 'margin-inline-end': 1,
        'align-self': 1, 'justify-self': 1, 'place-self': 1,
        'position-anchor': 1, 'position-area': 1, 'position-try-order': 1
    };

    function CSSPositionTryRule(name) {
        CSSRule.call(this);
        this.name = String(name || '').trim();
        this._style = (typeof globalThis.CSSStyleDeclaration === 'function')
            ? new globalThis.CSSStyleDeclaration()
            : { _props: {}, setProperty: function(p, v) { this._props[p] = v; } };
        this._style._rule = this;
        var origSet = this._style.setProperty.bind(this._style);
        this._style.setProperty = function(prop, val, prio) {
            var p = String(prop || '').trim().toLowerCase();
            if (!POSITION_TRY_ALLOWED[p]) return;
            origSet(prop, val, prio);
        };
    }
    CSSPositionTryRule.prototype = Object.create(CSSRule.prototype);
    CSSPositionTryRule.prototype.constructor = CSSPositionTryRule;

    Object.defineProperty(CSSPositionTryRule.prototype, 'style', {
        get: function() { return this._style; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(CSSPositionTryRule.prototype, 'cssText', {
        get: function() {
            var parts = [];
            var props = this._style._props || {};
            for (var k in props) {
                if (Object.prototype.hasOwnProperty.call(props, k) && props[k] !== undefined && props[k] !== "") {
                    parts.push(k + ': ' + props[k] + ';');
                }
            }
            return '@position-try ' + this.name + ' { ' + parts.join(' ') + ' }';
        },
        set: function(val) {
            this._style._props = {};
            var str = String(val);
            var body = str.indexOf('{') !== -1 ? str.slice(str.indexOf('{') + 1, str.lastIndexOf('}')).trim() : str.trim();
            var decls = body.split(';');
            for (var d = 0; d < decls.length; d++) {
                var item = decls[d].trim();
                if (!item) continue;
                var colon = item.indexOf(':');
                if (colon !== -1) {
                    var p = item.slice(0, colon).trim().toLowerCase();
                    if (POSITION_TRY_ALLOWED[p]) {
                        this._style.setProperty(p, item.slice(colon + 1).trim());
                    }
                }
            }
            this._notifyMutate();
        },
        configurable: true,
        enumerable: true
    });

    // ── CSSPageRule (CSS Paged Media § 3) ────────────────────────────────────
    var PAGE_RULE_ALLOWED = {
        'size': 1, 'page': 1, 'margin': 1, 'margin-top': 1, 'margin-right': 1,
        'margin-bottom': 1, 'margin-left': 1, 'page-orientation': 1, 'marks': 1
    };

    function CSSPageRule(sel) {
        CSSGroupingRule.call(this);
        this.type = CSSRule.PAGE_RULE;
        this.selectorText = String(sel || '').trim();
        this._style = (typeof globalThis.CSSStyleDeclaration === 'function')
            ? new globalThis.CSSStyleDeclaration()
            : { _props: {}, setProperty: function(p, v) { this._props[p] = v; } };
        this._style._rule = this;
        var origSet = this._style.setProperty.bind(this._style);
        this._style.setProperty = function(prop, val, prio) {
            var p = String(prop || '').trim().toLowerCase();
            if (!PAGE_RULE_ALLOWED[p]) return;
            origSet(prop, val, prio);
        };
    }
    CSSPageRule.prototype = Object.create(CSSGroupingRule.prototype);
    CSSPageRule.prototype.constructor = CSSPageRule;

    Object.defineProperty(CSSPageRule.prototype, 'style', {
        get: function() { return this._style; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(CSSPageRule.prototype, 'cssText', {
        get: function() {
            var parts = [];
            var props = this._style._props || {};
            for (var k in props) {
                if (Object.prototype.hasOwnProperty.call(props, k) && props[k] !== undefined && props[k] !== "") {
                    parts.push(k + ': ' + props[k] + ';');
                }
            }
            var header = this.selectorText ? '@page ' + this.selectorText : '@page';
            return header + ' { ' + parts.join(' ') + ' }';
        },
        set: function(val) {
            this._notifyMutate();
        },
        configurable: true,
        enumerable: true
    });

    // ── CSSKeyframesRule & CSSKeyframeRule ───────────────────────────────────
    function CSSKeyframeRule(keyText) {
        CSSRule.call(this);
        this.type = CSSRule.KEYFRAME_RULE;
        this.keyText = String(keyText || '').trim();
        this._style = (typeof globalThis.CSSStyleDeclaration === 'function')
            ? new globalThis.CSSStyleDeclaration()
            : { _props: {}, setProperty: function(p, v) { this._props[p] = v; } };
        this._style._rule = this;
        var origSet = this._style.setProperty.bind(this._style);
        this._style.setProperty = function(prop, val, prio) {
            var p = String(prop || '').trim().toLowerCase();
            if (p.startsWith('animation')) return;
            origSet(prop, val, prio);
        };
    }
    CSSKeyframeRule.prototype = Object.create(CSSRule.prototype);
    CSSKeyframeRule.prototype.constructor = CSSKeyframeRule;

    Object.defineProperty(CSSKeyframeRule.prototype, 'style', {
        get: function() { return this._style; },
        configurable: true,
        enumerable: true
    });
    Object.defineProperty(CSSKeyframeRule.prototype, 'cssText', {
        get: function() {
            var parts = [];
            var props = this._style._props || {};
            for (var k in props) {
                if (Object.prototype.hasOwnProperty.call(props, k) && props[k] !== undefined && props[k] !== "") {
                    parts.push(k + ': ' + props[k] + ';');
                }
            }
            return this.keyText + ' { ' + parts.join(' ') + ' }';
        },
        configurable: true,
        enumerable: true
    });

    function CSSKeyframesRule(name) {
        CSSGroupingRule.call(this);
        this.type = CSSRule.KEYFRAMES_RULE;
        this.name = String(name || '').trim();
    }
    CSSKeyframesRule.prototype = Object.create(CSSGroupingRule.prototype);
    CSSKeyframesRule.prototype.constructor = CSSKeyframesRule;

    // ── CSSImportRule ────────────────────────────────────────────────────────
    function CSSImportRule(href, media, layerName, supportsText, canonicalCss) {
        CSSRule.call(this);
        this.type = CSSRule.IMPORT_RULE;
        this.href = href || "";
        this.media = media || _createMediaList("");
        this.layerName = layerName !== undefined ? layerName : null;
        this.supportsText = supportsText || null;
        this.styleSheet = null;
        this.cssText = canonicalCss || "";
    }
    CSSImportRule.prototype = Object.create(CSSRule.prototype);
    CSSImportRule.prototype.constructor = CSSImportRule;

    // ── CSSStyleSheet (CSSOM § 2) ────────────────────────────────────────────
    function CSSStyleSheet(options) {
        this._rules = new CSSRuleList();
        this._disabled = !!(options && options.disabled);
        this._media = _createMediaList((options && options.media) || 'all');
        this._ownerNode = null;
    }

    Object.defineProperty(CSSStyleSheet.prototype, 'cssRules', {
        get: function() { return this._rules; },
        enumerable: true
    });
    Object.defineProperty(CSSStyleSheet.prototype, 'rules', {
        get: function() { return this._rules; },
        enumerable: true
    });
    Object.defineProperty(CSSStyleSheet.prototype, 'disabled', {
        get: function() { return this._disabled; },
        set: function(v) { this._disabled = !!v; },
        enumerable: true
    });
    Object.defineProperty(CSSStyleSheet.prototype, 'media', {
        get: function() { return this._media; },
        enumerable: true
    });
    Object.defineProperty(CSSStyleSheet.prototype, 'ownerNode', {
        get: function() { return this._ownerNode || null; },
        enumerable: true
    });

    CSSStyleSheet.prototype._serialize = function() {
        var lines = [];
        for (var i = 0; i < this._rules.length; i++) {
            var r = this._rules[i];
            var txt = (typeof r._serialize === 'function') ? r._serialize() : (r.cssText || '');
            if (txt) lines.push(txt);
        }
        return lines.join('\n');
    };

    CSSStyleSheet.prototype._onMutate = function() {
        if (this._ownerNode) {
            var text = this._serialize();
            this._ownerNode._sheetText = text;
            this._ownerNode.textContent = text;
        }
    };
