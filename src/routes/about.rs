use leptos::*;
use leptos_router::*;

/// Renders the About page of the application.
#[component]
pub fn AboutPage(cx: Scope) -> impl IntoView {
    view! { cx,
        <h1>"About Page"</h1>
    }
}
