//! Page layout: section layout (gutter, vertical alignment, text direction), page borders, unequal columns,
//! chapter page numbers, note options per section, layout settings, the page background, frames, drop caps and
//! notes with a custom mark. Each round-trips, writes what Word writes, undoes and stays valid.

use docxtor_core::model::{walk_paragraphs, write_block, find_block};
use docxtor_core::render::RBlock;
use docxtor_core::{package::Package, Document};
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

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn find_p(doc: &Document, needle: &str) -> u32 {
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

fn saved(doc: &mut Document, name: &str) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

fn para_xml(doc: &Document, id: u32) -> String {
    write_block(find_block(&doc.body, id).unwrap()).to_xml()
}

/// Two sections: "Alpha" then "Bravo" after a next-page break.
fn two_sections() -> Document {
    let mut doc = doc_with("Alpha text\n\nBravo text");
    let b = find_p(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":0},"kind":"next_page"}));
    doc
}

// ---------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------

#[test]
fn section_layout_is_written_read_and_undone() {
    let mut doc = two_sections();
    let alpha = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"set_section","section":alpha,"gutter":720,"header":500,"footer":600,"v_align":"center","text_direction":"tbRl"}));
    let ps = doc.page_setup();
    let s = &ps.sections[0];
    assert_eq!((s.gutter, s.header, s.footer), (720, 500, 600));
    assert_eq!(s.extras.v_align.as_deref(), Some("center"));
    assert_eq!(s.extras.text_direction.as_deref(), Some("tbRl"));
    // The other section is untouched.
    assert_eq!(ps.sections[1].gutter, 0);
    assert!(ps.sections[1].extras.v_align.is_none());
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"w:header="500""#) && xml.contains(r#"w:footer="600""#) && xml.contains(r#"w:gutter="720""#), "{xml}");
    assert!(xml.contains(r#"<w:vAlign w:val="center"/>"#) && xml.contains(r#"<w:textDirection w:val="tbRl"/>"#), "{xml}");
    // Schema order: vAlign before titlePg and textDirection, after cols.
    assert!(xml.find("<w:cols").unwrap() < xml.find("<w:vAlign").unwrap());
    assert!(xml.find("<w:vAlign").unwrap() < xml.find("<w:textDirection").unwrap());
    // Back to the defaults removes the elements.
    ops(&mut doc, json!({"op":"set_section","section":alpha,"v_align":"top","text_direction":"lrTb","rtl_gutter":true}));
    let s = &doc.page_setup().sections[0];
    assert!(s.extras.v_align.is_none() && s.extras.text_direction.is_none() && s.rtl_gutter);
    assert!(doc.undo() && doc.undo());
    assert!(doc.page_setup().sections[0].extras.v_align.is_none());
    assert!(err(&mut doc, json!({"op":"set_section","v_align":"middle"})).contains("Vertical alignment"));
    assert!(err(&mut doc, json!({"op":"set_section","gutter":-1})).contains("negative"));
}

#[test]
fn section_changes_can_go_from_a_point_forward() {
    let mut doc = two_sections();
    let alpha = find_p(&doc, "Alpha");
    let bravo = find_p(&doc, "Bravo");
    ops(&mut doc, json!({"op":"set_section","section":bravo,"from":true,"v_align":"bottom"}));
    let ps = doc.page_setup();
    assert!(ps.sections[0].extras.v_align.is_none());
    assert_eq!(ps.sections[1].extras.v_align.as_deref(), Some("bottom"));
    ops(&mut doc, json!({"op":"set_section","section":alpha,"from":true,"v_align":"both"}));
    let ps = doc.page_setup();
    assert!(ps.sections.iter().all(|s| s.extras.v_align.as_deref() == Some("both")));
    // The way a section starts.
    ops(&mut doc, json!({"op":"set_section","section":bravo,"start":"oddPage"}));
    assert_eq!(doc.page_setup().sections[1].start, "oddPage");
}

#[test]
fn page_borders_all_sides_art_and_removal() {
    let mut doc = two_sections();
    let alpha = find_p(&doc, "Alpha");
    let side = |style: &str| json!({"val":style,"sz":12,"space":24,"color":"FF4F00"});
    ops(&mut doc, json!({"op":"set_page_borders","section":alpha,"borders":{"offsetFrom":"text","display":"firstPage","zOrder":"back","top":side("double"),"left":side("double"),"bottom":{"val":"single","sz":4,"space":24,"color":"auto","shadow":true},"right":side("hearts")}}));
    let b = doc.page_setup().sections[0].extras.page_borders.clone().unwrap();
    assert_eq!((b.offset_from.as_str(), b.display.as_str(), b.z_order.as_str()), ("text", "firstPage", "back"));
    assert_eq!(b.top.as_ref().unwrap().val, "double");
    assert!(b.right.as_ref().unwrap().is_art() && !b.top.as_ref().unwrap().is_art());
    assert!(b.bottom.as_ref().unwrap().shadow);
    assert!(doc.page_setup().sections[1].extras.page_borders.is_none());
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:pgBorders w:zOrder="back" w:display="firstPage" w:offsetFrom="text">"#), "{xml}");
    assert!(xml.contains(r#"<w:top w:val="double" w:sz="12" w:space="24" w:color="FF4F00"/>"#), "{xml}");
    // Top, left, bottom, right in schema order, before the page number type and columns.
    let at = |s: &str| xml.find(s).unwrap_or_else(|| panic!("{s} in {xml}"));
    assert!(at("<w:top ") < at("<w:left ") && at("<w:left ") < at("<w:bottom ") && at("<w:bottom ") < at("<w:right "));
    assert!(at("<w:pgBorders") < at("<w:cols"));
    // It survives a reopen.
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(again.page_setup().sections[0].extras.page_borders, Some(b));
    // Removal, and undo.
    ops(&mut doc, json!({"op":"set_page_borders","section":alpha,"borders":null}));
    assert!(doc.page_setup().sections[0].extras.page_borders.is_none());
    assert!(!saved(&mut doc, "word/document.xml").contains("pgBorders"));
    assert!(doc.undo());
    assert!(doc.page_setup().sections[0].extras.page_borders.is_some());
    assert!(err(&mut doc, json!({"op":"set_page_borders","borders":{"offsetFrom":"sky","top":side("single")}})).contains("count from"));
}

#[test]
fn unequal_columns_are_written_with_their_spaces() {
    let mut doc = doc_with("Alpha text\n\nBravo text");
    ops(&mut doc, json!({"op":"set_columns","count":2,"widths":[3000,5400],"spaces":[600]}));
    let c = doc.page_setup().sections[0].cols.clone();
    assert_eq!((c.num, c.widths.clone(), c.spaces.clone()), (2, vec![3000, 5400], vec![600]));
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"w:equalWidth="0""#) && xml.contains(r#"<w:col w:w="3000" w:space="600"/><w:col w:w="5400"/>"#), "{xml}");
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(again.page_setup().sections[0].cols.widths, vec![3000, 5400]);
    // Equal columns again.
    ops(&mut doc, json!({"op":"set_columns","count":2}));
    assert!(doc.page_setup().sections[0].cols.widths.is_empty());
    assert!(!saved(&mut doc, "word/document.xml").contains("equalWidth"));
    assert!(doc.undo());
    assert_eq!(doc.page_setup().sections[0].cols.widths, vec![3000, 5400]);
    assert!(err(&mut doc, json!({"op":"set_columns","count":3,"widths":[3000,5000]})).contains("one width"));
    assert!(err(&mut doc, json!({"op":"set_columns","count":2,"widths":[100,5000]})).contains("one width"));
}

#[test]
fn chapter_page_numbers_are_written_and_removed() {
    let mut doc = doc_with("# One\n\nText");
    ops(&mut doc, json!({"op":"set_page_numbering","chap_style":1,"chap_sep":"enDash","format":"decimal"}));
    let s = &doc.page_setup().sections[0];
    assert_eq!((s.extras.chap_style, s.extras.chap_sep.as_deref()), (Some(1), Some("enDash")));
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:pgNumType w:chapStyle="1" w:chapSep="enDash"/>"#), "{xml}");
    // The hyphen is the default and isn't written.
    ops(&mut doc, json!({"op":"set_page_numbering","chap_sep":"hyphen"}));
    assert!(!saved(&mut doc, "word/document.xml").contains("chapSep"));
    ops(&mut doc, json!({"op":"set_page_numbering","chap_style":0}));
    assert!(doc.page_setup().sections[0].extras.chap_style.is_none());
    assert!(!saved(&mut doc, "word/document.xml").contains("pgNumType"));
    assert!(err(&mut doc, json!({"op":"set_page_numbering","chap_style":12})).contains("1 to 9"));
    assert!(err(&mut doc, json!({"op":"set_page_numbering","chap_style":1,"chap_sep":"dot"})).contains("separator"));
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[test]
fn layout_settings_round_trip_in_order() {
    let mut doc = doc_with("Alpha text");
    ops(&mut doc, json!({"op":"set_layout_settings","mirror_margins":true,"gutter_at_top":true,"even_and_odd_headers":true,"hyphenation_zone":360,"consecutive_hyphen_limit":2,"do_not_hyphenate_caps":true,"print_two_on_one":true}));
    let ps = doc.page_setup();
    assert!(ps.layout.mirror_margins && ps.layout.gutter_at_top && ps.even_and_odd_headers && ps.layout.print_two_on_one && ps.layout.do_not_hyphenate_caps);
    assert_eq!((ps.layout.hyphenation_zone, ps.layout.consecutive_hyphen_limit), (Some(360), Some(2)));
    let xml = saved(&mut doc, "word/settings.xml");
    let at = |s: &str| xml.find(s).unwrap_or_else(|| panic!("{s} in {xml}"));
    assert!(at("<w:mirrorMargins") < at("<w:gutterAtTop") && at("<w:gutterAtTop") < at("<w:defaultTabStop"));
    assert!(at("<w:consecutiveHyphenLimit") < at("<w:hyphenationZone") && at("<w:hyphenationZone") < at("<w:doNotHyphenateCaps"));
    assert!(at("<w:evenAndOddHeaders") < at("<w:printTwoOnOne"));
    ops(&mut doc, json!({"op":"set_layout_settings","book_fold":"bookFold","book_fold_sheets":4}));
    assert_eq!(doc.page_setup().layout.book_fold.as_deref(), Some("bookFold"));
    ops(&mut doc, json!({"op":"set_layout_settings","book_fold":"none","mirror_margins":false,"hyphenation_zone":0}));
    let ps = doc.page_setup();
    assert!(ps.layout.book_fold.is_none() && ps.layout.book_fold_sheets.is_none() && !ps.layout.mirror_margins && ps.layout.hyphenation_zone.is_none());
    let xml = saved(&mut doc, "word/settings.xml");
    assert!(!xml.contains("bookFold") && !xml.contains("mirrorMargins") && !xml.contains("hyphenationZone"), "{xml}");
    assert!(doc.undo() && doc.undo());
    assert!(doc.page_setup().layout.mirror_margins);
    assert!(err(&mut doc, json!({"op":"set_layout_settings","hyphenation_zone":-5})).contains("hyphenation zone"));
    assert!(err(&mut doc, json!({"op":"set_layout_settings","book_fold":"spine"})).contains("Book fold"));
}

#[test]
fn an_even_page_header_can_be_made() {
    let mut doc = doc_with("Alpha text");
    ops(&mut doc, json!({"op":"set_layout_settings","even_and_odd_headers":true}));
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","variant":"even"}));
    let ps = doc.page_setup();
    assert!(ps.sections[0].headers.contains_key("even"));
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:headerReference w:type="even""#), "{xml}");
}

// ---------------------------------------------------------------------------
// The page background
// ---------------------------------------------------------------------------

#[test]
fn page_background_gradient_texture_and_picture() {
    let mut doc = doc_with("Alpha text");
    ops(&mut doc, json!({"op":"set_page_background","fill":{"kind":"gradient","color":"FFF2CC","color2":"9CCBF0","angle":-90}}));
    let f = doc.page_setup().page_fill.clone().unwrap();
    assert_eq!((f.kind.as_str(), f.color.as_deref(), f.color2.as_deref(), f.angle), ("gradient", Some("FFF2CC"), Some("9CCBF0"), Some(-90.0)));
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains("xmlns:v=") && xml.contains("xmlns:o="), "{xml}");
    assert!(xml.contains(r##"<v:fill type="gradient" color2="#9CCBF0" angle="-90""##), "{xml}");
    assert!(xml.find("<w:background").unwrap() < xml.find("<w:body").unwrap());
    // Word shows the background only when the settings say so.
    assert!(saved(&mut doc, "word/settings.xml").contains("displayBackgroundShape"));
    // A texture is an image related from the document.
    let media = doc.add_media("png", vec![137, 80, 78, 71, 13, 10, 26, 10]);
    ops(&mut doc, json!({"op":"set_page_background","fill":{"kind":"texture","color":"FFFFFF","media":media,"title":"Paper bag"}}));
    let f = doc.page_setup().page_fill.clone().unwrap();
    assert_eq!((f.kind.as_str(), f.media.as_deref(), f.title.as_deref()), ("texture", Some(media.as_str()), Some("Paper bag")));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains(r#"type="tile""#) && xml.contains(r#"o:title="Paper bag""#) && xml.contains("r:id="), "{xml}");
    let rels = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
    assert!(rels.contains(&media.replace("word/", "")), "{rels}");
    assert!(pkg.get(&media).is_some());
    // The picture and pattern kinds.
    ops(&mut doc, json!({"op":"set_page_background","fill":{"kind":"picture","media":media}}));
    assert!(saved(&mut doc, "word/document.xml").contains(r#"type="frame""#));
    ops(&mut doc, json!({"op":"set_page_background","fill":{"kind":"pattern","color":"000000","color2":"FFFFFF","media":media,"title":"5%"}}));
    assert!(saved(&mut doc, "word/document.xml").contains(r#"type="pattern""#));
    // Radial from a corner.
    ops(&mut doc, json!({"op":"set_page_background","fill":{"kind":"radial","color":"FFFFFF","color2":"FF4F00","from":"topLeft"}}));
    let f = doc.page_setup().page_fill.clone().unwrap();
    assert_eq!((f.kind.as_str(), f.from.as_deref()), ("radial", Some("topLeft")));
    // A plain colour is still the page colour.
    ops(&mut doc, json!({"op":"set_page_background","fill":{"kind":"solid","color":"fff2cc"}}));
    let ps = doc.page_setup();
    assert_eq!(ps.page_color.as_deref(), Some("FFF2CC"));
    assert!(!saved(&mut doc, "word/document.xml").contains("<v:fill"));
    // Removal and undo.
    ops(&mut doc, json!({"op":"set_page_background","fill":null}));
    assert!(doc.page_setup().page_fill.is_none());
    assert!(!saved(&mut doc, "word/document.xml").contains("w:background"));
    assert!(doc.undo());
    assert_eq!(doc.page_setup().page_color.as_deref(), Some("FFF2CC"));
    assert!(err(&mut doc, json!({"op":"set_page_background","fill":{"kind":"gradient","color":"FFFFFF"}})).contains("two colours"));
    assert!(err(&mut doc, json!({"op":"set_page_background","fill":{"kind":"texture"}})).contains("image"));
    assert!(err(&mut doc, json!({"op":"set_page_background","fill":{"kind":"picture","media":"word/media/nope.png"}})).contains("not found"));
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

fn doc_with_notes() -> Document {
    let mut doc = doc_with("Alpha text here\n\nBravo text here");
    let (a, b) = (find_p(&doc, "Alpha"), find_p(&doc, "Bravo"));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":a,"o":5},"text":"First."}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":a,"o":10},"text":"Second."}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":b,"o":5},"text":"Third."}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":b,"o":9},"text":"Ending."}));
    let b = find_p(&doc, "Bravo");
    ops(&mut doc, json!({"op":"insert_break","at":{"p":b,"o":0},"kind":"next_page"}));
    doc
}

fn labels(doc: &Document) -> Vec<String> {
    doc.page_setup().notes.iter().map(|n| n.label.clone()).collect()
}

#[test]
fn notes_restart_in_each_section_and_follow_the_section_format() {
    let mut doc = doc_with_notes();
    assert_eq!(labels(&doc), ["1", "2", "3"]);
    ops(&mut doc, json!({"op":"set_note_props","kind":"footnote","restart":"eachSect"}));
    assert_eq!(labels(&doc), ["1", "2", "1"]);
    let ps = doc.page_setup();
    assert_eq!(ps.sections[0].extras.footnote_pr.restart, "eachSect");
    assert_eq!(ps.layout.footnote_pr.restart, "eachSect");
    let xml = saved(&mut doc, "word/settings.xml");
    assert!(xml.contains(r#"<w:numRestart w:val="eachSect"/>"#), "{xml}");
    // The text's own marks show the same labels.
    let r = doc.render_snapshot();
    let mut marks = Vec::new();
    for b in &r.blocks {
        if let RBlock::Para(p) = b {
            for s in &p.segs {
                if s.k == "note" && s.note.is_none() {
                    marks.push(s.label.clone().unwrap());
                }
            }
        }
    }
    assert_eq!(marks, ["1", "2", "1"]);
    // One section its own way: lower-case letters from c, starting again.
    let bravo = find_p(&doc, "Bravo");
    ops(&mut doc, json!({"op":"set_note_props","kind":"footnote","section":bravo,"format":"lowerLetter","start":3}));
    assert_eq!(labels(&doc), ["1", "2", "c"]);
    let xml = saved(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:footnotePr><w:numFmt w:val="lowerLetter"/><w:numStart w:val="3"/>"#), "{xml}");
    // The note's own mark says the same.
    let ps = doc.page_setup();
    let third = ps.notes.iter().find(|n| n.label == "c").unwrap();
    let mark = match &third.blocks[0] {
        RBlock::Para(p) => p.segs.iter().find(|s| s.k == "note").and_then(|s| s.label.clone()),
        _ => None,
    };
    assert_eq!(mark.as_deref(), Some("c"));
    assert!(doc.undo());
    assert_eq!(labels(&doc), ["1", "2", "1"]);
    // Page restarts are the layout's job; the file says so and the engine counts on.
    ops(&mut doc, json!({"op":"set_note_props","kind":"footnote","restart":"eachPage"}));
    assert_eq!(doc.page_setup().layout.footnote_pr.restart, "eachPage");
    assert_eq!(labels(&doc), ["1", "2", "3"]);
}

#[test]
fn note_position_and_validation() {
    let mut doc = doc_with_notes();
    ops(&mut doc, json!({"op":"set_note_props","kind":"footnote","pos":"beneathText"}));
    ops(&mut doc, json!({"op":"set_note_props","kind":"endnote","pos":"sectEnd","format":"decimal"}));
    let ps = doc.page_setup();
    assert_eq!(ps.layout.footnote_pr.pos, "beneathText");
    assert_eq!((ps.layout.endnote_pr.pos.as_str(), ps.layout.endnote_pr.fmt.as_str()), ("sectEnd", "decimal"));
    let xml = saved(&mut doc, "word/settings.xml");
    assert!(xml.contains(r#"<w:pos w:val="beneathText"/>"#) && xml.contains(r#"<w:pos w:val="sectEnd"/>"#), "{xml}");
    assert!(err(&mut doc, json!({"op":"set_note_props","kind":"footnote","pos":"sectEnd"})).contains("pageBottom"));
    assert!(err(&mut doc, json!({"op":"set_note_props","kind":"endnote","pos":"pageBottom"})).contains("end of the section"));
    assert!(err(&mut doc, json!({"op":"set_note_props","kind":"footnote","restart":"never"})).contains("restart"));
    assert!(err(&mut doc, json!({"op":"set_note_props","kind":"footnote","start":0})).contains("1 or more"));
}

#[test]
fn a_note_with_a_custom_mark() {
    let mut doc = doc_with("Alpha text here");
    let a = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":a,"o":2},"text":"Numbered."}));
    let r = ops(&mut doc, json!({"op":"insert_custom_note","at":{"p":a,"o":9},"kind":"footnote","mark":"*","text":"Starred."}));
    assert!(r[0].caret.is_some());
    // The numbered note stays 1; the starred one takes no number.
    assert_eq!(labels(&doc), ["1", ""]);
    let xml = para_xml(&doc, a);
    assert!(xml.contains(r#"w:customMarkFollows="1""#), "{xml}");
    assert!(xml.contains("<w:t>*</w:t>"), "{xml}");
    // The mark is in the note, in place of the number.
    let notes = saved(&mut doc, "word/footnotes.xml");
    assert!(notes.contains("<w:t>*</w:t>") && notes.matches("<w:footnoteRef/>").count() == 1, "{notes}");
    // The reference in the text shows nothing of its own; the mark that follows is ordinary text.
    let rendered = doc.render_snapshot();
    let RBlock::Para(p) = &rendered.blocks[0] else { panic!() };
    let notes: Vec<_> = p.segs.iter().filter(|s| s.k == "note").map(|s| s.label.clone().unwrap()).collect();
    assert_eq!(notes, ["1", ""]);
    assert!(texts(&doc)[0].contains('*'));
    // Endnotes too, and a bad mark is refused.
    ops(&mut doc, json!({"op":"insert_custom_note","at":{"p":a,"o":1},"kind":"endnote","mark":"†","text":"Dagger."}));
    assert!(saved(&mut doc, "word/endnotes.xml").contains("<w:t>†</w:t>"));
    assert!(err(&mut doc, json!({"op":"insert_custom_note","at":{"p":a,"o":1},"kind":"footnote","mark":"  "})).contains("mark"));
    // Undo takes the whole note out.
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(labels(&doc), ["1"]);
    assert!(!texts(&doc)[0].contains('*'));
}

#[test]
fn custom_separators_are_read() {
    let mut doc = doc_with("Alpha text here");
    let a = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":a,"o":2},"text":"Note."}));
    // Word's own separators are not reported.
    assert!(doc.page_setup().note_seps.is_none());
    let bytes = doc.save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    let part = String::from_utf8(pkg.get("word/footnotes.xml").unwrap().to_vec()).unwrap();
    let part = part
        .replace(r#"<w:r><w:separator/></w:r>"#, r#"<w:r><w:separator/></w:r><w:r><w:t>Notes</w:t></w:r>"#)
        .replace(r#"<w:footnote w:type="continuationSeparator" w:id="0">"#, r#"<w:footnote w:type="continuationNotice" w:id="1"><w:p><w:r><w:t>Continued</w:t></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0">"#);
    pkg.set("word/footnotes.xml", part.into_bytes());
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let seps = doc.page_setup().note_seps.unwrap().footnote.unwrap();
    let sep = seps.separator.unwrap();
    assert!(sep.line);
    assert!(matches!(&sep.blocks[0], RBlock::Para(p) if p.len == 5));
    let notice = seps.notice.unwrap();
    assert!(!notice.line && matches!(&notice.blocks[0], RBlock::Para(p) if p.len == 9));
    // The separators don't turn into notes.
    assert_eq!(doc.page_setup().notes.len(), 1);
}

// ---------------------------------------------------------------------------
// Frames and drop caps
// ---------------------------------------------------------------------------

#[test]
fn a_frame_is_written_read_and_removed() {
    let mut doc = doc_with("Sidebar text\n\nBody text");
    let side = find_p(&doc, "Sidebar");
    ops(&mut doc, json!({"op":"set_frame","ids":[side],"frame":{"w":2400,"h":1200,"hRule":"atLeast","wrap":"around","hAnchor":"margin","vAnchor":"text","xAlign":"right","hSpace":144,"vSpace":72}}));
    let xml = para_xml(&doc, side);
    assert!(xml.contains(r#"w:w="2400""#) && xml.contains(r#"w:xAlign="right""#) && xml.contains(r#"w:hRule="atLeast""#), "{xml}");
    let r = doc.render_snapshot();
    let RBlock::Para(p) = &r.blocks[0] else { panic!() };
    let f = p.fmt.frame.clone().unwrap();
    assert_eq!((f.w, f.x_align.as_deref(), f.h_space, f.wrap.as_deref()), (Some(2400), Some("right"), Some(144), Some("around")));
    saved(&mut doc, "word/document.xml");
    ops(&mut doc, json!({"op":"set_frame","ids":[side],"frame":null}));
    assert!(!para_xml(&doc, side).contains("framePr"));
    assert!(doc.undo());
    assert!(para_xml(&doc, side).contains("framePr"));
    assert!(err(&mut doc, json!({"op":"set_frame","ids":[side],"frame":{"wrap":"sideways"}})).contains("Wrapping"));
}

#[test]
fn a_drop_cap_moves_the_first_letter_into_a_frame() {
    let mut doc = doc_with("Lorem ipsum dolor sit amet.\n\nSecond paragraph.");
    let p = find_p(&doc, "Lorem");
    let r = ops(&mut doc, json!({"op":"set_drop_cap","id":p,"kind":"drop","lines":3,"distance":100}));
    assert_eq!(r[0].created.len(), 1);
    // The letter is a paragraph of its own, in front of the text.
    assert_eq!(texts(&doc)[..2], ["L".to_string(), "orem ipsum dolor sit amet.".to_string()]);
    let frame = r[0].created[0];
    let xml = para_xml(&doc, frame);
    assert!(xml.contains(r#"w:dropCap="drop""#) && xml.contains(r#"w:lines="3""#) && xml.contains(r#"w:hSpace="100""#) && xml.contains(r#"w:wrap="around""#), "{xml}");
    assert!(xml.contains(r#"w:lineRule="exact""#) && xml.contains(r#"<w:textAlignment w:val="baseline"/>"#), "{xml}");
    assert!(xml.contains("<w:position w:val=\"-8\"/>") && xml.contains("<w:sz w:val=\"110\"/>") || xml.contains("<w:sz w:val="), "{xml}");
    assert!(xml.find("<w:framePr").unwrap() < xml.find("<w:spacing").unwrap());
    // The text paragraph keeps its id and carries no frame.
    assert!(!para_xml(&doc, p).contains("framePr"));
    let out = saved(&mut doc, "word/document.xml");
    assert!(out.contains("w:dropCap=\"drop\""));
    // More lines, in the margin, in another font.
    ops(&mut doc, json!({"op":"set_drop_cap","id":p,"kind":"margin","lines":4,"font":"Georgia"}));
    let xml = para_xml(&doc, frame);
    assert!(xml.contains(r#"w:dropCap="margin""#) && xml.contains(r#"w:lines="4""#) && xml.contains(r#"w:ascii="Georgia""#), "{xml}");
    assert_eq!(texts(&doc).iter().filter(|t| t.as_str() == "L").count(), 1);
    // The renderer reports the frame for the editor to float.
    let rendered = doc.render_snapshot();
    let RBlock::Para(fp) = &rendered.blocks[0] else { panic!() };
    assert_eq!(fp.fmt.frame.as_ref().and_then(|f| f.drop_cap.as_deref()), Some("margin"));
    // None puts the letter back, without its big size.
    ops(&mut doc, json!({"op":"set_drop_cap","id":p,"kind":"none"}));
    assert_eq!(texts(&doc)[0], "Lorem ipsum dolor sit amet.");
    let xml = para_xml(&doc, p);
    assert!(!xml.contains("<w:sz ") && !xml.contains("w:position"), "{xml}");
    // Undo brings the frame back, and the document stays valid throughout.
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "L");
    assert!(doc.undo() && doc.undo());
    assert_eq!(texts(&doc)[0], "Lorem ipsum dolor sit amet.");
    saved(&mut doc, "word/document.xml");
    assert!(err(&mut doc, json!({"op":"set_drop_cap","id":p,"kind":"drop","lines":11})).contains("1 to 10"));
    assert!(err(&mut doc, json!({"op":"set_drop_cap","id":p,"kind":"fancy"})).contains("none"));
}

#[test]
fn a_drop_cap_on_a_paragraph_with_a_section_break_and_formatting() {
    let mut doc = doc_with("Alpha text\n\nBravo text");
    let alpha = find_p(&doc, "Alpha");
    // Alpha ends the first section: its break stays with the text, and the letter keeps the bold.
    ops(&mut doc, json!({"op":"insert_break","at":{"p":alpha,"o":10},"kind":"next_page"}));
    ops(&mut doc, json!({"op":"format_text","from":{"p":alpha,"o":0},"to":{"p":alpha,"o":5},"format":{"bold":true}}));
    ops(&mut doc, json!({"op":"set_drop_cap","id":alpha,"kind":"drop"}));
    assert!(para_xml(&doc, alpha).contains("<w:sectPr"));
    assert_eq!(texts(&doc)[0], "A");
    let ids = docxtor_core::model::paragraph_order(&doc.body);
    let fx = para_xml(&doc, ids[0]);
    assert!(!fx.contains("sectPr") && fx.contains("<w:b/>"), "{fx}");
    assert_eq!(doc.page_setup().sections.len(), 2);
    saved(&mut doc, "word/document.xml");
}

/// Found by LibreOffice (scripts/lo-check.mjs on the feature files): a custom note put in a header made a file LibreOffice
/// cannot load. Footnotes and endnotes were already main-text only; the custom note was not.
#[test]
fn a_custom_note_cannot_go_in_a_header() {
    let mut doc = doc_with("Alpha text here");
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","variant":"default"}));
    let mut header = None;
    for s in &doc.stories {
        walk_paragraphs(&s.blocks, &mut |p| {
            header.get_or_insert(p.id);
        });
    }
    let at = json!({"p": header.expect("the header has a paragraph"), "o": 0});
    for kind in ["footnote", "endnote"] {
        let e = err(&mut doc, json!({"op":"insert_custom_note","at":at,"kind":kind,"mark":"*","text":"x"}));
        assert!(e.contains("main text"), "{e}");
    }
}
