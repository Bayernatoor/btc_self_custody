// heartbeat-timeline.js — PQRST waveform generation, color computation, grid drawing,
// and timeline segment construction for the Block Heartbeat engine.

import { getState, COLORS, GRID_COLOR, POINT_WIDTH, MAX_FLATLINE_WIDTH } from './heartbeat-state.js';

// ── PQRST waveform generation ──────────────────────────────
// Converts block data into an array of y-offset points (from baseline).
// Negative y = upward on canvas (screen coords).
export function generatePQRST(block) {
    var points = [];
    var txCount = block.tx_count || 1;
    var fees = block.total_fees || 0;
    var interBlock = block.inter_block_seconds || 600;
    var weight = block.weight || 0;

    // Normalize inputs (0-1 range)
    var txNorm = Math.min(txCount / 6000, 1.0);
    var waitNorm = Math.min(interBlock / 1800, 1.0); // 30 min cap
    var fullNorm = Math.min(weight / 4000000, 1.0);

    // Fee normalization: shifted log scale so typical fee range (5M-50M sats)
    // spans the middle of the visual range. log10(5M)=6.7, log10(50M)=7.7
    // Subtract 5.5 to make low fees small and high fees tall:
    //   1M sats → 0.17, 5M → 0.40, 20M → 0.63, 50M → 0.73, 200M → 0.90, 1B → 1.0
    var feeNorm = fees > 0 ? Math.min(Math.max((Math.log10(fees) - 5.5) / 3.5, 0.05), 1.0) : 0;

    // Waveform amplitudes (pixels)
    var pAmp   = 6 + txNorm * 28;           // P wave: 6-34px
    var qDepth = 8 + waitNorm * 60;          // Q dip: 8-68px
    var rHeight = 20 + feeNorm * 280;         // R spike: 20-300px
    var sDepth = rHeight * 0.45;              // S dip: 45% of R
    var tAmp   = 4 + fullNorm * 30;           // T wave: 4-34px

    // Baseline flat lead-in (2-4 points)
    points.push(0, 0, 0);

    // P wave (small upward bump)
    var pSteps = 8;
    for (var i = 0; i <= pSteps; i++) {
        var t = i / pSteps;
        points.push(-pAmp * Math.sin(t * Math.PI));
    }

    // Brief return to baseline
    points.push(0, 0);

    // Q dip (downward)
    var qSteps = 4;
    for (var i = 1; i <= qSteps; i++) {
        points.push(qDepth * (i / qSteps));
    }

    // R spike (sharp upward)
    var rSteps = 6;
    for (var i = 0; i <= rSteps; i++) {
        var t = i / rSteps;
        // Q bottom to R top
        var val = qDepth - (qDepth + rHeight) * t;
        points.push(val);
    }

    // S dip (sharp downward from R peak)
    var sSteps = 5;
    for (var i = 1; i <= sSteps; i++) {
        var t = i / sSteps;
        points.push(-rHeight + (rHeight + sDepth) * t);
    }

    // Return from S to baseline
    var retSteps = 4;
    for (var i = 1; i <= retSteps; i++) {
        var t = i / retSteps;
        points.push(sDepth * (1 - t));
    }

    // Brief baseline
    points.push(0, 0);

    // T wave (gentle upward bump)
    var tSteps = 10;
    for (var i = 0; i <= tSteps; i++) {
        var t = i / tSteps;
        points.push(-tAmp * Math.sin(t * Math.PI));
    }

    // Trailing baseline
    points.push(0, 0, 0, 0);

    return points;
}

// ── Color computation ──────────────────────────────────────
export function computeColor(elapsedSec, feeRate, mempoolMB) {
    // Time: normalized over 20min (not 30). A 20min wait = full time stress.
    // Weight 50% so long waits dominate the color even with low fees.
    var timeStress = Math.min(elapsedSec / 1200, 1.0);
    var feeStress = Math.min(feeRate / 150, 1.0);
    var mempoolStress = Math.min(mempoolMB / 200, 1.0);

    var stress = timeStress * 0.5 + feeStress * 0.3 + mempoolStress * 0.2;

    if (stress < 0.2)  return COLORS.healthy;
    if (stress < 0.4)  return COLORS.steady;
    if (stress < 0.6)  return COLORS.elevated;
    if (stress < 0.8)  return COLORS.stressed;
    return COLORS.critical;
}

export function lerp(a, b, t) { return a + (b - a) * t; }

export function hexToRgb(hex) {
    if (!hex || typeof hex !== 'string') return [0, 150, 255]; // fallback: blue
    var r = parseInt(hex.slice(1, 3), 16);
    var g = parseInt(hex.slice(3, 5), 16);
    var b = parseInt(hex.slice(5, 7), 16);
    return [r, g, b];
}

export function lerpColor(a, b, t) {
    var ar = hexToRgb(a || '#0096ff'), br = hexToRgb(b || '#0096ff');
    var r = Math.round(ar[0] + (br[0] - ar[0]) * t);
    var g = Math.round(ar[1] + (br[1] - ar[1]) * t);
    var bl = Math.round(ar[2] + (br[2] - ar[2]) * t);
    return 'rgb(' + r + ',' + g + ',' + bl + ')';
}

// Fee rate to RGBA color prefix (close with opacity + ')')
export function feeRateColor(feeRate, medianFee) {
    if (feeRate < medianFee * 0.8) return 'rgba(33, 150, 243, ';
    if (feeRate < medianFee * 1.5) return 'rgba(0, 230, 118, ';
    if (feeRate < medianFee * 4)   return 'rgba(255, 152, 0, ';
    return 'rgba(244, 67, 54, ';
}

// Bloodstream cell radius based on transaction vsize
export function cellRadiusForVsize(vs) {
    if (vs < 200) return 2.0;
    if (vs < 400) return 3.0;
    if (vs < 800) return 4.0;
    if (vs < 2000) return 5.0;
    return 6.0;
}

// Compute flatline width in virtual pixels for a completed (history) flatline
export function historyFlatlineWidth(interBlockSeconds) {
    // Proportional: 1min=15px, 5min=75px, 10min=150px, 30min=300px (capped)
    return Math.max(10, Math.min(interBlockSeconds / 4, MAX_FLATLINE_WIDTH));
}

// ── Grid drawing ───────────────────────────────────────────
export function drawGrid(ctx, w, h) {
    ctx.beginPath();
    ctx.strokeStyle = GRID_COLOR;
    ctx.lineWidth = 0.5;
    var spacing = 40;
    for (var x = 0; x < w; x += spacing) {
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
    }
    for (var y = 0; y < h; y += spacing) {
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
    }
    ctx.stroke();
}

// ── Timeline segment helpers ───────────────────────────────

// Create a block segment from block data
export function createBlockSegment(block, xStart) {
    var _hb = getState();
    var points = generatePQRST(block);
    var width = points.length * POINT_WIDTH;
    var interBlock = block.inter_block_seconds || 600;
    var feeRate = block.total_fees ? block.total_fees / 100000 : 0;
    var color = computeColor(interBlock, feeRate, _hb ? _hb.mempoolMB : 0);

    return {
        type: 'block',
        height: block.height || 0,
        timestamp: block.timestamp || 0,
        tx_count: block.tx_count || 0,
        total_fees: block.total_fees || 0,
        weight: block.weight || 0,
        inter_block_seconds: interBlock,
        points: points,
        x_start: xStart,
        x_end: xStart + width,
        color: color
    };
}

// Create a flatline segment
export function createFlatlineSegment(xStart, xEnd) {
    return {
        type: 'flatline',
        x_start: xStart,
        x_end: xEnd,   // null for live (open-ended)
        color: null,    // set when closed (block arrives). null = use live color
        blips: []
    };
}
