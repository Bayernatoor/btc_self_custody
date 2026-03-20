//! Shared button components used across guide and product pages.

use leptos::prelude::*;

/// Internal link button with optional icon and description.
#[component]
#[allow(non_snake_case)]
pub fn GenericButton(
    path: String,
    wallet_title: String,
    #[prop(optional)] short_desc: String,
    #[prop(optional)] img_url: String,
    #[prop(optional)] img_alt: String,
    #[prop(optional)] text_color: String,
    #[prop(default = "8".to_string())] new_height: String,
    #[prop(default = "8".to_string())] new_width: String,
) -> impl IntoView {
    let has_img = !img_url.is_empty();
    let color = if text_color.is_empty() {
        "#123c64".to_string()
    } else {
        text_color
    };

    view! {
        <a href=path class="block">
            <button class="flex items-center gap-3 w-full max-w-xs px-4 py-2.5 mx-auto bg-white/95 border border-white/20 rounded-lg hover:bg-white hover:-translate-y-0.5 hover:shadow-md active:translate-y-0 transition-all duration-200">
                {has_img.then(|| view! {
                    <img
                        class=format!("h-{} w-{} shrink-0 object-contain", new_height, new_width)
                        src=img_url.clone()
                        alt=img_alt.clone()
                    />
                })}
                <div class="text-left">
                    <span class="text-sm font-medium" style=format!("color: {color}")>{wallet_title}</span>
                    {(!short_desc.is_empty()).then(|| view! {
                        <p class="text-xs text-slate-500 mt-0.5">{short_desc.clone()}</p>
                    })}
                </div>
            </button>
        </a>
    }
}

/// External link button with optional icon and description.
#[component]
#[allow(non_snake_case)]
pub fn GenericExternalButton(
    path: String,
    wallet_title: String,
    #[prop(optional)] short_desc: String,
    #[prop(optional)] img_url: String,
    #[prop(optional)] img_alt: String,
    #[prop(optional)] text_color: String,
    #[prop(default = "8".to_string())] new_height: String,
    #[prop(default = "8".to_string())] new_width: String,
) -> impl IntoView {
    let has_img = !img_url.is_empty();
    let color = if text_color.is_empty() {
        "#123c64".to_string()
    } else {
        text_color
    };

    view! {
        <a href=path rel="noreferrer" target="_blank" class="block">
            <button class="flex flex-col items-center gap-1.5 w-full max-w-xs px-4 py-3 mx-auto bg-white/95 border border-white/20 rounded-lg hover:bg-white hover:-translate-y-0.5 hover:shadow-md active:translate-y-0 transition-all duration-200">
                {has_img.then(|| view! {
                    <img
                        class=format!("h-{} w-{} shrink-0 object-contain", new_height, new_width)
                        src=img_url.clone()
                        alt=img_alt.clone()
                    />
                })}
                <span class="text-xs font-medium" style=format!("color: {color}")>{wallet_title}</span>
                {(!short_desc.is_empty()).then(|| view! {
                    <p class="text-xs text-slate-500">{short_desc.clone()}</p>
                })}
            </button>
        </a>
    }
}
