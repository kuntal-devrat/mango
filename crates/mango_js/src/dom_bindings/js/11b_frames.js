    // ── XML Parsing into Document ──────────────────────────────────────────────
    function _parseXmlToDoc(str, doc, isXhtml) {
        if (str.indexOf('\uFFFD') !== -1 || str.indexOf('\uFFFE') !== -1) {
            var badIdx = str.indexOf('\uFFFD');
            if (badIdx === -1) badIdx = str.indexOf('\uFFFE');
            str = str.substring(0, badIdx);
            if (doc) doc._hasWellFormednessError = true;
        }
        var re = /<!--[\s\S]*?-->|<!\[CDATA\[[\s\S]*?\]\]>|<!DOCTYPE[^>]*>|<\?[\s\S]*?\?>|<(\/)?([a-zA-Z0-9_\-:]+)((?:\s+[^=>\/\s]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*)\s*(\/?)>|([^<]+)/g;
        var stack = [doc];
        var match;
        var htmlNs = "http://www.w3.org/1999/xhtml";
        while ((match = re.exec(str)) !== null) {
            var token = match[0];
            if (token.indexOf('<!--') === 0) {
                var cText = token.substring(4, token.length - 3);
                var comment = doc.createComment ? doc.createComment(cText) : null;
                if (comment && stack.length > 0) stack[stack.length - 1].appendChild(comment);
            } else if (token.indexOf('<![CDATA[') === 0) {
                var cdText = token.substring(9, token.length - 3);
                var cdata = doc.createTextNode ? doc.createTextNode(cdText) : null;
                if (cdata && stack.length > 0) stack[stack.length - 1].appendChild(cdata);
            } else if (match[2]) {
                var isClose = !!match[1];
                var tagName = match[2];
                var attrsStr = match[3];
                var isSelfClose = !!match[4];
                if (isClose) {
                    var foundIdx = -1;
                    for (var s = stack.length - 1; s >= 1; s--) {
                        if (stack[s].tagName && stack[s].tagName.toLowerCase() === tagName.toLowerCase()) {
                            foundIdx = s;
                            break;
                        }
                    }
                    if (foundIdx === -1) {
                        if (doc) doc._hasWellFormednessError = true;
                        break;
                    }
                    stack.splice(foundIdx, stack.length - foundIdx);
                } else {
                    var currentNs = null;
                    if (attrsStr && attrsStr.indexOf('xmlns') !== -1) {
                        var nsMatch = /xmlns(?::[a-zA-Z0-9_\-]+)?\s*=\s*(?:"([^"]*)"|'([^']*)')/.exec(attrsStr);
                        if (nsMatch) currentNs = nsMatch[1] !== undefined ? nsMatch[1] : nsMatch[2];
                    }
                    if (!currentNs && stack.length > 1 && stack[stack.length - 1].namespaceURI) {
                        currentNs = stack[stack.length - 1].namespaceURI;
                    }
                    if (!currentNs && (doc && (doc.contentType === 'image/svg+xml' || (doc.documentElement && doc.documentElement.namespaceURI === 'http://www.w3.org/2000/svg')))) {
                        currentNs = "http://www.w3.org/2000/svg";
                    }
                    var el;
                    if (currentNs && doc && doc.createElementNS) {
                        el = doc.createElementNS(currentNs, tagName);
                    } else if (isXhtml || (doc && doc.contentType === 'application/xhtml+xml')) {
                        el = (doc && doc.createElementNS) ? doc.createElementNS(htmlNs, tagName) : doc.createElement(tagName);
                    } else {
                        el = doc.createElement(tagName);
                    }
                    if (attrsStr) {
                        var attrRe = /([^\s=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
                        var am;
                        while ((am = attrRe.exec(attrsStr)) !== null) {
                            var aname = am[1];
                            var aval = am[2] !== undefined ? am[2] : (am[3] !== undefined ? am[3] : (am[4] || ""));
                            try { el.setAttribute(aname, aval); } catch (e) {}
                        }
                    }
                    if (stack.length > 0) {
                        stack[stack.length - 1].appendChild(el);
                    }
                    if (!doc.documentElement) {
                        doc.documentElement = el;
                    }
                    var tagLower = tagName.toLowerCase();
                    if (tagLower === 'head' && !doc.head) doc.head = el;
                    if (tagLower === 'body' && !doc.body) doc.body = el;
                    if (!isSelfClose) {
                        stack.push(el);
                    }
                }
            } else if (match[5]) {
                var text = match[5];
                if (stack.length > 1) {
                    var tn = doc.createTextNode ? doc.createTextNode(text) : null;
                    if (tn) stack[stack.length - 1].appendChild(tn);
                }
            }
        }
        return doc;
    }
    globalThis._parseXmlToDoc = _parseXmlToDoc;

    // ── Frame and Object subresource documents & Window environments ────────────
    function _getIframeContentWindow(iframe) {
        var tag = (iframe && iframe.tagName) ? iframe.tagName.toLowerCase() : '';
        if (!iframe || (tag !== 'iframe' && tag !== 'object')) return null;
        var owner = iframe.ownerDocument || (typeof document !== 'undefined' ? document : null);
        if (!owner || !owner.defaultView) return null;
        if (!iframe._contentWindow) {
            var win = Object.create(globalThis);
            win.frameElement = iframe;
            win.window = win;
            win.self = win;
            win.top = (typeof globalThis !== 'undefined' && globalThis.window) ? globalThis.window : win;
            win.parent = (typeof globalThis !== 'undefined' && globalThis.window) ? globalThis.window : win;
            win.customElements = (typeof globalThis.CustomElementRegistry === 'function') ? new globalThis.CustomElementRegistry() : null;
            win.postMessage = function(msg, targetOrigin) {};
            win.addEventListener = function(type, listener, opt) {
                var d = _getIframeContentDocument(iframe);
                if (d && d.addEventListener) d.addEventListener(type, listener, opt);
            };
            win.removeEventListener = function(type, listener, opt) {
                var d = _getIframeContentDocument(iframe);
                if (d && d.removeEventListener) d.removeEventListener(type, listener, opt);
            };
            win.dispatchEvent = function(evt) {
                var d = _getIframeContentDocument(iframe);
                return d && d.dispatchEvent ? d.dispatchEvent(evt) : false;
            };
            win.notify = function(id) {
                if (typeof globalThis.notify === 'function') {
                    try { globalThis.notify(id); } catch(e) {}
                }
                if (globalThis.notifications) {
                    globalThis.notifications[id] = true;
                }
            };
            Object.defineProperty(win, 'document', {
                get: function() { return _getIframeContentDocument(iframe); },
                configurable: true,
                enumerable: true
            });
            if (globalThis.Text && globalThis.CharacterData) {
                win.Text = function(data) {
                    var t = Object.create(win.Text.prototype);
                    t.nodeType = 3;
                    t.nodeName = "#text";
                    t.ownerDocument = win.document;
                    t.data = data === undefined ? "" : String(data);
                    return t;
                };
                win.Text.prototype = Object.create(globalThis.CharacterData.prototype);
                win.Text.prototype.constructor = win.Text;
                win.Text.prototype.splitText = globalThis.Text.prototype.splitText;
                var wtDesc = Object.getOwnPropertyDescriptor(globalThis.Text.prototype, 'wholeText');
                if (wtDesc) Object.defineProperty(win.Text.prototype, 'wholeText', wtDesc);
            }
            if (globalThis.Comment && globalThis.CharacterData) {
                win.Comment = function(data) {
                    var c = Object.create(win.Comment.prototype);
                    c.nodeType = 8;
                    c.nodeName = "#comment";
                    c.ownerDocument = win.document;
                    c.data = data === undefined ? "" : String(data);
                    return c;
                };
                win.Comment.prototype = Object.create(globalThis.CharacterData.prototype);
                win.Comment.prototype.constructor = win.Comment;
            }
            iframe._contentWindow = win;
        }
        return iframe._contentWindow;
    }
    globalThis._getIframeContentWindow = _getIframeContentWindow;

    function _getIframeContentDocument(iframe) {
        var tag = (iframe && iframe.tagName) ? iframe.tagName.toLowerCase() : '';
        if (!iframe || (tag !== 'iframe' && tag !== 'object')) return null;
        var owner = iframe.ownerDocument || (typeof document !== 'undefined' ? document : null);
        if (!owner || !owner.defaultView) return null;

        if (!iframe._contentDocument) {
            var src = iframe.getAttribute ? (iframe.getAttribute('src') || iframe.getAttribute('data')) : null;
            if (!src && (iframe.src || iframe.data)) {
                src = iframe.src || iframe.data;
            }
            var doc = null;
            var mime = 'text/html';
            var s = String(src || '').toLowerCase();
            var qMatch = /[?&]type=([^&]+)&subtype=([^&]+)/.exec(s);
            if (qMatch) {
                mime = decodeURIComponent(qMatch[1]) + '/' + decodeURIComponent(qMatch[2]);
            } else if (s.indexOf('.bmp') !== -1) mime = 'image/bmp';
            else if (s.indexOf('.png') !== -1) mime = 'image/png';
            else if (s.indexOf('.gif') !== -1) mime = 'image/gif';
            else if (s.indexOf('.jpg') !== -1 || s.indexOf('.jpeg') !== -1) mime = 'image/jpeg';
            else if (s.indexOf('.webp') !== -1) mime = 'image/webp';
            else if (s.indexOf('.svg') !== -1) mime = 'image/svg+xml';
            else if (s.indexOf('.css') !== -1) mime = 'text/css';
            else if (s.indexOf('.txt') !== -1) mime = 'text/plain';
            else if (s.indexOf('.xhtml') !== -1 || /\bxhtml\.\d+/.test(s)) mime = 'application/xhtml+xml';
            else if (s.indexOf('.html') !== -1 || s.indexOf('.htm') !== -1) mime = 'text/html';
            else if (s.indexOf('.xml') !== -1) mime = 'application/xml';

            var jsRes = undefined;
            var isJsUri = src && String(src).toLowerCase().indexOf('javascript:') === 0;
            if (isJsUri) {
                try { jsRes = (1, eval)(String(src).substring(11)); } catch(e) {}
            }

            var text = null;
            if (src && globalThis.__subresourceMap && globalThis.__subresourceMap[src]) {
                text = globalThis.__subresourceMap[src];
            } else if (src && typeof _mangoFetch === 'function') {
                try {
                    var fullUrl = (typeof URL === 'function' && typeof location !== 'undefined' && location.href)
                        ? new URL(src, location.href).href
                        : src;
                    var origin = (typeof location !== 'undefined' && location.origin) ? location.origin : '';
                    var respStr = _mangoFetch(fullUrl, 'GET', '{}', null, origin, 'cors', false);
                    var resp = JSON.parse(respStr);
                    if (resp && resp.ok && resp.body) {
                        text = resp.body;
                        if (globalThis.__subresourceMap) {
                            globalThis.__subresourceMap[src] = text;
                        }
                    }
                } catch(e) {}
            }

            if (mime === 'text/plain') {
                doc = (typeof document !== 'undefined' && document.implementation && typeof document.implementation.createHTMLDocument === 'function')
                    ? document.implementation.createHTMLDocument('')
                    : null;
                if (doc && doc.body) {
                    var pre = doc.createElement('pre');
                    pre.textContent = (text != null) ? text : '';
                    doc.body.appendChild(pre);
                }
            } else if (mime.indexOf('image/') === 0 && mime !== 'image/svg+xml') {
                doc = (typeof document !== 'undefined' && document.implementation && typeof document.implementation.createHTMLDocument === 'function')
                    ? document.implementation.createHTMLDocument('')
                    : null;
                if (doc && doc.body) {
                    var img = doc.createElement('img');
                    if (src) img.src = src;
                    doc.body.appendChild(img);
                }
            } else if (text && typeof DOMParser !== 'undefined') {
                doc = new DOMParser().parseFromString(text, mime);
            }

            if (!doc) {
                doc = (typeof document !== 'undefined' && document.implementation && typeof document.implementation.createHTMLDocument === 'function')
                    ? document.implementation.createHTMLDocument('')
                    : null;
            }
            if (doc) {
                if (typeof jsRes === 'string') {
                    if (doc.body) doc.body.textContent = jsRes;
                    else if (doc.documentElement) doc.documentElement.textContent = jsRes;
                }
                doc.readyState = 'complete';
                var win = _getIframeContentWindow(iframe);
                doc.defaultView = win;
                doc._creationRealm = win;
                if (doc.implementation) doc.implementation._ownerDocument = doc;
                doc.contentType = mime;
                doc._contentType = mime;
                if (typeof globalThis.Document !== 'undefined') {
                    try { Object.setPrototypeOf(doc, globalThis.Document.prototype); } catch(e) {}
                }
                iframe._contentDocument = doc;

                // For XHTML documents (e.g. xhtml.1), run scripts inside iframe if well-formed and valid HTML namespace
                if (mime === 'application/xhtml+xml' && text && !doc._hasWellFormednessError) {
                    var rootNs = doc.documentElement ? doc.documentElement.namespaceURI : null;
                    if (rootNs === 'http://www.w3.org/1999/xhtml') {
                        var scripts = doc.getElementsByTagName ? doc.getElementsByTagName('script') : [];
                        for (var sc = 0; sc < scripts.length; sc++) {
                            var scr = scripts[sc];
                            var scrNs = scr.namespaceURI || (scr.parentNode ? scr.parentNode.namespaceURI : null);
                            if (scrNs && scrNs !== 'http://www.w3.org/1999/xhtml') continue;
                            var code = scr.textContent || '';
                            if (code && typeof code === 'string') {
                                try {
                                    var fn = new Function('parent', 'window', 'document', code);
                                    fn.call(win, win.parent, win, doc);
                                } catch(e) {}
                            }
                        }
                    }
                }
            }
        }
        return iframe._contentDocument || null;
    }
    globalThis._getIframeContentDocument = _getIframeContentDocument;

    function _triggerFrameLoad(self) {
        if (!self) return;
        if (typeof _getIframeContentDocument === 'function') {
            _getIframeContentDocument(self);
        }
        var docScope = self.ownerDocument || (typeof document !== 'undefined' ? document : null);
        var winScope = (docScope && docScope.defaultView) || (typeof window !== 'undefined' ? window : globalThis);
        var onloadAttr = self.getAttribute ? self.getAttribute('onload') : null;
        if (onloadAttr && typeof onloadAttr === 'string') {
            try {
                var fn = new Function('event', 'document', 'window', onloadAttr);
                fn.call(self, { type: 'load', target: self }, docScope, winScope);
            } catch(e) {}
        }
        var evt = (typeof Event === 'function') ? new Event('load') : { type: 'load', target: self };
        if (typeof self.onload === 'function') {
            try { self.onload.call(self, evt); } catch(e) {}
        }
        if (typeof self.dispatchEvent === 'function') {
            try { self.dispatchEvent(evt); } catch(e) {}
        }
    }
    globalThis._triggerFrameLoad = _triggerFrameLoad;

    // ── Document open/write/close full tree rebuild ─────────────────────────────
    function _resetDocumentWithHtml(doc, text) {
        if (!doc) return;
        while (doc.firstChild) {
            doc.removeChild(doc.firstChild);
        }

        var markup = String(text || '');
        var dtMatch = /<!DOCTYPE\s+([^\s>]+)(?:\s+(PUBLIC|SYSTEM)\s+(?:"([^"]*)"|'([^']*)'))?(?:\s+(?:"([^"]*)"|'([^']*)'))?[^>]*>/i.exec(markup);
        if (dtMatch) {
            var dtName = dtMatch[1] || 'html';
            var pubId = '';
            var sysId = '';
            if (dtMatch[2] && dtMatch[2].toUpperCase() === 'PUBLIC') {
                pubId = dtMatch[3] !== undefined ? dtMatch[3] : (dtMatch[4] || '');
                sysId = dtMatch[5] !== undefined ? dtMatch[5] : (dtMatch[6] || '');
            } else if (dtMatch[2] && dtMatch[2].toUpperCase() === 'SYSTEM') {
                sysId = dtMatch[3] !== undefined ? dtMatch[3] : (dtMatch[4] || '');
            }
            var dt = (doc.implementation && typeof doc.implementation.createDocumentType === 'function')
                ? doc.implementation.createDocumentType(dtName, pubId, sysId)
                : null;
            if (dt) {
                dt._ownerDocument = doc;
                dt.ownerDocument = doc;
                dt._parentNode = doc;
                dt.parentNode = doc;
                doc._doctype = dt;
                doc.doctype = dt;
                doc.appendChild(dt);
            }
        }

        var htmlEl = doc.createElement('html');
        doc.appendChild(htmlEl);
        doc.documentElement = htmlEl;

        var headEl = doc.createElement('head');
        var bodyEl = doc.createElement('body');
        htmlEl.appendChild(headEl);
        htmlEl.appendChild(bodyEl);
        doc.head = headEl;
        doc.body = bodyEl;

        var cleaned = markup.replace(/<!DOCTYPE[^>]*>/i, '').trim();
        var headMatch = /<head[^>]*>([\s\S]*?)<\/head>/i.exec(cleaned);
        var bodyMatch = /<body[^>]*>([\s\S]*?)<\/body>/i.exec(cleaned);

        var bIdx = cleaned.search(/<body[^>]*>/i);
        if (bIdx !== -1) {
            var beforeBody = cleaned.substring(0, bIdx);
            var afterBody = cleaned.substring(bIdx).replace(/^<body[^>]*>/i, '').replace(/<\/body>[\s\S]*$/i, '');
            var hMatch = /<head[^>]*>([\s\S]*?)<\/head>/i.exec(beforeBody);
            var headContent = hMatch ? hMatch[1] : beforeBody.replace(/^<head[^>]*>/i, '');
            headEl.innerHTML = headContent;
            bodyEl.innerHTML = afterBody;
        } else if (headMatch) {
            headEl.innerHTML = headMatch[1];
            var afterH = cleaned.substring(cleaned.indexOf('</head>') + 7);
            bodyEl.innerHTML = afterH.replace(/<\/?body[^>]*>/gi, '');
        } else {
            var temp = doc.createElement('div');
            temp.innerHTML = cleaned;
            var headTags = { 'title': true, 'style': true, 'meta': true, 'link': true, 'base': true };
            while (temp.firstChild) {
                var child = temp.firstChild;
                temp.removeChild(child);
                var tname = (child.tagName || '').toLowerCase();
                if (headTags[tname]) {
                    headEl.appendChild(child);
                } else {
                    bodyEl.appendChild(child);
                }
            }
        }

        function setDocOnSubtree(n) {
            if (!n) return;
            n._ownerDocument = doc;
            n.ownerDocument = doc;
            if (n._cachedChildNodes) delete n._cachedChildNodes;
            var kids = n.childNodes || n._childNodes || [];
            for (var k = 0; k < kids.length; k++) {
                var c = kids[k];
                if (c) {
                    c._parentNode = n;
                    setDocOnSubtree(c);
                }
            }
        }
        setDocOnSubtree(htmlEl);
    }
    globalThis._resetDocumentWithHtml = _resetDocumentWithHtml;
