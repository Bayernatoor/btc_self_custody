use leptos::*;

/// Renders the Blog page of the application.
#[component]
pub fn BlogPage() -> impl IntoView {

    view! {
        <div id="about" class="flex flex-col max-w-3xl mx-auto rounded-xl pb-10 animate-fadein">
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-[36px] text-white font-semibold">"Posts"</h1>
                <div class="flex justify-center pt-4 max-w-sm">
                    <p class="text-sm text-white">"Random thoughts about bitcoin and stuff."</p>
                </div>
            </div>
        </div>
    }
}
