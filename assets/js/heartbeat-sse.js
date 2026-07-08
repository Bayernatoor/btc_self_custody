// heartbeat-sse.js — SSE connection, tx batching, and mempool fallback
import { getState, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, POINT_WIDTH } from './heartbeat-state.js';
import { feeRateColor, fmtBtc } from './heartbeat-timeline.js';

// Run the SEQUENCED block reveal up to this entry zoom. It works across the whole
// normal viewing range: the spike stays at the head, the unfurl zooms OUT to frame
// the whole timeline (so no blur even from a close entry zoom), then it returns to
// the entry zoom. 9x ≈ where per-brick fee/value labels start rendering (zoom>8), so
// below it nobody's reading tx details = no "real work" to interrupt. Above it the
// reveal is skipped for the instant flow (block appears, mempool re-lays, no camera
// hijack; a pinned tx shows "confirmed in block #N" if it got mined). Tunable.
var REVEAL_MAX_ENTRY_ZOOM = 9.0;

// Mempool-removal sweep (ZMQ `sequence` R events → bricks for txs that left the
// mempool via RBF/eviction). The sweep is an O(n) pass over the live segment, so
// throttle it: run when at least this many removals are pending, or when this many
// seconds have passed since the last sweep (whichever comes first) — never every
// frame. The txid-filter on incoming adds runs every flush (cheap) so an add for an
// already-removed tx is skipped even before the next sweep (RBF R-before-A race).
var REMOVAL_SWEEP_MIN = 50;
var REMOVAL_SWEEP_SECS = 1.0;

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

    // Median fee for colour thresholds. Sample ~2000 txs with a stride instead of
    // building + sorting a 45k-element array on the load path: the median of a
    // 2000-sample stride is indistinguishable from the exact one, and the full
    // sort was most of the old ~171ms 'history' handler cost.
    var SAMPLE_TARGET = 2000;
    var stride = Math.max(1, Math.floor(txs.length / SAMPLE_TARGET));
    var historyRates = [];
    for (var hi = 0; hi < txs.length; hi += stride) {
        var htx = txs[hi];
        if (htx.fee && htx.vsize) historyRates.push(htx.fee / htx.vsize);
    }
    if (historyRates.length >= 20) {
        historyRates.sort(function(a, b) { return a - b; });
        _hb._wsMedianFee = Math.max(1, historyRates[Math.floor(historyRates.length / 2)]);
    }
    var medianFee = _hb._wsMedianFee || 5;
    var placed = 0;

    var flatlineSpan = _hb.virtualX - liveSeg.x_start;
    var stackMap = {};
    // Share the column map immediately so live-tx flushes stack correctly against
    // the (progressively filled) history during chunked placement below.
    liveSeg._colHeights = stackMap;
    var histMaxStack = (_hb.height || 400) * 0.35;

    // Tell the renderer the eventual size of this segment up front, so the LOD
    // decision (density-columns vs individual bricks) is made from the KNOWN
    // mempool count — not the running blips.length as it fills. Without this a
    // >60k-tx first load renders bricks until the fill crosses LOD_MIN_BLIPS, then
    // pops to columns mid-fill (a jarring switch). Cleared in finishHistory.
    liveSeg._loadFillCount = txs.length;

    console.log('[heartbeat] placeHistoryTxs: ' + txs.length + ' txs, flatlineSpan=' +
        Math.round(flatlineSpan) + 'px, elapsed=' + Math.round(elapsedSinceBlock) + 's');

    // Auto-fit zoom NOW (before filling) so the view frames the whole mempool
    // while the bricks stream in. Only on first load (not reconnect, where the
    // user may have zoomed); autoFollow stays on so head-following continues.
    if (!instant && _hb.width && flatlineSpan > 10) {
        var fit = (_hb.width * 0.8) / flatlineSpan;
        _hb.zoom = Math.max(_hb.minZoom, Math.min(fit, 1.9));
        if (_hb.autoFollow) {
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
        }
        console.log('[heartbeat] auto-fit zoom to ' + _hb.zoom.toFixed(3) + 'x');
        // First-load intro: if there's a zoom-IN worth doing (fit is well below the
        // resting ~1.5x — i.e. a sizeable mempool), arm the "load reveal" — hold on
        // the whole mempool, then slow-zoom to the head (runIntro in drawFrame).
        if (_hb.autoFollow && _hb.zoom < 1.5) {
            _hb._intro = { phase: 'hold', start: Date.now() / 1000, fromZoom: _hb.zoom };
        }
    }

    // Stagger bricks in left-to-right over ~1.5s (instant on reconnect).
    var dropNow = Date.now() / 1000;
    var DROP_DURATION = instant ? 0 : 1.5;
    var notableTxs = [];

    // Fill in CHUNKS across RAF frames rather than one synchronous pass: at 20k
    // (soon ~55k) a single loop stalls the main thread for tens of ms on load.
    // Bricks stream in over a few frames; the flatline width + fit are already
    // set so the framing is correct — it just fills. Cancel any in-flight job.
    if (_hb._historyRaf) { cancelAnimationFrame(_hb._historyRaf); _hb._historyRaf = null; }
    var HISTORY_CHUNK = 2000;
    var i = 0;

    var finishHistory = function() {
        // Fill done: drop the up-front count so the LOD gate reverts to the real
        // (now complete) blips.length for this segment.
        liveSeg._loadFillCount = 0;
        // Notable feed newest-first (server sends newest-first, so iterate back).
        if (notableTxs.length > 0 && window._notableFeed) {
            for (var ni = notableTxs.length - 1; ni >= 0; ni--) {
                window._notableFeed(notableTxs[ni]);
            }
        }
        console.log('[heartbeat] placed ' + placed + ' history bricks on timeline');
    };

    var placeHistoryChunk = function() {
        var s = getState();
        // Abort if torn down, or the live flatline closed/changed (a block
        // arrived mid-fill) — remaining history would be stale.
        if (!s || s.timeline[s.timeline.length - 1] !== liveSeg || liveSeg.x_end !== null) {
            _hb._historyRaf = null;
            finishHistory();
            return;
        }
        var end = Math.min(i + HISTORY_CHUNK, txs.length);
        for (; i < end; i++) {
            var tx = txs[i];
            if (!tx.fee || !tx.vsize) continue;

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
            if (stackY > histMaxStack && !isNotable) continue;
            stackMap[gridX] = stackY + brickH;

            var style = notableStyle(flags);
            var blipColor = style ? style.color : feeRateColor(feeRate, medianFee);
            var notableType = style ? style.type : null;

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
        if (i < txs.length) {
            _hb._historyRaf = requestAnimationFrame(placeHistoryChunk);
        } else {
            _hb._historyRaf = null;
            finishHistory();
        }
    };

    placeHistoryChunk();
}

// Process a live block from SSE (shared by live handler + queue replay)
export function processLiveBlock(block) {
    var _hb = getState();
    if (!_hb) return;

    // If a block reveal is running: ignore a re-broadcast of the SAME block (poller
    // re-send / estimated→real replace) so it can't restart or cut the reveal short;
    // finalize the reveal first if a genuinely NEW block arrives mid-reveal.
    if (_hb._reveal && block && block.height) {
        if (_hb._reveal.height === block.height) return;
        if (window._hbEndReveal) window._hbEndReveal();
    }

    // Start each block from a clean backlog-spread state so a previous block's
    // spread (still within its 2.5s ramp) can't bleed into this block's flatline.
    _hb._backlogSpreadPx = 0;
    _hb._backlogSpreadStart = 0;

    // Reveal-eligible unless the timeline is PAUSED. autoFollow/LIVE is NOT required:
    // "not anchored to the head" (play mode — the canvas still moves with time) is
    // still a live, watchable view, so the reveal should play. Only pause (or mid-
    // drag/pinch, or the toggle off, or >5x inspection) suppresses it.
    var revealEligible = _hb._revealEnabled !== false
        && !_hb.paused && !_hb.isDragging && !_hb._pinching
        && _hb.width > 0 && _hb.zoom > 0
        && _hb.zoom <= REVEAL_MAX_ENTRY_ZOOM;

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
    // _revealPending tells it to DEFER the leftover re-lay to the sequencer.
    _hb._harvestLive = true;
    _hb._revealPending = revealEligible;
    window.pushHeartbeatBlocks(blockJson, false);
    _hb._revealPending = false;

    // Consume the deferred-reveal handoff unconditionally so a stale one (e.g. a
    // block that produced no leftovers) can never carry into a later block and
    // re-lay an old leftover set.
    var pending = _hb._pendingReveal;
    _hb._pendingReveal = null;
    if (revealEligible && pending) {
        // Sequenced reveal: the spike now sits at the current head; the leftovers
        // are still visible in the old segment. Hand off to the sequencer (drawFrame
        // runs it) — form → harvest → relay. Skip the collapsed-gap re-add: the
        // reveal owns the post-block flow.
        setupBlockReveal(_hb, pending);
    } else if (collapsedPx > 40) {
        // Instant path: re-add the collapsed gap AFTER the spike so the backlog of
        // validation-suppressed txs fans across it (see _backlogSpreadPx in
        // flushTxBatch) instead of walling at the head.
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
    // (During a sequenced reveal, drawFrame suppresses the flush so they queue.)
}

// DEV: fire a fake block on demand so the reveal can be tested without waiting for a
// real block. Runs the full live path (harvest + reveal) against the CURRENT real
// mempool. Localhost-only so it can't pollute a deployed timeline. Console:
//   window._hbFakeBlock()        — a ~3000-tx block
//   window._hbFakeBlock(20000)   — a big block (harvests more, carries less)
window._hbFakeBlock = function(txCount) {
    if (location.hostname !== 'localhost' && location.hostname !== '127.0.0.1') {
        console.warn('[heartbeat] _hbFakeBlock is dev-only (localhost)');
        return;
    }
    var _hb = getState();
    if (!_hb) return;
    var h = 0;
    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        if (_hb.timeline[i].type === 'block') { h = _hb.timeline[i].height; break; }
    }
    var tc = txCount || 3000;
    processLiveBlock({
        height: (h || _hb.blockHeight || 900000) + 1,
        timestamp: Math.floor(Date.now() / 1000),
        tx_count: tc,
        total_fees: 12000000, // 0.12 BTC — plausible so fees/announcement render
        size: 1500000,
        weight: 3990000
    });
    console.log('[heartbeat] fake block injected (tx_count=' + tc + ') — reveal should play');
};

// Arm the block-reveal SEQUENCER from the deferred-leftover handoff (pr, built in
// pushHeartbeatBlocks). The sequencer itself runs in drawFrame (runBlockReveal):
// the form beat zooms into the spike-at-head; the harvest beat flies the confirmed
// bricks in and fades the leftovers; the relay beat re-lays the leftovers and
// follows the placement to the new head.
// P5 exact harvest: re-partition an armed (pre-shatter) reveal's active bricks by
// the block's ACTUAL txids — bricks in the block shatter into the spike, the rest
// carry forward. Replaces the top-fee approximation from pushHeartbeatBlocks. No-op
// once the shatter has started (r.harvested) — too late to change what's flying.
// All the active bricks are still in the old segment during the form beat, so this
// only re-labels which shatter vs carry; the harvest beat then acts on the new sets.
function applyExactHarvest(r, blockSet) {
    if (!r || r.harvested) return;
    var all = r.confirmed.concat(r.leftovers);
    var conf = [], left = [];
    for (var i = 0; i < all.length; i++) {
        var b = all[i];
        if (b.txid && blockSet.has(b.txid)) conf.push(b); else left.push(b);
    }
    r.confirmed = conf;
    r.leftovers = left;
    r._exact = true;
    console.log('[heartbeat] exact harvest: ' + conf.length + ' confirmed / ' + left.length + ' carried');
}

function setupBlockReveal(_hb, pr) {
    var spikeSeg = pr.spikeSeg;
    if (!spikeSeg || !spikeSeg.points) return;
    // One reveal per block height (backstop against a post-reveal re-broadcast
    // re-arming it; the active-reveal dupe is handled at the top of processLiveBlock).
    if (_hb._lastRevealHeight === spikeSeg.height) return;
    _hb._lastRevealHeight = spikeSeg.height;

    // Focal point = the R peak (highest = most-negative y offset).
    var peakIdx = 0, peakY = 0;
    for (var p = 0; p < spikeSeg.points.length; p++) {
        if (spikeSeg.points[p] < peakY) { peakY = spikeSeg.points[p]; peakIdx = p; }
    }
    // Kill any in-flight momentum scroll so it can't fight the reveal's viewOffset
    // (both write it per frame). The momentum tick stops itself once vel < 0.5.
    _hb._momentumVel = 0;
    _hb.viewOffsetY = 0; // start the reveal vertically centered (drop any prior vertical pan)
    _hb._intro = null; // a real block supersedes the first-load intro
    _hb._reveal = {
        phase: 'form',              // form → harvest → relay (see runBlockReveal)
        start: Date.now() / 1000,
        height: spikeSeg.height,
        entryZoom: _hb.zoom,
        spikeSeg: spikeSeg,         // the block segment (for the arrival-fired strike)
        spikeVX: spikeSeg.x_start + peakIdx * POINT_WIDTH,
        oldSeg: pr.oldSeg,
        newSeg: pr.newSeg,
        leftovers: pr.leftovers,
        confirmed: pr.confirmed,    // bricks to shatter (harvest beat sets fadeStart)
        absorbX: pr.absorbX,
        maxParts: pr.maxParts,
        // Block stats for the reveal-managed banners (block-found → remaining-count).
        blockFees: spikeSeg.total_fees || 0,
        blockTxCount: spikeSeg.tx_count || 0,
        blockColor: spikeSeg.color || null,
        _bannerBlockSet: false,     // block-found banner shown once (form beat)
        _bannerRemainSet: false,    // remaining-count banner shown once (unfurl beat)
        harvested: false,
        laid: false,
        unfurlSecs: 0,              // set at the harvest→unfurl transition (rate-based)
        fitZoom: 0,                 // set there too (frames the whole timeline)
        width: 0,
        fromZoom: undefined,
        fromOffset: undefined,
        _deferStrike: false         // set below: strike on arrival vs immediately
    };

    // Should the spike strike NOW or wait until the camera arrives? If the spike is
    // already near the head on screen and we're not about to zoom in much, the view
    // is effectively centered → strike immediately (pop + admire during the form
    // beat). Otherwise (scrolled away / zoomed elsewhere) keep it FLAT and let the
    // form beat fire the strike on arrival, so the "BAM" lands when you get there.
    var spikeVX = _hb._reveal.spikeVX;
    var workingZoom = Math.max(_hb.zoom, 1); // mirrors REVEAL_ZOOM=1 in render.js
    var spikeScreenX = (spikeVX - _hb.viewOffset) * _hb.zoom;
    var headScreenX = _hb.width * HEAD_POSITION_FRAC;
    var nearHead = spikeScreenX > 0 && spikeScreenX < _hb.width
        && Math.abs(spikeScreenX - headScreenX) < _hb.width * 0.35;
    var zoomingIn = workingZoom / _hb.zoom > 1.25;
    if (nearHead && !zoomingIn) {
        spikeSeg._strikeStart = Date.now() / 1000; // centered → pop now
    } else {
        _hb._reveal._deferStrike = true;           // travel → pop on arrival (form beat)
    }

    // If the block's exact txids already arrived (out-of-order before this block
    // event), refine the harvest immediately.
    if (_hb._blockTxids && _hb._blockTxids.height === spikeSeg.height) {
        applyExactHarvest(_hb._reveal, _hb._blockTxids.set);
    }
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
        // The block's exact txids (P5): refine the reveal's harvest to shatter
        // EXACTLY the mined bricks instead of the top-fee approximation.
        es.addEventListener('block_txids', function(e) {
            if (!_hb) return;
            try {
                var data = JSON.parse(e.data); // { type, height, txids: [...] }
                if (!data || !data.height || !Array.isArray(data.txids)) return;
                var set = new Set(data.txids);
                // Stash for setupBlockReveal (covers txids arriving before the block
                // event, e.g. out-of-order on the broadcast channel).
                _hb._blockTxids = { height: data.height, set: set };
                // If a reveal for this block is already armed (and hasn't shattered),
                // refine it now.
                if (_hb._reveal && _hb._reveal.height === data.height) {
                    applyExactHarvest(_hb._reveal, set);
                }
                console.log('[heartbeat] SSE block_txids:', data.txids.length, 'for block', data.height);
            } catch (err) {}
        });
        // Mempool removals (ZMQ `sequence` R events): txids that left the mempool
        // for a NON-block reason (RBF/eviction/expiry). Accumulate into a Set; the
        // per-frame flush sweeps matching live bricks (applyRemovals) and filters
        // pending adds, so the timeline tracks the real mempool instead of drifting
        // upward. Block-confirmed txs are handled by the harvest, not here.
        es.addEventListener('tx_removed', function(e) {
            if (!_hb) return;
            try {
                var data = JSON.parse(e.data); // { type, txids: [...] }
                if (!data || !Array.isArray(data.txids) || data.txids.length === 0) return;
                _hb._lastSseEventTs = Date.now() / 1000; // liveness
                if (!_hb._removedTxids) _hb._removedTxids = new Set();
                for (var ri = 0; ri < data.txids.length; ri++) {
                    _hb._removedTxids.add(data.txids[ri]);
                }
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

// Remove bricks for txs that left the mempool (ZMQ `sequence` R events). One O(n)
// pass over the LIVE segment only (closed segments are frozen history), rebuilding
// the column map like the memory cull. Throttled: runs when enough removals are
// pending or REMOVAL_SWEEP_SECS elapsed. Only drops ACTIVE (non-fading) bricks —
// a brick mid-harvest/fade is left alone. Clears the set when it runs.
function applyRemovals(_hb, liveSeg) {
    var removed = _hb._removedTxids;
    if (!removed || removed.size === 0) return;
    var now = Date.now() / 1000;
    if (removed.size < REMOVAL_SWEEP_MIN &&
        (now - (_hb._lastRemovalSweep || 0)) < REMOVAL_SWEEP_SECS) {
        return; // defer — the add-queue filter still runs every flush meanwhile
    }
    _hb._lastRemovalSweep = now;

    if (!liveSeg.blips || liveSeg.blips.length === 0) { removed.clear(); return; }

    var kept = [];
    var dropped = 0;
    liveSeg._colHeights = {};
    for (var i = 0; i < liveSeg.blips.length; i++) {
        var b = liveSeg.blips[i];
        if (b.fadeStart === 0 && b.txid && removed.has(b.txid)) { dropped++; continue; }
        kept.push(b);
        // Restack kept, non-fading bricks against the rebuilt column map.
        if (b.fadeStart > 0 || b.gridX === undefined) continue;
        var sy = liveSeg._colHeights[b.gridX] || 0;
        b.stackY = sy;
        b.height = (b.brickH || 0) + sy;
        liveSeg._colHeights[b.gridX] = sy + (b.brickH || 0);
    }
    if (dropped > 0) {
        liveSeg.blips = kept;
        if (_hb._pinnedBlip && kept.indexOf(_hb._pinnedBlip) === -1) _hb._pinnedBlip = null;
        if (_hb.hoveredBlip && kept.indexOf(_hb.hoveredBlip) === -1) _hb.hoveredBlip = null;
    }
    removed.clear();
}

// Flush queued individual transactions into visual bricks
export function flushTxBatch() {
    var _hb = getState();
    if (!_hb) return;
    var liveSeg = _hb.timeline[_hb.timeline.length - 1];
    if (!liveSeg || liveSeg.type !== 'flatline') return;

    // Sweep mempool removals first (independent of the add queue — R events arrive
    // on their own cadence). Throttled internally so it's not an O(n) pass/frame.
    applyRemovals(_hb, liveSeg);

    if (!_hb._txBatchQueue || _hb._txBatchQueue.length === 0) return;

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

    var removedSet = _hb._removedTxids;
    for (var i = 0; i < batch.length; i++) {
        var tx = batch[i];
        // Skip an add whose tx already left the mempool (RBF R-before-A race): the
        // removal landed before we laid the brick, so never lay it.
        if (removedSet && removedSet.size > 0 && tx.txid && removedSet.has(tx.txid)) {
            continue;
        }
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

    // Memory backstop only — NOT a visual density limiter (the draw loop already
    // viewport-culls). Runs inside the RAF flush, so it must be cheap and must
    // NOT reorder by fee: this segment is "the mempool", and the old fee-score
    // sort (a) hitched the frame with an O(n log n) sort of tens of thousands and
    // (b) evicted the low-fee tail — the exact thing the visualization exists to
    // show. Instead evict the OLDEST (leftmost / furthest behind the head) bricks
    // in a single O(n) partition: no sort, fee-neutral, and when following the
    // head those are already off-screen. Threshold sits ABOVE the history limit
    // (HEARTBEAT_HISTORY_LIMIT=150k) + inter-block live accumulation, so a full
    // mempool we intentionally show is never evicted — this only fires as a true
    // memory guard. TARGET matches the history ceiling so a culled session still
    // shows the full mempool it loaded.
    var CULL_THRESHOLD = 170000;
    var CULL_TARGET = 150000;
    if (liveSeg.blips.length > CULL_THRESHOLD) {
        var over = liveSeg.blips.length - CULL_TARGET;
        var flatW = Math.max(1, _hb.virtualX - liveSeg.x_start);
        // Cutoff x below which (leftmost/oldest) non-notable bricks are dropped.
        // Estimate by fraction of the flatline width, with a little overshoot so
        // one pass gets us under target even with a non-uniform distribution.
        var cutoffX = liveSeg.x_start + Math.min(0.95, (over / liveSeg.blips.length) * 1.15) * flatW;
        var kept = [];
        liveSeg._colHeights = {};
        for (var ki = 0; ki < liveSeg.blips.length; ki++) {
            var cb = liveSeg.blips[ki];
            var keep = cb.notableType || cb.fadeStart > 0
                || (cb.gridX !== undefined ? cb.gridX : cb.x) >= cutoffX;
            if (!keep) continue;
            kept.push(cb);
            // Restack kept, non-fading bricks against the rebuilt column map.
            if (cb.fadeStart > 0 || cb.gridX === undefined) continue;
            var newStackY = liveSeg._colHeights[cb.gridX] || 0;
            cb.stackY = newStackY;
            cb.height = (cb.brickH || 0) + newStackY;
            liveSeg._colHeights[cb.gridX] = newStackY + (cb.brickH || 0);
        }
        console.log('[heartbeat] cull (memory): ' + liveSeg.blips.length + ' -> ' + kept.length +
            ' (dropped oldest left of x=' + Math.round(cutoffX) + ')');
        liveSeg.blips = kept;
        if (_hb._pinnedBlip && kept.indexOf(_hb._pinnedBlip) === -1) _hb._pinnedBlip = null;
        if (_hb.hoveredBlip && kept.indexOf(_hb.hoveredBlip) === -1) _hb.hoveredBlip = null;
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
