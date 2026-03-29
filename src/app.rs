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
use crate::routes::observatory::{
    ObservatoryPage, ObservatoryOverview, NetworkChartsPage,
    FeeChartsPage, MiningChartsPage, EmbeddedChartsPage, SignalingPage,
};
use crate::routes::observatory::learn::protocols::ProtocolGuidePage;
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

                <meta name="description" content="Free, opinionated Bitcoin self-custody guides. Learn to secure your Bitcoin with mobile & desktop wallets, hardware wallets, and multisig setups. From beginner to advanced."/>
                <meta name="keywords" content="Bitcoin, self-custody, hardware wallet, Coldcard, Sparrow Wallet, multisig, Bitcoin security, Bitcoin guide, self sovereign, Bitcoin node"/>

                // Open Graph
                <meta property="og:title" content="WE HODL BTC — Bitcoin Self-Custody Guides"/>
                <meta property="og:type" content="website"/>
                <meta property="og:url" content="https://www.wehodlbtc.com/"/>
                <meta property="og:image" content="https://www.wehodlbtc.com/metadata_unfurl_image.png"/>
                <meta property="og:description" content="Free, opinionated Bitcoin self-custody guides. Learn to secure your Bitcoin with mobile wallets, hardware wallets, and multisig setups. From beginner to advanced."/>
                <meta property="og:site_name" content="WE HODL BTC"/>

                // Twitter
                <meta name="twitter:card" content="summary_large_image"/>
                <meta name="twitter:title" content="WE HODL BTC — Bitcoin Self-Custody Guides"/>
                <meta name="twitter:description" content="Free, opinionated Bitcoin self-custody guides. Learn to secure your Bitcoin with mobile wallets, hardware wallets, and multisig setups. From beginner to advanced."/>
                <meta name="twitter:url" content="https://www.wehodlbtc.com/"/>
                <meta name="twitter:image" content="https://www.wehodlbtc.com/metadata_unfurl_image.png"/>
                <meta name="twitter:site" content="@bayernatoor"/>
                <meta name="twitter:creator" content="@bayernatoor"/>

                // Favicons
                <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png"/>
                <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png"/>
                <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png"/>
                <link rel="manifest" href="/site.webmanifest"/>
                <link rel="mask-icon" href="/safari-pinned-tab.svg"/>
                <meta name="msapplication-TileColor" content="#123c64"/>
                <meta name="theme-color" content="#123c64"/>

                // ECharts for stats dashboard
                <script defer src="https://cdn.jsdelivr.net/npm/echarts@5/dist/echarts.min.js"></script>
                <script defer src="/stats.js"></script>

                // Fallback for browsers without WebAssembly (e.g. Vanadium)
                <script defer src="/wasm-fallback.js"></script>

                // Schema.org JSON-LD for search engines and LLMs
                <script defer src="/jsonld.js"></script>

                // Image lightbox for guide steps
                <script defer src="/lightbox.js"></script>

                // Collapsible sections within step content
                <script defer src="/sections.js"></script>

                // Analytics
                <script async src="https://www.poeticmetric.com/pm.js"></script>

                <HydrationScripts options/>
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
        <Title text="We Hodl BTC"/>

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
                        </ParentRoute>
                        <Route path=path!("/observatory/learn/protocols") view=ProtocolGuidePage/>
                        // Legacy redirect: /stats -> /observatory
                        <Route path=path!("/stats") view=StatsRedirect/>
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

/// Redirect /stats to /observatory
#[component]
fn StatsRedirect() -> impl IntoView {
    let navigate = leptos_router::hooks::use_navigate();
    navigate("/observatory", leptos_router::NavigateOptions {
        replace: true,
        ..Default::default()
    });
    view! {}
}
