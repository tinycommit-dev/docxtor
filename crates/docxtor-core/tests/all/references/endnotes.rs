//! Tests of: Endnotes.

use super::*;

#[test]
fn endnotes_insert_number_and_part() {
    let mut doc = doc_with("Alpha beta gamma.");
    let p = find_p(&doc, "Alpha");
    let r = ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":5},"text":"First."}));
    let caret = r[0].caret.unwrap();
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":10},"text":"Second."}));
    // The caret landed in the new note, which is a story.
    assert!(doc.find_any_para(caret.p.0).is_some() && find_block(&doc.body, caret.p.0).is_none());
    assert_eq!(story_text(&doc, StoryKind::Endnote), " First. Second.");
    // Numbered i, ii in reference order, with the mark drawn as an endnote.
    let ps = doc.page_setup();
    assert_eq!(ps.endnotes.len(), 2);
    assert_eq!(note_labels(&doc, true), ["i", "ii"]);
    assert_eq!(ps.endnotes[0].num, 1);
    assert!(ps.notes.is_empty());
    assert_eq!(body_labels(&doc, p), [("i".to_string(), Some("endnote".to_string())), ("ii".to_string(), Some("endnote".to_string()))]);
    // The note's own mark renders with its formatted number too.
    let first = serde_json::to_value(&ps.endnotes[0]).unwrap();
    assert!(first["blocks"][0]["segs"][0]["label"] == "i" && first["blocks"][0]["segs"][0]["note"] == "endnote");

    // The part: separators as Word writes them, the mark, the styles, the relationship and content type.
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let pkg = Package::open(&saved).unwrap();
    let notes = String::from_utf8(pkg.get("word/endnotes.xml").unwrap().to_vec()).unwrap();
    assert!(notes.contains(r#"<w:endnote w:type="separator" w:id="-1">"#) && notes.contains("<w:separator/>"), "{notes}");
    assert!(notes.contains(r#"<w:endnote w:type="continuationSeparator" w:id="0">"#) && notes.contains("<w:continuationSeparator/>"));
    assert!(notes.contains(r#"<w:endnote w:id="1">"#) && notes.contains("<w:endnoteRef/>") && notes.contains(r#"<w:pStyle w:val="EndnoteText"/>"#) && notes.contains(r#"<w:rStyle w:val="EndnoteReference"/>"#));
    let types = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(types.contains("/word/endnotes.xml") && types.contains("wordprocessingml.endnotes+xml"));
    let rels = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
    assert!(rels.contains("relationships/endnotes") && rels.contains(r#"Target="endnotes.xml""#));
    let styles = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"w:styleId="EndnoteText""#) && styles.contains(r#"w:styleId="EndnoteReference""#));
    let settings = String::from_utf8(pkg.get("word/settings.xml").unwrap().to_vec()).unwrap();
    assert!(settings.contains(r#"<w:endnotePr><w:endnote w:id="-1"/><w:endnote w:id="0"/></w:endnotePr>"#), "{settings}");
    assert!(body_xml(&doc).contains(r#"<w:endnoteReference w:id="1"/>"#));
    // Reopened, the notes are still there and numbered the same way.
    let re = Document::open(&saved).unwrap();
    assert_eq!(note_labels(&re, true), ["i", "ii"]);
    assert_eq!(story_text(&re, StoryKind::Endnote), " First. Second.");
}

#[test]
fn endnote_text_is_editable_and_undoable() {
    let mut doc = doc_with("Alpha beta.");
    let p = find_p(&doc, "Alpha");
    let r = ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":5},"text":"Note"}));
    let caret = r[0].caret.unwrap();
    ops(&mut doc, json!({"op":"insert_text","at":caret,"text":" typed in the note"}));
    ops(&mut doc, json!({"op":"format_text","from":{"p":caret.p.0,"o":2},"to":{"p":caret.p.0,"o":6},"format":{"bold":true}}));
    assert_eq!(story_text(&doc, StoryKind::Endnote), " Note typed in the note");
    let xml = saved_part(&mut doc, "word/endnotes.xml");
    assert!(xml.contains("<w:b/>") && xml.contains(" typed in the note"));
    // Undo twice: the text, then the note itself.
    assert!(doc.undo() && doc.undo() && doc.undo());
    assert_eq!(doc.page_setup().endnotes.len(), 0);
    assert!(!body_xml(&doc).contains("endnoteReference"));
    // The undone note leaves no part and no relationship behind.
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    assert!(pkg.get("word/endnotes.xml").is_none(), "the endnotes part of an undone note stays");
    let rels = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
    assert!(!rels.contains("relationships/endnotes"), "{rels}");
    assert!(doc.redo());
    assert_eq!(story_text(&doc, StoryKind::Endnote), " Note");
    assert!(doc.redo());
    assert_eq!(story_text(&doc, StoryKind::Endnote), " Note typed in the note");
    // Inserting again after an undo doesn't make a second part.
    assert!(doc.undo() && doc.undo());
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":2},"text":"Again"}));
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    assert_eq!(pkg.parts.iter().filter(|p| p.name.contains("endnotes")).count(), 1);
    let rels = String::from_utf8(pkg.get("word/_rels/document.xml.rels").unwrap().to_vec()).unwrap();
    assert_eq!(rels.matches("relationships/endnotes").count(), 1);
    assert_valid(&bytes);
}

#[test]
fn footnotes_and_endnotes_coexist() {
    let mut doc = doc_with("One two three four.");
    let p = find_p(&doc, "One");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":3},"text":"Foot A."}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":7},"text":"End A."}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":13},"text":"Foot B."}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":18},"text":"End B."}));
    // Both kinds use ids 1 and 2 in their own parts.
    assert_eq!(note_labels(&doc, false), ["1", "2"]);
    assert_eq!(note_labels(&doc, true), ["i", "ii"]);
    assert_eq!(story_text(&doc, StoryKind::Footnote), " Foot A. Foot B.");
    assert_eq!(story_text(&doc, StoryKind::Endnote), " End A. End B.");
    let kinds: Vec<_> = body_labels(&doc, p).into_iter().map(|(l, k)| format!("{l}:{}", k.unwrap_or_default())).collect();
    assert_eq!(kinds, ["1:", "i:endnote", "2:", "ii:endnote"]);
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let re = Document::open(&saved).unwrap();
    assert_eq!(note_labels(&re, false), ["1", "2"]);
    assert_eq!(note_labels(&re, true), ["i", "ii"]);
    // Editing a footnote leaves the endnotes part alone.
    let pkg = Package::open(&saved).unwrap();
    let end_before = pkg.get("word/endnotes.xml").unwrap().to_vec();
    let mut re = re;
    let fid = re.stories.iter().find(|s| s.kind == StoryKind::Footnote).unwrap().blocks[0].id();
    ops(&mut re, json!({"op":"insert_text","at":{"p":fid,"o":0},"text":"!"}));
    let pkg2 = Package::open(&re.save().unwrap()).unwrap();
    assert_eq!(pkg2.get("word/endnotes.xml").unwrap(), &end_before[..]);
    // The Markdown export keeps both lists apart.
    let md = docxtor_core::export::to_markdown(&re);
    assert!(md.contains("Foot A.") && md.contains("End A."), "{md}");
}

#[test]
fn note_formats_start_and_defaults() {
    let mut doc = doc_with("Alpha beta gamma.");
    let p = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":5},"text":"F1"}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":10},"text":"F2"}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":6},"text":"E1"}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":11},"text":"E2"}));
    // Word's defaults: footnotes decimal, endnotes lower Roman.
    assert_eq!(note_labels(&doc, false), ["1", "2"]);
    assert_eq!(note_labels(&doc, true), ["i", "ii"]);
    ops(&mut doc, json!({"op":"set_note_format","kind":"endnote","format":"decimal","start":3}));
    ops(&mut doc, json!({"op":"set_note_format","kind":"footnote","format":"upperLetter"}));
    assert_eq!(note_labels(&doc, true), ["3", "4"]);
    assert_eq!(note_labels(&doc, false), ["A", "B"]);
    // The body's marks follow after a render.
    let _ = doc.render_changed();
    let labels: Vec<_> = body_labels(&doc, p).into_iter().map(|(l, _)| l).collect();
    assert_eq!(labels, ["A", "3", "4", "B"]);
    ops(&mut doc, json!({"op":"set_note_format","kind":"footnote","format":"symbol"}));
    assert_eq!(note_labels(&doc, false), ["*", "\u{2020}"]);
    // settings.xml in schema order: the number format and start come before the separator references.
    let settings = saved_part(&mut doc, "word/settings.xml");
    assert!(settings.contains(r#"<w:endnotePr><w:endnote w:id="-1"/><w:endnote w:id="0"/></w:endnotePr>"#) || settings.contains(r#"<w:endnotePr><w:numFmt w:val="decimal"/><w:numStart w:val="3"/><w:endnote"#), "{settings}");
    assert!(settings.contains(r#"<w:footnotePr><w:numFmt w:val="chicago"/>"#), "{settings}");
    assert!(settings.find("<w:footnotePr>").unwrap() < settings.find("<w:endnotePr>").unwrap());
    assert!(settings.find("<w:endnotePr>").unwrap() < settings.find("<w:compat>").unwrap());
    // Survives a save and a reopen; undo goes back.
    let mut re = reopen(&mut doc);
    assert_eq!(note_labels(&re, true), ["3", "4"]);
    assert_eq!(note_labels(&re, false), ["*", "\u{2020}"]);
    assert!(doc.undo());
    assert_eq!(note_labels(&doc, false), ["A", "B"]);
    assert!(doc.undo() && doc.undo());
    assert_eq!(note_labels(&doc, true), ["i", "ii"]);
    // Bad input is refused.
    assert!(err(&mut re, json!({"op":"set_note_format","kind":"footnote","format":"hexagonal"})).contains("Unknown number format"));
    assert!(err(&mut re, json!({"op":"set_note_format","kind":"footnote","format":"decimal","start":0})).contains("1 or more"));
}

#[test]
fn note_format_in_the_last_section_wins() {
    let mut doc = doc_with("Alpha beta.");
    let p = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":5},"text":"F1"}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":6},"text":"E1"}));
    // Word-written section settings: lower letters for footnotes starting at 4, decimal endnotes.
    let mut re = rewrite(&mut doc, |pkg| {
        set_part(pkg, "word/document.xml", |s| s.replace("<w:pgSz", r#"<w:footnotePr><w:numFmt w:val="lowerLetter"/><w:numStart w:val="4"/></w:footnotePr><w:endnotePr><w:numFmt w:val="decimal"/></w:endnotePr><w:pgSz"#));
    });
    assert_eq!(note_labels(&re, false), ["d"]);
    assert_eq!(note_labels(&re, true), ["1"]);
    // Setting the document-wide format changes the section's too, so it isn't masked.
    ops(&mut re, json!({"op":"set_note_format","kind":"footnote","format":"upperRoman","start":2}));
    assert_eq!(note_labels(&re, false), ["II"]);
    let xml = saved_part(&mut re, "word/document.xml");
    assert!(xml.contains(r#"<w:footnotePr><w:numFmt w:val="upperRoman"/><w:numStart w:val="2"/></w:footnotePr><w:endnotePr><w:numFmt w:val="decimal"/></w:endnotePr><w:pgSz"#), "{xml}");
}

#[test]
fn convert_notes_between_footnotes_and_endnotes() {
    let mut doc = doc_with("Alpha beta gamma delta.");
    let p = find_p(&doc, "Alpha");
    let r1 = ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":5},"text":"One."}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":10},"text":"Two."}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":16},"text":"Three."}));
    let first_note_para = r1[0].caret.unwrap().p.0;
    // Convert only the second footnote (by its id).
    let second_id = doc.page_setup().notes[1].id.clone();
    let r = ops(&mut doc, json!({"op":"convert_notes","to":"endnote","ids":[second_id]}));
    assert_eq!(r[0].count, Some(1));
    assert_eq!(note_labels(&doc, false), ["1", "2"]);
    assert_eq!(note_labels(&doc, true), ["i"]);
    assert_eq!(story_text(&doc, StoryKind::Endnote), " Two.");
    assert_eq!(story_text(&doc, StoryKind::Footnote), " One. Three.");
    let _ = doc.render_changed();
    let kinds: Vec<_> = body_labels(&doc, p).into_iter().map(|(l, k)| format!("{l}:{}", k.unwrap_or_default())).collect();
    assert_eq!(kinds, ["1:", "i:endnote", "2:"]);
    // The converted note has the endnote styles and mark; the footnotes part lost it.
    let end = saved_part(&mut doc, "word/endnotes.xml");
    assert!(end.contains("<w:endnoteRef/>") && end.contains(r#"<w:pStyle w:val="EndnoteText"/>"#) && !end.contains("footnoteRef"), "{end}");
    let foot = saved_part(&mut doc, "word/footnotes.xml");
    assert!(!foot.contains("Two.") && foot.contains("One.") && foot.contains("Three."));
    // Everything goes back with the other direction; a note keeps its paragraph id.
    let r = ops(&mut doc, json!({"op":"convert_notes","to":"footnote"}));
    assert_eq!(r[0].count, Some(1));
    assert_eq!(note_labels(&doc, true), Vec::<String>::new());
    assert_eq!(note_labels(&doc, false), ["1", "2", "3"]);
    assert_eq!(story_text(&doc, StoryKind::Footnote), " One. Two. Three.");
    assert!(doc.find_any_para(first_note_para).is_some());
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let re = Document::open(&saved).unwrap();
    assert_eq!(note_labels(&re, false), ["1", "2", "3"]);
    // Undo restores the endnote, and nothing to convert is an error.
    assert!(doc.undo());
    assert_eq!(note_labels(&doc, true), ["i"]);
    let mut empty = doc_with("No notes here.");
    assert!(err(&mut empty, json!({"op":"convert_notes","to":"endnote"})).contains("no footnotes"));
}

#[test]
fn notes_cannot_be_inserted_inside_notes() {
    let mut doc = doc_with("Alpha beta.");
    let p = find_p(&doc, "Alpha");
    let r = ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":5},"text":"Note"}));
    let in_note = r[0].caret.unwrap().p.0;
    assert!(err(&mut doc, json!({"op":"insert_endnote","at":{"p":in_note,"o":1},"text":"x"})).contains("main text"));
    assert!(err(&mut doc, json!({"op":"insert_caption","after":in_note,"label":"Figure","text":"x"})).contains("main text"));
}
