//! ECharts option JSON builders.
//! Runs on the client (WASM) — takes typed data and produces JSON strings
//! that are passed to ECharts via JS interop.

use serde_json::json;

use super::types::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Consistent chart color palette
const DATA_COLOR: &str = "#f7931a"; // Primary data (bitcoin orange)
const DATA_COLOR_FADED: &str = "rgba(247,147,26,0.15)"; // Primary data area fill
const MA_COLOR: &str = "rgba(255,255,255,0.85)"; // Moving average (white)
const TARGET_COLOR: &str = "#e74c3c"; // Target/reference lines (red)
const RUNES_COLOR: &str = "#ff6b6b"; // Runes (coral red)
const OMNI_COLOR: &str = "#3b82f6"; // Omni Layer (blue)
const COUNTERPARTY_COLOR: &str = "#f59e0b"; // Counterparty (amber)
const CARRIER_COLOR: &str = "#bb8fff"; // Data carriers / other (purple)
const SIGNAL_YES: &str = "#2ecc71"; // Signaled (green)

// Address type colors
const P2PK_COLOR: &str = "#94a3b8";  // Slate gray (ancient/rare)
const P2PKH_COLOR: &str = "#ef4444"; // Red (legacy dominant)
const P2SH_COLOR: &str = "#f59e0b";  // Amber (multisig era)
const P2WPKH_COLOR: &str = "#3b82f6"; // Blue (SegWit v0)
const P2WSH_COLOR: &str = "#8b5cf6";  // Purple (SegWit v0 multisig)
const P2TR_COLOR: &str = "#22c55e";   // Green (Taproot)
const RBF_COLOR: &str = "#06b6d4";    // Cyan

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn chart_defaults() -> serde_json::Value {
    json!({
        "backgroundColor": "transparent",
        "textStyle": { "color": "#aaa", "fontFamily": "Inter, system-ui, sans-serif" },
        "grid": { "left": 55, "right": 20, "top": 35, "bottom": 65 },
        "legend": { "textStyle": { "color": "#ccc", "fontSize": 11 }, "top": 8, "left": "center" },
        "toolbox": {
            "feature": {
                "restore": { "title": "Reset zoom" },
                "dataZoom": { "title": { "zoom": "Zoom", "back": "Undo zoom" } },
                "saveAsImage": { "title": "Save" }
            },
            "iconStyle": { "borderColor": "#aaa" },
            "emphasis": { "iconStyle": { "borderColor": "#f7931a" } },
            "right": 10, "top": 0
        },
        "animation": true,
        "animationDuration": 300,
        "progressive": 500,
        "progressiveThreshold": 3000
    })
}

fn data_zoom() -> serde_json::Value {
    json!([
        { "type": "inside", "start": 0, "end": 100 },
        {
            "type": "slider", "start": 0, "end": 100, "height": 20, "bottom": 8,
            "borderColor": "#333", "fillerColor": "rgba(247,147,26,0.15)",
            "handleStyle": { "color": "#f7931a" }, "textStyle": { "color": "#aaa", "fontSize": 10 }
        }
    ])
}

fn tooltip_axis() -> serde_json::Value {
    json!({ "trigger": "axis" })
}

fn x_axis_for(is_daily: bool, categories: &[String]) -> serde_json::Value {
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

fn y_axis(name: &str) -> serde_json::Value {
    json!({
        "type": "value",
        "name": name,
        "nameTextStyle": { "color": "#aaa" },
        "axisLabel": { "color": "#aaa" },
        "axisLine": { "lineStyle": { "color": "#555" } },
        "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
    })
}

fn no_data_chart(title: &str) -> String {
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

fn moving_average(data: &[f64], window: usize) -> Vec<Option<f64>> {
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

fn ts_ms(unix_secs: u64) -> u64 {
    unix_secs * 1000
}

/// Whether to show moving average (skip for short ranges like 1D)
fn show_ma(data_len: usize) -> bool {
    data_len >= 200
}

/// Merge chart_defaults with additional fields.
fn build_option(extra: serde_json::Value) -> String {
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

    // Move toolbox to the left when price axis takes the right side
    if !overlays.price_data.is_empty() {
        if let Some(toolbox) = obj.get_mut("toolbox") {
            if let Some(t) = toolbox.as_object_mut() {
                t.remove("right");
                t.insert("left".into(), json!(55));
            }
        }
    }

    // --- Mark lines (halvings, BIP activations) ---
    let mut mark_lines: Vec<serde_json::Value> = Vec::new();

    if overlays.halvings {
        if is_daily {
            for &date in HALVING_DATES {
                mark_lines.push(json!({
                    "xAxis": date,
                    "lineStyle": { "color": "#f7931a", "type": "dashed", "width": 1.5 },
                    "label": { "show": true, "formatter": "½", "color": "#f7931a", "fontSize": 10, "position": "insideEndTop" }
                }));
            }
        } else {
            for &(_, ts, _label) in HALVINGS {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#f7931a", "type": "dashed", "width": 1.5 },
                    "label": { "show": true, "formatter": "½", "color": "#f7931a", "fontSize": 10, "position": "insideEndTop" }
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
                    "label": { "show": true, "formatter": name, "color": "#4ecdc4", "fontSize": 9, "position": "insideEndTop", "rotate": 90 }
                }));
            }
        } else {
            for &(_, ts, name) in BIP_ACTIVATIONS {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#4ecdc4", "type": "dotted", "width": 1 },
                    "label": { "show": true, "formatter": name, "color": "#4ecdc4", "fontSize": 9, "position": "insideEndTop", "rotate": 90 }
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
                    "label": { "show": true, "formatter": name, "color": "#a855f7", "fontSize": 8, "position": "insideEndTop", "rotate": 90 }
                }));
            }
        } else {
            for &(ts, name) in CORE_RELEASES {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#a855f7", "type": "dotted", "width": 1 },
                    "label": { "show": true, "formatter": name, "color": "#a855f7", "fontSize": 8, "position": "insideEndTop", "rotate": 90 }
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
                    "label": { "show": true, "formatter": name, "color": "#ef4444", "fontSize": 9, "position": "insideEndTop", "rotate": 90 }
                }));
            }
        } else {
            for &(ts, name) in EVENTS {
                mark_lines.push(json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": "#ef4444", "type": "solid", "width": 2 },
                    "label": { "show": true, "formatter": name, "color": "#ef4444", "fontSize": 9, "position": "insideEndTop", "rotate": 90 }
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

    serde_json::to_string(&opt).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Chart builders
// ---------------------------------------------------------------------------

/// Block size line chart with moving average.
pub fn block_size_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Block Size");
    }

    let raw_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.size as f64 / 1_000_000.0]))
        .collect();
    let vals: Vec<f64> =
        blocks.iter().map(|b| b.size as f64 / 1_000_000.0).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let x_axis = x_axis_for(false, &[]);
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Size", "type": "line", "sampling": "lttb", "data": raw_data,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis,
        "yAxis": y_axis("MB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block size chart from daily aggregates.
pub fn block_size_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Block Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let sizes: Vec<f64> =
        days.iter().map(|d| d.avg_size / 1_000_000.0).collect();
    let ma = moving_average(&sizes, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("MB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Size", "type": "line", "sampling": "lttb", "data": sizes,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Transaction count line chart with moving average.
pub fn tx_count_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Transaction Count");
    }

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.tx_count]))
        .collect();
    let vals: Vec<f64> = blocks.iter().map(|b| b.tx_count as f64).collect();
    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Tx Count", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Transaction count from daily aggregates.
pub fn tx_count_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Transaction Count");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| d.avg_tx_count).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Tx Count", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Fees line chart (per-block: total fees in sats).
pub fn fees_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Fees");
    }

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            if b.total_fees > 0 {
                json!([ts_ms(b.timestamp), b.total_fees])
            } else {
                json!([ts_ms(b.timestamp), null])
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("sats"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Fees", "type": "line", "sampling": "lttb", "data": raw,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Fees from daily aggregates.
pub fn fees_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Fees");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.total_fees > 0 && d.block_count > 0 {
                json!(d.total_fees as f64 / d.block_count as f64)
            } else {
                json!(null)
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("sats"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Fees", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Difficulty line chart.
pub fn difficulty_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Difficulty");
    }

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), b.difficulty / 1e12]))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("T"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Difficulty", "type": "line", "sampling": "lttb", "data": raw,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Difficulty from daily aggregates.
pub fn difficulty_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Difficulty");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| d.avg_difficulty / 1e12).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("T"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Difficulty", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Block interval dot plot (per-block only, not daily).
/// Data format: [timestamp_ms, interval_minutes, block_height] — third value enables click-to-detail.
pub fn block_interval_chart(blocks: &[BlockSummary]) -> String {
    if blocks.len() < 2 {
        return no_data_chart("Block Interval");
    }

    let mut dots: Vec<serde_json::Value> = Vec::with_capacity(blocks.len() - 1);
    let mut interval_vals: Vec<f64> = Vec::with_capacity(blocks.len() - 1);

    for i in 1..blocks.len() {
        let mins = ((blocks[i].timestamp as f64
            - blocks[i - 1].timestamp as f64)
            / 60.0
            * 100.0)
            .round()
            / 100.0;
        dots.push(json!([ts_ms(blocks[i].timestamp), mins, blocks[i].height]));
        interval_vals.push(mins);
    }

    let ma = moving_average(&interval_vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks[1..]
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let has_ma = show_ma(interval_vals.len());

    let mut series = vec![json!({
        "name": "Interval", "type": "scatter", "data": dots,
        "symbolSize": 5,
        "itemStyle": {
            "color": DATA_COLOR
        }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }
    series.push(json!({
        "name": "Target", "type": "line",
        "markLine": {
            "silent": true, "symbol": "none",
            "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 1 },
            "data": [{ "yAxis": 10, "label": { "formatter": "10 min", "color": TARGET_COLOR } }]
        },
        "data": []
    }));

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("min"),
        "dataZoom": data_zoom(),
        "tooltip": { "trigger": "item" },
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block interval from daily aggregates (avg minutes per block = 1440 / blocks_per_day).
pub fn block_interval_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Block Interval (daily)");
    }

    // Filter out partial days (< 50 blocks = likely start/end of range, not a full day)
    let full_days: Vec<&DailyAggregate> =
        days.iter().filter(|d| d.block_count >= 50).collect();
    if full_days.is_empty() {
        return no_data_chart("Block Interval (daily)");
    }
    let dates: Vec<String> = full_days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = full_days
        .iter()
        .map(|d| ((1440.0 / d.block_count as f64) * 100.0).round() / 100.0)
        .collect();
    let ma = moving_average(&vals, 7);

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("min"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Interval", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            },
            {
                "name": "Target", "type": "line",
                "markLine": {
                    "silent": true, "symbol": "none",
                    "lineStyle": { "type": "dashed", "color": TARGET_COLOR, "width": 1 },
                    "data": [{ "yAxis": 10, "label": { "formatter": "10 min", "color": TARGET_COLOR } }]
                },
                "data": []
            }
        ]
    }))
}

/// OP_RETURN count bar chart (runes vs data carriers).
pub fn op_return_count_chart(blocks: &[OpReturnBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Count");
    }

    let runes: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.runes_count])).collect();
    let omni: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.omni_count])).collect();
    let xcp: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.counterparty_count])).collect();
    let other: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.data_carrier_count])).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// OP_RETURN bytes bar chart.
pub fn op_return_bytes_chart(blocks: &[OpReturnBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Volume");
    }

    let runes: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.runes_bytes])).collect();
    let omni: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.omni_bytes])).collect();
    let xcp: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.counterparty_bytes])).collect();
    let other: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.data_carrier_bytes])).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// Protocol dominance — 100% stacked area showing share of each protocol.
pub fn runes_pct_chart(blocks: &[OpReturnBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Protocol Dominance");
    }

    let pct = |count: u64, total: u64| -> f64 {
        if total > 0 { (count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    };

    let runes_data: Vec<serde_json::Value> = blocks.iter().map(|b| {
        let total = b.op_return_count;
        json!([ts_ms(b.timestamp), pct(b.runes_count, total)])
    }).collect();
    let omni_data: Vec<serde_json::Value> = blocks.iter().map(|b| {
        let total = b.op_return_count;
        json!([ts_ms(b.timestamp), pct(b.omni_count, total)])
    }).collect();
    let xcp_data: Vec<serde_json::Value> = blocks.iter().map(|b| {
        let total = b.op_return_count;
        json!([ts_ms(b.timestamp), pct(b.counterparty_count, total)])
    }).collect();
    let other_data: Vec<serde_json::Value> = blocks.iter().map(|b| {
        let total = b.op_return_count;
        json!([ts_ms(b.timestamp), pct(b.data_carrier_count, total)])
    }).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Runes", "type": "line", "sampling": "lttb", "data": runes_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": RUNES_COLOR }, "itemStyle": { "color": RUNES_COLOR }, "symbol": "none" },
            { "name": "Omni", "type": "line", "sampling": "lttb", "data": omni_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": OMNI_COLOR }, "itemStyle": { "color": OMNI_COLOR }, "symbol": "none" },
            { "name": "Counterparty", "type": "line", "sampling": "lttb", "data": xcp_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": COUNTERPARTY_COLOR }, "itemStyle": { "color": COUNTERPARTY_COLOR }, "symbol": "none" },
            { "name": "Other", "type": "line", "sampling": "lttb", "data": other_data, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": CARRIER_COLOR }, "itemStyle": { "color": CARRIER_COLOR }, "symbol": "none" }
        ]
    }))
}

/// OP_RETURN count chart from daily aggregates.
pub fn op_return_count_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Embedded Data Count (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let avg = |total: u64, bc: u64| -> f64 {
        if bc > 0 { (total as f64 / bc as f64 * 1000.0).round() / 1000.0 } else { 0.0 }
    };
    let runes: Vec<f64> = days.iter().map(|d| avg(d.total_runes_count, d.block_count)).collect();
    let omni: Vec<f64> = days.iter().map(|d| avg(d.total_omni_count, d.block_count)).collect();
    let xcp: Vec<f64> = days.iter().map(|d| avg(d.total_counterparty_count, d.block_count)).collect();
    let other: Vec<f64> = days.iter().map(|d| avg(d.total_data_carrier_count, d.block_count)).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// OP_RETURN bytes chart from daily aggregates.
pub fn op_return_bytes_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Embedded Data Volume (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let avg_kb = |total: u64, bc: u64| -> f64 {
        if bc > 0 { ((total as f64 / bc as f64 / 1000.0) * 10.0).round() / 10.0 } else { 0.0 }
    };
    let runes: Vec<f64> = days.iter().map(|d| avg_kb(d.total_runes_bytes, d.block_count)).collect();
    let omni: Vec<f64> = days.iter().map(|d| avg_kb(d.total_omni_bytes, d.block_count)).collect();
    let xcp: Vec<f64> = days.iter().map(|d| avg_kb(d.total_counterparty_bytes, d.block_count)).collect();
    let other: Vec<f64> = days.iter().map(|d| avg_kb(d.total_data_carrier_bytes, d.block_count)).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
        "yAxis": y_axis("KB/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Runes", "type": "bar", "stack": "total", "data": runes, "itemStyle": { "color": RUNES_COLOR } },
            { "name": "Omni", "type": "bar", "stack": "total", "data": omni, "itemStyle": { "color": OMNI_COLOR } },
            { "name": "Counterparty", "type": "bar", "stack": "total", "data": xcp, "itemStyle": { "color": COUNTERPARTY_COLOR } },
            { "name": "Other", "type": "bar", "stack": "total", "data": other, "itemStyle": { "color": CARRIER_COLOR } }
        ]
    }))
}

/// Protocol dominance % from daily aggregates — 100% stacked area.
pub fn runes_pct_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Protocol Dominance (daily)");
    }
    let dates: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let pct = |count: u64, total: u64| -> f64 {
        if total > 0 { (count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    };
    let runes: Vec<f64> = days.iter().map(|d| pct(d.total_runes_count, d.total_op_return_count)).collect();
    let omni: Vec<f64> = days.iter().map(|d| pct(d.total_omni_count, d.total_op_return_count)).collect();
    let xcp: Vec<f64> = days.iter().map(|d| pct(d.total_counterparty_count, d.total_op_return_count)).collect();
    let other: Vec<f64> = days.iter().map(|d| pct(d.total_data_carrier_count, d.total_op_return_count)).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &dates),
        "yAxis": { "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Runes", "type": "line", "sampling": "lttb", "data": runes, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": RUNES_COLOR }, "itemStyle": { "color": RUNES_COLOR }, "symbol": "none" },
            { "name": "Omni", "type": "line", "sampling": "lttb", "data": omni, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": OMNI_COLOR }, "itemStyle": { "color": OMNI_COLOR }, "symbol": "none" },
            { "name": "Counterparty", "type": "line", "sampling": "lttb", "data": xcp, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": COUNTERPARTY_COLOR }, "itemStyle": { "color": COUNTERPARTY_COLOR }, "symbol": "none" },
            { "name": "Other", "type": "line", "sampling": "lttb", "data": other, "stack": "pct", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": CARRIER_COLOR }, "itemStyle": { "color": CARRIER_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Per-block signaling scatter/bar chart.
pub fn signaling_chart(blocks: &[SignalingBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("BIP Signaling");
    }

    let signaled: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| json!([ts_ms(b.timestamp), if b.signaled { 1 } else { 0 }]))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": {
            "type": "value", "name": "Signaled",
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": {
                "color": "#aaa",
                "formatter": "{value}"
            },
            "min": 0, "max": 1, "interval": 1,
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Signaled", "type": "scatter", "data": signaled,
                "itemStyle": { "color": DATA_COLOR },
                "symbolSize": 4
            }
        ]
    }))
}

/// Signaling percentage per retarget period bar chart.
pub fn signaling_periods_chart(
    periods: &[SignalingPeriod],
    threshold: f64,
) -> String {
    if periods.is_empty() {
        return no_data_chart("Signaling Periods");
    }

    let cats: Vec<String> =
        periods.iter().map(|p| format_num(p.start_height)).collect();

    let bar_data: Vec<serde_json::Value> = periods
        .iter()
        .map(|p| {
            let color = if p.signaled_pct >= threshold {
                SIGNAL_YES
            } else if p.signaled_pct > 0.0 {
                TARGET_COLOR
            } else {
                "#333"
            };
            json!({
                "value": (p.signaled_pct * 1000.0).round() / 1000.0,
                "itemStyle": { "color": color }
            })
        })
        .collect();

    build_option(json!({
        "xAxis": {
            "type": "category", "data": cats,
            "axisLabel": { "color": "#aaa", "rotate": 45, "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": {
            "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "grid": { "left": 45, "right": 20, "top": 35, "bottom": 80 },
        "dataZoom": data_zoom(),
        "tooltip": {
            "trigger": "axis",
            "formatter": "{b}<br/>Signaled: {c}%"
        },
        "series": [
            {
                "name": "Signaled %", "type": "bar", "data": bar_data,
                "barMaxWidth": 40,
                "markLine": {
                    "silent": true, "symbol": "none",
                    "lineStyle": { "color": "#f7931a", "type": "dashed", "width": 2 },
                    "data": [{ "yAxis": threshold, "label": { "formatter": format!("{}%", threshold), "color": "#f7931a", "fontSize": 12 } }]
                }
            }
        ]
    }))
}

// ---------------------------------------------------------------------------
// New chart builders
// ---------------------------------------------------------------------------

/// Block subsidy in satoshis for a given height.
fn block_subsidy(height: u64) -> u64 {
    let halvings = height / 210_000;
    if halvings >= 64 {
        return 0;
    }
    5_000_000_000u64 >> halvings
}

/// Block weight utilization as % of max (4,000,000 WU).
pub fn weight_utilization_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Weight Utilization");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            (b.weight as f64 / 4_000_000.0 * 100.0 * 1000.0).round() / 1000.0
        })
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Utilization %", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Block weight utilization from daily aggregates.
pub fn weight_utilization_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Weight Utilization");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| (d.avg_weight / 4_000_000.0 * 100.0 * 1000.0).round() / 1000.0)
        .collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Utilization %", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

const SUBSIDY_COLOR: &str = "#9b59b6";

/// Block subsidy vs fee revenue ratio (stacked area).
pub fn subsidy_vs_fees_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Subsidy vs Fees");
    }

    let subsidy_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let sub = block_subsidy(b.height) as f64 / 100_000_000.0;
            let rounded = (sub * 1000.0).round() / 1000.0;
            json!([ts_ms(b.timestamp), rounded])
        })
        .collect();

    let fee_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            let fee = b.total_fees as f64 / 100_000_000.0;
            let rounded = (fee * 1000.0).round() / 1000.0;
            json!([ts_ms(b.timestamp), rounded])
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Subsidy", "type": "line", "stack": "reward", "data": subsidy_data,
                "lineStyle": { "width": 1, "color": SUBSIDY_COLOR },
                "itemStyle": { "color": SUBSIDY_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(155,89,182,0.3)" }
            },
            {
                "name": "Fees", "type": "line", "stack": "reward", "data": fee_data,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Subsidy vs fees from daily aggregates.
pub fn subsidy_vs_fees_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Subsidy vs Fees");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    // Determine subsidy per day based on known halving dates
    let subsidy_vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.date.as_str() >= "2024-04-20" {
                3.125
            } else if d.date.as_str() >= "2020-05-11" {
                6.25
            } else if d.date.as_str() >= "2016-07-09" {
                12.5
            } else if d.date.as_str() >= "2012-11-28" {
                25.0
            } else {
                50.0
            }
        })
        .collect();
    let fee_vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.total_fees > 0 && d.block_count > 0 {
                let v =
                    d.total_fees as f64 / d.block_count as f64 / 100_000_000.0;
                let rounded = (v * 1000.0).round() / 1000.0;
                json!(rounded)
            } else {
                json!(null)
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("BTC"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Subsidy", "type": "line", "stack": "reward", "data": subsidy_vals,
                "lineStyle": { "width": 1, "color": SUBSIDY_COLOR },
                "itemStyle": { "color": SUBSIDY_COLOR }, "symbol": "none",
                "areaStyle": { "color": "rgba(155,89,182,0.3)" }
            },
            {
                "name": "Avg Fees", "type": "line", "stack": "reward", "data": fee_vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Average transaction size in bytes.
pub fn avg_tx_size_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Avg Transaction Size");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 0 {
                (b.size as f64 / b.tx_count as f64 * 1000.0).round() / 1000.0
            } else {
                0.0
            }
        })
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Avg Tx Size", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Avg transaction size from daily aggregates.
pub fn avg_tx_size_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Avg Transaction Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 0.0 {
                (d.avg_size / d.avg_tx_count * 1000.0).round() / 1000.0
            } else {
                0.0
            }
        })
        .collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("bytes"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Tx Size", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

fn format_num(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    bytes
        .rchunks(3)
        .rev()
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}

/// Fees line chart with unit toggle (sats or BTC).
pub fn fees_chart_unit(blocks: &[BlockSummary], unit: &str) -> String {
    if blocks.is_empty() {
        return no_data_chart("Fees");
    }

    let divisor = if unit == "btc" { 100_000_000.0 } else { 1.0 };
    let y_name = if unit == "btc" { "BTC" } else { "sats" };

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            if b.total_fees > 0 {
                let v = b.total_fees as f64 / divisor;
                let rounded = (v * 1000.0).round() / 1000.0;
                json!([ts_ms(b.timestamp), rounded])
            } else {
                json!([ts_ms(b.timestamp), null])
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis(y_name),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Fees", "type": "line", "sampling": "lttb", "data": raw,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

/// Fees from daily aggregates with unit toggle.
pub fn fees_chart_daily_unit(days: &[DailyAggregate], unit: &str) -> String {
    if days.is_empty() {
        return no_data_chart("Fees");
    }

    let divisor = if unit == "btc" { 100_000_000.0 } else { 1.0 };
    let y_name = if unit == "btc" { "BTC" } else { "sats" };

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<serde_json::Value> = days
        .iter()
        .map(|d| {
            if d.total_fees > 0 && d.block_count > 0 {
                let v = d.total_fees as f64 / d.block_count as f64 / divisor;
                let rounded = (v * 1000.0).round() / 1000.0;
                json!(rounded)
            } else {
                json!(null)
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis(y_name),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Avg Fees", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1.5, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
                "areaStyle": { "color": DATA_COLOR_FADED }
            }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Phase 2: Mining & Adoption charts
// ---------------------------------------------------------------------------

const PIE_COLORS: [&str; 11] = [
    "#4ecdc4", "#f7931a", "#ff6b6b", "#bb8fff", "#2ecc71", "#e74c3c",
    "#3498db", "#e67e22", "#1abc9c", "#9b59b6", "#95a5a6",
];

/// Miner dominance donut chart.
pub fn miner_dominance_chart(miners: &[MinerShare]) -> String {
    if miners.is_empty() {
        return no_data_chart("Miner Dominance");
    }

    // Top 10 + "Other"
    let (top, rest) = if miners.len() > 10 {
        (&miners[..10], &miners[10..])
    } else {
        (miners, &[][..])
    };

    let mut pie_data: Vec<serde_json::Value> = top
        .iter()
        .map(|m| {
            json!({
                "name": m.miner,
                "value": m.count
            })
        })
        .collect();

    if !rest.is_empty() {
        let other_count: u64 = rest.iter().map(|m| m.count).sum();
        pie_data.push(json!({
            "name": "Other",
            "value": other_count
        }));
    }

    let colors: Vec<&str> =
        PIE_COLORS.iter().copied().take(pie_data.len()).collect();

    serde_json::to_string(&json!({
        "backgroundColor": "transparent",
        "color": colors,
        "tooltip": {
            "trigger": "item",
            "formatter": "{b}: {c} blocks ({d}%)"
        },
        "legend": {
            "orient": "vertical",
            "right": 10,
            "top": 50,
            "textStyle": { "color": "#ccc", "fontSize": 11 }
        },
        "series": [{
            "name": "Miners",
            "type": "pie",
            "radius": ["45%", "70%"],
            "center": ["40%", "55%"],
            "avoidLabelOverlap": true,
            "itemStyle": {
                "borderRadius": 4,
                "borderColor": "#0e2a47",
                "borderWidth": 2
            },
            "label": {
                "show": true,
                "color": "#ccc",
                "fontSize": 11,
                "formatter": "{b}\n{d}%"
            },
            "emphasis": {
                "label": { "fontSize": 14, "fontWeight": "bold" }
            },
            "data": pie_data
        }]
    }))
    .unwrap_or_default()
}

/// Empty blocks scatter chart.
pub fn empty_blocks_chart(blocks: &[EmptyBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("No empty blocks in this range");
    }

    // Group empty blocks by month for a bar chart
    let mut monthly: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    for b in blocks {
        // Convert timestamp to YYYY-MM
        let dt = chrono::DateTime::from_timestamp(b.timestamp as i64, 0)
            .unwrap_or_default();
        let month = dt.format("%Y-%m").to_string();
        *monthly.entry(month).or_default() += 1;
    }

    let months: Vec<String> = monthly.keys().cloned().collect();
    let counts: Vec<u64> = monthly.values().copied().collect();

    build_option(json!({
        "xAxis": {
            "type": "category",
            "data": months,
            "axisLabel": { "color": "#aaa", "rotate": 45, "fontSize": 10 },
            "axisLine": { "lineStyle": { "color": "#555" } }
        },
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": { "trigger": "axis" },
        "legend": { "show": false },
        "grid": { "left": 45, "right": 20, "top": 25, "bottom": 80 },
        "series": [{
            "name": "Empty Blocks",
            "type": "bar",
            "data": counts,
            "itemStyle": { "color": DATA_COLOR },
            "barMaxWidth": 20
        }]
    }))
}

/// SegWit adoption % chart (per-block).
pub fn segwit_adoption_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("SegWit Adoption %");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 1 {
                let pct = b.segwit_spend_count as f64 / (b.tx_count - 1) as f64
                    * 100.0;
                (pct * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "SegWit %", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": DATA_COLOR },
        "itemStyle": { "color": DATA_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// SegWit adoption % from daily aggregates.
pub fn segwit_adoption_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("SegWit Adoption %");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 1.0 {
                let pct =
                    d.avg_segwit_spend_count / (d.avg_tx_count - 1.0) * 100.0;
                (pct * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("%"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "SegWit %", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

const TAPROOT_COLOR: &str = "#f7931a";

/// Taproot outputs per block chart.
pub fn taproot_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Taproot Outputs");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| b.taproot_spend_count as f64)
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_series: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Taproot Outputs", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": TAPROOT_COLOR },
        "itemStyle": { "color": TAPROOT_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_series,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Taproot outputs from daily aggregates.
pub fn taproot_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Taproot Outputs");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> =
        days.iter().map(|d| d.avg_taproot_spend_count).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "Taproot Outputs", "type": "line", "sampling": "lttb", "data": vals,
                "lineStyle": { "width": 1, "color": TAPROOT_COLOR },
                "itemStyle": { "color": TAPROOT_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// OP_RETURN bytes as percentage of total block size (per-block).
pub fn op_return_block_share_chart(blocks: &[OpReturnBlock]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Embedded Data Block Share");
    }

    let vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.size > 0 {
                (b.op_return_bytes as f64 / b.size as f64 * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();

    let raw: Vec<serde_json::Value> = blocks
        .iter()
        .zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v])))
        .collect();

    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "OP_RETURN %", "type": "line", "sampling": "lttb", "data": raw,
        "areaStyle": { "color": RUNES_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": RUNES_COLOR },
        "itemStyle": { "color": RUNES_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// OP_RETURN bytes as percentage of total block size (daily).
pub fn op_return_block_share_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Embedded Data Block Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days
        .iter()
        .map(|d| {
            let total_size = d.avg_size * d.block_count as f64;
            if total_size > 0.0 {
                (d.total_op_return_bytes as f64 / total_size * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma
        .iter()
        .map(|v| match v {
            Some(x) => json!(x),
            None => json!(null),
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            {
                "name": "OP_RETURN %", "type": "line", "sampling": "lttb", "data": vals,
                "areaStyle": { "color": RUNES_COLOR, "opacity": 0.15 },
                "lineStyle": { "width": 1, "color": RUNES_COLOR },
                "itemStyle": { "color": RUNES_COLOR }, "symbol": "none", "opacity": 0.4
            },
            {
                "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
                "lineStyle": { "width": 2, "color": MA_COLOR },
                "itemStyle": { "color": MA_COLOR }, "symbol": "none"
            }
        ]
    }))
}

const INSCRIPTION_COLOR: &str = "#ec4899"; // Pink for inscriptions

/// Ordinals inscription count per block.
pub fn inscription_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Inscriptions");
    }

    let vals: Vec<f64> = blocks.iter().map(|b| b.inscription_count as f64).collect();
    let raw: Vec<serde_json::Value> = blocks.iter().zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v])).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks.iter().zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v]))).collect();
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Inscriptions", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": INSCRIPTION_COLOR },
        "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Ordinals inscription count (daily).
pub fn inscription_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Inscriptions");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| d.avg_inscription_count).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma.iter()
        .map(|v| match v { Some(x) => json!(x), None => json!(null) }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Inscriptions", "type": "line", "sampling": "lttb", "data": vals,
              "lineStyle": { "width": 1, "color": INSCRIPTION_COLOR },
              "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Inscription data as % of block size (per-block).
pub fn inscription_share_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Inscription Block Share");
    }

    let vals: Vec<f64> = blocks.iter().map(|b| {
        if b.size > 0 { (b.inscription_bytes as f64 / b.size as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let raw: Vec<serde_json::Value> = blocks.iter().zip(vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v])).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks.iter().zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v]))).collect();
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Inscriptions %", "type": "line", "sampling": "lttb", "data": raw,
        "areaStyle": { "color": INSCRIPTION_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": INSCRIPTION_COLOR },
        "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Inscription data as % of block size (daily).
pub fn inscription_share_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Inscription Block Share");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| {
        if d.avg_size > 0.0 { (d.avg_inscription_bytes / d.avg_size * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma.iter()
        .map(|v| match v { Some(x) => json!(x), None => json!(null) }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Inscriptions %", "type": "line", "sampling": "lttb", "data": vals,
              "areaStyle": { "color": INSCRIPTION_COLOR, "opacity": 0.15 },
              "lineStyle": { "width": 1, "color": INSCRIPTION_COLOR },
              "itemStyle": { "color": INSCRIPTION_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals,
              "lineStyle": { "width": 2, "color": MA_COLOR },
              "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

const DISK_COLOR: &str = "#e74c3c"; // Red for disk size

/// Cumulative chain size over time (per-block).
/// `disk_size_gb` is the current size_on_disk from getblockchaininfo.
pub fn chain_size_chart(blocks: &[BlockSummary], disk_size_gb: f64) -> String {
    if blocks.is_empty() {
        return no_data_chart("Chain Size");
    }

    let mut cumulative: f64 = 0.0;
    let block_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            cumulative += b.size as f64 / 1_000_000_000.0;
            json!([ts_ms(b.timestamp), (cumulative * 1000.0).round() / 1000.0])
        })
        .collect();

    // Estimate disk size at each point using the known ratio at the tip
    let block_total = cumulative;
    let ratio = if block_total > 0.0 { disk_size_gb / block_total } else { 1.0 };
    let mut cumulative2: f64 = 0.0;
    let disk_data: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            cumulative2 += b.size as f64 / 1_000_000_000.0;
            let estimated = cumulative2 * ratio;
            json!([ts_ms(b.timestamp), (estimated * 1000.0).round() / 1000.0])
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("GB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Block Data", "type": "line", "sampling": "lttb", "data": block_data,
                "areaStyle": { "color": DATA_COLOR, "opacity": 0.1 },
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
            },
            {
                "name": "Disk Size (est.)", "type": "line", "sampling": "lttb", "data": disk_data,
                "lineStyle": { "width": 1.5, "color": DISK_COLOR, "type": "dashed" },
                "itemStyle": { "color": DISK_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Cumulative chain size over time (daily).
pub fn chain_size_chart_daily(days: &[DailyAggregate], disk_size_gb: f64) -> String {
    if days.is_empty() {
        return no_data_chart("Chain Size");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let mut cumulative: f64 = 0.0;
    let block_data: Vec<f64> = days
        .iter()
        .map(|d| {
            cumulative += d.avg_size * d.block_count as f64 / 1_000_000_000.0;
            (cumulative * 1000.0).round() / 1000.0
        })
        .collect();

    let block_total = cumulative;
    let ratio = if block_total > 0.0 { disk_size_gb / block_total } else { 1.0 };
    let mut cumulative2: f64 = 0.0;
    let disk_data: Vec<f64> = days
        .iter()
        .map(|d| {
            cumulative2 += d.avg_size * d.block_count as f64 / 1_000_000_000.0;
            let estimated = cumulative2 * ratio;
            (estimated * 1000.0).round() / 1000.0
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("GB"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Block Data", "type": "line", "sampling": "lttb", "data": block_data,
                "areaStyle": { "color": DATA_COLOR, "opacity": 0.1 },
                "lineStyle": { "width": 2, "color": DATA_COLOR },
                "itemStyle": { "color": DATA_COLOR }, "symbol": "none"
            },
            {
                "name": "Disk Size (est.)", "type": "line", "sampling": "lttb", "data": disk_data,
                "lineStyle": { "width": 1.5, "color": DISK_COLOR, "type": "dashed" },
                "itemStyle": { "color": DISK_COLOR }, "symbol": "none"
            }
        ]
    }))
}

const SEGWIT_V0_COLOR: &str = "#3b82f6"; // Blue for SegWit v0
const SEGWIT_V1_COLOR: &str = "#22c55e"; // Green for Taproot v1

/// SegWit v0 vs Taproot v1 stacked area chart (per-block).
pub fn witness_version_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Witness Versions");
    }

    // v0 = segwit_spend_count - taproot_spend_count (segwit includes all witness spends)
    // If segwit_spend_count already excludes taproot, use it directly
    let v0_vals: Vec<f64> = blocks
        .iter()
        .map(|b| {
            let v0 = b.segwit_spend_count.saturating_sub(b.taproot_spend_count);
            v0 as f64
        })
        .collect();
    let v1_vals: Vec<f64> = blocks
        .iter()
        .map(|b| b.taproot_spend_count as f64)
        .collect();

    let v0_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(v0_vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();
    let v1_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(v1_vals.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Spends"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit v0", "type": "line", "sampling": "lttb", "data": v0_data,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0.5, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot v1", "type": "line", "sampling": "lttb", "data": v1_data,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0.5, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// SegWit v0 vs Taproot v1 stacked area chart (daily).
pub fn witness_version_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Witness Versions");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let v0_vals: Vec<f64> = days
        .iter()
        .map(|d| {
            (d.avg_segwit_spend_count - d.avg_taproot_spend_count).max(0.0)
        })
        .collect();
    let v1_vals: Vec<f64> = days
        .iter()
        .map(|d| d.avg_taproot_spend_count)
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg Spends"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit v0", "type": "line", "sampling": "lttb", "data": v0_vals,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0.5, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot v1", "type": "line", "sampling": "lttb", "data": v1_vals,
                "stack": "witness", "areaStyle": { "opacity": 0.5 },
                "lineStyle": { "width": 0.5, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version percentage share — v0% vs v1% of total witness spends (per-block).
pub fn witness_version_pct_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Witness Version Share");
    }

    let v0_pct: Vec<f64> = blocks
        .iter()
        .map(|b| {
            let total = b.segwit_spend_count;
            if total > 0 {
                let v0 = total.saturating_sub(b.taproot_spend_count);
                (v0 as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let v1_pct: Vec<f64> = blocks
        .iter()
        .map(|b| {
            let total = b.segwit_spend_count;
            if total > 0 {
                (b.taproot_spend_count as f64 / total as f64 * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();

    let v0_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(v0_pct.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();
    let v1_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(v1_pct.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit v0", "type": "line", "sampling": "lttb", "data": v0_data,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot v1", "type": "line", "sampling": "lttb", "data": v1_data,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version percentage share (daily).
pub fn witness_version_pct_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Witness Version Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let v0_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            let total = d.avg_segwit_spend_count;
            if total > 0.0 {
                let v0 = (total - d.avg_taproot_spend_count).max(0.0);
                (v0 / total * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let v1_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            let total = d.avg_segwit_spend_count;
            if total > 0.0 {
                (d.avg_taproot_spend_count / total * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": { "type": "value", "name": "%", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "SegWit v0", "type": "line", "sampling": "lttb", "data": v0_pct,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot v1", "type": "line", "sampling": "lttb", "data": v1_pct,
                "stack": "pct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version as percentage of all transactions (per-block).
pub fn witness_version_tx_pct_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Witness Tx Share");
    }

    let v0_pct: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 1 {
                let v0 = b.segwit_spend_count.saturating_sub(b.taproot_spend_count);
                (v0 as f64 / (b.tx_count - 1) as f64 * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let v1_pct: Vec<f64> = blocks
        .iter()
        .map(|b| {
            if b.tx_count > 1 {
                (b.taproot_spend_count as f64 / (b.tx_count - 1) as f64 * 100.0 * 100.0).round()
                    / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let legacy_pct: Vec<f64> = v0_pct
        .iter()
        .zip(v1_pct.iter())
        .map(|(v0, v1)| (100.0 - v0 - v1).max(0.0))
        .collect();

    let v0_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(v0_pct.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();
    let v1_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(v1_pct.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();
    let legacy_data: Vec<serde_json::Value> = blocks
        .iter()
        .zip(legacy_pct.iter())
        .map(|(b, v)| json!([ts_ms(b.timestamp), v]))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": { "type": "value", "name": "% of Txs", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Legacy", "type": "line", "sampling": "lttb", "data": legacy_data,
                "stack": "txpct", "areaStyle": { "opacity": 0.4 },
                "lineStyle": { "width": 0, "color": "#888" },
                "itemStyle": { "color": "#888" }, "symbol": "none"
            },
            {
                "name": "SegWit v0", "type": "line", "sampling": "lttb", "data": v0_data,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot v1", "type": "line", "sampling": "lttb", "data": v1_data,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

/// Witness version as percentage of all transactions (daily).
pub fn witness_version_tx_pct_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Witness Tx Share");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let v0_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 1.0 {
                let v0 = (d.avg_segwit_spend_count - d.avg_taproot_spend_count).max(0.0);
                (v0 / (d.avg_tx_count - 1.0) * 100.0 * 100.0).round() / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let v1_pct: Vec<f64> = days
        .iter()
        .map(|d| {
            if d.avg_tx_count > 1.0 {
                (d.avg_taproot_spend_count / (d.avg_tx_count - 1.0) * 100.0 * 100.0).round()
                    / 100.0
            } else {
                0.0
            }
        })
        .collect();
    let legacy_pct: Vec<f64> = v0_pct
        .iter()
        .zip(v1_pct.iter())
        .map(|(v0, v1)| (100.0 - v0 - v1).max(0.0))
        .collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": { "type": "value", "name": "% of Txs", "max": 100,
            "nameTextStyle": { "color": "#aaa" },
            "axisLabel": { "color": "#aaa" },
            "axisLine": { "lineStyle": { "color": "#555" } },
            "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.20)", "type": "dashed" } }
        },
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            {
                "name": "Legacy", "type": "line", "sampling": "lttb", "data": legacy_pct,
                "stack": "txpct", "areaStyle": { "opacity": 0.4 },
                "lineStyle": { "width": 0, "color": "#888" },
                "itemStyle": { "color": "#888" }, "symbol": "none"
            },
            {
                "name": "SegWit v0", "type": "line", "sampling": "lttb", "data": v0_pct,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V0_COLOR },
                "itemStyle": { "color": SEGWIT_V0_COLOR }, "symbol": "none"
            },
            {
                "name": "Taproot v1", "type": "line", "sampling": "lttb", "data": v1_pct,
                "stack": "txpct", "areaStyle": { "opacity": 0.6 },
                "lineStyle": { "width": 0, "color": SEGWIT_V1_COLOR },
                "itemStyle": { "color": SEGWIT_V1_COLOR }, "symbol": "none"
            }
        ]
    }))
}

// ---------------------------------------------------------------------------
// Address type, RBF, UTXO flow, witness share charts
// ---------------------------------------------------------------------------

/// Address type evolution — stacked area (per-block).
pub fn address_type_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Address Types");
    }

    let make_data = |f: fn(&BlockSummary) -> u64| -> Vec<serde_json::Value> {
        blocks.iter().map(|b| json!([ts_ms(b.timestamp), f(b)])).collect()
    };

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2pkh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2sh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2wpkh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2wsh_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2tr_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "sampling": "lttb", "data": make_data(|b| b.p2pk_count), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Address type evolution — stacked area (daily).
pub fn address_type_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Address Types");
    }

    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg Outputs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "P2PKH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| d.avg_p2pkh_count).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PKH_COLOR }, "itemStyle": { "color": P2PKH_COLOR }, "symbol": "none" },
            { "name": "P2SH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| d.avg_p2sh_count).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2SH_COLOR }, "itemStyle": { "color": P2SH_COLOR }, "symbol": "none" },
            { "name": "P2WPKH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| d.avg_p2wpkh_count).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none" },
            { "name": "P2WSH", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| d.avg_p2wsh_count).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2WSH_COLOR }, "itemStyle": { "color": P2WSH_COLOR }, "symbol": "none" },
            { "name": "P2TR", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| d.avg_p2tr_count).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2TR_COLOR }, "itemStyle": { "color": P2TR_COLOR }, "symbol": "none" },
            { "name": "P2PK", "type": "line", "sampling": "lttb", "data": days.iter().map(|d| d.avg_p2pk_count).collect::<Vec<f64>>(), "stack": "addr", "areaStyle": { "opacity": 0.6 }, "lineStyle": { "width": 0, "color": P2PK_COLOR }, "itemStyle": { "color": P2PK_COLOR }, "symbol": "none" }
        ]
    }))
}

/// Witness data as % of block size (per-block).
pub fn witness_share_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("Witness Data Share");
    }

    let vals: Vec<f64> = blocks.iter().map(|b| {
        if b.size > 0 { (b.witness_bytes as f64 / b.size as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let raw: Vec<serde_json::Value> = blocks.iter().zip(vals.iter()).map(|(b, v)| json!([ts_ms(b.timestamp), v])).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks.iter().zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v]))).collect();
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "Witness %", "type": "line", "sampling": "lttb", "data": raw,
        "areaStyle": { "color": P2WPKH_COLOR, "opacity": 0.15 },
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": P2WPKH_COLOR },
        "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// Witness data as % of block size (daily).
pub fn witness_share_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("Witness Data Share");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| {
        if d.avg_size > 0.0 { (d.avg_witness_bytes / d.avg_size * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma.iter().map(|v| match v { Some(x) => json!(x), None => json!(null) }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "Witness %", "type": "line", "sampling": "lttb", "data": vals, "areaStyle": { "color": P2WPKH_COLOR, "opacity": 0.15 }, "lineStyle": { "width": 1, "color": P2WPKH_COLOR }, "itemStyle": { "color": P2WPKH_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals, "lineStyle": { "width": 2, "color": MA_COLOR }, "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// RBF adoption — % of transactions signaling RBF (per-block).
pub fn rbf_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("RBF Adoption");
    }

    let vals: Vec<f64> = blocks.iter().map(|b| {
        if b.tx_count > 1 { (b.rbf_count as f64 / (b.tx_count - 1) as f64 * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let raw: Vec<serde_json::Value> = blocks.iter().zip(vals.iter()).map(|(b, v)| json!([ts_ms(b.timestamp), v])).collect();
    let ma = moving_average(&vals, 144);
    let ma_data: Vec<serde_json::Value> = blocks.iter().zip(ma.iter())
        .filter_map(|(b, m)| m.map(|v| json!([ts_ms(b.timestamp), v]))).collect();
    let has_ma = show_ma(blocks.len());

    let mut series = vec![json!({
        "name": "RBF %", "type": "line", "sampling": "lttb", "data": raw,
        "lineStyle": { "width": if has_ma { 1.0 } else { 1.5 }, "color": RBF_COLOR },
        "itemStyle": { "color": RBF_COLOR }, "symbol": "none",
        "opacity": if has_ma { 0.4 } else { 1.0 }
    })];
    if has_ma {
        series.push(json!({
            "name": "144-block MA", "type": "line", "sampling": "lttb", "data": ma_data,
            "lineStyle": { "width": 2, "color": MA_COLOR },
            "itemStyle": { "color": MA_COLOR }, "symbol": "none"
        }));
    }

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("% of Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": has_ma },
        "series": series
    }))
}

/// RBF adoption (daily).
pub fn rbf_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("RBF Adoption");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let vals: Vec<f64> = days.iter().map(|d| {
        if d.avg_tx_count > 1.0 { (d.avg_rbf_count / (d.avg_tx_count - 1.0) * 100.0 * 100.0).round() / 100.0 } else { 0.0 }
    }).collect();
    let ma = moving_average(&vals, 7);
    let ma_vals: Vec<serde_json::Value> = ma.iter().map(|v| match v { Some(x) => json!(x), None => json!(null) }).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("% of Txs"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "series": [
            { "name": "RBF %", "type": "line", "sampling": "lttb", "data": vals, "lineStyle": { "width": 1, "color": RBF_COLOR }, "itemStyle": { "color": RBF_COLOR }, "symbol": "none", "opacity": 0.4 },
            { "name": "7-day MA", "type": "line", "sampling": "lttb", "data": ma_vals, "lineStyle": { "width": 2, "color": MA_COLOR }, "itemStyle": { "color": MA_COLOR }, "symbol": "none" }
        ]
    }))
}

/// UTXO flow — inputs (consumed) vs outputs (created) per block.
pub fn utxo_flow_chart(blocks: &[BlockSummary]) -> String {
    if blocks.is_empty() {
        return no_data_chart("UTXO Flow");
    }

    let inputs: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.input_count])).collect();
    let outputs: Vec<serde_json::Value> = blocks.iter().map(|b| json!([ts_ms(b.timestamp), b.output_count])).collect();

    build_option(json!({
        "xAxis": x_axis_for(false, &[]),
        "yAxis": y_axis("Count"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Inputs (consumed)", "type": "line", "sampling": "lttb", "data": inputs, "lineStyle": { "width": 1, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": 0.5 },
            { "name": "Outputs (created)", "type": "line", "sampling": "lttb", "data": outputs, "lineStyle": { "width": 1, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": 0.5 }
        ]
    }))
}

/// UTXO flow (daily).
pub fn utxo_flow_chart_daily(days: &[DailyAggregate]) -> String {
    if days.is_empty() {
        return no_data_chart("UTXO Flow");
    }
    let cats: Vec<String> = days.iter().map(|d| d.date.clone()).collect();
    let inputs: Vec<f64> = days.iter().map(|d| d.avg_input_count).collect();
    let outputs: Vec<f64> = days.iter().map(|d| d.avg_output_count).collect();

    build_option(json!({
        "xAxis": x_axis_for(true, &cats),
        "yAxis": y_axis("Avg/Block"),
        "dataZoom": data_zoom(),
        "tooltip": tooltip_axis(),
        "legend": { "show": true },
        "series": [
            { "name": "Inputs (consumed)", "type": "line", "sampling": "lttb", "data": inputs, "lineStyle": { "width": 1, "color": "#ef4444" }, "itemStyle": { "color": "#ef4444" }, "symbol": "none", "opacity": 0.5 },
            { "name": "Outputs (created)", "type": "line", "sampling": "lttb", "data": outputs, "lineStyle": { "width": 1, "color": "#22c55e" }, "itemStyle": { "color": "#22c55e" }, "symbol": "none", "opacity": 0.5 }
        ]
    }))
}

/// Mempool usage gauge chart.
pub fn mempool_gauge(usage: u64, max: u64) -> String {
    let pct = if max > 0 {
        (usage as f64 / max as f64 * 100.0 * 10.0).round() / 10.0
    } else {
        0.0
    };

    serde_json::to_string(&json!({
        "backgroundColor": "transparent",
        "series": [{
            "type": "gauge",
            "center": ["50%", "55%"],
            "radius": "85%",
            "detail": {
                "formatter": "{value}%",
                "color": "#e0e0e0",
                "fontSize": 16,
                "offsetCenter": [0, "65%"]
            },
            "data": [{ "value": pct, "name": "Mempool" }],
            "axisLine": {
                "lineStyle": {
                    "width": 12,
                    "color": [[0.5, "#4ecdc4"], [0.8, "#f7931a"], [1, "#e74c3c"]]
                }
            },
            "pointer": { "width": 3, "length": "65%" },
            "title": { "color": "#aaa", "fontSize": 11, "offsetCenter": [0, "82%"] },
            "axisTick": { "show": false },
            "splitLine": { "length": 8, "lineStyle": { "color": "#666" } },
            "axisLabel": { "color": "#666", "distance": 15, "fontSize": 10 }
        }]
    }))
    .unwrap_or_default()
}
