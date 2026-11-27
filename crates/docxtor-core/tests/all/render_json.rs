//! The browser gets the render as JSON written a block at a time (`render_json` over `render_all_each` and `render_changed_each`), so the drawing of a long
//! document is never all in memory at once. It must be the same object `render_all` and `render_changed` give: checked on every fixture,
//! for a full render, a typed character, a change of the Normal style (every block drawn again), a new paragraph (the order moves), undo
//! and redo. Two copies of the document take the same steps, one read each way. `DOCXTOR_RENDER_JSON_DIR=<dir>` checks the files in
//! another folder too (the public corpus: `~/.cache/docxtor-corpus`).

use docxtor_core::document::render_json;
use docxtor_core::Document;
use serde_json::{json, Value};

fn same(a: &mut Document, b: &mut Document, full: bool, what: &str) {
    // Both read back from text: serde_json's reader can land a float one step off the number written (JavaScript's does not), so
    // the two sides go through the same reading.
    let (streamed, kept) = if full { (render_json(|each| a.render_all_each(each)), serde_json::to_string(&b.render_all()).unwrap()) } else { (render_json(|each| a.render_changed_each(each)), serde_json::to_string(&b.render_changed()).unwrap()) };
    let (streamed, kept): (Value, Value) = (serde_json::from_str(&streamed).unwrap(), serde_json::from_str(&kept).unwrap());
    assert!(streamed == kept, "{what}: the JSON render differs\nstreamed: {streamed}\nkept: {kept}");
    let fonts = |d: &Document| d.fonts_used().into_iter().map(|u| (u.family, u.chars)).collect::<Vec<_>>();
    assert_eq!(fonts(a), fonts(b), "{what}: the fonts kept from the render differ");
}

fn both(a: &mut Document, b: &mut Document, op: Value) {
    let s = op.to_string();
    let (ra, rb) = (a.apply_json(&s, false).is_ok(), b.apply_json(&s, false).is_ok());
    assert_eq!(ra, rb);
}

#[test]
fn the_json_render_is_the_render_on_every_fixture() {
    let mut files = Vec::new();
    let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")];
    dirs.extend(std::env::var_os("DOCXTOR_RENDER_JSON_DIR").map(std::path::PathBuf::from));
    while let Some(dir) = dirs.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = e.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|x| x == "docx") {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut n = 0;
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let (Ok(mut a), Ok(mut b)) = (Document::open(&bytes), Document::open(&bytes)) else { continue };
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        same(&mut a, &mut b, true, &format!("{name}, opened"));
        let order = b.render_snapshot().order;
        if let Some(&first) = order.first() {
            both(&mut a, &mut b, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"x"}));
            same(&mut a, &mut b, false, &format!("{name}, typed"));
            both(&mut a, &mut b, json!({"op":"set_style_definition","style":"Normal","def":{"rpr":{"size":13.0}}}));
            same(&mut a, &mut b, false, &format!("{name}, Normal changed"));
            both(&mut a, &mut b, json!({"op":"insert_paragraph","after":first,"text":"A new paragraph."}));
            same(&mut a, &mut b, false, &format!("{name}, a paragraph added"));
            assert_eq!(a.undo(), b.undo());
            same(&mut a, &mut b, false, &format!("{name}, undone"));
            assert_eq!(a.redo(), b.redo());
            same(&mut a, &mut b, false, &format!("{name}, redone"));
            same(&mut a, &mut b, false, &format!("{name}, nothing changed"));
        }
        same(&mut a, &mut b, true, &format!("{name}, drawn again"));
        n += 1;
    }
    // A fresh clone has six documents here that open (the rest of fixtures/out comes from other tests and is not in git).
    assert!(n >= 6, "only {n} fixtures");
}

#[test]
fn a_long_render_comes_in_pieces_that_join_to_the_same_json() {
    let mut d = Document::new_blank();
    let first = d.render_all().order[0];
    let md = (0..3000).map(|i| format!("Paragraph {i} with *some* words and a little more text.")).collect::<Vec<_>>().join("\n\n");
    d.apply_json(&json!({"op":"insert_markdown","before":first,"markdown":md}).to_string(), false).unwrap();
    let bytes = d.save().unwrap();
    let (mut a, mut b) = (Document::open(&bytes).unwrap(), Document::open(&bytes).unwrap());
    let mut pieces = Vec::new();
    docxtor_core::document::render_json_pieces(|each| a.render_all_each(each), |p| pieces.push(p.to_string()));
    assert!(pieces.len() > 4, "{} pieces", pieces.len());
    assert!(pieces.iter().all(|p| p.len() < 80 << 10), "a piece is far over 64 KB");
    let joined: Value = serde_json::from_str(&pieces.concat()).unwrap();
    let whole: Value = serde_json::from_str(&render_json(|each| b.render_all_each(each))).unwrap();
    assert!(joined == whole);
}
