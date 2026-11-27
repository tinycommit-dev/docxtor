//! The main part is kept as pieces of whole blocks after a save (`document/body_xml.rs`, `Package::set_pieces`): the pieces
//! joined are always the whole body written again, a piece nothing changed in is the very same piece, and nothing on a save,
//! an edit or at idle joins the part into one buffer.

use crate::common::repo_path;
use docxtor_core::{package::Piece, Document};
use std::sync::Arc;

/// Small, so a 42 KB document is a dozen pieces and edits cross their boundaries.
const LIMIT: usize = 4096;

/// The document, edited once and saved, so its main part is written (an untouched document is saved as it was opened).
fn open() -> Document {
    docxtor_core::document::set_piece_limit(LIMIT);
    let mut d = Document::open(&std::fs::read(repo_path("fixtures/long.docx")).unwrap()).unwrap();
    let first = d.render_all().order[0];
    insert(&mut d, first, "start ");
    d.save().unwrap();
    d
}

fn pieces(d: &Document) -> Vec<Piece> {
    d.pkg.pieces_of(&d.main_path).expect("the main part is kept as pieces").to_vec()
}

/// The pieces, after checking they join to the whole body written again.
fn check(d: &Document) -> Vec<Piece> {
    let ps = pieces(d);
    let joined: Vec<u8> = ps.iter().flat_map(|p| p.iter().copied()).collect();
    assert!(joined == d.document_xml_whole(), "the pieces joined are not the whole body");
    ps
}

fn order(d: &mut Document) -> Vec<u32> {
    d.render_all().order.clone()
}

fn run(d: &mut Document, json: serde_json::Value) {
    d.apply_json(&json.to_string(), false).unwrap();
}

fn insert(d: &mut Document, id: u32, text: &str) {
    run(d, serde_json::json!({"op":"insert_text","at":{"p":id,"o":0},"text":text}));
}

fn reused(before: &[Piece], after: &[Piece]) -> usize {
    after.iter().filter(|x| before.iter().any(|y| Arc::ptr_eq(x, y))).count()
}

#[test]
fn an_edit_in_the_first_middle_or_last_piece_makes_that_piece_again_and_keeps_the_rest() {
    let mut d = open();
    d.save().unwrap();
    let mut prev = check(&d);
    assert!(prev.len() > 6, "{} pieces", prev.len());
    let ids = order(&mut d);
    for at in [0, ids.len() / 2, ids.len() - 1] {
        insert(&mut d, ids[at], "edited ");
        d.save().unwrap();
        let ps = check(&d);
        // The head and the tail are written every time; the edit makes one piece (or two at a boundary) again.
        assert!(reused(&prev, &ps) + 5 >= ps.len(), "block {at}: {} of {} pieces kept", reused(&prev, &ps), ps.len());
        assert!(reused(&prev, &ps) > 0);
        prev = ps;
    }
}

#[test]
fn a_block_that_grows_past_the_limit_is_a_piece_of_its_own() {
    let mut d = open();
    d.save().unwrap();
    let ids = order(&mut d);
    insert(&mut d, ids[ids.len() / 2], &"long ".repeat(LIMIT));
    d.save().unwrap();
    let ps = check(&d);
    assert_eq!(ps.iter().filter(|p| p.len() > LIMIT).count(), 1, "one piece over the limit");
    assert!(d.undo());
    d.save().unwrap();
    let ps = check(&d);
    assert!(ps.iter().all(|p| p.len() <= LIMIT));
}

#[test]
fn blocks_added_and_removed_across_piece_boundaries() {
    let mut d = open();
    d.save().unwrap();
    let mut prev = check(&d);
    let ids = order(&mut d);
    let mid = ids[ids.len() / 2];
    for n in 0..30 {
        run(&mut d, serde_json::json!({"op":"insert_paragraph","after":mid,"text":format!("a new paragraph number {n} with some words in it")}));
    }
    d.save().unwrap();
    let ps = check(&d);
    assert!(reused(&prev, &ps) > 0 && ps.len() > prev.len());
    prev = ps;
    let ids = order(&mut d);
    let gone: Vec<u32> = ids[10..ids.len() - 10].iter().step_by(2).copied().collect();
    run(&mut d, serde_json::json!({"op":"delete_blocks","ids":gone}));
    d.save().unwrap();
    let ps = check(&d);
    assert!(reused(&prev, &ps) > 0);
    // Added at the very start and the very end.
    let ids = order(&mut d);
    run(&mut d, serde_json::json!({"op":"insert_paragraph","before":ids[0],"text":"first"}));
    run(&mut d, serde_json::json!({"op":"insert_paragraph","after":ids[ids.len() - 1],"text":"last"}));
    d.save().unwrap();
    check(&d);
}

#[test]
fn undo_and_redo_across_saves() {
    let mut d = open();
    d.save().unwrap();
    let first = check(&d);
    let whole: Vec<u8> = first.iter().flat_map(|p| p.iter().copied()).collect();
    let ids = order(&mut d);
    insert(&mut d, ids[3], "one ");
    insert(&mut d, ids[ids.len() - 2], "two ");
    d.save().unwrap();
    check(&d);
    assert!(d.undo());
    d.save().unwrap();
    check(&d);
    assert!(d.undo());
    d.save().unwrap();
    let ps = check(&d);
    assert!(ps.iter().flat_map(|p| p.iter().copied()).collect::<Vec<u8>>() == whole, "undone twice: the body as it was");
    assert!(d.redo());
    d.save().unwrap();
    check(&d);
    // An undo with no save in between, then a save.
    insert(&mut d, ids[5], "three ");
    assert!(d.undo());
    d.save().unwrap();
    check(&d);
}

#[test]
fn saving_editing_and_idle_work_never_join_the_part() {
    let mut d = open();
    // The zip route (`save`) joins the part for the length of its write (see `write_zip`); the browser's route does not.
    let before = docxtor_core::perf::parts_joined();
    let ids = order(&mut d);
    for round in 0..3 {
        insert(&mut d, ids[round * 7], "typing ");
        let _ = d.unused_media();
        let mut pieces: Vec<usize> = Vec::new();
        d.save_packed_pieces(1 << 20, &mut |p| pieces.push(p.len())).unwrap();
        assert!(d.undo() && d.redo());
        let _ = d.render_all();
    }
    assert_eq!(docxtor_core::perf::parts_joined(), before, "something joined the main part into one buffer");
    // Asking for the part whole still works, and is counted.
    let whole = d.pkg.get(&d.main_path).unwrap().to_vec();
    assert!(whole == d.document_xml_whole());
    assert!(docxtor_core::perf::parts_joined() > before);
}

#[test]
fn the_zip_of_the_pieces_is_the_zip_of_the_joined_part() {
    let mut d = open();
    let ids = order(&mut d);
    insert(&mut d, ids[2], "x");
    d.save().unwrap();
    let zip = d.pkg.to_bytes().unwrap();
    let ps = check(&d);
    let mut joined_pkg = d.pkg.clone();
    let main = d.main_path.clone();
    joined_pkg.set(&main, ps.iter().flat_map(|p| p.iter().copied()).collect());
    assert!(joined_pkg.pieces_of(&main).is_none());
    let z2 = joined_pkg.to_bytes().unwrap();
    assert!(zip == z2, "the zip written from pieces differs from the zip written from one buffer");
    // And what the browser gets, joined, becomes the same zip.
    let mut packed: Vec<u8> = Vec::new();
    d.save_packed_pieces(1000, &mut |p| packed.extend_from_slice(p)).unwrap();
    assert!(docxtor_core::package::zip_packed(&packed).unwrap() == d.save().unwrap());
}

#[test]
fn a_package_put_back_by_a_journal_has_the_main_part_whole() {
    let mut d = open();
    d.save().unwrap();
    let main = d.main_path.clone();
    let whole = d.document_xml_whole();
    d.pkg.begin_journal();
    d.pkg.set(&main, b"<changed/>".to_vec());
    assert!(d.pkg.pieces_of(&main).is_none());
    d.pkg.rollback_journal();
    assert!(d.pkg.get(&main).unwrap() == whole.as_slice());
}
