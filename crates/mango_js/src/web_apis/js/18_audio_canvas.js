    // ── Web Audio API ──────────────────────────────────────────────────────────
    (function() {
        function AudioNode(context) {
            this.context = context;
            this.numberOfInputs = 1;
            this.numberOfOutputs = 1;
            this.channelCount = 2;
            this._connections = [];
        }
        AudioNode.prototype.connect = function(destination) {
            this._connections.push(destination);
            return destination;
        };
        AudioNode.prototype.disconnect = function() {
            this._connections = [];
        };

        function AudioParam(defaultValue) {
            this.value = defaultValue;
            this.defaultValue = defaultValue;
            this.minValue = -3.4028235e38;
            this.maxValue = 3.4028235e38;
        }
        AudioParam.prototype.setValueAtTime = function(val, startTime) {
            this.value = val;
            return this;
        };
        AudioParam.prototype.linearRampToValueAtTime = function(val, endTime) {
            this.value = val;
            return this;
        };
        AudioParam.prototype.exponentialRampToValueAtTime = function(val, endTime) {
            this.value = val;
            return this;
        };
        AudioParam.prototype.setTargetAtTime = function(target, startTime, timeConstant) {
            this.value = target;
            return this;
        };
        AudioParam.prototype.setValueCurveAtTime = function(values, startTime, duration) {
            if (values && values.length > 0) {
                this.value = values[values.length - 1];
            }
            return this;
        };
        AudioParam.prototype.cancelScheduledValues = function(startTime) {
            return this;
        };
        AudioParam.prototype.cancelAndHoldAtTime = function(cancelTime) {
            return this;
        };

        function GainNode(context) {
            AudioNode.call(this, context);
            this.gain = new AudioParam(1.0);
        }
        GainNode.prototype = Object.create(AudioNode.prototype);

        function OscillatorNode(context) {
            AudioNode.call(this, context);
            this.type = 'sine';
            this.frequency = new AudioParam(440);
            this.detune = new AudioParam(0);
            this._started = false;
            this._stopped = false;
        }
        OscillatorNode.prototype = Object.create(AudioNode.prototype);
        OscillatorNode.prototype.start = function(when) {
            this._started = true;
        };
        OscillatorNode.prototype.stop = function(when) {
            this._stopped = true;
        };

        function AudioBuffer(options) {
            this.numberOfChannels = (options && options.numberOfChannels) || 2;
            this.length = (options && options.length) || 0;
            this.sampleRate = (options && options.sampleRate) || 44100;
            this.duration = this.sampleRate > 0 ? (this.length / this.sampleRate) : 0;
            this._channelData = [];
            for (var i = 0; i < this.numberOfChannels; i++) {
                this._channelData.push(new Float32Array(this.length));
            }
        }
        AudioBuffer.prototype.getChannelData = function(channel) {
            if (channel < 0 || channel >= this._channelData.length) {
                throw new Error("IndexSizeError: channel out of bounds");
            }
            return this._channelData[channel];
        };
        AudioBuffer.prototype.copyFromChannel = function(dest, channelNumber, startInChannel) {
            var src = this.getChannelData(channelNumber);
            var offset = startInChannel || 0;
            dest.set(src.subarray(offset, offset + dest.length));
        };
        AudioBuffer.prototype.copyToChannel = function(src, channelNumber, startInChannel) {
            var dest = this.getChannelData(channelNumber);
            var offset = startInChannel || 0;
            dest.set(src, offset);
        };

        function AudioBufferSourceNode(context) {
            AudioNode.call(this, context);
            this.buffer = null;
            this.playbackRate = new AudioParam(1.0);
            this.detune = new AudioParam(0);
            this.loop = false;
            this.loopStart = 0;
            this.loopEnd = 0;
            this._started = false;
            this._stopped = false;
        }
        AudioBufferSourceNode.prototype = Object.create(AudioNode.prototype);
        AudioBufferSourceNode.prototype.start = function() { this._started = true; };
        AudioBufferSourceNode.prototype.stop = function() { this._stopped = true; };

        function AnalyserNode(context) {
            AudioNode.call(this, context);
            this.fftSize = 2048;
            this.frequencyBinCount = 1024;
            this.minDecibels = -100;
            this.maxDecibels = -30;
            this.smoothingTimeConstant = 0.8;
        }
        AnalyserNode.prototype = Object.create(AudioNode.prototype);
        AnalyserNode.prototype.getByteFrequencyData = function(array) {
            array.fill(0);
        };
        AnalyserNode.prototype.getByteTimeDomainData = function(array) {
            array.fill(128);
        };
        AnalyserNode.prototype.getFloatFrequencyData = function(array) {
            array.fill(-100);
        };
        AnalyserNode.prototype.getFloatTimeDomainData = function(array) {
            array.fill(0);
        };

        function BiquadFilterNode(context) {
            AudioNode.call(this, context);
            this.type = 'lowpass';
            this.frequency = new AudioParam(350);
            this.detune = new AudioParam(0);
            this.Q = new AudioParam(1);
            this.gain = new AudioParam(0);
        }
        BiquadFilterNode.prototype = Object.create(AudioNode.prototype);

        function PannerNode(context, options) {
            AudioNode.call(this, context);
            options = options || {};
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
        }
        PannerNode.prototype = Object.create(AudioNode.prototype);
        PannerNode.prototype.setPosition = function(x, y, z) {
            this.positionX.value = x;
            this.positionY.value = y;
            this.positionZ.value = z;
        };
        PannerNode.prototype.setOrientation = function(x, y, z) {
            this.orientationX.value = x;
            this.orientationY.value = y;
            this.orientationZ.value = z;
        };

        function StereoPannerNode(context, options) {
            AudioNode.call(this, context);
            options = options || {};
            this.pan = new AudioParam(options.pan !== undefined ? options.pan : 0);
        }
        StereoPannerNode.prototype = Object.create(AudioNode.prototype);

        function ConstantSourceNode(context, options) {
            AudioNode.call(this, context);
            options = options || {};
            this.offset = new AudioParam(options.offset !== undefined ? options.offset : 1);
            this._started = false;
            this._stopped = false;
        }
        ConstantSourceNode.prototype = Object.create(AudioNode.prototype);
        ConstantSourceNode.prototype.start = function() { this._started = true; };
        ConstantSourceNode.prototype.stop = function() { this._stopped = true; };

        function AudioListener() {
            this.positionX = new AudioParam(0);
            this.positionY = new AudioParam(0);
            this.positionZ = new AudioParam(0);
            this.forwardX = new AudioParam(0);
            this.forwardY = new AudioParam(0);
            this.forwardZ = new AudioParam(-1);
            this.upX = new AudioParam(0);
            this.upY = new AudioParam(1);
            this.upZ = new AudioParam(0);
        }
        AudioListener.prototype.setPosition = function(x, y, z) {
            this.positionX.value = x;
            this.positionY.value = y;
            this.positionZ.value = z;
        };
        AudioListener.prototype.setOrientation = function(x, y, z, xUp, yUp, zUp) {
            this.forwardX.value = x;
            this.forwardY.value = y;
            this.forwardZ.value = z;
            if (xUp !== undefined) this.upX.value = xUp;
            if (yUp !== undefined) this.upY.value = yUp;
            if (zUp !== undefined) this.upZ.value = zUp;
        };

        function AudioWorklet() {
            this._modules = [];
        }
        AudioWorklet.prototype.addModule = function(moduleURL, options) {
            this._modules.push(moduleURL);
            return Promise.resolve();
        };

        function AudioWorkletNode(context, name, options) {
            AudioNode.call(this, context);
            this.nodeName = name;
            this.parameters = new Map();
            if (options && options.parameterData) {
                for (var k in options.parameterData) {
                    this.parameters.set(k, new AudioParam(options.parameterData[k]));
                }
            }
            var channel = new MessageChannel();
            this.port = channel.port1;
            this._port2 = channel.port2;
        }
        AudioWorkletNode.prototype = Object.create(AudioNode.prototype);

        function AudioDestinationNode(context) {
            AudioNode.call(this, context);
            this.maxChannelCount = 2;
        }
        AudioDestinationNode.prototype = Object.create(AudioNode.prototype);

        function BaseAudioContext() {
            this.state = 'running';
            this.sampleRate = 44100;
            this.currentTime = 0;
            this.destination = new AudioDestinationNode(this);
            this.listener = new AudioListener();
            this.audioWorklet = new AudioWorklet();
        }
        BaseAudioContext.prototype.createGain = function() { return new GainNode(this); };
        BaseAudioContext.prototype.createOscillator = function() { return new OscillatorNode(this); };
        BaseAudioContext.prototype.createBufferSource = function() { return new AudioBufferSourceNode(this); };
        BaseAudioContext.prototype.createBuffer = function(numChannels, length, sampleRate) {
            return new AudioBuffer({ numberOfChannels: numChannels, length: length, sampleRate: sampleRate });
        };
        BaseAudioContext.prototype.createAnalyser = function() { return new AnalyserNode(this); };
        BaseAudioContext.prototype.createBiquadFilter = function() { return new BiquadFilterNode(this); };
        BaseAudioContext.prototype.createPanner = function() { return new PannerNode(this); };
        BaseAudioContext.prototype.createStereoPanner = function() { return new StereoPannerNode(this); };
        BaseAudioContext.prototype.createConstantSource = function() { return new ConstantSourceNode(this); };
        BaseAudioContext.prototype.decodeAudioData = function(audioData, successCb, errorCb) {
            var self = this;
            return new Promise(function(resolve, reject) {
                var buf = new AudioBuffer({ numberOfChannels: 2, length: 1024, sampleRate: self.sampleRate });
                if (typeof successCb === 'function') successCb(buf);
                resolve(buf);
            });
        };
        BaseAudioContext.prototype.resume = function() {
            this.state = 'running';
            return Promise.resolve();
        };
        BaseAudioContext.prototype.suspend = function() {
            this.state = 'suspended';
            return Promise.resolve();
        };
        BaseAudioContext.prototype.close = function() {
            this.state = 'closed';
            return Promise.resolve();
        };

        function AudioContext(options) {
            BaseAudioContext.call(this);
            if (options && options.sampleRate) this.sampleRate = options.sampleRate;
        }
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
    })();

    // ── OffscreenCanvas ────────────────────────────────────────────────────────
    (function() {
        if (typeof globalThis.OffscreenCanvas !== 'undefined') return;
        function OffscreenCanvas(width, height) {
            this.width = Math.max(0, parseInt(width) || 300);
            this.height = Math.max(0, parseInt(height) || 150);
            this._context2d = null;
        }
        OffscreenCanvas.prototype.getContext = function(contextId, options) {
            if (contextId === '2d') {
                if (!this._context2d) {
                    if (typeof _createCanvasRenderingContext2D === 'function') {
                        this._context2d = _createCanvasRenderingContext2D(this);
                    } else {
                        var self = this;
                        this._context2d = {
                            canvas: self,
                            fillStyle: '#000000',
                            strokeStyle: '#000000',
                            lineWidth: 1,
                            fillRect: function() {},
                            strokeRect: function() {},
                            clearRect: function() {},
                            beginPath: function() {},
                            closePath: function() {},
                            moveTo: function() {},
                            lineTo: function() {},
                            arc: function() {},
                            fill: function() {},
                            stroke: function() {},
                            fillText: function() {},
                            strokeText: function() {},
                            measureText: function(txt) { return { width: (txt || '').length * 8 }; },
                            drawImage: function() {},
                            getImageData: function(sx, sy, sw, sh) {
                                return { width: sw, height: sh, data: new Uint8ClampedArray(sw * sh * 4) };
                            },
                            putImageData: function() {},
                            save: function() {},
                            restore: function() {},
                            scale: function() {},
                            rotate: function() {},
                            translate: function() {},
                            transform: function() {},
                            setTransform: function() {},
                            resetTransform: function() {}
                        };
                    }
                }
            } else if (contextId === 'webgl') {
                if (typeof WebGLRenderingContext === 'function') {
                    return new WebGLRenderingContext(this);
                }
            } else if (contextId === 'webgl2') {
                if (typeof WebGL2RenderingContext === 'function') {
                    return new WebGL2RenderingContext(this);
                }
            } else if (contextId === 'webgpu') {
                if (typeof GPUCanvasContext === 'function') {
                    return new GPUCanvasContext(this);
                }
            }
            return null;
        };
        OffscreenCanvas.prototype.convertToBlob = function(options) {
            var type = (options && options.type) || 'image/png';
            return Promise.resolve(new Blob([], { type: type }));
        };
        OffscreenCanvas.prototype.transferToImageBitmap = function() {
            return {
                width: this.width,
                height: this.height,
                close: function() {}
            };
        };
        globalThis.OffscreenCanvas = OffscreenCanvas;

        if (typeof HTMLCanvasElement !== 'undefined') {
            HTMLCanvasElement.prototype.transferControlToOffscreen = function() {
                var off = new OffscreenCanvas(this.width || 300, this.height || 150);
                this._transferred = true;
                return off;
            };
        }
    })();

