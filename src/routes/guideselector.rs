//! Guide selector — two-step flow with slide animations.
//!
//! Step 1: Pick a level (Basic / Intermediate / Advanced)
//! Step 2: Pick a platform (Android / iOS / Desktop)
//! Each transition slides content in/out smoothly.

use leptos::prelude::*;
use leptos_meta::*;

use crate::guides;

/// Platform icon (inline SVG) for the platform buttons.
fn platform_icon(platform: &str) -> &'static str {
    match platform {
        "android" => r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M17.523 2.238l1.694-1.694a.5.5 0 00-.707-.707L16.6 1.747A7.96 7.96 0 0012 .5a7.96 7.96 0 00-4.6 1.247L5.49.838a.5.5 0 00-.707.707L6.477 2.24A7.96 7.96 0 004 8h16a7.96 7.96 0 00-2.477-5.762zM9 6a1 1 0 110-2 1 1 0 010 2zm6 0a1 1 0 110-2 1 1 0 010 2zM4 9v8a2 2 0 002 2h1v3.5a1.5 1.5 0 003 0V19h4v3.5a1.5 1.5 0 003 0V19h1a2 2 0 002-2V9H4zm-2.5 0A1.5 1.5 0 000 10.5v5a1.5 1.5 0 003 0v-5A1.5 1.5 0 001.5 9zm21 0a1.5 1.5 0 00-1.5 1.5v5a1.5 1.5 0 003 0v-5A1.5 1.5 0 0022.5 9z"/></svg>"#,
        "ios" => r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>"#,
        "desktop" => r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>"#,
        _ => "",
    }
}

/// Step indicator dots at the top.
#[component]
fn StepIndicator(step: Signal<u8>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 mb-6">
            <div class=move || {
                if step.get() == 1 {
                    "w-8 h-1.5 rounded-full bg-[#f7931a] transition-all duration-300"
                } else {
                    "w-4 h-1.5 rounded-full bg-white/20 transition-all duration-300"
                }
            }></div>
            <div class=move || {
                if step.get() == 2 {
                    "w-8 h-1.5 rounded-full bg-[#f7931a] transition-all duration-300"
                } else {
                    "w-4 h-1.5 rounded-full bg-white/20 transition-all duration-300"
                }
            }></div>
        </div>
    }
}

#[component]
pub fn GuideSelector() -> impl IntoView {
    let (selected_level, set_selected_level) = signal(None::<&'static str>);
    let step = Signal::derive(move || if selected_level.get().is_some() { 2u8 } else { 1u8 });

    view! {
        <Title text="Choose Your Guide | WE HODL BTC"/>
        <div class="flex flex-col items-center justify-center min-h-[70vh] px-6 opacity-0 animate-fadeinone">

            // Step indicator
            <StepIndicator step=step/>

            // Content area
            <div class="w-full max-w-md lg:max-w-lg">
                {move || {
                    match selected_level.get() {
                        None => {
                            // Step 1: Pick a level
                            view! {
                                <div class="flex flex-col items-center animate-scaleup">
                                    <h1 class="text-white text-[1.8rem] text-center font-title lg:text-[2.2rem] mb-2 md:text-[2.2rem]">
                                        "Choose Your Level"
                                    </h1>
                                    <p class="text-white/60 text-[0.9rem] text-center mb-8">
                                        "Based on how much Bitcoin you are protecting"
                                    </p>

                                    <div class="flex flex-col gap-3 w-full">
                                        {guides::ALL_LEVELS.iter().enumerate().map(|(i, level)| {
                                            let level_id = level.id;
                                            let delay = format!("animation-delay: {}ms", i * 80);
                                            view! {
                                                <button
                                                    class="opacity-0 animate-slideup group flex items-center w-full px-5 py-4 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer"
                                                    style=delay
                                                    on:click=move |_| set_selected_level.set(Some(level_id))
                                                >
                                                    <div class="flex-1 text-left">
                                                        <div class="text-lg font-semibold text-[#f7931a] group-hover:text-[#f4a949] transition-colors">{level.name}</div>
                                                        <p class="text-sm text-white/50 mt-0.5">{level.subtitle}</p>
                                                    </div>
                                                    <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                                                    </svg>
                                                </button>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            }.into_any()
                        }
                        Some(lid) => {
                            // Step 2: Pick a platform
                            match guides::find_level(lid) {
                                Some(level) => {
                                    view! {
                                        <div class="flex flex-col items-center animate-slideup">
                                            <div class="text-[#f7931a] text-xs font-semibold uppercase tracking-widest mb-2">
                                                {level.name}
                                            </div>
                                            <h1 class="text-white text-[1.8rem] text-center font-title lg:text-[2.2rem] mb-2 md:text-[2.2rem]">
                                                "Select Platform"
                                            </h1>
                                            <p class="text-white/60 text-[0.9rem] text-center mb-8">
                                                "Choose your device to get started"
                                            </p>

                                            <div class="flex flex-col gap-3 w-full">
                                                {level.platforms.iter().enumerate().map(|(i, platform)| {
                                                    let display = match *platform {
                                                        "android" => "Android",
                                                        "ios" => "iOS",
                                                        "desktop" => "Desktop",
                                                        p => p,
                                                    };
                                                    let path = format!("/guides/{}/{}", lid, platform);
                                                    let icon_html = platform_icon(platform);
                                                    let delay = format!("animation-delay: {}ms", i * 80);
                                                    view! {
                                                        <a href=path class="block opacity-0 animate-slideup" style=delay>
                                                            <button class="group flex items-center gap-4 w-full px-5 py-4 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer">
                                                                <div class="w-10 h-10 rounded-lg bg-[#f7931a]/10 flex items-center justify-center text-[#f7931a] group-hover:bg-[#f7931a]/20 transition-colors" inner_html=icon_html></div>
                                                                <div class="flex-1 text-left">
                                                                    <div class="text-base font-semibold text-white group-hover:text-[#f4a949] transition-colors">{display}</div>
                                                                </div>
                                                                <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                                                                </svg>
                                                            </button>
                                                        </a>
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>

                                            // Back button
                                            <button
                                                class="mt-6 inline-flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium text-white/60 border border-white/10 hover:text-white hover:border-white/30 hover:bg-white/5 transition-all duration-200 cursor-pointer"
                                                on:click=move |_| set_selected_level.set(None)
                                            >
                                                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                                                </svg>
                                                "Change level"
                                            </button>
                                        </div>
                                    }.into_any()
                                }
                                None => view! { <p class="text-white text-center">"Level not found."</p> }.into_any()
                            }
                        }
                    }
                }}
            </div>
        </div>
    }
}
