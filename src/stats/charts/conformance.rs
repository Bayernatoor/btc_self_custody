//! Does every registered chart actually behave the way it says it does?
//!
//! # Why this file exists
//!
//! The registry *declares* a `shape` and a `unit`. Almost every rule is then
//! *derived* from those: `supports_log`, `can_compare`, which charts are
//! offered as comparisons, whether the key figures read as a series or as
//! bands. Declare one wrong and every derived rule is quietly wrong with it,
//! in a direction no other test looks at.
//!
//! The failure that produces is not a crash. It is a picker offering a
//! comparison that silently never draws, or a log toggle that appears and
//! does nothing, on one chart out of 61, found by a reader rather than by us.
//!
//! So rather than documenting the rules and hoping the next person reads
//! them, this builds every chart from synthetic rows and checks the
//! declaration against the option that actually comes out. **Adding a chart
//! with the wrong `shape` fails the build, by name.**
//!
//! # The shape of the guarantee
//!
//! Three layers decide whether a comparison happens, and two of them are
//! derived from metadata:
//!
//! - `registry::is_valid_comparison` is editorial, from `shape` and `unit`
//! - `charts::accepts_comparison_series` is structural, from the built option
//! - the page adds axis contention, which no static check can see
//!
//! The first two must never disagree in the dangerous direction: **anything
//! offered must be drawable.** They can disagree the other way, since the
//! editorial layer is allowed to be stricter for reasons of taste, and a
//! histogram is exactly that case.
//!
//! # Adding a chart
//!
//! Nothing to do. Register it as usual and these run against it. If one
//! fails, the declaration and the builder disagree, and the fix is whichever
//! of the two is telling the truth.

use super::registry::{self, ChartMeta, Daily, Shape, Source, Unit};
use crate::stats::types::{
    BlockSummary, DailyAggregate, HistogramBucket, MinerShare,
};

/// Synthetic blocks, every numeric field populated and varying.
///
/// Every field rather than the handful a given chart reads, because which
/// handful that is differs per chart and the point here is to build all of
/// them. Values rise with the index and are never zero, so a chart cannot
/// come out empty, flat or divided by zero for a reason that has nothing to
/// do with what is being tested.
fn synthetic_blocks(n: usize) -> Vec<BlockSummary> {
    (0..n)
        .map(|i| {
            let i = i as u64;
            let f = i as f64;
            BlockSummary {
                // Spanning three halving eras, because one chart groups
                // by era and has nothing to compare when every block sits in
                // the same one.
                height: 400_000 + i * 700,
                hash: format!("{i:064x}"),
                // Ten minutes apart, except every seventh block, which
                // lands 30 seconds after its predecessor. One chart plots
                // only blocks found under a minute apart, so a perfectly
                // regular series gives it nothing to draw. Still monotonic,
                // since the shortfall is less than the stride.
                timestamp: 1_700_000_000 + i * 600
                    - if i > 0 && i.is_multiple_of(7) { 570 } else { 0 },
                tx_count: 1_000 + i * 3,
                size: 900_000 + i * 500,
                weight: 3_600_000 + i * 2_000,
                difficulty: 1.0e14 + f * 1.0e11,
                total_fees: 10_000_000 + i * 5_000,
                median_fee: 2_000 + i * 7,
                // A real spike near the end, because the fee spike detector
                // plots only blocks above 5x their trailing 144-block
                // average and a smoothly rising series never crosses that.
                // Without one its metric series is empty, which reads as
                // "this chart cannot be compared" when the truth is "this
                // fixture gave it nothing to draw".
                median_fee_rate: if i == 500 { 600.0 } else { 5.0 + f * 0.1 },
                segwit_spend_count: 700 + i,
                taproot_spend_count: 100 + i,
                multisig_count: 10 + i,
                unknown_script_count: 1 + i,
                input_count: 2_500 + i * 4,
                output_count: 2_600 + i * 4,
                rbf_count: 50 + i,
                witness_bytes: 400_000 + i * 300,
                inscription_count: 20 + i,
                inscription_bytes: 50_000 + i * 100,
                inscription_envelope_bytes: 60_000 + i * 100,
                op_return_count: 30 + i,
                op_return_bytes: 2_000 + i * 10,
                runes_count: 5 + i,
                runes_bytes: 300 + i * 2,
                omni_count: 1 + i,
                omni_bytes: 80 + i,
                counterparty_count: 1 + i,
                counterparty_bytes: 90 + i,
                data_carrier_count: 40 + i,
                data_carrier_bytes: 2_500 + i * 12,
                taproot_keypath_count: 80 + i,
                taproot_scriptpath_count: 20 + i,
                total_output_value: 500_000_000_000 + i * 1_000_000,
                total_input_value: 500_010_000_000 + i * 1_000_000,
                // Three builders refuse to draw at all unless these carry
                // something, so leaving them at zero silently dropped those
                // charts out of coverage.
                coinbase_text: format!("/pool{}/ block {i}", i % 4),
                inscription_fees: 400_000 + i * 90,
                runes_fees: 120_000 + i * 40,
                brc20_count: 3 + i,
                fee_rate_p10: 2.0 + f * 0.02,
                fee_rate_p25: 3.5 + f * 0.03,
                fee_rate_p75: 9.0 + f * 0.08,
                fee_rate_p90: 14.0 + f * 0.12,
                max_tx_fee: 3_000_000 + i * 1_100,
                largest_tx_size: 90_000 + i * 30,
                legacy_tx_count: 300 + i,
                segwit_tx_count: 1_300 + i * 2,
                taproot_tx_count: 400 + i,
                // The remainder are fee percentiles, protocol counters and
                // the coinbase text. Zero is a legitimate value for all of
                // them and none decides a shape, so the default is honest
                // here rather than lazy; a chart built only from one of them
                // still has its series and fails the emptiness check above if
                // it does not.
                ..Default::default()
            }
        })
        .collect()
}

/// Synthetic daily rows, on the same principle as [`synthetic_blocks`].
///
/// Spans enough days to cross the threshold where the category axis starts
/// naming months, so the calendar-tick path is exercised here too rather than
/// only in its own tests.
fn synthetic_days(n: usize) -> Vec<DailyAggregate> {
    let start = chrono::NaiveDate::from_ymd_opt(2018, 1, 1).expect("valid");
    (0..n)
        .map(|i| {
            let f = i as f64;
            let u = i as u64;
            DailyAggregate {
                date: (start + chrono::Duration::days(i as i64))
                    .format("%Y-%m-%d")
                    .to_string(),
                block_count: 140 + u % 20,
                avg_size: 900_000.0 + f * 40.0,
                avg_weight: 3_600_000.0 + f * 160.0,
                avg_tx_count: 2_000.0 + f,
                // Stepped every 14 days, not rising daily. Difficulty
                // retargets every 2,016 blocks and holds flat between, and a
                // chart that reads retargets off the daily series finds none
                // in a value that changes every single day.
                avg_difficulty: 1.0e12 + (i / 14) as f64 * 1.4e11,
                total_op_return_count: 4_000 + u * 3,
                total_op_return_bytes: 300_000 + u * 90,
                total_runes_count: 500 + u,
                total_runes_bytes: 30_000 + u * 6,
                total_omni_count: 100 + u,
                total_omni_bytes: 8_000 + u,
                total_counterparty_count: 90 + u,
                total_counterparty_bytes: 7_000 + u,
                total_data_carrier_count: 5_000 + u * 2,
                total_data_carrier_bytes: 350_000 + u * 40,
                total_fees: 1_400_000_000 + u * 900,
                avg_segwit_spend_count: 1_400.0 + f,
                avg_taproot_spend_count: 200.0 + f,
                avg_multisig_count: 15.0 + f * 0.1,
                avg_unknown_script_count: 2.0 + f * 0.01,
                avg_input_count: 5_000.0 + f * 2.0,
                avg_output_count: 5_200.0 + f * 2.0,
                avg_rbf_count: 90.0 + f * 0.2,
                avg_witness_bytes: 420_000.0 + f * 50.0,
                avg_inscription_count: 25.0 + f * 0.3,
                avg_inscription_bytes: 55_000.0 + f * 20.0,
                avg_taproot_keypath_count: 90.0 + f * 0.4,
                avg_taproot_scriptpath_count: 25.0 + f * 0.2,
                avg_stamps_count: 3.0 + f * 0.05,
                avg_median_fee_rate: 6.0 + f * 0.02,
                total_output_value: 700_000_000_000 + u * 2_000_000,
                total_input_value: 700_020_000_000 + u * 2_000_000,
                avg_inscription_envelope_bytes: 62_000.0 + f * 25.0,
                total_inscription_fees: 90_000_000 + u * 400,
                total_runes_fees: 12_000_000 + u * 90,
                avg_legacy_tx_count: 300.0 + f * 0.5,
                avg_segwit_tx_count: 1_300.0 + f,
                avg_taproot_tx_count: 400.0 + f * 0.8,
                ..Default::default()
            }
        })
        .collect()
}

/// Every chart this can build without a second data source, in both
/// resolutions it has.
///
/// The mining charts and the two distributions read resources of their own,
/// so they are absent here and named in
/// [`every_chart_is_either_checked_or_named`] instead. A new `Source` that
/// cannot be built from rows fails that test rather than quietly falling out
/// of this one.
fn built_charts() -> Vec<(&'static ChartMeta, bool, serde_json::Value)> {
    let blocks = synthetic_blocks(600);
    let days = synthetic_days(900);
    let mut out = Vec::new();
    for meta in registry::CHARTS {
        match meta.source {
            Source::Dashboard { per_block, daily } => {
                out.push((meta, false, per_block(&blocks)));
                if let Daily::Fn(f) = daily {
                    out.push((meta, true, f(&days)));
                }
            }
            Source::ChainSize => {
                out.push((
                    meta,
                    false,
                    super::chain_size_chart(&blocks, 620.0, 0, 785_000_000_000),
                ));
                out.push((
                    meta,
                    true,
                    super::chain_size_chart_daily(
                        &days,
                        620.0,
                        0,
                        785_000_000_000,
                    ),
                ));
            }
            Source::Fees => {
                out.push((meta, false, super::fees_chart_unit(&blocks, "btc")));
                out.push((
                    meta,
                    true,
                    super::fees_chart_daily_unit(&days, "btc"),
                ));
            }
            _ => {}
        }
    }
    out
}

/// Series types present in a built option, deduplicated.
fn series_types(option: &serde_json::Value) -> Vec<String> {
    let mut out: Vec<String> = option
        .get("series")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .map(|s| {
                    s.get("type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("line")
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

fn has_stack(option: &serde_json::Value) -> bool {
    option
        .get("series")
        .and_then(|s| s.as_array())
        .is_some_and(|a| a.iter().any(|s| s.get("stack").is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generators have to actually generate something, or every test
    /// below passes against an empty set and proves nothing.
    #[test]
    fn the_conformance_set_covers_most_of_the_registry() {
        let built = built_charts();
        let covered: std::collections::HashSet<&str> =
            built.iter().map(|(m, _, _)| m.slug).collect();
        assert!(
            covered.len() >= 50,
            "only {} of {} charts were built; the synthetic rows or the \
             builder match is probably what broke, not the registry",
            covered.len(),
            registry::CHARTS.len()
        );
        for (meta, daily, opt) in &built {
            assert!(
                opt.get("series")
                    .and_then(|s| s.as_array())
                    .is_some_and(|a| !a.is_empty()),
                "{} ({}) built with no series, so nothing below tested it",
                meta.slug,
                if *daily { "daily" } else { "per block" }
            );
        }
    }

    /// A chart that cannot be built from rows is a deliberate exception, not
    /// a chart that quietly slipped out of coverage. Adding a `Source` that
    /// needs its own resource fails here until someone decides which it is.
    #[test]
    fn every_chart_is_either_checked_or_named() {
        const NEEDS_ITS_OWN_RESOURCE: &[&str] = &[
            "miner-dominance",
            "diversity",
            "empty-blocks",
            "empty-by-pool",
            "fullness-dist",
            "time-dist",
        ];
        let covered: std::collections::HashSet<&str> =
            built_charts().iter().map(|(m, _, _)| m.slug).collect();
        for meta in registry::CHARTS {
            let named = NEEDS_ITS_OWN_RESOURCE.contains(&meta.slug);
            assert_ne!(
                covered.contains(meta.slug),
                named,
                "{} is both built and listed as unbuildable, or neither",
                meta.slug
            );
        }
        for slug in NEEDS_ITS_OWN_RESOURCE {
            assert!(
                registry::find(slug).is_some(),
                "{slug} is listed as unbuildable but is not a chart"
            );
        }
    }

    /// `has_daily` has to mean it. Two charts declared a daily builder that
    /// could only ever return `no_data_chart`, because daily aggregates carry
    /// neither coinbase text nor per-transaction sizes.
    ///
    /// The cost of that lie is quiet: `has_daily` answering true offers the
    /// chart as a comparison over long ranges where it draws nothing, and
    /// suppresses the "this chart needs a shorter range" notice in favour of
    /// an empty frame. `Daily::Unavailable` is the honest declaration and
    /// every derived rule then gets it right.
    #[test]
    fn a_declared_daily_builder_actually_builds_something() {
        let days = synthetic_days(900);
        for meta in registry::CHARTS {
            let Source::Dashboard {
                daily: Daily::Fn(f),
                ..
            } = meta.source
            else {
                continue;
            };
            let opt = f(&days);
            assert!(
                opt.get("series")
                    .and_then(|s| s.as_array())
                    .is_some_and(|a| !a.is_empty()),
                "{} declares a daily builder that draws nothing. If daily \
                 aggregates cannot carry what it needs, it wants \
                 Daily::Unavailable rather than a builder.",
                meta.slug
            );
            assert!(
                meta.has_daily(),
                "{} builds a daily chart but has_daily() says otherwise",
                meta.slug
            );
        }
    }

    /// The declaration that everything else is derived from.
    ///
    /// `shape` decides `supports_log`, `can_compare`, which key figures are
    /// computed and whether a second series may be laid over it. Get it wrong
    /// and all four are wrong together, with nothing else looking.
    #[test]
    fn every_chart_draws_the_shape_it_declares() {
        for (meta, daily, opt) in built_charts() {
            let types = series_types(&opt);
            let at = format!(
                "{} ({})",
                meta.slug,
                if daily { "daily" } else { "per block" }
            );
            // Only the distinctions a rule actually reads.
            //
            // `shape` feeds four derived answers: `has_time_axis`,
            // `accepts_second_series`, `supports_log` and which key figures
            // are computed. Between them they care about exactly three
            // things: is it a pie, is it stacked, and is it neither. Whether
            // a line accompanies the bars changes nothing, and asserting it
            // made this fail on `btc-volume`, whose daily builder drops the
            // line its per-block builder draws. That is a real and harmless
            // difference, and a test that forbids it is a maintenance cost
            // buying no protection.
            let is_pie = types == ["pie"];
            match meta.shape {
                Shape::Donut => {
                    assert!(is_pie, "{at} declares Donut but draws {types:?}")
                }
                Shape::StackedAbsolute | Shape::StackedPercent => assert!(
                    has_stack(&opt),
                    "{at} declares a stack but no series carries one"
                ),
                _ => {
                    assert!(
                        !is_pie,
                        "{at} draws a pie but does not declare Donut, so its \
                         key figures and comparison rules are wrong"
                    );
                    assert!(
                        !has_stack(&opt),
                        "{at} draws a stack but does not declare one, so a \
                         log axis and a second series will be offered on \
                         bands that cannot take them"
                    );
                }
            }
        }
    }

    /// Percentage bands are read against each other and must fill the frame,
    /// which is the whole reason a log axis and a second series are refused
    /// on them. A chart declaring `StackedPercent` without the bounded axis
    /// would have both refusals apply to something that is not actually a
    /// percentage chart.
    #[test]
    fn a_percentage_chart_declares_a_bounded_axis() {
        for (meta, daily, opt) in built_charts() {
            if meta.shape != Shape::StackedPercent {
                continue;
            }
            let bounded = match opt.get("yAxis") {
                Some(serde_json::Value::Array(a)) => a.iter().any(|x| {
                    x.get("max").and_then(|m| m.as_f64()) == Some(100.0)
                }),
                Some(other) => {
                    other.get("max").and_then(|m| m.as_f64()) == Some(100.0)
                }
                None => false,
            };
            assert!(
                bounded,
                "{} ({}) declares StackedPercent but its axis is not capped \
                 at 100",
                meta.slug,
                if daily { "daily" } else { "per block" }
            );
        }
    }

    /// Every data array a builder produces survives being serialised and
    /// parsed back.
    ///
    /// The batching defect is now unreachable rather than merely tested:
    /// `build_data_array_f64` writes `null` for anything non-finite, so an
    /// unrepresentable value becomes a gap instead of an unparseable array.
    /// This guards the assembled `Value` for the same class arriving by
    /// another route, `json!` with a computed float among them.
    ///
    /// It deliberately does **not** flag empty series. A filtered scatter
    /// with no matches is legitimately empty, and from outside that is
    /// indistinguishable from an array that failed to parse. Trying to tell
    /// them apart produced a false positive on `fee-spikes` immediately,
    /// which is why the fix moved to the point of construction.
    ///
    /// The data arrays are assembled as **text**, by `write!` into a String,
    /// and then parsed. `write!` will happily emit `NaN` or `inf` for an f64,
    /// neither of which is JSON, so one such value makes the whole array
    /// unparseable and `data_array_value` returns an empty one. The chart
    /// then renders blank, silently, and the chart-specific test passed
    /// because indexing a missing element yields null exactly as a real gap
    /// does.
    ///
    /// One assertion across every chart in both resolutions beats a null
    /// check per builder, because it does not depend on anyone remembering
    /// that `write!` and `serde_json` disagree about what a float is.
    #[test]
    fn no_built_chart_contains_a_value_json_cannot_represent() {
        fn walk(v: &serde_json::Value, path: &str, bad: &mut Vec<String>) {
            match v {
                serde_json::Value::Number(n) => {
                    if n.as_f64().is_some_and(|f| !f.is_finite()) {
                        bad.push(format!("{path} = {n}"));
                    }
                }
                serde_json::Value::Array(a) => {
                    for (i, x) in a.iter().enumerate() {
                        walk(x, &format!("{path}[{i}]"), bad);
                    }
                }
                serde_json::Value::Object(o) => {
                    for (k, x) in o {
                        walk(x, &format!("{path}.{k}"), bad);
                    }
                }
                _ => {}
            }
        }
        for (meta, daily, opt) in built_charts() {
            let at = format!(
                "{} ({})",
                meta.slug,
                if daily { "daily" } else { "per block" }
            );
            // Round-trips, so a text-assembled array that failed to parse
            // shows up as an empty series rather than passing silently.
            let text = serde_json::to_string(&opt)
                .unwrap_or_else(|e| panic!("{at} will not serialise: {e}"));
            let back: serde_json::Value = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("{at} will not parse back: {e}"));
            let mut bad = Vec::new();
            walk(&back, "", &mut bad);
            assert!(bad.is_empty(), "{at} carries non-finite numbers: {bad:?}");
        }
    }

    /// Every series on a daily chart spans the whole category axis.
    ///
    /// Daily points are bare numbers positioned by index, and two things
    /// depend on that index meaning the same thing in every series: the key
    /// figures date a peak by looking `idx` up in `xAxis.data`, and
    /// `single_series` now concatenates the metric's series so a measurement
    /// split across two is read whole. A series shorter than the axis would
    /// silently shift both.
    ///
    /// Per-block charts are exempt: their points carry their own timestamp,
    /// so a filtered series is self-locating and its index means nothing.
    #[test]
    fn daily_series_all_span_the_category_axis() {
        for (meta, daily, opt) in built_charts() {
            if !daily {
                continue;
            }
            let Some(cats) = opt
                .get("xAxis")
                .and_then(|x| x.get("data"))
                .and_then(|d| d.as_array())
            else {
                continue;
            };
            for s in opt["series"].as_array().into_iter().flatten() {
                let Some(data) = s.get("data").and_then(|d| d.as_array())
                else {
                    continue;
                };
                // Only positional series: an array element carries its own x.
                if data.first().is_some_and(|v| v.is_array()) {
                    continue;
                }
                // A series with no data at all is a carrier for a markLine,
                // which is how the reference lines are drawn. It plots
                // nothing, so it has no indices to misalign, and the key
                // figures already skip it.
                if data.is_empty() {
                    assert!(
                        s.get("markLine").is_some(),
                        "{} series {:?} is empty and carries no markLine, so \
                         it draws nothing at all",
                        meta.slug,
                        s.get("name").and_then(|n| n.as_str()).unwrap_or("?")
                    );
                    continue;
                }
                assert_eq!(
                    data.len(),
                    cats.len(),
                    "{} series {:?} has {} points against {} categories, so \
                     its indices do not line up with the dates",
                    meta.slug,
                    s.get("name").and_then(|n| n.as_str()).unwrap_or("?"),
                    data.len(),
                    cats.len()
                );
            }
        }
    }

    /// `registry::MULTI_METRIC` is exactly the set of charts whose built
    /// option plots more than one metric.
    ///
    /// The list is hardcoded because the fact is not derivable from metadata:
    /// `diff-adjustment` is `Shape::Bar`, the same declaration single-series
    /// bar charts carry. This is what makes a hardcoded list safe. It
    /// computes the truth from the real builders and fails in both
    /// directions, so a chart that gains a second metric cannot stay
    /// offerable and one that loses it cannot stay excluded.
    #[test]
    fn the_multi_metric_list_is_exactly_right() {
        use std::collections::BTreeSet;
        let mut actual: BTreeSet<&str> = BTreeSet::new();
        for (meta, _, opt) in built_charts() {
            // Stacked charts and donuts are already refused on shape, and
            // their bands are one measurement split by category rather than
            // several metrics. The list is about charts that look
            // single-metric and are not.
            if !meta.can_compare() {
                continue;
            }
            // Not `> 1`. Zero is the same defect from the other end:
            // `diff-ribbon` is seven moving averages with no base series, so
            // there is no one metric to lift and no honest label for whichever
            // one was. Both answers mean "this chart does not present a single
            // measurement", which is what the list is for.
            if super::super::metric_series_count(&opt) != 1 {
                actual.insert(meta.slug);
            }
        }
        let declared: BTreeSet<&str> =
            registry::MULTI_METRIC.iter().copied().collect();
        let missing: Vec<&&str> = actual.difference(&declared).collect();
        let stale: Vec<&&str> = declared.difference(&actual).collect();
        assert!(
            missing.is_empty(),
            "these plot more than one metric and are still offered as \
             comparisons: {missing:?}"
        );
        assert!(
            stale.is_empty(),
            "these are excluded but plot a single metric, so the exclusion \
             is costing a working comparison: {stale:?}"
        );
    }

    /// **The one that matters.** The picker is driven by metadata and the
    /// drawing is driven by the built option, and they are allowed to
    /// disagree in exactly one direction.
    ///
    /// Offering something that cannot be drawn is a control that silently
    /// does nothing. Declining to offer something that could be drawn is
    /// taste, which is why a histogram is fine on the other side of this.
    #[test]
    fn nothing_offered_as_a_comparison_would_be_refused_when_drawn() {
        for (meta, daily, opt) in built_charts() {
            if !meta.can_compare() {
                continue;
            }
            assert!(
                super::super::accepts_comparison_series(&opt),
                "{} ({}) offers comparison but its built option cannot hold \
                 a second series. Either the shape or unit it declares is \
                 wrong, or the builder changed under it.",
                meta.slug,
                if daily { "daily" } else { "per block" }
            );
        }
    }

    /// Early-chain readings must survive being plotted.
    ///
    /// The regression this pins is not a shape or a declaration but arithmetic
    /// inside the builders: block size is plotted in megabytes and fees in
    /// BTC, and rounding those to three decimal places turned the genesis
    /// block's 285 bytes and a block's 19,818 sats into zero. On a log axis,
    /// which cannot plot zero, they were then dropped with a notice blaming
    /// the data. 564 daily size averages and 1,073 fee readings across ALL.
    ///
    /// Stated as "a positive reading stays positive", in the charts' own
    /// units, so it holds whatever the rounding is rewritten to do. Checking
    /// the first plotted point specifically, because that is the oldest and
    /// therefore the smallest, and an average over a long window hides it.
    #[test]
    fn a_tiny_reading_still_plots_as_more_than_zero() {
        // Genesis-scale: 285-byte blocks, a handful of transactions, fees of
        // a few thousand satoshis. Every one of these is a real value from
        // the early chain rather than an invented edge case.
        let blocks: Vec<BlockSummary> = (0..400)
            .map(|i| BlockSummary {
                height: i,
                hash: format!("{i:064x}"),
                timestamp: 1_231_006_505 + i * 600,
                tx_count: 1 + i % 3,
                size: 285 + i * 2,
                weight: (285 + i * 2) * 4,
                difficulty: 1.0,
                total_fees: 19_818 + i,
                median_fee: 100,
                median_fee_rate: 0.5,
                input_count: 1,
                output_count: 2,
                total_output_value: 5_000_000_000,
                total_input_value: 5_000_000_000,
                coinbase_text: format!("/early{i}/"),
                ..Default::default()
            })
            .collect();

        let first_plotted = |opt: &serde_json::Value, series: usize| -> f64 {
            let point = opt["series"][series]["data"]
                .as_array()
                .and_then(|a| a.first())
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            match point {
                // Per-block builders emit [ts, value, height]; daily ones
                // emit the bare number.
                serde_json::Value::Array(a) => {
                    a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0)
                }
                other => other.as_f64().unwrap_or(0.0),
            }
        };

        let size = super::super::block_size_chart(&blocks);
        assert!(
            first_plotted(&size, 0) > 0.0,
            "a 285-byte block plotted 0 MB: {}",
            first_plotted(&size, 0)
        );

        let fees = super::super::fees_chart_unit(&blocks, "btc");
        assert!(
            first_plotted(&fees, 0) > 0.0,
            "19,818 sats plotted 0 BTC: {}",
            first_plotted(&fees, 0)
        );

        let chain =
            super::super::chain_size_chart(&blocks, 620.0, 0, 785_000_000_000);
        assert!(
            first_plotted(&chain, 0) > 0.0,
            "the first day of the chain plotted 0 GB: {}",
            first_plotted(&chain, 0)
        );

        // And the same in the daily builders, which have their own arithmetic.
        let days: Vec<DailyAggregate> = (0..40)
            .map(|i| DailyAggregate {
                date: format!("2009-01-{:02}", i % 28 + 1),
                block_count: 120,
                avg_size: 285.0 + i as f64,
                avg_weight: 1_140.0,
                avg_tx_count: 1.0,
                avg_difficulty: 1.0,
                total_fees: 19_818 + i,
                ..Default::default()
            })
            .collect();

        let size_daily = super::super::block_size_chart_daily(&days);
        assert!(
            first_plotted(&size_daily, 0) > 0.0,
            "a 285-byte daily average plotted 0 MB: {}",
            first_plotted(&size_daily, 0)
        );

        let fees_daily = super::super::fees_chart_daily_unit(&days, "btc");
        assert!(
            first_plotted(&fees_daily, 0) > 0.0,
            "a daily fee average plotted 0 BTC: {}",
            first_plotted(&fees_daily, 0)
        );
    }

    /// The disk estimate must not depend on which window you are looking at.
    ///
    /// Stated as the symptom a reader saw: selecting January to March 2020
    /// drew 270 GB of block data against a disk line ending at 876 GB, which
    /// is the node's size today. The ratio came from the window's own ending
    /// cumulative, so the last point was always today's disk size no matter
    /// how far back the window sat.
    ///
    /// Asserted as a ratio between two different windows over the same chain,
    /// which is a property no single window can fake.
    #[test]
    fn the_disk_estimate_is_calibrated_to_the_chain_not_the_window() {
        const CHAIN_TOTAL: u64 = 800_000_000_000; // 800 GB of block data
        const DISK_GB: f64 = 1_000.0; // 1 TB on disk, so a 1.25x overhead

        let last_of = |opt: &serde_json::Value, name: &str| -> f64 {
            let series = opt["series"]
                .as_array()
                .expect("series")
                .iter()
                .find(|s| s["name"] == name)
                .unwrap_or_else(|| panic!("no series named {name}"));
            let point =
                series["data"].as_array().expect("data").last().cloned();
            match point {
                Some(serde_json::Value::Array(a)) => {
                    a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0)
                }
                Some(other) => other.as_f64().unwrap_or(0.0),
                None => 0.0,
            }
        };

        // A historical window: 100 blocks starting 300 GB into the chain.
        let blocks = synthetic_blocks(100);
        let historical = super::super::chain_size_chart(
            &blocks,
            DISK_GB,
            300_000_000_000,
            CHAIN_TOTAL,
        );
        let hist_blocks = last_of(&historical, "Block Data");
        let hist_disk = last_of(&historical, "Disk Size (est.)");

        assert!(
            hist_disk < DISK_GB,
            "a window ending 300 GB into the chain must not end at today's \
             {DISK_GB} GB disk size, got {hist_disk}"
        );
        // The overhead factor, which is the only thing this series asserts.
        let factor = hist_disk / hist_blocks;
        assert!(
            (factor - 1.25).abs() < 0.01,
            "expected today's 1.25x overhead, got {factor}"
        );

        // A window further along the same chain must carry the same factor.
        let later = super::super::chain_size_chart(
            &blocks,
            DISK_GB,
            700_000_000_000,
            CHAIN_TOTAL,
        );
        let later_factor =
            last_of(&later, "Disk Size (est.)") / last_of(&later, "Block Data");
        assert!(
            (later_factor - factor).abs() < 0.01,
            "the factor moved with the window: {factor} then {later_factor}"
        );

        // No calibration, no series. Drawing block data scaled by an unknown
        // ratio would be inventing a line.
        let uncalibrated =
            super::super::chain_size_chart(&blocks, DISK_GB, 0, 0);
        assert!(
            !uncalibrated["series"]
                .as_array()
                .expect("series")
                .iter()
                .any(|s| s["name"] == "Disk Size (est.)"),
            "an uncalibrated disk estimate must be omitted, not guessed"
        );
    }

    /// A backward timestamp is not a zero-second block arrival.
    ///
    /// Miners choose their own timestamps, so a block can be stamped earlier
    /// than its parent. Subtracting on unsigned clamped those to 0, which then
    /// passed the under-a-minute filter and put 139 fabricated zero-second
    /// points on the 1M chart, a fifth of everything drawn there.
    ///
    /// Zero stays. Two blocks sharing a timestamp really did arrive zero
    /// seconds apart, and that is the fastest reading the chart exists to
    /// show; only pairs that run backwards are excluded.
    #[test]
    fn a_backward_timestamp_is_not_a_rapid_block() {
        let mk = |heights: &[(u64, u64)]| -> Vec<BlockSummary> {
            heights
                .iter()
                .map(|&(h, ts)| BlockSummary {
                    height: h,
                    hash: format!("{h:064x}"),
                    timestamp: ts,
                    tx_count: 1,
                    size: 1_000_000,
                    ..Default::default()
                })
                .collect()
        };

        // One genuine 30-second gap, one exact tie, one backward pair, and a
        // normal ten-minute gap.
        let blocks = mk(&[
            (1, 1_700_000_000),
            (2, 1_700_000_030), //  +30s, rapid
            (3, 1_700_000_030), //    0s, rapid (a real tie)
            (4, 1_700_000_000), //  -30s, not an interval
            (5, 1_700_000_600), // +600s, not rapid
        ]);

        let opt = super::super::block_propagation_chart(&blocks);
        let data = opt["series"][0]["data"].as_array().expect("data");
        assert_eq!(
            data.len(),
            2,
            "expected the +30s and the tie, got {data:?}"
        );

        let heights: Vec<u64> = data
            .iter()
            .map(|p| p[2].as_u64().expect("height in the third slot"))
            .collect();
        assert_eq!(heights, vec![2, 3], "the backward pair must not appear");

        // And a chart made only of backward pairs draws nothing rather than a
        // run of zeros.
        let backward =
            mk(&[(1, 1_700_000_600), (2, 1_700_000_300), (3, 1_700_000_000)]);
        let opt = super::super::block_propagation_chart(&backward);
        assert!(
            opt["series"][0]["data"]
                .as_array()
                .is_none_or(|d| d.is_empty()),
            "backward pairs alone must not produce zero-second points"
        );
    }

    /// No transaction to average over is not an average of zero.
    ///
    /// A coinbase-only block has no user transaction, and most of 2010 is
    /// coinbase-only. Emitting zero for those put 1,073 points on the axis
    /// floor over ALL, pulled the line and the key figures down with them, and
    /// on a log axis produced a notice saying 1,073 readings could not be
    /// plotted, blaming the data for a value the builder had invented.
    #[test]
    fn a_block_with_no_user_transaction_reports_no_fee_average() {
        let mk = |tx_count: u64, total_fees: u64| BlockSummary {
            height: 1,
            hash: "a".repeat(64),
            timestamp: 1_700_000_000,
            tx_count,
            total_fees,
            size: 1_000,
            ..Default::default()
        };
        // Coinbase only, a real fee-bearing block, and a block whose only
        // transaction is the coinbase but which somehow records fees.
        let blocks = vec![mk(1, 0), mk(3, 6_000), mk(1, 500)];

        let opt = super::super::avg_fee_per_tx_chart(&blocks);
        let data = opt["series"][0]["data"].as_array().expect("data");
        assert_eq!(data.len(), 3, "every block keeps its position");
        assert!(data[0][1].is_null(), "coinbase only: {:?}", data[0]);
        assert_eq!(data[1][1], 3_000.0, "6000 sats over two user txs");
        assert!(data[2][1].is_null(), "still no transaction to divide by");

        // And the same for the daily builder, whose zero was the one the
        // browser pass counted.
        let day = |avg_tx_count: f64, block_count: u64, total_fees: u64| {
            DailyAggregate {
                date: "2010-05-14".to_string(),
                avg_tx_count,
                block_count,
                total_fees,
                avg_size: 285.0,
                ..Default::default()
            }
        };
        let days = vec![day(1.0, 100, 0), day(2.0, 100, 500_000)];
        let opt = super::super::avg_fee_per_tx_chart_daily(&days);
        let data = opt["series"][0]["data"].as_array().expect("data");
        assert!(data[0].is_null(), "a day of coinbase-only blocks");
        assert_eq!(data[1], 5_000.0, "500k sats over 100 user txs");

        // Which is what clears the log notice: there is nothing unplottable
        // left to warn about.
        let mut v = super::super::avg_fee_per_tx_chart(&blocks);
        super::super::apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["type"], "log");
        assert!(
            v["graphic"].as_array().is_none_or(|g| !g
                .iter()
                .any(|e| e["id"] == "log-scale-notice")),
            "a gap is not an unplottable point"
        );
    }

    /// `registry::NON_TIME_X_AXIS` has to name exactly the charts whose
    /// builders produce a non-time x axis, or the comparison rule built on it
    /// is guessing.
    ///
    /// Checked per block only. Every daily builder puts dates on a category
    /// axis, so at that resolution the kinds agree trivially and the check
    /// would prove nothing; per block is where a chart either carries
    /// timestamps or does not.
    #[test]
    fn the_x_axis_exceptions_are_exactly_right() {
        use std::collections::BTreeSet;
        let mut actual: BTreeSet<&str> = BTreeSet::new();
        for (meta, daily, opt) in built_charts() {
            if daily || !meta.can_compare() {
                continue;
            }
            let axis = match opt.get("xAxis") {
                Some(serde_json::Value::Array(a)) => a.first().cloned(),
                other => other.cloned(),
            };
            let kind = axis.and_then(|x| {
                x.get("type").and_then(|t| t.as_str()).map(str::to_string)
            });
            if kind.as_deref() != Some("time") {
                actual.insert(meta.slug);
            }
        }
        let declared: BTreeSet<&str> =
            registry::NON_TIME_X_AXIS.iter().copied().collect();
        assert_eq!(
            actual, declared,
            "NON_TIME_X_AXIS disagrees with what the builders produce. A \
             chart missing from it is offered comparisons that cannot be \
             drawn; one listed wrongly loses comparisons that would work."
        );
    }

    /// The same contract, pairwise, which is where it was actually failing.
    ///
    /// The test above asks whether a chart can hold *a* second series. Two of
    /// `apply_comparison`'s three refusals cannot be answered by one chart
    /// alone: how many metrics the candidate plots, and whether the two agree
    /// on what their x axis means. Neither was ever checked against a real
    /// pair, so `fee-pressure` was offered on every time-axis chart and
    /// refused on every one of them at draw time. The picker advertised a
    /// comparison, the reader selected it, and nothing appeared.
    ///
    /// Testing one chart at a time cannot find a defect that only exists
    /// between two, which is the general lesson: the unit under test has to
    /// be the unit the invariant is about.
    #[test]
    fn the_offered_comparisons_can_all_actually_be_drawn() {
        for daily in [false, true] {
            let at: Vec<(&'static ChartMeta, serde_json::Value)> =
                built_charts()
                    .into_iter()
                    .filter(|(_, d, _)| *d == daily)
                    .map(|(m, _, o)| (m, o))
                    .collect();
            let mut checked = 0usize;
            for (primary, primary_opt) in &at {
                for (candidate, candidate_opt) in &at {
                    if !registry::is_valid_comparison(primary, candidate, daily)
                    {
                        continue;
                    }
                    let mut v = primary_opt.clone();
                    assert!(
                        super::super::apply_comparison(
                            &mut v,
                            candidate_opt,
                            candidate.title,
                            candidate.unit.label(),
                        ),
                        "{} is offered as a comparison on {} (daily={daily}) \
                         and then refused when drawn. Either it belongs in \
                         registry::MULTI_METRIC or registry::VALUE_X_AXIS, or \
                         the pairing is fine and the refusal in \
                         apply_comparison is too broad.",
                        candidate.slug,
                        primary.slug
                    );
                    checked += 1;
                }
            }
            // The pairs have to actually exist, or a rule that accidentally
            // refused everything would pass this silently.
            assert!(
                checked > 400,
                "only {checked} pairs offered at daily={daily}, which is too \
                 few for the feature to be working at all"
            );
        }
    }

    /// Same contract for the other derived control. A log toggle that appears
    /// and does nothing is the same defect wearing a different hat.
    #[test]
    fn nothing_offering_a_log_axis_would_refuse_one() {
        for (meta, daily, opt) in built_charts() {
            if !meta.supports_log() {
                continue;
            }
            assert!(
                super::super::log_scale_is_meaningful(&opt, 0),
                "{} ({}) offers a log axis but its built option refuses one",
                meta.slug,
                if daily { "daily" } else { "per block" }
            );
        }
    }

    /// A chart using both axes has no room for anything else, which is what
    /// `Unit::Mixed` means. Declaring it without drawing on the second axis,
    /// or the reverse, silently changes what the chart will accept.
    #[test]
    fn a_chart_claiming_both_axes_actually_uses_both() {
        for (meta, daily, opt) in built_charts() {
            let two_axes = opt
                .get("yAxis")
                .and_then(|a| a.as_array())
                .is_some_and(|a| a.len() > 1);
            assert_eq!(
                meta.unit == Unit::Mixed,
                two_axes,
                "{} ({}) declares unit {:?} but draws on {} y axes",
                meta.slug,
                if daily { "daily" } else { "per block" },
                meta.unit,
                if two_axes { "two" } else { "one" }
            );
        }
    }

    /// Key figures are chosen by shape, and the wrong choice prints a number
    /// that is not a fact about anything: an average across six stacked bands,
    /// or a single series' peak read off a donut.
    #[test]
    fn every_chart_produces_the_key_figures_its_shape_implies() {
        use super::super::kpi::{self, Kpis};
        let mut suppressed: Vec<String> = Vec::new();
        for (meta, daily, opt) in built_charts() {
            let json = serde_json::to_string(&opt).expect("serialisable");
            let at = format!(
                "{} ({})",
                meta.slug,
                if daily { "daily" } else { "per block" }
            );
            match (meta.shape, kpi::compute(&json, meta.shape)) {
                (Shape::Donut | Shape::Histogram, Kpis::Categorical { .. })
                | (
                    Shape::StackedAbsolute | Shape::StackedPercent,
                    Kpis::Bands { .. },
                ) => {}
                (Shape::Donut | Shape::Histogram, k)
                | (Shape::StackedAbsolute | Shape::StackedPercent, k) => {
                    panic!("{at} produced {k:?} for shape {:?}", meta.shape)
                }
                (_, Kpis::Series { .. }) => {}
                // A chart plotting several measurements at the same x has no
                // single average, peak or change, so the rail says nothing
                // rather than blending them into a number that describes
                // neither. `MULTI_METRIC` is exactly the set that does not
                // present one measurement, which is why the allowance is
                // keyed on it rather than listed again here.
                (_, Kpis::Unavailable) => suppressed.push(at),
                (_, k) => panic!("{at} produced {k:?} rather than a series"),
            }
        }
        // Pinned by name, so a chart going quiet is a visible change rather
        // than a silent one, and so the list can be read as what it is: the
        // rails that show no figures today.
        //
        // Every one plots several genuinely different measurements at the
        // same x, which has no single average, peak or change.
        //
        // `chain-size` was here and is not any more. Its two series measure
        // the same thing, the second being the first scaled into a disk-size
        // estimate, so it is a companion rather than a second metric, and it
        // now says so with `COMPANION_MARKER` instead of being guessed at
        // from its name. That is the shape every remaining entry wants and
        // what phase 2 generalises.
        //
        // This list should shrink. It must never grow without a reason
        // written down beside it.
        suppressed.sort();
        let expected = [
            "batching (daily)",
            "batching (per block)",
            "cumulative-adoption (daily)",
            "cumulative-adoption (per block)",
            "diff-ribbon (daily)",
            "diff-ribbon (per block)",
            "halving-era (daily)",
            "halving-era (per block)",
            "multi-velocity (daily)",
            "multi-velocity (per block)",
            "utxo-flow (daily)",
            "utxo-flow (per block)",
        ];
        assert_eq!(
            suppressed, expected,
            "the set of rails with no key figures changed. A chart added here \
             is one whose numbers a reader can no longer see; a chart removed \
             is progress and should update this list."
        );
    }
}

#[cfg(test)]
mod inventory {
    use super::*;

    /// Dump the structural facts of every registered chart, for the G0
    /// measurement inventory. Not an assertion: the semantic columns
    /// (aggregation, method, population) cannot be read off the option and are
    /// classified by hand from the builders. Run with --ignored --nocapture.
    #[test]
    #[ignore]
    fn dump_measurement_inventory() {
        println!("SLUG\tRES\tUNIT\tSHAPE\tXAXIS\tYAXES\tSERIES\tMETRICS\tCOMPANIONS\tSTACK\tPCT_BOUND\tNULLS\tSERIES_NAMES");
        // `built_charts()` covers Dashboard, ChainSize and Fees only, so the
        // four mining charts and the two histograms sit outside it and
        // therefore outside every conformance guard. Built here from their own
        // inputs so the G0 inventory is complete at 63; extending the shared
        // harness is G1 work, since it may surface real failures in six charts
        // that no test has ever built.
        let miners: Vec<MinerShare> =
            ["Foundry USA", "AntPool", "F2Pool", "Unknown"]
                .iter()
                .enumerate()
                .map(|(i, m)| MinerShare {
                    miner: (*m).to_string(),
                    count: 400 - (i as u64) * 90,
                    percentage: 40.0 - (i as f64) * 9.0,
                })
                .collect();
        let buckets: Vec<HistogramBucket> = (0..8)
            .map(|i| HistogramBucket {
                label: format!("2024-{:02}", i + 1),
                count: 100 + i * 7,
            })
            .collect();
        let blocks_for_dist = synthetic_blocks(600);
        let extra: Vec<(&'static ChartMeta, bool, serde_json::Value)> =
            registry::CHARTS
                .iter()
                .filter_map(|meta| {
                    let v = match meta.source {
                        Source::Mining(which) => match which {
                            registry::MiningChart::Dominance => {
                                super::super::miner_dominance_chart(&miners)
                            }
                            registry::MiningChart::Diversity => {
                                super::super::mining_diversity_chart(&miners)
                            }
                            registry::MiningChart::EmptyBlocks => {
                                super::super::empty_blocks_chart(&buckets)
                            }
                            registry::MiningChart::EmptyByPool => {
                                super::super::empty_blocks_by_pool_chart(
                                    &buckets,
                                )
                            }
                        },
                        Source::FullnessDist => {
                            super::super::block_fullness_distribution_chart(
                                &blocks_for_dist,
                            )
                        }
                        Source::TimeDist => {
                            super::super::block_time_distribution_chart(
                                &blocks_for_dist,
                            )
                        }
                        _ => return None,
                    };
                    Some((meta, false, v))
                })
                .collect();
        for (meta, daily, opt) in built_charts().into_iter().chain(extra) {
            let axis = match opt.get("xAxis") {
                Some(serde_json::Value::Array(a)) => a.first().cloned(),
                other => other.cloned(),
            };
            let xkind = axis
                .and_then(|x| {
                    x.get("type").and_then(|t| t.as_str()).map(str::to_string)
                })
                .unwrap_or_else(|| "none".into());
            let yaxes = match opt.get("yAxis") {
                Some(serde_json::Value::Array(a)) => a.len(),
                Some(_) => 1,
                None => 0,
            };
            let empty: Vec<serde_json::Value> = vec![];
            let series = opt
                .get("series")
                .and_then(|s| s.as_array())
                .unwrap_or(&empty);
            let names: Vec<String> = series
                .iter()
                .map(|s| {
                    s.get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("<unnamed>")
                        .to_string()
                })
                .collect();
            let companions = series
                .iter()
                .filter(|s| {
                    s.get(super::super::COMPANION_MARKER)
                        .and_then(|v| v.as_bool())
                        == Some(true)
                })
                .count();
            let nulls: usize = series
                .iter()
                .filter_map(|s| s.get("data").and_then(|d| d.as_array()))
                .map(|d| {
                    d.iter()
                        .filter(|p| match p {
                            serde_json::Value::Null => true,
                            serde_json::Value::Array(a) => {
                                a.get(1).is_some_and(|v| v.is_null())
                            }
                            _ => false,
                        })
                        .count()
                })
                .sum();
            let pct_bound = match opt.get("yAxis") {
                Some(serde_json::Value::Array(a)) => a.iter().any(|x| {
                    x.get("max").and_then(|m| m.as_f64()) == Some(100.0)
                }),
                Some(o) => o.get("max").and_then(|m| m.as_f64()) == Some(100.0),
                None => false,
            };
            println!(
                "{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                meta.slug,
                if daily { "daily" } else { "block" },
                meta.unit,
                meta.shape,
                xkind,
                yaxes,
                series.len(),
                super::super::metric_series(&opt).len(),
                companions,
                has_stack(&opt),
                pct_bound,
                nulls,
                names.join(" | ")
            );
        }
    }
}
