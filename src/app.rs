//! App shell and router.
//!
//! `shell()` renders the outer HTML document (head, meta, scripts).
//! `App` sets up the Leptos router with all page routes.
//!
//! Route structure:
//!   /                              → HomePage
//!   /guides                        → GuideSelector (pick level + platform)
//!   /guides/:level/:segment        → GuideTwoSegment (level intro or step page)
//!   /guides/:level/:platform/:wallet → GuideWalletPage (wallet stepper)
//!   /blog, /faq, /about            → Static pages

use crate::extras::footer::Footer;
use crate::extras::navbar::NavBar;
use crate::routes::about::AboutPage;
use crate::routes::blog::BlogPage;
use crate::routes::faq::FaqPage;
use crate::routes::guide::{GuideTwoSegment, GuideWalletPage};
use crate::routes::guideselector::GuideSelector;
use crate::routes::homepage::HomePage;
use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

/// Outer HTML shell — rendered once on the server, wraps the hydrated App.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>

                <meta name="description" content="Free, opinionated Bitcoin self-custody guides. Learn to secure your Bitcoin with mobile wallets, hardware wallets, and multisig setups. From beginner to advanced."/>
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
                    <Routes fallback=|| view! { <p>"Page not found."</p> }>
                        <Route path=path!("/") view=HomePage/>
                        <Route path=path!("/guides") view=GuideSelector/>
                        // Unified guide routes (2 parameterized routes replace 14 static ones)
                        <Route path=path!("/guides/:level/:segment") view=GuideTwoSegment/>
                        <Route path=path!("/guides/:level/:platform/:wallet") view=GuideWalletPage/>
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
