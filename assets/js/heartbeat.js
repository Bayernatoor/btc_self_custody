// Block Heartbeat v2 — Scrollable Timeline EKG engine
// Draws a hospital cardiac monitor driven by real Bitcoin block data.
// v2 replaces the circular sweep with an infinite scrollable horizontal
// timeline. Users can scroll/drag left to see past blocks, click on
// waveforms for block details, and see mempool blips on the live flatline.
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

    // ── Constants for timeline pixel mapping ───────────────────
    var POINT_WIDTH = 1.5;  // Each PQRST point = 1.5 virtual pixels wide

    // Compute flatline width in virtual pixels for a completed (history) flatline
    function historyFlatlineWidth(interBlockSeconds) {
        // Proportional: 1min=15px, 5min=75px, 10min=150px, 30min=300px (capped)
        return Math.max(10, Math.min(interBlockSeconds / 4, 300));
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

    // ── Timeline segment helpers ───────────────────────────────

    // Create a block segment from block data
    function createBlockSegment(block, xStart) {
        var points = generatePQRST(block);
        var width = points.length * POINT_WIDTH;
        var interBlock = block.inter_block_seconds || 600;
        var feeRate = block.total_fees ? block.total_fees / 100000 : 0;
        var color = computeColor(interBlock, feeRate, _hb ? _hb.mempoolMB : 0);

        return {
            type: 'block',
            height: block.height || 0,
            timestamp: block.timestamp || 0,
            tx_count: block.tx_count || 0,
            total_fees: block.total_fees || 0,
            weight: block.weight || 0,
            inter_block_seconds: interBlock,
            points: points,
            x_start: xStart,
            x_end: xStart + width,
            color: color
        };
    }

    // Create a flatline segment
    function createFlatlineSegment(xStart, xEnd) {
        return {
            type: 'flatline',
            x_start: xStart,
            x_end: xEnd,   // null for live (open-ended)
            color: null,    // set when closed (block arrives). null = use live color
            blips: []
        };
    }

    // ── Hit testing: find block segment under a virtual x position ──
    function blockAtVirtualX(vx) {
        if (!_hb) return null;
        for (var i = 0; i < _hb.timeline.length; i++) {
            var seg = _hb.timeline[i];
            if (seg.type === 'block' && vx >= seg.x_start && vx <= seg.x_end) {
                return seg;
            }
        }
        return null;
    }

    // Find flatline segment under a virtual x position, with context
    function flatlineAtVirtualX(vx) {
        if (!_hb) return null;
        for (var i = 0; i < _hb.timeline.length; i++) {
            var seg = _hb.timeline[i];
            var xEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
            if (seg.type === 'flatline' && vx >= seg.x_start && vx <= xEnd) {
                // Find surrounding blocks for interval display
                var prevBlock = null, nextBlock = null;
                for (var j = i - 1; j >= 0; j--) {
                    if (_hb.timeline[j].type === 'block') { prevBlock = _hb.timeline[j]; break; }
                }
                for (var j = i + 1; j < _hb.timeline.length; j++) {
                    if (_hb.timeline[j].type === 'block') { nextBlock = _hb.timeline[j]; break; }
                }
                return { segment: seg, prevBlock: prevBlock, nextBlock: nextBlock };
            }
        }
        return null;
    }

    // Convert canvas pixel x to virtual x (zoom-aware)
    function canvasToVirtual(canvasX) {
        if (!_hb) return 0;
        return _hb.viewOffset + canvasX / _hb.zoom;
    }

    // Convert virtual x to canvas pixel x (zoom-aware)
    function virtualToCanvas(vx) {
        if (!_hb) return 0;
        return (vx - _hb.viewOffset) * _hb.zoom;
    }

    // Find nearest blip within hitRadius virtual pixels
    function blipAtVirtualX(vx, hitRadius) {
        if (!_hb) return null;
        var best = null, bestDist = hitRadius;
        for (var i = 0; i < _hb.timeline.length; i++) {
            var seg = _hb.timeline[i];
            if (seg.type !== 'flatline' || !seg.blips) continue;
            for (var bi = 0; bi < seg.blips.length; bi++) {
                var blip = seg.blips[bi];
                var dist = Math.abs(blip.x - vx);
                if (dist < bestDist) {
                    bestDist = dist;
                    best = blip;
                }
            }
        }
        return best;
    }

    // ── Mempool WebSocket feed ─────────────────────────────────
    function connectMempoolFeed() {
        if (!_hb) return;
        _hb._lastMempoolTxCount = 0; // for diffing
        try {
            var ws = new WebSocket('wss://mempool.space/api/v1/ws');
            ws.onopen = function() {
                ws.send(JSON.stringify({ action: 'want', data: ['mempool-blocks'] }));
                _hb.wsConnected = true;
                _hb._wsRetryDelay = 5000; // reset backoff on success
            };
            ws.onmessage = function(e) {
                if (!_hb) return;
                try {
                    var data = JSON.parse(e.data);
                    if (data['mempool-blocks']) {
                        var mblocks = data['mempool-blocks'];
                        // Count total txs across all projected blocks
                        var totalTx = 0;
                        for (var i = 0; i < mblocks.length; i++) {
                            totalTx += mblocks[i].nTx || 0;
                        }
                        // Diff: only new txs since last update create visual blips
                        var newTx = 0;
                        if (_hb._lastMempoolTxCount > 0) {
                            newTx = Math.max(0, totalTx - _hb._lastMempoolTxCount);
                        }
                        _hb._lastMempoolTxCount = totalTx;

                        // Get fee rate distribution from the first projected block
                        var topFee = Math.max(2, mblocks[0] ? (mblocks[0].feeRange || [1, 5, 10])[2] || 10 : 5);
                        var medFee = Math.max(1, mblocks[0] ? mblocks[0].medianFee || 5 : 5);

                        if (newTx > 0) {
                            addMempoolTxs(newTx, medFee, topFee);
                        }
                    }
                } catch (err) {}
            };
            ws.onclose = function() {
                if (!_hb) return;
                _hb.wsConnected = false;
                // Exponential backoff: 5s, 10s, 20s, 40s... max 5min
                _hb._wsRetryDelay = Math.min((_hb._wsRetryDelay || 5000) * 2, 300000);
                setTimeout(function() {
                    if (_hb) connectMempoolFeed();
                }, _hb._wsRetryDelay);
            };
            ws.onerror = function() { ws.close(); };
            _hb.ws = ws;
        } catch (err) {}
    }

    // Add tx blips at the live head position. Each blip represents ~N new txs.
    // Placed at the current head so they naturally spread out as time advances.
    function addMempoolTxs(newTxCount, medianFeeRate, topFeeRate) {
        if (!_hb) return;
        var liveSeg = _hb.timeline[_hb.timeline.length - 1];
        if (!liveSeg || liveSeg.type !== 'flatline') return;

        // Visual blips: 1 blip per ~15 txs, cap at 15 per update
        var n = Math.min(Math.max(1, Math.ceil(newTxCount / 15)), 15);
        var txPerBlip = Math.ceil(newTxCount / n);

        for (var i = 0; i < n; i++) {
            // Vary fee rate per blip: use wider random range to show diversity
            // even in calm periods. Mix of min-fee txs and higher-fee txs.
            var roll = Math.random();
            var feeRate;
            if (roll < 0.5) {
                // Half at or below median
                feeRate = medianFeeRate * (0.3 + Math.random() * 0.7);
            } else if (roll < 0.85) {
                // 35% between median and top
                feeRate = medianFeeRate + Math.random() * (topFeeRate - medianFeeRate);
            } else {
                // 15% outliers (2-5x top fee) - these are the urgent/RBF txs
                feeRate = topFeeRate * (2 + Math.random() * 3);
            }
            feeRate = Math.max(0.5, feeRate);

            // Normalize: use log scale so low-fee differences are visible
            var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0); // log2(64)=6

            // Color: adaptive thresholds based on current median
            // This ensures color variety even when fees are uniformly low
            var color;
            if (feeRate < medianFeeRate * 0.8) {
                color = 'rgba(66, 165, 245, '; // blue (below median)
            } else if (feeRate < medianFeeRate * 1.5) {
                color = 'rgba(0, 230, 118, ';   // green (around median)
            } else if (feeRate < medianFeeRate * 4) {
                color = 'rgba(247, 147, 26, ';  // orange (above median)
            } else {
                color = 'rgba(255, 87, 34, ';   // red (outlier/urgent)
            }

            // Place at the live head with small random jitter
            liveSeg.blips.push({
                x: _hb.virtualX - Math.random() * 10,
                height: 10 + feeNorm * 60 + Math.random() * 20,  // 10-90px
                color: color,
                opacity: 0.5 + feeNorm * 0.4,
                txCount: txPerBlip,
                feeRate: Math.round(feeRate * 10) / 10,
                timestamp: Date.now() / 1000,
                fadeStart: 0
            });
        }
    }

    // ── Tooltip drawing ────────────────────────────────────────
    function drawTooltip(ctx, seg, canvasX, canvasY, baseline) {
        var padding = 8;
        var lineH = 16;
        var lines = [
            'Block #' + seg.height,
            'Txns: ' + seg.tx_count.toLocaleString(),
            'Fees: ' + (seg.total_fees / 100000000).toFixed(4) + ' BTC',
            'Wait: ' + formatDuration(seg.inter_block_seconds)
        ];
        if (seg.timestamp) {
            var d = new Date(seg.timestamp * 1000);
            lines.push(d.toISOString().replace('T', ' ').slice(0, 19) + ' UTC');
        }

        ctx.font = '12px monospace';
        var maxW = 0;
        for (var i = 0; i < lines.length; i++) {
            var m = ctx.measureText(lines[i]).width;
            if (m > maxW) maxW = m;
        }

        var boxW = maxW + padding * 2;
        var boxH = lines.length * lineH + padding * 2;
        var boxX = canvasX - boxW / 2;
        var boxY = canvasY - boxH - 12; // above the waveform

        // Clamp to canvas bounds
        if (boxX < 4) boxX = 4;
        if (boxX + boxW > _hb.width - 4) boxX = _hb.width - boxW - 4;
        if (boxY < 4) boxY = 4;

        // Background
        ctx.fillStyle = 'rgba(10, 25, 41, 0.92)';
        ctx.beginPath();
        roundRect(ctx, boxX, boxY, boxW, boxH, 4);
        ctx.fill();

        // Border
        ctx.strokeStyle = seg.color;
        ctx.lineWidth = 1;
        ctx.beginPath();
        roundRect(ctx, boxX, boxY, boxW, boxH, 4);
        ctx.stroke();

        // Text
        ctx.fillStyle = 'rgba(255, 255, 255, 0.85)';
        ctx.font = '12px monospace';
        for (var i = 0; i < lines.length; i++) {
            ctx.fillText(lines[i], boxX + padding, boxY + padding + (i + 1) * lineH - 3);
        }
    }

    function roundRect(ctx, x, y, w, h, r) {
        ctx.moveTo(x + r, y);
        ctx.lineTo(x + w - r, y);
        ctx.arcTo(x + w, y, x + w, y + r, r);
        ctx.lineTo(x + w, y + h - r);
        ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
        ctx.lineTo(x + r, y + h);
        ctx.arcTo(x, y + h, x, y + h - r, r);
        ctx.lineTo(x, y + r);
        ctx.arcTo(x, y, x + r, y, r);
    }

    // Blip tooltip: shows fee rate and tx info for a single blip
    function drawBlipTooltip(ctx, blip, canvasX, baseline) {
        var lines = [
            '~' + (blip.txCount || 1) + ' tx' + ((blip.txCount || 1) > 1 ? 's' : ''),
            'Fee: ~' + (blip.feeRate ? Math.round(blip.feeRate * 10) / 10 : '?') + ' sat/vB',
        ];
        var d = new Date((blip.timestamp || 0) * 1000);
        if (blip.timestamp) {
            lines.push(d.toLocaleTimeString());
        }

        var padding = 6;
        var lineH = 15;
        ctx.font = '11px monospace';
        var maxW = 0;
        for (var i = 0; i < lines.length; i++) {
            var m = ctx.measureText(lines[i]).width;
            if (m > maxW) maxW = m;
        }
        var boxW = maxW + padding * 2;
        var boxH = lines.length * lineH + padding * 2;
        var boxX = canvasX - boxW / 2;
        var boxY = baseline - blip.height - boxH - 12;
        if (boxX < 4) boxX = 4;
        if (boxX + boxW > _hb.width - 4) boxX = _hb.width - boxW - 4;
        if (boxY < 4) boxY = 4;

        ctx.fillStyle = 'rgba(10, 25, 41, 0.92)';
        ctx.beginPath();
        roundRect(ctx, boxX, boxY, boxW, boxH, 4);
        ctx.fill();
        var blipColor = blip.color ? blip.color + '0.6)' : 'rgba(0,230,118,0.6)';
        ctx.strokeStyle = blipColor;
        ctx.lineWidth = 1;
        ctx.beginPath();
        roundRect(ctx, boxX, boxY, boxW, boxH, 4);
        ctx.stroke();

        ctx.fillStyle = 'rgba(255, 255, 255, 0.8)';
        ctx.font = '11px monospace';
        for (var i = 0; i < lines.length; i++) {
            ctx.fillText(lines[i], boxX + padding, boxY + padding + (i + 1) * lineH - 3);
        }
    }

    // Flatline tooltip: shows interval between surrounding blocks
    function drawFlatlineTooltip(ctx, info, canvasX, baseline) {
        var lines = [];
        if (info.prevBlock && info.nextBlock) {
            var interval = info.nextBlock.timestamp - info.prevBlock.timestamp;
            lines.push('Block #' + info.prevBlock.height + ' \u2192 #' + info.nextBlock.height);
            lines.push('Interval: ' + formatDuration(interval));
        } else if (info.prevBlock && !info.nextBlock) {
            // Live flatline - show time since last block + tx count
            var elapsed = Math.floor(Date.now() / 1000 - info.prevBlock.timestamp);
            var totalTxs = 0;
            if (info.segment.blips) {
                for (var t = 0; t < info.segment.blips.length; t++) {
                    totalTxs += info.segment.blips[t].txCount || 1;
                }
            }
            lines.push('After block #' + info.prevBlock.height);
            lines.push('Waiting: ' + formatDuration(elapsed));
            if (totalTxs > 0) lines.push('~' + totalTxs.toLocaleString() + ' new txs in mempool');
        } else {
            lines.push('Waiting for blocks...');
        }
        lines = lines.filter(function(l) { return l.length > 0; });

        var padding = 8;
        var lineH = 16;
        ctx.font = '12px monospace';
        var maxW = 0;
        for (var i = 0; i < lines.length; i++) {
            var m = ctx.measureText(lines[i]).width;
            if (m > maxW) maxW = m;
        }

        var boxW = maxW + padding * 2;
        var boxH = lines.length * lineH + padding * 2;
        var boxX = canvasX - boxW / 2;
        var boxY = baseline - boxH - 20;

        if (boxX < 4) boxX = 4;
        if (boxX + boxW > _hb.width - 4) boxX = _hb.width - boxW - 4;
        if (boxY < 4) boxY = 4;

        ctx.fillStyle = 'rgba(10, 25, 41, 0.92)';
        ctx.beginPath();
        roundRect(ctx, boxX, boxY, boxW, boxH, 4);
        ctx.fill();

        ctx.strokeStyle = 'rgba(255, 255, 255, 0.15)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        roundRect(ctx, boxX, boxY, boxW, boxH, 4);
        ctx.stroke();

        ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
        ctx.font = '12px monospace';
        for (var i = 0; i < lines.length; i++) {
            ctx.fillText(lines[i], boxX + padding, boxY + padding + (i + 1) * lineH - 3);
        }
    }

    function formatDuration(sec) {
        if (sec < 60) return sec + 's';
        var m = Math.floor(sec / 60);
        var s = sec % 60;
        if (m < 60) return m + 'm ' + s + 's';
        var h = Math.floor(m / 60);
        m = m % 60;
        return h + 'h ' + m + 'm';
    }

    // ── "Jump to Live" button ──────────────────────────────────
    function drawJumpToLive(ctx, w, h) {
        var btnW = 110, btnH = 28;
        var btnX = w - btnW - 12;
        var btnY = 12;

        ctx.fillStyle = 'rgba(0, 230, 118, 0.15)';
        ctx.beginPath();
        roundRect(ctx, btnX, btnY, btnW, btnH, 4);
        ctx.fill();

        ctx.strokeStyle = COLORS.healthy;
        ctx.lineWidth = 1;
        ctx.beginPath();
        roundRect(ctx, btnX, btnY, btnW, btnH, 4);
        ctx.stroke();

        ctx.fillStyle = COLORS.healthy;
        ctx.font = 'bold 12px monospace';
        ctx.textAlign = 'center';
        ctx.fillText('Jump to Live', btnX + btnW / 2, btnY + 18);
        ctx.textAlign = 'left';

        // Store button bounds for click detection
        _hb._jumpBtn = { x: btnX, y: btnY, w: btnW, h: btnH };
    }

    // ── Main draw loop ─────────────────────────────────────────
    function drawFrame(frameTime) {
        if (!_hb) return;

        var ctx = _hb.ctx;
        var w = _hb.width;
        var h = _hb.height;
        var baseline = h * 0.55;

        // Compute time delta
        var now = Date.now() / 1000;
        var dt = _hb.lastFrameTime > 0 ? (now - _hb.lastFrameTime) : 0;
        if (dt > 0.5) dt = 0.5; // clamp to avoid huge jumps on tab switch
        _hb.lastFrameTime = now;

        // ── Advance live flatline ──────────────────────────────
        var liveSeg = _hb.timeline.length > 0 ? _hb.timeline[_hb.timeline.length - 1] : null;
        if (liveSeg && liveSeg.type === 'flatline' && liveSeg.x_end === null) {
            // Grow at 5 virtual pixels per second (10min block = ~3000px of flatline)
            _hb.virtualX += dt * 5.0;
        }

        // If auto-following, keep viewport pinned to the head (zoom-aware)
        if (_hb.autoFollow) {
            // Place the head at 85% of canvas width in virtual coords
            _hb.viewOffset = _hb.virtualX - (w * 0.85) / _hb.zoom;
        }

        // ── Compute current color from live state ──────────────
        var elapsed = _hb.lastBlockTime > 0 ? now - _hb.lastBlockTime : 0;
        var targetColor = computeColor(elapsed, _hb.nextBlockFee, _hb.mempoolMB);
        // Check for color change BEFORE advancing lerp to avoid state corruption
        if (targetColor !== _hb.targetColor) {
            _hb.prevColor = _hb.currentColor;
            _hb.targetColor = targetColor;
            _hb.colorLerp = 0;
        } else {
            _hb.colorLerp = Math.min(_hb.colorLerp + 0.02, 1);
            if (_hb.colorLerp >= 1) {
                _hb.currentColor = targetColor;
                _hb.prevColor = targetColor;
            }
        }
        var liveColor = _hb.colorLerp < 1
            ? lerpColor(_hb.prevColor, _hb.targetColor, _hb.colorLerp)
            : _hb.currentColor;

        // ── Clear and draw background ──────────────────────────
        ctx.fillStyle = BG_COLOR;
        ctx.fillRect(0, 0, w, h);
        drawGrid(ctx, w, h);

        // ── Visible virtual x range (zoom-aware) ────────────────
        var viewLeft = _hb.viewOffset;
        var viewRight = _hb.viewOffset + w / _hb.zoom;

        // ── Draw timeline segments ─────────────────────────────
        // Smooth organic tremor on long waits (sine-based, not random)
        var jitter = 0;
        if (elapsed > 600) {
            // Gradually increases: starts subtle at 10min, more pronounced at 30min+
            var intensity = Math.min((elapsed - 600) / 1200, 1.0) * 2.5;
            // Layered sine waves for organic feel (not mechanical)
            jitter = Math.sin(now * 2.3) * intensity
                   + Math.sin(now * 5.7) * intensity * 0.4
                   + Math.sin(now * 0.8) * intensity * 0.6;
        }

        for (var si = 0; si < _hb.timeline.length; si++) {
            var seg = _hb.timeline[si];

            // Determine segment x range
            var segStart = seg.x_start;
            var segEnd;
            if (seg.type === 'block') {
                segEnd = seg.x_end;
            } else {
                // Flatline: x_end is null for live segment
                segEnd = (seg.x_end !== null) ? seg.x_end : _hb.virtualX;
            }

            // Skip segments outside visible range
            if (segEnd < viewLeft || segStart > viewRight) continue;

            if (seg.type === 'block') {
                drawBlockSegment(ctx, seg, viewLeft, baseline, liveColor);
            } else {
                var flatColor = (seg.x_end === null) ? liveColor : (seg.color || COLORS.healthy);
                drawFlatlineSegment(ctx, seg, segEnd, viewLeft, viewRight, baseline, flatColor, jitter, seg.x_end === null);
            }
        }

        // ── Draw tooltip if hovering ───────────────────────────
        if (_hb.hoveredBlock) {
            var hSeg = _hb.hoveredBlock;
            var midX = virtualToCanvas((hSeg.x_start + hSeg.x_end) / 2);
            drawTooltip(ctx, hSeg, midX, baseline - 30, baseline);
        } else if (_hb.hoveredBlip) {
            drawBlipTooltip(ctx, _hb.hoveredBlip, _hb.hoverCanvasX, baseline);
        } else if (_hb.hoveredFlatline) {
            drawFlatlineTooltip(ctx, _hb.hoveredFlatline, _hb.hoverCanvasX || w / 2, baseline);
        }

        // ── Draw zoom indicator ────────────────────────────────
        if (_hb.zoom !== 1.0) {
            ctx.font = '11px monospace';
            ctx.fillStyle = 'rgba(255, 255, 255, 0.3)';
            ctx.textAlign = 'left';
            ctx.fillText(_hb.zoom.toFixed(1) + 'x', 10, h - 10);
        }

        // ── Draw "Jump to Live" if not auto-following ──────────
        if (!_hb.autoFollow) {
            drawJumpToLive(ctx, w, h);
        } else {
            _hb._jumpBtn = null;
        }

        // ── Draw live indicator dot ────────────────────────────
        if (_hb.autoFollow && liveSeg && liveSeg.x_end === null) {
            var dotX = virtualToCanvas(_hb.virtualX);
            if (dotX >= 0 && dotX <= w) {
                var dotPulse = 0.5 + 0.5 * Math.sin(now * 3);
                ctx.beginPath();
                ctx.arc(dotX, baseline, 3, 0, Math.PI * 2);
                ctx.fillStyle = liveColor;
                ctx.globalAlpha = 0.4 + dotPulse * 0.4;
                ctx.fill();
                ctx.globalAlpha = 1;
            }
        }

        _hb.rafId = requestAnimationFrame(drawFrame);
    }

    // Draw a single block waveform segment
    function drawBlockSegment(ctx, seg, viewLeft, baseline, fallbackColor) {
        var pts = seg.points;
        var color = seg.color || fallbackColor;
        var zoom = _hb.zoom;

        ctx.beginPath();
        ctx.strokeStyle = color;
        ctx.lineWidth = 2;
        ctx.shadowBlur = 10;
        ctx.shadowColor = color;

        var isHovered = (_hb.hoveredBlock === seg);
        if (isHovered) {
            ctx.lineWidth = 3;
            ctx.shadowBlur = 18;
        }

        for (var i = 0; i < pts.length; i++) {
            var vx = seg.x_start + i * POINT_WIDTH;
            var cx = virtualToCanvas(vx);
            var cy = baseline + pts[i];
            if (i === 0) {
                ctx.moveTo(cx, cy);
            } else {
                ctx.lineTo(cx, cy);
            }
        }
        ctx.stroke();
        ctx.shadowBlur = 0;
    }

    // Draw a flatline segment (with optional blips)
    function drawFlatlineSegment(ctx, seg, segEnd, viewLeft, viewRight, baseline, color, jitter, isLive) {
        // Clamp to visible range
        var drawStart = Math.max(seg.x_start, viewLeft);
        var drawEnd = Math.min(segEnd, viewRight);

        if (drawStart >= drawEnd) return;

        var cx1 = virtualToCanvas(drawStart);
        var cx2 = virtualToCanvas(drawEnd);
        var y = baseline + (isLive ? jitter : 0);

        ctx.beginPath();
        ctx.moveTo(cx1, y);
        ctx.lineTo(cx2, y);
        ctx.strokeStyle = color;
        ctx.lineWidth = 2;
        ctx.shadowBlur = 8;
        ctx.shadowColor = color;
        ctx.stroke();
        ctx.shadowBlur = 0;

        // Draw blips
        if (seg.blips && seg.blips.length > 0) {
            var zoom = _hb.zoom;
            for (var bi = 0; bi < seg.blips.length; bi++) {
                var blip = seg.blips[bi];
                // Skip blips outside visible range
                if (blip.x < viewLeft || blip.x > viewRight) continue;

                var bx = virtualToCanvas(blip.x);
                var bOpacity = blip.opacity;
                var now = Date.now() / 1000;

                // Fade out confirmed blips fast (block arrived)
                if (blip.fadeStart > 0) {
                    var fadeDt = now - blip.fadeStart;
                    bOpacity = Math.max(0, blip.opacity - fadeDt * 0.5);
                    if (bOpacity <= 0) continue;
                }

                var blipColor = blip.color || 'rgba(0, 230, 118, ';
                ctx.beginPath();
                ctx.moveTo(bx, baseline);
                ctx.lineTo(bx, baseline - blip.height);
                ctx.strokeStyle = blipColor + bOpacity + ')';
                // Widen blips when zoomed in for easier interaction
                ctx.lineWidth = Math.min(2 + (zoom - 1) * 1.5, 8);
                ctx.shadowBlur = 8;
                ctx.shadowColor = blipColor + (bOpacity * 0.5) + ')';
                ctx.stroke();
                ctx.shadowBlur = 0;
            }

            // Prune fully faded blips to prevent unbounded memory growth
            if (seg.blips.length > 300) {
                var cutoff = Date.now() / 1000;
                seg.blips = seg.blips.filter(function(b) {
                    if (b.fadeStart > 0) return (cutoff - b.fadeStart) < 3;
                    return (cutoff - b.timestamp) < 300; // 5 min max age
                });
            }
        }
    }

    // ── Input handling ─────────────────────────────────────────

    function setupInputHandlers(canvas) {
        // Track listeners for cleanup in destroyHeartbeat
        _hb._listeners = [];
        function listen(target, evt, fn, opts) {
            target.addEventListener(evt, fn, opts);
            _hb._listeners.push({ target: target, evt: evt, fn: fn, opts: opts });
        }

        // Mouse wheel: zoom (centered on cursor). Shift+wheel: pan.
        listen(canvas, 'wheel', function(e) {
            if (!_hb) return;
            e.preventDefault();

            if (e.shiftKey) {
                // Shift+wheel: horizontal pan
                _hb.viewOffset += e.deltaY * 0.8 / _hb.zoom;
                _hb.autoFollow = false;
                checkAutoFollow();
            } else {
                // Wheel: zoom centered on cursor position
                var rect = canvas.getBoundingClientRect();
                var mx = e.clientX - rect.left;
                // Virtual x under the cursor before zoom
                var vxUnderCursor = canvasToVirtual(mx);

                // Apply zoom
                var zoomDelta = e.deltaY > 0 ? 0.85 : 1.18; // scroll down = zoom out
                _hb.zoom = Math.max(_hb.minZoom, Math.min(_hb.maxZoom, _hb.zoom * zoomDelta));

                // Adjust viewOffset so the virtual point under cursor stays put
                _hb.viewOffset = vxUnderCursor - mx / _hb.zoom;

                _hb.autoFollow = false;
                checkAutoFollow();
            }
        }, { passive: false });

        // Mouse drag: click and drag to pan
        listen(canvas, 'mousedown', function(e) {
            if (!_hb) return;

            // Check "Jump to Live" button click
            if (_hb._jumpBtn) {
                var rect = canvas.getBoundingClientRect();
                var mx = e.clientX - rect.left;
                var my = e.clientY - rect.top;
                var btn = _hb._jumpBtn;
                if (mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h) {
                    _hb.autoFollow = true;
                    _hb.zoom = 1.0;
                    _hb.hoveredBlock = null;
                    return;
                }
            }

            _hb.isDragging = true;
            _hb.dragStartX = e.clientX;
            _hb.dragStartOffset = _hb.viewOffset;
            canvas.style.cursor = 'grabbing';
        });

        listen(window, 'mousemove', function(e) {
            if (!_hb) return;

            if (_hb.isDragging) {
                var dx = e.clientX - _hb.dragStartX;
                _hb.viewOffset = _hb.dragStartOffset - dx / _hb.zoom;
                _hb.autoFollow = false;
                checkAutoFollow();
            } else {
                // Hover detection
                var rect = canvas.getBoundingClientRect();
                var mx = e.clientX - rect.left;
                var my = e.clientY - rect.top;
                if (mx >= 0 && mx <= _hb.width && my >= 0 && my <= _hb.height) {
                    var vx = canvasToVirtual(mx);
                    var baseline = _hb.height * 0.55;
                    _hb.hoveredBlock = blockAtVirtualX(vx);
                    _hb.hoveredBlip = null;
                    _hb.hoveredFlatline = null;

                    if (!_hb.hoveredBlock) {
                        // Check for blip hover (above flatline, when zoomed in)
                        if (my < baseline && _hb.zoom >= 1.5) {
                            _hb.hoveredBlip = blipAtVirtualX(vx, 5 / _hb.zoom);
                        }
                        if (!_hb.hoveredBlip) {
                            _hb.hoveredFlatline = flatlineAtVirtualX(vx);
                        }
                    }
                    _hb.hoverCanvasX = mx;
                    _hb.hoverCanvasY = my;
                    canvas.style.cursor = _hb.hoveredBlock ? 'pointer'
                        : _hb.hoveredBlip ? 'pointer'
                        : _hb.hoveredFlatline ? 'crosshair' : 'default';
                } else {
                    _hb.hoveredBlock = null;
                    _hb.hoveredBlip = null;
                    _hb.hoveredFlatline = null;
                }
            }
        });

        listen(window, 'mouseup', function() {
            if (!_hb) return;
            if (_hb.isDragging) {
                _hb.isDragging = false;
                canvas.style.cursor = 'default';
            }
        });

        // Click: dispatch block-click event
        listen(canvas, 'click', function(e) {
            if (!_hb) return;

            // Jump to live button
            if (_hb._jumpBtn) {
                var rect = canvas.getBoundingClientRect();
                var mx = e.clientX - rect.left;
                var my = e.clientY - rect.top;
                var btn = _hb._jumpBtn;
                if (mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h) {
                    _hb.autoFollow = true;
                    _hb.zoom = 1.0;
                    _hb.hoveredBlock = null;
                    return;
                }
            }

            if (_hb.hoveredBlock) {
                window.dispatchEvent(new CustomEvent('heartbeat-block-click', {
                    detail: {
                        height: _hb.hoveredBlock.height,
                        timestamp: _hb.hoveredBlock.timestamp,
                        tx_count: _hb.hoveredBlock.tx_count,
                        total_fees: _hb.hoveredBlock.total_fees,
                        weight: _hb.hoveredBlock.weight,
                        inter_block_seconds: _hb.hoveredBlock.inter_block_seconds
                    }
                }));
            }
        });

        // Touch support for mobile
        listen(canvas, 'touchstart', function(e) {
            if (!_hb || e.touches.length !== 1) return;
            _hb.isDragging = true;
            _hb.dragStartX = e.touches[0].clientX;
            _hb.dragStartOffset = _hb.viewOffset;
        }, { passive: true });

        listen(canvas, 'touchmove', function(e) {
            if (!_hb || !_hb.isDragging) return;
            e.preventDefault();
            var dx = e.touches[0].clientX - _hb.dragStartX;
            _hb.viewOffset = _hb.dragStartOffset - dx / _hb.zoom;
            _hb.autoFollow = false;
            checkAutoFollow();
        }, { passive: false });

        listen(canvas, 'touchend', function(e) {
            if (!_hb) return;
            if (_hb.isDragging) {
                _hb.isDragging = false;

                // Tap detection (minimal drag distance = click)
                if (e.changedTouches && e.changedTouches.length > 0) {
                    var dx = Math.abs(e.changedTouches[0].clientX - _hb.dragStartX);
                    if (dx < 10) {
                        // Treat as tap
                        var rect = canvas.getBoundingClientRect();
                        var mx = e.changedTouches[0].clientX - rect.left;
                        var vx = canvasToVirtual(mx);
                        var tapped = blockAtVirtualX(vx);

                        // Check jump button
                        if (_hb._jumpBtn) {
                            var my = e.changedTouches[0].clientY - rect.top;
                            var btn = _hb._jumpBtn;
                            if (mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h) {
                                _hb.autoFollow = true;
                    _hb.zoom = 1.0;
                                _hb.hoveredBlock = null;
                                return;
                            }
                        }

                        if (tapped) {
                            _hb.hoveredBlock = (_hb.hoveredBlock === tapped) ? null : tapped;
                            if (_hb.hoveredBlock) {
                                window.dispatchEvent(new CustomEvent('heartbeat-block-click', {
                                    detail: {
                                        height: tapped.height,
                                        timestamp: tapped.timestamp,
                                        tx_count: tapped.tx_count,
                                        total_fees: tapped.total_fees,
                                        weight: tapped.weight,
                                        inter_block_seconds: tapped.inter_block_seconds
                                    }
                                }));
                            }
                        } else {
                            _hb.hoveredBlock = null;
                        }
                    }
                }
            }
        });
    }

    // Check if user has scrolled close enough to the live head to re-enable auto-follow
    function checkAutoFollow() {
        if (!_hb) return;
        var headOnCanvas = virtualToCanvas(_hb.virtualX);
        // If the head is visible and near the right edge, snap back to auto-follow
        // but preserve the user's zoom level
        if (headOnCanvas >= _hb.width * 0.7 && headOnCanvas <= _hb.width + 80) {
            _hb.autoFollow = true;
        }
    }

    // ── Public API ─────────────────────────────────────────────

    window.initHeartbeat = function(canvasId) {
        if (_hb) window.destroyHeartbeat();

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

        // Fill background and draw grid
        ctx.fillStyle = BG_COLOR;
        ctx.fillRect(0, 0, w, h);
        drawGrid(ctx, w, h);

        _hb = {
            canvas: canvas,
            ctx: ctx,
            width: w,
            height: h,
            dpr: dpr,

            // Virtual timeline
            timeline: [],
            virtualX: 0,
            viewOffset: -w * 0.85,  // start so that x=0 is near the right
            autoFollow: true,
            zoom: 1.0,              // 1.0 = normal, >1 = zoomed in, <1 = zoomed out
            minZoom: 0.1,
            maxZoom: 8.0,

            // Rendering
            rafId: null,
            lastFrameTime: 0,

            // Live state
            lastBlockTime: 0,
            nextBlockFee: 0,
            mempoolMB: 0,
            mempoolMinFee: 0,
            hashrateEH: 0,
            difficulty: 0,
            blockHeight: 0,
            currentColor: COLORS.healthy,
            prevColor: COLORS.healthy,
            targetColor: COLORS.healthy,
            colorLerp: 1,

            // Interaction
            isDragging: false,
            dragStartX: 0,
            dragStartOffset: 0,
            hoveredBlock: null,
            hoveredBlip: null,
            hoveredFlatline: null,
            hoverCanvasX: 0,
            hoverCanvasY: 0,
            _jumpBtn: null,

            // Mempool feed
            ws: null,
            wsConnected: false,

            // Recent blocks (kept for vital signs / rhythm strip)
            recentBlocks: []
        };

        // Start with a live flatline at position 0
        _hb.timeline.push(createFlatlineSegment(0, null));

        // Setup input handlers
        setupInputHandlers(canvas);

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
            });
            _hb.resizeObs.observe(container);
        }

        // Start animation loop
        _hb.rafId = requestAnimationFrame(drawFrame);

        // Connect mempool WebSocket feed
        connectMempoolFeed();
    };

    window.pushHeartbeatBlocks = function(json, replay) {
        if (!_hb) { console.warn('heartbeat: push called but _hb is null'); return; }
        var isReplay = !!replay;
        try {
            var blocks = JSON.parse(json);
            if (!Array.isArray(blocks)) return;
            console.log('heartbeat: pushing ' + blocks.length + ' block(s), replay=' + isReplay,
                blocks.length > 0 ? 'height=' + blocks[0].height : '');

            for (var i = 0; i < blocks.length; i++) {
                var b = blocks[i];
                var interBlock = b.inter_block_seconds || 600;
                // For live blocks, compute real interval from the previous block's timestamp
                // Use _prevBlockTime (set before LiveStats overwrites lastBlockTime)
                if (!isReplay && _hb._prevBlockTime > 0 && b.timestamp > _hb._prevBlockTime) {
                    interBlock = b.timestamp - _hb._prevBlockTime;
                } else if (!isReplay && _hb.lastBlockTime > 0 && b.timestamp > _hb.lastBlockTime) {
                    interBlock = b.timestamp - _hb.lastBlockTime;
                }

                // Close the current live flatline
                var lastSeg = _hb.timeline[_hb.timeline.length - 1];
                if (lastSeg && lastSeg.type === 'flatline' && lastSeg.x_end === null) {
                    if (isReplay) {
                        // For replay, use compressed flatline width based on inter-block time
                        lastSeg.x_end = lastSeg.x_start + historyFlatlineWidth(interBlock);
                        _hb.virtualX = lastSeg.x_end;
                        // Color based on inter-block time for replay
                        var feeRate = b.total_fees ? b.total_fees / 100000 : 0;
                        lastSeg.color = computeColor(interBlock, feeRate, 0);
                    } else {
                        // For live, close at current virtual head position
                        lastSeg.x_end = _hb.virtualX;
                    }
                    // Stamp the flatline with its color at close time (only for live, replay sets its own)
                    if (!isReplay) {
                        lastSeg.color = _hb.currentColor || COLORS.healthy;
                    }
                    // Mark existing blips for fade-out (they got confirmed)
                    if (lastSeg.blips) {
                        var fadeNow = Date.now() / 1000;
                        for (var bi = 0; bi < lastSeg.blips.length; bi++) {
                            if (lastSeg.blips[bi].fadeStart === 0) {
                                lastSeg.blips[bi].fadeStart = fadeNow;
                            }
                        }
                    }
                }

                // Create block segment with corrected inter-block time
                b.inter_block_seconds = interBlock;
                var blockSeg = createBlockSegment(b, _hb.virtualX);
                _hb.timeline.push(blockSeg);
                _hb.virtualX = blockSeg.x_end;

                // Update last block time
                if (b.timestamp) {
                    _hb.lastBlockTime = b.timestamp;
                }

                // Create a new live flatline after this block
                _hb.timeline.push(createFlatlineSegment(_hb.virtualX, null));

                // Maintain recentBlocks list (up to 2016 for period history)
                _hb.recentBlocks.push({
                    timestamp: b.timestamp,
                    height: b.height,
                    tx_count: b.tx_count,
                    total_fees: b.total_fees,
                    size: b.size,
                    weight: b.weight,
                    inter_block_seconds: b.inter_block_seconds
                });
            }

            // Trim recentBlocks to last 2016
            if (_hb.recentBlocks.length > 2016) {
                _hb.recentBlocks = _hb.recentBlocks.slice(-2016);
            }

            // Prune very old timeline segments to avoid unbounded memory growth.
            // Keep at least enough segments to fill 3x the canvas width behind the
            // current viewport, which is plenty for casual scrolling. If the user
            // scrolls further back after pruning, they just see a flat start.
            pruneTimeline();

            // After replay: fast-forward the live flatline to reflect time since last block
            if (isReplay && blocks.length > 0 && _hb.lastBlockTime > 0) {
                var nowSec = Date.now() / 1000;
                var elapsed = nowSec - _hb.lastBlockTime;
                if (elapsed > 0 && elapsed < 7200) { // cap at 2 hours
                    // Advance virtualX to represent the elapsed time
                    var advancePx = elapsed * 5.0; // same rate as live (5px/sec)
                    _hb.virtualX += advancePx;

                    // Scatter some synthetic blips across the gap to represent
                    // mempool activity that happened while we weren't watching
                    var liveSeg = _hb.timeline[_hb.timeline.length - 1];
                    if (liveSeg && liveSeg.type === 'flatline') {
                        var numBlips = Math.min(Math.floor(elapsed / 0.5), 1500); // ~2 blips per second, cap 1500
                        for (var bi = 0; bi < numBlips; bi++) {
                            var frac = bi / numBlips;
                            // Random position across the gap (not evenly spaced)
                            var blipX = liveSeg.x_start + Math.random() * advancePx;
                            var feeRoll = Math.random();
                            var color;
                            if (feeRoll < 0.5) color = 'rgba(0, 230, 118, ';
                            else if (feeRoll < 0.8) color = 'rgba(66, 165, 245, ';
                            else if (feeRoll < 0.95) color = 'rgba(247, 147, 26, ';
                            else color = 'rgba(255, 87, 34, ';
                            liveSeg.blips.push({
                                x: blipX,
                                height: 6 + Math.random() * 30,
                                color: color,
                                opacity: 0.3 + Math.random() * 0.3, // dimmer (synthetic)
                                txCount: Math.ceil(Math.random() * 10),
                                feeRate: Math.round((1 + Math.random() * 5) * 10) / 10,
                                timestamp: _hb.lastBlockTime + frac * elapsed,
                                fadeStart: 0
                            });
                        }
                    }
                    console.log('heartbeat: fast-forwarded ' + Math.round(elapsed) + 's (' + Math.round(advancePx) + 'px), ' + numBlips + ' synthetic blips');
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

    // Prune timeline segments that are far behind the viewport to limit memory
    function pruneTimeline() {
        if (!_hb || _hb.timeline.length < 200) return;
        var minX = _hb.viewOffset - _hb.width * 3;
        var cutIdx = 0;
        for (var i = 0; i < _hb.timeline.length; i++) {
            var seg = _hb.timeline[i];
            var segEnd = seg.type === 'block' ? seg.x_end : (seg.x_end !== null ? seg.x_end : _hb.virtualX);
            if (segEnd < minX) {
                cutIdx = i;
            } else {
                break;
            }
        }
        if (cutIdx > 0) {
            _hb.timeline = _hb.timeline.slice(cutIdx);
        }
    }

    window.updateHeartbeatLive = function(json) {
        if (!_hb) return;
        try {
            var data = JSON.parse(json);
            if (data.next_block_fee !== undefined) _hb.nextBlockFee = data.next_block_fee;
            if (data.mempool_mb !== undefined) _hb.mempoolMB = data.mempool_mb;
            if (data.block_time !== undefined && data.block_time > _hb.lastBlockTime) {
                // Store previous block time before overwriting (needed for inter-block calc)
                _hb._prevBlockTime = _hb.lastBlockTime;
                _hb.lastBlockTime = data.block_time;
            }
            if (data.hashrate_eh !== undefined) _hb.hashrateEH = data.hashrate_eh;
            if (data.mempool_min_fee !== undefined) _hb.mempoolMinFee = data.mempool_min_fee;
            if (data.difficulty !== undefined) _hb.difficulty = data.difficulty;
            if (data.block_height !== undefined) _hb.blockHeight = data.block_height;
        } catch (e) {
            console.warn('heartbeat: bad live json', e);
        }
    };

    window.destroyHeartbeat = function() {
        if (!_hb) return;
        if (_hb.rafId) cancelAnimationFrame(_hb.rafId);
        if (_hb.resizeObs) _hb.resizeObs.disconnect();
        if (_hb.ws) {
            try { _hb.ws.close(); } catch(e) {}
        }
        // Remove all event listeners registered via listen()
        if (_hb._listeners) {
            for (var i = 0; i < _hb._listeners.length; i++) {
                var l = _hb._listeners[i];
                l.target.removeEventListener(l.evt, l.fn, l.opts);
            }
        }
        _hb = null;
    };

    // ── Phase 2: Vital Signs computation ─────────────────────

    // Store recent block timestamps for heart rate calculation
    // _hb.recentBlocks = [{timestamp, height}, ...]

    // Compute blocks per 10 minutes from the last ~20 blocks (~3hr rolling window)
    function computeHeartRate() {
        if (!_hb || !_hb.recentBlocks || _hb.recentBlocks.length < 2) {
            return { bpm: 1.0, label: 'Normal', color: COLORS.healthy };
        }
        // Use last 20 blocks for a sensitive, recent reading
        var allBlocks = _hb.recentBlocks;
        var blocks = allBlocks.length > 20 ? allBlocks.slice(-20) : allBlocks;
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

        // Draw each block segment with its own color
        var x = 0;
        var vScale = 0.4; // compress vertically
        for (var wi = 0; wi < waveforms.length; wi++) {
            var wf = waveforms[wi];
            var interBlock = wf.block.inter_block_seconds || 600;

            // Color based on inter-block time and fee pressure
            var feeRate = wf.block.total_fees ? wf.block.total_fees / 100000 : 0;
            var segColor = computeColor(interBlock, feeRate, 0);

            ctx.beginPath();
            ctx.moveTo(x, baseline);

            // Draw flatline before waveform
            var flatEnd = x + wf.flatline * scale;
            ctx.lineTo(flatEnd, baseline);
            x = flatEnd;

            // Draw waveform points
            for (var pi = 0; pi < wf.points.length; pi++) {
                x += scale;
                ctx.lineTo(x, baseline + wf.points[pi] * vScale);
            }

            ctx.strokeStyle = segColor;
            ctx.lineWidth = 1;
            ctx.shadowBlur = 4;
            ctx.shadowColor = segColor;
            ctx.stroke();
            ctx.shadowBlur = 0;
        }

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
        console.log('heartbeat: playSound called, enabled=' + _soundEnabled + ', ctx=' + (_audioCtx ? _audioCtx.state : 'null'));
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
        var blockTxt = 'Block #' + (_hb.blockHeight || (_hb.recentBlocks && _hb.recentBlocks.length > 0
            ? _hb.recentBlocks[_hb.recentBlocks.length - 1].height
            : '---'));
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

    // Get recent blocks JSON for rhythm strip
    window.getHeartbeatRecentBlocks = function() {
        if (!_hb || !_hb.recentBlocks) return '[]';
        // Return last 144 blocks for the 24-hour rhythm strip
        var blocks = _hb.recentBlocks;
        if (blocks.length > 144) {
            blocks = blocks.slice(-144);
        }
        return JSON.stringify(blocks);
    };

})();
