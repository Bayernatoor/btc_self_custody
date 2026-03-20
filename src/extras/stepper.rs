//! Step-by-step guide component.
//!
//! Shows one step at a time with a progress bar and prev/next navigation.
//! Reuses `fetch_faq()` to load markdown content from `src/faqs/<faq_name>/`.

use crate::extras::accordion_menu::{fetch_faq, FAQ};
use crate::extras::spinner::Spinner;
use leptos::html::Div;
use leptos::prelude::*;
use leptos::web_sys;

use crate::helpers::markdown;

fn strip_title(raw: &str) -> String {
    if let Some(start) = raw.find('[') {
        if let Some(end) = raw.find("](") {
            return raw[start + 1..end].to_string();
        }
    }
    raw.trim_start_matches('#').trim().to_string()
}

/// Mobile progress: pill-shaped indicators (like the guide selector step dots).
/// Desktop: clickable numbered steps with connecting lines.
#[component]
fn StepperProgress(
    current: ReadSignal<usize>,
    steps: Vec<FAQ>,
    set_current: WriteSignal<usize>,
) -> impl IntoView {
    let total = steps.len();
    let titles: Vec<String> =
        steps.iter().map(|f| strip_title(&f.title)).collect();
    let titles_clone = titles.clone();

    view! {
        // Mobile: pill indicators + step label
        <div class="flex flex-col items-center gap-2 lg:hidden">
            <div class="flex items-center gap-1.5">
                {(0..total).map(|i| {
                    view! {
                        <div class=move || {
                            if i == current.get() {
                                "w-6 h-1.5 rounded-full bg-[#f7931a] transition-all duration-300"
                            } else if i < current.get() {
                                "w-3 h-1.5 rounded-full bg-[#f7931a] opacity-40 transition-all duration-300"
                            } else {
                                "w-3 h-1.5 rounded-full bg-white/15 transition-all duration-300"
                            }
                        }></div>
                    }
                }).collect::<Vec<_>>()}
            </div>
            <span class="text-xs text-white/50">
                {move || {
                    let title = titles_clone.get(current.get()).cloned().unwrap_or_default();
                    format!("Step {} of {} - {}", current.get() + 1, total, title)
                }}
            </span>
        </div>

        // Desktop: numbered circles with connecting lines (no titles - scales to any step count)
        <nav aria-label="Guide progress" class="hidden lg:block">
            <ol class="flex items-center justify-center w-full">
                {(0..total).map(|i| {
                    view! {
                        <li
                            class="flex items-center cursor-pointer"
                            class=("flex-1", i < total - 1)
                            on:click=move |_| set_current.set(i)
                        >
                            <div class=move || {
                                let base = "w-8 h-8 rounded-full flex items-center justify-center text-sm font-semibold shrink-0 transition-all duration-300 hover:ring-2 hover:ring-white/20";
                                if i == current.get() { format!("{base} bg-[#f7931a] text-white ring-2 ring-[#f7931a]/30") }
                                else if i < current.get() { format!("{base} bg-[#f7931a]/80 text-white") }
                                else { format!("{base} bg-white/10 text-white/30") }
                            }>
                                {i + 1}
                            </div>
                            {(i < total - 1).then(|| view! {
                                <div class=move || {
                                    let base = "h-0.5 flex-1 mx-1 transition-colors duration-300";
                                    if i < current.get() { format!("{base} bg-[#f7931a]") }
                                    else { format!("{base} bg-white/10") }
                                }></div>
                            })}
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ol>
        </nav>
    }
}

#[component]
fn StepperContent(
    current: ReadSignal<usize>,
    steps: Vec<FAQ>,
) -> impl IntoView {
    view! {
        <article>
            {move || {
                steps
                    .get(current.get())
                    .map(|faq| {
                        let content_html = markdown::to_html(&faq.content);
                        view! {
                            <div class="bg-white/[0.03] border border-white/[0.07] rounded-2xl p-6 lg:p-8 animate-slidedown">
                                <div
                                    class="step-content text-[0.85rem] lg:text-[0.95rem] text-white/75 leading-relaxed"
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
                    let base = "inline-flex items-center gap-1.5 px-4 py-2 rounded-xl text-sm font-medium transition-all duration-200";
                    if current.get() > 0 {
                        format!("{base} text-white/70 border border-white/10 hover:text-white hover:border-white/25 hover:bg-white/5 cursor-pointer")
                    } else {
                        format!("{base} opacity-20 cursor-not-allowed text-white")
                    }
                }
                disabled=move || current.get() == 0
                on:click=go_prev
            >
                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"/>
                </svg>
                "Previous"
            </button>

            <span class="text-white/30 text-xs">
                {move || format!("{} / {}", current.get() + 1, total)}
            </span>

            <button
                class=move || {
                    let base = "inline-flex items-center gap-1.5 px-4 py-2 rounded-xl text-sm font-medium transition-all duration-200";
                    if current.get() < total - 1 {
                        format!("{base} bg-[#f7931a] text-white hover:bg-[#f4a949] hover:scale-[1.02] active:scale-[0.98] cursor-pointer")
                    } else {
                        format!("{base} opacity-20 cursor-not-allowed text-white")
                    }
                }
                disabled=move || current.get() == total - 1
                on:click=go_next
            >
                "Next"
                <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/>
                </svg>
            </button>
        </nav>
    }
}

#[component]
pub fn Stepper(#[prop(optional)] faq_name: String) -> impl IntoView {
    let (current, set_current) = signal(0usize);
    let faqs = LocalResource::new(move || fetch_faq(faq_name.clone()));
    let stepper_ref = NodeRef::<Div>::new();

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
                                    <section aria-label="Step-by-step guide" class="flex flex-col gap-5">
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
                                }.into_any()
                            }
                            Err(_) => {
                                view! {
                                    <div class="flex flex-col items-center gap-4 py-12">
                                        <div class="w-12 h-12 rounded-full bg-white/5 flex items-center justify-center">
                                            <svg class="w-6 h-6 text-white/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4.5c-.77-.833-2.694-.833-3.464 0L3.34 16.5c-.77.833.192 2.5 1.732 2.5z"/>
                                            </svg>
                                        </div>
                                        <p class="text-sm text-white/50 text-center">"Unable to load guide content. Please refresh the page."</p>
                                        <button
                                            class="text-xs text-white/40 border border-white/10 rounded-lg px-4 py-2 hover:text-white/70 hover:border-white/20 transition-all cursor-pointer"
                                            on:click=move |_| {
                                                if let Some(w) = web_sys::window() {
                                                    let _ = w.location().reload();
                                                }
                                            }
                                        >
                                            "Refresh"
                                        </button>
                                    </div>
                                }.into_any()
                            }
                        }
                    })
                }}
            </Suspense>
        </div>
    }
}
