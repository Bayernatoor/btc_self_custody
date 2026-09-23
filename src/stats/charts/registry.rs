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
//! the four page sources and fails if either side holds an entry the other
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
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::Network => "Network",
            Self::Fees => "Fees",
            Self::Mining => "Mining",
            Self::Embedded => "Embedded Data",
        }
    }

    pub fn page_path(self) -> &'static str {
        match self {
            Self::Network => "/observatory/charts/network",
            Self::Fees => "/observatory/charts/fees",
            Self::Mining => "/observatory/charts/mining",
            Self::Embedded => "/observatory/charts/embedded",
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
    /// - **percentages**, which are already bounded and gain nothing
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
        desc_per_block: "Share of each block's bytes in detected protocol data: OP_RETURN script bytes plus estimated inscription payload",
        desc_daily: "Daily average share of block bytes in detected protocol data",
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
                quantity: "Share of block bytes in OP_RETURN scripts",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "OP_RETURN script bytes over serialized block bytes. Script bytes only, so the output's value and length fields are outside the numerator.",
            },
            Measurement {
                series: "Inscriptions",
                series_daily: "",
                quantity: "Share of block bytes in inscription payloads",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Estimated inscription payload over serialized block bytes. The two bands measure bytes on different bases, since one is exact script bytes and the other an estimated payload, so their sum is not a single well-defined quantity.",
            },
        ],
        about: Some(About {
            definition: Some("How much of a block is data rather than payments. Bitcoin has two places to put arbitrary bytes: an OP_RETURN output, which is a small amount of provably unspendable script, and the witness of a Taproot input, which is where inscriptions live. This is both, as a share of the block."),
            technical: "Combines OP_RETURN data (in outputs) and inscription data (in witness) as a percentage of total block size. These are disjoint categories that together represent all classified embedded data.",
        }),
    },
    ChartMeta {
        slug: "coinbase-msg-length",
        title: "Coinbase Message Length",
        desc_per_block: "Length of decoded ASCII text found in each block's coinbase transaction. Mining pools embed identifiers, timestamps, and occasionally custom messages in this space",
        desc_daily: "Length of decoded ASCII text found in each block's coinbase transaction. Mining pools embed identifiers, timestamps, and occasionally custom messages in this space",
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
                quantity: "Printable characters in the block's coinbase input",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Decoded ASCII run found in the coinbase scriptSig. No daily builder: the daily table stores no coinbase text, so a daily point would have nothing behind it.",
            },
        ],
        about: Some(About {
            definition: Some("Every block's first transaction has a field the miner fills in freely, which is where Satoshi left the Times headline in the genesis block. Most of what goes there now is a pool name and some binary. This is how many readable characters each one holds: the longest in the chain is 94."),
            technical: "Every block's coinbase transaction contains a scriptSig with arbitrary data. Miners use this to embed their pool identifier, block height (required since BIP-34), and sometimes custom messages or political statements. A longer message means more readable characters, not a particular pool or purpose.",
        }),
    },
    ChartMeta {
        slug: "inscription-envelope",
        title: "Inscription Payload vs Envelope",
        desc_per_block: "Detected inscription witness bytes split into an estimated payload and the envelope structure around it",
        desc_daily: "Daily average inscription payload vs envelope overhead per block",
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
                quantity: "Estimated inscription content bytes",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Matching witness item bytes less an estimated envelope overhead, so this is an estimate of content rather than a parsed payload. Denominated in kB of 1,000 bytes, matching every other byte unit on the site.",
            },
            Measurement {
                series: "Envelope Overhead",
                series_daily: "",
                quantity: "Estimated inscription framing bytes",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "The remainder after subtracting the estimated payload, floored at zero.",
            },
        ],
        about: Some(About {
            definition: Some("An inscription is content wrapped in a small protocol structure: markers saying what follows, the content type, and a terminator. This splits the two, so the cost of the wrapper is visible next to the thing being stored. Small inscriptions pay proportionally far more wrapper."),
            technical: "Every Ordinals inscription wraps content in a witness envelope: OP_FALSE OP_IF ... OP_ENDIF with push opcodes and the 'ord' marker. The split is estimated, not parsed: the envelope header, content-type section and terminator are located by pattern and subtracted, and anything not found falls back to 10 bytes. Measured across the more than 190,000 blocks holding a detection, that overhead is 7.7% of all matching witness bytes, while the median block puts it at 17.9% and individual blocks run from nothing to 94%. The share is high where inscriptions are small, since the envelope cost is close to fixed, which is why BRC-20 JSON operations sit at the top of that range.",
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
            population: "Detected inscription fees over total fees. Both terms are estimates: the numerator is a detector match and the denominator is coinbase-derived.",
        }],
        about: Some(About {
            definition: Some("How much of the fee market inscriptions were paying for. Everyone bids for the same block space, so this is one way of asking how much of the competition at a given moment came from data rather than payments."),
            technical: "Tracks how much of the block's fee revenue comes from transactions containing Ordinals inscriptions. During high-demand periods like BRC-20 launches, inscription fees can spike significantly as inscribers compete for block space.",
        }),
    },
    ChartMeta {
        slug: "inscription-share",
        title: "Inscription Block Share",
        desc_per_block: "Total inscription witness data (payload + envelope overhead) as a percentage of each block",
        desc_daily: "Daily average inscription data as a percentage of block size",
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
            quantity: "Share of block bytes in detected inscription envelopes",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Envelope bytes of matching witness items over serialized block bytes, so a byte fraction and not a count of anything. Falls back to payload bytes for blocks ingested before envelope sizes were stored, which makes the early series slightly lower than the late one on the same activity.",
        }],
        about: Some(About {
            definition: Some("How much of a block is inscription data. Inscriptions go in the witness, which the protocol charges a quarter rate for, so they can fill a block by bytes while leaving room by weight. Two blocks in the chain are 99.99% inscription by bytes: 892,279 and 899,687."),
            technical: "Includes both the inscription content (images, text, JSON) and the witness envelope structure (OP_FALSE OP_IF, push opcodes, 'ord' marker). This is the whole matching witness item, payload and envelope together, as the detector found it. Witness bytes count one weight unit each instead of four, so an inscription consumes a quarter of the block weight its byte count suggests.",
        }),
    },
    ChartMeta {
        slug: "inscriptions",
        title: "Ordinals Inscriptions",
        desc_per_block: "Inscriptions per block: images, text, and other data stored in witness data",
        desc_daily: "Daily average inscriptions per block",
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
            definition: Some("Inscriptions attach data such as an image or text to an individual satoshi, using the Ordinals convention introduced in 2023. The data rides in the witness part of a Taproot transaction, where each byte counts as one weight unit against the block limit instead of four, so it costs a quarter of what the same byte costs in the transaction body."),
            technical: "Counted by matching the inscription envelope pattern in Taproot witness data. This is a convention, not a consensus rule: nothing in the protocol knows what an inscription is, so a different encoding would not appear here. Inscriptions compete for the same block space as ordinary payments, which shows up in the fee charts over the same periods.",
        }),
    },
    ChartMeta {
        slug: "op-block-share",
        title: "OP_RETURN Block Share",
        desc_per_block: "OP_RETURN data as a percentage of each block's size",
        desc_daily: "Daily average OP_RETURN data as a percentage of block size",
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
            quantity: "Share of block bytes in OP_RETURN scripts",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "OP_RETURN scriptPubKey bytes over serialized block bytes. Script bytes only: the output's value and length fields are not included, so this understates the space those outputs occupy.",
        }],
        about: Some(About {
            definition: Some("How much of a block is OP_RETURN data. An OP_RETURN output is a deliberately unspendable output whose script carries a few dozen bytes, which can timestamp or label things on chain. 63.1% of all blocks contain at least one."),
            technical: "OP_RETURN script bytes over serialized block bytes. Script bytes only, so the output's value and length fields sit outside the numerator, which makes this a floor on the true on-chain cost rather than the whole of it.",
        }),
    },
    ChartMeta {
        slug: "opreturn-bytes",
        title: "OP_RETURN Volume",
        desc_per_block: "Bytes of data stored in OP_RETURN outputs per block by protocol",
        desc_daily: "Daily average OP_RETURN bytes per block by protocol",
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
            definition: Some("How many bytes of OP_RETURN data each block carried, split by the protocol that put them there. Runes, Omni and Counterparty are token systems that write their state into these outputs; anything unrecognised falls into the fourth band."),
            technical: "Byte counts include the full scriptPubKey: the OP_RETURN opcode, push opcodes, and the protocol payload. This is the actual on-chain storage footprint of each OP_RETURN output.",
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
            definition: Some("How many OP_RETURN outputs each block carried, split by the protocol that put them there. An output rather than a byte count, so a protocol writing many small records shows up here and a protocol writing few large ones shows up on the volume chart."),
            technical: "Detected by matching the leading bytes of each OP_RETURN script against known protocol prefixes. Unrecognised scripts fall into the fourth band, so it is a residual and not a category.",
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
                population: "A transaction matching both this and the Runes detector has its whole fee credited to both, so these bands can double-count and are not parts of one total despite being stacked.",
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
            definition: Some("Which kind of data-carrying transaction was paying more into the fee market at a given time. Two detectors side by side, because a transaction can match both and is then counted by both."),
            technical: "Fees paid by the transactions each detector matched, drawn as two totals beside the block's whole fee take rather than as parts of it. A transaction carrying both an inscription and a Runes payload is credited to both, so the two cannot be added: more than 125,000 blocks have both detectors firing and in 1,326 of them the sum exceeds the block's entire fee take. Which detector is larger in a period is a fact about the detectors, not about what drove the fee market.",
        }),
    },
    ChartMeta {
        slug: "runes-pct",
        title: "OP_RETURN Protocol Share",
        desc_per_block: "Detected OP_RETURN outputs by protocol. Runes were 97.0% of them in 2024, the year they launched, and over 97% so far in 2026",
        desc_daily: "Detected OP_RETURN outputs by protocol. Runes were 97.0% of them in 2024, the year they launched, and over 97% so far in 2026",
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
            population: "Each detector's count over the block's OP_RETURN count. Detectors match a prefix, so the residual band is whatever matched none of them rather than a named protocol.",
        }],
        about: Some(About {
            definition: Some("Which protocol is using the OP_RETURN space. Runes launched in the halving block, 840,000 on 2024-04-20, and took almost all of it immediately: 97.0% of detected OP_RETURN outputs that year."),
            technical: "Each protocol's detected OP_RETURN outputs as a share of all detected OP_RETURN outputs in the block. Detection is by script prefix, so an unrecognised protocol lands in the residual band rather than being missed entirely.",
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
            definition: Some("Everything the site detects as embedded data, in one place. The first four bands are OP_RETURN outputs and the last two are inscription-bearing witness items, which are different kinds of object: one is an output in the transaction body, the other is an item in a Taproot witness."),
            technical: "Stacked count of embedded data items by protocol. BRC-20 is a subset of Inscriptions (do not add them). Runes, Omni, and Counterparty are mutually exclusive subsets of OP_RETURN. See the Methodology page for the full taxonomy.",
        }),
    },
    ChartMeta {
        slug: "unified-volume",
        title: "Detected Embedded Data, Volume",
        desc_per_block: "Bytes per block by protocol: exact OP_RETURN script bytes and estimated inscription payload, on one stack",
        desc_daily: "Daily average bytes per block by protocol, mixing exact OP_RETURN script bytes with estimated inscription payload",
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
                // charts had one label for two different scopes. There is no
                // `brc20_bytes` column, so this band cannot be split the
                // same way without an ingestion change and a backfill; see
                // `notes/chart-quality-2026-09-15/runs/backfill-queue.md`.
                series: "Inscriptions incl. BRC-20",
                series_daily: "",
                quantity: "Estimated inscription payload bytes, BRC-20 included",
                method_per_block: Method::HeuristicallyDetected,
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "An estimated payload, whereas the OP_RETURN bands are exact script bytes, so the stack mixes two byte bases: about 6.3 GB of exact script bytes against about 41 GB of estimate over the whole chain. BRC-20 content is inside this band because no separate byte column exists for it, unlike the count chart where the two are disjoint.",
            },
        ],
        about: Some(About {
            definition: Some("Everything the site detects as embedded data, by size rather than by count. 63.1% of blocks carry an OP_RETURN output and 19.7% carry a detected inscription, and the two are measured on different bases, which the bands say."),
            technical: "OP_RETURN bands are exact script bytes. The inscription band is an estimated payload, and it includes BRC-20 content because no separate byte column exists for it, unlike the count chart where the two are disjoint. Over the whole chain that is about 6.3 GB of exact bytes against about 41 GB of estimate, so the stack is mostly estimate.",
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
            population: "Block fees over the block's transactions less one for the coinbase. A block or day with no user transaction reports no reading rather than zero. The fee total is itself coinbase-derived, so this inherits that estimate.",
        }],
        about: Some(About {
            definition: Some("What one transaction cost, on average, to get into a block. Fees are paid for space rather than for the amount being moved, so a transaction sending a fortune can pay less than one sending pennies if it takes up less room."),
            technical: "The block's total fees divided by its transactions, less one for the coinbase, which pays nothing. The total is itself taken from the coinbase output rather than summed per transaction, so a miner who underclaims makes this read low. A block with no user transaction reports nothing rather than zero.",
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
            technical: "Total value of all non-coinbase outputs. This includes both the payment and the change output, so it overstates actual economic activity. Still useful for relative comparisons across time periods.",
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
            definition: Some("What different transactions in the same block paid, rather than one number for the block. Each line is a percentile: p10 is the rate a tenth of that block's transactions came in under, the median is the middle one, and p90 is the rate a tenth paid more than. The gap between them is how unequal the block was."),
            technical: "Five percentile lines, each read directly off the block: p10 is the rate a tenth of that block's transactions paid less than, the median is the middle one, and p90 is the rate a tenth paid more than. Not stacked, because percentiles do not add: stacking them made the top boundary the sum of five quantiles rather than p90. The gap between p10 and p90 is the central 80% and says nothing about why any transaction paid what it did. Click legend items to isolate lines.",
        }),
    },
    ChartMeta {
        slug: "fee-pressure",
        title: "Fee Pressure vs Block Space",
        desc_per_block: "Each block placed by how full it was and what its middle transaction paid",
        desc_daily: "Each block placed by how full it was and what its middle transaction paid",
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
            definition: Some("Each dot is one block, placed by how full it was and what its middle transaction paid. Two things a reader might expect to move together, plotted against each other so it is visible when they do not."),
            technical: "Each dot is one block. X-axis shows how full the block is (weight utilization %), Y-axis shows the median fee rate. Top-right is a full block whose median fee rate was also high; bottom-right is a full block at a low rate, which is most of them; top-left is a high rate in a block that was not full. The pair of coordinates is what the chart shows. It does not establish why either number was what it was, and a block's fullness is set by what the miner included rather than by demand alone.",
        }),
    },
    ChartMeta {
        slug: "fee-revenue-share",
        title: "Fee Revenue Share",
        desc_per_block: "Percentage of total block reward that comes from fees rather than subsidy",
        desc_daily: "Daily average fee revenue as a percentage of total block reward",
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
            population: "Fees over subsidy plus fees. The subsidy comes from the block height schedule and the fees are coinbase-derived, so the share inherits that estimate. A falling share can mean falling fees rather than a growing subsidy.",
        }],
        about: Some(About {
            definition: Some("How much of a miner's income comes from fees rather than from new coins. The subsidy halves on a schedule and fees do not, so this is the ratio people watch when they ask what pays for mining once the subsidy is small. It has been volatile rather than trending: 6.5% across both 2023 and 2024, 1.03% in 2025, 0.63% so far in 2026."),
            technical: "Fees over subsidy plus fees, per block. The subsidy schedule is fixed and the fee side is not, so this does not rise with each halving: it was 6.5% across both 2023 and 2024, then fell to 1.03% in 2025 and 0.63% in 2026. Nor is the usual level a few per cent: more than half of all blocks are under 1%, and the median block in recent months is around a half of one per cent. The record is 93.2%, in a block that collected far more in fees than it was paid to produce.",
        }),
    },
    ChartMeta {
        slug: "fee-spikes",
        title: "Fee Spike Detector",
        desc_per_block: "Highlights blocks where the median fee rate exceeded 5x the trailing 144-block average. Red dots mark fee spike events",
        desc_daily: "Highlights blocks where the median fee rate exceeded 5x the trailing 144-block average. Red dots mark fee spike events",
        category: Category::Fees,
        unit: Unit::SatVb,
        shape: Shape::LineWithScatter,
        source: Source::Dashboard {
            per_block: super::fee_spike_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Spike (>5x avg)",
                series_daily: "",
                quantity: "Blocks whose median fee rate exceeded five times the trailing 144-block mean",
                method_per_block: Method::Calculated,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Needs at least 300 blocks of context to establish the trailing mean, so short ranges draw nothing. The accompanying moving average is a companion, not a second measurement. No daily builder.",
            },
        ],
        about: Some(About {
            definition: Some("Blocks whose middle transaction paid far more than the blocks around it did. Fee rates move in bursts rather than drifting, because everyone is bidding for the same space at the same time, so a single block can settle several times higher than the day it sits in. This marks those blocks: a way of finding the moments worth a second look, not an explanation of them."),
            technical: "The line is the 144-block trailing average of the median fee rate, which is about a day of blocks. A dot marks a block whose own median fee rate was more than five times that average. What produced the jump is not in this data: a burst of fee-paying transactions and a thin block from an unlucky interval both show up the same way. Needs at least 300 blocks, so a 1W or longer range.",
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
            population: "Every block in the window. The denomination follows the BTC/sats toggle, so the active unit is not fixed by this entry.",
        }],
        about: Some(About {
            definition: Some("What everyone paid, in total, to get into a given block. Each transaction pays a fee to be included, and the miner keeps every fee in the block they find. It is one half of what a miner earns. The other is the subsidy, which halves every four years and eventually reaches zero."),
            technical: "A transaction does not state its fee anywhere: a fee is what its inputs exceed its outputs by. This chart does not sum those, though. The block total is taken from the coinbase transaction, as the value the miner paid itself minus the subsidy the schedule allows, floored at zero. The two agree whenever a miner claims everything available, and a miner who underclaims makes this read low. Shown in BTC or satoshis by the toggle above, never in dollars of the day, so the figure is comparable across the whole chain.",
        }),
    },
    ChartMeta {
        slug: "halving-era",
        title: "Halving Era Comparison",
        desc_per_block: "Four block metrics compared across the halving eras present in the selected range, each normalised to its own highest era",
        desc_daily: "Four block metrics compared across the halving eras present in the selected range, each normalised to its own highest era",
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
            population: "Blocks grouped by subsidy era, drawn with **the four metrics along the x axis and one series per era**, then **each metric normalised so its largest era reads 100**, which is why something always peaks at 100 and why the plotted numbers are an index rather than a quantity. The four metrics do not share a provenance: block size and transaction count are measured, while total fees and the fee share of miner revenue are both coinbase-derived estimates. Only eras present in the window appear.",
        }],
        about: Some(About {
            definition: Some("Four things about an average block, compared across the subsidy eras. Every 210,000 blocks the subsidy halves, splitting the chain into periods of roughly four years, and this asks how a block in one period differs from a block in another. Each metric is scaled to its own highest era, so the bars show relative change rather than quantities."),
            technical: "Each group on the x axis is one metric, with a bar per halving era, so the comparison runs across eras within a metric rather than across metrics within an era. Every metric is divided by its own highest era, which is why something always reads 100 and why the plotted numbers are an index rather than a quantity. Only eras present in the selected range appear. Hover for the underlying values; two of the four are coinbase-derived estimates rather than measured.",
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
            definition: Some("The most any single transaction paid in each block. Usually a few thousand satoshis, occasionally something extraordinary: the record is 291.24 BTC in block 409,008, which is a mistake rather than a bid, and the kind of thing this chart exists to surface."),
            technical: "The largest single fee in the block, in BTC. Read per transaction during ingestion, so it is not derived from the block total. One outlier does not move any other series here, which is why this sits beside the median rather than replacing it.",
        }),
    },
    ChartMeta {
        slug: "median-rate",
        title: "Median Fee Rate",
        desc_per_block: "Median fee rate across all transactions in each block",
        desc_daily: "Median fee rate (per-block ranges only for daily)",
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
                population: "Each block contributes its own median fee rate, and the daily point averages those. A mean of medians is not the median of the day's transactions, and the two differ whenever blocks hold different numbers of transactions.",
            },
        ],
        about: Some(About {
            definition: Some("How much a transaction paid per unit of size to get into a block, taking the middle transaction. Fees are charged by the room a transaction takes, not the value it moves, so sending a large amount can cost less than sending a small one."),
            technical: "Fee divided by virtual size for every transaction in the block, then the middle value. Virtual size is weight divided by four, which is the unit the fee market prices in. The median rather than the mean, because it is less sensitive to a few unusually high fees. The coinbase transaction pays no fee and is excluded.",
        }),
    },
    ChartMeta {
        slug: "protocol-fees",
        title: "Protocol Fee Revenue",
        desc_per_block: "Fees paid by transactions matching the inscription and Runes detectors. A transaction matching both is counted in both, so the two do not sum to a share of block fees",
        desc_daily: "Fees paid by transactions matching the inscription and Runes detectors. A transaction matching both is counted in both, so the two do not sum to a share of block fees",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::protocol_fee_breakdown_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Inscriptions",
                series_daily: "",
                quantity: "Fees paid by transactions matching the inscription detector",
                method_per_block: Method::HeuristicallyDetected,
                // No daily builder, so there is no daily method either.
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Detector match, not a parse. A transaction matching both the inscription and Runes detectors has its whole fee credited to both, so these categories can overlap and are not a partition of the block's fees despite being stacked.",
            },
            Measurement {
                series: "Runes",
                series_daily: "",
                quantity: "Fees paid by transactions matching the Runes detector",
                method_per_block: Method::HeuristicallyDetected,
                // No daily builder, so there is no daily method either.
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As Inscriptions, and can double-count the same transaction's fee.",
            },
        ],
        about: Some(About {
            definition: Some("How much of a block's fee income came from transactions carrying inscription or Runes data. Two detectors drawn side by side rather than as parts of one total, because a transaction carrying both is counted by both."),
            technical: "Fees attributed to transactions each detector matched. The two are not a partition: more than 125,000 blocks have both firing and in 1,326 of them the totals sum to more than the block collected, so they are drawn unstacked and beside the block's own fee take. Which detector is larger in a period is a fact about the detectors.",
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
                method_daily: Method::Estimated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Per block the subsidy follows from the height's halving era and is exact. Daily it is derived from the date, which assigns the whole day a single era, so on a halving day every block is credited the era the date falls in and the blocks on the other side of the halving are wrong by a factor of two. It is a step at the wrong moment rather than a blend.",
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
            definition: Some("The two halves of what a miner collects for a block. The subsidy is new bitcoin the protocol creates, currently 3.125 BTC and halving every 210,000 blocks. The fees are what senders paid for space. Only the first is on a schedule."),
            technical: "The block subsidy (new BTC created) halves every 210,000 blocks (~4 years). After the 2024 halving, the subsidy is 3.125 BTC per block. The subsidy schedule is fixed by the protocol; the fee side is not. Fees are a larger share of revenue when they hold up as the subsidy falls, and a smaller one when they fall faster, so the balance between the two is an outcome to watch rather than a guarantee.",
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
            definition: Some("How much computing work the whole network is doing, per second. Miners guess numbers until one produces a block hash below the target, and the hash rate is how many guesses everyone is making together. Nobody can count those guesses, so this is inferred from the difficulty the network settled on rather than measured."),
            technical: "An estimate. Nobody can count the network's guesses, so it is inferred from the difficulty the network settled on: difficulty times 2^32, divided by the 600 second block target. Difficulty only moves every 2,016 blocks, so the line is flat between retargets while the real rate is not. When the two drift apart, blocks arrive faster or slower than ten minutes until the next adjustment closes the gap.",
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
            definition: Some("Every 2,016 blocks, roughly every 2 weeks, Bitcoin measures how long those blocks took and resets difficulty so that the next 2,016 are expected to take two weeks. Expected, not guaranteed: block discovery is random and hash rate keeps moving, so an epoch routinely runs days early or late. Every node computes the same correction from the same rule. If miners leave, blocks come slower and the network makes itself easier. If they arrive, it makes itself harder. This is that correction, as a percentage."),
            technical: "The largest fall on record is 27.94%, at height 689,472 in the week of the 2021 mining ban in China. The largest rises are from 2010, when the network was small enough for one operator to move it. Rises and falls are coloured separately so the sign is readable at a glance.",
        }),
    },
    ChartMeta {
        slug: "diff-ribbon",
        title: "Difficulty Ribbon",
        desc_per_block: "Seven moving averages of difficulty, from 9 to 128 blocks. There is no unsmoothed line: every series here is derived",
        desc_daily: "Seven moving averages of difficulty, from 7 to 128 days. There is no unsmoothed line: every series here is derived",
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
            definition: Some("Difficulty, smoothed over seven different spans at once. Each line is the same number averaged over a different length of time, so the short ones follow recent changes and the long ones lag. Together they show whether difficulty has been rising or falling for a while or has just turned."),
            technical: "Seven moving averages of difficulty form the ribbon. Per-block resolution smooths over 9, 14, 25, 40, 60, 90 and 128 blocks; daily resolution smooths over 7, 14, 25, 40, 60, 90 and 128 days. Only the shortest span differs between them, and a span in blocks is about ten minutes each while a span in days is a day each, so no line covers the same length of time in the two views. When the ribbon is wide, difficulty is rising steadily. When it compresses or inverts, the recent average has fallen below the longer one, which is what a falling difficulty looks like once smoothed. Why it fell is not in this data: hash rate leaving the network and hash rate simply being unlucky for an epoch produce the same shape.",
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
            definition: Some("How hard it currently is to mine a block. Every miner races to find a number that makes the block's hash fall below a target, and difficulty is the easiest target the protocol allows divided by the current one. So a harder target is a smaller number and a larger difficulty. No one sets it by hand: every node derives the same value from the previous 2,016 blocks."),
            technical: "Read from the header of every block, so this is the protocol's own value. It changes once every 2,016 blocks, roughly every two weeks, which is why the line steps. Multiply by 2^32 for the expected number of hashes per block, the figure hash-rate estimates are built on.",
        }),
    },
    ChartMeta {
        slug: "diversity",
        title: "Mining Diversity Index",
        desc_per_block: "Herfindahl-Hirschman Index (HHI) measuring mining concentration. Below 1000 is competitive, above 1800 is concentrated",
        desc_daily: "Herfindahl-Hirschman Index (HHI) measuring mining concentration. Below 1000 is competitive, above 1800 is concentrated",
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
            population: "A Herfindahl index over pool shares for the window, normalised across identified blocks only, so unattributed blocks do not dilute it. The band thresholds are a presentation convention, not a security threshold.",
        }],
        about: Some(About {
            definition: Some("One number for how concentrated block production is. It is the Herfindahl-Hirschman Index, borrowed from competition economics: square each pool's percentage share and add them up. Two pools at 50% each score 5,000; a hundred pools at 1% each score 100. It rises faster than a share does, which is the point, because concentration is what it is built to be sensitive to."),
            technical: "Computed from the shares of blocks in the selected range, by pool, after excluding blocks with no identified pool and renormalising the rest. So it describes identified block production and not ownership of hash rate, and OCEAN template miners count as separate participants because that is how they appear in the coinbase. The 1,000 and 1,800 boundaries and their colours are the conventional reading from antitrust practice, not validated thresholds for Bitcoin. A monopoly scores 10,000, perfectly distributed mining scores near 0. Below 1,000 (green): competitive market. 1,000-1,800 (yellow): moderate concentration. Above 1,800 (red): high concentration, meaning a small number of pools control most of the hashrate. Unknown miners are excluded from the calculation.",
        }),
    },
    ChartMeta {
        slug: "empty-blocks",
        title: "Empty Blocks",
        desc_per_block: "Blocks carrying only the coinbase transaction, which is almost always the early chain: 78,800 of them are from 2009 and 2010",
        desc_daily: "Blocks carrying only the coinbase transaction, which is almost always the early chain: 78,800 of them are from 2009 and 2010",
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
            definition: Some("A block carrying nothing but the miner's own payment to itself. Every block has that one transaction, the coinbase, so a block with a count of one collected no fees and moved nobody's coins. They are almost extinct: close to 90,000 exist in the whole chain, and almost all of them predate 2011."),
            technical: "Blocks whose transaction count is one, so the coinbase is all there is. This chart does not establish why a miner produced one: a pool can be building on a tip it is still validating, and the data shows the result rather than the reason. 86,656 of them are unattributed, because pool identification depends on coinbase patterns that did not exist in 2009 and 2010, which is also when almost all of them were mined. Modern pools typically include transactions within seconds of receiving a new block.",
        }),
    },
    ChartMeta {
        slug: "empty-by-pool",
        title: "Empty Blocks by Pool",
        desc_per_block: "Coinbase-only blocks in the range, grouped by the pool that mined them",
        desc_daily: "Coinbase-only blocks in the range, grouped by the pool that mined them",
        category: Category::Mining,
        unit: Unit::Count,
        shape: Shape::Bar,
        source: Source::Mining(MiningChart::EmptyByPool),
        measurements: &[Measurement {
            series: "",
            series_daily: "",
            quantity: "Blocks containing only a coinbase transaction, by pool",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "As Empty Blocks, grouped by matched coinbase tag instead of by month, so it inherits the attribution gap: unmatched blocks group as Unknown, and the early chain is largely unmatched.",
        }],
        about: Some(About {
            definition: Some("Which pools mined the coinbase-only blocks. Almost all of them are unattributed, because pool identification reads patterns in the coinbase that did not exist in 2009 and 2010, which is when nearly every empty block was mined. So the Unknown bar is a statement about the early chain rather than about anonymity today."),
            technical: "Blocks with a transaction count of one, grouped by the miner string ingestion matched, with unmatched blocks under Unknown. Aggregated in SQL over the height range rather than from the loaded rows, so it answers for the whole window at any resolution.",
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
            population: "Blocks in the window grouped by matched pool tag. Unmatched blocks are grouped as Unknown rather than dropped; attribution is far weaker before 2012, when pools did not tag coinbases.",
        }],
        about: Some(About {
            definition: Some("Which mining pools are finding blocks, and in what proportion. Miners join a pool to get a steady payout instead of a rare large one, and the pool chooses which transactions its members' blocks include. The shares therefore show how much of block production a few operators direct."),
            technical: "Attributed from the coinbase transaction, where pools tag themselves by convention. Nothing requires it, so a pool that stops tagging, or tags differently, moves between these categories while nothing changes on the network. Untagged blocks are counted as unknown, never shared out among the named pools.",
        }),
    },
    ChartMeta {
        slug: "address-types",
        title: "Address Type Evolution",
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
            population: "Outputs of non-coinbase transactions. Six script types are plotted; OP_RETURN, bare multisig and unrecognised scripts are not among them, so the bands do not sum to every output.",
        }],
        about: Some(About {
            definition: Some("What kind of address each new output pays to. Every output commits to a script, and the shape of that script says which address format the receiver gave out: a legacy '1', a '3', a SegWit 'bc1q' or a Taproot 'bc1p'. Counting them is how the chain shows a format being adopted."),
            technical: "Counts of outputs created per script type, from the classification stored per block. Daily values are totals for the day, not per-block averages: 3.0 per block over 10 blocks is 30. Coinbase outputs are excluded, because ingestion skips that transaction.",
        }),
    },
    ChartMeta {
        slug: "address-types-pct",
        title: "Address Type Share",
        desc_per_block: "Each output type as a percentage of total, showing the shift from legacy to SegWit to Taproot",
        desc_daily: "Each output type as a percentage of total, showing the shift from legacy to SegWit to Taproot",
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
            population: "Six classified output types normalised against their own sum, so the bands always total 100 and describe those six rather than every output. Bare multisig, unrecognised scripts and OP_RETURN are outside the denominator, which makes it narrower than the eight-type denominator P2PKH Sunset uses and narrower still than Output Type Breakdown, which divides by every output. The same script type therefore reads differently across those three charts.",
        }],
        about: Some(About {
            definition: Some("The same output types as a share of the total rather than as counts. Counts rise and fall with how busy the chain is, which hides a format gaining ground during a quiet week; a share strips that out and shows the types competing with each other. The bands total 100% by construction, so one type can only grow at another's expense."),
            technical: "Six payment types normalised to 100%, so the bands total the whole. OP_RETURN outputs, bare multisig and unrecognised scripts are outside that denominator, which is why this chart and P2PKH Sunset give different percentages for the same type. This one divides by the six normalised types; P2PKH Sunset divides by every output, so its denominator is larger and its figure for the same type is always the smaller of the two.",
        }),
    },
    ChartMeta {
        slug: "avg-tx-size",
        title: "Avg Transaction Size",
        desc_per_block: "Average size of a transaction in bytes. Smaller means more efficient use of block space",
        desc_daily: "Daily average transaction size in bytes. Smaller means more efficient use of block space",
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
            definition: Some("How big a transaction is, on average, in each block. Size drives what a transaction costs, because fees are charged for space. Divided out of the whole block, so the header and the coinbase are in the numerator too."),
            technical: "Serialized block bytes divided by the number of transactions, including the coinbase and the block's own header and counters. Moving signature data into the witness does not remove those bytes, it discounts their weight, so a SegWit or Taproot transaction is not necessarily smaller here than a legacy one of the same shape. Weight is what the 4,000,000 unit limit constrains, so a falling byte average does not by itself mean more transactions fit.",
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
                population: "Outputs over transactions less one for the coinbase. Higher can mean batching or more change outputs; it does not distinguish payments from change.",
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
            definition: Some("How many inputs and outputs a typical transaction has. A payment with one input and two outputs, one of them change, is the ordinary shape. A higher output count means one transaction created more outputs, which spreads its fixed overhead across more of them. Who sent it, and whether a given output was a payment or change, is not in this data. Currently around 2.3 outputs and 1.7 inputs per transaction."),
            technical: "A higher output count per transaction is consistent with batching, where one transaction pays many recipients, but nothing here identifies the sender or separates a payment from a change output. A typical non-batched transaction has 1-2 inputs and 2 outputs (payment + change).",
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
                quantity: "Block bytes scaled by the storage overhead a node carries today",
                // The one that makes a single per-chart badge impossible: a
                // measured series beside an estimated companion.
                method_per_block: Method::Estimated,
                method_daily: Method::Estimated,
                per_block: Aggregation::CumulativeInWindow,
                daily: Aggregation::CumulativeInWindow,
                population: "Present-day disk size divided by present-day block data, applied to the accumulated series. Not a reconstruction of historical disk usage, which block sizes cannot give.",
            },
        ],
        about: Some(About {
            definition: Some("The total size of the blockchain on disk, back to 2009. What makes a node trustless is that it checked every one of those blocks itself, not that it still has them: an archival node keeps them all, while a pruned node verifies each block as it arrives and then discards the old ones, keeping a few gigabytes. So this is the floor for archiving the chain, not the price of running a node."),
            technical: "Two lines, measuring two different things. Block Data is the blocks themselves, summed across the range and added to the stored total for everything before it. That is block bytes alone: it excludes the chainstate and index databases a node also keeps, so a full data directory is larger than this line.\n\nDisk Size is that same series scaled by one ratio, what my node reports on disk today divided by its block data today, which is how the chainstate, the indexes and the undo files enter the chart. Only today's point of it is measured; every earlier point assumes those extras have always been the same fraction of block data, and they have not, because the unspent-output set grows on its own terms. So read the gap between the two lines as roughly what a node keeps beyond the blocks, not as a reconstruction of any past day's disk usage.\n\nA pruned node stores a fraction of either line and still verifies everything.",
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
            definition: Some("A running total of SegWit and Taproot outputs created inside the selected window, so it answers how many have appeared rather than what share they hold. Select ALL for the whole stored history."),
            technical: "Accumulated within the window only, so the first point is not the chain's beginning unless the window is. Outputs created, not transactions: the SegWit band sums P2WPKH and P2WSH outputs.",
        }),
    },
    ChartMeta {
        slug: "fullness-dist",
        title: "Block Fullness Distribution",
        desc_per_block: "Distribution of blocks by weight utilization percentage. Shows how many blocks are nearly full vs partially empty",
        desc_daily: "Distribution of blocks by weight utilization percentage. Shows how many blocks are nearly full vs partially empty",
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
            definition: Some("How full blocks have been, as a distribution rather than a line. Each bar counts the blocks that landed in a 10% band of the weight limit. Recently almost everything is in the top band: around 97% of the last month's blocks are above 99% of the limit."),
            technical: "A histogram of block fullness. Most modern blocks cluster near 100% because miners maximize fee revenue. Near-empty blocks do occur, and this chart does not establish why: a miner can be working from a coinbase-only template while validating a new tip, and the data shows only the result. On longer ranges that include early Bitcoin history, more blocks appear at lower percentages since demand was much lower.",
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
            definition: Some("The time between one block and the next. Bitcoin targets ten minutes on average and holds that average by adjusting difficulty, but any single gap is close to random: a two-minute gap and a fifty-minute gap are both ordinary."),
            technical: "The difference between consecutive block header timestamps.\n\nA timestamp is not a clock reading: consensus asks only two things of it, that it be later than the median of the previous eleven blocks, and no more than two hours ahead of network time. Nothing requires it to be later than the block immediately before it, which is why about one consecutive pair in 60 stored here runs backwards. That same two-hour allowance is what bounds how far back a pair can run, since the most negative a gap can be is roughly how far ahead the earlier block was permitted to be stamped: the largest backward step in this chain is just under two hours. They are plotted as found rather than cleaned, so this shows what the headers say rather than a corrected series.\n\nThe daily arm measures nothing of the sort: it divides each day's 1,440 minutes by the number of blocks that day produced, so a day of 144 blocks reads exactly 10 minutes. That is a different quantity from the average of a day's gaps, and it cannot show the spread within a day.",
        }),
    },
    ChartMeta {
        slug: "largest-tx",
        title: "Largest Transaction",
        desc_per_block: "Size of the largest transaction in each block, in bytes",
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
            definition: Some("The biggest single transaction in each block, by bytes. Size comes from how many inputs and outputs a transaction carries and how much witness data arrives with them, so a transaction can be large for several unrelated reasons and a size alone does not separate them. The largest in the chain is 3,992,821 bytes, which is almost an entire block."),
            technical: "Serialized size of the largest transaction in the block, read per transaction during ingestion. Size rather than weight, so a witness-heavy transaction reads larger here than it costs against the limit.",
        }),
    },
    ChartMeta {
        slug: "multi-velocity",
        title: "Adoption Velocity",
        desc_per_block: "Change in each output type's share of outputs, in percentage points over a trailing window: 144 blocks per block, 30 days daily",
        desc_daily: "Change in each output type's share of outputs, in percentage points over a trailing window: 144 blocks per block, 30 days daily",
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
            population: "The difference between a smoothed share now and the same share one window earlier: 144 blocks per block, 30 days daily. The result is in percentage points, so a share moving from 10 to 15 is 5 points and not 5 percent. Divergent lines do not establish that users migrated between types.",
        }],
        about: Some(About {
            definition: Some("Whether an output type is gaining or losing ground, rather than how much it holds. Each line is the change in that type's share over a trailing window, so a line above zero means the share grew. Three lines moving in three directions does not establish that anyone switched between them."),
            technical: "Shows the 30-day rate of change for each address type's share. Positive values mean the type is gaining share, negative means declining. When P2TR velocity is positive and P2PKH is negative, Taproot is actively replacing legacy usage.",
        }),
    },
    ChartMeta {
        slug: "p2pkh-sunset",
        title: "P2PKH Sunset Tracker",
        desc_per_block: "Decline of legacy P2PKH address usage over time. Horizontal lines mark 10% and 5% thresholds",
        desc_daily: "Decline of legacy P2PKH address usage over time. Horizontal lines mark 10% and 5% thresholds",
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
            population: "P2PKH outputs over eight classified output types: the six payment types plus bare multisig and unrecognised scripts. OP_RETURN outputs are outside it, and so are coinbase outputs, which ingestion excludes. Note this is a wider denominator than Address Type Share uses, so P2PKH reads lower here than there. The 90-day smoothing exists only at daily resolution. A falling share does not establish that those users moved to another type.",
        }],
        about: Some(About {
            definition: Some("The decline of the original Bitcoin address format. P2PKH is the '1...' address every wallet used before SegWit, and its share of outputs has been falling for years: currently 6.37%, against 7.42% for Taproot. The reference lines at 10% and 5% are markers for reading the trend, not protocol thresholds."),
            technical: "P2PKH outputs as a share of the eight classified output types, with a 90-day moving average at daily resolution only. The horizontal lines at 10% and 5% are reference marks chosen for this chart, not protocol thresholds. A falling share means other output types grew faster; it does not follow that any particular wallet or user moved.",
        }),
    },
    ChartMeta {
        slug: "propagation",
        title: "Rapid Consecutive Blocks",
        desc_per_block: "Consecutive blocks whose header timestamps are less than 60 seconds apart. Miners choose those timestamps, so this is not a measure of propagation",
        desc_daily: "Consecutive blocks whose header timestamps are less than 60 seconds apart. Miners choose those timestamps, so this is not a measure of propagation",
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
            definition: Some("Blocks that arrived very close together. Mining is random, so short gaps are ordinary and this is the tail of that distribution rather than a sign of trouble. It is measured from header timestamps, which miners choose themselves, so it says nothing about how fast blocks moved across the network."),
            technical: "Consecutive blocks whose header timestamps are less than 60 seconds apart. Not a propagation measurement: no arrival time is recorded anywhere in a block. Backward pairs are excluded rather than clamped, and about one of the chain's consecutive pairs in 60 runs backwards.",
        }),
    },
    ChartMeta {
        slug: "rbf",
        title: "Explicit RBF Signaling",
        desc_per_block: "Share of each block's transactions whose inputs signal replaceability under BIP 125",
        desc_daily: "Daily share of transactions signalling replaceability under BIP 125",
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
            definition: Some("How many transactions said, when they were sent, that they might be replaced by a higher-fee version. It is a signal in the transaction rather than a thing that happened: 67.7% of recent transactions carry it, and Bitcoin Core has allowed replacement regardless since version 28."),
            technical: "Replace-By-Fee (BIP 125) lets senders bump fees on unconfirmed transactions. A transaction signals RBF by setting at least one input's sequence number below 0xfffffffe. Higher adoption means more wallets support fee bumping, which can help users during congestion.",
        }),
    },
    ChartMeta {
        slug: "segwit",
        title: "SegWit Adoption",
        desc_per_block: "Percentage of transactions using Segregated Witness",
        desc_daily: "Daily average SegWit adoption percentage",
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
            population: "Numerator: **transactions** containing at least one witness input, which is what ingestion counts, one per transaction rather than one per input. Denominator: non-coinbase transactions, obtained by subtracting one coinbase per block. A transaction spending ten witness inputs counts once, so this is the share of transactions using witness data and not a share of inputs.",
        }],
        about: Some(About {
            definition: Some("The share of transactions using Segregated Witness. SegWit, activated in 2017, moves signatures into a part of the block that counts less toward the size limit, which makes those transactions cheaper to send. Adoption took years rather than months."),
            technical: "A transaction counts as SegWit when at least one of its inputs carries witness data, which is what determines the fee saving. How many outputs are SegWit is a different question, answered by the address-type charts, and it moved on a different schedule.",
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
            definition: Some("How much data each block carries, in serialized bytes. The consensus limit is 4,000,000 weight units rather than a byte count, so Weight Utilization is the measure of how full a block is and this is the raw size beside it. A larger block is not better or worse; it means more, or larger, transactions were included."),
            technical: "The serialised size of the block as my node stores it, witness data included. Consensus limits weight rather than bytes, to 4 million weight units, and witness bytes count a quarter as much toward that. This is why blocks pass the old one-megabyte figure. Weight utilisation has its own chart.",
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
                population: "P2TR outputs of non-coinbase transactions. Reads p2tr_count; the identically-valued taproot_spend_count column is misnamed and no longer read here.",
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
            definition: Some("Two ways to spend a Taproot output, and which one people use. A key-path spend shows a single signature and nothing else. A script-path spend reveals one branch of the script behind the output. Currently 90.4% are key-path, which is what a working contract looks like when everyone cooperates rather than evidence of simple payments."),
            technical: "Key-path spends reveal a single signature and nothing else. That is the point of Taproot: a single signer, an aggregated multisignature and the cooperative close of a contract are **indistinguishable on chain**, because BIP 341 makes them the same shape. So a high key-path share does not mean simple payments. It means most Taproot spenders took the path that reveals nothing, which is what a well-designed contract does when everyone cooperates. Script-path spends reveal one branch of the script tree, which is usually the case where cooperation broke down or was never possible. Both counts come from classifying witness shape, so they are detector counts rather than verified totals.",
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
            population: "Differences between consecutive block header timestamps, bucketed. Ten minutes is the target mean, not the most common bucket: the distribution is exponential, so the shortest bucket is the largest. Miners choose timestamps, so about one of the chain's pairs in 60 runs backwards; those are excluded from both arms rather than counted as instant blocks.",
        }],
        about: Some(About {
            definition: Some("How long blocks wait for each other, as a distribution. Mining is memoryless, so short gaps are the most common and the tail runs long: the median is 6.9 minutes even though the average is ten."),
            technical: "The shortest bucket is always the largest one, which surprises people who expect a peak at ten minutes. Mining is memoryless: every hash attempt has the same tiny chance of winning whatever has happened already, so the waiting time is exponentially distributed and the most likely gap is a short one. Measured across every interval in the chain: the median is 6.9 minutes, 63.9% of blocks arrive in under ten minutes, 4.5% take more than half an hour and 0.3% take over an hour. An exponential distribution with a ten-minute mean predicts 63.2%, 5.0% and 0.25%, so the chain tracks the theory to within seven tenths of a percentage point. Miners choose their own timestamps, so a block can appear to arrive before its predecessor.",
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
            definition: Some("How many transactions per second the chain confirmed. This counts base-chain transactions only, so it excludes Lightning payments and transfers inside an exchange, both of which move value without a block recording each one."),
            technical: "Transactions in the block divided by the seconds since the previous one, so a short interval reads high and a long one reads low even at a steady rate. Base-chain settlement only. Nothing carried over Lightning or netted inside an exchange appears here, which makes this a floor on activity rather than a measure of it.",
        }),
    },
    ChartMeta {
        slug: "tx-density",
        title: "Transaction Density",
        desc_per_block: "Transactions per 1,000 serialized bytes of block space",
        desc_daily: "Daily average transaction density (transactions per KB)",
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
            definition: Some("How many transactions fit into a kilobyte of block space. It moves with the shape of the transactions rather than with demand: a block of many small payments is denser than a block of a few large ones."),
            technical: "More transactions per KB means the average transaction is smaller and block space is used more efficiently. SegWit and Taproot tend to improve density by moving signatures to the discounted witness section.",
        }),
    },
    ChartMeta {
        slug: "tx-type-evolution",
        title: "Transaction Type Evolution",
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
            definition: Some("Which transaction format senders are using, over time. A transaction is legacy if it has no witness at all, SegWit if it has one, and Taproot if it spends a Taproot output. Currently 88.2% of transactions carry a witness."),
            technical: "Counted per transaction rather than per output, which is the distinction this chart keeps and several others do not: an output's type is chosen by whoever receives it, while a transaction's type is chosen by whoever spends. Detected from witness shape, so these are detector counts.",
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
            definition: Some("How many transactions each block contains. It moves with two things at once: how much people are transacting, and how much room each transaction takes. A block of many small payments and a block of a few large ones can carry the same data and count very differently."),
            technical: "Counted from the block as my node stores it, including the coinbase transaction that pays the miner. That adds exactly one to every block, which matters when comparing against sources that leave it out. A transaction counts in the block that confirmed it, so this says nothing about how long it waited in the mempool.",
        }),
    },
    ChartMeta {
        slug: "utxo-flow",
        title: "UTXO Flow",
        desc_per_block: "Inputs spent vs outputs created per block. When outputs exceed inputs, the UTXO set grows",
        desc_daily: "Daily average inputs spent vs outputs created. When outputs exceed inputs, the UTXO set grows",
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
                quantity: "Outputs created by non-coinbase transactions",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Includes provably unspendable OP_RETURN outputs, which never enter the spendable set, so outputs exceeding inputs does not by itself mean that set grew. Coinbase transactions are excluded by ingestion, so their outputs are missing.",
            },
            Measurement {
                series: "Inputs (consumed)",
                series_daily: "",
                quantity: "Inputs spent by non-coinbase transactions",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Coinbase transactions are excluded by ingestion, so the coinbase input is not counted.",
            },
        ],
        about: Some(About {
            definition: Some("The two sides of every block: outputs consumed and outputs created. Bitcoin has no accounts, only discrete outputs, so spending means destroying some and making others. When creation outruns consumption the set of unspent outputs grows, and every node holds that set to validate."),
            technical: "Every transaction consumes UTXOs (inputs) and creates new ones (outputs). When outputs exceed inputs, the UTXO set grows, increasing the memory requirements for full nodes. Consolidation transactions (many inputs, few outputs) shrink the set.",
        }),
    },
    ChartMeta {
        slug: "utxo-growth",
        title: "UTXO Growth Rate",
        desc_per_block: "Estimated net change in the output set per block, from non-coinbase transactions and excluding detected OP_RETURN. Positive means more outputs were created than consumed",
        desc_daily: "Estimated net change in the output set per day, from non-coinbase transactions and excluding detected OP_RETURN",
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
            quantity: "Estimated net change in the spendable output set",
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::DailyTotal,
            population: "Outputs created less detected OP_RETURN outputs less inputs consumed, over non-coinbase transactions. Coinbase outputs are missing because ingestion excludes the coinbase, which leaves the series short by about three per block. Negative means a net reduction; it does not establish consolidation.",
        }],
        about: Some(About {
            definition: Some("Whether the set of spendable coins is growing or shrinking. Every transaction consumes existing outputs and creates new ones, and the running total of unspent ones is the UTXO set. Positive means more were created than consumed, and negative means the reverse, which is consistent with wallets consolidating small coins without establishing that any of them did."),
            technical: "Outputs created, minus the ones that can never be spent, minus inputs consumed. OP_RETURN outputs are provably unspendable and never enter the set, so they do not count as growth. The coinbase transaction is absent from these counts, so its own outputs are missing and this runs about three per block short of a node's own figure. Net, not cumulative, which is why it goes negative and why a log axis cannot plot every point. The set matters because every node holds it in memory to validate, making it a running cost to the whole network.",
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
            definition: Some("Whether the chain has a working week. Transactions and fees averaged per block and grouped by UTC day, which is the simplest way to ask whether human schedules show up in on-chain activity."),
            technical: "Averages per block within each UTC weekday, over the selected range. UTC rather than any local time, so a pattern driven by one region's working hours is smeared across the boundary rather than aligned to it.",
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
            definition: Some("How full each block is against the limit that actually binds. The cap is 4,000,000 weight units rather than a byte count, and witness bytes count a quarter of what other bytes do, so this is the measure of a block being full. Blocks have been running close to the limit: around 98% on average over the last month, and about 93% over the last year."),
            technical: "The consensus limit is 4,000,000 weight units (4 MWU) per block. Witness data gets a 75% discount, so a block full of SegWit transactions can fit more data than one full of legacy transactions. Consistently high utilization (>90%) means demand for block space is near capacity.",
        }),
    },
    ChartMeta {
        slug: "witness-pct",
        title: "Witness Version Share",
        desc_per_block: "SegWit v0 vs Taproot as a percentage of all witness outputs",
        desc_daily: "SegWit v0 vs Taproot as a percentage of all witness outputs",
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
            quantity: "Share of outputs created to a witness program",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Native v0 against Taproot, and **only** those two: the denominator is their sum, not all outputs, so this is the split within witness outputs rather than witness adoption. Unrecognised or future witness versions are in neither band. Use Output Type Breakdown for a share of every output.",
        }],
        about: Some(About {
            definition: Some("Native SegWit against Taproot, as shares of the witness outputs alone. This is the split within modern output types, which is a different question from what share of all outputs they hold."),
            technical: "P2WPKH and P2WSH against P2TR, normalised to those three, so legacy and P2SH-wrapped outputs are outside the denominator entirely. A rising Taproot share here can coexist with a falling Taproot share of all outputs.",
        }),
    },
    ChartMeta {
        slug: "witness-share",
        title: "Witness Data Share",
        desc_per_block: "Witness data as percentage of block size. Higher means more SegWit discount savings",
        desc_daily: "Witness data as percentage of block size. Higher means more SegWit discount savings",
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
            definition: Some("How much of a block is witness data: signatures and scripts moved out of the transaction body by SegWit, where each byte counts a quarter against the weight limit. Currently about half of every block by raw bytes."),
            technical: "Witness data receives a 75% weight discount under SegWit rules. A higher witness share means more of the block is discounted data, effectively increasing the block's capacity beyond the old 1 MB limit. Recent blocks run near half: about 51% over the last 10,000, with well under a tenth of them anywhere in a 60 to 70 per cent band.",
        }),
    },
    ChartMeta {
        slug: "witness-tx-pct",
        title: "Output Type Breakdown",
        desc_per_block: "Legacy vs SegWit vs Taproot as a percentage of all outputs",
        desc_daily: "Legacy vs SegWit vs Taproot as a percentage of all outputs",
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
            population: "Every output in a block's non-coinbase transactions, which is the widest denominator any share chart here uses. The coinbase transaction is outside it, because ingestion skips that transaction entirely, so a block with only a coinbase has no outputs to take a share of and all three bands are absent rather than reading 0, 0 and 100. The third band is the residual after native v0 and Taproot, so it absorbs OP_RETURN, P2SH-wrapped witness outputs, bare multisig and anything unrecognised, and is named Other outputs rather than Legacy for that reason.",
        }],
        about: Some(About {
            definition: Some("Output types as shares of every output in the block, which is the widest denominator any share chart here uses. The coinbase transaction is the one exclusion, and the residual band carries everything else that is not native SegWit or Taproot."),
            technical: "Three bands over every output: SegWit v0, Taproot, and the rest. The third absorbs OP_RETURN, P2SH-wrapped witness outputs, bare multisig and anything unrecognised, which is why it is labelled Other outputs and not Legacy.",
        }),
    },
    ChartMeta {
        slug: "witness-versions",
        title: "Witness Version Comparison",
        desc_per_block: "SegWit v0 (P2WPKH + P2WSH) vs Taproot (P2TR) output counts per block",
        desc_daily: "Daily average SegWit v0 vs Taproot output counts",
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
                population: "P2WPKH plus P2WSH outputs of non-coinbase transactions. Not every possible witness program: unrecognised or future witness versions are not counted here. The band is drawn as \"SegWit v0\", because \"SegWit\" alone read as every witness output rather than the two native v0 types it counts.",
            },
            Measurement {
                series: "Taproot",
                series_daily: "",
                quantity: "Outputs created to a v1 witness program",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "P2TR outputs of non-coinbase transactions.",
            },
        ],
        about: Some(About {
            definition: Some("Which version of the witness programme an output uses. SegWit v0 arrived at block 481,824 in 2017 and Taproot, which is v1, at 709,632 in 2021. An output commits to one of them, so counting versions shows the upgrade spreading through the output set."),
            technical: "Counts of outputs by witness version, from the stored script classification. Only versions this site recognises are counted, so a future witness version would not appear until ingestion learns it.",
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
pub fn related(meta: &ChartMeta, max: usize) -> Vec<&'static ChartMeta> {
    let mut out: Vec<&'static ChartMeta> = CHARTS
        .iter()
        .filter(|c| {
            c.slug != meta.slug
                && c.category == meta.category
                && c.unit == meta.unit
        })
        .collect();
    if out.len() < max {
        out.extend(
            CHARTS
                .iter()
                .filter(|c| {
                    c.slug != meta.slug
                        && c.category == meta.category
                        && c.unit != meta.unit
                })
                .take(max - out.len()),
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

    /// The four page sources, parsed rather than trusted, so the registry
    /// cannot drift from what is actually rendered.
    const PAGES: &[&str] = &[
        include_str!("../../routes/observatory/network.rs"),
        include_str!("../../routes/observatory/fees.rs"),
        include_str!("../../routes/observatory/mining.rs"),
        include_str!("../../routes/observatory/embedded.rs"),
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
        ["last ", "past ", "recent"].iter().any(|p| l.contains(p))
            && ["month", "week", "day", "year", "block"]
                .iter()
                .any(|u| l.contains(u))
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
                "Block data only: it excludes",
                "the chart draws two lines and that sentence described only \
                 one of them. `Disk Size (est.)` scales block bytes by \
                 today's disk-to-blocks ratio, which is precisely how the \
                 chainstate and the indexes enter the chart, so saying the \
                 chart excludes them contradicted its own second series.",
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
                "it subtracts detected OP_RETURN and omits coinbase outputs \
                 entirely, running about three per block short of the node's \
                 own figure. Declared Estimated.",
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

    /// True when the literal appears in one of the four page sources.
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
