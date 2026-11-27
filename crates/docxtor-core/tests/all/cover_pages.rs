//! Cover pages, and the ready-made headers and footers: each design goes in, shows the document's properties, writes
//! what Word writes, replaces or is removed, undoes and stays valid; and the gallery preview has a page per design.

use docxtor_core::model::walk_paragraphs;
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

fn doc_with(text: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":text}));
    doc
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn saved(doc: &mut Document) -> Package {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    Package::open(&bytes).unwrap()
}

fn part(pkg: &Package, name: &str) -> String {
    String::from_utf8(pkg.get(name).unwrap().to_vec()).unwrap()
}

fn props(doc: &mut Document) {
    ops(doc, json!({"op":"set_doc_props","title":"Field notes","author":"Ada Lovelace","subject":"A season on the coast"}));
}

#[test]
fn every_cover_goes_first_shows_the_properties_and_round_trips() {
    for d in Document::page_gallery("cover") {
        let mut doc = doc_with("Body text");
        props(&mut doc);
        ops(&mut doc, json!({"op":"insert_cover_page","design":d.id}));
        assert!(doc.has_cover_page(), "{}", d.id);
        let pkg = saved(&mut doc);
        let xml = part(&pkg, "word/document.xml");
        assert!(xml.contains(r#"<w:docPartGallery w:val="Cover Pages"/>"#), "{}", d.id);
        assert!(xml.contains("Field notes"), "{} shows the title", d.id);
        assert!(xml.contains("<w:titlePg/>"), "{}: no header or page number on the cover", d.id);
        assert!(xml.contains(r#"w:storeItemID="{6C3C8BC8-F283-45AE-878A-BAB7291924A1}""#), "{}: bound to the core properties", d.id);
        assert_eq!(texts(&doc).last().map(String::as_str), Some("Body text"));
        let again = Document::open(&doc.save().unwrap()).unwrap();
        assert!(again.has_cover_page(), "{} is still a cover page when opened again", d.id);
    }
}

#[test]
fn a_date_box_makes_the_cover_properties_part_as_word_does() {
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    let pkg = saved(&mut doc);
    let item = pkg.parts.iter().find(|p| p.name.starts_with("customXml/itemProps")).map(|p| String::from_utf8(p.data.clone()).unwrap()).expect("a properties part");
    assert!(item.contains("{55AF091B-3C7A-41E3-B477-F2FDAA23CFDA}"));
    assert!(part(&pkg, "word/document.xml").contains("ns0:PublishDate"));
}

#[test]
fn a_new_cover_replaces_the_old_and_remove_and_undo_work() {
    let mut doc = doc_with("Body text");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.band"}));
    let covers = part(&saved(&mut doc), "word/document.xml").matches("Cover Pages").count();
    assert_eq!(covers, 1, "one cover page");
    ops(&mut doc, json!({"op":"remove_cover_page"}));
    assert!(!doc.has_cover_page());
    assert!(!part(&saved(&mut doc), "word/document.xml").contains("<w:titlePg/>"), "the different first page goes with it");
    assert!(doc.undo());
    assert!(doc.has_cover_page());
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(!doc.has_cover_page());
    assert_eq!(texts(&doc), vec!["Body text".to_string()]);
}

#[test]
fn a_centred_cover_is_a_section_of_its_own() {
    let mut doc = doc_with("Body text");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.centered"}));
    let xml = part(&saved(&mut doc), "word/document.xml");
    assert!(xml.contains(r#"<w:vAlign w:val="center"/>"#));
    assert_eq!(xml.matches("<w:sectPr").count(), 2);
    ops(&mut doc, json!({"op":"remove_cover_page"}));
    assert_eq!(part(&saved(&mut doc), "word/document.xml").matches("<w:sectPr").count(), 1);
}

#[test]
fn typing_in_the_title_box_changes_the_document_title() {
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    // The title box is in the first paragraph and shows its placeholder; typing replaces the placeholder.
    let mut first = None;
    walk_paragraphs(&doc.body, &mut |p| {
        first.get_or_insert(p.id);
    });
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first.unwrap(),"o":0},"text":"Lab report"}));
    assert_eq!(doc.doc_props().title.as_deref(), Some("Lab report"));
}

#[test]
fn placeholders_are_in_the_interface_language() {
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.academic","labels":{"title":"[Título del documento]","submitted_by":"Presentado por"}}));
    let xml = part(&saved(&mut doc), "word/document.xml");
    assert!(xml.contains("[Título del documento]"));
    assert!(xml.contains("Presentado por"));
}

#[test]
fn a_saved_cover_page_goes_in_as_a_cover() {
    let mut doc = doc_with("Body");
    let first = doc.render_all().order[0];
    let frag = doc.copy_fragment(docxtor_core::Pos::new(first, 0), docxtor_core::Pos::new(first, 4)).unwrap();
    ops(&mut doc, json!({"op":"insert_cover_page","fragment":frag}));
    assert!(doc.has_cover_page());
    assert_eq!(texts(&doc)[0], "Body");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn header_and_footer_designs_fill_the_header_and_footer() {
    let mut doc = doc_with("Body");
    props(&mut doc);
    for d in Document::page_gallery("header") {
        ops(&mut doc, json!({"op":"insert_page_part","kind":"header","design":d.id}));
    }
    for d in Document::page_gallery("footer") {
        ops(&mut doc, json!({"op":"insert_page_part","kind":"footer","design":d.id}));
    }
    let pkg = saved(&mut doc);
    let all = |stem: &str| pkg.parts.iter().filter(|p| p.name.starts_with(stem)).map(|p| String::from_utf8(p.data.clone()).unwrap()).collect::<String>();
    let header = all("word/header");
    let footer = all("word/footer");
    assert!(header.contains("Field notes"), "the last header shows the title");
    assert!(header.contains(" PAGE "));
    assert!(footer.contains("<w:pStyle w:val=\"Footer\"/>"));
    assert!(doc.undo());
}

#[test]
fn the_preview_has_a_page_for_every_design() {
    let mut doc = doc_with("Body");
    props(&mut doc);
    for kind in ["cover", "header", "footer"] {
        let bytes = doc.page_gallery_preview(kind, &Default::default()).unwrap();
        assert_valid(&bytes);
        let p = Document::open(&bytes).unwrap();
        if kind == "cover" {
            let xml = part(&Package::open(&bytes).unwrap(), "word/document.xml");
            assert!(p.has_cover_page(), "{}", &xml[..xml.len().min(3000)]);
        }
        assert!(texts(&p).iter().any(|t| t.contains("Field notes")), "{kind} preview shows the title");
    }
}

#[test]
fn a_header_added_from_inside_a_centred_cover_belongs_to_the_text_and_stays_when_the_cover_goes() {
    for op in [json!({"op":"insert_page_part","kind":"header","design":"docxtor.title"}), json!({"op":"add_header_footer","kind":"header"})] {
        let mut doc = doc_with("Body text");
        let made = ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.centered"}));
        // The caret is left in the cover's first box: the header is asked for from there.
        let in_cover = made[0].caret.map(|c| c.p.0).unwrap_or_else(|| {
            let mut first = 0;
            walk_paragraphs(&doc.body, &mut |p| if first == 0 { first = p.id });
            first
        });
        let mut op = op.clone();
        op["section"] = json!(in_cover);
        ops(&mut doc, op.clone());
        ops(&mut doc, json!({"op":"remove_cover_page"}));
        let setup = serde_json::to_value(doc.page_setup()).unwrap();
        assert!(setup["headers"]["default"].is_object(), "{op}: the text still has its header: {}", setup["headers"]);
        saved(&mut doc);
    }
}

/// The document's own closing section properties (the last `w:sectPr`).
fn last_sect(xml: &str) -> &str {
    let at = xml.rfind("<w:sectPr").unwrap();
    &xml[at..]
}

fn header_on_every_page(doc: &mut Document) {
    ops(doc, json!({"op":"add_header_footer","kind":"header"}));
}

#[test]
fn replacing_a_cover_with_a_centred_one_takes_back_the_different_first_page_it_asked_for() {
    let mut doc = doc_with("Body text");
    header_on_every_page(&mut doc);
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    assert!(last_sect(&part(&saved(&mut doc), "word/document.xml")).contains("<w:titlePg/>"), "the first design turned it on");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.centered"}));
    let xml = part(&saved(&mut doc), "word/document.xml");
    assert!(!last_sect(&xml).contains("<w:titlePg/>"), "page 2 keeps its header and number: {}", last_sect(&xml));
    assert_eq!(xml.matches("<w:titlePg/>").count(), 1, "only the cover's own section has a first page of its own");
}

#[test]
fn removing_a_cover_leaves_a_different_first_page_the_person_set() {
    for design in ["docxtor.centered", "docxtor.simple"] {
        let mut doc = doc_with("Body text");
        header_on_every_page(&mut doc);
        ops(&mut doc, json!({"op":"set_title_page","on":true}));
        ops(&mut doc, json!({"op":"insert_cover_page","design":design}));
        ops(&mut doc, json!({"op":"remove_cover_page"}));
        let xml = part(&saved(&mut doc), "word/document.xml");
        assert!(xml.contains("<w:titlePg/>"), "{design}: the person's own setting stays: {xml}");
    }
    // One the cover asked for goes with it.
    let mut doc = doc_with("Body text");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    ops(&mut doc, json!({"op":"remove_cover_page"}));
    assert!(!part(&saved(&mut doc), "word/document.xml").contains("<w:titlePg/>"));
}

#[test]
fn a_continuous_first_section_does_not_start_on_the_covers_page() {
    let mut doc = doc_with("Body text");
    ops(&mut doc, json!({"op":"set_section","start":"continuous"}));
    assert!(part(&saved(&mut doc), "word/document.xml").contains(r#"<w:type w:val="continuous"/>"#));
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.centered"}));
    let xml = part(&saved(&mut doc), "word/document.xml");
    assert!(!last_sect(&xml).contains(r#"w:val="continuous""#), "the body starts on its own page: {}", last_sect(&xml));
}

#[test]
fn a_header_keeps_the_watermark_when_a_design_replaces_it() {
    let mut doc = doc_with("Body text");
    ops(&mut doc, json!({"op":"set_watermark","text":"DRAFT"}));
    ops(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.blank"}));
    let pkg = saved(&mut doc);
    let headers: Vec<String> = pkg.parts.iter().filter(|p| p.name.starts_with("word/header")).map(|p| String::from_utf8(p.data.clone()).unwrap()).collect();
    assert!(headers.iter().any(|h| h.contains("PowerPlusWaterMarkObject") && h.contains("DRAFT")), "the watermark is still there: {headers:?}");
    assert!(headers.iter().any(|h| h.contains("Cover Pages") || h.contains("Header") || h.contains("sdt")), "and the design too");
}

#[test]
fn the_placeholder_style_is_made_for_designs_in_a_document_without_one() {
    let mut doc = doc_with("Body text");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    ops(&mut doc, json!({"op":"insert_page_part","kind":"footer","design":"docxtor.blank"}));
    let pkg = saved(&mut doc);
    assert!(part(&pkg, "word/styles.xml").contains(r#"w:styleId="PlaceholderText""#));
}

#[test]
fn a_cover_date_written_the_way_word_writes_it_is_not_rewritten_by_an_edit() {
    let mut doc = doc_with("Body text");
    ops(&mut doc, json!({"op":"insert_cover_page","design":"docxtor.simple"}));
    let date = doc.list_controls().into_iter().find(|(_, c, _)| c.kind == "date").expect("a date box");
    ops(&mut doc, json!({"op":"control","action":"set","target":{"p":date.0.unwrap(),"n":date.1.n},"date":"2026-10-14"}));
    // Word's own form of the property has no zone letter.
    let bytes = doc.save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    let item = pkg.parts.iter().find(|p| p.name.starts_with("customXml/item") && !p.name.contains("Props") && String::from_utf8_lossy(&p.data).contains("PublishDate")).map(|p| p.name.clone()).unwrap();
    let text = String::from_utf8(pkg.get(&item).unwrap().to_vec()).unwrap();
    let wordish = text.replace("2026-10-14T00:00:00Z", "2026-10-14T00:00:00");
    assert_ne!(text, wordish, "the engine wrote a zone letter: {text}");
    pkg.set(&item, wordish.clone().into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    // Typing in the cover's title box touches the cover.
    let title = doc.list_controls().into_iter().find(|(_, c, _)| c.kind == "text").expect("a text box");
    ops(&mut doc, json!({"op":"insert_text","at":{"p":title.0.unwrap(),"o":0},"text":"Lab report"}));
    let after = saved(&mut doc);
    assert_eq!(part(&after, &item), wordish, "Word's form of the date stays as it was");
}
