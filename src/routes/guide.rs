//! Unified guide pages — replaces beginner.rs, intermediate.rs, advanced.rs.
//!
//! Two route components:
//! - GuideTwoSegment: /guides/:level/:segment — dispatches to level page or step page
//! - GuideWalletPage: /guides/:level/:platform/:wallet — wallet-specific stepper

use leptos::prelude::*;
use leptos_meta::*;

use crate::extras::buttons::GenericExternalButton;
use crate::extras::stepper::Stepper;
use crate::guides::{self, DownloadLink, GuideLevelDef, ProductLink, WalletDef};

// =============================================================================
// Shared sub-components
// =============================================================================

#[component]
fn PageHeader(
    title: String,
    #[prop(optional)] quote: String,
    #[prop(optional)] quote_author: String,
) -> impl IntoView {
    view! {
        <header class="flex flex-col mx-auto px-4 pt-10 lg:pt-0">
            <h1 class="text-center text-[1.65rem] text-[#f7931a] font-semibold leading-tight md:text-[2rem] lg:text-[2.5rem]">
                {title}
            </h1>
            {(!quote.is_empty()).then(|| view! {
                <div class="text-center max-w-sm mx-auto pt-4">
                    <p class="text-lg font-semibold text-white italic">{quote}</p>
                </div>
            })}
            {(!quote_author.is_empty()).then(|| view! {
                <div class="text-center max-w-sm mx-auto">
                    <p class="text-md text-white italic">{quote_author}</p>
                </div>
            })}
        </header>
    }
}

#[component]
fn DownloadButton(download: &'static DownloadLink) -> impl IntoView {
    view! {
        <a href=download.url rel="noreferrer" target="_blank" class="block">
            <button class="flex justify-center items-center w-56 px-3 py-2 mx-auto bg-white/95 border border-white/20 rounded-lg hover:bg-white hover:-translate-y-0.5 hover:shadow-md active:translate-y-0 transition-all duration-200">
                <img class="h-8 max-w-full object-contain" src=download.logo alt=download.logo_alt/>
            </button>
        </a>
    }
}

#[component]
fn WalletCard(wallet: &'static WalletDef, platform: String, level: String) -> impl IntoView {
    let path = format!("/guides/{}/{}/{}", level, platform, wallet.id);
    let color = wallet.color;
    view! {
        <a href=path class="block">
            <button class="flex items-center gap-3 w-full max-w-sm px-5 py-3.5 mx-auto bg-white/95 border border-white/20 rounded-xl hover:bg-white hover:-translate-y-0.5 hover:shadow-lg active:translate-y-0 transition-all duration-200 cursor-pointer">
                <img class="h-10 w-10 rounded-md shrink-0" src=wallet.logo alt=wallet.logo_alt/>
                <div class="text-left">
                    <h3 class="text-base font-semibold" style=format!("color: {color}")>{wallet.name}</h3>
                    <p class="text-sm text-slate-500">{wallet.tagline}</p>
                </div>
            </button>
        </a>
    }
}

#[component]
fn ProductButtons(products: &'static [ProductLink]) -> impl IntoView {
    view! {
        <div class="flex flex-col md:flex-row justify-center gap-4">
            {products.iter().map(|p| {
                view! {
                    <div class="flex justify-center">
                        <GenericExternalButton
                            path=p.url.to_string()
                            wallet_title=p.name.to_string()
                            img_url=p.logo.to_string()
                            img_alt=p.logo_alt.to_string()
                            new_width=p.logo_width.to_string()
                            new_height=p.logo_height.to_string()
                        />
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}

fn page_layout() -> &'static str {
    "grid gap-6 max-w-3xl mx-auto mt-8 mb-24 px-6 animate-fadeinone grid-rows-[auto_auto_1fr] md:max-w-4xl lg:max-w-5xl lg:gap-8 lg:px-8 md:my-28"
}

// =============================================================================
// Route: /guides/:level/:segment — dispatches to level page or step page
// =============================================================================

const PLATFORMS: &[&str] = &["android", "ios", "desktop"];

/// Handles /guides/:level/:segment where segment is either a platform (android/ios/desktop)
/// or a step ID (hardware-wallet/node). Dispatches accordingly.
#[component]
pub fn GuideTwoSegment() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();

    let level_id = move || params.read().get("level");
    let segment = move || params.read().get("segment");

    view! {
        {move || {
            let level_id = level_id();
            let segment = segment();

            match (level_id.as_deref(), segment.as_deref()) {
                (Some(lid), Some(seg)) => {
                    match guides::find_level(lid) {
                        Some(level) => {
                            if PLATFORMS.contains(&seg) {
                                // It's a platform → render level intro page
                                render_level_page(level, seg).into_any()
                            } else {
                                // It's a step ID → render step page
                                match level.steps.iter().find(|s| s.id == seg) {
                                    Some(step) => render_step_page(step, lid).into_any(),
                                    None => view! { <p class="text-white text-center p-8">"Step not found."</p> }.into_any(),
                                }
                            }
                        }
                        None => view! { <p class="text-white text-center p-8">"Guide not found."</p> }.into_any(),
                    }
                }
                _ => view! { <p class="text-white text-center p-8">"Invalid guide URL."</p> }.into_any(),
            }
        }}
    }
}

fn render_level_page(level: &'static GuideLevelDef, platform: &str) -> impl IntoView {
    let page_title = format!("{} | WE HODL BTC", level.title);
    let wallets = guides::wallets_for(level, platform);
    let platform_owned = platform.to_string();
    let platform_display = match platform {
        "android" => "Android",
        "ios" => "iOS",
        "desktop" => "Desktop",
        _ => platform,
    };
    let full_title = if level.id == "basic" {
        format!("Basic {} Self-Custody Guide", platform_display)
    } else {
        level.title.to_string()
    };

    view! {
        <Title text=page_title/>
        <article class=page_layout()>
            <PageHeader
                title=full_title
                quote=level.quote.to_string()
                quote_author=level.quote_author.to_string()
            />

            // Intro text
            <section class="px-6 pt-4 lg:pt-0 lg:px-0">
                {render_level_intro(level, platform)}
            </section>

            // Wallet picker (basic) or step navigation (intermediate/advanced)
            <section class="px-6 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>
                {if !wallets.is_empty() {
                    // Basic: show wallet picker
                    view! {
                        <div>
                            <h2 class="text-center pb-4 text-lg font-semibold text-[#f7931a]">
                                "Pick A Wallet"
                            </h2>
                            <div class="flex flex-col mx-auto justify-center lg:flex-row px-6 gap-4">
                                {wallets.iter().map(|w| {
                                    view! { <WalletCard wallet=w platform=platform_owned.clone() level=level.id.to_string()/> }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    }.into_any()
                } else if !level.steps.is_empty() {
                    // Intermediate: show step navigation
                    render_step_navigation(level).into_any()
                } else if let Some(faq_dir) = level.faq_dir {
                    // Advanced: show stepper directly
                    view! {
                        <div>
                            <h3 class="py-4 text-center text-lg text-[#f7931a] font-semibold">
                                "Advanced Setup"
                            </h3>
                            <Stepper faq_name=faq_dir.to_string()/>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </section>
        </article>
    }
}

fn render_level_intro(level: &'static GuideLevelDef, _platform: &str) -> impl IntoView {
    match level.id {
        "intermediate" => view! {
            <div>
                <h2 class="text-left text-lg text-[#f7931a] font-semibold lg:text-lg">
                    "Coldcard & Node Setup:"
                </h2>
                <p class="text-[0.9rem] text-white">
                    {level.intro}
                    " If you originally chose a mobile setup, I recommend that you install Sparrow Desktop wallet by following the"
                    <a class="text-[#8cb4ff] underline-offset-auto" href="/guides/basic/desktop">" basic desktop guide "</a>
                    "before continuing with this guide."
                </p>
                <p class="pt-2 text-[0.9rem] text-white">
                    "In this guide, we'll start by setting up a Coldcard signing device (AKA hardware wallet), and connecting it to Sparrow. In part two we'll decide which Bitcoin node implementation to use, and then connect our Sparrow wallet to it. Once we're through with this, you'll have a standards based, secure and private Bitcoin self-custody solution."
                </p>
            </div>
        }.into_any(),
        "advanced" => view! {
            <div>
                <h2 class="text-left text-lg text-[#f7931a] font-semibold">"MultiSignature Wallet"</h2>
                <p class="py-2 text-[0.9rem] text-white">{level.intro}</p>
                <p class="text-[0.9rem] text-white">"A secure and private advanced self-custody setup looks like the following:"</p>
                <ol class="list-decimal pl-8 pt-2 text-lg leading-normal text-white">
                    <li>"Setup and run your own Bitcoin node"</li>
                    <li>"Setup a 2 of 3 Multisig in Sparrow Wallet using 3 signing devices"</li>
                    <li>"Use Sparrow Wallet to coordinate the Multisig. Preferably on a dedicated computer"</li>
                    <li>"Backup your Seed Words and Passphrases on steel"</li>
                    <li>"Safely backup and store your Multisig Wallet's Output Descriptors"</li>
                    <li>"Store the backups and devices in different geographic locations"</li>
                </ol>
                <p class="italic pt-4 text-[0.9rem] text-white">
                    "Before starting, I encourage you to read through all the steps below, so as to get an understanding of the options available to you."
                </p>
            </div>
        }.into_any(),
        _ => view! {
            <div>
                <p class="text-[0.95rem] font-semibold text-white pb-2">
                    "Bitcoin Self-Custody: The act of taking possession of a bitcoin private key."
                </p>
                <p class="text-[0.9rem] text-white pb-2">{level.intro}</p>
            </div>
        }.into_any(),
    }
}

fn render_step_navigation(level: &'static GuideLevelDef) -> impl IntoView {
    // Show only the first step as a "Level up" entry point
    let first_step = level.steps.first();
    view! {
        <div class="pb-6 pt-4 flex flex-col items-center">
            {first_step.map(|step| {
                let path = format!("/guides/{}/{}", level.id, step.id);
                view! {
                    <a href=path class="block">
                        <button class="inline-flex items-center gap-3 px-5 py-3 bg-white/95 border border-white/20 rounded-xl hover:bg-white hover:-translate-y-0.5 hover:shadow-lg active:translate-y-0 transition-all duration-200 cursor-pointer">
                            <img class="h-10 w-10 shrink-0 object-contain" src=step.icon alt=step.icon_alt/>
                            <span class="text-base font-semibold text-[#f7931a]">"Level up to Intermediate"</span>
                        </button>
                    </a>
                }
            })}
        </div>
    }
}

// =============================================================================
// Route: /guides/:level/:platform/:wallet — Wallet-specific guide stepper
// =============================================================================

#[component]
pub fn GuideWalletPage() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();

    let wallet_id = move || params.read().get("wallet");
    let platform_id = move || params.read().get("platform");

    view! {
        {move || {
            let wallet_id = wallet_id();
            let platform_id = platform_id();

            match (wallet_id.as_deref(), platform_id.as_deref()) {
                (Some(wid), Some(pid)) => {
                    match guides::find_wallet(wid) {
                        Some(wallet) => render_wallet_page(wallet, pid).into_any(),
                        None => view! { <p class="text-white text-center p-8">"Wallet not found."</p> }.into_any(),
                    }
                }
                _ => view! { <p class="text-white text-center p-8">"Invalid wallet URL."</p> }.into_any(),
            }
        }}
    }
}

fn render_wallet_page(wallet: &'static WalletDef, platform: &str) -> impl IntoView {
    let page_title = format!("{} Guide | WE HODL BTC", wallet.name);
    let downloads = guides::downloads_for(wallet, platform);

    view! {
        <Title text=page_title/>
        <article class=page_layout()>
            <PageHeader
                title=wallet.name.to_string()
                quote=wallet.tagline.to_string()
                quote_author=wallet.description.to_string()
            />

            // Download buttons
            <div class="flex flex-col mx-auto justify-center px-6 py-2 max-w-2xl mx-auto gap-4">
                {downloads.iter().map(|d| {
                    view! { <DownloadButton download=d/> }
                }).collect::<Vec<_>>()}
            </div>

            // Stepper
            <div class="mx-auto max-w-5xl p-4 w-full">
                <div class="mx-auto border border-solid border-gray-400"></div>
                <h2 class="flex justify-center font-semibold text-[#f7931a] text-lg pt-6 pb-4">
                    "Get Started"
                </h2>
                <Stepper faq_name=wallet.faq_dir.to_string()/>
            </div>
        </article>
    }
}

fn render_step_page(step: &'static guides::GuideStep, level_id: &str) -> impl IntoView {
    let page_title = format!("{} | WE HODL BTC", step.title);

    view! {
        <Title text=page_title/>
        <article class=page_layout()>
            <PageHeader title=step.title.to_string()/>

            // Product purchase buttons
            {(!step.products.is_empty()).then(|| view! {
                <div class="px-4 lg:pt-0 lg:px-0">
                    <ProductButtons products=step.products/>
                </div>
            })}

            // Stepper
            <div class="px-4 lg:pb-4 lg:px-0">
                <hr class="border border-solid border-gray-400 mx-auto w-full mb-6"/>
                <h2 class="text-center pb-4 text-lg text-[#f7931a] font-semibold">
                    "Start Here"
                </h2>
                <Stepper faq_name=step.faq_dir.to_string()/>
            </div>

            // Next step link
            {step.next_step.map(|next| {
                let level = guides::find_level(level_id);
                let next_step = level.and_then(|l| l.steps.iter().find(|s| s.id == next));
                let path = format!("/guides/{}/{}", level_id, next);
                let heading = step.next_step_label.unwrap_or("Next Step");
                let button_label = step.next_step_button_label.unwrap_or(heading);
                let icon = next_step.map(|s| s.icon).unwrap_or("");
                let icon_alt = next_step.map(|s| s.icon_alt).unwrap_or("");
                view! {
                    <div class="px-4 lg:pb-4 lg:px-0">
                        <h3 class="text-center pb-4 text-lg text-[#f7931a] font-semibold">
                            {heading}
                        </h3>
                        <div class="pb-4 flex justify-center">
                            <a href=path class="block">
                                <button class="inline-flex items-center gap-3 px-5 py-3 bg-white/95 border border-white/20 rounded-xl hover:bg-white hover:-translate-y-0.5 hover:shadow-lg active:translate-y-0 transition-all duration-200 cursor-pointer">
                                    {(!icon.is_empty()).then(|| view! {
                                        <img class="h-10 w-10 shrink-0 object-contain" src=icon alt=icon_alt/>
                                    })}
                                    <span class="text-base font-semibold text-[#f7931a]">{button_label}</span>
                                </button>
                            </a>
                        </div>
                    </div>
                }
            })}
        </article>
    }
}
