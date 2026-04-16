//! Unified guide pages - replaces beginner.rs, intermediate.rs, advanced.rs.
//!
//! Two route components:
//! - GuideTwoSegment: /guides/:level/:segment - dispatches to level page or step page
//! - GuideWalletPage: /guides/:level/:platform/:wallet - wallet-specific stepper

use leptos::prelude::*;
use leptos_meta::*;

use crate::extras::stepper::Stepper;
use crate::guides::{
    self, DownloadLink, GuideLevelDef, ProductLink, WalletDef,
};
use crate::routes::guideselector::guide_selector_view;

// =============================================================================
// Shared sub-components
// =============================================================================

/// Breadcrumb trail: Guides > Level > Platform > Wallet
#[component]
fn Breadcrumbs(crumbs: Vec<(String, String)>) -> impl IntoView {
    let last = crumbs.len() - 1;
    view! {
        <nav aria-label="Breadcrumb" class="w-full mb-4">
            <ol class="flex items-center gap-1.5 text-xs text-white/40">
                <li>
                    <a href="/guides" class="hover:text-white/70 transition-colors">"Guides"</a>
                </li>
                {crumbs.into_iter().enumerate().map(|(i, (label, href))| {
                    let is_last = i == last;
                    view! {
                        <li class="flex items-center gap-1.5">
                            <svg class="w-3 h-3 text-white/20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                            </svg>
                            {if is_last {
                                view! { <span class="text-white/60">{label}</span> }.into_any()
                            } else {
                                view! { <a href=href class="hover:text-white/70 transition-colors">{label}</a> }.into_any()
                            }}
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ol>
        </nav>
    }
}

#[component]
fn PageHeader(
    title: String,
    #[prop(optional)] subtitle: String,
    #[prop(optional)] quote: String,
    #[prop(optional)] quote_author: String,
) -> impl IntoView {
    view! {
        <header class="text-center mb-8 animate-scaleup">
            <h1 class="text-[1.65rem] text-[#f7931a] font-semibold leading-tight font-title md:text-[2rem] lg:text-[2.5rem]">
                {title}
            </h1>
            <div class="w-16 h-0.5 bg-[#f7931a] mx-auto mt-3 mb-4"></div>
            {(!subtitle.is_empty()).then(|| view! {
                <p class="text-[0.9rem] text-white/60 max-w-lg mx-auto">{subtitle}</p>
            })}
            {(!quote.is_empty()).then(|| view! {
                <p class="text-[0.9rem] text-white/50 italic max-w-md mx-auto mt-2">{quote}</p>
            })}
            {(!quote_author.is_empty()).then(|| view! {
                <p class="text-xs text-white/30 mt-0.5">{quote_author}</p>
            })}
        </header>
    }
}

#[component]
fn DownloadButton(download: &'static DownloadLink) -> impl IntoView {
    let is_filled = download.icon.contains("fill-rule")
        || download.icon.contains("M18.71")
        || download.icon.contains("M3.609");
    view! {
        <a href=download.url rel="noreferrer" target="_blank" class="block w-full">
            <button
                class="group flex items-center gap-3 w-full px-5 py-3 rounded-xl border transition-all duration-200 cursor-pointer hover:scale-[1.02] active:scale-[0.98]"
                style=format!("background: {}15; border-color: {}30", download.color, download.color)
            >
                <div
                    class="w-8 h-8 rounded-lg flex items-center justify-center shrink-0"
                    style=format!("background: {}25", download.color)
                >
                    {if is_filled {
                        view! {
                            <svg class="w-4.5 h-4.5" viewBox="0 0 24 24" fill="currentColor" style=format!("color: {}", download.color) inner_html=download.icon></svg>
                        }.into_any()
                    } else {
                        view! {
                            <svg class="w-4.5 h-4.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" style=format!("color: {}", download.color) inner_html=download.icon></svg>
                        }.into_any()
                    }}
                </div>
                <span class="text-sm font-medium text-white/90 group-hover:text-white transition-colors">{download.label}</span>
                <svg class="w-4 h-4 ml-auto text-white/25 group-hover:text-white/50 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"/>
                </svg>
            </button>
        </a>
    }
}

#[component]
fn WalletCard(
    wallet: &'static WalletDef,
    platform: String,
    level: String,
) -> impl IntoView {
    let path = format!("/guides/{}/{}/{}", level, platform, wallet.id);
    let color = wallet.color;
    view! {
        <a href=path class="block">
            <button class="group flex items-center gap-4 w-full px-5 py-4 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer">
                <div class="w-11 h-11 rounded-lg bg-white/10 flex items-center justify-center shrink-0 group-hover:bg-white/15 transition-colors">
                    <img class="h-8 w-8 rounded-md" src=wallet.logo alt=wallet.logo_alt/>
                </div>
                <div class="flex-1 text-left">
                    <h3 class="text-base lg:text-lg font-semibold" style=format!("color: {color}")>{wallet.name}</h3>
                    <p class="text-sm text-white/50 mt-0.5">{wallet.tagline}</p>
                </div>
                <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                </svg>
            </button>
        </a>
    }
}

#[component]
fn ProductCard(product: &'static ProductLink) -> impl IntoView {
    view! {
        <a href=product.url rel="noreferrer" target="_blank" class="block h-full">
            <button class="group flex flex-col items-center justify-center gap-2 w-full h-full px-4 py-4 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/20 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer">
                <img
                    class=format!("h-{} w-{} object-contain opacity-80 group-hover:opacity-100 transition-opacity", product.logo_height, product.logo_width)
                    src=product.logo
                    alt=product.logo_alt
                />
                <span class="text-xs font-medium text-white/70 group-hover:text-white/90 transition-colors text-center">{product.name}</span>
            </button>
        </a>
    }
}

fn centered_layout() -> &'static str {
    "flex flex-col items-center max-w-2xl mx-auto px-6 mt-10 mb-24 opacity-0 animate-fadeinone lg:px-8 lg:max-w-3xl md:my-20"
}

// =============================================================================
// Route: /guides/:level/:segment - dispatches to level page or step page
// =============================================================================

const PLATFORMS: &[&str] = &[
    "android",
    "ios",
    "desktop",
    "desktop-linux",
    "desktop-macos",
    "desktop-windows",
];

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
                            if seg == "desktop" {
                                // /guides/basic/desktop → show OS picker
                                let (selected_level, set_selected_level) = signal(Some(level.id));
                                let (selected_platform, set_selected_platform) = signal(Some("desktop"));
                                guide_selector_view(selected_level, set_selected_level, selected_platform, set_selected_platform).into_any()
                            } else if PLATFORMS.contains(&seg) {
                                render_level_page(level, seg).into_any()
                            } else {
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

fn render_level_page(
    level: &'static GuideLevelDef,
    platform: &str,
) -> impl IntoView {
    let platform_display = guides::platform_display(platform);
    let page_title = format!("{} | WE HODL BTC", level.title);
    let wallets = guides::wallets_for(level, platform);
    let platform_owned = platform.to_string();
    let full_title = if level.id == "basic" {
        if guides::is_desktop_os(platform) {
            format!("Basic Desktop Self-Custody Guide - {}", platform_display)
        } else {
            format!("Basic {} Self-Custody Guide", platform_display)
        }
    } else {
        level.title.to_string()
    };

    let crumbs = vec![
        (level.name.to_string(), "/guides".to_string()),
        (
            platform_display.to_string(),
            format!("/guides/{}/{}", level.id, platform),
        ),
    ];

    let is_desktop = guides::is_desktop_os(platform);
    let pd = platform_display.to_string();
    let meta_desc = format!(
        "Free {} Bitcoin self-custody guide for {}. Learn to secure your own bitcoin with step-by-step wallet setup instructions.",
        level.name.to_lowercase(), platform_display
    );
    let canonical =
        format!("https://www.wehodlbtc.com/guides/{}/{}", level.id, platform);

    view! {
        <Title text=page_title/>
        <Meta name="description" content=meta_desc/>
        <Link rel="canonical" href=canonical/>
        <div class=centered_layout()>
            <Breadcrumbs crumbs=crumbs/>
            <PageHeader
                title=full_title
                quote=level.quote.to_string()
                quote_author=level.quote_author.to_string()
            />

            // OS badge for desktop variants
            {is_desktop.then(|| view! {
                <div class="flex justify-center mb-4">
                    <span class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium bg-[#f7931a]/10 text-[#f7931a] border border-[#f7931a]/20">
                        {pd.clone()}
                    </span>
                </div>
            })}

            // Intro
            <div class="w-full mb-8 animate-slideup" style="animation-delay: 100ms">
                {render_level_intro(level, platform)}
            </div>

            // Wallet picker / step nav / stepper
            <div class="w-full">
                {if !wallets.is_empty() {
                    view! {
                        <div class="animate-slideup" style="animation-delay: 200ms">
                            <h2 class="text-base text-[#f7931a] font-semibold text-center mb-4">"Pick a Wallet"</h2>
                            <div class="flex flex-col gap-3">
                                {wallets.iter().enumerate().map(|(i, w)| {
                                    let delay = format!("animation-delay: {}ms", 250 + i * 80);
                                    view! {
                                        <div class="opacity-0 animate-slideup" style=delay>
                                            <WalletCard wallet=w platform=platform_owned.clone() level=level.id.to_string()/>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    }.into_any()
                } else if !level.steps.is_empty() {
                    render_step_navigation(level).into_any()
                } else if let Some(faq_dir) = level.faq_dir {
                    view! {
                        <div class="animate-slideup" style="animation-delay: 200ms">
                            <h2 class="text-base text-[#f7931a] font-semibold text-center mb-4">"Advanced Setup"</h2>
                            <Stepper faq_name=faq_dir.to_string()/>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }}
            </div>
        </div>
    }
}

fn render_level_intro(
    level: &'static GuideLevelDef,
    _platform: &str,
) -> impl IntoView {
    match level.id {
        "intermediate" => view! {
            <div class="bg-white/5 border border-white/10 rounded-xl p-5">
                <h2 class="text-base text-[#f7931a] font-semibold mb-2">"Coldcard & Node Setup"</h2>
                <p class="text-[0.85rem] text-white/80 leading-relaxed lg:text-[0.95rem]">
                    {level.intro}
                    " If you originally chose a mobile setup, I recommend installing Sparrow Desktop wallet via the "
                    <a class="text-blue-400 hover:text-blue-300 transition-colors" href="/guides/basic/desktop">"basic desktop guides"</a>
                    " (available for Linux, macOS, and Windows) before continuing."
                </p>
                <p class="mt-3 text-[0.85rem] text-white/80 leading-relaxed lg:text-[0.95rem]">
                    "We'll start by setting up a Coldcard signing device and connecting it to Sparrow. In part two we'll choose a Bitcoin node implementation and connect our wallet to it."
                </p>
            </div>
        }.into_any(),
        "advanced" => view! {
            <div class="bg-white/5 border border-white/10 rounded-xl p-5">
                <h2 class="text-base text-[#f7931a] font-semibold mb-2">"MultiSignature Wallet"</h2>
                <p class="text-[0.85rem] text-white/80 leading-relaxed lg:text-[0.95rem] mb-3">{level.intro}</p>
                <ol class="list-decimal pl-5 text-[0.85rem] text-white/80 leading-relaxed lg:text-[0.95rem] space-y-1">
                    <li>"Setup and run your own Bitcoin node"</li>
                    <li>"Setup a 2 of 3 Multisig in Sparrow Wallet using 3 signing devices"</li>
                    <li>"Use Sparrow Wallet to coordinate the Multisig, preferably on a dedicated computer"</li>
                    <li>"Backup your Seed Words and Passphrases on steel"</li>
                    <li>"Safely backup and store your Multisig Wallet's Output Descriptors"</li>
                    <li>"Store the backups and devices in different geographic locations"</li>
                </ol>
                <p class="mt-3 text-[0.85rem] text-white/60 italic">
                    "Read through all the steps below before starting, so you understand the options available."
                </p>
            </div>
        }.into_any(),
        _ => view! {
            <div class="bg-white/5 border border-white/10 rounded-xl p-5">
                <p class="text-[0.85rem] font-medium text-white/90 mb-2">
                    "Bitcoin Self-Custody: The act of taking possession of a bitcoin private key."
                </p>
                <p class="text-[0.85rem] text-white/80 leading-relaxed lg:text-[0.95rem]">{level.intro}</p>
            </div>
        }.into_any(),
    }
}

fn render_step_navigation(level: &'static GuideLevelDef) -> impl IntoView {
    let first_step = level.steps.first();
    view! {
        <div class="flex flex-col items-center animate-slideup" style="animation-delay: 200ms">
            {first_step.map(|step| {
                let path = format!("/guides/{}/{}", level.id, step.id);
                view! {
                    <a href=path class="block">
                        <button class="group inline-flex items-center gap-4 px-6 py-4 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer">
                            <img class="h-10 w-10 shrink-0 object-contain" src=step.icon alt=step.icon_alt/>
                            <span class="text-base lg:text-lg font-semibold text-[#f7931a]">"Level up to Intermediate"</span>
                            <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                            </svg>
                        </button>
                    </a>
                }
            })}
        </div>
    }
}

// =============================================================================
// Route: /guides/:level/:platform/:wallet - Wallet-specific guide stepper
// =============================================================================

#[component]
pub fn GuideWalletPage() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let level_id = move || params.read().get("level");
    let wallet_id = move || params.read().get("wallet");
    let platform_id = move || params.read().get("platform");

    view! {
        {move || {
            let level_id = level_id();
            let wallet_id = wallet_id();
            let platform_id = platform_id();
            match (level_id.as_deref(), wallet_id.as_deref(), platform_id.as_deref()) {
                (Some(lid), Some(wid), Some(pid)) => {
                    let level = guides::find_level(lid);
                    match guides::find_wallet(wid) {
                        Some(wallet) => render_wallet_page(wallet, pid, lid, level).into_any(),
                        None => view! { <p class="text-white text-center p-8">"Wallet not found."</p> }.into_any(),
                    }
                }
                _ => view! { <p class="text-white text-center p-8">"Invalid wallet URL."</p> }.into_any(),
            }
        }}
    }
}

fn render_wallet_page(
    wallet: &'static WalletDef,
    platform: &str,
    level_id: &str,
    level: Option<&'static GuideLevelDef>,
) -> impl IntoView {
    let page_title = format!("{} Guide | WE HODL BTC", wallet.name);
    let downloads = guides::downloads_for(wallet, platform);
    let level_name = level.map(|l| l.name).unwrap_or("Guide");
    let platform_display = guides::platform_display(platform);
    let os_tip =
        guides::os_tip(platform).map(|(t, b)| (t.to_string(), b.to_string()));
    let is_desktop = guides::is_desktop_os(platform);
    let platform_display_owned = platform_display.to_string();

    let crumbs = vec![
        (level_name.to_string(), "/guides".to_string()),
        (
            platform_display.to_string(),
            format!("/guides/{}/{}", level_id, platform),
        ),
        (
            wallet.name.to_string(),
            format!("/guides/{}/{}/{}", level_id, platform, wallet.id),
        ),
    ];

    let meta_desc = format!(
        "Set up {} for Bitcoin self-custody. {}",
        wallet.name, wallet.tagline
    );
    let canonical = format!(
        "https://www.wehodlbtc.com/guides/{}/{}/{}",
        level_id, platform, wallet.id
    );

    view! {
        <Title text=page_title/>
        <Meta name="description" content=meta_desc/>
        <Link rel="canonical" href=canonical/>
        <div class=centered_layout()>
            <Breadcrumbs crumbs=crumbs/>
            <PageHeader
                title=wallet.name.to_string()
                subtitle=wallet.tagline.to_string()
                quote=wallet.description.to_string()
            />

            // OS badge
            {is_desktop.then(|| view! {
                <div class="flex justify-center mb-4 opacity-0 animate-slideup" style="animation-delay: 50ms">
                    <span class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium bg-[#f7931a]/10 text-[#f7931a] border border-[#f7931a]/20">
                        {platform_display_owned.clone()}
                    </span>
                </div>
            })}

            // OS-specific tip
            {os_tip.map(|(title, body)| view! {
                <div class="w-full mb-6 opacity-0 animate-slideup" style="animation-delay: 80ms">
                    <div class="flex gap-3 p-4 bg-[#f7931a]/5 border border-[#f7931a]/10 rounded-xl">
                        <svg class="w-5 h-5 text-[#f7931a] shrink-0 mt-0.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                        </svg>
                        <div>
                            <p class="text-sm font-medium text-[#f7931a] mb-1">{title}</p>
                            <p class="text-[0.8rem] text-white/60 leading-relaxed">{body}</p>
                        </div>
                    </div>
                </div>
            })}

            // Downloads
            <div class="w-full max-w-sm mx-auto flex flex-col gap-3 mb-8 opacity-0 animate-slideup" style="animation-delay: 100ms">
                {downloads.iter().map(|d| {
                    view! { <DownloadButton download=d/> }
                }).collect::<Vec<_>>()}
            </div>

            // Stepper
            <div class="w-full opacity-0 animate-slideup" style="animation-delay: 200ms">
                <h2 class="text-base text-[#f7931a] font-semibold text-center mb-4">"Get Started"</h2>
                <Stepper faq_name=wallet.faq_dir.to_string()/>
            </div>
        </div>
    }
}

fn render_step_page(
    step: &'static guides::GuideStep,
    level_id: &str,
) -> impl IntoView {
    let page_title = format!("{} | WE HODL BTC", step.title);
    let level = guides::find_level(level_id);
    let level_name = level.map(|l| l.name).unwrap_or("Guide");

    let crumbs = vec![
        (level_name.to_string(), "/guides".to_string()),
        (
            step.name.to_string(),
            format!("/guides/{}/{}", level_id, step.id),
        ),
    ];

    let meta_desc = format!(
        "{} - Bitcoin self-custody guide. Step-by-step instructions for {}.",
        step.title, level_name
    );
    let canonical =
        format!("https://www.wehodlbtc.com/guides/{}/{}", level_id, step.id);

    view! {
        <Title text=page_title/>
        <Meta name="description" content=meta_desc/>
        <Link rel="canonical" href=canonical/>
        <div class=centered_layout()>
            <Breadcrumbs crumbs=crumbs/>
            <PageHeader title=step.title.to_string()/>

            // Products
            {(!step.products.is_empty()).then(|| view! {
                <div class="w-full grid grid-cols-1 sm:grid-cols-3 gap-3 mb-8 opacity-0 animate-slideup" style="animation-delay: 100ms">
                    {step.products.iter().map(|p| {
                        view! { <ProductCard product=p/> }
                    }).collect::<Vec<_>>()}
                </div>
            })}

            // Stepper
            <div class="w-full opacity-0 animate-slideup" style="animation-delay: 200ms">
                <h2 class="text-base text-[#f7931a] font-semibold text-center mb-4">"Start Here"</h2>
                <Stepper faq_name=step.faq_dir.to_string()/>
            </div>

            // Next step
            {step.next_step.map(|next| {
                let level = guides::find_level(level_id);
                let next_step = level.and_then(|l| l.steps.iter().find(|s| s.id == next));
                let path = format!("/guides/{}/{}", level_id, next);
                let heading = step.next_step_label.unwrap_or("Next Step");
                let button_label = step.next_step_button_label.unwrap_or(heading);
                let icon = next_step.map(|s| s.icon).unwrap_or("");
                let icon_alt = next_step.map(|s| s.icon_alt).unwrap_or("");
                view! {
                    <div class="w-full mt-8 pt-6 border-t border-white/10">
                        <h3 class="text-base text-[#f7931a] font-semibold text-center mb-4">{heading}</h3>
                        <div class="flex justify-center">
                            <a href=path class="block">
                                <button class="group inline-flex items-center gap-3 px-5 py-3.5 bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/25 hover:scale-[1.02] active:scale-[0.98] transition-all duration-200 cursor-pointer">
                                    {(!icon.is_empty()).then(|| view! {
                                        <img class="h-10 w-10 shrink-0 object-contain" src=icon alt=icon_alt/>
                                    })}
                                    <span class="text-base lg:text-lg font-semibold text-[#f7931a]">{button_label}</span>
                                    <svg class="w-5 h-5 text-white/30 group-hover:text-white/60 group-hover:translate-x-0.5 transition-all" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                                    </svg>
                                </button>
                            </a>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}
