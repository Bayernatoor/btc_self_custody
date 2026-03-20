use leptos::prelude::*;

#[component]
pub fn BackButton(
    #[prop(optional)] _location: String,
    #[prop(optional)] button_image: String,
    #[prop(optional)] reload: bool,
) -> impl IntoView {
    let (reload_page, set_reload_page) = signal(false);

    if reload {
        Effect::new(move |_| {
            if reload_page.get() {
                let _ = window().location().reload();
            }
        });
    }

    view! {
        <a on:click=move |_| set_reload_page.set(true)>
            <button>
                <div class="active:bg-transparent left-0 top-0 h-12 w-12">
                    <img src=format!("{}", button_image) alt="back_button"/>
                </div>
            </button>
        </a>
    }
}
