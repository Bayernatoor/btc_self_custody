// heartbeat-interaction.js: User input handling, momentum scrolling, control buttons
import { getState, HEAD_POSITION_FRAC, FLATLINE_PX_PER_SEC } from './heartbeat-state.js';
import { canvasToVirtual, virtualToCanvas, blockAtVirtualX, pendingBlockAtVirtualX, flatlineAtVirtualX, blipAtCanvasXY, blipAtVirtualX } from './heartbeat-blips.js';
import { brickHeightScale } from './heartbeat-timeline.js';
import { isSoundEnabled, toggleSound, unlockAudioOnGesture } from './heartbeat-audio.js';

// Clamp vertical pan (viewOffsetY shifts the baseline DOWN when positive).
// Allows freely panning up/down so that even at 30x+ zoom (where brick stacks
// are taller than the entire viewport) you can drag the baseline past the bottom
// edge to inspect the top bricks of any stack, while keeping content reachable.
export function clampViewOffsetY(_hb) {
    if (!_hb) return;
    var h = _hb.height || 400;
    var base = (h < 350 ? h * 0.78 : h * 0.55);
    var zoom = _hb.zoom;
    var heightScale = brickHeightScale(zoom);
    // Measure actual tallest stack on the active live flatline when available
    // (fallback to ~160px virtual ceiling) so you can always reach the top brick
    // without scrolling into miles of empty sky.
    var tallestVirtual = 160;
    if (_hb.timeline && _hb.timeline.length > 0) {
        var tail = _hb.timeline[_hb.timeline.length - 1];
        if (tail && tail.type === 'flatline' && tail._colHeights) {
            var observedMax = 0;
            for (var k in tail._colHeights) {
                if (tail._colHeights[k] > observedMax) observedMax = tail._colHeights[k];
            }
            if (observedMax > 0) tallestVirtual = Math.max(80, Math.min(observedMax + 24, 300));
        }
    }
    var maxContentTop = Math.min(tallestVirtual, h * 0.45) * heightScale;
    var topMargin = 24;
    var bottomMargin = 24;
    var defaultHi = h - bottomMargin - base;
    var topRevealHi = topMargin + maxContentTop - base;
    // Allow panning UP far enough to inspect S-dips below the baseline, and panning
    // DOWN all the way to the bottom edge of the canvas (and further at high zoom so
    // the topmost brick of tall stacks is always reachable).
    var lo = Math.min(0, h * 0.30 - base);
    var hi = Math.max(defaultHi, topRevealHi);
    var v = _hb.viewOffsetY || 0;
    if (v < lo) v = lo;
    if (v > hi) v = hi;
    _hb.viewOffsetY = v;
}

// Momentum scrolling
export function startMomentum(velocity) {
    var _hb = getState();
    if (!_hb) return;
    _hb._momentumVel = velocity * 15; // scale up for visible effect
    _hb._momentumId = null;

    function tick() {
        // Re-read state each tick (not the captured _hb): after destroyHeartbeat's
        // setState(null) the captured ref stays truthy, so a stale tick would
        // reschedule forever. getState() goes null on teardown → the loop stops.
        var s = getState();
        if (!s || Math.abs(s._momentumVel) < 0.5) {
            if (s) s._momentumId = null;
            checkAutoFollow();
            return;
        }
        s.viewOffset -= s._momentumVel / s.zoom;
        s._momentumVel *= 0.92; // friction
        s._momentumId = requestAnimationFrame(tick);
    }
    tick();
}

export function stopMomentum() {
    var _hb = getState();
    if (_hb && _hb._momentumId) {
        cancelAnimationFrame(_hb._momentumId);
        _hb._momentumId = null;
    }
}

// Pinch-to-zoom helpers
export function touchDistance(t1, t2) {
    var dx = t1.clientX - t2.clientX;
    var dy = t1.clientY - t2.clientY;
    return Math.sqrt(dx * dx + dy * dy);
}

export function touchMidpoint(t1, t2, rect) {
    return {
        x: (t1.clientX + t2.clientX) / 2 - rect.left,
        y: (t1.clientY + t2.clientY) / 2 - rect.top
    };
}

export function tryControlClick(mx, my) {
    var _hb = getState();
    if (!_hb || !_hb._ctrlBtns) return false;
    for (var i = 0; i < _hb._ctrlBtns.length; i++) {
        var btn = _hb._ctrlBtns[i];
        if (mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h) {
            handleControlClick(btn.id);
            return true;
        }
    }
    return false;
}

export function checkControlHover(mx, my) {
    var _hb = getState();
    if (!_hb || !_hb._ctrlBtns) { if (_hb) _hb._ctrlHover = null; return; }
    _hb._ctrlHover = null;
    for (var i = 0; i < _hb._ctrlBtns.length; i++) {
        var btn = _hb._ctrlBtns[i];
        if (mx >= btn.x && mx <= btn.x + btn.w && my >= btn.y && my <= btn.y + btn.h) {
            _hb._ctrlHover = btn;
            return;
        }
    }
}

export function listen(target, evt, fn, opts) {
    var _hb = getState();
    target.addEventListener(evt, fn, opts);
    _hb._listeners.push({ target: target, evt: evt, fn: fn, opts: opts });
}

// Select and pin a brick on the canvas, notifying the Rust/WASM PinnedTxPanel
// when registered (or falling back to the legacy detail modal).
function selectBrick(_hb, blip) {
    if (!_hb || !blip || !blip.txid) return;
    _hb._pinnedTxid = blip.txid;
    _hb._pinnedBlip = blip;
    _hb.autoFollow = false;
    if (typeof window.__hbSelectTx === 'function') {
        window.__hbSelectTx(blip.txid);
    } else if (typeof window.showTxDetail === 'function') {
        window.showTxDetail(blip.txid, {
            fee: blip.fee != null ? blip.fee : null,
            vsize: blip.vsize != null ? blip.vsize : null,
            value: blip.value != null ? blip.value : null,
            feeRate: blip.feeRate != null ? blip.feeRate : null,
            firstSeen: blip.firstSeen || null
        });
    }
}

// Handle a canvas tap (touch). On a brick: opens the live in-place PinnedTxPanel
// immediately via window.__hbSelectTx when available (else falls back to two-tap
// tooltip -> showTxDetail). On a block spike: two-tap tooltip -> showBlockDetail.
function handleCanvasTap(mx, my) {
    var _hb = getState();
    if (!_hb) return;
    var vx = canvasToVirtual(mx);
    var baseline = (_hb.height < 350 ? _hb.height * 0.78 : _hb.height * 0.55) + (_hb.viewOffsetY || 0);

    // Brick (blip) hit-test above the baseline: mirror the hover zoom thresholds:
    // 2D at high zoom, x-only at moderate zoom, none when too dense/zoomed out.
    var blip = null;
    if (my < baseline) {
        if (_hb.zoom >= 4) blip = blipAtCanvasXY(mx, my, baseline);
        if (!blip && _hb.zoom >= 0.8) blip = blipAtVirtualX(vx, 6 / _hb.zoom);
    }
    if (blip && blip.txid) {
        if (typeof window.__hbSelectTx === 'function') {
            _hb.hoveredBlip = null;
            _hb.hoveredBlock = null;
            _hb.hoveredPendingBlock = null;
            selectBrick(_hb, blip);
        } else if (_hb._pinnedBlip === blip || _hb.hoveredBlip === blip) {
            // Legacy fallback: second tap on the same brick -> open the tx detail modal.
            selectBrick(_hb, blip);
            _hb.hoveredBlip = null;
        } else {
            // Legacy fallback: first tap -> pin + show the tooltip.
            _hb._pinnedTxid = blip.txid;
            _hb._pinnedBlip = blip;
            _hb.hoveredBlip = blip;
            _hb.hoverCanvasX = mx;
            _hb.autoFollow = false;
            _hb.hoveredBlock = null;
            _hb.hoveredPendingBlock = null;
        }
        return;
    }

    // Block spike.
    var tapped = blockAtVirtualX(vx);
    if (tapped) {
        if (_hb.hoveredBlock === tapped) {
            // Second tap on the same spike -> open the block detail modal.
            if (tapped.height != null && tapped.height >= 0 && typeof window.showBlockDetail === 'function') {
                window.showBlockDetail(tapped.height);
            }
            _hb.hoveredBlock = null;
        } else {
            // First tap -> select + show the tooltip.
            _hb.hoveredBlock = tapped;
            _hb.hoveredPendingBlock = null;
            _hb.hoveredBlip = null;
        }
        return;
    }

    // Pending block reserved slot.
    var tappedPending = pendingBlockAtVirtualX(vx);
    if (tappedPending) {
        _hb.hoveredPendingBlock = (_hb.hoveredPendingBlock === tappedPending) ? null : tappedPending;
        _hb.hoveredBlock = null;
        _hb.hoveredBlip = null;
        return;
    }

    // Tapped empty space -> dismiss hover tooltips AND pinned tx panel (matches desktop click).
    _hb.hoveredBlock = null;
    _hb.hoveredPendingBlock = null;
    _hb.hoveredBlip = null;
    _hb.hoveredFlatline = null;
    if (_hb._pinnedTxid || _hb._pinnedBlip) {
        _hb._pinnedTxid = null;
        _hb._pinnedBlip = null;
        if (typeof window !== 'undefined' && typeof window.__hbClearPinnedTx === 'function') {
            window.__hbClearPinnedTx();
        }
    }
}

export function setupInputHandlers(canvas) {
    var _hb = getState();
    // Track listeners for cleanup in destroyHeartbeat
    _hb._listeners = [];

    // Mouse wheel: zoom (centered on cursor). Shift+wheel: pan.
    listen(canvas, 'wheel', function(e) {
        if (!_hb) return;
        // End an in-progress block reveal: a wheel-zoom doesn't flip
        // isDragging/paused/pinching, so runBlockReveal wouldn't catch it otherwise.
        if (_hb._reveal && window._hbEndReveal) window._hbEndReveal();
        e.preventDefault();

        if (e.shiftKey) {
            // Shift+wheel: horizontal pan
            _hb.viewOffset += e.deltaY * 0.8 / _hb.zoom;
            _hb.autoFollow = false;
            checkAutoFollow();
        } else {
            // Wheel: zoom centered on cursor position
            var rect = canvas.getBoundingClientRect();
            var mx = e.clientX - rect.left;
            // Virtual x under the cursor before zoom
            var vxUnderCursor = canvasToVirtual(mx);

            // Apply zoom. Scale the step by the wheel delta (exp) rather than a
            // fixed factor per event, so it's consistent + fine across devices (a
            // mouse notch ≈ deltaY 100 → ~12%; a trackpad's many small deltas each
            // nudge a little instead of compounding to a big jump). Clamp so one
            // large delta can't leap multiple increments.
            var zoomDelta = Math.exp(-e.deltaY * 0.0012);
            zoomDelta = Math.max(0.5, Math.min(2, zoomDelta));
            _hb.zoom = Math.max(_hb.minZoom, Math.min(_hb.maxZoom, _hb.zoom * zoomDelta));

            // Adjust viewOffset so the virtual point under cursor stays put
            _hb.viewOffset = vxUnderCursor - mx / _hb.zoom;
            // Re-clamp vertical pan: zooming out shrinks the reachable range, so a
            // prior vertical pan could otherwise strand the timeline off-screen.
            clampViewOffsetY(_hb);

            // Keep LIVE engaged if the live tip remains visibly inside the canvas;
            // disengage cleanly if zooming aimed off-screen into older history.
            checkAutoFollow();
        }
    }, { passive: false });

    // Mouse drag: click and drag to pan
    listen(canvas, 'mousedown', function(e) {
        if (!_hb) return;
        if (_hb._reveal && window._hbEndReveal) window._hbEndReveal(); // user takes over

        var rect = canvas.getBoundingClientRect();
        var mx = e.clientX - rect.left, my = e.clientY - rect.top;
        stopMomentum();
        _hb.isDragging = true;
        _hb.autoFollow = false; // release immediately so drag isn't fought
        _hb.dragStartX = e.clientX;
        _hb.dragStartY = e.clientY;
        _hb.dragStartOffset = _hb.viewOffset;
        _hb.dragStartOffsetY = _hb.viewOffsetY || 0;
        _hb._lastDragX = e.clientX;
        _hb._lastDragTime = Date.now();
        _hb._dragVelocity = 0;
        canvas.style.cursor = 'grabbing';
    });

    listen(window, 'mousemove', function(e) {
        if (!_hb) return;

        if (_hb.isDragging) {
            var dx = e.clientX - _hb.dragStartX;
            var dy = e.clientY - (_hb.dragStartY || e.clientY);
            _hb.viewOffset = _hb.dragStartOffset - dx / _hb.zoom;
            _hb.viewOffsetY = (_hb.dragStartOffsetY || 0) + dy;
            clampViewOffsetY(_hb);
            _hb.autoFollow = false;
            // Track velocity for momentum (clamped to prevent flyaway on tiny dt)
            var now = Date.now();
            var dt = now - (_hb._lastDragTime || now);
            if (dt > 0) {
                var rawVel = (e.clientX - (_hb._lastDragX || e.clientX)) / dt;
                _hb._dragVelocity = Math.max(-500, Math.min(500, rawVel));
            }
            _hb._lastDragX = e.clientX;
            _hb._lastDragTime = now;
            checkAutoFollow();
        } else {
            // Hover detection
            var rect = canvas.getBoundingClientRect();
            var mx = e.clientX - rect.left;
            var my = e.clientY - rect.top;
            if (mx >= 0 && mx <= _hb.width && my >= 0 && my <= _hb.height) {
                var vx = canvasToVirtual(mx);
                var baseline = (_hb.height < 350 ? _hb.height * 0.78 : _hb.height * 0.55) + (_hb.viewOffsetY || 0);
                _hb.hoveredBlock = blockAtVirtualX(vx);
                _hb.hoveredPendingBlock = !_hb.hoveredBlock ? pendingBlockAtVirtualX(vx) : null;
                _hb.hoveredBlip = null;
                _hb.hoveredFlatline = null;

                if (!_hb.hoveredBlock && !_hb.hoveredPendingBlock) {
                    // In bloodstream mode, cells exist above AND below the baseline.
                    // Check the full tube zone for hits before falling back to flatline.
                    var isBs = _hb.renderMode === 'bloodstream';
                    var avgCellR = isBs ? 4.0 * Math.max(_hb.zoom * 0.4, 0.8) : 0;
                    var tubeZone = isBs ? Math.max(30, avgCellR * 2.5) + avgCellR : 0;
                    var inTube = isBs && Math.abs(my - baseline) < tubeZone;

                    // 2D hit test at zoom >= 4x (check both sides in bloodstream)
                    if ((my < baseline || inTube) && _hb.zoom >= 4) {
                        _hb.hoveredBlip = blipAtCanvasXY(mx, my, baseline);
                    }
                    // Fallback to x-only match down to zoom >= 0.8x
                    if (!_hb.hoveredBlip && (my < baseline || inTube) && _hb.zoom >= 0.8) {
                        var hitR = (_hb.zoom < 1.5 ? 5 : 4) / _hb.zoom;
                        _hb.hoveredBlip = blipAtVirtualX(vx, hitR);
                    }
                    // Flatline tooltip only when cursor is at/below baseline (not in brick area above)
                    if (!_hb.hoveredBlip && !inTube && my >= baseline - 5) {
                        _hb.hoveredFlatline = flatlineAtVirtualX(vx);
                    }
                }
                _hb.hoverCanvasX = mx;
                canvas.style.cursor = _hb.hoveredBlock ? 'pointer'
                    : _hb.hoveredPendingBlock ? 'help'
                    : _hb.hoveredBlip ? 'pointer'
                    : _hb.hoveredFlatline ? 'crosshair' : 'default';
            } else {
                _hb.hoveredBlock = null;
                _hb.hoveredPendingBlock = null;
                _hb.hoveredBlip = null;
                _hb.hoveredFlatline = null;
            }
        }
    });

    listen(window, 'mouseup', function() {
        if (!_hb) return;
        if (_hb.isDragging) {
            _hb.isDragging = false;
            canvas.style.cursor = 'default';
            // Start momentum scrolling
            var vel = _hb._dragVelocity || 0;
            if (Math.abs(vel) > 0.1) {
                startMomentum(vel);
            } else {
                checkAutoFollow();
            }
        }
    });

    listen(canvas, 'click', function(e) {
        if (!_hb) return;
        if (_hb._touchTapped) { _hb._touchTapped = false; return; }
        var dragDist = Math.abs(e.clientX - (_hb.dragStartX != null ? _hb.dragStartX : e.clientX))
            + Math.abs(e.clientY - (_hb.dragStartY != null ? _hb.dragStartY : e.clientY));
        if (dragDist > 6) return;
        // Click on a brick -> pin it and notify live PinnedTxPanel (or fallback detail modal)
        if (_hb.hoveredBlip && _hb.hoveredBlip.txid) {
            selectBrick(_hb, _hb.hoveredBlip);
        } else if (_hb.hoveredBlock && _hb.hoveredBlock.height != null && _hb.hoveredBlock.height >= 0 && typeof window.showBlockDetail === 'function') {
            // Clicking a block spike -> open block detail modal (including Genesis #0)
            window.showBlockDetail(_hb.hoveredBlock.height);
        } else if (!_hb.hoveredPendingBlock) {
            // Click on empty canvas space -> dismiss hover tooltips & pinned tx selection
            _hb.hoveredBlock = null;
            _hb.hoveredPendingBlock = null;
            _hb.hoveredBlip = null;
            _hb.hoveredFlatline = null;
            _hb._pinnedTxid = null;
            _hb._pinnedBlip = null;
            if (typeof window.__hbClearPinnedTx === 'function') {
                window.__hbClearPinnedTx();
            }
        }
    });

    // Keyboard shortcuts (ignored when typing in inputs, composing, or holding modifiers)
    listen(window, 'keydown', function(e) {
        if (!_hb || !e || e.defaultPrevented || e.isComposing || e.metaKey || e.ctrlKey || e.altKey) return;
        var k = e.key;
        if (k === 'Escape') {
            var hintEl = typeof document !== 'undefined'
                ? document.getElementById('heartbeat-hint-overlay')
                : null;
            if (hintEl && !hintEl.classList.contains('hidden')) {
                hintEl.click();
                return;
            }
            var lmClose = typeof document !== 'undefined'
                ? document.getElementById('heartbeat-landmarks-close')
                : null;
            if (lmClose) {
                lmClose.click();
                return;
            }
        }
        var active = typeof document !== 'undefined' ? document.activeElement : null;
        var activeTag = active && active.tagName ? active.tagName.toUpperCase() : '';
        if (active && (activeTag === 'INPUT' || activeTag === 'TEXTAREA' || activeTag === 'SELECT' || active.isContentEditable)) {
            return;
        }
        if (k === 'Escape') {
            var hadPinned = !!(_hb._pinnedTxid || _hb._pinnedBlip);
            _hb.hoveredBlock = null;
            _hb.hoveredPendingBlock = null;
            _hb.hoveredBlip = null;
            _hb.hoveredFlatline = null;
            _hb._pinnedTxid = null;
            _hb._pinnedBlip = null;
            if (hadPinned && typeof window.__hbClearPinnedTx === 'function') {
                window.__hbClearPinnedTx();
            }
            return;
        }
        if (e.repeat) return;
        if (k === '/') {
            var searchEl = typeof document !== 'undefined' ? document.getElementById('heartbeat-tx-search') : null;
            if (searchEl) {
                e.preventDefault();
                searchEl.focus();
                if (typeof searchEl.select === 'function') searchEl.select();
            }
            return;
        }
        if (k === ' ' || k === 'Spacebar') {
            if (e.shiftKey) return;
            var activeRole = (active && typeof active.getAttribute === 'function')
                ? active.getAttribute('role')
                : null;
            if (activeTag === 'BUTTON' || activeTag === 'A' || activeTag === 'SUMMARY'
                || activeRole === 'button' || activeRole === 'link') {
                return;
            }
            e.preventDefault();
            handleControlClick('pause');
        } else if (k === 'l' || k === 'L') {
            handleControlClick('live');
        } else if (k === 'f' || k === 'F') {
            handleControlClick('fit');
        } else if (k === 'ArrowLeft') {
            e.preventDefault();
            handleControlClick('prev');
        } else if (k === 'ArrowRight') {
            e.preventDefault();
            handleControlClick('next');
        } else if (k === '+' || k === '=') {
            handleControlClick('zoomIn');
        } else if (k === '-' || k === '_') {
            handleControlClick('zoomOut');
        } else if (k === 'm' || k === 'M') {
            handleControlClick('sound');
        }
    });

    // Touch support: 1 finger = pan with momentum, 2 fingers = pinch-to-zoom
    listen(canvas, 'touchstart', function(e) {
        if (!_hb) return;
        _hb._isTouch = true; // enables tap-to-select tooltips + "tap again" hints
        if (_hb._reveal && window._hbEndReveal) window._hbEndReveal(); // user takes over
        stopMomentum();

        if (e.touches.length === 2) {
            // Pinch start
            _hb._pinching = true;
            _hb._gestureWasPinch = true; // don't fire a tap when fingers lift
            _hb.isDragging = false;
            _hb._touchLocked = 'pinch';
            _hb._pinchStartDist = touchDistance(e.touches[0], e.touches[1]);
            _hb._pinchStartZoom = _hb.zoom;
            var rect = canvas.getBoundingClientRect();
            _hb._pinchMid = touchMidpoint(e.touches[0], e.touches[1], rect);
            _hb._pinchVx = canvasToVirtual(_hb._pinchMid.x);
            _hb._pinchLastMidY = _hb._pinchMid.y; // track finger travel for vertical pan
        } else if (e.touches.length === 1) {
            // Pan start: don't commit to horizontal drag yet
            _hb._pinching = false;
            _hb._gestureWasPinch = false; // a fresh single-finger touch can be a tap
            _hb.isDragging = false;
            _hb._touchLocked = null; // null = undecided, 'h' = horizontal, 'v' = vertical
            _hb._touchStartX = e.touches[0].clientX;
            _hb._touchStartY = e.touches[0].clientY;
            _hb.dragStartX = e.touches[0].clientX;
            _hb.dragStartOffset = _hb.viewOffset;
            _hb._lastDragX = e.touches[0].clientX;
            _hb._lastDragTime = Date.now();
            _hb._dragVelocity = 0;
        }
    }, { passive: true });

    listen(canvas, 'touchmove', function(e) {
        if (!_hb) return;

        if (_hb._pinching && e.touches.length === 2) {
            e.preventDefault();
            // Two-finger gesture = pinch-zoom AND 2D pan in one motion (maps-style).
            var rect = canvas.getBoundingClientRect();
            var dist = touchDistance(e.touches[0], e.touches[1]);
            var mid = touchMidpoint(e.touches[0], e.touches[1], rect);
            var scale = dist / _hb._pinchStartDist;
            var newZoom = Math.max(_hb.minZoom, Math.min(_hb.maxZoom, _hb._pinchStartZoom * scale));
            _hb.zoom = newZoom;
            // Horizontal: keep the virtual point grabbed at gesture start glued under
            // the CURRENT finger midpoint, so the gesture zooms and pans left/right.
            _hb.viewOffset = _hb._pinchVx - mid.x / newZoom;
            // Vertical: accumulate the midpoint's y travel into viewOffsetY. It's a
            // screen-space offset (not zoom-scaled) to match the baseline convention
            // and the mouse-drag path. Clamp so you can reach the tallest stack only.
            _hb.viewOffsetY = (_hb.viewOffsetY || 0) + (mid.y - _hb._pinchLastMidY);
            _hb._pinchLastMidY = mid.y;
            clampViewOffsetY(_hb);
            _hb.autoFollow = false;
        } else if (e.touches.length === 1) {
            // Decide direction on first significant move
            if (!_hb._touchLocked) {
                var tdx = Math.abs(e.touches[0].clientX - _hb._touchStartX);
                var tdy = Math.abs(e.touches[0].clientY - _hb._touchStartY);
                if (tdx + tdy < 8) return; // too small to decide
                _hb._touchLocked = tdx > tdy ? 'h' : 'v';
                if (_hb._touchLocked === 'h') {
                    _hb.isDragging = true;
                    _hb.autoFollow = false;
                }
            }
            if (_hb._touchLocked === 'v') return; // let browser handle vertical scroll
            e.preventDefault(); // horizontal pan — block scroll
            // Pan
            var dx = e.touches[0].clientX - _hb.dragStartX;
            _hb.viewOffset = _hb.dragStartOffset - dx / _hb.zoom;
            _hb.autoFollow = false;
            // Track velocity (clamped to prevent flyaway on tiny dt)
            var now = Date.now();
            var dt = now - (_hb._lastDragTime || now);
            if (dt > 0) {
                var rawVel = (e.touches[0].clientX - (_hb._lastDragX || e.touches[0].clientX)) / dt;
                _hb._dragVelocity = Math.max(-500, Math.min(500, rawVel));
            }
            _hb._lastDragX = e.touches[0].clientX;
            _hb._lastDragTime = now;
            checkAutoFollow();
        }
    }, { passive: false });

    listen(canvas, 'touchend', function(e) {
        if (!_hb) return;
        var wasDragging = _hb.isDragging;
        _hb._touchLocked = null; // reset direction lock for next touch
        _hb._touchTapped = false; // reset per touchend

        if (_hb._pinching) {
            _hb._pinching = false;
            checkAutoFollow();
            return;
        }

        if (wasDragging) {
            _hb.isDragging = false;
            // Momentum
            var vel = _hb._dragVelocity || 0;
            if (Math.abs(vel) > 0.1) {
                startMomentum(vel);
            } else {
                checkAutoFollow();
            }
        }

        // Tap detection: a single-finger touch that barely moved. Runs regardless of
        // whether it locked into a drag: a real finger tap always has micro-jitter,
        // which used to lock 'h'/'v' and skip tap handling entirely (so a tap either
        // did nothing or fell through to the synthetic click against stale state).
        // Skip if this gesture was ever a pinch: lifting the last finger of a pinch
        // fires a 1-touch touchend with a stale _touchStartX that would false-tap.
        if (!_hb._gestureWasPinch && e.changedTouches && e.changedTouches.length === 1) {
            var rect = canvas.getBoundingClientRect();
            var ex = e.changedTouches[0].clientX, ey = e.changedTouches[0].clientY;
            var movedX = Math.abs(ex - (_hb._touchStartX != null ? _hb._touchStartX : ex));
            var movedY = Math.abs(ey - (_hb._touchStartY != null ? _hb._touchStartY : ey));
            if (movedX < 12 && movedY < 12) {
                handleCanvasTap(ex - rect.left, ey - rect.top);
                _hb._touchTapped = true; // suppress the synthetic click that follows
            }
        }
    });
}

// Speaker icon reflects the current state. Toggles pre-rendered SVG icons
// (with text fallback if legacy span markup is present) so zero innerHTML is needed.
function syncSoundButton(on) {
    var onSvg = document.getElementById('heartbeat-sound-on-svg');
    var offSvg = document.getElementById('heartbeat-sound-off-svg');
    if (onSvg && offSvg) {
        onSvg.classList.toggle('hidden', !on);
        offSvg.classList.toggle('hidden', on);
    } else {
        var icon = document.getElementById('heartbeat-btn-sound-icon');
        if (icon) icon.textContent = on ? '\u{1F50A}' : '\u{1F507}';
    }
    var btn = document.getElementById('heartbeat-btn-sound');
    if (btn) {
        btn.setAttribute('aria-pressed', on ? 'true' : 'false');
        btn.title = on ? 'Block sound on (M)' : 'Block sound off (M)';
        if (on) {
            btn.style.color = '#00e676';
            btn.style.background = 'rgba(0,230,118,0.14)';
        } else {
            btn.style.color = '';
            btn.style.background = '';
        }
    }
}

export function initSoundButton() {
    var on = isSoundEnabled();
    syncSoundButton(on);
    if (on) unlockAudioOnGesture();
}

export function handleControlClick(id) {
    // Handled before the preamble below: the sound toggle changes no view state, so it
    // must not cancel an in-progress reveal the way a zoom or pause press does.
    if (id === 'sound') {
        var on = toggleSound();
        syncSoundButton(on);
        return on; // window._hbToggleSound() reports this
    }
    var _hb = getState();
    if (!_hb) return;
    // Any explicit control press ends an in-progress block reveal (finalizing the
    // leftover placement) or first-load intro so neither fights the user's view.
    if (_hb._reveal && window._hbEndReveal) window._hbEndReveal();
    _hb._intro = null;
    switch (id) {
        case 'pause':
            _hb.paused = !_hb.paused;
            if (_hb.paused) {
                _hb.autoFollow = false;
            } else {
                // Resume: re-engage LIVE if the tip is visibly on screen, else play in place
                _hb._playResumeOffset = _hb.viewOffset;
                checkAutoFollow();
            }
            break;
        case 'prev':
            // Scroll to the previous block segment
            var centerVX = canvasToVirtual(_hb.width / 2);
            for (var i = _hb.timeline.length - 1; i >= 0; i--) {
                var seg = _hb.timeline[i];
                if (seg.type === 'block' && seg.x_end < centerVX - 10) {
                    _hb.autoFollow = false;
                    _hb.paused = true;
                    var mid = (seg.x_start + seg.x_end) / 2;
                    _hb.viewOffset = mid - _hb.width / 2 / _hb.zoom;
                    break;
                }
            }
            break;
        case 'next':
            // Scroll forward to the next block segment (works in both LIVE & Historical views)
            var centerVXNext = canvasToVirtual(_hb.width / 2);
            for (var ni = 0; ni < _hb.timeline.length; ni++) {
                var nSeg = _hb.timeline[ni];
                if (nSeg.type === 'block' && nSeg.x_start > centerVXNext + 10) {
                    _hb.autoFollow = false;
                    _hb.paused = true;
                    var nMid = (nSeg.x_start + nSeg.x_end) / 2;
                    _hb.viewOffset = nMid - _hb.width / 2 / _hb.zoom;
                    break;
                }
            }
            break;
        case 'zoomIn':
            var cx = _hb.width / 2;
            var vx = canvasToVirtual(cx);
            var nextIn = Math.min(_hb.maxZoom, _hb.zoom * 1.2); // 20%/click (was 40% — too coarse)
            if (nextIn >= 0.6 && nextIn <= 2.4) nextIn = Math.round(nextIn * 5) / 5;
            if (nextIn <= _hb.zoom) nextIn = Math.min(_hb.maxZoom, _hb.zoom + 0.2);
            _hb.zoom = nextIn;
            _hb.viewOffset = vx - cx / _hb.zoom;
            clampViewOffsetY(_hb); // keep vertical pan in range as the reachable range changes
            checkAutoFollow();
            break;
        case 'zoomOut':
            var cx2 = _hb.width / 2;
            var vx2 = canvasToVirtual(cx2);
            var nextOut = Math.max(_hb.minZoom, _hb.zoom / 1.2);
            if (nextOut >= 0.6 && nextOut <= 2.4) nextOut = Math.round(nextOut * 5) / 5;
            if (nextOut >= _hb.zoom) nextOut = Math.max(_hb.minZoom, _hb.zoom - 0.2);
            _hb.zoom = nextOut;
            _hb.viewOffset = vx2 - cx2 / _hb.zoom;
            clampViewOffsetY(_hb); // zoom-out shrinks the range — avoid stranding the timeline
            checkAutoFollow();
            break;
        case 'center':
            if (_hb._viewMode === 'history') {
                // Re-center on the requested historical target block (or window midpoint)
                var hTarget = null;
                for (var hti = 0; hti < _hb.timeline.length; hti++) {
                    if (_hb.timeline[hti]._historyTarget) { hTarget = _hb.timeline[hti]; break; }
                }
                if (hTarget) {
                    var hMid = (hTarget.x_start + hTarget.x_end) / 2;
                    _hb.viewOffset = hMid - (_hb.width * 0.5) / _hb.zoom;
                } else if (_hb.timeline.length > 0) {
                    var hFirst = _hb.timeline[0];
                    var hLast = _hb.timeline[_hb.timeline.length - 1];
                    var hSpanMid = (hFirst.x_start + (hLast.x_end !== null ? hLast.x_end : _hb.virtualX)) / 2;
                    _hb.viewOffset = hSpanMid - (_hb.width * 0.5) / _hb.zoom;
                }
                _hb.viewOffsetY = 0;
                break;
            }
            // Center viewport on the live head and reset vertical pan
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
            _hb.viewOffsetY = 0;
            break;
        case 'fit':
            if (_hb._viewMode === 'history') {
                // Frame the entire historical timeline extent
                if (_hb.timeline.length > 0) {
                    var hfFirst = _hb.timeline[0];
                    var hfLast = _hb.timeline[_hb.timeline.length - 1];
                    var hfStart = hfFirst.x_start;
                    var hfEnd = hfLast.x_end !== null ? hfLast.x_end : _hb.virtualX;
                    var hfSpan = Math.max(50, hfEnd - hfStart);
                    _hb.zoom = Math.max(_hb.minZoom, Math.min((_hb.width * 0.9) / hfSpan, _hb.maxZoom));
                    _hb.viewOffset = (hfStart + hfEnd) / 2 - (_hb.width * 0.5) / _hb.zoom;
                }
                _hb.autoFollow = false;
                _hb.paused = false;
                _hb.viewOffsetY = 0;
                _hb.hoveredBlock = null;
                _hb.hoveredPendingBlock = null;
                _hb.hoveredBlip = null;
                break;
            }
            // Frame the whole current mempool (the live flatline span) and follow.
            var fseg = _hb.timeline[_hb.timeline.length - 1];
            var fstart = (fseg && fseg.type === 'flatline' && fseg.x_end === null)
                ? fseg.x_start : (_hb.virtualX - 100);
            var fspan = Math.max(50, _hb.virtualX - fstart);
            _hb.zoom = Math.max(_hb.minZoom, Math.min((_hb.width * 0.8) / fspan, _hb.maxZoom));
            _hb.autoFollow = true;
            _hb.paused = false;
            _hb.viewOffsetY = 0;
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
            _hb.hoveredBlock = null;
            _hb.hoveredPendingBlock = null;
            _hb.hoveredBlip = null;
            _hb._pinnedBlip = null;
            _hb._pinnedTxid = null;
            if (typeof window.__hbClearPinnedTx === 'function') {
                window.__hbClearPinnedTx();
            }
            break;
        case 'mode':
            _hb.renderMode = _hb.renderMode === 'bricks' ? 'bloodstream' : 'bricks';
            break;
        case 'live':
            if (_hb._viewMode === 'history' && typeof window.hbV2ExitHistoryMode === 'function') {
                window.hbV2ExitHistoryMode();
                break;
            }
            // Re-engage head-follow at the CURRENT zoom (Fit is the separate
            // "frame the whole mempool" action: Live shouldn't fight it by
            // snapping back to a fixed zoom).
            _hb.autoFollow = true;
            _hb.paused = false;
            _hb.viewOffsetY = 0;
            _hb._liveHeadFrac = HEAD_POSITION_FRAC;
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
            _hb.hoveredBlock = null;
            _hb.hoveredPendingBlock = null;
            _hb.hoveredBlip = null;
            _hb._pinnedBlip = null;
            _hb._pinnedTxid = null;
            if (typeof window.__hbClearPinnedTx === 'function') {
                window.__hbClearPinnedTx();
            }
            break;
    }
}

// Keep LIVE auto-follow engaged whenever the live tip sits visibly inside the
// canvas (5%-97% across) and the viewer is not paused, pinned, or in history mode,
// anchoring LIVE at the user's chosen horizontal position so zooming or nudging
// never kicks them out of LIVE.
export function checkAutoFollow() {
    var _hb = getState();
    if (!_hb || _hb.paused || _hb._pinnedTxid || _hb._viewMode === 'history' || !_hb.width) return;
    var headOnCanvas = virtualToCanvas(_hb.virtualX);
    if (headOnCanvas >= _hb.width * 0.05 && headOnCanvas <= _hb.width * 0.97) {
        _hb.autoFollow = true;
        _hb._liveHeadFrac = Math.max(0.05, Math.min(0.95, headOnCanvas / _hb.width));
    } else {
        _hb.autoFollow = false;
    }
}
