//! Key figures for the single-chart view, read back out of the built option.
//!
//! # Why read the option JSON rather than the data slice
//!
//! A number printed beside a chart must come from the same series the chart
//! drew, or the two disagree in front of the reader. Recomputing from the
//! `&[BlockSummary]` slice looks equivalent and is not: builders drop leading
//! points, apply moving averages, round, and in a few cases derive the plotted
//! value from two fields. Reading the option back means the figures are the
//! plotted values by construction. The click-to-detail bug fixed in
//! `fix/chart-correctness` was this same class of error, a tooltip and a modal
//! answering one click from two sources.
//!
//! # Why the figures differ by shape
//!
//! An average across six stacked bands is not a fact about anything, so this
//! does not compute one. Each shape gets the figures that are true for it, and
//! `Unavailable` is a real answer rather than a zero.

use super::registry::Shape;

/// One plotted point: `x` is the millisecond timestamp where the series has
/// one, `y` the plotted value.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    /// Millisecond timestamp, for the per-block charts whose series carry
    /// `[ts, value, height]`.
    pub x: Option<f64>,
    pub y: f64,
    /// Position in the series, so a daily chart's date can be recovered from
    /// the category axis. Daily builders emit bare numbers and put the dates
    /// on `xAxis.data`, so `x` is `None` for every one of them and the peak
    /// and low would otherwise render with no date at all.
    pub idx: usize,
}

/// Figures for one chart over the selected range.
#[derive(Clone, Debug, PartialEq)]
pub enum Kpis {
    /// A single series against time: the common case.
    Series {
        average: f64,
        peak: Point,
        low: Point,
        first: f64,
        last: f64,
        observations: usize,
        /// `xAxis.data` when the chart uses a category axis, so the caller can
        /// label the peak and low. Empty for a time axis, where `Point::x`
        /// carries the timestamp instead.
        axis_labels: Vec<String>,
    },
    /// Stacked bands, where the total and the leading band are the facts and
    /// an average is not.
    Bands {
        total_latest: f64,
        dominant: String,
        dominant_share_pct: f64,
        band_count: usize,
        observations: usize,
    },
    /// A donut or histogram: no time axis, so concentration is the fact.
    Categorical {
        top_name: String,
        top_value: f64,
        top_share_pct: f64,
        entries: usize,
    },
    /// No series, or nothing numeric in it. Rendered as "not available", never
    /// as zero.
    Unavailable,
}

/// Pull the y value out of one data element, which builders emit either as a
/// bare number or as `[x, y, ..]` with the extra slot carrying block height.
fn point_of(idx: usize, v: &serde_json::Value) -> Option<Point> {
    match v {
        serde_json::Value::Number(n) => Some(Point {
            x: None,
            y: n.as_f64()?,
            idx,
        }),
        serde_json::Value::Array(a) => match a.as_slice() {
            [x, y, ..] => Some(Point {
                x: x.as_f64(),
                y: y.as_f64()?,
                idx,
            }),
            [y] => Some(Point {
                x: None,
                y: y.as_f64()?,
                idx,
            }),
            [] => None,
        },
        // `{ name, value }`, which the donut builders use.
        serde_json::Value::Object(o) => Some(Point {
            x: None,
            y: o.get("value")?.as_f64()?,
            idx,
        }),
        _ => None,
    }
}

/// `xAxis.data`, the category labels. Daily charts put their dates here and
/// emit bare numbers in the series, so this is the only place the date of a
/// peak or low can come from for them.
fn axis_labels(opt: &serde_json::Value) -> Vec<String> {
    opt.get("xAxis")
        .and_then(|x| x.get("data"))
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn series_points(s: &serde_json::Value) -> Vec<Point> {
    s.get("data")
        .and_then(|d| d.as_array())
        .map(|a| {
            a.iter()
                .enumerate()
                .filter_map(|(i, v)| point_of(i, v))
                .collect()
        })
        .unwrap_or_default()
}

fn series_name(s: &serde_json::Value) -> String {
    s.get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("series")
        .to_string()
}

/// A moving average is a smoothing of the series it accompanies, not a second
/// metric, so it must not be mistaken for the primary series when one is
/// picked. Builders name them consistently ("144-block MA", "7d MA").
fn is_moving_average(s: &serde_json::Value) -> bool {
    let n = series_name(s).to_ascii_lowercase();
    n.contains(" ma") || n.contains("moving average") || n.ends_with("ma")
}

/// Key figures for the metric's own axis, which is the usual case.
pub fn compute(option_json: &str, shape: Shape) -> Kpis {
    compute_axis(option_json, shape, 0)
}

/// Key figures for one axis of a chart.
///
/// Axis 0 is the metric. Axis 1 is whatever was laid over it, and a reader
/// comparing two series wants the same five numbers for both: an overlay you
/// can see the shape of but not read the peak of is half a comparison.
///
/// `shape` is the *compared* chart's shape when reading axis 1, not the host
/// chart's, since it decides whether these read as a series, as bands or as
/// categories.
pub fn compute_axis(option_json: &str, shape: Shape, axis: u64) -> Kpis {
    let Ok(opt) = serde_json::from_str::<serde_json::Value>(option_json) else {
        return Kpis::Unavailable;
    };
    let Some(series) = opt.get("series").and_then(|s| s.as_array()) else {
        return Kpis::Unavailable;
    };
    // Only what the metric itself plots. A price overlay, a chain-size
    // overlay and a comparison series all sit on the right axis, and all
    // three are something laid beside the metric rather than part of it.
    //
    // This is what `bands` got wrong: it summed every series, so switching on
    // the price overlay added the dollar price to "latest total" on a stacked
    // chart and counted it as an extra band. Wrong before compare existed and
    // wrong more often afterwards, since a stacked absolute chart accepts a
    // comparison. Filtering here rather than in each branch, because the rule
    // is the same for all three and a fourth right-axis occupant would
    // otherwise have to remember it.
    let series: Vec<serde_json::Value> = series
        .iter()
        .filter(|s| {
            s.get("yAxisIndex").and_then(|i| i.as_u64()).unwrap_or(0) == axis
        })
        .cloned()
        .collect();
    if series.is_empty() {
        return Kpis::Unavailable;
    }

    match shape {
        Shape::Donut | Shape::Histogram => categorical(&opt, &series),
        Shape::StackedAbsolute | Shape::StackedPercent => bands(&series),
        _ => single_series(&opt, &series),
    }
}

fn single_series(
    opt: &serde_json::Value,
    series: &[serde_json::Value],
) -> Kpis {
    // Prefer series that are not moving averages; fall back to everything, so
    // a chart made only of averages still reports.
    let metric: Vec<&serde_json::Value> = series
        .iter()
        .filter(|s| !is_moving_average(s) && !series_points(s).is_empty())
        .collect();
    let metric = if metric.is_empty() {
        series
            .iter()
            .filter(|s| !series_points(s).is_empty())
            .collect()
    } else {
        metric
    };
    if metric.is_empty() {
        return Kpis::Unavailable;
    }
    // Every one of them, not just the first.
    //
    // A chart can split one measurement across several series for rendering
    // reasons. Difficulty adjustment draws "Harder" and "Easier" separately
    // so the bars can be coloured by sign, and each holds only its own half.
    // Reading the first meant the average excluded every easing retarget, the
    // low could never be negative, and the count was short by however many
    // there were.
    //
    // Concatenating is right for that case and harmless where there is one
    // series. Where there are genuinely several metrics the chart is stacked
    // or categorical, and neither routes through here.
    let pts: Vec<Point> =
        metric.iter().flat_map(|s| series_points(s)).collect();
    if pts.is_empty() {
        return Kpis::Unavailable;
    }

    let sum: f64 = pts.iter().map(|p| p.y).sum();
    let average = sum / pts.len() as f64;
    // `fold` rather than `max_by` so the first of equal extremes wins
    // deterministically, which keeps the reported date stable across renders.
    let peak = pts
        .iter()
        .fold(pts[0], |a, b| if b.y > a.y { *b } else { a });
    let low = pts
        .iter()
        .fold(pts[0], |a, b| if b.y < a.y { *b } else { a });

    Kpis::Series {
        average,
        peak,
        low,
        first: pts[0].y,
        last: pts[pts.len() - 1].y,
        observations: pts.len(),
        axis_labels: axis_labels(opt),
    }
}

fn bands(series: &[serde_json::Value]) -> Kpis {
    // The latest point of each band, which is what a stacked chart's right
    // edge shows. Bands with no data are skipped rather than counted as zero.
    let mut latest: Vec<(String, f64)> = Vec::new();
    let mut observations = 0usize;
    for s in series {
        let pts = series_points(s);
        if pts.is_empty() {
            continue;
        }
        observations = observations.max(pts.len());
        latest.push((series_name(s), pts[pts.len() - 1].y));
    }
    if latest.is_empty() {
        return Kpis::Unavailable;
    }
    let total: f64 = latest.iter().map(|(_, v)| *v).sum();
    let (dominant, top) = latest
        .iter()
        .fold(&latest[0], |a, b| if b.1 > a.1 { b } else { a })
        .clone();

    Kpis::Bands {
        total_latest: total,
        dominant,
        dominant_share_pct: if total > 0.0 {
            top / total * 100.0
        } else {
            0.0
        },
        band_count: latest.len(),
        observations,
    }
}

fn categorical(opt: &serde_json::Value, series: &[serde_json::Value]) -> Kpis {
    // A donut carries its labels on the data objects; a histogram carries them
    // on the category axis, positionally.
    let s = match series.iter().find(|s| !series_points(s).is_empty()) {
        Some(s) => s,
        None => return Kpis::Unavailable,
    };
    let data = match s.get("data").and_then(|d| d.as_array()) {
        Some(d) => d,
        None => return Kpis::Unavailable,
    };
    let labels = axis_labels(opt);

    let mut entries: Vec<(String, f64)> = Vec::new();
    for (i, v) in data.iter().enumerate() {
        let Some(p) = point_of(i, v) else { continue };
        let name = v
            .get("name")
            .and_then(|n| n.as_str())
            .map(str::to_string)
            .or_else(|| labels.get(i).cloned())
            .unwrap_or_else(|| format!("#{i}"));
        entries.push((name, p.y));
    }
    if entries.is_empty() {
        return Kpis::Unavailable;
    }
    let total: f64 = entries.iter().map(|(_, v)| *v).sum();
    let (top_name, top_value) = entries
        .iter()
        .fold(&entries[0], |a, b| if b.1 > a.1 { b } else { a })
        .clone();

    Kpis::Categorical {
        top_name,
        top_value,
        top_share_pct: if total > 0.0 {
            top_value / total * 100.0
        } else {
            0.0
        },
        entries: entries.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_figures_come_from_the_plotted_points() {
        let json = r#"{"series":[{"name":"TPS","data":[
            [1000,3.0,900],[2000,7.0,901],[3000,5.0,902]]}]}"#;
        match compute(json, Shape::Line) {
            Kpis::Series {
                average,
                peak,
                low,
                first,
                last,
                observations,
                ..
            } => {
                assert_eq!(observations, 3);
                assert!((average - 5.0).abs() < 1e-9);
                assert_eq!(
                    peak,
                    Point {
                        x: Some(2000.0),
                        y: 7.0,
                        idx: 1
                    }
                );
                assert_eq!(
                    low,
                    Point {
                        x: Some(1000.0),
                        y: 3.0,
                        idx: 0
                    }
                );
                assert_eq!(first, 3.0);
                assert_eq!(last, 5.0);
            }
            other => panic!("expected Series, got {other:?}"),
        }
    }

    /// A moving average smooths the primary series; reporting its average as
    /// the chart's average would quietly answer a different question.
    #[test]
    fn moving_average_series_is_not_mistaken_for_the_primary() {
        let json = r#"{"series":[
            {"name":"144-block MA","data":[[1000,100.0],[2000,100.0]]},
            {"name":"Fees","data":[[1000,1.0],[2000,3.0]]}]}"#;
        match compute(json, Shape::Line) {
            Kpis::Series { average, .. } => {
                assert!((average - 2.0).abs() < 1e-9)
            }
            other => panic!("expected Series, got {other:?}"),
        }
    }

    /// If every series is an average, report one rather than nothing.
    #[test]
    fn a_chart_made_only_of_averages_still_reports() {
        let json = r#"{"series":[{"name":"7d MA","data":[[1,2.0],[2,4.0]]}]}"#;
        assert!(matches!(
            compute(json, Shape::Line),
            Kpis::Series {
                observations: 2,
                ..
            }
        ));
    }

    #[test]
    fn stacked_reports_total_and_leading_band_not_an_average() {
        let json = r#"{"series":[
            {"name":"P2PKH","data":[[1,10.0],[2,20.0]],"stack":"t"},
            {"name":"P2TR","data":[[1,50.0],[2,60.0]],"stack":"t"}]}"#;
        match compute(json, Shape::StackedAbsolute) {
            Kpis::Bands {
                total_latest,
                dominant,
                dominant_share_pct,
                band_count,
                observations,
            } => {
                assert_eq!(band_count, 2);
                assert_eq!(observations, 2);
                assert!((total_latest - 80.0).abs() < 1e-9);
                assert_eq!(dominant, "P2TR");
                assert!((dominant_share_pct - 75.0).abs() < 1e-9);
            }
            other => panic!("expected Bands, got {other:?}"),
        }
    }

    #[test]
    fn donut_reports_concentration_from_named_slices() {
        let json = r#"{"series":[{"name":"Pools","data":[
            {"name":"Foundry","value":30.0},{"name":"AntPool","value":70.0}]}]}"#;
        match compute(json, Shape::Donut) {
            Kpis::Categorical {
                top_name,
                top_value,
                top_share_pct,
                entries,
            } => {
                assert_eq!(top_name, "AntPool");
                assert!((top_value - 70.0).abs() < 1e-9);
                assert!((top_share_pct - 70.0).abs() < 1e-9);
                assert_eq!(entries, 2);
            }
            other => panic!("expected Categorical, got {other:?}"),
        }
    }

    /// A histogram's labels live on the category axis, matched positionally.
    #[test]
    fn histogram_takes_its_label_from_the_category_axis() {
        let json = r#"{"xAxis":{"data":["0-1","1-2","2-3"]},
                       "series":[{"name":"Blocks","data":[5.0,40.0,2.0]}]}"#;
        match compute(json, Shape::Histogram) {
            Kpis::Categorical {
                top_name, entries, ..
            } => {
                assert_eq!(top_name, "1-2");
                assert_eq!(entries, 3);
            }
            other => panic!("expected Categorical, got {other:?}"),
        }
    }

    /// The skeleton shows an empty option while a range refetches, and a
    /// no-data chart has no series. Neither may render as zero.
    #[test]
    fn empty_and_malformed_options_are_unavailable_not_zero() {
        for json in [
            "",
            "{}",
            r#"{"series":[]}"#,
            "not json",
            r#"{"series":[{"data":[]}]}"#,
        ] {
            assert_eq!(
                compute(json, Shape::Line),
                Kpis::Unavailable,
                "expected Unavailable for {json:?}"
            );
        }
    }

    /// Daily builders emit bare numbers and keep their dates on the category
    /// axis. Without the index and the labels, the peak and low in the rail
    /// rendered with no date at all for every long range, which is most of
    /// them.
    #[test]
    fn daily_charts_expose_the_axis_label_for_their_extremes() {
        let json = r#"{"xAxis":{"data":["2026-01-01","2026-01-02","2026-01-03"]},
                       "series":[{"name":"Avg Tx Count","data":[10.0,99.0,20.0]}]}"#;
        match compute(json, Shape::Line) {
            Kpis::Series {
                peak,
                low,
                axis_labels,
                ..
            } => {
                assert_eq!(peak.x, None, "daily series carry no timestamp");
                assert_eq!(peak.idx, 1);
                assert_eq!(axis_labels.get(peak.idx).unwrap(), "2026-01-02");
                assert_eq!(axis_labels.get(low.idx).unwrap(), "2026-01-01");
            }
            other => panic!("expected Series, got {other:?}"),
        }
    }

    /// Equal extremes must resolve to the same point every render, or the
    /// reported date flickers between two dates with the same value.
    #[test]
    fn equal_extremes_resolve_deterministically() {
        let json =
            r#"{"series":[{"name":"x","data":[[1,5.0],[2,5.0],[3,5.0]]}]}"#;
        match compute(json, Shape::Line) {
            Kpis::Series { peak, low, .. } => {
                assert_eq!(peak.x, Some(1.0));
                assert_eq!(low.x, Some(1.0));
            }
            other => panic!("expected Series, got {other:?}"),
        }
    }

    /// A price overlay, a chain-size overlay and a comparison series all live
    /// on the right axis and are not part of the metric. Counting one as a
    /// band made "latest total" the sum of fees in BTC and a dollar price.
    #[test]
    fn right_axis_series_are_not_counted_as_bands() {
        let stacked = |extra: &str| {
            format!(
                r#"{{"series": [
                    {{"name": "Subsidy", "stack": "t", "yAxisIndex": 0,
                      "data": [10.0, 12.0]}},
                    {{"name": "Fees", "stack": "t", "yAxisIndex": 0,
                      "data": [1.0, 2.0]}}
                    {extra}
                ]}}"#
            )
        };
        let alone = compute(&stacked(""), Shape::StackedAbsolute);
        let with_price = compute(
            &stacked(
                r#", {"name": "Price (USD)", "yAxisIndex": 1,
                      "data": [90000.0, 95000.0]}"#,
            ),
            Shape::StackedAbsolute,
        );
        match (alone, with_price) {
            (
                Kpis::Bands {
                    total_latest: a,
                    band_count: ab,
                    ..
                },
                Kpis::Bands {
                    total_latest: b,
                    band_count: bb,
                    ..
                },
            ) => {
                assert_eq!(a, 14.0, "12 BTC subsidy plus 2 BTC fees");
                assert_eq!(
                    b, a,
                    "the overlay changed the total the metric reports"
                );
                assert_eq!(ab, 2);
                assert_eq!(bb, 2, "the overlay was counted as a third band");
            }
            other => panic!("expected Bands, got {other:?}"),
        }
    }

    /// And the single-series path must keep reading the metric, not the line
    /// laid over it, whatever order the series end up in.
    #[test]
    fn right_axis_series_are_not_mistaken_for_the_metric() {
        let json = r#"{"series": [
            {"name": "Difficulty", "yAxisIndex": 0,
             "data": [[1, 100.0], [2, 200.0]]},
            {"name": "Price (USD)", "yAxisIndex": 1,
             "data": [[1, 90000.0], [2, 95000.0]]}
        ]}"#;
        match compute(json, Shape::Line) {
            Kpis::Series { peak, .. } => {
                assert_eq!(peak.y, 200.0, "read the overlay instead");
            }
            other => panic!("expected Series, got {other:?}"),
        }
    }

    /// A chart whose only series is on the right axis has no metric to
    /// describe, and a number invented from the overlay would be a claim
    /// about data the chart is not about.
    #[test]
    fn a_chart_with_nothing_on_the_left_axis_reports_nothing() {
        let json = r#"{"series": [
            {"name": "Price (USD)", "yAxisIndex": 1, "data": [[1, 9.0]]}
        ]}"#;
        assert!(matches!(compute(json, Shape::Line), Kpis::Unavailable));
    }

    /// A chart may split one measurement across several series so the bars
    /// can be coloured by sign. Difficulty adjustment draws "Harder" and
    /// "Easier" separately, each holding only its own half, and reading the
    /// first meant the average excluded every easing retarget, the low could
    /// never be negative, and the count was short.
    #[test]
    fn a_metric_split_across_series_is_read_whole() {
        let json = r#"{"series": [
            {"name": "Harder", "data": [[1, 5.0], [3, 11.0]]},
            {"name": "Easier", "data": [[2, -27.9], [4, -3.0]]}
        ]}"#;
        match compute(json, Shape::Bar) {
            Kpis::Series {
                peak,
                low,
                observations,
                average,
                ..
            } => {
                assert_eq!(observations, 4, "counted only one half");
                assert_eq!(peak.y, 11.0);
                assert_eq!(low.y, -27.9, "the largest easing never appeared");
                assert!(
                    (average - (5.0 + 11.0 - 27.9 - 3.0) / 4.0).abs() < 1e-9
                );
            }
            other => panic!("expected a series: {other:?}"),
        }
    }

    /// A moving average is still a smoothing of its neighbour, not a second
    /// half of the metric, and must stay out of the figures.
    #[test]
    fn a_moving_average_is_still_excluded_when_several_series_are_read() {
        let json = r#"{"series": [
            {"name": "Fee Rate", "data": [[1, 10.0], [2, 20.0]]},
            {"name": "144-block MA", "data": [[1, 1000.0], [2, 2000.0]]}
        ]}"#;
        match compute(json, Shape::Line) {
            Kpis::Series {
                peak, observations, ..
            } => {
                assert_eq!(observations, 2);
                assert_eq!(peak.y, 20.0, "read the smoothing line");
            }
            other => panic!("expected a series: {other:?}"),
        }
    }

    /// Axis 1 is the comparison, and reading it must give that series'
    /// figures rather than the metric's. The same filter that keeps an
    /// overlay out of the metric's numbers is what makes this possible.
    #[test]
    fn each_axis_reports_its_own_series() {
        let json = r#"{"series": [
            {"name": "Difficulty", "yAxisIndex": 0,
             "data": [[1, 100.0], [2, 300.0]]},
            {"name": "Tx Count", "yAxisIndex": 1,
             "data": [[1, 4000.0], [2, 5000.0]]}
        ]}"#;
        match (
            compute_axis(json, Shape::Line, 0),
            compute_axis(json, Shape::Line, 1),
        ) {
            (Kpis::Series { peak: a, .. }, Kpis::Series { peak: b, .. }) => {
                assert_eq!(a.y, 300.0, "axis 0 should read the metric");
                assert_eq!(b.y, 5000.0, "axis 1 should read the comparison");
            }
            other => panic!("expected two series: {other:?}"),
        }
    }

    /// Asking for an axis nothing is plotted against reports nothing, rather
    /// than falling back to the metric and labelling it as the comparison.
    #[test]
    fn an_axis_with_no_series_reports_nothing() {
        let json = r#"{"series": [
            {"name": "Difficulty", "yAxisIndex": 0, "data": [[1, 100.0]]}
        ]}"#;
        assert!(matches!(
            compute_axis(json, Shape::Line, 1),
            Kpis::Unavailable
        ));
    }
}
