    // ── 34. Intl API (ECMA-402) ────────────────────────────────────────────────
    (function() {
        var Intl = globalThis.Intl || {};

        // 1. NumberFormat
        function NumberFormat(locales, options) {
            options = options || {};
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._style = options.style || 'decimal';
            this._currency = options.currency || 'USD';
            this._minimumFractionDigits = options.minimumFractionDigits !== undefined ? options.minimumFractionDigits : (this._style === 'currency' ? 2 : 0);
            this._maximumFractionDigits = options.maximumFractionDigits !== undefined ? options.maximumFractionDigits : (this._style === 'currency' ? 2 : 3);
            this._notation = options.notation || 'standard';
            this._useGrouping = options.useGrouping !== false;
        }
        NumberFormat.prototype.resolvedOptions = function() {
            return {
                locale: this._locale,
                numberingSystem: 'latn',
                style: this._style,
                currency: this._currency,
                minimumFractionDigits: this._minimumFractionDigits,
                maximumFractionDigits: this._maximumFractionDigits,
                useGrouping: this._useGrouping,
                notation: this._notation
            };
        };
        function _formatNumber(nf, num) {
            var n = Number(num);
            if (isNaN(n)) return 'NaN';
            if (nf._notation === 'compact') {
                var abs = Math.abs(n);
                var sign = n < 0 ? '-' : '';
                if (abs >= 1e9) return sign + (abs / 1e9).toFixed(1).replace(/\.0$/, '') + 'B';
                if (abs >= 1e6) return sign + (abs / 1e6).toFixed(1).replace(/\.0$/, '') + 'M';
                if (abs >= 1e3) return sign + (abs / 1e3).toFixed(1).replace(/\.0$/, '') + 'K';
                return sign + abs.toString();
            }
            var fixed = n.toFixed(nf._maximumFractionDigits);
            if (nf._minimumFractionDigits < nf._maximumFractionDigits) {
                var parts = fixed.split('.');
                if (parts.length > 1) {
                    var dec = parts[1];
                    while (dec.length > nf._minimumFractionDigits && dec.endsWith('0')) {
                        dec = dec.slice(0, -1);
                    }
                    fixed = dec.length > 0 ? parts[0] + '.' + dec : parts[0];
                }
            }
            if (nf._useGrouping) {
                var p = fixed.split('.');
                p[0] = p[0].replace(/\B(?=(\d{3})+(?!\d))/g, ',');
                fixed = p.join('.');
            }
            if (nf._style === 'currency') {
                var sym = (nf._currency === 'EUR') ? '€' : (nf._currency === 'GBP') ? '£' : (nf._currency === 'JPY') ? '¥' : '$';
                return sym + fixed;
            } else if (nf._style === 'percent') {
                return (n * 100).toFixed(nf._maximumFractionDigits) + '%';
            }
            return fixed;
        }

        Object.defineProperty(NumberFormat.prototype, 'format', {
            get: function() {
                var self = this;
                if (!this._boundFormat) {
                    this._boundFormat = function(num) {
                        return _formatNumber(self, num);
                    };
                }
                return this._boundFormat;
            },
            configurable: true,
            enumerable: false
        });

        NumberFormat.prototype.formatToParts = function(num) {
            return [{ type: 'integer', value: this.format(num) }];
        };
        NumberFormat.supportedLocalesOf = function(locales) {
            return Array.isArray(locales) ? locales.slice() : [locales || 'en-US'];
        };
        Intl.NumberFormat = NumberFormat;

        // 2. DateTimeFormat
        function DateTimeFormat(locales, options) {
            options = options || {};
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._options = options;
        }
        DateTimeFormat.prototype.resolvedOptions = function() {
            return Object.assign({
                locale: this._locale,
                calendar: 'gregory',
                numberingSystem: 'latn',
                timeZone: 'UTC'
            }, this._options);
        };
        function _formatDateTime(dtf, date) {
            var d = (date === undefined) ? new Date() : (date instanceof Date ? date : new Date(date));
            if (isNaN(d.getTime())) return 'Invalid Date';
            var opts = dtf._options || {};
            if (opts.year && opts.month && opts.day) {
                return (d.getMonth() + 1) + '/' + d.getDate() + '/' + d.getFullYear();
            }
            if (opts.hour || opts.minute) {
                return d.toLocaleTimeString ? d.toLocaleTimeString() : d.toTimeString().split(' ')[0];
            }
            return d.toLocaleDateString ? d.toLocaleDateString() : d.toDateString();
        }

        Object.defineProperty(DateTimeFormat.prototype, 'format', {
            get: function() {
                var self = this;
                if (!this._boundFormat) {
                    this._boundFormat = function(date) {
                        return _formatDateTime(self, date);
                    };
                }
                return this._boundFormat;
            },
            configurable: true,
            enumerable: false
        });
        DateTimeFormat.prototype.formatToParts = function(date) {
            return [{ type: 'literal', value: this.format(date) }];
        };
        DateTimeFormat.prototype.formatRange = function(start, end) {
            return this.format(start) + ' – ' + this.format(end);
        };
        DateTimeFormat.supportedLocalesOf = function(locales) {
            return Array.isArray(locales) ? locales.slice() : [locales || 'en-US'];
        };
        Intl.DateTimeFormat = DateTimeFormat;

        // 3. Collator
        function Collator(locales, options) {
            this._locale = (typeof locales === 'string') ? locales : 'en-US';
            this._options = options || {};
        }
        Collator.prototype.resolvedOptions = function() {
            return { locale: this._locale, usage: 'sort', sensitivity: 'variant' };
        };
        Collator.prototype.compare = function(a, b) {
            var sa = String(a), sb = String(b);
            return sa.localeCompare ? sa.localeCompare(sb) : (sa < sb ? -1 : (sa > sb ? 1 : 0));
        };
        Collator.supportedLocalesOf = function(locales) { return [locales || 'en-US']; };
        Intl.Collator = Collator;

        // 4. PluralRules
        function PluralRules(locales, options) {
            this._locale = locales || 'en-US';
        }
        PluralRules.prototype.resolvedOptions = function() { return { locale: this._locale, type: 'cardinal' }; };
        PluralRules.prototype.select = function(n) { return Number(n) === 1 ? 'one' : 'other'; };
        Intl.PluralRules = PluralRules;

        // 5. RelativeTimeFormat
        function RelativeTimeFormat(locales, options) {
            this._locale = locales || 'en-US';
            this._numeric = (options && options.numeric) || 'always';
        }
        RelativeTimeFormat.prototype.resolvedOptions = function() { return { locale: this._locale, numeric: this._numeric }; };
        RelativeTimeFormat.prototype.format = function(value, unit) {
            var v = Math.round(Number(value));
            var u = String(unit).toLowerCase();
            if (v === 0 && this._numeric === 'auto') {
                if (u === 'day') return 'today';
                if (u === 'hour') return 'this hour';
            }
            var pluralUnit = (Math.abs(v) === 1) ? u : (u.endsWith('s') ? u : u + 's');
            if (v < 0) return Math.abs(v) + ' ' + pluralUnit + ' ago';
            return 'in ' + v + ' ' + pluralUnit;
        };
        RelativeTimeFormat.prototype.formatToParts = function(v, u) {
            return [{ type: 'literal', value: this.format(v, u) }];
        };
        Intl.RelativeTimeFormat = RelativeTimeFormat;

        // 6. DisplayNames
        var _displayNamesDict = {
            'en': 'English', 'es': 'Spanish', 'fr': 'French', 'de': 'German',
            'zh': 'Chinese', 'ja': 'Japanese', 'ko': 'Korean', 'ru': 'Russian',
            'pt': 'Portuguese', 'it': 'Italian', 'ar': 'Arabic', 'hi': 'Hindi',
            'US': 'United States', 'GB': 'United Kingdom', 'CA': 'Canada',
            'FR': 'France', 'DE': 'Germany', 'JP': 'Japan', 'CN': 'China',
            'USD': 'US Dollar', 'EUR': 'Euro', 'GBP': 'British Pound', 'JPY': 'Japanese Yen'
        };
        function DisplayNames(locales, options) {
            this._locale = locales || 'en-US';
            this._type = (options && options.type) || 'language';
        }
        DisplayNames.prototype.resolvedOptions = function() { return { locale: this._locale, type: this._type }; };
        DisplayNames.prototype.of = function(code) {
            var c = String(code);
            return _displayNamesDict[c] || _displayNamesDict[c.toLowerCase()] || c;
        };
        Intl.DisplayNames = DisplayNames;

        // 7. ListFormat
        function ListFormat(locales, options) {
            this._locale = locales || 'en-US';
            this._type = (options && options.type) || 'conjunction';
        }
        ListFormat.prototype.resolvedOptions = function() { return { locale: this._locale, type: this._type }; };
        ListFormat.prototype.format = function(list) {
            if (!Array.isArray(list)) return '';
            if (list.length === 0) return '';
            if (list.length === 1) return String(list[0]);
            if (list.length === 2) return list[0] + (this._type === 'disjunction' ? ' or ' : ' and ') + list[1];
            var last = list[list.length - 1];
            var rest = list.slice(0, -1).join(', ');
            return rest + (this._type === 'disjunction' ? ', or ' : ', and ') + last;
        };
        ListFormat.prototype.formatToParts = function(list) {
            return [{ type: 'element', value: this.format(list) }];
        };
        Intl.ListFormat = ListFormat;

        // 8. Segmenter
        function Segmenter(locales, options) {
            this._locale = locales || 'en-US';
        }
        Segmenter.prototype.resolvedOptions = function() { return { locale: this._locale, granularity: 'grapheme' }; };
        Segmenter.prototype.segment = function(input) {
            var str = String(input);
            var chars = Array.from(str);
            var idx = 0;
            var segments = chars.map(function(ch) {
                var seg = { segment: ch, index: idx, input: str, isWordLike: /\\w/.test(ch) };
                idx += ch.length;
                return seg;
            });
            return {
                [Symbol.iterator]: function() {
                    var i = 0;
                    return {
                        next: function() {
                            return i < segments.length ? { value: segments[i++], done: false } : { done: true };
                        }
                    };
                }
            };
        };
        Intl.Segmenter = Segmenter;

        globalThis.Intl = Intl;
    })();

    // -------------------------------------------------------------
    // WebGL & WebGL2 Rendering Contexts (W3C WebGL / WebGL 2 Spec)
    // -------------------------------------------------------------
    (function() {
        var WEBGL_CONSTANTS = {
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
        };

        function WebGLShader(type) {
            this._type = type;
            this._source = '';
            this._compiled = false;
        }
        function WebGLProgram() {
            this._shaders = [];
            this._linked = false;
            this._uniforms = {};
            this._attribs = {};
        }
        function WebGLBuffer() {
            this._target = 0;
            this._data = null;
            this._usage = 0;
        }
        function WebGLTexture() {
            this._params = {};
            this._data = null;
        }
        function WebGLFramebuffer() {}
        function WebGLRenderbuffer() {}
        function WebGLUniformLocation(name) { this._name = name; }

        function WebGLRenderingContext(canvas) {
            this.canvas = canvas;
            this.drawingBufferWidth = canvas ? (canvas.width || 300) : 300;
            this.drawingBufferHeight = canvas ? (canvas.height || 150) : 150;
            this._clearColor = [0, 0, 0, 0];
            this._clearDepth = 1.0;
            this._clearStencil = 0;
            this._viewport = [0, 0, this.drawingBufferWidth, this.drawingBufferHeight];
            this._boundBuffers = {};
            this._boundTextures = {};
            this._currentProgram = null;
        }

        for (var k in WEBGL_CONSTANTS) {
            WebGLRenderingContext[k] = WEBGL_CONSTANTS[k];
            WebGLRenderingContext.prototype[k] = WEBGL_CONSTANTS[k];
        }

        WebGLRenderingContext.prototype.viewport = function(x, y, w, h) { this._viewport = [x, y, w, h]; };
        WebGLRenderingContext.prototype.clearColor = function(r, g, b, a) { this._clearColor = [r, g, b, a]; };
        WebGLRenderingContext.prototype.clearDepth = function(d) { this._clearDepth = d; };
        WebGLRenderingContext.prototype.clearStencil = function(s) { this._clearStencil = s; };
        WebGLRenderingContext.prototype.clear = function(mask) {};
        WebGLRenderingContext.prototype.enable = function(cap) {};
        WebGLRenderingContext.prototype.disable = function(cap) {};
        WebGLRenderingContext.prototype.blendFunc = function(sfactor, dfactor) {};
        WebGLRenderingContext.prototype.createShader = function(type) { return new WebGLShader(type); };
        WebGLRenderingContext.prototype.shaderSource = function(shader, source) { if (shader) shader._source = source; };
        WebGLRenderingContext.prototype.compileShader = function(shader) { if (shader) shader._compiled = true; };
        WebGLRenderingContext.prototype.getShaderParameter = function(shader, pname) {
            if (pname === WEBGL_CONSTANTS.COMPILE_STATUS) return true;
            if (pname === WEBGL_CONSTANTS.DELETE_STATUS) return false;
            return true;
        };
        WebGLRenderingContext.prototype.getShaderInfoLog = function(shader) { return ''; };
        WebGLRenderingContext.prototype.createProgram = function() { return new WebGLProgram(); };
        WebGLRenderingContext.prototype.attachShader = function(program, shader) { if (program && shader) program._shaders.push(shader); };
        WebGLRenderingContext.prototype.linkProgram = function(program) { if (program) program._linked = true; };
        WebGLRenderingContext.prototype.useProgram = function(program) { this._currentProgram = program; };
        WebGLRenderingContext.prototype.getProgramParameter = function(program, pname) {
            if (pname === WEBGL_CONSTANTS.LINK_STATUS) return true;
            if (pname === WEBGL_CONSTANTS.VALIDATE_STATUS) return true;
            return true;
        };
        WebGLRenderingContext.prototype.getProgramInfoLog = function(program) { return ''; };
        WebGLRenderingContext.prototype.createBuffer = function() { return new WebGLBuffer(); };
        WebGLRenderingContext.prototype.bindBuffer = function(target, buffer) { this._boundBuffers[target] = buffer; };
        WebGLRenderingContext.prototype.bufferData = function(target, data, usage) {
            var buf = this._boundBuffers[target];
            if (buf) { buf._data = data; buf._usage = usage; }
        };
        WebGLRenderingContext.prototype.bufferSubData = function(target, offset, data) {};
        WebGLRenderingContext.prototype.createTexture = function() { return new WebGLTexture(); };
        WebGLRenderingContext.prototype.bindTexture = function(target, texture) { this._boundTextures[target] = texture; };
        WebGLRenderingContext.prototype.texImage2D = function() {};
        WebGLRenderingContext.prototype.texParameteri = function(target, pname, param) {};
        WebGLRenderingContext.prototype.generateMipmap = function(target) {};
        WebGLRenderingContext.prototype.createFramebuffer = function() { return new WebGLFramebuffer(); };
        WebGLRenderingContext.prototype.bindFramebuffer = function(target, fb) {};
        WebGLRenderingContext.prototype.createRenderbuffer = function() { return new WebGLRenderbuffer(); };
        WebGLRenderingContext.prototype.bindRenderbuffer = function(target, rb) {};
        WebGLRenderingContext.prototype.getAttribLocation = function(program, name) { return 0; };
        WebGLRenderingContext.prototype.enableVertexAttribArray = function(index) {};
        WebGLRenderingContext.prototype.disableVertexAttribArray = function(index) {};
        WebGLRenderingContext.prototype.vertexAttribPointer = function() {};
        WebGLRenderingContext.prototype.getUniformLocation = function(program, name) { return new WebGLUniformLocation(name); };
        WebGLRenderingContext.prototype.uniform1f = function(loc, x) {};
        WebGLRenderingContext.prototype.uniform2f = function(loc, x, y) {};
        WebGLRenderingContext.prototype.uniform3f = function(loc, x, y, z) {};
        WebGLRenderingContext.prototype.uniform4f = function(loc, x, y, z, w) {};
        WebGLRenderingContext.prototype.uniform1i = function(loc, x) {};
        WebGLRenderingContext.prototype.uniformMatrix4fv = function(loc, transpose, val) {};
        WebGLRenderingContext.prototype.drawArrays = function(mode, first, count) {};
        WebGLRenderingContext.prototype.drawElements = function(mode, count, type, offset) {};
        WebGLRenderingContext.prototype.getExtension = function(name) { return null; };
        WebGLRenderingContext.prototype.getSupportedExtensions = function() { return ['OES_element_index_uint', 'EXT_texture_filter_anisotropic']; };
        WebGLRenderingContext.prototype.getParameter = function(pname) {
            if (pname === 0x1F00) return 'Mango WebGL Renderer';
            if (pname === 0x1F01) return 'Mango GPU Rasterizer';
            if (pname === 0x1F02) return 'WebGL 1.0 (Mango Engine)';
            if (pname === 0x8B8C) return 'WebGL GLSL ES 1.0';
            return 0;
        };
        WebGLRenderingContext.prototype.getError = function() { return 0; };

        var WEBGL2_CONSTANTS = Object.assign({}, WEBGL_CONSTANTS, {
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
        });

        function WebGLVertexArrayObject() {}
        function WebGLQuery() {}
        function WebGLSync() {}
        function WebGLTransformFeedback() {}
        function WebGLSampler() {}

        function WebGL2RenderingContext(canvas) {
            WebGLRenderingContext.call(this, canvas);
        }
        WebGL2RenderingContext.prototype = Object.create(WebGLRenderingContext.prototype);
        WebGL2RenderingContext.prototype.constructor = WebGL2RenderingContext;

        for (var k2 in WEBGL2_CONSTANTS) {
            WebGL2RenderingContext[k2] = WEBGL2_CONSTANTS[k2];
            WebGL2RenderingContext.prototype[k2] = WEBGL2_CONSTANTS[k2];
        }

        WebGL2RenderingContext.prototype.getParameter = function(pname) {
            if (pname === 0x1F00) return 'Mango WebGL Renderer';
            if (pname === 0x1F01) return 'Mango GPU Rasterizer';
            if (pname === 0x1F02) return 'WebGL 2.0 (Mango Engine)';
            if (pname === 0x8B8C) return 'WebGL GLSL ES 3.00 (Mango Engine)';
            return 0;
        };

        WebGL2RenderingContext.prototype.createVertexArray = function() { return new WebGLVertexArrayObject(); };
        WebGL2RenderingContext.prototype.bindVertexArray = function(vao) {};
        WebGL2RenderingContext.prototype.deleteVertexArray = function(vao) {};
        WebGL2RenderingContext.prototype.isVertexArray = function(vao) { return vao instanceof WebGLVertexArrayObject; };

        WebGL2RenderingContext.prototype.createQuery = function() { return new WebGLQuery(); };
        WebGL2RenderingContext.prototype.beginQuery = function(target, q) {};
        WebGL2RenderingContext.prototype.endQuery = function(target) {};
        WebGL2RenderingContext.prototype.deleteQuery = function(q) {};
        WebGL2RenderingContext.prototype.isQuery = function(q) { return q instanceof WebGLQuery; };
        WebGL2RenderingContext.prototype.getQueryParameter = function(q, pname) {
            if (pname === 0x8867) return true;
            if (pname === 0x8866) return 0;
            return 0;
        };

        WebGL2RenderingContext.prototype.fenceSync = function(condition, flags) { return new WebGLSync(); };
        WebGL2RenderingContext.prototype.clientWaitSync = function(sync, flags, timeout) { return 0x911A; };
        WebGL2RenderingContext.prototype.waitSync = function(sync, flags, timeout) {};
        WebGL2RenderingContext.prototype.deleteSync = function(sync) {};
        WebGL2RenderingContext.prototype.isSync = function(sync) { return sync instanceof WebGLSync; };

        WebGL2RenderingContext.prototype.createTransformFeedback = function() { return new WebGLTransformFeedback(); };
        WebGL2RenderingContext.prototype.bindTransformFeedback = function(target, tf) {};
        WebGL2RenderingContext.prototype.deleteTransformFeedback = function(tf) {};
        WebGL2RenderingContext.prototype.isTransformFeedback = function(tf) { return tf instanceof WebGLTransformFeedback; };
        WebGL2RenderingContext.prototype.beginTransformFeedback = function(primitiveMode) {};
        WebGL2RenderingContext.prototype.endTransformFeedback = function() {};
        WebGL2RenderingContext.prototype.pauseTransformFeedback = function() {};
        WebGL2RenderingContext.prototype.resumeTransformFeedback = function() {};

        WebGL2RenderingContext.prototype.createSampler = function() { return new WebGLSampler(); };
        WebGL2RenderingContext.prototype.bindSampler = function(unit, sampler) {};
        WebGL2RenderingContext.prototype.deleteSampler = function(sampler) {};
        WebGL2RenderingContext.prototype.isSampler = function(sampler) { return sampler instanceof WebGLSampler; };
        WebGL2RenderingContext.prototype.samplerParameteri = function(sampler, pname, param) {};
        WebGL2RenderingContext.prototype.samplerParameterf = function(sampler, pname, param) {};

        WebGL2RenderingContext.prototype.bindBufferBase = function(target, index, buffer) {};
        WebGL2RenderingContext.prototype.bindBufferRange = function(target, index, buffer, offset, size) {};
        WebGL2RenderingContext.prototype.getUniformBlockIndex = function(program, uniformBlockName) { return 0; };
        WebGL2RenderingContext.prototype.uniformBlockBinding = function(program, uniformBlockIndex, uniformBlockBinding) {};

        WebGL2RenderingContext.prototype.drawArraysInstanced = function(mode, first, count, instanceCount) {};
        WebGL2RenderingContext.prototype.drawElementsInstanced = function(mode, count, type, offset, instanceCount) {};
        WebGL2RenderingContext.prototype.vertexAttribDivisor = function(index, divisor) {};

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
        var GPUBufferUsage = {
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
        };
        globalThis.GPUBufferUsage = GPUBufferUsage;

        var GPUTextureUsage = {
            COPY_SRC: 0x01,
            COPY_DST: 0x02,
            TEXTURE_BINDING: 0x04,
            STORAGE_BINDING: 0x08,
            RENDER_ATTACHMENT: 0x10
        };
        globalThis.GPUTextureUsage = GPUTextureUsage;

        var GPUShaderStage = {
            VERTEX: 0x1,
            FRAGMENT: 0x2,
            COMPUTE: 0x4
        };
        globalThis.GPUShaderStage = GPUShaderStage;

        var GPUMapMode = {
            READ: 0x0001,
            WRITE: 0x0002
        };
        globalThis.GPUMapMode = GPUMapMode;

        function GPUBuffer(descriptor) {
            this.size = (descriptor && descriptor.size) || 0;
            this.usage = (descriptor && descriptor.usage) || 0;
            this.mapState = 'unmapped';
        }
        GPUBuffer.prototype.mapAsync = function(mode, offset, size) {
            this.mapState = 'mapped';
            return Promise.resolve();
        };
        GPUBuffer.prototype.getMappedRange = function(offset, size) {
            return new ArrayBuffer(size || this.size || 64);
        };
        GPUBuffer.prototype.unmap = function() { this.mapState = 'unmapped'; };
        GPUBuffer.prototype.destroy = function() { this.mapState = 'destroyed'; };

        function GPUTextureView(descriptor) {
            this.descriptor = descriptor || {};
        }
        globalThis.GPUTextureView = GPUTextureView;

        function GPUTexture(descriptor) {
            this.width = (descriptor && descriptor.size && descriptor.size.width) || 1;
            this.height = (descriptor && descriptor.size && descriptor.size.height) || 1;
            this.format = (descriptor && descriptor.format) || 'rgba8unorm';
        }
        GPUTexture.prototype.createView = function(descriptor) { return new GPUTextureView(descriptor); };
        GPUTexture.prototype.destroy = function() {};

        function GPUQueue() {}
        GPUQueue.prototype.submit = function(commandBuffers) {};
        GPUQueue.prototype.writeBuffer = function(buffer, bufferOffset, data, dataOffset, size) {};
        GPUQueue.prototype.writeTexture = function(destination, data, dataLayout, size) {};

        function GPUCommandBuffer() {}
        function GPURenderPassEncoder() {}
        GPURenderPassEncoder.prototype.setPipeline = function(pipeline) {};
        GPURenderPassEncoder.prototype.setVertexBuffer = function(slot, buffer) {};
        GPURenderPassEncoder.prototype.setIndexBuffer = function(buffer, indexFormat, offset, size) {};
        GPURenderPassEncoder.prototype.setBindGroup = function(index, bindGroup) {};
        GPURenderPassEncoder.prototype.draw = function(vertexCount, instanceCount, firstVertex, firstInstance) {};
        GPURenderPassEncoder.prototype.drawIndexed = function(indexCount, instanceCount, firstIndex, baseVertex, firstInstance) {};
        GPURenderPassEncoder.prototype.end = function() {};

        function GPUComputePassEncoder() {}
        GPUComputePassEncoder.prototype.setPipeline = function(pipeline) {};
        GPUComputePassEncoder.prototype.setBindGroup = function(index, bindGroup) {};
        GPUComputePassEncoder.prototype.dispatchWorkgroups = function(workgroupCountX, workgroupCountY, workgroupCountZ) {};
        GPUComputePassEncoder.prototype.end = function() {};

        function GPUCommandEncoder() {}
        GPUCommandEncoder.prototype.beginRenderPass = function(descriptor) { return new GPURenderPassEncoder(); };
        GPUCommandEncoder.prototype.beginComputePass = function(descriptor) { return new GPUComputePassEncoder(); };
        GPUCommandEncoder.prototype.finish = function() { return new GPUCommandBuffer(); };
        GPUCommandEncoder.prototype.copyBufferToBuffer = function(source, sourceOffset, destination, destinationOffset, size) {};
        GPUCommandEncoder.prototype.copyBufferToTexture = function(source, destination, copySize) {};
        GPUCommandEncoder.prototype.copyTextureToBuffer = function(source, destination, copySize) {};
        GPUCommandEncoder.prototype.copyTextureToTexture = function(source, destination, copySize) {};

        function GPUShaderModule(descriptor) {
            this.code = (descriptor && descriptor.code) || '';
        }
        GPUShaderModule.prototype.getCompilationInfo = function() {
            return Promise.resolve({ messages: [] });
        };

        function GPUBindGroupLayout(descriptor) {
            this.entries = (descriptor && descriptor.entries) || [];
        }

        function GPUBindGroup(descriptor) {
            this.layout = descriptor && descriptor.layout;
            this.entries = (descriptor && descriptor.entries) || [];
        }

        function GPUPipelineLayout(descriptor) {
            this.bindGroupLayouts = (descriptor && descriptor.bindGroupLayouts) || [];
        }

        function GPURenderPipeline(descriptor) {
            this.descriptor = descriptor;
            this.layout = descriptor && descriptor.layout;
        }
        GPURenderPipeline.prototype.getBindGroupLayout = function(index) {
            return new GPUBindGroupLayout({});
        };

        function GPUComputePipeline(descriptor) {
            this.descriptor = descriptor;
            this.layout = descriptor && descriptor.layout;
        }
        GPUComputePipeline.prototype.getBindGroupLayout = function(index) {
            return new GPUBindGroupLayout({});
        };

        function GPUDevice(adapter) {
            this.adapter = adapter;
            this.queue = new GPUQueue();
            this.features = new Set();
            this.limits = {
                maxTextureDimension2D: 8192,
                maxBufferSize: 268435456,
                maxStorageBufferBindingSize: 134217728,
                maxComputeWorkgroupSizeX: 256,
                maxComputeWorkgroupSizeY: 256,
                maxComputeWorkgroupSizeZ: 64
            };
        }
        GPUDevice.prototype.createBuffer = function(descriptor) { return new GPUBuffer(descriptor); };
        GPUDevice.prototype.createTexture = function(descriptor) { return new GPUTexture(descriptor); };
        GPUDevice.prototype.createSampler = function(descriptor) { return {}; };
        GPUDevice.prototype.createShaderModule = function(descriptor) { return new GPUShaderModule(descriptor); };
        GPUDevice.prototype.createBindGroupLayout = function(descriptor) { return new GPUBindGroupLayout(descriptor); };
        GPUDevice.prototype.createBindGroup = function(descriptor) { return new GPUBindGroup(descriptor); };
        GPUDevice.prototype.createPipelineLayout = function(descriptor) { return new GPUPipelineLayout(descriptor); };
        GPUDevice.prototype.createRenderPipeline = function(descriptor) { return new GPURenderPipeline(descriptor); };
        GPUDevice.prototype.createComputePipeline = function(descriptor) { return new GPUComputePipeline(descriptor); };
        GPUDevice.prototype.createRenderPipelineAsync = function(descriptor) { return Promise.resolve(new GPURenderPipeline(descriptor)); };
        GPUDevice.prototype.createComputePipelineAsync = function(descriptor) { return Promise.resolve(new GPUComputePipeline(descriptor)); };
        GPUDevice.prototype.createCommandEncoder = function(descriptor) { return new GPUCommandEncoder(); };
        GPUDevice.prototype.destroy = function() {};

        function GPUAdapter() {
            this.features = new Set();
            this.limits = { maxTextureDimension2D: 8192, maxBufferSize: 268435456 };
            this.isFallbackAdapter = false;
        }
        GPUAdapter.prototype.requestDevice = function(descriptor) {
            return Promise.resolve(new GPUDevice(this));
        };

        function GPU() {}
        GPU.prototype.requestAdapter = function(options) {
            return Promise.resolve(new GPUAdapter());
        };
        GPU.prototype.getPreferredCanvasFormat = function() {
            return 'bgra8unorm';
        };

        function GPUCanvasContext(canvas) {
            this.canvas = canvas;
            this._configured = false;
        }
        GPUCanvasContext.prototype.configure = function(config) {
            this._config = config;
            this._configured = true;
        };
        GPUCanvasContext.prototype.unconfigure = function() {
            this._configured = false;
        };
        GPUCanvasContext.prototype.getCurrentTexture = function() {
            var w = this.canvas ? (this.canvas.width || 300) : 300;
            var h = this.canvas ? (this.canvas.height || 150) : 150;
            return new GPUTexture({ size: { width: w, height: h }, format: (this._config && this._config.format) || 'bgra8unorm' });
        };

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

        if (typeof navigator !== 'undefined') {
            navigator.gpu = new GPU();
        }
    })();

