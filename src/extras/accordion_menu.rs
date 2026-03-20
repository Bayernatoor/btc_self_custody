//! Accordion menu component for FAQ sections.
//!
//! Loads markdown FAQ files from `src/faqs/<faq_name>/` via a server function,
//! renders them as collapsible accordion panels.

use crate::extras::spinner::Spinner;
use leptos::html::Button;
use leptos::prelude::*;
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
#[server(prefix = "/api", endpoint = "faq")]
pub async fn fetch_faq(faq_name: String) -> Result<Vec<FAQ>, ServerFnError> {
    use std::{fs, io};
    let path = format!("./src/faqs/{}", faq_name);

    let mut files = fs::read_dir(path)?
        .map(|dir| dir.map(|file| file.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;

    files.sort();

    let mut faqs = Vec::new();
    let mut id = 0;

    for faq in files {
        id += 1;
        let _file_name = faq.file_name().unwrap().to_str().unwrap();
        let content = fs::read_to_string(faq)?;
        let title = &content.split("\n").collect::<Vec<&str>>()[0].to_string();
        let faq_content =
            &content.split("\n").collect::<Vec<&str>>()[1..].join("\n");
        faqs.push(FAQ::new_faq(id, title.to_string(), faq_content.to_string()));
    }

    Ok(faqs)
}

// Parse the markdown and convert it to html
#[allow(non_snake_case)]
fn MarkdownToHtml(markdown: &str) -> String {
    let options = Options::empty();
    let parser = Parser::new_ext(markdown, options);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}

#[component]
#[allow(non_snake_case)]
fn Menu(
    faq_title: String,
    faq_content: String,
    menu_id: u32,
    open_menu: ReadSignal<Option<u32>>,
    set_open_menu: WriteSignal<Option<u32>>,
) -> impl IntoView {
    let title_ref = NodeRef::<Button>::new();

    let is_open = move || open_menu.get() == Some(menu_id);

    let handle_menu_click = move |_| {
        if is_open() {
            set_open_menu.set(None);
        } else {
            set_open_menu.set(Some(menu_id));
            if let Some(element) = title_ref.get() {
                element.scroll_into_view();
            }
        }
    };

    view! {
        <div class="mb-2">
            <h2>
                <button
                    type="button"
                    class="group flex items-center justify-between w-full px-5 py-3.5 text-left bg-white/5 border border-white/10 rounded-xl hover:bg-white/10 hover:border-white/20 transition-all duration-200 cursor-pointer"
                    aria-expanded=is_open
                    on:click=handle_menu_click
                    node_ref=title_ref
                >
                    <span class="text-white text-[0.95rem] font-medium pr-4" inner_html=faq_title></span>
                    <svg
                        class="w-4 h-4 shrink-0 text-white/40 group-hover:text-white/70 transition-transform duration-200"
                        class:rotate-180=move || is_open()
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                    >
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                    </svg>
                </button>
            </h2>
            <div class:hidden=move || !is_open()>
                <div class="mt-1 px-5 py-4 bg-white/5 border border-white/10 rounded-xl animate-slidedown">
                    <div
                        class="step-content text-[0.9rem] leading-relaxed text-white/80"
                        inner_html=faq_content
                    ></div>
                </div>
            </div>
        </div>
    }
}

#[component]
#[allow(non_snake_case)]
pub fn AccordionMenu(#[prop(optional)] faq_name: String) -> impl IntoView {
    let (open_menu, set_open_menu) = signal(None::<u32>);

    let faqs = LocalResource::new(move || fetch_faq(faq_name.clone()));

    // Check URL on component mount and open the corresponding FAQ
    Effect::new(move |_| {
        if let Some(data) = faqs.get() {
            if let Ok(faq_vec) = data.as_ref() {
                if let Ok(hash) = window().location().hash() {
                    for faq in faq_vec {
                        if let Some(anchor) = faq
                            .title
                            .split('(')
                            .nth(1)
                            .and_then(|s| s.split(')').next())
                        {
                            if anchor == hash {
                                set_open_menu.set(Some(faq.id));
                                break;
                            }
                        }
                    }
                }
            }
        }
    });

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
                    faqs.get().map(|result| {
                        match result {
                            Ok(faqs_vec) => {
                                view! {
                                    <div class="flex flex-col">
                                        <For
                                            each=move || faqs_vec.clone()
                                            key=|faqs| faqs.id
                                            children=move |faqs| {
                                                view! {
                                                    <Menu
                                                        faq_title=MarkdownToHtml(&faqs.title)
                                                        faq_content=MarkdownToHtml(&faqs.content)
                                                        menu_id=faqs.id
                                                        open_menu=open_menu
                                                        set_open_menu=set_open_menu
                                                    />
                                                }
                                            }
                                        />
                                    </div>
                                }.into_any()
                            }
                            Err(error) => {
                                let msg = format!("Error rendering faqs: {}", error);
                                view! { <div>{msg}</div> }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
