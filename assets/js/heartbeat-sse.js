// heartbeat-sse.js: Heartbeat v2 state bridge, deterministic brick placement,
// keyed reconciliation, pending-fee grace buffer, and DOM-safe notable feed.
import { getState, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, POINT_WIDTH } from './heartbeat-state.js';
import { feeRateColor, formatFeeRate, fmtBtc, createFlatlineSegment, resolveInterBlockSeconds, historyFlatlineWidth, waitColor, UNKNOWN_INTERVAL_COLOR, effectiveBlockWallTime } from './heartbeat-timeline.js';
import { playBlockCue } from './heartbeat-audio.js';

// Whether the viewer is currently watching the live tip (either auto-following OR
// unpaused & unpinned with the live tip visibly inside the viewport), so the
// cinematic block reveal triggers whenever you're watching live and never hijacks
// an inspecting/paused/pinned/off-screen-history viewer.
function isRevealEligible(_hb) {
    if (!_hb || _hb._suppressLiveVisuals || _hb._revealEnabled === false) return false;
    if (_hb._pinnedTxid || _hb.paused || _hb.isDragging || _hb._pinching) return false;
    if (!(_hb.width > 0 && _hb.zoom > 0 && _hb.zoom <= REVEAL_MAX_ENTRY_ZOOM)) return false;
    if (_hb.autoFollow) return true;
    var headScreenX = (_hb.virtualX - _hb.viewOffset) * _hb.zoom;
    return headScreenX >= _hb.width * 0.1 && headScreenX <= _hb.width * 0.98;
}

// Run the SEQUENCED block reveal at any active LIVE zoom up to maxZoom (50x):
// form pulls out smoothly to 2.2x to frame the full spike + absorption, then
// return glides right back to the viewer's exact entryZoom at the live tip.
var REVEAL_MAX_ENTRY_ZOOM = 50.0;

// Grace window (ms) to hold a newly observed membership_add before dropping it as a
// pending-fee slate brick, giving tx_details a moment to arrive with its real fee.
export var PENDING_GRACE_MS = 120;

// Memory backstop for long-running sessions with huge mempools (aligned above 150k-200k congestion).
var CULL_THRESHOLD = 220000;
var CULL_TARGET = 200000;

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

// Deterministic organic per-column target height (~45-145px) with gentle rolling
// hills + per-column jitter so even sustained high-rate bursts keep a natural,
// jagged skyline instead of water-leveling into a flat horizontal tabletop.
function organicColCap(gx, maxStack) {
    var col = (gx / 5) | 0;
    var h = Math.imul(col ^ 0x9e3779b9, 2654435761) >>> 0;
    var u = ((h >>> 16) & 0xffff) / 65536;
    var hill = Math.sin(col * 0.22) * 14;
    return Math.min(maxStack, Math.max(36, 58 + u * 72 + hill));
}

// Find a column near `prefGx` (within `[minGx, maxGx]`) minimizing overflow above
// each column's own `organicColCap` (rather than raw height, which water-levels
// columns into a flat tabletop). Only extends `_hb.virtualX` when `allowExtend`
// is true (bulk resyncs, hidden-tab drains, or >200-tx wake dumps).
function findColumnWithRoom(_hb, seg, prefGx, minGx, maxGx, brickH, softCap, hardCap, searchCols, allowExtend) {
    var gx = Math.max(minGx, Math.min(maxGx, prefGx));
    var sy = seg._colHeights[gx] || 0;
    var prefCap = Math.min(softCap, organicColCap(gx, softCap));
    if (sy === 0 || sy + brickH <= prefCap) {
        return { gx: gx, sy: sy, maxGx: maxGx };
    }
    var bestX = gx;
    var bestY = sy;
    var bestExcess = (sy + brickH) - prefCap;
    for (var step = 1; step <= searchCols; step++) {
        var cLeft = gx - step * 5;
        if (cLeft >= minGx) {
            var hLeft = seg._colHeights[cLeft] || 0;
            var capLeft = Math.min(softCap, organicColCap(cLeft, softCap));
            if (hLeft + brickH <= capLeft) {
                return { gx: cLeft, sy: hLeft, maxGx: maxGx };
            }
            var exLeft = (hLeft + brickH) - capLeft;
            if (exLeft < bestExcess) { bestX = cLeft; bestY = hLeft; bestExcess = exLeft; }
        }
        var cRight = gx + step * 5;
        if (cRight <= maxGx) {
            var hRight = seg._colHeights[cRight] || 0;
            var capRight = Math.min(softCap, organicColCap(cRight, softCap));
            if (hRight + brickH <= capRight) {
                return { gx: cRight, sy: hRight, maxGx: maxGx };
            }
            var exRight = (hRight + brickH) - capRight;
            if (exRight < bestExcess) { bestX = cRight; bestY = hRight; bestExcess = exRight; }
        }
    }
    if (allowExtend && bestY + brickH > hardCap) {
        var nextGx = maxGx + 5;
        if (_hb && nextGx + 5 > _hb.virtualX) {
            _hb.virtualX = nextGx + 5;
        }
        return { gx: nextGx, sy: seg._colHeights[nextGx] || 0, maxGx: nextGx };
    }
    return { gx: bestX, sy: bestY, maxGx: maxGx };
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

// Collect the open live flatline plus any flatlines sealed by an unresolved pending_block
// so tx_details, membership_remove, and block_inclusion stay live during slow block_details.
// O(1) tail scan: stops at the first confirmed block not preceded by an active pending slot.
function getActiveFlatlines(_hb) {
    var out = [];
    if (!_hb || !_hb.timeline) return out;
    var tl = _hb.timeline;
    for (var i = tl.length - 1; i >= 0; i--) {
        var seg = tl[i];
        if (seg.type === 'flatline' && (seg.x_end === null || seg._pendingForHash)) {
            ensureSegMaps(seg);
            out.push(seg);
        } else if (seg.type === 'block') {
            var prev1 = i > 0 ? tl[i - 1] : null;
            var prev2 = i > 1 ? tl[i - 2] : null;
            var hasPendingBefore = (prev1 && prev1.type === 'flatline' && !!prev1._pendingForHash)
                || (prev2 && prev2.type === 'pending_block');
            if (!hasPendingBefore) break;
        }
    }
    return out;
}

function findPendingBlockIndex(_hb, hash) {
    if (!_hb || !_hb.timeline || !hash) return -1;
    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        var seg = _hb.timeline[i];
        if (seg.type === 'pending_block' && seg.hash === hash) return i;
    }
    return -1;
}

// Temporarily swap to the parked live timeline while in 'history' mode so live SSE
// snapshots, effects, and tab recovery keep the live timeline up to date without
// disturbing the historical timeline or camera.
export function withLiveTimeline(_hb, fn) {
    if (!_hb || _hb._viewMode !== 'history' || !_hb._liveBackup) {
        return fn();
    }
    var histTimeline = _hb.timeline;
    var histVirtualX = _hb.virtualX;
    var histViewOffset = _hb.viewOffset;
    var histZoom = _hb.zoom;
    var histAutoFollow = _hb.autoFollow;
    var histPaused = _hb.paused;
    var histViewOffsetY = _hb.viewOffsetY;
    var prevSuppress = _hb._suppressLiveVisuals;
    var prevSuppressAnn = _hb._suppressAnnounce;

    _hb.timeline = _hb._liveBackup.timeline;
    _hb.virtualX = _hb._liveBackup.virtualX;
    _hb.viewOffset = _hb._liveBackup.viewOffset;
    _hb.zoom = _hb._liveBackup.zoom;
    _hb.autoFollow = false;
    _hb._suppressLiveVisuals = true;
    _hb._suppressAnnounce = true;
    try {
        return fn();
    } finally {
        _hb._liveBackup.timeline = _hb.timeline;
        _hb._liveBackup.virtualX = _hb.virtualX;
        _hb._liveBackup.viewOffset = _hb.viewOffset;
        _hb._liveBackup.zoom = _hb.zoom;

        _hb.timeline = histTimeline;
        _hb.virtualX = histVirtualX;
        _hb.viewOffset = histViewOffset;
        _hb.zoom = histZoom;
        _hb.autoFollow = histAutoFollow;
        _hb.paused = histPaused;
        _hb.viewOffsetY = histViewOffsetY;
        _hb._suppressLiveVisuals = prevSuppress;
        _hb._suppressAnnounce = prevSuppressAnn;
    }
}

// Rebuild a flatline segment's column height & blip maps after shifting its blips horizontally.
export function shiftSegmentBlips(seg, deltaX) {
    if (!seg || !seg.blips || seg.blips.length === 0) return;
    ensureSegMaps(seg);
    var oldMinGx = Math.ceil((seg.x_start - deltaX) / 5) * 5;
    var minGx = Math.ceil(seg.x_start / 5) * 5;
    var gridDelta = minGx - oldMinGx;
    seg._colHeights = {};
    seg._colBlips = new Map();
    seg._blipsByTxid = new Map();
    seg._sortedByX = false;
    for (var i = 0; i < seg.blips.length; i++) {
        var b = seg.blips[i];
        if (!b || b._removed) continue;
        b.x = (b.x || 0) + deltaX;
        if (b.absorbOriginX !== undefined) b.absorbOriginX += deltaX;
        if (b.absorbTargetX !== undefined) b.absorbTargetX += deltaX;
        if (b.fadeStart > 0) continue;
        var newGx = (b.gridX !== undefined ? b.gridX : b.x) + gridDelta;
        if (newGx < minGx) newGx = minGx;
        b.gridX = newGx;
        b.x = newGx;
        if (b.txid) seg._blipsByTxid.set(b.txid, b);
        var col = seg._colBlips.get(newGx);
        if (!col) {
            col = [];
            seg._colBlips.set(newGx, col);
        }
        col.push(b);
        var sy = seg._colHeights[newGx] || 0;
        b.stackY = sy;
        b.height = sy + (b.brickH || 0);
        seg._colHeights[newGx] = b.height;
    }
}

// Compress a sealed pre-block flatline from its raw real-time width to
// historyFlatlineWidth(interBlock), shifting ONLY segments to its right
// (never anything to the left of preFlatline.x_start). Preserves an inspecting
// viewer's camera to the exact pixel unless they were looking past preFlatline.x_end.
export function compressSealedFlatline(_hb, preFlatline, blockSegIndex, interBlock) {
    if (!_hb || !preFlatline || preFlatline.type !== 'flatline' || preFlatline._compressed) return 0;
    preFlatline._compressed = true;
    var oldEnd = preFlatline.x_end !== null ? preFlatline.x_end : _hb.virtualX;
    var oldW = Math.max(0, oldEnd - preFlatline.x_start);
    var targetW = historyFlatlineWidth(interBlock);
    var newEnd = preFlatline.x_start + targetW;
    var deltaX = newEnd - oldEnd;

    preFlatline.x_end = newEnd;
    preFlatline.inter_block_seconds = interBlock;
    preFlatline.color = interBlock === null ? UNKNOWN_INTERVAL_COLOR : waitColor(Math.max(0, interBlock));
    preFlatline._sortedByX = false;

    // Remap any shattering confirmed bricks still animating on preFlatline so they
    // stay inside [x_start, newEnd] and fly into the shifted block spike.
    if (preFlatline.blips && preFlatline.blips.length > 0 && Math.abs(deltaX) > 0.001) {
        var ratio = oldW > 0 ? (targetW / oldW) : 1;
        for (var bi = 0; bi < preFlatline.blips.length; bi++) {
            var blp = preFlatline.blips[bi];
            if (!blp || blp._removed) continue;
            var mappedX = preFlatline.x_start + (blp.x - preFlatline.x_start) * ratio;
            blp.x = mappedX;
            blp.gridX = mappedX;
            if (blp.absorbOriginX !== undefined) {
                blp.absorbOriginX = preFlatline.x_start + (blp.absorbOriginX - preFlatline.x_start) * ratio;
            }
            if (blp.absorbTargetX !== undefined) {
                blp.absorbTargetX += deltaX;
            }
        }
    }

    if (Math.abs(deltaX) < 0.001) return 0;

    // Resolve blockSegIndex defensively if not provided or shifted
    var startIdx = typeof blockSegIndex === 'number' ? blockSegIndex : (_hb.timeline.indexOf(preFlatline) + 1);
    if (startIdx < 1) startIdx = _hb.timeline.indexOf(preFlatline) + 1;
    for (var si = startIdx; si < _hb.timeline.length; si++) {
        var s = _hb.timeline[si];
        s.x_start += deltaX;
        if (s.x_end !== null) s.x_end += deltaX;
        if (typeof s._carryEndX === 'number') s._carryEndX += deltaX;
        if (s.type === 'flatline') {
            shiftSegmentBlips(s, deltaX);
        }
    }

    var tailSeg = _hb.timeline.length > 0 ? _hb.timeline[_hb.timeline.length - 1] : null;
    var minVirt = tailSeg ? tailSeg.x_start : 0;
    _hb.virtualX = Math.max(minVirt, _hb.virtualX + deltaX);

    if (_hb._searchHighlight && _hb._searchHighlight.x >= oldEnd) {
        _hb._searchHighlight.x += deltaX;
        if (_hb._searchHighlight.gridX !== undefined) _hb._searchHighlight.gridX += deltaX;
    }
    if (_hb._reveal) {
        _hb._reveal.spikeVX += deltaX;
        _hb._reveal.absorbX += deltaX;
        if (_hb._reveal.fromOffset !== undefined) _hb._reveal.fromOffset += deltaX;
    }

    // Camera invariance: never move an inspecting viewer looking at older history
    // to the left of preFlatline.x_end. During an active reveal or autoFollow, shift
    // viewOffset by deltaX so the spike/head stays locked on the exact same pixel.
    if (_hb._reveal || _hb.autoFollow) {
        _hb.viewOffset += deltaX;
    } else if (_hb.viewOffset >= oldEnd) {
        _hb.viewOffset += deltaX;
    }

    return deltaX;
}

// Reserve a pending_block slot on the live timeline when block_observed arrives so
// genuine post-block mempool admissions immediately land cleanly to its right.
export function reservePendingBlockSlot(_hb, hash, observedAt) {
    if (!_hb || !hash || !_hb.timeline) return null;

    // Dedupe against existing pending_block or confirmed block on the timeline
    for (var ti = _hb.timeline.length - 1; ti >= 0; ti--) {
        var existing = _hb.timeline[ti];
        if ((existing.type === 'pending_block' || existing.type === 'block') && existing.hash === hash) {
            return existing;
        }
    }

    // Finalize any active reveal for a previous block before sealing the live flatline
    if (_hb._reveal && window._hbEndReveal) {
        window._hbEndReveal(false);
    }

    var liveSeg = getLiveFlatline(_hb);
    if (!liveSeg) return null;

    var slotStart = Math.max(liveSeg.x_start, _hb.virtualX);
    var obsTs = observedAt || Math.floor(Date.now() / 1000);
    liveSeg.x_end = slotStart;
    liveSeg._pendingForHash = hash;
    if (_hb.lastBlockTime > 0 && obsTs >= _hb.lastBlockTime) {
        liveSeg.inter_block_seconds = obsTs - _hb.lastBlockTime;
        liveSeg.color = waitColor(liveSeg.inter_block_seconds);
    }

    var slotWidth = 51 * POINT_WIDTH;
    var pendingSeg = {
        type: 'pending_block',
        hash: hash,
        observed_at: obsTs,
        receivedWallMs: Date.now(),
        x_start: slotStart,
        x_end: slotStart + slotWidth,
        color: '#38bdf8'
    };
    _hb.timeline.push(pendingSeg);
    _hb.virtualX = pendingSeg.x_end;
    _hb.timeline.push(createFlatlineSegment(_hb.virtualX, null));
    return pendingSeg;
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
        blip.flags = (blip.flags || 0) | tx.flags;
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

function syncBlocksFromSnapshot(_hb, payload, isResync, skipPendingReserve) {
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
    var recent = Array.isArray(payload.recent_blocks) ? payload.recent_blocks : [];
    if (recent.length > 0) {
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
                var interSec = resolveInterBlockSeconds(sorted[i]);
                if (interSec === null && i > 0 && sorted[i].timestamp && sorted[i - 1].timestamp) {
                    interSec = sorted[i].timestamp - sorted[i - 1].timestamp;
                }
                replayList.push({
                    hash: sorted[i].hash || null,
                    height: sorted[i].height,
                    timestamp: sorted[i].timestamp,
                    observed_at: sorted[i].observed_at || 0,
                    tx_count: sorted[i].tx_count,
                    total_fees: sorted[i].total_fees,
                    size: sorted[i].size,
                    weight: sorted[i].weight,
                    miner: sorted[i].miner || null,
                    inter_block_seconds: interSec
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
                        observed_at: missed[k].observed_at || 0,
                        tx_count: missed[k].tx_count,
                        total_fees: missed[k].total_fees,
                        size: missed[k].size,
                        weight: missed[k].weight,
                        miner: missed[k].miner || null,
                        inter_block_seconds: resolveInterBlockSeconds(missed[k])
                    }]), false);
                }
                _hb._suppressAnnounce = prevSuppress;
            }
        }

        var newestBlock = sorted[sorted.length - 1];
        var newestWall = effectiveBlockWallTime(newestBlock, Math.floor(Date.now() / 1000));
        if (newestWall > 0 && newestWall > (_hb.lastBlockTime || 0)) {
            _hb.lastBlockTime = newestWall;
        }
    }

    // Reserve pending_block slots for any snapshot pending_blocks not already on the timeline
    // (deferred until after initial pre-block backlog placement when skipPendingReserve is true).
    if (!skipPendingReserve && Array.isArray(payload.pending_blocks) && payload.pending_blocks.length > 0) {
        for (var pbi = 0; pbi < payload.pending_blocks.length; pbi++) {
            var pb = payload.pending_blocks[pbi];
            if (pb && pb.hash) {
                reservePendingBlockSlot(_hb, pb.hash, pb.observed_at);
            }
        }
    }

    // Remove any orphaned pending_block slot at the tail that is in neither
    // payload.pending_blocks nor payload.recent_blocks.
    var validHashes = new Set();
    if (Array.isArray(payload.pending_blocks)) {
        for (var vp = 0; vp < payload.pending_blocks.length; vp++) {
            if (payload.pending_blocks[vp] && payload.pending_blocks[vp].hash) {
                validHashes.add(payload.pending_blocks[vp].hash);
            }
        }
    }
    for (var vr = 0; vr < recent.length; vr++) {
        if (recent[vr] && recent[vr].hash) validHashes.add(recent[vr].hash);
    }
    for (var oi = _hb.timeline.length - 1; oi >= Math.max(0, _hb.timeline.length - 64); oi--) {
        var oSeg = _hb.timeline[oi];
        if (oSeg.type === 'pending_block' && (!oSeg.hash || !validHashes.has(oSeg.hash))) {
            var oWidth = Math.max(0, (oSeg.x_end || oSeg.x_start) - oSeg.x_start);
            var oEnd = oSeg.x_end !== null ? oSeg.x_end : (oSeg.x_start + oWidth);
            if (_hb.hoveredPendingBlock === oSeg) _hb.hoveredPendingBlock = null;
            _hb.timeline.splice(oi, 1);
            mergeAdjacentFlatlines(_hb, oi, oWidth, oEnd);
        }
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

    return withLiveTimeline(_hb, function() {
        return _hbV2InstallSnapshotInner(_hb, payload, isResync);
    });
}

function _hbV2InstallSnapshotInner(_hb, payload, isResync) {
    _hb._coverage = payload.coverage || { status: 'unknown' };
    _hb._lastSseEventTs = Date.now() / 1000;

    var preActiveFlatlines = getActiveFlatlines(_hb);
    var preTotalActiveBlips = 0;
    for (var pafc = 0; pafc < preActiveFlatlines.length; pafc++) {
        preTotalActiveBlips += preActiveFlatlines[pafc]._blipsByTxid.size;
    }
    var deferPendingForInitial = (!isResync || preTotalActiveBlips === 0)
        && Array.isArray(payload.pending_blocks)
        && payload.pending_blocks.length > 0;

    // Sync confirmations and recent blocks before placing mempool txs; on initial load
    // with pending_blocks, reserve the pending_block slot AFTER placing pre-block txs so
    // the waiting mempool sits to the LEFT of the pending slot ready to absorb into it.
    syncBlocksFromSnapshot(_hb, payload, !!isResync, deferPendingForInitial);

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
    var activeFlatlines = getActiveFlatlines(_hb);
    var totalActiveBlips = 0;
    for (var afc = 0; afc < activeFlatlines.length; afc++) {
        totalActiveBlips += activeFlatlines[afc]._blipsByTxid.size;
    }
    if (isResync && totalActiveBlips > 0) {
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

        // Reconcile removals & updates across ALL active flatlines (both the open live
        // flatline and any flatline sealed by an unresolved pending_block) so resync never
        // duplicates 64k pre-block bricks onto postFlatline while a block is pending.
        var dirtyBySeg = new Map();
        for (var afi = 0; afi < activeFlatlines.length; afi++) {
            var aSeg = activeFlatlines[afi];
            var aDirty = new Set();
            var toRemove = [];
            aSeg._blipsByTxid.forEach(function(blip, txid) {
                if (!incomingMap.has(txid)) {
                    toRemove.push(txid);
                }
            });
            for (var ri = 0; ri < toRemove.length; ri++) {
                var rTxid = toRemove[ri];
                var rBlip = aSeg._blipsByTxid.get(rTxid);
                if (!rBlip) continue;
                aSeg._blipsByTxid.delete(rTxid);
                rBlip._removed = true;
                removeBlipFromArray(aSeg, rBlip);
                aDirty.add(rBlip.gridX);
                if (_hb.hoveredBlip === rBlip) _hb.hoveredBlip = null;
                // Clear the canvas blip ref if removed, but NEVER clear _hb._pinnedTxid:
                // WASM updates the pinned panel with confirmed/removed status.
                if (_hb._pinnedBlip === rBlip) _hb._pinnedBlip = null;
            }
            if (aDirty.size > 0) dirtyBySeg.set(aSeg, aDirty);
        }

        var newTxs = [];
        for (var ni = 0; ni < incomingOrder.length; ni++) {
            var itx = incomingOrder[ni];
            var existing = null;
            var existingSeg = null;
            for (var afi2 = 0; afi2 < activeFlatlines.length; afi2++) {
                if (activeFlatlines[afi2]._blipsByTxid.has(itx.txid)) {
                    existingSeg = activeFlatlines[afi2];
                    existing = existingSeg._blipsByTxid.get(itx.txid);
                    break;
                }
            }
            if (existing) {
                var hChanged = updateBlipMetadata(existing, itx);
                if (hChanged) {
                    var sd = dirtyBySeg.get(existingSeg);
                    if (!sd) { sd = new Set(); dirtyBySeg.set(existingSeg, sd); }
                    sd.add(existing.gridX);
                }
            } else {
                newTxs.push(itx);
            }
        }

        dirtyBySeg.forEach(function(cols, seg) {
            if (cols.size > 0) restackColumns(seg, cols);
        });

        // Append newly discovered txids cleanly without piling into flat-topped
        // skyscrapers or leaving a detached island after a sparse/disconnected gap:
        // sort chronologically, anchor right after the last dense brick column when
        // empty space already exists behind headGx, and respect softMaxStack.
        if (newTxs.length > 0) {
            newTxs.sort(function(a, b) {
                var aKnown = (a.first_seen || 0) > 0 ? 1 : 0;
                var bKnown = (b.first_seen || 0) > 0 ? 1 : 0;
                if (aKnown !== bKnown) return aKnown - bKnown;
                if (aKnown === 1 && a.first_seen !== b.first_seen) return a.first_seen - b.first_seen;
                return a.txid < b.txid ? -1 : (a.txid > b.txid ? 1 : 0);
            });
            var resyncSoftMax = Math.min(maxStack, 160);
            var minGx = Math.ceil(liveSeg.x_start / 5) * 5;
            var newTotalH = 0;
            for (var nhi = 0; nhi < newTxs.length; nhi++) {
                var nt0 = newTxs[nhi];
                var np0 = nt0.fee == null || nt0.vsize == null || nt0.vsize <= 0;
                var nr0 = np0 ? null : (nt0.fee / nt0.vsize);
                var nn0 = np0 ? 0.15 : Math.min(Math.log2(Math.max(0, nr0) + 1) / 6, 1.0);
                newTotalH += (anyNotable(notableFlags(nt0)) ? (18 + nn0 * 10 + 1.5) : (np0 ? 6 : (3 + nn0 * 14 + 1.5)));
            }
            var targetCols = Math.max(1, Math.ceil(newTotalH / 65));
            var neededSpan = targetCols * 5;
            var headGx = Math.max(minGx, Math.floor(_hb.virtualX / 5) * 5);
            // Locate the rightmost column that already holds settled bricks so
            // resync recovery backfills any empty gap directly instead of jumping
            // past it and stranding older first_seen txs at the far right edge.
            var lastBrickGx = -1;
            if (liveSeg._colHeights) {
                for (var ck in liveSeg._colHeights) {
                    if (liveSeg._colHeights[ck] > 0) {
                        var gxNum = Number(ck);
                        if (gxNum >= minGx && gxNum > lastBrickGx) lastBrickGx = gxNum;
                    }
                }
            }
            var startGx;
            var hasEmptyGap = lastBrickGx >= minGx && (headGx - lastBrickGx) > 20;
            if (hasEmptyGap) {
                startGx = lastBrickGx + 5;
                var gapCols = Math.max(1, Math.floor((headGx - startGx) / 5));
                var desiredCols = Math.min(gapCols, Math.max(targetCols * 2, 20));
                if (neededSpan > (headGx - startGx)) {
                    _hb.virtualX = startGx + neededSpan + 10;
                    headGx = Math.floor(_hb.virtualX / 5) * 5;
                } else {
                    // Compact unused trailing void when no bricks exist past lastBrickGx
                    // so small resyncs (e.g. 80-120 txs over a wide gap) don't turn into dust.
                    headGx = startGx + desiredCols * 5;
                    _hb.virtualX = headGx + 10;
                }
            } else if (newTxs.length > 100) {
                startGx = headGx + 5;
                _hb.virtualX = Math.max(_hb.virtualX, startGx + neededSpan + 10);
                headGx = Math.floor(_hb.virtualX / 5) * 5;
            } else {
                startGx = Math.max(minGx, headGx - neededSpan);
                if (newTxs.length > 30 && startGx + neededSpan + 10 > _hb.virtualX) {
                    _hb.virtualX = startGx + neededSpan + 10;
                    headGx = Math.floor(_hb.virtualX / 5) * 5;
                }
            }
            var availCols = Math.max(1, Math.floor(Math.max(5, headGx - startGx) / 5));
            for (var ai = 0; ai < newTxs.length; ai++) {
                var ntx = newTxs[ai];
                var gx;
                var nAge = ntx.first_seen > 0 ? (nowSec - ntx.first_seen) : -1;
                if (!hasEmptyGap && newTxs.length <= 100 && nAge >= 0 && nAge <= 300) {
                    var ageCols = Math.round((nAge * FLATLINE_PX_PER_SEC) / 5);
                    var jit = Math.round(txidSkylineOffset(ntx.txid) * 1.5);
                    gx = Math.max(minGx, Math.min(headGx, headGx - (ageCols + jit) * 5));
                } else {
                    var colIdx = Math.floor((ai * availCols) / newTxs.length);
                    var spreadJit = newTxs.length > 100 ? Math.round(txidSkylineOffset(ntx.txid) * Math.min(16, Math.floor(availCols * 0.05))) : 0;
                    var finalCol = Math.max(0, Math.min(availCols - 1, colIdx + spreadJit));
                    gx = startGx + finalCol * 5;
                }
                var resyncHardMax = Math.min(maxStack, 185);
                var nBlip = buildBlip(ntx, gx, gx, 0, nowSec, false);
                var rSlot = findColumnWithRoom(
                    _hb, liveSeg, gx, minGx, headGx, nBlip.brickH,
                    resyncSoftMax, resyncHardMax, 24, true
                );
                gx = rSlot.gx;
                var sy = rSlot.sy;
                headGx = rSlot.maxGx;
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
            _hb._pinnedBlip = null;
            for (var afi3 = 0; afi3 < activeFlatlines.length; afi3++) {
                if (activeFlatlines[afi3]._blipsByTxid.has(_hb._pinnedTxid)) {
                    _hb._pinnedBlip = activeFlatlines[afi3]._blipsByTxid.get(_hb._pinnedTxid);
                    break;
                }
            }
        }
        return;
    }

    // ── Initial ordered backlog placement ──
    // If empty-live resync arrived while pending_block slot(s) were already reserved
    // on the empty tail, unwind those empty tail slots so bucket 0 lands LEFT of the
    // earliest pending slot and slots are re-reserved cleanly between buckets.
    if (deferPendingForInitial) {
        while (_hb.timeline.length >= 3) {
            var tailF = _hb.timeline[_hb.timeline.length - 1];
            var prevP = _hb.timeline[_hb.timeline.length - 2];
            if (tailF.type === 'flatline' && tailF.x_end === null
                && (!tailF.blips || tailF.blips.length === 0)
                && prevP.type === 'pending_block') {
                _hb.timeline.pop(); // empty post-flatline
                var poppedP = _hb.timeline.pop(); // pending_block
                if (_hb.hoveredPendingBlock === poppedP) _hb.hoveredPendingBlock = null;
                var prevPre = _hb.timeline[_hb.timeline.length - 1];
                if (prevPre && prevPre.type === 'flatline') {
                    prevPre.x_end = null;
                    prevPre._pendingForHash = null;
                    _hb.virtualX = Math.max(prevPre.x_start, poppedP.x_start);
                }
            } else {
                break;
            }
        }
        liveSeg = getLiveFlatline(_hb);
        if (!liveSeg) return;
    }

    liveSeg.blips = [];
    liveSeg._blipsByTxid = new Map();
    liveSeg._colHeights = {};
    liveSeg._colBlips = new Map();

    // Sort pending blocks chronologically and partition rawTxs into buckets:
    // bucket 0 (left of pending[0]), buckets 1..N-1 (between pending[k-1] and pending[k]),
    // and bucket N (right of pending[N-1]).
    var sortedPending = [];
    var postPendingBuckets = [];
    if (deferPendingForInitial) {
        for (var epo = 0; epo < payload.pending_blocks.length; epo++) {
            var epb = payload.pending_blocks[epo];
            if (epb && epb.hash) {
                sortedPending.push(epb);
            }
        }
        sortedPending.sort(function(a, b) {
            return (a.observed_at || 0) - (b.observed_at || 0);
        });
        for (var pbi0 = 0; pbi0 < sortedPending.length; pbi0++) {
            postPendingBuckets.push([]);
        }
    }

    var TARGET_COL_H = 65;
    var parsedTxs = new Array(rawTxs.length);
    var parsedCount = 0;
    var totalBrickH = 0;
    var unknownBrickH = 0;
    for (var pi = 0; pi < rawTxs.length; pi++) {
        var ptx = compactTupleToTx(rawTxs[pi]);
        if (!ptx || !ptx.txid) continue;
        if (_hb._removedTxids && _hb._removedTxids.has(ptx.txid)) continue;
        if (deferPendingForInitial && sortedPending.length > 0 && ptx.first_seen > 0) {
            var placedBucket = -1;
            for (var pbi1 = sortedPending.length - 1; pbi1 >= 0; pbi1--) {
                var pObs = sortedPending[pbi1].observed_at || 0;
                if (pObs > 0 && ptx.first_seen > pObs) {
                    placedBucket = pbi1;
                    break;
                }
            }
            if (placedBucket >= 0) {
                postPendingBuckets[placedBucket].push(ptx);
                continue;
            }
        }
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

    // Auto-fit zoom ONLY on initial load when auto-following and not pinned (never in history mode).
    // Re-arm the load fade veil so dismissing the loading overlay fades cleanly
    // into the already-framed mempool with zero zoomed-in spike flash.
    if (!isResync && !_hb._suppressLiveVisuals) {
        _hb._loadFadeStart = nowSec;
    }
    if (!isResync && !_hb._suppressLiveVisuals && _hb.autoFollow && !_hb._pinnedTxid && _hb.width && flatlineSpan > 10) {
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
    var DROP_DURATION = (isResync || _hb._suppressLiveVisuals) ? 0 : 2.3;
    var dropOrigin = nowSec + ((isResync || _hb._suppressLiveVisuals) ? 0 : 0.08);
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
            if (pCol < knownStartCol) pCol = knownStartCol + (knownStartCol - pCol - 1);
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
    var snapRec = { seg: liveSeg, items: planned, nextIdx: 0, buildBlip: buildBlip };
    _hb._pendingSnapshot = snapRec;

    // On initial load with pending_blocks, reserve each pending slot in order and
    // place each inter-pending / post-pending bucket on the flatline directly after
    // its corresponding pending slot so every tx sits on the exact side it belongs on.
    if (deferPendingForInitial && sortedPending.length > 0) {
        var postSoftMax = Math.min(maxStack, 160);
        for (var dpbi = 0; dpbi < sortedPending.length; dpbi++) {
            var dpb = sortedPending[dpbi];
            reservePendingBlockSlot(_hb, dpb.hash, dpb.observed_at);
            var bucketTxs = postPendingBuckets[dpbi];
            if (bucketTxs && bucketTxs.length > 0) {
                var postSeg = getLiveFlatline(_hb);
                if (postSeg) {
                    ensureSegMaps(postSeg);
                    var postMinGx = Math.ceil(postSeg.x_start / 5) * 5;
                    var postMaxGx = Math.max(postMinGx, Math.floor(_hb.virtualX / 5) * 5);
                    for (var ppi = 0; ppi < bucketTxs.length; ppi++) {
                        var pptx = bucketTxs[ppi];
                        if (postSeg._blipsByTxid.has(pptx.txid)) continue;
                        var pColPos = findColumnWithRoom(_hb, postSeg, postMaxGx, postMinGx, postMaxGx, 10, postSoftMax, maxStack, 24, true);
                        postMaxGx = pColPos.maxGx;
                        registerBlipInSeg(postSeg, buildBlip(pptx, pColPos.gx, pColPos.sy, 0, nowSec, false));
                    }
                }
            }
        }
        if (!isResync && !_hb._suppressLiveVisuals && _hb.autoFollow && !_hb._pinnedTxid && _hb.width) {
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
        }
    }

    var finishSnapshot = function() {
        var curSeg = snapRec.seg;
        if (curSeg) curSeg._loadFillCount = 0;
        if (_hb._pendingSnapshot === snapRec) {
            _hb._pendingSnapshot = null;
        }
        if (_hb._deferredDetails) _hb._deferredDetails.clear();
        maybeClearRemovedTxids(_hb);
        if (_hb._pendingFocusTxid) {
            var pendingFocus = _hb._pendingFocusTxid;
            _hb._pendingFocusTxid = null;
            hbV2FocusTx(pendingFocus);
        } else if (_hb._pinnedTxid && curSeg && curSeg._blipsByTxid) {
            _hb._pinnedBlip = curSeg._blipsByTxid.get(_hb._pinnedTxid) || null;
        }
        if (notableTxs.length > 0 && window._notableFeed) {
            for (var ni = notableTxs.length - 1; ni >= 0; ni--) {
                window._notableFeed(notableTxs[ni]);
            }
        }
    };

    var placeChunk = function() {
        var s = getState();
        if (!s) return;
        var curSeg = snapRec.seg;
        var liveTl = (s._viewMode === 'history' && s._liveBackup) ? s._liveBackup.timeline : s.timeline;
        var stillActivePrePending = !!(curSeg && curSeg._pendingForHash && liveTl.indexOf(curSeg) >= 0);
        var isLiveTail = !!(curSeg && liveTl[liveTl.length - 1] === curSeg && curSeg.x_end === null);
        if (!isLiveTail && !stillActivePrePending) {
            s._historyRaf = null;
            finishSnapshot();
            return;
        }
        ensureSegMaps(curSeg);
        var chunkStartMs = nowPerfMs();
        var budgetMs = (s.width && s.width < 640) ? 5 : 10;
        var end = Math.min(idx + CHUNK_SIZE, planned.length);
        var span = Math.max(1, usableSpan);
        var chunkStartGridX = Math.ceil(curSeg.x_start / 5) * 5;
        var chunkDone = 0;
        for (; idx < end; idx++) {
            if (!isResync && chunkDone > 0 && (chunkDone & 255) === 0 && (nowPerfMs() - chunkStartMs) >= budgetMs) {
                break;
            }
            chunkDone++;
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
            if (curSeg._blipsByTxid.has(tx.txid)) continue;

            var gx = chunkStartGridX + item.col * 5;
            var sy = curSeg._colHeights[gx] || 0;
            var xFrac = Math.min(1, Math.max(0, (gx - curSeg.x_start) / span));
            // Use an uncorrelated per-brick delay (from txidSkylineOffset) plus a
            // gentle stackY offset so blue bricks rain down feathered like the
            // notables instead of launching in lockstep as one solid curved sheet.
            var rainJitter = ((txidSkylineOffset(tx.txid) + 1) * 0.5) * 0.55;
            var stackDelay = DROP_DURATION > 0 ? (sy * 0.0025 + rainJitter) : 0;
            var dropTs = dropOrigin + (DROP_DURATION > 0 ? xFrac * DROP_DURATION + stackDelay : 0);
            var blip = buildBlip(tx, gx, gx, sy, dropTs, !isResync);

            if (sy > 0 && sy + blip.brickH > softMaxStack) {
                var minZoneX = chunkStartGridX + (tx.first_seen ? knownStartCol : 0) * 5;
                var maxZoneX = chunkStartGridX + (tx.first_seen ? usableCols - 1 : Math.max(0, unknownCols - 1)) * 5;
                var bestX = gx;
                var bestY = sy;
                for (var step = 1; step <= 10; step++) {
                    var c1 = gx + step * 5;
                    if (c1 <= maxZoneX) {
                        var h1 = curSeg._colHeights[c1] || 0;
                        if (h1 < bestY) { bestX = c1; bestY = h1; if (bestY + blip.brickH <= softMaxStack) break; }
                    }
                    var c2 = gx - step * 5;
                    if (c2 >= minZoneX) {
                        var h2 = curSeg._colHeights[c2] || 0;
                        if (h2 < bestY) { bestX = c2; bestY = h2; if (bestY + blip.brickH <= softMaxStack) break; }
                    }
                }
                blip.x = bestX;
                blip.gridX = bestX;
                blip.stackY = bestY;
                blip.height = bestY + blip.brickH;
            }

            registerBlipInSeg(curSeg, blip);

            if (blip.notableType) {
                notableTxs.push(tx);
            }
        }
        if (s._pendingSnapshot === snapRec) {
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

// Merge two adjacent flatlines leftSeg + rightSeg after splicing out a segment
// of width `removedWidth` between them, preserving every surviving brick cleanly.
function mergeAdjacentFlatlines(_hb, spliceIdx, removedWidth, removedEnd) {
    if (spliceIdx > 0 && spliceIdx < _hb.timeline.length &&
        _hb.timeline[spliceIdx - 1].type === 'flatline' &&
        _hb.timeline[spliceIdx].type === 'flatline') {
        var leftSeg = _hb.timeline[spliceIdx - 1];
        var rightSeg = _hb.timeline[spliceIdx];
        var effRemovedEnd = typeof removedEnd === 'number' ? removedEnd : rightSeg.x_start;
        ensureSegMaps(leftSeg);
        ensureSegMaps(rightSeg);

        // Rebuild leftSeg maps from its active blips in case they were cleared on seal
        if (leftSeg.blips && leftSeg.blips.length > 0 && leftSeg._blipsByTxid.size === 0) {
            leftSeg._colHeights = {};
            leftSeg._colBlips = new Map();
            for (var li = 0; li < leftSeg.blips.length; li++) {
                var lb = leftSeg.blips[li];
                if (!lb || lb._removed || lb.fadeStart > 0) continue;
                if (lb.txid) leftSeg._blipsByTxid.set(lb.txid, lb);
                var lgx = lb.gridX !== undefined ? lb.gridX : Math.round(lb.x / 5) * 5;
                lb.gridX = lgx;
                var lcol = leftSeg._colBlips.get(lgx);
                if (!lcol) { lcol = []; leftSeg._colBlips.set(lgx, lcol); }
                lcol.push(lb);
                var lsy = leftSeg._colHeights[lgx] || 0;
                lb.stackY = lsy;
                lb.height = lsy + (lb.brickH || 0);
                leftSeg._colHeights[lgx] = lb.height;
            }
        }

        leftSeg._pendingForHash = rightSeg._pendingForHash || null;
        leftSeg._compressed = false;
        leftSeg._sortedByX = false;
        leftSeg.x_end = rightSeg.x_end === null ? null : Math.max(leftSeg.x_start, rightSeg.x_end - removedWidth);
        if (leftSeg.x_end === null) {
            leftSeg.color = null;
        }
        if (typeof rightSeg._carryEndX === 'number') {
            leftSeg._carryEndX = Math.max(leftSeg.x_start, rightSeg._carryEndX - removedWidth);
        }

        var minLeftGx = Math.ceil(leftSeg.x_start / 5) * 5;
        var gridShift = Math.round(removedWidth / 5) * 5;
        if (rightSeg.blips && rightSeg.blips.length > 0) {
            for (var rbi = 0; rbi < rightSeg.blips.length; rbi++) {
                var rb = rightSeg.blips[rbi];
                if (!rb || rb._removed) continue;
                if (rb.fadeStart === 0 && rb.txid && leftSeg._blipsByTxid.has(rb.txid)) continue;
                rb.x = Math.max(leftSeg.x_start, (rb.x || 0) - removedWidth);
                if (rb.absorbOriginX !== undefined) rb.absorbOriginX -= removedWidth;
                if (rb.absorbTargetX !== undefined) rb.absorbTargetX -= removedWidth;
                if (rb.fadeStart === 0) {
                    var rgx = (rb.gridX !== undefined ? rb.gridX : rb.x) - gridShift;
                    if (rgx < minLeftGx) rgx = minLeftGx;
                    rb.gridX = rgx;
                    rb.x = rgx;
                    var rsy = leftSeg._colHeights[rgx] || 0;
                    rb.stackY = rsy;
                    rb.height = rsy + (rb.brickH || 0);
                    registerBlipInSeg(leftSeg, rb);
                } else {
                    rb._segIdx = leftSeg.blips.length;
                    leftSeg.blips.push(rb);
                }
            }
        }

        if (_hb._pendingSnapshot && _hb._pendingSnapshot.seg === rightSeg) {
            _hb._pendingSnapshot.seg = leftSeg;
        }
        if (_hb._pendingCarry && _hb._pendingCarry.seg === rightSeg) {
            _hb._pendingCarry.seg = leftSeg;
        }
        _hb.timeline.splice(spliceIdx, 1);

        // Shift any segments still to the right of the merged flatline left by -removedWidth
        for (var si = spliceIdx; si < _hb.timeline.length; si++) {
            var rs = _hb.timeline[si];
            rs.x_start -= removedWidth;
            if (rs.x_end !== null) rs.x_end -= removedWidth;
            if (typeof rs._carryEndX === 'number') rs._carryEndX -= removedWidth;
            if (rs.type === 'flatline') shiftSegmentBlips(rs, -removedWidth);
        }
        _hb.virtualX = Math.max(leftSeg.x_start, _hb.virtualX - removedWidth);
        if (removedWidth > 0) {
            if (_hb._searchHighlight && _hb._searchHighlight.x >= effRemovedEnd) {
                _hb._searchHighlight.x -= removedWidth;
                if (_hb._searchHighlight.gridX !== undefined) _hb._searchHighlight.gridX -= removedWidth;
            }
            if (_hb._reveal) {
                _hb._reveal.spikeVX -= removedWidth;
                _hb._reveal.absorbX -= removedWidth;
                if (_hb._reveal.fromOffset !== undefined) _hb._reveal.fromOffset -= removedWidth;
            }
            if (_hb._reveal || _hb.autoFollow || _hb.viewOffset >= effRemovedEnd) {
                _hb.viewOffset -= removedWidth;
            }
        }
        return;
    }

    // Fallback: not flanked by two flatlines — just shift remaining right-hand segments left
    var fallbackRemovedEnd = typeof removedEnd === 'number'
        ? removedEnd
        : (spliceIdx < _hb.timeline.length ? _hb.timeline[spliceIdx].x_start : _hb.virtualX);
    for (var si2 = spliceIdx; si2 < _hb.timeline.length; si2++) {
        var rs2 = _hb.timeline[si2];
        rs2.x_start -= removedWidth;
        if (rs2.x_end !== null) rs2.x_end -= removedWidth;
        if (typeof rs2._carryEndX === 'number') rs2._carryEndX -= removedWidth;
        if (rs2.type === 'flatline') shiftSegmentBlips(rs2, -removedWidth);
    }
    var tail = _hb.timeline.length > 0 ? _hb.timeline[_hb.timeline.length - 1] : null;
    _hb.virtualX = Math.max(tail ? tail.x_start : 0, _hb.virtualX - removedWidth);
    if (removedWidth > 0) {
        if (_hb._searchHighlight && _hb._searchHighlight.x >= fallbackRemovedEnd) {
            _hb._searchHighlight.x -= removedWidth;
            if (_hb._searchHighlight.gridX !== undefined) _hb._searchHighlight.gridX -= removedWidth;
        }
        if (_hb._reveal) {
            _hb._reveal.spikeVX -= removedWidth;
            _hb._reveal.absorbX -= removedWidth;
            if (_hb._reveal.fromOffset !== undefined) _hb._reveal.fromOffset -= removedWidth;
        }
        if (_hb._reveal || _hb.autoFollow || _hb.viewOffset >= fallbackRemovedEnd) {
            _hb.viewOffset -= removedWidth;
        }
    }
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

    return withLiveTimeline(_hb, function() {
        return _hbV2ApplyEffectInner(_hb, effect);
    });
}

function _hbV2ApplyEffectInner(_hb, effect) {
    if (!_hb._pendingAdds) _hb._pendingAdds = new Map();
    if (!_hb._hiddenTxMap) _hb._hiddenTxMap = new Map();
    if (!_hb._txBatchQueue) _hb._txBatchQueue = [];
    if (!_hb._removedTxids) _hb._removedTxids = new Set();
    if (!_hb._exactConfirmedByHash) _hb._exactConfirmedByHash = new Map();

    _hb._lastSseEventTs = Date.now() / 1000;
    var liveSeg = getLiveFlatline(_hb);
    var activeFlatlines = getActiveFlatlines(_hb);
    var isHidden = typeof document !== 'undefined' && document.hidden;

    switch (effect.type) {
        case 'membership_add': {
            var addTxid = effect.txid;
            if (!addTxid) return;
            _hb._removedTxids.delete(addTxid);
            for (var afi = 0; afi < activeFlatlines.length; afi++) {
                if (activeFlatlines[afi]._blipsByTxid.has(addTxid)) return;
            }
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

            // 2. Already placed on the live flatline (or a pending-sealed pre-block flatline) -> recolour & restack in place.
            for (var dfi = 0; dfi < activeFlatlines.length; dfi++) {
                var aSeg = activeFlatlines[dfi];
                if (aSeg._blipsByTxid.has(dtx.txid)) {
                    var existingBlip = aSeg._blipsByTxid.get(dtx.txid);
                    var heightChanged = updateBlipMetadata(existingBlip, dtx);
                    if (heightChanged) {
                        restackColumns(aSeg, [existingBlip.gridX]);
                    }
                    if (anyNotable(notableFlags(dtx)) && window._notableFeed) {
                        window._notableFeed(dtx);
                    }
                    return;
                }
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
            var dirtyBySeg = new Map();

            for (var ri = 0; ri < remTxids.length; ri++) {
                var rtxid = remTxids[ri];
                if (!rtxid) continue;
                _hb._removedTxids.add(rtxid);
                _hb._pendingAdds.delete(rtxid);
                if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(rtxid);
                if (_hb._deferredDetails) _hb._deferredDetails.delete(rtxid);

                for (var rfi = 0; rfi < activeFlatlines.length; rfi++) {
                    var rfSeg = activeFlatlines[rfi];
                    if (rfSeg._blipsByTxid.has(rtxid)) {
                        var rBlip = rfSeg._blipsByTxid.get(rtxid);
                        rfSeg._blipsByTxid.delete(rtxid);
                        rBlip._removed = true;
                        removeBlipFromArray(rfSeg, rBlip);
                        var dCols = dirtyBySeg.get(rfSeg);
                        if (!dCols) { dCols = new Set(); dirtyBySeg.set(rfSeg, dCols); }
                        dCols.add(rBlip.gridX);
                        if (_hb.hoveredBlip === rBlip) _hb.hoveredBlip = null;
                        if (_hb._pinnedBlip === rBlip) _hb._pinnedBlip = null;
                    }
                }
            }

            purgeInFlightTxids(_hb, remTxids);

            if (_hb._txBatchQueue.length > 0) {
                _hb._txBatchQueue = _hb._txBatchQueue.filter(function(qtx) {
                    return !qtx || !remSet.has(qtx.txid);
                });
            }

            dirtyBySeg.forEach(function(cols, seg) {
                if (cols.size > 0) restackColumns(seg, cols);
            });
            return;
        }

        case 'block_observed': {
            if (!effect.hash) return;
            reservePendingBlockSlot(_hb, effect.hash, effect.observed_at);
            return;
        }

        case 'block_details': {
            if (!effect.height) return;
            var cueKey = effect.hash || ('h:' + effect.height);
            playBlockCue(cueKey);
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
            // If no measured block segment exists on the timeline yet AND the viewer is reveal-eligible,
            // keep the confirmed bricks intact (fadeStart === 0) on preFlatline/liveSeg and tag them
            // with _pendingInclusionHash so block_details puts them into reveal.confirmed and the
            // reveal's harvest beat visibly shatters them into the new spike.
            var blockSeg = findBlockSegment(_hb, effect.hash, effect.height);
            var pendingIdx = findPendingBlockIndex(_hb, effect.hash);
            var pendingSeg = pendingIdx >= 0 ? _hb.timeline[pendingIdx] : null;
            var revealEligible = !isHidden && isRevealEligible(_hb);

            if (!blockSeg && revealEligible) {
                for (var pci = 0; pci < effect.confirmed_txids.length; pci++) {
                    var pctxid = effect.confirmed_txids[pci];
                    if (!pctxid) continue;
                    _hb._pendingAdds.delete(pctxid);
                    if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(pctxid);
                    if (_hb._deferredDetails) _hb._deferredDetails.delete(pctxid);

                    var pBlip = null;
                    for (var pfi = 0; pfi < activeFlatlines.length; pfi++) {
                        if (activeFlatlines[pfi]._blipsByTxid.has(pctxid)) {
                            pBlip = activeFlatlines[pfi]._blipsByTxid.get(pctxid);
                            break;
                        }
                    }
                    if (!pBlip) pBlip = findInFlightBlip(_hb, pctxid);
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
            // resurrect them, purge in-flight bricks, and shatter active bricks immediately.
            var targetSeg = blockSeg || pendingSeg;
            var absorbX = targetSeg ? (targetSeg.x_start + targetSeg.x_end) / 2 : _hb.virtualX;
            var absorbNow = Date.now() / 1000;
            var zoom = _hb.zoom || 1;
            var maxPartsPerBlip = (zoom < 0.5 || confSet.size > 1000) ? 1
                : (zoom < 1.0 || confSet.size > 500) ? 2 : 3;
            var viewLeft = _hb.viewOffset || 0;
            var viewRight = viewLeft + (_hb.width || 800) / zoom;
            var incDirtyBySeg = new Map();

            for (var ci = 0; ci < effect.confirmed_txids.length; ci++) {
                var ctxid = effect.confirmed_txids[ci];
                if (!ctxid) continue;
                _hb._removedTxids.add(ctxid);
                _hb._pendingAdds.delete(ctxid);
                if (_hb._hiddenTxMap) _hb._hiddenTxMap.delete(ctxid);
                if (_hb._deferredDetails) _hb._deferredDetails.delete(ctxid);

                for (var ifi = 0; ifi < activeFlatlines.length; ifi++) {
                    var incSeg = activeFlatlines[ifi];
                    if (incSeg._blipsByTxid.has(ctxid)) {
                        var cBlip = incSeg._blipsByTxid.get(ctxid);
                        incSeg._blipsByTxid.delete(ctxid);
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
                            var idCols = incDirtyBySeg.get(incSeg);
                            if (!idCols) { idCols = new Set(); incDirtyBySeg.set(incSeg, idCols); }
                            idCols.add(cBlip.gridX);
                        }
                    }
                }
            }

            purgeInFlightTxids(_hb, effect.confirmed_txids);

            if (_hb._txBatchQueue.length > 0) {
                _hb._txBatchQueue = _hb._txBatchQueue.filter(function(qtx) {
                    return !qtx || !confSet.has(qtx.txid);
                });
            }

            incDirtyBySeg.forEach(function(cols, seg) {
                if (cols.size > 0) restackColumns(seg, cols);
            });
            return;
        }

        case 'block_disconnected': {
            if (!effect.hash) return;
            if (_hb._reveal && _hb._reveal.hash === effect.hash && window._hbEndReveal) {
                window._hbEndReveal();
            }
            _hb._exactConfirmedByHash.delete(effect.hash);
            for (var dfi2 = 0; dfi2 < activeFlatlines.length; dfi2++) {
                var afSeg = activeFlatlines[dfi2];
                if (afSeg.blips) {
                    for (var pdi = 0; pdi < afSeg.blips.length; pdi++) {
                        var pdb = afSeg.blips[pdi];
                        if (pdb && pdb._pendingInclusionHash === effect.hash) {
                            pdb._pendingInclusionHash = null;
                            pdb._confirmedHeight = 0;
                        }
                    }
                }
            }
            if (Array.isArray(_hb._blockQueue) && _hb._blockQueue.length > 0) {
                _hb._blockQueue = _hb._blockQueue.filter(function(qb) {
                    return !qb || qb.hash !== effect.hash;
                });
            }
            var removedConfirmed = false;
            if (Array.isArray(_hb.recentBlocks)) {
                var beforeLen = _hb.recentBlocks.length;
                _hb.recentBlocks = _hb.recentBlocks.filter(function(rb) {
                    return rb.hash !== effect.hash;
                });
                if (_hb.recentBlocks.length !== beforeLen) removedConfirmed = true;
            }
            for (var bi = _hb.timeline.length - 1; bi >= 0; bi--) {
                var seg = _hb.timeline[bi];
                if ((seg.type === 'pending_block' || seg.type === 'block') && seg.hash === effect.hash) {
                    var segWidth = Math.max(0, (seg.x_end || seg.x_start) - seg.x_start);
                    var segEnd = seg.x_end !== null ? seg.x_end : (seg.x_start + segWidth);
                    if (seg.type === 'block') {
                        removedConfirmed = true;
                        if (_hb.hoveredBlock === seg) _hb.hoveredBlock = null;
                    } else {
                        if (_hb.hoveredPendingBlock === seg) _hb.hoveredPendingBlock = null;
                    }
                    _hb.timeline.splice(bi, 1);
                    mergeAdjacentFlatlines(_hb, bi, segWidth, segEnd);
                    break;
                }
            }
            if (_hb._pinnedTxid) {
                _hb._pinnedBlip = lookupBlipOnTimeline(_hb, _hb._pinnedTxid);
            }
            // Refresh lastBlockTime from the new latest remaining block segment (wall-clamped).
            var latestTs = 0;
            var discNow = Math.floor(Date.now() / 1000);
            for (var li = _hb.timeline.length - 1; li >= 0; li--) {
                if (_hb.timeline[li].type === 'block') {
                    var wallTs = effectiveBlockWallTime(_hb.timeline[li], discNow);
                    if (wallTs > 0) {
                        latestTs = wallTs;
                        break;
                    }
                }
            }
            if (latestTs > 0) _hb.lastBlockTime = latestTs;
            else if (removedConfirmed) _hb.lastBlockTime = 0;
            if (removedConfirmed && window.renderRhythmStrip && window.getHeartbeatRecentBlocks) {
                window.renderRhythmStrip('rhythm-strip-canvas', window.getHeartbeatRecentBlocks());
            }
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
    if (_hb._viewMode === 'history' && typeof window.hbV2ExitHistoryMode === 'function') {
        window.hbV2ExitHistoryMode();
    }

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
    _hb.zoom = Math.max(_hb.zoom || 1, 4.2);
    _hb.viewOffset = blip.x - ((_hb.width || 800) * 0.5) / _hb.zoom;
    _hb.viewOffsetY = 0;
    _hb._pinnedTxid = blip.txid;
    _hb._pinnedBlip = blip;
    _hb._searchHighlight = {
        txid: blip.txid,
        blip: blip,
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
    if (!_hb || !_hb.timeline || height == null || height < 0) return false;
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

export function hbV2FocusPendingSlot(hash) {
    var _hb = getState();
    if (!_hb) return false;
    if (_hb._viewMode === 'history' && typeof window.hbV2ExitHistoryMode === 'function') {
        window.hbV2ExitHistoryMode();
    }
    if (!_hb.timeline) return false;
    var target = null;
    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        var seg = _hb.timeline[i];
        if (seg.type === 'pending_block' && (!hash || seg.hash === hash)) {
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
    _hb.hoveredPendingBlock = target;
    return true;
}

export function hbV2IsPendingSlotOffscreen(hash) {
    var _hb = getState();
    if (!_hb || !_hb.timeline || _hb._viewMode === 'history') return false;
    var viewLeft = _hb.viewOffset;
    var viewRight = viewLeft + (_hb.width || 800) / Math.max(_hb.zoom || 1, 0.01);
    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        var seg = _hb.timeline[i];
        if (seg.type === 'pending_block' && (!hash || seg.hash === hash)) {
            return seg.x_end < viewLeft || seg.x_start > viewRight;
        }
    }
    return false;
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
    // the reveal first if a genuinely NEW block (or same-height reorg hash) arrives mid-reveal.
    if (_hb._reveal && block && (block.hash || block.height)) {
        var sameRevealBlock = (block.hash && _hb._reveal.hash && _hb._reveal.hash === block.hash)
            || (!block.hash && block.height && _hb._reveal.height === block.height);
        if (sameRevealBlock) return;
        if (window._hbEndReveal) window._hbEndReveal();
    }

    _hb._backlogSpreadPx = 0;
    _hb._backlogSpreadStart = 0;

    // Inspection continuity rule: never hijack an inspecting, pinned, or history-mode viewer's camera.
    var revealEligible = isRevealEligible(_hb);

    var pendingIdx = block.hash ? findPendingBlockIndex(_hb, block.hash) : -1;

    // Collapse any empty processing gap past the rightmost active brick (direct path only;
    // when a pending_block slot exists, the live flatline holds genuine post-block arrivals).
    var collapsedPx = 0;
    if (pendingIdx < 0) {
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
    }

    var blockTs = block.timestamp || Math.floor(Date.now() / 1000);
    var inter = resolveInterBlockSeconds(block);
    if (inter === null) {
        var scanFrom = pendingIdx >= 0 ? (pendingIdx - 1) : (_hb.timeline.length - 1);
        for (var pi = scanFrom; pi >= 0; pi--) {
            var pSeg = _hb.timeline[pi];
            if (pSeg.type === 'pending_block') {
                break;
            }
            if (pSeg.type === 'block') {
                var prevTs = pSeg.timestamp;
                if (prevTs > 0 && blockTs > 0 && block.height && pSeg.height + 1 === block.height) {
                    inter = blockTs - prevTs;
                }
                break;
            }
        }
    }
    var blockJson = JSON.stringify([{
        hash: block.hash || null,
        height: block.height,
        timestamp: blockTs,
        observed_at: block.observed_at || 0,
        tx_count: block.tx_count || block.confirmed_count || 0,
        total_fees: block.total_fees || 0,
        size: block.size || 0,
        weight: block.weight || 0,
        miner: block.miner || null,
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
    } else if (pendingIdx < 0 && collapsedPx > 40) {
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

    if (!_hb._suppressLiveVisuals) {
        if (window.heartbeatFlash) window.heartbeatFlash();
        if (window.heartbeatPulse) window.heartbeatPulse();
    }

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
    var revealKey = spikeSeg.hash || String(spikeSeg.height);
    if (_hb._lastRevealKey === revealKey) return;
    _hb._lastRevealKey = revealKey;

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
        interBlock: pr.interBlock !== undefined ? pr.interBlock : null,
        leftovers: pr.leftovers || [],
        confirmed: pr.confirmed || [],
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
    var workingZoom = Math.min(Math.max(_hb.zoom, 1.4), 2.2);
    var spikeScreenX = (spikeVX - _hb.viewOffset) * _hb.zoom;
    var headScreenX = _hb.width * HEAD_POSITION_FRAC;
    var nearHead = spikeScreenX > 0 && spikeScreenX < _hb.width
        && Math.abs(spikeScreenX - headScreenX) < _hb.width * 0.35;
    var zoomRatio = Math.max(workingZoom / Math.max(_hb.zoom, 0.001), _hb.zoom / workingZoom);
    if (nearHead && zoomRatio <= 1.25) {
        spikeSeg._strikeStart = Date.now() / 1000;
    } else {
        _hb._reveal._deferStrike = true;
    }

    // If exact block_inclusion arrived before block_details, partition immediately.
    if (spikeSeg.hash && _hb._exactConfirmedByHash && _hb._exactConfirmedByHash.has(spikeSeg.hash)) {
        applyExactHarvest(_hb._reveal, _hb._exactConfirmedByHash.get(spikeSeg.hash).set);
    }
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
    if (hasTimeSpan) {
        // Allow up to 5m of slack before windowStartSec (covers future-skewed miner
        // block headers like +4m44s) without letting a single 25m-old rebroadcast
        // stretch tsSpan and carve an empty void on the left of the gap.
        var floorTs = Math.max(maxTs - 1800, windowStartSec - 300);
        for (var ti0 = 0; ti0 < buffered.length; ti0++) {
            var fts0 = buffered[ti0].first_seen || 0;
            if (fts0 > floorTs && fts0 < minTs) {
                minTs = fts0;
            }
        }
    } else {
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
        var drainSoftMax = Math.min(maxStack, 145);
        var drainHardMax = Math.min(maxStack, 180);
        var drainSearch = Math.max(32, Math.min(120, Math.floor(numCols * 0.25)));
        var blip = buildBlip(tx, gx, gx, 0, nowSec, false);
        var dSlot = findColumnWithRoom(
            _hb, liveSeg, gx, minGridX, endGx, blip.brickH,
            drainSoftMax, drainHardMax, drainSearch, true
        );
        endGx = dSlot.maxGx;
        blip.x = dSlot.gx;
        blip.gridX = dSlot.gx;
        blip.stackY = dSlot.sy;
        blip.height = dSlot.sy + blip.brickH;

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

    // While a block reveal is in form/harvest/unfurl/hold, hold incoming live
    // arrivals briefly so they don't pop onto the live tip off-camera; once the
    // camera begins zooming back in (return) and for 2.5s after, trickle-drain
    // them smoothly across frames so the tip comes alive gently with zero dump.
    var nowSec = Date.now() / 1000;
    var revealTrickle = false;
    if (nowMsOverride === undefined) {
        if (_hb._reveal && _hb._reveal.phase !== 'return') {
            return;
        }
        revealTrickle = !!(_hb._reveal && _hb._reveal.phase === 'return')
            || (_hb._postRevealTrickleUntil > 0 && nowSec < _hb._postRevealTrickleUntil);
    }

    var batch;
    if (revealTrickle && _hb._txBatchQueue.length > 6) {
        var take = Math.max(4, Math.min(18, Math.ceil(_hb._txBatchQueue.length / 18)));
        batch = _hb._txBatchQueue.splice(0, take);
    } else {
        batch = _hb._txBatchQueue;
        _hb._txBatchQueue = [];
    }

    // virtualX is advanced ONLY by drawFrame (single smooth 60fps clock).
    // Never jump _hb.virtualX here; spread live bursts gently behind the head.
    var maxStack = (_hb.height || 400) * 0.35;
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
    // virtualX is advanced ONLY by drawFrame (single smooth 60fps clock) during
    // normal live streaming so bursts never hitch or jump the camera forward.
    // Allow rightward overflow only on massive (>200-tx) sleep/wake dumps OR when
    // a narrow post-carry pinch (<60px past _carryEndX) would otherwise stack bricks
    // into a 30-brick skyscraper because only 2-3 columns exist.
    var isMassiveWakeDump = batch.length > 200;
    var tightPostCarry = !!(liveSeg._carryEndX && (headGridX - liveMinX) < 60);
    var allowFlushExtend = isMassiveWakeDump || tightPostCarry;
    if (isMassiveWakeDump) {
        var neededBatchSpan = Math.ceil(batch.length / 8) * 5;
        if (headGridX - liveMinX < neededBatchSpan) {
            var growTo = Math.max(_hb.virtualX, liveMinX + neededBatchSpan + 10);
            _hb.virtualX = growTo;
            headGridX = Math.max(minGridX, Math.floor(_hb.virtualX / 5) * 5);
            flatlineWidth = Math.max(0, _hb.virtualX - liveSeg.x_start);
        }
        if (neededBatchSpan > spread) spread = neededBatchSpan;
    }
    var spreadCols = Math.max(1, Math.floor(spread / 5));
    var removedSet = _hb._removedTxids;
    var liveSoftMax = Math.min(maxStack, 130);
    var liveHardMax = Math.min(maxStack, 140);

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

        // If Chrome held SSE deltas while a background tab or sleeping screen was
        // throttled, back-place aged catch-up txs across the elapsed stretch
        // instead of piling >5m wake-up txs into a skyscraper at headGridX.
        var ageSec = (tx.first_seen > 0) ? (nowSec - tx.first_seen) : 0;
        var gridX;
        var animateDrop = true;
        if (revealTrickle) {
            // Smooth feathered trickle during/just after reveal return: drop gently
            // near the live head with animated sky fall instead of popping in abruptly.
            var trSpread = Math.max(4, Math.min(16, spreadCols));
            var trOff = Math.floor(txidJitter(tx.txid) * trSpread);
            gridX = Math.max(liveMinX, headGridX - trOff * 5);
            animateDrop = true;
        } else if (ageSec > 2.5) {
            var clampedAge = Math.min(ageSec, 300);
            var ageCols = Math.round((clampedAge * FLATLINE_PX_PER_SEC) / 5);
            var catchJit = Math.round(txidSkylineOffset(tx.txid) * Math.max(2, Math.min(12, spreadCols * 0.25)));
            gridX = Math.max(liveMinX, Math.min(headGridX, headGridX - (ageCols + catchJit) * 5));
            animateDrop = ageSec < 4.0;
        } else {
            // Deterministic trailing spread behind the live head: calm flushes drop
            // right near the head; bursts fan across trailing columns behind headGridX.
            var offsetCols = spreadCols > 1 ? Math.floor(txidJitter(tx.txid) * spreadCols) : 0;
            gridX = Math.max(liveMinX, headGridX - offsetCols * 5);
        }
        var dropStamp = revealTrickle ? (nowSec + Math.random() * 0.35) : nowSec;
        var blip = buildBlip(tx, gridX, gridX, 0, dropStamp, animateDrop);
        var fSlot = findColumnWithRoom(
            _hb, liveSeg, gridX, liveMinX, headGridX, blip.brickH,
            liveSoftMax, liveHardMax, Math.max(60, spreadCols), allowFlushExtend
        );
        headGridX = fSlot.maxGx;
        blip.x = fSlot.gx;
        blip.gridX = fSlot.gx;
        blip.stackY = fSlot.sy;
        blip.height = fSlot.sy + blip.brickH;

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
            _feedSpan('text-white/75 min-w-0 truncate flex-1 sm:flex-none sm:max-w-[240px]',
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
    row.className = 'flex items-center gap-2 sm:gap-3 px-3 sm:px-4 py-2 border-b border-white/5 text-[11px] sm:text-xs font-mono min-w-0'
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
                    value: tx.value != null ? tx.value : null,
                    firstSeen: tx.first_seen || tx.timestamp || null
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
