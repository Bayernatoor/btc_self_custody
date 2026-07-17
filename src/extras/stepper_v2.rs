//! StepperV2 — the "Refined" guide wizard.
//!
//! Renders a typed `GuideV2` (see src/guides_v2.rs): an intro panel, one panel
//! per step (two-pane: short actions + a framed screenshot with numbered pins),
//! and a completion panel. All panels render in the DOM (SSR/SEO friendly);
//! only the active one is visible.
//!
//! Step state lives in the URL query (`?step=N`) via leptos_router, so steps are
//! deep-linkable, refresh-safe, and browser Back/Forward walks the guide. All
//! panels are server-rendered from static data, so there is no per-step fetch.
//!
//! Inline `**bold**` / `[text](url)` in copy is parsed to real nodes (never
//! inner_html) by `inline()`.

use leptos::portal::Portal;
use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use leptos_router::NavigateOptions;

use crate::guides::DownloadLink;
use crate::guides_v2::{Device, Frame, GuideV2, Step};

const BACKUP_SHEET_HREF: &str = "/downloads/seed-backup-sheet.html";

// ── inline text parser (**bold**, [text](url)) ──────────────────────────────

#[derive(Debug, PartialEq)]
enum Seg {
    Text(String),
    Bold(String),
    Link(String, String),
}

/// Parse a trusted copy string into inline segments. Only `**bold**` and
/// `[text](url)` are special; everything else is literal text. The markers are
/// ASCII, so byte slicing stays on char boundaries.
fn parse_inline(s: &str) -> Vec<Seg> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**") {
                if !buf.is_empty() {
                    out.push(Seg::Text(std::mem::take(&mut buf)));
                }
                out.push(Seg::Bold(after[..end].to_string()));
                i += 2 + end + 2;
                continue;
            }
        }
        if rest.starts_with('[') {
            if let Some(close) = rest.find(']') {
                let tail = &rest[close + 1..];
                if tail.starts_with('(') {
                    if let Some(paren) = tail.find(')') {
                        let text = rest[1..close].to_string();
                        let url = tail[1..paren].to_string();
                        if !buf.is_empty() {
                            out.push(Seg::Text(std::mem::take(&mut buf)));
                        }
                        out.push(Seg::Link(text, url));
                        i += close + 1 + paren + 1;
                        continue;
                    }
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        buf.push(ch);
        i += ch.len_utf8();
    }
    if !buf.is_empty() {
        out.push(Seg::Text(buf));
    }
    out
}

/// Render inline copy to real Leptos nodes (no inner_html). Supports `**bold**`
/// and `[text](url)`. Reused by the wallet-picker trait chips.
pub fn inline(s: &'static str) -> Vec<AnyView> {
    parse_inline(s)
        .into_iter()
        .map(|seg| match seg {
            Seg::Text(t) => view! { {t} }.into_any(),
            Seg::Bold(t) => view! { <strong class="g2-b">{t}</strong> }.into_any(),
            Seg::Link(t, u) => view! {
                <a class="g2-inline-link" href=u target="_blank" rel="noreferrer">{t}</a>
            }
            .into_any(),
        })
        .collect()
}

// ── small pieces ────────────────────────────────────────────────────────────

#[component]
fn BackupSheetCta() -> impl IntoView {
    view! {
        <a class="g2-sheet" href=BACKUP_SHEET_HREF target="_blank" rel="noreferrer">
            <span class="g2-sheet-ic">"🖨"</span>
            <span class="g2-sheet-tx">
                <b>"Print your backup sheet"</b>
                <small>"A We Hodl BTC sheet to write your recovery words on, by hand, the right way."</small>
            </span>
            <span class="g2-sheet-go">"Open →"</span>
        </a>
    }
}

#[component]
fn DeviceFrame(device: &'static Device) -> impl IntoView {
    let shots = device.shots;
    let n = shots.len();
    let frame_class = match device.frame {
        Frame::Phone => "g2-phone",
        Frame::Desktop => "g2-window",
    };
    // Desktop shots carry a caption under the (full-width) window so a long
    // carousel stays scannable. Phone guides stay decluttered (no caption).
    let show_caption = matches!(device.frame, Frame::Desktop);
    // carousel index + click-to-zoom modal state
    let (slide, set_slide) = signal(0usize);
    let (modal, set_modal) = signal(false);
    // current shot as a Copy signal so it can drive several reactive blocks
    let cur = Signal::derive(move || shots[slide.get().min(n.saturating_sub(1))]);

    view! {
        <figure class="g2-devicewrap">
            <div class=frame_class>
                <div class="g2-screen"
                    style=move || { let s = cur.get(); format!("aspect-ratio: {} / {}", s.img_w, s.img_h) }>
                    <button class="g2-shot-btn" on:click=move |_| set_modal.set(true) aria-label="Open screenshot">
                        {move || {
                            let s = cur.get();
                            view! {
                                <img class="g2-shot" src=s.image alt=s.alt loading="lazy"/>
                                {s.pins.iter().map(|p| {
                                    let style = format!("left:{}%;top:{}%", p.x, p.y);
                                    view! { <span class="g2-pin" style=style aria-hidden="true">{p.n}</span> }
                                }).collect::<Vec<_>>()}
                            }
                        }}
                    </button>
                </div>
            </div>

            {show_caption.then(|| view! {
                <figcaption class="g2-caption">{move || cur.get().caption}</figcaption>
            })}

            // carousel controls (only when there is more than one shot)
            {(n > 1).then(|| view! {
                <div class="g2-carousel">
                    <button class="g2-caro-arrow" aria-label="Previous screenshot"
                        on:click=move |_| set_slide.update(|i| *i = (*i + n - 1) % n)>"\u{2039}"</button>
                    <div class="g2-dots">
                        {(0..n).map(|k| view! {
                            <button
                                class=move || if slide.get() == k { "g2-dot g2-dot-on" } else { "g2-dot" }
                                aria-label=move || format!("Show screenshot {}", k + 1)
                                on:click=move |_| set_slide.set(k)></button>
                        }).collect::<Vec<_>>()}
                    </div>
                    <button class="g2-caro-arrow" aria-label="Next screenshot"
                        on:click=move |_| set_slide.update(|i| *i = (*i + 1) % n)>"\u{203a}"</button>
                </div>
            })}

            // click-to-zoom modal (centered overlay), closes on backdrop click
            {move || modal.get().then(|| {
                let s = cur.get();
                view! {
                    <Portal>
                        <div class="g2-modal" role="dialog" aria-modal="true" on:click=move |_| set_modal.set(false)>
                            <img class="g2-modal-img" src=s.image alt=s.alt/>
                        </div>
                    </Portal>
                }
            })}
        </figure>
    }
}

fn download_button(d: &'static DownloadLink) -> impl IntoView {
    view! {
        <a class="g2-dl" href=d.url target="_blank" rel="noreferrer">
            <span class="g2-dl-label">{d.label}</span>
            <svg class="g2-dl-ext" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"/>
            </svg>
        </a>
    }
}

// ── main component ───────────────────────────────────────────────────────────

#[component]
pub fn StepperV2(
    guide: &'static GuideV2,
    downloads: Vec<&'static DownloadLink>,
) -> impl IntoView {
    let steps = guide.steps;
    let n_steps = steps.len();
    let total = n_steps + 2; // intro + steps + completion
    let last = total - 1;

    // Step state from the URL query (?step=N). SSR reads the request query, so a
    // deep link renders the right panel server-side.
    let query = use_query_map();
    let location = use_location();
    let navigate = use_navigate();
    let current = Signal::derive(move || {
        query
            .read()
            .get("step")
            .and_then(|s| s.parse::<usize>().ok())
            .map(|n| n.min(last))
            .unwrap_or(0)
    });

    // Navigate to a step by pushing a new query (Back/Forward then walk the guide).
    let go_to = {
        let navigate = navigate.clone();
        move |n: usize| {
            let n = n.min(last);
            let path = location.pathname.get_untracked();
            navigate(
                &format!("{path}?step={n}"),
                NavigateOptions { scroll: false, ..Default::default() },
            );
        }
    };

    // Build the panels. Index 0 = intro, 1..=n_steps = step, last = completion.
    let intro_panel = {
        let intro = &guide.intro;
        view! {
            <div class="g2-intro">
                <h1 class="g2-h">{intro.title}</h1>
                <p class="g2-lede">{intro.lede}</p>
                <div class="g2-chips">
                    {intro.chips.iter().map(|c| view! { <span class="g2-chip">{*c}</span> }).collect::<Vec<_>>()}
                </div>
                <div class="g2-intro-grid">
                    <div class="g2-outcome">
                        <h4>"What you will have at the end"</h4>
                        <ul>
                            {intro.outcomes.iter().map(|o| view! {
                                <li><span class="g2-ck">"✓"</span><span>{*o}</span></li>
                            }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                    <div class="g2-intro-side">
                        {(!downloads.is_empty()).then(|| view! {
                            <div class="g2-get">
                                <h4>"Get the app"</h4>
                                <div class="g2-dls">
                                    {downloads.iter().map(|d| download_button(d)).collect::<Vec<_>>()}
                                </div>
                            </div>
                        })}
                        {intro.backup_cta.then(|| view! { <BackupSheetCta/> })}
                    </div>
                </div>
            </div>
        }
    };

    let step_panels = steps
        .iter()
        .enumerate()
        .map(|(si, step)| step_panel(si, n_steps, step, go_to.clone()))
        .collect::<Vec<_>>();

    let completion_panel = {
        let c = &guide.completion;
        view! {
            <div class="g2-done">
                <div class="g2-seal">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>
                </div>
                <div class="g2-eyebrow">"Now you're hodling"</div>
                <h1 class="g2-h">{c.title}</h1>
                <p class="g2-lede g2-center">{c.lede}</p>
                <div class="g2-done-cards">
                    {c.backup_cta.then(|| view! { <BackupSheetCta/> })}
                    {c.next_tier.map(|(label, href)| view! {
                        <a class="g2-next-tier" href=href>
                            <span class="g2-nt-k">"Grow into it"</span>
                            <span class="g2-nt-t">{label}</span>
                            <span class="g2-nt-s">"Ready for a bigger stack? Here is your next step up."</span>
                        </a>
                    })}
                </div>
            </div>
        }
    };

    // Footer nav labels/state.
    let next_label = Signal::derive(move || match current.get() {
        0 => "Start setup",
        c if c == last => "",
        _ => "Next step",
    });
    let status = Signal::derive(move || {
        let c = current.get();
        if c == 0 {
            "Overview".to_string()
        } else if c == last {
            "Complete".to_string()
        } else {
            format!("Step {} of {}", c, n_steps)
        }
    });

    let go_prev = {
        let go_to = go_to.clone();
        move |_| go_to(current.get_untracked().saturating_sub(1))
    };
    let go_next = {
        let go_to = go_to.clone();
        move |_| go_to(current.get_untracked() + 1)
    };

    view! {
        <div class="g2-wrap">
            // slim progress
            <div class="g2-prog">
                <div class="g2-prog-fill"
                    style=move || format!("width:{}%", (current.get() as f32 / last as f32) * 100.0)></div>
            </div>

            // panels (all in DOM; only active shown)
            <div class="g2-stage">
                <div class="g2-panel" class:g2-active=move || current.get() == 0>{intro_panel}</div>
                {step_panels.into_iter().enumerate().map(|(si, panel)| {
                    let idx = si + 1;
                    view! {
                        <div class="g2-panel" class:g2-active=move || current.get() == idx>{panel}</div>
                    }
                }).collect::<Vec<_>>()}
                <div class="g2-panel" class:g2-active=move || current.get() == last>{completion_panel}</div>
            </div>

            // sticky footer nav
            <div class="g2-nav">
                <div class="g2-nav-inner">
                    <button class="g2-btn-ghost" class:g2-hidden=move || current.get() == 0 on:click=go_prev>
                        "‹ Back"
                    </button>
                    <div class="g2-status">{status}</div>
                    <button class="g2-btn-primary" class:g2-hidden=move || current.get() == last on:click=go_next>
                        {move || next_label.get()}" →"
                    </button>
                </div>
            </div>
        </div>
    }
}

/// One step panel: rail (clickable step circles), two-pane (actions | device frame).
fn step_panel(
    si: usize,
    n_steps: usize,
    step: &'static Step,
    go_to: impl Fn(usize) + Clone + 'static,
) -> AnyView {
    // Steps without a screenshot (e.g. many hardware/node steps) render as a
    // single centered column instead of the two-pane. Desktop steps that DO have
    // shots stack the actions above a full-width window frame, so wide landscape
    // screenshots stay large instead of being crushed into the 280px phone column.
    let has_device = !step.device.shots.is_empty();
    let is_stack = has_device && matches!(step.device.frame, Frame::Desktop);
    let pane_cls = if !has_device {
        "g2-pane g2-solo"
    } else if is_stack {
        "g2-pane g2-stack"
    } else {
        "g2-pane"
    };
    view! {
        <div class="g2-step">
            // rail — each circle jumps to that step (URL step index = k + 1)
            <div class="g2-rail" role="list" aria-label="Guide progress">
                {(0..n_steps).map(|k| {
                    let cls = if k < si { "g2-node g2-node-done" }
                        else if k == si { "g2-node g2-node-cur" }
                        else { "g2-node" };
                    let is_last = k == n_steps - 1;
                    let go = go_to.clone();
                    view! {
                        <button class=cls aria-label=move || format!("Go to step {}", k + 1) on:click=move |_| go(k + 1)>{k + 1}</button>
                        {(!is_last).then(|| view! {
                            <div class=if k < si { "g2-seg g2-seg-fill" } else { "g2-seg" }></div>
                        })}
                    }
                }).collect::<Vec<_>>()}
            </div>

            <div class=pane_cls>
                <div class="g2-left">
                    <h2 class="g2-h2">{step.title}</h2>
                    <div class="g2-goal"><span><b>"Goal: "</b>{step.goal}</span></div>
                    <ol class="g2-actions">
                        {step.actions.iter().map(|a| view! { <li><span class="g2-action-txt">{inline(a)}</span></li> }).collect::<Vec<_>>()}
                    </ol>
                    {step.flag.map(|f| view! {
                        <div class="g2-flag"><span class="g2-flag-i">"⚠"</span><span>{f}</span></div>
                    })}
                    {step.backup_cta.then(|| view! { <BackupSheetCta/> })}
                    {step.why.map(|(summary, body)| view! {
                        <details class="g2-why">
                            <summary>{summary}</summary>
                            <div class="g2-why-body">{body}</div>
                        </details>
                    })}
                    {(!step.needs.is_empty()).then(|| view! {
                        <div class="g2-needs">
                            <span class="g2-needs-lab">"You will need"</span>
                            {step.needs.iter().map(|n| view! { <span class="g2-pill">{*n}</span> }).collect::<Vec<_>>()}
                        </div>
                    })}
                </div>
                {has_device.then(|| view! {
                    <div class="g2-right">
                        <DeviceFrame device=&step.device/>
                    </div>
                })}
            </div>
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_text() {
        assert_eq!(parse_inline("hello world"), vec![Seg::Text("hello world".into())]);
    }

    #[test]
    fn parses_bold() {
        assert_eq!(
            parse_inline("tap **Add a wallet**."),
            vec![
                Seg::Text("tap ".into()),
                Seg::Bold("Add a wallet".into()),
                Seg::Text(".".into()),
            ]
        );
    }

    #[test]
    fn parses_link() {
        assert_eq!(
            parse_inline("see [mempool](https://mempool.space) now"),
            vec![
                Seg::Text("see ".into()),
                Seg::Link("mempool".into(), "https://mempool.space".into()),
                Seg::Text(" now".into()),
            ]
        );
    }

    #[test]
    fn unterminated_bold_is_literal() {
        assert_eq!(parse_inline("a ** b"), vec![Seg::Text("a ** b".into())]);
    }
}
