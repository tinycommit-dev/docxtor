//! Tests of: Undo and redo of every op and importing and ai tools and positions and rendering.

use super::*;

#[test]
fn every_citation_op_undoes_and_redoes() {
    let mut doc = doc_with("Citing is hard here.");
    let base = doc.save().unwrap();
    let states = |doc: &mut Document| -> (Vec<String>, usize, String, usize) { (texts(doc), doc.sources().len(), doc.citation_style(), doc.citations().len()) };
    let mut history = vec![states(&mut doc)];
    let steps: Vec<Value> = vec![
        json!({"op":"add_source","source":smith()}),
        json!({"op":"add_source","source":lee()}),
        json!({"op":"insert_citation","at":{"p":find_p(&doc, "Citing"),"o":14},"tags":["Smi20"]}),
        json!({"op":"insert_bibliography"}),
        json!({"op":"set_citation_style","style":"mla"}),
        json!({"op":"update_source","tag":"Smi20","source":{"type":"book","authors":["Smythe, John"],"title":"Renamed","year":"2022"}}),
        json!({"op":"edit_citation","at":{"p":find_p(&doc, "Citing"),"o":15},"page":"3"}),
        json!({"op":"delete_source","tag":"Lee21"}),
        json!({"op":"update_bibliography"}),
    ];
    for step in &steps {
        ops(&mut doc, step.clone());
        history.push(states(&mut doc));
    }
    // Walk all the way back, comparing each state, then all the way forward.
    for i in (0..steps.len()).rev() {
        assert!(doc.undo(), "undo of step {i}: {}", steps[i]);
        assert_eq!(states(&mut doc), history[i], "after undoing step {i}: {}", steps[i]);
        assert_valid(&doc.save().unwrap());
    }
    let back = Package::open(&doc.save().unwrap()).unwrap();
    assert_eq!(back.get("word/document.xml").map(|b| String::from_utf8_lossy(b).contains("CITATION")), Some(false));
    assert_eq!(history[0].1, 0);
    let _ = base;
    for (i, step) in steps.iter().enumerate() {
        assert!(doc.redo(), "redo of step {i}");
        assert_eq!(states(&mut doc), history[i + 1], "after redoing step {i}: {step}");
    }
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert_eq!(Document::open(&out).unwrap().sources().len(), 1);
}

#[test]
fn undoing_the_first_source_after_a_save_leaves_a_valid_empty_list() {
    let mut doc = doc_with("Hello.");
    add(&mut doc, smith());
    let saved = doc.save().unwrap();
    assert!(has_part(&saved, "customXml/item1.xml"));
    assert!(doc.undo());
    assert!(doc.sources().is_empty());
    let out = doc.save().unwrap();
    assert_valid(&out);
    assert!(Document::open(&out).unwrap().sources().is_empty());
}

#[test]
fn a_failed_batch_changes_nothing() {
    let mut doc = cited_doc();
    let p = find_p(&doc, "Citing is hard");
    let r = doc.apply_json(&json!([{"op":"add_source","source":{"type":"misc","title":"New one"}},{"op":"insert_citation","at":{"p":p,"o":0},"tags":["Nope"]}]).to_string(), false);
    assert!(r.is_err());
    assert_eq!(doc.sources().len(), 2);
}

const GOOGLE_SCHOLAR: &str = r#"@article{vaswani2017attention,
  title={Attention is all you need},
  author={Vaswani, Ashish and Shazeer, Noam and Parmar, Niki and Uszkoreit, Jakob and Jones, Llion and Gomez, Aidan N and Kaiser, {\L}ukasz and Polosukhin, Illia},
  journal={Advances in neural information processing systems},
  volume={30},
  year={2017}
}

@inproceedings{he2016deep,
  title={Deep residual learning for image recognition},
  author={He, Kaiming and Zhang, Xiangyu and Ren, Shaoqing and Sun, Jian},
  booktitle={Proceedings of the IEEE conference on computer vision and pattern recognition},
  pages={770--778},
  year={2016}
}"#;

const ZOTERO: &str = r#"
@book{kuhn_structure_2012,
	address = {Chicago ; London},
	edition = {4th ed},
	series = {Fifty years on},
	title = {The structure of scientific revolutions},
	isbn = {978-0-226-45811-3},
	publisher = {The University of Chicago Press},
	author = {Kuhn, Thomas S.},
	year = {2012},
	note = {OCLC: 784579979},
	keywords = {Science, philosophy},
}

@article{bohr_quantum_1928,
	title = {The {Quantum} {Postulate} and the {Recent} {Development} of {Atomic} {Theory}},
	volume = {121},
	issn = {0028-0836, 1476-4687},
	url = {https://www.nature.com/articles/121580a0},
	doi = {10.1038/121580a0},
	language = {en},
	number = {3050},
	urldate = {2023-05-02},
	journal = {Nature},
	author = {Bohr, N.},
	month = apr,
	year = {1928},
	pages = {580--590},
}
"#;

#[test]
fn bibtex_from_google_scholar_and_zotero_goes_into_the_document() {
    let mut doc = doc_with("Hello.");
    for text in [GOOGLE_SCHOLAR, ZOTERO] {
        for s in bibtex::parse(text).unwrap() {
            ops(&mut doc, json!({"op":"add_source","source":s}));
        }
    }
    let s = doc.sources();
    assert_eq!(s.len(), 4);
    assert_eq!(s[0].authors.len(), 8);
    assert_eq!(s[0].authors[6], docxtor_core::citations::Name::person("Kaiser", "Łukasz"));
    assert_eq!(s[0].tag, "Vas17");
    assert_eq!(s[1].kind.id(), "conference");
    assert_eq!(s[1].pages.as_deref(), Some("770-778"));
    assert_eq!(s[2].edition.as_deref(), Some("4th ed"));
    assert_eq!(s[2].place.as_deref(), Some("Chicago ; London"));
    assert_eq!(s[3].title, "The Quantum Postulate and the Recent Development of Atomic Theory");
    assert_eq!((s[3].month.as_deref(), s[3].accessed.as_deref(), s[3].doi.as_deref()), (Some("April"), Some("2023-05-02"), Some("10.1038/121580a0")));
    assert_eq!(s[3].tag, "Boh28");
}

#[test]
fn csl_json_from_doi_org_and_zotero_parses() {
    let crossref = r#"{"DOI":"10.1038/nature14539","type":"article-journal","title":"Deep learning","author":[{"given":"Yann","family":"LeCun"},{"given":"Yoshua","family":"Bengio"}],"container-title":"Nature","issued":{"date-parts":[[2015,5,28]]},"volume":"521","issue":"7553","page":"436-444","URL":"http://dx.doi.org/10.1038/nature14539"}"#;
    let v = csl_json::parse(crossref).unwrap();
    assert_eq!(v.len(), 1);
    let mut doc = doc_with("Hello.");
    ops(&mut doc, json!({"op":"add_source","source":v[0]}));
    assert_eq!(doc.sources()[0].tag, "LeC15");
    let list = csl_json::parse(r#"[{"type":"book","title":"B","author":[{"family":"Roe","given":"Jane"}],"issued":{"date-parts":[[1999]]}}]"#).unwrap();
    assert_eq!(list[0].year.as_deref(), Some("1999"));
}

#[test]
fn bibtex_export_reads_back() {
    let mut doc = cited_doc();
    let out = bibtex::to_bibtex(&doc.sources());
    assert!(out.contains("@book{Smi20,") && out.contains("@article{Lee21,"), "{out}");
    assert!(out.contains("author = {Lee, Karen and Kim, Min-Jun}") || out.contains("author = {Lee, Karen and Kim"), "{out}");
    let back = bibtex::parse(&out).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[1].pages.as_deref(), Some("45-67"));
    let _ = &mut doc;
}

#[test]
fn the_ai_tools_cover_sources_citations_and_the_list() {
    let mut doc = doc_with("Citing is hard here.\n\nSecond paragraph about things.");
    let p1 = find_p(&doc, "Citing is hard");
    assert!(call_tool(&mut doc, "list_sources", &json!({})).unwrap().contains("no sources yet"));
    let r = call_tool(&mut doc, "add_source", &json!({"type":"book","authors":["Smith, John"],"title":"The Art of Citing","year":"2020","publisher":"Penguin","place":"New York"})).unwrap();
    assert!(r.contains("Smi20 [book] Smith, John (2020). The Art of Citing"), "{r}");
    let r = call_tool(&mut doc, "add_source", &json!({"bibtex":GOOGLE_SCHOLAR})).unwrap();
    assert!(r.contains("Added 2 source(s)") && r.contains("Vas17 [article]") && r.contains("He16 [conference]"), "{r}");
    let r = call_tool(&mut doc, "add_source", &json!({"type":"article","container_title":"Journal of Stuff","title":"A study","authors":["Lee, Karen"],"year":2021})).unwrap();
    assert!(r.contains("Lee21"), "{r}");
    assert_eq!(doc.sources()[3].container_title.as_deref(), Some("Journal of Stuff"));
    let list = call_tool(&mut doc, "list_sources", &json!({})).unwrap();
    assert!(list.contains("Citation style: apa. 4 source(s)") && list.contains("Smi20 [book]") && list.contains("cited 0 times"), "{list}");

    let r = call_tool(&mut doc, "cite", &json!({"id":p1,"after_text":"hard","tags":["Smi20"],"page":"12"})).unwrap();
    assert!(r.contains("Citation inserted") && r.contains("[@Smi20]"), "{r}");
    // Set off by a space, as Word writes a citation in the text.
    assert_eq!(texts(&doc)[0], "Citing is hard (Smith, 2020, p. 12) here.");
    // Tags can be given as one comma-separated string too.
    let p2 = find_p(&doc, "Second paragraph");
    call_tool(&mut doc, "cite", &json!({"id":p2,"tags":"Smi20, Lee21"})).unwrap();
    assert_eq!(texts(&doc)[1], "Second paragraph about things. (Smith, 2020; Lee, 2021)");
    let e = call_tool(&mut doc, "cite", &json!({"id":p1,"tags":["Nope"]})).unwrap_err();
    assert!(e.contains("no source with the tag 'Nope'"), "{e}");
    assert!(call_tool(&mut doc, "cite", &json!({"id":p1})).is_err());

    let r = call_tool(&mut doc, "insert_bibliography", &json!({})).unwrap();
    assert!(r.contains("Bibliography inserted (apa style)") && r.contains("References") && r.contains("Smith, J. (2020)"), "{r}");
    assert!(!r.contains("Nothing is cited yet"));
    let r = call_tool(&mut doc, "set_citation_style", &json!({"style":"harvard"})).unwrap();
    assert!(r.contains("harvard"), "{r}");
    // Set off by a space, as Word writes a citation in the text.
    assert_eq!(texts(&doc)[0], "Citing is hard (Smith, 2020, p. 12) here.");
    assert!(call_tool(&mut doc, "set_citation_style", &json!({"style":"nope"})).is_err());
    let list = call_tool(&mut doc, "list_sources", &json!({})).unwrap();
    assert!(list.contains("Citation style: harvard") && list.contains("cited 2 times"), "{list}");
    // The tool definitions and the guide mention them.
    let defs = docxtor_core::ai::tool_definitions().to_string();
    for t in ["list_sources", "add_source", "\"cite\"", "insert_bibliography", "set_citation_style"] {
        assert!(defs.contains(t), "{t}");
    }
    assert!(docxtor_core::ai::SYSTEM_GUIDE.contains("[@Smi20]"));
}

#[test]
fn an_empty_bibliography_from_the_tool_says_why() {
    let mut doc = doc_with("Hello.");
    call_tool(&mut doc, "add_source", &json!({"type":"book","authors":["Smith, John"],"title":"T","year":"2020"})).unwrap();
    let r = call_tool(&mut doc, "insert_bibliography", &json!({})).unwrap();
    assert!(r.contains("Nothing is cited yet"), "{r}");
}

#[test]
fn text_offsets_count_the_formatted_result_and_citations_are_findable() {
    let mut doc = cited_doc();
    cite(&mut doc, "Citing is hard", "hard", &["Smi20"], None);
    let c = doc.citations()[0].clone();
    assert_eq!((c.o, c.len), (14, 13));
    let p = doc.find_any_para(c.p).unwrap();
    assert_eq!(p.plain_text().encode_utf16().skip(c.o).take(c.len).collect::<Vec<u16>>(), "(Smith, 2020)".encode_utf16().collect::<Vec<u16>>());
    // The block is findable and its XML carries the structures.
    let b = find_block(&doc.body, c.p).unwrap();
    let xml = write_block(b).to_xml();
    assert!(xml.contains("<w:citation/>"));
    // The model json for the editor.
    let json = serde_json::to_value(doc.citations()).unwrap();
    assert_eq!(json[0]["tags"], json!(["Smi20"]));
    assert_eq!(json[0]["o"], 14);
    assert!(json[0].get("page").is_none());
}
