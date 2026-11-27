//! The index dialog's engine: columns, run-in or indented, right-aligned numbers, leaders, separators, language,
//! looks; and AutoMark.

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

fn pages(list: &[(u32, u32)]) -> Value {
    let mut m = serde_json::Map::new();
    for (p, n) in list {
        m.insert(p.to_string(), json!(n));
    }
    Value::Object(m)
}

/// A document with Apple, Pear and Ñandú marked.
fn marked() -> (Document, [u32; 3]) {
    let mut doc = doc_with("Apple pie.\n\nPear tart.\n\nAn \u{d1}and\u{fa} nest, a nest again. Nandu too.\n\nEnd.");
    let a = find_p(&doc, "Apple");
    let p = find_p(&doc, "Pear");
    let n = find_p(&doc, "nest");
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":a,"o":5},"entry":"Apple"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":p,"o":4},"entry":"Pear"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":n,"o":7},"entry":"\u{d1}and\u{fa}"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":n,"o":30},"entry":"Nandu"}));
    (doc, [a, p, n])
}

#[test]
fn the_dialogs_options_are_written_as_words_switches_and_read_back() {
    let (mut doc, [a, p, n]) = marked();
    let end = find_p(&doc, "End.");
    let opts = json!({"columns":2,"runIn":false,"headings":true,"rightAlign":true,"leader":"hyphen","pageSep":"; ","rangeSep":"-","language":3082,"look":"modern"});
    ops(&mut doc, json!({"op":"insert_index","after":end,"options":opts,"pages":pages(&[(a, 1), (p, 2), (n, 3)])}));
    let xml = body_xml(&doc);
    assert!(xml.contains(r#"INDEX \e ""#) && xml.contains(r#"\h "A""#) && xml.contains(r#"\l "; ""#) && xml.contains(r#"\g "-""#), "{xml}");
    assert!(xml.contains(r#"\c "2" \z "3082""#), "{xml}");
    assert!(xml.contains(r#"w:leader="hyphen""#));
    let t = texts(&doc);
    // Spanish sorts the letter with a tilde after n: Nandu, then Ñandú, in their own groups.
    let at = |s: &str| t.iter().position(|x| x == s || x.starts_with(&format!("{s}\t"))).unwrap_or_else(|| panic!("{s} in {t:?}"));
    assert!(at("Nandu") < at("\u{d1}and\u{fa}") && at("\u{d1}and\u{fa}") < at("Pear"), "{t:?}");
    assert!(t.contains(&"\u{d1}".to_string()), "its own heading: {t:?}");
    let o = doc.index_options(None).unwrap();
    assert_eq!((o.columns, o.run_in, o.headings, o.right_align, o.language), (2, false, true, true, Some(3082)));
    assert_eq!((o.page_sep.as_deref(), o.range_sep.as_deref(), o.leader.val()), (Some("; "), Some("-"), "hyphen"));
    // The index lives in a section of its own with two columns, and the file stays valid.
    assert!(xml.contains(r#"w:num="2""#));
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(again.index_options(None).unwrap().language, Some(3082));
    assert_valid(&doc.save().unwrap());
    assert!(doc.undo());
    assert!(!body_xml(&doc).contains("INDEX"));
}

#[test]
fn run_in_and_indented_and_not_right_aligned() {
    let (mut doc, [a, p, n]) = marked();
    let end = find_p(&doc, "End.");
    ops(&mut doc, json!({"op":"insert_index","after":end,"options":{"columns":1,"runIn":true,"pageSep":" / "},"pages":pages(&[(a, 1), (p, 2), (n, 3)])}));
    let xml = body_xml(&doc);
    assert!(xml.contains(r"\r") && !xml.contains("w:tabs"), "run-in has no tab stops: {xml}");
    // Same pages repeated join with the separator.
    ops(&mut doc, json!({"op":"set_index","options":{"columns":1,"runIn":false,"rightAlign":false}}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x == "Apple, 1"), "{t:?}");
    assert!(!body_xml(&doc).contains("w:tabs"));
    ops(&mut doc, json!({"op":"set_index","options":{"columns":1,"rightAlign":true,"leader":"underscore"}}));
    assert!(body_xml(&doc).contains(r#"w:leader="underscore""#));
    assert!(texts(&doc).iter().any(|x| x == "Apple\t1"));
    // The leader sticks when the fields are updated.
    ops(&mut doc, json!({"op":"update_fields","pages":pages(&[(a, 1), (p, 2), (n, 3)])}));
    assert!(body_xml(&doc).contains(r#"w:leader="underscore""#));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn changing_the_columns_makes_the_index_again_in_its_own_section() {
    let (mut doc, [a, p, n]) = marked();
    let end = find_p(&doc, "End.");
    ops(&mut doc, json!({"op":"insert_index","after":end,"options":{"columns":1},"pages":pages(&[(a, 1), (p, 2), (n, 3)])}));
    assert!(!body_xml(&doc).contains(r#"w:num="2""#));
    ops(&mut doc, json!({"op":"set_index","options":{"columns":2},"pages":pages(&[(a, 1), (p, 2), (n, 3)])}));
    let xml = body_xml(&doc);
    assert!(xml.contains(r#"w:num="2""#), "{xml}");
    assert_eq!(xml.matches("INDEX").count(), 1, "one index, not two");
    assert!(texts(&doc).iter().any(|x| x.starts_with("Apple")));
    ops(&mut doc, json!({"op":"set_index","options":{"columns":1}}));
    assert_eq!(body_xml(&doc).matches("INDEX").count(), 1);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn a_look_changes_the_index_styles() {
    let (mut doc, [a, p, n]) = marked();
    let end = find_p(&doc, "End.");
    ops(&mut doc, json!({"op":"insert_index","after":end,"options":{"columns":1,"look":"simple"},"pages":pages(&[(a, 1), (p, 2), (n, 3)])}));
    let styles = String::from_utf8(Package::open(&doc.save().unwrap()).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap();
    let i1 = &styles[styles.find(r#"w:styleId="Index1""#).unwrap()..];
    let i1 = &i1[..i1.find("</w:style>").unwrap()];
    assert!(!i1.contains("w:hanging"), "{i1}");
    ops(&mut doc, json!({"op":"set_index","options":{"columns":1,"look":"modern"}}));
    let styles = String::from_utf8(Package::open(&doc.save().unwrap()).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap();
    let i1 = &styles[styles.find(r#"w:styleId="Index1""#).unwrap()..];
    let i1 = &i1[..i1.find("</w:style>").unwrap()];
    assert!(i1.contains(r#"w:hanging="260""#) && i1.contains("<w:b/>"), "{i1}");
}

#[test]
fn auto_mark_marks_every_listed_text_once_per_paragraph() {
    let mut doc = doc_with("Apples and apples and Apples.\n\nA pear, then a pear.\n\nNothing here.");
    let r = ops(&mut doc, json!({"op":"auto_mark_index","entries":[
        {"text":"Apples","entry":"Fruit:Apple"},
        {"text":"pear","entry":"Fruit:Pear"},
        {"text":"missing","entry":"Nope"},
        {"text":"Nothing"}
    ]}));
    // Case matters ("apples" is not "Apples"); one mark per paragraph per entry; unknown text is skipped.
    assert_eq!(r[0].count, Some(3));
    let apples = find_p(&doc, "Apples and");
    let xml = write_block(find_block(&doc.body, apples).unwrap()).to_xml();
    assert_eq!(xml.matches(r#"XE "Fruit:Apple""#).count(), 1, "{xml}");
    assert!(write_block(find_block(&doc.body, find_p(&doc, "Nothing")).unwrap()).to_xml().contains(r#"XE "Nothing""#));
    // Marking again adds nothing.
    let r = ops(&mut doc, json!({"op":"auto_mark_index","entries":[{"text":"Apples","entry":"Fruit:Apple"}]}));
    assert_eq!(r[0].count, Some(0));
    assert_valid(&doc.save().unwrap());
    // They feed an index with sub-entries.
    ops(&mut doc, json!({"op":"insert_index","options":{"columns":1}}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x.starts_with("Fruit")) && t.iter().any(|x| x.starts_with("Apple")) && t.iter().any(|x| x.starts_with("Pear")), "{t:?}");
    assert!(doc.undo());
    assert!(!body_xml(&doc).contains("INDEX"));
}

#[test]
fn an_automark_file_is_a_two_column_table() {
    let bytes = docxtor_core::automark::template(&[]).unwrap();
    assert_valid(&bytes);
    let entries = docxtor_core::automark::read(&bytes).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!((entries[0].text.as_str(), entries[0].entry.as_str()), ("apple", "Fruit:Apple"));
    let own = docxtor_core::automark::template(&entries[..1]).unwrap();
    assert_eq!(docxtor_core::automark::read(&own).unwrap().len(), 1);
    // A document without a table is not an AutoMark file.
    let plain = doc_with("Just text.").save().unwrap();
    assert!(docxtor_core::automark::read(&plain).unwrap_err().contains("no table"));
}
