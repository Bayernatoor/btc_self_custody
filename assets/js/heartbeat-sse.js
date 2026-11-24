// heartbeat-sse.js: Heartbeat v2 state bridge, deterministic brick placement,
// keyed reconciliation, pending-fee grace buffer, and DOM-safe notable feed.
import { getState, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, POINT_WIDTH } from './heartbeat-state.js';
import { feeRateColor, formatFeeRate, fmtBtc } from './heartbeat-timeline.js';
import { playBlockCue } from './heartbeat-audio.js';

// Run the SEQUENCED block reveal up to this entry zoom (only when autoFollow && !_pinnedTxid).
var REVEAL_MAX_ENTRY_ZOOM = 9.0;

// Grace window (ms) to hold a newly observed membership_add before dropping it as a
// pending-fee slate brick, giving tx_details a moment to arrive with its real fee.
export var PENDING_GRACE_MS = 120;

// Chunk size for RAF-spread initial snapshot placement so 50k+ tx loads stay at 60fps.
var HISTORY_CHUNK = 2500;

// Memory backstop for long-running sessions with huge mempools.
var CULL_THRESHOLD = 170000;
var CULL_TARGET = 150000;

// ── TxFlags bitmask & notable classification ───────────────────────────────
// Bit layout matches Rust `TxFlags`:
//   bit 0: whale, bit 1: feeOutlier, bit 2: consolidation, bit 3: fanOut,
//   bit 4: largeInscription, bit 5: roundNumber, bit 6: opReturnMsg
export function decodeTxFlags(bits) {
    var b = bits | 0;
    return {
        whale: (b & 1) !== 0,
        feeOutlier: (b & 2) !== 0,
        consolidation: (b & 4) !== 0,
        fanOut: (b & 8) !== 0,
        largeInscription: (b & 16) !== 0,
        roundNumber: (b & 32) !== 0,
        opReturnMsg: (b & 64) !== 0
    };
}

// Accepts boolean flags, persisted notable_type string, or CompactTx flags bitmask.
function notableFlags(tx) {
    var nt = tx.notable_type || null;
    var b = typeof tx.flags === 'number' ? (tx.flags | 0) : 0;
    return {
        whale: !!tx.whale || nt === 'whale' || (b & 1) !== 0,
        feeOutlier: !!tx.fee_outlier || nt === 'fee_outlier' || (b & 2) !== 0,
        consolidation: !!tx.consolidation || nt === 'consolidation' || (b & 4) !== 0,
        fanOut: !!tx.fan_out || nt === 'fan_out' || (b & 8) !== 0,
        largeInscription: !!tx.large_inscription || nt === 'large_inscription' || (b & 16) !== 0,
        roundNumber: !!tx.round_number || nt === 'round_number' || (b & 32) !== 0,
        opReturnMsg: !!tx.op_return_msg || nt === 'op_return_msg' || (b & 64) !== 0
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

// Parse a CompactTx tuple:
// [txid, first_seen, fee?, vsize?, value?, flags?, op_return_text?, observed_order?]
export function compactTupleToTx(tuple) {
    if (!tuple) return null;
    if (!Array.isArray(tuple)) return tuple;
    return {
        txid: tuple[0] || '',
        first_seen: typeof tuple[1] === 'number' ? tuple[1] : 0,
        fee: typeof tuple[2] === 'number' ? tuple[2] : null,
        vsize: typeof tuple[3] === 'number' ? tuple[3] : null,
        value: typeof tuple[4] === 'number' ? tuple[4] : null,
        flags: typeof tuple[5] === 'number' ? tuple[5] : 0,
        op_return_text: typeof tuple[6] === 'string' ? tuple[6] : '',
        observed_order: typeof tuple[7] === 'number' ? tuple[7] : null
    };
}

// Deterministic 32-bit FNV-1a hash over full txid (stable across reloads/resyncs).
function txidHash32(txid) {
    if (!txid || txid.length === 0) return 2166136261;
    var h = 2166136261;
    for (var i = 0; i < txid.length; i++) {
        h ^= txid.charCodeAt(i);
        h = Math.imul(h, 16777619);
    }
    // Final avalanche mix so both low and high 16-bit halves have full entropy.
    h ^= h >>> 16;
    h = Math.imul(h, 2246822507);
    h ^= h >>> 13;
    h = Math.imul(h, 3266489909);
    h ^= h >>> 16;
    return h >>> 0;
}

// Deterministic [0, 1) jitter derived from txid so updates never shift brick height randomly.
function txidJitter(txid) {
    return (txidHash32(txid) & 0xffff) / 65536;
}

// Deterministic signed offset in [-1, 1] using the upper hash bits (independent
// of txidJitter) so bricks form an organic, reproducible skyline around their
// chronological column instead of a flat ruler-cut strip.
export function txidSkylineOffset(txid) {
    var h = txidHash32(txid);
    var u = ((h >>> 16) & 0xffff) / 65536;
    return u * 2 - 1;
}

function nowPerfMs() {
    return typeof performance !== 'undefined' ? performance.now() : Date.now();
}

export function ensureSegMaps(seg) {
    if (!seg) return;
    if (!seg._blipsByTxid) seg._blipsByTxid = new Map();
    if (!seg._colHeights) seg._colHeights = {};
    if (!seg._colBlips) seg._colBlips = new Map();
}

function getLiveFlatline(_hb) {
    if (!_hb || !_hb.timeline || _hb.timeline.length === 0) return null;
    var seg = _hb.timeline[_hb.timeline.length - 1];
    if (!seg || seg.type !== 'flatline' || seg.x_end !== null) return null;
    ensureSegMaps(seg);
    return seg;
}

export function registerBlipInSeg(seg, blip) {
    ensureSegMaps(seg);
    blip._removed = false;
    blip._segIdx = seg.blips.length;
    seg.blips.push(blip);
    if (blip.txid) seg._blipsByTxid.set(blip.txid, blip);
    var gx = blip.gridX;
    var col = seg._colBlips.get(gx);
    if (!col) {
        col = [];
        seg._colBlips.set(gx, col);
    }
    col.push(blip);
    seg._colHeights[gx] = (blip.stackY || 0) + (blip.brickH || 0);
}

function removeBlipFromArray(seg, blip) {
    var arr = seg.blips;
    if (!arr || arr.length === 0) return;
    var idx = blip._segIdx;
    if (idx === undefined || idx < 0 || idx >= arr.length || arr[idx] !== blip) {
        idx = arr.indexOf(blip);
    }
    if (idx >= 0) {
        var last = arr.pop();
        if (idx < arr.length) {
            arr[idx] = last;
            last._segIdx = idx;
        }
    }
    blip._segIdx = -1;
}

// Recompute vertical stackY and height ONLY for affected gridX columns, preserving
// each surviving brick's exact horizontal gridX and vertical bottom-to-top order.
export function restackColumns(liveSeg, dirtyGridXs) {
    if (!liveSeg || !dirtyGridXs) return;
    ensureSegMaps(liveSeg);
    var iter = Array.isArray(dirtyGridXs) ? dirtyGridXs : Array.from(dirtyGridXs);
    for (var di = 0; di < iter.length; di++) {
        var gx = iter[di];
        if (gx === undefined || gx === null) continue;
        var col = liveSeg._colBlips.get(gx);
        if (!col) {
            delete liveSeg._colHeights[gx];
            continue;
        }
        var sy = 0;
        var kept = [];
        for (var i = 0; i < col.length; i++) {
            var b = col[i];
            if (!b || b._removed || b.fadeStart > 0) continue;
            b.stackY = sy;
            b.height = sy + (b.brickH || 0);
            sy += (b.brickH || 0);
            kept.push(b);
        }
        if (kept.length > 0) {
            liveSeg._colBlips.set(gx, kept);
            liveSeg._colHeights[gx] = sy;
        } else {
            liveSeg._colBlips.delete(gx);
            delete liveSeg._colHeights[gx];
        }
    }
}

// Construct a brick blip with exact fee metadata and deterministic dimensions.
export function buildBlip(tx, x, gridX, stackY, dropTimestamp, isDrop) {
    var feePending = (tx.fee == null || tx.vsize == null || tx.vsize <= 0);
    var feeRate = feePending ? null : (tx.fee / tx.vsize);
    var feeNorm = feePending ? 0.15 : Math.min(Math.log2(feeRate + 1) / 6, 1.0);
    var flags = notableFlags(tx);
    var isNotable = anyNotable(flags);
    var style = notableStyle(flags);
    var jitter = txidJitter(tx.txid || '');
    var brickW = isNotable ? 4.4 : 4;
    var brickH = isNotable
        ? 18 + feeNorm * 10 + jitter * 3
        : (feePending ? 5 + jitter * 2 : 3 + feeNorm * 14 + jitter * 3);
    var blipColor = style ? style.color : feeRateColor(feeRate);
    var notableType = style ? style.type : null;

    return {
        x: x,
        gridX: gridX,
        height: brickH + stackY,
        brickH: brickH,
        brickW: brickW,
        stackY: stackY,
        color: blipColor,
        opacity: isNotable ? 1.0 : (feePending ? 0.65 : 0.75 + feeNorm * 0.2),
        txCount: 1,
        txid: tx.txid || null,
        fee: tx.fee != null ? tx.fee : null,
        vsize: tx.vsize != null ? tx.vsize : null,
        value: tx.value != null ? tx.value : null,
        feeRate: feeRate,
        feePending: feePending,
        firstSeen: typeof tx.first_seen === 'number' ? tx.first_seen : 0,
        flags: typeof tx.flags === 'number' ? tx.flags : 0,
        timestamp: dropTimestamp,
        isDrop: !!isDrop,
        fadeStart: 0,
        bobPhase: jitter * Math.PI * 2,
        bobSpeed: isNotable ? 0.6 : 1.2 + feeNorm * 0.8,
        lane: Math.floor(jitter * 5) - 2,
        feeRatio: feeRate != null ? feeRate / 5 : 1,
        whale: flags.whale,
        feeOutlier: flags.feeOutlier,
        notableType: notableType,
        valueUsd: tx.value_usd || 0,
        opReturnText: tx.op_return_text || '',
        inputCount: tx.input_count || 0,
        outputCount: tx.output_count || 0
    };
}

// Update an existing brick's fee/notable metadata in place without moving its gridX.
// Returns true if brickH changed (requiring single-column vertical restack).
export function updateBlipMetadata(blip, tx) {
    var feePending = (tx.fee == null || tx.vsize == null || tx.vsize <= 0);
    var feeRate = feePending ? null : (tx.fee / tx.vsize);
    var feeNorm = feePending ? 0.15 : Math.min(Math.log2(feeRate + 1) / 6, 1.0);
    var flags = notableFlags(tx);
    var isNotable = anyNotable(flags);
    var style = notableStyle(flags);
    var jitter = txidJitter(blip.txid || tx.txid || '');
    var newW = isNotable ? 4.4 : 4;
    var newH = isNotable
        ? 18 + feeNorm * 10 + jitter * 3
        : (feePending ? 5 + jitter * 2 : 3 + feeNorm * 14 + jitter * 3);
    var oldH = blip.brickH;

    blip.fee = tx.fee != null ? tx.fee : null;
    blip.vsize = tx.vsize != null ? tx.vsize : null;
    if (tx.value != null) {
        blip.value = tx.value;
    }
    blip.feeRate = feeRate;
    blip.feePending = feePending;
    if (typeof tx.first_seen === 'number' && tx.first_seen > 0) {
        blip.firstSeen = tx.first_seen;
    }
    if (typeof tx.flags === 'number') {
        blip.flags = tx.flags;
    }
    blip.color = style ? style.color : feeRateColor(feeRate);
    blip.opacity = isNotable ? 1.0 : (feePending ? 0.65 : 0.75 + feeNorm * 0.2);
    blip.brickW = newW;
    blip.brickH = newH;
    blip.height = (blip.stackY || 0) + newH;
    blip.bobSpeed = isNotable ? 0.6 : 1.2 + feeNorm * 0.8;
    blip.feeRatio = feeRate != null ? feeRate / 5 : 1;
    blip.whale = flags.whale;
    blip.feeOutlier = flags.feeOutlier;
    blip.notableType = style ? style.type : null;
    if (tx.value_usd) blip.valueUsd = tx.value_usd;
    if (tx.op_return_text) blip.opReturnText = tx.op_return_text;
    if (tx.input_count) blip.inputCount = tx.input_count;
    if (tx.output_count) blip.outputCount = tx.output_count;

    return Math.abs(oldH - newH) > 0.001;
}

// Build an O(1) txid -> blip index across a reveal (or pending-reveal) object's
// leftovers and confirmed arrays so mid-reveal removals, details, and inclusions
// never miss in-flight bricks.
export function rebuildRevealIndex(r) {
    if (!r) return;
    var m = new Map();
    if (r.leftovers) {
        for (var i = 0; i < r.leftovers.length; i++) {
            var b = r.leftovers[i];
            if (b && !b._removed && b.txid) m.set(b.txid, b);
        }
    }
    if (r.confirmed) {
        for (var j = 0; j < r.confirmed.length; j++) {
            var c = r.confirmed[j];
            if (c && !c._removed && c.txid) m.set(c.txid, c);
        }
    }
    r._revealByTxid = m;
}

// Look up an in-flight brick waiting in chunked post-block carry, pending reveal,
// or an active reveal before it has been re-registered on the live flatline.
function findInFlightBlip(_hb, txid) {
    if (!_hb || !txid) return null;
    if (_hb._pendingCarryByTxid && _hb._pendingCarryByTxid.has(txid)) {
        return _hb._pendingCarryByTxid.get(txid);
    }
    if (_hb._pendingReveal) {
        if (!_hb._pendingReveal._revealByTxid) rebuildRevealIndex(_hb._pendingReveal);
        if (_hb._pendingReveal._revealByTxid.has(txid)) {
            return _hb._pendingReveal._revealByTxid.get(txid);
        }
    }
    if (_hb._reveal && !_hb._reveal.laid) {
        if (!_hb._reveal._revealByTxid) rebuildRevealIndex(_hb._reveal);
        if (_hb._reveal._revealByTxid.has(txid)) {
            return _hb._reveal._revealByTxid.get(txid);
        }
    }
    return null;
}

// Clear the removal tombstone set once no chunked fill, carry, reveal, or queue
// can still resurrect a removed txid.
export function maybeClearRemovedTxids(_hb, minSize) {
    if (!_hb || !_hb._removedTxids || _hb._removedTxids.size === 0) return;
    if (minSize && _hb._removedTxids.size <= minSize) return;
    if (_hb._historyRaf || _hb._carryRaf || _hb._reveal || _hb._pendingReveal) return;
    if (_hb._pendingAdds && _hb._pendingAdds.size > 0) return;
    if (_hb._txBatchQueue && _hb._txBatchQueue.length > 0) return;
    if (_hb._hiddenTxMap && _hb._hiddenTxMap.size > 0) return;
    _hb._removedTxids.clear();
}

// Mark matching in-flight bricks removed across pending carry, pending reveal,
// and active reveal, filtering reveal arrays once per batch.
function purgeInFlightTxids(_hb, txids) {
    if (!_hb || !txids || txids.length === 0) return;
    var dirtyPendingReveal = false;
    var dirtyReveal = false;
    var pr = _hb._pendingReveal;
    var rv = (_hb._reveal && !_hb._reveal.laid) ? _hb._reveal : null;
    if (pr && !pr._revealByTxid) rebuildRevealIndex(pr);
    if (rv && !rv._revealByTxid) rebuildRevealIndex(rv);

    for (var i = 0; i < txids.length; i++) {
        var tid = txids[i];
        if (!tid) continue;
        if (_hb._pendingCarryByTxid && _hb._pendingCarryByTxid.has(tid)) {
            var cb = _hb._pendingCarryByTxid.get(tid);
            _hb._pendingCarryByTxid.delete(tid);
            if (cb) {
                cb._removed = true;
                if (_hb.hoveredBlip === cb) _hb.hoveredBlip = null;
                if (_hb._pinnedBlip === cb) _hb._pinnedBlip = null;
            }
        }
        if (pr && pr._revealByTxid && pr._revealByTxid.has(tid)) {
            var pb = pr._revealByTxid.get(tid);
            pr._revealByTxid.delete(tid);
            if (pb) {
                pb._removed = true;
                if (_hb.hoveredBlip === pb) _hb.hoveredBlip = null;
                if (_hb._pinnedBlip === pb) _hb._pinnedBlip = null;
            }
            dirtyPendingReveal = true;
        }
        if (rv && rv._revealByTxid && rv._revealByTxid.has(tid)) {
            var rb = rv._revealByTxid.get(tid);
            rv._revealByTxid.delete(tid);
            if (rb) {
                rb._removed = true;
                if (_hb.hoveredBlip === rb) _hb.hoveredBlip = null;
                if (_hb._pinnedBlip === rb) _hb._pinnedBlip = null;
            }
            dirtyReveal = true;
        }
    }

    if (dirtyPendingReveal && pr) {
        if (pr.leftovers) pr.leftovers = pr.leftovers.filter(function(b) { return b && !b._removed; });
        if (pr.confirmed) pr.confirmed = pr.confirmed.filter(function(b) { return b && !b._removed; });
    }
    if (dirtyReveal && rv) {
        if (rv.leftovers) rv.leftovers = rv.leftovers.filter(function(b) { return b && !b._removed; });
        if (rv.confirmed) rv.confirmed = rv.confirmed.filter(function(b) { return b && !b._removed; });
    }
}

// ── Snapshot installation & keyed reconciliation (v2) ────────────────────

function syncBlocksFromSnapshot(_hb, payload, isResync) {
    if (!_hb._exactConfirmedByHash) _hb._exactConfirmedByHash = new Map();
    if (Array.isArray(payload.recent_confirmations)) {
        for (var ci = 0; ci < payload.recent_confirmations.length; ci++) {
            var conf = payload.recent_confirmations[ci];
            if (conf && conf.hash && Array.isArray(conf.txids)) {
                _hb._exactConfirmedByHash.set(conf.hash, {
                    height: conf.height || 0,
                    set: new Set(conf.txids)
                });
            }
        }
    }
    _hb._pendingBlocks = Array.isArray(payload.pending_blocks) ? payload.pending_blocks.slice() : [];

    var recent = Array.isArray(payload.recent_blocks) ? payload.recent_blocks : [];
    if (recent.length === 0) return;

    var sorted = recent.slice().sort(function(a, b) { return (a.height || 0) - (b.height || 0); });
    var hasBlocks = false;
    var maxH = 0;
    for (var ti = _hb.timeline.length - 1; ti >= 0; ti--) {
        if (_hb.timeline[ti].type === 'block') {
            hasBlocks = true;
            if (_hb.timeline[ti].height > maxH) maxH = _hb.timeline[ti].height;
        }
    }

    if (!isResync && !hasBlocks && typeof window.pushHeartbeatBlocks === 'function') {
        var replayList = [];
        for (var i = 0; i < sorted.length; i++) {
            var prevTs = i > 0 ? sorted[i - 1].timestamp : (sorted[i].timestamp - 600);
            replayList.push({
                hash: sorted[i].hash || null,
                height: sorted[i].height,
                timestamp: sorted[i].timestamp,
                tx_count: sorted[i].tx_count,
                total_fees: sorted[i].total_fees,
                size: sorted[i].size,
                weight: sorted[i].weight,
                inter_block_seconds: Math.max(1, (sorted[i].timestamp || 0) - (prevTs || 0))
            });
        }
        window.pushHeartbeatBlocks(JSON.stringify(replayList), true);
    } else if (hasBlocks && typeof window.pushHeartbeatBlocks === 'function') {
        // Append only blocks newer than our current timeline tip without hijacking camera.
        var missed = [];
        for (var mi = 0; mi < sorted.length; mi++) {
            if ((sorted[mi].height || 0) > maxH) missed.push(sorted[mi]);
        }
        if (missed.length > 0) {
            var prevSuppress = _hb._suppressAnnounce;
            _hb._suppressAnnounce = true;
            for (var k = 0; k < missed.length; k++) {
                _hb._harvestLive = true;
                _hb._revealPending = false;
                window.pushHeartbeatBlocks(JSON.stringify([{
                    hash: missed[k].hash || null,
                    height: missed[k].height,
                    timestamp: missed[k].timestamp,
                    tx_count: missed[k].tx_count,
                    total_fees: missed[k].total_fees,
                    size: missed[k].size,
                    weight: missed[k].weight
                }]), false);
            }
            _hb._suppressAnnounce = prevSuppress;
        }
    }

    var newestBlock = sorted[sorted.length - 1];
    if (newestBlock && newestBlock.timestamp > (_hb.lastBlockTime || 0)) {
        _hb.lastBlockTime = newestBlock.timestamp;
    }
}

export function hbV2InstallSnapshot(payloadJson, isResync) {
    var _hb = getState();
    if (!_hb) return;

    var payload;
    try {
        payload = typeof payloadJson === 'string' ? JSON.parse(payloadJson) : payloadJson;
    } catch (err) {
        console.warn('[heartbeat] hbV2InstallSnapshot parse error:', err);
        return;
    }
    if (!payload || typeof payload !== 'object') return;

    _hb._coverage = payload.coverage || { status: 'unknown' };
    _hb._lastSseEventTs = Date.now() / 1000;

    // Sync confirmations, pending blocks, and recent blocks before placing mempool txs.
    syncBlocksFromSnapshot(_hb, payload, !!isResync);

    var liveSeg = getLiveFlatline(_hb);
    if (!liveSeg) return;

    // Cancel any in-flight chunked snapshot placement and clear transient queues.
    if (_hb._historyRaf) { cancelAnimationFrame(_hb._historyRaf); _hb._historyRaf = null; }
    if (!_hb._pendingAdds) _hb._pendingAdds = new Map();
    else _hb._pendingAdds.clear();
    _hb._txBatchQueue = [];
    if (!_hb._removedTxids) _hb._removedTxids = new Set();
    else _hb._removedTxids.clear();
    _hb._deferredDetails = new Map();

    var rawTxs = Array.isArray(payload.transactions) ? payload.transactions : [];
    var maxStack = (_hb.height || 400) * 0.35;
    var nowSec = Date.now() / 1000;

    // ── Non-destructive keyed reconciliation on resync ──
    if (isResync && liveSeg._blipsByTxid.size > 0) {
        liveSeg._loadFillCount = 0;
        var incomingMap = new Map();
        var incomingOrder = [];
        for (var ii = 0; ii < rawTxs.length; ii++) {
            var parsed = compactTupleToTx(rawTxs[ii]);
            if (!parsed || !parsed.txid) continue;
            if (!incomingMap.has(parsed.txid)) {
                incomingMap.set(parsed.txid, parsed);
                incomingOrder.push(parsed);
            }
        }

        var dirtyCols = new Set();
        var toRemove = [];
        liveSeg._blipsByTxid.forEach(function(blip, txid) {
            if (!incomingMap.has(txid)) {
                toRemove.push(txid);
            }
        });
        for (var ri = 0; ri < toRemove.length; ri++) {
            var rTxid = toRemove[ri];
            var rBlip = liveSeg._blipsByTxid.get(rTxid);
            if (!rBlip) continue;
            liveSeg._blipsByTxid.delete(rTxid);
            rBlip._removed = true;
            removeBlipFromArray(liveSeg, rBlip);
            dirtyCols.add(rBlip.gridX);
            if (_hb.hoveredBlip === rBlip) _hb.hoveredBlip = null;
            // Clear the canvas blip ref if removed, but NEVER clear _hb._pinnedTxid:
            // WASM updates the pinned panel with confirmed/removed status.
            if (_hb._pinnedBlip === rBlip) _hb._pinnedBlip = null;
        }

        var newTxs = [];
        for (var ni = 0; ni < incomingOrder.length; ni++) {
            var itx = incomingOrder[ni];
            var existing = liveSeg._blipsByTxid.get(itx.txid);
            if (existing) {
                var hChanged = updateBlipMetadata(existing, itx);
                if (hChanged) dirtyCols.add(existing.gridX);
            } else {
                newTxs.push(itx);
            }
        }

        if (dirtyCols.size > 0) {
            restackColumns(liveSeg, dirtyCols);
        }

        // Append newly discovered txids cleanly behind the live head without
        // moving existing bricks, jumping virtualX on small diffs, or moving
        // an inspecting viewer's camera.
        if (newTxs.length > 0) {
            var minGx = Math.ceil(liveSeg.x_start / 5) * 5;
            var targetCols = Math.max(1, Math.ceil(newTxs.length / 10));
            var neededSpan = targetCols * 5;
            var headGx = Math.max(minGx, Math.floor(_hb.virtualX / 5) * 5);
            var startGx = Math.max(minGx, headGx - neededSpan);
            if (newTxs.length > 50 && startGx + neededSpan + 10 > _hb.virtualX) {
                _hb.virtualX = startGx + neededSpan + 10;
                headGx = Math.floor(_hb.virtualX / 5) * 5;
            }
            var availCols = Math.max(1, Math.floor(Math.max(5, headGx - startGx) / 5));
            for (var ai = 0; ai < newTxs.length; ai++) {
                var ntx = newTxs[ai];
                var gx;
                var nAge = ntx.first_seen > 0 ? (nowSec - ntx.first_seen) : -1;
                if (nAge >= 0 && nAge <= 300) {
                    var ageCols = Math.round((nAge * FLATLINE_PX_PER_SEC) / 5);
                    var jit = Math.round(txidSkylineOffset(ntx.txid) * 1.5);
                    gx = Math.max(minGx, Math.min(headGx, headGx - (ageCols + jit) * 5));
                } else {
                    var colIdx = Math.floor((ai * availCols) / newTxs.length);
                    gx = startGx + colIdx * 5;
                }
                var sy = liveSeg._colHeights[gx] || 0;
                var nBlip = buildBlip(ntx, gx, gx, sy, nowSec, false);
                var steps = 0;
                while (sy > 0 && sy + nBlip.brickH > maxStack && steps < 20) {
                    gx = gx >= minGx + 5 ? gx - 5 : gx + 5;
                    sy = liveSeg._colHeights[gx] || 0;
                    steps++;
                }
                nBlip.x = gx;
                nBlip.gridX = gx;
                nBlip.stackY = sy;
                nBlip.height = sy + nBlip.brickH;
                registerBlipInSeg(liveSeg, nBlip);
                if (anyNotable(notableFlags(ntx)) && window._notableFeed) {
                    window._notableFeed(ntx);
                }
            }
        }

        if (_hb._pinnedTxid) {
            _hb._pinnedBlip = liveSeg._blipsByTxid.get(_hb._pinnedTxid) || null;
        }
        return;
    }

    // ── Initial ordered backlog placement ──
    liveSeg.blips = [];
    liveSeg._blipsByTxid = new Map();
    liveSeg._colHeights = {};
    liveSeg._colBlips = new Map();

    var TARGET_COL_H = 65;
    var parsedTxs = new Array(rawTxs.length);
    var parsedCount = 0;
    var totalBrickH = 0;
    var unknownBrickH = 0;
    for (var pi = 0; pi < rawTxs.length; pi++) {
        var ptx = compactTupleToTx(rawTxs[pi]);
        if (!ptx || !ptx.txid) continue;
        if (_hb._removedTxids && _hb._removedTxids.has(ptx.txid)) continue;
        parsedTxs[parsedCount++] = ptx;
        var pFlags = notableFlags(ptx);
        var pNotable = anyNotable(pFlags);
        var pPending = ptx.fee == null || ptx.vsize == null || ptx.vsize <= 0;
        var pRate = pPending ? null : (ptx.fee / ptx.vsize);
        var pNorm = pPending ? 0.15 : Math.min(Math.log2(Math.max(0, pRate) + 1) / 6, 1.0);
        var pJitter = txidJitter(ptx.txid);
        var pH = pNotable
            ? (18 + pNorm * 10 + pJitter * 3)
            : (pPending ? (5 + pJitter * 2) : (3 + pNorm * 14 + pJitter * 3));
        totalBrickH += pH;
        if (!ptx.first_seen) unknownBrickH += pH;
    }
    parsedTxs.length = parsedCount;

    var elapsedSinceBlock = _hb.lastBlockTime > 0 ? Math.max(0, nowSec - _hb.lastBlockTime) : 0;
    var timeWidth = elapsedSinceBlock * FLATLINE_PX_PER_SEC;
    var volumeCols = Math.max(1, Math.ceil(totalBrickH / TARGET_COL_H));
    var volumeWidth = volumeCols * 5;
    var neededX = liveSeg.x_start + Math.max(30, timeWidth, volumeWidth + 15);
    if (neededX > _hb.virtualX) {
        _hb.virtualX = neededX;
    }
    var flatlineSpan = _hb.virtualX - liveSeg.x_start;
    liveSeg._loadFillCount = parsedCount;

    // Auto-fit zoom ONLY on initial load when auto-following and not pinned.
    // Re-arm the load fade veil so dismissing the loading overlay fades cleanly
    // into the already-framed mempool with zero zoomed-in spike flash.
    if (!isResync) {
        _hb._loadFadeStart = nowSec;
    }
    if (!isResync && _hb.autoFollow && !_hb._pinnedTxid && _hb.width && flatlineSpan > 10) {
        var fit = (_hb.width * 0.8) / flatlineSpan;
        _hb.zoom = Math.max(_hb.minZoom, Math.min(fit, 1.9));
        _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
        if (_hb.zoom < 1.5) {
            _hb._intro = { phase: 'hold', start: nowSec, fromZoom: _hb.zoom };
        }
    }

    // Macro-ordered organic skyline: O(n) column bucketing (avoiding an O(n log n)
    // sort over 45k+ items on the main thread). Within each ~10-brick column bucket,
    // sort by deterministic yKey so bursts mix vertically with zero diagonal shear.
    var DROP_DURATION = isResync ? 0 : 2.3;
    var dropOrigin = nowSec + (isResync ? 0 : 0.08);
    var startGridX = Math.ceil(liveSeg.x_start / 5) * 5;
    var usableSpan = Math.max(5, flatlineSpan - 15);
    var usableCols = Math.max(1, Math.floor(usableSpan / 5));
    var denomH = Math.max(1, totalBrickH);
    var unknownCols = unknownBrickH > 0
        ? Math.max(1, Math.min(usableCols, Math.round((unknownBrickH / denomH) * usableCols)))
        : 0;
    var knownStartCol = unknownCols;
    var knownCols = Math.max(1, usableCols - knownStartCol);
    var localSpreadCols = Math.min(Math.max(12, Math.floor(usableCols * 0.08)), 160);
    var softMaxStack = Math.min(maxStack, 160);

    var colBuckets = new Array(usableCols);
    var cumH = 0;
    for (var pj = 0; pj < parsedCount; pj++) {
        var pTx = parsedTxs[pj];
        var pFlags2 = notableFlags(pTx);
        var pNotable2 = anyNotable(pFlags2);
        var pPending2 = pTx.fee == null || pTx.vsize == null || pTx.vsize <= 0;
        var pRate2 = pPending2 ? null : (pTx.fee / pTx.vsize);
        var pNorm2 = pPending2 ? 0.15 : Math.min(Math.log2(Math.max(0, pRate2) + 1) / 6, 1.0);
        var pJit2 = txidJitter(pTx.txid);
        var pH2 = pNotable2
            ? (18 + pNorm2 * 10 + pJit2 * 3)
            : (pPending2 ? (5 + pJit2 * 2) : (3 + pNorm2 * 14 + pJit2 * 3));
        var pCol;
        if (!pTx.first_seen && unknownCols > 0) {
            pCol = Math.floor(((txidSkylineOffset(pTx.txid) + 1) * 0.5) * unknownCols);
            if (pCol >= unknownCols) pCol = unknownCols - 1;
        } else {
            var baseCol = Math.min(usableCols - 1, Math.floor((cumH * usableCols) / denomH));
            var jitter = Math.round(txidSkylineOffset(pTx.txid) * localSpreadCols);
            pCol = baseCol + jitter;
            if (pCol < knownStartCol) pCol = knownStartCol + (knownStartCol - pCol);
            if (pCol >= usableCols) pCol = (usableCols - 1) - (pCol - (usableCols - 1));
            pCol = Math.max(knownStartCol, Math.min(usableCols - 1, pCol));
            if (knownCols <= 1) pCol = Math.min(usableCols - 1, baseCol);
        }
        cumH += pH2;
        var bucket = colBuckets[pCol];
        if (!bucket) bucket = colBuckets[pCol] = [];
        bucket.push({ tx: pTx, col: pCol, yKey: pJit2 });
    }

    var planned = new Array(parsedCount);
    var pOut = 0;
    for (var bc = 0; bc < usableCols; bc++) {
        var bList = colBuckets[bc];
        if (!bList) continue;
        if (bList.length > 1) {
            bList.sort(function(a, b) { return a.yKey - b.yKey; });
        }
        for (var bi2 = 0; bi2 < bList.length; bi2++) {
            planned[pOut++] = bList[bi2];
        }
    }
    planned.length = pOut;

    var notableTxs = [];
    var idx = 0;
    var CHUNK_SIZE = 4000;
    _hb._pendingSnapshot = { seg: liveSeg, items: planned, nextIdx: 0, buildBlip: buildBlip };

    var finishSnapshot = function() {
        liveSeg._loadFillCount = 0;
        if (_hb._pendingSnapshot && _hb._pendingSnapshot.seg === liveSeg) {
            _hb._pendingSnapshot = null;
        }
        if (_hb._deferredDetails) _hb._deferredDetails.clear();
        maybeClearRemovedTxids(_hb);
        if (_hb._pendingFocusTxid) {
            var pendingFocus = _hb._pendingFocusTxid;
            _hb._pendingFocusTxid = null;
            hbV2FocusTx(pendingFocus);
        } else if (_hb._pinnedTxid) {
            _hb._pinnedBlip = liveSeg._blipsByTxid.get(_hb._pinnedTxid) || null;
        }
        if (notableTxs.length > 0 && window._notableFeed) {
            for (var ni = notableTxs.length - 1; ni >= 0; ni--) {
                window._notableFeed(notableTxs[ni]);
            }
        }
    };

    var placeChunk = function() {
        var s = getState();
        if (!s || s.timeline[s.timeline.length - 1] !== liveSeg || liveSeg.x_end !== null) {
            if (s) s._historyRaf = null;
            finishSnapshot();
            return;
        }
        var end = Math.min(idx + CHUNK_SIZE, planned.length);
        var span = Math.max(1, usableSpan);
        for (; idx < end; idx++) {
            var item = planned[idx];
            var tx = item.tx;
            if (!tx || !tx.txid) continue;
            if (s._removedTxids && s._removedTxids.has(tx.txid)) continue;
            if (s._deferredDetails && s._deferredDetails.has(tx.txid)) {
                var deferred = s._deferredDetails.get(tx.txid);
                if (deferred.value == null && tx.value != null) deferred.value = tx.value;
                if (!deferred.first_seen && tx.first_seen) deferred.first_seen = tx.first_seen;
                tx = deferred;
                s._deferredDetails.delete(tx.txid);
            }
            if (liveSeg._blipsByTxid.has(tx.txid)) continue;

            var gx = startGridX + item.col * 5;
            var sy = liveSeg._colHeights[gx] || 0;
            var xFrac = Math.min(1, Math.max(0, (gx - liveSeg.x_start) / span));
            // Use an uncorrelated per-brick delay (from txidSkylineOffset) plus a
            // gentle stackY offset so blue bricks rain down feathered like the
            // notables instead of launching in lockstep as one solid curved sheet.
            var rainJitter = ((txidSkylineOffset(tx.txid) + 1) * 0.5) * 0.55;
            var stackDelay = DROP_DURATION > 0 ? (sy * 0.0025 + rainJitter) : 0;
            var dropTs = dropOrigin + (DROP_DURATION > 0 ? xFrac * DROP_DURATION + stackDelay : 0);
            var blip = buildBlip(tx, gx, gx, sy, dropTs, !isResync);

            if (sy > 0 && sy + blip.brickH > softMaxStack) {
                var minZoneX = startGridX + (tx.first_seen ? knownStartCol : 0) * 5;
                var maxZoneX = startGridX + (tx.first_seen ? usableCols - 1 : Math.max(0, unknownCols - 1)) * 5;
                var bestX = gx;
                var bestY = sy;
                for (var step = 1; step <= 10; step++) {
                    var c1 = gx + step * 5;
                    if (c1 <= maxZoneX) {
                        var h1 = liveSeg._colHeights[c1] || 0;
                        if (h1 < bestY) { bestX = c1; bestY = h1; if (bestY + blip.brickH <= softMaxStack) break; }
                    }
                    var c2 = gx - step * 5;
                    if (c2 >= minZoneX) {
                        var h2 = liveSeg._colHeights[c2] || 0;
                        if (h2 < bestY) { bestX = c2; bestY = h2; if (bestY + blip.brickH <= softMaxStack) break; }
                    }
                }
                blip.x = bestX;
                blip.gridX = bestX;
                blip.stackY = bestY;
                blip.height = bestY + blip.brickH;
            }

            registerBlipInSeg(liveSeg, blip);

            if (blip.notableType) {
                notableTxs.push(tx);
            }
        }
        if (s._pendingSnapshot && s._pendingSnapshot.seg === liveSeg) {
            s._pendingSnapshot.nextIdx = idx;
        }
        if (idx < planned.length) {
            s._historyRaf = requestAnimationFrame(placeChunk);
        } else {
            s._historyRaf = null;
            finishSnapshot();
        }
    };

    placeChunk();
}

// ── Incremental v2 effect application ────────────────────────────────────

function findBlockSegment(_hb, hash, height) {
    if (!_hb || !_hb.timeline) return null;
    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        var seg = _hb.timeline[i];
        if (seg.type !== 'block') continue;
        if (hash && seg.hash === hash) return seg;
        if (height && seg.height === height) return seg;
    }
    return null;
}

export function hbV2ApplyEffect(effectJson) {
    var _hb = getState();
    if (!_hb) return;

    var effect;
    try {
        effect = typeof effectJson === 'string' ? JSON.parse(effectJson) : effectJson;
    } catch (err) {
        console.warn('[heartbeat] hbV2ApplyEffect parse error:', err);
        return;
    }
    if (!effect || !effect.type) return;

    if (!_hb._pendingAdds) _hb._pendingAdds = new Map();
    if (!_hb._hiddenTxMap) _hb._hiddenTxMap = new Map();
    if (!_hb._txBatchQueue) _hb._txBatchQueue = [];
    if (!_hb._removedTxids) _hb._removedTxids = new Set();
    if (!_hb._exactConfirmedByHash) _hb._exactConfirmedByHash = new Map();
    if (!_hb._pendingBlocks) _hb._pendingBlocks = [];

    _hb._lastSseEventTs = Date.now() / 1000;
    var liveSeg = getLiveFlatline(_hb);
    var isHidden = typeof document !== 'undefined' && document.hidden;

    switch (effect.type) {
        case 'membership_add': {
            var addTxid = effect.txid;
            if (!addTxid) return;
            _hb._removedTxids.delete(addTxid);
            if (liveSeg && liveSeg._blipsByTxid.has(addTxid)) return;
            var inFlightAdd = findInFlightBlip(_hb, addTxid);
            if (inFlightAdd && !inFlightAdd._removed) return;
            var baseTx = {
                txid: addTxid,
                first_seen: typeof effect.first_seen === 'number' ? effect.first_seen : Math.floor(Date.now() / 1000),
                fee: null,
                vsize: null,
                value: null,
                flags: 0,
                op_return_text: ''
            };
            // While the tab is hidden, rAF is paused and virtualX is frozen;
            // buffer arrivals in _hiddenTxMap so recoverTimeline spreads them
            // evenly across the elapsed gap instead of stacking a needle spike.
            if (isHidden) {
                if (!_hb._hiddenTxMap.has(addTxid)) {
                    _hb._hiddenTxMap.set(addTxid, baseTx);
                }
                return;
            }
            if (_hb._pendingAdds.has(addTxid)) return;
            _hb._pendingAdds.set(addTxid, {
                tx: baseTx,
                queuedAtMs: nowPerfMs()
            });
            return;
        }

        case 'tx_details': {
            var dtx = compactTupleToTx(effect.tx);
            if (!dtx || !dtx.txid) return;
            if (_hb._removedTxids.has(dtx.txid)) return;

            // 0. Buffered while tab was hidden -> upgrade metadata in place.
            if (_hb._hiddenTxMap.has(dtx.txid)) {
                var hiddenPrev = _hb._hiddenTxMap.get(dtx.txid);
                if (!dtx.first_seen && hiddenPrev && hiddenPrev.first_seen) {
                    dtx.first_seen = hiddenPrev.first_seen;
                }
                if (dtx.value == null && hiddenPrev && hiddenPrev.value != null) {
                    dtx.value = hiddenPrev.value;
                }
                _hb._hiddenTxMap.set(dtx.txid, dtx);
                return;
            }

            // 1. Still waiting inside the 120ms pending-add grace buffer -> promote immediately.
            if (_hb._pendingAdds.has(dtx.txid)) {
                var pendingEntry = _hb._pendingAdds.get(dtx.txid);
                _hb._pendingAdds.delete(dtx.txid);
                if (!dtx.first_seen && pendingEntry && pendingEntry.tx.first_seen) {
                    dtx.first_seen = pendingEntry.tx.first_seen;
                }
                if (dtx.value == null && pendingEntry && pendingEntry.tx.value != null) {
                    dtx.value = pendingEntry.tx.value;
                }
                if (isHidden) {
                    _hb._hiddenTxMap.set(dtx.txid, dtx);
                } else {
                    _hb._txBatchQueue.push(dtx);
                }
                return;
            }

            // 2. Already placed on the live flatline -> recolour & restack its single column in place.
            if (liveSeg && liveSeg._blipsByTxid.has(dtx.txid)) {
                var existingBlip = liveSeg._blipsByTxid.get(dtx.txid);
                var heightChanged = updateBlipMetadata(existingBlip, dtx);
                if (heightChanged) {
                    restackColumns(liveSeg, [existingBlip.gridX]);
                }
                if (anyNotable(notableFlags(dtx)) && window._notableFeed) {
                    window._notableFeed(dtx);
                }
                return;
            }

            // 3. Waiting in the RAF batch queue (drained after 120ms, before RAF ran) -> upgrade in place.
            for (var qi = _hb._txBatchQueue.length - 1; qi >= 0; qi--) {
                if (_hb._txBatchQueue[qi] && _hb._txBatchQueue[qi].txid === dtx.txid) {
                    var prevQ = _hb._txBatchQueue[qi];
                    if (!dtx.first_seen && prevQ.first_seen) dtx.first_seen = prevQ.first_seen;
                    if (dtx.value == null && prevQ.value != null) dtx.value = prevQ.value;
                    _hb._txBatchQueue[qi] = dtx;
                    return;
                }
            }

            // 4. In-flight inside chunked post-block carry, pending reveal, or active reveal -> upgrade in place.
            var inFlight = findInFlightBlip(_hb, dtx.txid);
            if (inFlight && !inFlight._removed) {
                updateBlipMetadata(inFlight, dtx);
                if (anyNotable(notableFlags(dtx)) && window._notableFeed) {
                    window._notableFeed(dtx);
                }
                return;
            }

            // 5. Snapshot fill, carry, or reveal in flight -> stash for when placement reaches it.
            if (_hb._historyRaf || _hb._carryRaf || _hb._reveal || _hb._pendingReveal) {
                if (!_hb._deferredDetails) _hb._deferredDetails = new Map();
                _hb._deferredDetails.set(dtx.txid, dtx);
            }
            return;
        }

        case 'membership_remove': {
            var remTxids = effect.txids;
            if (!Array.isArray(remTxids) || remTxids.length === 0) return;
            var remSet = new Set(remTxids);
            var dirtyCols = new Set();

            for (var ri = 0; ri < remTxids.length; ri++) {
                var rtxid = remTxids[ri];
                if (!rtxid) continue;
                _hb._removedTxids.add(rtxid);
                _hb._pendingAdds.delete(rtxid);
                if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(rtxid);
                if (_hb._deferredDetails) _hb._deferredDetails.delete(rtxid);

                if (liveSeg && liveSeg._blipsByTxid.has(rtxid)) {
                    var rBlip = liveSeg._blipsByTxid.get(rtxid);
                    liveSeg._blipsByTxid.delete(rtxid);
                    rBlip._removed = true;
                    removeBlipFromArray(liveSeg, rBlip);
                    dirtyCols.add(rBlip.gridX);
                    if (_hb.hoveredBlip === rBlip) _hb.hoveredBlip = null;
                    if (_hb._pinnedBlip === rBlip) _hb._pinnedBlip = null;
                }
            }

            purgeInFlightTxids(_hb, remTxids);

            if (_hb._txBatchQueue.length > 0) {
                _hb._txBatchQueue = _hb._txBatchQueue.filter(function(qtx) {
                    return !qtx || !remSet.has(qtx.txid);
                });
            }

            if (liveSeg && dirtyCols.size > 0) {
                restackColumns(liveSeg, dirtyCols);
            }
            return;
        }

        case 'block_observed': {
            if (!effect.hash) return;
            var exists = false;
            for (var pi = 0; pi < _hb._pendingBlocks.length; pi++) {
                if (_hb._pendingBlocks[pi].hash === effect.hash) { exists = true; break; }
            }
            if (!exists) {
                _hb._pendingBlocks.push({
                    hash: effect.hash,
                    observed_at: effect.observed_at || Math.floor(Date.now() / 1000),
                    height: effect.height || null
                });
            }
            return;
        }

        case 'block_details': {
            if (effect.hash) {
                _hb._pendingBlocks = _hb._pendingBlocks.filter(function(pb) {
                    return pb.hash !== effect.hash;
                });
            }
            if (!effect.height) return;
            if (_hb._lastCuedBlockHeight !== effect.height) {
                _hb._lastCuedBlockHeight = effect.height;
                playBlockCue();
            }
            if (typeof document !== 'undefined' && document.hidden) {
                if (!_hb._blockQueue) _hb._blockQueue = [];
                _hb._blockQueue.push(effect);
                _hb._queuedBlockTime = Math.floor(Date.now() / 1000);
                return;
            }
            processLiveBlock(effect);
            return;
        }

        case 'block_inclusion': {
            if (!effect.hash || !Array.isArray(effect.confirmed_txids)) return;
            var confSet = new Set(effect.confirmed_txids);
            _hb._exactConfirmedByHash.set(effect.hash, {
                height: effect.height || 0,
                set: confSet
            });
            // Keep confirmation cache bounded to recent blocks.
            if (_hb._exactConfirmedByHash.size > 32) {
                var oldestKey = _hb._exactConfirmedByHash.keys().next().value;
                _hb._exactConfirmedByHash.delete(oldestKey);
            }

            // 1. If a reveal is armed for THIS block and hasn't shattered yet, partition it exactly.
            var sameReveal = _hb._reveal && (
                (effect.hash && _hb._reveal.hash === effect.hash) ||
                (effect.height && _hb._reveal.height === effect.height)
            );
            if (sameReveal && !_hb._reveal.harvested) {
                applyExactHarvest(_hb._reveal, confSet);
                for (var ci0 = 0; ci0 < effect.confirmed_txids.length; ci0++) {
                    var ctxid0 = effect.confirmed_txids[ci0];
                    if (!ctxid0) continue;
                    _hb._pendingAdds.delete(ctxid0);
                    if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(ctxid0);
                    if (_hb._deferredDetails) _hb._deferredDetails.delete(ctxid0);
                }
                if (_hb._txBatchQueue.length > 0) {
                    _hb._txBatchQueue = _hb._txBatchQueue.filter(function(qtx) {
                        return !qtx || !confSet.has(qtx.txid);
                    });
                }
                return;
            }

            // 2. If a reveal for THIS block is mid-harvest (harvested but leftovers not yet laid),
            // shatter any newly identified confirmed bricks from leftovers into the spike now.
            if (sameReveal && _hb._reveal.harvested && !_hb._reveal.laid) {
                var rv = _hb._reveal;
                if (!rv._revealByTxid) rebuildRevealIndex(rv);
                var rvNow = Date.now() / 1000;
                var rvDirty = false;
                for (var rci = 0; rci < effect.confirmed_txids.length; rci++) {
                    var rctxid = effect.confirmed_txids[rci];
                    if (!rctxid) continue;
                    _hb._removedTxids.add(rctxid);
                    _hb._pendingAdds.delete(rctxid);
                    if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(rctxid);
                    if (_hb._deferredDetails) _hb._deferredDetails.delete(rctxid);
                    var rvBlip = rv._revealByTxid ? rv._revealByTxid.get(rctxid) : null;
                    if (rvBlip && !rvBlip._removed && rvBlip.fadeStart === 0) {
                        rvBlip.fadeStart = rvNow;
                        rvBlip._confirmedHeight = rv.height || effect.height || 0;
                        rvBlip.absorbTargetX = rv.absorbX;
                        rvBlip.absorbOriginX = rvBlip.x;
                        rvBlip.particles = [];
                        for (var rpi = 0; rpi < (rv.maxParts || 2); rpi++) {
                            rvBlip.particles.push({
                                offsetX: (Math.random() - 0.5) * 8,
                                offsetY: Math.random() * (rvBlip.height || 10) * 0.8,
                                speed: 0.7 + Math.random() * 0.6,
                                arcHeight: 20 + Math.random() * 60,
                                delay: Math.random() * 0.3,
                                size: 1.5 + Math.random() * 2.5
                            });
                        }
                        rv.confirmed.push(rvBlip);
                        rvDirty = true;
                    }
                }
                if (rvDirty) {
                    rv.leftovers = rv.leftovers.filter(function(b) {
                        return b && !b._removed && b.fadeStart === 0 && (!b.txid || !confSet.has(b.txid));
                    });
                    rebuildRevealIndex(rv);
                }
                if (_hb._txBatchQueue.length > 0) {
                    _hb._txBatchQueue = _hb._txBatchQueue.filter(function(qtx) {
                        return !qtx || !confSet.has(qtx.txid);
                    });
                }
                return;
            }

            // 3. Server ordering: block_inclusion often arrives BEFORE block_details.
            // If no block segment exists on the timeline yet AND the viewer is reveal-eligible,
            // keep the confirmed bricks intact (fadeStart === 0) and tag them with
            // _pendingInclusionHash so block_details puts them into reveal.confirmed and the
            // reveal's harvest beat visibly shatters them into the new spike.
            var blockSeg = findBlockSegment(_hb, effect.hash, effect.height);
            var revealEligible = !isHidden
                && _hb._revealEnabled !== false
                && _hb.autoFollow && !_hb._pinnedTxid
                && !_hb.paused && !_hb.isDragging && !_hb._pinching
                && _hb.width > 0 && _hb.zoom > 0
                && _hb.zoom <= REVEAL_MAX_ENTRY_ZOOM;

            if (!blockSeg && revealEligible) {
                for (var pci = 0; pci < effect.confirmed_txids.length; pci++) {
                    var pctxid = effect.confirmed_txids[pci];
                    if (!pctxid) continue;
                    _hb._pendingAdds.delete(pctxid);
                    if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(pctxid);
                    if (_hb._deferredDetails) _hb._deferredDetails.delete(pctxid);

                    var pBlip = (liveSeg && liveSeg._blipsByTxid.has(pctxid))
                        ? liveSeg._blipsByTxid.get(pctxid)
                        : findInFlightBlip(_hb, pctxid);
                    if (pBlip && !pBlip._removed) {
                        pBlip._pendingInclusionHash = effect.hash;
                        pBlip._confirmedHeight = effect.height || 0;
                    }
                }
                if (_hb._txBatchQueue.length > 0) {
                    _hb._txBatchQueue = _hb._txBatchQueue.filter(function(qtx) {
                        return !qtx || !confSet.has(qtx.txid);
                    });
                }
                return;
            }

            // 4. Instant mode / inspecting / reveal disabled / block segment already exists / hidden:
            // tombstone confirmed txids so neither _pendingSnapshot nor placeCarriedLeftovers can
            // resurrect them, purge in-flight bricks, and shatter live bricks immediately.
            var absorbX = blockSeg ? (blockSeg.x_start + blockSeg.x_end) / 2 : _hb.virtualX;
            var absorbNow = Date.now() / 1000;
            var zoom = _hb.zoom || 1;
            var maxPartsPerBlip = (zoom < 0.5 || confSet.size > 1000) ? 1
                : (zoom < 1.0 || confSet.size > 500) ? 2 : 3;
            var viewLeft = _hb.viewOffset || 0;
            var viewRight = viewLeft + (_hb.width || 800) / zoom;
            var incDirtyCols = new Set();

            for (var ci = 0; ci < effect.confirmed_txids.length; ci++) {
                var ctxid = effect.confirmed_txids[ci];
                if (!ctxid) continue;
                _hb._removedTxids.add(ctxid);
                _hb._pendingAdds.delete(ctxid);
                if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(ctxid);
                if (_hb._deferredDetails) _hb._deferredDetails.delete(ctxid);

                if (liveSeg && liveSeg._blipsByTxid.has(ctxid)) {
                    var cBlip = liveSeg._blipsByTxid.get(ctxid);
                    liveSeg._blipsByTxid.delete(ctxid);
                    if (cBlip.fadeStart === 0) {
                        cBlip.fadeStart = absorbNow;
                        cBlip._confirmedHeight = effect.height || 0;
                        cBlip.absorbTargetX = absorbX;
                        cBlip.absorbOriginX = cBlip.x;
                        cBlip.particles = [];
                        if (cBlip.x >= viewLeft - 50 && cBlip.x <= viewRight + 50) {
                            for (var pp = 0; pp < maxPartsPerBlip; pp++) {
                                cBlip.particles.push({
                                    offsetX: (Math.random() - 0.5) * 8,
                                    offsetY: Math.random() * (cBlip.height || 10) * 0.8,
                                    speed: 0.7 + Math.random() * 0.6,
                                    arcHeight: 20 + Math.random() * 60,
                                    delay: Math.random() * 0.3,
                                    size: 1.5 + Math.random() * 2.5
                                });
                            }
                        }
                        incDirtyCols.add(cBlip.gridX);
                    }
                }
            }

            purgeInFlightTxids(_hb, effect.confirmed_txids);

            if (_hb._txBatchQueue.length > 0) {
                _hb._txBatchQueue = _hb._txBatchQueue.filter(function(qtx) {
                    return !qtx || !confSet.has(qtx.txid);
                });
            }

            if (liveSeg && incDirtyCols.size > 0) {
                restackColumns(liveSeg, incDirtyCols);
            }
            return;
        }

        case 'block_disconnected': {
            if (!effect.hash) return;
            if (_hb._reveal && _hb._reveal.hash === effect.hash && window._hbEndReveal) {
                window._hbEndReveal();
            }
            _hb._exactConfirmedByHash.delete(effect.hash);
            if (liveSeg && liveSeg.blips) {
                for (var pdi = 0; pdi < liveSeg.blips.length; pdi++) {
                    var pdb = liveSeg.blips[pdi];
                    if (pdb && pdb._pendingInclusionHash === effect.hash) {
                        pdb._pendingInclusionHash = null;
                        pdb._confirmedHeight = 0;
                    }
                }
            }
            _hb._pendingBlocks = _hb._pendingBlocks.filter(function(pb) {
                return pb.hash !== effect.hash;
            });
            if (Array.isArray(_hb.recentBlocks)) {
                _hb.recentBlocks = _hb.recentBlocks.filter(function(rb) {
                    return rb.hash !== effect.hash;
                });
            }
            for (var bi = _hb.timeline.length - 1; bi >= 0; bi--) {
                var seg = _hb.timeline[bi];
                if (seg.type === 'block' && seg.hash === effect.hash) {
                    if (_hb.hoveredBlock === seg) _hb.hoveredBlock = null;
                    _hb.timeline.splice(bi, 1);
                    // Bridge adjacent flatlines cleanly without moving any existing bricks.
                    if (bi > 0 && bi < _hb.timeline.length &&
                        _hb.timeline[bi - 1].type === 'flatline' &&
                        _hb.timeline[bi].type === 'flatline') {
                        _hb.timeline[bi - 1].x_end = _hb.timeline[bi].x_start;
                    }
                    break;
                }
            }
            // Refresh lastBlockTime from the new latest remaining block segment.
            var latestTs = 0;
            for (var li = _hb.timeline.length - 1; li >= 0; li--) {
                if (_hb.timeline[li].type === 'block' && _hb.timeline[li].timestamp) {
                    latestTs = _hb.timeline[li].timestamp;
                    break;
                }
            }
            if (latestTs > 0) _hb.lastBlockTime = latestTs;
            return;
        }

        case 'coverage_changed': {
            _hb._coverage = effect.coverage || { status: 'unknown' };
            return;
        }

        case 'metrics_updated': {
            var nm = effect.node_mempool || null;
            _hb._nodeMempool = nm;
            if (nm && typeof nm === 'object') {
                if (typeof nm.vbytes === 'number') {
                    _hb.mempoolMB = nm.vbytes / 1000000;
                } else if (typeof nm.vsize_bytes === 'number') {
                    _hb.mempoolMB = nm.vsize_bytes / 1000000;
                } else if (typeof nm.bytes === 'number') {
                    _hb.mempoolMB = nm.bytes / 1000000;
                } else if (typeof nm.mempool_mb === 'number') {
                    _hb.mempoolMB = nm.mempool_mb;
                }
                if (typeof nm.min_fee_rate === 'number') {
                    _hb.mempoolMinFee = nm.min_fee_rate;
                }
            }
            return;
        }
    }
}

// ── Navigation & connection-state entry points (v2) ──────────────────────

function lookupBlipOnTimeline(_hb, rawTxid) {
    if (!_hb || !rawTxid || !_hb.timeline) return null;
    var needle = String(rawTxid).trim().toLowerCase();
    if (!needle) return null;

    // 1. Exact O(1) lookup in the live segment first.
    var liveSeg = getLiveFlatline(_hb);
    if (liveSeg && liveSeg._blipsByTxid) {
        var exact = liveSeg._blipsByTxid.get(needle);
        if (exact && !exact._removed) return exact;
        if (needle.length < 64) {
            var liveMatch = null;
            liveSeg._blipsByTxid.forEach(function(b, k) {
                if (!liveMatch && !b._removed && k.toLowerCase().indexOf(needle) === 0) {
                    liveMatch = b;
                }
            });
            if (liveMatch) return liveMatch;
        }
    }

    // 2. Fallback scan across all flatline segments (newest first).
    for (var si = _hb.timeline.length - 1; si >= 0; si--) {
        var seg = _hb.timeline[si];
        if (seg.type !== 'flatline') continue;
        if (seg._blipsByTxid && seg._blipsByTxid.has(needle)) {
            var hit = seg._blipsByTxid.get(needle);
            if (hit && !hit._removed) return hit;
        }
        if (!seg.blips) continue;
        for (var bi = 0; bi < seg.blips.length; bi++) {
            var blip = seg.blips[bi];
            if (!blip || blip._removed || !blip.txid) continue;
            var low = blip.txid.toLowerCase();
            if (needle.length >= 64 ? low === needle : low.indexOf(needle) === 0) {
                return blip;
            }
        }
    }
    return null;
}

export function hbV2FocusTx(txid) {
    var _hb = getState();
    if (!_hb || !txid) return false;

    var blip = lookupBlipOnTimeline(_hb, txid);
    if (!blip) {
        if (_hb._historyRaf) {
            _hb._pendingFocusTxid = txid;
            _hb._pinnedTxid = txid;
            return true;
        }
        return false;
    }
    _hb._pendingFocusTxid = null;

    // End any active reveal/intro so the camera doesn't fight the focus target.
    if (_hb._reveal && window._hbEndReveal) {
        window._hbEndReveal();
        blip = lookupBlipOnTimeline(_hb, blip.txid) || blip;
    }
    _hb._intro = null;

    _hb.autoFollow = false;
    _hb.paused = false;
    _hb.zoom = Math.max(_hb.zoom || 1, 3.5);
    _hb.viewOffset = blip.x - ((_hb.width || 800) * 0.5) / _hb.zoom;
    _hb.viewOffsetY = 0;
    _hb._pinnedTxid = blip.txid;
    _hb._pinnedBlip = blip;
    _hb._searchHighlight = {
        x: blip.x,
        gridX: blip.gridX,
        start: Date.now() / 1000,
        duration: 5.0
    };
    return true;
}

export function hbV2SetPinnedTx(txidOrNull) {
    var _hb = getState();
    if (!_hb) return;
    _hb._pinnedTxid = txidOrNull || null;
    if (!_hb._pinnedTxid) {
        _hb._pinnedBlip = null;
        _hb._pendingFocusTxid = null;
        return;
    }
    _hb._pinnedBlip = lookupBlipOnTimeline(_hb, _hb._pinnedTxid);
}

export function hbV2FocusBlockHeight(height) {
    var _hb = getState();
    if (!_hb || !_hb.timeline || !height) return false;
    var target = null;
    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        var seg = _hb.timeline[i];
        if (seg.type === 'block' && seg.height === height) {
            target = seg;
            break;
        }
    }
    if (!target) return false;

    if (_hb._reveal && window._hbEndReveal) window._hbEndReveal();
    _hb._intro = null;
    _hb.autoFollow = false;
    _hb.zoom = Math.max(_hb.zoom || 1, 1.5);
    var mid = (target.x_start + target.x_end) / 2;
    _hb.viewOffset = mid - ((_hb.width || 800) * 0.5) / _hb.zoom;
    _hb.viewOffsetY = 0;
    _hb.hoveredBlock = target;
    return true;
}

export function hbV2SetConnectionState(state) {
    var _hb = getState();
    if (!_hb) return;
    var next = state || 'live';
    _hb._v2ConnState = next;
    if (next === 'disconnected') {
        if (!_hb._sseDisconnected) {
            _hb._sseDisconnected = true;
            _hb._sseDisconnectedSince = Date.now() / 1000;
        }
    } else {
        _hb._sseDisconnected = false;
        _hb._sseDisconnectedSince = 0;
    }
}

// ── Live block processing & cinematic reveal ─────────────────────────────

export function processLiveBlock(block) {
    var _hb = getState();
    if (!_hb) return;

    // Ignore a re-broadcast of the SAME block during an active reveal; finalize
    // the reveal first if a genuinely NEW block arrives mid-reveal.
    if (_hb._reveal && block && block.height) {
        if (_hb._reveal.height === block.height) return;
        if (window._hbEndReveal) window._hbEndReveal();
    }

    _hb._backlogSpreadPx = 0;
    _hb._backlogSpreadStart = 0;

    // Inspection continuity rule: never hijack an inspecting or pinned viewer's camera.
    var revealEligible = _hb._revealEnabled !== false
        && _hb.autoFollow && !_hb._pinnedTxid
        && !_hb.paused && !_hb.isDragging && !_hb._pinching
        && _hb.width > 0 && _hb.zoom > 0
        && _hb.zoom <= REVEAL_MAX_ENTRY_ZOOM;

    // Collapse any empty processing gap past the rightmost active brick.
    var collapsedPx = 0;
    var liveSeg = getLiveFlatline(_hb);
    if (liveSeg && liveSeg.blips.length > 0) {
        var rightmost = liveSeg.x_start;
        for (var ri = 0; ri < liveSeg.blips.length; ri++) {
            var rb = liveSeg.blips[ri];
            if (!rb._removed && rb.fadeStart === 0 && rb.x > rightmost) {
                rightmost = rb.x;
            }
        }
        var closeAt = rightmost + 5;
        if (closeAt < _hb.virtualX) {
            collapsedPx = _hb.virtualX - closeAt;
            _hb.virtualX = closeAt;
        }
    }

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
        hash: block.hash || null,
        height: block.height,
        timestamp: blockTs,
        tx_count: block.tx_count || block.confirmed_count || 0,
        total_fees: block.total_fees || 0,
        size: block.size || 0,
        weight: block.weight || 0,
        inter_block_seconds: inter
    }]);

    _hb._harvestLive = true;
    _hb._revealPending = revealEligible;
    if (typeof window.pushHeartbeatBlocks === 'function') {
        window.pushHeartbeatBlocks(blockJson, false);
    }
    _hb._revealPending = false;

    var pending = _hb._pendingReveal;
    _hb._pendingReveal = null;
    if (revealEligible && pending) {
        setupBlockReveal(_hb, pending);
    } else if (collapsedPx > 40) {
        var newSeg = getLiveFlatline(_hb);
        if (newSeg) {
            _hb.virtualX += collapsedPx;
            _hb._backlogSpreadPx = collapsedPx;
            _hb._backlogSpreadStart = Date.now() / 1000;
        }
    }

    _hb._blockMiningOverlay = false;
    if (_hb._miningDelayTimeout) { clearTimeout(_hb._miningDelayTimeout); _hb._miningDelayTimeout = null; }
    if (_hb._miningTimeout) { clearTimeout(_hb._miningTimeout); _hb._miningTimeout = null; }
    var miningEl = document.getElementById('heartbeat-mining-overlay');
    if (miningEl) miningEl.classList.add('hidden');

    if (window.heartbeatFlash) window.heartbeatFlash();
    if (window.heartbeatPulse) window.heartbeatPulse();

    if (window.renderRhythmStrip && window.getHeartbeatRecentBlocks) {
        window.renderRhythmStrip('rhythm-strip-canvas', window.getHeartbeatRecentBlocks());
    }
}

// DEV: fire a fake block on demand on localhost.
window._hbFakeBlock = function(txCount) {
    if (typeof location === 'undefined' || (location.hostname !== 'localhost' && location.hostname !== '127.0.0.1')) {
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
        hash: 'devfake-' + Date.now(),
        height: (h || _hb.blockHeight || 900000) + 1,
        timestamp: Math.floor(Date.now() / 1000),
        tx_count: tc,
        total_fees: 12000000,
        size: 1500000,
        weight: 3990000
    });
};

// Re-partition an armed (pre-shatter) reveal's active bricks by exact confirmed txids.
function applyExactHarvest(r, blockSet) {
    if (!r || r.harvested) return;
    var all = r.confirmed.concat(r.leftovers);
    var conf = [], left = [];
    for (var i = 0; i < all.length; i++) {
        var b = all[i];
        if (!b || b._removed) continue;
        if (b.txid && blockSet.has(b.txid)) {
            b._pendingInclusionHash = null;
            conf.push(b);
        } else {
            left.push(b);
        }
    }
    r.confirmed = conf;
    r.leftovers = left;
    r._exact = true;
    rebuildRevealIndex(r);
}

function setupBlockReveal(_hb, pr) {
    var spikeSeg = pr.spikeSeg;
    if (!spikeSeg || !spikeSeg.points) return;
    if (_hb._lastRevealHeight === spikeSeg.height) return;
    _hb._lastRevealHeight = spikeSeg.height;

    var peakIdx = 0, peakY = 0;
    for (var p = 0; p < spikeSeg.points.length; p++) {
        if (spikeSeg.points[p] < peakY) { peakY = spikeSeg.points[p]; peakIdx = p; }
    }
    _hb._momentumVel = 0;
    _hb.viewOffsetY = 0;
    _hb._intro = null;
    var wallNow = Date.now() / 1000;
    _hb._reveal = {
        phase: 'form',
        start: wallNow,
        revealWallStart: wallNow,
        hash: spikeSeg.hash || null,
        height: spikeSeg.height,
        entryZoom: _hb.zoom,
        spikeSeg: spikeSeg,
        spikeVX: spikeSeg.x_start + peakIdx * POINT_WIDTH,
        oldSeg: pr.oldSeg,
        newSeg: pr.newSeg,
        leftovers: pr.leftovers,
        confirmed: pr.confirmed,
        absorbX: pr.absorbX,
        maxParts: pr.maxParts,
        blockFees: spikeSeg.total_fees || 0,
        blockTxCount: spikeSeg.tx_count || 0,
        blockColor: spikeSeg.color || null,
        _bannerBlockSet: false,
        _bannerRemainSet: false,
        harvested: false,
        laid: false,
        unfurlSecs: 0,
        fitZoom: 0,
        width: 0,
        fromZoom: undefined,
        fromOffset: undefined,
        _deferStrike: false,
        _revealByTxid: null
    };
    rebuildRevealIndex(_hb._reveal);

    var spikeVX = _hb._reveal.spikeVX;
    var workingZoom = Math.max(_hb.zoom, 1);
    var spikeScreenX = (spikeVX - _hb.viewOffset) * _hb.zoom;
    var headScreenX = _hb.width * HEAD_POSITION_FRAC;
    var nearHead = spikeScreenX > 0 && spikeScreenX < _hb.width
        && Math.abs(spikeScreenX - headScreenX) < _hb.width * 0.35;
    var zoomingIn = workingZoom / _hb.zoom > 1.25;
    if (nearHead && !zoomingIn) {
        spikeSeg._strikeStart = Date.now() / 1000;
    } else {
        _hb._reveal._deferStrike = true;
    }

    // If exact block_inclusion arrived before block_details, partition immediately.
    if (spikeSeg.hash && _hb._exactConfirmedByHash && _hb._exactConfirmedByHash.has(spikeSeg.hash)) {
        applyExactHarvest(_hb._reveal, _hb._exactConfirmedByHash.get(spikeSeg.hash).set);
    }
}

// Legacy no-op stub: WASM HeartbeatClientController owns the single v2 EventSource.
export function connectOwnFeed() {}

// Widen the live flatline to reflect wall-clock time since `blockTs`.
export function fastForwardLiveFlatline(blockTs) {
    var _hb = getState();
    if (!_hb || !blockTs) return;
    var liveSeg = getLiveFlatline(_hb);
    if (!liveSeg) return;
    var age = Math.max(0, Math.min(Date.now() / 1000 - blockTs, 1800));
    var wanted = liveSeg.x_start + age * FLATLINE_PX_PER_SEC;
    if (wanted > _hb.virtualX) _hb.virtualX = wanted;
}

// ── Per-frame RAF tx flush + pending-fee grace drain ─────────────────────

// Spread buffered hidden-tab / occlusion-stall txs evenly across [gapStart, gapEnd]
// so returning from another tab fills the elapsed stretch naturally instead of
// stacking a skyscraper needle in a single frozen column.
export function drainHiddenTxsAcrossGap(gapStart, gapEnd, windowStartSec, windowEndSec) {
    var _hb = getState();
    if (!_hb) return;
    var liveSeg = getLiveFlatline(_hb);
    if (!liveSeg) return;

    var buffered = [];
    var seen = new Set();
    var pushUnique = function(tx) {
        if (!tx || !tx.txid || seen.has(tx.txid)) return;
        if (_hb._removedTxids && _hb._removedTxids.has(tx.txid)) return;
        if (liveSeg._blipsByTxid.has(tx.txid)) return;
        seen.add(tx.txid);
        buffered.push(tx);
    };

    if (_hb._hiddenTxMap && _hb._hiddenTxMap.size > 0) {
        _hb._hiddenTxMap.forEach(function(tx) { pushUnique(tx); });
        _hb._hiddenTxMap.clear();
    }
    if (_hb._pendingAdds && _hb._pendingAdds.size > 0) {
        _hb._pendingAdds.forEach(function(entry) {
            if (entry && entry.tx) pushUnique(entry.tx);
        });
        _hb._pendingAdds.clear();
    }
    if (_hb._txBatchQueue && _hb._txBatchQueue.length > 0) {
        for (var qi = 0; qi < _hb._txBatchQueue.length; qi++) {
            pushUnique(_hb._txBatchQueue[qi]);
        }
        _hb._txBatchQueue = [];
    }

    if (buffered.length === 0) return;

    buffered.sort(function(a, b) {
        var fa = a.first_seen || 0;
        var fb = b.first_seen || 0;
        if (fa !== fb) return fa - fb;
        return a.txid < b.txid ? -1 : (a.txid > b.txid ? 1 : 0);
    });

    var minGridX = Math.ceil(liveSeg.x_start / 5) * 5;
    var startGx = Math.max(minGridX, Math.floor(gapStart / 5) * 5);
    var endGx = Math.max(startGx, Math.floor(gapEnd / 5) * 5);
    var numCols = Math.max(1, Math.floor((endGx - startGx) / 5) + 1);
    var maxStack = (_hb.height || 400) * 0.35;
    var nowSec = Date.now() / 1000;

    // Position by each tx's actual first_seen timestamp anchored to the real
    // elapsed gap window [windowStartSec, windowEndSec] so one outlier older
    // timestamp can never squish all recent arrivals into the rightmost column.
    var hasTimeSpan = typeof windowStartSec === 'number' && typeof windowEndSec === 'number'
        && windowEndSec > windowStartSec;
    var minTs = hasTimeSpan ? windowStartSec : Infinity;
    var maxTs = hasTimeSpan ? windowEndSec : 0;
    if (!hasTimeSpan) {
        for (var ti = 0; ti < buffered.length; ti++) {
            var fts = buffered[ti].first_seen || 0;
            if (fts > 0) {
                if (fts < minTs) minTs = fts;
                if (fts > maxTs) maxTs = fts;
            }
        }
        hasTimeSpan = minTs < Infinity && maxTs > minTs;
    }
    var tsSpan = hasTimeSpan ? (maxTs - minTs) : 1;

    for (var i = 0; i < buffered.length; i++) {
        var tx = buffered[i];
        var colIdx;
        if (hasTimeSpan && tx.first_seen > 0) {
            var clampedTs = Math.max(minTs, Math.min(maxTs, tx.first_seen));
            var frac = (clampedTs - minTs) / tsSpan;
            var base = Math.round(frac * (numCols - 1));
            var jitter = numCols > 4 ? Math.round(txidSkylineOffset(tx.txid) * 2) : 0;
            colIdx = Math.max(0, Math.min(numCols - 1, base + jitter));
        } else {
            colIdx = Math.floor((i * numCols) / buffered.length);
        }
        var gx = startGx + colIdx * 5;
        var sy = liveSeg._colHeights[gx] || 0;
        var blip = buildBlip(tx, gx, gx, sy, nowSec, false);

        if (sy > 0 && sy + blip.brickH > maxStack) {
            var bestX = gx;
            var bestY = sy;
            for (var back = 1; back <= 20; back++) {
                var tryX = gx - back * 5;
                if (tryX < minGridX) break;
                var tryH = liveSeg._colHeights[tryX] || 0;
                if (tryH + blip.brickH <= maxStack) {
                    bestX = tryX;
                    bestY = tryH;
                    break;
                }
                if (tryH < bestY) {
                    bestX = tryX;
                    bestY = tryH;
                }
            }
            blip.x = bestX;
            blip.gridX = bestX;
            blip.stackY = bestY;
            blip.height = bestY + blip.brickH;
        }

        registerBlipInSeg(liveSeg, blip);
        if (blip.notableType && window._notableFeed) {
            window._notableFeed(tx);
        }
    }
}

export function flushTxBatch(nowMsOverride) {
    var _hb = getState();
    if (!_hb) return;
    // Never flush into a frozen virtualX while the tab is hidden (browsers
    // throttle rAF while hidden, which would stack arrivals into one column).
    if (nowMsOverride === undefined && typeof document !== 'undefined' && document.hidden) return;
    var liveSeg = getLiveFlatline(_hb);
    if (!liveSeg) return;

    // 1. Drain any pending membership_add older than PENDING_GRACE_MS (120ms)
    // as a pending-fee slate brick so live drops never stall if getmempoolentry lags.
    var nowMs = nowMsOverride !== undefined ? nowMsOverride : nowPerfMs();
    if (_hb._pendingAdds && _hb._pendingAdds.size > 0) {
        if (!_hb._txBatchQueue) _hb._txBatchQueue = [];
        var expired = [];
        _hb._pendingAdds.forEach(function(entry, txid) {
            if (nowMs - entry.queuedAtMs >= PENDING_GRACE_MS) {
                expired.push(txid);
            }
        });
        for (var ei = 0; ei < expired.length; ei++) {
            var etxid = expired[ei];
            var ent = _hb._pendingAdds.get(etxid);
            _hb._pendingAdds.delete(etxid);
            if (ent && ent.tx) {
                _hb._txBatchQueue.push(ent.tx);
            }
        }
    }

    if (!_hb._txBatchQueue || _hb._txBatchQueue.length === 0) {
        maybeClearRemovedTxids(_hb, 5000);
        return;
    }

    var batch = _hb._txBatchQueue;
    _hb._txBatchQueue = [];

    // virtualX is advanced ONLY by drawFrame (single smooth 60fps clock).
    // Never jump _hb.virtualX here; spread live bursts gently behind the head.
    var maxStack = (_hb.height || 400) * 0.35;
    var nowSec = Date.now() / 1000;
    var minGridX = Math.ceil(liveSeg.x_start / 5) * 5;
    var headGridX = Math.max(minGridX, Math.floor(_hb.virtualX / 5) * 5);
    var postCarryMinX = liveSeg._carryEndX ? (liveSeg._carryEndX + 5) : minGridX;
    var liveMinX = Math.min(headGridX, Math.max(minGridX, postCarryMinX));
    var flatlineWidth = Math.max(0, _hb.virtualX - liveSeg.x_start);
    var visibleVirtW = (_hb.width || 800) / Math.max(_hb.zoom || 1, 0.1);
    var burstFactor = Math.min(batch.length / 20, 4);
    var spread = Math.min(flatlineWidth * 0.6, visibleVirtW * 0.35, 35 + burstFactor * 80);

    if (_hb._backlogSpreadPx > 0) {
        var BACKLOG_SPREAD_SECS = 2.5;
        var bsElapsed = nowSec - (_hb._backlogSpreadStart || 0);
        if (bsElapsed < BACKLOG_SPREAD_SECS) {
            var bsWide = Math.min(_hb._backlogSpreadPx * (1 - bsElapsed / BACKLOG_SPREAD_SECS), flatlineWidth);
            if (bsWide > spread) spread = bsWide;
        } else {
            _hb._backlogSpreadPx = 0;
        }
    }
    var spreadCols = Math.max(1, Math.floor(spread / 5));
    var removedSet = _hb._removedTxids;

    for (var i = 0; i < batch.length; i++) {
        var tx = batch[i];
        if (!tx || !tx.txid) continue;
        if (removedSet && removedSet.has(tx.txid)) continue;

        // If already placed, update in place instead of duplicating.
        if (liveSeg._blipsByTxid.has(tx.txid)) {
            var existing = liveSeg._blipsByTxid.get(tx.txid);
            if (updateBlipMetadata(existing, tx)) {
                restackColumns(liveSeg, [existing.gridX]);
            }
            continue;
        }

        // If Chrome held SSE deltas while a background tab was frozen and
        // released them after visibilitychange already advanced virtualX,
        // place aged catch-up txs back at their true real-time column
        // (headGridX - age * 5px/s) so they fill the elapsed stretch instead
        // of leaving a blank gap and piling at the tip. Never stack them back
        // on top of pre-block carried columns (clamped to >= liveMinX).
        var ageSec = (tx.first_seen > 0) ? (nowSec - tx.first_seen) : 0;
        var gridX;
        var animateDrop = true;
        if (ageSec > 2.5 && ageSec <= 300) {
            var ageCols = Math.round((ageSec * FLATLINE_PX_PER_SEC) / 5);
            var catchJit = Math.round(txidSkylineOffset(tx.txid) * 1.5);
            gridX = Math.max(liveMinX, Math.min(headGridX, headGridX - (ageCols + catchJit) * 5));
            animateDrop = ageSec < 4.0;
        } else {
            // Deterministic trailing spread behind the live head: calm flushes drop
            // right near the head; bursts fan across trailing columns behind headGridX.
            var offsetCols = spreadCols > 1 ? Math.floor(txidJitter(tx.txid) * spreadCols) : 0;
            gridX = Math.max(liveMinX, headGridX - offsetCols * 5);
        }
        var sy = liveSeg._colHeights[gridX] || 0;
        var blip = buildBlip(tx, gridX, gridX, sy, nowSec, animateDrop);

        if (sy > 0 && sy + blip.brickH > maxStack) {
            var bestX = gridX;
            var bestY = sy;
            var maxBack = Math.max(20, spreadCols);
            for (var back = 1; back <= maxBack; back++) {
                var tryX = gridX - back * 5;
                if (tryX < liveMinX) break;
                var tryH = liveSeg._colHeights[tryX] || 0;
                if (tryH + blip.brickH <= maxStack) {
                    bestX = tryX;
                    bestY = tryH;
                    break;
                }
                if (tryH < bestY) {
                    bestX = tryX;
                    bestY = tryH;
                }
            }
            gridX = bestX;
            sy = bestY;
        }
        blip.x = gridX;
        blip.gridX = gridX;
        blip.stackY = sy;
        blip.height = sy + blip.brickH;

        registerBlipInSeg(liveSeg, blip);

        if (blip.notableType && window._notableFeed) {
            window._notableFeed(tx);
        }
    }

    // Memory backstop: evict oldest (leftmost) non-notable, non-pinned bricks while
    // keeping _blipsByTxid, _colHeights, and _colBlips strictly in sync.
    if (liveSeg.blips.length > CULL_THRESHOLD) {
        var over = liveSeg.blips.length - CULL_TARGET;
        var flatW = Math.max(1, _hb.virtualX - liveSeg.x_start);
        var cutoffX = liveSeg.x_start + Math.min(0.95, (over / liveSeg.blips.length) * 1.15) * flatW;
        var kept = [];
        liveSeg._blipsByTxid = new Map();
        liveSeg._colHeights = {};
        liveSeg._colBlips = new Map();

        for (var ki = 0; ki < liveSeg.blips.length; ki++) {
            var cb = liveSeg.blips[ki];
            if (!cb || cb._removed) continue;
            var isPinned = (_hb._pinnedTxid && cb.txid === _hb._pinnedTxid);
            var keep = isPinned || cb.notableType || cb.fadeStart > 0
                || (cb.gridX !== undefined ? cb.gridX : cb.x) >= cutoffX;
            if (!keep) {
                cb._removed = true;
                cb._segIdx = -1;
                continue;
            }
            cb._segIdx = kept.length;
            kept.push(cb);
            if (cb.fadeStart === 0 && cb.txid) {
                liveSeg._blipsByTxid.set(cb.txid, cb);
            }
            if (cb.fadeStart > 0 || cb.gridX === undefined) continue;
            var col = liveSeg._colBlips.get(cb.gridX);
            if (!col) {
                col = [];
                liveSeg._colBlips.set(cb.gridX, col);
            }
            var newStackY = liveSeg._colHeights[cb.gridX] || 0;
            cb.stackY = newStackY;
            cb.height = (cb.brickH || 0) + newStackY;
            liveSeg._colHeights[cb.gridX] = newStackY + (cb.brickH || 0);
            col.push(cb);
        }
        liveSeg.blips = kept;
        if (_hb._pinnedTxid) {
            _hb._pinnedBlip = liveSeg._blipsByTxid.get(_hb._pinnedTxid) || null;
        }
        if (_hb.hoveredBlip && _hb.hoveredBlip._removed) _hb.hoveredBlip = null;
    }

    maybeClearRemovedTxids(_hb, 5000);
}

// ── Whale Watch Feed ──────────────────────────────────
var MAX_WHALE_ENTRIES = 100;

// Track seen txids to prevent duplicates when history replays + live arrives
window._seenNotableTxids = window._seenNotableTxids || new Set();

// One feed cell. The text is assigned as text and is never parsed as markup.
//
// This matters because `op_return_text` is bytes chosen by whoever broadcast
// the transaction: anyone willing to pay a fee decides what goes in it, and
// `notable.rs::extract_readable_text` admits anything 70% printable, which
// `<img src=x onerror=...>` satisfies comfortably. Building the row by string
// concatenation therefore turned a few hundred sats into script execution in
// every visitor's browser. The row is assembled from nodes so there is no
// markup for a payload to be part of.
function _feedSpan(className, text, title) {
    var el = document.createElement('span');
    el.className = className;
    el.textContent = text;
    if (title) el.title = title;
    return el;
}

// Build the feed row's cells and styling from a tx object.
// Accepts live booleans, persisted notable_type string, or CompactTx flags bitmask.
function _buildNotableRow(tx) {
    var flags = notableFlags(tx);
    var isWhale = flags.whale, isFeeOutlier = flags.feeOutlier,
        isConsolidation = flags.consolidation, isFanOut = flags.fanOut,
        isLargeInscription = flags.largeInscription,
        isRoundNumber = flags.roundNumber, isOpReturnMsg = flags.opReturnMsg;

    var btcVal = (tx.value || 0) / 100000000;
    var feeRate = tx.fee != null && tx.vsize ? formatFeeRate(tx.fee / tx.vsize) : '?';
    var timestamp = tx.first_seen || tx.timestamp || Math.floor(Date.now() / 1000);
    var time = new Date(timestamp * 1000).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
    var txidShort = tx.txid ? tx.txid.substring(0, 12) + '...' : '?';
    var usdVal = Math.round(tx.value_usd || 0);
    var usdStr = usdVal > 0 ? '$' + usdVal.toLocaleString() : fmtBtc(btcVal) + ' BTC';
    var isConfirmed = !!tx.confirmed_height;
    var statusNode = isConfirmed
        ? _feedSpan('text-white/40 text-[10px] shrink-0', '\u2713', 'Confirmed')
        : null;

    var labelNodes, feedType;
    if (isWhale) {
        feedType = 'whale';
        labelNodes = [
            _feedSpan('text-[#ffd700] font-bold shrink-0', 'WHALE'),
            _feedSpan('text-[#ffd700]/80 shrink-0', usdStr)
        ];
    } else if (isRoundNumber) {
        feedType = 'round_number';
        // Show the round output value (the part that triggered detection), not total
        var roundBtc = (tx.max_output_value || tx.value || 0) / 100000000;
        labelNodes = [
            _feedSpan('text-[#90ee90] font-bold shrink-0', 'ROUND #'),
            _feedSpan('text-white/60 shrink-0', fmtBtc(roundBtc) + ' BTC out')
        ];
    } else if (isLargeInscription) {
        feedType = 'inscription';
        labelNodes = [
            _feedSpan('text-[#ff00c8] font-bold shrink-0', 'INSCRIPTION'),
            _feedSpan('text-white/50 shrink-0', fmtBtc(btcVal) + ' BTC')
        ];
    } else if (isConsolidation) {
        feedType = 'consolidation';
        var ioC = (tx.input_count || '?') + '->' + (tx.output_count || '?');
        labelNodes = [
            _feedSpan('text-[#a855f7] font-bold shrink-0', 'CONSOL'),
            _feedSpan('text-white/50 shrink-0', ioC)
        ];
    } else if (isFanOut) {
        feedType = 'fan_out';
        var ioF = (tx.input_count || '?') + '->' + (tx.output_count || '?');
        labelNodes = [
            _feedSpan('text-[#00d2ff] font-bold shrink-0', 'FAN-OUT'),
            _feedSpan('text-white/50 shrink-0', ioF)
        ];
    } else if (isFeeOutlier) {
        feedType = 'fee';
        labelNodes = [
            _feedSpan('text-[#ff4444] font-bold shrink-0', 'FEE'),
            _feedSpan('text-white/50 shrink-0', feeRate + ' sat/vB')
        ];
    } else if (isOpReturnMsg) {
        feedType = 'op_return';
        // The only branch carrying bytes an outsider chose. The full message is
        // the title and the first 30 characters are the visible text; both are
        // set as text, so neither can introduce markup.
        var fullMsg = tx.op_return_text || '';
        labelNodes = [
            _feedSpan('text-[#ffa500] font-bold shrink-0', 'MSG'),
            _feedSpan('text-white/60 shrink-0 truncate max-w-[200px]',
                      fullMsg.substring(0, 30), fullMsg)
        ];
    } else {
        feedType = 'other';
        labelNodes = [_feedSpan('text-white/50 font-bold shrink-0', 'NOTABLE')];
    }

    return {
        labelNodes: labelNodes,
        feedType: feedType,
        txidShort: txidShort,
        time: time,
        statusNode: statusNode
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

    for (var i = 0; i < built.labelNodes.length; i++) {
        row.appendChild(built.labelNodes[i]);
    }
    if (built.statusNode) row.appendChild(built.statusNode);

    // `txid` reaches the DOM through dataset rather than an attribute written
    // into a markup string, so a txid that is not the hex we expect cannot
    // break out of the attribute.
    var txSpan = _feedSpan(
        'text-white/30 hover:text-[#f7931a] transition-colors cursor-pointer',
        built.txidShort);
    txSpan.dataset.txidLink = txid;
    row.appendChild(txSpan);

    row.appendChild(_feedSpan('text-white/20 ml-auto shrink-0', built.time));

    // Click txid to open live PinnedTxPanel (or fallback detail modal)
    if (txSpan) {
        txSpan.addEventListener('click', function() {
            if (typeof window.__hbSelectTx === 'function') {
                window.__hbSelectTx(txid);
            } else if (window.showTxDetail) {
                window.showTxDetail(txid, {
                    fee: tx.fee != null ? tx.fee : null,
                    vsize: tx.vsize != null ? tx.vsize : null,
                    feeRate: tx.fee != null && tx.vsize ? tx.fee / tx.vsize : null,
                    value: tx.value != null ? tx.value : null
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
