//! The table of contents dialog's engine: options written as Word's switches, entries from styles, outline
//! levels and `TC` fields, leaders and looks in the `TOC` styles, update page numbers only, remove.

use docxtor_core::model::{find_block, walk_paragraphs, write_block};
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

fn body_xml(doc: &Document) -> String {
    doc.body.iter().map(|b| write_block(b).to_xml()).collect()
}

fn para_xml(doc: &Document, id: u32) -> String {
    write_block(find_block(&doc.body, id).unwrap()).to_xml()
}

fn saved_styles(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap()
}

fn pages(list: &[(u32, u32)]) -> Value {
    let mut m = serde_json::Map::new();
    for (p, n) in list {
        m.insert(p.to_string(), json!(n));
    }
    Value::Object(m)
}

fn sample() -> Document {
    doc_with("# Alpha\n\nText one.\n\n## Beta\n\nText two.\n\n### Gamma\n\nText three.\n\n# Delta\n\nEnd.")
}

#[test]
fn the_dialogs_options_are_written_as_words_switches_and_read_back() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"title":"Contents","options":{"levels":2,"links":false,"pageNumbers":true,"rightAlign":true,"leader":"hyphen","outline":false}}));
    let t = texts(&doc);
    assert_eq!(t[0], "Contents");
    assert!(t.iter().any(|x| x.starts_with("Alpha\t")) && t.iter().any(|x| x.starts_with("Beta\t")) && t.iter().any(|x| x.starts_with("Delta\t")));
    assert!(!t.iter().any(|x| x.starts_with("Gamma\t")), "level 3 stays out");
    let xml = body_xml(&doc);
    assert!(xml.contains(r#"TOC \o "1-2" \z"#), "{xml}");
    assert!(!xml.contains("w:hyperlink"), "no links");
    assert!(xml.contains(r#"w:leader="hyphen""#));
    // The leader is also in the TOC styles, as Word's dialog leaves it.
    let styles = saved_styles(&mut doc);
    assert!(styles.contains(r#"w:styleId="TOC1""#) && styles.contains(r#"w:leader="hyphen""#));
    let o = doc.toc_options(None).unwrap();
    assert_eq!((o.levels, o.links, o.page_numbers, o.right_align, o.outline), (2, false, true, true, false));
    assert_eq!(o.leader.val(), "hyphen");
    // The saved file reads back the same.
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(again.toc_options(None).unwrap(), o);
    assert!(texts(&again).iter().any(|x| x.starts_with("Beta\t")));
    // Undo takes the whole table away.
    assert!(doc.undo());
    assert!(!texts(&doc).iter().any(|x| x.starts_with("Beta\t")));
}

#[test]
fn page_numbers_off_and_not_right_aligned() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"options":{"levels":3,"pageNumbers":false}}));
    let xml = body_xml(&doc);
    assert!(xml.contains(r"\n"), "{xml}");
    assert!(!xml.contains("PAGEREF"), "no page numbers: {xml}");
    assert!(texts(&doc).iter().any(|x| x == "Alpha"));
    ops(&mut doc, json!({"op":"set_toc","options":{"levels":3,"pageNumbers":true,"rightAlign":false}}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x == "Alpha 1"), "{t:?}");
    assert!(!body_xml(&doc).contains("w:tabs"), "no tab stops when the numbers follow the text");
    let o = doc.toc_options(None).unwrap();
    assert!(!o.right_align && o.page_numbers);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn levels_without_page_numbers() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"options":{"levels":3,"hidePages":[3,3]}}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x == "Gamma"), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("Alpha\t")));
}

#[test]
fn styles_outline_levels_and_toc_entry_fields_make_entries() {
    let mut doc = doc_with("# Alpha\n\nA quote line.\n\nPlain text with an entry.\n\nOutline only.\n\n# Omega");
    let quote = find_p(&doc, "A quote line.");
    ops(&mut doc, json!({"op":"set_style","ids":[quote],"style":"Quote"}));
    let outline = find_p(&doc, "Outline only.");
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[outline],"format":{"outlineLvl":0}}));
    let plain = find_p(&doc, "Plain text");
    ops(&mut doc, json!({"op":"insert_toc_entry","at":{"p":plain,"o":5},"text":"Marked line","level":2}));
    ops(&mut doc, json!({"op":"insert_toc_entry","at":{"p":plain,"o":5},"text":"Other type","level":1,"id":"D"}));
    // The field is hidden and takes no room.
    assert!(texts(&doc).iter().any(|x| x == "Plain text with an entry."));
    assert!(para_xml(&doc, plain).contains("<w:vanish/>"));
    assert!(para_xml(&doc, plain).contains(r#"TC "Marked line" \l 2"#), "{}", para_xml(&doc, plain));

    let first = doc.render_all().order[0];
    let opts = json!({"levels":1,"outline":true,"styles":[{"style":"Quote","level":2}],"entries":true});
    ops(&mut doc, json!({"op":"insert_toc","before":first,"options":opts}));
    let xml = body_xml(&doc);
    assert!(xml.contains(r#"\t "Quote,2""#) && xml.contains(" \\f ") && xml.contains("\\u"), "{xml}");
    let t = texts(&doc);
    let lines: Vec<&String> = t.iter().take_while(|x| !x.starts_with("# ") && !x.starts_with("Alpha\n")).take(8).collect();
    let entry = |name: &str| t.iter().find(|x| x.starts_with(&format!("{name}\t"))).cloned();
    assert!(entry("Alpha").is_some(), "{lines:?}");
    assert!(entry("A quote line.").is_some(), "style entries: {t:?}");
    assert!(entry("Outline only.").is_some(), "outline entries: {t:?}");
    assert!(entry("Marked line").is_some(), "TC entries: {t:?}");
    assert!(entry("Other type").is_none(), "another entry type stays out");
    // The TC entry is level 2, the quote too: both use TOC 2.
    assert!(xml.contains(r#"w:val="TOC2""#));
    // An entry type of its own gets a table of its own.
    ops(&mut doc, json!({"op":"set_toc","options":{"levels":0,"outline":false,"entries":true,"entryId":"D"}}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x.starts_with("Other type\t")) && !t.iter().any(|x| x.starts_with("Marked line\t")), "{t:?}");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn switches_the_dialog_does_not_know_are_kept() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first}));
    // Give the field a switch of Word's the dialog has no control for.
    ops(&mut doc, json!({"op":"set_toc","options":{"levels":3}}));
    let id = doc.toc_at(None).unwrap();
    ops(&mut doc, json!({"op":"set_field_code","at":{"p":id,"o":0},"instr":r#"TOC \o "1-3" \h \z \u \w"#}));
    ops(&mut doc, json!({"op":"set_toc","options":{"levels":2}}));
    let xml = body_xml(&doc);
    assert!(xml.contains(r#"\o "1-2""#) && xml.contains("\\w"), "{xml}");
}

#[test]
fn a_look_restyles_the_toc_styles_and_survives_an_update() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"options":{"levels":3,"look":"formal","leader":"underscore"}}));
    let styles = saved_styles(&mut doc);
    let toc1 = &styles[styles.find(r#"w:styleId="TOC1""#).unwrap()..];
    let toc1 = &toc1[..toc1.find("</w:style>").unwrap()];
    assert!(toc1.contains("<w:caps/>") && toc1.contains("<w:b/>"), "{toc1}");
    assert!(toc1.contains(r#"w:leader="underscore""#));
    // Updating keeps the leader the table already has.
    ops(&mut doc, json!({"op":"update_toc"}));
    assert!(body_xml(&doc).contains(r#"w:leader="underscore""#));
    // Another look changes the formatting but not the table's entries.
    ops(&mut doc, json!({"op":"set_toc","options":{"levels":3,"look":"simple","leader":"underscore"}}));
    let styles = saved_styles(&mut doc);
    let toc1 = &styles[styles.find(r#"w:styleId="TOC1""#).unwrap()..];
    let toc1 = &toc1[..toc1.find("</w:style>").unwrap()];
    assert!(!toc1.contains("<w:caps/>"), "{toc1}");
}

#[test]
fn update_page_numbers_only_leaves_the_entries_alone() {
    let mut doc = sample();
    let alpha = find_p(&doc, "Alpha");
    let delta = find_p(&doc, "Delta");
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"pages":pages(&[(alpha, 1), (delta, 2)])}));
    assert!(texts(&doc).iter().any(|x| x == "Delta\t2"));
    // The heading is renamed: a full update would change the entry, page numbers only does not.
    ops(&mut doc, json!({"op":"set_paragraph_text","id":delta,"markdown":"Delta renamed"}));
    let r = ops(&mut doc, json!({"op":"update_toc","pages_only":true,"pages":pages(&[(alpha, 1), (delta, 5)])}));
    assert_eq!(r[0].count, Some(1));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x == "Delta\t5"), "{t:?}");
    assert!(!t.iter().any(|x| x.starts_with("Delta renamed\t")));
    ops(&mut doc, json!({"op":"update_toc","pages":pages(&[(alpha, 1), (delta, 5)])}));
    assert!(texts(&doc).iter().any(|x| x == "Delta renamed\t5"));
}

#[test]
fn remove_table_of_contents_takes_the_title_and_can_be_undone() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"title":"Contents"}));
    assert!(texts(&doc).iter().any(|x| x == "Contents"));
    ops(&mut doc, json!({"op":"remove_toc"}));
    let t = texts(&doc);
    assert!(!t.iter().any(|x| x == "Contents" || x.starts_with("Alpha\t")), "{t:?}");
    assert!(t.iter().any(|x| x == "Alpha") && t.iter().any(|x| x == "Delta"), "the headings stay");
    assert!(doc.toc_at(None).is_none());
    let err = doc.apply_json(&json!({"op":"remove_toc"}).to_string(), false).unwrap_err().to_string();
    assert!(err.contains("no table of contents"));
    assert_valid(&doc.save().unwrap());
    assert!(doc.undo());
    assert!(texts(&doc).iter().any(|x| x == "Contents") && doc.toc_at(None).is_some());
}

#[test]
fn a_word_made_toc_with_t_and_n_switches_is_rebuilt_as_it_asks() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first}));
    let id = doc.toc_at(None).unwrap();
    ops(&mut doc, json!({"op":"set_field_code","at":{"p":id,"o":0},"instr":r#"TOC \o "1-1" \h \z \t "Heading 3,2" \n "2-2""#}));
    ops(&mut doc, json!({"op":"update_toc"}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x.starts_with("Alpha\t")), "{t:?}");
    assert!(t.iter().any(|x| x == "Gamma" || x.starts_with("Gamma\t")), "Gamma is level 2 from the style list: {t:?}");
    assert!(t.iter().any(|x| x.starts_with("Alpha\t")) && t.iter().filter(|x| x.starts_with("Beta")).count() == 1, "Beta (level 2 heading) is not listed");
    assert!(t.iter().any(|x| x == "Gamma"), "level 2 has no page number");
}

#[test]
fn a_leader_change_keeps_style_children_in_schema_order() {
    let mut doc = sample();
    let first = doc.render_all().order[0];
    // The "formal" look gives TOC1 run properties (bold, caps), as a Word-made style would have.
    ops(&mut doc, json!({"op":"insert_toc","before":first,"options":{"levels":3,"look":"formal"}}));
    // Now change only the leader: the look stays "from the document", so the run properties are left in place.
    ops(&mut doc, json!({"op":"set_toc","options":{"levels":3,"leader":"hyphen"}}));
    let styles = saved_styles(&mut doc);
    let toc1 = &styles[styles.find(r#"w:styleId="TOC1""#).unwrap()..];
    let toc1 = &toc1[..toc1.find("</w:style>").unwrap()];
    let (ppr, rpr) = (toc1.find("<w:pPr>").expect(toc1), toc1.find("<w:rPr>").expect(toc1));
    assert!(ppr < rpr, "pPr must come before rPr: {toc1}");
    assert!(toc1.contains(r#"w:leader="hyphen""#));
}

#[test]
fn removing_a_toc_keeps_a_content_control_that_was_already_empty() {
    use docxtor_core::model::{Block, BlockContainer};
    let mut doc = sample();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first}));
    let shell = docxtor_core::xml::Element::new("w:sdt").with_child(docxtor_core::xml::Element::new("w:sdtPr"));
    doc.body.insert(0, Block::Container(BlockContainer { id: 900_000, shell, content_in: Some("w:sdtContent".into()), blocks: vec![] }));
    ops(&mut doc, json!({"op":"remove_toc"}));
    assert!(doc.body.iter().any(|b| b.id() == 900_000), "the empty control stays");
    assert!(doc.toc_at(None).is_none());
}

#[test]
fn a_numbered_heading_keeps_its_number_before_a_tab() {
    // Found by the real-file checks: updating dropped the heading numbers.
    let mut doc = sample();
    let (alpha, delta) = (find_p(&doc, "Alpha"), find_p(&doc, "Delta"));
    ops(&mut doc, json!({"op":"toggle_list","ids":[alpha, delta],"kind":"decimal"}));
    let bullet = find_p(&doc, "Beta");
    ops(&mut doc, json!({"op":"toggle_list","ids":[bullet],"kind":"bullet"}));
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"options":{"levels":3}}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x.starts_with("1.\tAlpha\t")), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("2.\tDelta\t")), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("Beta\t")), "a bullet is left out: {t:?}");
    assert!(t.iter().any(|x| x.starts_with("Gamma\t")), "{t:?}");
    // The text after the number starts at a left tab stop, as Word writes it; an entry without a number has none.
    let entry = find_p(&doc, "1.\tAlpha");
    let xml = para_xml(&doc, entry);
    assert!(xml.contains(r#"w:val="left" w:pos="440""#), "{xml}");
    assert!(!para_xml(&doc, find_p(&doc, "Gamma\t")).contains(r#"w:val="left""#));
    // Updating again gives the same, and the file reads back with the numbers.
    ops(&mut doc, json!({"op":"update_toc"}));
    assert!(texts(&doc).iter().any(|x| x.starts_with("1.\tAlpha\t")));
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert!(texts(&again).iter().any(|x| x.starts_with("2.\tDelta\t")));
}
