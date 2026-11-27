//! Breaks and layout: paragraph settings, section and column breaks, columns, line numbers,
//! page colour, hyphenation and the AI tools for them.

use docxtor_core::ai::call_tool;
use docxtor_core::model::{find_block, walk_paragraphs, write_block};
use docxtor_core::props::ParaFmt;
use docxtor_core::render::RBlock;
use docxtor_core::{fidelity, package::Package, Document, Op, Pos};
use serde_json::{json, Value};

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}; run python3 fixtures/make_fixtures.py"))
}

/// Every XML part of a saved package must be namespace-well-formed.
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

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn find_para(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn ids(doc: &Document) -> Vec<u32> {
    docxtor_core::model::paragraph_order(&doc.body)
}

/// A saved part as text.
fn saved_part(doc: &mut Document, name: &str) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

fn para_xml(doc: &Document, id: u32) -> String {
    write_block(find_block(&doc.body, id).unwrap()).to_xml()
}

/// Reopen a document after editing its package.
fn rewrite(doc: &mut Document, f: impl FnOnce(&mut Package)) -> Document {
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    f(&mut pkg);
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn set_part(pkg: &mut Package, name: &str, f: impl FnOnce(String) -> String) {
    let s = String::from_utf8(pkg.get(name).unwrap().to_vec()).unwrap();
    pkg.set(name, f(s).into_bytes());
}

fn fmt_at(doc: &Document, id: u32) -> ParaFmt {
    doc.format_at(Pos::new(id, 0)).unwrap().para
}

/// Section break shown after each paragraph.
fn breaks(doc: &Document) -> Vec<(u32, Option<String>)> {
    doc.render_snapshot()
        .blocks
        .iter()
        .filter_map(|b| match b {
            RBlock::Para(p) => Some((p.id, p.sect_break.clone())),
            _ => None,
        })
        .collect()
}

fn ends(doc: &Document) -> Vec<Option<u32>> {
    doc.page_setup().sections.iter().map(|s| s.end).collect()
}

fn starts(doc: &Document) -> Vec<String> {
    doc.page_setup().sections.iter().map(|s| s.start.clone()).collect()
}

// ---------------------------------------------------------------------------
// Paragraph settings
// ---------------------------------------------------------------------------

/// A blank document whose Normal style turns on keep-with-next and widow control.
fn doc_with_toggling_style() -> Document {
    let mut doc = doc_with("alpha");
    rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/styles.xml", |s| {
            s.replace(r#"<w:name w:val="Normal"/><w:qFormat/>"#, r#"<w:name w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:widowControl/></w:pPr>"#)
        })
    })
}

#[test]
fn paragraph_toggles_override_styles_and_round_trip() {
    let mut doc = doc_with_toggling_style();
    let p = find_para(&doc, "alpha");
    let f = fmt_at(&doc, p);
    assert_eq!((f.keep_next, f.widow_control, f.keep_lines), (Some(true), Some(true), None));

    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"widowControl":false,"keepNext":false,"keepLines":true,"suppressLineNumbers":true,"suppressAutoHyphens":true,"pageBreakBefore":true}}));
    let f = fmt_at(&doc, p);
    assert_eq!(f.widow_control, Some(false), "explicit off beats the style");
    assert_eq!(f.keep_next, Some(false));
    assert_eq!((f.keep_lines, f.suppress_line_numbers, f.suppress_auto_hyphens, f.page_break_before), (Some(true), Some(true), Some(true), Some(true)));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:widowControl w:val="0"/>"#) && xml.contains(r#"<w:keepNext w:val="0"/>"#), "{xml}");
    // Schema order inside w:pPr.
    let order = ["<w:keepNext", "<w:keepLines", "<w:pageBreakBefore", "<w:widowControl", "<w:suppressLineNumbers", "<w:suppressAutoHyphens"];
    let at: Vec<usize> = order.iter().map(|t| xml.find(t).unwrap_or_else(|| panic!("{t} in {xml}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{xml}");

    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let again = Document::open(&saved).unwrap();
    let f = fmt_at(&again, p);
    assert_eq!((f.widow_control, f.keep_next, f.keep_lines), (Some(false), Some(false), Some(true)));

    // Turning things back on, and off where nothing inherits them, removes or rewrites cleanly.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"widowControl":true,"keepNext":true,"keepLines":false,"suppressLineNumbers":false,"suppressAutoHyphens":false,"pageBreakBefore":false}}));
    let xml = para_xml(&doc, p);
    assert!(xml.contains("<w:widowControl/>") && xml.contains("<w:keepNext/>"), "{xml}");
    assert!(!xml.contains("keepLines") && !xml.contains("suppress") && !xml.contains("pageBreakBefore"), "{xml}");
    // Widow control is on by default in Word, so off is always explicit.
    let mut plain = doc_with("beta");
    let b = find_para(&plain, "beta");
    ops(&mut plain, json!({"op":"format_paragraphs","ids":[b],"format":{"widowControl":false}}));
    assert!(para_xml(&plain, b).contains(r#"<w:widowControl w:val="0"/>"#));
}

#[test]
fn paragraph_shading_and_borders() {
    let mut doc = doc_with("alpha\n\nbeta");
    let p = find_para(&doc, "alpha");
    let side = |sz: i64, color: &str| json!({"val":"single","sz":sz,"color":color,"space":2});
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"shading":"#ffcc00","borders":{"top":side(4,"auto"),"bottom":side(12,"FF0000"),"left":side(4,"auto"),"right":side(4,"auto")},"align":"center"}}));
    let f = fmt_at(&doc, p);
    assert_eq!(f.shading.as_deref(), Some("FFCC00"));
    let b = f.borders.unwrap();
    assert_eq!((b.top.unwrap().sz, b.bottom.unwrap().color, b.left.is_some(), b.right.is_some()), (4, "FF0000".to_string(), true, true));
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:shd w:val="clear" w:color="auto" w:fill="FFCC00"/>"#), "{xml}");
    // pBdr sides are in schema order, and pBdr precedes shd precedes jc.
    let sides: Vec<usize> = ["<w:top ", "<w:left ", "<w:bottom ", "<w:right "].iter().map(|t| xml.find(t).unwrap()).collect();
    assert!(sides.windows(2).all(|w| w[0] < w[1]), "{xml}");
    assert!(xml.find("<w:pBdr").unwrap() < xml.find("<w:shd").unwrap() && xml.find("<w:shd").unwrap() < xml.find("<w:jc").unwrap(), "{xml}");
    assert_valid(&doc.save().unwrap());

    // Borders are replaced as a whole.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"borders":{"bottom":side(8,"auto")}}}));
    let b = fmt_at(&doc, p).borders.unwrap();
    assert!(b.top.is_none() && b.left.is_none() && b.right.is_none() && b.bottom.unwrap().sz == 8);
    assert_eq!(para_xml(&doc, p).matches("<w:pBdr>").count(), 1);
    // No sides removes them; an empty shading removes the fill; clear drops both.
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"borders":{},"shading":""}}));
    let xml = para_xml(&doc, p);
    assert!(!xml.contains("pBdr") && !xml.contains("<w:shd"), "{xml}");
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert!(fmt_at(&again, find_para(&again, "alpha")).borders.is_none());
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"shading":"00FF00","clear":true}}));
    assert!(!para_xml(&doc, p).contains("<w:jc"));
    assert_eq!(fmt_at(&doc, p).shading.as_deref(), Some("00FF00"));
}

// ---------------------------------------------------------------------------
// Page and column breaks
// ---------------------------------------------------------------------------

#[test]
fn column_and_page_breaks() {
    let mut doc = doc_with("Alpha beta");
    let p = find_para(&doc, "Alpha");
    let r = doc.apply(&serde_json::from_value::<Op>(json!({"op":"insert_break","at":{"p":p,"o":6},"kind":"column"})).unwrap(), false).unwrap();
    assert_eq!(r.caret, Some(Pos::new(p, 7)));
    doc.apply_json(&json!({"op":"insert_break","at":{"p":p,"o":0},"kind":"page"}).to_string(), false).unwrap();
    let xml = para_xml(&doc, p);
    assert!(xml.contains(r#"<w:br w:type="column"/>"#) && xml.contains(r#"<w:br w:type="page"/>"#), "{xml}");
    let kinds: Vec<&str> = match doc.render_snapshot().blocks.iter().find(|b| matches!(b, RBlock::Para(x) if x.id == p)).unwrap() {
        RBlock::Para(x) => x.segs.iter().map(|s| s.k).collect(),
        _ => unreachable!(),
    };
    assert_eq!(kinds, ["pbr", "t", "cbr", "t"]);
    assert_eq!(doc.page_setup().sections.len(), 1);
    assert_valid(&doc.save().unwrap());
    // The older op still works and matches.
    let mut old = doc_with("Alpha beta");
    let q = find_para(&old, "Alpha");
    ops(&mut old, json!({"op":"insert_page_break","at":{"p":q,"o":6}}));
    assert_eq!(para_xml(&old, q).matches(r#"w:type="page""#).count(), 1);
}

// ---------------------------------------------------------------------------
// Section breaks
// ---------------------------------------------------------------------------

const KINDS: [(&str, &str); 4] = [("next_page", "nextPage"), ("continuous", "continuous"), ("even_page", "evenPage"), ("odd_page", "oddPage")];

#[test]
fn section_breaks_in_middle_start_and_end() {
    for (kind, xml_type) in KINDS {
        for (where_, off) in [("middle", 6usize), ("start", 0), ("end", 10)] {
            let mut doc = doc_with("Alpha beta\n\nSecond");
            let p = find_para(&doc, "Alpha");
            let r = doc.apply_json(&json!({"op":"insert_break","at":{"p":p,"o":off},"kind":kind}).to_string(), false).unwrap().remove(0);
            let new_id = r.created[0];
            assert_eq!(r.caret, Some(Pos::new(new_id, 0)), "{kind} {where_}");
            let t = texts(&doc);
            assert_eq!(t[0].clone() + &t[1], "Alpha beta", "{kind} {where_}");
            assert_eq!(t[0].len(), off, "{kind} {where_}");
            assert_eq!(ids(&doc)[..2], [p, new_id]);

            // Two sections: the first ends at the first half; the second starts the way the break says.
            assert_eq!(ends(&doc), [Some(p), None], "{kind} {where_}");
            assert_eq!(starts(&doc), ["nextPage", xml_type], "{kind} {where_}");
            // The break is shown on the paragraph that ends the first section, and only there.
            let shown: Vec<_> = breaks(&doc).into_iter().filter(|(_, b)| b.is_some()).collect();
            assert_eq!(shown, [(p, Some(xml_type.to_string()))], "{kind} {where_}");

            // XML: the first half's sectPr has the old (absent) type; the final one has the new type.
            let first = para_xml(&doc, p);
            assert!(first.contains("<w:sectPr>") && first.contains("<w:pgSz"), "{first}");
            assert!(!first.contains("<w:type"), "{first}");
            let body = saved_part(&mut doc, "word/document.xml");
            let last = &body[body.rfind("<w:sectPr").unwrap()..];
            if xml_type == "nextPage" {
                assert!(!last.contains("<w:type"), "{last}");
            } else {
                assert!(last.contains(&format!(r#"<w:type w:val="{xml_type}"/>"#)), "{last}");
            }
            // sectPr goes after pStyle/rPr-like props and is the last pPr child here.
            assert!(first.find("<w:sectPr>").unwrap() < first.rfind("</w:pPr>").unwrap());
            let again = Document::open(&doc.save().unwrap()).unwrap();
            assert_eq!(starts(&again), ["nextPage", xml_type]);
        }
    }
}

#[test]
fn second_section_break_in_a_section_that_already_ends_with_a_break() {
    let mut doc = doc_with("Alpha beta\n\nBravo charlie\n\nDelta");
    let a = find_para(&doc, "Alpha");
    let b = find_para(&doc, "Bravo");
    let at = |p: u32, o: usize, kind: &str| json!({"op":"insert_break","at":{"p":p,"o":o},"kind":kind});
    let a2 = doc.apply_json(&at(a, 6, "next_page").to_string(), false).unwrap().remove(0).created[0];
    let b2 = doc.apply_json(&at(b, 6, "continuous").to_string(), false).unwrap().remove(0).created[0];
    assert_eq!(ends(&doc), [Some(a), Some(b), None]);
    assert_eq!(starts(&doc), ["nextPage", "nextPage", "continuous"]);
    // A break inside the middle section, which already ends with B's sectPr.
    let a3 = doc.apply_json(&at(a2, 2, "even_page").to_string(), false).unwrap().remove(0).created[0];
    assert_eq!(ends(&doc), [Some(a), Some(a2), Some(b), None]);
    assert_eq!(starts(&doc), ["nextPage", "nextPage", "evenPage", "continuous"]);
    assert_eq!(texts(&doc).len(), 6, "{:?}", texts(&doc));
    let shown: Vec<_> = breaks(&doc).into_iter().filter_map(|(p, k)| k.map(|k| (p, k))).collect();
    assert_eq!(shown, [(a, "nextPage".to_string()), (a2, "evenPage".to_string()), (b, "continuous".to_string())]);
    let _ = (a3, b2);
    // Exactly one sectPr per section is saved.
    let xml = saved_part(&mut doc, "word/document.xml");
    assert_eq!(xml.matches("<w:sectPr").count(), 4);
}

#[test]
fn enter_in_a_paragraph_that_ends_a_section_moves_the_break_to_the_second_half() {
    let mut doc = doc_with("Alpha beta\n\nSecond");
    let p = find_para(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":p,"o":10},"kind":"continuous"}));
    assert_eq!(ends(&doc), [Some(p), None]);
    ops(&mut doc, json!({"op":"split_paragraph","at":{"p":p,"o":5}}));
    let np = ids(&doc)[1];
    assert_eq!(ends(&doc), [Some(np), None], "the section still ends at the last half");
    assert_eq!(doc.page_setup().sections.len(), 2);
    assert!(!para_xml(&doc, p).contains("sectPr"));
}

#[test]
fn section_breaks_need_a_regular_body_paragraph() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let cell = find_para(&doc, "$1.2M");
    let before = texts(&doc);
    let e = doc.apply_json(&json!({"op":"insert_break","at":{"p":cell,"o":0},"kind":"next_page"}).to_string(), false).unwrap_err();
    assert!(e.to_string().contains("Section breaks"), "{e}");
    assert_eq!(texts(&doc), before);
    assert_eq!(doc.page_setup().sections.len(), 1);
    // Page and column breaks are fine in a cell.
    ops(&mut doc, json!({"op":"insert_break","at":{"p":cell,"o":0},"kind":"column"}));
    // Headers and footnotes only take what makes sense there.
    let header = doc.page_setup().headers["default"].blocks.iter().find_map(|b| if let RBlock::Para(p) = b { Some(p.id) } else { None }).unwrap();
    assert!(doc.apply_json(&json!({"op":"insert_break","at":{"p":header,"o":0},"kind":"continuous"}).to_string(), false).is_err());
    assert!(doc.apply_json(&json!({"op":"insert_break","at":{"p":header,"o":0},"kind":"page"}).to_string(), false).is_err());
    assert!(doc.apply_json(&json!({"op":"insert_break","at":{"p":99999,"o":0},"kind":"continuous"}).to_string(), false).is_err());
}

#[test]
fn deleting_a_section_break_merges_the_sections() {
    // Backspace/Delete across the break.
    let mut doc = doc_with("Alpha beta\n\nSecond");
    let p = find_para(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":p,"o":6},"kind":"next_page"}));
    let np = ids(&doc)[1];
    assert_eq!(doc.page_setup().sections.len(), 2);
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":6},"to":{"p":np,"o":0}}));
    assert_eq!(doc.page_setup().sections.len(), 1);
    assert_eq!(texts(&doc)[0], "Alpha beta");
    assert!(breaks(&doc).iter().all(|(_, b)| b.is_none()));
    assert!(!para_xml(&doc, p).contains("sectPr"));
    assert_eq!(saved_part(&mut doc, "word/document.xml").matches("<w:sectPr").count(), 1);

    // The empty paragraph a break at offset 0 leaves behind merges away the same way.
    let mut doc = doc_with("Alpha beta");
    let p = find_para(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":p,"o":0},"kind":"continuous"}));
    let np = ids(&doc)[1];
    assert_eq!(texts(&doc), ["", "Alpha beta"]);
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":np,"o":0}}));
    assert_eq!(doc.page_setup().sections.len(), 1);
    assert_eq!(texts(&doc), ["Alpha beta"]);

    // When the later paragraph ends a section, its properties govern the merged one.
    let mut doc = doc_with("Alpha beta\n\nBravo charlie");
    let a = find_para(&doc, "Alpha");
    let b = find_para(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":a,"o":6},"kind":"next_page"}));
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":6},"kind":"continuous"}));
    let a2 = ids(&doc)[1];
    assert_eq!(ends(&doc), [Some(a), Some(b), None]);
    // Join the rest of A with B's first half (B ends the second section).
    ops(&mut doc, json!({"op":"delete","from":{"p":a2,"o":4},"to":{"p":b,"o":0}}));
    assert_eq!(ends(&doc), [Some(a), Some(a2), None]);
    assert_eq!(starts(&doc), ["nextPage", "nextPage", "continuous"]);
    // Deleting from the first section into the last drops every break between them.
    let third = ids(&doc)[2];
    ops(&mut doc, json!({"op":"delete","from":{"p":a,"o":6},"to":{"p":third,"o":0}}));
    assert_eq!(doc.page_setup().sections.len(), 1);
    assert_valid(&doc.save().unwrap());
    // Undo brings the break back.
    assert!(doc.undo());
    assert_eq!(doc.page_setup().sections.len(), 3);
}

#[test]
fn render_changed_follows_section_type_changes() {
    let mut doc = doc_with("Alpha beta\n\nBravo charlie");
    let a = find_para(&doc, "Alpha");
    let b = find_para(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":a,"o":6},"kind":"next_page"}));
    doc.render_all();
    // Another break in the following section changes what the first break shows only through the types.
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":6},"kind":"odd_page"}));
    let out = doc.render_changed();
    let shown = |out: &docxtor_core::document::RenderOut, id: u32| {
        out.blocks.iter().find_map(|b| match b {
            RBlock::Para(p) if p.id == id => Some(p.sect_break.clone()),
            _ => None,
        })
    };
    assert_eq!(shown(&out, a), Some(Some("nextPage".to_string())));
    assert_eq!(shown(&out, b), Some(Some("oddPage".to_string())));
    // Changing the last section's type touches the paragraph before it.
    let _ = doc.render_changed();
    let second = ids(&doc)[1];
    ops(&mut doc, json!({"op":"delete","from":{"p":a,"o":6},"to":{"p":second,"o":0}}));
    let out = doc.render_changed();
    assert_eq!(shown(&out, a), Some(None));
}

// ---------------------------------------------------------------------------
// Sections in the page setup
// ---------------------------------------------------------------------------

#[test]
fn sections_inherit_headers_and_footers() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let ps = doc.page_setup();
    assert_eq!(ps.sections.len(), 1);
    let s = &ps.sections[0];
    assert_eq!((s.end, s.start.as_str(), s.width, s.height), (None, "nextPage", ps.width, ps.height));
    assert_eq!(s.headers["default"], "word/header1.xml");
    assert!(ps.parts["word/header1.xml"].blocks.len() == 1 && ps.parts.contains_key("word/footer1.xml"));
    assert!(!ps.hyphenation && !ps.even_and_odd_headers);
    assert_eq!(ps.page_color, None);
    assert!(ps.lang.as_deref().is_some_and(|l| l.contains('-')) || ps.lang.is_none());

    let p = find_para(&doc, "Highlights");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":p,"o":0},"kind":"next_page"}));
    let ps = doc.page_setup();
    assert_eq!(ps.sections.len(), 2);
    assert_eq!(ps.sections[0].end, Some(p));
    assert_eq!(ps.sections[1].headers["default"], "word/header1.xml");
    // The last section drops its own header: it uses the one before it.
    ops(&mut doc, json!({"op":"remove_header_footer","kind":"header"}));
    let ps = doc.page_setup();
    assert!(ps.headers.is_empty(), "the final section has no header of its own");
    assert_eq!(ps.sections[1].headers["default"], "word/header1.xml");
    assert_eq!(ps.sections[0].headers["default"], "word/header1.xml");
    assert!(ps.sections[1].footers.contains_key("default"));
    // The caret's section is reported with the formatting.
    assert_eq!(doc.format_at(Pos::new(p, 0)).unwrap().section, 0);
    assert_eq!(doc.format_at(Pos::new(find_para(&doc, "Highlights"), 0)).unwrap().section, 1);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn set_page_setup_scopes() {
    let mut doc = doc_with("Alpha beta\n\nBravo");
    let a = find_para(&doc, "Alpha");
    let b = find_para(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":a,"o":6},"kind":"next_page"}));
    // One section only.
    ops(&mut doc, json!({"op":"set_page_setup","section":a,"width":16838,"height":11906,"margins":[720,720,720,720]}));
    let ps = doc.page_setup();
    assert_eq!((ps.sections[0].width, ps.sections[0].height, ps.sections[0].margin), (16838, 11906, [720; 4]));
    assert_eq!((ps.sections[1].width, ps.sections[1].margin), (12240, [1440; 4]));
    assert_eq!(ps.width, 12240, "the top-level fields describe the last section");
    let xml = para_xml(&doc, a);
    assert!(xml.contains(r#"w:orient="landscape""#), "{xml}");
    // The section that holds a paragraph, not just the one it ends.
    ops(&mut doc, json!({"op":"set_page_setup","section":b,"width":10000}));
    assert_eq!(doc.page_setup().sections[1].width, 10000);
    assert_eq!(doc.page_setup().sections[0].width, 16838);
    // No scope: every section.
    ops(&mut doc, json!({"op":"set_page_setup","width":11906,"height":16838}));
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| (s.width, s.height) == (11906, 16838)));
    assert!(!para_xml(&doc, a).contains("orient"));
    assert!(doc.apply_json(&json!({"op":"set_page_setup","section":99999,"width":1}).to_string(), false).is_err());
    assert_valid(&doc.save().unwrap());
}

// ---------------------------------------------------------------------------
// Columns and line numbers
// ---------------------------------------------------------------------------

#[test]
fn columns() {
    let mut doc = doc_with("Alpha beta\n\nBravo");
    let a = find_para(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":a,"o":6},"kind":"continuous"}));
    // The second section only.
    let last = ids(&doc)[2];
    ops(&mut doc, json!({"op":"set_columns","section":last,"count":2}));
    let ps = doc.page_setup();
    assert_eq!((ps.sections[0].cols.num, ps.sections[1].cols.num), (1, 2));
    assert_eq!((ps.sections[1].cols.space, ps.sections[1].cols.sep, ps.cols), (720, false, 2));
    ops(&mut doc, json!({"op":"set_columns","count":3,"space":360,"sep":true}));
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| s.cols.num == 3 && s.cols.space == 360 && s.cols.sep));
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:cols w:space="360" w:num="3" w:sep="1"/>"#) || xml.contains(r#"w:num="3""#), "{xml}");
    // One column drops the count, keeps the gap; sep false removes the line.
    ops(&mut doc, json!({"op":"set_columns","count":1,"sep":false}));
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| s.cols.num == 1 && s.cols.space == 360 && !s.cols.sep));
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(!xml.contains("w:num=") && !xml.contains("w:sep="), "{xml}");
    assert!(doc.apply_json(&json!({"op":"set_columns","count":0}).to_string(), false).is_err());
    assert!(doc.apply_json(&json!({"op":"set_columns","count":7}).to_string(), false).is_err());

    // Unequal columns from a Word file are read, then replaced by equal ones.
    let mut doc = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/document.xml", |s| {
            s.replacen(r#"<w:cols w:space="360"/>"#, r#"<w:cols w:num="2" w:space="400" w:equalWidth="0"><w:col w:w="3000" w:space="400"/><w:col w:w="5000"/></w:cols>"#, 2)
        })
    });
    let c = doc.page_setup().sections[1].cols.clone();
    assert_eq!((c.num, c.space, c.widths.clone()), (2, 400, vec![3000, 5000]));
    ops(&mut doc, json!({"op":"set_columns","count":3}));
    let c = doc.page_setup().sections[1].cols.clone();
    assert_eq!((c.num, c.space, c.widths.len()), (3, 400, 0));
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(!xml.contains("equalWidth") && !xml.contains("<w:col "), "{xml}");
}

#[test]
fn line_numbers() {
    let mut doc = doc_with("Alpha beta\n\nBravo");
    assert!(doc.page_setup().sections[0].line_numbers.is_none());
    ops(&mut doc, json!({"op":"set_line_numbers","on":true}));
    let ln = doc.page_setup().sections[0].line_numbers.clone().unwrap();
    assert_eq!((ln.count_by, ln.start, ln.restart.as_str(), ln.distance), (1, 1, "newPage", None));
    ops(&mut doc, json!({"op":"set_line_numbers","on":true,"count_by":5,"restart":"continuous"}));
    let ln = doc.page_setup().sections[0].line_numbers.clone().unwrap();
    assert_eq!((ln.count_by, ln.restart.as_str()), (5, "continuous"));
    // Keeps what it doesn't touch (distance and start from a Word file), and newPage is written as the default.
    let mut doc = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/document.xml", |s| s.replace(r#"w:countBy="5""#, r#"w:countBy="5" w:distance="360" w:start="3""#))
    });
    ops(&mut doc, json!({"op":"set_line_numbers","on":true,"restart":"newSection"}));
    let ln = doc.page_setup().sections[0].line_numbers.clone().unwrap();
    assert_eq!((ln.count_by, ln.start, ln.distance, ln.restart.as_str()), (5, 3, Some(360), "newSection"));
    ops(&mut doc, json!({"op":"set_line_numbers","on":true,"restart":"newPage"}));
    assert!(!saved_part(&mut doc, "word/document.xml").contains("w:restart"));
    assert!(doc.apply_json(&json!({"op":"set_line_numbers","on":true,"restart":"sometimes"}).to_string(), false).is_err());
    ops(&mut doc, json!({"op":"set_line_numbers","on":false}));
    assert!(doc.page_setup().sections[0].line_numbers.is_none());
    assert!(!saved_part(&mut doc, "word/document.xml").contains("lnNumType"));
    // Suppressing numbers on one paragraph is a paragraph setting.
    let a = find_para(&doc, "Alpha");
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[a],"format":{"suppressLineNumbers":true}}));
    assert_eq!(fmt_at(&doc, a).suppress_line_numbers, Some(true));
}

// ---------------------------------------------------------------------------
// Page colour and hyphenation (settings.xml)
// ---------------------------------------------------------------------------

#[test]
fn page_colour_set_clear_undo() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    ops(&mut doc, json!({"op":"set_page_color","color":"#ffcc00"}));
    assert_eq!(doc.page_setup().page_color.as_deref(), Some("FFCC00"));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let main = docxtor_core::xml::parse(pkg.get("word/document.xml").unwrap()).unwrap();
    let first = main.root.elements().next().unwrap();
    assert_eq!((first.name.as_str(), first.attr("w:color")), ("w:background", Some("FFCC00")));
    assert_eq!(main.root.elements().nth(1).unwrap().name, "w:body");
    let settings = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
    assert!(settings.contains("<w:displayBackgroundShape/>"), "{settings}");
    // Reopening reads it back.
    assert_eq!(Document::open(&bytes).unwrap().page_setup().page_color.as_deref(), Some("FFCC00"));

    // Changing it keeps a single element.
    ops(&mut doc, json!({"op":"set_page_color","color":"0a0b0c"}));
    assert_eq!(doc.page_setup().page_color.as_deref(), Some("0A0B0C"));
    assert_eq!(saved_part(&mut doc, "word/document.xml").matches("<w:background").count(), 1);
    assert!(doc.apply_json(&json!({"op":"set_page_color","color":"red"}).to_string(), false).is_err());
    assert!(doc.apply_json(&json!({"op":"set_page_color","color":"12345"}).to_string(), false).is_err());

    // Clearing removes both.
    ops(&mut doc, json!({"op":"set_page_color","color":null}));
    assert_eq!(doc.page_setup().page_color, None);
    assert!(!saved_part(&mut doc, "word/document.xml").contains("w:background"));
    assert!(!saved_part(&mut doc, "word/settings.xml").contains("displayBackgroundShape"));

    // Undo and redo restore exactly.
    assert!(doc.undo());
    assert_eq!(doc.page_setup().page_color.as_deref(), Some("0A0B0C"));
    assert!(saved_part(&mut doc, "word/settings.xml").contains("displayBackgroundShape"));
    assert!(doc.undo() && doc.undo());
    assert_eq!(doc.page_setup().page_color, None);
    assert!(!saved_part(&mut doc, "word/document.xml").contains("w:background"));
    assert!(!saved_part(&mut doc, "word/settings.xml").contains("displayBackgroundShape"));
    assert!(doc.redo());
    assert_eq!(doc.page_setup().page_color.as_deref(), Some("FFCC00"));
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:background w:color="FFCC00"/>"#));
    assert!(saved_part(&mut doc, "word/settings.xml").contains("displayBackgroundShape"));
}

#[test]
fn hyphenation_and_settings_order() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let original = Package::open(&fixture("rich.docx")).unwrap().get("word/settings.xml").unwrap().to_vec();
    // Other edits leave settings.xml byte-identical.
    let p = find_para(&doc, "Highlights");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":p,"o":0},"kind":"continuous"}));
    ops(&mut doc, json!({"op":"set_columns","count":2}));
    ops(&mut doc, json!({"op":"set_page_color","color":null}));
    let bytes = doc.save().unwrap();
    assert_eq!(Package::open(&bytes).unwrap().get("word/settings.xml").unwrap(), &original[..], "settings.xml untouched");
    let report = fidelity::compare(&Package::open(&fixture("rich.docx")).unwrap(), &Package::open(&bytes).unwrap());
    assert!(report.changed.iter().all(|c| !c.name.contains("settings")), "{report:?}");

    ops(&mut doc, json!({"op":"set_hyphenation","on":true}));
    assert!(doc.page_setup().hyphenation);
    let s = saved_part(&mut doc, "word/settings.xml");
    let at = |t: &str| s.find(t).unwrap_or_else(|| panic!("{t} in {s}"));
    assert!(at("<w:autoHyphenation/>") > at("<w:defaultTabStop"), "{s}");
    assert!(at("<w:autoHyphenation/>") < at("<w:compat>"), "{s}");
    assert!(s.find("<w:characterSpacingControl").is_none_or(|c| at("<w:autoHyphenation/>") < c), "{s}");
    assert!(Document::open(&doc.save().unwrap()).unwrap().page_setup().hyphenation);
    ops(&mut doc, json!({"op":"set_hyphenation","on":false}));
    assert!(!doc.page_setup().hyphenation);
    assert!(!saved_part(&mut doc, "word/settings.xml").contains("autoHyphenation"));
    assert!(doc.undo());
    assert!(doc.page_setup().hyphenation);
    assert!(saved_part(&mut doc, "word/settings.xml").contains("<w:autoHyphenation/>"));
    assert!(doc.undo());
    assert!(!doc.page_setup().hyphenation);
    assert!(doc.redo() && doc.redo());
    assert!(!doc.page_setup().hyphenation);

    // An explicit off in the file counts as off; evenAndOddHeaders is read.
    let doc = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/settings.xml", |s| s.replace("<w:compat>", r#"<w:autoHyphenation w:val="0"/><w:evenAndOddHeaders/><w:compat>"#))
    });
    let ps = doc.page_setup();
    assert!(!ps.hyphenation && ps.even_and_odd_headers);
}

#[test]
fn a_document_without_settings_gets_one() {
    let mut doc = Document::new_blank();
    let mut doc = rewrite(&mut doc, |pkg| {
        pkg.parts.retain(|p| p.name != "word/settings.xml");
        set_part(pkg, "word/_rels/document.xml.rels", |s| s.replace(r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>"#, ""));
        set_part(pkg, "[Content_Types].xml", |s| s.replace(r#"<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>"#, ""));
    });
    assert!(!doc.page_setup().hyphenation);
    // Turning things off creates nothing.
    ops(&mut doc, json!({"op":"set_hyphenation","on":false}));
    ops(&mut doc, json!({"op":"set_page_color","color":null}));
    assert!(Package::open(&doc.save().unwrap()).unwrap().get("word/settings.xml").is_none());

    ops(&mut doc, json!({"op":"set_hyphenation","on":true}));
    ops(&mut doc, json!({"op":"set_page_color","color":"FFF2CC"}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let settings = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
    assert!(settings.contains("<w:displayBackgroundShape/><w:autoHyphenation/>"), "{settings}");
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(ct.contains(r#"PartName="/word/settings.xml""#) && ct.contains("wordprocessingml.settings+xml"), "{ct}");
    let rels = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
    assert!(rels.contains("relationships/settings") && rels.contains(r#"Target="settings.xml""#), "{rels}");
    let again = Document::open(&bytes).unwrap();
    assert!(again.page_setup().hyphenation && again.page_setup().page_color.is_some());
    // Undoing back to no settings still saves a valid file.
    assert!(doc.undo() && doc.undo());
    assert_valid(&doc.save().unwrap());
}

#[test]
fn language_comes_from_the_style_defaults() {
    let doc = Document::new_blank();
    assert_eq!(doc.page_setup().lang.as_deref(), Some("en-US"));
}

// ---------------------------------------------------------------------------
// Page estimates and the table of contents
// ---------------------------------------------------------------------------

#[test]
fn toc_page_numbers_follow_the_next_section_type() {
    for (kind, page) in [("next_page", 2), ("continuous", 1), ("odd_page", 2)] {
        let mut doc = doc_with("# One\n\n# Two");
        let one = find_para(&doc, "One");
        ops(&mut doc, json!({"op":"insert_break","at":{"p":one,"o":3},"kind":kind}));
        let first = doc.render_all().order[0];
        ops(&mut doc, json!({"op":"insert_toc","before":first}));
        let text = docxtor_core::export::to_text(&doc);
        assert!(text.contains(&format!("Two\t{page}")), "{kind}: {text}");
    }
}

// ---------------------------------------------------------------------------
// AI tools
// ---------------------------------------------------------------------------

#[test]
fn ai_insert_break_and_docscript() {
    let mut doc = doc_with("Alpha beta gamma\n\nSecond");
    let p = find_para(&doc, "Alpha");
    let r = call_tool(&mut doc, "insert_break", &json!({"id": format!("p{p}"), "after_text": "beta", "kind": "continuous"})).unwrap();
    assert!(r.contains("section_break=continuous"), "{r}");
    assert_eq!(texts(&doc)[0], "Alpha beta");
    assert_eq!(texts(&doc)[1], " gamma");
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains(&format!("[p{p} section_break=continuous] Alpha beta")), "{script}");
    assert_eq!(script.matches("section_break=").count(), 1);
    assert!(docxtor_core::ai::summary(&mut doc).contains("2 sections"));

    // Page and column breaks, and default position at the paragraph end.
    let second = find_para(&doc, "Second");
    let r = call_tool(&mut doc, "insert_break", &json!({"id": second, "kind": "page"})).unwrap();
    assert!(r.contains("<pagebreak>"), "{r}");
    let r = call_tool(&mut doc, "insert_break", &json!({"id": second, "after_text": "Sec", "kind": "column"})).unwrap();
    assert!(r.contains("Sec<columnbreak>ond"), "{r}");
    assert!(call_tool(&mut doc, "insert_break", &json!({"id": second, "kind": "wiggly"})).is_err());
    assert!(call_tool(&mut doc, "insert_break", &json!({"id": second, "after_text": "nope", "kind": "page"})).is_err());
}

#[test]
fn ai_set_page_layout() {
    let mut doc = doc_with("Alpha beta\n\nBravo");
    let a = find_para(&doc, "Alpha");
    let b = find_para(&doc, "Bravo");
    call_tool(&mut doc, "insert_break", &json!({"id": a, "after_text": "Alpha", "kind": "next_page"})).unwrap();
    // Landscape for the whole document.
    let r = call_tool(&mut doc, "set_page_layout", &json!({"orientation": "landscape", "margins_pt": {"left": 36, "right": 36}, "hyphenation": true, "page_color": "ffe9a8"})).unwrap();
    assert!(r.contains("Section 2"), "{r}");
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| (s.width, s.height) == (15840, 12240) && s.margin == [1440, 720, 1440, 720]), "{:?}", ps.sections);
    assert!(ps.hyphenation && ps.page_color.as_deref() == Some("FFE9A8"));
    // One section: columns with a line, line numbers; the other stays as it was.
    call_tool(&mut doc, "set_page_layout", &json!({"section": b, "orientation": "portrait", "columns": 2, "column_spacing_pt": 18, "column_line": true, "line_numbers": "each_section"})).unwrap();
    let ps = doc.page_setup();
    assert_eq!((ps.sections[0].width, ps.sections[1].width, ps.sections[1].height), (15840, 12240, 15840));
    assert_eq!((ps.sections[0].cols.num, ps.sections[1].cols.num, ps.sections[1].cols.space, ps.sections[1].cols.sep), (1, 2, 360, true));
    assert_eq!(ps.sections[1].line_numbers.as_ref().map(|l| l.restart.as_str()), Some("newSection"));
    assert!(ps.sections[0].line_numbers.is_none());
    // Only spacing: keeps the column count.
    call_tool(&mut doc, "set_page_layout", &json!({"section": b, "column_spacing_pt": 36})).unwrap();
    assert_eq!((doc.page_setup().sections[1].cols.num, doc.page_setup().sections[1].cols.space), (2, 720));
    assert!(docxtor_core::ai::summary(&mut doc).contains("2 columns"));
    call_tool(&mut doc, "set_page_layout", &json!({"line_numbers": "off", "columns": 1, "page_color": "none", "hyphenation": false})).unwrap();
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| s.line_numbers.is_none() && s.cols.num == 1) && !ps.hyphenation && ps.page_color.is_none());
    assert!(call_tool(&mut doc, "set_page_layout", &json!({})).is_err());
    assert!(call_tool(&mut doc, "set_page_layout", &json!({"columns": 9})).is_err());
    assert!(call_tool(&mut doc, "set_page_layout", &json!({"orientation": "sideways"})).is_err());
    assert_valid(&doc.save().unwrap());
}

#[test]
fn ai_format_paragraphs_layout_fields() {
    let mut doc = doc_with("Alpha\n\nBeta");
    let a = find_para(&doc, "Alpha");
    call_tool(&mut doc, "format_paragraphs", &json!({"ids": [a], "keep_with_next": true, "keep_lines_together": true, "page_break_before": true, "prevent_single_lines": false, "shading": "#eeeeee", "borders": "box"})).unwrap();
    let f = fmt_at(&doc, a);
    assert_eq!((f.keep_next, f.keep_lines, f.page_break_before, f.widow_control), (Some(true), Some(true), Some(true), Some(false)));
    assert_eq!(f.shading.as_deref(), Some("EEEEEE"));
    let b = f.borders.unwrap();
    assert!([&b.top, &b.bottom, &b.left, &b.right].iter().all(|s| s.as_ref().is_some_and(|s| s.sz == 4 && s.color == "auto" && s.space == 1 && s.val == "single")));
    call_tool(&mut doc, "format_paragraphs", &json!({"ids": [a], "borders": "bottom", "shading": "none"})).unwrap();
    let f = fmt_at(&doc, a);
    let b = f.borders.unwrap();
    assert!(b.top.is_none() && b.bottom.is_some() && f.shading.is_none());
    call_tool(&mut doc, "format_paragraphs", &json!({"ids": [a], "borders": "none"})).unwrap();
    assert!(fmt_at(&doc, a).borders.is_none());
    assert!(call_tool(&mut doc, "format_paragraphs", &json!({"ids": [a], "borders": "wavy"})).is_err());
    assert_valid(&doc.save().unwrap());
}

#[test]
fn render_changed_updates_the_paragraph_before_a_merged_section() {
    let mut doc = doc_with("X\n\nAlpha beta");
    let x = find_para(&doc, "X");
    let a = find_para(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":x,"o":1},"kind":"continuous"}));
    ops(&mut doc, json!({"op":"insert_break","at":{"p":a,"o":6},"kind":"even_page"}));
    let a2 = ids(&doc).into_iter().find(|id| doc_text(&doc, *id) == "beta").unwrap();
    assert_eq!(breaks(&doc).into_iter().find(|(p, _)| *p == x).unwrap().1.as_deref(), Some("continuous"));
    doc.render_all();
    // The delete only touches the two paragraphs it joins, yet X's break now shows the last section's type.
    ops(&mut doc, json!({"op":"delete","from":{"p":a,"o":6},"to":{"p":a2,"o":0}}));
    let out = doc.render_changed();
    let x_break = out.blocks.iter().find_map(|b| match b {
        RBlock::Para(p) if p.id == x => Some(p.sect_break.clone()),
        _ => None,
    });
    assert_eq!(x_break, Some(Some("evenPage".to_string())));
}

fn doc_text(doc: &Document, id: u32) -> String {
    docxtor_core::model::find_para(&doc.body, id).unwrap().plain_text()
}

#[test]
fn layout_changes_ask_for_a_new_page_setup() {
    let mut doc = doc_with("Alpha\n\nBravo");
    doc.render_all();
    let a = find_para(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_text","at":{"p":a,"o":5},"text":"!"}));
    assert!(!doc.render_changed().stories, "typing leaves the page setup alone");
    for op in [
        json!({"op":"set_line_numbers","on":true}),
        json!({"op":"set_page_color","color":"FFF2CC"}),
        json!({"op":"set_hyphenation","on":true}),
        json!({"op":"set_columns","count":2}),
        json!({"op":"set_page_setup","margins":[720,720,720,720]}),
    ] {
        ops(&mut doc, op.clone());
        assert!(doc.render_changed().stories, "{op}");
        assert!(doc.undo());
        assert!(doc.render_changed().stories, "undo of {op}");
        assert!(doc.redo());
        assert!(doc.render_changed().stories, "redo of {op}");
    }
}

#[test]
fn headers_and_first_pages_per_section() {
    let mut doc = doc_with("Alpha\n\nBravo\n\nCharlie");
    let b = find_para(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":0},"kind":"next_page"}));
    let c = find_para(&doc, "Charlie");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":c,"o":0},"kind":"next_page"}));
    let a = find_para(&doc, "Alpha");
    let b = find_para(&doc, "Bravo");
    let c = find_para(&doc, "Charlie");
    // A header for the middle section only: the last section inherits it, the first has none.
    let r = doc.apply_json(&json!({"op":"add_header_footer","kind":"header","section":b}).to_string(), false).unwrap();
    assert!(r[0].caret.is_some());
    let ps = doc.page_setup();
    assert!(ps.sections[0].headers.is_empty());
    let part = ps.sections[1].headers["default"].clone();
    assert_eq!(ps.sections[2].headers["default"], part);
    assert!(ps.headers.is_empty(), "the last section has no header of its own");
    // Asking again finds it rather than making another.
    let r2 = doc.apply_json(&json!({"op":"add_header_footer","kind":"header","section":b}).to_string(), false).unwrap();
    assert_eq!(r2[0].caret, r[0].caret);
    // The last section gets its own, which stops the inheritance there.
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","section":c}));
    let ps = doc.page_setup();
    assert_ne!(ps.sections[2].headers["default"], part);
    assert_eq!(ps.sections[1].headers["default"], part);
    // A different first page for the first section only.
    ops(&mut doc, json!({"op":"set_title_page","on":true,"section":a}));
    let ps = doc.page_setup();
    assert!(ps.sections[0].title_page && !ps.sections[1].title_page && !ps.sections[2].title_page);
    ops(&mut doc, json!({"op":"add_header_footer","kind":"footer","variant":"first","section":a}));
    assert!(doc.page_setup().sections[0].footers.contains_key("first"));
    // Removing the middle section's header leaves the others.
    ops(&mut doc, json!({"op":"remove_header_footer","kind":"header","section":b}));
    let ps = doc.page_setup();
    assert!(ps.sections[1].headers.is_empty() && ps.sections[2].headers.contains_key("default"));
    assert!(doc.apply_json(&json!({"op":"remove_header_footer","kind":"header","section":b}).to_string(), false).is_err());
    // Without a section the ops still act on the last one.
    ops(&mut doc, json!({"op":"set_title_page","on":true}));
    assert!(doc.page_setup().sections[2].title_page);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let again = Document::open(&bytes).unwrap().page_setup();
    assert!(again.sections[0].title_page && again.sections[0].footers.contains_key("first"));
    // The AI tool takes a section too.
    let mut doc = doc_with("Alpha\n\nBravo");
    let b = find_para(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":0},"kind":"next_page"}));
    let b = find_para(&doc, "Bravo");
    call_tool(&mut doc, "set_header_footer", &json!({"kind":"footer","text":"Page {PAGE}","section":format!("p{b}")})).unwrap();
    let ps = doc.page_setup();
    assert!(ps.sections[0].footers.is_empty() && ps.sections[1].footers.contains_key("default"));
}

#[test]
fn page_numbering_per_section() {
    let mut doc = doc_with("Alpha\n\nBravo");
    let b = find_para(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":0},"kind":"next_page"}));
    let a = find_para(&doc, "Alpha");
    let b = find_para(&doc, "Bravo");
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| s.page_start.is_none() && s.page_format.is_none()));
    ops(&mut doc, json!({"op":"set_page_numbering","section":a,"format":"lowerRoman"}));
    ops(&mut doc, json!({"op":"set_page_numbering","section":b,"start":1}));
    let ps = doc.page_setup();
    assert_eq!((ps.sections[0].page_start, ps.sections[0].page_format.as_deref()), (None, Some("lowerRoman")));
    assert_eq!((ps.sections[1].page_start, ps.sections[1].page_format.as_deref()), (Some(1), None));
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:pgNumType w:fmt="lowerRoman"/>"#) && xml.contains(r#"<w:pgNumType w:start="1"/>"#), "{xml}");
    // Continuing and decimal remove what they don't need.
    ops(&mut doc, json!({"op":"set_page_numbering","section":a,"format":"decimal"}));
    ops(&mut doc, json!({"op":"set_page_numbering","section":b}));
    assert!(!saved_part(&mut doc, "word/document.xml").contains("pgNumType"));
    assert!(doc.apply_json(&json!({"op":"set_page_numbering","format":"klingon"}).to_string(), false).is_err());
    // Undo brings it back and tells the editor the page setup changed.
    doc.render_all();
    ops(&mut doc, json!({"op":"set_page_numbering","section":b,"start":5,"format":"upperRoman"}));
    assert!(doc.render_changed().stories);
    assert!(doc.undo());
    assert!(doc.render_changed().stories);
    assert_eq!(doc.page_setup().sections[1].page_start, None);
    // The AI tool.
    call_tool(&mut doc, "set_page_layout", &json!({"section":format!("p{b}"),"page_numbers_start":3,"page_number_format":"upperLetter"})).unwrap();
    let ps = doc.page_setup();
    assert_eq!((ps.sections[1].page_start, ps.sections[1].page_format.as_deref()), (Some(3), Some("upperLetter")));
    assert_eq!(ps.sections[0].page_start, None);
    assert_valid(&doc.save().unwrap());
}
