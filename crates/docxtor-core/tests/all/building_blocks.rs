//! Building blocks from a glossary document, and content controls that came from a gallery.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn find_p(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

const GLOSSARY: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:glossaryDocument xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:docParts>
<w:docPart><w:docPartPr><w:name w:val="Sign-off"/><w:category><w:name w:val="General"/><w:gallery w:val="autoText"/></w:category><w:types><w:type w:val="autoText"/></w:types><w:behaviors><w:behavior w:val="content"/></w:behaviors><w:description w:val="Closing line"/><w:guid w:val="{11111111-2222-3333-4444-555555555555}"/></w:docPartPr><w:docPartBody><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Kind regards,</w:t></w:r></w:p></w:docPartBody></w:docPart>
<w:docPart><w:docPartPr><w:name w:val="Notice"/><w:category><w:name w:val="Legal"/><w:gallery w:val="docParts"/></w:category><w:types><w:type w:val="bbPlcHdr"/></w:types><w:behaviors><w:behavior w:val="content"/></w:behaviors><w:guid w:val="{AAAAAAAA-2222-3333-4444-555555555555}"/></w:docPartPr><w:docPartBody><w:p><w:pPr><w:pStyle w:val="Notice"/></w:pPr><w:r><w:t>This is a notice.</w:t></w:r></w:p><w:p><w:r><w:t>Second line.</w:t></w:r></w:p></w:docPartBody></w:docPart>
</w:docParts></w:glossaryDocument>"#;

const GLOSSARY_STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:customStyle="1" w:styleId="Notice"><w:name w:val="Notice"/><w:basedOn w:val="Normal"/><w:rPr><w:i/></w:rPr></w:style></w:styles>"#;

fn with_glossary() -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"Dear all.\n\nBody text."}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    pkg.set("word/glossary/document.xml", GLOSSARY.as_bytes().to_vec());
    pkg.set("word/glossary/styles.xml", GLOSSARY_STYLES.as_bytes().to_vec());
    pkg.set(
        "word/glossary/_rels/document.xml.rels",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#.to_vec(),
    );
    pkg.ensure_override("word/glossary/document.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.glossary+xml");
    pkg.ensure_override("word/glossary/styles.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml");
    pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/glossaryDocument", "glossary/document.xml", false);
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

#[test]
fn the_glossary_lists_its_building_blocks_and_stays_byte_identical() {
    let mut doc = with_glossary();
    let blocks = doc.building_blocks();
    assert_eq!(blocks.len(), 2);
    assert_eq!((blocks[0].name.as_str(), blocks[0].gallery.as_deref(), blocks[0].category.as_deref(), blocks[0].description.as_deref()), ("Sign-off", Some("autoText"), Some("General"), Some("Closing line")));
    assert!(blocks[0].inline && !blocks[1].inline);
    assert_eq!(blocks[0].text, "Kind regards,");
    assert_eq!(blocks[1].text, "This is a notice. Second line.");
    assert_eq!(blocks[1].guid.as_deref(), Some("{AAAAAAAA-2222-3333-4444-555555555555}"));
    // An edit elsewhere leaves the glossary part untouched.
    let p = find_p(&doc, "Dear all");
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":">"}));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    assert_eq!(Package::open(&saved).unwrap().get("word/glossary/document.xml").unwrap(), GLOSSARY.as_bytes());
    assert_eq!(Package::open(&saved).unwrap().get("word/glossary/styles.xml").unwrap(), GLOSSARY_STYLES.as_bytes());
    assert_eq!(Document::open(&saved).unwrap().building_blocks().len(), 2);
    // A document without a glossary has none.
    assert!(Document::new_blank().building_blocks().is_empty());
}

#[test]
fn an_inline_block_goes_in_at_the_caret_and_a_block_brings_its_style() {
    let mut doc = with_glossary();
    let p = find_p(&doc, "Body text");
    let end = "Body text.".len();
    ops(&mut doc, json!({"op":"insert_building_block","at":{"p":p,"o":end},"name":"sign-off"}));
    assert!(texts(&doc).contains(&"Body text.Kind regards,".to_string()), "{:?}", texts(&doc));
    ops(&mut doc, json!({"op":"insert_building_block","at":{"p":p,"o":0},"guid":"{AAAAAAAA-2222-3333-4444-555555555555}"}));
    let t = texts(&doc);
    assert!(t.contains(&"This is a notice.".to_string()) && t.contains(&"Second line.Body text.Kind regards,".to_string()), "{t:?}");
    // The style the block uses came from the glossary's styles.
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let styles = String::from_utf8(Package::open(&saved).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"w:styleId="Notice""#) && styles.contains("<w:i/>"));
    let e = doc.apply_json(&json!({"op":"insert_building_block","at":{"p":p,"o":0},"name":"nope"}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("no such building block"));
    // Both insertions undo.
    assert!(doc.undo() && doc.undo());
    assert_eq!(texts(&doc)[1], "Body text.");
}

#[test]
fn content_controls_from_a_gallery_are_listed() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    let main = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let sdt = format!(r#"<w:sdt><w:sdtPr><w:alias w:val="Contents"/><w:docPartObj><w:docPartGallery w:val="Table of Contents"/><w:docPartCategory w:val="Built-in"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Inside the control</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p w:rsidR="{first}"/>"#);
    let _ = W;
    pkg.set("word/document.xml", main.replacen("<w:p/>", &sdt, 1).into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let c = doc.doc_part_controls();
    assert_eq!(c.len(), 1, "{c:?}");
    assert_eq!((c[0].gallery.as_str(), c[0].category.as_deref(), c[0].title.as_deref(), c[0].unique), ("Table of Contents", Some("Built-in"), Some("Contents"), true));
    // It survives a save unchanged.
    let mut doc = doc;
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    assert!(Document::open(&saved).unwrap().doc_part_controls().len() == 1);
}
