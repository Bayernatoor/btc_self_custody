use leptos::*;
use crate::extras::back::BackButton;

#[derive(Copy, Clone, Debug)]
pub struct NavbarContext {
    pub guide: WriteSignal<bool>,
}

#[component]
pub fn NavBar(cx: Scope) -> impl IntoView {
     let (guide, set_guide) = create_signal(cx, false);

     //provide_context(cx, NavbarContext {guide: set_guide});

     //log!("Guide: {}", guide());   
    //let setter = use_context::<WriteSignal<bool>>(cx).unwrap();

    let back_button = "./../../left-arrow_10024176.png".to_string() ;
    let (menu_clicked, set_menu_clicked) = create_signal(cx, false);

    view! { cx,
         <div class="bg-[#1a578f] shadow-md text-white sticky top-0 z-10 max-w-10xl mx-auto p-4 flex justify-between items-center font-sans">
            <Show
                when=move || {guide() == true}
                fallback=|cx| view! {cx, 
                        <div>
                            <h1 class="text-2xl font-medium text-white"><a href="/">"Bitcoin Barrack"</a></h1>
                        </div>
                }
            >
                <BackButton button_image=back_button.clone() reload=true/>
            </Show>
                <div class="hidden font-heading space-x-8 lg:flex"> 
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
                            class:hidden={move || menu_clicked() == false}
                          />
                      </svg>
                    </div>
                </div>
// TODO: add event listener to body of app to listen for click when menu is open
// set menu to false if body is clicked.

        <div class="lg:hidden flex flex-col justify-end absolute top-16 right-4 z-10 bg-white border border-gray-200 shadow-md rounded-md p-2 cursor-pointer" class:hidden={move || menu_clicked() == false} class=("animate-slideinfast", move|| menu_clicked())>
            <a href="/guides" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"Guides"</a>
            <a href="/faq" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"Help Desk"</a>
            <a href="/blog" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"Articles"</a>
            <a href="/about" class="block py-2 px-4 font-medium text-xl text-[#6B7990] hover:bg-blue-100" on:click=move |_| set_menu_clicked.set(false)>"About"</a>
        </div>

    }

}


