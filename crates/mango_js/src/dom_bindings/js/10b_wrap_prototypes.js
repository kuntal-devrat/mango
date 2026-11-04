    function _ensureWrapperPrototype(targetEl) {
        if (!targetEl || typeof targetEl !== 'object') return;
        var curProto = Object.getPrototypeOf(targetEl);
        if (curProto && curProto !== Object.prototype) return;
        var tag = targetEl.localName ? targetEl.localName.toLowerCase() : (targetEl.tagName ? targetEl.tagName.toLowerCase() : '');
        if (tag.indexOf(':') !== -1) tag = tag.split(':')[1];
        var nt = targetEl.nodeType;
        var pId = (typeof _getParentNode === 'function' && targetEl._nodeId !== undefined) ? _getParentNode(targetEl._nodeId) : null;
        var pNode = (pId !== null && typeof _getCachedWrapper === 'function') ? _getCachedWrapper(pId) : (targetEl._parentNode || null);
        var pDoc = targetEl._ownerDocument || targetEl.ownerDocument || (pNode ? (pNode._ownerDocument || pNode.ownerDocument) : null);
        if (!targetEl._creationRealm) {
            targetEl._creationRealm = (pNode && pNode._lastInnerHTMLRealm)
                || (targetEl._ownerDocument && (targetEl._ownerDocument._creationRealm || targetEl._ownerDocument.defaultView))
                || (pDoc && (pDoc._creationRealm || pDoc.defaultView))
                || (typeof globalThis !== 'undefined' ? globalThis : null);
        }
        var realm = targetEl._creationRealm || (typeof globalThis !== 'undefined' ? globalThis : null);
        if (realm) {
            if (nt === 11 && (realm.DocumentFragment || globalThis.DocumentFragment)) {
                Object.setPrototypeOf(targetEl, (realm.DocumentFragment || globalThis.DocumentFragment).prototype);
            } else if (nt === 9 && (realm.Document || globalThis.Document)) {
                Object.setPrototypeOf(targetEl, (realm.Document || globalThis.Document).prototype);
            } else if (nt === 3 && (realm.Text || globalThis.Text)) {
                Object.setPrototypeOf(targetEl, (realm.Text || globalThis.Text).prototype);
            } else if (nt === 8 && (realm.Comment || globalThis.Comment)) {
                if (targetEl._isPI && (realm.ProcessingInstruction || globalThis.ProcessingInstruction)) {
                    Object.setPrototypeOf(targetEl, (realm.ProcessingInstruction || globalThis.ProcessingInstruction).prototype);
                } else {
                    Object.setPrototypeOf(targetEl, (realm.Comment || globalThis.Comment).prototype);
                }
            } else if (nt === 7 && (realm.ProcessingInstruction || globalThis.ProcessingInstruction)) {
                Object.setPrototypeOf(targetEl, (realm.ProcessingInstruction || globalThis.ProcessingInstruction).prototype);
            } else if (nt === 10 && (realm.DocumentType || globalThis.DocumentType)) {
                delete targetEl.name;
                Object.setPrototypeOf(targetEl, (realm.DocumentType || globalThis.DocumentType).prototype);
            } else {
                var customCtor = (globalThis.customElements && globalThis.customElements.get) ? globalThis.customElements.get(tag) : null;
                if (customCtor && customCtor.prototype) {
                    Object.setPrototypeOf(targetEl, customCtor.prototype);
                    if (!customCtor.prototype.hasOwnProperty('constructor')) {
                        try { customCtor.prototype.constructor = customCtor; } catch(e) {}
                    }
                } else {
                    var _tagToCtorName = {
                        'svg': 'SVGSVGElement', 'use': 'SVGUseElement', 'g': 'SVGGElement', 'path': 'SVGPathElement', 'circle': 'SVGCircleElement', 'rect': 'SVGRectElement', 'text': 'SVGTextElement', 'tspan': 'SVGTextContentElement',
                        'template': 'HTMLTemplateElement', 'slot': 'HTMLSlotElement', 'dialog': 'HTMLDialogElement',
                        'progress': 'HTMLProgressElement', 'meter': 'HTMLMeterElement', 'output': 'HTMLOutputElement',
                        'datalist': 'HTMLDataListElement', 'map': 'HTMLMapElement', 'area': 'HTMLAreaElement',
                        'object': 'HTMLObjectElement', 'embed': 'HTMLEmbedElement', 'ruby': 'HTMLRubyElement',
                        'img': 'HTMLImageElement', 'iframe': 'HTMLIFrameElement', 'canvas': 'HTMLCanvasElement',
                        'video': 'HTMLVideoElement', 'audio': 'HTMLAudioElement', 'input': 'HTMLInputElement',
                        'select': 'HTMLSelectElement', 'option': 'HTMLOptionElement', 'optgroup': 'HTMLOptGroupElement',
                        'form': 'HTMLFormElement', 'fieldset': 'HTMLFieldSetElement', 'legend': 'HTMLLegendElement',
                        'textarea': 'HTMLTextAreaElement', 'button': 'HTMLButtonElement', 'a': 'HTMLAnchorElement',
                        'div': 'HTMLDivElement', 'span': 'HTMLSpanElement', 'p': 'HTMLParagraphElement',
                        'h1': 'HTMLHeadingElement', 'h2': 'HTMLHeadingElement', 'h3': 'HTMLHeadingElement',
                        'h4': 'HTMLHeadingElement', 'h5': 'HTMLHeadingElement', 'h6': 'HTMLHeadingElement',
                        'ul': 'HTMLUListElement', 'ol': 'HTMLOListElement', 'li': 'HTMLLIElement',
                        'dl': 'HTMLDListElement', 'table': 'HTMLTableElement', 'tr': 'HTMLTableRowElement',
                        'td': 'HTMLTableCellElement', 'th': 'HTMLTableCellElement',
                        'tbody': 'HTMLTableSectionElement', 'thead': 'HTMLTableSectionElement', 'tfoot': 'HTMLTableSectionElement',
                        'col': 'HTMLTableColElement', 'colgroup': 'HTMLTableColElement', 'caption': 'HTMLTableCaptionElement',
                        'body': 'HTMLBodyElement', 'head': 'HTMLHeadElement', 'html': 'HTMLHtmlElement',
                        'script': 'HTMLScriptElement', 'style': 'HTMLStyleElement', 'link': 'HTMLLinkElement',
                        'meta': 'HTMLMetaElement', 'title': 'HTMLTitleElement', 'label': 'HTMLLabelElement',
                        'br': 'HTMLBRElement', 'hr': 'HTMLHRElement', 'pre': 'HTMLPreElement',
                        'details': 'HTMLDetailsElement', 'summary': 'HTMLSummaryElement', 'time': 'HTMLTimeElement',
                        'data': 'HTMLDataElement', 'source': 'HTMLSourceElement', 'picture': 'HTMLPictureElement',
                        'track': 'HTMLTrackElement', 'base': 'HTMLBaseElement', 'blockquote': 'HTMLQuoteElement',
                        'q': 'HTMLQuoteElement', 'del': 'HTMLModElement', 'ins': 'HTMLModElement',
                        'dir': 'HTMLDirectoryElement', 'font': 'HTMLFontElement', 'frame': 'HTMLFrameElement',
                        'frameset': 'HTMLFrameSetElement', 'param': 'HTMLParamElement'
                    };
                    var ctorName = _tagToCtorName[tag];
                    var ctor = ctorName ? globalThis[ctorName] : null;
                    if (ctor && ctor.prototype) {
                        Object.setPrototypeOf(targetEl, ctor.prototype);
                    } else {
                        var customCtor2 = (globalThis.customElements && globalThis.customElements.get) ? globalThis.customElements.get(tag) : null;
                        if (customCtor2 && typeof customCtor2 === 'function') {
                            if (globalThis._upgradeCustomElement && !targetEl._isCustomConstructed) {
                                globalThis._upgradeCustomElement(targetEl, customCtor2);
                            } else if (customCtor2.prototype) {
                                Object.setPrototypeOf(targetEl, customCtor2.prototype);
                            }
                        } else if (tag.indexOf('-') === -1 && globalThis.HTMLUnknownElement) {
                            Object.setPrototypeOf(targetEl, globalThis.HTMLUnknownElement.prototype);
                        } else if (globalThis.HTMLElement) {
                            Object.setPrototypeOf(targetEl, globalThis.HTMLElement.prototype);
                        }
                    }
                }
            }
        }
    }
    globalThis._ensureWrapperPrototype = _ensureWrapperPrototype;

    function _upgradeAllCachedWrappers() {
        if (typeof _wrapperCache === 'object' && _wrapperCache) {
            for (var k in _wrapperCache) {
                var entry = _wrapperCache[k];
                if (!entry) continue;
                var item = entry.ref ? (entry.ref.deref ? entry.ref.deref() : null) : entry.value;
                if (item) {
                    _ensureWrapperPrototype(item);
                }
            }
        }
    }
    globalThis._upgradeAllCachedWrappers = _upgradeAllCachedWrappers;
