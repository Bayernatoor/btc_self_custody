// ECharts helpers for the stats page.
// Called from WASM via wasm_bindgen extern functions.
(function() {
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

    // Apply mobile-specific option tweaks (called on init and resize)
    function applyMobileAdjustments(opts) {
        if (window.innerWidth >= 640) return;
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
        // ECharts is loaded — reset retry counter and clear any error message
        el._echartsRetry = 0;
        if (!el._chart) {
            el.textContent = ''; // clear error message if one was shown
            el._chart = echarts.init(el, null, { renderer: 'canvas' });
            el._lastOptionJson = null; // reset dedup on fresh init
            new ResizeObserver(function() { if (el._chart) el._chart.resize(); }).observe(el);
        }
        // Skip if option JSON unchanged (prevents zoom/pan reset)
        if (el._lastOptionJson === optionJson) return;
        el._lastOptionJson = optionJson;
        el._storedOptionJson = optionJson; // keep a copy for resize re-apply
        try {
            var opts = JSON.parse(optionJson);
            opts.animation = false;
            // Custom tooltip: show block height in per-block mode, format values
            if (opts.tooltip && opts.tooltip.trigger === 'axis' && !opts.tooltip.formatter) {
                var yName = '';
                if (opts.yAxis) {
                    var ya = Array.isArray(opts.yAxis) ? opts.yAxis[0] : opts.yAxis;
                    yName = (ya && ya.name) || '';
                }
                var isPct = yName.indexOf('%') !== -1;
                opts.tooltip.formatter = function(params) {
                    if (!params || !params.length) return '';
                    // Header: use ECharts axis label for category axes, parse timestamp for time axes
                    var first = params[0];
                    var header;
                    if (first.axisType === 'xAxis.category') {
                        header = first.axisValueLabel || first.name || '';
                    } else {
                        var ts = first.data && Array.isArray(first.data) ? first.data[0] : first.value;
                        header = new Date(ts).toLocaleString();
                    }
                    var height = first.data && Array.isArray(first.data) && first.data.length >= 3 && typeof first.data[2] === 'number' ? first.data[2] : null;
                    if (height !== null) {
                        header += '&nbsp;&nbsp;<span style="color:#f7931a;font-size:11px">Block #' + height.toLocaleString() + '</span>';
                    }
                    var lines = '<div style="font-size:12px;color:rgba(255,255,255,0.5);margin-bottom:4px">' + header + '</div>';
                    // Series values
                    for (var i = 0; i < params.length; i++) {
                        var p = params[i];
                        var val = p.data && Array.isArray(p.data) ? p.data[1] : p.value;
                        var formatted;
                        if (typeof val !== 'number') {
                            formatted = val;
                        } else if (isPct) {
                            formatted = parseFloat(val.toPrecision(10)).toFixed(2) + '%';
                        } else if (Number.isInteger(val)) {
                            formatted = val.toLocaleString();
                        } else {
                            formatted = parseFloat(val.toPrecision(10)).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
                        }
                        lines += '<div style="display:flex;align-items:center;gap:6px;font-size:12px">';
                        lines += p.marker || '';
                        lines += '<span style="flex:1;color:rgba(255,255,255,0.7)">' + (p.seriesName || '') + '</span>';
                        lines += '<span style="font-weight:600;color:rgba(255,255,255,0.9)">' + formatted + '</span>';
                        lines += '</div>';
                    }
                    return lines;
                };
            }
            // Item tooltips (scatter/pie): show block height + value
            if (opts.tooltip && opts.tooltip.trigger === 'item' && !opts.tooltip.formatter) {
                opts.tooltip.formatter = function(p) {
                    if (!p.data || !Array.isArray(p.data)) return p.name || '';
                    var d = p.data;
                    var height = d.length >= 3 && typeof d[2] === 'number' ? 'Block #' + d[2].toLocaleString() : '';
                    var time = new Date(d[0]).toLocaleString();
                    var val = typeof d[1] === 'number' ? d[1].toFixed(2) : d[1];
                    var header = '<div style="color:rgba(255,255,255,0.5);font-size:12px;margin-bottom:4px">' + time;
                    if (height) header += '&nbsp;&nbsp;<span style="color:#f7931a;font-size:11px">' + height + '</span>';
                    header += '</div>';
                    return header + (p.marker || '') + ' ' + val;
                };
            }
            // Dark background for PNG downloads only (chart renders transparent)
            if (opts.toolbox && opts.toolbox.feature && opts.toolbox.feature.saveAsImage) {
                opts.toolbox.feature.saveAsImage.connectedBackgroundColor = '#0d2137';
            }
            applyMobileAdjustments(opts);
            el._chart.setOption(opts, { notMerge: true, lazyUpdate: true });
            // Click-to-detail: for axis-trigger charts (lines), use zrender click
            // to find the nearest data point under the cursor. For item-trigger
            // charts (scatter/pie), use ECharts native click.
            if (!el._clickRegistered) {
                el._clickRegistered = true;
                if (opts.tooltip && opts.tooltip.trigger === 'axis') {
                    el._chart.getZr().on('click', function(zrParams) {
                        var pointInPixel = [zrParams.offsetX, zrParams.offsetY];
                        if (!el._chart.containPixel('grid', pointInPixel)) return;
                        // Find closest series data point at this x position
                        var xVal = el._chart.convertFromPixel({ seriesIndex: 0 }, pointInPixel)[0];
                        var model = el._chart.getOption();
                        if (!model.series || !model.series.length) return;
                        var data = model.series[0].data;
                        if (!data || !data.length || !Array.isArray(data[0]) || data[0].length < 3) return;
                        // Binary search for nearest timestamp
                        var lo = 0, hi = data.length - 1, best = 0;
                        while (lo <= hi) {
                            var mid = (lo + hi) >> 1;
                            if (data[mid][0] <= xVal) { best = mid; lo = mid + 1; }
                            else { hi = mid - 1; }
                        }
                        if (best < data.length - 1 && Math.abs(data[best+1][0] - xVal) < Math.abs(data[best][0] - xVal)) best++;
                        var height = data[best][2];
                        if (typeof height === 'number') window.showBlockDetail(height);
                    });
                } else {
                    el._chart.on('click', function(params) {
                        if (params.data && Array.isArray(params.data) && params.data.length >= 3 && typeof params.data[2] === 'number') {
                            window.showBlockDetail(params.data[2]);
                        }
                    });
                }
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
        v.textContent = value != null ? String(value) : '\u2014';
        if (cls) v.className = cls;
        row.appendChild(l);
        row.appendChild(v);
        return row;
    }
    function bdCopyRow(label, displayValue, copyValue) {
        var row = bdRow(label, displayValue, 'bd-hash bd-copy');
        var val = row.lastChild;
        val.title = 'Click to copy';
        val.addEventListener('click', function() {
            navigator.clipboard.writeText(copyValue);
            var orig = val.textContent;
            val.textContent = 'Copied!';
            val.classList.add('bd-copied');
            setTimeout(function() {
                val.textContent = orig;
                val.classList.remove('bd-copied');
            }, 1500);
        });
        return row;
    }
    function bdSection(title) {
        var s = document.createElement('div');
        s.className = 'bd-section';
        s.textContent = title;
        return s;
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
                if (!b || b.error) return;
                var modal = document.getElementById('block-detail-modal');
                if (!modal) return;

                var satsToBtc = function(sats) {
                    return (sats / 1e8).toFixed(8) + ' BTC';
                };
                var halvings = Math.floor(b.height / 210000);
                var subsidySats = halvings >= 64 ? 0 : Math.floor(5000000000 / Math.pow(2, halvings));
                var feesBtc = b.total_fees != null ? satsToBtc(b.total_fees) : '\u2014';
                var subsidyBtc = satsToBtc(subsidySats);
                var rewardBtc = b.total_fees != null ? satsToBtc(subsidySats + b.total_fees) : '\u2014';
                var bit4 = (b.version & (1 << 4)) !== 0;
                var bip54 = b.coinbase_locktime === b.height - 1 && b.coinbase_sequence !== 4294967295;
                var versionHex = '0x' + (b.version >>> 0).toString(16).padStart(8, '0');
                var hashVal = b.hash || 'N/A';

                var body = document.getElementById('block-detail-body');
                body.textContent = '';

                // -- Block --
                body.appendChild(bdSection('Block'));
                body.appendChild(bdRow('Time', b.timestamp ? new Date(b.timestamp * 1000).toLocaleString() : '\u2014'));
                body.appendChild(bdCopyRow('Hash', hashVal.substring(0, 16) + '\u2026' + hashVal.slice(-8), hashVal));
                body.appendChild(bdRow('Miner', b.miner || 'Unknown'));
                body.appendChild(bdRow('Size', b.size != null ? (b.size / 1e6).toFixed(2) + ' MB' : '\u2014'));
                body.appendChild(bdRow('Weight', b.weight != null ? b.weight.toLocaleString() + ' WU' : '\u2014'));
                body.appendChild(bdRow('Transactions', b.tx_count != null ? b.tx_count.toLocaleString() : '\u2014'));

                // -- Reward & Fees --
                body.appendChild(bdDivider());
                body.appendChild(bdSection('Reward & Fees'));
                body.appendChild(bdRow('Block Reward', rewardBtc));
                body.appendChild(bdRow('  Subsidy', subsidyBtc));
                body.appendChild(bdRow('  Fees', feesBtc));
                body.appendChild(bdRow('Median Fee', b.median_fee ? b.median_fee.toLocaleString() + ' sats' : '\u2014'));
                body.appendChild(bdRow('Fee Rate', b.median_fee_rate ? b.median_fee_rate.toFixed(1) + ' sat/vB' : '\u2014'));
                body.appendChild(bdRow('Difficulty', b.difficulty != null ? (b.difficulty / 1e12).toFixed(2) + ' T' : '\u2014'));

                // -- Embedded Data --
                var hasEmbedded = (b.op_return_count || 0) > 0 || (b.inscription_count || 0) > 0;
                if (hasEmbedded) {
                    body.appendChild(bdDivider());
                    body.appendChild(bdSection('Embedded Data'));
                    if (b.op_return_count > 0) {
                        body.appendChild(bdRow('OP_RETURN', (b.op_return_count).toLocaleString() + ' outputs'));
                        if (b.runes_count > 0)
                            body.appendChild(bdRow('  Runes', b.runes_count.toLocaleString() + ' (' + (b.runes_bytes || 0).toLocaleString() + ' B)'));
                        if (b.data_carrier_count > 0)
                            body.appendChild(bdRow('  Data Carrier', b.data_carrier_count.toLocaleString() + ' (' + (b.data_carrier_bytes || 0).toLocaleString() + ' B)'));
                    }
                    if (b.inscription_count > 0)
                        body.appendChild(bdRow('Inscriptions', b.inscription_count.toLocaleString() + ' (' + (b.inscription_bytes || 0).toLocaleString() + ' B)'));
                }

                // — Signaling —
                body.appendChild(bdDivider());
                body.appendChild(bdSection('Consensus'));
                body.appendChild(bdCopyRow('Version', versionHex, versionHex));
                body.appendChild(bdRow('BIP-110 (bit 4)', bit4 ? '\u2713 Signaled' : '\u2717 No', bit4 ? '' : ''));
                body.appendChild(bdRow('BIP-54', bip54 ? '\u2713 Compliant' : '\u2717 No', bip54 ? '' : ''));

                // — External link —
                body.appendChild(bdDivider());
                var link = document.createElement('div');
                link.style.cssText = 'text-align:center;padding-top:4px';
                var a = document.createElement('a');
                a.href = 'https://mempool.space/block/' + b.hash;
                a.target = '_blank';
                a.rel = 'noopener noreferrer';
                a.style.cssText = 'color:#f7931a;font-size:12px;opacity:0.7;transition:opacity 0.15s';
                a.textContent = 'View on mempool.space \u2192';
                a.addEventListener('mouseenter', function() { a.style.opacity = '1'; });
                a.addEventListener('mouseleave', function() { a.style.opacity = '0.7'; });
                link.appendChild(a);
                body.appendChild(link);

                var titleEl = document.getElementById('block-detail-title');
                titleEl.textContent = 'Block #' + b.height.toLocaleString();
                titleEl.className = 'text-white font-medium cursor-pointer hover:text-[#f7931a] transition-colors';
                titleEl.title = 'Click to copy height';
                titleEl.onclick = function() {
                    navigator.clipboard.writeText(String(b.height));
                    var orig = titleEl.textContent;
                    titleEl.textContent = 'Copied!';
                    titleEl.style.color = '#22c55e';
                    setTimeout(function() {
                        titleEl.textContent = orig;
                        titleEl.style.color = '';
                    }, 1500);
                };
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
                if (!el._chart) return;
                el._chart.resize();
                // Re-apply full option with mobile adjustments if we have stored JSON
                if (el._storedOptionJson) {
                    el._lastOptionJson = null; // force re-apply
                    window.setChartOption(el.id, el._storedOptionJson);
                }
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
