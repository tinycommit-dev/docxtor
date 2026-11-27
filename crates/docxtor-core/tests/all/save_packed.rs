//! A save that goes on in steps (`stamp`, `save_packed`, then `zip_packed` anywhere) writes the same bytes as `save_stamped`,
//! and the first blocks' DocScript (`docscript_head`) is the start of the whole.

use crate::common::repo_path;
use docxtor_core::{ai, package::zip_packed, Document};

fn fixture(name: &str) -> Vec<u8> {
    match name.strip_prefix("built:") {
        Some(_) => Document::new_blank().save().unwrap(),
        None => std::fs::read(repo_path(&format!("fixtures/{name}"))).unwrap(),
    }
}

const NOW: &str = "2026-11-21T09:30:00Z";

#[test]
fn stepwise_save_is_byte_for_byte_the_whole_save() {
    for name in ["built:blank", "rich.docx"] {
        let bytes = fixture(name);
        // Untouched, and after an edit (so the dates and statistics are written).
        for edit in [false, true] {
            let mut a = Document::open(&bytes).unwrap();
            let mut b = Document::open(&bytes).unwrap();
            if edit {
                for d in [&mut a, &mut b] {
                    let first = d.render_snapshot().order[0];
                    d.apply_batch(&[docxtor_core::ops::Op::InsertText { at: docxtor_core::Pos::new(first, 0), text: "x".into(), format: None }], false).unwrap();
                }
            }
            let whole = a.save_stamped(NOW).unwrap();
            b.stamp(NOW);
            let packed = b.save_packed().unwrap();
            assert_eq!(zip_packed(&packed).unwrap(), whole, "{name} edit={edit}");
        }
    }
}

#[test]
fn packed_parts_that_are_cut_short_are_refused() {
    let mut d = Document::open(&fixture("rich.docx")).unwrap();
    let packed = d.save_packed().unwrap();
    assert!(zip_packed(&packed[..packed.len() - 3]).is_err());
}

#[test]
fn the_head_of_a_document_reads_like_the_start_of_the_whole() {
    let mut d = Document::open(&fixture("rich.docx")).unwrap();
    let whole = ai::docscript(&mut d, &ai::ScriptOpts { start: None, limit: 4 });
    let head = ai::docscript_head(&d, 4);
    let body = |s: &str| s.lines().skip(1).take_while(|l| !l.starts_with('#')).map(String::from).collect::<Vec<_>>();
    assert_eq!(body(&head), body(&whole));
}

/// What the browser gets: the parts in pieces (`save_packed_pieces`), joined, are the bytes `save_packed` gives, untouched and after
/// an edit, with pieces small enough that a part is cut in several and a piece holds several small parts.
#[test]
fn the_parts_in_pieces_join_to_the_packed_parts() {
    for name in ["built:blank", "rich.docx", "long.docx"] {
        for edit in [false, true] {
            let bytes = fixture(name);
            let (mut a, mut b) = (Document::open(&bytes).unwrap(), Document::open(&bytes).unwrap());
            if edit {
                for d in [&mut a, &mut b] {
                    let first = d.render_all().order[0];
                    d.apply_json(&serde_json::json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}).to_string(), false).unwrap();
                }
            }
            for max in [1000, 1 << 20] {
                let mut pieces: Vec<Vec<u8>> = Vec::new();
                a.save_packed_pieces(max, &mut |p| pieces.push(p.to_vec())).unwrap();
                assert!(pieces.iter().all(|p| p.len() <= max && !p.is_empty()), "{name}: a piece over {max} bytes, or empty");
                assert_eq!(pieces.concat(), b.save_packed().unwrap(), "{name}, edited {edit}, pieces of {max}");
                if max == 1000 {
                    assert!(pieces.len() > 3, "{name}: {} pieces", pieces.len());
                }
            }
        }
    }
}
