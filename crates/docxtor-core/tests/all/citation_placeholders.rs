//! Word's "Add new placeholder": a citation of a source that has only a tag, filled in later.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
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

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

#[test]
fn a_placeholder_is_cited_by_its_tag_until_it_is_filled_in() {
    for style in ["apa", "ieee", "chicago"] {
        let mut doc = doc_with("Claim one. Claim two.");
        ops(&mut doc, json!({"op":"set_citation_style","style":style}));
        let p = find_p(&doc, "Claim one");
        ops(&mut doc, json!({"op":"insert_citation_placeholder","at":{"p":p,"o":10}}));
        ops(&mut doc, json!({"op":"insert_citation_placeholder","at":{"p":p,"o":0},"tag":"Later"}));
        let sources = doc.sources();
        assert_eq!(sources.iter().map(|s| s.tag.as_str()).collect::<Vec<_>>(), ["Placeholder1", "Later"], "{style}");
        assert!(sources.iter().all(|s| s.is_placeholder()));
        let t = texts(&doc);
        assert!(t[0].contains("(Later)") && t[0].contains("(Placeholder1)"), "{style}: {t:?}");
        // The bibliography leaves placeholders out.
        ops(&mut doc, json!({"op":"insert_bibliography","after":p}));
        assert!(!texts(&doc).iter().any(|x| x.contains("Placeholder1") && !x.contains("Claim")), "{style}: {:?}", texts(&doc));
        // Fill one in: the citation and the bibliography follow.
        ops(&mut doc, json!({"op":"update_source","tag":"Placeholder1","source":{"tag":"Placeholder1","type":"book","authors":[{"family":"Smith","given":"Ann"}],"title":"A Real Book","year":"2020"}}));
        let t = texts(&doc);
        assert!(!t[0].contains("(Placeholder1)"), "{style}: {t:?}");
        assert!(t[0].contains("Smith") || t[0].contains("[1]"), "{style}: {t:?}");
        assert!(t.iter().any(|x| x.contains("A Real Book")), "{style}: {t:?}");
        assert_valid(&doc.save().unwrap());
        // The saved file keeps the placeholder as a source with its tag.
        let again = Document::open(&doc.save().unwrap()).unwrap();
        assert!(again.sources().iter().any(|s| s.tag == "Later" && s.is_placeholder()));
    }
}

#[test]
fn a_placeholder_tag_has_to_be_new_and_valid() {
    let mut doc = doc_with("Text.");
    let p = find_p(&doc, "Text.");
    ops(&mut doc, json!({"op":"insert_citation_placeholder","at":{"p":p,"o":0},"tag":"Todo"}));
    let e = doc.apply_json(&json!({"op":"insert_citation_placeholder","at":{"p":p,"o":0},"tag":"Todo"}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("already exists"), "{e}");
    let e = doc.apply_json(&json!({"op":"insert_citation_placeholder","at":{"p":p,"o":0},"tag":"has space"}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("can't be a source tag"), "{e}");
    assert_eq!(doc.sources().len(), 1);
    // Undo takes the source and the citation away together.
    assert!(doc.undo());
    assert!(doc.sources().is_empty());
}
