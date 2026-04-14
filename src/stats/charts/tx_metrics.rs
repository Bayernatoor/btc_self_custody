//! Transaction metric chart builders: address type evolution (stacked area),
//! address type share (%), witness data share, transaction batching (avg
//! inputs/outputs per tx), RBF adoption, and UTXO flow (inputs vs outputs).

use super::*;
use serde_json::json;
use std::fmt::Write;

/// Address type evolution - stacked area (per-block).
pub fn address_type_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Address Types");
    }

    let make_data = |f: fn(&BlockSummary) -> u64| -> serde_json::Value {
        let s = build_data_array_i64(blocks, |b| f(b) as i64);
        data_array_value(&s)
    };

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "data": make_data(|b| b.p2pkh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "data": make_data(|b| b.p2sh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "data": make_data(|b| b.p2wpkh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "data": make_data(|b| b.p2wsh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "data": make_data(|b| b.p2tr_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "data": make_data(|b| b.p2pk_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Address type evolution — stacked area (daily totals).
pub fn address_type_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Address Types");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let total = |avg: f64, bc: u64| -> f64 { round(avg * bc as f64, 0) };

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Outputs/Day"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "data": days.iter().map(|d| total(d.avg_p2pkh_count, d.block_count)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "data": days.iter().map(|d| total(d.avg_p2sh_count, d.block_count)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "data": days.iter().map(|d| total(d.avg_p2wpkh_count, d.block_count)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "data": days.iter().map(|d| total(d.avg_p2wsh_count, d.block_count)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "data": days.iter().map(|d| total(d.avg_p2tr_count, d.block_count)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "data": days.iter().map(|d| total(d.avg_p2pk_count, d.block_count)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Witness data as % of block size (per-block).
pub fn witness_share_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Witness Data Share");
    }

    let witness_fn = |b: &BlockSummary| {
        if b.size > 0 {
            (b.witness_bytes as f64 / b.size as f64 * 100.0 * 100.0).round()
                / 100.0
        } else {
            0.0
        }
    };
    let raw_str = build_data_array_f64(blocks, witness_fn);
    let raw = data_array_value(&raw_str);
    let vals: Vec<f64> = blocks.iter().map(witness_fn).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Witness %", "type": "line", "data": raw,
        "areaStyle": { "color": P2WPKH_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": P2WPKH_COLOR },
        "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none",
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
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Witness data as % of block size (daily).
pub fn witness_share_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Witness Data Share");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_size > 0.0 {
                (d.avg_witness_bytes / d.avg_size * 100.0 * 100.0).round()
                    / 100.0
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
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Witness %", "type": "line", "data": vals, "areaStyle": { "color": P2WPKH_COLOR, "opacity": 0.15 }, "lineStyle": { "width": 1, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals, "lineStyle": { "width": 2, "color": MA_COLOR }, "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Transaction batching — avg outputs and inputs per transaction (per-block).
pub fn batching_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Transaction Batching");
    }

    let out_fn = |b: &BlockSummary| {
        if b.tx_count > 0 { round(b.output_count as f64 / b.tx_count as f64, 2) } else { 0.0 }
    };
    let in_fn = |b: &BlockSummary| {
        if b.tx_count > 0 { round(b.input_count as f64 / b.tx_count as f64, 2) } else { 0.0 }
    };
    let out_raw_str = build_data_array_f64(blocks, out_fn);
    let out_raw = data_array_value(&out_raw_str);
    let in_raw_str = build_data_array_f64(blocks, in_fn);
    let in_raw = data_array_value(&in_raw_str);

    let out_per_tx: Vec<f64> = blocks.iter().map(out_fn).collect();
    let in_per_tx: Vec<f64> = blocks.iter().map(in_fn).collect();
    let out_ma = moving_average(&out_per_tx, 144);
    let in_ma = moving_average(&in_per_tx, 144);
    let out_ma_str = build_ma_array(blocks, &out_ma);
    let out_ma_data = data_array_value(&out_ma_str);
    let in_ma_str = build_ma_array(blocks, &in_ma);
    let in_ma_data = data_array_value(&in_ma_str);
    let has_ma = show_ma(blocks.len());

    let mut series = vec![
        json!({ "name": "Outputs/Tx", "type": "line", "data": out_raw, "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": if has_ma { 0.3 } else { 1.0 } }),
        json!({ "name": "Inputs/Tx", "type": "line", "data": in_raw, "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": if has_ma { 0.3 } else { 1.0 } }),
    ];
    if has_ma {
        series.push(json!({ "name": "Outputs MA", "type": "line", "data": out_ma_data, "lineStyle": { "width": 2, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none" }));
        series.push(json!({ "name": "Inputs MA", "type": "line", "data": in_ma_data, "lineStyle": { "width": 2, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none" }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Per Tx"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": series
    }))
}

/// Transaction batching (daily).
pub fn batching_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Transaction Batching");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let out_per_tx: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 0.0 {
                round(d.avg_output_count / d.avg_tx_count, 2)
            } else {
                0.0
            }
        })
        .collect();
    let in_per_tx: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 0.0 {
                round(d.avg_input_count / d.avg_tx_count, 2)
            } else {
                0.0
            }
        })
        .collect();
    let out_ma = moving_average(&out_per_tx, 7);
    let in_ma = moving_average(&in_per_tx, 7);
    let out_ma_vals: Vec<serde_json::Value> = out_ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();
    let in_ma_vals: Vec<serde_json::Value> = in_ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Per Tx"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Outputs/Tx", "type": "line", "data": out_per_tx, "lineStyle": { "width": 1, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": 0.3 },
            { "name": "Inputs/Tx", "type": "line", "data": in_per_tx, "lineStyle": { "width": 1, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": 0.3 },
            { "name": "Outputs MA", "type": "line", "data": out_ma_vals, "lineStyle": { "width": 2, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none" },
            { "name": "Inputs MA", "type": "line", "data": in_ma_vals, "lineStyle": { "width": 2, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none" }
        ]
    }))
}

/// Address type as % of total outputs (per-block) — 100% stacked area.
pub fn address_type_pct_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Address Type Share");
    }

    let make_pct = |f: fn(&BlockSummary) -> u64| -> serde_json::Value {
        let s = build_data_array_f64(blocks, |b| {
            let total = b.p2pkh_count
                + b.p2sh_count
                + b.p2wpkh_count
                + b.p2wsh_count
                + b.p2tr_count
                + b.p2pk_count;
            if total > 0 {
                round(f(b) as f64 / total as f64 * 100.0, 2)
            } else {
                0.0
            }
        });
        data_array_value(&s)
    };

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "%", "max": 100, "nameTextStyle": { "color": "#aaa" }, "axisLabel": { "color": "#aaa" }, "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } } },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "data": make_pct(|b| b.p2pkh_count), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "data": make_pct(|b| b.p2sh_count), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "data": make_pct(|b| b.p2wpkh_count), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "data": make_pct(|b| b.p2wsh_count), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "data": make_pct(|b| b.p2tr_count), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "data": make_pct(|b| b.p2pk_count), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Address type as % of total outputs (daily) — 100% stacked area.
pub fn address_type_pct_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Address Type Share");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let pct = |count: f64, total: f64| -> f64 {
        if total > 0.0 {
            round(count / total * 100.0, 2)
        } else {
            0.0
        }
    };
    let total_per_day: Vec<f64> = days
        .iter()
        .map(|d| {
            d.avg_p2pkh_count
                + d.avg_p2sh_count
                + d.avg_p2wpkh_count
                + d.avg_p2wsh_count
                + d.avg_p2tr_count
                + d.avg_p2pk_count
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": { "type": "value", "name": "%", "max": 100, "nameTextStyle": { "color": "#aaa" }, "axisLabel": { "color": "#aaa" }, "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } } },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "data": days.iter().zip(total_per_day.iter()).map(|(d, t)| pct(d.avg_p2pkh_count, *t)).collect::<Vec<f64>>(), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "data": days.iter().zip(total_per_day.iter()).map(|(d, t)| pct(d.avg_p2sh_count, *t)).collect::<Vec<f64>>(), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "data": days.iter().zip(total_per_day.iter()).map(|(d, t)| pct(d.avg_p2wpkh_count, *t)).collect::<Vec<f64>>(), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "data": days.iter().zip(total_per_day.iter()).map(|(d, t)| pct(d.avg_p2wsh_count, *t)).collect::<Vec<f64>>(), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "data": days.iter().zip(total_per_day.iter()).map(|(d, t)| pct(d.avg_p2tr_count, *t)).collect::<Vec<f64>>(), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "data": days.iter().zip(total_per_day.iter()).map(|(d, t)| pct(d.avg_p2pk_count, *t)).collect::<Vec<f64>>(), "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// RBF adoption — % of transactions signaling RBF (per-block).
/// BIP-125 opt-in RBF was merged in Core v0.12.0 (2016-02-23).
/// Pre-BIP-125 nSequence < 0xFFFFFFFE was used for other purposes
/// (original Satoshi replacement, nLockTime), so we gate to post-BIP-125.
const BIP125_TIMESTAMP: u64 = 1456185600; // 2016-02-23 00:00 UTC

pub fn rbf_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("RBF Adoption");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            // Only count RBF after BIP-125 (Core v0.12, Feb 2016)
            if b.timestamp < BIP125_TIMESTAMP {
                return 0.0;
            }
            if b.tx_count > 1 {
                let pct = b.rbf_count as f64 / (b.tx_count - 1) as f64 * 100.0;
                (pct.min(100.0) * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let mut raw_buf = String::with_capacity(blocks.len() * 30);
    raw_buf.push('[');
    for (i, (b, v)) in blocks.iter().zip(vals.iter()).enumerate() {
        if i > 0 { raw_buf.push(','); }
        if b.timestamp < BIP125_TIMESTAMP {
            let _ = write!(raw_buf, "[{},null]", ts_ms(b.timestamp));
        } else {
            let _ = write!(raw_buf, "[{},{},{}]", ts_ms(b.timestamp), v, b.height);
        }
    }
    raw_buf.push(']');
    let raw = data_array_value(&raw_buf);
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "RBF %", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": RBF_COLOR },
        "itemStyle": { "color": RBF_COLOR }, "symbol": "none",
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
        "yAxis": y_axis("% of Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// RBF adoption (daily).
pub fn rbf_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("RBF Adoption");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            // Gate to post-BIP-125 (Feb 2016)
            if d.date.as_str() < "2016-02-23" {
                return json!(null);
            }
            if d.avg_tx_count > 1.0 {
                let pct = d.avg_rbf_count / (d.avg_tx_count - 1.0) * 100.0;
                json!((pct.min(100.0) * 100.0).round() / 100.0)
            } else {
                json!(0.0)
            }
        })
        .collect();
    // Extract f64 for MA calculation (nulls become 0)
    let ma_input: Vec<f64> =
        vals.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
    let ma = moving_average(&ma_input, 7);
    let ma_vals: Vec<serde_json::Value> = days
        .iter()
        .zip(ma.iter())
        .map(|(d, v)| {
            if d.date.as_str() < "2016-02-23" {
                json!(null)
            } else {
                match v {
                    Some(x) => json!(x),
                    None => json!(null),
                }
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "RBF %", "type": "line", "data": vals, "lineStyle": { "width": 1, "color": RBF_COLOR }, "itemStyle": { "color": RBF_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals, "lineStyle": { "width": 2, "color": MA_COLOR }, "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// UTXO flow — inputs (consumed) vs outputs (created) per block.
pub fn utxo_flow_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("UTXO Flow");
    }

    let inputs_str = build_data_array_i64(blocks, |b| b.input_count as i64);
    let inputs = data_array_value(&inputs_str);
    let outputs_str = build_data_array_i64(blocks, |b| b.output_count as i64);
    let outputs = data_array_value(&outputs_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Inputs (consumed)", "type": "line", "data": inputs, "lineStyle": { "width": 1, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": 0.5 },
            { "name": "Outputs (created)", "type": "line", "data": outputs, "lineStyle": { "width": 1, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": 0.5 }
        ]
    }))
}

/// UTXO flow (daily).
pub fn utxo_flow_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("UTXO Flow");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let inputs: Vec<f64> =
        days.iter().map(|d| round(d.avg_input_count, 1)).collect();
    let outputs: Vec<f64> =
        days.iter().map(|d| round(d.avg_output_count, 1)).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Inputs (consumed)", "type": "line", "data": inputs, "lineStyle": { "width": 1, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": 0.5 },
            { "name": "Outputs (created)", "type": "line", "data": outputs, "lineStyle": { "width": 1, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": 0.5 }
        ]
    }))
}

/// Net UTXO set change per block (outputs minus inputs).
/// Positive = UTXO set growing, negative = consolidation.
pub fn utxo_growth_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("UTXO Growth Rate");
    }

    let data_str = build_data_array_i64(blocks, |b| b.output_count as i64 - b.input_count as i64);
    let data = data_array_value(&data_str);

    let raw: Vec<f64> = blocks.iter().map(|b| b.output_count as f64 - b.input_count as f64).collect();
    let ma = moving_average(&raw, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let mut series = vec![json!({
        "name": "Net UTXO Change", "type": "bar", "data": data,
        "itemStyle": { "color": DATA_COLOR }, "barMaxWidth": 3
    })];

    if show_ma(blocks.len()) {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Net UTXOs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": series
    }))
}

/// Net UTXO growth from daily aggregates.
pub fn utxo_growth_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("UTXO Growth Rate");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let data: Vec<f64> = days
        .iter()
        .map(|d| {
            let net = (d.avg_output_count - d.avg_input_count) * d.block_count as f64;
            round(net, 0)
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Net UTXOs / day"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "Net UTXO Change", "type": "bar", "data": data,
            "itemStyle": { "color": DATA_COLOR }
        }]
    }))
}

/// Transaction density: transactions per kilobyte of block space.
/// Higher = smaller, more efficient transactions. Lower = larger txs.
pub fn tx_density_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Transaction Density");
    }

    let density_fn = |b: &BlockSummary| {
        if b.size > 0 {
            round(b.tx_count as f64 / (b.size as f64 / 1000.0), 2)
        } else {
            0.0
        }
    };
    let data_str = build_data_array_f64(blocks, density_fn);
    let data = data_array_value(&data_str);

    let raw: Vec<f64> = blocks.iter().map(density_fn).collect();
    let ma = moving_average(&raw, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let mut series = vec![json!({
        "name": "TX/KB", "type": "line", "data": data,
        "lineStyle": { "width": 1, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
    })];

    if show_ma(blocks.len()) {
        series.push(json!({
            "name": "144-block MA", "type": "line", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("TX/KB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": series
    }))
}

/// Transaction density from daily aggregates.
pub fn tx_density_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Transaction Density");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let data: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_size > 0.0 {
                round(d.avg_tx_count / (d.avg_size / 1000.0), 2)
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("TX/KB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "TX Density", "type": "line", "data": data,
            "lineStyle": { "width": 1.5, "color": DATA_COLOR },
            "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
        }]
    }))
}

// ---------------------------------------------------------------------------
// Tier 2: Backfill v10 charts
// ---------------------------------------------------------------------------

/// Transaction type evolution: stacked percentage area of legacy, SegWit, and Taproot txs.
/// Shows the migration from legacy to modern transaction formats over time.
/// Requires backfill v10 for legacy_tx_count/segwit_tx_count/taproot_tx_count data.
pub fn tx_type_evolution_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Tx Type Evolution");
    }

    // Check that v10 tx type data is available
    let has_data = blocks.iter().any(|b| {
        b.legacy_tx_count > 0 || b.segwit_tx_count > 0 || b.taproot_tx_count > 0
    });
    if !has_data {
        return no_data_chart("Tx Type Evolution");
    }

    let legacy_str = build_data_array_f64(blocks, |b| {
        let total = b.legacy_tx_count + b.segwit_tx_count + b.taproot_tx_count;
        if total > 0 { round(b.legacy_tx_count as f64 / total as f64 * 100.0, 2) } else { 0.0 }
    });
    let legacy_data = data_array_value(&legacy_str);

    let segwit_str = build_data_array_f64(blocks, |b| {
        let total = b.legacy_tx_count + b.segwit_tx_count + b.taproot_tx_count;
        if total > 0 { round(b.segwit_tx_count as f64 / total as f64 * 100.0, 2) } else { 0.0 }
    });
    let segwit_data = data_array_value(&segwit_str);

    let taproot_str = build_data_array_f64(blocks, |b| {
        let total = b.legacy_tx_count + b.segwit_tx_count + b.taproot_tx_count;
        if total > 0 { round(b.taproot_tx_count as f64 / total as f64 * 100.0, 2) } else { 0.0 }
    });
    let taproot_data = data_array_value(&taproot_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Legacy", "type": "line", "stack": "txtype", "data": legacy_data,
                "lineStyle": { "width": 0, "color": P2PKH_COLOR },
                "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 }
            },
            {
                "name": "SegWit v0", "type": "line", "stack": "txtype", "data": segwit_data,
                "lineStyle": { "width": 0, "color": P2WPKH_COLOR },
                "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 }
            },
            {
                "name": "Taproot", "type": "line", "stack": "txtype", "data": taproot_data,
                "lineStyle": { "width": 0, "color": P2TR_COLOR },
                "itemStyle": { "color": P2TR_COLOR }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 }
            }
        ]
    }))
}
