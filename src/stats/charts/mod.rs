//! ECharts option JSON builders.
//! Runs on the client (WASM) — takes typed data and produces JSON strings
//! that are passed to ECharts via JS interop.

use serde_json::json;

use crate::stats::types::*;

pub mod network;
pub mod fees;
pub mod adoption;
pub mod embedded;
pub mod mining;
pub mod signaling;
pub mod tx_metrics;
pub mod gauges;

pub use network::*;
pub use fees::*;
pub use adoption::*;
pub use embedded::*;
pub use mining::*;
pub use signaling::*;
pub use tx_metrics::*;
pub use gauges::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Consistent chart color palette
pub(crate) const DATA_COLOR: &str = "#f7931a"; // Primary data (bitcoin orange)
pub(crate) const DATA_COLOR_FADED: &str = "rgba(247,147,26,0.15)"; // Primary data area fill
pub(crate) const MA_COLOR: &str = "rgba(255,255,255,0.85)"; // Moving average (white)
pub(crate) const TARGET_COLOR: &str = "#e74c3c"; // Target/reference lines (red)
pub(crate) const RUNES_COLOR: &str = "#ff6b6b"; // Runes (coral red)
pub(crate) const OMNI_COLOR: &str = "#3b82f6"; // Omni Layer (blue)
pub(crate) const COUNTERPARTY_COLOR: &str = "#f59e0b"; // Counterparty (amber)
pub(crate) const CARRIER_COLOR: &str = "#bb8fff"; // Data carriers / other (purple)
pub(crate) const SIGNAL_YES: &str = "#2ecc71"; // Signaled (green)

// Address type colors
pub(crate) const P2PK_COLOR: &str = "#94a3b8";  // Slate gray (ancient/rare)
pub(crate) const P2PKH_COLOR: &str = "#ef4444"; // Red (legacy dominant)
pub(crate) const P2SH_COLOR: &str = "#f59e0b";  // Amber (multisig era)
pub(crate) const P2WPKH_COLOR: &str = "#3b82f6"; // Blue (SegWit v0)
pub(crate) const P2WSH_COLOR: &str = "#8b5cf6";  // Purple (SegWit v0 multisig)
pub(crate) const P2TR_COLOR: &str = "#22c55e";   // Green (Taproot)
pub(crate) const RBF_COLOR: &str = "#06b6d4";    // Cyan

pub(crate) const SUBSIDY_COLOR: &str = "#9b59b6";
pub(crate) const DISK_COLOR: &str = "#e74c3c"; // Red for disk size
pub(crate) const TAPROOT_COLOR: &str = "#f7931a";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn chart_defaults() -> serde_json::Value {
    json!({
        "backgroundColor": "transparent",
        "textStyle": { "color": "#aaa", "fontFamily": "Inter, system-ui, sans-serif" },
        "grid": { "left": 55, "right": 20, "top": 45, "bottom": 65 },
        "legend": { "textStyle": { "color": "#ccc", "fontSize": 11 }, "top": 5, "left": 50, "right": 80, "type": "scroll" },
        "toolbox": {
            "feature": {
                "restore": { "title": "Reset zoom" },
                "dataZoom": { "title": { "zoom": "Zoom", "back": "Undo zoom" } },
                "saveAsImage": { "title": "Save" }
            },
            "iconStyle": { "borderColor": "#aaa" },
            "emphasis": { "iconStyle": { "borderColor": "#f7931a" } },
            "right": 10, "top": 0,
            "itemSize": 14
        },
        "animation": true,
        "animationDuration": 300,
        "progressive": 500,
        "progressiveThreshold": 3000
    })
}

pub(crate) fn data_zoom() -> serde_json::Value {
    json!([
        { "type": "inside", "start": 0, "end": 100 },
        {
            "type": "slider", "start": 0, "end": 100, "height": 20, "bottom": 8,
            "borderColor": "#333", "fillerColor": "rgba(247,147,26,0.15)",
            "handleStyle": { "color": "#f7931a" }, "textStyle": { "color": "#aaa", "fontSize": 10 }
        }
    ])
}

pub(crate) fn tooltip_axis() -> serde_json::Value {
    json!({ "trigger": "axis" })
}

pub(crate) fn x_axis_for(is_daily: bool, categories: &[String]) -> serde_json::Value {
    if is_daily {
        json!({
            "type": "category",
            "data": categories,
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } }
        })
    } else {
        json!({
            "type": "time",
            "axisLabel": { "color": "#aaa", "hideOverlap": true },
            "axisLine": { "lineStyle": { "color": "#555" } }
        })
    }
}

pub(crate) fn y_axis(name: &str) -> serde_json::Value {
    json!({
        "type": "value",
        "name": name,
        "nameTextStyle": { "color": "#aaa" },
        "axisLabel": { "color": "#aaa" },
        "axisLine": { "lineStyle": { "color": "#555" } },
        "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
    })
}

pub(crate) fn no_data_chart(title: &str) -> String {
    let mut opt = chart_defaults();
    let m = opt.as_object_mut().unwrap();
    m.insert(
        "title".into(),
        json!({
            "text": format!("{} — No data", title),
            "textStyle": { "color": "#aaa", "fontSize": 14 },
            "left": "center", "top": "middle"
        }),
    );
    serde_json::to_string(&opt).unwrap_or_default()
}

pub(crate) fn moving_average(data: &[f64], window: usize) -> Vec<Option<f64>> {
    let mut result = Vec::with_capacity(data.len());
    for i in 0..data.len() {
        if i < window.saturating_sub(1) {
            result.push(None);
        } else {
            let start = i + 1 - window;
            let sum: f64 = data[start..=i].iter().sum();
            let avg = sum / window as f64;
            result.push(Some((avg * 1000.0).round() / 1000.0));
        }
    }
    result
}

pub(crate) fn ts_ms(unix_secs: u64) -> u64 {
    unix_secs * 1000
}

/// Round to N decimal places.
pub(crate) fn round(val: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (val * factor).round() / factor
}

/// Whether to show moving average (skip for short ranges like 1D)
pub(crate) fn show_ma(data_len: usize) -> bool {
    data_len >= 200
}

/// Merge chart_defaults with additional fields.
pub(crate) fn build_option(extra: serde_json::Value) -> String {
    let mut base = chart_defaults();
    if let (Some(base_obj), Some(extra_obj)) =
        (base.as_object_mut(), extra.as_object())
    {
        for (k, v) in extra_obj {
            // Deep-merge legend so "show" doesn't wipe out default textStyle/position
            if k == "legend" {
                if let (Some(base_legend), Some(extra_legend)) = (
                    base_obj.get_mut("legend").and_then(|l| l.as_object_mut()),
                    v.as_object(),
                ) {
                    for (lk, lv) in extra_legend {
                        base_legend.insert(lk.clone(), lv.clone());
                    }
                    continue;
                }
            }
            base_obj.insert(k.clone(), v.clone());
        }
    }
    serde_json::to_string(&base).unwrap_or_default()
}

pub(crate) fn format_num(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}

/// Block subsidy in satoshis for a given height.
pub(crate) fn block_subsidy(height: u64) -> u64 {
    let halvings = height / 210_000;
    if halvings >= 64 {
        return 0;
    }
    5_000_000_000u64 >> halvings
}

// ---------------------------------------------------------------------------
// Overlay constants
// ---------------------------------------------------------------------------

/// Halving block heights and approximate timestamps (Unix seconds).
const HALVINGS: &[(u64, u64, &str)] = &[
    (210_000, 1_354_116_278, "Halving #1"),
    (420_000, 1_468_082_773, "Halving #2"),
    (630_000, 1_589_225_023, "Halving #3"),
    (840_000, 1_713_571_767, "Halving #4"),
];

/// Halving dates for daily-mode charts (YYYY-MM-DD).
const HALVING_DATES: &[&str] = &[
    "2012-11-28",
    "2016-07-09",
    "2020-05-11",
    "2024-04-20",
];

/// Notable BIP activation block heights and timestamps.
const BIP_ACTIVATIONS: &[(u64, u64, &str)] = &[
    (227_931, 1_363_636_474, "BIP-16 (P2SH)"),
    (363_725, 1_436_486_408, "BIP-66 (Strict DER)"),
    (388_381, 1_449_187_214, "BIP-65 (CLTV)"),
    (419_328, 1_467_331_589, "BIP-68/112/113 (CSV)"),
    (481_824, 1_503_539_857, "BIP-141 (SegWit)"),
    (709_632, 1_636_839_505, "BIP-341 (Taproot)"),
];

/// BIP activation dates for daily-mode charts (derived from block timestamps above).
const BIP_ACTIVATION_DATES: &[(&str, &str)] = &[
    ("2013-03-18", "BIP-16 (P2SH)"),
    ("2015-07-10", "BIP-66 (Strict DER)"),
    ("2015-12-04", "BIP-65 (CLTV)"),
    ("2016-07-01", "BIP-68/112/113 (CSV)"),
    ("2017-08-24", "BIP-141 (SegWit)"),
    ("2021-11-13", "BIP-341 (Taproot)"),
];

/// Bitcoin Core major release timestamps (Unix seconds) and labels.
const CORE_RELEASES: &[(u64, &str)] = &[
    (1231444060, "v0.1"),
    (1316736000, "v0.4"),
    (1321884081, "v0.5"),
    (1333065600, "v0.6"),
    (1347840000, "v0.7"),
    (1361232000, "v0.8"),
    (1395187200, "v0.9"),
    (1424044800, "v0.10"),
    (1436659200, "v0.11"),
    (1456185600, "v0.12"),
    (1471910400, "v0.13"),
    (1488931200, "v0.14"),
    (1505347200, "v0.15"),
    (1519603200, "v0.16"),
    (1538524800, "v0.17"),
    (1556755200, "v0.18"),
    (1573171200, "v0.19"),
    (1591142400, "v0.20"),
    (1610582400, "v0.21"),
    (1631577600, "v22"),
    (1650844800, "v23"),
    (1670803200, "v24"),
    (1685059200, "v25"),
    (1701820800, "v26"),
    (1712016000, "v27"),
    (1727827200, "v28"),
    (1744588800, "v29"),
    (1760083200, "v30"),
];

/// Bitcoin Core major release dates for daily-mode charts.
const CORE_RELEASE_DATES: &[(&str, &str)] = &[
    ("2009-01-08", "v0.1"),
    ("2011-09-23", "v0.4"),
    ("2011-11-21", "v0.5"),
    ("2012-03-30", "v0.6"),
    ("2012-09-17", "v0.7"),
    ("2013-02-19", "v0.8"),
    ("2014-03-19", "v0.9"),
    ("2015-02-16", "v0.10"),
    ("2015-07-12", "v0.11"),
    ("2016-02-23", "v0.12"),
    ("2016-08-23", "v0.13"),
    ("2017-03-08", "v0.14"),
    ("2017-09-14", "v0.15"),
    ("2018-02-26", "v0.16"),
    ("2018-10-03", "v0.17"),
    ("2019-05-02", "v0.18"),
    ("2019-11-08", "v0.19"),
    ("2020-06-03", "v0.20"),
    ("2021-01-14", "v0.21"),
    ("2021-09-14", "v22"),
    ("2022-04-25", "v23"),
    ("2022-12-12", "v24"),
    ("2023-05-26", "v25"),
    ("2023-12-06", "v26"),
    ("2024-04-02", "v27"),
    ("2024-10-02", "v28"),
    ("2025-04-14", "v29"),
    ("2025-10-10", "v30"),
];

/// Notable Bitcoin events (timestamp unix seconds, label).
const EVENTS: &[(u64, &str)] = &[
    (1392163200, "Mt. Gox Collapse"),
    (1495756800, "SegWit2x (NYA)"),
    (1501545600, "BCH Fork"),
    (1510358400, "SegWit2x Cancelled"),
    (1521072000, "Lightning Mainnet"),
    (1621900800, "China Mining Ban"),
    (1674259200, "Ordinals Launch"),
    (1678838400, "BRC-20 Launch"),
    (1713571767, "Runes Launch"),
];
const EVENT_DATES: &[(&str, &str)] = &[
    ("2014-02-12", "Mt. Gox Collapse"),
    ("2017-05-26", "SegWit2x (NYA)"),
    ("2017-08-01", "BCH Fork"),
    ("2017-11-11", "SegWit2x Cancelled"),
    ("2018-03-15", "Lightning Mainnet"),
    ("2021-05-25", "China Mining Ban"),
    ("2023-01-21", "Ordinals Launch"),
    ("2023-03-15", "BRC-20 Launch"),
    ("2024-04-20", "Runes Launch"),
];

/// Overlay flags — which overlays to merge into a chart option.
#[derive(Clone, Debug, Default)]
pub struct OverlayFlags {
    pub halvings: bool,
    pub bip_activations: bool,
    pub core_releases: bool,
    pub events: bool,
    /// Price overlay data (timestamp_ms, price_usd). Empty vec = disabled.
    pub price_data: Vec<(u64, f64)>,
    /// Chain size overlay data (timestamp_ms, cumulative_gb). Empty vec = disabled.
    pub chain_size_data: Vec<(u64, f64)>,
}

/// Merge overlay markLines and series into an already-built chart option JSON string.
/// Works for both time-axis (per-block) and category-axis (daily) charts.
pub fn apply_overlays(option_json: &str, overlays: &OverlayFlags, is_daily: bool) -> String {
    let has_any = overlays.halvings
        || overlays.bip_activations
        || overlays.core_releases
        || overlays.events
        || !overlays.price_data.is_empty()
        || !overlays.chain_size_data.is_empty();

    if !has_any {
        return option_json.to_string();
    }

    let mut opt: serde_json::Value = match serde_json::from_str(option_json) {
        Ok(v) => v,
        Err(_) => return option_json.to_string(),
    };

    let obj = match opt.as_object_mut() {
        Some(o) => o,
        None => return option_json.to_string(),
    };

    // Widen grid.right when we have price axis or markLine labels
    let need_right_space = !overlays.price_data.is_empty()
        || overlays.halvings
        || overlays.bip_activations
        || overlays.core_releases
        || overlays.events;
    if need_right_space {
        if let Some(grid) = obj.get_mut("grid") {
            if let Some(g) = grid.as_object_mut() {
                let right = if !overlays.price_data.is_empty() { 70 } else { 60 };
                g.insert("right".into(), json!(right));
            }
        }
    }

    // Track whether any right-side axes will be added (for toolbox repositioning later)
    let has_right_axis = !overlays.price_data.is_empty() || !overlays.chain_size_data.is_empty();

    // --- Mark lines (halvings, BIP activations) ---
    let mut mark_lines: Vec<serde_json::Value> = Vec::new();

    if overlays.halvings {
        if is_daily {
            for &date in HALVING_DATES {
                mark_lines.push(json!({
                    "xAxis": date,
                    "lineStyle": { "color": "#f7931a", "type": "dashed", "width": 1.5 },
                    "label": {
                        "show": true, "formatter": "½", "color": "#f7931a",
                        "fontSize": 13, "fontWeight": "bold", "position": "insideEndTop",
                        "backgroundColor": "rgba(10,25,41,0.8)", "padding": [2, 4], "borderRadius": 2
                    }
                }));
            }
        } else {
            for &(_, ts, _label) in HALVINGS {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#f7931a", "type": "dashed", "width": 1.5 },
                    "label": {
                        "show": true, "formatter": "½", "color": "#f7931a",
                        "fontSize": 13, "fontWeight": "bold", "position": "insideEndTop",
                        "backgroundColor": "rgba(10,25,41,0.8)", "padding": [2, 4], "borderRadius": 2
                    }
                }));
            }
        }
    }

    if overlays.bip_activations {
        if is_daily {
            for &(date, name) in BIP_ACTIVATION_DATES {
                mark_lines.push(json!({
                    "xAxis": date,
                    "lineStyle": { "color": "#4ecdc4", "type": "dotted", "width": 1 },
                    "label": {
                        "show": true, "formatter": name, "color": "#4ecdc4",
                        "fontSize": 11, "position": "insideEndTop", "rotate": 90,
                        "backgroundColor": "rgba(10,25,41,0.8)", "padding": [2, 4], "borderRadius": 2
                    }
                }));
            }
        } else {
            for &(_, ts, name) in BIP_ACTIVATIONS {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#4ecdc4", "type": "dotted", "width": 1 },
                    "label": {
                        "show": true, "formatter": name, "color": "#4ecdc4",
                        "fontSize": 11, "position": "insideEndTop", "rotate": 90,
                        "backgroundColor": "rgba(10,25,41,0.8)", "padding": [2, 4], "borderRadius": 2
                    }
                }));
            }
        }
    }

    if overlays.core_releases {
        if is_daily {
            for &(date, name) in CORE_RELEASE_DATES {
                mark_lines.push(json!({
                    "xAxis": date,
                    "lineStyle": { "color": "#a855f7", "type": "dotted", "width": 1 },
                    "label": {
                        "show": true, "formatter": name, "color": "#a855f7",
                        "fontSize": 10, "position": "insideEndTop", "rotate": 90,
                        "backgroundColor": "rgba(10,25,41,0.8)", "padding": [2, 3], "borderRadius": 2
                    }
                }));
            }
        } else {
            for &(ts, name) in CORE_RELEASES {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#a855f7", "type": "dotted", "width": 1 },
                    "label": {
                        "show": true, "formatter": name, "color": "#a855f7",
                        "fontSize": 10, "position": "insideEndTop", "rotate": 90,
                        "backgroundColor": "rgba(10,25,41,0.8)", "padding": [2, 3], "borderRadius": 2
                    }
                }));
            }
        }
    }

    if overlays.events {
        if is_daily {
            for &(date, name) in EVENT_DATES {
                mark_lines.push(json!({
                    "xAxis": date,
                    "lineStyle": { "color": "#ef4444", "type": "solid", "width": 2 },
                    "label": {
                        "show": true, "formatter": name, "color": "#ef4444",
                        "fontSize": 11, "fontWeight": "bold", "position": "insideEndTop", "rotate": 90,
                        "backgroundColor": "rgba(10,25,41,0.85)", "padding": [3, 5], "borderRadius": 3
                    }
                }));
            }
        } else {
            for &(ts, name) in EVENTS {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#ef4444", "type": "solid", "width": 2 },
                    "label": {
                        "show": true, "formatter": name, "color": "#ef4444",
                        "fontSize": 11, "fontWeight": "bold", "position": "insideEndTop", "rotate": 90,
                        "backgroundColor": "rgba(10,25,41,0.85)", "padding": [3, 5], "borderRadius": 3
                    }
                }));
            }
        }
    }

    // Attach markLines to the first series
    if !mark_lines.is_empty() {
        if let Some(series) = obj.get_mut("series") {
            if let Some(arr) = series.as_array_mut() {
                if let Some(first) = arr.first_mut() {
                    if let Some(s) = first.as_object_mut() {
                        s.insert(
                            "markLine".into(),
                            json!({
                                "silent": true,
                                "symbol": "none",
                                "data": mark_lines
                            }),
                        );
                    }
                }
            }
        }
    }

    // --- Price overlay (secondary Y-axis + line series) ---
    if !overlays.price_data.is_empty() {
        // Determine the chart's visible time range from existing data
        let (chart_min_ms, chart_max_ms) = if is_daily {
            let cats = obj
                .get("xAxis")
                .and_then(|x| x.get("data"))
                .and_then(|d| d.as_array());
            if let Some(cats) = cats {
                let parse_date = |s: &str| -> u64 {
                    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map(|d| {
                            d.and_hms_opt(0, 0, 0)
                                .unwrap()
                                .and_utc()
                                .timestamp() as u64
                                * 1000
                        })
                        .unwrap_or(0)
                };
                let first = cats.first().and_then(|v| v.as_str()).unwrap_or("");
                let last = cats.last().and_then(|v| v.as_str()).unwrap_or("");
                (parse_date(first), parse_date(last))
            } else {
                (0, u64::MAX)
            }
        } else {
            // For time-axis charts, scan first series data for min/max timestamps
            let mut min_ts = u64::MAX;
            let mut max_ts = 0u64;
            if let Some(series) = obj.get("series") {
                if let Some(first_s) = series.as_array().and_then(|a| a.first()) {
                    if let Some(data) = first_s.get("data").and_then(|d| d.as_array()) {
                        for pt in data {
                            if let Some(arr) = pt.as_array() {
                                // Handle both u64 and f64 number representations
                                let ts = arr.first().and_then(|v| {
                                    v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))
                                });
                                if let Some(ts) = ts {
                                    min_ts = min_ts.min(ts);
                                    max_ts = max_ts.max(ts);
                                }
                            }
                        }
                    }
                }
            }
            if min_ts == u64::MAX { min_ts = 0; }
            (min_ts, max_ts)
        };

        // Add padding (1 day each side) and filter price data to visible range
        let range_min = chart_min_ms.saturating_sub(86_400_000);
        let range_max = chart_max_ms.saturating_add(86_400_000);
        let filtered_prices: Vec<(u64, f64)> = overlays
            .price_data
            .iter()
            .filter(|&&(ts_ms, _)| ts_ms >= range_min && ts_ms <= range_max)
            .copied()
            .collect();

        // Only add overlay if we have price data in range
        if filtered_prices.is_empty() {
            return serde_json::to_string(&opt).unwrap_or_default();
        }

        // Convert existing yAxis to array if it's a single object
        let y_axis = obj.remove("yAxis");
        let mut y_axes = match y_axis {
            Some(serde_json::Value::Array(arr)) => arr,
            Some(obj_val) => vec![obj_val],
            None => vec![json!({ "type": "value" })],
        };

        // Add price Y-axis on the right
        let price_axis_idx = y_axes.len();
        y_axes.push(json!({
            "type": "value",
            "name": "USD",
            "nameTextStyle": { "color": "#e6c84e" },
            "position": "right",
            "axisLabel": { "color": "#e6c84e", "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#e6c84e" } },
            "splitLine": { "show": false }
        }));

        obj.insert("yAxis".into(), json!(y_axes));

        // Ensure existing series explicitly reference yAxisIndex: 0
        if let Some(series) = obj.get_mut("series") {
            if let Some(arr) = series.as_array_mut() {
                for s in arr.iter_mut() {
                    if let Some(s_obj) = s.as_object_mut() {
                        s_obj.entry("yAxisIndex").or_insert(json!(0));
                    }
                }
            }
        }

        // Build price series data using interpolation for smooth coverage.
        // Price data is daily but chart categories may not align exactly.
        let price_series_data: Vec<serde_json::Value> = if is_daily {
            // For daily/category charts: interpolate between price data points.
            // Convert each category date to a timestamp, then interpolate.
            let categories = obj
                .get("xAxis")
                .and_then(|x| x.get("data"))
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();

            // Price data is already sorted by timestamp (blockchain.info returns chronological)
            categories
                .iter()
                .map(|cat| {
                    let date_str = cat.as_str().unwrap_or_default();
                    let cat_ms = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                        .map(|d| {
                            d.and_hms_opt(12, 0, 0) // noon UTC for better matching
                                .unwrap()
                                .and_utc()
                                .timestamp() as u64
                                * 1000
                        })
                        .unwrap_or(0);

                    if cat_ms == 0 {
                        return json!(null);
                    }

                    // Binary search for surrounding price points and interpolate
                    match filtered_prices.binary_search_by_key(&cat_ms, |&(ts, _)| ts) {
                        Ok(idx) => json!(filtered_prices[idx].1),
                        Err(idx) => {
                            if idx == 0 {
                                // Before first price point
                                json!(null)
                            } else if idx >= filtered_prices.len() {
                                // After last price point — use last known
                                json!(filtered_prices.last().unwrap().1)
                            } else {
                                // Interpolate between surrounding points
                                let (t0, p0) = filtered_prices[idx - 1];
                                let (t1, p1) = filtered_prices[idx];
                                if t1 == t0 {
                                    json!(p0)
                                } else {
                                    let frac = (cat_ms - t0) as f64 / (t1 - t0) as f64;
                                    let interpolated = p0 + frac * (p1 - p0);
                                    json!((interpolated * 100.0).round() / 100.0)
                                }
                            }
                        }
                    }
                })
                .collect()
        } else {
            // For time-axis charts, use [ts_ms, price] pairs (already filtered)
            filtered_prices
                .iter()
                .map(|&(ts_ms, price)| json!([ts_ms, price]))
                .collect()
        };

        // Add price line series on the secondary y-axis
        let price_series = json!({
            "name": "Price (USD)",
            "type": "line",
            "yAxisIndex": price_axis_idx,
            "data": price_series_data,
            "connectNulls": true,
            "lineStyle": { "color": "#e6c84e", "width": 1.5, "opacity": 0.8 },
            "itemStyle": { "color": "#e6c84e" },
            "symbol": "none",
            "smooth": true,
            "z": 1
        });

        if let Some(series) = obj.get_mut("series") {
            if let Some(arr) = series.as_array_mut() {
                arr.push(price_series);
            }
        }

        // Add Price to legend
        if let Some(legend) = obj.get_mut("legend") {
            if let Some(l) = legend.as_object_mut() {
                l.insert("show".into(), json!(true));
            }
        }

    }

    // --- Chain size overlay (secondary Y-axis + line series) ---
    if !overlays.chain_size_data.is_empty() {
        // Determine visible range (reuse same logic as price)
        let (cs_min_ms, cs_max_ms) = if is_daily {
            let cats = obj
                .get("xAxis")
                .and_then(|x| x.get("data"))
                .and_then(|d| d.as_array());
            if let Some(cats) = cats {
                let parse_date = |s: &str| -> u64 {
                    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map(|d| {
                            d.and_hms_opt(12, 0, 0)
                                .unwrap()
                                .and_utc()
                                .timestamp() as u64
                                * 1000
                        })
                        .unwrap_or(0)
                };
                let first = cats.first().and_then(|v| v.as_str()).unwrap_or("");
                let last = cats.last().and_then(|v| v.as_str()).unwrap_or("");
                (parse_date(first), parse_date(last))
            } else {
                (0, u64::MAX)
            }
        } else {
            let mut min_ts = u64::MAX;
            let mut max_ts = 0u64;
            if let Some(series) = obj.get("series") {
                if let Some(first_s) = series.as_array().and_then(|a| a.first()) {
                    if let Some(data) = first_s.get("data").and_then(|d| d.as_array()) {
                        for pt in data {
                            if let Some(arr) = pt.as_array() {
                                let ts = arr.first().and_then(|v| {
                                    v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))
                                });
                                if let Some(ts) = ts {
                                    min_ts = min_ts.min(ts);
                                    max_ts = max_ts.max(ts);
                                }
                            }
                        }
                    }
                }
            }
            if min_ts == u64::MAX { min_ts = 0; }
            (min_ts, max_ts)
        };

        let cs_range_min = cs_min_ms.saturating_sub(86_400_000);
        let cs_range_max = cs_max_ms.saturating_add(86_400_000);
        let filtered_cs: Vec<(u64, f64)> = overlays
            .chain_size_data
            .iter()
            .filter(|&&(ts_ms, _)| ts_ms >= cs_range_min && ts_ms <= cs_range_max)
            .copied()
            .collect();

        if !filtered_cs.is_empty() {
            // Add or extend yAxis array
            let y_axis_val = obj.remove("yAxis");
            let mut y_axes = match y_axis_val {
                Some(serde_json::Value::Array(arr)) => arr,
                Some(obj_val) => vec![obj_val],
                None => vec![json!({ "type": "value" })],
            };

            let cs_axis_idx = y_axes.len();
            y_axes.push(json!({
                "type": "value",
                "name": "GB",
                "nameTextStyle": { "color": "#10b981" },
                "position": "right",
                "offset": if y_axes.len() > 1 { 60 } else { 0 },
                "axisLabel": { "color": "#10b981", "fontSize": 10 },
                "axisLine": { "lineStyle": { "color": "#10b981" } },
                "splitLine": { "show": false }
            }));

            obj.insert("yAxis".into(), json!(y_axes));

            // Ensure existing series have explicit yAxisIndex
            if let Some(series) = obj.get_mut("series") {
                if let Some(arr) = series.as_array_mut() {
                    for s in arr.iter_mut() {
                        if let Some(s_obj) = s.as_object_mut() {
                            s_obj.entry("yAxisIndex").or_insert(json!(0));
                        }
                    }
                }
            }

            // Widen grid for the extra axis
            if let Some(grid) = obj.get_mut("grid") {
                if let Some(g) = grid.as_object_mut() {
                    let current = g.get("right").and_then(|v| v.as_u64()).unwrap_or(20);
                    g.insert("right".into(), json!(current.max(70) + if cs_axis_idx > 1 { 60 } else { 0 }));
                }
            }

            let cs_series_data: Vec<serde_json::Value> = if is_daily {
                let categories = obj
                    .get("xAxis")
                    .and_then(|x| x.get("data"))
                    .and_then(|d| d.as_array())
                    .cloned()
                    .unwrap_or_default();

                categories
                    .iter()
                    .map(|cat| {
                        let date_str = cat.as_str().unwrap_or_default();
                        let cat_ms = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                            .map(|d| {
                                d.and_hms_opt(12, 0, 0)
                                    .unwrap()
                                    .and_utc()
                                    .timestamp() as u64
                                    * 1000
                            })
                            .unwrap_or(0);

                        if cat_ms == 0 {
                            return json!(null);
                        }

                        match filtered_cs.binary_search_by_key(&cat_ms, |&(ts, _)| ts) {
                            Ok(idx) => json!(filtered_cs[idx].1),
                            Err(idx) => {
                                if idx == 0 {
                                    json!(null)
                                } else if idx >= filtered_cs.len() {
                                    json!(filtered_cs.last().unwrap().1)
                                } else {
                                    let (t0, v0) = filtered_cs[idx - 1];
                                    let (t1, v1) = filtered_cs[idx];
                                    if t1 == t0 {
                                        json!(v0)
                                    } else {
                                        let frac = (cat_ms - t0) as f64 / (t1 - t0) as f64;
                                        json!((( v0 + frac * (v1 - v0)) * 100.0).round() / 100.0)
                                    }
                                }
                            }
                        }
                    })
                    .collect()
            } else {
                filtered_cs
                    .iter()
                    .map(|&(ts_ms, gb)| json!([ts_ms, gb]))
                    .collect()
            };

            let cs_series = json!({
                "name": "Chain Size (GB)",
                "type": "line",
                "yAxisIndex": cs_axis_idx,
                "data": cs_series_data,
                "connectNulls": true,
                "lineStyle": { "color": "#10b981", "width": 1.5, "opacity": 0.8 },
                "itemStyle": { "color": "#10b981" },
                "symbol": "none",
                "smooth": true,
                "z": 1
            });

            if let Some(series) = obj.get_mut("series") {
                if let Some(arr) = series.as_array_mut() {
                    arr.push(cs_series);
                }
            }

            if let Some(legend) = obj.get_mut("legend") {
                if let Some(l) = legend.as_object_mut() {
                    l.insert("show".into(), json!(true));
                }
            }
        }
    }

    // After all overlays applied: reposition toolbox clear of any right-side axes
    if has_right_axis {
        let grid_right = obj
            .get("grid")
            .and_then(|g| g.get("right"))
            .and_then(|v| v.as_u64())
            .unwrap_or(20);
        if let Some(grid) = obj.get_mut("grid") {
            if let Some(g) = grid.as_object_mut() {
                g.insert("top".into(), json!(45));
            }
        }
        if let Some(toolbox) = obj.get_mut("toolbox") {
            if let Some(t) = toolbox.as_object_mut() {
                // Place toolbox just inside the grid area, past all right-axis labels
                t.insert("right".into(), json!(grid_right + 25));
            }
        }
    }

    serde_json::to_string(&opt).unwrap_or_default()
}
