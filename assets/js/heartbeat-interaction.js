// heartbeat-interaction.js — User input handling, momentum scrolling, control buttons
import { getState, HEAD_POSITION_FRAC, FLATLINE_PX_PER_SEC } from './heartbeat-state.js';
import { canvasToVirtual, virtualToCanvas, blockAtVirtualX, flatlineAtVirtualX, blipAtCanvasXY, blipAtVirtualX } from './heartbeat-blips.js';

// Clamp vertical pan so the tallest brick stack can be pulled into view but the
// timeline can't be dragged off into empty space. Bricks stack to ~35% of canvas
// height and grow past 4x zoom (heightScale in the renderer), so the reachable
// range depends on zoom. viewOffsetY shifts the baseline DOWN (positive) to bring
// higher bricks into view; 0 is the resting position (whole EKG framed).
export function clampViewOffsetY(_hb) {
    if (!_hb) return;
    var h = _hb.height || 400;
    var base = (h < 350 ? h * 0.78 : h * 0.55);
    var zoom = _hb.zoom;
    var heightScale = zoom > 4 ? 1 + (zoom - 4) * 0.15 : 1;
    var maxContentTop = (h * 0.35) * heightScale; // tallest possible stack (screen px)
    var topMargin = 8;
    // How far the baseline must travel down to bring the tallest brick's top to
    // topMargin below the canvas top. Zero when the stack already fits.
    var upMax = Math.max(0, (topMargin - base) + maxContentTop);
    var v = _hb.viewOffsetY || 0;
    if (v > upMax) v = upMax;
    if (v < 0) v = 0;
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

// Handle a canvas tap (touch). Two-stage: the first tap on a brick or block spike
// SELECTS it (shows the tooltip, which carries a "tap again" hint on touch); a
// second tap on the same target opens its detail modal. Mirrors the desktop
// hover+click affordance for touch, where there's no hover.
function handleCanvasTap(mx, my) {
    var _hb = getState();
    if (!_hb) return;
    var vx = canvasToVirtual(mx);
    var baseline = (_hb.height < 350 ? _hb.height * 0.78 : _hb.height * 0.55) + (_hb.viewOffsetY || 0);

    // Brick (blip) hit-test above the baseline — mirror the hover zoom thresholds:
    // 2D at high zoom, x-only at moderate zoom, none when too dense/zoomed out.
    var blip = null;
    if (my < baseline) {
        if (_hb.zoom >= 4) blip = blipAtCanvasXY(mx, my, baseline);
        if (!blip && _hb.zoom >= 1.5) blip = blipAtVirtualX(vx, 6 / _hb.zoom);
    }
    if (blip && blip.txid) {
        if (_hb._pinnedBlip === blip || _hb.hoveredBlip === blip) {
            // Second tap on the same brick -> open the tx detail modal.
            if (typeof window.showTxDetail === 'function') {
                window.showTxDetail(blip.txid, {
                    fee: blip.fee, vsize: blip.vsize, value: blip.value, feeRate: blip.feeRate
                });
            }
            _hb._pinnedBlip = null; _hb.hoveredBlip = null;
        } else {
            // First tap -> select + show the tooltip.
            _hb._pinnedBlip = blip; _hb.hoveredBlip = blip; _hb.hoverCanvasX = mx;
            _hb.hoveredBlock = null;
        }
        return;
    }

    // Block spike.
    var tapped = blockAtVirtualX(vx);
    if (tapped) {
        if (_hb.hoveredBlock === tapped) {
            // Second tap on the same spike -> open the block detail modal.
            if (tapped.height > 0 && typeof window.showBlockDetail === 'function') {
                window.showBlockDetail(tapped.height);
            }
            _hb.hoveredBlock = null;
        } else {
            // First tap -> select + show the tooltip.
            _hb.hoveredBlock = tapped;
            _hb._pinnedBlip = null; _hb.hoveredBlip = null;
        }
        return;
    }

    // Tapped empty space -> clear any selection.
    _hb.hoveredBlock = null; _hb._pinnedBlip = null; _hb.hoveredBlip = null;
}

export function setupInputHandlers(canvas) {
    var _hb = getState();
    // Track listeners for cleanup in destroyHeartbeat
    _hb._listeners = [];

    // Mouse wheel: zoom (centered on cursor). Shift+wheel: pan.
    listen(canvas, 'wheel', function(e) {
        if (!_hb) return;
        // End an in-progress block reveal — a wheel-zoom doesn't flip
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

            _hb.autoFollow = false;
            // Don't checkAutoFollow here - zooming should stay where you aimed
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
                _hb.hoveredBlip = null;
                _hb.hoveredFlatline = null;

                if (!_hb.hoveredBlock) {
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
                    // Fallback to x-only match at lower zoom
                    if (!_hb.hoveredBlip && (my < baseline || inTube) && _hb.zoom >= 1.5) {
                        _hb.hoveredBlip = blipAtVirtualX(vx, 4 / _hb.zoom);
                    }
                    // Flatline tooltip only when cursor is at/below baseline (not in brick area above)
                    if (!_hb.hoveredBlip && !inTube && my >= baseline - 5) {
                        _hb.hoveredFlatline = flatlineAtVirtualX(vx);
                    }
                }
                _hb.hoverCanvasX = mx;
                canvas.style.cursor = _hb.hoveredBlock ? 'pointer'
                    : _hb.hoveredBlip ? 'pointer'
                    : _hb.hoveredFlatline ? 'crosshair' : 'default';
            } else {
                _hb.hoveredBlock = null;
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
            }
        }
    });

    listen(canvas, 'click', function(e) {
        if (!_hb) return;
        if (_hb._touchTapped) { _hb._touchTapped = false; return; }
        var rect = canvas.getBoundingClientRect();
        var mx = e.clientX - rect.left;
        var my = e.clientY - rect.top;
        // Click on a brick -> open tx detail modal immediately
        if (_hb.hoveredBlip && _hb.hoveredBlip.txid) {
            if (typeof window.showTxDetail === 'function') {
                window.showTxDetail(_hb.hoveredBlip.txid, {
                    fee: _hb.hoveredBlip.fee,
                    vsize: _hb.hoveredBlip.vsize,
                    value: _hb.hoveredBlip.value,
                    feeRate: _hb.hoveredBlip.feeRate
                });
            }
        } else if (_hb.hoveredBlock && _hb.hoveredBlock.height > 0 && typeof window.showBlockDetail === 'function') {
            // Clicking a block spike -> open block detail modal
            window.showBlockDetail(_hb.hoveredBlock.height);
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
            // Pan start — don't commit to horizontal drag yet
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
            }
        }

        // Tap detection: a single-finger touch that barely moved. Runs regardless of
        // whether it locked into a drag — a real finger tap always has micro-jitter,
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

export function handleControlClick(id) {
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
                // Resume: start scrolling from current position (don't jump to head)
                _hb._playResumeOffset = _hb.viewOffset;
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
        case 'zoomIn':
            var cx = _hb.width / 2;
            var vx = canvasToVirtual(cx);
            _hb.zoom = Math.min(_hb.maxZoom, _hb.zoom * 1.2); // 20%/click (was 40% — too coarse)
            _hb.viewOffset = vx - cx / _hb.zoom;
            clampViewOffsetY(_hb); // keep vertical pan in range as the reachable range changes
            _hb.autoFollow = false;
            break;
        case 'zoomOut':
            var cx2 = _hb.width / 2;
            var vx2 = canvasToVirtual(cx2);
            _hb.zoom = Math.max(_hb.minZoom, _hb.zoom / 1.2);
            _hb.viewOffset = vx2 - cx2 / _hb.zoom;
            clampViewOffsetY(_hb); // zoom-out shrinks the range — avoid stranding the timeline
            break;
        case 'center':
            // Center viewport on the live head and reset vertical pan
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
            _hb.viewOffsetY = 0;
            break;
        case 'fit':
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
            break;
        case 'mode':
            _hb.renderMode = _hb.renderMode === 'bricks' ? 'bloodstream' : 'bricks';
            break;
        case 'live':
            // Re-engage head-follow at the CURRENT zoom (Fit is the separate
            // "frame the whole mempool" action — Live shouldn't fight it by
            // snapping back to a fixed zoom).
            _hb.autoFollow = true;
            _hb.paused = false;
            _hb.viewOffsetY = 0;
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
            _hb.hoveredBlock = null;
            _hb.hoveredBlip = null;
            _hb._pinnedBlip = null;
            break;
    }
}

// Check if user has scrolled close enough to the live head to re-enable auto-follow
export function checkAutoFollow() {
    var _hb = getState();
    if (!_hb) return;
    var headOnCanvas = virtualToCanvas(_hb.virtualX);
    // If the head is visible and near the right edge, snap back to auto-follow
    // but preserve the user's zoom level
    if (headOnCanvas >= _hb.width * 0.7 && headOnCanvas <= _hb.width + 80) {
        _hb.autoFollow = true;
    }
}
