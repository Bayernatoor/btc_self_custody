//! Fee chart builders: total fees, avg fee per tx, median fee rate, fee rate
//! bands (p10/p50/p90), and subsidy-vs-fees stacked area.

use super::*;
use serde_json::json;

/// Fees line chart (per-block: total fees in sats).
pub fn fees_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fees");
    }

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            if b.total_fees > 0 {
                dp(b, b.total_fees)
            } else {
                json!([ts_ms(b.timestamp), null])
            }
        })
        .collect();

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

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            if b.total_fees > 0 {
                let v = b.total_fees as f64 / divisor;
                let rounded = (v * 1000.0).round() / 1000.0;
                dp(b, rounded)
            } else {
                json!([ts_ms(b.timestamp), null])
            }
        })
        .collect();

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

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            let user_tx = b.tx_count.saturating_sub(1); // exclude coinbase
            if user_tx > 0 {
                b.total_fees as f64 / user_tx as f64
            } else {
                0.0
            }
        })
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

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| (b.median_fee_rate * 100.0).round() / 100.0)
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

    let p10_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, (b.fee_rate_p10 * 100.0).round() / 100.0))
        .collect();

    let median_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, (b.median_fee_rate * 100.0).round() / 100.0))
        .collect();

    let p90_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| dp(b, (b.fee_rate_p90 * 100.0).round() / 100.0))
        .collect();

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

    let subsidy_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let sub = block_subsidy(b.height) as f64 / 100_000_000.0;
            let rounded = (sub * 1000.0).round() / 1000.0;
            dp(b, rounded)
        })
        .collect();

    let fee_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let fee = b.total_fees as f64 / 100_000_000.0;
            let rounded = (fee * 1000.0).round() / 1000.0;
            dp(b, rounded)
        })
        .collect();

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

    let raw_data: Vec<f64> = blocks
        .iter()
        .map(|b| {
            let subsidy = block_subsidy(b.height) as f64;
            let fees = b.total_fees as f64;
            if subsidy + fees > 0.0 {
                fees / (subsidy + fees) * 100.0
            } else {
                0.0
            }
        })
        .collect();

    let ma = moving_average(&raw_data, 144);

    let data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(raw_data.iter())
        .map(|(b, &v)| dp(b, round(v, 2)))
        .collect();

    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| json!([ts_ms(b.timestamp), m.map(|v| json!(v)).unwrap_or(json!(null))]))
        .collect();

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

    let raw: Vec<f64> = blocks
        .iter()
        .map(|b| b.total_output_value as f64 / 100_000_000.0)
        .collect();
    let ma = moving_average(&raw, 144);

    let data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(raw.iter())
        .map(|(b, &v)| dp(b, round(v, 2)))
        .collect();

    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .map(|(b, m)| json!([ts_ms(b.timestamp), m.map(|v| json!(v)).unwrap_or(json!(null))]))
        .collect();

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
/// Note: daily_blocks doesn't store total_output_value sum yet.
pub fn btc_volume_chart_daily(_days: &[DailyAggregate]) -> serde_json::Value {
    no_data_chart("BTC Volume (daily view coming soon)")
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
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "series": [
            {
                "name": "Fee Pressure", "type": "scatter", "data": data,
                "symbolSize": 3,
                "itemStyle": { "color": DATA_COLOR, "opacity": 0.4 }
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
/// Note: daily_blocks doesn't store value sums yet; per-block only for now.
pub fn value_flow_chart_daily(_days: &[DailyAggregate]) -> serde_json::Value {
    no_data_chart("Value Flow")
}
