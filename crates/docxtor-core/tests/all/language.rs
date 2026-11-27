//! The language of each paragraph in the rendering view model: a passage in another language reports it,
//! and everything else follows the document default.

use docxtor_core::model::{walk_paragraphs, Inline, Run, RunItem};
use docxtor_core::render::RBlock;
use docxtor_core::{xml, Document};
use serde_json::{json, Value};

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
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

/// The language the renderer reports for each paragraph, in order.
fn langs(doc: &Document) -> Vec<Option<String>> {
    doc.render_snapshot()
        .blocks
        .iter()
        .filter_map(|b| match b {
            RBlock::Para(p) => Some(p.lang.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn paragraphs_report_their_language() {
    let mut doc = doc_with("English text\n\nTexto en español\n\nMore English");
    // The blank document's default is en-US.
    assert_eq!(langs(&doc), vec![Some("en-US".into()); 3]);

    let es = find_p(&doc, "Texto");
    ops(&mut doc, json!({"op":"set_language","lang":"es-ES","ids":[es]}));
    assert_eq!(langs(&doc), vec![Some("en-US".into()), Some("es-ES".into()), Some("en-US".into())]);

    // Undo puts it back.
    assert!(doc.undo());
    assert_eq!(langs(&doc), vec![Some("en-US".into()); 3]);
}

#[test]
fn a_new_document_default_reaches_paragraphs_without_their_own() {
    let mut doc = doc_with("One\n\nDos\n\nThree");
    let dos = find_p(&doc, "Dos");
    ops(&mut doc, json!({"op":"set_language","lang":"es-ES","ids":[dos]}));
    doc.render_changed();
    ops(&mut doc, json!({"op":"set_language","lang":"fr-FR"}));
    // The paragraph with its own language keeps it; the others follow the new default.
    assert_eq!(langs(&doc), vec![Some("fr-FR".into()), Some("es-ES".into()), Some("fr-FR".into())]);
    // The editor only draws what changed: the change must reach every paragraph.
    let out = doc.render_changed();
    assert_eq!(out.blocks.iter().filter(|b| matches!(b, RBlock::Para(_))).count(), 3);
}

#[test]
fn the_first_run_with_text_decides_when_runs_differ() {
    let mut doc = doc_with("Hola mundo");
    let id = find_p(&doc, "Hola");
    let rpr = |lang: &str| {
        Some(xml::parse(format!(r#"<w:rPr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:lang w:val="{lang}"/></w:rPr>"#).as_bytes()).unwrap().root)
    };
    let run = |text: &str, lang: Option<&str>| Inline::Run(Run { rpr: lang.and_then(rpr), items: vec![RunItem::Text(text.into())], ..Default::default() });
    let set = |doc: &mut Document, inlines: Vec<Inline>| {
        doc.body.edit_paragraphs(&mut |p| {
            if p.id == id {
                p.inlines = inlines.clone();
                return true;
            }
            false
        });
    };
    // A run without text first (nothing to check in it), then Spanish, then English.
    set(&mut doc, vec![run("", Some("fr-FR")), run("Hola ", Some("es-ES")), run("world", Some("en-GB"))]);
    assert_eq!(langs(&doc), vec![Some("es-ES".into())]);
    // The first text run has none: the document default.
    set(&mut doc, vec![run("Hola ", None), run("world", Some("en-GB"))]);
    assert_eq!(langs(&doc), vec![Some("en-US".into())]);
}
