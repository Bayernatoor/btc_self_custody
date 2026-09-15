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
//! The builders are nearly uniform and the exceptions are real. 53 charts take
//! `&[BlockSummary]` with an optional `&[DailyAggregate]` variant. The other 8
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
    /// What is counted or measured, in the chart's own units.
    pub quantity: &'static str,
    /// Method at per-block resolution.
    ///
    /// Per resolution, not per chart, because the two genuinely differ.
    /// `diff-adjustment` reads the actual retarget blocks when it has them and
    /// reconstructs retargets from daily difficulty plateaus when it does not,
    /// so the same chart is exact at one resolution and estimated at the
    /// other. A single badge would be wrong at one of them.
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
            Self::Kilobytes => "KB",
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
    Histogram,
}

impl Shape {
    /// Donuts and histograms bucket their x axis, so "change over the range"
    /// and a shared time domain do not apply to them.
    pub fn has_time_axis(self) -> bool {
        !matches!(self, Self::Donut | Self::Histogram)
    }

    /// Stacked percentage bands already fill 0 to 100, so a second scale
    /// cannot be read against them. Refusing this here is why the clipping bug
    /// fixed in `fix/chart-zoom-axis` cannot recur through the compare path.
    pub fn accepts_second_series(self) -> bool {
        !matches!(self, Self::StackedPercent | Self::Donut | Self::Histogram)
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
    /// Optional, and the only optional half, because that is what the copy
    /// actually looks like: 36 charts arrived with a paragraph explaining how
    /// their number is computed and nothing explaining what it is. The
    /// subtitle carries a short answer meanwhile, and
    /// `most_charts_define_themselves` counts the gap so it cannot grow.
    pub definition: Option<&'static str>,
    /// How this site computes it, and what that excludes.
    ///
    /// Required, because it is the half that earns the word observatory, and
    /// because it is what the card's expandable held before the two were
    /// reconciled into one place.
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
    /// Empty during the migration: the cohort that proves the design carries
    /// declarations and the rest are being filled in.
    /// `the_undeclared_chart_count_only_shrinks` pins the remainder so the
    /// gap cannot grow and a new chart cannot be added without one.
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
    ///   describe the same period.
    /// - **A shape that accepts a second series.** Stacked percentage bands
    ///   already fill 0 to 100, and a donut or histogram has no shared x
    ///   domain to lay anything along.
    /// - **Not already using both axes.** `Unit::Mixed` charts have spent the
    ///   right axis on themselves.
    ///
    /// Derived rather than stored for the same reason `supports_log` is: a
    /// newly registered chart gets the right answer from what it already
    /// declares.
    pub fn can_compare(&self) -> bool {
        matches!(self.source, Source::Dashboard { .. })
            && self.shape.accepts_second_series()
            && !self.unit.occupies_both_axes()
    }
}

/// Every chart with a single-chart view, sorted by category then slug.
pub const CHARTS: &[ChartMeta] = &[
    ChartMeta {
        slug: "all-embedded-share",
        title: "All Embedded Data, Block Share",
        desc_per_block: "How much of each block is non-financial data (OP_RETURN outputs plus witness inscriptions)",
        desc_daily: "Daily average non-financial data share per block",
        category: Category::Embedded,
        unit: Unit::Percent,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::all_embedded_share_chart,
            daily: Daily::Fn(super::all_embedded_share_chart_daily),
        },
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
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
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Every block's coinbase transaction contains a scriptSig with arbitrary data. Miners use this to embed their pool identifier, block height (required since BIP-34), and sometimes custom messages or political statements. Longer messages indicate pools that pack additional data into this field.",
        }),
    },
    ChartMeta {
        slug: "inscription-envelope",
        title: "Inscription Payload vs Envelope",
        desc_per_block: "Breakdown of inscription witness data into actual content (payload) and protocol overhead (envelope structure)",
        desc_daily: "Daily average inscription payload vs envelope overhead per block",
        category: Category::Embedded,
        unit: Unit::Kilobytes,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::inscription_envelope_chart,
            daily: Daily::Fn(super::inscription_envelope_chart_daily),
        },
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Every Ordinals inscription wraps content in a witness envelope: OP_FALSE OP_IF ... OP_ENDIF with push opcodes and the 'ord' marker. The overhead is typically 10-15% of total inscription bytes. Higher overhead ratios indicate smaller inscriptions (like BRC-20 JSON operations) where the fixed envelope cost is a larger fraction.",
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
            quantity: "Share of fees paid by transactions matching the inscription detector",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Detected inscription fees over total fees. Both terms are estimates: the numerator is a detector match and the denominator is coinbase-derived.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
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
            quantity: "Share of transactions matching the inscription detector",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Matching witness items over the block's transactions. A matching item counts once, so several envelopes in one item count once.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Includes both the inscription content (images, text, JSON) and the witness envelope structure (OP_FALSE OP_IF, push opcodes, 'ord' marker). This represents the true on-chain footprint. Witness data gets a 75% weight discount, so inscriptions consume less block weight than their raw byte size suggests.",
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
            quantity: "Witness items matching the Ordinals marker",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::MeanOfPerBlockValues,
            population: "Every witness item in the block is scanned; a matching item counts once, so several envelopes in one item count once. The scan is not restricted to verified Taproot scripts.",
        }],
        about: Some(About {
            definition: Some("Inscriptions attach data such as an image or text to an individual satoshi, using the Ordinals convention introduced in 2023. The data rides in the witness part of a Taproot transaction, which is the cheapest room in a block."),
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
            quantity: "Share of block bytes in OP_RETURN scripts",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "OP_RETURN scriptPubKey bytes over serialized block bytes. Script bytes only: the output's value and length fields are not included, so this understates the space those outputs occupy.",
        }],
        about: None,
    },
    ChartMeta {
        slug: "opreturn-bytes",
        title: "OP_RETURN Volume",
        desc_per_block: "Bytes of data stored in OP_RETURN outputs per block by protocol",
        desc_daily: "Daily average OP_RETURN bytes per block by protocol",
        category: Category::Embedded,
        unit: Unit::Bytes,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::op_return_bytes_chart,
            daily: Daily::Fn(super::op_return_bytes_chart_daily),
        },
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
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
        measurements: &[],
        about: None,
    },
    ChartMeta {
        slug: "protocol-fee-competition",
        title: "Protocol Fee Competition",
        desc_per_block: "Fee revenue breakdown: standard transactions vs Ordinals inscriptions vs Runes protocol",
        desc_daily: "Daily fee revenue by protocol type",
        category: Category::Embedded,
        unit: Unit::Btc,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::protocol_fee_competition_chart,
            daily: Daily::Fn(super::protocol_fee_competition_chart_daily),
        },
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Shows how total fee revenue is split between standard Bitcoin transactions, Ordinals inscription transactions, and Runes protocol transactions. Reveals which protocol type is driving fee pressure at any given time.",
        }),
    },
    ChartMeta {
        slug: "runes-pct",
        title: "OP_RETURN Protocol Share",
        desc_per_block: "Which protocols are using the most OP_RETURN outputs. Runes dominate since their 2024 launch",
        desc_daily: "Which protocols are using the most OP_RETURN outputs. Runes dominate since their 2024 launch",
        category: Category::Embedded,
        unit: Unit::Percent,
        shape: Shape::StackedPercent,
        source: Source::Dashboard {
            per_block: super::runes_pct_chart,
            daily: Daily::Fn(super::runes_pct_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Share of OP_RETURN outputs, by protocol",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Each detector's count over the block's OP_RETURN count. Detectors match a prefix, so the residual band is whatever matched none of them rather than a named protocol.",
        }],
        about: None,
    },
    ChartMeta {
        slug: "unified-count",
        title: "All Embedded Data, Count",
        desc_per_block: "Outputs per block by protocol: Runes, Omni, Counterparty, Ordinals, BRC-20, and other data",
        desc_daily: "Daily average embedded outputs per block by protocol",
        category: Category::Embedded,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::unified_embedded_count_chart,
            daily: Daily::Fn(super::unified_embedded_count_chart_daily),
        },
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Stacked count of embedded data items by protocol. BRC-20 is a subset of Inscriptions (do not add them). Runes, Omni, and Counterparty are mutually exclusive subsets of OP_RETURN. See the Methodology page for the full taxonomy.",
        }),
    },
    ChartMeta {
        slug: "unified-volume",
        title: "All Embedded Data, Volume",
        desc_per_block: "Bytes of data embedded per block by protocol",
        desc_daily: "Daily average bytes of data embedded per block by protocol",
        category: Category::Embedded,
        unit: Unit::Bytes,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::unified_embedded_volume_chart,
            daily: Daily::Fn(super::unified_embedded_volume_chart_daily),
        },
        measurements: &[],
        about: None,
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
            quantity: "Fees per non-coinbase transaction",
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Block fees over the block's transactions less one for the coinbase. A block or day with no user transaction reports no reading rather than zero. The fee total is itself coinbase-derived, so this inherits that estimate.",
        }],
        about: None,
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
            quantity: "Value carried by a block's outputs",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::DailyTotal,
            population: "Sum of non-coinbase output values. Counts change returning to the sender, so it is throughput rather than value transferred between parties.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Total value of all non-coinbase outputs. This includes both the payment and the change output, so it overstates actual economic activity. Still useful for relative comparisons across time periods.",
        }),
    },
    ChartMeta {
        slug: "fee-heatmap",
        title: "Fee Rate Bands",
        desc_per_block: "Fee rate percentiles from p10 to p90 showing the full spread of fee rates per block. Click legend items to toggle bands",
        desc_daily: "Fee rate percentiles from p10 to p90 showing the full spread of fee rates per block. Click legend items to toggle bands",
        category: Category::Fees,
        unit: Unit::SatVb,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::fee_rate_heatmap_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "p10",
                quantity: "10th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Stored per-block percentile. Currently drawn as a stacked band, so the upper boundary of the plot is the sum of the five percentiles rather than p90; correcting the rendering is a separate measurement fix.",
            },
            Measurement {
                series: "p25",
                quantity: "25th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10. Stacked, so the drawn height is not the percentile.",
            },
            Measurement {
                series: "Median",
                quantity: "Median transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10. Stacked, so the drawn height is not the percentile.",
            },
            Measurement {
                series: "p75",
                quantity: "75th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10. Stacked, so the drawn height is not the percentile.",
            },
            Measurement {
                series: "p90",
                quantity: "90th percentile transaction fee rate in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As p10. p10 to p90 is the central 80 percent of transactions, not the full spread.",
            },
        ],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Five stacked bands showing fee rate percentiles. p10 (blue) is what the cheapest 10% of transactions paid. Median (orange) is the middle. p90 (red) is what urgent transactions paid. A wide spread means high fee variance. Click legend items to isolate specific bands.",
        }),
    },
    ChartMeta {
        slug: "fee-pressure",
        title: "Fee Pressure vs Block Space",
        desc_per_block: "Scatter plot showing the relationship between block fullness and fee rates. Clusters in the top-right indicate high-demand periods",
        desc_daily: "Scatter plot showing the relationship between block fullness and fee rates. Clusters in the top-right indicate high-demand periods",
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
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Each dot is one block. X-axis shows how full the block is (weight utilization %), Y-axis shows the median fee rate. When blocks are nearly full AND fees are high (top-right cluster), the network is under pressure. Dots in the bottom-right mean full blocks with low fees (normal operation). Top-left means high fees despite empty blocks (unusual).",
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
            quantity: "Fees as a share of what the miner earned",
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Fees over subsidy plus fees. The subsidy comes from the block height schedule and the fees are coinbase-derived, so the share inherits that estimate. A falling share can mean falling fees rather than a growing subsidy.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Shows fees as a percentage of total miner revenue (subsidy + fees). As the subsidy halves, this ratio increases. Typically 1-5% during normal periods, but has spiked to 10-40% during high-demand events.",
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
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "The white line shows the 144-block trailing average fee rate (roughly one day). Red dots appear when a block's median fee rate exceeds 5x that average, indicating sudden demand surges. Requires at least 300 blocks (1W+ range) for meaningful detection.",
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
            technical: "A transaction does not state its fee anywhere, so it is computed as inputs minus outputs. Denominated in BTC, not the dollars they were worth at the time, so the figure is comparable across the whole chain.",
        }),
    },
    ChartMeta {
        slug: "halving-era",
        title: "Halving Era Comparison",
        desc_per_block: "Side-by-side comparison of average block metrics across Bitcoin's halving eras. Shows how the network evolves between halvings",
        desc_daily: "Side-by-side comparison of average block metrics across Bitcoin's halving eras. Shows how the network evolves between halvings",
        category: Category::Fees,
        unit: Unit::Percent,
        shape: Shape::Bar,
        source: Source::Dashboard {
            per_block: super::halving_era_chart,
            daily: Daily::Fn(super::halving_era_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Comparison of metrics across halving eras",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Blocks grouped by subsidy era, then each metric normalised so its largest era reads 100, which is why every era peaks at 100 by construction. Only eras present in the window appear.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Each bar group represents one halving era (the period between two halvings). Metrics are normalized to percentages of the highest era so different scales are comparable. For example, if Era 4 has the highest average fee, it shows as 100% and other eras show relative to that. Click legend items to focus on specific metrics. Hover bars for actual values.",
        }),
    },
    ChartMeta {
        slug: "max-tx-fee",
        title: "Max Transaction Fee",
        desc_per_block: "Largest individual transaction fee per block in BTC. Fat-finger fees and high-priority transactions stand out",
        desc_daily: "Largest individual transaction fee per block in BTC. Fat-finger fees and high-priority transactions stand out",
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
                quantity: "Largest single transaction fee in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "One transaction per block, so this is an extreme rather than a total or a rate. No daily builder.",
            },
        ],
        about: None,
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
            technical: "Fee divided by virtual size for every transaction in the block, then the middle value. Virtual size is weight divided by four, which is the unit the fee market prices in. The median, because one very large fee drags an average somewhere no real transaction sat. The coinbase transaction pays no fee and is excluded.",
        }),
    },
    ChartMeta {
        slug: "protocol-fees",
        title: "Protocol Fee Revenue",
        desc_per_block: "Fee revenue breakdown by protocol: Ordinals inscriptions, Runes, and other transactions",
        desc_daily: "Fee revenue breakdown by protocol: Ordinals inscriptions, Runes, and other transactions",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::protocol_fee_breakdown_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "Inscriptions",
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
                quantity: "Fees paid by transactions matching the Runes detector",
                method_per_block: Method::HeuristicallyDetected,
                // No daily builder, so there is no daily method either.
                method_daily: Method::HeuristicallyDetected,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "As Inscriptions, and can double-count the same transaction's fee.",
            },
            Measurement {
                series: "Other",
                quantity: "The block's remaining fees",
                method_per_block: Method::Calculated,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "The residual after subtracting both detector totals, so it can be understated wherever they overlap. It does not establish that those transactions are ordinary payments.",
            },
        ],
        about: None,
    },
    ChartMeta {
        slug: "subsidy-fees",
        title: "Subsidy vs Fees",
        desc_per_block: "Block reward breakdown per block. The subsidy halves every 4 years while fees must eventually replace it",
        desc_daily: "Daily average block reward breakdown. The subsidy halves every 4 years while fees must eventually replace it",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::subsidy_vs_fees_chart,
            daily: Daily::Fn(super::subsidy_vs_fees_chart_daily),
        },
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "The block subsidy (new BTC created) halves every 210,000 blocks (~4 years). After the 2024 halving, the subsidy is 3.125 BTC per block. As the subsidy decreases over time, fees become a larger share of miner revenue.",
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
            definition: Some("How much computing work the whole network is doing, per second. Miners guess numbers until one produces a block hash below the target, and the hash rate is how many guesses everyone is making together. It is the closest thing to a price tag on attacking Bitcoin: an attacker has to out-compute everyone already mining."),
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
        source: Source::Dashboard {
            per_block: super::difficulty_adjustment_chart,
            daily: Daily::Fn(super::difficulty_adjustment_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Difficulty change at a retarget, as a percentage of the previous epoch",
            // The case that forced method to be per resolution. Per block it
            // reads the actual retarget blocks and the step is exact; daily it
            // reconstructs retargets from difficulty plateaus in the daily
            // column, so a window ending on a retarget day can carry a blended
            // value that is not any protocol difficulty.
            method_per_block: Method::Calculated,
            method_daily: Method::Estimated,
            per_block: Aggregation::WindowedDerived,
            daily: Aggregation::WindowedDerived,
            population: "One point per retarget, every 2,016 blocks, not per block. Drawn as two series split by sign so the bars can be coloured; together they are one series of retargets.",
        }],
        about: Some(About {
            definition: Some("Every 2,016 blocks, roughly a fortnight, Bitcoin measures how long those blocks took and resets difficulty so the next 2,016 should take exactly two weeks. Nobody votes and nobody decides. If miners leave, blocks come slower and the network makes itself easier. If they arrive, it makes itself harder. This is that correction, as a percentage."),
            technical: "The largest fall on record is 27.94%, at height 689,472 in the week of the 2021 mining ban in China. The largest rises are from 2010, when the network was small enough for one operator to move it. Rises and falls are coloured separately so the sign is readable at a glance.",
        }),
    },
    ChartMeta {
        slug: "diff-ribbon",
        title: "Difficulty Ribbon",
        desc_per_block: "Multiple moving averages of mining difficulty. When short MAs cross below long MAs, it may indicate miner capitulation",
        desc_daily: "Daily difficulty ribbon showing 7 moving averages from 7-day to 128-day",
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
                quantity: "Mining difficulty, smoothed over several windows",
                method_per_block: Method::Measured,
                method_daily: Method::Estimated,
                per_block: Aggregation::WindowedDerived,
                daily: Aggregation::WindowedDerived,
                population: "Seven moving averages of one quantity: 9, 14, 25, 40, 60, 90 and 128 blocks per block, and 7, 14, 25, 40, 60, 90 and 128 days daily. There is no unsmoothed series, so every line is derived. Inherits the retarget blending of the daily difficulty column.",
            },
        ],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Seven moving averages of difficulty (7, 14, 25, 40, 60, 90, 128 days) form a ribbon. When the ribbon is wide, difficulty is rising steadily. When it compresses or inverts (short MAs drop below long MAs), difficulty is declining, which can indicate less efficient miners are going offline. Historically, ribbon inversions have coincided with periods of reduced mining activity.",
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
                quantity: "Mining difficulty as the protocol reports it",
                method_per_block: Method::Measured,
                method_daily: Method::Estimated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Difficulty holds flat for 2,016 blocks and changes only at a retarget, so a daily mean equals the difficulty on every day except a retarget day, where it is a blend of two epochs and is no protocol difficulty. That is why the daily method is an estimate while the per-block one is not.",
            },
        ],
        about: Some(About {
            definition: Some("How hard it currently is to mine a block. Every miner races to find a number that makes the block's hash fall below a target, and difficulty is that target expressed as a multiple of the easiest one the protocol allows. Nobody sets it. It moves automatically with how much mining power is on the network."),
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
        shape: Shape::Donut,
        source: Source::Mining(MiningChart::Diversity),
        measurements: &[Measurement {
            series: "",
            quantity: "Concentration of block production across identified pools",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "A Herfindahl index over pool shares for the window, normalised across identified blocks only, so unattributed blocks do not dilute it. The band thresholds are a presentation convention, not a security threshold.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "The HHI is calculated by squaring each pool's market share percentage and summing the results. A monopoly scores 10,000, perfectly distributed mining scores near 0. Below 1,000 (green): competitive market. 1,000-1,800 (yellow): moderate concentration. Above 1,800 (red): high concentration, meaning a small number of pools control most of the hashrate. Unknown miners are excluded from the calculation.",
        }),
    },
    ChartMeta {
        slug: "empty-blocks",
        title: "Empty Blocks",
        desc_per_block: "Blocks with no user transactions, usually mined before the pool has received the previous block's transactions",
        desc_daily: "Blocks with no user transactions, usually mined before the pool has received the previous block's transactions",
        category: Category::Mining,
        unit: Unit::Count,
        shape: Shape::Histogram,
        source: Source::Mining(MiningChart::EmptyBlocks),
        measurements: &[Measurement {
            series: "",
            quantity: "Blocks containing only a coinbase transaction, by month",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Counted where tx_count is one, grouped by calendar month. An empty block is a valid block; the count does not establish why it was empty.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "A block with only a coinbase transaction (no user transactions). This happens when a miner finds a block before propagating the previous block's transactions. Common in early Bitcoin, rare today. Modern pools typically include transactions within seconds of receiving a new block.",
        }),
    },
    ChartMeta {
        slug: "empty-by-pool",
        title: "Empty Blocks by Pool",
        desc_per_block: "Which mining pools produce the most coinbase-only blocks",
        desc_daily: "Which mining pools produce the most coinbase-only blocks",
        category: Category::Mining,
        unit: Unit::Count,
        shape: Shape::Bar,
        source: Source::Mining(MiningChart::EmptyByPool),
        measurements: &[Measurement {
            series: "",
            quantity: "Blocks containing only a coinbase transaction, by pool",
            method_per_block: Method::HeuristicallyDetected,
            method_daily: Method::HeuristicallyDetected,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "As Empty Blocks, grouped by matched coinbase tag instead of by month, so it inherits the attribution gap: unmatched blocks group as Unknown, and the early chain is largely unmatched.",
        }],
        about: None,
    },
    ChartMeta {
        slug: "miner-dominance",
        title: "Mining Pool Share",
        desc_per_block: "Which mining pools are finding the most blocks. More distributed is healthier for the network",
        desc_daily: "Which mining pools are finding the most blocks. More distributed is healthier for the network",
        category: Category::Mining,
        unit: Unit::Percent,
        shape: Shape::Donut,
        source: Source::Mining(MiningChart::Dominance),
        measurements: &[Measurement {
            series: "",
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
        desc_per_block: "Output types per block. Watch P2PKH (legacy) shrink as P2WPKH (SegWit) and P2TR (Taproot) grow",
        desc_daily: "Daily average output types. Watch P2PKH (legacy) shrink as P2WPKH (SegWit) and P2TR (Taproot) grow",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::address_type_chart,
            daily: Daily::Fn(super::address_type_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Outputs created, by script type",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            // `avg * block_count`, so a total, which is why the old subtitle
            // "Daily average output types" was wrong.
            daily: Aggregation::DailyTotal,
            population: "Outputs of non-coinbase transactions. Six script types are plotted; OP_RETURN, bare multisig and unrecognised scripts are not among them, so the bands do not sum to every output.",
        }],
        about: None,
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
            quantity: "Share of outputs created, by script type",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Six classified output types normalised against their own sum, so OP_RETURN, bare multisig and unrecognised scripts are excluded from the denominator and the bands describe those six rather than every output.",
        }],
        about: None,
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
            quantity: "Mean serialized bytes per transaction",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Whole block size over transaction count, so header and transaction-count overhead are included and the coinbase is counted.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "SegWit and Taproot transactions are typically smaller than legacy because they move signature data to the witness section (which gets a weight discount). Lower values generally mean more transactions can fit per block.",
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
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Higher output counts per transaction indicate batching, where exchanges and services combine multiple payments into one transaction. This is more efficient use of block space. A typical non-batched transaction has 1-2 inputs and 2 outputs (payment + change).",
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
                quantity: "Serialized block bytes, accumulated",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::CumulativeInWindow,
                daily: Aggregation::CumulativeInWindow,
                population: "Every block in the window, plus the stored total for everything before it, so the value is absolute rather than range-relative.",
            },
            Measurement {
                series: "Disk Size (est.)",
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
            definition: Some("The total size of the block chain on disk. Every full node stores all of it, back to 2009, and that is what lets a node check the rules for itself instead of trusting anyone. The number matters because it sets the floor on what running one costs."),
            technical: "Blocks summed across the range and anchored to the size my node reports on disk now, so the present-day figure is measured rather than estimated. Block data only: it excludes the chainstate and index databases a node also keeps, so a full data directory is larger. A pruned node stores a fraction of this and still verifies everything.",
        }),
    },
    ChartMeta {
        slug: "cumulative-adoption",
        title: "Cumulative Adoption",
        desc_per_block: "Running total of SegWit transactions and Taproot outputs within this range. Select ALL for lifetime totals",
        desc_daily: "Cumulative SegWit and Taproot counts within this range. Select ALL for lifetime totals",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::cumulative_adoption_chart,
            daily: Daily::Fn(super::cumulative_adoption_chart_daily),
        },
        measurements: &[],
        about: None,
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
            quantity: "Blocks grouped by how much of the weight limit they used",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Weight divided by the four-million-unit limit, bucketed. Buckets are computed server-side for long ranges and from the blocks themselves for short ones; the count/percentage toggle changes the active unit.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "A histogram of block fullness. Most modern blocks cluster near 100% because miners maximize fee revenue. Empty or near-empty blocks usually appear right after a new block is found (before the miner has received transactions). On longer ranges that include early Bitcoin history, more blocks appear at lower percentages since demand was much lower.",
        }),
    },
    ChartMeta {
        slug: "interval",
        title: "Block Interval",
        desc_per_block: "Minutes between consecutive blocks. Target is 10 minutes",
        desc_daily: "Average daily block interval in minutes. Target is 10 minutes",
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
                quantity: "Time between blocks",
                method_per_block: Method::Calculated,
                method_daily: Method::Estimated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::WindowedDerived,
                population: "Per block this is the difference between consecutive header timestamps, which miners choose, so it can be zero or negative. Daily it is 1,440 minutes divided by the day's block count, which is not the mean of those differences and is wrong for an incomplete day; days with very few blocks are omitted.",
            },
        ],
        about: Some(About {
            definition: Some("The time between one block and the next. Bitcoin targets ten minutes on average and holds that average by adjusting difficulty, but any single gap is close to random: a two-minute gap and a fifty-minute gap are both ordinary."),
            technical: "The difference between consecutive block header timestamps. Miners set those timestamps and the protocol only loosely constrains them, so a handful of intervals in the chain's history are negative or implausibly long. They are plotted as found, because a cleaned series would be my data rather than the chain's.",
        }),
    },
    ChartMeta {
        slug: "largest-tx",
        title: "Largest Transaction",
        desc_per_block: "Size of the largest transaction in each block. Large transactions may indicate consolidations or complex scripts",
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
                quantity: "Size of the largest transaction in the block",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Serialized size of one transaction per block, not an average. No daily builder: the daily table stores no per-block maximum.",
            },
        ],
        about: None,
    },
    ChartMeta {
        slug: "multi-velocity",
        title: "Adoption Velocity",
        desc_per_block: "Rate of change for major address types. P2PKH declining, P2WPKH flattening, P2TR growing. Shows the transition between eras",
        desc_daily: "Rate of change for major address types. P2PKH declining, P2WPKH flattening, P2TR growing. Shows the transition between eras",
        category: Category::Network,
        unit: Unit::PercentagePoints,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::multi_velocity_chart,
            daily: Daily::Fn(super::multi_velocity_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Change in each script type's share of outputs",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::WindowedDerived,
            daily: Aggregation::WindowedDerived,
            population: "The difference between a smoothed share now and the same share one window earlier: 144 blocks per block, 30 days daily. The result is in percentage points, so a share moving from 10 to 15 is 5 points and not 5 percent. Divergent lines do not establish that users migrated between types.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
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
            quantity: "Share of outputs created to P2PKH",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "P2PKH outputs over the six classified output types, so OP_RETURN, bare multisig and unrecognised scripts are outside the denominator. The 90-day smoothing exists only at daily resolution. A falling share does not establish that those users moved to another type.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Tracks P2PKH (legacy '1' addresses) as a share of total outputs. The 90-day moving average smooths out noise. Horizontal lines mark the 10% and 5% thresholds. Crossing below these levels indicates the ecosystem is shifting away from legacy address formats.",
        }),
    },
    ChartMeta {
        slug: "propagation",
        title: "Rapid Consecutive Blocks",
        desc_per_block: "Blocks arriving within 60 seconds of each other, indicating fast mining luck or potential stale block races",
        desc_daily: "Blocks arriving within 60 seconds of each other, indicating fast mining luck or potential stale block races",
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
                quantity: "Gap to the previous block, where it is under 60 seconds",
                method_per_block: Method::Calculated,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Difference between consecutive block header timestamps, which miners choose, so this is not a measure of network propagation or of stale-block races. Backward pairs are excluded rather than clamped to zero. No daily builder.",
            },
        ],
        about: None,
    },
    ChartMeta {
        slug: "rbf",
        title: "RBF Adoption",
        desc_per_block: "Percentage of transactions opting into Replace-By-Fee per block",
        desc_daily: "Daily average RBF adoption percentage",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::rbf_chart,
            daily: Daily::Fn(super::rbf_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Share of transactions signalling replaceability",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Confirmed non-coinbase transactions whose input sequence numbers signal under BIP 125, over all confirmed non-coinbase transactions. Not the share actually replaced, and not wallet adoption: full-RBF policy means absence of the signal does not prevent replacement.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
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
            quantity: "Share of transactions spending a witness input",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::PerBlockObservation,
            // `avg_segwit_spend_count / (avg_tx_count - 1)`. Both terms carry
            // the same block count, so this is the day's pooled ratio rather
            // than the mean of per-block shares: it equals total segwit spends
            // over total non-coinbase transactions.
            daily: Aggregation::RatioOfTotals,
            population: "Numerator: inputs spending a witness program. Denominator: non-coinbase transactions, obtained by subtracting one coinbase per block.",
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
                quantity: "Serialized block size",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Every block in the window, including header and transaction-count overhead. Size is not weight: a block near the weight limit can be well under the size its witness discount allows.",
            },
        ],
        about: Some(About {
            definition: Some("How much data each block carries. Block space is limited and shared, so this is the clearest view of how full the chain is running. A larger block is not better or worse; it means more, or larger, transactions were included."),
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
                quantity: "Taproot outputs created",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Reads the taproot_spend_count column, which counts created P2TR outputs rather than inputs spending them; the column name is wrong and is being corrected. Outputs of non-coinbase transactions only.",
            },
        ],
        about: Some(About {
            definition: Some("How many new Taproot outputs are being created. Taproot, activated in 2021, is the most recent change to how Bitcoin outputs can be locked. It makes a complex spending condition, such as a multi-signature wallet, look the same on chain as an ordinary payment, which helps both privacy and fees."),
            technical: "Counts outputs with a pay-to-taproot script created in each block. Created, not spent: an output can sit unspent for years, so this leads the share of transactions that actually use Taproot. Inscriptions are stored in Taproot witness data, which is why this and the inscription charts move together from 2023.",
        }),
    },
    ChartMeta {
        slug: "taproot-spend-types",
        title: "Taproot Spend Types",
        desc_per_block: "Key-path vs script-path spends per block. How Taproot is actually being used",
        desc_daily: "Daily average key-path vs script-path spends. How Taproot is actually being used",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::taproot_spend_type_chart,
            daily: Daily::Fn(super::taproot_spend_type_chart_daily),
        },
        measurements: &[],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Key-path spends look like regular single-sig transactions on-chain, revealing no script details. Script-path spends reveal that a more complex script was involved (multisig, timelocks, etc.). A high key-path ratio suggests most Taproot usage is for simple payments rather than complex contracts.",
        }),
    },
    ChartMeta {
        slug: "time-dist",
        title: "Block Time Distribution",
        desc_per_block: "Distribution of time between consecutive blocks. Most cluster near the 10-minute target",
        desc_daily: "Distribution of time between consecutive blocks. Most cluster near the 10-minute target",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::Histogram,
        source: Source::TimeDist,
        measurements: &[Measurement {
            series: "",
            quantity: "Blocks grouped by the gap to their predecessor",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::GroupedSummary,
            daily: Aggregation::GroupedSummary,
            population: "Differences between consecutive block header timestamps, bucketed. Ten minutes is the target mean, not the most common bucket: the distribution is exponential, so the shortest bucket is the largest. Miners choose timestamps, so backward gaps exist.",
        }],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Shows how block intervals are distributed. The theoretical distribution is exponential with a 10-minute mean. Most blocks arrive within 20 minutes, but the long tail extends to 60+ minutes. This is normal Poisson process behavior, not a network problem.",
        }),
    },
    ChartMeta {
        slug: "tps",
        title: "Transactions per Second",
        desc_per_block: "Average TPS calculated from transactions per block interval",
        desc_daily: "Daily average transactions per second across all blocks",
        category: Category::Network,
        unit: Unit::TxPerSec,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::tps_chart,
            daily: Daily::Fn(super::tps_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Transactions per second",
            method_per_block: Method::Calculated,
            method_daily: Method::Calculated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::WindowedDerived,
            population: "Per block this is the block's transaction count over the gap to its predecessor, with a non-positive gap substituted as zero and the first block having none. Daily it is the day's transaction count over 86,400 seconds, including coinbase transactions, which is wrong for an incomplete day.",
        }],
        about: Some(About {
            definition: Some("How many transactions per second the chain is settling. It is a small number next to a card network, and deliberately so: every full node verifies every transaction, and that is what the limit buys."),
            technical: "Transactions in the block divided by the seconds since the previous one, so a short interval reads high and a long one reads low even at a steady rate. Base-chain settlement only. Nothing carried over Lightning or netted inside an exchange appears here, which makes this a floor on activity rather than a measure of it.",
        }),
    },
    ChartMeta {
        slug: "tx-density",
        title: "Transaction Density",
        desc_per_block: "Transactions per kilobyte of block space. Higher values indicate smaller, more efficient transactions",
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
                quantity: "Transactions per 1,000 serialized block bytes",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Transaction count over serialized size in kB, where size includes header and count overhead and the count includes the coinbase. It measures how many transactions fit in the bytes used, not payments per transaction and not fee efficiency: batching lowers density while using less space per payment.",
            },
        ],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
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
        shape: Shape::StackedAbsolute,
        source: Source::Dashboard {
            per_block: super::tx_type_evolution_chart,
            daily: Daily::Unavailable,
        },
        measurements: &[
            Measurement {
                series: "",
                quantity: "Share of transactions by input script type",
                method_per_block: Method::Measured,
                // No daily builder, so there is no daily method either.
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::Unsupported,
                population: "Three bands over the block's non-coinbase transactions. No daily builder: the daily table stores the component counts but this builder was never given a daily arm.",
            },
        ],
        about: None,
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
                quantity: "Outputs created by non-coinbase transactions",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Includes provably unspendable OP_RETURN outputs, which never enter the spendable set, so outputs exceeding inputs does not by itself mean that set grew. Coinbase transactions are excluded by ingestion, so their outputs are missing.",
            },
            Measurement {
                series: "Inputs (consumed)",
                quantity: "Inputs spent by non-coinbase transactions",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Coinbase transactions are excluded by ingestion, so the coinbase input is not counted.",
            },
        ],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Every transaction consumes UTXOs (inputs) and creates new ones (outputs). When outputs exceed inputs, the UTXO set grows, increasing the memory requirements for full nodes. Consolidation transactions (many inputs, few outputs) shrink the set.",
        }),
    },
    ChartMeta {
        slug: "utxo-growth",
        title: "UTXO Growth Rate",
        desc_per_block: "Net UTXO set change per block, counting only outputs that can be spent. Positive means the UTXO set is growing, negative means consolidation",
        desc_daily: "Daily net UTXO change across all blocks",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::BarWithLine,
        source: Source::Dashboard {
            per_block: super::utxo_growth_chart,
            daily: Daily::Fn(super::utxo_growth_chart_daily),
        },
        measurements: &[Measurement {
            series: "",
            quantity: "Estimated net change in the spendable output set",
            method_per_block: Method::Estimated,
            method_daily: Method::Estimated,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::DailyTotal,
            population: "Outputs created less detected OP_RETURN outputs less inputs consumed, over non-coinbase transactions. Coinbase outputs are missing because ingestion excludes the coinbase, which leaves the series short by about three per block. Negative means a net reduction; it does not establish consolidation.",
        }],
        about: Some(About {
            definition: Some("Whether the set of spendable coins is growing or shrinking. Every transaction consumes existing outputs and creates new ones, and the running total of unspent ones is the UTXO set. Positive means more were created than consumed; negative means wallets are consolidating many small coins into fewer large ones."),
            technical: "Outputs created, minus the ones that can never be spent, minus inputs consumed. OP_RETURN outputs are provably unspendable and never enter the set, so they do not count as growth. The coinbase transaction is absent from these counts, so its own outputs are missing and this runs about three per block short of a node's own figure. Net, not cumulative, which is why it goes negative and why a log axis cannot plot every point. The set matters because every node holds it in memory to validate, making it a running cost to the whole network.",
        }),
    },
    ChartMeta {
        slug: "weekday",
        title: "Weekday Activity",
        desc_per_block: "Average transaction count and fees by day of week. Reveals patterns between weekday and weekend network usage",
        desc_daily: "Average transaction count and fees by day of week. Reveals patterns between weekday and weekend network usage",
        category: Category::Network,
        unit: Unit::Mixed,
        shape: Shape::Bar,
        source: Source::Dashboard {
            per_block: super::weekday_activity_chart,
            daily: Daily::Fn(super::weekday_activity_chart_daily),
        },
        measurements: &[],
        about: None,
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
                quantity: "Share of the four-million-unit weight limit a block used",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "Block weight over 4,000,000. This is the capacity measure, unlike serialized size, because the limit is denominated in weight.",
            },
        ],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
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
            quantity: "Share of outputs created to a witness program",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Native v0 and Taproot outputs against the classified output total. Unrecognised or future witness versions are not counted.",
        }],
        about: None,
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
                quantity: "Witness bytes as a share of serialized block bytes",
                method_per_block: Method::Calculated,
                method_daily: Method::Calculated,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::RatioOfTotals,
                population: "Witness bytes over total block bytes. A byte fraction, not a measure of fee saving: the witness discount changes what those bytes cost in weight, not how many bytes they are.",
            },
        ],
        about: Some(About {
            // Migrated from the card's expandable, which was the
            // only place this was written. Definition still to come.
            definition: None,
            technical: "Witness data receives a 75% weight discount under SegWit rules. A higher witness share means more of the block is discounted data, effectively increasing the block's capacity beyond the old 1 MB limit. Modern blocks typically have 60-70% witness data.",
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
            quantity: "Share of outputs by script generation",
            method_per_block: Method::Measured,
            method_daily: Method::Measured,
            per_block: Aggregation::PerBlockObservation,
            daily: Aggregation::RatioOfTotals,
            population: "Three bands over the classified output total. The Legacy band is the residual after native v0 and Taproot, so it includes OP_RETURN, P2SH-wrapped witness outputs and anything unrecognised; it is not a count of legacy payment usage.",
        }],
        about: None,
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
                series: "SegWit",
                quantity: "Outputs created to a v0 witness program",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "P2WPKH plus P2WSH outputs of non-coinbase transactions. Not every possible witness program: unrecognised or future witness versions are not counted here. The band is drawn as \"SegWit\", which understates that it is native v0 only; renaming it is copy work.",
            },
            Measurement {
                series: "Taproot",
                quantity: "Outputs created to a v1 witness program",
                method_per_block: Method::Measured,
                method_daily: Method::Measured,
                per_block: Aggregation::PerBlockObservation,
                daily: Aggregation::MeanOfPerBlockValues,
                population: "P2TR outputs of non-coinbase transactions.",
            },
        ],
        about: None,
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
/// That split mattered. This used to be spread between `comparable_with` and
/// the picker's own gating, which meant "you cannot select an invalid
/// comparison" was enforced by the select not rendering. Safe only while the
/// selection died with the page; the moment it became shared state and
/// survived navigation, a comparison could arrive on a chart that never
/// offered it.
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
/// charts. `the_multi_metric_list_is_exactly_right` in the conformance suite
/// computes the truth from the real builders and fails if this drifts, which
/// is what keeps a hardcoded list honest.
///
/// **Twenty-one charts, a third of them.** I guessed three before running
/// the test. That gap is the measure of how far "offer every dashboard chart"
/// was from "offer every chart that can actually be laid over another", and it
/// is the strongest argument for the phase-2 contract: the feature loses a
/// third of its candidates to a limitation nobody had noticed, and the only
/// reason to accept that is that the alternative was showing one band of a
/// stacked chart under the whole chart's name.
///
/// **Temporary.** Phase 2 offers named measurements instead, at which point
/// "Difficulty Adjustment: Signed Retarget Change" and "Transaction Batching:
/// Outputs per Transaction" are selectable and complete, and this list deletes
/// itself. See `notes/phase-2-spec.md`.
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
    "tx-type-evolution",
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
        // Seven charts have no daily builder, so over a long range they would
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

    /// How many charts still lack the definition half, pinned so it can only
    /// shrink.
    ///
    /// 36 charts arrived with a paragraph explaining how their number is
    /// computed and nothing explaining what it is, because that copy was
    /// written as a card expandable rather than as a definition. Making
    /// `definition` optional is what let the migration happen without losing
    /// any of it; this is what stops optional turning into ignored.
    ///
    /// Lower the number when copy is written. Raising it is the thing this
    /// test exists to make someone argue for.
    #[test]
    fn the_definition_gap_only_shrinks() {
        const UNDEFINED_CEILING: usize = 48;
        let missing: Vec<&str> = CHARTS
            .iter()
            .filter(|c| c.about.is_none_or(|a| a.definition.is_none()))
            .map(|c| c.slug)
            .collect();
        assert!(
            missing.len() <= UNDEFINED_CEILING,
            "{} charts have no definition, up from {UNDEFINED_CEILING}: {missing:?}",
            missing.len()
        );
    }

    /// Copy on the page describes the metric, never this codebase's history
    /// with it. "Plotted raw and abbreviated on the axis: an earlier version
    /// divided by a trillion" shipped in the difficulty copy and reads as a
    /// code comment that escaped, because that is what it was.
    ///
    /// The style to match is the opening of any `definition`: present tense,
    /// direct, about the thing itself.
    #[test]
    fn long_copy_describes_the_metric_and_not_its_edit_history() {
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

    /// Comparison candidates are built from the dashboard rows already loaded
    /// for the range. A chart with any other source needs its own fetch over
    /// its own window, so overlaying it would put two series that describe
    /// different periods on one plot.
    #[test]
    fn only_dashboard_charts_can_be_compared() {
        for c in CHARTS {
            let dashboard = matches!(c.source, Source::Dashboard { .. });
            if !dashboard {
                assert!(
                    !c.can_compare(),
                    "{} is not built from the dashboard rows but offers \
                     comparison",
                    c.slug
                );
            }
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
    /// two answer identically for all 61 by 61 pairs at both resolutions.
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
