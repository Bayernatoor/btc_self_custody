use super::*;
use serde_json::json;

/// Address type evolution — stacked area (per-block).
pub fn address_type_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Address Types");
    }

    let make_data = |f: fn(&BlockSummary) -> u64| -> Vec<serde_json::Value> {
        blocks.iter().map(|b| dp(b, f(b))).collect()
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

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.size > 0 {
                (b.witness_bytes as f64 / b.size as f64 * 100.0 * 100.0).round()
                    / 100.0
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

    let out_per_tx: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 0 {
                round(b.output_count as f64 / b.tx_count as f64, 2)
            } else {
                0.0
            }
        })
        .collect();
    let in_per_tx: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 0 {
                round(b.input_count as f64 / b.tx_count as f64, 2)
            } else {
                0.0
            }
        })
        .collect();
    let out_raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(out_per_tx.iter())
        .map(|(b, v)| dp(b, v))
        .collect();
    let in_raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(in_per_tx.iter())
        .map(|(b, v)| dp(b, v))
        .collect();
    let out_ma = moving_average(&out_per_tx, 144);
    let in_ma = moving_average(&in_per_tx, 144);
    let out_ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(out_ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();
    let in_ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(in_ma.iter())
        .map(|(b, m)| {
            json!([
                ts_ms(b.timestamp),
                m.map(|v| json!(v)).unwrap_or(json!(null))
            ])
        })
        .collect();
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

    let pct = |count: u64, total: u64| -> f64 {
        if total > 0 {
            round(count as f64 / total as f64 * 100.0, 2)
        } else {
            0.0
        }
    };
    let make_pct = |f: fn(&BlockSummary) -> u64| -> Vec<serde_json::Value> {
        blocks
            .iter()
            .map(|b| {
                let total = b.p2pkh_count
                    + b.p2sh_count
                    + b.p2wpkh_count
                    + b.p2wsh_count
                    + b.p2tr_count
                    + b.p2pk_count;
                dp(b, pct(f(b), total))
            })
            .collect()
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
    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| {
            if b.timestamp < BIP125_TIMESTAMP {
                json!([ts_ms(b.timestamp), null])
            } else {
                dp(b, v)
            }
        })
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

    let inputs: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.input_count)).collect();
    let outputs: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.output_count)).collect();

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
