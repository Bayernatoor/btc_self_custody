use leptos::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage(cx: Scope) -> impl IntoView {
    view! { cx,
            <div class="flex md:flex-row flex-col justify-around max-w-5xl mx-auto md:p-20 md:mt-10 mt-0 p-10 md:text-left opacity-0 animate-fadein font-sans">
                <div class="flex flex-col justify-center md:text-left text-center text-white leading-loose">
                    <div class="text-6xl mt-10">
                        <h1>"Be your own bank"</h1>
                    </div>
                    <p class="text-base mt-6 mb-2">"Learn how to self custody your bitcoin today"</p>
                    <a class="text-base text-center bg-[#f79231] w-32 no-underline border-none rounded-xl py-2 hover:bg-[#f4a949] cursor-pointer" href="/guides">"Start Hodling"</a>
                </div>
                <div class="w-48 h-48 mx-auto mt-10">
                    <img src="./../../bitcoin_logo.png" alt="bitcoin logo"/>
                </div>
            </div>
       // <footer class="info">
       //     <p>"Knowledge is Freedom"</p>
       //     <p>"Created by "<a target="_blank" rel="noopener noreferrer" href="https://github.com/Bayernatoor">"Bayernator"</a></p>
       // </footer>

    }
}
