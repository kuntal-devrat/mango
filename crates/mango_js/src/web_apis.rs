//! Non-DOM Web API bindings for the Mango JS runtime.
//!
//! Registers native Rust implementations of:
//!   - `setTimeout` / `setInterval` / `clearTimeout` / `clearInterval`
//!   - `alert`
//!   - `_mangoNavigate` (internal navigation trigger)
//!   - `_mangoScheduleTimer` / `_mangoCancelTimer` (closure-preserving timer helpers)
//!   - `_mangoFetch` (synchronous HTTP bridge for the JS `fetch()` polyfill)
//!   - `_mangoLocalStorage*` (persistent localStorage native bridge)
//!
//! Then installs a large JS shim that builds the full browser global environment:
//! `window`, `location`, `navigator`, `screen`, `history`, `performance`,
//! `localStorage`/`sessionStorage`, `requestAnimationFrame`, `matchMedia`,
//! `getComputedStyle`, the DOM constructor hierarchy, event classes, observers,
//! `fetch()` with a real HTTP implementation, and `WebSocket` with graceful degradation.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};


use crate::event_loop::EventLoop;

// ── Shared handle types ───────────────────────────────────────────────────────

/// Shared mutable event loop accessible from both JS callbacks and the runtime.
pub type SharedEventLoop = Rc<RefCell<EventLoop>>;

/// Shared status text for `alert()` display.
pub type SharedStatusText = Rc<RefCell<String>>;

/// Shared pending navigation URL triggered by `location.href` assignment.
pub type SharedPendingNav = Rc<RefCell<Option<String>>>;

/// Represents a window opening request triggered by `window.open(url, target, features)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingWindowOpen {
    pub url: String,
    pub target: String,
    pub features: String,
}

/// Shared pending window open request.
pub type SharedPendingOpenWindow = Rc<RefCell<Option<PendingWindowOpen>>>;

/// Thread-safe localStorage store.  Keyed by string; values are UTF-8 strings.
/// `Arc<Mutex<_>>` so the browser can persist it to disk after each page unload.
pub type SharedLocalStorage = Arc<Mutex<HashMap<String, String>>>;

/// Thread-safe sessionStorage store (tab-scoped, in-memory). Keyed by string (`{origin}\x1f{key}`).
pub type SharedSessionStorage = Arc<Mutex<HashMap<String, String>>>;

/// Escapes a profile-field value so one entry always occupies one line.
pub fn escape_profile_field(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\t', "%09")
        .replace('\n', "%0A")
        .replace('\r', "%0D")
}

/// Unescapes a profile-field value.
pub fn unescape_profile_field(s: &str) -> String {
    s.replace("%0D", "\r")
        .replace("%0A", "\n")
        .replace("%09", "\t")
        .replace("%25", "%")
}

/// Restores a `localStorage` profile (`<origin>\t<key>\t<value>` per line) into the store.
pub fn load_local_storage_from_file(
    path: &std::path::Path,
    store: &SharedLocalStorage,
) -> std::io::Result<usize> {
    let body = std::fs::read_to_string(path)?;
    let mut count = 0usize;
    if let Ok(mut map) = store.lock() {
        for line in body.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let origin = unescape_profile_field(parts[0]);
                let k = unescape_profile_field(parts[1]);
                let value = unescape_profile_field(parts[2]);
                map.insert(format!("{origin}\x1f{k}"), value);
                count += 1;
            } else if parts.len() == 2 {
                map.insert(
                    unescape_profile_field(parts[0]),
                    unescape_profile_field(parts[1]),
                );
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Writes the `localStorage` store to its profile file.
pub fn save_local_storage_to_file(
    path: &std::path::Path,
    store: &SharedLocalStorage,
) -> std::io::Result<()> {
    let map = match store.lock() {
        Ok(m) => m,
        Err(_) => {
            return Err(std::io::Error::other("Lock poisoned"));
        }
    };
    let mut body = String::from("# Mango localStorage v1\n");
    for (key, value) in map.iter() {
        if let Some((origin, k)) = key.split_once('\x1f') {
            body.push_str(&format!(
                "{}\t{}\t{}\n",
                escape_profile_field(origin),
                escape_profile_field(k),
                escape_profile_field(value)
            ));
        } else {
            body.push_str(&format!(
                "{}\t{}\n",
                escape_profile_field(key),
                escape_profile_field(value)
            ));
        }
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, body)?;
    if std::fs::rename(&tmp_path, path).is_err() {
        let _ = std::fs::remove_file(path);
        std::fs::rename(&tmp_path, path)?;
    }
    Ok(())
}

/// Thread-safe cookie jar handle.
pub type SharedCookieStore = Arc<Mutex<mango_net::CookieJar>>;

#[cfg(windows)]
pub(crate) fn fill_secure_random(buf: &mut [u8]) {
    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn SystemFunction036(buffer: *mut u8, length: u32) -> u8;
    }
    if !buf.is_empty() {
        unsafe {
            SystemFunction036(buf.as_mut_ptr(), buf.len() as u32);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn fill_secure_random(buf: &mut [u8]) {
    use std::io::Read;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(buf);
    }
}

pub(crate) fn encode_base64_bytes(input: &[u8]) -> String {
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };

        out.push(B64_CHARS[b0 >> 2] as char);
        out.push(B64_CHARS[((b0 & 3) << 4) | (b1 >> 4)] as char);
        if chunk.len() > 1 {
            out.push(B64_CHARS[((b1 & 0xF) << 2) | (b2 >> 6)] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(B64_CHARS[b2 & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

// ── JS shim ───────────────────────────────────────────────────────────────────

pub fn build_web_apis_shim(page_url: &str) -> String {
    let safe_url = page_url.replace('\\', "\\\\").replace('"', "\\\"");

    format!(
        r##"
(function() {{
    // ── 1. Window / global aliases ────────────────────────────────────────────
    globalThis.window = globalThis;
    globalThis.self   = globalThis;
    globalThis.top    = globalThis;
    globalThis.parent = globalThis;
    globalThis.global = globalThis;
    if (!globalThis.process) {{
        globalThis.process = {{
            env: {{ NODE_ENV: 'production' }},
            browser: true,
            version: 'v18.0.0',
            versions: {{ node: '18.0.0', v8: '10.0.0' }},
            nextTick: (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) {{ setTimeout(cb, 0); }}
        }};
    }}

    // ── 2. location ───────────────────────────────────────────────────────────
    function _parseUrlComponents(urlStr, baseStr) {{
        var str = String(urlStr !== undefined && urlStr !== null ? urlStr : '').trim();
        if (baseStr && str.indexOf('://') === -1 && str.indexOf('about:') !== 0) {{
            var b = _parseUrlComponents(baseStr);
            if (str.indexOf('//') === 0) {{
                str = b.protocol + '//' + str.substring(2);
            }} else if (str.charAt(0) === '/') {{
                str = b.origin + str;
            }} else if (str.charAt(0) === '?') {{
                str = b.origin + b.pathname + str;
            }} else if (str.charAt(0) === '#') {{
                str = b.origin + b.pathname + b.search + str;
            }} else {{
                var p = b.pathname;
                var lastSlash = p.lastIndexOf('/');
                var dir = lastSlash !== -1 ? p.substring(0, lastSlash + 1) : '/';
                str = b.origin + dir + str;
            }}
        }}

        var res = {{
            href: str,
            protocol: 'http:',
            host: '',
            hostname: '',
            port: '',
            pathname: '/',
            search: '',
            hash: '',
            origin: ''
        }};

        if (str.indexOf('about:') === 0) {{
            res.protocol = 'about:';
            res.origin = 'null';
            res.pathname = str.substring(6);
            res.href = str;
            return res;
        }}

        var schemeSep = str.indexOf('://');
        if (schemeSep === -1) {{
            res.pathname = str.charAt(0) === '/' ? str : '/' + str;
            res.origin = 'null';
            res.href = str;
            return res;
        }}

        res.protocol = str.substring(0, schemeSep + 1).toLowerCase();
        var afterScheme = str.substring(schemeSep + 3);

        var slashIdx = afterScheme.indexOf('/');
        var qIdx = afterScheme.indexOf('?');
        var hashIdx = afterScheme.indexOf('#');

        var authorityEnd = afterScheme.length;
        if (slashIdx !== -1 && slashIdx < authorityEnd) authorityEnd = slashIdx;
        if (qIdx !== -1 && qIdx < authorityEnd) authorityEnd = qIdx;
        if (hashIdx !== -1 && hashIdx < authorityEnd) authorityEnd = hashIdx;

        var authority = afterScheme.substring(0, authorityEnd);
        var rest = afterScheme.substring(authorityEnd);

        res.host = authority;
        var colonIdx = authority.lastIndexOf(':');
        if (colonIdx !== -1 && authority.indexOf(']') === -1) {{
            res.hostname = authority.substring(0, colonIdx);
            res.port = authority.substring(colonIdx + 1);
        }} else {{
            res.hostname = authority;
            res.port = '';
        }}
        res.origin = res.protocol + '//' + res.host;

        var pathEnd = rest.length;
        var rQIdx = rest.indexOf('?');
        var rHIdx = rest.indexOf('#');
        if (rQIdx !== -1 && rQIdx < pathEnd) pathEnd = rQIdx;
        if (rHIdx !== -1 && rHIdx < pathEnd) pathEnd = rHIdx;

        res.pathname = pathEnd > 0 ? rest.substring(0, pathEnd) : '/';
        if (res.pathname.charAt(0) !== '/') res.pathname = '/' + res.pathname;

        if (rQIdx !== -1) {{
            if (rHIdx !== -1 && rHIdx > rQIdx) {{
                res.search = rest.substring(rQIdx, rHIdx);
                res.hash = rest.substring(rHIdx);
            }} else {{
                res.search = rest.substring(rQIdx);
                res.hash = '';
            }}
        }} else if (rHIdx !== -1) {{
            res.search = '';
            res.hash = rest.substring(rHIdx);
        }} else {{
            res.search = '';
            res.hash = '';
        }}

        res.href = res.origin + res.pathname + res.search + res.hash;
        return res;
    }}

    var _curHref = "{safe_url}";
    var _locParsed = _parseUrlComponents(_curHref);
    var _historyStack = [{{ url: _curHref, state: null }}];
    var _historyIdx   = 0;

    function _updateLocationHref(newHref, triggerNav, isReplace) {{
        var parsed = _parseUrlComponents(newHref, _curHref);
        var oldHref = _curHref;
        var oldHash = _locParsed ? _locParsed.hash : '';
        _curHref = parsed.href;
        _locParsed = parsed;
        if (isReplace && _historyStack.length > 0) {{
            _historyStack[_historyIdx] = {{ url: _curHref, state: _historyStack[_historyIdx].state }};
        }} else if (triggerNav) {{
            _historyStack.splice(_historyIdx + 1);
            _historyStack.push({{ url: _curHref, state: null }});
            _historyIdx = _historyStack.length - 1;
        }}
        if (triggerNav && typeof _mangoNavigate === 'function') {{
            _mangoNavigate(_curHref);
        }}
        if (parsed.hash !== oldHash && !triggerNav) {{
            if (typeof window !== 'undefined' && window.dispatchEvent && typeof HashChangeEvent === 'function') {{
                window.dispatchEvent(new HashChangeEvent('hashchange', {{
                    oldURL: oldHref,
                    newURL: _curHref
                }}));
            }}
        }}
    }}

    function Location() {{}}
    Location.prototype = {{
        constructor: Location,
        get href() {{ return _locParsed.href; }},
        set href(val) {{
            _updateLocationHref(String(val), true, false);
        }},
        get protocol() {{ return _locParsed.protocol; }},
        set protocol(val) {{
            var p = String(val).toLowerCase();
            if (p.charAt(p.length - 1) !== ':') p += ':';
            var newHref = p + '//' + _locParsed.host + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get host() {{ return _locParsed.host; }},
        set host(val) {{
            var newHref = _locParsed.protocol + '//' + String(val) + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get hostname() {{ return _locParsed.hostname; }},
        set hostname(val) {{
            var portPart = _locParsed.port ? ':' + _locParsed.port : '';
            var newHref = _locParsed.protocol + '//' + String(val) + portPart + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get port() {{ return _locParsed.port; }},
        set port(val) {{
            var p = String(val).trim();
            var portPart = p ? ':' + p : '';
            var newHref = _locParsed.protocol + '//' + _locParsed.hostname + portPart + _locParsed.pathname + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get pathname() {{ return _locParsed.pathname; }},
        set pathname(val) {{
            var p = String(val);
            if (p.charAt(0) !== '/') p = '/' + p;
            var newHref = _locParsed.origin + p + _locParsed.search + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get search() {{ return _locParsed.search; }},
        set search(val) {{
            var s = String(val);
            if (s && s.charAt(0) !== '?') s = '?' + s;
            var newHref = _locParsed.origin + _locParsed.pathname + s + _locParsed.hash;
            _updateLocationHref(newHref, true, false);
        }},
        get hash() {{ return _locParsed.hash; }},
        set hash(val) {{
            var h = String(val);
            if (h && h.charAt(0) !== '#') h = '#' + h;
            var oldHref = _curHref;
            var oldHash = _locParsed.hash;
            var newHref = _locParsed.origin + _locParsed.pathname + _locParsed.search + h;
            if (h !== oldHash) {{
                _historyStack.push({{ url: newHref, state: (globalThis.history ? globalThis.history.state : null) }});
                _historyIdx = _historyStack.length - 1;
                _updateLocationHref(newHref, false, false);
                if (typeof window !== 'undefined' && window.dispatchEvent && typeof PopStateEvent === 'function') {{
                    window.dispatchEvent(new PopStateEvent('popstate', {{
                        state: (globalThis.history ? globalThis.history.state : null)
                    }}));
                }}
            }}
        }},
        get origin() {{ return _locParsed.origin; }},
        assign: function(u) {{
            _updateLocationHref(String(u), true, false);
        }},
        replace: function(u) {{
            _updateLocationHref(String(u), true, true);
        }},
        reload: function() {{
            if (typeof _mangoNavigate === 'function') {{
                _mangoNavigate(_curHref);
            }}
        }},
        toString: function() {{
            return this.href;
        }}
    }};

    var _loc = Object.create(Location.prototype);
    globalThis.Location = Location;
    globalThis.location = _loc;

    // ── 3. navigator ─────────────────────────────────────────────────────────
    globalThis.navigator = {{
        userAgent:          "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Mango/0.1.0",
        appName:            "Netscape",
        appVersion:         "5.0 (Windows NT 10.0; Win64; x64)",
        platform:           "Win32",
        language:           "en-US",
        languages:          ["en-US", "en"],
        onLine:             true,
        cookieEnabled:      true,
        vendor:             "Google Inc.",
        hardwareConcurrency: 4,
        maxTouchPoints:     0,
        sendBeacon:         function() {{ return false; }},
        clipboard: {{
            readText:  function() {{ return Promise.resolve(""); }},
            writeText: function(t) {{ return Promise.resolve(); }}
        }},
        userAgentData: {{
            brands: [
                {{ brand: "Not-A.Brand", version: "24" }},
                {{ brand: "Chromium", version: "124" }},
                {{ brand: "Google Chrome", version: "124" }}
            ],
            mobile: false,
            platform: "Windows",
            getHighEntropyValues: function(hints) {{
                return Promise.resolve({{
                    architecture: "x86",
                    bitness: "64",
                    brands: this.brands,
                    mobile: false,
                    model: "",
                    platform: "Windows",
                    platformVersion: "15.0.0",
                    uaFullVersion: "124.0.0.0"
                }});
            }},
            toJSON: function() {{
                return {{ brands: this.brands, mobile: this.mobile, platform: this.platform }};
            }}
        }},
        connection: {{
            effectiveType: "4g",
            rtt: 50,
            downlink: 10,
            saveData: false,
            addEventListener: function() {{}},
            removeEventListener: function() {{}}
        }},
        permissions: {{
            query: function(desc) {{
                return Promise.resolve({{ state: "granted", addEventListener: function() {{}}, removeEventListener: function() {{}} }});
            }}
        }},
        mediaCapabilities: {{
            decodingInfo: function() {{ return Promise.resolve({{ supported: true, smooth: true, powerEfficient: true }}); }},
            encodingInfo: function() {{ return Promise.resolve({{ supported: true, smooth: true, powerEfficient: true }}); }}
        }},
        storage: {{
            estimate: function() {{ return Promise.resolve({{ quota: 1000000000, usage: 0 }}); }},
            persist:  function() {{ return Promise.resolve(true); }}
        }}
    }};

    // ── 4. screen ────────────────────────────────────────────────────────────
    globalThis.screen = {{
        width:       1920,
        height:      1080,
        availWidth:  1920,
        availHeight: 1040,
        colorDepth:  24,
        pixelDepth:  24
    }};

    // ── 5. history ───────────────────────────────────────────────────────────
    function History() {{}}
    History.prototype = {{
        constructor: History,
        get length() {{ return _historyStack.length; }},
        get state() {{ return _historyStack[_historyIdx] ? _historyStack[_historyIdx].state : null; }},
        get scrollRestoration() {{ return this._scrollRestoration || 'auto'; }},
        set scrollRestoration(v) {{ this._scrollRestoration = (v === 'manual') ? 'manual' : 'auto'; }},
        back: function() {{ this.go(-1); }},
        forward: function() {{ this.go(1); }},
        go: function(n) {{
            var delta = parseInt(n, 10) || 0;
            if (delta === 0) {{
                location.reload();
                return;
            }}
            var targetIdx = _historyIdx + delta;
            if (targetIdx >= 0 && targetIdx < _historyStack.length && targetIdx !== _historyIdx) {{
                var oldHref = _curHref;
                var oldHash = _locParsed.hash;
                _historyIdx = targetIdx;
                var entry = _historyStack[_historyIdx];
                _updateLocationHref(entry.url, false, false);
                if (typeof window !== 'undefined' && window.dispatchEvent && typeof PopStateEvent === 'function') {{
                    window.dispatchEvent(new PopStateEvent('popstate', {{
                        state: entry.state
                    }}));
                }}
            }}
        }},
        pushState: function(state, title, url) {{
            var newUrl = _curHref;
            if (url !== undefined && url !== null && url !== '') {{
                var resolved = _parseUrlComponents(url, _curHref);
                if (_locParsed.origin !== 'null' && resolved.origin !== 'null' && resolved.origin !== _locParsed.origin) {{
                    var err = typeof DOMException !== 'undefined' 
                        ? new DOMException("Failed to execute 'pushState' on 'History': A history state object cannot be added with URL of different origin.", "SecurityError")
                        : new Error("SecurityError: Failed to execute 'pushState' on 'History': A history state object cannot be added with URL of different origin.");
                    err.name = "SecurityError";
                    throw err;
                }}
                newUrl = resolved.href;
            }}
            var clonedState = null;
            if (state !== undefined && state !== null) {{
                try {{
                    clonedState = JSON.parse(JSON.stringify(state));
                }} catch(e) {{
                    clonedState = state;
                }}
            }}
            _historyStack.splice(_historyIdx + 1);
            _historyStack.push({{ url: newUrl, state: clonedState }});
            _historyIdx = _historyStack.length - 1;
            _updateLocationHref(newUrl, false, false);
        }},
        replaceState: function(state, title, url) {{
            var newUrl = _curHref;
            if (url !== undefined && url !== null && url !== '') {{
                var resolved = _parseUrlComponents(url, _curHref);
                if (_locParsed.origin !== 'null' && resolved.origin !== 'null' && resolved.origin !== _locParsed.origin) {{
                    var err = typeof DOMException !== 'undefined' 
                        ? new DOMException("Failed to execute 'replaceState' on 'History': A history state object cannot be added with URL of different origin.", "SecurityError")
                        : new Error("SecurityError: Failed to execute 'replaceState' on 'History': A history state object cannot be added with URL of different origin.");
                    err.name = "SecurityError";
                    throw err;
                }}
                newUrl = resolved.href;
            }}
            var clonedState = null;
            if (state !== undefined && state !== null) {{
                try {{
                    clonedState = JSON.parse(JSON.stringify(state));
                }} catch(e) {{
                    clonedState = state;
                }}
            }}
            _historyStack[_historyIdx] = {{ url: newUrl, state: clonedState }};
            _updateLocationHref(newUrl, false, false);
        }}
    }};

    var _hist = Object.create(History.prototype);
    _hist._scrollRestoration = 'auto';
    globalThis.History = History;
    globalThis.history = _hist;

    // ── 6. performance ───────────────────────────────────────────────────────
    var _p0 = Date.now();
    var _perfEntries = [];
    function PerformanceEntry(name, entryType, startTime, duration) {{
        this.name = String(name || '');
        this.entryType = String(entryType || '');
        this.startTime = Number(startTime || 0);
        this.duration = Number(duration || 0);
    }}
    PerformanceEntry.prototype.toJSON = function() {{
        return {{
            name: this.name,
            entryType: this.entryType,
            startTime: this.startTime,
            duration: this.duration
        }};
    }};
    globalThis.PerformanceEntry = PerformanceEntry;

    function PerformanceResourceTiming(name, initiatorType, startTime, duration, responseStatus) {{
        PerformanceEntry.call(this, name, 'resource', startTime, duration);
        this.initiatorType = initiatorType || 'fetch';
        this.responseStatus = responseStatus || 200;
        this.transferSize = 0;
        this.encodedBodySize = 0;
        this.decodedBodySize = 0;
        this.fetchStart = startTime;
        this.responseEnd = startTime + duration;
    }}
    PerformanceResourceTiming.prototype = Object.create(PerformanceEntry.prototype);
    PerformanceResourceTiming.prototype.constructor = PerformanceResourceTiming;
    globalThis.PerformanceResourceTiming = PerformanceResourceTiming;

    globalThis.performance = {{
        now:         function() {{ return Date.now() - _p0; }},
        timeOrigin:  _p0,
        timing:      {{ navigationStart: _p0, loadEventEnd: _p0 + 50 }},
        mark:        function(name) {{
            var entry = new PerformanceEntry(name, 'mark', Date.now() - _p0, 0);
            _perfEntries.push(entry);
            return entry;
        }},
        measure:     function(name, startMark, endMark) {{
            var start = 0;
            var end = Date.now() - _p0;
            if (startMark) {{
                for (var i = _perfEntries.length - 1; i >= 0; i--) {{
                    if (_perfEntries[i].name === startMark) {{ start = _perfEntries[i].startTime; break; }}
                }}
            }}
            if (endMark) {{
                for (var i = _perfEntries.length - 1; i >= 0; i--) {{
                    if (_perfEntries[i].name === endMark) {{ end = _perfEntries[i].startTime; break; }}
                }}
            }}
            var entry = new PerformanceEntry(name, 'measure', start, Math.max(0, end - start));
            _perfEntries.push(entry);
            return entry;
        }},
        clearMarks:  function(name) {{
            _perfEntries = _perfEntries.filter(function(e) {{
                return e.entryType !== 'mark' || (name && e.name !== name);
            }});
        }},
        clearMeasures: function(name) {{
            _perfEntries = _perfEntries.filter(function(e) {{
                return e.entryType !== 'measure' || (name && e.name !== name);
            }});
        }},
        clearResourceTimings: function() {{
            _perfEntries = _perfEntries.filter(function(e) {{ return e.entryType !== 'resource'; }});
        }},
        getEntries:  function() {{ return _perfEntries.slice(); }},
        getEntriesByName: function(name, type) {{
            return _perfEntries.filter(function(e) {{
                return e.name === name && (!type || e.entryType === type);
            }});
        }},
        getEntriesByType: function(type) {{
            return _perfEntries.filter(function(e) {{ return e.entryType === type; }});
        }}
    }};

    // ── 7. localStorage (backed by native Rust HashMap) ───────────────────────
    //
    // Reads/writes go through _mangoLs* native functions so the browser can
    // persist the map to disk across page loads.
    function Storage() {{}}
    var _storageProto = {{
        getItem: function(k) {{ return _mangoLsGet(String(k)); }},
        setItem: function(k, v) {{
            var old = _mangoLsGet(String(k));
            _mangoLsSet(String(k), String(v));
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
                window.dispatchEvent(new StorageEvent('storage', {{
                    key: String(k),
                    oldValue: old,
                    newValue: String(v),
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }}));
            }}
        }},
        removeItem: function(k) {{
            var old = _mangoLsGet(String(k));
            _mangoLsRemove(String(k));
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
                window.dispatchEvent(new StorageEvent('storage', {{
                    key: String(k),
                    oldValue: old,
                    newValue: null,
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }}));
            }}
        }},
        clear: function() {{
            _mangoLsClear();
            if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
                window.dispatchEvent(new StorageEvent('storage', {{
                    key: null,
                    oldValue: null,
                    newValue: null,
                    url: window.location ? window.location.href : '',
                    storageArea: globalThis.localStorage
                }}));
            }}
        }},
        get length() {{
            var keys = _mangoLsKeys();
            return keys ? keys.length : 0;
        }},
        key: function(i) {{
            var keys = _mangoLsKeys();
            return (keys && i >= 0 && i < keys.length) ? keys[i] : null;
        }}
    }};
    Storage.prototype = _storageProto;
    globalThis.Storage = Storage;

    var _lsBase = Object.create(Storage.prototype);
    if (typeof Proxy !== 'undefined') {{
        globalThis.localStorage = new Proxy(_lsBase, {{
            get: function(target, prop) {{
                if (prop in target || typeof prop === 'symbol') {{
                    return target[prop];
                }}
                var val = target.getItem(prop);
                return val !== null ? val : undefined;
            }},
            set: function(target, prop, value) {{
                if (prop in target) {{
                    target[prop] = value;
                }} else {{
                    target.setItem(prop, value);
                }}
                return true;
            }},
            deleteProperty: function(target, prop) {{
                if (!(prop in target)) {{
                    target.removeItem(prop);
                }}
                return true;
            }},
            has: function(target, prop) {{
                if (prop in target) {{
                    return true;
                }}
                return target.getItem(prop) !== null;
            }},
            ownKeys: function(target) {{
                var keys = [];
                for (var i = 0; i < target.length; i++) {{
                    var k = target.key(i);
                    if (k !== null) keys.push(k);
                }}
                return keys;
            }}
        }});
    }} else {{
        globalThis.localStorage = _lsBase;
    }}

    // sessionStorage: tab-scoped in-memory, partitioned per origin.
    var _ssBase = Object.create(Storage.prototype);
    _ssBase.getItem = function(k) {{ return _mangoSsGet(String(k)); }};
    _ssBase.setItem = function(k, v) {{
        var old = _mangoSsGet(String(k));
        _mangoSsSet(String(k), String(v));
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
            window.dispatchEvent(new StorageEvent('storage', {{
                key: String(k),
                oldValue: old,
                newValue: String(v),
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }}));
        }}
    }};
    _ssBase.removeItem = function(k) {{
        var old = _mangoSsGet(String(k));
        _mangoSsRemove(String(k));
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
            window.dispatchEvent(new StorageEvent('storage', {{
                key: String(k),
                oldValue: old,
                newValue: null,
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }}));
        }}
    }};
    _ssBase.clear = function() {{
        _mangoSsClear();
        if (typeof StorageEvent === 'function' && typeof window !== 'undefined' && window.dispatchEvent) {{
            window.dispatchEvent(new StorageEvent('storage', {{
                key: null,
                oldValue: null,
                newValue: null,
                url: window.location ? window.location.href : '',
                storageArea: globalThis.sessionStorage
            }}));
        }}
    }};
    Object.defineProperty(_ssBase, 'length', {{
        get: function() {{
            var keys = _mangoSsKeys();
            return keys ? keys.length : 0;
        }}
    }});
    _ssBase.key = function(i) {{
        var keys = _mangoSsKeys();
        return (keys && i >= 0 && i < keys.length) ? keys[i] : null;
    }};

    if (typeof Proxy !== 'undefined') {{
        globalThis.sessionStorage = new Proxy(_ssBase, {{
            get: function(target, prop) {{
                if (prop in target || typeof prop === 'symbol') {{
                    return target[prop];
                }}
                var val = target.getItem(prop);
                return val !== null ? val : undefined;
            }},
            set: function(target, prop, value) {{
                if (prop in target) {{
                    target[prop] = value;
                }} else {{
                    target.setItem(prop, value);
                }}
                return true;
            }},
            deleteProperty: function(target, prop) {{
                if (!(prop in target)) {{
                    target.removeItem(prop);
                }}
                return true;
            }},
            has: function(target, prop) {{
                if (prop in target) {{
                    return true;
                }}
                return target.getItem(prop) !== null;
            }},
            ownKeys: function(target) {{
                var keys = [];
                for (var i = 0; i < target.length; i++) {{
                    var k = target.key(i);
                    if (k !== null) keys.push(k);
                }}
                return keys;
            }}
        }});
    }} else {{
        globalThis.sessionStorage = _ssBase;
    }}

    // ── 7b. EventTarget base class ───────────────────────────────────────────
    function EventTarget() {{
        this._listeners = {{}};
    }}
    EventTarget.prototype = {{
        addEventListener: function(type, listener, opts) {{
            if (typeof globalThis._addDOMEventListener === 'function') {{
                globalThis._addDOMEventListener(this, type, listener, opts);
            }} else {{
                if (!listener) return;
                var t = String(type);
                if (!this._listeners) this._listeners = {{}};
                if (!this._listeners[t]) this._listeners[t] = [];
                var cap = false, onc = false;
                if (typeof opts === 'boolean') cap = opts;
                else if (opts && typeof opts === 'object') {{ cap = !!opts.capture; onc = !!opts.once; }}
                for (var i = 0; i < this._listeners[t].length; i++) {{
                    if (this._listeners[t][i].fn === listener && this._listeners[t][i].capture === cap) return;
                }}
                this._listeners[t].push({{ fn: listener, capture: cap, once: onc }});
            }}
        }},
        removeEventListener: function(type, listener, opts) {{
            if (typeof globalThis._removeDOMEventListener === 'function') {{
                globalThis._removeDOMEventListener(this, type, listener, opts);
            }} else {{
                if (!this._listeners) return;
                var t = String(type);
                if (!this._listeners[t]) return;
                var cap = false;
                if (typeof opts === 'boolean') cap = opts;
                else if (opts && typeof opts === 'object') cap = !!opts.capture;
                this._listeners[t] = this._listeners[t].filter(function(item) {{
                    return !(item.fn === listener && item.capture === cap);
                }});
            }}
        }},
        dispatchEvent: function(evt) {{
            if (typeof globalThis._dispatchDOMEvent === 'function') {{
                return globalThis._dispatchDOMEvent(this, evt);
            }} else {{
                if (!evt) return true;
                var t = String(evt.type || evt);
                var handler = 'on' + t;
                if (typeof this[handler] === 'function') {{
                    try {{ this[handler].call(this, evt); }} catch(e) {{}}
                }}
                if (!this._listeners || !this._listeners[t]) return true;
                var list = this._listeners[t].slice();
                for (var i = 0; i < list.length; i++) {{
                    var item = list[i];
                    if (item.once) this.removeEventListener(t, item.fn, {{ capture: item.capture }});
                    try {{
                        if (typeof item.fn === 'function') item.fn.call(this, evt);
                        else if (item.fn && typeof item.fn.handleEvent === 'function') item.fn.handleEvent(evt);
                    }} catch(e) {{}}
                }}
                return true;
            }}
        }}
    }};
    globalThis.EventTarget = EventTarget;

    // ── 8. Window EventTarget ─────────────────────────────────────────────────
    globalThis.addEventListener = function(type, fn, opts) {{
        if (typeof globalThis._addDOMEventListener === 'function') {{
            globalThis._addDOMEventListener(globalThis, type, fn, opts);
        }}
    }};
    globalThis.removeEventListener = function(type, fn, opts) {{
        if (typeof globalThis._removeDOMEventListener === 'function') {{
            globalThis._removeDOMEventListener(globalThis, type, fn, opts);
        }}
    }};
    globalThis.dispatchEvent = function(evt) {{
        if (typeof globalThis._dispatchDOMEvent === 'function') {{
            return globalThis._dispatchDOMEvent(globalThis, evt);
        }}
        return true;
    }};

    // ── 9. Timers with closure preservation ──────────────────────────────────
    var _timerCallbacks = {{}};
    var _timerNextId = 1;
    globalThis._mangoRunTimer = function(id) {{
        var item = _timerCallbacks[id];
        if (!item) return;
        try {{
            if (typeof item.fn === 'function') item.fn.apply(globalThis, item.args || []);
            else if (typeof item.fn === 'string') (0, eval)(item.fn);
        }} catch(e) {{}}
        if (!item.repeat) delete _timerCallbacks[id];
    }};
    globalThis.setTimeout = function(fn, delay) {{
        var args = Array.prototype.slice.call(arguments, 2);
        var id = _timerNextId++;
        _timerCallbacks[id] = {{ fn: fn, repeat: false, args: args }};
        if (typeof _mangoScheduleTimer === 'function') _mangoScheduleTimer(id, Number(delay) || 0, false);
        return id;
    }};
    globalThis.setInterval = function(fn, delay) {{
        var args = Array.prototype.slice.call(arguments, 2);
        var id = _timerNextId++;
        _timerCallbacks[id] = {{ fn: fn, repeat: true, args: args }};
        if (typeof _mangoScheduleTimer === 'function') _mangoScheduleTimer(id, Number(delay) || 0, true);
        return id;
    }};
    globalThis.clearTimeout = function(id) {{
        var numId = Number(id) || 0;
        delete _timerCallbacks[numId];
        if (typeof _mangoCancelTimer === 'function') _mangoCancelTimer(numId);
    }};
    globalThis.clearInterval = globalThis.clearTimeout;

    // ── 10. requestAnimationFrame ─────────────────────────────────────────────
    globalThis.requestAnimationFrame = function(cb) {{
        return globalThis.setTimeout(function() {{
            try {{ if (typeof cb === 'function') cb(performance.now()); }} catch(e) {{}}
        }}, 16);
    }};
    globalThis.cancelAnimationFrame = globalThis.clearTimeout;
    globalThis.requestIdleCallback = function(cb, opts) {{
        return globalThis.setTimeout(function() {{
            var start = Date.now();
            try {{
                if (typeof cb === 'function') {{
                    cb({{
                        didTimeout: false,
                        timeRemaining: function() {{ return Math.max(0, 50 - (Date.now() - start)); }}
                    }});
                }}
            }} catch(e) {{}}
        }}, 1);
    }};
    globalThis.cancelIdleCallback = globalThis.clearTimeout;

    // ── 11. matchMedia ────────────────────────────────────────────────────────
    globalThis.matchMedia = function(q) {{
        var qs = String(q || "");
        // Honour prefers-color-scheme: dark — reported as false (light mode).
        // Honour hover and pointer features for desktop.
        var matchMap = {{
            "prefers-color-scheme: light": true,
            "prefers-color-scheme: dark":  false,
            "hover: hover":                true,
            "pointer: fine":               true
        }};
        var matched = false;
        for (var k in matchMap) {{
            if (qs.indexOf(k) !== -1) {{ matched = matchMap[k]; break; }}
        }}
        var minW = qs.match(/min-width:\s*(\d+)px/);
        var maxW = qs.match(/max-width:\s*(\d+)px/);
        if (minW || maxW) {{
            var curW = (typeof window !== 'undefined' && window.innerWidth) || globalThis.innerWidth || 800;
            var ok = true;
            if (minW && curW < parseInt(minW[1], 10)) {{ ok = false; }}
            if (maxW && curW > parseInt(maxW[1], 10)) {{ ok = false; }}
            matched = ok;
        }}
        var mql = {{
            matches:             matched,
            media:               qs,
            onchange:            null,
            addListener:         function(fn) {{ this.addEventListener('change', fn); }},
            removeListener:      function(fn) {{ this.removeEventListener('change', fn); }},
            addEventListener:    function() {{}},
            removeEventListener: function() {{}},
            dispatchEvent:       function() {{ return false; }}
        }};
        return mql;
    }};

    // ── 12. getComputedStyle & CSSStyleDeclaration ────────────────────────────
    function CSSStyleDeclaration(el) {{
        this._el = el || null;
    }}
    CSSStyleDeclaration.prototype.getPropertyValue = function(prop) {{
        var p = String(prop || '').trim();
        if (!p) return "";
        var camel = p.replace(/-([a-z])/g, function(g) {{ return g[1].toUpperCase(); }});
        if (this[camel] !== undefined) return String(this[camel]);
        if (this[p] !== undefined) return String(this[p]);
        return "";
    }};
    CSSStyleDeclaration.prototype.getPropertyPriority = function(prop) {{
        return "";
    }};
    CSSStyleDeclaration.prototype.setProperty = function(prop, val, prio) {{
        // Computed style declaration is read-only
    }};
    CSSStyleDeclaration.prototype.removeProperty = function(prop) {{
        return "";
    }};
    CSSStyleDeclaration.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : "";
    }};
    CSSStyleDeclaration.prototype[Symbol.iterator] = function() {{
        var self = this;
        var idx = 0;
        return {{
            next: function() {{
                if (idx < self.length) {{
                    return {{ value: self[idx++], done: false }};
                }} else {{
                    return {{ value: undefined, done: true }};
                }}
            }}
        }};
    }};
    CSSStyleDeclaration.prototype[Symbol.toStringTag] = 'CSSStyleDeclaration';
    globalThis.CSSStyleDeclaration = CSSStyleDeclaration;

    globalThis.getComputedStyle = function(el, pseudo) {{
        var cs = new CSSStyleDeclaration(el);
        var s = (el && el.style) ? el.style : {{}};

        var rect = (el && typeof el.getBoundingClientRect === 'function') ? el.getBoundingClientRect() : null;
        var widthStr = (rect && rect.width > 0) ? (rect.width + 'px') : (s.width || '0px');
        var heightStr = (rect && rect.height > 0) ? (rect.height + 'px') : (s.height || '0px');

        var tag = (el && el.tagName) ? el.tagName.toUpperCase() : '';
        var defaultDisplay = 'block';
        if (tag === 'SPAN' || tag === 'A' || tag === 'B' || tag === 'I' || tag === 'EM' || tag === 'STRONG' || tag === 'LABEL') {{
            defaultDisplay = 'inline';
        }} else if (tag === 'BUTTON' || tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA' || tag === 'IMG') {{
            defaultDisplay = 'inline-block';
        }} else if (tag === 'TABLE') {{
            defaultDisplay = 'table';
        }} else if (tag === 'TR') {{
            defaultDisplay = 'table-row';
        }} else if (tag === 'TD' || tag === 'TH') {{
            defaultDisplay = 'table-cell';
        }} else if (tag === 'LI') {{
            defaultDisplay = 'list-item';
        }}

        var props = {{
            display:          s.display          || defaultDisplay,
            visibility:       s.visibility       || 'visible',
            opacity:          s.opacity          || '1',
            width:            widthStr,
            height:           heightStr,
            color:            s.color            || 'rgb(0, 0, 0)',
            backgroundColor:  s.backgroundColor  || 'rgba(0, 0, 0, 0)',
            fontSize:         s.fontSize         || '16px',
            fontFamily:       s.fontFamily       || 'sans-serif',
            fontWeight:       s.fontWeight       || 'normal',
            lineHeight:       s.lineHeight       || 'normal',
            position:         s.position         || 'static',
            top:              s.top              || 'auto',
            left:             s.left             || 'auto',
            right:            s.right            || 'auto',
            bottom:           s.bottom           || 'auto',
            margin:           s.margin           || '0px',
            marginTop:        s.marginTop        || s.margin || '0px',
            marginRight:      s.marginRight      || s.margin || '0px',
            marginBottom:     s.marginBottom     || s.margin || '0px',
            marginLeft:       s.marginLeft       || s.margin || '0px',
            padding:          s.padding          || '0px',
            paddingTop:       s.paddingTop       || s.padding || '0px',
            paddingRight:     s.paddingRight     || s.padding || '0px',
            paddingBottom:    s.paddingBottom    || s.padding || '0px',
            paddingLeft:      s.paddingLeft      || s.padding || '0px',
            border:           s.border           || '0px none rgb(0, 0, 0)',
            borderWidth:      s.borderWidth      || '0px',
            borderTopWidth:   s.borderTopWidth   || s.borderWidth || '0px',
            borderRightWidth: s.borderRightWidth || s.borderWidth || '0px',
            borderBottomWidth:s.borderBottomWidth|| s.borderWidth || '0px',
            borderLeftWidth:  s.borderLeftWidth  || s.borderWidth || '0px',
            borderStyle:      s.borderStyle      || 'none',
            borderColor:      s.borderColor      || 'rgb(0, 0, 0)',
            boxSizing:        s.boxSizing        || 'content-box',
            transform:        s.transform        || 'none',
            transition:       s.transition       || 'none',
            overflow:         s.overflow         || 'visible',
            overflowX:        s.overflowX        || s.overflow || 'visible',
            overflowY:        s.overflowY        || s.overflow || 'visible',
            zIndex:           s.zIndex           || 'auto',
            flexDirection:    s.flexDirection    || 'row',
            alignItems:       s.alignItems       || 'stretch',
            justifyContent:   s.justifyContent   || 'flex-start',
            cursor:           s.cursor           || 'auto'
        }};

        var propNames = Object.keys(props);
        cs.length = propNames.length;
        for (var i = 0; i < propNames.length; i++) {{
            var k = propNames[i];
            var v = props[k];
            cs[k] = v;
            var kebab = k.replace(/([A-Z])/g, '-$1').toLowerCase();
            cs[kebab] = v;
            cs[i] = kebab;
        }}
        return cs;
    }};
    if (typeof window !== 'undefined') {{
        window.getComputedStyle = globalThis.getComputedStyle;
    }}

    // ── 13. URL & URLSearchParams ─────────────────────────────────────────────
    function URLSearchParams(init) {{
        this._entries = [];
        var self = this;
        if (init) {{
            if (typeof init === 'string') {{
                var s = init.replace(/^\?/, '');
                if (s.length > 0) {{
                    var pairs = s.split('&');
                    for (var i = 0; i < pairs.length; i++) {{
                        var pair = pairs[i];
                        var idx = pair.indexOf('=');
                        if (idx !== -1) {{
                            var k = decodeURIComponent(pair.substring(0, idx).replace(/\+/g, ' '));
                            var v = decodeURIComponent(pair.substring(idx + 1).replace(/\+/g, ' '));
                            this._entries.push([k, v]);
                        }} else if (pair.length > 0) {{
                            var k = decodeURIComponent(pair.replace(/\+/g, ' '));
                            this._entries.push([k, '']);
                        }}
                    }}
                }}
            }} else if (init instanceof URLSearchParams || (init._entries && Array.isArray(init._entries))) {{
                init.forEach(function(v, k) {{ self._entries.push([String(k), String(v)]); }});
            }} else if (Array.isArray(init)) {{
                for (var i = 0; i < init.length; i++) {{
                    var item = init[i];
                    if (item && item.length >= 2) {{
                        this._entries.push([String(item[0]), String(item[1])]);
                    }}
                }}
            }} else if (typeof init === 'object') {{
                var keys = Object.keys(init);
                for (var i = 0; i < keys.length; i++) {{
                    var k = keys[i];
                    this._entries.push([String(k), String(init[k])]);
                }}
            }}
        }}
    }}
    URLSearchParams.prototype = {{
        append: function(name, value) {{
            this._entries.push([String(name), String(value)]);
            if (this._url) this._url._syncFromSearchParams();
        }},
        delete: function(name, value) {{
            var strName = String(name);
            if (value !== undefined) {{
                var strVal = String(value);
                this._entries = this._entries.filter(function(e) {{ return !(e[0] === strName && e[1] === strVal); }});
            }} else {{
                this._entries = this._entries.filter(function(e) {{ return e[0] !== strName; }});
            }}
            if (this._url) this._url._syncFromSearchParams();
        }},
        get: function(name) {{
            var strName = String(name);
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) return this._entries[i][1];
            }}
            return null;
        }},
        getAll: function(name) {{
            var strName = String(name);
            var res = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) res.push(this._entries[i][1]);
            }}
            return res;
        }},
        has: function(name, value) {{
            var strName = String(name);
            if (value !== undefined) {{
                var strVal = String(value);
                for (var i = 0; i < this._entries.length; i++) {{
                    if (this._entries[i][0] === strName && this._entries[i][1] === strVal) return true;
                }}
                return false;
            }}
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) return true;
            }}
            return false;
        }},
        set: function(name, value) {{
            var strName = String(name);
            var strVal = String(value);
            var found = false;
            var newEntries = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strName) {{
                    if (!found) {{
                        newEntries.push([strName, strVal]);
                        found = true;
                    }}
                }} else {{
                    newEntries.push(this._entries[i]);
                }}
            }}
            if (!found) newEntries.push([strName, strVal]);
            this._entries = newEntries;
            if (this._url) this._url._syncFromSearchParams();
        }},
        sort: function() {{
            this._entries.sort(function(a, b) {{
                if (a[0] < b[0]) return -1;
                if (a[0] > b[0]) return 1;
                return 0;
            }});
            if (this._url) this._url._syncFromSearchParams();
        }},
        forEach: function(cb, thisArg) {{
            for (var i = 0; i < this._entries.length; i++) {{
                var e = this._entries[i];
                cb.call(thisArg, e[1], e[0], this);
            }}
        }},
        keys: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][0], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        values: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][1], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        entries: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) {{
                        var e = self._entries[i++];
                        return {{ value: [e[0], e[1]], done: false }};
                    }}
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        toString: function() {{
            var parts = [];
            for (var i = 0; i < this._entries.length; i++) {{
                var e = this._entries[i];
                parts.push(encodeURIComponent(e[0]).replace(/%20/g, '+') + '=' + encodeURIComponent(e[1]).replace(/%20/g, '+'));
            }}
            return parts.join('&');
        }}
    }};
    URLSearchParams.prototype[Symbol.iterator] = URLSearchParams.prototype.entries;
    Object.defineProperty(URLSearchParams.prototype, 'size', {{
        get: function() {{ return this._entries.length; }}
    }});
    globalThis.URLSearchParams = URLSearchParams;

    function URL(url, base) {{
        if (!url && url !== '') throw new TypeError('Invalid URL: url is required');
        var rawUrl = String(url);
        var full;
        if (base !== undefined && base !== null && base !== '') {{
            var baseStr = String(base);
            if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(rawUrl)) {{
                full = rawUrl;
            }} else {{
                var bSi = baseStr.indexOf('://');
                if (bSi === -1) throw new TypeError('Invalid base URL: ' + baseStr);
                var bScheme = baseStr.substring(0, bSi + 3);
                var bRest = baseStr.substring(bSi + 3);
                var bSlash = bRest.indexOf('/');
                var bHost = bSlash !== -1 ? bRest.substring(0, bSlash) : bRest;
                var bPath = bSlash !== -1 ? bRest.substring(bSlash).split('?')[0].split('#')[0] : '/';
                if (rawUrl.indexOf('//') === 0) {{
                    full = baseStr.substring(0, bSi + 1) + rawUrl;
                }} else if (rawUrl.charAt(0) === '/') {{
                    full = bScheme + bHost + rawUrl;
                }} else if (rawUrl.charAt(0) === '?') {{
                    full = bScheme + bHost + bPath + rawUrl;
                }} else if (rawUrl.charAt(0) === '#') {{
                    var bQuery = bRest.indexOf('?') !== -1 ? '?' + bRest.substring(bRest.indexOf('?') + 1).split('#')[0] : '';
                    full = bScheme + bHost + bPath + bQuery + rawUrl;
                }} else {{
                    var lastSlash = bPath.lastIndexOf('/');
                    var dir = lastSlash !== -1 ? bPath.substring(0, lastSlash + 1) : '/';
                    full = bScheme + bHost + dir + rawUrl;
                }}
            }}
        }} else {{
            if (!/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(rawUrl)) {{
                throw new TypeError('Invalid URL: ' + rawUrl);
            }}
            full = rawUrl;
        }}
        
        this._searchParams = null;
        this._parse(full);
    }}
    URL.prototype = {{
        _parse: function(full) {{
            var si = full.indexOf('://');
            if (si === -1) {{
                var col = full.indexOf(':');
                if (col !== -1) {{
                    this.protocol = full.substring(0, col + 1).toLowerCase();
                    this.pathname = full.substring(col + 1);
                    this.host = '';
                    this.hostname = '';
                    this.port = '';
                    this.search = '';
                    this.hash = '';
                    this.origin = 'null';
                    this.href = full;
                    return;
                }}
                throw new TypeError('Invalid URL: ' + full);
            }}
            this.protocol = full.substring(0, si + 1).toLowerCase();
            var rest = full.substring(si + 3);
            var slash = rest.indexOf('/');
            var qm = rest.indexOf('?');
            var hash = rest.indexOf('#');
            var firstSep = Math.min(slash === -1 ? 99999 : slash, qm === -1 ? 99999 : qm, hash === -1 ? 99999 : hash);
            var authority = firstSep < 99999 ? rest.substring(0, firstSep) : rest;
            var pathAndRest = firstSep < 99999 ? rest.substring(firstSep) : '/';
            
            this.host = authority;
            var at = authority.indexOf('@');
            var hostPort = at !== -1 ? authority.substring(at + 1) : authority;
            var colon = hostPort.lastIndexOf(':');
            if (colon !== -1 && hostPort.indexOf(']') === -1) {{
                this.hostname = hostPort.substring(0, colon);
                this.port = hostPort.substring(colon + 1);
            }} else {{
                this.hostname = hostPort;
                this.port = '';
            }}
            
            var qIdx = pathAndRest.indexOf('?');
            var hIdx = pathAndRest.indexOf('#');
            if (qIdx !== -1) {{
                this.pathname = pathAndRest.substring(0, qIdx) || '/';
                if (hIdx !== -1 && hIdx > qIdx) {{
                    this.search = pathAndRest.substring(qIdx, hIdx);
                    this.hash = pathAndRest.substring(hIdx);
                }} else {{
                    this.search = pathAndRest.substring(qIdx);
                    this.hash = '';
                }}
            }} else if (hIdx !== -1) {{
                this.pathname = pathAndRest.substring(0, hIdx) || '/';
                this.search = '';
                this.hash = pathAndRest.substring(hIdx);
            }} else {{
                this.pathname = pathAndRest || '/';
                this.search = '';
                this.hash = '';
            }}
            var segments = this.pathname.split('/');
            var normalized = [];
            for (var si_idx = 0; si_idx < segments.length; si_idx++) {{
                var seg = segments[si_idx];
                if (seg === '' && si_idx > 0 && si_idx < segments.length - 1) continue;
                if (seg === '.') continue;
                if (seg === '..') {{
                    if (normalized.length > 1) {{
                        normalized.pop();
                    }}
                }} else {{
                    normalized.push(seg);
                }}
            }}
            this.pathname = normalized.join('/') || '/';
            if (!this.pathname.startsWith('/')) this.pathname = '/' + this.pathname;
            this.origin = this.protocol + '//' + this.host;
            this._updateHref();
        }},
        _updateHref: function() {{
            this.href = this.origin + this.pathname + this.search + this.hash;
        }},
        _syncFromSearchParams: function() {{
            if (!this._searchParams) return;
            var s = this._searchParams.toString();
            this.search = s.length > 0 ? '?' + s : '';
            this._updateHref();
        }},
        get searchParams() {{
            if (!this._searchParams) {{
                this._searchParams = new URLSearchParams(this.search);
                this._searchParams._url = this;
            }}
            return this._searchParams;
        }},
        toJSON: function() {{
            return this.href;
        }},
        toString: function() {{
            return this.href;
        }}
    }};
    URL.canParse = function(url, base) {{
        try {{
            new URL(url, base);
            return true;
        }} catch(e) {{
            return false;
        }}
    }};
    URL.createObjectURL = function(blob) {{
        var url = 'blob:mango-' + Math.random().toString(36).substring(2);
        if (blob && typeof blob.text === 'function') {{
            try {{
                blob.text().then(function(t) {{
                    if (typeof _mangoBlobRegister === 'function') _mangoBlobRegister(url, t);
                }});
            }} catch(e) {{}}
        }} else if (blob && blob._bytes && typeof _mangoBlobRegister === 'function') {{
            try {{
                var str = (typeof TextDecoder !== 'undefined') ? new TextDecoder().decode(blob._bytes) : String(blob._bytes);
                _mangoBlobRegister(url, str);
            }} catch(e) {{}}
        }}
        return url;
    }};
    URL.revokeObjectURL = function(url) {{}};
    globalThis.URL = URL;

    // ── 14. TextEncoder / TextDecoder ─────────────────────────────────────────
    function TextEncoder() {{
        this.encoding = 'utf-8';
    }}
    TextEncoder.prototype.encode = function(input) {{
        var s = String(input !== undefined ? input : '');
        var out = [];
        for (var i = 0; i < s.length; i++) {{
            var c = s.charCodeAt(i);
            if (c < 0x80) {{
                out.push(c);
            }} else if (c < 0x800) {{
                out.push(0xC0 | (c >> 6), 0x80 | (c & 0x3F));
            }} else if (c >= 0xD800 && c <= 0xDBFF && i + 1 < s.length) {{
                var c2 = s.charCodeAt(i + 1);
                if (c2 >= 0xDC00 && c2 <= 0xDFFF) {{
                    var codePoint = 0x10000 + ((c - 0xD800) << 10) + (c2 - 0xDC00);
                    out.push(0xF0 | (codePoint >> 18), 0x80 | ((codePoint >> 12) & 0x3F), 0x80 | ((codePoint >> 6) & 0x3F), 0x80 | (codePoint & 0x3F));
                    i++;
                    continue;
                }}
                out.push(0xEF, 0xBF, 0xBD);
            }} else {{
                out.push(0xE0 | (c >> 12), 0x80 | ((c >> 6) & 0x3F), 0x80 | (c & 0x3F));
            }}
        }}
        return new Uint8Array(out);
    }};
    TextEncoder.prototype.encodeInto = function(src, dest) {{
        var bytes = this.encode(src);
        var written = Math.min(bytes.length, dest.length);
        dest.set(bytes.subarray(0, written));
        return {{ read: String(src).length, written: written }};
    }};
    globalThis.TextEncoder = TextEncoder;

    function TextDecoder(enc, opts) {{
        this.encoding = enc ? String(enc).toLowerCase() : 'utf-8';
        this.fatal = !!(opts && opts.fatal);
        this.ignoreBOM = !!(opts && opts.ignoreBOM);
    }}
    TextDecoder.prototype.decode = function(input, opts) {{
        if (!input) return '';
        var arr;
        if (input instanceof ArrayBuffer) arr = new Uint8Array(input);
        else if (ArrayBuffer.isView(input)) arr = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
        else arr = new Uint8Array(input);
        
        var s = '';
        var i = 0;
        while (i < arr.length) {{
            var b = arr[i++];
            if (b < 0x80) {{
                s += String.fromCharCode(b);
            }} else if ((b & 0xE0) === 0xC0 && i < arr.length) {{
                var b2 = arr[i++] & 0x3F;
                s += String.fromCharCode(((b & 0x1F) << 6) | b2);
            }} else if ((b & 0xF0) === 0xE0 && i + 1 < arr.length) {{
                var b2 = arr[i++] & 0x3F;
                var b3 = arr[i++] & 0x3F;
                s += String.fromCharCode(((b & 0x0F) << 12) | (b2 << 6) | b3);
            }} else if ((b & 0xF8) === 0xF0 && i + 2 < arr.length) {{
                var b2 = arr[i++] & 0x3F;
                var b3 = arr[i++] & 0x3F;
                var b4 = arr[i++] & 0x3F;
                var code = ((b & 0x07) << 18) | (b2 << 12) | (b3 << 6) | b4;
                code -= 0x10000;
                s += String.fromCharCode(0xD800 + (code >> 10), 0xDC00 + (code & 0x3FF));
            }} else {{
                s += '\uFFFD';
            }}
        }}
        return s;
    }};
    globalThis.TextDecoder = TextDecoder;

    // ── 15. btoa / atob ───────────────────────────────────────────────────────
    var _B64CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    globalThis.btoa = function(str) {{
        var s = String(str);
        var out = '';
        for (var i = 0; i < s.length; i += 3) {{
            var a = s.charCodeAt(i);
            var b = s.charCodeAt(i + 1);
            var c = s.charCodeAt(i + 2);
            if (a > 255 || b > 255 || c > 255) {{
                throw new Error('InvalidCharacterError: String contains characters outside Latin1 range');
            }}
            out += _B64CHARS[a >> 2];
            out += _B64CHARS[((a & 3) << 4) | (isNaN(b) ? 0 : b >> 4)];
            out += isNaN(b) ? '=' : _B64CHARS[((b & 0xF) << 2) | (isNaN(c) ? 0 : c >> 6)];
            out += (isNaN(b) || isNaN(c)) ? '=' : _B64CHARS[c & 0x3F];
        }}
        return out;
    }};
    globalThis.atob = function(str) {{
        var s = String(str).replace(/[\t\n\f\r ]+/g, '');
        if (s.length % 4 !== 0) throw new Error('InvalidCharacterError: Invalid base64 string length');
        var sClean = s.replace(/=+$/, '');
        var out = '';
        for (var i = 0; i < sClean.length; i += 4) {{
            var v0 = _B64CHARS.indexOf(sClean[i]);
            var v1 = _B64CHARS.indexOf(sClean[i + 1]);
            var v2 = i + 2 < sClean.length ? _B64CHARS.indexOf(sClean[i + 2]) : 0;
            var v3 = i + 3 < sClean.length ? _B64CHARS.indexOf(sClean[i + 3]) : 0;
            if (v0 === -1 || v1 === -1 || (i + 2 < sClean.length && v2 === -1) || (i + 3 < sClean.length && v3 === -1)) {{
                throw new Error('InvalidCharacterError: Invalid character in base64 string');
            }}
            var n = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
            out += String.fromCharCode((n >> 16) & 255);
            if (i + 2 < sClean.length) out += String.fromCharCode((n >> 8) & 255);
            if (i + 3 < sClean.length) out += String.fromCharCode(n & 255);
        }}
        return out;
    }};

    // ── 16. crypto ────────────────────────────────────────────────────────────
    function _sha256(data) {{
        var bytes;
        if (typeof data === 'string') bytes = new TextEncoder().encode(data);
        else if (data instanceof ArrayBuffer) bytes = new Uint8Array(data);
        else if (ArrayBuffer.isView(data)) bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        else bytes = new Uint8Array(data);

        function rightRotate(value, amount) {{ return (value >>> amount) | (value << (32 - amount)); }}
        var mathPow = Math.pow;
        var maxWord = mathPow(2, 32);
        var i, j;
        var words = [];
        var asciiBitLength = bytes.length * 8;
        var hash = [];
        var k = [];
        var primeCounter = 0;
        var isComposite = {{}};
        for (var candidate = 2; primeCounter < 64; candidate++) {{
            if (!isComposite[candidate]) {{
                for (i = 0; i < 313; i += candidate) isComposite[i] = candidate;
                hash[primeCounter] = (mathPow(candidate, .5) * maxWord) | 0;
                k[primeCounter++] = (mathPow(candidate, 1 / 3) * maxWord) | 0;
            }}
        }}
        for (i = 0; i < bytes.length; i++) {{
            words[i >> 2] |= bytes[i] << (24 - (i % 4) * 8);
        }}
        words[asciiBitLength >> 5] |= 0x80 << (24 - (asciiBitLength % 32));
        words[(((asciiBitLength + 64) >> 9) << 4) + 15] = asciiBitLength;
        var w = [];
        for (i = 0; i < words.length; i += 16) {{
            var h0 = hash[0], h1 = hash[1], h2 = hash[2], h3 = hash[3], h4 = hash[4], h5 = hash[5], h6 = hash[6], h7 = hash[7];
            for (j = 0; j < 64; j++) {{
                if (j < 16) w[j] = words[j + i] | 0;
                else {{
                    var s0 = rightRotate(w[j - 15], 7) ^ rightRotate(w[j - 15], 18) ^ (w[j - 15] >>> 3);
                    var s1 = rightRotate(w[j - 2], 17) ^ rightRotate(w[j - 2], 19) ^ (w[j - 2] >>> 10);
                    w[j] = (((w[j - 16] + s0) | 0) + ((w[j - 7] + s1) | 0)) | 0;
                }}
                var S1 = rightRotate(h4, 6) ^ rightRotate(h4, 11) ^ rightRotate(h4, 25);
                var ch = (h4 & h5) ^ ((~h4) & h6);
                var temp1 = (((h7 + S1) | 0) + ((ch + k[j]) | 0) + w[j]) | 0;
                var S0 = rightRotate(h0, 2) ^ rightRotate(h0, 13) ^ rightRotate(h0, 22);
                var maj = (h0 & h1) ^ (h0 & h2) ^ (h1 & h2);
                var temp2 = (S0 + maj) | 0;
                h7 = h6; h6 = h5; h5 = h4; h4 = (h3 + temp1) | 0;
                h3 = h2; h2 = h1; h1 = h0; h0 = (temp1 + temp2) | 0;
            }}
            hash[0] = (hash[0] + h0) | 0; hash[1] = (hash[1] + h1) | 0;
            hash[2] = (hash[2] + h2) | 0; hash[3] = (hash[3] + h3) | 0;
            hash[4] = (hash[4] + h4) | 0; hash[5] = (hash[5] + h5) | 0;
            hash[6] = (hash[6] + h6) | 0; hash[7] = (hash[7] + h7) | 0;
        }}
        var out = new Uint8Array(32);
        for (i = 0; i < 8; i++) {{
            out[i * 4] = (hash[i] >> 24) & 0xFF;
            out[i * 4 + 1] = (hash[i] >> 16) & 0xFF;
            out[i * 4 + 2] = (hash[i] >> 8) & 0xFF;
            out[i * 4 + 3] = hash[i] & 0xFF;
        }}
        return out.buffer;
    }}

    var _subtle = {{
        digest: function(algorithm, data) {{
            try {{
                return Promise.resolve(_sha256(data));
            }} catch(e) {{
                return Promise.reject(e);
            }}
        }},
        importKey: function() {{ return Promise.resolve({{}}); }},
        exportKey: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        generateKey: function() {{ return Promise.resolve({{}}); }},
        encrypt: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        decrypt: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        sign: function() {{ return Promise.resolve(new ArrayBuffer(0)); }},
        verify: function() {{ return Promise.resolve(true); }}
    }};

    globalThis.crypto = {{
        subtle: _subtle,
        getRandomValues: function(buf) {{
            if (!buf || (typeof buf.byteLength !== 'number' && typeof buf.length !== 'number')) {{
                throw new TypeError('crypto.getRandomValues: invalid buffer');
            }}
            var len = buf.byteLength !== undefined ? buf.byteLength : buf.length;
            if (len > 65536) {{
                var err = new Error('crypto.getRandomValues: QuotaExceededError - requested length exceeds 65536 bytes');
                err.name = 'QuotaExceededError';
                throw err;
            }}
            if (typeof _mangoRandomBytes === 'function') {{
                var rand = _mangoRandomBytes(buf.length);
                for (var i = 0; i < rand.length; i++) {{
                    buf[i] = rand[i];
                }}
            }} else {{
                for (var i = 0; i < buf.length; i++) buf[i] = Math.floor(Math.random() * 256);
            }}
            return buf;
        }},
        randomUUID: function() {{
            var b = new Uint8Array(16);
            this.getRandomValues(b);
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            var hex = Array.from(b, function(x) {{ return x.toString(16).padStart(2, '0'); }});
            return hex[0]+hex[1]+hex[2]+hex[3]+'-'+hex[4]+hex[5]+'-'+hex[6]+hex[7]+'-'+hex[8]+hex[9]+'-'+hex[10]+hex[11]+hex[12]+hex[13]+hex[14]+hex[15];
        }}
    }};

    // ── 17. structuredClone ───────────────────────────────────────────────────
    globalThis.structuredClone = function(val, options) {{
        var seen = new Map();
        function clone(v) {{
            if (v === null || typeof v !== 'object') return v;
            if (seen.has(v)) return seen.get(v);
            
            if (v instanceof Date) return new Date(v.getTime());
            if (v instanceof RegExp) return new RegExp(v.source, v.flags);
            if (v instanceof ArrayBuffer) return v.slice(0);
            if (ArrayBuffer.isView(v)) {{
                if (v instanceof Uint8Array) return new Uint8Array(v);
                if (v instanceof Int8Array) return new Int8Array(v);
                if (v instanceof Uint16Array) return new Uint16Array(v);
                if (v instanceof Int16Array) return new Int16Array(v);
                if (v instanceof Uint32Array) return new Uint32Array(v);
                if (v instanceof Int32Array) return new Int32Array(v);
                if (v instanceof Float32Array) return new Float32Array(v);
                if (v instanceof Float64Array) return new Float64Array(v);
                return new Uint8Array(v);
            }}
            if (v instanceof Map) {{
                var m = new Map();
                seen.set(v, m);
                v.forEach(function(val, key) {{
                    m.set(clone(key), clone(val));
                }});
                return m;
            }}
            if (v instanceof Set) {{
                var s = new Set();
                seen.set(v, s);
                v.forEach(function(item) {{
                    s.add(clone(item));
                }});
                return s;
            }}
            if (Array.isArray(v)) {{
                var arr = [];
                seen.set(v, arr);
                for (var i = 0; i < v.length; i++) {{
                    arr[i] = clone(v[i]);
                }}
                return arr;
            }}
            
            var copy = {{}};
            seen.set(v, copy);
            var keys = Object.keys(v);
            for (var i = 0; i < keys.length; i++) {{
                var k = keys[i];
                copy[k] = clone(v[k]);
            }}
            return copy;
        }}
        return clone(val);
    }};

    // ── 18. FormData ──────────────────────────────────────────────────────────
    function FormData(form) {{
        this._entries = [];
        if (form && form.elements) {{
            var elements = Array.from(form.elements);
            for (var i = 0; i < elements.length; i++) {{
                var el = elements[i];
                if (el.name && el.value !== undefined && !el.disabled) {{
                    var type = (el.type || '').toLowerCase();
                    if (type === 'checkbox' || type === 'radio') {{
                        if (el.checked) this.append(el.name, el.value || 'on');
                    }} else if (type === 'file') {{
                        if (el.files && el.files.length) {{
                            for (var f = 0; f < el.files.length; f++) {{
                                this.append(el.name, el.files[f]);
                            }}
                        }} else {{
                            this.append(el.name, new File([], '', {{ type: 'application/octet-stream' }}));
                        }}
                    }} else {{
                        this.append(el.name, el.value);
                    }}
                }}
            }}
        }}
    }}
    FormData.prototype = {{
        append: function(k, v, filename) {{
            var val = v;
            if (filename !== undefined && !(val instanceof File)) {{
                val = new File([val], filename);
            }}
            this._entries.push([String(k), val]);
        }},
        set: function(k, v, filename) {{
            var strKey = String(k);
            var val = v;
            if (filename !== undefined && !(val instanceof File)) {{
                val = new File([val], filename);
            }}
            var found = false;
            var newEntries = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) {{
                    if (!found) {{
                        newEntries.push([strKey, val]);
                        found = true;
                    }}
                }} else {{
                    newEntries.push(this._entries[i]);
                }}
            }}
            if (!found) newEntries.push([strKey, val]);
            this._entries = newEntries;
        }},
        delete: function(k) {{
            var strKey = String(k);
            this._entries = this._entries.filter(function(e) {{ return e[0] !== strKey; }});
        }},
        get: function(k) {{
            var strKey = String(k);
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) return this._entries[i][1];
            }}
            return null;
        }},
        getAll: function(k) {{
            var strKey = String(k);
            var res = [];
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) res.push(this._entries[i][1]);
            }}
            return res;
        }},
        has: function(k) {{
            var strKey = String(k);
            for (var i = 0; i < this._entries.length; i++) {{
                if (this._entries[i][0] === strKey) return true;
            }}
            return false;
        }},
        forEach: function(cb, thisArg) {{
            for (var i = 0; i < this._entries.length; i++) {{
                var e = this._entries[i];
                cb.call(thisArg, e[1], e[0], this);
            }}
        }},
        keys: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][0], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        values: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) return {{ value: self._entries[i++][1], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        entries: function() {{
            var i = 0, self = this;
            var it = {{
                next: function() {{
                    if (i < self._entries.length) {{
                        var e = self._entries[i++];
                        return {{ value: [e[0], e[1]], done: false }};
                    }}
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        serializeUrlEncoded: function() {{
            var pairs = [];
            for (var i = 0; i < this._entries.length; i++) {{
                var k = encodeURIComponent(this._entries[i][0]).replace(/%20/g, '+');
                var v = encodeURIComponent(String(this._entries[i][1])).replace(/%20/g, '+');
                pairs.push(k + '=' + v);
            }}
            return pairs.join('&');
        }},
        toUrlSearchParams: function() {{
            if (typeof globalThis.URLSearchParams === 'function') {{
                var usp = new globalThis.URLSearchParams();
                for (var i = 0; i < this._entries.length; i++) {{
                    usp.append(this._entries[i][0], String(this._entries[i][1]));
                }}
                return usp;
            }}
            return null;
        }},
        serializeMultipart: function(boundary) {{
            var b = boundary || ('----WebKitFormBoundary' + Math.random().toString(36).substring(2));
            var lines = [];
            for (var i = 0; i < this._entries.length; i++) {{
                var k = this._entries[i][0];
                var v = this._entries[i][1];
                lines.push('--' + b);
                if (v && typeof v === 'object' && v.name !== undefined) {{
                    var fn = v.name || 'blob';
                    var ct = v.type || 'application/octet-stream';
                    lines.push('Content-Disposition: form-data; name="' + k + '"; filename="' + fn + '"');
                    lines.push('Content-Type: ' + ct);
                    lines.push('');
                    lines.push(v._data ? (typeof v._data === 'string' ? v._data : String(v._data)) : '');
                }} else {{
                    lines.push('Content-Disposition: form-data; name="' + k + '"');
                    lines.push('');
                    lines.push(String(v));
                }}
            }}
            lines.push('--' + b + '--');
            lines.push('');
            return {{
                boundary: b,
                contentType: 'multipart/form-data; boundary=' + b,
                body: lines.join('\r\n')
            }};
        }}
    }};
    FormData.prototype[Symbol.iterator] = FormData.prototype.entries;
    globalThis.FormData = FormData;

    // ── 19. Blob / File / FileReader ──────────────────────────────────────────
    function Blob(parts, opts) {{
        this.type = (opts && opts.type) ? String(opts.type).toLowerCase() : '';
        var byteChunks = [];
        var totalSize = 0;
        if (parts && parts.length) {{
            for (var i = 0; i < parts.length; i++) {{
                var p = parts[i];
                if (p instanceof Blob) {{
                    var chunk = p._getBytes();
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }} else if (p instanceof ArrayBuffer) {{
                    var chunk = new Uint8Array(p);
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }} else if (ArrayBuffer.isView(p)) {{
                    var chunk = new Uint8Array(p.buffer, p.byteOffset, p.byteLength);
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }} else {{
                    var chunk = new TextEncoder().encode(String(p));
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }}
            }}
        }}
        var merged = new Uint8Array(totalSize);
        var offset = 0;
        for (var i = 0; i < byteChunks.length; i++) {{
            merged.set(byteChunks[i], offset);
            offset += byteChunks[i].length;
        }}
        this._bytes = merged;
        this.size = totalSize;
    }}
    Blob.prototype = {{
        _getBytes: function() {{
            return this._bytes;
        }},
        slice: function(start, end, ct) {{
            var len = this.size;
            var s = start === undefined ? 0 : (start < 0 ? Math.max(len + start, 0) : Math.min(start, len));
            var e = end === undefined ? len : (end < 0 ? Math.max(len + end, 0) : Math.min(end, len));
            var span = Math.max(e - s, 0);
            var sub = this._bytes.subarray(s, s + span);
            return new Blob([sub], {{ type: ct !== undefined ? ct : this.type }});
        }},
        text: function() {{
            return Promise.resolve(new TextDecoder().decode(this._bytes));
        }},
        arrayBuffer: function() {{
            var copy = new Uint8Array(this._bytes.length);
            copy.set(this._bytes);
            return Promise.resolve(copy.buffer);
        }},
        bytes: function() {{
            return Promise.resolve(new Uint8Array(this._bytes));
        }},
        stream: function() {{
            return null;
        }}
    }};
    globalThis.Blob = Blob;

    function File(parts, name, opts) {{
        Blob.call(this, parts, opts);
        this.name = String(name || '');
        this.lastModified = (opts && opts.lastModified) ? Number(opts.lastModified) : Date.now();
        this.webkitRelativePath = '';
    }}
    File.prototype = Object.create(Blob.prototype);
    globalThis.File = File;

    function FileReader() {{
        EventTarget.call(this);
        this.readyState = 0; // EMPTY
        this.result = null;
        this.error = null;
        this.onloadstart = null;
        this.onprogress = null;
        this.onload = null;
        this.onabort = null;
        this.onerror = null;
        this.onloadend = null;
        this._aborted = false;
    }}
    FileReader.EMPTY   = 0;
    FileReader.LOADING = 1;
    FileReader.DONE    = 2;
    FileReader.prototype = Object.create(EventTarget.prototype);
    Object.assign(FileReader.prototype, {{ EMPTY: 0, LOADING: 1, DONE: 2 }});

    FileReader.prototype.readAsText = function(blob, encoding) {{
        this._read(blob, 'text', encoding);
    }};
    FileReader.prototype.readAsDataURL = function(blob) {{
        this._read(blob, 'dataurl');
    }};
    FileReader.prototype.readAsArrayBuffer = function(blob) {{
        this._read(blob, 'arraybuffer');
    }};
    FileReader.prototype.readAsBinaryString = function(blob) {{
        this._read(blob, 'binarystring');
    }};
    FileReader.prototype.abort = function() {{
        this._aborted = true;
        this.readyState = 2; // DONE
        this.result = null;
        this.dispatchEvent({{ type: 'abort', target: this }});
        this.dispatchEvent({{ type: 'loadend', target: this }});
    }};
    FileReader.prototype._read = function(blob, mode, enc) {{
        var self = this;
        if (!(blob instanceof Blob)) {{
            throw new TypeError("Failed to execute read on 'FileReader': parameter 1 is not of type 'Blob'.");
        }}
        this._aborted = false;
        this.readyState = 1; // LOADING
        this.result = null;
        this.error = null;
        this.dispatchEvent({{ type: 'loadstart', target: this }});
        
        var schedule = (typeof queueMicrotask === 'function') ? queueMicrotask : function(fn) {{ setTimeout(fn, 0); }};
        schedule(function() {{
            if (self._aborted) return;
            var bytes = blob._getBytes();
            if (mode === 'text') {{
                self.result = new TextDecoder(enc || 'utf-8').decode(bytes);
            }} else if (mode === 'dataurl') {{
                var b64 = '';
                for (var i = 0; i < bytes.length; i++) b64 += String.fromCharCode(bytes[i]);
                self.result = 'data:' + (blob.type || 'application/octet-stream') + ';base64,' + btoa(b64);
            }} else if (mode === 'arraybuffer') {{
                var copy = new Uint8Array(bytes.length);
                copy.set(bytes);
                self.result = copy.buffer;
            }} else if (mode === 'binarystring') {{
                var bs = '';
                for (var i = 0; i < bytes.length; i++) bs += String.fromCharCode(bytes[i]);
                self.result = bs;
            }}
            self.readyState = 2; // DONE
            self.dispatchEvent({{ type: 'progress', lengthComputable: true, loaded: bytes.length, total: bytes.length, target: self }});
            self.dispatchEvent({{ type: 'load', target: self }});
            self.dispatchEvent({{ type: 'loadend', target: self }});
        }});
    }};
    globalThis.FileReader = FileReader;

    // ── 20. AbortController / AbortSignal ────────────────────────────────────
    function AbortSignal() {{
        EventTarget.call(this);
        this.aborted = false;
        this.reason = undefined;
        this.onabort = null;
    }}
    AbortSignal.prototype = Object.create(EventTarget.prototype);
    AbortSignal.prototype.throwIfAborted = function() {{
        if (this.aborted) throw (this.reason !== undefined ? this.reason : new Error('The operation was aborted.'));
    }};
    AbortSignal.abort = function(reason) {{
        var sig = new AbortSignal();
        sig.aborted = true;
        sig.reason = reason !== undefined ? reason : new Error('This operation was aborted');
        return sig;
    }};
    AbortSignal.timeout = function(ms) {{
        var c = new AbortController();
        setTimeout(function() {{
            c.abort(new Error('TimeoutError: The operation timed out.'));
        }}, ms);
        return c.signal;
    }};
    AbortSignal.any = function(signals) {{
        var sigs = Array.from(signals || []);
        var ac = new AbortController();
        for (var i = 0; i < sigs.length; i++) {{
            var s = sigs[i];
            if (s.aborted) {{
                ac.abort(s.reason);
                return ac.signal;
            }}
            s.addEventListener('abort', function() {{
                ac.abort(this.reason);
            }});
        }}
        return ac.signal;
    }};
    globalThis.AbortSignal = AbortSignal;

    function AbortController() {{
        this.signal = new AbortSignal();
    }}
    AbortController.prototype.abort = function(reason) {{
        if (!this.signal.aborted) {{
            this.signal.aborted = true;
            this.signal.reason = reason !== undefined ? reason : new Error('The operation was aborted.');
            var evt = {{ type: 'abort', target: this.signal, currentTarget: this.signal }};
            this.signal.dispatchEvent(evt);
        }}
    }};
    globalThis.AbortController = AbortController;

    // ── 20b. WHATWG Streams API ──────────────────────────────────────────────
    function ByteLengthQueuingStrategy(options) {{
        this.highWaterMark = (options && options.highWaterMark !== undefined) ? Number(options.highWaterMark) : 0;
    }}
    ByteLengthQueuingStrategy.prototype.size = function(chunk) {{
        return (chunk && chunk.byteLength !== undefined) ? chunk.byteLength : 0;
    }};
    globalThis.ByteLengthQueuingStrategy = ByteLengthQueuingStrategy;

    function CountQueuingStrategy(options) {{
        this.highWaterMark = (options && options.highWaterMark !== undefined) ? Number(options.highWaterMark) : 0;
    }}
    CountQueuingStrategy.prototype.size = function() {{
        return 1;
    }};
    globalThis.CountQueuingStrategy = CountQueuingStrategy;

    function ReadableStreamDefaultReader(stream) {{
        if (!(stream instanceof ReadableStream)) {{
            throw new TypeError("ReadableStreamDefaultReader requires a ReadableStream");
        }}
        if (stream.locked) {{
            throw new TypeError("ReadableStream is already locked");
        }}
        this._stream = stream;
        stream._reader = this;
        this._pendingReads = [];
        var self = this;
        this.closed = new Promise(function(resolve, reject) {{
            self._closedResolve = resolve;
            self._closedReject = reject;
        }});
        if (stream._state === 'closed') {{
            this._closedResolve();
        }} else if (stream._state === 'errored') {{
            this._closedReject(stream._storedError);
        }}
    }}
    ReadableStreamDefaultReader.prototype = {{
        read: function() {{
            var stream = this._stream;
            if (!stream) {{
                return Promise.reject(new TypeError("Reader was released"));
            }}
            stream._disturbed = true;
            if (stream._queue.length > 0) {{
                var chunk = stream._queue.shift();
                return Promise.resolve({{ value: chunk, done: false }});
            }}
            if (stream._state === 'closed') {{
                return Promise.resolve({{ value: undefined, done: true }});
            }}
            if (stream._state === 'errored') {{
                return Promise.reject(stream._storedError);
            }}
            var self = this;
            return new Promise(function(resolve, reject) {{
                self._pendingReads.push({{ resolve: resolve, reject: reject }});
                if (typeof stream._underlyingSource.pull === 'function') {{
                    try {{
                        var p = stream._underlyingSource.pull(stream._controller);
                        if (p && typeof p.then === 'function') {{
                            p.catch(function(err) {{ stream._controller.error(err); }});
                        }}
                    }} catch(e) {{
                        stream._controller.error(e);
                    }}
                }}
            }});
        }},
        releaseLock: function() {{
            if (!this._stream) return;
            if (this._pendingReads.length > 0) {{
                throw new TypeError("Cannot release lock with pending reads");
            }}
            this._stream._reader = null;
            this._stream = null;
        }},
        cancel: function(reason) {{
            if (!this._stream) {{
                return Promise.reject(new TypeError("Reader was released"));
            }}
            return this._stream.cancel(reason);
        }}
    }};
    globalThis.ReadableStreamDefaultReader = ReadableStreamDefaultReader;

    function ReadableStream(underlyingSource, strategy) {{
        underlyingSource = underlyingSource || {{}};
        strategy = strategy || {{}};
        this._state = 'readable';
        this._storedError = undefined;
        this._queue = [];
        this._reader = null;
        this._disturbed = false;

        var self = this;
        var controller = {{
            get desiredSize() {{
                if (self._state === 'errored') return null;
                if (self._state === 'closed') return 0;
                var hwm = (strategy && strategy.highWaterMark !== undefined) ? strategy.highWaterMark : 1;
                return Math.max(0, hwm - self._queue.length);
            }},
            close: function() {{
                if (self._state !== 'readable') return;
                self._state = 'closed';
                if (self._reader) {{
                    while (self._reader._pendingReads.length > 0) {{
                        var r = self._reader._pendingReads.shift();
                        r.resolve({{ value: undefined, done: true }});
                    }}
                    if (self._reader._closedResolve) self._reader._closedResolve();
                }}
            }},
            enqueue: function(chunk) {{
                if (self._state !== 'readable') {{
                    throw new TypeError("Cannot enqueue chunk into non-readable stream");
                }}
                if (self._reader && self._reader._pendingReads.length > 0) {{
                    var r = self._reader._pendingReads.shift();
                    r.resolve({{ value: chunk, done: false }});
                }} else {{
                    self._queue.push(chunk);
                }}
            }},
            error: function(e) {{
                if (self._state !== 'readable') return;
                self._state = 'errored';
                self._storedError = e;
                if (self._reader) {{
                    while (self._reader._pendingReads.length > 0) {{
                        var r = self._reader._pendingReads.shift();
                        r.reject(e);
                    }}
                    if (self._reader._closedReject) self._reader._closedReject(e);
                }}
            }}
        }};
        this._controller = controller;
        this._underlyingSource = underlyingSource;

        if (typeof underlyingSource.start === 'function') {{
            try {{
                var res = underlyingSource.start(controller);
                if (res && typeof res.then === 'function') {{
                    res.catch(function(err) {{ controller.error(err); }});
                }}
            }} catch(e) {{
                controller.error(e);
            }}
        }}
    }}
    Object.defineProperty(ReadableStream.prototype, 'locked', {{
        get: function() {{ return this._reader !== null; }},
        configurable: true,
        enumerable: true
    }});
    ReadableStream.prototype.cancel = function(reason) {{
        if (this._reader !== null) {{
            return Promise.reject(new TypeError("Cannot cancel a locked stream"));
        }}
        this._state = 'closed';
        this._storedError = reason;
        this._queue = [];
        if (typeof this._underlyingSource.cancel === 'function') {{
            return Promise.resolve(this._underlyingSource.cancel(reason));
        }}
        return Promise.resolve();
    }};
    ReadableStream.prototype.getReader = function(options) {{
        if (this._reader !== null) {{
            throw new TypeError("ReadableStream is already locked");
        }}
        return new ReadableStreamDefaultReader(this);
    }};
    ReadableStream.prototype.pipeThrough = function(transform, options) {{
        if (!transform || !transform.writable || !transform.readable) {{
            throw new TypeError("transform must have writable and readable properties");
        }}
        this.pipeTo(transform.writable, options);
        return transform.readable;
    }};
    ReadableStream.prototype.pipeTo = function(destination, options) {{
        var self = this;
        var reader = this.getReader();
        var writer = destination.getWriter();
        return new Promise(function(resolve, reject) {{
            function pump() {{
                reader.read().then(function(res) {{
                    if (res.done) {{
                        writer.close().then(resolve, reject);
                        return;
                    }}
                    writer.write(res.value).then(pump, reject);
                }}).catch(reject);
            }}
            pump();
        }});
    }};
    ReadableStream.prototype.tee = function() {{
        var self = this;
        var branch1Ctrl, branch2Ctrl;
        var b1 = new ReadableStream({{
            start: function(c) {{ branch1Ctrl = c; }}
        }});
        var b2 = new ReadableStream({{
            start: function(c) {{ branch2Ctrl = c; }}
        }});
        var reader = self.getReader();
        function pump() {{
            reader.read().then(function(res) {{
                if (res.done) {{
                    branch1Ctrl.close();
                    branch2Ctrl.close();
                    return;
                }}
                branch1Ctrl.enqueue(res.value);
                branch2Ctrl.enqueue(res.value);
                pump();
            }}).catch(function(err) {{
                branch1Ctrl.error(err);
                branch2Ctrl.error(err);
            }});
        }}
        pump();
        return [b1, b2];
    }};
    if (typeof Symbol !== 'undefined' && Symbol.asyncIterator) {{
        ReadableStream.prototype[Symbol.asyncIterator] = function() {{
            var reader = this.getReader();
            return {{
                next: function() {{
                    return reader.read();
                }},
                return: function(val) {{
                    reader.releaseLock();
                    return Promise.resolve({{ value: val, done: true }});
                }}
            }};
        }};
    }}
    globalThis.ReadableStream = ReadableStream;

    function WritableStreamDefaultWriter(stream) {{
        if (!(stream instanceof WritableStream)) {{
            throw new TypeError("Expected WritableStream");
        }}
        if (stream.locked) throw new TypeError("WritableStream is locked");
        this._stream = stream;
        stream._writer = this;
        var self = this;
        this.closed = new Promise(function(res, rej) {{
            self._closedResolve = res;
            self._closedReject = rej;
        }});
        this.ready = Promise.resolve();
    }}
    Object.defineProperty(WritableStreamDefaultWriter.prototype, 'desiredSize', {{
        get: function() {{ return 1; }}
    }});
    WritableStreamDefaultWriter.prototype.write = function(chunk) {{
        if (!this._stream) return Promise.reject(new TypeError("Writer released"));
        var sink = this._stream._sink;
        if (typeof sink.write === 'function') {{
            return Promise.resolve(sink.write(chunk, this._stream._controller));
        }}
        return Promise.resolve();
    }};
    WritableStreamDefaultWriter.prototype.close = function() {{
        if (!this._stream) return Promise.reject(new TypeError("Writer released"));
        var self = this;
        return this._stream._closeInternal().then(function() {{
            self._closedResolve();
        }});
    }};
    WritableStreamDefaultWriter.prototype.abort = function(reason) {{
        if (!this._stream) return Promise.reject(new TypeError("Writer released"));
        return this._stream.abort(reason);
    }};
    WritableStreamDefaultWriter.prototype.releaseLock = function() {{
        if (!this._stream) return;
        this._stream._writer = null;
        this._stream = null;
    }};
    globalThis.WritableStreamDefaultWriter = WritableStreamDefaultWriter;

    function WritableStream(underlyingSink, strategy) {{
        underlyingSink = underlyingSink || {{}};
        this._sink = underlyingSink;
        this._strategy = strategy || {{}};
        this._state = 'writable';
        this._storedError = undefined;
        this._writer = null;
        var self = this;
        var controller = {{
            error: function(e) {{
                self._state = 'errored';
                self._storedError = e;
            }}
        }};
        this._controller = controller;
        if (typeof underlyingSink.start === 'function') {{
            try {{ underlyingSink.start(controller); }} catch(e) {{ controller.error(e); }}
        }}
    }}
    Object.defineProperty(WritableStream.prototype, 'locked', {{
        get: function() {{ return this._writer !== null; }},
        configurable: true,
        enumerable: true
    }});
    WritableStream.prototype.abort = function(reason) {{
        this._state = 'errored';
        this._storedError = reason;
        if (typeof this._sink.abort === 'function') {{
            return Promise.resolve(this._sink.abort(reason));
        }}
        return Promise.resolve();
    }};
    WritableStream.prototype.close = function() {{
        if (this.locked) return Promise.reject(new TypeError("Stream is locked"));
        return this._closeInternal();
    }};
    WritableStream.prototype._closeInternal = function() {{
        this._state = 'closed';
        if (typeof this._sink.close === 'function') {{
            return Promise.resolve(this._sink.close());
        }}
        return Promise.resolve();
    }};
    WritableStream.prototype.getWriter = function() {{
        if (this.locked) throw new TypeError("WritableStream is locked");
        return new WritableStreamDefaultWriter(this);
    }};
    globalThis.WritableStream = WritableStream;

    function TransformStream(transformer, writableStrategy, readableStrategy) {{
        transformer = transformer || {{}};
        var readableCtrl;
        var self = this;
        var r = new ReadableStream({{
            start: function(c) {{ readableCtrl = c; }}
        }}, readableStrategy);

        var controller = {{
            get desiredSize() {{ return readableCtrl.desiredSize; }},
            enqueue: function(chunk) {{ readableCtrl.enqueue(chunk); }},
            error: function(reason) {{ readableCtrl.error(reason); }},
            terminate: function() {{ readableCtrl.close(); }}
        }};

        var w = new WritableStream({{
            start: function(c) {{
                if (typeof transformer.start === 'function') {{
                    return transformer.start(controller);
                }}
            }},
            write: function(chunk, c) {{
                if (typeof transformer.transform === 'function') {{
                    return transformer.transform(chunk, controller);
                }}
                controller.enqueue(chunk);
            }},
            close: function() {{
                if (typeof transformer.flush === 'function') {{
                    return Promise.resolve(transformer.flush(controller)).then(function() {{
                        controller.terminate();
                    }});
                }}
                controller.terminate();
            }},
            abort: function(reason) {{
                controller.error(reason);
            }}
        }}, writableStrategy);

        this.readable = r;
        this.writable = w;
    }}
    globalThis.TransformStream = TransformStream;

    // ── 21. fetch() ───────────────────────────────────────────────────────────
    function Headers(init) {{
        this._map = {{}};
        var self = this;
        if (init) {{
            if (init instanceof Headers || (init._map && typeof init._map === 'object')) {{
                for (var k in init._map) {{
                    this._map[k] = init._map[k];
                }}
            }} else if (Array.isArray(init)) {{
                for (var i = 0; i < init.length; i++) {{
                    var p = init[i];
                    if (p && p.length >= 2) this.append(p[0], p[1]);
                }}
            }} else if (typeof init === 'object') {{
                var keys = Object.keys(init);
                for (var i = 0; i < keys.length; i++) {{
                    var k = keys[i];
                    this.append(k, init[k]);
                }}
            }}
        }}
    }}
    Headers.prototype = {{
        append: function(k, v) {{
            var lk = String(k).toLowerCase();
            this._map[lk] = this._map[lk] ? this._map[lk] + ', ' + String(v) : String(v);
        }},
        delete: function(k) {{
            delete this._map[String(k).toLowerCase()];
        }},
        get: function(k) {{
            var lk = String(k).toLowerCase();
            return this._map[lk] !== undefined ? this._map[lk] : null;
        }},
        getSetCookie: function() {{
            var c = this.get('set-cookie');
            return c ? c.split(', ') : [];
        }},
        has: function(k) {{
            return String(k).toLowerCase() in this._map;
        }},
        set: function(k, v) {{
            this._map[String(k).toLowerCase()] = String(v);
        }},
        forEach: function(cb, thisArg) {{
            for (var k in this._map) {{
                cb.call(thisArg, this._map[k], k, this);
            }}
        }},
        keys: function() {{
            var keys = Object.keys(this._map);
            var i = 0;
            var it = {{
                next: function() {{
                    if (i < keys.length) return {{ value: keys[i++], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        values: function() {{
            var keys = Object.keys(this._map);
            var self = this;
            var i = 0;
            var it = {{
                next: function() {{
                    if (i < keys.length) return {{ value: self._map[keys[i++]], done: false }};
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }},
        entries: function() {{
            var keys = Object.keys(this._map);
            var self = this;
            var i = 0;
            var it = {{
                next: function() {{
                    if (i < keys.length) {{
                        var k = keys[i++];
                        return {{ value: [k, self._map[k]], done: false }};
                    }}
                    return {{ value: undefined, done: true }};
                }}
            }};
            it[Symbol.iterator] = function() {{ return this; }};
            return it;
        }}
    }};
    Headers.prototype[Symbol.iterator] = Headers.prototype.entries;
    globalThis.Headers = Headers;

    function Request(input, init) {{
        init = init || {{}};
        if (input instanceof Request) {{
            this.url = input.url;
            this.method = init.method ? String(init.method).toUpperCase() : input.method;
            this.headers = new Headers(init.headers || input.headers);
            this.body = init.body !== undefined ? init.body : input.body;
            this.signal = init.signal || input.signal;
        }} else {{
            this.url = String(input || '');
            this.method = init.method ? String(init.method).toUpperCase() : 'GET';
            this.headers = new Headers(init.headers);
            this.body = init.body !== undefined ? init.body : null;
            this.signal = init.signal || null;
        }}
        this.mode = init.mode || 'cors';
        this.credentials = init.credentials || 'same-origin';
        this.cache = init.cache || 'default';
        this.redirect = init.redirect || 'follow';
        this.referrer = init.referrer || 'about:client';
        this.destination = init.destination || '';
        this.bodyUsed = false;
    }}
    Request.prototype = {{
        clone: function() {{
            return new Request(this, {{}});
        }},
        text: function() {{
            this.bodyUsed = true;
            return Promise.resolve(String(this.body || ''));
        }},
        json: function() {{
            this.bodyUsed = true;
            try {{ return Promise.resolve(JSON.parse(this.body || 'null')); }}
            catch(e) {{ return Promise.reject(e); }}
        }}
    }};
    globalThis.Request = Request;

    function Response(body, init) {{
        if (body instanceof ReadableStream) {{
            this._bodyStream = body;
            this._body = null;
        }} else {{
            this._body = body !== undefined && body !== null ? body : '';
            this._bodyStream = null;
        }}
        this.status = (init && init.status !== undefined) ? Number(init.status) : 200;
        this.statusText = (init && init.statusText !== undefined) ? String(init.statusText) : (this.status === 200 ? 'OK' : String(this.status));
        this.ok = this.status >= 200 && this.status < 300;
        this.headers = new Headers(init && init.headers);
        this.url = (init && init.url) ? String(init.url) : '';
        this.type = (init && init.type) ? String(init.type) : 'default';
        this.redirected = !!(init && init.redirected);
        this._bodyUsed = false;
    }}
    Object.defineProperty(Response.prototype, 'bodyUsed', {{
        get: function() {{
            return !!this._bodyUsed || (this._bodyStream ? this._bodyStream._disturbed : false);
        }},
        set: function(val) {{
            this._bodyUsed = !!val;
        }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(Response.prototype, 'body', {{
        get: function() {{
            if (this._bodyStream) return this._bodyStream;
            var self = this;
            var rawBody = this._body;
            var rawBase64 = this._raw && this._raw.bodyBase64;
            this._bodyStream = new ReadableStream({{
                start: function(controller) {{
                    var chunk = null;
                    if (rawBase64 && typeof atob === 'function') {{
                        try {{
                            var bin = atob(rawBase64);
                            chunk = new Uint8Array(bin.length);
                            for (var i = 0; i < bin.length; i++) chunk[i] = bin.charCodeAt(i);
                        }} catch(e) {{}}
                    }}
                    if (!chunk) {{
                        if (rawBody instanceof Uint8Array) {{
                            chunk = rawBody;
                        }} else if (rawBody instanceof ArrayBuffer) {{
                            chunk = new Uint8Array(rawBody);
                        }} else if (ArrayBuffer.isView(rawBody)) {{
                            chunk = new Uint8Array(rawBody.buffer, rawBody.byteOffset, rawBody.byteLength);
                        }} else if (typeof rawBody === 'string' && rawBody.length > 0) {{
                            chunk = new TextEncoder().encode(rawBody);
                        }}
                    }}
                    if (chunk && chunk.length > 0) {{
                        controller.enqueue(chunk);
                    }}
                    controller.close();
                }}
            }});
            return this._bodyStream;
        }},
        configurable: true,
        enumerable: true
    }});
    Response.prototype.text = function() {{
        if (this.bodyUsed && !this._bodyStream) {{
            return Promise.reject(new TypeError("Body already used"));
        }}
        this.bodyUsed = true;
        if (this._bodyStream && this._body === null) {{
            var reader = this._bodyStream.getReader();
            var chunks = [];
            function drain() {{
                return reader.read().then(function(res) {{
                    if (res.done) {{
                        var totalLen = 0;
                        for (var i = 0; i < chunks.length; i++) totalLen += chunks[i].length;
                        var merged = new Uint8Array(totalLen);
                        var off = 0;
                        for (var i = 0; i < chunks.length; i++) {{
                            merged.set(chunks[i], off);
                            off += chunks[i].length;
                        }}
                        return new TextDecoder().decode(merged);
                    }}
                    if (typeof res.value === 'string') {{
                        chunks.push(new TextEncoder().encode(res.value));
                    }} else if (res.value instanceof Uint8Array) {{
                        chunks.push(res.value);
                    }} else if (res.value && res.value.buffer) {{
                        chunks.push(new Uint8Array(res.value.buffer));
                    }}
                    return drain();
                }});
            }}
            return drain();
        }}
        if (typeof this._body === 'string') return Promise.resolve(this._body);
        if (this._body instanceof Blob) return this._body.text();
        if (this._body instanceof ArrayBuffer || ArrayBuffer.isView(this._body)) {{
            return Promise.resolve(new TextDecoder().decode(this._body));
        }}
        return Promise.resolve(String(this._body));
    }};
    Response.prototype.json = function() {{
        var self = this;
        return this.text().then(function(t) {{
            return JSON.parse(t);
        }});
    }};
    Response.prototype.blob = function() {{
        this.bodyUsed = true;
        if (this._body instanceof Blob) return Promise.resolve(this._body);
        var ct = this.headers.get('content-type') || 'text/plain';
        return Promise.resolve(new Blob([this._body], {{ type: ct }}));
    }};
    Response.prototype.arrayBuffer = function() {{
        this.bodyUsed = true;
        if (this._body instanceof ArrayBuffer) return Promise.resolve(this._body);
        if (ArrayBuffer.isView(this._body)) return Promise.resolve(this._body.buffer);
        if (this._body instanceof Blob) return this._body.arrayBuffer();
        var enc = new TextEncoder().encode(String(this._body));
        return Promise.resolve(enc.buffer);
    }};
    Response.prototype.formData = function() {{
        this.bodyUsed = true;
        var fd = new FormData();
        return this.text().then(function(t) {{
            var params = new URLSearchParams(t);
            params.forEach(function(v, k) {{ fd.append(k, v); }});
            return fd;
        }});
    }};
    Response.prototype.clone = function() {{
        return new Response(this._body, {{
            status: this.status,
            statusText: this.statusText,
            headers: this.headers,
            url: this.url,
            type: this.type,
            redirected: this.redirected
        }});
    }};
    Response.error = function() {{
        var r = new Response(null, {{ status: 0, statusText: '' }});
        r.type = 'error';
        r.ok = false;
        return r;
    }};
    Response.redirect = function(url, status) {{
        status = status || 302;
        if ([301, 302, 303, 307, 308].indexOf(status) === -1) {{
            throw new RangeError('Invalid redirect status');
        }}
        return new Response(null, {{ status: status, headers: {{ Location: String(url) }} }});
    }};
    Response.json = function(data, init) {{
        var body = JSON.stringify(data);
        var hdrs = new Headers(init && init.headers);
        if (!hdrs.has('content-type')) hdrs.set('content-type', 'application/json');
        var opts = Object.assign({{}}, init, {{ headers: hdrs }});
        return new Response(body, opts);
    }};
    globalThis.Response = Response;

    // ── Cache API (caches.open, caches.match, cache.put, cache.delete, cache.keys) ──
    function Cache() {{
        this._items = [];
    }}
    Cache.prototype = {{
        match: function(request, options) {{
            var url = (request instanceof Request) ? request.url : String(request);
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = ignoreSearch ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {{
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (cmpUrl === target) {{
                    return Promise.resolve(this._items[i].res.clone());
                }}
            }}
            return Promise.resolve(undefined);
        }},
        matchAll: function(request, options) {{
            var results = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = (url && ignoreSearch) ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {{
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (!target || cmpUrl === target) {{
                    results.push(this._items[i].res.clone());
                }}
            }}
            return Promise.resolve(results);
        }},
        add: function(request) {{
            return this.addAll([request]);
        }},
        addAll: function(requests) {{
            var self = this;
            var promises = requests.map(function(r) {{ return fetch(r); }});
            return Promise.all(promises).then(function(responses) {{
                for (var i = 0; i < requests.length; i++) {{
                    var req = (requests[i] instanceof Request) ? requests[i] : new Request(requests[i]);
                    self.put(req, responses[i]);
                }}
            }});
        }},
        put: function(request, response) {{
            var req = (request instanceof Request) ? request : new Request(request);
            var res = response.clone();
            for (var i = 0; i < this._items.length; i++) {{
                if (this._items[i].req.url === req.url) {{
                    this._items[i] = {{ req: req, res: res }};
                    return Promise.resolve();
                }}
            }}
            this._items.push({{ req: req, res: res }});
            return Promise.resolve();
        }},
        delete: function(request, options) {{
            var url = (request instanceof Request) ? request.url : String(request);
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = ignoreSearch ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {{
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (cmpUrl === target) {{
                    this._items.splice(i, 1);
                    return Promise.resolve(true);
                }}
            }}
            return Promise.resolve(false);
        }},
        keys: function(request, options) {{
            var keys = [];
            var url = request ? ((request instanceof Request) ? request.url : String(request)) : null;
            var ignoreSearch = !!(options && options.ignoreSearch);
            var target = (url && ignoreSearch) ? url.split('?')[0] : url;
            for (var i = 0; i < this._items.length; i++) {{
                var itemUrl = this._items[i].req.url;
                var cmpUrl = ignoreSearch ? itemUrl.split('?')[0] : itemUrl;
                if (!target || cmpUrl === target) {{
                    keys.push(this._items[i].req.clone());
                }}
            }}
            return Promise.resolve(keys);
        }}
    }};
    globalThis.Cache = Cache;

    function CacheStorage() {{
        this._caches = {{}};
    }}
    CacheStorage.prototype = {{
        open: function(cacheName) {{
            var name = String(cacheName);
            if (!this._caches[name]) {{
                this._caches[name] = new Cache();
            }}
            return Promise.resolve(this._caches[name]);
        }},
        has: function(cacheName) {{
            return Promise.resolve(Object.prototype.hasOwnProperty.call(this._caches, String(cacheName)));
        }},
        delete: function(cacheName) {{
            var name = String(cacheName);
            if (Object.prototype.hasOwnProperty.call(this._caches, name)) {{
                delete this._caches[name];
                return Promise.resolve(true);
            }}
            return Promise.resolve(false);
        }},
        keys: function() {{
            return Promise.resolve(Object.keys(this._caches));
        }},
        match: function(request, options) {{
            var self = this;
            if (options && options.cacheName) {{
                var single = this._caches[String(options.cacheName)];
                if (single) return single.match(request, options);
                return Promise.resolve(undefined);
            }}
            var keys = Object.keys(this._caches);
            function checkNext(idx) {{
                if (idx >= keys.length) return Promise.resolve(undefined);
                return self._caches[keys[idx]].match(request, options).then(function(res) {{
                    if (res) return res;
                    return checkNext(idx + 1);
                }});
            }}
            return checkNext(0);
        }}
    }};
    globalThis.CacheStorage = CacheStorage;
    globalThis.caches = new CacheStorage();

    var _nextFetchId = 1;
    var _pendingFetches = {{}};

    globalThis._mangoResolveFetch = function(id, rawJson) {{
        var entry = _pendingFetches[id];
        if (!entry) return;
        delete _pendingFetches[id];

        var raw;
        try {{
            raw = typeof rawJson === 'string' ? JSON.parse(rawJson) : rawJson;
        }} catch(e) {{
            entry.reject(new TypeError('fetch: invalid response from native bridge: ' + e));
            return;
        }}

        if (raw.error) {{
            entry.reject(new TypeError('fetch failed: ' + raw.error));
        }} else {{
            var res = _makeResponse(raw);
            if (typeof PerformanceResourceTiming !== 'undefined' && typeof _perfEntries !== 'undefined' && typeof _p0 !== 'undefined') {{
                var dur = Math.max(0, (Date.now() - _p0) - (entry.startTime || 0));
                _perfEntries.push(new PerformanceResourceTiming(entry.url || '', 'fetch', entry.startTime || 0, dur, raw.status || 200));
            }}
            entry.resolve(res);
        }}
    }};

    function _makeResponse(raw) {{
        var hdrs = new Headers();
        if (raw.headers) {{
            for (var k in raw.headers) {{
                hdrs.set(k, raw.headers[k]);
            }}
        }}
        if (!hdrs.has('content-type') && raw.contentType) {{
            hdrs.set('content-type', raw.contentType);
        }}

        var r = new Response(raw.body || '', {{
            status: raw.status,
            statusText: raw.statusText || (raw.status === 200 ? 'OK' : String(raw.status)),
            headers: hdrs
        }});
        r.ok = raw.ok;
        r.type = 'basic';
        r._raw = raw;

        if (raw.bodyBase64 && typeof atob === 'function') {{
            r.arrayBuffer = function() {{
                this.bodyUsed = true;
                try {{
                    var bin = atob(raw.bodyBase64);
                    var buf = new Uint8Array(bin.length);
                    for (var i = 0; i < bin.length; i++) {{
                        buf[i] = bin.charCodeAt(i);
                    }}
                    return Promise.resolve(buf.buffer);
                }} catch(e) {{
                    var enc = new TextEncoder().encode(raw.body || '');
                    return Promise.resolve(enc.buffer);
                }}
            }};
            r.blob = function() {{
                this.bodyUsed = true;
                return this.arrayBuffer().then(function(buf) {{
                    return new Blob([buf], {{ type: raw.contentType || 'application/octet-stream' }});
                }});
            }};
        }}
        return r;
    }}

    globalThis.fetch = function(input, init) {{
        var req;
        if (input instanceof Request) {{
            req = init ? new Request(input, init) : input;
        }} else {{
            req = new Request(input, init);
        }}
        var signal = req.signal;
        if (signal && signal.aborted) {{
            return Promise.reject(new Error('The user aborted a request.'));
        }}
        var url = req.url;
        if (url && url.indexOf('://') === -1 && url.indexOf('//') !== 0) {{
            try {{
                var base = globalThis.location ? globalThis.location.origin + (globalThis.location.pathname || '/') : '';
                url = new URL(url, base).href;
            }} catch(e) {{}}
        }}
        
        var hdrs = {{}};
        req.headers.forEach(function(v, k) {{ hdrs[k] = v; }});
        var bodyStr = (req.body !== undefined && req.body !== null && !(req.body instanceof ReadableStream)) ? String(req.body) : '';
        var pageOrigin = (globalThis.location && globalThis.location.origin) ? globalThis.location.origin : '';
        var reqMode = req.mode || (init && init.mode) || 'cors';
        var reqCreds = (req.credentials === 'include' || (init && init.credentials === 'include'));
        if (init && init._swBypass) req._swBypass = true;

        // Service Worker Fetch Event Interception
        if (!req._swBypass && typeof FetchEvent !== 'undefined' && globalThis.navigator && globalThis.navigator.serviceWorker && globalThis.navigator.serviceWorker.controller) {{
            var sw = globalThis.navigator.serviceWorker.controller;
            var hasFetchHandler = (sw._listeners && sw._listeners['fetch'] && sw._listeners['fetch'].length > 0) || (typeof sw.onfetch === 'function');
            if (hasFetchHandler && !sw._handlingFetch) {{
                try {{
                    sw._handlingFetch = true;
                    var fe = new FetchEvent('fetch', {{ request: req }});
                    sw.dispatchEvent(fe);
                    if (fe._responded && fe._responsePromise) {{
                        return fe._responsePromise;
                    }}
                }} finally {{
                    sw._handlingFetch = false;
                }}
            }}
        }}

        function performFetch(bodyPayload) {{
            return new Promise(function(resolve, reject) {{
                if (signal) {{
                    signal.addEventListener('abort', function() {{
                        reject(new Error('The user aborted a request.'));
                    }});
                }}

                var fetchStart = (typeof _p0 !== 'undefined') ? (Date.now() - _p0) : 0;
                if (typeof _mangoStartFetch === 'function') {{
                    var fetchId = _nextFetchId++;
                    _pendingFetches[fetchId] = {{ resolve: resolve, reject: reject, url: url, startTime: fetchStart }};
                    _mangoStartFetch(fetchId, url, req.method, JSON.stringify(hdrs), bodyPayload, pageOrigin, reqMode, reqCreds);
                }} else if (typeof _mangoFetch === 'function') {{
                    try {{
                        var resultJson = _mangoFetch(url, req.method, JSON.stringify(hdrs), bodyPayload, pageOrigin, reqMode, reqCreds);
                        var raw = JSON.parse(resultJson);
                        if (raw.error) reject(new TypeError('fetch failed: ' + raw.error));
                        else {{
                            var res = _makeResponse(raw);
                            if (typeof PerformanceResourceTiming !== 'undefined' && typeof _perfEntries !== 'undefined' && typeof _p0 !== 'undefined') {{
                                var dur = Math.max(0, (Date.now() - _p0) - fetchStart);
                                _perfEntries.push(new PerformanceResourceTiming(url, 'fetch', fetchStart, dur, raw.status || 200));
                            }}
                            resolve(res);
                        }}
                    }} catch(e) {{
                        reject(new TypeError('fetch error: ' + (e.message || e)));
                    }}
                }} else {{
                    reject(new TypeError('fetch: native bridge unavailable'));
                }}
            }});
        }}

        if (req.body instanceof ReadableStream) {{
            var reader = req.body.getReader();
            var streamChunks = [];
            function drainStream() {{
                return reader.read().then(function(res) {{
                    if (res.done) {{
                        var totalLen = 0;
                        for (var i = 0; i < streamChunks.length; i++) totalLen += streamChunks[i].length;
                        var merged = new Uint8Array(totalLen);
                        var off = 0;
                        for (var i = 0; i < streamChunks.length; i++) {{
                            merged.set(streamChunks[i], off);
                            off += streamChunks[i].length;
                        }}
                        return new TextDecoder().decode(merged);
                    }}
                    if (typeof res.value === 'string') {{
                        streamChunks.push(new TextEncoder().encode(res.value));
                    }} else if (res.value instanceof Uint8Array) {{
                        streamChunks.push(res.value);
                    }} else if (res.value && res.value.buffer) {{
                        streamChunks.push(new Uint8Array(res.value.buffer));
                    }}
                    return drainStream();
                }});
            }}
            return drainStream().then(function(drainedText) {{
                return performFetch(drainedText);
            }});
        }}

        return performFetch(bodyStr);
    }};

    // ── 22. XMLHttpRequest (legacy compatibility) ─────────────────────────────
    function XMLHttpRequestUpload() {{
        EventTarget.call(this);
        this.onloadstart = null;
        this.onprogress  = null;
        this.onabort     = null;
        this.onerror     = null;
        this.onload      = null;
        this.ontimeout   = null;
        this.onloadend   = null;
    }}
    XMLHttpRequestUpload.prototype = Object.create(EventTarget.prototype);
    globalThis.XMLHttpRequestUpload = XMLHttpRequestUpload;

    function XMLHttpRequest() {{
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
        this._headers      = {{}};
        this._responseHeaders = {{}};
        this._aborted      = false;
    }}
    XMLHttpRequest.prototype = Object.create(EventTarget.prototype);
    var _xhrConsts = {{ UNSENT: 0, OPENED: 1, HEADERS_RECEIVED: 2, LOADING: 3, DONE: 4 }};
    Object.assign(XMLHttpRequest, _xhrConsts);
    Object.assign(XMLHttpRequest.prototype, _xhrConsts);

    XMLHttpRequest.prototype.open = function(method, url, async, user, password) {{
        this._method = String(method || 'GET').toUpperCase();
        this._url    = String(url || '');
        this._async  = async !== false;
        this._aborted = false;
        this._setReadyState(1 /* OPENED */);
    }};
    XMLHttpRequest.prototype.setRequestHeader = function(k, v) {{
        var lk = String(k).toLowerCase();
        this._headers[lk] = this._headers[lk] ? this._headers[lk] + ', ' + String(v) : String(v);
    }};
    XMLHttpRequest.prototype.getResponseHeader = function(k) {{
        if (this.readyState < 2) return null;
        var lk = String(k).toLowerCase();
        return this._responseHeaders[lk] !== undefined ? this._responseHeaders[lk] : null;
    }};
    XMLHttpRequest.prototype.getAllResponseHeaders = function() {{
        if (this.readyState < 2) return '';
        var lines = [];
        for (var k in this._responseHeaders) {{
            lines.push(k + ': ' + this._responseHeaders[k]);
        }}
        return lines.join('\r\n') + (lines.length > 0 ? '\r\n' : '');
    }};
    XMLHttpRequest.prototype.overrideMimeType = function(m) {{
        this._overrideMime = m;
    }};
    XMLHttpRequest.prototype.abort = function() {{
        if (this.readyState > 0 && this.readyState < 4) {{
            this._aborted = true;
            this._setReadyState(4);
            var evt = {{ type: 'abort', target: this, currentTarget: this }};
            this.dispatchEvent(evt);
            var endEvt = {{ type: 'loadend', target: this, currentTarget: this }};
            this.dispatchEvent(endEvt);
        }}
    }};
    XMLHttpRequest.prototype.send = function(body) {{
        var self = this;
        if (this._aborted) return;
        this.dispatchEvent({{ type: 'loadstart', target: this }});
        this.upload.dispatchEvent({{ type: 'loadstart', target: this.upload }});
        
        var hdrs = JSON.stringify(this._headers);
        var b    = (body !== undefined && body !== null) ? String(body) : '';
        var url  = this._url;
        if (url && url.indexOf('://') === -1 && url.indexOf('//') !== 0) {{
            try {{
                var base = globalThis.location ? globalThis.location.origin + (globalThis.location.pathname || '/') : '';
                url = new URL(url, base).href;
            }} catch(e) {{}}
        }}
        
        setTimeout(function() {{
            if (self._aborted) return;
            try {{
                self.upload.dispatchEvent({{ type: 'progress', lengthComputable: true, loaded: b.length, total: b.length, target: self.upload }});
                self.upload.dispatchEvent({{ type: 'load', target: self.upload }});
                self.upload.dispatchEvent({{ type: 'loadend', target: self.upload }});
                
                var result = JSON.parse(_mangoFetch(url, self._method, hdrs, b));
                self._setReadyState(2 /* HEADERS_RECEIVED */);
                self.status       = result.status;
                self.statusText   = result.status === 200 ? 'OK' : String(result.status);
                self.responseURL  = url;
                if (result.contentType) {{
                    self._responseHeaders['content-type'] = result.contentType;
                }}
                
                self._setReadyState(3 /* LOADING */);
                var bodyText = result.body || '';
                self.responseText = bodyText;
                if (self.responseType === 'json') {{
                    try {{ self.response = JSON.parse(bodyText); }} catch(_) {{ self.response = null; }}
                }} else if (self.responseType === 'arraybuffer') {{
                    self.response = new TextEncoder().encode(bodyText).buffer;
                }} else if (self.responseType === 'blob') {{
                    self.response = new Blob([bodyText], {{ type: result.contentType || 'text/plain' }});
                }} else {{
                    self.response = bodyText;
                }}
                
                self._setReadyState(4 /* DONE */);
                self.dispatchEvent({{ type: 'progress', lengthComputable: true, loaded: bodyText.length, total: bodyText.length, target: self }});
                if (result.ok) {{
                    self.dispatchEvent({{ type: 'load', target: self }});
                }} else {{
                    self.dispatchEvent({{ type: 'error', target: self }});
                }}
                self.dispatchEvent({{ type: 'loadend', target: self }});
            }} catch(e) {{
                self.status = 0;
                self._setReadyState(4 /* DONE */);
                self.dispatchEvent({{ type: 'error', target: self, message: e.message }});
                self.dispatchEvent({{ type: 'loadend', target: self }});
            }}
        }}, 0);
    }};
    XMLHttpRequest.prototype._setReadyState = function(rs) {{
        this.readyState = rs;
        this.dispatchEvent({{ type: 'readystatechange', target: this }});
    }};
    globalThis.XMLHttpRequest = XMLHttpRequest;

    // ── 23. WebSocket ─────────────────────────────────────────────────────────
    var _nextWsId = 1;
    var _activeWebSockets = {{}};

    globalThis._mangoDispatchWsOpen = function(id, protocol) {{
        var ws = _activeWebSockets[id];
        if (!ws) return;
        ws.readyState = 1; // OPEN
        ws.protocol = protocol || '';
        var evt = (typeof globalThis.Event === 'function')
            ? new globalThis.Event('open')
            : {{ type: 'open', target: ws }};
        if (typeof ws.onopen === 'function') try {{ ws.onopen(evt); }} catch(e) {{}}
        ws.dispatchEvent(evt);
    }};

    globalThis._mangoDispatchWsMessage = function(id, data) {{
        var ws = _activeWebSockets[id];
        if (!ws) return;
        var evt = (typeof globalThis.MessageEvent === 'function')
            ? new globalThis.MessageEvent('message', {{ data: data, origin: ws.url, source: ws }})
            : {{ type: 'message', data: data, origin: ws.url, target: ws }};
        if (typeof ws.onmessage === 'function') try {{ ws.onmessage(evt); }} catch(e) {{}}
        ws.dispatchEvent(evt);
    }};

    globalThis._mangoDispatchWsError = function(id, msg) {{
        var ws = _activeWebSockets[id];
        if (!ws) return;
        var evt = (typeof globalThis.ErrorEvent === 'function')
            ? new globalThis.ErrorEvent('error', {{ message: msg }})
            : {{ type: 'error', message: msg, target: ws }};
        if (typeof ws.onerror === 'function') try {{ ws.onerror(evt); }} catch(e) {{}}
        ws.dispatchEvent(evt);
    }};

    globalThis._mangoDispatchWsClose = function(id, code, reason, wasClean) {{
        var ws = _activeWebSockets[id];
        if (!ws) return;
        ws.readyState = 3; // CLOSED
        var evt = (typeof globalThis.CloseEvent === 'function')
            ? new globalThis.CloseEvent('close', {{ code: code, reason: reason, wasClean: wasClean }})
            : {{ type: 'close', code: code, reason: reason, wasClean: wasClean, target: ws }};
        if (typeof ws.onclose === 'function') try {{ ws.onclose(evt); }} catch(e) {{}}
        ws.dispatchEvent(evt);
        delete _activeWebSockets[id];
    }};

    function WebSocket(url, protocols) {{
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

        if (typeof _mangoWsConnect === 'function') {{
            _mangoWsConnect(id, this.url, String(protocols || ''));
        }}
    }}
    WebSocket.CONNECTING = 0;
    WebSocket.OPEN       = 1;
    WebSocket.CLOSING    = 2;
    WebSocket.CLOSED     = 3;
    WebSocket.prototype = Object.create(EventTarget.prototype);
    Object.assign(WebSocket.prototype, {{
        CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3,
        send: function(data) {{
            if (this.readyState !== 1) {{
                throw new Error("InvalidStateError: The WebSocket connection is not in the OPEN state.");
            }}
            if (typeof _mangoWsSend === 'function') {{
                _mangoWsSend(this._wsId, String(data));
            }}
        }},
        close: function(code, reason) {{
            if (this.readyState === 2 || this.readyState === 3) return;
            this.readyState = 2; // CLOSING
            if (typeof _mangoWsClose === 'function') {{
                _mangoWsClose(this._wsId, code !== undefined ? code : 1000, String(reason || ''));
            }}
        }}
    }});
    globalThis.WebSocket = WebSocket;

    // ── 24. DOM constructor hierarchy ─────────────────────────────────────────

    globalThis.Node = function() {{}};
    globalThis.Node.prototype = Object.create(EventTarget.prototype);
    // Node type constants and Document Position bitmasks
    var _nodeConsts = {{
        ELEMENT_NODE:1, ATTRIBUTE_NODE:2, TEXT_NODE:3, CDATA_SECTION_NODE:4,
        ENTITY_REFERENCE_NODE:5, ENTITY_NODE:6, PROCESSING_INSTRUCTION_NODE:7, COMMENT_NODE:8,
        DOCUMENT_NODE:9, DOCUMENT_TYPE_NODE:10, DOCUMENT_FRAGMENT_NODE:11, NOTATION_NODE:12,
        DOCUMENT_POSITION_DISCONNECTED: 1,
        DOCUMENT_POSITION_PRECEDING: 2,
        DOCUMENT_POSITION_FOLLOWING: 4,
        DOCUMENT_POSITION_CONTAINS: 8,
        DOCUMENT_POSITION_CONTAINED_BY: 16,
        DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: 32
    }};
    Object.assign(globalThis.Node, _nodeConsts);
    Object.assign(globalThis.Node.prototype, _nodeConsts);
    globalThis.Node.prototype.insertBefore    = function(n, r) {{ return this.appendChild ? this.appendChild(n) : n; }};
    globalThis.Node.prototype.replaceChild    = function(n, o) {{ if (this.removeChild) this.removeChild(o); if (this.appendChild) this.appendChild(n); return o; }};
    globalThis.Node.prototype.cloneNode       = function(deep) {{ return this; }};
    globalThis.Node.prototype.contains        = function(o) {{ return o === this; }};
    globalThis.Node.prototype.hasChildNodes   = function() {{ return (this.children && this.children.length > 0) || false; }};
    globalThis.Node.prototype.normalize       = function() {{}};
    globalThis.Node.prototype.isConnected     = true;
    globalThis.Node.prototype.isEqualNode     = function(o) {{ return this === o; }};
    globalThis.Node.prototype.isSameNode      = function(o) {{ return this === o; }};
    Object.defineProperty(globalThis.Node.prototype, 'ownerDocument', {{
        get: function() {{
            return (typeof document !== 'undefined' && this === document) ? null : (typeof document !== 'undefined' ? document : null);
        }},
        configurable: true
    }});
    globalThis.Node.prototype.getRootNode     = function(options) {{
        var cur = this;
        while (cur) {{
            var parent = cur.parentNode || cur.parentElement;
            if (!parent) break;
            cur = parent;
        }}
        return cur || (typeof document !== 'undefined' ? document : this);
    }};
    globalThis.Node.prototype.compareDocumentPosition = function(other) {{
        if (this === other) return 0;
        if (!other || typeof other !== 'object') {{
            throw new TypeError("Failed to execute 'compareDocumentPosition' on 'Node': parameter 1 is not of type 'Node'.");
        }}
        var otherId = (other._nodeId !== undefined) ? other._nodeId : null;
        var myId = (this._nodeId !== undefined) ? this._nodeId : null;
        if (myId === null && typeof document !== 'undefined' && this === document) {{
            myId = 0;
        }}
        if (otherId === null && typeof document !== 'undefined' && other === document) {{
            otherId = 0;
        }}
        if (typeof _compareDocumentPosition === 'function' && myId !== null && otherId !== null) {{
            return _compareDocumentPosition(myId, otherId);
        }}
        return 1 | 32 | 4;
    }};

    globalThis.Element = function() {{}};
    globalThis.Element.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Element.prototype.getAttribute       = function(n) {{ return null; }};
    globalThis.Element.prototype.setAttribute       = function(n, v) {{}};
    globalThis.Element.prototype.removeAttribute    = function(n) {{}};
    globalThis.Element.prototype.hasAttribute       = function(n) {{ return false; }};
    globalThis.Element.prototype.toggleAttribute    = function(n, f) {{ var has = this.hasAttribute(n); if (f === undefined ? !has : f) this.setAttribute(n,''); else this.removeAttribute(n); return this.hasAttribute(n); }};
    globalThis.Element.prototype.getAttributeNS     = function(ns, n) {{ return this.getAttribute(n); }};
    globalThis.Element.prototype.setAttributeNS     = function(ns, n, v) {{ this.setAttribute(n, v); }};
    globalThis.Element.prototype.removeAttributeNS  = function(ns, n) {{ this.removeAttribute(n); }};
    globalThis.Element.prototype.hasAttributeNS     = function(ns, n) {{ return this.hasAttribute(n); }};
    globalThis.Element.prototype.getAttributeNames  = function() {{ return []; }};
    globalThis.Element.prototype.attachShadow = function(init) {{
        var sr = (typeof document !== 'undefined' && document.createElement) ? document.createElement('div') : {{}};
        sr.mode = (init && init.mode) || 'open'; sr.host = this; this.shadowRoot = sr; return sr;
    }};
    globalThis.Element.prototype.getBoundingClientRect = function() {{
        var r = (typeof _getElementRect === 'function' && this._nodeId !== undefined) ? _getElementRect(this._nodeId) : null;
        var x = (r && r[0]) || 0;
        var y = (r && r[1]) || 0;
        var w = (r && r[2]) || (this.offsetWidth || 0);
        var h = (r && r[3]) || (this.offsetHeight || 0);
        if (globalThis.DOMRect) return new globalThis.DOMRect(x, y, w, h);
        return {{
            x: x, y: y, left: x, top: y, width: w, height: h, right: x + w, bottom: y + h,
            toJSON: function() {{ return this; }}
        }};
    }};
    globalThis.Element.prototype.getClientRects = function() {{
        var r = this.getBoundingClientRect();
        if (globalThis.DOMRectList) return new globalThis.DOMRectList([r]);
        return [r];
    }};
    globalThis.Element.prototype.matches       = function(s) {{ return false; }};
    globalThis.Element.prototype.closest       = function(s) {{ return null; }};
    globalThis.Element.prototype.focus         = function() {{
        if (this.focus && this.focus !== globalThis.Element.prototype.focus) return this.focus.apply(this, arguments);
        if (this.disabled) return;
        if (typeof document !== 'undefined') {{
            var prev = document.activeElement;
            if (prev === this) return;
            document.activeElement = this;
            if (prev && prev !== this && prev.dispatchEvent) {{
                if (globalThis._setFocusState && prev._nodeId) globalThis._setFocusState(prev._nodeId, false);
                prev.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('blur', {{ bubbles: false, relatedTarget: this }}));
                prev.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusout', {{ bubbles: true, relatedTarget: this }}));
            }}
            if (globalThis._setFocusState && this._nodeId) globalThis._setFocusState(this._nodeId, true);
            this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focus', {{ bubbles: false, relatedTarget: prev }}));
            this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusin', {{ bubbles: true, relatedTarget: prev }}));
        }}
    }};
    globalThis.Element.prototype.blur          = function() {{
        if (this.blur && this.blur !== globalThis.Element.prototype.blur) return this.blur.apply(this, arguments);
        if (globalThis._setFocusState && this._nodeId) globalThis._setFocusState(this._nodeId, false);
        if (typeof document !== 'undefined' && document.activeElement === this) {{
            document.activeElement = document.body || null;
        }}
        var next = (typeof document !== 'undefined') ? document.activeElement : null;
        this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('blur', {{ bubbles: false, relatedTarget: next }}));
        this.dispatchEvent(new (globalThis.FocusEvent || globalThis.CustomEvent)('focusout', {{ bubbles: true, relatedTarget: next }}));
    }};
    globalThis.Element.prototype.click         = function() {{
        if (this.click && this.click !== globalThis.Element.prototype.click) return this.click.apply(this, arguments);
        if (this.disabled) return;
        var evt = (typeof globalThis.MouseEvent === 'function')
            ? new globalThis.MouseEvent('click', {{ bubbles: true, cancelable: true, view: (typeof window !== 'undefined' ? window : null), detail: 1 }})
            : {{ type: 'click', target: this, bubbles: true, cancelable: true, defaultPrevented: false }};
        var notPrevented = this.dispatchEvent(evt);
        if (notPrevented !== false && !evt.defaultPrevented) {{
            var tag = (this.tagName || '').toLowerCase();
            if (tag === 'input') {{
                var itype = (this.type || (this.getAttribute && this.getAttribute('type')) || 'text').toLowerCase();
                if (itype === 'checkbox') this.checked = !this.checked;
                else if (itype === 'radio') this.checked = true;
            }}
        }}
    }};
    globalThis.Element.prototype.scrollIntoView = function(options) {{
        var rect = this.getBoundingClientRect();
        var alignToTop = (options === undefined || options === true || (typeof options === 'object' && options.block !== 'end'));
        var top = (globalThis.scrollY || 0) + (alignToTop ? rect.top : (rect.bottom - ((typeof window !== 'undefined' && window.innerHeight) || 600)));
        var left = (globalThis.scrollX || 0) + rect.left;
        var behavior = (typeof options === 'object' && options.behavior) || 'auto';
        if (typeof globalThis.scrollTo === 'function') {{
            globalThis.scrollTo({{ top: top, left: left, behavior: behavior }});
        }}
    }};
    function _animateElemScroll(el, targetLeft, targetTop) {{
        var startLeft = el.scrollLeft || 0;
        var startTop = el.scrollTop || 0;
        var endLeft = (targetLeft !== undefined) ? targetLeft : startLeft;
        var endTop = (targetTop !== undefined) ? targetTop : startTop;
        var startTime = Date.now();
        var duration = 300;
        function step() {{
            var elapsed = Date.now() - startTime;
            var progress = Math.min(1.0, elapsed / duration);
            var ease = 1.0 - Math.pow(1.0 - progress, 3);
            el.scrollLeft = startLeft + (endLeft - startLeft) * ease;
            el.scrollTop = startTop + (endTop - startTop) * ease;
            if (progress < 1.0) {{
                if (typeof globalThis.requestAnimationFrame === 'function') {{
                    globalThis.requestAnimationFrame(step);
                }} else if (typeof setTimeout === 'function') {{
                    setTimeout(step, 16);
                }}
            }}
        }}
        step();
    }}
    globalThis.Element.prototype.scrollTo      = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            if (x.behavior === 'smooth') {{
                _animateElemScroll(this, x.left, x.top);
            }} else {{
                if (x.left !== undefined) this.scrollLeft = x.left;
                if (x.top !== undefined) this.scrollTop = x.top;
            }}
        }} else {{
            if (x !== undefined) this.scrollLeft = x;
            if (y !== undefined) this.scrollTop = y;
        }}
    }};
    globalThis.Element.prototype.scrollBy      = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            var curLeft = this.scrollLeft || 0;
            var curTop = this.scrollTop || 0;
            var targetLeft = (x.left !== undefined) ? curLeft + x.left : curLeft;
            var targetTop = (x.top !== undefined) ? curTop + x.top : curTop;
            if (x.behavior === 'smooth') {{
                _animateElemScroll(this, targetLeft, targetTop);
            }} else {{
                this.scrollLeft = targetLeft;
                this.scrollTop = targetTop;
            }}
        }} else {{
            if (x !== undefined) this.scrollLeft += x;
            if (y !== undefined) this.scrollTop += y;
        }}
    }};
    globalThis.Element.prototype.scroll        = globalThis.Element.prototype.scrollTo;
    globalThis.Element.prototype.prepend       = function() {{
        if (this.prepend && this.prepend !== globalThis.Element.prototype.prepend) return this.prepend.apply(this, arguments);
        var first = this.firstChild;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            if (first) this.insertBefore(node, first);
            else if (this.appendChild) this.appendChild(node);
        }}
    }};
    globalThis.Element.prototype.append        = function() {{
        if (this.append && this.append !== globalThis.Element.prototype.append) return this.append.apply(this, arguments);
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            if (this.appendChild) this.appendChild(node);
        }}
    }};
    globalThis.Element.prototype.before        = function() {{
        if (this.before && this.before !== globalThis.Element.prototype.before) return this.before.apply(this, arguments);
        var p = this.parentNode;
        if (!p || !p.insertBefore) return;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            p.insertBefore(node, this);
        }}
    }};
    globalThis.Element.prototype.after         = function() {{
        if (this.after && this.after !== globalThis.Element.prototype.after) return this.after.apply(this, arguments);
        var p = this.parentNode;
        if (!p || !p.insertBefore) return;
        var next = this.nextSibling;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            p.insertBefore(node, next);
        }}
    }};
    globalThis.Element.prototype.replaceWith   = function() {{
        if (this.replaceWith && this.replaceWith !== globalThis.Element.prototype.replaceWith) return this.replaceWith.apply(this, arguments);
        var p = this.parentNode;
        if (!p) return;
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            if (p.insertBefore) p.insertBefore(node, this);
        }}
        if (p.removeChild) p.removeChild(this);
    }};
    globalThis.Element.prototype.replaceChildren = function() {{
        if (this.replaceChildren && this.replaceChildren !== globalThis.Element.prototype.replaceChildren) return this.replaceChildren.apply(this, arguments);
        var nodes = [];
        for (var i = 0; i < arguments.length; i++) {{
            var arg = arguments[i];
            var node = (arg && (arg.nodeType !== undefined || arg._nodeId !== undefined)) ? arg : (typeof document !== 'undefined' && document.createTextNode ? document.createTextNode(String(arg)) : String(arg));
            nodes.push(node);
        }}
        while (this.firstChild && this.removeChild) {{
            this.removeChild(this.firstChild);
        }}
        for (var j = 0; j < nodes.length; j++) {{
            if (this.appendChild) this.appendChild(nodes[j]);
        }}
    }};
    globalThis.Element.prototype.remove        = function() {{
        if (this.remove && this.remove !== globalThis.Element.prototype.remove) return this.remove.apply(this, arguments);
        if (this.parentNode && this.parentNode.removeChild) this.parentNode.removeChild(this);
    }};
    globalThis.Element.prototype.insertAdjacentHTML = function(pos, html) {{
        if (this.insertAdjacentHTML && this.insertAdjacentHTML !== globalThis.Element.prototype.insertAdjacentHTML) return this.insertAdjacentHTML.apply(this, arguments);
        var p = String(pos).toLowerCase();
        if (p !== 'beforebegin' && p !== 'afterbegin' && p !== 'beforeend' && p !== 'afterend') {{
            throw new Error("SyntaxError: The value provided ('" + pos + "') is not one of 'beforebegin', 'afterbegin', 'beforeend', or 'afterend'.");
        }}
        if (typeof document !== 'undefined' && document.createRange) {{
            var range = document.createRange();
            var frag = range.createContextualFragment(String(html));
            if (p === 'beforebegin') {{
                if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(frag, this);
            }} else if (p === 'afterbegin') {{
                if (this.insertBefore) this.insertBefore(frag, this.firstChild);
            }} else if (p === 'beforeend') {{
                if (this.appendChild) this.appendChild(frag);
            }} else if (p === 'afterend') {{
                if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(frag, this.nextSibling);
            }}
        }}
    }};
    globalThis.Element.prototype.insertAdjacentElement = function(pos, el) {{
        if (this.insertAdjacentElement && this.insertAdjacentElement !== globalThis.Element.prototype.insertAdjacentElement) return this.insertAdjacentElement.apply(this, arguments);
        var p = String(pos).toLowerCase();
        if (p === 'beforebegin') {{
            if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(el, this);
        }} else if (p === 'afterbegin') {{
            if (this.insertBefore) this.insertBefore(el, this.firstChild);
        }} else if (p === 'beforeend') {{
            if (this.appendChild) this.appendChild(el);
        }} else if (p === 'afterend') {{
            if (this.parentNode && this.parentNode.insertBefore) this.parentNode.insertBefore(el, this.nextSibling);
        }} else {{
            throw new Error("SyntaxError: Invalid position: " + pos);
        }}
        return el;
    }};
    globalThis.Element.prototype.insertAdjacentText    = function(pos, text) {{
        if (this.insertAdjacentText && this.insertAdjacentText !== globalThis.Element.prototype.insertAdjacentText) return this.insertAdjacentText.apply(this, arguments);
        var node = (typeof document !== 'undefined' && document.createTextNode)
            ? document.createTextNode(String(text))
            : String(text);
        return this.insertAdjacentElement(pos, node);
    }};
    globalThis.Element.prototype.animate               = function() {{ return {{ finished: Promise.resolve(), cancel: function() {{}} }}; }};
    globalThis.Element.prototype.getAnimations         = function() {{ return []; }};

    globalThis.HTMLElement = function() {{}};
    globalThis.HTMLElement.prototype = Object.create(globalThis.Element.prototype);
    Object.defineProperties(globalThis.HTMLElement.prototype, {{
        offsetWidth: {{
            get: function() {{ return Math.round(this.getBoundingClientRect().width); }},
            configurable: true
        }},
        offsetHeight: {{
            get: function() {{ return Math.round(this.getBoundingClientRect().height); }},
            configurable: true
        }},
        offsetParent: {{
            get: function() {{
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var cs = window.getComputedStyle(this);
                    if (cs && cs.position === 'fixed') return null;
                }}
                if (this.tagName === 'BODY' || this.tagName === 'HTML') return null;
                var cur = this.parentElement;
                while (cur && cur.tagName !== 'BODY' && cur.tagName !== 'HTML') {{
                    if (typeof window !== 'undefined' && window.getComputedStyle) {{
                        var s = window.getComputedStyle(cur);
                        if (s && s.position && s.position !== 'static') return cur;
                    }}
                    cur = cur.parentElement;
                }}
                if (cur && cur.tagName === 'BODY') return cur;
                return null;
            }},
            configurable: true
        }},
        offsetLeft: {{
            get: function() {{
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().left - p.getBoundingClientRect().left);
                return Math.round(this.getBoundingClientRect().left);
            }},
            configurable: true
        }},
        offsetTop: {{
            get: function() {{
                var p = this.offsetParent;
                if (p) return Math.round(this.getBoundingClientRect().top - p.getBoundingClientRect().top);
                return Math.round(this.getBoundingClientRect().top);
            }},
            configurable: true
        }},
        clientWidth: {{
            get: function() {{
                var bl = 0, br = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    bl = parseFloat(s.borderLeftWidth) || 0;
                    br = parseFloat(s.borderRightWidth) || 0;
                }}
                return Math.max(0, Math.round(this.getBoundingClientRect().width - bl - br));
            }},
            configurable: true
        }},
        clientHeight: {{
            get: function() {{
                var bt = 0, bb = 0;
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    bt = parseFloat(s.borderTopWidth) || 0;
                    bb = parseFloat(s.borderBottomWidth) || 0;
                }}
                return Math.max(0, Math.round(this.getBoundingClientRect().height - bt - bb));
            }},
            configurable: true
        }},
        clientTop: {{
            get: function() {{
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderTopWidth) || 0);
                }}
                return 0;
            }},
            configurable: true
        }},
        clientLeft: {{
            get: function() {{
                if (typeof window !== 'undefined' && window.getComputedStyle) {{
                    var s = window.getComputedStyle(this);
                    return Math.round(parseFloat(s.borderLeftWidth) || 0);
                }}
                return 0;
            }},
            configurable: true
        }},
        scrollWidth: {{
            get: function() {{ return Math.max(this.clientWidth, Math.round(this.getBoundingClientRect().width)); }},
            configurable: true
        }},
        scrollHeight: {{
            get: function() {{ return Math.max(this.clientHeight, Math.round(this.getBoundingClientRect().height)); }},
            configurable: true
        }},
        scrollTop: {{
            get: function() {{ return this._scrollTop || 0; }},
            set: function(v) {{
                this._scrollTop = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {{
                    if (typeof window !== 'undefined') window.scrollY = this._scrollTop;
                }}
            }},
            configurable: true
        }},
        scrollLeft: {{
            get: function() {{ return this._scrollLeft || 0; }},
            set: function(v) {{
                this._scrollLeft = Math.max(0, Number(v) || 0);
                if (this === document.body || this === document.documentElement) {{
                    if (typeof window !== 'undefined') window.scrollX = this._scrollLeft;
                }}
            }},
            configurable: true
        }},
        contentEditable: {{
            get: function() {{
                var v = this.getAttribute ? this.getAttribute('contenteditable') : null;
                if (v === null) return 'inherit';
                var lower = v.toLowerCase();
                if (lower === '' || lower === 'true') return 'true';
                if (lower === 'false') return 'false';
                return 'inherit';
            }},
            set: function(val) {{
                var s = String(val).toLowerCase();
                if (s === 'true') {{
                    this.setAttribute('contenteditable', 'true');
                }} else if (s === 'false') {{
                    this.setAttribute('contenteditable', 'false');
                }} else if (s === 'inherit') {{
                    this.setAttribute('contenteditable', 'inherit');
                }} else {{
                    throw new Error("SyntaxError: The value provided is not a valid contentEditable state.");
                }}
            }},
            configurable: true
        }},
        isContentEditable: {{
            get: function() {{
                var cur = this;
                while (cur) {{
                    var ce = (cur.getAttribute) ? cur.getAttribute('contenteditable') : null;
                    if (ce !== null) {{
                        var lower = ce.toLowerCase();
                        if (lower === '' || lower === 'true') return true;
                        if (lower === 'false') return false;
                    }}
                    cur = cur.parentElement;
                }}
                return false;
            }},
            configurable: true
        }},
        draggable: {{
            get: function() {{
                var v = this.getAttribute ? this.getAttribute('draggable') : null;
                return v !== null && v.toLowerCase() === 'true';
            }},
            set: function(val) {{
                if (this.setAttribute) {{
                    this.setAttribute('draggable', val ? 'true' : 'false');
                }}
            }},
            configurable: true
        }}
    }});

    globalThis.SVGElement    = function() {{}};
    globalThis.SVGElement.prototype = Object.create(globalThis.Element.prototype);
    globalThis.SVGSVGElement = function() {{}};
    globalThis.SVGSVGElement.prototype = Object.create(globalThis.SVGElement.prototype);
    globalThis.SVGSVGElement.prototype.createSVGPoint = function() {{ return {{ x:0, y:0, matrixTransform: function() {{ return this; }} }}; }};
    globalThis.SVGSVGElement.prototype.getScreenCTM   = function() {{ return {{ a:1, b:0, c:0, d:1, e:0, f:0, inverse: function() {{ return this; }} }}; }};

    // Specialised HTML element subclasses
    var _htmlSubclasses = [
        'HTMLTemplateElement','HTMLSlotElement','HTMLScriptElement','HTMLStyleElement','HTMLAnchorElement',
        'HTMLInputElement','HTMLButtonElement','HTMLDivElement','HTMLSpanElement',
        'HTMLIFrameElement','HTMLFormElement','HTMLSelectElement','HTMLOptionElement',
        'HTMLTableElement','HTMLTableRowElement','HTMLTableCellElement',
        'HTMLParagraphElement','HTMLHeadingElement','HTMLImageElement',
        'HTMLLIElement','HTMLUListElement','HTMLOListElement',
        'HTMLTextAreaElement','HTMLLabelElement','HTMLLinkElement','HTMLMetaElement',
        'HTMLTitleElement','HTMLBodyElement','HTMLHtmlElement','HTMLHeadElement',
        'HTMLBaseElement','HTMLBRElement','HTMLHRElement','HTMLPreElement',
        'HTMLDetailsElement','HTMLSummaryElement','HTMLDialogElement',
        'HTMLProgressElement','HTMLMeterElement','HTMLOutputElement',
        'HTMLDataListElement','HTMLFieldSetElement','HTMLLegendElement','HTMLOptGroupElement',
        'HTMLMapElement','HTMLAreaElement','HTMLObjectElement','HTMLEmbedElement',
        'HTMLSourceElement','HTMLPictureElement','HTMLTrackElement',
        'HTMLTimeElement','HTMLDataElement','HTMLRubyElement','HTMLTableSectionElement',
        'HTMLTableColElement','HTMLTableCaptionElement','HTMLDListElement',
        'HTMLQuoteElement','HTMLModElement','HTMLUnknownElement','HTMLCanvasElement',
        'HTMLMediaElement','HTMLVideoElement','HTMLAudioElement'
    ];
    _htmlSubclasses.forEach(function(name) {{
        globalThis[name] = function() {{}};
        globalThis[name].prototype = Object.create(globalThis.HTMLElement.prototype);
        globalThis[name].prototype.constructor = globalThis[name];
    }});
    if (globalThis.HTMLMediaElement && globalThis.HTMLVideoElement) {{
        globalThis.HTMLVideoElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
        globalThis.HTMLVideoElement.prototype.constructor = globalThis.HTMLVideoElement;
    }}
    if (globalThis.HTMLMediaElement && globalThis.HTMLAudioElement) {{
        globalThis.HTMLAudioElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
        globalThis.HTMLAudioElement.prototype.constructor = globalThis.HTMLAudioElement;
    }}

    // HTMLSelectElement prototype surface
    globalThis.HTMLSelectElement.prototype.checkValidity = function() {{
        return typeof this._computeValidity === 'function' ? this._computeValidity().valid : true;
    }};
    globalThis.HTMLSelectElement.prototype.reportValidity = function() {{
        return this.checkValidity();
    }};
    globalThis.HTMLSelectElement.prototype.setCustomValidity = function(message) {{
        if (message) {{
            if (this.setAttribute) this.setAttribute('data-mango-custom-validity', String(message));
        }} else {{
            if (this.removeAttribute) this.removeAttribute('data-mango-custom-validity');
        }}
    }};
    globalThis.HTMLSelectElement.prototype.add = function(opt, before) {{
        if (before === undefined || before === null) {{
            this.appendChild(opt);
        }} else if (typeof before === 'number') {{
            var ref = (this.options || [])[before];
            if (ref) this.insertBefore(opt, ref);
            else this.appendChild(opt);
        }} else {{
            this.insertBefore(opt, before);
        }}
    }};
    globalThis.HTMLSelectElement.prototype.remove = function(index) {{
        var opts = this.options || [];
        var opt = opts[index];
        if (opt && opt.parentNode) opt.parentNode.removeChild(opt);
    }};
    Object.defineProperty(globalThis.HTMLSelectElement.prototype, 'value', {{
        get: function() {{
            var opts = this.options || [];
            for (var i = 0; i < opts.length; i++) {{
                if (opts[i].selected) {{
                    return opts[i].value;
                }}
            }}
            if (!this.multiple && opts.length > 0) {{
                return opts[0].value;
            }}
            return "";
        }},
        set: function(val) {{
            var targetVal = (val === null || val === undefined) ? "" : String(val);
            var opts = this.options || [];
            var matched = false;
            for (var i = 0; i < opts.length; i++) {{
                if (opts[i].value === targetVal) {{
                    opts[i].selected = true;
                    matched = true;
                }} else if (!this.multiple) {{
                    opts[i].selected = false;
                }}
            }}
            if (!matched && !this.multiple) {{
                this.selectedIndex = -1;
            }}
        }},
        configurable: true,
        enumerable: true
    }});

    // HTMLTextAreaElement prototype surface
    Object.defineProperty(globalThis.HTMLTextAreaElement.prototype, 'value', {{
        get: function() {{
            var v = this.getAttribute ? this.getAttribute("data-mango-value") : null;
            if (v !== null) return v;
            return this.textContent || "";
        }},
        set: function(val) {{
            var str = (val === null || val === undefined) ? "" : String(val);
            if (this.setAttribute) {{
                this.setAttribute("data-mango-value", str);
            }}
            this.textContent = str;
        }},
        configurable: true,
        enumerable: true
    }});

    // HTMLInputElement prototype surface
    Object.defineProperty(globalThis.HTMLInputElement.prototype, 'value', {{
        get: function() {{
            var v = this.getAttribute ? this.getAttribute("data-mango-value") : null;
            if (v !== null) return v;
            var attr = this.getAttribute ? this.getAttribute("value") : null;
            return attr === null ? "" : attr;
        }},
        set: function(val) {{
            if (this.getAttribute && this.getAttribute("data-mango-default-value") === null) {{
                var initVal = this.getAttribute("value");
                if (this.setAttribute) this.setAttribute("data-mango-default-value", initVal === null ? "" : initVal);
            }}
            var str = (val === null || val === undefined) ? "" : String(val);
            if (this.setAttribute) {{
                this.setAttribute("data-mango-value", str);
                this.setAttribute("value", str);
            }}
        }},
        configurable: true,
        enumerable: true
    }});

    Object.defineProperty(globalThis.HTMLInputElement.prototype, 'checked', {{
        get: function() {{
            var dyn = this.getAttribute ? this.getAttribute("data-mango-checked") : null;
            if (dyn !== null) return dyn === "true";
            return this.hasAttribute ? this.hasAttribute("checked") : false;
        }},
        set: function(val) {{
            if (this.hasAttribute && !this.hasAttribute("data-mango-default-checked")) {{
                this.setAttribute("data-mango-default-checked", this.hasAttribute("checked") ? "true" : "false");
            }}
            if (this.setAttribute) {{
                this.setAttribute("data-mango-checked", val ? "true" : "false");
                if (val) this.setAttribute("checked", "");
                else if (this.removeAttribute) this.removeAttribute("checked");
            }}
            if (this.dispatchEvent) {{
                this.dispatchEvent({{ type: 'change' }});
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    globalThis.HTMLInputElement.prototype.stepUp = function(n) {{
        var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
        var stepAttr = this.getAttribute ? this.getAttribute('step') : null;
        if (stepAttr && stepAttr.toLowerCase() === 'any') return;
        var step = parseFloat(stepAttr);
        if (isNaN(step) || step <= 0) step = 1.0;
        var cur = parseFloat(this.value);
        if (isNaN(cur)) {{
            var minVal = parseFloat(this.min);
            cur = isNaN(minVal) ? 0.0 : minVal;
        }}
        var next = cur + count * step;
        var min = parseFloat(this.min);
        if (!isNaN(min) && next < min) next = min;
        var max = parseFloat(this.max);
        if (!isNaN(max) && next > max) next = max;
        var decimals = 0;
        var stepStr = String(step);
        var dotIdx = stepStr.indexOf('.');
        if (dotIdx >= 0) decimals = stepStr.length - dotIdx - 1;
        this.value = decimals > 0 ? next.toFixed(decimals) : String(Math.round(next));
    }};
    globalThis.HTMLInputElement.prototype.stepDown = function(n) {{
        var count = (n === undefined) ? 1 : (parseInt(n, 10) || 0);
        this.stepUp(-count);
    }};
    globalThis.HTMLInputElement.prototype.checkValidity = function() {{
        var v = typeof this._computeValidity === 'function' ? this._computeValidity() : {{ valid: true }};
        if (!v.valid && this.dispatchEvent) {{
            this.dispatchEvent({{ type: 'invalid', target: this }});
        }}
        return v.valid;
    }};
    globalThis.HTMLInputElement.prototype.reportValidity = function() {{
        return this.checkValidity();
    }};
    globalThis.HTMLInputElement.prototype.setCustomValidity = function(message) {{
        if (message) {{
            if (this.setAttribute) this.setAttribute('data-mango-custom-validity', String(message));
        }} else {{
            if (this.removeAttribute) this.removeAttribute('data-mango-custom-validity');
        }}
    }};

    // HTMLFormElement prototype surface
    globalThis.HTMLFormElement.prototype.checkValidity = function() {{
        var elems = this.elements || [];
        var allValid = true;
        var firstInvalid = null;
        for (var i = 0; i < elems.length; i++) {{
            var el = elems[i];
            if (el.willValidate && typeof el.checkValidity === 'function' && !el.checkValidity()) {{
                if (!firstInvalid) firstInvalid = el;
                allValid = false;
            }}
        }}
        return allValid;
    }};
    globalThis.HTMLFormElement.prototype.reportValidity = function() {{
        return this.checkValidity();
    }};
    globalThis.HTMLFormElement.prototype.submit = function() {{
        if (this.dispatchEvent) {{
            this.dispatchEvent({{ type: 'submit', target: this }});
        }}
    }};
    globalThis.HTMLFormElement.prototype.reset = function() {{
        var elems = this.elements || [];
        for (var i = 0; i < elems.length; i++) {{
            var el = elems[i];
            var elTag = (el.tagName || '').toLowerCase();
            if (elTag === 'input') {{
                var t = el.type;
                if (t === 'checkbox' || t === 'radio') {{
                    el.checked = el.defaultChecked;
                }} else {{
                    el.value = el.defaultValue;
                }}
            }} else if (elTag === 'textarea') {{
                el.value = el.defaultValue;
            }} else if (elTag === 'select') {{
                var opts = el.options || [];
                for (var j = 0; j < opts.length; j++) {{
                    opts[j].selected = opts[j].defaultSelected;
                }}
            }}
        }}
        if (this.dispatchEvent) {{
            this.dispatchEvent({{ type: 'reset', target: this }});
        }}
    }};

    // HTMLDialogElement prototype surface
    globalThis.HTMLDialogElement.prototype.show = function() {{
        if (this.setAttribute) this.setAttribute('open', '');
        if (this.removeAttribute) this.removeAttribute('data-mango-modal');
    }};
    globalThis.HTMLDialogElement.prototype.showModal = function() {{
        if (this.hasAttribute && this.hasAttribute('open')) {{
            throw new Error('InvalidStateError: Dialog is already open');
        }}
        if (this.setAttribute) {{
            this.setAttribute('data-mango-modal', 'true');
            this.setAttribute('open', '');
        }}
        if (this.focus) this.focus();
    }};
    globalThis.HTMLDialogElement.prototype.close = function(retVal) {{
        if (this.hasAttribute && !this.hasAttribute('open')) return;
        if (retVal !== undefined) this.returnValue = String(retVal);
        if (this.removeAttribute) {{
            this.removeAttribute('open');
            this.removeAttribute('data-mango-modal');
        }}
        if (this.dispatchEvent) this.dispatchEvent({{ type: 'close', target: this }});
    }};
    Object.defineProperty(globalThis.HTMLDialogElement.prototype, 'open', {{
        get: function() {{ return this.hasAttribute ? this.hasAttribute('open') : false; }},
        set: function(v) {{ if (v) {{ if (this.setAttribute) this.setAttribute('open', ''); }} else {{ if (this.removeAttribute) this.removeAttribute('open'); }} }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLDialogElement.prototype, 'returnValue', {{
        get: function() {{ return this._returnValue || ''; }},
        set: function(v) {{ this._returnValue = String(v); }},
        configurable: true
    }});

    // HTMLProgressElement prototype surface
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'value', {{
        get: function() {{
            var v = this.getAttribute ? this.getAttribute('value') : null;
            if (v === null) return -1;
            var n = parseFloat(v);
            return isNaN(n) ? 0 : Math.max(0, n);
        }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('value', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'max', {{
        get: function() {{
            var m = this.getAttribute ? this.getAttribute('max') : null;
            if (m === null) return 1.0;
            var n = parseFloat(m);
            return isNaN(n) || n <= 0 ? 1.0 : n;
        }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('max', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLProgressElement.prototype, 'position', {{
        get: function() {{
            var v = this.getAttribute ? this.getAttribute('value') : null;
            if (v === null) return -1;
            var val = this.value;
            var max = this.max;
            return max > 0 ? Math.max(0, Math.min(1, val / max)) : -1;
        }},
        configurable: true
    }});

    // HTMLMeterElement prototype surface
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'min', {{
        get: function() {{ var m = this.getAttribute ? this.getAttribute('min') : null; var n = parseFloat(m); return isNaN(n) ? 0.0 : n; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('min', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'max', {{
        get: function() {{ var m = this.getAttribute ? this.getAttribute('max') : null; var n = parseFloat(m); var min = this.min; return isNaN(n) ? Math.max(min, 1.0) : Math.max(min, n); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('max', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'value', {{
        get: function() {{ var v = this.getAttribute ? this.getAttribute('value') : null; var n = parseFloat(v); var min = this.min; var max = this.max; if (isNaN(n)) return min; return Math.max(min, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('value', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'low', {{
        get: function() {{ var l = this.getAttribute ? this.getAttribute('low') : null; var n = parseFloat(l); var min = this.min; var max = this.max; if (isNaN(n)) return min; return Math.max(min, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('low', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'high', {{
        get: function() {{ var h = this.getAttribute ? this.getAttribute('high') : null; var n = parseFloat(h); var min = this.min; var max = this.max; var low = this.low; if (isNaN(n)) return max; return Math.max(low, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('high', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMeterElement.prototype, 'optimum', {{
        get: function() {{ var o = this.getAttribute ? this.getAttribute('optimum') : null; var n = parseFloat(o); var min = this.min; var max = this.max; if (isNaN(n)) return min + (max - min) / 2.0; return Math.max(min, Math.min(max, n)); }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('optimum', String(v)); }},
        configurable: true
    }});

    // HTMLOutputElement prototype surface
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'value', {{
        get: function() {{ return this.textContent || ''; }},
        set: function(v) {{ this.textContent = String(v); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'defaultValue', {{
        get: function() {{ return this._defaultValue !== undefined ? this._defaultValue : (this.textContent || ''); }},
        set: function(v) {{ this._defaultValue = String(v); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'type', {{
        get: function() {{ return 'output'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'name', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('name') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('name', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLOutputElement.prototype, 'htmlFor', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('for') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('for', String(v)); }},
        configurable: true
    }});

    // HTMLDataListElement prototype surface
    Object.defineProperty(globalThis.HTMLDataListElement.prototype, 'options', {{
        get: function() {{ return this.getElementsByTagName ? this.getElementsByTagName('option') : []; }},
        configurable: true
    }});

    // HTMLInputElement.list surface
    Object.defineProperty(globalThis.HTMLInputElement.prototype, 'list', {{
        get: function() {{
            var listId = this.getAttribute ? this.getAttribute('list') : null;
            if (!listId || typeof document === 'undefined') return null;
            var el = document.getElementById(listId);
            return (el && el.tagName && el.tagName.toLowerCase() === 'datalist') ? el : null;
        }},
        configurable: true
    }});

    // HTMLMapElement prototype surface
    Object.defineProperty(globalThis.HTMLMapElement.prototype, 'name', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('name') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('name', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLMapElement.prototype, 'areas', {{
        get: function() {{ return this.getElementsByTagName ? this.getElementsByTagName('area') : []; }},
        configurable: true
    }});

    // HTMLAreaElement prototype surface
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'shape', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('shape') || 'rect') : 'rect'; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('shape', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'coords', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('coords') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('coords', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'alt', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('alt') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('alt', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'target', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('target') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('target', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLAreaElement.prototype, 'href', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('href') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('href', String(v)); }},
        configurable: true
    }});

    // HTMLObjectElement prototype surface
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'data', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('data') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('data', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'type', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('type') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('type', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'name', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('name') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('name', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'width', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('width') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('width', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLObjectElement.prototype, 'height', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('height') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('height', String(v)); }},
        configurable: true
    }});

    // HTMLEmbedElement prototype surface
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'src', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('src') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('src', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'type', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('type') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('type', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'width', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('width') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('width', String(v)); }},
        configurable: true
    }});
    Object.defineProperty(globalThis.HTMLEmbedElement.prototype, 'height', {{
        get: function() {{ return this.getAttribute ? (this.getAttribute('height') || '') : ''; }},
        set: function(v) {{ if (this.setAttribute) this.setAttribute('height', String(v)); }},
        configurable: true
    }});

    globalThis.HTMLMediaElement = function() {{}};
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
    globalThis.HTMLMediaElement.prototype.play = function() {{
        this._playing = true; this._paused = false;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'true');
        var self = this;
        setTimeout(function() {{
            if (self.dispatchEvent) {{ self.dispatchEvent({{ type:'play' }}); self.dispatchEvent({{ type:'playing' }}); }}
        }}, 0);
        return (typeof Promise !== 'undefined') ? Promise.resolve() : null;
    }};
    globalThis.HTMLMediaElement.prototype.pause = function() {{
        this._playing = false; this._paused = true;
        if (this.setAttribute) this.setAttribute('data-mango-playing', 'false');
        var self = this;
        setTimeout(function() {{ if (self.dispatchEvent) self.dispatchEvent({{ type:'pause' }}); }}, 0);
    }};
    globalThis.HTMLMediaElement.prototype.load = function() {{
        var self = this;
        setTimeout(function() {{
            if (self.dispatchEvent) {{
                self.dispatchEvent({{ type:'loadstart' }});
                self.dispatchEvent({{ type:'loadedmetadata' }});
                self.dispatchEvent({{ type:'canplay' }});
            }}
        }}, 0);
    }};
    globalThis.HTMLMediaElement.prototype.canPlayType = function(type) {{
        if (!type) return '';
        var t = String(type).toLowerCase();
        if (t.indexOf('mp4') !== -1 || t.indexOf('webm') !== -1 || t.indexOf('ogg') !== -1 ||
            t.indexOf('audio/mpeg') !== -1 || t.indexOf('audio/mp3') !== -1 ||
            t.indexOf('audio/wav') !== -1 || t.indexOf('audio/ogg') !== -1 || t.indexOf('audio/aac') !== -1) {{
            return 'probably';
        }}
        return 'maybe';
    }};
    Object.defineProperties(globalThis.HTMLMediaElement.prototype, {{
        currentTime: {{
            get: function() {{
                var t = (this.getAttribute && this.getAttribute('data-mango-time')) || this._currentTime || 0;
                var num = parseFloat(t);
                return isNaN(num) ? 0 : num;
            }},
            set: function(val) {{
                var num = Math.max(0, parseFloat(val) || 0);
                this._currentTime = num;
                if (this.setAttribute) this.setAttribute('data-mango-time', String(num));
                if (this.dispatchEvent) this.dispatchEvent({{ type: 'timeupdate' }});
            }},
            configurable: true
        }},
        duration: {{
            get: function() {{
                var d = (this.getAttribute && this.getAttribute('data-mango-duration')) || this._duration || 100;
                var num = parseFloat(d);
                return isNaN(num) ? 100 : num;
            }},
            configurable: true
        }},
        paused: {{
            get: function() {{
                if (this._paused !== undefined) return this._paused;
                var p = this.getAttribute ? this.getAttribute('data-mango-playing') : null;
                if (p === 'true') return false;
                if (p === 'false') return true;
                return !this.autoplay;
            }},
            configurable: true
        }},
        volume: {{
            get: function() {{ return this._volume !== undefined ? this._volume : 1.0; }},
            set: function(val) {{
                this._volume = Math.max(0, Math.min(1, parseFloat(val) || 0));
                if (this.dispatchEvent) this.dispatchEvent({{ type: 'volumechange' }});
            }},
            configurable: true
        }},
        muted: {{
            get: function() {{
                if (this._muted !== undefined) return this._muted;
                return (this.hasAttribute && this.hasAttribute('muted')) ||
                    (this.getAttribute && this.getAttribute('data-mango-muted') === 'true');
            }},
            set: function(val) {{
                this._muted = !!val;
                if (this.setAttribute) {{
                    if (val) {{ this.setAttribute('muted', ''); this.setAttribute('data-mango-muted', 'true'); }}
                    else {{ if (this.removeAttribute) this.removeAttribute('muted'); this.setAttribute('data-mango-muted', 'false'); }}
                }}
                if (this.dispatchEvent) this.dispatchEvent({{ type: 'volumechange' }});
            }},
            configurable: true
        }},
        playbackRate: {{
            get: function() {{ return this._playbackRate !== undefined ? this._playbackRate : 1.0; }},
            set: function(val) {{
                this._playbackRate = parseFloat(val) || 1.0;
                if (this.dispatchEvent) this.dispatchEvent({{ type: 'ratechange' }});
            }},
            configurable: true
        }}
    }});
    globalThis.HTMLVideoElement = function() {{}};
    globalThis.HTMLVideoElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLAudioElement = function() {{}};
    globalThis.HTMLAudioElement.prototype = Object.create(globalThis.HTMLMediaElement.prototype);
    globalThis.HTMLCanvasElement = function() {{}};
    globalThis.HTMLCanvasElement.prototype = Object.create(globalThis.HTMLElement.prototype);
    globalThis.CanvasRenderingContext2D = function() {{}};
    globalThis.ImageData = function(dataOrWidth, widthOrHeight, heightOpt) {{
        if (typeof dataOrWidth === 'number') {{
            this.width  = Math.max(1, Math.floor(dataOrWidth));
            this.height = (typeof widthOrHeight === 'number') ? Math.max(1, Math.floor(widthOrHeight)) : 1;
            this.data   = new Uint8ClampedArray(this.width * this.height * 4);
        }} else {{
            this.data   = (dataOrWidth instanceof Uint8ClampedArray) ? dataOrWidth : new Uint8ClampedArray(dataOrWidth);
            this.width  = Math.max(1, Math.floor(widthOrHeight));
            this.height = heightOpt !== undefined ? Math.max(1, Math.floor(heightOpt)) : Math.floor(this.data.length / (this.width * 4));
        }}
    }};

    globalThis.Document = function() {{}};
    globalThis.Document.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Document.prototype.createElement      = function(t) {{ return (typeof document !== 'undefined') ? document.createElement(t) : null; }};
    globalThis.Document.prototype.createElementNS    = function(ns, t) {{ return (typeof document !== 'undefined' && document.createElementNS) ? document.createElementNS(ns, t) : this.createElement(t); }};
    globalThis.Document.prototype.createTextNode     = function(t) {{ return (typeof document !== 'undefined') ? document.createTextNode(t) : null; }};
    globalThis.Document.prototype.createDocumentFragment = function() {{ return (typeof document !== 'undefined' && document.createDocumentFragment) ? document.createDocumentFragment() : null; }};
    globalThis.Document.prototype.querySelector      = function(s) {{ return (typeof document !== 'undefined') ? document.querySelector(s) : null; }};
    globalThis.Document.prototype.querySelectorAll   = function(s) {{ return (typeof document !== 'undefined') ? document.querySelectorAll(s) : []; }};
    globalThis.Document.prototype.getElementById     = function(i) {{ return (typeof document !== 'undefined') ? document.getElementById(i) : null; }};
    globalThis.Document.prototype.getElementsByTagName    = function(t) {{ return (typeof document !== 'undefined') ? document.getElementsByTagName(t) : []; }};
    globalThis.Document.prototype.getElementsByClassName  = function(c) {{ return (typeof document !== 'undefined') ? document.getElementsByClassName(c) : []; }};
    globalThis.Document.prototype.createEvent        = function(t) {{ return (typeof document !== 'undefined' && document.createEvent) ? document.createEvent(t) : new globalThis.Event(t); }};
    globalThis.Document.prototype.importNode         = function(n, d) {{ return n && n.cloneNode ? n.cloneNode(d) : n; }};
    globalThis.Document.prototype.adoptNode          = function(n) {{ return n; }};
    globalThis.Document.prototype.elementFromPoint   = function(x, y) {{ return (typeof document !== 'undefined' && document.elementFromPoint) ? document.elementFromPoint(x, y) : null; }};
    globalThis.Document.prototype.elementsFromPoint  = function(x, y) {{ return (typeof document !== 'undefined' && document.elementsFromPoint) ? document.elementsFromPoint(x, y) : []; }};
    globalThis.Document.prototype.caretPositionFromPoint = function() {{ return null; }};
    globalThis.Document.prototype.createRange        = function() {{
        return (typeof document !== 'undefined' && document.createRange) ? document.createRange() : new globalThis.Range();
    }};
    globalThis.Document.prototype.getSelection      = function() {{
        return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null;
    }};
    Object.defineProperty(globalThis.Document.prototype, 'activeElement', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.activeElement : null; }},
        set: function(val) {{ if (typeof document !== 'undefined') document.activeElement = val; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'characterSet', {{
        get: function() {{ return (typeof document !== 'undefined' && document.characterSet) ? document.characterSet : 'UTF-8'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'charset', {{
        get: function() {{ return (typeof document !== 'undefined' && document.charset) ? document.charset : 'UTF-8'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'inputEncoding', {{
        get: function() {{ return (typeof document !== 'undefined' && document.inputEncoding) ? document.inputEncoding : 'UTF-8'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'contentType', {{
        get: function() {{ return (typeof document !== 'undefined' && document.contentType) ? document.contentType : 'text/html'; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'documentElement', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.documentElement : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'head', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.head : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'body', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.body : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'forms', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.forms : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'images', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.images : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'links', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.links : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'scripts', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.scripts : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'children', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.children : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'childNodes', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.childNodes : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'firstChild', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.firstChild : null; }},
        configurable: true
    }});
    Object.defineProperty(globalThis.Document.prototype, 'lastChild', {{
        get: function() {{ return (typeof document !== 'undefined') ? document.lastChild : null; }},
        configurable: true
    }});
    globalThis.Document.prototype.appendChild = function(c) {{ return (typeof document !== 'undefined' && document.appendChild) ? document.appendChild(c) : c; }};
    globalThis.Document.prototype.removeChild = function(c) {{ return (typeof document !== 'undefined' && document.removeChild) ? document.removeChild(c) : c; }};
    globalThis.Document.prototype.insertBefore = function(n, r) {{ return (typeof document !== 'undefined' && document.insertBefore) ? document.insertBefore(n, r) : n; }};
    globalThis.Document.prototype.replaceChild = function(n, o) {{ return (typeof document !== 'undefined' && document.replaceChild) ? document.replaceChild(n, o) : o; }};
    globalThis.HTMLDocument = globalThis.Document;

    // window / Window prototype
    globalThis.Window = function() {{}};
    globalThis.Window.prototype = Object.create(globalThis.EventTarget.prototype);
    try {{ Object.setPrototypeOf(globalThis, globalThis.Window.prototype); }} catch(e) {{}}
    var _winScrollX = 0;
    var _winScrollY = 0;

    function _setWinScroll(x, y, suppressEvent) {{
        var newX = Math.max(0, Number(x) || 0);
        var newY = Math.max(0, Number(y) || 0);
        var changed = (newX !== _winScrollX || newY !== _winScrollY);
        _winScrollX = newX;
        _winScrollY = newY;
        if (typeof document !== 'undefined') {{
            if (document.documentElement) {{
                document.documentElement._scrollLeft = newX;
                document.documentElement._scrollTop = newY;
            }}
            if (document.body) {{
                document.body._scrollLeft = newX;
                document.body._scrollTop = newY;
            }}
        }}
        if (changed && !suppressEvent) {{
            if (typeof globalThis._dispatchInternalEvent === 'function') {{
                try {{ globalThis._dispatchInternalEvent(-1, 'scroll', {{ bubbles: false, cancelable: false, eventType: 'ui' }}); }} catch(e) {{}}
            }} else if (typeof globalThis.dispatchEvent === 'function') {{
                try {{
                    var ev = (typeof globalThis.Event === 'function') ? new globalThis.Event('scroll') : {{ type: 'scroll' }};
                    globalThis.dispatchEvent(ev);
                }} catch(e) {{}}
            }}
        }}
    }}

    Object.defineProperty(globalThis, 'scrollX', {{
        get: function() {{ return _winScrollX; }},
        set: function(v) {{ _setWinScroll(v, _winScrollY, true); }},
        configurable: true
    }});
    Object.defineProperty(globalThis, 'scrollY', {{
        get: function() {{ return _winScrollY; }},
        set: function(v) {{ _setWinScroll(_winScrollX, v, true); }},
        configurable: true
    }});
    Object.defineProperty(globalThis, 'pageXOffset', {{
        get: function() {{ return _winScrollX; }},
        set: function(v) {{ _setWinScroll(v, _winScrollY, true); }},
        configurable: true
    }});
    Object.defineProperty(globalThis, 'pageYOffset', {{
        get: function() {{ return _winScrollY; }},
        set: function(v) {{ _setWinScroll(_winScrollX, v, true); }},
        configurable: true
    }});

    function _animateWinScroll(targetX, targetY) {{
        var startX = _winScrollX;
        var startY = _winScrollY;
        var startTime = Date.now();
        var duration = 300;
        function step() {{
            var elapsed = Date.now() - startTime;
            var progress = Math.min(1.0, elapsed / duration);
            var ease = 1.0 - Math.pow(1.0 - progress, 3);
            var curX = startX + (targetX - startX) * ease;
            var curY = startY + (targetY - startY) * ease;
            _setWinScroll(curX, curY, false);
            if (progress < 1.0) {{
                if (typeof globalThis.requestAnimationFrame === 'function') {{
                    globalThis.requestAnimationFrame(step);
                }} else if (typeof setTimeout === 'function') {{
                    setTimeout(step, 16);
                }}
            }}
        }}
        step();
    }}

    globalThis.scrollTo = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            var tx = (x.left !== undefined) ? x.left : _winScrollX;
            var ty = (x.top !== undefined) ? x.top : _winScrollY;
            if (x.behavior === 'smooth') {{
                _animateWinScroll(tx, ty);
            }} else {{
                _setWinScroll(tx, ty, false);
            }}
        }} else {{
            var tx = (x !== undefined) ? x : _winScrollX;
            var ty = (y !== undefined) ? y : _winScrollY;
            _setWinScroll(tx, ty, false);
        }}
    }};
    globalThis.scrollBy = function(x, y) {{
        if (typeof x === 'object' && x !== null) {{
            var dx = (x.left !== undefined) ? x.left : 0;
            var dy = (x.top !== undefined) ? x.top : 0;
            if (x.behavior === 'smooth') {{
                _animateWinScroll(_winScrollX + dx, _winScrollY + dy);
            }} else {{
                _setWinScroll(_winScrollX + dx, _winScrollY + dy, false);
            }}
        }} else {{
            var dx = (x !== undefined) ? x : 0;
            var dy = (y !== undefined) ? y : 0;
            _setWinScroll(_winScrollX + dx, _winScrollY + dy, false);
        }}
    }};
    globalThis.scroll = globalThis.scrollTo;

    if (globalThis.window) {{
        globalThis.window.scrollTo = globalThis.scrollTo;
        globalThis.window.scrollBy = globalThis.scrollBy;
        globalThis.window.scroll = globalThis.scroll;
        try {{
            Object.defineProperty(globalThis.window, 'scrollX', {{ get: function() {{ return _winScrollX; }}, set: function(v) {{ _setWinScroll(v, _winScrollY, true); }}, configurable: true }});
            Object.defineProperty(globalThis.window, 'scrollY', {{ get: function() {{ return _winScrollY; }}, set: function(v) {{ _setWinScroll(_winScrollX, v, true); }}, configurable: true }});
            Object.defineProperty(globalThis.window, 'pageXOffset', {{ get: function() {{ return _winScrollX; }}, set: function(v) {{ _setWinScroll(v, _winScrollY, true); }}, configurable: true }});
            Object.defineProperty(globalThis.window, 'pageYOffset', {{ get: function() {{ return _winScrollY; }}, set: function(v) {{ _setWinScroll(_winScrollX, v, true); }}, configurable: true }});
        }} catch(e) {{}}
    }}
    function WindowProxy(url, target, features, openerWindow) {{
        var self = this;
        this.closed = false;
        this.name = String(target || '_blank');
        this.opener = openerWindow || ((typeof globalThis.window !== 'undefined') ? globalThis.window : globalThis);
        this.location = {{
            _href: String(url || 'about:blank'),
            get href() {{ return this._href; }},
            set href(val) {{
                this._href = String(val);
                if (typeof _mangoOpenWindow === 'function') {{
                    _mangoOpenWindow(this._href, target, features);
                }}
            }},
            toString: function() {{ return this._href; }}
        }};
        this.focus = function() {{}};
        this.blur = function() {{}};
        this.close = function() {{
            self.closed = true;
        }};
        this._listeners = {{}};
        this.onmessage = null;
        this.addEventListener = function(type, listener) {{
            if (!self._listeners[type]) self._listeners[type] = [];
            self._listeners[type].push(listener);
        }};
        this.removeEventListener = function(type, listener) {{
            if (!self._listeners[type]) return;
            self._listeners[type] = self._listeners[type].filter(function(l) {{ return l !== listener; }});
        }};
        this.dispatchEvent = function(event) {{
            if (self.closed) return true;
            var list = (self._listeners[event.type] || []).slice();
            for (var i = 0; i < list.length; i++) {{
                try {{ list[i].call(self, event); }} catch(e) {{
                    if (typeof console !== 'undefined' && console.error) console.error(e);
                }}
            }}
            var onfn = self['on' + event.type];
            if (typeof onfn === 'function') {{
                try {{ onfn.call(self, event); }} catch(e) {{
                    if (typeof console !== 'undefined' && console.error) console.error(e);
                }}
            }}
            return !event.defaultPrevented;
        }};
        this.postMessage = function(message, targetOrigin, transfer) {{
            if (self.closed) return;
            var clonedData;
            try {{
                clonedData = (typeof structuredClone === 'function') ? structuredClone(message) : message;
            }} catch(e) {{
                clonedData = message;
            }}
            var origin = (typeof location !== 'undefined' && location.origin) ? location.origin : '*';
            var ev = (typeof globalThis.MessageEvent === 'function')
                ? new globalThis.MessageEvent('message', {{ data: clonedData, origin: origin, source: self.opener }})
                : {{ type: 'message', data: clonedData, origin: origin, source: self.opener, defaultPrevented: false }};
            var defer = (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) {{ setTimeout(cb, 0); }};
            defer(function() {{
                self.dispatchEvent(ev);
            }});
        }};
    }}
    globalThis.WindowProxy = WindowProxy;

    var _windowOpenFn = function(url, target, features) {{
        var finalUrl = String(url || 'about:blank');
        var targetStr = String(target || '_blank');
        var featuresStr = String(features || '');
        if (targetStr === '_self') {{
            if (typeof _mangoNavigate === 'function') {{
                _mangoNavigate(finalUrl);
            }}
            return (typeof globalThis.window !== 'undefined') ? globalThis.window : globalThis;
        }}
        if (typeof _mangoOpenWindow === 'function') {{
            _mangoOpenWindow(finalUrl, targetStr, featuresStr);
        }}
        var openerWin = (typeof globalThis.window !== 'undefined') ? globalThis.window : globalThis;
        return new WindowProxy(finalUrl, targetStr, featuresStr, openerWin);
    }};

    if (globalThis.window) {{
        globalThis.window.open = _windowOpenFn;
        if (!globalThis.window.opener) {{
            globalThis.window.opener = null;
        }}
        globalThis.window.postMessage = function(message, targetOrigin, transfer) {{
            var clonedData;
            try {{
                clonedData = (typeof structuredClone === 'function') ? structuredClone(message) : message;
            }} catch(e) {{
                clonedData = message;
            }}
            var origin = (typeof location !== 'undefined' && location.origin) ? location.origin : '*';
            var ev = (typeof globalThis.MessageEvent === 'function')
                ? new globalThis.MessageEvent('message', {{ data: clonedData, origin: origin, source: globalThis.window }})
                : {{ type: 'message', data: clonedData, origin: origin, source: globalThis.window, defaultPrevented: false }};
            var defer = (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) {{ setTimeout(cb, 0); }};
            defer(function() {{
                if (typeof globalThis.window.dispatchEvent === 'function') {{
                    globalThis.window.dispatchEvent(ev);
                }} else if (typeof globalThis.dispatchEvent === 'function') {{
                    globalThis.dispatchEvent(ev);
                }}
                if (typeof globalThis.window.onmessage === 'function') {{
                    try {{ globalThis.window.onmessage(ev); }} catch(e) {{}}
                }} else if (typeof globalThis.onmessage === 'function') {{
                    try {{ globalThis.onmessage(ev); }} catch(e) {{}}
                }}
            }});
        }};
    }}
    globalThis.open = _windowOpenFn;
    globalThis.window.close     = function() {{}};
    globalThis.window.focus     = function() {{}};
    globalThis.window.blur      = function() {{}};
    globalThis.window.print     = function() {{}};
    globalThis.window.confirm   = function(msg) {{ return false; }};
    globalThis.window.prompt    = function(msg, def) {{ return def || null; }};
    globalThis.window.getSelection = function() {{ return (typeof document !== 'undefined' && document.getSelection) ? document.getSelection() : null; }};

    // Shady DOM compatibility shims (used by Polymer / Lit)
    ['addEventListener','removeEventListener','dispatchEvent'].forEach(function(m) {{
        var nativeProp = '__shady_native_' + m;
        Object.defineProperty(globalThis, nativeProp, {{
            get: function() {{ return this['_'+nativeProp] || globalThis[m]; }},
            set: function(v) {{ this['_'+nativeProp] = v; }},
            configurable: true
        }});
    }});

    // ── 24. Other DOM types ───────────────────────────────────────────────────
    globalThis.DocumentFragment = function() {{}};
    globalThis.DocumentFragment.prototype = Object.create(globalThis.Node.prototype);
    globalThis.DocumentFragment.prototype.append = globalThis.Element.prototype.append;
    globalThis.DocumentFragment.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.DocumentFragment.prototype.replaceChildren = globalThis.Element.prototype.replaceChildren;

    globalThis.Document.prototype.append = globalThis.Element.prototype.append;
    globalThis.Document.prototype.prepend = globalThis.Element.prototype.prepend;
    globalThis.Document.prototype.replaceChildren = globalThis.Element.prototype.replaceChildren;

    globalThis.ShadowRoot = function() {{}};
    globalThis.ShadowRoot.prototype = Object.create(globalThis.DocumentFragment.prototype);
    globalThis.DOMImplementation = function() {{}};
    globalThis.CharacterData = function() {{}};
    globalThis.CharacterData.prototype = Object.create(globalThis.Node.prototype);
    globalThis.Text = function() {{}};
    globalThis.Text.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.Comment = function() {{}};
    globalThis.Comment.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.CDATASection = function() {{}};
    globalThis.CDATASection.prototype = Object.create(globalThis.Text.prototype);
    globalThis.ProcessingInstruction = function() {{}};
    globalThis.ProcessingInstruction.prototype = Object.create(globalThis.CharacterData.prototype);
    globalThis.DocumentType = function() {{}};
    globalThis.DocumentType.prototype = Object.create(globalThis.Node.prototype);
    globalThis.XMLDocument = function() {{}};
    globalThis.XMLDocument.prototype = Object.create(globalThis.Document.prototype);
    globalThis.Attr = function() {{}};
    globalThis.HTMLCollection = function() {{}};
    globalThis.HTMLCollection.prototype = Object.create(Object.prototype);
    globalThis.HTMLCollection.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    }};
    globalThis.HTMLCollection.prototype.namedItem = function(name) {{
        var str = String(name);
        for (var i = 0; i < this.length; i++) {{
            var el = this[i];
            if (el && (el.id === str || (el.getAttribute && el.getAttribute('name') === str))) {{
                return el;
            }}
        }}
        return null;
    }};
    globalThis.HTMLCollection.prototype[Symbol.iterator] = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: self[i++], done: false }};
                }}
                return {{ value: undefined, done: true }};
            }}
        }};
    }};
    globalThis.HTMLCollection.prototype[Symbol.toStringTag] = 'HTMLCollection';

    globalThis.NodeList = function() {{}};
    globalThis.NodeList.prototype = Object.create(Object.prototype);
    globalThis.NodeList.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    }};
    globalThis.NodeList.prototype.forEach = function(cb, thisArg) {{
        for (var i = 0; i < this.length; i++) {{
            cb.call(thisArg, this[i], i, this);
        }}
    }};
    globalThis.NodeList.prototype.entries = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: [i, self[i++]], done: false }};
                }}
                return {{ value: undefined, done: true }};
            }},
            [Symbol.iterator]: function() {{ return this; }}
        }};
    }};
    globalThis.NodeList.prototype.keys = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: i++, done: false }};
                }}
                return {{ value: undefined, done: true }};
            }},
            [Symbol.iterator]: function() {{ return this; }}
        }};
    }};
    globalThis.NodeList.prototype.values = function() {{
        var i = 0;
        var self = this;
        return {{
            next: function() {{
                if (i < self.length) {{
                    return {{ value: self[i++], done: false }};
                }}
                return {{ value: undefined, done: true }};
            }},
            [Symbol.iterator]: function() {{ return this; }}
        }};
    }};
    globalThis.NodeList.prototype[Symbol.iterator] = function() {{
        return this.values();
    }};
    globalThis.NodeList.prototype[Symbol.toStringTag] = 'NodeList';
    globalThis.NodeFilter = {{
        FILTER_ACCEPT:1, FILTER_REJECT:2, FILTER_SKIP:3,
        SHOW_ALL:-1, SHOW_ELEMENT:1, SHOW_ATTRIBUTE:2, SHOW_TEXT:4,
        SHOW_COMMENT:128, SHOW_DOCUMENT:256, SHOW_DOCUMENT_FRAGMENT:1024
    }};
    function DOMRectReadOnly(x, y, width, height) {{
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }}
    Object.defineProperties(DOMRectReadOnly.prototype, {{
        top:    {{ get: function() {{ return this.y; }}, enumerable: true }},
        left:   {{ get: function() {{ return this.x; }}, enumerable: true }},
        right:  {{ get: function() {{ return this.x + this.width; }}, enumerable: true }},
        bottom: {{ get: function() {{ return this.y + this.height; }}, enumerable: true }}
    }});
    DOMRectReadOnly.prototype.toJSON = function() {{
        return {{ x: this.x, y: this.y, width: this.width, height: this.height, top: this.top, left: this.left, right: this.right, bottom: this.bottom }};
    }};
    DOMRectReadOnly.fromRect = function(other) {{
        return new DOMRectReadOnly(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    }};
    DOMRectReadOnly.prototype[Symbol.toStringTag] = 'DOMRectReadOnly';
    globalThis.DOMRectReadOnly = DOMRectReadOnly;

    function DOMRect(x, y, width, height) {{
        this.x = Number(x) || 0;
        this.y = Number(y) || 0;
        this.width = Number(width) || 0;
        this.height = Number(height) || 0;
    }}
    DOMRect.prototype = Object.create(DOMRectReadOnly.prototype);
    DOMRect.prototype.constructor = DOMRect;
    DOMRect.fromRect = function(other) {{
        return new DOMRect(other ? other.x : 0, other ? other.y : 0, other ? other.width : 0, other ? other.height : 0);
    }};
    DOMRect.prototype[Symbol.toStringTag] = 'DOMRect';
    globalThis.DOMRect = DOMRect;

    function DOMRectList(rects) {{
        var arr = rects || [];
        this.length = arr.length;
        for (var i = 0; i < arr.length; i++) {{
            this[i] = arr[i];
        }}
    }}
    DOMRectList.prototype.item = function(index) {{
        var i = Number(index);
        return (i >= 0 && i < this.length) ? this[i] : null;
    }};
    DOMRectList.prototype[Symbol.iterator] = function() {{
        var self = this;
        var idx = 0;
        return {{
            next: function() {{
                if (idx < self.length) {{
                    return {{ value: self[idx++], done: false }};
                }} else {{
                    return {{ value: undefined, done: true }};
                }}
            }}
        }};
    }};
    DOMRectList.prototype[Symbol.toStringTag] = 'DOMRectList';
    globalThis.DOMRectList = DOMRectList;
    globalThis.DOMPoint = function(x,y,z,w) {{ this.x=x||0; this.y=y||0; this.z=z||0; this.w=w!==undefined?w:1; }};
    globalThis.DOMMatrix = function() {{
        this.a=1; this.b=0; this.c=0; this.d=1; this.e=0; this.f=0;
        this.is2D=true; this.isIdentity=true;
        this.transformPoint=function(p){{ return p; }};
        this.multiply=function(){{ return this; }};
        this.inverse=function(){{ return this; }};
    }};

    // ── 25. Image / Audio constructors ────────────────────────────────────────
    globalThis.Image = function(width, height) {{
        var img = (typeof document !== 'undefined' && document.createElement) ? document.createElement('img') : {{ tagName:'IMG', style:{{}} }};
        if (width  !== undefined) img.width  = width;
        if (height !== undefined) img.height = height;
        return img;
    }};
    globalThis.HTMLImageElement = globalThis.Image;
    globalThis.Audio = function(src) {{
        var audio = (typeof document !== 'undefined' && document.createElement) ? document.createElement('audio') : Object.create(globalThis.HTMLAudioElement.prototype);
        if (src !== undefined) audio.src = src;
        return audio;
    }};
    globalThis.Audio.prototype = globalThis.HTMLAudioElement.prototype;

    // ── 26. Observers ─────────────────────────────────────────────────────────
    function MutationRecord(type, target) {{
        this.type = type;
        this.target = target;
        this.addedNodes = [];
        this.removedNodes = [];
        this.previousSibling = null;
        this.nextSibling = null;
        this.attributeName = null;
        this.attributeNamespace = null;
        this.oldValue = null;
    }}
    MutationRecord.prototype[Symbol.toStringTag] = 'MutationRecord';
    globalThis.MutationRecord = MutationRecord;

    var _activeMutationObservers = [];

    function MutationObserver(cb) {{
        if (typeof cb !== 'function') throw new TypeError("MutationObserver callback must be a function");
        this._cb = cb;
        this._observations = [];
        this._records = [];
        this._scheduled = false;
        _activeMutationObservers.push(this);
    }}
    MutationObserver.prototype.observe = function(target, options) {{
        if (!target) return;
        var opts = options || {{}};
        this._observations = this._observations.filter(function(o) {{ return o.target !== target; }});
        this._observations.push({{ target: target, options: opts }});
    }};
    MutationObserver.prototype.disconnect = function() {{
        this._observations = [];
        this._records = [];
        var self = this;
        _activeMutationObservers = _activeMutationObservers.filter(function(o) {{ return o !== self; }});
    }};
    MutationObserver.prototype.takeRecords = function() {{
        var recs = this._records.slice();
        this._records = [];
        return recs;
    }};
    MutationObserver.prototype._queueRecord = function(record) {{
        this._records.push(record);
        if (this._scheduled) return;
        this._scheduled = true;
        var self = this;
        var deliver = function() {{
            self._scheduled = false;
            var records = self.takeRecords();
            if (records.length > 0) {{
                try {{
                    self._cb(records, self);
                }} catch(e) {{
                    if (typeof console !== 'undefined' && console.error) console.error(e);
                }}
            }}
        }};
        if (typeof queueMicrotask === 'function') {{
            queueMicrotask(deliver);
        }} else if (typeof Promise !== 'undefined') {{
            Promise.resolve().then(deliver);
        }} else if (typeof setTimeout !== 'undefined') {{
            setTimeout(deliver, 0);
        }} else {{
            deliver();
        }}
    }};
    MutationObserver.prototype[Symbol.toStringTag] = 'MutationObserver';
    globalThis.MutationObserver = MutationObserver;

    globalThis._reportDOMMutation = function(detail) {{
        if (_activeMutationObservers.length === 0) return;
        for (var i = 0; i < _activeMutationObservers.length; i++) {{
            var obs = _activeMutationObservers[i];
            for (var j = 0; j < obs._observations.length; j++) {{
                var ob = obs._observations[j];
                var matchesTarget = (ob.target === detail.target);
                if (!matchesTarget && ob.options.subtree && ob.target.contains) {{
                    matchesTarget = ob.target.contains(detail.target);
                }}
                if (!matchesTarget) continue;

                var opt = ob.options;
                if (detail.type === 'childList' && opt.childList) {{
                    var rec = new MutationRecord('childList', detail.target);
                    rec.addedNodes = detail.addedNodes || [];
                    rec.removedNodes = detail.removedNodes || [];
                    rec.previousSibling = detail.previousSibling || null;
                    rec.nextSibling = detail.nextSibling || null;
                    obs._queueRecord(rec);
                }} else if (detail.type === 'attributes' && (opt.attributes || opt.attributeFilter || opt.attributeOldValue)) {{
                    if (opt.attributeFilter && Array.isArray(opt.attributeFilter) && opt.attributeFilter.indexOf(detail.attributeName) === -1) {{
                        continue;
                    }}
                    var rec = new MutationRecord('attributes', detail.target);
                    rec.attributeName = detail.attributeName;
                    rec.attributeNamespace = detail.attributeNamespace || null;
                    if (opt.attributeOldValue) rec.oldValue = detail.oldValue !== undefined ? detail.oldValue : null;
                    obs._queueRecord(rec);
                }} else if (detail.type === 'characterData' && (opt.characterData || opt.characterDataOldValue)) {{
                    var rec = new MutationRecord('characterData', detail.target);
                    if (opt.characterDataOldValue) rec.oldValue = detail.oldValue !== undefined ? detail.oldValue : null;
                    obs._queueRecord(rec);
                }}
            }}
        }}
    }};

    function IntersectionObserverEntry(target, rootBounds, boundingClientRect, intersectionRect, isIntersecting, intersectionRatio, time) {{
        this.target = target;
        this.rootBounds = rootBounds;
        this.boundingClientRect = boundingClientRect;
        this.intersectionRect = intersectionRect;
        this.isIntersecting = isIntersecting;
        this.intersectionRatio = intersectionRatio;
        this.time = time || (typeof performance !== 'undefined' && performance.now ? performance.now() : Date.now());
        this.isVisible = false;
    }}
    IntersectionObserverEntry.prototype[Symbol.toStringTag] = 'IntersectionObserverEntry';
    globalThis.IntersectionObserverEntry = IntersectionObserverEntry;

    var _activeIntersectionObservers = [];

    function IntersectionObserver(cb, options) {{
        if (typeof cb !== 'function') throw new TypeError("IntersectionObserver callback must be a function");
        this._cb = cb;
        this._options = options || {{}};
        this.root = this._options.root || null;
        this.rootMargin = this._options.rootMargin || "0px 0px 0px 0px";
        this.thresholds = Array.isArray(this._options.threshold) ? this._options.threshold : [this._options.threshold !== undefined ? Number(this._options.threshold) : 0];
        this._targets = [];
        this._records = [];
        this._scheduled = false;
        _activeIntersectionObservers.push(this);
    }}
    IntersectionObserver.prototype.observe = function(target) {{
        if (!target) return;
        if (this._targets.indexOf(target) === -1) {{
            this._targets.push(target);
        }}
        this._computeAndSchedule();
    }};
    IntersectionObserver.prototype.unobserve = function(target) {{
        this._targets = this._targets.filter(function(t) {{ return t !== target; }});
    }};
    IntersectionObserver.prototype.disconnect = function() {{
        this._targets = [];
        this._records = [];
        var self = this;
        _activeIntersectionObservers = _activeIntersectionObservers.filter(function(o) {{ return o !== self; }});
    }};
    IntersectionObserver.prototype.takeRecords = function() {{
        var recs = this._records.slice();
        this._records = [];
        return recs;
    }};
    IntersectionObserver.prototype._computeAndSchedule = function() {{
        var self = this;
        var rootRect;
        if (self.root && typeof self.root.getBoundingClientRect === 'function') {{
            rootRect = self.root.getBoundingClientRect();
        }} else {{
            var vw = (typeof window !== 'undefined' && window.innerWidth) || 800;
            var vh = (typeof window !== 'undefined' && window.innerHeight) || 600;
            rootRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(0, 0, vw, vh) : {{ x:0, y:0, top:0, left:0, width:vw, height:vh, right:vw, bottom:vh }};
        }}

        var mTop = 0, mRight = 0, mBottom = 0, mLeft = 0;
        if (self.rootMargin) {{
            var parts = String(self.rootMargin).trim().split(/\s+/).map(function(p) {{ return parseFloat(p) || 0; }});
            if (parts.length === 1) {{ mTop = mRight = mBottom = mLeft = parts[0]; }}
            else if (parts.length === 2) {{ mTop = mBottom = parts[0]; mRight = mLeft = parts[1]; }}
            else if (parts.length === 4) {{ mTop = parts[0]; mRight = parts[1]; mBottom = parts[2]; mLeft = parts[3]; }}
        }}
        var rLeft = rootRect.left - mLeft;
        var rTop = rootRect.top - mTop;
        var rRight = rootRect.right + mRight;
        var rBottom = rootRect.bottom + mBottom;
        var rWidth = rRight - rLeft;
        var rHeight = rBottom - rTop;
        var expandedRootRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(rLeft, rTop, rWidth, rHeight) : {{ x:rLeft, y:rTop, top:rTop, left:rLeft, width:rWidth, height:rHeight, right:rRight, bottom:rBottom }};

        for (var i = 0; i < self._targets.length; i++) {{
            var target = self._targets[i];
            var bRect = (typeof target.getBoundingClientRect === 'function') ? target.getBoundingClientRect() : {{ x:0, y:0, top:0, left:0, width:0, height:0, right:0, bottom:0 }};
            var iLeft = Math.max(bRect.left, rLeft);
            var iTop = Math.max(bRect.top, rTop);
            var iRight = Math.min(bRect.right, rRight);
            var iBottom = Math.min(bRect.bottom, rBottom);

            var isIntersecting = (iRight > iLeft && iBottom > iTop) ||
                (bRect.width === 0 && bRect.height === 0 && bRect.left >= rLeft && bRect.right <= rRight && bRect.top >= rTop && bRect.bottom <= rBottom);
            var iWidth = isIntersecting ? Math.max(0, iRight - iLeft) : 0;
            var iHeight = isIntersecting ? Math.max(0, iBottom - iTop) : 0;
            var intersectRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(isIntersecting ? iLeft : 0, isIntersecting ? iTop : 0, iWidth, iHeight) : {{ x: isIntersecting ? iLeft : 0, y: isIntersecting ? iTop : 0, top: isIntersecting ? iTop : 0, left: isIntersecting ? iLeft : 0, width: iWidth, height: iHeight, right: isIntersecting ? iRight : 0, bottom: isIntersecting ? iBottom : 0 }};

            var targetArea = (bRect.width || 0) * (bRect.height || 0);
            var intersectArea = iWidth * iHeight;
            var ratio = 0;
            if (targetArea > 0) {{
                ratio = intersectArea / targetArea;
            }} else if (isIntersecting) {{
                ratio = 1;
            }}

            var entry = new IntersectionObserverEntry(target, expandedRootRect, bRect, intersectRect, isIntersecting, ratio);
            self._records.push(entry);
        }}

        if (!self._scheduled) {{
            self._scheduled = true;
            var deliver = function() {{
                self._scheduled = false;
                var records = self.takeRecords();
                if (records.length > 0) {{
                    try {{
                        self._cb(records, self);
                    }} catch(e) {{
                        if (typeof console !== 'undefined' && console.error) console.error(e);
                    }}
                }}
            }};
            if (typeof queueMicrotask === 'function') {{
                queueMicrotask(deliver);
            }} else if (typeof Promise !== 'undefined') {{
                Promise.resolve().then(deliver);
            }} else if (typeof setTimeout !== 'undefined') {{
                setTimeout(deliver, 0);
            }} else {{
                deliver();
            }}
        }}
    }};
    IntersectionObserver.prototype[Symbol.toStringTag] = 'IntersectionObserver';
    globalThis.IntersectionObserver = IntersectionObserver;

    // ── Lazy Loading with IntersectionObserver (loading="lazy") ────────────────
    var _lazyLoadObserver = null;
    function _getLazyObserver() {{
        if (!_lazyLoadObserver && typeof IntersectionObserver === 'function') {{
            _lazyLoadObserver = new IntersectionObserver(function(entries) {{
                entries.forEach(function(entry) {{
                    if (entry.isIntersecting) {{
                        var el = entry.target;
                        if (el) {{
                            var dataSrc = el.getAttribute ? el.getAttribute('data-src') : null;
                            if (dataSrc) {{
                                if (el.setAttribute) el.setAttribute('src', dataSrc);
                                else el.src = dataSrc;
                                if (el.removeAttribute) el.removeAttribute('data-src');
                            }}
                            el._lazyLoaded = true;
                            if (_lazyLoadObserver) _lazyLoadObserver.unobserve(el);
                        }}
                    }}
                }});
            }}, {{ rootMargin: '200px 0px' }});
        }}
        return _lazyLoadObserver;
    }}

    function _setupLazyLoadingProperty(proto) {{
        if (!proto) return;
        Object.defineProperty(proto, 'loading', {{
            get: function() {{ return this.getAttribute ? (this.getAttribute('loading') || 'eager') : 'eager'; }},
            set: function(val) {{
                var s = String(val).toLowerCase();
                if (this.setAttribute) this.setAttribute('loading', s);
                if (s === 'lazy') {{
                    var obs = _getLazyObserver();
                    if (obs) obs.observe(this);
                }}
            }},
            configurable: true
        }});
    }}
    _setupLazyLoadingProperty(globalThis.HTMLElement ? globalThis.HTMLElement.prototype : null);
    _setupLazyLoadingProperty(globalThis.HTMLImageElement ? globalThis.HTMLImageElement.prototype : null);
    _setupLazyLoadingProperty(globalThis.HTMLIFrameElement ? globalThis.HTMLIFrameElement.prototype : null);

    function ResizeObserverSize(inlineSize, blockSize) {{
        this.inlineSize = Number(inlineSize) || 0;
        this.blockSize = Number(blockSize) || 0;
        Object.freeze(this);
    }}
    ResizeObserverSize.prototype[Symbol.toStringTag] = 'ResizeObserverSize';
    globalThis.ResizeObserverSize = ResizeObserverSize;

    function ResizeObserverEntry(target) {{
        this.target = target;
        var r = (typeof target.getBoundingClientRect === 'function') ? target.getBoundingClientRect() : {{ x:0, y:0, width:0, height:0 }};
        var w = r.width || 0;
        var h = r.height || 0;
        this.contentRect = (globalThis.DOMRectReadOnly) ? new globalThis.DOMRectReadOnly(r.x, r.y, w, h) : r;
        this.borderBoxSize = Object.freeze([new ResizeObserverSize(w, h)]);
        this.contentBoxSize = Object.freeze([new ResizeObserverSize(w, h)]);
        var dpr = (typeof window !== 'undefined' && window.devicePixelRatio) || 1;
        this.devicePixelContentBoxSize = Object.freeze([new ResizeObserverSize(Math.round(w * dpr), Math.round(h * dpr))]);
    }}
    ResizeObserverEntry.prototype[Symbol.toStringTag] = 'ResizeObserverEntry';
    globalThis.ResizeObserverEntry = ResizeObserverEntry;

    var _activeResizeObservers = [];
    var _resizeObserverLoopCount = 0;
    var _resizeObserverResetScheduled = false;

    function ResizeObserver(cb) {{
        if (typeof cb !== 'function') throw new TypeError("ResizeObserver callback must be a function");
        this._cb = cb;
        this._targets = [];
        this._scheduled = false;
        _activeResizeObservers.push(this);
    }}
    ResizeObserver.prototype.observe = function(target, options) {{
        if (!target) return;
        var idx = this._targets.findIndex(function(t) {{ return t.target === target; }});
        if (idx === -1) {{
            this._targets.push({{ target: target, options: options || {{}} }});
        }}
        this._scheduleDelivery();
    }};
    ResizeObserver.prototype.unobserve = function(target) {{
        this._targets = this._targets.filter(function(t) {{ return t.target !== target; }});
    }};
    ResizeObserver.prototype.disconnect = function() {{
        this._targets = [];
        var self = this;
        _activeResizeObservers = _activeResizeObservers.filter(function(o) {{ return o !== self; }});
    }};
    ResizeObserver.prototype._scheduleDelivery = function() {{
        if (this._scheduled) return;
        this._scheduled = true;
        _resizeObserverLoopCount++;
        if (_resizeObserverLoopCount > 20) {{
            this._scheduled = false;
            if (typeof window !== 'undefined' && typeof window.dispatchEvent === 'function') {{
                var errEvt;
                if (typeof globalThis.ErrorEvent === 'function') {{
                    errEvt = new globalThis.ErrorEvent('error', {{
                        message: 'ResizeObserver loop completed with undelivered notifications.'
                    }});
                }} else if (typeof globalThis.Event === 'function') {{
                    errEvt = new globalThis.Event('error');
                    errEvt.message = 'ResizeObserver loop completed with undelivered notifications.';
                }}
                if (errEvt) window.dispatchEvent(errEvt);
            }}
            return;
        }}
        if (!_resizeObserverResetScheduled) {{
            _resizeObserverResetScheduled = true;
            var resetLoop = function() {{
                _resizeObserverLoopCount = 0;
                _resizeObserverResetScheduled = false;
            }};
            if (typeof setTimeout !== 'undefined') setTimeout(resetLoop, 0);
            else if (typeof queueMicrotask === 'function') queueMicrotask(resetLoop);
        }}
        var self = this;
        var deliver = function() {{
            self._scheduled = false;
            if (self._targets.length === 0) return;
            var entries = self._targets.map(function(t) {{
                return new ResizeObserverEntry(t.target);
            }});
            try {{
                self._cb(entries, self);
            }} catch(e) {{
                if (typeof console !== 'undefined' && console.error) console.error(e);
            }}
        }};
        if (typeof queueMicrotask === 'function') {{
            queueMicrotask(deliver);
        }} else if (typeof Promise !== 'undefined') {{
            Promise.resolve().then(deliver);
        }} else if (typeof setTimeout !== 'undefined') {{
            setTimeout(deliver, 0);
        }} else {{
            deliver();
        }}
    }};
    ResizeObserver.prototype[Symbol.toStringTag] = 'ResizeObserver';
    globalThis.ResizeObserver = ResizeObserver;
    globalThis.PerformanceObserver = function(cb) {{
        this._cb = cb;
        this.observe    = function() {{}};
        this.disconnect = function() {{}};
        this.takeRecords = function() {{ return []; }};
    }};

    // ── 27. Event classes ─────────────────────────────────────────────────────
    globalThis.Event = function(type, dict) {{
        this.type              = String(type);
        this.bubbles           = !!(dict && dict.bubbles);
        this.cancelable        = (dict && dict.cancelable !== undefined) ? !!dict.cancelable : true;
        this.composed          = !!(dict && dict.composed);
        this.defaultPrevented  = false;
        this.cancelBubble      = false;
        this.timeStamp         = Date.now();
        this.eventPhase        = 0;
        this.isTrusted         = false;
        this.target            = null;
        this.currentTarget     = null;
        this._stopped          = false;
        this._stopImmediate    = false;
        this._currentPassive   = false;
        this.preventDefault    = function() {{
            if (this._currentPassive) {{
                if (typeof console !== 'undefined' && console.warn) {{
                    console.warn("Unable to preventDefault inside passive event listener invocation.");
                }}
                return;
            }}
            if (this.cancelable) {{
                this.defaultPrevented = true;
            }}
        }};
        this.stopPropagation   = function() {{
            this._stopped = true;
            this.cancelBubble = true;
        }};
        this.stopImmediatePropagation = function() {{
            this._stopped = true;
            this._stopImmediate = true;
            this.cancelBubble = true;
        }};
        this.composedPath      = function() {{ return []; }};
    }};
    globalThis.Event.NONE = 0;
    globalThis.Event.CAPTURING_PHASE = 1;
    globalThis.Event.AT_TARGET = 2;
    globalThis.Event.BUBBLING_PHASE = 3;
    globalThis.Event.prototype.preventDefault = function() {{
        if (this._currentPassive) {{
            if (typeof console !== 'undefined' && console.warn) {{
                console.warn("Unable to preventDefault inside passive event listener invocation.");
            }}
            return;
        }}
        if (this.cancelable) {{
            this.defaultPrevented = true;
        }}
    }};
    globalThis.Event.prototype.stopPropagation = function() {{
        this._stopped = true;
        this.cancelBubble = true;
    }};
    globalThis.Event.prototype.stopImmediatePropagation = function() {{
        this._stopped = true;
        this._stopImmediate = true;
        this.cancelBubble = true;
    }};
    globalThis.Event.prototype.composedPath = function() {{ return []; }};
    globalThis.Event.prototype.constructor = globalThis.Event;

    globalThis.UIEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.view   = (dict && dict.view)   || globalThis.window;
        this.detail = (dict && dict.detail) || 0;
    }};
    globalThis.UIEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.UIEvent.prototype.constructor = globalThis.UIEvent;

    globalThis.MouseEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.clientX  = dict.clientX  || 0; this.clientY  = dict.clientY  || 0;
        this.screenX  = dict.screenX  || 0; this.screenY  = dict.screenY  || 0;
        this.pageX    = dict.pageX    || 0; this.pageY    = dict.pageY    || 0;
        this.offsetX  = dict.offsetX  || 0; this.offsetY  = dict.offsetY  || 0;
        this.button   = dict.button   || 0; this.buttons  = dict.buttons  || 0;
        this.ctrlKey  = !!dict.ctrlKey;  this.shiftKey = !!dict.shiftKey;
        this.altKey   = !!dict.altKey;   this.metaKey  = !!dict.metaKey;
        this.relatedTarget = dict.relatedTarget || null;
        this.getModifierState = function(key) {{ return false; }};
    }};
    globalThis.MouseEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.MouseEvent.prototype.constructor = globalThis.MouseEvent;

    globalThis.PointerEvent = function(type, dict) {{
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {{}};
        this.pointerId   = dict.pointerId   || 0;
        this.pointerType = dict.pointerType || 'mouse';
        this.isPrimary   = dict.isPrimary   !== false;
        this.pressure    = dict.pressure    || 0;
        this.width       = dict.width       || 1;
        this.height      = dict.height      || 1;
    }};
    globalThis.PointerEvent.prototype = Object.create(globalThis.MouseEvent.prototype);
    globalThis.PointerEvent.prototype.constructor = globalThis.PointerEvent;

    globalThis.KeyboardEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.key      = dict.key      || '';   this.code     = dict.code     || '';
        this.keyCode  = dict.keyCode  || 0;    this.charCode = dict.charCode || 0;
        this.which    = dict.which    || 0;    this.location = dict.location || 0;
        this.repeat   = !!dict.repeat;
        this.ctrlKey  = !!dict.ctrlKey;  this.shiftKey = !!dict.shiftKey;
        this.altKey   = !!dict.altKey;   this.metaKey  = !!dict.metaKey;
        this.getModifierState = function(k) {{ return false; }};
    }};
    globalThis.KeyboardEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.KeyboardEvent.prototype.constructor = globalThis.KeyboardEvent;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_STANDARD = 0;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_LEFT     = 1;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_RIGHT    = 2;
    globalThis.KeyboardEvent.DOM_KEY_LOCATION_NUMPAD   = 3;

    globalThis.FocusEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        this.relatedTarget = (dict && dict.relatedTarget) || null;
    }};
    globalThis.FocusEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.FocusEvent.prototype.constructor = globalThis.FocusEvent;

    globalThis.InputEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.data           = dict.data           || null;
        this.inputType      = dict.inputType      || '';
        this.isComposing    = !!dict.isComposing;
        this.dataTransfer   = dict.dataTransfer   || null;
    }};
    globalThis.InputEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.InputEvent.prototype.constructor = globalThis.InputEvent;

    globalThis.WheelEvent = function(type, dict) {{
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {{}};
        this.deltaX    = dict.deltaX    || 0;
        this.deltaY    = dict.deltaY    || 0;
        this.deltaZ    = dict.deltaZ    || 0;
        this.deltaMode = dict.deltaMode || 0;
    }};
    globalThis.WheelEvent.prototype = Object.create(globalThis.MouseEvent.prototype);
    globalThis.WheelEvent.prototype.constructor = globalThis.WheelEvent;
    globalThis.WheelEvent.DOM_DELTA_PIXEL = 0;
    globalThis.WheelEvent.DOM_DELTA_LINE  = 1;
    globalThis.WheelEvent.DOM_DELTA_PAGE  = 2;

    // Drag and Drop API (HTML5 §7.6 / Section 8.3)
    globalThis.DataTransfer = function() {{
        this.dropEffect = 'none';
        this.effectAllowed = 'none';
        this._data = {{}};
        this.files = [];
        var self = this;
        this.items = {{
            get length() {{ return Object.keys(self._data).length; }},
            add: function(data, type) {{
                var k = (type || 'text/plain').toLowerCase();
                self._data[k] = String(data);
                return {{ kind: 'string', type: k }};
            }},
            remove: function(idx) {{
                var keys = Object.keys(self._data);
                if (idx >= 0 && idx < keys.length) {{
                    delete self._data[keys[idx]];
                }}
            }},
            clear: function() {{
                self._data = {{}};
            }}
        }};
    }};
    Object.defineProperty(globalThis.DataTransfer.prototype, 'types', {{
        get: function() {{
            return Object.freeze(Object.keys(this._data || {{}}));
        }},
        enumerable: true,
        configurable: true
    }});
    globalThis.DataTransfer.prototype.setData = function(format, data) {{
        if (!this._data) this._data = {{}};
        this._data[String(format).toLowerCase()] = String(data);
    }};
    globalThis.DataTransfer.prototype.getData = function(format) {{
        if (!this._data) return '';
        return this._data[String(format).toLowerCase()] || '';
    }};
    globalThis.DataTransfer.prototype.clearData = function(format) {{
        if (!this._data) return;
        if (format === undefined) {{
            this._data = {{}};
        }} else {{
            delete this._data[String(format).toLowerCase()];
        }}
    }};
    globalThis.DataTransfer.prototype.setDragImage = function(img, x, y) {{
        this._dragImage = {{ image: img, x: x || 0, y: y || 0 }};
    }};

    globalThis.DragEvent = function(type, dict) {{
        globalThis.MouseEvent.call(this, type, dict);
        dict = dict || {{}};
        this.dataTransfer = (dict && dict.dataTransfer !== undefined) ? dict.dataTransfer : new globalThis.DataTransfer();
    }};
    globalThis.DragEvent.prototype = Object.create(globalThis.MouseEvent.prototype);
    globalThis.DragEvent.prototype.constructor = globalThis.DragEvent;

    globalThis.TouchEvent = function(type, dict) {{
        globalThis.UIEvent.call(this, type, dict);
        dict = dict || {{}};
        this.touches        = dict.touches        || [];
        this.targetTouches  = dict.targetTouches  || [];
        this.changedTouches = dict.changedTouches || [];
    }};
    globalThis.TouchEvent.prototype = Object.create(globalThis.UIEvent.prototype);
    globalThis.TouchEvent.prototype.constructor = globalThis.TouchEvent;

    globalThis.CustomEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.detail = (dict && dict.detail !== undefined) ? dict.detail : null;
        this.initCustomEvent = function(t, b, c, d) {{ this.type=t; this.bubbles=b; this.cancelable=c; this.detail=d; }};
    }};
    globalThis.CustomEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.CustomEvent.prototype.constructor = globalThis.CustomEvent;

    globalThis.ToggleEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.oldState = (dict && dict.oldState !== undefined) ? String(dict.oldState) : 'closed';
        this.newState = (dict && dict.newState !== undefined) ? String(dict.newState) : 'closed';
    }};
    globalThis.ToggleEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.ToggleEvent.prototype.constructor = globalThis.ToggleEvent;

    globalThis.HashChangeEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.oldURL = (dict && dict.oldURL) || '';
        this.newURL = (dict && dict.newURL) || '';
    }};
    globalThis.HashChangeEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.HashChangeEvent.prototype.constructor = globalThis.HashChangeEvent;

    globalThis.PopStateEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        this.state = (dict && dict.state) || null;
    }};
    globalThis.PopStateEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.PopStateEvent.prototype.constructor = globalThis.PopStateEvent;

    globalThis.ErrorEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.message  = dict.message  || '';
        this.filename = dict.filename || '';
        this.lineno   = dict.lineno   || 0;
        this.colno    = dict.colno    || 0;
        this.error    = dict.error    || null;
    }};
    globalThis.ErrorEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.ErrorEvent.prototype.constructor = globalThis.ErrorEvent;

    globalThis.StorageEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.key         = dict.key         || null;
        this.oldValue    = dict.oldValue    || null;
        this.newValue    = dict.newValue    || null;
        this.url         = dict.url         || '';
        this.storageArea = dict.storageArea || null;
    }};
    globalThis.StorageEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.StorageEvent.prototype.constructor = globalThis.StorageEvent;

    // TransitionEvent / AnimationEvent (for CSS animation callbacks)
    globalThis.TransitionEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.propertyName = dict.propertyName || '';
        this.elapsedTime  = dict.elapsedTime  || 0;
        this.pseudoElement= dict.pseudoElement|| '';
    }};
    globalThis.TransitionEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.TransitionEvent.prototype.constructor = globalThis.TransitionEvent;

    globalThis.AnimationEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.animationName = dict.animationName || '';
        this.elapsedTime   = dict.elapsedTime   || 0;
        this.pseudoElement = dict.pseudoElement || '';
    }};
    globalThis.AnimationEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.AnimationEvent.prototype.constructor = globalThis.AnimationEvent;

    globalThis.SubmitEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.submitter = dict.submitter || null;
    }};
    globalThis.SubmitEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.SubmitEvent.prototype.constructor = globalThis.SubmitEvent;

    globalThis.FormDataEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.formData = dict.formData || null;
    }};
    globalThis.FormDataEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.FormDataEvent.prototype.constructor = globalThis.FormDataEvent;

    globalThis.PageTransitionEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.persisted = !!dict.persisted;
    }};
    globalThis.PageTransitionEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.PageTransitionEvent.prototype.constructor = globalThis.PageTransitionEvent;

    globalThis.MessageEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.data = dict.data !== undefined ? dict.data : null;
        this.origin = dict.origin || '';
        this.lastEventId = dict.lastEventId || '';
        this.source = dict.source || null;
        this.ports = dict.ports || [];
    }};
    globalThis.MessageEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.MessageEvent.prototype.constructor = globalThis.MessageEvent;

    // ── 27b. MessagePort & MessageChannel (React Scheduler / WHATWG) ───────────
    function MessagePort() {{
        if (typeof globalThis.EventTarget === 'function') {{
            globalThis.EventTarget.call(this);
        }}
        this._listeners = {{}};
        this._entangled = null;
        this._closed = false;
        this._started = false;
        this._queue = [];
        this._onmessage = null;
        this._onmessageerror = null;
    }}
    if (typeof globalThis.EventTarget === 'function') {{
        MessagePort.prototype = Object.create(globalThis.EventTarget.prototype);
    }} else {{
        MessagePort.prototype = {{}};
    }}
    MessagePort.prototype.constructor = MessagePort;

    if (!MessagePort.prototype.addEventListener) {{
        MessagePort.prototype.addEventListener = function(type, listener, opts) {{
            var t = String(type);
            if (!this._listeners[t]) this._listeners[t] = [];
            this._listeners[t].push(listener);
        }};
        MessagePort.prototype.removeEventListener = function(type, listener, opts) {{
            var t = String(type);
            if (!this._listeners[t]) return;
            this._listeners[t] = this._listeners[t].filter(function(l) {{ return l !== listener; }});
        }};
        MessagePort.prototype.dispatchEvent = function(evt) {{
            var t = String(evt.type || evt);
            if (this._listeners[t]) {{
                var list = this._listeners[t].slice();
                for (var i = 0; i < list.length; i++) {{
                    try {{
                        if (typeof list[i] === 'function') list[i].call(this, evt);
                        else if (list[i] && typeof list[i].handleEvent === 'function') list[i].handleEvent(evt);
                    }} catch(e) {{}}
                }}
            }}
            return true;
        }};
    }}

    Object.defineProperty(MessagePort.prototype, 'onmessage', {{
        get: function() {{ return this._onmessage; }},
        set: function(fn) {{
            this._onmessage = fn;
            if (fn) {{
                this.start();
            }}
        }},
        configurable: true,
        enumerable: true
    }});

    Object.defineProperty(MessagePort.prototype, 'onmessageerror', {{
        get: function() {{ return this._onmessageerror; }},
        set: function(fn) {{ this._onmessageerror = fn; }},
        configurable: true,
        enumerable: true
    }});

    MessagePort.prototype.start = function() {{
        if (this._started) return;
        this._started = true;
        this._drain();
    }};

    MessagePort.prototype.close = function() {{
        this._closed = true;
        this._queue = [];
    }};

    MessagePort.prototype._drain = function() {{
        if (!this._started || this._closed) return;
        while (this._queue.length > 0) {{
            var msg = this._queue.shift();
            this._dispatch(msg);
        }}
    }};

    MessagePort.prototype._dispatch = function(msg) {{
        if (this._closed) return;
        var ev = (typeof globalThis.MessageEvent === 'function')
            ? new globalThis.MessageEvent('message', {{ data: msg.data, ports: msg.ports || [] }})
            : {{ type: 'message', data: msg.data, ports: msg.ports || [] }};
        if (typeof this._onmessage === 'function') {{
            try {{ this._onmessage.call(this, ev); }} catch(e) {{
                if (typeof console !== 'undefined' && console.error) console.error(e);
            }}
        }}
        if (typeof this.dispatchEvent === 'function') {{
            this.dispatchEvent(ev);
        }}
    }};

    MessagePort.prototype.postMessage = function(data, transfer) {{
        if (this._closed || !this._entangled || this._entangled._closed) return;
        var target = this._entangled;
        var clonedData = data;
        try {{
            if (typeof structuredClone === 'function') {{
                clonedData = structuredClone(data);
            }}
        }} catch(e) {{
            clonedData = data;
        }}

        var msg = {{
            data: clonedData,
            ports: Array.isArray(transfer) ? transfer : []
        }};

        var defer = (typeof queueMicrotask === 'function')
            ? queueMicrotask
            : function(cb) {{ setTimeout(cb, 0); }};
        defer(function() {{
            if (target._closed) return;
            if (target._started) {{
                target._dispatch(msg);
            }} else {{
                target._queue.push(msg);
            }}
        }});
    }};

    function MessageChannel() {{
        var p1 = new MessagePort();
        var p2 = new MessagePort();
        p1._entangled = p2;
        p2._entangled = p1;
        this.port1 = p1;
        this.port2 = p2;
    }}
    globalThis.MessagePort = MessagePort;
    globalThis.MessageChannel = MessageChannel;
    if (typeof window !== 'undefined') {{
        window.MessagePort = MessagePort;
        window.MessageChannel = MessageChannel;
    }}


    globalThis.CloseEvent = function(type, dict) {{
        globalThis.Event.call(this, type, dict);
        dict = dict || {{}};
        this.code = dict.code !== undefined ? dict.code : 1000;
        this.reason = dict.reason || '';
        this.wasClean = !!dict.wasClean;
    }};
    globalThis.CloseEvent.prototype = Object.create(globalThis.Event.prototype);
    globalThis.CloseEvent.prototype.constructor = globalThis.CloseEvent;

    // ── 28. Web Components ────────────────────────────────────────────────────
    globalThis.WebComponents = {{ ready: true }};
    globalThis.CustomElementRegistry = function() {{
        this._registry = {{}};
        this._whenDefinedPromises = {{}};
    }};
    globalThis.CustomElementRegistry.prototype.define = function(name, ctor, opts) {{
        name = String(name).toLowerCase();
        if (!/^[a-z][a-z0-9._-]*-[a-z0-9._-]*$/.test(name)) {{
            throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': '" + name + "' is not a valid custom element name.");
        }}
        var reserved = [
            'annotation-xml', 'color-profile', 'font-face', 'font-face-src',
            'font-face-uri', 'font-face-format', 'font-face-name', 'missing-glyph'
        ];
        if (reserved.indexOf(name) !== -1) {{
            throw new TypeError("Failed to execute 'define' on 'CustomElementRegistry': '" + name + "' is a reserved custom element name.");
        }}
        if (this._registry[name]) {{
            throw new Error("Failed to execute 'define' on 'CustomElementRegistry': the name '" + name + "' has already been used with this registry");
        }}
        this._registry[name] = ctor;
        // Run upgrades for any already-parsed elements with this tag name
        if (typeof document !== 'undefined' && document.querySelectorAll) {{
            try {{
                var els = document.querySelectorAll(name);
                for (var i = 0; i < els.length; i++) {{
                    var el = els[i];
                    if (ctor && ctor.prototype) Object.setPrototypeOf(el, ctor.prototype);
                    if (typeof el.connectedCallback === 'function' && el.isConnected) {{
                        try {{ el.connectedCallback(); }} catch(e) {{}}
                    }}
                }}
            }} catch(e) {{}}
        }}
        if (this._whenDefinedPromises && this._whenDefinedPromises[name]) {{
            var cbs = this._whenDefinedPromises[name];
            delete this._whenDefinedPromises[name];
            for (var j = 0; j < cbs.length; j++) {{
                try {{ cbs[j](ctor); }} catch(e) {{}}
            }}
        }}
    }};
    globalThis.CustomElementRegistry.prototype.get = function(n) {{ return this._registry[String(n).toLowerCase()]; }};
    globalThis.CustomElementRegistry.prototype.upgrade = function(root) {{
        if (!root || !root.querySelectorAll) return;
        var names = Object.keys(this._registry);
        for (var i = 0; i < names.length; i++) {{
            var name = names[i];
            var ctor = this._registry[name];
            var els = root.querySelectorAll(name);
            for (var j = 0; j < els.length; j++) {{
                var el = els[j];
                if (ctor && ctor.prototype) Object.setPrototypeOf(el, ctor.prototype);
                if (typeof el.connectedCallback === 'function' && el.isConnected) {{
                    try {{ el.connectedCallback(); }} catch(e) {{}}
                }}
            }}
        }}
    }};
    globalThis.CustomElementRegistry.prototype.whenDefined = function(n) {{
        var name = String(n).toLowerCase();
        if (this._registry[name]) {{
            return Promise.resolve(this._registry[name]);
        }}
        var self = this;
        return new Promise(function(resolve) {{
            if (!self._whenDefinedPromises) self._whenDefinedPromises = {{}};
            if (!self._whenDefinedPromises[name]) self._whenDefinedPromises[name] = [];
            self._whenDefinedPromises[name].push(resolve);
        }});
    }};
    globalThis.CustomElementRegistry.prototype.getName = function(ctor) {{
        return Object.keys(this._registry).find(function(k) {{ return this._registry[k] === ctor; }}, this) || null;
    }};
    if (!globalThis.customElements) {{
        globalThis.customElements = new globalThis.CustomElementRegistry();
    }}

    // ── 28.5 Constructable Stylesheets (CSSOM §2) ─────────────────────────────
    function CSSStyleSheet(options) {{
        this._rules = [];
        this._disabled = !!(options && options.disabled);
        this._media = (options && options.media) || 'all';
    }}

    Object.defineProperty(CSSStyleSheet.prototype, 'cssRules', {{
        get: function() {{ return this._rules; }},
        enumerable: true
    }});
    Object.defineProperty(CSSStyleSheet.prototype, 'rules', {{
        get: function() {{ return this._rules; }},
        enumerable: true
    }});
    Object.defineProperty(CSSStyleSheet.prototype, 'disabled', {{
        get: function() {{ return this._disabled; }},
        set: function(v) {{ this._disabled = !!v; }},
        enumerable: true
    }});
    Object.defineProperty(CSSStyleSheet.prototype, 'media', {{
        get: function() {{ return this._media; }},
        enumerable: true
    }});

    CSSStyleSheet.prototype.replaceSync = function(text) {{
        this._rules = [];
        var css = String(text);
        var blocks = css.split('}}');
        for (var i = 0; i < blocks.length; i++) {{
            var block = blocks[i].trim();
            if (!block) continue;
            var parts = block.split('{{');
            if (parts.length >= 2) {{
                var sel = parts[0].trim();
                var body = parts.slice(1).join('{{').trim();
                this._rules.push({{
                    cssText: sel + ' {{ ' + body + ' }}',
                    selectorText: sel,
                    style: {{ cssText: body }}
                }});
            }}
        }}
        return undefined;
    }};

    CSSStyleSheet.prototype.replace = function(text) {{
        var self = this;
        return new Promise(function(resolve) {{
            self.replaceSync(text);
            resolve(self);
        }});
    }};

    CSSStyleSheet.prototype.insertRule = function(rule, index) {{
        index = index === undefined ? 0 : index;
        var rStr = String(rule).trim();
        var parts = rStr.split('{{');
        var sel = parts[0].trim();
        var body = parts.length > 1 ? parts.slice(1).join('{{').replace(/}}$/, '').trim() : '';
        var parsed = {{
            cssText: rStr,
            selectorText: sel,
            style: {{ cssText: body }}
        }};
        this._rules.splice(index, 0, parsed);
        return index;
    }};

    CSSStyleSheet.prototype.deleteRule = function(index) {{
        if (index < 0 || index >= this._rules.length) {{
            throw new RangeError("Index out of range");
        }}
        this._rules.splice(index, 1);
    }};

    globalThis.CSSStyleSheet = CSSStyleSheet;
    if (typeof window !== 'undefined') {{
        window.CSSStyleSheet = CSSStyleSheet;
    }}

    // ── 29. Blob / File / FileReader ──────────────────────────────────────────
    // (Full standard implementations of Blob, File, and FileReader are defined above in section 19)

    // ── 30. Promise (ensure standard existence) ──────────────────────────────
    if (typeof Promise === 'undefined') {{
        // Minimal synchronous Promise polyfill as last-resort fallback
        globalThis.Promise = function(executor) {{
            this._state = 'pending'; this._val = undefined; this._cb = [];
            var self = this;
            function resolve(v) {{ if (self._state !== 'pending') return; self._state = 'fulfilled'; self._val = v; self._cb.forEach(function(c) {{ if (c.onFulfilled) try {{ c.onFulfilled(v); }} catch(e) {{}} }}); }}
            function reject(r)  {{ if (self._state !== 'pending') return; self._state = 'rejected';  self._val = r; self._cb.forEach(function(c) {{ if (c.onRejected)  try {{ c.onRejected(r);  }} catch(e) {{}} }}); }}
            try {{ executor(resolve, reject); }} catch(e) {{ reject(e); }}
        }};
        globalThis.Promise.prototype.then = function(onF, onR) {{
            var self = this;
            return new Promise(function(res, rej) {{
                self._cb.push({{ onFulfilled: onF ? function(v) {{ try {{ res(onF(v)); }} catch(e) {{ rej(e); }} }} : res, onRejected: onR ? function(r) {{ try {{ res(onR(r)); }} catch(e) {{ rej(e); }} }} : rej }});
                if (self._state === 'fulfilled' && onF) try {{ res(onF(self._val)); }} catch(e) {{ rej(e); }}
                if (self._state === 'rejected'  && onR) try {{ res(onR(self._val)); }} catch(e) {{ rej(e); }}
            }});
        }};
        globalThis.Promise.prototype.catch  = function(fn) {{ return this.then(undefined, fn); }};
        globalThis.Promise.prototype.finally= function(fn) {{ return this.then(function(v){{ fn(); return v; }}, function(r){{ fn(); throw r; }}); }};
        globalThis.Promise.resolve = function(v) {{ return new Promise(function(res) {{ res(v); }}); }};
        globalThis.Promise.reject  = function(r) {{ return new Promise(function(_, rej) {{ rej(r); }}); }};
        globalThis.Promise.all     = function(ps) {{ return new Promise(function(res, rej) {{ if (!ps.length) {{ res([]); return; }} var out=[],done=0; ps.forEach(function(p,i){{ Promise.resolve(p).then(function(v){{ out[i]=v; if(++done===ps.length) res(out); }},rej); }}); }}); }};
        globalThis.Promise.allSettled = function(ps) {{ return new Promise(function(res) {{ if (!ps.length) {{ res([]); return; }} var out=[],done=0; ps.forEach(function(p,i){{ Promise.resolve(p).then(function(v){{ out[i]={{status:'fulfilled',value:v}};  if(++done===ps.length) res(out); }},function(r){{ out[i]={{status:'rejected',reason:r}}; if(++done===ps.length) res(out); }}); }}); }}); }};
        globalThis.Promise.race    = function(ps) {{ return new Promise(function(res,rej) {{ ps.forEach(function(p){{ Promise.resolve(p).then(res,rej); }}); }}); }};
        globalThis.Promise.any     = function(ps) {{ return new Promise(function(res,rej) {{ var errs=[],done=0; ps.forEach(function(p,i){{ Promise.resolve(p).then(res,function(r){{ errs[i]=r; if(++done===ps.length) rej(new Error('All promises rejected')); }}); }}); }}); }};
    }}

    // ── 31. queueMicrotask ────────────────────────────────────────────────────
    globalThis.queueMicrotask = function(fn) {{
        Promise.resolve().then(function() {{ try {{ fn(); }} catch(e) {{}} }});
    }};

    // ── 32. Web Worker & Service Worker ──────────────────────────
    var _nextWorkerId = 1;
    var _activeWorkers = {{}};

    globalThis._mangoDispatchWorkerMessage = function(id, dataStr) {{
        var w = _activeWorkers[id];
        if (!w || w._terminated) return;
        var data;
        try {{ data = JSON.parse(dataStr); }} catch(e) {{ data = dataStr; }}
        var evt = (typeof globalThis.MessageEvent === 'function')
            ? new globalThis.MessageEvent('message', {{ data: data, source: w }})
            : {{ type: 'message', data: data, source: w, target: w }};
        w.dispatchEvent(evt);
    }};

    globalThis._mangoDispatchWorkerError = function(id, errMsg) {{
        var w = _activeWorkers[id];
        if (!w || w._terminated) return;
        var evt = (typeof globalThis.ErrorEvent === 'function')
            ? new globalThis.ErrorEvent('error', {{ message: errMsg }})
            : {{ type: 'error', message: errMsg, target: w }};
        w.dispatchEvent(evt);
    }};

    function Worker(scriptURL, options) {{
        EventTarget.call(this);
        this.scriptURL = String(scriptURL || '');
        this.options = options || {{}};
        this.onmessage = null;
        this.onerror = null;
        this.onmessageerror = null;
        this._listeners = {{}};
        this._terminated = false;

        var id = _nextWorkerId++;
        this._workerId = id;
        _activeWorkers[id] = this;

        var self = this;
        this.addEventListener = function(type, fn) {{
            if (!self._listeners[type]) self._listeners[type] = [];
            self._listeners[type].push(fn);
        }};
        this.removeEventListener = function(type, fn) {{
            if (!self._listeners[type]) return;
            self._listeners[type] = self._listeners[type].filter(function(f) {{ return f !== fn; }});
        }};
        this.dispatchEvent = function(evt) {{
            var type = evt.type;
            if (type === 'message' && typeof self.onmessage === 'function') {{
                try {{ self.onmessage(evt); }} catch(e) {{}}
            }}
            if (type === 'error' && typeof self.onerror === 'function') {{
                try {{ self.onerror(evt); }} catch(e) {{}}
            }}
            var list = self._listeners[type] || [];
            for (var i = 0; i < list.length; i++) {{
                try {{ list[i].call(self, evt); }} catch(e) {{}}
            }}
            return !evt.defaultPrevented;
        }};
        this.postMessage = function(message, transfer) {{
            if (self._terminated) return;
            var json = JSON.stringify(message);
            if (typeof _mangoWorkerPostMessage === 'function') {{
                _mangoWorkerPostMessage(self._workerId, json);
            }}
        }};
        this.terminate = function() {{
            self._terminated = true;
            if (typeof _mangoWorkerTerminate === 'function') {{
                _mangoWorkerTerminate(self._workerId);
            }}
            delete _activeWorkers[self._workerId];
        }};

        if (typeof _mangoWorkerCreate === 'function') {{
            _mangoWorkerCreate(id, this.scriptURL, JSON.stringify(this.options));
        }}
    }}
    Worker.prototype = Object.create(EventTarget.prototype);
    globalThis.Worker = Worker;

    // ── 13b. ServiceWorker API (W3C Service Workers) ──────────────────────────
    function ExtendableEvent(type, eventInitDict) {{
        Event.call(this, type, eventInitDict);
        this._extendLifetimePromises = [];
    }}
    ExtendableEvent.prototype = Object.create(Event.prototype);
    ExtendableEvent.prototype.waitUntil = function(p) {{
        this._extendLifetimePromises.push(Promise.resolve(p));
    }};
    globalThis.ExtendableEvent = ExtendableEvent;

    function FetchEvent(type, init) {{
        ExtendableEvent.call(this, type, init);
        this.request = (init && init.request) ? (init.request instanceof Request ? init.request : new Request(init.request)) : null;
        this.clientId = (init && init.clientId) || '';
        this.isReload = !!(init && init.isReload);
        this.resultingClientId = (init && init.resultingClientId) || '';
        this.handled = Promise.resolve();
        this.preloadResponse = Promise.resolve(undefined);
        this._responsePromise = null;
        this._responded = false;
    }}
    FetchEvent.prototype = Object.create(ExtendableEvent.prototype);
    FetchEvent.prototype.respondWith = function(r) {{
        this._responded = true;
        this._responsePromise = Promise.resolve(r);
    }};
    globalThis.FetchEvent = FetchEvent;

    function ExtendableMessageEvent(type, init) {{
        ExtendableEvent.call(this, type, init);
        this.data = (init && init.data) || null;
        this.origin = (init && init.origin) || '';
        this.lastEventId = (init && init.lastEventId) || '';
        this.source = (init && init.source) || null;
        this.ports = (init && init.ports) || [];
    }}
    ExtendableMessageEvent.prototype = Object.create(ExtendableEvent.prototype);
    globalThis.ExtendableMessageEvent = ExtendableMessageEvent;

    function ServiceWorker(scriptURL, state) {{
        EventTarget.call(this);
        this.scriptURL = scriptURL || '';
        this.state = state || 'activated';
        this.onstatechange = null;
        this.onerror = null;
        this.onfetch = null;
        this.oninstall = null;
        this.onactivate = null;
        this.onmessage = null;
        this._handlingFetch = false;
    }}
    ServiceWorker.prototype = Object.create(EventTarget.prototype);
    ServiceWorker.prototype._setState = function(newState) {{
        this.state = newState;
        var evt = new Event('statechange');
        if (typeof this.onstatechange === 'function') {{
            try {{ this.onstatechange.call(this, evt); }} catch(e) {{}}
        }}
        this.dispatchEvent(evt);
    }};
    ServiceWorker.prototype.postMessage = function(msg, transfer) {{
        var self = this;
        var evt = new ExtendableMessageEvent('message', {{ data: msg, source: self }});
        setTimeout(function() {{
            if (typeof self.onmessage === 'function') {{
                try {{ self.onmessage.call(self, evt); }} catch(e) {{}}
            }}
            self.dispatchEvent(evt);
        }}, 0);
    }};
    globalThis.ServiceWorker = ServiceWorker;

    function NavigationPreloadManager() {{
        this._enabled = false;
        this._header = '';
    }}
    NavigationPreloadManager.prototype.enable = function() {{ this._enabled = true; return Promise.resolve(); }};
    NavigationPreloadManager.prototype.disable = function() {{ this._enabled = false; return Promise.resolve(); }};
    NavigationPreloadManager.prototype.setHeaderValue = function(val) {{ this._header = String(val); return Promise.resolve(); }};
    NavigationPreloadManager.prototype.getState = function() {{ return Promise.resolve({{ enabled: this._enabled, headerValue: this._header }}); }};
    globalThis.NavigationPreloadManager = NavigationPreloadManager;

    function ServiceWorkerRegistration(scope, sw) {{
        EventTarget.call(this);
        this.scope = scope || '/';
        var activeWorker = sw || new ServiceWorker('', 'activated');
        this.installing = null;
        this.waiting = null;
        this.active = activeWorker;
        this.navigationPreload = new NavigationPreloadManager();
        this.onupdatefound = null;
    }}
    ServiceWorkerRegistration.prototype = Object.create(EventTarget.prototype);
    ServiceWorkerRegistration.prototype.update = function() {{ return Promise.resolve(this); }};
    ServiceWorkerRegistration.prototype.unregister = function() {{ return Promise.resolve(true); }};
    ServiceWorkerRegistration.prototype.showNotification = function(title, options) {{ return Promise.resolve(); }};
    ServiceWorkerRegistration.prototype.getNotifications = function() {{ return Promise.resolve([]); }};
    globalThis.ServiceWorkerRegistration = ServiceWorkerRegistration;

    function ServiceWorkerContainer() {{
        EventTarget.call(this);
        var self = this;
        var defaultActive = new ServiceWorker('/', 'activated');
        var defaultReg = new ServiceWorkerRegistration('/', defaultActive);
        this.controller = defaultActive;
        this._registrations = {{ '/': defaultReg }};
        this.ready = Promise.resolve(defaultReg);
        this.oncontrollerchange = null;
        this.onmessage = null;
    }}
    ServiceWorkerContainer.prototype = Object.create(EventTarget.prototype);
    ServiceWorkerContainer.prototype.register = function(scriptURL, options) {{
        var self = this;
        var scope = (options && options.scope) ? options.scope : '/';
        var sw = new ServiceWorker(scriptURL, 'installing');
        var reg = new ServiceWorkerRegistration(scope, sw);
        reg.installing = sw;
        self._registrations[scope] = reg;
        self.ready = Promise.resolve(reg);

        // Lifecycle: dispatch install event
        var installEvt = new ExtendableEvent('install');
        if (typeof sw.oninstall === 'function') {{
            try {{ sw.oninstall.call(sw, installEvt); }} catch(e) {{}}
        }}
        sw.dispatchEvent(installEvt);

        // Transition to installed (waiting)
        sw._setState('installed');
        reg.installing = null;
        reg.waiting = sw;

        // Transition to activating -> activated (active)
        sw._setState('activating');
        var activateEvt = new ExtendableEvent('activate');
        if (typeof sw.onactivate === 'function') {{
            try {{ sw.onactivate.call(sw, activateEvt); }} catch(e) {{}}
        }}
        sw.dispatchEvent(activateEvt);

        reg.waiting = null;
        reg.active = sw;
        sw._setState('activated');
        self.controller = sw;

        var changeEvt = new Event('controllerchange');
        if (typeof self.oncontrollerchange === 'function') {{
            try {{ self.oncontrollerchange.call(self, changeEvt); }} catch(e) {{}}
        }}
        self.dispatchEvent(changeEvt);

        return Promise.resolve(reg);
    }};
    ServiceWorkerContainer.prototype.getRegistration = function(clientURL) {{
        var url = clientURL || '/';
        var bestScope = null;
        for (var s in this._registrations) {{
            if (url.indexOf(s) === 0) {{
                if (!bestScope || s.length > bestScope.length) {{
                    bestScope = s;
                }}
            }}
        }}
        return Promise.resolve(bestScope ? this._registrations[bestScope] : undefined);
    }};
    ServiceWorkerContainer.prototype.getRegistrations = function() {{
        var list = [];
        for (var k in this._registrations) {{
            list.push(this._registrations[k]);
        }}
        return Promise.resolve(list);
    }};
    ServiceWorkerContainer.prototype.startMessages = function() {{}};
    globalThis.ServiceWorkerContainer = ServiceWorkerContainer;

    if (globalThis.navigator) {{
        globalThis.navigator.serviceWorker = new ServiceWorkerContainer();
    }}

    function Client(id, url) {{
        this.id = id || 'client-1';
        this.url = url || (globalThis.location ? globalThis.location.href : 'http://localhost/');
        this.type = 'window';
        this.frameType = 'top-level';
    }}
    Client.prototype.postMessage = function(msg) {{}};

    function Clients() {{}}
    Clients.prototype.get = function(id) {{ return Promise.resolve(new Client(id)); }};
    Clients.prototype.matchAll = function(options) {{ return Promise.resolve([new Client()]); }};
    Clients.prototype.openWindow = function(url) {{ return Promise.resolve(new Client(null, url)); }};
    Clients.prototype.claim = function() {{ return Promise.resolve(); }};

    globalThis.Client = Client;
    globalThis.Clients = Clients;
    if (!globalThis.clients) {{
        globalThis.clients = new Clients();
    }}
    if (!globalThis.skipWaiting) {{
        globalThis.skipWaiting = function() {{ return Promise.resolve(); }};
    }}

    // ── 13c. BroadcastChannel API (WHATWG HTML § 9.4) ─────────────────────────
    var _nextBroadcastId = 1;
    var _activeBroadcastChannels = {{}};

    function BroadcastChannel(name) {{
        if (!arguments.length) {{
            throw new TypeError("Failed to construct 'BroadcastChannel': 1 argument required, but only 0 present.");
        }}
        EventTarget.call(this);
        this.name = String(name);
        this._closed = false;
        this._id = _nextBroadcastId++;
        this.onmessage = null;
        this.onmessageerror = null;
        this._listeners = {{}};

        _activeBroadcastChannels[this._id] = this;

        if (typeof _mangoBroadcastCreate === 'function') {{
            _mangoBroadcastCreate(this._id, this.name);
        }}
    }}
    BroadcastChannel.prototype = Object.create(EventTarget.prototype);
    BroadcastChannel.prototype.constructor = BroadcastChannel;

    BroadcastChannel.prototype.addEventListener = function(type, fn) {{
        if (!this._listeners[type]) this._listeners[type] = [];
        this._listeners[type].push(fn);
    }};

    BroadcastChannel.prototype.removeEventListener = function(type, fn) {{
        if (!this._listeners[type]) return;
        this._listeners[type] = this._listeners[type].filter(function(f) {{ return f !== fn; }});
    }};

    BroadcastChannel.prototype.dispatchEvent = function(evt) {{
        var type = evt.type;
        if (type === 'message' && typeof this.onmessage === 'function') {{
            try {{ this.onmessage.call(this, evt); }} catch(e) {{ if (typeof console !== 'undefined') console.error(e); }}
        }}
        if (type === 'messageerror' && typeof this.onmessageerror === 'function') {{
            try {{ this.onmessageerror.call(this, evt); }} catch(e) {{ if (typeof console !== 'undefined') console.error(e); }}
        }}
        var list = this._listeners[type] || [];
        for (var i = 0; i < list.length; i++) {{
            try {{ list[i].call(this, evt); }} catch(e) {{ if (typeof console !== 'undefined') console.error(e); }}
        }}
        return !evt.defaultPrevented;
    }};

    BroadcastChannel.prototype.postMessage = function(message) {{
        if (this._closed) {{
            throw new DOMException("Failed to execute 'postMessage' on 'BroadcastChannel': The channel is closed.", "InvalidStateError");
        }}
        var cloned = message;
        try {{
            if (typeof structuredClone === 'function') {{
                cloned = structuredClone(message);
            }}
        }} catch(e) {{}}

        var self = this;
        for (var id in _activeBroadcastChannels) {{
            if (Number(id) !== self._id) {{
                var ch = _activeBroadcastChannels[id];
                if (ch && !ch._closed && ch.name === self.name) {{
                    (function(targetCh) {{
                        var defer = (typeof queueMicrotask === 'function') ? queueMicrotask : function(cb) {{ setTimeout(cb, 0); }};
                        defer(function() {{
                            if (targetCh._closed) return;
                            var ev = (typeof MessageEvent === 'function')
                                ? new MessageEvent('message', {{ data: cloned }})
                                : {{ type: 'message', data: cloned }};
                            targetCh.dispatchEvent(ev);
                        }});
                    }})(ch);
                }}
            }}
        }}

        if (typeof _mangoBroadcastPost === 'function') {{
            _mangoBroadcastPost(this._id, this.name, JSON.stringify(message));
        }}
    }};

    BroadcastChannel.prototype.close = function() {{
        if (this._closed) return;
        this._closed = true;
        if (typeof _mangoBroadcastClose === 'function') {{
            _mangoBroadcastClose(this._id, this.name);
        }}
        delete _activeBroadcastChannels[this._id];
    }};

    globalThis._mangoDispatchBroadcastMessage = function(channelId, jsonString) {{
        var ch = _activeBroadcastChannels[channelId];
        if (ch && !ch._closed) {{
            try {{
                var data = JSON.parse(jsonString);
                var ev = (typeof MessageEvent === 'function')
                    ? new MessageEvent('message', {{ data: data }})
                    : {{ type: 'message', data: data }};
                ch.dispatchEvent(ev);
            }} catch(e) {{}}
        }}
    }};

    globalThis.BroadcastChannel = BroadcastChannel;

    // ── 13d. SharedWorker API (WHATWG HTML § 10.1.2) ──────────────────────────
    var _nextSharedPortId = 1;
    var _activeSharedPorts = {{}};

    function SharedWorker(scriptURL, options) {{
        if (!arguments.length) {{
            throw new TypeError("Failed to construct 'SharedWorker': 1 argument required, but only 0 present.");
        }}
        EventTarget.call(this);
        this.scriptURL = String(scriptURL || '');
        var name = '';
        if (typeof options === 'string') {{
            name = options;
        }} else if (options && typeof options.name === 'string') {{
            name = options.name;
        }}
        this._name = name;
        this.onerror = null;

        var portId = _nextSharedPortId++;
        var port = new MessagePort();
        port._sharedPortId = portId;
        _activeSharedPorts[portId] = port;

        var origPostMessage = port.postMessage;
        port.postMessage = function(data, transfer) {{
            if (this._closed) return;
            if (typeof _mangoSharedWorkerPost === 'function') {{
                _mangoSharedWorkerPost(portId, JSON.stringify(data));
            }} else {{
                origPostMessage.call(port, data, transfer);
            }}
        }};

        var origClose = port.close;
        port.close = function() {{
            origClose.call(port);
            if (typeof _mangoSharedWorkerClosePort === 'function') {{
                _mangoSharedWorkerClosePort(portId);
            }}
            delete _activeSharedPorts[portId];
        }};

        this.port = port;

        if (typeof _mangoSharedWorkerConnect === 'function') {{
            _mangoSharedWorkerConnect(portId, this.scriptURL, this._name);
        }}
    }}
    SharedWorker.prototype = Object.create(EventTarget.prototype);
    SharedWorker.prototype.constructor = SharedWorker;

    globalThis._mangoDispatchSharedPortMessage = function(portId, jsonString) {{
        var port = _activeSharedPorts[portId];
        if (port && !port._closed) {{
            try {{
                var data = JSON.parse(jsonString);
                var msg = {{ data: data, ports: [] }};
                if (port._started || port._onmessage) {{
                    port._dispatch(msg);
                }} else {{
                    port._queue.push(msg);
                }}
            }} catch(e) {{}}
        }}
    }};

    globalThis.SharedWorker = SharedWorker;

    // ── 13e. Web Locks API (W3C Web Locks) ────────────────────────────────────
    var _nextClientId = 'client-' + Math.floor(Math.random() * 1000000);
    var _activeLockCallbacks = {{}};

    function LockManager() {{}}
    LockManager.prototype.request = function(name, optionsOrCallback, maybeCallback) {{
        var self = this;
        var callback = typeof optionsOrCallback === 'function' ? optionsOrCallback : maybeCallback;
        var options = typeof optionsOrCallback === 'object' && optionsOrCallback !== null ? optionsOrCallback : {{}};
        if (typeof callback !== 'function') {{
            return Promise.reject(new TypeError("Lock callback must be a function"));
        }}

        var mode = (options.mode === 'shared') ? 'shared' : 'exclusive';
        var ifAvailable = !!options.ifAvailable;
        var steal = !!options.steal;
        var signal = options.signal;

        return new Promise(function(resolve, reject) {{
            if (signal && signal.aborted) {{
                return reject(signal.reason || new DOMException("AbortError", "AbortError"));
            }}

            if (typeof _mangoLocksRequest !== 'function') {{
                var lockObj = {{ name: String(name), mode: mode }};
                return Promise.resolve(callback(lockObj)).then(resolve, reject);
            }}

            var resJson = _mangoLocksRequest(String(name), mode, ifAvailable, steal, _nextClientId);
            var res = JSON.parse(resJson);

            if (res.status === 'unavailable') {{
                return Promise.resolve(callback(null)).then(resolve, reject);
            }}

            var executeGrant = function(grantId, grantMode) {{
                var lockObj = {{ name: String(name), mode: grantMode || mode }};
                Promise.resolve()
                    .then(function() {{ return callback(lockObj); }})
                    .then(function(val) {{
                        if (typeof _mangoLocksRelease === 'function') {{
                            var unblockedJson = _mangoLocksRelease(String(name), grantId);
                            self._processUnblocked(unblockedJson);
                        }}
                        resolve(val);
                    }}, function(err) {{
                        if (typeof _mangoLocksRelease === 'function') {{
                            var unblockedJson = _mangoLocksRelease(String(name), grantId);
                            self._processUnblocked(unblockedJson);
                        }}
                        reject(err);
                    }});
            }};

            if (res.status === 'granted') {{
                executeGrant(res.grantId, res.mode);
            }} else if (res.status === 'queued') {{
                var reqId = res.requestId;
                _activeLockCallbacks[reqId] = {{
                    execute: executeGrant,
                    reject: reject,
                    name: String(name)
                }};

                if (signal) {{
                    signal.addEventListener('abort', function() {{
                        delete _activeLockCallbacks[reqId];
                        if (typeof _mangoLocksAbort === 'function') {{
                            _mangoLocksAbort(String(name), reqId);
                        }}
                        reject(signal.reason || new DOMException("AbortError", "AbortError"));
                    }}, {{ once: true }});
                }}
            }}
        }});
    }};

    LockManager.prototype._processUnblocked = function(unblockedJson) {{
        if (!unblockedJson) return;
        try {{
            var items = JSON.parse(unblockedJson);
            for (var i = 0; i < items.length; i++) {{
                var item = items[i];
                var cb = _activeLockCallbacks[item.requestId || item.grantId];
                if (cb) {{
                    delete _activeLockCallbacks[item.requestId || item.grantId];
                    cb.execute(item.grantId, item.mode);
                }}
            }}
        }} catch(e) {{}}
    }};

    LockManager.prototype.query = function() {{
        if (typeof _mangoLocksQuery === 'function') {{
            try {{
                var json = _mangoLocksQuery();
                return Promise.resolve(JSON.parse(json));
            }} catch(e) {{
                return Promise.reject(e);
            }}
        }}
        return Promise.resolve({{ held: [], pending: [] }});
    }};

    globalThis.LockManager = LockManager;
    if (globalThis.navigator) {{
        globalThis.navigator.locks = new LockManager();
    }}

    // ── 14. IndexedDB API (W3C Indexed Database API) ──────────────────────────
    function DOMStringList(arr) {{
        this._list = arr || [];
        for (var i = 0; i < this._list.length; i++) {{
            this[i] = this._list[i];
        }}
    }}
    DOMStringList.prototype.contains = function(str) {{
        return this._list.indexOf(str) !== -1;
    }};
    DOMStringList.prototype.item = function(index) {{
        return this._list[index] || null;
    }};
    Object.defineProperty(DOMStringList.prototype, 'length', {{
        get: function() {{ return this._list.length; }}
    }});
    globalThis.DOMStringList = DOMStringList;

    function IDBRequest() {{
        this.result = undefined;
        this.error = null;
        this.source = null;
        this.transaction = null;
        this.readyState = 'pending';
        this._onsuccess = null;
        this._onerror = null;
        this._listeners = {{}};
        this._successDispatched = false;
        this._errorDispatched = false;
    }}
    Object.defineProperty(IDBRequest.prototype, 'onsuccess', {{
        get: function() {{ return this._onsuccess; }},
        set: function(fn) {{
            this._onsuccess = fn;
            if (typeof fn === 'function' && this.readyState === 'done' && !this.error) {{
                if (this._needsUpgrade && !this._upgradeDispatched) {{
                    if (typeof this._onupgradeneeded === 'function') {{
                        this._upgradeDispatched = true;
                        try {{ this._onupgradeneeded.call(this, this._upgradeEvent); }} catch (e) {{}}
                    }}
                }}
                if (!this._successDispatched) {{
                    this._successDispatched = true;
                    try {{ fn.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
                }}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBRequest.prototype, 'onerror', {{
        get: function() {{ return this._onerror; }},
        set: function(fn) {{
            this._onerror = fn;
            if (typeof fn === 'function' && this.readyState === 'done' && this.error) {{
                if (!this._errorDispatched) {{
                    this._errorDispatched = true;
                    try {{ fn.call(this, {{ type: 'error', target: this }}); }} catch (e) {{}}
                }}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    IDBRequest.prototype.addEventListener = function(type, fn) {{
        if (!this._listeners[type]) this._listeners[type] = [];
        this._listeners[type].push(fn);
        if (type === 'upgradeneeded' && this._needsUpgrade && !this._upgradeDispatched) {{
            this._upgradeDispatched = true;
            try {{ fn.call(this, this._upgradeEvent); }} catch (e) {{}}
            if (typeof this._onsuccess === 'function' && !this._successDispatched) {{
                this._successDispatched = true;
                try {{ this._onsuccess.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
            }}
        }} else if (type === 'success' && this.readyState === 'done' && !this.error) {{
            if (!this._needsUpgrade || this._upgradeDispatched) {{
                try {{ fn.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
            }}
        }} else if (type === 'error' && this.readyState === 'done' && this.error) {{
            try {{ fn.call(this, {{ type: 'error', target: this }}); }} catch (e) {{}}
        }}
    }};
    IDBRequest.prototype.removeEventListener = function(type, fn) {{
        if (this._listeners[type]) {{
            this._listeners[type] = this._listeners[type].filter(function(f) {{ return f !== fn; }});
        }}
    }};
    IDBRequest.prototype.dispatchEvent = function(evt) {{
        var t = evt ? evt.type : 'success';
        if (t === 'success' && typeof this._onsuccess === 'function') {{
            try {{ this._onsuccess.call(this, evt); }} catch (e) {{}}
        }} else if (t === 'error' && typeof this._onerror === 'function') {{
            try {{ this._onerror.call(this, evt); }} catch (e) {{}}
        }} else if (t === 'upgradeneeded' && typeof this._onupgradeneeded === 'function') {{
            try {{ this._onupgradeneeded.call(this, evt); }} catch (e) {{}}
        }}
        if (this._listeners[t]) {{
            for (var i = 0; i < this._listeners[t].length; i++) {{
                try {{ this._listeners[t][i].call(this, evt); }} catch (e) {{}}
            }}
        }}
        return true;
    }};
    globalThis.IDBRequest = IDBRequest;

    function IDBOpenDBRequest() {{
        IDBRequest.call(this);
        this._onupgradeneeded = null;
        this._onblocked = null;
        this._needsUpgrade = false;
        this._upgradeDispatched = false;
        this._upgradeEvent = null;
    }}
    IDBOpenDBRequest.prototype = Object.create(IDBRequest.prototype);
    IDBOpenDBRequest.prototype.constructor = IDBOpenDBRequest;

    Object.defineProperty(IDBOpenDBRequest.prototype, 'onupgradeneeded', {{
        get: function() {{ return this._onupgradeneeded; }},
        set: function(fn) {{
            this._onupgradeneeded = fn;
            if (typeof fn === 'function' && this._needsUpgrade && !this._upgradeDispatched) {{
                this._upgradeDispatched = true;
                try {{ fn.call(this, this._upgradeEvent); }} catch (e) {{}}
                if (typeof this._onsuccess === 'function' && !this._successDispatched) {{
                    this._successDispatched = true;
                    try {{ this._onsuccess.call(this, {{ type: 'success', target: this }}); }} catch (e) {{}}
                }}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBOpenDBRequest.prototype, 'onblocked', {{
        get: function() {{ return this._onblocked; }},
        set: function(fn) {{ this._onblocked = fn; }},
        configurable: true,
        enumerable: true
    }});
    globalThis.IDBOpenDBRequest = IDBOpenDBRequest;

    function IDBVersionChangeEvent(type, dict) {{
        this.type = type;
        this.oldVersion = (dict && dict.oldVersion !== undefined) ? dict.oldVersion : 0;
        this.newVersion = (dict && dict.newVersion !== undefined) ? dict.newVersion : null;
        this.target = null;
    }}
    globalThis.IDBVersionChangeEvent = IDBVersionChangeEvent;

    function _extractIdbKey(obj, keyPath) {{
        if (!obj || typeof obj !== 'object' || keyPath === null || keyPath === undefined) return undefined;
        if (Array.isArray(keyPath)) {{
            var arr = [];
            for (var i = 0; i < keyPath.length; i++) {{
                arr.push(obj[keyPath[i]]);
            }}
            return arr;
        }}
        return obj[keyPath];
    }}

    function _idbKeyType(k) {{
        if (typeof k === 'number') return 1;
        if (k instanceof Date) return 2;
        if (typeof k === 'string') return 3;
        if (Array.isArray(k)) return 4;
        return 0;
    }}

    function IDBKeyRange(lower, upper, lowerOpen, upperOpen) {{
        this.lower = lower;
        this.upper = upper;
        this.lowerOpen = !!lowerOpen;
        this.upperOpen = !!upperOpen;
    }}
    IDBKeyRange.only = function(val) {{
        return new IDBKeyRange(val, val, false, false);
    }};
    IDBKeyRange.lowerBound = function(lower, open) {{
        return new IDBKeyRange(lower, undefined, open, true);
    }};
    IDBKeyRange.upperBound = function(upper, open) {{
        return new IDBKeyRange(undefined, upper, true, open);
    }};
    IDBKeyRange.bound = function(lower, upper, lowerOpen, upperOpen) {{
        return new IDBKeyRange(lower, upper, lowerOpen, upperOpen);
    }};
    IDBKeyRange.prototype.includes = function(key) {{
        if (this.lower !== undefined) {{
            var cLow = IDBFactory.prototype.cmp(key, this.lower);
            if (this.lowerOpen ? cLow <= 0 : cLow < 0) return false;
        }}
        if (this.upper !== undefined) {{
            var cUp = IDBFactory.prototype.cmp(key, this.upper);
            if (this.upperOpen ? cUp >= 0 : cUp > 0) return false;
        }}
        return true;
    }};
    globalThis.IDBKeyRange = IDBKeyRange;

    function IDBTransaction(db, storeNames, mode) {{
        this.db = db;
        this.mode = mode || 'readonly';
        this.error = null;
        this._oncomplete = null;
        this._onerror = null;
        this._onabort = null;
        this._storeNames = Array.isArray(storeNames) ? storeNames : [storeNames];
        this._active = true;
        this._listeners = {{}};
        this._completeDispatched = false;
        try {{
            this._snapshot = JSON.stringify(db._data);
        }} catch(e) {{
            this._snapshot = null;
        }}
    }}
    Object.defineProperty(IDBTransaction.prototype, 'oncomplete', {{
        get: function() {{ return this._oncomplete; }},
        set: function(fn) {{
            this._oncomplete = fn;
            if (typeof fn === 'function' && !this._completeDispatched) {{
                this._completeDispatched = true;
                try {{ fn.call(this, {{ type: 'complete', target: this }}); }} catch (e) {{}}
            }}
        }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBTransaction.prototype, 'onerror', {{
        get: function() {{ return this._onerror; }},
        set: function(fn) {{ this._onerror = fn; }},
        configurable: true,
        enumerable: true
    }});
    Object.defineProperty(IDBTransaction.prototype, 'onabort', {{
        get: function() {{ return this._onabort; }},
        set: function(fn) {{ this._onabort = fn; }},
        configurable: true,
        enumerable: true
    }});
    IDBTransaction.prototype.objectStore = function(name) {{
        if (!this.db._data.stores[name]) {{
            throw new Error("NotFoundError: The specified object store was not found: " + name);
        }}
        return new IDBObjectStore(name, this.db, this);
    }};
    IDBTransaction.prototype.abort = function() {{
        if (!this._active) return;
        this._active = false;
        if (this._snapshot) {{
            try {{
                this.db._data = JSON.parse(this._snapshot);
                this.db._persist();
            }} catch(e) {{}}
        }}
        var abortEvt = {{ type: 'abort', target: this }};
        if (typeof this._onabort === 'function') this._onabort(abortEvt);
        this.dispatchEvent(abortEvt);
    }};
    IDBTransaction.prototype.commit = function() {{
        this._active = false;
        var completeEvt = {{ type: 'complete', target: this }};
        if (typeof this._oncomplete === 'function') this._oncomplete(completeEvt);
        this.dispatchEvent(completeEvt);
    }};
    IDBTransaction.prototype.addEventListener = IDBRequest.prototype.addEventListener;
    IDBTransaction.prototype.removeEventListener = IDBRequest.prototype.removeEventListener;
    IDBTransaction.prototype.dispatchEvent = IDBRequest.prototype.dispatchEvent;
    globalThis.IDBTransaction = IDBTransaction;

    function IDBObjectStore(name, db, tx) {{
        this.name = name;
        this.transaction = tx;
        this._db = db;
        var sData = db._data.stores[name] || {{ keyPath: null, autoIncrement: false, indexes: {{}}, records: [], nextId: 1 }};
        this.keyPath = sData.keyPath;
        this.autoIncrement = sData.autoIncrement;
        this.indexNames = new DOMStringList(Object.keys(sData.indexes || {{}}));
    }}
    IDBObjectStore.prototype._save = function() {{
        this._db._persist();
    }};
    IDBObjectStore.prototype._findRecordIndex = function(key) {{
        var sData = this._db._data.stores[this.name];
        if (!sData) return -1;
        for (var i = 0; i < sData.records.length; i++) {{
            try {{
                if (IDBFactory.prototype.cmp(sData.records[i].key, key) === 0) return i;
            }} catch(e) {{
                if (sData.records[i].key === key) return i;
            }}
        }}
        return -1;
    }};
    IDBObjectStore.prototype.get = function(key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var idx = this._findRecordIndex(key);
        var res = idx !== -1 ? this._db._data.stores[this.name].records[idx].value : undefined;
        req.result = res;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.getAll = function(query, count) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var all = [];
        if (sData) {{
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {{
                var rec = sData.records[i];
                var isMatch = true;
                if (query !== undefined && query !== null) {{
                    if (query instanceof IDBKeyRange) {{
                        isMatch = query.includes(rec.key);
                    }} else if (query.includes) {{
                        isMatch = query.includes(rec.key);
                    }} else {{
                        try {{ isMatch = (IDBFactory.prototype.cmp(rec.key, query) === 0); }} catch(e) {{ isMatch = (rec.key === query); }}
                    }}
                }}
                if (isMatch) {{
                    all.push(rec.value);
                }}
            }}
        }}
        req.result = all;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.getAllKeys = function(query, count) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var all = [];
        if (sData) {{
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {{
                var rec = sData.records[i];
                var isMatch = true;
                if (query !== undefined && query !== null) {{
                    if (query instanceof IDBKeyRange) {{
                        isMatch = query.includes(rec.key);
                    }} else if (query.includes) {{
                        isMatch = query.includes(rec.key);
                    }} else {{
                        try {{ isMatch = (IDBFactory.prototype.cmp(rec.key, query) === 0); }} catch(e) {{ isMatch = (rec.key === query); }}
                    }}
                }}
                if (isMatch) {{
                    all.push(rec.key);
                }}
            }}
        }}
        req.result = all;
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.put = function(value, key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var recKey = key;
        if (recKey === undefined && this.keyPath) {{
            recKey = _extractIdbKey(value, this.keyPath);
        }}
        if (recKey === undefined && this.autoIncrement) {{
            recKey = sData.nextId++;
            if (this.keyPath && value && typeof value === 'object' && !Array.isArray(this.keyPath)) {{
                value[this.keyPath] = recKey;
            }}
        }}
        if (recKey === undefined) {{
            req.error = new Error("DataError: No key provided");
            req.readyState = 'done';
            if (typeof req._onerror === 'function') req._onerror.call(req, {{ type: 'error', target: req }});
            return req;
        }}
        // Check unique index constraints
        if (sData.indexes) {{
            for (var idxName in sData.indexes) {{
                var idxDef = sData.indexes[idxName];
                if (idxDef.unique) {{
                    var valToCheck = _extractIdbKey(value, idxDef.keyPath);
                    if (valToCheck !== undefined) {{
                        for (var r = 0; r < sData.records.length; r++) {{
                            if (sData.records[r].key !== recKey) {{
                                var existingVal = _extractIdbKey(sData.records[r].value, idxDef.keyPath);
                                try {{
                                    if (IDBFactory.prototype.cmp(existingVal, valToCheck) === 0) {{
                                        req.error = new Error("ConstraintError: Unique index constraint violated for index " + idxName);
                                        req.readyState = 'done';
                                        if (typeof req._onerror === 'function') req._onerror.call(req, {{ type: 'error', target: req }});
                                        return req;
                                    }}
                                }} catch(e) {{}}
                            }}
                        }}
                    }}
                }}
            }}
        }}
        var idx = this._findRecordIndex(recKey);
        if (idx !== -1) {{
            sData.records[idx].value = value;
        }} else {{
            sData.records.push({{ key: recKey, value: value }});
        }}
        this._save();
        req.result = recKey;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, {{ type: 'success', target: req }});
        return req;
    }};
    IDBObjectStore.prototype.add = function(value, key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var recKey = key;
        if (recKey === undefined && this.keyPath) {{
            recKey = _extractIdbKey(value, this.keyPath);
        }}
        if (recKey === undefined && this.autoIncrement) {{
            recKey = sData.nextId++;
            if (this.keyPath && value && typeof value === 'object' && !Array.isArray(this.keyPath)) {{
                value[this.keyPath] = recKey;
            }}
        }}
        if (recKey === undefined) {{
            req.error = new Error("DataError: No key provided");
            req.readyState = 'done';
            if (typeof req._onerror === 'function') req._onerror.call(req, {{ type: 'error', target: req }});
            return req;
        }}
        var idx = this._findRecordIndex(recKey);
        if (idx !== -1) {{
            req.error = new Error("ConstraintError: Key already exists in object store");
            req.readyState = 'done';
            if (typeof req._onerror === 'function') req._onerror.call(req, {{ type: 'error', target: req }});
            return req;
        }}
        // Check unique index constraints
        if (sData.indexes) {{
            for (var idxName in sData.indexes) {{
                var idxDef = sData.indexes[idxName];
                if (idxDef.unique) {{
                    var valToCheck = _extractIdbKey(value, idxDef.keyPath);
                    if (valToCheck !== undefined) {{
                        for (var r = 0; r < sData.records.length; r++) {{
                            var existingVal = _extractIdbKey(sData.records[r].value, idxDef.keyPath);
                            try {{
                                if (IDBFactory.prototype.cmp(existingVal, valToCheck) === 0) {{
                                    req.error = new Error("ConstraintError: Unique index constraint violated for index " + idxName);
                                    req.readyState = 'done';
                                    if (typeof req._onerror === 'function') req._onerror.call(req, {{ type: 'error', target: req }});
                                    return req;
                                }}
                            }} catch(e) {{}}
                        }}
                    }}
                }}
            }}
        }}
        sData.records.push({{ key: recKey, value: value }});
        this._save();
        req.result = recKey;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, {{ type: 'success', target: req }});
        return req;
    }};
    IDBObjectStore.prototype.delete = function(key) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var idx = this._findRecordIndex(key);
        if (idx !== -1) {{
            sData.records.splice(idx, 1);
            this._save();
        }}
        req.result = undefined;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, {{ type: 'success', target: req }});
        return req;
    }};
    IDBObjectStore.prototype.clear = function() {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        if (sData) {{
            sData.records = [];
            this._save();
        }}
        req.result = undefined;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, {{ type: 'success', target: req }});
        return req;
    }};
    IDBObjectStore.prototype.count = function(query) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var c = 0;
        if (sData) {{
            if (query === undefined || query === null) {{
                c = sData.records.length;
            }} else {{
                for (var i = 0; i < sData.records.length; i++) {{
                    var k = sData.records[i].key;
                    var isMatch = (query instanceof IDBKeyRange) ? query.includes(k) : (query.includes ? query.includes(k) : (k === query));
                    if (isMatch) c++;
                }}
            }}
        }}
        req.result = c;
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function') req._onsuccess.call(req, {{ type: 'success', target: req }});
        return req;
    }};
    IDBObjectStore.prototype.createIndex = function(name, keyPath, options) {{
        var sData = this._db._data.stores[this.name];
        sData.indexes[name] = {{
            name: name,
            keyPath: keyPath,
            unique: !!(options && options.unique),
            multiEntry: !!(options && options.multiEntry)
        }};
        this._save();
        this.indexNames = new DOMStringList(Object.keys(sData.indexes));
        return new IDBIndex(name, this);
    }};
    IDBObjectStore.prototype.index = function(name) {{
        var sData = this._db._data.stores[this.name];
        if (!sData.indexes[name]) {{
            throw new Error("NotFoundError: Index not found: " + name);
        }}
        return new IDBIndex(name, this);
    }};
    IDBObjectStore.prototype.deleteIndex = function(name) {{
        var sData = this._db._data.stores[this.name];
        delete sData.indexes[name];
        this._save();
        this.indexNames = new DOMStringList(Object.keys(sData.indexes));
    }};
    IDBObjectStore.prototype.openCursor = function(query, direction) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.transaction;
        var sData = this._db._data.stores[this.name];
        var records = sData ? sData.records.slice() : [];
        try {{
            records.sort(function(a, b) {{ return IDBFactory.prototype.cmp(a.key, b.key); }});
        }} catch(e) {{}}
        if (direction === 'prev' || direction === 'prevunique') {{
            records.reverse();
        }}
        var matched = [];
        for (var i = 0; i < records.length; i++) {{
            var k = records[i].key;
            var isMatch = true;
            if (query !== undefined && query !== null) {{
                if (query instanceof IDBKeyRange) {{
                    isMatch = query.includes(k);
                }} else if (query.includes) {{
                    isMatch = query.includes(k);
                }} else {{
                    try {{ isMatch = (IDBFactory.prototype.cmp(k, query) === 0); }} catch(e) {{ isMatch = (k === query); }}
                }}
            }}
            if (isMatch) {{
                matched.push(records[i]);
            }}
        }}
        if (matched.length > 0) {{
            var cursor = new IDBCursorWithValue(this, direction || 'next', matched, 0, req);
            cursor.key = matched[0].key;
            cursor.primaryKey = matched[0].key;
            cursor.value = matched[0].value;
            req.result = cursor;
        }} else {{
            req.result = null;
        }}
        req.readyState = 'done';
        return req;
    }};
    IDBObjectStore.prototype.openKeyCursor = function(query, direction) {{
        var req = this.openCursor(query, direction);
        if (req.result) {{
            req.result.value = undefined;
        }}
        return req;
    }};
    globalThis.IDBObjectStore = IDBObjectStore;

    function IDBCursor(source, direction, records, index, req) {{
        this.source = source;
        this.direction = direction || 'next';
        this._records = records;
        this._index = index;
        this._req = req;
        var cur = records[index];
        this.key = cur ? cur.key : undefined;
        this.primaryKey = cur ? (cur.primaryKey !== undefined ? cur.primaryKey : cur.key) : undefined;
    }}
    IDBCursor.prototype.continue = function() {{
        this._index++;
        if (this._index < this._records.length) {{
            var cur = this._records[this._index];
            this.key = cur.key;
            this.primaryKey = cur.primaryKey !== undefined ? cur.primaryKey : cur.key;
            if (this.value !== undefined) this.value = cur.value;
            this._req.result = this;
        }} else {{
            this._req.result = null;
        }}
        this._req._successDispatched = false;
        if (typeof this._req._onsuccess === 'function') {{
            this._req._successDispatched = true;
            try {{ this._req._onsuccess.call(this._req, {{ type: 'success', target: this._req }}); }} catch (e) {{}}
        }}
        this._req.dispatchEvent({{ type: 'success', target: this._req }});
    }};
    IDBCursor.prototype.advance = function(count) {{
        this._index += (count - 1);
        this.continue();
    }};
    IDBCursor.prototype.update = function(val) {{
        var req = new IDBRequest();
        req.source = this;
        var store = this.source.objectStore || this.source;
        var idx = store._findRecordIndex(this.primaryKey);
        if (idx !== -1) {{
            var sData = store._db._data.stores[store.name];
            sData.records[idx].value = val;
            store._save();
            this.value = val;
            req.result = this.primaryKey;
        }} else {{
            req.error = new Error("DataError: Record not found to update");
        }}
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function' && !req.error) {{
            req._onsuccess.call(req, {{ type: 'success', target: req }});
        }} else if (typeof req._onerror === 'function' && req.error) {{
            req._onerror.call(req, {{ type: 'error', target: req }});
        }}
        return req;
    }};
    IDBCursor.prototype.delete = function() {{
        var req = new IDBRequest();
        req.source = this;
        var store = this.source.objectStore || this.source;
        var idx = store._findRecordIndex(this.primaryKey);
        if (idx !== -1) {{
            var sData = store._db._data.stores[store.name];
            sData.records.splice(idx, 1);
            store._save();
            req.result = undefined;
        }} else {{
            req.error = new Error("DataError: Record not found to delete");
        }}
        req.readyState = 'done';
        if (typeof req._onsuccess === 'function' && !req.error) {{
            req._onsuccess.call(req, {{ type: 'success', target: req }});
        }} else if (typeof req._onerror === 'function' && req.error) {{
            req._onerror.call(req, {{ type: 'error', target: req }});
        }}
        return req;
    }};
    globalThis.IDBCursor = IDBCursor;

    function IDBCursorWithValue(source, direction, records, index, req) {{
        IDBCursor.call(this, source, direction, records, index, req);
        var cur = records[index];
        this.value = cur ? cur.value : undefined;
    }}
    IDBCursorWithValue.prototype = Object.create(IDBCursor.prototype);
    globalThis.IDBCursorWithValue = IDBCursorWithValue;

    function IDBIndex(name, store) {{
        this.name = name;
        this.objectStore = store;
        var idxData = store._db._data.stores[store.name].indexes[name];
        this.keyPath = idxData.keyPath;
        this.unique = idxData.unique;
        this.multiEntry = idxData.multiEntry;
    }}
    IDBIndex.prototype.get = function(key) {{
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var found = undefined;
        if (sData) {{
            for (var i = 0; i < sData.records.length; i++) {{
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                try {{
                    if (IDBFactory.prototype.cmp(idxKey, key) === 0) {{
                        found = rec.value;
                        break;
                    }}
                }} catch (e) {{
                    if (idxKey === key) {{ found = rec.value; break; }}
                }}
            }}
        }}
        req.result = found;
        req.readyState = 'done';
        return req;
    }};
    IDBIndex.prototype.getKey = function(key) {{
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var foundKey = undefined;
        if (sData) {{
            for (var i = 0; i < sData.records.length; i++) {{
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                try {{
                    if (IDBFactory.prototype.cmp(idxKey, key) === 0) {{
                        foundKey = rec.key;
                        break;
                    }}
                }} catch (e) {{
                    if (idxKey === key) {{ foundKey = rec.key; break; }}
                }}
            }}
        }}
        req.result = foundKey;
        req.readyState = 'done';
        return req;
    }};
    IDBIndex.prototype.getAll = function(query, count) {{
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var all = [];
        if (sData) {{
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {{
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                var isMatch = true;
                if (query !== undefined && query !== null) {{
                    if (query instanceof IDBKeyRange) {{
                        isMatch = query.includes(idxKey);
                    }} else if (query.includes) {{
                        isMatch = query.includes(idxKey);
                    }} else {{
                        try {{ isMatch = (IDBFactory.prototype.cmp(idxKey, query) === 0); }} catch (e) {{ isMatch = (idxKey === query); }}
                    }}
                }}
                if (isMatch) {{
                    all.push(rec.value);
                }}
            }}
        }}
        req.result = all;
        req.readyState = 'done';
        return req;
    }};
    IDBIndex.prototype.getAllKeys = function(query, count) {{
        var req = new IDBRequest();
        req.source = this;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var all = [];
        if (sData) {{
            var limit = count !== undefined ? count : sData.records.length;
            for (var i = 0; i < sData.records.length && all.length < limit; i++) {{
                var rec = sData.records[i];
                var idxKey = _extractIdbKey(rec.value, this.keyPath);
                var isMatch = true;
                if (query !== undefined && query !== null) {{
                    if (query instanceof IDBKeyRange) {{
                        isMatch = query.includes(idxKey);
                    }} else if (query.includes) {{
                        isMatch = query.includes(idxKey);
                    }} else {{
                        try {{ isMatch = (IDBFactory.prototype.cmp(idxKey, query) === 0); }} catch (e) {{ isMatch = (idxKey === query); }}
                    }}
                }}
                if (isMatch) {{
                    all.push(rec.key);
                }}
            }}
        }}
        req.result = all;
        req.readyState = 'done';
        return req;
    }};
    IDBIndex.prototype.count = function(query) {{
        return this.objectStore.count(query);
    }};
    IDBIndex.prototype.openCursor = function(query, direction) {{
        var req = new IDBRequest();
        req.source = this;
        req.transaction = this.objectStore.transaction;
        var sData = this.objectStore._db._data.stores[this.objectStore.name];
        var records = sData ? sData.records.slice() : [];
        var matched = [];
        for (var i = 0; i < records.length; i++) {{
            var rec = records[i];
            var idxKey = _extractIdbKey(rec.value, this.keyPath);
            if (idxKey !== undefined) {{
                var isMatch = true;
                if (query !== undefined && query !== null) {{
                    if (query instanceof IDBKeyRange) {{
                        isMatch = query.includes(idxKey);
                    }} else if (query.includes) {{
                        isMatch = query.includes(idxKey);
                    }} else {{
                        try {{ isMatch = (IDBFactory.prototype.cmp(idxKey, query) === 0); }} catch (e) {{ isMatch = (idxKey === query); }}
                    }}
                }}
                if (isMatch) {{
                    matched.push({{ key: idxKey, primaryKey: rec.key, value: rec.value }});
                }}
            }}
        }}
        try {{
            matched.sort(function(a, b) {{
                var c = IDBFactory.prototype.cmp(a.key, b.key);
                if (c !== 0) return c;
                return IDBFactory.prototype.cmp(a.primaryKey, b.primaryKey);
            }});
        }} catch (e) {{}}
        if (direction === 'prev' || direction === 'prevunique') {{
            matched.reverse();
        }}
        if (matched.length > 0) {{
            var cursor = new IDBCursorWithValue(this, direction || 'next', matched, 0, req);
            cursor.key = matched[0].key;
            cursor.primaryKey = matched[0].primaryKey;
            cursor.value = matched[0].value;
            req.result = cursor;
        }} else {{
            req.result = null;
        }}
        req.readyState = 'done';
        return req;
    }};
    IDBIndex.prototype.openKeyCursor = function(query, direction) {{
        var req = this.openCursor(query, direction);
        if (req.result) {{
            req.result.value = undefined;
        }}
        return req;
    }};
    globalThis.IDBIndex = IDBIndex;

    function IDBDatabase(name, version, data) {{
        this.name = name;
        this.version = version;
        this._data = data;
        this.objectStoreNames = new DOMStringList(Object.keys(data.stores));
        this._listeners = {{}};
    }}
    IDBDatabase.prototype._persist = function() {{
        _mangoIdbSave(this.name, JSON.stringify(this._data));
    }};
    IDBDatabase.prototype.createObjectStore = function(name, options) {{
        if (this._data.stores[name]) {{
            throw new Error("ConstraintError: An object store with the specified name already exists");
        }}
        this._data.stores[name] = {{
            name: name,
            keyPath: options && options.keyPath !== undefined ? options.keyPath : null,
            autoIncrement: !!(options && options.autoIncrement),
            indexes: {{}},
            records: [],
            nextId: 1
        }};
        this._persist();
        this.objectStoreNames = new DOMStringList(Object.keys(this._data.stores));
        return new IDBObjectStore(name, this, null);
    }};
    IDBDatabase.prototype.deleteObjectStore = function(name) {{
        if (!this._data.stores[name]) {{
            throw new Error("NotFoundError: The specified object store was not found");
        }}
        delete this._data.stores[name];
        this._persist();
        this.objectStoreNames = new DOMStringList(Object.keys(this._data.stores));
    }};
    IDBDatabase.prototype.transaction = function(storeNames, mode) {{
        return new IDBTransaction(this, storeNames, mode);
    }};
    IDBDatabase.prototype.close = function() {{}};
    IDBDatabase.prototype.addEventListener = IDBRequest.prototype.addEventListener;
    IDBDatabase.prototype.removeEventListener = IDBRequest.prototype.removeEventListener;
    IDBDatabase.prototype.dispatchEvent = IDBRequest.prototype.dispatchEvent;
    globalThis.IDBDatabase = IDBDatabase;

    function IDBFactory() {{}}
    IDBFactory.prototype.open = function(name, version) {{
        var req = new IDBOpenDBRequest();
        var raw = _mangoIdbLoad(name);
        var dbData = null;
        if (raw) {{
            try {{ dbData = JSON.parse(raw); }} catch (e) {{}}
        }}
        var isNew = !dbData;
        if (isNew) {{
            dbData = {{ name: name, version: 0, stores: {{}} }};
        }}
        var currentVersion = dbData.version || 0;
        var targetVersion = version !== undefined ? version : (currentVersion || 1);
        if (targetVersion < currentVersion) {{
            req.error = new Error("VersionError: Requested version is lower than current version");
            req.readyState = 'done';
            return req;
        }}
        var db = new IDBDatabase(name, targetVersion, dbData);
        req.result = db;
        req.readyState = 'done';
        if (targetVersion > currentVersion) {{
            var oldVersion = currentVersion;
            dbData.version = targetVersion;
            db.version = targetVersion;
            db._persist();
            var tx = new IDBTransaction(db, Object.keys(dbData.stores), 'versionchange');
            req.transaction = tx;
            var upEvt = new IDBVersionChangeEvent('upgradeneeded', {{
                oldVersion: oldVersion,
                newVersion: targetVersion
            }});
            upEvt.target = req;
            req._needsUpgrade = true;
            req._upgradeEvent = upEvt;
        }}
        return req;
    }};
    IDBFactory.prototype.deleteDatabase = function(name) {{
        var req = new IDBOpenDBRequest();
        _mangoIdbDelete(name);
        req.result = undefined;
        req.readyState = 'done';
        return req;
    }};
    IDBFactory.prototype.databases = function() {{
        var list = _mangoIdbList() || [];
        var res = [];
        for (var i = 0; i < list.length; i++) {{
            var raw = _mangoIdbLoad(list[i]);
            var ver = 1;
            if (raw) {{
                try {{ ver = JSON.parse(raw).version || 1; }} catch (e) {{}}
            }}
            res.push({{ name: list[i], version: ver }});
        }}
        return Promise.resolve(res);
    }};
    IDBFactory.prototype.cmp = function(a, b) {{
        var tA = _idbKeyType(a);
        var tB = _idbKeyType(b);
        if (tA === 0 || tB === 0) {{
            throw new TypeError("DataError: The operand is not a valid key.");
        }}
        if (tA !== tB) {{
            return tA < tB ? -1 : 1;
        }}
        if (tA === 1) {{
            return a < b ? -1 : (a > b ? 1 : 0);
        }}
        if (tA === 2) {{
            var t1 = a.getTime(), t2 = b.getTime();
            return t1 < t2 ? -1 : (t1 > t2 ? 1 : 0);
        }}
        if (tA === 3) {{
            return a < b ? -1 : (a > b ? 1 : 0);
        }}
        if (tA === 4) {{
            var len = Math.min(a.length, b.length);
            for (var i = 0; i < len; i++) {{
                var res = IDBFactory.prototype.cmp(a[i], b[i]);
                if (res !== 0) return res;
            }}
            return a.length < b.length ? -1 : (a.length > b.length ? 1 : 0);
        }}
        return 0;
    }};
    globalThis.IDBFactory = IDBFactory;
    globalThis.IDBDatabase = IDBDatabase;
    globalThis.IDBObjectStore = IDBObjectStore;
    globalThis.IDBIndex = IDBIndex;
    globalThis.IDBCursor = IDBCursor;
    globalThis.IDBCursorWithValue = IDBCursorWithValue;
    globalThis.indexedDB = new IDBFactory();

    // ── 34. Intl API (ECMA-402) ────────────────────────────────────────────────
    (function() {{
        var Intl = globalThis.Intl || {{}};

        // 1. NumberFormat
        function NumberFormat(locales, options) {{
            options = options || {{}};
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._style = options.style || 'decimal';
            this._currency = options.currency || 'USD';
            this._minimumFractionDigits = options.minimumFractionDigits !== undefined ? options.minimumFractionDigits : (this._style === 'currency' ? 2 : 0);
            this._maximumFractionDigits = options.maximumFractionDigits !== undefined ? options.maximumFractionDigits : (this._style === 'currency' ? 2 : 3);
            this._notation = options.notation || 'standard';
            this._useGrouping = options.useGrouping !== false;
        }}
        NumberFormat.prototype.resolvedOptions = function() {{
            return {{
                locale: this._locale,
                numberingSystem: 'latn',
                style: this._style,
                currency: this._currency,
                minimumFractionDigits: this._minimumFractionDigits,
                maximumFractionDigits: this._maximumFractionDigits,
                useGrouping: this._useGrouping,
                notation: this._notation
            }};
        }};
        NumberFormat.prototype.format = function(num) {{
            var n = Number(num);
            if (isNaN(n)) return 'NaN';
            if (this._notation === 'compact') {{
                var abs = Math.abs(n);
                var sign = n < 0 ? '-' : '';
                if (abs >= 1e9) return sign + (abs / 1e9).toFixed(1).replace(/\.0$/, '') + 'B';
                if (abs >= 1e6) return sign + (abs / 1e6).toFixed(1).replace(/\.0$/, '') + 'M';
                if (abs >= 1e3) return sign + (abs / 1e3).toFixed(1).replace(/\.0$/, '') + 'K';
                return sign + abs.toString();
            }}
            var fixed = n.toFixed(this._maximumFractionDigits);
            if (this._minimumFractionDigits < this._maximumFractionDigits) {{
                var parts = fixed.split('.');
                if (parts.length > 1) {{
                    var dec = parts[1];
                    while (dec.length > this._minimumFractionDigits && dec.endsWith('0')) {{
                        dec = dec.slice(0, -1);
                    }}
                    fixed = dec.length > 0 ? parts[0] + '.' + dec : parts[0];
                }}
            }}
            if (this._useGrouping) {{
                var p = fixed.split('.');
                p[0] = p[0].replace(/\B(?=(\d{{3}})+(?!\d))/g, ',');
                fixed = p.join('.');
            }}
            if (this._style === 'currency') {{
                var sym = (this._currency === 'EUR') ? '€' : (this._currency === 'GBP') ? '£' : (this._currency === 'JPY') ? '¥' : '$';
                return sym + fixed;
            }} else if (this._style === 'percent') {{
                return (n * 100).toFixed(this._maximumFractionDigits) + '%';
            }}
            return fixed;
        }};
        NumberFormat.prototype.formatToParts = function(num) {{
            return [{{ type: 'integer', value: this.format(num) }}];
        }};
        NumberFormat.supportedLocalesOf = function(locales) {{
            return Array.isArray(locales) ? locales.slice() : [locales || 'en-US'];
        }};
        Intl.NumberFormat = NumberFormat;

        // 2. DateTimeFormat
        function DateTimeFormat(locales, options) {{
            options = options || {{}};
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._options = options;
        }}
        DateTimeFormat.prototype.resolvedOptions = function() {{
            return Object.assign({{
                locale: this._locale,
                calendar: 'gregory',
                numberingSystem: 'latn',
                timeZone: 'UTC'
            }}, this._options);
        }};
        DateTimeFormat.prototype.format = function(date) {{
            var d = (date === undefined) ? new Date() : (date instanceof Date ? date : new Date(date));
            if (isNaN(d.getTime())) return 'Invalid Date';
            var opts = this._options;
            if (opts.year && opts.month && opts.day) {{
                return (d.getMonth() + 1) + '/' + d.getDate() + '/' + d.getFullYear();
            }}
            if (opts.hour || opts.minute) {{
                return d.toLocaleTimeString ? d.toLocaleTimeString() : d.toTimeString().split(' ')[0];
            }}
            return d.toLocaleDateString ? d.toLocaleDateString() : d.toDateString();
        }};
        DateTimeFormat.prototype.formatToParts = function(date) {{
            return [{{ type: 'literal', value: this.format(date) }}];
        }};
        DateTimeFormat.prototype.formatRange = function(start, end) {{
            return this.format(start) + ' – ' + this.format(end);
        }};
        DateTimeFormat.supportedLocalesOf = function(locales) {{
            return Array.isArray(locales) ? locales.slice() : [locales || 'en-US'];
        }};
        Intl.DateTimeFormat = DateTimeFormat;

        // 3. Collator
        function Collator(locales, options) {{
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._options = options || {{}};
        }}
        Collator.prototype.resolvedOptions = function() {{
            return {{ locale: this._locale, usage: 'sort', sensitivity: 'variant' }};
        }};
        Collator.prototype.compare = function(a, b) {{
            var sa = String(a), sb = String(b);
            return sa.localeCompare ? sa.localeCompare(sb) : (sa < sb ? -1 : (sa > sb ? 1 : 0));
        }};
        Collator.supportedLocalesOf = function(locales) {{ return [locales || 'en-US']; }};
        Intl.Collator = Collator;

        // 4. PluralRules
        function PluralRules(locales, options) {{
            this._locale = locales || 'en-US';
        }}
        PluralRules.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, type: 'cardinal' }}; }};
        PluralRules.prototype.select = function(n) {{ return Number(n) === 1 ? 'one' : 'other'; }};
        Intl.PluralRules = PluralRules;

        // 5. RelativeTimeFormat
        function RelativeTimeFormat(locales, options) {{
            this._locale = locales || 'en-US';
            this._numeric = (options && options.numeric) || 'always';
        }}
        RelativeTimeFormat.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, numeric: this._numeric }}; }};
        RelativeTimeFormat.prototype.format = function(value, unit) {{
            var v = Math.round(Number(value));
            var u = String(unit).toLowerCase();
            if (v === 0 && this._numeric === 'auto') {{
                if (u === 'day') return 'today';
                if (u === 'hour') return 'this hour';
            }}
            var pluralUnit = (Math.abs(v) === 1) ? u : (u.endsWith('s') ? u : u + 's');
            if (v < 0) return Math.abs(v) + ' ' + pluralUnit + ' ago';
            return 'in ' + v + ' ' + pluralUnit;
        }};
        RelativeTimeFormat.prototype.formatToParts = function(v, u) {{
            return [{{ type: 'literal', value: this.format(v, u) }}];
        }};
        Intl.RelativeTimeFormat = RelativeTimeFormat;

        // 6. DisplayNames
        var _displayNamesDict = {{
            'en': 'English', 'es': 'Spanish', 'fr': 'French', 'de': 'German',
            'zh': 'Chinese', 'ja': 'Japanese', 'ko': 'Korean', 'ru': 'Russian',
            'pt': 'Portuguese', 'it': 'Italian', 'ar': 'Arabic', 'hi': 'Hindi',
            'US': 'United States', 'GB': 'United Kingdom', 'CA': 'Canada',
            'FR': 'France', 'DE': 'Germany', 'JP': 'Japan', 'CN': 'China',
            'USD': 'US Dollar', 'EUR': 'Euro', 'GBP': 'British Pound', 'JPY': 'Japanese Yen'
        }};
        function DisplayNames(locales, options) {{
            this._locale = locales || 'en-US';
            this._type = (options && options.type) || 'language';
        }}
        DisplayNames.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, type: this._type }}; }};
        DisplayNames.prototype.of = function(code) {{
            var c = String(code);
            return _displayNamesDict[c] || _displayNamesDict[c.toLowerCase()] || c;
        }};
        Intl.DisplayNames = DisplayNames;

        // 7. ListFormat
        function ListFormat(locales, options) {{
            this._locale = locales || 'en-US';
            this._type = (options && options.type) || 'conjunction';
        }}
        ListFormat.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, type: this._type }}; }};
        ListFormat.prototype.format = function(list) {{
            if (!Array.isArray(list)) return '';
            if (list.length === 0) return '';
            if (list.length === 1) return String(list[0]);
            if (list.length === 2) return list[0] + (this._type === 'disjunction' ? ' or ' : ' and ') + list[1];
            var last = list[list.length - 1];
            var rest = list.slice(0, -1).join(', ');
            return rest + (this._type === 'disjunction' ? ', or ' : ', and ') + last;
        }};
        ListFormat.prototype.formatToParts = function(list) {{
            return [{{ type: 'element', value: this.format(list) }}];
        }};
        Intl.ListFormat = ListFormat;

        // 8. Segmenter
        function Segmenter(locales, options) {{
            this._locale = locales || 'en-US';
        }}
        Segmenter.prototype.resolvedOptions = function() {{ return {{ locale: this._locale, granularity: 'grapheme' }}; }};
        Segmenter.prototype.segment = function(input) {{
            var str = String(input);
            var chars = Array.from(str);
            var idx = 0;
            var segments = chars.map(function(ch) {{
                var seg = {{ segment: ch, index: idx, input: str, isWordLike: /\\w/.test(ch) }};
                idx += ch.length;
                return seg;
            }});
            return {{
                [Symbol.iterator]: function() {{
                    var i = 0;
                    return {{
                        next: function() {{
                            return i < segments.length ? {{ value: segments[i++], done: false }} : {{ done: true }};
                        }}
                    }};
                }}
            }};
        }};
        Intl.Segmenter = Segmenter;

        globalThis.Intl = Intl;
    }})();

    // -------------------------------------------------------------
    // WebGL & WebGL2 Rendering Contexts (W3C WebGL / WebGL 2 Spec)
    // -------------------------------------------------------------
    (function() {{
        var WEBGL_CONSTANTS = {{
            COLOR_BUFFER_BIT: 0x00004000,
            DEPTH_BUFFER_BIT: 0x00000100,
            STENCIL_BUFFER_BIT: 0x00000400,
            POINTS: 0x0000,
            LINES: 0x0001,
            LINE_LOOP: 0x0002,
            LINE_STRIP: 0x0003,
            TRIANGLES: 0x0004,
            TRIANGLE_STRIP: 0x0005,
            TRIANGLE_FAN: 0x0006,
            ARRAY_BUFFER: 0x8892,
            ELEMENT_ARRAY_BUFFER: 0x8893,
            STATIC_DRAW: 0x88E4,
            STREAM_DRAW: 0x88E0,
            DYNAMIC_DRAW: 0x88E8,
            FRAGMENT_SHADER: 0x8B30,
            VERTEX_SHADER: 0x8B31,
            COMPILE_STATUS: 0x8B81,
            LINK_STATUS: 0x8B82,
            VALIDATE_STATUS: 0x8B83,
            BYTE: 0x1400,
            UNSIGNED_BYTE: 0x1401,
            SHORT: 0x1402,
            UNSIGNED_SHORT: 0x1403,
            INT: 0x1404,
            UNSIGNED_INT: 0x1405,
            FLOAT: 0x1406,
            DEPTH_TEST: 0x0B71,
            BLEND: 0x0BE2,
            CULL_FACE: 0x0B44,
            TEXTURE_2D: 0x0DE1,
            TEXTURE_CUBE_MAP: 0x8513,
            TEXTURE_MAG_FILTER: 0x2800,
            TEXTURE_MIN_FILTER: 0x2801,
            TEXTURE_WRAP_S: 0x2802,
            TEXTURE_WRAP_T: 0x2803,
            NEAREST: 0x2600,
            LINEAR: 0x2601,
            RGBA: 0x1908,
            RGB: 0x1907,
            ALPHA: 0x1906,
            LUMINANCE: 0x1909,
            FRAMEBUFFER: 0x8D40,
            RENDERBUFFER: 0x8D41,
            COLOR_ATTACHMENT0: 0x8CE0,
            DEPTH_ATTACHMENT: 0x8D00,
            FRAMEBUFFER_COMPLETE: 0x8CD5,
            NO_ERROR: 0
        }};

        function WebGLShader(type) {{
            this._type = type;
            this._source = '';
            this._compiled = false;
        }}
        function WebGLProgram() {{
            this._shaders = [];
            this._linked = false;
            this._uniforms = {{}};
            this._attribs = {{}};
        }}
        function WebGLBuffer() {{
            this._target = 0;
            this._data = null;
            this._usage = 0;
        }}
        function WebGLTexture() {{
            this._params = {{}};
            this._data = null;
        }}
        function WebGLFramebuffer() {{}}
        function WebGLRenderbuffer() {{}}
        function WebGLUniformLocation(name) {{ this._name = name; }}

        function WebGLRenderingContext(canvas) {{
            this.canvas = canvas;
            this.drawingBufferWidth = canvas ? (canvas.width || 300) : 300;
            this.drawingBufferHeight = canvas ? (canvas.height || 150) : 150;
            this._clearColor = [0, 0, 0, 0];
            this._clearDepth = 1.0;
            this._clearStencil = 0;
            this._viewport = [0, 0, this.drawingBufferWidth, this.drawingBufferHeight];
            this._boundBuffers = {{}};
            this._boundTextures = {{}};
            this._currentProgram = null;
        }}

        for (var k in WEBGL_CONSTANTS) {{
            WebGLRenderingContext[k] = WEBGL_CONSTANTS[k];
            WebGLRenderingContext.prototype[k] = WEBGL_CONSTANTS[k];
        }}

        WebGLRenderingContext.prototype.viewport = function(x, y, w, h) {{ this._viewport = [x, y, w, h]; }};
        WebGLRenderingContext.prototype.clearColor = function(r, g, b, a) {{ this._clearColor = [r, g, b, a]; }};
        WebGLRenderingContext.prototype.clearDepth = function(d) {{ this._clearDepth = d; }};
        WebGLRenderingContext.prototype.clearStencil = function(s) {{ this._clearStencil = s; }};
        WebGLRenderingContext.prototype.clear = function(mask) {{}};
        WebGLRenderingContext.prototype.enable = function(cap) {{}};
        WebGLRenderingContext.prototype.disable = function(cap) {{}};
        WebGLRenderingContext.prototype.blendFunc = function(sfactor, dfactor) {{}};
        WebGLRenderingContext.prototype.createShader = function(type) {{ return new WebGLShader(type); }};
        WebGLRenderingContext.prototype.shaderSource = function(shader, source) {{ if (shader) shader._source = source; }};
        WebGLRenderingContext.prototype.compileShader = function(shader) {{ if (shader) shader._compiled = true; }};
        WebGLRenderingContext.prototype.getShaderParameter = function(shader, pname) {{
            if (pname === WEBGL_CONSTANTS.COMPILE_STATUS) return true;
            if (pname === WEBGL_CONSTANTS.DELETE_STATUS) return false;
            return true;
        }};
        WebGLRenderingContext.prototype.getShaderInfoLog = function(shader) {{ return ''; }};
        WebGLRenderingContext.prototype.createProgram = function() {{ return new WebGLProgram(); }};
        WebGLRenderingContext.prototype.attachShader = function(program, shader) {{ if (program && shader) program._shaders.push(shader); }};
        WebGLRenderingContext.prototype.linkProgram = function(program) {{ if (program) program._linked = true; }};
        WebGLRenderingContext.prototype.useProgram = function(program) {{ this._currentProgram = program; }};
        WebGLRenderingContext.prototype.getProgramParameter = function(program, pname) {{
            if (pname === WEBGL_CONSTANTS.LINK_STATUS) return true;
            if (pname === WEBGL_CONSTANTS.VALIDATE_STATUS) return true;
            return true;
        }};
        WebGLRenderingContext.prototype.getProgramInfoLog = function(program) {{ return ''; }};
        WebGLRenderingContext.prototype.createBuffer = function() {{ return new WebGLBuffer(); }};
        WebGLRenderingContext.prototype.bindBuffer = function(target, buffer) {{ this._boundBuffers[target] = buffer; }};
        WebGLRenderingContext.prototype.bufferData = function(target, data, usage) {{
            var buf = this._boundBuffers[target];
            if (buf) {{ buf._data = data; buf._usage = usage; }}
        }};
        WebGLRenderingContext.prototype.bufferSubData = function(target, offset, data) {{}};
        WebGLRenderingContext.prototype.createTexture = function() {{ return new WebGLTexture(); }};
        WebGLRenderingContext.prototype.bindTexture = function(target, texture) {{ this._boundTextures[target] = texture; }};
        WebGLRenderingContext.prototype.texImage2D = function() {{}};
        WebGLRenderingContext.prototype.texParameteri = function(target, pname, param) {{}};
        WebGLRenderingContext.prototype.generateMipmap = function(target) {{}};
        WebGLRenderingContext.prototype.createFramebuffer = function() {{ return new WebGLFramebuffer(); }};
        WebGLRenderingContext.prototype.bindFramebuffer = function(target, fb) {{}};
        WebGLRenderingContext.prototype.createRenderbuffer = function() {{ return new WebGLRenderbuffer(); }};
        WebGLRenderingContext.prototype.bindRenderbuffer = function(target, rb) {{}};
        WebGLRenderingContext.prototype.getAttribLocation = function(program, name) {{ return 0; }};
        WebGLRenderingContext.prototype.enableVertexAttribArray = function(index) {{}};
        WebGLRenderingContext.prototype.disableVertexAttribArray = function(index) {{}};
        WebGLRenderingContext.prototype.vertexAttribPointer = function() {{}};
        WebGLRenderingContext.prototype.getUniformLocation = function(program, name) {{ return new WebGLUniformLocation(name); }};
        WebGLRenderingContext.prototype.uniform1f = function(loc, x) {{}};
        WebGLRenderingContext.prototype.uniform2f = function(loc, x, y) {{}};
        WebGLRenderingContext.prototype.uniform3f = function(loc, x, y, z) {{}};
        WebGLRenderingContext.prototype.uniform4f = function(loc, x, y, z, w) {{}};
        WebGLRenderingContext.prototype.uniform1i = function(loc, x) {{}};
        WebGLRenderingContext.prototype.uniformMatrix4fv = function(loc, transpose, val) {{}};
        WebGLRenderingContext.prototype.drawArrays = function(mode, first, count) {{}};
        WebGLRenderingContext.prototype.drawElements = function(mode, count, type, offset) {{}};
        WebGLRenderingContext.prototype.getExtension = function(name) {{ return null; }};
        WebGLRenderingContext.prototype.getSupportedExtensions = function() {{ return ['OES_element_index_uint', 'EXT_texture_filter_anisotropic']; }};
        WebGLRenderingContext.prototype.getParameter = function(pname) {{
            if (pname === 0x1F00) return 'Mango WebGL Renderer';
            if (pname === 0x1F01) return 'Mango GPU Rasterizer';
            if (pname === 0x1F02) return 'WebGL 1.0 (Mango Engine)';
            if (pname === 0x8B8C) return 'WebGL GLSL ES 1.0';
            return 0;
        }};
        WebGLRenderingContext.prototype.getError = function() {{ return 0; }};

        var WEBGL2_CONSTANTS = Object.assign({{}}, WEBGL_CONSTANTS, {{
            READ_FRAMEBUFFER: 0x8CA8,
            DRAW_FRAMEBUFFER: 0x8CA9,
            UNIFORM_BUFFER: 0x8A11,
            UNIFORM_BUFFER_BINDING: 0x8A28,
            UNIFORM_BUFFER_START: 0x8A29,
            UNIFORM_BUFFER_SIZE: 0x8A2A,
            TRANSFORM_FEEDBACK_BUFFER: 0x8C8E,
            TRANSFORM_FEEDBACK_BUFFER_BINDING: 0x8C8F,
            TRANSFORM_FEEDBACK_BUFFER_START: 0x8C84,
            TRANSFORM_FEEDBACK_BUFFER_SIZE: 0x8C85,
            TRANSFORM_FEEDBACK: 0x8E22,
            TRANSFORM_FEEDBACK_PAUSED: 0x8E23,
            TRANSFORM_FEEDBACK_ACTIVE: 0x8E24,
            TRANSFORM_FEEDBACK_BINDING: 0x8E25,
            COLOR: 0x1800,
            DEPTH: 0x1801,
            STENCIL: 0x1802,
            RASTERIZER_DISCARD: 0x8C89,
            SAMPLER_2D: 0x8B5E,
            SAMPLER_CUBE: 0x8B60,
            SAMPLER_BINDING: 0x8919,
            QUERY_RESULT: 0x8866,
            QUERY_RESULT_AVAILABLE: 0x8867,
            ANY_SAMPLES_PASSED: 0x8C2F,
            ANY_SAMPLES_PASSED_CONSERVATIVE: 0x8D6A,
            SYNC_FENCE: 0x9112,
            SYNC_GPU_COMMANDS_COMPLETE: 0x9117,
            UNSIGNALED: 0x9118,
            SIGNALED: 0x9119,
            ALREADY_SIGNALED: 0x911A,
            TIMEOUT_EXPIRED: 0x911B,
            CONDITION_SATISFIED: 0x911C,
            WAIT_FAILED: 0x911D,
            SYNC_FLUSH_COMMANDS_BIT: 0x00000001
        }});

        function WebGLVertexArrayObject() {{}}
        function WebGLQuery() {{}}
        function WebGLSync() {{}}
        function WebGLTransformFeedback() {{}}
        function WebGLSampler() {{}}

        function WebGL2RenderingContext(canvas) {{
            WebGLRenderingContext.call(this, canvas);
        }}
        WebGL2RenderingContext.prototype = Object.create(WebGLRenderingContext.prototype);
        WebGL2RenderingContext.prototype.constructor = WebGL2RenderingContext;

        for (var k2 in WEBGL2_CONSTANTS) {{
            WebGL2RenderingContext[k2] = WEBGL2_CONSTANTS[k2];
            WebGL2RenderingContext.prototype[k2] = WEBGL2_CONSTANTS[k2];
        }}

        WebGL2RenderingContext.prototype.getParameter = function(pname) {{
            if (pname === 0x1F00) return 'Mango WebGL Renderer';
            if (pname === 0x1F01) return 'Mango GPU Rasterizer';
            if (pname === 0x1F02) return 'WebGL 2.0 (Mango Engine)';
            if (pname === 0x8B8C) return 'WebGL GLSL ES 3.00 (Mango Engine)';
            return 0;
        }};

        WebGL2RenderingContext.prototype.createVertexArray = function() {{ return new WebGLVertexArrayObject(); }};
        WebGL2RenderingContext.prototype.bindVertexArray = function(vao) {{}};
        WebGL2RenderingContext.prototype.deleteVertexArray = function(vao) {{}};
        WebGL2RenderingContext.prototype.isVertexArray = function(vao) {{ return vao instanceof WebGLVertexArrayObject; }};

        WebGL2RenderingContext.prototype.createQuery = function() {{ return new WebGLQuery(); }};
        WebGL2RenderingContext.prototype.beginQuery = function(target, q) {{}};
        WebGL2RenderingContext.prototype.endQuery = function(target) {{}};
        WebGL2RenderingContext.prototype.deleteQuery = function(q) {{}};
        WebGL2RenderingContext.prototype.isQuery = function(q) {{ return q instanceof WebGLQuery; }};
        WebGL2RenderingContext.prototype.getQueryParameter = function(q, pname) {{
            if (pname === 0x8867) return true;
            if (pname === 0x8866) return 0;
            return 0;
        }};

        WebGL2RenderingContext.prototype.fenceSync = function(condition, flags) {{ return new WebGLSync(); }};
        WebGL2RenderingContext.prototype.clientWaitSync = function(sync, flags, timeout) {{ return 0x911A; }};
        WebGL2RenderingContext.prototype.waitSync = function(sync, flags, timeout) {{}};
        WebGL2RenderingContext.prototype.deleteSync = function(sync) {{}};
        WebGL2RenderingContext.prototype.isSync = function(sync) {{ return sync instanceof WebGLSync; }};

        WebGL2RenderingContext.prototype.createTransformFeedback = function() {{ return new WebGLTransformFeedback(); }};
        WebGL2RenderingContext.prototype.bindTransformFeedback = function(target, tf) {{}};
        WebGL2RenderingContext.prototype.deleteTransformFeedback = function(tf) {{}};
        WebGL2RenderingContext.prototype.isTransformFeedback = function(tf) {{ return tf instanceof WebGLTransformFeedback; }};
        WebGL2RenderingContext.prototype.beginTransformFeedback = function(primitiveMode) {{}};
        WebGL2RenderingContext.prototype.endTransformFeedback = function() {{}};
        WebGL2RenderingContext.prototype.pauseTransformFeedback = function() {{}};
        WebGL2RenderingContext.prototype.resumeTransformFeedback = function() {{}};

        WebGL2RenderingContext.prototype.createSampler = function() {{ return new WebGLSampler(); }};
        WebGL2RenderingContext.prototype.bindSampler = function(unit, sampler) {{}};
        WebGL2RenderingContext.prototype.deleteSampler = function(sampler) {{}};
        WebGL2RenderingContext.prototype.isSampler = function(sampler) {{ return sampler instanceof WebGLSampler; }};
        WebGL2RenderingContext.prototype.samplerParameteri = function(sampler, pname, param) {{}};
        WebGL2RenderingContext.prototype.samplerParameterf = function(sampler, pname, param) {{}};

        WebGL2RenderingContext.prototype.bindBufferBase = function(target, index, buffer) {{}};
        WebGL2RenderingContext.prototype.bindBufferRange = function(target, index, buffer, offset, size) {{}};
        WebGL2RenderingContext.prototype.getUniformBlockIndex = function(program, uniformBlockName) {{ return 0; }};
        WebGL2RenderingContext.prototype.uniformBlockBinding = function(program, uniformBlockIndex, uniformBlockBinding) {{}};

        WebGL2RenderingContext.prototype.drawArraysInstanced = function(mode, first, count, instanceCount) {{}};
        WebGL2RenderingContext.prototype.drawElementsInstanced = function(mode, count, type, offset, instanceCount) {{}};
        WebGL2RenderingContext.prototype.vertexAttribDivisor = function(index, divisor) {{}};

        globalThis.WebGLRenderingContext = WebGLRenderingContext;
        globalThis.WebGL2RenderingContext = WebGL2RenderingContext;
        globalThis.WebGLShader = WebGLShader;
        globalThis.WebGLProgram = WebGLProgram;
        globalThis.WebGLBuffer = WebGLBuffer;
        globalThis.WebGLTexture = WebGLTexture;
        globalThis.WebGLVertexArrayObject = WebGLVertexArrayObject;
        globalThis.WebGLQuery = WebGLQuery;
        globalThis.WebGLSync = WebGLSync;
        globalThis.WebGLTransformFeedback = WebGLTransformFeedback;
        globalThis.WebGLSampler = WebGLSampler;

        // WebGPU API
        var GPUBufferUsage = {{
            MAP_READ: 0x0001,
            MAP_WRITE: 0x0002,
            COPY_SRC: 0x0004,
            COPY_DST: 0x0008,
            INDEX: 0x0010,
            VERTEX: 0x0020,
            UNIFORM: 0x0040,
            STORAGE: 0x0080,
            INDIRECT: 0x0100,
            QUERY_RESOLVE: 0x0200
        }};
        globalThis.GPUBufferUsage = GPUBufferUsage;

        var GPUTextureUsage = {{
            COPY_SRC: 0x01,
            COPY_DST: 0x02,
            TEXTURE_BINDING: 0x04,
            STORAGE_BINDING: 0x08,
            RENDER_ATTACHMENT: 0x10
        }};
        globalThis.GPUTextureUsage = GPUTextureUsage;

        var GPUShaderStage = {{
            VERTEX: 0x1,
            FRAGMENT: 0x2,
            COMPUTE: 0x4
        }};
        globalThis.GPUShaderStage = GPUShaderStage;

        var GPUMapMode = {{
            READ: 0x0001,
            WRITE: 0x0002
        }};
        globalThis.GPUMapMode = GPUMapMode;

        function GPUBuffer(descriptor) {{
            this.size = (descriptor && descriptor.size) || 0;
            this.usage = (descriptor && descriptor.usage) || 0;
            this.mapState = 'unmapped';
        }}
        GPUBuffer.prototype.mapAsync = function(mode, offset, size) {{
            this.mapState = 'mapped';
            return Promise.resolve();
        }};
        GPUBuffer.prototype.getMappedRange = function(offset, size) {{
            return new ArrayBuffer(size || this.size || 64);
        }};
        GPUBuffer.prototype.unmap = function() {{ this.mapState = 'unmapped'; }};
        GPUBuffer.prototype.destroy = function() {{ this.mapState = 'destroyed'; }};

        function GPUTextureView(descriptor) {{
            this.descriptor = descriptor || {{}};
        }}
        globalThis.GPUTextureView = GPUTextureView;

        function GPUTexture(descriptor) {{
            this.width = (descriptor && descriptor.size && descriptor.size.width) || 1;
            this.height = (descriptor && descriptor.size && descriptor.size.height) || 1;
            this.format = (descriptor && descriptor.format) || 'rgba8unorm';
        }}
        GPUTexture.prototype.createView = function(descriptor) {{ return new GPUTextureView(descriptor); }};
        GPUTexture.prototype.destroy = function() {{}};

        function GPUQueue() {{}}
        GPUQueue.prototype.submit = function(commandBuffers) {{}};
        GPUQueue.prototype.writeBuffer = function(buffer, bufferOffset, data, dataOffset, size) {{}};
        GPUQueue.prototype.writeTexture = function(destination, data, dataLayout, size) {{}};

        function GPUCommandBuffer() {{}}
        function GPURenderPassEncoder() {{}}
        GPURenderPassEncoder.prototype.setPipeline = function(pipeline) {{}};
        GPURenderPassEncoder.prototype.setVertexBuffer = function(slot, buffer) {{}};
        GPURenderPassEncoder.prototype.setIndexBuffer = function(buffer, indexFormat, offset, size) {{}};
        GPURenderPassEncoder.prototype.setBindGroup = function(index, bindGroup) {{}};
        GPURenderPassEncoder.prototype.draw = function(vertexCount, instanceCount, firstVertex, firstInstance) {{}};
        GPURenderPassEncoder.prototype.drawIndexed = function(indexCount, instanceCount, firstIndex, baseVertex, firstInstance) {{}};
        GPURenderPassEncoder.prototype.end = function() {{}};

        function GPUComputePassEncoder() {{}}
        GPUComputePassEncoder.prototype.setPipeline = function(pipeline) {{}};
        GPUComputePassEncoder.prototype.setBindGroup = function(index, bindGroup) {{}};
        GPUComputePassEncoder.prototype.dispatchWorkgroups = function(workgroupCountX, workgroupCountY, workgroupCountZ) {{}};
        GPUComputePassEncoder.prototype.end = function() {{}};

        function GPUCommandEncoder() {{}}
        GPUCommandEncoder.prototype.beginRenderPass = function(descriptor) {{ return new GPURenderPassEncoder(); }};
        GPUCommandEncoder.prototype.beginComputePass = function(descriptor) {{ return new GPUComputePassEncoder(); }};
        GPUCommandEncoder.prototype.finish = function() {{ return new GPUCommandBuffer(); }};
        GPUCommandEncoder.prototype.copyBufferToBuffer = function(source, sourceOffset, destination, destinationOffset, size) {{}};
        GPUCommandEncoder.prototype.copyBufferToTexture = function(source, destination, copySize) {{}};
        GPUCommandEncoder.prototype.copyTextureToBuffer = function(source, destination, copySize) {{}};
        GPUCommandEncoder.prototype.copyTextureToTexture = function(source, destination, copySize) {{}};

        function GPUShaderModule(descriptor) {{
            this.code = (descriptor && descriptor.code) || '';
        }}
        GPUShaderModule.prototype.getCompilationInfo = function() {{
            return Promise.resolve({{ messages: [] }});
        }};

        function GPUBindGroupLayout(descriptor) {{
            this.entries = (descriptor && descriptor.entries) || [];
        }}

        function GPUBindGroup(descriptor) {{
            this.layout = descriptor && descriptor.layout;
            this.entries = (descriptor && descriptor.entries) || [];
        }}

        function GPUPipelineLayout(descriptor) {{
            this.bindGroupLayouts = (descriptor && descriptor.bindGroupLayouts) || [];
        }}

        function GPURenderPipeline(descriptor) {{
            this.descriptor = descriptor;
            this.layout = descriptor && descriptor.layout;
        }}
        GPURenderPipeline.prototype.getBindGroupLayout = function(index) {{
            return new GPUBindGroupLayout({{}});
        }};

        function GPUComputePipeline(descriptor) {{
            this.descriptor = descriptor;
            this.layout = descriptor && descriptor.layout;
        }}
        GPUComputePipeline.prototype.getBindGroupLayout = function(index) {{
            return new GPUBindGroupLayout({{}});
        }};

        function GPUDevice(adapter) {{
            this.adapter = adapter;
            this.queue = new GPUQueue();
            this.features = new Set();
            this.limits = {{
                maxTextureDimension2D: 8192,
                maxBufferSize: 268435456,
                maxStorageBufferBindingSize: 134217728,
                maxComputeWorkgroupSizeX: 256,
                maxComputeWorkgroupSizeY: 256,
                maxComputeWorkgroupSizeZ: 64
            }};
        }}
        GPUDevice.prototype.createBuffer = function(descriptor) {{ return new GPUBuffer(descriptor); }};
        GPUDevice.prototype.createTexture = function(descriptor) {{ return new GPUTexture(descriptor); }};
        GPUDevice.prototype.createSampler = function(descriptor) {{ return {{}}; }};
        GPUDevice.prototype.createShaderModule = function(descriptor) {{ return new GPUShaderModule(descriptor); }};
        GPUDevice.prototype.createBindGroupLayout = function(descriptor) {{ return new GPUBindGroupLayout(descriptor); }};
        GPUDevice.prototype.createBindGroup = function(descriptor) {{ return new GPUBindGroup(descriptor); }};
        GPUDevice.prototype.createPipelineLayout = function(descriptor) {{ return new GPUPipelineLayout(descriptor); }};
        GPUDevice.prototype.createRenderPipeline = function(descriptor) {{ return new GPURenderPipeline(descriptor); }};
        GPUDevice.prototype.createComputePipeline = function(descriptor) {{ return new GPUComputePipeline(descriptor); }};
        GPUDevice.prototype.createRenderPipelineAsync = function(descriptor) {{ return Promise.resolve(new GPURenderPipeline(descriptor)); }};
        GPUDevice.prototype.createComputePipelineAsync = function(descriptor) {{ return Promise.resolve(new GPUComputePipeline(descriptor)); }};
        GPUDevice.prototype.createCommandEncoder = function(descriptor) {{ return new GPUCommandEncoder(); }};
        GPUDevice.prototype.destroy = function() {{}};

        function GPUAdapter() {{
            this.features = new Set();
            this.limits = {{ maxTextureDimension2D: 8192, maxBufferSize: 268435456 }};
            this.isFallbackAdapter = false;
        }}
        GPUAdapter.prototype.requestDevice = function(descriptor) {{
            return Promise.resolve(new GPUDevice(this));
        }};

        function GPU() {{}}
        GPU.prototype.requestAdapter = function(options) {{
            return Promise.resolve(new GPUAdapter());
        }};
        GPU.prototype.getPreferredCanvasFormat = function() {{
            return 'bgra8unorm';
        }};

        function GPUCanvasContext(canvas) {{
            this.canvas = canvas;
            this._configured = false;
        }}
        GPUCanvasContext.prototype.configure = function(config) {{
            this._config = config;
            this._configured = true;
        }};
        GPUCanvasContext.prototype.unconfigure = function() {{
            this._configured = false;
        }};
        GPUCanvasContext.prototype.getCurrentTexture = function() {{
            var w = this.canvas ? (this.canvas.width || 300) : 300;
            var h = this.canvas ? (this.canvas.height || 150) : 150;
            return new GPUTexture({{ size: {{ width: w, height: h }}, format: (this._config && this._config.format) || 'bgra8unorm' }});
        }};

        globalThis.GPU = GPU;
        globalThis.GPUAdapter = GPUAdapter;
        globalThis.GPUDevice = GPUDevice;
        globalThis.GPUQueue = GPUQueue;
        globalThis.GPUBuffer = GPUBuffer;
        globalThis.GPUTexture = GPUTexture;
        globalThis.GPUTextureView = GPUTextureView;
        globalThis.GPUCommandEncoder = GPUCommandEncoder;
        globalThis.GPURenderPassEncoder = GPURenderPassEncoder;
        globalThis.GPUComputePassEncoder = GPUComputePassEncoder;
        globalThis.GPUCommandBuffer = GPUCommandBuffer;
        globalThis.GPUShaderModule = GPUShaderModule;
        globalThis.GPUBindGroupLayout = GPUBindGroupLayout;
        globalThis.GPUBindGroup = GPUBindGroup;
        globalThis.GPUPipelineLayout = GPUPipelineLayout;
        globalThis.GPURenderPipeline = GPURenderPipeline;
        globalThis.GPUComputePipeline = GPUComputePipeline;
        globalThis.GPUCanvasContext = GPUCanvasContext;

        if (typeof navigator !== 'undefined') {{
            navigator.gpu = new GPU();
        }}
    }})();

    // ── Web Audio API ──────────────────────────────────────────────────────────
    (function() {{
        function AudioNode(context) {{
            this.context = context;
            this.numberOfInputs = 1;
            this.numberOfOutputs = 1;
            this.channelCount = 2;
            this._connections = [];
        }}
        AudioNode.prototype.connect = function(destination) {{
            this._connections.push(destination);
            return destination;
        }};
        AudioNode.prototype.disconnect = function() {{
            this._connections = [];
        }};

        function AudioParam(defaultValue) {{
            this.value = defaultValue;
            this.defaultValue = defaultValue;
            this.minValue = -3.4028235e38;
            this.maxValue = 3.4028235e38;
        }}
        AudioParam.prototype.setValueAtTime = function(val, startTime) {{
            this.value = val;
            return this;
        }};
        AudioParam.prototype.linearRampToValueAtTime = function(val, endTime) {{
            this.value = val;
            return this;
        }};
        AudioParam.prototype.exponentialRampToValueAtTime = function(val, endTime) {{
            this.value = val;
            return this;
        }};
        AudioParam.prototype.setTargetAtTime = function(target, startTime, timeConstant) {{
            this.value = target;
            return this;
        }};
        AudioParam.prototype.setValueCurveAtTime = function(values, startTime, duration) {{
            if (values && values.length > 0) {{
                this.value = values[values.length - 1];
            }}
            return this;
        }};
        AudioParam.prototype.cancelScheduledValues = function(startTime) {{
            return this;
        }};
        AudioParam.prototype.cancelAndHoldAtTime = function(cancelTime) {{
            return this;
        }};

        function GainNode(context) {{
            AudioNode.call(this, context);
            this.gain = new AudioParam(1.0);
        }}
        GainNode.prototype = Object.create(AudioNode.prototype);

        function OscillatorNode(context) {{
            AudioNode.call(this, context);
            this.type = 'sine';
            this.frequency = new AudioParam(440);
            this.detune = new AudioParam(0);
            this._started = false;
            this._stopped = false;
        }}
        OscillatorNode.prototype = Object.create(AudioNode.prototype);
        OscillatorNode.prototype.start = function(when) {{
            this._started = true;
        }};
        OscillatorNode.prototype.stop = function(when) {{
            this._stopped = true;
        }};

        function AudioBuffer(options) {{
            this.numberOfChannels = (options && options.numberOfChannels) || 2;
            this.length = (options && options.length) || 0;
            this.sampleRate = (options && options.sampleRate) || 44100;
            this.duration = this.sampleRate > 0 ? (this.length / this.sampleRate) : 0;
            this._channelData = [];
            for (var i = 0; i < this.numberOfChannels; i++) {{
                this._channelData.push(new Float32Array(this.length));
            }}
        }}
        AudioBuffer.prototype.getChannelData = function(channel) {{
            if (channel < 0 || channel >= this._channelData.length) {{
                throw new Error("IndexSizeError: channel out of bounds");
            }}
            return this._channelData[channel];
        }};
        AudioBuffer.prototype.copyFromChannel = function(dest, channelNumber, startInChannel) {{
            var src = this.getChannelData(channelNumber);
            var offset = startInChannel || 0;
            dest.set(src.subarray(offset, offset + dest.length));
        }};
        AudioBuffer.prototype.copyToChannel = function(src, channelNumber, startInChannel) {{
            var dest = this.getChannelData(channelNumber);
            var offset = startInChannel || 0;
            dest.set(src, offset);
        }};

        function AudioBufferSourceNode(context) {{
            AudioNode.call(this, context);
            this.buffer = null;
            this.playbackRate = new AudioParam(1.0);
            this.detune = new AudioParam(0);
            this.loop = false;
            this.loopStart = 0;
            this.loopEnd = 0;
            this._started = false;
            this._stopped = false;
        }}
        AudioBufferSourceNode.prototype = Object.create(AudioNode.prototype);
        AudioBufferSourceNode.prototype.start = function() {{ this._started = true; }};
        AudioBufferSourceNode.prototype.stop = function() {{ this._stopped = true; }};

        function AnalyserNode(context) {{
            AudioNode.call(this, context);
            this.fftSize = 2048;
            this.frequencyBinCount = 1024;
            this.minDecibels = -100;
            this.maxDecibels = -30;
            this.smoothingTimeConstant = 0.8;
        }}
        AnalyserNode.prototype = Object.create(AudioNode.prototype);
        AnalyserNode.prototype.getByteFrequencyData = function(array) {{
            array.fill(0);
        }};
        AnalyserNode.prototype.getByteTimeDomainData = function(array) {{
            array.fill(128);
        }};
        AnalyserNode.prototype.getFloatFrequencyData = function(array) {{
            array.fill(-100);
        }};
        AnalyserNode.prototype.getFloatTimeDomainData = function(array) {{
            array.fill(0);
        }};

        function BiquadFilterNode(context) {{
            AudioNode.call(this, context);
            this.type = 'lowpass';
            this.frequency = new AudioParam(350);
            this.detune = new AudioParam(0);
            this.Q = new AudioParam(1);
            this.gain = new AudioParam(0);
        }}
        BiquadFilterNode.prototype = Object.create(AudioNode.prototype);

        function PannerNode(context, options) {{
            AudioNode.call(this, context);
            options = options || {{}};
            this.panningModel = options.panningModel || 'equalpower';
            this.distanceModel = options.distanceModel || 'inverse';
            this.refDistance = options.refDistance !== undefined ? options.refDistance : 1;
            this.maxDistance = options.maxDistance !== undefined ? options.maxDistance : 10000;
            this.rolloffFactor = options.rolloffFactor !== undefined ? options.rolloffFactor : 1;
            this.coneInnerAngle = options.coneInnerAngle !== undefined ? options.coneInnerAngle : 360;
            this.coneOuterAngle = options.coneOuterAngle !== undefined ? options.coneOuterAngle : 360;
            this.coneOuterGain = options.coneOuterGain !== undefined ? options.coneOuterGain : 0;
            this.positionX = new AudioParam(options.positionX !== undefined ? options.positionX : 0);
            this.positionY = new AudioParam(options.positionY !== undefined ? options.positionY : 0);
            this.positionZ = new AudioParam(options.positionZ !== undefined ? options.positionZ : 0);
            this.orientationX = new AudioParam(options.orientationX !== undefined ? options.orientationX : 1);
            this.orientationY = new AudioParam(options.orientationY !== undefined ? options.orientationY : 0);
            this.orientationZ = new AudioParam(options.orientationZ !== undefined ? options.orientationZ : 0);
        }}
        PannerNode.prototype = Object.create(AudioNode.prototype);
        PannerNode.prototype.setPosition = function(x, y, z) {{
            this.positionX.value = x;
            this.positionY.value = y;
            this.positionZ.value = z;
        }};
        PannerNode.prototype.setOrientation = function(x, y, z) {{
            this.orientationX.value = x;
            this.orientationY.value = y;
            this.orientationZ.value = z;
        }};

        function StereoPannerNode(context, options) {{
            AudioNode.call(this, context);
            options = options || {{}};
            this.pan = new AudioParam(options.pan !== undefined ? options.pan : 0);
        }}
        StereoPannerNode.prototype = Object.create(AudioNode.prototype);

        function ConstantSourceNode(context, options) {{
            AudioNode.call(this, context);
            options = options || {{}};
            this.offset = new AudioParam(options.offset !== undefined ? options.offset : 1);
            this._started = false;
            this._stopped = false;
        }}
        ConstantSourceNode.prototype = Object.create(AudioNode.prototype);
        ConstantSourceNode.prototype.start = function() {{ this._started = true; }};
        ConstantSourceNode.prototype.stop = function() {{ this._stopped = true; }};

        function AudioListener() {{
            this.positionX = new AudioParam(0);
            this.positionY = new AudioParam(0);
            this.positionZ = new AudioParam(0);
            this.forwardX = new AudioParam(0);
            this.forwardY = new AudioParam(0);
            this.forwardZ = new AudioParam(-1);
            this.upX = new AudioParam(0);
            this.upY = new AudioParam(1);
            this.upZ = new AudioParam(0);
        }}
        AudioListener.prototype.setPosition = function(x, y, z) {{
            this.positionX.value = x;
            this.positionY.value = y;
            this.positionZ.value = z;
        }};
        AudioListener.prototype.setOrientation = function(x, y, z, xUp, yUp, zUp) {{
            this.forwardX.value = x;
            this.forwardY.value = y;
            this.forwardZ.value = z;
            if (xUp !== undefined) this.upX.value = xUp;
            if (yUp !== undefined) this.upY.value = yUp;
            if (zUp !== undefined) this.upZ.value = zUp;
        }};

        function AudioWorklet() {{
            this._modules = [];
        }}
        AudioWorklet.prototype.addModule = function(moduleURL, options) {{
            this._modules.push(moduleURL);
            return Promise.resolve();
        }};

        function AudioWorkletNode(context, name, options) {{
            AudioNode.call(this, context);
            this.nodeName = name;
            this.parameters = new Map();
            if (options && options.parameterData) {{
                for (var k in options.parameterData) {{
                    this.parameters.set(k, new AudioParam(options.parameterData[k]));
                }}
            }}
            var channel = new MessageChannel();
            this.port = channel.port1;
            this._port2 = channel.port2;
        }}
        AudioWorkletNode.prototype = Object.create(AudioNode.prototype);

        function AudioDestinationNode(context) {{
            AudioNode.call(this, context);
            this.maxChannelCount = 2;
        }}
        AudioDestinationNode.prototype = Object.create(AudioNode.prototype);

        function BaseAudioContext() {{
            this.state = 'running';
            this.sampleRate = 44100;
            this.currentTime = 0;
            this.destination = new AudioDestinationNode(this);
            this.listener = new AudioListener();
            this.audioWorklet = new AudioWorklet();
        }}
        BaseAudioContext.prototype.createGain = function() {{ return new GainNode(this); }};
        BaseAudioContext.prototype.createOscillator = function() {{ return new OscillatorNode(this); }};
        BaseAudioContext.prototype.createBufferSource = function() {{ return new AudioBufferSourceNode(this); }};
        BaseAudioContext.prototype.createBuffer = function(numChannels, length, sampleRate) {{
            return new AudioBuffer({{ numberOfChannels: numChannels, length: length, sampleRate: sampleRate }});
        }};
        BaseAudioContext.prototype.createAnalyser = function() {{ return new AnalyserNode(this); }};
        BaseAudioContext.prototype.createBiquadFilter = function() {{ return new BiquadFilterNode(this); }};
        BaseAudioContext.prototype.createPanner = function() {{ return new PannerNode(this); }};
        BaseAudioContext.prototype.createStereoPanner = function() {{ return new StereoPannerNode(this); }};
        BaseAudioContext.prototype.createConstantSource = function() {{ return new ConstantSourceNode(this); }};
        BaseAudioContext.prototype.decodeAudioData = function(audioData, successCb, errorCb) {{
            var self = this;
            return new Promise(function(resolve, reject) {{
                var buf = new AudioBuffer({{ numberOfChannels: 2, length: 1024, sampleRate: self.sampleRate }});
                if (typeof successCb === 'function') successCb(buf);
                resolve(buf);
            }});
        }};
        BaseAudioContext.prototype.resume = function() {{
            this.state = 'running';
            return Promise.resolve();
        }};
        BaseAudioContext.prototype.suspend = function() {{
            this.state = 'suspended';
            return Promise.resolve();
        }};
        BaseAudioContext.prototype.close = function() {{
            this.state = 'closed';
            return Promise.resolve();
        }};

        function AudioContext(options) {{
            BaseAudioContext.call(this);
            if (options && options.sampleRate) this.sampleRate = options.sampleRate;
        }}
        AudioContext.prototype = Object.create(BaseAudioContext.prototype);

        globalThis.AudioNode = AudioNode;
        globalThis.AudioParam = AudioParam;
        globalThis.GainNode = GainNode;
        globalThis.OscillatorNode = OscillatorNode;
        globalThis.AudioBuffer = AudioBuffer;
        globalThis.AudioBufferSourceNode = AudioBufferSourceNode;
        globalThis.AnalyserNode = AnalyserNode;
        globalThis.BiquadFilterNode = BiquadFilterNode;
        globalThis.PannerNode = PannerNode;
        globalThis.StereoPannerNode = StereoPannerNode;
        globalThis.ConstantSourceNode = ConstantSourceNode;
        globalThis.AudioListener = AudioListener;
        globalThis.AudioWorklet = AudioWorklet;
        globalThis.AudioWorkletNode = AudioWorkletNode;
        globalThis.AudioDestinationNode = AudioDestinationNode;
        globalThis.BaseAudioContext = BaseAudioContext;
        globalThis.AudioContext = AudioContext;
        globalThis.webkitAudioContext = AudioContext;
    }})();

    // ── OffscreenCanvas ────────────────────────────────────────────────────────
    (function() {{
        if (typeof globalThis.OffscreenCanvas !== 'undefined') return;
        function OffscreenCanvas(width, height) {{
            this.width = Math.max(0, parseInt(width) || 300);
            this.height = Math.max(0, parseInt(height) || 150);
            this._context2d = null;
        }}
        OffscreenCanvas.prototype.getContext = function(contextId, options) {{
            if (contextId === '2d') {{
                if (!this._context2d) {{
                    if (typeof _createCanvasRenderingContext2D === 'function') {{
                        this._context2d = _createCanvasRenderingContext2D(this);
                    }} else {{
                        var self = this;
                        this._context2d = {{
                            canvas: self,
                            fillStyle: '#000000',
                            strokeStyle: '#000000',
                            lineWidth: 1,
                            fillRect: function() {{}},
                            strokeRect: function() {{}},
                            clearRect: function() {{}},
                            beginPath: function() {{}},
                            closePath: function() {{}},
                            moveTo: function() {{}},
                            lineTo: function() {{}},
                            arc: function() {{}},
                            fill: function() {{}},
                            stroke: function() {{}},
                            fillText: function() {{}},
                            strokeText: function() {{}},
                            measureText: function(txt) {{ return {{ width: (txt || '').length * 8 }}; }},
                            drawImage: function() {{}},
                            getImageData: function(sx, sy, sw, sh) {{
                                return {{ width: sw, height: sh, data: new Uint8ClampedArray(sw * sh * 4) }};
                            }},
                            putImageData: function() {{}},
                            save: function() {{}},
                            restore: function() {{}},
                            scale: function() {{}},
                            rotate: function() {{}},
                            translate: function() {{}},
                            transform: function() {{}},
                            setTransform: function() {{}},
                            resetTransform: function() {{}}
                        }};
                    }}
                }}
            }} else if (contextId === 'webgl') {{
                if (typeof WebGLRenderingContext === 'function') {{
                    return new WebGLRenderingContext(this);
                }}
            }} else if (contextId === 'webgl2') {{
                if (typeof WebGL2RenderingContext === 'function') {{
                    return new WebGL2RenderingContext(this);
                }}
            }} else if (contextId === 'webgpu') {{
                if (typeof GPUCanvasContext === 'function') {{
                    return new GPUCanvasContext(this);
                }}
            }}
            return null;
        }};
        OffscreenCanvas.prototype.convertToBlob = function(options) {{
            var type = (options && options.type) || 'image/png';
            return Promise.resolve(new Blob([], {{ type: type }}));
        }};
        OffscreenCanvas.prototype.transferToImageBitmap = function() {{
            return {{
                width: this.width,
                height: this.height,
                close: function() {{}}
            }};
        }};
        globalThis.OffscreenCanvas = OffscreenCanvas;

        if (typeof HTMLCanvasElement !== 'undefined') {{
            HTMLCanvasElement.prototype.transferControlToOffscreen = function() {{
                var off = new OffscreenCanvas(this.width || 300, this.height || 150);
                this._transferred = true;
                return off;
            }};
        }}
    }})();

    // ── WebCodecs ───────────────────────────────────────────────────
    (function() {{
        // VideoFrame
        if (typeof globalThis.VideoFrame === 'undefined') {{
            function VideoFrame(source, init) {{
                if (!source) throw new TypeError("VideoFrame constructor requires a source");
                this._closed = false;
                this._colorSpace = (init && init.colorSpace) || {{
                    primaries: 'bt709',
                    transfer: 'iec61966-2-1',
                    matrix: 'rgb',
                    fullRange: true
                }};

                if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {{
                    if (!init || typeof init.codedWidth !== 'number' || typeof init.codedHeight !== 'number' || !init.format) {{
                        throw new TypeError("VideoFrame from buffer requires format, codedWidth, and codedHeight");
                    }}
                    this._format = init.format;
                    this._codedWidth = init.codedWidth;
                    this._codedHeight = init.codedHeight;
                    this._timestamp = typeof init.timestamp === 'number' ? init.timestamp : 0;
                    this._duration = typeof init.duration === 'number' ? init.duration : null;

                    var bpp = (this._format === 'RGBA' || this._format === 'BGRA') ? 4 : 1.5;
                    var minSize = Math.floor(this._codedWidth * this._codedHeight * bpp);
                    var u8 = source instanceof ArrayBuffer ? new Uint8Array(source) : new Uint8Array(source.buffer, source.byteOffset, source.byteLength);
                    this._buffer = new Uint8Array(minSize);
                    this._buffer.set(u8.subarray(0, Math.min(u8.length, minSize)));
                }} else if (source instanceof VideoFrame) {{
                    if (source._closed) throw new TypeError("Cannot clone a closed VideoFrame");
                    this._format = source._format;
                    this._codedWidth = source._codedWidth;
                    this._codedHeight = source._codedHeight;
                    this._timestamp = (init && typeof init.timestamp === 'number') ? init.timestamp : source._timestamp;
                    this._duration = (init && typeof init.duration === 'number') ? init.duration : source._duration;
                    this._buffer = new Uint8Array(source._buffer);
                }} else {{
                    var width = (source && source.width) || (init && init.codedWidth) || 300;
                    var height = (source && source.height) || (init && init.codedHeight) || 150;
                    this._format = (init && init.format) || 'RGBA';
                    this._codedWidth = width;
                    this._codedHeight = height;
                    this._timestamp = (init && typeof init.timestamp === 'number') ? init.timestamp : 0;
                    this._duration = (init && typeof init.duration === 'number') ? init.duration : null;
                    var sz = width * height * 4;
                    this._buffer = new Uint8Array(sz);
                    if (source && typeof source.getContext === 'function') {{
                        try {{
                            var ctx = source.getContext('2d');
                            if (ctx && typeof ctx.getImageData === 'function') {{
                                var imgData = ctx.getImageData(0, 0, width, height);
                                if (imgData && imgData.data) {{
                                    this._buffer.set(imgData.data);
                                }}
                            }}
                        }} catch(e) {{}}
                    }}
                }}

                var vr = (init && init.visibleRect) || {{ x: 0, y: 0, width: this._codedWidth, height: this._codedHeight }};
                this._visibleRect = {{ x: vr.x || 0, y: vr.y || 0, width: vr.width || this._codedWidth, height: vr.height || this._codedHeight }};
                this._displayWidth = (init && init.displayWidth) || this._visibleRect.width;
                this._displayHeight = (init && init.displayHeight) || this._visibleRect.height;
            }}

            Object.defineProperty(VideoFrame.prototype, 'format', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._format; }} }});
            Object.defineProperty(VideoFrame.prototype, 'codedWidth', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._codedWidth; }} }});
            Object.defineProperty(VideoFrame.prototype, 'codedHeight', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._codedHeight; }} }});
            Object.defineProperty(VideoFrame.prototype, 'codedRect', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return {{ x: 0, y: 0, width: this._codedWidth, height: this._codedHeight }}; }} }});
            Object.defineProperty(VideoFrame.prototype, 'visibleRect', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return {{ x: this._visibleRect.x, y: this._visibleRect.y, width: this._visibleRect.width, height: this._visibleRect.height }}; }} }});
            Object.defineProperty(VideoFrame.prototype, 'displayWidth', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._displayWidth; }} }});
            Object.defineProperty(VideoFrame.prototype, 'displayHeight', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._displayHeight; }} }});
            Object.defineProperty(VideoFrame.prototype, 'duration', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._duration; }} }});
            Object.defineProperty(VideoFrame.prototype, 'timestamp', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._timestamp; }} }});
            Object.defineProperty(VideoFrame.prototype, 'colorSpace', {{ get: function() {{ if (this._closed) throw new TypeError("VideoFrame is closed"); return this._colorSpace; }} }});

            VideoFrame.prototype.allocationSize = function(options) {{
                if (this._closed) throw new TypeError("VideoFrame is closed");
                var rect = (options && options.rect) || this._visibleRect;
                var bpp = (this._format === 'RGBA' || this._format === 'BGRA') ? 4 : 1.5;
                return Math.floor(rect.width * rect.height * bpp);
            }};

            VideoFrame.prototype.copyTo = function(destination, options) {{
                if (this._closed) throw new TypeError("VideoFrame is closed");
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._buffer.length);
                destBuf.set(this._buffer.subarray(0, toCopy));
                var stride = this._codedWidth * ((this._format === 'RGBA' || this._format === 'BGRA') ? 4 : 1);
                return Promise.resolve([{{ offset: 0, stride: stride }}]);
            }};

            VideoFrame.prototype.clone = function() {{
                if (this._closed) throw new TypeError("VideoFrame is closed");
                return new VideoFrame(this);
            }};

            VideoFrame.prototype.close = function() {{
                this._closed = true;
                this._buffer = null;
            }};
            globalThis.VideoFrame = VideoFrame;
        }}

        // AudioData
        if (typeof globalThis.AudioData === 'undefined') {{
            function AudioData(init) {{
                if (!init || !init.format || typeof init.sampleRate !== 'number' || typeof init.numberOfFrames !== 'number' || typeof init.numberOfChannels !== 'number' || !init.data) {{
                    throw new TypeError("AudioData constructor requires format, sampleRate, numberOfFrames, numberOfChannels, and data");
                }}
                this._closed = false;
                this._format = init.format;
                this._sampleRate = init.sampleRate;
                this._numberOfFrames = init.numberOfFrames;
                this._numberOfChannels = init.numberOfChannels;
                this._timestamp = typeof init.timestamp === 'number' ? init.timestamp : 0;
                this._duration = Math.round((this._numberOfFrames / this._sampleRate) * 1000000);

                var bytesPerSample = (this._format === 'f32' || this._format === 'f32-planar' || this._format === 's32' || this._format === 's32-planar') ? 4 : (this._format === 's16' || this._format === 's16-planar' ? 2 : 1);
                var totalBytes = this._numberOfFrames * this._numberOfChannels * bytesPerSample;
                var u8 = init.data instanceof ArrayBuffer ? new Uint8Array(init.data) : new Uint8Array(init.data.buffer, init.data.byteOffset, init.data.byteLength);
                this._buffer = new Uint8Array(totalBytes);
                this._buffer.set(u8.subarray(0, Math.min(u8.length, totalBytes)));
                this._bytesPerSample = bytesPerSample;
            }}

            Object.defineProperty(AudioData.prototype, 'format', {{ get: function() {{ if (this._closed) throw new TypeError("AudioData is closed"); return this._format; }} }});
            Object.defineProperty(AudioData.prototype, 'sampleRate', {{ get: function() {{ if (this._closed) throw new TypeError("AudioData is closed"); return this._sampleRate; }} }});
            Object.defineProperty(AudioData.prototype, 'numberOfFrames', {{ get: function() {{ if (this._closed) throw new TypeError("AudioData is closed"); return this._numberOfFrames; }} }});
            Object.defineProperty(AudioData.prototype, 'numberOfChannels', {{ get: function() {{ if (this._closed) throw new TypeError("AudioData is closed"); return this._numberOfChannels; }} }});
            Object.defineProperty(AudioData.prototype, 'duration', {{ get: function() {{ if (this._closed) throw new TypeError("AudioData is closed"); return this._duration; }} }});
            Object.defineProperty(AudioData.prototype, 'timestamp', {{ get: function() {{ if (this._closed) throw new TypeError("AudioData is closed"); return this._timestamp; }} }});

            AudioData.prototype.allocationSize = function(options) {{
                if (this._closed) throw new TypeError("AudioData is closed");
                var planeIndex = (options && typeof options.planeIndex === 'number') ? options.planeIndex : null;
                var frameCount = (options && typeof options.frameCount === 'number') ? options.frameCount : this._numberOfFrames;
                if (planeIndex !== null) {{
                    return frameCount * this._bytesPerSample;
                }}
                return frameCount * this._numberOfChannels * this._bytesPerSample;
            }};

            AudioData.prototype.copyTo = function(destination, options) {{
                if (this._closed) throw new TypeError("AudioData is closed");
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._buffer.length);
                destBuf.set(this._buffer.subarray(0, toCopy));
            }};

            AudioData.prototype.clone = function() {{
                if (this._closed) throw new TypeError("AudioData is closed");
                return new AudioData({{
                    format: this._format,
                    sampleRate: this._sampleRate,
                    numberOfFrames: this._numberOfFrames,
                    numberOfChannels: this._numberOfChannels,
                    timestamp: this._timestamp,
                    data: this._buffer.slice()
                }});
            }};

            AudioData.prototype.close = function() {{
                this._closed = true;
                this._buffer = null;
            }};
            globalThis.AudioData = AudioData;
        }}

        // EncodedVideoChunk & EncodedAudioChunk
        if (typeof globalThis.EncodedVideoChunk === 'undefined') {{
            function EncodedVideoChunk(init) {{
                if (!init || !init.type || typeof init.timestamp !== 'number' || !init.data) {{
                    throw new TypeError("EncodedVideoChunk requires type, timestamp, and data");
                }}
                this.type = init.type;
                this.timestamp = init.timestamp;
                this.duration = typeof init.duration === 'number' ? init.duration : null;
                var u8 = init.data instanceof ArrayBuffer ? new Uint8Array(init.data) : new Uint8Array(init.data.buffer, init.data.byteOffset, init.data.byteLength);
                this._data = new Uint8Array(u8);
                this.byteLength = this._data.byteLength;
            }}
            EncodedVideoChunk.prototype.copyTo = function(destination) {{
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._data.byteLength);
                destBuf.set(this._data.subarray(0, toCopy));
            }};
            globalThis.EncodedVideoChunk = EncodedVideoChunk;
        }}

        if (typeof globalThis.EncodedAudioChunk === 'undefined') {{
            function EncodedAudioChunk(init) {{
                if (!init || !init.type || typeof init.timestamp !== 'number' || !init.data) {{
                    throw new TypeError("EncodedAudioChunk requires type, timestamp, and data");
                }}
                this.type = init.type;
                this.timestamp = init.timestamp;
                this.duration = typeof init.duration === 'number' ? init.duration : null;
                var u8 = init.data instanceof ArrayBuffer ? new Uint8Array(init.data) : new Uint8Array(init.data.buffer, init.data.byteOffset, init.data.byteLength);
                this._data = new Uint8Array(u8);
                this.byteLength = this._data.byteLength;
            }}
            EncodedAudioChunk.prototype.copyTo = function(destination) {{
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._data.byteLength);
                destBuf.set(this._data.subarray(0, toCopy));
            }};
            globalThis.EncodedAudioChunk = EncodedAudioChunk;
        }}

        // VideoDecoder
        if (typeof globalThis.VideoDecoder === 'undefined') {{
            function VideoDecoder(init) {{
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {{
                    throw new TypeError("VideoDecoder requires output and error callbacks");
                }}
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.decodeQueueSize = 0;
                this._config = null;
            }}
            VideoDecoder.isConfigSupported = function(config) {{
                return Promise.resolve({{
                    supported: !!(config && config.codec),
                    config: config
                }});
            }};
            VideoDecoder.prototype.configure = function(config) {{
                if (this.state === 'closed') throw new TypeError("VideoDecoder is closed");
                if (!config || !config.codec) throw new TypeError("VideoDecoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            }};
            VideoDecoder.prototype.decode = function(chunk) {{
                if (this.state !== 'configured') throw new TypeError("VideoDecoder must be configured before decode");
                if (!(chunk instanceof EncodedVideoChunk)) throw new TypeError("decode expects an EncodedVideoChunk");
                this.decodeQueueSize++;
                var self = this;
                Promise.resolve().then(function() {{
                    if (self.state === 'closed') return;
                    self.decodeQueueSize = Math.max(0, self.decodeQueueSize - 1);
                    var width = (self._config && self._config.codedWidth) || 640;
                    var height = (self._config && self._config.codedHeight) || 480;
                    var frame = new VideoFrame(new Uint8Array(width * height * 4), {{
                        format: 'RGBA',
                        codedWidth: width,
                        codedHeight: height,
                        timestamp: chunk.timestamp,
                        duration: chunk.duration || 33333
                    }});
                    try {{
                        self._output(frame);
                    }} catch(e) {{
                        self._error(e);
                    }}
                }});
            }};
            VideoDecoder.prototype.flush = function() {{
                if (this.state !== 'configured') return Promise.reject(new TypeError("VideoDecoder not configured"));
                var self = this;
                return new Promise(function(resolve) {{
                    Promise.resolve().then(function() {{
                        self.decodeQueueSize = 0;
                        resolve();
                    }});
                }});
            }};
            VideoDecoder.prototype.reset = function() {{
                if (this.state === 'closed') throw new TypeError("VideoDecoder is closed");
                this.decodeQueueSize = 0;
            }};
            VideoDecoder.prototype.close = function() {{
                this.state = 'closed';
                this.decodeQueueSize = 0;
            }};
            globalThis.VideoDecoder = VideoDecoder;
        }}

        // AudioDecoder
        if (typeof globalThis.AudioDecoder === 'undefined') {{
            function AudioDecoder(init) {{
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {{
                    throw new TypeError("AudioDecoder requires output and error callbacks");
                }}
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.decodeQueueSize = 0;
                this._config = null;
            }}
            AudioDecoder.isConfigSupported = function(config) {{
                return Promise.resolve({{
                    supported: !!(config && config.codec),
                    config: config
                }});
            }};
            AudioDecoder.prototype.configure = function(config) {{
                if (this.state === 'closed') throw new TypeError("AudioDecoder is closed");
                if (!config || !config.codec) throw new TypeError("AudioDecoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            }};
            AudioDecoder.prototype.decode = function(chunk) {{
                if (this.state !== 'configured') throw new TypeError("AudioDecoder must be configured before decode");
                if (!(chunk instanceof EncodedAudioChunk)) throw new TypeError("decode expects an EncodedAudioChunk");
                this.decodeQueueSize++;
                var self = this;
                Promise.resolve().then(function() {{
                    if (self.state === 'closed') return;
                    self.decodeQueueSize = Math.max(0, self.decodeQueueSize - 1);
                    var sampleRate = (self._config && self._config.sampleRate) || 48000;
                    var channels = (self._config && self._config.numberOfChannels) || 2;
                    var frames = 1024;
                    var data = new Float32Array(frames * channels);
                    var audioData = new AudioData({{
                        format: 'f32',
                        sampleRate: sampleRate,
                        numberOfFrames: frames,
                        numberOfChannels: channels,
                        timestamp: chunk.timestamp,
                        data: data.buffer
                    }});
                    try {{
                        self._output(audioData);
                    }} catch(e) {{
                        self._error(e);
                    }}
                }});
            }};
            AudioDecoder.prototype.flush = function() {{
                if (this.state !== 'configured') return Promise.reject(new TypeError("AudioDecoder not configured"));
                var self = this;
                return new Promise(function(resolve) {{
                    Promise.resolve().then(function() {{
                        self.decodeQueueSize = 0;
                        resolve();
                    }});
                }});
            }};
            AudioDecoder.prototype.reset = function() {{
                if (this.state === 'closed') throw new TypeError("AudioDecoder is closed");
                this.decodeQueueSize = 0;
            }};
            AudioDecoder.prototype.close = function() {{
                this.state = 'closed';
                this.decodeQueueSize = 0;
            }};
            globalThis.AudioDecoder = AudioDecoder;
        }}

        // VideoEncoder
        if (typeof globalThis.VideoEncoder === 'undefined') {{
            function VideoEncoder(init) {{
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {{
                    throw new TypeError("VideoEncoder requires output and error callbacks");
                }}
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.encodeQueueSize = 0;
                this._config = null;
            }}
            VideoEncoder.isConfigSupported = function(config) {{
                return Promise.resolve({{
                    supported: !!(config && config.codec),
                    config: config
                }});
            }};
            VideoEncoder.prototype.configure = function(config) {{
                if (this.state === 'closed') throw new TypeError("VideoEncoder is closed");
                if (!config || !config.codec) throw new TypeError("VideoEncoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            }};
            VideoEncoder.prototype.encode = function(frame, options) {{
                if (this.state !== 'configured') throw new TypeError("VideoEncoder must be configured before encode");
                if (!(frame instanceof VideoFrame)) throw new TypeError("encode expects a VideoFrame");
                this.encodeQueueSize++;
                var isKey = (options && options.keyFrame) || false;
                var self = this;
                var ts = frame.timestamp;
                var dur = frame.duration;
                Promise.resolve().then(function() {{
                    if (self.state === 'closed') return;
                    self.encodeQueueSize = Math.max(0, self.encodeQueueSize - 1);
                    var dummyBytes = new Uint8Array([0x00, 0x00, 0x00, 0x01, isKey ? 0x67 : 0x41]);
                    var chunk = new EncodedVideoChunk({{
                        type: isKey ? 'key' : 'delta',
                        timestamp: ts,
                        duration: dur,
                        data: dummyBytes.buffer
                    }});
                    try {{
                        self._output(chunk, {{ decoderConfig: self._config }});
                    }} catch(e) {{
                        self._error(e);
                    }}
                }});
            }};
            VideoEncoder.prototype.flush = function() {{
                if (this.state !== 'configured') return Promise.reject(new TypeError("VideoEncoder not configured"));
                var self = this;
                return new Promise(function(resolve) {{
                    Promise.resolve().then(function() {{
                        self.encodeQueueSize = 0;
                        resolve();
                    }});
                }});
            }};
            VideoEncoder.prototype.reset = function() {{
                if (this.state === 'closed') throw new TypeError("VideoEncoder is closed");
                this.encodeQueueSize = 0;
            }};
            VideoEncoder.prototype.close = function() {{
                this.state = 'closed';
                this.encodeQueueSize = 0;
            }};
            globalThis.VideoEncoder = VideoEncoder;
        }}

        // AudioEncoder
        if (typeof globalThis.AudioEncoder === 'undefined') {{
            function AudioEncoder(init) {{
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {{
                    throw new TypeError("AudioEncoder requires output and error callbacks");
                }}
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.encodeQueueSize = 0;
                this._config = null;
            }}
            AudioEncoder.isConfigSupported = function(config) {{
                return Promise.resolve({{
                    supported: !!(config && config.codec),
                    config: config
                }});
            }};
            AudioEncoder.prototype.configure = function(config) {{
                if (this.state === 'closed') throw new TypeError("AudioEncoder is closed");
                if (!config || !config.codec) throw new TypeError("AudioEncoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            }};
            AudioEncoder.prototype.encode = function(data) {{
                if (this.state !== 'configured') throw new TypeError("AudioEncoder must be configured before encode");
                if (!(data instanceof AudioData)) throw new TypeError("encode expects AudioData");
                this.encodeQueueSize++;
                var self = this;
                var ts = data.timestamp;
                var dur = data.duration;
                Promise.resolve().then(function() {{
                    if (self.state === 'closed') return;
                    self.encodeQueueSize = Math.max(0, self.encodeQueueSize - 1);
                    var dummyBytes = new Uint8Array([0xFF, 0xF1, 0x50, 0x80]);
                    var chunk = new EncodedAudioChunk({{
                        type: 'key',
                        timestamp: ts,
                        duration: dur,
                        data: dummyBytes.buffer
                    }});
                    try {{
                        self._output(chunk, {{ decoderConfig: self._config }});
                    }} catch(e) {{
                        self._error(e);
                    }}
                }});
            }};
            AudioEncoder.prototype.flush = function() {{
                if (this.state !== 'configured') return Promise.reject(new TypeError("AudioEncoder not configured"));
                var self = this;
                return new Promise(function(resolve) {{
                    Promise.resolve().then(function() {{
                        self.encodeQueueSize = 0;
                        resolve();
                    }});
                }});
            }};
            AudioEncoder.prototype.reset = function() {{
                if (this.state === 'closed') throw new TypeError("AudioEncoder is closed");
                this.encodeQueueSize = 0;
            }};
            AudioEncoder.prototype.close = function() {{
                this.state = 'closed';
                this.encodeQueueSize = 0;
            }};
            globalThis.AudioEncoder = AudioEncoder;
        }}
    }})();

    // ── Origin Private File System (OPFS) ──────────────────────────
    (function() {{
        var _opfsRoot = {{
            kind: 'directory',
            name: '',
            children: new Map(),
            parent: null
        }};

        function FileSystemHandle(kind, name) {{
            this.kind = kind;
            this.name = name;
        }}
        FileSystemHandle.prototype.isSameEntry = function(other) {{
            if (!other || !(other instanceof FileSystemHandle)) return Promise.resolve(false);
            return Promise.resolve(this._entry === other._entry);
        }};
        globalThis.FileSystemHandle = FileSystemHandle;

        function FileSystemWritableFileStream(entry, initialBuffer) {{
            var self = this;
            this._entry = entry;
            this._position = 0;
            this._buffer = initialBuffer ? new Uint8Array(initialBuffer) : new Uint8Array(0);
            this._closed = false;

            var sink = {{
                write: function(chunk) {{ return self.write(chunk); }},
                close: function() {{ return self.close(); }},
                abort: function(reason) {{ return self.abort(reason); }}
            }};
            if (typeof WritableStream === 'function') {{
                try {{
                    WritableStream.call(this, sink);
                }} catch(e) {{}}
            }}
        }}
        if (typeof WritableStream === 'function') {{
            FileSystemWritableFileStream.prototype = Object.create(WritableStream.prototype);
            FileSystemWritableFileStream.prototype.constructor = FileSystemWritableFileStream;
        }}
        FileSystemWritableFileStream.prototype.write = function(data) {{
            if (this._closed) return Promise.reject(new DOMException("Stream is closed", "InvalidStateError"));
            var bytes = null;
            var pos = this._position;
            if (typeof data === 'string') {{
                bytes = new TextEncoder().encode(data);
            }} else if (data instanceof ArrayBuffer) {{
                bytes = new Uint8Array(data);
            }} else if (ArrayBuffer.isView(data)) {{
                bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
            }} else if (data && typeof data === 'object') {{
                if (data.type === 'seek') {{
                    return this.seek(data.position);
                }} else if (data.type === 'truncate') {{
                    return this.truncate(data.size);
                }} else if (data.type === 'write') {{
                    if (typeof data.position === 'number') {{
                        pos = Math.max(0, data.position);
                    }}
                    if (typeof data.data === 'string') {{
                        bytes = new TextEncoder().encode(data.data);
                    }} else if (data.data instanceof ArrayBuffer) {{
                        bytes = new Uint8Array(data.data);
                    }} else if (ArrayBuffer.isView(data.data)) {{
                        bytes = new Uint8Array(data.data.buffer, data.data.byteOffset, data.data.byteLength);
                    }}
                }}
            }}
            if (bytes) {{
                var needed = pos + bytes.length;
                if (needed > this._buffer.length) {{
                    var bigger = new Uint8Array(needed);
                    bigger.set(this._buffer);
                    this._buffer = bigger;
                }}
                this._buffer.set(bytes, pos);
                this._position = pos + bytes.length;
            }}
            return Promise.resolve();
        }};
        FileSystemWritableFileStream.prototype.seek = function(position) {{
            if (this._closed) return Promise.reject(new DOMException("Stream is closed", "InvalidStateError"));
            this._position = Math.max(0, parseInt(position) || 0);
            return Promise.resolve();
        }};
        FileSystemWritableFileStream.prototype.truncate = function(size) {{
            if (this._closed) return Promise.reject(new DOMException("Stream is closed", "InvalidStateError"));
            var newSize = Math.max(0, parseInt(size) || 0);
            var nextBuf = new Uint8Array(newSize);
            nextBuf.set(this._buffer.subarray(0, Math.min(this._buffer.length, newSize)));
            this._buffer = nextBuf;
            if (this._position > newSize) {{
                this._position = newSize;
            }}
            return Promise.resolve();
        }};
        FileSystemWritableFileStream.prototype.close = function() {{
            if (this._closed) return Promise.resolve();
            this._closed = true;
            this._entry.data = this._buffer;
            this._entry.lastModified = Date.now();
            this._entry.locked = false;
            return Promise.resolve();
        }};
        FileSystemWritableFileStream.prototype.abort = function(reason) {{
            this._closed = true;
            this._entry.locked = false;
            return Promise.resolve();
        }};
        globalThis.FileSystemWritableFileStream = FileSystemWritableFileStream;

        function FileSystemSyncAccessHandle(entry) {{
            this._entry = entry;
            this._cursor = 0;
            this._closed = false;
        }}
        FileSystemSyncAccessHandle.prototype.read = function(buffer, options) {{
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            if (!buffer || !buffer.buffer) throw new TypeError("buffer must be an ArrayBufferView");
            var at = (options && typeof options.at === 'number') ? options.at : this._cursor;
            var fileData = this._entry.data;
            if (at >= fileData.length) return 0;
            var available = fileData.length - at;
            var toRead = Math.min(buffer.byteLength, available);
            var destU8 = new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
            destU8.set(fileData.subarray(at, at + toRead));
            if (!options || typeof options.at !== 'number') {{
                this._cursor += toRead;
            }}
            return toRead;
        }};
        FileSystemSyncAccessHandle.prototype.write = function(buffer, options) {{
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            if (!buffer || !buffer.buffer) throw new TypeError("buffer must be an ArrayBufferView");
            var at = (options && typeof options.at === 'number') ? options.at : this._cursor;
            var srcU8 = new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength);
            var needed = at + srcU8.length;
            if (needed > this._entry.data.length) {{
                var bigger = new Uint8Array(needed);
                bigger.set(this._entry.data);
                this._entry.data = bigger;
            }}
            this._entry.data.set(srcU8, at);
            this._entry.lastModified = Date.now();
            if (!options || typeof options.at !== 'number') {{
                this._cursor += srcU8.length;
            }}
            return srcU8.length;
        }};
        FileSystemSyncAccessHandle.prototype.truncate = function(newSize) {{
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            var sz = Math.max(0, parseInt(newSize) || 0);
            var nextBuf = new Uint8Array(sz);
            nextBuf.set(this._entry.data.subarray(0, Math.min(this._entry.data.length, sz)));
            this._entry.data = nextBuf;
            this._entry.lastModified = Date.now();
            if (this._cursor > sz) this._cursor = sz;
        }};
        FileSystemSyncAccessHandle.prototype.getSize = function() {{
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
            return this._entry.data.length;
        }};
        FileSystemSyncAccessHandle.prototype.flush = function() {{
            if (this._closed) throw new DOMException("AccessHandle is closed", "InvalidStateError");
        }};
        FileSystemSyncAccessHandle.prototype.close = function() {{
            if (this._closed) return;
            this._closed = true;
            this._entry.locked = false;
        }};
        globalThis.FileSystemSyncAccessHandle = FileSystemSyncAccessHandle;

        function FileSystemFileHandle(entry) {{
            FileSystemHandle.call(this, 'file', entry.name);
            this._entry = entry;
        }}
        FileSystemFileHandle.prototype = Object.create(FileSystemHandle.prototype);
        FileSystemFileHandle.prototype.constructor = FileSystemFileHandle;
        FileSystemFileHandle.prototype.getFile = function() {{
            var self = this;
            var u8 = self._entry.data ? self._entry.data.slice() : new Uint8Array(0);
            var file = new File([u8], self.name, {{
                type: 'application/octet-stream',
                lastModified: self._entry.lastModified || Date.now()
            }});
            return Promise.resolve(file);
        }};
        FileSystemFileHandle.prototype.createWritable = function(options) {{
            if (this._entry.locked) {{
                return Promise.reject(new DOMException("The requested file could not be accessed because it is locked.", "NoModificationAllowedError"));
            }}
            this._entry.locked = true;
            var keep = options && options.keepExistingData;
            var initBuf = keep ? this._entry.data.slice() : new Uint8Array(0);
            var stream = new FileSystemWritableFileStream(this._entry, initBuf);
            return Promise.resolve(stream);
        }};
        FileSystemFileHandle.prototype.createSyncAccessHandle = function() {{
            if (this._entry.locked) {{
                return Promise.reject(new DOMException("The requested file could not be accessed because it is locked.", "NoModificationAllowedError"));
            }}
            this._entry.locked = true;
            return Promise.resolve(new FileSystemSyncAccessHandle(this._entry));
        }};
        globalThis.FileSystemFileHandle = FileSystemFileHandle;

        function FileSystemDirectoryHandle(entry) {{
            FileSystemHandle.call(this, 'directory', entry.name);
            this._entry = entry;
        }}
        FileSystemDirectoryHandle.prototype = Object.create(FileSystemHandle.prototype);
        FileSystemDirectoryHandle.prototype.constructor = FileSystemDirectoryHandle;
        FileSystemDirectoryHandle.prototype.getFileHandle = function(name, options) {{
            if (!name || typeof name !== 'string' || name.indexOf('/') !== -1 || name === '.' || name === '..') {{
                return Promise.reject(new TypeError("Invalid file name: " + name));
            }}
            var create = !!(options && options.create);
            if (this._entry.children.has(name)) {{
                var child = this._entry.children.get(name);
                if (child.kind === 'directory') {{
                    return Promise.reject(new DOMException("The entry is a directory, not a file.", "TypeMismatchError"));
                }}
                return Promise.resolve(new FileSystemFileHandle(child));
            }}
            if (create) {{
                var newChild = {{
                    kind: 'file',
                    name: name,
                    data: new Uint8Array(0),
                    lastModified: Date.now(),
                    locked: false,
                    parent: this._entry
                }};
                this._entry.children.set(name, newChild);
                return Promise.resolve(new FileSystemFileHandle(newChild));
            }}
            return Promise.reject(new DOMException("A requested file or directory could not be found.", "NotFoundError"));
        }};
        FileSystemDirectoryHandle.prototype.getDirectoryHandle = function(name, options) {{
            if (!name || typeof name !== 'string' || name.indexOf('/') !== -1 || name === '.' || name === '..') {{
                return Promise.reject(new TypeError("Invalid directory name: " + name));
            }}
            var create = !!(options && options.create);
            if (this._entry.children.has(name)) {{
                var child = this._entry.children.get(name);
                if (child.kind === 'file') {{
                    return Promise.reject(new DOMException("The entry is a file, not a directory.", "TypeMismatchError"));
                }}
                return Promise.resolve(new FileSystemDirectoryHandle(child));
            }}
            if (create) {{
                var newDir = {{
                    kind: 'directory',
                    name: name,
                    children: new Map(),
                    parent: this._entry
                }};
                this._entry.children.set(name, newDir);
                return Promise.resolve(new FileSystemDirectoryHandle(newDir));
            }}
            return Promise.reject(new DOMException("A requested file or directory could not be found.", "NotFoundError"));
        }};
        FileSystemDirectoryHandle.prototype.removeEntry = function(name, options) {{
            if (!this._entry.children.has(name)) {{
                return Promise.reject(new DOMException("Entry not found: " + name, "NotFoundError"));
            }}
            var child = this._entry.children.get(name);
            var recursive = !!(options && options.recursive);
            if (child.kind === 'directory' && child.children.size > 0 && !recursive) {{
                return Promise.reject(new DOMException("The entry is a non-empty directory.", "InvalidModificationError"));
            }}
            this._entry.children.delete(name);
            return Promise.resolve();
        }};
        FileSystemDirectoryHandle.prototype.resolve = function(possibleDescendant) {{
            if (!possibleDescendant || !possibleDescendant._entry) return Promise.resolve(null);
            var curr = possibleDescendant._entry;
            var path = [];
            while (curr && curr !== this._entry) {{
                path.unshift(curr.name);
                curr = curr.parent;
            }}
            if (curr === this._entry) {{
                return Promise.resolve(path);
            }}
            return Promise.resolve(null);
        }};
        function makeAsyncIterator(entries) {{
            var index = 0;
            var iter = {{
                next: function() {{
                    if (index < entries.length) {{
                        return Promise.resolve({{ value: entries[index++], done: false }});
                    }}
                    return Promise.resolve({{ value: undefined, done: true }});
                }}
            }};
            iter[Symbol.asyncIterator] = function() {{ return this; }};
            return iter;
        }}
        FileSystemDirectoryHandle.prototype.entries = function() {{
            var list = [];
            this._entry.children.forEach(function(val, key) {{
                var handle = val.kind === 'directory' ? new FileSystemDirectoryHandle(val) : new FileSystemFileHandle(val);
                list.push([key, handle]);
            }});
            return makeAsyncIterator(list);
        }};
        FileSystemDirectoryHandle.prototype.keys = function() {{
            var list = [];
            this._entry.children.forEach(function(val, key) {{
                list.push(key);
            }});
            return makeAsyncIterator(list);
        }};
        FileSystemDirectoryHandle.prototype.values = function() {{
            var list = [];
            this._entry.children.forEach(function(val) {{
                var handle = val.kind === 'directory' ? new FileSystemDirectoryHandle(val) : new FileSystemFileHandle(val);
                list.push(handle);
            }});
            return makeAsyncIterator(list);
        }};
        FileSystemDirectoryHandle.prototype[Symbol.asyncIterator] = function() {{
            return this.entries();
        }};
        globalThis.FileSystemDirectoryHandle = FileSystemDirectoryHandle;

        if (globalThis.navigator) {{
            if (!globalThis.navigator.storage) {{
                globalThis.navigator.storage = {{
                    estimate: function() {{ return Promise.resolve({{ quota: 1000000000, usage: 0 }}); }},
                    persist: function() {{ return Promise.resolve(true); }}
                }};
            }}
            globalThis.navigator.storage.getDirectory = function() {{
                return Promise.resolve(new FileSystemDirectoryHandle(_opfsRoot));
            }};
        }}
    }})();

    // ── WebTransport (HTTP/3 & QUIC) ────────────────────────────────
    (function() {{
        function WebTransportError(message, options) {{
            var err = new Error(message || "WebTransport error");
            err.name = "WebTransportError";
            err.source = (options && options.source) || "session";
            err.streamErrorCode = (options && typeof options.streamErrorCode === 'number') ? options.streamErrorCode : null;
            Object.setPrototypeOf(err, WebTransportError.prototype);
            return err;
        }}
        WebTransportError.prototype = Object.create(Error.prototype);
        WebTransportError.prototype.constructor = WebTransportError;
        globalThis.WebTransportError = WebTransportError;

        function WebTransportBidirectionalStream(readable, writable) {{
            this.readable = readable;
            this.writable = writable;
        }}
        globalThis.WebTransportBidirectionalStream = WebTransportBidirectionalStream;

        function WebTransport(url, options) {{
            if (!url) throw new TypeError("WebTransport requires a URL string");
            var parsed;
            try {{
                parsed = new URL(url);
            }} catch(e) {{
                throw new DOMException("Invalid WebTransport URL: " + url, "SyntaxError");
            }}
            if (parsed.protocol !== 'https:' && parsed.protocol !== 'wts:') {{
                throw new DOMException("WebTransport URL scheme must be https or wts", "SyntaxError");
            }}
            if (parsed.hash && parsed.hash.length > 0) {{
                throw new DOMException("WebTransport URL must not contain a fragment identifier", "SyntaxError");
            }}

            this._url = parsed.href;
            this._options = options || {{}};
            this.reliability = (options && options.requireUnreliable) ? 'supports-unreliable' : 'supports-unreliable';
            this.congestionControl = (options && options.congestionControl) || 'default';
            this._closed = false;

            var self = this;
            var resolveReady, rejectReady;
            this.ready = new Promise(function(res, rej) {{
                resolveReady = res;
                rejectReady = rej;
            }});

            var resolveClosed, rejectClosed;
            this.closed = new Promise(function(res, rej) {{
                resolveClosed = res;
                rejectClosed = rej;
            }});

            this._resolveClosed = resolveClosed;
            this._rejectClosed = rejectClosed;

            Promise.resolve().then(function() {{
                if (!self._closed) {{
                    resolveReady();
                }}
            }});

            var resolveDraining;
            this.draining = new Promise(function(res) {{
                resolveDraining = res;
            }});
            this._resolveDraining = resolveDraining;

            this.datagrams = {{
                maxDatagramSize: 1232,
                incomingMaxAge: null,
                outgoingMaxAge: null,
                incomingHighWaterMark: 1,
                outgoingHighWaterMark: 1,
                readable: new ReadableStream({{
                    start: function(controller) {{
                        self._datagramController = controller;
                    }},
                    pull: function(controller) {{}},
                    cancel: function(reason) {{}}
                }}),
                writable: new WritableStream({{
                    write: function(chunk) {{
                        return Promise.resolve();
                    }},
                    close: function() {{
                        return Promise.resolve();
                    }},
                    abort: function(reason) {{
                        return Promise.resolve();
                    }}
                }})
            }};

            this.incomingBidirectionalStreams = new ReadableStream({{
                start: function(controller) {{
                    self._incomingBidiController = controller;
                }}
            }});
            this.incomingUnidirectionalStreams = new ReadableStream({{
                start: function(controller) {{
                    self._incomingUniController = controller;
                }}
            }});
        }}

        WebTransport.prototype.createBidirectionalStream = function(options) {{
            if (this._closed) return Promise.reject(new WebTransportError("Connection closed", {{ source: 'session' }}));
            var readCtrl;
            var readable = new ReadableStream({{
                start: function(controller) {{ readCtrl = controller; }}
            }});
            var writable = new WritableStream({{
                write: function(chunk) {{ return Promise.resolve(); }},
                close: function() {{ return Promise.resolve(); }}
            }});
            var bidi = new WebTransportBidirectionalStream(readable, writable);
            return Promise.resolve(bidi);
        }};

        WebTransport.prototype.createUnidirectionalStream = function(options) {{
            if (this._closed) return Promise.reject(new WebTransportError("Connection closed", {{ source: 'session' }}));
            var writable = new WritableStream({{
                write: function(chunk) {{ return Promise.resolve(); }},
                close: function() {{ return Promise.resolve(); }}
            }});
            return Promise.resolve(writable);
        }};

        WebTransport.prototype.close = function(closeInfo) {{
            if (this._closed) return;
            this._closed = true;
            var code = (closeInfo && typeof closeInfo.closeCode === 'number') ? closeInfo.closeCode : 0;
            var reason = (closeInfo && closeInfo.reason) || "";
            this._resolveClosed({{ closeCode: code, reason: reason }});
        }};

        WebTransport.prototype.getStats = function() {{
            return Promise.resolve({{
                bytesSent: 0,
                packetsSent: 0,
                bytesReceived: 0,
                packetsReceived: 0,
                smoothedRtt: 15.0,
                rttVariation: 2.0,
                minRtt: 10.0,
                datagrams: {{
                    droppedSendRate: 0,
                    expiredOutgoing: 0,
                    lostOutgoing: 0
                }}
            }});
        }};
        globalThis.WebTransport = WebTransport;
    }})();

    // ── Multi-Threading, Shared Memory & Cross-Origin Isolation ─────
    (function() {{
        var _coi = false;
        var _coopHeader = "";
        var _coepHeader = "";

        function evaluateCoi() {{
            var coopOk = (_coopHeader === 'same-origin');
            var coepOk = (_coepHeader === 'require-corp' || _coepHeader === 'credentialless');
            _coi = coopOk && coepOk;
            return _coi;
        }}

        Object.defineProperty(globalThis, 'crossOriginIsolated', {{
            get: function() {{ return _coi; }},
            set: function(val) {{ _coi = !!val; }},
            configurable: true,
            enumerable: true
        }});

        if (typeof window !== 'undefined') {{
            Object.defineProperty(window, 'crossOriginIsolated', {{
                get: function() {{ return _coi; }},
                set: function(val) {{ _coi = !!val; }},
                configurable: true,
                enumerable: true
            }});
        }}

        globalThis.__setCrossOriginIsolationHeaders = function(coop, coep) {{
            _coopHeader = (coop || "").trim();
            _coepHeader = (coep || "").trim();
            return evaluateCoi();
        }};

        globalThis.__setCrossOriginIsolated = function(val) {{
            _coi = !!val;
            return _coi;
        }};
    }})();

}})();
"##
    )
}
