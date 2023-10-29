use leptos::ev::MouseEvent;
use leptos::*;

#[component]
fn BeginnerButton<F>(on_click: F) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    view! {
            <button class="flex flex-col p-2 max-w-sm mx-auto w-64 bg-slate-100 rounded-xl flex items-center" on:click=on_click>
                <div class="text-2xl font-semibold text-[#f79231]">"Beginner"</div>
                <p class="text-sm text-[#1a578f] mt-2">"What the hell is Bitcoin?"</p>
            </button>
    }
}

#[component]
fn IntermediateButton<F>(on_click: F) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    view! {
            <button class="flex flex-col p-2 max-w-sm mx-auto w-64 bg-slate-100 rounded-xl flex items-center my-5" on:click=on_click>
                <div class="text-2xl font-semibold text-[#f79231]">"Intermediate"</div>
                <p class="text-sm text-[#1a578f] mt-2">"I've got a wallet, I want to go deeper"</p>
            </button>
    }
}

#[component]
fn AdvancedButton<F>(on_click: F) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static,
{
    view! {
            <button class="flex flex-col p-2 max-w-sm mx-auto w-64 bg-slate-100 rounded-xl flex items-center mt-2" on:click=on_click>
                <div class="text-2xl font-semibold text-[#f79231]">"Advanced"</div>
                <p class="text-sm text-[#1a578f] mt-2">"It's time to jump down the rabbit hole"</p>
            </button>
    }
}

/// Renders the home page of your application.
#[allow(clippy::redundant_closure)]
#[allow(non_camel_case_types)]
#[component]
pub fn GuideSelector() -> impl IntoView {
    let (beginner_clicked, set_beginner_clicked) = create_signal(false);
    let (intermediate_clicked, set_intermediate_clicked) = create_signal(false);
    let (advanced_clicked, set_advanced_clicked) = create_signal(false);

    view! {
      <div id="test" class="container mx-auto max-w-5xl flex flex-col md:flex-row justify-center
                 items-center p-20 md:mt-10 text-white opacity-0 animate-fadeinone font-sans gap-8">
        <div class="basis-1/4">
            <img src="./../../../lock.png" alt="Financial privacy lock"/>
        </div>
        <div class="basis-1/2">

        <Show
            when=move || beginner_clicked()
            fallback= move || view! {<BeginnerButton on_click=move |_|  {set_beginner_clicked.update(|value| *value = !*value); set_intermediate_clicked.set(false); set_advanced_clicked.set(false)} /> }
            >
            <div class="flex justify-center items-center gap-2 animate-fadeinone">
                <button class="h-12 flex-grow-0">
                  <h2 class="w-32 flex items-center justify-center font-semibold text-[#f79231]">
                    <a class="box-border p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949] w-full" href="/guides/beginner/android">"Android"</a>
                  </h2>
                </button>
                <button class="h-12 flex-grow-0">
                  <h2 class="w-32 flex items-center justify-center font-semibold text-[#f79231]">
                    <a class="box-border p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949] w-full" href="/guides/beginner/ios">"iOS"</a>
                  </h2>
                </button>
            </div>
        </Show>

          <Show
            when=move || intermediate_clicked()
            fallback=move || view! {<IntermediateButton on_click=move |_| {set_intermediate_clicked.update(|value| *value = !*value); set_beginner_clicked.set(false); set_advanced_clicked.set(false)} /> }
          >
            <div class="flex justify-center items-center py-4 gap-2 animate-fadeinone">
                <button class="h-12 flex-grow-0">
                  <h2 class="w-32 flex items-center justify-center font-semibold text-[#f79231]">
                    <a class="box-border p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949] w-full" href="/guides/beginner/android">"Android"</a>
                  </h2>
                </button>
                <button class="h-12 flex-grow-0">
                  <h2 class="w-32 flex items-center justify-center font-semibold text-[#f79231]">
                    <a class="box-border p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949] w-full" href="/guides/beginner/ios">"iOS"</a>
                  </h2>
                </button>
                <button class="h-12 flex-grow-0">
                  <h2 class="w-32 flex items-center justify-center font-semibold text-[#f79231]">
                    <a class="box-border p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949] w-full" href="/guides/beginner/ios">"Desktop"</a>
                  </h2>
                </button>
            </div>
          </Show>
          <Show
            when=move || advanced_clicked()
            fallback=move || view! {<AdvancedButton on_click=move |_| {set_advanced_clicked.update(|value| *value = !*value); set_intermediate_clicked.set(false); set_beginner_clicked.set(false)}/> }
          >
            <div class="flex justify-center items-center gap-2 animate-fadeinone">
                <button class="h-12 flex-grow-0">
                  <h2 class="w-32 flex items-center justify-center font-semibold text-[#f79231]">
                    <a class="box-border p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949] w-full" href="/guides/beginner/android">"Android"</a>
                  </h2>
                </button>
                <button class="h-12 flex-grow-0">
                  <h2 class="w-32 flex items-center justify-center font-semibold text-[#f79231]">
                    <a class="box-border p-4 bg-slate-100 rounded-2xl no-underline text-[#f79231] hover:bg-[#f4a949] w-full" href="/guides/beginner/ios">"iOS"</a>
                  </h2>
                </button>
            </div>

          </Show>
        </div>
      </div>
    }
}
