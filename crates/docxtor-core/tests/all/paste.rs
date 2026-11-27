//! Copy and paste of rich content: the Docxtor fragment, between documents and within one.

use docxtor_core::fragment::{Fragment, PasteMode};
use docxtor_core::model::{find_para, walk_paragraphs};
use docxtor_core::{package::Package, Document, Op, Pos};
use serde_json::json;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}"))
}

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn paras(doc: &Document) -> Vec<u32> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.id));
    v
}

fn saved(doc: &mut Document, part: &str) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    String::from_utf8(pkg.get(part).unwrap().to_vec()).unwrap()
}

fn paste(doc: &mut Document, at: Pos, f: &Fragment, mode: PasteMode) {
    doc.apply(&Op::InsertFragment { at, fragment: f.clone(), mode }, false).unwrap();
}

#[test]
fn copying_the_whole_rich_document_and_pasting_it_into_a_blank_one_keeps_everything() {
    let mut src = Document::open(&fixture("rich.docx")).unwrap();
    let ids = paras(&src);
    let last = *ids.last().unwrap();
    let last_len = find_para(&src.body, last).unwrap().len16();
    let frag = src.copy_fragment(Pos::new(ids[0], 0), Pos::new(last, last_len)).expect("a fragment");
    assert!(frag.body.contains("<w:tbl>") || frag.body.contains("<w:tbl "), "table travels");
    assert!(!frag.styles.is_empty(), "styles travel");
    assert!(!frag.numbering.is_empty(), "lists travel");
    assert!(!frag.media.is_empty(), "the picture travels");
    assert!(frag.block);

    // Through JSON, as the clipboard carries it.
    let json = serde_json::to_string(&frag).unwrap();
    let frag = Fragment::from_json(&json).unwrap();

    let mut dst = Document::new_blank();
    let first = dst.render_all().order[0];
    paste(&mut dst, Pos::new(first, 0), &frag, PasteMode::Keep);
    let want = texts(&src);
    let got = texts(&dst);
    assert_eq!(got.iter().filter(|t| !t.is_empty()).collect::<Vec<_>>(), want.iter().filter(|t| !t.is_empty()).collect::<Vec<_>>());

    let doc_xml = saved(&mut dst, "word/document.xml");
    assert!(doc_xml.contains("<w:tbl>"));
    assert!(doc_xml.contains("w:drawing"));
    let styles = saved(&mut dst, "word/styles.xml");
    assert!(styles.contains("Heading1"), "{styles}");
    let numbering = saved(&mut dst, "word/numbering.xml");
    assert!(numbering.contains("w:abstractNum"));
    // The picture is a part of the new package, related from the document.
    let bytes = dst.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    assert!(pkg.parts.iter().any(|p| p.name.starts_with("word/media/")));
    // The blocks line up with the source.
    let a = serde_json::to_value(src.render_all()).unwrap();
    let b = serde_json::to_value(dst.render_all()).unwrap();
    let kinds = |v: &serde_json::Value| v["blocks"].as_array().unwrap().iter().filter(|b| b["t"] != "para" || b["len"].as_u64().unwrap_or(1) > 0).map(|b| b["t"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(kinds(&a), kinds(&b));

    // One undo step takes the paste away.
    assert!(dst.undo());
    assert_eq!(texts(&dst).iter().filter(|t| !t.is_empty()).count(), 0);
}

#[test]
fn a_piece_of_a_paragraph_pastes_inline_with_its_formatting() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Hello world"}).to_string(), false).unwrap();
    doc.apply_json(&json!({"op":"format_text","from":{"p":first,"o":6},"to":{"p":first,"o":11},"format":{"bold":true,"color":"FF0000"}}).to_string(), false).unwrap();
    let frag = doc.copy_fragment(Pos::new(first, 6), Pos::new(first, 11)).unwrap();
    assert!(!frag.block);
    assert!(frag.body.contains("<w:b/>"), "{}", frag.body);
    paste(&mut doc, Pos::new(first, 0), &frag, PasteMode::Keep);
    assert_eq!(texts(&doc)[0], "worldHello world");
    let f = doc.format_at(Pos::new(first, 2)).unwrap();
    assert_eq!(f.run.bold, Some(true));
    assert_eq!(f.run.color.as_deref(), Some("FF0000"));
    assert_ne!(doc.format_at(Pos::new(first, 7)).unwrap().run.bold, Some(true));
}

#[test]
fn merge_formatting_drops_fonts_and_colours_but_keeps_bold_and_lists() {
    let src = Document::open(&fixture("rich.docx")).unwrap();
    let ids = paras(&src);
    let frag = src.copy_fragment(Pos::new(ids[0], 0), Pos::new(*ids.last().unwrap(), 1)).unwrap();
    let mut dst = Document::new_blank();
    let first = dst.render_all().order[0];
    paste(&mut dst, Pos::new(first, 0), &frag, PasteMode::Merge);
    let xml = saved(&mut dst, "word/document.xml");
    assert!(!xml.contains("<w:color"), "no colours");
    assert!(!xml.contains("w:rFonts"), "no fonts");
    assert!(!xml.contains("w:pStyle w:val=\"Heading1\""), "no heading style");
    assert!(xml.contains("<w:b/>"), "bold stays");
    assert!(xml.contains("w:numPr"), "lists stay");
}

#[test]
fn copying_and_pasting_within_a_document_continues_the_list() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_markdown","after":first,"markdown":"1. one\n2. two\n3. three"}).to_string(), false).unwrap();
    let ps = paras(&doc);
    let (a, b) = (ps[1], ps[2]);
    let before = doc.numbering.doc.as_ref().unwrap().root.children.len();
    let last = *ps.last().unwrap();
    let len = find_para(&doc.body, last).unwrap().len16();
    let frag = doc.copy_fragment(Pos::new(a, 0), Pos::new(b, 3)).unwrap();
    assert!(frag.reuse_lists);
    paste(&mut doc, Pos::new(last, len), &frag, PasteMode::Keep);
    let after = doc.numbering.doc.as_ref().unwrap().root.children.len();
    assert_eq!(before, after, "no new list definition: the pasted items continue the list");
    // The pasted "one" joins the text it was pasted into, "two" starts the next item.
    let t: Vec<String> = texts(&doc).into_iter().filter(|t| !t.is_empty()).collect();
    assert_eq!(t, ["one", "two", "threeone", "two"]);
}

#[test]
fn undo_and_redo_of_a_paste_restore_styles_too() {
    let src = Document::open(&fixture("rich.docx")).unwrap();
    let ids = paras(&src);
    let frag = src.copy_fragment(Pos::new(ids[0], 0), Pos::new(ids[1], 5)).unwrap();
    let mut dst = Document::new_blank();
    let first = dst.render_all().order[0];
    paste(&mut dst, Pos::new(first, 0), &frag, PasteMode::Keep);
    assert!(texts(&dst).iter().any(|t| !t.is_empty()));
    assert!(dst.undo());
    assert!(dst.redo());
    assert!(texts(&dst).iter().any(|t| !t.is_empty()));
    saved(&mut dst, "word/document.xml");
}

#[test]
fn footnotes_equations_and_links_travel() {
    let mut src = Document::new_blank();
    let first = src.render_all().order[0];
    src.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"See this and that."}).to_string(), false).unwrap();
    src.apply_json(&json!({"op":"insert_footnote","at":{"p":first,"o":8},"text":"The note text"}).to_string(), false).unwrap();
    src.apply_json(&json!({"op":"set_link","from":{"p":first,"o":0},"to":{"p":first,"o":3},"url":"https://example.com/see"}).to_string(), false).unwrap();
    src.apply_json(&json!({"op":"insert_equation","at":{"p":first,"o":4},"latex":"x^2","display":false}).to_string(), false).unwrap();
    let len = find_para(&src.body, first).unwrap().len16();
    let frag = src.copy_fragment(Pos::new(first, 0), Pos::new(first, len)).unwrap();
    assert_eq!(frag.notes.len(), 1);
    assert_eq!(frag.links.values().collect::<Vec<_>>(), ["https://example.com/see"]);
    assert!(frag.body.contains("m:oMath"));

    let mut dst = Document::new_blank();
    let at = dst.render_all().order[0];
    paste(&mut dst, Pos::new(at, 0), &frag, PasteMode::Keep);
    assert_eq!(dst.stories.iter().filter(|s| s.kind == docxtor_core::stories::StoryKind::Footnote).count(), 1);
    let notes = saved(&mut dst, "word/footnotes.xml");
    assert!(notes.contains("The note text"));
    let rels = saved(&mut dst, "word/_rels/document.xml.rels");
    assert!(rels.contains("https://example.com/see"));
    let body = saved(&mut dst, "word/document.xml");
    assert!(body.contains("m:oMath") && body.contains("w:footnoteReference") && body.contains("w:hyperlink"));
    // Pasting twice gives two notes with their own ids.
    paste(&mut dst, Pos::new(at, 0), &frag, PasteMode::Keep);
    assert_eq!(dst.stories.iter().filter(|s| s.kind == docxtor_core::stories::StoryKind::Footnote).count(), 2);
    saved(&mut dst, "word/footnotes.xml");
    assert!(dst.undo());
    assert_eq!(dst.stories.iter().filter(|s| s.kind == docxtor_core::stories::StoryKind::Footnote).count(), 1);
}

#[test]
fn a_range_across_table_cells_copies_whole_cells_and_a_range_inside_one_cell_copies_its_text() {
    let mut src = Document::open(&fixture("rich.docx")).unwrap();
    let order = src.render_all().order;
    let _ = order;
    // rich.docx: paragraph 16 is the first cell of the table, 17 the one next to it.
    let one = src.copy_fragment(Pos::new(16, 0), Pos::new(16, 3)).unwrap();
    assert!(!one.body.contains("<w:tbl"), "inside one cell it is just text");
    assert_eq!(one.plain_text().len(), 3);
    let row = src.copy_fragment(Pos::new(16, 0), Pos::new(17, 1)).unwrap();
    assert!(row.body.contains("<w:tbl"));
    assert_eq!(row.body.matches("<w:tr>").count() + row.body.matches("<w:tr ").count(), 1);
    let mut dst = Document::new_blank();
    let at = dst.render_all().order[0];
    paste(&mut dst, Pos::new(at, 0), &row, PasteMode::Keep);
    let xml = saved(&mut dst, "word/document.xml");
    assert!(xml.contains("<w:tbl>") && xml.contains("<w:tblGrid>"));
}

#[test]
fn pasting_into_a_header_keeps_formatting_and_drops_notes() {
    let mut src = Document::new_blank();
    let first = src.render_all().order[0];
    src.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Bold words"}).to_string(), false).unwrap();
    src.apply_json(&json!({"op":"format_text","from":{"p":first,"o":0},"to":{"p":first,"o":4},"format":{"bold":true}}).to_string(), false).unwrap();
    let frag = src.copy_fragment(Pos::new(first, 0), Pos::new(first, 10)).unwrap();
    let mut dst = Document::new_blank();
    dst.apply_json(&json!({"op":"add_header_footer","kind":"header","variant":"default"}).to_string(), false).unwrap();
    let head = dst.stories.iter().find(|s| s.kind == docxtor_core::stories::StoryKind::Header).map(|s| s.blocks.iter().map(|b| b.id()).next().unwrap()).unwrap();
    paste(&mut dst, Pos::new(head, 0), &frag, PasteMode::Keep);
    let part = saved(&mut dst, "word/header1.xml");
    assert!(part.contains(">Bold<") && part.contains("<w:b/>"), "{part}");
}

#[test]
fn pasting_keeps_the_sources_default_font_and_size_on_plain_runs_but_not_over_their_own() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Start"}).to_string(), false).unwrap();
    let dest = doc.format_at(Pos::new(first, 0)).unwrap().run;
    assert_ne!(dest.font.as_deref(), Some("Courier New"));
    assert_ne!(dest.size, Some(8.5));

    // Plain runs sit directly under the paragraph. The second has a size of its own, the third a font of its own.
    let mut frag = Fragment {
        version: 1,
        body: "<w:p><w:r><w:t>plain</w:t></w:r><w:r><w:rPr><w:sz w:val=\"40\"/></w:rPr><w:t>big</w:t></w:r><w:r><w:rPr><w:rFonts w:ascii=\"Georgia\" w:hAnsi=\"Georgia\"/></w:rPr><w:t>serif</w:t></w:r></w:p>".into(),
        defaults: Some(docxtor_core::fragment::FragDefaults { font: Some("Courier New".into()), size: Some(8.5) }),
        ..Default::default()
    };
    frag.set_namespaces(&[]);
    paste(&mut doc, Pos::new(first, 5), &frag, PasteMode::Keep);
    assert_eq!(texts(&doc)[0], "Startplainbigserif");

    let plain = doc.format_at(Pos::new(first, 7)).unwrap().run;
    assert_eq!(plain.font.as_deref(), Some("Courier New"));
    assert_eq!(plain.size, Some(8.5));
    let big = doc.format_at(Pos::new(first, 11)).unwrap().run;
    assert_eq!(big.size, Some(20.0), "its own size stays");
    assert_eq!(big.font.as_deref(), Some("Courier New"));
    let serif = doc.format_at(Pos::new(first, 15)).unwrap().run;
    assert_eq!(serif.font.as_deref(), Some("Georgia"), "its own font stays");
    assert_eq!(serif.size, Some(8.5));
    // The text that was already there is untouched.
    assert_eq!(doc.format_at(Pos::new(first, 2)).unwrap().run.font, dest.font);

    let xml = saved(&mut doc, "word/document.xml");
    assert_eq!(xml.matches("w:val=\"17\"").count(), 4, "{xml}");
}

#[test]
fn a_fragment_from_the_clipboard_is_checked_before_it_goes_in() {
    let field = |instr: &str, result: &str| {
        format!(
            r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> {instr} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>{result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
        )
    };
    let body = format!(
        r#"<w:p><w:r><w:t>start </w:t></w:r>{}{}<w:fldSimple w:instr=" DDEAUTO cmd calc "><w:r><w:t>simple</w:t></w:r></w:fldSimple><w:r><w:object><w:t>ole</w:t></w:object></w:r><w:hyperlink r:id="l1"><w:r><w:t>link</w:t></w:r></w:hyperlink><w:r><w:drawing><wp:inline><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="m1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#,
        field(r#"INCLUDETEXT "C:\secret.docx""#, "shown"),
        field("PAGE", "7")
    );
    let frag: Fragment = serde_json::from_value(json!({
        "version": 1,
        "ns": [["w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"], ["r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"],
               ["wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"], ["a", "http://schemas.openxmlformats.org/drawingml/2006/main"],
               ["pic", "http://schemas.openxmlformats.org/drawingml/2006/picture"]],
        "body": body,
        "links": {"l1": "javascript:alert(1)"},
        "media": {"m1": {"ext": "../../evil", "data": "AAAA"}}
    }))
    .unwrap();
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    paste(&mut doc, Pos::new(first, 0), &frag, PasteMode::Keep);
    let xml = saved(&mut doc, "word/document.xml");
    for bad in ["INCLUDETEXT", "DDEAUTO", "w:object", "javascript:", "evil"] {
        assert!(!xml.contains(bad), "{bad} got through: {xml}");
    }
    // The text of the dropped fields and the harmless field stay.
    assert!(xml.contains("shown") && xml.contains("simple") && xml.contains("start"), "{xml}");
    assert!(xml.contains("PAGE"), "{xml}");
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    assert!(!pkg.parts.iter().any(|p| p.name.contains("evil")));
}

#[test]
fn a_field_copied_from_docxtor_pastes_back_as_a_field() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    doc.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Page "}).to_string(), false).unwrap();
    doc.apply_json(&json!({"op":"insert_field","at":{"p":first,"o":5},"field":"PAGE"}).to_string(), false).unwrap();
    let len = find_para(&doc.body, first).unwrap().len16();
    let frag = doc.copy_fragment(Pos::new(first, 0), Pos::new(first, len)).unwrap();
    assert!(frag.body.contains("PAGE"), "{}", frag.body);
    let mut dst = Document::new_blank();
    let at = dst.render_all().order[0];
    paste(&mut dst, Pos::new(at, 0), &frag, PasteMode::Keep);
    let xml = saved(&mut dst, "word/document.xml");
    assert!(xml.contains(r#"w:instr=" PAGE ""#), "{xml}");
}

const NONCE: &str = "test-session-nonce-0123456789";

/// This engine's secret, as the browser gives it once per page load.
fn own_session() {
    docxtor_core::fragment::set_session_nonce(NONCE);
}

const FRAG_NS: [[&str; 2]; 5] = [
    ["w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"],
    ["r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"],
    ["wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"],
    ["a", "http://schemas.openxmlformats.org/drawingml/2006/main"],
    ["pic", "http://schemas.openxmlformats.org/drawingml/2006/picture"],
];

fn picture_fragment(ext: &str, link: Option<&str>, origin: &str) -> Fragment {
    let click = if link.is_some() { r#"<a:hlinkClick r:id="l1"/>"# } else { "" };
    let body = format!(
        r#"<w:p><w:r><w:t>before </w:t></w:r><w:r><w:drawing><wp:inline><wp:extent cx="100" cy="100"/><wp:docPr id="1" name="Picture">{click}</wp:docPr><a:graphic><a:graphicData><pic:pic><pic:blipFill><a:blip r:embed="m1"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    );
    let links = link.map(|l| json!({"l1": l})).unwrap_or(json!({}));
    serde_json::from_value(json!({"version": 1, "ns": FRAG_NS, "body": body, "links": links, "media": {"m1": {"ext": ext, "data": "AAAA"}}, "origin": origin})).unwrap()
}

#[test]
fn pictures_in_the_formats_word_stores_paste_between_documents() {
    for ext in ["emz", "wmz", "jfif", "jpe", "dib", "ico", "tga", "pcz", "EMZ", ".wmz"] {
        let mut doc = Document::new_blank();
        let first = doc.render_all().order[0];
        paste(&mut doc, Pos::new(first, 0), &picture_fragment(ext, None, ""), PasteMode::Keep);
        let bytes = doc.save().unwrap();
        let pkg = Package::open(&bytes).unwrap();
        let want = format!(".{}", ext.trim_start_matches('.').to_ascii_lowercase());
        assert!(pkg.parts.iter().any(|p| p.name.starts_with("word/media/") && p.name.ends_with(&want)), "{ext}: the picture was dropped");
        let xml = saved(&mut doc, "word/document.xml");
        assert!(xml.contains("r:embed"), "{ext}: {xml}");
    }
    // The path-safety check stays: nothing but a short picture name reaches the archive.
    for ext in ["../../evil", "png/../x", "exe", ""] {
        let mut doc = Document::new_blank();
        let first = doc.render_all().order[0];
        paste(&mut doc, Pos::new(first, 0), &picture_fragment(ext, None, ""), PasteMode::Keep);
        let bytes = doc.save().unwrap();
        let pkg = Package::open(&bytes).unwrap();
        assert!(!pkg.parts.iter().any(|p| p.name.contains("evil") || p.name.ends_with(".exe")), "{ext}");
        assert!(!saved(&mut doc, "word/document.xml").contains("r:embed"), "{ext}");
    }
}

#[test]
fn a_file_link_copied_in_docxtor_pastes_back_as_a_link() {
    own_session();
    let url = "file://server/share/Q3 report.docx";
    let mut src = Document::new_blank();
    let first = src.render_all().order[0];
    src.apply_json(&json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Open the report."}).to_string(), false).unwrap();
    src.apply_json(&json!({"op":"set_link","from":{"p":first,"o":0},"to":{"p":first,"o":8},"url":url}).to_string(), false).unwrap();
    let len = find_para(&src.body, first).unwrap().len16();
    let frag = src.copy_fragment(Pos::new(first, 0), Pos::new(first, len)).unwrap();
    assert_eq!(frag.origin, NONCE);
    assert!(frag.is_own());
    // Through JSON, as the clipboard carries it.
    let frag: Fragment = serde_json::from_str(&serde_json::to_string(&frag).unwrap()).unwrap();

    let mut dst = Document::new_blank();
    let at = dst.render_all().order[0];
    paste(&mut dst, Pos::new(at, 0), &frag, PasteMode::Keep);
    assert!(saved(&mut dst, "word/_rels/document.xml.rels").contains("file://server/share/Q3 report.docx"));
    assert!(saved(&mut dst, "word/document.xml").contains("<w:hyperlink r:id="));

    // The same content from any other clipboard (no secret, or a guessed one) loses the `file:` link and keeps the text.
    for origin in ["", "a-guess-at-the-nonce-0000000"] {
        let mut other = frag.clone();
        other.origin = origin.into();
        assert!(!other.is_own());
        let mut dst = Document::new_blank();
        let at = dst.render_all().order[0];
        paste(&mut dst, Pos::new(at, 0), &other, PasteMode::Keep);
        assert!(!saved(&mut dst, "word/_rels/document.xml.rels").contains("file:"));
        let xml = saved(&mut dst, "word/document.xml");
        assert!(!xml.contains("<w:hyperlink r:id=") && xml.contains("Open"), "{xml}");
    }
}

#[test]
fn a_picture_with_a_file_link_copied_in_docxtor_keeps_both_and_a_stranger_clipboard_only_the_text() {
    own_session();
    let url = "file://server/share/chart.xlsx";
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    paste(&mut doc, Pos::new(first, 0), &picture_fragment("png", Some(url), NONCE), PasteMode::Keep);
    assert!(saved(&mut doc, "word/_rels/document.xml.rels").contains(url));
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains("a:hlinkClick") && xml.contains("r:embed") && xml.contains("before"), "{xml}");

    // From elsewhere the run with the link is dropped; javascript: stays refused even with the secret.
    for (url, origin) in [(url, ""), ("javascript:alert(1)", NONCE)] {
        let mut doc = Document::new_blank();
        let first = doc.render_all().order[0];
        paste(&mut doc, Pos::new(first, 0), &picture_fragment("png", Some(url), origin), PasteMode::Keep);
        let xml = saved(&mut doc, "word/document.xml");
        assert!(!xml.contains("hlinkClick") && !xml.contains("file:") && !xml.contains("javascript") && xml.contains("before"), "{xml}");
    }
}

#[test]
fn a_picture_part_name_given_up_by_undo_and_taken_again_has_a_new_generation() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    paste(&mut doc, Pos::new(first, 0), &picture_fragment("png", None, ""), PasteMode::Keep);
    let name = "word/media/docxtor1.png";
    assert!(doc.media(name).is_some());
    let pasted = doc.media_gen(name);
    assert_ne!(pasted, 0);
    // Undo takes the part out of the package, and a different picture takes the free name.
    assert!(doc.undo());
    assert!(doc.media(name).is_none());
    let gone = doc.media_gen(name);
    assert_ne!(gone, pasted);
    let added = doc.add_media("png", vec![0x89, b'P', b'N', b'G', 9, 9, 9, 9]);
    assert_eq!(added, name);
    assert!(doc.media_gen(name) != pasted && doc.media_gen(name) != gone, "the name must say it holds another picture now");
    // A part nobody touched keeps its number.
    assert_eq!(doc.media_gen("word/media/never.png"), 0);
}
