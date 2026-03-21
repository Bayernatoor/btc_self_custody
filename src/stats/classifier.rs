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
    DataCarrier,
}

use OpReturnType::DataCarrier;

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

    DataCarrier
}

/// Decode coinbase hex to ASCII and identify the mining pool.
/// Uses case-insensitive substring matching against known pool tags.
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
        ("ocean.xyz", "OCEAN"),
        ("ocean", "OCEAN"),
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

    "Unknown".to_string()
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
