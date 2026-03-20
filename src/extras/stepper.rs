//! Step-by-step guide component.
//!
//! Replaces the accordion for sequential guides. Shows one step at a time
//! with a progress bar and prev/next navigation. Reuses `fetch_faq()` to
//! load markdown content from `src/faqs/<faq_name>/`.

use crate::extras::accordion_menu::{fetch_faq, FAQ};
use crate::extras::spinner::Spinner;
use leptos::html::Div;
use leptos::prelude::*;
use pulldown_cmark::{html, Options, Parser};

fn markdown_to_html(markdown: &str) -> String {
    let options = Options::empty();
    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Strip markdown/HTML from a title to get plain text for the progress bar.
fn strip_title(raw: &str) -> String {
    if let Some(start) = raw.find('[') {
        if let Some(end) = raw.find("](") {
            return raw[start + 1..end].to_string();
        }
    }
    raw.trim_start_matches('#').trim().to_string()
}

#[component]
fn StepperProgress(
    current: ReadSignal<usize>,
    steps: Vec<FAQ>,
    set_current: WriteSignal<usize>,
) -> impl IntoView {
    let total = steps.len();
    let titles: Vec<String> = steps.iter().map(|f| strip_title(&f.title)).collect();
    let titles_clone = titles.clone();

    view! {
        // Mobile: compact "Step X of Y"
        <div class="flex items-center justify-center gap-2 text-white text-sm lg:hidden">
            <span class="text-[#f7931a] font-semibold">
                {move || format!("Step {} of {}", current.get() + 1, total)}
            </span>
            <span class="opacity-60">"—"</span>
            <span class="opacity-80 truncate max-w-48">
                {move || titles_clone.get(current.get()).cloned().unwrap_or_default()}
            </span>
        </div>

        // Desktop: full horizontal progress bar
        <nav aria-label="Guide progress" class="hidden lg:block">
            <ol class="flex items-center w-full">
                {titles
                    .into_iter()
                    .enumerate()
                    .map(|(i, title)| {
                        let set_current = set_current.clone();
                        view! {
                            <li
                                class="flex-1 cursor-pointer group"
                                on:click=move |_| set_current.set(i)
                            >
                                <div class="flex flex-col items-center gap-1.5">
                                    <div class="flex items-center w-full">
                                        // Left line
                                        <div class=move || {
                                            let base = "h-0.5 flex-1";
                                            if i == 0 {
                                                format!("{base} bg-transparent")
                                            } else if i <= current.get() {
                                                format!("{base} bg-[#f7931a]")
                                            } else {
                                                format!("{base} bg-white opacity-20")
                                            }
                                        }></div>
                                        // Circle
                                        <div class=move || {
                                            let base = "w-8 h-8 rounded-full flex items-center justify-center text-sm font-semibold shrink-0 transition-all duration-300";
                                            if i == current.get() {
                                                format!("{base} bg-[#f7931a] text-white ring-2 ring-[#f4a949]")
                                            } else if i < current.get() {
                                                format!("{base} bg-[#f7931a] text-white")
                                            } else {
                                                format!("{base} bg-white text-[#123c64] opacity-20")
                                            }
                                        }>
                                            {i + 1}
                                        </div>
                                        // Right line
                                        <div class=move || {
                                            let base = "h-0.5 flex-1";
                                            if i == total - 1 {
                                                format!("{base} bg-transparent")
                                            } else if i < current.get() {
                                                format!("{base} bg-[#f7931a]")
                                            } else {
                                                format!("{base} bg-white opacity-20")
                                            }
                                        }></div>
                                    </div>
                                    // Title
                                    <span class=move || {
                                        let base = "text-xs text-center px-1 leading-tight max-w-24 truncate transition-colors";
                                        if i <= current.get() {
                                            format!("{base} text-white")
                                        } else {
                                            format!("{base} text-white opacity-40")
                                        }
                                    }>
                                        {title}
                                    </span>
                                </div>
                            </li>
                        }
                    })
                    .collect::<Vec<_>>()}
            </ol>
        </nav>
    }
}

#[component]
fn StepperContent(current: ReadSignal<usize>, steps: Vec<FAQ>) -> impl IntoView {
    view! {
        <article class="animate-fadeinone">
            {move || {
                steps
                    .get(current.get())
                    .map(|faq| {
                        let title_html = markdown_to_html(&faq.title);
                        let content_html = markdown_to_html(&faq.content);
                        view! {
                            <div class="bg-[#1a4a72] rounded-xl p-6 lg:p-8">
                                <header class="mb-4">
                                    <div
                                        class="text-xl font-title font-semibold text-[#f7931a]"
                                        inner_html=title_html
                                    ></div>
                                </header>
                                <div
                                    class="text-white text-lg leading-relaxed max-w-none"
                                    inner_html=content_html
                                ></div>
                            </div>
                        }
                    })
            }}
        </article>
    }
}

#[component]
fn StepperNav(
    current: ReadSignal<usize>,
    set_current: WriteSignal<usize>,
    total: usize,
) -> impl IntoView {
    let go_prev = move |_| {
        let cur = current.get();
        if cur > 0 {
            set_current.set(cur - 1);
        }
    };

    let go_next = move |_| {
        let cur = current.get();
        if cur < total - 1 {
            set_current.set(cur + 1);
        }
    };

    view! {
        <nav aria-label="Step navigation" class="flex justify-between items-center pt-4">
            <button
                class=move || {
                    let base = "flex items-center gap-1.5 px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200";
                    if current.get() > 0 {
                        format!("{base} bg-white/95 text-[#123c64] border border-white/20 hover:-translate-y-0.5 hover:shadow-md active:translate-y-0")
                    } else {
                        format!("{base} opacity-20 cursor-not-allowed text-white")
                    }
                }
                disabled=move || current.get() == 0
                on:click=go_prev
            >
                "← Previous"
            </button>

            <span class="text-white opacity-40 text-xs">
                {move || format!("{} / {}", current.get() + 1, total)}
            </span>

            <button
                class=move || {
                    let base = "flex items-center gap-1.5 px-4 py-2 rounded-lg text-sm font-medium transition-all duration-200";
                    if current.get() < total - 1 {
                        format!("{base} bg-[#f79231] text-white hover:-translate-y-0.5 hover:bg-[#f4a949] hover:shadow-md active:translate-y-0")
                    } else {
                        format!("{base} opacity-20 cursor-not-allowed text-white")
                    }
                }
                disabled=move || current.get() == total - 1
                on:click=go_next
            >
                "Next →"
            </button>
        </nav>
    }
}

#[component]
pub fn Stepper(#[prop(optional)] faq_name: String) -> impl IntoView {
    let (current, set_current) = signal(0usize);
    let faqs = LocalResource::new(move || fetch_faq(faq_name.clone()));
    let stepper_ref = NodeRef::<Div>::new();

    // Scroll to stepper progress bar when step changes
    Effect::new(move |_| {
        let _ = current.get();
        if let Some(el) = stepper_ref.get() {
            el.scroll_into_view();
        }
    });

    view! {
        <div node_ref=stepper_ref>
            <Suspense fallback=move || {
                view! {
                    <div class="flex justify-center pt-8">
                        <Spinner/>
                    </div>
                }
            }>
                {move || {
                    faqs.get().map(|result| {
                        match result {
                            Ok(faqs_vec) => {
                                let total = faqs_vec.len();
                                view! {
                                    <section aria-label="Step-by-step guide" class="flex flex-col gap-6">
                                        <StepperProgress
                                            current=current
                                            steps=faqs_vec.clone()
                                            set_current=set_current
                                        />
                                        <StepperContent current=current steps=faqs_vec.clone()/>
                                        <StepperNav
                                            current=current
                                            set_current=set_current
                                            total=total
                                        />
                                    </section>
                                }
                                    .into_any()
                            }
                            Err(error) => {
                                let msg = format!("Error loading guide: {}", error);
                                view! { <div class="text-red-400 text-center p-4">{msg}</div> }
                                    .into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
