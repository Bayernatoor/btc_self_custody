//! Slug to chart lookup: the one place that knows which charts exist.
//!
//! The single-chart view at `/observatory/chart/:slug` can render any chart,
//! which needs a lookup from a URL slug to a builder plus enough metadata to
//! describe, unit-check and compare it. This is that lookup.
//!
//! # Adding a chart
//!
//! Add the `<ChartCard>` to its page as usual, then add one entry here with
//! the same `chart_id` minus the `chart-` prefix. Nothing else: `related` is
//! computed, and the route, KPIs and downloads all work off `source` and
//! `unit`.
//!
//! **You cannot forget.** `registry_covers_every_chart_on_every_page` parses
//! the five page sources and fails if either side holds an entry the other
//! lacks, and `registry_titles_match_the_pages` fails if a title drifts. A new
//! chart therefore breaks the build until it is registered, which is the point:
//! the drawer nav is a second hand-maintained list of the same charts, and a
//! stale entry there shipped a dangling link once already.
//!
//! # Why `Source` is an enum rather than one function pointer
//!
//! The builders are nearly uniform and the exceptions are real. 54 charts take
//! `&[BlockSummary]` with an optional `&[DailyAggregate]` variant. The other 9
//! need something else: `chain-size` needs disk size and a byte offset, `fees`
//! needs the BTC/sats toggle, `fullness-dist` and `time-dist` each have four
//! combinations (count or percent, per-block or from histogram buckets), and
//! four mining charts read a different resource entirely. Encoding that
//! variance keeps the odd cases in one `match` instead of spread across call
//! sites, and makes a new odd chart an explicit compile error rather than a
//! silently wrong render.

use serde_json::Value;

use crate::stats::types::{BlockSummary, DailyAggregate};

/// Which chart page an entry belongs to, and where "back" goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Network,
    Fees,
    Mining,
    Embedded,
    /// Which outputs have published a public key on chain.
    ///
    /// Its own category rather than a corner of Network because the question
    /// cuts across every script type and the framing has to travel with it:
    /// these charts count outputs **created**, never held, and nothing in
    /// them says when or whether any of it becomes exploitable.
    PostQuantum,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Network => "Network",
            Self::Fees => "Fees",
            Self::Mining => "Mining",
            Self::Embedded => "Embedded Data",
            Self::PostQuantum => "Post-Quantum",
        }
    }

    pub fn page_path(self) -> &'static str {
        match self {
            Self::Network => "/observatory/charts/network",
            Self::Fees => "/observatory/charts/fees",
            Self::Mining => "/observatory/charts/mining",
            Self::Embedded => "/observatory/charts/embedded",
            Self::PostQuantum => "/observatory/charts/post-quantum",
        }
    }
}

/// The dimension a chart's y axis measures, read off the `y_axis(..)` label the
/// builder already declares.
///
/// This exists for the compare rule: two charts share an axis only when they
/// share a unit. `Mixed` is `weekday` alone, which already plots counts on the
/// left and BTC on the right, so it has no free axis and must refuse both a
/// compare series and the price overlay.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Count,
    Btc,
    Sats,
    SatVb,
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Percent,
    /// The difference between two percentages, which is not a percentage.
    ///
    /// A share moving from 10% to 15% has risen 5 **percentage points**, not
    /// 5%: as a percentage of itself it rose 50%. Adoption Velocity plots
    /// exactly that difference, `current - previous` on two shares, and
    /// labelled its axis "% Change", so the one reading a reader is most
    /// likely to take was the wrong one.
    PercentagePoints,
    TxPerSec,
    /// Hashes per second. Declared `Count` before, which is what let the hash
    /// rate chart present an inferred rate as though it were a tally.
    HashesPerSecond,
    Minutes,
    Seconds,
    Difficulty,
    Ratio,
    Mixed,
}

/// How a resolution's points relate to the underlying per-block readings.
///
/// The site had one global sentence, "one point per day, averaged from every
/// block", applied to charts that plot daily totals, pooled ratios, cumulative
/// running sums and grouped summaries. It is correct for a minority of them.
/// `address-types` says "Daily average output types" and plots
/// `avg * block_count`, a total.
///
/// Declared rather than inferred, because nothing in the built option
/// distinguishes these: a daily total and a daily mean are both an array of
/// numbers on a category axis.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aggregation {
    /// One reading per block, plotted as it was measured.
    PerBlockObservation,
    /// The day's readings summed. `address-types` multiplies the stored mean
    /// back up by the block count to get here.
    DailyTotal,
    /// The unweighted mean of the day's per-block values, which is what the
    /// `avg_*` columns already hold.
    MeanOfPerBlockValues,
    /// A ratio formed from two daily totals, not the mean of per-block
    /// ratios. The two differ whenever the denominator varies between blocks,
    /// and the difference is large: two blocks with 1-of-1 and 0-of-9 matching
    /// transactions pool to 10% and average to 50%.
    RatioOfTotals,
    /// A running total across the selected window, so the first point is not
    /// the chain's beginning unless the window is.
    CumulativeInWindow,
    /// Derived over a trailing window, such as a moving average or a change
    /// across N blocks or days.
    WindowedDerived,
    /// A constant divided by the day's block count, so the value moves
    /// **inversely** with how many blocks the day held.
    ///
    /// Block Interval is the case and currently the only one: `1440 /
    /// block_count` minutes. It was declared `WindowedDerived`, which is
    /// wrong twice over. There is no window, and `expectation()` maps that
    /// variant to no prediction, so the chart was exempt from the scaling
    /// probe. Naming the shape lets the probe predict it: double the day's
    /// blocks and the interval halves.
    InverseOfDailyCount,
    /// Grouped into buckets or categories rather than positioned in time:
    /// histograms, donuts, era comparisons.
    GroupedSummary,
    /// The chart has no builder at this resolution.
    Unsupported,
}

/// How a number came to exist, kept separate from where it came from and from
/// how it was aggregated.
///
/// The distinction the site most needs and least had. Hash rate is inferred
/// from difficulty, UTXO growth is an estimate that omits coinbase outputs,
/// inscriptions are matched by a witness marker, and block size is read off
/// the block. A reader who can see which is which can calibrate; one who
/// cannot has to trust.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    /// Read from the block or the node and plotted, allowing deterministic
    /// unit conversion. A conversion alone does not make a reading an
    /// estimate.
    Measured,
    /// Derived arithmetically from measured inputs by a defined formula, with
    /// no unknowns: a share, a rate, a difference.
    Calculated,
    /// Derived using an assumption that may not hold, so the result is
    /// approximate even when the inputs are exact. Hash rate assumes a
    /// 600-second mean interval; the block total for fees is taken as coinbase
    /// value minus the scheduled subsidy, which a miner underclaiming the
    /// reward makes wrong.
    Estimated,
    /// Identified by a pattern match that can miss and can over-match.
    /// Inscription detection scans witness items for the Ordinals marker.
    HeuristicallyDetected,
}

impl Method {
    /// The short label a reader sees.
    pub fn label(self) -> &'static str {
        match self {
            Self::Measured => "Measured",
            Self::Calculated => "Calculated",
            Self::Estimated => "Estimated",
            Self::HeuristicallyDetected => "Detected",
        }
    }

    /// The one-line explanation behind that label. Every badge explains
    /// itself, or it is decoration.
    pub fn explanation(self) -> &'static str {
        match self {
            Self::Measured => "Read directly from the blocks my node stores.",
            Self::Calculated => {
                "Worked out from measured values by a fixed formula."
            }
            Self::Estimated => {
                "Worked out using an assumption that may not hold, so the \
                 result is approximate even where the inputs are exact."
            }
            Self::HeuristicallyDetected => {
                "Found by matching a pattern, which can miss some cases and \
                 wrongly include others."
            }
        }
    }
}

/// What one of a chart's measurements is, per resolution.
///
/// A chart is not always one quantity. `chain-size` plots measured block bytes
/// beside an estimated disk figure, and giving the pair a single badge would
/// be false for one of them. So the declaration is per measurement, and a
/// chart carries one or more.
#[derive(Clone, Copy)]
pub struct Measurement {
    /// Series name as the built option emits it, which is how a consumer ties
    /// this declaration to what is drawn.
    ///
    /// Empty means the declaration covers **every** series, because they are
    /// renderings of one quantity rather than separate measurements.
    /// `diff-ribbon` is seven moving averages of difficulty and
    /// `diff-adjustment` splits one retarget series by sign into Harder and
    /// Easier; in both, naming a series would imply the others measure
    /// something else.
    pub series: &'static str,
    /// The daily builder's name for the same series, where it differs.
    ///
    /// Empty means it does not. Three charts legitimately rename a series
    /// between resolutions because the quantity shifts with it: `subsidy-fees`
    /// draws "Fees" per block and "Avg Fees" daily, and both are accurate for
    /// what they plot. A single name would be wrong at one resolution, so the
    /// declaration carries the pair rather than forcing the builders to agree
    /// on a word that fits only one of them.
    pub series_daily: &'static str,
    /// What is counted or measured, in the chart's own units.
    pub quantity: &'static str,
    /// Method at per-block resolution.
    ///
    /// Per resolution, not per chart, because the two genuinely differ.
    /// `difficulty` is the example: per block it reads the value off the
    /// block, and daily it plots the day's mean, which is a blend of two
    /// epochs on the day a retarget lands. A single badge would be wrong at
    /// one of them.
    ///
    /// `diff-adjustment` was this field's original justification, while its
    /// daily arm reconstructed retargets from difficulty plateaus. It now
    /// reads the retarget blocks at both resolutions and is measured at both,
    /// which is the outcome to want: the field stays because the distinction
    /// is real, not because a chart is stuck on the wrong side of it.
    pub method_per_block: Method,
    /// Method at daily resolution.
    pub method_daily: Method,
    /// Aggregation at per-block resolution.
    pub per_block: Aggregation,
    /// Aggregation at daily resolution.
    pub daily: Aggregation,
    /// Numerator and denominator population, and what is excluded. Stated
    /// even when it is "every block in the window", because the exclusions
    /// are where the copy kept going wrong: coinbase, OP_RETURN outputs,
    /// unknown script types.
    pub population: &'static str,
}

impl Unit {
    pub fn label(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Btc => "BTC",
            Self::Sats => "sats",
            Self::SatVb => "sat/vB",
            Self::Bytes => "bytes",
            Self::Kilobytes => "kB",
            Self::Megabytes => "MB",
            Self::Gigabytes => "GB",
            Self::Percent => "%",
            Self::PercentagePoints => "pp",
            Self::TxPerSec => "tx/s",
            Self::HashesPerSecond => "hashes/s",
            Self::Minutes => "minutes",
            Self::Seconds => "seconds",
            // No unit multiplier: the difficulty charts plot the raw protocol
            // value and abbreviate it on the axis, because dividing by 1e12
            // put every value before 2013 below 1 where the labels degenerate
            // into leading zeros.
            Self::Difficulty => "difficulty",
            Self::Ratio => "ratio",
            Self::Mixed => "mixed",
        }
    }

    /// Whether adding this unit up across a window yields the same unit.
    ///
    /// The distinction is extensive against intensive. Counts, byte sizes and
    /// currency amounts accumulate: a month's outputs is the sum of its
    /// blocks' outputs, in outputs. Shares, rates, ratios and levels do not:
    /// summing 4,603 percentages gives 460,300%, which is not a percentage of
    /// anything, and the right figure for a share over a window is a ratio of
    /// totals rather than a total of ratios.
    ///
    /// **Shape cannot stand in for this.** `all-embedded-share` is a
    /// `StackedAbsolute` chart declaring `Percent`, so reading "is this
    /// stacked" as "can I total it" would sum shares on exactly one of the
    /// fifteen stacked charts and look right on the other fourteen.
    ///
    /// A ratio of totals is not recoverable from a percentage chart's built
    /// option anyway, which is why the key-figures rail reports no range
    /// total for these rather than a mean of shares wearing the same label:
    /// `Aggregation::RatioOfTotals` exists in this file because the two are
    /// different quantities.
    pub fn totals_over_a_range(self) -> bool {
        match self {
            Self::Count
            | Self::Btc
            | Self::Sats
            | Self::Bytes
            | Self::Kilobytes
            | Self::Megabytes
            | Self::Gigabytes => true,
            Self::SatVb
            | Self::Percent
            | Self::PercentagePoints
            | Self::TxPerSec
            | Self::HashesPerSecond
            | Self::Minutes
            | Self::Seconds
            | Self::Difficulty
            | Self::Ratio
            | Self::Mixed => false,
        }
    }

    /// A formatted value with its unit attached, for a single line of text.
    ///
    /// The key-figures rail used to pass the unit as a Fact's `note`, which
    /// renders `class="block"` because that slot carries the date under peak
    /// and low. A percentage chart therefore showed
    ///
    /// ```text
    /// average    68.04
    ///                %
    /// ```
    ///
    /// with the symbol stranded on its own line, reading as a stray
    /// character rather than as the unit of the number above it.
    ///
    /// `count`, `difficulty`, `ratio` and `mixed` are not units a reader
    /// wants appended to a number: "3,791 count" is worse than "3,791". They
    /// are names for "this has no unit", so they qualify nothing. `%` closes
    /// up against the number by convention; everything else takes a space.
    pub fn qualify(self, value: &str) -> String {
        match self {
            Self::Count | Self::Difficulty | Self::Ratio | Self::Mixed => {
                value.to_string()
            }
            Self::Percent => format!("{value}%"),
            _ => format!("{value} {}", self.label()),
        }
    }

    /// True when the chart already uses both y axes, leaving no room for a
    /// second series or the price overlay.
    pub fn occupies_both_axes(self) -> bool {
        matches!(self, Self::Mixed)
    }
}

/// How the chart draws, which decides whether the automatic KPIs read sensibly
/// and whether a second series can be laid over it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Line,
    Bar,
    BarWithLine,
    Scatter,
    LineWithScatter,
    StackedAbsolute,
    StackedPercent,
    Donut,
    /// One number against a band, which is a different thing from a donut
    /// even though both are circular.
    ///
    /// `diversity` declared `Donut` and drew a gauge, and nothing noticed
    /// because its source sat outside the conformance harness. The key figures
    /// then read it as a one-slice donut and reported "largest: Moderate,
    /// 100.0% of total, 1 entries", which is three facts about the rendering
    /// and none about the chain.
    Gauge,
    Histogram,
}

impl Shape {
    /// Donuts, gauges and histograms bucket or collapse their x axis, so
    /// "change over the range" and a shared time domain do not apply.
    pub fn has_time_axis(self) -> bool {
        !matches!(self, Self::Donut | Self::Histogram)
    }

    /// Stacked percentage bands already fill 0 to 100, so a second scale
    /// cannot be read against them. Refusing this here is why the clipping bug
    /// fixed in `fix/chart-zoom-axis` cannot recur through the compare path.
    pub fn accepts_second_series(self) -> bool {
        !matches!(
            self,
            Self::StackedPercent | Self::Donut | Self::Gauge | Self::Histogram
        )
    }
}

/// Whether a chart has a daily-aggregate variant for long ranges.
///
/// `Unavailable` is the 7 charts that render `no_data_chart` for daily ranges
/// today. The single view names the ranges the chart does support rather than
/// drawing an empty frame, which is the honest version of what the card does.
#[derive(Clone, Copy)]
pub enum Daily {
    Fn(fn(&[DailyAggregate]) -> Value),
    Unavailable,
}

/// The four charts fed by the mining resource rather than `dashboard_data`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MiningChart {
    Dominance,
    Diversity,
    EmptyBlocks,
    EmptyByPool,
}

/// Where a chart's option JSON comes from. See the module note on why this is
/// an enum rather than one function pointer.
#[derive(Clone, Copy)]
pub enum Source {
    Dashboard {
        per_block: fn(&[BlockSummary]) -> Value,
        daily: Daily,
    },
    /// Needs `disk_size_gb` and a byte offset from state.
    ChainSize,
    /// Needs the window's retarget blocks, which the daily rows cannot
    /// reconstruct.
    ///
    /// Per block it reads difficulty off the blocks it already has. Daily, the
    /// days supply only the category axis and every value comes from
    /// `fetch_retargets`: a day's mean difficulty is a blend wherever a
    /// retarget lands mid-day, and where one lands just after midnight there
    /// is no blend day to find. See `retarget_steps`.
    DiffAdjustment,
    /// Needs the BTC/sats unit toggle.
    Fees,
    /// Count or percent, per-block or from histogram buckets.
    FullnessDist,
    /// Same four combinations as `FullnessDist`.
    TimeDist,
    Mining(MiningChart),
}

/// Long-form copy for a chart, in the two parts a reader asks for in order.
///
/// Two fields rather than one paragraph with a marker in it, because the
/// split is the point: someone new needs `definition` and nothing else,
/// while someone checking our numbers needs `technical` and already knows the
/// definition. Rendering them as one block would make the first group read
/// past the implementation detail and the second group hunt for it.
///
/// `technical` is where this site earns the word "observatory": it says what
/// the figure is computed from and, where it matters, what it leaves out. A
/// caveat stated here is worth more than a more impressive-looking number.
#[derive(Clone, Copy)]
pub struct About {
    /// What the metric is, for a reader who has not met it. No jargon that
    /// is not defined in the same sentence.
    ///
    /// `Option` only because that is what let the gap be closed a category
    /// at a time. Every chart carries one now, and
    /// `every_chart_defines_itself` holds that, so a new chart cannot be
    /// registered without saying what its metric is.
    pub definition: Option<&'static str>,
    /// How this site computes it, and what that excludes.
    ///
    /// Required: this is the half that says what the figure is computed
    /// from and what it leaves out.
    pub technical: &'static str,
}

/// One chart, as the single-chart view needs to know it.
#[derive(Clone, Copy)]
pub struct ChartMeta {
    /// URL slug: the page's `chart_id` minus the `chart-` prefix. These become
    /// public, indexable, citable URLs, so treat them as fixed once shipped.
    pub slug: &'static str,
    /// Must match the page's `ChartCard` title exactly; a test enforces it.
    pub title: &'static str,
    /// The card's one-liner, which is also the single view's subheading and
    /// the two differ only by whether the range is per block or daily. Held
    /// here so both render the same sentence; a test asserts they match the
    /// page.
    pub desc_per_block: &'static str,
    pub desc_daily: &'static str,
    pub category: Category,
    pub unit: Unit,
    pub shape: Shape,
    pub source: Source,
    /// Long-form copy for the flagship charts. `None` renders the short
    /// description alone rather than an empty section.
    /// What this chart measures, per measurement and per resolution.
    ///
    /// Never empty. `every_chart_declares_what_it_measures` holds that, so a
    /// new chart cannot be registered without saying what its points mean at
    /// each resolution, how the number came to exist, and what its population
    /// excludes.
    pub measurements: &'static [Measurement],
    pub about: Option<About>,
}

impl ChartMeta {
    /// Link back to this chart's card in the context of its page.
    pub fn card_anchor(&self) -> String {
        format!("{}#chart-{}", self.category.page_path(), self.slug)
    }

    /// Whether a log y axis is meaningful here.
    ///
    /// Derived rather than stored, so a new chart gets the right answer from
    /// its shape and unit without another 61 values to maintain. Refused for:
    ///
    /// - **percentages**, which is the blunt half of this rule. Being
    ///   bounded at 100 is not the reason: that is about the top of the
    ///   range, and a log axis is about the bottom. Inscription Block Share
    ///   runs from 0.0001% to 65%, five orders of magnitude, and would read
    ///   far better logarithmic. The reason that actually holds is zeros:
    ///   `witness-share` is exactly zero for 3,150 of its 6,468 days,
    ///   everything before SegWit, and a log axis would drop half the chart.
    ///   Allowing it per chart needs a decision this method cannot make from
    ///   the unit alone; see `tasks/todo.md`
    /// - **stacked** charts, where the bands are read against each other and a
    ///   log axis makes the stack's heights lie
    /// - **donuts and histograms**, which have no value axis to rescale
    ///
    /// Everything left is a single quantity over time, which is exactly where
    /// a log axis earns its place: difficulty, chain size and cumulative
    /// counts all span orders of magnitude and are unreadable linear over ALL.
    ///
    /// Note ECharts drops non-positive points on a log axis rather than
    /// erroring, so the toggle is opt-in per view and never the default.
    pub fn supports_log(&self) -> bool {
        self.unit != Unit::Percent
            && self.unit != Unit::Mixed
            && !matches!(
                self.shape,
                Shape::Bar
                    | Shape::BarWithLine
                    | Shape::StackedAbsolute
                    | Shape::StackedPercent
                    | Shape::Donut
                    | Shape::Histogram
            )
    }

    pub fn has_daily(&self) -> bool {
        !matches!(
            self.source,
            Source::Dashboard {
                daily: Daily::Unavailable,
                ..
            }
        )
    }

    /// Whether this chart can take part in a comparison, on either side.
    ///
    /// Three conditions, and each rules out a different failure:
    ///
    /// - **Built from the dashboard rows.** Those are already loaded for the
    ///   selected range, so a comparison costs no extra request and the two
    ///   series are guaranteed to cover the same span. The mining charts and
    ///   the two distributions each need their own fetch over their own
    ///   window, which would mean overlaying two series that quietly do not
    ///   describe the same period. `DiffAdjustment` is included despite
    ///   fetching too, because its fetch is *derived from* the days already
    ///   loaded rather than from its own window, so the span guarantee holds.
    ///   It is also in `MULTI_METRIC`, so it can only ever be the primary
    ///   here, which is the side that needs no second builder.
    /// - **A shape that accepts a second series.** Stacked percentage bands
    ///   already fill 0 to 100, and a donut or histogram has no shared x
    ///   domain to lay anything along.
    /// - **Not already using both axes.** `Unit::Mixed` charts have spent the
    ///   right axis on themselves.
    ///
    /// Derived rather than stored for the same reason `supports_log` is: a
    /// newly registered chart gets the right answer from what it already
    /// declares.
    /// Whether the key-figure rail may report a change across the range.
    ///
    /// False where every point is already a difference, so subtracting the
    /// first from the last says nothing. See [`POINTS_ARE_CHANGES`].
    pub fn reports_change(&self) -> bool {
        !POINTS_ARE_CHANGES.contains(&self.slug)
            && !X_IS_NOT_A_PROGRESSION.contains(&self.slug)
            && !POINTS_ARE_SELECTED_EVENTS.contains(&self.slug)
    }

    /// Whether each plotted point is an extensive reading for its own x
    /// interval, so that adding the points up yields a real total.
    ///
    /// **This is what makes a "range total" mean anything, and the unit alone
    /// cannot answer it.** `witness-versions` is declared `Unit::Count`, and a
    /// count does add up, but above `MAX_PER_BLOCK_RANGE` the daily builder
    /// plots `avg_p2wpkh_count + avg_p2wsh_count`: a per-block *mean* for each
    /// day, which the axis labels "Avg Outputs". Summing 367 of those gave
    /// 2,267,947 over the last year against a true 323,439,450 witness
    /// outputs, a figure 143 times too small sitting under a heading that
    /// promises the range. Found on 2026-09-24 by three independent reviews
    /// and confirmed against the node.
    ///
    /// Eight of the nine totalling stacked charts declare
    /// `MeanOfPerBlockValues` at daily resolution. `address-types` is the only
    /// one declaring `DailyTotal`, which it reaches by multiplying the stored
    /// mean back up by the block count, and it is the only one whose daily
    /// points may be added.
    ///
    /// Per-block resolution is always safe: one point is one block.
    ///
    /// Note this says nothing about an *average*, which survives either way.
    /// A mean of daily per-block means is still a per-block mean: over the
    /// same window it read 6,179.7 against a true 6,176.8, so the two
    /// questions need two predicates.
    pub fn plots_interval_totals(&self, is_daily: bool) -> bool {
        !is_daily
            || self
                .measurements
                .iter()
                .all(|m| matches!(m.daily, Aggregation::DailyTotal))
    }

    pub fn can_compare(&self) -> bool {
        matches!(
            self.source,
            Source::Dashboard { .. } | Source::DiffAdjustment
        ) && self.shape.accepts_second_series()
            && !self.unit.occupies_both_axes()
    }
}

/// Every chart with a single-chart view, sorted by category then slug.
pub const CHARTS: &[ChartMeta] = &[
    ChartMeta {
        slug: "all-embedded-share",
        title: "Detected Embedded Data, Block Share",
        desc_per_block: "Share of each block's weight in detected protocol data: what OP_RETURN outputs cost plus inscription witness bytes",
        desc_daily: "Share of block weight in detected protocol data, totalled over each day",
        category: Category::Embedded,
        unit: Unit::Percent,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::all_embedded_share_chart,
            daily: Daily::Fn(super::all_embedded_share_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "OP_RETURN",
                series_daily: "",
                quantity: "Share of a block's weight spent on OP_RETURN outputs",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "The weight those outputs cost, over the block's weight. Non-witness bytes at four weight units each, counting the output's value and length fields as well as the script.",
            },
            Measurement {
                series: "Inscriptions",
                series_daily: "",
                quantity: "Share of a block's weight spent on inscription witness items",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Envelope bytes of matching witness items, over the block's weight. Witness bytes cost one weight unit each against the four a non-witness byte costs, and that asymmetry is the measurement rather than a quirk. Both bands are on the same basis, so their sum is the share of the block's weight the two together consumed.",
            },
        ],
        about: Some(About {
            definition: Some("The share of each block's weight spent on OP_RETURN data and Ordinals inscriptions combined. Everything in a Bitcoin block is data, including ordinary payments, so these two are grouped together because the protocol never executes or spends them. An OP_RETURN output is provably unspendable, and an inscription sits inside a Taproot script branch that never runs.\n\nBoth are easy to identify on chain: OP_RETURN outputs start with the OP_RETURN opcode, and inscriptions carry an ord tag in their witness envelope. Three other places can carry arbitrary data and are not counted here: the coinbase scriptSig (tracked separately in [Coinbase Message Length](/observatory/chart/coinbase-msg-length)), bare multisig outputs that hide data in 33-byte public-key slots, and fake P2PKH outputs (which look identical to real 20-byte public-key hashes)."),
            technical: "Adds OP_RETURN weight (non-witness outputs) and inscription weight (witness data) and divides by total block weight. Because OP_RETURN lives in outputs and inscriptions live in witness data, the two categories cannot overlap.\n\nBoth sides measure actual block space cost, not just raw payload. OP_RETURN includes the script plus each output's 8-byte value field and 1-byte length prefix, multiplied by 4 weight units per byte. Inscriptions include the full matching witness item (envelope and content-type included) at 1 weight unit per byte. Weight is used as the denominator because Bitcoin caps blocks at 4,000,000 weight units; measuring by raw byte size would treat non-witness and witness bytes as equal even though non-witness bytes cost four times as much.\n\nOP_RETURN scripts have recently grown a long tail. Bitcoin Core v0.12.0 set the default relay limit to 83 bytes in February 2016 (80 bytes of data, 1 byte for OP_RETURN, and 2 bytes to push it). Between 2016 and 2023, 250,905 blocks had a largest OP_RETURN script of exactly 83 bytes, and only 1 block exceeded it: block [425,742](block:425,742) at 86 bytes, included directly by a miner outside default relay policy. After that default cap was lifted, more than half of recent blocks include a script larger than 83 bytes, while the average OP_RETURN script is still about 20 bytes.",
        }),
    },
    ChartMeta {
        slug: "coinbase-payouts",
        title: "Coinbase Payout Count",
        desc_per_block: "How many outputs each block's coinbase creates. Today a typical block has about 6; a few pools settle with hundreds of miners in the coinbase itself",
        desc_daily: "Daily mean coinbase outputs per block. A mean flattens the spikes, so use a range of 1M or shorter to see them",
        category: Category::Embedded,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::coinbase_payout_count_chart,
            daily: Daily::Fn(super::coinbase_payout_count_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Coinbase outputs",
                series_daily: "Coinbase outputs",
                quantity: "Outputs created by the block's coinbase transaction",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Every output of the coinbase transaction, including any OP_RETURN the miner put there, such as the SegWit commitment or merge-mining tags. Before SegWit a single-address payout had 1 output; since SegWit the floor is usually 2 and modern blocks cluster around 6. The daily arm is a mean per block, not a total, so one 919-output block inside a 144-block day lifts that day's mean by about 6.4.",
            },
        ],
        about: Some(About {
            definition: Some("How many outputs each block's coinbase transaction creates. Before 2016, over nine in ten blocks paid the mining reward to a single output. Since SegWit activated, the practical floor is 2 outputs, and a typical block today has about 6 outputs because pools include the witness commitment and extra side outputs alongside the payout.\n\nA few blocks create hundreds of coinbase outputs. Those come from pools that pay miners directly inside the coinbase transaction instead of sending separate payout transactions later. Eligius did this from 2011 to 2017 and holds the record at 919 outputs in block [133,471](block:133,471); Ocean uses the same approach today."),
            technical: "Counts every output in the coinbase transaction directly from each block.\n\nThis total includes non-payment outputs. Almost every post-SegWit coinbase carries a SegWit witness commitment output, and many pools also add merge-mining OP_RETURN outputs. Over the last 10,000 blocks, the median coinbase has 6 outputs and fewer than 1 in 1,000 have a single output. A block with 400 coinbase outputs is paying roughly 400 recipients, minus those few non-payment outputs.\n\nOn daily ranges, each point is the mean per block for that day, which smooths out individual spikes: a single 919-output block in a 144-block day adds only about 6.4 to that day's average. Switch to 1M or shorter to see individual block spikes.",
        }),
    },
    ChartMeta {
        slug: "coinbase-msg-length",
        title: "Coinbase Message Length",
        desc_per_block: "Count of printable ASCII bytes concatenated from each block's coinbase scriptSig",
        desc_daily: "Printable ASCII bytes in each block's coinbase scriptSig (per-block ranges only)",
        category: Category::Embedded,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::coinbase_message_length_chart,
            // Daily aggregates carry no coinbase text, so the daily builder
            // could only ever return an empty chart. Declaring it as a
            // builder made `has_daily` answer true, which offered this as a
            // comparison over long ranges where it draws nothing and
            // suppressed the "this chart needs a shorter range" notice.
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Printable ASCII bytes in the block's coinbase input",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Every printable ASCII byte (0x20-0x7e) anywhere in the coinbase scriptSig, concatenated, not a contiguous text run. No daily builder: the daily table stores no coinbase text, so a daily point would have nothing behind it.",
            },
        ],
        about: Some(About {
            definition: Some("Every block's coinbase transaction has an input field (the scriptSig) where the miner can include arbitrary bytes, famous for the Times headline Satoshi placed in the genesis block. Today it usually contains a mining pool tag alongside binary extranonce bytes. This chart counts how many printable ASCII bytes appear anywhere in that field; the longest in the chain is 94."),
            technical: "Consensus rules restrict a coinbase scriptSig to between 2 and 100 bytes, which is why the 94-byte record is close to 100.\n\nTo compute this metric, every byte in the printable ASCII range (0x20 to 0x7e) is extracted from across the entire scriptSig and concatenated. That includes pool tags, the BIP-34 block height push, and any random extranonce bytes that happen to fall in the printable range. A 100-byte scriptSig that is mostly binary will show a small count here.",
        }),
    },
    ChartMeta {
        slug: "inscription-envelope",
        title: "Inscription Payload vs Envelope",
        desc_per_block: "Detected inscription witness bytes split into the remainder and the subtracted envelope markers",
        desc_daily: "Daily average inscription remainder against subtracted envelope markers per block",
        category: Category::Embedded,
        unit: Unit::Kilobytes,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::inscription_envelope_chart,
            daily: Daily::Fn(super::inscription_envelope_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Payload",
                series_daily: "",
                quantity: "Witness item bytes less subtracted envelope markers",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Matching witness item bytes less the estimated envelope markers (header, content-type section and terminator). Any Taproot reveal script before the envelope currently remains in this band. Denominated in kB of 1,000 bytes, matching every other byte unit on the site.",
            },
            Measurement {
                series: "Envelope Overhead",
                series_daily: "",
                quantity: "Estimated inscription envelope marker bytes",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The envelope header, content-type section and terminator subtracted from the matching witness item, floored at zero. Does not yet include the Taproot reveal script prefix before the envelope.",
            },
        ],
        about: Some(About {
            definition: Some("An Ordinals inscription wraps its content inside a small script envelope containing header markers, a content type, and a terminator. This chart separates those subtracted envelope markers from the rest of each matching witness item so you can see how much overhead small inscriptions pay."),
            technical: "Each Ordinals inscription uses a witness envelope (OP_FALSE OP_IF ... OP_ENDIF with push opcodes and the 'ord' marker). Here the split is estimated by locating the envelope header, content-type section, and terminator by pattern and subtracting them from the matching witness item.\n\nBecause this subtraction does not yet strip the Taproot reveal script that comes before the envelope (typically 34 bytes for the key push and OP_CHECKSIG), that prefix is currently included in the Payload band. Across the more than 200,000 blocks with a detection, the subtracted envelope markers alone account for 7.6% of all matching witness bytes, while the median block is 18.0% and individual blocks range up to 94%. Small inscriptions like BRC-20 JSON operations have the highest overhead share because the marker size is nearly fixed.",
        }),
    },
    ChartMeta {
        slug: "inscription-fee-share",
        title: "Inscription Fee Share",
        desc_per_block: "Percentage of total transaction fees paid by inscription-bearing transactions",
        desc_daily: "Daily inscription fee revenue as a percentage of total fees",
        category: Category::Embedded,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::inscription_fee_share_chart,
            daily: Daily::Fn(super::inscription_fee_share_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of fees paid by transactions matching the inscription detector",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Detected inscription fees over total fees. Both terms are estimates: the numerator credits the whole fee of any transaction matching the inscription detector, and the denominator is coinbase-derived.",
        }],
        about: Some(About {
            definition: Some("The share of total block fees paid by transactions carrying an Ordinals inscription. Because every transaction bids for the same block space, this shows how much of miner fee revenue came from inscription activity."),
            technical: "Divides the fees of transactions matching the inscription witness detector by total block fees. The entire fee of a matching transaction is counted, even if the transaction also did other work, and total block fees are derived from the coinbase output.\n\nAnnual shares have declined since the initial wave: 22.7% of all fees in 2023, 11.2% in 2024, and 7.5% in 2025. Only inscription creation (the reveal transaction) contains the envelope; later transfers of an inscribed output do not appear here. See [Protocol Fee Competition](/observatory/chart/protocol-fee-competition) to compare this against Runes.",
        }),
    },
    ChartMeta {
        slug: "inscription-share",
        title: "Inscription Block Share",
        desc_per_block: "Total inscription witness data (payload + envelope overhead) as a share of each block's weight",
        desc_daily: "Inscription witness data as a share of block weight, totaled over each day",
        category: Category::Embedded,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::inscription_share_chart,
            daily: Daily::Fn(super::inscription_share_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of a block's weight spent on inscription witness items",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Envelope bytes of matching witness items, over the block's weight, so a weight fraction and not a count of anything. Witness bytes cost one weight unit each against the four a non-witness byte costs, which is why an inscription can fill a block by bytes while leaving room by weight.",
        }],
        about: Some(About {
            definition: Some("The share of each block's weight occupied by Ordinals inscription data. Because inscriptions live in witness data and cost 1 weight unit per byte instead of 4, they can take up most of a block's raw bytes while using less of its weight limit. The two highest blocks on record are [892,279](block:892,279) and [899,687](block:899,687), both at 99.96% inscription by weight."),
            technical: "Counts the full matching witness item (both inscription content and envelope framing: OP_FALSE OP_IF, push opcodes, and 'ord' marker) divided by total block weight. Witness bytes cost 1 weight unit each instead of 4, so an inscription uses a quarter as much block weight as the same number of non-witness bytes.",
        }),
    },
    ChartMeta {
        slug: "inscriptions",
        title: "Ordinals Inscriptions",
        desc_per_block: "Inscription-bearing witness items per block: images, text, and other data stored in witness data",
        desc_daily: "Daily average inscription-bearing witness items per block",
        category: Category::Embedded,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::inscription_chart,
            daily: Daily::Fn(super::inscription_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Witness items matching the Ordinals marker",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::MeanOfPerBlockValues,
            population: "Every witness item in the block is scanned; a matching item counts once, so several envelopes in one item count once. The scan is not restricted to verified Taproot scripts.",
        }],
        about: Some(About {
            definition: Some("Inscriptions attach data such as images or text to individual satoshis using the Ordinals convention, which first appeared on chain in block [767,430](block:767,430) on 2022-12-14. The data is stored in the witness section of a Taproot transaction, where each byte counts as 1 weight unit against the block limit instead of 4."),
            technical: "Scans every witness item of every input for the Ordinals envelope marker (0063036f7264), without restricting the scan to verified Taproot scripts. Each matching witness item counts once, so a batch reveal containing multiple envelopes in a single witness item counts as one. Because inscriptions are an application-layer convention and not a consensus rule, different data-embedding schemes will not appear here.",
        }),
    },
    ChartMeta {
        slug: "op-block-share",
        title: "OP_RETURN Block Share",
        desc_per_block: "OP_RETURN cost as a percentage of each block's weight",
        desc_daily: "OP_RETURN cost as a share of block weight, totaled over each day",
        category: Category::Embedded,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::op_return_block_share_chart,
            daily: Daily::Fn(super::op_return_block_share_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of a block's weight spent on OP_RETURN outputs",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "The weight those outputs cost, over the block's weight, excluding coinbase OP_RETURN outputs. An OP_RETURN output is non-witness, so each of its bytes costs four weight units, and its serialized size is the script plus the eight-byte value field and a one-byte length prefix. Block space is rationed in weight, so this measures what the outputs cost the block rather than how many bytes of data they carry.",
        }],
        about: Some(About {
            definition: Some("The share of each block's weight spent on OP_RETURN outputs. An OP_RETURN output is a provably unspendable output carrying arbitrary script bytes, typically around 20 bytes though individual scripts have approached a megabyte. About 63% of all blocks contain at least one non-coinbase OP_RETURN output; coinbase outputs such as the SegWit commitment are excluded."),
            technical: "Measures what non-coinbase OP_RETURN outputs cost the block in weight units, divided by total block weight. Each output's serialized size is its script plus 9 bytes of output framing (an 8-byte value field and a 1-byte length prefix), multiplied by 4 weight units per byte. Coinbase OP_RETURN outputs (such as the SegWit commitment and merge-mining tags) are excluded.\n\nUsing weight instead of raw byte size changes how OP_RETURN compares to inscriptions, because non-witness bytes cost 4 weight units while witness bytes cost 1. Across blocks 920,000 to 975,000, OP_RETURN is 3.94% of byte size but 6.64% of block weight, while inscriptions are 8.09% of size and 3.41% of weight. Across that span, 20.1% of blocks swap which of the two is larger depending on whether you measure size or weight.\n\nCounting the script bytes alone understates OP_RETURN weight cost by about 46%. Most OP_RETURN scripts are only about 20 bytes (short Runes markers and protocol tags, well below the retired 83-byte relay default), so the fixed 9 bytes of serialization framing is nearly half the output.\n\nFor scripts of 253 bytes or longer, the length prefix widens from 1 byte to 3 bytes, which is not modeled here. Blocks containing at least one such script carry about 5% of all OP_RETURN outputs, capping the maximum possible shortfall at 0.3% (and the actual shortfall is much smaller, since most outputs in those blocks are still short).",
        }),
    },
    ChartMeta {
        slug: "opreturn-bytes",
        title: "OP_RETURN Volume",
        desc_per_block: "OP_RETURN script volume per block in kilobytes, by protocol",
        desc_daily: "Daily average OP_RETURN script volume per block in kilobytes, by protocol",
        category: Category::Embedded,
        unit: Unit::Kilobytes,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::op_return_bytes_chart,
            daily: Daily::Fn(super::op_return_bytes_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Runes",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching the Runes prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The day's total divided by its block count, so a per-block mean, then divided by 1,000 because the plotted unit is kB.",
            },
            Measurement {
                series: "Omni",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching the Omni prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Counterparty",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching the Counterparty prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Other",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching no known prefix",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The residual, so it is whatever the detectors missed rather than a named protocol.",
            },
        ],
        about: Some(About {
            definition: Some("How many kilobytes of OP_RETURN scripts each block carried, broken down by protocol prefix. Runes, Omni, and Counterparty are token protocols that store state in OP_RETURN outputs; any unrecognized or obfuscated output is grouped under Other."),
            technical: "Counts scriptPubKey bytes only (the OP_RETURN opcode, push opcodes, and payload), divided by 1,000 for decimal kilobytes on both per-block and daily ranges. Each output also adds 9 bytes of transaction serialization (value and length prefix) that are not included here; see [OP_RETURN Block Share](/observatory/chart/op-block-share) for full output weight.\n\nCoinbase OP_RETURN outputs are excluded. Protocol matching checks leading payload bytes in cleartext, so protocols that obfuscate their payloads (notably RC4-obfuscated Counterparty outputs from 2014 onward) currently appear under Other.",
        }),
    },
    ChartMeta {
        slug: "opreturn-count",
        title: "OP_RETURN Count",
        desc_per_block: "Number of OP_RETURN outputs per block by protocol",
        desc_daily: "Daily average OP_RETURN outputs per block by protocol",
        category: Category::Embedded,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::op_return_count_chart,
            daily: Daily::Fn(super::op_return_count_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Runes",
                series_daily: "",
                quantity: "OP_RETURN outputs matching the Runes prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The day's total divided by its block count, so a per-block mean rather than a daily total.",
            },
            Measurement {
                series: "Omni",
                series_daily: "",
                quantity: "OP_RETURN outputs matching the Omni prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Counterparty",
                series_daily: "",
                quantity: "OP_RETURN outputs matching the Counterparty prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Other",
                series_daily: "",
                quantity: "OP_RETURN outputs matching no known prefix",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The residual, not a named protocol.",
            },
        ],
        about: Some(About {
            definition: Some("How many OP_RETURN outputs each block carried, broken down by protocol prefix. Because this counts outputs instead of bytes, protocols with many small records stand out here, while protocols with fewer large payloads stand out on [OP_RETURN Volume](/observatory/chart/opreturn-bytes)."),
            technical: "Matches the leading bytes of each non-coinbase OP_RETURN script against known protocol prefixes: Runes (6a5d from block [840,000](block:840,000)), cleartext Omni, and cleartext Counterparty. Coinbase OP_RETURN outputs are excluded.\n\nUnrecognized scripts and protocols that obfuscate their payloads (such as RC4-obfuscated Counterparty outputs) fall into the Other band, which represents everything left over.",
        }),
    },
    ChartMeta {
        slug: "protocol-fee-competition",
        title: "Protocol Fee Competition",
        desc_per_block: "Fees paid by transactions matching each protocol detector, side by side. The detectors can match the same transaction, so these are not parts of one total",
        desc_daily: "Daily fees paid by transactions matching each protocol detector. The detectors can match the same transaction, so these are not parts of one total",
        category: Category::Embedded,
        unit: Unit::Btc,
        shape: Shape::Bar,
        source: Source::Dashboard {
            per_block: super::protocol_fee_competition_chart,
            daily: Daily::Fn(super::protocol_fee_competition_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Inscriptions",
                series_daily: "",
                quantity: "Fees paid by transactions matching the inscription detector",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::DailyTotal,
                population: "A transaction matching both this and the Runes detector has its whole fee credited to both, so these series can double-count and are drawn side by side rather than stacked.",
            },
            Measurement {
                series: "Runes",
                series_daily: "",
                quantity: "Fees paid by transactions matching the Runes detector",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::DailyTotal,
                population: "As Inscriptions, and can double-count the same fee.",
            },
        ],
        about: Some(About {
            definition: Some("Compares total fees paid by inscription-bearing transactions and Runes transactions side by side. The two series are kept unstacked because a single transaction can match both detectors and be counted in both."),
            technical: "Shows the total fees paid by transactions matching each detector as two separate bars. A transaction that carries both an inscription and a Runes payload has its full fee counted in both series, so the two bars should not be added together: more than 130,000 blocks contain transactions of both kinds, and in 1,327 blocks the double-counted overlap makes the sum exceed the block's total fees. Note that the inscription detector only sees inscription creation (reveals), while the Runes detector sees both mints and transfers.",
        }),
    },
    ChartMeta {
        slug: "runes-pct",
        title: "OP_RETURN Protocol Share",
        desc_per_block: "Detected OP_RETURN outputs by protocol. Runes took 98.4% of them over the rest of 2024 after launching, and over 97% so far in 2026",
        desc_daily: "Detected OP_RETURN outputs by protocol, computed from each day's totals. Runes took 98.4% of them over the rest of 2024 after launching, and over 97% so far in 2026",
        category: Category::Embedded,
        unit: Unit::Percent,
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::runes_pct_chart,
            daily: Daily::Fn(super::runes_pct_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of OP_RETURN outputs, by protocol",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Each detector's count over the block's non-coinbase OP_RETURN count. Detectors match a prefix, so the residual band is whatever matched none of them rather than a named protocol.",
        }],
        about: Some(About {
            definition: Some("The percentage share of OP_RETURN outputs belonging to each detected protocol. Runes launched in halving block [840,000](block:840,000) on 2024-04-20 and quickly dominated OP_RETURN usage, accounting for 98.4% of detected OP_RETURN outputs over the rest of 2024 (96.9% across calendar 2024 as a whole)."),
            technical: "Divides each protocol's detected OP_RETURN output count by all non-coinbase OP_RETURN outputs in the block (or in the day on daily ranges). Because matching checks cleartext prefixes, unrecognized protocols and obfuscated payloads (such as RC4-obfuscated Counterparty outputs) fall into the Other band. Coinbase OP_RETURN outputs are excluded.",
        }),
    },
    ChartMeta {
        slug: "unified-count",
        title: "Detected Embedded Data, Count",
        desc_per_block: "Per block, by protocol: OP_RETURN outputs for the first four bands and inscription-bearing witness items for the last two",
        desc_daily: "Daily average per block: OP_RETURN outputs and inscription-bearing witness items, by protocol",
        category: Category::Embedded,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::unified_embedded_count_chart,
            daily: Daily::Fn(super::unified_embedded_count_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Runes",
                series_daily: "",
                quantity: "OP_RETURN outputs matching the Runes prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Daily total over block count, so a per-block mean.",
            },
            Measurement {
                series: "Omni",
                series_daily: "",
                quantity: "OP_RETURN outputs matching the Omni prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Counterparty",
                series_daily: "",
                quantity: "OP_RETURN outputs matching the Counterparty prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Other OP_RETURN",
                series_daily: "",
                quantity: "OP_RETURN outputs matching no known prefix",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The residual of the OP_RETURN detectors.",
            },
            Measurement {
                series: "Inscriptions",
                series_daily: "",
                quantity: "Matching witness items other than BRC-20",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "BRC-20 is subtracted so the bands are disjoint as drawn, even though BRC-20 is a subset of inscriptions in the raw taxonomy. The chart mixes OP_RETURN outputs with witness items, which are different objects.",
            },
            Measurement {
                series: "BRC-20",
                series_daily: "",
                quantity: "Matching witness items carrying a BRC-20 marker",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Plotted separately, hence its removal from the Inscriptions band.",
            },
        ],
        about: Some(About {
            definition: Some("All detected embedded data items in one stacked chart. The first four bands count OP_RETURN outputs in the transaction body, and the last two count inscription-bearing witness items in Taproot inputs.\n\nOver the last 90 days, Runes has accounted for about 95% of all detections. Zoom out to ALL and the chart changes dramatically as different eras and protocols come and go."),
            technical: "Counts one unit per non-coinbase OP_RETURN output and one unit per witness item carrying an Ordinals envelope.\n\nEach band uses a raw script marker test. Runes matches OP_RETURN followed by OP_13 from block [840,000](block:840,000) onward (before Runes existed, those bytes were ordinary data). Omni and Counterparty match their cleartext ASCII tags. Other OP_RETURN holds every remaining OP_RETURN output so the four OP_RETURN bands sum exactly to the block's OP_RETURN count. BRC-20 is a subset of Inscriptions and is subtracted from the Inscriptions band here so the two bands do not double-count.\n\nCoinbase transactions are excluded completely, including both the SegWit witness commitment and miner merge-mining OP_RETURN outputs (for example, block [976,950](block:976,950) carries 4 merge-mining OP_RETURNs in its coinbase). See [Coinbase Message Length](/observatory/chart/coinbase-msg-length) for miner coinbase data.\n\nMarker tests do not mislabel ordinary outputs, but protocols that obfuscate their payloads (such as RC4-obfuscated Counterparty outputs) are missed by cleartext matching and land in Other OP_RETURN. See the Methodology page for the full taxonomy.",
        }),
    },
    ChartMeta {
        slug: "unified-volume",
        title: "Detected Embedded Data, Volume",
        desc_per_block: "Embedded data volume per block in kilobytes: full OP_RETURN scripts and full inscription witness items",
        desc_daily: "Daily average embedded data volume per block in kilobytes: full OP_RETURN scripts and full inscription witness items",
        category: Category::Embedded,
        unit: Unit::Kilobytes,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::unified_embedded_volume_chart,
            daily: Daily::Fn(super::unified_embedded_volume_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Runes",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching the Runes prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Daily total over block count then over 1,000, so a per-block mean in kB.",
            },
            Measurement {
                series: "Omni",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching the Omni prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Counterparty",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching the Counterparty prefix",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Runes.",
            },
            Measurement {
                series: "Other OP_RETURN",
                series_daily: "",
                quantity: "OP_RETURN script bytes matching no known prefix",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The residual.",
            },
            Measurement {
                // Named for what it holds. The count chart's "Inscriptions"
                // band subtracts BRC-20 and draws it separately, so the two
                // charts had one label for two different scopes. `brc20_bytes`
                // exists and is populated, but it is payload-basis while this
                // band plots whole witness items (`inscription_envelope_bytes`),
                // so a cost-basis BRC-20 split needs a `brc20_envelope_bytes`
                // column and a backfill sweep (see `rpc.rs:723-731`).
                series: "Inscriptions incl. BRC-20",
                series_daily: "",
                quantity: "Estimated inscription witness bytes, BRC-20 included",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Matching witness item bytes, whereas the OP_RETURN bands are measured script bytes, so the stack mixes two bases: about 6.7 GB measured against about 45 GB estimated over the whole chain. BRC-20 is folded into this band rather than drawn beside it, unlike the count chart where it has a band of its own.",
            },
        ],
        about: Some(About {
            definition: Some("All detected embedded data by volume in kilobytes. About 63% of all blocks contain an OP_RETURN output and over 20% contain a detected inscription. OP_RETURN bands count script bytes, while the inscription band counts full matching witness items."),
            technical: "Plots decimal kilobytes (bytes divided by 1,000) on both per-block and daily ranges. The OP_RETURN bands count the exact scriptPubKey bytes. BRC-20 is combined into the Inscriptions band here instead of drawn separately as it is on [Detected Embedded Data, Count](/observatory/chart/unified-count).\n\nInscriptions are found by scanning witness items for the Ordinals envelope marker and counting the full matching witness item (payload, envelope, and content-type together). Across the whole chain, that comes to about 6.7 GB of OP_RETURN scripts and about 45 GB of inscription witness data.\n\nRunes is detected by its OP_RETURN OP_13 runestone opcode from block [840,000](block:840,000) onward, accounting for 96.9% of OP_RETURN outputs since launch. Stamps is not currently detected and is excluded. Coinbase OP_RETURN outputs are excluded throughout.",
        }),
    },
    ChartMeta {
        slug: "avg-fee-tx",
        title: "Avg Fee per Transaction",
        desc_per_block: "Average fee paid per transaction in satoshis (excludes coinbase)",
        desc_daily: "Daily average fee per transaction in satoshis",
        category: Category::Fees,
        unit: Unit::Sats,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::avg_fee_per_tx_chart,
            daily: Daily::Fn(super::avg_fee_per_tx_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Fees per non-coinbase transaction",
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Block fees over the block's transactions less one for the coinbase. Per block, coinbase-only blocks leave a gap rather than zero; daily, days with zero total fees also leave a gap even when user transactions were present. The fee total is itself coinbase-derived, so this inherits that estimate.",
        }],
        about: Some(About {
            definition: Some("The average fee paid per transaction in each block. Bitcoin fees are based on transaction size in block space, not the amount of bitcoin moved, so sending 10 BTC can cost less than sending 1,000 sats if the transaction takes up fewer bytes."),
            technical: "Divides the block's total fees by its non-coinbase transaction count (excluding the coinbase, which pays no fee). Total fees are calculated from the coinbase output minus the scheduled subsidy, so if a miner underclaims the reward, this average will be lower than what users actually paid.\n\nOn per-block ranges, coinbase-only blocks leave a gap instead of plotting zero. On daily ranges, days with zero total fees also leave a gap even if the day included free user transactions.",
        }),
    },
    ChartMeta {
        slug: "btc-volume",
        title: "BTC Transferred Volume",
        desc_per_block: "Total non-coinbase output value per block in BTC",
        desc_daily: "Daily total non-coinbase output value in BTC",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::BarWithLine,
        source: Source::Dashboard {
            per_block: super::btc_volume_chart,
            daily: Daily::Fn(super::btc_volume_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Value carried by a block's outputs",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::DailyTotal,
            population: "Sum of non-coinbase output values. Counts change returning to the sender, so it is throughput rather than value transferred between parties.",
        }],
        about: Some(About {
            definition: Some("How much bitcoin moved in each block. Every transaction spends whole outputs and creates new ones, so change comes back to the sender inside the same total: a wallet sending 0.1 from a 10 BTC output shows as 10 moved, not 0.1. This counts value in motion, not value changing hands."),
            technical: "Sum of every non-coinbase output value in the block, converted from satoshis to BTC; coinbase reward outputs are excluded. Because change outputs returning to the sender are indistinguishable from payments at the protocol level, both are included in the sum, which therefore measures raw on-chain throughput rather than economic payment volume.",
        }),
    },
    ChartMeta {
        slug: "fee-heatmap",
        title: "Fee Rate Bands",
        desc_per_block: "Fee rate percentiles from p10 to p90, the central 80% of each block's transactions. Click legend items to toggle lines",
        desc_daily: "Fee rate percentiles from p10 to p90, the central 80% of each block's transactions. Click legend items to toggle lines",
        category: Category::Fees,
        unit: Unit::SatVb,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::fee_rate_heatmap_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "p10",
                series_daily: "",
                quantity: "10th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Stored per-block percentile, drawn as its own line. Percentiles are not additive, so these were stacked until 2026-09-15 and the top of the plot was their sum rather than p90.",
            },
            Measurement {
                series: "p25",
                series_daily: "",
                quantity: "25th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10, drawn as its own line.",
            },
            Measurement {
                series: "Median",
                series_daily: "",
                quantity: "Median transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10, drawn as its own line.",
            },
            Measurement {
                series: "p75",
                series_daily: "",
                quantity: "75th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10, drawn as its own line.",
            },
            Measurement {
                series: "p90",
                series_daily: "",
                quantity: "90th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10. p10 to p90 spans the central 80 percent of a block's transactions.",
            },
        ],
        about: Some(About {
            definition: Some("Shows what different transactions in the same block paid in fee rates. Each line is a percentile: p10 is the rate 10% of transactions paid less than, the median is the middle transaction, and p90 is the rate only 10% paid more than. A wide gap between p10 and p90 shows a wide spread of fee rates inside the block."),
            technical: "Sorts every non-coinbase transaction in the block by fee rate (sat/vB) with each transaction weighted equally, then takes five points: p10, p25, p50 (median), p75, and p90. Because every transaction has equal weight here, these percentiles can differ slightly from Bitcoin Core's getblockstats, which weights percentiles by transaction size.\n\nA block needs at least 10 user transactions for p10 and p90, and at least 4 for p25 and p75. Below those thresholds the chart leaves a gap instead of dropping to zero. The span from p10 to p90 covers the middle 80% of transactions; click legend items to isolate individual lines.",
        }),
    },
    ChartMeta {
        slug: "fee-pressure",
        title: "Fee Pressure vs Block Space",
        desc_per_block: "Each block placed by how full it was against its median fee rate",
        desc_daily: "Each block placed by how full it was against its median fee rate",
        category: Category::Fees,
        unit: Unit::SatVb,
        shape: Shape::Scatter,
        source: Source::Dashboard {
            per_block: super::fee_pressure_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Fee Pressure",
                series_daily: "",
                quantity: "Median fee rate against block weight utilisation",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "One point per block. The x axis is a percentage, not time, so this chart shares no domain with the time-series charts and takes no comparison. No daily builder.",
            },
        ],
        about: Some(About {
            definition: Some("Each dot is one block, plotted by how full it was (horizontal axis) against its median fee rate (vertical axis).\n\nBlock fullness reflects miner choices as well as demand. Almost every modern block is filled close to the weight limit, which is why most dots line up along the right edge. Partially empty blocks happen for two main reasons: either a block was found seconds after the previous one before the pool finished building a full template, or the pool filters certain transactions by policy and leaves space unused.\n\nThe fee rate floor has also shifted. Between 2019 and 2024, only 3 blocks with user transactions had a median under 1 sat/vB, and all 3 were tiny blocks of zero-fee transactions. Starting in mid-2025, sub-1 sat/vB medians became common."),
            technical: "Each dot represents one block: the X-axis is block weight utilization (%) and the Y-axis is the block's median fee rate (sat/vB). Top-right dots are full, high-fee blocks; bottom-right dots are full, low-fee blocks; top-left dots are partially filled blocks with high median fee rates.",
        }),
    },
    ChartMeta {
        slug: "fee-revenue-share",
        title: "Fee Revenue Share",
        desc_per_block: "Percentage of total block reward that comes from fees rather than subsidy",
        desc_daily: "Each day's fee revenue as a percentage of that day's total block reward",
        category: Category::Fees,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::fee_revenue_share_chart,
            daily: Daily::Fn(super::fee_revenue_share_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Fees as a share of what the miner earned",
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Fees over subsidy plus fees. Both resolutions sum block subsidies from the exact height schedule (including halving days), and fees are coinbase-derived, so the share inherits that estimate. A falling share can mean falling fees rather than a growing subsidy.",
        }],
        about: Some(About {
            definition: Some("The share of total miner block revenue that comes from transaction fees instead of the newly minted block subsidy. Because the subsidy halves every 210,000 blocks while fees fluctuate with demand, this ratio tracks how much fees contribute to Bitcoin's security budget.\n\nFee share has been volatile: 6.5% across both 2023 and 2024, 1.03% in 2025, and under 1% so far in 2026. More than half of all blocks in Bitcoin's history are under 1%. The record is 93.2%, in a block whose fees were thirteen times its subsidy."),
            technical: "Divides total fees by (subsidy + fees), block by block on short ranges and as a ratio of each day's totals on daily ranges. Both resolutions calculate the subsidy from the exact block-height schedule, so halving days accurately blend blocks from both eras.",
        }),
    },
    ChartMeta {
        slug: "fee-spikes",
        title: "Fee Spike Detector",
        desc_per_block: "Blocks whose median fee rate ran five times the trailing 144-block average, or 200 sat/vB above it",
        desc_daily: "Blocks whose median fee rate ran five times the trailing 144-block average, or 200 sat/vB above it",
        category: Category::Fees,
        unit: Unit::SatVb,
        shape: Shape::LineWithScatter,
        source: Source::Dashboard {
            per_block: super::fee_spike_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Spike (5x or +200)",
                series_daily: "",
                quantity: "Blocks whose median fee rate cleared five times the trailing 144-block mean, or stood 200 sat/vB above it",
                method_per_block: Method::Calculated,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Needs at least 300 blocks of context to establish the trailing mean and is computed per block only (below the 5,000-block daily switch), so it draws on 1W and 1M only. The accompanying moving average is a companion, not a second measurement. No daily builder.",
            },
        ],
        about: Some(About {
            definition: Some("Highlights blocks whose median fee rate jumped far above the surrounding blocks. Demand for block space often arrives in sudden bursts, pushing fee rates up for a few blocks before dropping back. Each dot marks a detected spike."),
            technical: "The line shows the trailing 144-block moving average of block median fee rates (roughly one day of blocks, skipping coinbase-only blocks).\n\nA block is flagged as a spike if its median fee rate is either more than 5x the trailing average or at least 200 sat/vB above it. Using both rules handles both quiet and busy fee markets: the 5x rule catches sudden jumps when baseline fees are low (for example, 1 sat/vB jumping to 6 sat/vB), while the +200 sat/vB rule catches huge jumps when fees are already high.\n\nThis chart requires at least 300 blocks and runs only at per-block resolution, so it is available on 1W and 1M ranges.",
        }),
    },
    ChartMeta {
        slug: "fees",
        title: "Total Fees per Block",
        desc_per_block: "Total transaction fees earned by miners in each block",
        desc_daily: "Average daily transaction fees earned by miners per block",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::Line,
        source: Source::Fees,
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Fees paid to the miner of a block, in the selected denomination",
            // Not measured, and this is the finding behind the declaration.
            // The block total is extracted as coinbase output value minus the
            // scheduled subsidy, so a miner who underclaims the reward makes
            // the figure wrong. Summing each transaction's inputs minus
            // outputs would be measured; that is not what ingestion does.
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::MeanOfPerBlockValues,
            population: "Blocks with positive total fees in the window; blocks (and days) with zero recorded fees leave a gap rather than plotting zero. The denomination follows the BTC/sats toggle, so the active unit is not fixed by this entry.",
        }],
        about: Some(About {
            definition: Some("The total transaction fees collected by the miner in each block. Alongside the block subsidy (which halves every four years), transaction fees make up the miner's total block reward."),
            technical: "Because Bitcoin transactions do not have an explicit fee field (a fee is the difference between inputs and outputs), this chart calculates total block fees from the coinbase transaction: total coinbase output value minus the scheduled block subsidy, floored at zero. That matches the true fee sum whenever a miner claims the full reward; if a miner underclaims, the unclaimed amount is permanently lost and this chart understates the block's fees.\n\nBlocks with zero recorded fees (coinbase-only blocks, zero-fee early blocks, and underclaimed blocks) leave a gap instead of plotting zero. Toggle between BTC and satoshis below the chart.",
        }),
    },
    ChartMeta {
        slug: "halving-era",
        title: "Halving Era Comparison",
        desc_per_block: "Four block metrics compared across the halving eras present in the selected range, each normalized to its own highest era",
        desc_daily: "Four block metrics compared across the halving eras present in the selected range, each normalized to its own highest era",
        category: Category::Fees,
        unit: Unit::Percent,
        shape: Shape::Bar,
        source: Source::Dashboard {
            per_block: super::halving_era_chart,
            daily: Daily::Fn(super::halving_era_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Comparison of metrics across halving eras",
            // The plotted value is a normalised index, so calculated
            // whatever the inputs were, and two of the four inputs are
            // coinbase-derived estimates.
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Blocks grouped by subsidy era, drawn with **the four metrics along the x axis and one series per era**, then **each metric divided by its own largest era**, so the leading bar in a group reads 100 by definition and every plotted number is a share of a maximum rather than a quantity. The four metrics do not share a provenance: block size and transaction count are measured, while total fees and the fee share of miner revenue are both coinbase-derived estimates. Only eras present in the window appear.",
        }],
        about: Some(About {
            definition: Some("Compares average block size, transaction count, fees, and fee revenue share across Bitcoin's halving eras. Every 210,000 blocks (roughly four years), the block subsidy cuts in half. Each metric is scaled to its own peak era so all four fit on one chart."),
            technical: "Each group on the horizontal axis is one metric, with one bar per halving era present in the selected range. Every metric is divided by its highest era so the tallest bar in each group equals 100 and the other eras show their percentage relative to that peak. Hover over any bar to see the actual underlying values (MB, transactions, BTC, or %).\n\nAverage fees and fee revenue share are calculated from coinbase outputs and stored block subsidies.",
        }),
    },
    ChartMeta {
        slug: "max-tx-fee",
        title: "Max Transaction Fee",
        desc_per_block: "The highest single transaction fee in each block, in BTC",
        desc_daily: "The highest single transaction fee in each block, in BTC",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::BarWithLine,
        source: Source::Dashboard {
            per_block: super::max_tx_fee_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Max Tx Fee",
                series_daily: "",
                quantity: "Largest single transaction fee in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "One transaction per block, so this is an extreme rather than a total or a rate. No daily builder.",
            },
        ],
        about: Some(About {
            definition: Some("The largest fee paid by any single transaction in each block. Usually tens of thousands of satoshis, with occasional huge outliers from fat-finger mistakes: the record is 291.24 BTC in block [409,008](block:409,008)."),
            technical: "Finds the highest individual transaction fee in each block, in BTC. Unlike [Total Fees per Block](/observatory/chart/fees) (which is derived from the coinbase), each individual fee here is computed directly from inputs minus outputs.\n\nBecause a single accidental overpayment stands out sharply here while leaving [Median Fee Rate](/observatory/chart/median-rate) unchanged, the two charts complement each other. When more than 200 blocks are in view, a 144-block moving average smooths those block maxima (not average transaction fees). Coinbase-only blocks have no user transactions and leave a gap.",
        }),
    },
    ChartMeta {
        slug: "median-rate",
        title: "Median Fee Rate",
        desc_per_block: "Median fee rate across all transactions in each block",
        desc_daily: "Each block's median fee rate, averaged over the day",
        category: Category::Fees,
        unit: Unit::SatVb,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::median_fee_rate_chart,
            daily: Daily::Fn(super::median_fee_rate_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Mean of per-block median transaction fee rates",
                method_per_block: Method::Measured,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Per block, coinbase-only blocks leave a gap because they have no user transactions to take a median of. Daily, the pre-aggregated mean currently averages across all blocks in the day (including coinbase-only zeros), which is negligible on recent days and drags early-chain days lower.",
            },
        ],
        about: Some(About {
            definition: Some("The median fee rate (in satoshis per virtual byte) paid by transactions in each block. Fees depend on how much block space a transaction uses, not how much bitcoin it transfers."),
            technical: "Computes fee divided by virtual size (weight divided by 4) for every non-coinbase transaction in the block and takes the median, weighting each transaction equally. Because every transaction counts once regardless of size, this can differ slightly from Bitcoin Core's getblockstats, which weights percentiles by transaction size.\n\nOn per-block ranges, each point is the block's median (with a 144-block moving average when more than 200 blocks are shown) and coinbase-only blocks leave a gap. On daily ranges, each point averages the block medians across all blocks that day (currently including coinbase-only blocks stored as zero, which barely affects recent days but lowers early-chain averages).",
        }),
    },
    ChartMeta {
        slug: "protocol-fees",
        title: "Inscription and Runes Fees",
        desc_per_block: "Fees paid by transactions carrying inscription data and by transactions carrying Runes data. A transaction carrying both counts in both lines",
        desc_daily: "Each day's fees from transactions carrying inscription data and from transactions carrying Runes data, averaged per block. A transaction carrying both counts in both lines",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::protocol_fee_breakdown_chart,
            daily: Daily::Fn(super::protocol_fee_breakdown_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Inscriptions",
                series_daily: "",
                quantity: "Fees paid by transactions matching the inscription detector",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Detector match, not a parse. A transaction matching both the inscription and Runes detectors has its whole fee credited to both, so these categories can overlap and are not a partition of the block's fees. Daily, each point is the day's total over the day's block count, so both arms read in BTC per block.",
            },
            Measurement {
                series: "Runes",
                series_daily: "",
                quantity: "Fees paid by transactions matching the Runes detector",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Inscriptions, and can double-count the same transaction's fee. Same daily divisor.",
            },
        ],
        about: Some(About {
            definition: Some("Compares block fees paid by inscription-bearing transactions and Runes transactions as two separate lines.\n\nBecause the two detectors inspect different parts of a transaction, one transaction can match both and have its full fee counted in both lines. As a result, the two lines cannot be added together: in 1,327 blocks the overlap is large enough that their sum exceeds the block's total fees, and many more blocks have smaller double-counting overlap."),
            technical: "Sums the fees of transactions matching each detector per block (or averaged per block on daily ranges). The inscription detector scans input witness data for the Ordinals envelope marker, and the Runes detector scans outputs for the OP_RETURN OP_13 runestone marker from block [840,000](block:840,000) onward.\n\nKeep one key difference in mind when comparing the two lines: an inscription envelope only appears when an inscription is created (minted), whereas a runestone appears on both mints and transfers. So the Runes line captures ongoing trading activity while the Inscriptions line only captures creation.",
        }),
    },
    ChartMeta {
        slug: "subsidy-fees",
        title: "Subsidy vs Fees",
        desc_per_block: "The two parts of what a miner collects per block: the subsidy, which halves on a fixed schedule, and fees, which do not",
        desc_daily: "The two parts of what a miner collects, averaged per block over each day: the subsidy, which halves on a fixed schedule, and fees, which do not",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::subsidy_vs_fees_chart,
            daily: Daily::Fn(super::subsidy_vs_fees_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Subsidy",
                series_daily: "",
                quantity: "Block subsidy",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Per block the subsidy follows from the height's halving era and is exact. Daily it reads the stored per-height sum (daily_blocks.total_subsidy) divided by the day's block count, so halving days blend the two eras accurately.",
            },
            Measurement {
                series: "Fees",
                series_daily: "Avg Fees",
                quantity: "Fees paid to the miner",
                method_per_block: Method::Estimated,
                method_daily: Method::Estimated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Coinbase value less the scheduled subsidy, so a miner underclaiming the reward makes it wrong. Halving cuts the subsidy, not total reward: fees are the other component and are not guaranteed to replace it.",
            },
        ],
        about: Some(About {
            definition: Some("The two components of miner block revenue: the block subsidy (new bitcoin minted by the protocol, currently 3.125 BTC and halving every 210,000 blocks) and transaction fees paid by users."),
            technical: "On per-block ranges, the subsidy is computed directly from block height and fees are calculated as coinbase output value minus the scheduled subsidy (floored at zero, so underclaimed blocks understate fees).\n\nOn daily ranges, both series are averaged per block from the day's stored totals. The subsidy sums each block's exact height-based subsidy, so halving days show the true blended average of blocks from both eras.",
        }),
    },
    ChartMeta {
        slug: "hash-rate",
        title: "Hash Rate",
        desc_per_block: "Estimated hashes per second the network is computing, derived from difficulty",
        desc_daily: "Estimated daily hash rate, derived from difficulty",
        category: Category::Mining,
        unit: Unit::HashesPerSecond,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::hash_rate_chart,
            daily: Daily::Fn(super::hash_rate_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Hash rate implied by the current difficulty",
            // Inferred, not measured: difficulty x 2^32 / 600 assumes blocks
            // arrive at the 600-second target on average. Nothing counts
            // hashes.
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            // Linear in difficulty, so the mean of the day's implied rates
            // equals the rate implied by the day's mean difficulty.
            daily: Aggregation::MeanOfPerBlockValues,
            population: "Every block in the window. Assumes a 600-second mean interval, so short-run values move with luck as well as with hardware.",
        }],
        about: Some(About {
            definition: Some("Estimated total SHA-256 hashes per second across all Bitcoin miners. Because nobody can directly observe every mining machine on Earth, hash rate is estimated from Bitcoin's current mining difficulty."),
            technical: "Calculated as difficulty times 2^32 divided by the 600-second block target. Because difficulty only adjusts once every 2,016 blocks, this line stays flat during an epoch while real hash rate fluctuates (causing blocks to arrive slightly faster or slower than 10 minutes until the next retarget).",
        }),
    },
    ChartMeta {
        slug: "diff-adjustment",
        title: "Difficulty Adjustment",
        desc_per_block: "How much difficulty moved at each retarget, every 2,016 blocks",
        desc_daily: "How much difficulty moved at each retarget, every 2,016 blocks",
        category: Category::Mining,
        unit: Unit::Percent,
        shape: Shape::Bar,
        source: Source::DiffAdjustment,
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Difficulty change at a retarget, as a percentage of the previous epoch",
            // Both resolutions read the retarget blocks themselves: their
            // stored difficulty is the new epoch's difficulty exactly and
            // their timestamp is when the epoch changed.
            //
            // **Calculated, not measured.** The plotted value is
            // `(new / previous - 1) * 100`, which `Method::Calculated`
            // describes precisely: worked out from measured values by a
            // fixed formula. `Measured` means read directly off a block,
            // and no block carries this percentage. Raised from Estimated
            // to Measured on 2026-09-16 and corrected to Calculated the same
            // day, after the review pointed out that the badge has a
            // definition and the definition decides this.
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::WindowedDerived,
            daily: Aggregation::WindowedDerived,
            population: "One point per retarget, every 2,016 blocks, not per block. Drawn as two series split by sign so the bars can be coloured; together they are one series of retargets. Both the date and the percentage come from the two retarget blocks, so the bar is the same wherever the range happens to end. A retarget that changed nothing is not drawn: the first sixteen epochs all sat at difficulty 1.0.",
        }],
        about: Some(About {
            definition: Some("Every 2,016 blocks (roughly every 2 weeks), Bitcoin automatically adjusts mining difficulty so the next 2,016 blocks average about 10 minutes apart. If miners leave and blocks slow down, difficulty drops; if more hash rate joins and blocks speed up, difficulty rises.\n\nThe largest drop on record is 27.94% at block [689,472](block:689,472) on 2021-07-03, about two weeks after the Sichuan shutdown following China's May 2021 mining ban. The largest increase happened in July 2010, when 2,016 blocks arrived in three days and eight hours (averaging two minutes and twenty-three seconds per block), still the fastest epoch in Bitcoin's history."),
            technical: "Calculated directly from block header difficulty values whenever difficulty changes from the previous block: ((new / previous) - 1) * 100. Epochs where difficulty stayed unchanged draw no bar. On daily ranges the preceding epoch is loaded alongside the window; on per-block ranges, a window starting right on a retarget block needs an earlier start block to display that adjustment.\n\nBitcoin caps each adjustment at a 4x move in either direction (+300% maximum, -75% minimum). That +300% ceiling has been hit once across more than 480 retargets, at block [68,544](block:68,544) in July 2010.",
        }),
    },
    ChartMeta {
        slug: "diff-ribbon",
        title: "Difficulty Ribbon",
        desc_per_block: "Seven moving averages of difficulty, from 9 to 128 blocks. Every series here is a smoothed average",
        desc_daily: "Seven moving averages of difficulty, from 7 to 128 days. Every series here is a smoothed average",
        category: Category::Mining,
        unit: Unit::Difficulty,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::difficulty_ribbon_chart,
            daily: Daily::Fn(super::difficulty_ribbon_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Mining difficulty, smoothed over several windows",
                method_per_block: Method::Measured,
                method_daily: Method::Estimated,
                per_block: Aggregation::WindowedDerived,
                daily: Aggregation::WindowedDerived,
                population: "Seven moving averages of one quantity: 9, 14, 25, 40, 60, 90 and 128 blocks per block, and 7, 14, 25, 40, 60, 90 and 128 days daily. There is no unsmoothed series, so every line is derived. Inherits the retarget blending of the daily difficulty column.",
            },
        ],
        about: Some(About {
            definition: Some("Plots seven moving averages of mining difficulty at the same time. Shorter averages react quickly to recent retargets while longer ones lag behind, making it easy to spot when difficulty growth is accelerating, slowing down, or reversing."),
            technical: "Per-block ranges smooth over 9, 14, 25, 40, 60, 90, and 128 blocks; daily ranges smooth over 7, 14, 25, 40, 60, 90, and 128 days. Because a block averages ~10 minutes while a daily point is a full day, the two views cover very different time spans.\n\nWhen the ribbon is wide, difficulty is rising steadily. Slowing growth narrows the band without crossing; an inversion (shorter averages dropping below longer ones) shows sustained falling difficulty, whether from hash rate leaving or an unlucky run of slow blocks.",
        }),
    },
    ChartMeta {
        slug: "difficulty",
        title: "Difficulty",
        desc_per_block: "Mining difficulty per block, adjusts every 2,016 blocks (~2 weeks)",
        desc_daily: "Daily mining difficulty, adjusts every 2,016 blocks (~2 weeks)",
        category: Category::Mining,
        unit: Unit::Difficulty,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::difficulty_chart,
            daily: Daily::Fn(super::difficulty_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Mining difficulty as the protocol reports it",
                method_per_block: Method::Measured,
                method_daily: Method::Estimated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Difficulty holds flat for 2,016 blocks and changes only at a retarget, so a daily mean equals the difficulty on every day except a retarget day, where it is a blend of two epochs and is no protocol difficulty. That is why the daily method is an estimate while the per-block one is not.",
            },
        ],
        about: Some(About {
            definition: Some("How hard it is to mine a valid Bitcoin block. Miners search for a block hash below the current target threshold, and difficulty is the easiest allowed target divided by the current target. Every full node independently computes the exact same difficulty every 2,016 blocks."),
            technical: "Read directly from each block header. Difficulty stays constant for 2,016 blocks (~2 weeks) and updates at each retarget, giving the chart its staircase shape. Multiplying difficulty by 2^32 gives the expected number of SHA-256 hashes needed to find one block.",
        }),
    },
    ChartMeta {
        slug: "diversity",
        title: "Mining Diversity Index",
        desc_per_block: "Herfindahl-Hirschman Index (HHI) measuring mining concentration. Conventionally below 1,000 is competitive and above 1,800 is concentrated",
        desc_daily: "Herfindahl-Hirschman Index (HHI) measuring mining concentration. Conventionally below 1,000 is competitive and above 1,800 is concentrated",
        category: Category::Mining,
        unit: Unit::Count,
        shape: Shape::Gauge,
        source: Source::Mining(MiningChart::Diversity),
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Concentration of block production across identified pools",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "A Herfindahl index over pool shares for the window, normalized across identified blocks only, so unattributed blocks do not dilute it. The band thresholds are a presentation convention, not a security threshold.",
        }],
        about: Some(About {
            definition: Some("Measures how concentrated Bitcoin block production is using the Herfindahl-Hirschman Index (HHI) from economics: square each pool's percentage share and sum the results. Two pools at 50% each score 5,000, while 100 pools at 1% each score 100. Higher scores indicate greater concentration."),
            technical: "Squares each identified pool's percentage share of blocks in the selected range and sums them. A single pool mining 100% of blocks scores 10,000; perfectly dispersed mining approaches zero. The gauge arc caps at 5,000 so modern readings remain easy to see (historical windows above 5,000 pin the arc while showing the exact score). The 1,000 and 1,800 color thresholds are standard antitrust conventions, not Bitcoin protocol limits.\n\nBlocks with no matched pool tag are excluded and shares are renormalized across identified blocks. On recent ranges almost all blocks are tagged so this makes little difference, but no block before 2012 carries a tag and half the chain to date is untagged: over ALL, excluding untagged blocks scores about 1,300, treating them as one miner scores about 2,900, and treating each as its own miner scores about 300. This metric is most reliable over recent years where pool tagging is nearly complete.\n\nOCEAN member sub-tags are folded into a single OCEAN pool here, matching [Mining Pool Share](/observatory/chart/miner-dominance).",
        }),
    },
    ChartMeta {
        slug: "empty-blocks",
        title: "Empty Blocks",
        desc_per_block: "Blocks carrying only the coinbase transaction, mostly from the early chain: 78,800 are from 2009 and 2010",
        desc_daily: "Blocks carrying only the coinbase transaction, mostly from the early chain: 78,800 are from 2009 and 2010",
        category: Category::Mining,
        unit: Unit::Count,
        shape: Shape::Histogram,
        source: Source::Mining(MiningChart::EmptyBlocks),
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Blocks containing only a coinbase transaction, by month",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Counted where tx_count is one, grouped by calendar month. An empty block is a valid block; the count does not establish why it was empty.",
        }],
        about: Some(About {
            definition: Some("Blocks containing only the miner's coinbase transaction (no user transactions and zero fees collected). Close to 90,000 empty blocks exist across the whole chain: 78,800 are from before 2011 and 91.6% are from before 2012.\n\nIn 2009, 99% of blocks were empty simply because very few people were transacting. Today empty blocks are rare and usually happen right after another block: since 2022, empty blocks have arrived under a minute after the previous block on average (compared to about ten minutes for normal blocks), with two in three arriving within thirty seconds.\n\nWhen a new block arrives, pools immediately send miners a temporary coinbase-only job while validating the new block and building a full template. How quickly a pool switches to a full template varies widely across operators; see [Empty Blocks by Pool](/observatory/chart/empty-by-pool)."),
            technical: "Counts blocks with a transaction count of 1 (coinbase only), grouped by calendar month. The short arrival intervals behind modern empty blocks are consistent with temporary coinbase-only templates during block validation, though the chain data only records what was in the block.\n\n86,656 empty blocks are unattributed because coinbase pool tagging did not exist before 2012, when most empty blocks were mined.",
        }),
    },
    ChartMeta {
        slug: "empty-by-pool",
        title: "Empty Blocks by Pool",
        desc_per_block: "Coinbase-only blocks in the range, grouped by the pool that mined them",
        desc_daily: "Coinbase-only blocks in the range, grouped by the pool that mined them",
        category: Category::Mining,
        unit: Unit::Count,
        // Histogram, not Bar, and the difference is the rail rather than the
        // drawing. A Bar routes through the series rail, which reported
        // "change -29.0, -96.7%" here: the smallest pool's count minus the
        // largest's, across buckets sorted by size, so it was guaranteed
        // negative and measured nothing. The sibling Empty Blocks chart was
        // already declared this way.
        shape: Shape::Histogram,
        source: Source::Mining(MiningChart::EmptyByPool),
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Blocks containing only a coinbase transaction, by pool",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "As the Empty Blocks chart, grouped by matched coinbase tag instead of by month, so it inherits the attribution gap: unmatched blocks group as Unknown, and the early chain is largely unmatched.",
        }],
        about: Some(About {
            definition: Some("Shows which mining pools produced coinbase-only (empty) blocks in the selected range. Over the full chain, most empty blocks appear under Unknown because pool tagging did not exist before 2012, when over nine in ten empty blocks were mined."),
            technical: "Groups blocks with a transaction count of 1 by their matched coinbase pool tag (unmatched blocks are grouped as Unknown).\n\nBecause these are raw counts, compare each bar against the pool's total block share: over the past year, SpiderPool leads named pools in empty blocks while mining about half as many total blocks as AntPool.\n\nRoughly 4% to 6% of blocks arrive within thirty seconds of the previous header timestamp (depending on whether backward-stamped pairs are included), yet pools handle those rapid blocks very differently: over the last two years, about a tenth of SpiderPool's rapid blocks came out empty compared to about a fortieth of AntPool's, and Foundry USA has never mined an empty block since its first block in December 2020.",
        }),
    },
    ChartMeta {
        slug: "miner-dominance",
        title: "Mining Pool Share",
        desc_per_block: "Share of blocks in the range by identified pool, with unattributed blocks kept separate",
        desc_daily: "Share of blocks in the range by identified pool, with unattributed blocks kept separate",
        category: Category::Mining,
        unit: Unit::Percent,
        shape: Shape::Donut,
        source: Source::Mining(MiningChart::Dominance),
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of blocks found, by mining pool",
            // Attribution is pattern-matching on coinbase contents, not a
            // signature. Roughly half of all blocks ever mined carry no tag
            // this can match.
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Blocks in the window grouped by matched pool tag, with OCEAN sub-tags folded into OCEAN. Unmatched blocks are grouped as Unknown in their own slice outside the top-10 named pools; attribution is far weaker before 2012, when pools did not tag coinbases.",
        }],
        about: Some(About {
            definition: Some("Shows each mining pool's share of blocks found in the selected range. Miners pool hash rate to smooth out payouts, while the pool operator typically builds the block template and selects which transactions to include."),
            technical: "Pools are identified by matching voluntary tags in the coinbase transaction. Because tagging is optional, a pool that changes or omits its tag shifts into Unknown without anything changing on the network. OCEAN member sub-tags are combined into a single OCEAN slice, matching [Mining Diversity Index](/observatory/chart/diversity).\n\nThe ten largest identified pools each get their own slice, smaller identified pools are grouped into Other, and untagged blocks appear in a separate Unknown slice outside the top ten.",
        }),
    },
    ChartMeta {
        slug: "address-types",
        title: "Address Type Count",
        desc_per_block: "Counts of outputs by script type in each block, one band per type",
        desc_daily: "Daily totals of outputs by script type, one band per type",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::address_type_chart,
            daily: Daily::Fn(super::address_type_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Outputs created, by script type",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            // `avg * block_count`, so a total, which is why the old subtitle
            // "Daily average output types" was wrong.
            daily: Aggregation::DailyTotal,
            population: "Outputs of every transaction, coinbase included. Six script types are plotted; OP_RETURN, bare multisig and unrecognised scripts are not among them, so the bands do not sum to every output.",
        }],
        about: Some(About {
            definition: Some("Counts how many new outputs of each address format are created. Every output locks bitcoin to a script: legacy P2PKH ('1...'), P2SH ('3...', which can wrap multisig or nested SegWit), native SegWit ('bc1q...'), and Taproot ('bc1p...'). The oldest band, P2PK, paid raw public keys directly before address formats existed; it still appears in occasional bursts, though most days have none at all."),
            technical: "Counts outputs created per script type from each block (and daily totals on daily ranges, where 3.0 per block over 10 blocks equals 30). Coinbase outputs are included.\n\nP2SH outputs ('3...') only commit to a script hash at creation time, so nested SegWit, multisig, and timelocks all look identical until spent and are grouped into one band here.\n\nAn output type can appear before its soft fork activates because creating the script shape is already valid. Six P2SH outputs appear in five blocks between 2012-03-07 and 2012-03-10 ahead of BIP-16 activation at block [173,805](block:173,805) on 2012-04-01, and Taproot outputs first appear in 2019 ahead of its 2021 activation. Enable the BIP activations overlay to compare activation dates against adoption.",
        }),
    },
    ChartMeta {
        slug: "address-types-pct",
        title: "Address Type Share",
        desc_per_block: "Each of the six payment output types as a percentage of their combined total",
        desc_daily: "Each of the six payment output types as a percentage of their combined total",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::address_type_pct_chart,
            daily: Daily::Fn(super::address_type_pct_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of outputs created, by script type",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Six classified output types normalized against their own sum, so the bands always total 100 and describe those six rather than every output. Bare multisig, unrecognized scripts and OP_RETURN are outside the denominator, which makes it narrower than the eight-type denominator the P2PKH Sunset chart uses and narrower still than the Output Type Breakdown chart, which divides by every output. The same script type therefore reads differently across those three charts.",
        }],
        about: Some(About {
            definition: Some("The same six payment output types shown as percentage shares totaling 100%. Showing shares removes overall transaction volume swings so you can clearly see formats gaining or losing adoption relative to each other."),
            technical: "Normalizes the six payment script types (P2PK, P2PKH, P2SH, P2WPKH, P2WSH, and P2TR) to 100%. OP_RETURN, bare multisig, and unrecognized scripts are excluded from the denominator.\n\nBecause [P2PKH Sunset Tracker](/observatory/chart/p2pkh-sunset) divides by eight types (adding bare multisig and unrecognized scripts), its denominator is always at least as large, so P2PKH's percentage there is never higher than here (and matches exactly on blocks with no bare multisig or unrecognized outputs).",
        }),
    },
    // ---------------------------------------------------------------------
    // Post-quantum exposure
    //
    // Two charts, not the three the scope doc proposed. Its "Taproot Exposure
    // Rate", P2TR outputs created per block, is the `taproot` chart that
    // already exists; building it again under a new title would have shipped
    // a duplicate. The post-quantum framing is the value there, not a second
    // plot, so this page links to it instead.
    // ---------------------------------------------------------------------
    ChartMeta {
        slug: "pq-exposed-value",
        title: "Value Exposed by Construction",
        desc_per_block: "Share of the bitcoin each block creates that lands in outputs publishing a public key",
        desc_daily: "Share of the bitcoin created each day that lands in outputs publishing a public key",
        category: Category::PostQuantum,
        unit: Unit::Percent,
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::pq_exposed_value_chart,
            daily: Daily::Fn(super::pq_exposed_value_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of satoshis created in outputs that publish a public key",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Every satoshi a block creates, the block reward included. That is wider than the non-coinbase total other charts use, and it has to be: almost every reward before 2012 was paid to a raw public key, so leaving the reward out would push the early years past 100%. Counts value as it is created, not value still held.",
        }],
        about: Some(About {
            definition: Some("The share of newly created bitcoin value that lands in outputs publishing a public key on chain. While [Exposure by Construction](/observatory/chart/pq-exposure) counts outputs, this chart weights them by BTC value: an output holding 0.004 BTC and one holding 1,000 BTC count equally by count, but represent very different amounts of value."),
            technical: "Weighting by value gives a very different picture from counting outputs. Since 2020, 15.2% of classified outputs publish a public key by count, but only 1.8% by value, because the average Taproot output created since 2020 holds about 0.18 BTC compared to about 1.58 BTC for the average classified output.\n\nThis measures value placed into key-exposing outputs when created, not current unspent balances (many older outputs have already been spent). For current unspent balances, see the UTXO snapshot at the bottom of the Post-Quantum page.\n\nExposure from address reuse is not counted here while already-spent outputs remain in the creation flow, so those two factors pull in opposite directions and this chart does not claim a net balance.",
        }),
    },
    ChartMeta {
        slug: "pq-exposure",
        title: "Exposure by Construction",
        desc_per_block: "Share of outputs created whose public key is on chain from the moment they exist",
        desc_daily: "Share of outputs created whose public key is on chain, totaled over each day",
        category: Category::PostQuantum,
        unit: Unit::Percent,
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::pq_exposure_chart,
            daily: Daily::Fn(super::pq_exposure_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of classified outputs created, by whether the output publishes a public key",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "The eight classified script types, summed, which is narrower than every output: provably unspendable outputs such as OP_RETURN carry no public key and never enter the UTXO set, so counting them would dilute the share for a reason unrelated to exposure. This is the same eight-type denominator P2PKH Sunset and Adoption Velocity use, so a script type reads identically across those three. It is narrower than Output Type Breakdown, which divides by every output including unspendable ones, and wider than Address Type Share, which divides by six types. Counts outputs created, never held: an output counted here may have been spent years ago.",
        }],
        about: Some(About {
            definition: Some("Quantum risk to Bitcoin depends on whether an output's public key has been published on chain (where Shor's algorithm could theoretically derive the private key). Three output types publish a public key immediately upon creation: P2PK, bare multisig, and Taproot (P2TR, whose output is a tweaked x-only public key). Hash-based formats (P2PKH, P2SH, P2WPKH, P2WSH) only publish a hash until the coins are spent."),
            technical: "Bands total 100% of classified outputs created in each block or day. Pay-to-anchor (P2A) is separated from unrecognized scripts because it is a fixed two-byte keyless program that is neither key-exposed nor hash-protected.\n\nTaproot publishes a tweaked key by design to keep outputs compact and make cooperative spends indistinguishable from single-signature payments.\n\nKeep two limits in mind: this tracks outputs created over time (a flow), not current unspent balances (which are measured in the UTXO snapshot at the bottom of the Post-Quantum page), and it does not track exposure caused by reusing hash-based addresses.",
        }),
    },
    ChartMeta {
        slug: "pq-p2pk-creation",
        title: "P2PK Outputs Created",
        desc_per_block: "Outputs paying a raw public key, the oldest exposed form, still created today",
        desc_daily: "Outputs paying a raw public key, averaged per block over each day",
        category: Category::PostQuantum,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::pq_p2pk_creation_chart,
            daily: Daily::Fn(super::pq_p2pk_creation_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Outputs created that pay a raw public key",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::MeanOfPerBlockValues,
            population: "P2PK outputs created, coinbase outputs included, which matters here because almost every early mining reward was one. Counts creation rather than holdings: many of these have since been spent, and this chart cannot say which.",
        }],
        about: Some(About {
            definition: Some("Pay-to-Public-Key (P2PK) is Bitcoin's oldest output format, placing the raw public key directly in the output script. Most early mining rewards used P2PK, including Satoshi's coins. It did not end in 2009: more P2PK outputs were created in 2019 than in any other year, and 71% of all P2PK outputs ever made were created between 2017 and 2021."),
            technical: "Plotted on its own axis because P2PK volume is too small to see on [Address Type Count](/observatory/chart/address-types) at modern transaction scales.\n\nP2PK outputs are matched by script shape with a valid public-key prefix byte, which excludes 2012-era outputs that stuffed ASCII text where a public key belongs (classified as nonstandard by Bitcoin Core and counted under unrecognized scripts here). Counts outputs created, not unspent coins still held.",
        }),
    },
    ChartMeta {
        slug: "avg-tx-size",
        title: "Avg Transaction Size",
        desc_per_block: "Average serialized size of a transaction in bytes, including header and coinbase",
        desc_daily: "Daily average serialized transaction size in bytes (block bytes over transactions)",
        category: Category::Network,
        unit: Unit::Bytes,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::avg_tx_size_chart,
            daily: Daily::Fn(super::avg_tx_size_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Mean serialized bytes per transaction",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Whole block size over transaction count, so header and transaction-count overhead are included and the coinbase is counted.",
        }],
        about: Some(About {
            definition: Some("The average serialized byte size per transaction in each block, computed from total block bytes divided by transaction count (including the block header and coinbase)."),
            technical: "Divides total serialized block bytes by the total number of transactions (coinbase included). Moving signatures into the witness section discounts their weight, not their serialized bytes, so SegWit and Taproot transactions are not necessarily smaller in raw bytes. Because blocks are capped at 4,000,000 weight units, a lower byte average does not automatically mean more transactions fit.",
        }),
    },
    ChartMeta {
        slug: "batching",
        title: "Transaction Batching",
        desc_per_block: "Average inputs and outputs per transaction in each block",
        desc_daily: "Daily average inputs and outputs per transaction",
        category: Category::Network,
        unit: Unit::Ratio,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::batching_chart,
            daily: Daily::Fn(super::batching_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Outputs/Tx",
                series_daily: "",
                quantity: "Outputs per non-coinbase transaction",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Non-coinbase outputs over transactions less one for the coinbase. Higher can mean batching or more change outputs; it does not distinguish payments from change.",
            },
            Measurement {
                series: "Inputs/Tx",
                series_daily: "",
                quantity: "Inputs per non-coinbase transaction",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Inputs over transactions less one for the coinbase. Higher suggests consolidation but does not establish it.",
            },
        ],
        about: Some(About {
            definition: Some("Average number of inputs and outputs per non-coinbase transaction. A standard single payment usually has 1-2 inputs and 2 outputs (payment plus change). Higher output counts suggest batched payouts that share fixed transaction overhead across many recipients; higher input counts suggest UTXO consolidation. Recently blocks average around 2.3 outputs and 1.7 inputs per transaction."),
            technical: "Both series divide by the number of non-coinbase transactions and exclude coinbase outputs from the numerator, so coinbase-only blocks leave a gap instead of plotting zero. On-chain data does not identify senders or distinguish payment outputs from change outputs.",
        }),
    },
    ChartMeta {
        slug: "chain-size",
        title: "Chain Size Growth",
        desc_per_block: "Total blockchain size over time, showing how fast the chain is growing",
        desc_daily: "Total blockchain size over time, showing how fast the chain is growing",
        category: Category::Network,
        unit: Unit::Gigabytes,
        shape: Shape::Line,
        source: Source::ChainSize,
        measurements: &[
            Measurement {
                series: "Block Data",
                series_daily: "",
                quantity: "Serialized block bytes, accumulated",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::CumulativeInWindow,
                daily: Aggregation::CumulativeInWindow,
                population: "Every block in the window, plus the stored total for everything before it, so the value is absolute rather than range-relative.",
            },
            Measurement {
                series: "Disk Size (est.)",
                series_daily: "",
                quantity: "Block bytes scaled by the block-and-undo disk ratio a node carries today",
                // The one that makes a single per-chart badge impossible: a
                // measured series beside an estimated companion.
                method_per_block: Method::Estimated,
                method_daily: Method::Estimated,
                per_block: Aggregation::CumulativeInWindow,
                daily: Aggregation::CumulativeInWindow,
                population: "Present-day size_on_disk (block plus undo files) divided by present-day block data, applied to the accumulated series. Excludes chainstate and indexes; not a reconstruction of historical disk usage.",
            },
        ],
        about: Some(About {
            definition: Some("Cumulative size of the Bitcoin blockchain from genesis to today. Full archival nodes keep every block on disk, while pruned nodes verify every block from genesis and then discard older blocks to stay within a few gigabytes."),
            technical: "Block Data sums serialized block bytes from genesis through each point.\n\nDisk Size (est.) scales Block Data by today's ratio of Bitcoin Core's size_on_disk to total block data on my node. In Bitcoin Core, size_on_disk includes block files and undo files only, so neither line includes the chainstate (UTXO set) or optional indexes like txindex; a full node data directory takes more disk space than either line. Only today's ratio is measured, so earlier points on the dashed line are an estimate using today's block-to-undo overhead.",
        }),
    },
    ChartMeta {
        slug: "cumulative-adoption",
        title: "Cumulative Adoption",
        desc_per_block: "Running total of native SegWit v0 and Taproot outputs created within this range. Select ALL for the full stored history",
        desc_daily: "Running total of native SegWit v0 and Taproot outputs created within this range. Select ALL for the full stored history",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::cumulative_adoption_chart,
            daily: Daily::Fn(super::cumulative_adoption_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "SegWit v0 Outputs",
                series_daily: "",
                quantity: "Native v0 witness outputs created, accumulated",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::CumulativeInWindow,
                daily: Aggregation::CumulativeInWindow,
                population: "P2WPKH plus P2WSH outputs, accumulated across the selected window only, so the first point is not the chain's beginning unless the window is. Outputs, not transactions.",
            },
            Measurement {
                series: "Taproot Outputs",
                series_daily: "",
                quantity: "Taproot outputs created, accumulated",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::CumulativeInWindow,
                daily: Aggregation::CumulativeInWindow,
                population: "P2TR outputs, accumulated across the window only.",
            },
        ],
        about: Some(About {
            definition: Some("Running cumulative count of native SegWit v0 and Taproot outputs created within the selected range. Select ALL to see totals from the start of stored history, making it easy to compare total adoption volume between the two formats side by side."),
            technical: "Accumulates output counts starting from the beginning of the selected range (not from genesis, unless ALL is selected). Counts outputs created, not transactions: SegWit v0 sums P2WPKH and P2WSH outputs.",
        }),
    },
    ChartMeta {
        slug: "fullness-dist",
        title: "Block Fullness Distribution",
        desc_per_block: "Distribution of blocks by weight utilization percentage, from near-empty to full",
        desc_daily: "Distribution of blocks by weight utilization percentage, from near-empty to full",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Histogram,
        source: Source::FullnessDist,
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Blocks grouped by how much of the weight limit they used",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Weight divided by the four-million-unit limit, bucketed. Buckets are computed server-side for long ranges and from the blocks themselves for short ones; the count/percentage toggle changes the active unit.",
        }],
        about: Some(About {
            definition: Some("Groups blocks into 10% buckets of the 4,000,000 weight unit limit. Recently almost all blocks fall into the top bucket: around 97% of the last month's blocks are above 99% full."),
            technical: "A histogram of block weight utilization. Most modern blocks cluster near 100% of the weight limit, while partially filled and near-empty blocks still occur (either from coinbase-only templates mined right after a new tip, or from pool transaction filtering; see [Fee Pressure vs Block Space](/observatory/chart/fee-pressure)). On longer ranges including Bitcoin's early years, many more blocks appear in the lower buckets.",
        }),
    },
    ChartMeta {
        slug: "interval",
        title: "Block Interval",
        desc_per_block: "Minutes between consecutive blocks. Target is 10 minutes",
        desc_daily: "Each day's 1,440 minutes divided by the blocks it produced. Target is 10 minutes",
        category: Category::Network,
        unit: Unit::Minutes,
        shape: Shape::LineWithScatter,
        source: Source::Dashboard {
            per_block: super::block_interval_chart,
            daily: Daily::Fn(super::block_interval_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Time between blocks",
                method_per_block: Method::Calculated,
                method_daily: Method::Estimated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::InverseOfDailyCount,
                population: "Per block this is the difference between consecutive header timestamps, which miners choose, so it can be zero or negative. Daily it is 1,440 minutes divided by the day's block count, which is close to the mean of those differences for a whole day and is not the same quantity. Every day in the window is plotted, including the 40 days of 2009 that ran genuinely slow. The final day is a gap, because a named range ends on a day still in progress and that day is not a whole 1,440 minutes; every earlier day is returned complete whatever time the window starts. A day with no blocks has no interval rather than an interval of zero.",
            },
        ],
        about: Some(About {
            definition: Some("Time elapsed between consecutive blocks. Bitcoin targets an average of 10 minutes through difficulty adjustments, but individual block intervals are random: a 2-minute gap and a 50-minute gap are both completely normal."),
            technical: "On per-block ranges, each point is the difference between consecutive block header timestamps.\n\nHeader timestamps are set by miners and do not have to increase monotonically: consensus only requires a block's timestamp to be greater than the median of the previous 11 blocks, and nodes separately reject blocks stamped more than 2 hours ahead of their own clock. Because of this, about 1 consecutive pair in 60 has a negative interval (up to almost two hours backward). These are plotted as recorded in the headers.\n\nOn daily ranges, each point divides the 1,440 minutes in a day by that day's block count (so a 144-block day equals exactly 10 minutes).",
        }),
    },
    ChartMeta {
        slug: "largest-tx",
        title: "Largest Transaction",
        desc_per_block: "Size of the largest transaction in each block, in kilobytes",
        desc_daily: "Largest transaction (per-block ranges only)",
        category: Category::Network,
        unit: Unit::Kilobytes,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::largest_tx_chart,
            // Daily aggregates carry no per-transaction sizes, so the daily
            // builder could only ever return an empty chart. See
            // `coinbase-msg-length` for what declaring one costs.
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Largest Tx",
                series_daily: "",
                quantity: "Size of the largest transaction in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Serialized size of one transaction per block, not an average. No daily builder: the daily table stores no per-block maximum.",
            },
        ],
        about: Some(About {
            definition: Some("Size of the single largest transaction in each block, in kilobytes.\n\nBitcoin caps block weight at 4,000,000 weight units (4 units per regular byte, 1 unit per witness byte). While a typical block is around half witness data and averages near 1.6 MB, a single mostly-witness transaction can approach 4 MB and still fit inside one block.\n\nMore than 100 blocks contain a transaction larger than 3.9 MB, and MARA mined more than nine in ten of them. Because Bitcoin Core's default relay policy caps standard transactions at 400,000 weight units (a tenth of a block), these multi-megabyte transactions were submitted directly to miners instead of propagating over the public mempool. Almost all carry an inscription, and 45 left room for fewer than 10 other transactions. The largest transaction in Bitcoin history is 3,992,821 bytes in block [839,842](block:839,842) on 2024-04-18. They began in February 2023 and still occur periodically."),
            technical: "Serialized byte size of the largest transaction in each block, divided by 1,000 for decimal kilobytes. Because this measures raw byte size instead of weight, witness-heavy transactions appear much larger than their share of the block weight limit.",
        }),
    },
    ChartMeta {
        slug: "multi-velocity",
        title: "Adoption Velocity",
        desc_per_block: "Change in each output type's share of outputs, in percentage points: a 144-block average against the same average 144 blocks earlier",
        desc_daily: "Change in each output type's share of outputs, in percentage points: a 30-day average against the same average 30 days earlier",
        category: Category::Network,
        unit: Unit::PercentagePoints,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::multi_velocity_chart,
            daily: Daily::Fn(super::multi_velocity_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Change in each script type's share of outputs",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::WindowedDerived,
            daily: Aggregation::WindowedDerived,
            population: "Each type's outputs over all outputs of the nine classified types, averaged across the window, minus the same average one window earlier: 144 blocks per block, 30 days daily. The result is in percentage points, so a share moving from 10 to 15 is 5 points and not 5 percent. All nine are drawn, so the lines sum to zero at every point. Divergent lines do not establish that users migrated between types.",
        }],
        about: Some(About {
            definition: Some("Tracks whether each output type is gaining or losing share over time. Values above zero mean that format's share grew compared to the previous window; values below zero mean it shrank.\n\nValues are in percentage points, not percent change: Taproot moving from 22% of outputs to 11% is a drop of 11 points (a 50% relative decline), and this chart plots -11. Because every output type is included, all lines sum to zero at every point.\n\nOpposite moves do not necessarily mean users migrated from one format to another. For example, through April 2026 P2WPKH gained more than 13 points while Taproot fell more than 11 points for independent reasons: inscriptions dropped by about two thirds (roughly halving Taproot output creation), while P2WPKH output counts grew by more than a quarter."),
            technical: "Computes each output type's share across all nine classified types, takes a trailing moving average, and subtracts the same moving average from one window earlier. Per-block ranges use a 144-block window (about a day, though real days held between 98 and 197 blocks over the past year); daily ranges use a 30-day window.\n\nAll nine classified types are shown: P2PKH, P2SH, P2WPKH, P2WSH, P2TR, P2PK, bare multisig, pay-to-anchor (P2A), and unrecognized scripts.\n\nHistorically, P2PK held virtually every output in January 2009 and fell to about 12% by late 2010 as payments moved to P2PKH (a handover of more than 40 points). Across late 2010 and early 2011, P2PK output counts stayed within half a percent of coinbase outputs as miners continued paying block rewards to raw public keys. Later, bare multisig spiked during the 2023 Stamps wave and P2WSH during the Ordinals ramp. P2A did not exist before June 2024 and has swung by nearly 5 points inside a 30-day window since. Unrecognized scripts now average about a hundredth of a percent. Click legend items to hide quiet series.",
        }),
    },
    ChartMeta {
        slug: "p2pkh-sunset",
        title: "P2PKH Sunset Tracker",
        desc_per_block: "Share of outputs paying the legacy '1...' address type, with reference lines at 10% and 5%",
        desc_daily: "Share of outputs paying the legacy '1...' address type, with a 90-day average and reference lines at 10% and 5%",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::address_sunset_chart,
            daily: Daily::Fn(super::address_sunset_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of outputs created to P2PKH",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "P2PKH outputs over eight classified output types: the six payment types plus bare multisig and unrecognized scripts (pay-to-anchor sits inside the unrecognized bucket here, so it is counted once rather than twice). OP_RETURN outputs are outside it. Coinbase outputs are counted, which matters most before 2012 when they were much of the chain. Note this is a wider denominator than the Address Type Share chart uses, so P2PKH never reads higher here than there. The 90-day smoothing exists only at daily resolution. A falling share does not establish that those users moved to another type.",
        }],
        about: Some(About {
            definition: Some("Tracks the long-term decline of Bitcoin's original legacy address format, P2PKH ('1...'). Between P2SH arriving and SegWit activating, P2PKH accounted for nearly nine in ten outputs.\n\nIts share has steadily declined as wallets upgraded. Taproot overtook P2PKH in 2023 and remains ahead, though through 2026 both fell while P2WPKH grew.\n\nThe dashed reference lines at 10% and 5% are visual milestones only, not protocol thresholds."),
            technical: "Divides P2PKH outputs by the eight classified output types (the six payment types plus bare multisig and unrecognized scripts, which include pay-to-anchor; OP_RETURN is excluded). Daily ranges add a 90-day moving average. Coinbase outputs are included.",
        }),
    },
    ChartMeta {
        slug: "propagation",
        title: "Rapid Consecutive Blocks",
        desc_per_block: "Consecutive blocks whose header timestamps are less than 60 seconds apart. Miners set these timestamps, so this does not measure network propagation",
        desc_daily: "Consecutive blocks whose header timestamps are less than 60 seconds apart. Miners set these timestamps, so this does not measure network propagation",
        category: Category::Network,
        unit: Unit::Seconds,
        shape: Shape::Scatter,
        source: Source::Dashboard {
            per_block: super::block_propagation_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Interval",
                series_daily: "",
                quantity: "Gap to the previous block, where it is under 60 seconds",
                method_per_block: Method::Calculated,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Difference between consecutive block header timestamps, which miners choose, so this is not a measure of network propagation or of stale-block races. Backward pairs are excluded rather than clamped to zero. No daily builder.",
            },
        ],
        about: Some(About {
            definition: Some("Highlights blocks whose header timestamp is less than 60 seconds after the previous block. Because mining is a random Poisson process, short intervals happen regularly and are not by themselves a sign of network trouble. Because miners choose header timestamps, this does not measure how fast blocks actually propagated across the network."),
            technical: "Plots consecutive block pairs where the header timestamp difference is between 0 and 60 seconds (per-block ranges only). Backward timestamp pairs (about one in 60 pairs across the chain) are excluded instead of clamped to zero.",
        }),
    },
    ChartMeta {
        slug: "rbf",
        title: "Explicit RBF Signaling",
        desc_per_block: "Share of each block's transactions whose inputs signal replaceability under BIP 125",
        desc_daily: "Daily share of transactions signaling replaceability under BIP 125",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::rbf_chart,
            daily: Daily::Fn(super::rbf_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of confirmed non-coinbase transactions whose input sequence numbers signal replaceability",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Confirmed non-coinbase transactions whose input sequence numbers signal under BIP 125, over all confirmed non-coinbase transactions. Not the share actually replaced, and not wallet adoption: full-RBF policy means absence of the signal does not prevent replacement.",
        }],
        about: Some(About {
            definition: Some("The share of confirmed transactions that explicitly signal opt-in Replace-By-Fee (BIP 125), indicating the sender allows an unconfirmed transaction to be replaced with a higher-fee version. It measures signaling, not how many transactions were actually replaced.\n\nExplicit RBF signaling passed half of all transactions in 2023 and has ranged between roughly a half and four fifths month by month since. Data begins in February 2016 when Bitcoin Core 0.12.0 introduced BIP 125. Since Bitcoin Core v28 enabled full-RBF by default, nodes allow fee replacement even without the flag, so this line reflects wallet defaults more than relay eligibility."),
            technical: "Counts a transaction once if at least one of its inputs has a sequence number below 0xfffffffe, divided by all non-coinbase transactions in the block (or day). A wallet can set the flag by default without ever bumping fees, and under full-RBF a user can replace a transaction that never set the flag.",
        }),
    },
    ChartMeta {
        slug: "segwit",
        title: "SegWit Adoption",
        desc_per_block: "Percentage of transactions using Segregated Witness",
        desc_daily: "Daily share of transactions using Segregated Witness",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::segwit_adoption_chart,
            daily: Daily::Fn(super::segwit_adoption_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of transactions spending a witness input",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::PerBlockObservation,
            // `avg_segwit_spend_count / (avg_tx_count - 1)`. Both terms carry
            // the same block count, so this is the day's pooled ratio rather
            // than the mean of per-block shares: it equals total segwit spends
            // over total non-coinbase transactions.
            daily: Aggregation::RatioOfTotals,
            population: "Numerator: **transactions** containing at least one witness input, counted one per transaction rather than one per input. Denominator: non-coinbase transactions, obtained by subtracting one coinbase per block. A transaction spending ten witness inputs counts once, so this is the share of transactions using witness data and not a share of inputs.",
        }],
        about: Some(About {
            definition: Some("The percentage of non-coinbase transactions using Segregated Witness (SegWit). Activated in 2017, SegWit moved signatures and unlocking scripts into a separate witness section where each byte counts as 1 weight unit instead of 4, lowering transaction fees and expanding effective block capacity."),
            technical: "Counts a transaction as SegWit if at least one of its inputs includes witness data, divided by all non-coinbase transactions in the block (or day on daily ranges). This tracks SegWit spending; to see SegWit and Taproot outputs being created, check [Address Type Share](/observatory/chart/address-types-pct).",
        }),
    },
    ChartMeta {
        slug: "size",
        title: "Block Size",
        desc_per_block: "How large each block is in megabytes",
        desc_daily: "Average block size per day in megabytes",
        category: Category::Network,
        unit: Unit::Megabytes,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::block_size_chart,
            daily: Daily::Fn(super::block_size_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Serialized block size",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Every block in the window, including header and transaction-count overhead. Size is not weight: a block near the weight limit can be well under the size its witness discount allows.",
            },
        ],
        about: Some(About {
            definition: Some("Total serialized size of each block in megabytes (decimal MB). Because Bitcoin's consensus limit is 4,000,000 weight units instead of a fixed byte cap, see [Weight Utilization](/observatory/chart/weight-util) for how full each block is."),
            technical: "Measures the full serialized size of each block as stored by my node, including witness data and block headers. Because witness bytes count as 1 weight unit instead of 4 toward the 4 million weight unit cap, SegWit and Taproot blocks routinely exceed the old 1 MB limit.",
        }),
    },
    ChartMeta {
        slug: "taproot",
        title: "Taproot Outputs",
        desc_per_block: "New Taproot (P2TR) outputs created per block",
        desc_daily: "Average Taproot (P2TR) outputs created per block each day",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::taproot_chart,
            daily: Daily::Fn(super::taproot_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Taproot outputs created",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "P2TR outputs, coinbase outputs included. Reads p2tr_count; the identically-valued taproot_spend_count column is misnamed and no longer read here.",
            },
        ],
        about: Some(About {
            definition: Some("How many new Taproot outputs are being created. Taproot, activated in 2021, is the most recent change to how Bitcoin outputs can be locked. It makes a complex spending condition, such as a multi-signature wallet, look the same on chain as an ordinary payment, which helps both privacy and fees."),
            technical: "Counts outputs with a pay-to-Taproot script created in each block. Created, not spent: an output can sit unspent for years, so this leads the share of transactions that actually use Taproot. Inscriptions are stored in Taproot witness data, which is why this and the inscription charts move together from 2023.",
        }),
    },
    ChartMeta {
        slug: "taproot-spend-types",
        title: "Taproot Spend Types",
        desc_per_block: "Key-path against script-path spends per block. Which spends revealed a script and which revealed nothing",
        desc_daily: "Daily average key-path against script-path spends. Which spends revealed a script and which revealed nothing",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::taproot_spend_type_chart,
            daily: Daily::Fn(super::taproot_spend_type_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Key-path",
                series_daily: "",
                quantity: "Inputs detected as key-path Taproot spends",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Classified from witness shape, so these are detector counts rather than verified spend totals. A key-path spend reveals a signature but not the signing arrangement: it can be a single signer, an aggregated multisignature, or a cooperative contract close, and BIP 341 makes those indistinguishable.",
            },
            Measurement {
                series: "Script-path",
                series_daily: "",
                quantity: "Inputs detected as script-path Taproot spends",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "As Key-path. A revealed script is not necessarily a complex contract.",
            },
        ],
        about: Some(About {
            definition: Some("Compares the two ways to spend a Taproot output: key-path spends (a single Schnorr signature with nothing else revealed) and script-path spends (revealing one branch of the underlying script tree). Over the last 10,000 blocks around nine in ten Taproot spends are key-path, though the mix swings widely with inscription waves: across the past year it has averaged around 69%, and through 2023 script-path spends outnumbered key-path more than two to one."),
            technical: "Key-path spends reveal only a single Schnorr signature. Under BIP 341, single-key wallets, aggregated multisigs (such as MuSig2), and cooperative contract closes all look identical on chain when spent via key-path.\n\nScript-path spends reveal a Tapscript leaf and control block. In practice, Ordinals inscription reveals make up most of the script-path band, though uncooperative contract closes have the same witness shape and are counted alongside them here. See [Ordinals Inscriptions](/observatory/chart/inscriptions) for the inscription portion over the same blocks. Both bands are classified directly from input witness structure.",
        }),
    },
    ChartMeta {
        slug: "time-dist",
        title: "Block Time Distribution",
        desc_per_block: "How long each block waited for the one before it. Ten minutes is the average, not the typical gap",
        desc_daily: "How long each block waited for the one before it. Ten minutes is the average, not the typical gap",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Histogram,
        source: Source::TimeDist,
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Blocks grouped by the gap to their predecessor",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Differences between consecutive block header timestamps, bucketed. Ten minutes is the target mean, not the most common bucket: the distribution is exponential, so the peak sits at the short end. Miners choose timestamps, so about one of the chain's pairs in 60 runs backwards; those are excluded from both arms rather than counted as instant blocks, which pulls the 0-1 minute bucket slightly below the 1-2 minute bucket across the full chain.",
        }],
        about: Some(About {
            definition: Some("Distribution of intervals between consecutive blocks. Because Bitcoin mining is a memoryless random process, short gaps are the most common and long waits form a long tail: the median interval is 6.9 minutes even though the target average is 10 minutes."),
            technical: "Groups consecutive block header timestamp differences into time buckets. Because mining is memoryless, waiting times follow an exponential distribution peaking at the short end. Backward timestamp pairs (where a miner stamped a block earlier than its parent) are excluded instead of counted as instant blocks, which keeps the 0-1 minute bar slightly below the 1-2 minute bar across the full chain.\n\nAcross the full chain: the median interval is 6.9 minutes, 63.9% of blocks arrive in under 10 minutes, 4.5% take more than half an hour, and 0.3% take over an hour (close to the theoretical 10-minute exponential predictions of 63.2%, 5.0%, and 0.25%).",
        }),
    },
    ChartMeta {
        slug: "tps",
        title: "Transactions per Second",
        desc_per_block: "Transactions in each block over the seconds since the previous one",
        desc_daily: "Each day's transactions over the 86,400 seconds in a day",
        category: Category::Network,
        unit: Unit::TxPerSec,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::tps_chart,
            daily: Daily::Fn(super::tps_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Transactions per second",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::PerBlockObservation,
            // The day's transaction count over a constant 86,400
            // seconds, so it moves with the day's blocks exactly as a total
            // does. Declared `WindowedDerived` until 2026-09-21, which
            // `expectation()` maps to no prediction, so the primary series
            // was exempt from the scaling probe that catches this class of
            // error. There is no window here: the divisor is a constant.
            daily: Aggregation::DailyTotal,
            population: "Per block this is the block's transaction count over the gap to its predecessor. A non-positive gap has no rate rather than a rate of zero, and the first block in the window has no predecessor, so both are gaps. Daily it is the day's transaction count over 86,400 seconds, coinbase included, so the final day is a gap: a named range ends on a day still in progress, which is not a whole 86,400 seconds."
        }],
        about: Some(About {
            definition: Some("On-chain transactions confirmed per second. Over the past year, Bitcoin averaged about 7 transactions per second, with individual days ranging between 3 and 11.\n\nThis counts base-layer transactions only: Lightning payments, batched payouts inside a single transaction, and off-chain transfers are not counted separately."),
            technical: "Per-block and daily ranges use different divisors.\n\nOn per-block ranges, each block's transaction count is divided by the seconds since the previous block's header timestamp. Short block intervals produce huge spikes: over the past year the median block is about 7 tx/s and the 95th percentile is over 100, peaking at 7,881 tx/s in block [972,934](block:972,934) (7,881 transactions stamped one second after its parent; see [Block Interval](/observatory/chart/interval) on header timestamps).\n\nOn daily ranges, each day's total transactions are divided by 86,400 seconds, smoothing out block-interval noise.",
        }),
    },
    ChartMeta {
        slug: "tx-density",
        title: "Transaction Density",
        desc_per_block: "Transactions per 1,000 serialized bytes of block space",
        desc_daily: "Daily transaction density (total transactions over total serialized kilobytes)",
        category: Category::Network,
        unit: Unit::Ratio,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::tx_density_chart,
            daily: Daily::Fn(super::tx_density_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Transactions per 1,000 serialized block bytes",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Transaction count over serialized size in kB, where size includes header and count overhead and the count includes the coinbase. It measures how many transactions fit in the bytes used, not payments per transaction and not fee efficiency: batching lowers density while using less space per payment.",
            },
        ],
        about: Some(About {
            definition: Some("Number of transactions per kilobyte of serialized block space. Blocks packed with small single payments have high density, while blocks full of large consolidations, batched payouts, or inscriptions have lower density."),
            technical: "Divides transaction count by serialized block size in kilobytes (per block, or total transactions over total serialized kilobytes on daily ranges). Because the divisor is serialized bytes and not weight, moving signatures into the witness section does not increase density: witness-heavy blocks carry more serialized bytes within the 4,000,000 weight unit cap and have lower density.",
        }),
    },
    ChartMeta {
        slug: "tx-type-evolution",
        title: "Transaction Type Share",
        desc_per_block: "Breakdown of transactions by input type: Legacy (non-witness), SegWit v0, and Taproot",
        desc_daily: "Breakdown of transactions by input type: Legacy (non-witness), SegWit v0, and Taproot",
        category: Category::Network,
        unit: Unit::Percent,
        // **`StackedPercent`, because the builder normalises to 100.**
        // `tx_metrics.rs:741` divides each count by the three-way total, so
        // the bands fill the axis by construction. Declared
        // `StackedAbsolute` until 2026-09-16, which made
        // `accepts_second_series` true, and therefore `can_compare` true, so
        // the picker offered to lay a price overlay or another metric over
        // bands that already occupy 0 to 100. That is the exact case the
        // `StackedPercent` refusal exists for. Being in `MULTI_METRIC` only
        // barred it as the candidate, not as the primary.
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::tx_type_evolution_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Share of transactions by input script type",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Three bands over the block's non-coinbase transactions. No daily builder: the daily table stores the component counts but this builder was never given a daily arm.",
            },
        ],
        about: Some(About {
            definition: Some("Share of non-coinbase transactions by spending input format: Legacy (no witness data), SegWit v0, or Taproot. Close to nine in ten transactions now include witness data."),
            technical: "Classifies each non-coinbase transaction by its input witness structure (per-block ranges only). Unlike output-type charts (which reflect what receivers generate), this tracks what senders are spending.",
        }),
    },
    ChartMeta {
        slug: "txcount",
        title: "Transaction Count",
        desc_per_block: "Number of transactions included in each block",
        desc_daily: "Average number of transactions per block each day",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::tx_count_chart,
            daily: Daily::Fn(super::tx_count_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Transactions confirmed in a block, including the coinbase",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            // The stored `avg_tx_count` column, plotted as it is: the
            // unweighted mean of the day's per-block counts.
            daily: Aggregation::MeanOfPerBlockValues,
            population: "Every block in the window. Counts include each block's coinbase transaction.",
        }],
        about: Some(About {
            definition: Some("Number of transactions confirmed in each block. This depends on both demand and transaction size: a full block of small payments holds thousands of transactions, while a full block of large inscriptions or consolidations holds far fewer."),
            technical: "Counted directly from each block on my node, including the coinbase transaction (adding 1 to every block compared to sources that exclude coinbases). Transactions are counted in the block where they confirm, not when they enter the mempool, and daily ranges show the average transactions per block for that day.",
        }),
    },
    ChartMeta {
        slug: "utxo-flow",
        title: "UTXO Flow",
        desc_per_block: "Inputs spent against outputs created per block. The gap includes unspendable outputs like OP_RETURN",
        desc_daily: "Daily average inputs spent against outputs created per block. The gap includes unspendable outputs like OP_RETURN",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::utxo_flow_chart,
            daily: Daily::Fn(super::utxo_flow_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Outputs (created)",
                series_daily: "",
                quantity: "Outputs created, coinbase included",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Every output in the block, coinbase included, and that is the raw tally rather than the change in the spendable set: provably unspendable outputs are in it. The UTXO Growth Rate chart subtracts them.",
            },
            Measurement {
                series: "Inputs (consumed)",
                series_daily: "",
                quantity: "Inputs spent by non-coinbase transactions",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The coinbase input is not counted.",
            },
        ],
        about: Some(About {
            definition: Some("Compares total outputs created against total inputs consumed in each block. Bitcoin has no account balances; every transaction spends existing outputs (inputs) and creates new ones (outputs).\n\nThe gap between the green and red lines is not the net change in the UTXO set, because provably unspendable outputs like OP_RETURN raise the Outputs line without ever entering the UTXO set. Over the past year, that gap averaged about two thousand outputs per block; across 2025, outputs exceeded inputs by about 450 per block while the spendable UTXO set actually shrank by about 390 per block. See [UTXO Growth Rate](/observatory/chart/utxo-growth) for the exact net UTXO set change."),
            technical: "Counts all outputs created (including coinbase outputs) and all non-coinbase inputs spent. The coinbase input is excluded because it creates new bitcoin instead of spending an existing UTXO.\n\nA coinbase-only block consumes zero inputs and creates only its coinbase outputs, which has happened about ninety thousand times across the chain. Almost all of those created a single output, though pools that pay miners directly in the coinbase (like Eligius from 2011 to 2017, peaking at 919 outputs in block [133,471](block:133,471), and Ocean today) create many. Daily ranges plot the mean per block.",
        }),
    },
    ChartMeta {
        slug: "utxo-growth",
        title: "UTXO Growth Rate",
        desc_per_block: "Net change in the spendable output set per block: every output created, less unspendable outputs, less inputs consumed (coinbase included)",
        desc_daily: "Net change in the spendable output set per day: every output created, less unspendable outputs, less inputs consumed (coinbase included)",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::BarWithLine,
        source: Source::Dashboard {
            per_block: super::utxo_growth_chart,
            daily: Daily::Fn(super::utxo_growth_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Net change in the spendable output set",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::DailyTotal,
            population: "Every output created, less every output that can never be spent, less the inputs consumed. Coinbase outputs are counted, and so are the coinbase's own unspendable outputs; genesis and the two BIP-30 duplicate coinbases add nothing and are treated as such. This reproduces the node's own utxo_increase_actual. Negative means a net reduction; it does not establish consolidation.",
        }],
        about: Some(About {
            definition: Some("Net change in Bitcoin's unspent transaction output (UTXO) set per block or per day. Positive values mean the UTXO set grew (more spendable outputs created than spent); negative values mean it shrank (for example during UTXO consolidation waves)."),
            technical: "Calculated as total outputs created (coinbase included) minus provably unspendable outputs (such as OP_RETURN) minus inputs spent, matching Bitcoin Core's utxo_increase_actual exactly (including genesis and the two BIP-30 duplicate coinbase exceptions).\n\nBecause every full node keeps the active UTXO set ready for block validation, steady UTXO growth increases node memory and disk overhead over time.",
        }),
    },
    ChartMeta {
        slug: "weekday",
        title: "Weekday Activity",
        desc_per_block: "Average transaction count and fees per block, grouped by UTC day of the week",
        desc_daily: "Average transaction count and fees per block, grouped by UTC day of the week",
        category: Category::Network,
        unit: Unit::Mixed,
        shape: Shape::Bar,
        source: Source::Dashboard {
            per_block: super::weekday_activity_chart,
            daily: Daily::Fn(super::weekday_activity_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "Avg Tx Count",
                series_daily: "",
                quantity: "Mean transactions per block, by day of week",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::GroupedSummary,
                daily: Aggregation::GroupedSummary,
                population: "Blocks grouped by the UTC weekday of their timestamp, then transactions over blocks within each group. This is why the chart declares mixed units: the second measurement is denominated in BTC.",
            },
            Measurement {
                series: "Avg Fees (BTC)",
                series_daily: "",
                quantity: "Mean fees per block, by day of week",
                method_per_block: Method::Estimated,
                method_daily: Method::Estimated,
                per_block: Aggregation::GroupedSummary,
                daily: Aggregation::GroupedSummary,
                population: "Fees over blocks within each weekday group. Coinbase-derived, so it inherits that estimate.",
            },
        ],
        about: Some(About {
            definition: Some("Average transactions and total fees per block grouped by UTC day of the week, showing whether on-chain activity dips on weekends or peaks during global business days."),
            technical: "Groups all blocks in the selected range by their UTC weekday and computes the mean transaction count and mean BTC fees per block. Because Bitcoin operates globally across time zones, regional weekday patterns partially overlap across UTC day boundaries.",
        }),
    },
    ChartMeta {
        slug: "weight-util",
        title: "Weight Utilization",
        desc_per_block: "How full each block is, as a percentage of the 4 million weight unit limit",
        desc_daily: "Average daily weight utilization as a percentage of the 4 million weight unit limit",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::weight_utilization_chart,
            daily: Daily::Fn(super::weight_utilization_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Share of the four-million-unit weight limit a block used",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Block weight over 4,000,000. This is the capacity measure, unlike serialized size, because the limit is denominated in weight.",
            },
        ],
        about: Some(About {
            definition: Some("How full each block is against Bitcoin's 4,000,000 weight unit consensus limit. Modern blocks run nearly full: around 98% on average over the last month, and about 95% over the last year."),
            technical: "Divides each block's weight by 4,000,000 (averaged per block on daily ranges). Because witness bytes receive a 75% weight discount, SegWit-heavy blocks fit more data within the same 4 million weight unit limit. Fullness depends on miner template choices as well as fee demand; see [Fee Pressure vs Block Space](/observatory/chart/fee-pressure).",
        }),
    },
    ChartMeta {
        slug: "witness-pct",
        title: "Witness Version Share",
        desc_per_block: "Native SegWit v0 against Taproot as a percentage of those two witness output types",
        desc_daily: "Native SegWit v0 against Taproot as a percentage of those two witness output types",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::witness_version_pct_chart,
            daily: Daily::Fn(super::witness_version_pct_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of outputs created to native SegWit v0 or Taproot",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Native v0 against Taproot, and **only** those two: the denominator is their sum, not all outputs and not pay-to-anchor (P2A, witness v1), so this is the split between v0 and Taproot rather than all witness outputs. Use the Output Type Breakdown chart for a share of every output.",
        }],
        about: Some(About {
            definition: Some("Compares native SegWit v0 outputs against Taproot (v1) outputs as shares totaling 100%. Legacy, P2SH-wrapped, and pay-to-anchor (P2A) outputs are excluded from the denominator so this isolates the split between the two modern payment formats."),
            technical: "Normalizes P2WPKH + P2WSH against P2TR to 100%. Because legacy, P2SH, OP_RETURN, and P2A outputs are excluded, Taproot's share here can rise even if its share of all outputs falls.",
        }),
    },
    ChartMeta {
        slug: "witness-share",
        title: "Witness Data Share",
        desc_per_block: "Witness data as a percentage of serialized block size",
        desc_daily: "Witness data as a percentage of serialized block size",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::witness_share_chart,
            daily: Daily::Fn(super::witness_share_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "",
                series_daily: "",
                quantity: "Witness bytes as a share of serialized block bytes",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Witness bytes over total block bytes. A byte fraction, not a measure of fee saving: the witness discount changes what those bytes cost in weight, not how many bytes they are.",
            },
        ],
        about: Some(About {
            definition: Some("The percentage of each block's serialized bytes taken up by witness data (signatures, scripts, and inscriptions moved out of the base transaction body by SegWit). Currently roughly half of a typical block's raw bytes are witness data."),
            technical: "Divides witness bytes by total serialized block bytes (per block, or summed across the day on daily ranges). Because witness bytes receive a 75% weight discount, blocks with high witness shares can exceed the old 1 MB serialized size while staying within 4,000,000 weight units. Over the last 10,000 blocks, witness share averages about 53%, with fewer than 6% of blocks falling in the 60% to 70% band.",
        }),
    },
    ChartMeta {
        slug: "witness-tx-pct",
        title: "Output Type Breakdown",
        desc_per_block: "SegWit v0, Taproot and other outputs as a percentage of all outputs",
        desc_daily: "SegWit v0, Taproot and other outputs as a percentage of all outputs",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::witness_version_tx_pct_chart,
            daily: Daily::Fn(super::witness_version_tx_pct_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Share of outputs by script generation",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Every output in the block, coinbase included, which is the widest denominator any share chart here uses. The third band is the residual after native v0 and Taproot, so it absorbs OP_RETURN, P2SH-wrapped witness outputs, bare multisig and anything unrecognized, and is named Other outputs rather than Legacy for that reason.",
        }],
        about: Some(About {
            definition: Some("Shows native SegWit v0, Taproot, and all other outputs as percentages of every output in the block (coinbase included), giving the broadest view of output-type shares across the chain."),
            technical: "Divides native SegWit v0 (P2WPKH + P2WSH), Taproot (P2TR), and all remaining outputs by total outputs in each block (coinbase included). The third band is labeled Other outputs because it includes legacy P2PKH, P2SH (including wrapped SegWit), OP_RETURN, bare multisig, and unrecognized scripts.",
        }),
    },
    ChartMeta {
        slug: "witness-versions",
        title: "Witness Version Count",
        desc_per_block: "SegWit v0 (P2WPKH + P2WSH) against Taproot (P2TR) output counts per block",
        desc_daily: "Daily average SegWit v0 against Taproot output counts",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::witness_version_chart,
            daily: Daily::Fn(super::witness_version_chart_daily),
        },
        measurements: &[
            Measurement {
                series: "SegWit v0",
                series_daily: "",
                quantity: "Outputs created to a v0 witness program",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "P2WPKH plus P2WSH outputs, coinbase included. Not every possible witness program: unrecognized or future witness versions are not counted here. The band is drawn as \"SegWit v0\", because \"SegWit\" alone read as every witness output rather than the two native v0 types it counts.",
            },
            Measurement {
                series: "Taproot",
                series_daily: "",
                quantity: "Outputs created to a v1 witness program",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "P2TR outputs, coinbase included.",
            },
        ],
        about: Some(About {
            definition: Some("Counts how many native SegWit v0 (activated at block [481,824](block:481,824) in 2017) and Taproot v1 (activated at block [709,632](block:709,632) in 2021) outputs are created per block."),
            technical: "Counts outputs by witness version directly from stored script classification (coinbase outputs included). Future unrecognized witness versions are excluded.\n\nNo v0 output exists anywhere before block 481,824. In contrast, 20 Taproot (P2TR) outputs were created in 19 blocks before activation at 709,632 (starting on 2019-12-17, with 14 in 2021 and 13 in the last two months before activation). Creating an unknown witness program was already consensus-valid (and anyone-can-spend under consensus until Taproot activated, though non-standard to relay). Enable the BIP activations overlay to compare activation height against creation volume.",
        }),
    },
];

/// Look up a chart by slug. `None` is a 404, not a blank chart.
pub fn find(slug: &str) -> Option<&'static ChartMeta> {
    CHARTS.iter().find(|c| c.slug == slug)
}

/// Charts worth offering next to `meta`, nearest first.
///
/// Computed rather than stored so a newly registered chart shows up in its
/// neighbours' lists without anyone maintaining a list of lists. Same unit in
/// the same category first, since those are the directly comparable ones, then
/// the rest of the category.
/// Charts whose strongest neighbour is in another category, named explicitly.
///
/// `related` derives its answer from category and unit, which are structural
/// facts standing in for "about the same thing". That works for most charts
/// and fails for the ones that matter most: the closest chart to Taproot
/// Spend Types is Ordinals Inscriptions, because inscription reveals are
/// about four fifths of its script-path band, and the derivation excludes it
/// for being filed under Embedded rather than Network.
///
/// A short curated list rather than a rule, because these are exceptions and
/// a rule that caught them would pull in far more than it should. Each entry
/// is checked against the registry by
/// `curated_related_names_charts_that_exist`, so a rename cannot leave a
/// dangling slug the way the drawer once did.
const RELATED_ACROSS_CATEGORIES: &[(&str, &[&str])] = &[
    // Most script-path spends are inscription reveals, and that chart counts
    // them over the same blocks. See the fourth paragraph of this chart's
    // method, which has to explain in prose what these two show together.
    (
        "taproot-spend-types",
        &["inscriptions", "inscription-share"],
    ),
    ("inscriptions", &["taproot-spend-types"]),
    // The witness discount is why inscriptions are affordable at all, so the
    // adoption charts explain the volume ones and the reverse.
    ("inscription-envelope", &["taproot"]),
];

/// The curated neighbours for a chart, resolved to registry entries.
fn curated_related(slug: &str) -> Vec<&'static ChartMeta> {
    RELATED_ACROSS_CATEGORIES
        .iter()
        .find(|(from, _)| *from == slug)
        .map(|(_, to)| {
            to.iter()
                .filter_map(|s| CHARTS.iter().find(|c| c.slug == *s))
                .collect()
        })
        .unwrap_or_default()
}

pub fn related(meta: &ChartMeta, max: usize) -> Vec<&'static ChartMeta> {
    // Curated first: these are named because the derivation below gets them
    // wrong, so burying them under it would defeat the point.
    let mut out: Vec<&'static ChartMeta> = curated_related(meta.slug);
    // Snapshot the curated slugs so the filters below can exclude them
    // without borrowing `out` while it is being extended.
    let curated: Vec<&'static str> = out.iter().map(|c| c.slug).collect();
    let fresh = |c: &&'static ChartMeta| {
        c.slug != meta.slug && !curated.contains(&c.slug)
    };

    out.extend(CHARTS.iter().filter(|c| {
        fresh(c) && c.category == meta.category && c.unit == meta.unit
    }));
    if out.len() < max {
        let want = max - out.len();
        out.extend(
            CHARTS
                .iter()
                .filter(|c| {
                    fresh(c)
                        && c.category == meta.category
                        && c.unit != meta.unit
                })
                .take(want),
        );
    }
    out.truncate(max);
    out
}

/// Charts that can be laid over `meta`, grouped by category for the picker.
///
/// Ordered so the useful answers are near the top: the same category first,
/// since a fee metric next to another fee metric is the comparison someone
/// actually came for, then everything else. Within a group, registry order,
/// which is alphabetical by slug.
///
/// `daily_only` drops charts with no daily aggregate when the selected range
/// is long enough to use them. Offering one there would build an empty series
/// and the comparison would silently not appear.
/// Whether `candidate` may be laid over `primary` at this resolution.
///
/// **The one rule.** Everything that decides a comparison is valid goes here
/// and nowhere else: the picker filters on it, the page resolves the selected
/// slug through it, and the effect that clears a stale selection asks it. Three
/// consumers, one answer, so they cannot drift into disagreeing about what is
/// offerable versus what is renderable.
///
/// One place because the selection is **shared state that survives
/// navigation**: a rule enforced by the picker not rendering would let a
/// comparison arrive on a chart that never offered it.
///
/// Editorial, not structural. This answers "should we offer this", which
/// includes taste: a histogram could technically hold a second line and it
/// would mean nothing. What a chart can *structurally* hold without lying is
/// answered by `charts::apply_comparison`, from the built option, and that is
/// the check no caller can skip.
/// Charts that do not present exactly one metric, and so cannot be laid over
/// another.
///
/// Both ends of that count. Most of these plot several measurements, and
/// offering one means offering a part of it under the whole chart's name.
/// `diff-ribbon` is the other end: seven moving averages with no base series,
/// so there is no metric to lift at all and no honest label for whichever of
/// the seven was chosen. They are refused structurally by
/// `charts::apply_comparison` too, but that refusal happens three layers below
/// the picker, which then advertises a comparison that never draws and
/// explains it as "not available for this range". It is available at no range.
///
/// A hardcoded list because the fact is not derivable from metadata:
/// `diff-adjustment` is `Shape::Bar`, exactly like the single-series bar
/// charts. `the_multi_metric_list_is_exactly_right` computes the truth from
/// the real builders in both directions, so a drifting list fails rather
/// than silently costing a working comparison.
///
/// Twenty of the 63, and the list dissolves once comparisons name a
/// measurement rather than a chart: `notes/phase-2-spec.md`.
//
// `tx-type-evolution` left this list on 2026-09-16 when it was corrected to
// `StackedPercent`: the shape now refuses it, so listing it as well made the
// list claim an exclusion that was doing nothing.
pub const MULTI_METRIC: &[&str] = &[
    "address-types",
    "all-embedded-share",
    "batching",
    "cumulative-adoption",
    "diff-adjustment",
    "diff-ribbon",
    "fee-heatmap",
    "halving-era",
    "inscription-envelope",
    "multi-velocity",
    "opreturn-bytes",
    "opreturn-count",
    "protocol-fee-competition",
    "protocol-fees",
    "subsidy-fees",
    "taproot-spend-types",
    "unified-count",
    "unified-volume",
    "utxo-flow",
    "witness-versions",
];

/// Charts whose x axis is not time, and so cannot take part in a comparison
/// at either end.
///
/// A comparison lays a second series along the primary's x axis, which only
/// means anything if both ends agree on what x *is*. Every other comparable
/// chart positions its points by when they happened: timestamps per block,
/// dates on a category axis once the rows are daily. These two do not.
///
/// - **`fee-pressure`** plots fee rate against block fullness, so x is a
///   percentage. Overlaying a time series on it put milliseconds where
///   percentages belong and dated the result to 1970.
/// - **`halving-era`** draws one bar per halving era, so x is four labels.
///   A 600-point series laid along four categories is not a comparison.
///
/// Excluded as primary as well as candidate, which is the part a narrower
/// rule got wrong. These charts have no x domain a second series can join, so
/// the picker does not appear on them at all rather than appearing full of
/// options that are all refused three layers down.
///
/// Hardcoded for the same reason `MULTI_METRIC` is, and the reason is sharper
/// here: this is **not** derivable from `shape`. `propagation` is
/// `Shape::Scatter` like `fee-pressure` and carries an ordinary time axis, so
/// the two scatter charts sit on opposite sides of this list. Shape describes
/// how a chart draws, not what it measures, which is exactly the substitution
/// phase 2 removes.
///
/// `the_x_axis_exceptions_are_exactly_right` computes the truth from the real
/// builders, and `the_offered_comparisons_can_all_actually_be_drawn` proves
/// the exclusion is sufficient rather than merely present.
pub const NON_TIME_X_AXIS: &[&str] = &["fee-pressure", "halving-era"];

/// Charts whose points are **already changes**, so the rail's "change" figure
/// would be a change of a change.
///
/// The rail reports `last - first`, which answers "how much did this move
/// across the range" and needs the plotted quantity to be a level. Where
/// every point is itself a difference, that subtraction has no meaning: over
/// 1Y, Difficulty Adjustment's first bar is +4.63% and its last is +1.31%, so
/// the rail read **-3.32, -71.7%**, which a reader takes as difficulty having
/// fallen 3.32% in a year. Difficulty actually fell **6.3%** over that window,
/// which is the product of all 26 factors and a different number entirely.
/// Found by browser pass on 2026-09-16, not by any test.
///
/// The other four figures stay, because they do mean something here: the mean
/// retarget, the largest rise and fall with their dates, and how many
/// retargets the range holds.
///
/// Suppressed rather than replaced. A compound figure is what a reader wants
/// on this chart, but computing it in a generic rail means declaring that
/// these points compose multiplicatively, and `utxo-growth` composes
/// additively while `multi-velocity` does not compose at all. That is a
/// per-measurement contract, which is phase 2; saying nothing is the honest
/// interim.
///
/// - **`diff-adjustment`** plots the change at each retarget.
/// - **`utxo-growth`** plots net change in the output set per block or day.
/// - **`multi-velocity`** plots the change in each share over a trailing
///   window. Its rail is already `Unavailable` because it draws several
///   series at the same x, so listing it changes nothing today and is here so
///   that if it ever routes through the series rail it arrives correct.
pub const POINTS_ARE_CHANGES: &[&str] =
    &["diff-adjustment", "multi-velocity", "utxo-growth"];

/// Charts whose x axis is a set of categories rather than a progression, so
/// `last - first` is the difference between two arbitrary ones.
///
/// Distinct from [`POINTS_ARE_CHANGES`], and the distinction is why adding
/// `weekday` there failed: these points are plain readings, not differences.
/// What is wrong is the *order*. Weekday draws seven fixed buckets, so the
/// rail reported "change -80.90, -5.4%", which is Sunday's average minus
/// Monday's. Both numbers are right and neither is a change in anything:
/// Monday is not a start, Sunday is not an end, and because the buckets are
/// fixed the figure does not move with the selected range either, reading the
/// same on 1Y as on ALL.
///
/// **Only the change tile is suppressed.** The mean, the busiest bucket and
/// the quietest one are real facts about seven categories and are the point
/// of the chart.
///
/// Not derivable from `Shape`, because a daily chart is also drawn on a
/// category axis and its categories *are* a progression. The difference is
/// whether the labels are dates, which is a property of the built option, so
/// `a_category_chart_without_dates_reports_no_change` pins this list against
/// what the builders actually emit.
///
/// - **`weekday`** is the one this was written for: seven fixed buckets.
/// - **`halving-era`** draws one bar per era. Its rail is already
///   `NotSummarizable` because it is in [`MULTI_METRIC`], so listing it
///   changes nothing today and is here so that if it ever routes through the
///   series rail it arrives correct. Same reasoning as `multi-velocity` in
///   [`POINTS_ARE_CHANGES`]. The guard found it; it was not on the list when
///   this was written.
pub const X_IS_NOT_A_PROGRESSION: &[&str] = &["halving-era", "weekday"];

/// Charts whose points are a **selection of events** rather than a reading at
/// every x, so the first and last are two arbitrary occurrences.
///
/// Distinct from both lists above, and the distinction is in what is wrong
/// with `last - first`. [`POINTS_ARE_CHANGES`] is about what each point *is*,
/// a difference already. [`X_IS_NOT_A_PROGRESSION`] is about the *order* of
/// the x values. Here the points are plain readings in genuine time order,
/// and the fault is that the series is sparse.
///
/// Fee Spike Detector draws only the blocks that cleared its threshold, 40 of
/// a month's 4,300, so the rail read "change +0.46, +11.4%": the last marked
/// block minus the first marked block. Neither bounds the range, and
/// retuning the threshold so one more block qualifies at either end moves the
/// figure with nothing in the chain having changed.
///
/// **Only the change tile is suppressed.** The average marked block, the
/// largest and smallest, and how many there were are what the chart is for.
///
/// Pinned by `a_sparse_series_reports_no_change`, which finds the sparse
/// charts from the built options rather than trusting this list.
pub const POINTS_ARE_SELECTED_EVENTS: &[&str] = &["fee-spikes"];

pub fn is_valid_comparison(
    primary: &ChartMeta,
    candidate: &ChartMeta,
    daily: bool,
) -> bool {
    primary.can_compare()
        && candidate.can_compare()
        // Not offerable as a comparison, whatever the range.
        && !MULTI_METRIC.contains(&candidate.slug)
        // Both ends have to mean the same thing by x, and these two mean
        // something a time series cannot join. Checked on the primary as
        // well, or the picker fills with candidates that are all refused
        // when drawn.
        && !NON_TIME_X_AXIS.contains(&primary.slug)
        && !NON_TIME_X_AXIS.contains(&candidate.slug)
        // A chart laid over itself draws two identical lines on two axes,
        // which reads as a rendering fault rather than a comparison.
        && candidate.slug != primary.slug
        // Nine charts have no daily builder, so over a long range they would
        // add an axis and no line.
        && (!daily || candidate.has_daily())
}

/// Resolve a slug from the URL or the picker into a comparison, or `None`.
///
/// The single entry point for turning stored state into something to render.
/// An unknown slug, a chart that cannot take part, this chart itself, and a
/// chart with nothing to draw at this range all answer `None`, so a caller
/// that forgets one of those cases cannot exist.
pub fn comparison_for(
    primary: &ChartMeta,
    slug: &str,
    daily: bool,
) -> Option<&'static ChartMeta> {
    find(slug).filter(|c| is_valid_comparison(primary, c, daily))
}

/// Charts that can be laid over `meta`, grouped by category for the picker.
///
/// Ordered so the useful answers are near the top: the same category first,
/// since a fee metric next to another fee metric is the comparison someone
/// actually came for, then everything else. Within a group, registry order,
/// which is alphabetical by slug.
pub fn comparable_with(
    meta: &ChartMeta,
    daily: bool,
) -> Vec<(Category, Vec<&'static ChartMeta>)> {
    let mut groups: Vec<(Category, Vec<&'static ChartMeta>)> = Vec::new();
    // `meta`'s own category first, then the others in the order they appear.
    let mut order: Vec<Category> = vec![meta.category];
    for c in CHARTS.iter() {
        if !order.contains(&c.category) {
            order.push(c.category);
        }
    }
    for cat in order {
        let members: Vec<&'static ChartMeta> = CHARTS
            .iter()
            .filter(|c| c.category == cat)
            .filter(|c| is_valid_comparison(meta, c, daily))
            .collect();
        if !members.is_empty() {
            groups.push((cat, members));
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This file, read as text so a test can inspect a table that lives
    /// inside another test's body. `the_copy_still_matches_the_chain` holds
    /// its rows as a local binding, and hoisting 650 lines out of it to make
    /// them importable would be a far larger change than parsing the two
    /// strings that matter.
    const SELF_SRC: &str = include_str!("registry.rs");

    /// The five page sources, parsed rather than trusted, so the registry
    /// cannot drift from what is actually rendered.
    const PAGES: &[&str] = &[
        include_str!("../../routes/observatory/network.rs"),
        include_str!("../../routes/observatory/fees.rs"),
        include_str!("../../routes/observatory/mining.rs"),
        include_str!("../../routes/observatory/embedded.rs"),
        include_str!("../../routes/observatory/post_quantum.rs"),
    ];

    /// Every `<ChartCard>` on a page, as (slug, title). Each chunk after a
    /// `<ChartCard` opener holds that tag's attributes, so the first
    /// occurrence of each attribute in the chunk belongs to the tag itself.
    fn rendered_charts() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for src in PAGES {
            for chunk in src.split("<ChartCard").skip(1) {
                let slug = chunk
                    .split("chart_id=\"")
                    .nth(1)
                    .and_then(|r| r.split('"').next());
                let title = chunk
                    .split("title=\"")
                    .nth(1)
                    .and_then(|r| r.split('"').next());
                if let (Some(s), Some(t)) = (slug, title) {
                    out.push((
                        s.trim_start_matches("chart-").to_string(),
                        t.to_string(),
                    ));
                }
            }
        }
        out
    }

    /// The long copy is the site's answer to "why should I believe this
    /// number", so the half that answers it cannot be missing, and neither
    /// half may quietly become a restatement of the one-liner already on the
    /// page above it.
    #[test]
    fn the_long_copy_says_something_the_short_copy_does_not() {
        let written: Vec<&ChartMeta> =
            CHARTS.iter().filter(|c| c.about.is_some()).collect();
        assert!(
            written.len() >= 40,
            "only {} charts have long copy; the card expandables were \
             migrated into the registry, so this should cover most of them",
            written.len()
        );
        for c in written {
            let about = c.about.unwrap();
            for (part, text) in about
                .definition
                .map(|d| ("definition", d))
                .into_iter()
                .chain([("technical", about.technical)])
            {
                assert!(
                    text.len() > 160,
                    "{}'s {part} is too short to be more than a restated \
                     one-liner",
                    c.slug
                );
                assert!(
                    text.trim().ends_with('.'),
                    "{}'s {part} does not end in a sentence",
                    c.slug
                );
                assert_ne!(
                    text, c.desc_per_block,
                    "{}'s {part} just repeats the card description",
                    c.slug
                );
                // The site's copy rule, enforced where the copy lives.
                assert!(
                    !text.contains('\u{2014}') && !text.contains('\u{2013}'),
                    "{}'s {part} contains a dash character",
                    c.slug
                );
            }
        }
    }

    /// Chart 64 cannot arrive without saying what it measures in words a
    /// reader who has not met it can use.
    ///
    /// This was a ceiling that only ever moved down, from 48 on 2026-09-16,
    /// because most charts explained how their number was computed without
    /// saying what it was. All 63 carry both halves now, so it is an
    /// absolute. The field stays `Option` because that is what let the gap
    /// be closed incrementally rather than in one unreviewable commit.
    #[test]
    fn every_chart_defines_itself() {
        let missing: Vec<&str> = CHARTS
            .iter()
            .filter(|c| c.about.is_none_or(|a| a.definition.is_none()))
            .map(|c| c.slug)
            .collect();
        assert!(
            missing.is_empty(),
            "every chart says what its metric is, so a new one cannot be \
             registered without a definition: {missing:?}"
        );
    }

    /// Copy on the page describes the metric, never this codebase's history
    /// with it. "Plotted raw and abbreviated on the axis: an earlier version
    /// divided by a trillion" shipped in the difficulty copy and reads as a
    /// code comment that escaped, because that is what it was.
    ///
    /// The style to match is the opening of any `definition`: present tense,
    /// direct, about the thing itself.
    /// The ribbon's copy has to name the spans the builders actually use.
    ///
    /// It said daily used "the same counts" as per-block, which is true of six
    /// of the seven and false of the shortest: the daily builder smooths over 7
    /// days, not 9. The entry's own `population` field had it right, so one
    /// `ChartMeta` stated both. Found by review on 2026-09-21. Reading the
    /// constants rather than restating them is what keeps this honest.
    #[test]
    fn the_ribbon_copy_names_the_spans_the_builders_use() {
        let ribbon = CHARTS
            .iter()
            .find(|c| c.slug == "diff-ribbon")
            .expect("the ribbon is registered");
        let technical = ribbon.about.expect("ribbon has About").technical;
        let population = ribbon.measurements[0].population;

        // Both sequences, in order, in both fields. A sequence is searched as
        // its rendered text so a reordering fails rather than passing on the
        // strength of every number being present somewhere.
        let render = |w: &[usize]| {
            w.iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let per_block = render(&crate::stats::charts::RIBBON_WINDOWS_PER_BLOCK);
        let daily = render(&crate::stats::charts::RIBBON_WINDOWS_DAILY);
        // Rendered as "9, 14, 25, 40, 60, 90, 128", while the prose ends with
        // "and 128", so compare on the part before the final separator.
        let head =
            |s: &str| s.rsplit_once(", ").expect("seven spans").0.to_string();

        for (field, text) in
            [("technical", technical), ("population", population)]
        {
            assert!(
                text.contains(&head(&per_block)),
                "the ribbon's {field} copy does not name the per-block spans \
                 {per_block}: {text}"
            );
            assert!(
                text.contains(&head(&daily)),
                "the ribbon's {field} copy does not name the daily spans \
                 {daily}: {text}"
            );
            assert!(
                !text.to_lowercase().contains("the same counts"),
                "the ribbon's {field} copy claims the two arms share their \
                 spans; only six of the seven are shared: {text}"
            );
        }
    }

    /// Every chart has a sitemap entry, and the sitemap has no dead ones.
    ///
    /// The branch added 63 indexable URLs and did not touch `sitemap.xml`, so
    /// none of them was discoverable. A hand-maintained list beside a
    /// generated one is the drift this project has already shipped once, as a
    /// dangling drawer link, so the list is pinned rather than trusted.
    #[test]
    fn every_chart_is_in_the_sitemap_and_the_sitemap_has_no_ghosts() {
        const SITEMAP: &str = include_str!("../../../assets/sitemap.xml");
        const PREFIX: &str = "https://www.wehodlbtc.com/observatory/chart/";

        for c in CHARTS {
            assert!(
                SITEMAP.contains(&format!("{PREFIX}{}<", c.slug)),
                "{} is a public chart URL with no sitemap entry, so it is                  not discoverable",
                c.slug
            );
        }
        // And nothing in the sitemap points at a chart that no longer exists.
        for line in SITEMAP.lines() {
            let Some(rest) = line.trim().strip_prefix("<loc>") else {
                continue;
            };
            let Some(url) = rest.strip_suffix("</loc>") else {
                continue;
            };
            let Some(slug) = url.strip_prefix(PREFIX) else {
                continue;
            };
            assert!(
                CHARTS.iter().any(|c| c.slug == slug),
                "the sitemap advertises /observatory/chart/{slug}, which is                  not a registered chart"
            );
        }
    }

    /// A number carries its unit on the same line, and a non-unit carries none.
    ///
    /// The rail passed the unit as a Fact's `note`, which renders on its own
    /// line because that slot holds the date under peak and low, so a
    /// percentage chart stranded a bare "%" beneath its average. Found by the
    /// owner during the acceptance pass on 2026-09-22.
    #[test]
    fn a_unit_qualifies_its_value_on_one_line() {
        assert_eq!(Unit::Percent.qualify("68.04"), "68.04%");
        assert_eq!(Unit::Btc.qualify("1.25"), "1.25 BTC");
        assert_eq!(Unit::SatVb.qualify("3.08"), "3.08 sat/vB");
        assert_eq!(Unit::PercentagePoints.qualify("+3.67"), "+3.67 pp");

        // The four that name the absence of a unit must not be appended:
        // "3,791 count" is worse than "3,791", and "mixed" is the same
        // generic that shipped in a meta description as "measured in mixed".
        for u in [Unit::Count, Unit::Difficulty, Unit::Ratio, Unit::Mixed] {
            assert_eq!(
                u.qualify("3,791"),
                "3,791",
                "{u:?} is a name for having no unit and must qualify nothing"
            );
        }

        // Every variant produces something a reader can read: no empty
        // suffix, no doubled space.
        for u in [
            Unit::Count,
            Unit::Btc,
            Unit::Sats,
            Unit::SatVb,
            Unit::Bytes,
            Unit::Kilobytes,
            Unit::Megabytes,
            Unit::Gigabytes,
            Unit::Percent,
            Unit::PercentagePoints,
            Unit::TxPerSec,
            Unit::HashesPerSecond,
            Unit::Minutes,
            Unit::Seconds,
            Unit::Difficulty,
            Unit::Ratio,
            Unit::Mixed,
        ] {
            let q = u.qualify("1");
            assert!(q.starts_with('1'), "{u:?} lost its value: {q}");
            assert!(!q.contains("  "), "{u:?} doubled a space: {q}");
            assert!(!q.ends_with(' '), "{u:?} left a trailing space: {q}");
        }
    }

    /// A method that runs long is broken into paragraphs.
    ///
    /// The single-chart view splits `technical` on a blank line, so copy that
    /// asks for no break still renders as one paragraph. What this holds is
    /// the other end: the interval chart's method covers the per-block
    /// difference, the consensus timestamp rules and the daily arm's
    /// different quantity, and as one blob that was ten unbroken lines across
    /// three subjects. Found by the owner on 2026-09-23.
    ///
    /// The threshold is deliberately generous. This is not a style rule about
    /// sentence length; it is a floor under the one case where a reader has
    /// to separate several subjects unaided.
    #[test]
    fn a_long_method_is_broken_into_paragraphs() {
        const TOO_LONG_UNBROKEN: usize = 900;
        for c in CHARTS.iter().filter(|c| c.about.is_some()) {
            let t = c.about.unwrap().technical;
            let longest = t.split("\n\n").map(str::len).max().unwrap_or(0);
            assert!(
                longest <= TOO_LONG_UNBROKEN,
                "{}'s method has an unbroken run of {longest} characters. \
                 Split it on a blank line at a subject boundary; the view \
                 renders each as its own paragraph.",
                c.slug
            );
        }
    }

    /// A figure measured over a recent window is hedged, not exact.
    ///
    /// Five sentences quoted a figure to two decimals over a window the chart
    /// does not offer: "97.2% of the last four months' blocks", "92.4% over
    /// the last 67,000 blocks", "6.20% against 5.88% over the last 1,000
    /// blocks". Three of the five were already wrong when checked on
    /// 2026-09-23, and the 1,000-block one had moved by more than a point,
    /// because a thousand blocks is a week.
    ///
    /// A number that describes a trailing window ages the moment it is
    /// written, so either round it ("around 97%") or tie it to something
    /// closed. An exact decimal is fine for a fact that does not move: a
    /// consensus limit, a named block, an all-time total.
    #[test]
    fn a_recent_window_figure_is_not_quoted_to_two_decimals() {
        let window = regex_lite_window;
        for c in CHARTS.iter() {
            let fields = [
                Some(c.desc_per_block),
                Some(c.desc_daily),
                c.about.and_then(|a| a.definition),
                c.about.map(|a| a.technical),
            ];
            for text in fields.into_iter().flatten() {
                for sentence in text.split(". ") {
                    if !window(sentence) {
                        continue;
                    }
                    // A decimal percentage in the same sentence as a trailing
                    // window is the shape that rots.
                    let decimal_pct = sentence.split_whitespace().any(|w| {
                        let w = w.trim_end_matches(&[',', '.', ';'][..]);
                        w.ends_with('%')
                            && w.trim_end_matches('%').contains('.')
                    });
                    assert!(
                        !decimal_pct,
                        "{}: \"{}\" quotes a decimal percentage over a \
                         trailing window. Round it, or tie it to a window \
                         that has closed.",
                        c.slug,
                        sentence.trim()
                    );
                }
            }
        }
    }

    /// True when a sentence describes a trailing window rather than a fixed
    /// point in time. Deliberately small: `regex` is not a dependency here and
    /// is not worth adding for four phrases.
    fn regex_lite_window(sentence: &str) -> bool {
        let l = sentence.to_lowercase();
        // "currently" and "now" are trailing windows too, and they are the
        // commonest phrasing: the first version of this guard listed only
        // "last/past/recent" and missed four sentences, all of which had
        // drifted. They need no unit word, because the window is implicit.
        //
        // "recent" is in that group rather than the one below it, which is
        // the second hole this guard has had. Requiring a unit word beside it
        // let "67.7% of recent transactions" through, because the thing being
        // counted is not a unit of time: the window is in the adjective, and
        // whatever it modifies is irrelevant. That figure then drifted to
        // 67.9%, on a line whose monthly share has run from 49% to 78% since it
        // first passed half in 2023.
        //
        // `now` is matched on word boundaries rather than as `" now "`, which
        // could never fire on a sentence that *starts* with it. The guard
        // splits on ". ", so "Now 67.7% of transactions carry it" is the
        // commonest phrasing of all and was structurally unreachable.
        let word_now = l
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| w == "now");
        if word_now
            || ["currently", "today", "recent"]
                .iter()
                .any(|p| l.contains(p))
        {
            return true;
        }
        // "last" and "past" do still need one, because both have meanings
        // that are not windows at all: the last band of a stack, a past
        // activation.
        ["last ", "past "].iter().any(|p| l.contains(p))
            && ["month", "week", "day", "year", "block"]
                .iter()
                .any(|u| l.contains(u))
    }

    /// A decimal percentage is not pinned to the year that is still running.
    ///
    /// The third hole in the trailing-window guards, and it is invisible to
    /// both of the others because nothing in the sentence says "recent" or
    /// "now": the window is named by the year itself. `fee-revenue-share`
    /// carried "0.63% so far in 2026" and "fell to ... 0.63% in 2026", which
    /// read as closed facts and are not, since the year has months left to
    /// run. Found by an independent audit on 2026-09-24.
    ///
    /// **Proximity, not co-occurrence, and the difference decides a real
    /// sentence.** `runes-pct` says "96.9% of them in 2024, the year they
    /// launched, and over 97% so far in 2026": the decimal belongs to a
    /// closed year and the open year is deliberately hedged, which is the
    /// correct way to write this. A guard that flagged any sentence mentioning
    /// both would have condemned the one sentence here already doing it right.
    #[test]
    fn a_decimal_percentage_is_not_pinned_to_the_year_still_running() {
        // How many words may sit between the figure and the year before they
        // are no longer talking about each other. Four spans "0.63% so far in
        // 2026" and stops short of runes-pct's nine.
        const REACH: usize = 4;
        let year = chrono::Utc::now().format("%Y").to_string();

        // A decimal figure needs its inner dot kept, and a sentence-final
        // year needs its outer one dropped. Trimming once cannot do both:
        // the first version of this kept `.` everywhere, so the last word of
        // a paragraph was "2026." and never matched the year. It passed the
        // suite and caught nothing, which a mutation showed and a green run
        // would not have.
        let trim = |w: &str| {
            w.trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '%' && c != '.'
            })
            .trim_end_matches('.')
            .to_string()
        };
        for c in CHARTS.iter() {
            let fields = [
                Some(c.desc_per_block),
                Some(c.desc_daily),
                c.about.and_then(|a| a.definition),
                c.about.map(|a| a.technical),
            ];
            for text in fields.into_iter().flatten() {
                for sentence in text.split(". ") {
                    let words: Vec<String> =
                        sentence.split_whitespace().map(&trim).collect();
                    for (i, w) in words.iter().enumerate() {
                        if *w != year {
                            continue;
                        }
                        let from = i.saturating_sub(REACH);
                        let near = words[from..i].iter().any(|v| {
                            v.ends_with('%')
                                && v.trim_end_matches('%').contains('.')
                        });
                        assert!(
                            !near,
                            "{}: \"{}\" pins a decimal percentage to {year}, \
                             which is still running, so the figure moves every \
                             month. Hedge it, or quote a year that has closed.",
                            c.slug,
                            sentence.trim()
                        );
                    }
                }
            }
        }
    }

    /// The deploy's cache bump still matches the file it rewrites.
    ///
    /// `deploy.yml` bumps the service worker with a `sed` anchored on
    /// `var CACHE_NAME = 'wehodlbtc-...'`. Changing `sw.js` bytes is the only
    /// thing that makes a browser treat the worker as updated, which is what
    /// fires the Refresh banner and purges the old cache. A `sed` that
    /// matches nothing exits 0, so if that line is ever reformatted the bump
    /// silently stops happening and every later deploy leaves returning
    /// visitors on stale assets: old CSS against new markup, which collapses
    /// the layout and leaves charts in a zero-height parent.
    ///
    /// Seen locally on 2026-09-23 from the browser's own HTTP cache, which is
    /// what prompted this. That instance was harmless; the silent version of
    /// it would not be, and nothing else would catch it.
    #[test]
    fn the_deploy_can_still_bump_the_service_worker() {
        const SW: &str = include_str!("../../../assets/sw.js");
        const DEPLOY: &str =
            include_str!("../../../.github/workflows/deploy.yml");
        const ANCHOR: &str = "var CACHE_NAME = 'wehodlbtc-";

        assert!(
            SW.contains(ANCHOR),
            "assets/sw.js no longer declares the cache name in the form \
             deploy.yml rewrites ({ANCHOR}...), so the bump will silently \
             match nothing and returning visitors will keep stale assets"
        );
        assert!(
            DEPLOY.contains(ANCHOR),
            "deploy.yml no longer anchors its cache bump on {ANCHOR}; if the \
             rewrite moved, this guard has to move with it"
        );
    }

    /// Every figure in the copy, re-measured against the real chain.
    ///
    /// **This is the check the normal suite cannot do.** Copy is
    /// `&'static str`, so a number in it is a snapshot of the day it was
    /// written. The other guards here hold the *shape* that rots fastest, a
    /// decimal percentage over a trailing window, but a hedged figure is not
    /// self-correcting: "around 97%" reads fine at 85%. Nine figures had
    /// drifted by the time anyone looked, and four of those were found by a
    /// reader asking "why four months?" rather than by anything automated.
    ///
    /// Ignored because it needs `bitcoin_stats.db`, which is 588 MB and
    /// gitignored, so CI has no data and every other test here builds from
    /// synthetic fixtures. Run it when the copy is edited, and periodically:
    ///
    /// ```text
    /// cargo test --features ssr the_copy_still_matches -- --ignored --nocapture
    /// ```
    ///
    /// Bounds are deliberately wider than the claim, because the point is to
    /// catch a sentence that has become *wrong*, not one that has moved in
    /// the last decimal. A failure means the copy needs rewriting; it does
    /// not mean the chart is broken.
    ///
    /// # DELETE THIS WHEN PHASE 2 LANDS
    ///
    /// This is a stopgap and should not outlive the prepared-metric contract.
    /// It had the same defect it exists to catch, one level up: it knew only
    /// the claims typed into the table below, so an untyped figure was
    /// invisible to it. `you_cannot_ship_an_unguarded_number` closes that for
    /// figures written in digits, and the table is still a hand-maintained
    /// list of facts that can go stale.
    ///
    /// The actual fix is for copy to reference a value the data already
    /// computes rather than restate it, which is what a prepared metric makes
    /// possible: `notes/phase-2-spec.md`, Part 1. When a chart's current
    /// figure can be interpolated into its own description, every row here
    /// becomes unnecessary and this test should be removed rather than
    /// extended. Reconciliation and sequencing:
    /// `notes/chart-quality-2026-09-15/runs/phase-2-reconciliation.md`.
    #[test]
    #[ignore]
    fn the_copy_still_matches_the_chain() {
        let conn = rusqlite::Connection::open_with_flags(
            "bitcoin_stats.db",
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("bitcoin_stats.db in the working directory");

        // (which chart, what the copy says, the query, the range it must
        // still fall in). The claim text is quoted so a failure names the
        // sentence to edit rather than only a number, and
        // `you_cannot_ship_an_unguarded_number` reads these same strings out
        // of the source to decide which figures are covered.
        let checks: &[(&str, &str, &str, f64, f64)] = &[
            (
                "weight-util",
                "around 98% on average over the last month",
                "SELECT AVG(weight)*100.0/4000000 FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-30 days')",
                95.0,
                100.0,
            ),
            (
                "weight-util",
                "about 95% over the last year",
                "SELECT AVG(weight)*100.0/4000000 FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-365 days')",
                91.0,
                98.0,
            ),
            (
                "fee-pressure",
                "Nearly every block today is filled to the weight limit",
                "SELECT 100.0*SUM(CASE WHEN weight >= 0.99*4000000 THEN 1 \
                 ELSE 0 END)/COUNT(*) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-30 days')",
                90.0,
                100.0,
            ),
            // Exact count of sub-1 sat/vB blocks with user transactions in
            // 2019-2024: heights 600,590, 617,750 and 626,507 (all 3-tx
            // zero-fee blocks). Do not filter `median_fee_rate > 0`, which
            // silently excluded those three blocks.
            (
                "fee-pressure",
                "Between 2019 and 2024 only 3 blocks with user transactions \
                 carried a median under 1 sat/vB",
                "SELECT COUNT(*) FROM blocks \
                 WHERE tx_count > 1 AND median_fee_rate < 1 \
                 AND timestamp >= strftime('%s','2019-01-01') \
                 AND timestamp < strftime('%s','2025-01-01')",
                3.0,
                3.0,
            ),
            (
                "fullness-dist",
                "around 97% of the last month's blocks are above 99%",
                "SELECT 100.0*SUM(CASE WHEN weight > 0.99*4000000 THEN 1 \
                 ELSE 0 END)/COUNT(*) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-30 days')",
                92.0,
                100.0,
            ),
            // The "both sit near 7%" wording this replaces went when the
            // copy stopped claiming the two lines were a seesaw. What is
            // still load-bearing is that Taproot leads, and the margin is
            // only about a point, so this is the likeliest row here to fail.
            (
                "p2pkh-sunset",
                "Taproot overtook it in 2023 and is still ahead",
                "SELECT (100.0*SUM(p2tr_count) - 100.0*SUM(p2pkh_count)) \
                 /SUM(p2pkh_count+p2sh_count+p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count+multisig_count+unknown_script_count) \
                 FROM blocks WHERE height >= \
                 (SELECT MAX(height)-9999 FROM blocks)",
                0.1,
                20.0,
            ),
            (
                "p2pkh-sunset",
                "Taproot first passed P2PKH in 2023",
                "SELECT CAST(strftime('%Y', datetime(timestamp,'unixepoch')) \
                 AS REAL) FROM blocks GROUP BY \
                 strftime('%Y-%m', datetime(timestamp,'unixepoch')) \
                 HAVING SUM(p2tr_count) > SUM(p2pkh_count) \
                 ORDER BY MIN(height) LIMIT 1",
                2023.0,
                2023.0,
            ),
            (
                "p2pkh-sunset",
                "nearly nine in ten outputs between P2SH and SegWit",
                "SELECT 100.0*SUM(p2pkh_count)/SUM(p2pkh_count+p2sh_count+p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count+multisig_count+unknown_script_count) \
                 FROM blocks WHERE height >= 173805 AND height < 481824",
                85.0,
                92.0,
            ),
            (
                "taproot-spend-types",
                "Over the last 10,000 blocks around nine in ten are key-path",
                "SELECT 100.0*SUM(taproot_keypath_count)/ \
                 NULLIF(SUM(taproot_keypath_count+taproot_scriptpath_count),0) \
                 FROM blocks WHERE height >= \
                 (SELECT MAX(height)-9999 FROM blocks)",
                85.0,
                95.0,
            ),
            (
                "taproot-spend-types",
                "across the past year it has run around 69%",
                "SELECT 100.0*SUM(taproot_keypath_count)/ \
                 NULLIF(SUM(taproot_keypath_count+taproot_scriptpath_count),0) \
                 FROM blocks WHERE timestamp >= strftime('%s','now','-365 days')",
                60.0,
                80.0,
            ),
            (
                "taproot-spend-types",
                "through 2023 script-path spends outnumbered key-path more than two to one",
                "SELECT 100.0*SUM(taproot_keypath_count)/ \
                 NULLIF(SUM(taproot_keypath_count+taproot_scriptpath_count),0) \
                 FROM blocks WHERE timestamp >= strftime('%s','2023-01-01') \
                 AND timestamp < strftime('%s','2024-01-01')",
                25.0,
                33.3,
            ),
            (
                "tx-type-evolution",
                "close to nine in ten transactions now carry a witness",
                "SELECT 100.0*SUM(segwit_tx_count+taproot_tx_count)/ \
                 NULLIF(SUM(tx_count-1),0) FROM blocks WHERE height >= \
                 (SELECT MAX(height)-9999 FROM blocks)",
                84.0,
                95.0,
            ),
            (
                "batching",
                "around 2.3 outputs per transaction",
                "SELECT SUM(output_count - coinbase_output_count)*1.0/SUM(tx_count-1) \
                 FROM blocks WHERE timestamp >= strftime('%s','now','-90 days') \
                 AND tx_count > 1",
                2.0,
                2.7,
            ),
            (
                "batching",
                "and 1.7 inputs per transaction",
                "SELECT SUM(input_count)*1.0/SUM(tx_count-1) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-90 days') \
                 AND tx_count > 1",
                1.4,
                2.2,
            ),
            // Only ever grows, so bound both floor and ceiling tightly.
            (
                "inscription-envelope",
                "more than 200,000 blocks holding a detection",
                "SELECT COUNT(*) FROM blocks WHERE inscription_count > 0",
                200_000.0,
                260_000.0,
            ),
            (
                "inscription-envelope",
                "the median block puts them at 18.0%",
                "SELECT pct FROM (SELECT 100.0*(inscription_envelope_bytes \
                 - inscription_bytes)/inscription_envelope_bytes AS pct \
                 FROM blocks WHERE inscription_count > 0 \
                 AND inscription_envelope_bytes > 0 ORDER BY pct) \
                 LIMIT 1 OFFSET (SELECT COUNT(*)/2 FROM blocks \
                 WHERE inscription_count > 0 AND inscription_envelope_bytes > 0)",
                16.5,
                19.5,
            ),
            (
                "inscription-fee-share",
                "22.7% of all fees in 2023",
                "SELECT 100.0*SUM(inscription_fees)/NULLIF(SUM(total_fees),0) \
                 FROM blocks WHERE timestamp >= strftime('%s','2023-01-01') \
                 AND timestamp < strftime('%s','2024-01-01')",
                22.4,
                23.0,
            ),
            (
                "inscription-fee-share",
                "11.2% in 2024",
                "SELECT 100.0*SUM(inscription_fees)/NULLIF(SUM(total_fees),0) \
                 FROM blocks WHERE timestamp >= strftime('%s','2024-01-01') \
                 AND timestamp < strftime('%s','2025-01-01')",
                10.9,
                11.5,
            ),
            (
                "inscription-fee-share",
                "and 7.5% in 2025",
                "SELECT 100.0*SUM(inscription_fees)/NULLIF(SUM(total_fees),0) \
                 FROM blocks WHERE timestamp >= strftime('%s','2025-01-01') \
                 AND timestamp < strftime('%s','2026-01-01')",
                7.2,
                7.8,
            ),
            (
                "inscriptions",
                "block 767,430 on 2022-12-14",
                "SELECT MIN(height) FROM blocks WHERE inscription_count > 0",
                767_430.0,
                767_430.0,
            ),
            (
                "inscription-share",
                "both 99.96% inscription by weight",
                "SELECT MIN(100.0*inscription_envelope_bytes/weight) \
                 FROM blocks WHERE height IN (892279, 899687)",
                99.9,
                100.0,
            ),
            (
                "witness-share",
                "about 53% over the last 10,000 blocks",
                "SELECT 100.0*SUM(witness_bytes)/NULLIF(SUM(size),0) \
                 FROM blocks WHERE height >= \
                 (SELECT MAX(height)-9999 FROM blocks)",
                47.0,
                59.0,
            ),
            (
                "fee-revenue-share",
                "more than half of all blocks are under 1%",
                "SELECT 100.0*SUM(CASE WHEN total_fees*1.0/(total_fees+ \
                 CASE WHEN height<210000 THEN 5000000000 \
                 WHEN height<420000 THEN 2500000000 \
                 WHEN height<630000 THEN 1250000000 \
                 WHEN height<840000 THEN 625000000 \
                 ELSE 312500000 END) < 0.01 THEN 1 ELSE 0 END)/COUNT(*) \
                 FROM blocks",
                50.0,
                70.0,
            ),
            (
                "fee-revenue-share",
                "under 1% so far in 2026",
                "SELECT 100.0*SUM(total_fees)/SUM(total_fees + 312500000) \
                 FROM blocks WHERE timestamp >= strftime('%s','2026-01-01') \
                 AND timestamp < strftime('%s','2027-01-01')",
                0.0,
                1.0,
            ),
            (
                "empty-blocks",
                "close to 90,000 exist in the whole chain",
                "SELECT COUNT(*) FROM blocks WHERE tx_count = 1",
                88_000.0,
                92_000.0,
            ),
            (
                "empty-blocks",
                "78,800 of them are from 2009 and 2010",
                "SELECT COUNT(*) FROM blocks WHERE tx_count=1 \
                 AND timestamp < strftime('%s','2011-01-01')",
                78_800.0,
                78_800.0,
            ),
            (
                "empty-blocks",
                "and 91.6% predate 2012",
                "SELECT 100.0*SUM(CASE WHEN timestamp < strftime('%s','2012-01-01') \
                 THEN 1 ELSE 0 END)/COUNT(*) FROM blocks WHERE tx_count = 1",
                91.0,
                92.2,
            ),
            (
                "empty-blocks",
                "86,656 are unattributed",
                "SELECT COUNT(*) FROM blocks WHERE tx_count = 1 AND miner = 'Unknown'",
                86_656.0,
                86_656.0,
            ),
            (
                "address-types",
                // Days with none, not a rate per block. A rate is the wrong
                // shape for something that arrives in bursts: "one block in
                // seven hundred" was measured in a quiet stretch and read
                // seven times too low the moment one started, on 2026-11-15.
                // The share of empty days survives a burst, because a burst
                // is concentrated into days rather than spread across them.
                "most days have none at all",
                "SELECT 100.0*SUM(CASE WHEN n = 0 THEN 1 ELSE 0 END)/ \
                 COUNT(*) FROM (SELECT SUM(p2pk_count) n FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-365 days') \
                 GROUP BY date(timestamp,'unixepoch'))",
                60.0,
                100.0,
            ),
            (
                "pq-exposed-value",
                // Matches `classified()` in `post_quantum.rs` exactly: do not
                // add `p2a_count` on top of `unknown_script_count`, which
                // already includes P2A and would double-count it.
                "since 2020, 15.2% publish a key by count",
                "SELECT 100.0*SUM(p2pk_count+multisig_count+p2tr_count)/ \
                 SUM(p2pkh_count+p2sh_count+p2wpkh_count+p2wsh_count \
                 +p2tr_count+p2pk_count+multisig_count+unknown_script_count) \
                 FROM blocks WHERE timestamp >= strftime('%s','2020-01-01')",
                14.0,
                16.5,
            ),
            (
                "pq-exposed-value",
                // The value arm, against the same window and the same
                // coinbase-inclusive denominator the builder uses.
                "and 1.8% by value",
                "SELECT 100.0*SUM(p2pk_value+p2tr_value+multisig_value)/ \
                 SUM(total_output_value + total_fees + \
                 CASE WHEN height < 210000 THEN 5000000000 \
                      WHEN height < 420000 THEN 2500000000 \
                      WHEN height < 630000 THEN 1250000000 \
                      WHEN height < 840000 THEN 625000000 \
                      ELSE 312500000 END) FROM blocks \
                 WHERE timestamp >= strftime('%s','2020-01-01')",
                1.4,
                2.2,
            ),
            (
                "pq-exposed-value",
                "average Taproot output created since 2020 holds about 0.18 BTC",
                "SELECT 1.0*SUM(p2tr_value)/SUM(p2tr_count)/1e8 FROM blocks \
                 WHERE timestamp >= strftime('%s','2020-01-01')",
                0.14,
                0.24,
            ),
            (
                "pq-exposed-value",
                "against about 1.58 BTC for the average classified output",
                "SELECT 1.0*SUM(total_output_value + total_fees + \
                 CASE WHEN height < 210000 THEN 5000000000 \
                      WHEN height < 420000 THEN 2500000000 \
                      WHEN height < 630000 THEN 1250000000 \
                      WHEN height < 840000 THEN 625000000 \
                      ELSE 312500000 END) \
                 /SUM(p2pkh_count+p2sh_count+p2wpkh_count+p2wsh_count \
                 +p2tr_count+p2pk_count+multisig_count+unknown_script_count)/1e8 \
                 FROM blocks WHERE timestamp >= strftime('%s','2020-01-01')",
                1.2,
                2.0,
            ),
            (
                "pq-p2pk-creation",
                "more P2PK outputs were created in 2019 than in any other year",
                "SELECT CAST(strftime('%Y', datetime(timestamp,'unixepoch')) AS REAL) \
                 FROM blocks GROUP BY 1 ORDER BY SUM(p2pk_count) DESC LIMIT 1",
                2019.0,
                2019.0,
            ),
            (
                "pq-p2pk-creation",
                "71% of all P2PK outputs ever made were created between 2017 and 2021",
                "SELECT 100.0*SUM(CASE WHEN timestamp >= strftime('%s','2017-01-01') \
                 AND timestamp < strftime('%s','2022-01-01') THEN p2pk_count ELSE 0 END) \
                 /SUM(p2pk_count) FROM blocks",
                69.0,
                73.0,
            ),
            (
                "address-types",
                "six P2SH outputs in five blocks before BIP-16 activated",
                "SELECT SUM(p2sh_count) FROM blocks WHERE p2sh_count > 0 \
                 AND height < 173805",
                6.0,
                6.0,
            ),
            (
                "address-types-pct",
                "the six-type share is never smaller than the eight-type share",
                "SELECT (100.0*SUM(p2pkh_count)/SUM(p2pkh_count+p2sh_count \
                 +p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count)) \
                 - (100.0*SUM(p2pkh_count)/SUM(p2pkh_count+p2sh_count \
                 +p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count \
                 +multisig_count+unknown_script_count)) FROM blocks \
                 WHERE height >= (SELECT MAX(height)-9999 FROM blocks)",
                0.0,
                5.0,
            ),
            (
                "witness-pct",
                "a share of witness outputs reads higher than a share of all \
                 outputs",
                "SELECT (100.0*SUM(p2tr_count)/SUM(p2wpkh_count \
                 +p2wsh_count+p2tr_count)) \
                 - (100.0*SUM(p2tr_count)/SUM(output_count)) FROM blocks \
                 WHERE height >= (SELECT MAX(height)-9999 FROM blocks)",
                0.5,
                20.0,
            ),
            (
                "taproot-spend-types",
                "inscriptions account for most of the script-path band",
                "SELECT 1.0*SUM(inscription_count)/ \
                 NULLIF(SUM(taproot_scriptpath_count),0) FROM blocks \
                 WHERE height >= (SELECT MAX(height)-9999 FROM blocks)",
                0.5,
                1.2,
            ),
            (
                "witness-versions",
                "20 P2TR outputs in 19 blocks before 709,632",
                "SELECT SUM(p2tr_count) FROM blocks WHERE height < 709632",
                20.0,
                20.0,
            ),
            (
                "witness-versions",
                "no v0 output exists anywhere before block 481,824",
                "SELECT COALESCE(SUM(p2wpkh_count+p2wsh_count),0) \
                 FROM blocks WHERE height < 481824",
                0.0,
                0.0,
            ),
            // Both ends of the range the sentence claims, measured as the
            // sentence measures it. The first version of this row read one
            // current 10,000-block window against a claim about every month
            // since 2023, so a month at 85% would have falsified the copy and
            // passed the guard. Measure the statistic the sentence quotes.
            (
                "rbf",
                "has run between roughly a half and four fifths (the low)",
                "SELECT MIN(p) FROM (SELECT 100.0*SUM(rbf_count)/ \
                 NULLIF(SUM(tx_count-1),0) p FROM blocks \
                 WHERE timestamp >= strftime('%s','2023-05-01') \
                 GROUP BY strftime('%Y-%m', datetime(timestamp,'unixepoch')))",
                40.0,
                55.0,
            ),
            (
                "rbf",
                "has run between roughly a half and four fifths (the high)",
                "SELECT MAX(p) FROM (SELECT 100.0*SUM(rbf_count)/ \
                 NULLIF(SUM(tx_count-1),0) p FROM blocks \
                 WHERE timestamp >= strftime('%s','2023-05-01') \
                 GROUP BY strftime('%Y-%m', datetime(timestamp,'unixepoch')))",
                60.0,
                80.0,
            ),
            (
                "rbf",
                "it passed half of all transactions in 2023",
                "SELECT CAST(strftime('%Y', datetime(timestamp,'unixepoch')) \
                 AS REAL) FROM blocks GROUP BY \
                 strftime('%Y-%m', datetime(timestamp,'unixepoch')) \
                 HAVING 100.0*SUM(rbf_count)/NULLIF(SUM(tx_count-1),0) > 50 \
                 ORDER BY MIN(height) LIMIT 1",
                2023.0,
                2023.0,
            ),
            // The copy's live claim, measured the way the sentence measures
            // it: a trailing year rather than the calendar year, because a
            // year-to-date window is nearly empty every January and the guard
            // would fail on the date rather than on the fact.
            //
            // This is the load-bearing one. "The set did not grow at all"
            // follows from the inequality alone: if the unspendable outputs
            // created exceed the whole gap, the corrected change is negative
            // without needing a second measurement. Ratio is 1.03 today.
            (
                "utxo-flow",
                "about two thousand outputs per block above the red line",
                "SELECT AVG(output_count)-AVG(input_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-12 months')",
                1_000.0,
                4_000.0,
            ),
            // A closed year, so these move only if the data is rewritten.
            // Both halves are asserted because the copy's point is that they
            // disagree in sign: guarding one would leave the claim half made.
            (
                "utxo-flow",
                "across 2025 the gap averaged about 450 outputs per block",
                "SELECT AVG(output_count)-AVG(input_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2025-01-01') \
                 AND timestamp < strftime('%s','2026-01-01')",
                430.0,
                460.0,
            ),
            (
                "utxo-flow",
                "the set itself shrank by about 390",
                "SELECT AVG(output_count)-AVG(unspendable_output_count) \
                 -AVG(input_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2025-01-01') \
                 AND timestamp < strftime('%s','2026-01-01')",
                -400.0,
                -380.0,
            ),
            (
                "utxo-flow",
                "a block carrying nothing but a coinbase ... about ninety \
                 thousand times",
                "SELECT COUNT(*) FROM blocks WHERE tx_count=1",
                85_000.0,
                95_000.0,
            ),
            // Regression guard on the 2026-10 sweep, not a copy figure. Until
            // it, coinbase outputs were skipped and a coinbase-only block read
            // output_count=0 about ninety thousand times. If that behaviour
            // ever returns this fails immediately, which the count above would
            // not: it counts blocks, and they exist either way.
            (
                "utxo-flow",
                "Its outputs are counted (coinbase-only blocks with no outputs)",
                "SELECT COUNT(*) FROM blocks WHERE tx_count=1 AND output_count=0",
                0.0,
                0.0,
            ),
            (
                "utxo-flow",
                "Almost all of those create a single output",
                "SELECT COUNT(*) FROM blocks WHERE tx_count=1 AND output_count=1",
                86_000.0,
                90_000.0,
            ),
            (
                "utxo-flow",
                "Eligius did this from 2011 to 2017",
                "SELECT CAST(strftime('%Y', datetime(MAX(timestamp),'unixepoch')) \
                 AS REAL) FROM blocks WHERE coinbase_text LIKE '%Eligius%'",
                2017.0,
                2017.0,
            ),
            (
                "coinbase-payouts",
                "holds the record at 919 outputs in block 133,471",
                "SELECT MAX(coinbase_output_count) FROM blocks",
                919.0,
                919.0,
            ),
            (
                "coinbase-payouts",
                "Before 2016 over nine in ten blocks read 1",
                "SELECT 100.0*SUM(CASE WHEN coinbase_output_count = 1 THEN 1 ELSE 0 END) \
                 /COUNT(*) FROM blocks WHERE timestamp < strftime('%s','2016-01-01')",
                92.0,
                98.0,
            ),
            (
                "coinbase-payouts",
                "median over the last 10,000 blocks is 6",
                "SELECT coinbase_output_count FROM blocks \
                 WHERE height >= (SELECT MAX(height)-9999 FROM blocks) \
                 ORDER BY coinbase_output_count LIMIT 1 OFFSET 5000",
                4.0,
                8.0,
            ),
            (
                "coinbase-payouts",
                "fewer than 1 in 1,000 read 1",
                "SELECT 1000.0*SUM(CASE WHEN coinbase_output_count = 1 THEN 1 ELSE 0 END) \
                 /COUNT(*) FROM blocks WHERE height >= (SELECT MAX(height)-9999 FROM blocks)",
                0.0,
                1.0,
            ),
            (
                "utxo-flow",
                "block 133,471, an Eligius block that paid 919",
                "SELECT coinbase_output_count FROM blocks WHERE height=133471",
                919.0,
                919.0,
            ),
            // Closed history, so this can only move if a backfill rewrites
            // it. Both halves are asserted because the copy's point is the
            // pair: the rise and the fall happened together and neither
            // caused the other.
            (
                "multi-velocity",
                "P2WPKH rose more than 13 points through April 2026",
                "SELECT (SELECT 100.0*SUM(p2wpkh_count)/SUM(p2pkh_count \
                 +p2sh_count+p2wpkh_count+p2wsh_count+p2tr_count \
                 +p2pk_count+multisig_count+unknown_script_count) \
                 FROM blocks WHERE timestamp >= strftime('%s','2026-03-27') \
                 AND timestamp < strftime('%s','2026-04-26')) \
                 - (SELECT 100.0*SUM(p2wpkh_count)/SUM(p2pkh_count \
                 +p2sh_count+p2wpkh_count+p2wsh_count+p2tr_count \
                 +p2pk_count+multisig_count+unknown_script_count) \
                 FROM blocks WHERE timestamp >= strftime('%s','2026-02-25') \
                 AND timestamp < strftime('%s','2026-03-27'))",
                13.0,
                14.5,
            ),
            (
                "multi-velocity",
                "Taproot fell more than eleven points over the same window",
                "SELECT (SELECT 100.0*SUM(p2tr_count)/SUM(p2pkh_count \
                 +p2sh_count+p2wpkh_count+p2wsh_count+p2tr_count \
                 +p2pk_count+multisig_count+unknown_script_count) \
                 FROM blocks WHERE timestamp >= strftime('%s','2026-03-27') \
                 AND timestamp < strftime('%s','2026-04-26')) \
                 - (SELECT 100.0*SUM(p2tr_count)/SUM(p2pkh_count \
                 +p2sh_count+p2wpkh_count+p2wsh_count+p2tr_count \
                 +p2pk_count+multisig_count+unknown_script_count) \
                 FROM blocks WHERE timestamp >= strftime('%s','2026-02-25') \
                 AND timestamp < strftime('%s','2026-03-27'))",
                -13.0,
                -11.0,
            ),
            // Written in words, so `measurement_figures` cannot see them and
            // the unguarded-number gate will not ask for these. Added by hand:
            // a claim nothing re-measures is unguarded whether or not a test
            // happens to notice.
            (
                "multi-velocity",
                "pay-to-anchor outputs did not exist before June 2024",
                "SELECT MIN(height) FROM blocks WHERE p2a_count > 0",
                849_000.0,
                851_000.0,
            ),
            (
                "multi-velocity",
                "have swung by nearly 5 points in a thirty-day window",
                "WITH sh AS (SELECT day, avg_p2a_count*100.0 \
                 /(avg_p2pk_count+avg_p2pkh_count+avg_p2sh_count \
                 +avg_p2wpkh_count+avg_p2wsh_count+avg_p2tr_count \
                 +avg_multisig_count+avg_unknown_script_count) AS p \
                 FROM daily_blocks \
                 WHERE (avg_p2pk_count+avg_p2pkh_count+avg_p2sh_count \
                 +avg_p2wpkh_count+avg_p2wsh_count+avg_p2tr_count \
                 +avg_multisig_count+avg_unknown_script_count) > 0 \
                 AND day >= date('now','-1 year')), \
                 a AS (SELECT day, AVG(p) OVER (ORDER BY day \
                 ROWS BETWEEN 29 PRECEDING AND CURRENT ROW) AS p30 FROM sh), \
                 v AS (SELECT p30 - LAG(p30,30) OVER (ORDER BY day) AS d \
                 FROM a) SELECT MAX(ABS(d)) FROM v",
                4.5,
                6.0,
            ),
            (
                "multi-velocity",
                "now averages about a hundredth of a per cent",
                "SELECT AVG((avg_unknown_script_count-avg_p2a_count)*100.0 \
                 /(avg_p2pk_count+avg_p2pkh_count+avg_p2sh_count \
                 +avg_p2wpkh_count+avg_p2wsh_count+avg_p2tr_count \
                 +avg_multisig_count+avg_unknown_script_count)) \
                 FROM daily_blocks \
                 WHERE (avg_p2pk_count+avg_p2pkh_count+avg_p2sh_count \
                 +avg_p2wpkh_count+avg_p2wsh_count+avg_p2tr_count \
                 +avg_multisig_count+avg_unknown_script_count) > 0 \
                 AND day >= date('now','-1 year')",
                0.0,
                0.05,
            ),
            (
                "multi-velocity",
                "P2PK held virtually every output in January 2009",
                "SELECT 100.0*SUM(p2pk_count)/SUM(p2pkh_count+p2sh_count \
                 +p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count \
                 +multisig_count+unknown_script_count) FROM blocks \
                 WHERE timestamp < strftime('%s','2009-02-01')",
                99.0,
                100.0,
            ),
            // Read 0.05% until the 2026-10 sweep and 11.60% after it, because
            // coinbase outputs are now classified rather than skipped and
            // essentially every P2PK output of this era is a block reward.
            // The old bound would have passed forever on a figure that had
            // stopped describing the chart.
            (
                "multi-velocity",
                "about 12% of them by late 2010",
                "SELECT 100.0*SUM(p2pk_count)/SUM(p2pkh_count+p2sh_count \
                 +p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count \
                 +multisig_count+unknown_script_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2010-11-01') \
                 AND timestamp < strftime('%s','2011-01-01')",
                11.0,
                13.0,
            ),
            // Guards the claim that what remains of P2PK after 2010 is the
            // block reward. Aggregate over the span, not per month: December
            // 2010 alone runs 3.3% out because some coinbases moved to P2PKH
            // early, so a monthly form of this would fail on true copy.
            (
                "multi-velocity",
                "within half a per cent of each other",
                "SELECT ABS(100.0*SUM(coinbase_output_count)/SUM(p2pk_count) - 100.0) \
                 FROM blocks \
                 WHERE timestamp >= strftime('%s','2010-11-01') \
                 AND timestamp < strftime('%s','2011-04-01')",
                0.0,
                0.5,
            ),
            // This one is deliberately the long form, while every row around
            // it is a plain ratio of totals. The claim describes the plotted
            // line, and the chart averages each day's share before
            // differencing, so the two methods answer different questions:
            // 56.6 points smoothed against 23.5 for the same handover read as
            // a ratio of totals. Guarding the cheaper form would pin a number
            // the reader never sees.
            (
                "multi-velocity",
                "one type's loss was the other's gain, by more than 40 points",
                "WITH sh AS (SELECT day, avg_p2pkh_count*100.0 \
                 /(avg_p2pk_count+avg_p2pkh_count+avg_p2sh_count \
                 +avg_p2wpkh_count+avg_p2wsh_count+avg_p2tr_count \
                 +avg_multisig_count+avg_unknown_script_count) AS h \
                 FROM daily_blocks \
                 WHERE (avg_p2pk_count+avg_p2pkh_count+avg_p2sh_count \
                 +avg_p2wpkh_count+avg_p2wsh_count+avg_p2tr_count \
                 +avg_multisig_count+avg_unknown_script_count) > 0), \
                 a AS (SELECT day, AVG(h) OVER (ORDER BY day \
                 ROWS BETWEEN 29 PRECEDING AND CURRENT ROW) AS h30 FROM sh), \
                 v AS (SELECT h30 - LAG(h30,30) OVER (ORDER BY day) AS d \
                 FROM a) SELECT MAX(d) FROM v",
                40.0,
                45.0,
            ),
            (
                "multi-velocity",
                "Taproot going from 22% of outputs to 11%: the 22",
                "SELECT 100.0*SUM(p2tr_count)/SUM(p2pkh_count+p2sh_count \
                 +p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count \
                 +multisig_count+unknown_script_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-02-25') \
                 AND timestamp < strftime('%s','2026-03-27')",
                21.0,
                24.0,
            ),
            (
                "multi-velocity",
                "Taproot going from 22% of outputs to 11%: the 11",
                "SELECT 100.0*SUM(p2tr_count)/SUM(p2pkh_count+p2sh_count \
                 +p2wpkh_count+p2wsh_count+p2tr_count+p2pk_count \
                 +multisig_count+unknown_script_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-03-27') \
                 AND timestamp < strftime('%s','2026-04-26')",
                10.0,
                12.0,
            ),
            // The three causes the copy gives for that move. Each is a ratio
            // of one window's raw count to the previous window's, so they
            // stand independently of the share arithmetic above: the point of
            // the sentence is that Taproot fell for a reason of its own and
            // P2WPKH rose for a different one.
            (
                "multi-velocity",
                "inscriptions dropped by about two thirds",
                "SELECT 1.0*(SELECT SUM(inscription_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-03-27') \
                 AND timestamp < strftime('%s','2026-04-26')) \
                 /(SELECT SUM(inscription_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-02-25') \
                 AND timestamp < strftime('%s','2026-03-27'))",
                0.28,
                0.42,
            ),
            (
                "multi-velocity",
                "which roughly halved Taproot output creation",
                "SELECT 1.0*(SELECT SUM(p2tr_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-03-27') \
                 AND timestamp < strftime('%s','2026-04-26')) \
                 /(SELECT SUM(p2tr_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-02-25') \
                 AND timestamp < strftime('%s','2026-03-27'))",
                0.42,
                0.58,
            ),
            (
                "multi-velocity",
                "P2WPKH's own output count rose by more than a quarter",
                "SELECT 1.0*(SELECT SUM(p2wpkh_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-03-27') \
                 AND timestamp < strftime('%s','2026-04-26')) \
                 /(SELECT SUM(p2wpkh_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','2026-02-25') \
                 AND timestamp < strftime('%s','2026-03-27'))",
                1.25,
                1.40,
            ),
            // The open year, so this is the row here most likely to move: the
            // claim is a floor, and Runes' share of detected OP_RETURN
            // outputs only has to slip below it once for the sentence to be
            // wrong.
            (
                "runes-pct",
                "over 97% so far in 2026",
                "SELECT 100.0*SUM(runes_count)/NULLIF(SUM(op_return_count),0) \
                 FROM blocks WHERE timestamp >= strftime('%s','2026-01-01') \
                 AND timestamp < strftime('%s','2027-01-01')",
                97.0,
                99.5,
            ),
            (
                "witness-share",
                "fewer than 6% of them in a 60% to 70% band",
                "SELECT 100.0*SUM(CASE WHEN witness_bytes*1.0/size \
                 BETWEEN 0.60 AND 0.70 THEN 1 ELSE 0 END)/COUNT(*) \
                 FROM (SELECT witness_bytes, size FROM blocks \
                 ORDER BY height DESC LIMIT 10000) WHERE size > 0",
                0.0,
                6.0,
            ),
            // "6.5% across both 2023 and 2024" is one sentence making two
            // claims, and they are guarded separately because the point of it
            // is that the two years agreed. A single row averaging them would
            // still read 6.5 if one rose and the other fell.
            (
                "fee-revenue-share",
                "6.5% across both 2023 and 2024: the 2023 half",
                "SELECT 100.0*SUM(total_fees)/SUM(total_fees + 625000000) \
                 FROM blocks WHERE timestamp >= strftime('%s','2023-01-01') \
                 AND timestamp < strftime('%s','2024-01-01')",
                6.2,
                6.8,
            ),
            (
                "fee-revenue-share",
                "6.5% across both 2023 and 2024: the 2024 half",
                "SELECT 100.0*SUM(total_fees)/SUM(total_fees + \
                 CASE WHEN height < 840000 THEN 625000000 \
                 ELSE 312500000 END) FROM blocks \
                 WHERE timestamp >= strftime('%s','2024-01-01') \
                 AND timestamp < strftime('%s','2025-01-01')",
                6.2,
                6.8,
            ),
            (
                "fee-revenue-share",
                "1.03% in 2025",
                "SELECT 100.0*SUM(total_fees)/SUM(total_fees + 312500000) \
                 FROM blocks WHERE timestamp >= strftime('%s','2025-01-01') \
                 AND timestamp < strftime('%s','2026-01-01')",
                1.0,
                1.06,
            ),
            (
                "fee-revenue-share",
                "the record is 93.2% (maximum across chain)",
                "SELECT MAX(100.0*total_fees/(total_fees + \
                 CASE WHEN height < 210000 THEN 5000000000 \
                      WHEN height < 420000 THEN 2500000000 \
                      WHEN height < 630000 THEN 1250000000 \
                      WHEN height < 840000 THEN 625000000 \
                      ELSE 312500000 END)) FROM blocks",
                93.15,
                93.25,
            ),
            (
                "fee-revenue-share",
                "the record is 93.2% (argmax height)",
                "SELECT height FROM blocks ORDER BY 100.0*total_fees/(total_fees + \
                 CASE WHEN height < 210000 THEN 5000000000 \
                      WHEN height < 420000 THEN 2500000000 \
                      WHEN height < 630000 THEN 1250000000 \
                      WHEN height < 840000 THEN 625000000 \
                      ELSE 312500000 END) DESC LIMIT 1",
                818_087.0,
                818_087.0,
            ),
            // The per-block arm's dispersion is the point the technical text
            // makes, so the median and the 95th percentile are guarded as a
            // pair. One without the other would let the gap between them
            // close without the copy noticing, and the gap is the claim.
            (
                "tps",
                "over the past year that has averaged about 7",
                "SELECT 1.0*SUM(tx_count)/(COUNT(DISTINCT \
                 date(timestamp,'unixepoch'))*86400.0) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-365 days')",
                5.5,
                8.0,
            ),
            (
                "tps",
                "a single day has run between 3 and 11 (low)",
                "SELECT MIN(t) FROM (SELECT SUM(tx_count)/86400.0 AS t \
                 FROM blocks WHERE timestamp >= strftime('%s','now','-365 days') \
                 GROUP BY date(timestamp,'unixepoch') \
                 HAVING COUNT(*) > 100)",
                2.0,
                4.5,
            ),
            (
                "tps",
                "a single day has run between 3 and 11 (high)",
                "SELECT MAX(t) FROM (SELECT SUM(tx_count)/86400.0 AS t \
                 FROM blocks WHERE timestamp >= strftime('%s','now','-365 days') \
                 GROUP BY date(timestamp,'unixepoch') \
                 HAVING COUNT(*) > 100)",
                9.5,
                13.0,
            ),
            (
                "tps",
                "the median block reads about 7 per second",
                "SELECT r FROM (SELECT b.tx_count*1.0/(b.timestamp-p.timestamp) \
                 AS r FROM blocks b JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.timestamp >= strftime('%s','now','-365 days') \
                 AND b.timestamp > p.timestamp ORDER BY r) \
                 LIMIT 1 OFFSET (SELECT COUNT(*)/2 FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.timestamp >= strftime('%s','now','-365 days') \
                 AND b.timestamp > p.timestamp)",
                6.0,
                8.5,
            ),
            (
                "tps",
                "the 95th percentile over 100",
                "SELECT r FROM (SELECT b.tx_count*1.0/(b.timestamp-p.timestamp) \
                 AS r FROM blocks b JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.timestamp >= strftime('%s','now','-365 days') \
                 AND b.timestamp > p.timestamp ORDER BY r) \
                 LIMIT 1 OFFSET (SELECT COUNT(*)*95/100 FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.timestamp >= strftime('%s','now','-365 days') \
                 AND b.timestamp > p.timestamp)",
                100.0,
                400.0,
            ),
            (
                "tps",
                "block 972,934 carried 7,881 transactions",
                "SELECT tx_count FROM blocks WHERE height = 972934",
                7_881.0,
                7_881.0,
            ),
            // The superlative, which is the part that drifts. The two rows
            // around it pin facts about one block that can never change, so
            // between them they could not notice the record moving, and it
            // moved on 2026-10-24 without anything failing.
            (
                "tps",
                "the highest at 7,881",
                "SELECT b.height FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.timestamp >= strftime('%s','now','-365 days') \
                 AND b.timestamp > p.timestamp \
                 ORDER BY b.tx_count*1.0/(b.timestamp-p.timestamp) DESC \
                 LIMIT 1",
                972_934.0,
                972_934.0,
            ),
            (
                "tps",
                "and was stamped one second after its predecessor",
                "SELECT b.timestamp - p.timestamp FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 WHERE b.height = 972934",
                1.0,
                1.0,
            ),
            (
                "largest-tx",
                "more than 100 blocks have gone above 3.9 MB",
                "SELECT COUNT(*) FROM blocks WHERE largest_tx_size > 3900000",
                100.0,
                250.0,
            ),
            (
                "largest-tx",
                "MARA mined more than nine in ten of them",
                "SELECT 1.0*SUM(miner = 'MARA')/COUNT(*) FROM blocks \
                 WHERE largest_tx_size > 3900000",
                0.85,
                1.0,
            ),
            (
                "largest-tx",
                "the largest in the chain is 3,992,821 bytes (maximum)",
                "SELECT MAX(largest_tx_size) FROM blocks",
                3_992_821.0,
                3_992_821.0,
            ),
            (
                "largest-tx",
                "the largest in the chain is in block 839,842 (argmax)",
                "SELECT height FROM blocks ORDER BY largest_tx_size DESC LIMIT 1",
                839_842.0,
                839_842.0,
            ),
            (
                "largest-tx",
                "they began in February 2023",
                "SELECT MIN(height) FROM blocks WHERE largest_tx_size > 3900000",
                774_628.0,
                774_628.0,
            ),
            (
                "largest-tx",
                "almost all carry an inscription",
                "SELECT 1.0*SUM(inscription_count > 0)/COUNT(*) FROM blocks \
                 WHERE largest_tx_size > 3900000",
                0.90,
                1.0,
            ),
            // Fewer than 10 other transactions beside the coinbase and the
            // large transaction itself means `tx_count <= 11`.
            (
                "largest-tx",
                "45 left room for fewer than 10 other transactions",
                "SELECT COUNT(*) FROM blocks \
                 WHERE largest_tx_size > 3900000 AND tx_count <= 11",
                45.0,
                65.0,
            ),
            // The pair that makes the four-megabyte sentence work: a cap of
            // 4,000,000 weight units only permits a near-4 MB transaction
            // because an ordinary block is about half witness. Both halves
            // are guarded, since either drifting alone breaks the arithmetic
            // the reader is being walked through.
            (
                "largest-tx",
                "a typical block is around half witness",
                "SELECT 100.0*SUM(witness_bytes)/SUM(size) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-365 days')",
                45.0,
                60.0,
            ),
            (
                "largest-tx",
                "a typical block lands near 1.6 MB",
                "SELECT AVG(size) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-365 days')",
                1_450_000.0,
                1_750_000.0,
            ),
            // Added 2026-09-29 after an audit found 35 of the 51 charts that
            // state a figure had no row here at all. These are the ones whose
            // claim is a live measurement rather than a protocol constant, so
            // they are the ones that can rot. Protocol facts (2,016 blocks,
            // 210,000 blocks, four million weight units) are deliberately not
            // listed: they cannot drift, and pinning them would only make this
            // table longer to read.
            (
                "runes-pct",
                "98.4% of detected OP_RETURN outputs over the rest of 2024",
                "SELECT 100.0*SUM(runes_count)/NULLIF(SUM(op_return_count),0) \
                 FROM blocks WHERE height >= 840000 \
                 AND timestamp < strftime('%s','2025-01-01')",
                98.2,
                98.6,
            ),
            (
                "runes-pct",
                "96.9% across calendar 2024 as a whole",
                "SELECT 100.0*SUM(runes_count)/NULLIF(SUM(op_return_count),0) \
                 FROM blocks WHERE strftime('%Y', \
                 datetime(timestamp,'unixepoch'))='2024'",
                96.7,
                97.1,
            ),
            // The dominance is now a claim about the chain rather than about
            // the detector, so it needs watching: if it falls, the sentence
            // "honestly rather than by over-capture" stops being the point.
            (
                "unified-volume",
                "96.9% of OP_RETURN outputs since that height are runestones",
                "SELECT 100.0*SUM(runes_count)/SUM(op_return_count) \
                 FROM blocks WHERE height >= 840000",
                95.0,
                98.5,
            ),
            (
                "unified-volume",
                "About 63% of blocks carry an OP_RETURN output",
                "SELECT 100.0*SUM(op_return_count>0)/COUNT(*) FROM blocks",
                62.5,
                63.5,
            ),
            (
                "unified-volume",
                "over 20% carry a detected inscription",
                "SELECT 100.0*SUM(inscription_count>0)/COUNT(*) FROM blocks",
                20.0,
                24.0,
            ),
            (
                "op-block-share",
                "About 63% of all blocks contain at least one",
                "SELECT 100.0*SUM(op_return_count>0)/COUNT(*) FROM blocks",
                62.5,
                63.5,
            ),
            (
                "unified-volume",
                "about 45 GB of inscription witness bytes chain-wide",
                "SELECT SUM(inscription_envelope_bytes)/1e9 FROM blocks",
                43.0,
                48.0,
            ),
            // Closed historical window (heights 920,000..975,000) so the five
            // size-against-weight comparison figures never drift with the tip.
            (
                "op-block-share",
                "OP_RETURN is 3.94% of size across heights 920,000 to 975,000",
                "SELECT 100.0*(SUM(op_return_bytes)+9*SUM(op_return_count))/SUM(size) \
                 FROM blocks WHERE height >= 920000 AND height < 975000",
                3.92,
                3.96,
            ),
            (
                "op-block-share",
                "but 6.64% of weight across heights 920,000 to 975,000",
                "SELECT 100.0*4*(SUM(op_return_bytes)+9*SUM(op_return_count))/SUM(weight) \
                 FROM blocks WHERE height >= 920000 AND height < 975000",
                6.62,
                6.66,
            ),
            (
                "op-block-share",
                "while inscriptions are 8.09% of size",
                "SELECT 100.0*SUM(inscription_envelope_bytes)/SUM(size) \
                 FROM blocks WHERE height >= 920000 AND height < 975000",
                8.07,
                8.11,
            ),
            (
                "op-block-share",
                "and 3.41% of weight",
                "SELECT 100.0*SUM(inscription_envelope_bytes)/SUM(weight) \
                 FROM blocks WHERE height >= 920000 AND height < 975000",
                3.39,
                3.43,
            ),
            (
                "op-block-share",
                "20.1% of blocks change which of the two is larger",
                "SELECT 100.0*SUM(CASE WHEN ((4.0*(op_return_bytes+9*op_return_count)/weight) > (1.0*inscription_envelope_bytes/weight)) <> ((1.0*(op_return_bytes+9*op_return_count)/size) > (1.0*inscription_envelope_bytes/size)) THEN 1 ELSE 0 END)/COUNT(*) \
                 FROM blocks WHERE height >= 920000 AND height < 975000",
                20.0,
                20.2,
            ),
            (
                "op-block-share",
                "a typical OP_RETURN script is only 20 bytes",
                "SELECT 1.0*SUM(op_return_bytes)/SUM(op_return_count) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-1 year') \
                 AND op_return_count > 0",
                19.0,
                22.0,
            ),
            (
                "op-block-share",
                "understates the cost by about 46%",
                "SELECT 100.0*9*SUM(op_return_count)/SUM(op_return_bytes) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-1 year')",
                43.0,
                49.0,
            ),
            (
                "op-block-share",
                "carry about 5% of all OP_RETURN outputs, capping the shortfall at 0.3%",
                "SELECT 100.0*(SELECT SUM(op_return_count) FROM blocks \
                 WHERE op_return_max_bytes >= 253)/SUM(op_return_count) FROM blocks",
                4.0,
                7.0,
            ),
            (
                "op-block-share",
                "capping the shortfall at 0.3%",
                "SELECT 100.0 * (SELECT SUM(op_return_count)*8 FROM blocks \
                 WHERE op_return_max_bytes>=253) \
                 / SUM(4.0*(op_return_bytes + 9*op_return_count)) FROM blocks",
                0.15,
                0.6,
            ),
            (
                "unified-volume",
                "about 6.7 GB of OP_RETURN script chain-wide",
                "SELECT SUM(op_return_bytes)/1e9 FROM blocks",
                6.5,
                7.0,
            ),
            (
                "time-dist",
                "the median interval is 6.9 minutes",
                "SELECT gap/60.0 FROM (SELECT timestamp - LAG(timestamp) \
                 OVER (ORDER BY height) gap FROM blocks) WHERE gap > 0 \
                 ORDER BY gap LIMIT 1 OFFSET (SELECT COUNT(*)/2 FROM (SELECT \
                 timestamp - LAG(timestamp) OVER (ORDER BY height) g \
                 FROM blocks) WHERE g > 0)",
                6.0,
                7.8,
            ),
            (
                "interval",
                "about one consecutive pair in 60 runs backwards",
                "SELECT 1.0*COUNT(*)/NULLIF(SUM(back),0) FROM (SELECT \
                 (timestamp < LAG(timestamp) OVER (ORDER BY height)) back \
                 FROM blocks) WHERE back IS NOT NULL",
                45.0,
                80.0,
            ),
            (
                "protocol-fees",
                "1,327 blocks where the two protocol totals exceed the block's",
                "SELECT COUNT(*) FROM blocks WHERE \
                 inscription_fees + runes_fees > total_fees AND total_fees > 0",
                1_327.0,
                1_350.0,
            ),
            (
                "protocol-fee-competition",
                "more than 130,000 blocks contain transactions of both kinds",
                "SELECT COUNT(*) FROM blocks \
                 WHERE inscription_count > 0 AND runes_count > 0",
                130_000.0,
                170_000.0,
            ),
            (
                "protocol-fee-competition",
                "in 1,327 blocks the overlap exceeds the block's entire fee take",
                "SELECT COUNT(*) FROM blocks WHERE \
                 inscription_fees + runes_fees > total_fees AND total_fees > 0",
                1_327.0,
                1_350.0,
            ),
            (
                "inscription-envelope",
                "subtracted envelope markers alone account for 7.6% of all matching witness bytes",
                "SELECT 100.0*SUM(inscription_envelope_bytes-inscription_bytes)/ \
                 NULLIF(SUM(inscription_envelope_bytes),0) FROM blocks \
                 WHERE inscription_count > 0",
                7.0,
                8.2,
            ),
            (
                "max-tx-fee",
                "the record is 291.24 BTC (maximum across chain)",
                "SELECT MAX(max_tx_fee)/1e8 FROM blocks",
                291.24,
                291.25,
            ),
            (
                "max-tx-fee",
                "the record is in block 409,008 (argmax height)",
                "SELECT height FROM blocks ORDER BY max_tx_fee DESC LIMIT 1",
                409_008.0,
                409_008.0,
            ),
            (
                "max-tx-fee",
                "Usually tens of thousands of satoshis (1y median)",
                "SELECT max_tx_fee FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-365 days') AND tx_count > 1 \
                 ORDER BY max_tx_fee LIMIT 1 OFFSET (SELECT COUNT(*)/2 FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-365 days') AND tx_count > 1)",
                20_000.0,
                200_000.0,
            ),
            (
                "coinbase-msg-length",
                "the longest readable coinbase message is 94 characters",
                "SELECT MAX(LENGTH(coinbase_text)) FROM blocks",
                94.0,
                94.0,
            ),
            (
                "diff-adjustment",
                "still the fastest epoch in the chain",
                "SELECT d.height FROM blocks d \
                 JOIN blocks p ON p.height = d.height-2016 \
                 WHERE d.height % 2016 = 0 \
                 ORDER BY d.timestamp - p.timestamp ASC LIMIT 1",
                68_544.0,
                68_544.0,
            ),
            (
                "diff-adjustment",
                "at height 68,544 in July 2010, which is why that bar reads exactly 300",
                "SELECT 100.0*(d.difficulty-p.difficulty)/p.difficulty \
                 FROM blocks d JOIN blocks p ON p.height = d.height-2016 \
                 WHERE d.height = 68544",
                299.99,
                300.01,
            ),
            (
                "diff-adjustment",
                "That ceiling has been reached once",
                "SELECT COUNT(*) FROM blocks d \
                 JOIN blocks p ON p.height = d.height-2016 \
                 WHERE d.height % 2016 = 0 AND p.difficulty > 0 \
                 AND d.difficulty/p.difficulty >= 3.999",
                1.0,
                1.0,
            ),
            (
                "diff-adjustment",
                "across more than 480 retargets",
                "SELECT CAST(MAX(height)/2016 AS REAL) FROM blocks",
                480.0,
                520.0,
            ),
            (
                "empty-by-pool",
                "over the past year SpiderPool leads among named pools",
                "SELECT (SELECT COUNT(*) FROM blocks WHERE tx_count = 1 \
                 AND miner = 'SpiderPool' \
                 AND timestamp >= strftime('%s','now','-365 days')) \
                 - (SELECT MAX(c) FROM (SELECT COUNT(*) AS c FROM blocks \
                 WHERE tx_count = 1 AND miner <> 'SpiderPool' AND miner <> 'Unknown' \
                 AND timestamp >= strftime('%s','now','-365 days') GROUP BY miner))",
                1.0,
                200.0,
            ),
            (
                "empty-by-pool",
                "while mining about half as many blocks as AntPool",
                "SELECT 1.0*(SELECT COUNT(*) FROM blocks \
                 WHERE miner = 'SpiderPool' \
                 AND timestamp >= strftime('%s','now','-365 days')) \
                 / (SELECT COUNT(*) FROM blocks WHERE miner = 'AntPool' \
                 AND timestamp >= strftime('%s','now','-365 days'))",
                0.40,
                0.60,
            ),
            (
                "empty-by-pool",
                "Roughly 4% to 6% of blocks arrive within thirty seconds",
                "SELECT 100.0*SUM(CASE WHEN b.timestamp - p.timestamp < 30 \
                 THEN 1 ELSE 0 END)/COUNT(*) FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.timestamp >= strftime('%s','now','-730 days')",
                4.0,
                6.5,
            ),
            (
                "empty-by-pool",
                "about a tenth of SpiderPool's came out empty",
                "SELECT 100.0*SUM(CASE WHEN b.tx_count <= 1 THEN 1 ELSE 0 \
                 END)/COUNT(*) FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.miner = 'SpiderPool' \
                 AND b.timestamp - p.timestamp < 30 \
                 AND b.timestamp >= strftime('%s','now','-730 days')",
                8.0,
                14.0,
            ),
            (
                "empty-by-pool",
                "against about a fortieth of AntPool's",
                "SELECT 100.0*SUM(CASE WHEN b.tx_count <= 1 THEN 1 ELSE 0 \
                 END)/COUNT(*) FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.miner = 'AntPool' \
                 AND b.timestamp - p.timestamp < 30 \
                 AND b.timestamp >= strftime('%s','now','-730 days')",
                1.5,
                4.0,
            ),
            (
                "empty-by-pool",
                "Foundry USA has never mined an empty block",
                "SELECT COUNT(*) FROM blocks \
                 WHERE miner = 'Foundry USA' AND tx_count <= 1",
                0.0,
                0.0,
            ),
            (
                "empty-by-pool",
                "since its first block in December 2020",
                "SELECT CAST(strftime('%Y', datetime(MIN(timestamp),'unixepoch')) AS REAL) \
                 FROM blocks WHERE miner = 'Foundry USA'",
                2020.0,
                2020.0,
            ),
            (
                "all-embedded-share",
                "250,905 blocks between 2016 and 2023",
                "SELECT COUNT(*) FROM blocks WHERE op_return_max_bytes = 83 \
                 AND timestamp >= strftime('%s','2016-01-01') \
                 AND timestamp < strftime('%s','2024-01-01')",
                250_905.0,
                250_905.0,
            ),
            // Core's `-datacarriersize` checks `scriptPubKey.size() > *max_datacarrier_bytes`
            // (policy.cpp:93-96), so the 83-byte ceiling counted the whole
            // scriptPubKey rather than the payload alone. Block 425,742 in
            // 2016 carried an 86-byte OP_RETURN script (nonstandard under
            // default relay policy, included directly by a miner) and is the
            // sole block above 83 bytes across 2016-2023.
            (
                "all-embedded-share",
                "86 bytes at",
                "SELECT MAX(op_return_max_bytes) FROM blocks \
                 WHERE timestamp >= strftime('%s','2016-01-01') \
                 AND timestamp < strftime('%s','2024-01-01')",
                86.0,
                86.0,
            ),
            (
                "all-embedded-share",
                "Exactly 1 block in that span carries more",
                "SELECT COUNT(*) FROM blocks WHERE op_return_max_bytes > 83 \
                 AND timestamp >= strftime('%s','2016-01-01') \
                 AND timestamp < strftime('%s','2024-01-01')",
                1.0,
                1.0,
            ),
            (
                "all-embedded-share",
                "more than half of recent blocks carry a script above the old ceiling",
                "SELECT 100.0*SUM(CASE WHEN op_return_over_80_count > 0 \
                 THEN 1 ELSE 0 END)/COUNT(*) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-90 days')",
                50.0,
                100.0,
            ),
            (
                "all-embedded-share",
                "still about 20 bytes",
                "SELECT 1.0*SUM(op_return_bytes)/ \
                 NULLIF(SUM(op_return_count),0) FROM blocks \
                 WHERE timestamp >= strftime('%s','now','-90 days')",
                15.0,
                25.0,
            ),
            // The dominance claim is the reason the default range looks like
            // one bar, so it is worth watching: if Runes activity falls, the
            // sentence stops describing what the reader sees.
            (
                "unified-count",
                "about 95% of detections over the last 90 days",
                "SELECT 100.0*SUM(runes_count)/SUM(runes_count+omni_count \
                 +counterparty_count+data_carrier_count+inscription_count) \
                 FROM blocks WHERE timestamp >= strftime('%s','now','-90 days')",
                88.0,
                99.0,
            ),
            (
                "empty-blocks",
                "99% of 2009's blocks are empty",
                "SELECT 100.0*SUM(CASE WHEN tx_count = 1 THEN 1 ELSE 0 END)/ \
                 COUNT(*) FROM blocks \
                 WHERE timestamp < strftime('%s','2010-01-01')",
                98.0,
                100.0,
            ),
            // The modern cause, which is the claim worth watching: an empty
            // block is one that arrived before its miner had a template.
            // Both halves are a trailing window, so both can move.
            (
                "empty-blocks",
                "since 2022 arrived under a minute after the block before it on average",
                "SELECT AVG(b.timestamp - p.timestamp) FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.tx_count = 1 \
                 AND b.timestamp >= strftime('%s','2022-01-01')",
                0.0,
                60.0,
            ),
            (
                "empty-blocks",
                "two in three land inside thirty seconds",
                "SELECT 100.0*SUM(CASE WHEN b.timestamp - p.timestamp < 30 \
                 THEN 1 ELSE 0 END)/COUNT(*) FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.tx_count = 1 \
                 AND b.timestamp >= strftime('%s','2022-01-01')",
                60.0,
                80.0,
            ),
            (
                "empty-blocks",
                "about ten minutes for every other block",
                "SELECT AVG(b.timestamp - p.timestamp) FROM blocks b \
                 JOIN blocks p ON p.height = b.height-1 \
                 WHERE b.tx_count > 1 \
                 AND b.timestamp >= strftime('%s','2022-01-01')",
                540.0,
                660.0,
            ),
            (
                "diversity",
                "dropping those blocks scores about 1,300",
                "SELECT SUM(s*s) FROM (SELECT 100.0*COUNT(*)/ \
                 (SELECT COUNT(*) FROM blocks WHERE miner <> 'Unknown') s \
                 FROM blocks WHERE miner <> 'Unknown' \
                 GROUP BY CASE WHEN miner LIKE 'OCEAN%' THEN 'OCEAN' \
                 ELSE miner END)",
                1_150.0,
                1_420.0,
            ),
            (
                "diversity",
                "counting them as one participant scores about 2,900",
                "SELECT SUM(s*s) FROM (SELECT 100.0*COUNT(*)/ \
                 (SELECT COUNT(*) FROM blocks) s FROM blocks GROUP BY miner)",
                2_700.0,
                3_200.0,
            ),
            (
                "diversity",
                "counting each as its own miner scores about 300",
                "SELECT (SELECT SUM(s*s) FROM (SELECT 100.0*COUNT(*)/ \
                 (SELECT COUNT(*) FROM blocks) s FROM blocks \
                 WHERE miner <> 'Unknown' GROUP BY miner)) \
                 + (SELECT COUNT(*) FROM blocks WHERE miner = 'Unknown') \
                 * (100.0/(SELECT COUNT(*) FROM blocks)) \
                 * (100.0/(SELECT COUNT(*) FROM blocks))",
                260.0,
                350.0,
            ),
            // The claim that no block before 2012 carries a tag.
            (
                "diversity",
                "no block before 2012 carries a tag",
                "SELECT COUNT(*) FROM blocks \
                 WHERE miner <> 'Unknown' \
                 AND timestamp < strftime('%s','2012-01-01')",
                0.0,
                0.0,
            ),
            (
                "diversity",
                "half the chain to date is untagged",
                "SELECT 100.0*SUM(CASE WHEN miner = 'Unknown' THEN 1 ELSE 0 \
                 END)/COUNT(*) FROM blocks",
                45.0,
                56.0,
            ),
            (
                "diff-adjustment",
                "the largest fall on record is 27.94% at height 689,472",
                "SELECT 100.0*(d.difficulty-p.difficulty)/p.difficulty \
                 FROM blocks d JOIN blocks p ON p.height = d.height-2016 \
                 WHERE d.height = 689472",
                -28.0,
                -27.9,
            ),
        ];

        let mut failures = Vec::new();
        for (slug, claim, sql, lo, hi) in checks {
            let got: f64 = conn
                .query_row(sql, [], |r| r.get(0))
                .unwrap_or_else(|e| panic!("{slug}: query failed: {e}"));
            // Six significant figures, not two decimal places. Printing
            // `{:.2}` showed a real 0.0015 as "0.00", which is the same
            // quantisation this suite exists to catch, in its own output.
            println!("{slug:22} {got:>16.6}   [{lo} .. {hi}]  {claim}");
            if got < *lo || got > *hi {
                failures.push(format!(
                    "{slug}: copy says \"{claim}\" but the chain now reads \
                     {got:.2}, outside [{lo}, {hi}]"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "copy has drifted from the data:\n  {}",
            failures.join("\n  ")
        );
    }

    /// Every (slug, claim) pair in the drift table, read out of this file.
    ///
    /// Each row opens with a bare `(` on its own line followed by two string
    /// literals, which `cargo fmt` guarantees for a tuple this wide, so the
    /// shape is stable rather than merely current.
    fn guarded_claims() -> Vec<(String, String)> {
        let table = SELF_SRC
            .split_once("let checks: &[(&str, &str, &str, f64, f64)] = &[")
            .expect("the drift table")
            .1
            .split_once("\n        let mut failures")
            .expect("the end of the drift table")
            .0;
        let mut out = Vec::new();
        let lines: Vec<&str> = table.lines().collect();
        let mut rows = 0usize;

        // Comments are skipped rather than counted as positions, because
        // reading the slug and claim at fixed offsets made a row stop being
        // guarded the moment anyone explained it. Two rows were already in
        // that state, and nothing said so: a comment above the claim is the
        // most natural thing to write and silently removed the guard.
        let quoted = |mut n: usize| -> Option<(String, usize)> {
            while lines.get(n)?.trim().starts_with("//")
                || lines.get(n)?.trim().is_empty()
            {
                n += 1;
            }
            // A claim long enough to need Rust's `\`-continuation is still
            // one string, and reading only its first line left the figures
            // in the rest of it unguarded. `witness-pct` and `utxo-flow`
            // were both in that state.
            let mut joined = String::new();
            loop {
                let l = lines.get(n)?.trim();
                match l.strip_suffix('\\') {
                    Some(head) => {
                        joined.push_str(head);
                        n += 1;
                    }
                    None => {
                        joined.push_str(l);
                        break;
                    }
                }
            }
            let inner = joined.strip_prefix('"')?.rsplit_once("\",")?.0;
            Some((inner.to_string(), n))
        };

        for (i, line) in lines.iter().enumerate() {
            if line.trim() != "(" {
                continue;
            }
            rows += 1;
            if let Some((slug, at)) = quoted(i + 1) {
                if let Some((claim, _)) = quoted(at + 1) {
                    out.push((slug, claim));
                }
            }
        }

        // A row the parser cannot read is a row with no guard behind it, and
        // the failure mode is silence: the chart's figures simply look
        // unguarded, or worse, look guarded by the row above.
        assert_eq!(
            out.len(),
            rows,
            "the drift table has {rows} rows and {} parsed; a row's slug or \
             claim is not a plain quoted string on its own line",
            out.len()
        );
        // Every drift row must name a registered chart slug, otherwise
        // `you_cannot_ship_an_unguarded_number` silently ignores the row.
        let invalid_slugs: Vec<&str> = out
            .iter()
            .map(|(slug, _)| slug.as_str())
            .filter(|slug| find(slug).is_none())
            .collect();
        assert!(
            invalid_slugs.is_empty(),
            "drift table rows reference unknown chart slugs: {invalid_slugs:?}"
        );
        out
    }

    /// Figures in a sentence that assert a measurement, as normalised digit
    /// strings (`"63.1%"`, `"190,000"`).
    ///
    /// **Digits only, and that is a deliberate hole.** Our copy also claims in
    /// words: "more than ninety blocks", "about two thirds", "four fifths".
    /// Admitting those means admitting "one transaction" and "at least one
    /// input", which are grammar rather than measurement, and a guard that
    /// cries wolf on 112 occurrences of the word "one" is a guard someone
    /// silences. Digits are where the risk concentrates, because a number
    /// typed to three significant figures is one nobody will re-derive by eye.
    /// Spelled-out claims stay the copy pass's job, and the inventory in
    /// `notes/unguarded-numbers.md` lists them so they are at least visible.
    ///
    /// A figure counts as a measurement when a hedge introduces it or a unit
    /// follows it. Bare integers in prose ("split into 2 groups") assert
    /// nothing about the chain and are skipped.
    /// Figures written in words are invisible here, and deliberately so.
    ///
    /// Only tokens starting with a digit are considered, so "almost four
    /// points" and "about a hundredth of a per cent" pass unseen. That is a
    /// real gap: both of those are claims about the chain.
    ///
    /// It is not worth closing automatically. Measured 2026-10-18 across the
    /// long copy: 28 word-number-plus-unit phrases, and most are ordinary
    /// English rather than measurements, like "more than one transaction" or
    /// "within one block". A rule that flagged them would be mostly noise,
    /// and a noisy gate teaches people to reach for an exemption, which is
    /// how a figure stops being re-read at all.
    ///
    /// So the obligation sits with the author: **a claim written in words
    /// still needs a drift row.** The P2A claims added that day are guarded by
    /// hand for exactly this reason.
    fn measurement_figures(text: &str) -> Vec<String> {
        const HEDGES: &[&str] = &[
            "about", "around", "roughly", "nearly", "almost", "than", "over",
            "least", "fewer", "under", "to", "averages", "averaged", "reads",
            "runs", "ran", "held", "holds", "close",
            // A figure can be introduced by what it does as well as by a
            // hedge. "dropping those blocks scores 1,286" cleared every
            // check: no hedge before it, no unit after, no percent sign.
            "scores", "score", "gives",
        ];
        const UNITS: &[&str] = &[
            "bytes",
            "byte",
            "mb",
            "gb",
            "kb",
            "blocks",
            "block",
            "transactions",
            "transaction",
            "outputs",
            "inputs",
            "seconds",
            "second",
            "minutes",
            "minute",
            "hours",
            "hour",
            "days",
            "day",
            "years",
            "year",
            "times",
            "points",
            "point",
            "btc",
            "sats",
            "cent",
            "per",
        ];
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut out = Vec::new();
        for (i, raw) in words.iter().enumerate() {
            let w = raw.trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '%' && c != '.' && c != ','
            });
            let core = w.trim_end_matches(['.', ',']);
            if core.is_empty()
                || !core.starts_with(|c: char| c.is_ascii_digit())
            {
                continue;
            }
            if !core
                .chars()
                .all(|c| c.is_ascii_digit() || c == ',' || c == '.' || c == '%')
            {
                continue;
            }
            let lower = |n: usize| -> String {
                words
                    .get(n)
                    .map(|s| {
                        s.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                            .to_lowercase()
                    })
                    .unwrap_or_default()
            };
            let hedged = (1..=2)
                .any(|b| i >= b && HEDGES.contains(&lower(i - b).as_str()));
            let united =
                (1..=2).any(|a| UNITS.contains(&lower(i + a).as_str()));
            if core.ends_with('%') || hedged || united {
                out.push(core.to_string());
            }
        }
        out
    }

    /// Protocol and configuration values, which assert nothing about the
    /// chain and so can have no drift row.
    ///
    /// Each is fixed by consensus or by our own code, and the ones set by our
    /// code are already guarded where they are set: the ribbon spans by
    /// `the_ribbon_copy_names_the_spans_the_builders_use`, the per-block
    /// window by `MAX_PER_BLOCK_RANGE`. A value only belongs here if changing
    /// the chain could not change it.
    const PROTOCOL_CONSTANTS: &[&str] = &[
        "4,000,000", // the block weight limit
        "2,016",     // the retarget interval
        "210,000",   // the halving interval
        "3.125",     // the subsidy of the current era
        "86,400",    // seconds in a day
        "1,440",     // minutes in a day
        "600",       // the block target, in seconds
        "144",       // blocks in a day at that target
        "10",        // that target again, in minutes
        "1,000",     // the kB this site denominates every byte unit in
        // Two unrelated constants share this figure, so neither use
        // may delete the entry: the SegWit witness discount, and the
        // floor of a difficulty retarget, which is clamped to a
        // factor of four either way.
        "75%",
        "2009", // the year of the genesis block
    ];

    /// Figures fixed by our own code or used as worked examples, per chart.
    ///
    /// **Per chart rather than global, which is the whole point.** A flat list
    /// holding "10%" would exempt `p2pkh-sunset`'s reference line and, at the
    /// same stroke, a real 10% reading on any of the other 62 charts. The
    /// exemption has to be no wider than the reason for it.
    ///
    /// Two kinds live here. Values our code sets, which a drift row could only
    /// re-measure against the same constant it came from, and which are
    /// guarded where they are set: the ribbon spans by
    /// `the_ribbon_copy_names_the_spans_the_builders_use`. And arithmetic
    /// worked through for the reader, where the numbers are chosen to be easy
    /// rather than true: nobody needs to know whether two pools really hold
    /// 50% each.
    const CONFIGURED_FIGURES: &[(&str, &str)] = &[
        ("address-types", "3.0"), // "3.0 per block over 10 blocks is 30"
        ("address-types", "30"),  // the other half of that same sentence
        ("address-types-pct", "100%"), // the bands are normalised to it
        // Same reason: a 100%-stacked chart's bands are normalised to it,
        // so the figure is the chart's shape and not a chain measurement.
        ("pq-exposure", "100%"),
        ("pq-exposed-value", "100%"),
        ("diff-ribbon", "128"),    // the longest span
        ("fullness-dist", "100%"), // the weight limit, which is the axis
        ("halving-era", "100"),    // each era is indexed against its own peak
        ("multi-velocity", "30"),  // the daily window
        ("size", "4"),             // "4 million weight units"
        ("subsidy-fees", "4"),     // "~4 years", the halving period
        ("weight-util", "4"),      // "4 million weight units" again
        ("diff-ribbon", "7"),      // span, per-block arm
        ("diff-ribbon", "9"),      // span, daily arm
        ("diff-ribbon", "14"),     // span, both arms
        ("diversity", "1%"),       // the HHI worked example
        ("diversity", "100"),      // the same example
        ("diversity", "5,000"),    // and the other half of it
        // The index's own ceiling: one participant at 100% squares
        // to 10,000, so no chain state can move it.
        ("diversity", "10,000"),
        ("diversity", "50%"),   // the same example
        ("diversity", "100%"),  // single-pool monopoly example
        ("fee-heatmap", "80%"), // p10 to p90 spans it by definition
        ("fee-heatmap", "10%"), // p10 and p90 percentile definition
        ("fee-heatmap", "4"),   // minimum transactions for quartiles
        // The old datacarrier relay default. Policy rather than
        // consensus, and no longer the default, but a fixed historical
        // value: no chain state can change what the limit was. What
        // blocks actually carried is guarded by drift rows instead.
        ("all-embedded-share", "80"),
        ("all-embedded-share", "2"), // 2 push opcode bytes in the 83-byte frame
        // The same retired default as it appears on chain: 80 bytes of
        // payload, plus OP_RETURN and the two bytes of OP_PUSHDATA1 that
        // push it. How many blocks stop there is chain state and has its
        // own drift row; that 83 is where they stop is arithmetic.
        ("all-embedded-share", "83"),
        // Same retired relay default, stated on the OP_RETURN-only chart too.
        ("op-block-share", "83"),
        ("op-block-share", "1"), // 1-byte CompactSize length prefix
        ("op-block-share", "253"), // CompactSize 1-byte to 3-byte threshold
        ("fee-spikes", "200"),   // the absolute arm of the spike test
        ("fee-spikes", "300"),   // the minimum range the builder needs
        ("fee-spikes", "6"),     // worked 5x example (1 -> 6 sat/vB)
        ("interval", "1"), // "about 1 consecutive pair in 60" (60 guarded)
        ("interval", "11"), // consensus median-time-past window
        ("interval", "2"), // node 2-hour future timestamp limit
        ("largest-tx", "1"), // witness byte = 1 weight unit
        ("largest-tx", "4"), // non-witness byte = 4 weight units
        ("pq-exposed-value", "0.004"), // worked output value example
        ("segwit", "4"),   // non-witness byte = 4 weight units
        ("size", "1"),     // pre-SegWit 1 MB block limit
        ("taproot-spend-types", "341"), // BIP 341
        ("witness-pct", "100%"), // 100%-stacked bands
        ("witness-versions", "2021"), // Taproot activation year
        ("max-tx-fee", "200"), // `show_ma` minimum block threshold
        ("median-rate", "200"), // `show_ma` minimum block threshold
        ("fullness-dist", "10%"), // the histogram's bucket width
        ("multi-velocity", "15"), // the points-against-percent example
        // Same example. It needed no entry until 2026-10 only because
        // the drift row "by more than 56 points" happened to contain a
        // "5", and coverage here is a substring match. Retuning that
        // row to 40 removed the accident and revealed the gap.
        ("multi-velocity", "5"), // "10 to 15 is 5 points, not 50%"
        // The serialised frame of any transaction output: an 8-byte value
        // field plus a 1-byte CompactSize length prefix. Fixed by the
        // transaction format, so no chain movement can change it.
        ("op-block-share", "9"),
        ("opreturn-bytes", "9"),
        // Consensus coinbase scriptSig bounds (tx_check.cpp:49): 2 to 100 bytes.
        ("coinbase-msg-length", "2"),
        ("coinbase-msg-length", "100"),
        // Standard Taproot script-path reveal prefix: 33-byte x-only key push
        // plus 1-byte OP_CHECKSIG = 34 bytes before the envelope marker.
        ("inscription-envelope", "34"),
        // "distributing to roughly 400 recipients" is an illustration of how
        // to read the axis, not a figure off the chain.
        ("coinbase-payouts", "400"),
        // 919 / 144, the worked example of why the daily mean hides the
        // spikes. The 919 itself is guarded by a drift row.
        ("coinbase-payouts", "6.4"),
        // Post-SegWit coinbase floor (payout + witness commitment). The
        // pre-2016 "read 1" and modern median "6" are guarded by drift rows.
        ("coinbase-payouts", "2"),
        // Eligius ran until 2017. Guarded as a date by the utxo-flow row that
        // measures it; repeated here because this chart states it too.
        ("coinbase-payouts", "2017"),
        // CompactSize: a length prefix is 1 byte below 253 and 3 at or above
        // it. Fixed by the serialisation format.
        ("op-block-share", "3"),
        // The shape of an unbatched payment, not a measurement: one or two
        // inputs, then payment plus change.
        ("batching", "2"),
        // The retarget target in weeks, which is 2,016 blocks at the ten
        // minute spacing. Both halves are consensus.
        ("diff-adjustment", "2"),
        // The pre-SegWit block size limit in MB.
        ("witness-share", "1"),
        ("multi-velocity", "50%"), // arithmetic on 22 and 11, both guarded
        ("p2pkh-sunset", "10%"),   // a drawn reference line
        ("p2pkh-sunset", "5%"),    // the other one
        ("propagation", "60"),     // the chart's own definition of close
        ("rbf", "125"),            // BIP 125, a number but not a quantity
        ("time-dist", "63.2%"),    // what an exponential predicts, not the
        ("time-dist", "5.0%"),     // chain. These three are mathematics and
        ("time-dist", "0.25%"),    // move only if the distribution does.
    ];

    /// Figures in live copy that nothing re-measures yet. Shrink-only: see
    /// `you_cannot_ship_an_unguarded_number`.
    ///
    /// Every one of these was classified by reading the sentence it sits in,
    /// not by its shape. They are real readings of the chain, so each wants a
    /// row in the drift table rather than an exemption.
    const KNOWN_UNGUARDED: &[(&str, &str)] = &[
        ("inscription-envelope", "94%"),
        ("interval", "40"),
        ("multi-velocity", "197"),
        ("multi-velocity", "98"),
        ("time-dist", "0.3%"),
        ("time-dist", "4.5%"),
        ("time-dist", "63.9%"),
    ];

    /// You cannot ship a number the drift test has never heard of.
    ///
    /// `the_copy_still_matches_the_chain` re-measures every figure listed in
    /// its own table, which makes it exactly as good as that table is
    /// complete, and nothing told us when it was not. Its own doc comment
    /// admitted the hole: "a thirteenth hardcoded figure is invisible to it".
    /// This closes that half. It does not check whether a figure is *right*,
    /// only whether anything is watching it.
    ///
    /// **A ratchet, not a cliff.** Landing this as a hard failure over every
    /// chart at once would have meant 40-odd failures on master on day one,
    /// and a test that fails on arrival gets `#[ignore]`d rather than fixed.
    /// `KNOWN_UNGUARDED` is the debt as it stood when this was written. New
    /// figures fail immediately; the listed ones fail only once someone has
    /// guarded them and left the entry behind, which keeps the list honest in
    /// both directions and makes it shrink-only.
    #[test]
    fn you_cannot_ship_an_unguarded_number() {
        let guarded = guarded_claims();
        assert!(
            guarded.len() > 50,
            "only parsed {} drift rows, so the parser has broken and this \
             test would pass by seeing nothing",
            guarded.len()
        );

        let mut found: Vec<(String, String)> = Vec::new();
        for c in CHARTS.iter() {
            let mut fields = vec![c.desc_per_block, c.desc_daily];
            if let Some(a) = c.about {
                fields.push(a.technical);
                if let Some(d) = a.definition {
                    fields.push(d);
                }
            }
            for m in c.measurements {
                fields.push(m.quantity);
                fields.push(m.population);
            }
            for text in fields {
                for fig in measurement_figures(text) {
                    if PROTOCOL_CONSTANTS.contains(&fig.as_str())
                        || CONFIGURED_FIGURES.contains(&(c.slug, fig.as_str()))
                    {
                        continue;
                    }
                    let bare = fig.trim_end_matches('%');
                    // Whole-token match, not a substring. Coverage used to be
                    // `claim.contains(bare)`, under which the figure `5` was
                    // "guarded" by the `5` inside a row labelled "by more than
                    // 56 points": an unrelated number that happened to contain
                    // it. Retuning that row to 40 removed the accident and
                    // exposed the gap, which is the only reason anyone looked.
                    let covered = guarded.iter().any(|(s, claim)| {
                        s == c.slug
                            && claim
                                .split(|ch: char| {
                                    !ch.is_ascii_digit()
                                        && ch != '.'
                                        && ch != ','
                                })
                                // Trailing punctuation belongs to the
                                // sentence, not the figure: a claim ending
                                // "in July 2010," tokenised to "2010," and
                                // never matched the figure "2010", so the
                                // row that guarded it did not count.
                                .any(|tok| tok.trim_matches(['.', ',']) == bare)
                    });
                    if !covered {
                        found.push((c.slug.to_string(), fig));
                    }
                }
            }
        }
        found.sort();
        found.dedup();

        let known: Vec<(String, String)> = KNOWN_UNGUARDED
            .iter()
            .map(|(s, f)| (s.to_string(), f.to_string()))
            .collect();

        let fresh: Vec<&(String, String)> =
            found.iter().filter(|p| !known.contains(p)).collect();
        let fixed: Vec<&(String, String)> =
            known.iter().filter(|p| !found.contains(p)).collect();

        assert!(
            fresh.is_empty(),
            "these figures are in the copy with nothing watching them. Add a \
             row to the drift table in `the_copy_still_matches_the_chain`, or \
             if the value cannot change with the chain, to \
             `PROTOCOL_CONSTANTS`:\n  {}",
            fresh
                .iter()
                .map(|(s, f)| format!("{s}: {f}"))
                .collect::<Vec<_>>()
                .join("\n  ")
        );
        assert!(
            fixed.is_empty(),
            "these are guarded now, so delete them from KNOWN_UNGUARDED. The \
             list only shrinks:\n  {}",
            fixed
                .iter()
                .map(|(s, f)| format!("{s}: {f}"))
                .collect::<Vec<_>>()
                .join("\n  ")
        );
    }

    /// The real catalog splits on `plots_interval_totals`, not just a fixture.
    ///
    /// Asserted over `CHARTS` rather than over a synthetic chart, because the
    /// fact being defended is a property of the declarations: eight of the
    /// nine totalling stacked charts plot a per-block mean per day, and
    /// exactly one multiplies back up to a daily total. A test that built its
    /// own `ChartMeta` would pass while every real chart drifted.
    ///
    /// Per-block resolution must be true for every chart without exception,
    /// since one point is one block there whatever the daily arm does.
    #[test]
    fn only_the_charts_declaring_a_daily_total_may_be_summed_daily() {
        let stacked: Vec<&ChartMeta> = CHARTS
            .iter()
            .filter(|c| {
                matches!(
                    c.shape,
                    Shape::StackedAbsolute | Shape::StackedPercent
                ) && c.unit.totals_over_a_range()
            })
            .collect();
        assert_eq!(
            stacked.len(),
            9,
            "the set of totalling stacked charts changed; re-check which of \
             them may be summed at daily resolution"
        );

        for c in CHARTS.iter() {
            assert!(
                c.plots_interval_totals(false),
                "{}: one point is one block at per-block resolution, so it \
                 is always summable there",
                c.slug
            );
        }

        let summable: Vec<&str> = stacked
            .iter()
            .filter(|c| c.plots_interval_totals(true))
            .map(|c| c.slug)
            .collect();
        assert_eq!(
            summable,
            vec!["address-types"],
            "only a chart declaring Aggregation::DailyTotal may have its \
             daily points added; everything else plots a per-block mean per \
             day, and summing those understated the last year by 143x"
        );
    }

    /// Copy carries no markup, because nothing renders it.
    ///
    /// The About block prints each string as text, so `**bold**` reaches the
    /// page as literal asterisks. One sentence on `taproot-spend-types` did
    /// exactly that, and read as a typo in the middle of the most careful
    /// paragraph on the site. Found by the owner on 2026-09-23.
    ///
    /// Two pieces of structure the renderer does honour, so neither is caught
    /// here. A blank line splits paragraphs. And in an About field,
    /// `[label](block:409,008)` becomes a control opening the block modal,
    /// parsed by `copy_with_links` in the single-chart view.
    ///
    /// A subtitle has no such renderer: it is printed as text on the card and
    /// again on the page, so a link there is literal-asterisk breakage and
    /// stays banned. Targets are checked against what the parser actually
    /// handles, because an unrecognised one falls back to plain text and the
    /// reader sees the raw markers.
    #[test]
    fn copy_carries_no_markup_the_page_cannot_render() {
        const MARKUP: &[(&str, &str)] = &[
            ("**", "bold"),
            ("__", "underline or bold"),
            ("`", "code"),
            ("<em", "raw html"),
            ("<strong", "raw html"),
        ];
        for c in CHARTS.iter() {
            let subtitles = [c.desc_per_block, c.desc_daily];
            let long: Vec<&str> = [
                c.about.and_then(|a| a.definition),
                c.about.map(|a| a.technical),
            ]
            .into_iter()
            .flatten()
            .collect();

            for text in subtitles.iter().chain(long.iter()) {
                for (mark, what) in MARKUP {
                    assert!(
                        !text.contains(mark),
                        "{}: copy contains {mark:?} ({what}), which the \
                         About block prints literally because it renders \
                         text rather than markup",
                        c.slug
                    );
                }
            }

            for text in subtitles {
                assert!(
                    !text.contains("]("),
                    "{}: a subtitle contains a link, and nothing renders one \
                     there",
                    c.slug
                );
            }

            for text in long {
                // Every opening bracket has to belong to a complete marker,
                // or the parser emits it verbatim and the reader sees it.
                assert_eq!(
                    text.matches('[').count(),
                    text.matches("](").count(),
                    "{}: a '[' in the copy does not open a link marker",
                    c.slug
                );
                for part in text.split("](").skip(1) {
                    let target = part
                        .split_once(')')
                        .unwrap_or_else(|| {
                            panic!("{}: a link marker is never closed", c.slug)
                        })
                        .0;
                    let digits =
                        target.strip_prefix("block:").unwrap_or_else(|| {
                            assert!(
                                target.starts_with('/'),
                                "{}: link target {target:?} is neither a \
                                 block height nor a path, and the parser \
                                 renders it as plain text",
                                c.slug
                            );
                            ""
                        });
                    if digits.is_empty() {
                        continue;
                    }
                    let height: u64 = digits
                        .chars()
                        .filter(|ch| ch.is_ascii_digit())
                        .collect::<String>()
                        .parse()
                        .unwrap_or(0);
                    assert!(
                        height > 0
                            && digits
                                .chars()
                                .all(|ch| ch.is_ascii_digit() || ch == ','),
                        "{}: block link target {target:?} is not a height",
                        c.slug
                    );
                }
            }
        }
    }

    #[test]
    fn long_copy_describes_the_metric_and_not_its_edit_history() {
        // Crude on purpose, and "used to" is the one that bites: it also
        // means "employed to", so write "which can" or "for" when that is
        // the sense. A guard is not loosened to fit a sentence.
        const NARRATES_HISTORY: &[&str] = &[
            "earlier version",
            "used to",
            "previously",
            "an older",
            "we changed",
            "no longer",
            "before this",
            "originally",
        ];
        for c in CHARTS.iter().filter(|c| c.about.is_some()) {
            let about = c.about.unwrap();
            for text in about.definition.into_iter().chain([about.technical]) {
                let lower = text.to_lowercase();
                for phrase in NARRATES_HISTORY {
                    assert!(
                        !lower.contains(phrase),
                        "{}'s copy narrates history ({phrase:?}): {text}",
                        c.slug
                    );
                }
            }
        }
    }

    /// Claims this site used to make and will not make again.
    ///
    /// The point of a list rather than a diff: a copy correction is the one
    /// kind of fix nothing defends. A wrong divisor gets a regression test
    /// and a wrong sentence gets rewritten, and the next person to reach for
    /// a confident-sounding line reintroduces it, because the reason it was
    /// wrong lived in a review document nobody reads twice. The review of
    /// 2026-09-16 made exactly that criticism of four declaration fixes that
    /// had no test behind them.
    ///
    /// Each entry carries why it is retired, in the failure message, so the
    /// test teaches rather than just refusing. Searched across the chart copy
    /// **and** the page sources, since a description exists in both.
    ///
    /// This grows as the copy work proceeds. A phrase belongs here once the
    /// claim has been checked against the protocol or the database and found
    /// wrong, not merely reworded for style.
    #[test]
    fn retired_claims_do_not_come_back() {
        const RETIRED: &[(&str, &str)] = &[
            (
                "full node stores all of it",
                "a pruned node verifies every block and keeps a few \
                 gigabytes. What makes a node trustless is having checked \
                 the blocks, not still holding them.",
            ),
            (
                "full node has to store all of it",
                "as above: pruning is not a lesser node.",
            ),
            (
                "Most cluster near the 10-minute target",
                "measured over every interval in the chain: the median is 6.9 \
                 minutes, the shortest bucket is the largest, and 63.9% \
                 arrive in under ten minutes. Mining is memoryless, so the \
                 distribution is exponential and has no peak at the mean.",
            ),
            (
                "simple payments rather than complex contracts",
                "BIP 341 makes a single signer, an aggregated \
                 multisignature and a cooperative contract close \
                 indistinguishable. A key-path spend reveals nothing about \
                 which it was.",
            ),
            (
                "cheapest room in a block",
                "witness bytes are discounted by weight, not by count. A \
                 byte in the witness is one weight unit against four for a \
                 byte in the transaction body.",
            ),
            (
                "clearest view of how full",
                "the consensus limit is 4,000,000 weight units, so raw size \
                 is not the capacity measure. Weight Utilization is.",
            ),
            (
                "price tag on attacking Bitcoin",
                "hash rate here is inferred from difficulty and a \
                 600-second assumption. It is not a cost model, and nothing \
                 here prices an attack.",
            ),
            (
                "should take exactly two weeks",
                "retargeting sets an expectation, not an outcome. Block \
                 discovery is random and hash rate moves, so epochs run \
                 days early or late routinely.",
            ),
            (
                "multiple of the easiest one",
                "inverted. Difficulty is the easiest target divided by the \
                 current one, so a harder target is a smaller number and a \
                 larger difficulty.",
            ),
            (
                "must eventually replace",
                "the subsidy schedule is protocol, the fee outcome is not. \
                 Fee share falls when fees fall faster than the subsidy.",
            ),
            (
                "a handful",
                "about one consecutive pair in 60 is negative, one in \
                 sixty. Measured, not estimated.",
            ),
            (
                "full spread",
                "p10 to p90 is the central 80% of a block's transactions.",
            ),
            (
                "urgent transactions paid",
                "a fee rate does not establish urgency, and p90 is a \
                 percentile rather than a class of transaction.",
            ),
            (
                "Five stacked bands",
                "the percentile lines are not stacked any more, because \
                 quantiles do not add. Stacked, the top boundary was the \
                 sum of five percentiles rather than p90.",
            ),
            (
                "true on-chain footprint",
                "the payload/envelope split is estimated by pattern, with a \
                 10-byte fallback. Measured overhead is 7.7% of matching \
                 witness bytes overall and 17.9% in the median block, \
                 ranging to 94%.",
            ),
            (
                "typically 10-15%",
                "as above: 7.7% by bytes, 17.9% in the median block, 0 to \
                 94% across blocks. One figure cannot stand for that.",
            ),
            (
                "More distributed is healthier",
                "an editorial judgement, and the index behind it excludes \
                 Unknown and renormalises the rest, so it is not a measure \
                 of hash-rate ownership.",
            ),
            (
                "before the miner has received transactions",
                "the data establishes coinbase-only blocks and nothing \
                 about why one was produced.",
            ),
            (
                "RBF Adoption",
                "it counts explicit BIP 125 signalling among confirmed \
                 non-coinbase transactions: not replacements, not fee \
                 bumps, not wallets. Full RBF has been default since Core \
                 28, so no signal does not mean not replaceable.",
            ),
            (
                "Batching Efficiency",
                "no efficiency score is plotted. The chart is Transaction \
                 Batching, and the drawer has to agree with it.",
            ),
            ("over 16 years", "stale by construction. Say since 2009."),
            (
                "less efficient miners are going offline",
                "a smoothed difficulty falling says difficulty fell. Hash \
                 rate leaving and hash rate being unlucky for an epoch \
                 produce the same shape, and neither is observed here.",
            ),
            (
                "indicate high-demand periods",
                "a block being full and its median fee being high are two \
                 coordinates. Fullness is set by what the miner included, \
                 so neither one establishes demand.",
            ),
            (
                "indicate pools that pack additional data",
                "a longer coinbase message is more readable characters. It \
                 identifies neither a pool nor a purpose.",
            ),
            (
                "smaller, more efficient transactions",
                "transactions per byte says they are smaller by bytes. \
                 Efficiency against the block limit is a weight question, \
                 which this does not measure.",
            ),
            (
                "may indicate consolidations",
                "hedging an inference does not make it measurable. The \
                 chart shows a size, not a purpose.",
            ),
            (
                "indicate batching, where exchanges",
                "a high output count is consistent with batching and does \
                 not identify the sender, nor separate a payment from a \
                 change output.",
            ),
            // The three below are rewordings of claims already retired above,
            // written into fresh `definition` copy by the same hand that
            // retired them. That is the limit of a literal-phrase guard worth
            // stating plainly: it holds the sentence, not the inference, so
            // the same claim in new words passes. "A large transaction is
            // usually one consolidating" was in fact *stronger* than the
            // hedged "may indicate consolidations" it replaced. When adding
            // copy, the test to apply is whether the measurement could
            // distinguish the claim from its opposite, not whether the
            // sentence is new.
            (
                "chainstate, the indexes and the undo files enter",
                "Bitcoin Core's `size_on_disk` sums block files and undo \
                 files only (`blockstorage.cpp:792-801`). Neither line on \
                 `chain-size` includes the chainstate or optional indexes \
                 such as `txindex`.",
            ),
            (
                "can only be attributed to one protocol",
                "the fee attribution runs two independent `if`s \
                 (`rpc.rs:1040-1045`), so a transaction bearing both an \
                 inscription and a Runes output has its whole fee counted in \
                 both columns. The methodology page asserted the opposite of \
                 what the code does and of what the chart copy says.",
            ),
            (
                "first known inscription (block 774,628)",
                "the first inscription this node holds is block 767,430, on \
                 2022-12-14, carrying 833 bytes. Block 774,628 is six weeks \
                 later and holds a single 3,938,161-byte inscription, so it \
                 is the near-full block rather than the first. 774,628 is \
                 also the exact number AGENTS.md records as a training-data \
                 hallucination from an earlier largest-block claim, which is \
                 the tell worth remembering.",
            ),
            (
                "suggest an exchange",
                "outputs per transaction counts outputs. No column of this \
                 data carries who signed a transaction, so an exchange \
                 cannot be told from any other sender of many outputs.",
            ),
            (
                "usually one consolidating",
                "a size is not a purpose, and \"usually\" is a claim about \
                 how often, which needs a population this chart does not \
                 measure. Large comes from input count, output count or \
                 witness bytes, and the size alone does not say which.",
            ),
            (
                "means wallets are consolidating",
                "net UTXO change is outputs created less inputs consumed. A \
                 negative figure is consistent with consolidation and also \
                 with payments to fewer outputs; this chart's own population \
                 field already said it does not establish consolidation.",
            ),
            (
                "non-financial",
                "Runes, Omni, Counterparty and BRC-20 are token protocols, \
                 so calling their data non-financial mislabels most of what \
                 these charts count. Say protocol data and inscription \
                 content.",
            ),
            (
                "intentional miner behavior",
                "the data shows a block carried only its coinbase, not why. \
                 78,800 of them are from 2009 and 2010, before pooled \
                 mining, so intent cannot explain the bulk of them.",
            ),
            (
                "60-70% witness data",
                "measured at about 51% over the last 10,000 blocks, with under \
                 6% of them anywhere in that band. The same chart's definition \
                 already said about half.",
            ),
            (
                "this ratio increases",
                "the fee share did not rise after the 2024 halving: 6.5% in \
                 2023 and 2024, 1.03% in 2025, 0.63% in 2026. The subsidy \
                 schedule is fixed and the fee side is not.",
            ),
            (
                "usually mined before the pool",
                "a cause the same chart's long copy refuses, and one that \
                 cannot explain the 78,800 from 2009 and 2010.",
            ),
            (
                "12.2% against 11.11%",
                "those are the numbers from a test fixture, not the chain. \
                 Real recent data gives 6.20% against 5.88%.",
            ),
            (
                "may indicate miner capitulation",
                "seven moving averages crossing says the recent average fell \
                 below the longer one. Nothing here observes a miner \
                 switching off.",
            ),
            (
                "P2PKH declining, P2WPKH flattening, P2TR growing",
                "three lines moving in three directions do not establish \
                 that users migrated between those types, and the windows \
                 differ by resolution: 144 blocks against 30 days.",
            ),
            (
                "averaged from every block in that day",
                "true of the charts that plot a mean and false of every \
                 total, cumulative total and pooled ratio, which is most of \
                 the catalog. The label says how it is computed depends on \
                 the chart now.",
            ),
            (
                "Running total of SegWit transactions",
                "it sums p2wpkh_count plus p2wsh_count, which is native v0 \
                 outputs created. Its own legend was already more accurate \
                 than its subtitle.",
            ),
            (
                "Most Taproot Spends",
                "the stored column counts P2TR outputs created, not inputs \
                 spending them. The two are byte-identical in every \
                 row because one of them is misnamed.",
            ),
            (
                "computed as inputs minus outputs",
                "that is what a fee is, but not what this plots: the block \
                 total comes from the coinbase output value minus the \
                 scheduled subsidy, floored at zero, so a miner who \
                 underclaims makes it read low.",
            ),
            (
                "only outputs that can be spent",
                "it now counts every output including the coinbase's, and \
                 decides spendability by the script rather than by protocol \
                 classification, so it matches the node's own \
                 utxo_increase_actual. Corrected 2026-10-11; it previously \
                 ran two or three per block short.",
            ),
            (
                "potential stale block races",
                "header timestamps are chosen by miners and do not measure \
                 arrival, so a short interval cannot evidence a race. about one in 60 \
                 of them runs backwards.",
            ),
            (
                "indicating sudden demand surges",
                "a median fee rate above its own trailing average does not \
                 identify a cause: a thin block from a short interval looks \
                 the same as a burst of fee-paying transactions.",
            ),
            (
                "Each bar group represents one halving era",
                "the x axis groups the four metrics and there is one series \
                 per era, so the comparison runs across eras within a \
                 metric. Read off fees.rs:890 and after.",
            ),
            (
                "indicates the ecosystem is shifting",
                "a falling share means other output types grew faster. It \
                 does not establish that any wallet or user moved.",
            ),
            (
                "Daily average output types",
                "the daily builder multiplies the stored mean back up by the \
                 block count, so 3.0 per block over 10 blocks is 30. \
                 Pinned by address_types_daily_plots_totals_not_averages.",
            ),
            (
                "'Inputs / Outputs'",
                "the counts exclude the coinbase while the Transactions row \
                 beside them includes it, so the label has to say \
                 non-coinbase.",
            ),
            (
                "Compliant",
                "the BIP-54 check reads coinbase locktime and sequence only. \
                 It is a pattern match, not compliance with the proposal, \
                 and matching does not imply that a miner supports it.",
            ),
            (
                "61 charts",
                "there are 63, and a hardcoded count in a comment goes \
                 stale silently. Say \"every registered chart\" instead.",
            ),
        ];

        let mut copy: Vec<String> = Vec::new();
        for c in CHARTS {
            copy.push(c.desc_per_block.to_string());
            copy.push(c.desc_daily.to_string());
            if let Some(a) = c.about {
                copy.push(a.technical.to_string());
                if let Some(d) = a.definition {
                    copy.push(d.to_string());
                }
            }
            for m in c.measurements {
                copy.push(m.quantity.to_string());
                copy.push(m.population.to_string());
            }
        }
        for (phrase, why) in RETIRED {
            for text in &copy {
                assert!(
                    !text.contains(phrase),
                    "retired claim {phrase:?} is back in chart copy: {why}\n\
                     in: {text}"
                );
            }
            // The pages carry the same descriptions and their own prose, so
            // a phrase can return through either.
            for (i, src) in PAGES.iter().enumerate() {
                assert!(
                    !src.contains(phrase),
                    "retired claim {phrase:?} is back in page {i}: {why}"
                );
            }
            assert!(
                !include_str!("../../routes/observatory/single_chart.rs")
                    .contains(phrase),
                "retired claim {phrase:?} is back in the single-chart view: \
                 {why}"
            );
            assert!(
                !include_str!("../../routes/observatory/mod.rs")
                    .contains(phrase),
                "retired claim {phrase:?} is back in the observatory shell: \
                 {why}"
            );
            // The drawer names charts and the modal labels block fields, so
            // a retired phrase can return through either.
            assert!(
                !include_str!("../../routes/observatory/shared/drawer.rs")
                    .contains(phrase),
                "retired claim {phrase:?} is back in the chart drawer: {why}"
            );
            assert!(
                !include_str!("../../../assets/js/stats.js").contains(phrase),
                "retired claim {phrase:?} is back in the block modal: {why}"
            );
            // The observatory shell's own prose and the Learn articles. A
            // phrase retired from chart copy survived in three of these on
            // 2026-09-16, because nothing searched them: the page `Meta`
            // description, the `seo_text`, and a Learn page.
            assert!(
                !include_str!("../../routes/observatory/learn/protocols.rs")
                    .contains(phrase),
                "retired claim {phrase:?} is back in the protocols article: \
                 {why}"
            );
            // The methodology article states how each number is computed, so
            // a claim retired from the charts surviving here is worse than
            // elsewhere: it reads as the authoritative account of the method.
            assert!(
                !include_str!("../../routes/observatory/learn/methodology.rs")
                    .contains(phrase),
                "retired claim {phrase:?} is back in the methodology \
                 article: {why}"
            );
        }
    }

    /// Every About block is followed by "Measured from my own Bitcoin node",
    /// so copy that says the same thing a sentence earlier is a reader being
    /// told twice in one card.
    ///
    /// Not a ban on mentioning the node. "as my node stores it" distinguishes
    /// the serialised block from other definitions of its size, and chain size
    /// is explicitly anchored to what the node reports on disk. Both earn it.
    /// Bare attribution does not.
    #[test]
    fn long_copy_does_not_repeat_the_footer_attribution() {
        const REDUNDANT: &[&str] =
            &["from my own node", "from my own Bitcoin node"];
        for c in CHARTS.iter().filter(|c| c.about.is_some()) {
            let about = c.about.unwrap();
            for text in about.definition.into_iter().chain([about.technical]) {
                for phrase in REDUNDANT {
                    assert!(
                        !text.contains(phrase),
                        "{} repeats the footer attribution ({phrase:?})",
                        c.slug
                    );
                }
            }
        }
    }

    /// The node is mine, singular, and the copy says so.
    ///
    /// The site is written in one person's voice: "my node", "I would
    /// recommend". A corporate "our node" or an impersonal "this site's own
    /// node" is a different publication speaking, and it slips in easily
    /// because the code's own comments use "we" freely.
    ///
    /// Checks the route sources as well as the registry, since the longest
    /// prose on a chart page is often the page's own, not a `ChartMeta`.
    #[test]
    fn the_node_is_mine() {
        const BANNED: &[&str] = &[
            "our node",
            "our own node",
            "our Bitcoin node",
            "this site's own node",
            "the site's own node",
        ];
        // Route prose is scanned as source; registry prose is scanned field
        // by field, because `SELF_SRC` contains the banned list above and
        // would match itself on every run.
        const SOURCES: &[(&str, &str)] = &[
            (
                "post_quantum.rs",
                include_str!("../../routes/observatory/post_quantum.rs"),
            ),
            (
                "components.rs",
                include_str!("../../routes/observatory/components.rs"),
            ),
        ];
        let mut copy: Vec<(String, String)> = SOURCES
            .iter()
            .map(|(n, s)| (n.to_string(), s.to_string()))
            .collect();
        for c in CHARTS {
            let mut text =
                format!("{} {} {}", c.title, c.desc_per_block, c.desc_daily);
            for m in c.measurements {
                text.push_str(m.quantity);
                text.push(' ');
                text.push_str(m.population);
            }
            if let Some(a) = c.about {
                text.push_str(a.definition.unwrap_or_default());
                text.push_str(a.technical);
            }
            copy.push((c.slug.to_string(), text));
        }
        let mut found = Vec::new();
        for (name, src) in &copy {
            let lower = src.to_lowercase();
            for phrase in BANNED {
                if lower.contains(&phrase.to_lowercase()) {
                    found.push(format!("{name}: {phrase:?}"));
                }
            }
        }
        assert!(
            found.is_empty(),
            "the site speaks as one person, so this is \"my node\": {found:?}"
        );
    }

    /// A comparison **candidate** has to be buildable from the dashboard rows
    /// already loaded for the range, because `build_from_dashboard` is the
    /// only thing that draws the overlaid series and it answers for nothing
    /// else. A chart needing its own fetch would either draw nothing or
    /// describe a different period.
    ///
    /// Being the **primary** asks less: the page builds it the same way it
    /// always does and the comparison is laid over it. So the rule is not
    /// "only dashboard charts can compare" but "only dashboard charts can be
    /// overlaid", and the two came apart on 2026-09-16 when
    /// `diff-adjustment` started reading the retarget blocks. It keeps its
    /// picker, and it is barred as a candidate by `MULTI_METRIC`, which is
    /// what this now checks rather than assuming.
    #[test]
    fn only_dashboard_charts_can_be_overlaid_on_another() {
        for c in CHARTS {
            if matches!(c.source, Source::Dashboard { .. }) || !c.can_compare()
            {
                continue;
            }
            // Not buildable as the overlaid series, so no primary may offer
            // it. Checked against the real validator over every primary
            // rather than against the exclusion lists, so a chart that
            // slipped out of both is caught here.
            let offered_by: Vec<&str> = CHARTS
                .iter()
                .filter(|p| {
                    [false, true]
                        .iter()
                        .any(|&daily| is_valid_comparison(p, c, daily))
                })
                .map(|p| p.slug)
                .collect();
            assert!(
                offered_by.is_empty(),
                "{} needs an input beyond the dashboard rows, so it cannot \
                 be drawn as an overlaid series, but it is offered as a \
                 comparison on {offered_by:?}",
                c.slug
            );
        }
        // And the shape and unit refusals actually bite, rather than the rule
        // collapsing to "every dashboard chart".
        let refused: Vec<&str> = CHARTS
            .iter()
            .filter(|c| {
                matches!(c.source, Source::Dashboard { .. }) && !c.can_compare()
            })
            .map(|c| c.slug)
            .collect();
        assert!(
            !refused.is_empty(),
            "no dashboard chart is refused, so the shape and unit checks are \
             doing nothing"
        );
        for slug in &refused {
            let c = find(slug).unwrap();
            assert!(
                !c.shape.accepts_second_series() || c.unit.occupies_both_axes(),
                "{slug} is refused for no stated reason"
            );
        }
    }

    /// The picker and the resolver have to agree, or a comparison the picker
    /// offered would be refused on selection, or worse, one it never offered
    /// would render because it arrived from a URL.
    ///
    /// This is the invariant that used to be held up by the select not
    /// rendering. Now both sides ask `is_valid_comparison`, and this asserts
    /// they still do.
    #[test]
    fn the_picker_and_the_resolver_agree_on_every_pair() {
        for daily in [false, true] {
            for primary in CHARTS.iter() {
                let offered: std::collections::HashSet<&str> =
                    comparable_with(primary, daily)
                        .into_iter()
                        .flat_map(|(_, m)| m)
                        .map(|c| c.slug)
                        .collect();
                for candidate in CHARTS.iter() {
                    let resolves =
                        comparison_for(primary, candidate.slug, daily)
                            .is_some();
                    assert_eq!(
                        offered.contains(candidate.slug),
                        resolves,
                        "{} over {}: offered={} resolves={} (daily={daily})",
                        candidate.slug,
                        primary.slug,
                        offered.contains(candidate.slug),
                        resolves
                    );
                }
            }
        }
    }

    /// The four ways a stored slug goes stale. Each one arrives by navigating
    /// or by pasting a link, and each has to answer `None` rather than render.
    #[test]
    fn a_stale_comparison_slug_resolves_to_nothing() {
        let primary = CHARTS
            .iter()
            .find(|c| c.can_compare())
            .expect("a comparable chart");

        // Never existed, or was renamed out of the registry.
        assert!(comparison_for(primary, "no-such-chart", false).is_none());
        assert!(comparison_for(primary, "", false).is_none());

        // Itself, which is what navigating to the chart you were comparing
        // with produces.
        assert!(comparison_for(primary, primary.slug, false).is_none());

        // A chart that cannot take part at all.
        let excluded = CHARTS
            .iter()
            .find(|c| !c.can_compare())
            .expect("an excluded chart");
        assert!(comparison_for(primary, excluded.slug, false).is_none());

        // Valid over a short range, nothing to draw over a long one.
        let no_daily = CHARTS
            .iter()
            .find(|c| {
                c.can_compare() && !c.has_daily() && c.slug != primary.slug
            })
            .expect("a comparable chart with no daily builder");
        assert!(comparison_for(primary, no_daily.slug, false).is_some());
        assert!(comparison_for(primary, no_daily.slug, true).is_none());
    }

    /// Landing on a chart that cannot take a comparison must drop one that
    /// arrived with it, rather than carrying it in from the previous chart.
    #[test]
    fn a_primary_that_cannot_compare_resolves_nothing() {
        let excluded = CHARTS
            .iter()
            .find(|c| !c.can_compare())
            .expect("an excluded chart");
        for candidate in CHARTS.iter() {
            assert!(
                comparison_for(excluded, candidate.slug, false).is_none(),
                "{} resolved over a chart that cannot compare",
                candidate.slug
            );
        }
        assert!(comparable_with(excluded, false).is_empty());
    }

    /// Ordering and uniqueness, which the picker owns and the resolver has
    /// no opinion about.
    ///
    /// Everything else the picker must not offer is covered by
    /// `the_picker_and_the_resolver_agree_on_every_pair`, which asserts the
    /// two answer identically for all 63 by 63 pairs at both resolutions.
    #[test]
    fn the_picker_leads_with_the_readers_own_category() {
        let meta = CHARTS
            .iter()
            .find(|c| c.can_compare())
            .expect("at least one comparable chart");
        for daily in [false, true] {
            let groups = comparable_with(meta, daily);
            assert!(!groups.is_empty(), "nothing offered at daily={daily}");
            // A fee metric beside another fee metric is the comparison the
            // reader came for, so it should not be below three other groups.
            assert_eq!(groups[0].0, meta.category);
            let mut seen = std::collections::HashSet::new();
            for (_, members) in &groups {
                for c in members {
                    assert!(seen.insert(c.slug), "{} offered twice", c.slug);
                }
            }
        }
    }

    #[test]
    fn registry_covers_every_chart_on_every_page() {
        let rendered = rendered_charts();
        assert!(
            rendered.len() > 50,
            "failed to parse chart cards out of the page sources, found {}. \
             The parser, not the registry, is probably what broke.",
            rendered.len()
        );

        let mut unregistered: Vec<&str> = rendered
            .iter()
            .filter(|(slug, _)| find(slug).is_none())
            .map(|(slug, _)| slug.as_str())
            .collect();
        unregistered.sort_unstable();
        unregistered.dedup();
        assert!(
            unregistered.is_empty(),
            "these charts render on a page but have no registry entry, so \
             /observatory/chart/<slug> would 404 for them: {unregistered:?}"
        );

        let mut orphaned: Vec<&str> = CHARTS
            .iter()
            .filter(|c| !rendered.iter().any(|(slug, _)| slug == c.slug))
            .map(|c| c.slug)
            .collect();
        orphaned.sort_unstable();
        assert!(
            orphaned.is_empty(),
            "these registry entries render on no page, so the single view \
             would be the only way to reach them: {orphaned:?}"
        );
    }

    #[test]
    fn registry_titles_match_the_pages() {
        let mut drifted = Vec::new();
        for (slug, title) in rendered_charts() {
            if let Some(meta) = find(&slug) {
                if meta.title != title {
                    drifted.push(format!(
                        "{slug}: page {title:?} vs registry {:?}",
                        meta.title
                    ));
                }
            }
        }
        assert!(
            drifted.is_empty(),
            "titles must match, since the registry's becomes the page <title> \
             and the card's is the on-page heading: {drifted:?}"
        );
    }

    /// The single view prints the same one-liner the card does, so a change
    /// on one side has to reach the other. Checks presence and per-block/daily
    /// distinctness rather than re-parsing `chart_desc`'s two arguments, which
    /// would just reimplement the extractor in the assertion.
    #[test]
    fn every_entry_has_usable_descriptions() {
        for c in CHARTS {
            assert!(
                c.desc_per_block.len() > 10,
                "{}: desc_per_block is too short to be a real sentence",
                c.slug
            );
            assert!(
                c.desc_daily.len() > 10,
                "{}: desc_daily is too short to be a real sentence",
                c.slug
            );
            for d in [c.desc_per_block, c.desc_daily] {
                assert!(
                    !d.contains('\u{2014}') && !d.contains('\u{2013}'),
                    "{}: emdash in a description",
                    c.slug
                );
                assert!(
                    src_contains(d),
                    "{}: description {d:?} appears in no page source, so the \
                     card and the single view now say different things",
                    c.slug
                );
            }
        }
    }

    /// True when the literal appears in one of the five page sources.
    fn src_contains(needle: &str) -> bool {
        PAGES.iter().any(|src| src.contains(needle))
    }

    #[test]
    fn slugs_are_unique_and_url_safe() {
        let mut seen = std::collections::HashSet::new();
        for c in CHARTS {
            assert!(seen.insert(c.slug), "duplicate slug {:?}", c.slug);
            assert!(
                !c.slug.is_empty()
                    && c.slug.bytes().all(|b| {
                        b.is_ascii_lowercase()
                            || b.is_ascii_digit()
                            || b == b'-'
                    }),
                "slug {:?} is not URL-safe; it becomes a public path",
                c.slug
            );
            assert!(
                !c.slug.starts_with("chart-"),
                "slug {:?} still carries the chart- prefix",
                c.slug
            );
        }
    }

    /// Titles become `<title>` tags on 61 indexable pages, and the house rule
    /// forbids emdashes in shipped copy.
    #[test]
    fn titles_carry_no_emdash() {
        let bad: Vec<&str> = CHARTS
            .iter()
            .filter(|c| {
                c.title.contains('\u{2014}') || c.title.contains('\u{2013}')
            })
            .map(|c| c.slug)
            .collect();
        assert!(bad.is_empty(), "emdash or endash in titles: {bad:?}");
    }

    /// Every curated neighbour names a chart that exists, and appears.
    ///
    /// `RELATED_ACROSS_CATEGORIES` is a hand-maintained list, which is the
    /// shape that has gone stale twice on this project: the drawer kept a
    /// dangling label after a rename, and the sitemap listed charts that had
    /// been deleted. A slug here that no longer exists would silently drop
    /// the neighbour rather than fail.
    #[test]
    fn curated_related_names_charts_that_exist() {
        for (from, to) in RELATED_ACROSS_CATEGORIES {
            let origin = CHARTS.iter().find(|c| c.slug == *from);
            assert!(
                origin.is_some(),
                "{from} is not a registered chart, so its curated \
                 neighbours are unreachable"
            );
            for slug in *to {
                assert!(
                    CHARTS.iter().any(|c| c.slug == *slug),
                    "{from} points at {slug}, which is not a registered chart"
                );
                assert_ne!(
                    slug, from,
                    "{from} lists itself as a related chart"
                );
            }
            // And the curation actually reaches the reader: these exist
            // because the derived answer omitted them.
            let shown: Vec<&str> =
                related(origin.unwrap(), 4).iter().map(|c| c.slug).collect();
            for slug in *to {
                assert!(
                    shown.contains(slug),
                    "{from} curates {slug} but related() returned {shown:?}; \
                     the curated entries must survive the max"
                );
            }
        }
    }

    #[test]
    fn related_never_returns_self_and_respects_max() {
        for c in CHARTS {
            let r = related(c, 4);
            assert!(r.len() <= 4);
            assert!(
                !r.iter().any(|o| o.slug == c.slug),
                "{} lists itself as related",
                c.slug
            );
        }
    }

    /// A donut or histogram has no time axis, so it must not also claim to
    /// accept a second series laid over one.
    #[test]
    fn shape_capabilities_are_consistent() {
        for c in CHARTS {
            if !c.shape.has_time_axis() {
                assert!(
                    !c.shape.accepts_second_series(),
                    "{} has no time axis but accepts a second series",
                    c.slug
                );
            }
        }
    }
}
