use leptos::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage(cx: Scope) -> impl IntoView {

    view! { cx,
        <div class="flex lg:flex-row justify-around max-w-4xl w-4/5 mx-auto p-20  mt-10 text-left opacity-0 animate-fadein">
            <div class="flex flex-col justify-around mr-12 items-start text-white">
                <div class="text-6xl mt-10 leading-tight">
                <h1>"Become your"</h1>
                <h1>"own bank"</h1>
                </div>
                <p class="text-base">"Learn how to self custody your bitcoin"</p>
                <div class="bg-[#f79231] flex text-center no-underline border-none rounded-xl pt-4 pr-6">
                    <a href="/guides">"Start Hodling"</a>
                </div>
            </div>
            <div class="flex items-center w-64 h-64 mt-10 ml-12">
                <img src="./../../bitcoin_logo.png" alt="bitcoin logo"/>
            </div>
        </div>
   // <footer class="info">
   //     <p>"Knowledge is Freedom"</p>
   //     <p>"Created by "<a target="_blank" rel="noopener noreferrer" href="https://github.com/Bayernatoor">"Bayernator"</a></p>
   // </footer>

}
}
