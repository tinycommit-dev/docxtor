//! Captions numbered by chapter ("Figure 2-1"), and cross-references to numbered items.

use docxtor_core::model::{find_block, walk_paragraphs, write_block};
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

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
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

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn para_xml(doc: &Document, id: u32) -> String {
    write_block(find_block(&doc.body, id).unwrap()).to_xml()
}

fn chapters_doc() -> Document {
    let mut doc = doc_with("# One\n\nFirst text.\n\n## Sub\n\nSub text.\n\n# Two\n\nSecond text.");
    let heads: Vec<u32> = ["One", "Two"].iter().map(|t| find_p(&doc, t)).collect();
    ops(&mut doc, json!({"op":"toggle_list","ids":heads,"kind":"decimal"}));
    doc
}

#[test]
fn captions_with_a_chapter_number_restart_under_each_heading() {
    let mut doc = chapters_doc();
    let first = find_p(&doc, "First text.");
    let second = find_p(&doc, "Second text.");
    ops(&mut doc, json!({"op":"insert_caption","after":first,"label":"Figure","text":"A","chapter":{"level":1}}));
    ops(&mut doc, json!({"op":"insert_caption","after":first,"label":"Figure","text":"B","chapter":{"level":1,"separator":"."}}));
    ops(&mut doc, json!({"op":"insert_caption","after":second,"label":"Figure","text":"C","chapter":{"level":1}}));
    let t = texts(&doc);
    // The second was inserted right after the first block, so it comes before A in the text.
    assert!(t.contains(&"Figure 1.1: B".to_string()), "{t:?}");
    assert!(t.contains(&"Figure 1-2: A".to_string()), "{t:?}");
    assert!(t.contains(&"Figure 2-1: C".to_string()), "{t:?}");
    let cap = find_p(&doc, "Figure 2-1");
    let xml = para_xml(&doc, cap);
    assert!(xml.contains(r"STYLEREF 1 \s") && xml.contains(r"SEQ Figure \* ARABIC \s 1"), "{xml}");
    // A caption in the middle of chapter 1 moves the later ones; Update fields agrees.
    ops(&mut doc, json!({"op":"insert_caption","after":first,"label":"Figure","text":"D","chapter":{"level":1}}));
    ops(&mut doc, json!({"op":"update_fields"}));
    let t = texts(&doc);
    let nums: Vec<&String> = t.iter().filter(|x| x.starts_with("Figure")).collect();
    assert_eq!(nums.len(), 4);
    assert!(t.contains(&"Figure 2-1: C".to_string()));
    assert_valid(&doc.save().unwrap());
    // The saved file reads back the same, and when the first chapter goes the second becomes the first.
    let mut re = Document::open(&doc.save().unwrap()).unwrap();
    assert!(texts(&re).contains(&"Figure 2-1: C".to_string()));
    let one = find_p(&re, "One");
    ops(&mut re, json!({"op":"toggle_list","ids":[one],"kind":"decimal"}));
    ops(&mut re, json!({"op":"update_fields"}));
    assert!(texts(&re).contains(&"Figure 1-1: C".to_string()), "{:?}", texts(&re));
}

#[test]
fn a_chapter_number_needs_numbered_headings_and_can_be_undone() {
    let mut doc = doc_with("# Plain\n\nText here.");
    let t = find_p(&doc, "Text here.");
    let e = doc.apply_json(&json!({"op":"insert_caption","after":t,"label":"Figure","chapter":{"level":1}}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("numbered heading"), "{e}");
    // Nothing was added.
    assert!(!texts(&doc).iter().any(|x| x.starts_with("Figure")));
    let mut doc = chapters_doc();
    let first = find_p(&doc, "First text.");
    ops(&mut doc, json!({"op":"insert_caption","after":first,"label":"Table","text":"T","chapter":{"level":1}}));
    assert!(texts(&doc).contains(&"Table 1-1: T".to_string()));
    assert!(doc.undo());
    assert!(!texts(&doc).iter().any(|x| x.starts_with("Table")));
}

#[test]
fn cross_references_to_a_chapter_caption_show_its_whole_number() {
    let mut doc = chapters_doc();
    let second = find_p(&doc, "Second text.");
    let cap = ops(&mut doc, json!({"op":"insert_caption","after":second,"label":"Figure","text":"Chart","chapter":{"level":1}}))[0].created[0];
    let host = find_p(&doc, "First text.");
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":host,"o":0},"target":{"type":"caption","p":cap},"show":"label_number"}));
    assert!(texts(&doc).contains(&"Figure 2-1First text.".to_string()), "{:?}", texts(&doc));
    let targets = doc.xref_targets();
    assert_eq!(targets.captions[0].number, "2-1");
    assert_eq!(targets.captions[0].label, "Figure");
    ops(&mut doc, json!({"op":"update_fields"}));
    assert!(texts(&doc).contains(&"Figure 2-1First text.".to_string()));
}

fn list_doc() -> Document {
    let mut doc = doc_with("Intro.\n\nAlpha\n\nBeta\n\nGamma\n\nDelta\n\nEpsilon\n\nEnd.");
    let ids: Vec<u32> = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"].iter().map(|t| find_p(&doc, t)).collect();
    let def = json!({"levels":[{"format":"decimal","text":"%1."},{"format":"lowerLetter","text":"%2)"},{"format":"lowerRoman","text":"%3."}]});
    ops(&mut doc, json!({"op":"define_list","def":def,"ids":ids}));
    // Alpha 1., Beta a), Gamma i., Delta a) (b), Epsilon 2.
    ops(&mut doc, json!({"op":"indent","ids":[ids[1]],"delta":1}));
    ops(&mut doc, json!({"op":"indent","ids":[ids[2]],"delta":2}));
    ops(&mut doc, json!({"op":"indent","ids":[ids[3]],"delta":1}));
    doc
}

#[test]
fn numbered_items_are_listed_with_their_numbers() {
    let doc = list_doc();
    let items = doc.numbered_items();
    let by_text: Vec<(String, String, String)> = items.iter().map(|i| (i.text.clone(), i.number.clone(), i.full.clone())).collect();
    assert_eq!(items.len(), 5, "{by_text:?}");
    let gamma = items.iter().find(|i| i.text == "Gamma").unwrap();
    assert_eq!((gamma.number.as_str(), gamma.full.as_str()), ("i", "1.a)i"), "{items:?}");
    let delta = items.iter().find(|i| i.text == "Delta").unwrap();
    assert_eq!((delta.number.as_str(), delta.full.as_str()), ("b)", "1.b)"));
}

#[test]
fn cross_reference_to_a_numbered_item_with_and_without_context() {
    let mut doc = list_doc();
    let gamma = find_p(&doc, "Gamma");
    let intro = find_p(&doc, "Intro.");
    let items = doc.numbered_items();
    let g = items.iter().find(|i| i.text == "Gamma").unwrap().clone();
    let end = find_p(&doc, "End.");
    // From outside the list: full context for \w, own number for \n, and \r is the full number too.
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":intro,"o":0},"target":{"type":"numbered","p":gamma},"show":"number_full_context"}));
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":end,"o":0},"target":{"type":"numbered","p":gamma},"show":"number_no_context"}));
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":end,"o":0},"target":{"type":"numbered","p":gamma},"show":"text","link":false}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x.starts_with(&g.full) && x.ends_with("Intro.")), "{t:?}");
    assert!(t.iter().any(|x| x.starts_with("Gamma") && x.contains(&g.number)), "{t:?}");
    let xml = para_xml(&doc, intro);
    assert!(xml.contains(r" \w \h"), "{xml}");
    assert!(para_xml(&doc, end).contains(r" \n \h"));
    // Inside the same list, relative context leaves out the levels both share.
    let delta = find_p(&doc, "Delta");
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":delta,"o":0},"target":{"type":"numbered","p":gamma},"show":"number"}));
    let rel = texts(&doc).into_iter().find(|x| x.ends_with("Delta")).unwrap();
    assert!(rel.len() <= g.full.len() + "Delta".len(), "{rel}");
    // The field updates with the list.
    ops(&mut doc, json!({"op":"update_fields"}));
    assert_valid(&doc.save().unwrap());
    // A paragraph that isn't numbered can't be pointed at as an item.
    let e = doc.apply_json(&json!({"op":"insert_cross_reference","at":{"p":end,"o":0},"target":{"type":"numbered","p":intro},"show":"number"}).to_string(), false).unwrap_err().to_string();
    assert!(e.contains("isn't a numbered item"), "{e}");
}
