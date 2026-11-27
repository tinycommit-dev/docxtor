//! Hostile text in a document must come out of the exports as text: Markdown that cannot run script when it is rendered, a web page whose
//! style sheet cannot be ended from a font name, and pictures that are SVG with script in them. (The browser side, with whole hostile files
//! in the app, is e2e/hostile.spec.ts.)

use docxtor_core::export::{epub::to_epub, html::to_html, to_markdown};
use docxtor_core::Document;
use serde_json::{json, Value};
use std::io::Read;

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn paragraph_with(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":text}));
    (doc, first)
}

#[test]
fn markdown_keeps_html_in_the_text_as_text() {
    let (doc, _) = paragraph_with("<img src=x onerror=alert(1)> and <script>alert(2)</script> and <javascript:alert(3)>");
    let md = to_markdown(&doc);
    // every `<` has a backslash before it, so a renderer shows it and starts no tag
    assert!(md.match_indices('<').all(|(i, _)| md[..i].ends_with('\\')), "{md}");
    assert!(md.contains("\\<img src=x onerror=alert(1)>"), "{md}");
}

#[test]
fn markdown_drops_links_that_run_script_and_keeps_the_text() {
    for url in ["javascript:alert(1)", "JaVaScRiPt:alert(1)", "data:text/html,<script>alert(1)</script>", "vbscript:x", "file:///etc/passwd"] {
        let (mut doc, p) = paragraph_with("click here now");
        ops(&mut doc, json!({"op":"set_link","from":{"p":p,"o":6},"to":{"p":p,"o":10},"url":url}));
        let md = to_markdown(&doc);
        assert!(md.contains("click here now"), "{url}: {md}");
        assert!(!md.contains("](") && !md.to_ascii_lowercase().contains("script:") && !md.contains("file:"), "{url}: {md}");
    }
    // A web address stays a link, and a `)` or a space in it cannot end the link early.
    let (mut doc, p) = paragraph_with("click here now");
    ops(&mut doc, json!({"op":"set_link","from":{"p":p,"o":6},"to":{"p":p,"o":10},"url":"https://example.com/a (b) c"}));
    let md = to_markdown(&doc);
    assert!(md.contains("[here](https://example.com/a%20%28b%29%20c)"), "{md}");
}

#[test]
fn the_web_page_font_names_cannot_end_the_style_sheet() {
    let (mut doc, p) = paragraph_with("styled");
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":6},"format":{"font":"</style><script>alert(1)</script><style>"}}));
    let html = to_html(&doc);
    assert!(!html.contains("<script"), "{html}");
    assert_eq!(html.matches("</style>").count(), 1, "{html}");
    let (mut doc, p) = paragraph_with("styled");
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":6},"format":{"font":"Arial\\"}}));
    let html = to_html(&doc);
    assert!(!html.contains("Arial\\"), "{html}");
}

#[test]
fn svg_pictures_leave_the_exports_without_script() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"><script>alert(2)</script><rect width="9" height="9" onclick="alert(3)"/><image href="http://evil.test/x.png"/></svg>"##;
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"Picture: X"}));
    let media = doc.add_media("svg", svg.to_vec());
    let mut id = None;
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains("Picture:") {
            id = Some(p.id);
        }
    });
    ops(&mut doc, json!({"op":"insert_image","at":{"p":id.unwrap(),"o":9},"media":media,"width":96,"height":48,"alt":"drawing"}));
    // the web page has it as a data address: decode it
    let html = to_html(&doc);
    let at = html.find("data:image/svg+xml;base64,").expect("the svg is in the page") + "data:image/svg+xml;base64,".len();
    let b64: String = html[at..].chars().take_while(|c| *c != '"').collect();
    let decoded = decode_base64(&b64);
    for bad in ["script", "onload", "onclick", "evil.test"] {
        assert!(!decoded.contains(bad), "{bad} in {decoded}");
    }
    assert!(decoded.contains("<rect"), "{decoded}");
    // and the book has it as a file
    let epub = to_epub(&doc, Some("2026-01-01T00:00:00Z")).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(epub)).unwrap();
    let mut checked = 0;
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap();
        if f.name().unwrap().ends_with(".svg") {
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            for bad in ["script", "onload", "onclick", "evil.test"] {
                assert!(!s.contains(bad), "{bad} in {s}");
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 1);
}

fn decode_base64(s: &str) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| *c != b'=') {
        acc = acc << 6 | T.iter().position(|t| *t == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    String::from_utf8(out).unwrap()
}
