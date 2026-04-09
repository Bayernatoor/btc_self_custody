// Block Heartbeat v3 — ES Module entry point
// Imports all sub-modules and wires up the window.* public API
// that the Leptos component calls via wasm_bindgen.

import { getState, setState, COLORS, BG_COLOR, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, RETARGET_PERIOD } from './heartbeat-state.js';
import { drawGrid, createBlockSegment, createFlatlineSegment, computeColor, historyFlatlineWidth, feeRateColor } from './heartbeat-timeline.js';
import { setupInputHandlers } from './heartbeat-interaction.js';
import { stopMomentum } from './heartbeat-interaction.js';
import { connectOwnFeed, placeHistoryTxs } from './heartbeat-sse.js';
import { drawFrame } from './heartbeat-render.js';
import {
    getHeartbeatVitals, renderRhythmStrip, getHeartbeatRecentBlocks,
    heartbeatCenter, heartbeatSearchTx,
    heartbeatPulse, heartbeatFlash,
    getOrganismStatus,
    heartbeatSoundToggle, heartbeatSoundIsEnabled, heartbeatPlaySound,
    heartbeatDownloadCapture
} from './heartbeat-vitals.js';

// ── Public API ─────────────────────────────────────────────

window.initHeartbeat = function(canvasId) {
    if (getState()) window.destroyHeartbeat();

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

    setState({
        canvas: canvas,
        ctx: ctx,
        width: w,
        height: h,
        dpr: dpr,

        // Virtual timeline
        timeline: [],
        virtualX: 0,
        viewOffset: -w * HEAD_POSITION_FRAC,
        viewOffsetY: 0,
        autoFollow: true,
        paused: false,
        renderMode: 'bricks',
        zoom: 1.9,
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
        _flashTimer: null,

        // Mempool feed
        ws: null,
        wsConnected: false,

        // SSE disconnection state (for overlay)
        _sseDisconnected: false,
        _sseDisconnectedSince: 0,

        // Recent blocks (kept for vital signs / rhythm strip)
        recentBlocks: []
    });

    var _hb = getState();

    // Start with a live flatline at position 0
    _hb.timeline.push(createFlatlineSegment(0, null));

    // Setup input handlers
    setupInputHandlers(canvas);

    // Show fullscreen button only if the API is supported (not on iOS)
    var fsBtn = document.getElementById('heartbeat-fullscreen-btn');
    if (fsBtn && (document.fullscreenEnabled || document.webkitFullscreenEnabled)) {
        fsBtn.classList.remove('hidden');
    }

    // Fullscreen sizing
    var wrap = document.getElementById('heartbeat-canvas-wrap');
    function _hbFullscreenChange() {
        var s = getState();
        if (!s || !wrap) return;
        var fsEl = document.fullscreenElement || document.webkitFullscreenElement;
        if (fsEl) {
            wrap.style.height = '';
        } else {
            wrap.style.height = '70vh';
        }
    }
    document.addEventListener('fullscreenchange', _hbFullscreenChange);
    document.addEventListener('webkitfullscreenchange', _hbFullscreenChange);
    _hb._fullscreenHandler = _hbFullscreenChange;

    // ResizeObserver for responsive canvas
    if (typeof ResizeObserver !== 'undefined') {
        _hb.resizeObs = new ResizeObserver(function() {
            var s = getState();
            if (!s) return;
            var r = canvas.getBoundingClientRect();
            canvas.width = r.width * dpr;
            canvas.height = r.height * dpr;
            s.ctx = canvas.getContext('2d');
            s.ctx.scale(dpr, dpr);
            s.width = r.width;
            s.height = r.height;
        });
        _hb.resizeObs.observe(container);
    }

    // Start animation loop
    _hb.rafId = requestAnimationFrame(drawFrame);

    // Connect to own node SSE feed (falls back to mempool.space WS)
    connectOwnFeed();
};

window.pushHeartbeatBlocks = function(json, replay) {
    var _hb = getState();
    if (!_hb) return;
    var isReplay = !!replay;
    try {
        var blocks = JSON.parse(json);
        if (!Array.isArray(blocks)) return;
        for (var i = 0; i < blocks.length; i++) {
            var b = blocks[i];

            // Deduplicate: skip if this block height is already in the timeline
            // Exception: if existing block was estimated (total_fees=0), replace with real data
            if (!isReplay && b.height) {
                var existingSeg = null;
                for (var di = _hb.timeline.length - 1; di >= Math.max(0, _hb.timeline.length - 30); di--) {
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
                        // Now show the announcement with real data
                        var feeBtc = b.total_fees / 100000000;
                        var heightStr = b.height.toLocaleString();
                        var nowAnn = Date.now() / 1000;
                        _hb._announcement = {
                            title: 'Block #' + heightStr + ' found',
                            subtitle: feeBtc.toFixed(4) + ' BTC fees \u00b7 ' + (b.tx_count || 0).toLocaleString() + ' txs',
                            color: replacement.color || _hb.currentColor || COLORS.healthy,
                            start: nowAnn,
                            end: nowAnn + 12.0
                        };
                    }
                    continue;
                }
            }

            var interBlock = b.inter_block_seconds || 600;
            // For live blocks, compute real interval from the previous block's timestamp
            if (!isReplay && _hb._prevBlockTime > 0 && b.timestamp > _hb._prevBlockTime) {
                interBlock = b.timestamp - _hb._prevBlockTime;
            } else if (!isReplay && _hb.lastBlockTime > 0 && b.timestamp > _hb.lastBlockTime) {
                interBlock = b.timestamp - _hb.lastBlockTime;
            }

            // Close the current live flatline
            var lastSeg = _hb.timeline[_hb.timeline.length - 1];
            if (lastSeg && lastSeg.type === 'flatline' && lastSeg.x_end === null) {
                if (isReplay) {
                    lastSeg.x_end = lastSeg.x_start + historyFlatlineWidth(interBlock);
                    _hb.virtualX = lastSeg.x_end;
                    var feeRate = b.total_fees ? b.total_fees / 100000 : 0;
                    lastSeg.color = computeColor(interBlock, feeRate, 0);
                } else {
                    // Scale min flatline with zoom so blocks always have a visible gap.
                    // At zoom 1.9x → 30px virtual = 57px screen.
                    // At zoom 0.6x → 50px virtual = 30px screen.
                    var minLiveFlatline = Math.max(30, Math.round(20 / Math.max(_hb.zoom, 0.1)));
                    if (_hb.virtualX - lastSeg.x_start < minLiveFlatline) {
                        _hb.virtualX = lastSeg.x_start + minLiveFlatline;
                    }
                    lastSeg.x_end = _hb.virtualX;
                }
                // Stamp the flatline with its color at close time
                if (!isReplay) {
                    lastSeg.color = _hb._preFlashColor || _hb.currentColor || COLORS.healthy;
                }
                // Shatter all blips into particles — they get absorbed into the spike
                if (lastSeg.blips) {
                    var absorbX = _hb.virtualX;
                    var absorbNow = Date.now() / 1000;
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

            // Show block arrival announcement (live blocks with real data only)
            if (!isReplay && b.height && b.total_fees > 0) {
                var feeBtc2 = b.total_fees / 100000000;
                var heightStr2 = b.height.toLocaleString();
                var nowAnn2 = Date.now() / 1000;
                _hb._announcement = {
                    title: 'Block #' + heightStr2 + ' found',
                    subtitle: feeBtc2.toFixed(4) + ' BTC fees \u00b7 ' + (b.tx_count || 0).toLocaleString() + ' txs',
                    color: blockSeg.color || _hb.currentColor || COLORS.healthy,
                    start: nowAnn2,
                    end: nowAnn2 + 12.0
                };
            }
        }

        if (_hb.recentBlocks.length > RETARGET_PERIOD) {
            _hb.recentBlocks = _hb.recentBlocks.slice(-RETARGET_PERIOD);
        }

        // Prune very old timeline segments
        pruneTimeline();

        // After replay: fast-forward the live flatline to reflect time since last block
        if (isReplay && blocks.length > 0 && _hb.lastBlockTime > 0) {
            var nowSec = Date.now() / 1000;
            var elapsed = nowSec - _hb.lastBlockTime;
            if (elapsed > 0 && elapsed < 7200) {
                var advancePx = elapsed * FLATLINE_PX_PER_SEC;
                _hb.virtualX += advancePx;
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
    var _hb = getState();
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
    var _hb = getState();
    if (!_hb) return;
    try {
        var data = JSON.parse(json);
        if (data.next_block_fee !== undefined) _hb.nextBlockFee = data.next_block_fee;
        if (data.mempool_mb !== undefined) _hb.mempoolMB = data.mempool_mb;
        // Don't update lastBlockTime from LiveStats — it resets the "Last block"
        // timer before the spike appears. lastBlockTime is set exclusively by
        // pushHeartbeatBlocks when the SSE block event arrives with complete data.
        if (data.hashrate_eh !== undefined) _hb.hashrateEH = data.hashrate_eh;
        if (data.mempool_min_fee !== undefined) _hb.mempoolMinFee = data.mempool_min_fee;
        if (data.difficulty !== undefined) _hb.difficulty = data.difficulty;
        if (data.block_height !== undefined) _hb.blockHeight = data.block_height;
    } catch (e) {
        // silently ignore malformed live JSON
    }
};

window.destroyHeartbeat = function() {
    var _hb = getState();
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
    if (_hb._listeners) {
        for (var i = 0; i < _hb._listeners.length; i++) {
            var l = _hb._listeners[i];
            l.target.removeEventListener(l.evt, l.fn, l.opts);
        }
    }
    window.removeEventListener('beforeunload', _hbBeforeUnload);
    document.removeEventListener('visibilitychange', _hbVisibilityChange);
    if (_hb._fullscreenHandler) {
        document.removeEventListener('fullscreenchange', _hb._fullscreenHandler);
        document.removeEventListener('webkitfullscreenchange', _hb._fullscreenHandler);
    }
    setState(null);
};

// Clean up on page unload
function _hbBeforeUnload() {
    if (window.destroyHeartbeat) window.destroyHeartbeat();
}
window.addEventListener('beforeunload', _hbBeforeUnload);

// When tab regains focus, preserve existing timeline and resume live flow.
// Don't reconnect SSE or clear blips — the live bricks stay where they were,
// virtualX catches up, queued blocks replay in place, and new txs resume at head.
function _hbVisibilityChange() {
    var _hb = getState();
    if (!_hb) return;
    if (document.hidden) {
        _hb._hiddenSince = Date.now() / 1000;
        return;
    }
    var now = Date.now() / 1000;

    // Advance virtualX by full elapsed hidden time
    if (_hb._hiddenSince > 0) {
        var hiddenDuration = now - _hb._hiddenSince;
        if (hiddenDuration > 0) {
            _hb.virtualX += hiddenDuration * FLATLINE_PX_PER_SEC;
            console.log('[heartbeat] tab return: advanced virtualX by', Math.round(hiddenDuration), 's (',
                Math.round(hiddenDuration * FLATLINE_PX_PER_SEC), 'px)');
        }
        _hb._hiddenSince = 0;
    }

    // Place buffered txs across the gap (the region between old bricks and
    // new virtualX). Timing is unreliable when hidden, but random spread
    // across the gap gives a natural fill instead of a visible hole.
    var liveSeg = _hb.timeline[_hb.timeline.length - 1];
    if (liveSeg && liveSeg.type === 'flatline' && liveSeg.x_end === null) {
        var buffered = (_hb._hiddenTxBuffer || []).concat(_hb._txBatchQueue || []);
        if (buffered.length > 0) {
            // Find where existing bricks end
            var rightmost = liveSeg.x_start;
            for (var bi = 0; bi < (liveSeg.blips || []).length; bi++) {
                if (liveSeg.blips[bi].fadeStart === 0 && liveSeg.blips[bi].x > rightmost) {
                    rightmost = liveSeg.blips[bi].x;
                }
            }
            var gapStart = rightmost + 5;
            var gapEnd = _hb.virtualX - 20; // leave 20px for live txs
            var gapSpan = gapEnd - gapStart;

            if (gapSpan > 10 && buffered.length > 0) {
                var medianFee = _hb._wsMedianFee || 5;
                var placed = 0;
                var maxStack = (_hb.height || 400) * 0.35;
                if (!liveSeg._colHeights) liveSeg._colHeights = {};

                for (var ti = 0; ti < buffered.length; ti++) {
                    var tx = buffered[ti];
                    if (!tx.fee || !tx.vsize) continue;
                    var feeRate = tx.fee / tx.vsize;
                    var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);
                    var brickH = 3 + feeNorm * 14 + Math.random() * 3;
                    var txVX = gapStart + Math.random() * gapSpan;
                    var gridX = Math.round(txVX / 5) * 5;
                    var stackY = liveSeg._colHeights[gridX] || 0;
                    if (stackY > maxStack) continue;
                    liveSeg._colHeights[gridX] = stackY + brickH;

                    liveSeg.blips.push({
                        x: txVX, gridX: gridX,
                        height: brickH + stackY, brickH: brickH, brickW: 4, stackY: stackY,
                        color: feeRateColor(feeRate, medianFee), opacity: 0.75 + feeNorm * 0.2,
                        txCount: 1, txid: tx.txid || null,
                        feeRate: Math.round(feeRate * 10) / 10,
                        vsize: tx.vsize || 0, value: tx.value || 0,
                        timestamp: now, isDrop: false, fadeStart: 0,
                        bobPhase: Math.random() * Math.PI * 2,
                        bobSpeed: 1.2 + feeNorm * 0.8,
                        lane: Math.floor(Math.random() * 5) - 2,
                        feeRatio: medianFee > 0 ? feeRate / medianFee : 1
                    });
                    placed++;
                }
                if (placed > 0) {
                    console.log('[heartbeat] tab return: placed', placed, 'buffered txs across gap');
                }
            }
        }
    }
    _hb._hiddenTxBuffer = [];
    _hb._txBatchQueue = [];

    // Replay any blocks that arrived while hidden as LIVE blocks (not compressed
    // replay). This places them at the correct virtualX positions with proper
    // inter-block flatlines sized by real timestamps.
    var queued = _hb._blockQueue || [];
    _hb._blockQueue = [];

    if (queued.length > 0) {
        console.log('[heartbeat] replaying', queued.length, 'queued blocks from background');
        for (var qi = 0; qi < queued.length; qi++) {
            var qb = queued[qi];
            var blockTs = qb.timestamp || Math.floor(now);
            // Compute inter-block from timeline's last block (same as processLiveBlock)
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
                height: qb.height,
                timestamp: blockTs,
                tx_count: qb.tx_count || qb.confirmed_count || 3000,
                total_fees: qb.total_fees || 0,
                size: qb.size || 0,
                weight: qb.weight || 3000000,
                inter_block_seconds: inter
            }]);
            window.pushHeartbeatBlocks(blockJson, false);
        }
    }

    // Sync frame timing so the draw loop doesn't try to catch up
    _hb.lastFrameTime = now;
}
document.addEventListener('visibilitychange', _hbVisibilityChange);

// ── Re-export vitals functions on window ───────────────────

window.getHeartbeatVitals = function() { return getHeartbeatVitals(); };
window.renderRhythmStrip = function(id, json) { renderRhythmStrip(id, json); };
window.getHeartbeatRecentBlocks = function() { return getHeartbeatRecentBlocks(); };
window.heartbeatCenter = function() { heartbeatCenter(); };
window.heartbeatSearchTx = function(txid) { return heartbeatSearchTx(txid); };
window.heartbeatPulse = function() { heartbeatPulse(); };
window.heartbeatFlash = function() { heartbeatFlash(); };
window.getOrganismStatus = function() { return getOrganismStatus(); };
window.heartbeatSoundToggle = function(enable) { return heartbeatSoundToggle(enable); };
window.heartbeatSoundIsEnabled = function() { return heartbeatSoundIsEnabled(); };
window.heartbeatPlaySound = function() { heartbeatPlaySound(); };
window.heartbeatDownloadCapture = function(json) { heartbeatDownloadCapture(json); };
