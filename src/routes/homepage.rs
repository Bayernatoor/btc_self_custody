use leptos::*;
use leptos_router::*;

/// Renders the home page of your application.
#[component]
pub fn HomePage(cx: Scope) -> impl IntoView {

    view! { cx,
    
    <div class="homepage_container">
        <div class="homepage_title_container">
            <div>
            <h1>"Become your"</h1>
            </div>
            <div>
            <h1>"own bank"</h1>
            </div>
            <p>"Learn how to self custody your bitcoin"</p>
            <a href="/guides">"Start Hodling"</a>
        </div>
        <div class="homepage_img_container">
            <img src="./../../bitcoin_logo.png" alt="bitcoin logo" height="auto" width="100%"/>
        </div>
    </div>
   // <footer class="info">
   //     <p>"Knowledge is Freedom"</p>
   //     <p>"Created by "<a target="_blank" rel="noopener noreferrer" href="https://github.com/Bayernatoor">"Bayernator"</a></p>
   // </footer>

}
}
