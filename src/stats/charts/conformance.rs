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
    BlockSummary, DailyAggregate, HistogramBucket, MinerShare, Retarget,
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
/// "Never zero" is a claim this has to earn field by field. The six
/// `avg_p2*_count` columns were left at their `Default` of zero until
/// 2026-09-15, so `address-types` and `address-types-pct` were built from
/// nothing in every test that touched them.
///
/// Spans enough days to cross the threshold where the category axis starts
/// naming months, so the calendar-tick path is exercised here too rather than
/// only in its own tests.
fn synthetic_days(n: usize) -> Vec<DailyAggregate> {
    // Straddles 2023-01-01, which several embedded builders use as a cutoff:
    // they emit `null` before it because inscriptions did not exist, so a
    // fixture wholly before that date left those bands null in every test that
    // built them. 900 days from here gives roughly 120 days on the early side
    // and 780 on the late one, so both branches are exercised.
    let start = chrono::NaiveDate::from_ymd_opt(2022, 9, 1).expect("valid");
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
                // The six output-type columns. Absent until 2026-09-15,
                // which meant every daily chart reading them was built from
                // zeros and every assertion about its values was vacuous.
                // Distinct and rising, so a chart that swaps two of them is
                // visible rather than symmetric.
                avg_p2pkh_count: 900.0 - f * 0.4,
                avg_p2sh_count: 300.0 + f * 0.1,
                avg_p2wpkh_count: 700.0 + f * 0.5,
                avg_p2wsh_count: 120.0 + f * 0.2,
                avg_p2tr_count: 200.0 + f * 0.9,
                avg_p2pk_count: 4.0 + f * 0.01,
                // The last three columns any builder reads that this
                // fixture left at `Default`, found by comparing every
                // `d.<field>` in the builders against the fields set here.
                // BRC-20 left the sixth band of Unified Embedded Count at
                // zero in every test that built it.
                avg_brc20_count: 2.0 + f * 0.01,
                avg_fee_rate_p10: 2.0 + f * 0.02,
                avg_fee_rate_p90: 14.0 + f * 0.12,
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

/// A run of empty days from a start date, for tests that only need the
/// category axis. The daily difficulty column is deliberately absent: the
/// Difficulty Adjustment chart does not read it any more.
fn days_from_conformance(start: &str, n: usize) -> Vec<DailyAggregate> {
    let d0 =
        chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").expect("valid");
    (0..n)
        .map(|i| DailyAggregate {
            date: (d0 + chrono::Duration::days(i as i64))
                .format("%Y-%m-%d")
                .to_string(),
            ..Default::default()
        })
        .collect()
}

/// Noon UTC on a date, as a timestamp. Mid-day so a retarget lands inside the
/// day it belongs to whatever the reader's offset.
fn at_noon(date: &str) -> u64 {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .expect("valid")
        .and_hms_opt(12, 0, 0)
        .expect("valid")
        .and_utc()
        .timestamp() as u64
}

/// Retarget blocks matching a run of synthetic days.
///
/// One every 14 days, at 06:34 UTC so it lands mid-day like almost every real
/// retarget, with the same stepped difficulty the days carry. Heights are
/// consecutive multiples of 2,016, which `retarget_steps` requires: a gap
/// there means a missing row rather than a bigger adjustment.
///
/// The step is `1.4e11` on a base of `1.0e12`, so early bars are +14% and
/// later ones smaller, which is the shape of the real series and keeps every
/// bar well clear of the 1e-9 no-change threshold.
fn synthetic_retargets(days: &[DailyAggregate]) -> Vec<Retarget> {
    days.iter()
        .enumerate()
        .filter(|(i, _)| i % 14 == 0)
        .map(|(i, d)| {
            let day = chrono::NaiveDate::parse_from_str(&d.date, "%Y-%m-%d")
                .expect("synthetic dates are valid");
            Retarget {
                height: 800_000 + (i / 14) as u64 * 2016,
                timestamp: day
                    .and_hms_opt(6, 34, 6)
                    .expect("valid time")
                    .and_utc()
                    .timestamp() as u64,
                difficulty: 1.0e12 + (i / 14) as f64 * 1.4e11,
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
            Source::DiffAdjustment => {
                out.push((
                    meta,
                    false,
                    super::difficulty_adjustment_chart(&blocks),
                ));
                out.push((
                    meta,
                    true,
                    super::difficulty_adjustment_chart_daily(
                        &days,
                        &synthetic_retargets(&days),
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
            // The four mining charts and the two histograms, built from their
            // own inputs. Outside this function until 2026-09-15, which meant
            // six of 63 charts sat outside every conformance guard.
            Source::Mining(which) => {
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
                out.push((
                    meta,
                    false,
                    match which {
                        registry::MiningChart::Dominance => {
                            super::miner_dominance_chart(&miners)
                        }
                        registry::MiningChart::Diversity => {
                            super::mining_diversity_chart(&miners)
                        }
                        registry::MiningChart::EmptyBlocks => {
                            super::empty_blocks_chart(&buckets)
                        }
                        registry::MiningChart::EmptyByPool => {
                            super::empty_blocks_by_pool_chart(&buckets)
                        }
                    },
                ));
            }
            Source::FullnessDist => out.push((
                meta,
                false,
                super::block_fullness_distribution_chart(&blocks),
            )),
            Source::TimeDist => out.push((
                meta,
                false,
                super::block_time_distribution_chart(&blocks),
            )),
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
        // Empty since 2026-09-15: every registered chart is built here,
        // including the mining and histogram sources, which used to be listed
        // as unbuildable and were therefore outside every guard in this file.
        // The first build found that `diversity` declared a donut and drew a
        // gauge.
        const NEEDS_ITS_OWN_RESOURCE: &[&str] = &[];
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
                Shape::Gauge => {
                    assert_eq!(
                        types,
                        vec!["gauge".to_string()],
                        "{} declares Gauge but draws {types:?}",
                        meta.slug
                    );
                }
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

    // -----------------------------------------------------------------------
    // Structured measurement declarations
    // -----------------------------------------------------------------------

    /// Chart 64 cannot arrive without a declaration.
    ///
    /// This was a shrinking pin while the catalog was migrated in tranches.
    /// All 63 are declared, so it is now an absolute: a registered chart
    /// without measurements fails the build.
    #[test]
    fn every_chart_declares_what_it_measures() {
        let undeclared: Vec<&str> = registry::CHARTS
            .iter()
            .filter(|c| c.measurements.is_empty())
            .map(|c| c.slug)
            .collect();
        assert_eq!(
            undeclared.len(),
            0,
            "every chart must declare what it measures, so a new one cannot \
             be added without one. Undeclared: {undeclared:?}"
        );
    }

    /// A declaration names the series it describes, so it has to match what
    /// the builder emits. Renaming a series in a builder without updating the
    /// declaration would otherwise leave a badge attached to nothing.
    #[test]
    fn every_declared_series_name_exists_in_the_built_option() {
        for (meta, daily, opt) in built_charts() {
            for m in meta.measurements {
                // The daily builder's name where it differs, so a chart that
                // legitimately renames a series between resolutions is
                // checked against the name actually emitted at each.
                let want = if daily && !m.series_daily.is_empty() {
                    m.series_daily
                } else {
                    m.series
                };
                if want.is_empty() {
                    continue; // covers every series
                }
                let names: Vec<&str> = opt["series"]
                    .as_array()
                    .map(|a| {
                        a.iter().filter_map(|s| s["name"].as_str()).collect()
                    })
                    .unwrap_or_default();
                assert!(
                    names.contains(&want),
                    "{} ({}) declares a measurement for series {want:?}, but \
                     the built option has {names:?}",
                    meta.slug,
                    if daily { "daily" } else { "per block" }
                );
            }
        }
    }

    /// Declaring a daily aggregation for a chart with no daily builder would
    /// promise a result that cannot exist, which is the mirror of the
    /// `coinbase-msg-length` defect.
    #[test]
    fn a_declared_daily_aggregation_implies_a_daily_builder() {
        for meta in registry::CHARTS.iter() {
            for m in meta.measurements {
                let declares_daily =
                    m.daily != registry::Aggregation::Unsupported;
                assert_eq!(
                    declares_daily,
                    meta.has_daily(),
                    "{} declares daily aggregation {:?} but has_daily() is {}",
                    meta.slug,
                    m.daily,
                    meta.has_daily()
                );
            }
        }
    }

    /// Dump a chart's built option for browser verification. Not an
    /// assertion; run with --ignored --nocapture and a slug in CQ_DUMP.
    #[test]
    #[ignore]
    fn dump_option_for_browser() {
        let want = std::env::var("CQ_DUMP").unwrap_or_default();
        for (meta, daily, opt) in built_charts() {
            if meta.slug == want && !daily {
                println!("{}", serde_json::to_string(&opt).expect("json"));
            }
        }
    }

    /// A measurement whose method differs by resolution is the case a single
    /// per-chart badge cannot express.
    ///
    /// `difficulty` is the example. Per block it reads the value off the
    /// block, which is exact. Daily it plots the mean of the day's blocks, and
    /// difficulty is constant for all 2,016 blocks of an epoch, so that mean
    /// is the difficulty on every day except the one a retarget lands on,
    /// where it is a blend of two epochs and is no protocol difficulty.
    ///
    /// `diff-adjustment` was the example here until 2026-09-16, when its daily
    /// arm stopped being an estimate: it reads the retarget blocks at both
    /// resolutions, so both the date and the percentage come from stored
    /// values and the two methods are the same. A chart graduating out of
    /// this test is the outcome to want.
    #[test]
    fn a_method_may_differ_between_resolutions() {
        let m = registry::find("difficulty")
            .expect("difficulty is registered")
            .measurements;
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].method_per_block, registry::Method::Measured);
        assert_eq!(m[0].method_daily, registry::Method::Estimated);
        assert_ne!(
            m[0].method_per_block, m[0].method_daily,
            "if these are ever equal for this chart, either the daily mean \
             stopped blending across retargets or the declaration is wrong"
        );

        // And the schema has to be able to carry the distinction at all, so at
        // least one chart in the catalog must use it. Collapsing the two
        // fields back into one would silently relabel every such daily view.
        let differing: Vec<&str> = registry::CHARTS
            .iter()
            .filter(|c| {
                c.measurements
                    .iter()
                    .any(|m| m.method_per_block != m.method_daily)
            })
            .map(|c| c.slug)
            .collect();
        assert!(
            differing.len() >= 2,
            "only {differing:?} distinguish method by resolution, which is too \
             few to justify the field; if that is genuinely correct, drop it"
        );
    }

    /// Every method label a reader can see must explain itself, or the badge
    /// is decoration.
    #[test]
    fn every_method_label_has_an_explanation() {
        for m in [
            registry::Method::Measured,
            registry::Method::Calculated,
            registry::Method::Estimated,
            registry::Method::HeuristicallyDetected,
        ] {
            assert!(!m.label().is_empty(), "{m:?} has no label");
            assert!(
                m.explanation().len() > 30,
                "{m:?} explanation is too short to explain anything: {:?}",
                m.explanation()
            );
        }
    }

    /// A declared aggregation must survive contact with its own builder.
    ///
    /// The gap this closes: nothing tied a classification to the arithmetic it
    /// claims to describe. A bulk survey that paired one chart's slug with
    /// another chart's builder produced exactly that error during this work,
    /// and every existing check passed, because series names still existed and
    /// the daily flag still matched. The mistake was caught by memory, which
    /// is not a mechanism.
    ///
    /// The probe needs no knowledge of which column a chart reads. Build each
    /// chart twice from days that are identical except that `block_count` is
    /// doubled, holding every `avg_*` field fixed. Then the declaration
    /// predicts the outcome:
    ///
    /// - `MeanOfPerBlockValues` plots a stored mean, so it must **not** move.
    /// - `RatioOfTotals` scales numerator and denominator together, so it must
    ///   **not** move.
    /// - `DailyTotal` and `CumulativeInWindow` multiply by the block count, so
    ///   they **must** move.
    ///
    /// A chart classified into the wrong one of those pairs fails here.
    #[test]
    fn a_declared_aggregation_predicts_how_the_builder_responds() {
        use registry::Aggregation::*;

        // Scaling `block_count` alone produces a day that cannot exist,
        // because the stored `total_*` columns are sums over that same day.
        // `op-block-share` divides `total_op_return_bytes` by
        // `avg_size * block_count`, so an unscaled numerator against a scaled
        // denominator made a correctly declared ratio look like it moved. The
        // day has to stay internally consistent for the probe to mean
        // anything, so the totals scale with it.
        let scaled = |k: u64, d: DailyAggregate| -> DailyAggregate {
            let m = |v: u64| v * k;
            DailyAggregate {
                block_count: d.block_count * k,
                total_op_return_count: m(d.total_op_return_count),
                total_op_return_bytes: m(d.total_op_return_bytes),
                total_runes_count: m(d.total_runes_count),
                total_runes_bytes: m(d.total_runes_bytes),
                total_omni_count: m(d.total_omni_count),
                total_omni_bytes: m(d.total_omni_bytes),
                total_counterparty_count: m(d.total_counterparty_count),
                total_counterparty_bytes: m(d.total_counterparty_bytes),
                total_data_carrier_count: m(d.total_data_carrier_count),
                total_data_carrier_bytes: m(d.total_data_carrier_bytes),
                total_fees: m(d.total_fees),
                total_output_value: m(d.total_output_value),
                total_input_value: m(d.total_input_value),
                total_inscription_fees: m(d.total_inscription_fees),
                total_runes_fees: m(d.total_runes_fees),
                ..d
            }
        };
        let base: Vec<DailyAggregate> = synthetic_days(900)
            .into_iter()
            .map(|d| scaled(1, d))
            .collect();
        let doubled: Vec<DailyAggregate> = synthetic_days(900)
            .into_iter()
            .map(|d| scaled(2, d))
            .collect();

        // Per series and by name, with gaps preserved as `None`.
        //
        // The name is what ties a series to the measurement that declares
        // it, which is how coverage became a per-measurement question. And
        // `None` stays `None` rather than becoming `0.0`: this test spent a
        // revision mapping nulls to zero, which threw away the
        // missing-versus-measured-zero distinction the rest of this branch
        // exists to protect, and then skipped every zero, so an absence
        // turning into a reading was invisible twice over.
        type Series = (String, bool, Vec<Option<f64>>);
        let per_series = |opt: &serde_json::Value| -> Vec<Series> {
            opt["series"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|s| {
                            let name = s["name"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string();
                            let points = s["data"]
                                .as_array()
                                .map(|d| d.as_slice())
                                .unwrap_or_default()
                                .iter()
                                .map(|p| match p {
                                    serde_json::Value::Number(n) => n.as_f64(),
                                    // `[x, y]` and `{value: y}` points:
                                    // the reading is the second element
                                    // or the field, and a null there is
                                    // still a gap.
                                    serde_json::Value::Array(a) => {
                                        a.get(1).and_then(|v| v.as_f64())
                                    }
                                    serde_json::Value::Object(o) => {
                                        o.get("value").and_then(|v| v.as_f64())
                                    }
                                    _ => None,
                                })
                                .collect();
                            (name, super::super::is_companion_series(s), points)
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        // What a measurement's declaration predicts when the day doubles.
        let expectation = |m: &registry::Measurement| match m.daily {
            DailyTotal | CumulativeInWindow => Some(2.0),
            MeanOfPerBlockValues | RatioOfTotals => Some(1.0),
            _ => None,
        };
        let mut checked = 0usize;
        for meta in registry::CHARTS.iter() {
            let Source::Dashboard {
                daily: Daily::Fn(f),
                ..
            } = meta.source
            else {
                continue;
            };
            let a = per_series(&f(&base));
            let b = per_series(&f(&doubled));
            assert_eq!(
                a.len(),
                b.len(),
                "{}: doubling the day changed the series count",
                meta.slug
            );

            // Per measurement, not per chart. A chart whose measurements
            // disagree about scaling used to be skipped entirely, which is
            // most of the catalog's interesting charts: `subsidy-fees` plots
            // a mean beside a total and neither was ever checked. Each
            // declaration is now held to its own series.
            let mut covered_series: std::collections::BTreeSet<usize> =
                Default::default();
            for m in meta.measurements {
                let want_name = if m.series_daily.is_empty() {
                    m.series
                } else {
                    m.series_daily
                };
                // An empty name covers every series, because the series are
                // renderings of one quantity rather than separate
                // measurements.
                let mine: Vec<usize> = a
                    .iter()
                    .enumerate()
                    .filter(|(_, (name, _, _))| {
                        want_name.is_empty() || name == want_name
                    })
                    .map(|(i, _)| i)
                    .collect();
                covered_series.extend(mine.iter().copied());
                let Some(want) = expectation(m) else {
                    continue;
                };
                let mut compared = 0usize;
                let mut decoration_only = true;
                for si in mine {
                    let (name, _, sa) = &a[si];
                    let sb = &b[si].2;
                    // A series with no points at all is decoration: the
                    // threshold markers on P2PKH Sunset carry their values in
                    // `markLine`. That is the only exemption, and it is read
                    // off the series itself rather than granted because a
                    // sibling passed.
                    if sa.is_empty() && sb.is_empty() {
                        continue;
                    }
                    decoration_only = false;
                    assert_eq!(
                        sa.len(),
                        sb.len(),
                        "{} series {name:?}: point count changed",
                        meta.slug
                    );
                    for (x, y) in sa.iter().zip(sb.iter()) {
                        match (x, y) {
                            // A gap has to stay a gap. An absence that
                            // becomes a number when the block count doubles
                            // is a builder substituting a computed zero for
                            // "no reading", which is the defect
                            // `an_absent_reading_is_a_gap_not_a_zero` guards
                            // at one resolution and this guards under
                            // scaling.
                            (None, None) => {}
                            (None, Some(v)) | (Some(v), None) => panic!(
                                "{} series {name:?}: a gap and a reading \
                                 swapped places when the day doubled ({v} \
                                 against nothing)",
                                meta.slug
                            ),
                            (Some(x), Some(y)) => {
                                if x.abs() < 1e-12 {
                                    // Zero is checked rather than skipped.
                                    // Skipping it accepted a zero baseline
                                    // becoming nonzero, which no declaration
                                    // here permits: twice nothing is
                                    // nothing, and a stored mean of zero
                                    // does not move either.
                                    assert!(
                                        y.abs() < 1e-12,
                                        "{} series {name:?}: a zero became \
                                         {y} when the day doubled",
                                        meta.slug
                                    );
                                    continue;
                                }
                                let factor = y / x;
                                // One percent, not float epsilon. Builders
                                // round their output, typically to two or
                                // four decimals, so doubling the input and
                                // rounding is not the same number as
                                // rounding and doubling: 0.9 becomes 1.8001
                                // rather than 1.8. The question is whether a
                                // series moved by a factor of two or not at
                                // all, and those differ by a hundred percent.
                                assert!(
                                    (factor - want).abs() < 0.01 * want,
                                    "{} series {name:?} declares {:?}, so \
                                     every value should move by {want}x when \
                                     the day doubles, but {x} became {y}, a \
                                     factor of {factor}",
                                    meta.slug,
                                    m.daily
                                );
                                compared += 1;
                            }
                        }
                    }
                }
                // Coverage per measurement. Asserted per chart until
                // 2026-09-16, which let one discriminating series stand in
                // for every declaration the chart made: a second series
                // could go entirely unexercised and the chart still passed.
                assert!(
                    compared > 0 || decoration_only,
                    "{}: the measurement for {:?} declares {:?} but nothing \
                     in its series was comparable, so that declaration was \
                     not checked; give the fixture non-zero values for the \
                     columns it reads",
                    meta.slug,
                    if want_name.is_empty() {
                        "every series"
                    } else {
                        want_name
                    },
                    m.daily
                );
                if !decoration_only {
                    checked += 1;
                }
            }

            // And no series may sit outside every declaration. A new series
            // added to a builder without a declaration would otherwise be
            // exempt from all of the above, which is the hole that makes
            // per-measurement coverage worth having.
            for (i, (name, companion, points)) in a.iter().enumerate() {
                if covered_series.contains(&i) || points.is_empty() {
                    continue;
                }
                // A companion is allowed to go undeclared, because it
                // renders a measurement the chart already declares: Transaction
                // Batching draws each ratio and its 7-day mean. Anything else
                // is a series the declarations do not know about, and it would
                // be exempt from every check above.
                assert!(
                    companion,
                    "{} emits series {name:?} that no measurement declares \
                     and that is not a companion, so nothing here checks it",
                    meta.slug
                );
                // A smoothed measurement still scales the way the
                // measurement does, so the companion is held to it whenever
                // the chart's declarations agree on a factor.
                let wants: Vec<f64> =
                    meta.measurements.iter().filter_map(expectation).collect();
                if wants.is_empty() || wants.iter().any(|w| *w != wants[0]) {
                    continue;
                }
                let want = wants[0];
                for (x, y) in points.iter().zip(b[i].2.iter()) {
                    if let (Some(x), Some(y)) = (x, y) {
                        if x.abs() < 1e-12 {
                            assert!(
                                y.abs() < 1e-12,
                                "{} companion {name:?}: a zero became {y}",
                                meta.slug
                            );
                            continue;
                        }
                        let factor = y / x;
                        assert!(
                            (factor - want).abs() < 0.01 * want,
                            "{} companion {name:?} smooths measurements that \
                             move by {want}x, but {x} became {y}, a factor \
                             of {factor}",
                            meta.slug
                        );
                    }
                }
            }
        }
        assert!(
            checked >= 10,
            "only {checked} measurements had a predictable declaration; the \
             probe is not covering enough to be worth running"
        );
    }

    /// SegWit Adoption's daily point is a pooled ratio **of its own day**,
    /// and the numbers here separate that from every neighbouring reading.
    ///
    /// A day of 10 blocks averaging 2.0 transactions and 0.5 witness spends
    /// holds 20 transactions, 10 of them coinbase, and 5 witness spends, so
    /// its pooled share is 5/10 = **50%**. A second day of 100 blocks
    /// averaging 11.0 transactions and 10.0 witness spends holds 1,100
    /// transactions, 100 of them coinbase, and 1,000 witness spends:
    /// **100%**.
    ///
    /// The populations are deliberately unequal, which is what the earlier
    /// version of this test was missing. Both of its days came out at 50%, so
    /// a builder pooling across the whole window rather than within each day
    /// produced the same two numbers and passed. Here every wrong reading
    /// lands somewhere different:
    ///
    /// - forgetting the coinbase gives 25% and 90.9%
    /// - reading the stored means as a share gives 25% and 90.9%
    /// - pooling across the window gives 1,005/1,010 = **99.5% on both days**
    /// - an unweighted mean of the two days gives **75% on both**
    ///
    /// Note what cannot be tested this way, since it is the other half of the
    /// same question: within a single day, a pooled ratio and a
    /// block-count-weighted one are the same number. The stored columns are
    /// already means over that day's blocks, so the count cancels between
    /// numerator and denominator. The distinction is only observable across
    /// days of unequal size, which is exactly what this fixture is.
    #[test]
    fn segwit_daily_is_a_pooled_ratio_of_its_own_day() {
        let day = |date: &str,
                   block_count: u64,
                   avg_tx: f64,
                   avg_segwit: f64| DailyAggregate {
            date: date.to_string(),
            block_count,
            avg_tx_count: avg_tx,
            avg_segwit_spend_count: avg_segwit,
            avg_size: 900_000.0,
            ..Default::default()
        };
        let opt = super::super::segwit_adoption_chart_daily(&[
            day("2024-04-01", 10, 2.0, 0.5),
            day("2024-04-02", 100, 11.0, 10.0),
        ]);
        let data = opt["series"][0]["data"].as_array().expect("data");
        assert_eq!(data[0].as_f64().unwrap(), 50.0, "5 of 10 non-coinbase");
        assert_eq!(
            data[1].as_f64().unwrap(),
            100.0,
            "1,000 of 1,000 non-coinbase, on a day 10 times the size"
        );
    }

    /// A cumulative total accumulates **inside the window** and a daily total
    /// does not, so dropping the window's first day tells them apart.
    ///
    /// The scaling probe cannot: doubling every day's block count doubles
    /// both, which is why `DailyTotal` and `CumulativeInWindow` share an
    /// expectation there. This is the property that separates them, and it is
    /// the declaration's own words: a cumulative point is the sum of
    /// everything loaded up to it, so removing the first day moves every
    /// later point down, while a daily point is a property of its own day and
    /// does not move at all.
    ///
    /// Matched by date rather than by index, since the two builds are
    /// different lengths.
    #[test]
    fn dropping_the_first_day_moves_a_cumulative_total_and_nothing_else() {
        use registry::Aggregation::*;
        let days = synthetic_days(120);
        let by_date = |opt: &serde_json::Value,
                       dates: &[String]|
         -> Vec<(String, String, Option<f64>)> {
            let cats: Vec<String> = opt["xAxis"]["data"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap_or_default().to_string())
                        .collect()
                })
                .unwrap_or_else(|| dates.to_vec());
            let mut out = Vec::new();
            for s in opt["series"].as_array().unwrap_or(&Vec::new()) {
                let name = s["name"].as_str().unwrap_or_default().to_string();
                for (i, p) in s["data"]
                    .as_array()
                    .map(|d| d.as_slice())
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    let v = match p {
                        serde_json::Value::Number(n) => n.as_f64(),
                        serde_json::Value::Array(a) => {
                            a.get(1).and_then(|v| v.as_f64())
                        }
                        serde_json::Value::Object(o) => {
                            o.get("value").and_then(|v| v.as_f64())
                        }
                        _ => None,
                    };
                    if let Some(date) = cats.get(i) {
                        out.push((name.clone(), date.clone(), v));
                    }
                }
            }
            out
        };
        let all_dates: Vec<String> =
            days.iter().map(|d| d.date.clone()).collect();
        // Chain Size is here as well as the dashboard charts, because it is
        // the catalog's other cumulative one and skipping it would leave the
        // "cumulative" side of this property resting on a single chart.
        let build = |meta: &ChartMeta,
                     rows: &[DailyAggregate]|
         -> Option<serde_json::Value> {
            match meta.source {
                Source::Dashboard {
                    daily: Daily::Fn(f),
                    ..
                } => Some(f(rows)),
                Source::ChainSize => {
                    Some(super::super::chain_size_chart_daily(
                        rows,
                        620.0,
                        0,
                        785_000_000_000,
                    ))
                }
                _ => None,
            }
        };
        let mut cumulative_checked = 0usize;
        let mut daily_checked = 0usize;
        for meta in registry::CHARTS.iter() {
            let kinds: std::collections::BTreeSet<&str> = meta
                .measurements
                .iter()
                .map(|m| match m.daily {
                    CumulativeInWindow => "cumulative",
                    DailyTotal | MeanOfPerBlockValues | RatioOfTotals => {
                        "per day"
                    }
                    _ => "unpredicted",
                })
                .collect();
            if kinds.len() != 1 {
                continue;
            }
            let kind = *kinds.iter().next().expect("one kind");
            if kind == "unpredicted" {
                continue;
            }
            let (Some(a_opt), Some(b_opt)) =
                (build(meta, &days), build(meta, &days[1..]))
            else {
                continue;
            };
            let full = by_date(&a_opt, &all_dates);
            let short = by_date(&b_opt, &all_dates[1..]);
            let mut moved = 0usize;
            let mut compared = 0usize;
            for (name, date, a) in &full {
                let Some((_, _, b)) =
                    short.iter().find(|(n, d, _)| n == name && d == date)
                else {
                    continue;
                };
                let (Some(a), Some(b)) = (a, b) else { continue };
                if a.abs() < 1e-12 {
                    continue;
                }
                compared += 1;
                if (a - b).abs() > 0.01 * a.abs() {
                    moved += 1;
                }
            }
            if compared == 0 {
                continue;
            }
            match kind {
                "cumulative" => {
                    assert!(
                        moved > 0,
                        "{} declares a cumulative total, but dropping the \
                         window's first day changed none of its {compared} \
                         points, so it is not accumulating inside the window",
                        meta.slug
                    );
                    cumulative_checked += 1;
                }
                _ => {
                    assert_eq!(
                        moved, 0,
                        "{} declares a per-day quantity, but {moved} of its \
                         {compared} points changed when an earlier day was \
                         dropped, which is what a cumulative total does",
                        meta.slug
                    );
                    daily_checked += 1;
                }
            }
        }
        assert!(
            cumulative_checked > 0 && daily_checked >= 5,
            "the property only bites if both sides are present: \
             {cumulative_checked} cumulative and {daily_checked} per-day \
             charts were compared"
        );
    }

    /// Address Type Evolution's daily point is a total, which is the finding
    /// behind its declaration: the subtitle said "Daily average output types"
    /// and the builder multiplies the stored mean back up by the block count.
    ///
    /// 3.0 per block over 10 blocks is **30**, not 3.0. One assertion, and the
    /// wrong alternative is an order of magnitude away.
    #[test]
    fn address_types_daily_plots_totals_not_averages() {
        let days = vec![DailyAggregate {
            date: "2024-04-01".to_string(),
            block_count: 10,
            avg_tx_count: 5.0,
            avg_p2pkh_count: 3.0,
            avg_p2tr_count: 1.5,
            avg_size: 900_000.0,
            ..Default::default()
        }];
        let opt = super::super::address_type_chart_daily(&days);
        let series = opt["series"].as_array().expect("series");
        let find = |name: &str| -> f64 {
            series
                .iter()
                .find(|s| s["name"] == name)
                .unwrap_or_else(|| panic!("no series {name}"))["data"][0]
                .as_f64()
                .expect("a number")
        };
        assert_eq!(find("P2PKH"), 30.0, "3.0 per block over 10 blocks");
        assert_eq!(find("P2TR"), 15.0, "1.5 per block over 10 blocks");
    }

    /// A halving day takes the **whole date** at the new subsidy, which is a
    /// step at the wrong moment rather than a blend of the two eras.
    ///
    /// One of the four declarations the round-one review found wrong, and the
    /// two that can be pinned by a fixture are pinned here rather than left
    /// as corrected prose. `daily_subsidy_btc` is `50 / 2^era_for_date` with
    /// the era read off the date, so 2020-05-11 is entirely 6.25 BTC even
    /// though the halving landed at block 630,000 part-way through it.
    ///
    /// Each wrong alternative lands somewhere else: a true mean of that day's
    /// 157 stored blocks is 11.544586 BTC, and an even split of the two eras
    /// would be 9.375. The day before must still be 12.5, or the boundary is
    /// off by one.
    #[test]
    fn a_halving_day_takes_the_whole_dates_subsidy() {
        assert_eq!(super::super::daily_subsidy_btc("2020-05-10"), 12.5);
        assert_eq!(
            super::super::daily_subsidy_btc("2020-05-11"),
            6.25,
            "the halving date is attributed entirely to the new era"
        );
        assert_eq!(super::super::daily_subsidy_btc("2020-05-12"), 6.25);
        // And the chart that reads it plots that, rather than a mean of the
        // blocks actually mined under each subsidy.
        let days: Vec<DailyAggregate> = ["2020-05-10", "2020-05-11"]
            .iter()
            .map(|d| DailyAggregate {
                date: (*d).to_string(),
                block_count: 157,
                total_fees: 100_000_000,
                avg_size: 900_000.0,
                ..Default::default()
            })
            .collect();
        let opt = super::super::subsidy_vs_fees_chart_daily(&days);
        let subsidy = opt["series"]
            .as_array()
            .expect("series")
            .iter()
            .find(|s| s["name"] == "Subsidy")
            .expect("a Subsidy series");
        assert_eq!(subsidy["data"][0].as_f64(), Some(12.5));
        assert_eq!(
            subsidy["data"][1].as_f64(),
            Some(6.25),
            "not 9.375, which is what blending the halving day would give"
        );
    }

    /// Inscription Block Share divides **bytes by bytes**, so it does not
    /// move with transaction or item counts at all.
    ///
    /// The second of the four round-one declaration errors, which said it was
    /// matching witness items over transactions. A fixture that varies the
    /// counts while holding the bytes fixed is what separates the two
    /// readings: 200 envelope bytes in a 1,000-byte block is 20% whether the
    /// block holds one inscription or fifty.
    #[test]
    fn inscription_block_share_is_bytes_over_bytes() {
        let block = |inscriptions: u64, tx_count: u64| BlockSummary {
            height: 800_000,
            timestamp: 1_700_000_000,
            size: 1_000,
            inscription_envelope_bytes: 200,
            inscription_count: inscriptions,
            tx_count,
            ..Default::default()
        };
        let share = |b: BlockSummary| -> f64 {
            let opt = super::super::inscription_share_chart(&[b]);
            opt["series"][0]["data"][0][1]
                .as_f64()
                .expect("a plotted share")
        };
        assert_eq!(share(block(1, 2)), 20.0, "200 bytes of a 1,000-byte block");
        assert_eq!(
            share(block(50, 3_000)),
            20.0,
            "fifty inscriptions across 3,000 transactions carrying the same \
             200 bytes is the same share; if this moved, the chart is \
             counting items or transactions"
        );
    }

    /// Percentiles are not additive, so the chart must not stack them.
    ///
    /// The spec's own acceptance case: for percentiles 1, 2, 3, 4 and 5 the p90
    /// boundary is 5, never 15. Stacked, every value a reader could take off
    /// the plot above the lowest band was a cumulative sum of quantiles.
    #[test]
    fn fee_percentiles_are_not_stacked_or_summed() {
        let blocks: Vec<BlockSummary> = (0..400)
            .map(|i| BlockSummary {
                height: 800_000 + i,
                hash: format!("{i:064x}"),
                timestamp: 1_700_000_000 + i * 600,
                tx_count: 2_000,
                size: 1_200_000,
                weight: 3_900_000,
                // The discriminating vector: 1, 2, 3, 4, 5.
                fee_rate_p10: 1.0,
                fee_rate_p25: 2.0,
                median_fee_rate: 3.0,
                fee_rate_p75: 4.0,
                fee_rate_p90: 5.0,
                ..Default::default()
            })
            .collect();

        let opt = super::super::fee_rate_heatmap_chart(&blocks);
        let series = opt["series"].as_array().expect("series");

        for s in series {
            assert!(
                s.get("stack").is_none(),
                "{:?} is stacked, so its drawn height is a running sum of \
                 quantiles rather than a quantile",
                s["name"]
            );
        }

        let value_of = |name: &str| -> f64 {
            let s = series
                .iter()
                .find(|s| s["name"] == name)
                .unwrap_or_else(|| panic!("no series {name}"));
            s["data"][0][1].as_f64().expect("a number")
        };
        // Each series carries its own percentile. Stacked, these would have
        // read 1, 3, 6, 10 and 15.
        assert_eq!(value_of("p10"), 1.0);
        assert_eq!(value_of("p25"), 2.0);
        assert_eq!(value_of("Median"), 3.0);
        assert_eq!(value_of("p75"), 4.0);
        assert_eq!(
            value_of("p90"),
            5.0,
            "the top boundary is p90, not the sum"
        );
    }

    /// An absent reading is not a measurement of zero.
    ///
    /// Three places wrote one. TPS has no interval for the first block in a
    /// range and no usable interval when a miner stamps a block at or before
    /// its parent's time; both were plotted as zero transactions per second,
    /// which says throughput stopped when in fact the clock did. SegWit
    /// Adoption divided by the day's non-coinbase transactions and wrote 0%
    /// when a day had none, which reads as "nobody used witness inputs"
    /// rather than "there were no transactions".
    #[test]
    fn an_absent_reading_is_a_gap_not_a_zero() {
        // TPS: a backward stamp in the middle, and no predecessor at the start.
        let blocks: Vec<BlockSummary> = [0u64, 600, 300, 900]
            .iter()
            .enumerate()
            .map(|(i, off)| BlockSummary {
                height: 800_000 + i as u64,
                hash: format!("{i:064x}"),
                timestamp: 1_700_000_000 + off,
                tx_count: 3_000,
                size: 1_200_000,
                weight: 3_900_000,
                ..Default::default()
            })
            .collect();
        let opt = super::super::tps_chart(&blocks);
        let d = opt["series"][0]["data"].as_array().expect("data");
        assert_eq!(d.len(), 4, "every block keeps its slot");
        assert!(
            d[0][1].is_null(),
            "first block has no predecessor: {:?}",
            d[0]
        );
        assert!(
            d[1][1].as_f64().expect("a rate") > 0.0,
            "600s gap is a rate"
        );
        assert!(d[2][1].is_null(), "backward stamp is not zero tps");
        assert!(d[3][1].as_f64().expect("a rate") > 0.0);
        // The height stays so the tooltip can still name the block.
        assert_eq!(d[2][2].as_u64(), Some(800_002));

        // SegWit: a day of coinbase-only blocks has no denominator.
        let day = |avg_tx: f64, avg_seg: f64| DailyAggregate {
            date: "2011-02-03".to_string(),
            block_count: 100,
            avg_tx_count: avg_tx,
            avg_segwit_spend_count: avg_seg,
            avg_size: 285.0,
            ..Default::default()
        };
        let opt = super::super::segwit_adoption_chart_daily(&[
            day(1.0, 0.0),
            day(2.0, 0.5),
        ]);
        let d = opt["series"][0]["data"].as_array().expect("data");
        assert!(
            d[0].is_null(),
            "no user transactions, so no share: {:?}",
            d[0]
        );
        assert_eq!(d[1].as_f64(), Some(50.0), "0.5 of 1.0 non-coinbase");
    }

    /// The Taproot output count must not be read from the column that calls
    /// itself a spend count.
    ///
    /// `taproot_spend_count` is byte-identical to `p2tr_count` in all 967,187
    /// stored rows, so the values never differed and the error was purely one
    /// of naming: a chart, a modal row and a hall-of-fame record all described
    /// created outputs as spent inputs. The record was arithmetically
    /// impossible, claiming 22,367 Taproot inputs in a block holding 327.
    ///
    /// This fixture makes them differ so a consumer reading the wrong one
    /// fails. It cannot happen in stored data, which is exactly why nothing
    /// caught it.
    #[test]
    fn taproot_charts_read_the_output_count_not_the_spend_count() {
        let blocks: Vec<BlockSummary> = (0..300)
            .map(|i| BlockSummary {
                height: 840_000 + i,
                hash: format!("{i:064x}"),
                timestamp: 1_713_000_000 + i * 600,
                tx_count: 200,
                size: 1_400_000,
                weight: 3_900_000,
                p2tr_count: 5_000,
                // Deliberately different, which stored rows never are.
                taproot_spend_count: 7,
                ..Default::default()
            })
            .collect();
        let opt = super::super::taproot_chart(&blocks);
        let first = opt["series"][0]["data"][0][1].as_f64().expect("a number");
        assert_eq!(
            first, 5_000.0,
            "read p2tr_count; {first} means the misnamed column is still wired \
             in"
        );
    }

    /// Overlapping detector totals must not be stacked, because their sum is
    /// not a quantity.
    ///
    /// Ingestion credits a transaction's whole fee to every detector it
    /// matches, and 125,180 stored blocks have both firing. In 1,326 of them
    /// the two totals exceed the block's entire fee take, which the old
    /// residual band hid by flooring at zero. A reader could therefore take a
    /// share-of-total figure off the chart that did not exist.
    ///
    /// The fixture makes the overlap total: one transaction's fee counted in
    /// full by both detectors. Stacked, the plot would show 1.5 BTC of fees in
    /// a block that collected 1.0.
    #[test]
    fn overlapping_protocol_fee_detectors_are_not_stacked() {
        let blocks: Vec<BlockSummary> = (0..300)
            .map(|i| BlockSummary {
                height: 840_000 + i,
                hash: format!("{i:064x}"),
                timestamp: 1_713_000_000 + i * 600,
                tx_count: 500,
                size: 1_400_000,
                weight: 3_900_000,
                total_fees: 100_000_000,
                // The same 0.75 BTC counted by both.
                inscription_fees: 75_000_000,
                runes_fees: 75_000_000,
                ..Default::default()
            })
            .collect();

        for (label, opt) in [
            (
                "protocol-fees",
                super::super::protocol_fee_breakdown_chart(&blocks),
            ),
            (
                "protocol-fee-competition",
                super::super::protocol_fee_competition_chart(&blocks),
            ),
        ] {
            let series = opt["series"].as_array().expect("series");
            for s in series {
                assert!(
                    s.get("stack").is_none(),
                    "{label}: {:?} is stacked, so the plot sums two totals \
                     that can count the same fee twice",
                    s["name"]
                );
            }
            let names: Vec<&str> =
                series.iter().filter_map(|s| s["name"].as_str()).collect();
            assert_eq!(
                names,
                vec!["Inscriptions", "Runes"],
                "{label}: the residual band cannot be recovered from stored \
                 data and must not be drawn"
            );
        }
    }

    /// The four share charts use four different denominators, and each has to
    /// keep the one it declares.
    ///
    /// They answer different questions, so the differences are not defects in
    /// themselves. What would be a defect is drift: a reader comparing P2PKH
    /// across two charts is already seeing two numbers, and the declarations
    /// now say why. This pins each denominator so a builder cannot quietly
    /// adopt another chart's.
    ///
    /// The fixture puts real values in the categories each denominator
    /// includes or excludes, which is what separates them. A block of 1,000
    /// outputs: 100 P2PKH, 100 P2SH, 200 P2WPKH, 100 P2WSH, 300 P2TR, 20 P2PK,
    /// 50 bare multisig, 30 unrecognised, and the remaining 100 OP_RETURN and
    /// other outputs that no classifier counts.
    ///
    /// - Address Type Share divides by the six payment types, 820, so its
    ///   bands total 100 by construction.
    /// - P2PKH Sunset divides by eight, 900, so P2PKH reads 11.11 rather than
    ///   the 12.20 the six-type denominator would give.
    /// - Witness Version Share divides by native v0 plus Taproot, 600, so
    ///   Taproot reads 50.
    /// - Output Type Breakdown divides by every output, 1,000, so Taproot
    ///   reads 30 and the Legacy residual absorbs the rest.
    #[test]
    fn each_share_chart_keeps_the_denominator_it_declares() {
        let blocks: Vec<BlockSummary> = (0..400)
            .map(|i| BlockSummary {
                height: 850_000 + i,
                hash: format!("{i:064x}"),
                timestamp: 1_720_000_000 + i * 600,
                tx_count: 500,
                size: 1_400_000,
                weight: 3_900_000,
                output_count: 1_000,
                p2pkh_count: 100,
                p2sh_count: 100,
                p2wpkh_count: 200,
                p2wsh_count: 100,
                p2tr_count: 300,
                p2pk_count: 20,
                multisig_count: 50,
                unknown_script_count: 30,
                op_return_count: 60,
                ..Default::default()
            })
            .collect();

        let first = |opt: &serde_json::Value, name: &str| -> f64 {
            let s = opt["series"]
                .as_array()
                .expect("series")
                .iter()
                .find(|s| s["name"] == name)
                .unwrap_or_else(|| panic!("no series {name}"));
            let p = &s["data"][0];
            match p {
                serde_json::Value::Array(a) => a[1].as_f64().expect("a number"),
                other => other.as_f64().expect("a number"),
            }
        };

        // Six payment types: 100+100+200+100+300+20 = 820.
        let pct = super::super::address_type_pct_chart(&blocks);
        assert!(
            (first(&pct, "P2PKH") - 100.0 / 820.0 * 100.0).abs() < 0.02,
            "Address Type Share must divide by the six payment types, got {}",
            first(&pct, "P2PKH")
        );

        // Eight classified types: 820 + 50 + 30 = 900.
        let sunset = super::super::address_sunset_chart(&blocks);
        assert!(
            (first(&sunset, "P2PKH %") - 100.0 / 900.0 * 100.0).abs() < 0.02,
            "P2PKH Sunset must divide by eight classified types, got {}",
            first(&sunset, "P2PKH %")
        );

        // Witness outputs only: 300 native v0 + 300 Taproot = 600.
        let wit = super::super::witness_version_pct_chart(&blocks);
        assert!(
            (first(&wit, "Taproot") - 50.0).abs() < 0.02,
            "Witness Version Share must divide by witness outputs alone, \
             got {}",
            first(&wit, "Taproot")
        );

        // Every output: 1,000.
        let brk = super::super::witness_version_tx_pct_chart(&blocks);
        assert!(
            (first(&brk, "Taproot") - 30.0).abs() < 0.02,
            "Output Type Breakdown must divide by every output, got {}",
            first(&brk, "Taproot")
        );
        // And its residual absorbs everything the other two bands miss.
        assert!(
            (first(&brk, "Other outputs") - 40.0).abs() < 0.02,
            "the residual must carry the 400 outputs that are neither native \
             v0 nor Taproot, got {}",
            first(&brk, "Other outputs")
        );
    }

    /// A share must not change because the range changed.
    ///
    /// This is the defect CQ-11 reported and I initially dismissed, having
    /// read the daily denominator and assumed the per-block arm matched.
    /// `p2pkh-sunset` divided by six classified output types per block and
    /// eight daily, and `multi-velocity` did the same, so switching range
    /// moved the line on identical data: P2PKH read 12.20% per block and
    /// 11.11% daily. Both per-block arms now use eight.
    ///
    /// The fixture gives a day whose per-block averages are exactly one
    /// block's counts, so the two resolutions are looking at the same chain
    /// and any difference is arithmetic rather than data.
    #[test]
    fn a_share_does_not_move_when_the_resolution_does() {
        let block = BlockSummary {
            height: 850_000,
            hash: "a".repeat(64),
            timestamp: 1_720_000_000,
            tx_count: 500,
            size: 1_400_000,
            weight: 3_900_000,
            output_count: 1_000,
            p2pkh_count: 100,
            p2sh_count: 100,
            p2wpkh_count: 200,
            p2wsh_count: 100,
            p2tr_count: 300,
            p2pk_count: 20,
            multisig_count: 50,
            unknown_script_count: 30,
            ..Default::default()
        };
        let blocks: Vec<BlockSummary> = (0..400)
            .map(|i| BlockSummary {
                height: 850_000 + i,
                hash: format!("{i:064x}"),
                timestamp: 1_720_000_000 + i * 600,
                ..block.clone()
            })
            .collect();
        let days: Vec<DailyAggregate> = (0..120)
            .map(|i| DailyAggregate {
                date: format!(
                    "{}",
                    chrono::NaiveDate::from_ymd_opt(2024, 6, 1).expect("valid")
                        + chrono::Duration::days(i)
                ),
                block_count: 144,
                avg_tx_count: block.tx_count as f64,
                avg_size: block.size as f64,
                avg_weight: block.weight as f64,
                avg_output_count: block.output_count as f64,
                avg_p2pkh_count: block.p2pkh_count as f64,
                avg_p2sh_count: block.p2sh_count as f64,
                avg_p2wpkh_count: block.p2wpkh_count as f64,
                avg_p2wsh_count: block.p2wsh_count as f64,
                avg_p2tr_count: block.p2tr_count as f64,
                avg_p2pk_count: block.p2pk_count as f64,
                avg_multisig_count: block.multisig_count as f64,
                avg_unknown_script_count: block.unknown_script_count as f64,
                ..Default::default()
            })
            .collect();

        let last = |opt: &serde_json::Value, name: &str| -> f64 {
            let s = opt["series"]
                .as_array()
                .expect("series")
                .iter()
                .find(|s| s["name"] == name)
                .unwrap_or_else(|| panic!("no series {name}"));
            let d = s["data"].as_array().expect("data");
            let p = d.last().expect("a point");
            match p {
                serde_json::Value::Array(a) => a[1].as_f64().expect("a number"),
                other => other.as_f64().expect("a number"),
            }
        };

        for (label, pb, dl, series) in [
            (
                "p2pkh-sunset",
                super::super::address_sunset_chart(&blocks),
                super::super::address_sunset_chart_daily(&days),
                "P2PKH %",
            ),
            (
                "address-types-pct",
                super::super::address_type_pct_chart(&blocks),
                super::super::address_type_pct_chart_daily(&days),
                "P2PKH",
            ),
            (
                "witness-pct",
                super::super::witness_version_pct_chart(&blocks),
                super::super::witness_version_pct_chart_daily(&days),
                "Taproot",
            ),
            (
                "witness-tx-pct",
                super::super::witness_version_tx_pct_chart(&blocks),
                super::super::witness_version_tx_pct_chart_daily(&days),
                "Taproot",
            ),
        ] {
            let a = last(&pb, series);
            let b = last(&dl, series);
            assert!(
                (a - b).abs() < 0.02,
                "{label}: {series} reads {a} per block and {b} daily on the \
                 same chain, so the two resolutions divide by different \
                 populations"
            );
        }
    }

    /// Every byte unit on the site is decimal, and the symbol matches.
    ///
    /// kB is 1,000 bytes and KiB is 1,024, a 2.4% difference that compounds to
    /// 4.9% at MB and 7.4% at GB. `inscription-envelope` divided by 1,024 and
    /// labelled the result KB, which is the one place the two were mixed, and
    /// it was the only chart doing so: block size, chain size, largest
    /// transaction and the OP_RETURN volume helpers were already decimal.
    ///
    /// Asserted by arithmetic rather than by grepping for a divisor, so it
    /// holds however the conversion is written. 2,048 bytes is 2.048 kB
    /// decimal and would be exactly 2 KiB binary, which is what makes it the
    /// discriminating input.
    #[test]
    fn byte_units_are_decimal_not_binary() {
        let blocks: Vec<BlockSummary> = (0..400)
            .map(|i| BlockSummary {
                height: 850_000 + i,
                hash: format!("{i:064x}"),
                timestamp: 1_720_000_000 + i * 600,
                tx_count: 100,
                size: 2_048_000,
                weight: 3_900_000,
                inscription_count: 5,
                inscription_bytes: 2_048,
                inscription_envelope_bytes: 3_048,
                ..Default::default()
            })
            .collect();

        let opt = super::super::inscription_envelope_chart(&blocks);
        let payload = opt["series"]
            .as_array()
            .expect("series")
            .iter()
            .find(|s| s["name"] == "Payload")
            .expect("a Payload series")["data"][0][1]
            .as_f64()
            .expect("a number");
        assert!(
            (payload - 2.048).abs() < 0.001,
            "2,048 bytes is 2.048 kB; {payload} means the divisor is 1,024, \
             which would be KiB under a kB label"
        );

        // And block size, which was already decimal, stays that way.
        let size = super::super::block_size_chart(&blocks);
        let mb = size["series"][0]["data"][0][1].as_f64().expect("a number");
        assert!(
            (mb - 2.048).abs() < 0.001,
            "2,048,000 bytes is 2.048 MB decimal, got {mb}"
        );

        // The symbol has to match the arithmetic.
        assert_eq!(registry::Unit::Kilobytes.label(), "kB");
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
                (Shape::Gauge, Kpis::NotSummarizable)
                | (Shape::Donut | Shape::Histogram, Kpis::Categorical { .. })
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
                (_, Kpis::NotSummarizable) => suppressed.push(at),
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
            // Joined when the percentile bands were unstacked. As a stacked
            // chart its rail reported "latest total" as the sum of five
            // quantiles, which is the same defect the chart had; five
            // percentile lines have no single average or peak, so the rail
            // says nothing instead of something false.
            "fee-heatmap (per block)",
            "halving-era (daily)",
            "halving-era (per block)",
            "multi-velocity (daily)",
            "multi-velocity (per block)",
            // Joined when the protocol fee bands were unstacked. Stacked,
            // their rail reported "latest total" as the sum of two detector
            // totals that can both count the same transaction's fee, which is
            // the defect the chart had. Two overlapping detector totals have
            // no single average or peak.
            "protocol-fee-competition (daily)",
            "protocol-fee-competition (per block)",
            "protocol-fees (per block)",
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

    /// A chart whose points are already changes must not report a change.
    ///
    /// The rail's figure is `last - first`, which answers "how much did this
    /// move across the range" and needs a level. Where every point is itself
    /// a difference, the subtraction is a change of a change, and the sign
    /// alone can mislead: two retargets of +10% and +5% leave difficulty
    /// 15.5% higher than it started, while `last - first` is **-5**.
    ///
    /// Found in the browser on 2026-09-16, where the 1Y rail read
    /// "change -3.32, -71.7%" over a year in which difficulty fell 6.3%.
    /// Nothing in this suite would have caught it, which is why the list is
    /// now checked here.
    #[test]
    fn a_chart_of_changes_does_not_report_a_change() {
        use super::super::kpi::{self, Kpis};

        // Both directions, computed from the declarations rather than
        // restated, because a list that only checks its own entries cannot
        // catch a missing one. That is the hole this suite found in its own
        // coverage check a day earlier: iterating the list proves nothing
        // about what is absent from it.
        //
        // A declared quantity is the only place "this point is a difference"
        // is written down, so the word is the test. It is a proxy in the same
        // way `is_companion_series` matching " ma" is a proxy: if a future
        // chart of changes words its quantity around the word, this fails
        // loudly at the list rather than silently at the rail.
        let by_declaration: std::collections::BTreeSet<&str> = registry::CHARTS
            .iter()
            .filter(|c| {
                c.measurements
                    .iter()
                    .any(|m| m.quantity.to_ascii_lowercase().contains("change"))
            })
            .map(|c| c.slug)
            .collect();
        let listed: std::collections::BTreeSet<&str> =
            registry::POINTS_ARE_CHANGES.iter().copied().collect();
        assert_eq!(
            by_declaration, listed,
            "every chart whose declared quantity is a change has to be in \
             POINTS_ARE_CHANGES, and nothing else may be: the rail's \
             `last - first` is a change of a change for exactly this set"
        );

        let mut load_bearing = 0usize;
        for slug in registry::POINTS_ARE_CHANGES {
            let meta = registry::find(slug)
                .unwrap_or_else(|| panic!("{slug} is not a registered chart"));
            assert!(
                !meta.reports_change(),
                "{slug} is listed as plotting changes but still reports one"
            );
            // The suppression has to be reachable. Either the chart draws the
            // series rail, where the tile would otherwise appear, or its rail
            // is already silent *for the one other reason a chart of changes
            // can be silent*: several measurements at the same x, which is
            // what `MULTI_METRIC` names. Any other silence means this entry
            // describes a chart that never had the tile, and listing it
            // implies a fix that did nothing.
            for (m, daily, opt) in built_charts() {
                if m.slug != *slug {
                    continue;
                }
                let json = serde_json::to_string(&opt).expect("serialisable");
                let at = if daily { "daily" } else { "per block" };
                match kpi::compute(&json, m.shape) {
                    Kpis::Series { .. } => load_bearing += 1,
                    Kpis::NotSummarizable
                        if registry::MULTI_METRIC.contains(&m.slug) => {}
                    k => panic!(
                        "{slug} ({at}) produced {k:?}, which has no change \
                         tile to suppress, so listing it is misleading"
                    ),
                }
            }
        }
        assert!(
            load_bearing > 0,
            "no chart in POINTS_ARE_CHANGES routes through the series rail, \
             so the list suppresses nothing and is dead weight"
        );

        // And the figure it suppresses really is the misleading one. Two
        // rises, so the rail's own first and last are +10 and +5.
        let days = days_from_conformance("2024-01-01", 40);
        let retargets = vec![
            Retarget {
                height: 800_000,
                timestamp: at_noon("2024-01-02"),
                difficulty: 1.0e14,
            },
            Retarget {
                height: 802_016,
                timestamp: at_noon("2024-01-16"),
                difficulty: 1.1e14,
            },
            Retarget {
                height: 804_032,
                timestamp: at_noon("2024-01-30"),
                difficulty: 1.155e14,
            },
        ];
        let opt =
            super::super::difficulty_adjustment_chart_daily(&days, &retargets);
        let json = serde_json::to_string(&opt).expect("serialisable");
        let Kpis::Series { first, last, .. } =
            kpi::compute(&json, registry::Shape::Bar)
        else {
            panic!("the fixture should reach the series rail");
        };
        assert!(
            (first - 10.0).abs() < 0.01,
            "first bar is +10%, got {first}"
        );
        assert!((last - 5.0).abs() < 0.01, "last bar is +5%, got {last}");
        // The suppressed figure against the honest one, both computed rather
        // than written down, so the contrast cannot go stale if the fixture
        // changes.
        let suppressed = last - first;
        let compound = (retargets.last().expect("retargets").difficulty
            / retargets.first().expect("retargets").difficulty
            - 1.0)
            * 100.0;
        assert!(
            suppressed < 0.0 && compound > 0.0,
            "the point of the suppression is that these disagree in sign: \
             the rail would have shown {suppressed} while difficulty moved \
             {compound}% across the same window"
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
        // Its own fixtures rather than `built_charts()`, which is now
        // complete and would do. Kept separate because this dump is the G0
        // inventory and its columns are read by hand against the notes from
        // that pass; the shared harness is free to change shape without
        // invalidating them.
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
