use leptos::ev::TouchEvent;
use leptos::html::Div;
use leptos::prelude::*;
use leptos_use::{on_click_outside_with_options, OnClickOutsideOptions};

#[allow(clippy::redundant_closure)]
#[component]
pub fn NavBar() -> impl IntoView {
    let (menu_open, set_menu_open) = signal(false);
    let navbar_menu = NodeRef::<Div>::new();

    let _ = on_click_outside_with_options(
        navbar_menu,
        move |_event| set_menu_open.set(false),
        OnClickOutsideOptions::default().ignore(["#navbar_hamburger_menu"]),
    );

    let on_touchstart = move |event: TouchEvent| {
        event.stop_immediate_propagation();
        set_menu_open.set(!menu_open.get());
    };

    view! {
        <nav class="bg-[#123c64] text-white sticky top-0 z-30 w-full">
            <div class="mx-auto px-6 py-3 2xl:px-8 2xl:py-4 flex justify-between items-center border-b border-white/15">
                // Logo
                <a href="/" class="text-xl 2xl:text-2xl font-medium font-title hover:text-[#f7931a] transition-colors">
                    "WE HODL BTC"
                </a>

                // Desktop nav
                <div class="hidden text-base 2xl:text-lg space-x-7 lg:flex">
                    <a href="/guides" class="text-white/80 hover:text-white transition-colors">"Guides"</a>
                    <a href="/faq" class="text-white/80 hover:text-white transition-colors">"Help Desk"</a>
                    <a href="/about" class="text-white/80 hover:text-white transition-colors">"About"</a>
                </div>

                // Mobile hamburger
                <button
                    id="navbar_hamburger_menu"
                    on:touchstart=on_touchstart
                    class="flex lg:hidden p-1.5 rounded-lg hover:bg-white/10 transition-colors cursor-pointer"
                    aria-label="Toggle menu"
                >
                    <svg
                        class="w-5 h-5 transition-transform duration-300"
                        class:rotate-90=move || menu_open.get()
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                        stroke-width="2"
                    >
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            d="M4 6h16M4 12h16M4 18h16"
                            class:hidden=move || menu_open.get()
                        />
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            d="M6 18L18 6M6 6l12 12"
                            class:hidden=move || !menu_open.get()
                        />
                    </svg>
                </button>
            </div>

            // Mobile dropdown
            <div
                node_ref=navbar_menu
                class="lg:hidden fixed top-12 right-4 z-40 min-w-44 overflow-hidden"
                class:hidden=move || !menu_open.get()
            >
                <div
                    class="bg-[#0e3052] border border-white/15 rounded-xl shadow-2xl p-1.5 animate-slidedown"
                >
                    <a
                        href="/guides"
                        class="flex items-center gap-3 px-4 py-2.5 rounded-lg text-sm text-white/80 hover:bg-white/10 hover:text-white transition-all duration-150"
                        on:click=move |_| set_menu_open.set(false)
                    >
                        <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"/>
                        </svg>
                        "Guides"
                    </a>
                    <a
                        href="/faq"
                        class="flex items-center gap-3 px-4 py-2.5 rounded-lg text-sm text-white/80 hover:bg-white/10 hover:text-white transition-all duration-150"
                        on:click=move |_| set_menu_open.set(false)
                    >
                        <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                        </svg>
                        "Help Desk"
                    </a>
                    <a
                        href="/about"
                        class="flex items-center gap-3 px-4 py-2.5 rounded-lg text-sm text-white/80 hover:bg-white/10 hover:text-white transition-all duration-150"
                        on:click=move |_| set_menu_open.set(false)
                    >
                        <svg class="w-4 h-4 text-[#f7931a]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
                        </svg>
                        "About"
                    </a>
                </div>
            </div>
        </nav>
    }
}
