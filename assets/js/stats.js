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
        el._resizeObs = new ResizeObserver(function() { if (el._chart) el._chart.resize(); });
        el._resizeObs.observe(el);
    };

    // Lazy chart rendering: only render charts visible (or within 200px of
    // viewport). When a chart scrolls far away (600px), dispose it to free
    // canvas memory. Re-inits automatically on scroll back via stored JSON.
    var _enterObserver = typeof IntersectionObserver !== 'undefined' ? new IntersectionObserver(function(entries) {
        entries.forEach(function(entry) {
            if (entry.isIntersecting) {
                var el = entry.target;
                el._lazyVisible = true;
                // Render from stored or pending JSON
                var json = el._pendingJson || el._storedOptionJson;
                if (json) {
                    window.setChartOption(el.id, json);
                    el._pendingJson = null;
                }
            }
        });
    }, { rootMargin: '200px' }) : null;

    // Separate observer for exit detection with larger margin.
    // Defer disposal to avoid destroying a chart mid-render cycle.
    var _exitObserver = typeof IntersectionObserver !== 'undefined' ? new IntersectionObserver(function(entries) {
        entries.forEach(function(entry) {
            if (!entry.isIntersecting) {
                var el = entry.target;
                if (el._chart) {
                    el._lazyVisible = false;
                    setTimeout(function() {
                        // Re-check: user may have scrolled back before timeout
                        if (el._lazyVisible || !el._chart) return;
                        if (el._resizeObs) { el._resizeObs.disconnect(); el._resizeObs = null; }
                        el._chart.dispose();
                        el._chart = null;
                        el._lastOptionJson = null;
                    }, 100);
                }
            }
        });
    }, { rootMargin: '600px' }) : null;


    window.setChartOptionLazy = function(elementId, optionJson) {
        var el = document.getElementById(elementId);
        if (!el) {
            // Element doesn't exist yet — retry next frame
            requestAnimationFrame(function() { window.setChartOptionLazy(elementId, optionJson); });
            return;
        }
        // Always store latest JSON for re-init on scroll back
        el._storedOptionJson = optionJson;
        if (el._lazyVisible) {
            // Already visible — render immediately
            window.setChartOption(elementId, optionJson);
        } else {
            // Store pending JSON and start observing
            el._pendingJson = optionJson;
            if (_enterObserver) {
                if (!el._lazyObserved) {
                    el._lazyObserved = true;
                    _enterObserver.observe(el);
                    if (_exitObserver) _exitObserver.observe(el);
                }
            } else {
                // Fallback: no IntersectionObserver support — render immediately
                window.setChartOption(elementId, optionJson);
            }
        }
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
            if (el._resizeObs) el._resizeObs.disconnect();
            el._resizeObs = new ResizeObserver(function() { if (el._chart) el._chart.resize(); });
            el._resizeObs.observe(el);
        }
        // Skip if option JSON unchanged (prevents zoom/pan reset)
        if (el._lastOptionJson === optionJson) return;
        el._lastOptionJson = optionJson;
        el._storedOptionJson = optionJson; // keep a copy for resize re-apply

        try {
            var opts = JSON.parse(optionJson);
            opts.animation = false;
            // Cache series array for tooltip (avoids getOption() deep-copy per hover)
            el._cachedSeries = opts.series || [];
            // Halving era chart: show actual values in tooltip
            if (opts.tooltip && opts.tooltip._useRawValues) {
                delete opts.tooltip._useRawValues;
                opts.tooltip.formatter = function(params) {
                    if (!params || !params.length) return '';
                    var header = '<div style="color:rgba(255,255,255,0.5);font-size:12px;margin-bottom:4px">' + (params[0].axisValueLabel || params[0].name || '') + '</div>';
                    var lines = '';
                    for (var i = 0; i < params.length; i++) {
                        var p = params[i];
                        var raw = p.data && p.data._raw !== undefined ? p.data._raw : p.value;
                        var unit = p.data && p.data._unit ? ' ' + p.data._unit : '';
                        var formatted = typeof raw === 'number' ? (raw < 1 && raw > 0 ? raw.toFixed(4) : raw.toLocaleString(undefined, {maximumFractionDigits: 2})) : raw;
                        lines += '<div style="display:flex;justify-content:space-between;gap:12px;line-height:1.6">';
                        lines += (p.marker || '') + '<span style="flex:1;color:rgba(255,255,255,0.7)">' + (p.seriesName || '') + '</span>';
                        lines += '<span style="font-weight:600;color:rgba(255,255,255,0.9)">' + formatted + unit + '</span>';
                        lines += '</div>';
                    }
                    return header + lines;
                };
            }
            // Custom tooltip: show block height in per-block mode, format values
            if (opts.tooltip && opts.tooltip.trigger === 'axis' && !opts.tooltip.formatter) {
                var yName = '';
                if (opts.yAxis) {
                    var ya = Array.isArray(opts.yAxis) ? opts.yAxis[0] : opts.yAxis;
                    yName = (ya && ya.name) || '';
                }
                var isPct = yName.indexOf('%') !== -1;
                var chartEl = el;
                opts.tooltip.formatter = function(params) {
                    try {
                    if (!params || !params.length) return '';
                    var first = params[0];
                    if (!first) return '';
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

                    // Build a lookup of series in params (ECharts may omit zero-value series)
                    var paramMap = {};
                    for (var i = 0; i < params.length; i++) {
                        if (params[i]) paramMap[params[i].seriesIndex] = params[i];
                    }

                    // Use cached series only when params is missing some series
                    // (stacked charts with zero values). Otherwise use params directly.
                    var cached = chartEl._cachedSeries || [];
                    var useCached = cached.length > 0 && params.length < cached.length;
                    var seriesCount = useCached ? cached.length : params.length;
                    var dataIndex = first.dataIndex;

                    for (var si = 0; si < seriesCount; si++) {
                        var p = paramMap[si];
                        var val, name, marker;
                        if (p) {
                            val = p.data && Array.isArray(p.data) ? p.data[1] : p.value;
                            name = p.seriesName || '';
                            marker = p.marker || '';
                        } else if (useCached) {
                            // Series not in params (zero value) — read from cache
                            var s = cached[si];
                            if (!s || !s.data || !Array.isArray(s.data) || dataIndex >= s.data.length || !s.data[dataIndex]) continue;
                            var d = s.data[dataIndex];
                            val = Array.isArray(d) ? d[1] : d;
                            name = s.name || '';
                            var color = (s.itemStyle && s.itemStyle.color) || '#ccc';
                            marker = '<span style="display:inline-block;margin-right:4px;border-radius:10px;width:10px;height:10px;background-color:' + color + '"></span>';
                        } else {
                            continue;
                        }

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
                        lines += marker;
                        lines += '<span style="flex:1;color:rgba(255,255,255,0.7)">' + name + '</span>';
                        lines += '<span style="font-weight:600;color:rgba(255,255,255,0.9)">' + formatted + '</span>';
                        lines += '</div>';
                    }
                    return lines;
                    } catch(e) { return ''; }
                };
            }
            // Item tooltips (scatter/pie): show block height + value
            if (opts.tooltip && opts.tooltip.trigger === 'item' && !opts.tooltip.formatter) {
                var noTime = opts.tooltip._noTimeFormat;
                delete opts.tooltip._noTimeFormat;
                opts.tooltip.formatter = function(p) {
                    if (!p.data || !Array.isArray(p.data)) return p.name || '';
                    var d = p.data;
                    var height = d.length >= 3 && typeof d[2] === 'number' ? 'Block #' + d[2].toLocaleString() : '';
                    if (noTime) {
                        // Non-time scatter: show x and y values directly
                        var xVal = typeof d[0] === 'number' ? d[0].toFixed(1) : d[0];
                        var yVal = typeof d[1] === 'number' ? d[1].toFixed(1) : d[1];
                        var lines = '';
                        if (height) lines += '<span style="color:#f7931a;font-size:11px">' + height + '</span><br>';
                        lines += (p.marker || '') + ' ' + xVal + '% / ' + yVal + ' sat/vB';
                        return lines;
                    }
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
            // Click-to-detail: use zrender click to find nearest data point.
            // Registered once per chart element. Works for both axis and item tooltips.
            if (!el._clickRegistered) {
                el._clickRegistered = true;
                el._chart.getZr().on('click', function(zrParams) {
                    var pointInPixel = [zrParams.offsetX, zrParams.offsetY];
                    if (!el._chart.containPixel('grid', pointInPixel)) return;
                    var xVal = el._chart.convertFromPixel({ seriesIndex: 0 }, pointInPixel)[0];
                    var model = el._chart.getOption();
                    if (!model.series || !model.series.length) return;
                    // Find the first series with height data (skip MA series which lack it)
                    var data = null;
                    for (var si = 0; si < model.series.length; si++) {
                        var sd = model.series[si].data;
                        if (sd && sd.length && Array.isArray(sd[0]) && sd[0].length >= 3) {
                            data = sd;
                            break;
                        }
                    }
                    if (!data) return;
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
            }
        } catch(e) {
            // Suppress ECharts internal grid/layout errors during init
            // (harmless race in bar chart rendering — chart recovers)
            if (e && e.message && e.message.indexOf('properties of undefined') !== -1) return;
            console.error('Chart error:', e);
        }
    };

    window.disposeChart = function(elementId) {
        var el = document.getElementById(elementId);
        if (!el) return;
        if (el._resizeObs) { el._resizeObs.disconnect(); el._resizeObs = null; }
        if (el._chart) { el._chart.dispose(); el._chart = null; }
        if (_enterObserver) _enterObserver.unobserve(el);
        if (_exitObserver) _exitObserver.unobserve(el);
        el._lazyObserved = false;
        el._lazyVisible = false;
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
                var fmtSize = function(bytes) {
                    if (bytes == null) return '\u2014';
                    if (bytes >= 1000000) return (bytes / 1e6).toFixed(2) + ' MB';
                    if (bytes >= 1000) return (bytes / 1e3).toFixed(1) + ' KB';
                    return bytes + ' B';
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
                var weightUtil = b.weight != null ? ((b.weight / 4000000) * 100).toFixed(1) + '%' : '\u2014';

                var body = document.getElementById('block-detail-body');
                body.textContent = '';

                // -- Block --
                body.appendChild(bdSection('Block'));
                body.appendChild(bdRow('Time', b.timestamp ? new Date(b.timestamp * 1000).toLocaleString() : '\u2014'));
                body.appendChild(bdCopyRow('Hash', hashVal.substring(0, 16) + '\u2026' + hashVal.slice(-8), hashVal));
                body.appendChild(bdRow('Miner', b.miner || 'Unknown'));
                body.appendChild(bdRow('Size', fmtSize(b.size)));
                body.appendChild(bdRow('Weight', b.weight != null ? b.weight.toLocaleString() + ' WU (' + weightUtil + ')' : '\u2014'));
                body.appendChild(bdRow('Transactions', b.tx_count != null ? b.tx_count.toLocaleString() : '\u2014'));
                if (b.input_count > 0)
                    body.appendChild(bdRow('Inputs / Outputs', b.input_count.toLocaleString() + ' / ' + (b.output_count || 0).toLocaleString()));

                // -- Reward & Fees --
                body.appendChild(bdDivider());
                body.appendChild(bdSection('Reward & Fees'));
                body.appendChild(bdRow('Block Reward', rewardBtc));
                body.appendChild(bdRow('  Subsidy', subsidyBtc));
                body.appendChild(bdRow('  Fees', feesBtc));
                body.appendChild(bdRow('Median Fee', b.median_fee ? b.median_fee.toLocaleString() + ' sats' : '\u2014'));
                body.appendChild(bdRow('Fee Rate', b.median_fee_rate ? b.median_fee_rate.toFixed(1) + ' sat/vB' : '\u2014'));
                body.appendChild(bdRow('Difficulty', b.difficulty != null ? (b.difficulty / 1e12).toFixed(2) + ' T' : '\u2014'));

                // -- Adoption --
                var hasAdoption = (b.segwit_spend_count || 0) > 0 || (b.taproot_spend_count || 0) > 0;
                if (hasAdoption) {
                    body.appendChild(bdDivider());
                    body.appendChild(bdSection('Adoption'));
                    if (b.segwit_spend_count > 0)
                        body.appendChild(bdRow('SegWit Txs', b.segwit_spend_count.toLocaleString()));
                    if (b.taproot_spend_count > 0)
                        body.appendChild(bdRow('Taproot Spends', b.taproot_spend_count.toLocaleString()));
                }

                // -- Embedded Data --
                var hasEmbedded = (b.op_return_count || 0) > 0 || (b.inscription_count || 0) > 0;
                if (hasEmbedded) {
                    body.appendChild(bdDivider());
                    body.appendChild(bdSection('Embedded Data'));
                    if (b.op_return_count > 0) {
                        body.appendChild(bdRow('OP_RETURN', (b.op_return_count).toLocaleString() + ' outputs'));
                        if (b.runes_count > 0)
                            body.appendChild(bdRow('  Runes', b.runes_count.toLocaleString() + ' (' + fmtSize(b.runes_bytes || 0) + ')'));
                        if (b.data_carrier_count > 0)
                            body.appendChild(bdRow('  Other OP_RETURN', b.data_carrier_count.toLocaleString() + ' (' + fmtSize(b.data_carrier_bytes || 0) + ')'));
                    }
                    if (b.inscription_count > 0)
                        body.appendChild(bdRow('Inscriptions', b.inscription_count.toLocaleString() + ' (' + fmtSize(b.inscription_bytes || 0) + ')'));
                }

                // -- BIP Signaling (collapsible) --
                body.appendChild(bdDivider());
                var sigToggle = document.createElement('div');
                sigToggle.className = 'bd-section';
                sigToggle.style.cssText = 'cursor:pointer;user-select:none;display:flex;align-items:center;justify-content:space-between';
                var sigLabel = document.createElement('span');
                sigLabel.textContent = 'BIP Signaling';
                var sigArrow = document.createElement('span');
                sigArrow.textContent = '\u25B6';
                sigArrow.style.cssText = 'font-size:10px;color:rgba(255,255,255,0.3);transition:transform 0.2s';
                sigToggle.appendChild(sigLabel);
                sigToggle.appendChild(sigArrow);
                body.appendChild(sigToggle);

                var sigContent = document.createElement('div');
                sigContent.style.cssText = 'display:none;overflow:hidden';
                sigContent.appendChild(bdRow('BIP-110 (bit 4)', bit4 ? '\u2713 Signaled' : '\u2717 No'));
                sigContent.appendChild(bdRow('BIP-54', bip54 ? '\u2713 Compliant' : '\u2717 No'));
                sigContent.appendChild(bdCopyRow('Block Version', versionHex, versionHex));
                body.appendChild(sigContent);

                sigToggle.addEventListener('click', function() {
                    var open = sigContent.style.display !== 'none';
                    sigContent.style.display = open ? 'none' : 'block';
                    sigArrow.style.transform = open ? '' : 'rotate(90deg)';
                });

                // -- External link --
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

    // ── Transaction detail modal ────────────────────────────
    window.showTxDetail = function(txid, localData) {
        var modal = document.getElementById('tx-detail-modal');
        if (!modal) return;

        var body = document.getElementById('tx-detail-body');
        body.textContent = '';

        var fmtSize = function(bytes) {
            if (bytes == null) return '\u2014';
            if (bytes >= 1000000) return (bytes / 1e6).toFixed(2) + ' MB';
            if (bytes >= 1000) return (bytes / 1e3).toFixed(1) + ' KB';
            return bytes + ' B';
        };

        var titleEl = document.getElementById('tx-detail-title');
        var shortTxid = txid.substring(0, 10) + '\u2026' + txid.slice(-8);
        titleEl.textContent = 'Transaction';
        titleEl.className = 'text-white font-medium';

        // Show local data immediately (from brick)
        if (localData) {
            body.appendChild(bdCopyRow('TXID', shortTxid, txid));
            if (localData.fee != null)
                body.appendChild(bdRow('Fee', localData.fee.toLocaleString() + ' sats'));
            if (localData.vsize != null)
                body.appendChild(bdRow('Size', fmtSize(localData.vsize)));
            if (localData.feeRate != null)
                body.appendChild(bdRow('Fee Rate', localData.feeRate.toFixed(1) + ' sat/vB'));
            if (localData.value != null && localData.value > 0)
                body.appendChild(bdRow('Value', (localData.value / 1e8).toFixed(8) + ' BTC'));
        } else {
            body.appendChild(bdCopyRow('TXID', shortTxid, txid));
        }

        // Loading indicator for API fetch
        var loadingRow = bdRow('', 'Loading details...');
        body.appendChild(bdDivider());
        body.appendChild(loadingRow);

        // External link
        body.appendChild(bdDivider());
        var link = document.createElement('div');
        link.style.cssText = 'text-align:center;padding-top:4px';
        var a = document.createElement('a');
        a.href = 'https://mempool.space/tx/' + txid;
        a.target = '_blank';
        a.rel = 'noopener noreferrer';
        a.style.cssText = 'color:#f7931a;font-size:12px;opacity:0.7;transition:opacity 0.15s';
        a.textContent = 'View on mempool.space \u2192';
        a.addEventListener('mouseenter', function() { a.style.opacity = '1'; });
        a.addEventListener('mouseleave', function() { a.style.opacity = '0.7'; });
        link.appendChild(a);
        body.appendChild(link);

        modal.classList.remove('hidden');

        // Fetch full details from mempool.space API
        fetch('https://mempool.space/api/tx/' + txid)
            .then(function(r) { return r.ok ? r.json() : null; })
            .then(function(tx) {
                if (!tx) {
                    loadingRow.lastChild.textContent = 'Details unavailable';
                    return;
                }
                // Remove loading row and rebuild with full data
                if (loadingRow.parentNode) loadingRow.parentNode.removeChild(loadingRow);

                // Find the divider before the link and insert details there
                var dividers = body.querySelectorAll('.bd-divider');
                var insertBefore = dividers.length > 1 ? dividers[dividers.length - 1] : link;

                var details = document.createDocumentFragment();
                details.appendChild(bdSection('Transaction Details'));

                // Inputs & outputs
                if (tx.vin) details.appendChild(bdRow('Inputs', tx.vin.length.toLocaleString()));
                if (tx.vout) details.appendChild(bdRow('Outputs', tx.vout.length.toLocaleString()));

                // Size & weight
                if (tx.size) details.appendChild(bdRow('Size', fmtSize(tx.size)));
                if (tx.weight) details.appendChild(bdRow('Weight', tx.weight.toLocaleString() + ' WU'));

                // Fee (from API, more accurate)
                if (tx.fee != null) {
                    details.appendChild(bdRow('Fee', tx.fee.toLocaleString() + ' sats'));
                    if (tx.weight) {
                        var vsize = Math.ceil(tx.weight / 4);
                        details.appendChild(bdRow('Fee Rate', (tx.fee / vsize).toFixed(1) + ' sat/vB'));
                    }
                }

                // Total output value
                if (tx.vout) {
                    var totalOut = 0;
                    for (var i = 0; i < tx.vout.length; i++) {
                        totalOut += tx.vout[i].value || 0;
                    }
                    if (totalOut > 0) {
                        details.appendChild(bdRow('Total Output', (totalOut / 1e8).toFixed(8) + ' BTC'));
                    }
                }

                // Confirmation status
                if (tx.status) {
                    if (tx.status.confirmed) {
                        details.appendChild(bdRow('Status', 'Confirmed'));
                        if (tx.status.block_height)
                            details.appendChild(bdRow('Block', '#' + tx.status.block_height.toLocaleString()));
                    } else {
                        details.appendChild(bdRow('Status', 'Unconfirmed'));
                    }
                }

                // Features
                var features = [];
                if (tx.vin && tx.vin.some(function(v) { return v.is_coinbase; })) features.push('Coinbase');
                if (tx.vin && tx.vin.some(function(v) { return v.witness && v.witness.length > 0; })) features.push('SegWit');
                if (tx.vin && tx.vin.some(function(v) { return v.prevout && v.prevout.scriptpubkey_type === 'v1_p2tr'; })) features.push('Taproot');
                var isRbf = tx.vin && tx.vin.some(function(v) { return v.sequence != null && v.sequence < 0xFFFFFFFE; });
                if (isRbf) features.push('RBF');
                if (features.length > 0) {
                    details.appendChild(bdRow('Features', features.join(', ')));
                }

                details.appendChild(bdDivider());
                body.insertBefore(details, insertBefore);
            })
            .catch(function() {
                if (loadingRow.parentNode) loadingRow.lastChild.textContent = 'Details unavailable';
            });
    };

    window.closeTxDetail = function() {
        var modal = document.getElementById('tx-detail-modal');
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
            // Close tx detail modal if open
            var txModal = document.getElementById('tx-detail-modal');
            if (txModal && !txModal.classList.contains('hidden')) {
                txModal.classList.add('hidden');
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

    // Scroll to #anchor on page load (for shareable chart links and drawer navigation).
    // Retries up to 5 times since charts render lazily after hydration.
    if (window.location.hash) {
        var _hashScrollAttempts = 0;
        function _tryHashScroll() {
            var id = window.location.hash.substring(1);
            var el = document.getElementById('card-' + id) || document.getElementById(id);
            if (el) {
                var rect = el.getBoundingClientRect();
                var offset = window.scrollY + rect.top - 80;
                window.scrollTo({ top: offset, behavior: 'smooth' });
            } else if (_hashScrollAttempts < 5) {
                _hashScrollAttempts++;
                setTimeout(_tryHashScroll, 1000);
            }
        }
        setTimeout(_tryHashScroll, 800);
    }
    // CSV export: extract data from an ECharts instance and trigger download.
    window.downloadChartCSV = function(chartId, title, range) {
        var el = document.getElementById(chartId);
        if (!el || !el._chart) return;

        var option = el._chart.getOption();
        if (!option || !option.series || !option.xAxis) return;

        // Filter out overlay series (price, chain size use yAxisIndex >= 1)
        var series = option.series.filter(function(s) {
            return !s.yAxisIndex || s.yAxisIndex === 0;
        });
        if (series.length === 0) return;

        var isCategory = option.xAxis[0] && option.xAxis[0].type === 'category';
        var rows = [];

        if (isCategory) {
            // Daily mode: dates from xAxis.data, values from series.data
            var dates = option.xAxis[0].data || [];
            var header = ['date'].concat(series.map(function(s) { return s.name || ''; }));
            rows.push(header.join(','));
            for (var i = 0; i < dates.length; i++) {
                var row = [dates[i]];
                for (var j = 0; j < series.length; j++) {
                    var val = series[j].data ? series[j].data[i] : null;
                    row.push(val != null ? val : '');
                }
                rows.push(row.join(','));
            }
        } else {
            // Per-block mode: data points are [timestamp_ms, value, height]
            var header = ['timestamp', 'height'].concat(series.map(function(s) { return s.name || ''; }));
            rows.push(header.join(','));

            // Use first series for timestamp/height reference
            var ref = series[0].data || [];
            for (var i = 0; i < ref.length; i++) {
                var point = ref[i];
                if (!point || !Array.isArray(point)) continue;
                var ts = new Date(point[0]).toISOString();
                var height = point.length >= 3 ? point[2] : '';
                var row = [ts, height];
                for (var j = 0; j < series.length; j++) {
                    var sp = series[j].data ? series[j].data[i] : null;
                    if (sp != null && Array.isArray(sp) && sp.length >= 2) {
                        row.push(sp[1] != null ? sp[1] : '');
                    } else {
                        row.push('');
                    }
                }
                rows.push(row.join(','));
            }
        }

        var csv = rows.join('\n');
        var blob = new Blob([csv], { type: 'text/csv;charset=utf-8' });
        var url = URL.createObjectURL(blob);
        var a = document.createElement('a');
        a.href = url;
        a.download = title.replace(/[^a-zA-Z0-9]/g, '_') + '_' + range.toUpperCase() + '.csv';
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    };
    // ── Custom tooltip system (data-tip) ──────────────────────────
    // Appends a single tooltip element to <body> so it's never clipped
    // by overflow:hidden ancestors. Shows on hover (desktop) and tap (mobile).
    (function() {
        var tip = document.createElement('div');
        tip.id = 'btc-tooltip';
        document.body.appendChild(tip);

        var isMobile = 'ontouchstart' in window;
        var activeEl = null;

        function show(el) {
            var text = el.getAttribute('data-tip');
            if (!text) return;
            tip.textContent = text;
            tip.classList.add('visible');
            activeEl = el;
            position(el);
        }

        function hide() {
            tip.classList.remove('visible');
            activeEl = null;
        }

        function position(el) {
            if (window.innerWidth <= 640) return; // mobile uses fixed CSS positioning
            var rect = el.getBoundingClientRect();
            var tipW = tip.offsetWidth;
            var left = rect.left + rect.width / 2 - tipW / 2;
            // Clamp to viewport
            if (left < 8) left = 8;
            if (left + tipW > window.innerWidth - 8) left = window.innerWidth - 8 - tipW;
            tip.style.left = left + 'px';
            tip.style.top = (rect.top - tip.offsetHeight - 8) + 'px';
        }

        // Event delegation on document
        // Guard: e.target may be a text node or SVG that lacks .closest()
        function findTip(e) {
            var t = e.target;
            if (!t || !t.closest) return null;
            return t.closest('[data-tip]:not([data-tip=""])');
        }

        document.addEventListener('mouseenter', function(e) {
            var el = findTip(e);
            if (el) show(el);
        }, true);

        document.addEventListener('mouseleave', function(e) {
            var el = findTip(e);
            if (el && el === activeEl) hide();
        }, true);

        // Mobile: tap to show, tap elsewhere to hide
        if (isMobile) {
            document.addEventListener('touchstart', function(e) {
                var el = findTip(e);
                if (el) {
                    if (activeEl === el) { hide(); return; }
                    show(el);
                } else {
                    hide();
                }
            }, { passive: true });
        }

        // Reposition on scroll
        document.addEventListener('scroll', function() {
            if (activeEl) position(activeEl);
        }, { passive: true });
    })();

})();
