use leptos::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="grid gap-2 md:gap-2 mx-auto justify-items-center max-w-3xl mt-20 opacity-0 animate-fadeinone md:grid-cols-1 lg:grid-cols-2 xl:grid-cols-2 md:my-28 lg:pb-28 md:max-w-4xl lg:max-w-5xl">
            <div class="flex flex-col text-center text-white leading-loose md:text-center lg:text-left xl:leading-relaxed xl:text-left md:pt-10 lg:pt-0 xl:pt-0">
                <div class="text-7xl lg:text-[112px] xl:text-[112px]">
                    <h1>"Be your own bank"</h1>
                </div>
                <div class="lg:hidden flex flex-col justify-center pb-10 pt-6 lg:pt-0 xl:pt-0">
                    <div class="h-auto w-32 md:w-36 md:h-36 lg:w-48 lg:h-48 xl:w-56 xl:h-56 mx-auto lg:mt-10 xl:mt-12">
                        <img src="./../../../bitcoin_logo.png" alt="bitcoin logo"/>
                    </div>
                </div>
                <p class="text-xl max-w-2xl px-6 mb-6 md:px-0 md:text-2xl md:my-10">
                    "Opinionated guides that cut through the noise,
                    easily learn to self custody your bitcoin today"

                </p>
                <a href="/guides">
                    <button
                        role="button"
                        class="text-xl text-center bg-[#f79231] w-40 lg:w-44 mx-auto font-semibold no-underline border-none rounded-xl p-3 hover:bg-[#f4a949] cursor-pointer"
                    >
                        <span>"Start Hodling"</span>
                    </button>
                </a>
            </div>
            <div class="invisible flex flex-col justify-center lg:visible">
                <div class="h-auto w-32 md:w-48 lg:w-64 mx-auto">
                    <img src="./../../../bitcoin_logo.png" alt="bitcoin logo"/>
                </div>
            </div>
        </div>
    }
}
