// Regression gate for Heartbeat in-flight brick reconciliation across chunked
// post-block carry, cinematic block reveals, and hidden-tab recovery.
//
//   node scripts/test-heartbeat-carry.mjs
//
// Drives the real assets/js ES modules under a minimal DOM + controllable rAF
// shim (same pattern as test-heartbeat-feed-escaping.mjs and probe-js-modules.mjs).

import { pathToFileURL } from 'node:url';
import { resolve } from 'node:path';

const noop = () => {};
const makeEl = () => ({
    style: {},
    dataset: {},
    classList: { add: noop, remove: noop, toggle: noop, contains: () => false },
    appendChild: noop,
    removeChild: noop,
    addEventListener: noop,
    removeEventListener: noop,
    setAttribute: noop,
    getAttribute: () => null,
    remove: noop,
    focus: noop,
    click: noop,
    querySelector: () => null,
    querySelectorAll: () => [],
    getBoundingClientRect: () => ({ width: 0, height: 0, top: 0, left: 0 }),
    insertAdjacentHTML: noop,
    scrollIntoView: noop,
    getContext: () => null,
});

// Controllable rAF queue so chunked placement and reveal beats can be stepped.
const rafQueue = [];
let rafId = 0;
const requestAnimationFrame = (cb) => {
    rafId += 1;
    rafQueue.push([rafId, cb]);
    return rafId;
};
const cancelAnimationFrame = (id) => {
    const i = rafQueue.findIndex(([q]) => q === id);
    if (i >= 0) rafQueue.splice(i, 1);
};
function drainRafs(maxSteps = 50) {
    let steps = 0;
    while (rafQueue.length > 0 && steps < maxSteps) {
        const [, cb] = rafQueue.shift();
        cb(Date.now());
        steps += 1;
    }
}

globalThis.requestAnimationFrame = requestAnimationFrame;
globalThis.cancelAnimationFrame = cancelAnimationFrame;
globalThis.window = {
    addEventListener: noop,
    removeEventListener: noop,
    devicePixelRatio: 1,
    innerWidth: 1200,
    innerHeight: 800,
    location: { href: 'http://localhost/', search: '', hostname: 'localhost' },
    matchMedia: () => ({ matches: false, addEventListener: noop }),
    requestAnimationFrame,
    cancelAnimationFrame,
    setTimeout: noop,
    clearTimeout: noop,
    setInterval: noop,
    clearInterval: noop,
    getComputedStyle: () => ({ getPropertyValue: () => '' }),
};
globalThis.document = {
    addEventListener: noop,
    removeEventListener: noop,
    readyState: 'complete',
    hidden: false,
    fullscreenElement: null,
    fullscreenEnabled: false,
    webkitFullscreenElement: null,
    webkitFullscreenEnabled: false,
    createElement: makeEl,
    createDocumentFragment: makeEl,
    getElementById: () => null,
    querySelector: () => null,
    querySelectorAll: () => [],
    body: makeEl(),
    documentElement: makeEl(),
};
Object.defineProperty(globalThis, 'navigator', {
    value: { clipboard: { writeText: () => Promise.resolve() }, userAgent: 'test-heartbeat-carry' },
    configurable: true,
});
globalThis.localStorage = { getItem: () => null, setItem: noop, removeItem: noop };
globalThis.ResizeObserver = class { observe() {} unobserve() {} disconnect() {} };
globalThis.EventSource = class { constructor() {} close() {} addEventListener() {} };
globalThis.AudioContext = class { constructor() {} createGain() { return { connect: noop, gain: {} }; } };
globalThis.performance = globalThis.performance || { now: () => Date.now() };

const modUrl = (name) => pathToFileURL(resolve('assets/js', name)).href;
const state = await import(modUrl('heartbeat-state.js'));
const timeline = await import(modUrl('heartbeat-timeline.js'));
const blips = await import(modUrl('heartbeat-blips.js'));
const interaction = await import(modUrl('heartbeat-interaction.js'));
const sse = await import(modUrl('heartbeat-sse.js'));
const render = await import(modUrl('heartbeat-render.js'));
const vitals = await import(modUrl('heartbeat-vitals.js'));
const hbMain = await import(modUrl('heartbeat.js'));

const failures = [];
let passed = 0;

function assert(cond, msg) {
    if (!cond) throw new Error(msg);
}

function runScenario(name, fn) {
    rafQueue.length = 0;
    document.hidden = false;
    try {
        fn();
        passed += 1;
        console.log(`PASS  ${name}`);
    } catch (e) {
        failures.push(`${name}: ${e.message}`);
        console.error(`FAIL  ${name}: ${e.message}`);
    }
}

function txid(n) {
    return n.toString(16).padStart(8, '0').repeat(8);
}

function resetState(overrides = {}) {
    const liveSeg = timeline.createFlatlineSegment(0, null);
    state.setState(Object.assign({
        timeline: [liveSeg],
        virtualX: 0,
        zoom: 1,
        minZoom: 0.03,
        maxZoom: 50,
        width: 1200,
        height: 400,
        viewOffset: 0,
        viewOffsetY: 0,
        autoFollow: false,
        paused: false,
        isDragging: false,
        _pinching: false,
        renderMode: 'bricks',
        lastBlockTime: Math.floor(Date.now() / 1000) - 300,
        recentBlocks: [],
        currentColor: '#00e676',
        _v2ConnState: 'live',
        _lastSseEventTs: Date.now() / 1000,
        _pendingAdds: new Map(),
        _txBatchQueue: [],
        _removedTxids: new Set(),
        _exactConfirmedByHash: new Map(),
        _hiddenTxMap: new Map(),
        _deferredDetails: new Map(),
        _pendingCarryByTxid: null,
        _revealEnabled: false,
    }, overrides));
    return state.getState();
}

function installTxs(count, pending = false) {
    const transactions = [];
    for (let i = 0; i < count; i++) {
        if (pending) {
            transactions.push([txid(i), 1700000000 + i, null, null, 500000, 0, '', 0]);
        } else {
            transactions.push([txid(i), 1700000000 + i, 1000, 200, 500000, 0, '', 0]);
        }
    }
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions,
        recent_blocks: [],
        pending_blocks: [],
        coverage: { status: 'complete' },
    }), false);
    drainRafs();
}

// ── Scenario 1: membership_remove during chunked post-block carry ────────────
runScenario('1. membership_remove during chunked post-block carry -> zero ghost brick', () => {
    const N = 6000; // > CARRY_CHUNK (4000) so carry spans multiple rAF chunks
    const victim = txid(5999);
    resetState({ _revealEnabled: false, autoFollow: false });
    installTxs(N, false);

    sse.processLiveBlock({
        hash: '11'.repeat(32),
        height: 900001,
        timestamp: Math.floor(Date.now() / 1000),
        tx_count: 10,
        total_fees: 1000000,
        size: 1000000,
        weight: 4000000,
    });

    const sMid = state.getState();
    const midSeg = sMid.timeline[sMid.timeline.length - 1];
    assert(rafQueue.length > 0, 'carry should be mid-flight across rAF chunks');
    assert(!midSeg._blipsByTxid.has(victim), 'victim should still be waiting in carry chunk');
    assert(sMid._pendingCarryByTxid && sMid._pendingCarryByTxid.has(victim),
        'victim should be indexed in _pendingCarryByTxid');

    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_remove', txids: [victim] }));
    assert(sMid._removedTxids.has(victim), 'victim should be recorded in _removedTxids while carry is active');

    drainRafs();

    const sEnd = state.getState();
    const finalSeg = sEnd.timeline[sEnd.timeline.length - 1];
    assert(!finalSeg._blipsByTxid.has(victim), 'removed victim resurrected in _blipsByTxid');
    assert(!finalSeg.blips.some((b) => b.txid === victim && !b._removed),
        'removed victim resurrected in seg.blips');
    assert(sEnd._pendingCarryByTxid === null, '_pendingCarryByTxid should be cleared after carry');
    assert(sEnd._removedTxids.size === 0, '_removedTxids should clear once idle');
});

// ── Scenario 2: tx_details during chunked post-block carry ───────────────────
runScenario('2. tx_details during chunked post-block carry -> upgrades fee/colour & preserves value', () => {
    const N = 6000;
    const victim = txid(5999);
    resetState({ _revealEnabled: false, autoFollow: false });
    installTxs(N, true); // feePending = true, value = 500000

    sse.processLiveBlock({
        hash: '22'.repeat(32),
        height: 900002,
        timestamp: Math.floor(Date.now() / 1000),
        tx_count: 10,
        total_fees: 1000000,
        size: 1000000,
        weight: 4000000,
    });

    assert(rafQueue.length > 0, 'carry should be mid-flight');

    // Reconcile hydration can deliver value: null alongside real fee & vsize.
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'tx_details',
        tx: [victim, 1700005999, 50000, 200, null, 0, '', 0],
    }));

    drainRafs();

    const finalSeg = state.getState().timeline[state.getState().timeline.length - 1];
    const upgraded = finalSeg._blipsByTxid.get(victim);
    assert(upgraded, 'victim brick should exist after carry');
    assert(!upgraded.feePending, 'brick stayed feePending=true after mid-carry tx_details');
    assert(upgraded.fee === 50000, `expected fee 50000, got ${upgraded.fee}`);
    assert(upgraded.feeRate === 250, `expected feeRate 250, got ${upgraded.feeRate}`);
    assert(upgraded.color === timeline.feeRateColor(250), `expected crimson fee color, got ${upgraded.color}`);
    assert(upgraded.value === 500000, `expected known value 500000 preserved when tx_details had null, got ${upgraded.value}`);
});

// ── Scenario 3: block_inclusion during chunked post-block carry ──────────────
runScenario('3. block_inclusion during chunked post-block carry -> mined tx never resurrects', () => {
    const N = 6000;
    const victim = txid(5999);
    const HASH = '33'.repeat(32);
    resetState({ _revealEnabled: false, autoFollow: false });
    installTxs(N, false);

    sse.processLiveBlock({
        hash: HASH,
        height: 900003,
        timestamp: Math.floor(Date.now() / 1000),
        tx_count: 10,
        total_fees: 1000000,
        size: 1000000,
        weight: 4000000,
    });

    assert(rafQueue.length > 0, 'carry should be mid-flight');
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_inclusion',
        hash: HASH,
        height: 900003,
        confirmed_txids: [victim],
    }));

    drainRafs();

    const finalSeg = state.getState().timeline[state.getState().timeline.length - 1];
    const ghost = finalSeg._blipsByTxid.get(victim);
    assert(!ghost, 'mined tx resurrected in _blipsByTxid after carry completed');
    assert(!finalSeg.blips.some((b) => b.txid === victim && !b._removed && b.fadeStart === 0),
        'mined tx resurrected as live brick in seg.blips');
});

// ── Scenario 4: Server real ordering (block_observed -> block_inclusion -> block_details) ──
runScenario('4. block_observed -> block_inclusion -> block_details on LIVE follower -> reveal absorbs all mined bricks', () => {
    const HASH = '44'.repeat(32);
    const N = 100;
    const MINED = 40;
    resetState({
        virtualX: 400,
        zoom: 1.9,
        autoFollow: true,
        _revealEnabled: true,
    });
    installTxs(N, false);

    const mined = [];
    for (let i = 0; i < MINED; i++) mined.push(txid(i));
    const nowTs = Math.floor(Date.now() / 1000);

    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: HASH, observed_at: nowTs }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: HASH, height: 900004, confirmed_txids: mined }));

    let s = state.getState();
    let seg = s.timeline.find((t) => t._pendingForHash === HASH) || s.timeline[s.timeline.length - 1];
    assert(s.timeline.some((t) => t.type === 'pending_block' && t.hash === HASH),
        'block_observed should reserve a pending_block slot');
    const prematureShatter = seg.blips.filter((b) => b.fadeStart > 0).length;
    assert(prematureShatter === 0, `expected 0 premature shatters before spike exists, got ${prematureShatter}`);
    assert(seg._blipsByTxid.get(mined[0])._pendingInclusionHash === HASH,
        'mined brick should be tagged with _pendingInclusionHash');

    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details',
        hash: HASH,
        height: 900004,
        timestamp: nowTs,
        tx_count: 2000,
        total_fees: 5000000,
        size: 1200000,
        weight: 3900000,
        observed_at: nowTs,
    }));

    s = state.getState();
    assert(!!s._reveal, 'reveal should be armed after block_details');
    assert(s._reveal.confirmed.length === MINED,
        `expected ${MINED} bricks in reveal.confirmed, got ${s._reveal.confirmed.length}`);
    assert(s._reveal.leftovers.length === N - MINED,
        `expected ${N - MINED} bricks in reveal.leftovers, got ${s._reveal.leftovers.length}`);

    // Verify edge case: if a DIFFERENT block arrives before hashA's block_details,
    // pending-inclusion bricks for hashA are excluded from carriedLeftovers.
    const HASH_A = '4a'.repeat(32);
    const HASH_B = '4b'.repeat(32);
    resetState({ virtualX: 400, zoom: 1.9, autoFollow: true, _revealEnabled: true });
    installTxs(20, false);
    const minedA = [txid(0), txid(1), txid(2)];
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: HASH_A, height: 900010, confirmed_txids: minedA }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details',
        hash: HASH_B,
        height: 900011,
        timestamp: nowTs,
        tx_count: 100,
        total_fees: 1000000,
        size: 500000,
        weight: 2000000,
        observed_at: nowTs,
    }));
    const sDiff = state.getState();
    assert(!!sDiff._reveal, 'reveal should be armed for block B');
    for (const tid of minedA) {
        assert(!sDiff._reveal.leftovers.some((b) => b.txid === tid),
            `tx ${tid} confirmed in block A leaked into block B carriedLeftovers`);
    }
});

// ── Scenario 5: LIVE 6s reveal window (form + harvest beats) with interleaved effects ──
runScenario('5. LIVE reveal window (form & harvest beats) with membership_remove, tx_details, and block_inclusion', () => {
    const HASH = '55'.repeat(32);
    const N = 80;
    resetState({
        virtualX: 400,
        zoom: 1.9,
        autoFollow: true,
        _revealEnabled: true,
    });
    installTxs(N, true); // feePending = true, value = 500000

    const nowTs = Math.floor(Date.now() / 1000);
    // Block details arrives first -> reveal starts in 'form' beat with all N bricks in leftovers.
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details',
        hash: HASH,
        height: 900005,
        timestamp: nowTs,
        tx_count: 1000,
        total_fees: 3000000,
        size: 1000000,
        weight: 3500000,
        observed_at: nowTs,
    }));

    const s = state.getState();
    assert(s._reveal && s._reveal.phase === 'form', 'reveal should be in form beat');
    assert(s._reveal.leftovers.length === N, `expected ${N} leftovers initially`);

    const removedInForm = txid(1);
    const upgradedInForm = txid(2);
    const includedInForm = [txid(10), txid(11), txid(12)];
    const removedInHarvest = txid(3);
    const upgradedInHarvest = txid(4);
    const lateIncludedInHarvest = [txid(20), txid(21)];

    // Interleaved effects during 'form' beat (0..2.5s):
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_remove', txids: [removedInForm] }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'tx_details',
        tx: [upgradedInForm, 1700000002, 20000, 200, null, 0, '', 0], // 100 sat/vB, value: null
    }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_inclusion',
        hash: HASH,
        height: 900005,
        confirmed_txids: includedInForm,
    }));

    assert(!s._reveal.leftovers.some((b) => b.txid === removedInForm),
        'removedInForm must be filtered out of reveal.leftovers');
    assert(s._reveal.confirmed.length === includedInForm.length,
        `expected ${includedInForm.length} confirmed in form beat, got ${s._reveal.confirmed.length}`);

    // Advance into 'harvest' beat (2.5..6.0s): mark harvested like startRevealHarvest does.
    s._reveal.phase = 'harvest';
    s._reveal.harvested = true;
    for (const b of s._reveal.confirmed) b.fadeStart = nowTs;

    // Interleaved effects during 'harvest' beat:
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_remove', txids: [removedInHarvest] }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'tx_details',
        tx: [upgradedInHarvest, 1700000004, 4000, 200, 750000, 0, '', 0], // 20 sat/vB, new value
    }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_inclusion',
        hash: HASH,
        height: 900005,
        confirmed_txids: lateIncludedInHarvest,
    }));

    // Queue a post-block arrival during the reveal to also test _carryEndX separation.
    const postBlockTx = txid(999);
    s._txBatchQueue.push({
        txid: postBlockTx,
        first_seen: nowTs - 10,
        fee: 2000,
        vsize: 200,
        value: 100000,
        flags: 0,
        op_return_text: '',
    });

    // Finalize reveal -> relays leftovers onto newSeg.
    window._hbEndReveal(true);
    drainRafs();

    const finalSeg = state.getState().timeline[state.getState().timeline.length - 1];
    assert(!finalSeg._blipsByTxid.has(removedInForm), 'removedInForm resurrected as ghost');
    assert(!finalSeg._blipsByTxid.has(removedInHarvest), 'removedInHarvest resurrected as ghost');
    for (const tid of includedInForm.concat(lateIncludedInHarvest)) {
        assert(!finalSeg._blipsByTxid.has(tid), `confirmed tx ${tid} resurrected as ghost on newSeg`);
    }

    const up1 = finalSeg._blipsByTxid.get(upgradedInForm);
    assert(up1 && !up1.feePending && up1.feeRate === 100 && up1.value === 500000,
        'upgradedInForm lost its fee/value update across reveal relay');

    const up2 = finalSeg._blipsByTxid.get(upgradedInHarvest);
    assert(up2 && !up2.feePending && up2.feeRate === 20 && up2.value === 750000,
        'upgradedInHarvest lost its fee/value update across reveal relay');

    const expectedRemaining = N - 2 - includedInForm.length - lateIncludedInHarvest.length;
    assert(finalSeg._blipsByTxid.size === expectedRemaining,
        `expected ${expectedRemaining} surviving bricks, got ${finalSeg._blipsByTxid.size}`);

    // Post-reveal flush must place post-block arrivals to the right of _carryEndX.
    assert(typeof finalSeg._carryEndX === 'number' && finalSeg._carryEndX > 0,
        '_carryEndX should be recorded on newSeg');
    state.getState().virtualX = finalSeg._carryEndX + 80;
    sse.flushTxBatch(Date.now());
    const postBlip = finalSeg._blipsByTxid.get(postBlockTx);
    assert(postBlip && postBlip.gridX >= finalSeg._carryEndX + 5,
        `post-block tx landed at ${postBlip ? postBlip.gridX : 'missing'}, expected >= ${finalSeg._carryEndX + 5}`);
});

// ── Scenario 6: Review polish & hidden-tab catch-up invariants ───────────────
runScenario('6. Polish invariants (stale badge, formatFeeRate, vbytes, future hit-test, fit unpin, hidden pre-block routing)', () => {
    // 6a. Stale connection badge
    const s = resetState({ _v2ConnState: 'live', _lastSseEventTs: Date.now() / 1000 - 120 });
    assert(window.getHeartbeatConnectionState() === 'stale',
        `expected stale after 120s silence, got ${window.getHeartbeatConnectionState()}`);
    s._lastSseEventTs = Date.now() / 1000;
    assert(window.getHeartbeatConnectionState() === 'live', 'expected live when fresh');

    // 6b. formatFeeRate(0) === '0.0'
    assert(timeline.formatFeeRate(0) === '0.0', `formatFeeRate(0) was ${timeline.formatFeeRate(0)}`);
    assert(timeline.formatFeeRate(0.25) === '0.25', `formatFeeRate(0.25) was ${timeline.formatFeeRate(0.25)}`);
    assert(timeline.formatFeeRate(12.34) === '12.3', `formatFeeRate(12.34) was ${timeline.formatFeeRate(12.34)}`);
    assert(timeline.formatFeeRate(null) === 'pending', `formatFeeRate(null) was ${timeline.formatFeeRate(null)}`);

    // 6c. metrics_updated reads nm.vbytes
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'metrics_updated',
        node_mempool: { tx_count: 5000, vbytes: 12345000, observed_at: 1700000000, stale: false },
    }));
    assert(Math.abs(s.mempoolMB - 12.345) < 1e-6, `expected mempoolMB 12.345, got ${s.mempoolMB}`);

    // 6d. Hit-testing skips future-timestamped bricks
    const seg = s.timeline[s.timeline.length - 1];
    const futureBlip = sse.buildBlip({ txid: txid(777), first_seen: 1700000000, fee: 1000, vsize: 200, value: 10000 }, 50, 50, 0, Date.now() / 1000 + 60, true);
    sse.registerBlipInSeg(seg, futureBlip);
    s.virtualX = 200;
    s.zoom = 5;
    assert(blips.blipAtVirtualX(50, 10) === null, 'blipAtVirtualX hit a future (un-dropped) brick');
    assert(blips.blipAtCanvasXY(blips.virtualToCanvas(50), 215, 220) === null, 'blipAtCanvasXY hit a future brick');

    // 6e. 'fit' clears pinned tx
    let clearCalled = 0;
    window.__hbClearPinnedTx = () => { clearCalled += 1; };
    s._pinnedTxid = txid(777);
    s._pinnedBlip = futureBlip;
    interaction.handleControlClick('fit');
    assert(s._pinnedTxid === null && s._pinnedBlip === null && clearCalled === 1,
        'fit control did not clear pinned tx state');

    // 6f. Hidden-tab single-block catch-up + pre-block _hiddenTxMap routing
    resetState({ virtualX: 200, zoom: 1.9, autoFollow: true, _revealEnabled: true });
    installTxs(10, false);
    const nowSec = Math.floor(Date.now() / 1000);
    const blkTs = nowSec - 30;
    const blkHash = '66'.repeat(32);
    const preConfirmed = txid(8001);
    const preUnconfirmed = txid(8002);
    const postHidden = txid(8003);

    document.hidden = true;
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: preConfirmed, first_seen: blkTs - 20 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: preUnconfirmed, first_seen: blkTs - 15 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: blkHash, height: 900006, confirmed_txids: [preConfirmed] }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: blkHash, height: 900006, timestamp: blkTs,
        tx_count: 500, total_fees: 2000000, size: 800000, weight: 3000000, observed_at: blkTs,
    }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: postHidden, first_seen: blkTs + 15 }));

    document.hidden = false;
    window._hbRecoverTimeline(60);
    drainRafs();

    const recSeg = state.getState().timeline[state.getState().timeline.length - 1];
    assert(!recSeg._blipsByTxid.has(preConfirmed), 'pre-block hidden confirmed tx should be dropped');
    const preBlip = recSeg._blipsByTxid.get(preUnconfirmed);
    const postBlip = recSeg._blipsByTxid.get(postHidden);
    assert(preBlip && !preBlip.isDrop, 'pre-block hidden unconfirmed tx should carry instantaneously (isDrop=false)');
    assert(postBlip && postBlip.gridX > recSeg._carryEndX,
        `post-block hidden tx (${postBlip ? postBlip.gridX : 'missing'}) should sit past carryEndX (${recSeg._carryEndX})`);
    assert(preBlip.gridX <= recSeg._carryEndX,
        `pre-block hidden tx (${preBlip.gridX}) should sit inside carried backlog (<= ${recSeg._carryEndX})`);
});

// ── Scenario 7: Long (>5m) hidden tab without blocks -> no tower wall at 300s boundary ──
runScenario('7. Long (20m) hidden tab spreads arrivals across actual window with zero 300s boundary tower', () => {
    resetState({ virtualX: 500, zoom: 1.0, height: 600, autoFollow: true, _revealEnabled: true });
    const nowSec = Math.floor(Date.now() / 1000);
    const hiddenSecs = 1200; // 20 minutes (> 300s clamp!)

    document.hidden = true;
    for (let i = 0; i < 1200; i++) {
        const id = txid(20000 + i);
        const ts = nowSec - hiddenSecs + i;
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: id, first_seen: ts }));
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [id, ts, 400, 200, 50000, 0, '', 0] }));
    }
    document.hidden = false;
    window._hbRecoverTimeline(hiddenSecs);

    const seg = state.getState().timeline[state.getState().timeline.length - 1];
    assert(seg._blipsByTxid.size === 1200, `expected all 1200 hidden txs placed, got ${seg._blipsByTxid.size}`);
    const colHeights = Object.values(seg._colHeights);
    const maxColH = Math.max(...colHeights);
    const colsUsed = colHeights.length;
    assert(colsUsed >= 250, `expected >=250 columns used across 20m gap, got ${colsUsed}`);
    assert(maxColH < 140, `300s boundary tower formed! maxColH=${Math.round(maxColH)}px`);
});

// ── Scenario 8: BlockObserved fast & 45s slow reveal, both arrival orders + reorg ──
runScenario('8. BlockObserved reserved slot (45s slow details, late inclusion, pending reorg, 0-leftover reveal)', () => {
    // 8a. Fixture A (976728-style 45s slow block_details):
    // Seed an older block #976727 + pre-block txs, reserve slot, receive 45s of
    // genuine post-block txs to the right of the pending slot, then resolve with block_details.
    const nowSec = Math.floor(Date.now() / 1000);
    const prevTs = nowSec - 2700; // 45m wait
    const HASH_PREV = '80'.repeat(32);
    const HASH_SLOW = '81'.repeat(32);
    resetState({
        virtualX: 200,
        zoom: 1.5,
        autoFollow: false,
        viewOffset: 10, // Inspecting older history to the left of preFlatline.x_end
        _revealEnabled: false,
    });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: HASH_PREV, height: 976727, timestamp: prevTs,
        tx_count: 1500, total_fees: 4000000, size: 1200000, weight: 3800000,
        inter_block_seconds: 600,
    }]), true);
    installTxs(40, false);

    const sA = state.getState();
    const olderBlock = sA.timeline.find((seg) => seg.type === 'block' && seg.height === 976727);
    assert(olderBlock, 'older block #976727 should exist');
    const olderStartBefore = olderBlock.x_start;
    const olderEndBefore = olderBlock.x_end;
    const preFlatline = sA.timeline[sA.timeline.length - 1];
    // Simulate a wide real-time live flatline before block_observed (e.g. 2000px > 672.5px)
    sA.virtualX = preFlatline.x_start + 2000;
    const inspectOffsetBefore = sA.viewOffset;

    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: HASH_SLOW, observed_at: nowSec - 45 }));
    const pendingSeg = sA.timeline.find((seg) => seg.type === 'pending_block' && seg.hash === HASH_SLOW);
    assert(pendingSeg, 'pending_block segment should be reserved on block_observed');
    assert(preFlatline._pendingForHash === HASH_SLOW, 'preFlatline should be tagged with _pendingForHash');

    // Simulate 45s of genuine post-block arrivals landing to the RIGHT of pending_block
    const postFlatline = sA.timeline[sA.timeline.length - 1];
    assert(postFlatline.type === 'flatline' && postFlatline.x_start === pendingSeg.x_end,
        'open postFlatline should start immediately at pendingSeg.x_end');
    sA.virtualX = postFlatline.x_start + 45 * state.FLATLINE_PX_PER_SEC;
    const postIds = [txid(81001), txid(81002), txid(81003)];
    for (let i = 0; i < postIds.length; i++) {
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: postIds[i], first_seen: nowSec - 30 + i }));
        sse.hbV2ApplyEffect(JSON.stringify({
            type: 'tx_details', tx: [postIds[i], nowSec - 30 + i, 2000, 200, 100000, 0, '', 0],
        }));
    }
    sse.flushTxBatch(Date.now());
    for (const pid of postIds) {
        const pb = postFlatline._blipsByTxid.get(pid);
        assert(pb && pb.gridX >= pendingSeg.x_end, `post-block tx ${pid} must land to the right of pending_block`);
    }

    // Check offscreen & focus helpers while pending_block is active
    sA.viewOffset = pendingSeg.x_end + 100;
    assert(window.hbV2IsPendingSlotOffscreen(HASH_SLOW) === true, 'pending slot should report offscreen when left of viewOffset');
    assert(window.hbV2FocusPendingSlot(HASH_SLOW) === true, 'hbV2FocusPendingSlot should find and focus the pending slot');
    assert(window.hbV2IsPendingSlotOffscreen(HASH_SLOW) === false, 'pending slot should be onscreen after focusing');
    // Restore inspecting viewOffset to older history to verify camera invariance across compression
    sA.autoFollow = false;
    sA.viewOffset = inspectOffsetBefore;

    // Now block_details arrives 45s later -> replaces pendingSeg in place & compresses preFlatline
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: HASH_SLOW, height: 976728, timestamp: nowSec,
        tx_count: 2500, total_fees: 8000000, size: 1500000, weight: 3950000,
    }));
    drainRafs();

    assert(!sA.timeline.some((seg) => seg.type === 'pending_block' && seg.hash === HASH_SLOW),
        'pending_block slot should be replaced once block_details arrives');
    const measuredBlock = sA.timeline.find((seg) => seg.type === 'block' && seg.height === 976728);
    assert(measuredBlock && measuredBlock.hash === HASH_SLOW, 'measured block #976728 should replace pending slot');
    assert(olderBlock.x_start === olderStartBefore && olderBlock.x_end === olderEndBefore,
        'compression must NEVER shift segments to the left of preFlatline');
    assert(sA.viewOffset === inspectOffsetBefore,
        'inspecting viewer looking at older history must stay frozen to the exact pixel');
    const expected45mW = timeline.historyFlatlineWidth(2700);
    assert(Math.abs((preFlatline.x_end - preFlatline.x_start) - expected45mW) < 0.01,
        `expected sealed preFlatline compressed to ${expected45mW}px, got ${preFlatline.x_end - preFlatline.x_start}`);
    assert(Math.abs(measuredBlock.x_start - preFlatline.x_end) < 0.01,
        'measuredBlock should sit seamlessly at compressed preFlatline.x_end');

    const finalLiveA = sA.timeline[sA.timeline.length - 1];
    assert(finalLiveA._blipsByTxid.size === 40 + postIds.length,
        `expected all 40 pre-block + ${postIds.length} post-block txs intact, got ${finalLiveA._blipsByTxid.size}`);
    for (const pid of postIds) {
        assert(finalLiveA._blipsByTxid.has(pid), `post-block tx ${pid} was lost during carry`);
    }

    // 8b. Fixture B (976726-style late block_inclusion 45s after block_details):
    const HASH_LATE = '82'.repeat(32);
    resetState({ virtualX: 300, zoom: 1.5, autoFollow: false, _revealEnabled: false });
    installTxs(30, false);
    const lateMined = [txid(0), txid(1), txid(2), txid(3), txid(4)];
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: HASH_LATE, observed_at: nowSec - 45 }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: HASH_LATE, height: 976726, timestamp: nowSec - 44,
        tx_count: 1000, total_fees: 2000000, size: 900000, weight: 3000000,
    }));
    drainRafs();
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_inclusion', hash: HASH_LATE, height: 976726, confirmed_txids: lateMined,
    }));
    const liveB = state.getState().timeline[state.getState().timeline.length - 1];
    for (const mid of lateMined) {
        assert(!liveB._blipsByTxid.has(mid), `late-included tx ${mid} remained as ghost in _blipsByTxid`);
        assert(!liveB.blips.some((b) => b.txid === mid && !b._removed && b.fadeStart === 0),
            `late-included tx ${mid} remained active in seg.blips`);
    }
    assert(liveB._blipsByTxid.size === 25, `expected 25 remaining bricks after late inclusion, got ${liveB._blipsByTxid.size}`);

    // 8c. block_disconnected on a pending_block slot merges adjacent flatlines with zero lost bricks:
    const HASH_ORPHAN = '83'.repeat(32);
    resetState({ virtualX: 200, zoom: 1.5, autoFollow: false, _revealEnabled: false });
    installTxs(15, false);
    const sC = state.getState();
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: HASH_ORPHAN, observed_at: nowSec }));
    const pendC = sC.timeline.find((seg) => seg.type === 'pending_block' && seg.hash === HASH_ORPHAN);
    const pendW = pendC.x_end - pendC.x_start;
    sC.virtualX += 50;
    const postOrphanTx = txid(83001);
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: postOrphanTx, first_seen: nowSec }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'tx_details', tx: [postOrphanTx, nowSec, 1500, 200, 50000, 0, '', 0],
    }));
    sse.flushTxBatch(Date.now());
    const virtBeforeDisconnect = sC.virtualX;
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_disconnected', hash: HASH_ORPHAN }));
    assert(sC.timeline.length === 1 && sC.timeline[0].type === 'flatline' && sC.timeline[0].x_end === null,
        `expected merged single open flatline after pending disconnect, got ${sC.timeline.length} segments`);
    assert(sC.timeline[0]._blipsByTxid.size === 16,
        `expected all 15 pre + 1 post bricks preserved after merge, got ${sC.timeline[0]._blipsByTxid.size}`);
    assert(Math.abs(sC.virtualX - (virtBeforeDisconnect - pendW)) < 0.01,
        `expected virtualX shrunk by pendingWidth (${pendW}), got ${sC.virtualX} vs ${virtBeforeDisconnect - pendW}`);

    // 8d. 0-leftover reveal edge case still arms _reveal cleanly:
    const HASH_ZERO = '84'.repeat(32);
    resetState({ virtualX: 200, zoom: 1.5, autoFollow: true, _revealEnabled: true });
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: HASH_ZERO, height: 976730, timestamp: nowSec,
        tx_count: 1, total_fees: 0, size: 300, weight: 1000,
    }));
    const sZ = state.getState();
    assert(!!sZ._reveal && sZ._reveal.leftovers.length === 0,
        '0-leftover live block should still arm _reveal cleanly');
});

// ── Scenario 9: Exact inter_block_seconds (0s & negative MTP), progressive width & 7-stop wait color ──
runScenario('9. inter_block_seconds (0s & negative MTP), progressive compression & 7-stop waitColor', () => {
    assert(timeline.resolveInterBlockSeconds({ inter_block_seconds: 0 }) === 0, '0s must be preserved');
    assert(timeline.resolveInterBlockSeconds({ inter_block_seconds: -18 }) === -18, 'negative MTP delta must be preserved');
    assert(timeline.resolveInterBlockSeconds({}) === null, 'missing inter_block_seconds must resolve to null');

    // Q dip index in generatePQRST: 3 lead-in + 9 P-wave + 2 baseline + 4 Q-dip (pts[17] = qDepth)
    const pq0 = timeline.generatePQRST({ inter_block_seconds: 0, tx_count: 100, total_fees: 1000000, weight: 1000000 });
    const pqNeg = timeline.generatePQRST({ inter_block_seconds: -15, tx_count: 100, total_fees: 1000000, weight: 1000000 });
    const pq600 = timeline.generatePQRST({ inter_block_seconds: 600, tx_count: 100, total_fees: 1000000, weight: 1000000 });
    assert(pq0[17] === 8, `expected 0s Q dip = 8, got ${pq0[17]} (not fake 600s = 28)`);
    assert(pqNeg[17] === 8, `expected negative MTP Q dip = 8, got ${pqNeg[17]}`);
    assert(Math.abs(pq600[17] - 28) < 1e-6, `expected 600s Q dip = 28, got ${pq600[17]}`);

    const seg0 = timeline.createBlockSegment({ inter_block_seconds: 0, height: 1, tx_count: 1, total_fees: 0, weight: 1000 }, 0);
    const segNeg = timeline.createBlockSegment({ inter_block_seconds: -9, height: 2, tx_count: 1, total_fees: 0, weight: 1000 }, 0);
    assert(seg0.inter_block_seconds === 0, `createBlockSegment must preserve 0, got ${seg0.inter_block_seconds}`);
    assert(segNeg.inter_block_seconds === -9, `createBlockSegment must preserve -9, got ${segNeg.inter_block_seconds}`);

    assert(timeline.formatDuration(0) === '0s', `formatDuration(0) was ${timeline.formatDuration(0)}`);
    assert(timeline.formatDuration(-12) === '-12s (MTP)', `formatDuration(-12) was ${timeline.formatDuration(-12)}`);
    assert(timeline.formatDuration(null) === 'unknown', `formatDuration(null) was ${timeline.formatDuration(null)}`);

    // Progressive width curve checkpoints (0m < 5m < 10m < 20m < 30m < 45m < 60m < 90m)
    const widths = [0, 300, 600, 1200, 1800, 2700, 3600, 5400].map((s) => timeline.historyFlatlineWidth(s));
    const expectedW = [10, 55, 100, 220, 380, 672.5, 1017.5, 1497.5];
    for (let i = 0; i < widths.length; i++) {
        assert(Math.abs(widths[i] - expectedW[i]) < 1e-6,
            `historyFlatlineWidth checkpoint ${i}: expected ${expectedW[i]}, got ${widths[i]}`);
        if (i > 0) assert(widths[i] > widths[i - 1], 'historyFlatlineWidth must strictly increase');
    }
    assert(timeline.historyFlatlineWidth(99999) === 1600, 'soft cap should be 1600px');
    assert(timeline.historyFlatlineWidth(null) === 60, 'unknown interval should use 60px neutral width (distinct from 600s = 100px)');

    // Progressive 7-stop waitColor ramp
    const c5 = timeline.waitColor(300);
    const c10 = timeline.waitColor(600);
    const c20 = timeline.waitColor(1200);
    const c30 = timeline.waitColor(1800);
    const c45 = timeline.waitColor(2700);
    const c60 = timeline.waitColor(3600);
    const c90 = timeline.waitColor(5400);
    assert(c5 === '#00e676' && c10 === '#38bdf8' && c20 === '#facc15' && c30 === '#f97316'
        && c45 === '#ff4500' && c60 === '#dc2626' && c90 === '#c026d3',
        `waitColor stops mismatch: ${[c5, c10, c20, c30, c45, c60, c90].join(', ')}`);
    assert(c30 !== c60 && c60 !== c90 && c30 !== c90, '30m, 60m, and 90m colors must be distinct');
    assert(timeline.buildFlatlineGradient(null, 0, 500, 3600) === c60,
        'buildFlatlineGradient should fall back to solid hex when ctx is unavailable');
});

// ── Scenario 10: Historical mode isolation & 1-click return to LIVE ──────────
runScenario('10. Historical mode isolation & 1-click return to LIVE', () => {
    const nowSec = Math.floor(Date.now() / 1000);
    const LIVE_H0 = 950000;
    const LIVE_H1 = 950001;
    resetState({ virtualX: 300, zoom: 1.9, autoFollow: true, _revealEnabled: false });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'a0'.repeat(32), height: LIVE_H0, timestamp: nowSec - 600,
        tx_count: 1000, total_fees: 2000000, size: 1000000, weight: 3000000,
        inter_block_seconds: 600,
    }]), true);
    installTxs(10, false);

    const modeEvents = [];
    window.__hbHistoryModeChange = (meta) => { modeEvents.push(meta); };

    // Build a 41-block historical window centered around #840000
    const histBlocks = [];
    for (let i = -20; i <= 20; i++) {
        histBlocks.push({
            hash: ('h' + (840000 + i).toString(16)).padEnd(64, '0'),
            height: 840000 + i,
            timestamp: 1713571767 + i * 600,
            tx_count: 3000,
            total_fees: 50000000,
            size: 1600000,
            weight: 3990000,
            inter_block_seconds: i === -20 ? null : 600,
        });
    }

    const entered = window.hbV2EnterHistoryMode(JSON.stringify({
        target_height: 840000,
        target_hash: histBlocks[20].hash,
        tip_height: LIVE_H0,
        blocks: histBlocks,
    }));
    assert(entered === true, 'hbV2EnterHistoryMode should return true');

    const s = state.getState();
    assert(s._viewMode === 'history', 'should be in history mode');
    assert(s.timeline.filter((seg) => seg.type === 'block').length === 41, 'should render 41 historical blocks');
    assert(s.timeline[s.timeline.length - 1].x_end !== null, 'history timeline must NOT have an open trailing live flatline');
    assert(s.timeline.some((seg) => seg._historyTarget && seg.height === 840000), 'target #840000 should be marked _historyTarget');

    const histTimelineRef = s.timeline;
    const histViewOffsetBefore = s.viewOffset;
    const histZoomBefore = s.zoom;
    const histLenBefore = s.timeline.length;

    // Receive live block_details + 300 live txs spread across 180s while browsing history!
    s._liveBackup.enteredAt = nowSec - 180;
    const liveDuringHistoryTx = txid(99001);
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: 'a1'.repeat(32), height: LIVE_H1, timestamp: nowSec - 120,
        tx_count: 1800, total_fees: 3500000, size: 1100000, weight: 3500000,
    }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: liveDuringHistoryTx, first_seen: nowSec - 10 }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'tx_details', tx: [liveDuringHistoryTx, nowSec - 10, 2500, 200, 200000, 0, '', 0],
    }));
    for (let hi = 0; hi < 300; hi++) {
        const htid = txid(99100 + hi);
        const hts = nowSec - 115 + Math.floor((hi / 300) * 110);
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: htid, first_seen: hts }));
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [htid, hts, 800, 200, 50000, 0, '', hi + 1] }));
    }
    drainRafs();

    // Historical timeline & camera MUST be 100% undisturbed
    assert(s.timeline === histTimelineRef && s.timeline.length === histLenBefore,
        'live arrivals must NOT mutate the active historical timeline');
    assert(s.viewOffset === histViewOffsetBefore && s.zoom === histZoomBefore,
        'live arrivals must NOT disturb historical camera viewOffset/zoom');
    assert(!s.recentBlocks.some((rb) => rb.height === 840000),
        'recentBlocks must NOT be polluted by historical block #840000');
    assert(s.recentBlocks.some((rb) => rb.height === LIVE_H1),
        'recentBlocks should still record live block #950001');

    // 1-click return to LIVE
    const exited = window.hbV2ExitHistoryMode();
    assert(exited === true, 'hbV2ExitHistoryMode should return true');
    assert(s._viewMode === 'live' && s.autoFollow === true, 'should return to LIVE autoFollow');
    assert(s.timeline.some((seg) => seg.type === 'block' && seg.height === LIVE_H1),
        'restored live timeline must include block #950001 that arrived during history mode');
    const restoredLive = s.timeline[s.timeline.length - 1];
    assert(restoredLive._blipsByTxid.has(liveDuringHistoryTx),
        'restored live timeline must preserve live tx arrival from while browsing history');
    const maxHistoryExitH = Math.max(...Object.values(restoredLive._colHeights));
    assert(maxHistoryExitH <= 165,
        `returning from history mode stacked a skyscraper tower! maxColH=${Math.round(maxHistoryExitH)}px`);
    assert(modeEvents.length === 2 && modeEvents[1] === '',
        '__hbHistoryModeChange should fire with meta on enter and empty string on exit');
    window.__hbHistoryModeChange = undefined;
});

// ── Scenario 11: Rhythm strip post-block scrub & zero innerHTML ───────────────
runScenario('11. Rhythm strip dynamic slider sync, post-block scrub & zero innerHTML', () => {
    let innerHTMLCount = 0;
    const makeSafeDomEl = (tag) => {
        const el = {
            tagName: tag,
            type: '',
            value: '',
            max: '',
            style: {},
            children: [],
            _text: '',
            width: 0,
            height: 0,
            oninput: null,
            addEventListener: noop,
            getBoundingClientRect: () => ({ width: 600, height: 80, left: 0, top: 0 }),
            getContext: () => ({
                scale: noop, fillRect: noop, beginPath: noop, moveTo: noop, lineTo: noop,
                stroke: noop, fillText: noop,
                createLinearGradient: () => ({ addColorStop: noop }),
            }),
            appendChild(c) { this.children.push(c); return c; },
            removeChild(c) {
                const idx = this.children.indexOf(c);
                if (idx >= 0) this.children.splice(idx, 1);
                return c;
            },
        };
        Object.defineProperty(el, 'firstChild', { get() { return this.children[0] || null; } });
        Object.defineProperty(el, 'textContent', {
            get() { return this._text + this.children.map((c) => c.textContent).join(' '); },
            set(v) { this._text = String(v); },
        });
        Object.defineProperty(el, 'innerHTML', {
            get() { return ''; },
            set() { innerHTMLCount += 1; throw new Error('innerHTML assigned in rhythm strip!'); },
        });
        return el;
    };

    const canvasEl = makeSafeDomEl('canvas');
    const sliderEl = makeSafeDomEl('input');
    const detailEl = makeSafeDomEl('div');
    const origGetById = document.getElementById;
    const origCreateEl = document.createElement;
    document.getElementById = (id) => {
        if (id === 'rhythm-strip-canvas') return canvasEl;
        if (id === 'rhythm-strip-slider') return sliderEl;
        if (id === 'rhythm-strip-detail') return detailEl;
        return null;
    };
    document.createElement = makeSafeDomEl;

    try {
        const baseTs = 1760000000;
        const initialBlocks = [
            { height: 950001, timestamp: baseTs, tx_count: 1000, total_fees: 2000000, weight: 3000000, inter_block_seconds: 600 },
            { height: 950002, timestamp: baseTs, tx_count: 500, total_fees: 1000000, weight: 2000000, inter_block_seconds: 0 },
            { height: 950003, timestamp: baseTs + 3600, tx_count: 3500, total_fees: 9000000, weight: 3990000, inter_block_seconds: 3600 },
            { height: 950004, timestamp: baseTs + 4200, tx_count: 2000, total_fees: 4000000, weight: 3500000, inter_block_seconds: 600 },
            { height: 950005, timestamp: baseTs + 4800, tx_count: 1800, total_fees: 3000000, weight: 3200000, inter_block_seconds: 600 },
        ];
        vitals.renderRhythmStrip('rhythm-strip-canvas', JSON.stringify(initialBlocks));
        assert(canvasEl._rhythmBlocks.length === 5, 'canvas._rhythmBlocks should store 5 initial blocks');
        assert(sliderEl.max === '4' && sliderEl.value === '4',
            `expected slider max=4, value=4, got max=${sliderEl.max}, value=${sliderEl.value}`);

        // New block #950006 arrives -> re-render rhythm strip
        const updatedBlocks = initialBlocks.concat([{
            height: 950006, timestamp: baseTs + 5400, tx_count: 2200, total_fees: 5000000, weight: 3700000, inter_block_seconds: 600,
        }]);
        vitals.renderRhythmStrip('rhythm-strip-canvas', JSON.stringify(updatedBlocks));
        assert(canvasEl._rhythmBlocks.length === 6, 'canvas._rhythmBlocks should update to 6 blocks');
        assert(sliderEl.max === '5' && sliderEl.value === '5',
            `slider at previous max must advance to new max 5, got max=${sliderEl.max}, value=${sliderEl.value}`);
        assert(detailEl.textContent.includes('950,006') || detailEl.textContent.includes('950006'),
            `detail should show new tip #950006, got: ${detailEl.textContent}`);

        // Scrub to index 1 (0s wait block), then back to index 5 (new block #950006)
        sliderEl.value = '1';
        sliderEl.oninput();
        assert(detailEl.textContent.includes('0s wait'),
            `0s wait block must show "0s wait" (not fake 10m), got: ${detailEl.textContent}`);
        assert(canvasEl._rhythmBlocks.length === 6,
            'scrubbing must NOT revert canvas._rhythmBlocks to stale initial 5-block closure');

        sliderEl.value = '5';
        sliderEl.oninput();
        assert(detailEl.textContent.includes('950,006') || detailEl.textContent.includes('950006'),
            `scrubbing back to index 5 must still display #950006, got: ${detailEl.textContent}`);
        assert(innerHTMLCount === 0, 'innerHTML must NEVER be assigned in rhythm strip');
    } finally {
        document.getElementById = origGetById;
        document.createElement = origCreateEl;
    }
});

// ── Scenario 12: Large (>5000) mempool survives pending_block draw + resync & burst caps ──
runScenario('12. Large (>5k) mempool never prunes during pending_block, stays flush, and avoids resync/burst skyscrapers', () => {
    const nowSec = Math.floor(Date.now() / 1000);
    resetState({
        virtualX: 500, zoom: 2.0, width: 1440, height: 800,
        autoFollow: true, _revealEnabled: true, lastBlockTime: nowSec - 1400,
    });
    const s = state.getState();
    const prevBlk = timeline.createBlockSegment({
        hash: 'a0'.repeat(32), height: 977402, timestamp: nowSec - 1400,
        tx_count: 3000, total_fees: 2000000, size: 1400000, weight: 3900000, inter_block_seconds: 300,
    }, 0);
    const openSeg = timeline.createFlatlineSegment(prevBlk.x_end, null);
    s.timeline = [prevBlk, openSeg];
    s.virtualX = prevBlk.x_end;

    // 12a. Install 6,500 txs (> MAX_BLIPS_PER_SEGMENT = 5000)
    const txTuples = [];
    for (let i = 0; i < 6500; i++) {
        txTuples.push([txid(300000 + i), nowSec - 6500 + i, 200, 200, 50000, 0, '', i + 1]);
    }
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions: txTuples, recent_blocks: [], pending_blocks: [],
        recent_confirmations: [], coverage: { status: 'complete' },
    }), false);
    drainRafs();
    assert(openSeg.blips.length === 6500, `expected 6500 bricks before block, got ${openSeg.blips.length}`);

    // 12b. block_observed seals openSeg -> drawFlatlineSegment runs on sealed pending segment
    const H1 = 'f1'.repeat(32);
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: H1, observed_at: nowSec }));
    assert(openSeg.x_end !== null && openSeg._pendingForHash === H1, 'openSeg should be sealed for H1');

    const fakeCtx = new Proxy({}, {
        get(_, k) {
            if (k === 'createLinearGradient') return () => ({ addColorStop: noop });
            return noop;
        },
    });
    render.drawFlatlineSegment(
        fakeCtx, openSeg, openSeg.x_end, s.virtualX - 600, s.virtualX + 200, 500, '#00e676', false, nowSec, 1400,
    );
    assert(openSeg.blips.length === 6500,
        `pending_block flatline MUST NOT prune bricks! expected 6500, got ${openSeg.blips.length}`);

    // 12c. Post-block arrival lands 60px right of pending slot before block_details
    s.virtualX += 60;
    const postId = txid(399999);
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: postId, first_seen: nowSec + 10 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [postId, nowSec + 10, 800, 200, 50000, 0, '', 99999] }));
    sse.flushTxBatch(Date.now() + 500);

    // 12d. block_inclusion (200 mined) + block_details -> reveal ends
    const minedIds = txTuples.slice(-200).map((t) => t[0]);
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: H1, height: 977403, confirmed_txids: minedIds }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: H1, height: 977403, timestamp: nowSec,
        tx_count: 3500, total_fees: 4000000, size: 1500000, weight: 3990000, observed_at: nowSec,
    }));
    window._hbEndReveal(true);
    drainRafs();

    const blk = s.timeline.find((seg) => seg.type === 'block' && seg.height === 977403);
    const liveSeg = s.timeline[s.timeline.length - 1];
    assert(liveSeg._blipsByTxid.size === 6301,
        `expected 6300 carried + 1 post-block = 6301 bricks, got ${liveSeg._blipsByTxid.size}`);
    const cols = Object.keys(liveSeg._colHeights).map(Number).sort((a, b) => a - b);
    assert(cols[0] - blk.x_end <= 5,
        `expected carried bricks flush right after block spike (<=5px), got gap=${cols[0] - blk.x_end}`);

    // 12e. Large resync recovery (2,000 new txs) must NOT pile into flat-topped skyscrapers (>190px)
    const extraTuples = txTuples.slice(0, 6300).concat([[postId, nowSec + 10, 800, 200, 50000, 0, '', 99999]]);
    for (let j = 0; j < 2000; j++) {
        extraTuples.push([txid(400000 + j), nowSec - 2000 + j, 200, 200, 50000, 0, '', 200000 + j]);
    }
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions: extraTuples, recent_blocks: [], pending_blocks: [],
        recent_confirmations: [], coverage: { status: 'complete' },
    }), true);
    const maxResyncH = Math.max(...Object.values(liveSeg._colHeights));
    assert(maxResyncH <= 190, `large resync built skyscraper wall! maxColH=${Math.round(maxResyncH)}px`);

    // 12f. Single-frame live burst (150 txs after quiet stretch) must stay capped around softMaxStack
    s.virtualX += 150;
    for (let k = 0; k < 150; k++) {
        const bid = txid(500000 + k);
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: bid, first_seen: nowSec + 30 }));
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [bid, nowSec + 30, 600, 200, 50000, 0, '', 300000 + k] }));
    }
    sse.flushTxBatch(Date.now() + 500);
    const maxBurstH = Math.max(...Object.values(liveSeg._colHeights));
    assert(maxBurstH <= 190, `live burst built needle spire! maxColH=${Math.round(maxBurstH)}px`);

    // 12g. 34x zoomed-in inspecting viewer on the live mempool is never stranded in empty void past virtualX
    s.autoFollow = false;
    s.paused = true;
    s.zoom = 34.2;
    s.viewOffset = s.virtualX - (s.width * 0.5) / s.zoom;
    const H2 = 'f2'.repeat(32);
    const minedBig = liveSeg.blips.slice(-1500).map((b) => b.txid).filter(Boolean);
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: H2, observed_at: nowSec + 60 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: H2, height: 977404, confirmed_txids: minedBig }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: H2, height: 977404, timestamp: nowSec + 60,
        tx_count: 5000, total_fees: 2000000, size: 1500000, weight: 3990000,
    }));
    drainRafs();
    assert(s.zoom === 34.2 && s.paused === true && s.autoFollow === false,
        'zoomed-in inspecting viewer must keep exact zoom/paused/inspecting state');
    assert(s.viewOffset <= s.virtualX,
        `34x zoomed-in viewer stranded in empty void past virtualX! viewOffset=${s.viewOffset}, virtualX=${s.virtualX}`);
});

// ── Scenario 13: Bidirectional infinite lazy-loading + 60-FPS zoomed-out LOD & binary search ──
runScenario('13. Bidirectional lazy-load (negative-X live prepend, genesis #0, history append, binary search, zoomed-out LOD)', () => {
    const nowSec = Math.floor(Date.now() / 1000);
    const makeBlk = (h, inter = 600) => ({
        hash: ('b' + h.toString(16)).padEnd(64, '0'),
        height: h,
        timestamp: 1700000000 + h * 600,
        tx_count: 2500,
        total_fees: 25000000,
        size: 1400000,
        weight: 3900000,
        inter_block_seconds: inter,
    });

    // 13a. LIVE timeline: prepend 250 older blocks leftward into negative virtual X
    resetState({
        virtualX: 0,
        zoom: 1.5,
        viewOffset: 120,
        autoFollow: false,
        _viewMode: 'live',
        _fetchGen: 0,
        _histFetchLeft: false,
        _histFetchRight: false,
        _histMinReached: false,
        _histMaxReached: false,
        _histTipHeight: 0,
        _histFetchCooldownUntil: 0,
        _revealEnabled: false,
    });
    window.pushHeartbeatBlocks(JSON.stringify([
        makeBlk(250, 600),
        makeBlk(251, 480),
    ]), true);
    installTxs(20, false);

    const s = state.getState();
    const existingBlk = s.timeline.find((seg) => seg.type === 'block' && seg.height === 250);
    const liveTail = s.timeline[s.timeline.length - 1];
    assert(existingBlk && liveTail && liveTail.type === 'flatline' && liveTail.x_end === null,
        'live timeline should have block #250 and open live flatline');
    const sampleBlip = liveTail.blips[0];
    assert(sampleBlip, 'live flatline should contain a live blip');

    const viewOffsetBefore = s.viewOffset;
    const virtualXBefore = s.virtualX;
    const existingBlkStartBefore = existingBlk.x_start;
    const existingBlkEndBefore = existingBlk.x_end;
    const sampleBlipXBefore = sampleBlip.x;

    // Prepend 250 older blocks (#0 .. #249) including genesis #0
    const older250 = [];
    for (let h = 0; h < 250; h++) {
        older250.push(makeBlk(h, h === 0 ? null : 540));
    }
    const prepended = window._hbPrependHistoricalBlocks(s, older250);
    assert(prepended === 250, `expected 250 prepended blocks, got ${prepended}`);

    // Zero shift to camera, virtualX, existing block segments, or live blips
    assert(s.viewOffset === viewOffsetBefore,
        `viewOffset shifted on prepend: ${viewOffsetBefore} -> ${s.viewOffset}`);
    assert(s.virtualX === virtualXBefore,
        `virtualX shifted on prepend: ${virtualXBefore} -> ${s.virtualX}`);
    assert(existingBlk.x_start === existingBlkStartBefore && existingBlk.x_end === existingBlkEndBefore,
        `existing block #250 coordinates mutated on prepend`);
    assert(sampleBlip.x === sampleBlipXBefore,
        `live blip.x shifted on prepend: ${sampleBlipXBefore} -> ${sampleBlip.x}`);

    // Genesis #0 must set _histMinReached = true and have NO preceding flatline
    assert(s._histMinReached === true, 'prepending genesis #0 must set _histMinReached = true');
    assert(s.timeline[0].type === 'block' && s.timeline[0].height === 0,
        `genesis #0 must be timeline[0] with no preceding flatline, got type=${s.timeline[0].type} height=${s.timeline[0].height}`);
    assert(s.timeline[0].x_start < 0, `expected genesis #0 in negative virtual X, got ${s.timeline[0].x_start}`);

    // Strict contiguity across all segments: seg[i].x_end === seg[i+1].x_start
    for (let i = 0; i < s.timeline.length - 1; i++) {
        const a = s.timeline[i];
        const b = s.timeline[i + 1];
        assert(Math.abs(a.x_end - b.x_start) < 1e-9,
            `discontinuity at index ${i}: seg[${i}].x_end=${a.x_end} !== seg[${i + 1}].x_start=${b.x_start}`);
    }

    // Binary search blockAtVirtualX & flatlineAtVirtualX accurately locate negative-X and positive-X segments
    const negBlkTarget = s.timeline.find((seg) => seg.type === 'block' && seg.height === 42);
    assert(negBlkTarget && negBlkTarget.x_end < 0, 'block #42 should sit in negative virtual X');
    const foundNeg = blips.blockAtVirtualX((negBlkTarget.x_start + negBlkTarget.x_end) / 2);
    assert(foundNeg === negBlkTarget, 'blockAtVirtualX failed to find negative-X prepended block #42');
    const foundGenesis = blips.blockAtVirtualX((s.timeline[0].x_start + s.timeline[0].x_end) / 2);
    assert(foundGenesis === s.timeline[0], 'blockAtVirtualX failed to find genesis #0');
    const foundPos = blips.blockAtVirtualX((existingBlk.x_start + existingBlk.x_end) / 2);
    assert(foundPos === existingBlk, 'blockAtVirtualX failed to find positive-X block #250');

    const negFlat = s.timeline.find((seg) => seg.type === 'flatline' && seg.x_end < 0);
    const foundNegFlat = blips.flatlineAtVirtualX((negFlat.x_start + negFlat.x_end) / 2);
    assert(foundNegFlat && foundNegFlat.segment === negFlat && foundNegFlat.prevBlock && foundNegFlat.nextBlock,
        'flatlineAtVirtualX failed to find negative-X flatline with surrounding blocks');

    // 13b. Historical View: append 250 newer blocks rightward
    const modeEvents = [];
    window.__hbHistoryModeChange = (metaJson) => { modeEvents.push(metaJson); };
    try {
        const seedHist = [];
        for (let h = 500000; h < 500020; h++) seedHist.push(makeBlk(h, 600));
        const entered = window.hbV2EnterHistoryMode(JSON.stringify({
            target_height: 500010,
            target_hash: seedHist[10].hash,
            tip_height: 500500,
            blocks: seedHist,
        }));
        assert(entered === true, 'hbV2EnterHistoryMode should succeed');
        const histViewOffsetBefore = s.viewOffset;

        const newer250 = [];
        for (let h = 500020; h < 500270; h++) newer250.push(makeBlk(h, 620));
        const appended = window._hbAppendHistoricalBlocks(s, newer250);
        assert(appended === 250, `expected 250 appended blocks, got ${appended}`);
        assert(s.viewOffset === histViewOffsetBefore, 'appending rightward must not jump viewOffset');
        assert(s._historyMeta.to_height === 500269,
            `expected _historyMeta.to_height=500269, got ${s._historyMeta.to_height}`);
        assert(modeEvents.length >= 2, '__hbHistoryModeChange must fire on append');
        const lastMeta = JSON.parse(modeEvents[modeEvents.length - 1]);
        assert(lastMeta.to_height === 500269,
            `__hbHistoryModeChange payload to_height should be 500269, got ${lastMeta.to_height}`);

        // Strict contiguity across entire historical timeline after append
        for (let i = 0; i < s.timeline.length - 1; i++) {
            const a = s.timeline[i];
            const b = s.timeline[i + 1];
            assert(Math.abs(a.x_end - b.x_start) < 1e-9,
                `history discontinuity at index ${i}: ${a.x_end} !== ${b.x_start}`);
        }
    } finally {
        window.__hbHistoryModeChange = undefined;
    }

    // 13c. Zoomed-out drawBlockSegment (lowDetail=true, skipGlow=true): <= 4 vertices & zero shadowBlur
    s.zoom = 0.08;
    s.viewOffset = 0;
    s.hoveredBlock = null;
    const lodSeg = timeline.createBlockSegment(makeBlk(840000, 600), 100);
    assert(typeof lodSeg._minY === 'number' && lodSeg._minY < 0 && typeof lodSeg._minYIdx === 'number',
        'createBlockSegment must pre-compute _minY and _minYIdx');
    assert(typeof lodSeg._maxY === 'number' && lodSeg._maxY > 0 && typeof lodSeg._maxYIdx === 'number',
        'createBlockSegment must pre-compute _maxY and _maxYIdx');

    let moveToCount = 0;
    let lineToCount = 0;
    let maxShadowBlur = 0;
    let recordedLineWidth = 0;
    let gradientCount = 0;
    let fillRectCount = 0;
    const spyCtx = {
        _shadowBlur: 0,
        _lineWidth: 0,
        set shadowBlur(v) { this._shadowBlur = v; if (v > maxShadowBlur) maxShadowBlur = v; },
        get shadowBlur() { return this._shadowBlur; },
        set lineWidth(v) { this._lineWidth = v; recordedLineWidth = v; },
        get lineWidth() { return this._lineWidth; },
        beginPath: noop,
        moveTo() { moveToCount += 1; },
        lineTo() { lineToCount += 1; },
        stroke: noop,
        fillRect() { fillRectCount += 1; },
        createLinearGradient() { gradientCount += 1; return { addColorStop: noop }; },
        save: noop,
        restore: noop,
    };

    render.drawBlockSegment(spyCtx, lodSeg, 0, 220, '#00e676', nowSec, true, true);
    const totalVertices = moveToCount + lineToCount;
    assert(totalVertices <= 4 && moveToCount === 1 && lineToCount === 3,
        `lowDetail drawBlockSegment should draw 4-point polyline (1 moveTo + 3 lineTo), got moveTo=${moveToCount}, lineTo=${lineToCount}`);
    assert(maxShadowBlur === 0,
        `lowDetail drawBlockSegment must set zero shadowBlur, got maxShadowBlur=${maxShadowBlur}`);
    assert(recordedLineWidth === 1.25,
        `lowDetail drawBlockSegment must use lineWidth 1.25, got ${recordedLineWidth}`);

    // Narrow zoomed-out historical flatline: zero gradient allocation, zero shadowBlur, zero floor glow fillRects
    const narrowFlat = timeline.createFlatlineSegment(100, 200); // 100 * 0.08 = 8px (< 18px)
    narrowFlat.inter_block_seconds = 1800;
    narrowFlat.color = timeline.waitColor(1800);
    render.drawFlatlineSegment(spyCtx, narrowFlat, narrowFlat.x_end, 0, 5000, 220, narrowFlat.color, false, nowSec, 1800, true);
    assert(gradientCount === 0, `narrow zoomed-out flatline must not allocate linear gradient, got ${gradientCount}`);
    assert(maxShadowBlur === 0, `skipGlow historical flatline must have zero shadowBlur, got ${maxShadowBlur}`);
    assert(fillRectCount === 0, `skipGlow historical flatline must skip floor-glow fillRects, got ${fillRectCount}`);
});

// ── Scenario 14: Overnight soak (sleep/wake burst, same-second needles, no pending cyan box) ──
runScenario('14. Overnight wake-up bursts, same-second bot pulses, and clean pending_block cue', () => {
    const nowSec = Math.floor(Date.now() / 1000);
    resetState({
        virtualX: 1000, zoom: 1.0, width: 1440, height: 800,
        autoFollow: true, _revealEnabled: true, lastBlockTime: nowSec - 1100,
    });
    const s = state.getState();
    const liveSeg = s.timeline[s.timeline.length - 1];

    // 14a. 1,200-tx overnight wake-up burst (including >5m-old ageSec > 300 txs!) hitting flushTxBatch
    for (let i = 0; i < 1200; i++) {
        const id = txid(700000 + i);
        const ts = nowSec - (i < 600 ? 600 : 10); // half >300s old, half fresh
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: id, first_seen: ts }));
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [id, ts, 200, 200, 50000, 0, '', i + 1] }));
    }
    sse.flushTxBatch(Date.now() + 500);
    const maxWakeH = Math.max(...Object.values(liveSeg._colHeights));
    assert(maxWakeH <= 195,
        `overnight wake-up burst built skyscraper! maxColH=${Math.round(maxWakeH)}px`);

    // 14b. 200-tx same-second blue bot pulse in drainHiddenTxsAcrossGap -> no 400px 1-column needle
    const gapStart = s.virtualX;
    s.virtualX += 300;
    document.hidden = true;
    const pulseTs = nowSec - 30;
    for (let j = 0; j < 200; j++) {
        const pid = txid(710000 + j);
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: pid, first_seen: pulseTs }));
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [pid, pulseTs, 100, 200, 20000, 0, '', j + 1] }));
    }
    document.hidden = false;
    sse.drainHiddenTxsAcrossGap(gapStart, s.virtualX - 5, nowSec - 60, nowSec);
    const maxPulseH = Math.max(...Object.values(liveSeg._colHeights));
    assert(maxPulseH <= 195,
        `same-second blue bot pulse built tall needle! maxColH=${Math.round(maxPulseH)}px`);

    // 14c. Small (80-tx) same-second isResync -> no 1-column needle
    const baseTxs = Array.from(liveSeg._blipsByTxid.values()).map((b) => [
        b.txid, b.firstSeen || nowSec, b.fee ?? 200, b.vsize ?? 200, b.value ?? 50000, 0, '', 0,
    ]);
    for (let k = 0; k < 80; k++) {
        baseTxs.push([txid(720000 + k), nowSec - 20, 100, 200, 20000, 0, '', 0]);
    }
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions: baseTxs, recent_blocks: [], pending_blocks: [],
        recent_confirmations: [], coverage: { status: 'complete' },
    }), true);
    const maxSmallResyncH = Math.max(...Object.values(liveSeg._colHeights));
    assert(maxSmallResyncH <= 195,
        `small same-second isResync built tall needle! maxColH=${Math.round(maxSmallResyncH)}px`);

    // 14d. drawPendingBlockSegment draws ZERO translucent fillRect boxes (< 2s AND >= 2s)
    let boxFills = 0;
    const pCtx = {
        save: noop, restore: noop, beginPath: noop, moveTo: noop, lineTo: noop,
        stroke: noop, arc: noop, fill: noop, fillText: noop,
        fillRect() { boxFills += 1; },
    };
    const fastPending = { x_start: 100, x_end: 176.5, receivedWallMs: Date.now() - 500, color: '#38bdf8' };
    const slowPending = { x_start: 100, x_end: 176.5, receivedWallMs: Date.now() - 5000, color: '#38bdf8' };
    render.drawPendingBlockSegment(pCtx, fastPending, 300, nowSec);
    render.drawPendingBlockSegment(pCtx, slowPending, 300, nowSec);
    assert(boxFills === 0, `drawPendingBlockSegment must NOT draw a cyan box fillRect, got ${boxFills}`);
});

// ── Scenario 15: Phase 4 probes P1, P1b, P2 + resync orphan cleanup & camera invariance ──
runScenario('15. Snapshot pending_blocks & back-to-back inclusion (P1/P1b), hidden-tab reorg (P2), orphan cleanup & pending tooltips', () => {
    const nowSec = Math.floor(Date.now() / 1000);
    const activeBrick = (s, id) => {
        for (const seg of s.timeline) {
            if (seg.type !== 'flatline' || !seg.blips) continue;
            for (const b of seg.blips) {
                if (b.txid === id && !b._removed && b.fadeStart === 0) return seg;
            }
        }
        return null;
    };

    // 15a (P1): Snapshot carrying non-empty pending_blocks -> block_inclusion -> block_details
    const H1 = 'c1'.repeat(32);
    resetState({ virtualX: 300, zoom: 1.2, autoFollow: false, _revealEnabled: false });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'c0'.repeat(32), height: 900000, timestamp: nowSec - 900,
        tx_count: 1000, total_fees: 2000000, size: 1e6, weight: 3e6, inter_block_seconds: 600,
    }]), true);
    const snapTxs = [];
    for (let i = 0; i < 50; i++) snapTxs.push([txid(i), nowSec - 800 + i, 1000, 200, 500000, 0, '', 0]);
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions: snapTxs,
        recent_blocks: [],
        pending_blocks: [{ hash: H1, observed_at: nowSec - 5, height: null }],
        coverage: { status: 'complete' },
    }), false);
    drainRafs();
    let s = state.getState();
    const slotIdx = s.timeline.findIndex((g) => g.type === 'pending_block' && g.hash === H1);
    assert(slotIdx >= 0, 'P1: pending_block slot reserved from snapshot');

    // Check pendingNext on preceding flatline + both-edge offscreen check
    const preFlat = s.timeline[slotIdx - 1];
    const flatInfo = blips.flatlineAtVirtualX((preFlat.x_start + preFlat.x_end) / 2);
    assert(flatInfo && flatInfo.pendingNext && flatInfo.pendingNext.hash === H1,
        'flatlineAtVirtualX must attach pendingNext when followed by a pending_block');
    const pendSeg = s.timeline[slotIdx];
    s.viewOffset = pendSeg.x_start - 5000;
    assert(window.hbV2IsPendingSlotOffscreen(H1) === true,
        'hbV2IsPendingSlotOffscreen must return true when pending slot is offscreen to the right');
    s.viewOffset = pendSeg.x_end + 100;
    assert(window.hbV2IsPendingSlotOffscreen(H1) === true,
        'hbV2IsPendingSlotOffscreen must return true when pending slot is offscreen to the left');

    const mined1 = [txid(0), txid(1), txid(2), txid(3), txid(4)];
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: H1, height: 900001, confirmed_txids: mined1 }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: H1, height: 900001, timestamp: nowSec,
        tx_count: 5, total_fees: 100000, size: 1e5, weight: 4e5,
    }));
    drainRafs(200);
    assert(mined1.every((m) => !activeBrick(state.getState(), m)),
        'P1: all mined bricks must be absorbed after snapshot pending_block -> inclusion -> details');

    // 15b (P1b): Two consecutive slots; details A, then inclusion B before details B
    const A = 'c3'.repeat(32), B = 'c4'.repeat(32);
    resetState({ virtualX: 300, zoom: 1.2, autoFollow: false, _revealEnabled: false });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'c2'.repeat(32), height: 930000, timestamp: nowSec - 900,
        tx_count: 1000, total_fees: 2000000, size: 1e6, weight: 3e6, inter_block_seconds: 600,
    }]), true);
    installTxs(50, false);
    const det = (hash, h, ts) => JSON.stringify({
        type: 'block_details', hash, height: h, timestamp: ts,
        tx_count: 5, total_fees: 1e5, size: 1e5, weight: 4e5,
    });
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: A, observed_at: nowSec - 30 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: B, observed_at: nowSec - 10 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: A, height: 930001, confirmed_txids: [txid(10)] }));
    sse.hbV2ApplyEffect(det(A, 930001, nowSec - 30));
    drainRafs(200);
    const minedB = [txid(0), txid(1), txid(2), txid(3), txid(4)];
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: B, height: 930002, confirmed_txids: minedB }));
    sse.hbV2ApplyEffect(det(B, 930002, nowSec - 10));
    drainRafs(200);
    assert(minedB.every((m) => !activeBrick(state.getState(), m)),
        'P1b: all mined bricks for B must be absorbed after two consecutive slots');

    // 15c (P2): Hidden-tab reorg (details A queued -> block_disconnected A -> A' replaces it)
    const A1 = 'd1'.repeat(32), A2 = 'd2'.repeat(32);
    resetState({ virtualX: 300, zoom: 1.2, autoFollow: true, _revealEnabled: false });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'd0'.repeat(32), height: 910000, timestamp: nowSec - 900,
        tx_count: 1000, total_fees: 2000000, size: 1e6, weight: 3e6, inter_block_seconds: 600,
    }]), true);
    installTxs(20, false);
    document.hidden = true;
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: A1, observed_at: nowSec - 120 }));
    sse.hbV2ApplyEffect(det(A1, 910001, nowSec - 120));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_disconnected', hash: A1 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: A2, observed_at: nowSec - 60 }));
    sse.hbV2ApplyEffect(det(A2, 910001, nowSec - 60));
    document.hidden = false;
    window._hbRecoverTimeline(130);
    drainRafs(200);
    s = state.getState();
    const segs910001 = s.timeline.filter((g) => g.type === 'block' && g.height === 910001);
    assert(segs910001.length === 1 && segs910001[0].hash === A2,
        `P2: expected exactly 1 spike (A') at 910001, got ${segs910001.length}`);

    // 15d: Orphan pending_block cleanup on resync & camera invariance when inspecting left of disconnect
    const ORPH = 'd9'.repeat(32);
    s.autoFollow = false;
    const inspectLeftOffset = 5;
    s.viewOffset = inspectLeftOffset;
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: ORPH, observed_at: nowSec - 2 }));
    assert(s.timeline.some((g) => g.type === 'pending_block' && g.hash === ORPH), 'orphan slot present before resync');
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions: [], recent_blocks: [], pending_blocks: [], coverage: { status: 'complete' },
    }), true);
    assert(!s.timeline.some((g) => g.type === 'pending_block' && g.hash === ORPH),
        'resync snapshot must remove orphaned pending_block not in pending_blocks or recent_blocks');
    assert(s.viewOffset === inspectLeftOffset,
        `inspecting camera looking left of removed slot must not shift: ${inspectLeftOffset} -> ${s.viewOffset}`);
});

// ── Scenario 16: Phase 4 probes P3 & P4 + rhythm strip selected-height tracking ──
runScenario('16. History exit pre-block txs (P3), LIVE 12k-seg cap (P4), and rhythm strip height tracking', () => {
    const nowSec = Math.floor(Date.now() / 1000);

    // 16a (P3): Exit history after a block arrived during history with 400 pre-block buffered txs
    resetState({ virtualX: 300, zoom: 1.9, autoFollow: true, _revealEnabled: false });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'e0'.repeat(32), height: 950000, timestamp: nowSec - 900,
        tx_count: 1000, total_fees: 2000000, size: 1e6, weight: 3e6, inter_block_seconds: 600,
    }]), true);
    installTxs(10, false);
    const hist = [];
    for (let i = -5; i <= 5; i++) {
        hist.push({
            hash: ('f' + (840000 + i).toString(16)).padEnd(64, '0'),
            height: 840000 + i,
            timestamp: 1713571767 + i * 600,
            tx_count: 3000, total_fees: 5e7, size: 1.6e6, weight: 3.99e6, inter_block_seconds: 600,
        });
    }
    assert(window.hbV2EnterHistoryMode(JSON.stringify({
        target_height: 840000, tip_height: 950000, blocks: hist,
    })), 'P3: entered history mode');
    let s = state.getState();
    s._liveBackup.enteredAt = nowSec - 240;
    for (let i = 0; i < 400; i++) {
        const id = txid(70000 + i);
        const ts = nowSec - 230 + Math.floor(i / 4);
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: id, first_seen: ts }));
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [id, ts, 900, 200, 50000, 0, '', i + 1] }));
    }
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: 'e1'.repeat(32), observed_at: nowSec - 60 }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: 'e1'.repeat(32), height: 950001, timestamp: nowSec - 60,
        tx_count: 10, total_fees: 100000, size: 1e5, weight: 4e5,
    }));
    drainRafs(200);
    window.hbV2ExitHistoryMode();
    drainRafs(200);
    const liveSeg = s.timeline[s.timeline.length - 1];
    const maxH = Math.max(0, ...Object.values(liveSeg._colHeights || {}));
    assert(maxH <= 165, `P3: skyscraper on Return to LIVE: maxH=${Math.round(maxH)}px`);

    // 16b (P4): LIVE-mode 30 leftward pages respect the 12,000-segment cap cleanly
    resetState({ virtualX: 300, zoom: 1, autoFollow: false, _revealEnabled: false });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'aa'.repeat(32), height: 960000, timestamp: nowSec - 600,
        tx_count: 1000, total_fees: 2000000, size: 1e6, weight: 3e6, inter_block_seconds: 600,
    }]), true);
    s = state.getState();
    let top = 960000;
    for (let page = 0; page < 30; page++) {
        const blocks = [];
        for (let h = top - 500; h < top; h++) {
            blocks.push({
                hash: h.toString(16).padStart(64, '0'), height: h,
                timestamp: 1.7e9 + h * 600, tx_count: 10, total_fees: 1e5,
                size: 1e5, weight: 4e5, inter_block_seconds: 600,
            });
        }
        window._hbPrependHistoricalBlocks(blocks);
        top -= 500;
        const vl = s.timeline[0].x_start;
        s.viewOffset = vl;
        hbMain.trimFarHistoricalSegments(s, vl, vl + 1200);
    }
    assert(s.timeline.length <= 12000, `P4: LIVE timeline exceeded 12,000 cap (${s.timeline.length})`);
    assert(s._histMinReached === true, 'P4: _histMinReached must be set when LIVE hits the 12,000-segment cap');

    // 16c: Rhythm strip preserves selected block height when sliding window shifts
    const makeDom = (tag) => {
        const el = {
            tagName: tag, type: '', value: '', max: '', style: {}, children: [], _text: '',
            width: 0, height: 0, oninput: null, addEventListener: noop,
            getBoundingClientRect: () => ({ width: 600, height: 80, left: 0, top: 0 }),
            getContext: () => ({
                scale: noop, fillRect: noop, beginPath: noop, moveTo: noop, lineTo: noop,
                stroke: noop, fillText: noop, createLinearGradient: () => ({ addColorStop: noop }),
            }),
            appendChild(c) { this.children.push(c); return c; },
            removeChild(c) { const ix = this.children.indexOf(c); if (ix >= 0) this.children.splice(ix, 1); return c; },
        };
        Object.defineProperty(el, 'firstChild', { get() { return this.children[0] || null; } });
        Object.defineProperty(el, 'textContent', {
            get() { return this._text + this.children.map((c) => c.textContent).join(' '); },
            set(v) { this._text = String(v); },
        });
        return el;
    };
    const rCanvas = makeDom('canvas'), rSlider = makeDom('input'), rDetail = makeDom('div');
    const origGet = document.getElementById, origCreate = document.createElement;
    document.getElementById = (id) => (id === 'rhythm-strip-canvas' ? rCanvas : id === 'rhythm-strip-slider' ? rSlider : id === 'rhythm-strip-detail' ? rDetail : null);
    document.createElement = makeDom;
    try {
        const mk = (h) => ({ height: h, timestamp: 1760000000 + h * 600, tx_count: 1000, total_fees: 2e6, weight: 3e6, inter_block_seconds: 600 });
        vitals.renderRhythmStrip('rhythm-strip-canvas', [mk(100), mk(101), mk(102), mk(103), mk(104)]);
        rSlider.value = '2'; // lock onto block #102
        rSlider.oninput();
        assert(rCanvas._rhythmSelected.block.height === 102, 'selected block #102');
        // Slide window forward by 1 block -> #102 moves from index 2 to index 1
        vitals.renderRhythmStrip('rhythm-strip-canvas', [mk(101), mk(102), mk(103), mk(104), mk(105)]);
        assert(rSlider.value === '1', `expected slider tracked block #102 to index 1, got ${rSlider.value}`);
        assert(rCanvas._rhythmSelected.block.height === 102 && rDetail.textContent.includes('102'),
            `detail must stay locked on block #102, got ${rDetail.textContent}`);
    } finally {
        document.getElementById = origGet;
        document.createElement = origCreate;
    }
});

// ── Scenario 17: Phase 4 probes P5 (out-of-order back-to-back slots) & P6 (unknown interval) ──
runScenario('17. Out-of-order back-to-back slots (P5) & unknown interval distinct from 600s (P6)', () => {
    const nowSec = Math.floor(Date.now() / 1000);
    const A = 'b1'.repeat(32), B = 'b2'.repeat(32);
    const tPrev = nowSec - 1200, tA = nowSec - 300, tB = nowSec - 240;

    // 17a (P5): Back-to-back slots with details B arriving before details A
    resetState({ virtualX: 300, zoom: 1.2, autoFollow: false, _revealEnabled: false });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'b0'.repeat(32), height: 920000, timestamp: tPrev,
        tx_count: 1000, total_fees: 2000000, size: 1e6, weight: 3e6, inter_block_seconds: 600,
    }]), true);
    installTxs(10, false);
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: A, observed_at: tA + 2 }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_observed', hash: B, observed_at: tB + 2 }));
    const d = (hash, h, ts) => JSON.stringify({
        type: 'block_details', hash, height: h, timestamp: ts,
        tx_count: 10, total_fees: 1e5, size: 1e5, weight: 4e5,
    });
    // B arrives first while A is still pending_block -> B's interval starts as null (unknown)
    sse.hbV2ApplyEffect(d(B, 920002, tB));
    let s = state.getState();
    let segB = s.timeline.find((g) => g.type === 'block' && g.height === 920002);
    assert(segB && segB.inter_block_seconds === null,
        `P5: B before A must leave interval null, got ${segB ? segB.inter_block_seconds : 'missing'}`);
    assert(segB.color === timeline.UNKNOWN_INTERVAL_COLOR,
        `P5: B with unknown interval must use UNKNOWN_INTERVAL_COLOR, got ${segB.color}`);

    // Now A arrives -> A gets tA - tPrev (900s), and B gets backfilled to exact tB - tA (60s)
    sse.hbV2ApplyEffect(d(A, 920001, tA));
    drainRafs(200);
    s = state.getState();
    const segA = s.timeline.find((g) => g.type === 'block' && g.height === 920001);
    segB = s.timeline.find((g) => g.type === 'block' && g.height === 920002);
    assert(segA && segB && segA.x_start < segB.x_start, 'P5: A and B present in chronological order');
    assert(segA.inter_block_seconds === tA - tPrev, `P5: A interval ${segA.inter_block_seconds}s !== ${tA - tPrev}s`);
    assert(segB.inter_block_seconds === tB - tA, `P5: B interval ${segB.inter_block_seconds}s !== ${tB - tA}s`);
    const betweenFlat = s.timeline[s.timeline.indexOf(segA) + 1];
    assert(betweenFlat && betweenFlat.inter_block_seconds === tB - tA,
        'P5: intervening flatline between A and B must update to B exact interval');
    assert(Math.abs((betweenFlat.x_end - betweenFlat.x_start) - timeline.historyFlatlineWidth(tB - tA)) < 0.01,
        'P5: intervening compressed flatline width must resize to historyFlatlineWidth(tB - tA)');
    const recB = s.recentBlocks.find((rb) => rb.height === 920002);
    assert(recB && recB.inter_block_seconds === tB - tA,
        'P5: recentBlocks entry for B must update to exact tB - tA');
    // Strict contiguity preserved after re-compression
    for (let i = 0; i < s.timeline.length - 1; i++) {
        assert(Math.abs(s.timeline[i].x_end - s.timeline[i + 1].x_start) < 1e-6,
            `P5: timeline discontinuity at ${i}: ${s.timeline[i].x_end} !== ${s.timeline[i + 1].x_start}`);
    }

    // 17b (P6): Unknown inter_block_seconds === null strictly distinct from 600s and 0s
    const base = { tx_count: 2000, total_fees: 2e7, weight: 3.9e6 };
    const unk = timeline.generatePQRST(Object.assign({}, base, { inter_block_seconds: null }));
    const zero = timeline.generatePQRST(Object.assign({}, base, { inter_block_seconds: 0 }));
    const ten = timeline.generatePQRST(Object.assign({}, base, { inter_block_seconds: 600 }));
    assert(unk[17] === 4, `P6: unknown Q dip should be 4, got ${unk[17]}`);
    assert(JSON.stringify(unk) !== JSON.stringify(ten), 'P6: unknown PQRST must differ from 600s');
    assert(JSON.stringify(unk) !== JSON.stringify(zero), 'P6: unknown PQRST must differ from 0s');
    assert(timeline.historyFlatlineWidth(null) === 60 && timeline.historyFlatlineWidth(null) !== timeline.historyFlatlineWidth(600),
        `P6: unknown flatline width (${timeline.historyFlatlineWidth(null)}) must differ from 600s (${timeline.historyFlatlineWidth(600)})`);
    const unkSeg = timeline.createBlockSegment(Object.assign({ height: 1 }, base, { inter_block_seconds: null }), 0);
    assert(unkSeg.color === timeline.UNKNOWN_INTERVAL_COLOR && unkSeg.color !== timeline.waitColor(600),
        `P6: unknown block color (${unkSeg.color}) must be neutral slate (${timeline.UNKNOWN_INTERVAL_COLOR})`);
});

// ── Scenario 18: Cold snapshot with pending_block places pre-block bricks LEFT of slot & reveals absorption ──
runScenario('18. Cold snapshot during pending_block places pre-block bricks left of slot & shatters on reveal', () => {
    const nowSec = Math.floor(Date.now() / 1000);
    const H = 'd9'.repeat(32);
    const obsAt = nowSec - 12;
    resetState({ virtualX: 300, zoom: 1.6, autoFollow: true, _revealEnabled: true });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'd8'.repeat(32), height: 977532, timestamp: nowSec - 300,
        tx_count: 1000, total_fees: 2000000, size: 1e6, weight: 3e6, inter_block_seconds: 300,
    }]), true);

    const transactions = [];
    // 25 pre-block waiting txs (5 of which will be confirmed in H) + 3 post-block arrivals
    for (let i = 0; i < 25; i++) {
        transactions.push([txid(i), obsAt - 200 + i, 1000, 200, 500000, 0, '', 0]);
    }
    for (let i = 25; i < 28; i++) {
        transactions.push([txid(i), obsAt + 4 + (i - 25), 1000, 200, 500000, 0, '', 0]);
    }

    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions,
        recent_blocks: [],
        pending_blocks: [{ hash: H, observed_at: obsAt, height: null }],
        coverage: { status: 'complete' },
    }), false);
    drainRafs(200);

    const s = state.getState();
    const slotIdx = s.timeline.findIndex((g) => g.type === 'pending_block' && g.hash === H);
    assert(slotIdx > 0, `pending_block slot should exist at idx > 0, got ${slotIdx}`);
    const preSeg = s.timeline[slotIdx - 1];
    const postSeg = s.timeline[slotIdx + 1];
    assert(preSeg && preSeg.type === 'flatline' && preSeg._blipsByTxid.size === 25,
        `expected 25 pre-block bricks LEFT of pending_block, got ${preSeg ? preSeg._blipsByTxid.size : 'missing'}`);
    assert(postSeg && postSeg.type === 'flatline' && postSeg._blipsByTxid.size === 3,
        `expected 3 post-block bricks RIGHT of pending_block, got ${postSeg ? postSeg._blipsByTxid.size : 'missing'}`);

    // Now block_inclusion + block_details arrive for H in LIVE reveal mode
    const mined = [txid(0), txid(1), txid(2), txid(3), txid(4)];
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_inclusion', hash: H, height: 977533, confirmed_txids: mined,
    }));
    sse.hbV2ApplyEffect(JSON.stringify({
        type: 'block_details', hash: H, height: 977533, timestamp: obsAt,
        tx_count: 2973, total_fees: 2215595, size: 1663750, weight: 3992770,
    }));

    const rv = state.getState()._reveal;
    assert(rv && rv.height === 977533, 'LIVE reveal should arm for #977533');
    assert(rv.confirmed.length === 5, `expected 5 confirmed bricks armed for absorption animation, got ${rv.confirmed.length}`);
    assert(rv.leftovers.length === 20, `expected 20 leftover pre-block bricks to carry, got ${rv.leftovers.length}`);

    // Finish reveal -> 5 mined shattered/removed, 20 leftovers + 3 post-block = 23 active on live flatline
    window._hbEndReveal(true);
    drainRafs(200);
    const liveTail = state.getState().timeline[state.getState().timeline.length - 1];
    assert(liveTail._blipsByTxid.size === 23,
        `expected 23 active bricks after reveal (20 carried + 3 post-block), got ${liveTail._blipsByTxid.size}`);
    for (const m of mined) {
        assert(!liveTail._blipsByTxid.has(m), `mined tx ${m} must not survive on live flatline`);
    }
});

// ── Scenario 19: Claude 2026-11-26 review fixes (B2, B3, P1, P2, P3, audio) ──
runScenario('19. Reveal return trickle (B2), multi-pending/post-slot sweep (B3), skewed observed_at (P1), 20fps color lerp (P3)', () => {
    const nowSec = Math.floor(Date.now() / 1000);

    // 19a (B3 / P2a): Cold snapshot with 2 pending blocks partitions inter-pending txs cleanly
    const H1 = 'a1'.repeat(32), H2 = 'a2'.repeat(32);
    resetState({ virtualX: 300, zoom: 1.6, autoFollow: true, _revealEnabled: true });
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: 'a0'.repeat(32), height: 980000, timestamp: nowSec - 600, observed_at: nowSec - 600,
        tx_count: 100, total_fees: 1e6, size: 1e5, weight: 4e5, inter_block_seconds: 600,
    }]), true);
    const txs2 = [];
    for (let i = 0; i < 10; i++) txs2.push([txid(i), nowSec - 120 + i, 1000, 200, 500000, 0, '', 0]);
    txs2.push([txid(50), nowSec - 20, 1000, 200, 500000, 0, '', 0]); // between H1 (-30) and H2 (-10)
    txs2.push([txid(51), nowSec - 4, 1000, 200, 500000, 0, '', 0]);  // after H2 (-10)
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions: txs2,
        recent_blocks: [],
        pending_blocks: [
            { hash: H1, observed_at: nowSec - 30, height: null },
            { hash: H2, observed_at: nowSec - 10, height: null },
        ],
        coverage: { status: 'complete' },
    }), false);
    drainRafs(200);
    let s = state.getState();
    const p1Idx = s.timeline.findIndex((g) => g.type === 'pending_block' && g.hash === H1);
    const p2Idx = s.timeline.findIndex((g) => g.type === 'pending_block' && g.hash === H2);
    assert(p1Idx > 0 && p2Idx > p1Idx + 1, 'both pending slots reserved in order');
    assert(s.timeline[p1Idx - 1]._blipsByTxid.size === 10, '10 pre-H1 txs left of H1');
    assert(s.timeline[p1Idx + 1]._blipsByTxid.has(txid(50)), 'tx50 placed between H1 and H2');
    assert(s.timeline[p2Idx + 1]._blipsByTxid.has(txid(51)), 'tx51 placed right of H2');

    // Confirm H1 & H2 (including tx50 in H2) -> tx50 must not survive on live flatline
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: H1, height: 980001, confirmed_txids: [txid(0)] }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_details', hash: H1, height: 980001, timestamp: nowSec - 30, observed_at: nowSec - 30, tx_count: 1, total_fees: 1e5, size: 1e5, weight: 4e5 }));
    window._hbEndReveal(true);
    drainRafs(200);
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_inclusion', hash: H2, height: 980002, confirmed_txids: [txid(50)] }));
    sse.hbV2ApplyEffect(JSON.stringify({ type: 'block_details', hash: H2, height: 980002, timestamp: nowSec - 10, observed_at: nowSec - 10, tx_count: 1, total_fees: 1e5, size: 1e5, weight: 4e5 }));
    window._hbEndReveal(true);
    drainRafs(200);
    const tailAfter = state.getState().timeline[state.getState().timeline.length - 1];
    assert(!tailAfter._blipsByTxid.has(txid(50)), 'tx50 mined in H2 must not survive on live flatline');
    assert(tailAfter._blipsByTxid.has(txid(51)), 'post-H2 tx51 must remain active');

    // 19b (P1 / P3b): Future-skewed block header (+284s) anchors lastBlockTime on observed_at (-240s)
    resetState({ virtualX: 0, zoom: 1.6, autoFollow: true, _revealEnabled: false });
    sse.hbV2InstallSnapshot(JSON.stringify({
        transactions: [[txid(90), nowSec - 100, 1000, 200, 500000, 0, '', 0]],
        recent_blocks: [{
            hash: 'e1'.repeat(32), height: 977544,
            timestamp: nowSec + 44, // +284s ahead of observed_at!
            observed_at: nowSec - 240,
            tx_count: 7554, total_fees: 270992, size: 1641327, weight: 3993918,
        }],
        pending_blocks: [],
        coverage: { status: 'complete' },
    }), false);
    drainRafs(50);
    s = state.getState();
    assert(Math.abs(s.lastBlockTime - (nowSec - 240)) <= 2,
        `lastBlockTime should anchor on observed_at (${nowSec - 240}), got ${s.lastBlockTime}`);
    const liveSegSkew = s.timeline[s.timeline.length - 1];
    assert(s.virtualX - liveSegSkew.x_start >= 1000,
        `240s wait should produce >=1000px live flatline, got ${s.virtualX - liveSegSkew.x_start}`);

    // 19c (P3): Time-based live color blend at 20 fps produces zero RGB snap-backs
    let prevColor = '#00e676', curColor = '#00e676', tgtColor = '#00e676', lerpT = 1;
    let lastDrawnRgb = timeline.hexToRgb('#00e676');
    let snapBacks = 0;
    const dt = 0.05; // 20 fps
    for (let t = 300; t <= 600; t += dt) {
        const nextTgt = timeline.computeColor(t, 1, 20);
        const shown = lerpT < 1 ? timeline.lerpColor(prevColor, tgtColor, lerpT) : curColor;
        if (nextTgt !== tgtColor) {
            prevColor = shown;
            curColor = shown;
            tgtColor = nextTgt;
            lerpT = 0;
        } else if (lerpT < 1) {
            lerpT = Math.min(lerpT + dt / 0.85, 1);
            if (lerpT >= 1) { curColor = tgtColor; prevColor = tgtColor; }
        }
        const liveHex = lerpT < 1 ? timeline.lerpColor(prevColor, tgtColor, lerpT) : curColor;
        const rgb = timeline.hexToRgb(liveHex);
        const l1 = Math.abs(rgb[0] - lastDrawnRgb[0]) + Math.abs(rgb[1] - lastDrawnRgb[1]) + Math.abs(rgb[2] - lastDrawnRgb[2]);
        if (l1 > 20) snapBacks++;
        lastDrawnRgb = rgb;
    }
    assert(snapBacks === 0, `expected 0 RGB snap-backs at 20fps, got ${snapBacks}`);
});

// ── Scenario 20: Disconnected virtualX freeze, resync gap backfill & adaptive tip pacing ──
runScenario('20. Disconnected virtualX freeze, resync gap backfill & quiet recovery without empty voids', () => {
    const nowSec = Math.floor(Date.now() / 1000);

    // 20a. Disconnected/syncing freezes virtualX; live mode paces adaptively (1.1 px/s quiet -> 5.0 px/s full).
    resetState({ virtualX: 500, zoom: 1.0, height: 600, autoFollow: true, _v2ConnState: 'disconnected' });
    const s20 = state.getState();
    const seg20a = s20.timeline[s20.timeline.length - 1];
    assert(render.computeLiveAdvanceRate(s20, seg20a) === 0, 'rate must be 0 while disconnected');
    s20._v2ConnState = 'syncing';
    assert(render.computeLiveAdvanceRate(s20, seg20a) === 0, 'rate must be 0 while syncing');

    // Silent stale socket (>90s since last SSE event) also freezes virtualX.
    s20._v2ConnState = 'live';
    s20._lastSseEventTs = (Date.now() / 1000) - 120;
    assert(render.computeLiveAdvanceRate(s20, seg20a) === 0, 'rate must be 0 while SSE silent >90s');
    s20._lastSseEventTs = Date.now() / 1000;

    // Quiet live tip (avgTipH = 0) -> 0.22 * 5.0 = 1.1 px/s.
    s20._v2ConnState = 'live';
    const quietRate = render.computeLiveAdvanceRate(s20, seg20a);
    assert(Math.abs(quietRate - 1.1) < 1e-6, `quiet tip should pace 1.1 px/s, got ${quietRate}`);

    // Full live tip (avgTipH >= 65) -> full 5.0 px/s.
    const baseTip = Math.floor(s20.virtualX / 5) * 5;
    for (let tc = 0; tc < 5; tc++) seg20a._colHeights[baseTip - tc * 5] = 80;
    const fullRate = render.computeLiveAdvanceRate(s20, seg20a);
    assert(Math.abs(fullRate - 5.0) < 1e-6, `full tip should pace 5.0 px/s, got ${fullRate}`);

    // 20b. isResync with >100 newTxs backfills directly after the last real brick column
    //      in chronological first_seen order instead of stranding a gap + detached right island.
    resetState({ virtualX: 400, zoom: 1.0, height: 600, autoFollow: true, _v2ConnState: 'live' });
    const initTxs = [];
    for (let i = 0; i < 60; i++) {
        initTxs.push([txid(90000 + i), nowSec - 1800 + i, 500, 200, 10000, 0, '', i + 1]);
    }
    sse.hbV2InstallSnapshot(JSON.stringify({ recent_blocks: [], transactions: initTxs }), false);
    drainRafs();
    const seg20 = state.getState().timeline[state.getState().timeline.length - 1];
    let lastInitGx = 0;
    for (const k of Object.keys(seg20._colHeights)) {
        if (seg20._colHeights[k] > 0 && Number(k) > lastInitGx) lastInitGx = Number(k);
    }
    // Simulate a wide empty stretch behind virtualX before resync arrives.
    state.getState().virtualX = lastInitGx + 600;
    const earlyId = txid(91000);
    const lateId = txid(91149);
    const resyncTxs = initTxs.slice();
    // Push in reverse timestamp order to prove isResync sorts chronologically.
    for (let j = 149; j >= 0; j--) {
        resyncTxs.push([txid(91000 + j), nowSec - 1500 + j * 10, 600, 200, 10000, 0, '', 100 + j]);
    }
    sse.hbV2InstallSnapshot(JSON.stringify({ recent_blocks: [], transactions: resyncTxs }), true);
    const earlyBlip = seg20._blipsByTxid.get(earlyId);
    const lateBlip = seg20._blipsByTxid.get(lateId);
    assert(earlyBlip && lateBlip, 'resync must place both early and late recovered txs');
    assert(
        earlyBlip.gridX <= lastInitGx + 100,
        `early recovered tx should backfill right after lastInitGx (${lastInitGx}), landed at ${earlyBlip.gridX}`
    );
    assert(
        earlyBlip.gridX < lateBlip.gridX,
        `resync must preserve chronological order (early=${earlyBlip.gridX} < late=${lateBlip.gridX})`
    );
    // And trailing empty void must be compacted rather than turning 150 txs into sparse dust.
    assert(
        state.getState().virtualX < lastInitGx + 500,
        `expected resync to compact unused gap, virtualX=${state.getState().virtualX} (lastInitGx=${lastInitGx})`
    );

    // 20c. Quiet 20m hidden tab with only 20 arrivals must NOT carve a 1,500px empty desert.
    resetState({ virtualX: 500, zoom: 1.0, height: 600, autoFollow: true });
    document.hidden = true;
    for (let k = 0; k < 20; k++) {
        const id = txid(92000 + k);
        const ts = nowSec - 1200 + k * 60;
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'membership_add', txid: id, first_seen: ts }));
        sse.hbV2ApplyEffect(JSON.stringify({ type: 'tx_details', tx: [id, ts, 400, 200, 50000, 0, '', 0] }));
    }
    document.hidden = false;
    window._hbRecoverTimeline(1200);
    const deltaX = state.getState().virtualX - 500;
    assert(deltaX <= 120, `quiet 20-tx hidden tab carved too wide a desert: deltaX=${deltaX}px`);
});

if (failures.length > 0) {
    console.error(`\nFAIL (${failures.length} of ${passed + failures.length} scenarios failed)`);
    process.exit(1);
}
console.log(`\nPASS  ${passed} of ${passed} Heartbeat carry/reveal/recovery scenarios passed`);
