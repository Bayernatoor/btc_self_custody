use leptos::logging::log;
use leptos::*;

#[component]
pub fn BackButton(
    #[prop(optional)] _location: String,
    #[prop(optional)] button_image: String,
    #[prop(optional)] reload: bool,
) -> impl IntoView {
    let (reload_page, set_reload_page) = create_signal(false);

    if reload {
        create_effect(move |_| {
            if reload_page() {
                let _ = window().location().reload();
                log!("reload window");
            }
        });
    }

    view! {
        //<div class="flex flex-row bg-transparent sticky z-10 max-w-10xl mx-auto py-4">
            <button on:click=move |_| set_reload_page(true)>
                <div class="left-0 top-0 h-8 w-8 ...">
                    <img class="h-8 w-8" src=format!("{}", button_image) alt="back_button" />
                </div>
            </button>
        //</div>

    }
}
