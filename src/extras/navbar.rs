use leptos::html::Div;
use leptos::*;
use leptos_use::on_click_outside;

#[allow(clippy::redundant_closure)]
#[component]
pub fn NavBar() -> impl IntoView {
    let (menu_clicked, set_menu_clicked) = create_signal(false);
    let navbar_menu = create_node_ref::<Div>();

    // Hook to close main menu when clicking outside
    let _ = on_click_outside(navbar_menu, move |_| {
        set_menu_clicked.set(false);
    });

    view! {
        <div class="bg-[#123c64] text-white sticky top-0 z-10 w-full mx-auto p-8 flex justify-between items-center">
            <div>
                <div class="text-3xl lg:text-4xl font-medium text-white font-title">
                    <a href="/">"WE HODL BTC"</a>
                </div>
            </div>
            <div class="hidden text-2xl space-x-8 lg:flex font-questrial">
                <a href="/guides">
                    "Guides"
                </a>
                <a href="/faq">
                    "Help Desk"
                </a>
                <a href="/blog">
                    "Articles"
                </a>
                <a href="/about">
                    "About"
                </a>
            </div>
            <div class="flex lg:hidden cursor-pointer" on:click=move |_| set_menu_clicked.update(|value| *value = !*value)>
                <div>
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        class="w-8 h-8 transition-transform duration-300 transform"
                        fill="none"
                        viewBox="0 0 24 24"
                        stroke="currentColor"
                    >
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M4 6h16M4 12h16M4 18h16"
                            class:hidden={move || menu_clicked()}
                        />
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M6 18L18 6M6 6l12 12"
                            class:hidden={move || !menu_clicked()}
                        />
                    </svg>
                </div>
            </div>
        </div>
        // TODO: add event listener to body of app to listen for click when menu is open
        // set menu to false if body is clicked.
        <div node_ref=navbar_menu class="lg:hidden flex flex-col justify-end fixed top-16 right-4 z-10 bg-white border border-gray-200 shadow-md rounded-md p-2 cursor-pointer" class:hidden={move || !menu_clicked()} class=("animate-slideinfast", move || menu_clicked())>
            <a href="/guides" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"Guides"</a>
            <a href="/faq" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"Help Desk"</a>
            <a href="/blog" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"Articles"</a>
            <a href="/about" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"About"</a>
        </div>
    }
}
