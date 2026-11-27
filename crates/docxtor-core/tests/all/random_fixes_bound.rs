//! Found by the random op harness: a box bound to a document
//! property shows exactly what the property holds, whichever way the value got there, and a saved and reopened
//! document looks the same. The invariant check (E15) is on in `cargo test`, so a box that says it shows its
//! placeholder while holding text fails these tests by itself.

use crate::common;

use common::*;
use docxtor_core::Document;
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn run(doc: &mut Document, v: Value) {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn same_after_reopen(doc: &mut Document) {
    let bytes = doc.save().unwrap();
    let again = Document::open(&bytes).unwrap();
    let (x, y) = (normalise(&render_model(doc)), normalise(&render_model(&again)));
    if let Some(d) = first_diff(&x, &y) {
        panic!("open(save(doc)) renders differently: {d}");
    }
}

/// A title set in the details box flows into a Title box in a gallery header, whatever the title holds. Every case
/// runs; the ones that fail are listed together.
#[test]
fn a_title_box_in_a_header_follows_the_title_without_showing_its_placeholder() {
    let mut failed = Vec::new();
    for fixture in ["simple.docx", "rich.docx"] {
        for before in [None, Some("A title")] {
            for title in [" ", "line\nbreak", "a\tb", "", "plain"] {
                let case = format!("{fixture}, title {before:?} then {title:?}");
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let mut doc = Start::File(fixture.to_string()).open();
                    if let Some(b) = before {
                        run(&mut doc, json!({"op":"set_doc_props","title":b}));
                    }
                    run(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.title"}));
                    run(&mut doc, json!({"op":"set_doc_props","author":"someone"}));
                    run(&mut doc, json!({"op":"set_doc_props","title":title}));
                    same_after_reopen(&mut doc);
                }));
                if result.is_err() {
                    failed.push(case);
                }
            }
        }
    }
    assert!(failed.is_empty(), "these cases broke: {failed:#?}");
}

/// L4: a page break typed into a placeholder of a cover page box that is bound to the company is the box's text now;
/// the company keeps it exactly, so the box shows the same after saving and opening again.
#[test]
fn a_break_inside_a_company_box_on_the_cover_survives_save_and_reopen() {
    let spanish = json!({"company":"Empresa","title":"Título","lang":"es-ES"});
    let mut applied = 0;
    let mut failed = Vec::new();
    for labels in [spanish, json!({})] {
        for design in ["docxtor.academic", "docxtor.simple"] {
            let mut doc = Start::File("simple.docx".to_string()).open();
            run(&mut doc, json!({"op":"insert_cover_page","design":design,"labels":labels}));
            let order = doc.render_all().order.clone();
            for p in order.into_iter().take(8) {
                for o in [0, 1, 3] {
                    let mut copy = Document::open(&doc.save().unwrap()).unwrap();
                    let op = json!({"op":"insert_break","at":{"p":p,"o":o},"kind":"page"});
                    if copy.apply_json(&op.to_string(), false).is_ok() {
                        applied += 1;
                        if catch_unwind(AssertUnwindSafe(|| same_after_reopen(&mut copy))).is_err() {
                            failed.push(format!("{design} {labels} break at {p}:{o}"));
                        }
                    }
                }
            }
        }
    }
    assert!(applied > 10, "only {applied} breaks went in");
    assert!(failed.is_empty(), "these cases broke: {failed:#?}");
}

fn para_with(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

fn instrs(doc: &Document, p: u32) -> Vec<String> {
    let mut out = Vec::new();
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |para| {
        if para.id == p {
            docxtor_core::fields::scan_inlines(&para.inlines, 0, &mut |s| out.push(s.instr.trim().to_string()));
        }
    });
    out
}

/// Found by the random search (E15): a TITLE field put inside the cover page's Subtitle box while it
/// showed its placeholder (the title empty, so the field showed nothing), then a title set and the fields updated. The
/// update is an op that leaves the body alone by its kind, so the steps after the batch skipped the body and the box
/// kept saying it showed its placeholder. Now it ends the placeholder, keeps the field, and keeps what was typed.
#[test]
fn a_field_updated_inside_a_placeholder_ends_the_placeholder_and_stays_a_field() {
    let mut doc = Start::File("simple.docx".to_string()).open();
    run(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    let p = para_with(&doc, "[Document subtitle]");
    let text = |doc: &Document| doc.list_controls().into_iter().find(|(_, c, _)| c.alias.as_deref() == Some("Subtitle")).map(|(_, c, t)| (c.placeholder, t)).expect("subtitle");
    // Where the box starts in its paragraph: the field goes three characters in.
    let at = doc.list_controls().into_iter().find(|(_, c, _)| c.alias.as_deref() == Some("Subtitle")).map(|(_, c, _)| c.start).unwrap();
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":at + 3},"field":"TITLE"}));
    assert_eq!(text(&doc), (true, "[Document subtitle]".to_string()), "an empty field changes nothing yet");
    run(&mut doc, json!({"op":"set_doc_props","title":"Moby Dick"}));
    run(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(text(&doc), (false, "Moby Dick".to_string()));
    assert!(instrs(&doc, p).iter().any(|i| i == "TITLE"), "the field is still there: {:?}", instrs(&doc, p));
    same_after_reopen(&mut doc);
    // Undo gives the placeholder back, with the field showing nothing.
    assert!(doc.undo());
    assert_eq!(text(&doc), (true, "[Document subtitle]".to_string()));
}

/// The same in a header: a field updated in place in a story ends a placeholder there too. The author is empty when
/// the field goes in, so the box still shows its placeholder until the fields are updated.
#[test]
fn a_field_updated_inside_a_placeholder_in_a_header_ends_the_placeholder() {
    let mut doc = Start::File("simple.docx".to_string()).open();
    run(&mut doc, json!({"op":"set_doc_props","author":""}));
    run(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.title"}));
    // The header box that shows its placeholder: its paragraph, where it starts, whether it shows it, and its text.
    let boxed = |doc: &Document| -> (u32, usize, bool, String) {
        let mut found = None;
        for s in &doc.stories {
            docxtor_core::model::walk_paragraphs(&s.blocks, &mut |p| {
                for (c, start, _) in docxtor_core::controls::sdts_in(&p.inlines) {
                    let v = c.shell.child("w:sdtPr").map(docxtor_core::controls::read_props).unwrap_or_default();
                    if found.is_none() && v.kind == "text" && v.alias.as_deref() == Some("Title") {
                        found = Some((p.id, start, v.placeholder, docxtor_core::controls::inline_text(&c.children)));
                    }
                }
            });
        }
        found.expect("a Title box in a header")
    };
    let (p, start, showing, text) = boxed(&doc);
    assert!(showing, "the new box shows its placeholder: {text:?}");
    // A field in a box bound to the title would be the title twice; an AUTHOR field shows the author.
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":start + 3},"field":"AUTHOR"}));
    let (_, _, showing, after_insert) = boxed(&doc);
    assert!(showing, "an empty field changes nothing yet: {after_insert:?}");
    run(&mut doc, json!({"op":"set_doc_props","author":"Herman"}));
    run(&mut doc, json!({"op":"update_fields"}));
    let (_, _, showing, text) = boxed(&doc);
    assert_eq!((showing, text.as_str()), (false, "Herman"));
    same_after_reopen(&mut doc);
}

/// Deleting text next to a field that shows nothing (an AUTHOR field before there is an author) keeps the field.
#[test]
fn deleting_text_keeps_a_field_that_shows_nothing() {
    let mut doc = Start::File("simple.docx".to_string()).open();
    run(&mut doc, json!({"op":"set_doc_props","author":""}));
    let p = para_with(&doc, "");
    run(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"abcd"}));
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":2},"field":"AUTHOR"}));
    assert_eq!(instrs(&doc, p), vec!["AUTHOR".to_string()]);
    run(&mut doc, json!({"op":"delete","from":{"p":p,"o":3},"to":{"p":p,"o":4}}));
    assert_eq!(instrs(&doc, p), vec!["AUTHOR".to_string()], "a delete after the field keeps it");
    run(&mut doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":1}}));
    assert_eq!(instrs(&doc, p), vec!["AUTHOR".to_string()], "a delete before the field keeps it");
}

/// Found by a run of every real file (LibreOffice's tdf148361.docx): the file's relationships name a glossary (and
/// headers and footers) that the package does not hold. A design with a placeholder box put its placeholder into that
/// missing glossary, so the box's text was looked up in nothing and the box said it showed its placeholder while
/// holding other text (E15). The glossary is now made where the file points.
#[test]
fn a_placeholder_box_in_a_file_that_points_at_a_missing_glossary_keeps_its_placeholder() {
    use docxtor_core::package::Package;
    let mut pkg = Package::open(&std::fs::read(repo_path("fixtures/simple.docx")).unwrap()).unwrap();
    let main = pkg.main_document_path();
    pkg.add_rel(&main, docxtor_core::controls::REL_GLOSSARY, "glossary/document.xml", false);
    assert!(!pkg.has("word/glossary/document.xml"));
    let bytes = pkg.to_bytes().unwrap();
    for op in [json!({"op":"insert_page_part","kind":"footer","design":"docxtor.blank"}), json!({"op":"insert_cover_page","design":"docxtor.simple"})] {
        let mut doc = Document::open(&bytes).unwrap();
        run(&mut doc, op.clone());
        let saved = doc.save().unwrap();
        let again = Package::open(&saved).unwrap();
        assert!(again.has("word/glossary/document.xml"), "{op}: the glossary is made where the file points");
        assert!(again.has("word/glossary/styles.xml"), "{op}");
        same_after_reopen(&mut doc);
    }
}
