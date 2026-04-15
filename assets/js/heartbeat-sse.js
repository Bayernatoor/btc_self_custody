// heartbeat-sse.js — SSE connection, tx batching, and mempool fallback
import { getState, FLATLINE_PX_PER_SEC } from './heartbeat-state.js';
import { feeRateColor } from './heartbeat-timeline.js';

// Place SSE history txs on the live flatline
export function placeHistoryTxs(txs, lastBlockTs, instant) {
    var _hb = getState();
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

    var flatlineSpan = _hb.virtualX - liveSeg.x_start;
    var stackMap = {};

    // Place all history txs — cull threshold is 30K so there's plenty of room.
    // Only cap by available grid columns to prevent extreme stacking.
    var gridCols = Math.max(1, Math.floor(flatlineSpan / 5));
    var maxBricks = gridCols * 3;

    console.log('[heartbeat] placeHistoryTxs: flatlineSpan=' + Math.round(flatlineSpan) +
        'px, virtualX=' + Math.round(_hb.virtualX) +
        ', segStart=' + Math.round(liveSeg.x_start) +
        ', effectiveBlockTs=' + effectiveBlockTs +
        ', elapsed=' + Math.round(elapsedSinceBlock) + 's');

    // Stagger history bricks so they animate in from left to right over ~1.5s.
    // On reconnect (instant=true), place bricks immediately with no animation.
    var dropNow = Date.now() / 1000;
    var DROP_DURATION = instant ? 0 : 1.5;

    for (var i = 0; i < txs.length && placed < maxBricks; i++) {
        var tx = txs[i];
        if (!tx.fee || !tx.vsize) continue;

        // Spread randomly across the flatline, leaving last 20px for live txs.
        var usableSpan = Math.max(10, flatlineSpan - 20);
        var txVX = liveSeg.x_start + Math.random() * usableSpan;

        var feeRate = tx.fee / tx.vsize;
        var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);

        var brickH = 3 + feeNorm * 14 + Math.random() * 3;
        var gridX = Math.round(txVX / 5) * 5;
        var stackY = stackMap[gridX] || 0;
        var histMaxStack = (_hb.height || 400) * 0.35;
        if (stackY > histMaxStack) continue;
        stackMap[gridX] = stackY + brickH;

        // Stagger: left-most bricks appear first, sweeping right
        var xFrac = flatlineSpan > 0 ? (txVX - liveSeg.x_start) / flatlineSpan : 0;
        var dropDelay = instant ? 0 : xFrac * DROP_DURATION;

        liveSeg.blips.push({
            x: txVX, gridX: gridX,
            height: brickH + stackY, brickH: brickH, brickW: 4, stackY: stackY,
            color: feeRateColor(feeRate, medianFee), opacity: 0.75 + feeNorm * 0.2,
            txCount: 1, txid: tx.txid || null,
            feeRate: Math.round(feeRate * 10) / 10,
            vsize: tx.vsize || 0, value: tx.value || 0,
            timestamp: dropNow + dropDelay, isDrop: !instant, fadeStart: 0,
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
export function processLiveBlock(block) {
    var _hb = getState();
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

    // Build block data — SSE includes real block stats from RPC + mempool fees.
    // Compute inter-block time from the timeline's last block segment (authoritative)
    // rather than the mutable lastBlockTime which can be overwritten by LiveStats.
    var blockTs = block.timestamp || Math.floor(Date.now() / 1000);
    var inter = 600;
    for (var pi = _hb.timeline.length - 1; pi >= 0; pi--) {
        if (_hb.timeline[pi].type === 'block') {
            var prevTs = _hb.timeline[pi].timestamp;
            if (prevTs > 0 && blockTs >= prevTs) {
                inter = Math.max(1, blockTs - prevTs);
            }
            break;
        }
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

    // Clear mining overlay and cancel pending delay (block arrived)
    _hb._sseDisconnected = false;
    _hb._sseDisconnectedSince = 0;
    _hb._blockMiningOverlay = false;
    if (_hb._miningDelayTimeout) { clearTimeout(_hb._miningDelayTimeout); _hb._miningDelayTimeout = null; }
    if (_hb._miningTimeout) { clearTimeout(_hb._miningTimeout); _hb._miningTimeout = null; }
    var miningEl = document.getElementById('heartbeat-mining-overlay');
    if (miningEl) miningEl.classList.add('hidden');

    if (window.heartbeatFlash) window.heartbeatFlash();
    if (window.heartbeatPulse) window.heartbeatPulse();
    if (window.heartbeatPlaySound) window.heartbeatPlaySound();

    // Update rhythm strip with the new block
    if (window.renderRhythmStrip && window.getHeartbeatRecentBlocks) {
        window.renderRhythmStrip('rhythm-strip-canvas', window.getHeartbeatRecentBlocks());
    }

    // Resume tx flow — new txs will now flush to the new post-block flatline
    _hb._blockProcessing = false;
}

// Own node SSE feed (primary)
export function connectOwnFeed() {
    var _hb = getState();
    if (!_hb) return;
    // Close any existing SSE connection to prevent leaking EventSources
    if (_hb._sse) {
        try { _hb._sse.close(); } catch(e) {}
        _hb._sse = null;
    }
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

                // Only place history on an empty flatline (fresh start or SSE error
                // reconnect). If the flatline already has live bricks, skip —
                // we don't want to overwrite the real live layout with random
                // history placement.
                var curSeg = _hb.timeline[_hb.timeline.length - 1];
                if (curSeg && curSeg.type === 'flatline' && curSeg.x_end === null
                    && curSeg.blips && curSeg.blips.length > 0) {
                    console.log('[heartbeat] SSE history: skipping — flatline already has', curSeg.blips.length, 'bricks');
                    return;
                }

                // First history = stagger animation, reconnects = instant
                var isReconnect = _hb._hasReceivedHistory || false;
                placeHistoryTxs(txs, lastBlockTs, isReconnect);
                _hb._hasReceivedHistory = true;
            } catch (err) { console.log('[heartbeat] SSE history error:', err); }
        });
        es.addEventListener('tx', function(e) {
            if (!_hb) return;
            try {
                var tx = JSON.parse(e.data);
                _hb._hasTxStream = true;
                _hb._sseTxCount++;
                // When hidden: buffer txs. virtualX is advanced in bulk
                // on visibility return (browsers throttle SSE events in
                // background tabs, making per-event dt unreliable).
                if (document.hidden) {
                    if (!_hb._hiddenTxBuffer) _hb._hiddenTxBuffer = [];
                    if (_hb._hiddenTxBuffer.length < 3000) _hb._hiddenTxBuffer.push(tx);
                    return;
                }
                if (_hb._txBatchQueue.length > 500) _hb._txBatchQueue.length = 500;
                _hb._txBatchQueue.push(tx);
                // Track fee rates for rolling median (computed on flush, not per-tx)
                if (tx.fee_rate) {
                    if (!_hb._recentFeeRates) _hb._recentFeeRates = [];
                    _hb._recentFeeRates.push(tx.fee_rate);
                    if (_hb._recentFeeRates.length > 200) _hb._recentFeeRates.shift();
                }
                var now = Date.now();
                if (now - _hb._txThrottleTime > 200) {
                    _hb._txThrottleTime = now;
                    // Recompute median on flush cadence instead of every tx
                    if (_hb._recentFeeRates && _hb._recentFeeRates.length >= 20) {
                        var sorted = _hb._recentFeeRates.slice().sort(function(a, b) { return a - b; });
                        _hb._wsMedianFee = Math.max(1, sorted[Math.floor(sorted.length / 2)]);
                    }
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
                console.log('SSE block:', block.height, block.hash);

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
        es.addEventListener('block_mining', function(e) {
            if (!_hb) return;
            // Node is processing a new block. Delay showing the overlay by
            // 90 seconds so it only appears for longer-than-usual processing.
            // Most blocks process in under 2 minutes, so the overlay only
            // shows for the tail end instead of the full duration.
            if (_hb._miningDelayTimeout) clearTimeout(_hb._miningDelayTimeout);
            if (_hb._miningTimeout) clearTimeout(_hb._miningTimeout);
            _hb._miningDelayTimeout = setTimeout(function() {
                if (!_hb) return;
                _hb._blockMiningOverlay = true;
                var miningEl = document.getElementById('heartbeat-mining-overlay');
                if (miningEl) miningEl.classList.remove('hidden');
            }, 90000); // 90 second delay before showing
            // Auto-dismiss after 120s from mining event (30s after overlay appears)
            _hb._miningTimeout = setTimeout(function() {
                if (_hb && _hb._blockMiningOverlay) {
                    _hb._blockMiningOverlay = false;
                    var el = document.getElementById('heartbeat-mining-overlay');
                    if (el) el.classList.add('hidden');
                }
            }, 120000);
            console.log('[heartbeat] block mining detected');
        });
        es.addEventListener('lag', function(e) {
            console.log('SSE lag:', e.data);
        });
        es.onopen = function() {
            console.log('[heartbeat] SSE connected');
            _hb.wsConnected = true;
            _hb._sseDisconnected = false;
            _hb._sseDisconnectedSince = 0;
            _hb._sseRetries = 0;
            // Clear mining overlay and delay on reconnect
            _hb._blockMiningOverlay = false;
            if (_hb._miningDelayTimeout) { clearTimeout(_hb._miningDelayTimeout); _hb._miningDelayTimeout = null; }
            if (_hb._miningTimeout) { clearTimeout(_hb._miningTimeout); _hb._miningTimeout = null; }
            var mel = document.getElementById('heartbeat-mining-overlay');
            if (mel) mel.classList.add('hidden');
        };
        es.onerror = function(err) {
            if (!_hb) return;
            _hb.wsConnected = false;
            if (!_hb._sseDisconnected) {
                _hb._sseDisconnected = true;
                _hb._sseDisconnectedSince = Date.now() / 1000;
            }
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

// Mempool.space WebSocket fallback
export function connectMempoolFallback() {
    var _hb = getState();
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
            _hb._sseDisconnected = false; // WS fallback provides data, clear overlay
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

// Flush queued individual transactions into visual bricks
export function flushTxBatch() {
    var _hb = getState();
    if (!_hb || !_hb._txBatchQueue || _hb._txBatchQueue.length === 0) return;
    // During block processing, keep txs queued — they'll flush to the new flatline
    if (_hb._blockProcessing) return;
    var liveSeg = _hb.timeline[_hb.timeline.length - 1];
    if (!liveSeg || liveSeg.type !== 'flatline') return;

    // Advance virtualX here too — RAF may be throttled (e.g. OS workspace
    // switch) but SSE events keep flowing and triggering flushes.
    if (liveSeg.x_end === null) {
        var now = Date.now() / 1000;
        var dt = _hb.lastFrameTime > 0 ? (now - _hb.lastFrameTime) : 0;
        if (dt > 0.05 && dt < 10) {
            _hb.virtualX += dt * FLATLINE_PX_PER_SEC;
            _hb.lastFrameTime = now;
        }
    }

    var batch = _hb._txBatchQueue;
    _hb._txBatchQueue = [];

    // Place ALL txs as individual bricks so every tx is searchable/inspectable.
    // Spread scales with visible virtual width to prevent stacking.
    // Clamp to actual flatline width so bricks don't all pile at the start
    // of a narrow flatline (e.g., right after a block).
    var visibleVirtW = (_hb.width || 800) / (_hb.zoom || 1);
    var flatlineWidth = _hb.virtualX - liveSeg.x_start;
    var spread = Math.max(10, Math.min(flatlineWidth * 0.8, visibleVirtW * 0.15, 300));
    var medianFee = _hb._wsMedianFee || 5;

    for (var i = 0; i < batch.length; i++) {
        var tx = batch[i];
        var feeRate = tx.fee && tx.vsize ? tx.fee / tx.vsize : 1;
        var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);
        var isWhale = tx.whale || false;
        var isFeeOutlier = tx.fee_outlier || false;
        var isNotable = isWhale || isFeeOutlier;

        // Notable txs get larger bricks
        var brickW = isNotable ? 8 : 4;
        var brickH = isNotable
            ? 20 + Math.random() * 8
            : 3 + feeNorm * 14 + Math.random() * 3;
        var brickX = _hb.virtualX - Math.random() * spread;
        var gridX = Math.round(brickX / 5) * 5;
        if (gridX < liveSeg.x_start) gridX = Math.round(liveSeg.x_start / 5) * 5;

        // Stack height via column map (O(1) instead of scanning all blips).
        // If the column is too tall, search backward for a shorter one.
        if (!liveSeg._colHeights) liveSeg._colHeights = {};
        var maxStack = (_hb.height || 400) * 0.35; // cap at 35% of canvas height
        var stackY = liveSeg._colHeights[gridX] || 0;
        if (stackY > maxStack) {
            // Walk backward along the flatline to find a shorter column
            var found = false;
            var maxBack = Math.max(30, Math.floor(spread / 5));
            for (var back = 1; back <= maxBack; back++) {
                var tryX = gridX - back * 5;
                if (tryX < liveSeg.x_start) break;
                var tryH = liveSeg._colHeights[tryX] || 0;
                if (tryH <= maxStack) {
                    gridX = tryX;
                    brickX = tryX;
                    stackY = tryH;
                    found = true;
                    break;
                }
            }
            // If all nearby columns are full, skip this brick entirely
            if (!found) continue;
        }

        var blipColor = isWhale ? '#ffd700' : isFeeOutlier ? '#ff4444' : feeRateColor(feeRate, medianFee);

        liveSeg.blips.push({
            x: brickX,
            gridX: gridX,
            height: brickH + stackY,
            brickH: brickH,
            brickW: brickW,
            stackY: stackY,
            color: blipColor,
            opacity: isNotable ? 1.0 : 0.75 + feeNorm * 0.2,
            txCount: 1,
            txid: tx.txid || null,
            feeRate: Math.round(feeRate * 10) / 10,
            vsize: tx.vsize || 0,
            value: tx.value || 0,
            timestamp: Date.now() / 1000,
            fadeStart: 0,
            bobPhase: Math.random() * Math.PI * 2,
            bobSpeed: isNotable ? 0.6 : 1.2 + feeNorm * 0.8,
            lane: Math.floor(Math.random() * 5) - 2,
            feeRatio: medianFee > 0 ? feeRate / medianFee : 1,
            whale: isWhale,
            feeOutlier: isFeeOutlier,
            valueUsd: tx.value_usd || 0
        });
        liveSeg._colHeights[gridX] = stackY + brickH;

        // Push notable txs to the feed
        if (isNotable && window._notableFeed) {
            window._notableFeed(tx);
        }
    }

    // Priority culling: only when truly excessive. Viewport culling already
    // skips off-screen blips during rendering, so memory is the only concern.
    var CULL_THRESHOLD = 30000;
    var CULL_TARGET = 25000;
    if (liveSeg.blips.length > CULL_THRESHOLD) {
        // Score each active blip: lower score = evict first
        // Priority: low fee + small vsize evicted first
        var scored = [];
        for (var ci = 0; ci < liveSeg.blips.length; ci++) {
            var cb = liveSeg.blips[ci];
            if (cb.fadeStart > 0) continue; // already fading, skip
            if (cb.whale || cb.feeOutlier) continue; // never cull notable blips
            var score = (cb.feeRate || 0.1) * Math.log2((cb.vsize || 100) + 1);
            scored.push({ idx: ci, score: score });
        }
        if (scored.length > CULL_TARGET) {
            scored.sort(function(a, b) { return a.score - b.score; });
            var toEvict = scored.length - CULL_TARGET;
            console.log('[heartbeat] culling:', liveSeg.blips.length, '->', CULL_TARGET,
                '(evicting', toEvict, ', threshold=' + CULL_THRESHOLD + ')');
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
            // Clear pinned/hovered blip if it was evicted
            if (_hb._pinnedBlip && kept.indexOf(_hb._pinnedBlip) === -1) {
                _hb._pinnedBlip = null;
            }
            if (_hb.hoveredBlip && kept.indexOf(_hb.hoveredBlip) === -1) {
                _hb.hoveredBlip = null;
            }
        }
    }
}

// Fallback: add aggregate blips when individual tx stream isn't available
export function addMempoolTxs(newTxCount, medianFeeRate, topFeeRate) {
    var _hb = getState();
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

// ── Whale Watch Feed ──────────────────────────────────
var MAX_WHALE_ENTRIES = 100;

window._notableFeed = function(tx) {
    var panel = document.getElementById('whale-feed-panel');
    var list = document.getElementById('whale-feed-list');
    if (!panel || !list) return;

    // Show panel on first notable tx
    panel.classList.remove('hidden');

    // Remove placeholder
    var placeholder = list.querySelector('.italic');
    if (placeholder) placeholder.remove();

    var isWhale = tx.whale || false;
    var isFeeOutlier = tx.fee_outlier || false;
    var btcVal = (tx.value || 0) / 100000000;
    var feeRate = tx.fee && tx.vsize ? (tx.fee / tx.vsize).toFixed(1) : '?';
    var feeBtc = (tx.fee || 0) / 100000000;
    var time = new Date().toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    var txidShort = tx.txid ? tx.txid.substring(0, 12) + '...' : '?';

    // Build label based on type
    var labelHtml;
    if (isWhale) {
        var usdVal = Math.round(tx.value_usd || 0);
        labelHtml = '<span class="text-[#ffd700] font-bold shrink-0">$' + usdVal.toLocaleString() + '</span>' +
            '<span class="text-white/50 shrink-0">' + btcVal.toFixed(4) + ' BTC</span>';
    } else {
        labelHtml = '<span class="text-[#ff4444] font-bold shrink-0">FEE: ' + feeRate + ' sat/vB</span>' +
            '<span class="text-white/50 shrink-0">' + feeBtc.toFixed(6) + ' BTC fee</span>';
    }

    var row = document.createElement('div');
    row.className = 'flex items-baseline gap-3 px-4 py-2 border-b border-white/5 text-xs font-mono opacity-0';
    row.style.animation = 'fadeinone 0.5s ease forwards';
    row.dataset.type = isWhale ? 'whale' : 'fee';
    row.innerHTML = labelHtml +
        '<a href="https://mempool.space/tx/' + (tx.txid || '') + '" target="_blank" rel="noopener" ' +
            'class="text-white/30 hover:text-[#f7931a] transition-colors">' + txidShort + '</a>' +
        '<span class="text-white/20 ml-auto shrink-0">' + time + '</span>';

    // Prepend (newest first)
    list.insertBefore(row, list.firstChild);

    // Cap entries
    while (list.children.length > MAX_WHALE_ENTRIES) {
        list.removeChild(list.lastChild);
    }

    // Apply active filter to new row
    var activeFilter = window._notableFilter || 'all';
    if (activeFilter !== 'all' && row.dataset.type !== activeFilter) {
        row.style.display = 'none';
    }
};

// Filter notable feed by type
window._notableFilter = 'all';
window._filterNotable = function(filter) {
    window._notableFilter = filter;
    var list = document.getElementById('whale-feed-list');
    if (!list) return;

    // Update button styles
    var btns = ['all', 'whale', 'fee'];
    for (var bi = 0; bi < btns.length; bi++) {
        var btn = document.getElementById('whale-filter-' + btns[bi]);
        if (!btn) continue;
        if (btns[bi] === filter) {
            btn.className = 'px-2 py-0.5 rounded text-[10px] font-mono bg-white/10 text-white/60 hover:bg-white/20 transition-colors';
        } else {
            btn.className = 'px-2 py-0.5 rounded text-[10px] font-mono bg-transparent text-white/30 hover:bg-white/10 transition-colors';
        }
    }

    // Show/hide rows
    var rows = list.children;
    for (var i = 0; i < rows.length; i++) {
        var rowType = rows[i].dataset.type;
        if (!rowType) continue;
        rows[i].style.display = (filter === 'all' || rowType === filter) ? '' : 'none';
    }
};
