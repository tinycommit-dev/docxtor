//! Tests of: AI tools and nothing else changes.

use super::*;

#[test]
fn ai_bookmark_tools_and_docscript() {
    let mut doc = doc_with("# Intro\n\nAlpha beta gamma\n\nSee beta");
    let (h, a, s) = (find_p(&doc, "Intro"), find_p(&doc, "Alpha"), find_p(&doc, "See"));
    let r = call_tool(&mut doc, "add_bookmark", &json!({"id": format!("p{a}"), "text": "beta", "name": "Beta_word"})).unwrap();
    assert!(r.contains(&format!("[p{a} bookmarks=Beta_word]")), "{r}");
    call_tool(&mut doc, "add_bookmark", &json!({"id": h, "name": "Top"})).unwrap();
    call_tool(&mut doc, "add_bookmark", &json!({"id": h, "text": "Int", "name": "Start"})).unwrap();
    assert_eq!(doc.bookmarks().iter().find(|b| b.name == "Top").unwrap().text, "Intro", "the whole paragraph by default");
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    // In document order: Start was put in front of Top at the same place.
    assert!(script.contains(&format!("[p{h} Heading1 bookmarks=Start,Top] Intro")), "{script}");
    assert!(err_tool(&mut doc, "add_bookmark", json!({"id": a, "name": "Top"})).contains("already a bookmark"));
    assert!(err_tool(&mut doc, "add_bookmark", json!({"id": a, "name": "bad name"})).contains("spaces"));
    assert!(err_tool(&mut doc, "add_bookmark", json!({"id": a, "text": "nope", "name": "Nope"})).contains("not found"));
    // Links to bookmarks are plain Markdown links.
    let r = call_tool(&mut doc, "set_link", &json!({"id": s, "text": "beta", "url": "#Beta_word"})).unwrap();
    assert!(r.contains("[beta](#Beta_word)"), "{r}");
    // Hidden bookmarks aren't shown.
    ops(&mut doc, json!({"op":"insert_toc"}));
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains("(#_Toc") && !script.contains("bookmarks=_") && !script.contains(",_Toc"), "{script}");
    // Delete.
    let r = call_tool(&mut doc, "delete_bookmark", &json!({"name": "top"})).unwrap();
    assert!(r.contains("deleted"));
    assert!(err_tool(&mut doc, "delete_bookmark", json!({"name": "Top"})).contains("no bookmark"));
    // Raw ops: rename.
    call_tool(&mut doc, "apply_ops", &json!({"ops": [{"op":"rename_bookmark","from":"Beta_word","to":"Word"}]})).unwrap();
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains("bookmarks=Word") && script.contains("[beta](#Word)"), "{script}");
    assert_valid(&doc.save().unwrap());
}

pub(super) fn err_tool(doc: &mut Document, name: &str, args: Value) -> String {
    call_tool(doc, name, &args).expect_err(name)
}

#[test]
fn ai_horizontal_line_and_watermark_tools() {
    let mut doc = doc_with("Alpha beta gamma\n\nLast");
    let a = find_p(&doc, "Alpha");
    let r = call_tool(&mut doc, "insert_horizontal_line", &json!({"id": a, "after_text": "beta"})).unwrap();
    assert!(r.contains("] Alpha beta") && r.contains(" hr]") && r.contains("]  gamma"), "{r}");
    assert_eq!(texts(&doc), ["Alpha beta", "", " gamma", "Last"]);
    // Without after_text, after the paragraph.
    let last = find_p(&doc, "Last");
    call_tool(&mut doc, "insert_horizontal_line", &json!({"id": last})).unwrap();
    assert_eq!(texts(&doc).len(), 6);
    assert!(err_tool(&mut doc, "insert_horizontal_line", json!({"id": a, "after_text": "missing"})).contains("not found"));

    let r = call_tool(&mut doc, "set_watermark", &json!({"text": "DRAFT"})).unwrap();
    assert!(r.contains("DRAFT") && r.contains("diagonal"), "{r}");
    assert_eq!(doc.page_setup().watermark.unwrap().layout, WatermarkLayout::Diagonal);
    let script = call_tool(&mut doc, "read_document", &json!({})).unwrap();
    assert!(script.contains("# Header (default)") && script.contains(r#"# Watermark: "DRAFT" (diagonal, C0C0C0)"#), "{script}");
    call_tool(&mut doc, "set_watermark", &json!({"text": "CONFIDENTIAL", "layout": "horizontal", "color": "FF0000"})).unwrap();
    let w = doc.page_setup().watermark.unwrap();
    assert_eq!((w.text.as_deref(), w.layout, w.color.as_deref()), (Some("CONFIDENTIAL"), WatermarkLayout::Horizontal, Some("FF0000")));
    assert!(err_tool(&mut doc, "set_watermark", json!({"text": "X", "layout": "upside_down"})).contains("layout"));
    assert!(err_tool(&mut doc, "set_watermark", json!({"text": "X", "color": "plaid"})).contains("not a colour"));
    assert_eq!(call_tool(&mut doc, "set_watermark", &json!({"text": "none"})).unwrap(), "Watermark removed.");
    assert!(doc.page_setup().watermark.is_none());
    assert!(err_tool(&mut doc, "set_watermark", json!({"text": "None"})).contains("no watermark"));
    // The tools are in the list the CLI and MCP serve.
    let defs = docxtor_core::ai::tool_definitions();
    let names: Vec<&str> = defs.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["add_bookmark", "delete_bookmark", "insert_horizontal_line", "set_watermark"] {
        assert!(names.contains(&n), "{n}");
    }
    let apply = defs.as_array().unwrap().iter().find(|t| t["name"] == "apply_ops").unwrap()["description"].as_str().unwrap().to_string();
    assert!(apply.contains("rename_bookmark") && apply.contains("insert_bookmark"));
    assert!(docxtor_core::ai::SYSTEM_GUIDE.contains("add_bookmark"));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn untouched_documents_save_byte_identical() {
    for name in ["simple.docx", "rich.docx", "complex.docx", "long.docx"] {
        let bytes = fixture(name);
        let mut doc = Document::open(&bytes).unwrap();
        // Reading the new views doesn't change anything.
        let _ = doc.bookmarks();
        let _ = doc.page_setup();
        doc.render_all();
        let out = doc.save().unwrap();
        let (a, b) = (Package::open(&bytes).unwrap(), Package::open(&out).unwrap());
        for part in &a.parts {
            assert_eq!(b.get(&part.name), Some(&part.data[..]), "{name}: {} changed", part.name);
        }
    }
}

#[test]
fn op_json_shapes() {
    let mut doc = doc_with("Alpha beta");
    let a = find_p(&doc, "Alpha");
    // Plain JSON ops, as the editor sends them; ids may be strings.
    doc.apply_json(&json!([{"op":"insert_bookmark","from":{"p":format!("p{a}"),"o":0},"to":{"p":a,"o":5},"name":"Name"}]).to_string(), false).unwrap();
    let ops: Vec<Op> = serde_json::from_value(json!([
        {"op":"insert_horizontal_line","at":{"p":a,"o":0}},
        {"op":"set_watermark","text":"X"},
        {"op":"set_watermark","text":"X","layout":"horizontal","color":"FF0000","font":"Arial","opacity":0.4},
        {"op":"set_watermark","text":null},
        {"op":"set_watermark"},
        {"op":"rename_bookmark","from":"a","to":"b"},
        {"op":"delete_bookmark","name":"b"},
    ]))
    .unwrap();
    assert_eq!(ops.len(), 7);
    let again: Vec<Op> = serde_json::from_str(&serde_json::to_string(&ops).unwrap()).unwrap();
    assert_eq!(again.len(), 7);
    let b = serde_json::to_value(doc.bookmarks()).unwrap();
    assert_eq!(b, json!([{"name":"Name","p":a,"o":0,"endP":a,"endO":5,"text":"Alpha"}]));
    doc.apply_json(&json!({"op":"set_watermark","text":"X"}).to_string(), false).unwrap();
    let w = serde_json::to_value(doc.page_setup().watermark).unwrap();
    assert_eq!(w["kind"], "text");
    assert_eq!(w["layout"], "diagonal");
    assert_eq!(w["text"], "X");
    assert_eq!(w["angle"], 315.0);
}

/// A link to a place in the document names a bookmark that is there (the random search met links to nowhere in 346
/// runs of 1,612): the link op refuses one and says which name is missing, Markdown keeps its words as
/// plain text, and a bookmark that exists, in any case, can be linked to. (Deleting a bookmark turns its links into
/// plain text.)
#[test]
fn a_link_to_a_place_in_the_document_needs_a_bookmark_that_is_there() {
    let mut doc = doc_with("Alpha paragraph here\n\nBeta paragraph here");
    let a = find_p(&doc, "Alpha");
    let b = find_p(&doc, "Beta");
    let e = err(&mut doc, json!({"op":"set_link","from":{"p":b,"o":0},"to":{"p":b,"o":4},"url":"#Nowhere"}));
    assert!(e.contains("no bookmark named “Nowhere”"), "{e}");
    ops(&mut doc, json!({"op":"insert_bookmark","name":"Start","from":{"p":a,"o":0},"to":{"p":a,"o":5}}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":b,"o":0},"to":{"p":b,"o":4},"url":"#start"}));
    // Markdown: a link to a bookmark that is there is a link; one to nowhere is its words.
    ops(&mut doc, json!({"op":"set_paragraph_text","id":b,"markdown":"[to the start](#Start) and [to nowhere](#b)"}));
    let xml = para_xml(&doc, b);
    assert!(xml.contains(r#"w:anchor="Start""#), "{xml}");
    assert_eq!(xml.matches("<w:hyperlink").count(), 1, "{xml}");
    assert!(texts(&doc).iter().any(|t| t == "to the start and to nowhere"));
}

/// A bookmark that goes with the text that held it (the random search: a table of contents, then the tab with its
/// headings deleted) takes its links with it: they become plain text, as deleting the bookmark itself does, and an undo
/// brings both back.
#[test]
fn deleting_the_text_that_holds_a_bookmark_turns_its_links_into_text() {
    let mut doc = doc_with("Alpha paragraph here\n\nBeta paragraph here");
    let a = find_p(&doc, "Alpha");
    let b = find_p(&doc, "Beta");
    ops(&mut doc, json!({"op":"insert_bookmark","name":"Start","from":{"p":a,"o":0},"to":{"p":a,"o":5}}));
    ops(&mut doc, json!({"op":"set_link","from":{"p":b,"o":0},"to":{"p":b,"o":4},"url":"#Start"}));
    assert!(para_xml(&doc, b).contains(r#"w:anchor="Start""#));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[a]}));
    let xml = para_xml(&doc, b);
    assert!(!xml.contains("w:hyperlink"), "the link is plain text now: {xml}");
    assert_eq!(texts(&doc), vec!["Beta paragraph here".to_string()]);
    assert!(doc.undo());
    assert!(para_xml(&doc, b).contains(r#"w:anchor="Start""#), "undo gives the link back");
}

/// The random search over real files: one batch inserts a table of contents, which bookmarks every heading,
/// and deletes a heading. That bookmark was never there before the batch or after it; the entry's link to it becomes plain
/// text, and the other entry stays a link. (Debug builds check every batch for links to nowhere, rule E14.)
#[test]
fn a_contents_entry_for_a_heading_the_same_batch_deletes_is_plain_text() {
    let mut doc = doc_with("# One\n\nText one\n\n# Two\n\nText two");
    let one = find_p(&doc, "One");
    let two = find_p(&doc, "Two");
    ops(&mut doc, json!([{"op":"insert_toc","before":one,"levels":2,"title":"Contents"},{"op":"delete_blocks","ids":[two]}]));
    let xml: String = doc.body.iter().map(|b| write_block(b).to_xml()).collect();
    assert_eq!(xml.matches("w:anchor=").count(), 1, "the entry for One is a link, the one for Two is not: {xml}");
    let broken: Vec<String> = doc.check_invariants().iter().filter(|v| v.rule == "E14").map(|v| v.to_string()).collect();
    assert!(broken.is_empty(), "{broken:?}");
}

/// A link to nowhere pasted into a header (a fragment with an anchor the document has no bookmark for) becomes plain text, as in the
/// body: the stories a batch touches are checked for new links too.
#[test]
fn a_link_to_nowhere_pasted_into_a_header_is_plain_text() {
    let mut doc = Document::new_blank();
    ops(&mut doc, json!({"op":"insert_page_part","kind":"header","design":"docxtor.blank"}));
    let header = doc.stories.iter().find(|s| s.kind == docxtor_core::stories::StoryKind::Header).expect("a header");
    let p = header.blocks.iter().find_map(|b| match b {
        docxtor_core::model::Block::Para(p) => Some(p.id),
        _ => None,
    });
    let fragment = json!({"version":1,"ns":[["w","http://schemas.openxmlformats.org/wordprocessingml/2006/main"]],"body":"<w:p><w:hyperlink w:anchor=\"nowhere\"><w:r><w:t>Up</w:t></w:r></w:hyperlink></w:p>","links":{},"media":{},"origin":""});
    ops(&mut doc, json!({"op":"insert_fragment","at":{"p":p.unwrap(),"o":0},"fragment":fragment}));
    let broken: Vec<String> = doc.check_invariants().iter().filter(|v| v.rule == "E14").map(|v| v.to_string()).collect();
    assert!(broken.is_empty(), "{broken:?}");
    let header = doc.stories.iter().find(|s| s.kind == docxtor_core::stories::StoryKind::Header).unwrap();
    let text: String = header.blocks.iter().map(|b| write_block(b).to_xml()).collect();
    assert!(text.contains(">Up<") && !text.contains("w:anchor="), "{text}");
}

/// A link to a hidden bookmark (the `_Toc…` a table of contents puts on a heading, which the link editor offers) is a link: only a
/// bookmark that is nowhere makes the link op refuse. (Linking to a heading from the
/// link editor once did nothing.)
#[test]
fn a_link_to_a_heading_bookmark_a_table_of_contents_made_is_a_link() {
    let mut doc = doc_with("# Highlights\n\nText\n\nEnd here");
    let h = find_p(&doc, "Highlights");
    let end = find_p(&doc, "End here");
    ops(&mut doc, json!({"op":"insert_toc","before":h,"levels":2,"title":"Contents"}));
    let xml = para_xml(&doc, h);
    let name = xml.split("w:name=\"").nth(1).and_then(|r| r.split('"').next()).expect("the heading has a bookmark").to_string();
    assert!(name.starts_with("_Toc"), "{name}");
    ops(&mut doc, json!({"op":"set_link","from":{"p":end,"o":0},"to":{"p":end,"o":3},"url":format!("#{name}")}));
    assert!(para_xml(&doc, end).contains(&format!("w:anchor=\"{name}\"")));
}
