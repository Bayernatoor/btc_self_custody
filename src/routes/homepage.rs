use leptos::*;

/// Renders the home page of the application.
#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <div class="grid gap-4 mx-auto justify-items-center max-w-5xl my-20 opacity-0 animate-fadeinone lg:grid-cols-2 lg:mt-0 lg:my-64">
            <div class="flex flex-col text-center text-white leading-loose lg:text-left">
                <div class="lg:text-[112px] text-7xl lg-10">
                    <h1>"Be your"<br></br>"own bank"</h1>
                </div>
                <p class="text-2xl mt-6 mb-4">"Learn how to self custody your bitcoin today"</p>
                
                <a href="/guides">
                    <div role="button" class="text-2xl text-center bg-[#f79231] w-44 mx-auto no-underline border-none rounded-xl p-3 hover:bg-[#f4a949] cursor-pointer lg:mx-0">
                       <span>"Start Hodling"</span>
                    </div>
                </a>
            </div>
            <div class="flex flex-col justify-center pb-10 pt-8 lg:pt-0">
                <div class="lg:w-56 lg:h-56 h-36 w-36 mx-auto lg:mt-10">
                    <img src="./../../../bitcoin_logo.png" alt="bitcoin logo"/>
                </div>
            </div>
        </div>
    }
}
