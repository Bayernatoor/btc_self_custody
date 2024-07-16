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
        <Meta property="og:title" content="We Hodl BTC"/>
        <Meta property="og:type" content="website"/>
        <Meta property="og:url" content="https://www.wehodlbtc.com/"/>
        <Meta property="og:image" content="https://www.wehodlbtc.com/metadata_unfurl_image.png"/>
        <Meta property="og:description" content="A Bitcoin self-custody guide"/>

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
                            path="/guides/basic/android/mutiny"
                            view=|| {
                                view! {
                                    <BeginnerWalletInstructions
                                        selected_wallet=WalletName::Mutiny
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
                            path="/guides/basic/ios/mutiny"
                            view=|| {
                                view! {
                                    <BeginnerWalletInstructions
                                        selected_wallet=WalletName::Mutiny
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
