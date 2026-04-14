//! Range selector components for the Observatory.
//!
//! `RangeSelector` renders preset range buttons (1D-ALL + Custom) and a date picker.
//! `FloatingRangePicker` provides a floating button in the bottom-right corner.

use leptos::prelude::*;

use crate::routes::observatory::helpers::*;
use super::state::ObservatoryState;

/// Range selector bar (1D through ALL + YTD)
#[component]
pub fn RangeSelector() -> impl IntoView {
    let state = expect_context::<ObservatoryState>();
    let range = state.range;
    let set_range = state.set_range;
    let set_custom_from = state.set_custom_from;
    let set_custom_to = state.set_custom_to;

    let (picker_open, set_picker_open) = signal(false);
    let (local_from, set_local_from) = signal(String::new());
    let (local_to, set_local_to) = signal(String::new());

    let range_label = move || {
        let r = range.get();
        if r == "custom" {
            "custom range"
        } else {
            let n = range_to_blocks(&r);
            if n > 5_000 {
                "daily averages"
            } else {
                "per block"
            }
        }
    };

    let apply_custom = move |_| {
        let f = local_from.get();
        let t = local_to.get();
        if f.is_empty() || t.is_empty() {
            return;
        }
        // Validate: from <= to, not before genesis, not in the future
        if f.as_str() > t.as_str() {
            return;
        }
        if f.as_str() < "2009-01-03" {
            return;
        }
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let to_clamped = if t.as_str() > today.as_str() { today } else { t };
        set_custom_from.set(Some(f));
        set_custom_to.set(Some(to_clamped));
        set_range.set("custom".to_string());
        set_picker_open.set(false);
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
        </div>
    }
}

/// Floating range picker button in the bottom-right corner.
/// Opens a popover with the full range selector when clicked.
#[component]
pub fn FloatingRangePicker() -> impl IntoView {
    let (open, set_open) = signal(false);

    view! {
        <div style="z-index: 10000" class="fixed bottom-6 right-6">
            // Toggle button
            <button
                class="w-11 h-11 sm:w-14 sm:h-14 rounded-full bg-[#0d2137] border border-[#f7931a]/30 shadow-lg shadow-black/30 flex items-center justify-center cursor-pointer hover:border-[#f7931a]/60 hover:scale-105 active:scale-95 transition-all"
                on:click=move |_| set_open.update(|v| *v = !*v)
                title="Change time range"
            >
                <svg class="w-5 h-5 sm:w-6 sm:h-6 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"/>
                </svg>
            </button>
            // Popover
            <Show when=move || open.get()>
                <div class="absolute bottom-14 right-0 bg-[#0d2137] border border-white/10 rounded-2xl shadow-2xl shadow-black/50 p-3 min-w-[280px]">
                    <div class="flex items-center justify-between mb-2">
                        <span class="text-xs text-white/50 font-medium">"Time Range"</span>
                        <button
                            class="text-white/30 hover:text-white/60 cursor-pointer"
                            on:click=move |_| set_open.set(false)
                        >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                            </svg>
                        </button>
                    </div>
                    <RangeSelector/>
                </div>
            </Show>
        </div>
    }
}
