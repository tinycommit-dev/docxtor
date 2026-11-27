//! Content controls, legacy form fields, document protection and Mark as final.

use docxtor_core::model::{paragraph_order, walk_paragraphs};
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

const NS: &str = r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml" mc:Ignorable="w14 w15""#;

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

fn part(bytes: &[u8], name: &str) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

fn package_with(body: &str, extra: impl FnOnce(&mut Package)) -> Vec<u8> {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:document ", &format!("<w:document {NS} "), 1).replacen("<w:body><w:p/>", &format!("<w:body>{body}"), 1);
    assert!(xml.contains(body), "the body was not replaced");
    pkg.set("word/document.xml", xml.into_bytes());
    extra(&mut pkg);
    pkg.to_bytes().unwrap()
}

fn open_with(body: &str) -> Document {
    Document::open(&package_with(body, |_| {})).unwrap()
}

fn ops(doc: &mut Document, v: Value) {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"));
}

fn err(doc: &mut Document, v: Value) -> String {
    doc.apply_json(&v.to_string(), false).expect_err(&v.to_string()).to_string()
}

fn saved(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    part(&bytes, "word/document.xml")
}

fn paras(doc: &Document) -> Vec<u32> {
    paragraph_order(&doc.body)
}

fn text_of(doc: &Document, id: u32) -> String {
    let mut t = String::new();
    walk_paragraphs(&doc.body, &mut |p| {
        if p.id == id {
            t = p.plain_text();
        }
    });
    t
}

/// The first paragraph's view as JSON.
fn para_json(doc: &mut Document, n: usize) -> Value {
    let out = doc.render_all();
    let all = serde_json::to_value(&out.blocks).unwrap();
    fn find(v: &Value, out: &mut Vec<Value>) {
        match v["t"].as_str() {
            Some("p") => out.push(v.clone()),
            Some("c") => v["blocks"].as_array().unwrap().iter().for_each(|b| find(b, out)),
            Some("tbl") => {
                for r in v["rows"].as_array().unwrap() {
                    for c in r["cells"].as_array().unwrap() {
                        c["blocks"].as_array().unwrap().iter().for_each(|b| find(b, out));
                    }
                }
            }
            _ => {}
        }
    }
    let mut ps = Vec::new();
    all.as_array().unwrap().iter().for_each(|b| find(b, &mut ps));
    ps[n].clone()
}

const CHECKBOX: &str = r#"<w:p><w:r><w:t xml:space="preserve">Agree </w:t></w:r><w:sdt><w:sdtPr><w:alias w:val="Agree"/><w:id w:val="123456"/><w14:checkbox><w14:checked w14:val="0"/><w14:checkedState w14:val="2612" w14:font="MS Gothic"/><w14:uncheckedState w14:val="2610" w14:font="MS Gothic"/></w14:checkbox></w:sdtPr><w:sdtContent><w:r><w:rPr><w:rFonts w:ascii="MS Gothic" w:eastAsia="MS Gothic" w:hAnsi="MS Gothic" w:hint="eastAsia"/></w:rPr><w:t>☐</w:t></w:r></w:sdtContent></w:sdt></w:p>"#;

const DROPDOWN: &str = r#"<w:p><w:sdt><w:sdtPr><w:alias w:val="Colour"/><w:id w:val="42"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013438"/></w:placeholder><w:showingPlcHdr/><w:dropDownList><w:listItem w:value="Choose an item."/><w:listItem w:displayText="Red" w:value="r"/><w:listItem w:displayText="Blue" w:value="b"/></w:dropDownList></w:sdtPr><w:sdtContent><w:r><w:rPr><w:rStyle w:val="PlaceholderText"/></w:rPr><w:t>Choose an item.</w:t></w:r></w:sdtContent></w:sdt></w:p>"#;

const DATE: &str = r#"<w:p><w:sdt><w:sdtPr><w:id w:val="43"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013437"/></w:placeholder><w:showingPlcHdr/><w:date><w:dateFormat w:val="dddd, MMMM d, yyyy"/><w:lid w:val="en-US"/><w:storeMappedDataAs w:val="dateTime"/><w:calendar w:val="gregorian"/></w:date></w:sdtPr><w:sdtContent><w:r><w:rPr><w:rStyle w:val="PlaceholderText"/></w:rPr><w:t>Click or tap to enter a date.</w:t></w:r></w:sdtContent></w:sdt></w:p>"#;

const TEXT: &str = r#"<w:p><w:r><w:t xml:space="preserve">Name: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="44"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013440"/></w:placeholder><w:showingPlcHdr/><w:text/></w:sdtPr><w:sdtContent><w:r><w:rPr><w:rStyle w:val="PlaceholderText"/></w:rPr><w:t>Click or tap here to enter text.</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve"> end</w:t></w:r></w:p>"#;

fn control(p: u32, n: usize) -> Value {
    json!({"p": p, "n": n})
}

// ---------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------

#[test]
fn controls_are_described_to_the_editor() {
    let mut doc = open_with(&format!("{CHECKBOX}{DROPDOWN}{DATE}{TEXT}"));
    let v = para_json(&mut doc, 0);
    let c = &v["controls"][0];
    assert_eq!(c["kind"], "checkbox");
    assert_eq!(c["checked"], false);
    assert_eq!(c["alias"], "Agree");
    assert_eq!((c["start"].as_u64(), c["end"].as_u64()), (Some(6), Some(7)));
    let c = &para_json(&mut doc, 1)["controls"][0];
    assert_eq!(c["kind"], "dropDown");
    assert_eq!(c["placeholder"], true);
    assert_eq!(c["items"].as_array().unwrap().len(), 3);
    assert_eq!(c["items"][1]["display"], "Red");
    let c = &para_json(&mut doc, 2)["controls"][0];
    assert_eq!((c["kind"].as_str(), c["dateFormat"].as_str()), (Some("date"), Some("dddd, MMMM d, yyyy")));
    let c = &para_json(&mut doc, 3)["controls"][0];
    assert_eq!(c["kind"], "text");
}

#[test]
fn an_untouched_document_with_controls_is_saved_byte_for_byte() {
    let bytes = package_with(&format!("{CHECKBOX}{DROPDOWN}{DATE}{TEXT}"), |_| {});
    let before = part(&bytes, "word/document.xml");
    let mut doc = Document::open(&bytes).unwrap();
    assert_eq!(saved(&mut doc), before);
}

// ---------------------------------------------------------------------------
// Filling in
// ---------------------------------------------------------------------------

#[test]
fn a_checkbox_toggles_and_undoes() {
    let mut doc = open_with(CHECKBOX);
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"checked":true}));
    let xml = saved(&mut doc);
    assert!(xml.contains(r#"<w14:checked w14:val="1"/>"#), "{xml}");
    assert!(xml.contains("☒"));
    assert!(!xml.contains("☐"));
    assert_eq!(para_json(&mut doc, 0)["controls"][0]["checked"], true);
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"checked":false}));
    assert!(saved(&mut doc).contains("☐"));
    assert!(doc.undo());
    assert!(saved(&mut doc).contains("☒"));
}

#[test]
fn a_dropdown_takes_an_entry_and_shows_its_placeholder_again() {
    let mut doc = open_with(DROPDOWN);
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"index":2}));
    assert_eq!(text_of(&doc, p), "Blue");
    let xml = saved(&mut doc);
    assert!(!xml.contains("showingPlcHdr"), "{xml}");
    assert!(!xml.contains("PlaceholderText"));
    // The prompt is the first entry: choosing it brings the placeholder back.
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"index":0}));
    let xml = saved(&mut doc);
    assert!(xml.contains("showingPlcHdr") && xml.contains("PlaceholderText"));
    assert_eq!(text_of(&doc, p), "Choose an item.");
    assert!(err(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"index":9})).contains("entries"));
}

#[test]
fn a_date_is_written_with_its_full_date_and_formatted_text() {
    let mut doc = open_with(DATE);
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"date":"2026-10-14"}));
    assert_eq!(text_of(&doc, p), "Wednesday, October 14, 2026");
    let xml = saved(&mut doc);
    assert!(xml.contains(r#"w:fullDate="2026-10-14T00:00:00Z""#), "{xml}");
    assert!(!xml.contains("showingPlcHdr"));
    ops(&mut doc, json!({"op":"control","action":"props","target":control(p,0),"props":{"dateFormat":"d MMM yyyy"}}));
    assert_eq!(text_of(&doc, p), "14 Oct 2026");
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"date":""}));
    assert_eq!(text_of(&doc, p), "Click or tap to enter a date.");
    assert!(saved(&mut doc).contains("showingPlcHdr"));
}

#[test]
fn typing_over_a_placeholder_removes_it_and_emptying_brings_it_back() {
    let mut doc = open_with(TEXT);
    let p = paras(&doc)[0];
    // The editor sends what is typed over a placeholder as the control's new text.
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"text":"Ada"}));
    assert_eq!(text_of(&doc, p), "Name: Ada end");
    let xml = saved(&mut doc);
    assert!(!xml.contains("showingPlcHdr"), "{xml}");
    assert!(!xml.contains("PlaceholderText"));
    assert!(xml.contains("<w:text/>"));
    // Deleting it all puts the placeholder back, as Word does.
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":6},"to":{"p":p,"o":9}}));
    assert_eq!(text_of(&doc, p), "Name: Click or tap here to enter text. end");
    let xml = saved(&mut doc);
    assert!(xml.contains("showingPlcHdr") && xml.contains("PlaceholderText"), "{xml}");
    assert_eq!(para_json(&mut doc, 0)["controls"][0]["placeholder"], true);
    // Typing into the empty control puts the text inside it.
    ops(&mut doc, json!({"op":"control","action":"props","target":control(p,0),"props":{"placeholder":"Name"}}));
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"text":"x"}));
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":6},"to":{"p":p,"o":7}}));
    assert_eq!(text_of(&doc, p), "Name: Name end");
}

#[test]
fn typing_inside_a_placeholder_keeps_only_what_was_typed() {
    let mut doc = open_with(TEXT);
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":12},"text":"Ada "}));
    assert_eq!(text_of(&doc, p), "Name: Ada  end");
    assert!(!saved(&mut doc).contains("showingPlcHdr"));
    assert!(doc.undo());
    assert_eq!(text_of(&doc, p), "Name: Click or tap here to enter text. end");
    // Deleting part of a placeholder leaves it as it was.
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":8},"to":{"p":p,"o":10}}));
    assert_eq!(text_of(&doc, p), "Name: Click or tap here to enter text. end");
}

#[test]
fn text_typed_into_an_empty_control_goes_inside_it() {
    let body = r#"<w:p><w:r><w:t xml:space="preserve">A </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="5"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t></w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve"> Z</w:t></w:r></w:p>"#;
    let mut doc = open_with(body);
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":2},"text":"mid"}));
    assert_eq!(doc.list_controls()[0].2, "mid");
}

// ---------------------------------------------------------------------------
// Locks
// ---------------------------------------------------------------------------

fn locked(lock: &str) -> String {
    format!(r#"<w:p><w:r><w:t xml:space="preserve">A </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="9"/><w:lock w:val="{lock}"/><w:richText/></w:sdtPr><w:sdtContent><w:r><w:t>locked</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve"> Z</w:t></w:r></w:p>"#)
}

#[test]
fn a_content_locked_control_cannot_be_edited_but_can_be_deleted() {
    let mut doc = open_with(&locked("contentLocked"));
    let p = paras(&doc)[0];
    assert!(err(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":4},"text":"x"})).contains("locked"));
    assert!(err(&mut doc, json!({"op":"delete","from":{"p":p,"o":3},"to":{"p":p,"o":5}})).contains("locked"));
    assert_eq!(text_of(&doc, p), "A locked Z");
    // Text outside is free, and the whole control can go.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":">"}));
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":11}}));
    assert_eq!(text_of(&doc, p), "");
}

#[test]
fn a_locked_control_cannot_be_deleted_but_can_be_edited() {
    let mut doc = open_with(&locked("sdtLocked"));
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":4},"text":"x"}));
    assert_eq!(text_of(&doc, p), "A loxcked Z");
    assert!(err(&mut doc, json!({"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":11}})).contains("can't be deleted"));
    assert!(err(&mut doc, json!({"op":"control","action":"remove","target":control(p,0)})).contains("can't be deleted"));
}

#[test]
fn a_fully_locked_control_cannot_be_changed_by_a_value_op_either() {
    let mut doc = open_with(&locked("sdtContentLocked").replace("<w:richText/>", "<w:text/>"));
    let p = paras(&doc)[0];
    assert!(err(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"text":"x"})).contains("locked"));
    // Unlocking is a property change.
    ops(&mut doc, json!({"op":"control","action":"props","target":control(p,0),"props":{"lock":"unlocked"}}));
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"text":"x"}));
    assert_eq!(text_of(&doc, p), "A x Z");
    assert!(!saved(&mut doc).contains("w:lock"));
}

// ---------------------------------------------------------------------------
// Inserting and properties
// ---------------------------------------------------------------------------

#[test]
fn every_kind_can_be_inserted_and_survives_a_round_trip() {
    for kind in docxtor_core::controls::KINDS {
        let mut doc = Document::new_blank();
        let p = paras(&doc)[0];
        ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"ab"}));
        ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":1},"kind":kind,"props":{"alias":"Thing","tag":"thing","color":"ff0000","appearance":"tags"}}));
        let bytes = doc.save().unwrap();
        assert_valid(&bytes);
        let mut re = Document::open(&bytes).unwrap();
        let all = re.list_controls();
        let found = all.iter().find(|(_, c, _)| c.kind == *kind).unwrap_or_else(|| panic!("no {kind}: {all:?}"));
        assert_eq!(found.1.alias.as_deref(), Some("Thing"), "{kind}");
        assert_eq!(found.1.tag.as_deref(), Some("thing"));
        assert_eq!(found.1.color.as_deref(), Some("FF0000"));
        assert_eq!(found.1.appearance.as_deref(), Some("tags"));
        let xml = saved(&mut re);
        assert!(xml.contains("w15:color") && xml.contains("w15:appearance"), "{kind}: {xml}");
        assert!(doc.undo());
        assert_eq!(doc.list_controls().len(), 0, "{kind}");
    }
}

#[test]
fn placeholder_text_goes_in_the_glossary_as_word_keeps_it() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":0},"kind":"text","props":{"placeholder":"Your name"}}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let g = part(&bytes, "word/glossary/document.xml");
    assert!(g.contains("Your name") && g.contains(r#"<w:gallery w:val="placeholder"/>"#) && g.contains("bbPlcHdr"), "{g}");
    let xml = part(&bytes, "word/document.xml");
    assert!(xml.contains("<w:showingPlcHdr/>") && xml.contains("<w:text/>") && xml.contains("<w:placeholder>"), "{xml}");
    assert!(part(&bytes, "[Content_Types].xml").contains("glossary+xml"));
    assert!(part(&bytes, "word/_rels/document.xml.rels").contains("glossaryDocument"));
    // The document gets Word's placeholder style.
    assert!(part(&bytes, "word/styles.xml").contains("PlaceholderText"));
    // Emptying the control brings the custom placeholder back.
    let mut re = Document::open(&bytes).unwrap();
    let p = paras(&re)[0];
    ops(&mut re, json!({"op":"control","action":"set","target":control(p,0),"text":"Ada"}));
    ops(&mut re, json!({"op":"control","action":"set","target":control(p,0),"text":""}));
    assert_eq!(text_of(&re, p), "Your name");
    // The same building block is reused for the same text.
    ops(&mut re, json!({"op":"control","action":"insert","at":{"p":p,"o":0},"kind":"richText","props":{"placeholder":"Your name"}}));
    let g2 = part(&re.save().unwrap(), "word/glossary/document.xml");
    assert_eq!(g2.matches("<w:docPart>").count(), 1);
}

#[test]
fn the_properties_of_a_control_are_edited_and_written_in_order() {
    let mut doc = open_with(DROPDOWN);
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"control","action":"props","target":control(p,0),"props":{
        "alias":"Paint","tag":"paint","color":"00FF00","lock":"sdtLocked",
        "items":[{"display":"Red","value":"r"},{"display":"Green","value":"g"}]}}));
    let xml = saved(&mut doc);
    let order = ["<w:alias", "<w:tag", "<w:id", "<w:lock", "<w:placeholder", "<w:showingPlcHdr", "<w15:color", "<w:dropDownList"];
    let at: Vec<usize> = order.iter().map(|o| xml.find(o).unwrap_or_else(|| panic!("{o} in {xml}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{xml}");
    assert!(xml.contains(r#"<w:listItem w:displayText="Green" w:value="g"/>"#));
    let c = &para_json(&mut doc, 0)["controls"][0];
    assert_eq!(c["lock"], "sdtLocked");
    assert_eq!(c["items"].as_array().unwrap().len(), 2);
    // Removing the alias removes the element.
    ops(&mut doc, json!({"op":"control","action":"props","target":control(p,0),"props":{"alias":""}}));
    assert!(!saved(&mut doc).contains("w:alias"));
}

#[test]
fn a_selection_becomes_the_content_of_a_rich_text_control() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"hello big world"}));
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":6},"to":{"p":p,"o":9},"kind":"richText"}));
    assert_eq!(text_of(&doc, p), "hello big world");
    let c = doc.list_controls();
    assert_eq!((c[0].1.start, c[0].1.end, c[0].2.as_str()), (6, 9, "big"));
    ops(&mut doc, json!({"op":"control","action":"remove","target":control(p,0)}));
    assert!(doc.list_controls().is_empty());
    assert_eq!(text_of(&doc, p), "hello big world");
}

#[test]
fn a_picture_control_takes_a_picture() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":0},"kind":"picture"}));
    let media = doc.add_media("png", vec![0x89, b'P', b'N', b'G']);
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"picture":{"media":media,"width":100.0,"height":50.0,"alt":"A logo"}}));
    let xml = saved(&mut doc);
    assert!(xml.contains("<w:picture/>") && xml.contains("<w:drawing"), "{xml}");
    assert!(!xml.contains("showingPlcHdr"));
    assert!(xml.contains(r#"descr="A logo""#));
}

// ---------------------------------------------------------------------------
// Repeating sections
// ---------------------------------------------------------------------------

const REPEATING: &str = r#"<w:sdt><w:sdtPr><w:alias w:val="Lines"/><w:id w:val="500"/><w15:repeatingSection><w15:sectionTitle w15:val="Line"/></w15:repeatingSection></w:sdtPr><w:sdtContent><w:sdt><w:sdtPr><w:id w:val="501"/><w15:repeatingSectionItem/></w:sdtPr><w:sdtContent><w:p><w:r><w:t xml:space="preserve">Item: </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="502"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013440"/></w:placeholder><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>filled in</w:t></w:r></w:sdtContent></w:sdt></w:p></w:sdtContent></w:sdt></w:sdtContent></w:sdt>"#;

#[test]
fn a_repeating_section_gets_an_item_with_clean_controls() {
    let mut doc = open_with(REPEATING);
    let render = doc.render_all();
    let v = serde_json::to_value(&render.blocks).unwrap();
    let sec = &v[0];
    assert_eq!(sec["control"]["kind"], "repeating");
    assert_eq!(sec["control"]["sectionTitle"], "Line");
    let section_id = sec["id"].as_u64().unwrap();
    let item = sec["blocks"][0]["id"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"control","action":"repeat","target":{"block":item}}));
    let after = paras(&doc);
    assert_eq!(after.len(), 2);
    assert_eq!(text_of(&doc, after[0]), "Item: filled in");
    assert_eq!(text_of(&doc, after[1]), "Item: Click or tap here to enter text.");
    let xml = saved(&mut doc);
    assert_eq!(xml.matches("<w15:repeatingSectionItem/>").count(), 2);
    // The copies carry their own ids.
    let ids: Vec<&str> = xml.split("<w:id w:val=\"").skip(1).map(|s| &s[..s.find('"').unwrap()]).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(ids.len(), sorted.len(), "{ids:?}");
    // A section adds at its end, an item can be removed, the last one stays.
    ops(&mut doc, json!({"op":"control","action":"repeat","target":{"block":section_id}}));
    assert_eq!(paras(&doc).len(), 3);
    let item2 = serde_json::to_value(&doc.render_all().blocks).unwrap()[0]["blocks"][1]["id"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"control","action":"repeat","target":{"block":item2},"remove":true}));
    assert_eq!(paras(&doc).len(), 2);
    ops(&mut doc, json!({"op":"control","action":"repeat","target":{"block":item},"remove":true}));
    let last = serde_json::to_value(&doc.render_all().blocks).unwrap()[0]["blocks"][0]["id"].as_u64().unwrap();
    assert!(err(&mut doc, json!({"op":"control","action":"repeat","target":{"block":last},"remove":true})).contains("at least one"));
    assert!(doc.undo());
}

const REPEATING_ROWS: &str = r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>Head</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Qty</w:t></w:r></w:p></w:tc></w:tr><w:sdt><w:sdtPr><w:id w:val="600"/><w15:repeatingSection/></w:sdtPr><w:sdtContent><w:sdt><w:sdtPr><w:id w:val="601"/><w15:repeatingSectionItem/></w:sdtPr><w:sdtContent><w:tr><w:tc><w:p><w:r><w:t>Pen</w:t></w:r></w:p></w:tc><w:tc><w:sdt><w:sdtPr><w:id w:val="602"/><w:placeholder><w:docPart w:val="DefaultPlaceholder_-1854013440"/></w:placeholder><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>3</w:t></w:r></w:sdtContent></w:sdt></w:p></w:tc></w:tr></w:sdtContent></w:sdt></w:sdtContent></w:sdt></w:tbl><w:p/>"#;

#[test]
fn table_rows_can_be_a_repeating_section() {
    // The control inside the cell sits in the paragraph; make the cell hold a paragraph around it.
    let body = REPEATING_ROWS.replace("<w:tc><w:sdt>", "<w:tc><w:p><w:sdt>");
    let mut doc = open_with(&body);
    let v = serde_json::to_value(&doc.render_all().blocks).unwrap();
    let row = &v[0]["rows"][1];
    assert_eq!(row["control"]["kind"], "repeatingItem");
    assert_eq!(row["section"]["kind"], "repeating");
    let item = row["control"]["ctl"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"control","action":"repeat","target":{"wrap":item}}));
    let v = serde_json::to_value(&doc.render_all().blocks).unwrap();
    assert_eq!(v[0]["rows"].as_array().unwrap().len(), 3);
    let xml = saved(&mut doc);
    assert_eq!(xml.matches("<w15:repeatingSectionItem/>").count(), 2);
    assert_eq!(xml.matches("<w15:repeatingSection/>").count(), 1);
    // The new row starts clean.
    let texts: Vec<String> = paras(&doc).iter().map(|p| text_of(&doc, *p)).collect();
    assert!(texts.contains(&"Click or tap here to enter text.".to_string()), "{texts:?}");
    let second = v[0]["rows"][2]["control"]["ctl"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"control","action":"repeat","target":{"wrap":second},"remove":true}));
    assert_eq!(serde_json::to_value(&doc.render_all().blocks).unwrap()[0]["rows"].as_array().unwrap().len(), 2);
    let only = serde_json::to_value(&doc.render_all().blocks).unwrap()[0]["rows"][1]["control"]["ctl"].as_u64().unwrap();
    assert!(err(&mut doc, json!({"op":"control","action":"repeat","target":{"wrap":only},"remove":true})).contains("at least one"));
}

#[test]
fn removing_a_repeating_row_removes_the_range_markers_in_its_control() {
    let body = REPEATING_ROWS.replace("<w:tc><w:sdt>", "<w:tc><w:p><w:sdt>");
    // A second item whose control also holds a bookmark around its row.
    let second = r#"<w:sdt><w:sdtPr><w:id w:val="611"/><w15:repeatingSectionItem/></w:sdtPr><w:sdtContent><w:bookmarkStart w:id="9" w:name="line2"/><w:tr><w:tc><w:p><w:r><w:t>Ink</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc></w:tr><w:bookmarkEnd w:id="9"/></w:sdtContent></w:sdt>"#;
    let body = body.replace("</w:sdtContent></w:sdt></w:sdtContent></w:sdt></w:tbl>", &format!("</w:sdtContent></w:sdt>{second}</w:sdtContent></w:sdt></w:tbl>"));
    let mut doc = open_with(&body);
    let v = serde_json::to_value(&doc.render_all().blocks).unwrap();
    assert_eq!(v[0]["rows"].as_array().unwrap().len(), 3);
    let item = v[0]["rows"][2]["control"]["ctl"].as_u64().unwrap();
    ops(&mut doc, json!({"op":"control","action":"repeat","target":{"wrap":item},"remove":true}));
    let xml = saved(&mut doc);
    assert_eq!(xml.matches("<w15:repeatingSectionItem/>").count(), 1, "{xml}");
    assert!(!xml.contains("bookmarkStart") && !xml.contains("bookmarkEnd"), "the markers go with the row: {xml}");
    assert!(!xml.contains("<w:sdtContent></w:sdtContent>") && !xml.contains("<w:sdtContent/>"), "no empty item is left: {xml}");
    // Undo brings the row and its bookmark back.
    assert!(doc.undo());
    let xml = saved(&mut doc);
    assert_eq!(xml.matches("<w15:repeatingSectionItem/>").count(), 2);
    assert_eq!(xml.matches("w:name=\"line2\"").count(), 1);
}

// ---------------------------------------------------------------------------
// Data binding
// ---------------------------------------------------------------------------

const BOUND: &str = r#"<w:p><w:sdt><w:sdtPr><w:id w:val="70"/><w:dataBinding w:prefixMappings="xmlns:ns0='urn:acme'" w:xpath="/ns0:order[1]/ns0:customer[1]" w:storeItemID="{11111111-2222-3333-4444-555555555555}"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>stale</w:t></w:r></w:sdtContent></w:sdt><w:r><w:t xml:space="preserve"> / </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="71"/><w:dataBinding w:prefixMappings="xmlns:ns0='urn:acme'" w:xpath="/ns0:order[1]/ns0:customer[1]" w:storeItemID="{11111111-2222-3333-4444-555555555555}"/><w:text/></w:sdtPr><w:sdtContent><w:r><w:t>stale</w:t></w:r></w:sdtContent></w:sdt></w:p>"#;

fn with_custom_xml(pkg: &mut Package) {
    pkg.set("customXml/item1.xml", br#"<order xmlns="urn:acme"><customer>Acme</customer></order>"#.to_vec());
    pkg.set(
        "customXml/itemProps1.xml",
        br#"<ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{11111111-2222-3333-4444-555555555555}"/>"#.to_vec(),
    );
    pkg.add_rel("customXml/item1.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps", "itemProps1.xml", false);
}

#[test]
fn a_bound_control_shows_the_xml_value_and_writes_edits_back() {
    let bytes = package_with(BOUND, with_custom_xml);
    let mut doc = Document::open(&bytes).unwrap();
    let p = paras(&doc)[0];
    assert_eq!(text_of(&doc, p), "Acme / Acme");
    // Typing in one control updates the XML and the other control.
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p,0),"text":"Globex"}));
    assert_eq!(text_of(&doc, p), "Globex / Globex");
    let out = doc.save().unwrap();
    assert!(part(&out, "customXml/item1.xml").contains("<customer>Globex</customer>"));
    // Undo puts the value back in the controls and the part.
    assert!(doc.undo());
    assert_eq!(text_of(&doc, p), "Acme / Acme");
    // Editing the text directly does the same.
    ops(&mut doc, json!([{"op":"delete","from":{"p":p,"o":0},"to":{"p":p,"o":4}},{"op":"insert_text","at":{"p":p,"o":0},"text":"Initech"}]));
    assert_eq!(text_of(&doc, p), "Initech / Initech");
    assert!(part(&doc.save().unwrap(), "customXml/item1.xml").contains("<customer>Initech</customer>"));
}

#[test]
fn opening_a_file_with_bound_controls_then_saving_leaves_its_styles_as_they_came() {
    // The values shown at open are not an edit, nor is the placeholder style that showing them may add: a file saved with no edit keeps
    // its styles part byte for byte (it used to gain a Placeholder Text style, or a styles part it did not have), and so does an undo after
    // an edit.
    let bytes = package_with(BOUND, with_custom_xml);
    let styles = |b: &[u8]| Package::open(b).unwrap().get("word/styles.xml").map(|s| s.to_vec());
    let mut doc = Document::open(&bytes).unwrap();
    assert!(styles(&doc.save().unwrap()) == styles(&bytes));
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"x"}));
    assert!(doc.undo());
    assert!(styles(&doc.save().unwrap()) == styles(&bytes));
}

// ---------------------------------------------------------------------------
// Legacy form fields
// ---------------------------------------------------------------------------

#[test]
fn legacy_form_fields_are_built_filled_and_kept_in_bounds() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Name: "}));
    ops(&mut doc, json!({"op":"form_field","action":"insert","at":{"p":p,"o":6},"kind":"text","props":{"maxLength":5,"format":"UPPERCASE"}}));
    let v = para_json(&mut doc, 0);
    assert_eq!(v["forms"][0]["kind"], "text");
    assert_eq!(v["forms"][0]["maxLength"], 5);
    let at = v["forms"][0]["start"].as_u64().unwrap() as usize;
    // Typing into the result is plain editing; the field keeps its length and format.
    ops(&mut doc, json!([{"op":"insert_text","at":{"p":p,"o":at + 2},"text":"abcdefgh"}]));
    assert_eq!(text_of(&doc, p), "Name: \u{2002}\u{2002}ABC");
    ops(&mut doc, json!({"op":"form_field","action":"set","at":{"p":p,"o":0},"n":0,"text":"abcdefgh"}));
    assert_eq!(text_of(&doc, p), "Name: ABCDE");
    // Deleting all of it leaves the field there to fill in again.
    ops(&mut doc, json!({"op":"delete","from":{"p":p,"o":at},"to":{"p":p,"o":at + 5}}));
    assert_eq!(para_json(&mut doc, 0)["forms"].as_array().unwrap().len(), 1);
    ops(&mut doc, json!({"op":"form_field","action":"set","at":{"p":p,"o":0},"n":0,"text":"zed"}));
    assert_eq!(text_of(&doc, p), "Name: ZED");
    // Check box and drop-down.
    let end = text_of(&doc, p).chars().count();
    ops(&mut doc, json!({"op":"form_field","action":"insert","at":{"p":p,"o":end},"kind":"checkbox"}));
    ops(&mut doc, json!({"op":"form_field","action":"insert","at":{"p":p,"o":end},"kind":"dropdown","props":{"items":["Tea","Coffee"]}}));
    let v = para_json(&mut doc, 0);
    assert_eq!(v["forms"].as_array().unwrap().len(), 3);
    let kinds: Vec<&str> = v["forms"].as_array().unwrap().iter().map(|f| f["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["text", "dropdown", "checkbox"]);
    ops(&mut doc, json!({"op":"form_field","action":"set","at":{"p":p,"o":0},"n":2,"checked":true}));
    ops(&mut doc, json!({"op":"form_field","action":"set","at":{"p":p,"o":0},"n":1,"index":1}));
    let v = para_json(&mut doc, 0);
    assert_eq!(v["forms"][2]["checked"], true);
    assert_eq!(v["forms"][1]["selected"], 1);
    // The drop-down and the box are drawn from segments at their place.
    let segs: Vec<String> = v["segs"].as_array().unwrap().iter().filter(|s| s["k"] == "form").map(|s| s["x"].as_str().unwrap().to_string()).collect();
    assert_eq!(segs, ["dropdown:1"]);
    // A check box is drawn where its field starts, and says which field it is.
    let boxes: Vec<(String, String)> = v["segs"].as_array().unwrap().iter().filter(|s| s["k"] == "ff").map(|s| (s["x"].as_str().unwrap().to_string(), s["fi"].as_str().unwrap().to_string())).collect();
    assert_eq!(boxes, [("1".to_string(), "2".to_string())]);
    assert_eq!(v["segs"].as_array().unwrap().iter().find(|s| s["k"] == "form").unwrap()["label"], "Coffee");
    let xml = saved(&mut doc);
    assert!(xml.contains("FORMTEXT") && xml.contains("FORMCHECKBOX") && xml.contains("FORMDROPDOWN"));
    assert!(xml.contains("<w:checked/>") && xml.contains(r#"<w:result w:val="1"/>"#) && xml.contains(r#"<w:maxLength w:val="5"/>"#));
    assert!(xml.contains(r#"<w:listEntry w:val="Coffee"/>"#));
    // Properties.
    ops(&mut doc, json!({"op":"form_field","action":"props","at":{"p":p,"o":0},"n":0,"props":{"maxLength":0,"name":"Who","help":"Your name"}}));
    let xml = saved(&mut doc);
    assert!(!xml.contains("maxLength") && xml.contains(r#"<w:name w:val="Who"/>"#) && xml.contains(r#"<w:helpText w:val="Your name"/>"#));
    assert!(doc.undo());
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

fn protected_doc(extra: &str, edit: &str) -> Document {
    let body = format!(r#"<w:p><w:r><w:t>Locked line</w:t></w:r></w:p><w:p><w:permStart w:id="7" w:edGrp="everyone"/><w:r><w:t>Open line</w:t></w:r><w:permEnd w:id="7"/></w:p>{extra}"#);
    let bytes = package_with(&body, |pkg| {
        let s = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
        let s = s.replace("<w:defaultTabStop", &format!(r#"<w:documentProtection w:edit="{edit}" w:enforcement="1"/><w:defaultTabStop"#));
        pkg.set("word/settings.xml", s.into_bytes());
    });
    Document::open(&bytes).unwrap()
}

#[test]
fn a_read_only_document_can_only_be_edited_in_its_ranges() {
    let mut doc = protected_doc("", "readOnly");
    let p = paras(&doc);
    let info = doc.protection_info();
    assert!(info.protection.enforced && info.protection.edit == "readOnly");
    assert_eq!(info.ranges.len(), 1);
    assert_eq!((info.ranges[0].from.p.0, info.ranges[0].from.o, info.ranges[0].to.o), (p[1], 0, 9));
    assert!(err(&mut doc, json!({"op":"insert_text","at":{"p":p[0],"o":0},"text":"x"})).contains("protected"));
    assert!(err(&mut doc, json!({"op":"format_text","from":{"p":p[0],"o":0},"to":{"p":p[0],"o":3},"format":{"bold":true}})).contains("protected"));
    assert!(err(&mut doc, json!({"op":"split_paragraph","at":{"p":p[0],"o":3}})).contains("protected"));
    assert!(err(&mut doc, json!({"op":"delete","from":{"p":p[0],"o":0},"to":{"p":p[1],"o":2}})).contains("protected"));
    assert_eq!(text_of(&doc, p[0]), "Locked line");
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p[1],"o":4},"text":"-"}));
    ops(&mut doc, json!({"op":"format_text","from":{"p":p[1],"o":0},"to":{"p":p[1],"o":3},"format":{"bold":true}}));
    assert_eq!(text_of(&doc, p[1]), "Open- line");
    // Enter inside the range is allowed.
    ops(&mut doc, json!({"op":"split_paragraph","at":{"p":p[1],"o":2}}));
    assert_eq!(paras(&doc).len(), 3);
    // The range is kept by the file.
    let xml = saved(&mut doc);
    assert!(xml.contains("w:permStart") && xml.contains("w:permEnd"));
    // The AI tools and the CLI go through the same ops, so they are held to it too.
    assert!(doc.undo());
}

#[test]
fn read_only_protection_also_covers_styles_theme_defaults_and_pages() {
    for edit in ["readOnly", "comments", "forms"] {
        let mut doc = protected_doc("", edit);
        let before = saved(&mut doc);
        for op in [
            json!({"op":"set_theme","theme":"facet"}),
            json!({"op":"set_style_set","style_set":"word-2010"}),
            json!({"op":"set_style_definition","style":"Heading1","def":{"bold":true}}),
            json!({"op":"delete_style","style":"Whatever"}),
            json!({"op":"set_default_format","format":{"bold":true}}),
            json!({"op":"set_paragraph_spacing","preset":"compact"}),
            json!({"op":"set_page_setup","width":10000}),
            json!({"op":"set_columns","count":2}),
        ] {
            assert!(err(&mut doc, op.clone()).contains("protected"), "{edit}: {op}");
        }
        assert_eq!(saved(&mut doc), before);
    }
    // Stopping the protection (no password) lets them through again.
    let mut doc = protected_doc("", "readOnly");
    ops(&mut doc, json!({"op":"protection","action":"stop"}));
    ops(&mut doc, json!({"op":"set_page_setup","width":10000}));
}

#[test]
fn stopping_protection_with_a_huge_spin_count_is_refused_at_once() {
    let bytes = package_with("<w:p><w:r><w:t>x</w:t></w:r></w:p>", |pkg| {
        let s = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
        let tag = r#"<w:documentProtection w:edit="readOnly" w:enforcement="1" w:algorithmName="SHA-512" w:hashValue="AAAA" w:saltValue="AAAA" w:spinCount="4000000000"/><w:defaultTabStop"#;
        pkg.set("word/settings.xml", s.replace("<w:defaultTabStop", tag).into_bytes());
    });
    let mut doc = Document::open(&bytes).unwrap();
    let t = std::time::Instant::now();
    let e = err(&mut doc, json!({"op":"protection","action":"stop","password":"anything"}));
    assert!(e.contains("rounds of password hashing"), "{e}");
    assert!(t.elapsed().as_millis() < 500);
    assert!(doc.protection_info().protection.enforced);
}

#[test]
fn limited_formatting_does_not_allow_changing_styles() {
    let mut doc = Document::new_blank();
    ops(&mut doc, json!({"op":"protection","action":"set","settings":{"formatting":true,"enforce":true}}));
    assert!(err(&mut doc, json!({"op":"set_theme","theme":"facet"})).contains("limited"));
    assert!(err(&mut doc, json!({"op":"set_paragraph_spacing","preset":"compact"})).contains("limited"));
}

#[test]
fn the_ranges_follow_the_users_group() {
    let body = r#"<w:p><w:permStart w:id="1" w:edGrp="editors"/><w:r><w:t>Editors</w:t></w:r><w:permEnd w:id="1"/></w:p><w:p><w:permStart w:id="2" w:ed="someone@example.com"/><w:r><w:t>Someone</w:t></w:r><w:permEnd w:id="2"/></w:p>"#;
    let bytes = package_with(body, |pkg| {
        let s = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/settings.xml", s.replace("<w:defaultTabStop", r#"<w:documentProtection w:edit="readOnly" w:enforcement="1"/><w:defaultTabStop"#).into_bytes());
    });
    let mut doc = Document::open(&bytes).unwrap();
    assert!(doc.protection_info().ranges.is_empty());
    let p = paras(&doc);
    assert!(err(&mut doc, json!({"op":"insert_text","at":{"p":p[0],"o":0},"text":"x"})).contains("protected"));
    doc.set_user_groups(vec!["editors".into()]);
    assert_eq!(doc.protection_info().ranges.len(), 1);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p[0],"o":0},"text":"x"}));
}

#[test]
fn a_forms_document_lets_controls_and_unprotected_sections_be_edited() {
    // The first section ends with the paragraph that holds its properties; the last one has forms protection off.
    let section_break = r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr></w:pPr><w:r><w:t>End of first section</w:t></w:r></w:p>"#;
    let body = format!(r#"<w:p><w:r><w:t>Locked line</w:t></w:r></w:p>{CHECKBOX}{TEXT}{section_break}<w:p><w:r><w:t>Free section</w:t></w:r></w:p>"#);
    let bytes = package_with(&body, |pkg| {
        let s = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/settings.xml", s.replace("<w:defaultTabStop", r#"<w:documentProtection w:edit="forms" w:enforcement="1"/><w:defaultTabStop"#).into_bytes());
        let d = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/document.xml", d.replace("</w:sectPr></w:body>", r#"<w:formProt w:val="0"/></w:sectPr></w:body>"#).into_bytes());
    });
    let mut doc = Document::open(&bytes).unwrap();
    let p = paras(&doc);
    assert!(err(&mut doc, json!({"op":"insert_text","at":{"p":p[0],"o":0},"text":"x"})).contains("protected"));
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p[1],0),"checked":true}));
    ops(&mut doc, json!({"op":"control","action":"set","target":control(p[2],0),"text":"Ada"}));
    assert_eq!(text_of(&doc, p[2]), "Name: Ada end");
    // Text next to a control stays protected.
    assert!(err(&mut doc, json!({"op":"insert_text","at":{"p":p[2],"o":0},"text":"x"})).contains("protected"));
    // The section with forms protection off is free.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p[4],"o":0},"text":"free "}));
    assert_eq!(doc.protection_info().open_paragraphs, vec![p[4]]);
}

#[test]
fn legacy_form_fields_can_be_filled_in_a_forms_document() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"A "}));
    ops(&mut doc, json!({"op":"form_field","action":"insert","at":{"p":p,"o":2},"kind":"text"}));
    ops(&mut doc, json!({"op":"form_field","action":"insert","at":{"p":p,"o":2},"kind":"checkbox"}));
    ops(&mut doc, json!({"op":"protection","action":"set","settings":{"edit":"forms","enforce":true}}));
    assert!(err(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"x"})).contains("protected"));
    ops(&mut doc, json!({"op":"form_field","action":"set","at":{"p":p,"o":0},"n":1,"text":"hi"}));
    ops(&mut doc, json!({"op":"form_field","action":"set","at":{"p":p,"o":0},"n":0,"checked":true}));
    assert_eq!(text_of(&doc, p), "A hi");
}

#[test]
fn starting_and_stopping_protection_with_a_password() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Text"}));
    ops(&mut doc, json!({"op":"protection","action":"set","settings":{"edit":"readOnly","enforce":true,"password":"pw1","salt":"AQIDBAUGBwgJCgsMDQ4PEA=="}}));
    let xml = part(&doc.save().unwrap(), "word/settings.xml");
    for want in [r#"w:edit="readOnly""#, r#"w:enforcement="1""#, r#"w:cryptProviderType="rsaAES""#, r#"w:cryptAlgorithmClass="hash""#, r#"w:cryptAlgorithmType="typeAny""#, r#"w:cryptAlgorithmSid="14""#, r#"w:cryptSpinCount="100000""#, r#"w:salt="AQIDBAUGBwgJCgsMDQ4PEA==""#, "w:hash="] {
        assert!(xml.contains(want), "{want} in {xml}");
    }
    // The element sits where the schema puts it.
    assert!(xml.find("w:documentProtection").unwrap() < xml.find("w:defaultTabStop").unwrap());
    assert!(err(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"x"})).contains("protected"));
    assert!(err(&mut doc, json!({"op":"protection","action":"set","settings":{"edit":"forms"}})).contains("stop"));
    assert!(err(&mut doc, json!({"op":"protection","action":"stop","password":"nope"})).contains("wrong password"));
    assert!(err(&mut doc, json!({"op":"protection","action":"stop"})).contains("wrong password"));
    assert!(doc.protection().enforced);
    ops(&mut doc, json!({"op":"protection","action":"stop","password":"pw1"}));
    assert!(!doc.protection().enforced);
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"x"}));
    let xml = part(&doc.save().unwrap(), "word/settings.xml");
    assert!(xml.contains(r#"w:enforcement="0""#) && !xml.contains("w:hash="), "{xml}");
    // Undo brings the protection back.
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(doc.protection().enforced && doc.protection().has_password);
}

#[test]
fn a_file_with_a_modern_hash_is_checked_by_it() {
    use docxtor_core::protection::{b64_encode, hash_password};
    let salt = [9u8; 16];
    let h = hash_password("SHA-512", &salt, 1000, "open sesame").unwrap();
    let settings = format!(
        r#"<w:documentProtection w:edit="readOnly" w:enforcement="1" w:algorithmName="SHA-512" w:hashValue="{}" w:saltValue="{}" w:spinCount="1000"/>"#,
        b64_encode(&h),
        b64_encode(&salt)
    );
    let bytes = package_with("<w:p><w:r><w:t>x</w:t></w:r></w:p>", |pkg| {
        let s = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
        pkg.set("word/settings.xml", s.replace("<w:defaultTabStop", &format!("{settings}<w:defaultTabStop")).into_bytes());
    });
    let mut doc = Document::open(&bytes).unwrap();
    assert!(err(&mut doc, json!({"op":"protection","action":"stop","password":"open"})).contains("wrong password"));
    ops(&mut doc, json!({"op":"protection","action":"stop","password":"open sesame"}));
    assert!(!doc.protection().enforced);
}

#[test]
fn a_file_with_only_the_old_hash_is_checked_by_the_old_algorithm() {
    let v = docxtor_core::protection::legacy_verifier("secret");
    let bytes = package_with("<w:p><w:r><w:t>x</w:t></w:r></w:p>", |pkg| {
        let s = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
        let el = format!(r#"<w:documentProtection w:edit="forms" w:enforcement="1" w:unprotectPassword="{v:08X}"/>"#);
        pkg.set("word/settings.xml", s.replace("<w:defaultTabStop", &format!("{el}<w:defaultTabStop")).into_bytes());
    });
    let mut doc = Document::open(&bytes).unwrap();
    assert!(doc.protection().legacy_hash && doc.protection().has_password);
    assert!(err(&mut doc, json!({"op":"protection","action":"stop","password":"guess"})).contains("wrong password"));
    ops(&mut doc, json!({"op":"protection","action":"stop","password":"secret"}));
}

#[test]
fn formatting_can_be_limited_to_some_styles() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Text"}));
    ops(&mut doc, json!({"op":"set_style","ids":[p],"style":"Heading 1"}));
    ops(&mut doc, json!({"op":"set_style","ids":[p],"style":"Title"}));
    ops(&mut doc, json!({"op":"set_style","ids":[p],"style":"Normal"}));
    ops(&mut doc, json!({"op":"protection","action":"set","settings":{"formatting":true,"enforce":true,"allowedStyles":["Normal","Heading1"]}}));
    assert!(err(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":2},"format":{"bold":true}})).contains("limited"));
    ops(&mut doc, json!({"op":"set_style","ids":[p],"style":"Heading 1"}));
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"ok "}));
    let info = doc.protection_info();
    assert!(info.protection.formatting && !info.protection.restricts_editing());
    assert!(!info.locked_styles.contains(&"Heading1".to_string()) && info.locked_styles.contains(&"Title".to_string()));
    assert!(err(&mut doc, json!({"op":"set_style","ids":[p],"style":"Title"})).contains("isn't allowed"));
    ops(&mut doc, json!({"op":"protection","action":"stop"}));
    let styles = part(&doc.save().unwrap(), "word/styles.xml");
    assert!(styles.contains("<w:locked/>"));
}

#[test]
fn mark_as_final_is_a_custom_property_and_undoes() {
    let mut doc = Document::new_blank();
    assert!(!doc.is_marked_final());
    ops(&mut doc, json!({"op":"protection","action":"mark_final","on":true}));
    assert!(doc.is_marked_final());
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let c = part(&bytes, "docProps/custom.xml");
    assert!(c.contains(r#"name="_MarkAsFinal""#) && c.contains("<vt:bool>true</vt:bool>"), "{c}");
    assert!(part(&bytes, "_rels/.rels").contains("custom-properties"));
    assert!(part(&bytes, "[Content_Types].xml").contains("custom-properties+xml"));
    assert!(Document::open(&bytes).unwrap().is_marked_final());
    ops(&mut doc, json!({"op":"protection","action":"mark_final","on":false}));
    assert!(!doc.is_marked_final());
    assert!(doc.undo());
    assert!(doc.is_marked_final());
}

#[test]
fn editable_ranges_are_set_and_removed_as_ops() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"one two three"}));
    ops(&mut doc, json!({"op":"protection","action":"editable_range","from":{"p":p,"o":4},"to":{"p":p,"o":7}}));
    let r = doc.protection_info().ranges;
    assert_eq!((r[0].from.o, r[0].to.o, r[0].group.as_str()), (4, 7, "everyone"));
    let xml = saved(&mut doc);
    assert!(xml.contains(r#"<w:permStart w:id="1000" w:edGrp="everyone"/>"#) && xml.contains(r#"<w:permEnd w:id="1000"/>"#), "{xml}");
    assert_eq!(text_of(&doc, p), "one two three");
    ops(&mut doc, json!({"op":"protection","action":"editable_range","from":{"p":p,"o":5},"to":{"p":p,"o":5},"remove":true}));
    assert!(doc.protection_info().ranges.is_empty());
    assert!(!saved(&mut doc).contains("permStart"));
    assert!(doc.undo());
    assert_eq!(doc.protection_info().ranges.len(), 1);
}

#[test]
fn typing_after_the_last_character_of_a_blank_goes_into_the_blank() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Name: "}));
    ops(&mut doc, json!({"op":"form_field","action":"insert","at":{"p":p,"o":6},"kind":"text"}));
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":11},"text":" end"}));
    ops(&mut doc, json!({"op":"form_field","action":"set","at":{"p":p,"o":0},"n":0,"text":"A"}));
    let v = para_json(&mut doc, 0);
    assert_eq!((v["forms"][0]["start"].as_u64(), v["forms"][0]["end"].as_u64()), (Some(6), Some(7)));
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":7},"text":"b"}));
    let v = para_json(&mut doc, 0);
    assert_eq!((v["forms"][0]["start"].as_u64(), v["forms"][0]["end"].as_u64()), (Some(6), Some(8)), "{v}");
}

#[test]
fn a_paragraph_wrapped_in_a_block_control_is_drawn_again() {
    let mut doc = Document::new_blank();
    let p = paras(&doc)[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"Line"}));
    let _ = doc.render_all();
    ops(&mut doc, json!({"op":"control","action":"insert","at":{"p":p,"o":0},"kind":"repeating"}));
    let out = doc.render_changed();
    assert_eq!(out.blocks.len(), 1);
    let v = serde_json::to_value(&out.blocks[0]).unwrap();
    assert_eq!((v["t"].as_str(), v["control"]["kind"].as_str()), (Some("c"), Some("repeating")));
    assert_eq!(v["blocks"][0]["control"]["kind"], "repeatingItem");
    // Taking the controls away draws what was inside them on its own again.
    ops(&mut doc, json!({"op":"control","action":"remove","target":{"block":v["id"].as_u64().unwrap()}}));
    let out = doc.render_changed();
    let item = serde_json::to_value(&out.blocks[0]).unwrap();
    assert_eq!(item["control"]["kind"], "repeatingItem");
    ops(&mut doc, json!({"op":"control","action":"remove","target":{"block":item["id"].as_u64().unwrap()}}));
    let out = doc.render_changed();
    assert!(out.blocks.iter().any(|b| matches!(b, docxtor_core::render::RBlock::Para(_))));
}

#[test]
fn the_assistant_can_list_fill_and_insert_and_is_held_to_protection() {
    use docxtor_core::ai::call_tool;
    let mut doc = open_with(&format!("{CHECKBOX}{DROPDOWN}"));
    let p = paras(&doc);
    let list = call_tool(&mut doc, "list_content_controls", &json!({})).unwrap();
    assert!(list.contains("checkbox \"Agree\" unchecked") && list.contains("dropDown \"Colour\" = (empty) choices: 0=Choose an item., 1=Red, 2=Blue"), "{list}");
    call_tool(&mut doc, "fill_content_control", &json!({"id": p[0], "checked": true})).unwrap();
    call_tool(&mut doc, "fill_content_control", &json!({"id": p[1], "choice": 1})).unwrap();
    assert_eq!(text_of(&doc, p[1]), "Red");
    let list = call_tool(&mut doc, "list_content_controls", &json!({})).unwrap();
    assert!(list.contains("checked") && list.contains("= \"Red\""), "{list}");
    call_tool(&mut doc, "insert_content_control", &json!({"id": p[1], "kind": "date", "alias": "Due", "date_format": "yyyy-MM-dd"})).unwrap();
    call_tool(&mut doc, "fill_content_control", &json!({"id": p[1], "n": 1, "date": "2026-10-14"})).unwrap();
    assert_eq!(text_of(&doc, p[1]), "Red2026-10-14");
    // Protection: the assistant may restrict, and then it is held to it like anyone.
    call_tool(&mut doc, "set_document_protection", &json!({"edit": "forms", "enforce": true})).unwrap();
    let info = call_tool(&mut doc, "get_document_protection", &json!({})).unwrap();
    assert!(info.contains("\"edit\": \"forms\"") && info.contains("\"enforced\": true"), "{info}");
    assert!(call_tool(&mut doc, "replace_text", &json!({"find": "Agree", "replace": "Accept"})).is_err());
    call_tool(&mut doc, "fill_content_control", &json!({"id": p[0], "checked": false})).unwrap();
    assert!(call_tool(&mut doc, "set_document_protection", &json!({"edit": "readOnly"})).is_err());
    let defs = docxtor_core::ai::tool_definitions().to_string();
    assert!(defs.contains("fill_content_control") && !defs.contains("\"password\":"));
}
