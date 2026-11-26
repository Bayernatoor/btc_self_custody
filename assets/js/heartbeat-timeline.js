// heartbeat-timeline.js: PQRST waveform generation, color computation, grid drawing,
// and timeline segment construction for the Block Heartbeat engine.

import { getState, COLORS, GRID_COLOR, POINT_WIDTH, MAX_FLATLINE_WIDTH } from './heartbeat-state.js';

// ── Exact inter-block interval helper ──────────────────────
// Preserves 0s and negative MTP header deltas; returns null when unknown/missing.
export function resolveInterBlockSeconds(block) {
    if (!block || typeof block !== 'object') return null;
    var v = block.inter_block_seconds;
    return (typeof v === 'number' && isFinite(v)) ? v : null;
}

// ── PQRST waveform generation ──────────────────────────────
// Converts block data into an array of y-offset points (from baseline).
// Negative y = upward on canvas (screen coords).
export function generatePQRST(block) {
    var points = [];
    var txCount = block.tx_count || 1;
    var fees = block.total_fees || 0;
    var rawInter = resolveInterBlockSeconds(block);
    var waitNorm = rawInter === null ? 0 : Math.min(Math.max(0, rawInter) / 1800, 1.0); // 30 min cap
    var weight = block.weight || 0;

    // Normalize inputs (0-1 range)
    var txNorm = Math.min(txCount / 6000, 1.0);
    var fullNorm = Math.min(weight / 4000000, 1.0);

    // Fee normalization: shifted log scale so typical fee range (5M-50M sats)
    // spans the middle of the visual range. log10(5M)=6.7, log10(50M)=7.7
    // Subtract 5.5 to make low fees small and high fees tall:
    //   1M sats → 0.17, 5M → 0.40, 20M → 0.63, 50M → 0.73, 200M → 0.90, 1B → 1.0
    var feeNorm = fees > 0 ? Math.min(Math.max((Math.log10(fees) - 5.5) / 3.5, 0.05), 1.0) : 0;

    // Waveform amplitudes (pixels)
    var pAmp   = 6 + txNorm * 28;                               // P wave: 6-34px
    var qDepth = rawInter === null ? 4 : (8 + waitNorm * 60);   // Q dip: 4px unknown, 8-68px measured
    var rHeight = 20 + feeNorm * 280;                           // R spike: 20-300px
    var sDepth = rHeight * 0.45;                                // S dip: 45% of R
    var size = block.size || 0;
    // Normal full blocks (<= 2.2 MB raw bytes) stay strictly in 4-34px based on
    // whole-block weight; witness-heavy mega-blocks (> 2.2 MB up to 4.0 MB, like
    // #836,964 at 3.99 MB) swell the T-wave up to 78px so they stand out visually
    // without distorting any normal block's proportions.
    var megaSizeBoost = size > 2200000
        ? Math.min((size - 2200000) / 1800000, 1.0) * 44
        : 0;
    var tAmp   = 4 + fullNorm * 30 + megaSizeBoost;             // T wave: 4-34px normal, up to 78px mega-block

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

// ── Progressive 7-stop wait color ramp ─────────────────────
// Continuous RGB interpolation across unmistakable milestones so 30m, 45m, 60m,
// and 90m+ droughts remain visually distinct.
var WAIT_COLOR_STOPS = [
    { sec: 300,  hex: '#00e676', rgb: [0, 230, 118] },    // 0-5m: emerald green
    { sec: 600,  hex: '#38bdf8', rgb: [56, 189, 248] },   // 10m:  sky-cyan
    { sec: 1200, hex: '#facc15', rgb: [250, 204, 21] },   // 20m:  warm gold
    { sec: 1800, hex: '#f97316', rgb: [249, 115, 22] },   // 30m:  vivid amber-orange
    { sec: 2700, hex: '#ff4500', rgb: [255, 69, 0] },     // 45m:  hot red-orange
    { sec: 3600, hex: '#dc2626', rgb: [220, 38, 38] },    // 60m:  deep crimson
    { sec: 5400, hex: '#c026d3', rgb: [192, 38, 211] }    // 90m+: intense magenta-violet
];

function rgbToHex(r, g, b) {
    var cr = Math.max(0, Math.min(255, Math.round(r)));
    var cg = Math.max(0, Math.min(255, Math.round(g)));
    var cb = Math.max(0, Math.min(255, Math.round(b)));
    return '#'
        + (cr < 16 ? '0' : '') + cr.toString(16)
        + (cg < 16 ? '0' : '') + cg.toString(16)
        + (cb < 16 ? '0' : '') + cb.toString(16);
}

// Continuous 7-stop wait color ramp returning a 7-char #rrggbb hex string.
export function waitColor(waitSec) {
    if (waitSec == null || !isFinite(waitSec) || waitSec <= WAIT_COLOR_STOPS[0].sec) {
        return WAIT_COLOR_STOPS[0].hex;
    }
    var last = WAIT_COLOR_STOPS[WAIT_COLOR_STOPS.length - 1];
    if (waitSec >= last.sec) {
        return last.hex;
    }
    for (var i = 1; i < WAIT_COLOR_STOPS.length; i++) {
        var hi = WAIT_COLOR_STOPS[i];
        if (waitSec <= hi.sec) {
            var lo = WAIT_COLOR_STOPS[i - 1];
            var t = (waitSec - lo.sec) / (hi.sec - lo.sec);
            return rgbToHex(
                lo.rgb[0] + (hi.rgb[0] - lo.rgb[0]) * t,
                lo.rgb[1] + (hi.rgb[1] - lo.rgb[1]) * t,
                lo.rgb[2] + (hi.rgb[2] - lo.rgb[2]) * t
            );
        }
    }
    return last.hex;
}

// Horizontal linear gradient stepping smoothly through every wait milestone
// from 0s (#00e676) up to waitSec (waitColor(waitSec)). Falls back to a solid
// hex color if the span is narrow or createLinearGradient is unavailable.
export function buildFlatlineGradient(ctx, x1, x2, waitSec) {
    var w = (typeof waitSec === 'number' && isFinite(waitSec)) ? Math.max(0, waitSec) : 0;
    var endColor = waitColor(w);
    if (!ctx || typeof ctx.createLinearGradient !== 'function'
        || !isFinite(x1) || !isFinite(x2) || (x2 - x1) < 4
        || w <= WAIT_COLOR_STOPS[0].sec) {
        return endColor;
    }
    var grad = ctx.createLinearGradient(x1, 0, x2, 0);
    grad.addColorStop(0, WAIT_COLOR_STOPS[0].hex);
    for (var i = 0; i < WAIT_COLOR_STOPS.length; i++) {
        var stopSec = WAIT_COLOR_STOPS[i].sec;
        if (stopSec > 0 && stopSec < w) {
            var pos = Math.max(0.001, Math.min(0.999, stopSec / w));
            grad.addColorStop(pos, WAIT_COLOR_STOPS[i].hex);
        }
    }
    grad.addColorStop(1, endColor);
    return grad;
}

// ── Color computation ──────────────────────────────────────
// Time alone freely drives the full 0m -> 90m+ ramp; high fees (>= 30 sat/vB)
// add a modest stress boost (up to +600s at 150+ sat/vB).
export function computeColor(elapsedSec, feeRate, mempoolMB) {
    var baseSec = (typeof elapsedSec === 'number' && isFinite(elapsedSec))
        ? Math.max(0, elapsedSec) : 0;
    var fee = (typeof feeRate === 'number' && isFinite(feeRate)) ? feeRate : 0;
    var feeBoostSec = fee >= 30 ? Math.min(600, ((fee - 30) / 120) * 600) : 0;
    return waitColor(baseSec + feeBoostSec);
}

export function lerp(a, b, t) { return a + (b - a) * t; }

export function hexToRgb(hex) {
    if (!hex || typeof hex !== 'string') return [0, 150, 255]; // fallback: blue
    if (hex.charAt(0) === '#') {
        var r = parseInt(hex.slice(1, 3), 16);
        var g = parseInt(hex.slice(3, 5), 16);
        var b = parseInt(hex.slice(5, 7), 16);
        if (isFinite(r) && isFinite(g) && isFinite(b)) return [r, g, b];
    }
    var m = hex.match(/(\d+)\D+(\d+)\D+(\d+)/);
    if (m) return [parseInt(m[1], 10), parseInt(m[2], 10), parseInt(m[3], 10)];
    return [0, 150, 255];
}

export function lerpColor(a, b, t) {
    var ar = hexToRgb(a || '#0096ff'), br = hexToRgb(b || '#0096ff');
    return rgbToHex(
        ar[0] + (br[0] - ar[0]) * t,
        ar[1] + (br[1] - ar[1]) * t,
        ar[2] + (br[2] - ar[2]) * t
    );
}

// Neutral slate color prefix for bricks whose fee metadata is still pending
export var PENDING_FEE_COLOR = 'rgba(100, 116, 139, ';

// Neutral slate hex for blocks and flatlines whose inter-block interval is unknown
export var UNKNOWN_INTERVAL_COLOR = '#64748b';

// Fixed 6-band + sub-1 + pending fee rate to RGBA color prefix (close with opacity + ')')
export function feeRateColor(feeRate) {
    if (feeRate == null || !isFinite(feeRate)) return PENDING_FEE_COLOR;
    if (feeRate < 1)  return 'rgba(37, 99, 235, ';    // #2563eb cobalt blue (< 1 sat/vB)
    if (feeRate < 2)  return 'rgba(56, 189, 248, ';   // #38bdf8 sky cyan (1-2 sat/vB)
    if (feeRate < 5)  return 'rgba(0, 230, 118, ';     // #00e676 green (2-5 sat/vB)
    if (feeRate < 15) return 'rgba(247, 147, 26, ';   // #f7931a amber (5-15 sat/vB)
    if (feeRate < 50) return 'rgba(255, 87, 34, ';    // #ff5722 orange-red (15-50 sat/vB)
    return 'rgba(244, 67, 54, ';                      // #f44336 crimson red (50+ sat/vB)
}

// Fixed 6-band + sub-1 + pending fee rate index (0..=6), matching Rust's
// `heartbeat_sync::fee_band_index` so legend spotlighting agrees 1:1.
export function feeBandIndex(feeRate, feePending) {
    if (feePending || feeRate == null || !isFinite(feeRate)) return 0;
    if (feeRate < 1)  return 1;
    if (feeRate < 2)  return 2;
    if (feeRate < 5)  return 3;
    if (feeRate < 15) return 4;
    if (feeRate < 50) return 5;
    return 6;
}

// Human-readable label for the fixed fee band
export function feeBandLabel(feeRate) {
    if (feeRate == null || !isFinite(feeRate)) return 'Fee pending';
    if (feeRate < 1)  return '< 1 sat/vB';
    if (feeRate < 2)  return '1-2 sat/vB';
    if (feeRate < 5)  return '2-5 sat/vB';
    if (feeRate < 15) return '5-15 sat/vB';
    if (feeRate < 50) return '15-50 sat/vB';
    return '50+ sat/vB';
}

// Format fee rate preserving sub-1 precision (2dp for 0 < r < 1 sat/vB, 1dp for 0 or >= 1)
export function formatFeeRate(feeRate) {
    if (feeRate == null || !isFinite(feeRate)) return 'pending';
    if (feeRate === 0) return '0.0';
    if (feeRate > 0 && feeRate < 1) return feeRate.toFixed(2);
    return feeRate.toFixed(1);
}

// Shared vertical magnification above 4x zoom so bricks read as crisp proportional
// tiles instead of flat strips, used identically by rendering, 2D hit-testing,
// vertical pan bounds, and Locate Brick reticles.
export function brickHeightScale(zoom) {
    return zoom > 4 ? 1 + (zoom - 4) * 0.38 : 1;
}

// Bloodstream cell radius based on transaction vsize
export function cellRadiusForVsize(vs) {
    if (vs < 200) return 2.0;
    if (vs < 400) return 3.0;
    if (vs < 800) return 4.0;
    if (vs < 2000) return 5.0;
    return 6.0;
}

// Progressive flatline width in virtual pixels for a completed (history/sealed) flatline.
// Unknown/null uses a distinct neutral 60px width (never the 100px 10m measured width).
export function historyFlatlineWidth(interBlockSeconds) {
    if (interBlockSeconds == null || !isFinite(interBlockSeconds)) return 60;
    var m = Math.max(0, interBlockSeconds) / 60;
    if (m <= 10) return 10 + m * 9;
    if (m <= 20) return 100 + (m - 10) * 12;
    if (m <= 30) return 220 + (m - 20) * 16;
    if (m <= 45) return 380 + (m - 30) * 19.5;
    if (m <= 60) return 672.5 + (m - 45) * 23;
    return Math.min(MAX_FLATLINE_WIDTH, 1017.5 + (m - 60) * 16);
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

// Resolve the wall-clock observation time for a block when sizing live wait
// elapsed time or flatlines: prefers the node's own `observed_at` over the
// miner's header `timestamp` (which consensus allows up to +2h in the future).
export function effectiveBlockWallTime(block, nowSec) {
    if (!block) return 0;
    if (block.observed_at && block.observed_at > 0) return block.observed_at;
    if (block.timestamp && block.timestamp > 0) {
        var cap = (typeof nowSec === 'number' && isFinite(nowSec) && nowSec > 0)
            ? nowSec
            : Math.floor(Date.now() / 1000);
        return Math.min(block.timestamp, cap);
    }
    return 0;
}

// Create a block segment from block data
export function createBlockSegment(block, xStart) {
    var _hb = getState();
    var points = generatePQRST(block);
    var width = points.length * POINT_WIDTH;
    var rawInter = resolveInterBlockSeconds(block);
    var blockVbytes = block.weight > 0 ? (block.weight / 4) : (block.size > 0 ? block.size : 1000000);
    var feeRate = block.total_fees ? (block.total_fees / blockVbytes) : 0;
    var color = rawInter === null
        ? UNKNOWN_INTERVAL_COLOR
        : computeColor(Math.max(0, rawInter), feeRate, _hb ? _hb.mempoolMB : 0);

    // Pre-compute simplified peak/trough summary once for zoomed-out LOD rendering
    var minY = 0, minYIdx = 0;
    var maxY = 0, maxYIdx = 0;
    for (var pi = 0; pi < points.length; pi++) {
        var py = points[pi];
        if (py < minY) { minY = py; minYIdx = pi; }
        if (py > maxY) { maxY = py; maxYIdx = pi; }
    }

    return {
        type: 'block',
        hash: block.hash || null,
        height: block.height || 0,
        timestamp: block.timestamp || 0,
        observed_at: block.observed_at || 0,
        tx_count: block.tx_count || 0,
        total_fees: block.total_fees || 0,
        size: block.size || 0,
        weight: block.weight || 0,
        miner: block.miner || null,
        inter_block_seconds: rawInter,
        points: points,
        _minY: minY,
        _minYIdx: minYIdx,
        _maxY: maxY,
        _maxYIdx: maxYIdx,
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
        inter_block_seconds: null,
        blips: [],
        _blipsByTxid: new Map(),
        _colHeights: {},
        _colBlips: new Map()
    };
}

// ── Shared formatting helpers ──────────────────────────────

// Format BTC preserving precision: avoids 99.99998 showing as "100.0000"
// (a misleading round number). Shared by the canvas tooltips and the feed.
export function fmtBtc(btc) {
    if (btc >= 100) {
        // 6 decimals with trailing-zero trim (min 2 decimals shown)
        var s = btc.toFixed(6).replace(/0+$/, '');
        if (s.endsWith('.')) s += '00';
        var dot = s.indexOf('.');
        if (dot >= 0 && s.length - dot - 1 < 2) s = btc.toFixed(2);
        return s;
    }
    if (btc >= 1) return btc.toFixed(4);
    if (btc >= 0.01) return btc.toFixed(6);
    return btc.toFixed(8);
}

// Human-readable duration from seconds (e.g. "0s", "-12s (MTP)", "9m 42s", "1h 3m", "unknown").
export function formatDuration(sec) {
    if (sec == null || !isFinite(sec)) return 'unknown';
    if (sec < 0) return '-' + Math.abs(Math.round(sec)) + 's (MTP)';
    var rounded = Math.round(sec);
    if (rounded === 0) return '0s';
    if (rounded < 60) return rounded + 's';
    var m = Math.floor(rounded / 60);
    var s = rounded % 60;
    if (m < 60) return m + 'm ' + s + 's';
    var h = Math.floor(m / 60);
    m = m % 60;
    return h + 'h ' + m + 'm';
}
