//! CSL styles in a document: choosing one, numeric citations, Chicago notes (citations as footnotes, short
//! forms, "Ibid."), switching between notes and in-text citations, and what is saved for Word.

use docxtor_core::model::{walk_paragraphs, Paragraph};
use docxtor_core::package::Package;
use docxtor_core::stories::StoryKind;
use docxtor_core::Document;
use serde_json::{json, Value};

fn csl(file: &str) -> String {
    let path = format!("{}/../../web/src/csl/{file}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn err(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).expect_err(&json.to_string()).to_string()
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn smith() -> Value {
    json!({"type":"book","authors":[{"family":"Smith","given":"John"}],"title":"The Art of Citing: A Guide","year":2020,"publisher":"Penguin","place":"New York"})
}

fn lee() -> Value {
    json!({"type":"article","authors":[{"family":"Lee","given":"Karen"},{"family":"Kim","given":"Min-Jun"}],"title":"A study of things","containerTitle":"Journal of Stuff","year":"2021","month":"3","volume":"12","issue":"3","pages":"45-67","doi":"10.1000/xyz123"})
}

fn add(doc: &mut Document, source: Value) -> String {
    let before = doc.sources().len();
    ops(doc, json!({"op":"add_source","source":source}));
    doc.sources()[before].tag.clone()
}

/// Two paragraphs, two sources and the styles registered.
fn start() -> Document {
    let mut doc = doc_with("Citing is hard here.\n\nSecond paragraph about things.");
    add(&mut doc, smith());
    add(&mut doc, lee());
    for (id, file) in [("vancouver", "elsevier-vancouver.csl"), ("chicago-nb", "chicago-notes-bibliography-16th-edition.csl"), ("nature", "nature.csl")] {
        doc.register_citation_style(id, &csl(file)).unwrap();
    }
    doc
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

fn cite(doc: &mut Document, para_text: &str, after: &str, tags: &[&str], page: Option<&str>) {
    let p = find_p(doc, para_text);
    let hay = doc.find_any_para(p).unwrap().plain_text();
    let o = hay.encode_utf16().count() - hay[hay.find(after).unwrap()..].encode_utf16().count() + after.encode_utf16().count();
    let mut op = json!({"op":"insert_citation","at":{"p":p,"o":o},"tags":tags});
    if let Some(pg) = page {
        op["page"] = json!(pg);
    }
    ops(doc, op);
}

/// Cite at the very end of a paragraph, after anything already there.
fn cite_end(doc: &mut Document, para_text: &str, tags: &[&str], page: Option<&str>) {
    let p = find_p(doc, para_text);
    let o = doc.find_any_para(p).unwrap().len16();
    let mut op = json!({"op":"insert_citation","at":{"p":p,"o":o},"tags":tags});
    if let Some(pg) = page {
        op["page"] = json!(pg);
    }
    ops(doc, op);
}

fn body_texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p: &Paragraph| v.push(p.plain_text()));
    v
}

fn collect_text(v: &Value, out: &mut String) {
    match v {
        Value::Object(o) => {
            if o.get("k").and_then(|k| k.as_str()) == Some("t") {
                if let Some(x) = o.get("x").and_then(|x| x.as_str()) {
                    out.push_str(x);
                }
            }
            for c in o.values() {
                collect_text(c, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_text(c, out)),
        _ => {}
    }
}

/// The text of the footnotes, in the order of their reference in the text.
fn notes(doc: &Document) -> Vec<String> {
    let v = serde_json::to_value(doc.page_setup().notes).unwrap();
    v.as_array()
        .unwrap()
        .iter()
        .map(|n| {
            let mut t = String::new();
            collect_text(n, &mut t);
            t.trim().to_string()
        })
        .collect()
}

/// How many footnotes the file holds, whatever refers to them.
#[allow(dead_code)]
fn note_stories(doc: &Document) -> usize {
    doc.stories.iter().filter(|s| s.kind == StoryKind::Footnote).count()
}

fn bib_texts(doc: &Document) -> Vec<String> {
    for b in &doc.body {
        if let docxtor_core::model::Block::Container(c) = b {
            let mut v = Vec::new();
            walk_paragraphs(&c.blocks, &mut |p| v.push(p.plain_text()));
            return v;
        }
    }
    panic!("no bibliography");
}

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = docxtor_core::xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            docxtor_core::xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn sources_part(bytes: &[u8]) -> String {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.starts_with("customXml/item") && !part.name.contains("Props") {
            return String::from_utf8(part.data.clone()).unwrap();
        }
    }
    panic!("no sources part");
}

#[test]
fn registering_styles() {
    let mut doc = Document::new_blank();
    let info = doc.register_citation_style("nature", &csl("nature.csl")).unwrap();
    assert_eq!((info.id.as_str(), info.title.as_str(), info.note, info.numeric), ("nature", "Nature", false, true));
    let info = doc.register_citation_style("chicago-nb", &csl("chicago-notes-bibliography-16th-edition.csl")).unwrap();
    assert!(info.note && !info.numeric);
    assert_eq!(doc.citation_styles().iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["chicago-nb", "nature"]);
    assert!(doc.register_citation_style("apa", &csl("nature.csl")).unwrap_err().contains("one of Docxtor's own"));
    assert!(doc.register_citation_style("two words", &csl("nature.csl")).is_err());
    assert!(doc.register_citation_style("x", "<html/>").unwrap_err().contains("not a CSL style"));
    // Asking for a style nobody added names the ones that exist.
    let e = err(&mut doc, json!({"op":"set_citation_style","style":"vancouver"}));
    assert!(e.contains("apa, mla, chicago, ieee or harvard") && e.contains("chicago-nb, nature"), "{e}");
}

#[test]
fn a_numeric_csl_style_rewrites_citations_and_the_bibliography() {
    let mut doc = start();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    cite(&mut doc, "Second paragraph", "Second", &["Lee21", "Smi20"], Some("5"));
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    assert_eq!(doc.citation_style(), "apa");
    ops(&mut doc, json!({"op":"set_citation_style","style":"vancouver"}));
    assert_eq!(doc.citation_style(), "vancouver");
    let t = body_texts(&doc);
    assert_eq!(t[0], "Citing is hard[1] here.");
    assert_eq!(t[1], "Second[1,2] paragraph about things.");
    let b = bib_texts(&doc);
    assert_eq!(b[0], "References");
    assert_eq!(b[1], "[1]\tSmith J. The Art of Citing: A Guide. New York: Penguin; 2020.");
    assert!(b[2].starts_with("[2]\tLee K, Kim M-J. A study of things. Journal of Stuff 2021;12:45"), "{}", b[2]);

    // Another CSL style, with a superscript run.
    ops(&mut doc, json!({"op":"set_citation_style","style":"nature"}));
    assert_eq!(body_texts(&doc)[0], "Citing is hard1 here.");
    let p = doc.find_any_para(find_p(&doc, "Citing is hard")).unwrap().clone();
    let xml = docxtor_core::model::write_block(&docxtor_core::model::Block::Para(p)).to_xml();
    assert!(xml.contains("<w:vertAlign w:val=\"superscript\"/>"), "{xml}");

    // Back to one of the five.
    ops(&mut doc, json!({"op":"set_citation_style","style":"apa"}));
    assert_eq!(body_texts(&doc)[0], "Citing is hard(Smith, 2020) here.");
    assert_eq!(bib_texts(&doc)[1], "Lee, K., & Kim, M.-J. (2021). A study of things. Journal of Stuff, 12(3), 45\u{2013}67. https://doi.org/10.1000/xyz123");
}

#[test]
fn a_csl_style_is_saved_in_a_way_word_can_read() {
    let mut doc = start();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    ops(&mut doc, json!({"op":"set_citation_style","style":"vancouver"}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let src = sources_part(&bytes);
    // Word updates the fields in the Word style closest to a numeric style, and the CSL id is kept.
    assert!(src.contains("SelectedStyle=\"\\IEEE2006OfficeOnline.xsl\"") && src.contains("StyleName=\"CSL:vancouver\""), "{src}");
    // The text is stored as text, so Word shows it as it is.
    let again = Document::open(&bytes).unwrap();
    assert_eq!(body_texts(&again)[0], "Citing is hard[1] here.");
    assert_eq!(again.citation_style(), "vancouver");
    // Opened where the style is not registered, nothing changes until the person picks a style.
    let mut again = Document::open(&bytes).unwrap();
    ops(&mut again, json!({"op":"update_bibliography"}));
    assert_eq!(body_texts(&again)[0], "Citing is hard[1] here.");
    again.register_citation_style("vancouver", &csl("elsevier-vancouver.csl")).unwrap();
    ops(&mut again, json!({"op":"update_bibliography"}));
    assert_eq!(body_texts(&again)[0], "Citing is hard[1] here.");
}

#[test]
fn chicago_notes_citations_are_footnotes() {
    let mut doc = start();
    ops(&mut doc, json!({"op":"set_citation_style","style":"chicago-nb"}));
    assert!(doc.citations_are_notes());
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], Some("23"));
    assert_eq!(body_texts(&doc)[0], "Citing is hard here.", "the text keeps only the note's mark: {:?}", body_texts(&doc));
    assert_eq!(notes(&doc), ["John Smith, The Art of Citing: A Guide (New York: Penguin, 2020), 23."]);
    assert_eq!(doc.citations().len(), 1, "the citation in the note is still a citation");

    // The same source again, after another: the short form.
    cite(&mut doc, "Second paragraph", "paragraph", &["Lee21"], None);
    cite(&mut doc, "Second paragraph", "things", &["Smi20"], Some("30"));
    let n = notes(&doc);
    assert_eq!(n.len(), 3);
    assert!(n[1].starts_with("Karen Lee and Min-Jun Kim, \u{201c}A Study of Things,\u{201d}"), "{}", n[1]);
    assert_eq!(n[2], "Smith, The Art of Citing, 30.");

    // And right after itself: Ibid.
    cite_end(&mut doc, "Second paragraph", &["Smi20"], Some("30"));
    cite_end(&mut doc, "Second paragraph", &["Smi20"], Some("31"));
    let n = notes(&doc);
    assert_eq!(n.len(), 5);
    assert_eq!(n[2], "Smith, The Art of Citing, 30.");
    assert_eq!(&n[3..], ["Ibid.", "Ibid., 31."].map(String::from));

    // The bibliography has the full entries.
    ops(&mut doc, json!({"op":"insert_bibliography"}));
    let b = bib_texts(&doc);
    assert_eq!(b[0], "Bibliography");
    assert_eq!(b[1], "Lee, Karen, and Min-Jun Kim. \u{201c}A Study of Things.\u{201d} Journal of Stuff 12, no. 3 (March 2021): 45\u{2013}67. doi:10.1000/xyz123.");
    assert_eq!(b[2], "Smith, John. The Art of Citing: A Guide. New York: Penguin, 2020.");

    // Saved, it is a footnote with a CITATION field in a content control, which Word can edit.
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let footnotes = String::from_utf8(pkg.get("word/footnotes.xml").unwrap().to_vec()).unwrap();
    assert!(footnotes.contains("<w:citation/>") && footnotes.contains("CITATION Smi20") && footnotes.contains("<w:footnoteRef/>"), "{footnotes}");
    assert!(sources_part(&bytes).contains("StyleName=\"CSL:chicago-nb\""));
    let again = Document::open(&bytes).unwrap();
    assert_eq!(notes(&again), notes(&doc));
}

#[test]
fn changing_a_citation_in_a_note_follows_through() {
    let mut doc = start();
    ops(&mut doc, json!({"op":"set_citation_style","style":"chicago-nb"}));
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    cite(&mut doc, "Second paragraph", "things", &["Smi20"], None);
    assert_eq!(notes(&doc)[1], "Ibid.", "{:?}", notes(&doc));
    let c = doc.citations();
    assert_eq!(c.len(), 2);
    // Give the second citation a page, from the note it sits in.
    ops(&mut doc, json!({"op":"edit_citation","at":{"p":c[1].p,"o":c[1].o + 1},"page":"12"}));
    assert_eq!(notes(&doc)[1], "Ibid., 12.");
    // Cite another source in the second note: the note is no longer "Ibid.".
    ops(&mut doc, json!({"op":"edit_citation","at":{"p":c[1].p,"o":c[1].o + 1},"tags":["Lee21"]}));
    assert!(notes(&doc)[1].starts_with("Karen Lee"), "{:?}", notes(&doc));
    // Undo is one step.
    assert!(doc.undo());
    assert_eq!(notes(&doc)[1], "Ibid., 12.");
}

#[test]
fn switching_between_notes_and_text_converts_in_one_step() {
    let mut doc = start();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    cite(&mut doc, "Second paragraph", "things", &["Lee21"], Some("46"));
    assert_eq!(body_texts(&doc)[0], "Citing is hard(Smith, 2020) here.");
    assert!(notes(&doc).is_empty());
    let undo_depth = doc.checkpoint();

    ops(&mut doc, json!({"op":"set_citation_style","style":"chicago-nb"}));
    assert_eq!(doc.checkpoint(), undo_depth + 1, "one undoable step");
    // The citations left the text; a note stands where each was, after the word and with no space before it.
    assert_eq!(body_texts(&doc)[0], "Citing is hard here.");
    assert_eq!(body_texts(&doc)[1], "Second paragraph about things.");
    let n = notes(&doc);
    assert_eq!(n.len(), 2);
    assert_eq!(n[0], "John Smith, The Art of Citing: A Guide (New York: Penguin, 2020).");
    assert_eq!(n[1], "Karen Lee and Min-Jun Kim, \u{201c}A Study of Things,\u{201d} Journal of Stuff 12, no. 3 (March 2021): 46, doi:10.1000/xyz123.");

    // And back: the notes go, the citations return, formatted in the new style.
    ops(&mut doc, json!({"op":"set_citation_style","style":"nature"}));
    assert!(notes(&doc).is_empty(), "{:?}", notes(&doc));
    assert_eq!(body_texts(&doc)[0], "Citing is hard1 here.");
    assert_eq!(body_texts(&doc)[1], "Second paragraph about things2.");
    assert_eq!(doc.citations().len(), 2);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let footnotes = String::from_utf8(pkg.get("word/footnotes.xml").unwrap().to_vec()).unwrap();
    assert!(!footnotes.contains("CITATION"), "{footnotes}");

    // Undo returns to the notes in one step.
    assert!(doc.undo());
    assert_eq!(notes(&doc).len(), 2);
}

#[test]
fn notes_with_other_text_are_left_alone_when_leaving_a_note_style() {
    let mut doc = start();
    ops(&mut doc, json!({"op":"set_citation_style","style":"chicago-nb"}));
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let p = find_p(&doc, "Second paragraph");
    // A note the author wrote.
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":6},"text":"See the appendix."}));
    assert_eq!(notes(&doc).len(), 2);
    ops(&mut doc, json!({"op":"set_citation_style","style":"vancouver"}));
    assert_eq!(notes(&doc), ["See the appendix."]);
    assert_eq!(body_texts(&doc)[0], "Citing is hard [1] here.");
}

#[test]
fn removing_a_citation_updates_the_rest_and_takes_its_note_along() {
    let mut doc = start();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    cite(&mut doc, "Second paragraph", "things", &["Lee21"], None);
    ops(&mut doc, json!({"op":"set_citation_style","style":"vancouver"}));
    assert_eq!(body_texts(&doc)[1], "Second paragraph about things[2].");
    let c = doc.citations();
    // Removing the first leaves the second as number 1.
    ops(&mut doc, json!({"op":"remove_citation","at":{"p":c[0].p,"o":c[0].o}}));
    assert_eq!(body_texts(&doc)[0], "Citing is hard here.");
    assert_eq!(body_texts(&doc)[1], "Second paragraph about things[1].");

    // In a note style the note goes with the citation.
    ops(&mut doc, json!({"op":"set_citation_style","style":"chicago-nb"}));
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    assert_eq!(notes(&doc).len(), 2);
    let c = doc.citations();
    let in_note = c.iter().find(|x| x.tags == ["Smi20"]).unwrap();
    ops(&mut doc, json!({"op":"remove_citation","at":{"p":in_note.p,"o":in_note.o + 1}}));
    assert_eq!(notes(&doc).len(), 1);
    assert_eq!(body_texts(&doc)[0], "Citing is hard here.");
    assert_eq!(doc.citations().len(), 1);
    let first = find_p(&doc, "Citing is hard");
    assert!(err(&mut doc, json!({"op":"remove_citation","at":{"p":first,"o":2}})).contains("no citation"));
    // One undo brings the note back.
    assert!(doc.undo());
    assert_eq!(notes(&doc).len(), 2);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
}

#[test]
fn identical_citations_next_to_each_other_stay_two_citations() {
    let mut doc = start();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    cite(&mut doc, "Citing is hard", "hard(Smith, 2020)", &["Smi20"], None);
    assert_eq!(body_texts(&doc)[0], "Citing is hard(Smith, 2020)(Smith, 2020) here.");
    assert_eq!(doc.citations().len(), 2);
    // The view model keeps them apart, so each can be clicked as a unit of its own.
    let r = doc.render_all();
    let first = find_p(&doc, "Citing is hard");
    let Some(docxtor_core::render::RBlock::Para(p)) = r.blocks.iter().find(|b| matches!(b, docxtor_core::render::RBlock::Para(p) if p.id == first)) else { panic!("no paragraph") };
    let cited: Vec<&str> = p.segs.iter().filter(|s| s.cite.is_some()).filter_map(|s| s.x.as_deref()).collect();
    assert_eq!(cited, ["(Smith, 2020)", "(Smith, 2020)"]);
}
