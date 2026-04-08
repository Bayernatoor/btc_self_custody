// heartbeat-blips.js — Hit testing and coordinate conversion for blips and
// timeline segments in the Block Heartbeat engine.

import { getState } from './heartbeat-state.js';
import { cellRadiusForVsize } from './heartbeat-timeline.js';

// ── Hit testing: find block segment under a virtual x position ──
export function blockAtVirtualX(vx) {
    var _hb = getState();
    if (!_hb) return null;
    for (var i = 0; i < _hb.timeline.length; i++) {
        var seg = _hb.timeline[i];
        if (seg.type === 'block' && vx >= seg.x_start && vx <= seg.x_end) {
            return seg;
        }
    }
    return null;
}

// Find flatline segment under a virtual x position, with context
export function flatlineAtVirtualX(vx) {
    var _hb = getState();
    if (!_hb) return null;
    for (var i = 0; i < _hb.timeline.length; i++) {
        var seg = _hb.timeline[i];
        var xEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
        if (seg.type === 'flatline' && vx >= seg.x_start && vx <= xEnd) {
            // Find surrounding blocks for interval display
            var prevBlock = null, nextBlock = null;
            for (var j = i - 1; j >= 0; j--) {
                if (_hb.timeline[j].type === 'block') { prevBlock = _hb.timeline[j]; break; }
            }
            for (var j = i + 1; j < _hb.timeline.length; j++) {
                if (_hb.timeline[j].type === 'block') { nextBlock = _hb.timeline[j]; break; }
            }
            return { segment: seg, prevBlock: prevBlock, nextBlock: nextBlock };
        }
    }
    return null;
}

// Convert canvas pixel x to virtual x (zoom-aware)
export function canvasToVirtual(canvasX) {
    var _hb = getState();
    if (!_hb) return 0;
    return _hb.viewOffset + canvasX / _hb.zoom;
}

// Convert virtual x to canvas pixel x (zoom-aware)
export function virtualToCanvas(vx) {
    var _hb = getState();
    if (!_hb) return 0;
    return (vx - _hb.viewOffset) * _hb.zoom;
}

// Find nearest blip within hitRadius virtual pixels
export function blipAtVirtualX(vx, hitRadius) {
    var _hb = getState();
    if (!_hb) return null;
    var best = null, bestDist = hitRadius;
    var viewLeft = _hb.viewOffset;
    var viewRight = _hb.viewOffset + _hb.width / _hb.zoom;
    for (var i = 0; i < _hb.timeline.length; i++) {
        var seg = _hb.timeline[i];
        if (seg.type !== 'flatline' || !seg.blips) continue;
        // Skip segments outside visible range
        var segEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
        if (segEnd < viewLeft || seg.x_start > viewRight) continue;
        for (var bi = 0; bi < seg.blips.length; bi++) {
            var blip = seg.blips[bi];
            if (blip.fadeStart > 0) continue;
            var dist = Math.abs(blip.x - vx);
            if (dist < bestDist) {
                bestDist = dist;
                best = blip;
            }
        }
    }
    return best;
}

// 2D hit test: find which brick/cell the cursor is inside (canvas coords)
export function blipAtCanvasXY(cx, cy, baseline) {
    var _hb = getState();
    if (!_hb) return null;
    var zoom = _hb.zoom;
    var isBs = _hb.renderMode === 'bloodstream';
    var heightScale = zoom > 4 ? 1 + (zoom - 4) * 0.15 : 1;
    var viewLeft = _hb.viewOffset;
    var viewRight = _hb.viewOffset + _hb.width / zoom;

    for (var i = _hb.timeline.length - 1; i >= 0; i--) {
        var seg = _hb.timeline[i];
        if (seg.type !== 'flatline' || !seg.blips) continue;
        var segEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
        if (segEnd < viewLeft || seg.x_start > viewRight) continue;
        for (var bi = seg.blips.length - 1; bi >= 0; bi--) {
            var blip = seg.blips[bi];
            if (blip.fadeStart > 0) continue;
            var bvx = blip.gridX || blip.x;
            if (bvx < viewLeft || bvx > viewRight) continue;

            var bx = virtualToCanvas(bvx);

            if (isBs) {
                // Circle hit test for bloodstream cells
                var vs = blip.vsize || 200;
                var bR = cellRadiusForVsize(vs);
                var cellR = bR * Math.max(zoom * 0.4, 0.8);
                var tubeH = Math.max(30, cellR * 2.5);
                var bobPhase = blip.bobPhase || 0;
                var colHash = ((blip.gridX || 0) * 0.6180339887) % 1;
                var tubePos = ((bobPhase + colHash * Math.PI * 2) % (Math.PI * 2)) / Math.PI - 1;
                var cellCY = baseline - tubePos * tubeH;
                var dx = cx - bx, dy = cy - cellCY;
                if (dx * dx + dy * dy <= cellR * cellR) {
                    return blip;
                }
            } else {
                // Rectangle hit test for bricks
                var bw = (blip.brickW || 4) * zoom;
                var bh = (blip.brickH || 10) * heightScale;
                var sy = (blip.stackY || 0) * heightScale;
                var left = bx - bw / 2;
                var top = baseline - sy - bh;
                if (cx >= left && cx <= left + bw && cy >= top && cy <= top + bh) {
                    return blip;
                }
            }
        }
    }
    return null;
}
