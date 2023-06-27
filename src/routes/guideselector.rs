use leptos::ev::MouseEvent;
use leptos::*;

#[component]
fn BeginnerButton<F>(cx: Scope, on_click: F) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    view! { cx,
            <button class="flex flex-col p-2 max-w-sm mx-auto w-64 bg-slate-100 rounded-xl flex items-center" on:click=on_click>
                <div class="text-2xl font-semibold text-[#f79231]">"Beginner"</div>
                <p class="text-sm text-[#1a578f] mt-2">"What the hell is Bitcoin?"</p>
            </button>
    }
}

#[component]
fn IntermediateButton<F>(cx: Scope, on_click: F) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    view! { cx,
            <button class="flex flex-col p-2 max-w-sm mx-auto w-64 bg-slate-100 rounded-xl flex items-center my-5" on:click=on_click>
                <div class="text-2xl font-semibold text-[#f79231]">"Intermediate"</div>
                <p class="text-sm text-[#1a578f] mt-2">"I've got a wallet, I want to go deeper"</p>
            </button>
    }
}

#[component]
fn AdvancedButton<F>(cx: Scope, on_click: F) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    view! { cx,
            <button class="flex flex-col p-2 max-w-sm mx-auto w-64 bg-slate-100 rounded-xl flex items-center" on:click=on_click>
                <div class="text-2xl font-semibold text-[#f79231]">"Advanced"</div>
                <p class="text-sm text-[#1a578f] mt-2">"It's time to jump down the rabbit hole"</p>
            </button>
    }
}

//#[component]
//pub fn GenericButton<F, G>(cx: Scope, on_click: F, level: G) -> impl IntoView
//    where
//        F: Fn(MouseEvent) + 'static,
//        G: Fn()
//    {
//        let result = match level {
//            beginner =>
//                 view! {cx,
//                            <button class="button-guides" on:click=on_click  >
//                                <h2>"Beginner"</h2>
//                                <p>"What the hell is Bitcoin?"</p>
//                            </button>
//                    },
//            intermediate =>
//                 view! {cx,
//                            <button class="button-guides" on:click=on_click  >
//                                <h2>"Intermediate"</h2>
//                                <p>"I've got a wallet, I want to go deeper"</p>
//                            </button>
//                    },
//            advanced =>
//                 view! {cx,
//                            <button class="button-guides" on:click=on_click  >
//                                <h2>"Intermediate"</h2>
//                                <p>"I've got a wallet, I want to go deeper"</p>
//                            </button>
//                    }
//            };
//        result
//}
/// Renders the home page of your application.
#[component]
pub fn GuideSelector(cx: Scope) -> impl IntoView {
    let (beginner_clicked, set_beginner_clicked) = create_signal(cx, false);
    let (intermediate_clicked, set_intermediate_clicked) = create_signal(cx, false);
    let (advanced_clicked, set_advanced_clicked) = create_signal(cx, false);

    view! { cx,
      <div class="container mx-auto max-w-5xl flex flex-col md:flex-row justify-evenly
                 items-center p-20 md:mt-10 text-white opacity-0 animate-fadein font-sans gap-8">
        <div class="basis-1/4">
            <img src="./../../lock.png" alt="Financial privacy lock"/>
        </div>
        <div class="basis-1/2">

          <Show
            when=move || beginner_clicked()
            fallback= move |cx| view! { cx, <BeginnerButton on_click=move |_|  {set_beginner_clicked.update(|value| *value = !*value); set_intermediate_clicked.set(false); set_advanced_clicked.set(false)} /> }
          >
            <div class="container text-center text-black rounded-2xl cursor-pointer border-none">
            <button class="basis-1/2">
                <h2 class="font-semibold text-[#f79231]"><a class="box-border border-4 p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/beginner/android">"Android"</a></h2>
            </button>
            <button class="basis-1/2">
                <h2 class="font-semibold text-[#f79231]"><a class="box-border border-4 p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/beginner/ios">"IOS"</a></h2>
            </button>
            </div>
          </Show>
          <Show
            when=move || intermediate_clicked()
            fallback=move |cx| view! { cx, <IntermediateButton on_click=move |_| {set_intermediate_clicked.update(|value| *value = !*value); set_beginner_clicked.set(false); set_advanced_clicked.set(false)} /> }
          >
            <div class="flex flex-row justify-evenly items-center text-center text-black rounded-2xl w-full cursor-pointer border-none">
                <h2 class="font-semibold"><a class="box-border h-32 w-16 p-4 border-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/intermediate/android">"Android"</a></h2>
                <h2 class="font-semibold"><a class="box-border h-32 w-16 p-4 border-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/intermediate/ios">"IOS"</a></h2>
                <h2 class="font-semibold"><a class="box-border h-32 w-16 p-4 border-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/intermediate/desktop">"Desktop"</a></h2>
            </div>
          </Show>
          <Show
            when=move || advanced_clicked()
            fallback=move |cx| view! { cx, <AdvancedButton on_click=move |_| {set_advanced_clicked.update(|value| *value = !*value); set_intermediate_clicked.set(false); set_beginner_clicked.set(false)}/> }
          >
            <div class="flex flex-row justify-evenly items-center text-center text-black rounded-2xl w-full cursor-pointer border-none">
                <h2 class="font-semibold"><a class="box-border h-32 w-16 p-4 border-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/advanced/android">"Android"</a></h2>
                <h2 class="font-semibold"><a class="box-border h-32 w-16 p-4 border-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/advanced/ios">"IOS"</a></h2>
                <h2 class="font-semibold"><a class="box-border h-32 w-16 p-4 border-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949]" href="/guides/advanced/desktop">"Desktop"</a></h2>
            </div>
          </Show>
      </div>
      </div>
    }
}
