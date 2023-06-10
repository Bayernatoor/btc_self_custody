use leptos::*;
use leptos::{ev::MouseEvent};

#[component]
fn BeginnerButton<F>(cx: Scope, on_click: F) -> impl IntoView 
    where
        F: Fn(MouseEvent) + 'static,
    {
    view! { cx, 
            <button class="button-guides" on:click=on_click  >
                <h2>"Beginner"</h2>
                <p>"What the hell is Bitcoin?"</p>
            </button>
    }

}

#[component]
fn IntermediateButton<F>(cx: Scope, on_click: F) -> impl IntoView 
    where
        F: Fn(MouseEvent) + 'static,
    {
    view! { cx, 
            <button class="button-guides" on:click=on_click  >
                <h2>"Intermediate"</h2>
                <p>"I've got a wallet, I want to go deeper"</p>
            </button>
    }

}

#[component]
fn AdvancedButton<F>(cx: Scope, on_click: F) -> impl IntoView 
    where
        F: Fn(MouseEvent) + 'static,
    {

    view! { cx,
            <button class="button-guides" on:click=on_click  >
                <h2>"Advanced"</h2>
                <p>"It's time to jump down the rabbit hole"</p>
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
    let (guide_clicked, set_guide_clicked) = create_signal(cx, false);

    view! { cx,
      <div class="main-container">
          <div class="left-container">
          <img src="./../../lock.png" alt="Financial privacy lock"/>
      </div>
      <div class="right-container">

          <Show
            when=move || beginner_clicked() 
            fallback= move |cx| view! { cx, <BeginnerButton on_click=move |_|  {set_beginner_clicked.update(|value| *value = !*value); set_intermediate_clicked.set(false); set_advanced_clicked.set(false)} /> }
          >
            <div class="button-pressed">
                <h2><a href="/guides/beginner/android">"Android"</a></h2>
                <h2><a href="/guides/beginner/ios">"IOS"</a></h2>
            </div> 
          </Show>
          <Show
            when=move || intermediate_clicked()
            fallback=move |cx| view! { cx, <IntermediateButton on_click=move |_| {set_intermediate_clicked.update(|value| *value = !*value); set_beginner_clicked.set(false); set_advanced_clicked.set(false)} /> }
          >
            <div class="button-pressed">
                <h2><a href="/guides/intermediate/android">"Android"</a></h2>
                <h2><a href="/guides/intermediate/ios">"IOS"</a></h2>
                <h2><a href="/guides/intermediate/desktop">"Desktop"</a></h2>
            </div>
          </Show>
          <Show
            when=move || advanced_clicked() 
            fallback=move |cx| view! { cx, <AdvancedButton on_click=move |_| {set_advanced_clicked.update(|value| *value = !*value); set_intermediate_clicked.set(false); set_beginner_clicked.set(false)}/> }
          >
            <div class="button-pressed">
                <h2><a href="/guides/advanced/android">"Android"</a></h2>
                <h2><a href="/guides/advanced/ios">"IOS"</a></h2>
                <h2><a href="/guides/advanced/desktop">"Desktop"</a></h2>
            </div>
          </Show>
      </div>
      </div>
    }
}


