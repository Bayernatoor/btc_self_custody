// Audio cue for a newly mined block.
//
// The page is a cardiac monitor, so the cue is the sound that metaphor implies: a
// monitor's beep on a QRS complex, then a lower tone as the beat settles. Two notes
// descending read as "completed"; a single beep reads as "alert".
//
// Weight comes from the envelope, not from bass. A downward pitch punch on the onset and
// a two-stage decay give the cue impact and a long ring; a low layer under it only masks
// the leading edge, which is the part carrying the meaning. A feedback delay mixed low
// adds a suggestion of room on top.
//
// Two layers ship disabled and are kept only as knobs, both because they made the cue
// sound synthetic: BODY_GAIN (a long sub-bass fall, recognisable as a stock sound effect)
// and VIBRATO_DEPTH_FRAC (full-depth modulation from the first sample, which nothing
// physical does). Neither costs anything at 0.
//
// Synthesised, so there is no asset to fetch and nothing for sw.js to cache-bust, and
// the character is tunable by the constants below the way REVEAL_* tunes the animation.
//
// The cue is a fixed figure rather than being scored to the reveal animation. Hidden
// tabs are the main reason to want audio, and rAF is throttled there so the reveal is
// not running: anything keyed to its beats would be silent exactly when it matters.
// Web Audio keeps running in a background tab, so a self-contained cue fires either way.

// ── Tuning ────────────────────────────────────────────────
// Overall loudness, and the first knob to reach for.
var MASTER_GAIN = 0.45;

// The beep. A quiet harmonic an octave up is what makes it read as crisp rather than
// muffled; the fundamental alone sounds further away at the same measured level.
//
// Held at peak before decaying, rather than decaying from the instant it starts. The
// spike on screen forms over REVEAL_FORM_SECS (2.5s); a 100ms blip announced an event
// far shorter than the one being drawn. The hold is what carries that intensity, while
// the fast attack keeps it a beep rather than a swell.
var BEEP_HZ = 1046;
var BEEP_HOLD_SECS = 0.13;
var BEEP_SECS = 1.15;
var BEEP_GAIN = 1.5;
var BEEP_HARMONIC_MULT = 2; // octave
var BEEP_HARMONIC_GAIN = 0.2;

// The beat settling. Lower and softer, so the pair falls.
var SETTLE_HZ = 660;
var SETTLE_HOLD_SECS = 0.1;
var SETTLE_SECS = 0.5;
var SETTLE_DELAY = 0.38; // after the beep starts; far enough back to read as two notes
var SETTLE_GAIN = 0.45;

// Body. Off. A long sine falling from 300Hz to 50Hz is the stock-library "drop" gesture,
// and the ear recognises it as a sound effect rather than an instrument even at very low
// gain. Kept as a knob, but it costs nothing at 0: the layer is guarded and no oscillator
// is created.
var BODY_HI_HZ = 300;
var BODY_LO_HZ = 50;
var BODY_DELAY = 0;
var BODY_HOLD_SECS = 0.05;
var BODY_SECS = 3;
var BODY_GAIN = 0;

// Echo. A feedback delay, mixed low. At ECHO_WET 0.1 the repeats are not heard as echoes
// at all, only as a suggestion of room around the tone; pushed past ~0.3 they separate
// into audible taps and the cue starts to sound like a delay pedal. Feedback must stay
// below 1.0 or the loop never decays.
var ECHO_TIME = 0.3;     // gap between repeats
var ECHO_FEEDBACK = 0.25; // how much of each repeat feeds the next
var ECHO_WET = 0.1;       // how much delayed signal is mixed back in

// Vibration. A slow pitch wobble on the held tones. Depth is a fraction of each tone's
// own frequency so the wobble is musically even across the beep and the settle rather
// than being deep on one and imperceptible on the other. Keep it small: past ~0.015 it
// stops sounding like life in the tone and starts sounding broken.
var VIBRATO_HZ = 6;
// Off. Instant full-depth modulation from the first sample is a machine artifact with
// no acoustic equivalent, and it was a large part of why the cue sounded synthetic.
var VIBRATO_DEPTH_FRAC = 0;

// Punch. Two things give a tone impact at the moment it starts.
//
// A pitch envelope: the tone begins above its target and falls to it within a few tens of
// milliseconds. Every struck thing does this, drums most obviously, and the ear reads that
// fast downward glide as force. It costs no low end, which is why it delivers a punch the
// bass sweep could not.
//
// And a short attack. 10ms, tuned by ear: faster reads as a click on the front of the
// tone, slower starts to feel like a swell rather than a hit.
var ATTACK_SECS = 0.01;
var PUNCH_PITCH_MULT = 1.55; // start this far above the target pitch
var PUNCH_PITCH_SECS = 0.022; // and reach it this quickly

// Tail. A single exponential from peak to silence is the sound of a synthesiser. Real
// resonant bodies drop fast at first as the strike energy dissipates, then ring on far
// longer and much quieter. Two stages: a fast fall to SUSTAIN_FRAC of peak, then a long
// decay from there. That is what makes a sound punch and then drag out rather than simply
// being loud and then being over.
var DECAY_FAST_SECS = 0.085;
var SUSTAIN_FRAC = 0.26;

// Wider than the full cue. Below that a burst (catch-up, reorg) would start a second cue
// over the ring of the first.
var MIN_CUE_GAP_MS = 3500;

var _ctx = null;
var _bus = null;   // tones + echo send -> limiter -> destination
var _dry = null;   // body only: bypasses the echo, since delayed bass smears to mud
var _delay = null; // echo send, built once: a per-cue delay would stack feedback loops
var _delayFb = null;
var _delayWet = null;
var _lastCueAt = 0;

// Off unless explicitly enabled. Unannounced audio is hostile, and browsers block it
// before a user gesture anyway, so the toggle click is what makes it possible.
var _enabled = (function () {
    try {
        return localStorage.getItem('hb_sound') === 'on';
    } catch (e) {
        return false;
    }
})();

export function isSoundEnabled() {
    return _enabled;
}

// Built on first enable so the AudioContext is created inside a user gesture, which the
// autoplay policy requires. A context made at page load starts suspended.
function ensureContext() {
    if (_ctx) return _ctx;
    var Ctor = window.AudioContext || window.webkitAudioContext;
    if (!Ctor) return null;
    try {
        _ctx = new Ctor();
    } catch (e) {
        return null;
    }
    // Gentle limiter, purely as a safety net: MASTER_GAIN is tunable at runtime and the
    // tones overlap, so this stops a hand-raised value turning into crackle. At the
    // defaults it barely engages.
    var comp = _ctx.createDynamicsCompressor();
    comp.threshold.value = -3;
    comp.knee.value = 6;
    comp.ratio.value = 4;
    comp.attack.value = 0.003;
    comp.release.value = 0.12;
    _bus = _ctx.createGain();
    _bus.gain.value = MASTER_GAIN;
    _bus.connect(comp);

    // The body bypasses the echo send. Repeats of a low tone overlap into a drone
    // instead of reading as space, so only the bright layers get the room.
    _dry = _ctx.createGain();
    _dry.gain.value = MASTER_GAIN;
    _dry.connect(comp);

    // Echo send. Built once and left connected: creating the delay per cue would leave
    // a feedback loop running for every block ever mined.
    _delay = _ctx.createDelay(1.0);
    _delay.delayTime.value = ECHO_TIME;
    _delayFb = _ctx.createGain();
    _delayFb.gain.value = ECHO_FEEDBACK;
    _delayWet = _ctx.createGain();
    _delayWet.gain.value = ECHO_WET;
    _bus.connect(_delay);
    _delay.connect(_delayFb);
    _delayFb.connect(_delay); // the feedback loop
    _delay.connect(_delayWet);
    _delayWet.connect(comp);

    comp.connect(_ctx.destination);
    return _ctx;
}

// Attack, hold, fast decay to a sustain level, then a long ring out, with an optional
// downward pitch punch on the onset. The two-stage decay is what lets the cue hit hard
// and still drag out; the hold keeps it from dying before you register it.
// exponentialRampToValueAtTime cannot reach 0, hence the small floor plus an explicit
// stop; a linear ramp to zero clicks.
function tone(ctx, startAt, freq, holdSecs, durSecs, peak, dest) {
    var osc = ctx.createOscillator();
    var g = ctx.createGain();
    osc.type = 'sine';
    // An array means [from, to]: the body slides down, the others punch down onto pitch.
    if (Array.isArray(freq)) {
        osc.frequency.setValueAtTime(freq[0], startAt);
        osc.frequency.exponentialRampToValueAtTime(freq[1], startAt + durSecs);
        freq = freq[0];
    } else if (PUNCH_PITCH_MULT > 1) {
        osc.frequency.setValueAtTime(freq * PUNCH_PITCH_MULT, startAt);
        osc.frequency.exponentialRampToValueAtTime(freq, startAt + PUNCH_PITCH_SECS);
    } else {
        osc.frequency.setValueAtTime(freq, startAt);
    }

    // Vibrato: an LFO summed into frequency. Depth is in Hz, taken as a fraction of this
    // tone's own pitch so the wobble sounds the same on the beep and on the settle.
    if (VIBRATO_DEPTH_FRAC > 0) {
        var lfo = ctx.createOscillator();
        var lfoGain = ctx.createGain();
        lfo.type = 'sine';
        lfo.frequency.setValueAtTime(VIBRATO_HZ, startAt);
        lfoGain.gain.setValueAtTime(freq * VIBRATO_DEPTH_FRAC, startAt);
        lfo.connect(lfoGain);
        lfoGain.connect(osc.frequency);
        lfo.start(startAt);
        lfo.stop(startAt + durSecs + 0.02);
    }

    var sustainAt = startAt + ATTACK_SECS + holdSecs + DECAY_FAST_SECS;
    g.gain.setValueAtTime(0.0001, startAt);
    g.gain.exponentialRampToValueAtTime(peak, startAt + ATTACK_SECS);
    g.gain.setValueAtTime(peak, startAt + ATTACK_SECS + holdSecs);
    // Fast fall to the sustain level, then the long ring out. Clamped so a short tone
    // cannot schedule the two stages out of order.
    if (sustainAt < startAt + durSecs) {
        g.gain.exponentialRampToValueAtTime(peak * SUSTAIN_FRAC, sustainAt);
    }
    g.gain.exponentialRampToValueAtTime(0.0001, startAt + durSecs);
    osc.connect(g);
    g.connect(dest || _bus);
    osc.start(startAt);
    osc.stop(startAt + durSecs + 0.02);
}

// Play the cue. Safe to call unconditionally: no-ops when disabled, unsupported or
// rate-limited, and never throws into the SSE handler.
export function playBlockCue() {
    if (!_enabled) return;
    var now = Date.now();
    if (now - _lastCueAt < MIN_CUE_GAP_MS) return;
    _lastCueAt = now;
    try {
        var ctx = ensureContext();
        if (!ctx || !_bus) return;
        // The OS or browser may have suspended it (audio focus lost, some platforms on
        // backgrounding). resume() is async, but the tones are scheduled off
        // currentTime, so a late resume still plays them in order.
        if (ctx.state === 'suspended') ctx.resume();
        var t0 = ctx.currentTime + 0.01;

        tone(ctx, t0, BEEP_HZ, BEEP_HOLD_SECS, BEEP_SECS, BEEP_GAIN);
        // Harmonic decays sooner than the fundamental, so the tone brightens on the
        // attack then mellows as it rings out instead of staying uniformly shrill.
        tone(ctx, t0, BEEP_HZ * BEEP_HARMONIC_MULT, BEEP_HOLD_SECS * 0.5, BEEP_SECS * 0.6, BEEP_HARMONIC_GAIN);
        tone(ctx, t0 + SETTLE_DELAY, SETTLE_HZ, SETTLE_HOLD_SECS, SETTLE_SECS, SETTLE_GAIN);
        if (BODY_GAIN > 0) {
            tone(ctx, t0 + BODY_DELAY, [BODY_HI_HZ, BODY_LO_HZ],
                BODY_HOLD_SECS, BODY_SECS, BODY_GAIN, _dry);
        }
    } catch (e) {
        /* audio is decoration; never let it break block handling */
    }
}

// Returns the new state. Plays the cue when switching on, both as confirmation and to
// unlock the context inside the click.
export function toggleSound() {
    _enabled = !_enabled;
    try {
        localStorage.setItem('hb_sound', _enabled ? 'on' : 'off');
    } catch (e) {}
    if (_enabled) {
        var ctx = ensureContext();
        if (ctx && ctx.state === 'suspended') ctx.resume();
        _lastCueAt = 0; // the confirmation cue should not be swallowed by the limiter
        playBlockCue();
    }
    return _enabled;
}

// Every tunable, as get/set pairs. One table so a reader and a writer can never drift
// apart: tuneSound and soundSettings are both derived from it.
var PARAMS = {
    MASTER_GAIN: [
        function () { return MASTER_GAIN; },
        function (v) {
            MASTER_GAIN = v;
            if (_bus) _bus.gain.value = v;
            if (_dry) _dry.gain.value = v;
        }
    ],
    BODY_HI_HZ: [function () { return BODY_HI_HZ; }, function (v) { BODY_HI_HZ = v; }],
    BODY_LO_HZ: [function () { return BODY_LO_HZ; }, function (v) { BODY_LO_HZ = v; }],
    BODY_DELAY: [function () { return BODY_DELAY; }, function (v) { BODY_DELAY = v; }],
    BODY_HOLD_SECS: [function () { return BODY_HOLD_SECS; }, function (v) { BODY_HOLD_SECS = v; }],
    BODY_SECS: [function () { return BODY_SECS; }, function (v) { BODY_SECS = v; }],
    BODY_GAIN: [function () { return BODY_GAIN; }, function (v) { BODY_GAIN = v; }],
    ATTACK_SECS: [function () { return ATTACK_SECS; }, function (v) { ATTACK_SECS = v; }],
    PUNCH_PITCH_MULT: [function () { return PUNCH_PITCH_MULT; }, function (v) { PUNCH_PITCH_MULT = v; }],
    PUNCH_PITCH_SECS: [function () { return PUNCH_PITCH_SECS; }, function (v) { PUNCH_PITCH_SECS = v; }],
    DECAY_FAST_SECS: [function () { return DECAY_FAST_SECS; }, function (v) { DECAY_FAST_SECS = v; }],
    SUSTAIN_FRAC: [function () { return SUSTAIN_FRAC; }, function (v) { SUSTAIN_FRAC = v; }],
    BEEP_HZ: [function () { return BEEP_HZ; }, function (v) { BEEP_HZ = v; }],
    BEEP_HOLD_SECS: [function () { return BEEP_HOLD_SECS; }, function (v) { BEEP_HOLD_SECS = v; }],
    BEEP_SECS: [function () { return BEEP_SECS; }, function (v) { BEEP_SECS = v; }],
    BEEP_GAIN: [function () { return BEEP_GAIN; }, function (v) { BEEP_GAIN = v; }],
    BEEP_HARMONIC_MULT: [function () { return BEEP_HARMONIC_MULT; }, function (v) { BEEP_HARMONIC_MULT = v; }],
    BEEP_HARMONIC_GAIN: [function () { return BEEP_HARMONIC_GAIN; }, function (v) { BEEP_HARMONIC_GAIN = v; }],
    SETTLE_HZ: [function () { return SETTLE_HZ; }, function (v) { SETTLE_HZ = v; }],
    SETTLE_HOLD_SECS: [function () { return SETTLE_HOLD_SECS; }, function (v) { SETTLE_HOLD_SECS = v; }],
    SETTLE_SECS: [function () { return SETTLE_SECS; }, function (v) { SETTLE_SECS = v; }],
    SETTLE_DELAY: [function () { return SETTLE_DELAY; }, function (v) { SETTLE_DELAY = v; }],
    SETTLE_GAIN: [function () { return SETTLE_GAIN; }, function (v) { SETTLE_GAIN = v; }],
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
    VIBRATO_HZ: [function () { return VIBRATO_HZ; }, function (v) { VIBRATO_HZ = v; }],
    VIBRATO_DEPTH_FRAC: [function () { return VIBRATO_DEPTH_FRAC; }, function (v) { VIBRATO_DEPTH_FRAC = v; }],
    MIN_CUE_GAP_MS: [function () { return MIN_CUE_GAP_MS; }, function (v) { MIN_CUE_GAP_MS = v; }]
};

// The defaults as loaded, so soundSettings() can show what has actually been changed.
var DEFAULTS = (function () {
    var d = {};
    Object.keys(PARAMS).forEach(function (k) { d[k] = PARAMS[k][0](); });
    return d;
})();

// Current values. Returns them for programmatic use and prints a paste-ready
// _hbSoundTune(...) call containing only what differs from the defaults, which is what
// you want when baking a tuning session back into the constants at the top of this file.
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

// Live tuning without a rebuild: window._hbSoundTune({ MASTER_GAIN: 0.5 }) then
// window._hbPlayBlockCue(). Nothing persists, so a reload restores the defaults above.
// Unknown keys are returned so a typo is visible instead of silently doing nothing.
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
