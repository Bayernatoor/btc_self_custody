//! Fee chart builders: total fees, avg fee per tx, median fee rate, fee rate
//! bands (p10/p50/p90), and subsidy-vs-fees stacked area.

use super::*;
use serde_json::json;
use std::fmt::Write;

/// Fees line chart (per-block: total fees in sats).
pub fn fees_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fees");
    }

    let mut raw_buf = String::with_capacity(blocks.len() * 30);
    raw_buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 { raw_buf.push(','); }
        if b.total_fees > 0 {
            let _ = write!(raw_buf, "[{},{},{}]", ts_ms(b.timestamp), b.total_fees, b.height);
        } else {
            let _ = write!(raw_buf, "[{},null]", ts_ms(b.timestamp));
        }
    }
    raw_buf.push(']');
    let raw = data_array_value(&raw_buf);

    build_option(json!({
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
pub fn fees_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
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

/// Fees line chart with unit toggle (sats or BTC).
pub fn fees_chart_unit(
    blocks: &[BlockSummary],
    unit: &str,
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fees");
    }

    let divisor = if unit == "btc" { 100_000_000.0 } else { 1.0 };
    let y_name = if unit == "btc" { "BTC" } else { "sats" };

    let mut raw_buf = String::with_capacity(blocks.len() * 30);
    raw_buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 { raw_buf.push(','); }
        if b.total_fees > 0 {
            let v = (b.total_fees as f64 / divisor * 1000.0).round() / 1000.0;
            let _ = write!(raw_buf, "[{},{},{}]", ts_ms(b.timestamp), v, b.height);
        } else {
            let _ = write!(raw_buf, "[{},null]", ts_ms(b.timestamp));
        }
    }
    raw_buf.push(']');
    let raw = data_array_value(&raw_buf);

    build_option(json!({
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
pub fn fees_chart_daily_unit(
    days: &[DailyAggregate],
    unit: &str,
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Fees");
    }

    let divisor = if unit == "btc" { 100_000_000.0 } else { 1.0 };
    let y_name = if unit == "btc" { "BTC" } else { "sats" };

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

/// Average fee per transaction (per-block: total_fees / user_tx_count).
pub fn avg_fee_per_tx_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Avg Fee per Tx");
    }

    let fee_fn = |b: &BlockSummary| {
        let user_tx = b.tx_count.saturating_sub(1); // exclude coinbase
        if user_tx > 0 {
            b.total_fees as f64 / user_tx as f64
        } else {
            0.0
        }
    };
    let raw_str = build_data_array_f64(blocks, fee_fn);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| fee_fn(b)).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Fee/Tx", "type": "line", "data": raw,
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
        "yAxis": y_axis("sats"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Average fee per transaction (daily).
pub fn avg_fee_per_tx_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Avg Fee per Tx");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let user_tx =
                (d.avg_tx_count * d.block_count as f64) - d.block_count as f64;
            if user_tx > 0.0 && d.total_fees > 0 {
                d.total_fees as f64 / user_tx
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
        "yAxis": y_axis("sats"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Fee/Tx", "type": "line", "data": vals,
              "lineStyle": { "width": 1, "color": DATA_COLOR },
              "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Median fee rate over time (per-block).
pub fn median_fee_rate_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Median Fee Rate");
    }

    let rate_fn = |b: &BlockSummary| (b.median_fee_rate * 100.0).round() / 100.0;
    let raw_str = build_data_array_f64(blocks, rate_fn);
    let raw = data_array_value(&raw_str);

    let vals: Vec<f64> = blocks.iter().map(|b| rate_fn(b)).collect();
    let ma = moving_average(&vals, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Median Rate", "type": "line", "data": raw,
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
        "yAxis": y_axis("sat/vB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Median fee rate over time (daily).
/// DailyAggregate doesn't store median_fee_rate directly, so we approximate
/// using total_fees / total_tx / avg_vsize. This is the average fee rate,
/// not the true median, but it's a reasonable proxy for daily granularity.
pub fn median_fee_rate_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Avg Fee Rate");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    // Approximate: total_fees / (total_tx * avg_vsize_per_tx)
    // avg_vsize ≈ avg_size * 0.75 (SegWit discount estimate)
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total_tx = d.avg_tx_count * d.block_count as f64;
            let avg_vsize = d.avg_size * 0.75; // approximate vsize from size
            if total_tx > 1.0 && avg_vsize > 0.0 {
                let avg_fee_per_tx = d.total_fees as f64 / total_tx;
                let avg_tx_vsize = avg_vsize / d.avg_tx_count;
                let rate = avg_fee_per_tx / avg_tx_vsize;
                (rate * 100.0).round() / 100.0
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
        "yAxis": y_axis("sat/vB (approx)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Avg Fee Rate", "type": "line", "data": vals,
              "lineStyle": { "width": 1, "color": DATA_COLOR },
              "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Fee rate percentile band (per-block: p10, median, p90).
/// Requires backfill v9 for p10/p90 data.
pub fn fee_rate_band_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fee Rate Band");
    }

    // Check if p10/p90 data is available (non-zero)
    let has_percentiles = blocks
        .iter()
        .any(|b| b.fee_rate_p10 > 0.0 || b.fee_rate_p90 > 0.0);
    if !has_percentiles {
        return no_data_chart("Fee Rate Band");
    }

    let p10_str = build_data_array_f64(blocks, |b| (b.fee_rate_p10 * 100.0).round() / 100.0);
    let p10_data = data_array_value(&p10_str);

    let median_str = build_data_array_f64(blocks, |b| (b.median_fee_rate * 100.0).round() / 100.0);
    let median_data = data_array_value(&median_str);

    let p90_str = build_data_array_f64(blocks, |b| (b.fee_rate_p90 * 100.0).round() / 100.0);
    let p90_data = data_array_value(&p90_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("sat/vB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "90th Percentile", "type": "line", "data": p90_data,
                "lineStyle": { "width": 1, "color": TARGET_COLOR, "opacity": 0.6 },
                "itemStyle": { "color": TARGET_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(231,76,60,0.08)" }
            },
            {
                "name": "Median", "type": "line", "data": median_data,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
            },
            {
                "name": "10th Percentile", "type": "line", "data": p10_data,
                "lineStyle": { "width": 1, "color": SIGNAL_YES, "opacity": 0.6 },
                "itemStyle": { "color": SIGNAL_YES }, "symbol": "none",
                "areaStyle": { "color": "rgba(46,204,113,0.08)" }
            }
        ]
    }))
}

/// Fee rate percentile band (daily).
pub fn fee_rate_band_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Fee Rate Band");
    }
    let has_data = days
        .iter()
        .any(|d| d.avg_fee_rate_p10 > 0.0 || d.avg_fee_rate_p90 > 0.0);
    if !has_data {
        return no_data_chart("Fee Rate Band");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let p10: Vec<f64> = days
        .iter()
        .map(|d| (d.avg_fee_rate_p10 * 100.0).round() / 100.0)
        .collect();
    let median: Vec<f64> = days
        .iter()
        .map(|d| (d.avg_median_fee_rate * 100.0).round() / 100.0)
        .collect();
    let p90: Vec<f64> = days
        .iter()
        .map(|d| (d.avg_fee_rate_p90 * 100.0).round() / 100.0)
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("sat/vB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "90th Percentile", "type": "line", "data": p90,
              "lineStyle": { "width": 1, "color": TARGET_COLOR, "opacity": 0.6 },
              "itemStyle": { "color": TARGET_COLOR }, "symbol": "none",
              "areaStyle": { "color": "rgba(231,76,60,0.08)" } },
            { "name": "Median", "type": "line", "data": median,
              "lineStyle": { "width": 2, "color": DATA_COLOR },
              "itemStyle": { "color": DATA_COLOR }, "symbol": "none" },
            { "name": "10th Percentile", "type": "line", "data": p10,
              "lineStyle": { "width": 1, "color": SIGNAL_YES, "opacity": 0.6 },
              "itemStyle": { "color": SIGNAL_YES }, "symbol": "none",
              "areaStyle": { "color": "rgba(46,204,113,0.08)" } }
        ]
    }))
}

/// Block subsidy vs fee revenue ratio (stacked area).
pub fn subsidy_vs_fees_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Subsidy vs Fees");
    }

    let subsidy_str = build_data_array_f64(blocks, |b| {
        let sub = block_subsidy(b.height) as f64 / 100_000_000.0;
        (sub * 1000.0).round() / 1000.0
    });
    let subsidy_data = data_array_value(&subsidy_str);

    let fee_str = build_data_array_f64(blocks, |b| {
        let fee = b.total_fees as f64 / 100_000_000.0;
        (fee * 1000.0).round() / 1000.0
    });
    let fee_data = data_array_value(&fee_str);

    build_option(json!({
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
pub fn subsidy_vs_fees_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
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

/// Fee revenue as percentage of total block reward (subsidy + fees).
/// Shows the long-term transition from subsidy-era to fee-era mining.
pub fn fee_revenue_share_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fee Revenue Share");
    }

    let share_fn = |b: &BlockSummary| {
        let subsidy = block_subsidy(b.height) as f64;
        let fees = b.total_fees as f64;
        if subsidy + fees > 0.0 {
            round(fees / (subsidy + fees) * 100.0, 2)
        } else {
            0.0
        }
    };
    let data_str = build_data_array_f64(blocks, share_fn);
    let data = data_array_value(&data_str);

    let raw_data: Vec<f64> = blocks.iter().map(|b| share_fn(b)).collect();
    let ma = moving_average(&raw_data, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let mut series = vec![json!({
        "name": "Fee Share", "type": "line", "data": data,
        "lineStyle": { "width": 1, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "areaStyle": { "color": DATA_COLOR_FADED }
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
        "yAxis": y_axis("Fee Share (%)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": series
    }))
}

/// Fee revenue share from daily aggregates.
pub fn fee_revenue_share_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Fee Revenue Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    let data: Vec<f64> = days
        .iter()
        .map(|d| {
            let subsidy_per_block = if d.date.as_str() >= "2024-04-20" {
                3.125
            } else if d.date.as_str() >= "2020-05-11" {
                6.25
            } else if d.date.as_str() >= "2016-07-09" {
                12.5
            } else if d.date.as_str() >= "2012-11-28" {
                25.0
            } else {
                50.0
            };
            let total_subsidy = subsidy_per_block * d.block_count as f64 * 100_000_000.0;
            let fees = d.total_fees as f64;
            if total_subsidy + fees > 0.0 {
                round(fees / (total_subsidy + fees) * 100.0, 2)
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Fee Share (%)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "Fee Revenue %", "type": "line", "data": data,
            "lineStyle": { "width": 1.5, "color": DATA_COLOR },
            "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
            "areaStyle": { "color": DATA_COLOR_FADED }
        }]
    }))
}

/// Total BTC transferred per block (non-coinbase output value).
pub fn btc_volume_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("BTC Transferred Volume");
    }

    let vol_fn = |b: &BlockSummary| round(b.total_output_value as f64 / 100_000_000.0, 2);
    let data_str = build_data_array_f64(blocks, vol_fn);
    let data = data_array_value(&data_str);

    let raw: Vec<f64> = blocks.iter().map(|b| vol_fn(b)).collect();
    let ma = moving_average(&raw, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let mut series = vec![json!({
        "name": "BTC Volume", "type": "bar", "data": data,
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
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": series
    }))
}

/// BTC transferred volume from daily aggregates.
pub fn btc_volume_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("BTC Transferred Volume");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let data: Vec<f64> = days
        .iter()
        .map(|d| round(d.total_output_value as f64 / 100_000_000.0, 2))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "BTC Volume", "type": "bar", "data": data,
            "itemStyle": { "color": DATA_COLOR }
        }]
    }))
}

/// Fee pressure scatter: weight utilization % vs median fee rate.
/// Each point represents one block, with height available in tooltip.
pub fn fee_pressure_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fee Pressure");
    }

    let data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let weight_util_pct = b.weight as f64 / 4_000_000.0 * 100.0;
            let median_fee_rate = (b.median_fee_rate * 100.0).round() / 100.0;
            json!([round(weight_util_pct, 2), median_fee_rate, b.height])
        })
        .collect();

    build_option(json!({
        "xAxis": {
            "type": "value",
            "name": "Weight Utilization (%)",
            "nameLocation": "center",
            "nameGap": 30,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "yAxis": y_axis("Median Fee Rate (sat/vB)"),
        "dataZoom": data_zoom(),
        "tooltip": {
            "trigger": "item",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 },
            "_noTimeFormat": true
        },
        "series": [
            {
                "name": "Fee Pressure", "type": "scatter", "data": data,
                "symbolSize": 7,
                "itemStyle": { "color": DATA_COLOR, "opacity": 0.7 }
            }
        ]
    }))
}

/// Input vs output value flow per block. The gap between the two lines is fees.
pub fn value_flow_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Value Flow");
    }

    let input_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, round(b.total_input_value as f64 / 100_000_000.0, 3)))
        .collect();

    let output_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, round(b.total_output_value as f64 / 100_000_000.0, 3)))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Input Value", "type": "line", "data": input_data,
                "lineStyle": { "width": 1, "color": "#ef4444" },
                "itemStyle": { "color": "#ef4444" }, "symbol": "none",
                "areaStyle": { "color": "rgba(239,68,68,0.08)" }
            },
            {
                "name": "Output Value", "type": "line", "data": output_data,
                "lineStyle": { "width": 1, "color": "#22c55e" },
                "itemStyle": { "color": "#22c55e" }, "symbol": "none",
                "areaStyle": { "color": "rgba(34,197,94,0.08)" }
            }
        ]
    }))
}

/// Input vs output value flow from daily aggregates.
pub fn value_flow_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Value Flow");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let input_data: Vec<f64> = days
        .iter()
        .map(|d| round(d.total_input_value as f64 / 100_000_000.0, 2))
        .collect();
    let output_data: Vec<f64> = days
        .iter()
        .map(|d| round(d.total_output_value as f64 / 100_000_000.0, 2))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Input Value", "type": "line", "data": input_data,
                "lineStyle": { "width": 1, "color": "#ef4444" },
                "itemStyle": { "color": "#ef4444" }, "symbol": "none",
                "areaStyle": { "color": "rgba(239,68,68,0.08)" }
            },
            {
                "name": "Output Value", "type": "line", "data": output_data,
                "lineStyle": { "width": 1, "color": "#22c55e" },
                "itemStyle": { "color": "#22c55e" }, "symbol": "none",
                "areaStyle": { "color": "rgba(34,197,94,0.08)" }
            }
        ]
    }))
}

/// Fee spike detector: scatter plot of fee rate spikes (>5x the 144-block trailing
/// average) overlaid on the trailing average line.
pub fn fee_spike_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fee Spike Detector");
    }
    if blocks.len() < 300 {
        return no_data_chart_with_hint("Fee Spike Detector", "Select a longer range (1W+) for enough data to detect fee spikes");
    }

    let rates: Vec<f64> = blocks.iter().map(|b| round(b.median_fee_rate, 2)).collect();
    let ma = moving_average(&rates, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    // Build spike scatter: only points where rate > 5x trailing average
    let mut spike_buf = String::with_capacity(blocks.len() * 10);
    spike_buf.push('[');
    let mut first = true;
    for (i, b) in blocks.iter().enumerate() {
        if let Some(avg) = ma[i] {
            if avg > 0.0 && rates[i] > avg * 5.0 {
                if !first { spike_buf.push(','); }
                first = false;
                let _ = write!(spike_buf, "[{},{},{}]", ts_ms(b.timestamp), rates[i], b.height);
            }
        }
    }
    spike_buf.push(']');
    let spike_data = data_array_value(&spike_buf);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("sat/vB"),
        "dataZoom": data_zoom(),
        "tooltip": {
            "trigger": "item",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "legend": { "show": true },
        "series": [
            {
                "name": "144-block Avg", "type": "line", "data": ma_data,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
            },
            {
                "name": "Spike (>5x avg)", "type": "scatter", "data": spike_data,
                "symbolSize": 6,
                "itemStyle": { "color": TARGET_COLOR }
            }
        ]
    }))
}

/// Halving era comparison: grouped bar chart comparing average block size, tx count,
/// total fees (BTC), and fee revenue percentage across halving eras.
pub fn halving_era_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Halving Era Comparison");
    }

    // Group blocks by halving era (210,000 blocks each)
    let mut era_data: std::collections::BTreeMap<u64, (f64, f64, f64, f64, u64)> =
        std::collections::BTreeMap::new();

    for b in blocks {
        let era = b.height / 210_000;
        let entry = era_data.entry(era).or_insert((0.0, 0.0, 0.0, 0.0, 0));
        entry.0 += b.size as f64 / 1_000_000.0;     // size in MB
        entry.1 += b.tx_count as f64;                // tx count
        entry.2 += b.total_fees as f64 / 100_000_000.0; // fees in BTC
        let subsidy = block_subsidy(b.height) as f64;
        let fees = b.total_fees as f64;
        if subsidy + fees > 0.0 {
            entry.3 += fees / (subsidy + fees) * 100.0; // fee revenue %
        }
        entry.4 += 1;
    }

    let subsidy_labels = ["50 BTC", "25 BTC", "12.5 BTC", "6.25 BTC", "3.125 BTC"];
    let era_colors = ["#fbbf24", "#f59e0b", "#d97706", "#b45309", "#92400e"];
    let metrics = ["Avg Size (MB)", "Avg Tx Count", "Avg Fees (BTC)", "Fee Revenue %"];

    let eras: Vec<u64> = era_data.keys().copied().collect();

    // Need at least 2 eras to compare
    if eras.len() < 2 {
        return no_data_chart_with_hint("Halving Era Comparison", "Select a range spanning multiple halving eras (try ALL range)");
    }

    // Compute per-era averages for each metric
    let mut era_avgs: Vec<[f64; 4]> = Vec::new();
    for &era in &eras {
        let (size_sum, tx_sum, fee_sum, pct_sum, count) = era_data[&era];
        let c = count as f64;
        era_avgs.push([
            round(size_sum / c, 3),
            round(tx_sum / c, 1),
            round(fee_sum / c, 4),
            round(pct_sum / c, 2),
        ]);
    }

    // Normalize each metric to 0-100% relative to its max across eras.
    // Tooltip shows the actual value.
    let mut maxes = [0.0f64; 4];
    for avgs in &era_avgs {
        for i in 0..4 {
            if avgs[i] > maxes[i] { maxes[i] = avgs[i]; }
        }
    }

    // Build one series per era. Each data point is [normalized%, rawValue]
    // so the tooltip can show actual values while bars are scaled.
    let units = ["MB", "txs", "BTC", "%"];
    let mut series = Vec::new();
    for (ei, &era) in eras.iter().enumerate() {
        let data: Vec<serde_json::Value> = (0..4).map(|i| {
            let norm = if maxes[i] > 0.0 { round(era_avgs[ei][i] / maxes[i] * 100.0, 1) } else { 0.0 };
            let raw = era_avgs[ei][i];
            json!({ "value": norm, "_raw": raw, "_unit": units[i] })
        }).collect();
        let label = if (era as usize) < subsidy_labels.len() {
            format!("Era {} ({})", era, subsidy_labels[era as usize])
        } else {
            format!("Era {}", era)
        };
        let color = era_colors.get(era as usize).unwrap_or(&"#78350f");
        series.push(json!({
            "name": label,
            "type": "bar",
            "data": data,
            "itemStyle": { "color": color }
        }));
    }

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": metrics,
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": y_axis("% of peak"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 },
            "_useRawValues": true
        },
        "legend": { "show": true },
        "series": series
    }))
}

/// Halving era comparison from daily aggregates. Uses halving dates to determine era.
pub fn halving_era_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Halving Era Comparison");
    }

    fn date_to_era(date: &str) -> u64 {
        if date >= "2024-04-20" { 4 }
        else if date >= "2020-05-11" { 3 }
        else if date >= "2016-07-09" { 2 }
        else if date >= "2012-11-28" { 1 }
        else { 0 }
    }

    let subsidy_btc = [50.0, 25.0, 12.5, 6.25, 3.125];
    let mut era_data: std::collections::BTreeMap<u64, (f64, f64, f64, f64, u64)> =
        std::collections::BTreeMap::new();

    for d in days {
        let era = date_to_era(&d.date);
        let entry = era_data.entry(era).or_insert((0.0, 0.0, 0.0, 0.0, 0));
        let bc = d.block_count as f64;
        entry.0 += d.avg_size / 1_000_000.0 * bc;
        entry.1 += d.avg_tx_count * bc;
        entry.2 += d.total_fees as f64 / 100_000_000.0;
        let sub = subsidy_btc.get(era as usize).copied().unwrap_or(3.125);
        let total_subsidy = sub * bc * 100_000_000.0;
        let fees = d.total_fees as f64;
        if total_subsidy + fees > 0.0 {
            entry.3 += fees / (total_subsidy + fees) * 100.0 * bc;
        }
        entry.4 += d.block_count;
    }

    let eras: Vec<u64> = era_data.keys().copied().collect();
    if eras.len() < 2 {
        return no_data_chart_with_hint("Halving Era Comparison", "Select a range spanning multiple halving eras (try ALL range)");
    }

    let subsidy_labels = ["50 BTC", "25 BTC", "12.5 BTC", "6.25 BTC", "3.125 BTC"];
    let era_colors = ["#fbbf24", "#f59e0b", "#d97706", "#b45309", "#92400e"];
    let metrics = ["Avg Size (MB)", "Avg Tx Count", "Avg Fees (BTC)", "Fee Revenue %"];

    let mut era_avgs: Vec<[f64; 4]> = Vec::new();
    for &era in &eras {
        let (size_sum, tx_sum, fee_sum, pct_sum, count) = era_data[&era];
        let c = count as f64;
        era_avgs.push([
            round(size_sum / c, 3),
            round(tx_sum / c, 1),
            round(fee_sum / c, 4),
            round(pct_sum / c, 2),
        ]);
    }

    let mut maxes = [0.0f64; 4];
    for avgs in &era_avgs {
        for i in 0..4 {
            if avgs[i] > maxes[i] { maxes[i] = avgs[i]; }
        }
    }

    let units = ["MB", "txs", "BTC", "%"];
    let mut series = Vec::new();
    for (ei, &era) in eras.iter().enumerate() {
        let data: Vec<serde_json::Value> = (0..4).map(|i| {
            let norm = if maxes[i] > 0.0 { round(era_avgs[ei][i] / maxes[i] * 100.0, 1) } else { 0.0 };
            json!({ "value": norm, "_raw": era_avgs[ei][i], "_unit": units[i] })
        }).collect();
        let label = if (era as usize) < subsidy_labels.len() {
            format!("Era {} ({})", era, subsidy_labels[era as usize])
        } else {
            format!("Era {}", era)
        };
        let color = era_colors.get(era as usize).unwrap_or(&"#78350f");
        series.push(json!({
            "name": label, "type": "bar", "data": data,
            "itemStyle": { "color": color }
        }));
    }

    build_option(json!({
        "xAxis": {
            "type": "category", "data": metrics,
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": y_axis("% of peak"),
        "tooltip": {
            "trigger": "axis",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 },
            "_useRawValues": true
        },
        "legend": { "show": true },
        "series": series
    }))
}

// ---------------------------------------------------------------------------
// Tier 2: Backfill v10 charts
// ---------------------------------------------------------------------------

/// Fee rate heatmap with 5 percentile bands (p10, p25, median, p75, p90).
/// Stacked area showing the distribution of fee rates across each block.
/// Requires backfill v10 for p25/p75 data.
pub fn fee_rate_heatmap_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fee Rate Heatmap");
    }

    // Check that v10 percentile data is available
    let has_v10 = blocks.iter().any(|b| b.fee_rate_p25 > 0.0 || b.fee_rate_p75 > 0.0);
    if !has_v10 {
        return no_data_chart("Fee Rate Heatmap");
    }

    let p10_str = build_data_array_f64(blocks, |b| round(b.fee_rate_p10, 2));
    let p10_data = data_array_value(&p10_str);

    let p25_str = build_data_array_f64(blocks, |b| round(b.fee_rate_p25, 2));
    let p25_data = data_array_value(&p25_str);

    let median_str = build_data_array_f64(blocks, |b| round(b.median_fee_rate, 2));
    let median_data = data_array_value(&median_str);

    let p75_str = build_data_array_f64(blocks, |b| round(b.fee_rate_p75, 2));
    let p75_data = data_array_value(&p75_str);

    let p90_str = build_data_array_f64(blocks, |b| round(b.fee_rate_p90, 2));
    let p90_data = data_array_value(&p90_str);

    // Distinct colors for each percentile band (cool to hot)
    const P10_COLOR: &str = "#3b82f6"; // blue (cheapest)
    const P25_COLOR: &str = "#22c55e"; // green
    const MED_COLOR: &str = "#f7931a"; // bitcoin orange (median)
    const P75_COLOR: &str = "#f59e0b"; // amber
    const P90_COLOR: &str = "#ef4444"; // red (most expensive)

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Fee Rate (sat/vB)"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "p10", "type": "line", "stack": "fee", "data": p10_data,
                "lineStyle": { "width": 0 }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 },
                "itemStyle": { "color": P10_COLOR }
            },
            {
                "name": "p25", "type": "line", "stack": "fee", "data": p25_data,
                "lineStyle": { "width": 0 }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 },
                "itemStyle": { "color": P25_COLOR }
            },
            {
                "name": "Median", "type": "line", "stack": "fee", "data": median_data,
                "lineStyle": { "width": 0 }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 },
                "itemStyle": { "color": MED_COLOR }
            },
            {
                "name": "p75", "type": "line", "stack": "fee", "data": p75_data,
                "lineStyle": { "width": 0 }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 },
                "itemStyle": { "color": P75_COLOR }
            },
            {
                "name": "p90", "type": "line", "stack": "fee", "data": p90_data,
                "lineStyle": { "width": 0 }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 },
                "itemStyle": { "color": P90_COLOR }
            }
        ]
    }))
}

/// Largest individual transaction fee per block (bar chart in BTC with 144-block MA).
/// Requires backfill v10 for max_tx_fee data.
pub fn max_tx_fee_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Max Tx Fee");
    }

    let has_data = blocks.iter().any(|b| b.max_tx_fee > 0);
    if !has_data {
        return no_data_chart("Max Tx Fee");
    }

    let fee_fn = |b: &BlockSummary| round(b.max_tx_fee as f64 / 100_000_000.0, 6);
    let data_str = build_data_array_f64(blocks, fee_fn);
    let data = data_array_value(&data_str);

    let raw: Vec<f64> = blocks.iter().map(|b| fee_fn(b)).collect();
    let ma = moving_average(&raw, 144);
    let ma_str = build_ma_array(blocks, &ma);
    let ma_data = data_array_value(&ma_str);

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Max Tx Fee", "type": "bar", "data": data,
        "itemStyle": { "color": DATA_COLOR }, "barMaxWidth": 3
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
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Protocol fee breakdown: inscription fees, runes fees, and other fees as stacked area.
/// All values in BTC. Requires backfill v10 for inscription_fees/runes_fees data.
pub fn protocol_fee_breakdown_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Protocol Fee Breakdown");
    }

    let has_data = blocks.iter().any(|b| b.inscription_fees > 0 || b.runes_fees > 0);
    if !has_data {
        return no_data_chart("Protocol Fee Breakdown");
    }

    let other_str = build_data_array_f64(blocks, |b| {
        let other = b.total_fees.saturating_sub(b.inscription_fees).saturating_sub(b.runes_fees);
        round(other as f64 / 100_000_000.0, 6)
    });
    let other_data = data_array_value(&other_str);

    let insc_str = build_data_array_f64(blocks, |b| {
        round(b.inscription_fees as f64 / 100_000_000.0, 6)
    });
    let insc_data = data_array_value(&insc_str);

    let runes_str = build_data_array_f64(blocks, |b| {
        round(b.runes_fees as f64 / 100_000_000.0, 6)
    });
    let runes_data = data_array_value(&runes_str);

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Other", "type": "line", "stack": "proto", "data": other_data,
                "lineStyle": { "width": 0, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 }
            },
            {
                "name": "Inscriptions", "type": "line", "stack": "proto", "data": insc_data,
                "lineStyle": { "width": 0, "color": "#06b6d4" },
                "itemStyle": { "color": "#06b6d4" }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 }
            },
            {
                "name": "Runes", "type": "line", "stack": "proto", "data": runes_data,
                "lineStyle": { "width": 0, "color": RUNES_COLOR },
                "itemStyle": { "color": RUNES_COLOR }, "symbol": "none",
                "areaStyle": { "opacity": 0.6 }
            }
        ]
    }))
}
