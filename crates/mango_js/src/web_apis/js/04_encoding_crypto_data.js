    // ── 14. TextEncoder / TextDecoder ─────────────────────────────────────────
    function TextEncoder() {
        this.encoding = 'utf-8';
    }
    TextEncoder.prototype.encode = function(input) {
        var s = String(input !== undefined ? input : '');
        var out = [];
        for (var i = 0; i < s.length; i++) {
            var c = s.charCodeAt(i);
            if (c < 0x80) {
                out.push(c);
            } else if (c < 0x800) {
                out.push(0xC0 | (c >> 6), 0x80 | (c & 0x3F));
            } else if (c >= 0xD800 && c <= 0xDBFF) {
                if (i + 1 < s.length) {
                    var c2 = s.charCodeAt(i + 1);
                    if (c2 >= 0xDC00 && c2 <= 0xDFFF) {
                        var codePoint = 0x10000 + ((c - 0xD800) << 10) + (c2 - 0xDC00);
                        out.push(0xF0 | (codePoint >> 18), 0x80 | ((codePoint >> 12) & 0x3F), 0x80 | ((codePoint >> 6) & 0x3F), 0x80 | (codePoint & 0x3F));
                        i++;
                        continue;
                    }
                }
                out.push(0xEF, 0xBF, 0xBD);
            } else if (c >= 0xDC00 && c <= 0xDFFF) {
                out.push(0xEF, 0xBF, 0xBD);
            } else {
                out.push(0xE0 | (c >> 12), 0x80 | ((c >> 6) & 0x3F), 0x80 | (c & 0x3F));
            }
        }
        return new Uint8Array(out);
    };
    TextEncoder.prototype.encodeInto = function(src, dest) {
        var bytes = this.encode(src);
        var written = Math.min(bytes.length, dest.length);
        dest.set(bytes.subarray(0, written));
        return { read: String(src).length, written: written };
    };
    globalThis.TextEncoder = TextEncoder;

    function TextDecoder(enc, opts) {
        this.encoding = enc ? String(enc).toLowerCase() : 'utf-8';
        this.fatal = !!(opts && opts.fatal);
        this.ignoreBOM = !!(opts && opts.ignoreBOM);
    }
    TextDecoder.prototype.decode = function(input, opts) {
        if (!input) return '';
        var arr;
        if (input instanceof ArrayBuffer) arr = new Uint8Array(input);
        else if (ArrayBuffer.isView(input)) arr = new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
        else arr = new Uint8Array(input);
        
        var s = '';
        var i = 0;
        while (i < arr.length) {
            var b = arr[i++];
            if (b < 0x80) {
                s += String.fromCharCode(b);
            } else if ((b & 0xE0) === 0xC0 && i < arr.length) {
                var b2 = arr[i++] & 0x3F;
                s += String.fromCharCode(((b & 0x1F) << 6) | b2);
            } else if ((b & 0xF0) === 0xE0 && i + 1 < arr.length) {
                var b2 = arr[i++] & 0x3F;
                var b3 = arr[i++] & 0x3F;
                s += String.fromCharCode(((b & 0x0F) << 12) | (b2 << 6) | b3);
            } else if ((b & 0xF8) === 0xF0 && i + 2 < arr.length) {
                var b2 = arr[i++] & 0x3F;
                var b3 = arr[i++] & 0x3F;
                var b4 = arr[i++] & 0x3F;
                var code = ((b & 0x07) << 18) | (b2 << 12) | (b3 << 6) | b4;
                code -= 0x10000;
                s += String.fromCharCode(0xD800 + (code >> 10), 0xDC00 + (code & 0x3FF));
            } else {
                s += '\uFFFD';
            }
        }
        return s;
    };
    globalThis.TextDecoder = TextDecoder;

    // ── 15. btoa / atob ───────────────────────────────────────────────────────
    var _B64CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    globalThis.btoa = function(str) {
        var s = String(str);
        var out = '';
        for (var i = 0; i < s.length; i += 3) {
            var a = s.charCodeAt(i);
            var b = s.charCodeAt(i + 1);
            var c = s.charCodeAt(i + 2);
            if (a > 255 || b > 255 || c > 255) {
                throw new Error('InvalidCharacterError: String contains characters outside Latin1 range');
            }
            out += _B64CHARS[a >> 2];
            out += _B64CHARS[((a & 3) << 4) | (isNaN(b) ? 0 : b >> 4)];
            out += isNaN(b) ? '=' : _B64CHARS[((b & 0xF) << 2) | (isNaN(c) ? 0 : c >> 6)];
            out += (isNaN(b) || isNaN(c)) ? '=' : _B64CHARS[c & 0x3F];
        }
        return out;
    };
    globalThis.atob = function(str) {
        var s = String(str).replace(/[\t\n\f\r ]+/g, '');
        if (s.length % 4 !== 0) throw new Error('InvalidCharacterError: Invalid base64 string length');
        var sClean = s.replace(/=+$/, '');
        var out = '';
        for (var i = 0; i < sClean.length; i += 4) {
            var v0 = _B64CHARS.indexOf(sClean[i]);
            var v1 = _B64CHARS.indexOf(sClean[i + 1]);
            var v2 = i + 2 < sClean.length ? _B64CHARS.indexOf(sClean[i + 2]) : 0;
            var v3 = i + 3 < sClean.length ? _B64CHARS.indexOf(sClean[i + 3]) : 0;
            if (v0 === -1 || v1 === -1 || (i + 2 < sClean.length && v2 === -1) || (i + 3 < sClean.length && v3 === -1)) {
                throw new Error('InvalidCharacterError: Invalid character in base64 string');
            }
            var n = (v0 << 18) | (v1 << 12) | (v2 << 6) | v3;
            out += String.fromCharCode((n >> 16) & 255);
            if (i + 2 < sClean.length) out += String.fromCharCode((n >> 8) & 255);
            if (i + 3 < sClean.length) out += String.fromCharCode(n & 255);
        }
        return out;
    };

    // ── 16. crypto ────────────────────────────────────────────────────────────
    function _sha256(data) {
        var bytes;
        if (typeof data === 'string') bytes = new TextEncoder().encode(data);
        else if (data instanceof ArrayBuffer) bytes = new Uint8Array(data);
        else if (ArrayBuffer.isView(data)) bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        else bytes = new Uint8Array(data);

        function rightRotate(value, amount) { return (value >>> amount) | (value << (32 - amount)); }
        var mathPow = Math.pow;
        var maxWord = mathPow(2, 32);
        var i, j;
        var words = [];
        var asciiBitLength = bytes.length * 8;
        var hash = [];
        var k = [];
        var primeCounter = 0;
        var isComposite = {};
        for (var candidate = 2; primeCounter < 64; candidate++) {
            if (!isComposite[candidate]) {
                for (i = 0; i < 313; i += candidate) isComposite[i] = candidate;
                hash[primeCounter] = (mathPow(candidate, .5) * maxWord) | 0;
                k[primeCounter++] = (mathPow(candidate, 1 / 3) * maxWord) | 0;
            }
        }
        for (i = 0; i < bytes.length; i++) {
            words[i >> 2] |= bytes[i] << (24 - (i % 4) * 8);
        }
        words[asciiBitLength >> 5] |= 0x80 << (24 - (asciiBitLength % 32));
        words[(((asciiBitLength + 64) >> 9) << 4) + 15] = asciiBitLength;
        var w = [];
        for (i = 0; i < words.length; i += 16) {
            var h0 = hash[0], h1 = hash[1], h2 = hash[2], h3 = hash[3], h4 = hash[4], h5 = hash[5], h6 = hash[6], h7 = hash[7];
            for (j = 0; j < 64; j++) {
                if (j < 16) w[j] = words[j + i] | 0;
                else {
                    var s0 = rightRotate(w[j - 15], 7) ^ rightRotate(w[j - 15], 18) ^ (w[j - 15] >>> 3);
                    var s1 = rightRotate(w[j - 2], 17) ^ rightRotate(w[j - 2], 19) ^ (w[j - 2] >>> 10);
                    w[j] = (((w[j - 16] + s0) | 0) + ((w[j - 7] + s1) | 0)) | 0;
                }
                var S1 = rightRotate(h4, 6) ^ rightRotate(h4, 11) ^ rightRotate(h4, 25);
                var ch = (h4 & h5) ^ ((~h4) & h6);
                var temp1 = (((h7 + S1) | 0) + ((ch + k[j]) | 0) + w[j]) | 0;
                var S0 = rightRotate(h0, 2) ^ rightRotate(h0, 13) ^ rightRotate(h0, 22);
                var maj = (h0 & h1) ^ (h0 & h2) ^ (h1 & h2);
                var temp2 = (S0 + maj) | 0;
                h7 = h6; h6 = h5; h5 = h4; h4 = (h3 + temp1) | 0;
                h3 = h2; h2 = h1; h1 = h0; h0 = (temp1 + temp2) | 0;
            }
            hash[0] = (hash[0] + h0) | 0; hash[1] = (hash[1] + h1) | 0;
            hash[2] = (hash[2] + h2) | 0; hash[3] = (hash[3] + h3) | 0;
            hash[4] = (hash[4] + h4) | 0; hash[5] = (hash[5] + h5) | 0;
            hash[6] = (hash[6] + h6) | 0; hash[7] = (hash[7] + h7) | 0;
        }
        var out = new Uint8Array(32);
        for (i = 0; i < 8; i++) {
            out[i * 4] = (hash[i] >> 24) & 0xFF;
            out[i * 4 + 1] = (hash[i] >> 16) & 0xFF;
            out[i * 4 + 2] = (hash[i] >> 8) & 0xFF;
            out[i * 4 + 3] = hash[i] & 0xFF;
        }
        return out.buffer;
    }

    var _subtle = {
        digest: function(algorithm, data) {
            try {
                return Promise.resolve(_sha256(data));
            } catch(e) {
                return Promise.reject(e);
            }
        },
        importKey: function() { return Promise.resolve({}); },
        exportKey: function() { return Promise.resolve(new ArrayBuffer(0)); },
        generateKey: function() { return Promise.resolve({}); },
        encrypt: function() { return Promise.resolve(new ArrayBuffer(0)); },
        decrypt: function() { return Promise.resolve(new ArrayBuffer(0)); },
        sign: function() { return Promise.resolve(new ArrayBuffer(0)); },
        verify: function() { return Promise.resolve(true); }
    };

    globalThis.crypto = {
        subtle: _subtle,
        getRandomValues: function(buf) {
            if (!buf || typeof buf !== 'object' || !ArrayBuffer.isView(buf)) {
                throw new TypeError('crypto.getRandomValues: argument must be an ArrayBufferView');
            }
            if (buf instanceof Float32Array || buf instanceof Float64Array) {
                throw new TypeError('crypto.getRandomValues: floating point arrays are not supported');
            }
            var byteLen = buf.byteLength;
            if (byteLen > 65536) {
                if (typeof DOMException !== 'undefined') {
                    throw new DOMException('The requested length exceeds 65,536 bytes', 'QuotaExceededError');
                } else {
                    var err = new Error('crypto.getRandomValues: QuotaExceededError - requested length exceeds 65536 bytes');
                    err.name = 'QuotaExceededError';
                    throw err;
                }
            }
            var u8 = new Uint8Array(buf.buffer, buf.byteOffset, byteLen);
            if (typeof _mangoRandomBytes === 'function') {
                var rand = _mangoRandomBytes(byteLen);
                for (var i = 0; i < byteLen; i++) {
                    u8[i] = rand[i];
                }
            } else {
                for (var i = 0; i < byteLen; i++) {
                    u8[i] = Math.floor(Math.random() * 256);
                }
            }
            return buf;
        },
        randomUUID: function() {
            var b = new Uint8Array(16);
            this.getRandomValues(b);
            b[6] = (b[6] & 0x0f) | 0x40;
            b[8] = (b[8] & 0x3f) | 0x80;
            var hex = Array.from(b, function(x) { return x.toString(16).padStart(2, '0'); });
            return hex[0]+hex[1]+hex[2]+hex[3]+'-'+hex[4]+hex[5]+'-'+hex[6]+hex[7]+'-'+hex[8]+hex[9]+'-'+hex[10]+hex[11]+hex[12]+hex[13]+hex[14]+hex[15];
        }
    };
    if (typeof window !== 'undefined') {
        window.crypto = globalThis.crypto;
    }

    // ── 17. structuredClone ───────────────────────────────────────────────────
    globalThis.structuredClone = function(val, options) {
        var seen = new Map();
        function clone(v) {
            if (v === null || typeof v !== 'object') return v;
            if (seen.has(v)) return seen.get(v);
            
            if (v instanceof Date) return new Date(v.getTime());
            if (v instanceof RegExp) return new RegExp(v.source, v.flags);
            if (v instanceof ArrayBuffer) return v.slice(0);
            if (ArrayBuffer.isView(v)) {
                if (v instanceof Uint8Array) return new Uint8Array(v);
                if (v instanceof Int8Array) return new Int8Array(v);
                if (v instanceof Uint16Array) return new Uint16Array(v);
                if (v instanceof Int16Array) return new Int16Array(v);
                if (v instanceof Uint32Array) return new Uint32Array(v);
                if (v instanceof Int32Array) return new Int32Array(v);
                if (v instanceof Float32Array) return new Float32Array(v);
                if (v instanceof Float64Array) return new Float64Array(v);
                return new Uint8Array(v);
            }
            if (v instanceof Map) {
                var m = new Map();
                seen.set(v, m);
                v.forEach(function(val, key) {
                    m.set(clone(key), clone(val));
                });
                return m;
            }
            if (v instanceof Set) {
                var s = new Set();
                seen.set(v, s);
                v.forEach(function(item) {
                    s.add(clone(item));
                });
                return s;
            }
            if (Array.isArray(v)) {
                var arr = [];
                seen.set(v, arr);
                for (var i = 0; i < v.length; i++) {
                    arr[i] = clone(v[i]);
                }
                return arr;
            }
            
            var copy = {};
            seen.set(v, copy);
            var keys = Object.keys(v);
            for (var i = 0; i < keys.length; i++) {
                var k = keys[i];
                copy[k] = clone(v[k]);
            }
            return copy;
        }
        return clone(val);
    };

    // ── 18. FormData ──────────────────────────────────────────────────────────
    function FormData(form) {
        this._entries = [];
        if (form && form.elements) {
            var elements = Array.from(form.elements);
            for (var i = 0; i < elements.length; i++) {
                var el = elements[i];
                if (el.name && el.value !== undefined && !el.disabled) {
                    var type = (el.type || '').toLowerCase();
                    if (type === 'checkbox' || type === 'radio') {
                        if (el.checked) this.append(el.name, el.value || 'on');
                    } else if (type === 'file') {
                        if (el.files && el.files.length) {
                            for (var f = 0; f < el.files.length; f++) {
                                this.append(el.name, el.files[f]);
                            }
                        } else {
                            this.append(el.name, new File([], '', { type: 'application/octet-stream' }));
                        }
                    } else {
                        this.append(el.name, el.value);
                    }
                }
            }
        }
    }
    FormData.prototype = {
        constructor: FormData,
        append: function(k, v, filename) {
            var val = v;
            if (filename !== undefined && !(val instanceof File)) {
                val = new File([val], filename);
            }
            this._entries.push([String(k), val]);
        },
        set: function(k, v, filename) {
            var strKey = String(k);
            var val = v;
            if (filename !== undefined && !(val instanceof File)) {
                val = new File([val], filename);
            }
            var found = false;
            var newEntries = [];
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strKey) {
                    if (!found) {
                        newEntries.push([strKey, val]);
                        found = true;
                    }
                } else {
                    newEntries.push(this._entries[i]);
                }
            }
            if (!found) newEntries.push([strKey, val]);
            this._entries = newEntries;
        },
        delete: function(k) {
            var strKey = String(k);
            this._entries = this._entries.filter(function(e) { return e[0] !== strKey; });
        },
        get: function(k) {
            var strKey = String(k);
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strKey) return this._entries[i][1];
            }
            return null;
        },
        getAll: function(k) {
            var strKey = String(k);
            var res = [];
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strKey) res.push(this._entries[i][1]);
            }
            return res;
        },
        has: function(k) {
            var strKey = String(k);
            for (var i = 0; i < this._entries.length; i++) {
                if (this._entries[i][0] === strKey) return true;
            }
            return false;
        },
        forEach: function(cb, thisArg) {
            for (var i = 0; i < this._entries.length; i++) {
                var e = this._entries[i];
                cb.call(thisArg, e[1], e[0], this);
            }
        },
        *keys() {
            for (var i = 0; i < this._entries.length; i++) {
                yield this._entries[i][0];
            }
        },
        *values() {
            for (var i = 0; i < this._entries.length; i++) {
                yield this._entries[i][1];
            }
        },
        *entries() {
            for (var i = 0; i < this._entries.length; i++) {
                var e = this._entries[i];
                yield [e[0], e[1]];
            }
        },
        serializeUrlEncoded: function() {
            var pairs = [];
            for (var i = 0; i < this._entries.length; i++) {
                var k = encodeURIComponent(this._entries[i][0]).replace(/%20/g, '+');
                var v = encodeURIComponent(String(this._entries[i][1])).replace(/%20/g, '+');
                pairs.push(k + '=' + v);
            }
            return pairs.join('&');
        },
        toUrlSearchParams: function() {
            if (typeof globalThis.URLSearchParams === 'function') {
                var usp = new globalThis.URLSearchParams();
                for (var i = 0; i < this._entries.length; i++) {
                    usp.append(this._entries[i][0], String(this._entries[i][1]));
                }
                return usp;
            }
            return null;
        },
        serializeMultipart: function(boundary) {
            var b = boundary || ('----WebKitFormBoundary' + Math.random().toString(36).substring(2));
            var lines = [];
            for (var i = 0; i < this._entries.length; i++) {
                var k = this._entries[i][0];
                var v = this._entries[i][1];
                lines.push('--' + b);
                if (v && typeof v === 'object' && v.name !== undefined) {
                    var fn = v.name || 'blob';
                    var ct = v.type || 'application/octet-stream';
                    lines.push('Content-Disposition: form-data; name="' + k + '"; filename="' + fn + '"');
                    lines.push('Content-Type: ' + ct);
                    lines.push('');
                    lines.push(v._data ? (typeof v._data === 'string' ? v._data : String(v._data)) : '');
                } else {
                    lines.push('Content-Disposition: form-data; name="' + k + '"');
                    lines.push('');
                    lines.push(String(v));
                }
            }
            lines.push('--' + b + '--');
            lines.push('');
            return {
                boundary: b,
                contentType: 'multipart/form-data; boundary=' + b,
                body: lines.join('\r\n')
            };
        }
    };
    FormData.prototype[Symbol.iterator] = FormData.prototype.entries;
    FormData.prototype[Symbol.toStringTag] = 'FormData';
    globalThis.FormData = FormData;

    // ── 19. Blob / File / FileReader ──────────────────────────────────────────
    function Blob(parts, opts) {
        this.type = (opts && opts.type) ? String(opts.type).toLowerCase() : '';
        var byteChunks = [];
        var totalSize = 0;
        if (parts && parts.length) {
            for (var i = 0; i < parts.length; i++) {
                var p = parts[i];
                if (p instanceof Blob) {
                    var chunk = p._getBytes();
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                } else if (p instanceof ArrayBuffer) {
                    var chunk = new Uint8Array(p);
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                } else if (ArrayBuffer.isView(p)) {
                    var chunk = new Uint8Array(p.buffer, p.byteOffset, p.byteLength);
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                } else {
                    var chunk = new TextEncoder().encode(String(p));
                    byteChunks.push(chunk);
                    totalSize += chunk.length;
                }
            }
        }
        var merged = new Uint8Array(totalSize);
        var offset = 0;
        for (var i = 0; i < byteChunks.length; i++) {
            merged.set(byteChunks[i], offset);
            offset += byteChunks[i].length;
        }
        this._bytes = merged;
        this.size = totalSize;
    }
    Blob.prototype = {
        _getBytes: function() {
            return this._bytes;
        },
        slice: function(start, end, ct) {
            var len = this.size;
            var s = start === undefined ? 0 : (start < 0 ? Math.max(len + start, 0) : Math.min(start, len));
            var e = end === undefined ? len : (end < 0 ? Math.max(len + end, 0) : Math.min(end, len));
            var span = Math.max(e - s, 0);
            var sub = this._bytes.subarray(s, s + span);
            return new Blob([sub], { type: ct !== undefined ? ct : this.type });
        },
        text: function() {
            return Promise.resolve(new TextDecoder().decode(this._bytes));
        },
        arrayBuffer: function() {
            var copy = new Uint8Array(this._bytes.length);
            copy.set(this._bytes);
            return Promise.resolve(copy.buffer);
        },
        bytes: function() {
            return Promise.resolve(new Uint8Array(this._bytes));
        },
        stream: function() {
            return null;
        }
    };
    globalThis.Blob = Blob;

    function File(parts, name, opts) {
        Blob.call(this, parts, opts);
        this.name = String(name || '');
        this.lastModified = (opts && opts.lastModified) ? Number(opts.lastModified) : Date.now();
        this.webkitRelativePath = '';
    }
    File.prototype = Object.create(Blob.prototype);
    globalThis.File = File;

    function FileReader() {
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
    }
    FileReader.EMPTY   = 0;
    FileReader.LOADING = 1;
    FileReader.DONE    = 2;
    FileReader.prototype = Object.create(EventTarget.prototype);
    Object.assign(FileReader.prototype, { EMPTY: 0, LOADING: 1, DONE: 2 });

    FileReader.prototype.readAsText = function(blob, encoding) {
        this._read(blob, 'text', encoding);
    };
    FileReader.prototype.readAsDataURL = function(blob) {
        this._read(blob, 'dataurl');
    };
    FileReader.prototype.readAsArrayBuffer = function(blob) {
        this._read(blob, 'arraybuffer');
    };
    FileReader.prototype.readAsBinaryString = function(blob) {
        this._read(blob, 'binarystring');
    };
    FileReader.prototype.abort = function() {
        this._aborted = true;
        this.readyState = 2; // DONE
        this.result = null;
        this.dispatchEvent({ type: 'abort', target: this });
        this.dispatchEvent({ type: 'loadend', target: this });
    };
    FileReader.prototype._read = function(blob, mode, enc) {
        var self = this;
        if (!(blob instanceof Blob)) {
            throw new TypeError("Failed to execute read on 'FileReader': parameter 1 is not of type 'Blob'.");
        }
        this._aborted = false;
        this.readyState = 1; // LOADING
        this.result = null;
        this.error = null;
        this.dispatchEvent({ type: 'loadstart', target: this });
        
        var schedule = (typeof queueMicrotask === 'function') ? queueMicrotask : function(fn) { setTimeout(fn, 0); };
        schedule(function() {
            if (self._aborted) return;
            var bytes = blob._getBytes();
            if (mode === 'text') {
                self.result = new TextDecoder(enc || 'utf-8').decode(bytes);
            } else if (mode === 'dataurl') {
                var b64 = '';
                for (var i = 0; i < bytes.length; i++) b64 += String.fromCharCode(bytes[i]);
                self.result = 'data:' + (blob.type || 'application/octet-stream') + ';base64,' + btoa(b64);
            } else if (mode === 'arraybuffer') {
                var copy = new Uint8Array(bytes.length);
                copy.set(bytes);
                self.result = copy.buffer;
            } else if (mode === 'binarystring') {
                var bs = '';
                for (var i = 0; i < bytes.length; i++) bs += String.fromCharCode(bytes[i]);
                self.result = bs;
            }
            self.readyState = 2; // DONE
            self.dispatchEvent({ type: 'progress', lengthComputable: true, loaded: bytes.length, total: bytes.length, target: self });
            self.dispatchEvent({ type: 'load', target: self });
            self.dispatchEvent({ type: 'loadend', target: self });
        });
    };
    globalThis.FileReader = FileReader;

    // ── 20. AbortController / AbortSignal ────────────────────────────────────
    function AbortSignal() {
        EventTarget.call(this);
        this.aborted = false;
        this.reason = undefined;
        this.onabort = null;
    }
    AbortSignal.prototype = Object.create(EventTarget.prototype);
    AbortSignal.prototype.throwIfAborted = function() {
        if (this.aborted) throw (this.reason !== undefined ? this.reason : new Error('The operation was aborted.'));
    };
    AbortSignal.abort = function(reason) {
        var sig = new AbortSignal();
        sig.aborted = true;
        sig.reason = reason !== undefined ? reason : new Error('This operation was aborted');
        return sig;
    };
    AbortSignal.timeout = function(ms) {
        var c = new AbortController();
        setTimeout(function() {
            c.abort(new Error('TimeoutError: The operation timed out.'));
        }, ms);
        return c.signal;
    };
    AbortSignal.any = function(signals) {
        var sigs = Array.from(signals || []);
        var ac = new AbortController();
        for (var i = 0; i < sigs.length; i++) {
            var s = sigs[i];
            if (s.aborted) {
                ac.abort(s.reason);
                return ac.signal;
            }
            s.addEventListener('abort', function() {
                ac.abort(this.reason);
            });
        }
        return ac.signal;
    };
    globalThis.AbortSignal = AbortSignal;

    function AbortController() {
        this.signal = new AbortSignal();
    }
    AbortController.prototype.abort = function(reason) {
        if (!this.signal.aborted) {
            this.signal.aborted = true;
            this.signal.reason = reason !== undefined ? reason : new Error('The operation was aborted.');
            var evt = { type: 'abort', target: this.signal, currentTarget: this.signal };
            this.signal.dispatchEvent(evt);
        }
    };
    globalThis.AbortController = AbortController;

