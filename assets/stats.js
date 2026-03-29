// ECharts helpers for the stats page.
// Called from WASM via wasm_bindgen extern functions.
(function() {
    function esc(s) {
        var d = document.createElement('div');
        d.textContent = s;
        return d.innerHTML;
    }

    window.scrollChartIntoView = function(elementId) {
        var el = document.getElementById(elementId);
        if (!el) return;
        // Find the parent card (rounded-2xl container)
        var card = el.closest('[class*="rounded-2xl"]') || el;
        // Small delay to let expand transition start
        setTimeout(function() {
            var rect = card.getBoundingClientRect();
            var offset = window.scrollY + rect.top - 70;
            window.scrollTo({ top: offset, behavior: 'smooth' });
        }, 50);
    };

    window.initChart = function(elementId) {
        var el = document.getElementById(elementId);
        if (!el || el._chart) return;
        if (typeof echarts === 'undefined') return;
        el._chart = echarts.init(el, null, { renderer: 'canvas' });
        new ResizeObserver(function() { if (el._chart) el._chart.resize(); }).observe(el);
    };

    window.setChartOption = function(elementId, optionJson) {
        var el = document.getElementById(elementId);
        if (!el) return;
        if (typeof echarts === 'undefined') {
            // ECharts not loaded yet — retry with limit to avoid infinite loop
            if (!el._echartsRetry) el._echartsRetry = 0;
            if (el._echartsRetry >= 25) { // ~5 seconds max
                console.warn('ECharts failed to load for', elementId);
                return;
            }
            el._echartsRetry++;
            setTimeout(function() { window.setChartOption(elementId, optionJson); }, 200);
            return;
        }
        if (!el._chart) {
            el._chart = echarts.init(el, null, { renderer: 'canvas' });
            new ResizeObserver(function() { if (el._chart) el._chart.resize(); }).observe(el);
        }
        // Skip if option JSON unchanged (prevents zoom/pan reset)
        if (el._lastOptionJson === optionJson) return;
        el._lastOptionJson = optionJson;
        try {
            var opts = JSON.parse(optionJson);
            opts.animation = false;
            // Round tooltip values to avoid floating point noise (59.86000000000001 → 59.86)
            if (opts.tooltip && opts.tooltip.trigger === 'axis' && !opts.tooltip.valueFormatter) {
                opts.tooltip.valueFormatter = function(v) {
                    if (typeof v !== 'number') return v;
                    if (Number.isInteger(v)) return v.toLocaleString();
                    return parseFloat(v.toPrecision(10)).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
                };
            }
            // Set background for PNG downloads (saveAsImage)
            if (opts.toolbox && opts.toolbox.feature && opts.toolbox.feature.saveAsImage) {
                opts.toolbox.feature.saveAsImage.backgroundColor = '#0d2137';
            }
            // Mobile adjustments
            if (window.innerWidth < 640) {
                // Hide toolbox — not usable on touch screens
                if (opts.toolbox) opts.toolbox.show = false;
                // Constrain legend within chart area
                if (opts.legend) {
                    opts.legend.left = 55;
                    opts.legend.right = 40;
                }
                // Shrink pie charts for mobile
                if (opts.series && Array.isArray(opts.series)) {
                    opts.series.forEach(function(s) {
                        if (s.type === 'pie') {
                            s.radius = ['35%', '65%'];
                        }
                    });
                }
            }
            // Hide canvas during setOption to prevent flash of blank/partial chart
            el.style.visibility = 'hidden';
            el._chart.setOption(opts, { notMerge: true, lazyUpdate: true });
            // Show after ECharts finishes layout (double rAF for paint completion)
            requestAnimationFrame(function() {
                requestAnimationFrame(function() {
                    el.style.visibility = 'visible';
                });
            });
            // Auto-register click handler for block detail (data format: [ts, value, height])
            if (!el._clickRegistered) {
                el._clickRegistered = true;
                el._chart.on('click', function(params) {
                    if (params.data && Array.isArray(params.data) && params.data.length >= 3 && typeof params.data[2] === 'number') {
                        window.showBlockDetail(params.data[2]);
                    }
                });
            }
        } catch(e) { console.error('Chart error:', e); }
    };

    window.disposeChart = function(elementId) {
        var el = document.getElementById(elementId);
        if (el && el._chart) {
            el._chart.dispose();
            el._chart = null;
        }
    };

    // Register click handler on a chart. When a data point is clicked,
    // calls showBlockDetail with the block height (stored as x-axis value
    // for time-series charts, or extracted from the data point).
    window.registerChartClick = function(elementId) {
        var el = document.getElementById(elementId);
        if (!el || !el._chart || el._clickRegistered) return;
        el._clickRegistered = true;
        el._chart.on('click', function(params) {
            if (params.data && Array.isArray(params.data) && params.data.length >= 3) {
                // [timestamp, value, height] format
                window.showBlockDetail(params.data[2]);
            }
        });
    };

    // Block detail modal
    window.showBlockDetail = function(height) {
        fetch('/api/stats/blocks/' + height)
            .then(function(r) { return r.json(); })
            .then(function(b) {
                if (b.error) return;
                var modal = document.getElementById('block-detail-modal');
                if (!modal) return;

                var feesBtc = (b.total_fees / 1e8).toFixed(6);
                var bit4 = (b.version & (1 << 4)) !== 0;
                var bip54 = b.coinbase_locktime === b.height - 1 && b.coinbase_sequence !== 4294967295;
                var versionHex = '0x' + (b.version >>> 0).toString(16).padStart(8, '0');

                var body = document.getElementById('block-detail-body');
                body.innerHTML =
                    '<div class="bd-row"><span>Time</span><span>' + new Date(b.timestamp * 1000).toLocaleString() + '</span></div>' +
                    '<div class="bd-row"><span>Hash</span><span class="bd-hash" style="cursor:pointer" onclick="navigator.clipboard.writeText(\'' + esc(b.hash) + '\')">' + esc(b.hash.substring(0, 20)) + '...</span></div>' +
                    '<div class="bd-row"><span>Size</span><span>' + (b.size / 1e6).toFixed(2) + ' MB</span></div>' +
                    '<div class="bd-row"><span>Weight</span><span>' + b.weight.toLocaleString() + ' WU</span></div>' +
                    '<div class="bd-row"><span>Tx Count</span><span>' + b.tx_count.toLocaleString() + '</span></div>' +
                    '<div class="bd-row"><span>Difficulty</span><span>' + (b.difficulty / 1e12).toFixed(2) + 'T</span></div>' +
                    '<div class="bd-divider"></div>' +
                    '<div class="bd-row"><span>Total Fees</span><span>' + feesBtc + ' BTC</span></div>' +
                    '<div class="bd-row"><span>Median Fee</span><span>' + (b.median_fee ? b.median_fee.toLocaleString() + ' sats' : '\u2014') + '</span></div>' +
                    '<div class="bd-row"><span>Median Fee Rate</span><span>' + (b.median_fee_rate ? b.median_fee_rate.toFixed(1) + ' sat/vB' : '\u2014') + '</span></div>' +
                    '<div class="bd-row"><span>Miner</span><span>' + esc(b.miner || 'Unknown') + '</span></div>' +
                    '<div class="bd-divider"></div>' +
                    '<div class="bd-row"><span>OP_RETURN</span><span>' + b.op_return_count + ' outputs</span></div>' +
                    '<div class="bd-row"><span>\u00a0\u00a0Runes</span><span>' + b.runes_count + ' (' + b.runes_bytes + ' B)</span></div>' +
                    '<div class="bd-row"><span>\u00a0\u00a0Data Carrier</span><span>' + b.data_carrier_count + ' (' + b.data_carrier_bytes + ' B)</span></div>' +
                    '<div class="bd-divider"></div>' +
                    '<div class="bd-row"><span>Version</span><span class="bd-hash">' + esc(versionHex) + '</span></div>' +
                    '<div class="bd-row"><span>BIP-110 (bit 4)</span><span class="' + (bit4 ? 'text-green-400' : 'text-red-400') + '">' + (bit4 ? 'YES' : 'NO') + '</span></div>' +
                    '<div class="bd-row"><span>BIP-54</span><span class="' + (bip54 ? 'text-green-400' : 'text-red-400') + '">' + (bip54 ? 'YES' : 'NO') + '</span></div>' +
                    '<div class="bd-row"><span>Coinbase Locktime</span><span>' + (b.coinbase_locktime != null ? b.coinbase_locktime.toLocaleString() : '\u2014') + '</span></div>' +
                    '<div class="bd-row"><span>Coinbase Sequence</span><span>' + (b.coinbase_sequence != null ? '0x' + (b.coinbase_sequence >>> 0).toString(16).padStart(8, '0') : '\u2014') + '</span></div>' +
                    '<div style="margin-top:12px;text-align:center"><a href="https://mempool.space/block/' + b.height + '" target="_blank" style="color:#f7931a;font-size:13px">View on mempool.space \u2192</a></div>';

                document.getElementById('block-detail-title').textContent = 'Block #' + b.height.toLocaleString();
                modal.classList.remove('hidden');
            })
            .catch(function(e) { console.error('Block detail error:', e); });
    };

    window.closeBlockDetail = function() {
        var modal = document.getElementById('block-detail-modal');
        if (modal) modal.classList.add('hidden');
    };

    // Global Escape key handler for modals
    document.addEventListener('keydown', function(e) {
        if (e.key === 'Escape') {
            // Close block detail modal if open
            var modal = document.getElementById('block-detail-modal');
            if (modal && !modal.classList.contains('hidden')) {
                modal.classList.add('hidden');
            }
        }
    });
})();
