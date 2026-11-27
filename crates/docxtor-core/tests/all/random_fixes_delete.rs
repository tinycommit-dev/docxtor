//! Fixes for what the random edit harness found around deleting: a range that holds a content control and a field,
//! a batch that adds a tab and removes its start, and a bookmark that loses one end when a block goes.

use docxtor_core::model::{walk_paragraphs, Inline};
use docxtor_core::Document;
use serde_json::{json, Value};

fn fixture(name: &str) -> Document {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    Document::open(&std::fs::read(path).unwrap()).unwrap()
}

fn run(doc: &mut Document, v: Value) {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn first_para(doc: &Document) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() {
            id = Some(p.id);
        }
    });
    id.unwrap()
}

fn para_len(doc: &Document, id: u32) -> usize {
    let mut n = 0;
    walk_paragraphs(&doc.body, &mut |p| {
        if p.id == id {
            n = p.inlines.iter().map(|i| i.len16()).sum();
        }
    });
    n
}

fn text_of(doc: &Document, id: u32) -> String {
    let mut t = String::new();
    walk_paragraphs(&doc.body, &mut |p| {
        if p.id == id {
            t = p.plain_text();
        }
    });
    t
}

fn inlines_of(doc: &Document, id: u32) -> Vec<Inline> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| {
        if p.id == id {
            v = p.inlines.clone();
        }
    });
    v
}

/// What the engine's own check says is wrong with the document, as plain messages.
fn problems(doc: &Document) -> Vec<String> {
    doc.check_invariants().into_iter().map(|v| format!("{}: {}", v.rule, v.message)).collect()
}

fn assert_clean(doc: &mut Document) {
    let p = problems(doc);
    assert!(p.is_empty(), "the document is not valid: {p:?}");
    let bytes = doc.save().unwrap();
    Document::open(&bytes).unwrap();
}

fn blank_with_text(text: &str) -> (Document, u32) {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    run(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":text}));
    (doc, p)
}

fn delete_all(doc: &mut Document, p: u32) {
    let end = para_len(doc, p);
    run(doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":end}}));
}

// ---- Finding 1: a control and a field in one deleted range ----------------------------------------------------------

#[test]
fn deleting_a_range_with_a_control_and_then_a_field_does_not_panic() {
    let (mut doc, p) = blank_with_text("Hello world");
    run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":2},"kind":"text"}));
    let end = para_len(&doc, p);
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":end},"field":"PAGE"}));
    delete_all(&mut doc, p);
    assert_eq!(text_of(&doc, p), "");
    assert!(docxtor_core::controls::sdts_in(&inlines_of(&doc, p)).is_empty(), "the control went with the range");
    assert_clean(&mut doc);
}

#[test]
fn deleting_a_range_with_a_field_and_then_a_control_does_not_panic() {
    let (mut doc, p) = blank_with_text("Hello world");
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":2},"field":"PAGE"}));
    let end = para_len(&doc, p);
    run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":end},"kind":"text"}));
    delete_all(&mut doc, p);
    assert_eq!(text_of(&doc, p), "");
    assert_clean(&mut doc);
}

#[test]
fn deleting_several_controls_and_fields_in_turn_does_not_panic() {
    let (mut doc, p) = blank_with_text("Hello world");
    for at in [1, 4, 7] {
        run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":at},"kind":"text"}));
        let end = para_len(&doc, p);
        run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":end},"field":"PAGE"}));
    }
    delete_all(&mut doc, p);
    assert_eq!(text_of(&doc, p), "");
    assert_clean(&mut doc);
}

#[test]
fn deleting_part_of_the_text_around_a_control_and_a_field_keeps_the_rest() {
    let (mut doc, p) = blank_with_text("Hello world");
    run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":5},"kind":"text"}));
    let end = para_len(&doc, p);
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":end},"field":"PAGE"}));
    // The field is the last thing; delete from the start to just before it.
    run(&mut doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":end}}));
    assert_clean(&mut doc);
    assert!(!text_of(&doc, p).contains("Hello"), "{}", text_of(&doc, p));
}

#[test]
fn a_control_inside_another_and_a_field_after_them_delete_together() {
    let (mut doc, p) = blank_with_text("Hello world");
    run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":2},"kind":"text"}));
    run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":3},"kind":"text"}));
    let end = para_len(&doc, p);
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":end},"field":"PAGE"}));
    delete_all(&mut doc, p);
    assert_eq!(text_of(&doc, p), "");
    assert_clean(&mut doc);
}

#[test]
fn a_bookmark_between_a_control_and_a_field_does_not_confuse_the_delete() {
    let (mut doc, p) = blank_with_text("Hello world");
    run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":2},"kind":"text"}));
    run(&mut doc, json!({"op":"insert_bookmark","from":{"p":p,"o":3},"to":{"p":p,"o":6},"name":"Mid"}));
    let end = para_len(&doc, p);
    run(&mut doc, json!({"op":"insert_field","at":{"p":p,"o":end},"field":"PAGE"}));
    delete_all(&mut doc, p);
    assert_eq!(text_of(&doc, p), "");
    assert_clean(&mut doc);
}

// ---- Finding 6: add a tab and delete its start in one batch -----------------------------------------------------------

#[test]
fn adding_a_tab_and_deleting_the_first_block_in_one_batch_is_refused() {
    let mut doc = fixture("simple.docx");
    let blocks = doc.body.len();
    let res = doc.apply_json(&json!([{"op":"tab","action":"add","name":"Target"},{"op":"delete_blocks","ids":[1]}]).to_string(), false);
    match res {
        Err(_) => {
            // Refused: the document is as it was.
            assert!(doc.tabs_info().tabs.is_empty());
            assert_eq!(doc.body.len(), blocks, "nothing changed");
        }
        Ok(_) => assert!(problems(&doc).iter().all(|m| !m.starts_with("E13")), "accepted but the tabs are broken: {:?}", problems(&doc)),
    }
    assert!(problems(&doc).iter().all(|m| !m.starts_with("E13")), "{:?}", problems(&doc));
}

#[test]
fn deleting_the_first_block_of_a_tab_that_already_exists_keeps_the_tabs_valid() {
    let mut doc = fixture("simple.docx");
    run(&mut doc, json!({"op":"tab","action":"add","name":"Target"}));
    let start = doc.tabs_info().tabs[1].start;
    // Tabs 11 is unchanged: the batch is refused (with the message about joining tabs), the tabs stay whole.
    let err = doc.apply_json(&json!([{"op":"delete_blocks","ids":[start]}]).to_string(), false).expect_err("refused");
    assert!(err.to_string().contains("Delete the tab from the tab list"), "{err}");
    assert_eq!(doc.tabs_info().tabs.len(), 2);
    assert!(problems(&doc).iter().all(|m| !m.starts_with("E13")), "{:?}", problems(&doc));
}

// ---- Finding 8: a bookmark that spans paragraphs loses one end ----------------------------------------------------------

fn bookmark_names(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| {
        for i in &p.inlines {
            if let Inline::Opaque(e) = i {
                if e.name == "w:bookmarkStart" {
                    v.push(e.attr("w:name").unwrap_or_default().to_string());
                }
            }
        }
    });
    v
}

fn spanning_bookmark(doc: &mut Document) {
    run(doc, json!({"op":"insert_bookmark","from":{"p":2,"o":23},"to":{"p":5,"o":24},"name":"Alpha"}));
}

fn no_half_bookmarks(doc: &Document) {
    let bad: Vec<String> = problems(doc).into_iter().filter(|m| m.contains("bookmark start (id") && m.contains("has no end") || m.contains("has no start")).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn deleting_the_paragraph_that_holds_a_bookmark_end_leaves_no_half_bookmark() {
    let mut doc = fixture("complex.docx");
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"delete_blocks","ids":[5]}));
    no_half_bookmarks(&doc);
}

#[test]
fn deleting_the_paragraph_that_holds_a_bookmark_start_leaves_no_half_bookmark() {
    let mut doc = fixture("complex.docx");
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"delete_blocks","ids":[2]}));
    no_half_bookmarks(&doc);
}

#[test]
fn deleting_a_range_across_paragraphs_that_holds_one_end_leaves_no_half_bookmark() {
    let mut doc = fixture("complex.docx");
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"delete","from":{"p":2,"o":10},"to":{"p":5,"o":3}}));
    no_half_bookmarks(&doc);
    let mut doc = fixture("complex.docx");
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"delete","from":{"p":4,"o":0},"to":{"p":5,"o":30}}));
    no_half_bookmarks(&doc);
}

#[test]
fn a_link_to_a_bookmark_that_lost_an_end_still_finds_it() {
    // The bookmark is kept (shrunk), so the link does not end up pointing at nothing.
    let mut doc = fixture("complex.docx");
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"set_link","from":{"p":6,"o":2},"to":{"p":6,"o":4},"url":"#Alpha"}));
    run(&mut doc, json!({"op":"delete_blocks","ids":[5]}));
    no_half_bookmarks(&doc);
}

#[test]
fn a_range_that_swallows_a_middle_paragraph_holding_one_end_leaves_no_half_bookmark() {
    let mut doc = fixture("complex.docx");
    // The bookmark runs from paragraph 4 to paragraph 6; the delete takes in all of 5 and the end of 4.
    run(&mut doc, json!({"op":"insert_bookmark","from":{"p":4,"o":3},"to":{"p":5,"o":10},"name":"Alpha"}));
    run(&mut doc, json!({"op":"insert_bookmark","from":{"p":5,"o":2},"to":{"p":6,"o":4},"name":"Beta"}));
    run(&mut doc, json!({"op":"delete","from":{"p":4,"o":20},"to":{"p":6,"o":1}}));
    no_half_bookmarks(&doc);
}

#[test]
fn a_bookmark_whose_text_is_only_partly_deleted_is_kept_whole() {
    let mut doc = fixture("complex.docx");
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"delete","from":{"p":4,"o":0},"to":{"p":4,"o":2}}));
    no_half_bookmarks(&doc);
    assert!(bookmark_names(&doc).contains(&"Alpha".to_string()));
}

#[test]
fn deleting_every_paragraph_between_leaves_the_bookmark_whole() {
    let mut doc = fixture("complex.docx");
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"delete_blocks","ids":[4]}));
    no_half_bookmarks(&doc);
}

#[test]
fn deleting_a_table_that_holds_one_end_of_a_bookmark_leaves_no_half_bookmark() {
    let mut doc = fixture("rich.docx");
    let first = first_para(&doc);
    let table = doc.body.iter().find(|b| matches!(b, docxtor_core::model::Block::Table(_))).map(|b| b.id());
    let table = table.expect("complex.docx has a table");
    let mut inside = None;
    if let Some(docxtor_core::model::Block::Table(t)) = doc.body.iter().find(|b| b.id() == table) {
        for r in t.rows() {
            for c in r.cells() {
                walk_paragraphs(&c.blocks, &mut |p| {
                    if inside.is_none() {
                        inside = Some(p.id);
                    }
                });
            }
        }
    }
    let inside = inside.expect("the table has a paragraph");
    run(&mut doc, json!({"op":"insert_bookmark","from":{"p":first,"o":0},"to":{"p":inside,"o":0},"name":"Alpha"}));
    run(&mut doc, json!({"op":"delete_blocks","ids":[table]}));
    no_half_bookmarks(&doc);
}

#[test]
fn joining_two_paragraphs_keeps_a_bookmark_that_spans_them_whole() {
    let mut doc = fixture("rich.docx");
    run(&mut doc, json!({"op":"insert_bookmark","from":{"p":4,"o":5},"to":{"p":5,"o":5},"name":"Alpha"}));
    run(&mut doc, json!({"op":"delete","from":{"p":4,"o":42},"to":{"p":5,"o":0}}));
    no_half_bookmarks(&doc);
}

#[test]
fn a_bookmark_that_was_already_half_a_bookmark_in_the_file_is_left_alone() {
    let mut doc = fixture("rich.docx");
    doc.body.edit_paragraphs(&mut |p| {
        if p.id == 7 {
            let e = docxtor_core::xml::Element::new("w:bookmarkStart").with_attr("w:id", "900").with_attr("w:name", "Lonely");
            p.inlines.insert(0, Inline::Opaque(e));
            return true;
        }
        false
    });
    assert!(problems(&doc).iter().any(|m| m.contains("bookmark start (id 900) has no end")), "{:?}", problems(&doc));
    run(&mut doc, json!({"op":"delete_blocks","ids":[6]}));
    assert!(problems(&doc).iter().any(|m| m.contains("bookmark start (id 900) has no end")), "the half that was there before is still there");
}

#[test]
fn typing_into_a_placeholder_keeps_a_bookmark_end_that_sits_inside_the_control() {
    let (mut doc, p) = blank_with_text("Hello world");
    run(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":2},"kind":"text"}));
    // The bookmark starts at the front and ends inside the control's placeholder.
    run(&mut doc, json!({"op":"insert_bookmark","from":{"p":p,"o":0},"to":{"p":p,"o":6},"name":"Inside"}));
    run(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":4},"text":"typed"}));
    assert!(bookmark_names(&doc).contains(&"Inside".to_string()));
    no_half_bookmarks(&doc);
}

#[test]
fn rebuilding_a_table_of_authorities_after_a_spanning_bookmark_leaves_no_half_bookmark() {
    let mut doc = fixture("complex.docx");
    run(&mut doc, json!({"op":"toa","action":"set_category_names","names":[{"category":8,"name":"Treaties"}]}));
    spanning_bookmark(&mut doc);
    run(&mut doc, json!({"op":"update_fields"}));
    no_half_bookmarks(&doc);
}

/// Found by the random search (E14): one batch sets a bookmark from the cover page into the text and
/// removes the cover page. The end lost its start, and the start was made in the same batch, so the copy of the body
/// before the batch had nothing to put back: the end stayed alone. A bookmark the batch made and cut again goes whole.
#[test]
fn a_bookmark_made_and_cut_in_one_batch_leaves_no_end_behind() {
    let mut doc = fixture("simple.docx");
    run(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.centered"}));
    let cover_para = first_para(&doc);
    let body_para = doc.body.iter().find_map(|b| match b {
        docxtor_core::model::Block::Para(p) if !p.plain_text().is_empty() => Some(p.id),
        _ => None,
    });
    let body_para = body_para.expect("a paragraph of text after the cover");
    run(&mut doc, json!([{"op":"insert_bookmark","name":"Top","from":{"p":cover_para,"o":0},"to":{"p":body_para,"o":0}},{"op":"remove_cover_page"}]));
    assert!(doc.bookmarks().iter().all(|b| b.name != "Top"), "no half of the bookmark is left");
    let saved = String::from_utf8(docxtor_core::package::Package::open(&doc.save().unwrap()).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    let (starts, ends) = (saved.matches("<w:bookmarkStart").count(), saved.matches("<w:bookmarkEnd").count());
    assert_eq!(starts, ends, "every bookmark end has its start");
}
