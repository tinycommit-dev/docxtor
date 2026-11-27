//! Pictures that are SVG, made safe to put in another file.
//!
//! An SVG is a program as well as a picture: it can hold script, event handlers, links that run script, `foreignObject` with a whole
//! web page in it, and references that make a viewer fetch something from the internet. The exporters (web page, EPUB, OpenDocument)
//! write a document's pictures into files other programs open, so an SVG goes through `clean_svg` first. What stays is the drawing:
//! shapes, text, gradients, styles, and pictures and references inside the file.

use crate::xml::{self, Element, Node};

/// Elements that run code, hold another document or play media; their whole subtree goes.
const DROP: &[&str] = &[
    "script", "foreignobject", "iframe", "frame", "object", "embed", "audio", "video", "canvas", "animate", "animatemotion", "animatetransform", "animatecolor", "set", "discard",
    "handler", "listener", "html", "body", "head", "link", "meta", "style_link",
];

/// An address a picture may hold: a place in the same file, or a raster picture written inline.
fn address_ok(v: &str) -> bool {
    let v = v.trim_start();
    let lower: String = v.chars().take(24).collect::<String>().to_ascii_lowercase();
    v.starts_with('#') || ["data:image/png", "data:image/jpeg", "data:image/jpg", "data:image/gif", "data:image/webp"].iter().any(|p| lower.starts_with(p))
}

/// Whether a value has an address that runs script, however it is spelled (case, tabs and breaks inside the word).
fn runs_script(v: &str) -> bool {
    let squeezed: String = v.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect::<String>().to_ascii_lowercase();
    squeezed.contains("javascript:") || squeezed.contains("vbscript:") || squeezed.contains("data:text/html") || squeezed.contains("data:application")
}

/// CSS with every `@import` and every `url(...)` that points outside the file taken out (`url(...)` becomes `none`). CSS that tries
/// anything cleverer (escapes, `expression(`, bindings) is dropped whole.
pub fn css_clean(css: &str) -> String {
    let lower = css.to_ascii_lowercase();
    if css.contains('\\') || ["expression(", "javascript:", "vbscript:", "-moz-binding", "behavior:", "<"].iter().any(|t| lower.contains(t)) {
        return String::new();
    }
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    while i < css.len() {
        let rest = &lower[i..];
        if rest.starts_with("@import") {
            i += rest.find(';').map_or(rest.len(), |e| e + 1);
        } else if rest.starts_with("url(") {
            let Some(close) = rest.find(')') else { break };
            let inner = css[i + 4..i + close].trim().trim_matches(|c| c == '"' || c == '\'').trim();
            out.push_str(if address_ok(inner) { &css[i..=i + close] } else { "none" });
            i += close + 1;
        } else {
            let c = css[i..].chars().next().unwrap();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

fn clean_element(e: &mut Element) {
    let in_style = e.local().eq_ignore_ascii_case("style");
    e.attrs.retain_mut(|(name, value)| {
        let local = xml::local_name(name.as_str()).to_ascii_lowercase();
        if local.starts_with("on") {
            return false;
        }
        let v = value.as_str();
        if matches!(local.as_str(), "href" | "src") {
            return address_ok(v);
        }
        if runs_script(v) {
            return false;
        }
        if local == "style" || v.to_ascii_lowercase().contains("url(") {
            let cleaned = css_clean(v);
            if cleaned.is_empty() && !v.trim().is_empty() {
                return false;
            }
            *value = cleaned.into();
        }
        true
    });
    e.children.retain_mut(|n| match n {
        Node::Element(c) => {
            let local = c.local().to_ascii_lowercase();
            if DROP.contains(&local.as_str()) {
                return false;
            }
            clean_element(c);
            true
        }
        Node::Text(t) | Node::CData(t) if in_style => {
            *t = css_clean(t);
            true
        }
        Node::PI(_) => false,
        _ => true,
    });
}

/// The SVG with script, handlers, media, nested documents and outside references taken out, or None when it is not an SVG document.
pub fn clean_svg(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut doc = xml::parse(bytes).ok()?;
    if !doc.root.local().eq_ignore_ascii_case("svg") {
        return None;
    }
    // A document type can define entities and name outside files; none is kept.
    doc.prolog.clear();
    clean_element(&mut doc.root);
    // The root itself: handlers and addresses.
    Some(doc.to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean(s: &str) -> String {
        String::from_utf8(clean_svg(s.as_bytes()).expect("an svg")).unwrap()
    }

    #[test]
    fn script_handlers_and_nested_documents_go_the_drawing_stays() {
        let out = clean(
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" onload="x()"><script>x()</script><rect width="5" fill="red" onclick="x()"/><a xlink:href="javascript:x()"><text>t</text></a><foreignObject><body xmlns="http://www.w3.org/1999/xhtml"><img src="http://e.test/x.png"/></body></foreignObject><use href="#a"/><use href="http://e.test/a.svg#a"/><image href="http://e.test/i.png"/><image href="data:image/png;base64,AAAA"/><set attributeName="onmouseover" to="x()"/></svg>"##,
        );
        for gone in ["script", "onload", "onclick", "javascript", "foreignObject", "e.test", "<set"] {
            assert!(!out.contains(gone), "{gone} is still in {out}");
        }
        for kept in ["<rect", "fill=\"red\"", "<text>t</text>", "href=\"#a\"", "data:image/png;base64,AAAA"] {
            assert!(out.contains(kept), "{kept} is gone from {out}");
        }
    }

    #[test]
    fn styles_lose_imports_and_outside_urls() {
        let out = clean(r##"<svg xmlns="http://www.w3.org/2000/svg"><style>@import url(http://e.test/a.css); .a { fill: url(http://e.test/f.svg#g); stroke: url(#g) }</style><rect style="fill:url('http://e.test/x');stroke:red" fill="url(//e.test/y)"/><circle style="width:expression(x())"/></svg>"##);
        assert!(!out.contains("e.test") && !out.contains("@import") && !out.contains("expression"), "{out}");
        assert!(out.contains("stroke: url(#g)") && out.contains("stroke:red"), "{out}");
    }

    #[test]
    fn an_address_split_by_whitespace_or_in_another_case_is_still_script() {
        let out = clean("<svg xmlns=\"http://www.w3.org/2000/svg\"><desc title=\"JaVa\tScRiPt:x()\">d</desc><a href=\" javascript:x()\"><text>t</text></a></svg>");
        assert!(!out.to_ascii_lowercase().contains("script:") && !out.contains("java\t"), "{out}");
    }

    #[test]
    fn a_page_that_is_not_an_svg_is_refused() {
        assert!(clean_svg(b"<html><script>x()</script></html>").is_none());
        assert!(clean_svg(b"not xml at all <svg").is_none());
    }

    #[test]
    fn a_document_type_with_entities_is_dropped() {
        let out = clean("<?xml version=\"1.0\"?><!DOCTYPE svg [<!ENTITY a \"b\">]><svg xmlns=\"http://www.w3.org/2000/svg\"><text>t</text></svg>");
        assert!(!out.contains("ENTITY") && !out.contains("DOCTYPE"), "{out}");
    }
}
