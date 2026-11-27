//! Pictures, charts and workbooks that nothing uses any more are left out of the saved file (a deleted picture is
//! not stored forever, and a deleted chart's data does not stay behind), while undo can still bring them back.

use docxtor_core::package::Package;
use docxtor_core::Document;
use serde_json::json;

fn edit(doc: &mut Document, value: serde_json::Value) {
    doc.apply_json(&value.to_string(), false).unwrap_or_else(|e| panic!("{e}: {value}"));
}

fn first_paragraph(doc: &mut Document) -> u32 {
    doc.render_all().order[0]
}

/// About `n` bytes that do not compress, different for every `seed`.
fn noise(seed: u32, n: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(2654435761).wrapping_add(12345);
    let mut out = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10];
    while out.len() < n {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        out.push((x >> 8) as u8);
    }
    out
}

fn saved(doc: &mut Document) -> Vec<u8> {
    doc.save().unwrap()
}

fn media_parts(bytes: &[u8]) -> Vec<String> {
    Package::open(bytes).unwrap().parts.iter().filter(|p| p.name.starts_with("word/media/")).map(|p| p.name.clone()).collect()
}

fn chart_parts(bytes: &[u8]) -> Vec<String> {
    Package::open(bytes).unwrap().parts.iter().filter(|p| p.name.starts_with("word/charts/") || p.name.starts_with("word/embeddings/")).map(|p| p.name.clone()).collect()
}

fn insert_picture(doc: &mut Document, p: u32, seed: u32) {
    let media = doc.add_media("png", noise(seed, 20_000));
    edit(doc, json!({"op": "insert_image", "at": {"p": p, "o": 0}, "media": media, "width": 100, "height": 50}));
}

fn delete_object(doc: &mut Document, p: u32) {
    edit(doc, json!({"op": "delete", "from": {"p": p, "o": 0}, "to": {"p": p, "o": 1}}));
}

fn chart_op(p: u32, secret: &str) -> serde_json::Value {
    json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","title":secret,"categories":["A","B","C"],"series":[{"name":secret,"values":[1,2,3]}]})
}

fn all_text(bytes: &[u8]) -> String {
    let pkg = Package::open(bytes).unwrap();
    let mut out = String::new();
    for p in &pkg.parts {
        out.push_str(&String::from_utf8_lossy(&p.data));
        // A workbook is a zip of its own.
        if p.name.ends_with(".xlsx") {
            let inner = Package::open(&p.data).unwrap();
            for q in &inner.parts {
                out.push_str(&String::from_utf8_lossy(&q.data));
            }
        }
    }
    out
}

#[test]
fn deleted_pictures_are_not_saved_and_the_file_stays_the_same_size() {
    let mut doc = Document::new_blank();
    let p = first_paragraph(&mut doc);
    let flat = saved(&mut doc).len();
    for round in 0..60 {
        insert_picture(&mut doc, p, round);
        assert_eq!(media_parts(&saved(&mut doc)).len(), 1, "a picture in use is saved");
        delete_object(&mut doc, p);
    }
    let bytes = saved(&mut doc);
    assert!(media_parts(&bytes).is_empty(), "{:?}", media_parts(&bytes));
    let rels = Package::open(&bytes).unwrap().rels_for("word/document.xml").unwrap();
    assert!(rels.iter().all(|r| !r.rel_type.ends_with("/image")), "no image relationships are left");
    assert!(bytes.len() <= flat + 200, "the file grew from {flat} to {}", bytes.len());
    // The saved file opens and has no picture.
    Document::open(&bytes).unwrap();
}

#[test]
fn undo_after_deleting_a_picture_brings_it_back_even_after_a_save() {
    let mut doc = Document::new_blank();
    let p = first_paragraph(&mut doc);
    insert_picture(&mut doc, p, 7);
    delete_object(&mut doc, p);
    assert!(media_parts(&saved(&mut doc)).is_empty());
    assert!(doc.undo());
    let bytes = saved(&mut doc);
    assert_eq!(media_parts(&bytes).len(), 1);
    // And the picture is really there: the file opens with it.
    let mut again = Document::open(&bytes).unwrap();
    let first = again.render_all().order[0];
    assert!(again.picture_at(docxtor_core::Pos::new(first, 0)).is_some());
    // Redo deletes it again.
    assert!(doc.redo());
    assert!(media_parts(&saved(&mut doc)).is_empty());
}

#[test]
fn the_same_picture_added_again_is_stored_once() {
    let mut doc = Document::new_blank();
    let a = doc.add_media("png", noise(1, 5_000));
    let b = doc.add_media("png", noise(1, 5_000));
    let c = doc.add_media("png", noise(2, 5_000));
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn a_picture_deleted_from_a_file_that_had_it_is_left_out() {
    let mut doc = Document::new_blank();
    let p = first_paragraph(&mut doc);
    insert_picture(&mut doc, p, 3);
    let bytes = saved(&mut doc);
    let mut doc = Document::open(&bytes).unwrap();
    let p = doc.render_all().order[0];
    delete_object(&mut doc, p);
    assert!(media_parts(&saved(&mut doc)).is_empty());
    assert!(doc.undo());
    assert_eq!(media_parts(&saved(&mut doc)).len(), 1);
}

#[test]
fn a_picture_the_file_already_did_not_use_is_kept() {
    // The file's own leftovers are not Docxtor's to throw away.
    let mut pkg = Package::open(&saved(&mut Document::new_blank())).unwrap();
    pkg.ensure_default("png", "image/png");
    pkg.set("word/media/leftover.png", noise(9, 3_000));
    let original = pkg.to_bytes().unwrap();
    let mut doc = Document::open(&original).unwrap();
    // Untouched: byte for byte.
    assert_eq!(saved(&mut doc), original);
    let p = doc.render_all().order[0];
    edit(&mut doc, json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "Hello"}));
    insert_picture(&mut doc, p, 4);
    delete_object(&mut doc, p);
    let bytes = saved(&mut doc);
    assert_eq!(media_parts(&bytes), vec!["word/media/leftover.png".to_string()]);
}

#[test]
fn a_picture_used_by_a_part_docxtor_does_not_model_is_kept() {
    let mut pkg = Package::open(&saved(&mut Document::new_blank())).unwrap();
    pkg.ensure_default("png", "image/png");
    pkg.set("word/media/legacy.png", noise(5, 3_000));
    pkg.set("word/legacy.vml", br#"<xml xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><v:shape><v:imagedata o:relid="rId1"/></v:shape></xml>"#.to_vec());
    pkg.add_rel("word/legacy.vml", docxtor_core::package::REL_IMAGE, "media/legacy.png", false);
    pkg.add_rel("word/document.xml", "http://example.com/relationships/legacy", "legacy.vml", false);
    // And one that is referenced only by name in an unmodelled part.
    pkg.set("word/media/byname.png", noise(6, 3_000));
    pkg.set("customXml/item9.xml", br#"<item><picture src="word/media/byname.png"/></item>"#.to_vec());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let p = doc.render_all().order[0];
    insert_picture(&mut doc, p, 8);
    delete_object(&mut doc, p);
    let mut kept = media_parts(&saved(&mut doc));
    kept.sort();
    assert_eq!(kept, vec!["word/media/byname.png".to_string(), "word/media/legacy.png".to_string()]);
}

#[test]
fn a_picture_used_in_a_header_stays_when_the_body_copy_goes() {
    let mut doc = Document::new_blank();
    let p = first_paragraph(&mut doc);
    let media = doc.add_media("png", noise(11, 4_000));
    edit(&mut doc, json!({"op": "insert_image", "at": {"p": p, "o": 0}, "media": media, "width": 100, "height": 50}));
    edit(&mut doc, json!({"op": "set_picture_watermark", "media": media, "width": 100, "height": 50}));
    delete_object(&mut doc, p);
    assert_eq!(media_parts(&saved(&mut doc)).len(), 1, "the watermark still uses it");
}

#[test]
fn deleted_charts_are_not_saved_and_their_data_does_not_stay_in_the_file() {
    let mut doc = Document::new_blank();
    let p = first_paragraph(&mut doc);
    let flat = saved(&mut doc).len();
    for _ in 0..40 {
        edit(&mut doc, chart_op(p, "TopSecretRevenue"));
        let during = saved(&mut doc);
        assert!(!chart_parts(&during).is_empty(), "a chart in use is saved");
        assert!(all_text(&during).contains("TopSecretRevenue"));
        delete_object(&mut doc, p);
    }
    let bytes = saved(&mut doc);
    assert!(chart_parts(&bytes).is_empty(), "{:?}", chart_parts(&bytes));
    assert!(!all_text(&bytes).contains("TopSecretRevenue"));
    let pkg = Package::open(&bytes).unwrap();
    assert!(pkg.rels_for("word/document.xml").unwrap().iter().all(|r| !r.rel_type.ends_with("/chart")));
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(!ct.contains("/word/charts/"), "{ct}");
    assert!(bytes.len() <= flat + 200, "the file grew from {flat} to {}", bytes.len());
    Document::open(&bytes).unwrap();
}

#[test]
fn undo_after_deleting_a_chart_brings_back_the_chart_and_its_workbook() {
    let mut doc = Document::new_blank();
    let p = first_paragraph(&mut doc);
    edit(&mut doc, chart_op(p, "Quarterly"));
    delete_object(&mut doc, p);
    assert!(chart_parts(&saved(&mut doc)).is_empty());
    assert!(doc.undo());
    let bytes = saved(&mut doc);
    assert!(chart_parts(&bytes).iter().any(|n| n.ends_with(".xlsx")), "{:?}", chart_parts(&bytes));
    assert!(all_text(&bytes).contains("Quarterly"));
    let mut again = Document::open(&bytes).unwrap();
    let first = again.render_all().order[0];
    assert_eq!(again.charts_in(first).len(), 1);
    // Undoing the insert itself takes the chart parts out for good.
    assert!(doc.undo());
    assert!(chart_parts(&saved(&mut doc)).is_empty());
    assert!(doc.redo());
    assert!(!chart_parts(&saved(&mut doc)).is_empty());
}

#[test]
fn chart_operations_do_not_copy_the_charts_nothing_uses() {
    // Many deleted charts stay in memory for undo, but each new chart operation costs about the same.
    let mut doc = Document::new_blank();
    let p = first_paragraph(&mut doc);
    for _ in 0..30 {
        edit(&mut doc, chart_op(p, "x"));
        delete_object(&mut doc, p);
    }
    let before = doc.undo_weight().0;
    edit(&mut doc, chart_op(p, "x"));
    let step = doc.undo_weight().0 - before;
    assert!(step < 100_000, "one more chart insert added about {step} bytes to the undo history");
}

#[test]
fn asking_which_pictures_are_unused_changes_nothing_a_save_depends_on() {
    // A change to the styles alone: the saved document.xml stays the file's own bytes, and the document is as modified
    // as it was.
    let original = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rich.docx")).unwrap();
    let body_of = |bytes: &[u8]| Package::open(bytes).unwrap().get("word/document.xml").unwrap().to_vec();
    let mut doc = Document::open(&original).unwrap();
    edit(&mut doc, json!({"op": "set_style_definition", "style": "Heading1", "def": {"bold": true}}));
    assert!(doc.is_modified());
    let _ = doc.unused_media();
    assert!(doc.is_modified());
    let bytes = saved(&mut doc);
    assert_eq!(body_of(&bytes), body_of(&original), "the body was rewritten because the sweep marked it changed");
    // And a document with nothing changed is still not modified afterwards.
    let mut clean = Document::open(&original).unwrap();
    let _ = clean.unused_media();
    assert!(!clean.is_modified());
}
