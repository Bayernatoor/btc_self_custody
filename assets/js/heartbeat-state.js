// heartbeat-state.js: Shared constants and state accessor for the Block Heartbeat engine.

// ── Color palette ──────────────────────────────────────────
export var COLORS = {
    healthy:  '#00e676',
    steady:   '#42a5f5',
    elevated: '#f7931a',
    stressed: '#ff5722',
    critical: '#f44336'
};

export var BG_COLOR = 'rgba(13, 33, 55, 1)'; // matches bg-[#0d2137]
export var GRID_COLOR = 'rgba(255, 255, 255, 0.03)';

// ── Thresholds & dimensions ───────────────────────────────
export var POINT_WIDTH = 1.5;          // virtual pixels per PQRST point
export var FLATLINE_PX_PER_SEC = 5.0;  // live flatline advance rate
export var HEAD_POSITION_FRAC = 0.65;  // viewport fraction for live head (comfortable center-right framing)
export var MAX_FLATLINE_WIDTH = 1600;  // max compressed flatline px (90m+ soft cap)
export var RETARGET_PERIOD = 2016;     // blocks per difficulty epoch
export var RHYTHM_STRIP_BLOCKS = 144;  // blocks in 24hr strip
export var MAX_BLIPS_PER_SEGMENT = 5000; // prune threshold (off-screen only)

// ── Shared state ──────────────────────────────────────────
let _hb = null;
export function getState() { return _hb; }
export function setState(val) { _hb = val; }
