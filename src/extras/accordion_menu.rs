use crate::server::api::fetch_faqs::*;
use leptos::logging::log;
use leptos::*;
use pulldown_cmark::{html, Options, Parser};

#[allow(non_snake_case)]
fn MarkdownToHtml(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(markdown, options);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}

// FAQ accordion menu button
#[component]
#[allow(non_snake_case)]
fn Menu(faq_title: String, faq_content: String) -> impl IntoView {
    let (menu_clicked, set_menu_clicked) = create_signal(false);

    // takes faq_content and faq_title to make a button and a accordion style container
    view! {
        <h2 id="accordion-collapse-heading">
            <button type="button" class="flex items-center justify-between w-72 p-4
            text-left text-gray-900 border border-b-0 border-gray-500 rounded-xl 
            hover:bg-[#3c6594]" aria-expanded="true" aria-controls="accordion-collapse-body" 
            on:click=move |_| { set_menu_clicked.update(|menu| *menu = !*menu)} >
                <span class="text-white text-sm" inner_html=faq_title/>
                <svg data-accordion-icon class="w-3 h-3 rotate-180 shrink-0" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6">
                    <path stroke="white" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                </svg>
            </button>
        </h2>
        <div aria-labelledby="accordion-collapse-heading" class:hidden=move || menu_clicked() == false  >
            <div class="p-5 border border-b-0 border-gray-500 rounded-xl text-sm animate-fadeinone">
                <div class="bg-[#3c6594] rounded-md p-4 leading-relaxed text-white" inner_html=faq_content/>
            </div>
        </div>
    }
}

// Accordion menu component for faqs, creates necessary number of Menu comps based on props passed.
#[component]
#[allow(non_snake_case)]
pub fn AccordionMenu(#[prop(optional)] faqs: String) -> impl IntoView {
    let markdown = MarkdownToHtml("![note] Hello, *there*");

    let new_faqs = create_resource(move || (), move |_| fetch_faq(faqs.clone()));

    // let faqs_test = match &new_faqs.get() {
    //     Some(Ok(faq_vecs)) =>  log!("markdown: {}", MarkdownToHtml(&faq_vecs[0].content)),
    //     None => log!("Nothing to see here"),
    //     Some(Err(error)) => log!("Error rendering faqs: {}", error)
    //
    // };

    //log!("markdown {:#?}", markdown);

    view! {
    <div id="accordion-collapse" data-accordion="collapse">
        <Suspense
            fallback=move || view! { <div>"Loading...."</div> }
        >
            {move || {
                match new_faqs.get() {
                    None => {
                        view! { <div>"No Data Available"</div> }
                    }
                    Some(Ok(faq_vec)) => {
                        view! {
                            <div class="flex flex-col items-center">
                                <For
                                    each=move || faq_vec.clone()
                                    key= |faqs| faqs.id
                                    children=move |faqs| {
                                        view! {<Menu faq_title=MarkdownToHtml(&faqs.title) faq_content=MarkdownToHtml(&faqs.content)/>}
                                    }
                                />
                            </div>
                        }
                    }
                    Some(Err(error)) => {
                        log!("Error rendering faqs: {}", error);
                        view! { <div>
                                "Oops we ran into an error"
                                </div> }
                    }
                }
            }}
        </Suspense>
    </div>
    }
}
