//! Post-quantum exposure: which outputs published a public key on chain.
//!
//! # The rule this module lives or dies by
//!
//! **State what is exposed. Never state when it will break.** No qubit counts,
//! no timelines, no "at risk by". `AGENTS.md` forbids opinions on Observatory
//! pages unless provable or undisputed, and this is the topic most likely to
//! tempt an exception. Everything here is a property of the chain.
//!
//! # Flow, not stock
//!
//! These count outputs **created**. 372.7 million P2TR outputs created is not
//! 372.7 million exposed UTXOs, because most have since been spent. Conflating
//! the two is the single easiest way to publish a wrong number on this subject,
//! so every title, axis and caption says "created". A current balance needs a
//! `dumptxoutset` snapshot, which is tier 2 and not built.
//!
//! **The two limits here push in opposite directions**, and saying they both
//! understate would be wrong. Omitting exposure by reuse can only make the
//! figures low. Counting creation rather than holdings makes them *high* for
//! any type whose outputs turn over quickly, because an output spent years ago
//! is still counted. Neither the page nor this module may claim a net
//! direction, because nothing here measures one.
//!
//! # What is and is not counted
//!
//! Exposed by construction, the public key is in the output itself:
//!
//! - **P2PK**, a raw public key in the output script
//! - **bare multisig**, public keys in the output script
//! - **P2TR**, where the output *is* an x-only tweaked public key
//!
//! Hash-protected, only a hash was published: P2PKH, P2SH, P2WPKH, P2WSH.
//!
//! **Exposure by reuse is not counted here**, and that is a correctness
//! decision rather than laziness. A hash-protected output reveals its key when
//! spent, so a reused address with a balance is exposed. Deciding that needs
//! the set of every scriptPubKey ever spent from, hundreds of millions of
//! entries, and it is scoped as its own project. Until it is measured, every
//! figure here is explicitly "by construction" and reads *low*, never high.
//!
//! # The denominator
//!
//! The eight classified script types, summed. Not `output_count`, which also
//! counts provably unspendable outputs: an OP_RETURN carries no public key and
//! never enters the UTXO set, so including it would dilute the share for a
//! reason that has nothing to do with exposure.
//!
//! This is the site's **existing** eight-type denominator, the same expression
//! `address_sunset_chart` and `multi_velocity_chart` already use, so a script
//! type reads identically across those three. It differs from
//! `address-types-pct`, which divides by six types, and from `witness-tx-pct`,
//! which divides by every output including unspendables.
//!
//! An earlier draft of this comment called it "a third denominator" and said
//! P2PKH Sunset divided "by eight including unspendables". Both were wrong:
//! `unspendable_output_count` appears in no denominator on the site, and
//! Sunset's own copy says "OP_RETURN outputs are outside it". Asserting a
//! difference without reading the other side is how that happened.
//!
//! Measured 2026-10-30: the eight types sum to `output_count` minus
//! `unspendable_output_count` in 618,034 of the 618,383 blocks where the two
//! differ. 349 blocks exceed it by one to three outputs, worst-case distortion
//! 0.0000262% on an all-time share. Recorded in `notes/post-quantum-preflight.md`
//! rather than discovered later.

use super::*;
use serde_json::json;

/// Outputs whose public key is on chain the moment they are created.
fn exposed(b: &BlockSummary) -> u64 {
    b.p2pk_count + b.multisig_count + b.p2tr_count
}

/// Outputs that published only a hash.
fn hash_protected(b: &BlockSummary) -> u64 {
    b.p2pkh_count + b.p2sh_count + b.p2wpkh_count + b.p2wsh_count
}

/// Outputs carrying no public key in any form.
///
/// Pay-to-anchor is `OP_1 <0x4e73>`, a fixed two-byte witness program with no
/// key in it, so it is neither exposed nor hash-protected. It is broken out
/// because it became the overwhelming majority of `unknown_script_count`:
/// measured 2026-10-30, **99.4% of unknown scripts in 2026 were anchors**,
/// 6,818,516 of 6,859,252. Left inside "unclassified" it drew a large and
/// growing band of apparently unknown risk on a page about risk, when the
/// truth is the opposite.
fn anchors(b: &BlockSummary) -> u64 {
    b.p2a_count
}

/// Scripts the classifier could not place, anchors excluded.
///
/// `p2a_count` is a subset of `unknown_script_count` rather than a sibling,
/// so it is subtracted rather than added. Adding it would double-count.
fn unclassified(b: &BlockSummary) -> u64 {
    b.unknown_script_count.saturating_sub(b.p2a_count)
}

/// Every classified output, which is this section's denominator.
///
/// `unknown_script_count` in full, because anchors are already inside it.
fn classified(b: &BlockSummary) -> u64 {
    exposed(b) + hash_protected(b) + b.unknown_script_count
}

/// Daily equivalents. Both arms must divide by the same thing or the chart
/// changes meaning with the range, which has happened on this site before.
fn exposed_daily(d: &DailyAggregate) -> f64 {
    d.avg_p2pk_count + d.avg_multisig_count + d.avg_p2tr_count
}

fn hash_protected_daily(d: &DailyAggregate) -> f64 {
    d.avg_p2pkh_count
        + d.avg_p2sh_count
        + d.avg_p2wpkh_count
        + d.avg_p2wsh_count
}

fn anchors_daily(d: &DailyAggregate) -> f64 {
    d.avg_p2a_count
}

fn unclassified_daily(d: &DailyAggregate) -> f64 {
    (d.avg_unknown_script_count - d.avg_p2a_count).max(0.0)
}

fn classified_daily(d: &DailyAggregate) -> f64 {
    exposed_daily(d) + hash_protected_daily(d) + d.avg_unknown_script_count
}

/// Every satoshi a block created, coinbase included.
///
/// **Not `total_output_value`**, which is non-coinbase only. The exposed
/// value columns do include the coinbase, and nearly every pre-2012 reward
/// paid a raw public key, so dividing by the non-coinbase total put 95,044
/// blocks above 100% when measured on 2026-11-11, all of them early and all
/// of them the ones this chart is about.
fn value_created(b: &BlockSummary) -> u64 {
    b.total_output_value + b.total_fees + block_subsidy(b.height)
}

fn value_created_daily(d: &DailyAggregate) -> f64 {
    (d.total_output_value + d.total_fees + d.total_subsidy) as f64
}

/// Value created in outputs that publish a public key, as a share of all
/// value created.
///
/// **The companion to the snapshot, and a different question from it.** This
/// is potential exposure: money being put into key-publishing outputs as it
/// is created. The snapshot answers how much is sitting in them now. Neither
/// substitutes for the other, and a reader needs both told apart.
///
/// Value-weighted rather than counted, which is the whole point. By count the
/// exposed share of outputs created since 2020 is 15.9%; by value it is 1.7%,
/// because the count is dominated by Taproot dust and inscription outputs
/// averaging four thousandths of a coin. A reader asking how much Bitcoin is
/// exposed and getting the count takes away roughly nine times the truth.
pub fn pq_exposed_value_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Value in Exposed Outputs");
    }

    let make_pct = |f: fn(&BlockSummary) -> u64| -> serde_json::Value {
        let s = build_data_array_f64(blocks, |b| {
            let total = value_created(b);
            if total > 0 {
                round(f(b) as f64 / total as f64 * 100.0, 3).min(100.0)
            } else {
                f64::NAN
            }
        });
        data_array_value(&s)
    };
    let not_exposed = |b: &BlockSummary| -> u64 {
        value_created(b)
            .saturating_sub(b.p2pk_value + b.p2tr_value + b.multisig_value)
    };

    let band = |name: &str, data: serde_json::Value, colour: &str| {
        json!({
            "name": name, "type": "line", "data": data, "stack": "pct",
            "areaStyle": { "opacity": 0.6 },
            "lineStyle": { "width": 0, "color": colour },
            "itemStyle": { "color": colour }, "symbol": "none"
        })
    };

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "% of value created", "max": 100, "nameTextStyle": { "color": "#d4d4d4" }, "axisLabel": { "color": "#d4d4d4" }, "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } } },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            band("P2TR", make_pct(|b| b.p2tr_value), P2TR_COLOR),
            band("P2PK", make_pct(|b| b.p2pk_value), P2PK_COLOR),
            band("Bare multisig", make_pct(|b| b.multisig_value), MULTISIG_COLOR),
            band("Not exposed", make_pct(not_exposed), P2WPKH_COLOR)
        ]
    }))
}

/// Daily arm, same denominator.
pub fn pq_exposed_value_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Value in Exposed Outputs");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let series = |f: fn(&DailyAggregate) -> f64| -> Vec<Option<f64>> {
        days.iter()
            .map(|d| {
                let total = value_created_daily(d);
                (total > 0.0)
                    .then(|| (round(f(d) / total * 100.0, 3)).min(100.0))
            })
            .collect()
    };
    let band = |name: &str, data: Vec<Option<f64>>, colour: &str| {
        json!({
            "name": name, "type": "line", "data": data, "stack": "pct",
            "areaStyle": { "opacity": 0.6 },
            "lineStyle": { "width": 0, "color": colour },
            "itemStyle": { "color": colour }, "symbol": "none"
        })
    };
    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": { "type": "value", "name": "% of value created", "max": 100, "nameTextStyle": { "color": "#d4d4d4" }, "axisLabel": { "color": "#d4d4d4" }, "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } } },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            band("P2TR", series(|d| d.total_p2tr_value as f64), P2TR_COLOR),
            band("P2PK", series(|d| d.total_p2pk_value as f64), P2PK_COLOR),
            band("Bare multisig", series(|d| d.total_multisig_value as f64), MULTISIG_COLOR),
            band("Not exposed", series(|d| {
                value_created_daily(d)
                    - (d.total_p2pk_value + d.total_p2tr_value
                        + d.total_multisig_value) as f64
            }), P2WPKH_COLOR)
        ]
    }))
}

/// Exposure by construction, as a share of classified outputs created.
///
/// Six bands that total 100 by construction: the three exposed types, the
/// hash-protected group, pay-to-anchor, and whatever is left unclassified.
///
/// The arc is the point. Near 100% in 2009 when every reward paid a raw public
/// key, collapsing as P2PKH took over, flat and low through the 2010s, then
/// climbing hard from Taproot's activation at height 709,632.
pub fn pq_exposure_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("Exposure by Construction");
    }

    let make_pct = |f: fn(&BlockSummary) -> u64| -> serde_json::Value {
        let s = build_data_array_f64(blocks, |b| {
            let total = classified(b);
            if total > 0 {
                round(f(b) as f64 / total as f64 * 100.0, 2)
            } else {
                // A block that classified no output at all has no mix, and a
                // zero here would be a claim about one. Same reasoning as
                // `address_type_pct_chart`, which this denominator differs from.
                f64::NAN
            }
        });
        data_array_value(&s)
    };

    let band = |name: &str, data: serde_json::Value, colour: &str| {
        json!({
            "name": name, "type": "line", "data": data, "stack": "pct",
            "areaStyle": { "opacity": 0.6 },
            "lineStyle": { "width": 0, "color": colour },
            "itemStyle": { "color": colour }, "symbol": "none"
        })
    };

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "% of outputs created", "max": 100, "nameTextStyle": { "color": "#d4d4d4" }, "axisLabel": { "color": "#d4d4d4" }, "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } } },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            band("P2TR", make_pct(|b| b.p2tr_count), P2TR_COLOR),
            band("P2PK", make_pct(|b| b.p2pk_count), P2PK_COLOR),
            band("Bare multisig", make_pct(|b| b.multisig_count), MULTISIG_COLOR),
            band("Hash-protected", make_pct(hash_protected), P2WPKH_COLOR),
            band("Pay-to-anchor", make_pct(anchors), P2A_COLOR),
            band("Unclassified", make_pct(unclassified), UNCLASSIFIED_COLOR)
        ]
    }))
}

/// Daily arm. Same six bands, same denominator.
pub fn pq_exposure_chart_daily(days: &[DailyAggregate]) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("Exposure by Construction");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    let series = |f: fn(&DailyAggregate) -> f64| -> Vec<Option<f64>> {
        days.iter()
            .map(|d| {
                let total = classified_daily(d);
                (total > 0.0).then(|| round(f(d) / total * 100.0, 2))
            })
            .collect()
    };

    let band = |name: &str, data: Vec<Option<f64>>, colour: &str| {
        json!({
            "name": name, "type": "line", "data": data, "stack": "pct",
            "areaStyle": { "opacity": 0.6 },
            "lineStyle": { "width": 0, "color": colour },
            "itemStyle": { "color": colour }, "symbol": "none"
        })
    };

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": { "type": "value", "name": "% of outputs created", "max": 100, "nameTextStyle": { "color": "#d4d4d4" }, "axisLabel": { "color": "#d4d4d4" }, "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.05)", "type": "dashed" } } },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            band("P2TR", series(|d| d.avg_p2tr_count), P2TR_COLOR),
            band("P2PK", series(|d| d.avg_p2pk_count), P2PK_COLOR),
            band("Bare multisig", series(|d| d.avg_multisig_count), MULTISIG_COLOR),
            band("Hash-protected", series(hash_protected_daily), P2WPKH_COLOR),
            band("Pay-to-anchor", series(anchors_daily), P2A_COLOR),
            band("Unclassified", series(unclassified_daily), UNCLASSIFIED_COLOR)
        ]
    }))
}

/// P2PK outputs created per block, the oldest exposed form.
///
/// Separate from the P2PK band inside `address-types` because at modern scale
/// that band is pinned to the axis floor and invisible. This is the 2009-2012
/// story on its own axis: every early mining reward paid a raw public key, and
/// the form then stopped being used.
pub fn pq_p2pk_creation_chart(blocks: &[BlockSummary]) -> serde_json::Value {
    if blocks.is_empty() {
        return no_data_chart("P2PK Outputs Created");
    }
    let s = build_data_array_i64(blocks, |b| b.p2pk_count as i64);
    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Outputs created"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "P2PK created", "type": "line",
            "data": data_array_value(&s),
            "areaStyle": { "color": P2PK_COLOR, "opacity": 0.2 },
            "lineStyle": { "width": 1, "color": P2PK_COLOR },
            "itemStyle": { "color": P2PK_COLOR }, "symbol": "none"
        }]
    }))
}

/// Daily arm: the day's mean per block, matching the per-block unit.
pub fn pq_p2pk_creation_chart_daily(
    days: &[DailyAggregate],
) -> serde_json::Value {
    if days.is_empty() {
        return no_data_chart("P2PK Outputs Created");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> =
        days.iter().map(|d| round(d.avg_p2pk_count, 2)).collect();
    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Outputs created"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [{
            "name": "P2PK created", "type": "line", "data": vals,
            "areaStyle": { "color": P2PK_COLOR, "opacity": 0.2 },
            "lineStyle": { "width": 1, "color": P2PK_COLOR },
            "itemStyle": { "color": P2PK_COLOR }, "symbol": "none"
        }]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(
        p2pk: u64,
        ms: u64,
        p2tr: u64,
        pkh: u64,
        unknown: u64,
    ) -> BlockSummary {
        BlockSummary {
            height: 800_000,
            timestamp: 1_700_000_000,
            p2pk_count: p2pk,
            multisig_count: ms,
            p2tr_count: p2tr,
            p2pkh_count: pkh,
            unknown_script_count: unknown,
            ..Default::default()
        }
    }

    /// The denominator excludes unspendable outputs, so a block full of
    /// OP_RETURN does not dilute the exposed share.
    ///
    /// This is the decision the module exists to get right: dividing by
    /// `output_count` would answer a different question.
    #[test]
    fn unspendable_outputs_are_outside_the_denominator() {
        // 50 of 200 classified, so the share is 25% and not the raw count.
        // Fixtures here deliberately do NOT sum to 100: at a denominator of
        // 100 the percentage step is an identity and nothing tests it.
        let mut b = block(0, 0, 50, 150, 0);
        // 900 OP_RETURN outputs in the same block, none of them classified.
        b.output_count = 1_100;
        b.unspendable_output_count = 900;
        let opt = pq_exposure_chart(&[b]);
        let p2tr = opt["series"][0]["data"][0][1].as_f64().expect("a share");
        assert_eq!(
            p2tr, 25.0,
            "50 P2TR of 200 classified outputs is 25%; {p2tr} means the \
             denominator picked up the 900 unspendable outputs, which carry \
             no key and never enter the UTXO set"
        );
    }

    /// The five bands partition the classified outputs exactly.
    #[test]
    fn the_bands_total_one_hundred() {
        let opt = pq_exposure_chart(&[block(6, 14, 80, 60, 40)]);
        let sum: f64 = (0..6)
            .map(|i| opt["series"][i]["data"][0][1].as_f64().unwrap_or(0.0))
            .sum();
        assert!(
            (sum - 100.0).abs() < 0.01,
            "the bands total {sum}, so a reader can take a share off this \
             chart that does not exist"
        );
    }

    /// A block that classified nothing has no mix, and zero is a claim.
    #[test]
    fn a_block_with_no_classified_output_plots_nothing() {
        let opt = pq_exposure_chart(&[block(0, 0, 0, 0, 0)]);
        assert!(
            opt["series"][0]["data"][0][1].is_null(),
            "an empty block reported a share of a mix that does not exist"
        );
    }

    /// Both arms must divide by the same thing, or the chart changes meaning
    /// with the range. That shipped once already on Inscription Block Share.
    #[test]
    fn both_arms_divide_by_the_same_denominator() {
        let per = pq_exposure_chart(&[block(0, 0, 50, 90, 60)]);
        let day = pq_exposure_chart_daily(&[DailyAggregate {
            date: "2024-02-01".to_string(),
            block_count: 1,
            avg_p2tr_count: 50.0,
            avg_p2pkh_count: 90.0,
            avg_unknown_script_count: 60.0,
            ..Default::default()
        }]);
        let a = per["series"][0]["data"][0][1].as_f64().expect("per block");
        let b = day["series"][0]["data"][0].as_f64().expect("daily");
        assert_eq!(
            a, b,
            "per-block says {a}% and daily says {b}% for the same chain"
        );
    }

    /// Anchors are their own band, and are not double-counted.
    ///
    /// `p2a_count` is a subset of `unknown_script_count`, not a sibling, so
    /// the unclassified band subtracts it. Adding instead would push the
    /// stack past 100 and credit the same output twice.
    ///
    /// It matters most on this page: an anchor is `OP_1 <0x4e73>` with no key
    /// in it, and in 2026 anchors were 99.4% of all unknown scripts. Left
    /// inside "unclassified" they drew a large growing band of apparently
    /// unknown risk on a page about risk.
    #[test]
    fn anchors_are_their_own_band_and_not_counted_twice() {
        let mut b = block(0, 0, 0, 180, 20);
        b.p2a_count = 16; // 16 of the 20 unknown scripts are anchors
        let opt = pq_exposure_chart(&[b]);
        let at =
            |i: usize| opt["series"][i]["data"][0][1].as_f64().unwrap_or(0.0);
        // 0 P2TR, 1 P2PK, 2 multisig, 3 hash-protected, 4 anchors, 5 other
        assert_eq!(at(4), 8.0, "16 anchors of 200 classified is 8%");
        assert_eq!(
            at(5),
            2.0,
            "unclassified is the 4 non-anchor unknown scripts of 200, so 2%; \
             10 means the anchors are counted in both bands"
        );
        let sum: f64 = (0..6).map(at).sum();
        assert!(
            (sum - 100.0).abs() < 0.01,
            "bands total {sum}, so anchors are double-counted"
        );
    }

    /// The first point of a named band, so a test says which band it means.
    fn band_at(opt: &serde_json::Value, name: &str, i: usize) -> Option<f64> {
        let s = opt["series"]
            .as_array()?
            .iter()
            .find(|s| s["name"] == name)?;
        let p = &s["data"][i];
        // The per-block arm plots `[timestamp, value]`, the daily arm plots
        // a bare value against a category axis.
        p.get(1).unwrap_or(p).as_f64()
    }

    /// The denominator includes the coinbase, or the early era breaks.
    ///
    /// This is the defect the chart was built around: the exposed value
    /// columns count coinbase outputs and `total_output_value` does not, so
    /// dividing by it alone put 95,044 blocks over 100%, all of them early
    /// and all of them the ones that matter.
    #[test]
    fn the_value_denominator_counts_the_coinbase() {
        // A 2009-shaped block: the whole 50 BTC reward paid to a raw key,
        // nothing else moving. total_output_value is non-coinbase, so zero.
        let b = BlockSummary {
            height: 1,
            timestamp: 1_231_000_000,
            p2pk_value: 5_000_000_000,
            total_output_value: 0,
            total_fees: 0,
            ..Default::default()
        };
        let opt = pq_exposed_value_chart(&[b]);
        let p2pk = band_at(&opt, "P2PK", 0).expect("a share");
        assert_eq!(
            p2pk, 100.0,
            "the whole reward went to a raw public key, so the share is \
             100%; {p2pk} means the denominator dropped the coinbase"
        );
    }

    /// Value bands total 100 and never exceed it.
    #[test]
    fn the_value_bands_total_one_hundred() {
        let b = BlockSummary {
            height: 800_000,
            timestamp: 1_700_000_000,
            p2pk_value: 100_000_000,
            p2tr_value: 300_000_000,
            multisig_value: 50_000_000,
            total_output_value: 2_000_000_000,
            total_fees: 10_000_000,
            ..Default::default()
        };
        let opt = pq_exposed_value_chart(&[b]);
        let sum: f64 = ["P2TR", "P2PK", "Bare multisig", "Not exposed"]
            .iter()
            .map(|n| band_at(&opt, n, 0).unwrap_or(0.0))
            .sum();
        assert!(
            (sum - 100.0).abs() < 0.01,
            "bands total {sum}, so a reader can take a share that does not \
             exist off the chart"
        );
    }

    /// Both arms of the value chart divide by the same thing.
    #[test]
    fn the_value_arms_agree() {
        let per = pq_exposed_value_chart(&[BlockSummary {
            height: 800_000,
            timestamp: 1_700_000_000,
            p2pk_value: 100_000_000,
            p2tr_value: 300_000_000,
            multisig_value: 50_000_000,
            total_output_value: 2_000_000_000,
            total_fees: 10_000_000,
            ..Default::default()
        }]);
        let day = pq_exposed_value_chart_daily(&[DailyAggregate {
            date: "2024-02-01".to_string(),
            block_count: 1,
            total_p2pk_value: 100_000_000,
            total_p2tr_value: 300_000_000,
            total_multisig_value: 50_000_000,
            total_output_value: 2_000_000_000,
            total_fees: 10_000_000,
            total_subsidy: block_subsidy(800_000),
            ..Default::default()
        }]);
        let a = band_at(&per, "P2PK", 0).expect("per block");
        let b = band_at(&day, "P2PK", 0).expect("daily");
        assert_eq!(a, b, "per-block says {a}% and daily says {b}%");
    }

    /// Counting and value-weighting answer different questions, and the
    /// chart must not be silently reading the count.
    #[test]
    fn value_weighting_differs_from_counting() {
        // One huge hash-protected output beside many tiny exposed ones.
        let b = BlockSummary {
            height: 800_000,
            timestamp: 1_700_000_000,
            p2tr_count: 99,
            p2pkh_count: 1,
            p2tr_value: 99_000,
            total_output_value: 10_000_000_000,
            total_fees: 0,
            ..Default::default()
        };
        let by_count =
            band_at(&pq_exposure_chart(std::slice::from_ref(&b)), "P2TR", 0)
                .expect("count share");
        let by_value = band_at(&pq_exposed_value_chart(&[b]), "P2TR", 0)
            .expect("value share");
        assert!(
            by_count > 90.0,
            "P2TR is 99 of 100 outputs, so the count share is high: {by_count}"
        );
        assert!(
            by_value < 1.0,
            "those outputs hold almost nothing, so the value share is low; \
             {by_value} means the value chart is counting"
        );
    }

    /// P2PK is plotted on its own axis, not as a share, so the early era is
    /// legible rather than pinned to the floor.
    #[test]
    fn p2pk_creation_plots_counts() {
        let opt = pq_p2pk_creation_chart(&[block(37, 0, 0, 0, 0)]);
        assert_eq!(opt["series"][0]["data"][0][1].as_f64(), Some(37.0));
    }

    /// The two exposure charts sit next to each other, so they stack the
    /// three exposed types in the same order. A reader comparing them should
    /// not have to re-read the legend.
    ///
    /// Only the exposed prefix is compared. The count chart can split the
    /// remainder into hash-protected, anchor and unclassified because it has
    /// a column for each; the value chart has value columns for the three
    /// exposed types only, so its remainder is one band by necessity.
    #[test]
    fn the_two_exposure_charts_stack_in_the_same_order() {
        let b = BlockSummary {
            height: 800_000,
            timestamp: 1_700_000_000,
            p2tr_count: 10,
            p2pk_count: 1,
            multisig_count: 1,
            p2pkh_count: 5,
            p2tr_value: 1_000,
            p2pk_value: 2_000,
            multisig_value: 3_000,
            total_output_value: 1_000_000_000,
            total_fees: 1_000,
            ..Default::default()
        };
        let names = |opt: &serde_json::Value| -> Vec<String> {
            opt["series"]
                .as_array()
                .expect("series")
                .iter()
                .map(|s| s["name"].as_str().unwrap_or("").to_string())
                .collect()
        };
        let count = names(&pq_exposure_chart(std::slice::from_ref(&b)));
        let value = names(&pq_exposed_value_chart(&[b]));
        assert_eq!(
            count[..3],
            value[..3],
            "the exposed bands are ordered differently"
        );
        assert_eq!(
            value.last().map(String::as_str),
            Some("Not exposed"),
            "the value chart's remainder is one band"
        );
    }
}
