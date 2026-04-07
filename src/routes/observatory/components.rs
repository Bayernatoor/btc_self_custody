//! Reusable chart and stats UI components.

use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = setChartOption)]
    fn set_chart_option(id: &str, option_json: &str);

    #[wasm_bindgen(js_name = setChartOptionLazy)]
    fn set_chart_option_lazy(id: &str, option_json: &str);
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = showBlockDetail)]
    pub fn show_block_detail(height: u64);

    #[wasm_bindgen(js_name = downloadChartCSV)]
    fn download_chart_csv(chart_id: &str, title: &str, range: &str);
}

#[cfg(not(feature = "hydrate"))]
#[allow(dead_code)]
fn set_chart_option(_id: &str, _json: &str) {}

#[cfg(not(feature = "hydrate"))]
fn set_chart_option_lazy(_id: &str, _json: &str) {}

#[cfg(not(feature = "hydrate"))]
pub fn show_block_detail(_height: u64) {}

#[cfg(not(feature = "hydrate"))]
fn download_chart_csv(_id: &str, _title: &str, _range: &str) {}

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
    let last_json = std::cell::RefCell::new(String::new());
    Effect::new(move |_| {
        let json = option.get();
        if !json.is_empty() && *last_json.borrow() != json {
            *last_json.borrow_mut() = json.clone();
            set_chart_option_lazy(&id_clone, &json);
        }
    });

    let css_class =
        class.unwrap_or_else(|| "w-full h-[350px] lg:h-[600px]".to_string());

    view! {
        <div id=id class=css_class></div>
    }
}

// ---------------------------------------------------------------------------
// Data load error with retry
// ---------------------------------------------------------------------------

/// Shared error state shown when a resource fetch fails.
/// Displays "Failed to load data" with a retry button.
#[component]
pub fn DataLoadError(
    #[prop(into)] on_retry: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="flex flex-col items-center justify-center min-h-[200px] gap-4">
            <p class="text-white/50 font-mono text-sm">"Failed to load data"</p>
            <button
                class="px-4 py-2 bg-white/10 hover:bg-white/20 text-white/70 rounded-lg font-mono text-sm cursor-pointer"
                on:click=move |_| { on_retry.run(()); }
            >"Retry"</button>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Chart card with expand toggle
// ---------------------------------------------------------------------------

#[allow(unused_variables)] // share button vars only used in hydrate feature
#[component]
pub fn ChartCard(
    #[prop(into)] title: String,
    #[prop(into)] description: Signal<String>,
    #[prop(into)] chart_id: String,
    #[prop(into)] option: Signal<String>,
    /// Optional content to render in the header right area (e.g. toggle button)
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    let anchor = chart_id.clone();
    let share_id = chart_id.clone();
    let download_id = chart_id.clone();
    let download_title = title.clone();
    let (copied, set_copied) = signal(false);
    let state = expect_context::<super::shared::ObservatoryState>();
    let loading = state.data_loading;
    let range = state.range;
    view! {
        // Normal inline card
        <div id=format!("card-{}", chart_id) class="bg-[#0d2137] border border-white/10 rounded-2xl p-5 lg:p-6">
            <div class="flex items-start justify-between mb-4">
                <div>
                    <h3
                        class="text-lg text-white font-semibold cursor-pointer hover:text-[#f7931a] transition-colors"
                        title="Click to copy link"
                        on:click={
                            let id = anchor.clone();
                            move |_| {
                                #[cfg(feature = "hydrate")]
                                {
                                    let url = super::shared::build_share_url(&id);
                                    let _ = leptos::prelude::window().navigator().clipboard().write_text(&url);
                                    set_copied.set(true);
                                    leptos::prelude::set_timeout(move || set_copied.set(false), std::time::Duration::from_secs(2));
                                }
                            }
                        }
                    >
                        {title.clone()}
                        " "
                        <span class="text-white/20 text-xs font-normal">{move || if copied.get() { "\u{2713} copied" } else { "#" }}</span>
                    </h3>
                    <p class="text-sm text-white/50 mt-0.5">{move || description.get()}</p>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                    {children.map(|c| c())}
                    <button
                        class="text-white/50 hover:text-[#f7931a] transition-colors cursor-pointer p-1.5 rounded-lg hover:bg-white/5"
                        title="Download CSV"
                        on:click={
                            let id = download_id.clone();
                            let t = download_title.clone();
                            move |_| {
                                download_chart_csv(&id, &t, &range.get_untracked());
                            }
                        }
                    >
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3"/>
                        </svg>
                    </button>
                    <button
                        class="text-white/50 hover:text-[#f7931a] transition-colors cursor-pointer p-1.5 rounded-lg hover:bg-white/5"
                        title="Copy link to chart"
                        on:click={
                            let _id = share_id.clone();
                            move |_| {
                                #[cfg(feature = "hydrate")]
                                {
                                    let url = super::shared::build_share_url(&_id);
                                    let _ = leptos::prelude::window().navigator().clipboard().write_text(&url);
                                    set_copied.set(true);
                                    leptos::prelude::set_timeout(move || set_copied.set(false), std::time::Duration::from_secs(2));
                                }
                            }
                        }
                    >
                        {move || if copied.get() {
                            view! {
                                <svg class="w-5 h-5 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5"/>
                                </svg>
                            }.into_any()
                        } else {
                            view! {
                                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="1.5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M13.19 8.688a4.5 4.5 0 0 1 1.242 7.244l-4.5 4.5a4.5 4.5 0 0 1-6.364-6.364l1.757-1.757m9.364-9.364a4.5 4.5 0 0 1 6.364 6.364l-1.757 1.757m-7.07 7.07 4.243-4.243"/>
                                </svg>
                            }.into_any()
                        }}
                    </button>
                    <button
                        class="text-white/50 hover:text-[#f7931a] transition-colors cursor-pointer p-1.5 rounded-lg hover:bg-white/5"
                        title="Expand"
                        on:click=move |_| set_expanded.set(true)
                    >
                        <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15"/>
                        </svg>
                    </button>
                </div>
            </div>
            <div class="h-[350px] lg:h-[600px] relative">
                <Chart id=chart_id.clone() option=option class="w-full h-full".to_string()/>
                // Show loading skeleton when chart data is empty or range is transitioning
                <Show when=move || option.get().is_empty() || loading.get()>
                    <div class="absolute inset-0 flex items-center justify-center bg-[#0d2137] rounded-2xl">
                        <div class="flex flex-col items-center gap-3">
                            <div class="animate-pulse">
                                <div class="w-12 h-12 rounded-lg bg-[#f7931a]/10 border border-[#f7931a]/20 flex items-center justify-center">
                                    <svg class="w-6 h-6 text-[#f7931a]/40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                                        <rect x="3" y="3" width="18" height="18" rx="2"/>
                                        <path d="M9 3v18M15 3v18M3 9h18M3 15h18"/>
                                    </svg>
                                </div>
                            </div>
                            <span class="text-xs text-white/30">"Mining blocks..."</span>
                        </div>
                    </div>
                </Show>
            </div>
        </div>

        // Fullscreen overlay when expanded
        <Show when=move || expanded.get()>
            // Close on Escape key
            {
                use leptos::ev::keydown;
                let handle = leptos::prelude::window_event_listener(keydown, move |ev| {
                    if ev.key() == "Escape" {
                        set_expanded.set(false);
                    }
                });
                leptos::prelude::on_cleanup(move || handle.remove());
            }
            <div
                class="fixed inset-0 flex flex-col pt-14 pb-4 px-2 sm:px-4 lg:pt-16 lg:pb-6 lg:px-8 overflow-hidden"
                style="z-index: 9999; background: #0a1929; max-width: 100vw;"
            >
                // Header with close button
                <div class="flex items-center justify-between mb-2 sm:mb-3 shrink-0">
                    <div class="min-w-0 mr-2">
                        <h3 class="text-sm sm:text-lg text-white font-semibold truncate">{title.clone()}</h3>
                        <p class="text-xs sm:text-sm text-white/50 mt-0.5 truncate">{move || description.get()}</p>
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
    #[prop(optional, into)] tooltip: Option<&'static str>,
) -> impl IntoView {
    let is_loading = Signal::derive(move || value.get() == "\u{2014}");
    view! {
        <div
            class="bg-[#0d2137] border border-white/10 rounded-lg p-2 sm:p-3 text-center overflow-hidden min-w-0"
            data-tip=tooltip.unwrap_or("")
            tabindex=if tooltip.is_some() { "0" } else { "-1" }
        >
            <div class="text-[0.6rem] sm:text-[0.7rem] text-[#8899aa] uppercase tracking-widest mb-1">{label}</div>
            <div
                class="text-xs sm:text-lg lg:text-xl font-bold font-mono truncate"
                title=move || value.get()
            >
                {move || if is_loading.get() {
                    view! {
                        <span class="inline-flex items-center gap-[3px] text-[#f7931a]/30">
                            <span class="inline-block w-1.5 h-3 sm:w-2 sm:h-4 rounded-sm bg-[#f7931a]/30 animate-block-stack" style="animation-delay: 0s"></span>
                            <span class="inline-block w-1.5 h-3 sm:w-2 sm:h-4 rounded-sm bg-[#f7931a]/30 animate-block-stack" style="animation-delay: 0.2s"></span>
                            <span class="inline-block w-1.5 h-3 sm:w-2 sm:h-4 rounded-sm bg-[#f7931a]/30 animate-block-stack" style="animation-delay: 0.4s"></span>
                        </span>
                    }.into_any()
                } else {
                    view! { <span class="text-[#f7931a]">{value.get()}</span> }.into_any()
                }}
            </div>
        </div>
    }
}
