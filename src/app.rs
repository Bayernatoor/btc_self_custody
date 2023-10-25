
use crate::routes::about::*;
use crate::routes::advanced::*;
use crate::routes::beginner::*;
use crate::routes::blog::*;
use crate::routes::faq::*;
use crate::routes::guideselector::*;
use crate::routes::homepage::*;
use crate::routes::intermediate::*;
use crate::extras::navbar::*;
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
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>

        // sets the document title
        <Title text="Bitcoin Self Custody"/>

        // sets the body background color throughout the app
        <Body class="bg-[#1a578f]"/>

        // Routes 
        <Router>
            <NavBar/>
            <main>
                <Routes>
                    <Route path="/" view=|| view! {<HomePage/> }/>
                    <Route path="/guides" view=|| view! {<GuideSelector/> }/>
                    <Route path="/guides/beginner/android" view=|| view! {<BeginnerPageAndroid />}/>
                    <Route path="/guides/beginner/android/samourai" view=|| view! {<BeginnerWalletInstructions samourai=true blue=false _green=false ios=false/>}/>
                    <Route path="/guides/beginner/android/blue" view=|| view! {<BeginnerWalletInstructions samourai=false blue=true _green=false ios=false/>}/>
                    <Route path="/guides/beginner/ios" view=|| view! {<BeginnerPageIOS/> }/>
                    <Route path="/guides/beginner/ios/blue" view=|| view! {<BeginnerWalletInstructions _green=false blue=true samourai=false ios=true/>}/>
                    <Route path="/guides/beginner/ios/blockstream" view=|| view! {<BeginnerWalletInstructions _green=true blue=false samourai=false ios=true/>}/>
                    <Route path="/guides/intermediate/android" view=|| view! {<IntermediatePage/> }/>
                    <Route path="/guides/intermediate/ios" view=|| view! {<IntermediatePage/> }/>
                    <Route path="/guides/intermediate/desktop" view=|| view! {<IntermediatePage/> }/>
                    <Route path="/guides/advanced" view=|| view! {<AdvancedPage/> }/>
                    <Route path="/blog" view=|| view! {<BlogPage/> }/>
                    <Route path="/faq" view=|| view! {<FaqPage/> }/>
                    <Route path="/about" view=|| view! {<AboutPage/> }/>
                </Routes>
            </main>
        </Router>
    }
}

