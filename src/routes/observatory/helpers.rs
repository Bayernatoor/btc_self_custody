//! Formatting and utility helpers for the stats page.

use chrono::Datelike;

/// Convert a range string (1d, 1w, 1m, etc.) to approximate block count.
pub fn range_to_blocks(range: &str) -> u64 {
    match range {
        "1d" => 144,
        "1w" => 1_008,
        "1m" => 4_320,
        "3m" => 12_960,
        "6m" => 25_920,
        "ytd" => {
            // Days since Jan 1 of current year × 144 blocks/day
            let now = chrono::Utc::now();
            let jan1 = chrono::NaiveDate::from_ymd_opt(now.year(), 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc();
            let days = (now - jan1).num_days().max(1) as u64;
            days * 144
        }
        "1y" => 52_560,
        "2y" => 105_120,
        "5y" => 262_800,
        "10y" => 525_600,
        "all" => 999_999,
        _ => 12_960,
    }
}

/// Format a u64 with comma separators.
pub fn format_number(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}

/// Format a f64 with comma separators and fixed decimal places.
pub fn format_number_f64(n: f64, decimals: usize) -> String {
    let rounded = format!("{:.prec$}", n, prec = decimals);
    // Add commas to the integer part
    let parts: Vec<&str> = rounded.split('.').collect();
    let int_part = parts[0];
    let bytes = int_part.as_bytes();
    let formatted = bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",");
    if parts.len() > 1 {
        format!("{}.{}", formatted, parts[1])
    } else {
        formatted
    }
}
