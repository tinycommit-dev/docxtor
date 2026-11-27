use docxtor_core::{fidelity, package::Package, Document, Op, Pos};
use serde_json::Value;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {p:?}; run python3 fixtures/make_fixtures.py"))
}

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

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn para_texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn find_para(doc: &Document, needle: &str) -> u32 {
    let mut id = None;
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().contains(needle) {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph contains {needle:?}"))
}

#[test]
fn unedited_roundtrip_is_lossless() {
    for f in ["simple.docx", "rich.docx", "long.docx"] {
        let bytes = fixture(f);
        let mut doc = Document::open(&bytes).unwrap();
        let out = doc.save().unwrap();
    assert_valid(&out);
        let report = fidelity::compare(&Package::open(&bytes).unwrap(), &Package::open(&out).unwrap());
        assert_eq!(report.score, 100.0, "{f}: {report:?}");
        assert!(report.missing.is_empty() && report.added.is_empty());
    }
}

#[test]
fn reserialized_document_is_semantically_identical() {
    // Force the body to be re-serialized without changes: the XML must be equivalent.
    let bytes = fixture("rich.docx");
    let mut doc = Document::open(&bytes).unwrap();
    doc.apply(&Op::FormatText { from: Pos::new(1, 0), to: Pos::new(1, 0), format: Default::default() }, false).unwrap();
    let out = doc.save().unwrap();
    assert_valid(&out);
    let report = fidelity::compare(&Package::open(&bytes).unwrap(), &Package::open(&out).unwrap());
    assert!(report.score >= 99.9, "{report:#?}");
}

#[test]
fn render_rich_document() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let r = serde_json::to_value(doc.render_all()).unwrap();
    let s = r.to_string();
    assert!(s.contains("\"link\":\"https://example.com/report\""), "hyperlink resolved");
    assert!(s.contains("\"t\":\"tbl\""), "table rendered");
    assert!(s.contains("\"span\":2"), "merged cell");
    assert!(s.contains("\"k\":\"img\""), "image");
    assert!(s.contains("\"text\":\"1.\"") && s.contains("\"text\":\"3.\""), "numbered list labels");
    assert!(s.contains("\"bullet\":true"), "bullets");
    assert!(s.contains("\"k\":\"pbr\""), "page break");
    let page = serde_json::to_value(doc.page_setup()).unwrap();
    let ps = page.to_string();
    assert!(ps.contains("confidential draft"), "header");
    assert!(ps.contains("\"fld\":\"PAGE\""), "page field in footer");
}

#[test]
fn editing_session() {
    let bytes = fixture("rich.docx");
    let mut doc = Document::open(&bytes).unwrap();
    let pid = find_para(&doc, "This report summarises");

    // typing inherits formatting of the preceding run
    ops(&mut doc, serde_json::json!({"op":"insert_text","at":{"p":pid,"o":4},"text":" quarterly"}));
    assert!(para_texts(&doc).iter().any(|t| t.starts_with("This quarterly report")));

    // bold a word, then split the paragraph and delete across the split
    ops(&mut doc, serde_json::json!({"op":"format_text","from":{"p":pid,"o":0},"to":{"p":pid,"o":4},"format":{"bold":true}}));
    let r = doc.apply(&Op::SplitParagraph { at: Pos::new(pid, 14) }, false).unwrap();
    let new_id = r.created[0];
    assert!(para_texts(&doc).iter().any(|t| t == "This quarterly"));
    ops(&mut doc, serde_json::json!({"op":"delete","from":{"p":pid,"o":14},"to":{"p":new_id,"o":0}}));
    assert!(para_texts(&doc).iter().any(|t| t.starts_with("This quarterly report summarises")));

    // AI-style ops
    ops(&mut doc, serde_json::json!({"op":"replace_text","find":"Churn down to 2.1%","replace":"Churn fell to 1.9%"}));
    let h = find_para(&doc, "Highlights");
    ops(&mut doc, serde_json::json!({"op":"insert_markdown","after":format!("p{h}"),"markdown":"Intro with **bold** and a [link](https://docxtor.tinycommit.dev).\n\n- one\n- two\n\n| A | B |\n|---|---|\n| 1 | 2 |"}));
    let one = find_para(&doc, "one");
    ops(&mut doc, serde_json::json!({"op":"toggle_list","ids":[one],"kind":"bullet"}));
    ops(&mut doc, serde_json::json!({"op":"toggle_list","ids":[one],"kind":"decimal"}));
    ops(&mut doc, serde_json::json!({"op":"set_style","ids":[one],"style":"Heading 3"}));
    let cell = find_para(&doc, "$1.2M");
    ops(&mut doc, serde_json::json!({"op":"table_edit","at":cell,"action":"row_below"}));
    ops(&mut doc, serde_json::json!({"op":"table_edit","at":cell,"action":"col_right"}));
    ops(&mut doc, serde_json::json!({"op":"set_link","from":{"p":pid,"o":0},"to":{"p":pid,"o":4},"url":"https://a.b"}));

    let out = doc.save().unwrap();
    assert_valid(&out);
    // The output must re-open and keep everything.
    let mut again = Document::open(&out).unwrap();
    let texts = para_texts(&again);
    assert!(texts.iter().any(|t| t == "Churn fell to 1.9%"));
    assert!(texts.iter().any(|t| t.contains("Intro with bold and a link.")));
    assert!(texts.iter().any(|t| t == "Final paragraph on page two."));
    let s = serde_json::to_value(again.render_all()).unwrap().to_string();
    assert!(s.contains("https://docxtor.tinycommit.dev"));
    assert!(s.contains("https://a.b"));
    // Headers, footers, media and theme must be untouched.
    let report = fidelity::compare(&Package::open(&bytes).unwrap(), &Package::open(&out).unwrap());
    for p in &report.changed {
        assert!(
            p.name.contains("document.xml") || p.name.contains("styles.xml") || p.name.contains("numbering.xml") || p.name.ends_with(".rels"),
            "unexpected change to {}",
            p.name
        );
    }
}

#[test]
fn undo_redo() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    let pid = find_para(&doc, "Hello");
    doc.apply(&Op::InsertText { at: Pos::new(pid, 5), text: ",".into(), format: None }, false).unwrap();
    doc.apply(&Op::InsertText { at: Pos::new(pid, 6), text: " dear".into(), format: None }, true).unwrap();
    assert_eq!(para_texts(&doc)[0], "Hello, dear world.");
    assert!(doc.undo());
    assert_eq!(para_texts(&doc)[0], "Hello world.");
    assert!(doc.redo());
    assert_eq!(para_texts(&doc)[0], "Hello, dear world.");
}

#[test]
fn blank_document_is_valid() {
    let mut doc = Document::new_blank();
    let pid = doc.body.iter().find_map(|b| if let docxtor_core::model::Block::Para(p) = b { Some(p.id) } else { None }).unwrap();
    ops(&mut doc, serde_json::json!([
        {"op":"insert_text","at":{"p":pid,"o":0},"text":"Title here"},
        {"op":"set_style","ids":[pid],"style":"Title"},
        {"op":"insert_markdown","after":pid,"markdown":"## Section\n\nBody text.\n\n1. first\n2. second"}
    ]));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    assert_eq!(para_texts(&again), ["Title here", "Section", "Body text.", "first", "second"]);
    let pkg = Package::open(&out).unwrap();
    assert!(pkg.has("word/numbering.xml"), "numbering part created");
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(ct.contains("numbering+xml"));
}

#[test]
fn paste_markdown() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    let pid = find_para(&doc, "Hello");
    ops(&mut doc, serde_json::json!({"op":"paste_markdown","at":{"p":pid,"o":6},"markdown":"**big** "}));
    assert_eq!(para_texts(&doc), ["Hello big world."]);
    ops(&mut doc, serde_json::json!({"op":"paste_markdown","at":{"p":pid,"o":6},"markdown":"one\n\n## Two\n\nthree "}));
    assert_eq!(para_texts(&doc), ["Hello one", "Two", "three big world."]);
}

#[test]
fn ai_reads_do_not_swallow_ui_changes() {
    let mut doc = Document::open(&fixture("simple.docx")).unwrap();
    doc.render_all();
    docxtor_core::ai::call_tool(&mut doc, "replace_text", &serde_json::json!({"find": "Hello", "replace": "Howdy"})).unwrap();
    let out = doc.render_changed();
    assert_eq!(out.blocks.len(), 1, "the edited paragraph must still be reported as changed after an AI tool echo");
}

#[test]
fn page_setup() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    ops(&mut doc, serde_json::json!({"op":"set_page_setup","width":16838,"height":11906,"margins":[720,720,720,720]}));
    let p = doc.page_setup();
    assert_eq!((p.width, p.height, p.margin), (16838, 11906, [720, 720, 720, 720]));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    assert_eq!(again.page_setup().width, 16838);
    // headers survive (header references come before pgSz in sectPr)
    assert!(serde_json::to_string(&again.page_setup()).unwrap().contains("confidential draft"));
}

#[test]
fn review_regressions() {
    // replace_text across an emoji and with case-folding length changes
    let mut doc = Document::new_blank();
    let pid = docxtor_core::model::paragraph_order(&doc.body)[0];
    ops(&mut doc, serde_json::json!({"op":"insert_text","at":{"p":pid,"o":0},"text":"😀ab İİ cat dog"}));
    ops(&mut doc, serde_json::json!({"op":"replace_text","find":"😀a","replace":"X"}));
    ops(&mut doc, serde_json::json!({"op":"replace_text","find":"DOG","replace":"fox","ignore_case":true}));
    assert_eq!(para_texts(&doc)[0], "Xb İİ cat fox");

    // checkpoints stay meaningful past the undo cap
    for i in 0..510 {
        ops(&mut doc, serde_json::json!({"op":"insert_text","at":{"p":pid,"o":0},"text": if i % 2 == 0 { "a" } else { "b" }}));
    }
    let cp = doc.checkpoint();
    ops(&mut doc, serde_json::json!({"op":"insert_text","at":{"p":pid,"o":0},"text":"Z"}));
    assert_eq!(doc.checkpoint(), cp + 1);
    assert_eq!(doc.diff_since(cp).len(), 1);
    assert!(doc.revert_to(cp));
    assert!(!para_texts(&doc)[0].starts_with('Z'));

    // a style-inherited list can be turned off and Enter on an empty item exits it
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let item = find_para(&doc, "Two new enterprise");
    ops(&mut doc, serde_json::json!({"op":"toggle_list","ids":[item],"kind":"bullet"}));
    let s = serde_json::to_string(&doc.render_all()).unwrap();
    assert!(!doc.numbering.labels(&doc.body, &doc.styles).contains_key(&item), "{}", s.len());
    let item2 = find_para(&doc, "Churn down");
    ops(&mut doc, serde_json::json!({"op":"delete","from":{"p":item2,"o":0},"to":{"p":item2,"o":18}}));
    ops(&mut doc, serde_json::json!({"op":"split_paragraph","at":{"p":item2,"o":0}}));
    assert!(!doc.numbering.labels(&doc.body, &doc.styles).contains_key(&item2));

    // typing after Enter at the end of a link doesn't inherit the link style
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let p = find_para(&doc, "This report");
    let t = docxtor_core::model::find_para(&doc.body, p).unwrap().text();
    let end_of_link = t[..t.find("the dashboard").unwrap() + 13].encode_utf16().count();
    // Cut the trailing text so the caret sits right at the end of the link, then press Enter.
    ops(&mut doc, serde_json::json!({"op":"delete","from":{"p":p,"o":end_of_link},"to":{"p":p,"o":t.encode_utf16().count()}}));
    let r = doc.apply(&docxtor_core::Op::SplitParagraph { at: docxtor_core::Pos::new(p, end_of_link) }, false).unwrap();
    let np = r.created[0];
    ops(&mut doc, serde_json::json!({"op":"insert_text","at":{"p":np,"o":0},"text":"plain"}));
    let xml = docxtor_core::model::write_block(docxtor_core::model::find_block(&doc.body, np).unwrap()).to_xml();
    assert!(!xml.contains("Hyperlink"), "{xml}");

    // changing the URL of part of a link keeps the rest linked
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let p = find_para(&doc, "This report");
    let text = docxtor_core::model::find_para(&doc.body, p).unwrap().text();
    let start = text.find("the dashboard").unwrap() + 4; // "dashboard"
    ops(&mut doc, serde_json::json!({"op":"set_link","from":{"p":p,"o":start},"to":{"p":p,"o":start+9},"url":"https://new.example"}));
    let s = serde_json::to_string(&doc.render_all()).unwrap();
    assert!(s.contains("https://example.com/report") && s.contains("https://new.example"));
}

#[test]
fn image_resize_and_wrap() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let mut p = 0;
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |x| {
        if p == 0 && x.text().contains('\u{FFFC}') {
            p = x.id;
        }
    });
    ops(&mut doc, serde_json::json!({"op":"set_image","at":{"p":p,"o":0},"width":200.0,"height":100.0,"wrap":"square","align":"right","alt":"A blue box"}));
    let s = serde_json::to_string(&doc.render_all()).unwrap();
    assert!(s.contains(r#""wrap":"square""#) && s.contains(r#""align":"right""#) && s.contains(r#""w":200.0"#), "{s}");
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    let s = serde_json::to_string(&again.render_snapshot()).unwrap();
    assert!(s.contains(r#""wrap":"square""#) && s.contains("A blue box"));
    let py = std::process::Command::new("python3").arg("-c").arg("import sys,docx; d=docx.Document(sys.argv[1]); print(len(d.paragraphs))").arg({
        let path = std::env::temp_dir().join("docxtor_img_test.docx");
        std::fs::write(&path, &out).unwrap();
        path
    }).output();
    // python-docx is a second opinion where it is installed; without it (or without python3) the check is skipped.
    if let Ok(o) = py {
        let err = String::from_utf8_lossy(&o.stderr);
        if o.status.code() == Some(1) && !err.contains("No module named 'docx'") {
            panic!("python-docx failed to open the output: {err}");
        }
    }
}

#[test]
fn table_cells_ops() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    // The table cell "Revenue" (not the bullet that starts with the same word).
    let mut a = 0;
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| {
        if a == 0 && p.plain_text() == "Revenue" {
            a = p.id;
        }
    });
    let b = find_para(&doc, "$1.2M");
    ops(&mut doc, serde_json::json!({"op":"table_cells","from":a,"to":b,"action":{"type":"merge"}}));
    ops(&mut doc, serde_json::json!({"op":"table_props","at":b.max(a),"widths":[3000,2000,2000],"borders":"horizontal","header":true}));
    let s = serde_json::to_string(&doc.render_all()).unwrap();
    assert!(s.contains("\"span\":2"), "merged");
    assert!(s.contains("\"header\":true"));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    assert!(serde_json::to_string(&again.render_snapshot()).unwrap().contains("Revenue"));
}

fn story_para(doc: &Document, kind: docxtor_core::stories::StoryKind) -> u32 {
    let s = doc.stories.iter().find(|s| s.kind == kind).expect("story");
    let mut id = None;
    docxtor_core::model::walk_paragraphs(&s.blocks, &mut |p| {
        id.get_or_insert(p.id);
    });
    id.unwrap()
}

fn story_text(doc: &Document, kind: docxtor_core::stories::StoryKind) -> String {
    let mut out = String::new();
    for s in doc.stories.iter().filter(|s| s.kind == kind) {
        docxtor_core::model::walk_paragraphs(&s.blocks, &mut |p| out.push_str(&p.plain_text()));
    }
    out
}

#[test]
fn edit_existing_header_and_footer() {
    use docxtor_core::stories::StoryKind;
    let bytes = fixture("rich.docx");
    let original = Package::open(&bytes).unwrap();
    let mut doc = Document::open(&bytes).unwrap();
    let ps = doc.page_setup();
    assert_eq!(ps.headers["default"].part, "word/header1.xml");

    // Editing the body leaves the header part byte-identical.
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": {"p": 1, "o": 0}, "text": "X"}));
    let out = doc.save().unwrap();
    let saved = Package::open(&out).unwrap();
    assert_eq!(saved.get("word/header1.xml"), original.get("word/header1.xml"));

    // Typing into the header goes to its part, not the body.
    let hp = story_para(&doc, StoryKind::Header);
    let before = para_texts(&doc);
    doc.render_changed();
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": {"p": hp, "o": 0}, "text": "Draft — "}));
    assert_eq!(para_texts(&doc), before);
    assert!(story_text(&doc, StoryKind::Header).starts_with("Draft — "));
    let r = doc.render_changed();
    assert!(r.stories && r.blocks.is_empty());
    let out = doc.save().unwrap();
    assert_valid(&out);
    let re = Document::open(&out).unwrap();
    assert!(story_text(&re, StoryKind::Header).starts_with("Draft — "));
    assert_eq!(Package::open(&out).unwrap().get("word/document.xml"), saved.get("word/document.xml"));

    // Undo restores the header.
    assert!(doc.undo());
    assert!(!story_text(&doc, StoryKind::Header).starts_with("Draft"));
    let re = Document::open(&doc.save().unwrap()).unwrap();
    assert!(!story_text(&re, StoryKind::Header).starts_with("Draft"));
}

#[test]
fn add_header_footer_and_fields() {
    use docxtor_core::stories::StoryKind;
    let mut doc = Document::new_blank();
    let r = doc.apply_json(r#"{"op":"add_header_footer","kind":"header"}"#, false).unwrap();
    let hp = r[0].caret.unwrap();
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": hp, "text": "Quarterly report"}));
    let r = doc.apply_json(r#"{"op":"add_header_footer","kind":"footer"}"#, false).unwrap();
    let fp = r[0].caret.unwrap();
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": fp, "text": "Page "}));
    ops(&mut doc, serde_json::json!({"op": "insert_field", "at": {"p": fp.p.0, "o": 5}, "field": "page"}));
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": {"p": fp.p.0, "o": 6}, "text": " of "}));
    ops(&mut doc, serde_json::json!({"op": "insert_field", "at": {"p": fp.p.0, "o": 10}, "field": "NUMPAGES"}));
    // Asking again for an existing header just returns its caret.
    let again = doc.apply_json(r#"{"op":"add_header_footer","kind":"header"}"#, false).unwrap();
    assert_eq!(again[0].caret.unwrap().p, hp.p);

    let ps = doc.page_setup();
    let foot = serde_json::to_value(&ps.footers["default"]).unwrap();
    let flds: Vec<String> = foot["blocks"][0]["segs"].as_array().unwrap().iter().filter_map(|s| s["fld"].as_str().map(String::from)).collect();
    assert_eq!(flds, ["PAGE", "NUMPAGES"]);

    let out = doc.save().unwrap();
    assert_valid(&out);
    let pkg = Package::open(&out).unwrap();
    let ct = String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap();
    assert!(ct.contains("/word/header1.xml") && ct.contains("header+xml") && ct.contains("footer+xml"));
    let docxml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    let h = docxml.find("w:headerReference").unwrap();
    assert!(h < docxml.find("w:pgSz").unwrap(), "references come first in sectPr");
    let footer = String::from_utf8(pkg.get("word/footer1.xml").unwrap().to_vec()).unwrap();
    assert!(footer.contains(r#"<w:fldSimple w:instr=" PAGE ">"#), "{footer}");

    let re = Document::open(&out).unwrap();
    assert_eq!(story_text(&re, StoryKind::Header), "Quarterly report");
    assert_eq!(story_text(&re, StoryKind::Footer), "Page 1 of 1");

    // Different first page, then remove it again.
    ops(&mut doc, serde_json::json!({"op": "add_header_footer", "kind": "header", "variant": "first"}));
    assert!(doc.page_setup().title_page);
    assert_eq!(doc.page_setup().headers.len(), 2);
    ops(&mut doc, serde_json::json!({"op": "remove_header_footer", "kind": "header", "variant": "first"}));
    ops(&mut doc, serde_json::json!({"op": "set_title_page", "on": false}));
    assert!(!doc.page_setup().title_page);
    assert_eq!(doc.page_setup().headers.len(), 1);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn footnotes() {
    use docxtor_core::stories::StoryKind;
    let mut doc = Document::new_blank();
    let p = doc.render_all().order[0];
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "Alpha beta."}));
    let r = doc.apply_json(&serde_json::json!({"op": "insert_footnote", "at": {"p": p, "o": 5}, "text": "First note."}).to_string(), false).unwrap();
    let caret = r[0].caret.unwrap();
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": caret, "text": " More."}));
    assert_eq!(story_text(&doc, StoryKind::Footnote), " First note. More.");
    // A note inserted earlier in the text takes number 1.
    ops(&mut doc, serde_json::json!({"op": "insert_footnote", "at": {"p": p, "o": 0}, "text": "Earlier."}));
    let out = doc.render_changed();
    assert!(out.stories);
    let ps = doc.page_setup();
    assert_eq!(ps.notes.len(), 2);
    let first = serde_json::to_value(&ps.notes[0]).unwrap();
    assert_eq!(ps.notes[0].num, 1);
    assert!(first.to_string().contains("Earlier."));
    let body = serde_json::to_value(doc.render_snapshot()).unwrap();
    let labels: Vec<String> = body["blocks"][0]["segs"].as_array().unwrap().iter().filter(|s| s["k"] == "note").map(|s| s["label"].as_str().unwrap().to_string()).collect();
    assert_eq!(labels, ["1", "2"]);

    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let re = Document::open(&saved).unwrap();
    assert_eq!(re.page_setup().notes.len(), 2);
    let pkg = Package::open(&saved).unwrap();
    let notes = String::from_utf8(pkg.get("word/footnotes.xml").unwrap().to_vec()).unwrap();
    assert!(notes.contains(r#"w:type="separator""#) && notes.contains("<w:footnoteRef/>"));

    // Undo the second note: it disappears from the part on save.
    assert!(doc.undo());
    let re = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(re.page_setup().notes.len(), 1);
    assert!(!story_text(&re, StoryKind::Footnote).contains("Earlier"));
}

#[test]
fn ai_header_footer_and_footnote_tools() {
    use docxtor_core::ai::call_tool;
    let mut doc = Document::new_blank();
    let p = doc.render_all().order[0];
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "The sky is blue. Water is wet."}));
    let out = call_tool(&mut doc, "set_header_footer", &serde_json::json!({"kind": "footer", "text": "**Acme** — Page {PAGE} of {NUMPAGES}"})).unwrap();
    assert!(out.contains("**Acme** — Page 1 of 1"), "{out}");
    let out = call_tool(&mut doc, "insert_footnote", &serde_json::json!({"id": format!("p{p}"), "after_text": "blue.", "text": "Rayleigh scattering."})).unwrap();
    assert!(out.contains("blue.[^1] Water"), "{out}");
    let script = call_tool(&mut doc, "read_document", &serde_json::json!({})).unwrap();
    assert!(script.contains("# Footer (default)") && script.contains("[^1] Rayleigh scattering."), "{script}");
    assert!(call_tool(&mut doc, "search", &serde_json::json!({"query": "Rayleigh"})).unwrap().contains("FootnoteText"));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn export_markdown_and_text() {
    use docxtor_core::export::{to_markdown, to_text};
    let doc = Document::open(&fixture("rich.docx")).unwrap();
    let md = to_markdown(&doc);
    assert!(md.starts_with("# Quarterly Product Report\n\n"), "{md}");
    assert!(md.contains("**bold progress**") && md.contains("[the dashboard](https://example.com/report)"));
    assert!(md.contains("- Revenue grew 18% year over year\n- Two new enterprise customers\n"));
    assert!(md.contains("1. Ship the editor\n2. Publish fidelity scores\n"));
    assert!(md.contains("| --- | --- | --- |") && md.contains("> Simplicity"));
    assert!(md.contains("![](media/image1.png)"));
    assert!(!md.contains("[p") && !md.contains("<pagebreak>"));

    // Markdown in, Markdown out: headings, lists, quotes, tables and footnotes survive.
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let src = "# Plan\n\nSome *text* here.\n\n- one\n  - nested\n- two\n\n> quoted\n\n| a | b |\n|---|---|\n| 1 | 2 |";
    docxtor_core::ai::call_tool(&mut doc, "insert_content", &serde_json::json!({"before": first, "markdown": src})).unwrap();
    docxtor_core::ai::call_tool(&mut doc, "delete_blocks", &serde_json::json!({"ids": [first]})).unwrap();
    let p = find_para(&doc, "Some text here.");
    ops(&mut doc, serde_json::json!({"op": "insert_footnote", "at": {"p": p, "o": 4}, "text": "A note."}));
    let md = to_markdown(&doc);
    assert!(md.contains("# Plan\n\nSome[^1] *text* here.\n\n- one\n  - nested\n- two\n\n> quoted\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n"), "{md}");
    assert!(md.trim_end().ends_with("[^1]: A note."), "{md}");
    let text = to_text(&doc);
    assert!(text.starts_with("Plan\nSome[1] text here.\n• one\n\t• nested\n• two\nquoted\na\tb\n1\t2\n"), "{text:?}");

    // A range: cut inside the first and last paragraphs, whole blocks between.
    use docxtor_core::export::range_to_markdown;
    let rich = Document::open(&fixture("rich.docx")).unwrap();
    let a = find_para(&rich, "This report summarises");
    let b = find_para(&rich, "Two new enterprise");
    let md = range_to_markdown(&rich, Pos::new(a, 23), Pos::new(b, 7)).unwrap();
    assert_eq!(md, "**bold progress**, *italic caveats* and <u>red underlined risks</u>. Visit [the dashboard](https://example.com/report) for live numbers.\n\n# Highlights\n\n- Revenue grew 18% year over year\n- Two new\n", "{md}");
    // Backwards selections and a range inside one paragraph.
    assert_eq!(range_to_markdown(&rich, Pos::new(a, 36), Pos::new(a, 23)).unwrap(), "**bold progress**\n");
}

#[test]
fn table_of_contents() {
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let first = doc.render_all().order[0];
    ops(&mut doc, serde_json::json!({"op": "insert_toc", "after": first, "title": "Contents", "pages": {}}));
    let script = docxtor_core::ai::docscript(&mut doc, &Default::default());
    assert!(script.contains("TOCHeading] Contents"), "{script}");
    assert!(script.contains("TOC1] [Highlights\t]") || script.contains("TOC1]"), "{script}");
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let pkg = Package::open(&saved).unwrap();
    let xml = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
    assert!(xml.contains(r#" TOC \o "1-3" \h \z \u "#));
    assert!(xml.contains(r#"<w:pStyle w:val="TOC2"/>"#) && xml.contains(r#"w:leader="dot""#));
    let anchor = xml.split(r#"w:anchor=""#).nth(1).unwrap().split('"').next().unwrap().to_string();
    assert!(anchor.starts_with("_Toc"));
    assert!(xml.contains(&format!(r#"w:name="{anchor}""#)), "heading bookmark missing");
    assert!(xml.contains(&format!(" PAGEREF {anchor} \\h ")));
    let styles = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"w:styleId="TOC1""#) && styles.contains(r#"w:styleId="TOCHeading""#));

    // Rename a heading and update: the entry follows, bookmarks are reused, nothing else moves.
    let script = docxtor_core::ai::docscript(&mut doc, &Default::default());
    let h: u32 = script.lines().find(|l| l.contains("Heading1] Highlights")).unwrap()[2..].split(' ').next().unwrap().parse().unwrap();
    ops(&mut doc, serde_json::json!({"op": "replace_text", "find": "Highlights", "replace": "Key results", "ids": [h]}));
    let r = doc.apply_json(&serde_json::json!({"op": "update_toc", "pages": {h.to_string(): 2}}).to_string(), false).unwrap();
    assert_eq!(r[0].count, Some(1));
    let text = docxtor_core::export::to_text(&doc);
    assert!(text.contains("Key results\t2"), "{text}");
    assert!(!text.contains("Highlights"));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    let xml = String::from_utf8(Package::open(&saved).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap();
    assert_eq!(xml.matches(r#"w:name="_Toc"#).count(), xml.matches("PAGEREF").count(), "one bookmark per entry");
    let again = Document::open(&saved).unwrap();
    assert!(docxtor_core::export::to_text(&again).contains("Key results\t2"));

    // A table of contents Word made is rebuilt in place, inside its content control.
    let mut doc = Document::open(&fixture("complex.docx")).unwrap();
    ops(&mut doc, serde_json::json!({"op": "update_toc"}));
    let text = docxtor_core::export::to_text(&doc);
    assert!(text.contains("Complex constructs\t1") && !text.contains("........"), "{text}");
    assert_valid(&doc.save().unwrap());
    // Undo puts the old one back.
    assert!(doc.undo());
    assert!(docxtor_core::export::to_text(&doc).contains("........"));

    // No TOC to update is an error the caller can show.
    let mut blank = Document::new_blank();
    assert!(blank.apply_json(r#"{"op":"update_toc"}"#, false).is_err());
}

#[test]
fn embedded_fonts() {
    use docxtor_core::fonts::{deobfuscate, embedded_fonts};
    let guid = "{0123ABCD-4567-89EF-0123-456789ABCDEF}";
    let font: Vec<u8> = (0..64u8).collect();
    let mut stored = font.clone();
    assert!(deobfuscate(&mut stored, guid)); // obfuscating is the same XOR
    assert_ne!(stored[..32], font[..32]);
    assert_eq!(stored[32..], font[32..]);
    // The key is the GUID's bytes in reverse: byte 0 is XOR-ed with 0xEF.
    assert_eq!(stored[0], font[0] ^ 0xEF);

    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let main = pkg.main_document_path();
    pkg.set(
        "word/fontTable.xml",
        format!(
            r#"<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:font w:name="Lato"><w:embedRegular r:id="rId1" w:fontKey="{guid}"/></w:font><w:font w:name="Calibri"/></w:fonts>"#
        )
        .into_bytes(),
    );
    pkg.add_rel("word/fontTable.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/font", "fonts/font1.odttf", false);
    pkg.set("word/fonts/font1.odttf", stored.clone());
    pkg.add_rel(&main, "http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable", "fontTable.xml", false);
    let bytes = pkg.to_bytes().unwrap();
    let doc = Document::open(&bytes).unwrap();
    let fonts = embedded_fonts(&doc.pkg, &doc.main_path);
    assert_eq!(fonts.len(), 1);
    assert_eq!((fonts[0].family.as_str(), fonts[0].style), ("Lato", "regular"));
    assert_eq!(fonts[0].data, font);
    // The package keeps the font exactly as stored.
    let mut doc = doc;
    let saved = doc.save().unwrap();
    assert_eq!(Package::open(&saved).unwrap().get("word/fonts/font1.odttf").unwrap(), &stored[..]);
}

#[test]
fn replace_font() {
    use docxtor_core::stories::StoryKind;
    let font_names = |doc: &Document| doc.fonts_used().into_iter().map(|u| u.family).collect::<Vec<_>>();
    // Theme fonts: rich.docx's text is in its theme's fonts.
    let mut doc = Document::open(&fixture("rich.docx")).unwrap();
    let before = font_names(&doc);
    assert!(!before.is_empty());
    let main = before[0].clone();
    ops(&mut doc, serde_json::json!({"op": "replace_font", "from": main.to_uppercase(), "to": "Lora"}));
    let after = font_names(&doc);
    assert!(after.contains(&"Lora".to_string()) && !after.contains(&main), "{before:?} -> {after:?}");
    assert!(doc.undo());
    assert_eq!(font_names(&doc), before);
    assert!(doc.apply_json(r#"{"op":"replace_font","from":"No Such Font","to":"Lora"}"#, false).is_err());

    // Direct formatting, in the body and in a header.
    let mut doc = Document::new_blank();
    let p = find_para_id(&doc);
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": {"p": p, "o": 0}, "text": "Hello world"}));
    ops(&mut doc, serde_json::json!({"op": "format_text", "from": {"p": p, "o": 0}, "to": {"p": p, "o": 5}, "format": {"font": "Garamond Premier"}}));
    let r = doc.apply_json(r#"{"op":"add_header_footer","kind":"header"}"#, false).unwrap();
    let hp = r[0].caret.unwrap();
    ops(&mut doc, serde_json::json!({"op": "insert_text", "at": hp, "text": "Report"}));
    ops(&mut doc, serde_json::json!({"op": "format_text", "from": hp, "to": {"p": hp.p.0, "o": 6}, "format": {"font": "Garamond Premier"}}));
    let uses = doc.fonts_used();
    assert_eq!(uses.iter().find(|u| u.family == "Garamond Premier").map(|u| u.chars), Some(11));
    ops(&mut doc, serde_json::json!({"op": "replace_font", "from": "Garamond Premier", "to": "EB Garamond"}));
    assert_eq!(font_names(&doc), ["EB Garamond", "Calibri"]);
    let out = doc.save().unwrap();
    assert_valid(&out);
    let pkg = Package::open(&out).unwrap();
    let header = String::from_utf8(pkg.get("word/header1.xml").unwrap().to_vec()).unwrap();
    assert!(header.contains(r#"w:ascii="EB Garamond""#) && !header.contains("Garamond Premier"), "{header}");
    assert_eq!(story_text(&Document::open(&out).unwrap(), StoryKind::Header), "Report");
}

fn find_para_id(doc: &Document) -> u32 {
    let mut id = None;
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| {
        id.get_or_insert(p.id);
    });
    id.expect("a paragraph")
}

// ---------------------------------------------------------------------------
// List styles, restart/continue numbering, checklists
// ---------------------------------------------------------------------------

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, serde_json::json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, serde_json::json!({"op":"delete_blocks","ids":[first]}));
    doc
}

/// Every paragraph's list label ("" when it is not a list item), in document order.
fn labels(doc: &Document) -> Vec<String> {
    let l = doc.numbering.labels(&doc.body, &doc.styles);
    let mut v = Vec::new();
    docxtor_core::model::walk_paragraphs(&doc.body, &mut |p| v.push(l.get(&p.id).map(|l| l.text.clone()).unwrap_or_default()));
    v
}

fn label_of(doc: &Document, id: u32) -> docxtor_core::numbering::ListLabel {
    doc.numbering.labels(&doc.body, &doc.styles).remove(&id).unwrap_or_else(|| panic!("p{id} is not a list item"))
}

/// (every run struck through, paragraph mark struck through)
fn struck(doc: &Document, id: u32) -> (bool, bool) {
    use docxtor_core::model::walk_runs;
    let p = docxtor_core::model::find_para(&doc.body, id).unwrap();
    let has = |e: &docxtor_core::xml::Element| e.child("w:strike").is_some_and(|s| s.attr("w:val") != Some("0"));
    let mut all = true;
    let mut any = false;
    walk_runs(&p.inlines, &mut 0, &mut |r, _, _| {
        if r.len16() > 0 {
            any = true;
            all &= r.rpr.as_ref().is_some_and(has);
        }
    });
    let mark = p.ppr.as_ref().and_then(|pp| pp.child("w:rPr")).is_some_and(has);
    (any && all, mark)
}

fn num_xml(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    String::from_utf8(pkg.get("word/numbering.xml").expect("numbering.xml").to_vec()).unwrap()
}

#[test]
fn list_style_presets_label_levels() {
    // (preset, labels at levels 0, 1 and 2)
    let cases = [
        ("decimal", ["1.", "a.", "i."]),
        ("paren", ["1)", "a)", "i)"]),
        ("outline", ["1.", "1.1.", "1.1.1."]),
        ("upper", ["A.", "a.", "i."]),
        ("roman", ["I.", "A.", "1."]),
        ("zero", ["01.", "a.", "i."]),
        ("disc", ["\u{25CF}", "\u{25CB}", "\u{25A0}"]),
        ("diamond", ["\u{2756}", "\u{27A2}", "\u{25A0}"]),
        ("arrow", ["\u{2794}", "\u{25C6}", "\u{25CF}"]),
        ("star", ["\u{2605}", "\u{25CB}", "\u{25A0}"]),
        ("arrowhead", ["\u{27A2}", "\u{25CB}", "\u{25A0}"]),
        ("box", ["\u{2751}", "\u{27A2}", "\u{25A0}"]),
        ("checklist", ["\u{2610}", "\u{2610}", "\u{2610}"]),
    ];
    for (style, expect) in cases {
        let mut doc = doc_with("alpha\n\nbeta\n\ngamma");
        let ids: Vec<u32> = ["alpha", "beta", "gamma"].iter().map(|t| find_para(&doc, t)).collect();
        ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":ids,"style":style}));
        ops(&mut doc, serde_json::json!({"op":"indent","ids":[ids[1]],"delta":1}));
        ops(&mut doc, serde_json::json!({"op":"indent","ids":[ids[2]],"delta":2}));
        assert_eq!(labels(&doc), expect, "{style}");
        // The definition carries the preset name, so it can be recognised later.
        doc.render_changed();
        let fs = doc.format_at(Pos::new(ids[0], 0)).unwrap();
        assert_eq!(fs.list_style.as_deref(), Some(style));
        assert_valid(&doc.save().unwrap());
    }

    // Numbers keep counting across items of one level.
    let mut doc = doc_with("a\n\nb\n\nc");
    let ids: Vec<u32> = ["a", "b", "c"].iter().map(|t| find_para(&doc, t)).collect();
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":ids,"style":"roman"}));
    assert_eq!(labels(&doc), ["I.", "II.", "III."]);
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":ids,"style":"zero"}));
    assert_eq!(labels(&doc), ["01.", "02.", "03."]);

    // Glyph fonts are written so Word shows the symbols.
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[ids[0]],"style":"diamond"}));
    let xml = num_xml(&mut doc);
    assert!(xml.contains(r#"<w:name w:val="Docxtor diamond"/>"#), "{xml}");
    assert!(xml.contains(r#"w:ascii="Segoe UI Symbol""#) && xml.contains(r#"w:hint="default""#));
    assert!(xml.contains(r#"w:ascii="Arial""#));

    // Unknown styles are a plain error and change nothing.
    let before = labels(&doc);
    let err = doc.apply_json(&serde_json::json!({"op":"set_list_style","ids":[ids[0]],"style":"wavy"}).to_string(), false).unwrap_err();
    assert!(err.to_string().contains("unknown list style 'wavy'"), "{err}");
    assert_eq!(labels(&doc), before);
}

#[test]
fn set_list_style_restyles_whole_list_and_new_lists_start_at_one() {
    let mut doc = doc_with("1. one\n2. two\n3. three\n\nbetween\n\n1. uno\n2. dos\n\nlater");
    assert_eq!(labels(&doc), ["1.", "2.", "3.", "", "1.", "2.", ""]);
    let two = find_para(&doc, "two");
    // One item of the list is enough: all three change.
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[two],"style":"upper"}));
    assert_eq!(labels(&doc), ["A.", "B.", "C.", "", "1.", "2.", ""]);
    // A paragraph outside any list becomes a new list starting at 1, even right under another list.
    let between = find_para(&doc, "between");
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[between],"style":"upper"}));
    assert_eq!(labels(&doc), ["A.", "B.", "C.", "A.", "1.", "2.", ""]);
    // Two lists in one request stay two lists.
    let (one, uno) = (find_para(&doc, "one"), find_para(&doc, "uno"));
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[one, uno],"style":"roman"}));
    assert_eq!(labels(&doc), ["I.", "II.", "III.", "A.", "I.", "II.", ""]);
    // Restyling to what it already is changes nothing.
    let xml_before = num_xml(&mut doc);
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[one],"style":"roman"}));
    assert_eq!(num_xml(&mut doc), xml_before);
    // Numbered to bullets and back; levels are kept.
    ops(&mut doc, serde_json::json!({"op":"indent","ids":[two],"delta":1}));
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[one],"style":"disc"}));
    assert_eq!(labels(&doc)[..3], ["\u{25CF}", "\u{25CB}", "\u{25CF}"]);
    // none takes the paragraph out of its list.
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[two],"style":"none"}));
    assert_eq!(labels(&doc)[..3], ["\u{25CF}", "", "\u{25CF}"]);

    // Undo is one step per batch.
    let after = labels(&doc);
    assert!(doc.undo());
    assert_eq!(labels(&doc)[..3], ["\u{25CF}", "\u{25CB}", "\u{25CF}"]);
    assert!(doc.redo());
    assert_eq!(labels(&doc), after);

    // The result survives a save and reopen.
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    assert_eq!(labels(&again), after);
}

#[test]
fn restart_and_continue_numbering() {
    let mut doc = doc_with("1. a\n2. b\n3. c\n4. d\n5. e");
    assert_eq!(labels(&doc), ["1.", "2.", "3.", "4.", "5."]);
    let d = find_para(&doc, "d");
    let (c, e) = (find_para(&doc, "c"), find_para(&doc, "e"));
    // Restarting at "d" renumbers it and everything after it.
    ops(&mut doc, serde_json::json!({"op":"restart_numbering","id":d}));
    assert_eq!(labels(&doc), ["1.", "2.", "3.", "1.", "2."]);
    // Joining it back continues the count.
    ops(&mut doc, serde_json::json!({"op":"continue_numbering","id":d}));
    assert_eq!(labels(&doc), ["1.", "2.", "3.", "4.", "5."]);
    // Restart at a chosen number.
    ops(&mut doc, serde_json::json!({"op":"restart_numbering","id":c,"start":10}));
    assert_eq!(labels(&doc), ["1.", "2.", "10.", "11.", "12."]);
    ops(&mut doc, serde_json::json!({"op":"restart_numbering","id":e,"start":1}));
    assert_eq!(labels(&doc), ["1.", "2.", "10.", "11.", "1."]);
    // Survives a save: numbering.xml holds the start overrides.
    let xml = num_xml(&mut doc);
    assert!(xml.contains("<w:startOverride w:val=\"10\"/>"), "{xml}");
    let mut again = Document::open(&doc.save().unwrap()).unwrap();
    assert_eq!(labels(&again), ["1.", "2.", "10.", "11.", "1."]);
    // One undo step per op.
    assert!(doc.undo());
    assert_eq!(labels(&doc), ["1.", "2.", "10.", "11.", "12."]);
    let e2 = find_para(&again, "e");
    ops(&mut again, serde_json::json!({"op":"continue_numbering","id":e2}));
    assert_eq!(labels(&again), ["1.", "2.", "10.", "11.", "12."]);

    // Nothing earlier to continue, or not a list: plain errors, document unchanged.
    let mut doc = doc_with("1. a\n2. b\n\nplain\n\n1. x");
    let (a, plain) = (find_para(&doc, "a"), find_para(&doc, "plain"));
    let err = doc.apply_json(&serde_json::json!({"op":"continue_numbering","id":a}).to_string(), false).unwrap_err();
    assert!(err.to_string().contains("there's no earlier list to continue"), "{err}");
    let err = doc.apply_json(&serde_json::json!({"op":"restart_numbering","id":plain}).to_string(), false).unwrap_err();
    assert!(err.to_string().contains("isn't in a list"), "{err}");
    assert_eq!(labels(&doc), ["1.", "2.", "", "1."]);
    // Different list definitions are not joined by continue (the doc's two markdown lists are separate definitions).
    let x = find_para(&doc, "x");
    assert!(doc.apply_json(&serde_json::json!({"op":"continue_numbering","id":x}).to_string(), false).is_err());
    // ...but lists of the same preset are.
    let (b, x) = (find_para(&doc, "b"), find_para(&doc, "x"));
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[b],"style":"decimal"}));
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[x],"style":"decimal"}));
    assert_eq!(labels(&doc), ["1.", "2.", "", "1."]);
    ops(&mut doc, serde_json::json!({"op":"continue_numbering","id":x}));
    assert_eq!(labels(&doc), ["1.", "2.", "", "3."]);
}

#[test]
fn checklists() {
    let mut doc = doc_with("buy milk\n\nwrite *code*\n\nship it");
    let ids: Vec<u32> = ["buy milk", "write code", "ship it"].iter().map(|t| find_para(&doc, t)).collect();
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":ids,"style":"checklist"}));
    assert_eq!(labels(&doc), ["\u{2610}"; 3]);
    let l = label_of(&doc, ids[0]);
    assert!(l.bullet && l.checklist && !l.checked);
    doc.render_changed();
    let fs = doc.format_at(Pos::new(ids[1], 0)).unwrap();
    assert_eq!(serde_json::to_value(&fs).unwrap()["list"], "checklist");
    assert_eq!(fs.list_style.as_deref(), Some("checklist"));
    assert_eq!(fs.checked, Some(false));

    // Checking swaps the glyph and strikes every run (not the paragraph mark).
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[1]],"checked":true}));
    assert_eq!(labels(&doc), ["\u{2610}", "\u{2611}", "\u{2610}"]);
    assert_eq!(struck(&doc, ids[1]), (true, false));
    assert_eq!(struck(&doc, ids[0]), (false, false));
    doc.render_changed();
    let fs = doc.format_at(Pos::new(ids[1], 0)).unwrap();
    assert_eq!((fs.checked, fs.list_style.as_deref()), (Some(true), Some("checklist")));
    let json = serde_json::to_value(doc.render_snapshot()).unwrap().to_string();
    assert!(json.contains("\"checked\":true,\"checklist\":true"), "{json}");
    // Checking again changes nothing; unchecking restores.
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[1]],"checked":true}));
    assert_eq!(labels(&doc), ["\u{2610}", "\u{2611}", "\u{2610}"]);
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[1]],"checked":false}));
    assert_eq!(labels(&doc), ["\u{2610}"; 3]);
    assert_eq!(struck(&doc, ids[1]), (false, false));
    let xml = num_xml(&mut doc);
    assert!(xml.contains("Docxtor checklist checked") && xml.contains("\u{2611}"));

    // The level is kept when toggling.
    ops(&mut doc, serde_json::json!({"op":"indent","ids":[ids[2]],"delta":1}));
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[2]],"checked":true}));
    assert_eq!(label_of(&doc, ids[2]).level, 1);
    assert_eq!(label_of(&doc, ids[2]).text, "\u{2611}");
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[2]],"checked":false}));
    ops(&mut doc, serde_json::json!({"op":"indent","ids":[ids[2]],"delta":-1}));

    // Items that aren't checklist items are an error, and nothing changes.
    let mut bullets = doc_with("- plain bullet\n- other");
    let b = find_para(&bullets, "plain bullet");
    let err = bullets.apply_json(&serde_json::json!({"op":"set_checked","ids":[b],"checked":true}).to_string(), false).unwrap_err();
    assert!(err.to_string().contains("checklist"), "{err}");
    let err = doc.apply_json(&serde_json::json!({"op":"set_checked","ids":[ids[0], 999999],"checked":true}).to_string(), false).unwrap_err();
    assert!(!err.to_string().is_empty());
    assert_eq!(labels(&doc), ["\u{2610}"; 3]);

    // Enter at the end of a checked item gives an unchecked one with no strikethrough.
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[1]],"checked":true}));
    let len = doc.find_any_para(ids[1]).unwrap().len16();
    let r = doc.apply(&Op::SplitParagraph { at: Pos::new(ids[1], len) }, false).unwrap();
    let new = r.created[0];
    assert_eq!(label_of(&doc, new).text, "\u{2610}");
    assert_eq!(label_of(&doc, ids[1]).text, "\u{2611}");
    assert_eq!(struck(&doc, ids[1]), (true, false));
    assert_eq!(struck(&doc, new), (false, false));
    // Typing in the new item isn't struck either.
    ops(&mut doc, serde_json::json!({"op":"insert_text","at":{"p":new,"o":0},"text":"new task"}));
    assert_eq!(struck(&doc, new), (false, false));
    // Enter in the middle of a checked item: the tail moves to an unchecked, unstruck item.
    let mid = doc.apply(&Op::SplitParagraph { at: Pos::new(ids[1], 5) }, false).unwrap().created[0];
    assert_eq!(label_of(&doc, ids[1]).text, "\u{2611}");
    assert_eq!(struck(&doc, ids[1]), (true, false));
    assert_eq!(label_of(&doc, mid).text, "\u{2610}");
    assert_eq!(struck(&doc, mid), (false, false));
    assert_eq!(doc.find_any_para(mid).unwrap().plain_text(), " code");
    // Enter in an unchecked item stays unchecked.
    let again = doc.apply(&Op::SplitParagraph { at: Pos::new(new, 3) }, false).unwrap().created[0];
    assert_eq!(label_of(&doc, again).text, "\u{2610}");

    // Undo restores in one step each.
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(labels(&doc), ["\u{2610}", "\u{2611}", "\u{2610}"]);
    assert_eq!(struck(&doc, ids[1]), (true, false));
    assert!(doc.undo());
    assert_eq!(labels(&doc), ["\u{2610}"; 3]);
    assert_eq!(struck(&doc, ids[1]), (false, false));

    // Turning a checked item into a plain bullet or out of the list drops the strikethrough.
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[0]],"checked":true}));
    assert_eq!(struck(&doc, ids[0]), (true, false));
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":ids,"style":"disc"}));
    assert_eq!(labels(&doc), ["\u{25CF}"; 3]);
    assert_eq!(struck(&doc, ids[0]), (false, false));
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":ids,"style":"checklist"}));
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[0]],"checked":true}));
    ops(&mut doc, serde_json::json!({"op":"toggle_list","ids":[ids[0]],"kind":"bullet"}));
    assert_eq!(struck(&doc, ids[0]), (false, false));
    assert_eq!(labels(&doc)[0], "");

    // Everything survives a save and reopen.
    ops(&mut doc, serde_json::json!({"op":"set_list_style","ids":[ids[0]],"style":"checklist"}));
    ops(&mut doc, serde_json::json!({"op":"set_checked","ids":[ids[0], ids[2]],"checked":true}));
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    assert_eq!(labels(&again), ["\u{2611}", "\u{2610}", "\u{2611}"]);
    assert_eq!(struck(&again, ids[0]), (true, false));
}

#[test]
fn toggle_bullet_under_a_checklist_is_not_a_checklist() {
    let mut doc = doc_with("- [ ] task\n\nplain");
    let plain = find_para(&doc, "plain");
    ops(&mut doc, serde_json::json!({"op":"toggle_list","ids":[plain],"kind":"bullet"}));
    assert_eq!(labels(&doc), ["\u{2610}", "\u{2022}"]);
}

#[test]
fn markdown_task_lists() {
    use docxtor_core::export::to_markdown;
    let mut doc = doc_with("Plan\n\n- [ ] draft\n- [x] review **hard**\n  - [X] nested\n- [ ] ship\n- plain bullet\n\n* [ ] star marker");
    assert_eq!(labels(&doc), ["", "\u{2610}", "\u{2611}", "\u{2611}", "\u{2610}", "\u{2022}", "\u{2610}"]);
    let reviewed = find_para(&doc, "review hard");
    assert_eq!(struck(&doc, reviewed), (true, false));
    assert_eq!(label_of(&doc, reviewed).level, 0);
    assert_eq!(label_of(&doc, find_para(&doc, "nested")).level, 1);
    let md = to_markdown(&doc);
    assert_eq!(md, "Plan\n\n- [ ] draft\n- [x] review **hard**\n  - [x] nested\n- [ ] ship\n- plain bullet\n- [ ] star marker\n", "{md}");
    assert!(!md.contains("~~"));
    // The text export shows the boxes.
    assert!(docxtor_core::export::to_text(&doc).contains("\u{2611} review hard"));
    // DocScript shows the state to the AI.
    let script = docxtor_core::ai::call_tool(&mut doc, "read_document", &serde_json::json!({})).unwrap();
    assert!(script.contains("list:\"\u{2611}\"") && script.contains("list:\"\u{2610}\""), "{script}");

    // Pasting task items (mid-paragraph and as blocks) keeps their state.
    let mut doc = doc_with("before and after");
    let p = find_para(&doc, "before and after");
    ops(&mut doc, serde_json::json!({"op":"paste_markdown","at":{"p":p,"o":6},"markdown":"- [x] done\n- [ ] todo"}));
    let all = labels(&doc);
    assert_eq!(all.len(), 4, "{all:?}");
    assert_eq!(&all[1..3], ["\u{2611}", "\u{2610}"]);
    assert_eq!(para_texts(&doc), ["before", "done", "todo", " and after"]);

    // A single pasted task line becomes a checklist item.
    let mut doc = doc_with("one\n\ntwo");
    let two = find_para(&doc, "two");
    ops(&mut doc, serde_json::json!({"op":"paste_markdown","at":{"p":two,"o":3},"markdown":"- [x] extra"}));
    assert!(labels(&doc).contains(&"\u{2611}".to_string()), "{:?}", labels(&doc));

    // Save and reopen; export again.
    let out = doc.save().unwrap();
    assert_valid(&out);
    let again = Document::open(&out).unwrap();
    assert_eq!(labels(&again), labels(&doc));
}

#[test]
fn list_ai_tools_and_ops_in_batches() {
    let mut doc = doc_with("alpha\n\nbeta");
    let ids: Vec<u32> = ["alpha", "beta"].iter().map(|t| find_para(&doc, t)).collect();
    docxtor_core::ai::call_tool(&mut doc, "format_paragraphs", &serde_json::json!({"ids": ids, "list_style": "paren"})).unwrap();
    assert_eq!(labels(&doc), ["1)", "2)"]);
    docxtor_core::ai::call_tool(&mut doc, "format_paragraphs", &serde_json::json!({"ids": ids, "list_style": "checklist"})).unwrap();
    docxtor_core::ai::call_tool(&mut doc, "format_paragraphs", &serde_json::json!({"ids": [ids[0]], "checked": true})).unwrap();
    assert_eq!(labels(&doc), ["\u{2611}", "\u{2610}"]);
    // A batch is one undo step and rolls back whole on error.
    let batch = serde_json::json!([
        {"op":"set_list_style","ids":ids,"style":"decimal"},
        {"op":"restart_numbering","id":ids[1],"start":5},
    ]);
    ops(&mut doc, batch);
    assert_eq!(labels(&doc), ["1.", "5."]);
    assert!(doc.undo());
    assert_eq!(labels(&doc), ["\u{2611}", "\u{2610}"]);
    let bad = serde_json::json!([
        {"op":"set_list_style","ids":ids,"style":"decimal"},
        {"op":"set_checked","ids":ids,"checked":true},
    ]);
    assert!(doc.apply_json(&bad.to_string(), false).is_err());
    assert_eq!(labels(&doc), ["\u{2611}", "\u{2610}"]);
}
