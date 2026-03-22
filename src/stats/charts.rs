//! ECharts option JSON builders.
//! Runs on the client (WASM) — takes typed data and produces JSON strings
//! that are passed to ECharts via JS interop.

use serde_json::json;

use super::types::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Consistent chart color palette
const DATA_COLOR: &str = "#4ecdc4"; // Primary data (teal)
const DATA_COLOR_FADED: &str = "rgba(78,205,196,0.3)"; // Primary data area fill
const MA_COLOR: &str = "rgba(255,255,255,0.85)"; // Moving average (white)
const TARGET_COLOR: &str = "#e74c3c"; // Target/reference lines (red)
const RUNES_COLOR: &str = "#ff6b6b"; // Runes (coral red)
const CARRIER_COLOR: &str = "#bb8fff"; // Data carriers (purple)
const SIGNAL_YES: &str = "#2ecc71"; // Signaled (green)
#[allow(dead_code)]
const SIGNAL_NO: &str = "rgba(231,76,60,0.3)"; // Not signaled (faded red)

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn chart_defaults() -> serde_json::Value {
    json!({
        "backgroundColor": "transparent",
        "textStyle": { "color": "#aaa", "fontFamily": "Inter, system-ui, sans-serif" },
        "grid": { "left": 55, "right": 20, "top": 50, "bottom": 65 },
        "legend": { "textStyle": { "color": "#ccc", "fontSize": 11 }, "top": 28, "left": "center" },
        "toolbox": {
            "feature": {
                "restore": { "title": "Reset zoom" },
                "dataZoom": { "title": { "zoom": "Zoom", "back": "Undo zoom" } },
                "saveAsImage": { "title": "Save" }
            },
            "iconStyle": { "borderColor": "#aaa" },
            "emphasis": { "iconStyle": { "borderColor": "#f7931a" } },
            "right": 10, "top": 0
        }
    })
}

fn data_zoom() -> serde_json::Value {
    json!([
        { "type": "inside", "start": 0, "end": 100 },
        {
            "type": "slider", "start": 0, "end": 100, "height": 20, "bottom": 8,
            "borderColor": "#333", "fillerColor": "rgba(247,147,26,0.15)",
            "handleStyle": { "color": "#f7931a" }, "textStyle": { "color": "#aaa", "fontSize": 10 }
        }
    ])
}

fn tooltip_axis() -> serde_json::Value {
    json!({ "trigger": "axis" })
}

fn x_axis_for(is_daily: bool, categories: &[String]) -> serde_json::Value {
    if is_daily {
        json!({
            "type": "category",
            "data": categories,
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } }
        })
    } else {
        json!({
            "type": "time",
            "axisLabel": { "color": "#aaa", "hideOverlap": true },
            "axisLine": { "lineStyle": { "color": "#555" } }
        })
    }
}

fn y_axis(name: &str) -> serde_json::Value {
    json!({
        "type": "value",
        "name": name,
        "nameTextStyle": { "color": "#aaa" },
        "axisLabel": { "color": "#aaa" },
        "axisLine": { "lineStyle": { "color": "#555" } },
        "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
    })
}

fn no_data_chart(title: &str) -> String {
    let mut opt = chart_defaults();
    let m = opt.as_object_mut().unwrap();
    m.insert(
        "title".into(),
        json!({
            "text": format!("{} — No data", title),
            "textStyle": { "color": "#aaa", "fontSize": 14 },
            "left": "center", "top": "middle"
        }),
    );
    serde_json::to_string(&opt).unwrap_or_default()
}

fn moving_average(data: &[f64], window: usize) -> Vec<Option<f64>> {
    let mut result = Vec::with_capacity(data.len());
    for i in 0..data.len() {
        if i < window.saturating_sub(1) {
            result.push(None);
        } else {
            let start = i + 1 - window;
            let sum: f64 = data[start..=i].iter().sum();
            let avg = sum / window as f64;
            result.push(Some((avg * 1000.0).round() / 1000.0));
        }
    }
    result
}

fn ts_ms(unix_secs: u64) -> u64 {
    unix_secs * 1000
}

/// Merge chart_defaults with additional fields.
fn build_option(extra: serde_json::Value) -> String {
    let mut base = chart_defaults();
    if let (Some(base_obj), Some(extra_obj)) =
        (base.as_object_mut(), extra.as_object())
    {
        for (k, v) in extra_obj {
            base_obj.insert(k.clone(), v.clone());
        }
    }
    serde_json::to_string(&base).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Chart builders
// ---------------------------------------------------------------------------

/// Block size line chart with moving average.
pub fn block_size_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Block Size");
    }

    let raw_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.size as f64 / 1_000_000.0]))
        .collect();
    let vals: Vec<f64> =
        blocks.iter().map(|b| b.size as f64 / 1_000_000.0).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let x_axis = x_axis_for(false, &[]);

    build_option(json!({
        "title": { "text": "Block Size", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis,
        "yAxis": y_axis("MB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Size", "type": "line", "data": raw_data,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_data,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Block size chart from daily aggregates.
pub fn block_size_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Block Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let sizes: Vec<f64> =
        days.iter().map(|d| d.avg_size / 1_000_000.0).collect();
    let ma = moving_average(&sizes, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "title": { "text": "Block Size (Daily Avg)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
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
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    build_option(json!({
        "title": { "text": "Transaction Count", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Tx Count", "type": "line", "data": raw,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_series,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Transaction count from daily aggregates.
pub fn tx_count_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Transaction Count");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| d.avg_tx_count).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "title": { "text": "Transaction Count (Daily Avg)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
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

/// Fees line chart (per-block: total fees in sats).
pub fn fees_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Fees");
    }

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            if b.total_fees > 0 {
                json!([ts_ms(b.timestamp), b.total_fees])
            } else {
                json!([ts_ms(b.timestamp), null])
            }
        })
        .collect();

    build_option(json!({
        "title": { "text": "Total Fees per Block (sats)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("sats"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Fees", "type": "line", "data": raw,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Fees from daily aggregates.
pub fn fees_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Fees");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.total_fees > 0 && d.block_count > 0 {
                json!(d.total_fees as f64 / d.block_count as f64)
            } else {
                json!(null)
            }
        })
        .collect();

    build_option(json!({
        "title": { "text": "Avg Fees per Block (sats/day)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("sats"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Fees", "type": "line", "data": vals,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
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
        "title": { "text": "Difficulty", "textStyle": { "color": "#ccc", "fontSize": 14 } },
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
pub fn difficulty_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Difficulty");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| d.avg_difficulty / 1e12).collect();

    build_option(json!({
        "title": { "text": "Difficulty (Daily Avg)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
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
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    build_option(json!({
        "title": { "text": "Block Interval", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("min"),
        "dataZoom": data_zoom(),
        "tooltip": { "trigger": "item" },
        "series": [
            {
                "name": "Interval", "type": "scatter", "data": dots,
                "symbolSize": 5,
                "itemStyle": {
                    "color": DATA_COLOR
                }
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_series,
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
        "title": { "text": "Avg Block Interval (daily)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
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

/// OP_RETURN count bar chart (runes vs data carriers).
pub fn op_return_count_chart(blocks: &[OpReturnBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("OP_RETURN Count");
    }

    let runes: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.runes_count]))
        .collect();
    let carriers: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.data_carrier_count]))
        .collect();

    build_option(json!({
        "title": { "text": "OP_RETURN Count by Type", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Runes", "type": "bar", "stack": "total", "data": runes,
                "itemStyle": { "color": RUNES_COLOR }
            },
            {
                "name": "Data Carriers", "type": "bar", "stack": "total", "data": carriers,
                "itemStyle": { "color": CARRIER_COLOR }
            }
        ]
    }))
}

/// OP_RETURN bytes bar chart.
pub fn op_return_bytes_chart(blocks: &[OpReturnBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("OP_RETURN Bytes");
    }

    let runes: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.runes_bytes]))
        .collect();
    let carriers: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.data_carrier_bytes]))
        .collect();

    build_option(json!({
        "title": { "text": "OP_RETURN Bytes by Type", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Runes", "type": "bar", "stack": "total", "data": runes,
                "itemStyle": { "color": RUNES_COLOR }
            },
            {
                "name": "Data Carriers", "type": "bar", "stack": "total", "data": carriers,
                "itemStyle": { "color": CARRIER_COLOR }
            }
        ]
    }))
}

/// Runes dominance percentage line chart with moving average.
pub fn runes_pct_chart(blocks: &[OpReturnBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Runes Dominance %");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            let total = b.runes_count + b.data_carrier_count;
            if total > 0 {
                let v = b.runes_count as f64 / total as f64 * 100.0;
                (v * 1000.0).round() / 1000.0
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
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    build_option(json!({
        "title": { "text": "Runes Dominance %", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Runes %", "type": "line", "data": raw,
                "lineStyle": { "width": 1, "color": RUNES_COLOR },
                "itemStyle": { "color": RUNES_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_series,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// OP_RETURN count chart from daily aggregates.
pub fn op_return_count_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("OP_RETURN Count (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let runes: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.block_count > 0 {
                (d.total_runes_count as f64 / d.block_count as f64 * 1000.0)
                    .round()
                    / 1000.0
            } else {
                0.0
            }
        })
        .collect();
    let carriers: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.block_count > 0 {
                (d.total_data_carrier_count as f64 / d.block_count as f64
                    * 1000.0)
                    .round()
                    / 1000.0
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "title": { "text": "OP_RETURN Count by Type (daily avg per block)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Data Carriers", "type": "bar", "stack": "total", "data": carriers, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// OP_RETURN bytes chart from daily aggregates.
pub fn op_return_bytes_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("OP_RETURN Bytes (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let runes: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.block_count > 0 {
                ((d.total_runes_bytes as f64 / d.block_count as f64 / 1000.0)
                    * 10.0)
                    .round()
                    / 10.0
            } else {
                0.0
            }
        })
        .collect();
    let carriers: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.block_count > 0 {
                ((d.total_data_carrier_bytes as f64
                    / d.block_count as f64
                    / 1000.0)
                    * 10.0)
                    .round()
                    / 10.0
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "title": { "text": "OP_RETURN Bytes by Type (daily avg KB per block)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("KB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Data Carriers", "type": "bar", "stack": "total", "data": carriers, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// Runes dominance % from daily aggregates.
pub fn runes_pct_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Runes Dominance % (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total = d.total_runes_count + d.total_data_carrier_count;
            if total > 0 {
                let v = d.total_runes_count as f64 / total as f64 * 100.0;
                (v * 1000.0).round() / 1000.0
            } else {
                0.0
            }
        })
        .collect();
    let ma = moving_average(&vals, 7);

    build_option(json!({
        "title": { "text": "Runes Dominance % (daily)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes %", "type": "line", "data": vals, "lineStyle": { "width": 1, "color": RUNES_COLOR }, "itemStyle": { "color": RUNES_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma, "lineStyle": { "width": 2, "color": MA_COLOR }, "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Per-block signaling scatter/bar chart.
pub fn signaling_chart(blocks: &[SignalingBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("BIP Signaling");
    }

    let signaled: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), if b.signaled { 1 } else { 0 }]))
        .collect();

    build_option(json!({
        "title": { "text": "Per-Block Signaling", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": {
            "type": "value", "name": "Signaled",
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": {
                "color": "#aaa",
                "formatter": "{value}"
            },
            "min": 0, "max": 1, "interval": 1,
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Signaled", "type": "scatter", "data": signaled,
                "itemStyle": { "color": DATA_COLOR },
                "symbolSize": 4
            }
        ]
    }))
}

/// Signaling percentage per retarget period bar chart.
pub fn signaling_periods_chart(
    periods: &[SignalingPeriod],
    threshold: f64,
) -> String {
    if periods.is_empty() {
        return no_data_chart("Signaling Periods");
    }

    let cats: Vec<String> =
        periods.iter().map(|p| format_num(p.start_height)).collect();

    let bar_data: Vec<serde_json::Value> = periods
        .iter()
        .map(|p| {
            let color = if p.signaled_pct >= threshold {
                SIGNAL_YES
            } else if p.signaled_pct > 0.0 {
                TARGET_COLOR
            } else {
                "#333"
            };
            json!({
                "value": (p.signaled_pct * 1000.0).round() / 1000.0,
                "itemStyle": { "color": color }
            })
        })
        .collect();

    build_option(json!({
        "title": { "text": "Signaling % per Retarget Period", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": {
            "type": "category", "data": cats,
            "axisLabel": { "color": "#aaa", "rotate": 45, "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": {
            "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "grid": { "left": 45, "right": 20, "top": 50, "bottom": 80 },
        "dataZoom": data_zoom(),
        "tooltip": {
            "trigger": "axis",
            "formatter": "{b}<br/>Signaled: {c}%"
        },
        "series": [
            {
                "name": "Signaled %", "type": "bar", "data": bar_data,
                "barMaxWidth": 40,
                "markLine": {
                    "silent": true, "symbol": "none",
                    "lineStyle": { "color": "#f7931a", "type": "dashed", "width": 2 },
                    "data": [{ "yAxis": threshold, "label": { "formatter": format!("{}%", threshold), "color": "#f7931a", "fontSize": 12 } }]
                }
            }
        ]
    }))
}

// ---------------------------------------------------------------------------
// New chart builders
// ---------------------------------------------------------------------------

/// Block subsidy in satoshis for a given height.
fn block_subsidy(height: u64) -> u64 {
    let halvings = height / 210_000;
    if halvings >= 64 {
        return 0;
    }
    5_000_000_000u64 >> halvings
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
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    build_option(json!({
        "title": { "text": "Block Weight Utilization", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Utilization %", "type": "line", "data": raw,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_series,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
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
        "title": { "text": "Weight Utilization (Daily Avg)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
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

const SUBSIDY_COLOR: &str = "#9b59b6";

/// Block subsidy vs fee revenue ratio (stacked area).
pub fn subsidy_vs_fees_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Subsidy vs Fees");
    }

    let subsidy_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let sub = block_subsidy(b.height) as f64 / 100_000_000.0;
            let rounded = (sub * 1000.0).round() / 1000.0;
            json!([ts_ms(b.timestamp), rounded])
        })
        .collect();

    let fee_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let fee = b.total_fees as f64 / 100_000_000.0;
            let rounded = (fee * 1000.0).round() / 1000.0;
            json!([ts_ms(b.timestamp), rounded])
        })
        .collect();

    build_option(json!({
        "title": { "text": "Subsidy vs Fees (BTC)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Subsidy", "type": "line", "stack": "reward", "data": subsidy_data,
                "lineStyle": { "width": 1, "color": SUBSIDY_COLOR },
                "itemStyle": { "color": SUBSIDY_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(155,89,182,0.3)" }
            },
            {
                "name": "Fees", "type": "line", "stack": "reward", "data": fee_data,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Subsidy vs fees from daily aggregates.
pub fn subsidy_vs_fees_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Subsidy vs Fees");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    // Determine subsidy per day based on known halving dates
    let subsidy_vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.date.as_str() >= "2024-04-20" {
                3.125
            } else if d.date.as_str() >= "2020-05-11" {
                6.25
            } else if d.date.as_str() >= "2016-07-09" {
                12.5
            } else if d.date.as_str() >= "2012-11-28" {
                25.0
            } else {
                50.0
            }
        })
        .collect();
    let fee_vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.total_fees > 0 && d.block_count > 0 {
                let v =
                    d.total_fees as f64 / d.block_count as f64 / 100_000_000.0;
                let rounded = (v * 1000.0).round() / 1000.0;
                json!(rounded)
            } else {
                json!(null)
            }
        })
        .collect();

    build_option(json!({
        "title": { "text": "Subsidy vs Fees (Daily Avg BTC)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Subsidy", "type": "line", "stack": "reward", "data": subsidy_vals,
                "lineStyle": { "width": 1, "color": SUBSIDY_COLOR },
                "itemStyle": { "color": SUBSIDY_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(155,89,182,0.3)" }
            },
            {
                "name": "Avg Fees", "type": "line", "stack": "reward", "data": fee_vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
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
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    build_option(json!({
        "title": { "text": "Avg Transaction Size", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Tx Size", "type": "line", "data": raw,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_series,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
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
        "title": { "text": "Avg Transaction Size (Daily)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
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

fn format_num(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}

/// Fees line chart with unit toggle (sats or BTC).
pub fn fees_chart_unit(blocks: &[BlockSummary], unit: &str) -> String {
    if blocks.is_empty() {
        return no_data_chart("Fees");
    }

    let divisor = if unit == "btc" { 100_000_000.0 } else { 1.0 };
    let y_name = if unit == "btc" { "BTC" } else { "sats" };
    let title = if unit == "btc" {
        "Total Fees per Block (BTC)"
    } else {
        "Total Fees per Block (sats)"
    };

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            if b.total_fees > 0 {
                let v = b.total_fees as f64 / divisor;
                let rounded = (v * 1000.0).round() / 1000.0;
                json!([ts_ms(b.timestamp), rounded])
            } else {
                json!([ts_ms(b.timestamp), null])
            }
        })
        .collect();

    build_option(json!({
        "title": { "text": title, "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis(y_name),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Fees", "type": "line", "data": raw,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Fees from daily aggregates with unit toggle.
pub fn fees_chart_daily_unit(days: &[DailyAggregate], unit: &str) -> String {
    if days.is_empty() {
        return no_data_chart("Fees");
    }

    let divisor = if unit == "btc" { 100_000_000.0 } else { 1.0 };
    let y_name = if unit == "btc" { "BTC" } else { "sats" };
    let title = if unit == "btc" {
        "Avg Fees per Block (BTC/day)"
    } else {
        "Avg Fees per Block (sats/day)"
    };

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.total_fees > 0 && d.block_count > 0 {
                let v = d.total_fees as f64 / d.block_count as f64 / divisor;
                let rounded = (v * 1000.0).round() / 1000.0;
                json!(rounded)
            } else {
                json!(null)
            }
        })
        .collect();

    build_option(json!({
        "title": { "text": title, "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis(y_name),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Fees", "type": "line", "data": vals,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Phase 2: Mining & Adoption charts
// ---------------------------------------------------------------------------

const PIE_COLORS: [&str; 11] = [
    "#4ecdc4", "#f7931a", "#ff6b6b", "#bb8fff", "#2ecc71", "#e74c3c",
    "#3498db", "#e67e22", "#1abc9c", "#9b59b6", "#95a5a6",
];

/// Miner dominance donut chart.
pub fn miner_dominance_chart(miners: &[MinerShare]) -> String {
    if miners.is_empty() {
        return no_data_chart("Miner Dominance");
    }

    // Top 10 + "Other"
    let (top, rest) = if miners.len() > 10 {
        (&miners[..10], &miners[10..])
    } else {
        (miners, &[][..])
    };

    let mut pie_data: Vec<serde_json::Value> = top
        .iter()
        .map(|m| {
            json!({
                "name": m.miner,
                "value": m.count
            })
        })
        .collect();

    if !rest.is_empty() {
        let other_count: u64 = rest.iter().map(|m| m.count).sum();
        pie_data.push(json!({
            "name": "Other",
            "value": other_count
        }));
    }

    let colors: Vec<&str> =
        PIE_COLORS.iter().copied().take(pie_data.len()).collect();

    serde_json::to_string(&json!({
        "backgroundColor": "transparent",
        "color": colors,
        "title": {
            "text": "Miner Dominance",
            "textStyle": { "color": "#ccc", "fontSize": 14 }
        },
        "tooltip": {
            "trigger": "item",
            "formatter": "{b}: {c} blocks ({d}%)"
        },
        "legend": {
            "orient": "vertical",
            "right": 10,
            "top": 50,
            "textStyle": { "color": "#ccc", "fontSize": 11 }
        },
        "series": [{
            "name": "Miners",
            "type": "pie",
            "radius": ["45%", "70%"],
            "center": ["40%", "55%"],
            "avoidLabelOverlap": true,
            "itemStyle": {
                "borderRadius": 4,
                "borderColor": "#0e2a47",
                "borderWidth": 2
            },
            "label": {
                "show": true,
                "color": "#ccc",
                "fontSize": 11,
                "formatter": "{b}\n{d}%"
            },
            "emphasis": {
                "label": { "fontSize": 14, "fontWeight": "bold" }
            },
            "data": pie_data
        }]
    }))
    .unwrap_or_default()
}

/// Empty blocks scatter chart.
pub fn empty_blocks_chart(blocks: &[EmptyBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("No empty blocks in this range");
    }

    // Group by miner for color coding
    let mut miner_series: std::collections::HashMap<
        String,
        Vec<serde_json::Value>,
    > = std::collections::HashMap::new();
    for b in blocks {
        let miner = if b.miner.is_empty() {
            "Unknown".to_string()
        } else {
            b.miner.clone()
        };
        miner_series.entry(miner).or_default().push(json!([
            ts_ms(b.timestamp),
            1,
            b.height
        ]));
    }

    let mut series: Vec<serde_json::Value> = Vec::new();
    for (i, (miner, data)) in miner_series.iter().enumerate() {
        let color = PIE_COLORS[i % PIE_COLORS.len()];
        series.push(json!({
            "name": miner,
            "type": "scatter",
            "data": data,
            "symbolSize": 8,
            "itemStyle": { "color": color }
        }));
    }

    build_option(json!({
        "title": { "text": "Empty Blocks (coinbase only)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": {
            "type": "value", "show": false, "min": 0, "max": 2
        },
        "dataZoom": data_zoom(),
        "tooltip": {
            "trigger": "item",
            "formatter": "{a}<br/>Block: {c}"
        },
        "series": series
    }))
}

/// SegWit adoption % chart (per-block).
pub fn segwit_adoption_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("SegWit Adoption %");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 1 {
                let pct = b.segwit_spend_count as f64 / (b.tx_count - 1) as f64
                    * 100.0;
                (pct * 100.0).round() / 100.0
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
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    build_option(json!({
        "title": { "text": "SegWit Adoption % (tx with witness data)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "SegWit %", "type": "line", "data": raw,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_series,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// SegWit adoption % from daily aggregates.
pub fn segwit_adoption_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("SegWit Adoption %");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 1.0 {
                let pct =
                    d.avg_segwit_spend_count / (d.avg_tx_count - 1.0) * 100.0;
                (pct * 100.0).round() / 100.0
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
        "title": { "text": "SegWit Adoption % (Daily Avg)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "SegWit %", "type": "line", "data": vals,
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

const TAPROOT_COLOR: &str = "#f7931a";

/// Taproot outputs per block chart.
pub fn taproot_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Taproot Outputs");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| b.taproot_spend_count as f64)
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
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    build_option(json!({
        "title": { "text": "Taproot Outputs per Block", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Taproot Outputs", "type": "line", "data": raw,
                "lineStyle": { "width": 1, "color": TAPROOT_COLOR },
                "itemStyle": { "color": TAPROOT_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "144-block MA", "type": "line", "data": ma_series,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Taproot outputs from daily aggregates.
pub fn taproot_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Taproot Outputs");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> =
        days.iter().map(|d| d.avg_taproot_spend_count).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "title": { "text": "Taproot Outputs (Daily Avg per Block)", "textStyle": { "color": "#ccc", "fontSize": 14 } },
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Taproot Outputs", "type": "line", "data": vals,
                "lineStyle": { "width": 1, "color": TAPROOT_COLOR },
                "itemStyle": { "color": TAPROOT_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Mempool usage gauge chart.
pub fn mempool_gauge(usage: u64, max: u64) -> String {
    let pct = if max > 0 {
        (usage as f64 / max as f64 * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    serde_json::to_string(&json!({
        "backgroundColor": "transparent",
        "series": [{
            "type": "gauge",
            "center": ["50%", "55%"],
            "radius": "85%",
            "detail": {
                "formatter": "{value}%",
                "color": "#e0e0e0",
                "fontSize": 16,
                "offsetCenter": [0, "65%"]
            },
            "data": [{ "value": pct, "name": "Mempool" }],
            "axisLine": {
                "lineStyle": {
                    "width": 12,
                    "color": [[0.5, "#4ecdc4"], [0.8, "#f7931a"], [1, "#e74c3c"]]
                }
            },
            "pointer": { "width": 3, "length": "65%" },
            "title": { "color": "#aaa", "fontSize": 11, "offsetCenter": [0, "82%"] },
            "axisTick": { "show": false },
            "splitLine": { "length": 8, "lineStyle": { "color": "#666" } },
            "axisLabel": { "color": "#666", "distance": 15, "fontSize": 10 }
        }]
    }))
    .unwrap_or_default()
}
