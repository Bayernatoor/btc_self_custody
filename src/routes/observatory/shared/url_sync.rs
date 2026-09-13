//! URL query param helpers for the Observatory (client-only).
//!
//! Keeps the browser address bar in sync with range, overlay, and section state
//! via `history.replaceState` without triggering Leptos router navigation.

/// Extract a query param value from a raw search string (e.g. "?range=3m&overlays=halvings").
#[cfg(feature = "hydrate")]
pub(super) fn get_query_param(search: &str, key: &str) -> Option<String> {
    let qs = search.strip_prefix('?').unwrap_or(search);
    qs.split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.to_string())
}

/// Percent-encode the characters that would otherwise break a query string.
///
/// Deliberately minimal. A general encoder would escape the comma that
/// `overlays` joins on, and [`get_query_param`] does not decode, so the read
/// side would then split `halvings%2Cbips` as one name and every overlay in a
/// shared link would come back off. Comma is a legal sub-delimiter in a query
/// value, so leaving it is correct as well as convenient.
///
/// What is escaped is what has structural meaning here: the pair separator,
/// the key separator, the fragment marker, the escape character itself, and
/// the space and plus that a careless reader would decode as each other.
///
/// Every value written today is already safe: slugs are `[a-z0-9-]`, ranges
/// are presets, dates are ISO. This exists so the next parameter added by
/// someone who has not read this file cannot silently corrupt the others.
///
/// Ungated, unlike the rest of this module, so it can be tested: everything
/// else here reaches for `window()` and only exists in the WASM build, while
/// the tests run under `ssr`. A pure string function is the one piece that
/// can be checked without a browser, so it is worth the attribute.
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
fn encode_value(v: &str) -> String {
    v.chars()
        .map(|c| match c {
            '%' => "%25".to_string(),
            '&' => "%26".to_string(),
            '=' => "%3D".to_string(),
            '#' => "%23".to_string(),
            '+' => "%2B".to_string(),
            ' ' => "%20".to_string(),
            other => other.to_string(),
        })
        .collect()
}

/// Build a query string from key-value pairs, omitting empty values.
#[cfg(feature = "hydrate")]
fn build_query_string(params: &[(&str, Option<String>)]) -> String {
    let parts: Vec<String> = params
        .iter()
        .filter_map(|(k, v)| {
            v.as_ref()
                .filter(|val| !val.is_empty())
                .map(|val| format!("{k}={}", encode_value(val)))
        })
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

/// Which scale each value axis is on.
///
/// A struct rather than two more positional `bool` arguments, because
/// `sync_url_to_state` already takes eight and two adjacent bools at a call
/// site are one transposition away from putting the overlay's scale on the
/// metric's axis in every shared link.
#[derive(Clone, Copy, Default)]
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub(super) struct Scales {
    pub left_log: bool,
    pub right_log: bool,
}

impl Scales {
    /// `Some("log")` or `None`, since linear is the default and a default in
    /// the URL is noise in something meant to be pasted into a sentence.
    #[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
    fn param(on: bool) -> Option<String> {
        on.then(|| "log".to_string())
    }
}

/// Update the browser URL bar to reflect current Observatory state
/// without triggering a Leptos router navigation (uses history.replaceState).
#[cfg(feature = "hydrate")]
pub(super) fn sync_url_to_state(
    pathname: &str,
    range: &str,
    overlays: &[(&str, bool)],
    section: Option<&str>,
    custom_from: Option<&str>,
    custom_to: Option<&str>,
    scales: Scales,
    compare: &str,
) {
    let range_param = if range != "1y" {
        Some(range.to_string())
    } else {
        None
    };
    let active: Vec<&str> = overlays
        .iter()
        .filter(|(_, on)| *on)
        .map(|(name, _)| *name)
        .collect();
    let overlays_param = if active.is_empty() {
        None
    } else {
        Some(active.join(","))
    };
    let section_param = section.map(|s| s.to_string());
    let from_param = custom_from.map(|s| s.to_string());
    let to_param = custom_to.map(|s| s.to_string());

    let qs = build_query_string(&[
        ("range", range_param),
        ("overlays", overlays_param),
        ("section", section_param),
        ("from", from_param),
        ("to", to_param),
        ("scale", Scales::param(scales.left_log)),
        ("overlay_scale", Scales::param(scales.right_log)),
        // Written whatever the current chart is. It is a stored intent, and
        // the page resolving it decides whether it can be drawn, so a link
        // copied from a chart where it does not apply still restores it on a
        // chart where it does.
        ("compare", Some(compare.to_string())),
    ]);
    let hash = leptos::prelude::window()
        .location()
        .hash()
        .unwrap_or_default();
    let url = format!("{pathname}{qs}{hash}");
    let _ = leptos::prelude::window()
        .history()
        .expect("history")
        .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
}

/// Build the full shareable URL for a specific chart, including current state.
#[cfg(feature = "hydrate")]
pub fn build_share_url(chart_id: &str) -> String {
    let window = leptos::prelude::window();
    let origin = window.location().origin().unwrap_or_default();
    let pathname = window.location().pathname().unwrap_or_default();
    let search = window.location().search().unwrap_or_default();
    format!("{origin}{pathname}{search}#{chart_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The separators that would let one parameter eat another. Every value
    /// written today is safe, so this guards the next one rather than a
    /// current bug.
    #[test]
    fn encoding_escapes_what_would_break_the_query_string() {
        assert_eq!(encode_value("a&b"), "a%26b");
        assert_eq!(encode_value("a=b"), "a%3Db");
        assert_eq!(encode_value("a#b"), "a%23b");
        assert_eq!(encode_value("a%b"), "a%25b");
        assert_eq!(encode_value("a b"), "a%20b");
        assert_eq!(encode_value("a+b"), "a%2Bb");
    }

    /// The comma stays. `overlays` joins on it and `get_query_param` does not
    /// decode, so escaping it would split `halvings,bips` as a single unknown
    /// name and quietly drop every overlay from a shared link.
    #[test]
    fn encoding_leaves_the_overlay_separator_alone() {
        assert_eq!(encode_value("halvings,bips,core"), "halvings,bips,core");
    }

    /// Everything written today passes through untouched, so adding the
    /// encoder cannot have changed a single existing link.
    #[test]
    fn encoding_is_a_no_op_for_every_value_written_today() {
        for v in [
            "3m",
            "all",
            "custom",
            "1y",
            "halvings,bips,core,price,chain_size,events",
            "2024-04-20",
            "2009-01-03",
            "utxo-growth",
            "diff-ribbon",
            "median-rate",
            "all-embedded-share",
            "log",
            "network",
            "fees",
        ] {
            assert_eq!(encode_value(v), v, "{v} would have been rewritten");
        }
    }

    /// Linear is the default, and a default in the URL is noise in something
    /// meant to be pasted into a sentence.
    #[test]
    fn only_a_logarithmic_axis_reaches_the_url() {
        assert_eq!(Scales::param(true).as_deref(), Some("log"));
        assert_eq!(Scales::param(false), None);
    }
}
