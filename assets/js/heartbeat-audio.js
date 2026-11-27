// Audio cue for a newly mined block.
//
// Synthesised in Web Audio so there is no audio asset to fetch or cache-bust, and
// Web Audio keeps running in background tabs where rAF is throttled.
//
// Default character ("pulse"): a tactile organic "lub-DUB" cardiac thump routed dry
// (bypassing delay so low-end stays tight on both headphones and laptop speakers),
// blooming on the decisive second beat into a warm lower-register perfect-fifth
// resonance (D4 + A4 + soft D5 shimmer) with subtle room tail to match the dramatic
// glow of the EKG R-spike without the piercing 1–3 kHz hospital alarm pitch.

// ── Tuning ────────────────────────────────────────────────
var MASTER_GAIN = 0.58;

// S1 ("lub"): warm initial chest pulse + subtle woody transient for small speakers.
var LUB_HI_HZ = 96;
var LUB_LO_HZ = 42;
var LUB_SECS = 0.125;
var LUB_GAIN = 0.95;
var LUB_LP_HZ = 170;
var LUB_KNOCK_HI_HZ = 165;
var LUB_KNOCK_LO_HZ = 72;
var LUB_KNOCK_SECS = 0.05;
var LUB_KNOCK_GAIN = 0.32;

// S2 ("DUB"): stronger decisive systolic strike ~190ms later as the R-spike rises.
var DUB_DELAY = 0.19;
var DUB_HI_HZ = 122;
var DUB_LO_HZ = 38;
var DUB_SECS = 0.21;
var DUB_GAIN = 1.25;
var DUB_LP_HZ = 200;
var DUB_KNOCK_HI_HZ = 195;
var DUB_KNOCK_LO_HZ = 76;
var DUB_KNOCK_SECS = 0.068;
var DUB_KNOCK_GAIN = 0.42;

// Resonant spike bloom: warm D4 + A4 (perfect fifth) + soft D5 top shimmer.
// Lower register and soft attack give weight and authority without shrillness.
var CHIME_DELAY = 0.19;
var CHIME_HZ = 293.66;         // D4
var CHIME_HOLD_SECS = 0.07;
var CHIME_SECS = 1.6;
var CHIME_GAIN = 0.48;
var CHIME_FIFTH_MULT = 1.4983; // A4 (equal-tempered fifth)
var CHIME_FIFTH_DELAY = 0.015;
var CHIME_FIFTH_GAIN = 0.24;
var CHIME_OCT_MULT = 2.0;      // D5 gentle shimmer
var CHIME_OCT_GAIN = 0.09;

// Soft attack & two-stage natural resonant decay on the bloom tones.
var ATTACK_SECS = 0.018;
var PUNCH_PITCH_MULT = 1.03; // subtle organic bloom, not an alarm chirp
var PUNCH_PITCH_SECS = 0.03;
var DECAY_FAST_SECS = 0.16;
var SUSTAIN_FRAC = 0.32;

// Subtle room echo on the chime bus only (bass stays dry).
var ECHO_TIME = 0.24;
var ECHO_FEEDBACK = 0.28;
var ECHO_WET = 0.15;

// Wider than the full cue so burst catch-ups or reorgs never stack overlapping hits.
var MIN_CUE_GAP_MS = 3500;

var PRESETS = {
    // Default: organic lub-DUB heartbeat blooming into a warm D4/A4 resonant spike.
    pulse: {
        MASTER_GAIN: 0.58,
        LUB_GAIN: 0.95,
        LUB_KNOCK_GAIN: 0.32,
        DUB_DELAY: 0.19,
        DUB_GAIN: 1.25,
        DUB_KNOCK_GAIN: 0.42,
        CHIME_DELAY: 0.19,
        CHIME_HZ: 293.66,
        CHIME_SECS: 1.6,
        CHIME_GAIN: 0.48,
        CHIME_FIFTH_GAIN: 0.24,
        CHIME_OCT_GAIN: 0.09,
        ATTACK_SECS: 0.018,
        PUNCH_PITCH_MULT: 1.03,
        ECHO_WET: 0.15
    },
    // Pure tactile lub-DUB cardiac pulse with just a faint low-register sub-bloom.
    heartbeat: {
        MASTER_GAIN: 0.65,
        LUB_GAIN: 1.15,
        LUB_KNOCK_GAIN: 0.42,
        DUB_DELAY: 0.21,
        DUB_GAIN: 1.45,
        DUB_KNOCK_GAIN: 0.55,
        CHIME_DELAY: 0.21,
        CHIME_HZ: 220.0,
        CHIME_SECS: 0.9,
        CHIME_GAIN: 0.14,
        CHIME_FIFTH_GAIN: 0.05,
        CHIME_OCT_GAIN: 0.0,
        ATTACK_SECS: 0.02,
        PUNCH_PITCH_MULT: 1.01,
        ECHO_WET: 0.08
    },
    // Refined warm EKG tone: mellow 587 Hz (D5) backed by a light cardiac pulse.
    ekg: {
        MASTER_GAIN: 0.50,
        LUB_GAIN: 0.55,
        LUB_KNOCK_GAIN: 0.18,
        DUB_DELAY: 0.16,
        DUB_GAIN: 0.72,
        DUB_KNOCK_GAIN: 0.22,
        CHIME_DELAY: 0.16,
        CHIME_HZ: 587.33,
        CHIME_SECS: 1.05,
        CHIME_GAIN: 0.42,
        CHIME_FIFTH_GAIN: 0.0,
        CHIME_OCT_GAIN: 0.06,
        ATTACK_SECS: 0.012,
        PUNCH_PITCH_MULT: 1.08,
        ECHO_WET: 0.12
    }
};

var _ctx = null;
var _bus = null;   // chime + echo send -> limiter -> destination
var _dry = null;   // cardiac thump only: bypasses echo so low-end stays tight
var _delay = null;
var _delayFb = null;
var _delayWet = null;
var _lastCueAt = 0;
var _cuedHashes = new Set();

var _enabled = (function () {
    try {
        return localStorage.getItem('hb_sound') !== 'off';
    } catch (e) {
        return true;
    }
})();
var _unlockAttached = false;

export function isSoundEnabled() {
    return _enabled;
}

// Browsers suspend AudioContext until a valid user activation gesture
// (click / touchend / keydown; iOS Safari rejects pointerdown). Keep
// listeners active until `ctx.state === 'running'` so a scroll or tap on
// iOS never burns a `{ once: true }` listener without unlocking audio.
export function unlockAudioOnGesture() {
    if (_unlockAttached || !_enabled || typeof window === 'undefined' || typeof window.addEventListener !== 'function') {
        return;
    }
    _unlockAttached = true;
    var events = ['click', 'touchend', 'keydown'];
    var detach = function () {
        if (!_unlockAttached) return;
        _unlockAttached = false;
        for (var i = 0; i < events.length; i++) {
            try {
                window.removeEventListener(events[i], onGesture, { passive: true });
            } catch (e) {
                try { window.removeEventListener(events[i], onGesture, false); } catch (e2) {}
            }
        }
    };
    var onGesture = function () {
        if (!_enabled) {
            detach();
            return;
        }
        var ctx = ensureContext();
        if (!ctx) {
            detach();
            return;
        }
        if (ctx.state === 'running') {
            detach();
            return;
        }
        try {
            var p = ctx.resume();
            if (p && typeof p.then === 'function') {
                p.then(function () {
                    if (ctx.state === 'running') detach();
                }).catch(function () {});
            }
        } catch (e) {}
    };
    for (var i = 0; i < events.length; i++) {
        try {
            window.addEventListener(events[i], onGesture, { passive: true });
        } catch (e) {
            window.addEventListener(events[i], onGesture, false);
        }
    }
}

function ensureContext() {
    if (_ctx) return _ctx;
    var Ctor = (typeof window !== 'undefined') && (window.AudioContext || window.webkitAudioContext);
    if (!Ctor) return null;
    try {
        _ctx = new Ctor();
    } catch (e) {
        return null;
    }
    var comp = _ctx.createDynamicsCompressor();
    comp.threshold.value = -4;
    comp.knee.value = 8;
    comp.ratio.value = 4;
    comp.attack.value = 0.004;
    comp.release.value = 0.14;

    _bus = _ctx.createGain();
    _bus.gain.value = MASTER_GAIN;
    _bus.connect(comp);

    _dry = _ctx.createGain();
    _dry.gain.value = MASTER_GAIN;
    _dry.connect(comp);

    _delay = _ctx.createDelay(1.0);
    _delay.delayTime.value = ECHO_TIME;
    _delayFb = _ctx.createGain();
    _delayFb.gain.value = ECHO_FEEDBACK;
    _delayWet = _ctx.createGain();
    _delayWet.gain.value = ECHO_WET;

    _bus.connect(_delay);
    _delay.connect(_delayFb);
    _delayFb.connect(_delay);
    _delay.connect(_delayWet);
    _delayWet.connect(comp);

    comp.connect(_ctx.destination);
    return _ctx;
}

// Low-pass filtered pitch-dropping thump for organic cardiac S1/S2 pulses.
function thump(ctx, startAt, hiHz, loHz, durSecs, peak, lpHz) {
    if (peak <= 0 || durSecs <= 0) return;
    var osc = ctx.createOscillator();
    var filter = ctx.createBiquadFilter();
    var g = ctx.createGain();

    osc.type = 'sine';
    osc.frequency.setValueAtTime(hiHz, startAt);
    osc.frequency.exponentialRampToValueAtTime(Math.max(20, loHz), startAt + durSecs);

    filter.type = 'lowpass';
    filter.frequency.setValueAtTime(lpHz || 180, startAt);
    filter.Q.value = 0.8;

    var atk = Math.min(0.012, durSecs * 0.25);
    g.gain.setValueAtTime(0.0001, startAt);
    g.gain.exponentialRampToValueAtTime(peak, startAt + atk);
    g.gain.exponentialRampToValueAtTime(0.0001, startAt + durSecs);

    osc.connect(filter);
    filter.connect(g);
    g.connect(_dry);
    osc.start(startAt);
    osc.stop(startAt + durSecs + 0.02);
}

// Warm two-stage resonant tone for the spike bloom.
function tone(ctx, startAt, freq, holdSecs, durSecs, peak, dest) {
    if (peak <= 0 || durSecs <= 0) return;
    var osc = ctx.createOscillator();
    var g = ctx.createGain();
    osc.type = 'sine';

    if (PUNCH_PITCH_MULT > 1) {
        osc.frequency.setValueAtTime(freq * PUNCH_PITCH_MULT, startAt);
        osc.frequency.exponentialRampToValueAtTime(freq, startAt + PUNCH_PITCH_SECS);
    } else {
        osc.frequency.setValueAtTime(freq, startAt);
    }

    var sustainAt = startAt + ATTACK_SECS + holdSecs + DECAY_FAST_SECS;
    var sustainPeak = Math.max(0.0002, peak * Math.max(0.001, SUSTAIN_FRAC));
    g.gain.setValueAtTime(0.0001, startAt);
    g.gain.exponentialRampToValueAtTime(peak, startAt + ATTACK_SECS);
    g.gain.setValueAtTime(peak, startAt + ATTACK_SECS + holdSecs);
    if (sustainAt < startAt + durSecs) {
        g.gain.exponentialRampToValueAtTime(sustainPeak, sustainAt);
    }
    g.gain.exponentialRampToValueAtTime(0.0001, startAt + durSecs);

    osc.connect(g);
    g.connect(dest || _bus);
    osc.start(startAt);
    osc.stop(startAt + durSecs + 0.02);
}

function safeResume(ctx) {
    if (!ctx || ctx.state !== 'suspended' || typeof ctx.resume !== 'function') return;
    try {
        var p = ctx.resume();
        if (p && typeof p.catch === 'function') p.catch(function () {});
    } catch (e) {}
}

// Play the cue. Deduplicates per accepted block hash (capped at 64 entries) so
// duplicate block_details never double-fire while a same-height reorg replacement
// plays once. Confirmation/audition cues (`isConfirmation = true`) never stamp
// `_lastCueAt`, so a real block arriving within 3.5s is never swallowed.
export function playBlockCue(blockHash, isConfirmation) {
    if (!_enabled) return;
    // Never create an AudioContext or queue oscillators on a suspended clock
    // before the user's first activation gesture; otherwise every missed block
    // chime would fire in a simultaneous burst the moment the user clicks.
    if (!isConfirmation && (!_ctx || _ctx.state !== 'running')) return;
    var now = Date.now();
    if (!isConfirmation && now - _lastCueAt < MIN_CUE_GAP_MS) return;
    if (blockHash) {
        if (_cuedHashes.has(blockHash)) return;
        _cuedHashes.add(blockHash);
        if (_cuedHashes.size > 64) {
            var oldest = _cuedHashes.values().next().value;
            _cuedHashes.delete(oldest);
        }
    }
    if (!isConfirmation) _lastCueAt = now;
    try {
        var ctx = ensureContext();
        if (!ctx || !_bus || !_dry) return;
        safeResume(ctx);
        if (!isConfirmation && ctx.state !== 'running') return;
        var t0 = ctx.currentTime + 0.015;

        // S1: "lub"
        thump(ctx, t0, LUB_HI_HZ, LUB_LO_HZ, LUB_SECS, LUB_GAIN, LUB_LP_HZ);
        thump(ctx, t0, LUB_KNOCK_HI_HZ, LUB_KNOCK_LO_HZ, LUB_KNOCK_SECS, LUB_KNOCK_GAIN, 360);

        // S2: "DUB"
        var tDub = t0 + DUB_DELAY;
        thump(ctx, tDub, DUB_HI_HZ, DUB_LO_HZ, DUB_SECS, DUB_GAIN, DUB_LP_HZ);
        thump(ctx, tDub, DUB_KNOCK_HI_HZ, DUB_KNOCK_LO_HZ, DUB_KNOCK_SECS, DUB_KNOCK_GAIN, 420);

        // Resonant spike bloom synchronized with the DUB strike
        var tChime = t0 + CHIME_DELAY;
        tone(ctx, tChime, CHIME_HZ, CHIME_HOLD_SECS, CHIME_SECS, CHIME_GAIN, _bus);
        if (CHIME_FIFTH_GAIN > 0) {
            tone(
                ctx,
                tChime + CHIME_FIFTH_DELAY,
                CHIME_HZ * CHIME_FIFTH_MULT,
                CHIME_HOLD_SECS * 0.7,
                CHIME_SECS * 0.82,
                CHIME_FIFTH_GAIN,
                _bus
            );
        }
        if (CHIME_OCT_GAIN > 0) {
            tone(
                ctx,
                tChime + 0.008,
                CHIME_HZ * CHIME_OCT_MULT,
                CHIME_HOLD_SECS * 0.3,
                CHIME_SECS * 0.45,
                CHIME_OCT_GAIN,
                _bus
            );
        }
    } catch (e) {
        /* audio is decoration; never let it break block handling */
    }
}

export function toggleSound() {
    _enabled = !_enabled;
    try {
        localStorage.setItem('hb_sound', _enabled ? 'on' : 'off');
    } catch (e) {}
    if (_enabled) {
        var ctx = ensureContext();
        safeResume(ctx);
        playBlockCue(null, true);
    }
    return _enabled;
}

var PARAMS = {
    MASTER_GAIN: [
        function () { return MASTER_GAIN; },
        function (v) {
            MASTER_GAIN = v;
            if (_bus) _bus.gain.value = v;
            if (_dry) _dry.gain.value = v;
        }
    ],
    LUB_HI_HZ: [function () { return LUB_HI_HZ; }, function (v) { LUB_HI_HZ = v; }],
    LUB_LO_HZ: [function () { return LUB_LO_HZ; }, function (v) { LUB_LO_HZ = v; }],
    LUB_SECS: [function () { return LUB_SECS; }, function (v) { LUB_SECS = v; }],
    LUB_GAIN: [function () { return LUB_GAIN; }, function (v) { LUB_GAIN = v; }],
    LUB_LP_HZ: [function () { return LUB_LP_HZ; }, function (v) { LUB_LP_HZ = v; }],
    LUB_KNOCK_HI_HZ: [function () { return LUB_KNOCK_HI_HZ; }, function (v) { LUB_KNOCK_HI_HZ = v; }],
    LUB_KNOCK_LO_HZ: [function () { return LUB_KNOCK_LO_HZ; }, function (v) { LUB_KNOCK_LO_HZ = v; }],
    LUB_KNOCK_SECS: [function () { return LUB_KNOCK_SECS; }, function (v) { LUB_KNOCK_SECS = v; }],
    LUB_KNOCK_GAIN: [function () { return LUB_KNOCK_GAIN; }, function (v) { LUB_KNOCK_GAIN = v; }],
    DUB_DELAY: [function () { return DUB_DELAY; }, function (v) { DUB_DELAY = v; }],
    DUB_HI_HZ: [function () { return DUB_HI_HZ; }, function (v) { DUB_HI_HZ = v; }],
    DUB_LO_HZ: [function () { return DUB_LO_HZ; }, function (v) { DUB_LO_HZ = v; }],
    DUB_SECS: [function () { return DUB_SECS; }, function (v) { DUB_SECS = v; }],
    DUB_GAIN: [function () { return DUB_GAIN; }, function (v) { DUB_GAIN = v; }],
    DUB_LP_HZ: [function () { return DUB_LP_HZ; }, function (v) { DUB_LP_HZ = v; }],
    DUB_KNOCK_HI_HZ: [function () { return DUB_KNOCK_HI_HZ; }, function (v) { DUB_KNOCK_HI_HZ = v; }],
    DUB_KNOCK_LO_HZ: [function () { return DUB_KNOCK_LO_HZ; }, function (v) { DUB_KNOCK_LO_HZ = v; }],
    DUB_KNOCK_SECS: [function () { return DUB_KNOCK_SECS; }, function (v) { DUB_KNOCK_SECS = v; }],
    DUB_KNOCK_GAIN: [function () { return DUB_KNOCK_GAIN; }, function (v) { DUB_KNOCK_GAIN = v; }],
    CHIME_DELAY: [function () { return CHIME_DELAY; }, function (v) { CHIME_DELAY = v; }],
    CHIME_HZ: [function () { return CHIME_HZ; }, function (v) { CHIME_HZ = v; }],
    CHIME_HOLD_SECS: [function () { return CHIME_HOLD_SECS; }, function (v) { CHIME_HOLD_SECS = v; }],
    CHIME_SECS: [function () { return CHIME_SECS; }, function (v) { CHIME_SECS = v; }],
    CHIME_GAIN: [function () { return CHIME_GAIN; }, function (v) { CHIME_GAIN = v; }],
    CHIME_FIFTH_MULT: [function () { return CHIME_FIFTH_MULT; }, function (v) { CHIME_FIFTH_MULT = v; }],
    CHIME_FIFTH_DELAY: [function () { return CHIME_FIFTH_DELAY; }, function (v) { CHIME_FIFTH_DELAY = v; }],
    CHIME_FIFTH_GAIN: [function () { return CHIME_FIFTH_GAIN; }, function (v) { CHIME_FIFTH_GAIN = v; }],
    CHIME_OCT_MULT: [function () { return CHIME_OCT_MULT; }, function (v) { CHIME_OCT_MULT = v; }],
    CHIME_OCT_GAIN: [function () { return CHIME_OCT_GAIN; }, function (v) { CHIME_OCT_GAIN = v; }],
    ATTACK_SECS: [function () { return ATTACK_SECS; }, function (v) { ATTACK_SECS = v; }],
    PUNCH_PITCH_MULT: [function () { return PUNCH_PITCH_MULT; }, function (v) { PUNCH_PITCH_MULT = v; }],
    PUNCH_PITCH_SECS: [function () { return PUNCH_PITCH_SECS; }, function (v) { PUNCH_PITCH_SECS = v; }],
    DECAY_FAST_SECS: [function () { return DECAY_FAST_SECS; }, function (v) { DECAY_FAST_SECS = v; }],
    SUSTAIN_FRAC: [function () { return SUSTAIN_FRAC; }, function (v) { SUSTAIN_FRAC = v; }],
    ECHO_TIME: [
        function () { return ECHO_TIME; },
        function (v) { ECHO_TIME = v; if (_delay) _delay.delayTime.value = v; }
    ],
    ECHO_FEEDBACK: [
        function () { return ECHO_FEEDBACK; },
        function (v) { ECHO_FEEDBACK = v; if (_delayFb) _delayFb.gain.value = v; }
    ],
    ECHO_WET: [
        function () { return ECHO_WET; },
        function (v) { ECHO_WET = v; if (_delayWet) _delayWet.gain.value = v; }
    ],
    MIN_CUE_GAP_MS: [function () { return MIN_CUE_GAP_MS; }, function (v) { MIN_CUE_GAP_MS = v; }]
};

var DEFAULTS = (function () {
    var d = {};
    Object.keys(PARAMS).forEach(function (k) { d[k] = PARAMS[k][0](); });
    return d;
})();

export function soundSettings() {
    var out = {};
    var changed = {};
    Object.keys(PARAMS).forEach(function (k) {
        var v = PARAMS[k][0]();
        out[k] = v;
        if (v !== DEFAULTS[k]) changed[k] = v;
    });
    return { values: out, changed: changed };
}

export function tuneSound(overrides) {
    var applied = {};
    var unknown = [];
    Object.keys(overrides || {}).forEach(function (k) {
        if (PARAMS[k]) {
            PARAMS[k][1](overrides[k]);
            applied[k] = overrides[k];
        } else {
            unknown.push(k);
        }
    });
    return { applied: applied, unknown: unknown };
}

// Audition built-in sound presets ('pulse' | 'heartbeat' | 'ekg') live in the console.
export function setSoundPreset(name) {
    if (typeof name !== 'string' || !Object.prototype.hasOwnProperty.call(PRESETS, name)) {
        return { ok: false, available: Object.keys(PRESETS) };
    }
    var p = PRESETS[name];
    tuneSound(DEFAULTS);
    tuneSound(p);
    var prevEnabled = _enabled;
    _enabled = true;
    playBlockCue(null, true);
    _enabled = prevEnabled;
    return { ok: true, preset: name };
}
