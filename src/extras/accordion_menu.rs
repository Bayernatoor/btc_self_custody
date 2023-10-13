use leptos::*;

#[derive(Debug, Clone, Default)]
pub struct FAQS {
    pub titles: Vec<String>,
    pub contents: Vec<String>,
}

// FAQ accordion menu button 
#[component]
fn Menu(
    cx: Scope,
    faq_title: String,
    faq_content: String,
    #[prop(optional)] markdown: String,
) -> impl IntoView
{
    let (menu_clicked, set_menu_clicked) = create_signal(cx, false);
    
    // takes faq_content and faq_title to make a button and a accordion style container
    view! {cx,
        <h2 id="accordion-collapse-heading-1">
            <button type="button" class="flex items-center justify-between w-full p-5 font-medium
            text-left text-gray-900 border border-b-0 border-gray-700 rounded-t-xl focus:ring-2
            focus:ring-gray-200 hover:bg-[#3c6594]"
            data-accordion-target="#accordion-collapse-body-1" aria-expanded="true"
            aria-controls="accordion-collapse-body-1" on:click=move |_| { set_menu_clicked.update(|menu| *menu = !*menu)} >
                <span inner_html=faq_title/>
                <svg data-accordion-icon class="w-3 h-3 rotate-180 shrink-0" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6">
                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                </svg>
            </button>
        </h2>
        <div id="accordion-collapse-body-1"  aria-labelledby="accordion-collapse-heading-1" class:hidden=move || menu_clicked() == false  >
            <div class="p-5 border border-b-0 border-gray-700">
                <div class="bg-[#3c6594] rounded-md p-2.5" inner_html=faq_content/>
            </div>
        </div>
    }
}

// Accordion menu for faqs, creates necessary number of Menu comps based on props passed.  
#[component]
pub fn AccordionMenu(cx: Scope, #[prop(optional)] faqs: FAQS) -> impl IntoView {
    
    view! {cx,
        <div id="accordion-collapse" data-accordion="collapse">
            // turn items of faqs struct into into iters, zip em together and then map to Menu comp. 
            // title and content come in as markdown, convert to html
           {faqs.titles.iter().zip(faqs.contents.iter())
               .map(|(title, content)| view! {cx, <Menu faq_title=markdown::to_html(title) faq_content=markdown::to_html(content) />})
               .collect::<Vec<_>>()}
        </div>
    }
}
