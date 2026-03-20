//! Guide selector - multi-step flow with slide animations.
//!
//! Step 1: Pick a level (Basic / Intermediate / Advanced)
//! Step 2: Pick a platform (Android / iOS / Desktop)
//! Step 3: If Desktop → Pick OS (Linux / macOS / Windows)

use leptos::prelude::*;
use leptos_meta::*;

use crate::guides;

fn platform_icon(platform: &str) -> &'static str {
    match platform {
        "android" => {
            r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M17.523 2.238l1.694-1.694a.5.5 0 00-.707-.707L16.6 1.747A7.96 7.96 0 0012 .5a7.96 7.96 0 00-4.6 1.247L5.49.838a.5.5 0 00-.707.707L6.477 2.24A7.96 7.96 0 004 8h16a7.96 7.96 0 00-2.477-5.762zM9 6a1 1 0 110-2 1 1 0 010 2zm6 0a1 1 0 110-2 1 1 0 010 2zM4 9v8a2 2 0 002 2h1v3.5a1.5 1.5 0 003 0V19h4v3.5a1.5 1.5 0 003 0V19h1a2 2 0 002-2V9H4zm-2.5 0A1.5 1.5 0 000 10.5v5a1.5 1.5 0 003 0v-5A1.5 1.5 0 001.5 9zm21 0a1.5 1.5 0 00-1.5 1.5v5a1.5 1.5 0 003 0v-5A1.5 1.5 0 0022.5 9z"/></svg>"#
        }
        "ios" => {
            r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>"#
        }
        "desktop" => {
            r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>"#
        }
        _ => "",
    }
}

fn os_icon(os: &str) -> &'static str {
    match os {
        "desktop-linux" => {
            r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2.025.134.063.198.114.333l.003.003c.391.778 1.113 1.132 1.884 1.071.771-.06 1.592-.536 2.257-1.306.631-.765 1.683-1.084 2.378-1.503.348-.199.629-.469.649-.853.023-.4-.2-.811-.714-1.376v-.097l-.003-.003c-.17-.2-.25-.535-.338-.926-.2-.868-.564-2.086-2.201-3.292-.533-.395-.934-.809-1.175-1.397-.13-.318-.164-.665-.238-1.078-.075-.415-.18-.876-.533-1.408v-.003l.003-.003c-.21-.27-.4-.47-.54-.672-.14-.199-.23-.335-.23-.535 0-.265.12-.534.27-.872.15-.334.33-.8.33-1.4 0-1.737-1.04-3.396-2.601-4.192C14.042.132 13.303 0 12.504 0zm-.107 1.2c.658 0 1.26.112 1.77.405 1.24.72 2.082 2.082 2.082 3.54 0 .4-.134.734-.27 1.066-.135.335-.3.674-.3 1.135 0 .4.17.669.324.887.155.22.33.4.53.668v.003c.3.468.38.87.45 1.27.07.4.13.8.305 1.2.3.535.749 1 1.325 1.47 1.424 1.065 1.722 2.068 1.904 2.87.091.4.155.736.373 1.004l.003.003v.003c.453.5.601.8.587 1.035-.013.2-.18.335-.45.503-.538.335-1.455.6-2.215 1.47-.596.7-1.345 1.135-2.005 1.201-.66.064-1.22-.198-1.564-.87a.824.824 0 01-.068-.2c-.023-.2.003-.467.04-.6h-.003c.027-.267.003-.535-.135-.87a1.14 1.14 0 00-.328-.432c-.6-.534-1.8-.468-2.578-.2-.8.2-1.6.066-2.457-.399-.933-.533-2.166-.403-2.878-.602-.348-.133-.535-.2-.618-.4-.083-.2-.035-.535.2-1.135.166-.4.066-.87.003-1.27a3.782 3.782 0 01-.05-.468c0-.134.024-.266.063-.4.533-1.527 1.69-3.1 2.505-4.067.8-1.133 1.075-2.134 1.158-3.335.09-1.267-.4-4.735 2.609-4.952.127-.013.26-.013.392-.013z"/></svg>"#
        }
        "desktop-macos" => {
            r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.8-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/></svg>"#
        }
        "desktop-windows" => {
            r#"<svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.9-1.801"/></svg>"#
        }
        _ => "",
    }
}

/// Step indicator pills.
#[component]
fn StepIndicator(step: Signal<u8>, total_steps: Signal<u8>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-1.5 mb-6">
            {move || {
                let total = total_steps.get();
                let current = step.get();
                (1..=total).map(|i| {
                    view! {
                        <div class=move || {
                            if i == current {
                                "w-8 h-1.5 rounded-full bg-[#f7931a] transition-all duration-300"
                            } else if i < current {
                                "w-4 h-1.5 rounded-full bg-[#f7931a] opacity-40 transition-all duration-300"
                            } else {
                                "w-4 h-1.5 rounded-full bg-white/15 transition-all duration-300"
                            }
                        }></div>
                    }
                }).collect::<Vec<_>>()
            }}
        </div>
    }
}

fn back_button_view(
    label: &'static str,
    on_click: impl Fn(leptos::ev::MouseEvent) + 'static,
) -> impl IntoView {
    view! {
        <button
            class="mt-6 inline-flex items-center gap-2 px-5 py-2.5 rounded-xl text-sm font-medium text-white/60 border border-white/10 hover:text-white hover:border-white/30 hover:bg-white/5 transition-all duration-200 cursor-pointer"
            on:click=on_click
        >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
            </svg>
            {label}
        </button>
    }
}

#[component]
pub fn GuideSelector() -> impl IntoView {
    let (selected_level, set_selected_level) = signal(None::<&'static str>);
    let (selected_platform, set_selected_platform) =
        signal(None::<&'static str>);

    // Step: 1 = level, 2 = platform, 3 = OS (desktop only)
    let step = Signal::derive(move || {
        if selected_platform.get().is_some() {
            3u8
        } else if selected_level.get().is_some() {
            2u8
        } else {
            1u8
        }
    });
    let total_steps = Signal::derive(move || {
        // 3 steps if a level with desktop is selected, otherwise 2
        if selected_platform.get() == Some("desktop") {
            3u8
        } else if selected_level.get().is_some() {
            let level = selected_level.get().and_then(guides::find_level);
            if level.is_some_and(|l| l.platforms.contains(&"desktop")) {
                3u8
            } else {
                2u8
            }
        } else {
            2u8
        }
    });

    view! {
        <Title text="Choose Your Guide | WE HODL BTC"/>
        <div class="flex flex-col items-center justify-center min-h-[70vh] px-6 opacity-0 animate-fadeinone">
            <StepIndicator step=step total_steps=total_steps/>

            <div class="w-full max-w-md lg:max-w-xl">
                {move || {
                    let level_sel = selected_level.get();
                    let platform_sel = selected_platform.get();

                    match (level_sel, platform_sel) {
                        // Step 3: Desktop → pick OS
                        (Some(lid), Some("desktop")) => {
                            view! {
                                <div class="flex flex-col items-center animate-slideup">
                                    <div class="text-[#f7931a] text-xs font-semibold uppercase tracking-widest mb-2">
                                        {guides::find_level(lid).map(|l| l.name).unwrap_or("")}
                                        " · Desktop"
                                    </div>
                                    <h1 class="text-white text-[1.8rem] text-center font-title md:text-[2.2rem] lg:text-[2.5rem] mb-2">
                                        "Select OS"
                                    </h1>
                                    <p class="text-white/60 text-[0.9rem] lg:text-base text-center mb-8">
                                        "Choose your operating system"
                                    </p>

                                    <div class="flex flex-col gap-3 w-full">
                                        {guides::DESKTOP_OS.iter().enumerate().map(|(i, (os_id, os_name))| {
                                            let path = format!("/guides/{}/{}", lid, os_id);
                                            let icon_html = os_icon(os_id);
                                            let delay = format!("animation-delay: {}ms", i * 80);
                                            view! {
                                                <a href=path class="block opacity-0 animate-slideup" style=delay>
                                                    <button class="group flex items-center gap-4 w-full px-5 py-5 lg:px-6 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer">
                                                        <div class="w-10 h-10 rounded-lg bg-[#f7931a]/10 flex items-center justify-center text-[#f7931a] group-hover:bg-[#f7931a]/20 transition-colors" inner_html=icon_html></div>
                                                        <div class="flex-1 text-left">
                                                            <div class="text-base lg:text-lg font-semibold text-white group-hover:text-[#f4a949] transition-colors">{*os_name}</div>
                                                        </div>
                                                        <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                                                        </svg>
                                                    </button>
                                                </a>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>

                                    {back_button_view("Change platform", move |_| set_selected_platform.set(None))}
                                </div>
                            }.into_any()
                        }

                        // Step 2: Pick a platform
                        (Some(lid), None) => {
                            match guides::find_level(lid) {
                                Some(level) => {
                                    view! {
                                        <div class="flex flex-col items-center animate-slideup">
                                            <div class="text-[#f7931a] text-xs font-semibold uppercase tracking-widest mb-2">
                                                {level.name}
                                            </div>
                                            <h1 class="text-white text-[1.8rem] text-center font-title md:text-[2.2rem] lg:text-[2.5rem] mb-2">
                                                "Select Platform"
                                            </h1>
                                            <p class="text-white/60 text-[0.9rem] lg:text-base text-center mb-8">
                                                "Choose your device to get started"
                                            </p>

                                            <div class="flex flex-col gap-3 w-full">
                                                {level.platforms.iter().enumerate().map(|(i, platform)| {
                                                    let display = guides::platform_display(platform);
                                                    let icon_html = platform_icon(platform);
                                                    let delay = format!("animation-delay: {}ms", i * 80);
                                                    let platform_str = *platform;

                                                    if platform_str == "desktop" {
                                                        // Desktop → go to OS sub-step instead of navigating
                                                        view! {
                                                            <div class="opacity-0 animate-slideup" style=delay>
                                                                <button
                                                                    class="group flex items-center gap-4 w-full px-5 py-5 lg:px-6 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer"
                                                                    on:click=move |_| set_selected_platform.set(Some("desktop"))
                                                                >
                                                                    <div class="w-10 h-10 rounded-lg bg-[#f7931a]/10 flex items-center justify-center text-[#f7931a] group-hover:bg-[#f7931a]/20 transition-colors" inner_html=icon_html></div>
                                                                    <div class="flex-1 text-left">
                                                                        <div class="text-base lg:text-lg font-semibold text-white group-hover:text-[#f4a949] transition-colors">{display}</div>
                                                                    </div>
                                                                    <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                                                                    </svg>
                                                                </button>
                                                            </div>
                                                        }.into_any()
                                                    } else {
                                                        // Mobile platforms → navigate directly
                                                        let path = format!("/guides/{}/{}", lid, platform_str);
                                                        view! {
                                                            <a href=path class="block opacity-0 animate-slideup" style=delay>
                                                                <button class="group flex items-center gap-4 w-full px-5 py-5 lg:px-6 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer">
                                                                    <div class="w-10 h-10 rounded-lg bg-[#f7931a]/10 flex items-center justify-center text-[#f7931a] group-hover:bg-[#f7931a]/20 transition-colors" inner_html=icon_html></div>
                                                                    <div class="flex-1 text-left">
                                                                        <div class="text-base lg:text-lg font-semibold text-white group-hover:text-[#f4a949] transition-colors">{display}</div>
                                                                    </div>
                                                                    <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                                                                    </svg>
                                                                </button>
                                                            </a>
                                                        }.into_any()
                                                    }
                                                }).collect::<Vec<_>>()}
                                            </div>

                                            {back_button_view("Change level", move |_| set_selected_level.set(None))}
                                        </div>
                                    }.into_any()
                                }
                                None => view! { <p class="text-white text-center">"Level not found."</p> }.into_any()
                            }
                        }

                        // Step 1: Pick a level
                        (None, _) => {
                            view! {
                                <div class="flex flex-col items-center animate-scaleup">
                                    <h1 class="text-white text-[1.8rem] text-center font-title md:text-[2.2rem] lg:text-[2.5rem] mb-2">
                                        "Choose Your Level"
                                    </h1>
                                    <p class="text-white/60 text-[0.9rem] lg:text-base text-center mb-8">
                                        "Based on how much Bitcoin you are protecting"
                                    </p>

                                    <div class="flex flex-col gap-3 w-full">
                                        {guides::ALL_LEVELS.iter().enumerate().map(|(i, level)| {
                                            let level_id = level.id;
                                            let delay = format!("animation-delay: {}ms", i * 80);
                                            view! {
                                                <button
                                                    class="opacity-0 animate-slideup group flex items-center w-full px-5 py-5 lg:px-6 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer"
                                                    style=delay
                                                    on:click=move |_| set_selected_level.set(Some(level_id))
                                                >
                                                    <div class="flex-1 text-left">
                                                        <div class="text-lg lg:text-xl font-semibold text-[#f7931a] group-hover:text-[#f4a949] transition-colors">{level.name}</div>
                                                        <p class="text-sm lg:text-base text-white/50 mt-0.5">{level.subtitle}</p>
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

                        _ => view! { <div></div> }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
