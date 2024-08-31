use crate::extras::footer::*;
use crate::extras::navbar::*;
use crate::routes::about::*;
use crate::routes::advanced::*;
use crate::routes::beginner::*;
use crate::routes::blog::*;
use crate::routes::faq::*;
use crate::routes::guideselector::*;
use crate::routes::homepage::*;
use crate::routes::intermediate::*;
//use crate::routes::not_found::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/we_hodl_btc.css"/>
        <Meta name="We Hodl BTC" content="A bitcoin self-custody guide"/>

        // open graph properties
        <Meta property="og:title" content="We Hodl BTC"/>
        <Meta property="og:type" content="website"/>
        <Meta property="og:url" content="https://www.wehodlbtc.com/"/>
        <Meta property="og:image" content="https://www.wehodlbtc.com/metadata_unfurl_image.png"/>
        <Meta property="og:description" content="A Bitcoin self-custody guide"/>

        // Twitter OG properties
        <Meta name="twitter:card" content="summary_large_image"/>
        <Meta name="twitter:title" content="We Hodl BTC"/>
        <Meta name="twitter:description" content="A Bitcoin self-custody guide"/>
        <Meta name="twitter:url" content="https://www.wehodlbtc.com/"/>
        <Meta name="twitter:image" content="https://www.wehodlbtc.com/metadata_unfurl_image.png"/>
        <Meta name="twitter:site" content="@bayernatoor"/>
        <Meta name="twitter:creator" content="@bayernatoor"/>

        // favicons
        <Link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png"/>
        <Link rel="icon" type_="image/png" sizes="32x32" href="/favicon-32x32.png"/>
        <Link rel="icon" type_="image/png" sizes="16x16" href="/favicon-16x16.png"/>
        <Link rel="manifest" href="/site.webmanifest"/>
        <Link rel="mask-icon" href="/safari-pinned-tab.svg"/>
        <Meta name="msapplication-TileColor" content="#123c64"/>
        <Meta name="theme-color" content="#123c64"/>

        // Poetic Metric - https://www.poeticmetric.com/
        <script async src="https://www.poeticmetric.com/pm.js"></script>

        // sets the document title
        <Title text="We Hodl BTC"/>

        // sets the body background color throughout the app
        // <Body class="bg-[#1a578f]"/>
        <Body class="bg-[#123c64]"/>

        // Routes
        <Router>
            <div class="flex flex-col justify-between h-screen">
                <NavBar/>
                <main>
                    <Routes>
                        <Route path="/" view=|| view! { <HomePage/> }/>
                        <Route path="/guides" view=|| view! { <GuideSelector/> }/>
                        // Basic guide routes
                        <Route path="/guides/basic/desktop" view=|| view! { <RenderDesktopPage/> }/>
                        <Route
                            path="/guides/basic/desktop/sparrow"
                            view=|| {
                                view! {
                                    <BeginnerWalletInstructions
                                        selected_wallet=WalletName::Sparrow
                                        ios=false
                                    />
                                }
                            }
                        />

                        <Route path="/guides/basic/android" view=|| view! { <RenderAndroidPage/> }/>
                        <Route
                            path="/guides/basic/android/green"
                            view=|| {
                                view! {
                                    <BeginnerWalletInstructions
                                        selected_wallet=WalletName::Green
                                        ios=false
                                    />
                                }
                            }
                        />

                        <Route
                            path="/guides/basic/android/blue"
                            view=|| {
                                view! {
                                    <BeginnerWalletInstructions
                                        selected_wallet=WalletName::Blue
                                        ios=false
                                    />
                                }
                            }
                        />

                        <Route path="/guides/basic/ios" view=|| view! { <RenderIosPage/> }/>
                        <Route
                            path="/guides/basic/ios/blue"
                            view=|| {
                                view! {
                                    <BeginnerWalletInstructions
                                        selected_wallet=WalletName::Blue
                                        ios=true
                                    />
                                }
                            }
                        />

                        <Route
                            path="/guides/basic/ios/green"
                            view=|| {
                                view! {
                                    <BeginnerWalletInstructions
                                        selected_wallet=WalletName::Green
                                        ios=true
                                    />
                                }
                            }
                        />

                        // Intermediate guide routes
                        <Route
                            path="/guides/intermediate/desktop"
                            view=|| view! { <IntermediateIntroPage/> }
                        />
                        <Route
                            path="/guides/intermediate/hardware-wallet"
                            view=|| view! { <IntermediateHardwarePage/> }
                        />
                        <Route
                            path="/guides/intermediate/node"
                            view=|| view! { <IntermediateNodePage/> }
                        />
                        // Advanced guide routes
                        <Route path="/guides/advanced/desktop" view=|| view! { <AdvancedPage/> }/>
                        // other routes
                        <Route path="/blog" view=|| view! { <BlogPage/> }/>
                        <Route path="/faq" view=|| view! { <FaqPage/> }/>
                        <Route path="/about" view=|| view! { <AboutPage/> }/>
                    // <Route path="/*" view=|| view! {<NotFound/> }/>
                    </Routes>
                </main>
                <Footer/>
            </div>
        </Router>
    }
}
