use crate::extras::buttons::GenericButton;
use crate::guides;
use leptos::prelude::*;
use leptos_meta::*;

/// Data-driven guide selector — pick a level and then a platform.
#[component]
pub fn GuideSelector() -> impl IntoView {
    let (selected_level, set_selected_level) = signal(None::<&'static str>);

    let explainer = move || {
        if selected_level.get().is_some() {
            "Select your preferred platform".to_string()
        } else {
            "Select a guide based on how much Bitcoin you are protecting.".to_string()
        }
    };

    view! {
        <Title text="Guides | WE HODL BTC"/>

        <div class="grid gap-4 md:gap-2 mx-auto justify-items-center max-w-3xl mt-14 mb-24 opacity-0 animate-fadeinone md:grid-cols-1 lg:grid-cols-2 xl:grid-cols-2 md:max-w-4xl lg:max-w-5xl md:my-28">
            <div class="flex flex-col justify-center items-center">
                <div>
                    <h1 class="text-white text-4xl text-center pb-1 md:text-5xl 2xl:text-5xl font-title">
                        "Unlock Financial Privacy"
                    </h1>
                </div>
                <img
                    class="w-36 h-auto py-4 2xl:w-44"
                    src="./../../../only_lock.png"
                    alt="Financial privacy lock"
                />
                <div class="px-6 pt-2 max-w-3xl">
                    <p class="text-white text-base text-center pb-2 md:text-lg">
                        {explainer}
                    </p>
                </div>
            </div>
            <div class="flex flex-col gap-2">
                {move || {
                    match selected_level.get() {
                        None => {
                            // Show level buttons
                            guides::ALL_LEVELS.iter().map(|level| {
                                let level_id = level.id;
                                let name = level.name.to_string();
                                let subtitle = level.subtitle.to_string();
                                view! {
                                    <button
                                        class="flex flex-col z-20 p-3 max-w-md mx-auto w-64 bg-white rounded-lg items-center mt-4 shadow-md hover:bg-[#f2f2f2] transition ease-in-out duration-300"
                                        on:click=move |_| set_selected_level.set(Some(level_id))
                                    >
                                        <div class="text-xl font-bold text-[#f79231]">{name.clone()}</div>
                                        <p class="text-sm text-[#123c64] mt-2">{subtitle.clone()}</p>
                                    </button>
                                }.into_any()
                            }).collect::<Vec<_>>()
                        }
                        Some(lid) => {
                            // Show platform buttons for selected level
                            match guides::find_level(lid) {
                                Some(level) => {
                                    let mut views: Vec<AnyView> = level.platforms.iter().map(|platform| {
                                        let display = match *platform {
                                            "android" => "Android",
                                            "ios" => "iOS",
                                            "desktop" => "Desktop",
                                            other => other,
                                        };
                                        let path = format!("/guides/{}/{}", lid, platform);
                                        view! {
                                            <div class="animate-fadeinone">
                                                <GenericButton
                                                    path=path
                                                    wallet_title=display.to_string()
                                                    text_color="#f79231".to_string()
                                                />
                                            </div>
                                        }.into_any()
                                    }).collect();
                                    views.push(view! {
                                        <div class="mt-4 flex justify-center">
                                            <button
                                                class="text-sm text-white underline opacity-60 hover:opacity-100"
                                                on:click=move |_| set_selected_level.set(None)
                                            >
                                                "Back to levels"
                                            </button>
                                        </div>
                                    }.into_any());
                                    views
                                }
                                None => vec![view! { <p class="text-white">"Level not found."</p> }.into_any()]
                            }
                        }
                    }
                }}
            </div>
        </div>
    }
}
