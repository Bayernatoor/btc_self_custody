//! Formatting and utility helpers for the stats page.

use chrono::Datelike;
use leptos::prelude::*;

/// Create a reactive chart description that changes based on whether
/// the current range shows per-block or daily-averaged data.
pub fn chart_desc(
    range: ReadSignal<String>,
    per_block: &'static str,
    daily: &'static str,
) -> Signal<String> {
    Signal::derive(move || {
        if range_to_blocks(&range.get()) > 5_000 {
            daily.to_string()
        } else {
            per_block.to_string()
        }
    })
}

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
                .expect("Jan 1 is always valid")
                .and_hms_opt(0, 0, 0)
                .expect("00:00:00 is always valid")
                .and_utc();
            let days = (now - jan1).num_days().max(1) as u64;
            days * 144
        }
        "1y" => 52_560,
        "2y" => 105_120,
        "5y" => 262_800,
        "10y" => 525_600,
        "all" => 999_999,
        "custom" => 999_999, // custom ranges always use daily aggregates
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
        .map(|c| std::str::from_utf8(c).expect("digit chars are valid UTF-8"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Format a large number in compact human-readable form (1.24M, 84.2K, etc.).
pub fn format_compact(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format_number(n)
    }
}

/// Format bytes as human-readable size (MB or GB).
pub fn format_data_size(bytes: u64) -> String {
    let gb = bytes as f64 / 1_000_000_000.0;
    if gb >= 1.0 {
        format!("{:.2} GB", gb)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    }
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
        .map(|c| std::str::from_utf8(c).expect("digit chars are valid UTF-8"))
        .collect::<Vec<_>>()
        .join(",");
    if parts.len() > 1 {
        format!("{}.{}", formatted, parts[1])
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_number_zero() {
        assert_eq!(format_number(0), "0");
    }

    #[test]
    fn format_number_small() {
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn format_number_thousands() {
        assert_eq!(format_number(1_000), "1,000");
        assert_eq!(format_number(52_416), "52,416");
    }

    #[test]
    fn format_number_millions() {
        assert_eq!(format_number(182_300_000), "182,300,000");
    }

    #[test]
    fn format_compact_small() {
        assert_eq!(format_compact(0), "0");
        assert_eq!(format_compact(999), "999");
        assert_eq!(format_compact(9_999), "9,999");
    }

    #[test]
    fn format_compact_thousands() {
        assert_eq!(format_compact(10_000), "10.0K");
        assert_eq!(format_compact(84_200), "84.2K");
        assert_eq!(format_compact(999_999), "1000.0K");
    }

    #[test]
    fn format_compact_millions() {
        assert_eq!(format_compact(1_000_000), "1.00M");
        assert_eq!(format_compact(1_240_000), "1.24M");
        assert_eq!(format_compact(182_300_000), "182.30M");
    }

    #[test]
    fn format_compact_billions() {
        assert_eq!(format_compact(1_000_000_000), "1.00B");
        assert_eq!(format_compact(48_200_000_000), "48.20B");
    }

    #[test]
    fn format_f64_with_commas() {
        assert_eq!(format_number_f64(3241.5, 2), "3,241.50");
        assert_eq!(format_number_f64(0.12, 2), "0.12");
        assert_eq!(format_number_f64(1_000_000.0, 0), "1,000,000");
    }

    #[test]
    fn range_to_blocks_presets() {
        assert_eq!(range_to_blocks("1d"), 144);
        assert_eq!(range_to_blocks("1w"), 1_008);
        assert_eq!(range_to_blocks("1y"), 52_560);
        assert_eq!(range_to_blocks("all"), 999_999);
    }

    #[test]
    fn range_to_blocks_unknown_defaults() {
        assert_eq!(range_to_blocks("invalid"), 12_960);
    }
}
