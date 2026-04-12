//! Network chart builders: block size, tx count, TPS, difficulty, block interval,
//! weight utilization, avg tx size, chain size growth, and largest transaction.

use super::*;
use serde_json::json;

/// Block size line chart with moving average.
pub fn block_size_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Block Size");
    }

    let raw_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, round(b.size as f64 / 1_000_000.0, 3)))
        .collect();
    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| round(b.size as f64 / 1_000_000.0, 3))
        .collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();

    let x_axis = x_axis_for(false, &[]);
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
        "xAxis": x_axis,
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

    let raw: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.tx_count)).collect();
    let vals: Vec<f64> = blocks.iter().map(|b| b.tx_count as f64).collect();
    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();

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

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(all_vals.iter())
        .map(|(b, v)| dp(b, *v))
        .collect();

    let ma = moving_average(&all_vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();

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

    let raw: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.difficulty / 1e12)).collect();

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
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
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

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            (b.weight as f64 / 4_000_000.0 * 100.0 * 1000.0).round() / 1000.0
        })
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| dp(b, v))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();

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
        .map(|(b, v)| dp(b, v))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();

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

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| b.largest_tx_size as f64 / 1_000.0) // KB
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| dp(b, *v))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();

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
