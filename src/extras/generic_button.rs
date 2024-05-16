use leptos::*;

#[component]
#[allow(non_snake_case)]
pub fn GenericButton(
    path: String,
    wallet_title: String,
    #[prop(default = "xl".to_string())] text_size: String,
    #[prop(optional)] short_desc: String,
    #[prop(optional)] img_url: String,
    #[prop(optional)] img_alt: String,
    #[prop(optional)] text_color: String,
    #[prop(optional)] new_height: String,
    #[prop(optional)] new_width: String,
) -> impl IntoView {
    let (width, set_width) = create_signal("12".to_string());
    let (height, set_heigth) = create_signal("12".to_string());

    if !new_height.is_empty() {
        set_heigth(new_height)
    }

    if !new_width.is_empty() {
        set_width(new_width)
    }

    view! {
        <a href=path>
            <button class="flex justify-center shrink-0 h-18 w-72 p-2 mx-auto bg-white rounded-xl items-center">
              <div class="flex justify-center basis-1/3 shrink-0">
                <img class=format!("h-{} w-{}", height.get(), width.get()) src=img_url alt=img_alt/>
              </div>
              <div class="basis-2/3">
                <h3 class=format!("text-{text_size} font-medium text-[{text_color}]")>{wallet_title}</h3>
                <p class="text-slate-500">{short_desc}</p>
              </div>
            </button>
        </a>
    }
}

#[component]
#[allow(non_snake_case)]
pub fn GenericExternalButton(
    path: String,
    wallet_title: String,
    #[prop(optional)] short_desc: String,
    #[prop(optional)] img_url: String,
    #[prop(optional)] img_alt: String,
    #[prop(optional)] text_color: String,
    #[prop(optional)] new_height: String,
    #[prop(optional)] new_width: String,
) -> impl IntoView {
    let (width, set_width) = create_signal("12".to_string());
    let (height, set_heigth) = create_signal("12".to_string());

    if !new_height.is_empty() {
        set_heigth(new_height)
    }

    if !new_width.is_empty() {
        set_width(new_width)
    }

    view! {
        <a href=path rel="noreferrer" target="_blank" rel="noreferrer" class="flex h-18 w-72">
            <button class="flex justify-center shrink-0 h-18 w-72 p-2 mx-auto bg-white rounded-xl items-center space-x-4 shadow-inner">
              <div class="flex justify-center basis-1/3">
                <img class=format!("h-{} w-{}", height.get(), width.get()) src=img_url alt=img_alt/>
              </div>
              <div class="basis-2/3">
                <h3 class=format!("text-md font-medium text-[{text_color}]")>{wallet_title}</h3>
                <p class="text-slate-500">{short_desc}</p>
              </div>
            </button>
        </a>
    }
}

#[component]
#[allow(non_snake_case)]
pub fn GenericImageSubTextButton(
    path: String,
    #[prop(optional)] title: String,
    #[prop(optional)] short_desc: String,
    #[prop(optional)] _img_url: String,
    #[prop(optional)] _img_alt: String,
    #[prop(optional)] text_color: String,
    #[prop(optional)] _new_height: String,
    #[prop(optional)] _new_width: String,
) -> impl IntoView {
    view! {
        <a href=path rel="noreferrer" target="_blank">
            <button class="flex justify-center shrink-0 h-18 w-72 p-2 mx-auto bg-white rounded-xl items-center space-x-4 shadow-inner">
              <div>
                //<img class=format!("h-{} w-{} inline-flex", height.get(), width.get()) src=img_url alt=img_alt/>
                <h3 class=format!("text-lg font-medium text-[{text_color}]")>{title}</h3>
                <p class="text-slate-600">{short_desc}</p>
              </div>
            </button>
        </a>
    }
}
