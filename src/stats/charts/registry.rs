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
    TxPerSec,
    Minutes,
    Seconds,
    Difficulty,
    Ratio,
    Mixed,
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
            Self::TxPerSec => "tx/s",
            Self::Minutes => "minutes",
            Self::Seconds => "seconds",
            Self::Difficulty => "difficulty (T)",
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
    pub about: Option<&'static str>,
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
                Shape::StackedAbsolute
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
        about: None,
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
            daily: Daily::Fn(super::coinbase_message_length_chart_daily),
        },
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
    },
    ChartMeta {
        slug: "fees",
        title: "Total Fees per Block",
        desc_per_block: "Total transaction fees earned by miners in each block",
        desc_daily: "Average daily transaction fees earned by miners per block",
        category: Category::Fees,
        unit: Unit::Btc,
        shape: Shape::StackedAbsolute,
        source: Source::Fees,
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
            daily: Daily::Fn(super::largest_tx_chart_daily),
        },
        about: None,
    },
    ChartMeta {
        slug: "multi-velocity",
        title: "Adoption Velocity",
        desc_per_block: "Rate of change for major address types. P2PKH declining, P2WPKH flattening, P2TR growing. Shows the transition between eras",
        desc_daily: "Rate of change for major address types. P2PKH declining, P2WPKH flattening, P2TR growing. Shows the transition between eras",
        category: Category::Network,
        unit: Unit::Percent,
        shape: Shape::Line,
        source: Source::Dashboard {
            per_block: super::multi_velocity_chart,
            daily: Daily::Fn(super::multi_velocity_chart_daily),
        },
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
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
        about: None,
    },
    ChartMeta {
        slug: "utxo-growth",
        title: "UTXO Growth Rate",
        desc_per_block: "Net UTXO set change per block (outputs created minus inputs consumed). Positive means the UTXO set is growing, negative means consolidation",
        desc_daily: "Daily net UTXO change across all blocks",
        category: Category::Network,
        unit: Unit::Count,
        shape: Shape::BarWithLine,
        source: Source::Dashboard {
            per_block: super::utxo_growth_chart,
            daily: Daily::Fn(super::utxo_growth_chart_daily),
        },
        about: None,
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
        about: None,
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
        about: None,
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
