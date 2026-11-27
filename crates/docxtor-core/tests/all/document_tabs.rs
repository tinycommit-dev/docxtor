//! Document tabs, as in Google Docs: added, renamed, given an emoji, nested, moved, duplicated and deleted, each an undo
//! step; kept in a part of Docxtor's own that Word ignores, with every tab after the first starting a new section so Word
//! shows them one after another; and a document without tabs gets nothing added.

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

fn ops(doc: &mut Document, v: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&v.to_string(), false).unwrap_or_else(|e| panic!("{e}: {v}"))
}

fn doc_with(text: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":text}));
    doc
}

fn names(doc: &Document) -> Vec<String> {
    doc.tabs_info().tabs.iter().map(|t| t.name.clone()).collect()
}

fn id_of(doc: &Document, name: &str) -> String {
    doc.tabs_info().tabs.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("no tab {name}")).id.clone()
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

/// Type into the first paragraph of a tab.
fn type_in(doc: &mut Document, name: &str, text: &str) {
    let info = doc.tabs_info();
    let start = info.tabs.iter().find(|t| t.name == name).unwrap().start;
    ops(doc, json!({"op":"insert_text","at":{"p":start,"o":0},"text":text}));
}

#[test]
fn a_document_without_tabs_is_left_as_it_is() {
    let mut doc = doc_with("Plain text");
    assert!(doc.tabs_info().tabs.is_empty());
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    assert!(!pkg.parts.iter().any(|p| p.name.starts_with("customXml/")), "no tabs part");
    assert!(!String::from_utf8_lossy(pkg.get("word/document.xml").unwrap()).contains("_DocxtorTab_"));
}

#[test]
fn adding_a_tab_makes_the_text_the_first_tab_and_starts_a_section() {
    let mut doc = doc_with("Overview text");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Details","first_name":"Overview"}));
    assert_eq!(names(&doc), vec!["Overview", "Details"]);
    type_in(&mut doc, "Details", "Details text");
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = String::from_utf8(Package::open(&bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains("_DocxtorTab_"), "the second tab is marked");
    assert_eq!(xml.matches("<w:sectPr").count(), 2, "the second tab starts a section of its own");
    // Opened again, the tabs are still there.
    let again = Document::open(&bytes).unwrap();
    assert_eq!(names(&again), vec!["Overview", "Details"]);
    assert_eq!(texts(&again), vec!["Overview text".to_string(), "Details text".to_string()]);
}

#[test]
fn rename_emoji_nest_move_duplicate_and_delete_each_undo() {
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Three"}));
    let two = id_of(&doc, "Two");
    let one = id_of(&doc, "One");
    ops(&mut doc, json!({"op":"tab","action":"rename","id":two,"name":"Second"}));
    ops(&mut doc, json!({"op":"tab","action":"set_icon","id":two,"icon":"📄"}));
    assert_eq!(doc.tabs_info().tabs[1].icon.as_deref(), Some("📄"));
    // Nest Three in One.
    let three = id_of(&doc, "Three");
    ops(&mut doc, json!({"op":"tab","action":"move","id":three,"parent":one}));
    let info = doc.tabs_info();
    assert_eq!(info.tabs.iter().map(|t| (t.name.as_str(), t.depth)).collect::<Vec<_>>(), vec![("One", 0), ("Three", 1), ("Second", 0)]);
    ops(&mut doc, json!({"op":"tab","action":"duplicate","id":two}));
    assert_eq!(names(&doc).len(), 4);
    ops(&mut doc, json!({"op":"tab","action":"delete","id":two}));
    assert!(!names(&doc).contains(&"Second".to_string()));
    assert_valid(&doc.save().unwrap());
    // Undo the five changes, one step each: delete, duplicate, move, emoji, rename.
    for _ in 0..5 {
        assert!(doc.undo());
    }
    assert_eq!(names(&doc), vec!["One", "Two", "Three"]);
}

#[test]
fn no_op_opens_a_tab_and_saving_writes_the_one_the_editor_names() {
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    let (one, two) = (id_of(&doc, "One"), id_of(&doc, "Two"));
    // Which tab is open is the pane's: adding, duplicating and deleting tabs leave the engine's hint where it was.
    assert_eq!(doc.tabs_info().active.as_deref(), Some(one.as_str()));
    ops(&mut doc, json!({"op":"tab","action":"duplicate","id":two}));
    assert_eq!(doc.tabs_info().active.as_deref(), Some(one.as_str()));
    // There is no `open` action any more.
    assert!(doc.apply_json(&json!({"op":"tab","action":"open","id":one}).to_string(), false).is_err());
    // The editor names the tab it shows; that is not an edit, and the file keeps it.
    let steps = doc.undo_labels().len();
    doc.set_open_tab(&two).unwrap();
    assert_eq!(doc.undo_labels().len(), steps, "no undo step");
    assert!(doc.set_open_tab("nope").is_err());
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let again = Document::open(&bytes).unwrap();
    assert_eq!(again.tabs_info().active.as_deref(), Some(two.as_str()), "the file round-trips the open tab");
}

#[test]
fn deleting_the_text_between_tabs_keeps_each_tab_its_start() {
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    type_in(&mut doc, "Two", "Second text");
    let first = doc.tabs_info().tabs[0].start;
    let second = doc.tabs_info().tabs[1].start;
    // A delete across the boundary is refused or keeps both tabs: the tab list must stay whole.
    let r = doc.apply_json(&json!({"op":"delete","from":{"p":first,"o":1},"to":{"p":second,"o":3}}).to_string(), false);
    if r.is_ok() {
        assert_eq!(doc.tabs_info().tabs.len(), 2);
    }
    assert_valid(&doc.save().unwrap());
}

/// The three things Word needs to keep the tabs part: a relationship from the document, its properties part and the
/// properties part's relationship and content type.
fn tabs_part_is_wired(bytes: &[u8]) -> bool {
    let pkg = Package::open(bytes).unwrap();
    let text = |n: &str| pkg.get(n).map(|d| String::from_utf8_lossy(d).into_owned()).unwrap_or_default();
    let rels = text("word/_rels/document.xml.rels");
    let item = text("customXml/item1.xml");
    item.contains("tabs/1")
        && rels.contains("customXml/item1.xml")
        && text("customXml/_rels/item1.xml.rels").contains("itemProps1.xml")
        && pkg.get("customXml/itemProps1.xml").is_some()
        && text("[Content_Types].xml").contains("/customXml/itemProps1.xml")
}

#[test]
fn deleting_a_tab_saving_undoing_and_saving_again_keeps_the_tabs_part_wired() {
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    assert!(tabs_part_is_wired(&doc.save().unwrap()));
    let two = id_of(&doc, "Two");
    ops(&mut doc, json!({"op":"tab","action":"delete","id":two}));
    let gone = doc.save().unwrap();
    assert!(!Package::open(&gone).unwrap().parts.iter().any(|p| p.name.starts_with("customXml/")), "no tabs, no part");
    assert!(doc.undo());
    let back = doc.save().unwrap();
    assert_valid(&back);
    assert!(tabs_part_is_wired(&back), "the part is related again after undo");
    assert_eq!(names(&Document::open(&back).unwrap()), vec!["One", "Two"]);
}

#[test]
fn adding_a_tab_saving_undoing_saving_and_redoing_keeps_the_tabs_part_wired() {
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    assert!(tabs_part_is_wired(&doc.save().unwrap()));
    assert!(doc.undo());
    assert!(!Package::open(&doc.save().unwrap()).unwrap().parts.iter().any(|p| p.name.starts_with("customXml/")));
    assert!(doc.redo());
    let again = doc.save().unwrap();
    assert_valid(&again);
    assert!(tabs_part_is_wired(&again), "the part is related again after redo");
}

/// A document whose body ends with a bookmark end between the last paragraph and the closing section properties, with
/// a title page set (as Word writes these).
fn doc_with_trailing_bookmark_end() -> Document {
    let mut blank = Document::new_blank();
    let mut pkg = Package::open(&blank.save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:body>", r#"<w:body><w:p><w:bookmarkStart w:id="3" w:name="Closing"/><w:r><w:t>Last paragraph</w:t></w:r></w:p><w:bookmarkEnd w:id="3"/>"#, 1).replacen("<w:p/><w:sectPr>", "<w:sectPr><w:titlePg/>", 1);
    let xml = if xml.contains("<w:titlePg/>") { xml } else { xml.replacen("<w:sectPr>", "<w:sectPr><w:titlePg/>", 1) };
    pkg.set("word/document.xml", xml.into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

#[test]
fn a_bookmark_end_after_a_tabs_last_paragraph_keeps_the_guard_and_the_page_setup() {
    let mut doc = doc_with_trailing_bookmark_end();
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    type_in(&mut doc, "Two", "Second text");
    let (first, second) = (doc.tabs_info().tabs[0].start, doc.tabs_info().tabs[1].start);
    // Joining the two tabs' paragraphs is refused (the guard is still watching), and nothing changes.
    let before = (texts(&doc), names(&doc));
    assert!(doc.apply_json(&json!({"op":"delete","from":{"p":first,"o":3},"to":{"p":second,"o":2}}).to_string(), false).is_err(), "joining tabs is refused");
    assert_eq!((texts(&doc), names(&doc)), before);
    // A third tab changes nothing about the first tab's section.
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Three"}));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let xml = String::from_utf8(Package::open(&saved).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    let first_break = xml.split("</w:sectPr>").next().unwrap();
    assert!(first_break.contains("<w:titlePg/>"), "the first tab keeps its title page: {xml}");
    assert_eq!(xml.matches("<w:sectPr").count(), 3, "one section per tab");
}

#[test]
fn each_tab_lists_the_blocks_the_page_draws_for_it() {
    // A bookmark end between paragraphs of the first tab: a body block the page does not draw.
    let mut doc = doc_with_trailing_bookmark_end();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"Intro "}));
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    type_in(&mut doc, "Two", "Second");
    let order = doc.render_all().order;
    let info = doc.tabs_info();
    // Every drawn block is in exactly one tab, in the order the page has them, and the bookmark end is in none.
    let all: Vec<u32> = info.tabs.iter().flat_map(|t| t.ids.iter().copied()).collect();
    assert_eq!(all, order, "the tabs' ids are the page's order, split");
    assert!(info.tabs[0].blocks > info.tabs[0].ids.len(), "the body counts the invisible bookmark end, the ids don't");
    assert_eq!(info.tabs[1].ids.len(), 1);
}

#[test]
fn an_imported_chunk_in_a_tab_is_listed_as_its_blocks() {
    use docxtor_core::package::Package as P;
    let mut blank = Document::new_blank();
    let mut pkg = P::open(&blank.save().unwrap()).unwrap();
    let rid = pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/aFChunk", "afchunk.htm", false);
    pkg.set("word/afchunk.htm", b"<p>One</p><p>Two</p><p>Three</p>".to_vec());
    pkg.ensure_override("word/afchunk.htm", "text/html");
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replacen("<w:body>", &format!(r#"<w:body><w:p><w:r><w:t>Start</w:t></w:r></w:p><w:altChunk r:id="{rid}"/>"#), 1);
    let xml = if xml.contains("xmlns:r=") { xml } else { xml.replacen("<w:document ", r#"<w:document xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#, 1) };
    pkg.set("word/document.xml", xml.into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    let order = doc.render_all().order;
    let info = doc.tabs_info();
    let all: Vec<u32> = info.tabs.iter().flat_map(|t| t.ids.iter().copied()).collect();
    assert_eq!(all, order);
    assert!(info.tabs[0].ids.len() >= 4, "Start and the chunk's three paragraphs: {:?}", info.tabs[0].ids);
}

#[test]
fn duplicating_a_tab_with_a_long_name_works_and_keeps_the_name_within_the_limit() {
    let mut doc = doc_with("One");
    let long = "N".repeat(100);
    ops(&mut doc, json!({"op":"tab","action":"add","name":long,"first_name":"One"}));
    let id = id_of(&doc, &long);
    ops(&mut doc, json!({"op":"tab","action":"duplicate","id":id}));
    let copy = names(&doc).into_iter().find(|n| n.starts_with("Copy of")).expect("a copy");
    assert_eq!(copy.chars().count(), 100);
    // A name chosen by the caller beyond the limit is still refused, with a message that says why.
    assert!(doc.apply_json(&json!({"op":"tab","action":"duplicate","id":id,"name":"M".repeat(101)}).to_string(), false).unwrap_err().to_string().contains("100 characters"));
    // And what is opened again has names within the limit.
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert!(names(&again).iter().all(|n| n.chars().count() <= 100));
}

#[test]
fn the_assistants_outline_lists_the_tabs_with_their_counts_and_the_headings_under_each() {
    let mut doc = doc_with("Intro text here");
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"set_style","ids":[first],"style":"Heading1"}));
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Details","first_name":"Overview"}));
    let start = doc.tabs_info().tabs[1].start;
    ops(&mut doc, json!({"op":"insert_text","at":{"p":start,"o":0},"text":"Two words"}));
    ops(&mut doc, json!({"op":"set_style","ids":[start],"style":"Heading2"}));
    let outline = docxtor_core::ai::call_tool(&mut doc, "get_outline", &json!({})).unwrap();
    let lines: Vec<&str> = outline.lines().collect();
    let (o, d) = (lines.iter().position(|l| l.contains("Overview")).unwrap(), lines.iter().position(|l| l.contains("[tab") && l.contains("Details")).unwrap());
    assert!(lines[o].starts_with("[tab t1]") && lines[o].contains("3 words"), "{outline}");
    assert!(lines[d].contains("2 words"), "{outline}");
    // Each heading comes under its own tab.
    let h1 = lines.iter().position(|l| l.contains("Intro text here")).unwrap();
    let h2 = lines.iter().position(|l| l.contains("Two words") && l.contains("[p")).unwrap();
    assert!(o < h1 && h1 < d && d < h2, "{outline}");
    // A document without tabs is unchanged.
    let mut plain = doc_with("Only text");
    assert!(!docxtor_core::ai::call_tool(&mut plain, "get_outline", &json!({})).unwrap().contains("[tab"));
}

#[test]
fn deleting_the_first_block_of_a_tab_keeps_the_tab_and_starts_it_at_the_next_block() {
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    type_in(&mut doc, "Two", "First of two");
    let start = doc.tabs_info().tabs[1].start;
    ops(&mut doc, json!({"op":"insert_paragraph","after":start,"text":"Second of two"}));
    // The first block of the tab goes (it holds the tab's marker); the tab stays and starts at what was its second.
    ops(&mut doc, json!({"op":"delete_blocks","ids":[start]}));
    assert_eq!(names(&doc), vec!["One", "Two"]);
    let info = doc.tabs_info();
    assert_eq!(texts(&doc).iter().filter(|t| t.contains("of two")).count(), 1);
    let second = info.tabs[1].ids[0];
    assert!(doc.find_any_para(second).unwrap().plain_text().contains("Second of two"));
    assert_valid(&doc.save().unwrap());
    // A tab's text and its break all going at once is still refused, with nothing changed.
    let before = texts(&doc);
    let all = doc.tabs_info().tabs[1].ids.clone();
    let first = doc.tabs_info().tabs[0].ids[0];
    let err = doc.apply_json(&json!({"op":"delete_blocks","ids":[first, all[0]]}).to_string(), false);
    if err.is_err() {
        assert_eq!(texts(&doc), before);
    }
}

#[test]
fn the_open_tab_survives_the_tabs_going_and_coming_back_and_follows_the_text_order() {
    // Found by the random search. Deleting a tab that leaves one takes the tabs away, and that forgot the tab the
    // file was saved on: undo brought the tabs back with another one open.
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    let (one, two) = (id_of(&doc, "One"), id_of(&doc, "Two"));
    doc.set_open_tab(&two).unwrap();
    let mut doc = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(doc.tabs_info().active.as_deref(), Some(two.as_str()));
    ops(&mut doc, json!({"op":"tab","action":"delete","id":one}));
    assert!(doc.undo());
    assert_eq!(doc.tabs_info().active.as_deref(), Some(two.as_str()), "undo opens the tab the file was saved on again");
    // With no tab asked for, the open one is the first in the text, before a save and after it, whatever order the list keeps.
    let mut doc = doc_with("One");
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Two","first_name":"One"}));
    ops(&mut doc, json!({"op":"tab","action":"add","name":"Three"}));
    let three = id_of(&doc, "Three");
    let one = id_of(&doc, "One");
    ops(&mut doc, json!({"op":"tab","action":"move","id":one,"parent":three}));
    let first_in_text = doc.tabs_info().tabs[0].id.clone();
    assert_eq!(doc.tabs_info().active.as_deref(), Some(first_in_text.as_str()));
    let again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(again.tabs_info().active, doc.tabs_info().active, "a reopened file opens the same tab");
}
