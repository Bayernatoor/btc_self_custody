//! ECharts option JSON builders.
//!
//! Runs on the client (WASM) - takes typed data (`BlockSummary`, `DailyAggregate`,
//! etc.) and produces `serde_json::Value` objects that are serialized to JSON strings
//! and passed to ECharts via JS interop (`/js/stats.js`).
//!
//! Each chart function comes in two variants: per-block (for short ranges under
//! ~5000 blocks, using time-axis with click-to-detail) and daily (for longer ranges,
//! using category axis with averaged values).
//!
//! Submodules:
//! - `network`   - Block size, weight utilization, tx count, TPS, intervals, chain size, largest tx
//! - `adoption`  - SegWit adoption, Taproot outputs, witness versions, address types, spend types
//! - `fees`      - Total fees, avg fee/tx, median fee rate, fee bands, subsidy vs fees
//! - `mining`    - Difficulty, miner dominance donut, empty blocks
//! - `embedded`  - OP_RETURN protocols, inscriptions, combined embedded data
//! - `signaling` - BIP signaling scatter and period history bar chart
//! - `gauges`    - Mempool usage gauge
//! - `tx_metrics` - Address type evolution, witness share, RBF, UTXO flow, batching
//!
//! Shared helpers: `chart_defaults()`, `build_option()`, `data_zoom()`, `tooltip_axis()`,
//! `moving_average()`, `apply_overlays()`, and the `chart_memo!` macro (in `shared.rs`).

use serde_json::json;

use crate::stats::types::*;

pub mod adoption;
pub mod embedded;
pub mod fees;
pub mod gauges;
pub mod mining;
pub mod network;
pub mod signaling;
pub mod tx_metrics;

pub use adoption::*;
pub use embedded::*;
pub use fees::*;
pub use gauges::*;
pub use mining::*;
pub use network::*;
pub use signaling::*;
pub use tx_metrics::*;

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
pub(crate) const P2PK_COLOR: &str = "#94a3b8"; // Slate gray (ancient/rare)
pub(crate) const P2PKH_COLOR: &str = "#ef4444"; // Red (legacy dominant)
pub(crate) const P2SH_COLOR: &str = "#f59e0b"; // Amber (multisig era)
pub(crate) const P2WPKH_COLOR: &str = "#3b82f6"; // Blue (SegWit v0)
pub(crate) const P2WSH_COLOR: &str = "#8b5cf6"; // Purple (SegWit v0 multisig)
pub(crate) const P2TR_COLOR: &str = "#22c55e"; // Green (Taproot)
pub(crate) const RBF_COLOR: &str = "#06b6d4"; // Cyan

pub(crate) const SUBSIDY_COLOR: &str = "#9b59b6";
pub(crate) const DISK_COLOR: &str = "#e74c3c"; // Red for disk size
pub(crate) const TAPROOT_COLOR: &str = "#f7931a";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Base ECharts option with dark theme defaults (transparent bg, dark grid, toolbox, animation).
pub(crate) fn chart_defaults() -> serde_json::Value {
    json!({
        "backgroundColor": "transparent",
        "textStyle": { "color": "#aaa", "fontFamily": "Inter, system-ui, sans-serif" },
        "grid": { "left": 55, "right": 20, "top": 50, "bottom": 65, "containLabel": true },
        "legend": { "textStyle": { "color": "#ccc", "fontSize": 11 }, "top": 25, "left": "center", "type": "scroll" },
        "toolbox": {
            "feature": {
                "restore": { "title": "Reset zoom" },
                "dataZoom": { "title": { "zoom": "Zoom", "back": "Undo zoom" } },
                "saveAsImage": { "title": "Save image", "backgroundColor": "#0d2137" }
            },
            "iconStyle": { "borderColor": "#aaa" },
            "emphasis": { "iconStyle": { "borderColor": "#f7931a" } },
            "right": 10, "top": 0,
            "itemSize": 14
        },
        "animation": true,
        "animationDuration": 300,
        "progressive": 500,
        "progressiveThreshold": 3000,
        // Attribution watermark, Glassnode-style: centered behind the data
        // as a large faded brand mark. Baked into every render, so it shows
        // up in ECharts PNG exports, OS screenshots, and mobile screenshots
        // without any export pipeline work. `z: 0` keeps it BEHIND the
        // series (default z: 2) so data lines remain fully legible.
        // `silent: true` stops it from intercepting chart mouse events.
        "graphic": [{
            "type": "text",
            "left": "center",
            "top": "middle",
            "silent": true,
            "z": 0,
            "style": {
                "text": "wehodlbtc",
                "fill": "rgba(255,255,255,0.06)",
                "font": "bold 56px Inter, system-ui, sans-serif",
                "textAlign": "center",
                "textVerticalAlign": "middle"
            }
        }]
    })
}

/// Standard data zoom: bottom slider only. No `inside` component.
///
/// Rationale: ECharts' `inside` dataZoom intercepts wheel events for the
/// entire chart area, even when `zoomOnMouseWheel` is disabled or gated
/// behind a modifier key — it still calls `preventDefault()` to keep the
/// option available, which traps page scrolling when the cursor passes
/// over a chart. Removing the `inside` component lets wheel events pass
/// through to the page untouched. Users can still zoom via the bottom
/// slider (drag handles to set range) and via the toolbox "zoom" brush
/// (box-select a region), which cover every use case the inside wheel
/// zoom did.
pub(crate) fn data_zoom() -> serde_json::Value {
    json!([
        {
            "type": "slider", "start": 0, "end": 100, "height": 20, "bottom": 8,
            "borderColor": "#333", "fillerColor": "rgba(247,147,26,0.15)",
            "handleStyle": { "color": "#f7931a" }, "textStyle": { "color": "#aaa", "fontSize": 10 }
        }
    ])
}

/// Standard axis-trigger tooltip with dark theme styling.
pub(crate) fn tooltip_axis() -> serde_json::Value {
    json!({
        "trigger": "axis",
        "axisPointer": { "type": "line" },
        "backgroundColor": "rgba(13,33,55,0.95)",
        "borderColor": "rgba(255,255,255,0.1)",
        "textStyle": { "color": "rgba(255,255,255,0.85)", "fontSize": 12 }
    })
}

/// X-axis config: time-axis for per-block charts, category-axis for daily charts.
pub(crate) fn x_axis_for(
    is_daily: bool,
    categories: &[String],
) -> serde_json::Value {
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

/// Y-axis config with name label, dashed grid lines, and dark theme styling.
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

/// Fallback chart option when no data is available for the current range.
pub(crate) fn no_data_chart(title: &str) -> serde_json::Value {
    no_data_chart_with_hint(
        title,
        "Select a shorter range (1M or less) to view per-block data",
    )
}

/// Fallback chart with a custom hint message.
pub(crate) fn no_data_chart_with_hint(
    title: &str,
    hint: &str,
) -> serde_json::Value {
    let mut opt = chart_defaults();
    let m = opt.as_object_mut().unwrap();
    m.insert(
        "title".into(),
        json!({
            "text": title,
            "subtext": hint,
            "textStyle": { "color": "rgba(255,255,255,0.4)", "fontSize": 14 },
            "subtextStyle": { "color": "rgba(255,255,255,0.45)", "fontSize": 12 },
            "left": "center", "top": "middle",
            "itemGap": 8
        }),
    );
    opt
}

/// Compute a simple moving average with the given window size. Returns `None`
/// for the first `window-1` elements where insufficient data exists.
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

/// Data point with block height for click-to-detail: [timestamp_ms, value, height]
pub(crate) fn dp(
    b: &BlockSummary,
    value: impl serde::Serialize,
) -> serde_json::Value {
    json!([ts_ms(b.timestamp), value, b.height])
}

// ---------------------------------------------------------------------------
// Fast data array builders (direct String writes, avoid json!() per point)
// ---------------------------------------------------------------------------

use std::fmt::Write;

/// Build a JSON array of [timestamp_ms, value, height] data points as a raw JSON string.
/// Avoids 4000+ json!() allocations by writing directly to a String buffer.
/// The returned RawValue can be embedded in a serde_json::Value via `data_array_value()`.
pub(crate) fn build_data_array_f64(
    blocks: &[BlockSummary],
    value_fn: impl Fn(&BlockSummary) -> f64,
) -> String {
    let mut buf = String::with_capacity(blocks.len() * 30);
    buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        let v = value_fn(b);
        let _ = write!(buf, "[{},{},{}]", ts_ms(b.timestamp), v, b.height);
    }
    buf.push(']');
    buf
}

/// Build a JSON array of [timestamp_ms, value, height] for integer values.
pub(crate) fn build_data_array_i64(
    blocks: &[BlockSummary],
    value_fn: impl Fn(&BlockSummary) -> i64,
) -> String {
    let mut buf = String::with_capacity(blocks.len() * 30);
    buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        let _ = write!(
            buf,
            "[{},{},{}]",
            ts_ms(b.timestamp),
            value_fn(b),
            b.height
        );
    }
    buf.push(']');
    buf
}

/// Build a JSON array of [timestamp_ms, value] for moving average data.
/// None values are written as null.
pub(crate) fn build_ma_array(
    blocks: &[BlockSummary],
    ma: &[Option<f64>],
) -> String {
    let mut buf = String::with_capacity(blocks.len() * 24);
    buf.push('[');
    for (i, (b, m)) in blocks.iter().zip(ma.iter()).enumerate() {
        if i > 0 {
            buf.push(',');
        }
        match m {
            Some(v) => {
                let _ = write!(buf, "[{},{}]", ts_ms(b.timestamp), v);
            }
            None => {
                let _ = write!(buf, "[{},null]", ts_ms(b.timestamp));
            }
        }
    }
    buf.push(']');
    buf
}

/// Convert a pre-built JSON array string into a serde_json::Value for embedding
/// in chart option objects. Uses RawValue to avoid re-parsing.
pub(crate) fn data_array_value(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or(json!([]))
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

/// Merge chart_defaults with additional fields (consumes extra to avoid cloning).
pub(crate) fn build_option(extra: serde_json::Value) -> serde_json::Value {
    let mut base = chart_defaults();
    if let (Some(base_obj), serde_json::Value::Object(extra_obj)) =
        (base.as_object_mut(), extra)
    {
        for (k, v) in extra_obj {
            // Deep-merge legend so "show" doesn't wipe out default textStyle/position
            if k == "legend" {
                if let Some(base_legend) =
                    base_obj.get_mut("legend").and_then(|l| l.as_object_mut())
                {
                    if let serde_json::Value::Object(extra_legend) = v {
                        for (lk, lv) in extra_legend {
                            base_legend.insert(lk, lv);
                        }
                        continue;
                    }
                }
            }
            base_obj.insert(k, v);
        }
    }
    base
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
const HALVING_DATES: &[&str] =
    &["2012-11-28", "2016-07-09", "2020-05-11", "2024-04-20"];

/// Notable BIP activation block heights and timestamps.
const BIP_ACTIVATIONS: &[(u64, u64, &str)] = &[
    (227_835, 1_363_609_548, "BIP-34 (Height in Coinbase)"),
    (227_931, 1_363_636_474, "BIP-16 (P2SH)"),
    (363_725, 1_436_486_408, "BIP-66 (Strict DER)"),
    (388_381, 1_449_187_214, "BIP-65 (CLTV)"),
    (419_328, 1_467_331_589, "BIP-68/112/113 (CSV)"),
    (477_120, 1_500_584_608, "BIP-91 (SegWit Signaling)"),
    (481_824, 1_503_539_857, "BIP-141 (SegWit)"),
    (709_632, 1_636_866_927, "BIP-341/342 (Taproot + Tapscript)"),
];

/// BIP activation dates for daily-mode charts (derived from block timestamps above).
const BIP_ACTIVATION_DATES: &[(&str, &str)] = &[
    ("2013-03-18", "BIP-34 (Height in Coinbase)"),
    ("2013-03-18", "BIP-16 (P2SH)"),
    ("2015-07-10", "BIP-66 (Strict DER)"),
    ("2015-12-04", "BIP-65 (CLTV)"),
    ("2016-07-01", "BIP-68/112/113 (CSV)"),
    ("2017-07-21", "BIP-91 (SegWit Signaling)"),
    ("2017-08-24", "BIP-141 (SegWit)"),
    ("2021-11-14", "BIP-341/342 (Taproot + Tapscript)"),
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

impl OverlayFlags {
    /// Compact string key for cache differentiation.
    pub fn cache_key(&self) -> String {
        format!(
            "h{}b{}c{}e{}p{}s{}",
            self.halvings as u8,
            self.bip_activations as u8,
            self.core_releases as u8,
            self.events as u8,
            self.price_data.len(),
            self.chain_size_data.len(),
        )
    }
}

/// Configuration for a single mark line overlay type.
struct MarkLineStyle {
    color: &'static str,
    line_type: &'static str,
    width: f64,
    font_size: u32,
    font_weight: &'static str,
    rotate: Option<u32>,
    bg_alpha: &'static str,
    padding: [u32; 2],
    border_radius: u32,
}

/// Create mark line entries for a given overlay type.
/// `daily_data` is used for category-axis charts, `ts_data` for time-axis charts.
fn make_mark_lines(
    is_daily: bool,
    daily_data: &[(&str, &str)],
    ts_data: &[(u64, &str)],
    style: &MarkLineStyle,
) -> Vec<serde_json::Value> {
    if is_daily {
        daily_data
            .iter()
            .map(|&(date, name)| {
                json!({
                    "xAxis": date,
                    "lineStyle": { "color": style.color, "type": style.line_type, "width": style.width },
                    "label": {
                        "show": true, "formatter": name, "color": style.color,
                        "fontSize": style.font_size, "fontWeight": style.font_weight,
                        "position": "insideEndTop",
                        "rotate": style.rotate.unwrap_or(0),
                        "backgroundColor": format!("rgba(10,25,41,{})", style.bg_alpha),
                        "padding": style.padding, "borderRadius": style.border_radius
                    }
                })
            })
            .collect()
    } else {
        ts_data
            .iter()
            .map(|&(ts, name)| {
                json!({
                    "xAxis": ts * 1000,
                    "lineStyle": { "color": style.color, "type": style.line_type, "width": style.width },
                    "label": {
                        "show": true, "formatter": name, "color": style.color,
                        "fontSize": style.font_size, "fontWeight": style.font_weight,
                        "position": "insideEndTop",
                        "rotate": style.rotate.unwrap_or(0),
                        "backgroundColor": format!("rgba(10,25,41,{})", style.bg_alpha),
                        "padding": style.padding, "borderRadius": style.border_radius
                    }
                })
            })
            .collect()
    }
}

/// Extract the visible time range (in milliseconds) from a chart option.
fn chart_visible_range(
    obj: &serde_json::Map<String, serde_json::Value>,
    is_daily: bool,
) -> (u64, u64) {
    if is_daily {
        let cats = obj
            .get("xAxis")
            .and_then(|x| x.get("data"))
            .and_then(|d| d.as_array());
        if let Some(cats) = cats {
            let parse = |s: &str| -> u64 {
                chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                    .map(|d| {
                        d.and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp()
                            as u64
                            * 1000
                    })
                    .unwrap_or(0)
            };
            let first = cats.first().and_then(|v| v.as_str()).unwrap_or("");
            let last = cats.last().and_then(|v| v.as_str()).unwrap_or("");
            (parse(first), parse(last))
        } else {
            (0, u64::MAX)
        }
    } else {
        let mut min_ts = u64::MAX;
        let mut max_ts = 0u64;
        if let Some(series) = obj.get("series") {
            if let Some(first_s) = series.as_array().and_then(|a| a.first()) {
                if let Some(data) =
                    first_s.get("data").and_then(|d| d.as_array())
                {
                    for pt in data {
                        if let Some(arr) = pt.as_array() {
                            let ts = arr.first().and_then(|v| {
                                v.as_u64()
                                    .or_else(|| v.as_f64().map(|f| f as u64))
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
        if min_ts == u64::MAX {
            min_ts = 0;
        }
        (min_ts, max_ts)
    }
}

/// Interpolate overlay data points onto chart categories/timestamps.
fn interpolate_overlay_data(
    obj: &serde_json::Map<String, serde_json::Value>,
    filtered: &[(u64, f64)],
    is_daily: bool,
) -> Vec<serde_json::Value> {
    if is_daily {
        let categories = obj
            .get("xAxis")
            .and_then(|x| x.get("data"))
            .and_then(|d| d.as_array())
            .cloned()
            .unwrap_or_default();

        categories
            .iter()
            .map(|cat| {
                let cat_ms = chrono::NaiveDate::parse_from_str(
                    cat.as_str().unwrap_or_default(),
                    "%Y-%m-%d",
                )
                .map(|d| {
                    d.and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp()
                        as u64
                        * 1000
                })
                .unwrap_or(0);
                if cat_ms == 0 {
                    return json!(null);
                }
                interpolate_value(filtered, cat_ms, false)
            })
            .collect()
    } else {
        // For time-axis: interpolate at each block timestamp
        let block_timestamps: Vec<u64> = obj
            .get("series")
            .and_then(|s| s.as_array())
            .and_then(|a| a.first())
            .and_then(|s| s.get("data"))
            .and_then(|d| d.as_array())
            .map(|pts| {
                pts.iter()
                    .filter_map(|pt| {
                        pt.as_array().and_then(|a| a.first()).and_then(|v| {
                            v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        if block_timestamps.is_empty() {
            filtered.iter().map(|&(ts, v)| json!([ts, v])).collect()
        } else {
            block_timestamps
                .iter()
                .map(|&bts| interpolate_value(filtered, bts, true))
                .collect()
        }
    }
}

/// Interpolate a single value from sorted data points at a given timestamp.
fn interpolate_value(
    data: &[(u64, f64)],
    ts: u64,
    as_array: bool,
) -> serde_json::Value {
    match data.binary_search_by_key(&ts, |&(t, _)| t) {
        Ok(idx) => {
            if as_array {
                json!([ts, data[idx].1])
            } else {
                json!(data[idx].1)
            }
        }
        Err(idx) => {
            if idx == 0 {
                if as_array {
                    json!([ts, serde_json::Value::Null])
                } else {
                    json!(null)
                }
            } else if idx >= data.len() {
                let v = data.last().unwrap().1;
                if as_array {
                    json!([ts, v])
                } else {
                    json!(v)
                }
            } else {
                let (t0, v0) = data[idx - 1];
                let (t1, v1) = data[idx];
                let interp = if t1 == t0 {
                    v0
                } else {
                    let frac = (ts - t0) as f64 / (t1 - t0) as f64;
                    ((v0 + frac * (v1 - v0)) * 100.0).round() / 100.0
                };
                if as_array {
                    json!([ts, interp])
                } else {
                    json!(interp)
                }
            }
        }
    }
}

/// Add a secondary Y-axis overlay series (price or chain size).
fn add_series_overlay(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    source_data: &[(u64, f64)],
    is_daily: bool,
    name: &str,
    unit: &str,
    color: &str,
) {
    let (min_ms, max_ms) = chart_visible_range(obj, is_daily);
    let range_min = min_ms.saturating_sub(86_400_000);
    let range_max = max_ms.saturating_add(86_400_000);
    let filtered: Vec<(u64, f64)> = source_data
        .iter()
        .filter(|&&(ts, _)| ts >= range_min && ts <= range_max)
        .copied()
        .collect();
    if filtered.is_empty() {
        return;
    }

    // Convert yAxis to array and add secondary axis
    let y_axis = obj.remove("yAxis");
    let mut y_axes = match y_axis {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(v) => vec![v],
        None => vec![json!({ "type": "value" })],
    };
    let axis_idx = y_axes.len();
    y_axes.push(json!({
        "type": "value",
        "name": unit,
        "nameTextStyle": { "color": color },
        "position": "right",
        "offset": if y_axes.len() > 1 { 60 } else { 0 },
        "axisLabel": { "color": color, "fontSize": 10 },
        "axisLine": { "lineStyle": { "color": color } },
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
            g.insert(
                "right".into(),
                json!(current.max(70) + if axis_idx > 1 { 60 } else { 0 }),
            );
        }
    }

    let series_data = interpolate_overlay_data(obj, &filtered, is_daily);

    let series_obj = json!({
        "name": name,
        "type": "line",
        "yAxisIndex": axis_idx,
        "data": series_data,
        "connectNulls": true,
        "lineStyle": { "color": color, "width": 1.5, "opacity": 0.8 },
        "itemStyle": { "color": color },
        "symbol": "none",
        "smooth": true,
        "z": 1
    });

    if let Some(series) = obj.get_mut("series") {
        if let Some(arr) = series.as_array_mut() {
            arr.push(series_obj);
        }
    }
    if let Some(legend) = obj.get_mut("legend") {
        if let Some(l) = legend.as_object_mut() {
            l.insert("show".into(), json!(true));
        }
    }
}

/// Merge overlay markLines and series into an already-parsed chart option Value.
/// Works for both time-axis (per-block) and category-axis (daily) charts.
pub fn apply_overlays(
    opt: &mut serde_json::Value,
    overlays: &OverlayFlags,
    is_daily: bool,
) {
    let has_any = overlays.halvings
        || overlays.bip_activations
        || overlays.core_releases
        || overlays.events
        || !overlays.price_data.is_empty()
        || !overlays.chain_size_data.is_empty();

    if !has_any {
        return;
    }

    let obj = match opt.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    // Widen grid.right when a price axis or markLine labels are present
    let need_right_space = !overlays.price_data.is_empty()
        || overlays.halvings
        || overlays.bip_activations
        || overlays.core_releases
        || overlays.events;
    if need_right_space {
        if let Some(grid) = obj.get_mut("grid") {
            if let Some(g) = grid.as_object_mut() {
                g.insert(
                    "right".into(),
                    json!(if !overlays.price_data.is_empty() {
                        70
                    } else {
                        60
                    }),
                );
            }
        }
    }

    let has_right_axis =
        !overlays.price_data.is_empty() || !overlays.chain_size_data.is_empty();

    // --- Mark lines ---
    let mut mark_lines: Vec<serde_json::Value> = Vec::new();

    if overlays.halvings {
        // Halvings use a special label ("½") instead of the name
        let halving_daily: Vec<(&str, &str)> =
            HALVING_DATES.iter().map(|&d| (d, "½")).collect();
        let halving_ts: Vec<(u64, &str)> =
            HALVINGS.iter().map(|&(_, ts, _)| (ts, "½")).collect();
        mark_lines.extend(make_mark_lines(
            is_daily,
            &halving_daily,
            &halving_ts,
            &MarkLineStyle {
                color: "#f7931a",
                line_type: "dashed",
                width: 1.5,
                font_size: 13,
                font_weight: "bold",
                rotate: None,
                bg_alpha: "0.8",
                padding: [2, 4],
                border_radius: 2,
            },
        ));
    }
    if overlays.bip_activations {
        let ts_data: Vec<(u64, &str)> =
            BIP_ACTIVATIONS.iter().map(|&(_, ts, n)| (ts, n)).collect();
        mark_lines.extend(make_mark_lines(
            is_daily,
            BIP_ACTIVATION_DATES,
            &ts_data,
            &MarkLineStyle {
                color: "#4ecdc4",
                line_type: "dotted",
                width: 1.0,
                font_size: 11,
                font_weight: "normal",
                rotate: Some(90),
                bg_alpha: "0.8",
                padding: [2, 4],
                border_radius: 2,
            },
        ));
    }
    if overlays.core_releases {
        mark_lines.extend(make_mark_lines(
            is_daily,
            CORE_RELEASE_DATES,
            CORE_RELEASES,
            &MarkLineStyle {
                color: "#a855f7",
                line_type: "dotted",
                width: 1.0,
                font_size: 10,
                font_weight: "normal",
                rotate: Some(90),
                bg_alpha: "0.8",
                padding: [2, 3],
                border_radius: 2,
            },
        ));
    }
    if overlays.events {
        mark_lines.extend(make_mark_lines(
            is_daily,
            EVENT_DATES,
            EVENTS,
            &MarkLineStyle {
                color: "#ef4444",
                line_type: "solid",
                width: 2.0,
                font_size: 11,
                font_weight: "bold",
                rotate: Some(90),
                bg_alpha: "0.85",
                padding: [3, 5],
                border_radius: 3,
            },
        ));
    }

    // Attach markLines to the first series
    if !mark_lines.is_empty() {
        if let Some(series) = obj.get_mut("series") {
            if let Some(arr) = series.as_array_mut() {
                if let Some(first) = arr.first_mut() {
                    if let Some(s) = first.as_object_mut() {
                        s.insert(
                            "markLine".into(),
                            json!({ "silent": true, "symbol": "none", "data": mark_lines }),
                        );
                    }
                }
            }
        }
    }

    // --- Series overlays (price, chain size) ---
    if !overlays.price_data.is_empty() {
        add_series_overlay(
            obj,
            &overlays.price_data,
            is_daily,
            "Price (USD)",
            "USD",
            "#e6c84e",
        );
    }
    if !overlays.chain_size_data.is_empty() {
        add_series_overlay(
            obj,
            &overlays.chain_size_data,
            is_daily,
            "Chain Size (GB)",
            "GB",
            "#10b981",
        );
    }

    // Reposition toolbox clear of any right-side axes
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
                t.insert("right".into(), json!(grid_right + 25));
            }
        }
        if let Some(legend) = obj.get_mut("legend") {
            if let Some(l) = legend.as_object_mut() {
                l.insert("right".into(), json!(grid_right + 10));
                l.insert("pageButtonItemGap".into(), json!(2));
                l.insert("pageIconSize".into(), json!(10));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // moving_average
    // -----------------------------------------------------------------------

    #[test]
    fn ma_empty() {
        let result = moving_average(&[], 3);
        assert!(result.is_empty());
    }

    #[test]
    fn ma_shorter_than_window() {
        let result = moving_average(&[1.0, 2.0], 3);
        assert_eq!(result, vec![None, None]);
    }

    #[test]
    fn ma_exact_window() {
        let result = moving_average(&[1.0, 2.0, 3.0], 3);
        assert_eq!(result, vec![None, None, Some(2.0)]);
    }

    #[test]
    fn ma_longer_than_window() {
        let result = moving_average(&[1.0, 2.0, 3.0, 4.0, 5.0], 3);
        assert_eq!(result, vec![None, None, Some(2.0), Some(3.0), Some(4.0)]);
    }

    #[test]
    fn ma_window_1() {
        let data = vec![10.0, 20.0, 30.0];
        let result = moving_average(&data, 1);
        assert_eq!(result, vec![Some(10.0), Some(20.0), Some(30.0)]);
    }

    #[test]
    fn ma_rounds_to_3_decimals() {
        // 1/3 = 0.33333... should round to 0.333
        let result = moving_average(&[0.0, 0.0, 1.0], 3);
        assert_eq!(result[2], Some(0.333));
    }

    // -----------------------------------------------------------------------
    // round
    // -----------------------------------------------------------------------

    #[test]
    fn round_zero_decimals() {
        assert_eq!(round(3.7, 0), 4.0);
        assert_eq!(round(3.4, 0), 3.0);
    }

    #[test]
    fn round_two_decimals() {
        assert_eq!(round(3.456, 2), 3.46);
        assert_eq!(round(3.454, 2), 3.45);
    }

    #[test]
    fn round_negative() {
        assert_eq!(round(-1.555, 2), -1.56);
    }

    #[test]
    fn round_zero() {
        assert_eq!(round(0.0, 5), 0.0);
    }

    // -----------------------------------------------------------------------
    // ts_ms
    // -----------------------------------------------------------------------

    #[test]
    fn ts_ms_conversion() {
        assert_eq!(ts_ms(0), 0);
        assert_eq!(ts_ms(1), 1000);
        assert_eq!(ts_ms(1_700_000_000), 1_700_000_000_000);
    }

    // -----------------------------------------------------------------------
    // block_subsidy (charts/mod.rs version)
    // -----------------------------------------------------------------------

    #[test]
    fn chart_block_subsidy() {
        assert_eq!(block_subsidy(0), 5_000_000_000);
        assert_eq!(block_subsidy(210_000), 2_500_000_000);
        assert_eq!(block_subsidy(840_000), 312_500_000);
        assert_eq!(block_subsidy(13_440_000), 0);
    }

    // -----------------------------------------------------------------------
    // show_ma
    // -----------------------------------------------------------------------

    #[test]
    fn show_ma_threshold() {
        assert!(!show_ma(0));
        assert!(!show_ma(144));
        assert!(!show_ma(199));
        assert!(show_ma(200));
        assert!(show_ma(1000));
    }

    // -----------------------------------------------------------------------
    // format_num
    // -----------------------------------------------------------------------

    #[test]
    fn format_num_thousands() {
        assert_eq!(format_num(0), "0");
        assert_eq!(format_num(999), "999");
        assert_eq!(format_num(1000), "1,000");
        assert_eq!(format_num(1_000_000), "1,000,000");
        assert_eq!(format_num(840_000), "840,000");
    }

    // -----------------------------------------------------------------------
    // dp (data point helper)
    // -----------------------------------------------------------------------

    #[test]
    fn dp_creates_triple() {
        let block = BlockSummary {
            height: 100,
            hash: String::new(),
            timestamp: 1_700_000,
            tx_count: 0,
            size: 0,
            weight: 0,
            difficulty: 0.0,
            total_fees: 0,
            median_fee: 0,
            median_fee_rate: 0.0,
            segwit_spend_count: 0,
            taproot_spend_count: 0,
            p2pk_count: 0,
            p2pkh_count: 0,
            p2sh_count: 0,
            p2wpkh_count: 0,
            p2wsh_count: 0,
            p2tr_count: 0,
            multisig_count: 0,
            unknown_script_count: 0,
            input_count: 0,
            output_count: 0,
            rbf_count: 0,
            witness_bytes: 0,
            inscription_count: 0,
            inscription_bytes: 0,
            inscription_envelope_bytes: 0,
            brc20_count: 0,
            op_return_count: 0,
            op_return_bytes: 0,
            runes_count: 0,
            runes_bytes: 0,
            omni_count: 0,
            omni_bytes: 0,
            counterparty_count: 0,
            counterparty_bytes: 0,
            data_carrier_count: 0,
            data_carrier_bytes: 0,
            taproot_keypath_count: 0,
            taproot_scriptpath_count: 0,
            total_output_value: 0,
            total_input_value: 0,
            fee_rate_p10: 0.0,
            fee_rate_p90: 0.0,
            stamps_count: 0,
            largest_tx_size: 0,
            max_tx_fee: 0,
            inscription_fees: 0,
            runes_fees: 0,
            legacy_tx_count: 0,
            segwit_tx_count: 0,
            taproot_tx_count: 0,
            coinbase_text: String::new(),
            fee_rate_p25: 0.0,
            fee_rate_p75: 0.0,
        };
        let result = dp(&block, 42.5);
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0], 1_700_000_000u64); // ts_ms
        assert_eq!(arr[1], 42.5);
        assert_eq!(arr[2], 100); // height
    }

    // -----------------------------------------------------------------------
    // no_data_chart
    // -----------------------------------------------------------------------

    #[test]
    fn no_data_chart_has_title() {
        let opt = no_data_chart("Test Chart");
        let title = opt.get("title").unwrap();
        let text = title.get("text").unwrap().as_str().unwrap();
        assert!(text.contains("Test Chart"));
        let subtext = title.get("subtext").unwrap().as_str().unwrap();
        assert!(subtext.contains("shorter range"));
    }

    // -----------------------------------------------------------------------
    // build_option merges correctly
    // -----------------------------------------------------------------------

    #[test]
    fn build_option_preserves_defaults() {
        let opt = build_option(json!({
            "xAxis": { "type": "time" }
        }));
        // Should have chart_defaults fields
        assert!(opt.get("toolbox").is_some());
        assert!(opt.get("animation").is_some());
        // Should have our override
        assert_eq!(opt["xAxis"]["type"], "time");
    }

    #[test]
    fn build_option_deep_merges_legend() {
        let opt = build_option(json!({
            "legend": { "show": false }
        }));
        let legend = opt.get("legend").unwrap();
        // Should have "show: false" from our override
        assert_eq!(legend["show"], false);
        // Should still have default textStyle from chart_defaults
        assert!(legend.get("textStyle").is_some());
    }

    // -----------------------------------------------------------------------
    // New chart function tests
    // -----------------------------------------------------------------------

    fn test_block(height: u64, timestamp: u64) -> BlockSummary {
        BlockSummary {
            height,
            hash: format!("h{}", height),
            timestamp,
            tx_count: 2000,
            size: 1_000_000,
            weight: 3_800_000,
            difficulty: 50_000_000_000_000.0,
            total_fees: 50_000_000,
            median_fee: 5000,
            median_fee_rate: 10.5,
            segwit_spend_count: 1500,
            taproot_spend_count: 200,
            p2pk_count: 0,
            p2pkh_count: 500,
            p2sh_count: 300,
            p2wpkh_count: 2000,
            p2wsh_count: 100,
            p2tr_count: 400,
            multisig_count: 10,
            unknown_script_count: 5,
            input_count: 4000,
            output_count: 5000,
            rbf_count: 800,
            witness_bytes: 600_000,
            inscription_count: 50,
            inscription_bytes: 200_000,
            inscription_envelope_bytes: 220_000,
            brc20_count: 5,
            op_return_count: 100,
            op_return_bytes: 10_000,
            runes_count: 30,
            runes_bytes: 3_000,
            omni_count: 2,
            omni_bytes: 200,
            counterparty_count: 1,
            counterparty_bytes: 100,
            data_carrier_count: 67,
            data_carrier_bytes: 6_700,
            taproot_keypath_count: 150,
            taproot_scriptpath_count: 50,
            total_output_value: 500_000_000_000,
            total_input_value: 500_050_000_000,
            fee_rate_p10: 2.0,
            fee_rate_p90: 50.0,
            stamps_count: 0,
            largest_tx_size: 50_000,
            max_tx_fee: 1_000_000,
            inscription_fees: 5_000_000,
            runes_fees: 3_000_000,
            legacy_tx_count: 200,
            segwit_tx_count: 1200,
            taproot_tx_count: 600,
            coinbase_text: String::new(),
            fee_rate_p25: 5.0,
            fee_rate_p75: 25.0,
        }
    }

    #[test]
    fn fee_revenue_share_produces_valid_percentage() {
        let blocks = vec![test_block(840_000, 1713571200)]; // 4th halving block
        let chart = fees::fee_revenue_share_chart(&blocks);
        let series = chart.get("series").unwrap().as_array().unwrap();
        assert!(!series.is_empty());
        // At height 840k: subsidy = 312,500,000 sats, fees = 50,000,000
        // Fee share = 50M / (312.5M + 50M) * 100 = 13.79%
        let data = series[0].get("data").unwrap().as_array().unwrap();
        let val = data[0].as_array().unwrap()[1].as_f64().unwrap();
        assert!(val > 13.0 && val < 14.0, "Expected ~13.79%, got {}", val);
    }

    #[test]
    fn utxo_growth_computes_net_change() {
        let blocks = vec![test_block(100, 1700000000)];
        let chart = tx_metrics::utxo_growth_chart(&blocks);
        let series = chart.get("series").unwrap().as_array().unwrap();
        let data = series[0].get("data").unwrap().as_array().unwrap();
        // output_count(5000) - input_count(4000) = 1000
        let val = data[0].as_array().unwrap()[1].as_i64().unwrap();
        assert_eq!(val, 1000);
    }

    #[test]
    fn tx_density_computes_tx_per_kb() {
        let blocks = vec![test_block(100, 1700000000)];
        let chart = tx_metrics::tx_density_chart(&blocks);
        let series = chart.get("series").unwrap().as_array().unwrap();
        let data = series[0].get("data").unwrap().as_array().unwrap();
        // tx_count(2000) / (size(1000000) / 1000) = 2.0
        let val = data[0].as_array().unwrap()[1].as_f64().unwrap();
        assert!((val - 2.0).abs() < 0.01, "Expected 2.0, got {}", val);
    }

    #[test]
    fn btc_volume_converts_sats_to_btc() {
        let blocks = vec![test_block(100, 1700000000)];
        let chart = fees::btc_volume_chart(&blocks);
        let series = chart.get("series").unwrap().as_array().unwrap();
        let data = series[0].get("data").unwrap().as_array().unwrap();
        // total_output_value = 500_000_000_000 sats = 5000.0 BTC
        let val = data[0].as_array().unwrap()[1].as_f64().unwrap();
        assert_eq!(val, 5000.0);
    }

    #[test]
    fn block_subsidy_at_halvings() {
        assert_eq!(block_subsidy(0), 5_000_000_000); // 50 BTC
        assert_eq!(block_subsidy(209_999), 5_000_000_000); // last block era 0
        assert_eq!(block_subsidy(210_000), 2_500_000_000); // 25 BTC
        assert_eq!(block_subsidy(420_000), 1_250_000_000); // 12.5 BTC
        assert_eq!(block_subsidy(630_000), 625_000_000); // 6.25 BTC
        assert_eq!(block_subsidy(840_000), 312_500_000); // 3.125 BTC
    }

    #[test]
    fn no_data_chart_with_hint_custom_message() {
        let opt = no_data_chart_with_hint("Test", "Custom hint");
        let title = opt.get("title").unwrap();
        assert_eq!(
            title.get("subtext").unwrap().as_str().unwrap(),
            "Custom hint"
        );
    }

    #[test]
    fn histogram_from_buckets_produces_chart() {
        let buckets = vec![
            HistogramBucket {
                label: "0-10%".into(),
                count: 100,
            },
            HistogramBucket {
                label: "90-100%".into(),
                count: 5000,
            },
        ];
        let chart = network::block_fullness_histogram_from_buckets(&buckets);
        let series = chart.get("series").unwrap().as_array().unwrap();
        assert!(!series.is_empty());
    }
}
