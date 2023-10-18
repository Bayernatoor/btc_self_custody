use leptos::*;
use leptos::logging::log;
use crate::server::api::fetch_faqs::*;


//fn get_faqs(cx: Scope) -> String {
//
//        let file_path = "../faqs/samourai/samourai_faq1.txt";
//
//        let mut file = File::open(file_path).unwrap();
//
//        let mut contents = String::new();
//
//        file.read_to_string(&mut contents).unwrap();
//
//        return contents;
//}

// FAQ accordion menu button 
#[component]
#[allow(non_snake_case)]
fn Menu(
    faq_title: String,
    faq_content: String,
) -> impl IntoView
{
    let (menu_clicked, set_menu_clicked) = create_signal(false);
    
    // takes faq_content and faq_title to make a button and a accordion style container
    view! {
        <h2 id="accordion-collapse-heading">
            <button type="button" class="flex items-center justify-between w-full p-5 font-medium
            text-left text-gray-900 border border-b-0 border-gray-700 rounded-t-xl focus:ring-2
            focus:ring-gray-200 hover:bg-[#3c6594]" aria-expanded="true" aria-controls="accordion-collapse-body" 
            on:click=move |_| { set_menu_clicked.update(|menu| *menu = !*menu)} >
                <span inner_html=faq_title/>
                <svg data-accordion-icon class="w-3 h-3 rotate-180 shrink-0" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6">
                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                </svg>
            </button>
        </h2>
        <div aria-labelledby="accordion-collapse-heading" class:hidden=move || menu_clicked() == false  >
            <div class="p-5 border border-b-0 border-gray-700">
                <div class="bg-[#3c6594] rounded-md p-2.5" inner_html=faq_content/>
            </div>
        </div>
    }
}

// Accordion menu component for faqs, creates necessary number of Menu comps based on props passed.  
#[component]
#[allow(non_snake_case)]
pub fn AccordionMenu(#[prop(optional)] faqs: String) -> impl IntoView {
    
      
      let new_faqs = create_resource( 
            move || (),
            move |_| fetch_faq(faqs.clone())
            );

      view! {
        <div id="accordion-collapse" data-accordion="collapse">
            <Suspense
                fallback=move || view! { <p>"Loading...."</p> }
            >
                {move || {
                    match new_faqs.get() {
                        None => {
                            view! { <p>"No Data Available"</p> }
                        }
                        Some(Ok(faq_vec)) => {
                            view! { 
                                <p>
                                    <For 
                                        each=move || faq_vec.clone()
                                        key= |faqs| faqs.id
                                        children=move |faqs| {
                                            view! {<Menu faq_title=markdown::to_html(&faqs.title) faq_content=markdown::to_html(&faqs.content) />}
                                        }
                                    />
                                </p>
                            }
                        }
                        Some(Err(error)) => {
                            log!("Error rendering faqs: {}", error);
                            view! { <p>
                                    "Oops we ran into an error"
                                    </p> }
                        }
                    }
                }}
            </Suspense>
        </div>
        }
        
}
       

