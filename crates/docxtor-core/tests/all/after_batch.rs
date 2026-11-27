//! The two single homes in the engine: `Op::class` (what kind of op each op is, with no catch-all) and `after_batch`
//! (what runs after every batch, for the body and for every story it touched). The invariant check (E14, E15) is on in
//! `cargo test`, so a placeholder left showing in a header would fail these tests by itself.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::stories::StoryKind;
use docxtor_core::{package::Package, Document, Op};
use serde_json::{json, Value};

fn op(v: Value) -> Op {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).err().map(|e| e.to_string()).unwrap_or_else(|| panic!("expected an error: {json}"))
}

fn doc_with(text: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":text}));
    doc
}

fn pos(p: u32) -> Value {
    json!({"p": p, "o": 0})
}

// ---------------------------------------------------------------------------
// The classifier
// ---------------------------------------------------------------------------

/// Flags as the four hand-kept lists and `is_view_only` gave them before the classifier: (main text only, edits
/// styles, changes page layout, leaves the body alone, view only). The classifier must agree on every op here.
struct Old(bool, bool, bool, bool, bool);

#[test]
fn the_classifier_gives_the_answers_the_old_lists_gave() {
    let none = Old(false, false, false, false, false);
    let table: Vec<(Value, Old)> = vec![
        (json!({"op":"insert_text","at":pos(1),"text":"x"}), none),
        (json!({"op":"delete","from":pos(1),"to":pos(1)}), Old(false, false, false, false, false)),
        (json!({"op":"set_theme","theme":"facet"}), Old(false, true, false, true, false)),
        (json!({"op":"set_style_set","style_set":"modern"}), Old(false, true, false, true, false)),
        (json!({"op":"set_paragraph_spacing","preset":"compact"}), Old(false, true, false, true, false)),
        (json!({"op":"set_default_format","format":{}}), Old(false, true, false, true, false)),
        (json!({"op":"delete_style","style":"Foo"}), Old(false, true, false, false, false)),
        (json!({"op":"set_page_setup"}), Old(false, false, true, false, false)),
        (json!({"op":"set_columns","count":2}), Old(false, false, true, false, false)),
        (json!({"op":"set_page_color","color":"ffffff"}), Old(false, false, true, false, false)),
        (json!({"op":"add_header_footer","kind":"header"}), Old(false, false, true, false, false)),
        (json!({"op":"remove_header_footer","kind":"footer"}), Old(false, false, true, false, false)),
        (json!({"op":"set_title_page","on":true}), Old(false, false, true, false, false)),
        (json!({"op":"set_watermark"}), Old(false, false, true, false, false)),
        (json!({"op":"set_doc_props","title":"T"}), Old(false, false, false, true, false)),
        (json!({"op":"set_custom_props"}), Old(false, false, false, true, false)),
        (json!({"op":"set_language","lang":"en-US"}), Old(false, false, false, true, false)),
        (json!({"op":"update_fields"}), Old(false, false, false, true, false)),
        (json!({"op":"update_fields","open":true}), Old(false, false, false, true, true)),
        // Updating before printing is view only: a read-only document updates it.
        (json!({"op":"update_fields","quiet":true}), Old(false, false, false, true, true)),
        (json!({"op":"insert_endnote","at":pos(1),"text":"x"}), Old(true, false, false, false, false)),
        (json!({"op":"insert_footnote","at":pos(1),"text":"x"}), Old(true, false, false, false, false)),
        (json!({"op":"insert_page_break","at":pos(1)}), Old(true, false, false, false, false)),
        (json!({"op":"insert_toc"}), Old(true, false, false, false, false)),
        (json!({"op":"protection","action":"stop"}), Old(false, false, false, true, false)),
        (json!({"op":"protection","action":"mark_final","on":true}), Old(false, false, false, true, false)),
        (json!({"op":"protection","action":"editable_range","from":pos(1),"to":pos(1)}), Old(false, false, false, false, false)),
        (json!({"op":"tab","action":"rename","id":"t1","name":"A"}), Old(false, false, false, true, false)),
        // Showing a tab is no op (view state of each pane): `Op` has no `open` action to classify.
        (json!({"op":"tab","action":"delete","id":"t1"}), Old(false, false, false, false, false)),
        (json!({"op":"toa","action":"set_category_names","names":[]}), Old(true, false, false, true, false)),
        (json!({"op":"control","action":"remove","target":{"p":1,"n":0}}), Old(false, false, false, false, false)),
        (json!({"op":"set_mail_merge"}), Old(false, false, false, true, false)),
    ];
    for (json, old) in table {
        let o = op(json.clone());
        let c = o.class();
        let got = (c.main_text_only, c.edits_styles, c.changes_page_layout, c.leaves_body_alone, c.view_only);
        assert_eq!(got, (old.0, old.1, old.2, old.3, old.4), "{json}");
        // The wrappers say the same as the classifier.
        assert_eq!((o.main_text_only(), o.edits_styles(), o.changes_page_layout(), o.leaves_body_alone(), o.is_view_only()), got, "{json}");
    }
}

#[test]
fn each_flag_has_ops_that_set_it_and_ops_that_do_not() {
    // main text only
    assert!(op(json!({"op":"insert_endnote","at":pos(1),"text":"x"})).class().main_text_only);
    assert!(!op(json!({"op":"insert_text","at":pos(1),"text":"x"})).class().main_text_only);
    // edits styles
    assert!(op(json!({"op":"set_theme","theme":"facet"})).class().edits_styles);
    assert!(!op(json!({"op":"set_page_setup"})).class().edits_styles);
    // changes page layout
    assert!(op(json!({"op":"set_columns","count":2})).class().changes_page_layout);
    assert!(!op(json!({"op":"set_theme","theme":"facet"})).class().changes_page_layout);
    // leaves the body alone
    assert!(op(json!({"op":"set_doc_props","title":"T"})).class().leaves_body_alone);
    assert!(!op(json!({"op":"insert_text","at":pos(1),"text":"x"})).class().leaves_body_alone);
    // view only
    assert!(op(json!({"op":"update_fields","open":true})).class().view_only);
    assert!(!op(json!({"op":"update_fields"})).class().view_only);
    // charts, whole document, within a paragraph, limited formatting
    assert!(op(json!({"op":"insert_text","at":pos(1),"text":"x"})).class().within_paragraph);
    assert!(!op(json!({"op":"split_paragraph","at":pos(1)})).class().within_paragraph);
    assert!(op(json!({"op":"remove_citation","at":pos(1)})).class().whole_document);
    assert!(op(json!({"op":"format_text","from":pos(1),"to":pos(1),"format":{"bold":true}})).class().limited_formatting);
    assert!(!op(json!({"op":"insert_text","at":pos(1),"text":"x"})).class().limited_formatting);
    assert!(op(json!({"op":"set_chart","at":pos(1)})).class().touches_charts);
}

/// Cover pages finding 2: inserting a header or footer design is a change of page layout, and a protected document
/// refuses it like every other header change.
#[test]
fn a_protected_document_refuses_a_header_design() {
    assert!(op(json!({"op":"insert_page_part","kind":"header","design":"docxtor.blank"})).class().changes_page_layout);
    assert!(op(json!({"op":"insert_cover_page","design":"docxtor.simple"})).class().changes_page_layout);
    assert!(op(json!({"op":"remove_page_numbers"})).class().changes_page_layout);

    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.blank"}));
    let header_before = doc.stories.iter().find(|s| s.kind == StoryKind::Header).map(|s| format!("{:?}", s.blocks.len()));
    ops(&mut doc, json!({"op":"protection","action":"set","settings":{"edit":"readOnly","enforce":true}}));
    for kind in ["header", "footer"] {
        let e = err(&mut doc, json!({"op":"insert_page_part","kind":kind,"design":"docxtor.three"}));
        assert!(e.contains("protected"), "{e}");
    }
    let e = err(&mut doc, json!({"op":"insert_page_number","place":"bottom"}));
    assert!(e.contains("protected"), "{e}");
    assert_eq!(doc.stories.iter().find(|s| s.kind == StoryKind::Header).map(|s| format!("{:?}", s.blocks.len())), header_before);
}

// ---------------------------------------------------------------------------
// After the batch: stories
// ---------------------------------------------------------------------------

fn story_first_para(doc: &Document, kind: StoryKind) -> u32 {
    let s = doc.stories.iter().find(|s| s.kind == kind).expect("a story of that kind");
    let mut first = None;
    walk_paragraphs(&s.blocks, &mut |p| {
        first.get_or_insert(p.id);
    });
    first.expect("a paragraph")
}

fn story_text(doc: &Document, kind: StoryKind) -> String {
    let s = doc.stories.iter().find(|s| s.kind == kind).expect("a story of that kind");
    let mut out = String::new();
    walk_paragraphs(&s.blocks, &mut |p| out.push_str(&p.plain_text()));
    out
}

fn saved_part(doc: &mut Document, prefix: &str) -> String {
    let pkg = Package::open(&doc.save().unwrap()).unwrap();
    pkg.parts.iter().filter(|p| p.name.starts_with(prefix)).map(|p| String::from_utf8(p.data.clone()).unwrap()).collect()
}

#[test]
fn typing_in_a_header_box_clears_its_placeholder() {
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.blank"}));
    let p = story_first_para(&doc, StoryKind::Header);
    assert_eq!(story_text(&doc, StoryKind::Header), "[Type here]");
    assert!(saved_part(&mut doc, "word/header").contains("w:showingPlcHdr"));

    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Quarterly report"}));
    assert_eq!(story_text(&doc, StoryKind::Header), "Quarterly report");
    let xml = saved_part(&mut doc, "word/header");
    assert!(!xml.contains("w:showingPlcHdr"), "{xml}");
    assert!(!xml.contains("PlaceholderText\"/></w:rPr><w:t>Quarterly"), "{xml}");
    assert!(xml.contains("Quarterly report"));

    // Emptying the box brings the placeholder back.
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":16}}));
    assert_eq!(story_text(&doc, StoryKind::Header), "[Type here]");
    assert!(saved_part(&mut doc, "word/header").contains("w:showingPlcHdr"));

    // Undo gives back what was typed.
    assert!(doc.undo());
    assert_eq!(story_text(&doc, StoryKind::Header), "Quarterly report");
}

#[test]
fn the_placeholder_style_is_defined_wherever_a_design_writes_a_placeholder() {
    let mut doc = doc_with("Body");
    assert!(!saved_part(&mut doc, "word/styles").contains("w:styleId=\"PlaceholderText\""));
    ops(&mut doc, json!({"op":"insert_page_part","kind":"footer","design":"docxtor.authorPage"}));
    assert!(saved_part(&mut doc, "word/styles").contains("w:styleId=\"PlaceholderText\""));
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    assert!(saved_part(&mut doc, "word/styles").contains("w:styleId=\"PlaceholderText\""));
    // A control the file did not have a style for gets it too.
    let mut doc = doc_with("Body");
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":first,"o":0},"kind":"text"}));
    assert!(saved_part(&mut doc, "word/styles").contains("w:styleId=\"PlaceholderText\""));
}

#[test]
fn typing_in_a_headers_title_box_changes_the_title_and_survives_a_sync() {
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"set_doc_props","title":"Field notes"}));
    ops(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.title"}));
    let p = story_first_para(&doc, StoryKind::Header);
    assert_eq!(story_text(&doc, StoryKind::Header).to_lowercase(), "field notes");

    // Type in the middle of what the box shows.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":5},"text":" 2"}));
    assert_eq!(doc.doc_props().title.as_deref(), Some("Field 2 notes"));

    // Something that syncs the bound controls (a second header design) puts the property back in the boxes, not the old
    // title over the new one.
    ops(&mut doc, json!({"op":"insert_page_part","kind":"footer","design":"docxtor.banded"}));
    assert_eq!(doc.doc_props().title.as_deref(), Some("Field 2 notes"));
    assert!(saved_part(&mut doc, "word/footer").to_lowercase().contains("field 2 notes") || saved_part(&mut doc, "word/footer").contains("Field 2 notes"));

    // Undo gives back the old title, and the header shows it.
    assert!(doc.undo()); // the footer
    assert!(doc.undo()); // the typing
    assert_eq!(doc.doc_props().title.as_deref(), Some("Field notes"));
    assert_eq!(story_text(&doc, StoryKind::Header).to_lowercase(), "field notes");
    // And redo brings the typed title back.
    assert!(doc.redo());
    assert_eq!(doc.doc_props().title.as_deref(), Some("Field 2 notes"));
}

#[test]
fn typing_in_a_footnote_control_clears_its_placeholder() {
    let mut doc = doc_with("Body text");
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":first,"o":4},"text":"See"}));
    let p = story_first_para(&doc, StoryKind::Footnote);
    // A text control at the end of the note (the note's mark counts as one character before its text).
    let end = story_text(&doc, StoryKind::Footnote).chars().count() as u32 + 1;
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":end},"kind":"text"}));
    let note = story_text(&doc, StoryKind::Footnote);
    assert!(note.contains("Click or tap"), "{note}");
    assert!(saved_part(&mut doc, "word/footnotes").contains("w:showingPlcHdr"));

    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":end + 1},"text":"the appendix"}));
    let xml = saved_part(&mut doc, "word/footnotes");
    assert!(!xml.contains("w:showingPlcHdr"), "{xml}");
    assert!(story_text(&doc, StoryKind::Footnote).contains("the appendix"));
    assert!(!story_text(&doc, StoryKind::Footnote).contains("Click or tap"));
}

#[test]
fn a_locked_control_in_a_header_cannot_be_edited() {
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.blank"}));
    let p = story_first_para(&doc, StoryKind::Header);
    ops(&mut doc, json!({"op":"control","action":"props","target":{"p":p,"n":0},"props":{"lock":"contentLocked"}}));
    let e = err(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"x"}));
    assert!(e.contains("locked"), "{e}");
}
