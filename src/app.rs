//! App shell and router.
//!
//! `shell()` renders the outer HTML document (head, meta, scripts).
//! `App` sets up the Leptos router with all page routes.
//!
//! Route structure:
//!   /                                        -> HomePage
//!   /guides                                  -> GuideSelector
//!   /guides/:level/:segment                  -> GuideTwoSegment
//!   /guides/:level/:platform/:wallet         -> GuideWalletPage
//!   /observatory                             -> Dashboard
//!   /observatory/charts/network              -> Network charts
//!   /observatory/charts/fees                 -> Fee charts
//!   /observatory/charts/mining               -> Mining charts
//!   /observatory/charts/embedded             -> Embedded data charts
//!   /observatory/signaling                   -> BIP signaling
//!   /observatory/learn/protocols             -> Protocol guide
//!   /blog, /faq, /about                      -> Static pages

use crate::extras::footer::Footer;
use crate::extras::navbar::NavBar;
use crate::routes::about::AboutPage;
use crate::routes::blog::BlogPage;
use crate::routes::faq::FaqPage;
use crate::routes::guide::{GuideTwoSegment, GuideWalletPage};
use crate::routes::guideselector::{GuideLevelSelector, GuideSelector};
use crate::routes::homepage::HomePage;
use crate::routes::observatory::learn::protocols::ProtocolGuidePage;
use crate::routes::observatory::{
    EmbeddedChartsPage, FeeChartsPage, MiningChartsPage, NetworkChartsPage,
    ObservatoryOverview, ObservatoryPage, SignalingPage, StatsSummaryPage,
};
use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    components::{ParentRoute, Route, Router, Routes},
    path,
};

/// Outer HTML shell — rendered once on the server, wraps the hydrated App.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>

                // Preload fonts to eliminate flash of unstyled text
                <link rel="preconnect" href="https://fonts.googleapis.com"/>
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>

                // description and keywords are set via leptos_meta in App() so pages can override
                // Open Graph
                <meta property="og:title" content="WE HODL BTC — Bitcoin Self-Custody & Blockchain Analytics"/>
                <meta property="og:type" content="website"/>
                <meta property="og:url" content="https://www.wehodlbtc.com/"/>
                <meta property="og:image" content="https://www.wehodlbtc.com/img/metadata_unfurl_image.png"/>
                <meta property="og:description" content="Free Bitcoin self-custody guides and The Bitcoin Observatory — live blockchain analytics with real-time network stats, fee charts, mining data, and BIP signaling tracker."/>
                <meta property="og:site_name" content="WE HODL BTC"/>

                // Twitter
                <meta name="twitter:card" content="summary_large_image"/>
                <meta name="twitter:title" content="WE HODL BTC — Bitcoin Self-Custody & Blockchain Analytics"/>
                <meta name="twitter:description" content="Free Bitcoin self-custody guides and The Bitcoin Observatory — live blockchain analytics with real-time network stats, fee charts, mining data, and BIP signaling tracker."/>
                <meta name="twitter:url" content="https://www.wehodlbtc.com/"/>
                <meta name="twitter:image" content="https://www.wehodlbtc.com/img/metadata_unfurl_image.png"/>
                <meta name="twitter:site" content="@bayernatoor"/>
                <meta name="twitter:creator" content="@bayernatoor"/>

                // Favicons
                <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png"/>
                <link rel="icon" type="image/svg+xml" href="/favicon.svg"/>
                <link rel="icon" type="image/png" sizes="96x96" href="/favicon-96x96.png"/>
                <link rel="manifest" href="/site.webmanifest"/>
                <link rel="mask-icon" href="/safari-pinned-tab.svg"/>
                <meta name="msapplication-TileColor" content="#123c64"/>
                <meta name="theme-color" content="#123c64"/>

                // ECharts for stats dashboard
                <script defer src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
                <script defer src="/js/stats.js"></script>

                // Fallback for browsers without WebAssembly (e.g. Vanadium)
                <script defer src="/js/wasm-fallback.js"></script>

                // Schema.org JSON-LD for search engines and LLMs
                <script defer src="/js/jsonld.js"></script>

                // Image lightbox for guide steps
                <script defer src="/js/lightbox.js"></script>

                // Collapsible sections within step content
                <script defer src="/js/sections.js"></script>

                // Service Worker registration (PWA) with update detection
                <script>"
                    if ('serviceWorker' in navigator) {
                        navigator.serviceWorker.register('/sw.js').then(function(reg) {
                            reg.addEventListener('updatefound', function() {
                                var newSW = reg.installing;
                                if (!newSW) return;
                                newSW.addEventListener('statechange', function() {
                                    if (newSW.state === 'installed' && navigator.serviceWorker.controller) {
                                        var banner = document.createElement('div');
                                        banner.id = 'sw-update-banner';
                                        banner.style.cssText = 'position:fixed;bottom:24px;right:24px;z-index:99999;background:#0d2137;border:1px solid rgba(247,147,26,0.4);border-radius:12px;padding:12px 18px;display:flex;align-items:center;gap:12px;box-shadow:0 8px 32px rgba(0,0,0,0.4);font-family:Inter,system-ui,sans-serif';
                                        banner.innerHTML = '<span style=\"color:rgba(255,255,255,0.7);font-size:13px\">Update available</span><button style=\"background:#f7931a;color:#0a1a2e;border:none;padding:6px 14px;border-radius:8px;font-size:12px;font-weight:600;cursor:pointer\">Refresh</button>';
                                        banner.querySelector('button').addEventListener('click', function() {
                                            newSW.postMessage('SKIP_WAITING');
                                            banner.remove();
                                        });
                                        document.body.appendChild(banner);
                                    }
                                });
                            });
                        });
                        navigator.serviceWorker.addEventListener('controllerchange', function() {
                            window.location.reload();
                        });
                    }
                "</script>

                // Analytics
                <script async src="https://www.poeticmetric.com/pm.js"></script>

                <HydrationScripts options/>
                <MetaTags/>
                <link rel="stylesheet" id="leptos" href="/pkg/we_hodl_btc.css"/>
            </head>
            <body class="bg-[#123c64]">
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="WE HODL BTC - Bitcoin Self-Custody Guides & Blockchain Analytics"/>
        <Meta name="description" content="Free, opinionated Bitcoin self-custody guides and The Bitcoin Observatory — live blockchain analytics with real-time network stats, fee charts, mining data, and BIP signaling tracker."/>
        <Meta name="keywords" content="Bitcoin, self-custody, hardware wallet, Coldcard, Sparrow Wallet, multisig, Bitcoin security, blockchain analytics, Bitcoin charts, mining difficulty, SegWit, Taproot, BIP signaling, mempool, Bitcoin node"/>

        <Router>
            <div class="flex flex-col justify-between h-screen">
                <NavBar/>
                <main>
                    <Routes fallback=|| view! {
                        <div class="flex flex-col items-center justify-center min-h-[60vh] px-6 opacity-0 animate-fadeinone">
                            <div class="text-[4rem] lg:text-[5rem] font-title text-white/10 font-bold mb-4">"404"</div>
                            <h1 class="text-xl lg:text-2xl text-white font-semibold mb-2">"Page not found"</h1>
                            <p class="text-sm text-white/50 mb-8 text-center max-w-sm">"The page you're looking for doesn't exist or has been moved."</p>
                            <a href="/guides" class="inline-flex items-center gap-2 px-5 py-2.5 bg-[#f7931a] text-white text-sm font-medium rounded-xl hover:bg-[#f4a949] hover:scale-[1.02] active:scale-[0.98] transition-all duration-200">
                                "Browse Guides"
                                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                                </svg>
                            </a>
                        </div>
                    }>
                        <Route path=path!("/") view=HomePage/>
                        <Route path=path!("/guides") view=GuideSelector/>
                        <Route path=path!("/guides/:level") view=GuideLevelSelector/>
                        <Route path=path!("/guides/:level/:segment") view=GuideTwoSegment/>
                        <Route path=path!("/guides/:level/:platform/:wallet") view=GuideWalletPage/>
                        // Observatory: parent route wraps all sub-pages with shared shell
                        <ParentRoute path=path!("/observatory") view=ObservatoryPage>
                            <Route path=path!("/") view=ObservatoryOverview/>
                            <Route path=path!("/charts/network") view=NetworkChartsPage/>
                            <Route path=path!("/charts/fees") view=FeeChartsPage/>
                            <Route path=path!("/charts/mining") view=MiningChartsPage/>
                            <Route path=path!("/charts/embedded") view=EmbeddedChartsPage/>
                            <Route path=path!("/signaling") view=SignalingPage/>
                            <Route path=path!("/stats") view=StatsSummaryPage/>
                            <Route path=path!("/*any") view=|| view! {
                                <div class="flex flex-col items-center justify-center min-h-[40vh] px-6">
                                    <div class="text-[4rem] font-title text-white/10 font-bold mb-4">"404"</div>
                                    <h1 class="text-xl text-white font-semibold mb-2">"Page not found"</h1>
                                    <p class="text-sm text-white/50 mb-8">"This observatory page doesn\u{2019}t exist."</p>
                                    <a href="/observatory" class="px-5 py-2.5 bg-[#f7931a] text-white text-sm font-medium rounded-xl hover:bg-[#f4a949] transition-all">"Back to Dashboard"</a>
                                </div>
                            }/>
                        </ParentRoute>
                        <Route path=path!("/observatory/learn/protocols") view=ProtocolGuidePage/>
                        // Other routes
                        <Route path=path!("/blog") view=BlogPage/>
                        <Route path=path!("/faq") view=FaqPage/>
                        <Route path=path!("/about") view=AboutPage/>
                    </Routes>
                </main>
                <Footer/>
            </div>
        </Router>
    }
}
