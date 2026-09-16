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

/// The retargets a daily window can draw, as (day index, percent).
///
/// Read off the retarget blocks themselves rather than reconstructed from the
/// daily difficulty column, which is what this chart did until 2026-09-16.
/// Two facts defeat reconstruction, and the second one has no workaround:
///
/// - **A retarget lands mid-day**, so the day it happens on averages blocks
///   from both epochs and its mean is no protocol difficulty. February 2026
///   went from 125.86T to 144.40T, one step of +14.73%, and stepping through
///   the blend drew +2.59% then +11.83%.
/// - **Sometimes there is no blend day.** Block 899,136 is stamped
///   2025-05-31 00:01:30, so all 161 blocks that day carry the new
///   difficulty. From daily means, that day is indistinguishable from a
///   blend, and every rule that treated one-day runs as blends lost this
///   retarget or invented one.
///
/// A retarget block's own difficulty is the new epoch's difficulty exactly,
/// and its own timestamp is when the epoch changed, so both halves of a bar
/// come from stored values and the result is exact by construction at every
/// range boundary. `query_retargets_for_window` supplies the epoch before the
/// window as well, so the first retarget inside it has something to be a
/// percentage of.
///
/// Consecutive pairs only, and only 2,016 apart. A gap in the retarget rows
/// would otherwise let one bar carry two epochs' worth of change dated to the
/// later one, which is a wrong reading rather than a missing one.
///
/// Unchanged difficulty is not a step: the first sixteen epochs all sat at
/// difficulty 1.0, and a bar of 0% would read as a retarget that did nothing
/// rather than as the network having nothing to correct.
fn retarget_steps(
    days: &[DailyAggregate],
    retargets: &[Retarget],
) -> Vec<(usize, f64)> {
    let index_of: std::collections::HashMap<&str, usize> = days
        .iter()
        .enumerate()
        .map(|(i, d)| (d.date.as_str(), i))
        .collect();
    let mut steps = Vec::new();
    for pair in retargets.windows(2) {
        let (before, after) = (&pair[0], &pair[1]);
        if before.difficulty <= 0.0 || after.height != before.height + 2016 {
            continue;
        }
        if (after.difficulty - before.difficulty).abs() / before.difficulty
            <= 1e-9
        {
            continue;
        }
        // The retarget block's own UTC day. A retarget whose day is outside
        // the loaded window has no slot to sit in, which happens at the far
        // end when the window stops mid-day.
        let Some(&i) =
            chrono::DateTime::from_timestamp(after.timestamp as i64, 0)
                .map(|t| t.format("%Y-%m-%d").to_string())
                .as_deref()
                .and_then(|d| index_of.get(d))
        else {
            continue;
        };
        steps.push((i, (after.difficulty / before.difficulty - 1.0) * 100.0));
    }
    steps
}

/// Difficulty Adjustment, daily: one bar per retarget in the window.
///
/// Takes the retarget rows alongside the days because the days supply only
/// the category axis here. Every value on the bars comes from
/// [`retarget_steps`], which reads the retarget blocks; the daily
/// `avg_difficulty` column is not consulted at all.
pub fn difficulty_adjustment_chart_daily(
    days: &[DailyAggregate],
    retargets: &[Retarget],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Difficulty Adjustment");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let steps = retarget_steps(days, retargets);
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

    fn day(date: &str) -> DailyAggregate {
        DailyAggregate {
            date: date.to_string(),
            // Deliberately zero. The daily difficulty column is not an input
            // to this chart any more, and a fixture that filled it could let
            // a reconstruction creep back in without a test noticing.
            avg_difficulty: 0.0,
            ..Default::default()
        }
    }

    fn days_from(start: &str, n: usize) -> Vec<DailyAggregate> {
        let d0 = chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d")
            .expect("valid date");
        (0..n)
            .map(|i| {
                day(&(d0 + chrono::Duration::days(i as i64))
                    .format("%Y-%m-%d")
                    .to_string())
            })
            .collect()
    }

    fn at(stamp: &str) -> u64 {
        chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S")
            .expect("valid timestamp")
            .and_utc()
            .timestamp() as u64
    }

    fn rt(height: u64, stamp: &str, difficulty: f64) -> Retarget {
        Retarget {
            height,
            timestamp: at(stamp),
            difficulty,
        }
    }

    // The China-ban retarget as the node stores it. Heights 687,456 and
    // 689,472, read out of `bitcoin_stats.db` on 2026-09-16, so the tests
    // below say the chart agrees with the chain rather than with itself.
    const BAN_OLD: f64 = 19_932_791_027_262.73;
    const BAN_NEW: f64 = 14_363_025_673_659.96;
    fn china_ban() -> Vec<Retarget> {
        vec![
            rt(687_456, "2021-06-13 20:07:16", BAN_OLD),
            rt(689_472, "2021-07-03 06:34:06", BAN_NEW),
        ]
    }

    /// The largest fall on record, dated to the day the block landed.
    ///
    /// Block 689,472 at 2021-07-03 06:34:06 UTC took difficulty from
    /// 19,932,791,027,262.73 to 14,363,025,673,659.96, a change of -27.9427%.
    /// Both figures are the stored difficulties of the two retarget blocks,
    /// not a day's mean, so the bar is exact.
    #[test]
    fn the_china_ban_retarget_is_dated_to_the_day_it_happened() {
        let days = days_from("2021-06-30", 7);
        let steps = retarget_steps(&days, &china_ban());
        assert_eq!(steps.len(), 1, "one retarget: {steps:?}");
        let (idx, pct) = steps[0];
        assert_eq!(
            days[idx].date, "2021-07-03",
            "the retarget block's own timestamp is 2021-07-03 06:34 UTC"
        );
        assert!(
            (pct - (BAN_NEW / BAN_OLD - 1.0) * 100.0).abs() < 1e-9,
            "expected -27.9427%, got {pct}"
        );
        assert!(
            (pct + 27.9427).abs() < 0.001,
            "the percentage came from something other than the two retarget \
             blocks: {pct}"
        );
    }

    /// **The R3 acceptance case.** A retarget in the window is reported
    /// whatever day the range stops on.
    ///
    /// Reconstruction from daily means could not do this. It needed a settled
    /// day of the new epoch after the blend, so the same retarget appeared or
    /// vanished depending on data from *after* it: ending 2021-07-03 or 07-04
    /// reported nothing while 07-05 reported -27.94%. Now the bar depends only
    /// on the two retarget blocks, both of which are inside any window that
    /// contains 07-03 at all.
    #[test]
    fn a_retarget_is_reported_whatever_day_the_range_ends_on() {
        for last in ["2021-07-03", "2021-07-04", "2021-07-05"] {
            let n = chrono::NaiveDate::parse_from_str(last, "%Y-%m-%d")
                .expect("valid")
                .signed_duration_since(
                    chrono::NaiveDate::from_ymd_opt(2021, 6, 30)
                        .expect("valid"),
                )
                .num_days() as usize
                + 1;
            let days = days_from("2021-06-30", n);
            let steps = retarget_steps(&days, &china_ban());
            assert_eq!(
                steps.len(),
                1,
                "a range ending {last} lost the retarget: {steps:?}"
            );
            assert_eq!(days[steps[0].0].date, "2021-07-03");
            assert!(
                (steps[0].1 + 27.9427).abs() < 0.001,
                "range ending {last} gave {}",
                steps[0].1
            );
        }
    }

    /// **The case that made the query necessary.** A retarget just after
    /// midnight leaves no blend day at all.
    ///
    /// Block 899,136 is stamped 2025-05-31 00:01:30 and the 161 blocks stored
    /// for that day all carry the new difficulty, so from daily means the day
    /// looks exactly like a settled one-day fragment. Every rule that read
    /// one-day runs as blends either lost this retarget or invented one, and
    /// no further exception could separate the two. From the retarget blocks
    /// it is +4.38%, with no special case.
    #[test]
    fn a_retarget_just_after_midnight_is_found_without_a_blend_day() {
        let days = days_from("2025-05-25", 10);
        let steps = retarget_steps(
            &days,
            &[
                rt(897_120, "2025-05-17 14:01:56", 121_658_450_774_825.0),
                rt(899_136, "2025-05-31 00:01:30", 126_982_285_146_989.3),
            ],
        );
        assert_eq!(steps.len(), 1, "got {steps:?}");
        assert_eq!(days[steps[0].0].date, "2025-05-31");
        assert!(
            (round(steps[0].1, 2) - 4.38).abs() < 1e-9,
            "expected +4.38%, got {}",
            steps[0].1
        );
    }

    /// The row before the window is context, not a bar.
    ///
    /// The query returns the last retarget stamped before the range starts so
    /// the first one inside it has a denominator. That earlier retarget
    /// happened outside the window and drawing it would put a bar on a day
    /// the chart is not showing.
    #[test]
    fn the_epoch_before_the_window_supplies_a_denominator_not_a_bar() {
        // The window starts 2021-06-30, well after 687,456 on 06-13.
        let days = days_from("2021-06-30", 7);
        let steps = retarget_steps(&days, &china_ban());
        assert_eq!(steps.len(), 1);
        assert_eq!(days[steps[0].0].date, "2021-07-03");

        // And with only the predecessor in view there is nothing to draw,
        // rather than a bar of 0% or a panic.
        assert!(retarget_steps(&days, &china_ban()[..1]).is_empty());
        assert!(retarget_steps(&days, &[]).is_empty());
    }

    /// A retarget that changed nothing is not a bar.
    ///
    /// The first sixteen epochs sat at difficulty 1.0. A 0% bar would read as
    /// a retarget that did nothing, which is a different claim from the
    /// network having nothing to correct, and 464 of the ~480 stored
    /// retargets are the ones that moved.
    #[test]
    fn an_unchanged_difficulty_is_not_a_retarget() {
        let days = days_from("2009-01-09", 30);
        let flat: Vec<Retarget> = (0..5)
            .map(|i| {
                rt(
                    i * 2016,
                    &format!("2009-01-{:02} 12:00:00", 10 + i * 3),
                    1.0,
                )
            })
            .collect();
        assert!(retarget_steps(&days, &flat).is_empty());
    }

    /// A missing retarget row is a gap, not a bigger bar.
    ///
    /// Two rows 4,032 apart span two adjustments. Multiplying them together
    /// and dating the product to the later block would be a wrong reading of
    /// a real retarget, which is worse than showing nothing.
    #[test]
    fn a_missing_retarget_row_does_not_merge_two_epochs() {
        let days = days_from("2021-06-30", 30);
        let steps = retarget_steps(
            &days,
            &[
                rt(687_456, "2021-06-13 20:07:16", BAN_OLD),
                // 689,472 absent, so this is 4,032 above its predecessor.
                rt(691_488, "2021-07-17 23:32:17", 13_672_594_272_814.1),
            ],
        );
        assert!(steps.is_empty(), "two epochs became one bar: {steps:?}");
    }

    /// A retarget whose day is not in the window has no slot to sit in.
    ///
    /// This is the window's far end: a range ending mid-day, or a retarget
    /// block returned by timestamp whose UTC day the daily rows do not carry.
    /// Positioning it anyway would mean choosing a wrong day.
    #[test]
    fn a_retarget_outside_the_loaded_days_is_dropped() {
        let days = days_from("2021-07-10", 5);
        assert!(retarget_steps(&days, &china_ban()).is_empty());
    }

    /// Every day without a retarget carries null, not zero.
    ///
    /// Zero would draw a flat bar on all 366 categories and read as "no
    /// change today", which is a different statement from "no retarget
    /// today". The two series also must not both hold a value in one slot: a
    /// retarget is either a rise or a fall.
    #[test]
    fn days_without_a_retarget_are_null_in_both_series() {
        let days = days_from("2021-06-30", 7);
        let opt = difficulty_adjustment_chart_daily(&days, &china_ban());
        let series = opt["series"].as_array().expect("two series");
        assert_eq!(series.len(), 2);
        let harder = series[0]["data"].as_array().expect("data");
        let easier = series[1]["data"].as_array().expect("data");
        assert_eq!(harder.len(), 7);
        assert_eq!(easier.len(), 7);
        assert!(
            harder.iter().all(|v| v.is_null()),
            "a fall put a value in Harder: {harder:?}"
        );
        for (i, v) in easier.iter().enumerate() {
            if i == 3 {
                assert_eq!(v.as_f64(), Some(-27.94), "the bar's own value");
            } else {
                assert!(v.is_null(), "day {i} is not a retarget but holds {v}");
            }
        }
    }

    /// No days means no chart, and no retargets means an empty frame rather
    /// than a missing one.
    #[test]
    fn an_empty_window_draws_nothing_rather_than_guessing() {
        let empty = difficulty_adjustment_chart_daily(&[], &china_ban());
        assert_eq!(empty, no_data_chart("Difficulty Adjustment"));

        let days = days_from("2024-01-01", 5);
        let opt = difficulty_adjustment_chart_daily(&days, &[]);
        let series = opt["series"].as_array().expect("two series");
        for s in series {
            assert!(
                s["data"]
                    .as_array()
                    .expect("data")
                    .iter()
                    .all(|v| v.is_null()),
                "no retargets in the window, but a bar was drawn"
            );
        }
    }

    /// The four R3 acceptance windows, end to end against the live database.
    ///
    /// The unit tests above use the retarget blocks as constants, which
    /// proves the arithmetic and the dating. This proves the other half: that
    /// `query_retargets_for_window` hands the builder the right two rows for
    /// a window cut at each of these boundaries. Ignored because it needs the
    /// real `bitcoin_stats.db`.
    ///
    ///     cargo test --features ssr the_r3_acceptance -- --ignored --nocapture
    #[test]
    #[ignore]
    fn the_r3_acceptance_windows_against_the_live_database() {
        let conn = rusqlite::Connection::open_with_flags(
            "bitcoin_stats.db",
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("bitcoin_stats.db in the working directory");

        // (first day, last day, the day the bar must land on, its value)
        let cases = [
            ("2021-04-01", "2021-07-03", "2021-07-03", -27.94),
            ("2021-04-01", "2021-07-04", "2021-07-03", -27.94),
            ("2021-04-01", "2021-07-05", "2021-07-03", -27.94),
            ("2025-04-01", "2025-05-31", "2025-05-31", 4.38),
        ];
        for (from, to, want_day, want_pct) in cases {
            let d0 = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d")
                .expect("valid");
            let d1 = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d")
                .expect("valid");
            let days = days_from(from, (d1 - d0).num_days() as usize + 1);
            let rows = crate::stats::db::query_retargets_for_window(
                &conn,
                d0.and_hms_opt(0, 0, 0)
                    .expect("valid")
                    .and_utc()
                    .timestamp() as u64,
                d1.and_hms_opt(23, 59, 59)
                    .expect("valid")
                    .and_utc()
                    .timestamp() as u64,
            )
            .expect("query");
            let steps = retarget_steps(&days, &rows);
            let found: Vec<(&str, f64)> = steps
                .iter()
                .map(|&(i, pct)| (days[i].date.as_str(), round(pct, 2)))
                .collect();
            assert!(
                found.contains(&(want_day, want_pct)),
                "{from}..{to} should hold {want_day} at {want_pct}%, got \
                 {found:?} from {} retarget rows",
                rows.len()
            );
            println!("{from}..{to}: {found:?}");
            // With `CQ_OPTION` set, the built option for the matching window
            // goes to stdout so it can be rendered in a browser and read back
            // off the chart model rather than judged from the JSON.
            if std::env::var("CQ_OPTION").as_deref() == Ok(to) {
                println!(
                    "{}",
                    serde_json::to_string(&difficulty_adjustment_chart_daily(
                        &days, &rows
                    ))
                    .expect("json")
                );
            }
        }
    }

    /// Tooling, not coverage: every bar this chart would draw over the whole
    /// stored history, as TSV, so it can be diffed against the same question
    /// asked in SQL.
    ///
    /// Ignored because it needs the real `bitcoin_stats.db`, which is not in
    /// the repository. The comparison it feeds is the full-history check:
    /// `LAG(difficulty)` over the retarget blocks, in SQLite, against what
    /// the chart actually emits, in Rust. Neither side can be right for the
    /// other's reason.
    ///
    ///     cargo test --features ssr dump_retarget_steps -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump_retarget_steps_over_the_whole_history() {
        let conn = rusqlite::Connection::open_with_flags(
            "bitcoin_stats.db",
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("bitcoin_stats.db in the working directory");
        let rows = crate::stats::db::query_retargets_for_window(
            &conn,
            0,
            4_000_000_000,
        )
        .expect("query");

        // Every UTC day from the first retarget to the last, so a bar has a
        // slot wherever it lands. This is the widest window the chart can be
        // asked for.
        let first = chrono::DateTime::from_timestamp(
            rows.first().expect("retargets").timestamp as i64,
            0,
        )
        .expect("valid")
        .date_naive();
        let last = chrono::DateTime::from_timestamp(
            rows.last().expect("retargets").timestamp as i64,
            0,
        )
        .expect("valid")
        .date_naive();
        let days: Vec<DailyAggregate> = (0..=(last - first).num_days())
            .map(|i| DailyAggregate {
                date: (first + chrono::Duration::days(i))
                    .format("%Y-%m-%d")
                    .to_string(),
                ..Default::default()
            })
            .collect();

        println!("DATE\tPCT");
        for (i, pct) in retarget_steps(&days, &rows) {
            println!("{}\t{:.4}", days[i].date, pct);
        }
        eprintln!(
            "{} retarget rows, {} bars",
            rows.len(),
            retarget_steps(&days, &rows).len()
        );
    }
}
