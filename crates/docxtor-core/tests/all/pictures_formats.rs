//! TIFF, EMF and WMF pictures: the engine shows them as PNG or SVG (`Document::media_display`), the file keeps
//! the originals, and the HTML and EPUB downloads carry the converted pictures.

use docxtor_core::export::html::to_html;
use docxtor_core::package::Package;
use docxtor_core::Document;
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

fn le32(v: &mut Vec<u8>, n: u32) {
    v.extend(n.to_le_bytes());
}

/// A 2 x 2 uncompressed RGB TIFF: red and green over blue and white.
fn tiny_tiff() -> Vec<u8> {
    let mut t = vec![b'I', b'I', 42, 0];
    le32(&mut t, 8);
    let entries: [(u16, u16, u32, u32); 9] = [(256, 3, 1, 2), (257, 3, 1, 2), (258, 3, 3, 122), (259, 3, 1, 1), (262, 3, 1, 2), (273, 4, 1, 128), (277, 3, 1, 3), (278, 3, 1, 2), (279, 4, 1, 12)];
    t.extend((entries.len() as u16).to_le_bytes());
    for (tag, ty, count, value) in entries {
        t.extend(tag.to_le_bytes());
        t.extend(ty.to_le_bytes());
        le32(&mut t, count);
        le32(&mut t, value);
    }
    le32(&mut t, 0);
    assert_eq!(t.len(), 122);
    for _ in 0..3 {
        t.extend(8u16.to_le_bytes());
    }
    t.extend([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
    t
}

/// An EMF with nothing drawn: the header and the end record, for a 100 x 50 unit picture.
fn empty_emf() -> Vec<u8> {
    let mut e = Vec::new();
    for n in [1u32, 88] {
        le32(&mut e, n);
    }
    for n in [0i32, 0, 99, 49, 0, 0, 2646, 1323] {
        e.extend(n.to_le_bytes());
    }
    le32(&mut e, 0x464D4520);
    for n in [0x10000u32, 108, 2, 1, 0, 0, 0] {
        le32(&mut e, n);
    }
    for n in [1024i32, 768, 320, 240] {
        e.extend(n.to_le_bytes());
    }
    e.resize(88, 0);
    for n in [14u32, 20, 0, 16, 20] {
        le32(&mut e, n);
    }
    let total = e.len() as u32;
    e[48..52].copy_from_slice(&total.to_le_bytes());
    e
}

#[test]
fn tiff_emf_and_wmf_are_shown_as_png_and_svg_and_kept_as_they_are() {
    let mut doc = Document::new_blank();
    let p = doc.render_snapshot().order[0];
    let tiff = doc.add_media("tif", tiny_tiff());
    let emf = doc.add_media("emf", empty_emf());
    let broken = doc.add_media("wmf", vec![1, 2, 3, 4]);
    let plain = doc.add_media("png", vec![0x89, b'P', b'N', b'G']);
    assert!(tiff.ends_with(".tif") && emf.ends_with(".emf"));
    let png = doc.media_display(&tiff).expect("TIFF shows as PNG");
    assert_eq!((png.mime, &png.bytes[1..4]), ("image/png", &b"PNG"[..]));
    let svg = doc.media_display(&emf).expect("EMF shows as SVG");
    assert_eq!(svg.mime, "image/svg+xml");
    assert!(String::from_utf8(svg.bytes).unwrap().contains("<svg"));
    // What the browser shows as it is, and what cannot be converted.
    assert!(doc.media_display(&plain).is_none() && doc.media_display_bytes(&plain).is_empty());
    assert_eq!(doc.media_display_bytes(&broken), vec![0]);
    assert_eq!(doc.media_display_bytes(&tiff)[0], 1);
    assert_eq!(doc.media_display_bytes(&emf)[0], 2);
    // They can be put in a document; the file keeps the originals and says what they are.
    ops(&mut doc, json!({"op": "insert_image", "at": {"p": p, "o": 0}, "media": tiff, "width": 20, "height": 20}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    assert_eq!(pkg.get(&tiff).unwrap(), tiny_tiff().as_slice());
    let types = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(types.contains(r#"Extension="tif""#) && types.contains("image/tiff"), "{types}");
    let again = Document::open(&bytes).unwrap();
    assert!(again.media_display(&tiff).is_some());
}

#[test]
fn the_html_download_carries_a_tiff_as_a_png() {
    let mut doc = Document::new_blank();
    let p = doc.render_snapshot().order[0];
    let tiff = doc.add_media("tiff", tiny_tiff());
    ops(&mut doc, json!({"op": "insert_image", "at": {"p": p, "o": 0}, "media": tiff, "width": 20, "height": 20}));
    let html = to_html(&doc);
    assert!(html.contains("<img src=\"data:image/png;base64,"), "{html}");
}
