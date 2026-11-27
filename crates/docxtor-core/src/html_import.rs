//! HTML to a `Fragment`: what the clipboard carries when text is copied from Word, Google Docs, web pages
//! and other Docxtor documents, and what a `.html` file holds. The reader is forgiving (it takes the
//! HTML browsers and Word write, not only well-formed markup) and keeps fonts, sizes, colours, highlights,
//! alignment, indents, spacing, lists (including Word's `mso-list`), tables, pictures, links and equations.
//! Pictures come from `data:` addresses or from the bytes the caller fetched, by address.

mod convert;
mod css;
mod dom;
mod table;

use crate::fragment::Fragment;
use std::collections::HashMap;

const START: &str = "<!--StartFragment-->";
const END: &str = "<!--EndFragment-->";

/// Read HTML into a fragment. `images` holds pictures the caller fetched, by the address in the markup.
/// The clipboard's `StartFragment` and `EndFragment` markers say which part was copied, and only that is read.
pub fn html_to_fragment(html: &str, images: &HashMap<String, Vec<u8>>) -> Fragment {
    read_html(html, images, true)
}

/// Read a whole HTML page (a file, a chunk of a document): the clipboard's markers are ordinary comments here and
/// everything in the body is read.
pub fn html_page_to_fragment(html: &str, images: &HashMap<String, Vec<u8>>) -> Fragment {
    read_html(html, images, false)
}

fn read_html(html: &str, images: &HashMap<String, Vec<u8>>, markers: bool) -> Fragment {
    // Too long to be something pasted: nothing is read (the budget's paste size, see `crate::budget`).
    if crate::budget::within(html.len(), crate::budget::caps::PASTE_BYTES).is_err() {
        return Fragment::default();
    }
    // The whole page gives the styles; the clipboard's markers say which part was copied.
    let all = dom::parse(html);
    let mut sheet = css::Sheet::default();
    css::collect_styles(&all, &mut sheet);
    let marked = match (html.find(START), html.find(END)) {
        (Some(a), Some(b)) if markers && a + START.len() <= b => Some(dom::parse(&html[a + START.len()..b])),
        _ => None,
    };
    let nodes = marked.unwrap_or(all);
    let body = find_body(&nodes).unwrap_or(&nodes);
    let mut conv = convert::Conv::new(sheet, images);
    conv.inline_only = !body.iter().any(|n| matches!(n, dom::HNode::El(e) if convert::is_block(e) || e.any(&convert::is_block)));
    let mut out = Vec::new();
    conv.blocks(body, &convert::Ctx::default(), &convert::Run::default(), &mut out);
    conv.finish(out)
}

fn find_body(nodes: &[dom::HNode]) -> Option<&[dom::HNode]> {
    for n in nodes {
        if let dom::HNode::El(e) = n {
            if e.tag == "body" {
                return Some(&e.children);
            }
            if let Some(b) = find_body(&e.children) {
                return Some(b);
            }
        }
    }
    None
}

/// The addresses of the pictures in some HTML that are not `data:` addresses, in order, without repeats.
/// The caller can fetch these and pass the bytes to `html_to_fragment`.
pub fn html_image_sources(html: &str) -> Vec<String> {
    fn go(nodes: &[dom::HNode], out: &mut Vec<String>) {
        for n in nodes {
            if let dom::HNode::El(e) = n {
                if e.tag == "img" {
                    if let Some(src) = e.attr("src").map(str::trim) {
                        if !src.is_empty() && !src.starts_with("data:") && !out.iter().any(|s| s == src) {
                            out.push(src.to_string());
                        }
                    }
                }
                go(&e.children, out);
            }
        }
    }
    let mut out = Vec::new();
    go(&dom::parse(html), &mut out);
    out
}
