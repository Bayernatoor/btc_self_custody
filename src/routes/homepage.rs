use leptos::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
            <div class="flex flex-col justify-around items-center max-w-5xl mx-auto  mt-0 p-5 opacity-0 animate-fadeinone font-sans md:flex-row md:p-20 md:mt-10 md:text-left md:transform md:scale-125">
                <div class="flex flex-col justify-center md:text-left text-center text-white leading-loose">
                    <div class="md:text-8xl text-6xl mt-10">
                        <h1>"Be your" <br></br> "own bank"</h1>
                    </div>
                    <p class="text-xl mt-6 mb-4">"Learn how to self custody your bitcoin today"</p>
                    <a class="text-lg text-center bg-[#f79231] md:mx-0 mx-auto w-36 no-underline border-none rounded-xl py-2 hover:bg-[#f4a949] cursor-pointer" href="/guides">"Start Hodling"</a>
                </div>
                <div class="md:w-48 md:h-48 h-36 w-36 mx-auto mt-10">
                    <img src="./../../../bitcoin_logo.png" alt="bitcoin logo"/>
                </div>
            </div>
        //<footer class="mx-auto text-white">
        //    <p>"Knowledge is Freedom"</p>
        //    <p>"Created by "<a target="_blank" rel="noopener noreferrer" href="https://github.com/Bayernatoor">"Bayernator"</a></p>
        //</footer>

    }
}
