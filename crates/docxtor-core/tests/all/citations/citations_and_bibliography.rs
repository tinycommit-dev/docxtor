//! Tests of: Citations and the bibliography.

use super::*;

#[test]
fn a_citation_is_written_as_word_writes_it() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    assert_eq!(texts(&doc)[0], "Citing is hard(Smith, 2020) here.");
    let out = doc.save().unwrap();
    assert_valid(&out);
    let xml = part(&out, "word/document.xml");
    let start = xml.find("<w:sdt>").unwrap();
    let end = xml.find("</w:sdt>").unwrap() + "</w:sdt>".len();
    let sdt = &xml[start..end];
    assert!(sdt.starts_with("<w:sdt><w:sdtPr><w:id w:val=\""), "{sdt}");
    assert!(sdt.contains("</w:sdtPr><w:sdtContent>"), "{sdt}");
    assert!(sdt.contains("<w:citation/></w:sdtPr>"), "{sdt}");
    assert!(sdt.contains(r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> CITATION Smi20 \l 1033 </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>"#), "{sdt}");
    assert!(sdt.contains("(Smith, 2020)") && sdt.contains("<w:noProof/>"), "{sdt}");
    assert!(sdt.ends_with(r#"<w:r><w:fldChar w:fldCharType="end"/></w:r></w:sdtContent></w:sdt>"#), "{sdt}");
    // The text around it is split in place, in order.
    assert!(xml.find("Citing is hard").unwrap() < start && xml.find(" here.").unwrap() > end);
    let id: i64 = sdt.split("w:val=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap();
    assert!(id > 0);
    // A second citation gets another id.
    cite(&mut doc, "Second paragraph", "things", &["Lee21"], Some("45-67"));
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert!(xml.contains(r#" CITATION Lee21 \p 45-67 \l 1033 "#), "{xml}");
    assert_eq!(texts(&doc)[1], "Second paragraph about things(Lee & Kim, 2021, pp. 45\u{2013}67).");
    let ids: Vec<&str> = xml.split("<w:id w:val=\"").skip(1).map(|s| s.split('"').next().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn several_sources_page_and_suppressed_parts_make_the_switches_word_uses() {
    let mut doc = cited_doc();
    let p = find_p(&doc, "Citing is hard");
    ops(&mut doc, json!({"op":"insert_citation","at":{"p":p,"o":0},"tags":["Smi20","Lee21"],"page":"23","suppress_author":true}));
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"> CITATION Smi20 \m Lee21 \n \p 23 \l 1033 <"#), "{xml}");
    let c = &doc.citations()[0];
    assert!(c.suppress_author && !c.suppress_year);
    // The switches apply to every source in the citation.
    assert_eq!(texts(&doc)[0], "(2020, p. 23; 2021)Citing is hard here.");
}

#[test]
fn citing_a_missing_source_or_inside_a_citation_is_an_error() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let p = find_p(&doc, "Citing is hard");
    assert!(err(&mut doc, json!({"op":"insert_citation","at":{"p":p,"o":14},"tags":["Nope"]})).contains("no source with the tag 'Nope'"));
    assert!(err(&mut doc, json!({"op":"insert_citation","at":{"p":p,"o":20},"tags":["Lee21"]})).contains("inside another citation"));
    assert!(err(&mut doc, json!({"op":"insert_citation","at":{"p":p,"o":0},"tags":[]})).contains("at least one source"));
    assert_eq!(doc.citations().len(), 1, "nothing was inserted");
}

#[test]
fn editing_a_citation_changes_its_sources_and_page() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let c = doc.citations()[0].clone();
    // A position inside the citation, and one right after it, both find it.
    ops(&mut doc, json!({"op":"edit_citation","at":{"p":c.p,"o":c.o+3},"page":"7"}));
    assert_eq!(texts(&doc)[0], "Citing is hard(Smith, 2020, p. 7) here.");
    ops(&mut doc, json!({"op":"edit_citation","at":{"p":c.p,"o":c.o+"(Smith, 2020, p. 7)".len()},"tags":["Smi20","Lee21"],"suppress_year":true}));
    assert_eq!(texts(&doc)[0], "Citing is hard(Smith, p. 7; Lee & Kim) here.");
    ops(&mut doc, json!({"op":"edit_citation","at":{"p":c.p,"o":c.o},"page":"","suppress_year":false}));
    assert_eq!(texts(&doc)[0], "Citing is hard(Smith, 2020; Lee & Kim, 2021) here.");
    assert_eq!(doc.citations()[0].page, None);
    assert!(err(&mut doc, json!({"op":"edit_citation","at":{"p":c.p,"o":2},"page":"1"})).contains("no citation at that position"));
    assert!(err(&mut doc, json!({"op":"edit_citation","at":{"p":c.p,"o":c.o+1},"tags":["Nope"]})).contains("no source with the tag"));
}

#[test]
fn citations_in_notes_are_updated_but_not_inserted_there() {
    let mut doc = cited_doc();
    let p = find_p(&doc, "Citing is hard");
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":6},"text":"See sources."}));
    let note = doc.stories.iter().find(|s| s.kind == docxtor_core::stories::StoryKind::Footnote).unwrap();
    let np = paragraph_order(&note.blocks)[0];
    let e = err(&mut doc, json!({"op":"insert_citation","at":{"p":np,"o":4},"tags":["Smi20"]}));
    assert!(e.contains("main text"), "{e}");
}

#[test]
fn ieee_numbers_follow_first_citation_and_renumber_when_one_is_inserted_earlier() {
    let mut doc = cited_doc();
    ops(&mut doc, json!({"op":"set_citation_style","style":"ieee"}));
    cite(&mut doc, "Second paragraph", "things", &["Smi20"], None);
    assert_eq!(texts(&doc)[1], "Second paragraph about things[1].");
    cite(&mut doc, "Citing is hard", "hard", &["Lee21"], Some("9"));
    // Lee21 is now cited first in the text.
    assert_eq!(texts(&doc)[0], "Citing is hard[1, p. 9] here.");
    assert_eq!(texts(&doc)[1], "Second paragraph about things[2].");
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    let b = bib_texts(&doc);
    assert_eq!(b[1], "[1]\tK. Lee and M.-J. Kim, \"A study of things,\" Journal of Stuff, vol. 12, no. 3, pp. 45\u{2013}67, Mar. 2021, doi: 10.1000/xyz123.");
    assert_eq!(b[2], "[2]\tJ. Smith, The Art of Citing. New York: Penguin, 2020.");
}

#[test]
fn same_author_same_year_gets_letters_in_citations_and_the_list() {
    let mut doc = doc_with("One.\n\nTwo.");
    add(&mut doc, json!({"type":"book","authors":["Smith, John"],"title":"Zebras","year":"2020","publisher":"P"}));
    add(&mut doc, json!({"type":"book","authors":["Smith, John"],"title":"Aardvarks","year":"2020","publisher":"P"}));
    cite(&mut doc, "One.", "One", &["Smi20"], None);
    cite(&mut doc, "Two.", "Two", &["Smi20a"], None);
    assert_eq!(texts(&doc)[0], "One(Smith, 2020b).");
    assert_eq!(texts(&doc)[1], "Two(Smith, 2020a).");
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    let b = bib_texts(&doc);
    assert_eq!(b[1], "Smith, J. (2020a). Aardvarks. P.");
    assert_eq!(b[2], "Smith, J. (2020b). Zebras. P.");
}

#[test]
fn a_bibliography_is_written_as_word_writes_it() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    cite(&mut doc, "Second paragraph", "things", &["Lee21"], None);
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    let b = bib_texts(&doc);
    assert_eq!(
        b,
        [
            "References",
            "Lee, K., & Kim, M.-J. (2021). A study of things. Journal of Stuff, 12(3), 45\u{2013}67. https://doi.org/10.1000/xyz123",
            "Smith, J. (2020). The Art of Citing. Penguin.",
            "",
        ]
    );
    let out = doc.save().unwrap();
    assert_valid(&out);
    let xml = part(&out, "word/document.xml");
    // The bibliography is the control with the document part gallery.
    let bib = &xml[xml[..xml.find("docPartObj").unwrap()].rfind("<w:sdt>").unwrap()..];
    assert!(bib.starts_with("<w:sdt><w:sdtPr><w:id w:val=\""), "{bib}");
    assert!(bib.contains(r#"<w:docPartObj><w:docPartGallery w:val="Bibliographies"/><w:docPartUnique/></w:docPartObj></w:sdtPr><w:sdtContent>"#), "{bib}");
    assert!(bib.contains(r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t xml:space="preserve">References</w:t></w:r></w:p>"#), "{bib}");
    assert!(bib.contains(r#"<w:pPr><w:pStyle w:val="Bibliography"/><w:ind w:left="720" w:hanging="720"/><w:rPr><w:noProof/></w:rPr></w:pPr><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> BIBLIOGRAPHY </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>"#), "{bib}");
    assert!(bib.contains("<w:i/><w:iCs/>"), "italic journal name");
    assert!(bib.contains(r#"<w:r><w:fldChar w:fldCharType="end"/></w:r></w:p></w:sdtContent></w:sdt>"#), "{bib}");
    // The Bibliography style exists (built-in), and so does Heading 1.
    let styles = part(&out, "word/styles.xml");
    assert!(styles.contains(r#"w:styleId="Bibliography""#) && styles.contains(r#"w:styleId="Heading1""#), "styles");
    // Rendered as a container labelled Bibliography.
    let r = doc.render_all();
    assert!(r.blocks.iter().any(|b| matches!(b, RBlock::Container { label: Some(l), .. } if l == "Bibliography")));
    // Reopened, it is the same.
    let again = Document::open(&out).unwrap();
    assert_eq!(bib_texts(&again), b);
}

#[test]
fn the_bibliography_lists_only_cited_sources_and_follows_changes() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    ops(&mut doc, json!({"op":"insert_bibliography","heading":"Sources"}));
    assert_eq!(bib_texts(&doc), ["Sources", "Smith, J. (2020). The Art of Citing. Penguin.", ""]);
    // A second citation adds its entry; no manual update is needed.
    cite(&mut doc, "Second paragraph", "things", &["Lee21"], None);
    assert_eq!(bib_texts(&doc).len(), 4);
    // Editing a source updates its entry.
    let mut s = doc.sources()[1].clone();
    s.title = "A study of other things".into();
    ops(&mut doc, json!({"op":"update_source","tag":"Lee21","source":s}));
    assert!(bib_texts(&doc)[1].contains("A study of other things"), "{:?}", bib_texts(&doc));
    // Only one bibliography.
    assert!(err(&mut doc, json!({"op":"insert_bibliography"})).contains("already has a bibliography"));
    // update_bibliography rewrites everything and says how many fields.
    let r = doc.apply_json(r#"{"op":"update_bibliography"}"#, false).unwrap();
    assert_eq!(r[0].count, Some(3), "two citations and the bibliography");
}

#[test]
fn a_bibliography_goes_where_asked() {
    let mut doc = doc_with("# Intro\n\nBody.\n\n# End");
    add(&mut doc, smith());
    let intro = find_p(&doc, "Intro");
    ops(&mut doc, json!({"op":"insert_bibliography","after":intro,"heading":""}));
    let t = texts(&doc);
    assert_eq!(t[0], "Intro");
    assert_eq!((t[1].as_str(), t[2].as_str()), ("", ""), "no heading, no entries: the empty field's two paragraphs");
    assert_eq!(t[3], "Body.");
    let mut doc = doc_with("A.\n\nB.");
    add(&mut doc, smith());
    let b = find_p(&doc, "B.");
    ops(&mut doc, json!({"op":"insert_bibliography","before":b}));
    assert_eq!(texts(&doc)[1], "References");
    // At the end by default.
    let mut doc = doc_with("A.\n\nB.");
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    assert_eq!(texts(&doc).first().map(String::as_str), Some("A."));
    assert!(texts(&doc).contains(&"References".to_string()));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn switching_style_renames_the_default_heading_only() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    ops(&mut doc, json!({"op":"set_citation_style","style":"mla"}));
    assert_eq!(bib_texts(&doc)[0], "Works Cited");
    assert_eq!(bib_texts(&doc)[1], "Smith, John. The Art of Citing. Penguin, 2020.");
    ops(&mut doc, json!({"op":"set_citation_style","style":"chicago"}));
    assert_eq!(bib_texts(&doc)[0], "Bibliography");
    // A heading the author wrote is theirs.
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    ops(&mut doc, json!({"op":"insert_bibliography","heading":"My reading"}));
    ops(&mut doc, json!({"op":"set_citation_style","style":"mla"}));
    assert_eq!(bib_texts(&doc)[0], "My reading");
    assert!(err(&mut doc, json!({"op":"set_citation_style","style":"vancouver"})).contains("apa, mla, chicago, ieee or harvard"));
}

#[test]
fn ieee_entries_hang_from_the_number() {
    let mut doc = cited_doc();
    ops(&mut doc, json!({"op":"set_citation_style","style":"ieee"}));
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    let xml = part(&doc.save().unwrap(), "word/document.xml");
    assert!(xml.contains(r#"<w:ind w:left="540" w:hanging="540"/>"#), "{xml}");
    assert!(xml.contains("[1]</w:t></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:tab/>") || xml.contains("<w:tab/>"), "the number is followed by a tab");
}
