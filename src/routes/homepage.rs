use leptos::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="grid gap-2 md:gap-2 mx-auto justify-items-center max-w-3xl my-20 opacity-0 animate-fadeinone md:grid-cols-1 lg:grid-cols-2 xl:grid-cols-2 md:my-28 lg:pb-28 md:max-w-4xl lg:max-w-5xl">
            <div class="flex flex-col text-center text-white leading-loose md:text-center lg:text-left xl:leading-relaxed xl:text-left md:pt-10 lg:pt-0 xl:pt-0">
                <div class="text-7xl lg:text-[112px] xl:text-[112px] lg-10">
                    <h1>"Be your" <br/> "own bank"</h1>
                </div>
                <p class="text-xl px-6 mt-6 md:px-0 md:text-2xl ">
                    "Opinionated guides that cut through the noise"
                </p>
                <p class="text-xl px-6 mb-6 md:px-0 md:text-2xl ">
                    "Easily learn to self custody your bitcoin today"
                </p>

                <a href="/guides">
                    <button
                        role="button"
                        class="text-xl md:text-xl text-center bg-[#f79231] w-36 md:w-40 lg:w-44 xl:w-48 mx-auto no-underline border-none rounded-xl p-3 hover:bg-[#f4a949] cursor-pointer lg:mx-0"
                    >
                        <span>"Start Hodling"</span>
                    </button>
                </a>
            </div>
            <div class="flex flex-col justify-center pb-10 pt-6 lg:pt-0 xl:pt-0">
                <div class="h-auto w-32 md:w-36 md:h-36 lg:w-48 lg:h-48 xl:w-56 xl:h-56 mx-auto lg:mt-10 xl:mt-12">
                    <img src="./../../../bitcoin_logo.png" alt="bitcoin logo"/>
                </div>
            </div>
        </div>
    }
}
