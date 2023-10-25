use leptos::*;

/// Renders the About page of the application.
#[component]
pub fn AboutPage() -> impl IntoView {

    view! {
        <div id="about" class="flex flex-col max-w-3xl mx-auto rounded-xl pb-10 animate-fadein">
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-[36px] text-white font-semibold">"About"</h1>
                <div class="flex justify-center pt-4 max-w-sm">
                    <p class="text-sm text-white">"Just a simple website to help you self-custody your bitcoin."</p>
                </div>
            </div>
        </div>
    }

}
