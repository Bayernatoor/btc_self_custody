//! Network chart builders: block size, tx count, TPS, difficulty, block interval,
//! weight utilization, avg tx size, chain size growth, and largest transaction.

use super::*;
use chrono::Datelike;
use serde_json::json;
use std::fmt::Write;

/// Block size line chart with moving average.
pub fn block_size_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Block Size");
    }

    let size_fn = |b: &BlockSummary| round(b.size as f64 / 1_000_000.0, 3);
    let raw_str = build_data_array_f64(blocks, size_fn);
    let raw_data = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| size_fn(b)).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Size", "type": "line", "data": raw_data,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("MB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block size chart from daily aggregates.
pub fn block_size_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Block Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let sizes: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_size / 1_000_000.0, 3))
        .collect();
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
                "name": "Avg Size", "type": "line", "data": sizes,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Transaction count line chart with moving average.
pub fn tx_count_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Transaction Count");
    }

    let raw_str = build_data_array_i64(blocks, |b| b.tx_count as i64);
    let raw = data_array_value(&raw_str);
    let vals: Vec<f64> = blocks.iter().map(|b| b.tx_count as f64).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_series = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Tx Count", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_series,
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
pub fn tx_count_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Transaction Count");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> =
        days.iter().map(|d| round(d.avg_tx_count, 1)).collect();
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
                "name": "Avg Tx Count", "type": "line", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Average transactions per second (per-block).
/// Computed as tx_count / seconds_since_previous_block.
pub fn tps_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.len() < 2 {
        return no_data_chart("Transactions per Second");
    }

    let vals: Vec<f64> = blocks
        .windows(2)
        .map(|w| {
            let interval = w[1].timestamp.saturating_sub(w[0].timestamp);
            if interval > 0 {
                round(w[1].tx_count as f64 / interval as f64, 2)
            } else {
                0.0
            }
        })
        .collect();

    // First block has no previous — use 0
    let mut all_vals = vec![0.0];
    all_vals.extend_from_slice(&vals);

    let mut raw_buf = String::with_capacity(blocks.len() * 30);
    raw_buf.push('[');
    for (i, (b, v)) in blocks.iter().zip(all_vals.iter()).enumerate() {
        if i > 0 { raw_buf.push(','); }
        let _ = write!(raw_buf, "[{},{},{}]", ts_ms(b.timestamp), v, b.height);
    }
    raw_buf.push(']');
    let raw = data_array_value(&raw_buf);

    let ma = moving_average(&all_vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "TPS", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("tx/s"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Average transactions per second (daily).
/// Computed as (avg_tx_count * block_count) / 86400 seconds.
pub fn tps_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Transactions per Second");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total_tx = d.avg_tx_count * d.block_count as f64;
            round(total_tx / 86_400.0, 2)
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
        "yAxis": y_axis("tx/s"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Avg TPS", "type": "line", "data": vals,
              "lineStyle": { "width": 1, "color": DATA_COLOR },
              "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Difficulty line chart.
pub fn difficulty_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Difficulty");
    }

    let raw_str = build_data_array_f64(blocks, |b| b.difficulty / 1e12);
    let raw = data_array_value(&raw_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("T"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Difficulty", "type": "line", "data": raw,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Difficulty from daily aggregates.
pub fn difficulty_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
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
                "name": "Difficulty", "type": "line", "data": vals,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Block interval dot plot (per-block only, not daily).
/// Data format: [timestamp_ms, interval_minutes, block_height] — third value enables click-to-detail.
pub fn block_interval_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.len() < 2 {
        return no_data_chart("Block Interval");
    }

    let mut interval_vals: Vec<f64> = Vec::with_capacity(blocks.len() - 1);
    let mut dots_buf = String::with_capacity((blocks.len() - 1) * 30);
    dots_buf.push('[');
    for i in 1..blocks.len() {
        let mins = ((blocks[i].timestamp as f64
            - blocks[i - 1].timestamp as f64)
            / 60.0
            * 100.0)
            .round()
            / 100.0;
        if i > 1 { dots_buf.push(','); }
        let _ = write!(dots_buf, "[{},{},{}]", ts_ms(blocks[i].timestamp), mins, blocks[i].height);
        interval_vals.push(mins);
    }
    dots_buf.push(']');
    let dots = data_array_value(&dots_buf);

    let ma = moving_average(&interval_vals, 144);
    let ma_str = build_ma_array(&blocks[1..], &ma);
    let ma_series = data_array_value(&ma_str);

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
            "name": "144-block MA", "type": "line", "data": ma_series,
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
        "tooltip": {
            "trigger": "item",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block interval from daily aggregates (avg minutes per block = 1440 / blocks_per_day).
pub fn block_interval_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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
                "name": "Avg Interval", "type": "line", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "data": ma,
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
pub fn weight_utilization_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Weight Utilization");
    }

    let weight_fn = |b: &BlockSummary| (b.weight as f64 / 4_000_000.0 * 100.0 * 1000.0).round() / 1000.0;
    let raw_str = build_data_array_f64(blocks, weight_fn);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| weight_fn(b)).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_series = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Utilization %", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_series,
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
pub fn weight_utilization_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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
                "name": "Utilization %", "type": "line", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Average transaction size in bytes.
pub fn avg_tx_size_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Avg Transaction Size");
    }

    let avg_fn = |b: &BlockSummary| {
        if b.tx_count > 0 {
            (b.size as f64 / b.tx_count as f64 * 1000.0).round() / 1000.0
        } else {
            0.0
        }
    };
    let raw_str = build_data_array_f64(blocks, avg_fn);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| avg_fn(b)).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_series = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Avg Tx Size", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_series,
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
pub fn avg_tx_size_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
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
                "name": "Avg Tx Size", "type": "line", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Cumulative chain size over time (per-block).
/// `disk_size_gb` is the current size_on_disk from getblockchaininfo.
/// `offset_bytes` is the total block data before the first block in this window.
pub fn chain_size_chart(
    blocks: &[BlockSummary],
    disk_size_gb: f64,
    offset_bytes: u64,
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Chain Size");
    }

    let mut cumulative: f64 = offset_bytes as f64 / 1_000_000_000.0;
    let block_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            cumulative += b.size as f64 / 1_000_000_000.0;
            dp(b, (cumulative * 1000.0).round() / 1000.0)
        })
        .collect();

    // The final cumulative is the total block data size
    let block_total = cumulative;
    let show_disk = block_total >= 20.0 && disk_size_gb > 0.0;

    let mut series = vec![json!({
        "name": "Block Data", "type": "line", "data": block_data,
        "areaStyle": { "color": DATA_COLOR, "opacity": 0.1 },
        "lineStyle": { "width": 2, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
    })];

    if show_disk {
        let ratio = disk_size_gb / block_total;
        let offset_disk = offset_bytes as f64 / 1_000_000_000.0 * ratio;
        let mut cumulative2 = offset_disk;
        let disk_data: Vec<serde_json::Value> = blocks
            .iter()
            .map(|b| {
                cumulative2 += b.size as f64 / 1_000_000_000.0 * ratio;
                dp(b, (cumulative2 * 1000.0).round() / 1000.0)
            })
            .collect();
        series.push(json!({
            "name": "Disk Size (est.)", "type": "line", "data": disk_data,
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
pub fn chain_size_chart_daily(
    days: &[DailyAggregate],
    disk_size_gb: f64,
    offset_bytes: u64,
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Chain Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let mut cumulative: f64 = offset_bytes as f64 / 1_000_000_000.0;
    let block_data: Vec<f64> = days
        .iter()
        .map(|d| {
            cumulative += d.avg_size * d.block_count as f64 / 1_000_000_000.0;
            (cumulative * 1000.0).round() / 1000.0
        })
        .collect();

    let block_total = cumulative;
    let show_disk = block_total >= 20.0 && disk_size_gb > 0.0;

    let mut series = vec![json!({
        "name": "Block Data", "type": "line", "data": block_data,
        "areaStyle": { "color": DATA_COLOR, "opacity": 0.1 },
        "lineStyle": { "width": 2, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
    })];

    if show_disk {
        let ratio = disk_size_gb / block_total;
        let offset_disk = offset_bytes as f64 / 1_000_000_000.0 * ratio;
        let mut cumulative2 = offset_disk;
        let disk_data: Vec<f64> = days
            .iter()
            .map(|d| {
                cumulative2 +=
                    d.avg_size * d.block_count as f64 / 1_000_000_000.0 * ratio;
                (cumulative2 * 1000.0).round() / 1000.0
            })
            .collect();
        series.push(json!({
            "name": "Disk Size (est.)", "type": "line", "data": disk_data,
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

/// Largest transaction size per block (per-block).
/// Requires backfill v9 for historical data.
pub fn largest_tx_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Largest Transaction");
    }

    let has_data = blocks.iter().any(|b| b.largest_tx_size > 0);
    if !has_data {
        return no_data_chart("Largest Transaction");
    }

    let size_fn = |b: &BlockSummary| b.largest_tx_size as f64 / 1_000.0;
    let raw_str = build_data_array_f64(blocks, size_fn);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| size_fn(b)).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Largest Tx", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("KB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Largest transaction size per block (daily — not available).
pub fn largest_tx_chart_daily(_days: &[DailyAggregate]) -> serde_json::Value {
    no_data_chart("Largest Transaction (per-block ranges only)")
}

/// Histogram of block weight utilization in 10% buckets.
pub fn block_fullness_distribution_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Block Fullness Distribution");
    }

    let labels: Vec<&str> = vec![
        "0-10%", "10-20%", "20-30%", "30-40%", "40-50%",
        "50-60%", "60-70%", "70-80%", "80-90%", "90-100%",
    ];
    let mut counts = [0u64; 10];

    for b in blocks {
        let pct = b.weight as f64 / 4_000_000.0 * 100.0;
        let idx = if pct >= 100.0 {
            9
        } else {
            (pct / 10.0) as usize
        };
        counts[idx] += 1;
    }

    let data: Vec<u64> = counts.to_vec();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("Block Count"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "Block Count", "type": "bar", "data": data,
            "itemStyle": { "color": DATA_COLOR }
        }]
    }))
}

/// Histogram of inter-block times in 1-minute buckets (0-60+ min).
pub fn block_time_distribution_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Block Time Distribution");
    }

    // 61 buckets: 0-1, 1-2, ..., 59-60, 60+
    let mut counts = vec![0u64; 61];
    let mut labels: Vec<String> = (0..60)
        .map(|i| format!("{}-{}", i, i + 1))
        .collect();
    labels.push("60+".to_string());

    for i in 1..blocks.len() {
        let interval = blocks[i].timestamp.saturating_sub(blocks[i - 1].timestamp);
        let mins = interval as f64 / 60.0;
        let idx = if mins >= 60.0 {
            60
        } else {
            mins as usize
        };
        counts[idx] += 1;
    }

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("Block Count"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "Block Count", "type": "bar", "data": counts,
            "itemStyle": { "color": DATA_COLOR },
            "markLine": {
                "silent": true, "symbol": "none",
                "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 2 },
                "data": [{ "xAxis": "9-10", "label": { "formatter": "Target", "color": TARGET_COLOR } }]
            }
        }]
    }))
}

/// Block fullness distribution as percentage of total blocks.
pub fn block_fullness_distribution_pct_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Block Fullness Distribution (%)");
    }

    let labels: Vec<&str> = vec![
        "0-10%", "10-20%", "20-30%", "30-40%", "40-50%",
        "50-60%", "60-70%", "70-80%", "80-90%", "90-100%",
    ];
    let mut counts = [0u64; 10];
    for b in blocks {
        let pct = b.weight as f64 / 4_000_000.0 * 100.0;
        let idx = if pct >= 100.0 { 9 } else { (pct / 10.0) as usize };
        counts[idx] += 1;
    }
    let total = blocks.len() as f64;
    let data: Vec<f64> = counts.iter().map(|&c| round(c as f64 / total * 100.0, 2)).collect();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("% of Blocks"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "% of Blocks", "type": "bar", "data": data,
            "itemStyle": { "color": DATA_COLOR }
        }]
    }))
}

/// Block time distribution as percentage of total blocks.
pub fn block_time_distribution_pct_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Block Time Distribution (%)");
    }

    let mut counts = vec![0u64; 61];
    let mut labels: Vec<String> = (0..60).map(|i| format!("{}-{}", i, i + 1)).collect();
    labels.push("60+".to_string());

    for i in 1..blocks.len() {
        let interval = blocks[i].timestamp.saturating_sub(blocks[i - 1].timestamp);
        let mins = interval as f64 / 60.0;
        let idx = if mins >= 60.0 { 60 } else { mins as usize };
        counts[idx] += 1;
    }

    let total = blocks.len().saturating_sub(1) as f64;
    let data: Vec<f64> = counts.iter().map(|&c| {
        if total > 0.0 { round(c as f64 / total * 100.0, 2) } else { 0.0 }
    }).collect();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("% of Blocks"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "% of Blocks", "type": "bar", "data": data,
            "itemStyle": { "color": DATA_COLOR },
            "markLine": {
                "silent": true, "symbol": "none",
                "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 2 },
                "data": [{ "xAxis": "9-10", "label": { "formatter": "Target", "color": TARGET_COLOR } }]
            }
        }]
    }))
}

/// Scatter plot of rapid consecutive blocks (interval < 60 seconds).
pub fn block_propagation_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Rapid Blocks");
    }

    let mut dots: Vec<serde_json::Value> = Vec::new();

    for i in 1..blocks.len() {
        let interval = blocks[i].timestamp.saturating_sub(blocks[i - 1].timestamp);
        if interval < 60 {
            dots.push(dp(&blocks[i], interval));
        }
    }

    if dots.is_empty() {
        return no_data_chart_with_hint("No rapid consecutive blocks found", "No blocks arrived within 60 seconds of each other in this range");
    }

    let count = dots.len();

    build_option(json!({
        "title": {
            "text": format!("Rapid Blocks ({} found)", count),
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 14 },
            "left": "center"
        },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Seconds"),
        "dataZoom": data_zoom(),
        "tooltip": {
            "trigger": "item",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "Interval", "type": "scatter", "data": dots,
            "symbolSize": 5,
            "itemStyle": { "color": TARGET_COLOR }
        }]
    }))
}

/// Block fullness histogram from pre-computed server-side buckets.
/// Works on any range including ALL (data is already aggregated).
pub fn block_fullness_histogram_from_buckets(
    buckets: &[super::HistogramBucket],
) -> serde_json::Value {
    if buckets.is_empty() {
        return no_data_chart("Block Fullness Distribution");
    }
    let labels: Vec<&str> = buckets.iter().map(|b| b.label.as_str()).collect();
    let counts: Vec<u64> = buckets.iter().map(|b| b.count).collect();

    build_option(json!({
        "xAxis": {
            "type": "category", "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("Block Count"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "Block Count", "type": "bar", "data": counts,
            "itemStyle": { "color": DATA_COLOR }
        }]
    }))
}

/// Block fullness histogram from buckets as percentage.
pub fn block_fullness_histogram_from_buckets_pct(
    buckets: &[super::HistogramBucket],
) -> serde_json::Value {
    if buckets.is_empty() {
        return no_data_chart("Block Fullness Distribution (%)");
    }
    let labels: Vec<&str> = buckets.iter().map(|b| b.label.as_str()).collect();
    let total: u64 = buckets.iter().map(|b| b.count).sum();
    let data: Vec<f64> = buckets.iter().map(|b| round(b.count as f64 / total as f64 * 100.0, 2)).collect();

    build_option(json!({
        "xAxis": {
            "type": "category", "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("% of Blocks"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "% of Blocks", "type": "bar", "data": data,
            "itemStyle": { "color": DATA_COLOR }
        }]
    }))
}

/// Block time histogram from pre-computed server-side buckets.
pub fn block_time_histogram_from_buckets(
    buckets: &[super::HistogramBucket],
) -> serde_json::Value {
    if buckets.is_empty() {
        return no_data_chart("Block Time Distribution");
    }
    let labels: Vec<&str> = buckets.iter().map(|b| b.label.as_str()).collect();
    let counts: Vec<u64> = buckets.iter().map(|b| b.count).collect();

    build_option(json!({
        "xAxis": {
            "type": "category", "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("Block Count"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "Block Count", "type": "bar", "data": counts,
            "itemStyle": { "color": DATA_COLOR },
            "markLine": {
                "silent": true, "symbol": "none",
                "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 2 },
                "data": [{ "xAxis": "9-10", "label": { "formatter": "Target", "color": TARGET_COLOR } }]
            }
        }]
    }))
}

/// Difficulty ribbon chart: 7 moving averages of difficulty at different windows.
/// When short MAs cross below long MAs, it signals miner capitulation.
pub fn difficulty_ribbon_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Difficulty Ribbon");
    }
    // Need enough blocks for the longest MA (128) to produce meaningful spread
    if blocks.len() < 500 {
        return no_data_chart_with_hint("Difficulty Ribbon", "Select a longer range (3M+) to see the ribbon spread across difficulty adjustments");
    }

    let windows = [9, 14, 25, 40, 60, 90, 128];
    let colors = [
        "rgba(173,216,255,0.7)",  // lightest blue
        "rgba(135,190,255,0.7)",
        "rgba(100,165,255,0.7)",
        "rgba(70,140,240,0.7)",
        "rgba(45,115,220,0.7)",
        "rgba(25,90,200,0.7)",
        "rgba(10,60,170,0.7)",    // darkest blue
    ];

    let diff_vals: Vec<f64> = blocks.iter().map(|b| b.difficulty / 1e12).collect();

    let mut series = Vec::with_capacity(windows.len());
    for (i, &w) in windows.iter().enumerate() {
        let ma = moving_average(&diff_vals, w);
        let ma_str = build_ma_array(blocks, &ma);
        let ma_data = data_array_value(&ma_str);
        series.push(json!({
            "name": format!("{}-block MA", w),
            "type": "line",
            "data": ma_data,
            "lineStyle": { "width": 1.5, "color": colors[i] },
            "itemStyle": { "color": colors[i] },
            "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Difficulty (T)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": series
    }))
}

/// Difficulty ribbon chart from daily aggregates.
pub fn difficulty_ribbon_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Difficulty Ribbon");
    }

    let windows = [7, 14, 25, 40, 60, 90, 128];
    let colors = [
        "rgba(173,216,255,0.7)",
        "rgba(135,190,255,0.7)",
        "rgba(100,165,255,0.7)",
        "rgba(70,140,240,0.7)",
        "rgba(45,115,220,0.7)",
        "rgba(25,90,200,0.7)",
        "rgba(10,60,170,0.7)",
    ];

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let diff_vals: Vec<f64> = days.iter().map(|d| d.avg_difficulty / 1e12).collect();

    let mut series = Vec::with_capacity(windows.len());
    for (i, &w) in windows.iter().enumerate() {
        let ma = moving_average(&diff_vals, w);
        let ma_vals: Vec<serde_json::Value> = ma
            .iter()
            .map(|v| match v {
                Some(x) => json!(x),
                None => json!(null),
            })
            .collect();
        series.push(json!({
            "name": format!("{}-day MA", w),
            "type": "line",
            "data": ma_vals,
            "lineStyle": { "width": 1.5, "color": colors[i] },
            "itemStyle": { "color": colors[i] },
            "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Difficulty (T)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": series
    }))
}

/// Weekend vs weekday activity: grouped bar chart showing average tx count
/// and average fees (BTC) by day of week.
pub fn weekday_activity_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Weekday Activity");
    }

    // Accumulate per-day-of-week totals
    // (timestamp / 86400 + 4) % 7 gives 0=Mon..6=Sun
    let mut tx_sums = [0.0f64; 7];
    let mut fee_sums = [0.0f64; 7];
    let mut counts = [0u64; 7];

    for b in blocks {
        let dow = ((b.timestamp / 86400 + 4) % 7) as usize;
        tx_sums[dow] += b.tx_count as f64;
        fee_sums[dow] += b.total_fees as f64 / 100_000_000.0;
        counts[dow] += 1;
    }

    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let avg_tx: Vec<f64> = (0..7)
        .map(|i| if counts[i] > 0 { round(tx_sums[i] / counts[i] as f64, 1) } else { 0.0 })
        .collect();
    let avg_fees: Vec<f64> = (0..7)
        .map(|i| if counts[i] > 0 { round(fee_sums[i] / counts[i] as f64, 4) } else { 0.0 })
        .collect();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": day_names,
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": [
            {
                "type": "value", "name": "Avg Tx Count",
                "nameTextStyle": { "color": "#aaa" },
                "axisLabel": { "color": "#aaa" },
                "axisLine": { "lineStyle": { "color": "#555" } },
                "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.10)", "type": "dashed" } }
            },
            {
                "type": "value", "name": "Avg Fees (BTC)",
                "nameTextStyle": { "color": "#aaa" },
                "axisLabel": { "color": "#aaa" },
                "axisLine": { "lineStyle": { "color": "#555" } },
                "splitLine": { "show": false }
            }
        ],
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Avg Tx Count", "type": "bar", "data": avg_tx,
                "yAxisIndex": 0,
                "itemStyle": { "color": DATA_COLOR }
            },
            {
                "name": "Avg Fees (BTC)", "type": "bar", "data": avg_fees,
                "yAxisIndex": 1,
                "itemStyle": { "color": "#3b82f6" }
            }
        ]
    }))
}

/// Weekday activity from daily aggregates. Groups by day-of-week using the date string.
pub fn weekday_activity_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Weekday Activity");
    }

    let mut tx_sums = [0.0f64; 7];
    let mut fee_sums = [0.0f64; 7];
    let mut counts = [0u64; 7];

    for d in days {
        // Parse date to get day of week
        if let Ok(date) = chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d") {
            let dow = date.weekday().num_days_from_monday() as usize;
            tx_sums[dow] += d.avg_tx_count * d.block_count as f64;
            fee_sums[dow] += d.total_fees as f64 / 100_000_000.0;
            counts[dow] += d.block_count;
        }
    }

    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let avg_tx: Vec<f64> = (0..7)
        .map(|i| if counts[i] > 0 { round(tx_sums[i] / counts[i] as f64, 1) } else { 0.0 })
        .collect();
    let avg_fees: Vec<f64> = (0..7)
        .map(|i| if counts[i] > 0 { round(fee_sums[i] / counts[i] as f64, 4) } else { 0.0 })
        .collect();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": day_names,
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": [
            {
                "type": "value", "name": "Avg Tx Count",
                "nameTextStyle": { "color": "#aaa" },
                "axisLabel": { "color": "#aaa" },
                "axisLine": { "lineStyle": { "color": "#555" } },
                "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.10)", "type": "dashed" } }
            },
            {
                "type": "value", "name": "Avg Fees (BTC)",
                "nameTextStyle": { "color": "#aaa" },
                "axisLabel": { "color": "#aaa" },
                "axisLine": { "lineStyle": { "color": "#555" } },
                "splitLine": { "show": false }
            }
        ],
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Avg Tx Count", "type": "bar", "data": avg_tx,
                "yAxisIndex": 0,
                "itemStyle": { "color": DATA_COLOR }
            },
            {
                "name": "Avg Fees (BTC)", "type": "bar", "data": avg_fees,
                "yAxisIndex": 1,
                "itemStyle": { "color": "#3b82f6" }
            }
        ]
    }))
}

/// Block time histogram from buckets as percentage.
pub fn block_time_histogram_from_buckets_pct(
    buckets: &[super::HistogramBucket],
) -> serde_json::Value {
    if buckets.is_empty() {
        return no_data_chart("Block Time Distribution (%)");
    }
    let labels: Vec<&str> = buckets.iter().map(|b| b.label.as_str()).collect();
    let total: u64 = buckets.iter().map(|b| b.count).sum();
    let data: Vec<f64> = buckets.iter().map(|b| round(b.count as f64 / total as f64 * 100.0, 2)).collect();

    build_option(json!({
        "xAxis": {
            "type": "category", "data": labels,
            "axisLabel": { "color": "rgba(255,255,255,0.6)" },
            "axisLine": { "lineStyle": { "color": "rgba(255,255,255,0.15)" } }
        },
        "yAxis": y_axis("% of Blocks"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [{
            "name": "% of Blocks", "type": "bar", "data": data,
            "itemStyle": { "color": DATA_COLOR },
            "markLine": {
                "silent": true, "symbol": "none",
                "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 2 },
                "data": [{ "xAxis": "9-10", "label": { "formatter": "Target", "color": TARGET_COLOR } }]
            }
        }]
    }))
}
