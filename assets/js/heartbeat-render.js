// heartbeat-render.js: Main draw loop and all drawing helpers for the heartbeat canvas.

import { getState, COLORS, BG_COLOR, GRID_COLOR, POINT_WIDTH, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, MAX_BLIPS_PER_SEGMENT } from './heartbeat-state.js';
import { computeColor, lerpColor, drawGrid, hexToRgb, lerp, feeRateColor, PENDING_FEE_COLOR, UNKNOWN_INTERVAL_COLOR, feeBandLabel, feeBandIndex, formatFeeRate, cellRadiusForVsize, brickHeightScale, fmtBtc, formatDuration, buildFlatlineGradient, waitColor, resolveInterBlockSeconds } from './heartbeat-timeline.js';
import { canvasToVirtual, virtualToCanvas, blockAtVirtualX, flatlineAtVirtualX, blipAtCanvasXY } from './heartbeat-blips.js';
import { stopMomentum } from './heartbeat-interaction.js';
import { flushTxBatch, maybeClearRemovedTxids, compressSealedFlatline } from './heartbeat-sse.js';

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

// Generic tooltip box: draws a rounded rect with lines of text, with 4-sided
// and narrow-viewport clamping (truncating overly long lines cleanly with '...').
// `anchorY` is the y position to place the box above; `borderColor` sets the border.
export function drawTooltipBox(ctx, lines, canvasX, anchorY, borderColor, opts) {
    var _hb = getState();
    var padding = (opts && opts.padding) || 8;
    var lineH = (opts && opts.lineH) || 16;
    var fontSize = (opts && opts.fontSize) || '12px monospace';
    var canvasW = _hb ? _hb.width : 800;
    var canvasH = _hb ? _hb.height : 400;
    var maxBoxW = Math.max(120, canvasW - 12);
    var maxTextW = Math.max(40, maxBoxW - padding * 2);

    ctx.font = fontSize;
    var fittedLines = new Array(lines.length);
    var maxW = 0;
    for (var i = 0; i < lines.length; i++) {
        var rawEntry = lines[i];
        var isObj = rawEntry && typeof rawEntry === 'object';
        var line = String(isObj ? rawEntry.text : rawEntry);
        var lineBold = isObj && !!rawEntry.bold;
        ctx.font = lineBold ? ('bold ' + fontSize) : fontSize;
        var m = ctx.measureText(line).width;
        if (m > maxTextW) {
            var ellipsis = '...';
            var ellW = ctx.measureText(ellipsis).width;
            var lo = 0, hi = line.length;
            while (lo < hi) {
                var mid = (lo + hi + 1) >> 1;
                if (ctx.measureText(line.slice(0, mid)).width + ellW <= maxTextW) lo = mid;
                else hi = mid - 1;
            }
            line = line.slice(0, Math.max(1, lo)) + ellipsis;
            m = ctx.measureText(line).width;
        }
        fittedLines[i] = {
            text: line,
            color: (isObj && rawEntry.color) ? rawEntry.color : ((opts && opts.textColor) || 'rgba(255, 255, 255, 0.85)'),
            bold: lineBold
        };
        if (m > maxW) maxW = m;
    }

    var boxW = Math.min(maxBoxW, maxW + padding * 2);
    var boxH = fittedLines.length * lineH + padding * 2;
    var boxX = canvasX - boxW / 2;
    var boxY = anchorY - boxH;

    // 4-sided clamping within [6, canvasW/H - boxW/H - 6]
    var maxX = Math.max(6, canvasW - boxW - 6);
    var maxY = Math.max(6, canvasH - boxH - 6);
    if (boxX < 6) boxX = 6;
    if (boxX > maxX) boxX = maxX;
    if (boxY < 6) boxY = 6;
    if (boxY > maxY) boxY = maxY;

    // Background
    ctx.fillStyle = 'rgba(10, 25, 41, 0.94)';
    ctx.beginPath();
    roundRect(ctx, boxX, boxY, boxW, boxH, 5);
    ctx.fill();

    // Border
    ctx.strokeStyle = borderColor;
    ctx.lineWidth = 1;
    ctx.beginPath();
    roundRect(ctx, boxX, boxY, boxW, boxH, 5);
    ctx.stroke();

    // Text: always left-aligned
    ctx.textAlign = 'left';
    for (var fi = 0; fi < fittedLines.length; fi++) {
        var fl = fittedLines[fi];
        ctx.font = fl.bold ? ('bold ' + fontSize) : fontSize;
        ctx.fillStyle = fl.color;
        ctx.fillText(fl.text, boxX + padding, boxY + padding + (fi + 1) * lineH - 3);
    }
}

// Wrap a plain-text description into at most `maxLines` lines of ~`maxChars`
// characters on word boundaries so landmark tooltips stay compact and readable.
function wrapTooltipText(text, maxChars, maxLines) {
    if (!text) return [];
    var words = String(text).trim().split(/\s+/);
    var out = [];
    var cur = '';
    for (var i = 0; i < words.length; i++) {
        var w = words[i];
        var cand = cur ? (cur + ' ' + w) : w;
        if (cand.length <= maxChars) {
            cur = cand;
        } else {
            if (cur) out.push(cur);
            cur = w;
            if (out.length >= maxLines - 1) {
                var rest = words.slice(i).join(' ');
                if (rest.length > maxChars) {
                    rest = rest.slice(0, Math.max(1, maxChars - 1)).replace(/\s+\S*$/, '') + '\u2026';
                }
                out.push(rest);
                return out;
            }
        }
    }
    if (cur && out.length < maxLines) out.push(cur);
    return out;
}

export function drawTooltip(ctx, seg, canvasX, canvasY, baseline) {
    var lmMap = (typeof window !== 'undefined' && window._hbLandmarks) ? window._hbLandmarks : null;
    var lm = lmMap && seg.height != null ? lmMap[String(seg.height)] : null;
    var heightLabel = seg.height != null ? seg.height.toLocaleString() : '?';

    var lines = [
        { text: 'Block #' + heightLabel, color: '#ffffff', bold: true }
    ];
    if (lm) {
        lines.push({ text: '\u2605 ' + lm.title, color: lm.color || '#f7931a', bold: true });
        var descLines = wrapTooltipText(lm.description, 40, 2);
        for (var di = 0; di < descLines.length; di++) {
            lines.push({ text: descLines[di], color: 'rgba(255, 255, 255, 0.72)' });
        }
    }
    if (seg.miner && seg.miner !== 'Unknown') {
        lines.push({ text: 'Miner: ' + seg.miner, color: '#38bdf8' });
    }
    lines.push('Txns: ' + (seg.tx_count != null ? seg.tx_count.toLocaleString() : '0'));
    if (seg.weight && seg.weight > 0) {
        var fullPct = Math.min(100, (seg.weight / 4000000) * 100);
        lines.push('Fullness: ' + fullPct.toFixed(1) + '%');
    }
    lines.push('Fees: ' + fmtBtc((seg.total_fees || 0) / 100000000) + ' BTC');
    lines.push('Wait: ' + formatDuration(resolveInterBlockSeconds(seg)));
    if (seg.timestamp) {
        var d = new Date(seg.timestamp * 1000);
        lines.push({ text: d.toISOString().replace('T', ' ').slice(0, 19) + ' UTC', color: 'rgba(255, 255, 255, 0.55)' });
    }
    var _hbt = getState();
    if (_hbt && _hbt._isTouch) {
        lines.push({ text: '\u2192 Tap again for full details', color: 'rgba(247, 147, 26, 0.85)' });
    } else if (lm) {
        lines.push({ text: 'Click spike for full story & details', color: 'rgba(247, 147, 26, 0.85)' });
    }
    drawTooltipBox(ctx, lines, canvasX, canvasY - 12, lm ? (lm.color || '#f7931a') : seg.color);
}

// Pending block reserved-slot tooltip
export function drawPendingBlockTooltip(ctx, seg, canvasX, canvasY) {
    var nowSec = Math.floor(Date.now() / 1000);
    var ago = seg && seg.observed_at ? Math.max(0, nowSec - seg.observed_at) : 0;
    var lines = [
        'Pending Block',
        'Observed ' + formatDuration(ago) + ' ago',
        'Fetching block details...'
    ];
    drawTooltipBox(ctx, lines, canvasX, canvasY - 12, (seg && seg.color) || '#38bdf8');
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
    if (blip.feePending || blip.feeRate == null || !isFinite(blip.feeRate)) {
        lines.push('Fee rate: Pending...');
    } else {
        lines.push('Fee rate: ' + formatFeeRate(blip.feeRate) + ' sat/vB (' + feeBandLabel(blip.feeRate) + ')');
    }
    lines.push('Fee paid: ' + (blip.fee != null ? blip.fee.toLocaleString() + ' sats' : 'Pending...'));
    if (blip.vsize != null && blip.vsize > 0) {
        lines.push('Size: ' + Math.round(blip.vsize) + ' vB');
    }
    if (blip.value != null && blip.value > 0) {
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
            if (blip.inputCount > 0 && blip.outputCount > 0) {
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
    if (blip.firstSeen && blip.firstSeen > 0) {
        var d = new Date(blip.firstSeen * 1000);
        var nowD = new Date();
        var sameDay = d.getFullYear() === nowD.getFullYear()
            && d.getMonth() === nowD.getMonth()
            && d.getDate() === nowD.getDate();
        if (sameDay) {
            lines.push('First seen: ' + d.toLocaleTimeString());
        } else {
            var dateOpts = d.getFullYear() === nowD.getFullYear()
                ? { month: 'short', day: 'numeric' }
                : { month: 'short', day: 'numeric', year: 'numeric' };
            lines.push('First seen: ' + d.toLocaleDateString(undefined, dateOpts) + ', ' + d.toLocaleTimeString());
        }
    } else {
        lines.push('First seen: Already waiting (time unknown)');
    }
    // Show the "open detail" hint only when the live PinnedTxPanel callback isn't wired.
    if (blip.txid && _hb._pinnedBlip === blip && typeof window.__hbSelectTx !== 'function') {
        lines.push(_hb._isTouch ? '\u2192 Tap again for details' : '\u2192 Click again to view on mempool.space');
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
        var heightScale = brickHeightScale(_hb.zoom);
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
    if (info.pendingNext) {
        if (info.prevBlock) {
            lines.push('After block #' + info.prevBlock.height);
        }
        lines.push('Pending block \u00b7 fetching details');
        var pendingTxs = 0;
        if (info.segment.blips) {
            for (var pt = 0; pt < info.segment.blips.length; pt++) {
                if (!info.segment.blips[pt]._removed && info.segment.blips[pt].fadeStart === 0) {
                    pendingTxs += info.segment.blips[pt].txCount || 1;
                }
            }
        }
        if (pendingTxs > 0) lines.push('~' + pendingTxs.toLocaleString() + ' txs in pre-block mempool');
    } else if (info.prevBlock && info.nextBlock) {
        var interval = (info.prevBlock.timestamp && info.nextBlock.timestamp)
            ? (info.nextBlock.timestamp - info.prevBlock.timestamp)
            : resolveInterBlockSeconds(info.nextBlock);
        lines.push('Block #' + info.prevBlock.height + ' \u2192 #' + info.nextBlock.height);
        lines.push('Interval: ' + formatDuration(interval));
    } else if (info.prevBlock && !info.nextBlock) {
        var elapsed = info.prevBlock.timestamp
            ? Math.max(0, Math.floor(Date.now() / 1000 - info.prevBlock.timestamp))
            : null;
        var totalTxs = 0;
        if (info.segment.blips) {
            for (var t = 0; t < info.segment.blips.length; t++) {
                if (!info.segment.blips[t]._removed && info.segment.blips[t].fadeStart === 0) {
                    totalTxs += info.segment.blips[t].txCount || 1;
                }
            }
        }
        lines.push('After block #' + info.prevBlock.height);
        lines.push('Waiting: ' + formatDuration(elapsed));
        if (totalTxs > 0) lines.push('~' + totalTxs.toLocaleString() + ' new txs in mempool');
    } else {
        var segInter = resolveInterBlockSeconds(info.segment);
        if (info.nextBlock && segInter !== null) {
            lines.push('Before block #' + info.nextBlock.height);
            lines.push('Interval: ' + formatDuration(segInter));
        } else {
            lines.push('Waiting for blocks...');
        }
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
//   form: the spike sits at the CURRENT head (no jump/whip-left). Camera zooms
//             ONTO the head (spike kept AT the head, no leftward jog) so you see the
//             fresh spike ("block found"). No harvest, no fade yet.
//   harvest: the confirmed top-fee bricks fly up into the spike; the leftover
//             mempool fades out in place (oldSeg._revealFade).
//   unfurl: the leftovers re-lay on the new flatline; the camera follows that
//             placement FRONT rightward WHILE zooming OUT to frame the whole timeline
//             (early = bricks falling up close; end = the whole mempool in view).
//   hold: hold on the whole timeline (~2s): "here's the size of the mempool".
//   return: zoom back IN to the head at the ORIGINAL entry zoom, back where you were.
// Works at any entry zoom (form/harvest keep the spike at the head; the unfurl zoom-out
// + return restore the entry framing). At fit-all the unfurl/return are near-no-ops
// (already framing the whole thing). Above REVEAL_MAX_ENTRY_ZOOM (sse.js) the reveal is
// skipped entirely (instant path), as is the case when the reveal toggle is off.
// While a reveal is active it OWNS virtualX + the camera (drawFrame suppresses the
// normal advance/flush/follow). Durations are top-level constants, tune freely. The
// unfurl is RATE-based (scales with the tx count) so the fill pace stays consistent.
var REVEAL_FORM_SECS = 2.5;     // zoom onto the head; the spike appears ("block found")
var REVEAL_HARVEST_SECS = 3.5;  // confirmed bricks fly into the spike; leftovers fade
var REVEAL_RELAY_RATE = 7000;   // bricks/sec laid down in the unfurl beat
var REVEAL_RELAY_MIN = 4.0;     // unfurl beat clamps (seconds)
var REVEAL_RELAY_MAX = 9.0;
var REVEAL_HOLD_SECS = 2.0;     // hold on the whole timeline ("here's the size")
var REVEAL_RETURN_SECS = 2.8;   // smooth unhurried exponential zoom back in to the live head
var REVEAL_BANNER2_DELAY = 1.5; // how far into the unfurl the "remaining" banner appears
                                // (keeps the "Block found" banner up a bit longer)
var REVEAL_ZOOM = 1.0;          // close-up working zoom (only zooms IN from further out)
var REVEAL_FIT_MARGIN = 0.82;   // fraction of width the whole timeline fills at unfurl end

// Ease-in-out (smoothstep): 0 at t=0, 1 at t=1, zero slope at both ends, a
// deliberate cinematic glide, vs an abrupt fast start.
function smoothstep(t) { t = t < 0 ? 0 : (t > 1 ? 1 : t); return t * t * (3 - 2 * t); }
// smootherstep (Ken Perlin's 6t^5-15t^4+10t^3): like smoothstep but with zero
// FIRST AND SECOND derivative at both ends, so the ease in/out is even gentler,
// used for the reveal's big camera moves (pan-out + zoom-back) so they glide
// rather than ramp.
function smootherstep(t) { t = t < 0 ? 0 : (t > 1 ? 1 : t); return t * t * t * (t * (t * 6 - 15) + 10); }
// easeOutBack: 0 -> 1 with an overshoot past 1 near the end, settling exactly to 1.
// Gives the spike its "BAM": shoots up, overshoots, snaps to rest.
function easeOutBack(t) {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    var c1 = 1.70158, c3 = c1 + 1, x = t - 1;
    return 1 + c3 * x * x * x + c1 * x * x;
}

// ── Block-spike strike ("BAM") ────────────────────────────────
// A live block's spike shoots up from the baseline with an overshoot, plus a
// radiating shockwave from the peak. Driven by seg._strikeStart (set on live
// blocks in pushHeartbeatBlocks) so it plays on both the cinematic reveal AND the
// instant path, and never on history/replay (which have no _strikeStart).
var SPIKE_STRIKE_SECS = 0.75;     // total strike duration (build + snap + settle)
var SPIKE_BAM_AT = 0.6;           // fraction of the strike spent on the slow build
                                  // before the snap; the shockwave/flash fire here
var SPIKE_SHOCKWAVE_SECS = 0.6;   // expanding ring lifetime (from the BAM moment)
var SPIKE_SHOCKWAVE_MAX_R = 130;  // ring max radius, screen px (zoom-independent)
// First-load fade: the whole timeline (grid + spikes + bricks) eases up from the
// background over this window instead of hard-cutting in. Paired with the brick
// drop-stagger, the structure fades in and the txs settle onto it.
var LOAD_FADE_SECS = 2.5; // extends past the loader + the ~1.5s brick drop-in so it reads;
                          // nudge up for more drama, down for snappier

// Spike strike shape: a slow, accelerating BUILD to a low level for the first
// SPIKE_BAM_AT of the duration (you see the wave forming), then a snap up past full
// with overshoot (the "BAM"), settling to exactly 1. Continuous at the seam.
function spikeStrike(t) {
    if (t <= 0) return 0;
    if (t >= 1) return 1;
    var BUILD_TO = 0.32;
    if (t < SPIKE_BAM_AT) {
        var u = t / SPIKE_BAM_AT;
        return BUILD_TO * u * u; // ease-in build, slow then quickening
    }
    var v = (t - SPIKE_BAM_AT) / (1 - SPIKE_BAM_AT);
    return BUILD_TO + (1 - BUILD_TO) * easeOutBack(v); // snap + overshoot
}

// Two expanding, fading rings from the impact point, the shockwave. globalAlpha
// (save/restore) so it works with any strokeStyle color format.
function drawShockwave(ctx, cx, cy, t, color) {
    var eo = 1 - (1 - t) * (1 - t); // easeOut on the radius
    var r = SPIKE_SHOCKWAVE_MAX_R * eo;
    ctx.save();
    ctx.strokeStyle = color;
    ctx.globalAlpha = (1 - t) * 0.55;
    ctx.lineWidth = Math.max(1, 3 * (1 - t));
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.stroke();
    ctx.globalAlpha = (1 - t) * 0.3;
    ctx.beginPath();
    ctx.arc(cx, cy, r * 0.62, 0, Math.PI * 2);
    ctx.stroke();
    ctx.restore();
}

// Kick off the harvest: the confirmed bricks (marked but left in place during the
// form beat) begin flying up into the spike from `now`. Kept in the sequencer (not
// pushHeartbeatBlocks) so the shatter is a distinct beat AFTER the spike appears.
function startRevealHarvest(r, now) {
    if (!r || !r.confirmed || r.harvested) return;
    r.harvested = true;
    for (var i = 0; i < r.confirmed.length; i++) {
        var blp = r.confirmed[i];
        if (!blp || blp._removed || blp.fadeStart !== 0) continue;
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
// ones) and re-lay leftovers on the new flatline, staggered over `staggerSecs`.
// Compression of `r.oldSeg` is intentionally deferred until reveal completion
// (`return` end / `endBlockReveal`) so historical spikes never teleport onto the
// left half of the viewport mid-reveal while `unfurl` is playing.
function relayRevealLeftovers(_hb, r, staggerSecs) {
    if (r.laid) return;
    r.laid = true;
    if (r.leftovers) {
        r.leftovers = r.leftovers.filter(function(b) {
            return b && !b._removed && (!(_hb._removedTxids && b.txid && _hb._removedTxids.has(b.txid)));
        });
    }
    r._revealByTxid = null;
    if (r.oldSeg) {
        r.oldSeg.blips = r.oldSeg.blips.filter(function(b) { return b && !b._removed && b.fadeStart > 0; });
        r.oldSeg._revealFade = undefined;
    }
    if (r.leftovers && r.leftovers.length > 0 && window._hbPlaceCarried) {
        window._hbPlaceCarried(r.leftovers, staggerSecs);
    }
    // placeCarriedLeftovers sets _hb.virtualX = x_start + width SYNCHRONOUSLY (before
    // its RAF-chunked placement), so virtualX is already the final head here, that
    // synchronous commit is what makes this width read correct. Don't defer it there.
    r.width = Math.max(1, _hb.virtualX - (r.newSeg ? r.newSeg.x_start : 0));
}

function finalizeRevealCompression(_hb, r) {
    if (!r || !r.oldSeg || r.oldSeg._compressed) return;
    var spikeIdx = r.spikeSeg ? _hb.timeline.indexOf(r.spikeSeg) : (_hb.timeline.indexOf(r.oldSeg) + 1);
    compressSealedFlatline(_hb, r.oldSeg, spikeIdx, r.interBlock);
}

// End the reveal: make sure the harvest + leftover placement have actually
// happened (if we abort mid-beat), then clear it. Camera is left where it is, on
// a natural finish it's at the head; on a user takeover the user now owns it.
function endBlockReveal(_hb, restoreLiveCamera) {
    var r = _hb._reveal;
    if (!r) return;
    var now = Date.now() / 1000;
    // Fire a still-deferred strike so an aborted-mid-travel spike doesn't stay flat.
    if (r._deferStrike && r.spikeSeg && !r.spikeSeg._strikeStart) {
        r.spikeSeg._strikeStart = now;
    }
    startRevealHarvest(r, now);     // shatter confirmed if we hadn't yet
    relayRevealLeftovers(_hb, r, 0); // lay leftovers instantly if we hadn't yet
    if (r.newSeg) r.newSeg._revealLineEnd = undefined;
    finalizeRevealCompression(_hb, r);
    if (restoreLiveCamera && _hb.autoFollow && !_hb._pinnedTxid && r.entryZoom > 0 && _hb.width > 0) {
        _hb.zoom = r.entryZoom;
        _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
    }
    _hb._reveal = null;
    _hb._postRevealTrickleUntil = now + 2.5;
    maybeClearRemovedTxids(_hb);
}
window._hbEndReveal = function(restoreLiveCamera) { var s = getState(); if (s) endBlockReveal(s, !!restoreLiveCamera); };

// Exponential (geometric) zoom interpolation: constant perceptual zoom speed across
// wide ratios (e.g. 1.6x <-> 0.04x) without the slam/whip of linear zoom lerps.
function lerpZoomExp(fromZ, toZ, t) {
    var a = Math.max(0.001, fromZ);
    var b = Math.max(0.001, toZ);
    if (Math.abs(a - b) < 1e-6) return b;
    return a * Math.pow(b / a, t);
}

function runBlockReveal(_hb, dt, now, w) {
    var r = _hb._reveal;
    // Abort the moment the user takes over. NOT gated on autoFollow, the reveal is
    // allowed to run when not head-following (play mode). Drag/pinch/pause are caught
    // here; a wheel-zoom doesn't flip any of these, so the wheel handler ends the
    // reveal explicitly (as do the control buttons).
    if (_hb.isDragging || _hb.paused || _hb._pinching) {
        endBlockReveal(_hb); // user took over — finalize + hand back
        return false;
    }
    var elapsed = now - r.start;
    // Frame the spike + shattering bricks at a crisp close-up working zoom (1.4x-2.2x)
    // whether the viewer started zoomed way out (0.05x) or zoomed way in (12x).
    var working = Math.min(_hb.maxZoom, Math.min(Math.max(r.entryZoom, 1.4), 2.2));
    if (r.fromZoom === undefined) { r.fromZoom = _hb.zoom; r.fromOffset = _hb.viewOffset; }
    // Offset that puts a virtual x at a given viewport fraction for zoom z.
    var liveFrac = _hb._liveHeadFrac || HEAD_POSITION_FRAC;
    var spikeFrac = Math.min(liveFrac, 0.62); // frame the spike near center-right so left absorption & right unfurl both shine
    function fracOff(vx, z, frac) { return vx - (w * frac) / z; }
    function headOff(vx, z) { return fracOff(vx, z, liveFrac); }

    if (r.phase === 'form') {
        // Keep the post-spike timeline cleanly hidden until unfurl eases it out.
        if (r.newSeg) r.newSeg._revealLineEnd = r.newSeg.x_start;
        // When already near the spike, give just enough time for the 0.75s strike + BAM
        // (0.9s) before immediately sucking confirmed bricks in; when traveling from far
        // zoom, glide smoothly onto the spike over 1.8s then fire the strike.
        var formSecs = r._deferStrike ? 1.8 : 0.9;
        // "Block #X found" banner, timed to fade out as the harvest ends (the unfurl
        // then swaps in the remaining-count banner). Set once, on the first frame.
        if (!r._bannerBlockSet) {
            r._bannerBlockSet = true;
            var feeBtc = (r.blockFees || 0) / 100000000;
            _hb._announcement = {
                title: 'Block #' + r.height.toLocaleString() + ' found',
                subtitle: (r.blockTxCount || 0).toLocaleString() + ' txs confirmed'
                    + (feeBtc > 0 ? ' \u00b7 ' + feeBtc.toFixed(4) + ' BTC fees' : ''),
                color: r.blockColor || COLORS.healthy,
                start: now,
                // Linger into the unfurl (by REVEAL_BANNER2_DELAY) so it fades out
                // right as the "remaining" banner appears: no gap, no rush.
                end: now + formSecs + REVEAL_HARVEST_SECS + REVEAL_BANNER2_DELAY
            };
        }
        // Exponential zoom + smooth glide onto the spike with zero camera swoop.
        var tf = smoothstep(elapsed / formSecs);
        _hb.zoom = lerpZoomExp(r.fromZoom, working, tf);
        var startFocusVX = r.fromOffset + (w * spikeFrac) / Math.max(0.001, r.fromZoom);
        var focusVX = startFocusVX + (r.spikeVX - startFocusVX) * tf;
        _hb.viewOffset = fracOff(focusVX, _hb.zoom, spikeFrac);
        if (elapsed >= formSecs) {
            // Camera has arrived at the spike. If the strike was deferred (we had to
            // travel here), fire it NOW so the spike pops on arrival, then harvest.
            if (r._deferStrike && r.spikeSeg && !r.spikeSeg._strikeStart) {
                r.spikeSeg._strikeStart = now;
            }
            startRevealHarvest(r, now); // the shatter begins as the harvest beat opens
            r.phase = 'harvest'; r.start = now; r.fromZoom = undefined;
        }
    } else if (r.phase === 'harvest') {
        if (r.newSeg) r.newSeg._revealLineEnd = r.newSeg.x_start;
        // Hold cleanly on the spike while confirmed bricks fly into it and the
        // leftover mempool fades out in place. If the block had 0 confirmed bricks
        // in our mempool (e.g. coinbase-only block), keep the beat crisp instead
        // of holding 2 full seconds over an empty fade.
        var harvestDur = (r.confirmed && r.confirmed.length > 0) ? REVEAL_HARVEST_SECS : 0.65;
        var th = smoothstep(elapsed / harvestDur);
        _hb.zoom = working;
        _hb.viewOffset = fracOff(r.spikeVX, working, spikeFrac);
        if (r.oldSeg) r.oldSeg._revealFade = 1 - th;
        if (elapsed >= harvestDur) {
            // Open the unfurl: re-lay the leftovers (staggered over a rate-based
            // duration) and compute the fit-zoom that frames the whole timeline.
            // If leftovers is empty, skip 'unfurl' cleanly and glide back to live head.
            var hasLeftovers = r.leftovers && r.leftovers.length > 0;
            var unfurlSecs = hasLeftovers
                ? Math.max(REVEAL_RELAY_MIN, Math.min(REVEAL_RELAY_MAX, r.leftovers.length / REVEAL_RELAY_RATE))
                : 0;
            relayRevealLeftovers(_hb, r, unfurlSecs); // lays leftovers, sets r.width, virtualX = head (oldSeg compression deferred to reveal end)
            if (!r.leftovers || r.leftovers.length === 0) {
                r.fitZoom = working;
                r.phase = 'return';
                r.start = now;
                r.fromZoom = undefined;
            } else {
                r.unfurlSecs = unfurlSecs;
                var span = Math.max(1, (r.newSeg.x_start + r.width) - r.spikeVX);
                r.fitZoom = Math.max(_hb.minZoom, Math.min(working, (w * REVEAL_FIT_MARGIN) / span));
                r.phase = 'unfurl'; r.start = now; r.fromZoom = undefined;
            }
        }
    } else if (r.phase === 'unfurl') {
        // Advance virtualX smoothly in real time once leftovers are laid so live
        // post-block arrivals drop continuously with zero jump when return finishes.
        if (dt > 0) _hb.virtualX += dt * FLATLINE_PX_PER_SEC;
        // Follow the falling-brick FRONT rightward while exponentially zooming OUT
        // at constant perceptual speed to frame the spike + entire carried mempool.
        var tu = elapsed / r.unfurlSecs; if (tu > 1) tu = 1;
        var su = smootherstep(tu);
        // Ease the glowing flatline ribbon smoothly rightward from the spike just
        // ahead of the raining brick wave instead of popping it in on frame 0.
        if (r.newSeg) {
            var lineT = smootherstep(Math.min(1, tu * 1.06));
            r.newSeg._revealLineEnd = r.newSeg.x_start + (_hb.virtualX - r.newSeg.x_start) * lineT;
        }
        _hb.zoom = lerpZoomExp(working, r.fitZoom, su);
        var targetEndVX = _hb.virtualX;
        var frontVX = r.spikeVX + (targetEndVX - r.spikeVX) * su;
        var unfurlFrac = spikeFrac + (0.88 - spikeFrac) * su;
        _hb.viewOffset = fracOff(frontVX, _hb.zoom, unfurlFrac);
        // A bit into the unfurl, cross-fade to the remaining-count banner (same
        // style): the mempool after the harvest. r.leftovers is the exact carried
        // set (P5). Persists through the rest of the unfurl + hold, fades on return.
        if (!r._bannerRemainSet && elapsed >= REVEAL_BANNER2_DELAY) {
            r._bannerRemainSet = true;
            var confirmedBlockN = r.blockTxCount || (r.confirmed ? r.confirmed.length : 0);
            var clearedLocalN = r.confirmed ? r.confirmed.length : 0;
            var remainN = r.leftovers ? r.leftovers.length : 0;
            _hb._announcement = {
                title: confirmedBlockN.toLocaleString() + ' confirmed \u00b7 ' + remainN.toLocaleString() + ' remaining',
                subtitle: clearedLocalN > 0
                    ? clearedLocalN.toLocaleString() + ' cleared from my mempool'
                    : 'transactions remaining in my mempool',
                color: r.blockColor || COLORS.healthy,
                start: now,
                end: now + (r.unfurlSecs - elapsed) + REVEAL_HOLD_SECS + REVEAL_RETURN_SECS
            };
        }
        if (elapsed >= r.unfurlSecs) {
            if (r.newSeg) r.newSeg._revealLineEnd = undefined;
            r.phase = 'hold'; r.start = now; r.fromZoom = undefined;
        }
    } else if (r.phase === 'hold') {
        if (r.newSeg) r.newSeg._revealLineEnd = undefined;
        // Keep the live head gliding smoothly at real-time speed while holding.
        if (dt > 0) _hb.virtualX += dt * FLATLINE_PX_PER_SEC;
        // Hold on the whole timeline: "here's the size of the mempool".
        _hb.zoom = r.fitZoom;
        _hb.viewOffset = fracOff(_hb.virtualX, r.fitZoom, 0.88);
        if (elapsed >= REVEAL_HOLD_SECS) { r.phase = 'return'; r.start = now; r.fromZoom = undefined; }
    } else { // return
        if (r.newSeg) r.newSeg._revealLineEnd = undefined;
        // Keep virtualX advancing smoothly at real-time speed right into LIVE handoff
        // so there is zero leap or brick dump when the reveal concludes.
        if (dt > 0) _hb.virtualX += dt * FLATLINE_PX_PER_SEC;
        // Exponentially zoom back IN to the live tip at the ORIGINAL entry zoom.
        var tr = smootherstep(elapsed / REVEAL_RETURN_SECS);
        _hb.zoom = lerpZoomExp(r.fitZoom, r.entryZoom, tr);
        var retFrac = 0.88 + (liveFrac - 0.88) * tr;
        _hb.viewOffset = fracOff(_hb.virtualX, _hb.zoom, retFrac);
        if (elapsed >= REVEAL_RETURN_SECS) {
            // Now that the camera is back at entryZoom on the right-hand live head
            // (with oldSeg off-screen to the left), compress oldSeg silently so zero
            // pixels jump on screen while past live blocks keep progressive spacing.
            finalizeRevealCompression(_hb, r);
            _hb.zoom = r.entryZoom;
            _hb.viewOffset = headOff(_hb.virtualX, r.entryZoom);
            _hb._reveal = null;
            _hb._postRevealTrickleUntil = now + 2.5;
            maybeClearRemovedTxids(_hb);
            return false; // back at the head, entry zoom — normal follow resumes
        }
    }
    return true;
}

// ── First-load intro ──────────────────────────────────────────
// On first load, after placeHistoryTxs lays the whole mempool at fit-all, this
// gives a "load reveal": hold on the whole mempool a beat (bricks streaming in),
// then slow-zoom to a resting zoom on the head: a better first impression than a
// static fit-all view. Camera-only (live tx flow keeps running, unlike the block
// reveal). Armed by placeHistoryTxs (heartbeat-sse.js), runs from drawFrame, aborts
// on any user takeover (autoFollow drops / drag / pinch / control press).
var INTRO_HOLD_SECS = 3.5;    // let the raining brick cascade finish settling before zooming in
var INTRO_ZOOM_SECS = 3.2;    // smooth exponential zoom to the head across wide fit ratios
var INTRO_TARGET_ZOOM = 1.6;  // first-load resting zoom (5 * 1.6 = 8px exact column pitch)
function runIntro(_hb, dt, now, w) {
    var it = _hb._intro;
    if (!_hb.autoFollow || _hb.isDragging || _hb.paused || _hb._pinching) {
        _hb._intro = null;
        return false; // user took over
    }
    var elapsed = now - it.start;
    if (it.phase === 'hold') {
        // Keep the whole mempool framed while bricks stream in; wait until both
        // INTRO_HOLD_SECS has elapsed and any chunked snapshot placement has
        // finished on slower mobile devices before starting the zoom glide.
        _hb.viewOffset = _hb.virtualX - (w * HEAD_POSITION_FRAC) / _hb.zoom;
        if (elapsed >= INTRO_HOLD_SECS && !_hb._pendingSnapshot) {
            it.phase = 'zoom';
            it.start = now;
            it.fromZoom = _hb.zoom;
        }
    } else { // zoom
        // Use exponential zoom interpolation (`lerpZoomExp`) + `smootherstep` so
        // wide mobile/desktop ratios (e.g. 0.04x -> 1.6x = 40x) glide smoothly
        // at constant perceptual speed instead of slamming/whipping into the tip.
        var t = smootherstep(elapsed / INTRO_ZOOM_SECS);
        _hb.zoom = lerpZoomExp(it.fromZoom, INTRO_TARGET_ZOOM, t);
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
// Seconds without a frame that count as a "stall": the rAF loop was throttled or
// paused (window occluded, on another monitor, or the machine slept) while SSE kept
// flowing. Set well above any legitimate frame hitch, the dt clamp below already
// absorbs sub-second hitches, and nothing on an active tab stalls the loop for
// seconds: so this only fires on a genuine background/sleep gap.
var GAP_RECOVERY_SECS = 5;

// Adaptive live flatline advance rate (virtual px/sec).
// Returns 0 while in history mode, during a block reveal, or while SSE is
// disconnected/syncing so server restarts or network drops never carve empty
// voids. During calm traffic, paces smoothly from 1.1 px/s (quiet tip) up to
// full 5.0 px/s once tip columns reach ~65px so live streaming flows cleanly
// out of a carried/snapshot skyline instead of dropping off a 10x density cliff.
export function computeLiveAdvanceRate(_hb, liveSeg) {
    if (!_hb || !liveSeg || liveSeg.type !== 'flatline' || liveSeg.x_end !== null) return 0;
    if (_hb._viewMode === 'history' || _hb._reveal) return 0;
    if (_hb._v2ConnState === 'disconnected' || _hb._v2ConnState === 'syncing') return 0;
    if (_hb._lastSseEventTs > 0 && ((Date.now() / 1000) - _hb._lastSseEventTs) > 90) return 0;
    var tipGx = Math.floor(_hb.virtualX / 5) * 5;
    var minTipGx = Math.ceil(liveSeg.x_start / 5) * 5;
    var tipSum = 0;
    var tipCols = 0;
    if (liveSeg._colHeights) {
        for (var tc = 0; tc < 5; tc++) {
            var cx = tipGx - tc * 5;
            if (cx < minTipGx) break;
            tipSum += (liveSeg._colHeights[cx] || 0);
            tipCols++;
        }
    }
    var avgTipH = tipCols > 0 ? (tipSum / tipCols) : 0;
    var fillRatio = Math.min(1, avgTipH / 65);
    return FLATLINE_PX_PER_SEC * (0.22 + 0.78 * fillRatio);
}

export function drawFrame(frameTime) {
    var _hb = getState();
    if (!_hb) return;

    var now = Date.now() / 1000;

    // Gap-aware recovery: if the loop was stalled for GAP_RECOVERY_SECS+ while the
    // tab stayed "visible" (no visibilitychange fired), virtualX froze while SSE kept
    // delivering: recover the same way a tab return does instead of grinding the
    // backlog into one column (tall stacks + clustered spikes). The hidden-tab case
    // is owned by visibilitychange (it sets _hiddenSince); exclude it here via
    // !_hb._hiddenSince so the two paths never double-fire.
    var realDt = _hb.lastFrameTime > 0 ? (now - _hb.lastFrameTime) : 0;
    if (realDt > GAP_RECOVERY_SECS && !document.hidden && !_hb._hiddenSince && window._hbRecoverTimeline) {
        window._hbRecoverTimeline(realDt);
        _hb = getState();
        if (!_hb) return;         // a long-gap reset can tear down + rebuild state
        now = Date.now() / 1000;  // recovery reset lastFrameTime to ~now
    }

    var ctx = _hb.ctx;
    var w = _hb.width;
    var h = _hb.height;
    var baseline = (h < 350 ? h * 0.78 : h * 0.55) + (_hb.viewOffsetY || 0);

    // Compute time delta (small after a recovery, since it reset lastFrameTime)
    var dt = _hb.lastFrameTime > 0 ? (now - _hb.lastFrameTime) : 0;
    if (dt > 0.5) dt = 0.5; // clamp to avoid huge jumps on tab switch
    _hb.lastFrameTime = now;

    var isHistory = _hb._viewMode === 'history';

    // Safety net: replay any block that arrived while the tab was hidden but
    // wasn't drained by visibilitychange (would otherwise strand, canvas stuck
    // on the old block while the header shows the new one). RAF only runs at full
    // rate when visible, so this catches up within a frame of the tab showing.
    // Cheap guard: the queue is empty in normal operation.
    if (!isHistory && _hb._blockQueue && _hb._blockQueue.length > 0 && !document.hidden && window._hbDrainBlockQueue) {
        window._hbDrainBlockQueue();
    }

    // ── Advance live flatline ──────────────────────────────
    var liveSeg = (!isHistory && _hb.timeline.length > 0) ? _hb.timeline[_hb.timeline.length - 1] : null;
    var liveRatePx = computeLiveAdvanceRate(_hb, liveSeg);
    if (liveRatePx > 0) {
        _hb.virtualX += dt * liveRatePx;
    }

    // Lay down queued mempool-tx bricks once per frame (RAF-driven), spreading
    // the work across frames instead of a 200ms off-RAF burst that hitched the
    // scroll. Placed after the virtualX advance so bricks land at the live head.
    // Suppressed in history mode and during the form/harvest/unfurl/hold beats of a
    // reveal; allowed during `return` so queued arrivals trickle smoothly onto the tip.
    if (!isHistory && (!_hb._reveal || _hb._reveal.phase === 'return')) flushTxBatch();

    // Block-reveal sequencer takes precedence when armed, it drives virtualX,
    // the leftover placement, zoom AND viewOffset (spike -> relay). It returns
    // false when it finishes or the user takes over, falling through to normal
    // handling in the same frame. In history mode, camera stays static under user control.
    if (isHistory) {
        // User owns panning/zooming in history mode; no auto-follow or play drift.
    } else if (_hb._reveal && runBlockReveal(_hb, dt, now, w)) {
        // Sequencer owns the camera this frame.
    } else if (_hb._intro && runIntro(_hb, dt, now, w)) {
        // First-load intro owns the camera this frame.
    } else if (_hb.autoFollow && !_hb.isDragging) {
        var activeHeadFrac = _hb._liveHeadFrac || HEAD_POSITION_FRAC;
        var targetOffset = _hb.virtualX - (w * activeHeadFrac) / _hb.zoom;
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
    } else if (!_hb.paused && !_hb.isDragging && !_hb._pinching && liveRatePx > 0) {
        // Play mode: scroll at the same rate as the live head (don't jump to head)
        _hb.viewOffset += dt * liveRatePx;
    }

    // ── Compute current color from live state ──────────────
    var elapsed = _hb.lastBlockTime > 0 ? Math.max(0, now - _hb.lastBlockTime) : 0;
    var targetColor = computeColor(elapsed, _hb.nextBlockFee, _hb.mempoolMB);
    var displayedColor = _hb.colorLerp < 1
        ? lerpColor(_hb.prevColor, _hb.targetColor, _hb.colorLerp)
        : _hb.currentColor;
    // Start any new blend from the exact color currently displayed and advance by
    // elapsed time (~0.85s) so low frame rates (<25 fps) never snap back.
    if (targetColor !== _hb.targetColor) {
        _hb.prevColor = displayedColor;
        _hb.currentColor = displayedColor;
        _hb.targetColor = targetColor;
        _hb.colorLerp = 0;
    } else if (_hb.colorLerp < 1) {
        var stepT = (dt > 0 ? dt : (1 / 60)) / 0.85;
        _hb.colorLerp = Math.min(_hb.colorLerp + stepT, 1);
        if (_hb.colorLerp >= 1) {
            _hb.currentColor = targetColor;
            _hb.prevColor = targetColor;
        }
    }
    var liveColor = _hb.colorLerp < 1
        ? lerpColor(_hb.prevColor, _hb.targetColor, _hb.colorLerp)
        : _hb.currentColor;

    // First-load fade-in: _loadFadeStart is set at canvas init (and re-armed after a
    // big gap-recovery), so the veil covers the spikes + bricks as they stream in and
    // eases the whole timeline up from the bg over LOAD_FADE_SECS.
    var loadFade = 1;
    if (!isHistory && _hb._loadFadeStart) {
        var lfe = now - _hb._loadFadeStart;
        if (lfe < LOAD_FADE_SECS) loadFade = smootherstep(lfe / LOAD_FADE_SECS);
    }

    // ── Clear and draw background ──────────────────────────
    ctx.fillStyle = BG_COLOR;
    ctx.fillRect(0, 0, w, h);
    drawGrid(ctx, w, h);

    // ── Visible virtual x range (zoom-aware) ────────────────
    var viewLeft = _hb.viewOffset;
    var viewRight = _hb.viewOffset + w / _hb.zoom;

    // ── Bidirectional lazy-loading of historical blocks ─────
    if (window._hbMaybeFetchMoreBlocks) {
        window._hbMaybeFetchMoreBlocks(_hb, viewLeft, viewRight);
    }

    // ── Draw timeline segments ─────────────────────────────
    // No vertical tremor on long waits: the flatline color shift (via
    // computeColor on elapsed) + the "last block" timer already convey the
    // wait, and a heaving baseline added motion without information.
    var spikeScreenW = 51 * POINT_WIDTH * _hb.zoom;
    var lowDetailBlock = spikeScreenW < 14;
    var skipGlow = _hb.zoom < 0.25;

    // Binary-search first segment whose end >= viewLeft (O(log N) over thousands of loaded blocks)
    var tl = _hb.timeline;
    var bLo = 0, bHi = tl.length;
    while (bLo < bHi) {
        var bMid = (bLo + bHi) >> 1;
        var bSeg = tl[bMid];
        var bEnd = (bSeg.type === 'block' || bSeg.type === 'pending_block')
            ? bSeg.x_end
            : (bSeg.x_end !== null ? bSeg.x_end : _hb.virtualX);
        if (bEnd < viewLeft) bLo = bMid + 1;
        else bHi = bMid;
    }

    var lastBlockLabelRight = -Infinity;
    var showBlockLabels = !lowDetailBlock && spikeScreenW >= 34;

    for (var si = bLo; si < tl.length; si++) {
        var seg = tl[si];
        var segStart = seg.x_start;
        var segEnd = (seg.type === 'block' || seg.type === 'pending_block')
            ? seg.x_end
            : (seg.x_end !== null ? seg.x_end : _hb.virtualX);

        if (segEnd < viewLeft) continue;   // before viewport
        if (segStart > viewRight) break;   // past viewport — all further segments are too

        if (seg.type === 'block') {
            drawBlockSegment(ctx, seg, viewLeft, baseline, liveColor, now, lowDetailBlock, skipGlow);
            // Subtle block-height timeline tick underneath each spike when zoom gives clean spacing
            // (skipped on _historyTarget since it already draws its orange #height tag above the peak)
            if (showBlockLabels && seg.height != null && !seg._historyTarget) {
                var lmMapLbl = (typeof window !== 'undefined' && window._hbLandmarks) ? window._hbLandmarks : null;
                var isLmLbl = !!(lmMapLbl && lmMapLbl[String(seg.height)]);
                var lblMidX = virtualToCanvas((seg.x_start + seg.x_end) * 0.5);
                var lblText = (isLmLbl ? '\u2605 #' : '#') + seg.height.toLocaleString();
                var lblW = lblText.length * 6.2 + 10;
                if (lblMidX - lblW * 0.5 > lastBlockLabelRight + 8) {
                    var deepestY = seg._maxY != null ? seg._maxY : 24;
                    var lblY = Math.min(h - 10, baseline + Math.max(26, deepestY + 14));
                    var isHovLbl = _hb.hoveredBlock === seg || seg._historyTarget;
                    ctx.save();
                    ctx.font = ((isHovLbl || isLmLbl) ? 'bold 10px monospace' : '10px monospace');
                    ctx.textAlign = 'center';
                    ctx.textBaseline = 'alphabetic';
                    ctx.fillStyle = isHovLbl
                        ? (seg._historyTarget ? '#f7931a' : (seg.color || '#ffffff'))
                        : (isLmLbl ? 'rgba(247, 147, 26, 0.78)' : 'rgba(255, 255, 255, 0.28)');
                    ctx.fillText(lblText, lblMidX, lblY);
                    ctx.restore();
                    lastBlockLabelRight = lblMidX + lblW * 0.5;
                }
            }
        } else if (seg.type === 'pending_block') {
            drawPendingBlockSegment(ctx, seg, baseline, now);
        } else {
            var isOpenFlat = (seg.x_end === null);
            var flatWaitSec;
            var flatColor;
            if (isOpenFlat) {
                var feeBoost = (_hb.nextBlockFee || 0) >= 30
                    ? Math.min(600, ((_hb.nextBlockFee - 30) / 120) * 600) : 0;
                flatWaitSec = elapsed + feeBoost;
                flatColor = liveColor;
            } else {
                var segInter = resolveInterBlockSeconds(seg);
                if (segInter === null && si + 1 < tl.length && tl[si + 1].type === 'block') {
                    segInter = resolveInterBlockSeconds(tl[si + 1]);
                }
                flatWaitSec = segInter === null ? null : Math.max(0, segInter);
                flatColor = seg.color || (segInter === null ? UNKNOWN_INTERVAL_COLOR : waitColor(flatWaitSec));
            }
            var drawSegEnd = (typeof seg._revealLineEnd === 'number')
                ? Math.min(segEnd, seg._revealLineEnd)
                : segEnd;
            drawFlatlineSegment(ctx, seg, drawSegEnd, viewLeft, viewRight, baseline, flatColor, isOpenFlat, now, flatWaitSec, skipGlow);
        }
    }

    // ── Draw tooltip if hovering ───────────────────────────
    // Hover preview takes priority even while another tx is pinned in the HTML panel.
    // Suppress redundant canvas tooltip for _pinnedBlip when __hbSelectTx is active.
    ctx.globalAlpha = 1;
    if (_hb.hoveredBlock) {
        var hSeg = _hb.hoveredBlock;
        var midX = virtualToCanvas((hSeg.x_start + hSeg.x_end) / 2);
        drawTooltip(ctx, hSeg, midX, baseline - 30, baseline);
    } else if (_hb.hoveredPendingBlock) {
        var hpSeg = _hb.hoveredPendingBlock;
        var hpMidX = virtualToCanvas((hpSeg.x_start + hpSeg.x_end) / 2);
        drawPendingBlockTooltip(ctx, hpSeg, hpMidX, baseline - 30);
    } else if (_hb.hoveredBlip && !_hb.hoveredBlip._removed) {
        drawBlipTooltip(ctx, _hb.hoveredBlip, _hb.hoverCanvasX, baseline);
    } else if (!isHistory && _hb._pinnedBlip && !_hb._pinnedBlip._removed && _hb._pinnedBlip.x !== undefined && typeof window.__hbSelectTx !== 'function') {
        var pbx = virtualToCanvas(_hb._pinnedBlip.x);
        drawBlipTooltip(ctx, _hb._pinnedBlip, pbx, baseline);
    } else if (_hb.hoveredFlatline) {
        drawFlatlineTooltip(ctx, _hb.hoveredFlatline, _hb.hoverCanvasX || w / 2, baseline);
    }

    // Zoom indicator is now in HTML control bar

    // Jump to Live is now in the control bar

    // ── Draw search highlight reticle (locked onto the actual brick) ──
    if (!isHistory && _hb._searchHighlight) {
        var sh = _hb._searchHighlight;
        var shElapsed = now - sh.start;
        if (shElapsed > sh.duration) {
            _hb._searchHighlight = null;
        } else {
            // Resolve the live brick reference so coordinates stay exact even if columns restack
            var targetBlip = (sh.txid && _hb._pinnedBlip && _hb._pinnedBlip.txid === sh.txid && !_hb._pinnedBlip._removed)
                ? _hb._pinnedBlip
                : (sh.blip && !sh.blip._removed ? sh.blip : null);
            var shVirtX = targetBlip ? (targetBlip.x !== undefined ? targetBlip.x : sh.x) : sh.x;
            var shCX = (shVirtX - _hb.viewOffset) * _hb.zoom;
            var shHeightScale = brickHeightScale(_hb.zoom);
            var shBW = targetBlip ? Math.max(4, (targetBlip.brickW || 4) * _hb.zoom) : Math.max(6, 4 * _hb.zoom);
            var shBH = targetBlip ? Math.max(4, (targetBlip.brickH || targetBlip.height || 6) * shHeightScale) : 8;
            var shSY = targetBlip ? (targetBlip.stackY || 0) * shHeightScale : 0;
            var shLeft = shCX - shBW * 0.5;
            var shTop = baseline - shSY - shBH;

            // Smooth lock-in over first 0.45s, then gentle breathe & clean fade-out
            var lockT = Math.min(1, shElapsed / 0.45);
            var lockEase = 1 - Math.pow(1 - lockT, 3);
            var fadeOut = shElapsed > (sh.duration - 1.2)
                ? Math.max(0, (sh.duration - shElapsed) / 1.2)
                : 1.0;
            var breathe = 0.5 + 0.5 * Math.sin(shElapsed * 5.0);
            var pad = (22 * (1 - lockEase)) + 5 + breathe * 1.8;

            var bx1 = shLeft - pad;
            var by1 = shTop - pad;
            var bx2 = shLeft + shBW + pad;
            var by2 = shTop + shBH + pad;
            var cornerLen = Math.max(6, Math.min(13, Math.min(bx2 - bx1, by2 - by1) * 0.32));

            ctx.save();
            // 1. Subtle vertical beacon column from baseline up above the target brick
            var beaconTop = Math.max(12, shTop - 36);
            var beaconGrad = ctx.createLinearGradient(0, beaconTop, 0, baseline);
            beaconGrad.addColorStop(0, 'rgba(0, 230, 118, 0)');
            beaconGrad.addColorStop(0.55, 'rgba(0, 230, 118, ' + (0.14 * fadeOut).toFixed(3) + ')');
            beaconGrad.addColorStop(1, 'rgba(0, 230, 118, ' + (0.04 * fadeOut).toFixed(3) + ')');
            ctx.fillStyle = beaconGrad;
            ctx.fillRect(shLeft - 2, beaconTop, shBW + 4, Math.max(0, baseline - beaconTop));

            // 2. Crisp 4-corner viewfinder brackets around the brick
            ctx.strokeStyle = 'rgba(0, 230, 118, ' + (0.95 * fadeOut).toFixed(3) + ')';
            ctx.lineWidth = 2;
            ctx.lineCap = 'round';
            ctx.shadowColor = '#00e676';
            ctx.shadowBlur = 10 * fadeOut;
            ctx.beginPath();
            // Top-left
            ctx.moveTo(bx1, by1 + cornerLen); ctx.lineTo(bx1, by1); ctx.lineTo(bx1 + cornerLen, by1);
            // Top-right
            ctx.moveTo(bx2 - cornerLen, by1); ctx.lineTo(bx2, by1); ctx.lineTo(bx2, by1 + cornerLen);
            // Bottom-left
            ctx.moveTo(bx1, by2 - cornerLen); ctx.lineTo(bx1, by2); ctx.lineTo(bx1 + cornerLen, by2);
            // Bottom-right
            ctx.moveTo(bx2 - cornerLen, by2); ctx.lineTo(bx2, by2); ctx.lineTo(bx2, by2 - cornerLen);
            ctx.stroke();

            // 3. Gently bobbing downward chevron directly above the brick
            var chevY = by1 - 8 - breathe * 3.5;
            if (chevY > 10) {
                ctx.fillStyle = 'rgba(0, 230, 118, ' + (0.95 * fadeOut).toFixed(3) + ')';
                ctx.beginPath();
                ctx.moveTo(shCX, chevY + 5);
                ctx.lineTo(shCX - 5, chevY - 2);
                ctx.lineTo(shCX + 5, chevY - 2);
                ctx.closePath();
                ctx.fill();
            }
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
    if (!isHistory && liveSeg && liveSeg.x_end === null) {
        var dotVirtX = (typeof liveSeg._revealLineEnd === 'number')
            ? Math.min(_hb.virtualX, liveSeg._revealLineEnd)
            : _hb.virtualX;
        var hideDotForReveal = (typeof liveSeg._revealLineEnd === 'number' && liveSeg._revealLineEnd <= liveSeg.x_start + 0.5);
        var dotX = virtualToCanvas(dotVirtX);
        if (!hideDotForReveal && dotX >= -10 && dotX <= w + 10) {
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

    // First-load / resume fade veil: covers the whole timeline SCENE, grid, spikes,
    // bricks AND the live head dot: with the bg color, retreating over LOAD_FADE_SECS
    // so it all eases up from black together. Drawn here (after the head dot, before
    // the disconnect overlay) so nothing in the scene is left un-faded. (A blanket
    // globalAlpha won't work: per-element draws set their own alpha.)
    if (loadFade < 1) {
        ctx.globalAlpha = 1 - loadFade;
        ctx.fillStyle = BG_COLOR;
        ctx.fillRect(0, 0, w, h);
        ctx.globalAlpha = 1;
    }

    // SSE disconnection overlay: semi-transparent with mining animation
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
export function drawBlockSegment(ctx, seg, viewLeft, baseline, fallbackColor, now, lowDetail, skipGlow) {
    var _hb = getState();
    var pts = seg.points;
    var color = seg.color || fallbackColor;
    var isHovered = (_hb && _hb.hoveredBlock === seg);

    // Strike animation: for a live block's spike, scale the whole QRS from the
    // baseline up: a slow build then a snap with overshoot (spikeStrike). amp=1
    // (settled) after the window and for any segment without _strikeStart
    // (history/replay draw at full amplitude immediately).
    var amp = 1;
    if (seg._strikeStart && now) {
        var se = now - seg._strikeStart;
        if (se >= 0 && se < SPIKE_STRIKE_SECS) amp = spikeStrike(se / SPIKE_STRIKE_SECS);
    } else if (seg._strikePending) {
        // Reveal spike awaiting its strike. Stay flat only while an active reveal
        // still owns it (camera traveling to the head); otherwise it's orphaned
        // (reveal ended/never armed): strike it now so it can never stay flat.
        if (_hb && _hb._reveal && _hb._reveal.spikeSeg === seg) {
            amp = 0;
        } else {
            seg._strikeStart = now;
            amp = 0; // = spikeStrike(0); animates from the next frame
        }
    }

    // The shockwave + flash fire at the BAM moment (offset into the strike, when the
    // wave snaps up), not at creation, and outlive the strike slightly.
    var shockT = -1;
    if (seg._strikeStart && now) {
        var we = now - (seg._strikeStart + SPIKE_STRIKE_SECS * SPIKE_BAM_AT);
        if (we >= 0 && we < SPIKE_SHOCKWAVE_SECS) shockT = we / SPIKE_SHOCKWAVE_SECS;
    }

    var isStriking = !!seg._strikePending
        || !!(seg._strikeStart && (!now || (now - seg._strikeStart) < (SPIKE_STRIKE_SECS * SPIKE_BAM_AT + SPIKE_SHOCKWAVE_SECS)));

    // Zoomed-out LOD: draw a crisp 4-point peak/trough polyline with zero blur
    // instead of 51 sub-pixel vertices per spike.
    if (lowDetail && !isHovered && !isStriking && !seg._historyTarget) {
        var minY = seg._minY;
        var minIdx = seg._minYIdx;
        var maxY = seg._maxY;
        var maxIdx = seg._maxYIdx;
        if (minIdx === undefined || maxIdx === undefined) {
            minY = 0; minIdx = 0; maxY = 0; maxIdx = 0;
            for (var li = 0; li < pts.length; li++) {
                if (pts[li] < minY) { minY = pts[li]; minIdx = li; }
                if (pts[li] > maxY) { maxY = pts[li]; maxIdx = li; }
            }
            seg._minY = minY; seg._minYIdx = minIdx;
            seg._maxY = maxY; seg._maxYIdx = maxIdx;
        }
        var lx1 = virtualToCanvas(seg.x_start);
        var lx2 = virtualToCanvas(seg.x_end);
        var lMinX = virtualToCanvas(seg.x_start + minIdx * POINT_WIDTH);
        var lMaxX = virtualToCanvas(seg.x_start + maxIdx * POINT_WIDTH);
        ctx.beginPath();
        ctx.strokeStyle = color;
        ctx.lineWidth = 1.25;
        ctx.shadowBlur = 0;
        ctx.moveTo(lx1, baseline);
        if (minIdx <= maxIdx) {
            ctx.lineTo(lMinX, baseline + minY * amp);
            ctx.lineTo(lMaxX, baseline + maxY * amp);
        } else {
            ctx.lineTo(lMaxX, baseline + maxY * amp);
            ctx.lineTo(lMinX, baseline + minY * amp);
        }
        ctx.lineTo(lx2, baseline);
        ctx.stroke();
        return;
    }

    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = isHovered ? 3 : 2;
    if (skipGlow && !isHovered && !isStriking) {
        ctx.shadowBlur = 0;
    } else {
        ctx.shadowColor = color;
        // Flash: extra glow peaking at the BAM and decaying.
        ctx.shadowBlur = (isHovered ? 24 : 14) + (shockT >= 0 ? (1 - shockT) * 34 : 0);
    }

    for (var i = 0; i < pts.length; i++) {
        var vx = seg.x_start + i * POINT_WIDTH;
        var cx = virtualToCanvas(vx);
        var cy = baseline + pts[i] * amp;
        if (i === 0) {
            ctx.moveTo(cx, cy);
        } else {
            ctx.lineTo(cx, cy);
        }
    }
    ctx.stroke();
    ctx.shadowBlur = 0;

    // Radiating shockwave from the peak, fired at the BAM.
    if (shockT >= 0) {
        var peakIdx = seg._minYIdx;
        var peakY = seg._minY;
        if (peakIdx === undefined) {
            peakIdx = 0; peakY = 0;
            for (var pk = 0; pk < pts.length; pk++) {
                if (pts[pk] < peakY) { peakY = pts[pk]; peakIdx = pk; }
            }
        }
        var peakX = virtualToCanvas(seg.x_start + peakIdx * POINT_WIDTH);
        drawShockwave(ctx, peakX, baseline + peakY * amp, shockT, color);
    }

    // Subtle amber/cyan target marker over the requested historical block spike
    if (seg._historyTarget) {
        var hPeakIdx = seg._minYIdx;
        var hPeakY = seg._minY;
        if (hPeakIdx === undefined) {
            hPeakIdx = 0; hPeakY = 0;
            for (var hpk = 0; hpk < pts.length; hpk++) {
                if (pts[hpk] < hPeakY) { hPeakY = pts[hpk]; hPeakIdx = hpk; }
            }
        }
        var hcx1 = virtualToCanvas(seg.x_start);
        var hcx2 = virtualToCanvas(seg.x_end);
        var hPeakX = virtualToCanvas(seg.x_start + hPeakIdx * POINT_WIDTH);
        var hPeakScreenY = baseline + hPeakY * amp;
        ctx.save();
        ctx.fillStyle = 'rgba(247, 147, 26, 0.08)';
        ctx.fillRect(hcx1, 8, Math.max(2, hcx2 - hcx1), (_hb ? _hb.height : 400) - 16);
        ctx.strokeStyle = 'rgba(56, 189, 248, 0.85)';
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.arc(hPeakX, hPeakScreenY, 6, 0, Math.PI * 2);
        ctx.stroke();
        var tagText = '#' + (seg.height || 0).toLocaleString();
        ctx.font = 'bold 11px monospace';
        ctx.textAlign = 'center';
        ctx.fillStyle = '#f7931a';
        var tagY = Math.max(18, hPeakScreenY - 12);
        ctx.fillText(tagText, hPeakX, tagY);
        ctx.restore();
    }
}

// Draw a reserved pending_block slot: always keep the EKG baseline level (no fake
// provisional peak and no jarring cyan box on fast blocks). During the 2s grace period
// it blends seamlessly into the live baseline; only on a genuinely slow block (>= 2s)
// does it show a subtle pulsing ring on the baseline and quiet "Pending block" label.
export function drawPendingBlockSegment(ctx, seg, baseline, nowSec) {
    var _hb = getState();
    var cx1 = virtualToCanvas(seg.x_start);
    var cx2 = virtualToCanvas(seg.x_end);
    var slotW = Math.max(2, cx2 - cx1);
    var ageMs = seg.receivedWallMs ? (Date.now() - seg.receivedWallMs) : 0;
    var showPendingCue = ageMs >= 2000;
    var color = showPendingCue ? (seg.color || '#38bdf8') : ((_hb && _hb.currentColor) || COLORS.healthy);
    var pulse = 0.5 + 0.5 * Math.sin((nowSec || Date.now() / 1000) * 3.0);

    ctx.save();
    // Level EKG baseline across the reserved slot (never a fake provisional spike)
    ctx.beginPath();
    ctx.moveTo(cx1, baseline);
    ctx.lineTo(cx2, baseline);
    ctx.strokeStyle = color;
    ctx.lineWidth = (_hb && _hb.hoveredPendingBlock === seg) ? 3 : 2;
    ctx.shadowColor = color;
    ctx.shadowBlur = showPendingCue ? (8 + pulse * 4) : 8;
    ctx.stroke();
    ctx.shadowBlur = 0;

    // Quiet baseline cue ONLY after the 2s grace period (fast blocks never flash a box)
    if (showPendingCue) {
        var midX = (cx1 + cx2) / 2;
        ctx.beginPath();
        ctx.arc(midX, baseline, 4 + pulse * 2.5, 0, Math.PI * 2);
        ctx.strokeStyle = 'rgba(56, 189, 248, ' + (0.45 + pulse * 0.45).toFixed(3) + ')';
        ctx.lineWidth = 1.5;
        ctx.stroke();

        ctx.beginPath();
        ctx.arc(midX, baseline, 2.2, 0, Math.PI * 2);
        ctx.fillStyle = color;
        ctx.fill();

        if (slotW >= 24) {
            ctx.font = '10px monospace';
            ctx.textAlign = 'center';
            ctx.fillStyle = 'rgba(56, 189, 248, ' + (0.6 + pulse * 0.25).toFixed(3) + ')';
            ctx.fillText('Pending block', midX, baseline + 20);
            ctx.textAlign = 'left';
        }
    }
    ctx.restore();
}

// LOD engages only when a segment has more than this many bricks (≈ the mempool
// size) AND is zoomed out. DEFAULT 100000: color-batched individual bricks stay
// smooth up to ~100k; LOD engages above 100k as a performance safety valve.
var LOD_MIN_BLIPS = 100000;

// ── P2: Level-of-detail blip rendering ────────────────────────
// When zoomed out so far that grid columns are sub-pixel, drawing every brick
// (fillRect + gap math + gradient + outline, per brick, per frame) is the
// dominant cost: tens of thousands of ops that produced the ~141ms RAF
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
    // hop columns frame-to-frame: and since each bar's height is the tallest
    // brick in its column, that hopping made bars jump up/down = the flicker.
    // Virtual buckets are stable under panning; apply the view offset once, here.
    var offPx = _hb.viewOffset * zoom;
    var colW = Math.max(1, 5 * zoom); // grid pitch in canvas px, floored at 1
    var spotBand = (typeof _hb._spotlightBand === 'number' && _hb._spotlightBand >= 0) ? _hb._spotlightBand : -1;
    var cols = {};                    // stable virtual bucket -> aggregate
    for (var i = 0; i < blips.length; i++) {
        var b = blips[i];
        if (!b || b._removed || b.fadeStart > 0) continue; // skip removed/absorbing particles
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
        if (!c) c = cols[k] = { h: 0, notable: false, ncolor: null, counts: {}, topN: 0, topColor: null, spotMatch: false, spotColor: null };
        if (top > c.h) c.h = top;
        if (spotBand >= 0 && feeBandIndex(b.feeRate, b.feePending) === spotBand) {
            c.spotMatch = true;
            if (!c.spotColor) c.spotColor = (b.feePending && !b.notableType) ? PENDING_FEE_COLOR : (b.color || feeRateColor(b.feeRate));
        }
        if (b.notableType) {
            if (!c.notable) { c.notable = true; c.ncolor = b.color; }
        } else {
            // Dominant (mode) fixed fee band color for this column.
            var colKey = b.color || feeRateColor(b.feeRate);
            var cc = (c.counts[colKey] || 0) + 1;
            c.counts[colKey] = cc;
            if (cc > c.topN) { c.topN = cc; c.topColor = colKey; }
        }
    }
    // During a block reveal, the old segment's leftovers fade out (harvest beat).
    var rf = seg._revealFade !== undefined ? seg._revealFade : 1;
    if (rf <= 0) return;
    for (var key in cols) {
        var col = cols[key];
        // blip.color is an "rgba(r, g, b, " prefix awaiting an alpha + ")".
        var color, alpha;
        if (spotBand >= 0) {
            if (col.spotMatch) {
                color = col.spotColor || col.topColor || PENDING_FEE_COLOR;
                alpha = 0.95 * rf;
            } else {
                color = col.topColor || PENDING_FEE_COLOR;
                alpha = 0.10 * rf;
            }
        } else {
            color = col.notable ? col.ncolor : (col.topColor || PENDING_FEE_COLOR);
            alpha = (col.notable ? 0.95 : 0.72) * rf;
        }
        ctx.fillStyle = color + alpha + ')';
        ctx.fillRect(Number(key) - offPx, baseline - col.h, colW, col.h);
    }
    // Keep the pinned brick visibly marked even when LOD is active.
    if (_hb._pinnedTxid && _hb._pinnedBlip && !_hb._pinnedBlip._removed && _hb._pinnedBlip.fadeStart === 0) {
        var pb = _hb._pinnedBlip;
        var pvx = pb.gridX !== undefined ? pb.gridX : pb.x;
        if (pvx >= viewLeft && pvx <= viewRight) {
            var px = (pvx - _hb.viewOffset) * zoom;
            var ph = Math.max(6, pb.height || pb.brickH || 8);
            ctx.save();
            ctx.strokeStyle = '#ffffff';
            ctx.lineWidth = 2;
            ctx.strokeRect(px - 2, baseline - ph - 2, Math.max(colW, 4) + 4, ph + 4);
            ctx.restore();
        }
    }
}

// Color-batched fast brick renderer for zoom <= 4 (plain solid rectangles,
// zero gradients/roundRects/text labels). Batches up to 50k+ visible bricks
// into ~14 fixed-color beginPath()+rect()+fill() calls instead of 45k individual
// fillStyle+fillRect calls, preserving 100% pixel-identical individual bricks
// at a locked 60 FPS during initial cascade, zoomed-out overview, and reveals.
function drawFlatlineBricksBatched(ctx, _hb, seg, viewLeft, viewRight, baseline, zoom, nowSec) {
    var blips = seg.blips;
    var rf = seg._revealFade !== undefined ? seg._revealFade : 1;
    var viewOffset = _hb.viewOffset;
    var pinnedTxid = _hb._pinnedTxid;
    var pinnedRect = null;
    var spotBand = (typeof _hb._spotlightBand === 'number' && _hb._spotlightBand >= 0) ? _hb._spotlightBand : -1;

    var dimmedNorm = Object.create(null);
    var settledNorm = Object.create(null);
    var settledNotable = Object.create(null);
    var enteringLow = Object.create(null);
    var enteringMid = Object.create(null);
    var notableEdges = [];
    var absorbing = [];
    // Below ~0.56x zoom, 5px columns are < 2.8 screen px wide: draw bricks seamlessly
    // at full column pitch (+0.35px overlap, zero gap) using smooth float coordinates
    // so there are no sub-pixel seams to Moire-shimmer AND no per-column rounding jitter.
    var colStep = 5 * zoom;
    var seamlessZoom = colStep < 2.8;

    var _wStart = 0;
    var _wSorted = false;
    var _isProtected = seg.x_end === null
        || !!seg._pendingForHash
        || (_hb._reveal && _hb._reveal.oldSeg === seg)
        || (_hb._pendingReveal && _hb._pendingReveal.oldSeg === seg);
    if (!_isProtected && blips.length > 512) {
        if (!seg._sortedByX) {
            blips.sort(function(a, b) { return (a.x || 0) - (b.x || 0); });
            seg._sortedByX = true;
        }
        _wSorted = true;
        var _lo = 0, _hi = blips.length;
        while (_lo < _hi) {
            var _mid = (_lo + _hi) >> 1;
            if ((blips[_mid].x || 0) < viewLeft) _lo = _mid + 1; else _hi = _mid;
        }
        _wStart = _lo;
    }

    for (var i = _wStart; i < blips.length; i++) {
        var blip = blips[i];
        if (!blip || blip._removed) continue;
        var bxVirt = blip.x;
        if (_wSorted && bxVirt > viewRight) break;
        if (bxVirt < viewLeft || bxVirt > viewRight) continue;
        if (typeof seg._revealLineEnd === 'number' && bxVirt > seg._revealLineEnd) continue;

        if (blip.fadeStart > 0) {
            absorbing.push(blip);
            continue;
        }
        if (rf <= 0) continue;

        var age = nowSec - blip.timestamp;
        if (age < 0) continue;

        var isNotable = !!blip.notableType;
        var fadeTime = isNotable ? 1.45 : 1.15;
        var bw = (blip.brickW || 4) * zoom;
        var bh = blip.brickH || blip.height;
        var sy = blip.stackY || 0;
        var bx = (bxVirt - viewOffset) * zoom;
        var baseColor = (blip.feePending && !isNotable) ? PENDING_FEE_COLOR : (blip.color || PENDING_FEE_COLOR);

        // Compute drop offset when falling: smooth sky-to-baseline glide with a
        // 2-tier soft opacity fade-in and gentle cushion settle (no abrupt pop).
        var dropOffset = 0;
        var enterTier = 0; // 0 = settled, 1 = softest entry, 2 = mid fade-in
        if (age < fadeTime) {
            var fadeIn = age / fadeTime;
            var baseDrop = isNotable ? Math.min(200, Math.max(150, baseline * 0.50)) : Math.min(145, Math.max(105, baseline * 0.36));
            // Per-brick height variation (from uncorrelated bobPhase) breaks up
            // the solid carpet underside into individual falling raindrops.
            var heightVar = 0.70 + ((blip.bobPhase || 0) / 6.2831853) * 0.60;
            var dropHeight = baseDrop * heightVar;
            var inv = 1 - fadeIn;
            var dropEase = Math.pow(inv, 2.2);
            var bounceAmt = isNotable ? 0.16 : 0.07;
            var bounce = fadeIn > 0.80 ? Math.sin((fadeIn - 0.80) * (Math.PI / 0.20)) * bounceAmt * 0.05 : 0;
            dropOffset = (dropEase + bounce) * dropHeight;
            if (fadeIn < 0.16) enterTier = 1;
            else if (fadeIn < 0.38) enterTier = 2;
        }

        var rx, ry, rw, rh;
        if (seamlessZoom) {
            rx = bx - colStep * 0.5;
            ry = baseline - sy - bh - dropOffset;
            rw = Math.max(colStep + 0.35, 1.0);
            rh = Math.max(bh + 0.2, 1.0);
        } else {
            var minDim = bw < bh ? bw : bh;
            var gap = minDim > 3.2 ? Math.min(Math.max(minDim * 0.13, 0.5), 1.1) : 0;
            rx = bx - bw * 0.5 + gap;
            ry = baseline - sy - bh + gap - dropOffset;
            rw = Math.max(0.5, bw - gap * 2);
            rh = Math.max(0.5, bh - gap * 2);
        }

        var isPinnedHere = !!(pinnedTxid && blip.txid === pinnedTxid);
        var dimmedBySpot = spotBand >= 0 && !isPinnedHere && feeBandIndex(blip.feeRate, blip.feePending) !== spotBand;

        if (dimmedBySpot) {
            var dm = dimmedNorm[baseColor];
            if (!dm) dm = dimmedNorm[baseColor] = [];
            dm.push(rx, ry, rw, rh);
        } else if (isNotable) {
            var nb = settledNotable[baseColor];
            if (!nb) nb = settledNotable[baseColor] = [];
            nb.push(rx, ry, rw, rh);
            if (!seamlessZoom && rw >= 2.2 && rh >= 4) {
                notableEdges.push(rx, ry, rw, rh);
            }
        } else if (enterTier === 1) {
            var el = enteringLow[baseColor];
            if (!el) el = enteringLow[baseColor] = [];
            el.push(rx, ry, rw, rh);
        } else if (enterTier === 2) {
            var em = enteringMid[baseColor];
            if (!em) em = enteringMid[baseColor] = [];
            em.push(rx, ry, rw, rh);
        } else {
            var arr = settledNorm[baseColor];
            if (!arr) arr = settledNorm[baseColor] = [];
            arr.push(rx, ry, rw, rh);
        }

        if (isPinnedHere) {
            pinnedRect = [rx, ry, rw, rh];
        }
    }

    // 0. Dimmed background bricks when a fee-band spotlight is active
    if (spotBand >= 0) {
        var dimAlpha = (0.10 * rf).toFixed(2);
        for (var dmKey in dimmedNorm) {
            var dmCoords = dimmedNorm[dmKey];
            ctx.fillStyle = dmKey + dimAlpha + ')';
            ctx.beginPath();
            for (var dmci = 0; dmci < dmCoords.length; dmci += 4) {
                ctx.rect(dmCoords[dmci], dmCoords[dmci + 1], dmCoords[dmci + 2], dmCoords[dmci + 3]);
            }
            ctx.fill();
        }
    }

    // 1. Batched normal bricks (2-stage soft entry fade + main settled/falling path;
    // single beginPath+fill per color = zero overdraw seams and smooth visual glide)
    var lowAlpha = (0.20 * rf).toFixed(2);
    for (var elKey in enteringLow) {
        var elCoords = enteringLow[elKey];
        ctx.fillStyle = elKey + lowAlpha + ')';
        ctx.beginPath();
        for (var elci = 0; elci < elCoords.length; elci += 4) {
            ctx.rect(elCoords[elci], elCoords[elci + 1], elCoords[elci + 2], elCoords[elci + 3]);
        }
        ctx.fill();
    }
    var midAlpha = (0.54 * rf).toFixed(2);
    for (var emKey in enteringMid) {
        var emCoords = enteringMid[emKey];
        ctx.fillStyle = emKey + midAlpha + ')';
        ctx.beginPath();
        for (var emci = 0; emci < emCoords.length; emci += 4) {
            ctx.rect(emCoords[emci], emCoords[emci + 1], emCoords[emci + 2], emCoords[emci + 3]);
        }
        ctx.fill();
    }
    var normAlpha = (0.84 * rf).toFixed(2);
    for (var cKey in settledNorm) {
        var coords = settledNorm[cKey];
        ctx.fillStyle = cKey + normAlpha + ')';
        ctx.beginPath();
        for (var ci = 0; ci < coords.length; ci += 4) {
            ctx.rect(coords[ci], coords[ci + 1], coords[ci + 2], coords[ci + 3]);
        }
        ctx.fill();
    }

    // 2. Batched notable fills + crisp dark seam & subtle top highlight
    // (keeps clustered notables separated as distinct bricks without bleeding across columns)
    var notAlpha = (1.0 * rf).toFixed(2);
    for (var nKey in settledNotable) {
        var nCoords = settledNotable[nKey];
        ctx.fillStyle = nKey + notAlpha + ')';
        ctx.beginPath();
        for (var ni = 0; ni < nCoords.length; ni += 4) {
            ctx.rect(nCoords[ni], nCoords[ni + 1], nCoords[ni + 2], nCoords[ni + 3]);
        }
        ctx.fill();
    }
    if (notableEdges.length > 0) {
        ctx.strokeStyle = 'rgba(8, 15, 30, ' + (0.72 * rf).toFixed(2) + ')';
        ctx.lineWidth = Math.max(0.8, Math.min(1.2, zoom * 0.4));
        ctx.beginPath();
        for (var ei = 0; ei < notableEdges.length; ei += 4) {
            ctx.rect(notableEdges[ei], notableEdges[ei + 1], notableEdges[ei + 2], notableEdges[ei + 3]);
        }
        ctx.stroke();
        // Subtle bright top rim inside each notable brick so it reads crisp and elevated
        ctx.fillStyle = 'rgba(255, 255, 255, ' + (0.28 * rf).toFixed(2) + ')';
        ctx.beginPath();
        for (var ti = 0; ti < notableEdges.length; ti += 4) {
            if (notableEdges[ti + 2] >= 2.5 && notableEdges[ti + 3] >= 6) {
                ctx.rect(notableEdges[ti] + 0.5, notableEdges[ti + 1] + 0.5, notableEdges[ti + 2] - 1, 1);
            }
        }
        ctx.fill();
    }

    // 4. Absorbing block-inclusion shatter particles
    if (absorbing.length > 0) {
        var spikeTopY = baseline - 80 * zoom;
        for (var ai = 0; ai < absorbing.length; ai++) {
            var ab = absorbing[ai];
            var fadeDt = nowSec - ab.fadeStart;
            if (ab.particles && ab.particles.length > 0) {
                var animDist = Math.abs(ab.absorbTargetX - ab.absorbOriginX);
                var animDuration = Math.max(1.5, Math.min(animDist / 40, 4.0));
                if (fadeDt >= animDuration / 0.7 + 0.3) continue;
                var aColor = (ab.feePending && !ab.notableType) ? PENDING_FEE_COLOR : (ab.color || PENDING_FEE_COLOR);
                for (var pi = 0; pi < ab.particles.length; pi++) {
                    var p = ab.particles[pi];
                    var pElapsed = fadeDt - p.delay;
                    if (pElapsed < 0) {
                        var sx = (ab.x - viewOffset) * zoom + p.offsetX * Math.min(zoom, 2);
                        var sy0 = baseline - p.offsetY;
                        ctx.fillStyle = aColor + (ab.opacity * 0.85) + ')';
                        ctx.fillRect(sx, sy0, p.size * Math.min(zoom, 2), p.size * Math.min(zoom, 2));
                        continue;
                    }
                    var pt = Math.min(pElapsed / (animDuration / p.speed), 1.0);
                    if (pt >= 1.0) continue;
                    var ease = pt * pt * pt;
                    var curVX = ab.absorbOriginX + (ab.absorbTargetX - ab.absorbOriginX) * ease;
                    var px = (curVX - viewOffset) * zoom + p.offsetX * (1 - ease);
                    var startY = baseline - p.offsetY;
                    var targetY = spikeTopY + (baseline - spikeTopY) * 0.3;
                    var arc = Math.sin(pt * Math.PI * 0.7) * p.arcHeight * 0.3;
                    var py = startY + (targetY - startY) * ease - arc;
                    var pSize = p.size * Math.min(zoom, 2) * (pt > 0.6 ? 1 - (pt - 0.6) / 0.4 * 0.6 : 1);
                    var pAlpha = ab.opacity * (pt > 0.7 ? (1 - pt) / 0.3 : 1);
                    ctx.fillStyle = aColor + pAlpha + ')';
                    ctx.fillRect(px - pSize / 2, py - pSize / 2, pSize, pSize);
                }
            }
        }
    }

    // 5. Persistent highlight ring around the pinned brick
    if (pinnedRect) {
        ctx.save();
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = Math.max(2, Math.min(3.5, zoom * 0.35));
        ctx.shadowColor = '#00e676';
        ctx.shadowBlur = 10;
        ctx.strokeRect(pinnedRect[0] - 2, pinnedRect[1] - 2, pinnedRect[2] + 4, pinnedRect[3] + 4);
        ctx.restore();
    }
}

// Draw a flatline segment (with optional blips)
export function drawFlatlineSegment(ctx, seg, segEnd, viewLeft, viewRight, baseline, color, isLive, nowSec, waitSec, skipGlow) {
    var _hb = getState();
    // Clamp to visible range
    var drawStart = Math.max(seg.x_start, viewLeft);
    var drawEnd = Math.min(segEnd, viewRight);

    if (drawStart >= drawEnd) return;

    var cx1 = virtualToCanvas(drawStart);
    var cx2 = virtualToCanvas(drawEnd);
    var fullCx1 = virtualToCanvas(seg.x_start);
    var fullCx2 = virtualToCanvas(segEnd);
    var y = baseline;

    // Anchor the progressive wait gradient to the full segment span so panning/zooming
    // never slides the gradient with the viewport edge. Narrow flatlines (< 18px) use
    // a solid color to avoid allocating hundreds of linear gradients per frame zoomed out.
    var strokeStyle = ((fullCx2 - fullCx1) >= 18 && typeof waitSec === 'number' && isFinite(waitSec) && waitSec > 300)
        ? buildFlatlineGradient(ctx, fullCx1, fullCx2, waitSec)
        : color;

    var noGlow = !!skipGlow && !isLive;

    // Draw the flatline
    ctx.beginPath();
    ctx.moveTo(cx1, y);
    ctx.lineTo(cx2, y);
    ctx.strokeStyle = strokeStyle;
    ctx.lineWidth = 2;
    if (noGlow) {
        ctx.shadowBlur = 0;
    } else {
        ctx.shadowBlur = 8;
        ctx.shadowColor = color;
    }
    ctx.stroke();
    ctx.shadowBlur = 0;

    // Subtle floor glow below baseline (skipped on non-live flatlines when zoomed way out)
    if (!noGlow) {
        ctx.globalAlpha = 0.06;
        ctx.fillStyle = strokeStyle;
        ctx.fillRect(cx1, baseline + 1, cx2 - cx1, 8);
        ctx.globalAlpha = 0.03;
        ctx.fillRect(cx1, baseline + 9, cx2 - cx1, 6);
        ctx.globalAlpha = 1;
    }

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

        // Vessel tube: centered on baseline, symmetric above/below
        // Tube scales with zoom so vessel walls match cell spread
        var avgR = 4.0 * Math.max(zoom * 0.4, 0.8);
        var tubeH = Math.max(30, avgR * 2.5);
        // Skip vessel rendering if values are non-finite (canvas resize race)
        // but don't return: blips still need to render below
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

        // Vessel walls: symmetric tube boundary
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
        //: tested smooth to ~55k, and full-fit-at-congestion is rare. The console
        // toggle (_hbToggleLod) forces bricks always (or LOD).
        // Use the load-fill target count (if a first-load fill is in progress) so
        // the choice is stable from the first frame; otherwise the real blips count.
        var lodCount = seg._loadFillCount || seg.blips.length;
        if (_hb._lodEnabled !== false && 5 * zoom < 1.5 && lodCount > LOD_MIN_BLIPS) {
            drawFlatlineBlipsLOD(ctx, seg, viewLeft, viewRight, baseline, zoom, nowSec);
        } else if (_hb.renderMode !== 'bloodstream' && zoom <= 4) {
            drawFlatlineBricksBatched(ctx, _hb, seg, viewLeft, viewRight, baseline, zoom, nowSec);
        } else {
        // P3: for closed (immutable) segments, lazily sort blips by x once and
        // binary-search the visible window so a zoomed-in view of a dense
        // historical region only iterates on-screen bricks. The live (open)
        // segment mutates every flush, so we don't sort it, its per-blip cull
        // below is already cheap.
        var _wStart = 0;
        var _wSorted = false;
        var _isProtectedSeg = seg.x_end === null
            || !!seg._pendingForHash
            || (_hb._reveal && _hb._reveal.oldSeg === seg)
            || (_hb._pendingReveal && _hb._pendingReveal.oldSeg === seg);
        if (!_isProtectedSeg && seg.blips.length > 512) {
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
        // Zoom-invariant per-frame constants, hoisted out of the per-brick loop
        // (recomputing these ~40k times/frame at a full zoomed-out mempool is waste).
        var heightScale = brickHeightScale(zoom);
        var lw = zoom > 10 ? 1.5 : zoom > 6 ? 1 : 0.5;
        for (var bi = _wStart; bi < seg.blips.length; bi++) {
            var blip = seg.blips[bi];
            if (!blip || blip._removed) continue;
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

                    // Draw particle (no shadowBlur: see brick-rendering note)
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
                var bh = (blip.brickH || blipH) * heightScale;
                var sy = (blip.stackY || 0) * heightScale;

                // Full-size sky-to-baseline drop at close zoom: fast opacity entry over
                // the first 18% of the fall so incoming bricks stay clearly visible from
                // high above, followed by a subtle landing bounce and brief edge glint.
                var age = nowSec - blip.timestamp;
                if (age < 0) continue; // staggered history brick not yet visible
                var isNotable = !!blip.notableType;
                var fadeTime = isNotable ? 1.15 : 0.85;
                var fadeIn = age >= fadeTime ? 1 : age / fadeTime;
                var dropOffset = 0;
                var trailAlpha = 0;
                var landingFlash = 0;
                if (fadeIn < 1 && _hb.renderMode !== 'bloodstream') {
                    var inv = 1 - fadeIn;
                    var dropEase = Math.pow(inv, 2.2);
                    if (fadeIn > 0.78) {
                        var bounceAmt = isNotable ? 0.30 : 0.14;
                        dropEase += bounceAmt * Math.sin((fadeIn - 0.78) * (Math.PI / 0.22)) * 0.06;
                    }
                    var baseDrop = isNotable
                        ? Math.min(340, Math.max(220, baseline * 0.65))
                        : Math.min(260, Math.max(170, baseline * 0.50));
                    var hVar = 0.82 + ((blip.bobPhase || 0) / 6.2831853) * 0.36;
                    dropOffset = dropEase * baseDrop * hVar;
                    if (fadeIn < 0.78) {
                        trailAlpha = Math.sin((fadeIn / 0.78) * Math.PI) * 0.38;
                    }
                    bOpacity *= Math.min(1, fadeIn / 0.18);
                } else if (age >= fadeTime && age < fadeTime + 0.28 && _hb.renderMode !== 'bloodstream') {
                    landingFlash = 1 - (age - fadeTime) / 0.28;
                }
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
                    if (zoom > 14 && cellRadius > 8) {
                        ctx.font = Math.round(cellRadius * 0.5) + 'px monospace';
                        ctx.textAlign = 'center';
                        ctx.textBaseline = 'middle';
                        ctx.fillStyle = 'rgba(255, 255, 255, 0.85)';
                        ctx.fillText(blip.feePending ? 'pending' : formatFeeRate(blip.feeRate), cellCX, cellCY);
                    }

                    // Persistent highlight ring around the pinned cell
                    if (_hb._pinnedTxid && blip.txid === _hb._pinnedTxid) {
                        ctx.save();
                        ctx.strokeStyle = '#ffffff';
                        ctx.lineWidth = Math.max(2, Math.min(3.5, zoom * 0.3));
                        ctx.shadowColor = '#00e676';
                        ctx.shadowBlur = 10;
                        ctx.beginPath();
                        ctx.arc(cellCX, cellCY, cellRadius + 3, 0, Math.PI * 2);
                        ctx.stroke();
                        ctx.restore();
                    }
                } else {
                    // Brick rendering (original)
                    var brickBaseColor = blip.feePending && !blip.notableType
                        ? PENDING_FEE_COLOR
                        : blipColor;
                    // Draw filled rectangle with 1px gap for separation
                    // Gap scales with zoom so bricks stay visually separated.
                    // strokeRect bleeds half its lineWidth outside the rect,
                    // so the gap must exceed that to remain visible. (lw hoisted above.)
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

                    var useRound = zoom >= 5 && rw > 4 && rh > 4;
                    var cornerR = useRound ? Math.min(3.5, rw * 0.14, rh * 0.18) : 0;
                    var isHoveredBlip = (_hb.hoveredBlip === blip);

                    // Subtle vertical motion-trail streak directly above a falling brick
                    if (trailAlpha > 0.01 && rw >= 3) {
                        var trailH = Math.min(44, Math.max(16, rh * 1.4));
                        var trailTop = ry - trailH;
                        var tGrad = ctx.createLinearGradient(rx, trailTop, rx, ry);
                        tGrad.addColorStop(0, brickBaseColor + '0)');
                        tGrad.addColorStop(1, brickBaseColor + (trailAlpha * bOpacity).toFixed(3) + ')');
                        ctx.fillStyle = tGrad;
                        ctx.fillRect(rx + rw * 0.15, trailTop, rw * 0.7, trailH);
                    }

                    // At zoom >= 5, use a subtle top-bevel vertical gradient for crisp tile depth
                    var spotBandZoom = (typeof _hb._spotlightBand === 'number' && _hb._spotlightBand >= 0) ? _hb._spotlightBand : -1;
                    var isPinnedZoom = !!(_hb._pinnedTxid && blip.txid === _hb._pinnedTxid);
                    var dimmedZoom = spotBandZoom >= 0 && !isPinnedZoom && feeBandIndex(blip.feeRate, blip.feePending) !== spotBandZoom;
                    var drawAlpha = isHoveredBlip ? Math.min(1, bOpacity + 0.18) : bOpacity;
                    if (dimmedZoom) {
                        drawAlpha = Math.max(0.06, drawAlpha * 0.14);
                    }
                    if (zoom >= 5 && rh > 5) {
                        var rgbMatch = brickBaseColor.match(/(\d+)\D+(\d+)\D+(\d+)/);
                        if (rgbMatch) {
                            var bR = parseInt(rgbMatch[1]);
                            var bG = parseInt(rgbMatch[2]);
                            var bB = parseInt(rgbMatch[3]);
                            var hoverBoost = isHoveredBlip ? 24 : 0;
                            var topR = Math.min(bR + 28 + hoverBoost, 255);
                            var topG = Math.min(bG + 22 + hoverBoost, 255);
                            var topB = Math.min(bB + 18 + hoverBoost, 255);
                            var botR = Math.max(bR - 26 + Math.floor(hoverBoost * 0.5), 0);
                            var botG = Math.max(bG - 22 + Math.floor(hoverBoost * 0.5), 0);
                            var botB = Math.max(bB - 18 + Math.floor(hoverBoost * 0.5), 0);
                            var bGrad = ctx.createLinearGradient(rx, ry, rx, ry + rh);
                            bGrad.addColorStop(0, 'rgba(' + topR + ',' + topG + ',' + topB + ',' + drawAlpha + ')');
                            bGrad.addColorStop(0.45, 'rgba(' + Math.min(bR + hoverBoost, 255) + ',' + Math.min(bG + hoverBoost, 255) + ',' + Math.min(bB + hoverBoost, 255) + ',' + drawAlpha + ')');
                            bGrad.addColorStop(1, 'rgba(' + botR + ',' + botG + ',' + botB + ',' + drawAlpha + ')');
                            ctx.fillStyle = bGrad;
                        } else {
                            ctx.fillStyle = brickBaseColor + drawAlpha + ')';
                        }
                    } else {
                        ctx.fillStyle = brickBaseColor + drawAlpha + ')';
                    }

                    // No shadowBlur. The glow was the dominant render cost (an
                    // offscreen blur pass per brick per frame: the "lag on large
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
                    if (blip.notableType && !dimmedZoom) {
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

                    // Outline at 4x+ zoom: subtle dashed slate outline for fee-pending bricks,
                    // crisp dark outline for priced bricks (suppressed when dimmed by spotlight).
                    if (!dimmedZoom && zoom >= 4 && !blip.notableType) {
                        if (blip.feePending) {
                            ctx.save();
                            var pendLw = Math.max(1, lw);
                            var pendIns = pendLw / 2;
                            ctx.strokeStyle = 'rgba(148, 163, 184, 0.75)';
                            ctx.lineWidth = pendLw;
                            if (typeof ctx.setLineDash === 'function') {
                                ctx.setLineDash([Math.max(2, zoom * 0.35), Math.max(2, zoom * 0.25)]);
                            }
                            if (useRound) {
                                ctx.beginPath();
                                roundRect(ctx, rx + pendIns, ry + pendIns, Math.max(0.5, rw - pendLw), Math.max(0.5, rh - pendLw), Math.max(cornerR - pendIns, 0));
                                ctx.stroke();
                            } else {
                                ctx.strokeRect(rx + pendIns, ry + pendIns, Math.max(0.5, rw - pendLw), Math.max(0.5, rh - pendLw));
                            }
                            ctx.restore();
                        } else if (zoom > 4) {
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
                    }

                    // Brief crisp top-edge glint right as a falling brick lands on its stack
                    if (landingFlash > 0.02 && rw > 4 && rh > 3) {
                        ctx.save();
                        ctx.strokeStyle = 'rgba(255, 255, 255, ' + (landingFlash * 0.55).toFixed(3) + ')';
                        ctx.lineWidth = 1.5;
                        if (useRound) {
                            ctx.beginPath();
                            roundRect(ctx, rx + 0.75, ry + 0.75, Math.max(0.5, rw - 1.5), Math.max(0.5, rh - 1.5), Math.max(cornerR - 0.75, 0));
                            ctx.stroke();
                        } else {
                            ctx.strokeRect(rx + 0.75, ry + 0.75, Math.max(0.5, rw - 1.5), Math.max(0.5, rh - 1.5));
                        }
                        ctx.restore();
                    }

                    // Tactile hover border + soft fee-color glow on the hovered brick
                    if (isHoveredBlip && (!_hb._pinnedTxid || blip.txid !== _hb._pinnedTxid)) {
                        ctx.save();
                        ctx.strokeStyle = 'rgba(255, 255, 255, 0.92)';
                        ctx.lineWidth = Math.max(1.5, Math.min(2.2, zoom * 0.2));
                        ctx.shadowColor = brickBaseColor + '0.85)';
                        ctx.shadowBlur = 10;
                        if (useRound) {
                            ctx.beginPath();
                            roundRect(ctx, rx - 0.75, ry - 0.75, rw + 1.5, rh + 1.5, cornerR + 0.75);
                            ctx.stroke();
                        } else {
                            ctx.strokeRect(rx - 0.75, ry - 0.75, rw + 1.5, rh + 1.5);
                        }
                        ctx.restore();
                    }

                    // At zoom >= 5: show fee rate + value on brick face (clipped to brick)
                    if (!dimmedZoom && zoom >= 5 && rw > 20 && rh > 11) {
                        ctx.save();
                        ctx.beginPath();
                        if (useRound) {
                            roundRect(ctx, rx, ry, rw, rh, cornerR);
                        } else {
                            ctx.rect(rx, ry, rw, rh);
                        }
                        ctx.clip();
                        var padX = Math.max(3, Math.min(6, rw * 0.07));
                        var padY = Math.max(3, rh * 0.11);
                        var availW = Math.max(4, rw - padX * 2);
                        var feeStr = blip.feePending
                            ? 'pending'
                            : (formatFeeRate(blip.feeRate) + ' s/vB');
                        // Monospace glyphs average ~0.63*fs wide; fit feeStr inside availW with zero right-edge clip.
                        var maxFsForWidth = Math.floor(availW / (Math.max(1, feeStr.length) * 0.64));
                        var fs = Math.min(Math.floor(rh * 0.34), maxFsForWidth, 13);
                        if (fs >= 5) {
                            // Fee rate text
                            ctx.font = 'bold ' + fs + 'px monospace';
                            ctx.textAlign = 'left';
                            ctx.textBaseline = 'top';
                            var textY = ry + padY;
                            // Dark text shadow for readability on any color
                            ctx.fillStyle = 'rgba(0,0,0,0.55)';
                            ctx.fillText(feeStr, rx + padX + 1, textY + 1);
                            ctx.fillStyle = blip.feePending ? 'rgba(226,232,240,0.85)' : 'rgba(255,255,255,0.95)';
                            ctx.fillText(feeStr, rx + padX, textY);

                            // Value text (second line)
                            if (rh > 23 && fs >= 6) {
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
                                    var maxValFs = Math.floor(availW / (valText.length * 0.64));
                                    var valFs = Math.min(fs - 1, maxValFs);
                                    if (valFs >= 5) {
                                        ctx.font = valFs + 'px monospace';
                                        var valY = textY + fs + Math.max(3, rh * 0.08);
                                        ctx.fillStyle = 'rgba(0,0,0,0.45)';
                                        ctx.fillText(valText, rx + padX + 1, valY + 1);
                                        ctx.fillStyle = 'rgba(255,255,255,0.8)';
                                        ctx.fillText(valText, rx + padX, valY);
                                    }
                                }
                            }
                        }
                        ctx.restore();
                    }

                    // Persistent highlight outline/ring around the currently pinned brick
                    // so it stays unmistakably identified even after the 5s search pulse fades.
                    if (_hb._pinnedTxid && blip.txid === _hb._pinnedTxid) {
                        ctx.save();
                        var pPad = Math.max(2, Math.min(4, zoom * 0.35));
                        var pLw = Math.max(2, Math.min(3.5, zoom * 0.3));
                        ctx.strokeStyle = '#ffffff';
                        ctx.lineWidth = pLw;
                        ctx.shadowColor = '#00e676';
                        ctx.shadowBlur = 10;
                        if (useRound) {
                            ctx.beginPath();
                            roundRect(ctx, rx - pPad, ry - pPad, rw + pPad * 2, rh + pPad * 2, cornerR + pPad * 0.5);
                            ctx.stroke();
                        } else {
                            ctx.strokeRect(rx - pPad, ry - pPad, rw + pPad * 2, rh + pPad * 2);
                        }
                        ctx.strokeStyle = '#00e676';
                        ctx.lineWidth = Math.max(1, pLw * 0.5);
                        ctx.shadowBlur = 0;
                        var outerPad = pPad + 1.5;
                        if (useRound) {
                            ctx.beginPath();
                            roundRect(ctx, rx - outerPad, ry - outerPad, rw + outerPad * 2, rh + outerPad * 2, cornerR + outerPad * 0.5);
                            ctx.stroke();
                        } else {
                            ctx.strokeRect(rx - outerPad, ry - outerPad, rw + outerPad * 2, rh + outerPad * 2);
                        }
                        ctx.restore();
                    }
                }
            }
            ctx.shadowBlur = 0;
        }
        } // end else (non-LOD per-blip draw)

        // Prune blips: never prune the live (open) flatline, nor a flatline sealed
        // by an unresolved pending_block or active reveal, so all mempool bricks
        // persist until harvested and carried forward. Only prune finished historical segments.
        var isLiveSeg = (seg.type === 'flatline') && (
            seg.x_end === null
            || !!seg._pendingForHash
            || (_hb._reveal && _hb._reveal.oldSeg === seg)
            || (_hb._pendingReveal && _hb._pendingReveal.oldSeg === seg)
        );
        if (!isLiveSeg && seg.blips.length > MAX_BLIPS_PER_SEGMENT) {
            // This block reassigns/reorders seg.blips (the concat below is not
            // x-ordered), so invalidate the P3 sort cache, the next draw will
            // re-sort lazily before binary-searching.
            seg._sortedByX = false;
            var cutoff = nowSec;
            // Always remove faded absorption particles (after max ~6.01s flight finishes)
            seg.blips = seg.blips.filter(function(b) {
                if (b.fadeStart > 0) return (cutoff - b.fadeStart) < 6.5;
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
                if (seg.blips[fi].fadeStart > 0 && (cutoff2 - seg.blips[fi].fadeStart) >= 6.5) {
                    hasFaded = true; break;
                }
            }
            if (hasFaded) {
                var keptLive = [];
                for (var fl = 0; fl < seg.blips.length; fl++) {
                    var lb = seg.blips[fl];
                    if (!lb || lb._removed) continue;
                    if (lb.fadeStart > 0 && (cutoff2 - lb.fadeStart) >= 6.5) continue;
                    lb._segIdx = keptLive.length;
                    keptLive.push(lb);
                }
                seg.blips = keptLive;
            }
        }
    }
}
