//! Mining chart builders: miner dominance donut, empty blocks by month,
//! empty blocks by pool, and mining diversity index (HHI).

use super::*;
use serde_json::json;

const PIE_COLORS: [&str; 11] = [
    "#4ecdc4", "#f7931a", "#ff6b6b", "#bb8fff", "#2ecc71", "#e74c3c",
    "#3498db", "#e67e22", "#1abc9c", "#9b59b6", "#95a5a6",
];

/// Miner dominance donut chart.
pub fn miner_dominance_chart(miners: &[MinerShare]) -> serde_json::Value {
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

    json!({
        "backgroundColor": "transparent",
        "color": colors,
        "tooltip": {
            "trigger": "item",
            "formatter": "{b}: {c} blocks ({d}%)",
            "backgroundColor": "rgba(13,33,55,0.95)",
            "borderColor": "rgba(255,255,255,0.1)",
            "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
        },
        "legend": { "show": false },
        "series": [{
            "name": "Miners",
            "type": "pie",
            "radius": ["40%", "70%"],
            "center": ["50%", "50%"],
            "avoidLabelOverlap": true,
            "itemStyle": {
                "borderRadius": 4,
                "borderColor": "#0e2a47",
                "borderWidth": 2
            },
            "label": {
                "show": true,
                "color": "#ccc",
                "fontSize": 10,
                "formatter": "{b}\n{d}%"
            },
            "labelLine": {
                "length": 10,
                "length2": 8
            },
            "emphasis": {
                "label": { "fontSize": 13, "fontWeight": "bold" }
            },
            "data": pie_data
        }]
    })
}

/// Empty blocks per month. Buckets are pre-aggregated in SQL
/// (`query_empty_blocks_monthly`); this only shapes them for ECharts.
pub fn empty_blocks_chart(buckets: &[HistogramBucket]) -> serde_json::Value {
    if buckets.is_empty() {
        return no_data_chart_with_hint(
            "No empty blocks in this range",
            "Try a longer range (1Y or ALL) to find empty blocks",
        );
    }

    let months: Vec<String> = buckets.iter().map(|b| b.label.clone()).collect();
    let counts: Vec<u64> = buckets.iter().map(|b| b.count).collect();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": months,
            "axisLabel": { "color": "#aaa", "rotate": 45, "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": false },
        "grid": { "left": 45, "right": 20, "top": 25, "bottom": 80 },
        "series": [{
            "name": "Empty Blocks",
            "type": "bar",
            "data": counts,
            "itemStyle": { "color": DATA_COLOR },
            "barMaxWidth": 20
        }]
    }))
}

/// Empty blocks grouped by mining pool. Shows which pools mine the most
/// coinbase-only blocks as a horizontal bar chart.
pub fn empty_blocks_by_pool_chart(
    buckets: &[HistogramBucket],
) -> serde_json::Value {
    if buckets.is_empty() {
        return no_data_chart_with_hint(
            "No empty blocks in this range",
            "Try a longer range (1Y or ALL) to find empty blocks",
        );
    }

    // Already ordered by count descending in SQL.
    let pools: Vec<&str> = buckets.iter().map(|b| b.label.as_str()).collect();
    let counts: Vec<u64> = buckets.iter().map(|b| b.count).collect();

    build_option(json!({
        "xAxis": {
            "type": "value",
            "name": "Empty Blocks",
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.1)" } }
        },
        "yAxis": {
            "type": "category",
            "data": pools,
            "inverse": true,
            "axisLabel": { "color": "#ccc", "fontSize": 11 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "grid": { "left": 120, "right": 30, "top": 25, "bottom": 30 },
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "Empty Blocks",
            "type": "bar",
            "data": counts,
            "itemStyle": { "color": DATA_COLOR },
            "barMaxWidth": 24
        }]
    }))
}

/// Mining diversity index (Herfindahl-Hirschman Index) computed from pool shares.
/// HHI ranges from 0 (perfectly distributed) to 10,000 (single miner).
/// Lower = more decentralized mining. Displayed as a single gauge-style value.
pub fn mining_diversity_chart(miners: &[MinerShare]) -> serde_json::Value {
    if miners.is_empty() {
        return no_data_chart("Mining Diversity");
    }

    // Exclude "Unknown" miners from HHI - early blocks have unidentifiable
    // miners lumped under one label, which inflates concentration artificially.
    let known: Vec<&MinerShare> =
        miners.iter().filter(|m| m.miner != "Unknown").collect();
    let total: u64 = known.iter().map(|m| m.count).sum();
    if total == 0 {
        return no_data_chart("Mining Diversity");
    }

    let hhi: f64 = known
        .iter()
        .map(|m| {
            let share = m.count as f64 / total as f64 * 100.0;
            share * share
        })
        .sum();

    let hhi_rounded = (hhi * 10.0).round() / 10.0;

    // Interpret: <1000 = competitive, 1000-1800 = moderate, >1800 = concentrated
    let (label, color) = if hhi < 1000.0 {
        ("Competitive", "#22c55e")
    } else if hhi < 1800.0 {
        ("Moderate", "#f59e0b")
    } else {
        ("Concentrated", "#ef4444")
    };

    let pool_count = known.len();

    json!({
        "backgroundColor": "transparent",
        "series": [{
            "type": "gauge",
            "startAngle": 200,
            "endAngle": -20,
            "min": 0,
            "max": 5000,
            "splitNumber": 5,
            "center": ["50%", "60%"],
            "radius": "85%",
            "progress": { "show": true, "roundCap": true, "width": 12 },
            "pointer": { "show": false },
            "axisLine": {
                "roundCap": true,
                "lineStyle": { "width": 12, "color": [[0.2, "#22c55e"], [0.36, "#f59e0b"], [1.0, "#ef4444"]] }
            },
            "axisTick": { "show": false },
            "splitLine": { "show": false },
            "axisLabel": { "show": false },
            "title": {
                "show": true,
                "offsetCenter": [0, "75%"],
                "fontSize": 13,
                "color": color
            },
            "detail": {
                "valueAnimation": false,
                "fontSize": 28,
                "fontWeight": "bold",
                "offsetCenter": [0, "40%"],
                "color": "#fff",
                "formatter": format!("{{value}}\n{pool_count} pools")
            },
            "data": [{
                "value": hhi_rounded,
                "name": label
            }]
        }]
    })
}

// ---------------------------------------------------------------------------
// Derived from difficulty and block timing. No extra ingest.
// ---------------------------------------------------------------------------

/// Hashes per second implied by the difficulty the network is currently
/// enforcing.
///
/// `difficulty x 2^32 / 600`. A valid block needs a hash below a target, and
/// difficulty is that target expressed against the easiest one allowed, so
/// `difficulty x 2^32` is how many hashes a miner expects to try per block.
/// Dividing by the ten-minute target gives a rate.
///
/// **An estimate, and the chart says so.** Nobody measures the network's hash
/// rate; it is inferred from how hard the network has chosen to make the
/// problem, and difficulty only moves every 2,016 blocks. Between retargets
/// this line is flat while the real rate is not.
const SECONDS_PER_BLOCK_TARGET: f64 = 600.0;

fn hashes_per_second(difficulty: f64) -> f64 {
    difficulty * 4_294_967_296.0 / SECONDS_PER_BLOCK_TARGET
}

pub fn hash_rate_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Hash Rate");
    }
    let raw_str =
        build_data_array_f64(blocks, |b| hashes_per_second(b.difficulty));
    let raw = data_array_value(&raw_str);
    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        // SI-abbreviated rather than a fixed unit: this runs from a few
        // hundred thousand hashes a second at genesis to nine hundred
        // quintillion today, so any divisor is wrong at one end.
        "yAxis": y_axis_si("Hashes/sec"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "Hash Rate", "type": "line", "data": raw,
            "lineStyle": { "width": 2, "color": DATA_COLOR },
            "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
            "areaStyle": { "color": DATA_COLOR_FADED }
        }]
    }))
}

pub fn hash_rate_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Hash Rate");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| hashes_per_second(d.avg_difficulty))
        .collect();
    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis_si("Hashes/sec"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "Hash Rate", "type": "line", "data": vals,
            "lineStyle": { "width": 2, "color": DATA_COLOR },
            "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
            "areaStyle": { "color": DATA_COLOR_FADED }
        }]
    }))
}

/// How much difficulty moved at each retarget, as a signed percentage.
///
/// The difficulty chart shows the level. This shows the mechanism working:
/// hash rate leaves, blocks come slower, and 2,016 blocks later the network
/// makes itself easier without anyone deciding to.
///
/// Retargets are found by looking for a change in the value rather than by
/// computing heights, because a range rarely starts on an epoch boundary and
/// the daily series has no heights at all. A change of exactly zero is not a
/// retarget, which is what keeps the bars sparse instead of one per block.
fn difficulty_steps<T, F>(rows: &[T], difficulty: F) -> Vec<(usize, f64)>
where
    F: Fn(&T) -> f64,
{
    let mut out = Vec::new();
    let mut previous: Option<f64> = None;
    for (i, row) in rows.iter().enumerate() {
        let d = difficulty(row);
        if d <= 0.0 {
            continue;
        }
        if let Some(prev) = previous {
            // Floating point: two reads of the same epoch differ in the last
            // bits, which would otherwise draw a bar at every block.
            if (d - prev).abs() / prev > 1e-9 {
                out.push((i, (d / prev - 1.0) * 100.0));
            }
        }
        previous = Some(d);
    }
    out
}

pub fn difficulty_adjustment_chart(
    blocks: &[BlockSummary],
) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Difficulty Adjustment");
    }
    let steps = difficulty_steps(blocks, |b| b.difficulty);
    // Two series rather than one with per-bar colours, because a colour
    // chosen per point needs a callback and this option is serialised from
    // Rust. It also gives the chart a legend that explains its own colours,
    // which a conditional fill would not.
    let split = |want_rise: bool| -> Vec<serde_json::Value> {
        steps
            .iter()
            .filter(|&&(_, pct)| (pct > 0.0) == want_rise)
            .map(|&(i, pct)| {
                json!([
                    blocks[i].timestamp * 1000,
                    round(pct, 2),
                    blocks[i].height
                ])
            })
            .collect()
    };
    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Harder", "type": "bar", "data": split(true),
              "barMaxWidth": 6, "itemStyle": { "color": "#22c55e" } },
            { "name": "Easier", "type": "bar", "data": split(false),
              "barMaxWidth": 6, "itemStyle": { "color": "#ef4444" } }
        ]
    }))
}

pub fn difficulty_adjustment_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Difficulty Adjustment");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let steps = difficulty_steps(days, |d| d.avg_difficulty);
    // Positioned by category index, so every day needs a slot and the ones
    // without a retarget carry null rather than zero. Zero would draw a bar
    // of no height at every point and read as "no change today", which is a
    // different claim from "no retarget today".
    let series_for = |want_rise: bool| -> Vec<serde_json::Value> {
        let mut out = vec![serde_json::Value::Null; days.len()];
        for &(i, pct) in &steps {
            if (pct > 0.0) == want_rise {
                out[i] = json!(round(pct, 2));
            }
        }
        out
    };
    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Harder", "type": "bar", "data": series_for(true),
              "barMaxWidth": 6, "itemStyle": { "color": "#22c55e" } },
            { "name": "Easier", "type": "bar", "data": series_for(false),
              "barMaxWidth": 6, "itemStyle": { "color": "#ef4444" } }
        ]
    }))
}

/// Blocks found per day against the 144 the protocol aims for.
///
/// Difficulty holds the *average* at ten minutes, and this is what that
/// average is made of. Any single day is chance: the last five years run from
/// 58 blocks to 197, averaging 145.5. A reader who thinks ten minutes is a
/// promise learns more here than from the interval chart, which shows the
/// same randomness one gap at a time.
///
/// Per-block ranges have no sensible reading, so this is daily only.
pub fn mining_luck_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Mining Luck");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| d.block_count as f64).collect();
    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Blocks"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Blocks found", "type": "bar", "data": vals,
              "itemStyle": { "color": DATA_COLOR }, "barMaxWidth": 4 },
            // The target, drawn rather than described. Without it the bars
            // are a number with nothing to be high or low against.
            { "name": "Target (144)", "type": "line",
              "data": vec![144.0; days.len()],
              "lineStyle": { "width": 2, "color": MA_COLOR, "type": "dashed" },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Placeholder for charts that only make sense over daily aggregates.
///
/// `Source::Dashboard` needs a per-block builder, and mining luck is a count
/// per day: at per-block resolution there is no day to count within. Saying
/// so is better than plotting something that looks like an answer.
pub fn no_data_for_per_block_ranges(
    _blocks: &[BlockSummary],
) -> serde_json::Value {
    no_data_chart("Mining Luck (daily ranges only)")
}
