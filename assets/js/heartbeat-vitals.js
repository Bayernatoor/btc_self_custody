// heartbeat-vitals.js — Vital signs, rhythm strip, search, effects, sound, capture
import { getState, COLORS, POINT_WIDTH, HEAD_POSITION_FRAC, FLATLINE_PX_PER_SEC, RHYTHM_STRIP_BLOCKS, BG_COLOR, GRID_COLOR } from './heartbeat-state.js';
import { generatePQRST, computeColor, hexToRgb } from './heartbeat-timeline.js';

// ── Module-level audio state ─────────────────────────────
var _audioCtx = null;
var _soundEnabled = false;

// ── Helper ───────────────────────────────────────────────
function formatDuration(sec) {
    if (sec <= 1) return '< 1s';
    if (sec < 60) return sec + 's';
    var m = Math.floor(sec / 60);
    var s = sec % 60;
    if (m < 60) return m + 'm ' + s + 's';
    var h = Math.floor(m / 60);
    m = m % 60;
    return h + 'h ' + m + 'm';
}

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

export function renderRhythmStrip(canvasId, blocksJson) {
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

    // Draw highlight for slider-selected block (not mouse hover)
    if (canvas._rhythmSelected) {
        var sel = canvas._rhythmSelected;
        var selMid = (sel.x1 + sel.x2) / 2;
        // Re-match against new hit regions (positions may shift on resize)
        for (var hri = 0; hri < hitRegions.length; hri++) {
            if (hitRegions[hri].block.height === sel.block.height) {
                sel = hitRegions[hri];
                canvas._rhythmSelected = sel;
                selMid = (sel.x1 + sel.x2) / 2;
                break;
            }
        }
        // Subtle highlight line under the selected spike
        ctx.strokeStyle = sel.color;
        ctx.lineWidth = 2;
        ctx.globalAlpha = 0.6;
        ctx.beginPath();
        ctx.moveTo(sel.x1, baseline + 4);
        ctx.lineTo(sel.x2, baseline + 4);
        ctx.stroke();
        ctx.globalAlpha = 1;
    }

    // Only set up event handlers once (skip on redraws)
    if (canvas._rhythmHandlersSet) return;
    canvas._rhythmHandlersSet = true;

    // Click → open block on mempool.space
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

    // Pointer cursor on hover (no tooltip overlay)
    canvas.addEventListener('mousemove', function(e) {
        var r = canvas.getBoundingClientRect();
        var mx = e.clientX - r.left;
        var regions = canvas._rhythmHitRegions;
        var over = false;
        for (var ri = 0; ri < regions.length; ri++) {
            if (mx >= regions[ri].x1 && mx <= regions[ri].x2) {
                over = true;
                break;
            }
        }
        canvas.style.cursor = over ? 'pointer' : '';
    });

    canvas.addEventListener('mouseleave', function() {
        canvas.style.cursor = '';
    });

    // ── Slider scrubber for block selection ──────────────
    var slider = document.getElementById('rhythm-strip-slider');
    var detailEl = document.getElementById('rhythm-strip-detail');
    if (slider && detailEl) {
        slider.max = Math.max(0, blocks.length - 1);
        slider.value = blocks.length - 1;

        function updateSliderDetail(idx) {
            var regions = canvas._rhythmHitRegions;
            if (!regions || idx < 0 || idx >= regions.length) {
                detailEl.innerHTML = '';
                canvas._rhythmSelected = null;
                renderRhythmStrip(canvasId, blocksJson);
                return;
            }
            var r = regions[idx];
            var b = r.block;
            var feeBtc = ((b.total_fees || 0) / 1e8).toFixed(4);
            var wait = formatDuration(b.inter_block_seconds || 600);
            detailEl.innerHTML =
                '<span style="color:' + r.color + '">Block #' + (b.height || 0).toLocaleString() + '</span>' +
                '<span>' + (b.tx_count || 0).toLocaleString() + ' txs</span>' +
                '<span>' + feeBtc + ' BTC fees</span>' +
                '<span>' + wait + ' wait</span>' +
                '<a href="https://mempool.space/block/' + (b.height || 0) + '" target="_blank" ' +
                'style="color:' + r.color + ';text-decoration:underline;text-underline-offset:2px">' +
                'View on mempool.space \u2197</a>';
            // Highlight on canvas
            canvas._rhythmSelected = r;
            renderRhythmStrip(canvasId, blocksJson);
        }

        slider.oninput = function() {
            updateSliderDetail(parseInt(slider.value, 10));
        };

        // On mobile: tap canvas selects nearest block via slider (no redirect)
        canvas.addEventListener('touchstart', function(e) {
            var touch = e.touches[0];
            var rect = canvas.getBoundingClientRect();
            var mx = touch.clientX - rect.left;
            var regions = canvas._rhythmHitRegions;
            if (!regions) return;
            var bestIdx = 0, bestDist = Infinity;
            for (var ri = 0; ri < regions.length; ri++) {
                var mid = (regions[ri].x1 + regions[ri].x2) / 2;
                var d = Math.abs(mx - mid);
                if (d < bestDist) { bestDist = d; bestIdx = ri; }
            }
            e.preventDefault();
            slider.value = bestIdx;
            updateSliderDetail(bestIdx);
        }, { passive: false });

        // Show the last block by default
        updateSliderDetail(blocks.length - 1);
    }
}

// ── Center view on live head ─────────────────────────────

export function heartbeatCenter() {
    var _hb = getState();
    if (!_hb) return;
    _hb.autoFollow = true;
    _hb.viewOffsetY = 0;
    _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
    _hb.hoveredBlock = null;
    _hb.hoveredBlip = null;
    _hb._pinnedBlip = null;
}

// Search for a txid across all flatline segments. If found, scroll to
// it, zoom in, and highlight with a pulsing glow.
export function heartbeatSearchTx(txid) {
    var _hb = getState();
    if (!_hb || !txid) return false;
    var needle = txid.trim().toLowerCase();
    if (needle.length < 8) return false;

    // Scan all flatline segments for a matching blip
    for (var si = _hb.timeline.length - 1; si >= 0; si--) {
        var seg = _hb.timeline[si];
        if (seg.type !== 'flatline' || !seg.blips) continue;
        for (var bi = 0; bi < seg.blips.length; bi++) {
            var blip = seg.blips[bi];
            if (!blip.txid) continue;
            if (blip.txid.toLowerCase().indexOf(needle) === 0) {
                // Found it — scroll viewport to center on this blip
                _hb.autoFollow = false;
                _hb.zoom = Math.max(_hb.zoom, 3.0); // zoom in enough to see it
                _hb.viewOffset = blip.x - (_hb.width * 0.5) / _hb.zoom;
                _hb.viewOffsetY = 0;

                // Highlight: pin the blip and set a glow timer
                _hb._pinnedBlip = blip;
                _hb._searchHighlight = { x: blip.x, gridX: blip.gridX, start: Date.now() / 1000, duration: 5.0 };

                console.log('[heartbeat] found tx', blip.txid, 'at virtualX', Math.round(blip.x));
                return true;
            }
        }
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
}

// Organism status text
export function getOrganismStatus() {
    var _hb = getState();
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
}

// ── Sound (Web Audio heartbeat) ──────────────────────────

export function heartbeatSoundToggle(enable) {
    _soundEnabled = !!enable;
    if (_soundEnabled && !_audioCtx) {
        try {
            _audioCtx = new (window.AudioContext || window.webkitAudioContext)();
        } catch(e) {
            _soundEnabled = false;
        }
    }
    return _soundEnabled;
}

export function heartbeatSoundIsEnabled() {
    return _soundEnabled;
}

// Play lub-dub heartbeat sound
export function heartbeatPlaySound() {
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

    // Copy current EKG canvas — larger share of the card
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
