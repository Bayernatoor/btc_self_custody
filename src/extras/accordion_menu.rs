use crate::extras::spinner::Spinner;
use leptos::logging::log;
use leptos::{server, ServerFnError, *};
use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};

// An FAQ - values come from an md file
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FAQ {
    pub id: u32,
    pub title: String,
    pub content: String,
}

impl FAQ {
    pub fn new_faq(id: u32, title: String, content: String) -> Self {
        Self { id, title, content }
    }
}

// Server function to fetch FAQ md files 
#[server(FetchFaq, "/api", "Url", "faq")]
pub async fn fetch_faq(faq_name: String) -> Result<Vec<FAQ>, ServerFnError> {
    use std::{fs, io};
    let path = format!("./src/faqs/{}", faq_name);

    // create a ReadDir, retreive each file and extract the path.
    // add individual paths to a vec.
    let mut files = fs::read_dir(path)?
        .map(|dir| dir.map(|file| file.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    // sort files in order they appear in dir - ReadDir retrieves files unordered
    files.sort();

    let mut faqs = Vec::new();
    let mut id = 0;

    // Iterate over all files and create new a `FAQ` struct for each one.
    // Add them to faqs vec
    for faq in files {
        // increment id for each new file
        id += 1;
        // get name of file
        let _file_name = faq.file_name().unwrap().to_str().unwrap();

        // read contents of file
        let content = fs::read_to_string(faq)?;

        // get the faq title
        let title = &content.split("\n").collect::<Vec<&str>>()[0].to_string();

        // get faq content
        let faq_content =
            &content.split("\n").collect::<Vec<&str>>()[1..].join("\n");

        // add created faq to vec
        faqs.push(FAQ::new_faq(id, title.to_string(), faq_content.to_string()));
    }

    Ok(faqs)
}

// Pasre the markdown and convert it to html
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

// Generate FAQs menu
#[component]
#[allow(non_snake_case)]
fn Menu(faq_title: String, faq_content: String) -> impl IntoView {
    let (menu_clicked, set_menu_clicked) = create_signal(false);

    // takes faq_content and faq_title to make a button and a accordion style container
    view! {
        <h2 id="accordion-collapse-heading">
            <button
                type="button"
                class=format!(
                    "flex justify-between w-full p-4
            text-left text-gray-900 border border-gray-500 rounded-xl 
            hover:bg-[#3c6594]",
                )
                aria-expanded="true"
                aria-controls="accordion-collapse-body"
                on:click=move |_| { set_menu_clicked.update(|menu| *menu = !*menu) }
            >
                <span class="text-white text-xl" inner_html=faq_title></span>
                <svg
                    data-accordion-icon
                    class="w-3 h-3 rotate-180 shrink-0"
                    aria-hidden="true"
                    xmlns="http://www.w3.org/2000/svg"
                    fill="none"
                    viewBox="0 0 10 6"
                >
                    <path
                        stroke="white"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        stroke-width="2"
                        d="M9 5 5 1 1 5"
                    ></path>
                </svg>
            </button>
        </h2>
        <div aria-labelledby="accordion-collapse-heading" class:hidden=move || !menu_clicked()>
            <div class="p-5 border border-gray-500 rounded-xl text-sm animate-fadeinone">
                <div
                    class="bg-[#3c6594] rounded-md p-4 leading-relaxed text-white text-lg"
                    inner_html=faq_content
                ></div>
            </div>
        </div>
    }
}

// Accordion menu component for faqs, creates necessary number of Menu comps based on props passed.
#[component]
#[allow(non_snake_case)]
pub fn AccordionMenu(#[prop(optional)] faq_name: String) -> impl IntoView {
    // returns a Vec containing a FAQS struct
    let faqs =
        create_resource(move || (), move |_| fetch_faq(faq_name.clone()));

    view! {
        <div>
            <Suspense fallback=move || {
                view! {
                    <div class="flex justify-center pt-4">
                        <Spinner/>
                    </div>
                }
            }>
                {move || {
                    match faqs.get() {
                        Some(Ok(faqs_vec)) => {
                            view! {
                                <div class="px-2 flex flex-col lg:px-0">
                                    <For
                                        each=move || faqs_vec.clone()
                                        key=|faqs| faqs.id
                                        children=move |faqs| {
                                            view! {
                                                <Menu
                                                    faq_title=MarkdownToHtml(&faqs.title)
                                                    faq_content=MarkdownToHtml(&faqs.content)
                                                />
                                            }
                                        }
                                    />

                                </div>
                            }
                                .into_view()
                        }
                        Some(Err(error)) => {
                            log!("Error rendering faqs: {}", error);
                            view! { <div>"Oops we ran into an error"</div> }.into_view()
                        }
                        None => view! { <div>"No Data Available"</div> }.into_view(),
                    }
                }}

            </Suspense>
        </div>
    }
}
