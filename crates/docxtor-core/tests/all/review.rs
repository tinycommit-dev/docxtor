//! The Review tab's engine: not checking spelling, word counts, comparing formatting, combining documents and
//! Inspect document.

use docxtor_core::compare::{combine, compare_view, redline};
use docxtor_core::model::{walk_paragraphs, Block};
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

fn from_body(body: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let s = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    pkg.set("word/document.xml", s.replacen("<w:p/>", body, 1).into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    String::from_utf8(Package::open(bytes).unwrap().get(name).unwrap().to_vec()).unwrap()
}

fn has_part(bytes: &[u8], name: &str) -> bool {
    Package::open(bytes).unwrap().has(name)
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

// ---------------------------------------------------------------------------
// Do not check spelling and grammar
// ---------------------------------------------------------------------------

#[test]
fn no_proof_is_read_set_and_undone() {
    let mut doc = from_body(r#"<w:p><w:r><w:rPr><w:noProof/></w:rPr><w:t>Teh code</w:t></w:r><w:r><w:t> and prose</w:t></w:r></w:p>"#);
    let json = serde_json::to_string(&doc.render_all()).unwrap();
    assert!(json.contains(r#""noProof":true"#), "{json}");
    // Set it on the prose.
    let p = find_p(&doc, "prose");
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":8},"to":{"p":p,"o":19},"format":{"noProof":true}}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml");
    // The prose joined the run that already had it.
    assert_eq!(xml.matches("<w:noProof/>").count(), 1, "{xml}");
    assert!(xml.contains("<w:noProof/></w:rPr><w:t>Teh code and prose</w:t>"), "{xml}");
    // And off again.
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":19},"format":{"noProof":false}}));
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert_eq!(xml.matches("<w:noProof/>").count(), 0, "{xml}");
    assert!(doc.undo());
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert_eq!(xml.matches("<w:noProof/>").count(), 1);
}

#[test]
fn no_proof_from_a_style_reaches_the_text() {
    let mut doc = from_body(r#"<w:p><w:r><w:rPr><w:rStyle w:val="Code"/></w:rPr><w:t>fn main</w:t></w:r></w:p>"#);
    // A character style that turns checking off.
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    let styles = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
    let style = r#"<w:style w:type="character" w:customStyle="1" w:styleId="Code"><w:name w:val="Code"/><w:rPr><w:noProof/></w:rPr></w:style>"#;
    pkg.set("word/styles.xml", styles.replace("</w:styles>", &format!("{style}</w:styles>")).into_bytes());
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let json = serde_json::to_string(&doc.render_all()).unwrap();
    assert!(json.contains(r#""noProof":true"#), "{json}");
}

#[test]
fn hiding_errors_in_this_document_is_a_setting() {
    let mut doc = doc_with("Some text");
    assert!(!doc.page_setup().layout.hide_spelling_errors);
    ops(&mut doc, json!({"op":"set_layout_settings","hide_spelling_errors":true,"hide_grammatical_errors":true}));
    let l = doc.page_setup().layout;
    assert!(l.hide_spelling_errors && l.hide_grammatical_errors);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/settings.xml");
    assert!(xml.contains("<w:hideSpellingErrors/>") && xml.contains("<w:hideGrammaticalErrors/>"), "{xml}");
    // In the order the schema wants: after gutterAtTop, before proofState and the rest.
    assert!(!xml.contains("<w:hideSpellingErrors/><w:hideSpellingErrors/>"));
    ops(&mut doc, json!({"op":"set_layout_settings","hide_spelling_errors":false}));
    assert!(!part(&doc.save().unwrap(), "word/settings.xml").contains("hideSpellingErrors"));
    assert!(doc.undo());
    assert!(doc.page_setup().layout.hide_spelling_errors);
    let reopened = Document::open(&bytes).unwrap();
    assert!(reopened.page_setup().layout.hide_spelling_errors);
}

// ---------------------------------------------------------------------------
// Word count
// ---------------------------------------------------------------------------

#[test]
fn text_stats_count_paragraphs_notes_and_text_boxes() {
    let mut doc = doc_with("One two three\n\nFour five\n\n| a | b |\n|---|---|\n| c d | e |");
    let main = doc.text_stats().main;
    assert_eq!(main.words, 3 + 2 + 5);
    // Two paragraphs, four cells and the empty paragraph after the table.
    assert_eq!(main.paragraphs, 2 + 4 + 1);
    assert_eq!(doc.text_stats().notes.words, 0);
    let p = find_p(&doc, "Four five");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":4},"text":"A note with six little words"}));
    let s = doc.text_stats();
    assert_eq!(s.notes.words, 6, "{s:?}");
    assert_eq!(s.notes.paragraphs, 1);
    // The note adds nothing to the main text.
    assert_eq!(s.main.words, 10);
    ops(&mut doc, json!({"op":"insert_text_box","at":{"p":p,"o":0},"x":0,"y":0,"w":100,"h":50,"text":"Box with four words"}));
    let s = doc.text_stats();
    assert_eq!(s.boxes.words, 4, "{s:?}");
    assert_eq!(s.main.words, 10);
    // Characters: spaces count in one and not in the other, and paragraph marks in neither.
    let a = doc_with("ab cd").text_stats().main;
    assert_eq!((a.chars, a.chars_no_spaces), (5, 4));
}

// ---------------------------------------------------------------------------
// Compare: formatting
// ---------------------------------------------------------------------------

#[test]
fn compare_notices_character_formatting() {
    let old = doc_with("A plain word here");
    let new = doc_with("A **plain** word here");
    let view = compare_view(&old, &new);
    assert_eq!(view.len(), 1);
    let b = &view[0];
    assert_eq!(b.kind, "formatted");
    let marked: Vec<_> = b.parts.iter().filter(|p| p.t == "fmt").collect();
    assert_eq!(marked.len(), 1, "{b:?}");
    assert_eq!(marked[0].text, "plain");
    assert_eq!(marked[0].fmt, ["bold"]);
    // The words around it are the same.
    let text: String = b.parts.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(text, "A plain word here");
    assert!(b.para_fmt.is_empty());
}

#[test]
fn compare_notices_paragraph_formatting() {
    let old = doc_with("Same words\n\nOther");
    let mut new = doc_with("Same words\n\nOther");
    let p = find_p(&new, "Same words");
    ops(&mut new, json!({"op":"format_paragraphs","ids":[p],"format":{"align":"center"}}));
    let view = compare_view(&old, &new);
    assert_eq!(view[0].kind, "formatted");
    assert_eq!(view[0].para_fmt, ["align"]);
    assert_eq!(view[1].kind, "same");
}

#[test]
fn compare_reports_formatting_next_to_changed_words() {
    let old = doc_with("The quick brown fox");
    let new = doc_with("The slow **brown** fox");
    let b = &compare_view(&old, &new)[0];
    assert_eq!(b.kind, "changed");
    assert!(b.parts.iter().any(|p| p.t == "del" && p.text == "quick"));
    assert!(b.parts.iter().any(|p| p.t == "ins" && p.text == "slow"));
    assert!(b.parts.iter().any(|p| p.t == "fmt" && p.text == "brown" && p.fmt == ["bold"]));
}

#[test]
fn the_redline_carries_formatting_as_tracked_changes() {
    let old = doc_with("A plain word here\n\nSecond one");
    let mut new = doc_with("A **plain** word here\n\nSecond one");
    let p = find_p(&new, "Second");
    ops(&mut new, json!({"op":"format_paragraphs","ids":[p],"format":{"align":"right"}}));
    let bytes = redline(&old, &new, "Tester", "2026-10-12T09:30:00Z").unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml");
    // The bold run says what it was before: no formatting.
    assert!(xml.contains(r#"<w:b/><w:bCs/><w:rPrChange w:id=""#), "{xml}");
    assert!(xml.contains(r#"w:author="Tester" w:date="2026-10-12T09:30:00Z"><w:rPr/></w:rPrChange>"#), "{xml}");
    // The right-aligned paragraph says what its properties were.
    assert!(xml.contains("<w:pPrChange "), "{xml}");
    let change = xml.split("<w:pPrChange ").nth(1).unwrap().split("</w:pPrChange>").next().unwrap();
    assert!(change.contains("<w:pPr") && !change.contains("w:jc"), "{change}");
    // The words are untouched: no insertions or deletions.
    assert!(!xml.contains("<w:ins ") && !xml.contains("<w:del "));
    // The unique revision ids.
    let ids: Vec<&str> = xml.split("w:id=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "{ids:?}");
    // Opened again, the text is the new one's.
    assert_eq!(texts(&Document::open(&bytes).unwrap()), ["A plain word here", "Second one"]);
}

#[test]
fn identical_formatting_is_not_a_change() {
    let old = doc_with("A **bold** word and *italic* too");
    let new = doc_with("A **bold** word and *italic* too");
    let b = &compare_view(&old, &new)[0];
    assert_eq!(b.kind, "same");
    let xml = part(&redline(&old, &new, "T", "2026-10-12T09:30:00Z").unwrap(), "word/document.xml");
    assert!(!xml.contains("PrChange"));
}

// ---------------------------------------------------------------------------
// Combine
// ---------------------------------------------------------------------------

const BOB: &str = r#"w:author="Bob" w:date="2026-10-01T10:00:00Z""#;

#[test]
fn combine_keeps_the_revised_documents_own_tracked_changes() {
    let original = from_body(r#"<w:p><w:r><w:t>The first line.</w:t></w:r></w:p><w:p><w:r><w:t>The second line stays.</w:t></w:r></w:p><w:p><w:r><w:t>A third line.</w:t></w:r></w:p>"#);
    let revised = from_body(&format!(
        r#"<w:p><w:r><w:t xml:space="preserve">The first </w:t></w:r><w:ins w:id="7" {BOB}><w:r><w:t xml:space="preserve">new </w:t></w:r></w:ins><w:r><w:t>line.</w:t></w:r></w:p><w:p><w:r><w:t>The second line stays.</w:t></w:r></w:p><w:p><w:r><w:t>A fourth line.</w:t></w:r></w:p>"#
    ));
    let bytes = combine(&original, &revised, "Me", "2026-10-12T09:30:00Z").unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml");
    // Bob's insertion is still Bob's, and nothing wraps the first paragraph again.
    assert!(xml.contains(&format!(r#"<w:ins w:id="7" {BOB}>"#)), "{xml}");
    let first = xml.split("</w:p>").next().unwrap();
    assert!(!first.contains("\"Me\""), "{first}");
    // The untracked edit in the last paragraph is marked as mine.
    assert!(xml.contains(r#"<w:del "#) && xml.contains(r#"w:author="Me""#), "{xml}");
    assert!(xml.contains(r#"<w:delText xml:space="preserve">third</w:delText>"#) && xml.contains("<w:t>fourth</w:t>"), "{xml}");
    // Revision ids stay unique.
    let ids: Vec<&str> = xml.split("w:id=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "{ids:?}");
}

#[test]
fn combine_leaves_a_paragraph_bob_added_alone() {
    let original = from_body(r#"<w:p><w:r><w:t>Keep me.</w:t></w:r></w:p>"#);
    let revised = from_body(&format!(
        r#"<w:p><w:r><w:t>Keep me.</w:t></w:r></w:p><w:p><w:pPr><w:rPr><w:ins w:id="9" {BOB}/></w:rPr></w:pPr><w:ins w:id="10" {BOB}><w:r><w:t>Bob wrote this.</w:t></w:r></w:ins></w:p>"#
    ));
    let bytes = combine(&original, &revised, "Me", "2026-10-12T09:30:00Z").unwrap();
    let xml = part(&bytes, "word/document.xml");
    assert!(!xml.contains("\"Me\""), "{xml}");
    assert_eq!(xml.matches("Bob wrote this.").count(), 1);
}

// ---------------------------------------------------------------------------
// Inspect document
// ---------------------------------------------------------------------------

/// A document with a comment, a tracked change, hidden text, a header, custom XML, custom properties and a preview picture.
fn inspected() -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let body = format!(
        r#"<w:p><w:r><w:t xml:space="preserve">Visible </w:t></w:r><w:commentRangeStart w:id="0"/><w:r><w:t>commented</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="0"/></w:r><w:ins w:id="5" {BOB}><w:r><w:t xml:space="preserve"> inserted</w:t></w:r></w:ins><w:del w:id="6" {BOB}><w:r><w:delText xml:space="preserve"> removed</w:delText></w:r></w:del><w:r><w:rPr><w:vanish/></w:rPr><w:t xml:space="preserve"> secret</w:t></w:r></w:p>"#
    );
    let s = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    pkg.set("word/document.xml", s.replacen("<w:p/>", &body, 1).into_bytes());
    let w = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    pkg.set("word/comments.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="{w}"><w:comment w:id="0" w:author="Carol" w:initials="C" w:date="2026-10-02T10:00:00Z"><w:p><w:r><w:t>Check this</w:t></w:r></w:p></w:comment></w:comments>"#).into_bytes());
    pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments", "comments.xml", false);
    pkg.ensure_override("word/comments.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml");
    pkg.set("customXml/item2.xml", br#"<?xml version="1.0" encoding="UTF-8"?><data xmlns="urn:acme"><secret>1</secret></data>"#.to_vec());
    pkg.set("customXml/itemProps2.xml", br#"<?xml version="1.0" encoding="UTF-8"?><ds:datastoreItem xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml" ds:itemID="{11111111-2222-3333-4444-555555555555}"/>"#.to_vec());
    pkg.add_rel("customXml/item2.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXmlProps", "itemProps2.xml", false);
    pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml", "../customXml/item2.xml", false);
    pkg.set("docProps/app.xml", br#"<?xml version="1.0" encoding="UTF-8"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Word</Application><Manager>Dana</Manager><Company>Acme Corp</Company></Properties>"#.to_vec());
    pkg.add_rel("", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties", "docProps/app.xml", false);
    pkg.set("docProps/custom.xml", br#"<?xml version="1.0" encoding="UTF-8"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties"><property fmtid="{D5CDD505-2E9C-101B-9397-08002B2CF9AE}" pid="2" name="Project"><lpwstr xmlns="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">Falcon</lpwstr></property></Properties>"#.to_vec());
    pkg.add_rel("", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties", "docProps/custom.xml", false);
    pkg.set("docProps/thumbnail.jpeg", vec![0xff, 0xd8, 0xff, 0xd9]);
    pkg.add_rel("", "http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail", "docProps/thumbnail.jpeg", false);
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    ops(&mut doc, json!({"op":"set_doc_props","author":"Erin Example"}));
    ops(&mut doc, json!({"op":"add_header_footer","kind":"header","variant":"default"}));
    let h = {
        let mut id = None;
        for s in &doc.stories {
            if let Some(Block::Para(p)) = s.blocks.first() {
                id = Some(p.id);
            }
        }
        id.unwrap()
    };
    ops(&mut doc, json!({"op":"set_paragraph_text","id":h,"markdown":"Confidential draft"}));
    doc
}

fn found(doc: &Document, id: &str) -> docxtor_core::document::Finding {
    doc.inspect().into_iter().find(|f| f.id == id).unwrap_or_else(|| panic!("no finding {id}"))
}

#[test]
fn inspect_lists_what_the_file_carries() {
    let doc = inspected();
    let f = doc.inspect();
    assert_eq!(f.iter().map(|f| f.id).collect::<Vec<_>>(), docxtor_core::document::CATEGORIES);
    let props = found(&doc, "properties");
    assert!(props.detail.iter().any(|d| d == "author: Erin Example"), "{props:?}");
    assert!(props.detail.iter().any(|d| d == "company: Acme Corp") && props.detail.iter().any(|d| d == "manager: Dana"));
    assert!(props.detail.iter().any(|d| d == "customProperties: 1") && props.detail.iter().any(|d| d == "thumbnail"));
    assert_eq!(found(&doc, "authors").detail, ["Bob", "Carol"]);
    assert_eq!(found(&doc, "comments").count, 1);
    assert_eq!(found(&doc, "revisions").count, 2);
    assert_eq!(found(&doc, "hiddenText").count, 1);
    assert_eq!(found(&doc, "headersFooters").count, 1);
    assert_eq!(found(&doc, "customXml").count, 1);
    assert!(!found(&doc, "embedded").removable);
}

#[test]
fn a_clean_document_has_nothing_to_remove() {
    let mut doc = doc_with("Just text");
    ops(&mut doc, json!({"op":"set_doc_props","author":""}));
    for f in doc.inspect() {
        assert_eq!(f.count, 0, "{f:?}");
    }
}

#[test]
fn remove_all_takes_everything_out_and_undo_brings_it_back() {
    let mut doc = inspected();
    let before = doc.save().unwrap();
    assert!(has_part(&before, "word/comments.xml") && has_part(&before, "customXml/item2.xml") && has_part(&before, "docProps/thumbnail.jpeg"));
    let all: Vec<&str> = docxtor_core::document::CATEGORIES.iter().copied().filter(|c| *c != "embedded").collect();
    ops(&mut doc, json!({"op":"remove_hidden_data","what":all}));
    for f in doc.inspect() {
        assert_eq!(f.count, 0, "{f:?}");
    }
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    // The body: comments gone, insertion accepted, deletion and hidden text gone.
    let xml = part(&bytes, "word/document.xml");
    for gone in ["commentRange", "commentReference", "w:ins", "w:del", "vanish", "removed", "secret", "Bob"] {
        assert!(!xml.contains(gone), "{gone} in {xml}");
    }
    assert!(xml.contains("Visible ") && xml.contains("commented") && xml.contains(" inserted"), "{xml}");
    // The parts and what points at them.
    for name in ["word/comments.xml", "customXml/item2.xml", "customXml/itemProps2.xml", "customXml/_rels/item2.xml.rels", "docProps/thumbnail.jpeg", "docProps/custom.xml"] {
        assert!(!has_part(&bytes, name), "{name}");
    }
    let rels = part(&bytes, "word/_rels/document.xml.rels");
    assert!(!rels.contains("comments.xml") && !rels.contains("item2.xml") && !rels.contains("header"), "{rels}");
    let root = part(&bytes, "_rels/.rels");
    assert!(!root.contains("thumbnail") && !root.contains("custom.xml") && root.contains("app.xml"), "{root}");
    let ct = part(&bytes, "[Content_Types].xml");
    assert!(!ct.contains("comments.xml") && !ct.contains("header1.xml"), "{ct}");
    // Properties: the names are blank, the application stays.
    let core = part(&bytes, "docProps/core.xml");
    assert!(!core.contains("Erin"), "{core}");
    let app = part(&bytes, "docProps/app.xml");
    assert!(!app.contains("Acme") && !app.contains("Dana") && app.contains("<Application>Word</Application>"), "{app}");
    assert!(part(&bytes, "word/settings.xml").contains("<w:removePersonalInformation/>"));
    // Reopened, the file is clean and still opens.
    let again = Document::open(&bytes).unwrap();
    assert_eq!(texts(&again), ["Visible commented inserted"]);
    for f in again.inspect() {
        assert_eq!(f.count, 0, "{f:?}");
    }
    // One undo puts everything back.
    assert!(doc.undo());
    let back = doc.save().unwrap();
    assert!(has_part(&back, "word/comments.xml") && has_part(&back, "customXml/item2.xml") && has_part(&back, "docProps/thumbnail.jpeg"));
    assert!(part(&back, "word/document.xml").contains("secret"));
    assert!(part(&back, "docProps/app.xml").contains("Acme Corp"));
    assert!(part(&back, "docProps/core.xml").contains("Erin Example"));
    assert_eq!(found(&doc, "comments").count, 1);
}

#[test]
fn removed_personal_information_stays_out_of_later_saves() {
    let mut doc = inspected();
    doc.set_save_info(docxtor_core::document::SaveInfo { user: Some("Frank Saver".into()), pages: Some(3), lines: None });
    ops(&mut doc, json!({"op":"remove_hidden_data","what":["properties"]}));
    // The name of whoever saves is not written back, but the statistics are.
    let bytes = doc.save_stamped("2026-10-14T09:30:00Z").unwrap();
    assert_valid(&bytes);
    let core = part(&bytes, "docProps/core.xml");
    assert!(!core.contains("Frank") && !core.contains("Erin") && core.contains("2026-10-14T09:30:00Z"), "{core}");
    let app = part(&bytes, "docProps/app.xml");
    assert!(!app.contains("Acme") && app.contains("<Pages>3</Pages>"), "{app}");
    // Properties typed in afterwards are kept: Inspect does not hold a stale copy of the part.
    ops(&mut doc, json!({"op":"set_doc_props","company":"Beta"}));
    let bytes = doc.save_stamped("2026-10-14T09:31:00Z").unwrap();
    let app = part(&bytes, "docProps/app.xml");
    assert!(app.contains("Beta") && !app.contains("Acme"), "{app}");
    assert!(found(&doc, "properties").detail.iter().any(|d| d == "company: Beta"));
    // Undo of the later edit and of the removal brings Acme back.
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(part(&doc.save().unwrap(), "docProps/app.xml").contains("Acme Corp"));
}

#[test]
fn a_removed_header_is_not_counted_any_more() {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains("<w:sectPr>") && xml.contains("xmlns:r="), "{xml}");
    let rid = pkg.add_rel("word/document.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header", "header1.xml", false);
    pkg.set("word/document.xml", xml.replacen("<w:sectPr>", &format!(r#"<w:sectPr><w:headerReference w:type="default" r:id="{rid}"/>"#), 1).into_bytes());
    let w = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    pkg.set("word/header1.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr xmlns:w="{w}"><w:p><w:ins w:id="9" {BOB}><w:r><w:t>Draft</w:t></w:r></w:ins><w:r><w:rPr><w:vanish/></w:rPr><w:t>hidden</w:t></w:r></w:p></w:hdr>"#).into_bytes());
    pkg.ensure_override("word/header1.xml", "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml");
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert_eq!((found(&doc, "revisions").count, found(&doc, "hiddenText").count, found(&doc, "headersFooters").count), (1, 1, 1));
    assert_eq!(found(&doc, "authors").detail, ["Bob"]);
    ops(&mut doc, json!({"op":"remove_hidden_data","what":["headersFooters"]}));
    for id in ["revisions", "hiddenText", "headersFooters", "authors"] {
        assert_eq!(found(&doc, id).count, 0, "{id}");
    }
    assert!(!has_part(&doc.save().unwrap(), "word/header1.xml"));
    // Undo brings the header and what it held back.
    assert!(doc.undo());
    assert_eq!((found(&doc, "revisions").count, found(&doc, "hiddenText").count), (1, 1));
}

#[test]
fn removing_the_authors_renames_them_everywhere() {
    let mut doc = inspected();
    ops(&mut doc, json!({"op":"remove_hidden_data","what":["authors"]}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let xml = part(&bytes, "word/document.xml");
    assert!(!xml.contains("Bob") && xml.contains(r#"w:author="Author""#), "{xml}");
    let comments = part(&bytes, "word/comments.xml");
    assert!(!comments.contains("Carol") && comments.contains(r#"w:author="Author""#) && comments.contains(r#"w:initials="A""#), "{comments}");
    // The words stay, and so do the tracked changes themselves.
    assert!(xml.contains("<w:ins ") && xml.contains("<w:del "));
    assert!(found(&doc, "authors").detail.is_empty());
}

#[test]
fn accepting_tracked_changes_joins_paragraphs_whose_mark_was_deleted() {
    let mut doc = from_body(&format!(
        r#"<w:p><w:r><w:t xml:space="preserve">First half, </w:t></w:r><w:pPr><w:rPr><w:del w:id="1" {BOB}/></w:rPr></w:pPr></w:p><w:p><w:r><w:t>second half.</w:t></w:r></w:p><w:p><w:pPr><w:rPr><w:ins w:id="2" {BOB}/></w:rPr></w:pPr><w:r><w:t>Last.</w:t></w:r></w:p>"#
    ));
    ops(&mut doc, json!({"op":"remove_hidden_data","what":["revisions"]}));
    assert_eq!(texts(&doc), ["First half, second half.", "Last."]);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert!(!part(&bytes, "word/document.xml").contains("w:author"));
}

#[test]
fn unknown_and_unremovable_categories_are_refused() {
    let mut doc = inspected();
    let e = doc.apply_json(&json!({"op":"remove_hidden_data","what":["nothing"]}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("not something to remove"), "{e}");
    let e = doc.apply_json(&json!({"op":"remove_hidden_data","what":["embedded"]}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("embedded"), "{e}");
    // Nothing was removed by the failed ops.
    assert_eq!(found(&doc, "comments").count, 1);
}

// ---------------------------------------------------------------------------
// AI tools
// ---------------------------------------------------------------------------

#[test]
fn the_ai_tools_count_inspect_and_remove() {
    use docxtor_core::ai::call_tool;
    let mut doc = inspected();
    let counted = call_tool(&mut doc, "count_words", &json!({})).unwrap();
    assert!(counted.contains("paragraphs") && counted.contains("words"), "{counted}");
    let with = call_tool(&mut doc, "count_words", &json!({"include_extras": true})).unwrap();
    assert!(with.contains("with notes and text boxes"), "{with}");
    let report = call_tool(&mut doc, "inspect_document", &json!({})).unwrap();
    assert!(report.contains("(comments): 1 found"), "{report}");
    assert!(report.contains("author: Erin Example"), "{report}");
    assert!(report.contains("(embedded): none found"), "{report}");
    let removed = call_tool(&mut doc, "remove_hidden_data", &json!({"what": ["comments", "hiddenText"]})).unwrap();
    assert!(removed.contains("comments, hiddenText"), "{removed}");
    let again = call_tool(&mut doc, "inspect_document", &json!({})).unwrap();
    assert!(again.contains("(comments): none found") && again.contains("(hiddenText): none found"), "{again}");
    assert!(call_tool(&mut doc, "remove_hidden_data", &json!({"what": []})).is_err());
    assert!(call_tool(&mut doc, "remove_hidden_data", &json!({"what": ["bogus"]})).is_err());
    let tools = docxtor_core::ai::tool_definitions();
    for name in ["count_words", "inspect_document", "remove_hidden_data"] {
        assert!(tools.as_array().unwrap().iter().any(|t| t["name"] == name), "{name}");
    }
    // The run property is in the format tool's schema.
    assert!(tools.to_string().contains("noProof"));
}

#[test]
fn embedded_objects_are_listed_and_cannot_be_removed() {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    pkg.set("word/embeddings/oleObject1.bin", vec![1, 2, 3]);
    pkg.set("word/vbaProject.bin", vec![4, 5]);
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let f = found(&doc, "embedded");
    assert_eq!(f.count, 2);
    assert!(!f.removable);
    assert!(f.detail.iter().any(|d| d.ends_with("oleObject1.bin")));
    let e = doc.apply_json(&json!({"op":"remove_hidden_data","what":["embedded"]}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("embedded"), "{e}");
    // They stay in the saved file.
    assert!(has_part(&doc.save().unwrap(), "word/embeddings/oleObject1.bin"));
}

#[test]
fn a_bibliography_sources_part_is_not_custom_data() {
    let mut doc = from_body(r#"<w:p><w:r><w:t>Text</w:t></w:r></w:p>"#);
    ops(&mut doc, json!({"op":"add_source","source":{"type":"book","authors":["Lee, K."],"title":"A study","year":"2020"}}));
    let bytes = doc.save().unwrap();
    assert!(has_part(&bytes, "customXml/item1.xml"));
    let mut doc = Document::open(&bytes).unwrap();
    assert_eq!(found(&doc, "customXml").count, 0, "the list of sources isn't reported");
    ops(&mut doc, json!({"op":"remove_hidden_data","what":["customXml"]}));
    assert!(has_part(&doc.save().unwrap(), "customXml/item1.xml"), "and it is not removed");
}

#[test]
fn removing_in_a_table_cell_and_undoing() {
    let mut doc = from_body(&format!(
        r#"<w:p><w:r><w:t>Intro</w:t></w:r></w:p><w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t xml:space="preserve">Cell </w:t></w:r><w:del w:id="3" {BOB}><w:r><w:delText>gone</w:delText></w:r></w:del><w:r><w:rPr><w:vanish/></w:rPr><w:t>hidden</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#
    ));
    assert_eq!(found(&doc, "revisions").count, 1);
    assert_eq!(found(&doc, "hiddenText").count, 1);
    ops(&mut doc, json!({"op":"remove_hidden_data","what":["revisions","hiddenText"]}));
    assert!(texts(&doc).iter().any(|t| t == "Cell "));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert!(doc.undo());
    assert_eq!(found(&doc, "revisions").count, 1);
}
