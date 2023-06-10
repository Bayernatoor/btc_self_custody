use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use crate::routes::navbar::*;
use crate::routes::homepage::*;
use crate::routes::guideselector::*;
use crate::routes::beginner::*;
use crate::routes::intermediate::*;
use crate::routes::advanced::*;

#[component]
pub fn App(cx: Scope) -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context(cx);

    view! {
        cx,
        //<Meta name="color-scheme" content="dark" />
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/leptos_start.css"/>

        // sets the document title
        <Title text="Bitcoin Self Custody"/>

        // content for this welcome page
        <Router>
            <NavBar /> 
            <main>
                <Routes>
                    <Route path="/" view=|cx| view! { cx, <HomePage/> }/>
                    <Route path="/guides" view=|cx| view! { cx, <GuideSelector/> }/>
                    <Route path="/guides/beginner/android" view=|cx| view! { cx, <BeginnerPageAndroid/> }/>
                    <Route path="/guides/beginner/ios" view=|cx| view! { cx, <BeginnerPageIOS/> }/>
                    <Route path="/guides/intermediate" view=|cx| view! { cx, <IntermediatePage/> }/>
                    <Route path="/guides/advanced" view=|cx| view! { cx, <AdvancedPage/> }/>
                </Routes>
            </main>
        </Router>
    }
}

