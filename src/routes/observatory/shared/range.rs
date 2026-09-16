//! Range selector for the Observatory.
//!
//! `RangeSelector` renders preset range buttons (1D-ALL + Custom) and a
//! date picker. Rendered inline in the chart-page toolbar, and inside the
//! unified `ChartSettingsPanel`'s Range tab (see `shared/mod.rs`) as the
//! floating-access equivalent.

use crate::stats::types::uses_daily_aggregates;
use leptos::prelude::*;

use super::state::ObservatoryState;
use crate::routes::observatory::helpers::*;

/// The first block's UTC date. A range starting before it holds no blocks.
const GENESIS_DATE: &str = "2009-01-03";

/// Check a pair of `YYYY-MM-DD` dates and clamp the end to today, or say why
/// not.
///
/// Pure, and shared by both pickers, because there are two: one under the
/// chart header and one in the settings panel. They had one validator each
/// the moment the second was written, which is how two arms of the same
/// question end up answering it differently.
///
/// The reason strings are shown to the reader. The previous version returned
/// early on each of these conditions, so an invalid pair made "Go" a button
/// that did nothing and said nothing.
pub fn validate_custom_range(
    from: &str,
    to: &str,
    today: &str,
) -> Result<(String, String), &'static str> {
    if from.is_empty() || to.is_empty() {
        return Err("Pick both dates.");
    }
    if from > to {
        return Err("The start date is after the end date.");
    }
    if from < GENESIS_DATE {
        return Err("The chain starts on 3 January 2009.");
    }
    if from > today {
        return Err("That start date is in the future.");
    }
    // A future end date is clamped rather than refused: asking for "up to the
    // end of the month" before the month is over is a reasonable thing to
    // type, and the answer is everything up to now.
    let end = if to > today { today } else { to };
    Ok((from.to_string(), end.to_string()))
}

/// Range selector bar (1D through ALL + YTD)
#[component]
pub fn RangeSelector() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let set_range = state.set_range;
    let set_custom_from = state.set_custom_from;
    let set_custom_to = state.set_custom_to;

    // Open when the current range already is a custom one, so arriving here
    // with a window selected shows the window rather than hiding it behind a
    // button that is already highlighted.
    let (picker_open, set_picker_open) =
        signal(range.get_untracked() == "custom");
    let (local_from, set_local_from) =
        signal(state.custom_from.get_untracked().unwrap_or_default());
    let (local_to, set_local_to) =
        signal(state.custom_to.get_untracked().unwrap_or_default());
    let (problem, set_problem) = signal::<Option<&'static str>>(None);

    let range_label = move || {
        let r = range.get();
        if r == "custom" {
            "custom range"
        } else {
            let n = range_to_blocks(&r);
            if uses_daily_aggregates(n) {
                "daily averages"
            } else {
                "per block"
            }
        }
    };

    let apply_custom = move |_| {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        match validate_custom_range(&local_from.get(), &local_to.get(), &today)
        {
            Ok((from, to)) => {
                set_problem.set(None);
                set_custom_from.set(Some(from));
                set_custom_to.set(Some(to));
                // Left open, like the header's picker: after applying, the
                // window in force is worth showing rather than hiding behind
                // a button that says only "Custom".
                set_range.set("custom".to_string());
            }
            // Said rather than swallowed: this was five silent early
            // returns, so a bad pair left the button doing nothing.
            Err(why) => set_problem.set(Some(why)),
        }
    };

    let select_preset = move |r: String| {
        set_custom_from.set(None);
        set_custom_to.set(None);
        set_picker_open.set(false);
        set_range.set(r);
    };

    let presets = [
        "1d", "1w", "1m", "3m", "6m", "ytd", "1y", "2y", "5y", "10y", "all",
    ];

    view! {
        <div class="flex flex-col gap-2">
            // Mobile: dropdown + label
            <div class="flex sm:hidden items-center gap-2">
                <div class="relative inline-block">
                    <select
                        aria-label="Time range"
                        class="appearance-none bg-[#0a1a2e] text-white/80 text-sm border border-white/10 rounded-xl pl-3 pr-8 py-2 cursor-pointer focus:outline-none focus:border-[#f7931a]/40 transition-colors"
                        prop:value=move || range.get()
                        on:change=move |ev| {
                            use wasm_bindgen::JsCast;
                            if let Some(t) = ev.target() {
                                if let Ok(s) = t.dyn_into::<leptos::web_sys::HtmlSelectElement>() {
                                    if s.value() == "custom" {
                                        set_picker_open.set(true);
                                    } else {
                                        select_preset(s.value());
                                    }
                                }
                            }
                        }
                    >
                        {presets.into_iter().map(|r| {
                            let val = r.to_string();
                            let label = r.to_uppercase();
                            view! { <option value=val>{label}</option> }
                        }).collect::<Vec<_>>()}
                        <option value="custom">"Custom"</option>
                    </select>
                    <svg class="absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none w-3.5 h-3.5 text-white/40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                    </svg>
                </div>
                <span class="text-xs text-white/40">{range_label}</span>
            </div>
            // Desktop: button grid + label
            <div class="hidden sm:flex items-center">
                <div class="flex gap-1.5 bg-[#0a1a2e] rounded-xl p-1.5 border border-white/5">
                    {presets.into_iter().map(|r| {
                        let r_str = r.to_string();
                        let r_display = r.to_uppercase();
                        let r_clone = r_str.clone();
                        view! {
                            <button
                                class=move || {
                                    if range.get() == r_clone {
                                        "px-3 py-1 text-xs rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                                    } else {
                                        "px-3 py-1 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                                    }
                                }
                                on:click={
                                    let r = r_str.clone();
                                    move |_| select_preset(r.clone())
                                }
                            >
                                {r_display}
                            </button>
                        }
                    }).collect::<Vec<_>>()}
                    <button
                        class=move || {
                            if range.get() == "custom" {
                                "px-3 py-1 text-xs rounded-lg bg-[#f7931a] text-[#1a1a2e] font-semibold cursor-pointer"
                            } else {
                                "px-3 py-1 text-xs rounded-lg text-white/40 hover:text-white/70 hover:bg-white/5 transition-all cursor-pointer"
                            }
                        }
                        on:click=move |_| set_picker_open.update(|v| *v = !*v)
                    >
                        "Custom"
                    </button>
                </div>
                <span class="ml-3 text-xs text-white/60 self-center">{range_label}</span>
            </div>
            // Date picker (shown when Custom is active/clicked)
            <Show when=move || picker_open.get()>
                <div class="flex items-center gap-2 bg-[#0a1a2e] rounded-xl p-2 border border-white/10">
                    <input
                        type="date"
                        min="2009-01-03"
                        max=move || chrono::Utc::now().format("%Y-%m-%d").to_string()
                        class="bg-[#0d2137] text-white text-xs border border-white/10 rounded-lg px-2 py-1.5 focus:outline-none focus:border-[#f7931a]/40"
                        style="color-scheme: dark"
                        prop:value=move || local_from.get()
                        on:input=move |ev| {
                                set_local_from.set(event_target_value(&ev));
                        }
                    />
                    <span class="text-white/30 text-xs">"to"</span>
                    <input
                        type="date"
                        min="2009-01-03"
                        max=move || chrono::Utc::now().format("%Y-%m-%d").to_string()
                        class="bg-[#0d2137] text-white text-xs border border-white/10 rounded-lg px-2 py-1.5 focus:outline-none focus:border-[#f7931a]/40"
                        style="color-scheme: dark"
                        prop:value=move || local_to.get()
                        on:input=move |ev| {
                                set_local_to.set(event_target_value(&ev));
                        }
                    />
                    <button
                        class="px-3 py-1.5 text-xs bg-[#f7931a] text-[#1a1a2e] font-semibold rounded-lg cursor-pointer hover:bg-[#f4a949] transition-colors"
                        on:click=apply_custom
                    >
                        "Go"
                    </button>
                </div>
            </Show>
            // Why nothing happened, when nothing happened.
            <Show when=move || problem.get().is_some()>
                <p class="text-xs text-[#f7931a]">{move || problem.get().unwrap_or_default()}</p>
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::validate_custom_range;

    const TODAY: &str = "2026-09-16";

    /// The pair a reader is most likely to type, and the one the browser pass
    /// used: a single day, start equal to end.
    #[test]
    fn one_day_is_a_valid_window() {
        assert_eq!(
            validate_custom_range("2021-07-04", "2021-07-04", TODAY),
            Ok(("2021-07-04".to_string(), "2021-07-04".to_string()))
        );
    }

    /// Every rejection has to say which rejection it is. These were five
    /// silent early returns, so "Go" did nothing and explained nothing, which
    /// is the same defect class as a comparison the picker offers and cannot
    /// draw.
    #[test]
    fn each_refusal_says_what_is_wrong() {
        assert_eq!(
            validate_custom_range("", "2024-01-01", TODAY),
            Err("Pick both dates.")
        );
        assert_eq!(
            validate_custom_range("2024-01-01", "", TODAY),
            Err("Pick both dates.")
        );
        assert_eq!(
            validate_custom_range("2024-06-01", "2024-01-01", TODAY),
            Err("The start date is after the end date.")
        );
        assert_eq!(
            validate_custom_range("2008-12-31", "2024-01-01", TODAY),
            Err("The chain starts on 3 January 2009.")
        );
        assert_eq!(
            validate_custom_range("2030-01-01", "2030-02-01", TODAY),
            Err("That start date is in the future.")
        );
    }

    /// Genesis itself is inside the chain, not before it.
    #[test]
    fn the_first_day_of_the_chain_is_allowed() {
        assert!(
            validate_custom_range("2009-01-03", "2009-02-01", TODAY).is_ok()
        );
        assert!(
            validate_custom_range("2009-01-02", "2009-02-01", TODAY).is_err()
        );
    }

    /// A future end date is clamped rather than refused, because asking for
    /// the rest of the current month is a reasonable thing to type and the
    /// honest answer is everything up to now.
    #[test]
    fn a_future_end_is_clamped_to_today() {
        assert_eq!(
            validate_custom_range("2026-09-01", "2026-12-31", TODAY),
            Ok(("2026-09-01".to_string(), TODAY.to_string()))
        );
        // Today itself is not the future.
        assert_eq!(
            validate_custom_range(TODAY, TODAY, TODAY),
            Ok((TODAY.to_string(), TODAY.to_string()))
        );
    }

    /// String comparison is a chronological comparison only because the
    /// format is zero-padded. A test here so that reformatting the inputs
    /// cannot quietly break the ordering checks.
    #[test]
    fn dates_are_compared_chronologically() {
        assert_eq!(
            validate_custom_range("2024-09-01", "2024-10-01", TODAY),
            Ok(("2024-09-01".to_string(), "2024-10-01".to_string()))
        );
        // The case a non-padded format would get wrong: "9" > "10" as text.
        assert!(
            validate_custom_range("2024-10-01", "2024-09-01", TODAY).is_err()
        );
    }
}
