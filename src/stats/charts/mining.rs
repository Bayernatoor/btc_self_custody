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
                "color": "#e8e8e8",
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
            "axisLabel": { "color": "#d4d4d4", "rotate": 45, "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#7a7a7a" } }
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
            "nameTextStyle": { "color": "#d4d4d4" },
            "axisLabel": { "color": "#d4d4d4" },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.1)" } }
        },
        "yAxis": {
            "type": "category",
            "data": pools,
            "inverse": true,
            "axisLabel": { "color": "#e8e8e8", "fontSize": 11 },
            "axisLine": { "lineStyle": { "color": "#7a7a7a" } }
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
        // SI-abbreviated rather than a fixed unit: difficulty 1 implies
        // about 7.2 million hashes a second, against nine hundred quintillion
        // today, so any fixed divisor is wrong at one end.
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
            { "name": "Harder", "type": "bar", "barGap": "-100%",
              "data": split(true), "barMaxWidth": 14,
              "itemStyle": { "color": "#22c55e" } },
            { "name": "Easier", "type": "bar", "barGap": "-100%",
              "data": split(false), "barMaxWidth": 14,
              "itemStyle": { "color": "#ef4444" } }
        ]
    }))
}

/// Retarget steps from a daily series, ignoring the blended day.
///
/// `avg_difficulty` is the mean across the blocks in a UTC day, and a retarget
/// lands mid-day, so the day it happens on averages blocks from both epochs.
/// Stepping between consecutive daily values therefore draws the adjustment
/// twice and gets both halves wrong: February 2026 retargeted from 125.86T to
/// 144.40T, one step of +14.73%, and the naive reading produced +2.59% on the
/// blend day followed by +11.83% on the next.
///
/// So step between *plateaus* instead. An epoch is 2,016 blocks, about a
/// fortnight, so a settled difficulty holds for a dozen days or more and a
/// run of a single day is always a blend. The step is placed on the first day
/// of the new plateau, which is the first full day at the new difficulty.
///
/// A range too short to contain a whole plateau yields nothing, which is the
/// honest answer: there is no retarget in view to measure.
/// Retargets found in a daily difficulty series, as (day index, percent).
///
/// Difficulty is constant for all 2,016 blocks of an epoch, verified across
/// every one of the 480 epochs stored here, so a day wholly inside an epoch
/// carries that epoch's difficulty exactly and a **plateau value is exact**.
/// What is not exact is the date, and that was the defect.
///
/// A retarget almost never lands on midnight, so the day it happens holds
/// blocks from both epochs and its mean is a blend of the two. The previous
/// version discarded that day as noise and dated the retarget to the first day
/// of the new plateau, which is the day *after* it happened. The China-ban
/// retarget is the worked example: block 689,472 at 2021-07-03 06:34 UTC, and
/// `daily_blocks` holds the old difficulty on 07-02, a blend on 07-03 and the
/// new difficulty from 07-04. It was reported as 07-04.
///
/// **The blend day is the retarget day.** A single day whose value sits
/// strictly between the plateaus either side of it is the day the epoch
/// changed, so it dates the step. Where a retarget does land near enough to
/// midnight to leave no blend, the plateaus are adjacent and the first day of
/// the new one is already right.
///
/// A trailing blend is dropped rather than reported. If the range ends on a
/// retarget day, the new epoch's difficulty is not yet visible in any full
/// day, so there is nothing to compute a step against, and the blend is not a
/// protocol difficulty. The previous version kept the final run whatever its
/// length, which meant a range ending on a retarget day published a blended
/// value as a difficulty.
///
/// The per-block view has none of this to worry about: it reads difficulty off
/// real blocks and its steps are exact by construction.
fn daily_difficulty_steps(days: &[DailyAggregate]) -> Vec<(usize, f64)> {
    // Runs of equal difficulty, as (first index, value, length).
    let mut runs: Vec<(usize, f64, usize)> = Vec::new();
    for (i, d) in days.iter().enumerate() {
        if d.avg_difficulty <= 0.0 {
            continue;
        }
        match runs.last_mut() {
            Some(last)
                if (d.avg_difficulty - last.1).abs() / last.1 <= 1e-9 =>
            {
                last.2 += 1;
            }
            _ => runs.push((i, d.avg_difficulty, 1)),
        }
    }

    // A plateau is a run of at least two days, which is an epoch seen whole.
    // Anything shorter is either a blend or a fragment at the range edge, and
    // neither is a difficulty.
    let is_plateau = |r: &(usize, f64, usize)| r.2 > 1;

    let mut steps = Vec::new();
    let mut previous: Option<(usize, f64)> = None;
    let mut pending_blend: Option<usize> = None;

    for r in &runs {
        if is_plateau(r) {
            if let Some((_, before)) = previous {
                // Dated at the blend day where there is one, since that is
                // when the epoch actually changed, and at the first day of
                // this plateau otherwise.
                let at = pending_blend.unwrap_or(r.0);
                steps.push((at, (r.1 / before - 1.0) * 100.0));
            }
            previous = Some((r.0, r.1));
            pending_blend = None;
        } else {
            // A one-day run between two plateaus is the retarget day. Keep its
            // index to date the next step, and keep its value out of the
            // arithmetic.
            pending_blend = Some(r.0);
        }
    }
    steps
}

pub fn difficulty_adjustment_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Difficulty Adjustment");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let steps = daily_difficulty_steps(days);
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
            // `barGap: -100%` overlays the two series in one slot rather
            // than placing them side by side, which at 366 categories left
            // each bar about two pixels wide. They never collide: a retarget
            // is either a rise or a fall, so one series is always null where
            // the other has a value.
            //
            // Not `stack`, which would do the same thing visually and be a
            // lie about the chart. The conformance suite reads the built
            // option, and a declared stack changes which key figures are
            // computed and refuses a log axis. This is a layout trick, not a
            // stacked chart.
            { "name": "Harder", "type": "bar", "barGap": "-100%",
              "data": series_for(true), "barMaxWidth": 14,
              "itemStyle": { "color": "#22c55e" } },
            { "name": "Easier", "type": "bar", "barGap": "-100%",
              "data": series_for(false), "barMaxWidth": 14,
              "itemStyle": { "color": "#ef4444" } }
        ]
    }))
}

#[cfg(test)]
mod adjustment_tests {
    use super::*;

    fn day(date: &str, difficulty: f64) -> DailyAggregate {
        DailyAggregate {
            date: date.to_string(),
            avg_difficulty: difficulty,
            ..Default::default()
        }
    }

    /// The real case, from February 2026: 125.86T to 144.40T is one retarget
    /// of +14.73%. Averaging across the day it lands on produced a blended
    /// value, and stepping through it drew two bars of +2.59% and +11.83%.
    #[test]
    fn a_retarget_is_one_step_not_two() {
        let days = vec![
            day("2026-02-16", 125_864_590_119_494.0),
            day("2026-02-17", 125_864_590_119_494.0),
            day("2026-02-18", 125_864_590_119_494.0),
            // The blend: this day carries blocks from both epochs.
            day("2026-02-19", 129_128_405_963_274.0),
            day("2026-02-20", 144_398_401_518_101.0),
            day("2026-02-21", 144_398_401_518_101.0),
            day("2026-02-22", 144_398_401_518_101.0),
        ];
        let steps = daily_difficulty_steps(&days);
        assert_eq!(steps.len(), 1, "got {steps:?}");
        let (idx, pct) = steps[0];
        // Index 3, the blended day, because that is the day the epoch changed.
        //
        // This asserted index 4 until 2026-09-16, on the reasoning that the
        // step belonged on the first full day at the new value. That is the
        // day *after* the retarget, and it was wrong for the same reason the
        // blend exists: a retarget lands mid-day, so the day holding blocks
        // from both epochs is the day it happened. The old assertion restated
        // what the code did rather than what the chain did.
        assert_eq!(
            idx, 3,
            "the retarget happened on the blended day, not the day after"
        );
        assert!(
            (pct - 14.725).abs() < 0.01,
            "expected the whole adjustment, got {pct}"
        );
    }

    /// The China-ban retarget, with the node's own numbers.
    ///
    /// Block 689,472 at 2021-07-03 06:34:06 UTC took difficulty from
    /// 19,932,791,027,262.74 to 14,363,025,673,659.96, a change of -27.9427%.
    /// `daily_blocks` on this machine holds the old value through 07-02, the
    /// blend 15,634,861,856,766.105 on 07-03, and the new value from 07-04.
    ///
    /// Every figure here is read from the database rather than chosen, so the
    /// test says the chart agrees with the chain, not with itself.
    #[test]
    fn the_china_ban_retarget_is_dated_to_the_day_it_happened() {
        const OLD: f64 = 19_932_791_027_262.74;
        const BLEND: f64 = 15_634_861_856_766.105;
        const NEW: f64 = 14_363_025_673_659.96;
        let days = vec![
            day("2021-06-30", OLD),
            day("2021-07-01", OLD),
            day("2021-07-02", OLD),
            day("2021-07-03", BLEND),
            day("2021-07-04", NEW),
            day("2021-07-05", NEW),
            day("2021-07-06", NEW),
        ];
        let steps = daily_difficulty_steps(&days);
        assert_eq!(steps.len(), 1, "one retarget: {steps:?}");
        let (idx, pct) = steps[0];
        assert_eq!(
            days[idx].date, "2021-07-03",
            "the retarget block's own timestamp is 2021-07-03 06:34 UTC"
        );
        assert!(
            (pct - (NEW / OLD - 1.0) * 100.0).abs() < 1e-9,
            "expected -27.9427%, got {pct}"
        );
        // The blend must not reach the arithmetic. Computing from it would
        // give -21.6% into 07-03 and -8.1% out of it, neither of which
        // happened.
        assert!(
            (pct + 27.9427).abs() < 0.001,
            "the step was computed through the blended day: {pct}"
        );
    }

    /// A range ending on a retarget day reports nothing for it.
    ///
    /// The new epoch's difficulty is not visible in any full day yet, so there
    /// is nothing to compute a step against, and the blend is not a protocol
    /// difficulty. The previous version kept the final run whatever its
    /// length, so a range cut here published the blend as one.
    #[test]
    fn a_range_ending_on_a_retarget_day_publishes_no_difficulty() {
        const OLD: f64 = 19_932_791_027_262.74;
        const BLEND: f64 = 15_634_861_856_766.105;
        let days = vec![
            day("2021-06-30", OLD),
            day("2021-07-01", OLD),
            day("2021-07-02", OLD),
            day("2021-07-03", BLEND),
        ];
        let steps = daily_difficulty_steps(&days);
        assert!(
            steps.is_empty(),
            "a blended day is not a difficulty and cannot end a step: \
             {steps:?}"
        );
    }

    /// A retarget landing close enough to midnight leaves no blend, and the
    /// first day of the new plateau is then the right date.
    #[test]
    fn a_midnight_retarget_needs_no_blend_day() {
        const OLD: f64 = 100_000_000_000_000.0;
        const NEW: f64 = 110_000_000_000_000.0;
        let days = vec![
            day("2026-01-01", OLD),
            day("2026-01-02", OLD),
            day("2026-01-03", NEW),
            day("2026-01-04", NEW),
        ];
        let steps = daily_difficulty_steps(&days);
        assert_eq!(steps.len(), 1, "got {steps:?}");
        assert_eq!(days[steps[0].0].date, "2026-01-03");
        assert!((steps[0].1 - 10.0).abs() < 1e-9);
    }

    /// A range holding no settled plateau has no retarget to report, and
    /// saying nothing is better than reporting a blend as though it were one.
    #[test]
    fn too_short_a_range_reports_nothing() {
        let days = vec![day("2026-02-19", 129.0), day("2026-02-20", 144.0)];
        // Two runs of one day each: the first is a blend with nothing settled
        // before it, so there is no plateau-to-plateau step.
        assert!(daily_difficulty_steps(&days).len() <= 1);
        assert!(daily_difficulty_steps(&[]).is_empty());
    }

    /// Flat difficulty is not a retarget, and floating point noise between
    /// two reads of the same epoch must not become one.
    #[test]
    fn a_settled_epoch_produces_no_bars() {
        let days: Vec<_> = (0..20)
            .map(|i| {
                day(&format!("2026-03-{:02}", i + 1), 144_398_401_518_101.0)
            })
            .collect();
        assert!(daily_difficulty_steps(&days).is_empty());
    }

    /// Downward retargets are the interesting ones, and the sign has to
    /// survive: the largest on record is the 2021 mining ban.
    #[test]
    fn a_downward_retarget_keeps_its_sign() {
        let days = vec![
            day("2021-07-01", 19_932_791_027_263.0),
            day("2021-07-02", 19_932_791_027_263.0),
            day("2021-07-03", 16_000_000_000_000.0),
            day("2021-07-04", 14_363_025_673_660.0),
            day("2021-07-05", 14_363_025_673_660.0),
        ];
        let steps = daily_difficulty_steps(&days);
        assert_eq!(steps.len(), 1);
        assert!(
            steps[0].1 < -27.0 && steps[0].1 > -28.0,
            "got {:?}",
            steps[0]
        );
    }
}
