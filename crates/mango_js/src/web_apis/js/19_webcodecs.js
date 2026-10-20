    // ── WebCodecs ───────────────────────────────────────────────────
    (function() {
        // VideoFrame
        if (typeof globalThis.VideoFrame === 'undefined') {
            function VideoFrame(source, init) {
                if (!source) throw new TypeError("VideoFrame constructor requires a source");
                this._closed = false;
                this._colorSpace = (init && init.colorSpace) || {
                    primaries: 'bt709',
                    transfer: 'iec61966-2-1',
                    matrix: 'rgb',
                    fullRange: true
                };

                if (source instanceof ArrayBuffer || ArrayBuffer.isView(source)) {
                    if (!init || typeof init.codedWidth !== 'number' || typeof init.codedHeight !== 'number' || !init.format) {
                        throw new TypeError("VideoFrame from buffer requires format, codedWidth, and codedHeight");
                    }
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
                } else if (source instanceof VideoFrame) {
                    if (source._closed) throw new TypeError("Cannot clone a closed VideoFrame");
                    this._format = source._format;
                    this._codedWidth = source._codedWidth;
                    this._codedHeight = source._codedHeight;
                    this._timestamp = (init && typeof init.timestamp === 'number') ? init.timestamp : source._timestamp;
                    this._duration = (init && typeof init.duration === 'number') ? init.duration : source._duration;
                    this._buffer = new Uint8Array(source._buffer);
                } else {
                    var width = (source && source.width) || (init && init.codedWidth) || 300;
                    var height = (source && source.height) || (init && init.codedHeight) || 150;
                    this._format = (init && init.format) || 'RGBA';
                    this._codedWidth = width;
                    this._codedHeight = height;
                    this._timestamp = (init && typeof init.timestamp === 'number') ? init.timestamp : 0;
                    this._duration = (init && typeof init.duration === 'number') ? init.duration : null;
                    var sz = width * height * 4;
                    this._buffer = new Uint8Array(sz);
                    if (source && typeof source.getContext === 'function') {
                        try {
                            var ctx = source.getContext('2d');
                            if (ctx && typeof ctx.getImageData === 'function') {
                                var imgData = ctx.getImageData(0, 0, width, height);
                                if (imgData && imgData.data) {
                                    this._buffer.set(imgData.data);
                                }
                            }
                        } catch(e) {}
                    }
                }

                var vr = (init && init.visibleRect) || { x: 0, y: 0, width: this._codedWidth, height: this._codedHeight };
                this._visibleRect = { x: vr.x || 0, y: vr.y || 0, width: vr.width || this._codedWidth, height: vr.height || this._codedHeight };
                this._displayWidth = (init && init.displayWidth) || this._visibleRect.width;
                this._displayHeight = (init && init.displayHeight) || this._visibleRect.height;
            }

            Object.defineProperty(VideoFrame.prototype, 'format', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._format; } });
            Object.defineProperty(VideoFrame.prototype, 'codedWidth', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._codedWidth; } });
            Object.defineProperty(VideoFrame.prototype, 'codedHeight', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._codedHeight; } });
            Object.defineProperty(VideoFrame.prototype, 'codedRect', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return { x: 0, y: 0, width: this._codedWidth, height: this._codedHeight }; } });
            Object.defineProperty(VideoFrame.prototype, 'visibleRect', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return { x: this._visibleRect.x, y: this._visibleRect.y, width: this._visibleRect.width, height: this._visibleRect.height }; } });
            Object.defineProperty(VideoFrame.prototype, 'displayWidth', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._displayWidth; } });
            Object.defineProperty(VideoFrame.prototype, 'displayHeight', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._displayHeight; } });
            Object.defineProperty(VideoFrame.prototype, 'duration', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._duration; } });
            Object.defineProperty(VideoFrame.prototype, 'timestamp', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._timestamp; } });
            Object.defineProperty(VideoFrame.prototype, 'colorSpace', { get: function() { if (this._closed) throw new TypeError("VideoFrame is closed"); return this._colorSpace; } });

            VideoFrame.prototype.allocationSize = function(options) {
                if (this._closed) throw new TypeError("VideoFrame is closed");
                var rect = (options && options.rect) || this._visibleRect;
                var bpp = (this._format === 'RGBA' || this._format === 'BGRA') ? 4 : 1.5;
                return Math.floor(rect.width * rect.height * bpp);
            };

            VideoFrame.prototype.copyTo = function(destination, options) {
                if (this._closed) throw new TypeError("VideoFrame is closed");
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._buffer.length);
                destBuf.set(this._buffer.subarray(0, toCopy));
                var stride = this._codedWidth * ((this._format === 'RGBA' || this._format === 'BGRA') ? 4 : 1);
                return Promise.resolve([{ offset: 0, stride: stride }]);
            };

            VideoFrame.prototype.clone = function() {
                if (this._closed) throw new TypeError("VideoFrame is closed");
                return new VideoFrame(this);
            };

            VideoFrame.prototype.close = function() {
                this._closed = true;
                this._buffer = null;
            };
            globalThis.VideoFrame = VideoFrame;
        }

        // AudioData
        if (typeof globalThis.AudioData === 'undefined') {
            function AudioData(init) {
                if (!init || !init.format || typeof init.sampleRate !== 'number' || typeof init.numberOfFrames !== 'number' || typeof init.numberOfChannels !== 'number' || !init.data) {
                    throw new TypeError("AudioData constructor requires format, sampleRate, numberOfFrames, numberOfChannels, and data");
                }
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
            }

            Object.defineProperty(AudioData.prototype, 'format', { get: function() { if (this._closed) throw new TypeError("AudioData is closed"); return this._format; } });
            Object.defineProperty(AudioData.prototype, 'sampleRate', { get: function() { if (this._closed) throw new TypeError("AudioData is closed"); return this._sampleRate; } });
            Object.defineProperty(AudioData.prototype, 'numberOfFrames', { get: function() { if (this._closed) throw new TypeError("AudioData is closed"); return this._numberOfFrames; } });
            Object.defineProperty(AudioData.prototype, 'numberOfChannels', { get: function() { if (this._closed) throw new TypeError("AudioData is closed"); return this._numberOfChannels; } });
            Object.defineProperty(AudioData.prototype, 'duration', { get: function() { if (this._closed) throw new TypeError("AudioData is closed"); return this._duration; } });
            Object.defineProperty(AudioData.prototype, 'timestamp', { get: function() { if (this._closed) throw new TypeError("AudioData is closed"); return this._timestamp; } });

            AudioData.prototype.allocationSize = function(options) {
                if (this._closed) throw new TypeError("AudioData is closed");
                var planeIndex = (options && typeof options.planeIndex === 'number') ? options.planeIndex : null;
                var frameCount = (options && typeof options.frameCount === 'number') ? options.frameCount : this._numberOfFrames;
                if (planeIndex !== null) {
                    return frameCount * this._bytesPerSample;
                }
                return frameCount * this._numberOfChannels * this._bytesPerSample;
            };

            AudioData.prototype.copyTo = function(destination, options) {
                if (this._closed) throw new TypeError("AudioData is closed");
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._buffer.length);
                destBuf.set(this._buffer.subarray(0, toCopy));
            };

            AudioData.prototype.clone = function() {
                if (this._closed) throw new TypeError("AudioData is closed");
                return new AudioData({
                    format: this._format,
                    sampleRate: this._sampleRate,
                    numberOfFrames: this._numberOfFrames,
                    numberOfChannels: this._numberOfChannels,
                    timestamp: this._timestamp,
                    data: this._buffer.slice()
                });
            };

            AudioData.prototype.close = function() {
                this._closed = true;
                this._buffer = null;
            };
            globalThis.AudioData = AudioData;
        }

        // EncodedVideoChunk & EncodedAudioChunk
        if (typeof globalThis.EncodedVideoChunk === 'undefined') {
            function EncodedVideoChunk(init) {
                if (!init || !init.type || typeof init.timestamp !== 'number' || !init.data) {
                    throw new TypeError("EncodedVideoChunk requires type, timestamp, and data");
                }
                this.type = init.type;
                this.timestamp = init.timestamp;
                this.duration = typeof init.duration === 'number' ? init.duration : null;
                var u8 = init.data instanceof ArrayBuffer ? new Uint8Array(init.data) : new Uint8Array(init.data.buffer, init.data.byteOffset, init.data.byteLength);
                this._data = new Uint8Array(u8);
                this.byteLength = this._data.byteLength;
            }
            EncodedVideoChunk.prototype.copyTo = function(destination) {
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._data.byteLength);
                destBuf.set(this._data.subarray(0, toCopy));
            };
            globalThis.EncodedVideoChunk = EncodedVideoChunk;
        }

        if (typeof globalThis.EncodedAudioChunk === 'undefined') {
            function EncodedAudioChunk(init) {
                if (!init || !init.type || typeof init.timestamp !== 'number' || !init.data) {
                    throw new TypeError("EncodedAudioChunk requires type, timestamp, and data");
                }
                this.type = init.type;
                this.timestamp = init.timestamp;
                this.duration = typeof init.duration === 'number' ? init.duration : null;
                var u8 = init.data instanceof ArrayBuffer ? new Uint8Array(init.data) : new Uint8Array(init.data.buffer, init.data.byteOffset, init.data.byteLength);
                this._data = new Uint8Array(u8);
                this.byteLength = this._data.byteLength;
            }
            EncodedAudioChunk.prototype.copyTo = function(destination) {
                var destBuf = destination instanceof ArrayBuffer ? new Uint8Array(destination) : new Uint8Array(destination.buffer, destination.byteOffset, destination.byteLength);
                var toCopy = Math.min(destBuf.length, this._data.byteLength);
                destBuf.set(this._data.subarray(0, toCopy));
            };
            globalThis.EncodedAudioChunk = EncodedAudioChunk;
        }

        // VideoDecoder
        if (typeof globalThis.VideoDecoder === 'undefined') {
            function VideoDecoder(init) {
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {
                    throw new TypeError("VideoDecoder requires output and error callbacks");
                }
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.decodeQueueSize = 0;
                this._config = null;
            }
            VideoDecoder.isConfigSupported = function(config) {
                return Promise.resolve({
                    supported: !!(config && config.codec),
                    config: config
                });
            };
            VideoDecoder.prototype.configure = function(config) {
                if (this.state === 'closed') throw new TypeError("VideoDecoder is closed");
                if (!config || !config.codec) throw new TypeError("VideoDecoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            };
            VideoDecoder.prototype.decode = function(chunk) {
                if (this.state !== 'configured') throw new TypeError("VideoDecoder must be configured before decode");
                if (!(chunk instanceof EncodedVideoChunk)) throw new TypeError("decode expects an EncodedVideoChunk");
                this.decodeQueueSize++;
                var self = this;
                Promise.resolve().then(function() {
                    if (self.state === 'closed') return;
                    self.decodeQueueSize = Math.max(0, self.decodeQueueSize - 1);
                    var width = (self._config && self._config.codedWidth) || 640;
                    var height = (self._config && self._config.codedHeight) || 480;
                    var frame = new VideoFrame(new Uint8Array(width * height * 4), {
                        format: 'RGBA',
                        codedWidth: width,
                        codedHeight: height,
                        timestamp: chunk.timestamp,
                        duration: chunk.duration || 33333
                    });
                    try {
                        self._output(frame);
                    } catch(e) {
                        self._error(e);
                    }
                });
            };
            VideoDecoder.prototype.flush = function() {
                if (this.state !== 'configured') return Promise.reject(new TypeError("VideoDecoder not configured"));
                var self = this;
                return new Promise(function(resolve) {
                    Promise.resolve().then(function() {
                        self.decodeQueueSize = 0;
                        resolve();
                    });
                });
            };
            VideoDecoder.prototype.reset = function() {
                if (this.state === 'closed') throw new TypeError("VideoDecoder is closed");
                this.decodeQueueSize = 0;
            };
            VideoDecoder.prototype.close = function() {
                this.state = 'closed';
                this.decodeQueueSize = 0;
            };
            globalThis.VideoDecoder = VideoDecoder;
        }

        // AudioDecoder
        if (typeof globalThis.AudioDecoder === 'undefined') {
            function AudioDecoder(init) {
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {
                    throw new TypeError("AudioDecoder requires output and error callbacks");
                }
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.decodeQueueSize = 0;
                this._config = null;
            }
            AudioDecoder.isConfigSupported = function(config) {
                return Promise.resolve({
                    supported: !!(config && config.codec),
                    config: config
                });
            };
            AudioDecoder.prototype.configure = function(config) {
                if (this.state === 'closed') throw new TypeError("AudioDecoder is closed");
                if (!config || !config.codec) throw new TypeError("AudioDecoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            };
            AudioDecoder.prototype.decode = function(chunk) {
                if (this.state !== 'configured') throw new TypeError("AudioDecoder must be configured before decode");
                if (!(chunk instanceof EncodedAudioChunk)) throw new TypeError("decode expects an EncodedAudioChunk");
                this.decodeQueueSize++;
                var self = this;
                Promise.resolve().then(function() {
                    if (self.state === 'closed') return;
                    self.decodeQueueSize = Math.max(0, self.decodeQueueSize - 1);
                    var sampleRate = (self._config && self._config.sampleRate) || 48000;
                    var channels = (self._config && self._config.numberOfChannels) || 2;
                    var frames = 1024;
                    var data = new Float32Array(frames * channels);
                    var audioData = new AudioData({
                        format: 'f32',
                        sampleRate: sampleRate,
                        numberOfFrames: frames,
                        numberOfChannels: channels,
                        timestamp: chunk.timestamp,
                        data: data.buffer
                    });
                    try {
                        self._output(audioData);
                    } catch(e) {
                        self._error(e);
                    }
                });
            };
            AudioDecoder.prototype.flush = function() {
                if (this.state !== 'configured') return Promise.reject(new TypeError("AudioDecoder not configured"));
                var self = this;
                return new Promise(function(resolve) {
                    Promise.resolve().then(function() {
                        self.decodeQueueSize = 0;
                        resolve();
                    });
                });
            };
            AudioDecoder.prototype.reset = function() {
                if (this.state === 'closed') throw new TypeError("AudioDecoder is closed");
                this.decodeQueueSize = 0;
            };
            AudioDecoder.prototype.close = function() {
                this.state = 'closed';
                this.decodeQueueSize = 0;
            };
            globalThis.AudioDecoder = AudioDecoder;
        }

        // VideoEncoder
        if (typeof globalThis.VideoEncoder === 'undefined') {
            function VideoEncoder(init) {
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {
                    throw new TypeError("VideoEncoder requires output and error callbacks");
                }
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.encodeQueueSize = 0;
                this._config = null;
            }
            VideoEncoder.isConfigSupported = function(config) {
                return Promise.resolve({
                    supported: !!(config && config.codec),
                    config: config
                });
            };
            VideoEncoder.prototype.configure = function(config) {
                if (this.state === 'closed') throw new TypeError("VideoEncoder is closed");
                if (!config || !config.codec) throw new TypeError("VideoEncoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            };
            VideoEncoder.prototype.encode = function(frame, options) {
                if (this.state !== 'configured') throw new TypeError("VideoEncoder must be configured before encode");
                if (!(frame instanceof VideoFrame)) throw new TypeError("encode expects a VideoFrame");
                this.encodeQueueSize++;
                var isKey = (options && options.keyFrame) || false;
                var self = this;
                var ts = frame.timestamp;
                var dur = frame.duration;
                Promise.resolve().then(function() {
                    if (self.state === 'closed') return;
                    self.encodeQueueSize = Math.max(0, self.encodeQueueSize - 1);
                    var dummyBytes = new Uint8Array([0x00, 0x00, 0x00, 0x01, isKey ? 0x67 : 0x41]);
                    var chunk = new EncodedVideoChunk({
                        type: isKey ? 'key' : 'delta',
                        timestamp: ts,
                        duration: dur,
                        data: dummyBytes.buffer
                    });
                    try {
                        self._output(chunk, { decoderConfig: self._config });
                    } catch(e) {
                        self._error(e);
                    }
                });
            };
            VideoEncoder.prototype.flush = function() {
                if (this.state !== 'configured') return Promise.reject(new TypeError("VideoEncoder not configured"));
                var self = this;
                return new Promise(function(resolve) {
                    Promise.resolve().then(function() {
                        self.encodeQueueSize = 0;
                        resolve();
                    });
                });
            };
            VideoEncoder.prototype.reset = function() {
                if (this.state === 'closed') throw new TypeError("VideoEncoder is closed");
                this.encodeQueueSize = 0;
            };
            VideoEncoder.prototype.close = function() {
                this.state = 'closed';
                this.encodeQueueSize = 0;
            };
            globalThis.VideoEncoder = VideoEncoder;
        }

        // AudioEncoder
        if (typeof globalThis.AudioEncoder === 'undefined') {
            function AudioEncoder(init) {
                if (!init || typeof init.output !== 'function' || typeof init.error !== 'function') {
                    throw new TypeError("AudioEncoder requires output and error callbacks");
                }
                this._output = init.output;
                this._error = init.error;
                this.state = 'unconfigured';
                this.encodeQueueSize = 0;
                this._config = null;
            }
            AudioEncoder.isConfigSupported = function(config) {
                return Promise.resolve({
                    supported: !!(config && config.codec),
                    config: config
                });
            };
            AudioEncoder.prototype.configure = function(config) {
                if (this.state === 'closed') throw new TypeError("AudioEncoder is closed");
                if (!config || !config.codec) throw new TypeError("AudioEncoder configure requires valid config with codec");
                this._config = config;
                this.state = 'configured';
            };
            AudioEncoder.prototype.encode = function(data) {
                if (this.state !== 'configured') throw new TypeError("AudioEncoder must be configured before encode");
                if (!(data instanceof AudioData)) throw new TypeError("encode expects AudioData");
                this.encodeQueueSize++;
                var self = this;
                var ts = data.timestamp;
                var dur = data.duration;
                Promise.resolve().then(function() {
                    if (self.state === 'closed') return;
                    self.encodeQueueSize = Math.max(0, self.encodeQueueSize - 1);
                    var dummyBytes = new Uint8Array([0xFF, 0xF1, 0x50, 0x80]);
                    var chunk = new EncodedAudioChunk({
                        type: 'key',
                        timestamp: ts,
                        duration: dur,
                        data: dummyBytes.buffer
                    });
                    try {
                        self._output(chunk, { decoderConfig: self._config });
                    } catch(e) {
                        self._error(e);
                    }
                });
            };
            AudioEncoder.prototype.flush = function() {
                if (this.state !== 'configured') return Promise.reject(new TypeError("AudioEncoder not configured"));
                var self = this;
                return new Promise(function(resolve) {
                    Promise.resolve().then(function() {
                        self.encodeQueueSize = 0;
                        resolve();
                    });
                });
            };
            AudioEncoder.prototype.reset = function() {
                if (this.state === 'closed') throw new TypeError("AudioEncoder is closed");
                this.encodeQueueSize = 0;
            };
            AudioEncoder.prototype.close = function() {
                this.state = 'closed';
                this.encodeQueueSize = 0;
            };
            globalThis.AudioEncoder = AudioEncoder;
        }

        // ── MediaSource & SourceBuffer (MSE) ─────────────────────────
        if (typeof globalThis.MediaSource === 'undefined') {
            function SourceBuffer(type) {
                if (typeof EventTarget === 'function') EventTarget.call(this);
                this.mode = 'segments';
                this.updating = false;
                this.buffered = {
                    length: 1,
                    start: function(i) { return 0; },
                    end: function(i) { return 1000; }
                };
                this.timestampOffset = 0;
                this.audioTracks = [];
                this.videoTracks = [];
                this.appendWindowStart = 0;
                this.appendWindowEnd = Infinity;
                this.onupdatestart = null;
                this.onupdate = null;
                this.onupdateend = null;
                this.onerror = null;
                this.onabort = null;
            }
            if (typeof EventTarget === 'function') {
                SourceBuffer.prototype = Object.create(EventTarget.prototype);
                SourceBuffer.prototype.constructor = SourceBuffer;
            }
            SourceBuffer.prototype.appendBuffer = function(data) {
                var self = this;
                this.updating = true;
                if (typeof self.onupdatestart === 'function') try { self.onupdatestart(); } catch(e) {}
                setTimeout(function() {
                    self.updating = false;
                    if (typeof self.onupdate === 'function') try { self.onupdate(); } catch(e) {}
                    if (typeof self.onupdateend === 'function') try { self.onupdateend(); } catch(e) {}
                }, 0);
            };
            SourceBuffer.prototype.abort = function() {
                this.updating = false;
                if (typeof this.onabort === 'function') try { this.onabort(); } catch(e) {}
            };
            SourceBuffer.prototype.remove = function(start, end) {
                var self = this;
                this.updating = true;
                setTimeout(function() {
                    self.updating = false;
                    if (typeof self.onupdateend === 'function') try { self.onupdateend(); } catch(e) {}
                }, 0);
            };
            globalThis.SourceBuffer = SourceBuffer;

            function SourceBufferList() {
                var list = [];
                list.__proto__ = SourceBufferList.prototype;
                return list;
            }
            SourceBufferList.prototype = Object.create(Array.prototype);
            SourceBufferList.prototype.constructor = SourceBufferList;
            globalThis.SourceBufferList = SourceBufferList;

            function MediaSource() {
                if (typeof EventTarget === 'function') EventTarget.call(this);
                this.sourceBuffers = new SourceBufferList();
                this.activeSourceBuffers = new SourceBufferList();
                this.readyState = 'closed';
                this.duration = NaN;
                this.onsourceopen = null;
                this.onsourceended = null;
                this.onsourceclose = null;

                var self = this;
                setTimeout(function() {
                    self.readyState = 'open';
                    var evt = { type: 'sourceopen', target: self };
                    if (typeof self.onsourceopen === 'function') try { self.onsourceopen(evt); } catch(e) {}
                    if (self.dispatchEvent) self.dispatchEvent(evt);
                }, 0);
            }
            if (typeof EventTarget === 'function') {
                MediaSource.prototype = Object.create(EventTarget.prototype);
                MediaSource.prototype.constructor = MediaSource;
            }
            MediaSource.isTypeSupported = function(type) {
                if (!type) return false;
                var t = String(type).toLowerCase();
                return t.indexOf('mp4') !== -1 || t.indexOf('webm') !== -1 || t.indexOf('aac') !== -1 || t.indexOf('mp3') !== -1;
            };
            MediaSource.prototype.addSourceBuffer = function(type) {
                var sb = new SourceBuffer(type);
                this.sourceBuffers.push(sb);
                this.activeSourceBuffers.push(sb);
                return sb;
            };
            MediaSource.prototype.removeSourceBuffer = function(sb) {
                var idx = this.sourceBuffers.indexOf(sb);
                if (idx !== -1) this.sourceBuffers.splice(idx, 1);
                var aIdx = this.activeSourceBuffers.indexOf(sb);
                if (aIdx !== -1) this.activeSourceBuffers.splice(aIdx, 1);
            };
            MediaSource.prototype.endOfStream = function(error) {
                this.readyState = 'ended';
                var evt = { type: 'sourceended', target: this };
                if (typeof this.onsourceended === 'function') try { this.onsourceended(evt); } catch(e) {}
                if (this.dispatchEvent) this.dispatchEvent(evt);
            };
            MediaSource.prototype.setLiveSeekableRange = function(start, end) {};
            MediaSource.prototype.clearLiveSeekableRange = function() {};

            globalThis.MediaSource = MediaSource;
            if (typeof window !== 'undefined') {
                window.MediaSource = MediaSource;
            }
        }
    })();

