use leptos::*;

#[component]
pub fn NavBar(cx: Scope) -> impl IntoView {
    view! { cx,
         <div class="bg-[#1a578f] shadow-md text-white sticky top-0 z-10 max-w-10xl mx-auto p-4 flex justify-between items-center font-sans">
            <nav class="text-2xl">
                <h1><a href="/">"The Bitcoin Barrack"</a></h1>
            </nav>

            <nav class="flex items-center justify-end gap-2">
                <a href="/faq">
                    "FAQ"
                </a>
                <a href="/blog">
                    "Blog"
                </a>
                <a href="/about">
                   "About"
                </a>
            </nav>
        </div>
    }
}
