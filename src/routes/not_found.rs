use crate::extras::back::BackButton;
use leptos::*;

/// Renders the About page of the application.
#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div id="about" class="flex flex-col max-w-3xl mx-auto rounded-xl pb-10 animate-fadeinone">
            <div class="flex flex-col p-6 pt-10 max-w-3xl mx-auto">
                    <h1 class="flex justify-center text-[36px] text-white font-semibold">"This Page Doesn't Exist"</h1>
                <div class="flex justify-center pt-4 max-w-sm">
                    <p class="text-sm text-white">"Go Home"</p>
                </div>
                <BackButton _location="/home".to_string() button_image="./../../../golden_home.png".to_string()/>
            </div>
        </div>
    }
}
