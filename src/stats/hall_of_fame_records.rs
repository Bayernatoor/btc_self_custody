//! Re-measuring the Archives records against the chain they describe.
//!
//! The nine `Records` entries in `hall_of_fame_data.rs` are the only curated
//! copy on the site that claims to be a *superlative*. "The largest block ever
//! mined" is not a figure that ages gracefully: it is either still true or it
//! is a lie, and nothing about the sentence tells you which.
//!
//! Every figure in them was hand-typed from a query someone ran once. That is
//! the same shape as the chart copy `the_copy_still_matches_the_chain` exists
//! to guard, with one extra hazard: a chart's description is wrong by a few
//! points when it drifts, whereas a record is wrong about which block holds it.
//!
//! **The error this would have caught.** `most-taproot-spends` shipped
//! claiming 22,367 Taproot spend inputs in a block that holds 327 inputs in
//! total. No query was involved; the figure came from a column whose name
//! disagreed with what it counted. A `ContainsAtLeast` row would have failed
//! on `327 >= 22367` the first time anyone ran the gate.
//!
//! Four tests, and only the last needs a database:
//!
//! - `every_record_entry_has_at_least_one_derivation` closes the gap where a
//!   tenth record ships with nothing watching it.
//! - `every_derivation_quotes_a_figure_its_entry_prints` fails the moment the
//!   copy and the derivation disagree about what the number is, with no
//!   database and no chain access, so it runs in CI.
//! - `you_cannot_ship_an_unguarded_record_figure` is the ratchet: every
//!   numeric token in a record's copy is derived, structurally accounted for,
//!   or listed with a reason.
//! - `the_copy_still_matches_the_records` re-measures against the chain. It is
//!   named to match the existing drift gate so the documented command picks
//!   both up:
//!
//! ```text
//! cargo test --features ssr the_copy_still_matches -- --ignored --nocapture
//! ```

use super::hall_of_fame_data::HALL_OF_FAME;
use super::types::{HallOfFameEntry, HofCategory};

/// How a derivation's query relates to the sentence it guards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecordKind {
    /// The copy claims a chain-wide record, so the query ranks every block
    /// and the winner must still be this entry's block.
    Superlative,
    /// The figure is a property of one named block rather than a record, so
    /// only the value is checked.
    BlockFact,
    /// The copy's figure belongs to a single transaction, which the `blocks`
    /// table cannot see. The query reads the block-level quantity that
    /// contains it, so the claim is bounded rather than confirmed.
    ///
    /// A weaker assertion than the other two, and still worth making: it is
    /// the one that fails on an arithmetically impossible claim.
    ContainsAtLeast,
}

/// One re-measurable figure from a `Records` entry.
pub struct RecordDerivation {
    /// The entry this belongs to.
    pub slug: &'static str,
    pub kind: RecordKind,
    /// Returns exactly one row, `(height, value)`, with `value` already
    /// converted into the units the copy prints. Comparing in the reader's
    /// units is deliberate: a tolerance of "half the last digit shown" is
    /// then readable straight off the sentence.
    pub sql: &'static str,
    /// The figure as the copy states it, in those same units.
    pub stated: f64,
    /// How far the measurement may sit from `stated`. Half the last digit the
    /// copy prints, so a rounded claim is not failed by its own rounding.
    /// Ignored for `ContainsAtLeast`, which asserts a bound.
    pub tolerance: f64,
    /// The figure exactly as the entry spells it, unit included.
    ///
    /// Load-bearing twice over. It pins the derivation to the sentence, so
    /// editing one without the other fails in CI with no database. And the
    /// unit disambiguates: a bare `3.99` would match any other 3.99 in the
    /// same entry.
    pub as_written: &'static str,
}

/// Every figure the nine `Records` entries print, and the query that re-finds
/// it.
///
/// More than one row may name the same slug; an entry's headline record and
/// the supporting facts beside it are separate claims and fail separately.
pub const RECORD_DERIVATIONS: &[RecordDerivation] = &[
    RecordDerivation {
        slug: "largest-block",
        kind: RecordKind::Superlative,
        sql: "SELECT height, size/1000000.0 FROM blocks \
              ORDER BY size DESC LIMIT 1",
        stated: 3.99,
        tolerance: 0.005,
        as_written: "3.99 MB",
    },
    RecordDerivation {
        slug: "largest-block",
        kind: RecordKind::BlockFact,
        sql: "SELECT height, tx_count FROM blocks WHERE height = 836964",
        stated: 5.0,
        tolerance: 0.0,
        as_written: "5 transactions",
    },
    RecordDerivation {
        slug: "highest-fee-block",
        kind: RecordKind::Superlative,
        sql: "SELECT height, total_fees/100000000.0 FROM blocks \
              ORDER BY total_fees DESC LIMIT 1",
        stated: 291.5,
        tolerance: 0.05,
        as_written: "291.5 BTC",
    },
    RecordDerivation {
        slug: "highest-fee-block",
        kind: RecordKind::BlockFact,
        sql: "SELECT height, max_tx_fee/1000000000.0 FROM blocks \
              WHERE height = 409008",
        stated: 29.1,
        tolerance: 0.05,
        as_written: "29.1 billion sats",
    },
    RecordDerivation {
        slug: "most-transactions-block",
        kind: RecordKind::Superlative,
        sql: "SELECT height, tx_count FROM blocks \
              ORDER BY tx_count DESC LIMIT 1",
        stated: 12_239.0,
        tolerance: 0.0,
        as_written: "12,239 transactions",
    },
    RecordDerivation {
        slug: "largest-transaction",
        kind: RecordKind::Superlative,
        sql: "SELECT height, largest_tx_size/1000000.0 FROM blocks \
              ORDER BY largest_tx_size DESC LIMIT 1",
        stated: 3.99,
        tolerance: 0.005,
        as_written: "3.99 MB",
    },
    // The sweep is a transaction-level claim and `blocks` holds only the
    // block's total, so this bounds it: a block of 20,894 inputs can contain
    // a 20,000-input transaction, and a smaller block could not.
    RecordDerivation {
        slug: "utxo-cleanup-20k",
        kind: RecordKind::ContainsAtLeast,
        sql: "SELECT height, input_count FROM blocks WHERE height = 367885",
        stated: 20_000.0,
        tolerance: 0.0,
        as_written: "20,000 anyone-can-spend dust outputs",
    },
    RecordDerivation {
        slug: "most-outputs-block",
        kind: RecordKind::Superlative,
        sql: "SELECT height, output_count FROM blocks \
              ORDER BY output_count DESC LIMIT 1",
        stated: 26_908.0,
        tolerance: 0.0,
        as_written: "26,908 new UTXOs",
    },
    RecordDerivation {
        slug: "most-outputs-block",
        kind: RecordKind::BlockFact,
        sql: "SELECT height, tx_count FROM blocks WHERE height = 826052",
        stated: 480.0,
        tolerance: 0.0,
        as_written: "480 transactions",
    },
    RecordDerivation {
        slug: "most-inscriptions-block",
        kind: RecordKind::Superlative,
        sql: "SELECT height, inscription_count FROM blocks \
              ORDER BY inscription_count DESC LIMIT 1",
        stated: 12_835.0,
        tolerance: 0.0,
        as_written: "12,835 Ordinals inscriptions",
    },
    RecordDerivation {
        slug: "most-inscriptions-block",
        kind: RecordKind::BlockFact,
        sql: "SELECT height, inscription_bytes/1000.0 FROM blocks \
              WHERE height = 806480",
        stated: 766.0,
        tolerance: 0.5,
        as_written: "766 KB",
    },
    RecordDerivation {
        slug: "most-taproot-spends",
        kind: RecordKind::Superlative,
        sql: "SELECT height, p2tr_count FROM blocks \
              ORDER BY p2tr_count DESC LIMIT 1",
        stated: 22_367.0,
        tolerance: 0.0,
        as_written: "22,367 Taproot (P2TR) outputs",
    },
    RecordDerivation {
        slug: "most-taproot-spends",
        kind: RecordKind::BlockFact,
        sql: "SELECT height, tx_count FROM blocks WHERE height = 840655",
        stated: 186.0,
        tolerance: 0.0,
        as_written: "186 transactions",
    },
    // The sentence that makes the record readable, and the one whose absence
    // let the original error ship: 22,367 inputs cannot fit in 327.
    RecordDerivation {
        slug: "most-taproot-spends",
        kind: RecordKind::BlockFact,
        sql: "SELECT height, input_count FROM blocks WHERE height = 840655",
        stated: 327.0,
        tolerance: 0.0,
        as_written: "327 inputs",
    },
    // The fee belongs to one transaction, and `max_tx_fee` holds exactly
    // that, so this is an equality rather than a bound. It lands on
    // 20,000,000,000 sats to the satoshi, which is itself the evidence that
    // the figure was typed by a human rather than calculated.
    RecordDerivation {
        slug: "fat-finger-200btc",
        kind: RecordKind::BlockFact,
        sql: "SELECT height, max_tx_fee/100000000.0 FROM blocks \
              WHERE height = 254642",
        stated: 200.0,
        tolerance: 0.0,
        as_written: "200 BTC fee",
    },
];

/// Figures in a record's copy that no query can re-measure, with the reason.
///
/// Kept per slug rather than global for the reason `CONFIGURED_FIGURES` in
/// `registry.rs` is: an exemption has to be no wider than its justification.
/// A bare "200" waved through everywhere would cover a real 200-block count
/// on some future entry.
const FIGURES_WITHOUT_A_DERIVATION: &[(&str, &str, &str)] = &[
    (
        "most-transactions-block",
        "1",
        "the pre-SegWit 1 MB block size limit, fixed by the consensus rules \
         of the time",
    ),
    (
        "most-taproot-spends",
        "120",
        "22,367 divided by 186, worked through for the reader rather than \
         measured",
    ),
    (
        "fat-finger-200btc",
        "20,000",
        "the fee's USD value in 2013, a price claim and not a chain \
         measurement",
    ),
    (
        "highest-fee-block",
        "14.8",
        "the transaction's fee rate. `blocks` stores that transaction's fee \
         in `max_tx_fee` but not its vsize, so the rate is not derivable \
         from stored columns",
    ),
];

/// Every figure a `Records` entry prints, as whole numeric tokens.
///
/// Deliberately blunter than `measurement_figures` in `registry.rs`, which
/// only accepts a figure that sits beside a hedge or a known unit. That
/// heuristic is tuned for chart copy and would read straight past
/// "created 26,908 new UTXOs" and "swept 20,000 anyone-can-spend dust
/// outputs", which is the exact failure mode this file exists to close. Nine
/// curated entries are short enough to take every token and account for it.
fn figures_in(entry: &HallOfFameEntry) -> Vec<String> {
    let mut out = Vec::new();
    for text in record_text(entry) {
        for word in text.split_whitespace() {
            // Trim punctuation but not letters, so an identifier with a digit
            // in it keeps the letters that disqualify it. Trimming to the
            // first digit instead turned "(P2TR)" into the figure "2".
            let core = word
                .trim_matches(|c: char| {
                    !c.is_ascii_alphanumeric()
                        && c != '%'
                        && c != '.'
                        && c != ','
                })
                .trim_end_matches(['.', ',']);
            if core.starts_with(|c: char| c.is_ascii_digit())
                && core
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, ',' | '.' | '%'))
            {
                out.push(core.to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The reader-facing strings of an entry, which are what a figure can hide in.
fn record_text(entry: &HallOfFameEntry) -> Vec<&'static str> {
    let mut v = vec![entry.title, entry.description];
    v.extend(entry.short_context);
    v
}

/// The numeric token a derivation guards, taken off the front of
/// `as_written` so the two cannot disagree.
fn figure_token(as_written: &str) -> &str {
    as_written
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(['.', ','])
}

fn records() -> impl Iterator<Item = &'static HallOfFameEntry> {
    HALL_OF_FAME
        .iter()
        .filter(|e| e.category == HofCategory::Records)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record with no derivation is a superlative nobody is checking.
    #[test]
    fn every_record_entry_has_at_least_one_derivation() {
        let missing: Vec<&str> = records()
            .map(|e| e.slug)
            .filter(|s| !RECORD_DERIVATIONS.iter().any(|d| d.slug == *s))
            .collect();
        assert!(
            missing.is_empty(),
            "these Records entries claim a figure with nothing re-measuring \
             it. Add a row to RECORD_DERIVATIONS:\n  {}",
            missing.join("\n  ")
        );

        // And the reverse, which is how a derivation outlives the entry it
        // guarded and starts passing against nothing.
        let orphans: Vec<&str> = RECORD_DERIVATIONS
            .iter()
            .map(|d| d.slug)
            .filter(|s| !records().any(|e| e.slug == *s))
            .collect();
        assert!(
            orphans.is_empty(),
            "these derivations name no Records entry:\n  {}",
            orphans.join("\n  ")
        );
    }

    /// The derivation and the sentence must agree on what the number is.
    ///
    /// This is the half that runs without a database, and it is the half that
    /// catches the likeliest mistake: editing a figure in the copy and
    /// leaving the row behind, or correcting the row and not the copy.
    #[test]
    fn every_derivation_quotes_a_figure_its_entry_prints() {
        for d in RECORD_DERIVATIONS {
            let entry = records()
                .find(|e| e.slug == d.slug)
                .unwrap_or_else(|| panic!("no entry for {:?}", d.slug));
            assert!(
                record_text(entry).iter().any(|t| t.contains(d.as_written)),
                "entry {:?} does not contain {:?}, so this derivation is \
                 guarding a sentence that no longer exists",
                d.slug,
                d.as_written
            );

            // The stated value has to be the figure as printed, or the
            // measurement is compared against something the reader never saw.
            let printed: f64 = figure_token(d.as_written)
                .replace(',', "")
                .parse()
                .unwrap_or_else(|_| {
                    panic!("{:?} does not start with a number", d.as_written)
                });
            assert!(
                (printed - d.stated).abs() < f64::EPSILON,
                "entry {:?} prints {printed} but the derivation states \
                 {}; the gate would pass while the copy is wrong",
                d.slug,
                d.stated
            );
        }
    }

    /// You cannot ship a record figure nothing accounts for.
    ///
    /// Two exemptions are structural rather than listed, because both are
    /// already guarded elsewhere and a hand-written list of them would rot:
    /// the entry's own block height, which `block_heights_are_plausible` and
    /// every `Superlative` row already pin, and the year of its own `date`,
    /// which `entries_are_structurally_sound` parses.
    #[test]
    fn you_cannot_ship_an_unguarded_record_figure() {
        let mut unguarded: Vec<String> = Vec::new();
        for entry in records() {
            let own_height = entry.block.map(group_digits);
            let own_year = &entry.date[..4];
            for fig in figures_in(entry) {
                if Some(&fig) == own_height.as_ref() || fig == own_year {
                    continue;
                }
                let derived = RECORD_DERIVATIONS.iter().any(|d| {
                    d.slug == entry.slug && figure_token(d.as_written) == fig
                });
                let excused = FIGURES_WITHOUT_A_DERIVATION
                    .iter()
                    .any(|(s, f, _)| *s == entry.slug && *f == fig);
                if !derived && !excused {
                    unguarded.push(format!("{}: {fig}", entry.slug));
                }
            }
        }
        assert!(
            unguarded.is_empty(),
            "these record figures have nothing watching them. Add a row to \
             RECORD_DERIVATIONS, or if no query can reach the value, to \
             FIGURES_WITHOUT_A_DERIVATION with the reason:\n  {}",
            unguarded.join("\n  ")
        );

        // An exemption for a figure that is no longer in the copy is debt
        // pretending to be a decision.
        let stale: Vec<String> = FIGURES_WITHOUT_A_DERIVATION
            .iter()
            .filter(|(s, f, _)| {
                !records().any(|e| {
                    e.slug == *s && figures_in(e).iter().any(|g| g == f)
                })
            })
            .map(|(s, f, _)| format!("{s}: {f}"))
            .collect();
        assert!(
            stale.is_empty(),
            "these exemptions name a figure the copy no longer prints, so \
             delete them:\n  {}",
            stale.join("\n  ")
        );
    }

    /// Render a height the way the copy does, so the two can be compared.
    fn group_digits(n: u64) -> String {
        let s = n.to_string();
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
        }
        out
    }

    #[test]
    fn heights_are_grouped_the_way_the_copy_writes_them() {
        assert_eq!(group_digits(836_964), "836,964");
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(1_000), "1,000");
        assert_eq!(group_digits(170), "170");
    }

    /// Every record, re-measured against the chain.
    ///
    /// Ignored because it needs `bitcoin_stats.db`, which is 680 MB and
    /// gitignored, so CI has no data. Run it when a record is edited, and
    /// periodically: a superlative is a claim about every block mined since,
    /// so it can be falsified by a block nobody looked at.
    #[test]
    #[ignore]
    fn the_copy_still_matches_the_records() {
        let conn = rusqlite::Connection::open_with_flags(
            "bitcoin_stats.db",
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("bitcoin_stats.db in the working directory");

        let mut failures: Vec<String> = Vec::new();
        for d in RECORD_DERIVATIONS {
            let entry = records()
                .find(|e| e.slug == d.slug)
                .unwrap_or_else(|| panic!("no entry for {:?}", d.slug));
            let (height, value): (u64, f64) = conn
                .query_row(d.sql, [], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap_or_else(|e| panic!("{}: {e}\n{}", d.slug, d.sql));

            if d.kind == RecordKind::Superlative && entry.block != Some(height)
            {
                failures.push(format!(
                    "{}: the record moved. The copy names block {:?}, the \
                     chain now says {height}. The entry needs rewriting, not \
                     just renumbering.",
                    d.slug, entry.block
                ));
                continue;
            }

            let ok = match d.kind {
                RecordKind::ContainsAtLeast => value >= d.stated,
                _ => (value - d.stated).abs() <= d.tolerance,
            };
            if !ok {
                failures.push(format!(
                    "{}: copy says {:?}, chain says {value}",
                    d.slug, d.as_written
                ));
            }
        }

        assert!(
            failures.is_empty(),
            "{} of {} record claims no longer hold:\n  {}",
            failures.len(),
            RECORD_DERIVATIONS.len(),
            failures.join("\n  ")
        );
    }
}
