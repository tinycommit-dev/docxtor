//! Tests of: Updating fields and index.

use super::*;

#[test]
fn update_fields_leaves_unknown_fields_and_untouched_documents_alone() {
    // A document with fields Docxtor doesn't update (DATE, a macro button, a made-up one) keeps them as they are.
    let body = r#"<w:p><w:r><w:t xml:space="preserve">Today is </w:t></w:r><w:fldSimple w:instr=" DATE \@ &quot;MMMM d, yyyy&quot; "><w:r><w:t>January 1, 2020</w:t></w:r></w:fldSimple><w:r><w:t xml:space="preserve"> and </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> MACROBUTTON NoMacro [click] </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>[click]</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve"> page </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>5</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#;
    let mut doc = doc_from_body(body);
    let before = para_xml(&doc, find_p(&doc, "Today"));
    let r = ops(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(r[0].count, Some(0));
    assert_eq!(para_xml(&doc, find_p(&doc, "Today")), before);
    assert_eq!(texts(&doc)[0], "Today is January 1, 2020 and [click] page 5");

    // An untouched document written back after an update that changed nothing is byte-identical, part by part.
    for name in ["simple.docx", "rich.docx"] {
        let original = fixture(name);
        let mut d = Document::open(&original).unwrap();
        ops(&mut d, json!({"op":"update_fields"}));
        let saved = d.save().unwrap();
        let (a, b) = (Package::open(&original).unwrap(), Package::open(&saved).unwrap());
        for part in &a.parts {
            assert_eq!(b.get(&part.name), Some(&part.data[..]), "{name}: {} changed", part.name);
        }
        assert_eq!(a.parts.len(), b.parts.len(), "{name}");
    }
}

#[test]
fn update_fields_scope_limits_the_paragraphs() {
    let mut doc = doc_with("One.\n\nTwo.");
    let (a, b) = (find_p(&doc, "One"), find_p(&doc, "Two"));
    ops(&mut doc, json!({"op":"insert_caption","after":a,"label":"Figure","text":"x"}));
    ops(&mut doc, json!({"op":"insert_caption","after":b,"label":"Figure","text":"y"}));
    // Make both stale by editing the XML, then update only the second.
    let mut re = rewrite(&mut doc, |pkg| set_part(pkg, "word/document.xml", |s| s.replace("<w:t>1</w:t>", "<w:t>8</w:t>").replace("<w:t>2</w:t>", "<w:t>9</w:t>")));
    assert_eq!(texts(&re), ["One.", "Figure 8: x", "Two.", "Figure 9: y"]);
    let second = find_p(&re, "Figure 9");
    ops(&mut re, json!({"op":"update_fields","scope":[second]}));
    assert_eq!(texts(&re), ["One.", "Figure 8: x", "Two.", "Figure 2: y"]);
    // The count before it still came from every caption, not just the scoped ones.
    ops(&mut re, json!({"op":"update_fields"}));
    assert_eq!(texts(&re), ["One.", "Figure 1: x", "Two.", "Figure 2: y"]);
}

#[test]
fn index_entries_are_hidden_and_take_no_space() {
    let mut doc = doc_with("Apples are red.\n\nBananas are yellow.");
    let p = find_p(&doc, "Apples");
    let before = texts(&doc);
    let r = ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":p,"o":6},"entry":"Apple","bold":true,"see":"See also Fruit"}));
    assert_eq!(r[0].caret.unwrap(), Pos::new(p, 6));
    assert_eq!(texts(&doc), before);
    assert_eq!(doc.text_range(Pos::new(p, 0), Pos::new(p, 15)), "Apples are red.");
    // Word's markup: a field of its own, hidden, with the switches.
    let x = para_xml(&doc, p);
    assert!(x.contains(r#"<w:instrText xml:space="preserve"> XE "Apple" \b \t "See also Fruit" </w:instrText>"#), "{x}");
    assert_eq!(x.matches("<w:vanish/>").count(), 3);
    assert!(x.contains(r#"<w:fldChar w:fldCharType="begin"/>"#) && x.contains(r#"<w:fldChar w:fldCharType="end"/>"#));
    assert!(!x.contains("separate"));
    // Rendering: no segment, no width. Offsets of what follows are unchanged.
    let s = segs(&doc, p);
    let shown: String = s.iter().map(|x| x["x"].as_str().unwrap_or("")).collect();
    assert_eq!(shown, "Apples are red.");
    assert_eq!(s.len(), 1, "{s:?}");
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":11},"text":"very "}));
    assert_eq!(texts(&doc)[0], "Apples are very red.");
    assert!(para_xml(&doc, p).contains(" XE "));
    // The first run of the paragraph is split at the mark, nothing else.
    assert_valid(&doc.save().unwrap());
    assert!(err(&mut doc, json!({"op":"mark_index_entry","at":{"p":p,"o":0},"entry":"  "})).contains("some text"));
}

#[test]
fn mark_all_marks_the_first_occurrence_in_each_paragraph() {
    let mut doc = doc_with("The apple fell. Another apple.\n\nNo fruit here.\n\nApple pie and apple tart.");
    let r = ops(&mut doc, json!({"op":"mark_all_index_entries","text":"apple","entry":"Apple"}));
    // Case-sensitive: "Apple pie" only matches through its second word; three paragraphs have the word.
    assert_eq!(r[0].count, Some(2));
    let first = find_p(&doc, "The apple");
    assert_eq!(para_xml(&doc, first).matches(" XE ").count(), 1);
    assert_eq!(para_xml(&doc, find_p(&doc, "No fruit")).matches(" XE ").count(), 0);
    // Marking again adds nothing, even ignoring case; text that isn't there is an error.
    let r = ops(&mut doc, json!({"op":"mark_all_index_entries","text":"apple","entry":"Apple"}));
    assert_eq!(r[0].count, Some(0));
    let r = ops(&mut doc, json!({"op":"mark_all_index_entries","text":"APPLE","entry":"Apple","ignore_case":true}));
    assert_eq!(r[0].count, Some(0));
    assert!(err(&mut doc, json!({"op":"mark_all_index_entries","text":"durian"})).contains("isn't in the document"));
    let r = ops(&mut doc, json!({"op":"mark_all_index_entries","text":"fruit","whole_word":true,"sub":"generic"}));
    assert_eq!(r[0].count, Some(1));
    assert!(para_xml(&doc, find_p(&doc, "No fruit")).contains(r#"XE "fruit:generic""#));
}

pub(super) fn indexed_doc() -> (Document, [u32; 4]) {
    let mut doc = doc_with("Apples grow on trees.\n\nBananas are yellow.\n\nA Gala is a kind of apple.\n\nCherries are small.");
    let ps = [find_p(&doc, "Apples"), find_p(&doc, "Bananas"), find_p(&doc, "Gala"), find_p(&doc, "Cherries")];
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[0],"o":6},"entry":"Apple"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[1],"o":7},"entry":"Banana","bold":true}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[2],"o":6},"entry":"Apple","sub":"Gala"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[2],"o":25},"entry":"apple"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[3],"o":8},"entry":"Cherry","see":"See Fruit"}));
    (doc, ps)
}

#[test]
fn index_is_generated_sorted_with_subentries_and_pages() {
    let (mut doc, ps) = indexed_doc();
    let pg = pages(&[(ps[0], 2), (ps[1], 3), (ps[2], 4), (ps[3], 9)]);
    let r = ops(&mut doc, json!({"op":"insert_index","after":ps[3],"pages":pg}));
    let created = r[0].created.clone();
    let t = texts(&doc);
    let idx = t.iter().position(|x| x.starts_with("Apple\t")).unwrap();
    // Sorted without regard to case, sub-entries under their entry, page numbers merged, see-references in place of pages.
    assert_eq!(&t[idx..idx + 5], ["Apple\t2, 4", "Gala\t4", "Banana\t3", "Cherry\tSee Fruit", ""]);
    assert_eq!(created.len(), 4);
    // Styles: Index 1 for entries, Index 2 for the sub-entry.
    let x = |i: usize| para_xml(&doc, created[i]);
    assert!(x(0).contains(r#"<w:pStyle w:val="Index1"/>"#) && x(1).contains(r#"<w:pStyle w:val="Index2"/>"#));
    // A right tab with dot leader at the column's right edge (text width 9360, two columns, 720 gap).
    assert!(x(0).contains(r#"<w:tab w:val="right" w:leader="dot" w:pos="4320"/>"#), "{}", x(0));
    // The field: begin, instruction, separate in the first paragraph; end in the last.
    assert!(x(0).contains(r#"<w:instrText xml:space="preserve"> INDEX \e "	" \c "2" \z "1033" </w:instrText>"#), "{}", x(0));
    assert!(x(0).contains(r#"w:fldCharType="separate""#) && x(3).contains(r#"<w:fldChar w:fldCharType="end"/>"#));
    // Bold page number for the bold entry; italic "See".
    assert!(x(2).contains(r#"<w:rPr><w:b/></w:rPr><w:t xml:space="preserve">3</w:t>"#), "{}", x(2));
    assert!(x(3).contains(r#"<w:rPr><w:i/></w:rPr><w:t xml:space="preserve">See</w:t>"#), "{}", x(3));
    // The index sits in a continuous section of its own with two columns, between two sections of one.
    let ps_ = doc.page_setup();
    assert_eq!(ps_.sections.len(), 3);
    assert_eq!((ps_.sections[0].cols.num, ps_.sections[1].cols.num, ps_.sections[2].cols.num), (1, 2, 1));
    assert_eq!(ps_.sections[1].start, "continuous");
    assert_eq!(ps_.sections[2].start, "continuous");
    assert_eq!(ps_.sections[1].end, Some(created[3]));
    assert_eq!(ps_.sections[0].end, Some(ps[3]));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let re = Document::open(&saved).unwrap();
    assert_eq!(re.page_setup().sections.len(), 3);
    assert_eq!(texts(&re), t);
    // The styles exist.
    let styles = saved_part(&mut doc, "word/styles.xml");
    assert!(styles.contains(r#"w:styleId="Index1""#) && styles.contains(r#"w:styleId="Index2""#));
    // Undo takes the whole index and its sections away.
    assert!(doc.undo());
    assert_eq!(doc.page_setup().sections.len(), 1);
    assert!(!texts(&doc).iter().any(|x| x.starts_with("Apple\t")));
}

#[test]
fn index_page_ranges_headings_and_run_in() {
    let mut doc = doc_with("Alpha one.\n\nAlpha two.\n\nAlpha three.\n\nAlpha four.\n\nZeta and the end.");
    let ps: Vec<u32> = ["Alpha one", "Alpha two", "Alpha three", "Alpha four", "Zeta"].iter().map(|t| find_p(&doc, t)).collect();
    for p in &ps[..4] {
        ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":p,"o":5},"entry":"Alpha"}));
    }
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[4],"o":4},"entry":"Zeta","sub":"final"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[4],"o":4},"entry":"Zeta","sub":"other"}));
    // Consecutive pages run together as a range; others are listed.
    let pg = pages(&[(ps[0], 2), (ps[1], 4), (ps[2], 5), (ps[3], 6), (ps[4], 9)]);
    let r = ops(&mut doc, json!({"op":"insert_index","columns":1,"headings":true,"pages":pg}));
    let t = texts(&doc);
    let i = t.iter().position(|x| x == "A").unwrap();
    assert_eq!(&t[i..i + 6], ["A", "Alpha\t2, 4\u{2013}6", "Z", "Zeta", "final\t9", "other\t9"]);
    assert!(para_xml(&doc, r[0].created[0]).contains(r#"<w:pStyle w:val="IndexHeading"/>"#));
    // One column needs no section of its own.
    assert_eq!(doc.page_setup().sections.len(), 1);
    assert!(para_xml(&doc, r[0].created[0]).contains(r#"\h "A""#));

    // Run-in: sub-entries follow their entry in one paragraph.
    let mut doc2 = doc_with("Alpha one.\n\nZeta and the end.");
    let (a, z) = (find_p(&doc2, "Alpha"), find_p(&doc2, "Zeta"));
    ops(&mut doc2, json!({"op":"mark_index_entry","at":{"p":z,"o":4},"entry":"Zeta","sub":"final"}));
    ops(&mut doc2, json!({"op":"mark_index_entry","at":{"p":z,"o":4},"entry":"Zeta"}));
    ops(&mut doc2, json!({"op":"mark_index_entry","at":{"p":a,"o":5},"entry":"Zeta","sub":"second"}));
    let r = ops(&mut doc2, json!({"op":"insert_index","after":z,"columns":2,"run_in":true,"pages":pages(&[(a, 1), (z, 3)])}));
    let t = texts(&doc2);
    assert!(t.iter().any(|x| x == "Zeta, 3: final, 3; second, 1"), "{t:?}");
    assert!(para_xml(&doc2, r[0].created[0]).contains(r#"\r"#));
}

#[test]
fn index_updates_after_an_entry_is_added() {
    let (mut doc, ps) = indexed_doc();
    let pg = pages(&[(ps[0], 2), (ps[1], 3), (ps[2], 4), (ps[3], 9)]);
    ops(&mut doc, json!({"op":"insert_index","after":ps[3],"pages":pg}));
    // A new entry, and a heading text that moved.
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[3],"o":0},"entry":"Berry"}));
    ops(&mut doc, json!({"op":"mark_index_entry","at":{"p":ps[1],"o":0},"entry":"Apple","sub":"Granny"}));
    let r = ops(&mut doc, json!({"op":"update_fields","pages":pages(&[(ps[0], 2), (ps[1], 3), (ps[2], 4), (ps[3], 10)])}));
    assert_eq!(r[0].count, Some(1));
    let t = texts(&doc);
    let idx = t.iter().position(|x| x.starts_with("Apple\t")).unwrap();
    assert_eq!(&t[idx..idx + 6], ["Apple\t2, 4", "Gala\t4", "Granny\t3", "Banana\t3", "Berry\t10", "Cherry\tSee Fruit"]);
    // Still one index in its own two-column section, with the break paragraphs where they were.
    let setup = doc.page_setup();
    assert_eq!(setup.sections.len(), 3);
    assert_eq!(setup.sections[1].cols.num, 2);
    assert_eq!(setup.sections[0].end, Some(ps[3]));
    assert!(t.iter().filter(|x| x.starts_with("Cherry")).count() == 1);
    assert_valid(&doc.save().unwrap());
    // Without a pages map the page numbers are estimated from page breaks.
    ops(&mut doc, json!({"op":"insert_page_break","at":{"p":ps[0],"o":21}}));
    ops(&mut doc, json!({"op":"update_fields"}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x == "Banana\t2"), "{t:?}");
}

#[test]
fn word_made_index_is_read_and_rebuilt_with_its_section_break() {
    // Word's shape: XE fields in the text, and an INDEX field in a continuous two-column section whose
    // section-break paragraph also holds the field's end. The tab of `\e "<tab>"` is a tab run between
    // two instruction runs.
    let xe = |entry: &str| format!(r#"<w:r><w:rPr><w:vanish/></w:rPr><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:instrText xml:space="preserve"> XE "{entry}" </w:instrText></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:fldChar w:fldCharType="end"/></w:r>"#);
    let body = format!(
        r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:cols w:space="720"/></w:sectPr></w:pPr><w:r><w:t>Intro</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">Mango </w:t></w:r>{}</w:p><w:p><w:r><w:t xml:space="preserve">Kiwi </w:t></w:r>{}</w:p><w:p><w:pPr><w:pStyle w:val="Index1"/><w:tabs><w:tab w:val="right" w:leader="dot" w:pos="4310"/></w:tabs></w:pPr><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> INDEX \e "</w:instrText></w:r><w:r><w:tab/></w:r><w:r><w:instrText xml:space="preserve">" \c "2" \z "1033" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Stale</w:t></w:r><w:r><w:tab/></w:r><w:r><w:t>99</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Index1"/><w:sectPr><w:type w:val="continuous"/><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/><w:cols w:num="2" w:space="720"/></w:sectPr></w:pPr><w:r><w:t>Old entry</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p><w:p><w:r><w:t>After the index.</w:t></w:r></w:p>"#,
        xe("Mango"),
        xe("Kiwi")
    );
    let mut doc = doc_from_body(&body);
    // The XE fields are invisible and the paragraph text is just the words.
    assert_eq!(texts(&doc)[1], "Mango ");
    let setup = doc.page_setup();
    assert_eq!(setup.sections.len(), 3);
    let mango = find_p(&doc, "Mango");
    let kiwi = find_p(&doc, "Kiwi");
    let r = ops(&mut doc, json!({"op":"update_fields","pages":pages(&[(mango, 3), (kiwi, 5)])}));
    assert_eq!(r[0].count, Some(1));
    let t = texts(&doc);
    assert!(t.contains(&"Kiwi\t5".to_string()) && t.contains(&"Mango\t3".to_string()), "{t:?}");
    assert!(!t.iter().any(|x| x.contains("Stale") || x.contains("Old entry")));
    // The index is Kiwi, Mango in order, and the section structure survived: the last index paragraph still
    // carries the continuous two-column section properties.
    let i = t.iter().position(|x| x == "Kiwi\t5").unwrap();
    assert_eq!(t[i + 1], "Mango\t3");
    let setup = doc.page_setup();
    assert_eq!(setup.sections.len(), 3);
    assert_eq!(setup.sections[1].cols.num, 2);
    assert_eq!(setup.sections[1].start, "continuous");
    assert!(t.last().unwrap().contains("After the index."));
    // The instruction is kept, tab and all.
    let x = body_xml(&doc);
    assert!(x.contains("INDEX \\e \"\t\" \\c \"2\" \\z \"1033\""), "{x}");
    assert_valid(&doc.save().unwrap());
}
