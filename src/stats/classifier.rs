//! Classification of OP_RETURN outputs and mining pool identification.
//!
//! OP_RETURN types:
//! - SegwitCommit: coinbase segwit commitment (6a24aa21a9ed prefix, excluded from totals)
//! - Runes: Runes protocol markers (6a5d prefix or ≤6 byte tiny outputs)
//! - DataCarrier: traditional data embedding (everything else)
//!
//! Miner identification: case-insensitive substring matching on coinbase scriptSig ASCII.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpReturnType {
    SegwitCommit,
    Runes,
    Omni,
    Counterparty,
    DataCarrier,
}

/// Classify a nulldata output by its scriptPubKey hex string.
pub fn classify(hex: &str) -> OpReturnType {
    // SegWit commitment: OP_RETURN OP_PUSHBYTES_36 0xaa21a9ed (always 38 bytes)
    if hex.starts_with("6a24aa21a9ed") {
        return OpReturnType::SegwitCommit;
    }

    // Runes protocol: OP_RETURN OP_13 (0x5d)
    if hex.starts_with("6a5d") {
        return OpReturnType::Runes;
    }

    // Very tiny outputs (≤6 bytes) are typically Runes markers (e.g. 6a023a29)
    // Outputs 7-10 bytes without 6a5d prefix are more likely small data carriers
    let byte_len = hex.len() / 2;
    if byte_len <= 6 {
        return OpReturnType::Runes;
    }

    // After OP_RETURN (6a), the next byte(s) are a push length.
    // We need to check the pushed data payload, which starts after the push opcode(s).
    // For most protocols, the prefix appears right after "6a" + push length byte(s).
    let payload = &hex[2..]; // skip 6a (OP_RETURN)

    // Omni Layer: payload starts with "6f6d6e69" ("omni" in ASCII)
    // Typical format: 6a 14 6f6d6e69... (OP_RETURN OP_PUSHBYTES_20 "omni"...)
    if contains_payload(payload, "6f6d6e69") {
        return OpReturnType::Omni;
    }

    // Counterparty: payload starts with "434e545250525459" ("CNTRPRTY" in ASCII)
    // Typical format: 6a 28 434e545250525459... (OP_RETURN OP_PUSHBYTES_40 "CNTRPRTY"...)
    if contains_payload(payload, "434e545250525459") {
        return OpReturnType::Counterparty;
    }

    OpReturnType::DataCarrier
}

/// Check if the payload (after OP_RETURN) starts with the given prefix
/// after skipping the push-length opcode byte(s).
fn contains_payload(payload: &str, prefix: &str) -> bool {
    // Most OP_RETURN: single push-length byte (OP_PUSHBYTES_1 to OP_PUSHBYTES_75)
    // Data starts at hex offset 2 (1 byte = 2 hex chars for the push length)
    if payload.len() >= 2 + prefix.len() && payload[2..].starts_with(prefix) {
        return true;
    }
    // OP_PUSHDATA1: 0x4c + 1-byte length, data at hex offset 4
    if payload.starts_with("4c")
        && payload.len() >= 4 + prefix.len()
        && payload[4..].starts_with(prefix)
    {
        return true;
    }
    // OP_PUSHDATA2: 0x4d + 2-byte length, data at hex offset 6
    if payload.starts_with("4d")
        && payload.len() >= 6 + prefix.len()
        && payload[6..].starts_with(prefix)
    {
        return true;
    }
    false
}

/// Decode coinbase hex to ASCII and identify the mining pool.
/// Uses case-insensitive substring matching against known pool tags.
///
/// OCEAN template miners: OCEAN lets miners build their own block templates.
/// Coinbase format: `< OCEAN.XYZ >` followed by the miner's tag (e.g. "234 Alberta",
/// "Barefoot Mining"). We extract these dynamically as "OCEAN / <miner>".
pub fn identify_miner(coinbase_hex: &str) -> String {
    let bytes: Vec<u8> = (0..coinbase_hex.len())
        .step_by(2)
        .filter_map(|i| {
            coinbase_hex
                .get(i..i + 2)
                .and_then(|s| u8::from_str_radix(s, 16).ok())
        })
        .collect();
    let ascii = String::from_utf8_lossy(&bytes).to_lowercase();

    // Patterns matched against lowercased coinbase text.
    // Order matters: more specific patterns first to avoid false positives.
    let pools: &[(&str, &str)] = &[
        ("foundry usa", "Foundry USA"),
        ("foundry", "Foundry USA"),
        ("antpool", "AntPool"),
        ("viabtc", "ViaBTC"),
        ("f2pool", "F2Pool"),
        ("binance", "Binance Pool"),
        ("mara pool", "MARA"),
        ("marapool", "MARA"),
        ("mara", "MARA"),
        ("braiins", "Braiins"),
        ("slush", "Braiins"),
        ("sbi crypto", "SBI Crypto"),
        ("sbicrypto", "SBI Crypto"),
        ("luxor", "Luxor"),
        ("whitepool", "WhitePool"),
        ("whitebit", "WhiteBit"),
        ("spiderpool", "SpiderPool"),
        ("spider", "SpiderPool"),
        ("btc.com", "BTC.com"),
        ("btccom", "BTC.com"),
        ("poolin", "Poolin"),
        ("emcd", "EMCD"),
        ("titan", "Titan"),
        ("secpool", "SecPool"),
        ("rawpool", "Rawpool"),
        ("ultimus", "Ultimus Pool"),
        ("sigmapool", "SigmaPool"),
        ("ckpool", "CKPool"),
        ("solo ck", "CKPool"),
        ("kano", "KanoPool"),
    ];

    for (pattern, name) in pools {
        if ascii.contains(pattern) {
            return name.to_string();
        }
    }

    // OCEAN template miners: coinbase contains "ocean.xyz" or "ocean" followed
    // by the miner's own tag. Extract printable ASCII after ">" as the sub-miner.
    if let Some(ocean_pos) = ascii.find("ocean.xyz").or_else(|| ascii.find("ocean")) {
        if let Some(sub_miner) = extract_ocean_subminer(&bytes, ocean_pos) {
            return format!("OCEAN / {sub_miner}");
        }
        return "OCEAN".to_string();
    }

    "Unknown".to_string()
}

/// Extract the sub-miner name from an OCEAN coinbase.
/// Looks for printable ASCII (2+ chars) after the ">" that closes the OCEAN tag.
fn extract_ocean_subminer(bytes: &[u8], ocean_pos: usize) -> Option<String> {
    // Find ">" after the OCEAN tag
    let search_start = ocean_pos;
    let gt_pos = bytes[search_start..].iter().position(|&b| b == b'>')?;
    let after_gt = search_start + gt_pos + 1;

    // Collect printable ASCII chars after ">", skipping leading control chars
    let name: String = bytes[after_gt..]
        .iter()
        .skip_while(|&&b| b < 0x20 || b > 0x7e) // skip control chars
        .take_while(|&&b| b >= 0x20 && b <= 0x7e) // take printable ASCII
        .map(|&b| b as char)
        .collect();

    let trimmed = name.trim();
    if trimmed.len() >= 2 {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Calculate block subsidy in satoshis for a given height.
/// Subsidy starts at 50 BTC (5,000,000,000 sats) and halves every 210,000 blocks.
pub fn block_subsidy(height: u64) -> u64 {
    let halvings = height / 210_000;
    if halvings >= 64 {
        return 0;
    }
    5_000_000_000u64 >> halvings
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // block_subsidy
    // -----------------------------------------------------------------------

    #[test]
    fn subsidy_era_0() {
        assert_eq!(block_subsidy(0), 5_000_000_000); // 50 BTC
        assert_eq!(block_subsidy(1), 5_000_000_000);
        assert_eq!(block_subsidy(209_999), 5_000_000_000);
    }

    #[test]
    fn subsidy_era_1() {
        assert_eq!(block_subsidy(210_000), 2_500_000_000); // 25 BTC
        assert_eq!(block_subsidy(419_999), 2_500_000_000);
    }

    #[test]
    fn subsidy_era_2() {
        assert_eq!(block_subsidy(420_000), 1_250_000_000); // 12.5 BTC
        assert_eq!(block_subsidy(629_999), 1_250_000_000);
    }

    #[test]
    fn subsidy_era_3() {
        assert_eq!(block_subsidy(630_000), 625_000_000); // 6.25 BTC
        assert_eq!(block_subsidy(839_999), 625_000_000);
    }

    #[test]
    fn subsidy_era_4_current() {
        assert_eq!(block_subsidy(840_000), 312_500_000); // 3.125 BTC
        assert_eq!(block_subsidy(941_000), 312_500_000);
    }

    #[test]
    fn subsidy_zero_after_64_halvings() {
        // 64 halvings × 210,000 = block 13,440,000
        assert_eq!(block_subsidy(13_440_000), 0);
        assert_eq!(block_subsidy(100_000_000), 0);
    }

    #[test]
    fn subsidy_last_nonzero_era() {
        // Era 63: subsidy = 5B >> 63 = 0 (right-shift past all bits)
        // Actually 5B >> 63 in u64: 5_000_000_000 has ~33 bits, so >>63 = 0
        // Era 32: 5B >> 32 = 1 sat (last era with nonzero)
        assert!(block_subsidy(32 * 210_000) > 0);
        // Era 33: 5B >> 33 = 0 sats
        assert_eq!(block_subsidy(33 * 210_000), 0);
    }

    // -----------------------------------------------------------------------
    // classify
    // -----------------------------------------------------------------------

    #[test]
    fn classify_segwit_commit() {
        // Real SegWit commitment: OP_RETURN OP_PUSHBYTES_36 0xaa21a9ed...
        let hex = "6a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf9";
        assert_eq!(classify(hex), OpReturnType::SegwitCommit);
    }

    #[test]
    fn classify_runes_prefix() {
        // Runes: OP_RETURN OP_13 (6a5d) + payload
        assert_eq!(classify("6a5d0014"), OpReturnType::Runes);
        assert_eq!(classify("6a5d"), OpReturnType::Runes);
    }

    #[test]
    fn classify_runes_tiny() {
        // ≤6 bytes without 6a5d prefix → Runes heuristic
        assert_eq!(classify("6a023a29"), OpReturnType::Runes); // 4 bytes
        assert_eq!(classify("6a0400000000"), OpReturnType::Runes); // 6 bytes
    }

    #[test]
    fn classify_omni() {
        // Omni Layer: OP_RETURN OP_PUSHBYTES_20 "omni"...
        // 6a + 14 (push 20 bytes) + 6f6d6e69 ("omni") + rest
        let hex = "6a146f6d6e6900000000000000010000000005f5e100";
        assert_eq!(classify(hex), OpReturnType::Omni);
    }

    #[test]
    fn classify_counterparty() {
        // Counterparty: OP_RETURN OP_PUSHBYTES_40 "CNTRPRTY"...
        // 6a + 28 (push 40 bytes) + 434e545250525459 + rest
        let hex = "6a28434e5452505254590000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(classify(hex), OpReturnType::Counterparty);
    }

    #[test]
    fn classify_data_carrier() {
        // Generic data carrier: OP_RETURN + arbitrary data > 6 bytes
        let hex = "6a0f48656c6c6f2c20776f726c6421"; // "Hello, world!"
        assert_eq!(classify(hex), OpReturnType::DataCarrier);
    }

    #[test]
    fn classify_omni_pushdata1() {
        // Omni with OP_PUSHDATA1 (0x4c): 6a + 4c + len + 6f6d6e69...
        let hex = "6a4c146f6d6e6900000000000000010000000005f5e100";
        assert_eq!(classify(hex), OpReturnType::Omni);
    }

    // -----------------------------------------------------------------------
    // identify_miner
    // -----------------------------------------------------------------------

    #[test]
    fn miner_foundry() {
        let hex = hex::encode("Foundry USA Pool");
        assert_eq!(identify_miner(&hex), "Foundry USA");
    }

    #[test]
    fn miner_antpool() {
        let hex = hex::encode("/AntPool/");
        assert_eq!(identify_miner(&hex), "AntPool");
    }

    #[test]
    fn miner_f2pool() {
        let hex = hex::encode("Mined by F2Pool");
        assert_eq!(identify_miner(&hex), "F2Pool");
    }

    #[test]
    fn miner_ocean_plain() {
        let hex = hex::encode("ocean.xyz");
        assert_eq!(identify_miner(&hex), "OCEAN");
    }

    #[test]
    fn miner_ocean_template_234_alberta() {
        // Real OCEAN coinbase pattern: "< OCEAN.XYZ >" + control char + "234 Alberta"
        let coinbase = b"\x03:c\x0e\x1a< OCEAN.XYZ >\x0f234 Alberta\x07\x11\x10AR";
        let hex = coinbase.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        assert_eq!(identify_miner(&hex), "OCEAN / 234 Alberta");
    }

    #[test]
    fn miner_ocean_template_barefoot() {
        let coinbase = b"\x03a\x0e\x1e< OCEAN.XYZ >\x0fBarefoot Mining\x07\x11xK";
        let hex = coinbase.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        assert_eq!(identify_miner(&hex), "OCEAN / Barefoot Mining");
    }

    #[test]
    fn miner_ocean_no_subminer() {
        // OCEAN tag with no readable sub-miner after ">"
        let coinbase = b"< OCEAN.XYZ >\x01\x02\x03";
        let hex = coinbase.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        assert_eq!(identify_miner(&hex), "OCEAN");
    }

    #[test]
    fn miner_unknown() {
        let hex = hex::encode("random coinbase text");
        assert_eq!(identify_miner(&hex), "Unknown");
    }

    #[test]
    fn miner_empty() {
        assert_eq!(identify_miner(""), "Unknown");
    }

    #[test]
    fn miner_case_insensitive() {
        let hex = hex::encode("FOUNDRY USA POOL");
        assert_eq!(identify_miner(&hex), "Foundry USA");
    }

    // Helper: encode string to hex (no external dep needed)
    mod hex {
        pub fn encode(s: &str) -> String {
            s.bytes().map(|b| format!("{:02x}", b)).collect()
        }
    }
}
