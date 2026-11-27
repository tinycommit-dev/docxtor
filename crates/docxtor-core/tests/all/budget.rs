//! The input budget (`docxtor_core::budget`): one limit every file reader spends from, so no file from outside can
//! crash, freeze or use up the engine.
//!
//! Every shape of deep nesting a file can have is opened, drawn, edited, saved, reopened and dropped on a thread
//! with a 1 MB stack, the size of the WebAssembly stack; a file past the limits is refused with one plain message.

use docxtor_core::budget::{Budget, Limits, NEST_LIMIT, XML_DEPTH};
use docxtor_core::package::Package;
use docxtor_core::Document;
use serde_json::json;
use std::io::Write;
use std::time::{Duration, Instant};

const AF_CHUNK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/aFChunk";
const DOCX_TYPE: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const TOO_BIG: &str = "This file is too large or too deeply nested to open.";

/// Run `f` on a thread with a 1 MB stack, the size of the engine's stack in the browser. A stack overflow stops
/// the whole test run, which is how it shows.
fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(1 << 20).spawn(f).unwrap().join().unwrap()
}

/// A document whose body is `body` (then the section properties).
fn docx_with_body(body: &str) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:body>", &format!("<w:body>{body}"), 1).replacen("<w:p/><w:sectPr", "<w:sectPr", 1);
    let xml = if xml.contains("xmlns:r=") { xml } else { xml.replacen("<w:document ", r#"<w:document xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#, 1) };
    let xml = xml.replacen("<w:document ", r#"<w:document xmlns:v="urn:schemas-microsoft-com:vml" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" "#, 1);
    pkg.set("word/document.xml", xml.into_bytes());
    pkg.to_bytes().unwrap()
}

const LEAF: &str = r#"<w:p><w:r><w:t>deep</w:t></w:r></w:p>"#;

fn tables(n: usize) -> String {
    let mut s = LEAF.to_string();
    for _ in 0..n {
        s = format!(r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr>{s}<w:p/></w:tc></w:tr></w:tbl>"#);
    }
    s
}

fn controls(n: usize) -> String {
    (0..n).fold(LEAF.to_string(), |s, _| format!("<w:sdt><w:sdtPr/><w:sdtContent>{s}</w:sdtContent></w:sdt>"))
}

fn custom_xml(n: usize) -> String {
    (0..n).fold(LEAF.to_string(), |s, _| format!(r#"<w:customXml w:element="x">{s}</w:customXml>"#))
}

/// One paragraph whose text sits `n` levels down in tracked insertions.
fn insertions(n: usize) -> String {
    let open = r#"<w:ins w:id="1" w:author="a" w:date="2020-01-01T00:00:00Z">"#.repeat(n);
    format!("<w:p>{open}<w:r><w:t>deep</w:t></w:r>{}</w:p>", "</w:ins>".repeat(n))
}

fn links(n: usize) -> String {
    format!("<w:p>{}<w:r><w:t>deep</w:t></w:r>{}</w:p>", "<w:hyperlink>".repeat(n), "</w:hyperlink>".repeat(n))
}

fn inline_controls(n: usize) -> String {
    format!("<w:p>{}<w:r><w:t>deep</w:t></w:r>{}</w:p>", "<w:sdt><w:sdtPr/><w:sdtContent>".repeat(n), "</w:sdtContent></w:sdt>".repeat(n))
}

/// Elements nobody knows, `n` deep, in a run (kept as they are) and, in math, equations in equations.
fn unknown_elements(n: usize) -> String {
    format!("<w:p><w:r>{}{}</w:r></w:p>", "<w:foo>".repeat(n), "</w:foo>".repeat(n))
}

fn unknown_blocks(n: usize) -> String {
    format!("{}{}{LEAF}", "<w:foo>".repeat(n), "</w:foo>".repeat(n))
}

fn equations(n: usize) -> String {
    format!("<w:p><m:oMath>{}<m:r><m:t>x</m:t></m:r>{}</m:oMath></w:p>", "<m:sSup><m:e>".repeat(n), "</m:e></m:sSup>".repeat(n))
}

fn picture_groups(n: usize) -> String {
    format!("<w:p><w:r><w:pict>{}<v:rect/>{}</w:pict></w:r></w:p>", "<v:group>".repeat(n), "</v:group>".repeat(n))
}

/// A text box in a text box, `n` times.
fn text_boxes(n: usize) -> String {
    (0..n).fold(LEAF.to_string(), |s, _| format!("<w:p><w:r><w:pict><v:shape><v:textbox><w:txbxContent>{s}</w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>"))
}

/// Open, draw, edit, undo, save, reopen and drop: everything that walks the blocks.
fn use_it_all(bytes: Vec<u8>) {
    let mut doc = Document::open(&bytes).unwrap_or_else(|e| panic!("opens: {e}"));
    let _ = doc.render_all();
    // The text sits in the deepest block: try the paragraph ids in turn until one takes the edit.
    let edited = (1..400u32).any(|p| doc.apply_json(&json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Hi "}).to_string(), false).is_ok());
    assert!(edited, "some paragraph takes an edit");
    let saved = doc.save().unwrap();
    assert!(doc.undo());
    let again = doc.save().unwrap();
    let mut reopened = Document::open(&saved).unwrap();
    reopened.render_all();
    drop(reopened);
    drop(Document::open(&again).unwrap());
    drop(doc);
}

// ---------------------------------------------------------------------------
// Depth: a file within the limits is safe on a 1 MB stack, a file past them is refused
// ---------------------------------------------------------------------------

#[test]
fn deepest_allowed_tables_open_draw_edit_and_save_on_a_one_megabyte_stack() {
    let bytes = docx_with_body(&tables(NEST_LIMIT / 3));
    on_small_stack(move || use_it_all(bytes));
}

#[test]
fn deepest_allowed_content_controls_custom_xml_and_text_boxes_work_on_a_one_megabyte_stack() {
    for body in [controls(NEST_LIMIT / 2), custom_xml(NEST_LIMIT), text_boxes(NEST_LIMIT / 12)] {
        let bytes = docx_with_body(&body);
        on_small_stack(move || use_it_all(bytes));
    }
}

#[test]
fn deepest_allowed_inline_nesting_works_on_a_one_megabyte_stack() {
    for body in [insertions(NEST_LIMIT / 2), links(NEST_LIMIT / 2), inline_controls(NEST_LIMIT / 2)] {
        let bytes = docx_with_body(&body);
        on_small_stack(move || use_it_all(bytes));
    }
}

#[test]
fn elements_the_engine_only_keeps_nest_to_the_element_limit_on_a_one_megabyte_stack() {
    let n = XML_DEPTH - 12;
    for body in [unknown_elements(n), unknown_blocks(n), equations(n / 2), picture_groups(n)] {
        let bytes = docx_with_body(&body);
        on_small_stack(move || use_it_all(bytes));
    }
}

#[test]
fn nesting_past_the_limit_is_refused_with_one_plain_message() {
    for body in [tables(NEST_LIMIT / 3 + 1), controls(NEST_LIMIT / 2 + 1), custom_xml(NEST_LIMIT + 1), text_boxes(NEST_LIMIT / 12 + 1), insertions(NEST_LIMIT / 2 + 1), links(NEST_LIMIT / 2 + 1)] {
        let bytes = docx_with_body(&body);
        let e = on_small_stack(move || docxtor_core::open::open(&bytes, "deep.docx").err().expect("refused"));
        assert_eq!(e.code, "too-big");
        assert_eq!(e.message, TOO_BIG);
    }
}

#[test]
fn elements_nested_past_the_limit_are_refused_and_a_hundred_thousand_levels_do_not_crash() {
    for body in [unknown_elements(XML_DEPTH + 1), unknown_elements(100_000), insertions(100_000), tables(100_000 / 4)] {
        let bytes = docx_with_body(&body);
        let e = on_small_stack(move || docxtor_core::open::open(&bytes, "deep.docx").err().expect("refused"));
        assert_eq!(e.code, "too-big");
        assert_eq!(e.message, TOO_BIG);
    }
}

#[test]
fn document_open_says_the_same_plain_thing() {
    let bytes = docx_with_body(&controls(NEST_LIMIT));
    assert_eq!(Document::open(&bytes).err().unwrap().to_string(), TOO_BIG);
}

#[test]
fn a_deep_side_part_ends_the_open_too_not_a_silent_skip() {
    // The styles part nested too deeply: the open must not quietly go on without it.
    let mut pkg = Package::open(&docx_with_body(LEAF)).unwrap();
    let deep = format!("<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">{}{}</w:styles>", "<w:x>".repeat(XML_DEPTH + 10), "</w:x>".repeat(XML_DEPTH + 10));
    pkg.set("word/styles.xml", deep.into_bytes());
    let bytes = pkg.to_bytes().unwrap();
    assert_eq!(docxtor_core::open::open(&bytes, "").err().unwrap().code, "too-big");
}

// ---------------------------------------------------------------------------
// Totals: bytes inflated, parts, work
// ---------------------------------------------------------------------------

fn zip_of(parts: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (n, d) in parts {
        w.start_file(n.as_str(), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
        w.write_all(d).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn with_extra_parts(extra: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let pkg = Package::open(&docx_with_body(LEAF)).unwrap();
    let mut parts: Vec<(String, Vec<u8>)> = pkg.parts.iter().map(|p| (p.name.clone(), p.data.clone())).collect();
    parts.extend(extra);
    zip_of(&parts)
}

#[test]
fn a_zip_that_inflates_past_the_budget_is_refused_whatever_its_size() {
    // 8 MB of zeros is a few KB zipped. The budget here allows 1 MB in all.
    let bytes = with_extra_parts(vec![("word/media/bomb.bin".into(), vec![0u8; 8 << 20])]);
    assert!(bytes.len() < 100_000, "the file is small: {}", bytes.len());
    let small = Budget::new(Limits { inflated: 1 << 20, ..Limits::file() });
    let e = docxtor_core::open::open_with(&bytes, "", &small).err().expect("refused");
    assert_eq!((e.code, e.message.as_str()), ("too-big", TOO_BIG));
    // The same file, with room for it, opens.
    assert!(docxtor_core::open::open_with(&bytes, "", &Budget::new(Limits { inflated: 64 << 20, ..Limits::file() })).is_ok());
}

#[test]
fn the_budget_counts_every_part_not_only_the_biggest() {
    let extra = (0..300).map(|i| (format!("word/media/p{i}.bin"), vec![b'x'; 4096])).collect();
    let bytes = with_extra_parts(extra);
    let few_parts = Budget::new(Limits { parts: 100, ..Limits::file() });
    assert_eq!(docxtor_core::open::open_with(&bytes, "", &few_parts).err().unwrap().code, "too-big");
    let little_room = Budget::new(Limits { inflated: 300 * 1024, ..Limits::file() });
    assert_eq!(docxtor_core::open::open_with(&bytes, "", &little_room).err().unwrap().code, "too-big");
    assert!(docxtor_core::open::open(&bytes, "").is_ok());
}

#[test]
fn package_open_spends_from_the_budget_too() {
    let bytes = with_extra_parts(vec![("word/media/bomb.bin".into(), vec![0u8; 8 << 20])]);
    assert!(Package::open_with(&bytes, &Budget::new(Limits { inflated: 1 << 20, ..Limits::file() })).is_err());
    assert!(Package::open(&bytes).is_ok());
}

#[test]
fn work_steps_run_out_with_the_same_error() {
    let b = Budget::new(Limits { work: 10, ..Limits::file() });
    assert!(b.spend_work(5).is_ok());
    assert!(b.spend_work(6).is_err());
    assert!(b.is_out() && b.check().is_err());
    assert_eq!(b.check().unwrap_err().to_string(), TOO_BIG);
    let tiny = Budget::new(Limits { work: 100, ..Limits::file() });
    let bytes = docx_with_body(LEAF);
    assert_eq!(docxtor_core::open::open_with(&bytes, "", &tiny).err().unwrap().code, "too-big");
}

// ---------------------------------------------------------------------------
// Real files still open
// ---------------------------------------------------------------------------

fn repo_file(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

#[test]
fn every_fixture_opens_inside_the_default_budget() {
    let mut n = 0;
    for e in std::fs::read_dir(repo_file("fixtures")).unwrap().flatten() {
        let path = e.path();
        if path.extension().is_some_and(|x| x == "docx") {
            let bytes = std::fs::read(&path).unwrap();
            docxtor_core::open::open(&bytes, "").unwrap_or_else(|e| panic!("{}: {}", path.display(), e.message));
            n += 1;
        }
    }
    assert!(n >= 4, "fixtures found: {n}");
}

#[test]
fn a_long_document_uses_a_small_part_of_the_budget() {
    let bytes = std::fs::read(repo_file("fixtures/long.docx")).unwrap();
    let budget = Budget::default();
    let opened = docxtor_core::open::open_with(&bytes, "", &budget).unwrap();
    drop(opened);
    // Well under a thousandth of what a file may spend.
    assert!(budget.limits().inflated > 1000 * bytes.len() as u64 * 10);
}

// ---------------------------------------------------------------------------
// Content chunks 1: a .docx chunk opens its own chunks with no depth limit
// ---------------------------------------------------------------------------

/// A document with `n` copies of one chunk element naming a .docx part that holds `data`.
fn with_docx_chunks(data: &[u8], n: usize) -> Vec<u8> {
    let mut pkg = Package::open(&docx_with_body("")).unwrap();
    let rid = pkg.add_rel("word/document.xml", AF_CHUNK, "inner.docx", false);
    pkg.set("word/inner.docx", data.to_vec());
    pkg.ensure_override("word/inner.docx", DOCX_TYPE);
    let chunks = format!(r#"<w:p><w:r><w:t>Before</w:t></w:r></w:p>{}"#, format!(r#"<w:altChunk r:id="{rid}"/>"#).repeat(n));
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    pkg.set("word/document.xml", xml.replacen("<w:body>", &format!("<w:body>{chunks}"), 1).into_bytes());
    pkg.to_bytes().unwrap()
}

fn shown(doc: &mut Document) -> String {
    serde_json::to_string(&doc.render_all()).unwrap()
}

#[test]
fn a_docx_inside_a_docx_inside_a_docx_stops_at_a_small_depth_and_opens() {
    let mut bytes = docx_with_body(LEAF);
    for _ in 0..40 {
        bytes = with_docx_chunks(&bytes, 1);
    }
    let started = Instant::now();
    let seen = on_small_stack(move || {
        let mut doc = Document::open(&bytes).expect("opens, the deepest levels stay placeholders");
        let seen = shown(&mut doc).matches("Before").count();
        let saved = doc.save().unwrap();
        drop(Document::open(&saved).unwrap());
        seen
    });
    assert!(started.elapsed() < Duration::from_secs(20));
    assert!((2..=3).contains(&seen), "levels shown: {seen}");
}

#[test]
fn two_hundred_chunks_naming_one_part_four_levels_deep_do_not_open_billions_of_documents() {
    let mut bytes = docx_with_body(LEAF);
    for _ in 0..4 {
        bytes = with_docx_chunks(&bytes, 200);
    }
    let started = Instant::now();
    let opened = on_small_stack(move || Document::open(&bytes).is_ok());
    assert!(opened);
    assert!(started.elapsed() < Duration::from_secs(30), "took {:?}", started.elapsed());
}

#[test]
fn chunks_past_the_count_limit_stay_placeholders() {
    let inner = docx_with_body(LEAF);
    let bytes = with_docx_chunks(&inner, 50);
    let few = Budget::new(Limits { chunks: 10, ..Limits::file() });
    let mut doc = docxtor_core::open::open_with(&bytes, "", &few).unwrap().doc;
    assert_eq!(shown(&mut doc).matches("deep").count(), 10);
}

// ---------------------------------------------------------------------------
// Content chunks 3: EMF gradient records not clamped to the record's length
// ---------------------------------------------------------------------------

fn emf_with_gradients(records: usize, n_tri: u32) -> Vec<u8> {
    let mut b = Vec::new();
    let put = |b: &mut Vec<u8>, v: i32| b.extend_from_slice(&v.to_le_bytes());
    // Header: type 1, size 88, bounds, frame, " EMF", then zeros to 88 bytes.
    put(&mut b, 1);
    put(&mut b, 88);
    for v in [0, 0, 99, 99, 0, 0, 2500, 2500] {
        put(&mut b, v);
    }
    put(&mut b, 0x464D_4520);
    while b.len() < 88 {
        put(&mut b, 0);
    }
    for _ in 0..records {
        // EMR_GRADIENTFILL of 52 bytes: bounds, nVer = 1, nTri as given, mode 0, and one vertex.
        put(&mut b, 118);
        put(&mut b, 52);
        for _ in 0..4 {
            put(&mut b, 0);
        }
        put(&mut b, 1);
        put(&mut b, n_tri as i32);
        put(&mut b, 0);
        for _ in 0..4 {
            put(&mut b, 0);
        }
    }
    put(&mut b, 14);
    put(&mut b, 20);
    while b.len() % 4 != 0 {
        b.push(0);
    }
    b
}

#[test]
fn a_tiny_emf_gradient_record_does_not_make_megabytes_of_svg() {
    // One record says 100,000 triangles but holds none: it used to give about 19 MB of gradient definitions.
    let emf = emf_with_gradients(2, 100_000);
    assert!(emf.len() < 300);
    let svg = docxtor_core::imgconv::emf_to_svg(&emf).unwrap_or_default();
    assert!(svg.len() < 100_000, "svg of {} bytes", svg.len());
}

#[test]
fn many_such_gradient_records_stay_small_too() {
    let emf = emf_with_gradients(1000, 100_000);
    let svg = docxtor_core::imgconv::emf_to_svg(&emf).unwrap_or_default();
    assert!(svg.len() < 1_000_000, "svg of {} bytes", svg.len());
}

// ---------------------------------------------------------------------------
// Tabs 9: a crafted tabs part makes opening take cubic time
// ---------------------------------------------------------------------------

fn tabs_part(n: usize) -> String {
    let mut s = String::from(r#"<ft:tabs xmlns:ft="https://docxtor.tinycommit.dev/ns/tabs/1" version="1" next="0">"#);
    for i in 0..n / 2 {
        s.push_str(&format!(r#"<ft:tab id="a{i}"/>"#));
    }
    for i in 0..n / 2 {
        s.push_str(&format!(r#"<ft:tab id="b{i}" parent="a{}"/>"#, n / 2 - 1));
    }
    s.push_str("</ft:tabs>");
    s
}

#[test]
fn a_crafted_tabs_part_opens_in_linear_time() {
    let mut pkg = Package::open(&docx_with_body(LEAF)).unwrap();
    pkg.set("customXml/item1.xml", tabs_part(20_000).into_bytes());
    let bytes = pkg.to_bytes().unwrap();
    let started = Instant::now();
    let doc = Document::open(&bytes).unwrap();
    drop(doc);
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
}

#[test]
fn tabs_read_from_a_file_are_capped_and_depths_are_linear() {
    let doc = docxtor_core::xml::parse(tabs_part(20_000).as_bytes()).unwrap();
    let started = Instant::now();
    let (list, _) = docxtor_core::tabs::from_xml(&doc).unwrap();
    assert!(list.tabs.len() <= Limits::file().tabs);
    let depths = list.depths();
    assert_eq!(depths.len(), list.tabs.len());
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn tab_names_and_icons_from_a_file_are_cut_to_the_length_ops_allow() {
    let long = "x".repeat(5000);
    let part = format!(r#"<ft:tabs xmlns:ft="https://docxtor.tinycommit.dev/ns/tabs/1"><ft:tab id="t1" name="{long}" icon="{long}"/></ft:tabs>"#);
    let (list, _) = docxtor_core::tabs::from_xml(&docxtor_core::xml::parse(part.as_bytes()).unwrap()).unwrap();
    assert!(list.tabs[0].name.chars().count() <= docxtor_core::tabs::MAX_NAME);
    assert!(list.tabs[0].icon.as_ref().unwrap().chars().count() <= 16);
}

#[test]
fn an_edit_that_would_nest_tables_past_the_limit_is_refused_and_leaves_the_document_as_it_was() {
    // A file holds as many tables in tables as it may; a table put in the innermost cell would be one too many, and a
    // file like that would save but not open again.
    let at_limit = NEST_LIMIT / 3;
    on_small_stack(move || {
        let mut doc = Document::open(&docx_with_body(&tables(at_limit))).unwrap();
        let hit = doc.find("deep", &Default::default()).unwrap().hits[0].clone();
        let before = doc.save().unwrap();
        let op = json!({"op":"insert_table","after":hit.p,"rows":1,"cols":1});
        let err = doc.apply_json(&op.to_string(), false).unwrap_err().to_string();
        assert!(err.contains("nested that deeply"), "{err}");
        assert_eq!(doc.save().unwrap(), before, "nothing changed");
        // One level less deep is fine.
        let mut shallow = Document::open(&docx_with_body(&tables(at_limit - 1))).unwrap();
        let hit = shallow.find("deep", &Default::default()).unwrap().hits[0].clone();
        shallow.apply_json(&json!({"op":"insert_table","after":hit.p,"rows":1,"cols":1}).to_string(), false).unwrap();
    });
}
