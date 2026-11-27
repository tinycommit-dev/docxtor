//! A comment's anchor in the text takes no room and shows nothing, and the file keeps it.

use docxtor_core::model::{walk_paragraphs, write_block};
use docxtor_core::{package::Package, Document};

fn doc_with(body: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let s = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    pkg.set("word/document.xml", s.replacen("<w:p/>", body, 1).into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

#[test]
fn comment_reference_is_zero_width_and_kept() {
    let body = r#"<w:p><w:r><w:t>Before </w:t></w:r><w:commentRangeStart w:id="0"/><w:r><w:t>commented</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="0"/></w:r><w:r><w:t> after</w:t></w:r></w:p>"#;
    let mut doc = doc_with(body);
    let mut text = String::new();
    let mut len = 0;
    walk_paragraphs(&doc.body, &mut |p| {
        text.push_str(&p.text());
        len += p.len16();
    });
    assert_eq!(text, "Before commented after");
    assert_eq!(len, "Before commented after".encode_utf16().count());
    // The render has no object segment for it.
    let json = serde_json::to_string(&doc.render_all()).unwrap();
    assert!(!json.contains("\"obj\""), "{json}");
    assert!(!json.to_lowercase().contains("object\""), "no Object chip: {json}");
    // The file keeps the markers.
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains(r#"<w:commentRangeStart w:id="0"/>"#));
    assert!(xml.contains(r#"<w:commentRangeEnd w:id="0"/>"#));
    assert!(xml.contains(r#"<w:commentReference w:id="0"/>"#));
    let mut again = Document::open(&bytes).unwrap();
    let first = again.body.iter().find_map(|b| if let docxtor_core::model::Block::Para(p) = b { Some(write_block(&docxtor_core::model::Block::Para(p.clone())).to_xml()) } else { None }).unwrap();
    assert!(first.contains("w:commentReference"));
    let _ = again.save();
}
