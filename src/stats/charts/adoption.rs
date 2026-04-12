//! Adoption chart builders: SegWit adoption %, Taproot outputs, witness version
//! comparison and share, output type breakdown (legacy vs witness vs Taproot),
//! and Taproot spend types (key-path vs script-path).

use super::*;
use serde_json::json;
use std::fmt::Write;

const SEGWIT_V0_COLOR: &str = "#3b82f6"; // Blue for SegWit v0
const SEGWIT_V1_COLOR: &str = "#22c55e"; // Green for Taproot v1

/// SegWit adoption % chart (per-block).
pub fn segwit_adoption_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("SegWit Adoption %");
    }

    let segwit_fn = |b: &BlockSummary| {
        if b.tx_count > 1 {
            let pct = b.segwit_spend_count as f64 / (b.tx_count - 1) as f64
                * 100.0;
            (pct * 100.0).round() / 100.0
        } else {
            0.0
        }
    };
    let raw_str = build_data_array_f64(blocks, segwit_fn);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| segwit_fn(b)).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_series = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "SegWit %", "type": "line", "data": raw,
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

/// SegWit adoption % from daily aggregates.
pub fn segwit_adoption_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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

/// Taproot outputs per block chart.
pub fn taproot_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Taproot Outputs");
    }

    let raw_str = build_data_array_f64(blocks, |b| b.taproot_spend_count as f64);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| b.taproot_spend_count as f64).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_series = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Taproot Outputs", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": TAPROOT_COLOR },
        "itemStyle": { "color": TAPROOT_COLOR }, "symbol": "none",
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
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Taproot outputs from daily aggregates.
pub fn taproot_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Taproot Outputs");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_taproot_spend_count, 1))
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

/// SegWit v0 vs Taproot v1 stacked area chart (per-block).
pub fn witness_version_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Witness Versions");
    }

    // v0 outputs = P2WPKH + P2WSH, v1 outputs = P2TR
    let v0_str = build_data_array_f64(blocks, |b| (b.p2wpkh_count + b.p2wsh_count) as f64);
    let v0_data = data_array_value(&v0_str);
    let v1_str = build_data_array_f64(blocks, |b| b.p2tr_count as f64);
    let v1_data = data_array_value(&v1_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit", "type": "line", "data": v0_data,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot", "type": "line", "data": v1_data,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// SegWit v0 vs Taproot v1 stacked area chart (daily).
pub fn witness_version_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Witness Versions");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let v0_vals: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_p2wpkh_count + d.avg_p2wsh_count, 1))
        .collect();
    let v1_vals: Vec<f64> =
        days.iter().map(|d| round(d.avg_p2tr_count, 1)).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit", "type": "line", "data": v0_vals,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot", "type": "line", "data": v1_vals,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version percentage share — v0% vs v1% of total witness spends (per-block).
pub fn witness_version_pct_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Witness Version Share");
    }

    let v0_str = build_data_array_f64(blocks, |b| {
        let v0 = b.p2wpkh_count + b.p2wsh_count;
        let total = v0 + b.p2tr_count;
        if total > 0 {
            (v0 as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        }
    });
    let v0_data = data_array_value(&v0_str);
    let v1_str = build_data_array_f64(blocks, |b| {
        let v0 = b.p2wpkh_count + b.p2wsh_count;
        let total = v0 + b.p2tr_count;
        if total > 0 {
            (b.p2tr_count as f64 / total as f64 * 100.0 * 100.0).round()
                / 100.0
        } else {
            0.0
        }
    });
    let v1_data = data_array_value(&v1_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit", "type": "line", "data": v0_data,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot", "type": "line", "data": v1_data,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version percentage share (daily).
pub fn witness_version_pct_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Witness Version Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let v0_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            let v0 = d.avg_p2wpkh_count + d.avg_p2wsh_count;
            let total = v0 + d.avg_p2tr_count;
            if total > 0.0 {
                (v0 / total * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let v1_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            let v0 = d.avg_p2wpkh_count + d.avg_p2wsh_count;
            let total = v0 + d.avg_p2tr_count;
            if total > 0.0 {
                (d.avg_p2tr_count / total * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": { "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit", "type": "line", "data": v0_pct,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot", "type": "line", "data": v1_pct,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version as percentage of all transactions (per-block).
pub fn witness_version_tx_pct_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Witness Tx Share");
    }

    let pct_fn = |count: u64, total: u64| -> f64 {
        if total > 0 {
            (count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        }
    };
    let mut v0_buf = String::with_capacity(blocks.len() * 30);
    let mut v1_buf = String::with_capacity(blocks.len() * 30);
    let mut leg_buf = String::with_capacity(blocks.len() * 30);
    v0_buf.push('['); v1_buf.push('['); leg_buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 { v0_buf.push(','); v1_buf.push(','); leg_buf.push(','); }
        let v0 = pct_fn(b.p2wpkh_count + b.p2wsh_count, b.output_count);
        let v1 = pct_fn(b.p2tr_count, b.output_count);
        let leg = (100.0 - v0 - v1).max(0.0);
        let _ = write!(v0_buf, "[{},{},{}]", ts_ms(b.timestamp), v0, b.height);
        let _ = write!(v1_buf, "[{},{},{}]", ts_ms(b.timestamp), v1, b.height);
        let _ = write!(leg_buf, "[{},{},{}]", ts_ms(b.timestamp), leg, b.height);
    }
    v0_buf.push(']'); v1_buf.push(']'); leg_buf.push(']');
    let v0_data = data_array_value(&v0_buf);
    let v1_data = data_array_value(&v1_buf);
    let legacy_data = data_array_value(&leg_buf);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "% of Outputs", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Legacy", "type": "line", "data": legacy_data,
                "stack": "txpct", "areaStyle": { "opacity": 0.4 },
                "lineStyle": { "width": 0, "color": "#888" },
                "itemStyle": { "color": "#888" }, "symbol": "none"
            },
            {
                "name": "SegWit", "type": "line", "data": v0_data,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot", "type": "line", "data": v1_data,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version as percentage of all transactions (daily).
pub fn witness_version_tx_pct_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Witness Tx Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let v0_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_output_count > 0.0 {
                let v0 = d.avg_p2wpkh_count + d.avg_p2wsh_count;
                (v0 / d.avg_output_count * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let v1_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_output_count > 0.0 {
                (d.avg_p2tr_count / d.avg_output_count * 100.0 * 100.0).round()
                    / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let legacy_pct: Vec<f64> = v0_pct
        .iter()
        .zip(v1_pct.iter())
        .map(|(v0, v1)| (100.0 - v0 - v1).max(0.0))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": { "type": "value", "name": "% of Outputs", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Legacy", "type": "line", "data": legacy_pct,
                "stack": "txpct", "areaStyle": { "opacity": 0.4 },
                "lineStyle": { "width": 0, "color": "#888" },
                "itemStyle": { "color": "#888" }, "symbol": "none"
            },
            {
                "name": "SegWit", "type": "line", "data": v0_pct,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot", "type": "line", "data": v1_pct,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

const KEYPATH_COLOR: &str = "#22c55e"; // Green — privacy (indistinguishable from any spend)
const SCRIPTPATH_COLOR: &str = "#f59e0b"; // Amber — programmability (inscriptions, scripts)

/// Taproot key-path vs script-path spends (per-block).
pub fn taproot_spend_type_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Taproot Spend Types");
    }

    let keypath_str = build_data_array_i64(blocks, |b| b.taproot_keypath_count as i64);
    let keypath = data_array_value(&keypath_str);
    let scriptpath_str = build_data_array_i64(blocks, |b| b.taproot_scriptpath_count as i64);
    let scriptpath = data_array_value(&scriptpath_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Spends"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Key-path", "type": "line", "data": keypath, "stack": "tr", "areaStyle": { "opacity": 0.5 }, "lineStyle": { "width": 0, "color": KEYPATH_COLOR }, "itemStyle": { "color": KEYPATH_COLOR }, "symbol": "none" },
            { "name": "Script-path", "type": "line", "data": scriptpath, "stack": "tr", "areaStyle": { "opacity": 0.5 }, "lineStyle": { "width": 0, "color": SCRIPTPATH_COLOR }, "itemStyle": { "color": SCRIPTPATH_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Taproot key-path vs script-path spends (daily).
pub fn taproot_spend_type_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Taproot Spend Types");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let keypath: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_taproot_keypath_count, 1))
        .collect();
    let scriptpath: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_taproot_scriptpath_count, 1))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Key-path", "type": "line", "data": keypath, "stack": "tr", "areaStyle": { "opacity": 0.5 }, "lineStyle": { "width": 0, "color": KEYPATH_COLOR }, "itemStyle": { "color": KEYPATH_COLOR }, "symbol": "none" },
            { "name": "Script-path", "type": "line", "data": scriptpath, "stack": "tr", "areaStyle": { "opacity": 0.5 }, "lineStyle": { "width": 0, "color": SCRIPTPATH_COLOR }, "itemStyle": { "color": SCRIPTPATH_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Taproot adoption velocity (per-block).
/// Computes a 144-block moving average of Taproot output %, then derives
/// the rate of change (velocity) as the difference from 144 blocks ago.
/// Positive values indicate accelerating adoption, negative values indicate slowing.
pub fn taproot_velocity_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Taproot Velocity");
    }

    // Compute Taproot output % per block
    let pct: Vec<f64> = blocks
        .iter()
        .map(|b| {
            let total = b.p2pk_count + b.p2pkh_count + b.p2sh_count
                + b.p2wpkh_count + b.p2wsh_count + b.p2tr_count
                + b.multisig_count + b.unknown_script_count;
            if total > 0 {
                b.p2tr_count as f64 / total as f64 * 100.0
            } else {
                0.0
            }
        })
        .collect();

    // 144-block moving average
    let ma = moving_average(&pct, 144);

    // Velocity: ma[i] - ma[i-144]
    let velocity: Vec<Option<f64>> = (0..ma.len())
        .map(|i| {
            if i >= 144 {
                match (ma[i], ma[i - 144]) {
                    (Some(cur), Some(prev)) => Some(round(cur - prev, 4)),
                    _ => None,
                }
            } else {
                None
            }
        })
        .collect();

    let mut data_buf = String::with_capacity(blocks.len() * 30);
    data_buf.push('[');
    for (i, (b, v)) in blocks.iter().zip(velocity.iter()).enumerate() {
        if i > 0 { data_buf.push(','); }
        match v {
            Some(val) => { let _ = write!(data_buf, "[{},{},{}]", ts_ms(b.timestamp), val, b.height); }
            None => { let _ = write!(data_buf, "[{},null]", ts_ms(b.timestamp)); }
        }
    }
    data_buf.push(']');
    let data = data_array_value(&data_buf);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Taproot % Change (144-block)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Velocity", "type": "line", "data": data,
                "lineStyle": { "width": 1.5, "color": P2TR_COLOR },
                "itemStyle": { "color": P2TR_COLOR }, "symbol": "none",
                "markLine": {
                    "silent": true,
                    "data": [{ "yAxis": 0 }],
                    "lineStyle": { "color": "#aaa", "type": "dashed" },
                    "label": { "show": false }
                }
            }
        ]
    }))
}

/// Taproot adoption velocity from daily aggregates.
/// Same concept as per-block but using 30-day moving average and daily velocity.
pub fn taproot_velocity_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Taproot Velocity");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    // Compute Taproot % from avg counts
    let pct: Vec<f64> = days
        .iter()
        .map(|d| {
            let total = d.avg_p2pk_count + d.avg_p2pkh_count + d.avg_p2sh_count
                + d.avg_p2wpkh_count + d.avg_p2wsh_count + d.avg_p2tr_count
                + d.avg_multisig_count + d.avg_unknown_script_count;
            if total > 0.0 {
                d.avg_p2tr_count / total * 100.0
            } else {
                0.0
            }
        })
        .collect();

    // 30-day moving average
    let ma = moving_average(&pct, 30);

    // Velocity: ma[i] - ma[i-30]
    let velocity: Vec<serde_json::Value> = (0..ma.len())
        .map(|i| {
            if i >= 30 {
                match (ma[i], ma[i - 30]) {
                    (Some(cur), Some(prev)) => json!(round(cur - prev, 4)),
                    _ => json!(null),
                }
            } else {
                json!(null)
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Taproot % Change (30-day)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Velocity", "type": "line", "data": velocity,
                "lineStyle": { "width": 1.5, "color": P2TR_COLOR },
                "itemStyle": { "color": P2TR_COLOR }, "symbol": "none",
                "markLine": {
                    "silent": true,
                    "data": [{ "yAxis": 0 }],
                    "lineStyle": { "color": "#aaa", "type": "dashed" },
                    "label": { "show": false }
                }
            }
        ]
    }))
}

/// Cumulative SegWit and Taproot transaction count over time.
pub fn cumulative_adoption_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Cumulative Adoption");
    }

    let mut segwit_total: u64 = 0;
    let mut taproot_total: u64 = 0;

    let mut segwit_buf = String::with_capacity(blocks.len() * 30);
    let mut taproot_buf = String::with_capacity(blocks.len() * 30);
    segwit_buf.push('['); taproot_buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 { segwit_buf.push(','); taproot_buf.push(','); }
        segwit_total += b.segwit_spend_count;
        taproot_total += b.taproot_spend_count;
        let _ = write!(segwit_buf, "[{},{},{}]", ts_ms(b.timestamp), segwit_total, b.height);
        let _ = write!(taproot_buf, "[{},{},{}]", ts_ms(b.timestamp), taproot_total, b.height);
    }
    segwit_buf.push(']'); taproot_buf.push(']');
    let segwit_data = data_array_value(&segwit_buf);
    let taproot_data = data_array_value(&taproot_buf);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Cumulative Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit Transactions", "type": "line", "data": segwit_data,
                "lineStyle": { "width": 1.5, "color": P2WPKH_COLOR },
                "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(59,130,246,0.08)" }
            },
            {
                "name": "Taproot Outputs", "type": "line", "data": taproot_data,
                "lineStyle": { "width": 1.5, "color": P2TR_COLOR },
                "itemStyle": { "color": P2TR_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(34,197,94,0.08)" }
            }
        ]
    }))
}

/// Cumulative adoption from daily aggregates.
pub fn cumulative_adoption_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Cumulative Adoption");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    let mut segwit_total: f64 = 0.0;
    let segwit_data: Vec<f64> = days
        .iter()
        .map(|d| {
            segwit_total += d.avg_segwit_spend_count * d.block_count as f64;
            round(segwit_total, 0)
        })
        .collect();

    let mut taproot_total: f64 = 0.0;
    let taproot_data: Vec<f64> = days
        .iter()
        .map(|d| {
            taproot_total += d.avg_taproot_spend_count * d.block_count as f64;
            round(taproot_total, 0)
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Cumulative Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit Transactions", "type": "line", "data": segwit_data,
                "lineStyle": { "width": 1.5, "color": P2WPKH_COLOR },
                "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(59,130,246,0.08)" }
            },
            {
                "name": "Taproot Outputs", "type": "line", "data": taproot_data,
                "lineStyle": { "width": 1.5, "color": P2TR_COLOR },
                "itemStyle": { "color": P2TR_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(34,197,94,0.08)" }
            }
        ]
    }))
}

/// P2PKH address type sunset tracker (daily). Shows the declining share of
/// legacy P2PKH outputs with a 90-day MA and threshold markers at 10% and 5%.
pub fn address_sunset_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Address Type Sunset");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total = d.avg_p2pkh_count + d.avg_p2sh_count + d.avg_p2wpkh_count
                + d.avg_p2wsh_count + d.avg_p2tr_count + d.avg_p2pk_count
                + d.avg_multisig_count + d.avg_unknown_script_count;
            if total > 0.0 {
                round(d.avg_p2pkh_count / total * 100.0, 2)
            } else {
                0.0
            }
        })
        .collect();

    let ma = moving_average(&vals, 90);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("P2PKH Share (%)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "P2PKH %", "type": "line", "data": vals,
                "lineStyle": { "width": 1, "color": P2PKH_COLOR },
                "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none",
                "opacity": 0.4
            },
            {
                "name": "90-day MA", "type": "line", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            },
            {
                "name": "Thresholds", "type": "line", "data": [],
                "markLine": {
                    "silent": true, "symbol": "none",
                    "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 1 },
                    "data": [
                        { "yAxis": 10, "label": { "formatter": "10%", "color": TARGET_COLOR } },
                        { "yAxis": 5, "label": { "formatter": "5%", "color": TARGET_COLOR } }
                    ]
                }
            }
        ]
    }))
}
