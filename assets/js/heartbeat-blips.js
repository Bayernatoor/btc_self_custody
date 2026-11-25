// heartbeat-blips.js: Hit testing and coordinate conversion for blips and
// timeline segments in the Block Heartbeat engine.

import { getState } from './heartbeat-state.js';
import { cellRadiusForVsize, brickHeightScale } from './heartbeat-timeline.js';

// Binary-search the first timeline segment whose effective end is >= vx.
// Segments are strictly ordered left-to-right by x_start/x_end.
function findFirstSegIndexAtX(tl, vx, virtualX) {
    var lo = 0, hi = tl.length;
    while (lo < hi) {
        var mid = (lo + hi) >> 1;
        var seg = tl[mid];
        var segEnd = seg.x_end !== null ? seg.x_end : virtualX;
        if (segEnd < vx) lo = mid + 1;
        else hi = mid;
    }
    return lo;
}

// ── Hit testing: find block segment under a virtual x position ──
export function blockAtVirtualX(vx) {
    var _hb = getState();
    if (!_hb || !_hb.timeline || _hb.timeline.length === 0) return null;
    var tl = _hb.timeline;
    var i = findFirstSegIndexAtX(tl, vx, _hb.virtualX);
    for (; i < tl.length; i++) {
        var seg = tl[i];
        if (seg.x_start > vx) break;
        if (seg.type === 'block' && vx >= seg.x_start && vx <= seg.x_end) {
            return seg;
        }
    }
    return null;
}

// Hit testing: find pending_block segment under a virtual x position
export function pendingBlockAtVirtualX(vx) {
    var _hb = getState();
    if (!_hb || !_hb.timeline || _hb.timeline.length === 0) return null;
    var tl = _hb.timeline;
    var i = findFirstSegIndexAtX(tl, vx, _hb.virtualX);
    for (; i < tl.length; i++) {
        var seg = tl[i];
        if (seg.x_start > vx) break;
        if (seg.type === 'pending_block' && vx >= seg.x_start && vx <= seg.x_end) {
            return seg;
        }
    }
    return null;
}

// Find flatline segment under a virtual x position, with context
export function flatlineAtVirtualX(vx) {
    var _hb = getState();
    if (!_hb || !_hb.timeline || _hb.timeline.length === 0) return null;
    var tl = _hb.timeline;
    var i = findFirstSegIndexAtX(tl, vx, _hb.virtualX);
    for (; i < tl.length; i++) {
        var seg = tl[i];
        if (seg.x_start > vx) break;
        var xEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
        if (seg.type === 'flatline' && vx >= seg.x_start && vx <= xEnd) {
            // Find surrounding blocks (or an immediately following pending_block) for interval display
            var prevBlock = null, nextBlock = null, pendingNext = null;
            if (i + 1 < tl.length && tl[i + 1].type === 'pending_block') {
                pendingNext = tl[i + 1];
            }
            for (var j = i - 1; j >= 0; j--) {
                if (tl[j].type === 'block') { prevBlock = tl[j]; break; }
            }
            for (var j = i + 1; j < tl.length; j++) {
                if (tl[j].type === 'block') { nextBlock = tl[j]; break; }
            }
            return { segment: seg, prevBlock: prevBlock, nextBlock: nextBlock, pendingNext: pendingNext };
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
    if (!_hb || !_hb.timeline || _hb.timeline.length === 0) return null;
    var best = null, bestDist = hitRadius;
    var viewLeft = _hb.viewOffset;
    var viewRight = _hb.viewOffset + _hb.width / _hb.zoom;
    var nowSec = Date.now() / 1000;
    var tl = _hb.timeline;
    var startIdx = findFirstSegIndexAtX(tl, viewLeft, _hb.virtualX);
    for (var i = startIdx; i < tl.length; i++) {
        var seg = tl[i];
        if (seg.x_start > viewRight) break;
        if (seg.type !== 'flatline' || !seg.blips) continue;
        var segEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
        if (segEnd < viewLeft) continue;
        for (var bi = 0; bi < seg.blips.length; bi++) {
            var blip = seg.blips[bi];
            if (!blip || blip._removed || blip.fadeStart > 0 || blip.timestamp > nowSec) continue;
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
    if (!_hb || !_hb.timeline || _hb.timeline.length === 0) return null;
    var zoom = _hb.zoom;
    var isBs = _hb.renderMode === 'bloodstream';
    var heightScale = brickHeightScale(zoom);
    var viewLeft = _hb.viewOffset;
    var viewRight = _hb.viewOffset + _hb.width / zoom;
    var nowSec = Date.now() / 1000;
    var tl = _hb.timeline;
    var startIdx = findFirstSegIndexAtX(tl, viewLeft, _hb.virtualX);
    // Binary-search last segment with x_start <= viewRight
    var lo = startIdx, hi = tl.length;
    while (lo < hi) {
        var mid = (lo + hi) >> 1;
        if (tl[mid].x_start <= viewRight) lo = mid + 1;
        else hi = mid;
    }
    var endIdx = lo - 1;

    for (var i = endIdx; i >= startIdx; i--) {
        var seg = tl[i];
        if (seg.type !== 'flatline' || !seg.blips) continue;
        var segEnd = seg.x_end !== null ? seg.x_end : _hb.virtualX;
        if (segEnd < viewLeft || seg.x_start > viewRight) continue;
        for (var bi = seg.blips.length - 1; bi >= 0; bi--) {
            var blip = seg.blips[bi];
            if (!blip || blip._removed || blip.fadeStart > 0 || blip.timestamp > nowSec) continue;
            var bvx = blip.gridX || blip.x;
            if (bvx < viewLeft || bvx > viewRight) continue;

            var bx = virtualToCanvas(bvx);
            // Cheap x-prune: a blip can only be under the cursor if its column is
            // within ~a brick-width of it. `brickW * zoom` is a safe upper bound on
            // both the brick half-width and the bloodstream cell radius, so this
            // never skips a real hit, but it does skip the per-blip rect/circle
            // math for the far-away majority: keeping the test cheap even if the
            // live segment ever holds an extreme number of blips.
            if (Math.abs(bx - cx) > (blip.brickW || 8) * zoom) continue;

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
