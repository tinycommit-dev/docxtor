//! Tests of: Other shapes Word and other tools write and editing around citations.

use super::*;

pub(super) fn cite_para(tag: &str, result: &str) -> String {
    format!(r#"<w:p><w:r><w:t xml:space="preserve">As shown </w:t></w:r>{}<w:r><w:t>.</w:t></w:r></w:p>"#, field(&format!("CITATION {tag} \\l 1033"), result, 4242))
}

pub(super) fn run_xml(inner: &str) -> String {
    format!("<w:r>{inner}</w:r>")
}

pub(super) fn fld(kind: &str) -> String {
    run_xml(&format!(r#"<w:fldChar w:fldCharType="{kind}"/>"#))
}

pub(super) fn instr(text: &str) -> String {
    run_xml(&format!(r#"<w:instrText xml:space="preserve"> {text} </w:instrText>"#))
}

pub(super) fn plain_run(text: &str) -> String {
    run_xml(&format!(r#"<w:t xml:space="preserve">{text}</w:t>"#))
}

#[test]
fn a_bibliography_field_that_ends_inside_its_last_entry_is_rewritten() {
    // Some writers put the end of the field in the last entry's paragraph rather than a paragraph of its own.
    let body = format!(
        "{}<w:p>{}{}{}{}</w:p><w:p>{}{}</w:p><w:p><w:r><w:t>After.</w:t></w:r></w:p>",
        cite_para("Smi20", "(Smith, 2020)"),
        fld("begin"),
        instr("BIBLIOGRAPHY"),
        fld("separate"),
        plain_run("Old first entry."),
        plain_run("Old second entry."),
        fld("end"),
    );
    let (mut doc, _) = word_package(&body);
    ops(&mut doc, json!({"op":"update_bibliography"}));
    assert_eq!(texts(&doc), ["As shown (Smith, 2020).", "Smith, J. (2020). The Art of Citing. Penguin.", "", "After."]);
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert_eq!(xml.matches("fldCharType=\"begin\"").count(), 2);
    assert_eq!(xml.matches("fldCharType=\"end\"").count(), 2);
}

#[test]
fn a_bibliography_in_one_paragraph_and_without_a_control_is_rewritten() {
    let body = format!("{}<w:p>{}{}{}{}{}</w:p>", cite_para("Lee21", "(Lee &amp; Kim, 2021)"), fld("begin"), instr("BIBLIOGRAPHY \\l 1033"), fld("separate"), plain_run("stale"), fld("end"));
    let (mut doc, _) = word_package(&body);
    ops(&mut doc, json!({"op":"update_bibliography"}));
    let t = texts(&doc);
    assert_eq!(t.len(), 3, "{t:?}");
    assert!(t[1].starts_with("Lee, K., & Kim, M.-J. (2021). A study of things. Journal of Stuff, 12(3), 45"), "{t:?}");
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"BIBLIOGRAPHY \l 1033"#), "the instruction is kept as it was");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn a_citation_of_a_source_that_is_not_in_the_list_is_left_alone() {
    let (mut doc, _) = word_package(&format!("{}{}", cite_para("Nope99", "(Nobody, 1999)"), cite_para("Smi20", "(Old, 1)")));
    ops(&mut doc, json!({"op":"update_bibliography"}));
    assert_eq!(texts(&doc), ["As shown (Nobody, 1999).", "As shown (Smith, 2020)."]);
    // And it can't be cited by hand either.
    let p = find_p(&doc, "As shown");
    assert!(err(&mut doc, json!({"op":"insert_citation","at":{"p":p,"o":0},"tags":["Nope99"]})).contains("no source with the tag"));
}

#[test]
fn citations_in_tables_and_notes_follow_the_style() {
    let cell = format!(r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr>{}</w:tc></w:tr></w:tbl><w:p/>"#, cite_para("Smi20", "(Smith, 2020)"));
    let (mut doc, _) = word_package(&cell);
    // A footnote with a citation (Word-style), added by editing the saved package.
    let p = find_p(&doc, "As shown");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":3},"text":"Note text"}));
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    let notes = part(&pkg.to_bytes().unwrap(), "word/footnotes.xml");
    // The citation goes at the end of the last note's paragraph.
    let at = notes.rfind("</w:p></w:footnote>").expect("a footnote");
    let patched = format!("{}{}{}", &notes[..at], field("CITATION Lee21 \\l 1033", "(old)", 777), &notes[at..]);
    pkg.set("word/footnotes.xml", patched.into_bytes());
    let before = pkg.to_bytes().unwrap();
    let mut doc = Document::open(&before).unwrap();
    assert_eq!(doc.citations().len(), 2, "the body's and the note's");
    ops(&mut doc, json!({"op":"set_citation_style","style":"mla"}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert!(part(&out, "word/footnotes.xml").contains("(Lee and Kim)"), "{}", part(&out, "word/footnotes.xml"));
    assert!(part(&out, "word/document.xml").contains("(Smith)"));
    // Changing the style rewrote the note's part and the body, and left other parts alone.
    let (a, b) = (Package::open(&before).unwrap(), Package::open(&out).unwrap());
    for p in &a.parts {
        if !matches!(p.name.as_str(), "word/footnotes.xml" | "word/document.xml" | "customXml/item1.xml") {
            assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{} changed", p.name);
        }
    }
    // A table cell's citation was found too.
    assert!(doc.citations().iter().any(|c| c.tags == ["Smi20"]));
}

#[test]
fn source_types_word_has_but_docxtor_lacks_survive_an_update() {
    let sources = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?><b:Sources SelectedStyle="\ISO690.XSL" StyleName="ISO 690 - First Element and Date" Version="6" xmlns:b="http://schemas.openxmlformats.org/officeDocument/2006/bibliography" xmlns="http://schemas.openxmlformats.org/officeDocument/2006/bibliography"><b:Source><b:Tag>Mag99</b:Tag><b:SourceType>ArticleInAPeriodical</b:SourceType><b:Guid>{11111111-2222-3333-4444-555555555555}</b:Guid><b:Title>Cover story</b:Title><b:PeriodicalTitle>The Weekly</b:PeriodicalTitle><b:Year>1999</b:Year><b:Pages>12</b:Pages><b:Author><b:Author><b:NameList><b:Person><b:Last>Roe</b:Last><b:First>Jane</b:First></b:Person></b:NameList></b:Author><b:Translator><b:NameList><b:Person><b:Last>Tr</b:Last></b:Person></b:NameList></b:Translator></b:Author><b:Comments>keep me</b:Comments><b:RefOrder>1</b:RefOrder></b:Source><b:Source><b:Tag>Mov01</b:Tag><b:SourceType>Film</b:SourceType><b:Guid>{99999999-2222-3333-4444-555555555555}</b:Guid><b:Title>The Film</b:Title><b:Year>2001</b:Year><b:Director><b:NameList><b:Person><b:Last>Dir</b:Last></b:Person></b:NameList></b:Director><b:RefOrder>2</b:RefOrder></b:Source></b:Sources>"#;
    let (mut doc, _) = word_package_with(&cite_para("Mag99", "(Roe, 1999)"), sources);
    let s = doc.sources();
    assert_eq!((s[0].kind.id(), s[0].container_title.as_deref()), ("article", Some("The Weekly")));
    assert_eq!(s[1].kind.id(), "misc");
    // An unknown Word style (ISO 690) reads as the nearest of ours.
    assert_eq!(doc.citation_style(), "harvard");
    let mut edited = s[0].clone();
    edited.pages = Some("12-13".into());
    ops(&mut doc, json!({"op":"update_source","tag":"Mag99","source":edited}));
    ops(&mut doc, json!({"op":"update_source","tag":"Mov01","source":{"type":"misc","title":"The Film (restored)","year":"2001"}}));
    let item = part(&doc.save().unwrap(), "customXml/item1.xml");
    assert!(item.contains("<b:SourceType>ArticleInAPeriodical</b:SourceType>"), "the exact Word type is kept: {item}");
    assert!(item.contains("<b:PeriodicalTitle>The Weekly</b:PeriodicalTitle>") && item.contains("<b:Pages>12-13</b:Pages>"), "{item}");
    assert!(item.contains("<b:Translator>") && item.contains("<b:Comments>keep me</b:Comments>"), "{item}");
    assert!(item.contains("<b:SourceType>Film</b:SourceType>") && item.contains("<b:Director>"), "{item}");
    assert!(item.contains("<b:Title>The Film (restored)</b:Title>"), "{item}");
    assert!(item.contains(r#"SelectedStyle="\ISO690.XSL""#), "the style is untouched until chosen");
}

#[test]
fn a_source_is_json_the_editor_can_read_and_write() {
    let (doc, _) = word_made();
    let v = serde_json::to_value(doc.sources()).unwrap();
    assert_eq!(
        v[1],
        json!({"tag":"Lee21","type":"article","authors":[{"family":"Lee","given":"Karen"},{"family":"Kim","given":"Min-Jun"}],"title":"A study of things","containerTitle":"Journal of Stuff","year":"2021","month":"March","volume":"12","issue":"3","pages":"45-67","guid":"{0C3A1B6E-55A2-4E39-9C3B-3E2B1D2F8A22}"})
    );
    // Input is forgiving: numbers for years and pages, strings for names, type aliases.
    let mut doc = doc_with("Hello.");
    ops(&mut doc, json!({"op":"add_source","source":{"type":"journal_article","authors":["Roe, Jane",{"literal":"The Lab"}],"title":"X","year":1999,"volume":3,"pages":"1-2"}}));
    let s = &doc.sources()[0];
    assert_eq!((s.kind.id(), s.year.as_deref(), s.volume.as_deref()), ("article", Some("1999"), Some("3")));
    assert_eq!(s.authors.len(), 2);
    let e = doc.apply_json(r#"{"op":"add_source","source":{"authors":[{"given":"x"}],"title":"X"}}"#, false).unwrap_err().to_string();
    assert!(e.contains("family name"), "{e}");
}

#[test]
fn updating_a_word_made_citation_after_a_source_edit_keeps_the_markup() {
    let (mut doc, bytes) = word_made();
    let mut s = doc.sources()[0].clone();
    s.publisher = Some("Penguin Books".into());
    ops(&mut doc, json!({"op":"update_source","tag":"Smi20","source":s}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    // Only the sources part and the document changed; the citations are still Word's content controls.
    let (a, b) = (Package::open(&bytes).unwrap(), Package::open(&out).unwrap());
    for p in &a.parts {
        if !matches!(p.name.as_str(), "word/document.xml" | "customXml/item1.xml") {
            assert_eq!(b.get(&p.name), Some(p.data.as_slice()), "{} changed", p.name);
        }
    }
    let xml = part(&out, "word/document.xml");
    assert!(xml.contains(r#"<w:sdt><w:sdtPr><w:id w:val="-1234567"/><w:citation/></w:sdtPr>"#), "{xml}");
    assert!(xml.contains("Penguin Books"), "the bibliography entry changed");
    let item = part(&out, "customXml/item1.xml");
    assert!(item.contains("<b:Publisher>Penguin Books</b:Publisher>") && item.contains("<b:LCID>1033</b:LCID>"), "{item}");
}

#[test]
fn typing_next_to_a_citation_stays_outside_it() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let c = doc.citations()[0].clone();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":c.p,"o":c.o+c.len},"text":", yes"}));
    ops(&mut doc, json!({"op":"insert_text","at":{"p":c.p,"o":c.o},"text":" so"}));
    assert_eq!(texts(&doc)[0], "Citing is hard so(Smith, 2020), yes here.");
    let after = doc.citations()[0].clone();
    assert_eq!((after.o, after.len), (c.o + 3, c.len), "the citation is the same size, moved by what was typed before it");
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert!(!xml.contains("<w:noProof/></w:rPr><w:t xml:space=\"preserve\">, yes"), "typed text doesn't inherit the citation's formatting: {xml}");
    // The text that follows is still outside the control.
    let sdt_end = xml.find("</w:sdt>").unwrap();
    assert!(xml[sdt_end..].contains(", yes"), "{xml}");
    // An update changes nothing about what was typed.
    ops(&mut doc, json!({"op":"update_bibliography"}));
    assert_eq!(texts(&doc)[0], "Citing is hard so(Smith, 2020), yes here.");
}

#[test]
fn deleting_a_citation_removes_it_for_good() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let c = doc.citations()[0].clone();
    ops(&mut doc, json!({"op":"delete","from":{"p":c.p,"o":c.o},"to":{"p":c.p,"o":c.o+c.len}}));
    assert_eq!(texts(&doc)[0], "Citing is hard here.");
    assert!(doc.citations().is_empty());
    ops(&mut doc, json!({"op":"update_bibliography"}));
    assert_eq!(texts(&doc)[0], "Citing is hard here.", "the citation does not come back");
    assert!(!part(&doc.save().unwrap(), "word/document.xml").contains("CITATION"));
    // Selecting across it works the same.
    assert!(doc.undo() && doc.undo());
    let c = doc.citations()[0].clone();
    ops(&mut doc, json!({"op":"delete","from":{"p":c.p,"o":c.o-3},"to":{"p":c.p,"o":c.o+c.len+2}}));
    assert_eq!(texts(&doc)[0], "Citing is here.");
    assert!(doc.citations().is_empty());
}

#[test]
fn enter_before_or_after_a_citation_keeps_it_whole() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let c = doc.citations()[0].clone();
    ops(&mut doc, json!({"op":"split_paragraph","at":{"p":c.p,"o":c.o+c.len}}));
    assert_eq!(texts(&doc)[..2], ["Citing is hard(Smith, 2020)", " here."]);
    assert_eq!(doc.citations().len(), 1);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn the_view_model_follows_citation_ops_and_undo() {
    let mut doc = cited_doc();
    doc.render_all();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let r = doc.render_changed();
    let c = doc.citations()[0].clone();
    let para = r.blocks.iter().find_map(|b| match b {
        RBlock::Para(p) if p.id == c.p => Some(p),
        _ => None,
    });
    let para = para.expect("the paragraph with the new citation is re-rendered");
    assert!(para.segs.iter().any(|s| s.cite.as_deref() == Some(&["Smi20".to_string()][..]) && s.x.as_deref() == Some("(Smith, 2020)")));
    assert_eq!(para.len, "Citing is hard(Smith, 2020) here.".len());
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    let r = doc.render_changed();
    let bib = r.blocks.iter().find_map(|b| match b {
        RBlock::Container { label, blocks, .. } => Some((label.clone(), blocks.len())),
        _ => None,
    });
    assert_eq!(bib, Some((Some("Bibliography".to_string()), 3)));
    assert_eq!(r.order.len(), 3, "two paragraphs and the bibliography");
    // Adding a source and changing the style are felt where they show.
    ops(&mut doc, json!({"op":"set_citation_style","style":"ieee"}));
    let r = doc.render_changed();
    let xs: Vec<String> = r.blocks.iter().filter_map(|b| if let RBlock::Para(p) = b { p.segs.iter().find(|s| s.cite.is_some()).and_then(|s| s.x.clone()) } else { None }).collect();
    assert_eq!(xs, ["[1]"]);
    assert!(doc.undo());
    let r = doc.render_changed();
    assert!(r.blocks.iter().any(|b| matches!(b, RBlock::Container { .. })), "the bibliography is re-rendered after undo");
    assert!(doc.undo());
    let r = doc.render_changed();
    assert_eq!(r.order.len(), 2, "the bibliography is gone");
}

#[test]
fn update_fields_also_writes_citations_and_the_bibliography_again() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    ops(&mut doc, json!({"op":"insert_bibliography","heading":"Sources"}));
    let r = doc.apply_json(r#"{"op":"update_fields"}"#, false).unwrap();
    assert!(r[0].count.unwrap_or(0) >= 2, "the citation and the bibliography: {:?}", r[0].count);
    assert_eq!(bib_texts(&doc)[1], "Smith, J. (2020). The Art of Citing. Penguin.");
}
