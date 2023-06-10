use leptos::*;
use leptos_router::*;

#[component]
pub fn NavBar(cx: Scope) -> impl IntoView {
    view! { cx, 
         <header class="header">
            <nav class="text-3xl font-bold underline">
                <h1><A href="/">"The Bitcoin Barrack"</A></h1>
            </nav>
            <nav class="nav_links">
                <A href="/faq">
                    "FAQ"
                </A>
                <A href="/blog">
                    "Blog"
                </A>
                <A href="/about">
                   "About"
                </A>
            </nav>
        </header>
    }
}
