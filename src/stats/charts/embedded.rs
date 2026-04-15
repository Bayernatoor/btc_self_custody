//! Embedded data chart builders: OP_RETURN count and volume by protocol (Runes,
//! Omni, Counterparty, other), OP_RETURN protocol share, OP_RETURN block share,
//! Ordinals inscriptions, inscription block share, combined embedded data overview
//! (share, count, volume), and Stamps.

use super::*;
use serde_json::json;
use std::fmt::Write;

const INSCRIPTION_COLOR: &str = "#06b6d4"; // Cyan for inscriptions

/// OP_RETURN count bar chart (runes vs data carriers).
pub fn op_return_count_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Count");
    }

    let runes_str = build_data_array_i64(blocks, |b| b.runes_count as i64);
    let runes = data_array_value(&runes_str);
    let omni_str = build_data_array_i64(blocks, |b| b.omni_count as i64);
    let omni = data_array_value(&omni_str);
    let xcp_str = build_data_array_i64(blocks, |b| b.counterparty_count as i64);
    let xcp = data_array_value(&xcp_str);
    let other_str = build_data_array_i64(blocks, |b| b.data_carrier_count as i64);
    let other = data_array_value(&other_str);

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
pub fn op_return_count_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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

    let runes_str = build_data_array_i64(blocks, |b| b.runes_bytes as i64);
    let runes = data_array_value(&runes_str);
    let omni_str = build_data_array_i64(blocks, |b| b.omni_bytes as i64);
    let omni = data_array_value(&omni_str);
    let xcp_str = build_data_array_i64(blocks, |b| b.counterparty_bytes as i64);
    let xcp = data_array_value(&xcp_str);
    let other_str = build_data_array_i64(blocks, |b| b.data_carrier_bytes as i64);
    let other = data_array_value(&other_str);

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
pub fn op_return_bytes_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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

    let runes_str = build_data_array_f64(blocks, |b| pct(b.runes_count, b.op_return_count));
    let runes_data = data_array_value(&runes_str);
    let omni_str = build_data_array_f64(blocks, |b| pct(b.omni_count, b.op_return_count));
    let omni_data = data_array_value(&omni_str);
    let xcp_str = build_data_array_f64(blocks, |b| pct(b.counterparty_count, b.op_return_count));
    let xcp_data = data_array_value(&xcp_str);
    let other_str = build_data_array_f64(blocks, |b| pct(b.data_carrier_count, b.op_return_count));
    let other_data = data_array_value(&other_str);

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
pub fn op_return_block_share_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Block Share");
    }

    let share_fn = |b: &BlockSummary| {
        if b.size > 0 {
            ((b.op_return_bytes as f64 / b.size as f64 * 100.0 * 100.0)
                .round()
                / 100.0)
                .min(100.0)
        } else {
            0.0
        }
    };
    let raw_str = build_data_array_f64(blocks, share_fn);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(share_fn).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

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
pub fn op_return_block_share_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Embedded Data Block Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total_size = d.avg_size * d.block_count as f64;
            if total_size > 0.0 {
                ((d.total_op_return_bytes as f64 / total_size * 100.0 * 100.0)
                    .round()
                    / 100.0)
                    .min(100.0)
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

    let raw_str = build_data_array_f64(blocks, |b| b.inscription_count as f64);
    let raw = data_array_value(&raw_str);
    let vals: Vec<f64> = blocks.iter().map(|b| b.inscription_count as f64).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);
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

    // Use envelope bytes (full witness item size) for accurate block share
    let insc_fn = |b: &BlockSummary| {
        if b.size > 0 {
            let bytes = if b.inscription_envelope_bytes > 0 {
                b.inscription_envelope_bytes
            } else {
                b.inscription_bytes // fallback for pre-backfill blocks
            };
            ((bytes as f64 / b.size as f64 * 100.0 * 100.0)
                .round()
                / 100.0)
                .min(100.0)
        } else {
            0.0
        }
    };
    let raw_str = build_data_array_f64(blocks, insc_fn);
    let raw = data_array_value(&raw_str);
    let vals: Vec<f64> = blocks.iter().map(insc_fn).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);
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
pub fn inscription_share_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Inscription Block Share");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    // Use envelope bytes when available for accurate block share
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_size > 0.0 {
                let bytes = if d.avg_inscription_envelope_bytes > 0.0 {
                    d.avg_inscription_envelope_bytes
                } else {
                    d.avg_inscription_bytes
                };
                ((bytes / d.avg_size * 100.0 * 100.0).round()
                    / 100.0)
                    .min(100.0)
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

    let op_str = build_data_array_f64(blocks, |b| {
        if b.size > 0 {
            round(b.op_return_bytes as f64 / b.size as f64 * 100.0, 2)
        } else {
            0.0
        }
    });
    let op_data = data_array_value(&op_str);
    // Inscriptions launched ~block 774,000 (Jan 2023). Emit null before that to avoid
    // ECharts rendering a ghost area fill at the zero baseline across years of no data.
    let mut insc_buf = String::with_capacity(blocks.len() * 30);
    insc_buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 { insc_buf.push(','); }
        if b.height < 774_000 {
            let _ = write!(insc_buf, "[{},null,{}]", ts_ms(b.timestamp), b.height);
        } else if b.size > 0 {
            let _ = write!(insc_buf, "[{},{},{}]", ts_ms(b.timestamp),
                round(b.inscription_bytes as f64 / b.size as f64 * 100.0, 2), b.height);
        } else {
            let _ = write!(insc_buf, "[{},0,{}]", ts_ms(b.timestamp), b.height);
        }
    }
    insc_buf.push(']');
    let insc_data = data_array_value(&insc_buf);

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
                "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none",
                "connectNulls": false
            }
        ]
    }))
}

/// All embedded data as % of block size — stacked (daily).
pub fn all_embedded_share_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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
    // Emit null for inscription values before Jan 2023 to avoid ghost area fill
    let insc_vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.date.as_str() < "2023-01-01" {
                json!(null)
            } else {
                let total_size = d.avg_size * d.block_count as f64;
                if total_size > 0.0 {
                    json!(round(
                        d.avg_inscription_bytes * d.block_count as f64
                            / total_size
                            * 100.0,
                        2,
                    ))
                } else {
                    json!(0.0)
                }
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
                "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none",
                "connectNulls": false
            }
        ]
    }))
}

const STAMPS_COLOR: &str = "#94a3b8"; // Slate gray for Stamps/multisig
const BRC20_COLOR: &str = "#e879f9"; // Fuchsia for BRC-20 tokens

/// Unified embedded data count — all protocols + inscriptions + stamps (per-block).
pub fn unified_embedded_count_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("All Embedded Data Count");
    }

    let runes_str = build_data_array_i64(blocks, |b| b.runes_count as i64);
    let runes = data_array_value(&runes_str);
    let omni_str = build_data_array_i64(blocks, |b| b.omni_count as i64);
    let omni = data_array_value(&omni_str);
    let xcp_str = build_data_array_i64(blocks, |b| b.counterparty_count as i64);
    let xcp = data_array_value(&xcp_str);
    let other_op_str = build_data_array_i64(blocks, |b| b.data_carrier_count as i64);
    let other_op = data_array_value(&other_op_str);
    // BRC-20 is a subset of inscriptions -- split them to avoid double-counting
    let inscriptions_str = build_data_array_i64(blocks, |b| b.inscription_count.saturating_sub(b.brc20_count) as i64);
    let inscriptions = data_array_value(&inscriptions_str);
    let brc20_str = build_data_array_i64(blocks, |b| b.brc20_count as i64);
    let brc20 = data_array_value(&brc20_str);
    // Stamps removed — detection requires Counterparty protocol decoding (TODO)

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
            { "name": "BRC-20", "type": "bar", "stack": "total", "data": brc20, "itemStyle": { "color": BRC20_COLOR } }
        ]
    }))
}

/// Unified embedded data count (daily).
pub fn unified_embedded_count_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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
        .map(|d| {
            round((d.avg_inscription_count - d.avg_brc20_count).max(0.0), 1)
        })
        .collect();
    let brc20: Vec<f64> =
        days.iter().map(|d| round(d.avg_brc20_count, 1)).collect();
    // Stamps removed — detection requires Counterparty protocol decoding (TODO)

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
            { "name": "BRC-20", "type": "bar", "stack": "total", "data": brc20, "itemStyle": { "color": BRC20_COLOR } }
        ]
    }))
}

/// Unified embedded data volume — all protocols by bytes (per-block).
pub fn unified_embedded_volume_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("All Embedded Data Volume");
    }

    let runes_str = build_data_array_i64(blocks, |b| b.runes_bytes as i64);
    let runes = data_array_value(&runes_str);
    let omni_str = build_data_array_i64(blocks, |b| b.omni_bytes as i64);
    let omni = data_array_value(&omni_str);
    let xcp_str = build_data_array_i64(blocks, |b| b.counterparty_bytes as i64);
    let xcp = data_array_value(&xcp_str);
    let other_op_str = build_data_array_i64(blocks, |b| b.data_carrier_bytes as i64);
    let other_op = data_array_value(&other_op_str);
    let inscriptions_str = build_data_array_i64(blocks, |b| b.inscription_bytes as i64);
    let inscriptions = data_array_value(&inscriptions_str);

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
pub fn unified_embedded_volume_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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

/// Stamps output count over time (per-block).
/// Requires backfill v9 for historical data.
pub fn stamps_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Stamps");
    }

    let has_data = blocks.iter().any(|b| b.stamps_count > 0);
    if !has_data {
        return no_data_chart("Stamps");
    }

    let raw_str = build_data_array_f64(blocks, |b| b.stamps_count as f64);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| b.stamps_count as f64).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Stamps", "type": "line", "data": raw,
        "areaStyle": { "color": STAMPS_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": STAMPS_COLOR },
        "itemStyle": { "color": STAMPS_COLOR }, "symbol": "none",
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

/// Stamps count (daily).
pub fn stamps_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Stamps");
    }
    let has_data = days.iter().any(|d| d.avg_stamps_count > 0.0);
    if !has_data {
        return no_data_chart("Stamps");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> =
        days.iter().map(|d| round(d.avg_stamps_count, 2)).collect();
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
        "yAxis": y_axis("Count/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Stamps", "type": "line", "data": vals,
              "areaStyle": { "color": STAMPS_COLOR, "opacity": 0.15 },
              "lineStyle": { "width": 1, "color": STAMPS_COLOR },
              "itemStyle": { "color": STAMPS_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Inscription envelope vs payload charts (v11 backfill)
// ---------------------------------------------------------------------------

const ENVELOPE_COLOR: &str = "#f97316"; // Orange for envelope overhead
const PAYLOAD_COLOR: &str = "#06b6d4"; // Cyan for payload (matches inscription)

/// Inscription payload vs envelope bytes per block, showing overhead from
/// witness envelope structure (opcodes, push data, ord marker).
pub fn inscription_envelope_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Inscription Payload vs Envelope");
    }
    let has_data = blocks.iter().any(|b| b.inscription_envelope_bytes > 0);
    if !has_data {
        return no_data_chart("Inscription Payload vs Envelope");
    }

    let payload_str = build_data_array_f64(blocks, |b| b.inscription_bytes as f64 / 1024.0);
    let envelope_str = build_data_array_f64(blocks, |b| {
        b.inscription_envelope_bytes.saturating_sub(b.inscription_bytes) as f64 / 1024.0
    });

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("KB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Payload", "type": "bar", "stack": "total",
              "data": data_array_value(&payload_str),
              "itemStyle": { "color": PAYLOAD_COLOR } },
            { "name": "Envelope Overhead", "type": "bar", "stack": "total",
              "data": data_array_value(&envelope_str),
              "itemStyle": { "color": ENVELOPE_COLOR } }
        ]
    }))
}

/// Inscription payload vs envelope bytes (daily averages).
pub fn inscription_envelope_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Inscription Payload vs Envelope");
    }
    let has_data = days.iter().any(|d| d.avg_inscription_envelope_bytes > 0.0);
    if !has_data {
        return no_data_chart("Inscription Payload vs Envelope");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let payload: Vec<f64> = days.iter().map(|d| round(d.avg_inscription_bytes / 1024.0, 2)).collect();
    let overhead: Vec<f64> = days.iter().map(|d| {
        let oh = d.avg_inscription_envelope_bytes - d.avg_inscription_bytes;
        round(oh.max(0.0) / 1024.0, 2)
    }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("KB/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Payload", "type": "bar", "stack": "total", "data": payload,
              "itemStyle": { "color": PAYLOAD_COLOR } },
            { "name": "Envelope Overhead", "type": "bar", "stack": "total", "data": overhead,
              "itemStyle": { "color": ENVELOPE_COLOR } }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Inscription fee share chart
// ---------------------------------------------------------------------------

/// Inscription fees as a percentage of total block fees.
pub fn inscription_fee_share_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Inscription Fee Share");
    }
    let has_data = blocks.iter().any(|b| b.inscription_fees > 0);
    if !has_data {
        return no_data_chart("Inscription Fee Share");
    }

    let share_fn = |b: &BlockSummary| {
        if b.total_fees > 0 {
            round(b.inscription_fees as f64 / b.total_fees as f64 * 100.0, 2)
        } else {
            0.0
        }
    };
    let raw_str = build_data_array_f64(blocks, share_fn);
    let raw = data_array_value(&raw_str);
    let vals: Vec<f64> = blocks.iter().map(share_fn).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Inscription Fees %", "type": "line", "data": raw,
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
        "yAxis": y_axis("% of Fees"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Inscription fee share (daily).
pub fn inscription_fee_share_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Inscription Fee Share");
    }
    let has_data = days.iter().any(|d| d.total_inscription_fees > 0);
    if !has_data {
        return no_data_chart("Inscription Fee Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| {
        if d.total_fees > 0 {
            round(d.total_inscription_fees as f64 / d.total_fees as f64 * 100.0, 2)
        } else {
            0.0
        }
    }).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma.iter().map(|v| match v {
        Some(x) => json!(x),
        None => json!(null),
    }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Fees"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Inscription Fees %", "type": "line", "data": vals,
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
// Protocol fee competition chart
// ---------------------------------------------------------------------------

const RUNES_FEE_COLOR: &str = "#f7931a"; // Bitcoin orange for Runes fees
const OTHER_FEE_COLOR: &str = "#6366f1"; // Indigo for other fees

/// Fee revenue breakdown by protocol: Inscriptions, Runes, and standard transactions.
pub fn protocol_fee_competition_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Protocol Fee Competition");
    }
    let has_data = blocks.iter().any(|b| b.inscription_fees > 0 || b.runes_fees > 0);
    if !has_data {
        return no_data_chart("Protocol Fee Competition");
    }

    let insc_str = build_data_array_f64(blocks, |b| b.inscription_fees as f64 / 100_000_000.0);
    let runes_str = build_data_array_f64(blocks, |b| b.runes_fees as f64 / 100_000_000.0);
    let other_str = build_data_array_f64(blocks, |b| {
        let other = b.total_fees.saturating_sub(b.inscription_fees).saturating_sub(b.runes_fees);
        other as f64 / 100_000_000.0
    });

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Other", "type": "bar", "stack": "fees",
              "data": data_array_value(&other_str),
              "itemStyle": { "color": OTHER_FEE_COLOR } },
            { "name": "Inscriptions", "type": "bar", "stack": "fees",
              "data": data_array_value(&insc_str),
              "itemStyle": { "color": INSCRIPTION_COLOR } },
            { "name": "Runes", "type": "bar", "stack": "fees",
              "data": data_array_value(&runes_str),
              "itemStyle": { "color": RUNES_FEE_COLOR } }
        ]
    }))
}

/// Protocol fee competition (daily totals).
pub fn protocol_fee_competition_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Protocol Fee Competition");
    }
    let has_data = days.iter().any(|d| d.total_inscription_fees > 0 || d.total_runes_fees > 0);
    if !has_data {
        return no_data_chart("Protocol Fee Competition");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let insc: Vec<f64> = days.iter().map(|d| round(d.total_inscription_fees as f64 / 100_000_000.0, 4)).collect();
    let runes: Vec<f64> = days.iter().map(|d| round(d.total_runes_fees as f64 / 100_000_000.0, 4)).collect();
    let other: Vec<f64> = days.iter().map(|d| {
        let o = d.total_fees.saturating_sub(d.total_inscription_fees).saturating_sub(d.total_runes_fees);
        round(o as f64 / 100_000_000.0, 4)
    }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Other", "type": "bar", "stack": "fees", "data": other,
              "itemStyle": { "color": OTHER_FEE_COLOR } },
            { "name": "Inscriptions", "type": "bar", "stack": "fees", "data": insc,
              "itemStyle": { "color": INSCRIPTION_COLOR } },
            { "name": "Runes", "type": "bar", "stack": "fees", "data": runes,
              "itemStyle": { "color": RUNES_FEE_COLOR } }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Coinbase message charts
// ---------------------------------------------------------------------------

/// Average coinbase message length per block (bytes of decoded ASCII text).
/// Longer messages often indicate pools encoding custom data, timestamps, or
/// signaling information in the coinbase.
pub fn coinbase_message_length_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Coinbase Message Length");
    }
    let has_data = blocks.iter().any(|b| !b.coinbase_text.is_empty());
    if !has_data {
        return no_data_chart("Coinbase Message Length");
    }

    let len_fn = |b: &BlockSummary| b.coinbase_text.len() as f64;
    let raw_str = build_data_array_f64(blocks, len_fn);
    let raw = data_array_value(&raw_str);
    let vals: Vec<f64> = blocks.iter().map(len_fn).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Message Length", "type": "line", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": "#a78bfa" },
        "itemStyle": { "color": "#a78bfa" }, "symbol": "none",
        "opacity": if has_ma { 0.3 } else { 1.0 }
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
        "yAxis": y_axis("Characters"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Coinbase message length chart (daily - no daily aggregate available,
/// so return no_data for daily ranges).
pub fn coinbase_message_length_chart_daily(
    _days: &[DailyAggregate],
) -> serde_json::Value {
    no_data_chart("Coinbase Message Length")
}
