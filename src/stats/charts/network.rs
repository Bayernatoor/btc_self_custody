use serde_json::json;
use super::*;

/// Block size line chart with moving average.
pub fn block_size_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Block Size");
    }

    let raw_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), round(b.size as f64 / 1_000_000.0, 3)]))
        .collect();
    let vals: Vec<f64> =
        blocks.iter().map(|b| round(b.size as f64 / 1_000_000.0, 3)).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| json!([ts_ms(b.timestamp), m.map(|v| json!(v)).unwrap_or(json!(null))]))
        .collect();

    let x_axis = x_axis_for(false, &[]);
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Size", "type": "line", "sampling": "lttb", "data": raw_data,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis,
        "yAxis": y_axis("MB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block size chart from daily aggregates.
pub fn block_size_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Block Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let sizes: Vec<f64> =
        days.iter().map(|d| round(d.avg_size / 1_000_000.0, 3)).collect();
    let ma = moving_average(&sizes, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("MB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Size", "type": "line", "sampling": "lttb", "data": sizes,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Transaction count line chart with moving average.
pub fn tx_count_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Transaction Count");
    }

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.tx_count]))
        .collect();
    let vals: Vec<f64> = blocks.iter().map(|b| b.tx_count as f64).collect();
    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| json!([ts_ms(b.timestamp), m.map(|v| json!(v)).unwrap_or(json!(null))]))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Tx Count", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Transaction count from daily aggregates.
pub fn tx_count_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Transaction Count");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| round(d.avg_tx_count, 1)).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(round(*x, 1)),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Tx Count", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Difficulty line chart.
pub fn difficulty_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Difficulty");
    }

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.difficulty / 1e12]))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("T"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Difficulty", "type": "line", "sampling": "lttb", "data": raw,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Difficulty from daily aggregates.
pub fn difficulty_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Difficulty");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| d.avg_difficulty / 1e12).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("T"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Difficulty", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Block interval dot plot (per-block only, not daily).
/// Data format: [timestamp_ms, interval_minutes, block_height] — third value enables click-to-detail.
pub fn block_interval_chart(blocks: &[BlockSummary]) -> String {
    if blocks.len() < 2 {
        return no_data_chart("Block Interval");
    }

    let mut dots: Vec<serde_json::Value> = Vec::with_capacity(blocks.len() - 1);
    let mut interval_vals: Vec<f64> = Vec::with_capacity(blocks.len() - 1);

    for i in 1..blocks.len() {
        let mins = ((blocks[i].timestamp as f64
            - blocks[i - 1].timestamp as f64)
            / 60.0
            * 100.0)
            .round()
            / 100.0;
        dots.push(json!([ts_ms(blocks[i].timestamp), mins, blocks[i].height]));
        interval_vals.push(mins);
    }

    let ma = moving_average(&interval_vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks[1..]
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| json!([ts_ms(b.timestamp), m.map(|v| json!(v)).unwrap_or(json!(null))]))
        .collect();

    let has_ma = show_ma(interval_vals.len());

    let mut series = vec![json!({
        "name": "Interval", "type": "scatter", "data": dots,
        "symbolSize": 5,
        "itemStyle": {
            "color": DATA_COLOR
        }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }
    series.push(json!({
        "name": "Target", "type": "line",
        "markLine": {
            "silent": true, "symbol": "none",
            "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 1 },
            "data": [{ "yAxis": 10, "label": { "formatter": "10 min", "color": TARGET_COLOR } }]
        },
        "data": []
    }));

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("min"),
        "dataZoom": data_zoom(),
        "tooltip": { "trigger": "item" },
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block interval from daily aggregates (avg minutes per block = 1440 / blocks_per_day).
pub fn block_interval_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Block Interval (daily)");
    }

    // Filter out partial days (< 50 blocks = likely start/end of range, not a full day)
    let full_days: Vec<&DailyAggregate> =
        days.iter().filter(|d| d.block_count >= 50).collect();
    if full_days.is_empty() {
        return no_data_chart("Block Interval (daily)");
    }
    let dates: Vec<String> = full_days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = full_days
        .iter()
        .map(|d| ((1440.0 / d.block_count as f64) * 100.0).round() / 100.0)
        .collect();
    let ma = moving_average(&vals, 7);

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("min"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Interval", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            },
            {
                "name": "Target", "type": "line",
                "markLine": {
                    "silent": true, "symbol": "none",
                    "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 1 },
                    "data": [{ "yAxis": 10, "label": { "formatter": "10 min", "color": TARGET_COLOR } }]
                },
                "data": []
            }
        ]
    }))
}

/// Block weight utilization as % of max (4,000,000 WU).
pub fn weight_utilization_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Weight Utilization");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            (b.weight as f64 / 4_000_000.0 * 100.0 * 1000.0).round() / 1000.0
        })
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| json!([ts_ms(b.timestamp), m.map(|v| json!(v)).unwrap_or(json!(null))]))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Utilization %", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block weight utilization from daily aggregates.
pub fn weight_utilization_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Weight Utilization");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| (d.avg_weight / 4_000_000.0 * 100.0 * 1000.0).round() / 1000.0)
        .collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Utilization %", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Average transaction size in bytes.
pub fn avg_tx_size_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Avg Transaction Size");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 0 {
                (b.size as f64 / b.tx_count as f64 * 1000.0).round() / 1000.0
            } else {
                0.0
            }
        })
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| json!([ts_ms(b.timestamp), m.map(|v| json!(v)).unwrap_or(json!(null))]))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Avg Tx Size", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Avg transaction size from daily aggregates.
pub fn avg_tx_size_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Avg Transaction Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 0.0 {
                (d.avg_size / d.avg_tx_count * 1000.0).round() / 1000.0
            } else {
                0.0
            }
        })
        .collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Tx Size", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Cumulative chain size over time (per-block).
/// `disk_size_gb` is the current size_on_disk from getblockchaininfo.
pub fn chain_size_chart(blocks: &[BlockSummary], disk_size_gb: f64) -> String {
    if blocks.is_empty() {
        return no_data_chart("Chain Size");
    }

    let mut cumulative: f64 = 0.0;
    let block_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            cumulative += b.size as f64 / 1_000_000_000.0;
            json!([ts_ms(b.timestamp), (cumulative * 1000.0).round() / 1000.0])
        })
        .collect();

    // Only show disk size estimate when we have enough history for the ratio to be meaningful.
    // On short ranges the cumulative starts at 0 (not the true chain total), making the ratio
    // wildly wrong.  Heuristic: need at least 100 GB of block data in the window.
    let block_total = cumulative;
    let show_disk = block_total >= 20.0 && disk_size_gb > 0.0;

    let mut series = vec![json!({
        "name": "Block Data", "type": "line", "sampling": "lttb", "data": block_data,
        "areaStyle": { "color": DATA_COLOR, "opacity": 0.1 },
        "lineStyle": { "width": 2, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
    })];

    if show_disk {
        let ratio = disk_size_gb / block_total;
        let mut cumulative2: f64 = 0.0;
        let disk_data: Vec<serde_json::Value> = blocks
            .iter()
            .map(|b| {
                cumulative2 += b.size as f64 / 1_000_000_000.0;
                let estimated = cumulative2 * ratio;
                json!([ts_ms(b.timestamp), (estimated * 1000.0).round() / 1000.0])
            })
            .collect();
        series.push(json!({
            "name": "Disk Size (est.)", "type": "line", "sampling": "lttb", "data": disk_data,
            "lineStyle": { "width": 1.5, "color": DISK_COLOR, "type": "dashed" },
            "itemStyle": { "color": DISK_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("GB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": show_disk },
        "series": series
    }))
}

/// Cumulative chain size over time (daily).
pub fn chain_size_chart_daily(days: &[DailyAggregate], disk_size_gb: f64) -> String {
    if days.is_empty() {
        return no_data_chart("Chain Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let mut cumulative: f64 = 0.0;
    let block_data: Vec<f64> = days
        .iter()
        .map(|d| {
            cumulative += d.avg_size * d.block_count as f64 / 1_000_000_000.0;
            (cumulative * 1000.0).round() / 1000.0
        })
        .collect();

    // Same heuristic as per-block: only show disk size when the window covers enough
    // of the chain for the ratio to be meaningful.
    let block_total = cumulative;
    let show_disk = block_total >= 20.0 && disk_size_gb > 0.0;

    let mut series = vec![json!({
        "name": "Block Data", "type": "line", "sampling": "lttb", "data": block_data,
        "areaStyle": { "color": DATA_COLOR, "opacity": 0.1 },
        "lineStyle": { "width": 2, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
    })];

    if show_disk {
        let ratio = disk_size_gb / block_total;
        let mut cumulative2: f64 = 0.0;
        let disk_data: Vec<f64> = days
            .iter()
            .map(|d| {
                cumulative2 += d.avg_size * d.block_count as f64 / 1_000_000_000.0;
                let estimated = cumulative2 * ratio;
                (estimated * 1000.0).round() / 1000.0
            })
            .collect();
        series.push(json!({
            "name": "Disk Size (est.)", "type": "line", "sampling": "lttb", "data": disk_data,
            "lineStyle": { "width": 1.5, "color": DISK_COLOR, "type": "dashed" },
            "itemStyle": { "color": DISK_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("GB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": show_disk },
        "series": series
    }))
}
