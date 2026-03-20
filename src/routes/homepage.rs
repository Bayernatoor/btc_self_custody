use leptos::prelude::*;
use leptos_meta::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Title text="WE HODL BTC — Bitcoin Self-Custody Guides"/>

        <section aria-label="Hero" class="grid gap-2 mx-auto justify-items-center max-w-3xl mt-14 px-6 opacity-0 animate-fadeinone md:grid-cols-1 lg:grid-cols-2 md:my-24 lg:pb-24 md:max-w-4xl lg:max-w-5xl lg:px-8">
            <div class="flex flex-col text-center text-white leading-normal md:text-center lg:text-left md:pt-8 lg:pt-0">
                <h1 class="text-5xl font-title font-normal tracking-tight md:text-6xl lg:text-[5rem]">
                    "Be your" <br/> "own bank"
                </h1>
                <div class="lg:hidden flex flex-col justify-center pb-8 pt-5">
                    <div class="h-auto w-24 md:w-28 mx-auto">
                        <img src="./../../../bitcoin_logo.png" alt="Bitcoin logo" width="112" height="112"/>
                    </div>
                </div>
                <p class="text-base max-w-xl mb-6 md:text-lg lg:text-xl md:my-8 leading-relaxed">
                    "Opinionated guides that cut through the noise to help you easily self-custody your Bitcoin."
                </p>
                <a href="/guides">
                    <button
                        role="button"
                        class="text-sm lg:text-base text-center bg-[#f79231] w-36 lg:w-40 mx-auto font-semibold no-underline border-none rounded-lg py-2.5 lg:py-3 px-4 lg:px-5 hover:bg-[#f4a949] cursor-pointer shadow-md hover:shadow-lg transition-all duration-300 lg:mx-0"
                    >
                        <span>"Start Hodling"</span>
                    </button>
                </a>
            </div>
            <div class="invisible flex flex-col justify-center lg:visible">
                <div class="h-auto w-28 md:w-40 lg:w-60 mx-auto">
                    <img src="./../../../bitcoin_logo.png" alt="Bitcoin logo" width="208" height="208"/>
                </div>
            </div>
        </section>
    }
}
