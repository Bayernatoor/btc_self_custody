use crate::extras::navbar::*;
use crate::routes::about::*;
use crate::routes::advanced::*;
use crate::routes::beginner::*;
use crate::routes::blog::*;
use crate::routes::faq::*;
use crate::routes::guideselector::*;
use crate::routes::homepage::*;
use crate::routes::intermediate::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/btc_self_custody.css"/>

        // sets the document title
        <Title text="Bitcoin Self Custody"/>

        // sets the body background color throughout the app
        //<Body class="bg-[#1a578f]"/>
        <Body class="bg-[#123c64]"/>

        // Routes
        <Router>
            <NavBar/>
            <main>
                <Routes>
                    <Route path="/" view=|| view! {<HomePage/> }/>
                    <Route path="/guides" view=|| view! {<GuideSelector/> }/>
                    // Basic guide routes
                    <Route path="/guides/basic/desktop" view=|| view! {<RenderDesktopPage/>}/>
                    <Route path="/guides/basic/desktop/sparrow" view=|| view! {<BeginnerWalletInstructions selected_wallet=WalletName::Sparrow ios=false/>}/>
                    <Route path="/guides/basic/android" view=|| view! {<RenderAndroidPage/>}/>
                    <Route path="/guides/basic/android/samourai" view=|| view! {<BeginnerWalletInstructions selected_wallet=WalletName::Samourai ios=false/>}/>
                    <Route path="/guides/basic/android/blue" view=|| view! {<BeginnerWalletInstructions selected_wallet=WalletName::Blue ios=false/>}/>
                    <Route path="/guides/basic/ios" view=|| view! {<RenderIosPage/> }/>
                    <Route path="/guides/basic/ios/blue" view=|| view! {<BeginnerWalletInstructions selected_wallet=WalletName::Blue ios=true/>}/>
                    <Route path="/guides/basic/ios/blockstream" view=|| view! {<BeginnerWalletInstructions selected_wallet=WalletName::Green ios=true/>}/>
                    // Intermediate guide routes
                    <Route path="/guides/intermediate" view=|| view! {<IntermediatePage/> }/>
                    // Advanced guide routes
                    <Route path="/guides/advanced" view=|| view! {<AdvancedPage/> }/>
                    <Route path="/blog" view=|| view! {<BlogPage/> }/>
                    <Route path="/faq" view=|| view! {<FaqPage/> }/>
                    <Route path="/about" view=|| view! {<AboutPage/> }/>
                    //<Route path="/*" view=|| view! {<NotFound/> }/>
                </Routes>
            </main>
        </Router>
    }
}
