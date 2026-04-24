//! Notable-transaction detection.
//!
//! Classifies parsed transactions into categories (whale, consolidation,
//! fan-out, etc.) for the Lookout SSE stream and the notable-tx persistence
//! path. Kept separate from `zmq_subscriber` so the ZMQ module owns only
//! socket I/O + byte parsing, and so future code paths (historical backfill,
//! batch reclassification) can call the same classifier without pulling in
//! ZMQ machinery.
//!
//! ## Inputs / outputs
//!
//! - [`ParsedTx`] — the minimal transaction shape the classifier needs
//!   (value, input/output counts, witness size, inscription marker, OP_RETURN
//!   text). Populated by `zmq_subscriber::parse_raw_tx`.
//! - [`classify_notable`] — returns [`NotableFlags`], a struct where each
//!   category has its own boolean plus a [`NotableFlags::primary_type`]
//!   helper that picks the highest-priority label for persistence/logging.
//!   Returning the full struct (rather than just an `Option<&str>`) lets
//!   callers read individual flags for the SSE broadcast without
//!   recomputing the threshold logic.
//!
//! ## Byte helpers
//!
//! [`has_inscription_marker`] and [`extract_readable_text`] live here
//! because they inform classification directly (inscription envelope
//! detection, human-readable OP_RETURN filtering) and are called from the
//! parser before [`ParsedTx`] is constructed.

// === Thresholds ===

/// Minimum USD value to flag a transaction as a whale tx.
pub const WHALE_THRESHOLD_USD: f64 = 1_000_000.0;

/// Fee rate above which a tx is flagged as a fee outlier (sat/vB).
/// Raised from 500 to 2000 to reduce false positives during mempool congestion.
pub const FEE_RATE_OUTLIER_THRESHOLD: f64 = 2000.0;

/// Absolute fee above which a tx is flagged as a fee outlier (satoshis = 0.1 BTC).
/// Raised from 0.05 BTC to avoid flagging large consolidations.
pub const FEE_ABSOLUTE_OUTLIER_THRESHOLD: u64 = 10_000_000;

/// Input count above which a tx is flagged as a consolidation (with few outputs).
pub const CONSOLIDATION_INPUT_THRESHOLD: u64 = 50;

/// Output count above which a tx is flagged as a fan-out (with few inputs).
/// Raised from 50 to 100 to focus on genuine batch payouts.
pub const FAN_OUT_OUTPUT_THRESHOLD: u64 = 100;

/// Witness data size above which a tx is flagged as a large inscription (bytes).
pub const LARGE_INSCRIPTION_THRESHOLD: u64 = 100_000;

/// Exact round BTC amounts to detect (in satoshis). Humans often send round numbers.
pub const ROUND_NUMBER_AMOUNTS: &[u64] = &[
    100_000_000,     // 1 BTC
    1_000_000_000,   // 10 BTC
    10_000_000_000,  // 100 BTC
    100_000_000_000, // 1000 BTC
];

/// Tolerance for round number detection (sats). Exact match up to dust threshold.
pub const ROUND_NUMBER_TOLERANCE: u64 = 1_000;

/// Minimum USD value for a round number tx to be flagged (avoids 1 BTC dust at low prices).
pub const ROUND_NUMBER_MIN_USD: f64 = 100_000.0;

/// Ordinals inscription envelope marker: OP_FALSE(00) OP_IF(63) OP_PUSH3(03) "ord"(6f7264).
/// A witness item containing this byte sequence is treated as an Ordinals inscription.
pub const INSCRIPTION_ENVELOPE: &[u8] = &[0x00, 0x63, 0x03, 0x6f, 0x72, 0x64];

// === Types ===

/// Minimal parsed info from a raw Bitcoin transaction — just enough to
/// classify it. Populated by the raw-tx parser, consumed by
/// [`classify_notable`] and by the SSE broadcast path.
pub struct ParsedTx {
    pub txid: String,
    /// Sum of output values in sats.
    pub value: u64,
    pub input_count: u64,
    pub output_count: u64,
    /// Total witness data size in bytes.
    pub witness_bytes: u64,
    /// True if any witness item contains the Ordinals envelope marker.
    pub has_inscription: bool,
    /// Largest single output value in sats (used for round-number detection).
    pub max_output_value: u64,
    /// First OP_RETURN output decoded as readable ASCII, if any.
    pub op_return_text: Option<String>,
}

/// Individual classification flags computed by [`classify_notable`].
///
/// Returning the full struct (rather than only a primary label) lets the
/// SSE broadcast emit each flag directly without re-running the threshold
/// checks. [`primary_type`](Self::primary_type) picks the highest-priority
/// label for persistence/logging.
#[derive(Debug, Default, Clone, Copy)]
pub struct NotableFlags {
    pub whale: bool,
    pub fee_outlier: bool,
    pub consolidation: bool,
    pub fan_out: bool,
    pub large_inscription: bool,
    pub round_number: bool,
    pub op_return_msg: bool,
}

impl NotableFlags {
    /// Highest-priority label if any flag is set. Priority order is
    /// value → structural → fee → data: whale beats structural patterns
    /// (consolidation, fan-out, inscription) which beat fee outliers
    /// which beats opportunistic OP_RETURN messages.
    pub fn primary_type(&self) -> Option<&'static str> {
        if self.whale {
            Some("whale")
        } else if self.round_number {
            Some("round_number")
        } else if self.large_inscription {
            Some("large_inscription")
        } else if self.consolidation {
            Some("consolidation")
        } else if self.fan_out {
            Some("fan_out")
        } else if self.fee_outlier {
            Some("fee_outlier")
        } else if self.op_return_msg {
            Some("op_return_msg")
        } else {
            None
        }
    }

    /// Convenience: `primary_type().is_some()`.
    pub fn is_notable(&self) -> bool {
        self.primary_type().is_some()
    }
}

// === Classification ===

/// Compute all notable-tx flags for a parsed transaction.
///
/// `fee` and `fee_rate` come from `getmempoolentry`; `price_usd` is the
/// currently cached BTC/USD price (0.0 disables USD-dependent flags
/// cleanly — whales and round-numbers simply never fire).
pub fn classify_notable(
    parsed: &ParsedTx,
    fee: u64,
    fee_rate: f64,
    price_usd: f64,
) -> NotableFlags {
    let whale = if price_usd > 0.0 {
        parsed.value as f64 * price_usd / 100_000_000.0 >= WHALE_THRESHOLD_USD
    } else {
        false
    };

    let fee_outlier = fee_rate >= FEE_RATE_OUTLIER_THRESHOLD
        || fee >= FEE_ABSOLUTE_OUTLIER_THRESHOLD;

    let consolidation = parsed.input_count >= CONSOLIDATION_INPUT_THRESHOLD
        && parsed.output_count <= 3;

    let fan_out = parsed.input_count <= 3
        && parsed.output_count >= FAN_OUT_OUTPUT_THRESHOLD;

    let large_inscription = parsed.has_inscription
        && parsed.witness_bytes >= LARGE_INSCRIPTION_THRESHOLD;

    let round_number = if price_usd > 0.0 {
        let matches_round = ROUND_NUMBER_AMOUNTS.iter().any(|&amt| {
            parsed.max_output_value >= amt.saturating_sub(ROUND_NUMBER_TOLERANCE)
                && parsed.max_output_value <= amt + ROUND_NUMBER_TOLERANCE
        });
        matches_round
            && parsed.max_output_value as f64 * price_usd / 100_000_000.0
                >= ROUND_NUMBER_MIN_USD
    } else {
        false
    };

    let op_return_msg = parsed.op_return_text.is_some();

    NotableFlags {
        whale,
        fee_outlier,
        consolidation,
        fan_out,
        large_inscription,
        round_number,
        op_return_msg,
    }
}

// === Byte helpers ===

/// True if a witness item contains the Ordinals inscription envelope
/// anywhere within its bytes. Used by the parser while scanning each input's
/// witness data; promoting the inline check to a named helper keeps
/// classification primitives co-located.
pub fn has_inscription_marker(witness_item: &[u8]) -> bool {
    witness_item.len() >= INSCRIPTION_ENVELOPE.len()
        && witness_item
            .windows(INSCRIPTION_ENVELOPE.len())
            .any(|w| w == INSCRIPTION_ENVELOPE)
}

/// Extract readable ASCII text from an OP_RETURN payload. Returns `Some`
/// only when the result looks like a genuine human message: at least one
/// substantial alphabetic word (≥6 chars) or multiple shorter words with
/// natural separators. Rejects binary protocol fragments (Runes, SRC-20
/// headers, etc.) that happen to contain a few printable bytes.
pub fn extract_readable_text(payload: &[u8]) -> Option<String> {
    // Skip push opcodes at start (OP_PUSH* or OP_PUSHDATA*)
    let mut start = 0;
    while start < payload.len() && payload[start] < 0x20 {
        start += 1;
        if start >= payload.len() {
            return None;
        }
    }
    let slice = &payload[start..];

    // Require high printable ratio (>= 70%) to filter binary noise
    let printable_count =
        slice.iter().filter(|&&b| (0x20..=0x7e).contains(&b)).count();
    if printable_count < 6 || printable_count * 100 < slice.len() * 70 {
        return None;
    }

    // Build string from printable chars, collapse runs of non-printable to single space
    let mut result = String::with_capacity(slice.len());
    let mut last_space = false;
    for &b in slice {
        if (0x20..=0x7e).contains(&b) {
            result.push(b as char);
            last_space = false;
        } else if !last_space {
            result.push(' ');
            last_space = true;
        }
    }
    let trimmed = result.trim().to_string();

    // Strip non-alphanumeric leading/trailing chars. Protocol fragments often
    // have noise prefixes like "=|" before their readable part. Real messages
    // typically start and end with word characters.
    let core: String = trimmed
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_string();

    if core.len() < 6 {
        return None;
    }

    // Core must be mostly letters (>= 50%)
    let letter_count = core.chars().filter(|c| c.is_alphabetic()).count();
    if letter_count < 5 || letter_count * 2 < core.len() {
        return None;
    }

    // Require at least one word of 6+ consecutive alphabetic chars OR
    // multiple shorter words with natural separators (real sentences).
    // Catches "SATFLOW", "Bitcoin", "EXODUS", "Hello world" but rejects
    // "lifiQ" (5 chars, often a fragment of binary protocol data).
    let longest_word = core
        .split(|c: char| !c.is_alphabetic())
        .map(|w| w.len())
        .max()
        .unwrap_or(0);
    let word_count = core
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.len() >= 3)
        .count();
    let has_natural_separators =
        core.contains(' ') || core.contains('.') || core.contains(',');
    // Accept if: one substantial word (6+) OR multiple words with separators
    if longest_word < 6 && !(word_count >= 2 && has_natural_separators) {
        return None;
    }

    Some(trimmed.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a ParsedTx with sensible defaults. Override specific fields.
    fn make_tx() -> ParsedTx {
        ParsedTx {
            txid: "aaaa".to_string(),
            value: 50_000, // 0.0005 BTC
            input_count: 1,
            output_count: 2,
            witness_bytes: 200,
            has_inscription: false,
            max_output_value: 40_000,
            op_return_text: None,
        }
    }

    const TEST_PRICE: f64 = 100_000.0; // $100k/BTC for easy math

    /// Convenience: call classify and collapse to its primary type label.
    /// Keeps the test assertions mirror the old `Option<&str>` API for
    /// readability.
    fn primary(
        parsed: &ParsedTx,
        fee: u64,
        fee_rate: f64,
        price_usd: f64,
    ) -> Option<&'static str> {
        classify_notable(parsed, fee, fee_rate, price_usd).primary_type()
    }

    // --- 1. WHALE ---

    #[test]
    fn whale_above_threshold() {
        // 15 BTC total output at $100k = $1.5M > $1M threshold
        let tx = ParsedTx {
            value: 15_0000_0000,
            max_output_value: 15_0000_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), Some("whale"));
    }

    #[test]
    fn whale_below_threshold() {
        // 5 BTC total output at $100k = $500k < $1M threshold
        let tx = ParsedTx {
            value: 5_0000_0000,
            max_output_value: 5_0000_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), None);
    }

    #[test]
    fn whale_exactly_at_threshold() {
        // 10 BTC at $100k = exactly $1M
        let tx = ParsedTx {
            value: 10_0000_0000,
            max_output_value: 10_0000_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), Some("whale"));
    }

    #[test]
    fn whale_no_price_available() {
        // Price = 0 means whales can't be detected
        let tx = ParsedTx {
            value: 100_0000_0000,
            max_output_value: 100_0000_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, 0.0), None);
    }

    #[test]
    fn whale_takes_priority_over_consolidation() {
        // 100 inputs, 1 output, 20 BTC = whale AND consolidation. Whale wins.
        let tx = ParsedTx {
            value: 20_0000_0000,
            max_output_value: 20_0000_0000,
            input_count: 100,
            output_count: 1,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 5000, 3.0, TEST_PRICE), Some("whale"));
    }

    // --- 2. ROUND NUMBER ---

    #[test]
    fn round_number_exact_1_btc() {
        // Exactly 1 BTC output at $100k = $100k, meets min threshold
        // Total value stays below $1M so whale doesn't trigger
        let tx = ParsedTx {
            value: 1_5000_0000, // 1.5 BTC total (1 + 0.5 change)
            max_output_value: 1_0000_0000, // 1 BTC output
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), Some("round_number"));
    }

    #[test]
    fn round_number_10_btc() {
        // Test with lower price: 10 BTC at $50k = $525k total (not whale),
        // max output = $500k (> $100k min).
        let tx = ParsedTx {
            value: 10_5000_0000,            // 10.5 BTC total
            max_output_value: 10_0000_0000, // 10 BTC round output
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, 50_000.0), Some("round_number"));
    }

    #[test]
    fn round_number_with_tolerance() {
        // 0.999999 BTC (100 sats off 1 BTC, within 1_000 sat tolerance).
        // At $101k the max output = $100,999.90 > $100k min.
        let tx = ParsedTx {
            value: 1_5000_0000,
            max_output_value: 99_999_900,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, 101_000.0), Some("round_number"));
    }

    #[test]
    fn round_number_outside_tolerance() {
        // 0.99 BTC (0.01 BTC off, well outside 1_000 sat tolerance)
        let tx = ParsedTx {
            value: 1_5000_0000,
            max_output_value: 9900_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), None);
    }

    #[test]
    fn round_number_1_btc_at_low_price() {
        // 1 BTC at $50k = $50k < $100k min -> not flagged
        let tx = ParsedTx {
            value: 1_5000_0000,
            max_output_value: 1_0000_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 500, 5.0, 50_000.0), None);
    }

    #[test]
    fn round_number_whale_takes_priority() {
        // 100 BTC round output + total > $1M = whale takes priority
        let tx = ParsedTx {
            value: 105_0000_0000,
            max_output_value: 100_0000_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), Some("whale"));
    }

    // --- 3. LARGE INSCRIPTION ---

    #[test]
    fn inscription_with_envelope_and_large_witness() {
        let tx = ParsedTx {
            witness_bytes: 200_000,
            has_inscription: true,
            ..make_tx()
        };
        assert_eq!(
            primary(&tx, 500, 2.0, TEST_PRICE),
            Some("large_inscription")
        );
    }

    #[test]
    fn inscription_small_witness_ignored() {
        // Has envelope but witness is only 50KB (below 100KB threshold)
        let tx = ParsedTx {
            witness_bytes: 50_000,
            has_inscription: true,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 500, 2.0, TEST_PRICE), None);
    }

    #[test]
    fn large_witness_without_envelope_not_inscription() {
        // 200KB witness but NO inscription envelope = not an inscription
        // (the consolidation-with-many-sigs case)
        let tx = ParsedTx {
            witness_bytes: 200_000,
            has_inscription: false,
            input_count: 500,
            output_count: 1,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 500, 2.0, TEST_PRICE), Some("consolidation"));
    }

    // --- 4. CONSOLIDATION ---

    #[test]
    fn consolidation_classic() {
        let tx = ParsedTx {
            input_count: 100,
            output_count: 1,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 2000, 2.0, TEST_PRICE), Some("consolidation"));
    }

    #[test]
    fn consolidation_with_change() {
        // 50 inputs -> 2 outputs (1 destination + 1 change)
        let tx = ParsedTx {
            input_count: 50,
            output_count: 2,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 2000, 2.0, TEST_PRICE), Some("consolidation"));
    }

    #[test]
    fn consolidation_at_threshold() {
        // Exactly 50 inputs -> 3 outputs (boundary)
        let tx = ParsedTx {
            input_count: 50,
            output_count: 3,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 2.0, TEST_PRICE), Some("consolidation"));
    }

    #[test]
    fn consolidation_below_threshold() {
        let tx = ParsedTx {
            input_count: 49,
            output_count: 1,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 2.0, TEST_PRICE), None);
    }

    #[test]
    fn consolidation_too_many_outputs() {
        // 100 inputs -> 4 outputs = not consolidation
        let tx = ParsedTx {
            input_count: 100,
            output_count: 4,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 2.0, TEST_PRICE), None);
    }

    #[test]
    fn consolidation_with_high_fee_keeps_consolidation_type() {
        // 200 inputs, 1 output, high fee rate. Should be consolidation, not fee_outlier.
        let tx = ParsedTx {
            input_count: 200,
            output_count: 1,
            ..make_tx()
        };
        assert_eq!(
            primary(&tx, 50_000_000, 3000.0, TEST_PRICE),
            Some("consolidation")
        );
    }

    // --- 5. FAN-OUT ---

    #[test]
    fn fan_out_classic() {
        let tx = ParsedTx {
            input_count: 1,
            output_count: 200,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 5000, 5.0, TEST_PRICE), Some("fan_out"));
    }

    #[test]
    fn fan_out_at_threshold() {
        // 2 inputs -> 100 outputs (boundary)
        let tx = ParsedTx {
            input_count: 2,
            output_count: 100,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 5000, 5.0, TEST_PRICE), Some("fan_out"));
    }

    #[test]
    fn fan_out_below_threshold() {
        let tx = ParsedTx {
            input_count: 2,
            output_count: 99,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 5000, 5.0, TEST_PRICE), None);
    }

    #[test]
    fn fan_out_too_many_inputs() {
        // 4 inputs -> 200 outputs: not fan_out (>3 inputs)
        let tx = ParsedTx {
            input_count: 4,
            output_count: 200,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 5000, 5.0, TEST_PRICE), None);
    }

    #[test]
    fn fan_out_not_consolidation() {
        // 50 inputs -> 200 outputs: neither
        let tx = ParsedTx {
            input_count: 50,
            output_count: 200,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 5000, 5.0, TEST_PRICE), None);
    }

    // --- 6. FEE OUTLIER ---

    #[test]
    fn fee_outlier_high_rate() {
        let tx = make_tx();
        assert_eq!(
            primary(&tx, 500_000, 2500.0, TEST_PRICE),
            Some("fee_outlier")
        );
    }

    #[test]
    fn fee_outlier_high_absolute() {
        let tx = make_tx();
        // 0.15 BTC fee = 15_000_000 sats > 10_000_000 threshold
        assert_eq!(
            primary(&tx, 15_000_000, 100.0, TEST_PRICE),
            Some("fee_outlier")
        );
    }

    #[test]
    fn fee_outlier_below_both_thresholds() {
        let tx = make_tx();
        assert_eq!(primary(&tx, 9_000_000, 1999.0, TEST_PRICE), None);
    }

    #[test]
    fn fee_outlier_at_rate_boundary() {
        let tx = make_tx();
        // Exactly 2000 sat/vB triggers (>=)
        assert_eq!(
            primary(&tx, 400_000, 2000.0, TEST_PRICE),
            Some("fee_outlier")
        );
    }

    #[test]
    fn fee_outlier_consolidation_takes_priority() {
        // Big consolidation with high fee rate. Consolidation wins.
        let tx = ParsedTx {
            input_count: 100,
            output_count: 1,
            ..make_tx()
        };
        assert_eq!(
            primary(&tx, 30_000_000, 3000.0, TEST_PRICE),
            Some("consolidation")
        );
    }

    // --- 7. OP_RETURN MESSAGE ---

    #[test]
    fn op_return_msg_detected() {
        let tx = ParsedTx {
            op_return_text: Some(
                "Hello from the Bitcoin blockchain!".to_string(),
            ),
            ..make_tx()
        };
        assert_eq!(primary(&tx, 500, 5.0, TEST_PRICE), Some("op_return_msg"));
    }

    #[test]
    fn op_return_no_text() {
        let tx = make_tx();
        assert_eq!(primary(&tx, 500, 5.0, TEST_PRICE), None);
    }

    #[test]
    fn op_return_lowest_priority() {
        // Has OP_RETURN text AND is a fee outlier. Fee outlier wins.
        let tx = ParsedTx {
            op_return_text: Some("Some message".to_string()),
            ..make_tx()
        };
        assert_eq!(
            primary(&tx, 15_000_000, 3000.0, TEST_PRICE),
            Some("fee_outlier")
        );
    }

    // --- PRIORITY ORDER / EDGE CASES ---

    #[test]
    fn whale_consolidation_round_all_at_once() {
        // 100 BTC round amount, 500 inputs, 1 output = whale + consolidation + round.
        // Whale wins (highest priority).
        let tx = ParsedTx {
            value: 100_0000_0000,
            max_output_value: 100_0000_0000,
            input_count: 500,
            output_count: 1,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 5000, 2.0, TEST_PRICE), Some("whale"));
    }

    #[test]
    fn inscription_takes_priority_over_consolidation() {
        // 2-input tx with inscription envelope + big witness: should be inscription.
        let tx = ParsedTx {
            input_count: 2,
            output_count: 1,
            witness_bytes: 500_000,
            has_inscription: true,
            ..make_tx()
        };
        assert_eq!(
            primary(&tx, 1000, 5.0, TEST_PRICE),
            Some("large_inscription")
        );
    }

    #[test]
    fn normal_tx_not_notable() {
        let tx = ParsedTx {
            value: 100_000,
            input_count: 2,
            output_count: 2,
            witness_bytes: 200,
            has_inscription: false,
            max_output_value: 90_000,
            op_return_text: None,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 500, 5.0, TEST_PRICE), None);
    }

    // --- ROUND NUMBER EDGE: max_output_value vs total ---

    #[test]
    fn round_number_checks_max_output_not_total() {
        // Total = 3.5 BTC but max single output = 1 BTC (round)
        let tx = ParsedTx {
            value: 3_5000_0000,
            max_output_value: 1_0000_0000,
            output_count: 3,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), Some("round_number"));
    }

    #[test]
    fn round_number_7_btc_not_round() {
        let tx = ParsedTx {
            value: 7_0000_0000,
            max_output_value: 7_0000_0000,
            ..make_tx()
        };
        assert_eq!(primary(&tx, 1000, 5.0, TEST_PRICE), None);
    }

    // --- NotableFlags::is_notable and flag exposure ---

    #[test]
    fn flags_expose_individual_booleans() {
        // A whale-and-fee-outlier tx exposes both flags even though
        // primary_type() only returns "whale".
        let tx = ParsedTx {
            value: 20_0000_0000,
            max_output_value: 20_0000_0000,
            ..make_tx()
        };
        let flags = classify_notable(&tx, 15_000_000, 3000.0, TEST_PRICE);
        assert!(flags.whale);
        assert!(flags.fee_outlier);
        assert!(!flags.consolidation);
        assert!(flags.is_notable());
        assert_eq!(flags.primary_type(), Some("whale"));
    }

    #[test]
    fn flags_all_false_is_not_notable() {
        let flags = NotableFlags::default();
        assert!(!flags.is_notable());
        assert_eq!(flags.primary_type(), None);
    }

    // --- Real-world classifier fixtures (ParsedTx constructed directly) ---

    #[test]
    fn real_multisig_batch_not_flagged() {
        // txid: 3cd122cb06d492d792ecfd46b4facb798c032ab95d1126a39d73e6f9b71cebba
        // 1 input (multisig) -> 16 outputs. Batch payment but below fan_out
        // threshold (100). Should NOT be flagged as notable.
        let tx = ParsedTx {
            txid: "3cd122cb06d492d792ecfd46b4facb798c032ab95d1126a39d73e6f9b71cebba".to_string(),
            value: 10_362_839,
            input_count: 1,
            output_count: 16,
            witness_bytes: 247,
            has_inscription: false,
            max_output_value: 8_481_168,
            op_return_text: None,
        };
        assert_eq!(primary(&tx, 2_000, 3.1, 100_000.0), None);
    }

    #[test]
    fn real_fan_out_177_outputs() {
        // txid: e1289d501169e3dc58fd5e631f8bca12574615f5a49c571fef603888d9522ff9
        let tx = ParsedTx {
            txid: "e1289d501169e3dc58fd5e631f8bca12574615f5a49c571fef603888d9522ff9".to_string(),
            value: 113_983,
            input_count: 2,
            output_count: 177,
            witness_bytes: 208,
            has_inscription: false,
            max_output_value: 15_929,
            op_return_text: None,
        };
        assert_eq!(primary(&tx, 3_000, 0.5, 100_000.0), Some("fan_out"));
    }

    #[test]
    fn real_segwit_consolidation_436_inputs() {
        // txid: 562c22eb7a7d620347d285452a938b7fd78e5c178e11d3dfed3f9e2b90e93013
        // 436 inputs -> 1 output, 44.3KB witness. Consolidation, NOT large_inscription.
        let tx = ParsedTx {
            txid: "562c22eb7a7d620347d285452a938b7fd78e5c178e11d3dfed3f9e2b90e93013".to_string(),
            value: 47_806_688,
            input_count: 436,
            output_count: 1,
            witness_bytes: 45_341,
            has_inscription: false,
            max_output_value: 47_806_688,
            op_return_text: None,
        };
        assert_eq!(primary(&tx, 5_000, 0.17, 100_000.0), Some("consolidation"));
    }

    // --- extract_readable_text tests ---

    #[test]
    fn extract_readable_text_alphabetic() {
        let text = b"Hello, world from Bitcoin!";
        let result = extract_readable_text(text);
        assert!(result.is_some());
        assert!(result.unwrap().contains("Hello"));
    }

    #[test]
    fn extract_readable_text_too_short() {
        assert_eq!(extract_readable_text(b"hi"), None);
    }

    #[test]
    fn extract_readable_text_binary_junk() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        assert_eq!(extract_readable_text(&data), None);
    }

    #[test]
    fn extract_readable_text_needs_letters() {
        assert_eq!(extract_readable_text(b"1234567890"), None);
    }

    #[test]
    fn extract_readable_text_rejects_low_quality() {
        // Noisy fragments with letters but no real message.
        assert_eq!(extract_readable_text(b"=|1ifi T"), None);
        assert_eq!(extract_readable_text(b"x!@#$%^&"), None);
    }

    #[test]
    fn extract_readable_text_rejects_protocol_fragments() {
        // li.fi-style "=|lifi…" headers — not natural text.
        assert_eq!(extract_readable_text(b"=|lifiZ"), None);
        assert_eq!(extract_readable_text(b"=|lifiQ1Oq"), None);
    }

    #[test]
    fn extract_readable_text_satflow() {
        // Real-world OP_RETURN from tx 20ebb964...
        assert!(extract_readable_text(b"SATFLOW").is_some());
    }

    #[test]
    fn extract_readable_text_with_binary_wrapper() {
        // Binary prefix (push opcodes) followed by real text
        let mut data = vec![0x0c]; // push 12 bytes
        data.extend_from_slice(b"Hello Bitcoin!");
        assert!(extract_readable_text(&data).is_some());
    }

    // --- has_inscription_marker ---

    #[test]
    fn inscription_marker_detects_envelope() {
        let witness = [
            0x00, 0x63, 0x03, 0x6f, 0x72, 0x64, // OP_FALSE OP_IF OP_PUSH3 "ord"
            0xAA, 0xBB, 0xCC,
        ];
        assert!(has_inscription_marker(&witness));
    }

    #[test]
    fn inscription_marker_finds_envelope_mid_item() {
        let mut witness = vec![0xDE, 0xAD, 0xBE, 0xEF];
        witness.extend_from_slice(INSCRIPTION_ENVELOPE);
        witness.extend_from_slice(&[0x01, 0x02]);
        assert!(has_inscription_marker(&witness));
    }

    #[test]
    fn inscription_marker_rejects_plain_witness() {
        // Normal signature bytes — no envelope.
        let witness = [0x48; 72];
        assert!(!has_inscription_marker(&witness));
    }

    #[test]
    fn inscription_marker_rejects_short_item() {
        // Too short to even hold the marker.
        let witness = [0x00, 0x63, 0x03];
        assert!(!has_inscription_marker(&witness));
    }
}
