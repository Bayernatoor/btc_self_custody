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
        if i > 0 {
            raw_buf.push(',');
        }
        if b.total_fees > 0 {
            let _ = write!(
                raw_buf,
                "[{},{},{}]",
                ts_ms(b.timestamp),
                b.total_fees,
                b.height
            );
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
        if i > 0 {
            raw_buf.push(',');
        }
        if b.total_fees > 0 {
            let v = round_plot(b.total_fees as f64 / divisor);
            let _ =
                write!(raw_buf, "[{},{},{}]", ts_ms(b.timestamp), v, b.height);
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
                let rounded = round_plot(v);
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

    // `None`, not zero, where there is no transaction to average over.
    //
    // A coinbase-only block has no user transaction, and "the average fee was
    // 0 sats" is a different claim from "there was no fee to average": the
    // first is a reading and the second is its absence. Emitting zero put
    // 1,073 of them on the chart over ALL, drew the line down to the axis
    // through the whole of 2010, dragged the average and the low in the key
    // figures to zero, and on a log axis produced a notice announcing that
    // 1,073 points could not be plotted, as though the data were at fault.
    //
    // Same distinction the sibling fee builders already make with
    // `[ts, null]`. A null is a gap, which every consumer handles.
    let fee_fn = |b: &BlockSummary| {
        let user_tx = b.tx_count.saturating_sub(1); // exclude coinbase
        (user_tx > 0).then(|| round_plot(b.total_fees as f64 / user_tx as f64))
    };
    let raw_str = build_data_array_opt_f64(blocks, fee_fn);
    let raw = data_array_value(&raw_str);

    // 144 *blocks*, not 144 readings. Filtering the gaps out before
    // averaging let the window reach past its own name; see
    // `moving_average_over_gaps`.
    let readings: Vec<Option<f64>> = blocks.iter().map(fee_fn).collect();
    let aligned = moving_average_over_gaps(&readings, 144);
    let ma_str = build_ma_array(blocks, &aligned);
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
    // `None` where the day has no transaction to average over; see the
    // per-block twin for why that is not a zero.
    let fee_of = |d: &DailyAggregate| -> Option<f64> {
        let user_tx =
            (d.avg_tx_count * d.block_count as f64) - d.block_count as f64;
        (user_tx > 0.0 && d.total_fees > 0)
            .then(|| round_plot(d.total_fees as f64 / user_tx))
    };
    let readings: Vec<Option<f64>> = days.iter().map(fee_of).collect();
    let vals: Vec<serde_json::Value> = readings
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    // Seven days, not seven readings; see `moving_average_over_gaps`.
    let ma_vals: Vec<serde_json::Value> =
        moving_average_over_gaps(&readings, 7)
            .into_iter()
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

    // A block with no transactions has no median to take.
    //
    // 89,930 blocks carry only the coinbase, and ingestion stores 0.0 for
    // their median fee rate because there is nothing to rank. Plotted, that
    // reads as "the middle transaction paid nothing", which is a measurement
    // the block cannot support, and it drags the 144-block moving average
    // toward zero across the early chain. Same class as the percentile
    // sentinel on the fee-rate bands: absent is not zero.
    let rate_fn = |b: &BlockSummary| -> Option<f64> {
        (b.tx_count > 1).then(|| (b.median_fee_rate * 100.0).round() / 100.0)
    };
    let raw_str = build_data_array_opt_f64(blocks, rate_fn);
    let raw = data_array_value(&raw_str);

    // The moving average skips the gaps rather than averaging zeros into
    // them, which is what `moving_average_over_gaps` exists for.
    let vals: Vec<Option<f64>> = blocks.iter().map(rate_fn).collect();
    let ma = moving_average_over_gaps(&vals, 144);
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
///
/// Reads `avg_median_fee_rate`, the mean of each block's own median, which the
/// ingest computes and stores.
///
/// Not derived from daily fee totals and a guessed vsize. That proxy gave
/// 0.183 sat/vB for 2024-04-20 against a stored median of 880.163, so it is
/// not an approximation in any useful sense.
pub fn median_fee_rate_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Avg Fee Rate");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| round(d.avg_median_fee_rate, 2))
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
        "yAxis": y_axis("sat/vB"),
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

    let p10_str = build_data_array_f64(blocks, |b| {
        (b.fee_rate_p10 * 100.0).round() / 100.0
    });
    let p10_data = data_array_value(&p10_str);

    let median_str = build_data_array_f64(blocks, |b| {
        (b.median_fee_rate * 100.0).round() / 100.0
    });
    let median_data = data_array_value(&median_str);

    let p90_str = build_data_array_f64(blocks, |b| {
        (b.fee_rate_p90 * 100.0).round() / 100.0
    });
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
        round_plot(sub)
    });
    let subsidy_data = data_array_value(&subsidy_str);

    let fee_str = build_data_array_f64(blocks, |b| {
        let fee = b.total_fees as f64 / 100_000_000.0;
        round_plot(fee)
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

    let subsidy_vals: Vec<f64> =
        days.iter().map(|d| daily_subsidy_btc(&d.date)).collect();
    let fee_vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.total_fees > 0 && d.block_count > 0 {
                let v =
                    d.total_fees as f64 / d.block_count as f64 / 100_000_000.0;
                let rounded = round_plot(v);
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

    let raw_data: Vec<f64> = blocks.iter().map(share_fn).collect();
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
pub fn fee_revenue_share_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Fee Revenue Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    let data: Vec<f64> = days
        .iter()
        .map(|d| {
            let subsidy_per_block = daily_subsidy_btc(&d.date);
            let total_subsidy =
                subsidy_per_block * d.block_count as f64 * 100_000_000.0;
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

    let vol_fn = |b: &BlockSummary| {
        round(b.total_output_value as f64 / 100_000_000.0, 2)
    };
    let data_str = build_data_array_f64(blocks, vol_fn);
    let data = data_array_value(&data_str);

    let raw: Vec<f64> = blocks.iter().map(vol_fn).collect();
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
            "nameTextStyle": { "color": "#d4d4d4" },
            "axisLabel": { "color": "#d4d4d4" },
            "axisLine": { "lineStyle": { "color": "#7a7a7a" } },
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

/// Fee spike detector: scatter plot of fee rate spikes (>5x the 144-block trailing
/// average) overlaid on the trailing average line.
pub fn fee_spike_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Fee Spike Detector");
    }
    if blocks.len() < 300 {
        return no_data_chart_with_hint(
            "Fee Spike Detector",
            "Select a longer range (1W+) for enough data to detect fee spikes",
        );
    }

    let rates: Vec<f64> =
        blocks.iter().map(|b| round(b.median_fee_rate, 2)).collect();
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
                if !first {
                    spike_buf.push(',');
                }
                first = false;
                let _ = write!(
                    spike_buf,
                    "[{},{},{}]",
                    ts_ms(b.timestamp),
                    rates[i],
                    b.height
                );
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
                // "MA", not "Avg". `kpi::is_moving_average` matches on " ma",
            // "moving average" and a trailing "ma", so this was the one
            // companion series in the codebase it did not recognise, and the
            // key figures reported the smoothing line instead of the spikes
            // the chart exists to show.
            "name": "144-block MA", "type": "line", "data": ma_data,
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
    let mut era_data: std::collections::BTreeMap<
        u64,
        (f64, f64, f64, f64, u64),
    > = std::collections::BTreeMap::new();

    for b in blocks {
        let era = b.height / 210_000;
        let entry = era_data.entry(era).or_insert((0.0, 0.0, 0.0, 0.0, 0));
        entry.0 += b.size as f64 / 1_000_000.0; // size in MB
        entry.1 += b.tx_count as f64; // tx count
        entry.2 += b.total_fees as f64 / 100_000_000.0; // fees in BTC
        let subsidy = block_subsidy(b.height) as f64;
        let fees = b.total_fees as f64;
        if subsidy + fees > 0.0 {
            entry.3 += fees / (subsidy + fees) * 100.0; // fee revenue %
        }
        entry.4 += 1;
    }

    let subsidy_labels =
        ["50 BTC", "25 BTC", "12.5 BTC", "6.25 BTC", "3.125 BTC"];
    let era_colors = ["#fbbf24", "#f59e0b", "#d97706", "#b45309", "#92400e"];
    let metrics = [
        "Avg Size (MB)",
        "Avg Tx Count",
        "Avg Fees (BTC)",
        "Fee Revenue %",
    ];

    let eras: Vec<u64> = era_data.keys().copied().collect();

    // Need at least 2 eras to compare
    if eras.len() < 2 {
        return no_data_chart_with_hint(
            "Halving Era Comparison",
            "Select a range spanning multiple halving eras (try ALL range)",
        );
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
            if avgs[i] > maxes[i] {
                maxes[i] = avgs[i];
            }
        }
    }

    // Build one series per era. Each data point is [normalized%, rawValue]
    // so the tooltip can show actual values while bars are scaled.
    let units = ["MB", "txs", "BTC", "%"];
    let mut series = Vec::new();
    for (ei, &era) in eras.iter().enumerate() {
        let data: Vec<serde_json::Value> = (0..4)
            .map(|i| {
                let norm = if maxes[i] > 0.0 {
                    round(era_avgs[ei][i] / maxes[i] * 100.0, 1)
                } else {
                    0.0
                };
                let raw = era_avgs[ei][i];
                json!({ "value": norm, "_raw": raw, "_unit": units[i] })
            })
            .collect();
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
            "axisLabel": { "color": "#d4d4d4" },
            "axisLine": { "lineStyle": { "color": "#7a7a7a" } }
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

    let subsidy_btc = [50.0, 25.0, 12.5, 6.25, 3.125];
    let mut era_data: std::collections::BTreeMap<
        u64,
        (f64, f64, f64, f64, u64),
    > = std::collections::BTreeMap::new();

    for d in days {
        let era = halving_era_for_date(&d.date) as u64;
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
        return no_data_chart_with_hint(
            "Halving Era Comparison",
            "Select a range spanning multiple halving eras (try ALL range)",
        );
    }

    let subsidy_labels =
        ["50 BTC", "25 BTC", "12.5 BTC", "6.25 BTC", "3.125 BTC"];
    let era_colors = ["#fbbf24", "#f59e0b", "#d97706", "#b45309", "#92400e"];
    let metrics = [
        "Avg Size (MB)",
        "Avg Tx Count",
        "Avg Fees (BTC)",
        "Fee Revenue %",
    ];

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
            if avgs[i] > maxes[i] {
                maxes[i] = avgs[i];
            }
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
            "axisLabel": { "color": "#d4d4d4" },
            "axisLine": { "lineStyle": { "color": "#7a7a7a" } }
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
    let has_v10 = blocks
        .iter()
        .any(|b| b.fee_rate_p25 > 0.0 || b.fee_rate_p75 > 0.0);
    if !has_v10 {
        return no_data_chart("Fee Rate Heatmap");
    }

    // **A percentile with too small a population is absent, not zero.**
    //
    // Ingestion stores 0.0 when the rank is undefined: p10 and p90 need at
    // least 10 fee-rate observations, p25 and p75 at least 4
    // (`rpc.rs:1072-1090`, and `docs/DATA_DICTIONARY.md` note 5). Plotted
    // literally, that sentinel is not merely a low reading, it is an
    // impossible one: block 963,786 has 9 transactions and stores
    // p10 = 0, p25 = 1.21, median = 2.00, p75 = 3.08, p90 = 0, so the 90th
    // percentile draws *below* the 75th. Chain-wide 5,740 blocks put p90
    // under a positive median.
    //
    // The sentinel cannot be told from a genuine zero by value, because a
    // genuine zero is common: 56,562 pre-2016 blocks have a real p10 of 0
    // with a positive median, from the free-transaction era. The population
    // size is what disambiguates, and `tx_count - 1` is the count of
    // non-coinbase transactions. It is an upper bound on the fee-rate
    // observations rather than exactly equal, which is the right way round:
    // below the threshold the sentinel is certain, so nothing genuine is
    // suppressed. Verified against the database: gating on it nulls all
    // 5,740 impossible p90 readings and leaves every genuine zero standing.
    let user_txs = |b: &BlockSummary| b.tx_count.saturating_sub(1);
    let p10_str = build_data_array_opt_f64(blocks, |b| {
        (user_txs(b) >= 10).then(|| round(b.fee_rate_p10, 2))
    });
    let p10_data = data_array_value(&p10_str);

    let p25_str = build_data_array_opt_f64(blocks, |b| {
        (user_txs(b) >= 4).then(|| round(b.fee_rate_p25, 2))
    });
    let p25_data = data_array_value(&p25_str);

    // The median needs only one observation, and ingestion stores 0.0 for it
    // only when there are none at all, which is a coinbase-only block.
    let median_str = build_data_array_opt_f64(blocks, |b| {
        (user_txs(b) >= 1).then(|| round(b.median_fee_rate, 2))
    });
    let median_data = data_array_value(&median_str);

    let p75_str = build_data_array_opt_f64(blocks, |b| {
        (user_txs(b) >= 4).then(|| round(b.fee_rate_p75, 2))
    });
    let p75_data = data_array_value(&p75_str);

    let p90_str = build_data_array_opt_f64(blocks, |b| {
        (user_txs(b) >= 10).then(|| round(b.fee_rate_p90, 2))
    });
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
        // Five percentile lines, not a stack.
        //
        // These were stacked, so the top of the plot was p10 + p25 + median +
        // p75 + p90 rather than p90: for percentiles 1, 2, 3, 4 and 5 the
        // upper boundary read 15. Every value a reader could take off the
        // chart above the first band was a cumulative sum of quantiles, which
        // is not a quantity.
        //
        // Lines rather than bands between adjacent percentiles, because a
        // band's tooltip would report the gap between two quantiles while the
        // legend named a percentile. The exact stored value stays readable at
        // every series, which is what the export and the tooltip need.
        "series": [
            {
                "name": "p10", "type": "line", "data": p10_data,
                "lineStyle": { "width": 1.5, "color": P10_COLOR },
                "symbol": "none", "itemStyle": { "color": P10_COLOR }
            },
            {
                "name": "p25", "type": "line", "data": p25_data,
                "lineStyle": { "width": 1.5, "color": P25_COLOR },
                "symbol": "none", "itemStyle": { "color": P25_COLOR }
            },
            {
                "name": "Median", "type": "line", "data": median_data,
                "lineStyle": { "width": 2, "color": MED_COLOR },
                "symbol": "none", "itemStyle": { "color": MED_COLOR }
            },
            {
                "name": "p75", "type": "line", "data": p75_data,
                "lineStyle": { "width": 1.5, "color": P75_COLOR },
                "symbol": "none", "itemStyle": { "color": P75_COLOR }
            },
            {
                "name": "p90", "type": "line", "data": p90_data,
                "lineStyle": { "width": 1.5, "color": P90_COLOR },
                "symbol": "none", "itemStyle": { "color": P90_COLOR }
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

    let fee_fn =
        |b: &BlockSummary| round(b.max_tx_fee as f64 / 100_000_000.0, 6);
    let data_str = build_data_array_f64(blocks, fee_fn);
    let data = data_array_value(&data_str);

    let raw: Vec<f64> = blocks.iter().map(fee_fn).collect();
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
pub fn protocol_fee_breakdown_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Protocol Fee Breakdown");
    }

    let has_data = blocks
        .iter()
        .any(|b| b.inscription_fees > 0 || b.runes_fees > 0);
    if !has_data {
        return no_data_chart("Protocol Fee Breakdown");
    }

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
        // Two detector totals, unstacked, and no residual.
        //
        // These were stacked with an "Other" band as though the three were
        // parts of one total. They are not: ingestion credits a transaction's
        // *whole* fee to every detector it matches, and 125,180 blocks have
        // both firing. In 1,326 of them the two totals exceed the block's
        // entire fee take, which the residual hid by flooring at zero.
        //
        // So the sum was never a quantity, and a reader could take a
        // share-of-total figure off this chart that did not exist. Each line
        // on its own is sound: no single detector total ever exceeds a block's
        // fees. The residual is dropped rather than corrected, because it was
        // understated by exactly the overlap and cannot be recovered from what
        // is stored. A true partition needs an ingestion change and a
        // full-chain backfill; see notes/chart-quality-2026-09-15/runs/.
        "series": [
            {
                "name": "Inscriptions", "type": "line", "data": insc_data,
                "lineStyle": { "width": 1.5, "color": "#06b6d4" },
                "itemStyle": { "color": "#06b6d4" }, "symbol": "none"
            },
            {
                "name": "Runes", "type": "line", "data": runes_data,
                "lineStyle": { "width": 1.5, "color": RUNES_COLOR },
                "itemStyle": { "color": RUNES_COLOR }, "symbol": "none"
            }
        ]
    }))
}
