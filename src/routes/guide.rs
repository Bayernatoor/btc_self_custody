//! Unified guide pages - replaces beginner.rs, intermediate.rs, advanced.rs.
//!
//! Two route components:
//! - GuideTwoSegment: /guides/:level/:segment - dispatches to level page or step page
//! - GuideWalletPage: /guides/:level/:platform/:wallet - wallet-specific stepper

use leptos::prelude::*;
use leptos_meta::*;

use crate::extras::stepper::Stepper;
use crate::extras::stepper_v2::{inline, StepperV2};
use crate::guides::{
    self, DownloadLink, GuideLevelDef, ProductLink, WalletDef,
};
use crate::guides_v2;
use crate::routes::guideselector::guide_selector_view;

// =============================================================================
// Shared sub-components
// =============================================================================

/// Breadcrumb trail: Guides > Level > Platform > Wallet. Shared wizard chrome,
/// used by the selector and every guide page. Safe when `crumbs` is empty (root):
/// an empty `href` renders the label as the current (non-link) crumb.
#[component]
pub fn Breadcrumbs(crumbs: Vec<(String, String)>) -> impl IntoView {
    let last = crumbs.len().saturating_sub(1);
    view! {
        <nav aria-label="Breadcrumb" class="g2-crumbs">
            <a href="/guides">"Guides"</a>
            {crumbs.into_iter().enumerate().map(|(i, (label, href))| {
                let is_last = i == last;
                view! {
                    <span class="g2-crumb-sep" aria-hidden="true">"›"</span>
                    {if is_last || href.is_empty() {
                        view! { <span class="g2-crumb-cur">{label}</span> }.into_any()
                    } else {
                        view! { <a href=href>{label}</a> }.into_any()
                    }}
                }
            }).collect::<Vec<_>>()}
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
    // The card is a <div> (not a link) so trait chips can carry their own links
    // (e.g. BDK). The "Choose" link stretches over the whole card via ::after, so
    // the card stays fully clickable while inner links remain independently usable.
    view! {
        <div class="g2-wcard">
            <div class="g2-wcard-head">
                <div class="g2-wcard-ic">
                    <img class="h-8 w-8 rounded-md" src=wallet.logo alt=wallet.logo_alt/>
                </div>
                <h3 class="g2-wcard-name">{wallet.name}</h3>
            </div>
            <p class="g2-wcard-why">{wallet.tagline}</p>
            <ul class="g2-wcard-traits">
                {wallet.highlights.iter().map(|h| view! {
                    <li><span class="g2-wcard-ck">"\u{2713}"</span><span>{inline(h)}</span></li>
                }).collect::<Vec<_>>()}
            </ul>
            <a href=path class="g2-wcard-go">"Choose "<span class="g2-wcard-arrow">"\u{2192}"</span></a>
        </div>
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
    // Fills the viewport (minus navbar) and centers short content, so the footer
    // is consistently pushed below the fold and pages don't jump when clicking
    // through. Long content (steppers) exceeds the min-height and top-aligns.
    "flex flex-col items-center justify-center min-h-[calc(100vh-4rem)] max-w-2xl mx-auto px-6 py-10 opacity-0 animate-fadeinone lg:px-8 lg:max-w-3xl"
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
    let page_title = format!("{} | We Hodl BTC", level.title);
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
    // A wallet picker is a choose-one panel (like the selector), so it gets the
    // slim progress bar; the intermediate/advanced level pages don't.
    let has_picker = !wallets.is_empty();
    // The basic level's intro is mobile-framed ("mobile wallet", "carry cash");
    // the desktop path (Sparrow, single-sig + passphrase) needs its own framing.
    let picker_lede = if level.id == "basic" && is_desktop {
        "This basic desktop setup is a sturdier base than a phone wallet. You will create a single-signature Sparrow wallet, protected by a passphrase, and take possession of your own keys. It is a great everyday setup, and a solid foundation to grow into a hardware wallet or multisig as your stack grows."
    } else {
        level.intro
    };
    // Opinionated framing: these are curated recommendations, not a neutral list.
    let wallet_title = if wallets.len() == 1 { "Recommended Wallet" } else { "Recommended Wallets" };
    let single_wallet = wallets.len() == 1;

    view! {
        <Title text=page_title/>
        <Meta name="description" content=meta_desc/>
        <Link rel="canonical" href=canonical/>
        {match guides_v2::find_level_guide_v2(level.id) {
            // A level with a v2 guide (e.g. Intermediate) renders the wizard directly.
            Some(guide) => view! {
                <div class="g2-shell">
                    <Breadcrumbs crumbs=crumbs.clone()/>
                    <StepperV2 guide=guide downloads=Vec::new()/>
                </div>
            }.into_any(),
            None => view! {
        <div class="g2-shell">
            <Breadcrumbs crumbs=crumbs/>
            {has_picker.then(|| view! {
                <div class="g2-prog"><div class="g2-prog-fill" style="width:45%"></div></div>
            })}
            <div class="g2-flow">
            <div class="g2-flow-inner">
            {if has_picker {
                // Refined "choose a wallet" header — matches the selector + guide intro
                // (eyebrow + white Oswald title + muted lede). No orange title / underline
                // / quote / boxed definition; the OS is carried by the eyebrow.
                view! {
                    <header class="text-center mb-8">
                        <h1 class="g2-h">{full_title.clone()}</h1>
                        {(!level.quote.is_empty()).then(|| view! {
                            <p class="text-[0.95rem] text-[#f7931a] italic max-w-md mx-auto mt-1">{level.quote}</p>
                            <p class="text-xs text-[#f7931a] opacity-70 mt-0.5">{level.quote_author}</p>
                        })}
                        <p class="g2-lede g2-center mt-5">{picker_lede}</p>
                    </header>
                }.into_any()
            } else {
                // v1 header for the intermediate/advanced level pages (migrated later).
                view! {
                    <PageHeader
                        title=full_title.clone()
                        quote=level.quote.to_string()
                        quote_author=level.quote_author.to_string()
                    />
                    {is_desktop.then(|| view! {
                        <div class="flex justify-center mb-4">
                            <span class="inline-flex items-center gap-1.5 px-3 py-1 rounded-full text-xs font-medium bg-[#f7931a]/10 text-[#f7931a] border border-[#f7931a]/20">
                                {pd.clone()}
                            </span>
                        </div>
                    })}
                    <div class="w-full mb-8 animate-slideup" style="animation-delay: 100ms">
                        {render_level_intro(level, platform)}
                    </div>
                }.into_any()
            }}

            // Wallet picker / step nav / stepper
            <div class="w-full">
                {if !wallets.is_empty() {
                    view! {
                        <div class="animate-slideup" style="animation-delay: 200ms">
                            <h2 class="font-title uppercase tracking-wider text-[0.8rem] text-[#f7931a] text-center mb-4">{wallet_title}</h2>
                            <div class=if single_wallet { "g2-wcards g2-wcards-one" } else { "g2-wcards" }>
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
            </div>
        </div>
            }.into_any(),
        }}
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
    let page_title = format!("{} Guide | We Hodl BTC", wallet.name);
    let downloads = guides::downloads_for(wallet, platform);
    let level_name = level.map(|l| l.name).unwrap_or("Guide");
    let platform_display = guides::platform_display(platform);

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

    // v2 wallets render the refined wizard; everything else keeps the v1 layout.
    let guide_v2 = guides_v2::find_guide_v2(wallet.id);

    view! {
        <Title text=page_title/>
        <Meta name="description" content=meta_desc/>
        <Link rel="canonical" href=canonical/>
        {match guide_v2 {
            Some(guide) => view! {
                <div class="g2-shell">
                    <Breadcrumbs crumbs=crumbs/>
                    <StepperV2 guide=guide downloads=downloads/>
                </div>
            }.into_any(),
            None => {
                let os_tip = guides::os_tip(platform).map(|(t, b)| (t.to_string(), b.to_string()));
                let is_desktop = guides::is_desktop_os(platform);
                let platform_display_owned = platform_display.to_string();
                view! {
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
                }.into_any()
            }
        }}
    }
}

fn render_step_page(
    step: &'static guides::GuideStep,
    level_id: &str,
) -> impl IntoView {
    let page_title = format!("{} | We Hodl BTC", step.title);
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
