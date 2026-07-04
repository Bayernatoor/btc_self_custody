// heartbeat-sse.js — SSE connection, tx batching, and mempool fallback
import { getState, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC } from './heartbeat-state.js';
import { feeRateColor, fmtBtc } from './heartbeat-timeline.js';

// ── Notable-tx classification (shared by history placement, live flush, feed) ──
// Accepts both the live SSE shape (boolean flags) and the persisted notable_txs
// shape (notable_type string). For live txs notable_type is absent, so each
// check reduces to the boolean.
function notableFlags(tx) {
    var nt = tx.notable_type || null;
    return {
        whale: tx.whale || nt === 'whale',
        feeOutlier: tx.fee_outlier || nt === 'fee_outlier',
        consolidation: tx.consolidation || nt === 'consolidation',
        fanOut: tx.fan_out || nt === 'fan_out',
        largeInscription: tx.large_inscription || nt === 'large_inscription',
        roundNumber: tx.round_number || nt === 'round_number',
        opReturnMsg: tx.op_return_msg || nt === 'op_return_msg'
    };
}

function anyNotable(f) {
    return f.whale || f.feeOutlier || f.consolidation || f.fanOut
        || f.largeInscription || f.roundNumber || f.opReturnMsg;
}

// Blip color prefix + notableType label, priority order matching the backend
// (value > structural > fee > data). Returns null when not notable (caller
// falls back to feeRateColor).
function notableStyle(f) {
    if (f.whale) return { color: 'rgba(255, 215, 0, ', type: 'whale' };
    if (f.roundNumber) return { color: 'rgba(144, 238, 144, ', type: 'round_number' };
    if (f.largeInscription) return { color: 'rgba(255, 0, 200, ', type: 'large_inscription' };
    if (f.consolidation) return { color: 'rgba(168, 85, 247, ', type: 'consolidation' };
    if (f.fanOut) return { color: 'rgba(0, 210, 255, ', type: 'fan_out' };
    if (f.feeOutlier) return { color: 'rgba(255, 68, 68, ', type: 'fee_outlier' };
    if (f.opReturnMsg) return { color: 'rgba(255, 165, 0, ', type: 'op_return_msg' };
    return null;
}

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

    // Widen the flatline to hold the mempool by VOLUME, not just elapsed time.
    // Elapsed time alone (elapsed * 5px/s) is far too narrow for a full mempool:
    // e.g. 20k txs across 143s of width (717px) stacks ~40 high and the column
    // cap drops most of them. Size it to ~10 bricks per 5px column (same density
    // as the live flush + the post-block harvest) so the mempool lays out
    // long-and-low and (nearly) all of it fits. Take the max with the time-based
    // width so a small mempool still reflects real elapsed time.
    var nowSec = Date.now() / 1000;
    var elapsedSinceBlock = effectiveBlockTs > 0 ? nowSec - effectiveBlockTs : 0;
    var timeWidth = elapsedSinceBlock > 0 ? elapsedSinceBlock * FLATLINE_PX_PER_SEC : 0;
    var volumeWidth = Math.ceil(txs.length / 10) * 5;
    var neededX = liveSeg.x_start + Math.max(timeWidth, volumeWidth);
    if (neededX > _hb.virtualX) {
        _hb.virtualX = neededX;
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

    // Place ALL received history txs (the server caps the feed — see
    // HEARTBEAT_HISTORY_LIMIT in api.rs). Density is still governed by the
    // per-column stack cap (histMaxStack, 35% of canvas height) below — the exact
    // limiter live flushTxBatch uses — so a wide flatline fills naturally and a
    // narrow one won't over-stack. Placement is one synchronous pass; at ~10-20k
    // that's a brief sub-frame cost on load. If the server limit is raised much
    // further and load starts to hitch, chunk this across RAF frames like the
    // live queue (bounded to 500/frame).
    var maxBricks = txs.length;

    console.log('[heartbeat] placeHistoryTxs: flatlineSpan=' + Math.round(flatlineSpan) +
        'px, virtualX=' + Math.round(_hb.virtualX) +
        ', segStart=' + Math.round(liveSeg.x_start) +
        ', effectiveBlockTs=' + effectiveBlockTs +
        ', elapsed=' + Math.round(elapsedSinceBlock) + 's');

    // Stagger history bricks so they animate in from left to right over ~1.5s.
    // On reconnect (instant=true), place bricks immediately with no animation.
    var dropNow = Date.now() / 1000;
    var DROP_DURATION = instant ? 0 : 1.5;

    // Collect notable txs from history for the feed panel
    var notableTxs = [];

    for (var i = 0; i < txs.length && placed < maxBricks; i++) {
        var tx = txs[i];
        if (!tx.fee || !tx.vsize) continue;

        // Notable classification (shared helper; handles history + live shapes)
        var flags = notableFlags(tx);
        var isWhale = flags.whale, isFeeOutlier = flags.feeOutlier,
            isConsolidation = flags.consolidation, isFanOut = flags.fanOut,
            isLargeInscription = flags.largeInscription,
            isRoundNumber = flags.roundNumber, isOpReturnMsg = flags.opReturnMsg;
        var isNotable = anyNotable(flags);

        // Spread randomly across the flatline, leaving last 20px for live txs.
        var usableSpan = Math.max(10, flatlineSpan - 20);
        var txVX = liveSeg.x_start + Math.random() * usableSpan;

        var feeRate = tx.fee / tx.vsize;
        var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);

        var brickW = isNotable ? 6 : 4;
        var brickH = isNotable
            ? 22 + feeNorm * 12 + Math.random() * 4
            : 3 + feeNorm * 14 + Math.random() * 3;
        var gridX = Math.round(txVX / 5) * 5;
        var stackY = stackMap[gridX] || 0;
        var histMaxStack = (_hb.height || 400) * 0.35;
        if (stackY > histMaxStack && !isNotable) continue;
        stackMap[gridX] = stackY + brickH;

        var style = notableStyle(flags);
        var blipColor = style ? style.color : feeRateColor(feeRate, medianFee);
        var notableType = style ? style.type : null;

        // Stagger: left-most bricks appear first, sweeping right
        var xFrac = flatlineSpan > 0 ? (txVX - liveSeg.x_start) / flatlineSpan : 0;
        var dropDelay = instant ? 0 : xFrac * DROP_DURATION;

        liveSeg.blips.push({
            x: txVX, gridX: gridX,
            height: brickH + stackY, brickH: brickH, brickW: brickW, stackY: stackY,
            color: blipColor, opacity: isNotable ? 1.0 : 0.75 + feeNorm * 0.2,
            txCount: 1, txid: tx.txid || null,
            feeRate: Math.round(feeRate * 10) / 10,
            vsize: tx.vsize || 0, value: tx.value || 0,
            timestamp: dropNow + dropDelay, isDrop: !instant, fadeStart: 0,
            bobPhase: Math.random() * Math.PI * 2,
            bobSpeed: isNotable ? 0.6 : 1.2 + feeNorm * 0.8,
            lane: Math.floor(Math.random() * 5) - 2,
            feeRatio: medianFee > 0 ? feeRate / medianFee : 1,
            whale: isWhale,
            feeOutlier: isFeeOutlier,
            notableType: notableType,
            valueUsd: tx.value_usd || 0,
            opReturnText: tx.op_return_text || '',
            inputCount: tx.input_count || 0,
            outputCount: tx.output_count || 0
        });
        placed++;

        if (isNotable) {
            notableTxs.push({
                txid: tx.txid, fee: tx.fee, vsize: tx.vsize, value: tx.value,
                whale: isWhale, fee_outlier: isFeeOutlier,
                consolidation: isConsolidation, fan_out: isFanOut,
                large_inscription: isLargeInscription,
                round_number: isRoundNumber, op_return_msg: isOpReturnMsg,
                op_return_text: tx.op_return_text || '',
                value_usd: tx.value_usd || 0,
                input_count: tx.input_count || 0,
                output_count: tx.output_count || 0
            });
        }
    }

    // Populate feed panel with notable txs from history
    if (notableTxs.length > 0 && window._notableFeed) {
        for (var ni = notableTxs.length - 1; ni >= 0; ni--) {
            window._notableFeed(notableTxs[ni]);
        }
    }
    // Seed the segment's column height map so live blips stack correctly
    liveSeg._colHeights = stackMap;
    console.log('[heartbeat] placed', placed, 'history bricks on timeline');

    // Auto-fit zoom on first load so the whole mempool + recent spikes frame
    // themselves, instead of opening at 1.9x showing only a tiny head slice.
    // Only on the first history (not reconnect, where the user may have zoomed).
    // autoFollow stays on, so head-following continues at the fitted zoom.
    if (!instant && _hb.width) {
        var span = _hb.virtualX - liveSeg.x_start;
        if (span > 10) {
            // Fit the flatline into ~80% of the viewport (head sits at
            // HEAD_POSITION_FRAC, leaving room on the right). Never zoom IN past
            // the 1.9x default for a small mempool — only out to fit a big one.
            var fit = (_hb.width * 0.8) / span;
            _hb.zoom = Math.max(_hb.minZoom, Math.min(fit, 1.9));
            if (_hb.autoFollow) {
                _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
            }
            console.log('[heartbeat] auto-fit zoom to ' + _hb.zoom.toFixed(3) +
                'x (mempool span ' + Math.round(span) + 'px)');
        }
    }
}

// Process a live block from SSE (shared by live handler + queue replay)
export function processLiveBlock(block) {
    var _hb = getState();
    if (!_hb) return;
    // Start each block from a clean backlog-spread state so a previous block's
    // spread (still within its 2.5s ramp) can't bleed into this block's flatline.
    _hb._backlogSpreadPx = 0;
    _hb._backlogSpreadStart = 0;

    // Collapse the dead processing gap: the backend suppresses txs while
    // building block data (~2-5s, longer on a node stall), so virtualX has
    // advanced past the last brick with nothing placed. Find the rightmost blip
    // and snap virtualX to just past it. This is exact — no time-based
    // estimation that can overshoot. `collapsedPx` = the width of that no-brick
    // window; we re-add it after the spike below so the backlog can spread.
    var collapsedPx = 0;
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
            collapsedPx = _hb.virtualX - closeAt;
            console.log('[heartbeat] gap collapse: ' + Math.round(_hb.virtualX) + ' -> ' + Math.round(closeAt) +
                ' (' + Math.round(collapsedPx) + 'px removed, block=' + block.height + ')');
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
    // Mark this as a genuine live block so pushHeartbeatBlocks runs the fee-rate
    // harvest (confirm top-fee bricks into the spike, carry the rest forward).
    _hb._harvestLive = true;
    window.pushHeartbeatBlocks(blockJson, false);

    // Re-add the collapsed gap AFTER the spike. The gap we just removed is the
    // no-brick window — the node suppressed tx broadcast while validating (and
    // for longer on a stall). The backlog of txs that piled during it arrives as
    // a burst right after the spike; giving the fresh post-block flatline that
    // same width, and telling flushTxBatch to spread the burst across it (see
    // _backlogSpreadPx there), lets the backlog fan out instead of walling in one
    // column at the head. Only for real windows (>40px ≈ 8s) — small validation
    // gaps are already covered by the flush's normal spread. (Camera easing in
    // drawFrame turns the resulting virtualX jump into a smooth slide.)
    if (collapsedPx > 40) {
        var newSeg = _hb.timeline[_hb.timeline.length - 1];
        if (newSeg && newSeg.type === 'flatline' && newSeg.x_end === null) {
            _hb.virtualX += collapsedPx;
            _hb._backlogSpreadPx = collapsedPx;
            _hb._backlogSpreadStart = Date.now() / 1000;
            console.log('[heartbeat] post-block backlog: re-added ' + Math.round(collapsedPx) +
                'px for the burst to spread across');
        }
    }

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
    // New txs flush to the new post-block flatline on the next RAF frame.
    // (No _blockProcessing gate needed: this function is fully synchronous, so it
    // always completes — creating the new flatline — before any drawFrame runs.)
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
    // Cancel any pending reconnect timer so retries can't stack.
    if (_hb._sseRetryTimer) { clearTimeout(_hb._sseRetryTimer); _hb._sseRetryTimer = null; }
    console.log('[heartbeat] connecting to SSE /api/stats/heartbeat');
    _hb._txThrottleTime = 0;
    _hb._txBatchQueue = [];
    _hb._sseTxCount = 0;
    try {
        var es = new EventSource('/api/stats/heartbeat');
        es.addEventListener('notable_history', function(e) {
            if (!_hb) return;
            try {
                var notables = JSON.parse(e.data);
                if (!Array.isArray(notables) || notables.length === 0) return;
                console.log('[heartbeat] SSE notable_history:', notables.length, 'txs');
                // Server sends newest-first. Append preserves that order (newest at top).
                for (var ni = 0; ni < notables.length; ni++) {
                    if (window._notableFeed) {
                        window._notableFeed(notables[ni], { append: true });
                    }
                }
            } catch (err) {
                console.log('[heartbeat] notable_history parse error:', err);
            }
        });
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
                _hb._lastSseEventTs = Date.now() / 1000; // liveness (before hidden buffering)
                _hb._sseTxCount++;
                // When hidden: buffer txs. virtualX is advanced in bulk
                // on visibility return (browsers throttle SSE events in
                // background tabs, making per-event dt unreliable).
                if (document.hidden) {
                    if (!_hb._hiddenTxBuffer) _hb._hiddenTxBuffer = [];
                    if (_hb._hiddenTxBuffer.length < 3000) _hb._hiddenTxBuffer.push(tx);
                    return;
                }
                // Bound the queue during a backlog; keep the NEWEST (setting
                // .length truncates the tail, which dropped the freshest arrivals).
                if (_hb._txBatchQueue.length >= 500) {
                    _hb._txBatchQueue = _hb._txBatchQueue.slice(-499);
                }
                _hb._txBatchQueue.push(tx);
                // Track fee rates for rolling median (computed on flush, not per-tx)
                if (tx.fee_rate) {
                    if (!_hb._recentFeeRates) _hb._recentFeeRates = [];
                    _hb._recentFeeRates.push(tx.fee_rate);
                    if (_hb._recentFeeRates.length > 200) _hb._recentFeeRates.shift();
                }
                // Bricks are laid down by the RAF loop (drawFrame → flushTxBatch),
                // which spreads the work per frame instead of a 200ms off-RAF burst
                // that hitched the scroll ("pause + pull"). The handler just queues.
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
                _hb._lastSseEventTs = Date.now() / 1000; // liveness
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
        // TODO: block_mining overlay disabled — triggers at wrong times, needs review.
        // See tasks/todo.md "Heartbeat > Review block_mining overlay trigger logic".
        // es.addEventListener('block_mining', function(e) {
        //     if (!_hb) return;
        //     // Node is processing a new block. Delay showing the overlay by
        //     // 90 seconds so it only appears for longer-than-usual processing.
        //     if (_hb._miningDelayTimeout) clearTimeout(_hb._miningDelayTimeout);
        //     if (_hb._miningTimeout) clearTimeout(_hb._miningTimeout);
        //     _hb._miningDelayTimeout = setTimeout(function() {
        //         if (!_hb) return;
        //         _hb._blockMiningOverlay = true;
        //         var miningEl = document.getElementById('heartbeat-mining-overlay');
        //         if (miningEl) miningEl.classList.remove('hidden');
        //     }, 90000);
        //     _hb._miningTimeout = setTimeout(function() {
        //         if (_hb && _hb._blockMiningOverlay) {
        //             _hb._blockMiningOverlay = false;
        //             var el = document.getElementById('heartbeat-mining-overlay');
        //             if (el) el.classList.add('hidden');
        //         }
        //     }, 120000);
        //     console.log('[heartbeat] block mining detected');
        // });
        es.addEventListener('lag', function(e) {
            // A lag event proves the byte stream is live (client just fell behind
            // the broadcast channel), so refresh the stall clock — otherwise a
            // perpetually-lagging client would falsely read as 'stale'.
            if (_hb) _hb._lastSseEventTs = Date.now() / 1000;
            console.log('SSE lag:', e.data);
        });
        es.onopen = function() {
            console.log('[heartbeat] SSE connected');
            _hb.wsConnected = true;
            _hb._sseDisconnected = false;
            _hb._sseDisconnectedSince = 0;
            _hb._lastSseEventTs = Date.now() / 1000; // reset stall clock on connect
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
            // Reconnect the primary feed INDEFINITELY with capped backoff
            // (2s → 15s), so a deploy or node blip reconnects within one interval
            // of the server returning — no giving up, no fallback stranding.
            var delay = Math.min(2000 * Math.pow(1.5, Math.min(_hb._sseRetries - 1, 8)), 15000);
            console.log('[heartbeat] SSE error, retry', _hb._sseRetries, 'in', Math.round(delay / 1000) + 's');
            // Track the handle so destroyHeartbeat can cancel it, and re-check
            // getState() (not the captured _hb, which stays truthy after destroy)
            // so a stale retry can't fire connectOwnFeed after teardown/reset.
            _hb._sseRetryTimer = setTimeout(function() {
                if (getState()) connectOwnFeed();
            }, delay);
        };
        _hb._sse = es;
    } catch (err) {
        // EventSource construction failed — retry shortly.
        _hb._sseRetryTimer = setTimeout(function() { if (getState()) connectOwnFeed(); }, 3000);
    }
}

// Widen the live flatline so its width reflects the real time since `blockTs`
// (the newest block's age). Used ONLY by the tab-return catch-up path
// (catchUpBlocks), where real wall-clock time genuinely elapsed while the tab
// was hidden, so the resumed live tx flow needs a flatline that wide to spread
// across instead of piling at the head. The live path (processLiveBlock) does
// NOT use this — it re-adds the exact collapsed gap instead (block age is ~0 for
// a fresh block, so it would never widen there). Only ever moves virtualX
// forward; clamped for miner-clock skew. (Camera easing in drawFrame turns the
// resulting virtualX jump into a smooth slide.)
export function fastForwardLiveFlatline(blockTs) {
    var _hb = getState();
    if (!_hb || !blockTs) return;
    var liveSeg = _hb.timeline[_hb.timeline.length - 1];
    if (!liveSeg || liveSeg.type !== 'flatline' || liveSeg.x_end !== null) return;
    var age = Math.max(0, Math.min(Date.now() / 1000 - blockTs, 1800));
    var wanted = liveSeg.x_start + age * FLATLINE_PX_PER_SEC;
    if (wanted > _hb.virtualX) _hb.virtualX = wanted;
}

// Flush queued individual transactions into visual bricks
export function flushTxBatch() {
    var _hb = getState();
    if (!_hb || !_hb._txBatchQueue || _hb._txBatchQueue.length === 0) return;
    var liveSeg = _hb.timeline[_hb.timeline.length - 1];
    if (!liveSeg || liveSeg.type !== 'flatline') return;

    // virtualX is advanced ONLY by the RAF loop (drawFrame) now — this used to
    // advance it too, a second out-of-phase clock that nudged virtualX between
    // frames and drifted it ahead. Single clock = smoother scroll.

    // Rolling median fee for colour thresholds (recomputed as batches flush).
    if (_hb._recentFeeRates && _hb._recentFeeRates.length >= 20) {
        var sortedFees = _hb._recentFeeRates.slice().sort(function(a, b) { return a - b; });
        _hb._wsMedianFee = Math.max(1, sortedFees[Math.floor(sortedFees.length / 2)]);
    }

    var batch = _hb._txBatchQueue;
    _hb._txBatchQueue = [];

    // Place ALL txs as individual bricks so every tx is searchable/inspectable.
    // Spread them behind the head so a burst doesn't wall up in one column.
    // The head sits at HEAD_POSITION_FRAC (0.85) of the viewport, so most of the
    // visible width is to the LEFT of the head and free to fill — hence the 0.7
    // viewport cap (the old 0.15 crammed everything into ~16 columns at zoom).
    // Spread also scales with burst size: calm flushes stay tight (live feel);
    // truly heavy sustained volume still stacks past the cap (intentional busy signal).
    var visibleVirtW = (_hb.width || 800) / (_hb.zoom || 1);
    var flatlineWidth = _hb.virtualX - liveSeg.x_start;
    var burstFactor = Math.min(batch.length / 25, 4); // ramps 0→4 by ~100 txs/flush
    var spreadCap = 150 + burstFactor * 250;          // ~150px calm → ~1150px heavy
    var spread = Math.max(10, Math.min(flatlineWidth * 0.8, visibleVirtW * 0.7, spreadCap));

    // Post-block backlog: right after a block, processLiveBlock re-adds the
    // collapsed no-brick gap to the flatline and sets _backlogSpreadPx. Widen the
    // spread to that full width so the burst fans across the re-added region
    // instead of walling at the head (the normal 0.8×flatlineWidth cap is tiny on
    // a fresh flatline). Ramp back to normal over BACKLOG_SPREAD_SECS as the
    // backlog drains and genuine live txs take over at the head.
    if (_hb._backlogSpreadPx > 0) {
        var BACKLOG_SPREAD_SECS = 2.5;
        var bsElapsed = (Date.now() / 1000) - (_hb._backlogSpreadStart || 0);
        if (bsElapsed < BACKLOG_SPREAD_SECS) {
            var bsWide = Math.min(_hb._backlogSpreadPx * (1 - bsElapsed / BACKLOG_SPREAD_SECS), flatlineWidth);
            if (bsWide > spread) spread = bsWide;
        } else {
            _hb._backlogSpreadPx = 0;
        }
    }
    var medianFee = _hb._wsMedianFee || 5;

    for (var i = 0; i < batch.length; i++) {
        var tx = batch[i];
        var feeRate = tx.fee && tx.vsize ? tx.fee / tx.vsize : 1;
        var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);
        var flags = notableFlags(tx);
        var isWhale = flags.whale, isFeeOutlier = flags.feeOutlier;
        var isNotable = anyNotable(flags);

        // Notable txs: slightly wider and taller, but not so tall they fill columns fast.
        // Reduced from 35-60px (which caused tx drops) to 22-38px.
        var brickW = isNotable ? 6 : 4;
        var brickH = isNotable
            ? 22 + feeNorm * 12 + Math.random() * 4
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
            // Normal bricks: skip when all nearby columns full.
            // Notable bricks: never skip — force-place at shortest nearby column.
            if (!found) {
                if (isNotable) {
                    // Find shortest column in a wider search to force placement
                    var shortest = stackY;
                    var shortestX = gridX;
                    for (var bk = 1; bk <= 60; bk++) {
                        var tx2 = gridX - bk * 5;
                        if (tx2 < liveSeg.x_start) break;
                        var th2 = liveSeg._colHeights[tx2] || 0;
                        if (th2 < shortest) {
                            shortest = th2;
                            shortestX = tx2;
                        }
                    }
                    gridX = shortestX;
                    brickX = shortestX;
                    stackY = shortest;
                } else {
                    continue;
                }
            }
        }

        var style = notableStyle(flags);
        var blipColor = style ? style.color : feeRateColor(feeRate, medianFee);
        var notableType = style ? style.type : null;

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
            notableType: notableType,
            valueUsd: tx.value_usd || 0,
            opReturnText: tx.op_return_text || '',
            inputCount: tx.input_count || 0,
            outputCount: tx.output_count || 0
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
            if (cb.notableType) continue; // never cull notable blips
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

// ── Whale Watch Feed ──────────────────────────────────
var MAX_WHALE_ENTRIES = 100;

// Track seen txids to prevent duplicates when history replays + live arrives
window._seenNotableTxids = window._seenNotableTxids || new Set();

// Build the feed row HTML and styling from a tx object.
// Accepts both live SSE format (whale/fee_outlier/... booleans) and
// persistent notable_txs format (notable_type string).
function _buildNotableRow(tx) {
    // Accepts both live (boolean flags) and history (notable_type) shapes.
    var flags = notableFlags(tx);
    var isWhale = flags.whale, isFeeOutlier = flags.feeOutlier,
        isConsolidation = flags.consolidation, isFanOut = flags.fanOut,
        isLargeInscription = flags.largeInscription,
        isRoundNumber = flags.roundNumber, isOpReturnMsg = flags.opReturnMsg;

    var btcVal = (tx.value || 0) / 100000000;
    var feeRate = tx.fee && tx.vsize ? (tx.fee / tx.vsize).toFixed(1) : '?';
    var feeBtc = (tx.fee || 0) / 100000000;
    var timestamp = tx.first_seen || tx.timestamp || Math.floor(Date.now() / 1000);
    var time = new Date(timestamp * 1000).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    var txidShort = tx.txid ? tx.txid.substring(0, 12) + '...' : '?';
    var usdVal = Math.round(tx.value_usd || 0);
    var usdStr = usdVal > 0 ? '$' + usdVal.toLocaleString() : fmtBtc(btcVal) + ' BTC';
    var isConfirmed = !!tx.confirmed_height;
    var statusIcon = isConfirmed ? '<span class="text-white/40 text-[10px] shrink-0" title="Confirmed">\u2713</span>' : '';

    var labelHtml, feedType;
    if (isWhale) {
        feedType = 'whale';
        labelHtml = '<span class="text-[#ffd700] font-bold shrink-0">WHALE</span>' +
            '<span class="text-[#ffd700]/80 shrink-0">' + usdStr + '</span>';
    } else if (isRoundNumber) {
        feedType = 'round_number';
        // Show the round output value (the part that triggered detection), not total
        var roundBtc = (tx.max_output_value || tx.value || 0) / 100000000;
        labelHtml = '<span class="text-[#90ee90] font-bold shrink-0">ROUND #</span>' +
            '<span class="text-white/60 shrink-0">' + fmtBtc(roundBtc) + ' BTC out</span>';
    } else if (isLargeInscription) {
        feedType = 'inscription';
        labelHtml = '<span class="text-[#ff00c8] font-bold shrink-0">INSCRIPTION</span>' +
            '<span class="text-white/50 shrink-0">' + fmtBtc(btcVal) + ' BTC</span>';
    } else if (isConsolidation) {
        feedType = 'consolidation';
        var ioC = (tx.input_count || '?') + '->' + (tx.output_count || '?');
        labelHtml = '<span class="text-[#a855f7] font-bold shrink-0">CONSOL</span>' +
            '<span class="text-white/50 shrink-0">' + ioC + '</span>';
    } else if (isFanOut) {
        feedType = 'fan_out';
        var ioF = (tx.input_count || '?') + '->' + (tx.output_count || '?');
        labelHtml = '<span class="text-[#00d2ff] font-bold shrink-0">FAN-OUT</span>' +
            '<span class="text-white/50 shrink-0">' + ioF + '</span>';
    } else if (isFeeOutlier) {
        feedType = 'fee';
        labelHtml = '<span class="text-[#ff4444] font-bold shrink-0">FEE</span>' +
            '<span class="text-white/50 shrink-0">' + feeRate + ' sat/vB</span>';
    } else if (isOpReturnMsg) {
        feedType = 'op_return';
        var msgText = (tx.op_return_text || '').substring(0, 30);
        labelHtml = '<span class="text-[#ffa500] font-bold shrink-0">MSG</span>' +
            '<span class="text-white/60 shrink-0 truncate max-w-[200px]" title="' + (tx.op_return_text || '').replace(/"/g, '&quot;') + '">' + msgText + '</span>';
    } else {
        feedType = 'other';
        labelHtml = '<span class="text-white/50 font-bold shrink-0">NOTABLE</span>';
    }

    return {
        labelHtml: labelHtml,
        feedType: feedType,
        txidShort: txidShort,
        time: time,
        statusIcon: statusIcon
    };
}

window._notableFeed = function(tx, opts) {
    var panel = document.getElementById('whale-feed-panel');
    var list = document.getElementById('whale-feed-list');
    if (!panel || !list) return;

    var txid = tx.txid || '';
    if (!txid) return;

    // Dedup: skip if we've already rendered this txid
    if (window._seenNotableTxids.has(txid)) return;
    window._seenNotableTxids.add(txid);

    // Show panel on first notable tx
    panel.classList.remove('hidden');

    // Remove placeholder (use explicit attribute, not fragile .italic class)
    var placeholder = list.querySelector('[data-placeholder]');
    if (placeholder) placeholder.remove();

    var built = _buildNotableRow(tx);

    var row = document.createElement('div');
    var appendMode = opts && opts.append;
    row.className = 'flex items-baseline gap-3 px-4 py-2 border-b border-white/5 text-xs font-mono'
        + (appendMode ? '' : ' opacity-0');
    if (!appendMode) row.style.animation = 'fadeinone 0.5s ease forwards';
    row.dataset.type = built.feedType;
    row.dataset.txid = txid;
    row.innerHTML = built.labelHtml +
        built.statusIcon +
        '<span class="text-white/30 hover:text-[#f7931a] transition-colors cursor-pointer" data-txid-link="' + txid + '">' + built.txidShort + '</span>' +
        '<span class="text-white/20 ml-auto shrink-0">' + built.time + '</span>';

    // Click txid to open tx detail modal
    var txSpan = row.querySelector('[data-txid-link]');
    if (txSpan) {
        txSpan.addEventListener('click', function() {
            if (window.showTxDetail) {
                window.showTxDetail(txid, {
                    fee: tx.fee || null,
                    vsize: tx.vsize || null,
                    feeRate: tx.fee && tx.vsize ? tx.fee / tx.vsize : null,
                    value: tx.value || null
                });
            }
        });
    }

    // Append mode for history (preserve chronological order),
    // prepend mode for live (newest first)
    if (appendMode) {
        list.appendChild(row);
    } else {
        list.insertBefore(row, list.firstChild);
    }

    // Cap entries
    while (list.children.length > MAX_WHALE_ENTRIES) {
        var removed = list.lastChild;
        if (removed && removed.dataset && removed.dataset.txid) {
            window._seenNotableTxids.delete(removed.dataset.txid);
        }
        list.removeChild(removed);
    }

    // Apply active filter to new row
    var activeFilter = window._notableFilter || 'all';
    if (activeFilter !== 'all' && row.dataset.type !== activeFilter) {
        row.style.display = 'none';
    }
};

// Filter notable feed by type
window._notableFilter = 'all';
// Color map per filter type (matches blip colors)
var _filterColors = {
    'all': '255,255,255',
    'whale': '255,215,0',
    'round_number': '144,238,144',
    'inscription': '255,0,200',
    'consolidation': '168,85,247',
    'fan_out': '0,210,255',
    'fee': '255,68,68',
    'op_return': '255,165,0'
};

window._filterNotable = function(filter) {
    window._notableFilter = filter;
    var list = document.getElementById('whale-feed-list');
    if (!list) return;

    // Update button styles: active uses type-colored background at 20% opacity
    var btns = document.querySelectorAll('[id^="whale-filter-"]');
    for (var bi = 0; bi < btns.length; bi++) {
        var btnEl = btns[bi];
        var btnFilter = btnEl.id.replace('whale-filter-', '');
        var rgb = _filterColors[btnFilter] || '255,255,255';
        if (btnFilter === filter) {
            btnEl.style.background = 'rgba(' + rgb + ',0.2)';
            btnEl.style.borderColor = 'rgba(' + rgb + ',0.5)';
            btnEl.style.opacity = '1';
        } else {
            btnEl.style.background = 'transparent';
            btnEl.style.borderColor = 'rgba(255,255,255,0.08)';
            btnEl.style.opacity = '0.7';
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
