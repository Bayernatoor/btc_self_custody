use leptos::prelude::*;
use leptos_meta::*;

/// Renders the Blog page of the application.
#[component]
pub fn BlogPage() -> impl IntoView {
    view! {
        <Title text="Articles | WE HODL BTC"/>
        <Meta name="description" content="Bitcoin articles covering self-custody, network analysis, security best practices, and blockchain data insights."/>
        <Link rel="canonical" href="https://www.wehodlbtc.com/blog"/>

        <div
            id="about"
            class="grid gap-6 max-w-6xl mx-auto pb-20 px-6 animate-fadeinone grid-rows-[auto_auto_1fr] lg:gap-8 lg:px-8 md:my-28"
        >
            // Section 1: Title and Subtitle
            <div class="flex flex-col mx-auto px-4 pt-10 lg:pt-0">
                <h1 class="text-center text-3xl text-[#f7931a] font-semibold font-title leading-tight lg:text-4xl">
                    "Posts"
                </h1>
                <div class="text-center max-w-sm mx-auto pt-4">
                    <p class="text-base text-white">"Coming...soon \u{2122}"</p>
                </div>
            </div>

            // Section 2: Under Construction Image
            <div class="px-4 lg:pt-0 lg:px-0 flex justify-center">
                <img
                    src="/img/writing.jpg"
                    alt="Under construction image"
                    class="max-w-full h-auto rounded-md"
                />
            </div>
        </div>
    }
}
