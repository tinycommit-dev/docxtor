//! Tests of: Bookmarks.

use super::*;

#[test]
fn bookmarks_on_a_range_and_at_a_point() {
    let mut doc = doc_with("Alpha beta gamma\n\nSecond paragraph here");
    let a = find_p(&doc, "Alpha");
    let b = find_p(&doc, "Second");
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":a,"o":10},"name":"Beta"}));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":b,"o":7},"to":{"p":b,"o":7},"name":"Spot"}));
    let list = doc.bookmarks();
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].name.as_str(), list[0].p, list[0].o, list[0].end_p, list[0].end_o, list[0].text.as_str()), ("Beta", a, 6, a, 10, "beta"));
    // A point bookmark reports the start of its paragraph as its text, and starts where it ends.
    assert_eq!((list[1].name.as_str(), list[1].p, list[1].o, list[1].end_p, list[1].end_o, list[1].text.as_str()), ("Spot", b, 7, b, 7, "Second paragraph here"));
    let xml = para_xml(&doc, a);
    assert!(xml.contains(r#"<w:bookmarkStart w:id="1" w:name="Beta"/>"#) && xml.contains(r#"<w:bookmarkEnd w:id="1"/>"#), "{xml}");
    let (s, e) = (xml.find("bookmarkStart").unwrap(), xml.find("bookmarkEnd").unwrap());
    assert!(xml[..s].contains("Alpha ") && xml[s..e].contains("beta") && xml[e..].contains(" gamma"), "{xml}");
    let xml = para_xml(&doc, b);
    assert!(xml.find("bookmarkStart").unwrap() < xml.find("bookmarkEnd").unwrap(), "a point bookmark starts, then ends: {xml}");
    // Markers are zero-width: text and offsets are untouched.
    assert_eq!(texts(&doc), ["Alpha beta gamma", "Second paragraph here"]);
    // The render lists the names that start in a paragraph.
    let r = doc.render_snapshot();
    let RBlock::Para(rp) = &r.blocks[0] else { panic!() };
    assert_eq!(rp.bookmarks, ["Beta"]);
    // Saved and reopened, they are the same.
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let flat = |l: &[docxtor_core::marks::BookmarkInfo]| l.iter().map(|b| (b.name.clone(), b.o, b.end_o, b.text.clone())).collect::<Vec<_>>();
    assert_eq!(flat(&Document::open(&bytes).unwrap().bookmarks()), flat(&list));
}

#[test]
fn a_bookmark_can_span_paragraphs_and_long_text_is_trimmed() {
    let long = "word ".repeat(30);
    let mut doc = doc_with(&format!("First paragraph\n\nSecond paragraph\n\n{long}"));
    let (a, b, c) = (find_p(&doc, "First"), find_p(&doc, "Second"), find_p(&doc, "word"));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":b,"o":6},"name":"Across"}));
    let bm = &doc.bookmarks()[0];
    assert_eq!((bm.p, bm.o, bm.end_p, bm.end_o), (a, 6, b, 6));
    assert_eq!(bm.text, "paragraph Second");
    // The start is in the first paragraph, the end in the last.
    assert!(para_xml(&doc, a).contains("bookmarkStart") && !para_xml(&doc, a).contains("bookmarkEnd"));
    assert!(para_xml(&doc, b).contains("bookmarkEnd") && !para_xml(&doc, b).contains("bookmarkStart"));
    // Reversed positions are fine.
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":c,"o":100},"to":{"p":c,"o":0},"name":"Long"}));
    let long_bm = doc.bookmarks().into_iter().find(|b| b.name == "Long").unwrap();
    assert!(long_bm.text.chars().count() <= 61 && long_bm.text.ends_with('…'), "{:?}", long_bm.text);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn bookmarks_in_headers_get_their_own_ids_and_come_after_the_body() {
    let mut doc = doc_with("Alpha beta");
    let a = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":0},"to":{"p":a,"o":5},"name":"Body_one"}));
    let r = doc.apply_json(&json!({"op":"add_header_footer","kind":"header"}).to_string(), false).unwrap();
    let h = r[0].caret.unwrap().p.0;
    ops(&mut doc, json!({"op":"insert_text","at":{"p":h,"o":0},"text":"Running head"}));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":h,"o":0},"to":{"p":h,"o":7},"name":"Head"}));
    // A bookmark defined first in the header is still listed after the body's.
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":a,"o":10},"name":"Body_two"}));
    assert_eq!(names(&doc), ["Body_one", "Body_two", "Head"]);
    let head = doc.bookmarks().into_iter().find(|b| b.name == "Head").unwrap();
    assert_eq!((head.p, head.o, head.end_o, head.text.as_str()), (h, 0, 7, "Running"));
    // It says where it lives; the body's bookmarks say nothing.
    let story = head.story.as_ref().expect("a header bookmark names its header");
    assert_eq!((story.kind, story.part.as_str(), story.note_id.as_deref()), (docxtor_core::stories::StoryKind::Header, "word/header1.xml", None));
    assert!(doc.bookmarks().iter().filter(|b| b.name != "Head").all(|b| b.story.is_none()));
    // Names are unique across the body and the header; so are ids.
    assert!(err(&mut doc, json!({"op":"insert_bookmark","from":{"p":h,"o":0},"to":{"p":h,"o":1},"name":"body_ONE"})).contains("already a bookmark"));
    let body_xml = saved_part(&mut doc, "word/document.xml");
    let head_xml = saved_part(&mut doc, "word/header1.xml");
    let mut ids = bookmark_ids(&body_xml);
    ids.extend(bookmark_ids(&head_xml));
    let mut dedup = ids.clone();
    dedup.sort();
    dedup.dedup();
    assert_eq!(ids.len(), 3);
    assert_eq!(dedup.len(), 3, "bookmark ids are unique across parts: {ids:?}");
    assert_eq!(names(&reopen(&mut doc)), ["Body_one", "Body_two", "Head"]);
    // And a note's bookmark names the note.
    let r = doc.apply_json(&json!({"op":"insert_footnote","at":{"p":a,"o":5},"text":"A note"}).to_string(), false).unwrap();
    let n = r[0].caret.unwrap().p.0;
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":n,"o":2},"to":{"p":n,"o":6},"name":"In_note"}));
    let note = doc.bookmarks().into_iter().find(|b| b.name == "In_note").unwrap();
    let story = note.story.expect("a note bookmark names its note");
    assert_eq!((story.kind, story.part.as_str(), story.note_id.as_deref()), (docxtor_core::stories::StoryKind::Footnote, "word/footnotes.xml", Some("1")));
}

#[test]
fn bookmark_names_follow_words_rules() {
    let mut doc = doc_with("Alpha beta");
    let a = find_p(&doc, "Alpha");
    let at = |name: &str| json!({"op":"insert_bookmark","from":{"p":a,"o":0},"to":{"p":a,"o":5},"name":name});
    for (bad, hint) in [("", "Give the bookmark a name"), ("two words", "two_words"), ("1st", "start with a letter"), ("_hidden", "start with a letter"), ("a-b", "letters, numbers and underscores"), (&"x".repeat(41), "40 characters")] {
        let e = err(&mut doc, at(bad));
        assert!(e.contains(hint), "{bad:?}: {e}");
    }
    assert!(doc.bookmarks().is_empty());
    assert!(!doc.can_undo() || texts(&doc) == ["Alpha beta"]);
    ops(&mut doc, at("Intro"));
    // Duplicates are rejected whatever the case; nothing changes.
    let before = doc.can_undo();
    assert!(err(&mut doc, at("INTRO")).contains("already a bookmark called INTRO"));
    assert_eq!(names(&doc), ["Intro"]);
    assert_eq!(doc.can_undo(), before);
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":a,"o":10},"name":"Intro_2"}));
    assert!(err(&mut doc, json!({"op":"insert_bookmark","from":{"p":999,"o":0},"to":{"p":999,"o":0},"name":"Gone"})).contains("not found"));
}

#[test]
fn deleting_a_bookmark_removes_both_markers_and_turns_links_to_it_into_plain_text() {
    let mut doc = doc_with("Alpha beta gamma\n\nSee the beta and the web");
    let (a, b) = (find_p(&doc, "Alpha"), find_p(&doc, "See"));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":a,"o":10},"name":"Beta"}));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":11},"to":{"p":a,"o":16},"name":"Gamma"}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":b,"o":8},"to":{"p":b,"o":12},"url":"#Beta"}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":b,"o":0},"to":{"p":b,"o":3},"url":"#Gamma"}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":b,"o":21},"to":{"p":b,"o":24},"url":"https://example.com/"}));
    assert!(err(&mut doc, json!({"op":"delete_bookmark","name":"Nope"})).contains("no bookmark called Nope"));
    ops(&mut doc, json!({"op":"delete_bookmark","name":"beta"}));
    assert_eq!(names(&doc), ["Gamma"]);
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(!xml.contains(r#"w:name="Beta""#), "{xml}");
    // A link must lead somewhere: the link to the deleted bookmark is plain text now, its words kept; the other
    // links stay links.
    assert!(!xml.contains(r#"w:anchor="Beta""#), "{xml}");
    assert!(xml.contains(r#"w:anchor="Gamma""#), "{xml}");
    assert_eq!(xml.matches(r#"w:val="Hyperlink""#).count(), 2, "only the two remaining links look like links: {xml}");
    assert!(texts(&doc).iter().any(|t| t == "See the beta and the web"), "the words stay");
    assert!(doc.undo());
    assert_eq!(names(&doc), ["Beta", "Gamma"]);
    assert!(saved_part(&mut doc, "word/document.xml").contains(r#"w:anchor="Beta""#), "undo brings the link back");
    assert!(doc.redo());
    assert_eq!(names(&doc), ["Gamma"]);
    // A file with duplicate names loses all of them.
    let mut dup = doc_with("One two");
    let mut dup = rewrite(&mut dup, |pkg| {
        set_part(pkg, "word/document.xml", |s| {
            let marks = r#"<w:bookmarkStart w:id="7" w:name="Twice"/><w:bookmarkEnd w:id="7"/><w:bookmarkStart w:id="8" w:name="Twice"/><w:bookmarkEnd w:id="8"/>"#;
            s.replacen("<w:sectPr", &format!("<w:p>{marks}<w:r><w:t>x</w:t></w:r></w:p><w:sectPr"), 1)
        })
    });
    assert_eq!(names(&dup).len(), 2);
    ops(&mut dup, json!({"op":"delete_bookmark","name":"Twice"}));
    assert!(dup.bookmarks().is_empty() && !saved_part(&mut dup, "word/document.xml").contains("bookmark"));
}

#[test]
fn deleting_a_bookmark_turns_field_links_to_it_into_plain_text() {
    // Word can write a link to a bookmark as a HYPERLINK \l field, complex or simple.
    let mut doc = doc_with("Target");
    let a = find_p(&doc, "Target");
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":0},"to":{"p":a,"o":6},"name":"Goal"}));
    let mut doc = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/document.xml", |s| {
            let complex = r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> HYPERLINK \l "Goal" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>first</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#;
            let simple = r#"<w:p><w:fldSimple w:instr=" HYPERLINK \l &quot;Goal&quot; "><w:r><w:t>second</w:t></w:r></w:fldSimple></w:p>"#;
            s.replacen("<w:sectPr", &format!("{complex}{simple}<w:sectPr"), 1)
        })
    });
    ops(&mut doc, json!({"op":"delete_bookmark","name":"Goal"}));
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(!xml.contains("HYPERLINK"), "{xml}");
    assert!(texts(&doc).iter().any(|t| t == "first") && texts(&doc).iter().any(|t| t == "second"), "the words stay: {:?}", texts(&doc));
}

/// Paragraph XML with a simple field, complex fields (one with its instruction split in pieces) and others.
pub(super) fn fields_xml() -> String {
    let complex = |pieces: &[&str]| {
        let instr: String = pieces.iter().map(|p| format!(r#"<w:r><w:instrText xml:space="preserve">{p}</w:instrText></w:r>"#)).collect();
        format!(r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r>{instr}<w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>3</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#)
    };
    format!(
        r#"<w:p><w:fldSimple w:instr=" REF Intro \h "><w:r><w:t>simple</w:t></w:r></w:fldSimple>{}{}{}{}<w:hyperlink w:anchor="intro"><w:r><w:t>case</w:t></w:r></w:hyperlink></w:p>"#,
        complex(&[" PAGEREF ", "Intro", " \\h "]),
        complex(&[" REF intro \\h "]),
        complex(&[" REF Other \\h "]),
        complex(&[" HYPERLINK \\l \"Intro\" "]),
    )
}

#[test]
fn renaming_a_bookmark_updates_links_and_fields() {
    let mut doc = doc_with("Alpha beta gamma\n\nSee the beta");
    let (a, b) = (find_p(&doc, "Alpha"), find_p(&doc, "See"));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":0},"to":{"p":a,"o":5},"name":"Intro"}));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":a,"o":10},"name":"Other"}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":b,"o":8},"to":{"p":b,"o":12},"url":"#Intro"}));
    let mut doc = rewrite(&mut doc, |pkg| set_part(pkg, "word/document.xml", |s| s.replacen("<w:sectPr", &format!("{}<w:sectPr", fields_xml()), 1)));
    assert_eq!(names(&doc), ["Intro", "Other"]);

    assert!(err(&mut doc, json!({"op":"rename_bookmark","from":"Intro","to":"other"})).contains("already a bookmark"));
    assert!(err(&mut doc, json!({"op":"rename_bookmark","from":"Missing","to":"Fresh"})).contains("no bookmark called Missing"));
    assert!(err(&mut doc, json!({"op":"rename_bookmark","from":"Intro","to":"Not valid"})).contains("spaces"));
    ops(&mut doc, json!({"op":"rename_bookmark","from":"intro","to":"Start"}));
    assert_eq!(names(&doc), ["Start", "Other"]);
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"w:name="Start""#) && !xml.contains(r#"w:name="Intro""#), "{xml}");
    assert!(xml.contains(r#"w:anchor="Start""#) && !xml.contains(r#"w:anchor="Intro""#) && !xml.contains(r#"w:anchor="intro""#), "links follow: {xml}");
    assert!(xml.contains(r#"w:instr=" REF Start \h ""#), "simple field: {xml}");
    assert!(xml.contains(" REF Start \\h ") && xml.contains(r#" HYPERLINK \l "Start" "#), "complex fields: {xml}");
    // An instruction split over several pieces is rewritten into the first one.
    assert!(xml.contains(" PAGEREF Start \\h "), "{xml}");
    assert!(!xml.contains(">Intro<") && !xml.contains(">intro<"), "{xml}");
    // Fields and links to other bookmarks stay.
    assert!(xml.contains(" REF Other \\h "), "{xml}");
    // The link text renders with the new target.
    let r = doc.render_snapshot();
    let links: Vec<String> = r.blocks.iter().filter_map(|b| if let RBlock::Para(p) = b { Some(p) } else { None }).flat_map(|p| p.segs.iter().filter_map(|s| s.link.clone())).collect();
    assert!(links.contains(&"#Start".to_string()) && !links.contains(&"#Intro".to_string()), "{links:?}");
    // Changing only the case is a rename too, and undo restores the old name everywhere.
    ops(&mut doc, json!({"op":"rename_bookmark","from":"Start","to":"START"}));
    assert_eq!(names(&doc)[0], "START");
    assert!(doc.undo() && doc.undo());
    assert_eq!(names(&doc), ["Intro", "Other"]);
    assert!(saved_part(&mut doc, "word/document.xml").contains(r#"w:anchor="Intro""#));
}

#[test]
fn renaming_reaches_links_in_other_stories() {
    let mut doc = doc_with("Alpha beta");
    let a = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":0},"to":{"p":a,"o":5},"name":"Top"}));
    let r = doc.apply_json(&json!({"op":"add_header_footer","kind":"header"}).to_string(), false).unwrap();
    let h = r[0].caret.unwrap().p.0;
    ops(&mut doc, json!({"op":"insert_text","at":{"p":h,"o":0},"text":"Back to top"}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":h,"o":8},"to":{"p":h,"o":11},"url":"#Top"}));
    doc.render_all();
    ops(&mut doc, json!({"op":"rename_bookmark","from":"Top","to":"Summit"}));
    assert!(doc.render_changed().stories, "the editor reloads headers after a rename");
    assert!(saved_part(&mut doc, "word/header1.xml").contains(r#"w:anchor="Summit""#));
    assert!(saved_part(&mut doc, "word/document.xml").contains(r#"w:name="Summit""#));
}

#[test]
fn hidden_bookmarks_stay_out_of_the_list_but_keep_their_render_names() {
    let mut doc = doc_with("# Title\n\nBody text");
    ops(&mut doc, json!({"op":"insert_toc"}));
    let t = doc.render_snapshot().blocks.iter().find_map(|b| match b {
        RBlock::Para(p) if p.style.as_deref() == Some("Heading1") => Some(p.id),
        _ => None,
    });
    let t = t.unwrap();
    // The table of contents made `_Toc…` bookmarks on the headings; people don't see them.
    assert!(doc.bookmarks().is_empty());
    let r = doc.render_snapshot();
    let heading = r.blocks.iter().find_map(|b| match b {
        RBlock::Para(p) if p.id == t => Some(p),
        _ => None,
    });
    assert!(heading.unwrap().bookmarks.iter().any(|n| n.starts_with("_Toc")), "links in the table of contents need these");
    // New bookmarks get fresh ids beside them, and a hidden name can't be taken.
    let body = find_p(&doc, "Body");
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":body,"o":0},"to":{"p":body,"o":4},"name":"Mine"}));
    assert_eq!(names(&doc), ["Mine"]);
    let xml = saved_part(&mut doc, "word/document.xml");
    let ids = bookmark_ids(&xml);
    let mut d = ids.clone();
    d.sort();
    d.dedup();
    assert_eq!(ids.len(), d.len(), "{ids:?}");
    // Hidden ones can't be created, but the TOC can still be rebuilt.
    assert!(err(&mut doc, json!({"op":"insert_bookmark","from":{"p":body,"o":0},"to":{"p":body,"o":1},"name":"_Toc99"})).contains("start with a letter"));
    ops(&mut doc, json!({"op":"update_toc"}));
    assert_eq!(names(&doc), ["Mine"]);
}

#[test]
fn bookmarks_undo_redo_and_survive_a_round_trip() {
    let mut doc = doc_with("Alpha beta gamma");
    let a = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":a,"o":10},"name":"Beta"}));
    assert!(doc.undo());
    assert!(doc.bookmarks().is_empty() && !para_xml(&doc, a).contains("bookmark"));
    assert!(doc.redo());
    assert_eq!(names(&doc), ["Beta"]);
    // A batch is one undo step.
    ops(&mut doc, json!([
        {"op":"rename_bookmark","from":"Beta","to":"Gamma"},
        {"op":"insert_bookmark","from":{"p":a,"o":0},"to":{"p":a,"o":5},"name":"First"},
    ]));
    assert_eq!(names(&doc), ["First", "Gamma"]);
    assert!(doc.undo());
    assert_eq!(names(&doc), ["Beta"]);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    assert_eq!(names(&Document::open(&bytes).unwrap()), ["Beta"]);
}

#[test]
fn links_to_bookmarks_round_trip() {
    let mut doc = doc_with("Alpha beta gamma\n\nSee the beta");
    let (a, b) = (find_p(&doc, "Alpha"), find_p(&doc, "See"));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":a,"o":6},"to":{"p":a,"o":10},"name":"Beta"}));
    let r = call_tool(&mut doc, "set_link", &json!({"id": b, "text": "beta", "url": "#Beta"})).unwrap();
    assert!(r.contains("[beta](#Beta)"), "{r}");
    let xml = saved_part(&mut doc, "word/document.xml");
    assert!(xml.contains(r#"<w:hyperlink w:anchor="Beta" w:history="1">"#), "{xml}");
    let mut again = reopen(&mut doc);
    let rendered = again.render_snapshot();
    let RBlock::Para(p) = &rendered.blocks[1] else { panic!() };
    assert_eq!(p.segs.iter().find_map(|s| s.link.as_deref()), Some("#Beta"));
    let script = call_tool(&mut again, "read_document", &json!({})).unwrap();
    assert!(script.contains("[beta](#Beta)"), "{script}");
    // Saving again changes nothing.
    assert_eq!(saved_part(&mut again, "word/document.xml"), xml);
}
