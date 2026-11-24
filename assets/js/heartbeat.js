// Block Heartbeat v3: ES Module entry point
// Imports all sub-modules and wires up the window.* public API
// that the Leptos component calls via wasm_bindgen.

import { getState, setState, COLORS, BG_COLOR, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, RETARGET_PERIOD } from './heartbeat-state.js';
import { drawGrid, createBlockSegment, createFlatlineSegment, computeColor, historyFlatlineWidth } from './heartbeat-timeline.js';
import { setupInputHandlers, handleControlClick, initSoundButton } from './heartbeat-interaction.js';
import { stopMomentum } from './heartbeat-interaction.js';
import {
    hbV2InstallSnapshot, hbV2ApplyEffect, hbV2FocusTx, hbV2SetPinnedTx,
    hbV2FocusBlockHeight, hbV2SetConnectionState,
    fastForwardLiveFlatline, drainHiddenTxsAcrossGap, ensureSegMaps,
    registerBlipInSeg, txidSkylineOffset, buildBlip, updateBlipMetadata,
    rebuildRevealIndex, maybeClearRemovedTxids
} from './heartbeat-sse.js';
import { playBlockCue, tuneSound, soundSettings } from './heartbeat-audio.js';
import { drawFrame } from './heartbeat-render.js';
import {
    getHeartbeatVitals, renderRhythmStrip, getHeartbeatRecentBlocks,
    heartbeatCenter, heartbeatSearchTx,
    heartbeatPulse, heartbeatFlash,
    getOrganismStatus,
    heartbeatDownloadCapture
} from './heartbeat-vitals.js';

// Seconds of SSE silence before a 'live' connection is reported as 'stale'.
var HEARTBEAT_STALL_SECS = 60;

// ── Public API ─────────────────────────────────────────────

window.initHeartbeat = function(canvasId) {
    if (getState()) window.destroyHeartbeat();
    if (window._seenNotableTxids) window._seenNotableTxids.clear();

    var canvas = document.getElementById(canvasId);
    if (!canvas) return;

    var container = canvas.parentElement;
    var dpr = window.devicePixelRatio || 1;
    var rect = canvas.getBoundingClientRect();

    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;

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
        // Cinematic block reveal (form→harvest→unfurl→hold→return). Off = instant
        // flow (spike appears, harvest, mempool re-lays, no camera hijack; follow
        // state respected). Toggle via window._hbToggleReveal(); persisted.
        _revealEnabled: (function() { try { return localStorage.getItem('hb_reveal') !== 'off'; } catch (e) { return true; } })(),
        zoom: 1.9,
        // minZoom must go low enough to fit the whole mempool ribbon + spikes on
        // one screen even at large mempools (volume-sized flatline grows with tx
        // count). 0.03 fits ~60-80k txs in a typical viewport.
        minZoom: 0.03,
        maxZoom: 50.0,

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
        _pendingBlocks: [],
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

    // Show fullscreen button only if the API is supported (not on iOS)
    var fsBtn = document.getElementById('heartbeat-fullscreen-btn');
    if (fsBtn && (document.fullscreenEnabled || document.webkitFullscreenEnabled)) {
        fsBtn.classList.remove('hidden');
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
            var r = canvas.getBoundingClientRect();
            canvas.width = r.width * dpr;
            canvas.height = r.height * dpr;
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
        // Pause icon (inner span, so the tooltip sibling survives)
        var pb = document.getElementById('heartbeat-btn-pause-icon');
        if (pb) pb.textContent = s.paused ? '\u25B6' : '\u23F8';
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

window.pushHeartbeatBlocks = function(json, replay) {
    var _hb = getState();
    if (!_hb) return;
    var isReplay = !!replay;
    // Harvest only for genuine live blocks (set by processLiveBlock), not replay
    // /history/tab-return. Consume the one-shot flag.
    var harvestLive = !!_hb._harvestLive;
    _hb._harvestLive = false;
    // When set (by processLiveBlock for a reveal-eligible live block), defer the
    // leftover re-lay to the block-reveal sequencer and keep the leftovers in the
    // closing segment so they stay visible through the form beat (they fade during
    // the harvest beat, then re-lay during the relay beat).
    var revealPending = !!_hb._revealPending;
    _hb._revealPending = false;
    try {
        var blocks = JSON.parse(json);
        if (!Array.isArray(blocks)) return;
        for (var i = 0; i < blocks.length; i++) {
            var b = blocks[i];

            // Deduplicate: skip if this block height is already in the timeline.
            // Exception: an estimated spike (getblockstats was down in the fast
            // path, so size/weight/fees were broadcast as 0) gets re-broadcast
            // with authoritative data. Trigger the replace when the incoming
            // event materializes structural data the existing one lacks \u2014 keyed
            // on weight OR fees, matching the backend `estimated = size==0||weight==0`
            // definition. (Keying on total_fees alone missed the case where the
            // fee RPC stayed down but getblock gave real size/weight.)
            if (!isReplay && (b.height || b.hash)) {
                var existingSeg = null;
                for (var di = _hb.timeline.length - 1; di >= Math.max(0, _hb.timeline.length - 30); di--) {
                    var cand = _hb.timeline[di];
                    if (cand.type === 'block' && ((b.hash && cand.hash === b.hash) || (b.height && cand.height === b.height))) {
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
                        existingSeg.color = replacement.color;
                        existingSeg.tx_count = replacement.tx_count;
                        existingSeg.total_fees = replacement.total_fees;
                        existingSeg.size = replacement.size;
                        existingSeg.weight = replacement.weight;
                        existingSeg.inter_block_seconds = replacement.inter_block_seconds;
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
                    continue;
                }
            }

            var interBlock = b.inter_block_seconds || 600;
            // For live blocks, compute the real interval from the last block's timestamp.
            if (!isReplay && _hb.lastBlockTime > 0 && b.timestamp > _hb.lastBlockTime) {
                interBlock = b.timestamp - _hb.lastBlockTime;
            }

            // Close the current live flatline
            var lastSeg = _hb.timeline[_hb.timeline.length - 1];
            if (lastSeg && lastSeg.type === 'flatline' && lastSeg.x_end === null) {
                if (isReplay) {
                    lastSeg.x_end = lastSeg.x_start + historyFlatlineWidth(interBlock);
                    _hb.virtualX = lastSeg.x_end;
                    var feeRate = b.total_fees ? b.total_fees / 100000 : 0;
                    lastSeg.color = computeColor(interBlock, feeRate, 0);
                } else {
                    // Scale min flatline with zoom so blocks always have a visible gap.
                    // At zoom 1.9x → 30px virtual = 57px screen.
                    // At zoom 0.6x → 50px virtual = 30px screen.
                    var minLiveFlatline = Math.max(30, Math.round(20 / Math.max(_hb.zoom, 0.1)));
                    if (_hb.virtualX - lastSeg.x_start < minLiveFlatline) {
                        _hb.virtualX = lastSeg.x_start + minLiveFlatline;
                    }
                    lastSeg.x_end = _hb.virtualX;
                }
                // Stamp the flatline with its color at close time
                if (!isReplay) {
                    lastSeg.color = _hb._preFlashColor || _hb.currentColor || COLORS.healthy;
                }
                // Exact-inclusion harvest ONLY: never approximate confirmation by fee rate.
                // If block_inclusion has already arrived for this block's hash, shatter those
                // exact txids and carry the rest forward. If block_inclusion hasn't arrived yet,
                // carry all active bricks forward cleanly so block_inclusion shatters the exact
                // confirmed set in place moments later (or partitions an armed reveal).
                var carriedLeftovers = [];
                if (lastSeg.blips) {
                    var absorbX = _hb.virtualX;
                    var absorbNow = Date.now() / 1000;
                    var zoom = _hb.zoom;

                    var activeCount = 0;
                    for (var ci = 0; ci < lastSeg.blips.length; ci++) {
                        if (!lastSeg.blips[ci]._removed && lastSeg.blips[ci].fadeStart === 0) activeCount++;
                    }
                    var maxPartsPerBlip;
                    if (zoom < 0.5 || activeCount > 1000) maxPartsPerBlip = 1;
                    else if (zoom < 1.0 || activeCount > 500) maxPartsPerBlip = 2;
                    else maxPartsPerBlip = 3 + Math.floor(Math.random() * 3);

                    var exactConf = (!isReplay && b.hash && _hb._exactConfirmedByHash)
                        ? _hb._exactConfirmedByHash.get(b.hash)
                        : null;

                    var kept = [];
                    var revealConfirmed = revealPending ? [] : null;
                    for (var bi = 0; bi < lastSeg.blips.length; bi++) {
                        var blp = lastSeg.blips[bi];
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
                    lastSeg.blips = kept;
                    if (lastSeg._blipsByTxid) lastSeg._blipsByTxid.clear();
                    if (lastSeg._colBlips) lastSeg._colBlips.clear();
                    lastSeg._colHeights = {};
                }
                // Never drop bricks that were still queued in an in-flight chunked
                // carry or snapshot placement for `lastSeg` (e.g. back-to-back blocks
                // or hidden-tab catch-up): harvest all remaining unplaced items too.
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
                    if (_hb._pendingCarry && _hb._pendingCarry.seg === lastSeg) {
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
                    if (_hb._pendingSnapshot && _hb._pendingSnapshot.seg === lastSeg) {
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
            }

            // Create block segment with corrected inter-block time
            b.inter_block_seconds = interBlock;
            var blockSeg = createBlockSegment(b, _hb.virtualX);
            // Spike strike ("BAM"). Live INSTANT blocks strike immediately. For a
            // cinematic reveal we instead mark the spike _strikePending (drawn FLAT)
            // and let the reveal fire the strike: right away if the view is already
            // at the head, or on ARRIVAL if the camera has to travel there, so the
            // pop lands when you're looking at it, not mid-scroll. Replay/history
            // appear settled (neither flag).
            if (!isReplay) {
                if (revealPending) blockSeg._strikePending = true;
                else blockSeg._strikeStart = Date.now() / 1000;
            }
            _hb.timeline.push(blockSeg);
            _hb.virtualX = blockSeg.x_end;

            // Update last block time
            if (b.timestamp) {
                _hb.lastBlockTime = b.timestamp;
            }

            // Create a new live flatline after this block
            _hb.timeline.push(createFlatlineSegment(_hb.virtualX, null));

            // Carry unconfirmed leftovers onto the new (current-mempool) flatline.
            // For a reveal-eligible live block, DEFER: stash them for the reveal
            // sequencer, which re-lays them progressively during the relay beat (they stay
            // visible in the old segment meanwhile). Otherwise re-lay now (instant).
            if (harvestLive && !isReplay && carriedLeftovers.length > 0) {
                if (_hb._deferCarryList) {
                    // Batch catch-up of multiple hidden blocks: stash leftovers in
                    // memory and run placeCarriedLeftovers ONCE after the final block.
                    _hb._deferCarryList = carriedLeftovers;
                } else if (revealPending) {
                    _hb._pendingReveal = {
                        leftovers: carriedLeftovers,
                        confirmed: revealConfirmed,   // shatter deferred to the harvest beat
                        absorbX: absorbX,             // spike location the particles fly to
                        maxParts: maxPartsPerBlip,
                        oldSeg: lastSeg,
                        newSeg: _hb.timeline[_hb.timeline.length - 1],
                        spikeSeg: blockSeg,
                        _revealByTxid: null
                    };
                    rebuildRevealIndex(_hb._pendingReveal);
                } else {
                    placeCarriedLeftovers(carriedLeftovers, _hb._instantCarry ? 0 : undefined);
                }
            }

            // Maintain recentBlocks list (up to 2016 for period history)
            _hb.recentBlocks.push({
                hash: b.hash || null,
                timestamp: b.timestamp,
                height: b.height,
                tx_count: b.tx_count,
                total_fees: b.total_fees,
                size: b.size,
                weight: b.weight,
                inter_block_seconds: b.inter_block_seconds
            });

            // Show block arrival announcement. When a cinematic reveal is running
            // it OWNS the banners (block-found \u2192 remaining-count, timed to the
            // beats), so skip here. The instant path (>9x / off / tab-return) shows
            // it as before, trimmed ~1s.
            if (!isReplay && !revealPending && !_hb._suppressAnnounce && b.height && b.total_fees > 0) {
                var feeBtc2 = b.total_fees / 100000000;
                var heightStr2 = b.height.toLocaleString();
                var nowAnn2 = Date.now() / 1000;
                _hb._announcement = {
                    title: 'Block #' + heightStr2 + ' found',
                    subtitle: feeBtc2.toFixed(4) + ' BTC fees \u00b7 ' + (b.tx_count || 0).toLocaleString() + ' txs',
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
                if (_hb.autoFollow && !_hb._pinnedTxid) {
                    _hb.zoom = 1.9;
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
    // Don't prune while a block reveal is running: it holds refs to the two newest
    // segments (oldSeg/newSeg) and slicing the array out from under it would drop
    // the fading old segment mid-beat. (They're at the head, so this only matters
    // defensively; the reveal is short.)
    if (!_hb || _hb.timeline.length < 5000 || _hb._reveal) return;
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
    }
}

// Re-drop carried-forward (unconfirmed) leftover bricks onto the current live
// flatline right after a block, widening it to fit them. Reuses each brick's own
// object (color/size/txid/fee preserved): same mempool txs, moved to the new
// mempool segment with a staggered drop-in. Density mirrors live/history: ~10
// bricks per 5px column, skip a column once it hits the 35%-height cap.
function placeCarriedLeftovers(leftovers, staggerSecs) {
    var _hb = getState();
    if (!_hb) return;
    var seg = _hb.timeline[_hb.timeline.length - 1];
    if (!seg || seg.type !== 'flatline' || seg.x_end !== null) return;

    var n = leftovers.length;
    // Drop-in sweep duration: the reveal sequencer passes the (rate-based) relay
    // duration so the bricks appear in step with the camera; the non-reveal path
    // uses 2.5s; 0 finalizes instantly (abort / new block).
    var stagger = staggerSecs !== undefined ? staggerSecs : 2.5;
    var TARGET_COL_H = 65;
    var maxStack = (_hb.height || 400) * 0.35;
    var softMaxStack = Math.min(maxStack, 160);
    // Preserve deterministic relative left-to-right order: unknown-time first
    // sorted by txid, then ascending (firstSeen, txid).
    var ordered = leftovers.slice().sort(function(a, b) {
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
    // Set the geometry UP FRONT (before placing), matching the exact carried
    // backlog footprint so no empty void appears between carried bricks and live head.
    _hb.virtualX = seg.x_start + width;
    ensureSegMaps(seg);
    var now = Date.now() / 1000;

    if (_hb._carryRaf) { cancelAnimationFrame(_hb._carryRaf); _hb._carryRaf = null; }
    _hb._pendingCarry = null;
    _hb._pendingCarryByTxid = null;
    var CARRY_CHUNK = 4000;
    var startGridX = Math.ceil(seg.x_start / 5) * 5;
    var usableSpan = Math.max(5, (_hb.virtualX - seg.x_start) - 15);
    var usableCols = Math.max(1, Math.floor(usableSpan / 5));
    seg._carryEndX = startGridX + usableCols * 5;
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
            if (oCol < knownStartCol) oCol = knownStartCol + (knownStartCol - oCol);
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
    _hb._pendingCarry = { seg: seg, items: plannedCarry, nextIdx: 0 };
    _hb._pendingCarryByTxid = pendingCarryByTxid;
    var placeChunk = function() {
        var s = getState();
        if (!s || s.timeline[s.timeline.length - 1] !== seg || seg.x_end !== null) {
            if (s) {
                s._carryRaf = null;
                if (s._pendingCarry && s._pendingCarry.seg === seg) {
                    s._pendingCarry = null;
                    s._pendingCarryByTxid = null;
                }
            }
            return;
        }
        var end = Math.min(i + CARRY_CHUNK, plannedCarry.length);
        var span = Math.max(1, usableSpan);
        for (; i < end; i++) {
            var item = plannedCarry[i];
            var src = item.src;
            if (src && src.txid && s._pendingCarryByTxid) {
                s._pendingCarryByTxid.delete(src.txid);
            }
            if (!src || src._removed) continue;
            if (s._removedTxids && src.txid && s._removedTxids.has(src.txid)) continue;
            if (src.txid && seg._blipsByTxid.has(src.txid)) continue;
            if (s._deferredDetails && src.txid && s._deferredDetails.has(src.txid)) {
                updateBlipMetadata(src, s._deferredDetails.get(src.txid));
                s._deferredDetails.delete(src.txid);
            }

            var brickH = src.brickH || 6;
            var gx = startGridX + item.col * 5;
            var stackY = seg._colHeights[gx] || 0;

            if (stackY > 0 && stackY + brickH > softMaxStack) {
                var minZoneX = startGridX + (src.firstSeen ? knownStartCol : 0) * 5;
                var maxZoneX = startGridX + (src.firstSeen ? usableCols - 1 : Math.max(0, unknownCols - 1)) * 5;
                var bestX = gx;
                var bestY = stackY;
                for (var step = 1; step <= 10; step++) {
                    var c1 = gx + step * 5;
                    if (c1 <= maxZoneX) {
                        var h1 = seg._colHeights[c1] || 0;
                        if (h1 < bestY) { bestX = c1; bestY = h1; if (bestY + brickH <= softMaxStack) break; }
                    }
                    var c2 = gx - step * 5;
                    if (c2 >= minZoneX) {
                        var h2 = seg._colHeights[c2] || 0;
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
            var xFrac = Math.min(1, Math.max(0, (gx - seg.x_start) / span));
            var rainJitter = ((txidSkylineOffset(src.txid) + 1) * 0.5) * 0.55;
            var stackDelay = stagger > 0 ? (stackY * 0.0025 + rainJitter) : 0;
            src.timestamp = now + xFrac * stagger + stackDelay;

            registerBlipInSeg(seg, src);
            if (s._pinnedTxid && src.txid === s._pinnedTxid) {
                s._pinnedBlip = src;
            }
            placed++;
        }
        if (s._pendingCarry && s._pendingCarry.seg === seg) {
            s._pendingCarry.nextIdx = i;
        }
        if (i < plannedCarry.length) {
            s._carryRaf = requestAnimationFrame(placeChunk);
        } else {
            s._carryRaf = null;
            if (s._pendingCarry && s._pendingCarry.seg === seg) {
                s._pendingCarry = null;
                s._pendingCarryByTxid = null;
            }
            maybeClearRemovedTxids(s);
            if (s._pinnedTxid) {
                s._pinnedBlip = seg._blipsByTxid.get(s._pinnedTxid) || null;
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
//   window._hbSoundTune({ MASTER_GAIN: 0.8, SPIKE_PEAK_HZ: 1600 })
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
        weight: qb.weight || 0
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

    var q = queued
        .filter(function(b) { return b && b.height && b.height > maxH; })
        .sort(function(a, b) { return a.height - b.height; });
    if (q.length === 0) return 0;

    var hasExtra = Array.isArray(extraPreBlockTxs) && extraPreBlockTxs.length > 0;
    var carried = null;
    var newestTs = 0;
    _hb._instantCarry = true;
    try {
        for (var i = 0; i < q.length; i++) {
            var qb = q[i];
            if (i > 0) {
                var gapSecs = Math.max(0, (qb.timestamp || 0) - (q[i - 1].timestamp || 0));
                _hb.virtualX += historyFlatlineWidth(gapSecs);
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
            newestTs = qb.timestamp || newestTs;
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
            if (qb.timestamp && qb.timestamp > newestQueuedTs) newestQueuedTs = qb.timestamp;
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
        var clampedPostElapsed = Math.min(actualPostElapsed, 300);
        var postVolPx = Math.ceil(countBufferedHidden() / 8) * 5;
        var postSpanPx = Math.max(clampedPostElapsed * FLATLINE_PX_PER_SEC, postVolPx);
        if (postSpanPx > 0) {
            _hb.virtualX += postSpanPx;
        }
        var winStart = newestBlockTs > 0 ? newestBlockTs : (now - actualPostElapsed);
        drainHiddenTxsAcrossGap(gapStart, Math.max(gapStart, _hb.virtualX - 5), winStart, now);
    } else {
        // No blocks while away: advance the live head across the elapsed gap
        // (sized to hold all hidden arrivals at normal skyline height without
        // clamping timestamps to 300s, which piled older hidden txs into a
        // tower wall at the 300s boundary).
        var gapStartNoBlk = _hb.virtualX;
        var actualGap = gapSecs > 0 ? gapSecs : 0;
        var clampedGap = Math.min(actualGap, 300);
        var volPx = Math.ceil(countBufferedHidden() / 8) * 5;
        var spanPx = Math.max(clampedGap * FLATLINE_PX_PER_SEC, volPx);
        if (spanPx > 0) {
            _hb.virtualX += spanPx;
        }
        drainHiddenTxsAcrossGap(gapStartNoBlk, Math.max(gapStartNoBlk, _hb.virtualX - 5), now - actualGap, now);
    }

    if (_hb.autoFollow && !_hb._pinnedTxid && _hb.width > 0 && _hb.zoom > 0) {
        _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
    }

    _hb.lastFrameTime = now;
}
window._hbRecoverTimeline = function(gapSecs) { recoverTimeline(gapSecs); };

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
window.hbV2FocusBlockHeight = function(height) {
    return hbV2FocusBlockHeight(height);
};
window.hbV2SetConnectionState = function(state) {
    return hbV2SetConnectionState(state);
};

// ── Re-export vitals functions on window ───────────────────

window.getHeartbeatVitals = function() { return getHeartbeatVitals(); };
window.renderRhythmStrip = function(id, json) { renderRhythmStrip(id, json); };
window.getHeartbeatRecentBlocks = function() { return getHeartbeatRecentBlocks(); };
// Freshest block on the timeline (height + on-chain timestamp).
window.getHeartbeatLatestBlock = function() {
    var _hb = getState();
    if (!_hb || !_hb.timeline) return '{"height":0,"timestamp":0}';
    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        var seg = _hb.timeline[i];
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
