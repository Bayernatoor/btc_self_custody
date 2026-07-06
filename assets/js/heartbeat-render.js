// heartbeat-render.js — Main draw loop and all drawing helpers for the heartbeat canvas.

import { getState, COLORS, BG_COLOR, GRID_COLOR, POINT_WIDTH, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, MAX_BLIPS_PER_SEGMENT } from './heartbeat-state.js';
import { computeColor, lerpColor, drawGrid, hexToRgb, lerp, feeRateColor, cellRadiusForVsize, fmtBtc, formatDuration } from './heartbeat-timeline.js';
import { canvasToVirtual, virtualToCanvas, blockAtVirtualX, flatlineAtVirtualX, blipAtCanvasXY } from './heartbeat-blips.js';
import { stopMomentum } from './heartbeat-interaction.js';
import { flushTxBatch } from './heartbeat-sse.js';

// ── Control bar constants ──────────────────────────────────────
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

// ── Helper: rounded rectangle path ────────────────────────────
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

// ── Tooltip drawing ────────────────────────────────────────

// Generic tooltip box: draws a rounded rect with lines of text.
// `anchorY` is the y position to place the box above; `borderColor` sets the border.
export function drawTooltipBox(ctx, lines, canvasX, anchorY, borderColor, opts) {
    var _hb = getState();
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

    // Text — always left-aligned
    ctx.textAlign = 'left';
    ctx.fillStyle = (opts && opts.textColor) || 'rgba(255, 255, 255, 0.85)';
    ctx.font = fontSize;
    for (var i = 0; i < lines.length; i++) {
        ctx.fillText(lines[i], boxX + padding, boxY + padding + (i + 1) * lineH - 3);
    }
}

export function drawTooltip(ctx, seg, canvasX, canvasY, baseline) {
    var lines = [
        'Block #' + seg.height,
        'Txns: ' + seg.tx_count.toLocaleString(),
        'Fees: ' + fmtBtc(seg.total_fees / 100000000) + ' BTC',
        'Wait: ' + formatDuration(seg.inter_block_seconds)
    ];
    if (seg.timestamp) {
        var d = new Date(seg.timestamp * 1000);
        lines.push(d.toISOString().replace('T', ' ').slice(0, 19) + ' UTC');
    }
    drawTooltipBox(ctx, lines, canvasX, canvasY - 12, seg.color);
}

// Blip tooltip: shows fee rate and tx info for a single blip/brick
export function drawBlipTooltip(ctx, blip, canvasX, baseline) {
    var _hb = getState();
    var lines = [];
    if (blip.txid) {
        lines.push('TX: ' + blip.txid.substring(0, 16) + '...');
    } else {
        lines.push('~' + (blip.txCount || 1) + ' tx' + ((blip.txCount || 1) > 1 ? 's' : ''));
    }
    if (blip._confirmedHeight) {
        // This tx just got mined (harvested into the block) while pinned/hovered.
        lines.push('✓ Confirmed in block #' + blip._confirmedHeight);
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
        lines.push('Value: ' + fmtBtc(btcVal) + ' BTC');
    }
    if (blip.notableType) {
        var typeLabels = {
            'whale': 'WHALE',
            'fee_outlier': 'FEE OUTLIER',
            'consolidation': 'CONSOLIDATION',
            'fan_out': 'FAN-OUT (BATCH PAYOUT)',
            'large_inscription': 'LARGE INSCRIPTION',
            'round_number': 'ROUND-NUMBER TRANSFER',
            'op_return_msg': 'OP_RETURN MESSAGE'
        };
        lines.push(typeLabels[blip.notableType] || blip.notableType.toUpperCase());
        // Show USD value for value-centric types (not op_return or fee_outlier where USD is noise)
        if (blip.valueUsd && blip.valueUsd > 100
            && blip.notableType !== 'op_return_msg'
            && blip.notableType !== 'fee_outlier') {
            lines.push('~$' + Math.round(blip.valueUsd).toLocaleString() + ' USD');
        }
        // Show input/output structure for structural types
        if (blip.notableType === 'consolidation' || blip.notableType === 'fan_out') {
            if (blip.inputCount != null && blip.outputCount != null) {
                lines.push(blip.inputCount + ' in -> ' + blip.outputCount + ' out');
            }
        }
        // Show message text for op_return_msg
        if (blip.notableType === 'op_return_msg' && blip.opReturnText) {
            var msg = blip.opReturnText.length > 60
                ? blip.opReturnText.substring(0, 57) + '...'
                : blip.opReturnText;
            lines.push('"' + msg + '"');
        }
    }
    if (blip.timestamp) {
        var d = new Date(blip.timestamp * 1000);
        lines.push(d.toLocaleTimeString());
    }
    // Show link hint for pinned blips with txid
    if (blip.txid && _hb._pinnedBlip === blip) {
        lines.push('\u2192 Click again to view on mempool.space');
    }
    var tooltipColors = {
        'whale': 'rgba(255,215,0,0.8)',
        'fee_outlier': 'rgba(255,68,68,0.8)',
        'consolidation': 'rgba(168,85,247,0.8)',
        'fan_out': 'rgba(0,210,255,0.8)',
        'large_inscription': 'rgba(255,0,200,0.8)',
        'round_number': 'rgba(144,238,144,0.8)',
        'op_return_msg': 'rgba(255,165,0,0.8)'
    };
    var blipColor = (blip.notableType && tooltipColors[blip.notableType])
        ? tooltipColors[blip.notableType]
        : (blip.color ? blip.color + '0.6)' : 'rgba(0,230,118,0.6)');
    // Position tooltip above the blip
    var tipY;
    if (_hb.renderMode === 'bloodstream') {
        // Match the actual cell rendering position (bobPhase + colHash + tubeH scaling)
        var zoom = _hb.zoom;
        var vs = blip.vsize || 200;
        var bR = cellRadiusForVsize(vs);
        var cellR = bR * Math.max(zoom * 0.4, 0.8);
        var tubeH = Math.max(30, cellR * 2.5);
        var bobPhase = blip.bobPhase || 0;
        var colHash = ((blip.gridX || 0) * 0.6180339887) % 1;
        var tubePos = ((bobPhase + colHash * Math.PI * 2) % (Math.PI * 2)) / Math.PI - 1;
        var cellCY = baseline - tubePos * tubeH;
        tipY = cellCY - cellR - 16;
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
export function drawFlatlineTooltip(ctx, info, canvasX, baseline) {
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


// ── Control bar (bottom center) ──────────────────────────────
export function drawControlBar(ctx, w, h) {
    var _hb = getState();
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
        ctx.textBaseline = 'middle';
        ctx.fillText(label, bx + CTRL_BTN_SIZE / 2, btnY + CTRL_BTN_SIZE / 2);

        _hb._ctrlBtns.push({
            id: def.id, tip: def.tip,
            x: bx, y: btnY, w: CTRL_BTN_SIZE, h: CTRL_BTN_SIZE
        });
    }
    ctx.textAlign = 'left';
    ctx.textBaseline = 'alphabetic';

    // Draw tooltip if hovering a control button
    if (_hb._ctrlHover) {
        ctx.font = '11px monospace';
        ctx.fillStyle = 'rgba(255,255,255,0.6)';
        ctx.textAlign = 'center';
        ctx.fillText(_hb._ctrlHover.tip, _hb._ctrlHover.x + CTRL_BTN_SIZE / 2, btnY - 6);
        ctx.textAlign = 'left';
    }
}

// ── Cinematic block-reveal camera ─────────────────────────────
// Block-reveal SEQUENCER. Instead of building the whole post-block state at once
// and animating a camera over the finished picture, this DRIVES the timeline
// mutations on the animation clock so the flow fits the animation. Armed by
// setupBlockReveal (heartbeat-sse.js) on a genuine live block while following an
// overview. `_hb._reveal` holds the state; runBlockReveal runs it from drawFrame.
// Five beats:
//   form    — the spike sits at the CURRENT head (no jump/whip-left). Camera zooms
//             ONTO the head (spike kept AT the head, no leftward jog) so you see the
//             fresh spike ("block found"). No harvest, no fade yet.
//   harvest — the confirmed top-fee bricks fly up into the spike; the leftover
//             mempool fades out in place (oldSeg._revealFade).
//   unfurl  — the leftovers re-lay on the new flatline; the camera follows that
//             placement FRONT rightward WHILE zooming OUT to frame the whole timeline
//             (early = bricks falling up close; end = the whole mempool in view).
//   hold    — hold on the whole timeline (~2s): "here's the size of the mempool".
//   return  — zoom back IN to the head at the ORIGINAL entry zoom — back where you were.
// Works at any entry zoom (form/harvest keep the spike at the head; the unfurl zoom-out
// + return restore the entry framing). At fit-all the unfurl/return are near-no-ops
// (already framing the whole thing). Above REVEAL_MAX_ENTRY_ZOOM (sse.js) the reveal is
// skipped entirely (instant path), as is the case when the reveal toggle is off.
// While a reveal is active it OWNS virtualX + the camera (drawFrame suppresses the
// normal advance/flush/follow). Durations are top-level constants — tune freely. The
// unfurl is RATE-based (scales with the tx count) so the fill pace stays consistent.
var REVEAL_FORM_SECS = 2.5;     // zoom onto the head; the spike appears ("block found")
var REVEAL_HARVEST_SECS = 3.5;  // confirmed bricks fly into the spike; leftovers fade
var REVEAL_RELAY_RATE = 7000;   // bricks/sec laid down in the unfurl beat
var REVEAL_RELAY_MIN = 4.0;     // unfurl beat clamps (seconds)
var REVEAL_RELAY_MAX = 9.0;
var REVEAL_HOLD_SECS = 2.0;     // hold on the whole timeline ("here's the size")
var REVEAL_RETURN_SECS = 2.0;   // zoom back in to the head at the entry zoom
var REVEAL_BANNER2_DELAY = 1.5; // how far into the unfurl the "remaining" banner appears
                                // (keeps the "Block found" banner up a bit longer)
var REVEAL_ZOOM = 1.0;          // close-up working zoom (only zooms IN from further out)
var REVEAL_FIT_MARGIN = 0.82;   // fraction of width the whole timeline fills at unfurl end

// Ease-in-out (smoothstep): 0 at t=0, 1 at t=1, zero slope at both ends — a
// deliberate cinematic glide, vs an abrupt fast start.
function smoothstep(t) { t = t < 0 ? 0 : (t > 1 ? 1 : t); return t * t * (3 - 2 * t); }

// Kick off the harvest: the confirmed bricks (marked but left in place during the
// form beat) begin flying up into the spike from `now`. Kept in the sequencer (not
// pushHeartbeatBlocks) so the shatter is a distinct beat AFTER the spike appears.
function startRevealHarvest(r, now) {
    if (!r || !r.confirmed || r.harvested) return;
    r.harvested = true;
    for (var i = 0; i < r.confirmed.length; i++) {
        var blp = r.confirmed[i];
        if (blp.fadeStart !== 0) continue;
        blp.fadeStart = now;
        blp._confirmedHeight = r.height; // so a pinned/hovered tx shows "confirmed in block #N"
        blp.absorbTargetX = r.absorbX;
        blp.absorbOriginX = blp.x;
        blp.particles = [];
        for (var pi = 0; pi < r.maxParts; pi++) {
            blp.particles.push({
                offsetX: (Math.random() - 0.5) * 8,
                offsetY: Math.random() * (blp.height || 10) * 0.8,
                speed: 0.7 + Math.random() * 0.6,
                arcHeight: 20 + Math.random() * 60,
                delay: Math.random() * 0.3,
                size: 1.5 + Math.random() * 2.5
            });
        }
    }
}

// Detach the faded leftovers from the old segment (keep the shattering confirmed
// ones) and re-lay them on the new flatline, staggered over `staggerSecs`.
function relayRevealLeftovers(_hb, r, staggerSecs) {
    if (r.laid) return;
    r.laid = true;
    if (r.oldSeg) { r.oldSeg.blips = r.oldSeg.blips.filter(function(b) { return b.fadeStart > 0; }); r.oldSeg._revealFade = undefined; }
    if (window._hbPlaceCarried) window._hbPlaceCarried(r.leftovers, staggerSecs);
    // placeCarriedLeftovers sets _hb.virtualX = x_start + width SYNCHRONOUSLY (before
    // its RAF-chunked placement), so virtualX is already the final head here — that
    // synchronous commit is what makes this width read correct. Don't defer it there.
    r.width = Math.max(1, _hb.virtualX - r.newSeg.x_start);
}

// End the reveal: make sure the harvest + leftover placement have actually
// happened (if we abort mid-beat), then clear it. Camera is left where it is — on
// a natural finish it's at the head; on a user takeover the user now owns it.
function endBlockReveal(_hb) {
    var r = _hb._reveal;
    if (!r) return;
    var now = Date.now() / 1000;
    startRevealHarvest(r, now);     // shatter confirmed if we hadn't yet
    relayRevealLeftovers(_hb, r, 0); // lay leftovers instantly if we hadn't yet
    _hb._reveal = null;
}
window._hbEndReveal = function() { var s = getState(); if (s) endBlockReveal(s); };

function runBlockReveal(_hb, dt, now, w) {
    var r = _hb._reveal;
    // Abort the moment the user takes over. NOT gated on autoFollow — the reveal is
    // allowed to run when not head-following (play mode). Drag/pinch/pause are caught
    // here; a wheel-zoom doesn't flip any of these, so the wheel handler ends the
    // reveal explicitly (as do the control buttons).
    if (_hb.isDragging || _hb.paused || _hb._pinching) {
        endBlockReveal(_hb); // user took over — finalize + hand back
        return false;
    }
    var elapsed = now - r.start;
    var working = Math.min(_hb.maxZoom, Math.max(r.entryZoom, REVEAL_ZOOM)); // close-up zoom
    if (r.fromZoom === undefined) { r.fromZoom = _hb.zoom; r.fromOffset = _hb.viewOffset; }
    // Offset that puts a virtual x at the head position (~0.85) for a given zoom.
    function headOff(vx, z) { return vx - (w * HEAD_POSITION_FRAC) / z; }

    if (r.phase === 'form') {
        // "Block #X found" banner, timed to fade out as the harvest ends (the unfurl
        // then swaps in the remaining-count banner). Set once, on the first frame.
        if (!r._bannerBlockSet) {
            r._bannerBlockSet = true;
            var feeBtc = (r.blockFees || 0) / 100000000;
            _hb._announcement = {
                title: 'Block #' + r.height.toLocaleString() + ' found',
                subtitle: (feeBtc > 0 ? feeBtc.toFixed(4) + ' BTC fees · ' : '')
                    + (r.blockTxCount || 0).toLocaleString() + ' txs',
                color: r.blockColor || COLORS.healthy,
                start: now,
                // Linger into the unfurl (by REVEAL_BANNER2_DELAY) so it fades out
                // right as the "remaining" banner appears — no gap, no rush.
                end: now + REVEAL_FORM_SECS + REVEAL_HARVEST_SECS + REVEAL_BANNER2_DELAY
            };
        }
        // Zoom ONTO the head where the spike appeared — keep the spike AT the head
        // (no leftward jog). In the 0.75–2x range this is ~a no-op zoom.
        var tf = smoothstep(elapsed / REVEAL_FORM_SECS);
        _hb.zoom = r.fromZoom + (working - r.fromZoom) * tf;
        _hb.viewOffset = r.fromOffset + (headOff(r.spikeVX, _hb.zoom) - r.fromOffset) * tf;
        if (elapsed >= REVEAL_FORM_SECS) {
            startRevealHarvest(r, now); // the shatter begins as the harvest beat opens
            r.phase = 'harvest'; r.start = now; r.fromZoom = undefined;
        }
    } else if (r.phase === 'harvest') {
        // Hold on the head while the confirmed bricks fly into the spike and the
        // leftover mempool fades out in place.
        var th = smoothstep(elapsed / REVEAL_HARVEST_SECS);
        _hb.zoom = working;
        _hb.viewOffset = headOff(r.spikeVX, working);
        if (r.oldSeg) r.oldSeg._revealFade = 1 - th;
        if (elapsed >= REVEAL_HARVEST_SECS) {
            // Open the unfurl: re-lay the leftovers (staggered over a rate-based
            // duration) and compute the fit-zoom that frames the whole timeline.
            var unfurlSecs = Math.max(REVEAL_RELAY_MIN, Math.min(REVEAL_RELAY_MAX, r.leftovers.length / REVEAL_RELAY_RATE));
            relayRevealLeftovers(_hb, r, unfurlSecs); // sets r.width, virtualX = head
            r.unfurlSecs = unfurlSecs;
            var span = Math.max(1, (r.newSeg.x_start + r.width) - r.spikeVX);
            r.fitZoom = Math.max(_hb.minZoom, Math.min(working, (w * REVEAL_FIT_MARGIN) / span));
            r.phase = 'unfurl'; r.start = now; r.fromZoom = undefined;
        }
    } else if (r.phase === 'unfurl') {
        // Follow the falling-brick FRONT rightward while zooming OUT to frame the
        // whole timeline. Early = bricks falling up close; end = the whole mempool.
        // Ease the offset OUT from where harvest ended (r.fromOffset) so there's no
        // pop as the framing shifts from the spike peak to the advancing front.
        var tu = elapsed / r.unfurlSecs; if (tu > 1) tu = 1;
        var su = smoothstep(tu);
        _hb.zoom = working + (r.fitZoom - working) * su;
        var frontVX = r.newSeg.x_start + tu * r.width;
        _hb.viewOffset = r.fromOffset + (headOff(frontVX, _hb.zoom) - r.fromOffset) * su;
        // A bit into the unfurl, cross-fade to the remaining-count banner (same
        // style) — the mempool after the harvest. r.leftovers is the exact carried
        // set (P5). Persists through the rest of the unfurl + hold, fades on return.
        if (!r._bannerRemainSet && elapsed >= REVEAL_BANNER2_DELAY) {
            r._bannerRemainSet = true;
            _hb._announcement = {
                title: r.leftovers.length.toLocaleString() + ' unconfirmed',
                subtitle: 'transactions remaining in my mempool',
                color: r.blockColor || COLORS.healthy,
                start: now,
                end: now + (r.unfurlSecs - elapsed) + REVEAL_HOLD_SECS + REVEAL_RETURN_SECS
            };
        }
        if (elapsed >= r.unfurlSecs) { r.phase = 'hold'; r.start = now; r.fromZoom = undefined; }
    } else if (r.phase === 'hold') {
        // Hold on the whole timeline — "here's the size of the mempool".
        _hb.zoom = r.fitZoom;
        _hb.viewOffset = headOff(_hb.virtualX, r.fitZoom);
        if (elapsed >= REVEAL_HOLD_SECS) { r.phase = 'return'; r.start = now; r.fromZoom = undefined; }
    } else { // return
        // Zoom back IN to the head at the ORIGINAL entry zoom — back where you were.
        var tr = smoothstep(elapsed / REVEAL_RETURN_SECS);
        _hb.zoom = r.fitZoom + (r.entryZoom - r.fitZoom) * tr;
        _hb.viewOffset = headOff(_hb.virtualX, _hb.zoom);
        if (elapsed >= REVEAL_RETURN_SECS) {
            _hb.zoom = r.entryZoom;
            _hb.viewOffset = headOff(_hb.virtualX, r.entryZoom);
            _hb._reveal = null;
            return false; // back at the head, entry zoom — normal follow resumes
        }
    }
    return true;
}

// ── First-load intro ──────────────────────────────────────────
// On first load, after placeHistoryTxs lays the whole mempool at fit-all, this
// gives a "load reveal": hold on the whole mempool a beat (bricks streaming in),
// then slow-zoom to a resting zoom on the head — a better first impression than a
// static fit-all view. Camera-only (live tx flow keeps running, unlike the block
// reveal). Armed by placeHistoryTxs (heartbeat-sse.js), runs from drawFrame, aborts
// on any user takeover (autoFollow drops / drag / pinch / control press).
var INTRO_HOLD_SECS = 2.0;    // show the whole mempool laying in
var INTRO_ZOOM_SECS = 2.5;    // slow zoom to the head
var INTRO_TARGET_ZOOM = 1.5;  // first-load resting zoom (also gated in placeHistoryTxs)
function runIntro(_hb, dt, now, w) {
    var it = _hb._intro;
    if (!_hb.autoFollow || _hb.isDragging || _hb.paused || _hb._pinching) {
        _hb._intro = null;
        return false; // user took over
    }
    var elapsed = now - it.start;
    if (it.phase === 'hold') {
        // Keep the whole mempool framed (head at 0.85 at the fit zoom) while bricks
        // stream in; virtualX advances live, so track it.
        _hb.viewOffset = _hb.virtualX - (w * HEAD_POSITION_FRAC) / _hb.zoom;
        if (elapsed >= INTRO_HOLD_SECS) { it.phase = 'zoom'; it.start = now; it.fromZoom = _hb.zoom; }
    } else { // zoom
        var t = smoothstep(elapsed / INTRO_ZOOM_SECS);
        _hb.zoom = it.fromZoom + (INTRO_TARGET_ZOOM - it.fromZoom) * t;
        _hb.viewOffset = _hb.virtualX - (w * HEAD_POSITION_FRAC) / _hb.zoom;
        if (elapsed >= INTRO_ZOOM_SECS) {
            _hb.zoom = INTRO_TARGET_ZOOM;
            _hb.viewOffset = _hb.virtualX - (w * HEAD_POSITION_FRAC) / INTRO_TARGET_ZOOM;
            _hb._intro = null;
            return false; // at the head, resting zoom — normal follow resumes
        }
    }
    return true;
}

// ── Main draw loop ─────────────────────────────────────────
export function drawFrame(frameTime) {
    var _hb = getState();
    if (!_hb) return;

    var ctx = _hb.ctx;
    var w = _hb.width;
    var h = _hb.height;
    var baseline = (h < 350 ? h * 0.78 : h * 0.55) + (_hb.viewOffsetY || 0);

    // Compute time delta
    var now = Date.now() / 1000;
    var dt = _hb.lastFrameTime > 0 ? (now - _hb.lastFrameTime) : 0;
    if (dt > 0.5) dt = 0.5; // clamp to avoid huge jumps on tab switch
    _hb.lastFrameTime = now;

    // Safety net: replay any block that arrived while the tab was hidden but
    // wasn't drained by visibilitychange (would otherwise strand — canvas stuck
    // on the old block while the header shows the new one). RAF only runs at full
    // rate when visible, so this catches up within a frame of the tab showing.
    // Cheap guard: the queue is empty in normal operation.
    if (_hb._blockQueue && _hb._blockQueue.length > 0 && !document.hidden && window._hbDrainBlockQueue) {
        window._hbDrainBlockQueue();
    }

    // ── Advance live flatline ──────────────────────────────
    // Suppressed during a block reveal: the sequencer owns virtualX then (the form
    // + harvest beats hold it at the spike, the relay beat advances it as the
    // mempool re-lays).
    var liveSeg = _hb.timeline.length > 0 ? _hb.timeline[_hb.timeline.length - 1] : null;
    if (!_hb._reveal && liveSeg && liveSeg.type === 'flatline' && liveSeg.x_end === null) {
        _hb.virtualX += dt * FLATLINE_PX_PER_SEC;

        // Prune _colHeights every ~30s to prevent unbounded growth.
        // Remove entries more than 2000px behind the viewport.
        if (!_hb._lastColPrune) _hb._lastColPrune = now;
        if (now - _hb._lastColPrune > 30 && liveSeg._colHeights) {
            _hb._lastColPrune = now;
            var pruneThresh = _hb.viewOffset - 2000;
            var keys = Object.keys(liveSeg._colHeights);
            for (var pk = 0; pk < keys.length; pk++) {
                if (Number(keys[pk]) < pruneThresh) {
                    delete liveSeg._colHeights[keys[pk]];
                }
            }
        }
    }

    // Lay down queued mempool-tx bricks once per frame (RAF-driven), spreading
    // the work across frames instead of a 200ms off-RAF burst that hitched the
    // scroll. Placed after the virtualX advance so bricks land at the live head.
    // Suppressed during a block reveal so live txs queue up and flush afterward
    // (they'd otherwise pile at the reveal's controlled head).
    if (!_hb._reveal) flushTxBatch();

    // Block-reveal sequencer takes precedence when armed — it drives virtualX,
    // the leftover placement, zoom AND viewOffset (spike → relay). It returns
    // false when it finishes or the user takes over, falling through to normal
    // handling in the same frame.
    if (_hb._reveal && runBlockReveal(_hb, dt, now, w)) {
        // Sequencer owns the camera this frame.
    } else if (_hb._intro && runIntro(_hb, dt, now, w)) {
        // First-load intro owns the camera this frame.
    } else if (_hb.autoFollow && !_hb.isDragging) {
        var targetOffset = _hb.virtualX - (w * HEAD_POSITION_FRAC) / _hb.zoom;
        var offDiff = targetOffset - _hb.viewOffset;
        // Ease the head-follow so a post-block fast-forward (or a frame-hitch
        // catch-up) slides into place instead of lurching. Snap only for very
        // large repositions (long tab-hide, initial load), where a slide would
        // be a disorienting blur. Frame-rate independent (dt-scaled).
        if (Math.abs(offDiff) > 1500) {
            _hb.viewOffset = targetOffset;
        } else {
            _hb.viewOffset += offDiff * Math.min(1, dt * 8);
        }
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
    // No vertical tremor on long waits — the flatline color shift (via
    // computeColor on elapsed) + the "last block" timer already convey the
    // wait, and a heaving baseline added motion without information.
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
            drawFlatlineSegment(ctx, seg, segEnd, viewLeft, viewRight, baseline, flatColor, seg.x_end === null, now);
        }
    }

    // ── Draw tooltip if hovering ───────────────────────────
    ctx.globalAlpha = 1;
    if (_hb.hoveredBlock) {
        var hSeg = _hb.hoveredBlock;
        var midX = virtualToCanvas((hSeg.x_start + hSeg.x_end) / 2);
        drawTooltip(ctx, hSeg, midX, baseline - 30, baseline);
    } else if (_hb._pinnedBlip && _hb._pinnedBlip.x !== undefined) {
        var pbx = virtualToCanvas(_hb._pinnedBlip.x);
        drawBlipTooltip(ctx, _hb._pinnedBlip, pbx, baseline);
    } else if (_hb.hoveredBlip) {
        drawBlipTooltip(ctx, _hb.hoveredBlip, _hb.hoverCanvasX, baseline);
    } else if (_hb.hoveredFlatline) {
        drawFlatlineTooltip(ctx, _hb.hoveredFlatline, _hb.hoverCanvasX || w / 2, baseline);
    }

    // Zoom indicator is now in HTML control bar

    // Jump to Live is now in the control bar

    // ── Draw search highlight glow ────────────────────────
    if (_hb._searchHighlight) {
        var sh = _hb._searchHighlight;
        var shElapsed = now - sh.start;
        if (shElapsed > sh.duration) {
            _hb._searchHighlight = null;
        } else {
            var shCX = (sh.x - _hb.viewOffset) * _hb.zoom;
            var shCY = (h < 350 ? h * 0.78 : h * 0.55) + (_hb.viewOffsetY || 0);
            // Pulsing glow: alpha fades, radius breathes
            var shFade = 1.0 - shElapsed / sh.duration;
            var shPulse = 1.0 + 0.3 * Math.sin(shElapsed * 4);
            var shRadius = 20 * _hb.zoom * shPulse;
            ctx.save();
            ctx.beginPath();
            ctx.arc(shCX, shCY, shRadius, 0, Math.PI * 2);
            ctx.strokeStyle = 'rgba(0, 230, 118, ' + (shFade * 0.8) + ')';
            ctx.lineWidth = 2;
            ctx.shadowColor = '#00e676';
            ctx.shadowBlur = 15 * shFade;
            ctx.stroke();
            // Inner crosshair
            ctx.beginPath();
            ctx.moveTo(shCX - shRadius * 0.4, shCY);
            ctx.lineTo(shCX + shRadius * 0.4, shCY);
            ctx.moveTo(shCX, shCY - shRadius * 0.4);
            ctx.lineTo(shCX, shCY + shRadius * 0.4);
            ctx.strokeStyle = 'rgba(0, 230, 118, ' + (shFade * 0.5) + ')';
            ctx.lineWidth = 1;
            ctx.shadowBlur = 0;
            ctx.stroke();
            ctx.restore();
        }
    }

    // ── Draw control bar ─────────────────────────────────
    // Control bar is now HTML (outside canvas). Sync button states.
    if (_hb._syncControls) _hb._syncControls();

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

    // SSE disconnection overlay — semi-transparent with mining animation
    if (_hb._sseDisconnected) {
        drawDisconnectedOverlay(ctx, w, h, now);
    }

    _hb.rafId = requestAnimationFrame(drawFrame);
}

// Draw disconnection overlay with animated mining indicator
export function drawDisconnectedOverlay(ctx, w, h, nowSec) {
    var _hb = getState();
    // Semi-transparent dark overlay
    ctx.fillStyle = 'rgba(13, 33, 55, 0.55)';
    ctx.fillRect(0, 0, w, h);

    var cx = w / 2;
    var cy = h / 2 - 20;
    var elapsed = _hb._sseDisconnectedSince > 0 ? nowSec - _hb._sseDisconnectedSince : 0;

    // Animated pulsing block icon
    var pulse = 0.7 + Math.sin(nowSec * 3) * 0.3;
    var blockSize = 24;
    var bx = cx - blockSize / 2;
    var by = cy - blockSize / 2 - 10;

    // Draw a simple block shape (cube-like)
    ctx.globalAlpha = pulse;
    ctx.fillStyle = '#f7931a';
    ctx.fillRect(bx, by, blockSize, blockSize);
    // Top face
    ctx.fillStyle = '#ffb74d';
    ctx.beginPath();
    ctx.moveTo(bx, by);
    ctx.lineTo(bx + 8, by - 8);
    ctx.lineTo(bx + blockSize + 8, by - 8);
    ctx.lineTo(bx + blockSize, by);
    ctx.closePath();
    ctx.fill();
    // Right face
    ctx.fillStyle = '#e65100';
    ctx.beginPath();
    ctx.moveTo(bx + blockSize, by);
    ctx.lineTo(bx + blockSize + 8, by - 8);
    ctx.lineTo(bx + blockSize + 8, by + blockSize - 8);
    ctx.lineTo(bx + blockSize, by + blockSize);
    ctx.closePath();
    ctx.fill();
    ctx.globalAlpha = 1;

    // Show different text for block mining vs SSE disconnect
    var isMining = _hb._blockMiningOverlay || false;
    var dots = '.'.repeat(Math.floor(nowSec * 2) % 4);
    ctx.fillStyle = 'rgba(255, 255, 255, 0.85)';
    ctx.font = '14px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(isMining ? 'Mining' + dots : 'Reconnecting' + dots, cx, cy + 30);

    // Elapsed time (only for reconnecting, mining is expected to be brief)
    if (!isMining && elapsed > 3) {
        ctx.fillStyle = 'rgba(255, 255, 255, 0.5)';
        ctx.font = '11px monospace';
        var elapsedStr = elapsed < 60 ? Math.round(elapsed) + 's' :
            Math.floor(elapsed / 60) + 'm ' + Math.round(elapsed % 60) + 's';
        ctx.fillText('(' + elapsedStr + ')', cx, cy + 52);
    }

    ctx.textAlign = 'start';
    ctx.textBaseline = 'alphabetic';
}

// Draw a single block waveform segment
export function drawBlockSegment(ctx, seg, viewLeft, baseline, fallbackColor) {
    var _hb = getState();
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

// LOD engages only when a segment has more than this many bricks (≈ the mempool
// size) AND is zoomed out. DEFAULT 60000: individual bricks up to ~60k (preferred,
// tested smooth to ~55k), LOD as the safety valve above that. Tune in prod.
var LOD_MIN_BLIPS = 60000;

// ── P2: Level-of-detail blip rendering ────────────────────────
// When zoomed out so far that grid columns are sub-pixel, drawing every brick
// (fillRect + gap math + gradient + outline, per brick, per frame) is the
// dominant cost — tens of thousands of ops that produced the ~141ms RAF
// violation at full-mempool zoom-out. Instead aggregate each segment's bricks
// into per-canvas-column density bars: one bar per pixel column, its height the
// tallest stack in that column, colored by the column's standout tx (notables
// win so they still punch through the ribbon, else the highest-fee brick). At
// this scale it's visually near-identical to the packed bricks, but the draw is
// O(visible columns) instead of O(bricks). One O(n) aggregation pass replaces
// the O(n) heavy-draw pass, so per-frame cost stays flat as N grows past 45k.
export function drawFlatlineBlipsLOD(ctx, seg, viewLeft, viewRight, baseline, zoom, nowSec) {
    var _hb = getState();
    var blips = seg.blips;
    // Bucket in VIRTUAL space (vx*zoom), not canvas space. Canvas-space buckets
    // shift every frame as the auto-follow view eases, so bricks near a boundary
    // hop columns frame-to-frame — and since each bar's height is the tallest
    // brick in its column, that hopping made bars jump up/down = the flicker.
    // Virtual buckets are stable under panning; apply the view offset once, here.
    var offPx = _hb.viewOffset * zoom;
    var colW = Math.max(1, 5 * zoom); // grid pitch in canvas px, floored at 1
    var cols = {};                    // stable virtual bucket -> aggregate
    for (var i = 0; i < blips.length; i++) {
        var b = blips[i];
        if (b.fadeStart > 0) continue; // skip absorbing/fading particles
        var vx = b.gridX !== undefined ? b.gridX : b.x;
        if (vx < viewLeft || vx > viewRight) continue;
        // Honour the drop-in: bricks arrive left-to-right on first load
        // (dropDelay baked into timestamp), so the ribbon sweeps in instead of
        // popping up all at once. After load ages are large → fadeIn=1 (static).
        var age = nowSec - b.timestamp;
        if (age < 0) continue; // staggered brick not yet visible
        var fadeIn = age < 0.6 ? age / 0.6 : 1;
        var k = (vx * zoom) | 0;
        var top = (b.height || b.brickH || 4) * fadeIn;
        var c = cols[k];
        if (!c) c = cols[k] = { h: 0, notable: false, ncolor: null, counts: {}, topN: 0, topColor: null };
        if (top > c.h) c.h = top;
        if (b.notableType) {
            // Notables punch through: any notable in the column colours it (they
            // read as the sparse coloured bars among the ribbon, like the
            // individual notable bricks in the zoomed-in view).
            if (!c.notable) { c.notable = true; c.ncolor = b.color; }
        } else {
            // Colour by the DOMINANT (mode) fee tier, so the ribbon reads by the
            // majority of txs in the column (mostly low-fee/blue) rather than its
            // single hottest tx, which biased every column toward red/orange.
            var cc = (c.counts[b.color] || 0) + 1;
            c.counts[b.color] = cc;
            if (cc > c.topN) { c.topN = cc; c.topColor = b.color; }
        }
    }
    // During a block reveal, the old segment's leftovers fade out (harvest beat).
    var rf = seg._revealFade !== undefined ? seg._revealFade : 1;
    if (rf <= 0) return;
    for (var key in cols) {
        var col = cols[key];
        // blip.color is an "rgba(r, g, b, " prefix awaiting an alpha + ")".
        var color = col.notable ? col.ncolor : (col.topColor || 'rgba(0, 230, 118, ');
        ctx.fillStyle = color + (col.notable ? 0.95 : 0.72) * rf + ')';
        ctx.fillRect(Number(key) - offPx, baseline - col.h, colW, col.h);
    }
}

// Draw a flatline segment (with optional blips)
export function drawFlatlineSegment(ctx, seg, segEnd, viewLeft, viewRight, baseline, color, isLive, nowSec) {
    var _hb = getState();
    // Clamp to visible range
    var drawStart = Math.max(seg.x_start, viewLeft);
    var drawEnd = Math.min(segEnd, viewRight);

    if (drawStart >= drawEnd) return;

    var cx1 = virtualToCanvas(drawStart);
    var cx2 = virtualToCanvas(drawEnd);
    var y = baseline;

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
        // Tube scales with zoom so vessel walls match cell spread
        var avgR = 4.0 * Math.max(zoom * 0.4, 0.8);
        var tubeH = Math.max(30, avgR * 2.5);
        // Skip vessel rendering if values are non-finite (canvas resize race)
        // but don't return — blips still need to render below
        if (!isFinite(baseline) || !isFinite(tubeH)) tubeH = 30;
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
        // DEFAULT (2026-07-06): individual bricks are preferred; LOD only engages as
        // a performance safety valve above LOD_MIN_BLIPS (~the mempool size) AND when
        // zoomed out (bricks sub-pixel). Below that we always draw individual bricks
        // — tested smooth to ~55k, and full-fit-at-congestion is rare. The console
        // toggle (_hbToggleLod) forces bricks always (or LOD).
        if (_hb._lodEnabled !== false && 5 * zoom < 1.5 && seg.blips.length > LOD_MIN_BLIPS) {
            drawFlatlineBlipsLOD(ctx, seg, viewLeft, viewRight, baseline, zoom, nowSec);
        } else {
        // P3: for closed (immutable) segments, lazily sort blips by x once and
        // binary-search the visible window so a zoomed-in view of a dense
        // historical region only iterates on-screen bricks. The live (open)
        // segment mutates every flush, so we don't sort it — its per-blip cull
        // below is already cheap.
        var _wStart = 0;
        var _wSorted = false;
        if (seg.x_end !== null && seg.blips.length > 512) {
            if (!seg._sortedByX) {
                seg.blips.sort(function(a, b) { return (a.x || 0) - (b.x || 0); });
                seg._sortedByX = true;
            }
            _wSorted = true;
            var _lo = 0, _hi = seg.blips.length;
            while (_lo < _hi) {
                var _mid = (_lo + _hi) >> 1;
                if ((seg.blips[_mid].x || 0) < viewLeft) _lo = _mid + 1; else _hi = _mid;
            }
            _wStart = _lo;
        }
        for (var bi = _wStart; bi < seg.blips.length; bi++) {
            var blip = seg.blips[bi];
            // Skip blips outside visible range. When sorted, everything past the
            // right edge is too, so stop iterating.
            if (_wSorted && blip.x > viewRight) break;
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
                var bsTubeH = Math.max(30, bsCellR * 2.5);
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
                ctx.fill(); // no shadowBlur (see brick-rendering note)
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

                    // Draw particle (no shadowBlur — see brick-rendering note)
                    ctx.fillStyle = blipColor + pOpacity + ')';
                    ctx.fillRect(px - pSize / 2, py - pSize / 2, pSize, pSize);
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

                // Fade-in: grow from 0. Notable txs get slower, more dramatic drop.
                var age = nowSec - blip.timestamp;
                if (age < 0) continue; // staggered history brick not yet visible
                var isNotable = !!blip.notableType;
                // Fall duration — bumped a touch so the higher drop (below) reads as
                // a graceful rain rather than a streak.
                var fadeTime = isNotable ? 1.1 : 0.8;
                var fadeIn = Math.min(age / fadeTime, 1.0);
                // Drop from above with bounce easing. Raised so bricks visibly rain
                // down (was 50/120 — too subtle at a full mempool). Notable = higher.
                var dropHeight = isNotable ? 220 : 120;
                // Cubic ease-out with subtle overshoot for a bouncy feel.
                var inv = 1 - fadeIn;
                var dropEase = inv * inv * inv;
                // Subtle bounce near landing (last 25%) for all bricks, stronger for notable
                var bounceAmt = isNotable ? 0.35 : 0.15;
                if (fadeIn > 0.75) {
                    var bouncePhase = (fadeIn - 0.75) * 4; // 0..1
                    dropEase += bounceAmt * Math.sin(bouncePhase * Math.PI) * 0.08;
                }
                var dropOffset = _hb.renderMode !== 'bloodstream' ? dropEase * dropHeight : 0;
                bh *= fadeIn;
                bOpacity *= fadeIn;
                // During a block reveal, the old segment's leftovers fade out
                // (harvest beat) before re-laying on the new flatline.
                if (seg._revealFade !== undefined) bOpacity *= seg._revealFade;

                if (_hb.renderMode === 'bloodstream') {
                    // ═══ Blood cell rendering (vein tube) ═══
                    // Cells pack tightly in a tube centered on the flatline
                    var vs = blip.vsize || 200;
                    var baseR = cellRadiusForVsize(vs);
                    // Scale with zoom: gentle at low zoom, grows with zoom at high
                    var cellRadius = baseR * Math.max(zoom * 0.4, 0.8);
                    var cellDiam = cellRadius * 2;

                    // Tube distribution: each cell gets a unique Y position.
                    // Tube scales with zoom so cells don't overlap at high zoom
                    // (cellRadius grows with zoom but tube was fixed at ±30px).
                    var tubeH = Math.max(30, cellRadius * 2.5);
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
                    var lw = zoom > 10 ? 1.5 : zoom > 6 ? 1 : 0.5;
                    // Gap between bricks (mempool.space-style separation) at ALL
                    // zooms, not just high zoom, so dense bricks read as a crisp
                    // grid instead of a blob. Based on the smaller dimension so it
                    // never eats a thin brick; 0 when bricks are sub-pixel small.
                    var minDim = Math.min(bw, bh);
                    var gap = minDim > 2.5 ? Math.min(Math.max(minDim * 0.13, 0.5), 1.1) : 0;
                    var rx = bx - bw / 2 + gap;
                    var ry = baseline - sy - bh + gap - dropOffset;
                    var rw = Math.max(bw - gap * 2, 0.5);
                    var rh = Math.max(bh - gap * 2, 0.5);

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

                    // No shadowBlur. The glow was the dominant render cost (an
                    // offscreen blur pass per brick per frame — the "lag on large
                    // flushes") and it bled across the gaps into a blob. Notables
                    // now stand out via size + colour + a crisp bright outline
                    // (below); normal bricks via colour + the inter-brick gap.
                    if (useRound) {
                        ctx.beginPath();
                        roundRect(ctx, rx, ry, rw, rh, cornerR);
                        ctx.fill();
                    } else {
                        ctx.fillRect(rx, ry, rw, rh);
                    }
                    // Notable txs: draw a second pass for stronger glow + bright outline
                    if (blip.notableType) {
                        // Second glow pass (doubles the bloom)
                        if (useRound) {
                            ctx.beginPath();
                            roundRect(ctx, rx, ry, rw, rh, cornerR);
                            ctx.fill();
                        } else {
                            ctx.fillRect(rx, ry, rw, rh);
                        }
                        // Bright colored outline at all zoom levels
                        ctx.shadowBlur = 0;
                        var outlineColors = {
                            'whale': 'rgba(255, 215, 0, 0.8)',
                            'fee_outlier': 'rgba(255, 68, 68, 0.7)',
                            'consolidation': 'rgba(168, 85, 247, 0.7)',
                            'fan_out': 'rgba(0, 210, 255, 0.7)',
                            'large_inscription': 'rgba(255, 0, 200, 0.7)',
                            'round_number': 'rgba(144, 238, 144, 0.8)',
                            'op_return_msg': 'rgba(255, 165, 0, 0.7)'
                        };
                        ctx.strokeStyle = outlineColors[blip.notableType] || 'rgba(255, 255, 255, 0.5)';
                        // Slightly heavier than before to keep notables prominent
                        // now that the glow halo is gone.
                        ctx.lineWidth = Math.max(1.5, zoom * 0.5);
                        if (useRound) {
                            ctx.beginPath();
                            roundRect(ctx, rx, ry, rw, rh, cornerR);
                            ctx.stroke();
                        } else {
                            ctx.strokeRect(rx, ry, rw, rh);
                        }
                    }
                    ctx.shadowBlur = 0;

                    // Dark outline at 4x+ zoom for clear brick separation
                    if (zoom > 4 && !blip.notableType) {
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
                        var padX = Math.max(6, rw * 0.1);
                        var padY = Math.max(5, rh * 0.12);
                        var fs = Math.min(Math.floor(rh * 0.28), Math.floor(rw * 0.2), 13);
                        if (fs >= 5) {
                            // Fee rate text
                            ctx.font = 'bold ' + fs + 'px monospace';
                            ctx.textAlign = 'left';
                            ctx.textBaseline = 'top';
                            var feeStr = (blip.feeRate ? blip.feeRate.toFixed(1) : '?') + ' s/vB';
                            var textY = ry + padY;
                            // Dark text shadow for readability on any color
                            ctx.fillStyle = 'rgba(0,0,0,0.55)';
                            ctx.fillText(feeStr, rx + padX + 1, textY + 1);
                            ctx.fillStyle = 'rgba(255,255,255,0.95)';
                            ctx.fillText(feeStr, rx + padX, textY);

                            // Value text (second line)
                            if (rh > 24 && fs > 6) {
                                var valFs = fs - 1;
                                ctx.font = valFs + 'px monospace';
                                var valY = textY + fs + Math.max(3, rh * 0.08);
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
                                    ctx.fillText(valText, rx + padX + 1, valY + 1);
                                    ctx.fillStyle = 'rgba(255,255,255,0.8)';
                                    ctx.fillText(valText, rx + padX, valY);
                                }
                            }
                        }
                        ctx.restore();
                    }
                }
            }
            ctx.shadowBlur = 0;
        }
        } // end else (non-LOD per-blip draw)

        // Prune blips — never prune the live (open) flatline so txs persist
        // until the next block. Only prune closed (historical) segments.
        var isLiveSeg = (seg.type === 'flatline' && seg.x_end === null);
        if (!isLiveSeg && seg.blips.length > MAX_BLIPS_PER_SEGMENT) {
            // This block reassigns/reorders seg.blips (the concat below is not
            // x-ordered), so invalidate the P3 sort cache — the next draw will
            // re-sort lazily before binary-searching.
            seg._sortedByX = false;
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
