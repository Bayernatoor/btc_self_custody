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
                // The six script-type columns were left at their default of
                // zero, so every address-type chart was built from nothing and
                // its conformance check passed on an empty fixture: six
                // registered charts had no real test at all.
                //
                // These sum to exactly `output_count - unknown_script_count`
                // (2,599 + 3i against 2,600 + 4i outputs less 1 + i unknown),
                // so any residual band computed as outputs-minus-the-parts is
                // zero rather than negative, and a chart that draws one is
                // exercised rather than crashed.
                p2pk_count: 9,
                p2pkh_count: 800 + i,
                p2sh_count: 400 + i,
                p2wpkh_count: 900 + i,
                p2wsh_count: 200,
                p2tr_count: 290,
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
                // Large enough, and moving fast enough, to discriminate.
                // At `2.0 + f * 0.01` this was a 0.09% band whose share
                // barely moved, so Adoption Velocity's sum-to-zero check
                // passed with the series deleted AND with the term dropped
                // from the denominator: the error sat far below any sane
                // tolerance. A fixture value can be present and still make an
                // assertion vacuous if it is too small to move the result.
                avg_unknown_script_count: 40.0 + f * 1.5,
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
            // six charts sat outside every conformance guard.
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
    /// A card for a chart with no daily builder says which range to pick.
    ///
    /// `a_declared_daily_builder_actually_builds_something` cannot catch this,
    /// because the registry was honest: all nine of these declare
    /// `Daily::Unavailable`. It was the **pages** that diverged. The single
    /// chart view drew the notice naming the range, while the category card
    /// fell through to a bare "No data in the selected range", so the same
    /// chart explained itself on its own page and left the reader guessing in
    /// its card. Two of the nine reached it through a function named
    /// `*_chart_daily` that only ever returned the generic frame, and one of
    /// those put its reason inside the chart title. Found on the H-03
    /// acceptance row on 2026-09-21.
    ///
    /// The hint is specific to this cause on purpose. `no_data_chart`'s own
    /// documentation records that a blanket "select a shorter range" was
    /// removed for being shown when resolution was not the reason at all;
    /// here it is the reason, and it is known from the registry.
    #[test]
    fn a_chart_with_no_daily_builder_says_which_range_to_pick() {
        const PAGES: &[(&str, &str)] = &[
            (
                "network.rs",
                include_str!("../../routes/observatory/network.rs"),
            ),
            ("fees.rs", include_str!("../../routes/observatory/fees.rs")),
            (
                "mining.rs",
                include_str!("../../routes/observatory/mining.rs"),
            ),
            (
                "embedded.rs",
                include_str!("../../routes/observatory/embedded.rs"),
            ),
        ];
        for (name, src) in PAGES {
            for arm in src.split("|_days|").skip(1) {
                let head = &arm[..arm.len().min(120)];
                assert!(
                    !head.contains("no_data_chart("),
                    "{name} renders a bare no-data frame for a chart with no \
                     daily builder: {head:?}. Use no_daily_builder_chart, \
                     which names the range to pick. A reader told only that \
                     there is no data has to guess which range would have it."
                );
            }
        }
        // And the frame really carries the actionable half, so the check above
        // is not passing on the strength of a renamed function.
        let frame = super::super::no_daily_builder_chart("Anything");
        let subtext = frame["title"]["subtext"].as_str().unwrap_or_default();
        assert!(
            subtext.contains("Pick 1M or shorter"),
            "the no-daily-builder frame no longer names a range: {subtext:?}"
        );
    }

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

    /// Dump every chart's built option, both resolutions, as one JSON
    /// document for the browser pass.
    ///
    /// V-06 asks for all 63 entries in both views, which by hand is 126
    /// charts opened one at a time. The structural half of that is
    /// mechanical: does ECharts accept the option, how many series does it
    /// resolve, what does it make of the axes, does any point arrive as NaN.
    /// This emits the inputs for that so `runs/render-all.html` can answer it
    /// in one pass, leaving the reader to judge copy and layout, which is
    /// the half a machine cannot.
    ///
    /// Not an assertion. Run with:
    ///
    /// ```text
    /// cargo test --features ssr dump_all_options -- --ignored --nocapture \
    ///   > notes/chart-quality-2026-09-15/runs/all-options.json
    /// ```
    #[test]
    #[ignore]
    fn dump_all_options() {
        let mut out = serde_json::Map::new();
        for (meta, daily, opt) in built_charts() {
            let key = format!(
                "{}::{}",
                meta.slug,
                if daily { "daily" } else { "per-block" }
            );
            out.insert(
                key,
                serde_json::json!({
                    "slug": meta.slug,
                    "title": meta.title,
                    "daily": daily,
                    "category": format!("{:?}", meta.category),
                    "unit": format!("{:?}", meta.unit),
                    "shape": format!("{:?}", meta.shape),
                    "supports_log": meta.supports_log(),
                    "can_compare": meta.can_compare(),
                    "reports_change": meta.reports_change(),
                    "declared_series": meta
                        .measurements
                        .iter()
                        .map(|m| if daily && !m.series_daily.is_empty() {
                            m.series_daily
                        } else {
                            m.series
                        })
                        .collect::<Vec<_>>(),
                    "option": opt,
                }),
            );
        }
        println!(
            "{}",
            serde_json::to_string(&serde_json::Value::Object(out))
                .expect("serialisable")
        );
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
    /// resolutions, so the two methods are the same. Both are `Calculated`,
    /// because the plotted value is a ratio of two stored difficulties
    /// rather than a reading off either of them. A chart graduating out of
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
            // A constant over the day's block count, so doubling the day
            // halves the value. Block Interval is the only one, and naming
            // the shape is what lets it be checked at all.
            InverseOfDailyCount => Some(0.5),
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
                // Counted per series rather than per measurement. A
                // declaration with an empty name covers every series the
                // chart draws, so one comparable series was standing in for
                // all of them: `address-types` passed with an entirely zero
                // P2PK band because another output type supplied the
                // coverage. Found by the review of 2026-09-16.
                for si in mine {
                    let (name, _, sa) = &a[si];
                    let sb = &b[si].2;
                    let mut compared = 0usize;
                    // A series with no points at all is decoration: the
                    // threshold markers on P2PKH Sunset carry their values in
                    // `markLine`. That is the only exemption, and it is read
                    // off the series itself rather than granted because a
                    // sibling passed.
                    if sa.is_empty() && sb.is_empty() {
                        continue;
                    }
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
                    // Every non-empty series this declaration covers has
                    // to contain at least one discriminating point of its
                    // own.
                    assert!(
                        compared > 0,
                        "{} series {name:?} is covered by the {:?} \
                         declaration but holds nothing comparable, so that \
                         series was not checked; give the fixture non-zero \
                         values for the column it reads",
                        meta.slug,
                        m.daily
                    );
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
                // The same three checks the declared series get, rather than
                // the weaker loop this was. `zip` silently truncated, so a
                // companion could lose every point but its warmup nulls and
                // pass, and only numeric pairs were compared, so a warmup
                // null turning into a zero was invisible. Both were
                // reproduced by the review of 2026-09-16 against the real
                // `Outputs MA` series.
                let other = &b[i].2;
                assert_eq!(
                    points.len(),
                    other.len(),
                    "{} companion {name:?}: point count changed",
                    meta.slug
                );

                // A moving average has no value until its window fills, so
                // its first positions must be gaps. This is the one escape
                // scaling cannot see: substituting zero for the warmup
                // changes both runs together, so every factor still agrees
                // and the series is quietly claiming a reading it does not
                // have. Checked by name rather than by the declared marker,
                // because Chain Size's "Disk Size (est.)" is a companion
                // with no window and no warmup.
                let smoothed = name.to_ascii_lowercase();
                if smoothed.contains(" ma")
                    || smoothed.contains("moving average")
                    || smoothed.ends_with("ma")
                {
                    assert!(
                        points.first().is_some_and(|v| v.is_none()),
                        "{} companion {name:?} is a moving average, so its \
                         first point cannot have a value: a window needs \
                         more than one reading to fill. A zero here is a \
                         substituted gap",
                        meta.slug
                    );
                }
                let mut compared = 0usize;
                for (x, y) in points.iter().zip(other.iter()) {
                    match (x, y) {
                        (None, None) => {}
                        (None, Some(v)) | (Some(v), None) => panic!(
                            "{} companion {name:?}: a gap and a reading \
                             swapped places when the day doubled ({v} \
                             against nothing). A moving average's warmup \
                             nulls are the usual case here, and they must \
                             stay null",
                            meta.slug
                        ),
                        (Some(x), Some(y)) => {
                            if x.abs() < 1e-12 {
                                assert!(
                                    y.abs() < 1e-12,
                                    "{} companion {name:?}: a zero became \
                                     {y}",
                                    meta.slug
                                );
                                continue;
                            }
                            let factor = y / x;
                            assert!(
                                (factor - want).abs() < 0.01 * want,
                                "{} companion {name:?} smooths measurements \
                                 that move by {want}x, but {x} became {y}, \
                                 a factor of {factor}",
                                meta.slug
                            );
                            compared += 1;
                        }
                    }
                }
                assert!(
                    compared > 0,
                    "{} companion {name:?} holds nothing comparable, so it \
                     was not checked at all",
                    meta.slug
                );
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

    /// The audit's third CQ-09 point, checked rather than taken from a
    /// comment: a non-positive interval is a gap and not a rate of zero, and
    /// the first block of a window has no predecessor.
    #[test]
    fn per_block_tps_gaps_what_it_cannot_divide() {
        let mut blocks = synthetic_blocks(6);
        // A backward timestamp, which 16,020 real pairs have, and a repeat.
        blocks[2].timestamp = blocks[1].timestamp - 30;
        blocks[4].timestamp = blocks[3].timestamp;
        let opt = super::super::tps_chart(&blocks);
        let data = opt["series"][0]["data"].as_array().expect("data");
        let y = |i: usize| data[i].as_array().and_then(|p| p.get(1).cloned());
        assert!(
            y(0).is_some_and(|v| v.is_null()),
            "the first block has no predecessor: {:?}",
            y(0)
        );
        assert!(
            y(2).is_some_and(|v| v.is_null()),
            "a backward interval is not a rate of zero: {:?}",
            y(2)
        );
        assert!(
            y(4).is_some_and(|v| v.is_null()),
            "a zero interval is not a rate of zero: {:?}",
            y(4)
        );
        assert!(
            y(1).is_some_and(|v| v.as_f64().is_some()),
            "an ordinary interval still reports: {:?}",
            y(1)
        );
    }

    /// The two daily rate charts divide by a whole day, so a day still in
    /// progress is a gap rather than a reading.
    ///
    /// CQ-09. Block interval is `1440 / block_count` and TPS is the day's
    /// transactions over 86,400 seconds, and a named range ends on today.
    ///
    /// The **first** day is kept. An earlier version withheld it as well, on
    /// the reasoning that a named range starts mid-morning, but
    /// `query_daily_aggregates_fast` reads `daily_blocks` by date and returns
    /// that day complete. The review of 2026-09-16 caught the claim.
    ///
    /// The interval chart also filtered days with fewer than 50 blocks **out
    /// of the category axis**, which joined the line across missing dates,
    /// made its 7-day average mean seven surviving days, and hid the 40 days
    /// of 2009 where the interval genuinely ran long. Those days are back.
    #[test]
    fn a_daily_rate_withholds_the_day_in_progress_and_keeps_every_date() {
        // Ten days, one of them a 2009-style slow day in the interior.
        let mut days = days_from_conformance("2009-01-05", 10);
        for (i, d) in days.iter_mut().enumerate() {
            d.block_count = if i == 4 { 12 } else { 144 };
            d.avg_tx_count = 2.0;
        }
        let slow_date = days[4].date.clone();

        for (slug, opt) in [
            (
                "block-interval",
                super::super::block_interval_chart_daily(&days),
            ),
            ("tps", super::super::tps_chart_daily(&days)),
        ] {
            let cats: Vec<&str> = opt["xAxis"]["data"]
                .as_array()
                .expect("category axis")
                .iter()
                .map(|v| v.as_str().unwrap_or_default())
                .collect();
            assert_eq!(
                cats.len(),
                10,
                "{slug} dropped days from the axis: {cats:?}"
            );
            assert!(
                cats.contains(&slow_date.as_str()),
                "{slug} dropped the slow day, which is a real reading"
            );

            let data = opt["series"][0]["data"].as_array().expect("data");
            assert_eq!(data.len(), 10, "{slug}: a point per day");
            assert!(
                data[9].is_null(),
                "{slug}: the final day may still be in progress, so it is \
                 divided by an elapsed time it does not have: {data:?}"
            );
            for (i, v) in data.iter().enumerate().take(9) {
                assert!(
                    v.as_f64().is_some(),
                    "{slug}: day {i} has fully elapsed and is measurable, \
                     including the first, which the daily table returns \
                     complete whatever time of day the window starts"
                );
            }
        }

        // The interval the slow day reports is the slow one, not a hidden or
        // averaged-away value: 1440 / 12 = 120 minutes per block.
        let opt = super::super::block_interval_chart_daily(&days);
        assert_eq!(opt["series"][0]["data"][4].as_f64(), Some(120.0));
        // And TPS on a 144-block day of 2.0 transactions each is
        // 288 / 86,400 = 0.00333 transactions per second, which is what the
        // chart must show.
        //
        // This asserted `Some(0.0)` until 2026-09-22, with a comment noting
        // the true value and calling the zero "rounded to two places by the
        // builder". That is a test pinning a defect: `round(_, 2)` quantised
        // every rate below 0.005 to exactly zero, which flatlined 631 of
        // 6,467 days across 2009 and 2010 and then had them dropped from a
        // log axis by a notice blaming the data. The builder now uses
        // `round_plot`, which keeps six significant figures.
        let tps = super::super::tps_chart_daily(&days);
        assert_eq!(
            tps["series"][0]["data"][1].as_f64(),
            Some(0.00333333),
            "a real rate below 0.005 tx/s must survive as a small number \
             rather than becoming zero"
        );
    }

    /// A day with no blocks has no interval, rather than an interval of zero.
    #[test]
    fn a_day_with_no_blocks_reports_no_interval() {
        let mut days = days_from_conformance("2024-01-01", 5);
        for d in days.iter_mut() {
            d.block_count = 144;
            d.avg_tx_count = 2.0;
        }
        days[2].block_count = 0;
        let opt = super::super::block_interval_chart_daily(&days);
        assert!(
            opt["series"][0]["data"][2].is_null(),
            "a blockless day divided 1440 by zero"
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

    /// Address Type Count's daily point is a total, which is the finding
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
            match (
                meta.shape,
                kpi::compute(
                    &json,
                    meta.shape,
                    meta.unit,
                    meta.plots_interval_totals(daily),
                ),
            ) {
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

    /// A share of non-coinbase transactions is absent, not zero, in a block
    /// that has none.
    ///
    /// Both charts' daily arms have always gapped these; their per-block
    /// arms returned `0.0`, putting "0% of this block" on the axis for all
    /// 89,929 coinbase-only blocks. `avg-fee-tx` is the chart that already
    /// documents the convention, so these two were the ones outside it.
    ///
    /// `rbf` additionally fabricated a reading for the whole pre-BIP 125
    /// chain: the raw series emitted null there, but the same positions held
    /// a computed zero that fed the 144-block average, so the smoothed line
    /// read near zero before 2016 and stayed dragged down for 144 blocks
    /// past the boundary. The gap has to reach the average too.
    #[test]
    fn a_share_of_non_coinbase_transactions_gaps_when_there_are_none() {
        let mut blocks = synthetic_blocks(8);
        for b in blocks.iter_mut() {
            // Well after BIP 125, so only the coinbase case is in play here.
            b.timestamp = 1_700_000_000 + b.height * 600;
        }
        // A coinbase-only block: one transaction, so none to take a share of.
        blocks[3].tx_count = 1;
        blocks[3].segwit_spend_count = 0;
        blocks[3].rbf_count = 0;

        for (name, opt) in [
            ("segwit", super::super::segwit_adoption_chart(&blocks)),
            ("rbf", super::super::rbf_chart(&blocks)),
        ] {
            let raw = opt["series"][0]["data"].as_array().expect("raw series");
            assert!(
                raw[3][1].is_null(),
                "{name}: a coinbase-only block reports {} rather than a gap",
                raw[3][1]
            );
            assert!(
                raw[2][1].as_f64().is_some(),
                "{name}: an ordinary block lost its reading"
            );
        }

        // And the pre-BIP 125 chain must not reach the rbf average as zero.
        // 2014, comfortably before BIP 125. Indexed rather than derived
        // from `height`, which the fixture sets in the 800,000s, so a
        // height-based timestamp lands after the boundary rather than before
        // it.
        let mut old = synthetic_blocks(300);
        for (i, b) in old.iter_mut().enumerate() {
            b.timestamp = 1_400_000_000 + i as u64 * 600;
        }
        let opt = super::super::rbf_chart(&old);
        for s in opt["series"].as_array().expect("series") {
            for p in s["data"].as_array().expect("data") {
                let y = p.get(1).cloned().unwrap_or(serde_json::Value::Null);
                assert!(
                    y.is_null(),
                    "rbf reports {y} before BIP 125, when the signal did \
                     not exist; a computed zero here is what dragged the \
                     144-block average down"
                );
            }
        }
    }

    /// A 100%-stacked protocol share adds up to 100%.
    ///
    /// The bands were divided by `op_return_count`, which they do not
    /// partition. Ingestion classifies every nulldata output into exactly one
    /// band and then increments the total, so the identity holds by
    /// construction for new rows and does hold across the last 1,000 blocks.
    /// But `update_block_extras` rewrites the bands and not the total, so
    /// 291,016 historical blocks carry a larger total than the sum of their
    /// parts and the stack stopped short of 100% with nothing to explain the
    /// gap. Found by review on 2026-09-21.
    #[test]
    fn a_protocol_share_stack_reaches_one_hundred_percent() {
        // A block whose stored total exceeds its parts, which is what 30% of
        // the chain looks like.
        let mut blocks = synthetic_blocks(3);
        for b in blocks.iter_mut() {
            b.runes_count = 3;
            b.omni_count = 1;
            b.counterparty_count = 1;
            b.data_carrier_count = 5;
            // Ten classified, but the stale column claims fifteen.
            b.op_return_count = 15;
        }
        let opt = super::super::runes_pct_chart(&blocks);
        let total: f64 = opt["series"]
            .as_array()
            .expect("bands")
            .iter()
            .map(|s| s["data"][0][1].as_f64().unwrap_or(0.0))
            .sum();
        assert!(
            (total - 100.0).abs() < 0.05,
            "the four bands sum to {total}%, not 100%. They are being divided \
             by the stale op_return_count rather than by their own sum."
        );
    }

    /// The first inscription wave is drawn, not hidden behind a constant.
    ///
    /// The chart nulled every block below height 774,000 on the strength of a
    /// comment saying inscriptions launched there. The database says the first
    /// inscription-bearing block is 767,430 (2022-12-14), and 116 blocks below
    /// the gate carry 129 inscriptions and 5,140,776 payload bytes, including
    /// some of the largest block shares in the series. Found by review on
    /// 2026-09-21. Gating on the block's own detections needs no constant.
    #[test]
    fn an_early_inscription_block_is_not_hidden_by_a_height_constant() {
        let mut blocks = synthetic_blocks(3);
        for b in blocks.iter_mut() {
            b.height = 770_000; // below the old gate, above the real first
            b.size = 1_000_000;
            b.inscription_count = 0;
            b.inscription_bytes = 0;
        }
        // One real early inscription block, 65% inscription data.
        blocks[1].inscription_count = 4;
        blocks[1].inscription_bytes = 650_000;

        let opt = super::super::all_embedded_share_chart(&blocks);
        let insc = opt["series"]
            .as_array()
            .expect("series")
            .iter()
            .find(|s| {
                s["name"].as_str().is_some_and(|n| n.contains("nscription"))
            })
            .expect("an inscriptions band")
            .clone();
        let y = insc["data"][1][1].as_f64();
        assert_eq!(
            y,
            Some(65.0),
            "an inscription-bearing block below height 774,000 was nulled; \
             the first inscription is block 767,430, not 774,000"
        );
        // Only the LEADING run is absent. A later block that simply carried no
        // inscription has a measured share of zero, and nulling it punctures
        // a stacked area with `connectNulls: false`. The first version of this
        // fix got that wrong, and this fixture is what pins it: block 0 is
        // before any inscription (absent), block 2 is after one (zero).
        assert!(
            insc["data"][0][1].is_null(),
            "a block before any inscription should be absent, not zero"
        );
        assert_eq!(
            insc["data"][2][1].as_f64(),
            Some(0.0),
            "a block AFTER the first inscription that carries none has a \
             measured share of zero; nulling it breaks the stacked band"
        );

        // **And the daily arm, which is the one that matters here.** Any
        // window over 5,000 blocks resolves to daily, so ALL, the only range
        // from which anyone looks at the start of inscriptions, never touches
        // the per-block arm above. The first version of this test built only
        // the per-block arm and passed while the daily arm still hid the
        // first inscriptions behind a hardcoded "2023-01-01".
        let mut days = synthetic_days(4);
        days[0].date = "2022-12-12".to_string();
        days[1].date = "2022-12-14".to_string();
        days[2].date = "2022-12-16".to_string();
        days[3].date = "2022-12-18".to_string();
        for d in days.iter_mut() {
            d.avg_inscription_bytes = 0.0;
        }
        days[1].avg_inscription_bytes = 500.0;
        let opt = super::super::all_embedded_share_chart_daily(&days);
        let insc = opt["series"]
            .as_array()
            .expect("series")
            .iter()
            .find(|s| {
                s["name"].as_str().is_some_and(|n| n.contains("nscription"))
            })
            .expect("an inscriptions band")
            .clone();
        assert!(
            insc["data"][0].is_null(),
            "the day before the first inscription should be absent"
        );
        assert!(
            insc["data"][1].as_f64().is_some_and(|v| v > 0.0),
            "2022-12-14 carries the first inscription and must be drawn; a \
             hardcoded 2023-01-01 gate hides it at the only resolution that \
             can show it"
        );
    }

    /// An undefined percentile is absent, and a real zero still plots.
    ///
    /// Ingestion stores 0.0 when a block has too few fee-rate observations to
    /// define the rank. Plotted literally that produced an impossible chart:
    /// block 963,786 stores p90 = 0 beside p75 = 3.08, so the 90th percentile
    /// drew below the 75th, and 5,740 blocks chain-wide put p90 under a
    /// positive median.
    ///
    /// Both halves matter, which is why this test has two fixtures. Nulling
    /// every zero would have been the easy wrong fix: 56,562 pre-2016 blocks
    /// have a genuine p10 of 0 with a positive median, from the era of free
    /// transactions. The population size is the only thing that separates
    /// them. Found by review on 2026-09-21.
    #[test]
    fn an_undefined_percentile_is_absent_but_a_real_zero_is_not() {
        let series_named = |opt: &serde_json::Value, name: &str| {
            opt["series"]
                .as_array()
                .expect("series")
                .iter()
                .find(|s| s["name"].as_str() == Some(name))
                .unwrap_or_else(|| panic!("no series named {name}"))
                .clone()
        };

        // Too few transactions for p10/p90, enough for p25/p75. The stored
        // zeros for p10 and p90 are sentinels and must not be drawn.
        let mut small = synthetic_blocks(3);
        for b in small.iter_mut() {
            b.tx_count = 9;
            b.fee_rate_p10 = 0.0;
            b.fee_rate_p25 = 1.21;
            b.median_fee_rate = 2.0;
            b.fee_rate_p75 = 3.08;
            b.fee_rate_p90 = 0.0;
        }
        let opt = super::super::fee_rate_heatmap_chart(&small);
        for band in ["p10", "p90"] {
            let s = series_named(&opt, band);
            let pts = s["data"].as_array().expect("points");
            assert!(
                pts.iter().all(|p| p[1].is_null()),
                "{band} drew the too-few-transactions sentinel as a real fee \
                 rate; with 9 transactions the rank is undefined. Got {:?}",
                pts.first()
            );
        }
        // And the bands that ARE defined still draw.
        let p75 = series_named(&opt, "p75");
        assert_eq!(
            p75["data"][0][1].as_f64(),
            Some(3.08),
            "p75 has 8 observations, over its threshold of 4, so it must draw"
        );

        // A genuine zero, from a block with plenty of free transactions.
        let mut free = synthetic_blocks(3);
        for b in free.iter_mut() {
            b.tx_count = 80;
            b.fee_rate_p10 = 0.0;
            b.fee_rate_p25 = 0.0;
            b.median_fee_rate = 0.5;
            b.fee_rate_p75 = 1.0;
            b.fee_rate_p90 = 2.0;
        }
        let opt = super::super::fee_rate_heatmap_chart(&free);
        let p10 = series_named(&opt, "p10");
        assert_eq!(
            p10["data"][0][1].as_f64(),
            Some(0.0),
            "a block with 79 fee-rate observations and a true p10 of zero \
             must still plot zero; suppressing it would erase the \
             free-transaction era"
        );
    }

    /// Event markers survive a reader hiding a band.
    ///
    /// An ECharts `markLine` belongs to a series, so mark lines attached to
    /// `series[0]` disappear when that series is deselected in the legend. On
    /// Address Type Count that meant isolating P2SH removed the BIP-16
    /// activation line, which is the marker that explains where the band
    /// starts. Found by the owner on 2026-09-23.
    ///
    /// The carrier must also stay out of the legend, or a reader gets a
    /// control for a series that draws nothing.
    #[test]
    fn event_markers_do_not_belong_to_a_chart_band() {
        let blocks = synthetic_blocks(400);
        let flags = super::super::OverlayFlags {
            bip_activations: true,
            halvings: true,
            ..Default::default()
        };
        let mut opt = super::super::address_type_chart(&blocks);
        super::super::apply_overlays(&mut opt, &flags, false);

        let series = opt["series"].as_array().expect("series");
        let carrier = super::super::MARKER_SERIES;

        // No real band owns the mark lines.
        for s in series {
            let name = s["name"].as_str().unwrap_or_default();
            if name == carrier {
                continue;
            }
            assert!(
                s.get("markLine").is_none(),
                "{name} carries the event mark lines, so hiding that band \
                 would hide the markers with it"
            );
        }

        // The carrier exists and holds them.
        let marker = series
            .iter()
            .find(|s| s["name"].as_str() == Some(carrier))
            .expect("a dedicated marker series");
        assert!(
            marker["markLine"]["data"]
                .as_array()
                .is_some_and(|d| !d.is_empty()),
            "the marker series carries no mark lines"
        );

        // And it is not offered in the legend, where it would be a control
        // for a series with no data.
        let legend: Vec<&str> = opt["legend"]["data"]
            .as_array()
            .expect("an explicit legend list")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(
            !legend.contains(&carrier),
            "the marker carrier is in the legend: {legend:?}"
        );
        assert!(
            legend.contains(&"P2PKH") && legend.contains(&"P2SH"),
            "the real bands were dropped from the legend: {legend:?}"
        );
    }

    /// A chart does not overlay itself.
    ///
    /// The chain-size overlay applied to Chain Size Growth drew that chart's
    /// own quantity again on a second axis. Each axis fits its own bounds, so
    /// identical values landed at different heights: the tooltip read
    /// "Block Data 770.67, Chain Size (GB) 770.67" with the lines far apart.
    /// The comparison picker has refused this since it was written; the
    /// overlay path had no equivalent. Found by the owner on 2026-09-23.
    #[test]
    fn an_overlay_does_not_duplicate_the_chart_it_is_laid_over() {
        let blocks = synthetic_blocks(40);
        let series_names = |opt: &serde_json::Value| -> Vec<String> {
            opt["series"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s["name"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };

        let overlay: Vec<(u64, f64)> =
            blocks.iter().map(|b| (b.timestamp * 1000, 770.0)).collect();
        let flags = super::super::OverlayFlags {
            chain_size_data: overlay.clone(),
            ..Default::default()
        };

        // The chain-size chart plots gigabytes on its own axis, so the
        // gigabyte overlay must be refused.
        let mut own = super::super::chain_size_chart(&blocks, 800.0, 0, 0);
        super::super::apply_overlays(&mut own, &flags, false);
        assert!(
            !series_names(&own).iter().any(|n| n == "Chain Size (GB)"),
            "the chain-size chart was laid over itself: {:?}",
            series_names(&own)
        );

        // The two halves of this refusal must name the same charts. The
        // overlay refuses on the built axis unit ("GB"); the rail's toggle
        // refuses on the registry unit (`Unit::Gigabytes`), because a
        // component cannot read a built option. If those ever disagree, one
        // of them is wrong: the control would claim a refusal the chart does
        // not make, or offer one it does.
        let gigabyte_charts: Vec<&str> = registry::CHARTS
            .iter()
            .filter(|c| c.unit == registry::Unit::Gigabytes)
            .map(|c| c.slug)
            .collect();
        assert_eq!(
            gigabyte_charts,
            vec!["chain-size"],
            "the set of charts the toggle refuses no longer matches the one \
             the overlay refuses; both guards have to name the same charts"
        );

        // And the guard is not "never draw it": a chart in other units still
        // gets the overlay, which is what makes the check above meaningful.
        let mut other = super::super::tx_count_chart(&blocks);
        super::super::apply_overlays(&mut other, &flags, false);
        assert!(
            series_names(&other).iter().any(|n| n == "Chain Size (GB)"),
            "the overlay stopped working on charts that do want it: {:?}",
            series_names(&other)
        );
    }

    /// A small daily average survives being plotted.
    ///
    /// The daily arms rounded to one decimal, so any average below 0.05 per
    /// block became exactly 0.0: a different number, not a small one. Taproot
    /// Outputs showed a flat zero across 2019 to 2021 while the database held
    /// six real outputs, and the same rounding erased 167 days of outputs per
    /// transaction, 80 of script-path spends, 47 of inputs, 14 of Taproot and
    /// 13 of inscriptions. Found by the owner on 2026-09-23, by picking a
    /// custom range around the first Taproot output and seeing nothing.
    ///
    /// Same defect `round_plot` was written for, and the same one that
    /// flatlined daily TPS across 2009 and 2010. `avg_tx_count` is
    /// deliberately not converted: a block always carries at least its
    /// coinbase, so that average cannot reach the quantising range.
    #[test]
    fn a_small_daily_average_is_not_rounded_to_zero() {
        let mut days = synthetic_days(4);
        for d in days.iter_mut() {
            d.block_count = 141;
            // One output across 141 blocks: 0.00709 per block, which is what
            // 2019-12-17 actually holds.
            d.avg_p2tr_count = 1.0 / 141.0;
            d.avg_inscription_count = 1.0 / 141.0;
            d.avg_taproot_scriptpath_count = 1.0 / 141.0;
        }

        let first_point = |opt: &serde_json::Value, series: usize| {
            opt["series"][series]["data"][0].as_f64()
        };

        for (name, opt) in [
            ("taproot", super::super::taproot_chart_daily(&days)),
            ("inscriptions", super::super::inscription_chart_daily(&days)),
        ] {
            let v = first_point(&opt, 0).unwrap_or(0.0);
            assert!(
                v > 0.0,
                "{name}: a real average of 0.0071 per block plotted as {v}. \
                 One decimal quantises every small reading to zero, which is \
                 a different number rather than a small one."
            );
        }
    }

    /// Both halves of the fixed-decimal tooltip sentinel still exist.
    ///
    /// A JS function cannot be serialised from Rust, so the option carries a
    /// marker string and `stats.js` swaps in the real formatter. The two are
    /// in different languages and different files, so nothing but this ties
    /// them together: rename either side and the tooltip silently renders the
    /// literal string `__fixed3__` beside every value.
    ///
    /// Asserted against the shipped file rather than a copy of the constant,
    /// which is the only version that can catch a rename in `stats.js`.
    #[test]
    fn the_fixed_decimal_tooltip_sentinel_matches_the_javascript() {
        use crate::stats::charts::TOOLTIP_FIXED3_SENTINEL;
        let js = include_str!("../../../assets/js/stats.js");
        assert!(
            js.contains(TOOLTIP_FIXED3_SENTINEL),
            "stats.js no longer mentions {TOOLTIP_FIXED3_SENTINEL:?}, so the \
             tooltip would print the sentinel instead of a number"
        );
        assert!(
            js.contains("applyTooltipSentinel"),
            "stats.js declares the sentinel but no longer applies it"
        );
        // And the charts that ask for it really do.
        for (label, opt) in [
            (
                "daily",
                crate::stats::charts::multi_velocity_chart_daily(
                    &synthetic_days(180),
                ),
            ),
            (
                "per block",
                crate::stats::charts::multi_velocity_chart(&synthetic_blocks(
                    600,
                )),
            ),
        ] {
            assert_eq!(
                opt["tooltip"]["valueFormatter"].as_str(),
                Some(TOOLTIP_FIXED3_SENTINEL),
                "{label}: Adoption Velocity lost its fixed-decimal tooltip, \
                 so its eight lines print at four different precisions and \
                 stop looking like they cancel"
            );
        }
    }

    /// Adoption Velocity's lines account for each other, at both resolutions.
    ///
    /// The eight series are shares of one denominator, so their moving
    /// averages sum to 100 and the differences of those averages sum to zero.
    /// **That is only true because all eight are drawn.** Until 2026-09-24
    /// four were, and a reader could not reconcile P2WPKH at +13.29 against
    /// the three visible falls because the missing 0.60 sat in bands with no
    /// line. Restoring one of the four to the denominator alone, or dropping
    /// a series, breaks this.
    ///
    /// The tolerance is for the builders' own `round(_, 3)` on each point,
    /// not for slack in the invariant.
    #[test]
    fn every_velocity_line_is_accounted_for_by_the_others() {
        // A window containing a day that classified nothing is the case the
        // first version of this test could not see, because `synthetic_days`
        // never produces one. On the live chart at 2010-03-19 the eight lines
        // summed to 6.67, because the shares add to 100 on a populated day
        // and to 0 on an empty one, so a plain mean tracked the fraction of
        // populated days and the velocity differenced two such fractions.
        let mut gapped = synthetic_days(180);
        for i in [40usize, 41, 95] {
            gapped[i].avg_p2pkh_count = 0.0;
            gapped[i].avg_p2sh_count = 0.0;
            gapped[i].avg_p2wpkh_count = 0.0;
            gapped[i].avg_p2wsh_count = 0.0;
            gapped[i].avg_p2tr_count = 0.0;
            gapped[i].avg_p2pk_count = 0.0;
            gapped[i].avg_multisig_count = 0.0;
            gapped[i].avg_unknown_script_count = 0.0;
        }
        for (label, opt) in [
            (
                "daily",
                crate::stats::charts::multi_velocity_chart_daily(
                    &synthetic_days(180),
                ),
            ),
            (
                "daily with empty days",
                crate::stats::charts::multi_velocity_chart_daily(&gapped),
            ),
            (
                "per block",
                crate::stats::charts::multi_velocity_chart(&synthetic_blocks(
                    600,
                )),
            ),
        ] {
            let series = opt["series"].as_array().expect("series");
            assert_eq!(
                series.len(),
                8,
                "{label}: all eight classified types must be drawn, or the \
                 lines cannot sum to zero"
            );
            let len = series[0]["data"].as_array().expect("data").len();
            let mut checked = 0usize;
            for i in 0..len {
                let mut total = 0.0f64;
                let mut present = 0usize;
                for s in series {
                    let p = &s["data"].as_array().expect("data")[i];
                    // Daily emits bare numbers, per block emits [ts, value].
                    let v = match p {
                        serde_json::Value::Array(a) => a[1].as_f64(),
                        other => other.as_f64(),
                    };
                    if let Some(v) = v {
                        total += v;
                        present += 1;
                    }
                }
                if present == 0 {
                    continue;
                }
                assert_eq!(
                    present, 8,
                    "{label}: point {i} has {present} of 8 lines defined, so \
                     the residual is invisible at that x"
                );
                assert!(
                    total.abs() < 0.05,
                    "{label}: the eight velocities at point {i} sum to \
                     {total}, not zero, so one type's gain is not the \
                     others' loss"
                );
                checked += 1;
            }
            assert!(
                checked > 10,
                "{label}: only {checked} points had data, so this asserted \
                 almost nothing"
            );
        }
    }

    /// A weekday bar is labelled with the day it actually describes.
    ///
    /// The per-block arm computed `(timestamp / 86400 + 4) % 7`, which is the
    /// 0=Sunday convention, and indexed it into a Monday-first label array.
    /// Every bar sat one slot right: Sunday's blocks under "Mon", Saturday's
    /// under "Sun". The daily arm used chrono's `num_days_from_monday` and was
    /// correct, so one chart gave two different answers either side of the
    /// 5,000-block resolution switch, and the weekend signal the chart exists
    /// to show landed in the working week. Found by review on 2026-09-21.
    ///
    /// Anchored on real block timestamps rather than synthetic ones, because
    /// the defect was in the epoch arithmetic itself and a synthetic fixture
    /// would have inherited whatever convention the test author assumed.
    #[test]
    fn a_weekday_bar_is_labelled_with_its_own_day() {
        // Block 1000, 2009-01-19 06:34:42 UTC, a Monday (SQLite %w = 1).
        // Block 1, 2009-01-09 02:54:25 UTC, a Friday (%w = 5).
        const MONDAY_TS: u64 = 1_232_346_882;
        const FRIDAY_TS: u64 = 1_231_469_665;

        let index_of = |ts: u64| ((ts / 86_400 + 3) % 7) as usize;
        assert_eq!(index_of(MONDAY_TS), 0, "Monday must be index 0");
        assert_eq!(index_of(FRIDAY_TS), 4, "Friday must be index 4");

        // And the built chart must put a Monday-only fixture's mass under the
        // "Mon" category, not beside it.
        let mut blocks = synthetic_blocks(3);
        for (i, b) in blocks.iter_mut().enumerate() {
            b.timestamp = MONDAY_TS + i as u64 * 600;
            b.tx_count = 1234;
        }
        let opt = super::super::weekday_activity_chart(&blocks);
        let cats = opt["xAxis"]["data"]
            .as_array()
            .expect("day categories")
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>();
        let monday = cats
            .iter()
            .position(|c| c == "Mon")
            .expect("a Monday category");
        let tx = opt["series"][0]["data"]
            .as_array()
            .expect("tx series")
            .iter()
            .filter_map(|v| v.as_f64())
            .collect::<Vec<f64>>();
        assert_eq!(
            tx[monday], 1234.0,
            "blocks stamped on a Monday are not under the Mon bar; the \
             day-of-week index and the label array disagree. Bars: {tx:?}"
        );
        for (i, v) in tx.iter().enumerate() {
            if i != monday {
                assert_eq!(
                    *v, 0.0,
                    "a Monday-only fixture put mass under {}",
                    cats[i]
                );
            }
        }
    }

    /// Both interval histograms must treat a backward pair the same way,
    /// and that way is to exclude it.
    ///
    /// `time-dist` switches between a per-block arm and a SQL histogram at
    /// long ranges. The per-block arm used `saturating_sub` on unsigned
    /// timestamps, turning each of the chain's 16,022 backward pairs into an
    /// interval of zero in the shortest bucket, while the SQL filters
    /// `gap >= 0`. Same range, two different shortest buckets, decided only
    /// by which arm served it.
    #[test]
    fn a_backward_interval_is_excluded_from_both_histogram_arms() {
        let mut blocks = synthetic_blocks(6);
        // Two ordinary ten-minute gaps, then one that runs backwards, which
        // is what a miner's chosen timestamp can do.
        for (i, b) in blocks.iter_mut().enumerate() {
            b.timestamp = 1_700_000_000 + i as u64 * 600;
        }
        blocks[4].timestamp = blocks[3].timestamp - 120;

        for (label, opt) in [
            (
                "count",
                super::super::block_time_distribution_chart(&blocks),
            ),
            (
                "percent",
                super::super::block_time_distribution_pct_chart(&blocks),
            ),
        ] {
            let data = opt["series"][0]["data"]
                .as_array()
                .expect("bucket counts")
                .iter()
                .filter_map(|v| v.as_f64())
                .collect::<Vec<f64>>();
            // Five pairs, one backward and one recovering, so three ten
            // minute gaps remain. The shortest bucket must hold none of
            // them: a backward pair is not a block that arrived instantly.
            assert_eq!(
                data[0], 0.0,
                "{label}: a backward pair was clamped into the 0-1 minute \
                 bucket"
            );
            assert!(
                data[10] > 0.0,
                "{label}: the ordinary ten-minute gaps were lost too"
            );
        }

        // Which buckets is only half the question. The first version of this
        // test asserted the buckets and not the denominator, so it passed
        // while the percentage arm still divided by every pair including the
        // backward one, and the bars summed to 80%. Found by review on
        // 2026-09-21. A share is of the intervals that were measured.
        let pct = super::super::block_time_distribution_pct_chart(&blocks);
        let shares = pct["series"][0]["data"]
            .as_array()
            .expect("bucket shares")
            .iter()
            .filter_map(|v| v.as_f64())
            .collect::<Vec<f64>>();
        let sum: f64 = shares.iter().sum();
        assert!(
            (sum - 100.0).abs() < 0.01,
            "the shares sum to {sum}, not 100: the excluded backward pair is \
             still in the denominator"
        );
        // Four intervals survive: three of ten minutes and one of twenty two,
        // the latter being the recovery across the backward stamp.
        assert_eq!(shares[10], 75.0, "three of four intervals are 10-11 min");
        assert_eq!(shares[22], 25.0, "one of four is the 22-23 min recovery");

        // And the two arms must agree. The SQL arm filters `gap >= 0` in the
        // query, so it never sees the backward pair at all; handed the same
        // retained counts it must produce the same shares, or the answer
        // depends on the range length rather than on the data.
        let buckets: Vec<crate::stats::types::HistogramBucket> = (0..61)
            .map(|i| crate::stats::types::HistogramBucket {
                label: if i == 60 {
                    "60+".to_string()
                } else {
                    format!("{}-{}", i, i + 1)
                },
                count: match i {
                    10 => 3,
                    22 => 1,
                    _ => 0,
                },
            })
            .collect();
        let sql_arm =
            super::super::block_time_histogram_from_buckets_pct(&buckets);
        assert_eq!(
            sql_arm["series"][0]["data"], pct["series"][0]["data"],
            "the two histogram arms disagree on the same intervals"
        );
    }

    /// A block with no outputs has no share of them, and no residual.
    ///
    /// `witness-tx-pct` divides by `output_count`, which ingestion
    /// accumulates over non-coinbase transactions only, so the 89,929
    /// coinbase-only blocks have zero. Both named bands returned 0 and the
    /// residual computed `100 - 0 - 0`, drawing **"Other outputs, 100%" for
    /// a block with no outputs at all**. Found by review on 2026-09-16, and
    /// the same class as the zero-for-absent defects fixed earlier here: the
    /// wrong answer is the plausible-looking one.
    #[test]
    fn a_block_with_no_outputs_has_no_output_shares() {
        let mut blocks = synthetic_blocks(4);
        blocks[2].output_count = 0;
        blocks[2].p2wpkh_count = 0;
        blocks[2].p2wsh_count = 0;
        blocks[2].p2tr_count = 0;
        let opt = super::super::witness_version_tx_pct_chart(&blocks);
        for s in opt["series"].as_array().expect("three bands") {
            let name = s["name"].as_str().unwrap_or_default();
            let y = s["data"][2][1].clone();
            assert!(
                y.is_null(),
                "{name} reports {y} for a block with no outputs; the \
                 residual reading 100 is the defect this guards"
            );
            // And a block that does have outputs still reports.
            assert!(
                s["data"][1][1].as_f64().is_some(),
                "{name} lost an ordinary block"
            );
        }
    }

    /// The empty-retarget answer must be distinguishable from the real one,
    /// or a cache keyed on the days alone re-serves it forever.
    ///
    /// The defect this pins, found by review on 2026-09-16: `chart_memo!`
    /// keys on the call site, the range and a fingerprint of the dashboard
    /// rows, and its cached-base path returns the stored string without
    /// evaluating the builder closure. `state.retargets` is derived from the
    /// resolved days, so it cannot fetch until they land, and a resource
    /// keeps its previous value while refetching. Coming from a per-block
    /// range, where it resolves to an empty list, the first build after the
    /// switch used the new days and the old empty list.
    ///
    /// The output of that build is the thing to guard: it is a **non-null,
    /// perfectly reasonable-looking chart**, so it was cached and re-served
    /// for the session. The fix puts the rows in the key; this asserts the
    /// two answers differ at all, which is what makes keying on them work.
    #[test]
    fn an_empty_retarget_list_does_not_look_like_a_real_answer() {
        let days = days_from_conformance("2021-06-30", 7);
        let real = super::super::difficulty_adjustment_chart_daily(
            &days,
            &[
                Retarget {
                    height: 687_456,
                    timestamp: 1_623_614_836,
                    difficulty: 19_932_791_027_262.73,
                },
                Retarget {
                    height: 689_472,
                    timestamp: 1_625_294_046,
                    difficulty: 14_363_025_673_659.96,
                },
            ],
        );
        let stale = super::super::difficulty_adjustment_chart_daily(&days, &[]);

        assert_ne!(
            real, stale,
            "the same days with and without retargets must not build the \
             same option, or no cache key over the days can tell them apart"
        );
        // And the empty one is not null, which is why it was cacheable.
        assert!(
            !stale.is_null(),
            "if this were null the macro would skip caching it and the \
             defect could not have occurred; it is a real chart saying no \
             retarget is in range"
        );
        assert!(
            serde_json::to_string(&stale)
                .expect("serialisable")
                .contains("No difficulty adjustment in this range"),
            "the stale answer is the honest no-retarget frame, which is \
             exactly why it was not noticed: {stale:?}"
        );
    }

    /// A gauge has data and no summary, which is a different answer from
    /// having no data.
    ///
    /// Reproduces the review's case: two equally sized pools give one HHI
    /// value of 5,000. That is `NotSummarizable`, because an average over one
    /// point, a peak equal to it and a low equal to both are three
    /// restatements of the same number. The rail said the chart "plots
    /// several separate measurements", which was true of Transaction Batching
    /// and false here, so the message is neutral about the reason now.
    #[test]
    fn a_gauge_has_no_summary_rather_than_no_data() {
        use super::super::kpi::{self, Kpis};
        let miners: Vec<MinerShare> = ["Foundry USA", "AntPool"]
            .iter()
            .map(|m| MinerShare {
                miner: (*m).to_string(),
                count: 500,
                percentage: 50.0,
            })
            .collect();
        let opt = super::super::mining_diversity_chart(&miners);
        let json = serde_json::to_string(&opt).expect("serialisable");

        // The value is there: 50^2 + 50^2 = 5,000.
        assert_eq!(
            opt["series"][0]["data"][0]["value"].as_f64(),
            Some(5000.0),
            "two equal pools are a concentrated market"
        );
        assert!(
            matches!(
                kpi::compute(
                    &json,
                    registry::Shape::Gauge,
                    registry::Unit::Count,
                    true
                ),
                Kpis::NotSummarizable
            ),
            "a populated gauge is not missing data"
        );

        // And an empty one is the other answer.
        let empty =
            serde_json::to_string(&super::super::mining_diversity_chart(&[]))
                .expect("serialisable");
        assert!(matches!(
            kpi::compute(
                &empty,
                registry::Shape::Gauge,
                registry::Unit::Count,
                true,
            ),
            Kpis::Unavailable
        ));
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
                match kpi::compute(
                    &json,
                    m.shape,
                    m.unit,
                    m.plots_interval_totals(daily),
                ) {
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
        let Kpis::Series { first, last, .. } = kpi::compute(
            &json,
            registry::Shape::Bar,
            registry::Unit::Count,
            true,
        ) else {
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
