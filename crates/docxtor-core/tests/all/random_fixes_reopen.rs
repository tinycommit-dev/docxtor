//! Found by the random op harness: a document must render the
//! same after it is saved and opened again as it did before.

use crate::common;

use common::*;
use docxtor_core::Document;
use serde_json::{json, Value};

fn run(doc: &mut Document, v: Value) {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

/// The render model before saving and after a save and reopen must be the same (ids and part names renumbered).
fn same_after_reopen(doc: &mut Document) {
    let bytes = doc.save().unwrap();
    let again = Document::open(&bytes).unwrap();
    let (x, y) = (normalise(&render_model(doc)), normalise(&render_model(&again)));
    if let Some(d) = first_diff(&x, &y) {
        panic!("open(save(doc)) renders differently: {d}");
    }
}

fn open_fixture(name: &str) -> Document {
    Start::File(name.to_string()).open()
}

fn first_para(doc: &mut Document) -> u32 {
    doc.render_all().order[0]
}

#[test]
fn a_caption_with_a_tab_or_a_line_break_survives_save_and_reopen() {
    for text in ["a\tb", "line\nbreak"] {
        let mut doc = build("plain");
        let after = first_para(&mut doc);
        run(&mut doc, json!({"op":"insert_caption","after":after,"label":"Figure","text":text}));
        same_after_reopen(&mut doc);
    }
}

#[test]
fn a_title_with_a_tab_in_the_cover_page_survives_save_and_reopen() {
    for text in ["a\tb", "x\ny"] {
        let mut doc = build("plain");
        run(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
        run(&mut doc, json!({"op":"set_doc_props","title":text}));
        same_after_reopen(&mut doc);
    }
}

#[test]
fn a_cover_page_made_after_the_properties_were_set_shows_a_tab_in_them_as_a_tab() {
    let mut doc = build("rich");
    run(&mut doc, json!({"op":"set_doc_props","author":"tab\there","keywords":"x","title":"line\nbreak"}));
    run(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    same_after_reopen(&mut doc);
}

#[test]
fn a_space_typed_in_an_empty_bound_title_box_survives_save_and_reopen() {
    let mut doc = build("rich");
    run(&mut doc, json!({"op":"insert_text","at":{"p":20,"o":14},"text":" "}));
    same_after_reopen(&mut doc);
}

#[test]
fn text_typed_with_spaces_around_it_in_a_bound_title_box_keeps_them_after_reopen() {
    // The bound property holds exactly what the box shows (Word does the same), so reopening shows the same text.
    for typed in [" ", "Hi ", " Hi", "a  b"] {
        let mut doc = build("rich");
        run(&mut doc, json!({"op":"insert_text","at":{"p":20,"o":14},"text":typed}));
        same_after_reopen(&mut doc);
    }
}

#[test]
fn a_tab_in_the_text_of_a_footer_page_number_label_is_a_tab_element() {
    let mut doc = build("plain");
    run(&mut doc, json!({"op":"insert_page_number","place":"bottom","style":"x_of_y","page_word":"P\tg","of_word":"o\nf"}));
    same_after_reopen(&mut doc);
}

#[test]
fn a_table_put_last_in_a_cell_gets_the_paragraph_the_file_needs() {
    let mut doc = open_fixture("rich.docx");
    run(&mut doc, json!({"op":"insert_table","after":26,"rows":1,"cols":1,"data":[["x"]]}));
    same_after_reopen(&mut doc);
}

#[test]
fn a_text_box_added_after_a_reopen_shows_its_text_before_saving() {
    for text in ["back\\slash \\\\ \n \t", "plain", "two\nlines", "a\tb"] {
        let mut doc = build("rich");
        let mut doc = Document::open(&doc.save().unwrap()).unwrap();
        run(&mut doc, json!({"op":"add_header_footer","kind":"footer","variant":"default"}));
        run(&mut doc, json!({"op":"insert_text_box","at":{"p":8,"o":19},"floating":true,"h":40,"w":100,"x":10,"y":10,"text":text}));
        run(&mut doc, json!({"op":"tab","action":"duplicate","id":"t1"}));
        same_after_reopen(&mut doc);
    }
}

#[test]
fn a_page_break_next_to_an_empty_control_is_a_break_before_and_after_saving() {
    let mut doc = open_fixture("simple.docx");
    run(
        &mut doc,
        json!({"op":"toa","action":"insert","options":{"category":0,"headings":true,"keepFormatting":false,"leader":"dot","passim":true,"rightAlign":true},"pages":{"3":1,"4":2,"5":3,"6":4,"7":5}}),
    );
    run(
        &mut doc,
        json!([{"op":"control","action":"insert","at":{"p":3,"o":4},"kind":"text"},{"op":"insert_break","at":{"p":3,"o":17},"kind":"page"}]),
    );
    same_after_reopen(&mut doc);
}

#[test]
fn a_bibliography_after_a_paragraph_in_a_table_cell_has_the_same_blocks_after_saving() {
    let mut doc = open_fixture("rich.docx");
    run(&mut doc, json!({"op":"insert_bibliography","after":26,"heading":""}));
    same_after_reopen(&mut doc);
}
