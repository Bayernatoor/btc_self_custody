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

/// Build a query string from key-value pairs, omitting empty values.
#[cfg(feature = "hydrate")]
fn build_query_string(params: &[(&str, Option<String>)]) -> String {
    let parts: Vec<String> = params
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|val| format!("{k}={val}")))
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
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
