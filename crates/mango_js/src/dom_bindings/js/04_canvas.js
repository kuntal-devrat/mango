    // ── Path2D & Canvas 2D Level 2 ───────────────────────────────────────────────
    function _parseSvgPath(path, d) {
        var str = String(d || '').trim();
        if (!str) return;
        var re = /([a-df-z])([^a-df-z]*)/gi;
        var curX = 0, curY = 0, startX = 0, startY = 0;
        var match;
        while ((match = re.exec(str)) !== null) {
            var cmd = match[1];
            var argsStr = match[2].trim();
            var numRe = /[-+]?(?:\d*\.\d+|\d+)(?:[eE][-+]?\d+)?/g;
            var nums = [];
            var numMatch;
            while ((numMatch = numRe.exec(argsStr)) !== null) {
                nums.push(parseFloat(numMatch[0]));
            }
            var isRel = (cmd === cmd.toLowerCase());
            var c = cmd.toUpperCase();
            var i = 0;
            switch(c) {
                case 'M':
                    while (i + 1 < nums.length || (i === 0 && nums.length >= 2)) {
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        startX = x; startY = y;
                        path.moveTo(x, y);
                        c = 'L';
                    }
                    break;
                case 'L':
                    while (i + 1 < nums.length) {
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        path.lineTo(x, y);
                    }
                    break;
                case 'H':
                    while (i < nums.length) {
                        var x = nums[i++];
                        if (isRel) x += curX;
                        curX = x;
                        path.lineTo(curX, curY);
                    }
                    break;
                case 'V':
                    while (i < nums.length) {
                        var y = nums[i++];
                        if (isRel) y += curY;
                        curY = y;
                        path.lineTo(curX, curY);
                    }
                    break;
                case 'C':
                    while (i + 5 < nums.length) {
                        var x1 = nums[i++], y1 = nums[i++];
                        var x2 = nums[i++], y2 = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) {
                            x1 += curX; y1 += curY;
                            x2 += curX; y2 += curY;
                            x += curX; y += curY;
                        }
                        curX = x; curY = y;
                        path.bezierCurveTo(x1, y1, x2, y2, x, y);
                    }
                    break;
                case 'S':
                    while (i + 3 < nums.length) {
                        var x2 = nums[i++], y2 = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x2 += curX; y2 += curY; x += curX; y += curY; }
                        var x1 = curX, y1 = curY;
                        curX = x; curY = y;
                        path.bezierCurveTo(x1, y1, x2, y2, x, y);
                    }
                    break;
                case 'Q':
                    while (i + 3 < nums.length) {
                        var cx = nums[i++], cy = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { cx += curX; cy += curY; x += curX; y += curY; }
                        curX = x; curY = y;
                        path.quadraticCurveTo(cx, cy, x, y);
                    }
                    break;
                case 'T':
                    while (i + 1 < nums.length) {
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        path.quadraticCurveTo(curX, curY, x, y);
                    }
                    break;
                case 'A':
                    while (i + 6 < nums.length) {
                        var rx = nums[i++], ry = nums[i++];
                        var rot = nums[i++], largeArc = nums[i++], sweep = nums[i++];
                        var x = nums[i++], y = nums[i++];
                        if (isRel) { x += curX; y += curY; }
                        curX = x; curY = y;
                        path.arcTo(curX, curY, x, y, Math.max(rx, ry));
                    }
                    break;
                case 'Z':
                    path.closePath();
                    curX = startX; curY = startY;
                    break;
            }
        }
    }

    function Path2D(pathOrSvg) {
        this._isPath2D = true;
        this._ops = [];
        if (pathOrSvg instanceof Path2D) {
            this._ops = pathOrSvg._ops.map(function(op) { return op.slice(); });
        } else if (typeof pathOrSvg === 'string') {
            _parseSvgPath(this, pathOrSvg);
        }
    }
    Path2D.prototype = {
        addPath: function(path, transform) {
            if (path && path._ops) {
                for (var i = 0; i < path._ops.length; i++) {
                    this._ops.push(path._ops[i].slice());
                }
            }
        },
        closePath: function() { this._ops.push(['closePath']); },
        moveTo: function(x, y) { this._ops.push(['moveTo', Number(x)||0, Number(y)||0]); },
        lineTo: function(x, y) { this._ops.push(['lineTo', Number(x)||0, Number(y)||0]); },
        bezierCurveTo: function(cp1x, cp1y, cp2x, cp2y, x, y) {
            this._ops.push(['bezierCurveTo', Number(cp1x)||0, Number(cp1y)||0, Number(cp2x)||0, Number(cp2y)||0, Number(x)||0, Number(y)||0]);
        },
        quadraticCurveTo: function(cpx, cpy, x, y) {
            this._ops.push(['quadraticCurveTo', Number(cpx)||0, Number(cpy)||0, Number(x)||0, Number(y)||0]);
        },
        arc: function(x, y, radius, startAngle, endAngle, counterclockwise) {
            this._ops.push(['arc', Number(x)||0, Number(y)||0, Number(radius)||0, Number(startAngle)||0, Number(endAngle)||0, !!counterclockwise]);
        },
        arcTo: function(x1, y1, x2, y2, radius) {
            this._ops.push(['arcTo', Number(x1)||0, Number(y1)||0, Number(x2)||0, Number(y2)||0, Number(radius)||0]);
        },
        ellipse: function(x, y, radiusX, radiusY, rotation, startAngle, endAngle, counterclockwise) {
            this._ops.push(['arc', Number(x)||0, Number(y)||0, Number(radiusX)||0, Number(startAngle)||0, Number(endAngle)||0, !!counterclockwise]);
        },
        rect: function(x, y, w, h) {
            this._ops.push(['rect', Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0]);
        },
        roundRect: function(x, y, w, h, radii) {
            this._ops.push(['rect', Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0]);
        }
    };
    globalThis.Path2D = Path2D;

    var _nextOffscreenId = 0x80000000;
    var _offscreenFinalizer = (typeof FinalizationRegistry === 'function')
        ? new FinalizationRegistry(function(id) {
            try { _canvasRemove(id); } catch (e) {}
        })
        : null;
    function ImageBitmap(canvasOrWidth, height) {
        if (canvasOrWidth && canvasOrWidth.width !== undefined) {
            this.width = canvasOrWidth.width;
            this.height = canvasOrWidth.height;
            this._nodeId = canvasOrWidth._nodeId;
        } else {
            this.width = Number(canvasOrWidth) || 0;
            this.height = Number(height) || 0;
        }
    }
    ImageBitmap.prototype.close = function() {
        this.width = 0;
        this.height = 0;
    };
    globalThis.ImageBitmap = ImageBitmap;

    function OffscreenCanvas(width, height) {
        this.width = Math.max(0, Math.floor(Number(width) || 300));
        this.height = Math.max(0, Math.floor(Number(height) || 150));
        this._nodeId = _nextOffscreenId++;
        this._context2d = null;
        if (_offscreenFinalizer) _offscreenFinalizer.register(this, this._nodeId);
    }
    OffscreenCanvas.prototype = {
        getContext: function(type, options) {
            if (type === '2d') {
                if (!this._context2d) {
                    this._context2d = _createCanvasRenderingContext2D(this);
                }
                return this._context2d;
            }
            if (type === 'webgl') {
                if (typeof globalThis.WebGLRenderingContext === 'function') {
                    return new globalThis.WebGLRenderingContext(this);
                }
            }
            if (type === 'webgl2') {
                if (typeof globalThis.WebGL2RenderingContext === 'function') {
                    return new globalThis.WebGL2RenderingContext(this);
                }
            }
            if (type === 'webgpu') {
                if (typeof globalThis.GPUCanvasContext === 'function') {
                    return new globalThis.GPUCanvasContext(this);
                }
            }
            return null;
        },
        transferToImageBitmap: function() {
            return new ImageBitmap(this);
        },
        convertToBlob: function(options) {
            var type = (options && options.type) || 'image/png';
            var dataUrl = _canvasToDataUrl(this._nodeId, type);
            var bCtor = globalThis.Blob || function(p, opts){ return { parts: p, type: opts && opts.type }; };
            return Promise.resolve(new bCtor([dataUrl], { type: type }));
        }
    };
    globalThis.OffscreenCanvas = OffscreenCanvas;

    function _createCanvasRenderingContext2D(canvas) {
        var ctx = {
            _canvas: canvas,
            _fillStyle: '#000000',
            _strokeStyle: '#000000',
            _lineWidth: 1,
            _lineCap: 'butt',
            _lineJoin: 'miter',
            _miterLimit: 10,
            _globalAlpha: 1.0,
            _font: '10px sans-serif',
            _textAlign: 'start',
            _textBaseline: 'alphabetic',
            _filter: 'none',
            shadowBlur: 0,
            shadowColor: 'rgba(0, 0, 0, 0)',
            shadowOffsetX: 0,
            shadowOffsetY: 0,
            globalCompositeOperation: 'source-over',
            imageSmoothingEnabled: true,

            get canvas() { return this._canvas; },

            get filter() { return this._filter || 'none'; },
            set filter(val) { this._filter = String(val || 'none'); },

            get fillStyle() { return this._fillStyle; },
            set fillStyle(val) {
                if (typeof val === 'string') {
                    this._fillStyle = val;
                    _canvasSetFillStyle(this._canvas._nodeId, val);
                } else if (val && typeof val === 'object') {
                    this._fillStyle = val;
                }
            },

            get strokeStyle() { return this._strokeStyle; },
            set strokeStyle(val) {
                if (typeof val === 'string') {
                    this._strokeStyle = val;
                    _canvasSetStrokeStyle(this._canvas._nodeId, val);
                } else if (val && typeof val === 'object') {
                    this._strokeStyle = val;
                }
            },

            get lineWidth() { return this._lineWidth; },
            set lineWidth(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num > 0) {
                    this._lineWidth = num;
                    _canvasSetLineWidth(this._canvas._nodeId, num);
                }
            },

            get lineCap() { return this._lineCap; },
            set lineCap(val) {
                this._lineCap = String(val);
                _canvasSetLineCap(this._canvas._nodeId, this._lineCap);
            },

            get lineJoin() { return this._lineJoin; },
            set lineJoin(val) {
                this._lineJoin = String(val);
                _canvasSetLineJoin(this._canvas._nodeId, this._lineJoin);
            },

            get miterLimit() { return this._miterLimit; },
            set miterLimit(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num > 0) {
                    this._miterLimit = num;
                    _canvasSetMiterLimit(this._canvas._nodeId, num);
                }
            },

            get globalAlpha() { return this._globalAlpha; },
            set globalAlpha(val) {
                var num = parseFloat(val);
                if (!isNaN(num) && num >= 0 && num <= 1) {
                    this._globalAlpha = num;
                    _canvasSetGlobalAlpha(this._canvas._nodeId, num);
                }
            },

            get font() { return this._font; },
            set font(val) {
                this._font = String(val);
                _canvasSetFont(this._canvas._nodeId, this._font);
            },

            get textAlign() { return this._textAlign; },
            set textAlign(val) {
                this._textAlign = String(val);
                _canvasSetTextAlign(this._canvas._nodeId, this._textAlign);
            },

            get textBaseline() { return this._textBaseline; },
            set textBaseline(val) {
                this._textBaseline = String(val);
                _canvasSetTextBaseline(this._canvas._nodeId, this._textBaseline);
            },

            fillRect: function(x, y, w, h) {
                _canvasFillRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },
            strokeRect: function(x, y, w, h) {
                _canvasStrokeRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },
            clearRect: function(x, y, w, h) {
                _canvasClearRect(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(w)||0, Number(h)||0);
            },

            beginPath: function() {
                _canvasBeginPath(this._canvas._nodeId);
            },
            closePath: function() {
                _canvasClosePath(this._canvas._nodeId);
            },
            moveTo: function(x, y) {
                _canvasMoveTo(this._canvas._nodeId, Number(x)||0, Number(y)||0);
            },
            lineTo: function(x, y) {
                _canvasLineTo(this._canvas._nodeId, Number(x)||0, Number(y)||0);
            },
            rect: function(x, y, w, h) {
                var nx = Number(x) || 0;
                var ny = Number(y) || 0;
                var nw = Number(w) || 0;
                var nh = Number(h) || 0;
                this.moveTo(nx, ny);
                this.lineTo(nx + nw, ny);
                this.lineTo(nx + nw, ny + nh);
                this.lineTo(nx, ny + nh);
                this.closePath();
            },
            arc: function(x, y, radius, startAngle, endAngle, counterclockwise) {
                _canvasArc(this._canvas._nodeId, Number(x)||0, Number(y)||0, Number(radius)||0, Number(startAngle)||0, Number(endAngle)||0, !!counterclockwise);
            },
            arcTo: function(x1, y1, x2, y2, radius) {
                this.lineTo(Number(x1) || 0, Number(y1) || 0);
            },
            bezierCurveTo: function(cp1x, cp1y, cp2x, cp2y, x, y) {
                this.lineTo(Number(x) || 0, Number(y) || 0);
            },
            quadraticCurveTo: function(cpx, cpy, x, y) {
                this.lineTo(Number(x) || 0, Number(y) || 0);
            },

            _applyPath2D: function(path) {
                if (!path || !path._ops) return;
                this.beginPath();
                for (var i = 0; i < path._ops.length; i++) {
                    var op = path._ops[i];
                    var fn = this[op[0]];
                    if (typeof fn === 'function') {
                        fn.apply(this, op.slice(1));
                    }
                }
            },

            fill: function(pathOrRule, rule) {
                var fillRule = 'nonzero';
                if (pathOrRule && pathOrRule._isPath2D) {
                    this._applyPath2D(pathOrRule);
                    fillRule = rule || 'nonzero';
                } else if (typeof pathOrRule === 'string') {
                    fillRule = pathOrRule;
                }
                _canvasFill(this._canvas._nodeId, String(fillRule || 'nonzero'));
            },
            stroke: function(path) {
                if (path && path._isPath2D) {
                    this._applyPath2D(path);
                }
                _canvasStroke(this._canvas._nodeId);
            },
            clip: function(pathOrRule, rule) {
                if (pathOrRule && pathOrRule._isPath2D) {
                    this._applyPath2D(pathOrRule);
                }
            },
            isPointInPath: function(pathOrX, xOrY, yOrRule, rule) { return false; },

            save: function() {
                _canvasSave(this._canvas._nodeId);
            },
            restore: function() {
                _canvasRestore(this._canvas._nodeId);
            },
            scale: function(sx, sy) {
                _canvasScale(this._canvas._nodeId, Number(sx)||1, Number(sy)||1);
            },
            rotate: function(angle) {
                _canvasRotate(this._canvas._nodeId, Number(angle)||0);
            },
            translate: function(dx, dy) {
                _canvasTranslate(this._canvas._nodeId, Number(dx)||0, Number(dy)||0);
            },
            transform: function(a, b, c, d, e, f) {
                _canvasTransform(this._canvas._nodeId, Number(a)||1, Number(b)||0, Number(c)||0, Number(d)||1, Number(e)||0, Number(f)||0);
            },
            setTransform: function(a, b, c, d, e, f) {
                if (arguments.length === 0) {
                    _canvasResetTransform(this._canvas._nodeId);
                } else {
                    _canvasSetTransform(this._canvas._nodeId, Number(a)||1, Number(b)||0, Number(c)||0, Number(d)||1, Number(e)||0, Number(f)||0);
                }
            },
            resetTransform: function() {
                _canvasResetTransform(this._canvas._nodeId);
            },
            getTransform: function() {
                return { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 };
            },

            fillText: function(text, x, y, maxWidth) {
                var mw = maxWidth !== undefined ? Number(maxWidth) : null;
                _canvasFillText(this._canvas._nodeId, String(text), Number(x)||0, Number(y)||0, mw);
            },
            strokeText: function(text, x, y, maxWidth) {
                var mw = maxWidth !== undefined ? Number(maxWidth) : null;
                _canvasStrokeText(this._canvas._nodeId, String(text), Number(x)||0, Number(y)||0, mw);
            },
            measureText: function(text) {
                var str = String(text || '');
                var w = _canvasMeasureText(this._canvas._nodeId, str);
                return {
                    width: w,
                    actualBoundingBoxAscent: 10,
                    actualBoundingBoxDescent: 2,
                    fontBoundingBoxAscent: 10,
                    fontBoundingBoxDescent: 2
                };
            },

            getImageData: function(sx, sy, sw, sh) {
                var w = Math.max(1, Math.floor(sw || 1));
                var h = Math.max(1, Math.floor(sh || 1));
                var arr = _canvasGetImageData(this._canvas._nodeId, Math.floor(sx || 0), Math.floor(sy || 0), w, h);
                var u8 = new Uint8ClampedArray(arr);
                if (typeof globalThis.ImageData === 'function') {
                    return new globalThis.ImageData(u8, w, h);
                }
                return { data: u8, width: w, height: h };
            },
            putImageData: function(imgData, dx, dy, dirtyX, dirtyY, dirtyWidth, dirtyHeight) {
                if (!imgData || !imgData.data) return;
                var w = imgData.width || 0;
                var h = imgData.height || 0;
                var dX = dirtyX !== undefined ? Math.floor(dirtyX) : 0;
                var dY = dirtyY !== undefined ? Math.floor(dirtyY) : 0;
                var dW = dirtyWidth !== undefined ? Math.floor(dirtyWidth) : (w - dX);
                var dH = dirtyHeight !== undefined ? Math.floor(dirtyHeight) : (h - dY);
                var raw = [];
                for (var i = 0; i < imgData.data.length; i++) {
                    raw.push(imgData.data[i]);
                }
                _canvasPutImageData(this._canvas._nodeId, raw, Math.floor(dx || 0), Math.floor(dy || 0), dX, dY, dW, dH, w);
            },
            createImageData: function(w, h) {
                var width = Math.max(1, Math.floor(w || 1));
                var height = Math.max(1, Math.floor(h || 1));
                var u8 = new Uint8ClampedArray(width * height * 4);
                if (typeof globalThis.ImageData === 'function') {
                    return new globalThis.ImageData(u8, width, height);
                }
                return { data: u8, width: width, height: height };
            },
            createLinearGradient: function(x0, y0, x1, y1) {
                return {
                    addColorStop: function(offset, color) {}
                };
            },
            createRadialGradient: function(x0, y0, r0, x1, y1, r1) {
                return {
                    addColorStop: function(offset, color) {}
                };
            },
            createConicGradient: function(startAngle, x, y) {
                var stops = [];
                return {
                    addColorStop: function(offset, color) {
                        stops.push({ offset: Number(offset) || 0, color: String(color) });
                    },
                    _type: 'conic',
                    _startAngle: Number(startAngle) || 0,
                    _x: Number(x) || 0,
                    _y: Number(y) || 0,
                    _stops: stops
                };
            },
            createPattern: function(image, repetition) {
                return {};
            },
            drawImage: function() {}
        };

        // Initialize backing store with the canvas's width and height
        _canvasResize(canvas._nodeId, canvas.width, canvas.height);

        if (typeof globalThis.CanvasRenderingContext2D === 'function') {
            Object.setPrototypeOf(ctx, globalThis.CanvasRenderingContext2D.prototype);
        }
        return ctx;
    }
    globalThis._createCanvasRenderingContext2D = _createCanvasRenderingContext2D;

