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

    window.setChartOption = function(elementId, optionJson, isRetry) {
        var el = document.getElementById(elementId);
        if (!el) {
            // Element doesn't exist yet — one-frame retry for reactive closure timing
            if (!isRetry) {
                requestAnimationFrame(function() { window.setChartOption(elementId, optionJson, true); });
            }
            return;
        }
        if (typeof echarts === 'undefined') {
            // ECharts not loaded yet — retry with limit to avoid infinite loop
            if (!el._echartsRetry) el._echartsRetry = 0;
            if (el._echartsRetry >= 25) { // ~5 seconds max
                console.warn('ECharts failed to load for', elementId);
                el.textContent = '';
                var msg = document.createElement('div');
                msg.style.cssText = 'display:flex;align-items:center;justify-content:center;height:100%;color:rgba(255,255,255,0.3);font-size:13px';
                msg.textContent = 'Charts failed to load. Please refresh the page.';
                el.appendChild(msg);
                return;
            }
            el._echartsRetry++;
            setTimeout(function() { window.setChartOption(elementId, optionJson); }, 200);
            return;
        }
        if (!el._chart) {
            el._chart = echarts.init(el, null, { renderer: 'canvas' });
            el._lastOptionJson = null; // reset dedup on fresh init
            new ResizeObserver(function() { if (el._chart) el._chart.resize(); }).observe(el);
        }
        // Skip if option JSON unchanged (prevents zoom/pan reset)
        if (el._lastOptionJson === optionJson) return;
        el._lastOptionJson = optionJson;
        try {
            var opts = JSON.parse(optionJson);
            opts.animation = false;
            // Round tooltip values and add % suffix for percentage charts
            if (opts.tooltip && opts.tooltip.trigger === 'axis' && !opts.tooltip.valueFormatter) {
                var yName = '';
                if (opts.yAxis) {
                    var ya = Array.isArray(opts.yAxis) ? opts.yAxis[0] : opts.yAxis;
                    yName = (ya && ya.name) || '';
                }
                var isPct = yName.indexOf('%') !== -1;
                opts.tooltip.valueFormatter = function(v) {
                    if (typeof v !== 'number') return v;
                    if (isPct) return parseFloat(v.toPrecision(10)).toFixed(2) + '%';
                    if (Number.isInteger(v)) return v.toLocaleString();
                    return parseFloat(v.toPrecision(10)).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
                };
            }
            // Dark background for PNG downloads only (chart renders transparent)
            if (opts.toolbox && opts.toolbox.feature && opts.toolbox.feature.saveAsImage) {
                opts.toolbox.feature.saveAsImage.connectedBackgroundColor = '#0d2137';
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
            el._chart.setOption(opts, { notMerge: true, lazyUpdate: true });
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

    // Block detail modal — uses textContent (XSS-safe) instead of innerHTML
    function bdRow(label, value, cls) {
        var row = document.createElement('div');
        row.className = 'bd-row';
        var l = document.createElement('span');
        l.textContent = label;
        var v = document.createElement('span');
        v.textContent = value;
        if (cls) v.className = cls;
        row.appendChild(l);
        row.appendChild(v);
        return row;
    }
    function bdDivider() {
        var d = document.createElement('div');
        d.className = 'bd-divider';
        return d;
    }

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
                body.textContent = ''; // clear safely

                body.appendChild(bdRow('Time', new Date(b.timestamp * 1000).toLocaleString()));

                var hashRow = bdRow('Hash', b.hash.substring(0, 20) + '...', 'bd-hash');
                hashRow.lastChild.style.cursor = 'pointer';
                hashRow.lastChild.addEventListener('click', function() { navigator.clipboard.writeText(b.hash); });
                body.appendChild(hashRow);

                body.appendChild(bdRow('Size', (b.size / 1e6).toFixed(2) + ' MB'));
                body.appendChild(bdRow('Weight', b.weight.toLocaleString() + ' WU'));
                body.appendChild(bdRow('Tx Count', b.tx_count.toLocaleString()));
                body.appendChild(bdRow('Difficulty', (b.difficulty / 1e12).toFixed(2) + 'T'));
                body.appendChild(bdDivider());
                body.appendChild(bdRow('Total Fees', feesBtc + ' BTC'));
                body.appendChild(bdRow('Median Fee', b.median_fee ? b.median_fee.toLocaleString() + ' sats' : '\u2014'));
                body.appendChild(bdRow('Median Fee Rate', b.median_fee_rate ? b.median_fee_rate.toFixed(1) + ' sat/vB' : '\u2014'));
                body.appendChild(bdRow('Miner', b.miner || 'Unknown'));
                body.appendChild(bdDivider());
                body.appendChild(bdRow('OP_RETURN', b.op_return_count + ' outputs'));
                body.appendChild(bdRow('\u00a0\u00a0Runes', b.runes_count + ' (' + b.runes_bytes + ' B)'));
                body.appendChild(bdRow('\u00a0\u00a0Data Carrier', b.data_carrier_count + ' (' + b.data_carrier_bytes + ' B)'));
                body.appendChild(bdDivider());
                body.appendChild(bdRow('Version', versionHex, 'bd-hash'));
                body.appendChild(bdRow('BIP-110 (bit 4)', bit4 ? 'YES' : 'NO', bit4 ? 'text-green-400' : 'text-red-400'));
                body.appendChild(bdRow('BIP-54', bip54 ? 'YES' : 'NO', bip54 ? 'text-green-400' : 'text-red-400'));
                body.appendChild(bdRow('Coinbase Locktime', b.coinbase_locktime != null ? b.coinbase_locktime.toLocaleString() : '\u2014'));
                body.appendChild(bdRow('Coinbase Sequence', b.coinbase_sequence != null ? '0x' + (b.coinbase_sequence >>> 0).toString(16).padStart(8, '0') : '\u2014'));

                var link = document.createElement('div');
                link.style.cssText = 'margin-top:12px;text-align:center';
                var a = document.createElement('a');
                a.href = 'https://mempool.space/block/' + b.height;
                a.target = '_blank';
                a.style.cssText = 'color:#f7931a;font-size:13px';
                a.textContent = 'View on mempool.space \u2192';
                link.appendChild(a);
                body.appendChild(link);

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

    // Re-apply mobile adjustments on device rotation / resize
    var _resizeTimer;
    window.addEventListener('resize', function() {
        clearTimeout(_resizeTimer);
        _resizeTimer = setTimeout(function() {
            var charts = document.querySelectorAll('[id^="chart-"], [id="mempool-gauge"]');
            charts.forEach(function(el) {
                if (!el._chart || !el._lastOptionJson) return;
                el._lastOptionJson = null; // force re-apply on next setOption
                // Re-trigger with stored option
                try {
                    el._chart.resize();
                } catch(e) { /* ignore */ }
            });
        }, 250);
    });

    // Scroll to #anchor on page load (for shareable chart links)
    // Delayed to allow charts to render first
    if (window.location.hash) {
        setTimeout(function() {
            var id = window.location.hash.substring(1);
            // Chart anchors use card-{id} prefix to avoid duplicate IDs with ECharts
            var el = document.getElementById('card-' + id) || document.getElementById(id);
            if (el) {
                var rect = el.getBoundingClientRect();
                var offset = window.scrollY + rect.top - 80;
                window.scrollTo({ top: offset, behavior: 'smooth' });
            }
        }, 1500);
    }
})();
