    // ── CSSOM Parser & StyleSheet Methods ────────────────────────────────────
    function _unwrapParens(s) {
        s = s.trim();
        if (s.startsWith('(') && s.endsWith(')')) {
            return s.slice(1, -1).trim();
        }
        return s || null;
    }

    function _parseScopePrelude(prelude) {
        prelude = String(prelude || '').trim();
        if (!prelude) return { start: null, end: null };
        var toIdx = prelude.search(/\bto\b/i);
        var startStr = null;
        var endStr = null;
        if (toIdx !== -1) {
            var left = prelude.slice(0, toIdx).trim();
            var right = prelude.slice(toIdx + 2).trim();
            if (left) startStr = _unwrapParens(left);
            if (right) endStr = _unwrapParens(right);
        } else {
            startStr = _unwrapParens(prelude);
        }
        return { start: startStr, end: endStr };
    }

    function _parseCssStatements(cssText, parentRule, parentSheet) {
        var str = String(cssText || '').trim();
        var rules = new CSSRuleList();
        var i = 0;
        var len = str.length;

        function skipWhitespaceAndComments() {
            while (i < len) {
                var ch = str.charCodeAt(i);
                if (ch <= 32) {
                    i++;
                } else if (ch === 47 && str.charCodeAt(i + 1) === 42) {
                    var endC = str.indexOf('*/', i + 2);
                    if (endC === -1) { i = len; break; }
                    i = endC + 2;
                } else {
                    break;
                }
            }
        }

        var pendingDecls = [];

        function flushPendingDecls() {
            if (pendingDecls.length > 0) {
                var combined = pendingDecls.join('; ').trim();
                if (combined) {
                    var nd = new CSSNestedDeclarations(combined);
                    nd.parentRule = parentRule || null;
                    nd.parentStyleSheet = parentSheet || null;
                    rules.push(nd);
                }
                pendingDecls = [];
            }
        }

        while (i < len) {
            skipWhitespaceAndComments();
            if (i >= len) break;

            var start = i;

            if (str[i] === '@') {
                flushPendingDecls();
                var header = "";
                var depth = 0;
                var inQuote = null;
                var foundBrace = false;
                var foundSemi = false;
                while (i < len) {
                    var c = str[i];
                    if (inQuote) {
                        if (c === '\\') { i += 2; continue; }
                        if (c === inQuote) inQuote = null;
                        i++;
                        continue;
                    }
                    if (c === '"' || c === "'") { inQuote = c; i++; continue; }
                    if (c === '(') { depth++; i++; continue; }
                    if (c === ')') { if (depth > 0) depth--; i++; continue; }
                    if (depth === 0) {
                        if (c === '{') { foundBrace = true; break; }
                        if (c === ';') { foundSemi = true; break; }
                    }
                    i++;
                }

                if (foundSemi) {
                    var statement = str.slice(start, i).trim();
                    i++;
                    var r = _createAtRuleStatement(statement, parentRule, parentSheet);
                    if (r) rules.push(r);
                    continue;
                } else if (foundBrace) {
                    var header = str.slice(start, i).trim();
                    i++;
                    var bodyStart = i;
                    var braceDepth = 1;
                    inQuote = null;
                    while (i < len && braceDepth > 0) {
                        var c = str[i];
                        if (inQuote) {
                            if (c === '\\') { i += 2; continue; }
                            if (c === inQuote) inQuote = null;
                            i++;
                            continue;
                        }
                        if (c === '"' || c === "'") { inQuote = c; i++; continue; }
                        if (c === '{') braceDepth++;
                        else if (c === '}') braceDepth--;
                        i++;
                    }
                    var body = str.slice(bodyStart, i - 1);
                    var r = _createAtRuleBlock(header, body, parentRule, parentSheet);
                    if (r) rules.push(r);
                    continue;
                } else {
                    break;
                }
            }

            var scan = i;
            var depth = 0;
            var inQuote = null;
            var firstSemi = -1;
            var firstBrace = -1;

            while (scan < len) {
                var c = str[scan];
                if (inQuote) {
                    if (c === '\\') { scan += 2; continue; }
                    if (c === inQuote) inQuote = null;
                    scan++;
                    continue;
                }
                if (c === '"' || c === "'") { inQuote = c; scan++; continue; }
                if (c === '(' || c === '[') { depth++; scan++; continue; }
                if (c === ')' || c === ']') { if (depth > 0) depth--; scan++; continue; }
                if (depth === 0) {
                    if (c === ';' && firstSemi === -1 && firstBrace === -1) {
                        firstSemi = scan;
                        break;
                    }
                    if (c === '{' && firstBrace === -1) {
                        firstBrace = scan;
                        break;
                    }
                }
                scan++;
            }

            if (firstBrace !== -1 && (firstSemi === -1 || firstBrace < firstSemi)) {
                flushPendingDecls();
                var sel = str.slice(start, firstBrace).trim();
                i = firstBrace + 1;
                var bodyStart = i;
                var braceDepth = 1;
                inQuote = null;
                while (i < len && braceDepth > 0) {
                    var c = str[i];
                    if (inQuote) {
                        if (c === '\\') { i += 2; continue; }
                        if (c === inQuote) inQuote = null;
                        i++;
                        continue;
                    }
                    if (c === '"' || c === "'") { inQuote = c; i++; continue; }
                    if (c === '{') braceDepth++;
                    else if (c === '}') braceDepth--;
                    i++;
                }
                var body = str.slice(bodyStart, i - 1);
                var sr = new CSSStyleRule(sel, body);
                sr.parentRule = parentRule || null;
                sr.parentStyleSheet = parentSheet || null;
                rules.push(sr);
                continue;
            } else if (firstSemi !== -1) {
                var decl = str.slice(start, firstSemi).trim();
                i = firstSemi + 1;
                if (decl) {
                    pendingDecls.push(decl);
                }
                continue;
            } else {
                var rest = str.slice(start).trim();
                if (rest && rest.indexOf(':') !== -1) {
                    pendingDecls.push(rest);
                }
                i = len;
                break;
            }
        }

        flushPendingDecls();
        return rules;
    }

    function _createAtRuleStatement(statement, parentRule, parentSheet) {
        if (statement.startsWith('@layer')) {
            var afterLayer = statement.slice(6).trim();
            var names = afterLayer.split(',').map(function(s) { return s.trim(); }).filter(Boolean);
            var r = new CSSLayerStatementRule(names);
            r.parentRule = parentRule || null;
            r.parentStyleSheet = parentSheet || null;
            return r;
        }

        if (statement.startsWith('@import')) {
            var afterImport = statement.slice(7).trim();
            var rawUrl = "";
            var urlStr = "";
            var rest = "";

            if (afterImport.startsWith('url(')) {
                var closeParen = afterImport.indexOf(')');
                if (closeParen !== -1) {
                    rawUrl = afterImport.slice(4, closeParen).trim();
                    if ((rawUrl.startsWith('"') && rawUrl.endsWith('"')) || (rawUrl.startsWith("'") && rawUrl.endsWith("'"))) {
                        rawUrl = rawUrl.slice(1, -1);
                    }
                    urlStr = "url('" + rawUrl + "')";
                    rest = afterImport.slice(closeParen + 1).trim();
                }
            } else if (afterImport.startsWith('"') || afterImport.startsWith("'")) {
                var q = afterImport[0];
                var endQ = afterImport.indexOf(q, 1);
                if (endQ !== -1) {
                    rawUrl = afterImport.slice(1, endQ);
                    urlStr = "url('" + rawUrl + "')";
                    rest = afterImport.slice(endQ + 1).trim();
                }
            }

            var layerStr = "";
            var layerNameVal = null;
            var supportsStr = "";
            var mediaList = [];

            if (rest) {
                if (rest === 'layer' || rest.startsWith('layer ')) {
                    layerStr = 'layer';
                    layerNameVal = '';
                    rest = rest.slice(5).trim();
                } else if (/^layer\s*\(/.test(rest)) {
                    var lpClose = rest.indexOf(')');
                    if (lpClose !== -1) {
                        var inner = rest.slice(rest.indexOf('(') + 1, lpClose).trim();
                        layerStr = 'layer(' + inner + ')';
                        layerNameVal = inner;
                        rest = rest.slice(lpClose + 1).trim();
                    }
                }

                if (rest.startsWith('supports(')) {
                    var spClose = rest.lastIndexOf(')');
                    if (spClose !== -1) {
                        supportsStr = rest.slice(0, spClose + 1);
                        rest = rest.slice(spClose + 1).trim();
                    }
                }

                if (rest) {
                    mediaList.push(rest);
                }
            }

            var canonicalCss = '@import ' + urlStr;
            if (layerStr) canonicalCss += ' ' + layerStr;
            if (supportsStr) canonicalCss += ' ' + supportsStr;
            if (mediaList.length > 0) canonicalCss += ' ' + mediaList.join(' ');
            canonicalCss += ';';

            var med = _createMediaList(mediaList.join(' '));
            var ir = new CSSImportRule(rawUrl, med, layerNameVal, supportsStr || null, canonicalCss);
            ir.parentRule = parentRule || null;
            ir.parentStyleSheet = parentSheet || null;
            return ir;
        }

        var generic = new CSSRule();
        generic.parentRule = parentRule || null;
        generic.parentStyleSheet = parentSheet || null;
        generic.cssText = statement + ';';
        return generic;
    }

    function _createAtRuleBlock(header, body, parentRule, parentSheet) {
        if (header.startsWith('@scope')) {
            var prelude = header.slice(6).trim();
            var parsedPrelude = _parseScopePrelude(prelude);
            var scopeRule = new CSSScopeRule(parsedPrelude.start, parsedPrelude.end);
            scopeRule.parentRule = parentRule || null;
            scopeRule.parentStyleSheet = parentSheet || null;
            var inner = _parseCssStatements(body, scopeRule, parentSheet);
            for (var k = 0; k < inner.length; k++) {
                scopeRule._rules.push(inner[k]);
            }
            return scopeRule;
        }

        if (header.startsWith('@layer')) {
            var name = header.slice(6).trim();
            var layerRule = new CSSLayerBlockRule(name);
            layerRule.parentRule = parentRule || null;
            layerRule.parentStyleSheet = parentSheet || null;
            var inner = _parseCssStatements(body, layerRule, parentSheet);
            for (var k = 0; k < inner.length; k++) {
                layerRule._rules.push(inner[k]);
            }
            return layerRule;
        }

        if (header.startsWith('@media')) {
            var query = header.slice(6).trim();
            var mediaRule = new CSSMediaRule(query);
            mediaRule.parentRule = parentRule || null;
            mediaRule.parentStyleSheet = parentSheet || null;
            var inner = _parseCssStatements(body, mediaRule, parentSheet);
            for (var k = 0; k < inner.length; k++) {
                mediaRule._rules.push(inner[k]);
            }
            return mediaRule;
        }

        if (header.startsWith('@supports')) {
            var cond = header.slice(9).trim();
            var supportsRule = new CSSSupportsRule(cond);
            supportsRule.parentRule = parentRule || null;
            supportsRule.parentStyleSheet = parentSheet || null;
            var inner = _parseCssStatements(body, supportsRule, parentSheet);
            for (var k = 0; k < inner.length; k++) {
                supportsRule._rules.push(inner[k]);
            }
            return supportsRule;
        }

        if (header.startsWith('@position-try')) {
            var name = header.slice(13).trim();
            var tryRule = new CSSPositionTryRule(name);
            tryRule.parentRule = parentRule || null;
            tryRule.parentStyleSheet = parentSheet || null;
            var decls = body.split(';');
            for (var d = 0; d < decls.length; d++) {
                var item = decls[d].trim();
                if (!item) continue;
                var colon = item.indexOf(':');
                if (colon !== -1) {
                    tryRule._style.setProperty(item.slice(0, colon).trim(), item.slice(colon + 1).trim());
                }
            }
            return tryRule;
        }

        if (header.startsWith('@page')) {
            var sel = header.slice(5).trim();
            var pageRule = new CSSPageRule(sel);
            pageRule.parentRule = parentRule || null;
            pageRule.parentStyleSheet = parentSheet || null;
            var decls = body.split(';');
            for (var d = 0; d < decls.length; d++) {
                var item = decls[d].trim();
                if (!item) continue;
                var colon = item.indexOf(':');
                if (colon !== -1) {
                    pageRule._style.setProperty(item.slice(0, colon).trim(), item.slice(colon + 1).trim());
                }
            }
            return pageRule;
        }

        if (header.startsWith('@keyframes') || header.startsWith('@-webkit-keyframes')) {
            var name = header.replace(/^@(-webkit-)?keyframes\s+/, '').trim();
            var kfRule = new CSSKeyframesRule(name);
            kfRule.parentRule = parentRule || null;
            kfRule.parentStyleSheet = parentSheet || null;
            var blocks = _parseCssStatements(body, kfRule, parentSheet);
            for (var b = 0; b < blocks.length; b++) {
                var blk = blocks[b];
                var keyRule = new CSSKeyframeRule(blk.selectorText || blk._selectorText || 'from');
                keyRule.parentRule = kfRule;
                keyRule.parentStyleSheet = parentSheet || null;
                var p = (blk.style && blk.style._props) ? blk.style._props : {};
                for (var pk in p) {
                    keyRule.style.setProperty(pk, p[pk]);
                }
                kfRule._rules.push(keyRule);
            }
            return kfRule;
        }

        var grouping = new CSSGroupingRule();
        grouping.parentRule = parentRule || null;
        grouping.parentStyleSheet = parentSheet || null;
        grouping.cssText = header + ' { ' + body.trim() + ' }';
        var inner = _parseCssStatements(body, grouping, parentSheet);
        for (var k = 0; k < inner.length; k++) {
            grouping._rules.push(inner[k]);
        }
        return grouping;
    }

    // ── Bind Prototype Methods ───────────────────────────────────────────────
    CSSGroupingRule.prototype.insertRule = function(ruleText, index) {
        index = index === undefined ? 0 : Number(index);
        if (index < 0 || index > this._rules.length) {
            throw new DOMException("Failed to execute 'insertRule': index " + index + " is out of range", "IndexSizeError");
        }
        var parsed = _parseCssStatements(String(ruleText), this, this.parentStyleSheet || this);
        if (parsed.length === 0) {
            throw new DOMException("Failed to execute 'insertRule': no rule parsed", "SyntaxError");
        }
        var newRule = parsed[0];
        newRule.parentRule = this;
        newRule.parentStyleSheet = this.parentStyleSheet || (this instanceof CSSStyleSheet ? this : null);
        this._rules.splice(index, 0, newRule);
        this._notifyMutate();
        return index;
    };

    CSSGroupingRule.prototype.deleteRule = function(index) {
        index = Number(index);
        if (index < 0 || index >= this._rules.length) {
            throw new DOMException("Failed to execute 'deleteRule': index " + index + " is out of range", "IndexSizeError");
        }
        this._rules.splice(index, 1);
        this._notifyMutate();
    };

    CSSStyleRule.prototype._parseBody = function(bodyText) {
        var parsed = _parseCssStatements(bodyText, this, this.parentStyleSheet);
        for (var i = 0; i < parsed.length; i++) {
            var r = parsed[i];
            if (i === 0 && (r instanceof CSSNestedDeclarations)) {
                var p = r._style._props || {};
                for (var k in p) {
                    if (Object.prototype.hasOwnProperty.call(p, k)) {
                        this._style.setProperty(k, p[k]);
                    }
                }
            } else {
                this._rules.push(r);
            }
        }
    };

    CSSStyleSheet.prototype.replaceSync = function(text) {
        this._rules = _parseCssStatements(String(text), null, this);
        this._onMutate();
        return undefined;
    };

    CSSStyleSheet.prototype.replace = function(text) {
        var self = this;
        return new Promise(function(resolve) {
            self.replaceSync(text);
            resolve(self);
        });
    };

    CSSStyleSheet.prototype.insertRule = function(rule, index) {
        index = index === undefined ? 0 : Number(index);
        var rStr = String(rule).trim();

        if (rStr.startsWith('@layer')) {
            var afterLayer = rStr.slice(6).trim();
            if (afterLayer.endsWith(';')) {
                var statementContent = afterLayer.slice(0, -1).trim();
                if (!statementContent) {
                    throw new DOMException("Failed to execute 'insertRule' on 'CSSStyleSheet': Invalid @layer statement", "SyntaxError");
                }
                var names = statementContent.split(',');
                for (var i = 0; i < names.length; i++) {
                    var n = names[i].trim();
                    if (!n) throw new DOMException("Failed to execute 'insertRule': empty layer name", "SyntaxError");
                    var parts = n.split('.');
                    for (var j = 0; j < parts.length; j++) {
                        var p = parts[j].trim();
                        if (p !== parts[j] || !/^[a-zA-Z_-][a-zA-Z0-9_-]*$/.test(p)) {
                            throw new DOMException("Failed to execute 'insertRule': invalid layer name", "SyntaxError");
                        }
                    }
                }
            } else if (afterLayer.indexOf('{') !== -1) {
                var header = afterLayer.slice(0, afterLayer.indexOf('{')).trim();
                if (header.indexOf(',') !== -1) {
                    throw new DOMException("Failed to execute 'insertRule': @layer block cannot have multiple layer names", "SyntaxError");
                }
                if (header) {
                    var parts = header.split('.');
                    for (var j = 0; j < parts.length; j++) {
                        var p = parts[j].trim();
                        if (p !== parts[j] || !/^[a-zA-Z_-][a-zA-Z0-9_-]*$/.test(p)) {
                            throw new DOMException("Failed to execute 'insertRule': invalid layer name", "SyntaxError");
                        }
                    }
                }
            }
        }

        if (index < 0 || index > this._rules.length) {
            throw new DOMException("Failed to execute 'insertRule': index out of range", "IndexSizeError");
        }

        var parsed = _parseCssStatements(rStr, null, this);
        if (parsed.length === 0) {
            throw new DOMException("Failed to execute 'insertRule': invalid rule syntax", "SyntaxError");
        }
        var newRule = parsed[0];
        newRule.parentStyleSheet = this;
        this._rules.splice(index, 0, newRule);
        this._onMutate();
        return index;
    };

    CSSStyleSheet.prototype.deleteRule = function(index) {
        index = Number(index);
        if (index < 0 || index >= this._rules.length) {
            throw new DOMException("Index out of range", "IndexSizeError");
        }
        this._rules.splice(index, 1);
        this._onMutate();
    };

    // ── Document.prototype.styleSheets ───────────────────────────────────────
    if (typeof globalThis.Document !== 'undefined') {
        Object.defineProperty(globalThis.Document.prototype, 'styleSheets', {
            get: function() {
                var styles = this.querySelectorAll ? this.querySelectorAll('style, link[rel="stylesheet"]') : [];
                var sheets = [];
                for (var i = 0; i < styles.length; i++) {
                    if (styles[i].sheet) sheets.push(styles[i].sheet);
                }
                sheets.item = function(idx) { return sheets[idx] || null; };
                return sheets;
            },
            configurable: true,
            enumerable: true
        });
    }

    // ── Export Globals ───────────────────────────────────────────────────────
    globalThis.CSSRule = CSSRule;
    globalThis.CSSRuleList = CSSRuleList;
    globalThis.CSSGroupingRule = CSSGroupingRule;
    globalThis.CSSConditionRule = CSSConditionRule;
    globalThis.CSSMediaRule = CSSMediaRule;
    globalThis.CSSSupportsRule = CSSSupportsRule;
    globalThis.CSSLayerBlockRule = CSSLayerBlockRule;
    globalThis.CSSLayerStatementRule = CSSLayerStatementRule;
    globalThis.CSSScopeRule = CSSScopeRule;
    globalThis.CSSStyleRule = CSSStyleRule;
    globalThis.CSSNestedDeclarations = CSSNestedDeclarations;
    globalThis.CSSPositionTryRule = CSSPositionTryRule;
    globalThis.CSSPageRule = CSSPageRule;
    globalThis.CSSKeyframesRule = CSSKeyframesRule;
    globalThis.CSSKeyframeRule = CSSKeyframeRule;
    globalThis.CSSImportRule = CSSImportRule;
    globalThis.CSSStyleSheet = CSSStyleSheet;

    if (typeof window !== 'undefined') {
        window.CSSRule = CSSRule;
        window.CSSRuleList = CSSRuleList;
        window.CSSGroupingRule = CSSGroupingRule;
        window.CSSConditionRule = CSSConditionRule;
        window.CSSMediaRule = CSSMediaRule;
        window.CSSSupportsRule = CSSSupportsRule;
        window.CSSLayerBlockRule = CSSLayerBlockRule;
        window.CSSLayerStatementRule = CSSLayerStatementRule;
        window.CSSScopeRule = CSSScopeRule;
        window.CSSStyleRule = CSSStyleRule;
        window.CSSNestedDeclarations = CSSNestedDeclarations;
        window.CSSPositionTryRule = CSSPositionTryRule;
        window.CSSPageRule = CSSPageRule;
        window.CSSKeyframesRule = CSSKeyframesRule;
        window.CSSKeyframeRule = CSSKeyframeRule;
        window.CSSImportRule = CSSImportRule;
        window.CSSStyleSheet = CSSStyleSheet;
    }
