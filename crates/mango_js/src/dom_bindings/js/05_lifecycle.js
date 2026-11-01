    // Per-node wrapper cache. DOM element identity requires that wrapping the
    // same node twice yields the same object (`document.querySelector('#a') ===
    // document.querySelector('#a')`). A strong cache would keep every wrapper
    // (and therefore every detached DOM node) alive for the lifetime of the
    // document, so store weak references and use a FinalizationRegistry to tell
    // Rust when the last wrapper for a node has been collected. Detached nodes
    // can then be recycled safely.
    var _wrapperCache = {};
    var _wrapperRegistry = (typeof FinalizationRegistry === 'function')
        ? new FinalizationRegistry(function(held) {
            var entry = _wrapperCache[held.id];
            if (entry && entry.token === held.token) {
                delete _wrapperCache[held.id];
            }
            if (typeof _releaseWrapper === 'function') {
                _releaseWrapper(held.id);
            }
        })
        : null;

    function _getCachedWrapper(nodeId) {
        var entry = _wrapperCache[nodeId];
        if (!entry) return null;
        if (entry.ref) {
            var obj = entry.ref.deref();
            if (obj) return obj;
            delete _wrapperCache[nodeId];
            return null;
        }
        return entry.value || null;
    }

    function _storeWrapper(nodeId, el) {
        if (typeof WeakRef === 'function' && _wrapperRegistry) {
            var token = {};
            _wrapperCache[nodeId] = { ref: new WeakRef(el), token: token };
            _wrapperRegistry.register(el, { id: nodeId, token: token }, token);
            if (typeof _retainWrapper === 'function') _retainWrapper(nodeId);
        } else {
            // Very old V8 builds without WeakRef keep the legacy strong cache.
            _wrapperCache[nodeId] = { value: el };
        }
    }

    function _invokeConnectedCallback(node, visited) {
        if (!node || typeof node !== 'object' || node.nodeType !== 1) return;
        if (!visited) visited = new Set();
        if (visited.has(node)) return;
        visited.add(node);

        if (typeof node.connectedCallback === 'function' && !node._isCustomConnected) {
            node._isCustomConnected = true;
            try {
                node.connectedCallback();
            } catch(e) {
                if (typeof console !== 'undefined' && console.warn) {
                    console.warn("[_invokeConnectedCallback connectedCallback error on " + (node.tagName||"") + "]:", (e && e.stack) ? e.stack : e);
                }
            }
        }
        if (typeof node.attached === 'function' && !node._isAttachedCalled) {
            node._isAttachedCalled = true;
            try {
                node.attached();
            } catch(e) {
                if (typeof console !== 'undefined' && console.warn) {
                    console.warn("[_invokeConnectedCallback attached error on " + (node.tagName||"") + "]:", (e && e.stack) ? e.stack : e);
                }
            }
        }
        if (node.usePatchedLifecycles) {
            node.lifecycleIsConnected = true;
        }
        var ctrl = node.polymerController;
        if (ctrl && typeof ctrl === 'object') {
            if (typeof ctrl.connectedCallback === 'function' && !ctrl._isCustomConnected) {
                ctrl._isCustomConnected = true;
                try {
                    ctrl.connectedCallback();
                } catch(e) {
                    if (typeof console !== 'undefined' && console.warn) {
                        console.warn("[_invokeConnectedCallback polymerController.connectedCallback error]:", e);
                    }
                }
            }
            if (typeof ctrl.attached === 'function' && !ctrl._isAttachedCalled) {
                ctrl._isAttachedCalled = true;
                try { ctrl.attached(); } catch(e) {}
            }
        }
        if (typeof node.dispatchEvent === 'function') {
            try {
                node.dispatchEvent((typeof globalThis.CustomEvent === 'function')
                    ? new globalThis.CustomEvent('attached', { bubbles: true, composed: true })
                    : { type: 'attached', target: node, bubbles: true, composed: true });
            } catch(e) {}
        }
        if (node && (node.id === 'page-manager' || (node.tagName && String(node.tagName).toLowerCase() === 'ytd-page-manager'))) {
            try {
                var appEl = (typeof document !== 'undefined' && document.querySelector) ? document.querySelector('ytd-app') : null;
                var appCtrl = appEl && (appEl.polymerController || appEl);
                if (appCtrl && appCtrl.pageManagerAttachedPromise && typeof appCtrl.pageManagerAttachedPromise.resolve === 'function') appCtrl.pageManagerAttachedPromise.resolve();
            } catch(e) {}
        }
        var nodeTag = (node.tagName || '').toLowerCase();
        if (nodeTag === 'script' && node.isConnected && !node._alreadyStarted && node._dynamicallyInserted) {
            node._alreadyStarted = true;
            var scriptType = (node.getAttribute && node.getAttribute('type')) ? String(node.getAttribute('type')).trim().toLowerCase() : '';
            var isJs = !scriptType || scriptType === 'text/javascript' || scriptType === 'application/javascript' || scriptType === 'module' || scriptType === 'text/ecmascript';
            var code = (node.text !== undefined && node.text !== null) ? node.text : (node.textContent || '');
            if (isJs && code.trim()) {
                var prevScript = (typeof document !== 'undefined') ? document.currentScript : null;
                if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(node);
                try {
                    (1, eval)(code);
                } catch(err) {
                    if (typeof console !== 'undefined' && console.error) console.error('[Script error]:', err);
                } finally {
                    if (typeof globalThis.__setCurrentScript === 'function') globalThis.__setCurrentScript(prevScript);
                }
            }
        }
        if ((nodeTag === 'script' || nodeTag === 'link' || nodeTag === 'style') && !node._hasDispatchedLoad) {
            node._hasDispatchedLoad = true;
            setTimeout(function() {
                try {
                    if (typeof node.onload === 'function') node.onload({ type: 'load', target: node });
                    if (typeof node.dispatchEvent === 'function') node.dispatchEvent(new Event('load'));
                } catch(e) {}
            }, 0);
        }
        var kids = node.childNodes;
        if (kids) {
            for (var i = 0; i < kids.length; i++) {
                _invokeConnectedCallback(kids[i], visited);
            }
        }
        var shadow = node.shadowRoot || node._shadowRoot || node._shadowRootInstance;
        if (shadow) {
            _invokeConnectedCallback(shadow, visited);
        } else if (node.root && node.root !== node && node.root.host === node) {
            _invokeConnectedCallback(node.root, visited);
        }
    }
    globalThis._invokeConnectedCallback = _invokeConnectedCallback;

    function _invokeDisconnectedCallback(node, visited) {
        if (!node || typeof node !== 'object') return;
        if (!visited) visited = new Set();
        if (visited.has(node)) return;
        visited.add(node);

        if (typeof node.disconnectedCallback === 'function' && node._isCustomConnected) { node._isCustomConnected = false; try { node.disconnectedCallback(); } catch(e) {} }
        if (typeof node.detached === 'function' && node._isAttachedCalled) { node._isAttachedCalled = false; try { node.detached(); } catch(e) {} }
        if (node.polymerController && typeof node.polymerController.detached === 'function' && node.polymerController._isAttachedCalled) { node.polymerController._isAttachedCalled = false; try { node.polymerController.detached(); } catch(e) {} }
        var kids = node.childNodes;
        if (kids) {
            for (var i = 0; i < kids.length; i++) {
                _invokeDisconnectedCallback(kids[i], visited);
            }
        }
        var disShadow = node.shadowRoot || node._shadowRoot || node._shadowRootInstance;
        if (disShadow) {
            _invokeDisconnectedCallback(disShadow, visited);
        } else if (node.root && node.root !== node && node.root.host === node) {
            _invokeDisconnectedCallback(node.root, visited);
        }
    }

    function _invokeAttributeChangedCallback(el, name, oldVal, newVal) {
        if (!el || typeof el !== 'object') return;
        if (oldVal === newVal) return;
        var tag = el.tagName ? String(el.tagName).toLowerCase() : '';
        var ctor = (typeof globalThis !== 'undefined' && globalThis.customElements && globalThis.customElements.get && tag)
            ? globalThis.customElements.get(tag)
            : null;
        ctor = ctor || el.constructor;
        var obs = (ctor && ctor.observedAttributes) || el.observedAttributes;
        if (Array.isArray(obs)) {
            var matches = false;
            for (var i = 0; i < obs.length; i++) {
                if (String(obs[i]).toLowerCase() === String(name).toLowerCase()) {
                    matches = true;
                    break;
                }
            }
            if (matches && typeof el.attributeChangedCallback === 'function') {
                try {
                    el.attributeChangedCallback(name, oldVal, newVal, null);
                } catch(e) {}
            }
        }
    }
