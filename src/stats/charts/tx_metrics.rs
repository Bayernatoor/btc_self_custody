use serde_json::json;
use super::*;

/// Address type evolution — stacked area (per-block).
pub fn address_type_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Address Types");
    }

    let make_data = |f: fn(&BlockSummary) -> u64| -> Vec<serde_json::Value> {
        blocks.iter().map(|b| json!([ts_ms(b.timestamp), f(b)])).collect()
    };

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2pkh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2sh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2wpkh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2wsh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2tr_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2pk_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Address type evolution — stacked area (daily).
pub fn address_type_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Address Types");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| round(d.avg_p2pkh_count, 1)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| round(d.avg_p2sh_count, 1)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| round(d.avg_p2wpkh_count, 1)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| round(d.avg_p2wsh_count, 1)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| round(d.avg_p2tr_count, 1)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| round(d.avg_p2pk_count, 1)).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Witness data as % of block size (per-block).
pub fn witness_share_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Witness Data Share");
    }

    let vals: Vec<f64> = blocks.iter().map(|b| {
        if b.size > 0 { (b.witness_bytes as f64 / b.size as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let raw: Vec<serde_json::Value> = blocks.iter().zip(vals.iter()).map(|(b, v)| json!([ts_ms(b.timestamp), v])).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks.iter().zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v]))).collect();
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Witness %", "type": "line", "sampling": "lttb", "data": raw,
        "areaStyle": { "color": P2WPKH_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": P2WPKH_COLOR },
        "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none",
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
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Witness data as % of block size (daily).
pub fn witness_share_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Witness Data Share");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| {
        if d.avg_size > 0.0 { (d.avg_witness_bytes / d.avg_size * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma.iter().map(|v| match v { Some(x) => json!(x), None => json!(null) }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Witness %", "type": "line", "sampling": "lttb", "data": vals, "areaStyle": { "color": P2WPKH_COLOR, "opacity": 0.15 }, "lineStyle": { "width": 1, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals, "lineStyle": { "width": 2, "color": MA_COLOR }, "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// RBF adoption — % of transactions signaling RBF (per-block).
pub fn rbf_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("RBF Adoption");
    }

    let vals: Vec<f64> = blocks.iter().map(|b| {
        if b.tx_count > 1 { (b.rbf_count as f64 / (b.tx_count - 1) as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let raw: Vec<serde_json::Value> = blocks.iter().zip(vals.iter()).map(|(b, v)| json!([ts_ms(b.timestamp), v])).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks.iter().zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v]))).collect();
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "RBF %", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": RBF_COLOR },
        "itemStyle": { "color": RBF_COLOR }, "symbol": "none",
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
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("% of Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// RBF adoption (daily).
pub fn rbf_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("RBF Adoption");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| {
        if d.avg_tx_count > 1.0 { (d.avg_rbf_count / (d.avg_tx_count - 1.0) * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma.iter().map(|v| match v { Some(x) => json!(x), None => json!(null) }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "RBF %", "type": "line", "sampling": "lttb", "data": vals, "lineStyle": { "width": 1, "color": RBF_COLOR }, "itemStyle": { "color": RBF_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals, "lineStyle": { "width": 2, "color": MA_COLOR }, "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// UTXO flow — inputs (consumed) vs outputs (created) per block.
pub fn utxo_flow_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("UTXO Flow");
    }

    let inputs: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.input_count])).collect();
    let outputs: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.output_count])).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Inputs (consumed)", "type": "line", "sampling": "lttb", "data": inputs, "lineStyle": { "width": 1, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": 0.5 },
            { "name": "Outputs (created)", "type": "line", "sampling": "lttb", "data": outputs, "lineStyle": { "width": 1, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": 0.5 }
        ]
    }))
}

/// UTXO flow (daily).
pub fn utxo_flow_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("UTXO Flow");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let inputs: Vec<f64> = days.iter().map(|d| round(d.avg_input_count, 1)).collect();
    let outputs: Vec<f64> = days.iter().map(|d| round(d.avg_output_count, 1)).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Inputs (consumed)", "type": "line", "sampling": "lttb", "data": inputs, "lineStyle": { "width": 1, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": 0.5 },
            { "name": "Outputs (created)", "type": "line", "sampling": "lttb", "data": outputs, "lineStyle": { "width": 1, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": 0.5 }
        ]
    }))
}
