//! JSON-LD structured data (schema.org).
//!
//! Two reasons this exists: search engines use it for rich results and entity
//! resolution, and language models lean on it heavily when summarising a page,
//! which is increasingly how readers arrive.
//!
//! Two kinds of document live here. The functions below build schema from the guide data
//! at render time, so they cannot drift from the page they describe. [`static_docs`] holds
//! the hand-authored documents, embedded with `include_str!` from `assets/schema/`, each
//! rendered only on the page it is about.
//!
//! Emitted via `inner_html` rather than as `<script>` children on purpose. A text
//! node inside `<script>` in the `view!` macro breaks hydration, so the JSON has to
//! be set as raw HTML instead.
//!
//! Every string is escaped by `serde_json`, and `<` is additionally escaped to
//! `<` so no value can terminate the script element early.

use leptos::prelude::*;
use serde_json::{json, Value};

/// Canonical origin. Matches the `rel=canonical` hrefs, which must agree or the
//  structured data points at a different URL than the page claims to be.
pub const SITE: &str = "https://www.wehodlbtc.com";

const BRAND: &str = "We Hodl BTC";

/// Serialise and neutralise `<` so a value cannot close the script element.
fn render(v: Value) -> String {
    v.to_string().replace('<', "\\u003c")
}

/// Strip the inline markdown the guide data uses. Structured data wants plain
/// prose: `**bold**` and `[text](url)` would otherwise leak into snippets.
pub fn strip_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
            }
            '[' => {
                // Keep the label, drop the target.
                for lc in chars.by_ref() {
                    if lc == ']' {
                        break;
                    }
                    out.push(lc);
                }
                if chars.peek() == Some(&'(') {
                    for uc in chars.by_ref() {
                        if uc == ')' {
                            break;
                        }
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Renders a JSON-LD block. `data` must already be serialised.
#[component]
pub fn JsonLd(data: String) -> impl IntoView {
    view! { <script type="application/ld+json" inner_html=data></script> }
}

/// Renders a compile-time schema document from [`static_docs`].
#[component]
pub fn StaticJsonLd(doc: &'static str) -> impl IntoView {
    view! { <script type="application/ld+json" inner_html=doc></script> }
}

/// Hand-authored schema documents, embedded at compile time.
///
/// Each renders only on the page it describes, so no node claims something untrue of the
/// page carrying it. Server-rendered so consumers that do not execute JavaScript still
/// see them, and `include_str!` means the tests below can reject a malformed document at
/// build time.
pub mod static_docs {
    /// `Organization` + `WebSite`. Site-level identity belongs on the root only.
    pub const SITE: &str = include_str!("../../assets/schema/site.json");
    /// `WebApplication` + the blockchain-statistics `Dataset`.
    pub const OBSERVATORY: &str = include_str!("../../assets/schema/observatory.json");
    /// Embedded-data `Dataset`, on the chart page it measures.
    pub const DATASET_EMBEDDED: &str =
        include_str!("../../assets/schema/dataset-embedded.json");
    /// BIP-signaling `Dataset`, on the signaling page.
    pub const DATASET_SIGNALING: &str =
        include_str!("../../assets/schema/dataset-signaling.json");
    /// `Article` for the embedding-protocols guide.
    pub const ARTICLE_PROTOCOLS: &str =
        include_str!("../../assets/schema/article-protocols.json");
    /// `Article` for the data-methodology write-up.
    pub const ARTICLE_METHODOLOGY: &str =
        include_str!("../../assets/schema/article-methodology.json");
    /// `FAQPage`, on `/faq` and nowhere else.
    pub const FAQ: &str = include_str!("../../assets/schema/faq.json");

    /// Every document, for the validation tests.
    pub const ALL: &[(&str, &str)] = &[
        ("site", SITE),
        ("observatory", OBSERVATORY),
        ("dataset-embedded", DATASET_EMBEDDED),
        ("dataset-signaling", DATASET_SIGNALING),
        ("article-protocols", ARTICLE_PROTOCOLS),
        ("article-methodology", ARTICLE_METHODOLOGY),
        ("faq", FAQ),
    ];
}

fn publisher() -> Value {
    json!({
        "@type": "Organization",
        "name": BRAND,
        "url": SITE,
        "logo": format!("{SITE}/img/metadata_unfurl_image.png"),
        "sameAs": ["https://x.com/bayernatoor"],
    })
}

/// Breadcrumb trail. Takes the same `(label, href)` pairs the visual breadcrumbs
/// use, so the two cannot disagree. Empty hrefs mark the current page.
pub fn breadcrumbs(crumbs: &[(String, String)]) -> String {
    let mut items: Vec<Value> = vec![json!({
        "@type": "ListItem",
        "position": 1,
        "name": "Guides",
        "item": format!("{SITE}/guides"),
    })];
    for (i, (label, href)) in crumbs.iter().enumerate() {
        let mut item = json!({
            "@type": "ListItem",
            "position": i + 2,
            "name": label,
        });
        if !href.is_empty() {
            item["item"] = json!(format!("{SITE}{href}"));
        }
        items.push(item);
    }
    render(json!({
        "@context": "https://schema.org",
        "@type": "BreadcrumbList",
        "itemListElement": items,
    }))
}

/// A step for [`how_to`]: heading, the goal line, its ordered actions, and the
/// first screenshot if the step has one.
pub struct SchemaStep {
    pub name: String,
    pub goal: String,
    pub actions: Vec<String>,
    pub image: Option<String>,
}

/// `HowTo` for a wallet setup guide. This is the schema that actually matches what
/// these pages are, and it gives a model the ordered steps without having to infer
/// them from markup.
pub fn how_to(
    name: &str,
    description: &str,
    url: &str,
    minutes: Option<u32>,
    supplies: &[String],
    steps: &[SchemaStep],
) -> String {
    let step_items: Vec<Value> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let text = if s.actions.is_empty() {
                s.goal.clone()
            } else {
                format!("{} {}", s.goal, s.actions.join(" "))
            };
            let mut v = json!({
                "@type": "HowToStep",
                "position": i + 1,
                "name": s.name,
                "text": text,
                "url": format!("{url}?step={}", i + 1),
            });
            if let Some(img) = &s.image {
                v["image"] = json!(format!("{SITE}{img}"));
            }
            v
        })
        .collect();

    let mut doc = json!({
        "@context": "https://schema.org",
        "@type": "HowTo",
        "name": name,
        "description": description,
        "url": url,
        "inLanguage": "en",
        "publisher": publisher(),
        "step": step_items,
    });
    if let Some(m) = minutes {
        doc["totalTime"] = json!(format!("PT{m}M"));
    }
    if !supplies.is_empty() {
        doc["supply"] = json!(supplies
            .iter()
            .map(|s| json!({ "@type": "HowToSupply", "name": s }))
            .collect::<Vec<_>>());
    }
    render(doc)
}

/// `Article` for a dated, evolving write-up such as the security advisory. The
/// dates matter most: readers and models both need to know how current it is.
pub fn article(
    headline: &str,
    description: &str,
    url: &str,
    published_iso: &str,
    modified_iso: &str,
) -> String {
    render(json!({
        "@context": "https://schema.org",
        "@type": "Article",
        "headline": headline,
        "description": description,
        "url": url,
        "mainEntityOfPage": { "@type": "WebPage", "@id": url },
        "datePublished": published_iso,
        "dateModified": modified_iso,
        "inLanguage": "en",
        "author": publisher(),
        "publisher": publisher(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bold_and_link_targets() {
        assert_eq!(strip_md("tap **Next** now"), "tap Next now");
        assert_eq!(
            strip_md("see [BDK](https://bitcoindevkit.org) docs"),
            "see BDK docs"
        );
        assert_eq!(
            strip_md("**[Bull's post](https://example.com)**"),
            "Bull's post"
        );
        assert_eq!(strip_md("no markup here"), "no markup here");
    }

    /// A stray `</script>` in guide copy would otherwise end the block early and
    /// let the rest of the JSON render as page text.
    #[test]
    fn escapes_angle_brackets() {
        let out = article("a</script>b", "d", "u", "2026-01-01", "2026-01-02");
        assert!(!out.contains("</script>"));
        assert!(out.contains("\\u003c"));
    }

    #[test]
    fn breadcrumbs_number_from_guides_root() {
        let out = breadcrumbs(&[
            ("Basic".into(), "/guides/basic".into()),
            ("Cove".into(), String::new()),
        ]);
        assert!(out.contains("\"position\":1"));
        assert!(out.contains("\"position\":3"));
        // The current page carries no `item` URL.
        assert!(out.contains("\"name\":\"Cove\""));
    }

    #[test]
    fn how_to_emits_ordered_steps_and_optional_fields() {
        let steps = vec![SchemaStep {
            name: "Install".into(),
            goal: "Get the app.".into(),
            actions: vec!["Tap Next.".into()],
            image: Some("/guide-images/x.png".into()),
        }];
        let out = how_to("G", "d", "https://e/x", Some(20), &["Phone".into()], &steps);
        assert!(out.contains("\"@type\":\"HowTo\""));
        assert!(out.contains("\"totalTime\":\"PT20M\""));
        assert!(out.contains("HowToSupply"));
        assert!(out.contains("Get the app. Tap Next."));
        assert!(out.contains("?step=1"));
    }

    #[test]
    fn how_to_omits_absent_optional_fields() {
        let out = how_to("G", "d", "https://e/x", None, &[], &[]);
        assert!(!out.contains("totalTime"));
        assert!(!out.contains("supply"));
    }

    /// Malformed JSON-LD is dropped whole by crawlers, so it must not reach a release.
    #[test]
    fn every_static_document_is_valid_json_ld() {
        for (name, doc) in static_docs::ALL {
            let v: Value = serde_json::from_str(doc)
                .unwrap_or_else(|e| panic!("{name}: invalid JSON: {e}"));
            assert_eq!(v["@context"], "https://schema.org", "{name}: missing @context");
            let has_type = v.get("@type").is_some()
                || v["@graph"].as_array().is_some_and(|g| {
                    !g.is_empty() && g.iter().all(|n| n.get("@type").is_some())
                });
            assert!(has_type, "{name}: no @type on the node or its @graph members");
        }
    }

    /// Each document renders on exactly one page, so a node that names a URL must name
    /// the page it lives on.
    #[test]
    fn static_documents_point_at_their_own_page() {
        let expected = [
            ("faq", "/faq"),
            ("article-protocols", "/observatory/learn/protocols"),
            ("article-methodology", "/observatory/learn/methodology"),
            ("dataset-embedded", "/observatory/charts/embedded"),
            ("dataset-signaling", "/observatory/signaling"),
        ];
        for (name, path) in expected {
            let doc = static_docs::ALL.iter().find(|(n, _)| *n == name).unwrap().1;
            let v: Value = serde_json::from_str(doc).unwrap();
            let url = v["url"].as_str().unwrap_or_default();
            assert_eq!(url, format!("{SITE}{path}"), "{name}: url does not match its page");
        }
    }

    /// Exactly one document may *declare* the site as a top-level entity. Inline
    /// `provider`/`creator`/`publisher` objects are references, not declarations, and are
    /// expected on the pages that carry a Dataset or WebApplication.
    #[test]
    fn only_the_site_document_declares_website_identity() {
        for (name, doc) in static_docs::ALL {
            let v: Value = serde_json::from_str(doc).unwrap();
            let top: Vec<&str> = match v["@graph"].as_array() {
                Some(g) => g.iter().filter_map(|n| n["@type"].as_str()).collect(),
                None => v["@type"].as_str().into_iter().collect(),
            };
            let declares = top
                .iter()
                .any(|t| *t == "WebSite" || *t == "Organization");
            assert_eq!(
                declares,
                *name == "site",
                "{name}: top-level types {top:?}; site identity belongs only in site.json"
            );
        }
    }
}
