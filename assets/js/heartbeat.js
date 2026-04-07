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
        steady:   '#42a5f5',
        elevated: '#f7931a',
        stressed: '#ff5722',
        critical: '#f44336'
    };

    var BG_COLOR = 'rgba(13, 33, 55, 1)'; // matches bg-[#0d2137]
    var GRID_COLOR = 'rgba(255, 255, 255, 0.03)';

    // ── Thresholds & dimensions ───────────────────────────────
    var POINT_WIDTH = 1.5;          // virtual pixels per PQRST point
    var FLATLINE_PX_PER_SEC = 5.0;  // live flatline advance rate
    var HEAD_POSITION_FRAC = 0.85;  // viewport fraction for live head
    var MAX_FLATLINE_WIDTH = 300;   // max compressed flatline px
    var RETARGET_PERIOD = 2016;     // blocks per difficulty epoch
    var RHYTHM_STRIP_BLOCKS = 144;  // blocks in 24hr strip
    var MAX_BLIPS_PER_SEGMENT = 5000; // prune threshold (off-screen only)

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
        var waitNorm = Math.min(interBlock / 1800, 1.0); // 30 min cap
        var fullNorm = Math.min(weight / 4000000, 1.0);

        // Fee normalization: power curve for wider visible spread.
        // pow(x, 0.4) maps: 1M→0.10, 2M→0.15, 5M→0.22, 10M→0.30,
        //                   20M→0.42, 50M→0.63, 100M→0.83, 200M→1.0
        // This spreads the typical 1M-50M range across 0.10-0.63 (vs sqrt 0.14-1.0)
        // giving more dynamic range for the common case while still showing
        // fee spikes (>50M sats) as dramatically tall spikes.
        var feeNorm = fees > 0 ? Math.min(Math.pow(fees / 200000000, 0.4), 1.0) : 0;

        // Waveform amplitudes (pixels)
        var pAmp   = 6 + txNorm * 28;           // P wave: 6-34px
        var qDepth = 8 + waitNorm * 60;          // Q dip: 8-68px
        var rHeight = 15 + feeNorm * 235;         // R spike: 15-250px (lower base, wider range)
        var sDepth = rHeight * 0.45;              // S dip: 45% of R
        var tAmp   = 4 + fullNorm * 30;           // T wave: 4-34px

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
        if (stress < 0.4)  return COLORS.steady;
        if (stress < 0.6)  return COLORS.elevated;
        if (stress < 0.8)  return COLORS.stressed;
        return COLORS.critical;
    }

    function lerp(a, b, t) { return a + (b - a) * t; }

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

    // Fee rate to RGBA color prefix (close with opacity + ')')
    function feeRateColor(feeRate, medianFee) {
        if (feeRate < medianFee * 0.8) return 'rgba(33, 150, 243, ';
        if (feeRate < medianFee * 1.5) return 'rgba(0, 230, 118, ';
        if (feeRate < medianFee * 4)   return 'rgba(255, 152, 0, ';
        return 'rgba(244, 67, 54, ';
    }

    // Compute flatline width in virtual pixels for a completed (history) flatline
    function historyFlatlineWidth(interBlockSeconds) {
        // Proportional: 1min=15px, 5min=75px, 10min=150px, 30min=300px (capped)
        return Math.max(10, Math.min(interBlockSeconds / 4, MAX_FLATLINE_WIDTH));
    }

    // ── Grid drawing ───────────────────────────────────────────
    function drawGrid(ctx, w, h) {
        ctx.beginPath();
        ctx.strokeStyle = GRID_COLOR;
        ctx.lineWidth = 0.5;
        var spacing = 40;
        for (var x = 0; x < w; x += spacing) {
            ctx.moveTo(x, 0);
            ctx.lineTo(x, h);
        }
        for (var y = 0; y < h; y += spacing) {
            ctx.moveTo(0, y);
            ctx.lineTo(w, y);
        }
        ctx.stroke();
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
        var viewLeft = _hb.viewOffset;
        var viewRight = _hb.viewOffset + _hb.width / _hb.zoom;
        for (var i = 0; i < _hb.timeline.length; i++) {
            var seg = _hb.timeline[i];
            if (seg.type !== 'flatline' || !seg.blips) continue;
            // Skip segments outside visible range
            var segEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
            if (segEnd < viewLeft || seg.x_start > viewRight) continue;
            for (var bi = 0; bi < seg.blips.length; bi++) {
                var blip = seg.blips[bi];
                if (blip.fadeStart > 0) continue;
                var dist = Math.abs(blip.x - vx);
                if (dist < bestDist) {
                    bestDist = dist;
                    best = blip;
                }
            }
        }
        return best;
    }

    // 2D hit test: find which brick the cursor is inside (canvas coords)
    function blipAtCanvasXY(cx, cy, baseline) {
        if (!_hb) return null;
        var zoom = _hb.zoom;
        var heightScale = zoom > 4 ? 1 + (zoom - 4) * 0.15 : 1;
        var viewLeft = _hb.viewOffset;
        var viewRight = _hb.viewOffset + _hb.width / zoom;

        for (var i = _hb.timeline.length - 1; i >= 0; i--) {
            var seg = _hb.timeline[i];
            if (seg.type !== 'flatline' || !seg.blips) continue;
            // Skip segments entirely outside visible range
            var segEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
            if (segEnd < viewLeft || seg.x_start > viewRight) continue;
            // Check blips in reverse order (top of stack first)
            for (var bi = seg.blips.length - 1; bi >= 0; bi--) {
                var blip = seg.blips[bi];
                if (blip.fadeStart > 0) continue;
                // Quick x-range check before computing canvas coords
                var bvx = blip.gridX || blip.x;
                if (bvx < viewLeft || bvx > viewRight) continue;

                var bx = virtualToCanvas(bvx);
                var bw = (blip.brickW || 4) * zoom;
                var bh = (blip.brickH || 10) * heightScale;
                var sy = (blip.stackY || 0) * heightScale;

                var left = bx - bw / 2;
                var top = baseline - sy - bh;
                var right = left + bw;
                var bottom = top + bh;

                if (cx >= left && cx <= right && cy >= top && cy <= bottom) {
                    return blip;
                }
            }
        }
        return null;
    }

    // Place SSE history txs on the live flatline
    function placeHistoryTxs(txs, lastBlockTs) {
        if (!_hb) return;
        var liveSeg = _hb.timeline[_hb.timeline.length - 1];
        if (!liveSeg || liveSeg.type !== 'flatline' || liveSeg.x_end !== null) return;

        // Prefer the SSE's lastBlockTs (real timestamp from DB) over the replay's
        // lastBlockTime (synthetic, can drift). Update lastBlockTime so the rest
        // of the system (color computation, vital signs) uses the real value.
        var effectiveBlockTs = lastBlockTs || _hb.lastBlockTime;
        if (effectiveBlockTs > 0 && effectiveBlockTs !== _hb.lastBlockTime) {
            _hb.lastBlockTime = effectiveBlockTs;
        }

        // Fast-forward virtualX to match real elapsed time since last block.
        // On page load, replay creates compressed flatlines, so virtualX is only
        // a few pixels past the last block. But real time may have passed (e.g. 10min).
        // Without this, history txs cluster at the head because flatlineSpan is tiny.
        var nowSec = Date.now() / 1000;
        var elapsedSinceBlock = effectiveBlockTs > 0 ? nowSec - effectiveBlockTs : 0;
        if (elapsedSinceBlock > 0) {
            var neededX = liveSeg.x_start + elapsedSinceBlock * FLATLINE_PX_PER_SEC;
            if (neededX > _hb.virtualX) {
                console.log('[heartbeat] fast-forwarding virtualX for history:',
                    Math.round(_hb.virtualX), '->', Math.round(neededX),
                    '(' + Math.round(elapsedSinceBlock) + 's since last block)');
                _hb.virtualX = neededX;
            }
        }

        // Compute median fee from history txs so colors match live bricks
        var historyRates = [];
        for (var hi = 0; hi < txs.length; hi++) {
            if (txs[hi].fee && txs[hi].vsize) {
                historyRates.push(txs[hi].fee / txs[hi].vsize);
            }
        }
        if (historyRates.length >= 20) {
            historyRates.sort(function(a, b) { return a - b; });
            _hb._wsMedianFee = Math.max(1, historyRates[Math.floor(historyRates.length / 2)]);
        }
        var medianFee = _hb._wsMedianFee || 5;
        var placed = 0;
        var maxBricks = 5000; // match server-side SSE history limit

        var flatlineSpan = _hb.virtualX - liveSeg.x_start;
        var stackMap = {};

        console.log('[heartbeat] placeHistoryTxs: flatlineSpan=' + Math.round(flatlineSpan) +
            'px, virtualX=' + Math.round(_hb.virtualX) +
            ', segStart=' + Math.round(liveSeg.x_start) +
            ', effectiveBlockTs=' + effectiveBlockTs +
            ', elapsed=' + Math.round(elapsedSinceBlock) + 's');

        // Cap density: don't cram thousands of bricks into a short flatline
        var maxForSpan = Math.max(Math.floor(flatlineSpan / 5 * 3), 200);
        if (maxForSpan < maxBricks) maxBricks = maxForSpan;

        // Stagger history bricks so they animate in from left to right over ~1.5s.
        // Each brick's timestamp is set to now + a delay based on its x position.
        var dropNow = Date.now() / 1000;
        var DROP_DURATION = 1.5; // total sweep time in seconds

        for (var i = 0; i < txs.length && placed < maxBricks; i++) {
            var tx = txs[i];
            if (!tx.fee || !tx.vsize) continue;

            // Spread all history txs uniformly across the flatline
            var txVX = liveSeg.x_start + Math.random() * flatlineSpan * 0.95;

            var feeRate = tx.fee / tx.vsize;
            var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);

            var brickH = 3 + feeNorm * 14 + Math.random() * 3;
            var gridX = Math.round(txVX / 5) * 5;
            var stackY = stackMap[gridX] || 0;
            if (stackY > 150) continue;
            stackMap[gridX] = stackY + brickH;

            // Stagger: left-most bricks appear first, sweeping right
            var xFrac = flatlineSpan > 0 ? (txVX - liveSeg.x_start) / flatlineSpan : 0;
            var dropDelay = xFrac * DROP_DURATION;

            liveSeg.blips.push({
                x: txVX, gridX: gridX,
                height: brickH + stackY, brickH: brickH, brickW: 4, stackY: stackY,
                color: feeRateColor(feeRate, medianFee), opacity: 0.75 + feeNorm * 0.2,
                txCount: 1, txid: tx.txid || null,
                feeRate: Math.round(feeRate * 10) / 10,
                vsize: tx.vsize || 0, value: tx.value || 0,
                timestamp: dropNow + dropDelay, isDrop: true, fadeStart: 0,
                bobPhase: Math.random() * Math.PI * 2,
                bobSpeed: 1.2 + feeNorm * 0.8,
                lane: Math.floor(Math.random() * 5) - 2,
                feeRatio: medianFee > 0 ? feeRate / medianFee : 1
            });
            placed++;
        }
        // Seed the segment's column height map so live blips stack correctly
        liveSeg._colHeights = stackMap;
        console.log('[heartbeat] placed', placed, 'history bricks on timeline');
    }

    // Process a live block from SSE (shared by live handler + queue replay)
    function processLiveBlock(block) {
        if (!_hb) return;
        _hb._blockProcessing = true;

        // Collapse the dead processing gap: the backend suppresses txs while
        // building block data (~2-5s), so virtualX has advanced past the last
        // brick. Find the rightmost blip and snap virtualX to just past it.
        // This is exact — no time-based estimation that can overshoot.
        var liveSeg = _hb.timeline[_hb.timeline.length - 1];
        if (liveSeg && liveSeg.type === 'flatline' && liveSeg.x_end === null && liveSeg.blips.length > 0) {
            var rightmost = liveSeg.x_start;
            for (var ri = 0; ri < liveSeg.blips.length; ri++) {
                if (liveSeg.blips[ri].fadeStart === 0 && liveSeg.blips[ri].x > rightmost) {
                    rightmost = liveSeg.blips[ri].x;
                }
            }
            var closeAt = rightmost + 5; // one grid unit past last brick
            if (closeAt < _hb.virtualX) {
                console.log('[heartbeat] gap collapse: ' + Math.round(_hb.virtualX) + ' -> ' + Math.round(closeAt) +
                    ' (' + Math.round(_hb.virtualX - closeAt) + 'px removed, block=' + block.height + ')');
                _hb.virtualX = closeAt;
            }
        } else {
            console.log('[heartbeat] processLiveBlock: block=' + block.height +
                ', no blips on flatline, virtualX=' + Math.round(_hb.virtualX));
        }

        // Build block data — SSE now includes real block stats from RPC + mempool fees
        var blockTs = block.timestamp || Math.floor(Date.now() / 1000);
        var inter = 600;
        if (_hb.lastBlockTime > 0 && blockTs > _hb.lastBlockTime) {
            inter = blockTs - _hb.lastBlockTime;
        }
        var blockJson = JSON.stringify([{
            height: block.height,
            timestamp: blockTs,
            tx_count: block.tx_count || block.confirmed_count || 3000,
            total_fees: block.total_fees || 0,
            size: block.size || 0,
            weight: block.weight || 3000000,
            inter_block_seconds: inter
        }]);
        window.pushHeartbeatBlocks(blockJson, false);
        if (window.heartbeatFlash) window.heartbeatFlash();
        if (window.heartbeatPulse) window.heartbeatPulse();

        // Resume tx flow — new txs will now flush to the new post-block flatline
        _hb._blockProcessing = false;
    }

    // ── Own node SSE feed (primary) ─────────────────────────────
    function connectOwnFeed() {
        if (!_hb) return;
        console.log('[heartbeat] connecting to SSE /api/stats/heartbeat');
        _hb._txThrottleTime = 0;
        _hb._txBatchQueue = [];
        _hb._sseTxCount = 0;
        try {
            var es = new EventSource('/api/stats/heartbeat');
            es.addEventListener('history', function(e) {
                if (!_hb) return;
                try {
                    var data = JSON.parse(e.data);
                    var txs = data.txs || data;
                    var lastBlockTs = data.last_block_ts || 0;
                    if (!Array.isArray(txs)) return;
                    console.log('[heartbeat] SSE history:', txs.length, 'txs, last_block_ts:', lastBlockTs);
                    if (txs.length === 0) return;
                    _hb._hasTxStream = true;

                    // Defer history placement until after block replay has fast-forwarded
                    // the timeline. Without this, flatlineSpan ≈ 0 and all bricks cluster
                    // at the start, creating a huge gap after the replay blocks.
                    if (!_hb.lastBlockTime && _hb.virtualX < 100) {
                        console.log('[heartbeat] deferring history (replay not ready)');
                        _hb._pendingHistory = { txs: txs, last_block_ts: lastBlockTs };
                        return;
                    }

                    placeHistoryTxs(txs, lastBlockTs);
                } catch (err) { console.log('[heartbeat] SSE history error:', err); }
            });
            es.addEventListener('tx', function(e) {
                if (!_hb) return;
                try {
                    var tx = JSON.parse(e.data);
                    _hb._hasTxStream = true;
                    _hb._sseTxCount++;
                    // When hidden: buffer txs (capped) but don't flush.
                    // They'll be spread across the gap on tab return.
                    if (document.hidden) {
                        if (!_hb._hiddenTxBuffer) _hb._hiddenTxBuffer = [];
                        if (_hb._hiddenTxBuffer.length < 3000) _hb._hiddenTxBuffer.push(tx);
                        return;
                    }
                    if (_hb._txBatchQueue.length > 500) _hb._txBatchQueue.length = 500;
                    _hb._txBatchQueue.push(tx);
                    // Track rolling median fee rate from own node data
                    if (tx.fee_rate) {
                        if (!_hb._recentFeeRates) _hb._recentFeeRates = [];
                        _hb._recentFeeRates.push(tx.fee_rate);
                        if (_hb._recentFeeRates.length > 200) _hb._recentFeeRates.shift();
                        if (_hb._recentFeeRates.length >= 20) {
                            var sorted = _hb._recentFeeRates.slice().sort(function(a, b) { return a - b; });
                            _hb._wsMedianFee = Math.max(1, sorted[Math.floor(sorted.length / 2)]);
                        }
                    }
                    var now = Date.now();
                    if (now - _hb._txThrottleTime > 200) {
                        _hb._txThrottleTime = now;
                        flushTxBatch();
                    }
                    if (_hb._sseTxCount % 100 === 0) {
                        console.log('[heartbeat] SSE live txs:', _hb._sseTxCount);
                    }
                } catch (err) {}
            });
            es.addEventListener('block', function(e) {
                if (!_hb) return;
                try {
                    var block = JSON.parse(e.data);
                    if (!block.height) return;
                    console.log('SSE block:', block.height, block.hash,
                        'unconfirmed:', (block.unconfirmed_txids || []).length);

                    // When tab is hidden, queue blocks for replay on return
                    // instead of processing live (which stacks them at same virtualX)
                    if (document.hidden) {
                        if (!_hb._blockQueue) _hb._blockQueue = [];
                        _hb._blockQueue.push(block);
                        // Track last block time so inter-block calc stays correct
                        _hb._queuedBlockTime = Math.floor(Date.now() / 1000);
                        console.log('[heartbeat] block queued (tab hidden):', block.height);
                        return;
                    }

                    processLiveBlock(block);
                } catch (err) {}
            });
            es.addEventListener('lag', function(e) {
                console.log('SSE lag:', e.data);
            });
            es.onopen = function() {
                console.log('[heartbeat] SSE connected');
                _hb.wsConnected = true;
                _hb._sseRetries = 0; // reset retry count on successful connect
            };
            es.onerror = function(err) {
                if (!_hb) return;
                _hb.wsConnected = false;
                es.close();
                _hb._sseRetries = (_hb._sseRetries || 0) + 1;
                if (_hb._sseRetries <= 3) {
                    // Retry SSE with backoff: 3s, 6s, 12s
                    var delay = 3000 * Math.pow(2, _hb._sseRetries - 1);
                    console.log('[heartbeat] SSE error, retry', _hb._sseRetries, 'in', delay / 1000 + 's');
                    setTimeout(function() {
                        if (_hb) connectOwnFeed();
                    }, delay);
                } else {
                    console.log('[heartbeat] SSE failed after 3 retries, falling back to mempool.space WS');
                    connectMempoolFallback();
                }
            };
            _hb._sse = es;
        } catch (err) {
            connectMempoolFallback();
        }
    }

    // ── Mempool.space WebSocket fallback ──────────────────────
    function connectMempoolFallback() {
        if (!_hb) return;
        _hb._lastMempoolTxCount = 0;

        // Periodically try to reconnect to SSE while on WS fallback
        if (_hb._sseReconnectTimer) clearInterval(_hb._sseReconnectTimer);
        _hb._sseReconnectTimer = setInterval(function() {
            if (!_hb) return;
            // Try SSE silently — if it connects, close WS and cancel this timer
            var testEs = new EventSource('/api/stats/heartbeat');
            testEs.onopen = function() {
                console.log('[heartbeat] SSE recovered, switching back from WS');
                testEs.close();
                if (_hb.ws) { try { _hb.ws.close(); } catch(e) {} }
                if (_hb._sseReconnectTimer) { clearInterval(_hb._sseReconnectTimer); _hb._sseReconnectTimer = null; }
                _hb._sseRetries = 0;
                connectOwnFeed();
            };
            testEs.onerror = function() { testEs.close(); };
        }, 60000); // try every 60s

        try {
            var ws = new WebSocket('wss://mempool.space/api/v1/ws');
            ws.onopen = function() {
                ws.send(JSON.stringify({ action: 'want', data: ['mempool-blocks', 'stats'] }));
                _hb.wsConnected = true;
                _hb._wsRetryDelay = 5000;
                _hb._txThrottleTime = 0;
                _hb._txBatchQueue = [];
            };
            ws.onmessage = function(e) {
                if (!_hb) return;
                try {
                    var data = JSON.parse(e.data);
                    var txArr = data.transactions || data.txs || null;
                    if (!txArr && data.tx && data.tx.txid) {
                        txArr = [data.tx];
                    }
                    if (txArr && Array.isArray(txArr) && txArr.length > 0) {
                        _hb._hasTxStream = true;
                        for (var ti = 0; ti < txArr.length; ti++) {
                            _hb._txBatchQueue.push(txArr[ti]);
                        }
                        var now = Date.now();
                        if (now - _hb._txThrottleTime > 200) {
                            _hb._txThrottleTime = now;
                            flushTxBatch();
                        }
                    }
                    if (data['mempool-blocks']) {
                        var mblocks = data['mempool-blocks'];
                        var totalTx = 0;
                        for (var i = 0; i < mblocks.length; i++) {
                            totalTx += mblocks[i].nTx || 0;
                        }
                        var newTx = 0;
                        if (_hb._lastMempoolTxCount > 0) {
                            newTx = Math.max(0, totalTx - _hb._lastMempoolTxCount);
                        }
                        _hb._lastMempoolTxCount = totalTx;
                        _hb._wsMedianFee = Math.max(1, mblocks[0] ? mblocks[0].medianFee || 5 : 5);
                        if (!_hb._hasTxStream && newTx > 0) {
                            var topFee = Math.max(2, mblocks[0] ? (mblocks[0].feeRange || [1, 5, 10])[2] || 10 : 5);
                            addMempoolTxs(newTx, _hb._wsMedianFee, topFee);
                        }
                    }
                } catch (err) {}
            };
            ws.onclose = function() {
                if (!_hb) return;
                _hb.wsConnected = false;
                _hb._wsRetryDelay = Math.min((_hb._wsRetryDelay || 5000) * 2, 300000);
                setTimeout(function() {
                    if (_hb) connectMempoolFallback();
                }, _hb._wsRetryDelay);
            };
            ws.onerror = function() { ws.close(); };
            _hb.ws = ws;
        } catch (err) {}
    }

    // Add tx blips at the live head position. Each blip represents ~N new txs.
    // Placed at the current head so they naturally spread out as time advances.
    // Flush queued individual transactions into visual bricks
    function flushTxBatch() {
        if (!_hb || !_hb._txBatchQueue || _hb._txBatchQueue.length === 0) return;
        // During block processing, keep txs queued — they'll flush to the new flatline
        if (_hb._blockProcessing) return;
        var liveSeg = _hb.timeline[_hb.timeline.length - 1];
        if (!liveSeg || liveSeg.type !== 'flatline') return;

        var batch = _hb._txBatchQueue;
        _hb._txBatchQueue = [];

        // Cap visual bricks at 10 per flush (batch the rest into 1 aggregate)
        var maxBricks = 10;
        var medianFee = _hb._wsMedianFee || 5;

        for (var i = 0; i < Math.min(batch.length, maxBricks); i++) {
            var tx = batch[i];
            var feeRate = tx.fee && tx.vsize ? tx.fee / tx.vsize : 1;
            var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);

            var brickW = 4;
            var brickH = 3 + feeNorm * 14 + Math.random() * 3;
            var brickX = _hb.virtualX - Math.random() * 15;
            var gridX = Math.round(brickX / 5) * 5;

            // Stack height via column map (O(1) instead of scanning all blips)
            if (!liveSeg._colHeights) liveSeg._colHeights = {};
            var stackY = liveSeg._colHeights[gridX] || 0;

            liveSeg.blips.push({
                x: brickX,
                gridX: gridX,
                height: brickH + stackY,
                brickH: brickH,
                brickW: brickW,
                stackY: stackY,
                color: feeRateColor(feeRate, medianFee),
                opacity: 0.75 + feeNorm * 0.2,
                txCount: 1,
                txid: tx.txid || null,
                feeRate: Math.round(feeRate * 10) / 10,
                vsize: tx.vsize || 0,
                value: tx.value || 0,
                timestamp: Date.now() / 1000,
                fadeStart: 0,
                bobPhase: Math.random() * Math.PI * 2,
                bobSpeed: 1.2 + feeNorm * 0.8,
                lane: Math.floor(Math.random() * 5) - 2,
                feeRatio: medianFee > 0 ? feeRate / medianFee : 1
            });
            liveSeg._colHeights[gridX] = stackY + brickH;
        }

        // If there are excess txs beyond maxBricks, create one aggregate brick
        if (batch.length > maxBricks) {
            var remaining = batch.length - maxBricks;
            var avgFee = 0;
            for (var j = maxBricks; j < batch.length; j++) {
                avgFee += (batch[j].fee && batch[j].vsize ? batch[j].fee / batch[j].vsize : 1);
            }
            avgFee /= remaining;
            var aggFeeNorm = Math.min(Math.log2(avgFee + 1) / 6, 1.0);
            var aggColor = avgFee < medianFee ? 'rgba(0, 230, 118, ' : 'rgba(255, 152, 0, ';
            var aggX = _hb.virtualX - Math.random() * 10;
            var aggGridX = Math.round(aggX / 5) * 5;
            if (!liveSeg._colHeights) liveSeg._colHeights = {};
            var aggStackY = liveSeg._colHeights[aggGridX] || 0;
            var aggH = 3 + aggFeeNorm * 10;
            liveSeg.blips.push({
                x: aggX, gridX: aggGridX,
                height: aggH + aggStackY, brickH: aggH, brickW: 4, stackY: aggStackY,
                color: aggColor, opacity: 0.5,
                txCount: remaining, feeRate: Math.round(avgFee * 10) / 10,
                timestamp: Date.now() / 1000, fadeStart: 0,
                bobPhase: Math.random() * Math.PI * 2,
                bobSpeed: 1.2 + aggFeeNorm * 0.8,
                lane: Math.floor(Math.random() * 5) - 2,
                feeRatio: medianFee > 0 ? avgFee / medianFee : 1
            });
            liveSeg._colHeights[aggGridX] = aggStackY + aggH;
        }

        // Priority culling: when too many blips, evict lowest-value ones.
        // Threshold scales with flatline span — longer flatlines can hold more bricks.
        // Viewport culling already skips off-screen blips, so the segment can hold many.
        var flatSpan = _hb.virtualX - liveSeg.x_start;
        var CULL_THRESHOLD = Math.max(5000, Math.floor(flatSpan / 5 * 3));
        var CULL_TARGET = Math.floor(CULL_THRESHOLD * 0.8);
        if (liveSeg.blips.length > CULL_THRESHOLD) {
            // Score each active blip: lower score = evict first
            // Priority: low fee + small vsize evicted first
            var scored = [];
            for (var ci = 0; ci < liveSeg.blips.length; ci++) {
                var cb = liveSeg.blips[ci];
                if (cb.fadeStart > 0) continue; // already fading, skip
                var score = (cb.feeRate || 0.1) * Math.log2((cb.vsize || 100) + 1);
                scored.push({ idx: ci, score: score });
            }
            if (scored.length > CULL_TARGET) {
                scored.sort(function(a, b) { return a.score - b.score; });
                var toEvict = scored.length - CULL_TARGET;
                console.log('[heartbeat] culling:', liveSeg.blips.length, '->', CULL_TARGET,
                    '(evicting', toEvict, ', threshold=' + CULL_THRESHOLD + ', span=' + Math.round(flatSpan) + 'px)');
                // Mark lowest-scored blips for immediate removal
                var evictSet = {};
                for (var ei = 0; ei < toEvict; ei++) {
                    evictSet[scored[ei].idx] = true;
                }
                var kept = [];
                liveSeg._colHeights = {};
                for (var ki = 0; ki < liveSeg.blips.length; ki++) {
                    if (!evictSet[ki]) {
                        kept.push(liveSeg.blips[ki]);
                    }
                }
                // Restack: recalculate stackY/height for all kept blips
                for (var ri = 0; ri < kept.length; ri++) {
                    var rb = kept[ri];
                    if (rb.fadeStart > 0 || rb.gridX === undefined) continue;
                    var newStackY = liveSeg._colHeights[rb.gridX] || 0;
                    rb.stackY = newStackY;
                    rb.height = (rb.brickH || 0) + newStackY;
                    liveSeg._colHeights[rb.gridX] = newStackY + (rb.brickH || 0);
                }
                liveSeg.blips = kept;
            }
        }
    }

    // Fallback: add aggregate blips when individual tx stream isn't available
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

            var brickW = 4;
            var brickH = 3 + feeNorm * 14 + Math.random() * 3; // 3-20px tall

            // Place at the live head, snap to tight grid for clean columns
            var brickX = _hb.virtualX - Math.random() * 15;
            var gridX = Math.round(brickX / 5) * 5; // snap to 5px grid (matches brick width)

            // Stack: O(1) column height lookup
            if (!liveSeg._colHeights) liveSeg._colHeights = {};
            var stackY = Math.min(liveSeg._colHeights[gridX] || 0, 150);

            liveSeg.blips.push({
                x: brickX,
                gridX: gridX,
                height: brickH + stackY,
                brickH: brickH,
                brickW: brickW,
                stackY: stackY,
                color: feeRateColor(feeRate, medianFeeRate),
                opacity: 0.6 + feeNorm * 0.3,
                txCount: txPerBlip,
                feeRate: Math.round(feeRate * 10) / 10,
                timestamp: Date.now() / 1000,
                fadeStart: 0,
                bobPhase: Math.random() * Math.PI * 2,
                bobSpeed: 1.2 + feeNorm * 0.8,
                lane: Math.floor(Math.random() * 5) - 2,
                feeRatio: (medianFeeRate || 5) > 0 ? feeRate / (medianFeeRate || 5) : 1
            });
            liveSeg._colHeights[gridX] = (liveSeg._colHeights[gridX] || 0) + brickH;
        }
    }

    // ── Tooltip drawing ────────────────────────────────────────

    // Generic tooltip box: draws a rounded rect with lines of text.
    // `anchorY` is the y position to place the box above; `borderColor` sets the border.
    function drawTooltipBox(ctx, lines, canvasX, anchorY, borderColor, opts) {
        var padding = (opts && opts.padding) || 8;
        var lineH = (opts && opts.lineH) || 16;
        var fontSize = (opts && opts.fontSize) || '12px monospace';

        ctx.font = fontSize;
        var maxW = 0;
        for (var i = 0; i < lines.length; i++) {
            var m = ctx.measureText(lines[i]).width;
            if (m > maxW) maxW = m;
        }

        var boxW = maxW + padding * 2;
        var boxH = lines.length * lineH + padding * 2;
        var boxX = canvasX - boxW / 2;
        var boxY = anchorY - boxH;

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
        ctx.strokeStyle = borderColor;
        ctx.lineWidth = 1;
        ctx.beginPath();
        roundRect(ctx, boxX, boxY, boxW, boxH, 4);
        ctx.stroke();

        // Text
        ctx.fillStyle = (opts && opts.textColor) || 'rgba(255, 255, 255, 0.85)';
        ctx.font = fontSize;
        for (var i = 0; i < lines.length; i++) {
            ctx.fillText(lines[i], boxX + padding, boxY + padding + (i + 1) * lineH - 3);
        }
    }

    function drawTooltip(ctx, seg, canvasX, canvasY, baseline) {
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
        drawTooltipBox(ctx, lines, canvasX, canvasY - 12, seg.color);
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

    // Blip tooltip: shows fee rate and tx info for a single blip/brick
    function drawBlipTooltip(ctx, blip, canvasX, baseline) {
        var lines = [];
        if (blip.txid) {
            lines.push('TX: ' + blip.txid.substring(0, 16) + '...');
        } else {
            lines.push('~' + (blip.txCount || 1) + ' tx' + ((blip.txCount || 1) > 1 ? 's' : ''));
        }
        lines.push('Fee rate: ' + (blip.feeRate ? Math.round(blip.feeRate * 10) / 10 : '?') + ' sat/vB');
        if (blip.feeRate && blip.vsize) {
            var feeSats = Math.round(blip.feeRate * blip.vsize);
            lines.push('Fee paid: ' + feeSats.toLocaleString() + ' sats');
        }
        if (blip.vsize) {
            lines.push('Size: ' + Math.round(blip.vsize) + ' vB');
        }
        if (blip.value) {
            var btcVal = blip.value / 100000000;
            lines.push('Value: ' + (btcVal < 0.001 ? btcVal.toFixed(8) : btcVal.toFixed(4)) + ' BTC');
        }
        if (blip.timestamp) {
            var d = new Date(blip.timestamp * 1000);
            lines.push(d.toLocaleTimeString());
        }
        // Show link hint for pinned blips with txid
        if (blip.txid && _hb._pinnedBlip === blip) {
            lines.push('\u2192 Click again to view on mempool.space');
        }
        var blipColor = blip.color ? blip.color + '0.6)' : 'rgba(0,230,118,0.6)';
        // Clamp tooltip y to stay within canvas (at least 10px from top)
        var tipY;
        if (_hb.renderMode === 'bloodstream') {
            // Use actual cell position in tube
            var tubeY = Math.sin((blip.bobPhase || 0) * 3.71) * 18;
            var cellR = 3 * Math.max(_hb.zoom * 0.4, 0.8);
            tipY = baseline - tubeY - cellR - 12;
        } else {
            var heightScale = _hb.zoom > 4 ? 1 + (_hb.zoom - 4) * 0.15 : 1;
            tipY = baseline - ((blip.stackY || 0) + (blip.brickH || blip.height)) * heightScale - 12;
        }
        tipY = Math.max(10, Math.min(tipY, _hb.height - 80));
        drawTooltipBox(ctx, lines, canvasX, tipY, blipColor, {
            padding: 6, lineH: 15, fontSize: '11px monospace', textColor: 'rgba(255, 255, 255, 0.9)'
        });
    }

    // Flatline tooltip: shows interval between surrounding blocks
    function drawFlatlineTooltip(ctx, info, canvasX, baseline) {
        var lines = [];
        if (info.prevBlock && info.nextBlock) {
            var interval = info.nextBlock.timestamp - info.prevBlock.timestamp;
            lines.push('Block #' + info.prevBlock.height + ' \u2192 #' + info.nextBlock.height);
            lines.push('Interval: ' + formatDuration(interval));
        } else if (info.prevBlock && !info.nextBlock) {
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
        drawTooltipBox(ctx, lines, canvasX, baseline - 20, 'rgba(255, 255, 255, 0.15)', {
            textColor: 'rgba(255, 255, 255, 0.7)'
        });
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

    // Jump to Live is now part of the control bar (drawControlBar)

    // ── Control bar (bottom center) ──────────────────────────────
    var CTRL_BTNS = [
        { id: 'mode', label: '\u2B24', tip: 'Toggle cell/brick view' },
        { id: 'prev',    label: '\u23EE', tip: 'Previous block' },
        { id: 'pause',   label: '\u23F8', tip: 'Pause' },
        { id: 'zoomOut', label: '\u2212', tip: 'Zoom out' },
        { id: 'zoomIn',  label: '+',      tip: 'Zoom in' },
        { id: 'center',  label: '\u2316', tip: 'Center on live head' },
        { id: 'live',    label: '\u25C9', tip: 'Jump to Live' }
    ];
    var CTRL_BTN_SIZE = 32;
    var CTRL_BTN_GAP = 6;

    function drawControlBar(ctx, w, h) {
        // Filter buttons: hide 'live' when already auto-following
        var btns = [];
        for (var fi = 0; fi < CTRL_BTNS.length; fi++) {
            var def = CTRL_BTNS[fi];
            if (def.id === 'live' && _hb.autoFollow) continue;
            btns.push(def);
        }
        var totalW = btns.length * CTRL_BTN_SIZE + (btns.length - 1) * CTRL_BTN_GAP;
        var startX = (w - totalW) / 2;
        var btnY = h - CTRL_BTN_SIZE - 2;
        _hb._ctrlBtns = [];

        for (var i = 0; i < btns.length; i++) {
            var def = btns[i];
            var bx = startX + i * (CTRL_BTN_SIZE + CTRL_BTN_GAP);

            // Highlight based on state
            var isActive = (def.id === 'pause' && _hb.paused) ||
                           (def.id === 'live' && !_hb.autoFollow);
            var bgAlpha = isActive ? 0.25 : 0.12;
            var borderColor = isActive ? (def.id === 'live' ? COLORS.healthy : COLORS.elevated)
                                       : 'rgba(255,255,255,0.3)';
            var textColor = isActive ? (def.id === 'live' ? COLORS.healthy : COLORS.elevated)
                                     : 'rgba(255,255,255,0.7)';

            // Dynamic label for pause/play
            var label = def.label;
            if (def.id === 'pause') {
                label = _hb.paused ? '\u25B6' : '\u23F8';
            }
            if (def.id === 'mode') {
                label = _hb.renderMode === 'bricks' ? '\u25A0'      // square
                      : '\u2B24';                                     // circle (bloodstream)
            }

            ctx.fillStyle = 'rgba(255,255,255,' + bgAlpha + ')';
            ctx.beginPath();
            roundRect(ctx, bx, btnY, CTRL_BTN_SIZE, CTRL_BTN_SIZE, 4);
            ctx.fill();

            ctx.strokeStyle = borderColor;
            ctx.lineWidth = 1;
            ctx.beginPath();
            roundRect(ctx, bx, btnY, CTRL_BTN_SIZE, CTRL_BTN_SIZE, 4);
            ctx.stroke();

            ctx.fillStyle = textColor;
            ctx.font = 'bold 16px sans-serif';
            ctx.textAlign = 'center';
            ctx.fillText(label, bx + CTRL_BTN_SIZE / 2, btnY + CTRL_BTN_SIZE / 2 + 6);

            _hb._ctrlBtns.push({
                id: def.id, tip: def.tip,
                x: bx, y: btnY, w: CTRL_BTN_SIZE, h: CTRL_BTN_SIZE
            });
        }
        ctx.textAlign = 'left';

        // Draw tooltip if hovering a control button
        if (_hb._ctrlHover) {
            ctx.font = '11px monospace';
            ctx.fillStyle = 'rgba(255,255,255,0.6)';
            ctx.textAlign = 'center';
            ctx.fillText(_hb._ctrlHover.tip, _hb._ctrlHover.x + CTRL_BTN_SIZE / 2, btnY - 6);
            ctx.textAlign = 'left';
        }
    }

    function handleControlClick(id) {
        if (!_hb) return;
        switch (id) {
            case 'pause':
                _hb.paused = !_hb.paused;
                if (_hb.paused) {
                    _hb.autoFollow = false;
                } else {
                    // Resume: start scrolling from current position (don't jump to head)
                    _hb._playResumeOffset = _hb.viewOffset;
                }
                break;
            case 'prev':
                // Scroll to the previous block segment
                var centerVX = canvasToVirtual(_hb.width / 2);
                for (var i = _hb.timeline.length - 1; i >= 0; i--) {
                    var seg = _hb.timeline[i];
                    if (seg.type === 'block' && seg.x_end < centerVX - 10) {
                        _hb.autoFollow = false;
                        _hb.paused = true;
                        var mid = (seg.x_start + seg.x_end) / 2;
                        _hb.viewOffset = mid - _hb.width / 2 / _hb.zoom;
                        break;
                    }
                }
                break;
            case 'zoomIn':
                var cx = _hb.width / 2;
                var vx = canvasToVirtual(cx);
                _hb.zoom = Math.min(_hb.maxZoom, _hb.zoom * 1.4);
                _hb.viewOffset = vx - cx / _hb.zoom;
                _hb.autoFollow = false;
                break;
            case 'zoomOut':
                var cx2 = _hb.width / 2;
                var vx2 = canvasToVirtual(cx2);
                _hb.zoom = Math.max(_hb.minZoom, _hb.zoom / 1.4);
                _hb.viewOffset = vx2 - cx2 / _hb.zoom;
                break;
            case 'center':
                // Center viewport on the live head without changing zoom
                _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
                break;
            case 'mode':
                _hb.renderMode = _hb.renderMode === 'bricks' ? 'bloodstream' : 'bricks';
                break;
            case 'live':
                // Jump to live: snap viewport to the live head immediately
                _hb.autoFollow = true;
                _hb.paused = false;
                _hb.zoom = 1.9;
                _hb.viewOffsetY = 0;
                _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / 1.9;
                _hb.hoveredBlock = null;
                _hb.hoveredBlip = null;
                _hb._pinnedBlip = null;
                break;
        }
    }

    // ── Main draw loop ─────────────────────────────────────────
    function drawFrame(frameTime) {
        if (!_hb) return;

        var ctx = _hb.ctx;
        var w = _hb.width;
        var h = _hb.height;
        var baseline = h * 0.55 + (_hb.viewOffsetY || 0);

        // Compute time delta
        var now = Date.now() / 1000;
        var dt = _hb.lastFrameTime > 0 ? (now - _hb.lastFrameTime) : 0;
        if (dt > 0.5) dt = 0.5; // clamp to avoid huge jumps on tab switch
        _hb.lastFrameTime = now;

        // ── Advance live flatline ──────────────────────────────
        var liveSeg = _hb.timeline.length > 0 ? _hb.timeline[_hb.timeline.length - 1] : null;
        if (liveSeg && liveSeg.type === 'flatline' && liveSeg.x_end === null) {
            _hb.virtualX += dt * FLATLINE_PX_PER_SEC;
        }

        // If auto-following, keep viewport pinned to the head (zoom-aware)
        if (_hb.autoFollow) {
            _hb.viewOffset = _hb.virtualX - (w * HEAD_POSITION_FRAC) / _hb.zoom;
        } else if (!_hb.paused && !_hb.isDragging && !_hb._pinching) {
            // Play mode: scroll at the same rate as time (don't jump to head)
            _hb.viewOffset += dt * FLATLINE_PX_PER_SEC;
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
            var segStart = seg.x_start;
            var segEnd = seg.type === 'block' ? seg.x_end
                       : (seg.x_end !== null ? seg.x_end : _hb.virtualX);

            if (segEnd < viewLeft) continue;   // before viewport
            if (segStart > viewRight) break;   // past viewport — all further segments are too

            if (seg.type === 'block') {
                drawBlockSegment(ctx, seg, viewLeft, baseline, liveColor);
            } else {
                var flatColor = (seg.x_end === null) ? liveColor : (seg.color || COLORS.healthy);
                drawFlatlineSegment(ctx, seg, segEnd, viewLeft, viewRight, baseline, flatColor, jitter, seg.x_end === null, now);
            }
        }

        // ── Draw tooltip if hovering ───────────────────────────
        ctx.globalAlpha = 1;
        if (_hb.hoveredBlock) {
            var hSeg = _hb.hoveredBlock;
            var midX = virtualToCanvas((hSeg.x_start + hSeg.x_end) / 2);
            drawTooltip(ctx, hSeg, midX, baseline - 30, baseline);
        } else if (_hb._pinnedBlip) {
            var pbx = virtualToCanvas(_hb._pinnedBlip.x);
            drawBlipTooltip(ctx, _hb._pinnedBlip, pbx, baseline);
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

        // Jump to Live is now in the control bar

        // ── Draw control bar ─────────────────────────────────
        drawControlBar(ctx, w, h);

        // ── Draw block arrival announcement ───────────────────
        if (_hb._announcement && now < _hb._announcement.end) {
            var ann = _hb._announcement;
            var annElapsed = now - ann.start;
            var annDuration = ann.end - ann.start;
            // Fade in 0-0.5s, hold, fade out last 1.5s
            var annAlpha;
            if (annElapsed < 0.5) {
                annAlpha = annElapsed / 0.5;
            } else if (annElapsed < annDuration - 1.5) {
                annAlpha = 1.0;
            } else {
                annAlpha = (annDuration - annElapsed) / 1.5;
            }
            annAlpha = Math.max(0, Math.min(1, annAlpha));

            ctx.save();
            ctx.textAlign = 'center';
            ctx.globalAlpha = annAlpha;

            // Block number
            ctx.font = 'bold 22px monospace';
            ctx.fillStyle = ann.color;
            ctx.fillText(ann.title, w / 2, 46);

            // Subtle horizontal accent lines
            ctx.strokeStyle = ann.color;
            ctx.lineWidth = 1;
            ctx.globalAlpha = annAlpha * 0.3;
            ctx.beginPath();
            ctx.moveTo(w / 2 - 80, 52);
            ctx.lineTo(w / 2 + 80, 52);
            ctx.stroke();
            ctx.globalAlpha = annAlpha;

            // Fee + tx count subtitle
            ctx.font = '14px monospace';
            ctx.fillStyle = 'rgba(255, 255, 255, 0.7)';
            ctx.fillText(ann.subtitle, w / 2, 70);

            ctx.globalAlpha = 1;
            ctx.textAlign = 'left';
            ctx.restore();
        } else if (_hb._announcement && now >= _hb._announcement.end) {
            _hb._announcement = null;
        }

        // ── Draw live indicator dot ────────────────────────────
        if (liveSeg && liveSeg.x_end === null) {
            var dotX = virtualToCanvas(_hb.virtualX);
            if (dotX >= -10 && dotX <= w + 10) {
                var dotPulse = 0.5 + 0.5 * Math.sin(now * 3);
                // Outer glow ring
                ctx.beginPath();
                ctx.arc(dotX, baseline, 8, 0, Math.PI * 2);
                ctx.fillStyle = liveColor;
                ctx.globalAlpha = 0.1 + dotPulse * 0.15;
                ctx.fill();
                // Middle ring
                ctx.beginPath();
                ctx.arc(dotX, baseline, 5, 0, Math.PI * 2);
                ctx.globalAlpha = 0.2 + dotPulse * 0.2;
                ctx.fill();
                // Inner dot
                ctx.beginPath();
                ctx.arc(dotX, baseline, 2.5, 0, Math.PI * 2);
                ctx.globalAlpha = 0.6 + dotPulse * 0.4;
                ctx.fill();
                // Trailing afterglow on the flatline
                var grad = ctx.createLinearGradient(dotX - 40, 0, dotX, 0);
                grad.addColorStop(0, 'rgba(0,0,0,0)');
                grad.addColorStop(1, liveColor);
                ctx.globalAlpha = 0.15 + dotPulse * 0.1;
                ctx.fillStyle = grad;
                ctx.fillRect(dotX - 40, baseline - 1, 40, 2);
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

        var isHovered = (_hb.hoveredBlock === seg);
        ctx.beginPath();
        ctx.strokeStyle = color;
        ctx.lineWidth = isHovered ? 3 : 2;
        ctx.shadowBlur = isHovered ? 24 : 14;
        ctx.shadowColor = color;

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
    function drawFlatlineSegment(ctx, seg, segEnd, viewLeft, viewRight, baseline, color, jitter, isLive, nowSec) {
        // Clamp to visible range
        var drawStart = Math.max(seg.x_start, viewLeft);
        var drawEnd = Math.min(segEnd, viewRight);

        if (drawStart >= drawEnd) return;

        var cx1 = virtualToCanvas(drawStart);
        var cx2 = virtualToCanvas(drawEnd);
        var y = baseline + (isLive ? jitter : 0);

        // Draw the flatline
        ctx.beginPath();
        ctx.moveTo(cx1, y);
        ctx.lineTo(cx2, y);
        ctx.strokeStyle = color;
        ctx.lineWidth = 2;
        ctx.shadowBlur = 8;
        ctx.shadowColor = color;
        ctx.stroke();
        ctx.shadowBlur = 0;

        // Subtle floor glow below baseline
        ctx.globalAlpha = 0.06;
        ctx.fillStyle = color;
        ctx.fillRect(cx1, baseline + 1, cx2 - cx1, 8);
        ctx.globalAlpha = 0.03;
        ctx.fillRect(cx1, baseline + 9, cx2 - cx1, 6);
        ctx.globalAlpha = 1;

        // Vessel walls + atmosphere in bloodstream mode
        if (_hb.renderMode === 'bloodstream') {
            // Count visible cells for density-based effects
            var cellCount = 0;
            if (seg.blips) {
                for (var vci = 0; vci < seg.blips.length; vci++) {
                    var vcb = seg.blips[vci];
                    if (vcb.fadeStart === 0 && vcb.x >= viewLeft && vcb.x <= viewRight) cellCount++;
                }
            }

            // Vessel tube — centered on baseline, symmetric above/below
            var tubeH = 30; // ±30px from baseline (60px total)
            var underglowAlpha = 0.02 + Math.min(cellCount / 500, 0.04);
            if (isLive) {
                underglowAlpha *= 0.85 + 0.15 * Math.sin(nowSec * 1.2);
            }
            var grad = ctx.createLinearGradient(0, baseline - tubeH, 0, baseline + tubeH);
            grad.addColorStop(0, 'rgba(180, 30, 50, 0)');
            grad.addColorStop(0.15, 'rgba(180, 30, 50, ' + underglowAlpha + ')');
            grad.addColorStop(0.5, 'rgba(180, 30, 50, ' + (underglowAlpha * 1.2) + ')');
            grad.addColorStop(0.85, 'rgba(180, 30, 50, ' + underglowAlpha + ')');
            grad.addColorStop(1, 'rgba(180, 30, 50, 0)');
            ctx.fillStyle = grad;
            ctx.fillRect(cx1, baseline - tubeH, cx2 - cx1, tubeH * 2);

            // Vessel walls — symmetric tube boundary
            var wallAlpha = 0.04 + Math.min(cellCount / 600, 0.04);
            ctx.globalAlpha = wallAlpha;
            ctx.strokeStyle = 'rgba(255, 180, 180, 0.25)';
            ctx.lineWidth = 0.5;
            ctx.beginPath();
            ctx.moveTo(cx1, baseline - tubeH);
            ctx.lineTo(cx2, baseline - tubeH);
            ctx.stroke();
            ctx.beginPath();
            ctx.moveTo(cx1, baseline + tubeH);
            ctx.lineTo(cx2, baseline + tubeH);
            ctx.stroke();
            ctx.globalAlpha = 1;
        }

        // Draw blips
        if (seg.blips && seg.blips.length > 0) {
            var zoom = _hb.zoom;
            for (var bi = 0; bi < seg.blips.length; bi++) {
                var blip = seg.blips[bi];
                // Skip blips outside visible range
                if (blip.x < viewLeft || blip.x > viewRight) continue;

                var blipX = blip.x;
                var blipH = blip.height;
                var bOpacity = blip.opacity;
                var isAbsorbing = false;
                var absorbT = 0;


                // Bloodstream mode: cells flow toward the waveform with deformation
                if (_hb.renderMode === 'bloodstream' && blip.fadeStart > 0) {
                    var bsFadeDt = nowSec - blip.fadeStart;
                    var bsAnimDist = Math.abs(blip.absorbTargetX - blip.absorbOriginX);
                    var bsDuration = Math.max(1.5, Math.min(bsAnimDist / 40, 4.0));
                    var bsT = Math.min(bsFadeDt / bsDuration, 1.0);
                    if (bsT >= 1.0) continue;

                    // Cubic ease-in: slow start, accelerating rush
                    var bsEase = bsT * bsT * bsT;

                    // Position: slide from origin toward target
                    var bsVX = blip.absorbOriginX + (blip.absorbTargetX - blip.absorbOriginX) * bsEase;
                    var bsCX = virtualToCanvas(bsVX);

                    // Cell sizing (same tiers as normal rendering)
                    var bsVs = blip.vsize || 200;
                    var bsBaseR;
                    if (bsVs < 200) bsBaseR = 2.0;
                    else if (bsVs < 400) bsBaseR = 3.0;
                    else if (bsVs < 800) bsBaseR = 4.0;
                    else if (bsVs < 2000) bsBaseR = 5.0;
                    else bsBaseR = 6.0;
                    var bsCellR = bsBaseR * Math.max(zoom * 0.4, 0.8);

                    // Start from tube position, converge to baseline as cell approaches spike
                    // Use bobPhase + gridX hash (same as normal rendering)
                    var bsBobPhase = blip.bobPhase || 0;
                    var bsTubeH = 30;
                    var bsColHash = ((blip.gridX || 0) * 0.6180339887) % 1;
                    var bsTubePos = ((bsBobPhase + bsColHash * Math.PI * 2) % (Math.PI * 2)) / Math.PI - 1;
                    var bsStartY = baseline - bsTubePos * bsTubeH;
                    // Converge to baseline
                    var bsCellCY = bsStartY + (baseline - bsStartY) * bsEase;

                    // Deformation: stretch horizontally as cell accelerates
                    var bsStretchX = 1 + bsEase * 0.6;
                    var bsStretchY = 1 - bsEase * 0.25;

                    // Color: brighten toward white (oxygenation metaphor)
                    var bsFeeRatio = (blip.feeRate || 1) / (_hb._wsMedianFee || 5);
                    var bsBaseColor;
                    if (bsFeeRatio < 0.5) bsBaseColor = [139, 34, 82];
                    else if (bsFeeRatio < 1.0) bsBaseColor = [220, 53, 69];
                    else if (bsFeeRatio < 2.0) bsBaseColor = [255, 71, 87];
                    else if (bsFeeRatio < 4.0) bsBaseColor = [255, 107, 129];
                    else bsBaseColor = [255, 224, 230];
                    var bsBright = Math.min(bsEase * 1.8, 1.0);
                    var bsR = Math.round(lerp(bsBaseColor[0], 255, bsBright));
                    var bsG = Math.round(lerp(bsBaseColor[1], 255, bsBright));
                    var bsB = Math.round(lerp(bsBaseColor[2], 255, bsBright));

                    // Opacity: bright until last 25%
                    var bsAlpha = bsT < 0.75 ? blip.opacity : blip.opacity * (1 - (bsT - 0.75) / 0.25);
                    bsAlpha = Math.max(0, bsAlpha);

                    // Draw deformed cell (ellipse)
                    ctx.save();
                    ctx.translate(bsCX, bsCellCY);
                    ctx.scale(bsStretchX, bsStretchY);
                    ctx.beginPath();
                    ctx.arc(0, 0, bsCellR, 0, Math.PI * 2);
                    ctx.fillStyle = 'rgba(' + bsR + ',' + bsG + ',' + bsB + ',' + bsAlpha + ')';
                    if (zoom >= 1.0) {
                        ctx.shadowBlur = 4 + bsEase * 14;
                        ctx.shadowColor = 'rgba(' + bsR + ',' + bsG + ',' + bsB + ',' + (bsAlpha * 0.5) + ')';
                    }
                    ctx.fill();
                    ctx.shadowBlur = 0;
                    ctx.restore();
                    continue;
                }

                // Particle absorption: blip shatters into fragments sucked into the spike
                if (blip.fadeStart > 0 && blip.particles && blip.particles.length > 0) {
                    var fadeDt = nowSec - blip.fadeStart;
                    var dist = Math.abs(blip.absorbTargetX - blip.absorbOriginX);
                    var animDuration = Math.max(1.5, Math.min(dist / 40, 4.0));
                    // The blip's lifetime must cover the slowest particle.
                    // Each particle has speed (0.7-1.3) and delay (0-0.3s),
                    // so the slowest needs animDuration/0.7 + 0.3s to finish.
                    var maxParticleDuration = animDuration / 0.7 + 0.3;
                    absorbT = Math.min(fadeDt / maxParticleDuration, 1.0);
                    if (absorbT >= 1.0) continue;
                    isAbsorbing = true;

                    var targetCX = virtualToCanvas(blip.absorbTargetX);
                    var originCX = virtualToCanvas(blip.absorbOriginX);
                    var blipColor = blip.color || 'rgba(0, 230, 118, ';

                    // Spike height: cached per flatline per frame to avoid re-scanning
                    // the timeline for every blip (was O(blips * segments) per frame)
                    if (!seg._spikeTopFrame || seg._spikeTopFrame !== _hb.rafId) {
                        seg._spikeTopFrame = _hb.rafId;
                        seg._cachedSpikeTop = baseline - 60;
                        for (var si4 = 0; si4 < _hb.timeline.length; si4++) {
                            var tSeg = _hb.timeline[si4];
                            if (tSeg.type === 'block' && tSeg.x_start <= blip.absorbTargetX && tSeg.x_end >= blip.absorbTargetX) {
                                var peakY = 0;
                                if (tSeg.points) {
                                    for (var pp = 0; pp < tSeg.points.length; pp++) {
                                        if (tSeg.points[pp] < peakY) peakY = tSeg.points[pp];
                                    }
                                }
                                seg._cachedSpikeTop = baseline + peakY * zoom * 0.7;
                                break;
                            }
                        }
                    }
                    var spikeTop = seg._cachedSpikeTop;

                    var spikeRange = baseline - spikeTop;

                    for (var pi = 0; pi < blip.particles.length; pi++) {
                        var p = blip.particles[pi];
                        var pDuration = animDuration * (1 / p.speed);
                        var pt = Math.max(0, (fadeDt - p.delay) / pDuration);
                        pt = Math.min(pt, 1.0);
                        if (pt <= 0) continue;

                        // Ease-in with acceleration (gravity pull toward spike)
                        var pEase = pt * pt * pt; // cubic ease-in: slow start, fast finish

                        // Each particle targets a different point along the spike
                        var pTargetY = spikeTop + spikeRange * (p.offsetY / (blip.height || 40));
                        // Slight X spread along the waveform width
                        var pTargetX = targetCX + (p.offsetX * 3);

                        // X: lerp from origin to scattered target
                        var startX = originCX + p.offsetX;
                        var px = startX + (pTargetX - startX) * pEase;

                        // Y: lift up slightly, then accelerate down into the spike
                        var startY = baseline - p.offsetY;
                        var lift = p.arcHeight * 0.3 * Math.sin(pt * Math.PI * 0.7);
                        var py = startY + (pTargetY - startY) * pEase - lift;

                        // Particle shrinks in the last 40% (not throughout)
                        var pSize = pt < 0.6 ? p.size : p.size * (1 - (pt - 0.6) / 0.4 * 0.85);

                        // Opacity: stays bright until last 30%
                        var pOpacity = pt < 0.7 ? blip.opacity : blip.opacity * (1 - (pt - 0.7) / 0.3 * 0.8);
                        pOpacity = Math.max(0, Math.min(1, pOpacity));

                        // Draw particle — skip expensive shadowBlur at low zoom
                        ctx.fillStyle = blipColor + pOpacity + ')';
                        if (zoom >= 1.0) {
                            ctx.shadowBlur = 3 + pEase * 10;
                            ctx.shadowColor = blipColor + (pOpacity * 0.4) + ')';
                        }
                        ctx.fillRect(px - pSize / 2, py - pSize / 2, pSize, pSize);
                        ctx.shadowBlur = 0;
                    }
                } else if (blip.fadeStart > 0) {
                    // Simple fade for blips without absorption target (edge case)
                    var fadeDt = nowSec - blip.fadeStart;
                    bOpacity = Math.max(0, blip.opacity - fadeDt * 0.4);
                    if (bOpacity <= 0) continue;
                }

                // Draw normal blip as a stacked brick
                // In line mode, blips are drawn as notches in the flatline path
                if (!isAbsorbing) {
                    var bx = virtualToCanvas(blip.gridX || blipX);
                    var blipColor = blip.color || 'rgba(0, 230, 118, ';
                    // Both width and height scale with zoom so bricks become large tiles
                    var bw = (blip.brickW || 4) * zoom;
                    var heightScale = zoom > 4 ? 1 + (zoom - 4) * 0.15 : 1; // height grows at high zoom
                    var bh = (blip.brickH || blipH) * heightScale;
                    var sy = (blip.stackY || 0) * heightScale;

                    // Fade-in: grow from 0 over 0.4s. Bricks also drop from above.
                    var age = nowSec - blip.timestamp;
                    if (age < 0) continue; // staggered history brick not yet visible
                    var fadeIn = Math.min(age / 0.4, 1.0);
                    // Drop from above for bricks, not for bloodstream (cells use tube position)
                    var dropOffset = _hb.renderMode !== 'bloodstream' ? (1 - fadeIn) * (1 - fadeIn) * 20 : 0;
                    bh *= fadeIn;
                    bOpacity *= fadeIn;

                    if (_hb.renderMode === 'bloodstream') {
                        // ═══ Blood cell rendering (vein tube) ═══
                        // Cells pack tightly in a tube centered on the flatline
                        var vs = blip.vsize || 200;
                        var baseR;
                        if (vs < 200)       baseR = 2.0;
                        else if (vs < 400)  baseR = 3.0;
                        else if (vs < 800)  baseR = 4.0;
                        else if (vs < 2000) baseR = 5.0;
                        else                baseR = 6.0;
                        // Scale with zoom: gentle at low zoom, grows with zoom at high
                        var cellRadius = baseR * Math.max(zoom * 0.4, 0.8);
                        var cellDiam = cellRadius * 2;

                        // Tube distribution: each cell gets a unique Y position
                        // within the tube. Use bobPhase + gridX to spread cells so
                        // neighbors at the same x don't overlap.
                        var tubeH = 30;
                        var bobPhase = blip.bobPhase || 0;
                        var bobSpeed = blip.bobSpeed || 1.5;
                        // Mix bobPhase with gridX to separate cells at the same column.
                        // Golden ratio offset ensures even distribution across columns.
                        var colHash = ((blip.gridX || 0) * 0.6180339887) % 1;
                        var tubePos = ((bobPhase + colHash * Math.PI * 2) % (Math.PI * 2)) / Math.PI - 1;
                        var cellCY = baseline - tubePos * tubeH - dropOffset;

                        // Bobbing: visible organic drift within the tube
                        var bobAmpY = Math.min(cellRadius * 0.8, 5);
                        var bobAmpX = Math.min(cellRadius * 0.5, 3);
                        cellCY += Math.sin(nowSec * bobSpeed + bobPhase) * bobAmpY;
                        // X jitter for organic feel (not grid-locked)
                        var cellCX = bx + Math.cos(bobPhase * 2.31) * cellRadius * 0.4
                                       + Math.cos(nowSec * bobSpeed * 0.7 + bobPhase) * bobAmpX;

                        // Cell color based on fee rate (warm circulatory palette)
                        var feeRatio = blip.feeRatio || 1;
                        var cellRGB;
                        if (feeRatio < 0.5)      cellRGB = [139, 34, 82];   // deep maroon
                        else if (feeRatio < 1.0)  cellRGB = [220, 53, 69];   // crimson
                        else if (feeRatio < 2.0)  cellRGB = [255, 71, 87];   // bright red
                        else if (feeRatio < 4.0)  cellRGB = [255, 107, 129]; // pink
                        else                      cellRGB = [255, 224, 230]; // pale/white

                        // Solid opacity
                        var cellOpacity = Math.min(bOpacity * fadeIn + 0.15, 1.0);
                        var cellColorStr = 'rgba(' + cellRGB[0] + ',' + cellRGB[1] + ',' + cellRGB[2] + ',';

                        // Draw cell body
                        if (zoom > 6 && cellRadius > 6) {
                            // High zoom: radial gradient (3D sphere effect)
                            var cGrad = ctx.createRadialGradient(
                                cellCX - cellRadius * 0.25, cellCY - cellRadius * 0.25, cellRadius * 0.05,
                                cellCX, cellCY, cellRadius
                            );
                            var coreR = Math.min(cellRGB[0] + 60, 255);
                            var coreG = Math.min(cellRGB[1] + 50, 255);
                            var coreB = Math.min(cellRGB[2] + 40, 255);
                            cGrad.addColorStop(0, 'rgba(' + coreR + ',' + coreG + ',' + coreB + ',' + cellOpacity + ')');
                            cGrad.addColorStop(0.6, cellColorStr + cellOpacity + ')');
                            cGrad.addColorStop(1, cellColorStr + (cellOpacity * 0.8) + ')');
                            ctx.beginPath();
                            ctx.arc(cellCX, cellCY, cellRadius, 0, Math.PI * 2);
                            ctx.fillStyle = cGrad;
                            ctx.fill();
                        } else {
                            // Low zoom: solid filled circle
                            ctx.beginPath();
                            ctx.arc(cellCX, cellCY, cellRadius, 0, Math.PI * 2);
                            ctx.fillStyle = cellColorStr + cellOpacity + ')';
                            ctx.fill();
                        }

                        // Subtle glow for high-fee cells
                        if (feeRatio > 2.0) {
                            ctx.shadowBlur = 3 + Math.min(feeRatio, 6);
                            ctx.shadowColor = cellColorStr + '0.3)';
                            ctx.beginPath();
                            ctx.arc(cellCX, cellCY, cellRadius, 0, Math.PI * 2);
                            ctx.fill();
                            ctx.shadowBlur = 0;
                        }

                        // Cell outline for separation at zoom > 3
                        if (zoom > 3 && cellRadius > 3) {
                            ctx.strokeStyle = 'rgba(8, 15, 30, 0.5)';
                            ctx.lineWidth = zoom > 8 ? 1.5 : 0.8;
                            ctx.beginPath();
                            ctx.arc(cellCX, cellCY, cellRadius, 0, Math.PI * 2);
                            ctx.stroke();
                        }

                        // Fee rate text inside large cells at very high zoom
                        if (zoom > 14 && cellRadius > 8 && blip.feeRate) {
                            ctx.font = Math.round(cellRadius * 0.5) + 'px monospace';
                            ctx.textAlign = 'center';
                            ctx.textBaseline = 'middle';
                            ctx.fillStyle = 'rgba(255, 255, 255, 0.85)';
                            ctx.fillText(blip.feeRate.toFixed(1), cellCX, cellCY);
                        }
                    } else {
                        // Brick rendering (original)
                        // Draw filled rectangle with 1px gap for separation
                        // Gap scales with zoom so bricks stay visually separated.
                        // strokeRect bleeds half its lineWidth outside the rect,
                        // so the gap must exceed that to remain visible.
                        var lw = zoom > 10 ? 2 : 1;
                        var gap = zoom > 4 ? Math.ceil(lw) + 1 : 0;
                        var rx = bx - bw / 2 + gap;
                        var ry = baseline - sy - bh + gap - dropOffset;
                        var rw = bw - gap * 2;
                        var rh = bh - gap * 2;

                        var useRound = zoom > 6 && rw > 4 && rh > 4;
                        var cornerR = useRound ? Math.min(3, rw * 0.12, rh * 0.12) : 0;

                        // At zoom > 8, use a vertical gradient for depth
                        if (zoom > 8 && rh > 6) {
                            var rgbMatch = blipColor.match(/(\d+)\D+(\d+)\D+(\d+)/);
                            if (rgbMatch) {
                                var bR = parseInt(rgbMatch[1]);
                                var bG = parseInt(rgbMatch[2]);
                                var bB = parseInt(rgbMatch[3]);
                                var topR = Math.min(bR + 25, 255);
                                var topG = Math.min(bG + 20, 255);
                                var topB = Math.min(bB + 15, 255);
                                var botR = Math.max(bR - 30, 0);
                                var botG = Math.max(bG - 25, 0);
                                var botB = Math.max(bB - 20, 0);
                                var bGrad = ctx.createLinearGradient(rx, ry, rx, ry + rh);
                                bGrad.addColorStop(0, 'rgba(' + topR + ',' + topG + ',' + topB + ',' + bOpacity + ')');
                                bGrad.addColorStop(0.45, blipColor + bOpacity + ')');
                                bGrad.addColorStop(1, 'rgba(' + botR + ',' + botG + ',' + botB + ',' + bOpacity + ')');
                                ctx.fillStyle = bGrad;
                            } else {
                                ctx.fillStyle = blipColor + bOpacity + ')';
                            }
                        } else {
                            ctx.fillStyle = blipColor + bOpacity + ')';
                        }

                        // Shadow glow only at mid-zoom where bricks are visible but
                        // not outlined. Skip at low zoom (sub-pixel, too expensive
                        // with thousands of bricks) and high zoom (outlines instead).
                        if (zoom >= 1.0 && zoom < 4) {
                            ctx.shadowBlur = 4;
                            ctx.shadowColor = blipColor + (bOpacity * 0.4) + ')';
                        }
                        if (useRound) {
                            ctx.beginPath();
                            roundRect(ctx, rx, ry, rw, rh, cornerR);
                            ctx.fill();
                        } else {
                            ctx.fillRect(rx, ry, rw, rh);
                        }
                        ctx.shadowBlur = 0;

                        // Dark outline at 4x+ zoom for clear brick separation
                        if (zoom > 4) {
                            ctx.lineWidth = lw;
                            var ins = lw / 2;
                            if (useRound) {
                                ctx.strokeStyle = 'rgba(8, 15, 30, 0.6)';
                                ctx.beginPath();
                                roundRect(ctx, rx + ins, ry + ins, rw - lw, rh - lw, Math.max(cornerR - ins, 0));
                                ctx.stroke();
                                // Darker bottom edge for shadow depth
                                if (zoom > 8 && rh > 8) {
                                    ctx.strokeStyle = 'rgba(0, 0, 0, 0.35)';
                                    ctx.lineWidth = Math.min(lw + 0.5, 2.5);
                                    var botIns = ctx.lineWidth / 2;
                                    ctx.beginPath();
                                    ctx.moveTo(rx + cornerR, ry + rh - botIns);
                                    ctx.lineTo(rx + rw - cornerR, ry + rh - botIns);
                                    ctx.stroke();
                                    ctx.lineWidth = lw;
                                }
                            } else {
                                ctx.strokeStyle = 'rgba(8, 15, 30, 0.7)';
                                // Inset by half lineWidth so stroke stays inside the rect
                                ctx.strokeRect(rx + ins, ry + ins, rw - lw, rh - lw);
                            }
                        }

                        // At high zoom: show fee rate + value on brick face (clipped to brick)
                        if (zoom > 8 && rw > 22 && rh > 10) {
                            ctx.save();
                            ctx.beginPath();
                            if (useRound) {
                                roundRect(ctx, rx, ry, rw, rh, cornerR);
                            } else {
                                ctx.rect(rx, ry, rw, rh);
                            }
                            ctx.clip();
                            var pad = Math.max(4, rw * 0.08);
                            var fs = Math.min(Math.floor(rh * 0.32), Math.floor(rw * 0.22), 13);
                            if (fs >= 5) {
                                // Fee rate text
                                ctx.font = 'bold ' + fs + 'px monospace';
                                ctx.textAlign = 'left';
                                ctx.textBaseline = 'top';
                                var feeStr = (blip.feeRate ? blip.feeRate.toFixed(1) : '?') + ' s/vB';
                                var textY = ry + pad;
                                // Dark text shadow for readability on any color
                                ctx.fillStyle = 'rgba(0,0,0,0.55)';
                                ctx.fillText(feeStr, rx + pad + 1, textY + 1);
                                ctx.fillStyle = 'rgba(255,255,255,0.95)';
                                ctx.fillText(feeStr, rx + pad, textY);

                                // Value text (second line)
                                if (rh > 24 && fs > 6) {
                                    var valFs = fs - 1;
                                    ctx.font = valFs + 'px monospace';
                                    var valY = textY + fs + Math.max(2, rh * 0.06);
                                    var valText = '';
                                    if (blip.value) {
                                        var btc = blip.value / 1e8;
                                        var valStr;
                                        if (btc >= 1.0)       valStr = btc.toFixed(2);
                                        else if (btc >= 0.01) valStr = btc.toFixed(4);
                                        else if (btc >= 0.001) valStr = btc.toFixed(5);
                                        else                  valStr = btc.toFixed(6);
                                        valText = '\u{20bf}' + valStr;
                                    } else if (blip.vsize) {
                                        valText = Math.round(blip.vsize) + ' vB';
                                    }
                                    if (valText) {
                                        ctx.fillStyle = 'rgba(0,0,0,0.45)';
                                        ctx.fillText(valText, rx + pad + 1, valY + 1);
                                        ctx.fillStyle = 'rgba(255,255,255,0.8)';
                                        ctx.fillText(valText, rx + pad, valY);
                                    }
                                }
                            }
                            ctx.restore();
                        }
                    }
                }
                ctx.shadowBlur = 0;
            }

            // Prune blips — never prune the live (open) flatline so txs persist
            // until the next block. Only prune closed (historical) segments.
            var isLiveSeg = (seg.type === 'flatline' && seg.x_end === null);
            if (!isLiveSeg && seg.blips.length > MAX_BLIPS_PER_SEGMENT) {
                var cutoff = nowSec;
                // Always remove faded absorption particles
                seg.blips = seg.blips.filter(function(b) {
                    if (b.fadeStart > 0) return (cutoff - b.fadeStart) < 3;
                    return true;
                });
                // If still over cap, only remove blips behind the viewport left edge
                if (seg.blips.length > MAX_BLIPS_PER_SEGMENT) {
                    var excess = seg.blips.length - MAX_BLIPS_PER_SEGMENT;
                    var offscreen = [];
                    var onscreen = [];
                    for (var pi = 0; pi < seg.blips.length; pi++) {
                        var bx = seg.blips[pi].gridX || seg.blips[pi].x;
                        if (bx < viewLeft) {
                            offscreen.push(seg.blips[pi]);
                        } else {
                            onscreen.push(seg.blips[pi]);
                        }
                    }
                    if (offscreen.length >= excess) {
                        offscreen.sort(function(a, b) { return (b.timestamp || 0) - (a.timestamp || 0); });
                        seg.blips = onscreen.concat(offscreen.slice(0, offscreen.length - excess));
                    }
                }
            } else if (isLiveSeg) {
                // Live segment: only clean up faded absorption particles, never prune txs
                var cutoff2 = nowSec;
                var hasFaded = false;
                for (var fi = 0; fi < seg.blips.length; fi++) {
                    if (seg.blips[fi].fadeStart > 0 && (cutoff2 - seg.blips[fi].fadeStart) >= 3) {
                        hasFaded = true; break;
                    }
                }
                if (hasFaded) {
                    seg.blips = seg.blips.filter(function(b) {
                        if (b.fadeStart > 0) return (cutoff2 - b.fadeStart) < 3;
                        return true;
                    });
                }
            }
        }
    }

    // ── Momentum scrolling ──────────────────────────────────────
    function startMomentum(velocity) {
        if (!_hb) return;
        _hb._momentumVel = velocity * 15; // scale up for visible effect
        _hb._momentumId = null;

        function tick() {
            if (!_hb || Math.abs(_hb._momentumVel) < 0.5) {
                _hb._momentumId = null;
                checkAutoFollow();
                return;
            }
            _hb.viewOffset -= _hb._momentumVel / _hb.zoom;
            _hb._momentumVel *= 0.92; // friction
            _hb._momentumId = requestAnimationFrame(tick);
        }
        tick();
    }

    function stopMomentum() {
        if (_hb && _hb._momentumId) {
            cancelAnimationFrame(_hb._momentumId);
            _hb._momentumId = null;
        }
    }

    // ── Pinch-to-zoom helper ──────────────────────────────────
    function touchDistance(t1, t2) {
        var dx = t1.clientX - t2.clientX;
        var dy = t1.clientY - t2.clientY;
        return Math.sqrt(dx * dx + dy * dy);
    }

    function touchMidpoint(t1, t2, rect) {
        return {
            x: (t1.clientX + t2.clientX) / 2 - rect.left,
            y: (t1.clientY + t2.clientY) / 2 - rect.top
        };
    }

    // ── Input handling ─────────────────────────────────────────

    // Legacy: tryJumpToLive now handled by control bar
    function tryJumpToLive() { return false; }

    function tryControlClick(mx, my) {
        if (!_hb || !_hb._ctrlBtns) return false;
        for (var i = 0; i < _hb._ctrlBtns.length; i++) {
            var btn = _hb._ctrlBtns[i];
            if (mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h) {
                handleControlClick(btn.id);
                return true;
            }
        }
        return false;
    }

    function checkControlHover(mx, my) {
        if (!_hb || !_hb._ctrlBtns) { _hb._ctrlHover = null; return; }
        _hb._ctrlHover = null;
        for (var i = 0; i < _hb._ctrlBtns.length; i++) {
            var btn = _hb._ctrlBtns[i];
            if (mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h) {
                _hb._ctrlHover = btn;
                return;
            }
        }
    }

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
                // Don't checkAutoFollow here - zooming should stay where you aimed
            }
        }, { passive: false });

        // Mouse drag: click and drag to pan
        listen(canvas, 'mousedown', function(e) {
            if (!_hb) return;

            var rect = canvas.getBoundingClientRect();
            var mx = e.clientX - rect.left, my = e.clientY - rect.top;
            if (tryJumpToLive(mx, my)) return;
            // Don't start drag if clicking a control button (handled in click event)
            if (_hb._ctrlBtns) {
                for (var ci = 0; ci < _hb._ctrlBtns.length; ci++) {
                    var cb = _hb._ctrlBtns[ci];
                    if (mx >= cb.x && mx <= cb.x + cb.w && my >= cb.y && my <= cb.y + cb.h) return;
                }
            }

            stopMomentum();
            _hb.isDragging = true;
            _hb.dragStartX = e.clientX;
            _hb.dragStartY = e.clientY;
            _hb.dragStartOffset = _hb.viewOffset;
            _hb.dragStartOffsetY = _hb.viewOffsetY || 0;
            _hb._lastDragX = e.clientX;
            _hb._lastDragTime = Date.now();
            _hb._dragVelocity = 0;
            canvas.style.cursor = 'grabbing';
        });

        listen(window, 'mousemove', function(e) {
            if (!_hb) return;

            if (_hb.isDragging) {
                var dx = e.clientX - _hb.dragStartX;
                var dy = e.clientY - (_hb.dragStartY || e.clientY);
                _hb.viewOffset = _hb.dragStartOffset - dx / _hb.zoom;
                _hb.viewOffsetY = (_hb.dragStartOffsetY || 0) + dy;
                _hb.autoFollow = false;
                // Track velocity for momentum
                var now = Date.now();
                var dt = now - (_hb._lastDragTime || now);
                if (dt > 0) {
                    _hb._dragVelocity = (e.clientX - (_hb._lastDragX || e.clientX)) / dt;
                }
                _hb._lastDragX = e.clientX;
                _hb._lastDragTime = now;
                checkAutoFollow();
            } else {
                // Hover detection
                var rect = canvas.getBoundingClientRect();
                var mx = e.clientX - rect.left;
                var my = e.clientY - rect.top;
                checkControlHover(mx, my);
                // Skip block/flatline hover when over control buttons
                if (_hb._ctrlHover) {
                    _hb.hoveredBlock = null;
                    _hb.hoveredBlip = null;
                    _hb.hoveredFlatline = null;
                    canvas.style.cursor = 'pointer';
                } else if (mx >= 0 && mx <= _hb.width && my >= 0 && my <= _hb.height) {
                    var vx = canvasToVirtual(mx);
                    var baseline = _hb.height * 0.55 + (_hb.viewOffsetY || 0);
                    _hb.hoveredBlock = blockAtVirtualX(vx);
                    _hb.hoveredBlip = null;
                    _hb.hoveredFlatline = null;

                    if (!_hb.hoveredBlock) {
                        // Check for blip hover
                        // 2D hit test at zoom >= 4x
                        if (my < baseline && _hb.zoom >= 4) {
                            _hb.hoveredBlip = blipAtCanvasXY(mx, my, baseline);
                        }
                        // Fallback to x-only match at lower zoom
                        if (!_hb.hoveredBlip && my < baseline && _hb.zoom >= 1.5) {
                            _hb.hoveredBlip = blipAtVirtualX(vx, 4 / _hb.zoom);
                        }
                        if (!_hb.hoveredBlip && my >= baseline) {
                            // Only show flatline tooltip when cursor is below the baseline
                            _hb.hoveredFlatline = flatlineAtVirtualX(vx);
                        }
                    }
                    _hb.hoverCanvasX = mx;
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
                // Start momentum scrolling
                var vel = _hb._dragVelocity || 0;
                if (Math.abs(vel) > 0.1) {
                    startMomentum(vel);
                }
            }
        });

        listen(canvas, 'click', function(e) {
            if (!_hb) return;
            var rect = canvas.getBoundingClientRect();
            var mx = e.clientX - rect.left;
            var my = e.clientY - rect.top;
            if (tryJumpToLive(mx, my)) return;
            if (tryControlClick(mx, my)) return;

            // Click on a brick -> pin tooltip. If already pinned, unpin (or open mempool)
            if (_hb._pinnedBlip) {
                // If clicking the pinned brick again -> open mempool.space
                if (_hb.hoveredBlip && _hb.hoveredBlip === _hb._pinnedBlip && _hb._pinnedBlip.txid) {
                    window.open('https://mempool.space/tx/' + _hb._pinnedBlip.txid, '_blank');
                }
                // Always unpin on any click when pinned
                _hb._pinnedBlip = null;
            } else if (_hb.hoveredBlip) {
                // Pin the hovered brick
                _hb._pinnedBlip = _hb.hoveredBlip;
            }
        });

        // Touch support: 1 finger = pan with momentum, 2 fingers = pinch-to-zoom
        listen(canvas, 'touchstart', function(e) {
            if (!_hb) return;
            stopMomentum();

            if (e.touches.length === 2) {
                // Pinch start
                _hb._pinching = true;
                _hb.isDragging = false;
                _hb._touchLocked = 'pinch';
                _hb._pinchStartDist = touchDistance(e.touches[0], e.touches[1]);
                _hb._pinchStartZoom = _hb.zoom;
                var rect = canvas.getBoundingClientRect();
                _hb._pinchMid = touchMidpoint(e.touches[0], e.touches[1], rect);
                _hb._pinchVx = canvasToVirtual(_hb._pinchMid.x);
            } else if (e.touches.length === 1) {
                // Pan start — don't commit to horizontal drag yet
                _hb._pinching = false;
                _hb.isDragging = false;
                _hb._touchLocked = null; // null = undecided, 'h' = horizontal, 'v' = vertical
                _hb._touchStartX = e.touches[0].clientX;
                _hb._touchStartY = e.touches[0].clientY;
                _hb.dragStartX = e.touches[0].clientX;
                _hb.dragStartOffset = _hb.viewOffset;
                _hb._lastDragX = e.touches[0].clientX;
                _hb._lastDragTime = Date.now();
                _hb._dragVelocity = 0;
            }
        }, { passive: true });

        listen(canvas, 'touchmove', function(e) {
            if (!_hb) return;

            if (_hb._pinching && e.touches.length === 2) {
                e.preventDefault();
                // Pinch zoom
                var dist = touchDistance(e.touches[0], e.touches[1]);
                var scale = dist / _hb._pinchStartDist;
                var newZoom = Math.max(_hb.minZoom, Math.min(_hb.maxZoom, _hb._pinchStartZoom * scale));
                _hb.zoom = newZoom;
                _hb.viewOffset = _hb._pinchVx - _hb._pinchMid.x / newZoom;
                _hb.autoFollow = false;
            } else if (e.touches.length === 1) {
                // Decide direction on first significant move
                if (!_hb._touchLocked) {
                    var tdx = Math.abs(e.touches[0].clientX - _hb._touchStartX);
                    var tdy = Math.abs(e.touches[0].clientY - _hb._touchStartY);
                    if (tdx + tdy < 8) return; // too small to decide
                    _hb._touchLocked = tdx > tdy ? 'h' : 'v';
                    if (_hb._touchLocked === 'h') _hb.isDragging = true;
                }
                if (_hb._touchLocked === 'v') return; // let browser handle vertical scroll
                e.preventDefault(); // horizontal pan — block scroll
                // Pan
                var dx = e.touches[0].clientX - _hb.dragStartX;
                _hb.viewOffset = _hb.dragStartOffset - dx / _hb.zoom;
                _hb.autoFollow = false;
                // Track velocity
                var now = Date.now();
                var dt = now - (_hb._lastDragTime || now);
                if (dt > 0) {
                    _hb._dragVelocity = (e.touches[0].clientX - (_hb._lastDragX || e.touches[0].clientX)) / dt;
                }
                _hb._lastDragX = e.touches[0].clientX;
                _hb._lastDragTime = now;
                checkAutoFollow();
            }
        }, { passive: false });

        listen(canvas, 'touchend', function(e) {
            if (!_hb) return;
            _hb._touchLocked = null; // reset direction lock for next touch

            if (_hb._pinching) {
                _hb._pinching = false;
                checkAutoFollow();
                return;
            }

            if (_hb.isDragging) {
                _hb.isDragging = false;

                // Momentum
                var vel = _hb._dragVelocity || 0;
                if (Math.abs(vel) > 0.1) {
                    startMomentum(vel);
                }

                // Tap detection (minimal drag distance = click)
                if (e.changedTouches && e.changedTouches.length > 0) {
                    var dx = Math.abs(e.changedTouches[0].clientX - _hb.dragStartX);
                    if (dx < 10) {
                        // Treat as tap
                        var rect = canvas.getBoundingClientRect();
                        var mx = e.changedTouches[0].clientX - rect.left;
                        var vx = canvasToVirtual(mx);
                        var tapped = blockAtVirtualX(vx);

                        var my = e.changedTouches[0].clientY - rect.top;
                        if (tryJumpToLive(mx, my)) return;

                        if (tapped) {
                            _hb.hoveredBlock = (_hb.hoveredBlock === tapped) ? null : tapped;
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
            viewOffset: -w * HEAD_POSITION_FRAC,
            viewOffsetY: 0,       // vertical pan offset (0 = default baseline)
            autoFollow: true,
            paused: false,
            renderMode: 'bricks', // 'bricks' or 'bloodstream'
            zoom: 1.9,              // default zoom for brick-level detail
            minZoom: 0.1,
            maxZoom: 50.0,

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
            _pinnedBlip: null,
            hoveredFlatline: null,
            hoverCanvasX: 0,
            _jumpBtn: null,
            _flashTimer: null,

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

        // Connect to own node SSE feed (falls back to mempool.space WS)
        connectOwnFeed();
    };

    window.pushHeartbeatBlocks = function(json, replay) {
        if (!_hb) return;
        var isReplay = !!replay;
        try {
            var blocks = JSON.parse(json);
            if (!Array.isArray(blocks)) return;
            for (var i = 0; i < blocks.length; i++) {
                var b = blocks[i];

                // Deduplicate: skip if this block height is already in the timeline
                // (SSE block handler + LiveStats poll can both push the same block)
                // Exception: if existing block was estimated (total_fees=0), replace with real data
                if (!isReplay && b.height) {
                    var existingSeg = null;
                    for (var di = _hb.timeline.length - 1; di >= Math.max(0, _hb.timeline.length - 10); di--) {
                        if (_hb.timeline[di].type === 'block' && _hb.timeline[di].height === b.height) {
                            existingSeg = _hb.timeline[di];
                            break;
                        }
                    }
                    if (existingSeg) {
                        // Replace estimated waveform with real data
                        if (existingSeg.total_fees === 0 && b.total_fees > 0) {
                            var replacement = createBlockSegment(b, existingSeg.x_start);
                            replacement.x_start = existingSeg.x_start;
                            replacement.x_end = existingSeg.x_end;
                            existingSeg.points = replacement.points;
                            existingSeg.color = replacement.color;
                            existingSeg.tx_count = replacement.tx_count;
                            existingSeg.total_fees = replacement.total_fees;
                            existingSeg.weight = replacement.weight;
                            existingSeg.inter_block_seconds = replacement.inter_block_seconds;
                        }
                        continue;
                    }
                }

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
                    // Use pre-flash color if flash is active (avoid white flatline)
                    if (!isReplay) {
                        lastSeg.color = _hb._preFlashColor || _hb.currentColor || COLORS.healthy;
                    }
                    // Shatter all blips into particles — they get absorbed into the spike
                    if (lastSeg.blips) {
                        var absorbX = _hb.virtualX;
                        var absorbNow = Date.now() / 1000;
                        // Scale particle count with zoom: at low zoom particles are
                        // sub-pixel, so fewer are needed. Also cap total particles to
                        // avoid lag spikes when thousands of blips absorb at once.
                        var activeBlips = 0;
                        for (var ci = 0; ci < lastSeg.blips.length; ci++) {
                            if (lastSeg.blips[ci].fadeStart === 0) activeBlips++;
                        }
                        var zoom = _hb.zoom;
                        var maxPartsPerBlip;
                        if (zoom < 0.5 || activeBlips > 1000) maxPartsPerBlip = 1;
                        else if (zoom < 1.0 || activeBlips > 500) maxPartsPerBlip = 2;
                        else maxPartsPerBlip = 3 + Math.floor(Math.random() * 3);

                        for (var bi = 0; bi < lastSeg.blips.length; bi++) {
                            var blp = lastSeg.blips[bi];
                            if (blp.fadeStart === 0) {
                                blp.fadeStart = absorbNow;
                                blp.absorbTargetX = absorbX;
                                blp.absorbOriginX = blp.x;
                                var nParts = maxPartsPerBlip;
                                blp.particles = [];
                                for (var pi = 0; pi < nParts; pi++) {
                                    blp.particles.push({
                                        offsetX: (Math.random() - 0.5) * 8,
                                        offsetY: Math.random() * blp.height * 0.8,
                                        speed: 0.7 + Math.random() * 0.6,
                                        arcHeight: 20 + Math.random() * 60,
                                        delay: Math.random() * 0.3,
                                        size: 1.5 + Math.random() * 2.5
                                    });
                                }
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

                // Show block arrival announcement (live blocks only)
                if (!isReplay && b.height) {
                    var feeBtc = (b.total_fees || 0) / 100000000;
                    var heightStr = b.height.toLocaleString();
                    var nowAnn = Date.now() / 1000;
                    _hb._announcement = {
                        title: 'Block #' + heightStr + ' found',
                        subtitle: feeBtc.toFixed(4) + ' BTC fees \u00b7 ' + (b.tx_count || 0).toLocaleString() + ' txs',
                        color: blockSeg.color || _hb.currentColor || COLORS.healthy,
                        start: nowAnn,
                        end: nowAnn + 12.0
                    };
                }
            }

            if (_hb.recentBlocks.length > RETARGET_PERIOD) {
                _hb.recentBlocks = _hb.recentBlocks.slice(-RETARGET_PERIOD);
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
                    var advancePx = elapsed * FLATLINE_PX_PER_SEC;
                    _hb.virtualX += advancePx;

                    // No synthetic blips - only real txs from the WS feed
                    // Reset viewport to default state
                    _hb.zoom = 1.9;
                    _hb.viewOffsetY = 0;
                    _hb.autoFollow = true;
                    _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC);

                }

                // Place any SSE history txs that arrived before the replay
                if (_hb._pendingHistory) {
                    var ph = _hb._pendingHistory;
                    _hb._pendingHistory = null;
                    console.log('[heartbeat] placing deferred history after replay');
                    placeHistoryTxs(ph.txs, ph.last_block_ts);
                }
            }

        } catch (e) {
            // silently ignore malformed block JSON
        }
    };

    // Prune timeline segments that are far behind the viewport to limit memory
    function pruneTimeline() {
        // Keep all replay history — only prune when timeline is very large
        // and segments are far behind the viewport (10x canvas width)
        if (!_hb || _hb.timeline.length < 5000) return;
        var minX = _hb.viewOffset - _hb.width * 10 / Math.max(_hb.zoom, 0.1);
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
            // silently ignore malformed live JSON
        }
    };

    window.destroyHeartbeat = function() {
        if (!_hb) return;
        stopMomentum();
        if (_hb.rafId) cancelAnimationFrame(_hb.rafId);
        if (_hb._flashTimer) clearTimeout(_hb._flashTimer);
        if (_hb.resizeObs) _hb.resizeObs.disconnect();
        if (_hb._sseReconnectTimer) clearInterval(_hb._sseReconnectTimer);
        if (_hb._sse) {
            try { _hb._sse.close(); } catch(e) {}
        }
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

    // Clean up SSE connection on page unload
    window.addEventListener('beforeunload', function() {
        if (window.destroyHeartbeat) window.destroyHeartbeat();
    });

    // When tab regains focus, replay queued blocks as compressed history
    // and advance virtualX to catch up with elapsed time
    document.addEventListener('visibilitychange', function() {
        if (!_hb || document.hidden) return;
        var now = Date.now() / 1000;
        var elapsed = now - (_hb.lastFrameTime || now);

        // Grab buffered txs and clear queues
        var hiddenTxs = _hb._hiddenTxBuffer || [];
        _hb._hiddenTxBuffer = [];
        _hb._txBatchQueue = [];

        // Replay any blocks that arrived while hidden
        var queued = _hb._blockQueue || [];
        _hb._blockQueue = [];

        if (queued.length > 0) {
            console.log('[heartbeat] replaying', queued.length, 'queued blocks from background');
            // Build block array with inter-block times and replay as compressed history
            var replayBlocks = [];
            var prevTs = _hb.lastBlockTime || (now - 600);
            for (var qi = 0; qi < queued.length; qi++) {
                var qb = queued[qi];
                var blockTs = _hb.lastBlockTime > 0 ? prevTs + 600 : now;
                // Use confirmed_count if available, else estimate
                replayBlocks.push({
                    height: qb.height,
                    timestamp: blockTs,
                    tx_count: qb.confirmed_count || 3000,
                    total_fees: qb.total_fees || 0,
                    size: qb.size || 0,
                    weight: qb.weight || 3000000,
                    inter_block_seconds: 600
                });
                prevTs = blockTs;
            }
            // Use replay=true so they get compressed flatlines (not live positioning)
            // pushHeartbeatBlocks(replay=true) already fast-forwards virtualX for
            // elapsed time since the last block — no additional advance needed here.
            window.pushHeartbeatBlocks(JSON.stringify(replayBlocks), true);
        } else if (elapsed > 2) {
            // No queued blocks — just advance the live flatline to catch up
            var liveSeg2 = _hb.timeline[_hb.timeline.length - 1];
            if (liveSeg2 && liveSeg2.type === 'flatline' && liveSeg2.x_end === null) {
                _hb.virtualX += elapsed * FLATLINE_PX_PER_SEC;
            }
        }

        // Spread buffered txs across the current live flatline
        if (hiddenTxs.length > 0) {
            var liveSeg3 = _hb.timeline[_hb.timeline.length - 1];
            if (liveSeg3 && liveSeg3.type === 'flatline' && liveSeg3.x_end === null) {
                // Use the actual flatline bounds, not time-based estimation
                var gapStart = liveSeg3.x_start;
                var gapSpan = _hb.virtualX - liveSeg3.x_start;
                if (gapSpan < 10) gapSpan = 0;
                var medianFee = _hb._wsMedianFee || 5;
                // Cap density like history placement
                var maxForGap = gapSpan > 0 ? Math.max(Math.floor(gapSpan / 5 * 3), 100) : 0;
                var cap = Math.min(hiddenTxs.length, maxForGap);
                // Use the segment's column heights so stacking is consistent
                if (!liveSeg3._colHeights) liveSeg3._colHeights = {};
                var htStackMap = liveSeg3._colHeights;
                for (var hti = 0; hti < cap; hti++) {
                    var htx = hiddenTxs[hti];
                    if (!htx.fee || !htx.vsize) continue;
                    var htFeeRate = htx.fee / htx.vsize;
                    var htFeeNorm = Math.min(Math.log2(htFeeRate + 1) / 6, 1.0);
                    var htVX = gapStart + Math.random() * gapSpan * 0.95;
                    var htBrickH = 3 + htFeeNorm * 14 + Math.random() * 3;
                    var htGridX = Math.round(htVX / 5) * 5;
                    var htStackY = htStackMap[htGridX] || 0;
                    htStackMap[htGridX] = htStackY + htBrickH;
                    liveSeg3.blips.push({
                        x: htVX, gridX: htGridX,
                        height: htBrickH + htStackY, brickH: htBrickH, brickW: 4, stackY: htStackY,
                        color: feeRateColor(htFeeRate, medianFee), opacity: 0.75 + htFeeNorm * 0.2,
                        txCount: 1, txid: htx.txid || null,
                        feeRate: Math.round(htFeeRate * 10) / 10,
                        vsize: htx.vsize || 0, value: htx.value || 0,
                        timestamp: Date.now() / 1000, fadeStart: 0,
                        bobPhase: Math.random() * Math.PI * 2,
                        bobSpeed: 1.2 + htFeeNorm * 0.8,
                        lane: Math.floor(Math.random() * 5) - 2,
                        feeRatio: medianFee > 0 ? htFeeRate / medianFee : 1
                    });
                }
                console.log('[heartbeat] placed', Math.min(cap, hiddenTxs.length), 'buffered txs across tab-hidden gap');
            }
        }

        // Update lastFrameTime so the draw loop doesn't also try to catch up
        _hb.lastFrameTime = now;
    });

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
            label = 'Bradycardia'; color = COLORS.steady;
        } else if (bpm <= 1.3) {
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

        // Draw each block segment with its own color, store hit regions
        var x = 0;
        var vScale = 0.4; // compress vertically
        var hitRegions = [];
        for (var wi = 0; wi < waveforms.length; wi++) {
            var wf = waveforms[wi];
            var interBlock = wf.block.inter_block_seconds || 600;

            var feeRate = wf.block.total_fees ? wf.block.total_fees / 100000 : 0;
            var segColor = computeColor(interBlock, feeRate, 0);

            ctx.beginPath();
            ctx.moveTo(x, baseline);

            // Draw flatline before waveform
            var flatEnd = x + wf.flatline * scale;
            ctx.lineTo(flatEnd, baseline);

            var waveStart = flatEnd;
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

            hitRegions.push({ x1: waveStart, x2: x, block: wf.block, color: segColor });
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

        // ── Rhythm strip interactivity: hover tooltip + click to open ──
        canvas._rhythmHitRegions = hitRegions;

        // Draw tooltip for hovered block (persists across redraws)
        if (canvas._rhythmHovered) {
            var hov = canvas._rhythmHovered;
            // Re-match against new hit regions (positions may shift on resize)
            var hovMid = (hov.x1 + hov.x2) / 2;
            for (var hri = 0; hri < hitRegions.length; hri++) {
                if (hitRegions[hri].block.height === hov.block.height) {
                    hov = hitRegions[hri];
                    canvas._rhythmHovered = hov;
                    hovMid = (hov.x1 + hov.x2) / 2;
                    break;
                }
            }
            var lines = [
                'Block #' + (hov.block.height || 0).toLocaleString(),
                (hov.block.tx_count || 0).toLocaleString() + ' txs',
                ((hov.block.total_fees || 0) / 100000000).toFixed(4) + ' BTC fees',
                formatDuration(hov.block.inter_block_seconds || 600) + ' wait'
            ];
            drawTooltipBox(ctx, lines, hovMid, baseline - 20, hov.color);
        }

        // Only set up event handlers once (skip on tooltip redraws)
        if (canvas._rhythmHandlersSet) return;
        canvas._rhythmHandlersSet = true;

        canvas.addEventListener('click', function(e) {
            var r = canvas.getBoundingClientRect();
            var mx = e.clientX - r.left;
            var regions = canvas._rhythmHitRegions;
            for (var ri = 0; ri < regions.length; ri++) {
                if (mx >= regions[ri].x1 && mx <= regions[ri].x2 && regions[ri].block.height) {
                    window.open('https://mempool.space/block/' + regions[ri].block.height, '_blank');
                    return;
                }
            }
        });

        canvas.addEventListener('mousemove', function(e) {
            var r = canvas.getBoundingClientRect();
            var mx = e.clientX - r.left;
            var regions = canvas._rhythmHitRegions;
            var found = null;
            for (var ri = 0; ri < regions.length; ri++) {
                if (mx >= regions[ri].x1 && mx <= regions[ri].x2) {
                    found = regions[ri];
                    break;
                }
            }
            if (found === canvas._rhythmHovered) return;
            canvas._rhythmHovered = found;
            canvas.style.cursor = found ? 'pointer' : '';
            window.renderRhythmStrip(canvasId, blocksJson);
        });

        canvas.addEventListener('mouseleave', function() {
            if (canvas._rhythmHovered) {
                canvas._rhythmHovered = null;
                canvas.style.cursor = '';
                window.renderRhythmStrip(canvasId, blocksJson);
            }
        });
    };

    // ── Center view on live head ─────────────────────────────
    window.heartbeatCenter = function() {
        if (!_hb) return;
        _hb.autoFollow = true;
        _hb.viewOffsetY = 0;
        _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
        _hb.hoveredBlock = null;
        _hb.hoveredBlip = null;
        _hb._pinnedBlip = null;
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
    // Stores the real color in _preFlashColor so flatline stamping isn't affected
    window.heartbeatFlash = function() {
        if (!_hb) return;
        _hb._preFlashColor = _hb.currentColor; // preserve for flatline stamping
        _hb._flashColor = _hb.currentColor;
        _hb.currentColor = '#ffffff';
        _hb.prevColor = '#ffffff';
        _hb.colorLerp = 1;
        if (_hb._flashTimer) clearTimeout(_hb._flashTimer);
        _hb._flashTimer = setTimeout(function() {
            if (_hb && _hb._flashColor) {
                // Skip the lerp — just snap to the correct color.
                // The gradual white-to-color fade looked good in theory but
                // caused the line to stay white when computeColor triggered
                // a target change mid-lerp (prevColor was still '#ffffff').
                _hb.currentColor = _hb._flashColor;
                _hb.prevColor = _hb._flashColor;
                _hb.targetColor = _hb._flashColor;
                _hb.colorLerp = 1;
                _hb._flashColor = null;
                _hb._preFlashColor = null;
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
        } else if (color === COLORS.steady) {
            condition = 'Steady';
            description = 'Holding pace. Some activity building.';
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

    function heartbeatCapture(vitalsJson) {
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
    }

    // Trigger download of captured image
    window.heartbeatDownloadCapture = function(vitalsJson) {
        var dataUrl = heartbeatCapture(vitalsJson);
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
        if (blocks.length > RHYTHM_STRIP_BLOCKS) {
            blocks = blocks.slice(-RHYTHM_STRIP_BLOCKS);
        }
        return JSON.stringify(blocks);
    };

})();
