use crate::extras::back::BackButton;
use leptos::ev::MouseEvent;
use leptos::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct GuideDetails {
    id: u32,
    level_name: String,
    device: String,
    path: String,
}

impl GuideDetails {
    fn new_guide_details(
        id: u32,
        level_name: String,
        device: String,
        path: String,
    ) -> Self {
        Self {
            id,
            level_name,
            device,
            path,
        }
    }
}
#[allow(clippy::redundant_closure)]
#[component]
fn LevelButton<F>(
    on_click: F,
    setter: ReadSignal<bool>,
    hidden: ReadSignal<bool>,
    name: String,
    subtitle: String,
    #[prop(optional)] devices: Vec<String>,
) -> impl IntoView
where
    F: Fn(MouseEvent) + 'static + Clone,
{
    let (guide, set_guide) = create_signal::<Vec<GuideDetails>>(vec![]);
    let mut guides = Vec::new();
    let mut id = 0;

    for device in devices {
        id += 1;
        let lower_name = name.to_lowercase();
        let device_name = device.to_lowercase();
        let path = format!("/guides/{lower_name}/{device_name}");
        guides.push(GuideDetails::new_guide_details(
            id,
            name.clone(),
            device,
            path,
        ))
    }

    set_guide(guides);

    view! {
        <Show
            when=move || setter()
            fallback=move || view! {
                <button class="flex flex-col p-4 max-w-md mx-auto w-72 bg-white rounded-xl items-center mt-6 shadow-md hover:bg-[#f2f2f2] transition ease-in-out duration-300" class:hidden=move || hidden() on:click=on_click.clone()>
                    <div class="text-3xl font-bold text-[#f79231]">{name.clone()}</div>
                    <p class="text-lg text-[#123c64] mt-3">{subtitle.clone()}</p>
                </button>}
        >
            <div class="flex flex-col items-center py-5 gap-5 animate-fadeinone">
                <For
                    each=move || guide()
                    key= |guide| guide.id
                    children=move |guide| {
                        view! {
                            <a href=guide.path>
                                <button class="flex-grow-0 max-w-md-">
                                    <h2 class="w-48 flex items-center justify-center font-semibold text-[#f79231]">
                                        <div class="box-border p-5 bg-white text-2xl rounded-2xl no-underline text-[#f79231] hover:bg-[#f2f2f2] w-full transition ease-in-out duration-300">
                                            <span>{guide.device}</span>
                                        </div>
                                    </h2>
                                </button>
                            </a>
                        }.into_view()
                    }
                />
            </div>
            <div class="mt-5 flex flex-col md:flex-row items-center justify-center">
                <BackButton button_image="./../../../arrow-111-512.png".to_string() reload=true />
            </div>
        </Show>
    }.into_view()
}

#[allow(clippy::redundant_closure)]
#[allow(non_camel_case_types)]
#[component]
pub fn GuideSelector() -> impl IntoView {
    // set on_click
    let (basic_clicked, set_basic_clicked) = create_signal(false);
    let (intermediate_clicked, set_intermediate_clicked) = create_signal(false);
    let (advanced_clicked, set_advanced_clicked) = create_signal(false);

    // used to hidden other buttons on click
    let (basic_hidden, set_basic_hidden) = create_signal(false);
    let (intermediate_hidden, set_intermediate_hidden) = create_signal(false);
    let (advanced_hidden, set_advanced_hidden) = create_signal(false);

    // devices to be included in guide level
    let basic_devices: Vec<String> = vec![
        "Android".to_string(),
        "iOS".to_string(),
        "Desktop".to_string(),
    ];
    let intermediate_devices: Vec<String> = vec!["Desktop".to_string()];
    let advanced_devices: Vec<String> = vec!["Desktop".to_string()];

    view! {
        <div class="grid gap-4 mx-auto justify-items-center max-w-5xl my-20 opacity-0 animate-fadeinone lg:grid-cols-2 lg:mt-0 lg:my-64">
            <div class="flex justify-center items-center">
                <img class="w-64 h-auto lg:w-80" src="./../../../lock_new_blue.png" alt="Financial privacy lock"/>
            </div>
            <div class="">
                <LevelButton on_click=move |_| { set_basic_clicked.update(|value| *value = !*value); set_intermediate_hidden.set(true); set_advanced_hidden.set(true) }
                             name="Basic".to_string()
                             subtitle="I have a teeny weeny stack".to_string()
                             hidden=basic_hidden
                             setter=basic_clicked
                             devices=basic_devices/>

                <LevelButton on_click=move |_| { set_intermediate_clicked.update(|value| *value = !*value); set_basic_hidden.set(true); set_advanced_hidden.set(true) }
                             name="Intermediate".to_string()
                             subtitle="I have an average stack".to_string()
                             hidden=intermediate_hidden
                             setter=intermediate_clicked
                             devices=intermediate_devices/>

                <LevelButton on_click=move |_| { set_advanced_clicked.update(|value| *value = !*value); set_basic_hidden.set(true); set_intermediate_hidden.set(true) }
                             name="Advanced".to_string()
                             subtitle="I am well equipped".to_string()
                             hidden=advanced_hidden
                             setter=advanced_clicked
                             devices=advanced_devices/>
            </div>
      </div>
    }
}
