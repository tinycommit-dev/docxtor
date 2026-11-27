//! Imported content chunks (`w:altChunk`): HTML, plain text, RTF, a web page archive and a whole .docx put inside a
//! document by the tools that make .docx files. Each is shown as its content, a save leaves the file as it came, the
//! first edit inside one makes it ordinary content (the chunk's part goes) in one undo step, and the file stays valid.

use docxtor_core::package::Package;
use docxtor_core::Document;
use serde_json::json;

const AF_CHUNK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/aFChunk";

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

/// A document of "Before", the chunk (or the chunk in a table cell), "After"; the chunk's part holds `data`.
fn with_chunk(name: &str, content_type: &str, data: &[u8], in_table: bool) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let rid = pkg.add_rel("word/document.xml", AF_CHUNK, name, false);
    pkg.set(&format!("word/{name}"), data.to_vec());
    pkg.ensure_override(&format!("word/{name}"), content_type);
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let chunk = format!(r#"<w:altChunk r:id="{rid}"/>"#);
    let chunk = if in_table {
        format!(r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr>{chunk}<w:p/></w:tc></w:tr></w:tbl>"#)
    } else {
        chunk
    };
    let body = format!(r#"<w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p>{chunk}<w:p><w:r><w:t>After</w:t></w:r></w:p>"#);
    let xml = xml.replacen("<w:body>", &body, 1).replacen("<w:p/><w:sectPr", "<w:sectPr", 1);
    let xml = if xml.contains("xmlns:r=") { xml } else { xml.replacen("<w:document ", r#"<w:document xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#, 1) };
    pkg.set("word/document.xml", xml.into_bytes());
    pkg.to_bytes().unwrap()
}

fn shown(doc: &mut Document) -> String {
    serde_json::to_string(&doc.render_all()).unwrap()
}

fn part(bytes: &[u8], name: &str) -> Option<String> {
    Package::open(bytes).unwrap().get(name).map(|d| String::from_utf8_lossy(d).into_owned())
}

const HTML: &str = r#"<html><head><style>p.lead{color:#c00000}</style></head><body><h1>Quarterly report</h1><p class="lead">Sales grew <b>twelve</b> percent.</p><ul><li>North</li><li>South</li></ul><table border="1"><tr><td>Region</td><td>Units</td></tr><tr><td>North</td><td>40</td></tr></table></body></html>"#;

#[test]
fn an_html_chunk_shows_its_text_lists_and_table() {
    let bytes = with_chunk("afchunk.htm", "text/html", HTML.as_bytes(), false);
    let mut doc = Document::open(&bytes).unwrap();
    let r = shown(&mut doc);
    for s in ["Before", "Quarterly report", "twelve", "North", "South", "Region", "Units", "After"] {
        assert!(r.contains(s), "{s} is shown");
    }
    // Before, then the chunk's text, then After, in order.
    let at = |s: &str| r.find(&format!(r#""x":"{s}""#)).unwrap_or_else(|| panic!("{s}"));
    assert!(at("Before") < at("Quarterly report") && at("Region") < at("After"));
}

#[test]
fn saving_untouched_keeps_the_chunk_and_its_part_as_they_came() {
    let bytes = with_chunk("afchunk.htm", "text/html", HTML.as_bytes(), false);
    let mut doc = Document::open(&bytes).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    assert_eq!(part(&saved, "word/document.xml"), part(&bytes, "word/document.xml"));
    assert_eq!(part(&saved, "word/afchunk.htm"), part(&bytes, "word/afchunk.htm"));
}

#[test]
fn editing_inside_a_chunk_makes_it_ordinary_content_in_one_undo_step() {
    let bytes = with_chunk("afchunk.htm", "text/html", HTML.as_bytes(), false);
    let mut doc = Document::open(&bytes).unwrap();
    let order = doc.render_all().order;
    // The chunk's first paragraph is the one after Before.
    let inside = order[1];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":inside,"o":0},"text":"Draft: "}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let xml = part(&saved, "word/document.xml").unwrap();
    assert!(!xml.contains("w:altChunk"), "the chunk is real content now");
    assert!(xml.contains("Draft: ") && xml.contains("Quarterly report") && xml.contains("Region"));
    assert!(part(&saved, "word/afchunk.htm").is_none(), "its part is gone");
    assert!(doc.undo());
    let back = doc.save().unwrap();
    assert!(part(&back, "word/document.xml").unwrap().contains("w:altChunk"), "undo brings the chunk back");
    assert!(part(&back, "word/afchunk.htm").is_some());
}

#[test]
fn make_editable_without_a_change() {
    let bytes = with_chunk("afchunk.htm", "text/html", HTML.as_bytes(), false);
    let mut doc = Document::open(&bytes).unwrap();
    let inside = doc.render_all().order[1];
    doc.apply_json(&json!({"op":"expand_alt_chunk","id":inside}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    assert!(!part(&saved, "word/document.xml").unwrap().contains("w:altChunk"));
}

#[test]
fn plain_text_rtf_and_web_archive_chunks_are_shown() {
    let text = with_chunk("chunk.txt", "text/plain", "First line\r\nSecond line".as_bytes(), false);
    assert!(shown(&mut Document::open(&text).unwrap()).contains("Second line"));
    let rtf = with_chunk("chunk.rtf", "application/rtf", br"{\rtf1\ansi{\fonttbl{\f0 Arial;}}\f0 Hello from {\b RTF}\par Next paragraph\par}", false);
    let r = shown(&mut Document::open(&rtf).unwrap());
    assert!(r.contains("Hello from") && r.contains("Next paragraph"));
    let mht = "MIME-Version: 1.0\r\nContent-Type: multipart/related; boundary=\"XYZ\"\r\n\r\n--XYZ\r\nContent-Type: text/html; charset=\"utf-8\"\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Location: file:///C:/page.htm\r\n\r\n<html><body><p>Caf=C3=A9 menu</p></body></html>\r\n--XYZ--\r\n";
    let archive = with_chunk("chunk.mht", "message/rfc822", mht.as_bytes(), false);
    assert!(shown(&mut Document::open(&archive).unwrap()).contains("Café menu"));
}

#[test]
fn a_docx_chunk_brings_its_own_styles() {
    let mut inner = Document::new_blank();
    let first = inner.render_all().order[0];
    inner.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"From another document"}).to_string(), false).unwrap();
    inner.apply_json(&json!({"op":"set_style","ids":[first],"style":"Heading1"}).to_string(), false).unwrap();
    let data = inner.save().unwrap();
    let bytes = with_chunk("chunk.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml", &data, false);
    let mut doc = Document::open(&bytes).unwrap();
    assert!(shown(&mut doc).contains("From another document"));
    let inside = doc.render_all().order[1];
    doc.apply_json(&json!({"op":"expand_alt_chunk","id":inside}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    assert!(part(&saved, "word/document.xml").unwrap().contains("From another document"));
}

#[test]
fn a_chunk_in_a_table_cell_is_shown_and_saved_as_it_came() {
    let bytes = with_chunk("afchunk.htm", "text/html", b"<p>In the cell</p>", true);
    let mut doc = Document::open(&bytes).unwrap();
    assert!(shown(&mut doc).contains("In the cell"));
    let saved = doc.save().unwrap();
    assert_eq!(part(&saved, "word/document.xml"), part(&bytes, "word/document.xml"));
}

#[test]
fn replace_all_and_replace_font_reach_a_chunk_in_a_table_cell_and_are_saved() {
    let bytes = with_chunk("afchunk.htm", "text/html", b"<p>Figures for 2025</p>", true);
    let mut doc = Document::open(&bytes).unwrap();
    doc.apply_json(&json!({"op":"replace_text","find":"2025","replace":"2026","all":true}).to_string(), false).unwrap();
    assert!(shown(&mut doc).contains("Figures for 2026"));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let xml = part(&saved, "word/document.xml").unwrap();
    assert!(xml.contains("Figures for 2026"), "the replacement is saved: {xml}");
    assert!(!xml.contains("w:altChunk"), "the chunk became ordinary content: {xml}");
    // One undo step gives the chunk back, with its old text.
    assert!(doc.undo());
    assert!(shown(&mut doc).contains("Figures for 2025"));
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert!(xml.contains("w:altChunk") && !xml.contains("2026"), "{xml}");

    // An op that touches the whole document (Replace font) too.
    let mut doc = Document::open(&with_chunk("afchunk.htm", "text/html", b"<p style=\"font-family:Georgia\">In Georgia</p>", true)).unwrap();
    doc.apply_json(&json!({"op":"replace_font","from":"Georgia","to":"Arial"}).to_string(), false).unwrap();
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert!(!xml.contains("w:altChunk") && xml.contains("Arial"), "{xml}");
}

/// "Before", two chunks one after the other (each its own part), "After".
fn with_two_chunks(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let ra = pkg.add_rel("word/document.xml", AF_CHUNK, "a.htm", false);
    pkg.set("word/a.htm", a.to_vec());
    pkg.ensure_override("word/a.htm", "text/html");
    let rb = pkg.add_rel("word/document.xml", AF_CHUNK, "b.htm", false);
    pkg.set("word/b.htm", b.to_vec());
    pkg.ensure_override("word/b.htm", "text/html");
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let body = format!(r#"<w:body><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:altChunk r:id="{ra}"/><w:altChunk r:id="{rb}"/><w:p><w:r><w:t>After</w:t></w:r></w:p>"#);
    let xml = xml.replacen("<w:body>", &body, 1).replacen("<w:p/><w:sectPr", "<w:sectPr", 1);
    let xml = if xml.contains("xmlns:r=") { xml } else { xml.replacen("<w:document ", r#"<w:document xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#, 1) };
    pkg.set("word/document.xml", xml.into_bytes());
    pkg.to_bytes().unwrap()
}

#[test]
fn an_edit_from_one_chunk_across_another_works() {
    let bytes = with_two_chunks(b"<p>Alpha one</p>", b"<p>Beta two</p>");
    let mut doc = Document::open(&bytes).unwrap();
    let order = doc.render_all().order;
    let (a, after) = (order[1], *order.last().unwrap());
    // Bold from the middle of chunk A to the end of the text after chunk B.
    doc.apply_json(&json!({"op":"format_text","from":{"p":a,"o":3},"to":{"p":after,"o":2},"format":{"bold":true}}).to_string(), false).unwrap();
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert!(!xml.contains("w:altChunk"), "{xml}");
    assert!(xml.contains("Beta two") && xml.matches("<w:b/>").count() >= 3, "{xml}");
    assert!(doc.undo());
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert_eq!(xml.matches("w:altChunk").count(), 2, "undo brings both chunks back");
}

#[test]
fn a_delete_across_two_chunks_works() {
    let bytes = with_two_chunks(b"<p>Alpha one</p>", b"<p>Beta two</p>");
    let mut doc = Document::open(&bytes).unwrap();
    let order = doc.render_all().order;
    let (a, after) = (order[1], *order.last().unwrap());
    doc.apply_json(&json!({"op":"delete","from":{"p":a,"o":3},"to":{"p":after,"o":2}}).to_string(), false).unwrap();
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert!(xml.contains("Alp") && xml.contains("ter") && !xml.contains("Beta"), "{xml}");
}

#[test]
fn typing_after_a_footnote_in_a_docx_chunk_lands_where_the_caret_was() {
    let mut inner = Document::new_blank();
    let first = inner.render_all().order[0];
    inner.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Alpha beta"}).to_string(), false).unwrap();
    inner.apply_json(&json!({"op":"insert_footnote","at":{"p":first,"o":5},"text":"A note"}).to_string(), false).unwrap();
    let data = inner.save().unwrap();
    let bytes = with_chunk("chunk.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml", &data, false);
    let mut doc = Document::open(&bytes).unwrap();
    let inside = doc.render_all().order[1];
    // The note mark counts one: "Alpha", the mark, then offset 6 is right after the mark.
    doc.apply_json(&json!({"op":"insert_text","at":{"p":inside,"o":6},"text":"X"}).to_string(), false).unwrap();
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert!(xml.contains("X beta"), "X lands right after the note mark, not one character late: {xml}");
}

#[test]
fn what_a_chunk_shows_and_what_it_becomes_have_the_same_length() {
    let mut inner = Document::new_blank();
    let first = inner.render_all().order[0];
    inner.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Alpha beta"}).to_string(), false).unwrap();
    inner.apply_json(&json!({"op":"insert_footnote","at":{"p":first,"o":5},"text":"A note"}).to_string(), false).unwrap();
    let data = inner.save().unwrap();
    let bytes = with_chunk("chunk.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml", &data, false);
    let mut doc = Document::open(&bytes).unwrap();
    let r = doc.render_all();
    let inside = r.order[1];
    let v = serde_json::to_value(&r).unwrap();
    let len_of = |v: &serde_json::Value| -> usize { v.to_string().matches("\"k\":\"note\"").count() + "Alpha beta".len() };
    // The shown paragraph has the note mark in it, so its end is 11, as the real paragraph's is.
    assert_eq!(len_of(&v), 11, "{v}");
    doc.apply_json(&json!({"op":"insert_text","at":{"p":inside,"o":11},"text":"X"}).to_string(), false).unwrap();
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert!(xml.contains("betaX"), "{xml}");
}

const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

#[test]
fn copying_from_a_chunk_keeps_its_link_list_and_picture() {
    let html = format!(r#"<html><body><p>See <a href="https://example.com/chunk-link">the page</a></p><ul><li>North</li><li>South</li></ul><p><img src="data:image/png;base64,{PNG_1X1}" width="20" height="20"></p></body></html>"#);
    let bytes = with_chunk("afchunk.htm", "text/html", html.as_bytes(), false);
    let mut src = Document::open(&bytes).unwrap();
    let order = src.render_all().order;
    let (first, last) = (order[0], order[order.len() - 2]);
    let last_len = 1;
    // From the start of "Before" (outside the chunk) over the whole chunk, and from inside it only.
    for from in [(first, 0usize), (order[1], 0)] {
        let frag = src.copy_fragment(docxtor_core::Pos::new(from.0, from.1), docxtor_core::Pos::new(last, last_len)).expect("a fragment");
        let mut dest = Document::new_blank();
        let at = dest.render_all().order[0];
        dest.apply_json(&json!({"op":"insert_fragment","at":{"p":at,"o":0},"fragment":frag}).to_string(), false).unwrap();
        let saved = dest.save().unwrap();
        assert_valid(&saved);
        let pkg = Package::open(&saved).unwrap();
        let xml = part(&saved, "word/document.xml").unwrap();
        let rels = part(&saved, "word/_rels/document.xml.rels").unwrap();
        assert!(rels.contains("https://example.com/chunk-link") && xml.contains("w:hyperlink"), "the link: {rels}");
        assert!(xml.contains("w:numPr") && xml.contains("North") && xml.contains("South"), "the list: {xml}");
        assert!(xml.contains("<w:drawing") && pkg.parts.iter().any(|p| p.name.starts_with("word/media/")), "the picture: {xml}");
    }
}

#[test]
fn copying_a_range_over_a_chunk_and_text_around_it_joins_both_sources() {
    let html = format!(r#"<html><body><p><a href="https://example.com/in-chunk">chunk link</a></p><p><img src="data:image/png;base64,{PNG_1X1}" width="20" height="20"></p></body></html>"#);
    let bytes = with_chunk("afchunk.htm", "text/html", html.as_bytes(), false);
    let mut src = Document::open(&bytes).unwrap();
    // A link of the document's own, in the paragraph after the chunk.
    let order = src.render_all().order;
    let after = *order.last().unwrap();
    src.apply_json(&json!({"op":"set_link","from":{"p":after,"o":0},"to":{"p":after,"o":5},"url":"https://example.com/own-link"}).to_string(), false).unwrap();
    let order = src.render_all().order;
    let frag = src.copy_fragment(docxtor_core::Pos::new(order[1], 0), docxtor_core::Pos::new(*order.last().unwrap(), 5)).expect("a fragment");
    let mut dest = Document::new_blank();
    let at = dest.render_all().order[0];
    dest.apply_json(&json!({"op":"insert_fragment","at":{"p":at,"o":0},"fragment":frag}).to_string(), false).unwrap();
    let saved = dest.save().unwrap();
    assert_valid(&saved);
    let rels = part(&saved, "word/_rels/document.xml.rels").unwrap();
    assert!(rels.contains("https://example.com/in-chunk") && rels.contains("https://example.com/own-link"), "{rels}");
    assert!(part(&saved, "word/document.xml").unwrap().contains("<w:drawing"));
}

/// What the package says about the chunk named `name`: its relationship, its override, and its bytes.
fn chunk_traces(saved: &[u8], name: &str) -> (bool, bool, Option<Vec<u8>>) {
    let rels = part(saved, "word/_rels/document.xml.rels").unwrap();
    let types = part(saved, "[Content_Types].xml").unwrap();
    (rels.contains(&format!("Target=\"{name}\"")), types.contains(&format!("PartName=\"/word/{name}\"")), Package::open(saved).unwrap().get(&format!("word/{name}")).map(|d| d.to_vec()))
}

#[test]
fn editing_text_outside_a_chunk_writes_the_chunk_element_relationship_part_and_override_as_they_were() {
    let bytes = with_chunk("afchunk.htm", "text/html", HTML.as_bytes(), false);
    let mut doc = Document::open(&bytes).unwrap();
    let before = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":before,"o":0},"text":"Draft "}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    // The body was written out again (an edit), so the chunk went through the chunk-writing code.
    let xml = part(&saved, "word/document.xml").unwrap();
    let original = part(&bytes, "word/document.xml").unwrap();
    assert!(xml.contains("Draft Before") && xml.contains(r#"<w:altChunk r:id=""#), "{xml}");
    let rid = |x: &str| x.split(r#"<w:altChunk r:id=""#).nth(1).unwrap().split('"').next().unwrap().to_string();
    assert_eq!(rid(&xml), rid(&original));
    assert_eq!(chunk_traces(&saved, "afchunk.htm"), chunk_traces(&bytes, "afchunk.htm"));
    assert!(chunk_traces(&saved, "afchunk.htm").0 && chunk_traces(&saved, "afchunk.htm").1);
    assert_eq!(chunk_traces(&saved, "afchunk.htm").2.as_deref(), Some(HTML.as_bytes()), "the bytes are the chunk's own");
    // And reopening shows it again.
    assert!(shown(&mut Document::open(&saved).unwrap()).contains("Quarterly report"));
}

#[test]
fn a_chunk_in_a_table_cell_survives_an_edit_elsewhere_and_a_content_control_chunk_is_read() {
    let bytes = with_chunk("afchunk.htm", "text/html", b"<p>In the cell</p>", true);
    let mut doc = Document::open(&bytes).unwrap();
    let before = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":before,"o":0},"text":"Draft "}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    assert!(part(&saved, "word/document.xml").unwrap().contains("<w:altChunk"));
    assert_eq!(chunk_traces(&saved, "afchunk.htm"), chunk_traces(&bytes, "afchunk.htm"));

    // The same chunk inside a block-level content control.
    let sdt = |bytes: &[u8]| {
        let mut pkg = Package::open(bytes).unwrap();
        let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
        let (a, b) = (xml.find("<w:altChunk").unwrap(), xml.find("<w:altChunk").unwrap());
        let end = xml[b..].find("/>").unwrap() + b + 2;
        let chunk = &xml[a..end];
        let xml = xml.replacen(chunk, &format!("<w:sdt><w:sdtPr><w:alias w:val=\"Report\"/></w:sdtPr><w:sdtContent>{chunk}</w:sdtContent></w:sdt>"), 1);
        pkg.set("word/document.xml", xml.into_bytes());
        pkg.to_bytes().unwrap()
    };
    let bytes = sdt(&with_chunk("afchunk.htm", "text/html", b"<p>In the control</p>", false));
    let mut doc = Document::open(&bytes).unwrap();
    assert!(shown(&mut doc).contains("In the control"));
    let before = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":before,"o":0},"text":"Draft "}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let xml = part(&saved, "word/document.xml").unwrap();
    assert!(xml.contains("<w:sdt>") && xml.contains("<w:altChunk"), "{xml}");
    assert_eq!(chunk_traces(&saved, "afchunk.htm"), chunk_traces(&bytes, "afchunk.htm"));
    // Replace all reaches it there too, and the chunk becomes ordinary content.
    doc.apply_json(&json!({"op":"replace_text","find":"control","replace":"box","all":true}).to_string(), false).unwrap();
    let xml = part(&doc.save().unwrap(), "word/document.xml").unwrap();
    assert!(xml.contains("In the box") && !xml.contains("<w:altChunk"), "{xml}");
}

#[test]
fn expanding_one_of_two_elements_that_share_a_part_keeps_the_part() {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let rid = pkg.add_rel("word/document.xml", AF_CHUNK, "shared.htm", false);
    pkg.set("word/shared.htm", b"<p>Shared text</p>".to_vec());
    pkg.ensure_override("word/shared.htm", "text/html");
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let body = format!(r#"<w:body><w:altChunk r:id="{rid}"/><w:altChunk r:id="{rid}"/>"#);
    let xml = xml.replacen("<w:body>", &body, 1);
    let xml = if xml.contains("xmlns:r=") { xml } else { xml.replacen("<w:document ", r#"<w:document xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#, 1) };
    pkg.set("word/document.xml", xml.into_bytes());
    let bytes = pkg.to_bytes().unwrap();
    let mut doc = Document::open(&bytes).unwrap();
    let first = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"One: "}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let xml = part(&saved, "word/document.xml").unwrap();
    assert_eq!(xml.matches("<w:altChunk").count(), 1, "the other element is still a chunk: {xml}");
    assert!(xml.contains("One: Shared text"));
    let t = chunk_traces(&saved, "shared.htm");
    assert!(t.0 && t.1 && t.2.is_some(), "the shared part, its relationship and override stay: {t:?}");
    // Expanding the second one too lets the part go.
    let second = doc.render_all().order.iter().copied().find(|id| doc.alt_chunk_of(*id).is_some()).unwrap();
    doc.apply_json(&json!({"op":"insert_text","at":{"p":second,"o":0},"text":"Two: "}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert!(part(&saved, "word/document.xml").unwrap().contains("Two: Shared text"));
    assert_eq!(chunk_traces(&saved, "shared.htm"), (false, false, None));
}

#[test]
fn undo_after_a_save_brings_the_chunk_back_whole() {
    let bytes = with_chunk("afchunk.htm", "text/html", HTML.as_bytes(), false);
    let mut doc = Document::open(&bytes).unwrap();
    let inside = doc.render_all().order[1];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":inside,"o":0},"text":"Draft: "}).to_string(), false).unwrap();
    let saved = doc.save().unwrap();
    assert_eq!(chunk_traces(&saved, "afchunk.htm"), (false, false, None));
    assert!(doc.undo());
    let back = doc.save().unwrap();
    assert_valid(&back);
    assert!(part(&back, "word/document.xml").unwrap().contains("<w:altChunk"));
    assert_eq!(chunk_traces(&back, "afchunk.htm"), chunk_traces(&bytes, "afchunk.htm"));
    assert!(shown(&mut Document::open(&back).unwrap()).contains("Quarterly report"));
}

#[test]
fn a_docx_chunk_with_an_embedded_object_says_so_when_the_file_opens() {
    let mut inner = Document::new_blank();
    let mut pkg = Package::open(&inner.save().unwrap()).unwrap();
    let rid = pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject", "embeddings/object1.bin", false);
    pkg.set("word/embeddings/object1.bin", b"binary".to_vec());
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let body = format!(r#"<w:body><w:p><w:r><w:t>Text</w:t></w:r><w:r><w:object><o:OLEObject xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{rid}"/></w:object></w:r></w:p>"#);
    pkg.set("word/document.xml", xml.replacen("<w:body>", &body, 1).into_bytes());
    let data = pkg.to_bytes().unwrap();
    let bytes = with_chunk("chunk.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml", &data, false);
    let opened = docxtor_core::open::open(&bytes, "").unwrap();
    assert!(opened.report.has("chunk-objects"), "{:?}", opened.report.notes);
    // A plain chunk says nothing.
    let plain = docxtor_core::open::open(&with_chunk("afchunk.htm", "text/html", HTML.as_bytes(), false), "").unwrap();
    assert!(!plain.report.has("chunk-objects"));
}

#[test]
fn a_chunk_follows_a_change_to_the_documents_styles_and_undo_of_it() {
    let bytes = with_chunk("afchunk.htm", "text/html", b"<p>Chunk text</p>", false);
    let mut doc = Document::open(&bytes).unwrap();
    // The size a paragraph's first segment is drawn at, for "Before" (the document's) and the chunk's.
    let sizes = |doc: &mut Document| -> (f64, f64) {
        let v = serde_json::to_value(doc.render_all()).unwrap();
        let size = |text: &str| v["blocks"].as_array().unwrap().iter().find(|b| b["segs"][0]["x"] == text).unwrap()["segs"][0]["f"]["size"].as_f64().unwrap();
        (size("Before"), size("Chunk text"))
    };
    let (own, chunk) = sizes(&mut doc);
    assert_eq!(own, chunk, "both start at the same size");
    doc.apply_json(&json!({"op":"set_style_definition","style":"Normal","def":{"rpr":{"size":20.0}}}).to_string(), false).unwrap();
    let (own2, chunk2) = sizes(&mut doc);
    assert_eq!(own2, 20.0, "the document follows the style");
    assert_eq!(chunk2, own2, "so does the chunk");
    assert!(doc.undo());
    assert_eq!(sizes(&mut doc), (own, chunk), "and undo takes both back");
    // The chunk is still a chunk, with the same blocks (ids), and still saves as it came.
    assert_eq!(part(&doc.save().unwrap(), "word/afchunk.htm"), part(&bytes, "word/afchunk.htm"));
}

#[test]
fn clipboard_markers_in_a_chunk_or_html_file_do_not_cut_the_page() {
    let page = "<html><body><p>Intro</p><!--StartFragment--><p>Body</p><!--EndFragment--><p>Outro</p></body></html>";
    let mut doc = Document::open(&with_chunk("afchunk.htm", "text/html", page.as_bytes(), false)).unwrap();
    let r = shown(&mut doc);
    assert!(r.contains("Intro") && r.contains("Body") && r.contains("Outro"), "{r}");
    // The same for an .html file opened as a document.
    let opened = docxtor_core::open::open(page.as_bytes(), "page.html").unwrap();
    let mut d = opened.doc;
    let r = shown(&mut d);
    assert!(r.contains("Intro") && r.contains("Outro"), "{r}");
    // A paste still takes only the marked part.
    let frag = docxtor_core::html_import::html_to_fragment(page, &Default::default());
    assert!(frag.plain_text().contains("Body") && !frag.plain_text().contains("Outro"));
}

/// An EMF of one gradient rectangle between two vertices: (x, y, red, green, blue) each; `mode` 0 across, 1 down.
fn emf_gradient_rect(v0: (i32, i32, u16, u16, u16), v1: (i32, i32, u16, u16, u16), mode: i32) -> Vec<u8> {
    let mut b = Vec::new();
    let put = |b: &mut Vec<u8>, v: i32| b.extend_from_slice(&v.to_le_bytes());
    let put16 = |b: &mut Vec<u8>, v: u16| b.extend_from_slice(&v.to_le_bytes());
    put(&mut b, 1);
    put(&mut b, 88);
    for v in [0, 0, 99, 99, 0, 0, 2500, 2500] {
        put(&mut b, v);
    }
    put(&mut b, 0x464D_4520);
    while b.len() < 88 {
        put(&mut b, 0);
    }
    put(&mut b, 118);
    put(&mut b, 76);
    for v in [0, 0, 99, 99] {
        put(&mut b, v);
    }
    put(&mut b, 2);
    put(&mut b, 1);
    put(&mut b, mode);
    for v in [v0, v1] {
        put(&mut b, v.0);
        put(&mut b, v.1);
        for c in [v.2, v.3, v.4, 0] {
            put16(&mut b, c);
        }
    }
    put(&mut b, 0);
    put(&mut b, 1);
    put(&mut b, 14);
    put(&mut b, 20);
    b
}

#[test]
fn a_gradient_rectangle_shades_by_where_its_vertices_are_not_by_their_order() {
    let red = (0xff00, 0, 0);
    let blue = (0, 0, 0xff00);
    let stops = |svg: &str| -> Vec<String> { svg.split("stop-color=\"").skip(1).map(|s| s.split('"').next().unwrap().to_lowercase()).collect() };
    // Across: the left vertex's colour comes first whichever vertex is named first.
    let a = docxtor_core::imgconv::emf_to_svg(&emf_gradient_rect((0, 0, blue.0, blue.1, blue.2), (100, 100, red.0, red.1, red.2), 0)).unwrap();
    let b = docxtor_core::imgconv::emf_to_svg(&emf_gradient_rect((100, 0, red.0, red.1, red.2), (0, 100, blue.0, blue.1, blue.2), 0)).unwrap();
    assert_eq!(stops(&a), stops(&b), "{a}\n{b}");
    assert_eq!(stops(&a).first().map(String::as_str), Some("#0000ff"));
    // Down: the top vertex's colour comes first.
    let c = docxtor_core::imgconv::emf_to_svg(&emf_gradient_rect((0, 100, red.0, red.1, red.2), (100, 0, blue.0, blue.1, blue.2), 1)).unwrap();
    assert_eq!(stops(&c).first().map(String::as_str), Some("#0000ff"), "{c}");
}
