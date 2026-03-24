//! Reusable chart and stats UI components.

use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = setChartOption)]
    fn set_chart_option(id: &str, option_json: &str);
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = showBlockDetail)]
    pub fn show_block_detail(height: u64);
}

#[cfg(not(feature = "hydrate"))]
fn set_chart_option(_id: &str, _json: &str) {}

#[cfg(not(feature = "hydrate"))]
pub fn show_block_detail(_height: u64) {}

// ---------------------------------------------------------------------------
// Chart component
// ---------------------------------------------------------------------------

#[component]
pub fn Chart(
    #[prop(into)] id: String,
    #[prop(into)] option: Signal<String>,
    #[prop(optional, into)] class: Option<String>,
) -> impl IntoView {
    let id_clone = id.clone();
    Effect::new(move |_| {
        let json = option.get();
        if !json.is_empty() {
            set_chart_option(&id_clone, &json);
        }
    });

    let css_class =
        class.unwrap_or_else(|| "w-full h-[350px] lg:h-[600px]".to_string());

    view! {
        <div id=id class=css_class></div>
    }
}

// ---------------------------------------------------------------------------
// Chart card with expand toggle
// ---------------------------------------------------------------------------

#[component]
pub fn ChartCard(
    #[prop(into)] title: String,
    #[prop(into)] description: String,
    #[prop(into)] chart_id: String,
    #[prop(into)] option: Signal<String>,
    /// Optional content to render in the header right area (e.g. toggle button)
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    view! {
        // Normal inline card
        <div class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
            <div class="flex items-start justify-between mb-4">
                <div>
                    <h3 class="text-lg text-white font-semibold">{title.clone()}</h3>
                    <p class="text-sm text-white/50 mt-0.5">{description.clone()}</p>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    {children.map(|c| c())}
                    <button
                        class="text-white/30 hover:text-white/60 transition-colors cursor-pointer p-1"
                        title="Expand"
                        on:click=move |_| set_expanded.set(true)
                    >
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15"/>
                        </svg>
                    </button>
                </div>
            </div>
            <div class="h-[350px] lg:h-[600px] relative">
                <Chart id=chart_id.clone() option=option class="w-full h-full".to_string()/>
                {
                    // Only show spinner on initial load, not on range changes
                    let (has_loaded, set_has_loaded) = signal(false);
                    Effect::new(move |_| {
                        if !option.get().is_empty() {
                            set_has_loaded.set(true);
                        }
                    });
                    view! {
                        <Show when=move || !has_loaded.get()>
                            <div class="absolute inset-0 flex items-center justify-center">
                                <div class="flex items-center gap-2 text-white/30">
                                    <svg class="w-5 h-5 animate-spin" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    <span class="text-sm">"Loading chart data..."</span>
                                </div>
                            </div>
                        </Show>
                    }
                }
            </div>
        </div>

        // Fullscreen overlay when expanded
        <Show when=move || expanded.get()>
            <div
                class="fixed inset-0 flex flex-col pt-14 pb-4 px-4 lg:pt-16 lg:pb-6 lg:px-8 overflow-hidden"
                style="z-index: 9999; background: #0a1929"
            >
                // Header with close button
                <div class="flex items-center justify-between mb-3 shrink-0">
                    <div>
                        <h3 class="text-lg text-white font-semibold">{title.clone()}</h3>
                        <p class="text-sm text-white/50 mt-0.5">{description.clone()}</p>
                    </div>
                    <button
                        class="text-white/60 hover:text-white transition-colors cursor-pointer p-2.5 rounded-xl hover:bg-white/10 border border-white/10 hover:border-white/20"
                        title="Close (Esc)"
                        on:click=move |_| set_expanded.set(false)
                    >
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
                        </svg>
                    </button>
                </div>
                // Chart fills remaining space
                <div class="flex-1 min-h-0 bg-[#0a1a2e] border border-white/10 rounded-xl p-2">
                    <Chart id=format!("{}-fullscreen", chart_id) option=option class="w-full h-full".to_string()/>
                </div>
            </div>
        </Show>
    }
}

// ---------------------------------------------------------------------------
// Live stat card
// ---------------------------------------------------------------------------

#[component]
pub fn LiveCard(
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="bg-[#0d2137] border border-white/10 rounded-lg p-3 text-center">
            <div class="text-[0.65rem] text-white/50 uppercase tracking-widest mb-1">{label}</div>
            <div
                class="text-lg lg:text-xl text-[#f7931a] font-bold font-mono truncate"
                title=move || value.get()
            >{move || value.get()}</div>
        </div>
    }
}
