//! Shared markdown-to-HTML conversion used by stepper and accordion.

use pulldown_cmark::{html, Options, Parser};

pub fn to_html(markdown: &str) -> String {
    let options = Options::empty();
    let parser = Parser::new_ext(markdown, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}
