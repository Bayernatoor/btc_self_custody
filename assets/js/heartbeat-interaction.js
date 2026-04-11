// heartbeat-interaction.js — User input handling, momentum scrolling, control buttons
import { getState, HEAD_POSITION_FRAC, FLATLINE_PX_PER_SEC } from './heartbeat-state.js';
import { canvasToVirtual, virtualToCanvas, blockAtVirtualX, flatlineAtVirtualX, blipAtCanvasXY, blipAtVirtualX } from './heartbeat-blips.js';

// Momentum scrolling
export function startMomentum(velocity) {
    var _hb = getState();
    if (!_hb) return;
    _hb._momentumVel = velocity * 15; // scale up for visible effect
    _hb._momentumId = null;

    function tick() {
        if (!_hb || Math.abs(_hb._momentumVel) < 0.5) {
            _hb._momentumId = null;
            checkAutoFollow();
            return;
        }
        _hb.viewOffset -= _hb._momentumVel / _hb.zoom;
        _hb._momentumVel *= 0.92; // friction
        _hb._momentumId = requestAnimationFrame(tick);
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

export function setupInputHandlers(canvas) {
    var _hb = getState();
    // Track listeners for cleanup in destroyHeartbeat
    _hb._listeners = [];

    // Mouse wheel: zoom (centered on cursor). Shift+wheel: pan.
    listen(canvas, 'wheel', function(e) {
        if (!_hb) return;
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

            // Apply zoom
            var zoomDelta = e.deltaY > 0 ? 0.85 : 1.18; // scroll down = zoom out
            _hb.zoom = Math.max(_hb.minZoom, Math.min(_hb.maxZoom, _hb.zoom * zoomDelta));

            // Adjust viewOffset so the virtual point under cursor stays put
            _hb.viewOffset = vxUnderCursor - mx / _hb.zoom;

            _hb.autoFollow = false;
            // Don't checkAutoFollow here - zooming should stay where you aimed
        }
    }, { passive: false });

    // Mouse drag: click and drag to pan
    listen(canvas, 'mousedown', function(e) {
        if (!_hb) return;

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
                    // Flatline tooltip only when cursor is outside the tube zone
                    if (!_hb.hoveredBlip && !inTube) {
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
        var rect = canvas.getBoundingClientRect();
        var mx = e.clientX - rect.left;
        var my = e.clientY - rect.top;
        // Click on a brick -> pin tooltip. If already pinned, unpin (or open mempool)
        if (_hb._pinnedBlip) {
            // If clicking the pinned brick again -> open mempool.space
            if (_hb.hoveredBlip && _hb.hoveredBlip === _hb._pinnedBlip && _hb._pinnedBlip.txid) {
                window.open('https://mempool.space/tx/' + _hb._pinnedBlip.txid, '_blank');
            }
            // Always unpin on any click when pinned
            _hb._pinnedBlip = null;
        } else if (_hb.hoveredBlip) {
            // Pin the hovered brick
            _hb._pinnedBlip = _hb.hoveredBlip;
        } else if (_hb.hoveredBlock && _hb.hoveredBlock.height > 0 && typeof window.showBlockDetail === 'function') {
            // Clicking a block spike -> open block detail modal
            window.showBlockDetail(_hb.hoveredBlock.height);
        }
    });

    // Touch support: 1 finger = pan with momentum, 2 fingers = pinch-to-zoom
    listen(canvas, 'touchstart', function(e) {
        if (!_hb) return;
        stopMomentum();

        if (e.touches.length === 2) {
            // Pinch start
            _hb._pinching = true;
            _hb.isDragging = false;
            _hb._touchLocked = 'pinch';
            _hb._pinchStartDist = touchDistance(e.touches[0], e.touches[1]);
            _hb._pinchStartZoom = _hb.zoom;
            var rect = canvas.getBoundingClientRect();
            _hb._pinchMid = touchMidpoint(e.touches[0], e.touches[1], rect);
            _hb._pinchVx = canvasToVirtual(_hb._pinchMid.x);
        } else if (e.touches.length === 1) {
            // Pan start — don't commit to horizontal drag yet
            _hb._pinching = false;
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
            // Pinch zoom
            var dist = touchDistance(e.touches[0], e.touches[1]);
            var scale = dist / _hb._pinchStartDist;
            var newZoom = Math.max(_hb.minZoom, Math.min(_hb.maxZoom, _hb._pinchStartZoom * scale));
            _hb.zoom = newZoom;
            _hb.viewOffset = _hb._pinchVx - _hb._pinchMid.x / newZoom;
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
        _hb._touchLocked = null; // reset direction lock for next touch

        if (_hb._pinching) {
            _hb._pinching = false;
            checkAutoFollow();
            return;
        }

        if (_hb.isDragging) {
            _hb.isDragging = false;

            // Momentum
            var vel = _hb._dragVelocity || 0;
            if (Math.abs(vel) > 0.1) {
                startMomentum(vel);
            }

            // Tap detection (minimal drag distance = click)
            if (e.changedTouches && e.changedTouches.length > 0) {
                var dx = Math.abs(e.changedTouches[0].clientX - _hb.dragStartX);
                if (dx < 10) {
                    // Treat as tap
                    var rect = canvas.getBoundingClientRect();
                    var mx = e.changedTouches[0].clientX - rect.left;
                    var vx = canvasToVirtual(mx);
                    var tapped = blockAtVirtualX(vx);

                    var my = e.changedTouches[0].clientY - rect.top;

                    if (tapped) {
                        if (_hb.hoveredBlock === tapped) {
                            // Second tap on same spike: open block detail
                            if (tapped.height > 0 && typeof window.showBlockDetail === 'function') {
                                window.showBlockDetail(tapped.height);
                            }
                            _hb.hoveredBlock = null;
                        } else {
                            // First tap: show tooltip
                            _hb.hoveredBlock = tapped;
                        }
                    } else {
                        _hb.hoveredBlock = null;
                    }
                }
            }
        }
    });
}

export function handleControlClick(id) {
    var _hb = getState();
    if (!_hb) return;
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
            _hb.zoom = Math.min(_hb.maxZoom, _hb.zoom * 1.4);
            _hb.viewOffset = vx - cx / _hb.zoom;
            _hb.autoFollow = false;
            break;
        case 'zoomOut':
            var cx2 = _hb.width / 2;
            var vx2 = canvasToVirtual(cx2);
            _hb.zoom = Math.max(_hb.minZoom, _hb.zoom / 1.4);
            _hb.viewOffset = vx2 - cx2 / _hb.zoom;
            break;
        case 'center':
            // Center viewport on the live head without changing zoom
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / _hb.zoom;
            break;
        case 'mode':
            _hb.renderMode = _hb.renderMode === 'bricks' ? 'bloodstream' : 'bricks';
            break;
        case 'live':
            // Jump to live: snap viewport to the live head immediately
            _hb.autoFollow = true;
            _hb.paused = false;
            _hb.zoom = 1.9;
            _hb.viewOffsetY = 0;
            _hb.viewOffset = _hb.virtualX - (_hb.width * HEAD_POSITION_FRAC) / 1.9;
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
