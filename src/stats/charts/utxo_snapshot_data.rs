//! The unspent output set, measured once and committed.
//!
//! **Everything else on this site is a flow; this is the stock.** Every chart
//! is derived from blocks, so it counts outputs *created*. 371.7 million
//! Taproot outputs have been created and 52.9 million are unspent, and
//! conflating the two is the easiest way to publish a wrong number on this
//! subject.
//!
//! The figures come from `dumptxoutset` on our own node, parsed by
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

use super::{MULTISIG_COLOR, P2PK_COLOR, P2TR_COLOR};

/// When the snapshot was taken, as the page states it.
pub const TAKEN: &str = "11 November 2026";
/// The block the set is unspent as of.
pub const BASE_HEIGHT: u64 = 975_464;
pub const BASE_HASH: &str =
    "0000000000000000000138036994e1733914b51591f3ad91a8e6b3d349fa7dec";

/// Unspent outputs in the whole set.
pub const TOTAL_UTXOS: u64 = 163_370_361;
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
/// not call it "at risk": nothing here distinguishes lost from deliberately
/// untouched, and coins that are lost cannot be stolen in any sense that
/// matters to their owner.
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
/// The line is cumulative share across all three types, which is the question
/// a reader actually arrives with: how much of this was already sitting there
/// by year X.
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
        "name": "Cumulative share",
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
        "tooltip": { "trigger": "axis", "axisPointer": { "type": "shadow" } },
        "xAxis": {
            "type": "category",
            "data": years.iter().map(|y| y.to_string()).collect::<Vec<_>>(),
            "axisLabel": { "color": "#888", "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#333" } }
        },
        "yAxis": [
            {
                "type": "log",
                "name": "BTC",
                "nameTextStyle": { "color": "#888", "fontSize": 11 },
                "min": floor,
                "axisLabel": { "color": "#888", "fontSize": 10 },
                "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.06)" } }
            },
            {
                "type": "value",
                "name": "cumulative %",
                "nameTextStyle": { "color": "#888", "fontSize": 11 },
                "min": 0, "max": 100,
                "axisLabel": { "color": "#888", "fontSize": 10, "formatter": "{value}%" },
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

    let labels: Vec<&str> = rows.iter().map(|(k, _, _)| *k).collect();
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
        "tooltip": { "trigger": "axis", "axisPointer": { "type": "shadow" } },
        "xAxis": {
            "type": "value",
            "name": "% of the set",
            "nameLocation": "middle",
            "nameGap": 28,
            "nameTextStyle": { "color": "#888", "fontSize": 11 },
            "axisLabel": { "color": "#888", "fontSize": 10, "formatter": "{value}%" },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.06)" } }
        },
        "yAxis": {
            "type": "category",
            "data": labels,
            "inverse": true,
            "axisLabel": { "color": "#888", "fontSize": 11 },
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
            .find(|s| s["name"] == "Cumulative share")
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
            let (c, v) = by_kind
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(label))
                .map(|(_, v)| *v)
                .unwrap_or_else(|| panic!("{label} absent from the file"));
            assert_eq!((c, v), (*count, *value), "{label} disagrees");
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
}
