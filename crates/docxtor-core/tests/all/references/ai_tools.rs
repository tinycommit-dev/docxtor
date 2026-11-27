//! Tests of: AI tools.

use super::*;

#[test]
fn ai_tools_for_notes_captions_references_and_index() {
    let mut doc = doc_with("# Background\n\nThe market grew fast. Prices fell.\n\nA chart goes here.");
    let (h, text, chart) = (find_p(&doc, "Background"), find_p(&doc, "The market"), find_p(&doc, "A chart"));
    let r = call_tool(&mut doc, "insert_endnote", &json!({"id": format!("p{text}"), "after_text": "grew fast.", "text": "Source: annual report."})).unwrap();
    assert!(r.contains("Endnote added") && r.contains("Source: annual report."), "{r}");
    let s = docscript(&mut doc, &ScriptOpts::default());
    // Endnote marks appear as [^end:i] and the notes are listed after the footnotes.
    assert!(s.contains("[^end:i]") && s.contains("# Endnotes"), "{s}");
    assert!(s.contains("Endnotes\n[p") && s.contains("Source: annual report."), "{s}");

    let r = call_tool(&mut doc, "insert_caption", &json!({"after": format!("p{chart}"), "label": "Figure", "text": "Market size"})).unwrap();
    assert!(r.contains("Caption] Figure 1: Market size"), "{r}");
    let cap = find_p(&doc, "Market size");

    let targets = call_tool(&mut doc, "list_cross_reference_targets", &json!({})).unwrap();
    assert!(targets.contains("Background") && targets.contains(&format!("p{cap} Figure 1")) && targets.contains("Source: annual report."), "{targets}");

    let r = call_tool(
        &mut doc,
        "insert_cross_reference",
        &json!({"id": format!("p{text}"), "after_text": "Prices fell.", "target": {"type":"caption","p":format!("p{cap}")}, "show": "label_number"}),
    )
    .unwrap();
    assert!(r.contains("Prices fell.Figure 1"), "{r}");
    // The cross-reference reads as plain text: no hidden bookmark name in a link.
    let s = docscript(&mut doc, &ScriptOpts::default());
    assert!(s.contains("Prices fell.Figure 1") && !s.contains("_Ref"), "{s}");
    let bad = call_tool(&mut doc, "insert_cross_reference", &json!({"id": format!("p{text}"), "target": {"type":"heading","p":format!("p{text}")}, "show": "text"}));
    assert!(bad.unwrap_err().contains("isn't a heading"));
    let bad = call_tool(&mut doc, "insert_cross_reference", &json!({"id": format!("p{text}"), "target": {"type":"heading","p":format!("p{h}")}, "show": "sideways"}));
    assert!(bad.unwrap_err().contains("show must be"));

    let r = call_tool(&mut doc, "mark_index_entry", &json!({"id": format!("p{text}"), "text": "market", "entry": "Market", "sub": "growth"})).unwrap();
    assert!(r.contains("Marked"), "{r}");
    let r = call_tool(&mut doc, "mark_index_entry", &json!({"text": "Prices", "entry": "Price", "all": true})).unwrap();
    assert!(r.contains("Marked 1 paragraph"), "{r}");
    // The marks don't show in DocScript.
    let s = docscript(&mut doc, &ScriptOpts::default());
    assert!(s.contains("The market grew fast.") && !s.contains("XE"), "{s}");

    let r = call_tool(&mut doc, "insert_index", &json!({"after": format!("p{cap}"), "columns": 2})).unwrap();
    assert!(r.contains("Index inserted") && r.contains("Index1] Market") && r.contains("Index2] growth\t1") && r.contains("] Price\t1"), "{r}");
    let r = call_tool(&mut doc, "insert_table_of_figures", &json!({"after": format!("p{h}"), "label": "Figure"})).unwrap();
    assert!(r.contains("TableofFigures] [Figure 1: Market size"), "{r}");
    // A new caption first; update_fields brings everything in line.
    call_tool(&mut doc, "insert_caption", &json!({"before": format!("p{chart}"), "label": "Figure", "text": "Prices"})).unwrap();
    let r = call_tool(&mut doc, "update_fields", &json!({})).unwrap();
    assert!(r.contains("Updated"), "{r}");
    let s = docscript(&mut doc, &ScriptOpts::default());
    assert!(s.contains("Prices fell.Figure 2") && s.contains("Figure 1: Prices") && s.contains("Figure 2: Market size"), "{s}");
    // The tools are in the tool list.
    let defs = docxtor_core::ai::tool_definitions();
    let names: Vec<&str> = defs.as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap()).collect();
    for n in ["insert_endnote", "insert_caption", "insert_cross_reference", "mark_index_entry", "insert_index", "insert_table_of_figures", "update_fields", "list_cross_reference_targets"] {
        assert!(names.contains(&n), "{n}");
    }
    assert!(call_tool(&mut doc, "no_such_tool", &json!({})).unwrap_err().contains("unknown tool"));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn ops_serialize_with_documented_shapes() {
    // The JSON the editor sends.
    let op: docxtor_core::Op = serde_json::from_value(json!({"op":"insert_cross_reference","at":{"p":3,"o":1},"target":{"type":"footnote","id":2},"show":"number_formatted"})).unwrap();
    let back = serde_json::to_value(&op).unwrap();
    assert_eq!(back["target"], json!({"type":"footnote","id":"2"}));
    assert_eq!(back["link"], true);
    assert_eq!(back["above_below"], false);
    let op: docxtor_core::Op = serde_json::from_value(json!({"op":"insert_index","after":"p4"})).unwrap();
    let back = serde_json::to_value(&op).unwrap();
    assert_eq!((back["columns"].clone(), back["run_in"].clone()), (json!(2), json!(false)));
    let op: docxtor_core::Op = serde_json::from_value(json!({"op":"convert_notes","to":"endnote"})).unwrap();
    assert_eq!(serde_json::to_value(&op).unwrap()["to"], "endnote");
    let mut doc = Document::new_blank();
    assert!(doc.apply_json(r#"{"op":"insert_caption","label":"Figure"}"#, false).is_err());
}
