// heartbeat-vitals.js: Vital signs, rhythm strip, search, effects, capture
//
// Block audio lives in heartbeat-audio.js, not here: it is cued from the SSE handler
// so it also fires for a hidden tab, which this module's effects deliberately do not.
import { getState, COLORS, POINT_WIDTH, HEAD_POSITION_FRAC, FLATLINE_PX_PER_SEC, RHYTHM_STRIP_BLOCKS, BG_COLOR, GRID_COLOR } from './heartbeat-state.js';
import { generatePQRST, computeColor, hexToRgb, formatDuration, resolveInterBlockSeconds, historyFlatlineWidth, waitColor, buildFlatlineGradient, UNKNOWN_INTERVAL_COLOR } from './heartbeat-timeline.js';

// ── Module-level audio state ─────────────────────────────

// ── Vital Signs Computation ──────────────────────────────

// Compute blocks per 10 minutes from the last ~20 blocks (~3hr rolling window)
export function computeHeartRate() {
    var _hb = getState();
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
export function computeTemperature(mempoolMB) {
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
export function computeImmune(hashrateEH) {
    var label, color;
    if (hashrateEH >= 900) {
        label = 'Excellent'; color = COLORS.healthy;
    } else if (hashrateEH >= 600) {
        label = 'Strong'; color = COLORS.healthy;
    } else if (hashrateEH >= 300) {
        label = 'Moderate'; color = COLORS.elevated;
    } else {
        label = 'Weak'; color = COLORS.stressed;
    }
    return { hashrate: hashrateEH, label: label, color: color };
}

// Blood pressure: systolic = next block fee, diastolic = mempool min fee
export function computeBloodPressure(nextBlockFee, mempoolMinFee) {
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
export function getHeartbeatVitals() {
    var _hb = getState();
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
}

// ── 24-Hour Rhythm Strip ─────────────────────────────────

function clearNodeChildren(el) {
    if (!el) return;
    while (el.firstChild) {
        el.removeChild(el.firstChild);
    }
    el.textContent = '';
}

function renderRhythmDetailDom(canvas, detailEl, idx) {
    if (!detailEl) return;
    clearNodeChildren(detailEl);
    var regions = canvas._rhythmHitRegions;
    if (!regions || idx < 0 || idx >= regions.length) {
        canvas._rhythmSelected = null;
        return;
    }
    var r = regions[idx];
    var b = r.block;
    var feeBtc = ((b.total_fees || 0) / 1e8).toFixed(4);
    var wait = formatDuration(r.interBlockSeconds);

    var hSpan = document.createElement('span');
    hSpan.style.color = r.color;
    hSpan.style.fontWeight = '600';
    hSpan.textContent = 'Block #' + (b.height || 0).toLocaleString();
    detailEl.appendChild(hSpan);

    var txSpan = document.createElement('span');
    txSpan.textContent = (b.tx_count || 0).toLocaleString() + ' txs';
    detailEl.appendChild(txSpan);

    var feeSpan = document.createElement('span');
    feeSpan.textContent = feeBtc + ' BTC fees';
    detailEl.appendChild(feeSpan);

    var waitSpan = document.createElement('span');
    waitSpan.textContent = wait + ' wait';
    detailEl.appendChild(waitSpan);

    var btn = document.createElement('button');
    btn.type = 'button';
    btn.textContent = 'View Block Data';
    btn.style.color = r.color;
    btn.style.fontWeight = '600';
    btn.style.textDecoration = 'underline';
    btn.style.textUnderlineOffset = '2px';
    btn.style.cursor = 'pointer';
    btn.style.background = 'transparent';
    btn.style.border = 'none';
    btn.style.padding = '0';
    btn.style.font = 'inherit';
    var blockHeight = b.height || 0;
    btn.addEventListener('click', function() {
        if (typeof window.showBlockDetail === 'function') {
            window.showBlockDetail(blockHeight);
        }
    });
    detailEl.appendChild(btn);

    canvas._rhythmSelected = r;
}

export function renderRhythmStrip(canvasId, blocksJson, _skipSliderSync) {
    var canvas = document.getElementById(canvasId);
    if (!canvas) return;

    var blocks;
    if (Array.isArray(blocksJson)) {
        blocks = blocksJson;
    } else {
        try {
            blocks = JSON.parse(blocksJson);
        } catch(e) {
            return;
        }
    }
    if (!Array.isArray(blocks) || blocks.length === 0) return;

    // Always store the latest blocks array on the canvas so slider/touch handlers
    // never close over a stale initial-load snapshot.
    canvas._rhythmBlocks = blocks;

    var dpr = Math.min(window.devicePixelRatio || 1, 2);
    var rect = canvas.getBoundingClientRect();
    var w = rect.width || 600;
    var h = rect.height || 80;
    canvas.width = w * dpr;
    canvas.height = h * dpr;

    var ctx = canvas.getContext('2d');
    if (ctx) {
        if (typeof ctx.scale === 'function') ctx.scale(dpr, dpr);
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
    }

    // Generate all waveform data with exact inter-block seconds & progressive width
    var waveforms = [];
    var totalPoints = 0;
    for (var i = 0; i < blocks.length; i++) {
        var blk = blocks[i];
        var rawInter = resolveInterBlockSeconds(blk);
        if (rawInter === null && i > 0 && blk.timestamp && blocks[i - 1].timestamp) {
            rawInter = blk.timestamp - blocks[i - 1].timestamp;
        }
        var pts = generatePQRST(rawInter === null ? blk : Object.assign({}, blk, { inter_block_seconds: rawInter }));
        var flatPx = Math.max(2, Math.min(historyFlatlineWidth(rawInter) / 14, 115));
        waveforms.push({ points: pts, flatline: flatPx, block: blk, interBlockSeconds: rawInter });
        totalPoints += pts.length + flatPx;
    }

    // Scale to fit canvas width
    var scale = totalPoints > 0 ? (w / totalPoints) : 1;
    if (scale > 3) scale = 3; // cap individual point width

    // Draw each block segment with progressive wait gradient on flatline and color on spike
    var x = 0;
    var baselineY = h * 0.55;
    var vScale = 0.4; // compress vertically
    var hitRegions = [];
    for (var wi = 0; wi < waveforms.length; wi++) {
        var wf = waveforms[wi];
        var rawInter2 = wf.interBlockSeconds;
        var effInter = rawInter2 === null ? null : Math.max(0, rawInter2);
        var wfVbytes = wf.block.weight > 0 ? (wf.block.weight / 4) : (wf.block.size > 0 ? wf.block.size : 1000000);
        var feeRate = wf.block.total_fees ? (wf.block.total_fees / wfVbytes) : 0;
        var segColor = rawInter2 === null ? UNKNOWN_INTERVAL_COLOR : computeColor(effInter, feeRate, 0);

        var flatStart = x;
        var flatEnd = x + wf.flatline * scale;
        var waveStart = flatEnd;
        x = flatEnd;

        if (ctx) {
            // Flatline leading into the spike: progressive wait color gradient (or neutral slate if unknown)
            ctx.beginPath();
            ctx.moveTo(flatStart, baselineY);
            ctx.lineTo(flatEnd, baselineY);
            ctx.strokeStyle = rawInter2 === null
                ? UNKNOWN_INTERVAL_COLOR
                : buildFlatlineGradient(ctx, flatStart, flatEnd, effInter);
            ctx.lineWidth = 1;
            ctx.shadowBlur = 3;
            ctx.shadowColor = rawInter2 === null ? UNKNOWN_INTERVAL_COLOR : waitColor(effInter);
            ctx.stroke();

            // Waveform spike
            ctx.beginPath();
            ctx.moveTo(waveStart, baselineY);
            for (var pi = 0; pi < wf.points.length; pi++) {
                x += scale;
                ctx.lineTo(x, baselineY + wf.points[pi] * vScale);
            }
            ctx.strokeStyle = segColor;
            ctx.lineWidth = 1;
            ctx.shadowBlur = 4;
            ctx.shadowColor = segColor;
            ctx.stroke();
            ctx.shadowBlur = 0;
        } else {
            x += wf.points.length * scale;
        }

        hitRegions.push({
            x1: waveStart,
            x2: x,
            block: wf.block,
            color: segColor,
            interBlockSeconds: rawInter2
        });
    }

    // Draw UTC hour markers interpolated between actual drawn block x-positions (A16)
    if (ctx && hitRegions.length > 1) {
        var firstTs = hitRegions[0].block.timestamp;
        var lastTs = hitRegions[hitRegions.length - 1].block.timestamp;
        var spanHrs = (lastTs - firstTs) / 3600;
        if (firstTs && lastTs && spanHrs > 1) {
            ctx.fillStyle = 'rgba(255,255,255,0.52)';
            ctx.font = '10px monospace';
            var interval = spanHrs > 12 ? 4 : 2;
            var startHour = Math.ceil((firstTs / 3600)) * 3600;
            for (var ts = startHour; ts < lastTs; ts += interval * 3600) {
                var mx = null;
                for (var hi = 0; hi < hitRegions.length - 1; hi++) {
                    var r0 = hitRegions[hi];
                    var r1 = hitRegions[hi + 1];
                    var t0 = r0.block.timestamp;
                    var t1 = r1.block.timestamp;
                    if (t0 <= ts && t1 >= ts) {
                        var x0 = (r0.x1 + r0.x2) / 2;
                        var x1 = (r1.x1 + r1.x2) / 2;
                        mx = t1 > t0 ? x0 + ((ts - t0) / (t1 - t0)) * (x1 - x0) : x0;
                        break;
                    }
                }
                if (mx === null) continue;
                ctx.fillRect(mx, 0, 0.5, h);
                var d = new Date(ts * 1000);
                var hourLabel = d.getUTCHours() + ':00';
                ctx.fillText(hourLabel, mx + 2, h - 3);
            }
        }
    }

    // ── Rhythm strip interactivity: hit regions + dynamic slider sync ──
    canvas._rhythmHitRegions = hitRegions;

    var slider = document.getElementById('rhythm-strip-slider');
    var detailEl = document.getElementById('rhythm-strip-detail');

    if (slider && detailEl && !_skipSliderSync) {
        var prevMax = parseInt(slider.max, 10);
        var prevVal = parseInt(slider.value, 10);
        var newMax = Math.max(0, blocks.length - 1);
        var wasAtEnd = !canvas._rhythmHandlersSet || isNaN(prevVal) || isNaN(prevMax) || prevVal >= prevMax;
        slider.max = String(newMax);
        var targetIdx;
        if (wasAtEnd) {
            targetIdx = newMax;
        } else {
            var trackedHeight = (canvas._rhythmSelected && canvas._rhythmSelected.block)
                ? canvas._rhythmSelected.block.height
                : null;
            var foundIdx = -1;
            if (trackedHeight != null) {
                for (var ti = 0; ti < blocks.length; ti++) {
                    if (blocks[ti] && blocks[ti].height === trackedHeight) {
                        foundIdx = ti;
                        break;
                    }
                }
            }
            targetIdx = foundIdx >= 0 ? foundIdx : Math.min(Math.max(0, prevVal), newMax);
        }
        slider.value = String(targetIdx);
        renderRhythmDetailDom(canvas, detailEl, targetIdx);
    }

    // Draw highlight for slider-selected block (not mouse hover)
    if (ctx && canvas._rhythmSelected) {
        var sel = canvas._rhythmSelected;
        for (var hri = 0; hri < hitRegions.length; hri++) {
            if (hitRegions[hri].block.height === sel.block.height) {
                sel = hitRegions[hri];
                canvas._rhythmSelected = sel;
                break;
            }
        }
        ctx.strokeStyle = sel.color;
        ctx.lineWidth = 2;
        ctx.globalAlpha = 0.6;
        ctx.beginPath();
        ctx.moveTo(sel.x1, baselineY + 4);
        ctx.lineTo(sel.x2, baselineY + 4);
        ctx.stroke();
        ctx.globalAlpha = 1;
    }

    // Only set up event handlers once (skip on redraws)
    if (canvas._rhythmHandlersSet) return;
    canvas._rhythmHandlersSet = true;

    // Click -> open block detail modal
    canvas.addEventListener('click', function(e) {
        var r = canvas.getBoundingClientRect();
        var mx = e.clientX - r.left;
        var regions = canvas._rhythmHitRegions;
        if (!regions) return;
        for (var ri = 0; ri < regions.length; ri++) {
            if (mx >= regions[ri].x1 && mx <= regions[ri].x2 && regions[ri].block.height != null && regions[ri].block.height >= 0) {
                if (typeof window.showBlockDetail === 'function') {
                    window.showBlockDetail(regions[ri].block.height);
                }
                return;
            }
        }
    });

    // Pointer cursor on hover (no tooltip overlay)
    canvas.addEventListener('mousemove', function(e) {
        var r = canvas.getBoundingClientRect();
        var mx = e.clientX - r.left;
        var regions = canvas._rhythmHitRegions;
        var over = false;
        if (regions) {
            for (var ri = 0; ri < regions.length; ri++) {
                if (mx >= regions[ri].x1 && mx <= regions[ri].x2) {
                    over = true;
                    break;
                }
            }
        }
        canvas.style.cursor = over ? 'pointer' : '';
    });

    canvas.addEventListener('mouseleave', function() {
        canvas.style.cursor = '';
    });

    // ── Slider scrubber for block selection ──────────────
    if (slider && detailEl) {
        function updateSliderDetail(idx) {
            renderRhythmDetailDom(canvas, detailEl, idx);
            if (canvas._rhythmBlocks) {
                renderRhythmStrip(canvasId, canvas._rhythmBlocks, true);
            }
        }

        slider.oninput = function() {
            updateSliderDetail(parseInt(slider.value, 10));
        };

        // On mobile: tap canvas selects nearest block via slider (no redirect)
        canvas.addEventListener('touchstart', function(e) {
            var touch = e.touches[0];
            var rect2 = canvas.getBoundingClientRect();
            var mx = touch.clientX - rect2.left;
            var regions = canvas._rhythmHitRegions;
            if (!regions || regions.length === 0) return;
            var bestIdx = 0, bestDist = Infinity;
            for (var ri = 0; ri < regions.length; ri++) {
                var mid = (regions[ri].x1 + regions[ri].x2) / 2;
                var d = Math.abs(mx - mid);
                if (d < bestDist) { bestDist = d; bestIdx = ri; }
            }
            e.preventDefault();
            slider.value = String(bestIdx);
            updateSliderDetail(bestIdx);
        }, { passive: false });
    }
}

// ── Center view on live head ─────────────────────────────

export function heartbeatCenter() {
    var _hb = getState();
    if (!_hb) return;
    if (_hb._viewMode === 'history' && typeof window !== 'undefined' && typeof window.hbV2ExitHistoryMode === 'function') {
        window.hbV2ExitHistoryMode();
        return;
    }
    _hb.autoFollow = true;
    _hb.viewOffsetY = 0;
    _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
    _hb.hoveredBlock = null;
    _hb.hoveredPendingBlock = null;
    _hb.hoveredBlip = null;
    _hb._pinnedBlip = null;
    _hb._pinnedTxid = null;
    if (typeof window !== 'undefined' && typeof window.__hbClearPinnedTx === 'function') {
        window.__hbClearPinnedTx();
    }
}

// Search for a txid across the timeline via the v2 focus entry point.
export function heartbeatSearchTx(txid) {
    if (typeof window !== 'undefined' && typeof window.hbV2FocusTx === 'function') {
        return window.hbV2FocusTx(txid);
    }
    return false;
}

// ── Effects ──────────────────────────────────────────────

// Background breathing pulse on block arrival
export function heartbeatPulse() {
    var _hb = getState();
    var card = document.getElementById('heartbeat-card');
    if (!card) return;
    card.style.transition = 'box-shadow 0.3s ease-in';
    var color = (_hb && _hb.currentColor) ? _hb.currentColor : COLORS.healthy;
    card.style.boxShadow = '0 0 30px ' + color + '40, inset 0 0 15px ' + color + '15';
    setTimeout(function() {
        card.style.transition = 'box-shadow 1.2s ease-out';
        card.style.boxShadow = 'none';
    }, 300);
}

// Flash the EKG line white on new block
// Stores the real color in _preFlashColor so flatline stamping isn't affected
export function heartbeatFlash() {
    var _hb = getState();
    if (!_hb) return;
    _hb._preFlashColor = _hb.currentColor; // preserve for flatline stamping
    _hb._flashColor = _hb.currentColor;
    _hb.currentColor = '#ffffff';
    _hb.prevColor = '#ffffff';
    _hb.colorLerp = 1;
    if (_hb._flashTimer) clearTimeout(_hb._flashTimer);
    _hb._flashTimer = setTimeout(function() {
        if (_hb && _hb._flashColor) {
            // Skip the lerp: just snap to the correct color.
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
}

// Organism status text
export function getOrganismStatus() {
    var _hb = getState();
    if (!_hb) return '{}';
    var now = Date.now() / 1000;
    var elapsed = _hb.lastBlockTime > 0 ? Math.max(0, now - _hb.lastBlockTime) : 0;
    var fee = _hb.nextBlockFee || 0;
    var feeBoostSec = fee >= 30 ? Math.min(600, ((fee - 30) / 120) * 600) : 0;
    var effectiveWait = elapsed + feeBoostSec;
    var color = computeColor(elapsed, fee, _hb.mempoolMB);

    var condition, description;
    if (effectiveWait >= 3600) {
        condition = 'Critical';
        description = 'Holding its breath. Waiting for the next block...';
    } else if (effectiveWait >= 1800) {
        condition = 'Stressed';
        description = 'Heavy breathing. Block space is scarce.';
    } else if (effectiveWait >= 1200) {
        condition = 'Elevated';
        description = 'Running warm. Activity is rising.';
    } else if (effectiveWait >= 600) {
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
}

// ── Moment Capture ───────────────────────────────────────

export function heartbeatCapture(vitalsJson) {
    var _hb = getState();
    if (!_hb) return null;

    // Create an offscreen canvas for the share card
    var cardW = 1200, cardH = 675;
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

    // Copy current EKG canvas: larger share of the card
    try {
        var ekgH = 350;
        octx.drawImage(_hb.canvas, 20, 20, cardW - 40, ekgH);
    } catch(e) {}

    // Draw vital signs text
    var vitals;
    try { vitals = JSON.parse(vitalsJson); } catch(e) { vitals = {}; }

    octx.font = 'bold 22px monospace';
    octx.fillStyle = color;
    var y = 395;
    var col1 = 40, col2 = 340, col3 = 640, col4 = 940;

    // Heart Rate
    octx.fillStyle = vitals.heart_rate_color || color;
    octx.fillText('Heart Rate', col1, y);
    octx.font = 'bold 32px monospace';
    var hrBpm = vitals.heart_rate_bpm || 0;
    var hrPct = typeof hrBpm === 'number' ? Math.round(hrBpm * 100) : hrBpm;
    octx.fillText(hrPct + '% of target', col1, y + 36);
    octx.font = '16px monospace';
    octx.fillText(vitals.heart_rate_label || '', col1, y + 56);

    // Blood Pressure
    octx.font = 'bold 22px monospace';
    octx.fillStyle = vitals.bp_color || color;
    octx.fillText('Blood Pressure', col2, y);
    octx.font = 'bold 32px monospace';
    octx.fillText((vitals.bp_systolic || '0') + ' / ' + (vitals.bp_diastolic || '0'), col2, y + 36);
    octx.font = '16px monospace';
    octx.fillText(vitals.bp_label || '', col2, y + 56);

    // Temperature
    octx.font = 'bold 22px monospace';
    octx.fillStyle = vitals.temp_color || color;
    octx.fillText('Temperature', col3, y);
    octx.font = 'bold 32px monospace';
    octx.fillText((vitals.temp_c || '36.5') + '\u00B0C', col3, y + 36);
    octx.font = '16px monospace';
    octx.fillText(vitals.temp_label || '', col3, y + 56);

    // Immune System
    octx.font = 'bold 22px monospace';
    octx.fillStyle = vitals.immune_color || color;
    octx.fillText('Immune System', col4, y);
    octx.font = 'bold 32px monospace';
    octx.fillText((vitals.immune_eh || '0') + ' EH/s', col4, y + 36);
    octx.font = '16px monospace';
    octx.fillText(vitals.immune_label || '', col4, y + 56);

    // Organism status
    var status;
    try { status = JSON.parse(getOrganismStatus()); } catch(e) { status = {}; }
    octx.font = 'bold 20px monospace';
    octx.fillStyle = status.color || color;
    var statusY = 520;
    octx.fillText('Organism Status: ' + (status.condition || 'Unknown'), col1, statusY);
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
export function heartbeatDownloadCapture(vitalsJson) {
    var dataUrl = heartbeatCapture(vitalsJson);
    if (!dataUrl) return;
    var link = document.createElement('a');
    link.download = 'bitcoin-heartbeat-' + Date.now() + '.png';
    link.href = dataUrl;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
}

// Get recent blocks JSON for rhythm strip
export function getHeartbeatRecentBlocks() {
    var _hb = getState();
    if (!_hb || !_hb.recentBlocks) return '[]';
    // Return last 144 blocks for the 24-hour rhythm strip
    var blocks = _hb.recentBlocks;
    if (blocks.length > RHYTHM_STRIP_BLOCKS) {
        blocks = blocks.slice(-RHYTHM_STRIP_BLOCKS);
    }
    return JSON.stringify(blocks);
}
