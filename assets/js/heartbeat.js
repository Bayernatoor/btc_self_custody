// Block Heartbeat — EKG canvas animation engine
// Draws a hospital cardiac monitor sweep line driven by real Bitcoin block data.
// Each block arrival produces a PQRST waveform spike. The flatline between
// beats represents the real wait for the next block.
(function() {
    'use strict';

    var _hb = null; // heartbeat state, null when inactive

    // ── Color palette ──────────────────────────────────────────
    var COLORS = {
        healthy:  '#00e676',
        calm:     '#42a5f5',
        elevated: '#f7931a',
        stressed: '#ff5722',
        critical: '#f44336'
    };

    var BG_COLOR = 'rgba(13, 33, 55, 1)'; // matches bg-[#0d2137]
    var BG_FADE  = 'rgba(13, 33, 55, 0.06)';
    var GRID_COLOR = 'rgba(255, 255, 255, 0.03)';

    // ── PQRST waveform generation ──────────────────────────────
    // Converts block data into an array of y-offset points (from baseline).
    // Negative y = upward on canvas (screen coords).
    function generatePQRST(block) {
        var points = [];
        var txCount = block.tx_count || 1;
        var fees = block.total_fees || 0;
        var interBlock = block.inter_block_seconds || 600;
        var weight = block.weight || 0;

        // Normalize inputs (0-1 range)
        var txNorm = Math.min(txCount / 6000, 1.0);
        var feeNorm = Math.min(fees / 5000000, 1.0);    // 0.05 BTC — typical range 0.003-0.03
        var waitNorm = Math.min(interBlock / 1800, 1.0); // 30 min cap
        var fullNorm = Math.min(weight / 4000000, 1.0);

        // Use log scale for fees to amplify differences in normal range
        var feeLog = fees > 0 ? Math.min(Math.log10(fees) / 7.7, 1.0) : 0; // log10(50M)=7.7

        // Waveform amplitudes (pixels)
        var pAmp   = 4 + txNorm * 20;           // P wave: 4-24px
        var qDepth = 6 + waitNorm * 45;          // Q dip: 6-51px
        var rHeight = 20 + feeLog * 130;          // R spike: 20-150px (log scale)
        var sDepth = rHeight * 0.45;              // S dip: 45% of R
        var tAmp   = 3 + fullNorm * 22;           // T wave: 3-25px

        // Baseline flat lead-in (2-4 points)
        points.push(0, 0, 0);

        // P wave (small upward bump)
        var pSteps = 8;
        for (var i = 0; i <= pSteps; i++) {
            var t = i / pSteps;
            points.push(-pAmp * Math.sin(t * Math.PI));
        }

        // Brief return to baseline
        points.push(0, 0);

        // Q dip (downward)
        var qSteps = 4;
        for (var i = 1; i <= qSteps; i++) {
            points.push(qDepth * (i / qSteps));
        }

        // R spike (sharp upward)
        var rSteps = 6;
        for (var i = 0; i <= rSteps; i++) {
            var t = i / rSteps;
            // Q bottom to R top
            var val = qDepth - (qDepth + rHeight) * t;
            points.push(val);
        }

        // S dip (sharp downward from R peak)
        var sSteps = 5;
        for (var i = 1; i <= sSteps; i++) {
            var t = i / sSteps;
            points.push(-rHeight + (rHeight + sDepth) * t);
        }

        // Return from S to baseline
        var retSteps = 4;
        for (var i = 1; i <= retSteps; i++) {
            var t = i / retSteps;
            points.push(sDepth * (1 - t));
        }

        // Brief baseline
        points.push(0, 0);

        // T wave (gentle upward bump)
        var tSteps = 10;
        for (var i = 0; i <= tSteps; i++) {
            var t = i / tSteps;
            points.push(-tAmp * Math.sin(t * Math.PI));
        }

        // Trailing baseline
        points.push(0, 0, 0, 0);

        return points;
    }

    // Compute flatline pixels for inter-block time.
    // replay=true compresses history, false = real-time proportional gaps
    function flatlinePixels(interBlockSeconds, replay) {
        if (replay) {
            // Compressed: show relative differences but keep it tight
            return Math.max(15, Math.min(interBlockSeconds / 15, 80));
        }
        // Real-time: longer wait = more flatline. 10min=~150px, 30min=~350px
        return Math.max(40, Math.min(interBlockSeconds / 4, 350));
    }

    // ── Color computation ──────────────────────────────────────
    function computeColor(elapsedSec, feeRate, mempoolMB) {
        var timeStress = Math.min(elapsedSec / 1800, 1.0);
        var feeStress = Math.min(feeRate / 150, 1.0);
        var mempoolStress = Math.min(mempoolMB / 200, 1.0);

        var stress = timeStress * 0.4 + feeStress * 0.35 + mempoolStress * 0.25;

        if (stress < 0.2)  return COLORS.healthy;
        if (stress < 0.4)  return COLORS.calm;
        if (stress < 0.6)  return COLORS.elevated;
        if (stress < 0.8)  return COLORS.stressed;
        return COLORS.critical;
    }

    function hexToRgb(hex) {
        var r = parseInt(hex.slice(1, 3), 16);
        var g = parseInt(hex.slice(3, 5), 16);
        var b = parseInt(hex.slice(5, 7), 16);
        return [r, g, b];
    }

    function lerpColor(a, b, t) {
        var ar = hexToRgb(a), br = hexToRgb(b);
        var r = Math.round(ar[0] + (br[0] - ar[0]) * t);
        var g = Math.round(ar[1] + (br[1] - ar[1]) * t);
        var bl = Math.round(ar[2] + (br[2] - ar[2]) * t);
        return 'rgb(' + r + ',' + g + ',' + bl + ')';
    }

    // ── Grid drawing ───────────────────────────────────────────
    function drawGrid(ctx, w, h) {
        ctx.strokeStyle = GRID_COLOR;
        ctx.lineWidth = 0.5;
        var spacing = 40;
        for (var x = 0; x < w; x += spacing) {
            ctx.beginPath();
            ctx.moveTo(x, 0);
            ctx.lineTo(x, h);
            ctx.stroke();
        }
        for (var y = 0; y < h; y += spacing) {
            ctx.beginPath();
            ctx.moveTo(0, y);
            ctx.lineTo(w, y);
            ctx.stroke();
        }
    }

    // ── Main draw loop ─────────────────────────────────────────
    function drawFrame() {
        if (!_hb) return;

        var ctx = _hb.ctx;
        var w = _hb.width;
        var h = _hb.height;
        var baseline = h * 0.55; // slightly below center

        // Eraser bar: clear a strip ahead of the sweep line
        var eraserWidth = 30;
        ctx.fillStyle = BG_COLOR;
        ctx.fillRect(_hb.x, 0, eraserWidth, h);

        // Redraw grid in erased area
        ctx.save();
        ctx.beginPath();
        ctx.rect(_hb.x, 0, eraserWidth, h);
        ctx.clip();
        drawGrid(ctx, w, h);
        ctx.restore();

        // Compute current color
        var now = Date.now() / 1000;
        var elapsed = _hb.lastBlockTime > 0 ? now - _hb.lastBlockTime : 0;
        var targetColor = computeColor(elapsed, _hb.nextBlockFee, _hb.mempoolMB);
        _hb.colorLerp += 0.02;
        if (_hb.colorLerp >= 1) {
            _hb.currentColor = targetColor;
            _hb.prevColor = targetColor;
            _hb.colorLerp = 1;
        }
        if (targetColor !== _hb.targetColor) {
            _hb.prevColor = _hb.currentColor;
            _hb.targetColor = targetColor;
            _hb.colorLerp = 0;
        }
        var drawColor = _hb.colorLerp < 1
            ? lerpColor(_hb.prevColor, _hb.targetColor, _hb.colorLerp)
            : _hb.currentColor;

        // Subtle jitter on long waits
        var jitter = 0;
        if (elapsed > 1200) {
            jitter = (Math.random() - 0.5) * Math.min((elapsed - 1200) / 600, 1) * 3;
        }

        // Determine y value
        var y = baseline + jitter;

        if (_hb.currentWave) {
            // Drawing a waveform
            if (_hb.waveIdx < _hb.currentWave.length) {
                y = baseline + _hb.currentWave[_hb.waveIdx];
                _hb.waveIdx++;
            } else {
                // Waveform complete
                _hb.currentWave = null;
                _hb.waveIdx = 0;
                _hb.flatlineRemaining = 0;
            }
        } else if (_hb.flatlineRemaining > 0) {
            // Between waveforms: flatline
            _hb.flatlineRemaining--;
            y = baseline + jitter;
        } else if (_hb.queue.length > 0) {
            // Start next waveform from queue
            var next = _hb.queue.shift();
            _hb.currentWave = next.points;
            _hb.waveIdx = 0;
            _hb.flatlineRemaining = next.flatline;
            // Update last block time for color
            if (next.timestamp) {
                _hb.lastBlockTime = next.timestamp;
            }
            y = baseline + jitter;
        } else {
            // No queued waveforms — real-time flatline
            y = baseline + jitter;
        }

        // Draw line segment
        ctx.beginPath();
        ctx.moveTo(_hb.prevX, _hb.prevY);
        ctx.lineTo(_hb.x, y);
        ctx.strokeStyle = drawColor;
        ctx.lineWidth = 2;
        ctx.shadowBlur = 12;
        ctx.shadowColor = drawColor;
        ctx.stroke();
        ctx.shadowBlur = 0;

        // Store for next frame
        _hb.prevX = _hb.x;
        _hb.prevY = y;

        // Advance sweep position
        _hb.x += _hb.speed;
        if (_hb.x >= w) {
            _hb.x = 0;
            _hb.prevX = 0;
            _hb.prevY = baseline;
        }

        _hb.rafId = requestAnimationFrame(drawFrame);
    }

    // ── Public API ─────────────────────────────────────────────

    window.initHeartbeat = function(canvasId) {
        if (_hb) destroyHeartbeat();

        var canvas = document.getElementById(canvasId);
        if (!canvas) return;

        var container = canvas.parentElement;
        var dpr = window.devicePixelRatio || 1;
        var rect = canvas.getBoundingClientRect();

        canvas.width = rect.width * dpr;
        canvas.height = rect.height * dpr;

        var ctx = canvas.getContext('2d');
        ctx.scale(dpr, dpr);

        var w = rect.width;
        var h = rect.height;
        var baseline = h * 0.55;

        // Fill background and draw grid
        ctx.fillStyle = BG_COLOR;
        ctx.fillRect(0, 0, w, h);
        drawGrid(ctx, w, h);

        _hb = {
            canvas: canvas,
            ctx: ctx,
            width: w,
            height: h,
            x: 0,
            prevX: 0,
            prevY: baseline,
            speed: 1.5,
            queue: [],
            currentWave: null,
            waveIdx: 0,
            flatlineRemaining: 0,
            lastBlockTime: 0,
            nextBlockFee: 0,
            mempoolMB: 0,
            currentColor: COLORS.healthy,
            prevColor: COLORS.healthy,
            targetColor: COLORS.healthy,
            colorLerp: 1,
            rafId: null
        };

        // ResizeObserver for responsive canvas
        if (typeof ResizeObserver !== 'undefined') {
            _hb.resizeObs = new ResizeObserver(function() {
                if (!_hb) return;
                var r = canvas.getBoundingClientRect();
                canvas.width = r.width * dpr;
                canvas.height = r.height * dpr;
                _hb.ctx = canvas.getContext('2d');
                _hb.ctx.scale(dpr, dpr);
                _hb.width = r.width;
                _hb.height = r.height;
                _hb.x = 0;
                _hb.prevX = 0;
                _hb.prevY = r.height * 0.55;
                _hb.ctx.fillStyle = BG_COLOR;
                _hb.ctx.fillRect(0, 0, r.width, r.height);
                drawGrid(_hb.ctx, r.width, r.height);
            });
            _hb.resizeObs.observe(container);
        }

        _hb.rafId = requestAnimationFrame(drawFrame);
    };

    window.pushHeartbeatBlocks = function(json, replay) {
        if (!_hb) return;
        var isReplay = !!replay;
        try {
            var blocks = JSON.parse(json);
            if (!Array.isArray(blocks)) return;

            for (var i = 0; i < blocks.length; i++) {
                var b = blocks[i];
                var interBlock = b.inter_block_seconds || 600;
                var points = generatePQRST(b);
                var flat = flatlinePixels(interBlock, isReplay);

                _hb.queue.push({
                    points: points,
                    flatline: flat,
                    timestamp: b.timestamp || 0,
                    height: b.height || 0
                });

                // Update last block time
                if (b.timestamp) {
                    _hb.lastBlockTime = b.timestamp;
                }
            }

            // Dispatch event for UI updates
            if (blocks.length > 0) {
                var last = blocks[blocks.length - 1];
                window.dispatchEvent(new CustomEvent('heartbeat-block', {
                    detail: {
                        height: last.height,
                        timestamp: last.timestamp,
                        tx_count: last.tx_count,
                        total_fees: last.total_fees
                    }
                }));
            }
        } catch (e) {
            console.warn('heartbeat: bad block json', e);
        }
    };

    window.updateHeartbeatLive = function(json) {
        if (!_hb) return;
        try {
            var data = JSON.parse(json);
            if (data.next_block_fee !== undefined) _hb.nextBlockFee = data.next_block_fee;
            if (data.mempool_mb !== undefined) _hb.mempoolMB = data.mempool_mb;
            if (data.block_time !== undefined && data.block_time > _hb.lastBlockTime) {
                _hb.lastBlockTime = data.block_time;
            }
        } catch (e) {
            console.warn('heartbeat: bad live json', e);
        }
    };

    // ── Phase 2: Vital Signs computation ─────────────────────

    // Store recent block timestamps for heart rate calculation
    // _hb.recentBlocks = [{timestamp, height}, ...]

    // Compute blocks per 10 minutes from the last ~6 blocks (rolling 1hr window)
    function computeHeartRate() {
        if (!_hb || !_hb.recentBlocks || _hb.recentBlocks.length < 2) {
            return { bpm: 1.0, label: 'Normal', color: COLORS.healthy };
        }
        var blocks = _hb.recentBlocks;
        var newest = blocks[blocks.length - 1].timestamp;
        var oldest = blocks[0].timestamp;
        var spanSec = newest - oldest;
        if (spanSec <= 0) return { bpm: 1.0, label: 'Normal', color: COLORS.healthy };

        // blocks per 600 seconds (10 minutes)
        var bpm = ((blocks.length - 1) / spanSec) * 600;
        var label, color;
        if (bpm < 0.7) {
            label = 'Bradycardia'; color = COLORS.calm;
        } else if (bpm <= 1.5) {
            label = 'Normal'; color = COLORS.healthy;
        } else {
            label = 'Tachycardia'; color = COLORS.elevated;
        }
        return { bpm: bpm, label: label, color: color };
    }

    // Body temperature from mempool fullness
    function computeTemperature(mempoolMB) {
        // 10 vMB = 36.5C (healthy), 150+ vMB = 39+ (fever)
        // Linear mapping: 0-10 = 36.0-36.5, 10-150 = 36.5-39.5, 150+ = 39.5+
        var temp;
        if (mempoolMB <= 10) {
            temp = 36.0 + (mempoolMB / 10) * 0.5;
        } else if (mempoolMB <= 150) {
            temp = 36.5 + ((mempoolMB - 10) / 140) * 3.0;
        } else {
            temp = 39.5 + Math.min((mempoolMB - 150) / 100, 1.5);
        }
        var label, color;
        if (temp < 37.0) {
            label = 'Healthy'; color = COLORS.healthy;
        } else if (temp < 38.0) {
            label = 'Warm'; color = COLORS.elevated;
        } else if (temp < 39.0) {
            label = 'Fever'; color = COLORS.stressed;
        } else {
            label = 'Critical'; color = COLORS.critical;
        }
        return { temp: temp, label: label, color: color };
    }

    // Immune system (hashrate)
    function computeImmune(hashrateEH) {
        var label, color;
        if (hashrateEH >= 700) {
            label = 'Excellent'; color = COLORS.healthy;
        } else if (hashrateEH >= 400) {
            label = 'Strong'; color = COLORS.healthy;
        } else if (hashrateEH >= 200) {
            label = 'Moderate'; color = COLORS.elevated;
        } else {
            label = 'Weak'; color = COLORS.stressed;
        }
        return { hashrate: hashrateEH, label: label, color: color };
    }

    // Blood pressure: systolic = next block fee, diastolic = mempool min fee
    function computeBloodPressure(nextBlockFee, mempoolMinFee) {
        var systolic = nextBlockFee || 0;
        var diastolic = mempoolMinFee || 1;
        var label, color;
        var avg = (systolic + diastolic) / 2;
        if (avg < 10) {
            label = 'Normal'; color = COLORS.healthy;
        } else if (avg < 30) {
            label = 'Elevated'; color = COLORS.elevated;
        } else if (avg < 80) {
            label = 'High'; color = COLORS.stressed;
        } else {
            label = 'Critical'; color = COLORS.critical;
        }
        return { systolic: systolic, diastolic: diastolic, label: label, color: color };
    }

    // Get all vital signs as a JSON string for Rust consumption
    window.getHeartbeatVitals = function() {
        if (!_hb) return '{}';
        var hr = computeHeartRate();
        var temp = computeTemperature(_hb.mempoolMB);
        var immune = computeImmune(_hb.hashrateEH || 0);
        var bp = computeBloodPressure(_hb.nextBlockFee, _hb.mempoolMinFee || 1);
        return JSON.stringify({
            heart_rate_bpm: Math.round(hr.bpm * 10) / 10,
            heart_rate_label: hr.label,
            heart_rate_color: hr.color,
            bp_systolic: Math.round(bp.systolic * 10) / 10,
            bp_diastolic: Math.round(bp.diastolic * 10) / 10,
            bp_label: bp.label,
            bp_color: bp.color,
            temp_c: Math.round(temp.temp * 10) / 10,
            temp_label: temp.label,
            temp_color: temp.color,
            immune_eh: Math.round(immune.hashrate * 10) / 10,
            immune_label: immune.label,
            immune_color: immune.color
        });
    };

    // ── Phase 3: 24-Hour Rhythm Strip ─────────────────────────

    window.renderRhythmStrip = function(canvasId, blocksJson) {
        var canvas = document.getElementById(canvasId);
        if (!canvas) return;

        var dpr = window.devicePixelRatio || 1;
        var rect = canvas.getBoundingClientRect();
        canvas.width = rect.width * dpr;
        canvas.height = rect.height * dpr;

        var ctx = canvas.getContext('2d');
        ctx.scale(dpr, dpr);

        var w = rect.width;
        var h = rect.height;
        var baseline = h * 0.55;

        // Fill background
        ctx.fillStyle = BG_COLOR;
        ctx.fillRect(0, 0, w, h);

        // Subtle grid
        ctx.strokeStyle = GRID_COLOR;
        ctx.lineWidth = 0.5;
        var gridH = 30;
        for (var gy = 0; gy < h; gy += gridH) {
            ctx.beginPath();
            ctx.moveTo(0, gy);
            ctx.lineTo(w, gy);
            ctx.stroke();
        }

        var blocks;
        try {
            blocks = JSON.parse(blocksJson);
        } catch(e) {
            return;
        }
        if (!Array.isArray(blocks) || blocks.length === 0) return;

        // Generate all waveform data
        var waveforms = [];
        var totalPoints = 0;
        for (var i = 0; i < blocks.length; i++) {
            var pts = generatePQRST(blocks[i]);
            var interBlock = blocks[i].inter_block_seconds || 600;
            // Compressed flatline: proportional to gap but tight
            var flatPx = Math.max(3, Math.min(interBlock / 40, 20));
            waveforms.push({ points: pts, flatline: flatPx, block: blocks[i] });
            totalPoints += pts.length + flatPx;
        }

        // Scale to fit canvas width
        var scale = w / totalPoints;
        if (scale > 3) scale = 3; // cap individual point width

        // Draw
        ctx.beginPath();
        ctx.moveTo(0, baseline);
        var x = 0;
        for (var wi = 0; wi < waveforms.length; wi++) {
            var wf = waveforms[wi];
            var interBlock = wf.block.inter_block_seconds || 600;

            // Color for this block's segment
            var feeRate = wf.block.total_fees ? wf.block.total_fees / 100000 : 0; // rough approx
            var segColor = computeColor(interBlock, feeRate, 0);

            // Draw flatline before waveform
            var flatEnd = x + wf.flatline * scale;
            ctx.lineTo(flatEnd, baseline);
            x = flatEnd;

            // Draw waveform points with vertical scaling (compressed)
            var vScale = 0.4; // compress vertically
            for (var pi = 0; pi < wf.points.length; pi++) {
                x += scale;
                ctx.lineTo(x, baseline + wf.points[pi] * vScale);
            }
        }

        ctx.strokeStyle = COLORS.healthy;
        ctx.lineWidth = 1;
        ctx.shadowBlur = 4;
        ctx.shadowColor = COLORS.healthy;
        ctx.stroke();
        ctx.shadowBlur = 0;

        // Draw hour markers at the bottom
        if (blocks.length > 1) {
            var firstTs = blocks[0].timestamp;
            var lastTs = blocks[blocks.length - 1].timestamp;
            var spanHrs = (lastTs - firstTs) / 3600;
            if (spanHrs > 1) {
                ctx.fillStyle = 'rgba(255,255,255,0.15)';
                ctx.font = '9px monospace';
                // Mark every ~4 hours
                var interval = spanHrs > 12 ? 4 : 2;
                var startHour = Math.ceil((firstTs / 3600)) * 3600;
                for (var ts = startHour; ts < lastTs; ts += interval * 3600) {
                    var frac = (ts - firstTs) / (lastTs - firstTs);
                    var mx = frac * w;
                    ctx.fillRect(mx, 0, 0.5, h);
                    var d = new Date(ts * 1000);
                    var hourLabel = d.getUTCHours() + ':00';
                    ctx.fillText(hourLabel, mx + 2, h - 3);
                }
            }
        }
    };

    // ── Phase 4: Polish effects ───────────────────────────────

    // Background breathing pulse on block arrival
    window.heartbeatPulse = function() {
        var card = document.getElementById('heartbeat-card');
        if (!card) return;
        card.style.transition = 'box-shadow 0.3s ease-in';
        var color = (_hb && _hb.currentColor) ? _hb.currentColor : COLORS.healthy;
        card.style.boxShadow = '0 0 30px ' + color + '40, inset 0 0 15px ' + color + '15';
        setTimeout(function() {
            card.style.transition = 'box-shadow 1.2s ease-out';
            card.style.boxShadow = 'none';
        }, 300);
    };

    // Flash the EKG line white on new block
    var _flashTimer = null;
    window.heartbeatFlash = function() {
        if (!_hb) return;
        _hb._flashColor = _hb.currentColor;
        _hb.currentColor = '#ffffff';
        _hb.prevColor = '#ffffff';
        _hb.colorLerp = 1;
        if (_flashTimer) clearTimeout(_flashTimer);
        _flashTimer = setTimeout(function() {
            if (_hb && _hb._flashColor) {
                _hb.prevColor = '#ffffff';
                _hb.targetColor = _hb._flashColor;
                _hb.colorLerp = 0;
                _hb._flashColor = null;
            }
        }, 200);
    };

    // Organism status text
    window.getOrganismStatus = function() {
        if (!_hb) return '{}';
        var now = Date.now() / 1000;
        var elapsed = _hb.lastBlockTime > 0 ? now - _hb.lastBlockTime : 0;
        var color = computeColor(elapsed, _hb.nextBlockFee, _hb.mempoolMB);

        var condition, description;
        if (color === COLORS.critical) {
            condition = 'Critical';
            description = 'Holding its breath. Waiting for the next block...';
        } else if (color === COLORS.stressed) {
            condition = 'Stressed';
            description = 'Heavy breathing. Block space is scarce.';
        } else if (color === COLORS.elevated) {
            condition = 'Elevated';
            description = 'Running warm. Activity is rising.';
        } else if (color === COLORS.calm) {
            condition = 'Calm';
            description = 'Deep, slow breaths. The network rests.';
        } else {
            condition = 'Healthy';
            description = 'Steady rhythm. The network hums.';
        }

        return JSON.stringify({
            condition: condition,
            description: description,
            color: color
        });
    };

    // ── Phase 5: Sound (Web Audio heartbeat) ──────────────────

    var _audioCtx = null;
    var _soundEnabled = false;

    window.heartbeatSoundToggle = function(enable) {
        _soundEnabled = !!enable;
        if (_soundEnabled && !_audioCtx) {
            try {
                _audioCtx = new (window.AudioContext || window.webkitAudioContext)();
            } catch(e) {
                _soundEnabled = false;
            }
        }
        return _soundEnabled;
    };

    window.heartbeatSoundIsEnabled = function() {
        return _soundEnabled;
    };

    // Play lub-dub heartbeat sound
    window.heartbeatPlaySound = function() {
        if (!_soundEnabled || !_audioCtx) return;
        // Resume if suspended (autoplay policy)
        if (_audioCtx.state === 'suspended') {
            _audioCtx.resume();
        }
        var now = _audioCtx.currentTime;

        // "Lub" - 80Hz sine, 60ms
        var lub = _audioCtx.createOscillator();
        var lubGain = _audioCtx.createGain();
        lub.frequency.value = 80;
        lub.type = 'sine';
        lubGain.gain.setValueAtTime(0.3, now);
        lubGain.gain.exponentialRampToValueAtTime(0.001, now + 0.06);
        lub.connect(lubGain);
        lubGain.connect(_audioCtx.destination);
        lub.start(now);
        lub.stop(now + 0.07);

        // "Dub" - 60Hz sine, 40ms, 120ms after lub
        var dub = _audioCtx.createOscillator();
        var dubGain = _audioCtx.createGain();
        dub.frequency.value = 60;
        dub.type = 'sine';
        dubGain.gain.setValueAtTime(0.2, now + 0.12);
        dubGain.gain.exponentialRampToValueAtTime(0.001, now + 0.16);
        dub.connect(dubGain);
        dubGain.connect(_audioCtx.destination);
        dub.start(now + 0.12);
        dub.stop(now + 0.17);
    };

    // ── Phase 5: Moment Capture ───────────────────────────────

    window.heartbeatCapture = function(vitalsJson) {
        if (!_hb) return null;

        // Create an offscreen canvas for the share card
        var cardW = 1200, cardH = 630;
        var offscreen = document.createElement('canvas');
        offscreen.width = cardW;
        offscreen.height = cardH;
        var octx = offscreen.getContext('2d');

        // Background
        octx.fillStyle = '#0a1929';
        octx.fillRect(0, 0, cardW, cardH);

        // Border
        var color = _hb.currentColor || COLORS.healthy;
        octx.strokeStyle = color;
        octx.lineWidth = 2;
        octx.strokeRect(8, 8, cardW - 16, cardH - 16);

        // Copy current EKG canvas
        try {
            var ekgH = 280;
            octx.drawImage(_hb.canvas, 20, 20, cardW - 40, ekgH);
        } catch(e) {}

        // Draw vital signs text
        var vitals;
        try { vitals = JSON.parse(vitalsJson); } catch(e) { vitals = {}; }

        octx.font = 'bold 24px monospace';
        octx.fillStyle = color;
        var y = 330;
        var col1 = 40, col2 = 340, col3 = 640, col4 = 940;

        // Heart Rate
        octx.fillStyle = vitals.heart_rate_color || color;
        octx.fillText('Heart Rate', col1, y);
        octx.font = 'bold 36px monospace';
        octx.fillText((vitals.heart_rate_bpm || '---') + ' bpm', col1, y + 40);
        octx.font = '18px monospace';
        octx.fillText(vitals.heart_rate_label || '', col1, y + 65);

        // Blood Pressure
        octx.font = 'bold 24px monospace';
        octx.fillStyle = vitals.bp_color || color;
        octx.fillText('Blood Pressure', col2, y);
        octx.font = 'bold 36px monospace';
        octx.fillText((vitals.bp_systolic || '0') + ' / ' + (vitals.bp_diastolic || '0'), col2, y + 40);
        octx.font = '18px monospace';
        octx.fillText(vitals.bp_label || '', col2, y + 65);

        // Temperature
        octx.font = 'bold 24px monospace';
        octx.fillStyle = vitals.temp_color || color;
        octx.fillText('Temperature', col3, y);
        octx.font = 'bold 36px monospace';
        octx.fillText((vitals.temp_c || '36.5') + '\u00B0C', col3, y + 40);
        octx.font = '18px monospace';
        octx.fillText(vitals.temp_label || '', col3, y + 65);

        // Immune System
        octx.font = 'bold 24px monospace';
        octx.fillStyle = vitals.immune_color || color;
        octx.fillText('Immune System', col4, y);
        octx.font = 'bold 36px monospace';
        octx.fillText((vitals.immune_eh || '0') + ' EH/s', col4, y + 40);
        octx.font = '18px monospace';
        octx.fillText(vitals.immune_label || '', col4, y + 65);

        // Organism status
        var status;
        try { status = JSON.parse(window.getOrganismStatus()); } catch(e) { status = {}; }
        octx.font = 'bold 22px monospace';
        octx.fillStyle = status.color || color;
        var statusY = 470;
        octx.fillText('Condition: ' + (status.condition || 'Unknown'), col1, statusY);
        octx.font = 'italic 18px monospace';
        octx.fillStyle = 'rgba(255,255,255,0.6)';
        octx.fillText('"' + (status.description || '') + '"', col1, statusY + 30);

        // Block height
        octx.font = 'bold 18px monospace';
        octx.fillStyle = 'rgba(255,255,255,0.4)';
        var blockTxt = 'Block ' + (_hb.recentBlocks && _hb.recentBlocks.length > 0
            ? '#' + _hb.recentBlocks[_hb.recentBlocks.length - 1].height
            : '---');
        octx.fillText(blockTxt, col1, cardH - 50);

        // Timestamp
        var d = new Date();
        octx.fillText(d.toISOString().replace('T', ' ').slice(0, 19) + ' UTC', col1, cardH - 28);

        // Watermark
        octx.font = 'bold 20px monospace';
        octx.fillStyle = 'rgba(255,255,255,0.25)';
        octx.textAlign = 'right';
        octx.fillText('wehodlbtc.com', cardW - 40, cardH - 30);
        octx.textAlign = 'left';

        // Return data URL
        return offscreen.toDataURL('image/png');
    };

    // Trigger download of captured image
    window.heartbeatDownloadCapture = function(vitalsJson) {
        var dataUrl = window.heartbeatCapture(vitalsJson);
        if (!dataUrl) return;
        var link = document.createElement('a');
        link.download = 'bitcoin-heartbeat-' + Date.now() + '.png';
        link.href = dataUrl;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
    };

    // ── Update existing functions to support new features ─────

    // Override pushHeartbeatBlocks to also track recent blocks
    var _origPush = window.pushHeartbeatBlocks;
    window.pushHeartbeatBlocks = function(json, replay) {
        _origPush(json, replay);
        if (!_hb) return;
        // Maintain recentBlocks list (up to 144 for rhythm strip)
        if (!_hb.recentBlocks) _hb.recentBlocks = [];
        try {
            var blocks = JSON.parse(json);
            if (Array.isArray(blocks)) {
                for (var i = 0; i < blocks.length; i++) {
                    _hb.recentBlocks.push({
                        timestamp: blocks[i].timestamp,
                        height: blocks[i].height,
                        tx_count: blocks[i].tx_count,
                        total_fees: blocks[i].total_fees,
                        size: blocks[i].size,
                        weight: blocks[i].weight,
                        inter_block_seconds: blocks[i].inter_block_seconds
                    });
                }
                // Keep last 144
                if (_hb.recentBlocks.length > 144) {
                    _hb.recentBlocks = _hb.recentBlocks.slice(-144);
                }
            }
        } catch(e) {}
    };

    // Override updateHeartbeatLive to accept additional fields
    var _origUpdate = window.updateHeartbeatLive;
    window.updateHeartbeatLive = function(json) {
        _origUpdate(json);
        if (!_hb) return;
        try {
            var data = JSON.parse(json);
            if (data.hashrate_eh !== undefined) _hb.hashrateEH = data.hashrate_eh;
            if (data.mempool_min_fee !== undefined) _hb.mempoolMinFee = data.mempool_min_fee;
            if (data.difficulty !== undefined) _hb.difficulty = data.difficulty;
            if (data.block_height !== undefined) _hb.blockHeight = data.block_height;
        } catch(e) {}
    };

    // Get recent blocks JSON for rhythm strip
    window.getHeartbeatRecentBlocks = function() {
        if (!_hb || !_hb.recentBlocks) return '[]';
        return JSON.stringify(_hb.recentBlocks);
    };

    window.destroyHeartbeat = function() {
        if (!_hb) return;
        if (_hb.rafId) cancelAnimationFrame(_hb.rafId);
        if (_hb.resizeObs) _hb.resizeObs.disconnect();
        _hb = null;
    };

})();
