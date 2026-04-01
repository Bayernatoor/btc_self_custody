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

    window.destroyHeartbeat = function() {
        if (!_hb) return;
        if (_hb.rafId) cancelAnimationFrame(_hb.rafId);
        if (_hb.resizeObs) _hb.resizeObs.disconnect();
        _hb = null;
    };

})();
