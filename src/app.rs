use crate::routes::about::*;
use crate::routes::advanced::*;
use crate::routes::beginner::*;
use crate::routes::blog::*;
use crate::routes::faq::*;
use crate::routes::guideselector::*;
use crate::routes::homepage::*;
use crate::routes::intermediate::*;
use crate::routes::navbar::*;
use leptos::*;
use leptos_meta::*;
use leptos_router::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context(cx);

    view! {
        cx,
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>

        // sets the document title
        <Title text="Bitcoin Self Custody"/>

        // sets the body background color throughout the app
        <Body class="bg-[#1a578f]"/>

        // content for this welcome page
        <Router>
            <NavBar />
            <main>
                <Routes>
                    <Route path="/" view=|cx| view! { cx, <HomePage/> }/>
                    <Route path="/guides" view=|cx| view! { cx, <GuideSelector/> }/>
                    <Route path="/guides/beginner/android" view=|cx| view! { cx, <BeginnerPageAndroid/> }/>
                    <Route path="/guides/beginner/ios" view=|cx| view! { cx, <BeginnerPageIOS/> }/>
                    <Route path="/guides/intermediate/android" view=|cx| view! { cx, <IntermediatePage/> }/>
                    <Route path="/guides/intermediate/ios" view=|cx| view! { cx, <IntermediatePage/> }/>
                    <Route path="/guides/intermediate/desktop" view=|cx| view! { cx, <IntermediatePage/> }/>
                    <Route path="/guides/advanced" view=|cx| view! { cx, <AdvancedPage/> }/>
                    <Route path="/blog" view=|cx| view! { cx, <BlogPage/> }/>
                    <Route path="/faq" view=|cx| view! { cx, <FaqPage/> }/>
                    <Route path="/about" view=|cx| view! { cx, <AboutPage/> }/>
                </Routes>
            </main>
        </Router>
    }
}
