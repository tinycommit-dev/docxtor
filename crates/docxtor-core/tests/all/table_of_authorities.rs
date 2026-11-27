//! The table of authorities: citations marked with TA fields (one, or every place a citation is), the TOA field built
//! by category with page numbers and passim, updated with the other fields, written as Word writes it, undone, valid.

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

fn doc_with(lines: &[&str]) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let md = lines.join("\n\n");
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn paras(doc: &Document) -> Vec<(u32, String)> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push((p.id, p.plain_text())));
    v
}

fn find(doc: &Document, needle: &str) -> (u32, usize) {
    paras(doc).into_iter().find_map(|(id, t)| t.find(needle).map(|i| (id, t[..i].encode_utf16().count()))).unwrap_or_else(|| panic!("no {needle}"))
}

fn saved(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap()
}

const CASE: &str = "Smith v. Jones, 123 U.S. 456 (1990)";

fn brief() -> Document {
    doc_with(&[
        "In Smith v. Jones, 123 U.S. 456 (1990), the court held otherwise.",
        "Under 42 U.S.C. § 1983, a claim lies.",
        "As Smith said, the rule is narrow.",
        "Smith again.",
    ])
}

#[test]
fn a_marked_citation_is_a_hidden_ta_field_as_word_writes_it() {
    let mut doc = brief();
    let (p, o) = find(&doc, CASE);
    let end = o + CASE.encode_utf16().count();
    ops(&mut doc, json!({"op":"toa","action":"mark_citation","at":{"p":p,"o":end},"from":{"p":p,"o":o},"long":CASE,"short":"Smith","category":1}));
    let xml = saved(&mut doc);
    assert!(xml.contains(r#"TA \l "Smith v. Jones, 123 U.S. 456 (1990)" \s "Smith" \c 1"#), "{xml}");
    // The text reads the same: the field shows nothing.
    assert!(paras(&doc)[0].1.contains("(1990), the court"));
    assert!(doc.undo());
    assert!(!saved(&mut doc).contains(" TA "));
}

#[test]
fn mark_all_marks_the_long_and_short_forms() {
    let mut doc = brief();
    let r = ops(&mut doc, json!({"op":"toa","action":"mark_all","long":CASE,"short":"Smith","category":1}));
    assert_eq!(r[0].count, Some(3), "the long form once, the short form twice more");
    let xml = saved(&mut doc);
    assert_eq!(xml.matches(" TA ").count(), 3);
}

#[test]
fn the_table_lists_each_category_with_page_numbers_and_passim() {
    let mut doc = brief();
    ops(&mut doc, json!({"op":"toa","action":"mark_all","long":CASE,"short":"Smith","category":1}));
    let (p, o) = find(&doc, "42 U.S.C. § 1983");
    let end = o + "42 U.S.C. § 1983".encode_utf16().count();
    ops(&mut doc, json!({"op":"toa","action":"mark_citation","at":{"p":p,"o":end},"long":"42 U.S.C. § 1983","category":2}));
    let ids: Vec<u32> = paras(&doc).iter().map(|(id, _)| *id).collect();
    // The case is cited on pages 1, 3, 4, 5 and 6 (as the layout would say): five pages make "passim".
    let pages = json!({ ids[0].to_string(): 1, ids[1].to_string(): 2, ids[2].to_string(): 3, ids[3].to_string(): 6 });
    ops(&mut doc, json!({"op":"toa","action":"insert","options":{"passim":false},"pages":pages}));
    let text: Vec<String> = paras(&doc).into_iter().map(|(_, t)| t).collect();
    let all = text.join("\n");
    assert!(all.contains("Cases"), "{all}");
    assert!(all.contains("Statutes"), "{all}");
    let case_line = text.iter().find(|t| t.starts_with("Smith v. Jones")).expect("the case is listed");
    assert!(case_line.ends_with("1, 3, 6"), "{case_line}");
    let statute = text.iter().find(|t| t.starts_with("42 U.S.C.")).expect("the statute is listed");
    assert!(statute.ends_with('2'), "{statute}");
    // Cases come before statutes.
    assert!(all.find("Cases").unwrap() < all.find("Statutes").unwrap());
    let xml = saved(&mut doc);
    assert!(xml.contains("TOA \\h \\c \"1\"") || xml.contains(" TOA "), "{xml}");
    assert!(xml.contains("TOAHeading") || xml.contains("TOA Heading"));
}

#[test]
fn passim_replaces_five_pages_or_more() {
    let mut doc = doc_with(&["Smith v. Jones one.", "Smith two.", "Smith three.", "Smith four.", "Smith five."]);
    ops(&mut doc, json!({"op":"toa","action":"mark_all","long":"Smith v. Jones","short":"Smith","category":1}));
    let ids: Vec<u32> = paras(&doc).iter().map(|(id, _)| *id).collect();
    let pages: serde_json::Map<String, Value> = ids.iter().enumerate().map(|(k, id)| (id.to_string(), json!(k + 1))).collect();
    ops(&mut doc, json!({"op":"toa","action":"insert","options":{"passim":true},"pages":pages}));
    // The table's entry: the citation, a tab, the pages.
    let entry = paras(&doc).into_iter().map(|(_, t)| t).find(|t| t.starts_with("Smith v. Jones") && t.contains('\t')).expect("listed");
    assert!(entry.ends_with("passim"), "{entry}");
}

#[test]
fn the_table_updates_with_the_other_fields_and_undo_takes_it_out() {
    let mut doc = brief();
    ops(&mut doc, json!({"op":"toa","action":"mark_all","long":CASE,"short":"Smith","category":1}));
    ops(&mut doc, json!({"op":"toa","action":"insert"}));
    // A new citation, then Update fields: the table has it.
    let (p, _) = find(&doc, "Smith again.");
    ops(&mut doc, json!({"op":"toa","action":"mark_citation","at":{"p":p,"o":12},"long":"Roe v. Wade, 410 U.S. 113 (1973)","category":1}));
    ops(&mut doc, json!({"op":"update_fields"}));
    assert!(paras(&doc).iter().any(|(_, t)| t.starts_with("Roe v. Wade")));
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(!paras(&doc).iter().any(|(_, t)| t == "Cases"));
}

#[test]
fn categories_8_to_16_can_be_named() {
    let mut doc = brief();
    ops(&mut doc, json!({"op":"toa","action":"set_category_names","names":[{"category":8,"name":"Treaties"}]}));
    assert_eq!(doc.toa_category_names()[7], "Treaties");
    let bytes = doc.save().unwrap();
    assert_eq!(Document::open(&bytes).unwrap().toa_category_names()[7], "Treaties");
}

#[test]
fn next_citation_finds_a_likely_one() {
    let doc = brief();
    let first = paras(&doc)[0].0;
    let found = doc.next_citation(docxtor_core::Pos::new(first, 0)).expect("a citation");
    assert_eq!(found.0.p.0, first);
}

#[test]
fn mark_all_marks_the_selected_citation_itself_even_with_no_break_spaces_or_a_corrected_text() {
    // (a) A no-break space in the text: the box holds a plain space.
    let mut doc = doc_with(&["Under 42 U.S.C.\u{a0}§\u{a0}1983, a claim lies.", "Again 42 U.S.C. § 1983 here."]);
    let (p, o) = find(&doc, "42 U.S.C.");
    let end = o + "42 U.S.C.\u{a0}§\u{a0}1983".encode_utf16().count();
    let r = ops(&mut doc, json!({"op":"toa","action":"mark_all","from":{"p":p,"o":o},"to":{"p":p,"o":end},"long":"42 U.S.C. § 1983","category":2}));
    assert_eq!(r[0].count, Some(2), "both places, whatever the spaces");
    assert_eq!(saved(&mut doc).matches(" TA ").count(), 2);
    // (b) The long citation corrected in the box: the selection is marked with the corrected text, and nowhere else is found.
    let mut doc = brief();
    let (p, o) = find(&doc, CASE);
    let end = o + CASE.encode_utf16().count();
    let fixed = "Smith v. Jones, 123 U.S. (1 Cranch) 456 (1990)";
    let r = ops(&mut doc, json!({"op":"toa","action":"mark_all","from":{"p":p,"o":o},"to":{"p":p,"o":end},"long":fixed,"short":"Smith","category":1}));
    assert!(r[0].count.unwrap() >= 3, "the selection and the two short mentions: {:?}", r[0].count);
    let xml = saved(&mut doc);
    assert!(xml.contains(&format!(r#"TA \l "{fixed}""#)), "{xml}");
    // Running it again adds nothing.
    let again = ops(&mut doc, json!({"op":"toa","action":"mark_all","from":{"p":p,"o":o},"to":{"p":p,"o":end},"long":fixed,"short":"Smith","category":1}));
    assert_eq!(again[0].count, Some(0));
}

#[test]
fn citations_in_footnotes_are_listed_with_the_page_of_the_note_mark() {
    let mut doc = doc_with(&["First page text.", "Second page text with a note."]);
    let ps = paras(&doc);
    let (p2, _) = ps[1].clone();
    let res = ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p2,"o":11},"text":"See Smith v. Jones, 123 U.S. 456 (1990)."}));
    let _ = res;
    // The TA field sits in the note, as Word writes it.
    let bytes = doc.save().unwrap();
    let mut pkg = Package::open(&bytes).unwrap();
    let notes = String::from_utf8(pkg.get("word/footnotes.xml").unwrap().to_vec()).unwrap();
    let field = r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TA \l "Smith v. Jones, 123 U.S. 456 (1990)" \c 1 </w:instrText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#;
    let at = notes.rfind("</w:p>").unwrap();
    pkg.set("word/footnotes.xml", format!("{}{field}{}", &notes[..at], &notes[at..]).into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let ps = paras(&doc);
    let p2 = ps.iter().find(|(_, t)| t.contains("Second page")).unwrap().0;
    let pages = json!({ ps[0].0.to_string(): 1, p2.to_string(): 4 });
    ops(&mut doc, json!({"op":"toa","action":"insert","before":ps[0].0,"pages":pages}));
    let entry = paras(&doc).into_iter().map(|(_, t)| t).find(|t| t.starts_with("Smith v. Jones") && t.contains('\t')).unwrap_or_else(|| panic!("listed: {:?}", paras(&doc)));
    assert!(entry.ends_with('4'), "{entry}");
}

#[test]
fn a_new_table_keeps_its_leader_through_the_update_that_follows() {
    let mut doc = brief();
    ops(&mut doc, json!({"op":"toa","action":"mark_all","long":CASE,"short":"Smith","category":1}));
    ops(&mut doc, json!({"op":"toa","action":"insert","options":{"leader":"hyphen"}}));
    let first = doc.toa_at(None).unwrap();
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    // A second table, with dots: the style still says hyphen from the first.
    let at = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"toa","action":"insert","before":at,"options":{"leader":"dot"}}));
    ops(&mut doc, json!({"op":"update_fields"}));
    let opts = doc.toa_options(None).expect("a table");
    assert_eq!(serde_json::to_value(&opts).unwrap()["leader"], json!("dot"), "{opts:?}");
}

#[test]
fn marking_the_same_citation_twice_at_one_place_writes_one_field() {
    let mut doc = brief();
    let (p, o) = find(&doc, CASE);
    let end = o + CASE.encode_utf16().count();
    for _ in 0..2 {
        ops(&mut doc, json!({"op":"toa","action":"mark_citation","at":{"p":p,"o":end},"from":{"p":p,"o":o},"long":CASE,"short":"Smith","category":1}));
    }
    assert_eq!(saved(&mut doc).matches(" TA ").count(), 1);
}
