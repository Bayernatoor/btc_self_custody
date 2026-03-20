use crate::extras::footer::Footer;
use crate::extras::navbar::NavBar;
use crate::routes::about::AboutPage;
use crate::routes::advanced::AdvancedPage;
use crate::routes::beginner::*;
use crate::routes::blog::BlogPage;
use crate::routes::faq::FaqPage;
use crate::routes::guideselector::GuideSelector;
use crate::routes::homepage::HomePage;
use crate::routes::intermediate::*;
use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>

                <meta name="description" content="A bitcoin self-custody guide"/>

                // Open Graph
                <meta property="og:title" content="We Hodl BTC"/>
                <meta property="og:type" content="website"/>
                <meta property="og:url" content="https://www.wehodlbtc.com/"/>
                <meta property="og:image" content="https://www.wehodlbtc.com/metadata_unfurl_image.png"/>
                <meta property="og:description" content="A Bitcoin self-custody guide"/>

                // Twitter
                <meta name="twitter:card" content="summary_large_image"/>
                <meta name="twitter:title" content="We Hodl BTC"/>
                <meta name="twitter:description" content="A Bitcoin self-custody guide"/>
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

// Wrapper components for routes that need props
#[component]
fn SparrowDesktopGuide() -> impl IntoView {
    view! { <BeginnerWalletInstructions selected_wallet=WalletName::Sparrow ios=false/> }
}

#[component]
fn GreenAndroidGuide() -> impl IntoView {
    view! { <BeginnerWalletInstructions selected_wallet=WalletName::Green ios=false/> }
}

#[component]
fn BlueAndroidGuide() -> impl IntoView {
    view! { <BeginnerWalletInstructions selected_wallet=WalletName::Blue ios=false/> }
}

#[component]
fn BlueIosGuide() -> impl IntoView {
    view! { <BeginnerWalletInstructions selected_wallet=WalletName::Blue ios=true/> }
}

#[component]
fn GreenIosGuide() -> impl IntoView {
    view! { <BeginnerWalletInstructions selected_wallet=WalletName::Green ios=true/> }
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
                        // Basic guide routes
                        <Route path=path!("/guides/basic/desktop") view=RenderDesktopPage/>
                        <Route path=path!("/guides/basic/desktop/sparrow") view=SparrowDesktopGuide/>
                        <Route path=path!("/guides/basic/android") view=RenderAndroidPage/>
                        <Route path=path!("/guides/basic/android/green") view=GreenAndroidGuide/>
                        <Route path=path!("/guides/basic/android/blue") view=BlueAndroidGuide/>
                        <Route path=path!("/guides/basic/ios") view=RenderIosPage/>
                        <Route path=path!("/guides/basic/ios/blue") view=BlueIosGuide/>
                        <Route path=path!("/guides/basic/ios/green") view=GreenIosGuide/>
                        // Intermediate guide routes
                        <Route path=path!("/guides/intermediate/desktop") view=IntermediateIntroPage/>
                        <Route path=path!("/guides/intermediate/hardware-wallet") view=IntermediateHardwarePage/>
                        <Route path=path!("/guides/intermediate/node") view=IntermediateNodePage/>
                        // Advanced guide routes
                        <Route path=path!("/guides/advanced/desktop") view=AdvancedPage/>
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
