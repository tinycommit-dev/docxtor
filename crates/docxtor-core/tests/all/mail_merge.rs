//! Mail merge, labels and envelopes: starting a merge from a CSV or Excel list, merge fields, previewing a record,
//! merging into a new document, and the file layout (Word's `w:mailMerge`, Docxtor's own part with the recipients).
//! Each edit is an op, so it also undoes and survives a save.

use docxtor_core::document::MergeRange;
use docxtor_core::merge::run::Answers;
use docxtor_core::merge::{csv, xlsx};
use docxtor_core::package::Package;
use docxtor_core::xml;
use docxtor_core::Document;
use serde_json::{json, Value};
use std::io::Write;

/// Every XML part of a saved package must be namespace-well-formed.
fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn part(bytes: &[u8], name: &str) -> Option<String> {
    Package::open(bytes).unwrap().get(name).map(|b| String::from_utf8(b.to_vec()).unwrap())
}

const PEOPLE: &str = "First Name,Last Name,Title,Street,City,State,ZIP,Country\nAda,Lovelace,Ms.,12 Crumb Rd,London,,N1 9GU,United Kingdom\nGrace,Hopper,Rear Adm.,1 Navy Way,Arlington,VA,22202,United States\nAlan,Turing,,5 Main St,Wilmslow,,SK9 1AA,United Kingdom\n";

/// The `set_mail_merge` op for a CSV.
fn merge_op(main_type: &str, text: &str, name: &str) -> Value {
    let (t, delim) = csv::parse(text, None).unwrap();
    json!({"op": "set_mail_merge", "merge": {"main_type": main_type, "source": {"kind": "csv", "name": name, "delimiter": delim.to_string()}, "columns": t.columns, "rows": t.rows}})
}

fn first_para(doc: &Document) -> u32 {
    docxtor_core::model::paragraph_order(&doc.body)[0]
}

fn text(doc: &Document) -> String {
    docxtor_core::export::to_text(doc)
}

/// A letter: "Dear «First Name» «Last Name»," then a second paragraph with a rule.
fn letter() -> Document {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "Dear  ,"}));
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p, "o": 6}, "field": {"kind": "field", "name": "Last Name"}}));
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p, "o": 5}, "field": {"kind": "field", "name": "First Name"}}));
    ops(&mut doc, json!({"op": "insert_paragraph", "after": p, "text": "Your state is: "}));
    ops(&mut doc, json!({"op": "set_mail_merge", "merge": serde_json::from_value::<Value>(merge_op("formLetters", PEOPLE, "people.csv")["merge"].clone()).unwrap()}));
    doc
}

fn body_paras(doc: &Document) -> Vec<u32> {
    docxtor_core::model::paragraph_order(&doc.body)
}

#[test]
fn start_a_merge_writes_word_settings_and_keeps_the_recipients() {
    let mut doc = letter();
    let info = doc.merge_info();
    assert!(info.active && info.has_recipients);
    assert_eq!((info.main_type.as_str(), info.records, info.included, info.source_name.as_deref()), ("formLetters", 3, 3, Some("people.csv")));
    assert_eq!(info.columns[0], "First Name");
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    // Word's own settings: the kind of document, the link to the file, how it reads it.
    let settings = part(&bytes, "word/settings.xml").unwrap();
    for want in ["<w:mailMerge>", "w:mainDocumentType w:val=\"formLetters\"", "<w:linkToQuery/>", "w:dataType w:val=\"textFile\"", "SELECT * FROM people.csv", "<w:odso>", "<w:fHdr/>", "w:colDelim w:val=\"44\""] {
        assert!(settings.contains(want), "settings.xml should hold {want}: {settings}");
    }
    // The link is a relationship of the settings part, to the file.
    let rels = part(&bytes, "word/_rels/settings.xml.rels").unwrap();
    assert!(rels.contains("mailMergeSource") && rels.contains("file:///people.csv") && rels.contains("External"), "{rels}");
    // The recipients themselves are in a part of Docxtor's own, found again when the file opens.
    let again = Document::open(&bytes).unwrap();
    let info = again.merge_info();
    assert_eq!((info.records, info.has_recipients), (3, true));
    let data = again.merge_data().unwrap();
    assert_eq!(data.rows[1], ["Grace", "Hopper", "Rear Adm.", "1 Navy Way", "Arlington", "VA", "22202", "United States"]);
    // The address fields are matched by name: Street is Address 1, ZIP the postal code.
    let m = |f: &str| data.matched.iter().find(|(n, _)| n == f).and_then(|(_, c)| c.clone());
    assert_eq!((m("Address 1").as_deref(), m("Postal Code").as_deref(), m("Courtesy Title").as_deref(), m("Nickname")), (Some("Street"), Some("ZIP"), Some("Title"), None));
}

#[test]
fn starting_a_merge_undoes_and_ending_it_takes_everything_out() {
    let mut doc = Document::new_blank();
    let before = doc.save().unwrap();
    ops(&mut doc, merge_op("mailingLabels", PEOPLE, "people.csv"));
    assert!(doc.merge_info().active);
    assert!(doc.undo());
    assert!(!doc.merge_info().active && !doc.merge_info().has_recipients);
    assert!(doc.redo());
    assert_eq!(doc.merge_info().main_type, "mailingLabels");
    let saved = doc.save().unwrap();
    assert!(Package::open(&saved).unwrap().parts.iter().any(|p| p.name.starts_with("customXml/item")));
    // End the merge: no settings, no list of people left in the file.
    ops(&mut doc, json!({"op": "set_mail_merge", "merge": null}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert!(!part(&bytes, "word/settings.xml").unwrap().contains("mailMerge"));
    assert!(!Package::open(&bytes).unwrap().parts.iter().any(|p| p.name.starts_with("customXml/")), "the list of people should be gone");
    let doc2 = Document::open(&bytes).unwrap();
    assert!(!doc2.merge_info().active && !doc2.merge_info().has_recipients);
    // A document that never had a merge keeps the settings it had.
    assert!(part(&before, "word/settings.xml").unwrap().contains("defaultTabStop") && part(&bytes, "word/settings.xml").unwrap().contains("defaultTabStop"));
    assert!(err(&mut doc, json!({"op": "set_mail_merge", "merge": {"main_type": "poster"}})).contains("unknown mail merge type"));
}

#[test]
fn recipients_are_ticked_filtered_sorted_and_matched() {
    let mut doc = letter();
    ops(&mut doc, json!({"op": "set_merge_recipients", "active": [true, false, true]}));
    assert_eq!(doc.merge_info().included, 2);
    ops(&mut doc, json!({"op": "set_merge_recipients", "filter": [{"field": "Country", "op": "eq", "value": "United Kingdom"}], "sort": [{"field": "Last Name", "desc": true}]}));
    let data = doc.merge_data().unwrap();
    // Grace is unticked and in the US anyway; Turing sorts before Lovelace when descending.
    assert_eq!(data.order, [2, 0]);
    assert_eq!(doc.merge_find("lovelace", None), [1]);
    assert_eq!(doc.merge_find("Wilms", Some("City")), [0]);
    assert!(doc.merge_find("zzz", None).is_empty());
    // A mapping names the column for an address field, and is written into Word's settings.
    ops(&mut doc, json!({"op": "set_merge_recipients", "mapping": {"First Name": "Last Name"}}));
    let settings = part(&doc.save().unwrap(), "word/settings.xml").unwrap();
    assert!(settings.contains("<w:mappedName w:val=\"Last Name\"/>"), "{settings}");
    assert!(err(&mut doc, json!({"op": "set_merge_recipients", "mapping": {"First Name": "Nope"}})).contains("no column called"));
    assert!(err(&mut doc, json!({"op": "set_merge_recipients", "active": [true]})).contains("3 recipients"));
    // All of it is one undo step each.
    assert!(doc.undo() && doc.undo());
    assert_eq!(doc.merge_data().unwrap().order, [0, 2]);
    assert!(doc.undo());
    assert_eq!(doc.merge_data().unwrap().order, [0, 1, 2]);
}

#[test]
fn merge_fields_are_complex_fields_with_a_placeholder() {
    let doc = letter();
    let fields = doc.list_fields();
    let codes: Vec<&str> = fields.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(codes, ["MERGEFIELD", "MERGEFIELD"]);
    assert_eq!(fields[0].result, "«First_Name»");
    assert_eq!(text(&doc).lines().next().unwrap(), "Dear «First_Name» «Last_Name»,");
    assert!(fields.iter().all(|f| f.instr.trim().starts_with("MERGEFIELD ")));
    // Written as begin, instruction, separate, result, end: not a simple field.
    let mut d = letter();
    let xml = String::from_utf8(Package::open(&d.save().unwrap()).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains("w:fldCharType=\"begin\"") && !xml.contains("fldSimple"), "{xml}");
}

#[test]
fn a_record_can_be_previewed_without_changing_the_document() {
    let mut doc = letter();
    let saved_template = doc.save().unwrap();
    let a = Answers::default();
    let info = doc.merge_preview(Some(1), &a).unwrap().unwrap();
    assert_eq!((info.position, info.count, info.used, info.skipped), (1, 3, 1, false));
    assert_eq!(text(&doc).lines().next().unwrap(), "Dear Grace Hopper,");
    assert_eq!(doc.merge_info().previewing, Some(1));
    // Moving to another record.
    doc.merge_preview(Some(2), &a).unwrap();
    assert_eq!(text(&doc).lines().next().unwrap(), "Dear Alan Turing,");
    // While a record is shown the document can't be edited, and it saves as the template.
    let p = first_para(&doc);
    assert!(err(&mut doc, json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "x"})).contains("Stop previewing"));
    assert!(!doc.undo());
    assert_eq!(doc.save().unwrap(), saved_template);
    // Past the end is the last record.
    assert_eq!(doc.merge_preview(Some(99), &a).unwrap().unwrap().position, 2);
    // Stopping brings the template back, with the fields in it.
    assert!(doc.merge_preview(None, &a).unwrap().is_none());
    assert_eq!(text(&doc).lines().next().unwrap(), "Dear «First_Name» «Last_Name»,");
    assert_eq!(doc.list_fields().len(), 2);
    assert!(doc.merge_info().previewing.is_none());
    // Editing works again.
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "x"}));
}

#[test]
fn merging_makes_a_new_document_with_a_section_for_each_record() {
    let mut doc = letter();
    // Skip nobody; the second paragraph shows a rule's result.
    let p2 = body_paras(&doc)[1];
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p2, "o": 15}, "field": {"kind": "if", "field": "State", "op": "eq", "value": "VA", "then": "Virginia", "otherwise": "somewhere else"}}));
    let before = doc.save().unwrap();
    let (bytes, result) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!((result.records, result.copies, result.skipped), (3, 3, 0));
    assert_valid(&bytes);
    let merged = Document::open(&bytes).unwrap();
    let lines: Vec<String> = text(&merged).lines().map(String::from).collect();
    assert_eq!(lines, ["Dear Ada Lovelace,", "Your state is: somewhere else", "Dear Grace Hopper,", "Your state is: Virginia", "Dear Alan Turing,", "Your state is: somewhere else"]);
    // A next-page section break between records, and the closing one: three sections.
    let xml = part(&bytes, "word/document.xml").unwrap();
    assert_eq!(xml.matches("<w:sectPr").count(), 3, "{xml}");
    // The merged document is an ordinary one: no merge fields, no merge settings, no list of people.
    assert!(merged.list_fields().is_empty());
    assert!(!merged.merge_info().active && !merged.merge_info().has_recipients);
    assert!(!part(&bytes, "word/settings.xml").unwrap().contains("mailMerge"));
    assert!(!Package::open(&bytes).unwrap().parts.iter().any(|p| p.name.starts_with("customXml/item")));
    // The template is as it was.
    assert_eq!(doc.save().unwrap(), before);
    // A range of the records, and the same answer twice (a pure function of the document and the record).
    let (b2, r2) = doc.merge_to_docx(&Answers::default(), &MergeRange { from: Some(1), to: Some(2) }).unwrap();
    assert_eq!(r2.copies, 1);
    assert_eq!(text(&Document::open(&b2).unwrap()).lines().next().unwrap(), "Dear Grace Hopper,");
    let (again, _) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!(text(&Document::open(&again).unwrap()), text(&merged));
    assert!(doc.merge_to_docx(&Answers::default(), &MergeRange { from: Some(5), to: Some(5) }).is_err());
}

#[test]
fn a_directory_flows_on_without_a_page_for_each_record() {
    let mut doc = letter();
    ops(&mut doc, json!({"op": "set_mail_merge", "merge": {"main_type": "catalog"}}));
    assert_eq!(doc.merge_info().main_type, "catalog");
    // Changing only the kind keeps the recipients.
    assert_eq!(doc.merge_info().records, 3);
    let (bytes, r) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!(r.copies, 3);
    let merged = Document::open(&bytes).unwrap();
    assert_eq!(merged.page_setup().sections.len(), 1);
    assert_eq!(text(&merged).lines().filter(|l| l.starts_with("Dear")).count(), 3);
    assert_valid(&bytes);
}

#[test]
fn a_filter_and_the_ticks_decide_who_is_merged() {
    let mut doc = letter();
    ops(&mut doc, json!({"op": "set_merge_recipients", "active": [true, true, false], "sort": [{"field": "First Name", "desc": true}]}));
    let (bytes, r) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!(r.copies, 2);
    let lines: Vec<String> = text(&Document::open(&bytes).unwrap()).lines().step_by(2).map(String::from).collect();
    assert_eq!(lines, ["Dear Grace Hopper,", "Dear Ada Lovelace,"]);
    // A skip rule leaves a record out.
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p, "o": 0}, "field": {"kind": "skip_if", "field": "First Name", "op": "eq", "value": "Grace"}}));
    let (bytes, r) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!((r.copies, r.skipped), (1, 1));
    assert_eq!(text(&Document::open(&bytes).unwrap()).lines().next().unwrap(), "Dear Ada Lovelace,");
}

#[test]
fn ask_and_fill_in_are_answered_by_the_caller() {
    let mut doc = letter();
    let p2 = body_paras(&doc)[1];
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p2, "o": 15}, "field": {"kind": "fill_in", "prompt": "Which room?", "default": "A1"}}));
    let prompts = doc.merge_prompts();
    assert_eq!((prompts.len(), prompts[0].key.as_str(), prompts[0].default.as_str(), prompts[0].once), (1, "fill:Which room?", "A1", false));
    let mut a = Answers::default();
    a.records.insert(1, [("fill:Which room?".to_string(), "B2".to_string())].into());
    let (bytes, _) = doc.merge_to_docx(&a, &MergeRange::default()).unwrap();
    let lines: Vec<String> = text(&Document::open(&bytes).unwrap()).lines().filter(|l| l.starts_with("Your")).map(String::from).collect();
    assert_eq!(lines, ["Your state is: A1", "Your state is: B2", "Your state is: A1"]);
}

#[test]
fn the_address_block_and_greeting_line_use_the_recipients_columns() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p, "o": 0}, "field": {"kind": "greeting_line"}}));
    ops(&mut doc, json!({"op": "insert_paragraph", "after": p, "text": ""}));
    let p2 = body_paras(&doc)[1];
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p2, "o": 0}, "field": {"kind": "address_block", "name": "title_first_last", "company": false, "country": "different", "except": "United States"}}));
    ops(&mut doc, merge_op("formLetters", PEOPLE, "people.csv"));
    // The dialogs preview what a field would give for a record.
    let code = docxtor_core::merge::instr::instruction_of(&serde_json::from_value(json!({"kind": "address_block", "name": "title_first_last", "country": "different", "except": "United States"})).unwrap()).unwrap();
    assert_eq!(doc.merge_field_preview(&code, 0), "Ms. Ada Lovelace\n12 Crumb Rd\nLondon, N1 9GU\nUNITED KINGDOM");
    assert_eq!(doc.merge_field_preview(&code, 1), "Rear Adm. Grace Hopper\n1 Navy Way\nArlington, VA 22202");
    let (bytes, _) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    let merged = text(&Document::open(&bytes).unwrap());
    assert!(merged.starts_with("Dear Ms. Lovelace,\nMs. Ada Lovelace\n12 Crumb Rd\nLondon, N1 9GU\nUNITED KINGDOM\n"), "{merged}");
    // No title: the greeting leaves it out; no name at all would give "Dear Sir or Madam,".
    assert!(merged.contains("Dear Hopper,") || merged.contains("Dear Rear Adm. Hopper,"), "{merged}");
    assert!(merged.contains("Dear Turing,"), "{merged}");
}

#[test]
fn labels_are_a_table_with_wordss_geometry_and_next_record_fields() {
    let products = docxtor_core::merge::labels::products();
    let spec = products.iter().find(|p| p.id == "avery-5160").unwrap();
    let mut doc = Document::new_blank();
    ops(&mut doc, json!({"op": "make_labels", "spec": spec, "fill": {"kind": "same", "lines": ["Ada Lovelace", "12 Crumb Rd"]}, "look": {"size_pt": 10}}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml").unwrap();
    // Letter paper, margins that put the first label 0.5 in down and 0.1875 in across, ten rows of exactly 1 in.
    assert!(xml.contains("<w:pgSz w:w=\"12240\" w:h=\"15840\"/>"), "{xml}");
    assert!(xml.contains("w:top=\"720\"") && xml.contains("w:left=\"270\""), "{xml}");
    assert_eq!(xml.matches("<w:tr>").count() + xml.matches("<w:tr ").count(), 10);
    assert!(xml.contains("<w:trHeight w:val=\"1440\" w:hRule=\"exact\"/>"));
    assert_eq!(xml.matches("<w:gridCol").count(), 5);
    assert_eq!(xml.matches("Ada Lovelace").count(), 30);
    assert!(doc.undo());
    assert!(!text(&doc).contains("Ada Lovelace"));
    // One label.
    ops(&mut doc, json!({"op": "make_labels", "spec": spec, "fill": {"kind": "single", "lines": ["Only me"], "row": 3, "col": 2}}));
    assert_eq!(text(&doc).matches("Only me").count(), 1);
    assert!(err(&mut doc, json!({"op": "make_labels", "spec": spec, "fill": {"kind": "single", "lines": ["x"], "row": 11, "col": 1}})).contains("no label at row 11"));
    // A merge: the first label is empty, the others start with Next Record; Update labels copies the first into them.
    ops(&mut doc, json!({"op": "make_labels", "spec": spec, "fill": {"kind": "merge"}}));
    assert_eq!(doc.list_fields().len(), 29);
    let first = body_paras(&doc).into_iter().next().unwrap();
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": first, "o": 0}, "field": {"kind": "field", "name": "First Name"}}));
    ops(&mut doc, json!({"op": "update_labels"}));
    assert_eq!(doc.list_fields().len(), 30 + 29);
    ops(&mut doc, merge_op("mailingLabels", PEOPLE, "people.csv"));
    let (bytes, r) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    // Three recipients fit on one sheet of 30.
    assert_eq!((r.records, r.copies), (3, 1));
    let merged = text(&Document::open(&bytes).unwrap());
    assert!(merged.contains("Ada") && merged.contains("Grace") && merged.contains("Alan"));
    assert_eq!(merged.matches("Ada").count() + merged.matches("Grace").count() + merged.matches("Alan").count(), 3);
    assert_valid(&bytes);
}

#[test]
fn a_second_sheet_of_labels_is_made_when_the_list_is_longer() {
    let products = docxtor_core::merge::labels::products();
    let spec = products.iter().find(|p| p.id == "avery-5164").unwrap();
    let mut doc = Document::new_blank();
    ops(&mut doc, json!({"op": "make_labels", "spec": spec, "fill": {"kind": "merge"}}));
    let first = body_paras(&doc).into_iter().next().unwrap();
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": first, "o": 0}, "field": {"kind": "field", "name": "First Name"}}));
    ops(&mut doc, json!({"op": "update_labels"}));
    let mut csv_text = String::from("First Name\n");
    for i in 1..=8 {
        csv_text.push_str(&format!("Person{i}\n"));
    }
    ops(&mut doc, merge_op("mailingLabels", &csv_text, "p.csv"));
    // Six labels to a sheet: eight people need two.
    let (bytes, r) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!((r.records, r.copies), (8, 2));
    let xml = part(&bytes, "word/document.xml").unwrap();
    assert_eq!(xml.matches("<w:tbl>").count(), 2);
    assert_eq!(xml.matches("<w:sectPr").count(), 2);
}

#[test]
fn an_envelope_is_a_new_first_section_with_the_envelopes_page_size() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "Dear Ada,"}));
    ops(&mut doc, json!({"op": "insert_envelope", "size": "10", "layout": {"delivery": "Ada Lovelace\n12 Crumb Rd\nLondon", "return": "Grace Hopper\n1 Navy Way"}}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml").unwrap();
    let sections = doc.page_setup().sections;
    assert_eq!(sections.len(), 2);
    assert_eq!((sections[0].width, sections[0].height), (13680, 5940));
    // The letter is still on letter paper, in the second section.
    assert_eq!((sections[1].width, sections[1].height), (12240, 15840));
    assert!(xml.find("Ada Lovelace").unwrap() < xml.find("Dear Ada,").unwrap());
    assert!(xml.contains("w:orient=\"landscape\""));
    let t = text(&doc);
    assert!(t.starts_with("Grace Hopper\n1 Navy Way\nAda Lovelace\n12 Crumb Rd\nLondon\nDear Ada,"), "{t}");
    // Adding one again changes the envelope instead of adding a second.
    ops(&mut doc, json!({"op": "insert_envelope", "size": "dl", "layout": {"delivery": "Alan Turing", "return": ""}}));
    let s = doc.page_setup().sections;
    assert_eq!(s.len(), 2);
    assert_eq!((s[0].width, s[0].height), (12472, 6236));
    assert!(text(&doc).starts_with("Alan Turing\nDear Ada,"));
    // One undo takes the change back, another the envelope.
    assert!(doc.undo());
    assert!(text(&doc).starts_with("Grace Hopper"));
    assert!(doc.undo());
    assert_eq!(doc.page_setup().sections.len(), 1);
    assert!(err(&mut doc, json!({"op": "insert_envelope", "size": "zz", "layout": {"delivery": ""}})).contains("unknown envelope size"));
}

#[test]
fn adding_an_envelope_keeps_a_first_section_that_only_has_an_envelopes_size() {
    // A B5 report section (176 x 250 mm, portrait, ordinary margins) followed by a letter section.
    let mut pkg = docxtor_core::package::Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let body = r#"<w:p><w:r><w:t>Report chapter</w:t></w:r></w:p><w:p><w:pPr><w:sectPr><w:pgSz w:w="9979" w:h="14175"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:pPr></w:p><w:p><w:r><w:t>Second part</w:t></w:r></w:p>"#;
    pkg.set("word/document.xml", xml.replacen("<w:p/>", body, 1).into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert!(text(&doc).contains("Report chapter"));
    ops(&mut doc, json!({"op": "insert_envelope", "size": "10", "layout": {"delivery": "Ada Lovelace", "return": ""}}));
    let t = text(&doc);
    assert!(t.starts_with("Ada Lovelace") && t.contains("Report chapter") && t.contains("Second part"), "{t}");
    assert_eq!(doc.page_setup().sections.len(), 3);
    // Only the envelope that is first is changed; one further down is left alone.
    ops(&mut doc, json!({"op": "insert_envelope", "size": "dl", "layout": {"delivery": "Alan Turing", "return": ""}}));
    let t = text(&doc);
    assert!(t.starts_with("Alan Turing\nReport chapter"), "{t}");
    assert_eq!(doc.page_setup().sections.len(), 3);
}

#[test]
fn a_merge_envelope_is_the_whole_document_with_an_address_block() {
    let mut doc = Document::new_blank();
    ops(&mut doc, json!({"op": "insert_envelope", "size": "c5", "layout": {"delivery": "", "return": "Grace Hopper"}, "replace": true, "address_block": true}));
    ops(&mut doc, merge_op("envelopes", PEOPLE, "people.csv"));
    assert_eq!(doc.list_fields().iter().map(|f| f.code.clone()).collect::<Vec<_>>(), ["ADDRESSBLOCK"]);
    let sections = doc.page_setup().sections;
    assert_eq!((sections.len(), sections[0].width, sections[0].height), (1, 12983, 9184));
    let (bytes, r) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!(r.copies, 3);
    let merged = Document::open(&bytes).unwrap();
    // An envelope for each recipient, each on its own page.
    assert_eq!(merged.page_setup().sections.len(), 3);
    assert!(text(&merged).contains("Ada Lovelace\n12 Crumb Rd\nLondon, N1 9GU"));
    assert_valid(&bytes);
}

/// A small workbook: one sheet with a header row, shared strings, a number and a date.
fn workbook() -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let parts: [(&str, &str); 6] = [
        ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#),
        ("_rels/.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#),
        ("xl/workbook.xml", r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Members" sheetId="1" r:id="rId1"/></sheets></workbook>"#),
        ("xl/_rels/workbook.xml.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#),
        ("xl/sharedStrings.xml", r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>First Name</t></si><si><t>Dues</t></si><si><t>Ada</t></si><si><t>Grace</t></si></sst>"#),
        ("xl/worksheets/sheet1.xml", r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c></row><row r="2"><c r="A2" t="s"><v>2</v></c><c r="B2"><v>25</v></c></row><row r="3"><c r="A3" t="s"><v>3</v></c><c r="B3"><v>12.5</v></c></row></sheetData></worksheet>"#),
    ];
    for (name, data) in parts {
        w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        w.write_all(data.as_bytes()).unwrap();
    }
    w.finish().unwrap().into_inner()
}

#[test]
fn an_excel_list_is_a_data_source_too() {
    let wb = xlsx::read(&workbook(), None).unwrap();
    assert_eq!((wb.sheet.as_str(), wb.table.columns.clone()), ("Members", vec!["First Name".to_string(), "Dues".to_string()]));
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p, "o": 0}, "field": {"kind": "field", "name": "First Name", "after": " owes "}}));
    let p = first_para(&doc);
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": p, "o": 12}, "field": {"kind": "field", "name": "Dues", "picture": "$#,##0.00"}}));
    ops(&mut doc, json!({"op": "set_mail_merge", "merge": {"main_type": "formLetters", "source": {"kind": "xlsx", "name": "members.xlsx", "table": wb.sheet}, "columns": wb.table.columns, "rows": wb.table.rows}}));
    let bytes = doc.save().unwrap();
    let settings = part(&bytes, "word/settings.xml").unwrap();
    assert!(settings.contains("w:dataType w:val=\"native\"") && settings.contains("Members$") && settings.contains("Microsoft.ACE.OLEDB.12.0"), "{settings}");
    let (out, r) = doc.merge_to_docx(&Answers::default(), &MergeRange::default()).unwrap();
    assert_eq!(r.copies, 2);
    let lines: Vec<String> = text(&Document::open(&out).unwrap()).lines().map(String::from).collect();
    assert_eq!(lines, ["Ada owes $25.00", "Grace owes $12.50"]);
}

#[test]
fn what_word_wrote_in_the_settings_is_kept() {
    // A file made by Word: a merge linked to a text file, with settings Docxtor doesn't write.
    let mut doc = Document::new_blank();
    let mut bytes = doc.save().unwrap();
    {
        let mut pkg = Package::open(&bytes).unwrap();
        let mut settings = part(&bytes, "word/settings.xml").unwrap();
        settings = settings.replace("<w:settings ", "<w:settings xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ");
        settings = settings.replace("<w:defaultTabStop", "<w:mailMerge><w:mainDocumentType w:val=\"envelopes\"/><w:linkToQuery/><w:dataType w:val=\"textFile\"/><w:connectString w:val=\"\"/><w:query w:val=\"SELECT * FROM C:\\Users\\ada\\Documents\\clients.csv\"/><w:dataSource r:id=\"rId1\"/><w:mailSubject w:val=\"News\"/><w:viewMergedData/><w:odso><w:udl w:val=\"x\"/><w:src r:id=\"rId1\"/><w:colDelim w:val=\"44\"/><w:type w:val=\"text\"/><w:fHdr/><w:fieldMapData><w:column/><w:lid w:val=\"en-US\"/></w:fieldMapData></w:odso></w:mailMerge><w:defaultTabStop");
        pkg.set("word/settings.xml", settings.into_bytes());
        pkg.add_rel("word/settings.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/mailMergeSource", "file:///C:\\Users\\ada\\Documents\\clients.csv", true);
        bytes = pkg.to_bytes().unwrap();
    }
    let mut doc = Document::open(&bytes).unwrap();
    let info = doc.merge_info();
    assert!(info.active && info.view_merged && !info.has_recipients);
    assert_eq!((info.main_type.as_str(), info.source_name.as_deref()), ("envelopes", Some("clients.csv")));
    // An untouched document saves byte for byte.
    assert_eq!(part(&doc.save().unwrap(), "word/settings.xml"), part(&bytes, "word/settings.xml"));
    // Choosing recipients writes ours and keeps what we don't manage.
    ops(&mut doc, merge_op("envelopes", PEOPLE, "people.csv"));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let s = part(&out, "word/settings.xml").unwrap();
    assert!(s.contains("<w:mailSubject w:val=\"News\"/>") && s.contains("<w:viewMergedData/>") && s.contains("SELECT * FROM people.csv") && !s.contains("clients.csv"), "{s}");
    // Changing only the kind of document keeps the source Word had linked.
    let mut again = Document::open(&bytes).unwrap();
    ops(&mut again, json!({"op": "set_mail_merge", "merge": {"main_type": "formLetters"}}));
    let s = part(&again.save().unwrap(), "word/settings.xml").unwrap();
    assert!(s.contains("w:mainDocumentType w:val=\"formLetters\"") && s.contains("clients.csv") && s.contains("<w:mailSubject"), "{s}");
}

#[test]
fn the_ai_can_start_a_merge_and_insert_fields() {
    let mut doc = Document::new_blank();
    let p = first_para(&doc);
    let out = docxtor_core::ai::call_tool(&mut doc, "start_mail_merge", &json!({"type": "formLetters", "csv": PEOPLE, "name": "people.csv"})).unwrap();
    assert!(out.contains("3 recipients"), "{out}");
    docxtor_core::ai::call_tool(&mut doc, "insert_merge_field", &json!({"id": p.to_string(), "offset": 0, "field": {"kind": "field", "name": "First Name"}})).unwrap();
    let info = docxtor_core::ai::call_tool(&mut doc, "mail_merge_info", &json!({})).unwrap();
    assert!(info.contains("formLetters") && info.contains("First Name") && info.contains("MERGEFIELD"), "{info}");
    assert!(docxtor_core::ai::call_tool(&mut doc, "start_mail_merge", &json!({"type": "formLetters", "csv": ""})).is_err());
}

#[test]
fn the_data_sources_relationship_is_given_back_when_the_op_is_refused_or_undone() {
    let rels = |doc: &Document| doc.pkg.rels_for("word/settings.xml").unwrap_or_default().len();
    let mut doc = Document::new_blank();
    let before = rels(&doc);
    // Two ops in one batch: the second is refused, so the first one's relationship must go too.
    let good = merge_op("formLetters", PEOPLE, "people.csv");
    let bad = json!({"op": "set_mail_merge", "merge": {"main_type": "noSuchType"}});
    assert!(doc.apply_json(&json!([good.clone(), bad]).to_string(), false).is_err());
    assert_eq!(rels(&doc), before, "a refused batch left a relationship");
    ops(&mut doc, good);
    assert_eq!(rels(&doc), before + 1);
    assert!(doc.undo());
    assert_eq!(rels(&doc), before, "undo left the relationship");
    assert!(doc.redo());
    assert_eq!(rels(&doc), before + 1);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn next_fields_stop_at_the_end_of_the_range() {
    let products = docxtor_core::merge::labels::products();
    let spec = products.iter().find(|p| p.id == "avery-5160").unwrap();
    let mut doc = Document::new_blank();
    ops(&mut doc, json!({"op": "make_labels", "spec": spec, "fill": {"kind": "merge"}}));
    let first = body_paras(&doc).into_iter().next().unwrap();
    ops(&mut doc, json!({"op": "insert_merge_field", "at": {"p": first, "o": 0}, "field": {"kind": "field", "name": "First Name"}}));
    ops(&mut doc, json!({"op": "update_labels"}));
    ops(&mut doc, merge_op("mailingLabels", PEOPLE, "people.csv"));
    // The message for record 1 only: the labels after the first Next Record belong to the other recipients.
    let (bytes, r) = doc.merge_to_docx(&Answers::default(), &MergeRange { from: Some(0), to: Some(1) }).unwrap();
    assert_eq!(r.records, 1);
    let merged = text(&Document::open(&bytes).unwrap());
    assert!(merged.contains("Ada"), "{merged}");
    assert!(!merged.contains("Grace") && !merged.contains("Alan"), "{merged}");
    // The middle record alone is its own too.
    let (bytes, _) = doc.merge_to_docx(&Answers::default(), &MergeRange { from: Some(1), to: Some(2) }).unwrap();
    let merged = text(&Document::open(&bytes).unwrap());
    assert!(merged.contains("Grace") && !merged.contains("Alan") && !merged.contains("Ada"), "{merged}");
}
