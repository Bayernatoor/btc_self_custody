use super::*;
use serde_json::json;

const INSCRIPTION_COLOR: &str = "#06b6d4"; // Cyan for inscriptions

/// OP_RETURN count bar chart (runes vs data carriers).
pub fn op_return_count_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Count");
    }

    let runes: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.runes_count)).collect();
    let omni: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.omni_count)).collect();
    let xcp: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.counterparty_count)).collect();
    let other: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.data_carrier_count)).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// OP_RETURN count chart from daily aggregates.
pub fn op_return_count_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Embedded Data Count (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let avg = |total: u64, bc: u64| -> f64 {
        if bc > 0 {
            (total as f64 / bc as f64 * 1000.0).round() / 1000.0
        } else {
            0.0
        }
    };
    let runes: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_runes_count, d.block_count))
        .collect();
    let omni: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_omni_count, d.block_count))
        .collect();
    let xcp: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_counterparty_count, d.block_count))
        .collect();
    let other: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_data_carrier_count, d.block_count))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// OP_RETURN bytes bar chart.
pub fn op_return_bytes_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Volume");
    }

    let runes: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.runes_bytes)).collect();
    let omni: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.omni_bytes)).collect();
    let xcp: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.counterparty_bytes)).collect();
    let other: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.data_carrier_bytes)).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// OP_RETURN bytes chart from daily aggregates.
pub fn op_return_bytes_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Embedded Data Volume (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let avg_kb = |total: u64, bc: u64| -> f64 {
        if bc > 0 {
            ((total as f64 / bc as f64 / 1000.0) * 10.0).round() / 10.0
        } else {
            0.0
        }
    };
    let runes: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_runes_bytes, d.block_count))
        .collect();
    let omni: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_omni_bytes, d.block_count))
        .collect();
    let xcp: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_counterparty_bytes, d.block_count))
        .collect();
    let other: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_data_carrier_bytes, d.block_count))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("KB/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// Protocol dominance — 100% stacked area showing share of each protocol.
pub fn runes_pct_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Protocol Dominance");
    }

    let pct = |count: u64, total: u64| -> f64 {
        if total > 0 {
            (count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        }
    };

    let runes_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let total = b.op_return_count;
            dp(b, pct(b.runes_count, total))
        })
        .collect();
    let omni_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let total = b.op_return_count;
            dp(b, pct(b.omni_count, total))
        })
        .collect();
    let xcp_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let total = b.op_return_count;
            dp(b, pct(b.counterparty_count, total))
        })
        .collect();
    let other_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let total = b.op_return_count;
            dp(b, pct(b.data_carrier_count, total))
        })
        .collect();

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
            { "name": "Runes", "type": "line", "data": runes_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": RUNES_COLOR }, "itemStyle": { "color": RUNES_COLOR }, "symbol": "none" },
            { "name": "Omni", "type": "line", "data": omni_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": OMNI_COLOR }, "itemStyle": { "color": OMNI_COLOR }, "symbol": "none" },
            { "name": "Counterparty", "type": "line", "data": xcp_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": COUNTERPARTY_COLOR }, "itemStyle": { "color": COUNTERPARTY_COLOR }, "symbol": "none" },
            { "name": "Other", "type": "line", "data": other_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": CARRIER_COLOR }, "itemStyle": { "color": CARRIER_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Protocol dominance % from daily aggregates — 100% stacked area.
pub fn runes_pct_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Protocol Dominance (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let pct = |count: u64, total: u64| -> f64 {
        if total > 0 {
            (count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        }
    };
    let runes: Vec<f64> = days
        .iter()
        .map(|d| pct(d.total_runes_count, d.total_op_return_count))
        .collect();
    let omni: Vec<f64> = days
        .iter()
        .map(|d| pct(d.total_omni_count, d.total_op_return_count))
        .collect();
    let xcp: Vec<f64> = days
        .iter()
        .map(|d| pct(d.total_counterparty_count, d.total_op_return_count))
        .collect();
    let other: Vec<f64> = days
        .iter()
        .map(|d| pct(d.total_data_carrier_count, d.total_op_return_count))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
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
            { "name": "Runes", "type": "line", "data": runes, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": RUNES_COLOR }, "itemStyle": { "color": RUNES_COLOR }, "symbol": "none" },
            { "name": "Omni", "type": "line", "data": omni, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": OMNI_COLOR }, "itemStyle": { "color": OMNI_COLOR }, "symbol": "none" },
            { "name": "Counterparty", "type": "line", "data": xcp, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": COUNTERPARTY_COLOR }, "itemStyle": { "color": COUNTERPARTY_COLOR }, "symbol": "none" },
            { "name": "Other", "type": "line", "data": other, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": CARRIER_COLOR }, "itemStyle": { "color": CARRIER_COLOR }, "symbol": "none" }
        ]
    }))
}

/// OP_RETURN bytes as percentage of total block size (per-block).
pub fn op_return_block_share_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Block Share");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.size > 0 {
                (b.op_return_bytes as f64 / b.size as f64 * 100.0 * 100.0)
                    .round()
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
        "name": "OP_RETURN %", "type": "line", "data": raw,
        "areaStyle": { "color": RUNES_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": RUNES_COLOR },
        "itemStyle": { "color": RUNES_COLOR }, "symbol": "none",
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

/// OP_RETURN bytes as percentage of total block size (daily).
pub fn op_return_block_share_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Embedded Data Block Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total_size = d.avg_size * d.block_count as f64;
            if total_size > 0.0 {
                (d.total_op_return_bytes as f64 / total_size * 100.0 * 100.0)
                    .round()
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
            {
                "name": "OP_RETURN %", "type": "line", "data": vals,
                "areaStyle": { "color": RUNES_COLOR, "opacity": 0.15 },
                "lineStyle": { "width": 1, "color": RUNES_COLOR },
                "itemStyle": { "color": RUNES_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Ordinals inscription count per block.
pub fn inscription_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Inscriptions");
    }

    let vals: Vec<f64> =
        blocks.iter().map(|b| b.inscription_count as f64).collect();
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
        "name": "Inscriptions", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": INSCRIPTION_COLOR },
        "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none",
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
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Ordinals inscription count (daily).
pub fn inscription_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Inscriptions");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_inscription_count, 1))
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
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Inscriptions", "type": "line", "data": vals,
              "lineStyle": { "width": 1, "color": INSCRIPTION_COLOR },
              "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Inscription data as % of block size (per-block).
pub fn inscription_share_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Inscription Block Share");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.size > 0 {
                (b.inscription_bytes as f64 / b.size as f64 * 100.0 * 100.0)
                    .round()
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
        "name": "Inscriptions %", "type": "line", "data": raw,
        "areaStyle": { "color": INSCRIPTION_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": INSCRIPTION_COLOR },
        "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none",
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

/// Inscription data as % of block size (daily).
pub fn inscription_share_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Inscription Block Share");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_size > 0.0 {
                (d.avg_inscription_bytes / d.avg_size * 100.0 * 100.0).round()
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
            { "name": "Inscriptions %", "type": "line", "data": vals,
              "areaStyle": { "color": INSCRIPTION_COLOR, "opacity": 0.15 },
              "lineStyle": { "width": 1, "color": INSCRIPTION_COLOR },
              "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Unified embedded data charts (OP_RETURN + Inscriptions combined)
// ---------------------------------------------------------------------------

const OPRETURN_COLOR: &str = "#f59e0b"; // Amber for OP_RETURN aggregate

/// All embedded data as % of block size — OP_RETURN + inscriptions stacked (per-block).
pub fn all_embedded_share_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("All Embedded Data Share");
    }

    let op_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let v = if b.size > 0 {
                round(b.op_return_bytes as f64 / b.size as f64 * 100.0, 2)
            } else {
                0.0
            };
            dp(b, v)
        })
        .collect();
    let insc_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let v = if b.size > 0 {
                round(b.inscription_bytes as f64 / b.size as f64 * 100.0, 2)
            } else {
                0.0
            };
            dp(b, v)
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "OP_RETURN", "type": "line", "data": op_data,
                "stack": "embed", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": OPRETURN_COLOR },
                "itemStyle": { "color": OPRETURN_COLOR }, "symbol": "none"
            },
            {
                "name": "Inscriptions", "type": "line", "data": insc_data,
                "stack": "embed", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": INSCRIPTION_COLOR },
                "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// All embedded data as % of block size — stacked (daily).
pub fn all_embedded_share_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("All Embedded Data Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let op_vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total_size = d.avg_size * d.block_count as f64;
            if total_size > 0.0 {
                round(d.total_op_return_bytes as f64 / total_size * 100.0, 2)
            } else {
                0.0
            }
        })
        .collect();
    let insc_vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total_size = d.avg_size * d.block_count as f64;
            if total_size > 0.0 {
                round(
                    d.avg_inscription_bytes * d.block_count as f64 / total_size
                        * 100.0,
                    2,
                )
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "OP_RETURN", "type": "line", "data": op_vals,
                "stack": "embed", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": OPRETURN_COLOR },
                "itemStyle": { "color": OPRETURN_COLOR }, "symbol": "none"
            },
            {
                "name": "Inscriptions", "type": "line", "data": insc_vals,
                "stack": "embed", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0, "color": INSCRIPTION_COLOR },
                "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none"
            }
        ]
    }))
}

const STAMPS_COLOR: &str = "#94a3b8"; // Slate gray for Stamps/multisig
const BRC20_COLOR: &str = "#e879f9"; // Fuchsia for BRC-20 tokens

/// Unified embedded data count — all protocols + inscriptions + stamps (per-block).
pub fn unified_embedded_count_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("All Embedded Data Count");
    }

    let runes: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.runes_count)).collect();
    let omni: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.omni_count)).collect();
    let xcp: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.counterparty_count)).collect();
    let other_op: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.data_carrier_count)).collect();
    // BRC-20 is a subset of inscriptions — split them to avoid double-counting
    let inscriptions: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, b.inscription_count.saturating_sub(b.brc20_count)))
        .collect();
    let brc20: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.brc20_count)).collect();
    // Stamps launched ~block 783,000 (March 2023). Before that, multisig was legitimate multi-sig.
    let stamps: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, if b.height >= 783_000 { b.multisig_count } else { 0 }))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other OP_RETURN", "type": "bar", "stack": "total", "data": other_op, "itemStyle": { "color": CARRIER_COLOR } },
            { "name": "Inscriptions", "type": "bar", "stack": "total", "data": inscriptions, "itemStyle": { "color": INSCRIPTION_COLOR } },
            { "name": "BRC-20", "type": "bar", "stack": "total", "data": brc20, "itemStyle": { "color": BRC20_COLOR } },
            { "name": "Stamps (multisig)", "type": "bar", "stack": "total", "data": stamps, "itemStyle": { "color": STAMPS_COLOR } }
        ]
    }))
}

/// Unified embedded data count (daily).
pub fn unified_embedded_count_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("All Embedded Data Count");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let avg = |total: u64, bc: u64| -> f64 {
        if bc > 0 {
            round(total as f64 / bc as f64, 1)
        } else {
            0.0
        }
    };
    let runes: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_runes_count, d.block_count))
        .collect();
    let omni: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_omni_count, d.block_count))
        .collect();
    let xcp: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_counterparty_count, d.block_count))
        .collect();
    let other_op: Vec<f64> = days
        .iter()
        .map(|d| avg(d.total_data_carrier_count, d.block_count))
        .collect();
    // BRC-20 is a subset of inscriptions — split them to avoid double-counting
    let inscriptions: Vec<f64> = days
        .iter()
        .map(|d| round((d.avg_inscription_count - d.avg_brc20_count).max(0.0), 1))
        .collect();
    let brc20: Vec<f64> =
        days.iter().map(|d| round(d.avg_brc20_count, 1)).collect();
    // Stamps launched March 2023. Before that, multisig was legitimate multi-sig.
    let stamps: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.date.as_str() >= "2023-03-01" {
                round(d.avg_multisig_count, 1)
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other OP_RETURN", "type": "bar", "stack": "total", "data": other_op, "itemStyle": { "color": CARRIER_COLOR } },
            { "name": "Inscriptions", "type": "bar", "stack": "total", "data": inscriptions, "itemStyle": { "color": INSCRIPTION_COLOR } },
            { "name": "BRC-20", "type": "bar", "stack": "total", "data": brc20, "itemStyle": { "color": BRC20_COLOR } },
            { "name": "Stamps (multisig)", "type": "bar", "stack": "total", "data": stamps, "itemStyle": { "color": STAMPS_COLOR } }
        ]
    }))
}

/// Unified embedded data volume — all protocols by bytes (per-block).
pub fn unified_embedded_volume_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("All Embedded Data Volume");
    }

    let runes: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.runes_bytes)).collect();
    let omni: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.omni_bytes)).collect();
    let xcp: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.counterparty_bytes)).collect();
    let other_op: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.data_carrier_bytes)).collect();
    let inscriptions: Vec<serde_json::Value> =
        blocks.iter().map(|b| dp(b, b.inscription_bytes)).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other OP_RETURN", "type": "bar", "stack": "total", "data": other_op, "itemStyle": { "color": CARRIER_COLOR } },
            { "name": "Inscriptions", "type": "bar", "stack": "total", "data": inscriptions, "itemStyle": { "color": INSCRIPTION_COLOR } }
        ]
    }))
}

/// Unified embedded data volume (daily).
pub fn unified_embedded_volume_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("All Embedded Data Volume");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let avg_kb = |total: u64, bc: u64| -> f64 {
        if bc > 0 {
            round(total as f64 / bc as f64 / 1000.0, 1)
        } else {
            0.0
        }
    };
    let runes: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_runes_bytes, d.block_count))
        .collect();
    let omni: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_omni_bytes, d.block_count))
        .collect();
    let xcp: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_counterparty_bytes, d.block_count))
        .collect();
    let other_op: Vec<f64> = days
        .iter()
        .map(|d| avg_kb(d.total_data_carrier_bytes, d.block_count))
        .collect();
    let inscriptions: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_inscription_bytes / 1000.0, 1))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("KB/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other OP_RETURN", "type": "bar", "stack": "total", "data": other_op, "itemStyle": { "color": CARRIER_COLOR } },
            { "name": "Inscriptions", "type": "bar", "stack": "total", "data": inscriptions, "itemStyle": { "color": INSCRIPTION_COLOR } }
        ]
    }))
}
