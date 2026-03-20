use leptos::prelude::*;
use leptos_meta::*;

use crate::guides::{self, GuideLevelDef};

#[component]
fn LevelButton(level: &'static GuideLevelDef, hidden: Signal<bool>) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);

    let on_click = move |_| {
        set_expanded.update(|v| *v = !*v);
    };

    view! {
        <div class:hidden=move || hidden.get() && !expanded.get()>
            <Show
                when=move || expanded.get()
                fallback=move || {
                    view! {
                        <button
                            class="flex flex-col w-72 px-6 py-5 mx-auto bg-white/95 border border-white/20 rounded-xl items-center mt-5 hover:bg-white hover:-translate-y-1 hover:shadow-lg active:translate-y-0 transition-all duration-200 cursor-pointer"
                            on:click=on_click
                        >
                            <div class="text-xl font-bold text-[#f79231]">{level.name}</div>
                            <p class="text-sm text-[#123c64] mt-1.5">{level.subtitle}</p>
                        </button>
                    }.into_any()
                }
            >
                <div class="flex flex-col items-center py-5 gap-3 animate-fadeinone">
                    {level.platforms.iter().map(|platform| {
                        let display = match *platform {
                            "android" => "Android",
                            "ios" => "iOS",
                            "desktop" => "Desktop",
                            p => p,
                        };
                        let path = format!("/guides/{}/{}", level.id, platform);
                        view! {
                            <a href=path class="block">
                                <button class="w-56 px-5 py-3 bg-white/95 border border-white/20 rounded-xl text-base font-semibold text-[#f79231] hover:bg-white hover:-translate-y-0.5 hover:shadow-md active:translate-y-0 transition-all duration-200 cursor-pointer">
                                    {display}
                                </button>
                            </a>
                        }
                    }).collect::<Vec<_>>()}
                </div>
                <div class="mt-4 flex justify-center">
                    <button
                        class="inline-flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium text-white/70 border border-white/15 hover:text-white hover:border-white/40 hover:bg-white/5 transition-all duration-200 cursor-pointer"
                        on:click=move |_| set_expanded.set(false)
                    >
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                        </svg>
                        "Back"
                    </button>
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn GuideSelector() -> impl IntoView {
    let (selected, set_selected) = signal(None::<&'static str>);

    let explainer = move || {
        if selected.get().is_some() {
            "Select your preferred OS".to_string()
        } else {
            "Select a guide based on how much Bitcoin you are protecting.".to_string()
        }
    };

    view! {
        <Title text="Choose Your Guide | WE HODL BTC"/>
        <div class="grid gap-4 md:gap-2 mx-auto justify-items-center max-w-3xl mt-16 mb-24 px-6 opacity-0 animate-fadeinone md:grid-cols-1 lg:grid-cols-2 xl:grid-cols-2 md:max-w-4xl lg:max-w-5xl md:my-24 lg:px-8">
            <div class="flex flex-col justify-center items-center">
                <h1 class="text-white text-[2rem] text-center pb-1 2xl:text-[2.5rem]">"Unlock Financial Privacy"</h1>
                <img class="w-36 h-auto py-4 2xl:w-44" src="/only_lock.png" alt="Financial privacy lock"/>
                <div class="px-6 pt-2 max-w-3xl">
                    <p class="text-white text-[0.95rem] text-center pb-2 2xl:text-base leading-relaxed">{explainer}</p>
                </div>
            </div>
            <div class="flex flex-col gap-2">
                {guides::ALL_LEVELS.iter().map(|level| {
                    let level_id = level.id;
                    let is_hidden = Signal::derive(move || {
                        selected.get().is_some_and(|s| s != level_id)
                    });

                    view! {
                        <div
                            on:click=move |_| {
                                if selected.get() == Some(level_id) {
                                    set_selected.set(None);
                                } else {
                                    set_selected.set(Some(level_id));
                                }
                            }
                        >
                            <LevelButton level=level hidden=is_hidden/>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}
