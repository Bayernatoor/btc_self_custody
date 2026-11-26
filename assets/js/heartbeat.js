// Block Heartbeat v3: ES Module entry point
// Imports all sub-modules and wires up the window.* public API
// that the Leptos component calls via wasm_bindgen.

import { getState, setState, COLORS, BG_COLOR, POINT_WIDTH, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, RETARGET_PERIOD } from './heartbeat-state.js';
import { drawGrid, createBlockSegment, createFlatlineSegment, computeColor, historyFlatlineWidth, resolveInterBlockSeconds, waitColor, UNKNOWN_INTERVAL_COLOR, effectiveBlockWallTime } from './heartbeat-timeline.js';
import { setupInputHandlers, handleControlClick, initSoundButton } from './heartbeat-interaction.js';
import { stopMomentum } from './heartbeat-interaction.js';
import {
    hbV2InstallSnapshot, hbV2ApplyEffect, hbV2FocusTx, hbV2SetPinnedTx,
    hbV2FocusBlockHeight, hbV2FocusPendingSlot, hbV2IsPendingSlotOffscreen,
    hbV2SetConnectionState, withLiveTimeline, compressSealedFlatline, shiftSegmentBlips,
    drainHiddenTxsAcrossGap, ensureSegMaps, restackColumns,
    registerBlipInSeg, txidSkylineOffset, buildBlip, updateBlipMetadata,
    rebuildRevealIndex, maybeClearRemovedTxids, flushTxBatch
} from './heartbeat-sse.js';
import { playBlockCue, tuneSound, soundSettings, setSoundPreset } from './heartbeat-audio.js';
import { drawFrame } from './heartbeat-render.js';
import {
    getHeartbeatVitals, renderRhythmStrip, getHeartbeatRecentBlocks,
    heartbeatCenter, heartbeatSearchTx,
    heartbeatPulse, heartbeatFlash,
    getOrganismStatus,
    heartbeatDownloadCapture
} from './heartbeat-vitals.js';

// Seconds of SSE silence before a 'live' connection is reported as 'stale'.
// Set above observed 64s quiet mempool stretches while still catching dead sockets
// (metrics_updated refreshes _lastSseEventTs every ~15s on a live node).
var HEARTBEAT_STALL_SECS = 90;

// ── Public API ─────────────────────────────────────────────

window.initHeartbeat = function(canvasId) {
    if (getState()) window.destroyHeartbeat();
    if (window._seenNotableTxids) window._seenNotableTxids.clear();

    var canvas = document.getElementById(canvasId);
    if (!canvas) return;

    var container = canvas.parentElement;
    // Pin canvas to fill container's flex-computed box even when wrap.style.height='auto'
    // in fullscreen (otherwise CSS `height: 100%` resolves against intrinsic canvas.height).
    if (canvas.style) {
        canvas.style.position = 'absolute';
        canvas.style.inset = '0';
        canvas.style.width = '100%';
        canvas.style.height = '100%';
        canvas.style.display = 'block';
    }
    var dpr = Math.min(window.devicePixelRatio || 1, 2);
    var rect = (container || canvas).getBoundingClientRect();

    canvas.width = Math.max(1, Math.round(rect.width * dpr));
    canvas.height = Math.max(1, Math.round(rect.height * dpr));

    var ctx = canvas.getContext('2d');
    ctx.scale(dpr, dpr);

    var w = rect.width;
    var h = rect.height;

    // Fill background and draw grid
    ctx.fillStyle = BG_COLOR;
    ctx.fillRect(0, 0, w, h);
    drawGrid(ctx, w, h);

    setState({
        canvas: canvas,
        ctx: ctx,
        width: w,
        height: h,
        dpr: dpr,

        // Virtual timeline
        timeline: [],
        virtualX: 0,
        viewOffset: -w * HEAD_POSITION_FRAC,
        viewOffsetY: 0,
        autoFollow: true,
        paused: false,
        renderMode: 'bricks',
        // Level-of-detail: when zoomed way out (<0.3x) with an extreme mempool
        // (>100k bricks), draw per-column density bars instead of every brick.
        // Toggle at runtime with window._hbToggleLod() (persisted in localStorage).
        _lodEnabled: (function() { try { return localStorage.getItem('hb_lod') !== 'off'; } catch (e) { return true; } })(),
        // Cinematic block reveal (form->harvest->unfurl->hold->return). Off = instant
        // flow (spike appears, harvest, mempool re-lays, no camera hijack; follow
        // state respected). Respects prefers-reduced-motion: reduce unless overridden.
        _revealEnabled: (function() {
            try {
                var pref = localStorage.getItem('hb_reveal');
                if (pref === 'off') return false;
                if (pref === 'on') return true;
            } catch (e) {}
            if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
                try {
                    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return false;
                } catch (e) {}
            }
            return true;
        })(),
        zoom: 1.6,
        // minZoom must go low enough to fit the whole mempool ribbon + spikes on
        // one screen even during 150k-200k congestion (~75k-100k virtual px).
        minZoom: 0.012,
        maxZoom: 50.0,

        // View mode ('live' | 'history') and parked live backup while browsing history
        _viewMode: 'live',
        _liveBackup: null,
        _historyMeta: null,
        _suppressLiveVisuals: false,

        // Bidirectional infinite historical block lazy-loading state
        _fetchGen: 0,
        _histFetchLeft: false,
        _histFetchRight: false,
        _histMinReached: false,
        _histMaxReached: false,
        _histTipHeight: 0,
        _histFetchCooldownUntil: 0,

        // Rendering
        rafId: null,
        lastFrameTime: 0,
        // First-load fade veil start (drawFrame eases the timeline up from the bg
        // over LOAD_FADE_SECS). Set at init so the fade begins when the canvas
        // appears (loader dismissed right after) and covers the spikes + bricks
        // streaming in; re-armed after a big gap-recovery so a resume also fades.
        _loadFadeStart: Date.now() / 1000,

        // Live state
        lastBlockTime: 0,
        nextBlockFee: 0,
        mempoolMB: 0,
        mempoolMinFee: 0,
        hashrateEH: 0,
        difficulty: 0,
        blockHeight: 0,
        currentColor: COLORS.healthy,
        prevColor: COLORS.healthy,
        targetColor: COLORS.healthy,
        colorLerp: 1,

        // Interaction
        isDragging: false,
        dragStartX: 0,
        dragStartOffset: 0,
        hoveredBlock: null,
        hoveredPendingBlock: null,
        hoveredBlip: null,
        _pinnedTxid: null,
        _pinnedBlip: null,
        hoveredFlatline: null,
        hoverCanvasX: 0,
        _flashTimer: null,

        // Mempool feed (v2 state owned by Rust/WASM client)
        ws: null,
        wsConnected: false,
        _v2ConnState: 'live',
        _coverage: { status: 'unknown' },
        _pendingAdds: new Map(),
        _txBatchQueue: [],
        _removedTxids: new Set(),
        _exactConfirmedByHash: new Map(),
        _deferredDetails: new Map(),
        _pendingCarryByTxid: null,

        // Connection overlay state
        _sseDisconnected: false,
        _sseDisconnectedSince: 0,

        // Recent blocks (kept for vital signs / rhythm strip)
        recentBlocks: []
    });

    var _hb = getState();

    // Start with a live flatline at position 0
    _hb.timeline.push(createFlatlineSegment(0, null));

    // Setup input handlers
    setupInputHandlers(canvas);
    // Paint the speaker icon from the stored preference: the markup ships in the off
    // state, so a returning visitor with sound on would otherwise see the wrong icon.
    initSoundButton();

    // Show fullscreen buttons only if the API is supported (not on iOS)
    if (document.fullscreenEnabled || document.webkitFullscreenEnabled) {
        var fsBtn = document.getElementById('heartbeat-fullscreen-btn');
        if (fsBtn) fsBtn.classList.remove('hidden');
        var fsBtnMob = document.getElementById('heartbeat-fullscreen-btn-mobile');
        if (fsBtnMob) fsBtnMob.classList.remove('hidden');
    }

    // Fullscreen sizing
    var wrap = document.getElementById('heartbeat-canvas-wrap');
    var card = document.getElementById('heartbeat-card');
    function _hbFullscreenChange() {
        var s = getState();
        if (!s || !wrap || !card) return;
        var fsEl = document.fullscreenElement || document.webkitFullscreenElement;
        var modal = document.getElementById('block-detail-modal');
        if (fsEl) {
            card.style.width = '100vw';
            card.style.height = '100vh';
            card.style.position = 'fixed';
            card.style.top = '0';
            card.style.left = '0';
            card.style.zIndex = '9999';
            card.style.borderRadius = '0';
            // Override Tailwind's sm:h-[40vh] so flex-1 fills fullscreen
            wrap.style.height = 'auto';
            wrap.style.minHeight = '0';
            // Move block detail modal inside fullscreen element so it's visible
            if (modal && modal.parentNode !== card) {
                s._modalOrigParent = modal.parentNode;
                s._modalOrigNext = modal.nextSibling;
                card.appendChild(modal);
            }
            // Center the timeline in the tall fullscreen canvas. Deferred to the
            // ResizeObserver (below) since s.height isn't updated until the canvas
            // relayouts to fullscreen.
            s._recenterPending = true;
        } else {
            card.style.width = '';
            card.style.height = '';
            card.style.position = '';
            card.style.top = '';
            card.style.left = '';
            card.style.zIndex = '';
            card.style.borderRadius = '';
            // Clear overrides, let Tailwind classes take over
            wrap.style.height = '';
            wrap.style.minHeight = '';
            // Move modal back to its original position
            if (modal && s._modalOrigParent) {
                if (s._modalOrigNext) {
                    s._modalOrigParent.insertBefore(modal, s._modalOrigNext);
                } else {
                    s._modalOrigParent.appendChild(modal);
                }
                s._modalOrigParent = null;
                s._modalOrigNext = null;
            }
            // Back to the card: reset the vertical offset to the default framing.
            s.viewOffsetY = 0;
            s._recenterPending = false;
        }
    }
    document.addEventListener('fullscreenchange', _hbFullscreenChange);
    document.addEventListener('webkitfullscreenchange', _hbFullscreenChange);
    _hb._fullscreenHandler = _hbFullscreenChange;

    // ResizeObserver for responsive canvas
    if (typeof ResizeObserver !== 'undefined') {
        _hb.resizeObs = new ResizeObserver(function() {
            var s = getState();
            if (!s) return;
            var r = (container || canvas).getBoundingClientRect();
            if (r.width <= 0 || r.height <= 0) return;
            canvas.width = Math.round(r.width * dpr);
            canvas.height = Math.round(r.height * dpr);
            s.ctx = canvas.getContext('2d');
            s.ctx.scale(dpr, dpr);
            s.width = r.width;
            s.height = r.height;
            // On fullscreen-enter the canvas gets much taller; drop the baseline to
            // ~0.60h so the timeline sits centered (default 0.55h read top-heavy,
            // 0.66h too low: 0.60 is the midpoint). Tunable knob.
            if (s._recenterPending && r.height > 0) {
                s._recenterPending = false;
                var base = (s.height < 350 ? s.height * 0.78 : s.height * 0.55);
                s.viewOffsetY = s.height * 0.60 - base;
            }
        });
        _hb.resizeObs.observe(container);
    }

    // Start animation loop
    _hb.rafId = requestAnimationFrame(drawFrame);

    // Sync HTML control button states periodically (every frame via RAF)
    _hb._syncControls = function() {
        var s = getState();
        if (!s) return;
        // Zoom label (2 decimals when zoomed out past 1x so 0.03x-0.1x reads)
        var zl = document.getElementById('heartbeat-zoom-label');
        if (zl) zl.textContent = s.zoom.toFixed(s.zoom < 1 ? 2 : 1) + 'x';
        // Pause / Play icon: toggle pre-rendered SVGs (fallback to text if legacy span)
        var pauseSvg = document.getElementById('heartbeat-pause-svg');
        var playSvg = document.getElementById('heartbeat-play-svg');
        if (pauseSvg && playSvg) {
            pauseSvg.classList.toggle('hidden', !!s.paused);
            playSvg.classList.toggle('hidden', !s.paused);
        } else {
            var pb = document.getElementById('heartbeat-btn-pause-icon');
            if (pb) pb.textContent = s.paused ? '\u25B6' : '\u23F8';
        }
        var pauseBtn = document.getElementById('heartbeat-btn-pause');
        if (pauseBtn) {
            pauseBtn.setAttribute('aria-pressed', s.paused ? 'true' : 'false');
        }
        // LIVE pill: glows green while following the head, dims when free-panned.
        var lb = document.getElementById('heartbeat-btn-live');
        var dot = document.getElementById('heartbeat-live-dot');
        if (lb) {
            if (s.autoFollow) {
                lb.style.color = '#00e676';
                lb.style.background = 'rgba(0,230,118,0.12)';
                lb.style.borderColor = 'rgba(0,230,118,0.4)';
                lb.style.opacity = '1';
                if (dot) dot.style.background = '#00e676';
            } else {
                lb.style.color = 'rgba(255,255,255,0.55)';
                lb.style.background = 'rgba(255,255,255,0.05)';
                lb.style.borderColor = 'rgba(255,255,255,0.12)';
                lb.style.opacity = '0.9';
                if (dot) dot.style.background = 'rgba(255,255,255,0.4)';
            }
        }
    };
};

// Harvest active bricks from `closingSeg` (plus any unplaced items queued in
// `_pendingCarry` or `_pendingSnapshot` for `closingSeg`), partitioning strictly
// by exact confirmed txids (`_exactConfirmedByHash.get(b.hash)`).
function harvestFlatlineBricks(_hb, closingSeg, b, absorbX, isReplay, harvestLive, revealPending) {
    var carriedLeftovers = [];
    var revealConfirmed = revealPending ? [] : null;
    var maxPartsPerBlip = 2;

    if (!closingSeg) {
        return { carriedLeftovers: carriedLeftovers, revealConfirmed: revealConfirmed, maxPartsPerBlip: maxPartsPerBlip };
    }

    var exactConf = (!isReplay && b.hash && _hb._exactConfirmedByHash)
        ? _hb._exactConfirmedByHash.get(b.hash)
        : null;

    if (closingSeg.blips) {
        var absorbNow = Date.now() / 1000;
        var zoom = _hb.zoom || 1;
        var activeCount = 0;
        for (var ci = 0; ci < closingSeg.blips.length; ci++) {
            if (!closingSeg.blips[ci]._removed && closingSeg.blips[ci].fadeStart === 0) activeCount++;
        }
        if (zoom < 0.5 || activeCount > 1000) maxPartsPerBlip = 1;
        else if (zoom < 1.0 || activeCount > 500) maxPartsPerBlip = 2;
        else maxPartsPerBlip = 3 + Math.floor(Math.random() * 3);

        var kept = [];
        for (var bi = 0; bi < closingSeg.blips.length; bi++) {
            var blp = closingSeg.blips[bi];
            if (!blp || blp._removed) continue;
            if (blp.fadeStart !== 0) { kept.push(blp); continue; } // already fading
            if (!isReplay && blp._pendingInclusionHash && blp._pendingInclusionHash !== b.hash) {
                // Already confirmed by a different block whose block_details has not
                // arrived yet: exclude from carriedLeftovers so it cannot survive live.
                blp._removed = true;
                continue;
            }

            var confirm;
            if (isReplay) {
                confirm = true;
            } else if (harvestLive) {
                confirm = !!(exactConf && blp.txid && exactConf.set.has(blp.txid));
            } else {
                confirm = false;
            }

            if (!confirm) {
                carriedLeftovers.push(blp);
                if (revealPending) kept.push(blp);
                continue;
            }
            blp._pendingInclusionHash = null;
            if (revealPending) {
                revealConfirmed.push(blp);
                kept.push(blp);
                continue;
            }
            blp.fadeStart = absorbNow;
            blp._confirmedHeight = b.height;
            blp.absorbTargetX = absorbX;
            blp.absorbOriginX = blp.x;
            blp.particles = [];
            for (var pi = 0; pi < maxPartsPerBlip; pi++) {
                blp.particles.push({
                    offsetX: (Math.random() - 0.5) * 8,
                    offsetY: Math.random() * (blp.height || 10) * 0.8,
                    speed: 0.7 + Math.random() * 0.6,
                    arcHeight: 20 + Math.random() * 60,
                    delay: Math.random() * 0.3,
                    size: 1.5 + Math.random() * 2.5
                });
            }
            kept.push(blp);
        }
        closingSeg.blips = kept;
        if (closingSeg._blipsByTxid) closingSeg._blipsByTxid.clear();
        if (closingSeg._colBlips) closingSeg._colBlips.clear();
        closingSeg._colHeights = {};
    }

    // Never drop bricks that were still queued in an in-flight chunked carry or
    // snapshot placement for `closingSeg` (e.g. back-to-back blocks or hidden-tab catch-up).
    if (!isReplay && harvestLive) {
        var seenCarried = null;
        var appendRemaining = function(blp) {
            if (!blp || blp._removed) return;
            if (_hb._removedTxids && blp.txid && _hb._removedTxids.has(blp.txid)) return;
            if (blp._pendingInclusionHash && blp._pendingInclusionHash !== b.hash) {
                blp._removed = true;
                return;
            }
            if (exactConf && blp.txid && exactConf.set.has(blp.txid)) return;
            if (!seenCarried) {
                seenCarried = new Set();
                for (var sc = 0; sc < carriedLeftovers.length; sc++) {
                    if (carriedLeftovers[sc] && carriedLeftovers[sc].txid) {
                        seenCarried.add(carriedLeftovers[sc].txid);
                    }
                }
            }
            if (blp.txid && seenCarried.has(blp.txid)) return;
            if (blp.txid) seenCarried.add(blp.txid);
            carriedLeftovers.push(blp);
        };
        if (_hb._pendingCarry && _hb._pendingCarry.seg === closingSeg) {
            if (_hb._carryRaf) { cancelAnimationFrame(_hb._carryRaf); _hb._carryRaf = null; }
            var pc = _hb._pendingCarry;
            _hb._pendingCarry = null;
            _hb._pendingCarryByTxid = null;
            for (var pci = pc.nextIdx; pci < pc.items.length; pci++) {
                var pItem = pc.items[pci];
                if (pItem && pItem.src) {
                    if (_hb._deferredDetails && pItem.src.txid && _hb._deferredDetails.has(pItem.src.txid)) {
                        updateBlipMetadata(pItem.src, _hb._deferredDetails.get(pItem.src.txid));
                        _hb._deferredDetails.delete(pItem.src.txid);
                    }
                    appendRemaining(pItem.src);
                }
            }
        }
        if (_hb._pendingSnapshot && _hb._pendingSnapshot.seg === closingSeg) {
            if (_hb._historyRaf) { cancelAnimationFrame(_hb._historyRaf); _hb._historyRaf = null; }
            var ps = _hb._pendingSnapshot;
            _hb._pendingSnapshot = null;
            for (var psi = ps.nextIdx; psi < ps.items.length; psi++) {
                var sItem = ps.items[psi];
                if (sItem && sItem.tx && ps.buildBlip) {
                    var sTx = sItem.tx;
                    if (_hb._removedTxids && _hb._removedTxids.has(sTx.txid)) continue;
                    if (_hb._deferredDetails && _hb._deferredDetails.has(sTx.txid)) {
                        var sDef = _hb._deferredDetails.get(sTx.txid);
                        if (sDef.value == null && sTx.value != null) sDef.value = sTx.value;
                        if (!sDef.first_seen && sTx.first_seen) sDef.first_seen = sTx.first_seen;
                        sTx = sDef;
                        _hb._deferredDetails.delete(sTx.txid);
                    }
                    appendRemaining(ps.buildBlip(sTx, 0, 0, 0, Date.now() / 1000, false));
                }
            }
        }
    }

    return {
        carriedLeftovers: carriedLeftovers,
        revealConfirmed: revealConfirmed,
        maxPartsPerBlip: maxPartsPerBlip
    };
}

window.pushHeartbeatBlocks = function(json, replay) {
    var _hb = getState();
    if (!_hb) return;
    var isReplay = !!replay;
    // Harvest only for genuine live blocks (set by processLiveBlock), not replay
    // /history/tab-return. Consume the one-shot flag.
    var harvestLive = !!_hb._harvestLive;
    _hb._harvestLive = false;
    // When set (by processLiveBlock for a reveal-eligible live block), defer the
    // leftover re-lay and flatline compression to the block-reveal sequencer so
    // leftovers stay visible through form/harvest before re-laying on unfurl.
    var revealPending = !!_hb._revealPending;
    _hb._revealPending = false;
    try {
        var blocks = JSON.parse(json);
        if (!Array.isArray(blocks)) return;
        for (var i = 0; i < blocks.length; i++) {
            var b = blocks[i];

            // Deduplicate: skip if this block hash/height is already confirmed on the timeline.
            // Exception: an estimated spike (weight==0 || total_fees==0) gets upgraded when
            // authoritative data arrives.
            if (!isReplay && (b.height || b.hash)) {
                var existingSeg = null;
                for (var di = _hb.timeline.length - 1; di >= Math.max(0, _hb.timeline.length - 30); di--) {
                    var cand = _hb.timeline[di];
                    if (cand.type === 'block' && ((b.hash && cand.hash === b.hash) || (!b.hash && b.height && cand.height === b.height))) {
                        existingSeg = cand;
                        break;
                    }
                }
                if (existingSeg) {
                    if (b.hash && !existingSeg.hash) existingSeg.hash = b.hash;
                    var existingMissing = (existingSeg.weight === 0 || existingSeg.total_fees === 0);
                    var incomingHasData = (b.weight > 0 || b.total_fees > 0);
                    if (existingMissing && incomingHasData) {
                        var replacement = createBlockSegment(b, existingSeg.x_start);
                        replacement.x_start = existingSeg.x_start;
                        replacement.x_end = existingSeg.x_end;
                        existingSeg.points = replacement.points;
                        existingSeg._minY = replacement._minY;
                        existingSeg._minYIdx = replacement._minYIdx;
                        existingSeg._maxY = replacement._maxY;
                        existingSeg._maxYIdx = replacement._maxYIdx;
                        existingSeg.color = replacement.color;
                        existingSeg.tx_count = replacement.tx_count;
                        existingSeg.total_fees = replacement.total_fees;
                        existingSeg.size = replacement.size;
                        existingSeg.weight = replacement.weight;
                        if (replacement.miner) existingSeg.miner = replacement.miner;
                        existingSeg.inter_block_seconds = replacement.inter_block_seconds;
                        if (!_hb._suppressAnnounce) {
                            var heightStr = b.height.toLocaleString();
                            var feePart = b.total_fees > 0 ? (b.total_fees / 100000000).toFixed(4) + ' BTC fees \u00b7 ' : '';
                            var nowAnn = Date.now() / 1000;
                            _hb._announcement = {
                                title: 'Block #' + heightStr + ' found',
                                subtitle: feePart + (b.tx_count || 0).toLocaleString() + ' txs',
                                color: replacement.color || _hb.currentColor || COLORS.healthy,
                                start: nowAnn,
                                end: nowAnn + 12.0
                            };
                        }
                    }
                    continue;
                }
            }

            // Check whether a reserved pending_block slot exists for this block hash.
            var pendingIdx = -1;
            if (!isReplay && b.hash) {
                for (var pti = _hb.timeline.length - 1; pti >= 0; pti--) {
                    if (_hb.timeline[pti].type === 'pending_block' && _hb.timeline[pti].hash === b.hash) {
                        pendingIdx = pti;
                        break;
                    }
                }
            }

            // Resolve exact signed inter_block_seconds (preserving 0 and negative MTP deltas).
            // Only measure against an immediately preceding block whose height matches b.height - 1;
            // if a pending_block sits between them or height differs, leave rawInter = null.
            var rawInter = resolveInterBlockSeconds(b);
            if (!isReplay && rawInter === null && b.timestamp) {
                var prevScanFrom = pendingIdx >= 0 ? (pendingIdx - 1) : (_hb.timeline.length - 1);
                var sawPriorBlockOrPending = false;
                for (var pbi = prevScanFrom; pbi >= 0; pbi--) {
                    var pSeg = _hb.timeline[pbi];
                    if (pSeg.type === 'pending_block') {
                        sawPriorBlockOrPending = true;
                        break;
                    }
                    if (pSeg.type === 'block') {
                        sawPriorBlockOrPending = true;
                        if (pSeg.timestamp && b.height && pSeg.height + 1 === b.height) {
                            rawInter = b.timestamp - pSeg.timestamp;
                        }
                        break;
                    }
                }
                if (rawInter === null && !sawPriorBlockOrPending && _hb.lastBlockTime > 0) {
                    rawInter = b.timestamp - _hb.lastBlockTime;
                }
            }
            var flatColorForInter = function(interVal) {
                return interVal === null ? UNKNOWN_INTERVAL_COLOR : waitColor(Math.max(0, interVal));
            };
            b.inter_block_seconds = rawInter;

            var blockSeg;
            var blockIdx = -1;
            var closingSeg = null;
            var targetLiveSeg = null;
            var absorbX = _hb.virtualX;
            var harvestRes = { carriedLeftovers: [], revealConfirmed: revealPending ? [] : null, maxPartsPerBlip: 2 };

            if (pendingIdx >= 0) {
                // ── Path A: Replace reserved pending_block slot in place ──
                var pendingSeg = _hb.timeline[pendingIdx];
                if (_hb.hoveredPendingBlock === pendingSeg) _hb.hoveredPendingBlock = null;
                closingSeg = (pendingIdx > 0 && _hb.timeline[pendingIdx - 1].type === 'flatline')
                    ? _hb.timeline[pendingIdx - 1]
                    : null;
                var postFlatline = (pendingIdx + 1 < _hb.timeline.length && _hb.timeline[pendingIdx + 1].type === 'flatline')
                    ? _hb.timeline[pendingIdx + 1]
                    : null;

                if (closingSeg) {
                    closingSeg._pendingForHash = null;
                    closingSeg.inter_block_seconds = rawInter;
                    closingSeg.color = flatColorForInter(rawInter);
                }
                absorbX = (pendingSeg.x_start + pendingSeg.x_end) / 2;
                harvestRes = harvestFlatlineBricks(_hb, closingSeg, b, absorbX, false, harvestLive, revealPending);

                // B3: if block_inclusion previously tagged any confirmed brick for b.hash
                // on an active flatline to the RIGHT of pendingSeg (e.g., a race or multi-pending
                // snapshot), sweep and harvest it now so no confirmed brick survives live.
                if (harvestLive && b.hash) {
                    var postAbsorbNow = Date.now() / 1000;
                    for (var rfi = pendingIdx + 1; rfi < _hb.timeline.length; rfi++) {
                        var rfSeg = _hb.timeline[rfi];
                        if (!rfSeg || rfSeg.type !== 'flatline' || !rfSeg.blips || rfSeg.blips.length === 0) continue;
                        var rfDirty = new Set();
                        for (var rbi = 0; rbi < rfSeg.blips.length; rbi++) {
                            var rblp = rfSeg.blips[rbi];
                            if (!rblp || rblp._removed || rblp.fadeStart !== 0) continue;
                            if (rblp._pendingInclusionHash !== b.hash) continue;
                            rblp._pendingInclusionHash = null;
                            if (rfSeg._blipsByTxid && rblp.txid) rfSeg._blipsByTxid.delete(rblp.txid);
                            if (rblp.gridX !== undefined) rfDirty.add(rblp.gridX);
                            if (revealPending && harvestRes.revealConfirmed) {
                                // Detach from post-flatline and absorb with the reveal
                                rblp._removed = true;
                                if (_hb.hoveredBlip === rblp) _hb.hoveredBlip = null;
                                if (_hb._pinnedBlip === rblp) _hb._pinnedBlip = null;
                            } else {
                                rblp.fadeStart = postAbsorbNow;
                                rblp._confirmedHeight = b.height;
                                rblp.absorbTargetX = absorbX;
                                rblp.absorbOriginX = rblp.x;
                                rblp.particles = [];
                                for (var rpi = 0; rpi < harvestRes.maxPartsPerBlip; rpi++) {
                                    rblp.particles.push({
                                        offsetX: (Math.random() - 0.5) * 8,
                                        offsetY: Math.random() * (rblp.height || 10) * 0.8,
                                        speed: 0.7 + Math.random() * 0.6,
                                        arcHeight: 20 + Math.random() * 60,
                                        delay: Math.random() * 0.3,
                                        size: 1.5 + Math.random() * 2.5
                                    });
                                }
                            }
                        }
                        if (rfDirty.size > 0) {
                            rfSeg.blips = rfSeg.blips.filter(function(x) { return x && !x._removed; });
                            for (var reIdx = 0; reIdx < rfSeg.blips.length; reIdx++) {
                                rfSeg.blips[reIdx]._segIdx = reIdx;
                            }
                            restackColumns(rfSeg, rfDirty);
                        }
                    }
                }

                blockSeg = createBlockSegment(b, pendingSeg.x_start);
                if (revealPending) blockSeg._strikePending = true;
                else blockSeg._strikeStart = Date.now() / 1000;
                _hb.timeline[pendingIdx] = blockSeg;
                blockIdx = pendingIdx;

                // Ensure we have an open live flatline at the tail
                var tailSeg = _hb.timeline[_hb.timeline.length - 1];
                if (!tailSeg || tailSeg.type !== 'flatline' || tailSeg.x_end !== null) {
                    tailSeg = createFlatlineSegment(Math.max(_hb.virtualX, blockSeg.x_end), null);
                    _hb.timeline.push(tailSeg);
                }
                targetLiveSeg = (postFlatline && postFlatline.x_end === null) ? postFlatline : tailSeg;

                // Compress the sealed pre-block flatline immediately in non-reveal mode
                // (in reveal mode, relayRevealLeftovers compresses it after harvest).
                if (!revealPending && closingSeg) {
                    compressSealedFlatline(_hb, closingSeg, pendingIdx, rawInter);
                    absorbX = (blockSeg.x_start + blockSeg.x_end) / 2;
                }
            } else {
                // ── Path B: Direct block_details (no prior pending_block slot) ──
                // If an older block arrives out-of-order after a newer confirmed block (or
                // when a newer pending_block sits at the tail), insert in height order rather
                // than appending an older height after a newer confirmed block.
                var insertBeforeIdx = -1;
                if (!isReplay && b.height) {
                    for (var obi = _hb.timeline.length - 1; obi >= 0; obi--) {
                        var oCand = _hb.timeline[obi];
                        if (oCand.type === 'block') {
                            if (oCand.height > b.height) insertBeforeIdx = obi;
                            else break;
                        }
                    }
                }

                if (insertBeforeIdx > 0 && _hb.timeline[insertBeforeIdx - 1].type === 'flatline') {
                    var gapFlat = _hb.timeline[insertBeforeIdx - 1];
                    var nextConfirmed = _hb.timeline[insertBeforeIdx];
                    var oldNextStart = nextConfirmed.x_start;

                    // Re-resolve rawInter strictly against the block immediately before gapFlat.
                    rawInter = resolveInterBlockSeconds(b);
                    if (rawInter === null && b.timestamp && insertBeforeIdx >= 2) {
                        var priorSeg = _hb.timeline[insertBeforeIdx - 2];
                        if (priorSeg.type === 'block' && priorSeg.timestamp && priorSeg.height + 1 === b.height) {
                            rawInter = b.timestamp - priorSeg.timestamp;
                        }
                    }
                    b.inter_block_seconds = rawInter;

                    var gapW = historyFlatlineWidth(rawInter);
                    gapFlat.x_end = gapFlat.x_start + gapW;
                    gapFlat.inter_block_seconds = rawInter;
                    gapFlat.color = flatColorForInter(rawInter);
                    gapFlat._compressed = true;
                    gapFlat._sortedByX = false;

                    blockSeg = createBlockSegment(b, gapFlat.x_end);
                    blockSeg._strikeStart = Date.now() / 1000;

                    var afterInter = (nextConfirmed.height === b.height + 1 && b.timestamp && nextConfirmed.timestamp)
                        ? (nextConfirmed.timestamp - b.timestamp)
                        : resolveInterBlockSeconds(nextConfirmed);
                    var afterFlat = createFlatlineSegment(blockSeg.x_end, blockSeg.x_end + historyFlatlineWidth(afterInter));
                    afterFlat.inter_block_seconds = afterInter;
                    afterFlat.color = flatColorForInter(afterInter);
                    afterFlat._compressed = true;

                    var shiftDelta = afterFlat.x_end - oldNextStart;
                    _hb.timeline.splice(insertBeforeIdx, 0, blockSeg, afterFlat);
                    blockIdx = insertBeforeIdx;

                    if (Math.abs(shiftDelta) > 0.001) {
                        for (var sri = insertBeforeIdx + 2; sri < _hb.timeline.length; sri++) {
                            var sSeg = _hb.timeline[sri];
                            sSeg.x_start += shiftDelta;
                            if (sSeg.x_end !== null) sSeg.x_end += shiftDelta;
                            if (typeof sSeg._carryEndX === 'number') sSeg._carryEndX += shiftDelta;
                            if (sSeg.type === 'flatline') shiftSegmentBlips(sSeg, shiftDelta);
                        }
                        _hb.virtualX += shiftDelta;
                        if (_hb._reveal || _hb.autoFollow || _hb.viewOffset >= oldNextStart) {
                            _hb.viewOffset += shiftDelta;
                        }
                    }
                    // Out-of-order historical/late insert does not harvest the current live mempool.
                    harvestLive = false;
                    revealPending = false;
                } else {
                    closingSeg = _hb.timeline[_hb.timeline.length - 1];
                    if (closingSeg && closingSeg.type === 'flatline' && closingSeg.x_end === null) {
                        if (isReplay) {
                            closingSeg.x_end = closingSeg.x_start + historyFlatlineWidth(rawInter);
                            closingSeg._compressed = true;
                            _hb.virtualX = closingSeg.x_end;
                            closingSeg.inter_block_seconds = rawInter;
                            closingSeg.color = flatColorForInter(rawInter);
                        } else {
                            var minLiveFlatline = Math.max(30, Math.round(20 / Math.max(_hb.zoom, 0.1)));
                            if (_hb.virtualX - closingSeg.x_start < minLiveFlatline) {
                                _hb.virtualX = closingSeg.x_start + minLiveFlatline;
                            }
                            closingSeg.x_end = _hb.virtualX;
                            closingSeg.inter_block_seconds = rawInter;
                            closingSeg.color = flatColorForInter(rawInter);
                        }
                        absorbX = _hb.virtualX;
                        harvestRes = harvestFlatlineBricks(_hb, closingSeg, b, absorbX, isReplay, harvestLive, revealPending);
                    }

                    blockSeg = createBlockSegment(b, _hb.virtualX);
                    if (!isReplay) {
                        if (revealPending) blockSeg._strikePending = true;
                        else blockSeg._strikeStart = Date.now() / 1000;
                    }
                    var blockSegIdx = _hb.timeline.length;
                    _hb.timeline.push(blockSeg);
                    blockIdx = blockSegIdx;
                    _hb.virtualX = blockSeg.x_end;

                    targetLiveSeg = createFlatlineSegment(_hb.virtualX, null);
                    _hb.timeline.push(targetLiveSeg);

                    // Compress the sealed live flatline immediately in non-reveal live mode
                    // (in reveal mode, relayRevealLeftovers compresses it cleanly after harvest).
                    if (!isReplay && !revealPending && closingSeg && closingSeg.type === 'flatline') {
                        compressSealedFlatline(_hb, closingSeg, blockSegIdx, rawInter);
                        absorbX = (blockSeg.x_start + blockSeg.x_end) / 2;
                    }
                }
            }

            // If block B (height + 1) already resolved before this block A with an unknown
            // interval, backfill B's exact interval, waveform, color, intervening flatline,
            // and recentBlocks entry now that A's timestamp is known.
            var nextBlkIdx = blockIdx >= 0 ? (blockIdx + 2) : -1;
            if (!isReplay && nextBlkIdx >= 2 && nextBlkIdx < _hb.timeline.length) {
                var nextBlk = _hb.timeline[nextBlkIdx];
                if (nextBlk && nextBlk.type === 'block' && b.height
                    && nextBlk.height === b.height + 1
                    && nextBlk.inter_block_seconds === null
                    && b.timestamp && nextBlk.timestamp) {
                    var bInter = nextBlk.timestamp - b.timestamp;
                    nextBlk.inter_block_seconds = bInter;
                    var refreshedNext = createBlockSegment(nextBlk, nextBlk.x_start);
                    nextBlk.points = refreshedNext.points;
                    nextBlk._minY = refreshedNext._minY;
                    nextBlk._minYIdx = refreshedNext._minYIdx;
                    nextBlk._maxY = refreshedNext._maxY;
                    nextBlk._maxYIdx = refreshedNext._maxYIdx;
                    nextBlk.color = refreshedNext.color;

                    var betweenFlat = _hb.timeline[blockIdx + 1];
                    if (betweenFlat && betweenFlat.type === 'flatline') {
                        betweenFlat.inter_block_seconds = bInter;
                        betweenFlat.color = flatColorForInter(bInter);
                        if (betweenFlat._compressed) {
                            betweenFlat._compressed = false;
                            compressSealedFlatline(_hb, betweenFlat, nextBlkIdx, bInter);
                        }
                    }

                    if (Array.isArray(_hb.recentBlocks)) {
                        for (var rbu = _hb.recentBlocks.length - 1; rbu >= 0; rbu--) {
                            var rbe = _hb.recentBlocks[rbu];
                            if ((nextBlk.hash && rbe.hash === nextBlk.hash) || rbe.height === nextBlk.height) {
                                rbe.inter_block_seconds = bInter;
                                break;
                            }
                        }
                    }
                }
            }

            // Update last block time using node observation wall-clock (clamped to now)
            // so a miner's future-skewed header timestamp cannot collapse live wait width.
            var wallBlockTime = effectiveBlockWallTime(b, Math.floor(Date.now() / 1000));
            if (wallBlockTime > 0 && wallBlockTime >= (_hb.lastBlockTime || 0)) {
                _hb.lastBlockTime = wallBlockTime;
            }

            // Carry unconfirmed leftovers onto the open live flatline (or arm reveal even when
            // leftovers is empty so the strike & "Block #X found" banner still fire cleanly).
            var carriedLeftovers = harvestRes.carriedLeftovers;
            if (harvestLive && !isReplay) {
                if (_hb._deferCarryList) {
                    _hb._deferCarryList = carriedLeftovers;
                } else if (revealPending) {
                    _hb._pendingReveal = {
                        leftovers: carriedLeftovers,
                        confirmed: harvestRes.revealConfirmed || [],
                        absorbX: absorbX,
                        maxParts: harvestRes.maxPartsPerBlip,
                        oldSeg: closingSeg,
                        newSeg: targetLiveSeg,
                        spikeSeg: blockSeg,
                        interBlock: rawInter,
                        _revealByTxid: null
                    };
                    rebuildRevealIndex(_hb._pendingReveal);
                } else if (carriedLeftovers.length > 0) {
                    placeCarriedLeftovers(carriedLeftovers, _hb._instantCarry ? 0 : undefined);
                } else if (!_hb.autoFollow && !_hb._reveal && !_hb._suppressLiveVisuals && _hb.width > 0 && _hb.zoom > 0) {
                    var emptyVisW = _hb.width / _hb.zoom;
                    if (_hb.viewOffset > _hb.virtualX - emptyVisW * 0.25) {
                        _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
                    }
                }
            }

            // Maintain recentBlocks list in height order (up to 2016 for period history)
            var recEntry = {
                hash: b.hash || null,
                timestamp: b.timestamp,
                height: b.height,
                tx_count: b.tx_count,
                total_fees: b.total_fees,
                size: b.size,
                weight: b.weight,
                miner: b.miner || null,
                inter_block_seconds: b.inter_block_seconds
            };
            if (_hb.recentBlocks.length > 0 && b.height && (_hb.recentBlocks[_hb.recentBlocks.length - 1].height || 0) > b.height) {
                var insIdx = _hb.recentBlocks.length;
                while (insIdx > 0 && (_hb.recentBlocks[insIdx - 1].height || 0) > b.height) {
                    insIdx--;
                }
                _hb.recentBlocks.splice(insIdx, 0, recEntry);
            } else {
                _hb.recentBlocks.push(recEntry);
            }

            // Show block arrival announcement on the instant path (reveal owns its own banners).
            if (!isReplay && !revealPending && !_hb._suppressAnnounce && b.height) {
                var feeBtc2 = (b.total_fees || 0) / 100000000;
                var heightStr2 = b.height.toLocaleString();
                var confStr2 = (b.tx_count || 0).toLocaleString() + ' confirmed';
                var remainPart2 = (harvestLive && carriedLeftovers && carriedLeftovers.length > 0)
                    ? (' \u00b7 ' + carriedLeftovers.length.toLocaleString() + ' remaining')
                    : '';
                var feePart2 = feeBtc2 > 0 ? (' \u00b7 ' + feeBtc2.toFixed(4) + ' BTC fees') : '';
                var nowAnn2 = Date.now() / 1000;
                _hb._announcement = {
                    title: 'Block #' + heightStr2 + ' found',
                    subtitle: confStr2 + remainPart2 + feePart2,
                    color: blockSeg.color || _hb.currentColor || COLORS.healthy,
                    start: nowAnn2,
                    end: nowAnn2 + 11.0
                };
            }
        }

        if (_hb.recentBlocks.length > RETARGET_PERIOD) {
            _hb.recentBlocks = _hb.recentBlocks.slice(-RETARGET_PERIOD);
        }

        // Prune very old timeline segments
        pruneTimeline();

        // After replay: fast-forward the live flatline to reflect time since last block
        if (isReplay && blocks.length > 0 && _hb.lastBlockTime > 0) {
            var nowSec = Date.now() / 1000;
            var elapsed = nowSec - _hb.lastBlockTime;
            if (elapsed > 0 && elapsed < 7200) {
                var advancePx = elapsed * FLATLINE_PX_PER_SEC;
                _hb.virtualX += advancePx;
                if (_hb.autoFollow && !_hb._pinnedTxid && !_hb._suppressLiveVisuals) {
                    _hb.zoom = 1.6;
                    _hb.viewOffsetY = 0;
                    _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
                }
            }
        }

    } catch (e) {
        // silently ignore malformed block JSON
    }
};

// Prune timeline segments that are far behind the viewport to limit memory
function pruneTimeline() {
    var _hb = getState();
    // Don't prune while a block reveal is running, while the user is scrolled back
    // inspecting older history (!autoFollow), or while browsing History Mode.
    if (!_hb || _hb.timeline.length < MAX_TIMELINE_SEGS || _hb._reveal || !_hb.autoFollow || _hb._viewMode === 'history') return;
    var minX = _hb.viewOffset - _hb.width * 10 / Math.max(_hb.zoom, 0.1);
    var cutIdx = 0;
    for (var i = 0; i < _hb.timeline.length; i++) {
        var seg = _hb.timeline[i];
        var segEnd = seg.type === 'block' ? seg.x_end : (seg.x_end !== null ? seg.x_end : _hb.virtualX);
        if (segEnd < minX) {
            cutIdx = i;
        } else {
            break;
        }
    }
    if (cutIdx > 0) {
        _hb.timeline = _hb.timeline.slice(cutIdx);
        _hb._histMinReached = false;
    }
}

// Re-drop carried-forward (unconfirmed) leftover bricks onto the current live
// flatline right after a block, widening it to fit them. Reuses each brick's own
// object (color/size/txid/fee preserved): same mempool txs, moved to the new
// mempool segment with a staggered drop-in. Density mirrors live/history: ~10
// bricks per 5px column, skip a column once it hits the 35%-height cap.
function placeCarriedLeftovers(leftovers, staggerSecs) {
    var _hb = getState();
    if (!_hb || !leftovers || leftovers.length === 0) return;
    var seg = _hb.timeline[_hb.timeline.length - 1];
    if (!seg || seg.type !== 'flatline' || seg.x_end !== null) return;
    ensureSegMaps(seg);

    // Filter out any txids already placed on `seg` (genuine post-block arrivals)
    // so we never duplicate or lose them.
    var filtered = [];
    for (var fi = 0; fi < leftovers.length; fi++) {
        var lf = leftovers[fi];
        if (!lf || lf._removed) continue;
        if (_hb._removedTxids && lf.txid && _hb._removedTxids.has(lf.txid)) continue;
        if (lf.txid && seg._blipsByTxid.has(lf.txid)) continue;
        filtered.push(lf);
    }
    if (filtered.length === 0) return;

    // Drop-in sweep duration: the reveal sequencer passes the (rate-based) relay
    // duration so the bricks appear in step with the camera; the non-reveal path
    // uses 2.5s; 0 finalizes instantly (abort / new block).
    var stagger = staggerSecs !== undefined ? staggerSecs : 2.5;
    var TARGET_COL_H = 65;
    var maxStack = (_hb.height || 400) * 0.35;
    var softMaxStack = Math.min(maxStack, 160);
    // Preserve deterministic relative left-to-right order: unknown-time first
    // sorted by txid, then ascending (firstSeen, txid).
    var ordered = filtered.slice().sort(function(a, b) {
        var aKnown = (a.firstSeen || 0) > 0 ? 1 : 0;
        var bKnown = (b.firstSeen || 0) > 0 ? 1 : 0;
        if (aKnown !== bKnown) return aKnown - bKnown;
        if (aKnown === 1 && a.firstSeen !== b.firstSeen) return a.firstSeen - b.firstSeen;
        var at = a.txid || '';
        var bt = b.txid || '';
        return at < bt ? -1 : (at > bt ? 1 : 0);
    });

    var totalBrickH = 0;
    var unknownBrickH = 0;
    for (var ti = 0; ti < ordered.length; ti++) {
        var oCheck = ordered[ti];
        if (!oCheck || oCheck._removed) continue;
        if (_hb._removedTxids && oCheck.txid && _hb._removedTxids.has(oCheck.txid)) continue;
        var bh0 = (oCheck.brickH || 6);
        totalBrickH += bh0;
        if (!oCheck.firstSeen) unknownBrickH += bh0;
    }
    var numCols = Math.max(1, Math.ceil(totalBrickH / TARGET_COL_H));
    var width = Math.max(30, numCols * 5 + 15);

    // If genuine post-block bricks ALREADY landed on `seg` during a slow block_details wait,
    // shift them right so they begin cleanly at `carryEndX + 10` (removing any dead empty
    // gap before them while preserving their relative spacing and never duplicating any tx).
    var baseStartGx = Math.ceil(seg.x_start / 5) * 5;
    var usableSpan = Math.max(5, width - 15);
    var usableCols = Math.max(1, Math.floor(usableSpan / 5));
    var carryEndX = baseStartGx + usableCols * 5;
    var hasExistingPost = seg._blipsByTxid.size > 0;
    if (hasExistingPost) {
        var minPostGx = Infinity;
        var maxPostGx = -Infinity;
        for (var mi = 0; mi < seg.blips.length; mi++) {
            var mb = seg.blips[mi];
            if (!mb || mb._removed || mb.fadeStart > 0) continue;
            var mgx = mb.gridX !== undefined ? mb.gridX : mb.x;
            if (mgx < minPostGx) minPostGx = mgx;
            if (mgx > maxPostGx) maxPostGx = mgx;
        }
        var targetPostStart = carryEndX + 10;
        var shiftCols = isFinite(minPostGx)
            ? Math.ceil((targetPostStart - minPostGx) / 5) * 5
            : Math.ceil(width / 5) * 5;
        seg._colHeights = {};
        seg._colBlips = new Map();
        seg._sortedByX = false;
        for (var ei = 0; ei < seg.blips.length; ei++) {
            var eb = seg.blips[ei];
            if (!eb || eb._removed) continue;
            eb.x = (eb.x || 0) + shiftCols;
            if (eb.fadeStart === 0) {
                eb.gridX = (eb.gridX !== undefined ? eb.gridX : eb.x) + shiftCols;
                eb.x = eb.gridX;
                var ecol = seg._colBlips.get(eb.gridX);
                if (!ecol) { ecol = []; seg._colBlips.set(eb.gridX, ecol); }
                ecol.push(eb);
                var esy = seg._colHeights[eb.gridX] || 0;
                eb.stackY = esy;
                eb.height = esy + (eb.brickH || 0);
                if (stagger > 0) {
                    var postWaveSpan = Math.max(0.4, stagger * 0.78);
                    var postJit = ((txidSkylineOffset(eb.txid || '') + 1) * 0.5) * 0.6;
                    eb.isDrop = true;
                    eb.timestamp = (Date.now() / 1000) + 0.35 + postWaveSpan + 0.15 + postJit;
                }
                seg._colHeights[eb.gridX] = eb.height;
            }
        }
        var shiftedMaxX = isFinite(maxPostGx) ? (maxPostGx + shiftCols + 15) : (seg.x_start + width + 15);
        _hb.virtualX = Math.max(shiftedMaxX, seg.x_start + width + 15);
    } else {
        // Set the geometry UP FRONT (before placing), matching the exact carried
        // backlog footprint so no empty void appears between carried bricks and live head.
        _hb.virtualX = Math.max(_hb.virtualX, seg.x_start + width);
    }
    // If an inspecting viewer (!autoFollow, e.g. zoomed way in at 34x on the live
    // mempool) was stranded in empty space past the new live head when mining +
    // flatline compression shrank the mempool, pull their viewOffset back to the
    // new live head while preserving their exact zoom & paused state. (Viewers
    // inspecting older history left of seg.x_start are never touched.)
    if (!_hb.autoFollow && !_hb._reveal && !_hb._suppressLiveVisuals && _hb.width > 0 && _hb.zoom > 0) {
        var visW = _hb.width / _hb.zoom;
        if (_hb.viewOffset > _hb.virtualX - visW * 0.25) {
            _hb.viewOffset = _hb.virtualX - _hb.width * HEAD_POSITION_FRAC / _hb.zoom;
        }
    }
    var now = Date.now() / 1000;

    if (_hb._carryRaf) { cancelAnimationFrame(_hb._carryRaf); _hb._carryRaf = null; }
    _hb._pendingCarry = null;
    _hb._pendingCarryByTxid = null;
    var CARRY_CHUNK = 4000;
    seg._carryEndX = carryEndX;
    var denomH = Math.max(1, totalBrickH);
    var unknownCols = unknownBrickH > 0
        ? Math.max(1, Math.min(usableCols, Math.round((unknownBrickH / denomH) * usableCols)))
        : 0;
    var knownStartCol = unknownCols;
    var knownCols = Math.max(1, usableCols - knownStartCol);
    var localSpreadCols = Math.min(Math.max(12, Math.floor(usableCols * 0.08)), 160);
    var colBuckets = new Array(usableCols);
    var cumH = 0;
    for (var pi = 0; pi < ordered.length; pi++) {
        var oSrc = ordered[pi];
        if (!oSrc || oSrc._removed) continue;
        if (_hb._removedTxids && oSrc.txid && _hb._removedTxids.has(oSrc.txid)) continue;
        var oH = oSrc.brickH || 6;
        var oCol;
        if (!oSrc.firstSeen && unknownCols > 0) {
            oCol = Math.floor(((txidSkylineOffset(oSrc.txid) + 1) * 0.5) * unknownCols);
            if (oCol >= unknownCols) oCol = unknownCols - 1;
        } else {
            var baseCol = Math.min(usableCols - 1, Math.floor((cumH * usableCols) / denomH));
            var jitter = Math.round(txidSkylineOffset(oSrc.txid) * localSpreadCols);
            oCol = baseCol + jitter;
            if (oCol < knownStartCol) oCol = knownStartCol + (knownStartCol - oCol - 1);
            if (oCol >= usableCols) oCol = (usableCols - 1) - (oCol - (usableCols - 1));
            oCol = Math.max(knownStartCol, Math.min(usableCols - 1, oCol));
            if (knownCols <= 1) oCol = Math.min(usableCols - 1, baseCol);
        }
        cumH += oH;
        var yKey = oSrc.bobPhase || 0;
        var cBucket = colBuckets[oCol];
        if (!cBucket) cBucket = colBuckets[oCol] = [];
        cBucket.push({ src: oSrc, col: oCol, yKey: yKey });
    }
    var plannedCarry = [];
    for (var bc = 0; bc < usableCols; bc++) {
        var bList = colBuckets[bc];
        if (!bList) continue;
        if (bList.length > 1) {
            bList.sort(function(a, b) { return a.yKey - b.yKey; });
        }
        for (var bi2 = 0; bi2 < bList.length; bi2++) {
            plannedCarry.push(bList[bi2]);
        }
    }

    var pendingCarryByTxid = new Map();
    for (var pci = 0; pci < plannedCarry.length; pci++) {
        var pcSrc = plannedCarry[pci].src;
        if (pcSrc && pcSrc.txid) pendingCarryByTxid.set(pcSrc.txid, pcSrc);
    }

    var i = 0;
    var placed = 0;
    var carryRec = { seg: seg, items: plannedCarry, nextIdx: 0 };
    _hb._pendingCarry = carryRec;
    _hb._pendingCarryByTxid = pendingCarryByTxid;
    var placeChunk = function() {
        var s = getState();
        if (!s) return;
        var curSeg = carryRec.seg;
        var liveTl = (s._viewMode === 'history' && s._liveBackup) ? s._liveBackup.timeline : s.timeline;
        if (!curSeg || liveTl[liveTl.length - 1] !== curSeg || curSeg.x_end !== null) {
            s._carryRaf = null;
            if ((!curSeg || !curSeg._pendingForHash) && s._pendingCarry === carryRec) {
                s._pendingCarry = null;
                s._pendingCarryByTxid = null;
            }
            return;
        }
        ensureSegMaps(curSeg);
        var chunkStartMs = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
        var budgetMs = (s.width && s.width < 640) ? 5 : 10;
        var end = Math.min(i + CARRY_CHUNK, plannedCarry.length);
        var span = Math.max(1, usableSpan);
        var startGridX = Math.ceil(curSeg.x_start / 5) * 5;
        curSeg._carryEndX = startGridX + usableCols * 5;
        var chunkDone = 0;
        for (; i < end; i++) {
            if (stagger > 0 && chunkDone > 0 && (chunkDone & 255) === 0) {
                var nowMs = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
                if (nowMs - chunkStartMs >= budgetMs) break;
            }
            chunkDone++;
            var item = plannedCarry[i];
            var src = item.src;
            if (src && src.txid && s._pendingCarryByTxid) {
                s._pendingCarryByTxid.delete(src.txid);
            }
            if (!src || src._removed) continue;
            if (s._removedTxids && src.txid && s._removedTxids.has(src.txid)) continue;
            if (src.txid && curSeg._blipsByTxid.has(src.txid)) continue;
            if (s._deferredDetails && src.txid && s._deferredDetails.has(src.txid)) {
                updateBlipMetadata(src, s._deferredDetails.get(src.txid));
                s._deferredDetails.delete(src.txid);
            }

            var brickH = src.brickH || 6;
            var gx = startGridX + item.col * 5;
            var stackY = curSeg._colHeights[gx] || 0;

            if (stackY > 0 && stackY + brickH > softMaxStack) {
                var minZoneX = startGridX + (src.firstSeen ? knownStartCol : 0) * 5;
                var maxZoneX = startGridX + (src.firstSeen ? usableCols - 1 : Math.max(0, unknownCols - 1)) * 5;
                var bestX = gx;
                var bestY = stackY;
                for (var step = 1; step <= 10; step++) {
                    var c1 = gx + step * 5;
                    if (c1 <= maxZoneX) {
                        var h1 = curSeg._colHeights[c1] || 0;
                        if (h1 < bestY) { bestX = c1; bestY = h1; if (bestY + brickH <= softMaxStack) break; }
                    }
                    var c2 = gx - step * 5;
                    if (c2 >= minZoneX) {
                        var h2 = curSeg._colHeights[c2] || 0;
                        if (h2 < bestY) { bestX = c2; bestY = h2; if (bestY + brickH <= softMaxStack) break; }
                    }
                }
                gx = bestX;
                stackY = bestY;
            }

            src.x = gx;
            src.gridX = gx;
            src.stackY = stackY;
            src.height = brickH + stackY;
            src.fadeStart = 0;
            src.particles = null;
            src.absorbTargetX = undefined;
            src.absorbOriginX = undefined;
            src.isDrop = stagger > 0;
            if (stagger > 0) {
                var xFrac = Math.min(1, Math.max(0, (gx - curSeg.x_start) / span));
                // Match the camera's smoothstep glide with a gentle 0.35s lead-in and
                // wide per-brick + vertical feathering so bricks rain in smoothly instead
                // of bursting ahead of the camera on frame 0.
                var smoothX = xFrac * xFrac * (3 - 2 * xFrac);
                var waveFrac = xFrac * 0.55 + smoothX * 0.45;
                var rainJitter = ((txidSkylineOffset(src.txid) + 1) * 0.5) * 0.95;
                var stackDelay = stackY * 0.0042 + rainJitter;
                var waveSpan = Math.max(0.4, stagger * 0.78);
                src.timestamp = now + 0.35 + waveFrac * waveSpan + stackDelay;
            } else {
                src.timestamp = now;
            }

            registerBlipInSeg(curSeg, src);
            if (s._pinnedTxid && src.txid === s._pinnedTxid) {
                s._pinnedBlip = src;
            }
            placed++;
        }
        if (s._pendingCarry === carryRec) {
            s._pendingCarry.nextIdx = i;
        }
        if (i < plannedCarry.length) {
            s._carryRaf = requestAnimationFrame(placeChunk);
        } else {
            s._carryRaf = null;
            if (s._pendingCarry === carryRec) {
                s._pendingCarry = null;
                s._pendingCarryByTxid = null;
            }
            maybeClearRemovedTxids(s);
            if (s._pinnedTxid) {
                s._pinnedBlip = curSeg._blipsByTxid.get(s._pinnedTxid) || null;
            }
        }
    };
    placeChunk();
}
// Exposed for the block-reveal sequencer (heartbeat-render.js), which re-lays the
// carried leftovers during the relay beat with a stagger matched to the camera follow.
window._hbPlaceCarried = placeCarriedLeftovers;

// Toggle LOD (density-column) rendering vs always-individual-bricks, for A/B
// testing look vs perf. Persists to localStorage so it survives reloads. Call
// from the console: window._hbToggleLod().
window._hbToggleLod = function() {
    var s = getState();
    if (!s) return;
    s._lodEnabled = !s._lodEnabled;
    try { localStorage.setItem('hb_lod', s._lodEnabled ? 'on' : 'off'); } catch (e) {}
    console.log('[heartbeat] LOD ' + (s._lodEnabled
        ? 'ENABLED (density columns when zoomed out <0.3x and >100k bricks)'
        : 'DISABLED — individual bricks always'));
    return s._lodEnabled;
};

// Toggle the cinematic block reveal on/off. Off = the instant flow (spike appears,
// harvest, mempool re-lays, no camera hijack; follow state respected). Persists to
// localStorage. Call from the console: window._hbToggleReveal().
// Fire the block cue by hand, for testing without waiting ~10 minutes for a block.
// Enable the sound with the speaker button FIRST: a console call is not a user gesture,
// so a context created here would start suspended and stay silent.
//
// To verify the hidden-tab path (the reason the cue is fired before the hidden branch in
// heartbeat-sse.js), schedule it and switch away:
//   setTimeout(window._hbPlayBlockCue, 5000)
window._hbPlayBlockCue = function() {
    playBlockCue();
};

// Tune the cue live, without a rebuild. Values are not persisted, so a reload restores
// the module defaults. Example:
//   window._hbSoundTune({ MASTER_GAIN: 0.8, CHIME_HZ: 330 })
//   window._hbPlayBlockCue()
window._hbSoundTune = function(overrides) {
    var r = tuneSound(overrides);
    if (r.unknown.length) console.warn('[heartbeat] unknown sound params (typo?):', r.unknown);
    console.log('[heartbeat] applied:', r.applied);
    return r.applied;
};

// Print the current tuning. Also prints a paste-ready _hbSoundTune(...) of just what you
// have changed, which is the thing to hand over when baking a session into the constants
// at the top of heartbeat-audio.js.
window._hbSoundValues = function() {
    var s = soundSettings();
    console.table(s.values);
    var keys = Object.keys(s.changed);
    if (keys.length === 0) {
        console.log('[heartbeat] all values are at their defaults');
    } else {
        console.log('[heartbeat] changed from defaults:\n_hbSoundTune(' +
            JSON.stringify(s.changed, null, 2) + ')');
    }
    return s.values;
};

window._hbSoundPreset = function(name) {
    var r = setSoundPreset(name);
    if (!r.ok) {
        console.warn('[heartbeat] unknown sound preset:', name, 'available:', r.available);
    } else {
        console.log('[heartbeat] auditioning sound preset:', r.preset);
    }
    return r;
};

// Block audio cue on/off, persisted in localStorage. Mirrors the speaker button in the
// control bar. Call from the console: window._hbToggleSound().
window._hbToggleSound = function() {
    var on = handleControlClick('sound');
    console.log('[heartbeat] block sound ' + (on ? 'ENABLED' : 'DISABLED'));
    return on;
};

window._hbToggleReveal = function() {
    var s = getState();
    if (!s) return;
    s._revealEnabled = !s._revealEnabled;
    try { localStorage.setItem('hb_reveal', s._revealEnabled ? 'on' : 'off'); } catch (e) {}
    console.log('[heartbeat] cinematic reveal ' + (s._revealEnabled ? 'ENABLED' : 'DISABLED (instant flow)'));
    return s._revealEnabled;
};

window.setHeartbeatLandmarks = function(json) {
    try {
        var parsed = JSON.parse(json);
        if (parsed && typeof parsed === 'object') {
            window._hbLandmarks = parsed;
        }
    } catch (e) {
        // ignore malformed landmark map
    }
};

window.updateHeartbeatLive = function(json) {
    var _hb = getState();
    if (!_hb) return;
    try {
        var data = JSON.parse(json);
        if (data.next_block_fee !== undefined) _hb.nextBlockFee = data.next_block_fee;
        if (data.mempool_mb !== undefined) _hb.mempoolMB = data.mempool_mb;
        // Don't update lastBlockTime from LiveStats: it resets the "Last block"
        // timer before the spike appears. lastBlockTime is set exclusively by
        // pushHeartbeatBlocks when the SSE block event arrives with complete data.
        if (data.hashrate_eh !== undefined) _hb.hashrateEH = data.hashrate_eh;
        if (data.mempool_min_fee !== undefined) _hb.mempoolMinFee = data.mempool_min_fee;
        if (data.difficulty !== undefined) _hb.difficulty = data.difficulty;
        if (data.block_height !== undefined) _hb.blockHeight = data.block_height;
    } catch (e) {
        // silently ignore malformed live JSON
    }
};

// Expose handleControlClick for HTML buttons
window.handleControlClick = handleControlClick;

window.destroyHeartbeat = function() {
    var _hb = getState();
    if (!_hb) return;
    _hb._fetchGen = (_hb._fetchGen || 0) + 1;
    _hb._histFetchLeft = false;
    _hb._histFetchRight = false;
    _hb._histMinReached = false;
    _hb._histMaxReached = false;
    _hb._histTipHeight = 0;
    _hb._histFetchCooldownUntil = 0;
    stopMomentum();
    if (_hb.rafId) cancelAnimationFrame(_hb.rafId);
    if (_hb._flashTimer) clearTimeout(_hb._flashTimer);
    if (_hb.resizeObs) _hb.resizeObs.disconnect();
    // Cancel the pending SSE reconnect timeout so it can't fire connectOwnFeed
    // after teardown (its closure's captured _hb stays truthy post-destroy).
    if (_hb._sseRetryTimer) { clearTimeout(_hb._sseRetryTimer); _hb._sseRetryTimer = null; }
    if (_hb._historyRaf) { cancelAnimationFrame(_hb._historyRaf); _hb._historyRaf = null; }
    if (_hb._carryRaf) { cancelAnimationFrame(_hb._carryRaf); _hb._carryRaf = null; }
    _hb._pendingCarry = null;
    _hb._pendingCarryByTxid = null;
    if (_hb._sse) {
        try { _hb._sse.close(); } catch(e) {}
    }
    if (_hb.ws) {
        try { _hb.ws.close(); } catch(e) {}
    }
    if (_hb._listeners) {
        for (var i = 0; i < _hb._listeners.length; i++) {
            var l = _hb._listeners[i];
            l.target.removeEventListener(l.evt, l.fn, l.opts);
        }
    }
    window.removeEventListener('beforeunload', _hbBeforeUnload);
    document.removeEventListener('visibilitychange', _hbVisibilityChange);
    if (_hb._fullscreenHandler) {
        document.removeEventListener('fullscreenchange', _hb._fullscreenHandler);
        document.removeEventListener('webkitfullscreenchange', _hb._fullscreenHandler);
    }
    if (window._seenNotableTxids) window._seenNotableTxids.clear();
    setState(null);
};

// Clean up on page unload
function _hbBeforeUnload() {
    if (window.destroyHeartbeat) window.destroyHeartbeat();
}
window.addEventListener('beforeunload', _hbBeforeUnload);

// When tab regains focus, preserve existing timeline and resume live flow.
// If hidden for > 5 minutes, do a full reset instead of trying to reconcile
// stale timeline state (blocks bunched up, flatline position wrong).
// Replay a single block (from _blockQueue or a catch-up fetch) as a live block.
// Harvest ON so the mempool is carried forward (top-fee shatters into the spike,
// the rest re-lays on the new flatline): same as a live block, but INSTANT (this
// isn't processLiveBlock, so no cinematic reveal is armed). Without harvest the fee
// threshold is -Infinity and the WHOLE mempool shatters into the spike, leaving the
// new flatline empty: a block you were away for would wipe the mempool.
function replayQueuedBlock(qb) {
    var _hb = getState();
    if (_hb) _hb._harvestLive = true;
    window.pushHeartbeatBlocks(JSON.stringify([{
        hash: qb.hash || null,
        height: qb.height,
        timestamp: qb.timestamp || Math.floor(Date.now() / 1000),
        tx_count: qb.tx_count || qb.confirmed_count || 0,
        total_fees: qb.total_fees || 0,
        size: qb.size || 0,
        weight: qb.weight || 0,
        miner: qb.miner || null
    }]), false);
}

// Replay blocks queued while the tab was hidden.
// Carries surviving bricks in memory across intermediate blocks (sized
// proportionally via historyFlatlineWidth) and runs placeCarriedLeftovers ONCE
// with stagger=0 (instant, settled, no rain-down after returning to a hidden tab).
// Optional `extraPreBlockTxs` carries pre-block txs buffered in `_hiddenTxMap`
// into the carried backlog instead of clamping them onto `gapStart`.
function catchUpBlocks(queued, extraPreBlockTxs) {
    var _hb = getState();
    if (!_hb || !queued || queued.length === 0) return 0;

    var maxH = 0;
    for (var hi = _hb.timeline.length - 1; hi >= 0; hi--) {
        if (_hb.timeline[hi].type === 'block') { maxH = _hb.timeline[hi].height; break; }
    }

    // Deduplicate by height keeping only the last queued entry per height so a
    // replaced orphan can never replay alongside its replacement.
    var byHeight = new Map();
    for (var qi = 0; qi < queued.length; qi++) {
        var candB = queued[qi];
        if (candB && candB.height && candB.height > maxH) {
            byHeight.set(candB.height, candB);
        }
    }
    var q = Array.from(byHeight.values()).sort(function(a, b) { return a.height - b.height; });
    if (q.length === 0) return 0;

    var hasExtra = Array.isArray(extraPreBlockTxs) && extraPreBlockTxs.length > 0;
    var carried = null;
    var newestTs = 0;
    _hb._instantCarry = true;
    try {
        for (var i = 0; i < q.length; i++) {
            var qb = q[i];
            if (i > 0) {
                var hasPendingSlot = false;
                if (qb.hash && _hb.timeline) {
                    for (var psi = _hb.timeline.length - 1; psi >= 0; psi--) {
                        if (_hb.timeline[psi].type === 'pending_block' && _hb.timeline[psi].hash === qb.hash) {
                            hasPendingSlot = true;
                            break;
                        }
                    }
                }
                if (!hasPendingSlot) {
                    var prevWall = effectiveBlockWallTime(q[i - 1], Math.floor(Date.now() / 1000));
                    var curWall = effectiveBlockWallTime(qb, Math.floor(Date.now() / 1000));
                    var gapSecs = (prevWall > 0 && curWall >= prevWall)
                        ? (curWall - prevWall)
                        : Math.max(0, (qb.timestamp || 0) - (q[i - 1].timestamp || 0));
                    _hb.virtualX += historyFlatlineWidth(gapSecs);
                }
            }
            if (i < q.length - 1) {
                // Defer chunked placement on intermediate blocks; accumulate carried bricks in memory.
                _hb._deferCarryList = [];
                replayQueuedBlock(qb);
                var stepCarried = _hb._deferCarryList;
                _hb._deferCarryList = null;
                if (carried === null) {
                    carried = stepCarried;
                } else {
                    // Filter previously carried bricks against this intermediate block's exact inclusions.
                    var conf = (qb.hash && _hb._exactConfirmedByHash) ? _hb._exactConfirmedByHash.get(qb.hash) : null;
                    if (conf && conf.set && conf.set.size > 0) {
                        carried = carried.filter(function(b) { return b && !b._removed && (!b.txid || !conf.set.has(b.txid)); });
                    }
                    if (stepCarried && stepCarried.length > 0) {
                        carried = carried.concat(stepCarried);
                    }
                }
            } else if (carried !== null || hasExtra) {
                // Final block after deferred intermediate blocks (or with extra pre-block hidden txs):
                // filter against final block's exact inclusions, merge extra pre-block txs, and carry ONCE.
                var finalConf = (qb.hash && _hb._exactConfirmedByHash) ? _hb._exactConfirmedByHash.get(qb.hash) : null;
                if (carried !== null && finalConf && finalConf.set && finalConf.set.size > 0) {
                    carried = carried.filter(function(b) { return b && !b._removed && (!b.txid || !finalConf.set.has(b.txid)); });
                }
                _hb._deferCarryList = [];
                replayQueuedBlock(qb);
                var tailCarried = _hb._deferCarryList;
                _hb._deferCarryList = null;
                if (carried === null) carried = [];
                if (tailCarried && tailCarried.length > 0) {
                    carried = carried.concat(tailCarried);
                }
                if (hasExtra) {
                    var seenIds = new Set();
                    for (var si = 0; si < carried.length; si++) {
                        if (carried[si] && carried[si].txid) seenIds.add(carried[si].txid);
                    }
                    var nowSec = Date.now() / 1000;
                    for (var ei = 0; ei < extraPreBlockTxs.length; ei++) {
                        var etx = extraPreBlockTxs[ei];
                        if (!etx || !etx.txid || seenIds.has(etx.txid)) continue;
                        if (_hb._removedTxids && _hb._removedTxids.has(etx.txid)) continue;
                        seenIds.add(etx.txid);
                        var eBlip = buildBlip(etx, 0, 0, 0, nowSec, false);
                        carried.push(eBlip);
                        if (eBlip.notableType && window._notableFeed) {
                            window._notableFeed(etx);
                        }
                    }
                }
                if (carried.length > 0) {
                    placeCarriedLeftovers(carried, 0);
                }
            } else {
                replayQueuedBlock(qb);
            }
            // Prefer node observation time over miner header timestamp for timeline
            // partitioning, since Bitcoin consensus allows block timestamps up to
            // +2h in the future (e.g. SpiderPool stamping #977,544 +4m44s ahead).
            var qbObs = (qb.observed_at && qb.observed_at > 0)
                ? qb.observed_at
                : Math.min(qb.timestamp || 0, Math.floor(Date.now() / 1000));
            newestTs = qbObs || newestTs;
        }
    } finally {
        _hb._instantCarry = false;
    }
    return newestTs;
}

window._hbDrainBlockQueue = function() {
    var _hb = getState();
    if (!_hb || document.hidden) return;
    if (!_hb._blockQueue || _hb._blockQueue.length === 0) return;
    var queued = _hb._blockQueue.slice();
    _hb._blockQueue = [];
    catchUpBlocks(queued);
};

// Recover the timeline smoothly after a hidden-tab or rAF stall without ever
// wiping state or resetting an inspecting viewer's camera or pinned tx.
// The Rust/WASM v2 client handles stream resync automatically.
function recoverTimeline(gapSecs) {
    var _hb = getState();
    if (!_hb) return;
    withLiveTimeline(_hb, function() {
        _recoverTimelineInner(_hb, gapSecs);
    });
}

function _recoverTimelineInner(_hb, gapSecs) {
    var now = Date.now() / 1000;

    // Finalize any in-progress reveal; if the viewer was following LIVE, restore
    // their entry zoom and snap cleanly back to the live head rather than leaving
    // them stranded zoomed in on an old block spike.
    if (_hb._reveal && window._hbEndReveal) window._hbEndReveal(true);

    var queuedBlocks = (_hb._blockQueue || []).slice();
    _hb._blockQueue = [];

    // Check whether any queued block is actually newer than our current timeline tip.
    var maxH = 0;
    for (var hi = _hb.timeline.length - 1; hi >= 0; hi--) {
        if (_hb.timeline[hi].type === 'block') { maxH = _hb.timeline[hi].height; break; }
    }
    var newQueued = queuedBlocks.filter(function(b) { return b && b.height && b.height > maxH; });

    var countBufferedHidden = function() {
        return (_hb._hiddenTxMap ? _hb._hiddenTxMap.size : 0)
            + (_hb._pendingAdds ? _hb._pendingAdds.size : 0)
            + (_hb._txBatchQueue ? _hb._txBatchQueue.length : 0);
    };

    if (newQueued.length > 0) {
        // Partition _hiddenTxMap BEFORE calling catchUpBlocks:
        // - pre-block txs (first_seen > 0 && first_seen < newestQueuedTs) are dropped if
        //   confirmed in any queued block and otherwise merged into the carried backlog;
        // - post-block txs (first_seen >= newestQueuedTs) stay in _hiddenTxMap for
        //   drainHiddenTxsAcrossGap across the post-block elapsed span.
        var newestQueuedTs = 0;
        var queuedConfirmedSets = [];
        for (var qi = 0; qi < newQueued.length; qi++) {
            var qb = newQueued[qi];
            var qbWall = (qb.observed_at && qb.observed_at > 0)
                ? qb.observed_at
                : Math.min(qb.timestamp || 0, Math.floor(now));
            if (qbWall > newestQueuedTs) newestQueuedTs = qbWall;
            if (qb.hash && _hb._exactConfirmedByHash && _hb._exactConfirmedByHash.has(qb.hash)) {
                var cEnt = _hb._exactConfirmedByHash.get(qb.hash);
                if (cEnt && cEnt.set && cEnt.set.size > 0) queuedConfirmedSets.push(cEnt.set);
            }
        }
        var preBlockHidden = [];
        if (_hb._hiddenTxMap && _hb._hiddenTxMap.size > 0 && newestQueuedTs > 0) {
            var toDelete = [];
            _hb._hiddenTxMap.forEach(function(htx, htxid) {
                if (htx && htx.first_seen > 0 && htx.first_seen < newestQueuedTs) {
                    toDelete.push(htxid);
                    if (_hb._removedTxids && _hb._removedTxids.has(htxid)) return;
                    for (var cs = 0; cs < queuedConfirmedSets.length; cs++) {
                        if (queuedConfirmedSets[cs].has(htxid)) return;
                    }
                    preBlockHidden.push(htx);
                }
            });
            for (var di = 0; di < toDelete.length; di++) {
                _hb._hiddenTxMap.delete(toDelete[di]);
            }
        }

        // Replay hidden blocks FIRST so all pre-block bricks (including pre-block
        // hidden arrivals) carry cleanly onto the post-block flatline, then advance
        // only by real elapsed time since the newest block (or volume width if
        // many txs arrived) and map timestamps over the ACTUAL post-block window.
        var newestBlockTs = catchUpBlocks(newQueued, preBlockHidden);
        var gapStart = _hb.virtualX;
        var actualPostElapsed = newestBlockTs > 0 ? Math.max(0, now - newestBlockTs) : Math.max(0, gapSecs);
        var postBuffered = countBufferedHidden();
        var postSpanPx;
        if (postBuffered > 0) {
            var postVolPx = Math.max(25, Math.ceil(postBuffered / 8) * 5);
            var postTimePx = Math.min(actualPostElapsed, 300) * FLATLINE_PX_PER_SEC;
            postSpanPx = Math.max(postVolPx, Math.min(postTimePx, Math.max(postVolPx * 2.5, 100)));
        } else {
            postSpanPx = Math.min(actualPostElapsed, 15) * (FLATLINE_PX_PER_SEC * 0.22);
        }
        if (postSpanPx > 0) {
            _hb.virtualX += postSpanPx;
        }
        var winStart = newestBlockTs > 0 ? newestBlockTs : (now - actualPostElapsed);
        drainHiddenTxsAcrossGap(gapStart, Math.max(gapStart, _hb.virtualX - 5), winStart, now);
    } else {
        // No blocks while away: advance the live head proportionally to buffered
        // arrivals so quiet background tabs never carve a 1,500px sparse desert
        // while busy tabs still spread smoothly across the actual elapsed window.
        var gapStartNoBlk = _hb.virtualX;
        var actualGap = gapSecs > 0 ? gapSecs : 0;
        var noBlkBuffered = countBufferedHidden();
        var spanPx;
        if (noBlkBuffered > 0) {
            var volPx = Math.max(25, Math.ceil(noBlkBuffered / 8) * 5);
            var timePx = Math.min(actualGap, 300) * FLATLINE_PX_PER_SEC;
            spanPx = Math.max(volPx, Math.min(timePx, Math.max(volPx * 2.5, 100)));
        } else {
            spanPx = Math.min(actualGap, 15) * (FLATLINE_PX_PER_SEC * 0.22);
        }
        if (spanPx > 0) {
            _hb.virtualX += spanPx;
        }
        drainHiddenTxsAcrossGap(gapStartNoBlk, Math.max(gapStartNoBlk, _hb.virtualX - 5), now - actualGap, now);
    }

    if (!_hb._suppressLiveVisuals && _hb.autoFollow && !_hb._pinnedTxid && _hb.width > 0 && _hb.zoom > 0) {
        _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
    }

    _hb.lastFrameTime = now;
}
window._hbRecoverTimeline = function(gapSecs) { recoverTimeline(gapSecs); };

// ── Bidirectional Infinite Historical Block Lazy-Loading ──────────────────

var MAX_TIMELINE_SEGS = 12000;

export function getTimelineBlockBounds(timeline) {
    var minH = -1, minIdx = -1, maxH = -1, maxIdx = -1;
    if (!timeline || timeline.length === 0) {
        return { minH: minH, minIdx: minIdx, maxH: maxH, maxIdx: maxIdx };
    }
    for (var i = 0; i < timeline.length; i++) {
        if (timeline[i].type === 'block') {
            minH = timeline[i].height;
            minIdx = i;
            break;
        }
    }
    for (var j = timeline.length - 1; j >= 0; j--) {
        if (timeline[j].type === 'block') {
            maxH = timeline[j].height;
            maxIdx = j;
            break;
        }
    }
    return { minH: minH, minIdx: minIdx, maxH: maxH, maxIdx: maxIdx };
}

export function trimFarHistoricalSegments(_hb, viewLeft, viewRight) {
    if (!_hb || !_hb.timeline || _hb.timeline.length <= MAX_TIMELINE_SEGS || _hb._reveal) return;
    var tl = _hb.timeline;
    var excess = tl.length - MAX_TIMELINE_SEGS;
    var viewSpan = Math.max(100, isFinite(viewRight - viewLeft) ? (viewRight - viewLeft) : 1200);
    var farDist = 3 * viewSpan;
    var leftCutLimit = viewLeft - farDist;
    var rightCutLimit = viewRight + farDist;

    // Never touch the live open flatline (or pending_block) at the right end in 'live' mode.
    var canTrimRight = (_hb._viewMode === 'history');
    var rightEdge = tl[tl.length - 1].x_end !== null ? tl[tl.length - 1].x_end : _hb.virtualX;
    var leftDist = viewLeft - tl[0].x_start;
    var rightDist = rightEdge - viewRight;
    var rightFirst = canTrimRight && rightDist > leftDist;

    var trimLeft = function(cutLimit) {
        if (excess <= 0) return;
        var cut = 0;
        var maxCut = Math.min(excess, Math.max(0, _hb.timeline.length - 4));
        while (cut < maxCut) {
            var seg = _hb.timeline[cut];
            if (seg.x_end === null || seg.type === 'pending_block' || seg._pendingForHash) break;
            if (seg.x_end < cutLimit) cut++;
            else break;
        }
        if (cut > 0) {
            _hb.timeline = _hb.timeline.slice(cut);
            excess -= cut;
            _hb._histMinReached = false;
            if (_hb._viewMode === 'history' && _hb._historyMeta) {
                var bL = getTimelineBlockBounds(_hb.timeline);
                if (bL.minIdx >= 0) _hb._historyMeta.from_height = bL.minH;
            }
        }
    };

    var trimRight = function(cutLimit) {
        if (!canTrimRight || excess <= 0) return;
        var cut = 0;
        var maxCut = Math.min(excess, Math.max(0, _hb.timeline.length - 4));
        while (cut < maxCut) {
            var seg = _hb.timeline[_hb.timeline.length - 1 - cut];
            if (seg.x_end === null || seg.type === 'pending_block') break;
            if (seg.x_start > cutLimit) cut++;
            else break;
        }
        if (cut > 0) {
            var newLen = _hb.timeline.length - cut;
            // Keep history right edge ending on a block segment so subsequent appends stay alternating
            if (newLen > 1 && _hb.timeline[newLen - 1].type === 'flatline') {
                newLen -= 1;
                cut += 1;
            }
            _hb.timeline = _hb.timeline.slice(0, newLen);
            excess -= cut;
            _hb._histMaxReached = false;
            var newLast = _hb.timeline[_hb.timeline.length - 1];
            if (newLast && newLast.x_end !== null) _hb.virtualX = newLast.x_end;
            if (_hb._viewMode === 'history' && _hb._historyMeta) {
                var bR = getTimelineBlockBounds(_hb.timeline);
                if (bR.maxIdx >= 0) _hb._historyMeta.to_height = bR.maxH;
            }
        }
    };

    if (rightFirst) {
        trimRight(rightCutLimit);
        trimLeft(leftCutLimit);
        if (canTrimRight && excess > 0) {
            trimRight(viewRight);
            trimLeft(viewLeft);
        }
    } else {
        trimLeft(leftCutLimit);
        trimRight(rightCutLimit);
        if (canTrimRight && excess > 0) {
            trimLeft(viewLeft);
            trimRight(viewRight);
        }
    }
}

// Prepend older historical blocks leftward (into negative virtual X as needed)
// without mutating any existing segment, blip, virtualX, or camera viewOffset.
// Works in both 'live' and 'history' modes.
export function prependHistoricalBlocks(_hb, blocks) {
    if (!_hb || !_hb.timeline || _hb.timeline.length === 0) return 0;
    if (!Array.isArray(blocks) || blocks.length === 0) {
        _hb._histMinReached = true;
        return 0;
    }

    var bounds = getTimelineBlockBounds(_hb.timeline);
    var currentMinH = bounds.minIdx >= 0 ? bounds.minH : Infinity;

    var filtered = blocks.filter(function(b) {
        return b && typeof b.height === 'number' && isFinite(b.height) && b.height >= 0 && b.height < currentMinH;
    }).sort(function(a, b) { return a.height - b.height; });

    var dedup = [];
    for (var di = 0; di < filtered.length; di++) {
        if (dedup.length === 0 || filtered[di].height !== dedup[dedup.length - 1].height) {
            dedup.push(filtered[di]);
        }
    }
    filtered = dedup;
    if (filtered.length === 0) {
        _hb._histMinReached = true;
        return 0;
    }

    // In LIVE mode, cap leftward prepends at MAX_TIMELINE_SEGS cleanly instead of
    // growing unbounded or dropping freshly fetched blocks in a refetch loop.
    var firstSeg = _hb.timeline[0];
    var cappedLive = false;
    if (_hb._viewMode !== 'history') {
        var extraLeadFlat = (firstSeg.type === 'block' && firstSeg.height > 0) ? 1 : 0;
        var headroom = Math.floor((MAX_TIMELINE_SEGS - _hb.timeline.length - extraLeadFlat) / 2);
        if (headroom <= 0) {
            _hb._histMinReached = true;
            return 0;
        }
        if (filtered.length > headroom) {
            filtered = filtered.slice(-headroom);
            cappedLive = true;
        }
    }

    var cursor = _hb.timeline[0].x_start;
    var prefixRev = [];

    // If the leftmost segment is a block without a preceding flatline (and not genesis),
    // insert its preceding flatline ending at cursor before prepending older blocks.
    if (firstSeg.type === 'block' && firstSeg.height > 0) {
        var firstRawInter = resolveInterBlockSeconds(firstSeg);
        var newestPre = filtered[filtered.length - 1];
        if (firstRawInter === null && firstSeg.timestamp && newestPre.timestamp && newestPre.height === firstSeg.height - 1) {
            firstRawInter = firstSeg.timestamp - newestPre.timestamp;
        }
        var firstFlatW = historyFlatlineWidth(firstRawInter);
        var firstFlat = createFlatlineSegment(cursor - firstFlatW, cursor);
        firstFlat.inter_block_seconds = firstRawInter;
        firstFlat.color = firstRawInter === null ? UNKNOWN_INTERVAL_COLOR : waitColor(Math.max(0, firstRawInter));
        firstFlat._compressed = true;
        cursor = firstFlat.x_start;
        prefixRev.push(firstFlat);
    }

    var blockSpan = 51 * POINT_WIDTH;
    for (var i = filtered.length - 1; i >= 0; i--) {
        var b = filtered[i];
        var rawInter = resolveInterBlockSeconds(b);
        if (rawInter === null && i > 0 && b.timestamp && filtered[i - 1].timestamp && b.height === filtered[i - 1].height + 1) {
            rawInter = b.timestamp - filtered[i - 1].timestamp;
        }
        var blkInput = rawInter !== b.inter_block_seconds
            ? Object.assign({}, b, { inter_block_seconds: rawInter })
            : b;
        var blkSeg = createBlockSegment(blkInput, cursor - blockSpan);
        if (blkSeg.x_end !== cursor) {
            var actualW = blkSeg.x_end - blkSeg.x_start;
            blkSeg.x_start = cursor - actualW;
            blkSeg.x_end = cursor;
        }
        cursor = blkSeg.x_start;
        prefixRev.push(blkSeg);

        if (b.height > 0) {
            var flatW = historyFlatlineWidth(rawInter);
            var flat = createFlatlineSegment(cursor - flatW, cursor);
            flat.inter_block_seconds = rawInter;
            flat.color = rawInter === null ? UNKNOWN_INTERVAL_COLOR : waitColor(Math.max(0, rawInter));
            flat._compressed = true;
            cursor = flat.x_start;
            prefixRev.push(flat);
        }
    }

    prefixRev.reverse();
    _hb.timeline = prefixRev.concat(_hb.timeline);

    if (filtered[0].height === 0 || cappedLive) {
        _hb._histMinReached = true;
    }

    if (_hb._viewMode === 'history' && _hb._historyMeta) {
        _hb._historyMeta.from_height = filtered[0].height;
        if (typeof window.__hbHistoryModeChange === 'function') {
            window.__hbHistoryModeChange(JSON.stringify(_hb._historyMeta));
        }
    }

    if (_hb.timeline.length > MAX_TIMELINE_SEGS) {
        var vLeft = _hb.viewOffset || 0;
        var vRight = vLeft + (_hb.width || 1200) / Math.max(_hb.zoom || 1, 0.01);
        trimFarHistoricalSegments(_hb, vLeft, vRight);
    }

    return filtered.length;
}

// Append newer historical blocks rightward ('history' mode ONLY).
export function appendHistoricalBlocks(_hb, blocks) {
    if (!_hb || _hb._viewMode !== 'history' || !_hb.timeline || _hb.timeline.length === 0) return 0;
    if (!Array.isArray(blocks) || blocks.length === 0) {
        _hb._histMaxReached = true;
        return 0;
    }

    var bounds = getTimelineBlockBounds(_hb.timeline);
    var currentMaxH = bounds.maxIdx >= 0 ? bounds.maxH : -1;

    var filtered = blocks.filter(function(b) {
        return b && typeof b.height === 'number' && isFinite(b.height) && b.height > currentMaxH;
    }).sort(function(a, b) { return a.height - b.height; });

    var dedup = [];
    for (var di = 0; di < filtered.length; di++) {
        if (dedup.length === 0 || filtered[di].height !== dedup[dedup.length - 1].height) {
            dedup.push(filtered[di]);
        }
    }
    filtered = dedup;
    if (filtered.length === 0) {
        _hb._histMaxReached = true;
        return 0;
    }

    var lastSeg = _hb.timeline[_hb.timeline.length - 1];
    var cursor = lastSeg.x_end !== null ? lastSeg.x_end : _hb.virtualX;
    var prevBlk = bounds.maxIdx >= 0 ? _hb.timeline[bounds.maxIdx] : null;

    for (var i = 0; i < filtered.length; i++) {
        var b = filtered[i];
        var rawInter = resolveInterBlockSeconds(b);
        if (rawInter === null && b.timestamp && prevBlk && prevBlk.timestamp && b.height === prevBlk.height + 1) {
            rawInter = b.timestamp - prevBlk.timestamp;
        }
        var flatW = historyFlatlineWidth(rawInter);
        var flat = createFlatlineSegment(cursor, cursor + flatW);
        flat.inter_block_seconds = rawInter;
        flat.color = rawInter === null ? UNKNOWN_INTERVAL_COLOR : waitColor(Math.max(0, rawInter));
        flat._compressed = true;
        _hb.timeline.push(flat);
        cursor = flat.x_end;

        var blkInput = rawInter !== b.inter_block_seconds
            ? Object.assign({}, b, { inter_block_seconds: rawInter })
            : b;
        var blkSeg = createBlockSegment(blkInput, cursor);
        _hb.timeline.push(blkSeg);
        cursor = blkSeg.x_end;
        prevBlk = blkSeg;
    }

    _hb.virtualX = cursor;

    var lastH = filtered[filtered.length - 1].height;
    if (_hb._histTipHeight > 0 && lastH >= _hb._histTipHeight) {
        _hb._histMaxReached = true;
    }

    if (_hb._historyMeta) {
        _hb._historyMeta.to_height = lastH;
        if (typeof window.__hbHistoryModeChange === 'function') {
            window.__hbHistoryModeChange(JSON.stringify(_hb._historyMeta));
        }
    }

    if (_hb.timeline.length > MAX_TIMELINE_SEGS) {
        var vLeft = _hb.viewOffset || 0;
        var vRight = vLeft + (_hb.width || 1200) / Math.max(_hb.zoom || 1, 0.01);
        trimFarHistoricalSegments(_hb, vLeft, vRight);
    }

    return filtered.length;
}

// Called from drawFrame: triggers seamless leftward (live + history) and
// rightward (history-only) historical block fetches when the viewport nears an edge.
export function maybeFetchMoreBlocks(_hb, viewLeft, viewRight) {
    if (!_hb || _hb._reveal || typeof fetch !== 'function') return;
    if (!_hb.timeline || _hb.timeline.length === 0) return;
    if (!isFinite(viewLeft) || !isFinite(viewRight)) return;

    var nowMs = Date.now();
    if (nowMs < (_hb._histFetchCooldownUntil || 0)) return;

    var margin = Math.max(800, (viewRight - viewLeft) * 0.75);
    var bounds = getTimelineBlockBounds(_hb.timeline);

    // Left trigger (both 'live' and 'history' modes)
    if (!_hb._histFetchLeft
        && !_hb._histMinReached
        && bounds.minIdx >= 0
        && bounds.minH > 0
        && viewLeft < _hb.timeline[0].x_start + margin) {
        var fromL = Math.max(0, bounds.minH - 250);
        var toL = bounds.minH - 1;
        _hb._histFetchLeft = true;
        var genL = _hb._fetchGen || 0;
        var modeL = _hb._viewMode;
        fetch('/api/stats/heartbeat/blocks?from=' + fromL + '&to=' + toL)
            .then(function(resp) {
                if (!resp || !resp.ok) throw new Error('HTTP ' + (resp ? resp.status : 'err'));
                return resp.json();
            })
            .then(function(data) {
                var s = getState();
                if (!s || (s._fetchGen || 0) !== genL || s._viewMode !== modeL) return;
                if (data && typeof data.tip_height === 'number' && isFinite(data.tip_height) && data.tip_height > 0) {
                    s._histTipHeight = data.tip_height;
                }
                var blks = (data && Array.isArray(data.blocks)) ? data.blocks : [];
                var added = prependHistoricalBlocks(s, blks);
                if (fromL === 0 || blks.length === 0 || added === 0) {
                    s._histMinReached = true;
                }
            })
            .catch(function() {
                var s = getState();
                if (s && (s._fetchGen || 0) === genL) {
                    s._histFetchCooldownUntil = Date.now() + 2000;
                }
            })
            .finally(function() {
                var s = getState();
                if (s && (s._fetchGen || 0) === genL) {
                    s._histFetchLeft = false;
                }
            });
    }

    // Right trigger ('history' mode ONLY)
    if (_hb._viewMode === 'history'
        && !_hb._histFetchRight
        && !_hb._histMaxReached
        && bounds.maxIdx >= 0
        && bounds.maxH >= 0
        && (_hb._histTipHeight === 0 || bounds.maxH < _hb._histTipHeight)) {
        var lastSeg = _hb.timeline[_hb.timeline.length - 1];
        var rightEdge = lastSeg.x_end !== null ? lastSeg.x_end : _hb.virtualX;
        if (viewRight > rightEdge - margin) {
            var fromR = bounds.maxH + 1;
            var toR = _hb._histTipHeight > 0
                ? Math.min(_hb._histTipHeight, fromR + 249)
                : (fromR + 249);
            _hb._histFetchRight = true;
            var genR = _hb._fetchGen || 0;
            fetch('/api/stats/heartbeat/blocks?from=' + fromR + '&to=' + toR)
                .then(function(resp) {
                    if (!resp || !resp.ok) throw new Error('HTTP ' + (resp ? resp.status : 'err'));
                    return resp.json();
                })
                .then(function(data) {
                    var s = getState();
                    if (!s || (s._fetchGen || 0) !== genR || s._viewMode !== 'history') return;
                    if (data && typeof data.tip_height === 'number' && isFinite(data.tip_height) && data.tip_height > 0) {
                        s._histTipHeight = data.tip_height;
                    }
                    var blks = (data && Array.isArray(data.blocks)) ? data.blocks : [];
                    var added = appendHistoricalBlocks(s, blks);
                    var newBounds = getTimelineBlockBounds(s.timeline);
                    if (blks.length === 0 || added === 0 || (s._histTipHeight > 0 && newBounds.maxH >= s._histTipHeight)) {
                        s._histMaxReached = true;
                    }
                })
                .catch(function() {
                    var s = getState();
                    if (s && (s._fetchGen || 0) === genR) {
                        s._histFetchCooldownUntil = Date.now() + 2000;
                    }
                })
                .finally(function() {
                    var s = getState();
                    if (s && (s._fetchGen || 0) === genR) {
                        s._histFetchRight = false;
                    }
                });
        }
    }
}

window._hbPrependHistoricalBlocks = function(a, b) {
    if (Array.isArray(a)) {
        var s = getState();
        return s ? prependHistoricalBlocks(s, a) : 0;
    }
    return a ? prependHistoricalBlocks(a, b) : 0;
};
window._hbAppendHistoricalBlocks = function(a, b) {
    if (Array.isArray(a)) {
        var s = getState();
        return s ? appendHistoricalBlocks(s, a) : 0;
    }
    return a ? appendHistoricalBlocks(a, b) : 0;
};
window._hbMaybeFetchMoreBlocks = function(hbState, viewLeft, viewRight) {
    var s = hbState || getState();
    if (!s) return;
    maybeFetchMoreBlocks(s, viewLeft, viewRight);
};

// ── On-demand Historical Block Window Mode ────────────────────────────────

window.hbV2EnterHistoryMode = function(payloadJson) {
    var _hb = getState();
    if (!_hb) return false;

    var payload;
    try {
        payload = typeof payloadJson === 'string' ? JSON.parse(payloadJson) : payloadJson;
    } catch (err) {
        console.warn('[heartbeat] hbV2EnterHistoryMode parse error:', err);
        return false;
    }
    if (!payload || !Array.isArray(payload.blocks) || payload.blocks.length === 0) return false;

    if (_hb._reveal && window._hbEndReveal) window._hbEndReveal(false);
    _hb._intro = null;
    stopMomentum();

    // Park live state when transitioning from 'live' -> 'history'
    if (_hb._viewMode !== 'history') {
        _hb._liveBackup = {
            timeline: _hb.timeline,
            virtualX: _hb.virtualX,
            viewOffset: _hb.viewOffset,
            zoom: _hb.zoom,
            autoFollow: _hb.autoFollow,
            paused: _hb.paused,
            viewOffsetY: _hb.viewOffsetY || 0,
            enteredAt: Date.now() / 1000
        };
    }

    var sorted = payload.blocks.slice().sort(function(a, b) {
        return (a.height || 0) - (b.height || 0);
    });

    var histTimeline = [];
    var hx = 0;
    var targetSeg = null;
    for (var i = 0; i < sorted.length; i++) {
        var blk = sorted[i];
        var rawInter = resolveInterBlockSeconds(blk);
        if (rawInter === null && i > 0 && blk.timestamp && sorted[i - 1].timestamp) {
            rawInter = blk.timestamp - sorted[i - 1].timestamp;
        }
        var flatW = i === 0 ? 40 : historyFlatlineWidth(rawInter);
        var flat = createFlatlineSegment(hx, hx + flatW);
        flat._compressed = true;
        flat.inter_block_seconds = rawInter;
        flat.color = rawInter === null ? UNKNOWN_INTERVAL_COLOR : waitColor(Math.max(0, rawInter));
        histTimeline.push(flat);
        hx = flat.x_end;

        var blkCopy = Object.assign({}, blk, { inter_block_seconds: rawInter });
        var bSeg = createBlockSegment(blkCopy, hx);
        if ((payload.target_height != null && bSeg.height === payload.target_height)
            || (payload.target_hash && bSeg.hash === payload.target_hash)) {
            bSeg._historyTarget = true;
            targetSeg = bSeg;
        }
        histTimeline.push(bSeg);
        hx = bSeg.x_end;
    }

    _hb._fetchGen = (_hb._fetchGen || 0) + 1;
    _hb._histFetchLeft = false;
    _hb._histFetchRight = false;
    _hb._histTipHeight = (typeof payload.tip_height === 'number' && isFinite(payload.tip_height) && payload.tip_height > 0)
        ? payload.tip_height : 0;
    _hb._histMinReached = sorted[0].height <= 0;
    _hb._histMaxReached = _hb._histTipHeight > 0 && sorted[sorted.length - 1].height >= _hb._histTipHeight;
    _hb._histFetchCooldownUntil = 0;

    _hb._viewMode = 'history';
    _hb._historyMeta = {
        target_height: payload.target_height != null ? payload.target_height : (targetSeg ? targetSeg.height : sorted[0].height),
        target_hash: payload.target_hash || (targetSeg ? targetSeg.hash : null),
        tip_height: payload.tip_height != null ? payload.tip_height : null,
        from_height: sorted[0].height,
        to_height: sorted[sorted.length - 1].height
    };
    _hb.timeline = histTimeline;
    _hb.virtualX = hx;
    _hb.autoFollow = false;
    _hb.paused = false;
    _hb.viewOffsetY = 0;
    _hb.hoveredBlock = null;
    _hb.hoveredPendingBlock = null;
    _hb.hoveredBlip = null;
    _hb.hoveredFlatline = null;
    _hb._announcement = null;

    _hb.zoom = 1.2;
    var focusSeg = targetSeg || histTimeline[Math.floor(histTimeline.length / 2)];
    if (focusSeg) {
        var mid = (focusSeg.x_start + focusSeg.x_end) / 2;
        _hb.viewOffset = mid - ((_hb.width || 800) * 0.5) / _hb.zoom;
    } else {
        _hb.viewOffset = 0;
    }

    if (typeof window.__hbHistoryModeChange === 'function') {
        window.__hbHistoryModeChange(JSON.stringify(_hb._historyMeta));
    }
    if (_hb._syncControls) _hb._syncControls();
    return true;
};

window.hbV2ExitHistoryMode = function() {
    var _hb = getState();
    if (!_hb || _hb._viewMode !== 'history') return false;
    stopMomentum();

    _hb._fetchGen = (_hb._fetchGen || 0) + 1;
    _hb._histFetchLeft = false;
    _hb._histFetchRight = false;
    _hb._histMinReached = false;
    _hb._histMaxReached = false;
    _hb._histTipHeight = 0;
    _hb._histFetchCooldownUntil = 0;

    var backup = _hb._liveBackup;
    _hb._liveBackup = null;
    _hb._historyMeta = null;
    _hb._viewMode = 'live';
    if (backup) {
        _hb.timeline = backup.timeline;
        _hb.virtualX = backup.virtualX;
        _hb.zoom = backup.zoom || 1.6;
    }
    _hb.autoFollow = true;
    _hb.paused = false;
    _hb.viewOffsetY = 0;
    _hb.hoveredBlock = null;
    _hb.hoveredPendingBlock = null;
    _hb.hoveredBlip = null;
    _hb.hoveredFlatline = null;

    // Advance virtualX across the time spent browsing history (and ensure enough
    // horizontal room for all buffered live arrivals) before spreading them by
    // real first_seen timestamp, preventing a narrow skyscraper tower on Return to LIVE.
    var queuedDuringHistory = (_hb._pendingAdds ? _hb._pendingAdds.size : 0)
        + (_hb._txBatchQueue ? _hb._txBatchQueue.length : 0)
        + (_hb._hiddenTxMap ? _hb._hiddenTxMap.size : 0);
    var nowExitSec = Date.now() / 1000;
    var enteredAt = (backup && backup.enteredAt > 0) ? backup.enteredAt : nowExitSec;
    var actualAway = Math.max(0, nowExitSec - enteredAt);
    if (queuedDuringHistory > 0) {
        var gapStartExit = _hb.virtualX;
        var winStartExit = enteredAt;
        if (_hb.lastBlockTime > 0 && _hb.lastBlockTime > winStartExit && _hb.lastBlockTime < nowExitSec) {
            winStartExit = _hb.lastBlockTime;
            actualAway = Math.max(0, nowExitSec - winStartExit);
        }
        // Size the recovery span proportionally to the queued arrivals so we
        // neither build a narrow skyscraper nor carve a huge empty desert.
        var volSpanPx = Math.max(30, Math.ceil(queuedDuringHistory / 10) * 5);
        var timeCapPx = Math.min(actualAway, 180) * FLATLINE_PX_PER_SEC;
        var exitSpanPx = Math.min(Math.max(volSpanPx, Math.min(timeCapPx, volSpanPx * 2)), Math.max(volSpanPx, 1500));
        if (exitSpanPx > 0) {
            _hb.virtualX += exitSpanPx;
        }
        drainHiddenTxsAcrossGap(gapStartExit, Math.max(gapStartExit, _hb.virtualX - 5), winStartExit, nowExitSec);
    } else if (actualAway > 1) {
        // No queued arrivals while browsing history: keep the gap modest so
        // returning to LIVE does not drop the viewer into a 1,500px empty void.
        _hb.virtualX += Math.min(actualAway, 10) * FLATLINE_PX_PER_SEC;
    }

    _hb.viewOffset = _hb.virtualX - ((_hb.width || 800) * HEAD_POSITION_FRAC) / _hb.zoom;

    if (_hb._pinnedTxid) {
        hbV2SetPinnedTx(_hb._pinnedTxid);
    }
    if (typeof window.__hbHistoryModeChange === 'function') {
        window.__hbHistoryModeChange('');
    }
    if (typeof window.__hbResyncCanvasFromMirror === 'function') {
        window.__hbResyncCanvasFromMirror();
    }
    if (_hb._syncControls) _hb._syncControls();
    return true;
};

function _hbVisibilityChange() {
    var _hb = getState();
    if (!_hb) return;
    if (document.hidden) {
        _hb._hiddenSince = Date.now() / 1000;
        return;
    }
    var gapSecs = _hb._hiddenSince > 0 ? (Date.now() / 1000 - _hb._hiddenSince) : 0;
    _hb._hiddenSince = 0;
    recoverTimeline(gapSecs);
}
document.addEventListener('visibilitychange', _hbVisibilityChange);

// ── Heartbeat v2 bridge entry points (called by Rust/WASM client) ────────

window.hbV2InstallSnapshot = function(payloadJson, isResync) {
    return hbV2InstallSnapshot(payloadJson, isResync);
};
window.hbV2ApplyEffect = function(effectJson) {
    return hbV2ApplyEffect(effectJson);
};
window.hbV2FocusTx = function(txid) {
    return hbV2FocusTx(txid);
};
window.hbV2SetPinnedTx = function(txidOrNull) {
    return hbV2SetPinnedTx(txidOrNull);
};
window.hbV2SetSpotlightBand = function(band) {
    var _hb = getState();
    if (!_hb) return;
    _hb._spotlightBand = (typeof band === 'number' && band >= 0 && band <= 6) ? band : -1;
};
window.hbV2FocusBlockHeight = function(height) {
    return hbV2FocusBlockHeight(height);
};
window.hbV2FocusPendingSlot = function(hash) {
    return hbV2FocusPendingSlot(hash);
};
window.hbV2IsPendingSlotOffscreen = function(hash) {
    return hbV2IsPendingSlotOffscreen(hash);
};
window.hbV2SetConnectionState = function(state) {
    return hbV2SetConnectionState(state);
};

// ── Re-export vitals functions on window ───────────────────

window.getHeartbeatVitals = function() { return getHeartbeatVitals(); };
window.renderRhythmStrip = function(id, json) { renderRhythmStrip(id, json); };
window.getHeartbeatRecentBlocks = function() { return getHeartbeatRecentBlocks(); };
// Freshest block on the live timeline (height + on-chain timestamp).
window.getHeartbeatLatestBlock = function() {
    var _hb = getState();
    if (!_hb) return '{"height":0,"timestamp":0}';
    var tl = (_hb._viewMode === 'history' && _hb._liveBackup) ? _hb._liveBackup.timeline : _hb.timeline;
    if (!tl) return '{"height":0,"timestamp":0}';
    for (var i = tl.length - 1; i >= 0; i--) {
        var seg = tl[i];
        if (seg.type === 'block') {
            return JSON.stringify({ height: seg.height || 0, timestamp: seg.timestamp || 0 });
        }
    }
    return '{"height":0,"timestamp":0}';
};
// Connection state owned by the Rust/WASM v2 client via hbV2SetConnectionState.
// Promotes 'live' to 'stale' after HEARTBEAT_STALL_SECS of SSE silence.
window.getHeartbeatConnectionState = function() {
    var _hb = getState();
    if (!_hb) return 'live';
    var conn = _hb._v2ConnState || 'live';
    if (conn === 'live' && _hb._lastSseEventTs > 0 &&
        (Date.now() / 1000 - _hb._lastSseEventTs) > HEARTBEAT_STALL_SECS) {
        return 'stale';
    }
    return conn;
};
window.heartbeatCenter = function() { heartbeatCenter(); };
window.heartbeatSearchTx = function(txid) { return heartbeatSearchTx(txid); };
window.heartbeatPulse = function() { heartbeatPulse(); };
window.heartbeatFlash = function() { heartbeatFlash(); };
window.getOrganismStatus = function() { return getOrganismStatus(); };
window.heartbeatDownloadCapture = function(json) { heartbeatDownloadCapture(json); };
