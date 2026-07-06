// Block Heartbeat v3 — ES Module entry point
// Imports all sub-modules and wires up the window.* public API
// that the Leptos component calls via wasm_bindgen.

import { getState, setState, COLORS, BG_COLOR, FLATLINE_PX_PER_SEC, HEAD_POSITION_FRAC, RETARGET_PERIOD } from './heartbeat-state.js';
import { drawGrid, createBlockSegment, createFlatlineSegment, computeColor, historyFlatlineWidth, feeRateColor } from './heartbeat-timeline.js';
import { setupInputHandlers, handleControlClick } from './heartbeat-interaction.js';
import { stopMomentum } from './heartbeat-interaction.js';
import { connectOwnFeed, placeHistoryTxs, fastForwardLiveFlatline } from './heartbeat-sse.js';
import { drawFrame } from './heartbeat-render.js';
import {
    getHeartbeatVitals, renderRhythmStrip, getHeartbeatRecentBlocks,
    heartbeatCenter, heartbeatSearchTx,
    heartbeatPulse, heartbeatFlash,
    getOrganismStatus,
    heartbeatSoundToggle, heartbeatSoundIsEnabled, heartbeatPlaySound,
    heartbeatDownloadCapture
} from './heartbeat-vitals.js';

// Seconds of SSE silence (no tx/block) before the feed is considered stale.
// Txs normally arrive within seconds even in a quiet mempool, so this only
// trips when the node/stream has genuinely gone quiet (e.g. an RPC stall).
var HEARTBEAT_STALL_SECS = 60;

// ── Public API ─────────────────────────────────────────────

window.initHeartbeat = function(canvasId) {
    if (getState()) window.destroyHeartbeat();

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
        // Level-of-detail: when zoomed way out (<0.3x) with a very full mempool
        // (>3000 bricks), draw per-column density bars instead of every brick, to
        // avoid redrawing ~45k+ rects/frame. Toggle at runtime with
        // window._hbToggleLod() (persisted in localStorage) — off = individual
        // bricks always (nicer, but can jank at a very full mempool zoomed out).
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
        _pinnedBlip: null,
        hoveredFlatline: null,
        hoverCanvasX: 0,
        _flashTimer: null,

        // Mempool feed
        ws: null,
        wsConnected: false,

        // SSE disconnection state (for overlay)
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

    // Connect to own node SSE feed (falls back to mempool.space WS)
    connectOwnFeed();
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
            if (!isReplay && b.height) {
                var existingSeg = null;
                for (var di = _hb.timeline.length - 1; di >= Math.max(0, _hb.timeline.length - 30); di--) {
                    if (_hb.timeline[di].type === 'block' && _hb.timeline[di].height === b.height) {
                        existingSeg = _hb.timeline[di];
                        break;
                    }
                }
                if (existingSeg) {
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
                        existingSeg.weight = replacement.weight;
                        existingSeg.inter_block_seconds = replacement.inter_block_seconds;
                        // Announce with whatever real data landed. Fees may still be
                        // unknown (0) if getblockstats stayed down \u2014 omit them rather
                        // than show "0.0000 BTC".
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
                // A block confirms some mempool txs and leaves the rest waiting.
                // HARVEST (live blocks): split the active bricks by fee rate — the
                // top `tx_count` (≈ what the block took) shatter into the spike;
                // the lower-fee remainder is carried forward onto the new flatline
                // (still unconfirmed = still in the mempool). Replay/history blocks
                // shatter everything (no live mempool to carry).
                var carriedLeftovers = [];
                if (lastSeg.blips) {
                    var absorbX = _hb.virtualX;
                    var absorbNow = Date.now() / 1000;
                    var zoom = _hb.zoom;

                    // Count active (non-fading) bricks. maxPartsPerBlip scales the
                    // shatter down when there's a lot to keep the frame cheap.
                    var activeCount = 0;
                    for (var ci = 0; ci < lastSeg.blips.length; ci++) {
                        if (lastSeg.blips[ci].fadeStart === 0) activeCount++;
                    }
                    var maxPartsPerBlip;
                    if (zoom < 0.5 || activeCount > 1000) maxPartsPerBlip = 1;
                    else if (zoom < 1.0 || activeCount > 500) maxPartsPerBlip = 2;
                    else maxPartsPerBlip = 3 + Math.floor(Math.random() * 3);

                    // Which active bricks did the block confirm? The top `tx_count`
                    // by fee rate. Rather than sort the (up to ~45k) brick OBJECTS
                    // and build a Set — a multi-ms stall right as the reveal camera
                    // starts moving — sort just their fee rates (numbers, cheap) to
                    // find the cutoff, then partition by comparing each brick's rate
                    // to it. feeThreshold = -Infinity confirms everything (block took
                    // >= what we show), matching prior behavior.
                    var feeThreshold = -Infinity;
                    var tieBudget = Infinity; // confirmations allowed AT the threshold
                    if (harvestLive && !isReplay && activeCount > 0) {
                        var harvestCount = Math.min(b.tx_count || activeCount, activeCount);
                        if (harvestCount < activeCount) {
                            var rates = new Float64Array(activeCount);
                            var ri = 0;
                            for (var fi = 0; fi < lastSeg.blips.length; fi++) {
                                if (lastSeg.blips[fi].fadeStart === 0) {
                                    rates[ri++] = lastSeg.blips[fi].feeRate || 0;
                                }
                            }
                            rates.sort(); // TypedArray sorts numerically ascending
                            // The harvestCount-th highest rate is the cutoff. Bricks
                            // strictly above it all confirm; bricks exactly at it fill
                            // the remaining budget (feeRate is stored rounded to 1dp,
                            // so equality is exact — no float epsilon). Capping at
                            // harvestCount stops a mass of equal low-fee bricks (a
                            // quiet 1 sat/vB mempool) from all confirming at once.
                            feeThreshold = rates[activeCount - harvestCount];
                            var above = 0;
                            for (var k = activeCount - 1; k >= 0 && rates[k] > feeThreshold; k--) above++;
                            tieBudget = harvestCount - above; // >= 0 by construction
                        }
                    }

                    // Single partition pass: confirmed bricks shatter into the spike;
                    // the rest carry forward. Rebuild lastSeg.blips in place = kept
                    // (already-fading + confirmed), dropping the carried — no separate
                    // filter pass or Sets.
                    var kept = [];
                    var revealConfirmed = revealPending ? [] : null;
                    for (var bi = 0; bi < lastSeg.blips.length; bi++) {
                        var blp = lastSeg.blips[bi];
                        if (blp.fadeStart !== 0) { kept.push(blp); continue; } // already fading
                        var confirm;
                        if (blp.feeRate > feeThreshold) {
                            confirm = true;
                        } else if (blp.feeRate === feeThreshold && tieBudget > 0) {
                            confirm = true;
                            tieBudget--;
                        } else {
                            confirm = false;
                        }
                        if (!confirm) {
                            carriedLeftovers.push(blp); // leftover: carry forward
                            if (revealPending) kept.push(blp); // keep visible during the reveal (fades, then re-laid)
                            continue;
                        }
                        if (revealPending) {
                            // Defer the shatter to the reveal's harvest beat: keep the
                            // brick visible (normal) in the old segment until then; the
                            // sequencer sets fadeStart + particles when that beat opens.
                            revealConfirmed.push(blp);
                            kept.push(blp);
                            continue;
                        }
                        blp.fadeStart = absorbNow;
                        blp._confirmedHeight = b.height; // pinned/hovered tx shows "confirmed in block #N"
                        blp.absorbTargetX = absorbX;
                        blp.absorbOriginX = blp.x;
                        blp.particles = [];
                        for (var pi = 0; pi < maxPartsPerBlip; pi++) {
                            blp.particles.push({
                                offsetX: (Math.random() - 0.5) * 8,
                                offsetY: Math.random() * blp.height * 0.8,
                                speed: 0.7 + Math.random() * 0.6,
                                arcHeight: 20 + Math.random() * 60,
                                delay: Math.random() * 0.3,
                                size: 1.5 + Math.random() * 2.5
                            });
                        }
                        kept.push(blp);
                    }
                    lastSeg.blips = kept;
                }
            }

            // Create block segment with corrected inter-block time
            b.inter_block_seconds = interBlock;
            var blockSeg = createBlockSegment(b, _hb.virtualX);
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
                if (revealPending) {
                    _hb._pendingReveal = {
                        leftovers: carriedLeftovers,
                        confirmed: revealConfirmed,   // shatter deferred to the harvest beat
                        absorbX: absorbX,             // spike location the particles fly to
                        maxParts: maxPartsPerBlip,
                        oldSeg: lastSeg,
                        newSeg: _hb.timeline[_hb.timeline.length - 1],
                        spikeSeg: blockSeg
                    };
                } else {
                    placeCarriedLeftovers(carriedLeftovers);
                }
            }

            // Maintain recentBlocks list (up to 2016 for period history)
            _hb.recentBlocks.push({
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
                _hb.zoom = 1.9;
                _hb.viewOffsetY = 0;
                _hb.autoFollow = true;
                _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC);
            }

            // Place any SSE history txs that arrived before the replay
            if (_hb._pendingHistory) {
                var ph = _hb._pendingHistory;
                _hb._pendingHistory = null;
                console.log('[heartbeat] placing deferred history after replay');
                placeHistoryTxs(ph.txs, ph.last_block_ts);
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
// object (color/size/txid/fee preserved) — same mempool txs, moved to the new
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
    var maxStack = (_hb.height || 400) * 0.35;
    var width = Math.max(30, Math.ceil(n / 10) * 5); // ~10 bricks/column
    // Set the geometry UP FRONT (before placing), so the flatline width, virtualX,
    // and the reveal-camera gate that reads them are all correct immediately — the
    // bricks then stream in over the frames below.
    _hb.virtualX = seg.x_start + width;
    if (!seg._colHeights) seg._colHeights = {};
    var usable = Math.max(10, width - 20);
    var now = Date.now() / 1000;

    // Place in CHUNKS across RAF frames instead of one synchronous pass. A
    // full-mempool carry (~40k bricks) in a single loop stalls the frame the block
    // lands on — which is exactly when the reveal camera starts moving, so the
    // stall reads as jank at the start of the animation. Chunking also spreads the
    // drop-in over the reveal so the camera visibly "follows" the placement. Each
    // brick's drop timing stays keyed to its x (now + xFrac), so the left→right
    // sweep holds regardless of which chunk placed it. Cancel any in-flight job.
    if (_hb._carryRaf) { cancelAnimationFrame(_hb._carryRaf); _hb._carryRaf = null; }
    var CARRY_CHUNK = 2000;
    var i = 0;
    var placed = 0;
    var placeChunk = function() {
        var s = getState();
        // Abort if torn down, or the flatline closed/changed (a new block arrived
        // mid-carry) — the remaining leftovers would land on a stale segment.
        if (!s || s.timeline[s.timeline.length - 1] !== seg || seg.x_end !== null) {
            _hb._carryRaf = null;
            return;
        }
        var end = Math.min(i + CARRY_CHUNK, n);
        for (; i < end; i++) {
            var src = leftovers[i];
            var txVX = seg.x_start + Math.random() * usable;
            var gridX = Math.round(txVX / 5) * 5;
            var stackY = seg._colHeights[gridX] || 0;
            var isNotable = !!src.notableType;
            if (stackY > maxStack && !isNotable) continue; // column full
            var brickH = src.brickH || 6;
            seg._colHeights[gridX] = stackY + brickH;
            // Reposition + reset the existing blip object for a staggered drop-in.
            src.x = txVX;
            src.gridX = gridX;
            src.stackY = stackY;
            src.height = brickH + stackY;
            src.fadeStart = 0;
            src.particles = null;
            src.absorbTargetX = undefined;
            src.absorbOriginX = undefined;
            src.isDrop = true;
            var xFrac = width > 0 ? (txVX - seg.x_start) / width : 0;
            src.timestamp = now + xFrac * stagger; // sweep left→right over `stagger`
            seg.blips.push(src);
            placed++;
        }
        if (i < n) {
            _hb._carryRaf = requestAnimationFrame(placeChunk);
        } else {
            _hb._carryRaf = null;
            console.log('[heartbeat] harvest: carried ' + placed + '/' + n +
                ' unconfirmed leftovers onto new flatline (width ' + Math.round(width) + 'px, chunked)');
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
        ? 'ENABLED — density columns when zoomed out (<0.3x) + full (>3000 bricks)'
        : 'DISABLED — individual bricks always'));
    return s._lodEnabled;
};

// Toggle the cinematic block reveal on/off. Off = the instant flow (spike appears,
// harvest, mempool re-lays, no camera hijack; follow state respected). Persists to
// localStorage. Call from the console: window._hbToggleReveal().
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
        // Don't update lastBlockTime from LiveStats — it resets the "Last block"
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
// the rest re-lays on the new flatline) — same as a live block, but INSTANT (this
// isn't processLiveBlock, so no cinematic reveal is armed). Without harvest the fee
// threshold is -Infinity and the WHOLE mempool shatters into the spike, leaving the
// new flatline empty — a block you were away for would wipe the mempool.
function replayQueuedBlock(qb) {
    var _hb = getState();
    if (_hb) _hb._harvestLive = true;
    window.pushHeartbeatBlocks(JSON.stringify([{
        height: qb.height,
        timestamp: qb.timestamp || Math.floor(Date.now() / 1000),
        tx_count: qb.tx_count || qb.confirmed_count || 3000,
        total_fees: qb.total_fees || 0,
        size: qb.size || 0,
        weight: qb.weight || 3000000
    }]), false);
}

// After a tab return, make the timeline end at the live tip. SSE-queued blocks
// (delivered while hidden) are the freshest, most complete source, so replay
// those first; if the timeline is still behind the tip, fetch /api/stats/blocks
// once and replay the remainder. pushHeartbeatBlocks dedups by height, so an
// overlap between the queue and the fetch cannot double-render a block.
function catchUpBlocks(queued, tipHeight) {
    var _hb = getState();
    if (!_hb) return;

    function timelineMaxHeight() {
        for (var i = _hb.timeline.length - 1; i >= 0; i--) {
            if (_hb.timeline[i].type === 'block') return _hb.timeline[i].height;
        }
        return 0;
    }

    var maxH = timelineMaxHeight();
    var q = (queued || [])
        .filter(function(b) { return b && b.height; })
        .sort(function(a, b) { return a.height - b.height; });
    var replayed = 0;
    var newestTs = 0;
    for (var i = 0; i < q.length; i++) {
        if (q[i].height <= maxH) continue;
        replayQueuedBlock(q[i]);
        maxH = q[i].height;
        newestTs = q[i].timestamp || newestTs;
        replayed++;
    }
    if (replayed > 0) {
        console.log('[heartbeat] tab return: replayed', replayed, 'queued blocks (tip ' + tipHeight + ')');
        // Widen the post-block flatline to the newest block's age so the resumed
        // live tx flow spreads instead of piling (same as processLiveBlock).
        fastForwardLiveFlatline(newestTs);
    }

    // Still behind the tip? The queue missed some (e.g. SSE reconnected while
    // hidden). Fetch the canonical list once and replay the gap.
    if (tipHeight > maxH) {
        var fromH = maxH;
        fetch('/api/stats/blocks')
            .then(function(r) { return r.json(); })
            .then(function(data) {
                // Re-read state fresh: a full reset could have replaced it while
                // the fetch was in flight. Advancing virtualX must target the same
                // object replayQueuedBlock writes to.
                var s = getState();
                if (!s) return;
                var blocks = (data.blocks || [])
                    .filter(function(b) { return b.height > fromH; })
                    .sort(function(a, b) { return a.height - b.height; });
                for (var j = 0; j < blocks.length; j++) {
                    // Give inner gap blocks a proportional (compressed) lead-in
                    // flatline from their real inter-block time, so a multi-block
                    // fetch-fill doesn't collapse to ~30px per block. The first
                    // block's lead-in is the existing live flatline (already sized
                    // to time since the last known block), so only widen for j>0.
                    if (j > 0) {
                        var gap = Math.max(0, blocks[j].timestamp - blocks[j - 1].timestamp);
                        s.virtualX += historyFlatlineWidth(gap);
                    }
                    replayQueuedBlock(blocks[j]);
                }
                if (blocks.length > 0) {
                    console.log('[heartbeat] tab return: filled', blocks.length, 'blocks from fetch');
                    fastForwardLiveFlatline(blocks[blocks.length - 1].timestamp);
                }
            })
            .catch(function() {});
    }
}

// Safety net for blocks queued while the tab was hidden (see the SSE 'block'
// handler). Normally _hbVisibilityChange drains the queue on return, but if that
// event doesn't fire — a brief/edge-case hide, a focus race, or the block event
// landing with document.hidden===true just before the tab is shown — the queued
// block would strand: the canvas stays on the old block while the header (fed by
// LiveStats, which updates regardless of visibility) shows the new height.
// drawFrame calls this every frame, and RAF only runs at full rate when the tab
// is genuinely visible, so a stranded block is replayed within a frame of the
// tab being shown — no dependence on visibilitychange. Cheap: no-ops instantly
// when the queue is empty (the normal case).
window._hbDrainBlockQueue = function() {
    var _hb = getState();
    if (!_hb || document.hidden) return;
    if (!_hb._blockQueue || _hb._blockQueue.length === 0) return;
    var queued = _hb._blockQueue.slice();
    var tipHeight = _hb.blockHeight || 0;
    _hb._blockQueue = [];
    console.log('[heartbeat] draining', queued.length, 'queued block(s) on visible frame');
    catchUpBlocks(queued, tipHeight);
};

function _hbVisibilityChange() {
    var _hb = getState();
    if (!_hb) return;
    if (document.hidden) {
        _hb._hiddenSince = Date.now() / 1000;
        return;
    }
    var now = Date.now() / 1000;

    // Finalize any in-progress block reveal FIRST. Its beats are keyed to a
    // wall-clock start (now - r.start); resuming after a hide would make elapsed
    // huge, teleporting through the beats and snapping the camera back to a spike
    // that's now far behind the advanced head. Ending it settles the state cleanly
    // before the catch-up below.
    if (_hb._reveal && window._hbEndReveal) window._hbEndReveal();

    // Blocks that arrived via SSE while hidden (the freshest source) plus the
    // live tip height from LiveStats. Captured before any reset so we can catch
    // the timeline up to the real tip on return (see catchUpBlocks).
    var queuedBlocks = (_hb._blockQueue || []).slice();
    var tipHeight = _hb.blockHeight || 0;
    _hb._blockQueue = [];

    // Full reset if hidden for more than 5 minutes
    if (_hb._hiddenSince > 0 && (now - _hb._hiddenSince) > 300) {
        var canvasId = _hb.canvas ? _hb.canvas.id : null;
        console.log('[heartbeat] tab return after', Math.round(now - _hb._hiddenSince), 's - full reset');
        if (canvasId) {
            // Fetch blocks FIRST, then destroy/init/replay so SSE (which starts
            // inside initHeartbeat) can't deliver blocks before history is loaded.
            fetch('/api/stats/blocks')
                .then(function(r) { return r.json(); })
                .then(function(data) {
                    var blocks = data.blocks || [];
                    // Compute inter_block_seconds from timestamps (API doesn't include it)
                    for (var i = 0; i < blocks.length; i++) {
                        var prevTs = i > 0 ? blocks[i - 1].timestamp : (blocks[i].timestamp - 600);
                        blocks[i].inter_block_seconds = Math.max(0, blocks[i].timestamp - prevTs);
                    }
                    window.destroyHeartbeat();
                    window.initHeartbeat(canvasId);
                    // Suppress announcements until replay finishes (SSE may
                    // deliver blocks between init and replay completion)
                    var s = getState();
                    if (s) s._suppressAnnounce = true;
                    if (blocks.length > 0) {
                        console.log('[heartbeat] replaying', blocks.length, 'blocks after reset');
                        window.pushHeartbeatBlocks(JSON.stringify(blocks), true);
                    }
                    // /api/stats/blocks can trail the ZMQ tip (the 15s poller
                    // writes it; the ZMQ block path does not), so the fetched
                    // rebuild may be behind. Catch up from the SSE-queued blocks
                    // captured before the reset, then fetch-fill if still short.
                    catchUpBlocks(queuedBlocks, tipHeight);
                    if (s) s._suppressAnnounce = false;
                })
                .catch(function(err) {
                    console.log('[heartbeat] block refetch failed:', err);
                    // Still reset even if fetch fails
                    window.destroyHeartbeat();
                    window.initHeartbeat(canvasId);
                });
        }
        return;
    }

    // Advance virtualX by full elapsed hidden time
    if (_hb._hiddenSince > 0) {
        var hiddenDuration = now - _hb._hiddenSince;
        if (hiddenDuration > 0) {
            _hb.virtualX += hiddenDuration * FLATLINE_PX_PER_SEC;
            console.log('[heartbeat] tab return: advanced virtualX by', Math.round(hiddenDuration), 's (',
                Math.round(hiddenDuration * FLATLINE_PX_PER_SEC), 'px)');
        }
        _hb._hiddenSince = 0;
    }

    // Place buffered txs across the gap (the region between old bricks and
    // new virtualX). Timing is unreliable when hidden, but random spread
    // across the gap gives a natural fill instead of a visible hole.
    var liveSeg = _hb.timeline[_hb.timeline.length - 1];
    if (liveSeg && liveSeg.type === 'flatline' && liveSeg.x_end === null) {
        var buffered = (_hb._hiddenTxBuffer || []).concat(_hb._txBatchQueue || []);
        if (buffered.length > 0) {
            // Find where existing bricks end
            var rightmost = liveSeg.x_start;
            for (var bi = 0; bi < (liveSeg.blips || []).length; bi++) {
                if (liveSeg.blips[bi].fadeStart === 0 && liveSeg.blips[bi].x > rightmost) {
                    rightmost = liveSeg.blips[bi].x;
                }
            }
            var gapStart = rightmost + 5;
            var gapEnd = _hb.virtualX - 20; // leave 20px for live txs
            var gapSpan = gapEnd - gapStart;

            if (gapSpan > 10 && buffered.length > 0) {
                var medianFee = _hb._wsMedianFee || 5;
                var placed = 0;
                var maxStack = (_hb.height || 400) * 0.35;
                if (!liveSeg._colHeights) liveSeg._colHeights = {};

                for (var ti = 0; ti < buffered.length; ti++) {
                    var tx = buffered[ti];
                    if (!tx.fee || !tx.vsize) continue;
                    var feeRate = tx.fee / tx.vsize;
                    var feeNorm = Math.min(Math.log2(feeRate + 1) / 6, 1.0);
                    var brickH = 3 + feeNorm * 14 + Math.random() * 3;
                    var txVX = gapStart + Math.random() * gapSpan;
                    var gridX = Math.round(txVX / 5) * 5;
                    var stackY = liveSeg._colHeights[gridX] || 0;
                    if (stackY > maxStack) continue;
                    liveSeg._colHeights[gridX] = stackY + brickH;

                    liveSeg.blips.push({
                        x: txVX, gridX: gridX,
                        height: brickH + stackY, brickH: brickH, brickW: 4, stackY: stackY,
                        color: feeRateColor(feeRate, medianFee), opacity: 0.75 + feeNorm * 0.2,
                        txCount: 1, txid: tx.txid || null,
                        feeRate: Math.round(feeRate * 10) / 10,
                        vsize: tx.vsize || 0, value: tx.value || 0,
                        timestamp: now, isDrop: false, fadeStart: 0,
                        bobPhase: Math.random() * Math.PI * 2,
                        bobSpeed: 1.2 + feeNorm * 0.8,
                        lane: Math.floor(Math.random() * 5) - 2,
                        feeRatio: medianFee > 0 ? feeRate / medianFee : 1
                    });
                    placed++;
                }
                if (placed > 0) {
                    console.log('[heartbeat] tab return: placed', placed, 'buffered txs across gap');
                }
            }
        }
    }
    _hb._hiddenTxBuffer = [];
    _hb._txBatchQueue = [];

    // Catch the timeline up to the live tip: replay the SSE-queued blocks that
    // arrived while hidden, then fetch-fill any remaining gap (see catchUpBlocks).
    catchUpBlocks(queuedBlocks, tipHeight);

    // Sync frame timing so the draw loop doesn't try to catch up
    _hb.lastFrameTime = now;
}
document.addEventListener('visibilitychange', _hbVisibilityChange);

// ── Re-export vitals functions on window ───────────────────

window.getHeartbeatVitals = function() { return getHeartbeatVitals(); };
window.renderRhythmStrip = function(id, json) { renderRhythmStrip(id, json); };
window.getHeartbeatRecentBlocks = function() { return getHeartbeatRecentBlocks(); };
// Freshest block on the timeline (height + on-chain timestamp), from the
// SSE/ZMQ stream. The Rust header polls this so the block height and
// "last block" timer follow the fastest source, not just the LiveStats RPC.
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
// Connection state for the header LIVE indicator: 'disconnected' when the SSE
// EventSource errored, 'stale' when it's open but silent past the threshold
// (node RPC stall), otherwise 'live'. The Rust header polls this on its 1s tick.
window.getHeartbeatConnectionState = function() {
    var _hb = getState();
    if (!_hb) return 'live';
    if (_hb._sseDisconnected) return 'disconnected';
    var last = _hb._lastSseEventTs || 0;
    if (last > 0 && (Date.now() / 1000 - last) > HEARTBEAT_STALL_SECS) return 'stale';
    return 'live';
};
window.heartbeatCenter = function() { heartbeatCenter(); };
window.heartbeatSearchTx = function(txid) { return heartbeatSearchTx(txid); };
window.heartbeatPulse = function() { heartbeatPulse(); };
window.heartbeatFlash = function() { heartbeatFlash(); };
window.getOrganismStatus = function() { return getOrganismStatus(); };
window.heartbeatSoundToggle = function(enable) { return heartbeatSoundToggle(enable); };
window.heartbeatSoundIsEnabled = function() { return heartbeatSoundIsEnabled(); };
window.heartbeatPlaySound = function() { heartbeatPlaySound(); };
window.heartbeatDownloadCapture = function(json) { heartbeatDownloadCapture(json); };
