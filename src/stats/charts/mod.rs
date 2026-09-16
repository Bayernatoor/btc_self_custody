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
/// Builds every registered chart and checks it behaves the way the registry
/// says it does. Test-only: `shape` and `unit` are declarations, and nearly
/// every rule is derived from them.
#[cfg(test)]
mod conformance;
pub mod embedded;
pub mod fees;
pub mod gauges;
pub mod kpi;
pub mod mining;
pub mod network;
pub mod registry;
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

/// Default right-hand grid margin, in pixels.
///
/// Sized so a centred final x-axis label cannot run off the canvas. Daily
/// charts use a category axis whose labels centre on their tick, and the last
/// tick sits at the grid's right edge, so a 10-character date needs about half
/// its width in clearance. `containLabel` does not provide it: it reserves room
/// for label extents on the axis it measures, not for horizontal overflow of
/// the first and last x labels.
///
/// Measured by SSR-rendering against the ECharts the site loads (5.6.1) over 21
/// combinations of width (700/1050/1400) and range (30 to 6400 days): at 20 the
/// final label was clipped in one case, at 40 in none, and the label count
/// barely moves (250 to 244 across all cases).
///
/// The `unwrap_or(GRID_RIGHT)` fallbacks below read this back out of the
/// option tree, so they must not diverge from it.
pub(crate) const GRID_RIGHT: u64 = 40;

// A 10-character date at the 12px chart font is roughly 61px wide, so a label
// centred on a tick at the grid's right edge overhangs by about 31px. Enforced
// at compile time rather than in a test: both sides are constants, so a runtime
// assertion is a tautology (clippy rejects it), while this fails the build if
// the margin is ever tightened past the point where the final label fits.
const _: () = assert!(GRID_RIGHT >= 31);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Base ECharts option with dark theme defaults (transparent bg, dark grid,
/// toolbox, progressive rendering, attribution watermark).
pub(crate) fn chart_defaults() -> serde_json::Value {
    json!({
        "backgroundColor": "transparent",
        "textStyle": { "color": "#d4d4d4", "fontFamily": "Inter, system-ui, sans-serif" },
        "grid": { "left": 55, "right": GRID_RIGHT, "top": 50, "bottom": 65, "containLabel": true },
        "legend": { "textStyle": { "color": "#e8e8e8", "fontSize": 11 }, "top": 25, "left": "center", "type": "scroll" },
        "toolbox": {
            "feature": {
                "restore": { "title": "Reset zoom" },
                // yAxisIndex "none" restricts the box-select to time. Without
                // it ECharts zooms BOTH axes, and stats.js arms this brush on
                // every desktop chart so a plain drag triggers it. On a 100%
                // stacked chart that silently clips the top bands out of the
                // visible y range: dragging a box on Address Type Share made
                // the P2TR band vanish while the tooltip still reported 20.73%,
                // because the data was never affected, only the view. It also
                // leaves the axis bounded by wherever the drag happened to end,
                // which is why the y axis read 97.63 instead of 100.
                "dataZoom": { "yAxisIndex": "none", "title": { "zoom": "Zoom", "back": "Undo zoom" } }
                // No `saveAsImage`. Every view now has a labelled PNG button
                // in its header, and the toolbox icon sat a centimetre from
                // the header's CSV arrow: two similar glyphs, two formats,
                // neither saying which. The toolbox keeps only what acts on
                // the chart itself, which is zoom, undo and reset.
            },
            "iconStyle": { "borderColor": "#c8c8c8" },
            "emphasis": { "iconStyle": { "borderColor": "#f7931a" } },
            "right": 10, "top": 0,
            "itemSize": 14
        },
        // No "animation" key: stats.js sets `opts.animation = false` on every
        // render before handing the option to ECharts, so anything specified
        // here was dead config that read as if charts animated. Transitions on
        // a 30-chart page were the reason it was disabled; leaving the flag
        // here only misleads.
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
            // 32px, up from 20. The slider is both a control and a preview
            // of the whole series, and at 20px the preview was a smudge and
            // the grab target was thinner than a scrollbar.
            "type": "slider", "start": 0, "end": 100, "height": 32, "bottom": 8,
            "borderColor": "#333", "fillerColor": "rgba(247,147,26,0.15)",
            "handleStyle": { "color": "#f7931a" }, "textStyle": { "color": "#d4d4d4", "fontSize": 10 }
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
        let mut label = json!({
            "color": "#d4d4d4",
            // Thin the labels out rather than letting them collide, which
            // is what produced the odd 9-month gaps between ticks on ALL.
            "hideOverlap": true,
            "formatter": date_label_sentinel(categories),
        });
        if let Some(ticks) = calendar_tick_indices(categories) {
            let o = label
                .as_object_mut()
                .expect("built as an object one line above");
            o.insert("interval".into(), json!(CALENDAR_TICK_SENTINEL));
            o.insert(CALENDAR_TICK_DATA_KEY.into(), json!(ticks));
        }
        json!({
            "type": "category",
            "data": categories,
            "axisLabel": label,
            "axisLine": { "lineStyle": { "color": "#7a7a7a" } }
        })
    } else {
        json!({
            "type": "time",
            "axisLabel": {
                "color": "#d4d4d4",
                "hideOverlap": true,
                // Per level, because a time axis picks its own tick
                // granularity from the span and then formats every level with
                // the same string unless told otherwise. The default prints a
                // bare day number at day level, which reads as a count rather
                // than a date, and a bare hour at hour level. Naming both
                // means a 1D window reads as clock time and a 1W window as
                // dates without the axis having to know which range is in
                // effect.
                "formatter": {
                    "year": "{yyyy}",
                    "month": "{MMM} '{yy}",
                    "day": "{d} {MMM}",
                    "hour": "{HH}:{mm}",
                    "minute": "{HH}:{mm}",
                    "second": "{HH}:{mm}:{ss}",
                    "millisecond": "{HH}:{mm}:{ss}",
                    "none": "{d} {MMM} {HH}:{mm}"
                }
            },
            "axisLine": { "lineStyle": { "color": "#7a7a7a" } }
        })
    }
}

/// Which category indices sit on a calendar boundary worth labelling, or
/// `None` to leave ECharts' own index-based thinning in place.
///
/// `hideOverlap` thins labels by *index*, and the categories are days. An
/// even index step across 6,456 daily categories does not land on even
/// calendar months, so the months walk forward: `Jan '10, Aug '10, Feb '11,
/// Aug '11, Mar '12`. Worse, the step comes from the available width, so
/// enabling an overlay, collapsing the rail or resizing the window moved the
/// labels. They were never calendar-aligned; one step size happened to look
/// like it was.
///
/// So hand ECharts a candidate set that is calendar-aligned by construction.
/// `hideOverlap` stays on and still drops what will not fit, but every
/// candidate is the first plotted day of a month on a fixed stride, so
/// whatever subset survives still reads as clean boundaries, and zooming in
/// reveals more of them rather than shifting the ones already there.
///
/// Returns indices rather than installing a predicate because
/// `axisLabel.interval` only takes one as a JS function, which cannot be
/// serialised from here. Same sentinel handshake as the SI and date
/// formatters: Rust decides, `stats.js` installs.
fn calendar_tick_indices(categories: &[String]) -> Option<Vec<usize>> {
    let span = days_between(categories.first()?, categories.last()?)?;
    // Under this the labels are days, where nobody reads the axis as calendar
    // boundaries and the drift is invisible. Same threshold that switches the
    // label format, so the two decisions cannot disagree.
    if span <= DATE_LABEL_MONTH_YEAR_DAYS {
        return None;
    }
    let months = (span as f64 / 30.44).round() as i64;
    let step = CALENDAR_TICK_STEPS
        .iter()
        .copied()
        .find(|&s| months / s as i64 <= CALENDAR_TICK_TARGET)
        .unwrap_or_else(|| {
            *CALENDAR_TICK_STEPS
                .last()
                .expect("CALENDAR_TICK_STEPS is never empty")
        });

    let mut out = Vec::new();
    let mut prev: Option<(u32, u32)> = None;
    for (idx, c) in categories.iter().enumerate() {
        let Some(ym) = year_month(c) else { continue };
        // First plotted day of its month, so a gap in the data moves the
        // label to the next day that exists instead of losing it.
        let starts_a_month = prev != Some(ym);
        prev = Some(ym);
        if starts_a_month && is_tick_month(ym.0, ym.1, step) {
            out.push(idx);
        }
    }
    // One label is worse than none: it reads as the axis having failed.
    (out.len() >= 2).then_some(out)
}

/// Whether a calendar month falls on the labelling stride.
///
/// Anchored to January, and for multi-year strides to years divisible by the
/// stride, rather than to wherever the data happens to begin. That way the
/// same chart viewed over two different ranges labels the same months, and a
/// range whose first day is mid-month does not shift every label after it.
fn is_tick_month(year: u32, month: u32, step_months: u32) -> bool {
    if step_months >= 12 {
        return month == 1 && year.is_multiple_of(step_months / 12);
    }
    (month - 1).is_multiple_of(step_months)
}

/// Year and month from a `YYYY-MM-DD` category label.
///
/// Parsed directly rather than through `chrono`, because the day is not
/// needed and a category that is not a date at all (the weekday and
/// fee-bucket axes use words) must fall through rather than be rejected as an
/// error.
fn year_month(date: &str) -> Option<(u32, u32)> {
    let month: u32 = date.get(5..7)?.parse().ok()?;
    if !(1..=12).contains(&month) || date.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    Some((date.get(0..4)?.parse().ok()?, month))
}

/// Roughly how many candidate labels to emit across the span.
///
/// Deliberately more than fit. `hideOverlap` thins them to the width
/// available, so this only has to be dense enough that a zoomed-in window
/// still has labels in it, and sparse enough that the thinning is not doing
/// all the work.
const CALENDAR_TICK_TARGET: i64 = 30;

/// Strides in months, coarsest last. Each is a boundary a reader recognises,
/// which is why the ladder skips 4, 5 and 8: nobody reads "every five months"
/// as a calendar step.
const CALENDAR_TICK_STEPS: &[u32] = &[1, 2, 3, 6, 12, 24, 60];

/// Marks a category axis whose label interval `stats.js` should install from
/// [`CALENDAR_TICK_DATA_KEY`]. Must match the constant of the same name there.
pub(crate) const CALENDAR_TICK_SENTINEL: &str = "__calendar_ticks__";

/// Where the indices ride until `stats.js` turns them into a predicate and
/// deletes the key. Not an ECharts option; the double underscore marks it as
/// ours to anyone reading a serialised option.
pub(crate) const CALENDAR_TICK_DATA_KEY: &str = "__calendarTicks";

/// Which date format the category axis should render, as a sentinel for
/// `stats.js` to swap for a real formatter.
///
/// The axis `data` stays as `YYYY-MM-DD` and is never rewritten: the key
/// figures read it back to date the peak and low, and the CSV export takes
/// its first column from it. Only the *label* changes.
///
/// Format chosen from the span rather than from the range name, because the
/// builder is handed a slice and not a range, and because a custom window can
/// be any length. Sixteen years of history as `2009-10-05` at nine-month
/// intervals is unreadable; `Jul '09` is not.
fn date_label_sentinel(categories: &[String]) -> &'static str {
    let span_days = match (categories.first(), categories.last()) {
        (Some(a), Some(b)) => days_between(a, b).unwrap_or(0),
        _ => 0,
    };
    if span_days > DATE_LABEL_MONTH_YEAR_DAYS {
        DATE_LABEL_MONTH_YEAR
    } else {
        DATE_LABEL_DAY_MONTH
    }
}

/// Span above which the category axis labels months rather than days.
///
/// Two years is where day-level labels stop being legible: beyond it there
/// are too many to thin down to something evenly spaced. Shared with
/// [`calendar_tick_indices`], which only aligns ticks to the calendar once
/// the labels name months, so the two cannot disagree about which regime the
/// axis is in.
const DATE_LABEL_MONTH_YEAR_DAYS: i64 = 730;

fn days_between(from: &str, to: &str) -> Option<i64> {
    let a = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d").ok()?;
    let b = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d").ok()?;
    Some((b - a).num_days())
}

/// `2009-07-15` renders as `Jul '09`. Must match `stats.js`.
pub(crate) const DATE_LABEL_MONTH_YEAR: &str = "__date_month_year__";
/// `2026-06-13` renders as `13 Jun`. Must match `stats.js`.
pub(crate) const DATE_LABEL_DAY_MONTH: &str = "__date_day_month__";

/// Y-axis config with name label, dashed grid lines, and dark theme styling.
pub(crate) fn y_axis(name: &str) -> serde_json::Value {
    json!({
        "type": "value",
        "name": name,
        "nameTextStyle": { "color": "#d4d4d4" },
        "axisLabel": { "color": "#d4d4d4" },
        "axisLine": { "lineStyle": { "color": "#7a7a7a" } },
        "splitLine": { "lineStyle": { "color": "rgba(255,255,255,0.28)", "type": "dashed" } }
    })
}

/// Switch the left value axis to a logarithmic scale.
///
/// Only the first y axis: the second belongs to the price or chain-size
/// overlay, and rescaling someone else's axis underneath them is how the
/// clipping bug in `fix/chart-zoom-axis` happened. A category axis is left
/// alone, since there is nothing to rescale.
///
/// ECharts does not so much drop a non-positive point on a log axis as fail to
/// place it, so switching also nulls the points the axis cannot plot and
/// reports how many; see [`drop_non_positive`]. Which charts are offered the
/// switch at all is `registry::ChartMeta::supports_log`.
///
/// **Apply once, to a freshly built option.** Sanitising is destructive, so a
/// second application has nothing left to count and would clear the notice
/// while leaving the gap. That is how the chart views work already: every
/// render rebuilds the option from the rows and decorates it once.
pub fn apply_log_scale(option: &mut serde_json::Value, on: bool) {
    // Counted before the axis is switched, because switching it nulls the
    // points being counted. Getting this backwards leaves the gap in the line
    // with nothing to explain it.
    let dropped = if on && log_scale_is_meaningful(option, 0) {
        non_positive_count(option, 0)
    } else {
        0
    };
    apply_axis_log(option, 0, on);
    // Reconciled unconditionally, so switching back to linear clears a notice
    // the log view left behind. Doing it inside the axis block meant the
    // `!on` path returned before reaching it, and the warning stayed on a
    // linear chart.
    //
    // Left-axis only, which is why `apply_scales_with` exists: it counts both
    // axes and writes one notice covering them.
    set_log_notice(option, dropped);
}

/// Switch one value axis, and make its own series safe to draw there.
///
/// Per axis, which is the part that was wrong. Eligibility was decided from
/// **every** series in the option, so a bar comparison landing on the right
/// axis disabled the metric's own log scale: Difficulty with a `max-tx-fee`
/// comparison reported Log as selected and rendered linear, because the
/// refusal that exists to stop bars running from a log floor was being applied
/// to an axis that had no bars on it. The right axis meanwhile applied no
/// refusal at all, so the bars it did hold were free to do exactly that.
fn apply_axis_log(option: &mut serde_json::Value, axis_idx: u64, on: bool) {
    if on && !log_scale_is_meaningful(option, axis_idx) {
        return;
    }
    // Read before the axis is borrowed mutably: both come from the series
    // alongside it.
    let extent = positive_extent(option, axis_idx);
    set_axis_scale(option, axis_idx, on, extent);
    if on {
        drop_non_positive(option, axis_idx);
    }
}

/// Switch the right value axis, the one an overlay owns, to a logarithmic
/// scale independently of the metric's own axis.
///
/// Separate from [`apply_log_scale`] because the two axes carry unrelated
/// quantities and a reader wants them scaled independently: price over ALL is
/// unreadable on a linear axis whatever the metric beside it is doing, and
/// forcing both to log to get that is how the metric ends up on a scale
/// nobody asked for. Glassnode calls the combination "Mixed"; here it is just
/// two switches, which says which axis each one moves.
///
/// No shape refusals, unlike the left axis: the right axis holds one line,
/// never a stack or a percentage band. A chart with no right axis is a no-op,
/// which is what makes this safe to call unconditionally after the overlays.
///
/// It **does** need the dropped-point notice, and that is worth spelling out
/// because this function shipped without one. The reasoning was that price
/// and chain size are the only two series that ever claim this axis and both
/// are strictly positive. True when written, and `apply_comparison` broke it
/// three hours later: any comparable chart can now claim the right axis,
/// including `utxo-growth`, whose own copy says it goes negative. The notice
/// is written by [`apply_scales`], which is the only place that knows what
/// both axes dropped.
///
/// The lesson is about the comment rather than the code. An invariant
/// justified by naming today's callers is a note that expires silently.
pub fn apply_right_log_scale(option: &mut serde_json::Value, on: bool) {
    if !has_right_value_axis(option) {
        return;
    }
    apply_axis_log(option, RIGHT_AXIS_IDX, on);
}

/// Apply both axis scales in the order they depend on, and reconcile the one
/// notice that covers them both.
///
/// The single entry point the chart views call, so neither has to remember
/// that the right axis only exists after the overlays have run, that the left
/// one refuses on some shapes, or that a log axis cannot plot zero.
///
/// One notice rather than one per axis: it is a `graphic` element pinned to
/// the top of the plot, so two would overlap. The count is the total across
/// both, which is what a reader needs to know, and the wording does not name
/// an axis for that reason.
pub fn apply_scales(option: &mut serde_json::Value, flags: &OverlayFlags) {
    apply_scales_with(option, flags.log_scale, flags.right_log_scale)
}

/// [`apply_scales`] with the two switches passed separately.
///
/// The single-chart view needs this because its left-axis switch is its own
/// page state gated on `supports_log`, not the shared overlay flag, and it was
/// therefore calling the two axis functions itself and getting a notice that
/// counted only one of them.
///
/// **The counting has to happen first.** Switching an axis to log now nulls
/// the points that axis cannot plot, so counting afterwards counts nothing and
/// the notice explaining the gap disappears with the gap still there.
pub fn apply_scales_with(
    option: &mut serde_json::Value,
    left: bool,
    right: bool,
) {
    let dropped = if left && log_scale_is_meaningful(option, 0) {
        non_positive_count(option, 0)
    } else {
        0
    } + if right
        && has_right_value_axis(option)
        && log_scale_is_meaningful(option, RIGHT_AXIS_IDX)
    {
        non_positive_count(option, RIGHT_AXIS_IDX)
    } else {
        0
    };
    apply_axis_log(option, 0, left);
    if has_right_value_axis(option) {
        apply_axis_log(option, RIGHT_AXIS_IDX, right);
    }
    set_log_notice(option, dropped);
}

/// The axis index an overlay's own series is plotted against.
///
/// `add_series_overlay` pushes the overlay axis onto the end of `yAxis`, so
/// with one overlay it is index 1. With two (price and chain size) the second
/// lands at index 2 and keeps its own linear scale; treating only index 1 as
/// the overlay axis is a deliberate simplification, since the pair is rare
/// and a third scale switch would cost more in UI than it buys.
const RIGHT_AXIS_IDX: u64 = 1;

/// Whether an overlay has added a right-hand value axis to rescale.
///
/// Both scale types count. Asking only for `value` meant that once the axis
/// had been switched to `log` this refused to recognise it, so the switch
/// back to linear did nothing and the axis was stuck. A category axis is the
/// real exclusion: there is no scale there to change.
fn has_right_value_axis(option: &serde_json::Value) -> bool {
    matches!(
        option
            .get("yAxis")
            .and_then(|a| a.as_array())
            .and_then(|a| a.get(RIGHT_AXIS_IDX as usize))
            .and_then(|a| a.get("type"))
            .and_then(|t| t.as_str()),
        Some("value" | "log")
    )
}

/// Set a value axis's type, and its bounds when logarithmic.
fn set_axis_scale(
    option: &mut serde_json::Value,
    axis_idx: u64,
    on: bool,
    extent: Option<(f64, f64)>,
) {
    let Some(axis) = option.get_mut("yAxis") else {
        return;
    };
    // `yAxis` is an object on single-axis charts and an array once an overlay
    // has added the right-hand one.
    let target = match axis {
        serde_json::Value::Array(a) => a.get_mut(axis_idx as usize),
        other if axis_idx == 0 => Some(other),
        _ => None,
    };
    let Some(serde_json::Value::Object(o)) = target else {
        return;
    };
    if o.get("type").and_then(|t| t.as_str()) == Some("category") {
        return;
    }
    o.insert(
        "type".to_string(),
        serde_json::Value::String(if on { "log" } else { "value" }.to_string()),
    );

    if !on {
        // Remove only what the log path added.
        //
        // This used to strip `min` and `max` unconditionally, on a comment
        // claiming it handed the axis back as the builder configured it. It
        // did the opposite: a percentage chart declares `max: 100` so its
        // bands are read against a fixed frame, and that was being deleted on
        // every render with log off, which is the default. Hiding one band
        // then let the axis refit to the remainder, so a 90% band vanishing
        // rescaled the chart from 0-100 to 0-10. That reaches the existing
        // multi-chart pages, not just this view.
        //
        // The marker is how the two are told apart. Bounds carrying it came
        // from here; anything else belongs to the builder and is left alone.
        if o.remove(LOG_BOUNDS_MARKER).is_some() {
            o.remove("min");
            o.remove("max");
        }
        if o.remove(LOG_FORMATTER_MARKER).is_some() {
            if let Some(label) =
                o.get_mut("axisLabel").and_then(|l| l.as_object_mut())
            {
                label.remove("formatter");
            }
        }
        o.remove("minorTick");
        o.remove("minorSplitLine");
        return;
    }

    // Bounds come from the data, not from decade boundaries.
    //
    // Left to itself ECharts picks decades from its split count, which on
    // difficulty over ALL put the top tick eight decades above the real
    // maximum. Rounding out to the enclosing decades fixes that case and
    // ruins narrow ones: over 1Y difficulty spans 140 T to 160 T, which
    // rounds to a 100-to-1000 axis with every point pinned to the floor.
    //
    // The data extent with a small multiplicative pad is right at both ends
    // of that scale, because on a log axis a ratio is the same shape wherever
    // it sits. A narrow range then looks close to linear, which is the honest
    // rendering of narrow data rather than a defect.
    if let Some((lo, hi)) = extent {
        // Degenerate case: one distinct value, where any ratio-based pad
        // collapses. Give it a decade either side so the line has somewhere
        // to sit instead of an axis of zero height.
        let (lo, hi) = if hi / lo < 1.000_001 {
            (lo / 10.0, hi * 10.0)
        } else {
            (lo * (1.0 - LOG_AXIS_PAD), hi * (1.0 + LOG_AXIS_PAD))
        };
        // Rounded, because ECharts prints an explicit min and max verbatim as
        // axis labels. The raw padded values are floats off the data, so the
        // axis read "159.09249284" and "122.4342086864" while the linear
        // version of the same chart showed 0, 30, 60, 90, 120, 150. Three
        // significant figures keeps the fit tight and the label legible.
        o.insert("min".to_string(), json!(round_sig(lo, 3, RoundDir::Down)));
        o.insert("max".to_string(), json!(round_sig(hi, 3, RoundDir::Up)));
        // No `splitNumber`, because measurement says it does nothing here.
        //
        // Rendered in Chrome 153 against ECharts 5.6.0, this axis produces the
        // same three labels with `splitNumber: 8` and without it, at every
        // height from 200px to 3200px. An earlier version of this comment
        // asserted that 8 splits across 7.36 decades placed ticks at
        // fractional powers of ten which ECharts then declined to label. That
        // was a plausible mechanism and it is false; removing the hint is
        // tidying, not a fix.
        //
        // What actually governs the label count is whether the **bounds are
        // exact powers of the base**. Measured, same span either way:
        //
        //     min 1, max 1e8            -> 9 labels, every decade
        //     min 0.000285, max 2.29e7  -> 3 labels: min, 1, max
        //
        // So the sparse axis is a direct cost of fitting bounds to the data,
        // which is deliberate and is the right trade for a narrow range: over
        // 1Y difficulty spans 140T to 160T, under one decade, and rounding out
        // to enclosing decades pins every point to the floor. Confirmed by the
        // same probe, where that range yields two labels whatever is done.
        //
        // A span-dependent rule, fitting narrow ranges and aligning wide ones,
        // would get both and is recorded as chart-quality work rather than
        // done here. ECharts 5 offers no explicit tick list for a value or log
        // axis, so that is the only lever.
        //
        // Evidence: notes/chart-quality-2026-09-15/runs/axis-label-probe.md

        // Guarantee a formatter, because ECharts prints an explicit bound
        // verbatim and its own idea of the bound is not the number we set.
        //
        // `round_sig` hands over exactly 22,900,000, and the axis rendered
        // "22,899,999.9999999739": log extents are held as exponents, so the
        // value comes back as 10^log10(22900000), which is 22900000.00000002,
        // and then prints in full. Rust cannot round its way out of that. The
        // abbreviating formatter reads it as 22.9M and is what `push_right_axis`
        // has always installed; a log axis needs it just as much, and the
        // builders that never asked for it are exactly the ones that showed
        // the raw float.
        //
        // Only where the builder has not chosen its own, since a percentage or
        // unit-suffixed formatter is a deliberate choice.
        let has_formatter = o
            .get("axisLabel")
            .and_then(|l| l.get("formatter"))
            .is_some();
        if !has_formatter {
            match o.get_mut("axisLabel").and_then(|l| l.as_object_mut()) {
                Some(label) => {
                    label.insert(
                        "formatter".to_string(),
                        json!(SI_AXIS_SENTINEL),
                    );
                }
                None => {
                    o.insert(
                        "axisLabel".to_string(),
                        json!({
                            "color": "#d4d4d4",
                            "formatter": SI_AXIS_SENTINEL
                        }),
                    );
                }
            }
            o.insert(LOG_FORMATTER_MARKER.to_string(), json!(true));
        }
        // So the switch back knows these bounds are ours to remove.
        o.insert(LOG_BOUNDS_MARKER.to_string(), json!(true));
        // Minor ticks are the only way to give a log axis structure between
        // its labels, because ECharts labels a log axis at powers of the base
        // and nowhere else. `splitNumber` above is a hint it can only honour
        // by choosing how many decades to step; inside a decade it has no
        // tick to offer.
        //
        // That falls apart on a span under one decade, which is common for a
        // comparison series: UTXO flow over 10Y runs 1.9K to 11K, so the only
        // power of ten inside the range is 10K and the axis draws three
        // labels, two of them the bounds. Minor split lines put a rule at 2,
        // 3, 4 and so on within each decade, so the reader can place a value
        // even where there is no label to read.
        o.insert("minorTick".to_string(), json!({ "show": true }));
        o.insert(
            "minorSplitLine".to_string(),
            json!({
                "show": true,
                "lineStyle": {
                    "color": "rgba(255,255,255,0.06)",
                    "type": "dotted"
                }
            }),
        );
    }
}

/// Replace the points a log axis cannot plot with `null`.
///
/// ECharts does not skip a non-positive point on a log axis so much as fail to
/// place it, and the damage is not confined to the point: on Chain Size over
/// ALL, whose first readings are fractions of a gigabyte, the filled area
/// under the line was drawn as a straight diagonal wedge from the bottom left
/// corner to the top right. Seven decades of real curve with a triangle
/// pasted over it, and nothing in the data corresponds to that edge.
///
/// A `null` is a gap, which ECharts does handle, so the series simply starts
/// where it becomes plottable. The count is taken before this runs and shown
/// as a notice, so the reader is told how many readings are missing rather
/// than left to infer it.
///
/// This also settles a disagreement the rail could not win. `kpi` reads its
/// figures back out of the built option precisely so they describe the series
/// that was drawn; with the points still present it reported a low of zero for
/// a chart whose axis could not reach zero. Now both sides see the same
/// series.
fn drop_non_positive(option: &mut serde_json::Value, axis_idx: u64) {
    let Some(series) = option.get_mut("series").and_then(|s| s.as_array_mut())
    else {
        return;
    };
    for s in series.iter_mut() {
        if series_axis(s) != axis_idx {
            continue;
        }
        let Some(data) = s.get_mut("data").and_then(|d| d.as_array_mut())
        else {
            continue;
        };
        for point in data.iter_mut() {
            match point {
                // Daily builders emit bare numbers positioned by the category
                // axis, so the slot has to stay occupied: removing it would
                // shift every later point one place to the left.
                serde_json::Value::Number(n) => {
                    if n.as_f64().is_some_and(|y| y <= 0.0) {
                        *point = serde_json::Value::Null;
                    }
                }
                // Per-block builders emit [ts, value, height]. Only the value
                // is nulled, so the tooltip still knows which block it was.
                serde_json::Value::Array(a)
                    if a.get(1)
                        .and_then(|y| y.as_f64())
                        .is_some_and(|y| y <= 0.0) =>
                {
                    a[1] = serde_json::Value::Null;
                }
                _ => {}
            }
        }
    }
}

/// Which y axis a series is plotted against. Absent means the first one,
/// which is what ECharts assumes and what every single-axis builder relies on.
fn series_axis(s: &serde_json::Value) -> u64 {
    s.get("yAxisIndex").and_then(|i| i.as_u64()).unwrap_or(0)
}

/// How many of an axis's points a log scale cannot plot.
fn non_positive_count(option: &serde_json::Value, axis_idx: u64) -> usize {
    let Some(series) = option.get("series").and_then(|s| s.as_array()) else {
        return 0;
    };
    series
        .iter()
        .filter(|s| series_axis(s) == axis_idx)
        .flat_map(|s| {
            s.get("data")
                .and_then(|d| d.as_array())
                .map(|a| a.as_slice())
                .unwrap_or_default()
        })
        .filter(|v| {
            let y = match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::Array(a) => {
                    a.get(1).and_then(|y| y.as_f64())
                }
                _ => None,
            };
            matches!(y, Some(y) if y <= 0.0)
        })
        .count()
}

/// Say so, in the chart, when a log axis cannot plot every point.
///
/// Written into the option rather than returned to the caller so it reaches
/// every view for free: the multi-chart cards, the single-chart page and the
/// PNG export all render the same option and none of them need to know this
/// rule exists. The alternative, returning a status for each view to render,
/// would have to be plumbed through `chart_memo!` and then remembered at
/// every call site.
///
/// Zero and negative are legitimate values here (fees on an empty block, net
/// UTXO growth), so this is a normal state to be in, not an error.
fn set_log_notice(option: &mut serde_json::Value, dropped: usize) {
    let Some(graphic) = option.get_mut("graphic") else {
        return;
    };
    let Some(items) = graphic.as_array_mut() else {
        return;
    };
    // Idempotent: re-applying must not stack notices, since the option is
    // rebuilt and re-decorated on every range and overlay change.
    items.retain(|g| {
        g.get("id").and_then(|i| i.as_str()) != Some(LOG_NOTICE_ID)
    });
    if dropped == 0 {
        return;
    }
    let text = if dropped == 1 {
        "1 point is zero or negative and cannot be shown on a log axis"
            .to_string()
    } else {
        format!(
            "{dropped} points are zero or negative and cannot be shown on a log axis"
        )
    };
    items.push(json!({
        "id": LOG_NOTICE_ID,
        "type": "text",
        "left": "center",
        "top": 4,
        "silent": true,
        "z": 10,
        "style": {
            "text": text,
            "fill": "#f0d9a8",
            "font": "11px Inter, system-ui, sans-serif",
            "textAlign": "center"
        }
    }));
}

/// Marks the notice so it can be removed rather than duplicated on re-apply.
const LOG_NOTICE_ID: &str = "log-scale-notice";

/// Marks an axis whose bounds this module set, so switching back to linear
/// removes those and not the builder's own. Not an ECharts option; the double
/// underscore marks it as ours to anyone reading a serialised option.
const LOG_BOUNDS_MARKER: &str = "__logBounds";

/// Marks a series that accompanies another rather than measuring something of
/// its own, so the key figures and the comparison lift can skip it.
///
/// A **declared** role, which is the point. The existing test for this is
/// `kpi::is_moving_average`, matching " ma" and "moving average" in a series
/// name, and it works only because every smoothing series happens to be named
/// that way. Chain Size's "Disk Size (est.)" is the same kind of thing, block
/// data multiplied by today's storage overhead, and no naming convention
/// covers it: the rail read the two series as separate measurements and
/// reported an average across a quantity and its own rescaling.
///
/// Extending the name matcher to catch "(est.)" would have been a second
/// guess of the same kind. A builder knows which of its series is the
/// measurement, so it says so. This is the narrow form of the declared roles
/// phase 2 generalises; see `notes/phase-2-spec.md`.
pub(crate) const COMPANION_MARKER: &str = "__companion";

/// Marks an axis whose SI label formatter this module installed, so switching
/// back to linear removes it and leaves a builder's own formatter alone.
const LOG_FORMATTER_MARKER: &str = "__logFormatter";

/// Which way `round_sig` breaks, so a bound never crops the data it is meant
/// to contain: the lower bound rounds down, the upper rounds up.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RoundDir {
    Down,
    Up,
}

/// Round to `digits` significant figures, away from the data.
///
/// Significant figures rather than decimal places because a log axis spans
/// orders of magnitude: the same chart can need 0.221 at one end and 159 at
/// the other, and a fixed number of decimals is wrong at one of them.
fn round_sig(v: f64, digits: i32, dir: RoundDir) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    let mag = v.abs().log10().floor();
    let scale = 10f64.powi(digits - 1 - mag as i32);
    let scaled = v * scale;
    let snapped = match dir {
        RoundDir::Down => scaled.floor(),
        RoundDir::Up => scaled.ceil(),
    };
    // Multiply by the reciprocal power rather than dividing by `scale`.
    //
    // For a value in the tens of millions `scale` is 1e-5, and 229 / 1e-5 is
    // 22899999.999999996, not 22900000. ECharts prints an explicit max
    // verbatim, so the axis label read "22,899,999.999999996" where three
    // significant figures were the entire point of rounding.
    //
    // The same class of defect this function was written to fix, one layer
    // down: it stopped the raw padded float reaching the label and then
    // produced its own.
    let rounded = snapped * 10f64.powi(mag as i32 - (digits - 1));
    // Clamped, because neither form of the scaling is exact. Dividing by a
    // fractional scale turned 22,900,000 into 22,899,999.999999996, and
    // multiplying by the reciprocal turns 0.00123 into
    // 0.0012300000000000002. The first printed a nonsense axis label; the
    // second breaks the promise this function exists for, that a floor never
    // rises above the data it is meant to contain.
    //
    // So round, then refuse to cross the input. Where the float cooperates
    // the bound is exact to the significant figures asked for; where it does
    // not, the bound is the original value, which is a worse label and a
    // correct one.
    match dir {
        RoundDir::Down => rounded.min(v),
        RoundDir::Up => rounded.max(v),
    }
}

/// Breathing room above and below the plotted range on a log axis, as a
/// fraction. Multiplicative rather than absolute, since that is scale
/// invariant and a log axis is all ratios.
const LOG_AXIS_PAD: f64 = 0.02;

/// Smallest and largest strictly positive value plotted on the left axis,
/// which is what a log axis can actually show.
fn positive_extent(
    option: &serde_json::Value,
    axis_idx: u64,
) -> Option<(f64, f64)> {
    let series = option.get("series")?.as_array()?;
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for s in series {
        if series_axis(s) != axis_idx {
            continue;
        }
        let Some(data) = s.get("data").and_then(|d| d.as_array()) else {
            continue;
        };
        for v in data {
            let y = match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::Array(a) => {
                    a.get(1).and_then(|y| y.as_f64())
                }
                _ => None,
            };
            if let Some(y) = y.filter(|y| *y > 0.0) {
                lo = lo.min(y);
                hi = hi.max(y);
            }
        }
    }
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

/// Whether a log axis would say something true about this chart.
///
/// Decided from the built option rather than from chart metadata, so one
/// global toggle can be applied to every chart without each of the 53
/// `chart_memo!` call sites having to declare its shape. Three refusals:
///
/// - **Stacked series.** The bands are read as a sum, and a log axis makes the
///   stack's heights mean nothing.
/// - **A percentage axis**, bounded to 100. Already a bounded scale; a log
///   version only compresses the top.
/// - **Any non-positive plotted value.** This is the one that silently lies:
///   ECharts drops zero and negative points on a log axis rather than
///   erroring, so a chart with a single zero would quietly lose it. Several
///   here legitimately hit zero (empty blocks, fees on an empty block) and
///   `utxo-growth` is net, so it goes negative.
fn log_scale_is_meaningful(option: &serde_json::Value, axis_idx: u64) -> bool {
    let Some(all) = option.get("series").and_then(|s| s.as_array()) else {
        return false;
    };
    // This axis's own series, not every series in the option. A comparison
    // draws on the right axis and has no say in whether the left one can be
    // logarithmic, and vice versa.
    let series: Vec<&serde_json::Value> =
        all.iter().filter(|s| series_axis(s) == axis_idx).collect();
    if series.is_empty() {
        return false;
    }
    if series.iter().any(|s| s.get("stack").is_some()) {
        return false;
    }
    // Bars, for the reason `fit_value_axis` already refuses them: a bar's
    // length is read from zero, and a log axis has no zero. Every bar then
    // runs from the axis floor to its value, so on max-tx-fee with a floor of
    // 0.00001 and values to 0.379 the whole plot filled solid and the shape
    // of the data disappeared.
    //
    // The reasoning was written for axis fitting and never applied here,
    // where it matters more: fitting only rescales, this makes the chart
    // unreadable.
    if series
        .iter()
        .any(|s| s.get("type").and_then(|t| t.as_str()) == Some("bar"))
    {
        return false;
    }
    let bounded_to_100 = |a: &serde_json::Value| {
        a.get("max").and_then(|m| m.as_f64()) == Some(100.0)
    };
    // Also this axis's own bound. A percentage metric beside a comparison on
    // an unbounded right axis must still refuse, and must not make the
    // comparison refuse with it.
    let percent_axis = match option.get("yAxis") {
        Some(serde_json::Value::Array(a)) => {
            a.get(axis_idx as usize).is_some_and(bounded_to_100)
        }
        Some(other) if axis_idx == 0 => bounded_to_100(other),
        _ => false,
    };
    if percent_axis {
        return false;
    }
    // Non-positive points are deliberately NOT a refusal. ECharts cannot plot
    // zero or negative on a log axis, but the honest response is to switch
    // and say so rather than to ignore the request: `set_log_notice` writes a
    // notice into the option. Glassnode does the same, and a control that
    // silently does nothing is worse than one that explains.
    //
    // Deliberately no minimum span either. A narrow range renders almost
    // identically to linear, which is the correct outcome rather than a
    // reason to refuse: the reader asked for a log axis and gets one, at 7d
    // as at ALL. An earlier version refused under two decades, which only
    // papered over decade-rounded bounds that were themselves the bug.
    //
    // So the only hard refusals left are the two where a log axis would be
    // actively misleading rather than merely uninformative: stacked bands and
    // a bounded percentage axis.
    true
}

/// A value axis whose labels are abbreviated with SI suffixes by the client.
///
/// For quantities that span many orders of magnitude and have no natural
/// scaled unit. Difficulty is the case that forced it: it runs from 1 at
/// genesis to 1.6e14 today, so no fixed divisor works. Dividing by 1e12 and
/// calling the axis "T" put every historical value below 1, where ECharts
/// falls back to full decimal notation and the floor label rendered as
/// "0.0000000001".
///
/// Block size in MB, chain size in GB and inscription payloads in KB do not
/// need this: each spans three or four decades and stays above ~0.001, so a
/// fixed unit reads naturally.
///
/// The formatter is a sentinel string rather than a function because this
/// option is serialised from Rust, where a JS function cannot be expressed.
/// `stats.js` swaps it for a real one; see `SI_AXIS_SENTINEL` there.
pub(crate) fn y_axis_si(name: &str) -> serde_json::Value {
    let mut axis = y_axis(name);
    if let Some(o) = axis.as_object_mut() {
        o.insert(
            "axisLabel".to_string(),
            json!({ "color": "#d4d4d4", "formatter": SI_AXIS_SENTINEL }),
        );
    }
    axis
}

/// Marks an axis whose labels `stats.js` should abbreviate. Must match the
/// constant of the same name there.
pub(crate) const SI_AXIS_SENTINEL: &str = "__si_suffix__";

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
    if window == 0 {
        return vec![None; data.len()];
    }
    let mut result = Vec::with_capacity(data.len());
    for i in 0..data.len() {
        if i < window.saturating_sub(1) {
            result.push(None);
        } else {
            let start = i + 1 - window;
            let sum: f64 = data[start..=i].iter().sum();
            let avg = sum / window as f64;
            result.push(Some(round_plot(avg)));
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
        // `null` for anything JSON cannot hold. These arrays are assembled
        // as text and parsed afterwards, and `write!` will happily emit `NaN`
        // or `inf` for an f64. One of those makes the whole array
        // unparseable, so `data_array_value` returns an empty one and the
        // chart renders blank, silently, with no clue which value did it.
        //
        // Writing null instead turns that into a gap at the one point that
        // had no value, which is both survivable and the correct rendering.
        // Guarding here rather than at each call site is what makes the whole
        // class unreachable: the batching chart hit it by returning NaN for a
        // block with no transactions to average over, and 90,000 blocks
        // qualify.
        let v = value_fn(b);
        let _ = if v.is_finite() {
            write!(buf, "[{},{},{}]", ts_ms(b.timestamp), v, b.height)
        } else {
            write!(buf, "[{},null,{}]", ts_ms(b.timestamp), b.height)
        };
    }
    buf.push(']');
    buf
}

/// Build a JSON array of [timestamp_ms, value, height], writing `null` where
/// there is nothing to measure.
///
/// A share-of-total chart has no meaningful value for a block with no
/// population to divide: an empty block carries only the coinbase, so it has no
/// inputs to classify. Writing `0` there is not a smaller value, it is a
/// different claim — on a stacked percentage chart three zeros collapse the
/// stack to the baseline and read as missing data. `null` leaves a gap, which
/// is what actually happened.
pub(crate) fn build_data_array_opt_f64(
    blocks: &[BlockSummary],
    value_fn: impl Fn(&BlockSummary) -> Option<f64>,
) -> String {
    let mut buf = String::with_capacity(blocks.len() * 30);
    buf.push('[');
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            buf.push(',');
        }
        let ts = ts_ms(b.timestamp);
        let _ = match value_fn(b) {
            Some(v) => write!(buf, "[{},{},{}]", ts, v, b.height),
            None => write!(buf, "[{},null,{}]", ts, b.height),
        };
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

/// Convert a pre-built JSON array string into a `serde_json::Value` for
/// embedding in chart option objects.
///
/// The fallback on malformed input is an empty array, which renders as a blank
/// chart. That failure is otherwise completely silent: no console error, no
/// server log, just a chart with no data and no explanation. Since these arrays
/// are hand-written by `build_data_array_*` rather than serialised by serde, a
/// formatting bug there is exactly the kind of thing this hides, so log before
/// swallowing it.
pub(crate) fn data_array_value(raw: &str) -> serde_json::Value {
    match serde_json::from_str(raw) {
        Ok(value) => value,
        Err(e) => {
            // leptos::logging routes to console.error in the browser and to
            // stderr under SSR, so this surfaces in both builds without
            // pulling in web-sys or a cfg split.
            leptos::logging::error!(
                "chart data array failed to parse ({e}); chart will render empty. First 120 chars: {}",
                raw.chars().take(120).collect::<String>()
            );
            json!([])
        }
    }
}

/// Round to N decimal places.
pub(crate) fn round(val: f64, decimals: u32) -> f64 {
    let factor = 10f64.powi(decimals as i32);
    (val * factor).round() / factor
}

/// Moving average over a series that has gaps, keeping the window's meaning.
///
/// Take the chronological window first, then average whatever readings are
/// inside it. The obvious alternative, filtering the gaps out and averaging
/// the survivors, silently redefines the window: "7-day MA" becomes "the mean
/// of the last seven days that had data", which can reach arbitrarily far
/// back. Measured on a series with one reading of 100, then eight gaps, then
/// six zeroes, that produced **14.2857** at the final position by pulling the
/// 100 in from fourteen positions away, where the last seven positions hold
/// nothing but zeroes and the honest answer is **0**.
///
/// `None` where the window holds no reading at all, since an average of
/// nothing is not zero. A window holding some readings averages those, which
/// is the standard treatment and keeps the line continuous across short gaps
/// without inventing values for them.
pub(crate) fn moving_average_over_gaps(
    readings: &[Option<f64>],
    window: usize,
) -> Vec<Option<f64>> {
    if window == 0 {
        return vec![None; readings.len()];
    }
    (0..readings.len())
        .map(|i| {
            let lo = (i + 1).saturating_sub(window);
            let present: Vec<f64> =
                readings[lo..=i].iter().filter_map(|v| *v).collect();
            if present.is_empty() {
                None
            } else {
                Some(round_plot(
                    present.iter().sum::<f64>() / present.len() as f64,
                ))
            }
        })
        .collect()
}

/// Round a plotted value, keeping the payload small **without** deleting it.
///
/// Three decimal places is the wrong tool for a unit-converted quantity, and
/// it was silently destroying data. Block size is plotted in megabytes, so the
/// genesis block's 285 bytes is 0.000285 and rounded to `0.000`; block fees are
/// plotted in BTC, so a block carrying 19,818 sats became `0.0`. Across ALL
/// that turned 564 daily size averages and 1,073 fee readings into zeros, and
/// on a log axis, which cannot plot zero, they were then dropped with a notice
/// blaming the data.
///
/// Significant figures instead. Six of them hold four bytes' worth of size and
/// a single satoshi, while keeping a serialised point to roughly the same
/// width as before, which is what the rounding was for: the payload, not the
/// precision of the underlying measurement.
///
/// The floor is not a display concern. A chart label showing `0.00 BTC` is a
/// formatting choice and `stats.js` makes it; a **plotted value** of zero is a
/// different number from 0.000285 and no formatter downstream can recover it.
pub(crate) fn round_plot(val: f64) -> f64 {
    const DIGITS: i32 = 6;
    if val == 0.0 || !val.is_finite() {
        return val;
    }
    let mag = val.abs().log10().floor() as i32;
    // Already coarser than six significant figures, so rounding is a no-op
    // and the scaling below would only introduce float noise.
    if mag >= DIGITS {
        return val.round();
    }
    let factor = 10f64.powi(DIGITS - 1 - mag);
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
    fit_value_axis(&mut base);
    base
}

/// Let the left value axis fit its data instead of always reaching zero.
///
/// ECharts defaults a value axis to `scale: false`, which forces zero into
/// range. For a quantity that never goes near zero that throws the detail
/// away: difficulty over three months sat between 120 T and 142 T on a
/// 0-to-150 axis, so every adjustment in the quarter read as a flat line.
/// Fitting the axis is what makes the same chart legible, and it is why the
/// log view appeared to be such an improvement: log was quietly doing the
/// fitting the linear axis should have been doing all along.
///
/// Three kinds of chart still need zero, and the reason differs each time:
///
/// - **Bars.** Bar length encodes magnitude, so a cut axis misstates ratios.
///   This is the one where fitting would actually mislead rather than just
///   look different.
/// - **Stacked.** The stack is a sum measured from zero.
/// - **Percentages.** 0 to 100 is the frame that gives the number meaning.
///
/// Decided from the assembled option rather than from per-chart metadata, the
/// same way the log refusals are, so a new chart gets the right axis without
/// declaring anything.
fn fit_value_axis(option: &mut serde_json::Value) {
    let series = option
        .get("series")
        .and_then(|s| s.as_array())
        .map(|a| a.as_slice())
        .unwrap_or_default();
    if series.is_empty() {
        return;
    }
    let has_bars = series.iter().any(|s| {
        s.get("type").and_then(|t| t.as_str()) == Some("bar")
            || s.get("stack").is_some()
    });
    if has_bars {
        return;
    }
    let Some(axis) = option.get_mut("yAxis") else {
        return;
    };
    let first = match axis {
        serde_json::Value::Array(a) => a.first_mut(),
        other => Some(other),
    };
    let Some(serde_json::Value::Object(o)) = first else {
        return;
    };
    if o.get("type").and_then(|t| t.as_str()) != Some("value") {
        return;
    }
    // A percentage axis declares its own frame; leave it alone.
    if o.get("max").and_then(|m| m.as_f64()) == Some(100.0)
        || o.get("min").is_some()
    {
        return;
    }
    o.insert("scale".to_string(), serde_json::Value::Bool(true));
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

/// Halving dates as UTC calendar dates, newest first. Paired with
/// [`halving_era_for_date`]; per-block code should use height instead.
const HALVING_DATE_BOUNDARIES: &[&str] =
    &["2024-04-20", "2020-05-11", "2016-07-09", "2012-11-28"];

/// Halving era (0 = 50 BTC era) for a UTC date string, `YYYY-MM-DD`.
///
/// Daily aggregates carry no height, so era has to be derived from the date.
/// That makes the halving day itself an approximation: a halving happens at a
/// block, part-way through a day, so the halving date contains blocks from both
/// eras and this attributes all of them to the new one. Four days in the chain's
/// history are affected, one per halving, and only in daily mode. Per-block
/// charts use [`block_subsidy`] and are exact.
pub(crate) fn halving_era_for_date(date: &str) -> u32 {
    // Boundaries the date has not yet reached. The list is newest-first, so
    // subtracting these from the total leaves the number of halvings that had
    // already happened, which is the era. Dates are `YYYY-MM-DD`, so a string
    // compare is a chronological compare.
    let not_yet_reached = HALVING_DATE_BOUNDARIES
        .iter()
        .filter(|boundary| date < **boundary)
        .count();
    (HALVING_DATE_BOUNDARIES.len() - not_yet_reached) as u32
}

/// Block subsidy in BTC for a UTC date string. See [`halving_era_for_date`]
/// for the halving-day caveat.
pub(crate) fn daily_subsidy_btc(date: &str) -> f64 {
    50.0 / 2f64.powi(halving_era_for_date(date) as i32)
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
    /// Render the left value axis logarithmically.
    ///
    /// Lives here rather than per chart because it is a view option applied
    /// after the base chart is built, exactly like the annotations, so it
    /// belongs to the same cache generation. `apply_log_scale` refuses it
    /// where it would mislead, which is what lets one global switch cover
    /// charts of every shape.
    pub log_scale: bool,
    /// Render the right value axis, the one the price or chain-size overlay
    /// owns, logarithmically.
    ///
    /// Independent of `log_scale` because the two axes carry unrelated
    /// quantities. Price over ALL needs a log axis to be readable at all,
    /// and tying that to the metric's own scale would mean accepting a scale
    /// change on the thing the chart is actually about in order to read the
    /// overlay beside it.
    pub right_log_scale: bool,
}

impl OverlayFlags {
    /// Whether an overlay will add a right-hand value axis, and so whether
    /// `right_log_scale` has anything to act on.
    ///
    /// Answered from the flags rather than from a built option because the UI
    /// has to decide whether to show the second scale switch before any chart
    /// has been built.
    pub fn has_right_axis(&self) -> bool {
        !self.price_data.is_empty() || !self.chain_size_data.is_empty()
    }

    /// Compact string key for cache differentiation.
    ///
    /// Every field that changes the rendered option has to appear here.
    /// Omitting one serves the previous render on toggle, which is the defect
    /// `fix/chart-correctness` fixed and the reason both scale flags are
    /// keyed even though they are applied long after the base chart is built.
    pub fn cache_key(&self) -> String {
        format!(
            "h{}b{}c{}e{}p{}s{}l{}r{}",
            self.halvings as u8,
            self.bip_activations as u8,
            self.core_releases as u8,
            self.events as u8,
            self.price_data.len(),
            self.chain_size_data.len(),
            self.log_scale as u8,
            self.right_log_scale as u8,
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

    let axis_idx = push_right_axis(obj, unit, color);

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

/// Lay a second chart's series over this one on the right axis.
///
/// The comparison chart is built from the same rows over the same range by
/// the same family of builders, so the two series already share an x domain
/// and need no interpolation, unlike the price and chain-size overlays whose
/// data arrives from elsewhere on its own dates. That is also why this
/// refuses rather than resamples when the shapes do not line up: a mismatch
/// means an assumption broke, and silently drawing a misaligned series is the
/// worst available outcome on a chart whose whole claim is that the numbers
/// are checkable.
///
/// Takes the other chart's one **metric** series, which is not the same thing
/// as its first. `fee-spikes` draws its 144-block moving average at index 0
/// and the spikes themselves at index 1, so lifting by position drew the
/// smoothing companion under the label "Fee Spike Detector" while the rail
/// beside it reported the spikes. Position is a proxy for meaning and this is
/// what it costs; `metric_series` answers the question actually being asked.
///
/// Returns whether it applied, so the caller can say "not available over this
/// range" instead of showing a picker that quietly does nothing.
pub fn apply_comparison(
    option: &mut serde_json::Value,
    other: &serde_json::Value,
    label: &str,
    axis_name: &str,
) -> bool {
    if !accepts_comparison_series(option) {
        return false;
    }
    // Refuse a source with more than one real metric, and a source whose x
    // means something different.
    //
    // One series is lifted, so a multi-metric source would arrive as one of
    // its parts under the whole chart's name: comparing difficulty adjustment
    // dropped every easing retarget, and comparing transaction batching drew
    // outputs per transaction while calling itself both.
    //
    // And sharing dashboard rows does not mean sharing an x domain. Fee
    // pressure plots block fullness on a value axis, so overlaying it on a
    // time axis put percentages where milliseconds belong and dated its
    // extrema to 1970.
    //
    // **Temporary, and narrower than the feature should be.** The right fix
    // is to offer named measurements rather than charts, so "Transaction
    // Batching: Outputs per Transaction" is selectable and complete. Until
    // then refusing is the honest answer; see `notes/phase-2-spec.md`.
    let metrics = metric_series(other);
    if metrics.len() != 1 || !x_domains_match(option, other) {
        return false;
    }
    let source = metrics[0];
    let Some(data) = source
        .get("data")
        .and_then(|d| d.as_array())
        .filter(|d| !d.is_empty())
    else {
        return false;
    };
    let Some(obj) = option.as_object_mut() else {
        return false;
    };
    let Some(own) = obj
        .get("series")
        .and_then(|s| s.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("data"))
        .and_then(|d| d.as_array())
    else {
        return false;
    };
    // A daily chart plots bare numbers positioned by the category axis, so an
    // unequal count silently shifts the whole comparison sideways. A per-block
    // chart carries its own timestamps and is self-aligning, so a different
    // count there is fine and expected.
    let positional = own.first().is_some_and(|v| !v.is_array());
    if positional && own.len() != data.len() {
        return false;
    }
    let data = data.clone();

    let axis_idx = push_right_axis(obj, axis_name, COMPARISON_COLOR);
    let Some(serde_json::Value::Array(mut arr)) = obj.remove("series") else {
        return false;
    };
    // The metric's own series stay on the left axis. ECharts defaults an
    // absent `yAxisIndex` to 0, but only while there is one axis.
    for s in arr.iter_mut() {
        if let Some(o) = s.as_object_mut() {
            o.entry("yAxisIndex").or_insert(json!(0));
        }
    }
    // Drawn the way its own chart draws it. Forcing everything to a smoothed
    // line turned the difficulty-adjustment series, which is sparse bars at
    // retargets and null in between, into a continuous curve through data
    // that does not exist.
    let bars = source.get("type").and_then(|t| t.as_str()) == Some("bar");
    let mut series = json!({
        "name": label,
        "type": if bars { "bar" } else { "line" },
        "yAxisIndex": axis_idx,
        "data": data,
        "itemStyle": { "color": COMPARISON_COLOR },
        "z": 1
    });
    if let Some(o) = series.as_object_mut() {
        if bars {
            o.insert("barMaxWidth".into(), json!(6));
        } else {
            o.insert(
                "lineStyle".into(),
                json!({ "color": COMPARISON_COLOR, "width": 1.5, "opacity": 0.85 }),
            );
            o.insert("symbol".into(), json!("none"));
            o.insert("smooth".into(), json!(true));
            // No `connectNulls`. A gap in the compared chart is a gap in its
            // data, and bridging it draws a line the source never claimed.
            o.insert("connectNulls".into(), json!(false));
        }
    }
    arr.push(series);
    obj.insert("series".into(), json!(arr));
    // Two unlabelled lines is a puzzle rather than a comparison.
    if let Some(l) = obj.get_mut("legend").and_then(|l| l.as_object_mut()) {
        l.insert("show".into(), json!(true));
    }
    true
}

/// How many real metrics an option plots, excluding smoothing companions.
///
/// Honours `COMPANION_MARKER` first, then falls back to the naming convention
/// every companion series follows. That convention had exactly one exception
/// and it has been corrected, so the check is currently accurate; it is still
/// a name-based proxy and the phase-2 contract replaces it with a declared
/// role.
fn metric_series(option: &serde_json::Value) -> Vec<&serde_json::Value> {
    option
        .get("series")
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter(|s| {
                    let has_points = s
                        .get("data")
                        .and_then(|d| d.as_array())
                        .is_some_and(|d| !d.is_empty());
                    !is_companion_series(s) && has_points
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether a series accompanies another rather than measuring something of
/// its own.
///
/// The declared marker first, then the naming convention every smoothing
/// series follows. Named rather than inlined because two places need the same
/// answer: the key figures skip companions, and the conformance suite has to
/// know which series a measurement declaration is allowed not to name.
pub(crate) fn is_companion_series(s: &serde_json::Value) -> bool {
    if s.get(COMPANION_MARKER).and_then(|v| v.as_bool()) == Some(true) {
        return true;
    }
    s.get("name")
        .and_then(|n| n.as_str())
        .map(|n| {
            let n = n.to_ascii_lowercase();
            n.contains(" ma")
                || n.contains("moving average")
                || n.ends_with("ma")
        })
        .unwrap_or(false)
}

/// The count alone, which is what the conformance suite asserts on.
#[cfg(test)]
fn metric_series_count(option: &serde_json::Value) -> usize {
    metric_series(option).len()
}

/// Whether two options put the same kind of thing on their x axis.
fn x_domains_match(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    let kind = |v: &serde_json::Value| {
        let axis = match v.get("xAxis") {
            Some(serde_json::Value::Array(arr)) => arr.first().cloned(),
            other => other.cloned(),
        };
        axis.and_then(|x| {
            x.get("type").and_then(|t| t.as_str()).map(str::to_string)
        })
    };
    match (kind(a), kind(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// Whether this option can hold a second series on a right axis without lying.
///
/// Structural, and decided from the built option rather than from chart
/// metadata, for the same reason `log_scale_is_meaningful` and
/// `fit_value_axis` are: it then covers every call site, including ones that
/// do not exist yet, and a new chart gets the right answer without declaring
/// anything.
///
/// This is deliberately a second line of defence behind
/// `registry::is_valid_comparison`. That one is editorial and gates what the
/// picker offers; this one gates what can actually be drawn. Keeping them
/// separate is what stops "the select never renders on such a chart" from
/// being the thing holding the invariant up, which is how a comparison
/// reached a donut the moment the selection outlived the page.
///
/// Four refusals:
///
/// - **A pie series.** There are no cartesian axes to hang anything from, so
///   this pushed a value axis onto a donut and drew a line across it.
/// - **Two value axes already.** `Unit::Mixed` charts spend the right axis on
///   themselves, and an overlay that has already claimed it leaves nothing
///   free. A third axis on one plot is unreadable, so this is also what makes
///   the price/chain-size/comparison exclusion structural rather than a rule
///   the UI remembers to grey out.
/// - **Stacked percentage bands.** They fill 0 to 100 and are read against
///   each other; a second scale cannot be read against them at all.
/// - **No series.** Nothing to compare against.
fn accepts_comparison_series(option: &serde_json::Value) -> bool {
    let Some(series) = option.get("series").and_then(|s| s.as_array()) else {
        return false;
    };
    if series.is_empty() {
        return false;
    }
    if series
        .iter()
        .any(|s| s.get("type").and_then(|t| t.as_str()) == Some("pie"))
    {
        return false;
    }
    if option
        .get("yAxis")
        .and_then(|a| a.as_array())
        .is_some_and(|a| a.len() > 1)
    {
        return false;
    }
    let bounded_to_100 = |a: &serde_json::Value| {
        a.get("max").and_then(|m| m.as_f64()) == Some(100.0)
    };
    let percent_axis = match option.get("yAxis") {
        Some(serde_json::Value::Array(a)) => a.iter().any(bounded_to_100),
        Some(other) => bounded_to_100(other),
        None => false,
    };
    !(percent_axis && series.iter().any(|s| s.get("stack").is_some()))
}

/// Blue, because the two existing right-axis occupants are gold (price) and
/// green (chain size) and only one of the three is ever on at once.
const COMPARISON_COLOR: &str = "#60a5fa";

/// Append a right-hand value axis and return its index.
///
/// Shared with the price and chain-size overlays so all three occupants of
/// the right axis get the same fitting, the same abbreviated labels and the
/// same grid widening. Before this was factored out, the overlay axis had
/// picked up `scale` and the SI formatter and a second implementation would
/// have started without them.
fn push_right_axis(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    name: &str,
    color: &str,
) -> usize {
    let mut y_axes = match obj.remove("yAxis") {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(v) => vec![v],
        None => vec![json!({ "type": "value" })],
    };
    let axis_idx = y_axes.len();
    y_axes.push(json!({
        "type": "value",
        "name": name,
        "nameTextStyle": { "color": color },
        "position": "right",
        "offset": if axis_idx > 1 { 60 } else { 0 },
        // Fit the series to its own data for the same reason the metric axis
        // does: ECharts reaches for zero by default, and price from $0.05 to
        // $120,000 on a zero-based axis is a flat line along the bottom until
        // 2017. A series on the right axis is there to be compared against,
        // so it has to be legible on its own terms.
        "scale": true,
        "axisLabel": {
            "color": color,
            "fontSize": 10,
            // Price crosses six decades and chain size three, the same
            // problem the metric axis has. Without this a log price axis
            // labels its floor "0.05000000000000001".
            "formatter": SI_AXIS_SENTINEL
        },
        "axisLine": { "lineStyle": { "color": color } },
        "splitLine": { "show": false },
        // The axis name sits above the axis, which is exactly where the
        // toolbox icons are. Default `nameGap` is 15, which put "USD" level
        // with the restore icon and on top of it once the toolbox moved left
        // to make room. 6 drops the name to just above the first tick, below
        // the icon row.
        "nameGap": 6
    }));
    obj.insert("yAxis".into(), json!(y_axes));

    // Room for the axis and its labels, kept at the widest any occupant needs
    // so toggling between them does not resize the plot.
    if let Some(g) = obj.get_mut("grid").and_then(|g| g.as_object_mut()) {
        let current = g
            .get("right")
            .and_then(|v| v.as_u64())
            .unwrap_or(GRID_RIGHT);
        g.insert(
            "right".into(),
            json!(current.max(70) + if axis_idx > 1 { 60 } else { 0 }),
        );
    }
    axis_idx
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
            .unwrap_or(GRID_RIGHT);
        if let Some(grid) = obj.get_mut("grid") {
            if let Some(g) = grid.as_object_mut() {
                g.insert("top".into(), json!(45));
            }
        }
        if let Some(toolbox) = obj.get_mut("toolbox") {
            if let Some(t) = toolbox.as_object_mut() {
                // Clear of the axis name as well as the axis. +25 left the
                // two touching.
                t.insert("right".into(), json!(grid_right + 45));
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

    /// Daily categories from `start` for `days`, the shape every daily
    /// builder hands to `x_axis_for`.
    fn daily_categories(start: &str, days: i64) -> Vec<String> {
        let from = chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d")
            .expect("test date");
        (0..days)
            .map(|d| {
                (from + chrono::Duration::days(d))
                    .format("%Y-%m-%d")
                    .to_string()
            })
            .collect()
    }

    /// The reported bug: months walked forward (`Jan '10, Aug '10, Feb '11`)
    /// because labels were thinned by index across daily categories. Every
    /// candidate must now be the first plotted day of a month, so no subset
    /// ECharts keeps can land mid-month.
    #[test]
    fn calendar_ticks_only_ever_start_a_month() {
        let cats = daily_categories("2009-01-03", 6456);
        let ticks =
            calendar_tick_indices(&cats).expect("16 years is above the floor");
        assert!(ticks.len() >= 2);
        for &i in &ticks {
            let (year, month) = year_month(&cats[i]).expect("a date");
            // First plotted day of its month: either index 0, or the previous
            // category belongs to a different month.
            let starts_month =
                i == 0 || year_month(&cats[i - 1]) != Some((year, month));
            assert!(
                starts_month,
                "tick {i} at {} is not the first day of its month",
                cats[i]
            );
        }
    }

    /// A missing day must move a label to the next day that exists rather
    /// than dropping the month. Real daily data has gaps.
    #[test]
    fn calendar_ticks_survive_a_missing_first_of_the_month() {
        let mut cats = daily_categories("2015-01-01", 2200);
        let dropped = cats
            .iter()
            .position(|c| c == "2017-01-01")
            .expect("in range");
        cats.remove(dropped);
        let ticks = calendar_tick_indices(&cats).unwrap();
        let labelled: Vec<&str> =
            ticks.iter().map(|&i| cats[i].as_str()).collect();
        assert!(
            labelled.contains(&"2017-01-02"),
            "January 2017 lost its label to a one-day gap: {labelled:?}"
        );
    }

    /// Below two years the labels are days, where nobody reads the axis as
    /// calendar boundaries. Aligning there would thin a 3M chart to three
    /// labels for no gain, so the span floor has to hold.
    #[test]
    fn calendar_ticks_are_declined_on_short_and_non_date_axes() {
        assert!(calendar_tick_indices(&daily_categories("2025-01-01", 90))
            .is_none());
        assert!(calendar_tick_indices(&daily_categories("2024-01-01", 730))
            .is_none());
        assert!(calendar_tick_indices(&[]).is_none());
        // The weekday and fee-bucket axes are words, not dates.
        let words: Vec<String> = ["Mon", "Tue", "Wed"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(calendar_tick_indices(&words).is_none());
    }

    /// The stride has to stay dense enough that a zoomed-in window still has
    /// labels, and sparse enough that `hideOverlap` is not doing all the
    /// thinning. Both ends of that are what make the alignment visible.
    #[test]
    fn calendar_tick_density_stays_in_a_usable_band() {
        for (start, days) in [
            ("2009-01-03", 6456),
            ("2020-01-01", 2192),
            ("2023-01-01", 900),
        ] {
            let cats = daily_categories(start, days);
            let n = calendar_tick_indices(&cats)
                .unwrap_or_else(|| panic!("{days} days should align"))
                .len();
            assert!(
                (6..=CALENDAR_TICK_TARGET as usize + 2).contains(&n),
                "{days} days produced {n} candidate labels"
            );
        }
    }

    /// The sentinel is only useful paired with the indices, and `stats.js`
    /// reads both off `axisLabel`. A rename on either side has to fail here
    /// rather than silently label nothing.
    #[test]
    fn the_category_axis_carries_both_halves_of_the_handshake() {
        let cats = daily_categories("2009-01-03", 6456);
        let axis = x_axis_for(true, &cats);
        let label = &axis["axisLabel"];
        assert_eq!(label["interval"], CALENDAR_TICK_SENTINEL);
        assert!(
            label[CALENDAR_TICK_DATA_KEY]
                .as_array()
                .is_some_and(|a| a.len() >= 2),
            "sentinel present with no indices to install"
        );
        // And a short range leaves ECharts' own thinning alone.
        let short = x_axis_for(true, &daily_categories("2025-01-01", 90));
        assert!(short["axisLabel"].get("interval").is_none());
        assert!(short["axisLabel"].get(CALENDAR_TICK_DATA_KEY).is_none());
    }

    /// A time axis formats every tick level with one string unless told
    /// otherwise, which printed a bare day number where a date belonged and
    /// no clock time at all on a 1D range.
    #[test]
    fn the_time_axis_names_a_format_for_every_level() {
        let axis = x_axis_for(false, &[]);
        let fmt = &axis["axisLabel"]["formatter"];
        for level in
            ["year", "month", "day", "hour", "minute", "second", "none"]
        {
            assert!(
                fmt[level].as_str().is_some_and(|s| !s.is_empty()),
                "time axis has no format for {level}"
            );
        }
        assert_eq!(fmt["hour"], "{HH}:{mm}", "1D has to read as clock time");
    }

    /// The whole decoration pipeline over a real builder, in the order the
    /// page applies it.
    ///
    /// Every other test here builds an option by hand with exactly the series
    /// it wants to check. That is why both defects on this branch survived to
    /// the end: each function was right and the composition was not. This
    /// starts from a real chart and asserts what a reader would see.
    fn pipeline(
        flags: &OverlayFlags,
        compare_with: Option<&serde_json::Value>,
    ) -> serde_json::Value {
        let days: Vec<_> = daily_categories("2012-01-01", 3000)
            .iter()
            .enumerate()
            .map(|(i, d)| daily_with_difficulty(d, 1.0 + i as f64 * 1e9))
            .collect();
        let mut v = super::network::difficulty_chart_daily(&days);
        apply_overlays(&mut v, flags, true);
        if let Some(other) = compare_with {
            apply_comparison(&mut v, other, "Transaction Count", "count");
        }
        apply_scales(&mut v, flags);
        v
    }

    /// Mark lines, a price overlay and both scales at once. The combination a
    /// reader reaches by turning on everything in the rail.
    #[test]
    fn the_full_pipeline_leaves_each_axis_where_it_was_asked_to_be() {
        let cats = daily_categories("2012-01-01", 3000);
        let price: Vec<(u64, f64)> = cats
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let ms = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp_millis() as u64;
                (ms, 5.0 + i as f64 * 40.0)
            })
            .collect();
        let flags = OverlayFlags {
            halvings: true,
            price_data: price,
            log_scale: true,
            right_log_scale: true,
            ..Default::default()
        };
        let v = pipeline(&flags, None);

        assert_eq!(v["yAxis"][0]["type"], "log", "metric axis");
        assert_eq!(v["yAxis"][1]["type"], "log", "overlay axis");
        // Independently: the reader can have one without the other.
        let mixed = pipeline(
            &OverlayFlags {
                log_scale: false,
                ..flags.clone()
            },
            None,
        );
        assert_eq!(mixed["yAxis"][0]["type"], "value");
        assert_eq!(mixed["yAxis"][1]["type"], "log");

        // The calendar ticks survive everything downstream of them.
        let label = &v["xAxis"]["axisLabel"];
        assert_eq!(label["interval"], CALENDAR_TICK_SENTINEL);
        assert!(label[CALENDAR_TICK_DATA_KEY].as_array().unwrap().len() >= 2);

        // And the mark lines did not take the axis's room back.
        assert!(v["grid"]["right"].as_u64().unwrap() >= 70);
    }

    /// The two defects this branch shipped with, asserted at the level they
    /// actually appeared: what the rail prints beside the chart.
    #[test]
    fn key_figures_describe_the_metric_and_not_what_is_laid_over_it() {
        let cats = daily_categories("2012-01-01", 3000);
        let other: Vec<_> = cats
            .iter()
            .enumerate()
            // Deliberately far larger than difficulty here, so counting it
            // would be unmissable in the peak.
            .map(|(i, d)| daily_with_difficulty(d, 1e30 + i as f64))
            .collect();
        let compare = super::network::difficulty_chart_daily(&other);

        let plain = pipeline(&OverlayFlags::default(), None);
        let with_compare = pipeline(&OverlayFlags::default(), Some(&compare));

        // The comparison is drawn...
        assert_eq!(
            with_compare["series"].as_array().unwrap().len(),
            plain["series"].as_array().unwrap().len() + 1
        );
        assert!(has_right_value_axis(&with_compare));

        // ...and the key figures do not see it.
        let kpis = |v: &serde_json::Value| {
            crate::stats::charts::kpi::compute(
                &serde_json::to_string(v).unwrap(),
                crate::stats::charts::registry::Shape::Line,
            )
        };
        let before = kpis(&plain);
        // Asserting equality alone would pass if both sides were
        // `Unavailable`, which is exactly what a bad axis filter would
        // produce. Pin the real figures first.
        match &before {
            crate::stats::charts::kpi::Kpis::Series {
                peak,
                observations,
                ..
            } => {
                assert_eq!(*observations, 3000);
                assert!(peak.y > 1e12, "read the metric, not a placeholder");
            }
            other => panic!("the metric alone should report: {other:?}"),
        }
        assert_eq!(
            before,
            kpis(&with_compare),
            "the comparison series changed the metric's own key figures"
        );
    }

    /// The defect a review caught: `apply_right_log_scale` shipped with no
    /// dropped-point notice, justified by a comment naming price and chain
    /// size as the only occupants of that axis. `apply_comparison` then let
    /// any comparable chart claim it, including `utxo-growth`, which is net
    /// and goes negative. Switching the overlay axis to log dropped those
    /// points with nothing on screen saying so.
    #[test]
    fn a_negative_comparison_on_a_log_overlay_axis_says_so() {
        let mut v = daily_chart(&[1.0, 2.0, 3.0]);
        // `graphic` is where the notice lands; real builders always have one.
        v["graphic"] = json!([]);
        let negative = daily_chart(&[5.0, -1.0, 0.0]);
        assert!(apply_comparison(&mut v, &negative, "UTXO Growth", "count"));

        let flags = OverlayFlags {
            right_log_scale: true,
            ..Default::default()
        };
        apply_scales(&mut v, &flags);
        let notice = v["graphic"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] == LOG_NOTICE_ID)
            .expect("two non-positive points went unmentioned");
        let text = notice["style"]["text"].as_str().unwrap();
        assert!(text.starts_with('2'), "wrong count: {text}");

        // And it clears when the axis goes back to linear.
        apply_scales(&mut v, &OverlayFlags::default());
        assert!(!v["graphic"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == LOG_NOTICE_ID));
    }

    /// One notice, not one per axis: it is pinned to the top of the plot, so
    /// two would sit on top of each other. The count has to be the total.
    #[test]
    fn both_axes_dropping_points_produce_one_combined_notice() {
        let mut v = daily_chart(&[1.0, 0.0, 3.0]);
        v["graphic"] = json!([]);
        let negative = daily_chart(&[5.0, -1.0, 0.0]);
        assert!(apply_comparison(&mut v, &negative, "UTXO Growth", "count"));
        apply_scales(
            &mut v,
            &OverlayFlags {
                log_scale: true,
                right_log_scale: true,
                ..Default::default()
            },
        );
        let notices: Vec<&serde_json::Value> = v["graphic"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g["id"] == LOG_NOTICE_ID)
            .collect();
        assert_eq!(notices.len(), 1, "one notice, whatever dropped");
        let text = notices[0]["style"]["text"].as_str().unwrap();
        assert!(
            text.starts_with('3'),
            "one zero on the left plus two on the right: {text}"
        );

        // And each axis counts only its own. A positive metric beside a
        // negative comparison must not claim points it plots perfectly well
        // were dropped.
        let mut left_only = daily_chart(&[1.0, 2.0, 3.0]);
        left_only["graphic"] = json!([]);
        assert!(apply_comparison(
            &mut left_only,
            &daily_chart(&[5.0, -1.0, -2.0]),
            "UTXO Growth",
            "count"
        ));
        apply_scales(
            &mut left_only,
            &OverlayFlags {
                log_scale: true,
                ..Default::default()
            },
        );
        assert!(
            !left_only["graphic"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["id"] == LOG_NOTICE_ID),
            "the metric plots every point; nothing was dropped"
        );
    }

    /// A log axis labels only at powers of ten, so a span under one decade
    /// gets three labels, two of which are its own bounds. Minor split lines
    /// are what make it readable between them, and they have to come off
    /// again with everything else when the axis goes back to linear.
    #[test]
    fn a_log_axis_carries_minor_gridlines_and_gives_them_back() {
        // The real case: UTXO flow beside difficulty, 1.9K to 11K.
        let mut v = daily_chart(&[1_900.0, 5_000.0, 11_000.0]);
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["type"], "log");
        assert_eq!(v["yAxis"]["minorTick"]["show"], true);
        assert_eq!(v["yAxis"]["minorSplitLine"]["show"], true);

        apply_log_scale(&mut v, false);
        assert!(v["yAxis"].get("minorTick").is_none());
        assert!(v["yAxis"].get("minorSplitLine").is_none());
        assert!(v["yAxis"].get("splitNumber").is_none());
    }

    /// A pie has no cartesian axes, so this pushed a value axis onto a donut
    /// and drew a line across it.
    ///
    /// **Matching lengths, deliberately.** The first probe of this used
    /// mismatched lengths and reported "refused", which was the length guard
    /// firing, not a real refusal. Reading that as safety is how this would
    /// have shipped, so the test pins the case that actually applied.
    #[test]
    fn a_comparison_is_refused_on_a_donut() {
        let mut donut = json!({
            "series": [{
                "name": "Pools", "type": "pie",
                "data": [{"name": "Foundry", "value": 30.0},
                         {"name": "AntPool", "value": 20.0}]
            }]
        });
        let before = donut.clone();
        assert!(!apply_comparison(
            &mut donut,
            &daily_chart(&[1.0, 2.0]),
            "Other",
            "count"
        ));
        assert_eq!(donut, before, "a refused comparison must change nothing");
    }

    /// A chart already using both axes has nothing free, and a third value
    /// axis on one plot cannot be read. This is also what makes the
    /// price/chain-size/comparison exclusion structural: with an overlay
    /// already on, there is no room, whatever the UI did or did not grey out.
    #[test]
    fn a_comparison_is_refused_when_both_axes_are_taken() {
        // A Unit::Mixed chart, which arrives with two axes of its own.
        let mut mixed = json!({
            "yAxis": [{"type": "value"}, {"type": "value"}],
            "series": [
                {"name": "Count", "yAxisIndex": 0, "data": [1.0, 2.0, 3.0]},
                {"name": "BTC", "yAxisIndex": 1, "data": [4.0, 5.0, 6.0]}
            ]
        });
        let before = mixed.clone();
        assert!(!apply_comparison(
            &mut mixed,
            &daily_chart(&[1.0, 2.0, 3.0]),
            "Other",
            "count"
        ));
        assert_eq!(mixed, before);

        // And the same refusal once an overlay has claimed the right axis,
        // reached through the real path rather than a hand-built option.
        let mut v = per_block_chart(&[
            (1_231_006_505_000, 1.0),
            (1_600_000_000_000, 2.0),
        ]);
        apply_overlays(
            &mut v,
            &OverlayFlags {
                price_data: vec![
                    (1_231_006_505_000, 0.05),
                    (1_600_000_000_000, 120_000.0),
                ],
                ..Default::default()
            },
            false,
        );
        assert!(!apply_comparison(
            &mut v,
            &per_block_chart(&[(1_231_006_505_000, 9.0)]),
            "Other",
            "count"
        ));
        assert_eq!(
            v["yAxis"].as_array().unwrap().len(),
            2,
            "a third axis was created"
        );
    }

    /// Stacked percentage bands fill 0 to 100 and are read against each
    /// other, so a second scale cannot be read against them at all. The same
    /// refusal the log axis makes, for the same reason.
    #[test]
    fn a_comparison_is_refused_on_stacked_percentage_bands() {
        let mut pct = json!({
            "yAxis": {"type": "value", "max": 100.0},
            "series": [
                {"name": "P2PKH", "stack": "t", "data": [60.0, 55.0, 50.0]},
                {"name": "P2WPKH", "stack": "t", "data": [40.0, 45.0, 50.0]}
            ]
        });
        let before = pct.clone();
        assert!(!apply_comparison(
            &mut pct,
            &daily_chart(&[1.0, 2.0, 3.0]),
            "Other",
            "count"
        ));
        assert_eq!(pct, before);
    }

    /// A permalink can ask for a comparison and the price overlay at once,
    /// and only one of them can have the right axis. The overlay wins, which
    /// is the precedence `apply_overlays` already sets between price and
    /// chain size, and the comparison has to be refused rather than half
    /// applied.
    #[test]
    fn an_overlay_and_a_comparison_cannot_both_hold_the_right_axis() {
        let mut v = per_block_chart(&[
            (1_231_006_505_000, 1.0),
            (1_600_000_000_000, 2.0),
        ]);
        apply_overlays(
            &mut v,
            &OverlayFlags {
                price_data: vec![
                    (1_231_006_505_000, 0.05),
                    (1_600_000_000_000, 120_000.0),
                ],
                ..Default::default()
            },
            false,
        );
        let after_overlay = v.clone();
        assert!(!apply_comparison(
            &mut v,
            &per_block_chart(&[(1_231_006_505_000, 9.0)]),
            "Other",
            "count"
        ));
        assert_eq!(v, after_overlay, "a refused comparison changed the chart");
        assert_eq!(
            v["yAxis"].as_array().unwrap().len(),
            2,
            "exactly one right axis, held by the overlay"
        );
        assert_eq!(v["yAxis"][1]["name"], "USD");
    }

    /// A comparison is drawn the way its own chart draws it.
    ///
    /// Everything used to become a smoothed line with `connectNulls`, which
    /// turned difficulty adjustment, sparse bars at retargets with nulls
    /// between, into a continuous curve through data that does not exist.
    #[test]
    fn a_comparison_keeps_the_shape_of_its_source() {
        let mut v = daily_chart(&[1.0, 2.0, 3.0]);
        let mut bars = daily_chart(&[10.0, 20.0, 30.0]);
        bars["series"][0]["type"] = json!("bar");
        assert!(apply_comparison(&mut v, &bars, "Adjustment", "%"));
        let laid = &v["series"][1];
        assert_eq!(laid["type"], "bar");
        assert!(laid.get("smooth").is_none(), "a bar was smoothed");
        assert!(laid.get("connectNulls").is_none());

        // A line source still arrives as a line.
        let mut w = daily_chart(&[1.0, 2.0, 3.0]);
        assert!(apply_comparison(
            &mut w,
            &daily_chart(&[9.0, 8.0, 7.0]),
            "Other",
            "count"
        ));
        assert_eq!(w["series"][1]["type"], "line");
    }

    /// A gap in the compared chart is a gap in its data. Bridging it draws a
    /// line the source never claimed.
    #[test]
    fn a_comparison_does_not_bridge_gaps_in_its_source() {
        let mut v = daily_chart(&[1.0, 2.0, 3.0]);
        let mut sparse = daily_chart(&[10.0, 20.0, 30.0]);
        sparse["series"][0]["data"] =
            json!([10.0, serde_json::Value::Null, 30.0]);
        assert!(apply_comparison(&mut v, &sparse, "Sparse", "count"));
        assert_eq!(v["series"][1]["connectNulls"], false);
        // And the gap survives into the plotted data rather than being filled.
        assert!(v["series"][1]["data"][1].is_null());
    }

    /// A percentage chart declares `max: 100` so its bands are read against
    /// a fixed frame. That was being stripped on every render with log off,
    /// which is the default, so hiding one band let the axis refit to the
    /// remainder: a 90% band vanishing rescaled the chart from 0-100 to 0-10.
    /// It reached the existing multi-chart pages, not just this view.
    #[test]
    fn a_builders_own_axis_bounds_survive_the_linear_path() {
        let mut pct = json!({
            "yAxis": {"type": "value", "max": 100.0, "min": 0.0},
            "series": [
                {"name": "P2PKH", "stack": "t", "data": [60.0, 55.0]},
                {"name": "P2WPKH", "stack": "t", "data": [40.0, 45.0]}
            ]
        });
        let before = pct.clone();
        // The default state, which is what almost every render is.
        apply_scales(&mut pct, &OverlayFlags::default());
        assert_eq!(
            pct["yAxis"]["max"], 100.0,
            "the percentage frame was removed"
        );
        assert_eq!(pct["yAxis"]["min"], 0.0);
        assert_eq!(pct, before, "the linear path changed the axis at all");
    }

    /// And bounds this module sets for a log axis still come off again, or a
    /// log view would leave its fitted bounds on a linear chart.
    #[test]
    fn log_bounds_are_still_removed_on_the_way_back() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "Count", "data": [[1, 1.0], [2, 1000.0]]}]
        });
        apply_log_scale(&mut v, true);
        assert!(v["yAxis"]["min"].as_f64().is_some(), "log set no bounds");
        apply_log_scale(&mut v, false);
        assert!(v["yAxis"].get("min").is_none(), "stale log floor");
        assert!(v["yAxis"].get("max").is_none(), "stale log ceiling");
        assert!(v["yAxis"].get("splitNumber").is_none());
        // And the marker itself does not leak into the option.
        assert!(v["yAxis"].get(LOG_BOUNDS_MARKER).is_none());
    }

    /// The bounds print verbatim as axis labels, so they have to be numbers a
    /// person would write. Dividing by a fractional scale reintroduced the
    /// float the rounding existed to remove: the fee-per-transaction chart's
    /// max read "22,899,999.999999996".
    #[test]
    fn rounded_bounds_are_exact_at_every_magnitude() {
        for (v, digits, want) in [
            (22_817_400.0, 3, 22_900_000.0),
            (0.000_123_4, 3, 0.000_124),
            (1_600_000_000_000_000.0, 3, 1_600_000_000_000_000.0),
            (7.2, 2, 7.2),
        ] {
            let up = round_sig(v, digits, RoundDir::Up);
            assert!(up >= v, "{up} does not contain {v}");
            // Exact to the significant figures asked for, not one ulp off.
            let scaled =
                up / 10f64.powi(up.abs().log10().floor() as i32 - (digits - 1));
            assert!(
                (scaled - scaled.round()).abs() < 1e-9,
                "round_sig({v}, {digits}) gave {up}, which is not {digits} \
                 significant figures"
            );
            assert!(
                (up - want).abs() <= want.abs() * 1e-12,
                "got {up}, want {want}"
            );
        }
    }

    /// A daily chart: bare numbers positioned by the category axis.
    fn daily_chart(vals: &[f64]) -> serde_json::Value {
        json!({
            "grid": {"right": 20},
            "legend": {"show": false},
            "xAxis": {"type": "category", "data": ["a", "b", "c"]},
            "yAxis": {"type": "value", "name": "Count"},
            "series": [{"name": "Count", "type": "line", "data": vals}]
        })
    }

    /// A per-block chart: `[timestamp, value]` pairs that carry their own x.
    fn per_block_chart(pts: &[(u64, f64)]) -> serde_json::Value {
        let data: Vec<_> = pts.iter().map(|&(t, v)| json!([t, v])).collect();
        json!({
            "grid": {"right": 20},
            "legend": {"show": false},
            "xAxis": {"type": "time"},
            "yAxis": {"type": "value", "name": "Count"},
            "series": [{"name": "Count", "type": "line", "data": data}]
        })
    }

    /// The comparison lands on its own axis with the metric left where it was.
    #[test]
    fn a_comparison_takes_the_right_axis_and_leaves_the_metric_alone() {
        let mut v = daily_chart(&[1.0, 2.0, 3.0]);
        let other = daily_chart(&[100.0, 200.0, 300.0]);
        assert!(apply_comparison(&mut v, &other, "Fee rate", "sat/vB"));

        let axes = v["yAxis"].as_array().expect("two axes now");
        assert_eq!(axes.len(), 2);
        assert_eq!(axes[0]["name"], "Count", "the metric's axis is unchanged");
        assert_eq!(axes[1]["name"], "sat/vB");
        assert_eq!(axes[1]["position"], "right");

        let series = v["series"].as_array().unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series[0]["yAxisIndex"], 0, "metric pinned to the left");
        assert_eq!(series[1]["yAxisIndex"], 1);
        assert_eq!(series[1]["name"], "Fee rate");
        assert_eq!(series[1]["data"], json!([100.0, 200.0, 300.0]));
        // Two unlabelled lines is a puzzle, not a comparison.
        assert_eq!(v["legend"]["show"], true);
        // And the plot has to make room, or the axis is drawn over the data.
        assert!(v["grid"]["right"].as_u64().unwrap() >= 70);
    }

    /// A daily chart positions bare numbers by index, so an unequal count
    /// shifts the entire comparison sideways with nothing on screen saying
    /// so. Refusing is the only safe answer on a site whose claim is that the
    /// numbers are checkable.
    #[test]
    fn a_daily_comparison_refuses_a_length_mismatch() {
        let mut v = daily_chart(&[1.0, 2.0, 3.0]);
        let short = daily_chart(&[100.0, 200.0]);
        let before = v.clone();
        assert!(!apply_comparison(&mut v, &short, "Other", "count"));
        assert_eq!(v, before, "a refused comparison must change nothing");
    }

    /// Per-block points carry their own timestamps, so they self-align and a
    /// different count is normal rather than a mismatch.
    #[test]
    fn a_per_block_comparison_does_not_need_matching_lengths() {
        let mut v = per_block_chart(&[(1, 1.0), (2, 2.0), (3, 3.0)]);
        let other = per_block_chart(&[(1, 9.0), (3, 7.0)]);
        assert!(apply_comparison(&mut v, &other, "Other", "count"));
        assert_eq!(v["series"].as_array().unwrap().len(), 2);
    }

    /// A chart with no rows for this range would otherwise add an axis and an
    /// empty line, which reads as "the comparison is flat at zero".
    #[test]
    fn a_comparison_with_no_data_is_refused() {
        let mut v = daily_chart(&[1.0, 2.0, 3.0]);
        let before = v.clone();
        assert!(!apply_comparison(
            &mut v,
            &daily_chart(&[]),
            "Other",
            "count"
        ));
        assert!(!apply_comparison(
            &mut v,
            &json!({"series": []}),
            "Other",
            "count"
        ));
        assert!(!apply_comparison(&mut v, &json!({}), "Other", "count"));
        assert_eq!(v, before);
    }

    /// The comparison is the right axis while it is on, so the second scale
    /// switch has to move it. Without this the switch would look broken on
    /// exactly the charts where a log overlay matters most.
    #[test]
    fn the_scale_switch_reaches_a_comparison_axis() {
        let mut v = daily_chart(&[1.0, 2.0, 3.0]);
        let other = daily_chart(&[1.0, 1000.0, 1_000_000.0]);
        assert!(apply_comparison(&mut v, &other, "Other", "count"));
        assert!(has_right_value_axis(&v));
        apply_right_log_scale(&mut v, true);
        assert_eq!(v["yAxis"][1]["type"], "log");
        assert_eq!(v["yAxis"][0]["type"], "value");
    }

    /// Mark lines set `grid.right` to a fixed width for their labels. An axis
    /// added before them would be overdrawn, so the ordering in `decorate` is
    /// load-bearing and this is the assertion that holds it.
    #[test]
    fn a_comparison_survives_mark_lines_widening_the_grid() {
        let mut v = per_block_chart(&[
            (1_231_006_505_000, 1.0),
            (1_600_000_000_000, 2.0),
        ]);
        apply_overlays(
            &mut v,
            &OverlayFlags {
                halvings: true,
                ..Default::default()
            },
            false,
        );
        let after_overlays = v["grid"]["right"].as_u64().unwrap();
        let other = per_block_chart(&[
            (1_231_006_505_000, 9.0),
            (1_600_000_000_000, 8.0),
        ]);
        assert!(apply_comparison(&mut v, &other, "Other", "count"));
        assert!(
            v["grid"]["right"].as_u64().unwrap() >= after_overlays.max(70),
            "the comparison axis did not claim its own width"
        );
    }

    /// An option with one axis, and the same option after an overlay has
    /// added the right-hand one.
    fn with_overlay_axis() -> serde_json::Value {
        json!({
            "yAxis": [
                {"type": "value", "name": "Count", "scale": true},
                {"type": "value", "name": "USD", "scale": true}
            ],
            "series": [
                {"name": "Count", "yAxisIndex": 0, "data": [[1, 5.0], [2, 9.0]]},
                {"name": "Price (USD)", "yAxisIndex": 1,
                 "data": [[1, 0.05], [2, 120000.0]]}
            ]
        })
    }

    /// The point of the second switch: price goes logarithmic while the
    /// metric stays where the reader left it.
    #[test]
    fn the_overlay_axis_scales_independently_of_the_metric() {
        let mut v = with_overlay_axis();
        apply_right_log_scale(&mut v, true);
        assert_eq!(v["yAxis"][1]["type"], "log");
        assert_eq!(
            v["yAxis"][0]["type"], "value",
            "the metric's own axis must not follow the overlay"
        );
        // And the bounds fit the price data rather than reaching for zero,
        // which a log axis cannot represent anyway.
        let lo = v["yAxis"][1]["min"].as_f64().expect("a fitted floor");
        assert!(lo > 0.0 && lo <= 0.05, "floor {lo} does not contain $0.05");
    }

    /// Switching back has to hand the axis over exactly as the overlay built
    /// it. The left-axis version of this bug left a log chart's bounds on a
    /// linear axis.
    #[test]
    fn turning_the_overlay_axis_back_to_linear_clears_its_bounds() {
        let mut v = with_overlay_axis();
        apply_right_log_scale(&mut v, true);
        apply_right_log_scale(&mut v, false);
        assert_eq!(v["yAxis"][1]["type"], "value");
        assert!(v["yAxis"][1].get("min").is_none(), "stale log floor");
        assert!(v["yAxis"][1].get("max").is_none(), "stale log ceiling");
        assert!(v["yAxis"][1].get("splitNumber").is_none());
        assert_eq!(
            v["yAxis"][1]["scale"], true,
            "the overlay still has to fit its own data when linear"
        );
    }

    /// Called unconditionally after the overlays, so with no overlay on there
    /// is no right axis and nothing may change.
    #[test]
    fn the_overlay_scale_is_a_no_op_without_an_overlay() {
        let mut v = json!({
            "yAxis": {"type": "value", "name": "Count"},
            "series": [{"name": "Count", "data": [[1, 5.0], [2, 9.0]]}]
        });
        let before = v.clone();
        apply_right_log_scale(&mut v, true);
        assert_eq!(v, before);
        // Nor may it invent an axis on a chart whose y axis is categorical.
        let mut cat = json!({
            "yAxis": [{"type": "category", "data": ["Mon"]}],
            "series": [{"data": [1.0]}]
        });
        let cat_before = cat.clone();
        apply_right_log_scale(&mut cat, true);
        assert_eq!(cat, cat_before);
    }

    /// Both scales are applied after the base chart is cached, so a flag that
    /// is not in the key serves the previous render. That is the defect
    /// `fix/chart-correctness` fixed, and it has to stay fixed for each new
    /// flag.
    #[test]
    fn every_scale_flag_reaches_the_cache_key() {
        let base = OverlayFlags::default();
        let left = OverlayFlags {
            log_scale: true,
            ..Default::default()
        };
        let right = OverlayFlags {
            right_log_scale: true,
            ..Default::default()
        };
        let both = OverlayFlags {
            log_scale: true,
            right_log_scale: true,
            ..Default::default()
        };
        let keys = [
            base.cache_key(),
            left.cache_key(),
            right.cache_key(),
            both.cache_key(),
        ];
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                assert_ne!(a, b, "two scale states share a cache key");
            }
        }
    }

    /// The UI shows the second switch from the flags alone, before any chart
    /// exists, so that answer has to agree with what the overlays actually
    /// build.
    #[test]
    fn has_right_axis_agrees_with_the_built_option() {
        for flags in [
            OverlayFlags {
                price_data: vec![(1_231_006_505_000, 0.05)],
                ..Default::default()
            },
            OverlayFlags {
                chain_size_data: vec![(1_231_006_505_000, 0.1)],
                ..Default::default()
            },
            OverlayFlags {
                halvings: true,
                ..Default::default()
            },
            OverlayFlags::default(),
        ] {
            let mut v = json!({
                "xAxis": {"type": "time"},
                "grid": {"right": 20},
                "yAxis": {"type": "value"},
                "series": [{"name": "Count", "data": [
                    [1_231_006_505_000u64, 1.0], [1_231_100_000_000u64, 2.0]
                ]}]
            });
            apply_overlays(&mut v, &flags, false);
            assert_eq!(
                has_right_value_axis(&v),
                flags.has_right_axis(),
                "flags and built option disagree for {}",
                flags.cache_key()
            );
        }
    }

    /// The right axis crosses six decades, so it needs the same abbreviation
    /// the metric axis got. Without it a log price floor labels itself
    /// "0.05000000000000001".
    #[test]
    fn the_overlay_axis_abbreviates_its_labels() {
        let mut v = json!({
            "xAxis": {"type": "time"},
            "grid": {"right": 20},
            "yAxis": {"type": "value"},
            "series": [{"name": "Count", "data": [
                [1_231_006_505_000u64, 1.0], [1_600_000_000_000u64, 2.0]
            ]}]
        });
        let flags = OverlayFlags {
            price_data: vec![
                (1_231_006_505_000, 0.05),
                (1_600_000_000_000, 120_000.0),
            ],
            ..Default::default()
        };
        apply_overlays(&mut v, &flags, false);
        assert_eq!(v["yAxis"][1]["axisLabel"]["formatter"], SI_AXIS_SENTINEL);
    }

    /// The right axis belongs to the price or chain-size overlay. Rescaling it
    /// under them is the shape of the clipping bug fixed in
    /// `fix/chart-zoom-axis`.
    #[test]
    fn log_scale_only_touches_the_left_axis() {
        // A positive series, so the meaningfulness guard is satisfied and
        // this test is about the axis mechanics alone.
        let mut v = json!({
            "yAxis": [
                {"type": "value", "name": "Count"},
                {"type": "value", "name": "Price"}
            ],
            "series": [{"name": "Count", "data": [[1, 1.0], [2, 1000.0]]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"][0]["type"], "log");
        assert_eq!(v["yAxis"][1]["type"], "value", "overlay axis untouched");
        apply_log_scale(&mut v, false);
        assert_eq!(v["yAxis"][0]["type"], "value");
    }

    /// ECharts prints an explicit min and max verbatim, so unrounded padded
    /// bounds became axis labels like "122.4342086864" beside clean decade
    /// ticks. Both ends must read as numbers a person would write.
    #[test]
    fn log_axis_bounds_are_rounded_not_raw_floats() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "difficulty", "data": [
                [1, 122.4342086864], [2, 141.7344641525]
            ]}]
        });
        apply_log_scale(&mut v, true);
        let lo = v["yAxis"]["min"].as_f64().unwrap();
        let hi = v["yAxis"]["max"].as_f64().unwrap();
        assert_eq!(lo, 119.0, "min should read as a round number");
        assert_eq!(hi, 145.0, "max should read as a round number");
        // And still contain the data after rounding.
        assert!(lo < 122.4342086864 && hi > 141.7344641525);
    }

    /// Rounding must work at both ends of a wide axis, where one bound needs
    /// three decimal places and the other needs none.
    #[test]
    fn log_axis_bounds_round_across_orders_of_magnitude() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "difficulty", "data": [
                [1, 0.2213162147], [2, 159.09249284]
            ]}]
        });
        apply_log_scale(&mut v, true);
        let lo = v["yAxis"]["min"].as_f64().unwrap();
        let hi = v["yAxis"]["max"].as_f64().unwrap();
        assert_eq!(lo, 0.216);
        assert_eq!(hi, 163.0);
        assert!(lo < 0.2213162147 && hi > 159.09249284);
    }

    /// Rounding away from the data, never into it: a bound that crops a point
    /// would hide data to tidy a label.
    #[test]
    fn rounding_never_crops_the_data() {
        for (lo_raw, hi_raw) in
            [(1.0001, 9.9999), (0.00123, 0.00987), (7.77, 7_777_777.0)]
        {
            let lo = round_sig(lo_raw, 3, RoundDir::Down);
            let hi = round_sig(hi_raw, 3, RoundDir::Up);
            assert!(lo <= lo_raw, "{lo} must not exceed {lo_raw}");
            assert!(hi >= hi_raw, "{hi} must not fall short of {hi_raw}");
        }
    }

    /// Sixteen years of history labelled `2009-10-05` at nine-month intervals
    /// is unreadable. The format follows the span, not the range name, since
    /// the builder is handed a slice and a custom window can be any length.
    #[test]
    fn date_labels_follow_the_span() {
        let all: Vec<String> =
            ["2009-01-03".into(), "2026-09-11".into()].into();
        assert_eq!(date_label_sentinel(&all), DATE_LABEL_MONTH_YEAR);

        let quarter: Vec<String> =
            ["2026-06-13".into(), "2026-09-11".into()].into();
        assert_eq!(date_label_sentinel(&quarter), DATE_LABEL_DAY_MONTH);

        // Two years exactly stays day-level; past it, months.
        let two_years: Vec<String> =
            ["2024-09-11".into(), "2026-09-10".into()].into();
        assert_eq!(date_label_sentinel(&two_years), DATE_LABEL_DAY_MONTH);
        let over: Vec<String> =
            ["2024-09-11".into(), "2026-10-11".into()].into();
        assert_eq!(date_label_sentinel(&over), DATE_LABEL_MONTH_YEAR);

        // Empty or unparseable must not panic, and falls back to day level.
        assert_eq!(date_label_sentinel(&[]), DATE_LABEL_DAY_MONTH);
        let junk: Vec<String> = ["nope".into(), "also nope".into()].into();
        assert_eq!(date_label_sentinel(&junk), DATE_LABEL_DAY_MONTH);
    }

    /// The axis `data` must stay as ISO dates whatever the label format: the
    /// key figures read it back to date the peak and low, and the CSV export
    /// takes its first column from it.
    #[test]
    fn date_axis_keeps_iso_values_and_only_shortens_labels() {
        let cats: Vec<String> =
            ["2009-01-03".into(), "2026-09-11".into()].into();
        let axis = x_axis_for(true, &cats);
        assert_eq!(axis["data"][0], "2009-01-03");
        assert_eq!(axis["data"][1], "2026-09-11");
        assert_eq!(axis["axisLabel"]["formatter"], DATE_LABEL_MONTH_YEAR);
        assert_eq!(axis["axisLabel"]["hideOverlap"], json!(true));
    }

    /// Difficulty has no natural scaled unit: 1 at genesis against 1.6e14
    /// today. Dividing by 1e12 and calling the axis "T" put every historical
    /// value below 1, where the floor label rendered as "0.0000000001".
    #[test]
    fn difficulty_uses_raw_values_with_an_si_axis() {
        let opt = difficulty_chart_daily(&[
            daily_with_difficulty("2009-01-03", 1.0),
            daily_with_difficulty("2026-09-11", 1.6e14),
        ]);
        // Raw, not pre-divided: 1e12 would put genesis at 1e-12.
        let data = &opt["series"][0]["data"];
        assert_eq!(data[0].as_f64().unwrap(), 1.0);
        assert_eq!(data[1].as_f64().unwrap(), 1.6e14);
        // And the axis asks the client to abbreviate.
        assert_eq!(opt["yAxis"]["axisLabel"]["formatter"], SI_AXIS_SENTINEL);
        assert_eq!(opt["yAxis"]["name"], "Difficulty");
    }

    /// A log axis must not ask for a tick count it cannot honour.
    ///
    /// `splitNumber: 8` divided the exponent range, and since these bounds fit
    /// the data they never align to decades: 7.36 decades over 8 splits put
    /// every tick at a fractional power of ten, which ECharts declines to
    /// label. Avg Fee/Tx over ALL therefore carried one interior label across
    /// seven decades, having asked for eight.
    #[test]
    fn a_log_axis_does_not_ask_for_fractional_decade_ticks() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "d", "data": [[1, 1.0], [2, 1.6e14]]}]
        });
        apply_log_scale(&mut v, true);
        assert!(
            v["yAxis"].get("splitNumber").is_none(),
            "let ECharts step by whole decades"
        );
    }

    /// Every log axis gets an abbreviating formatter, because ECharts prints
    /// an explicit bound verbatim and its own idea of that bound is not the
    /// number we set: the max comes back as 10^log10(max), so 22,900,000
    /// rendered as "22,899,999.9999999739". No amount of rounding in Rust
    /// reaches that, since the corruption happens after it.
    #[test]
    fn a_log_axis_always_has_a_label_formatter() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "fees", "data": [[1, 1.0], [2, 22_373_000.0]]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(
            v["yAxis"]["axisLabel"]["formatter"], SI_AXIS_SENTINEL,
            "an unformatted log axis prints the raw float"
        );
        // And it is given back, or the linear view inherits abbreviations the
        // builder never asked for.
        apply_log_scale(&mut v, false);
        assert!(
            v["yAxis"]["axisLabel"].get("formatter").is_none(),
            "our formatter must not persist onto the linear axis"
        );
    }

    /// A builder that chose its own formatter keeps it. A percentage or a
    /// unit-suffixed axis is a deliberate decision, not an omission.
    #[test]
    fn a_log_axis_keeps_the_builders_own_formatter() {
        let mut v = json!({
            "yAxis": {
                "type": "value",
                "axisLabel": {"color": "#d4d4d4", "formatter": "{value}%"}
            },
            "series": [{"name": "pct", "data": [[1, 0.5], [2, 40.0]]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["axisLabel"]["formatter"], "{value}%");
        apply_log_scale(&mut v, false);
        assert_eq!(
            v["yAxis"]["axisLabel"]["formatter"], "{value}%",
            "removing ours must not remove theirs"
        );
    }

    /// A narrow range must still get a log axis, fitted to its own data.
    /// Glassnode offers log from 7d to ALL and so should this; an earlier
    /// version refused under two decades, which was really covering for
    /// decade-rounded bounds.
    #[test]
    fn log_scale_fits_a_narrow_range_instead_of_refusing_it() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "difficulty", "data": [
                [1, 140.0], [2, 152.0], [3, 160.0]
            ]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["type"], "log");
        // Fitted to 140..160 with a little padding, nowhere near the
        // enclosing 100..1000 decades that pinned the series to the floor.
        let lo = v["yAxis"]["min"].as_f64().unwrap();
        let hi = v["yAxis"]["max"].as_f64().unwrap();
        assert!(lo > 135.0 && lo < 140.0, "min was {lo}");
        assert!(hi > 160.0 && hi < 165.0, "max was {hi}");
    }

    /// One distinct value has no ratio to pad, so it gets a decade either
    /// side rather than an axis of zero height.
    #[test]
    fn log_scale_handles_a_single_distinct_value() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "flat", "data": [[1, 50.0], [2, 50.0]]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["min"], json!(5.0));
        assert_eq!(v["yAxis"]["max"], json!(500.0));
    }

    /// ECharts picks log decades from its split count, not from the data, so
    /// difficulty's top tick landed eight decades above its real maximum and
    /// pushed the whole series into a third of the plot.
    /// Wide data must fill the plot rather than sitting under eight decades
    /// of empty space, which is what ECharts' own split choice produced.
    #[test]
    fn log_scale_fits_a_range_spanning_many_decades() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "difficulty", "data": [
                [1, 1.5e-12], [2, 1.4e2]
            ]}]
        });
        apply_log_scale(&mut v, true);
        let lo = v["yAxis"]["min"].as_f64().unwrap();
        let hi = v["yAxis"]["max"].as_f64().unwrap();
        assert!(lo < 1.5e-12 && lo > 1.4e-12, "min was {lo}");
        assert!(hi > 1.4e2 && hi < 1.5e2, "max was {hi}");

        // Back to linear must drop the bounds, or they crop the linear view.
        apply_log_scale(&mut v, false);
        assert_eq!(v["yAxis"]["type"], "value");
        assert!(v["yAxis"].get("min").is_none());
        assert!(v["yAxis"].get("max").is_none());
    }

    /// The overlay's own series must not widen the metric's axis.
    #[test]
    fn log_bounds_ignore_the_right_axis_series() {
        let mut v = json!({
            "yAxis": [{"type": "value"}, {"type": "value"}],
            "series": [
                {"name": "count", "data": [[1, 0.5], [2, 80.0]]},
                {"name": "price", "yAxisIndex": 1, "data": [[1, 90000.0]]}
            ]
        });
        apply_log_scale(&mut v, true);
        // Padded data extent, 80 * 1.02, and crucially not widened by the
        // 90000 sitting on the right axis.
        let hi = v["yAxis"][0]["max"].as_f64().unwrap();
        assert!(hi > 80.0 && hi < 85.0, "max was {hi}");
    }

    /// A line of a quantity far from zero must fit its data, or the detail is
    /// thrown away: difficulty over 3M sat between 120 T and 142 T on a
    /// 0-to-150 axis and read as a flat line.
    #[test]
    fn value_axis_fits_a_line_chart() {
        let opt = build_option(json!({
            "yAxis": y_axis("T"),
            "series": [{"name": "Difficulty", "type": "line", "data": [
                [1, 122.0], [2, 142.0]
            ]}]
        }));
        assert_eq!(opt["yAxis"]["scale"], json!(true));
    }

    /// Bar length encodes magnitude, so a cut axis misstates ratios. This is
    /// the exclusion where fitting would mislead rather than merely differ.
    #[test]
    fn value_axis_stays_zero_based_for_bars() {
        let opt = build_option(json!({
            "yAxis": y_axis("Count"),
            "series": [{"name": "c", "type": "bar", "data": [[1, 5.0]]}]
        }));
        assert!(opt["yAxis"].get("scale").is_none());
    }

    /// The stack is a sum measured from zero, so it keeps zero even though
    /// its series are lines.
    #[test]
    fn value_axis_stays_zero_based_for_stacks() {
        let opt = build_option(json!({
            "yAxis": y_axis("Outputs"),
            "series": [
                {"name": "a", "type": "line", "stack": "t", "data": [[1, 1.0]]},
                {"name": "b", "type": "line", "stack": "t", "data": [[1, 2.0]]}
            ]
        }));
        assert!(opt["yAxis"].get("scale").is_none());
    }

    /// 0 to 100 is the frame that gives a percentage meaning.
    #[test]
    fn value_axis_stays_zero_based_for_percentages() {
        let mut axis = y_axis("%");
        axis["max"] = json!(100.0);
        let opt = build_option(json!({
            "yAxis": axis,
            "series": [{"name": "share", "type": "line", "data": [[1, 40.0]]}]
        }));
        assert!(opt["yAxis"].get("scale").is_none());
    }

    /// A builder that set its own bounds meant them; fitting would override
    /// a deliberate frame.
    #[test]
    fn value_axis_respects_bounds_the_builder_already_set() {
        let mut axis = y_axis("min");
        axis["min"] = json!(0.0);
        let opt = build_option(json!({
            "yAxis": axis,
            "series": [{"name": "interval", "type": "line", "data": [[1, 9.0]]}]
        }));
        assert!(opt["yAxis"].get("scale").is_none());
    }

    /// Fitting must not touch the overlay's axis, and a no-data chart with no
    /// series must not gain one.
    #[test]
    fn value_axis_fitting_leaves_other_axes_and_empty_charts_alone() {
        let opt = build_option(json!({
            "yAxis": [y_axis("Count"), y_axis("Price")],
            "series": [{"name": "c", "type": "line", "data": [[1, 5.0]]}]
        }));
        assert_eq!(opt["yAxis"][0]["scale"], json!(true));
        assert!(opt["yAxis"][1].get("scale").is_none());

        let empty = build_option(json!({"yAxis": y_axis("Count")}));
        assert!(empty["yAxis"].get("scale").is_none());
    }

    /// A stacked chart's bands are read as a sum, so a log axis makes their
    /// heights mean nothing.
    #[test]
    fn log_scale_refuses_stacked_charts() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [
                {"name": "a", "stack": "t", "data": [[1, 5.0]]},
                {"name": "b", "stack": "t", "data": [[1, 5.0]]}
            ]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["type"], "value", "stacked must stay linear");
    }

    /// A 0-to-100 axis is already bounded; a log version only compresses the
    /// top of it.
    #[test]
    fn log_scale_refuses_a_percentage_axis() {
        let mut v = json!({
            "yAxis": {"type": "value", "max": 100.0},
            "series": [{"name": "share", "data": [[1, 40.0]]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["type"], "value");
    }

    /// Zero and negative points do not refuse a log axis, they annotate it.
    /// ECharts cannot plot them, so the chart says which and how many rather
    /// than the toggle appearing to do nothing. Fees on an empty block are
    /// zero and net UTXO growth goes negative, so this is a normal state.
    #[test]
    fn log_scale_switches_and_reports_points_it_cannot_plot() {
        let mut v = json!({
            "graphic": [{"type": "text", "style": {"text": "wehodlbtc"}}],
            "yAxis": {"type": "value"},
            "series": [{"name": "fees", "data": [
                [1, 3.0], [2, 0.0], [3, -1.0], [4, 900.0]
            ]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["type"], "log", "must still switch");

        let note = v["graphic"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] == "log-scale-notice")
            .expect("expected a notice");
        let text = note["style"]["text"].as_str().unwrap();
        assert!(text.starts_with("2 points"), "got {text:?}");

        // Bounds come from the positive points alone.
        let lo = v["yAxis"]["min"].as_f64().unwrap();
        assert!(lo > 2.9 && lo < 3.0, "min was {lo}");

        // The unplottable points are gone from the series, so the gap the
        // notice describes is a real gap rather than a mis-drawn one.
        let data = v["series"][0]["data"].as_array().unwrap();
        assert_eq!(data.len(), 4, "positions must be preserved");
        assert!(
            data[1][1].is_null(),
            "the zero must be a gap: {:?}",
            data[1]
        );
        assert!(data[2][1].is_null(), "the negative must be a gap");
        assert_eq!(data[1][0], 2, "the timestamp stays, so the tooltip knows");
        assert_eq!(data[3][1], 900.0, "a plottable point is untouched");

        // Notices must not stack. Seeded as if a previous render left one,
        // because `set_log_notice` removes before it adds.
        let mut stale = json!({
            "graphic": [
                {"type": "text", "style": {"text": "wehodlbtc"}},
                {"id": "log-scale-notice", "type": "text",
                 "style": {"text": "99 points are stale"}}
            ],
            "yAxis": {"type": "value"},
            "series": [{"name": "fees", "data": [[1, 3.0], [2, 0.0]]}]
        });
        apply_log_scale(&mut stale, true);
        let notices: Vec<&serde_json::Value> = stale["graphic"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g["id"] == "log-scale-notice")
            .collect();
        assert_eq!(notices.len(), 1, "stacked: {notices:?}");
        assert!(notices[0]["style"]["text"]
            .as_str()
            .unwrap()
            .starts_with("1 point"));

        // And going back to linear clears it.
        apply_log_scale(&mut stale, false);
        assert!(!stale["graphic"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["id"] == "log-scale-notice"));
    }

    /// A bar on one axis must not decide the other axis's scale.
    ///
    /// The refusal exists because a bar's length is read from zero and a log
    /// axis has no zero, so every bar runs from the floor and the plot fills
    /// solid. Applied to the whole option rather than to one axis, it meant a
    /// bar **comparison** silently disabled the metric's own log scale:
    /// Difficulty with `?compare=max-tx-fee&scale=log` reported Log as
    /// selected and rendered linear.
    #[test]
    fn a_bar_comparison_does_not_disable_the_metrics_log_axis() {
        let mut v = json!({
            "yAxis": [{"type": "value"}, {"type": "value"}],
            "series": [
                {"name": "Difficulty", "type": "line",
                 "yAxisIndex": 0, "data": [[1, 1.0e12], [2, 1.4e14]]},
                {"name": "Max Tx Fee", "type": "bar",
                 "yAxisIndex": 1, "data": [[1, 0.01], [2, 0.38]]}
            ]
        });
        apply_scales_with(&mut v, true, false);
        assert_eq!(v["yAxis"][0]["type"], "log", "the line's own axis");
        assert_eq!(v["yAxis"][1]["type"], "value", "untouched");
    }

    /// And the reverse, which had no refusal at all: the right axis was free
    /// to put its bars on a log scale and fill solid.
    #[test]
    fn the_right_axis_refuses_a_log_scale_for_its_own_bars() {
        let mut v = json!({
            "yAxis": [{"type": "value"}, {"type": "value"}],
            "series": [
                {"name": "Difficulty", "type": "line",
                 "yAxisIndex": 0, "data": [[1, 1.0e12], [2, 1.4e14]]},
                {"name": "Max Tx Fee", "type": "bar",
                 "yAxisIndex": 1, "data": [[1, 0.01], [2, 0.38]]}
            ]
        });
        apply_scales_with(&mut v, false, true);
        assert_eq!(v["yAxis"][1]["type"], "value", "bars refuse log");
        assert_eq!(v["yAxis"][0]["type"], "value", "and not by accident");

        // A line on the right axis is the ordinary case and still switches.
        let mut ok = json!({
            "yAxis": [{"type": "value"}, {"type": "value"}],
            "series": [
                {"name": "Difficulty", "type": "line",
                 "yAxisIndex": 0, "data": [[1, 1.0e12], [2, 1.4e14]]},
                {"name": "Price", "type": "line",
                 "yAxisIndex": 1, "data": [[1, 0.05], [2, 120000.0]]}
            ]
        });
        apply_scales_with(&mut ok, false, true);
        assert_eq!(ok["yAxis"][1]["type"], "log");
    }

    /// A percentage metric must refuse for itself without dragging down an
    /// unbounded comparison beside it.
    #[test]
    fn a_bounded_percentage_axis_refuses_only_itself() {
        let mut v = json!({
            "yAxis": [
                {"type": "value", "max": 100.0},
                {"type": "value"}
            ],
            "series": [
                {"name": "SegWit %", "type": "line",
                 "yAxisIndex": 0, "data": [[1, 5.0], [2, 90.0]]},
                {"name": "Tx Count", "type": "line",
                 "yAxisIndex": 1, "data": [[1, 200.0], [2, 4000.0]]}
            ]
        });
        apply_scales_with(&mut v, true, true);
        assert_eq!(v["yAxis"][0]["type"], "value", "0-100 stays linear");
        assert_eq!(
            v["yAxis"][1]["type"], "log",
            "the comparison still gets log"
        );
    }

    /// Both axes dropping points yields one notice counting both, which is
    /// what the single-chart view was missing when it called the two axis
    /// functions itself.
    #[test]
    fn one_notice_counts_both_axes() {
        let mut v = json!({
            "graphic": [],
            "yAxis": [{"type": "value"}, {"type": "value"}],
            "series": [
                {"name": "metric", "yAxisIndex": 0,
                 "data": [[1, 0.0], [2, 5.0]]},
                {"name": "compared", "yAxisIndex": 1,
                 "data": [[1, -2.0], [2, 8.0]]}
            ]
        });
        apply_scales_with(&mut v, true, true);
        let notices: Vec<&serde_json::Value> = v["graphic"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g["id"] == "log-scale-notice")
            .collect();
        assert_eq!(notices.len(), 1, "one notice, not one per axis");
        assert!(
            notices[0]["style"]["text"]
                .as_str()
                .unwrap()
                .starts_with("2 points"),
            "got {:?}",
            notices[0]["style"]["text"]
        );
    }

    /// The singular wording, since "1 points" reads as a bug.
    #[test]
    fn log_notice_is_singular_for_one_point() {
        let mut v = json!({
            "graphic": [],
            "yAxis": {"type": "value"},
            "series": [{"name": "x", "data": [[1, 5.0], [2, 0.0], [3, 500.0]]}]
        });
        apply_log_scale(&mut v, true);
        let text = v["graphic"][0]["style"]["text"].as_str().unwrap();
        assert!(text.starts_with("1 point is"), "got {text:?}");
    }

    /// A strictly positive series is the case log exists for.
    #[test]
    fn log_scale_accepts_a_strictly_positive_series() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "difficulty", "data": [[1, 1.0], [2, 1e12]]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v_type(&v), "log");
    }

    /// An overlay on the right axis keeps its own linear scale, so a zero in
    /// the price series must not veto log on the metric the page is about.
    #[test]
    fn log_scale_ignores_non_positive_points_on_the_overlay_axis() {
        let mut v = json!({
            "yAxis": [{"type": "value"}, {"type": "value"}],
            "series": [
                {"name": "count", "data": [[1, 1.0], [2, 500.0]]},
                {"name": "price", "yAxisIndex": 1, "data": [[1, 0.0]]}
            ]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"][0]["type"], "log");
        assert_eq!(v["yAxis"][1]["type"], "value");
    }

    fn daily_with_difficulty(
        date: &str,
        d: f64,
    ) -> crate::stats::types::DailyAggregate {
        crate::stats::types::DailyAggregate {
            date: date.to_string(),
            avg_difficulty: d,
            ..Default::default()
        }
    }

    fn v_type(v: &serde_json::Value) -> &str {
        v["yAxis"]["type"].as_str().unwrap_or("")
    }

    /// The unit tests above exercise `x_axis_for` directly, which proves the
    /// helper and nothing about whether a chart reaches it. This goes through
    /// a real builder, so a daily chart that hand-rolls its category axis
    /// instead of calling the helper fails here.
    #[test]
    fn a_real_daily_chart_gets_calendar_aligned_ticks() {
        let days: Vec<_> = daily_categories("2009-01-03", 6456)
            .iter()
            .enumerate()
            // Rising, positive and never flat, so nothing else refuses.
            .map(|(i, d)| daily_with_difficulty(d, 1.0 + i as f64 * 1e9))
            .collect();
        let opt = super::network::difficulty_chart_daily(&days);
        let label = &opt["xAxis"]["axisLabel"];
        assert_eq!(
            label["interval"], CALENDAR_TICK_SENTINEL,
            "the difficulty chart does not route through x_axis_for"
        );
        let ticks = label[CALENDAR_TICK_DATA_KEY]
            .as_array()
            .expect("indices alongside the sentinel");
        let cats = opt["xAxis"]["data"].as_array().expect("categories");
        // Every label the axis will draw names a January, because sixteen
        // years lands on the twelve-month stride.
        for t in ticks {
            let date = cats[t.as_u64().unwrap() as usize].as_str().unwrap();
            assert!(
                date.starts_with(&format!("{}-01", &date[..4])),
                "tick on {date} is not a January"
            );
        }
        assert!(ticks.len() >= 6, "too few labels to read the axis by");
    }

    /// The cache key must move with the flag. If it does not, toggling the
    /// axis serves the previous render, which is the stale-chart defect
    /// fix/chart-correctness fixed.
    #[test]
    fn log_scale_changes_the_overlay_cache_key() {
        let linear = OverlayFlags::default();
        let logged = OverlayFlags {
            log_scale: true,
            ..Default::default()
        };
        assert_ne!(
            linear.cache_key(),
            logged.cache_key(),
            "a chart cached linear would be served for a log request"
        );
    }

    /// Single-axis charts carry `yAxis` as an object, not an array.
    #[test]
    fn log_scale_handles_a_bare_axis_object() {
        let mut v = json!({
            "yAxis": {"type": "value"},
            "series": [{"name": "x", "data": [[1, 2.0], [2, 2000.0]]}]
        });
        apply_log_scale(&mut v, true);
        assert_eq!(v["yAxis"]["type"], "log");
    }

    /// A category axis has no scale to change, and a chart with no yAxis at
    /// all (a donut) must not panic.
    #[test]
    fn log_scale_leaves_category_and_missing_axes_alone() {
        let mut cat = json!({
            "yAxis": {"type": "category", "data": ["a"]},
            "series": [{"name": "x", "data": [[1, 2.0], [2, 2000.0]]}]
        });
        apply_log_scale(&mut cat, true);
        assert_eq!(cat["yAxis"]["type"], "category");

        let mut none = json!({"series": []});
        apply_log_scale(&mut none, true);
        assert!(none.get("yAxis").is_none());
    }
    use super::*;

    // -----------------------------------------------------------------------
    // x_axis_for
    // -----------------------------------------------------------------------

    /// The daily x-axis must not force which labels ECharts shows.
    ///
    /// The last label was clipped at the grid edge, and two rounds of trying
    /// to fix it with label options made it worse, so this pins the approach
    /// that works: leave label selection to ECharts and give the grid enough
    /// right margin (see [`GRID_RIGHT`]) that a centred final label fits.
    ///
    /// Why not the label options, both measured over 21 width/range
    /// combinations by SSR render:
    ///
    /// - `alignMaxLabel: "right"` alone right-aligns the last label so it
    ///   cannot clip, but shifting its box toward its neighbour makes ECharts
    ///   treat it as overlapping and **drop** it. The axis then ended earlier
    ///   than before the fix.
    /// - Adding `showMaxLabel: true` forces it back, but that switches off
    ///   ECharts' overlap avoidance for that label, so it collides with its
    ///   neighbour instead. Observed on a 1-year range as two dates printed
    ///   on top of each other.
    /// - `alignMinLabel: "left"` is the same trap at the other end: it
    ///   dropped the FIRST label (2026-06-11 became 2026-06-21).
    ///
    /// Not forcing anything keeps ECharts' overlap avoidance intact, which
    /// uses real font metrics rather than an estimate, so collisions are
    /// impossible by construction. The margin then removes the only remaining
    /// failure, the clip.
    #[test]
    fn daily_axis_leaves_label_selection_to_echarts() {
        let daily =
            x_axis_for(true, &["2026-09-07".into(), "2026-09-08".into()]);
        for opt in ["showMaxLabel", "alignMaxLabel", "alignMinLabel"] {
            assert!(
                daily["axisLabel"][opt].is_null(),
                "{opt} overrides ECharts' overlap avoidance; the clip is \
                 solved with GRID_RIGHT instead"
            );
        }
        assert_eq!(daily["axisLabel"]["color"], "#d4d4d4");
    }

    /// `chart_defaults` must read the constant rather than repeat a literal,
    /// or the `unwrap_or(GRID_RIGHT)` fallbacks stop matching the real margin.
    /// The margin's minimum is enforced at compile time beside the constant.
    #[test]
    fn chart_defaults_uses_the_grid_right_constant() {
        assert_eq!(
            chart_defaults()["grid"]["right"],
            GRID_RIGHT,
            "chart_defaults must use the constant, not a literal"
        );
    }

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

    /// A rolling-sum rewrite of `moving_average` was tried and rejected.
    ///
    /// This is O(n*w), and the common case is a 144-wide window over a 4,320
    /// point series, so about 18.7M f64 additions across a 30-chart page. That
    /// sounds worth fixing until measured: roughly 20ms total in WASM, under
    /// 1ms per chart, against the 50-150ms per chart that ECharts setOption
    /// already costs (see the render budget in stats.js).
    ///
    /// The rolling form subtracts the departing element instead of re-summing,
    /// which drifts. Against a 500-point series it disagreed with this
    /// implementation on 2 of ~350 values, each by exactly 0.001: the drift
    /// landed on the rounding boundary and flipped the third decimal. So the
    /// rewrite changed published chart values to save under a millisecond.
    /// If it is ever revisited, keep this test and make it pass.
    #[test]
    fn ma_agrees_with_a_direct_window_sum() {
        let data: Vec<f64> = (0..500)
            .map(|i| ((i * 7919 % 613) as f64) / 3.0 + (i as f64) * 0.25)
            .collect();
        for window in [1usize, 2, 3, 7, 144, 200, 499, 500] {
            let expected: Vec<Option<f64>> = (0..data.len())
                .map(|i| {
                    if i + 1 < window {
                        None
                    } else {
                        let sum: f64 = data[i + 1 - window..=i].iter().sum();
                        Some(round_plot(sum / window as f64))
                    }
                })
                .collect();
            assert_eq!(
                moving_average(&data, window),
                expected,
                "window {window}"
            );
        }
    }

    #[test]
    fn ma_window_zero_is_all_none() {
        assert_eq!(moving_average(&[1.0, 2.0], 0), vec![None, None]);
    }

    #[test]
    fn ma_rounds_without_destroying_small_values() {
        // 1/3 = 0.33333..., kept to six significant figures.
        let result = moving_average(&[0.0, 0.0, 1.0], 3);
        assert_eq!(result[2], Some(0.333333));

        // The case three decimal places got wrong. A block carrying 19,818
        // sats is 0.00019818 BTC, and the average of a window holding it must
        // still be a number: rounding to 0.000 deleted a real reading and a
        // log axis then dropped it as unplottable.
        let btc = 19_818.0 / 100_000_000.0;
        let avg = moving_average(&[btc, btc, btc], 3);
        assert_eq!(avg[2], Some(btc), "a real fee must survive the average");
        assert!(avg[2].unwrap() > 0.0);
    }

    // -----------------------------------------------------------------------
    // moving_average_over_gaps
    // -----------------------------------------------------------------------

    /// A named window has to mean what it says.
    ///
    /// The independently computed case: one reading of 100, eight gaps, then
    /// six zeroes. The last seven positions hold nothing but zeroes, so a
    /// seven-position mean is 0. Filtering the gaps out first and averaging
    /// the survivors gives 100/7 = 14.2857, because it reaches fourteen
    /// positions back to find its seventh reading. That is the defect this
    /// replaces, and it affected the 7-day windows on SegWit Adoption and
    /// Avg Fee/Tx and the 144-block windows on TPS and Avg Fee/Tx.
    #[test]
    fn a_named_window_does_not_reach_past_its_own_length() {
        let mut readings: Vec<Option<f64>> = vec![Some(100.0)];
        readings.extend(std::iter::repeat_n(None, 8));
        readings.extend(std::iter::repeat_n(Some(0.0), 6));

        let ma = moving_average_over_gaps(&readings, 7);
        assert_eq!(
            ma.last().copied().flatten(),
            Some(0.0),
            "the last seven positions hold only zeroes; 14.2857 would mean \
             the 100 was pulled in from fourteen positions back"
        );
    }

    /// Short gaps are bridged by averaging what the window does hold, which
    /// keeps the line continuous without inventing values for the gaps.
    #[test]
    fn a_window_averages_the_readings_it_contains() {
        let readings =
            vec![Some(10.0), None, Some(20.0), None, None, Some(30.0)];
        let ma = moving_average_over_gaps(&readings, 3);
        // Positions 3..=5 hold one reading, 30, so the mean is 30.
        assert_eq!(ma[5], Some(30.0));
        // Positions 0..=2 hold 10 and 20.
        assert_eq!(ma[2], Some(15.0));
        // A window with nothing in it is not zero.
        assert_eq!(
            moving_average_over_gaps(&[None, None], 2),
            vec![None, None]
        );
    }

    /// With no gaps it must agree with the plain moving average, or the two
    /// are silently different functions.
    #[test]
    fn with_no_gaps_it_matches_the_plain_moving_average() {
        let data: Vec<f64> = (0..60)
            .map(|i| ((i * 7919 % 613) as f64) / 3.0 + (i as f64) * 0.25)
            .collect();
        let opt: Vec<Option<f64>> = data.iter().map(|v| Some(*v)).collect();
        for window in [1usize, 3, 7, 30] {
            let plain = moving_average(&data, window);
            let gapped = moving_average_over_gaps(&opt, window);
            for i in 0..data.len() {
                match (plain[i], gapped[i]) {
                    // The plain version emits None until the window fills;
                    // this one averages the shorter prefix instead, which is
                    // the deliberate difference and the only one.
                    (None, Some(_)) => assert!(i + 1 < window),
                    (Some(a), Some(b)) => assert!(
                        (a - b).abs() < 1e-9,
                        "window {window} position {i}: {a} against {b}"
                    ),
                    other => panic!("window {window} position {i}: {other:?}"),
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // round_plot
    // -----------------------------------------------------------------------

    /// The contract is "never turn a real reading into nothing", stated in the
    /// units the charts actually plot rather than in terms of the scaling.
    #[test]
    fn round_plot_keeps_small_readings() {
        // The genesis block, plotted in megabytes.
        let genesis_mb = 285.0 / 1_000_000.0;
        assert_eq!(round_plot(genesis_mb), 0.000285);

        // A block carrying 19,818 sats, plotted in BTC.
        assert_eq!(round_plot(19_818.0 / 100_000_000.0), 0.00019818);

        // One satoshi in BTC, which is the smallest reading that exists.
        assert!(round_plot(1.0 / 100_000_000.0) > 0.0);

        // The first day of the chain, plotted in gigabytes.
        assert!(round_plot(285.0 * 100.0 / 1_000_000_000.0) > 0.0);
    }

    /// And it still has to round, or it is not doing its job: the reason this
    /// exists at all is payload size.
    #[test]
    fn round_plot_still_rounds() {
        assert_eq!(round_plot(1.0 / 3.0), 0.333333);
        assert_eq!(round_plot(1.234_567_891), 1.23457);
        // Above six significant figures there is nothing to keep, so a whole
        // megabyte count stays whole rather than gaining float noise.
        assert_eq!(round_plot(3_998_274.6), 3_998_275.0);
        assert_eq!(round_plot(0.0), 0.0);
        assert_eq!(round_plot(-0.000_285), -0.000285);
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
    fn halving_era_from_date_matches_height_eras() {
        assert_eq!(halving_era_for_date("2009-01-03"), 0);
        assert_eq!(halving_era_for_date("2012-11-27"), 0);
        assert_eq!(halving_era_for_date("2012-11-28"), 1);
        assert_eq!(halving_era_for_date("2016-07-09"), 2);
        assert_eq!(halving_era_for_date("2020-05-11"), 3);
        assert_eq!(halving_era_for_date("2024-04-20"), 4);
        assert_eq!(halving_era_for_date("2026-09-08"), 4);
    }

    #[test]
    fn daily_subsidy_agrees_with_block_subsidy_at_each_halving() {
        // The date form and the height form must not drift apart.
        let pairs = [
            ("2009-01-03", 0u64),
            ("2012-11-28", 210_000),
            ("2016-07-09", 420_000),
            ("2020-05-11", 630_000),
            ("2024-04-20", 840_000),
        ];
        for (date, height) in pairs {
            let from_date = daily_subsidy_btc(date);
            let from_height = block_subsidy(height) as f64 / 100_000_000.0;
            assert_eq!(
                from_date, from_height,
                "date {date} and height {height} disagree on subsidy"
            );
        }
    }

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

    /// The toolbox box-select must never zoom the value axis.
    ///
    /// stats.js arms this brush on every desktop chart, so a plain drag on the
    /// plot area triggers it. With ECharts' default (both axes) that clips the
    /// top of a 100% stacked chart out of view while the tooltip keeps
    /// reporting the real values, which reads as missing data rather than as a
    /// zoom. Every chart inherits this from chart_defaults, so assert it here.
    #[test]
    fn toolbox_zoom_is_restricted_to_the_time_axis() {
        let opt = chart_defaults();
        let zoom = opt
            .get("toolbox")
            .and_then(|t| t.get("feature"))
            .and_then(|f| f.get("dataZoom"))
            .expect("toolbox exposes a dataZoom feature");
        assert_eq!(
            zoom.get("yAxisIndex").and_then(|v| v.as_str()),
            Some("none"),
            "box-select would otherwise zoom the value axis and hide stacked bands"
        );
    }

    #[test]
    fn build_option_preserves_defaults() {
        let opt = build_option(json!({
            "xAxis": { "type": "time" }
        }));
        // Should have chart_defaults fields
        assert!(opt.get("toolbox").is_some());
        assert!(opt.get("progressive").is_some());
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

    #[test]
    fn opt_data_array_writes_null_and_stays_valid_json() {
        let blocks = vec![test_block(1, 1_000), test_block(2, 2_000)];
        let raw = build_data_array_opt_f64(&blocks, |b| {
            (b.height == 1).then_some(12.5)
        });
        assert!(raw.contains(",null,"), "expected a null value: {raw}");

        // data_array_value swallows a parse error into an empty array, so a
        // malformed gap would blank the whole chart silently instead of
        // failing loudly. Parse it here so that cannot happen unnoticed.
        let parsed = data_array_value(&raw);
        let rows = parsed.as_array().expect("expected an array");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1], json!(12.5));
        assert!(rows[1][1].is_null());
        // Height survives on the gap row, so click-to-detail still resolves.
        assert_eq!(rows[1][2], json!(2));
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

    /// This test asserted `outputs - inputs = 1000` and passed for as long as
    /// the chart was wrong, because both it and the builder used the formula
    /// rather than the definition. `test_block` gives 100 OP_RETURN outputs,
    /// so the honest answer is 900: those hundred can never be spent and
    /// never enter the set.
    ///
    /// Worth keeping the note. A test written from the implementation cannot
    /// catch the implementation being wrong, and this one made a 2.6x error
    /// look verified for as long as it existed.
    #[test]
    fn utxo_growth_excludes_outputs_that_can_never_be_spent() {
        let blocks = vec![test_block(100, 1700000000)];
        let chart = tx_metrics::utxo_growth_chart(&blocks);
        let series = chart.get("series").unwrap().as_array().unwrap();
        let data = series[0].get("data").unwrap().as_array().unwrap();
        // outputs(5000) - op_return(100) - inputs(4000)
        let val = data[0].as_array().unwrap()[1].as_i64().unwrap();
        assert_eq!(val, 900);
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
