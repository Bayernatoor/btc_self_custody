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
await import(modUrl('heartbeat.js'));

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
        _pendingBlocks: [],
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
    let seg = s.timeline[s.timeline.length - 1];
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

if (failures.length > 0) {
    console.error(`\nFAIL (${failures.length} of ${passed + failures.length} scenarios failed)`);
    process.exit(1);
}
console.log(`\nPASS  ${passed} of ${passed} Heartbeat carry/reveal/recovery scenarios passed`);
