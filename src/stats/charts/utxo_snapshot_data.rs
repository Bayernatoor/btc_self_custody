//! The unspent output set, measured once and committed.
//!
//! **Everything else on this site is a flow; this is the stock.** Every chart
//! is derived from blocks, so it counts outputs *created*. 372.7 million
//! Taproot outputs have been created and 52.9 million are unspent, and
//! conflating the two is the easiest way to publish a wrong number on this
//! subject.
//!
//! The figures come from `dumptxoutset` on my own node, parsed by
//! `src/bin/parse_utxo_snapshot.rs`. They live here as constants rather than
//! in the database because they are reference data refreshed a few times a
//! year, not something the droplet needs to recompute or sync.
//!
//! Refreshing it:
//!
//! ```text
//! s9 -rpcclienttimeout=0 dumptxoutset utxo-YYYYMMDD.dat latest
//! cargo run --release --features ssr --bin parse_utxo_snapshot -- <file> --json
//! ```
//!
//! Then update every constant below together. `the_snapshot_is_internally_
//! consistent` catches a partial update, and `the_snapshot_matches_the_file`
//! re-derives the whole thing from the file when it is on disk.

use serde_json::{json, Value};

use super::{tooltip_axis, MULTISIG_COLOR, P2PK_COLOR, P2TR_COLOR};

/// When the snapshot was taken, as the page states it.
pub const TAKEN: &str = "11 November 2026";
/// The block the set is unspent as of.
pub const BASE_HEIGHT: u64 = 975_464;
pub const BASE_HASH: &str =
    "0000000000000000000138036994e1733914b51591f3ad91a8e6b3d349fa7dec";

/// Unspent outputs in the whole set.
pub const TOTAL_UTXOS: u64 = 163_370_361;
/// Taproot outputs ever created, as of `BASE_HEIGHT`.
///
/// Read from the blocks table, not from the snapshot, because the snapshot
/// knows only what survives. Stated at the snapshot's own height so it is a
/// fixed fact rather than a number that moves every ten minutes:
///
/// ```text
/// SELECT SUM(p2tr_count) FROM blocks WHERE height <= 975464;
/// ```
///
/// It sits beside the 52.9 million Taproot outputs still unspent, and the gap
/// between the two is the whole reason this module exists.
pub const P2TR_CREATED: u64 = 372_725_872;
/// Satoshis in the whole set.
///
/// Verified against `gettxoutsetinfo none 975464`, which returns the total at
/// the snapshot's own height. There is no way to decompress 163 million
/// amounts incorrectly and land on the same satoshi, which is what makes the
/// rest of this module worth publishing.
pub const TOTAL_SATS: u128 = 2_011_059_798_556_717;

/// Unspent outputs by script type: label, count, satoshis.
pub const BY_KIND: &[(&str, u64, u128)] = &[
    ("P2TR", 52_948_228, 22_123_432_242_588),
    ("P2WPKH", 49_625_058, 830_282_571_948_495),
    ("P2PKH", 43_655_607, 450_736_054_622_776),
    ("P2SH", 11_527_370, 368_178_018_998_027),
    ("P2WSH", 2_832_798, 168_014_190_386_670),
    ("Bare multisig", 2_618_476, 6_836_485_714),
    ("Other witness", 97_495, 53_977_057),
    ("P2PK", 44_513, 171_456_879_013_488),
    ("Other", 20_816, 261_760_881_902),
    // Zero, and that is the measurement rather than a gap in the table.
    // Anchors are 240-sat anyone-can-spend outputs, so they are swept as
    // fast as they appear. Confirmed against the node on 2026-11-12, which
    // is the kind of claim that has to carry its probe:
    // `scantxoutset start '["raw(51024e73)"]'` returned no unspents at
    // height 975,571 while scanning all 163,267,199 outputs.
    ("P2A", 0, 0),
];

/// The labels in `BY_KIND` whose outputs carry a public key in the script.
pub const EXPOSED_KINDS: &[&str] = &["P2PK", "Bare multisig", "P2TR"];

/// Exposed outputs by the year created and the type: year, kind, count, sats.
///
/// Split by type because that is the finding. The exposure before 2013 is
/// almost entirely P2PK, mining rewards paid to a raw key; the exposure after
/// 2022 is almost entirely Taproot. One number per year would hide two
/// different phenomena behind one curve.
///
/// The year is the *creation* height's year, not the year anything moved. A
/// pre-2012 output in this table has been unspent ever since, which is the
/// whole reason the table is interesting and also the reason the page must
/// not call it "at risk": nothing here says a published key can be broken,
/// or when.
///
/// **Do not write that a lost coin cannot be stolen.** It is true of a
/// stolen key and false of a derived one, which is the only threat this
/// section is about: the attacker works from the public key on chain and
/// never needs the owner's copy. A lost coin is the softest target here,
/// because nobody is watching it and nobody will move it behind a hash.
pub const EXPOSED_BY_YEAR: &[(u16, &str, u64, u128)] = &[
    (2009, "P2PK", 23_217, 116_347_209_000_000),
    (2010, "P2PK", 10_388, 52_261_318_555_826),
    (2011, "P2PK", 370, 1_747_035_002_745),
    (2012, "P2PK", 201, 879_470_498_097),
    (2012, "Bare multisig", 2, 1_500_000),
    (2013, "P2PK", 2_903, 220_883_089_701),
    (2013, "Bare multisig", 17_058, 53_939_164),
    (2014, "P2PK", 203, 613_422_772),
    (2014, "Bare multisig", 183_833, 1_442_581_793),
    (2015, "P2PK", 122, 33_337_712),
    (2015, "Bare multisig", 124_212, 2_152_365_000),
    (2016, "P2PK", 326, 17_444_246),
    (2016, "Bare multisig", 19_089, 194_197_773),
    (2017, "P2PK", 1_117, 109_773_200),
    (2017, "Bare multisig", 2_080, 12_112_935),
    (2018, "P2PK", 1_564, 66_581_601),
    (2018, "Bare multisig", 4_694, 10_800_787),
    (2019, "P2PK", 1_596, 73_697_611),
    (2019, "Bare multisig", 5_137, 9_111_579),
    (2020, "P2PK", 712, 7_117_429),
    (2020, "Bare multisig", 806, 2_737_367),
    (2021, "P2PK", 1_635, 27_649_398),
    (2021, "P2TR", 8_915, 27_972_038_595),
    (2021, "Bare multisig", 3_950, 28_591_582),
    (2022, "P2PK", 51, 2_121_310),
    (2022, "P2TR", 101_489, 234_050_090_576),
    (2022, "Bare multisig", 13_463, 104_982_519),
    (2023, "P2PK", 16, 227_512),
    (2023, "P2TR", 20_149_662, 1_315_534_665_813),
    (2023, "Bare multisig", 754_884, 1_638_954_010),
    (2024, "P2PK", 23, 652_149),
    (2024, "P2TR", 19_770_207, 2_421_721_698_724),
    (2024, "Bare multisig", 815_911, 649_403_461),
    (2025, "P2PK", 39, 358_553),
    (2025, "P2TR", 9_583_345, 4_680_976_863_956),
    (2025, "Bare multisig", 584_254, 463_585_346),
    (2026, "P2PK", 30, 10_483_626),
    (2026, "P2TR", 3_334_610, 13_443_176_884_924),
    (2026, "Bare multisig", 89_103, 71_622_398),
];

/// `TAKEN` with the month abbreviated, for the date chips on the cards.
///
/// Derived rather than a second constant, so a refresh that updates `TAKEN`
/// cannot leave the chips quoting the previous snapshot.
pub fn taken_short() -> String {
    let mut parts = TAKEN.split(' ');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(d), Some(m), Some(y)) if m.len() > 3 => {
            format!("{d} {} {y}", &m[..3])
        }
        _ => TAKEN.to_string(),
    }
}

/// Unspent outputs that publish a public key.
pub fn exposed_utxos() -> u64 {
    BY_KIND
        .iter()
        .filter(|(k, _, _)| EXPOSED_KINDS.contains(k))
        .map(|(_, c, _)| c)
        .sum()
}

/// Satoshis in them.
pub fn exposed_sats() -> u128 {
    BY_KIND
        .iter()
        .filter(|(k, _, _)| EXPOSED_KINDS.contains(k))
        .map(|(_, _, v)| v)
        .sum()
}

fn btc(sats: u128) -> f64 {
    sats as f64 / 1e8
}

/// Exposed BTC by the year the output was created, split by type.
///
/// Bars on a log axis, because the span is eight orders of magnitude: 2009
/// holds 1.16 million BTC and 2020 holds a twentieth of one. A linear axis
/// plots every year but the first two as nothing at all, which is a true
/// picture of the ratio and a useless one of the shape.
///
/// Lines rather than bars. ECharts baselines a bar on a log axis at 1 rather
/// than at the axis minimum, so every value below one coin drew a bar
/// hanging downward and read as a negative quantity. A line has no baseline
/// to get wrong.
///
/// The line is the running total as a share of **all** exposed bitcoin, so
/// its denominator is `exposed_sats()` and not the year sitting beside it in
/// the tooltip. Two readers in a row read the two as one figure, which is why
/// the series carries its own denominator in its name.
pub fn snapshot_exposed_by_year_chart() -> Value {
    let total = exposed_sats() as f64;
    // The floor is derived, not chosen. A hardcoded one silently clipped the
    // smallest bar to nothing the moment the data was split by type, and the
    // smallest bar is exactly the one a reader would otherwise never guess
    // at: 2023 holds 0.002 BTC of P2PK.
    let smallest = EXPOSED_BY_YEAR
        .iter()
        .map(|(.., v)| btc(*v))
        .fold(f64::INFINITY, f64::min);
    let floor = 10f64.powf(smallest.log10().floor());
    let mut years: Vec<u16> =
        EXPOSED_BY_YEAR.iter().map(|(y, ..)| *y).collect();
    years.dedup();

    let colour = |k: &str| match k {
        "P2PK" => P2PK_COLOR,
        "Bare multisig" => MULTISIG_COLOR,
        _ => P2TR_COLOR,
    };
    // A year with nothing of a type plots `null`, not zero. A log axis has
    // no zero, and a gap is also the honest reading: Taproot did not exist
    // before 2021, and plotting a floor value there invents a measurement.
    let lines: Vec<Value> = EXPOSED_KINDS
        .iter()
        .map(|kind| {
            let data: Vec<Value> = years
                .iter()
                .map(|y| {
                    EXPOSED_BY_YEAR
                        .iter()
                        .find(|(ry, rk, ..)| ry == y && rk == kind)
                        .map(|(.., v)| json!(btc(*v)))
                        .unwrap_or(Value::Null)
                })
                .collect();
            json!({
                "name": kind,
                "type": "line",
                "data": data,
                "yAxisIndex": 0,
                "symbol": "circle",
                "symbolSize": 6,
                // A year with none of a type is a break in the line, not a
                // bridge across it: Taproot did not exist before 2021.
                "connectNulls": false,
                "lineStyle": { "width": 2, "color": colour(kind) },
                "itemStyle": { "color": colour(kind) }
            })
        })
        .collect();

    let mut run: u128 = 0;
    let cumulative: Vec<f64> = years
        .iter()
        .map(|y| {
            run += EXPOSED_BY_YEAR
                .iter()
                .filter(|(ry, ..)| ry == y)
                .map(|(.., v)| v)
                .sum::<u128>();
            100.0 * run as f64 / total
        })
        .collect();

    let mut series = lines;
    series.push(json!({
        "name": "Cumulative % of all exposed BTC",
        "type": "line",
        "data": cumulative,
        "yAxisIndex": 1,
        "symbol": "circle",
        "symbolSize": 5,
        "lineStyle": { "width": 2, "type": "dashed", "color": "#f7931a" },
        "itemStyle": { "color": "#f7931a" }
    }));

    json!({
        "backgroundColor": "transparent",
        "textStyle": {
            "color": "#d4d4d4",
            "fontFamily": "Inter, system-ui, sans-serif"
        },
        "legend": {
            "show": true, "top": 0, "left": "center",
            "textStyle": { "color": "#e8e8e8", "fontSize": 11 }
        },
        "grid": { "left": 70, "right": 60, "top": 40, "bottom": 30, "containLabel": true },
        "tooltip": tooltip_axis(),
        "xAxis": {
            "type": "category",
            "data": years.iter().map(|y| y.to_string()).collect::<Vec<_>>(),
            "axisLabel": { "color": "#d4d4d4", "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#333" } }
        },
        "yAxis": [
            {
                "type": "log",
                "name": "BTC",
                "nameTextStyle": { "color": "#d4d4d4", "fontSize": 11 },
                "min": floor,
                "axisLabel": { "color": "#d4d4d4", "fontSize": 10 },
                "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } }
            },
            {
                "type": "value",
                "name": "% of all exposed",
                "nameTextStyle": { "color": "#d4d4d4", "fontSize": 11 },
                "min": 0, "max": 100,
                "axisLabel": { "color": "#d4d4d4", "fontSize": 10, "formatter": "{value}%" },
                "splitLine": { "show": false }
            }
        ],
        "series": series
    })
}

/// The whole set by script type, as a share of outputs beside a share of
/// value.
///
/// Two bars rather than one, because the gap between them is the whole
/// finding. P2PK is 0.03% of the unspent outputs and 8.5% of the value in
/// them; bare multisig and Taproot are the mirror image. A chart showing
/// either share alone tells half of that and invites the wrong half.
pub fn snapshot_composition_chart() -> Value {
    // A type with nothing unspent is worth recording in `BY_KIND` and not
    // worth a zero-width bar.
    let mut rows: Vec<&(&str, u64, u128)> =
        BY_KIND.iter().filter(|(_, c, _)| *c > 0).collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.2));

    // The chart shows every type in the set, not only the exposed ones, and
    // nothing said so: a reader arriving from a section about public keys
    // could read the P2WPKH bar as exposed. ECharts takes a per-category
    // `textStyle` in `yAxis.data`, so the three key-publishing types are
    // marked in the axis itself rather than in a caption nobody reads.
    let labels: Vec<Value> = rows
        .iter()
        .map(|(k, _, _)| {
            let exposed = EXPOSED_KINDS.contains(k);
            json!({
                "value": k,
                "textStyle": {
                    "color": if exposed { "#f7931a" } else { "#d4d4d4" },
                    "fontWeight": if exposed { "bold" } else { "normal" },
                }
            })
        })
        .collect();
    let by_value: Vec<f64> = rows
        .iter()
        .map(|(_, _, v)| 100.0 * *v as f64 / TOTAL_SATS as f64)
        .collect();
    let by_count: Vec<f64> = rows
        .iter()
        .map(|(_, c, _)| 100.0 * *c as f64 / TOTAL_UTXOS as f64)
        .collect();

    json!({
        "backgroundColor": "transparent",
        "textStyle": { "color": "#d4d4d4", "fontFamily": "Inter, system-ui, sans-serif" },
        "legend": {
            "show": true, "top": 0, "left": "center",
            "textStyle": { "color": "#e8e8e8", "fontSize": 11 }
        },
        "grid": { "left": 100, "right": 40, "top": 34, "bottom": 34, "containLabel": true },
        "tooltip": tooltip_axis(),
        "xAxis": {
            "type": "value",
            "name": "% of the UTXO set",
            "nameLocation": "middle",
            "nameGap": 28,
            "nameTextStyle": { "color": "#d4d4d4", "fontSize": 11 },
            "axisLabel": { "color": "#d4d4d4", "fontSize": 10, "formatter": "{value}%" },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } }
        },
        "yAxis": {
            "type": "category",
            "data": labels,
            "inverse": true,
            "axisLabel": { "fontSize": 11 },
            "axisLine": { "lineStyle": { "color": "#333" } }
        },
        "series": [
            {
                "name": "Share of value",
                "type": "bar",
                "data": by_value,
                "itemStyle": { "color": "#f7931a" }
            },
            {
                "name": "Share of outputs",
                "type": "bar",
                "data": by_count,
                "itemStyle": { "color": "#4a6fa5" }
            }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A partial update is the failure this module invites: the figures are
    /// copied in by hand from one run, and a refresh that updates the totals
    /// without the breakdowns leaves the page quoting two different
    /// snapshots in adjacent sentences.
    #[test]
    fn the_snapshot_is_internally_consistent() {
        let kind_utxos: u64 = BY_KIND.iter().map(|(_, c, _)| c).sum();
        let kind_sats: u128 = BY_KIND.iter().map(|(_, _, v)| v).sum();
        assert_eq!(
            kind_utxos, TOTAL_UTXOS,
            "the per-type counts total {kind_utxos} against TOTAL_UTXOS"
        );
        assert_eq!(
            kind_sats, TOTAL_SATS,
            "the per-type values total {kind_sats} against TOTAL_SATS"
        );

        let year_utxos: u64 =
            EXPOSED_BY_YEAR.iter().map(|(_, _, c, _)| c).sum();
        let year_sats: u128 =
            EXPOSED_BY_YEAR.iter().map(|(_, _, _, v)| v).sum();
        assert_eq!(
            year_utxos,
            exposed_utxos(),
            "the yearly breakdown totals {year_utxos} exposed outputs and \
             the per-type one totals {}",
            exposed_utxos()
        );
        assert_eq!(
            year_sats,
            exposed_sats(),
            "the yearly breakdown and the per-type one disagree on exposed \
             value"
        );
    }

    /// No label in `EXPOSED_KINDS` may be absent from `BY_KIND`, or the
    /// headline silently drops a type and reads lower than the truth.
    #[test]
    fn every_exposed_kind_is_a_kind() {
        for k in EXPOSED_KINDS {
            assert!(
                BY_KIND.iter().any(|(label, _, _)| label == k),
                "{k} is counted as exposed but is not in BY_KIND"
            );
        }
    }

    /// The labels here must be the labels the parser emits, capitalisation
    /// included, or the next refresh pastes in rows that match nothing.
    #[cfg(feature = "ssr")]
    #[test]
    fn the_labels_match_the_parsers() {
        use crate::stats::utxo_snapshot::ScriptKind;
        for k in ScriptKind::ALL {
            let want = k.label();
            let found = BY_KIND
                .iter()
                .any(|(label, _, _)| label.eq_ignore_ascii_case(want));
            assert!(found, "the parser emits {want:?} and BY_KIND has no row");
            if k.is_exposed() {
                assert!(
                    EXPOSED_KINDS.iter().any(|e| e.eq_ignore_ascii_case(want)),
                    "the parser calls {want:?} exposed and EXPOSED_KINDS \
                     does not"
                );
            }
        }
    }

    /// The cumulative line ends at 100 and never goes backwards.
    #[test]
    fn the_cumulative_share_reaches_one_hundred() {
        let opt = snapshot_exposed_by_year_chart();
        let line = opt["series"]
            .as_array()
            .expect("series")
            .iter()
            .find(|s| s["name"] == "Cumulative % of all exposed BTC")
            .expect("the line")["data"]
            .as_array()
            .expect("points");
        let vals: Vec<f64> =
            line.iter().map(|v| v.as_f64().expect("a share")).collect();
        for w in vals.windows(2) {
            assert!(
                w[1] >= w[0],
                "the cumulative share fell from {} to {}",
                w[0],
                w[1]
            );
        }
        let last = *vals.last().expect("a last point");
        assert!(
            (last - 100.0).abs() < 0.001,
            "the cumulative share ends at {last}, not 100"
        );
    }

    /// The log axis cannot plot a zero, and three years are close enough to
    /// it that a floor is load-bearing rather than cosmetic.
    #[test]
    fn no_bar_falls_below_the_log_floor() {
        let opt = snapshot_exposed_by_year_chart();
        let floor = opt["yAxis"][0]["min"].as_f64().expect("a floor");
        for (y, k, _, v) in EXPOSED_BY_YEAR {
            let b = btc(*v);
            assert!(
                b >= floor,
                "{y} {k} holds {b} BTC, below the {floor} log floor, so its \
                 bar is clipped to nothing"
            );
        }
    }

    /// The composition bars are shares of one total, so they add to 100.
    #[test]
    fn the_composition_shares_total_one_hundred() {
        let opt = snapshot_composition_chart();
        for (i, what) in [(0usize, "value"), (1, "outputs")] {
            let sum: f64 = opt["series"][i]["data"]
                .as_array()
                .expect("bars")
                .iter()
                .map(|d| d.as_f64().expect("a share"))
                .sum();
            assert!(
                (sum - 100.0).abs() < 0.001,
                "the share-of-{what} bars total {sum}"
            );
        }
    }

    /// Re-derive every constant from the snapshot file itself.
    ///
    /// The file is 8.68 GiB and lives outside the repo, so this cannot run in
    /// CI and is not meant to. It is what to run after a refresh, and what
    /// proves the constants above were not mistyped:
    ///
    /// ```text
    /// UTXO_SNAPSHOT=~/opt/utxo-20261111.dat \
    ///   cargo test --release --features ssr the_snapshot_matches_the_file \
    ///   -- --ignored --nocapture
    /// ```
    #[cfg(feature = "ssr")]
    #[test]
    #[ignore]
    fn the_snapshot_matches_the_file() {
        use crate::stats::utxo_snapshot::parse;
        use std::collections::BTreeMap;

        let path = std::env::var("UTXO_SNAPSHOT")
            .expect("UTXO_SNAPSHOT pointing at a dumptxoutset file");
        let f = std::fs::File::open(&path).expect("open the snapshot");
        let mut by_kind: BTreeMap<&'static str, (u64, u128)> = BTreeMap::new();
        let mut coins = 0u64;
        let mut sats = 0u128;
        let header =
            parse(std::io::BufReader::with_capacity(1 << 22, f), |c| {
                let e = by_kind.entry(c.kind.label()).or_default();
                e.0 += 1;
                e.1 += u128::from(c.value_sats);
                coins += 1;
                sats += u128::from(c.value_sats);
            })
            .expect("parse");

        assert_eq!(header.base_hash_hex(), BASE_HASH, "a different snapshot");
        assert_eq!(coins, TOTAL_UTXOS);
        assert_eq!(sats, TOTAL_SATS);
        for (label, count, value) in BY_KIND {
            // A type the parse never saw is zero, not missing. `P2A` is
            // exactly that and the table records it as a measurement, so
            // panicking here failed the verification on a correct snapshot.
            let (c, v) = by_kind
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(label))
                .map(|(_, v)| *v)
                .unwrap_or((0, 0));
            assert_eq!((c, v), (*count, *value), "{label} disagrees");
        }
        // And nothing the parse saw may be missing from the table.
        for (label, (c, v)) in &by_kind {
            assert!(
                BY_KIND
                    .iter()
                    .any(|(l, _, _)| l.eq_ignore_ascii_case(label)),
                "the file holds {c} {label} outputs worth {v} sats and \
                 BY_KIND has no row for them"
            );
        }
    }
    /// A type with no unspent outputs is in the table and not on the chart.
    #[test]
    fn an_empty_type_is_recorded_but_not_plotted() {
        let empty: Vec<&str> = BY_KIND
            .iter()
            .filter(|(_, c, _)| *c == 0)
            .map(|(k, _, _)| *k)
            .collect();
        assert!(
            !empty.is_empty(),
            "no empty type left to test with; drop this test or the filter"
        );
        let opt = snapshot_composition_chart();
        let plotted = opt["yAxis"]["data"].as_array().expect("labels").len();
        assert_eq!(
            plotted,
            BY_KIND.len() - empty.len(),
            "{empty:?} should be in BY_KIND and off the chart"
        );
    }
    /// Every figure the page states in prose, re-derived from the constants.
    ///
    /// The snapshot section quotes about a dozen numbers in its copy, each one
    /// typed by hand off a parse run. A refresh updates the constants, the
    /// charts redraw, and the prose beside them goes on asserting last year's
    /// measurement with nothing to say so. This is that guard.
    ///
    /// It caught one on arrival: the copy claimed "34,131 outputs created
    /// before the first halving", a figure rolled up by block height, which
    /// nothing committed here can reproduce. The chart plots years, so the
    /// sentence now counts years and both agree.
    #[test]
    fn the_page_copy_states_the_measured_figures() {
        // Four places quote these figures, and three of them are the ones
        // nobody looks at again: the meta description, the crawlable SEO
        // paragraph, the JSON-LD dataset and llms.txt. A refresh that
        // updates the constants and the visible copy would leave all three
        // asserting last year's snapshot to search engines and to models.
        const SOURCES: &[(&str, &str)] = &[
            (
                "post_quantum.rs",
                include_str!("../../routes/observatory/post_quantum.rs"),
            ),
            (
                "dataset-post-quantum.json",
                include_str!(
                    "../../../assets/schema/dataset-post-quantum.json"
                ),
            ),
            ("llms.txt", include_str!("../../../assets/llms.txt")),
        ];
        const PAGE: &str =
            include_str!("../../routes/observatory/post_quantum.rs");

        let kind = |label: &str| -> (u64, u128) {
            BY_KIND
                .iter()
                .find(|(k, _, _)| *k == label)
                .map(|(_, c, v)| (*c, *v))
                .unwrap_or_else(|| panic!("{label} is not a kind"))
        };
        let (p2pk_c, p2pk_v) = kind("P2PK");
        let (p2tr_c, p2tr_v) = kind("P2TR");

        // Everything created up to and including 2012, which is the span the
        // copy calls Bitcoin's first four years.
        let early: Vec<_> = EXPOSED_BY_YEAR
            .iter()
            .filter(|(y, ..)| *y <= 2012)
            .collect();
        let early_c: u64 = early.iter().map(|(_, _, c, _)| c).sum();
        let early_v: u128 = early.iter().map(|(.., v)| v).sum();

        // The running total's first point, which the note quotes to show
        // what the dashed line is a share of.
        let first_year = EXPOSED_BY_YEAR.first().map(|(y, ..)| *y).unwrap_or(0);
        let first_v: u128 = EXPOSED_BY_YEAR
            .iter()
            .filter(|(y, ..)| *y == first_year)
            .map(|(.., v)| v)
            .sum();

        let group = |n: u64| {
            n.to_string()
                .as_bytes()
                .rchunks(3)
                .rev()
                .map(|c| String::from_utf8_lossy(c).to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let pct = |a: u128, b: u128| 100.0 * a as f64 / b as f64;

        // Only figures the copy types out. The snapshot total and the exposed
        // counts are interpolated from these same constants at render time,
        // so they cannot drift and there is nothing for a guard to catch.
        let claims: &[(String, &str)] = &[
            (group(p2pk_c), "P2PK outputs"),
            (
                format!("{:.2}%", pct(p2pk_c as u128, TOTAL_UTXOS as u128)),
                "P2PK share of outputs",
            ),
            (
                format!("{:.1}%", pct(p2pk_v, TOTAL_SATS)),
                "P2PK share of value",
            ),
            (
                format!("{:.1} BTC", p2pk_v as f64 / 1e8 / p2pk_c as f64),
                "average P2PK",
            ),
            (
                format!("{:.3} BTC", p2tr_v as f64 / 1e8 / p2tr_c as f64),
                "average Taproot",
            ),
            (group(early_c), "outputs from the first four years"),
            (
                format!("{:.2} million BTC", early_v as f64 / 1e8 / 1e6),
                "value in them",
            ),
            (
                format!("{:.0}%", pct(early_v, exposed_sats())),
                "their share of exposed value",
            ),
            (
                format!("{:.1}%", pct(early_v, TOTAL_SATS)),
                "their share of all bitcoin",
            ),
            (
                format!("{:.0}%", pct(first_v, exposed_sats())),
                "the running total at its first year",
            ),
        ];
        let missing: Vec<String> = claims
            .iter()
            .filter(|(figure, _)| !PAGE.contains(figure.as_str()))
            .map(|(figure, what)| format!("{figure} ({what})"))
            .collect();
        assert!(
            missing.is_empty(),
            "the snapshot copy no longer states these measured figures, so \
             it is quoting an older parse: {missing:?}"
        );

        // The headline pair, which every off-page description repeats.
        let headline: &[(String, &str)] = &[
            (group(exposed_utxos()), "exposed outputs"),
            (group(exposed_sats() as u64 / 100_000_000), "exposed BTC"),
        ];
        // Pull the two metadata strings out of the page source and treat
        // each as its own source. Checking the file as a whole let a stale
        // meta description hide behind a correct `seo_text` three hundred
        // lines away, which is exactly the pair most likely to drift apart.
        let between = |src: &'static str, after: &str| -> String {
            src.split_once(after)
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(v, _)| v.to_string())
                .unwrap_or_default()
        };
        let page = SOURCES[0].1;
        let meta = between(page, "<Meta name=\"description\" content=\"");
        let seo = between(page, "seo_text=\"");
        assert!(
            meta.len() > 80 && seo.len() > 80,
            "could not read the meta description ({}) or seo_text ({}) out \
             of the page; the guard below is checking nothing",
            meta.len(),
            seo.len()
        );
        let mut sources: Vec<(&str, &str)> = SOURCES.to_vec();
        sources.push(("meta description", meta.as_str()));
        sources.push(("seo_text", seo.as_str()));

        let mut stale = Vec::new();
        for (name, src) in &sources {
            for (figure, what) in headline {
                if !src.contains(figure.as_str()) {
                    stale.push(format!("{name}: {figure} ({what})"));
                }
            }
        }
        assert!(
            stale.is_empty(),
            "these quote the snapshot but not its current figures: {stale:?}"
        );
    }
    /// The chip date is the same date as the heading, just shorter.
    #[test]
    fn the_short_date_is_the_same_date() {
        let short = taken_short();
        let (d, rest) = TAKEN.split_once(' ').expect("a day and a month");
        let (month, year) = rest.split_once(' ').expect("a month and a year");
        assert!(
            short.starts_with(d) && short.ends_with(year),
            "{short:?} is not {TAKEN:?} with the month shortened"
        );
        assert!(
            month.starts_with(
                short
                    .trim_start_matches(d)
                    .trim()
                    .trim_end_matches(year)
                    .trim()
            ),
            "{short:?} abbreviates to something {month:?} does not start with"
        );
        assert!(short.len() <= TAKEN.len(), "the short form is not shorter");
    }
    /// Exactly the key-publishing types are marked on the axis.
    ///
    /// The chart carries every type in the set, so the mark is the only
    /// thing separating the three that publish a public key from the six
    /// that do not. Marking one too many would say an output is exposed when
    /// it is not, which is the unsafe direction on this subject.
    #[test]
    fn the_axis_marks_the_exposed_types_and_no_others() {
        let opt = snapshot_composition_chart();
        let cats = opt["yAxis"]["data"].as_array().expect("categories");
        let mut marked: Vec<&str> = Vec::new();
        for c in cats {
            let label = c["value"].as_str().expect("a label");
            let colour = c["textStyle"]["color"].as_str().expect("a colour");
            if colour == "#f7931a" {
                marked.push(label);
            }
            assert_eq!(
                EXPOSED_KINDS.contains(&label),
                colour == "#f7931a",
                "{label} is marked as exposed: {}, but is exposed: {}",
                colour == "#f7931a",
                EXPOSED_KINDS.contains(&label)
            );
        }
        let mut want: Vec<&str> = EXPOSED_KINDS
            .iter()
            .copied()
            .filter(|k| BY_KIND.iter().any(|(l, c, _)| l == k && *c > 0))
            .collect();
        want.sort_unstable();
        marked.sort_unstable();
        assert_eq!(marked, want, "the marked set is not the exposed set");
    }
    /// The year the running total first rounds to 88%, which the page states.
    ///
    /// The copy said 2012 and the data says 2011. Nothing checked it, because
    /// the figure itself, 88%, is correct for both the first-four-years total
    /// and the 2011 crossing, so the guard on figures could not see the wrong
    /// year beside the right number.
    #[test]
    fn the_running_total_reaches_eighty_eight_at_2011() {
        let total = exposed_sats();
        let mut years: Vec<u16> =
            EXPOSED_BY_YEAR.iter().map(|(y, ..)| *y).collect();
        years.dedup();
        let mut run: u128 = 0;
        let mut first = None;
        for y in years {
            run += EXPOSED_BY_YEAR
                .iter()
                .filter(|(ry, ..)| *ry == y)
                .map(|(.., v)| v)
                .sum::<u128>();
            if first.is_none() && (100.0 * run as f64 / total as f64) >= 87.5 {
                first = Some(y);
            }
        }
        assert_eq!(
            first,
            Some(2011),
            "the page says the running total reaches 88% by 2011"
        );
    }
    /// These two charts sit beside registry charts and have to look like
    /// them, but nothing in `conformance.rs` covers them: the registry's
    /// model is a function of blocks or days, and a snapshot is neither.
    ///
    /// Written after both shipped with `#888` axes, which read as disabled
    /// next to the `#d4d4d4` of every chart above them on the same page.
    #[test]
    fn the_snapshot_charts_use_the_sites_axis_colours() {
        for (name, opt) in [
            ("by year", snapshot_exposed_by_year_chart()),
            ("composition", snapshot_composition_chart()),
        ] {
            let json = opt.to_string();
            for dim in ["#888", "#999", "#777", "rgba(255,255,255,0.06)"] {
                assert!(
                    !json.contains(dim),
                    "{name} uses {dim}, which is dimmer than the axis \
                     colour every other chart on the page uses"
                );
            }
            // Every axis label and axis name is the shared colour, except
            // the deliberate orange on the key-publishing categories.
            let mut checked = 0;
            for axis in ["xAxis", "yAxis"] {
                let axes = match opt[axis].as_array() {
                    Some(a) => a.clone(),
                    None => vec![opt[axis].clone()],
                };
                for a in axes {
                    for key in ["axisLabel", "nameTextStyle"] {
                        if let Some(c) = a[key]["color"].as_str() {
                            assert_eq!(
                                c, "#d4d4d4",
                                "{name} {axis}.{key} is {c}"
                            );
                            checked += 1;
                        }
                    }
                }
            }
            // Two is the floor: the composition chart leaves its category
            // axis without a colour of its own, because each label carries
            // its own `textStyle` to mark the key-publishing types.
            assert!(checked >= 2, "{name}: only {checked} axis colours seen");
        }
    }
}
