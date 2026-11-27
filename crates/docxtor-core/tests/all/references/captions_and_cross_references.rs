//! Tests of: Captions and cross-references.

use super::*;

#[test]
fn captions_are_numbered_per_label_in_document_order() {
    let mut doc = doc_with("Intro.\n\nMiddle.\n\nEnd.");
    let (a, b, c) = (find_p(&doc, "Intro"), find_p(&doc, "Middle"), find_p(&doc, "End"));
    ops(&mut doc, json!({"op":"insert_caption","after":a,"label":"Figure","text":"Revenue"}));
    ops(&mut doc, json!({"op":"insert_caption","after":b,"label":"Table","text":"Costs"}));
    ops(&mut doc, json!({"op":"insert_caption","after":c,"label":"Figure","text":"Churn"}));
    assert_eq!(texts(&doc), ["Intro.", "Figure 1: Revenue", "Middle.", "Table 1: Costs", "End.", "Figure 2: Churn"]);
    // A new first figure pushes the others along; tables are numbered on their own.
    let r = ops(&mut doc, json!({"op":"insert_caption","before":a,"label":"Figure","text":"Cover"}));
    assert_eq!(texts(&doc), ["Figure 1: Cover", "Intro.", "Figure 2: Revenue", "Middle.", "Table 1: Costs", "End.", "Figure 3: Churn"]);
    let cover = r[0].created[0];
    // The caret is at the end of the new caption.
    assert_eq!(r[0].caret.unwrap(), Pos::new(cover, "Figure 1: Cover".len()));
    // Word's markup: style Caption, label, SEQ field with its number, then the text.
    let xml = para_xml(&doc, cover);
    assert!(xml.starts_with(r#"<w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr><w:r><w:t xml:space="preserve">Figure </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> SEQ Figure \* ARABIC </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>: Cover</w:t></w:r></w:p>"#), "{xml}");
    // Undo takes it away and the numbers go back.
    assert!(doc.undo());
    assert_eq!(texts(&doc)[1], "Figure 1: Revenue");
    // Saved with the Caption style Word defines.
    let styles = saved_part(&mut doc, "word/styles.xml");
    assert!(styles.contains(r#"w:styleId="Caption""#) && styles.contains(r#"<w:name w:val="caption"/>"#) && styles.contains("<w:i/>") && styles.contains(r#"<w:sz w:val="18"/>"#) && styles.contains(r#"w:themeColor="text2""#), "{styles}");
}

#[test]
fn caption_options_position_separator_and_empty_text() {
    let mut doc = doc_with("Table goes here.");
    let t = find_p(&doc, "Table goes");
    // `position` says where the caption goes relative to the block being captioned.
    let r = ops(&mut doc, json!({"op":"insert_caption","after":t,"position":"above","label":"Table","text":"Prices","separator":" - "}));
    assert_eq!(texts(&doc), ["Table 1 - Prices", "Table goes here."]);
    let r2 = ops(&mut doc, json!({"op":"insert_caption","before":t,"position":"below","label":"Equation"}));
    assert_eq!(texts(&doc), ["Table 1 - Prices", "Table goes here.", "Equation 1"]);
    assert!(r[0].caret.is_some() && r2[0].created.len() == 1);
    // A label with a space gets an underscore in the field, as Word writes it.
    ops(&mut doc, json!({"op":"insert_caption","after":t,"label":"Photo album","text":"x"}));
    assert!(body_xml(&doc).contains(" SEQ Photo_album \\* ARABIC "));
    assert!(err(&mut doc, json!({"op":"insert_caption","after":t,"label":"  "})).contains("label"));
    assert!(err(&mut doc, json!({"op":"insert_caption","label":"Figure"})).contains("next to"));
}

#[test]
fn update_fields_renumbers_and_handles_seq_switches() {
    // Word's markup for three Figure captions where the numbers are stale; the second restarts at 5 (\r 5),
    // and a Table counts on its own.
    let seq = |name: &str, extra: &str, shown: &str| {
        format!(
            r#"<w:r><w:t xml:space="preserve">{name} </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> SEQ {name} \* ARABIC{extra} </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t>{shown}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
        )
    };
    let body = format!(
        r#"<w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr>{}</w:p><w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr>{}</w:p><w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr>{}</w:p><w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr>{}</w:p><w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr>{}</w:p><w:p><w:r><w:t>Hidden count </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> SEQ Figure \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#,
        seq("Figure", "", "7"),
        seq("Figure", r" \r 5", "2"),
        seq("Table", "", "9"),
        seq("Figure", "", "1"),
        seq("Figure", r" \c", "1"),
    );
    let mut doc = doc_from_body(&body);
    assert_eq!(texts(&doc)[0], "Figure 7");
    let r = ops(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(r[0].count, Some(5));
    // 1; restarts at 5; the table is 1; the next figure is 6; \c repeats the last number; \h shows nothing but counts.
    assert_eq!(texts(&doc), ["Figure 1", "Figure 5", "Table 1", "Figure 6", "Figure 6", "Hidden count "]);
    // Formatting of the result run is kept.
    assert!(para_xml(&doc, find_p(&doc, "Figure 5")).contains(r#"<w:rPr><w:noProof/></w:rPr><w:t>5</w:t>"#));
    // Nothing left to change: the same call leaves the document alone.
    let r = ops(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(r[0].count, Some(0));
    assert!(saved_part(&mut doc, "word/document.xml").contains(r#"\* ARABIC \r 5 "#));
}

#[test]
fn typing_after_a_field_stays_outside_it_and_deleting_the_result_removes_the_field() {
    let mut doc = doc_with("Intro.");
    let intro = find_p(&doc, "Intro");
    let r = ops(&mut doc, json!({"op":"insert_caption","after":intro,"label":"Figure"}));
    let (cap, caret) = (r[0].created[0], r[0].caret.unwrap());
    assert_eq!(caret, Pos::new(cap, 8));
    // Typing right after the caption's number goes after the field, not into its result.
    ops(&mut doc, json!({"op":"insert_text","at":caret,"text":" is nice"}));
    assert_eq!(texts(&doc)[1], "Figure 1 is nice");
    let x = para_xml(&doc, cap);
    assert!(x.contains(r#"<w:t>1</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve"> is nice</w:t></w:r>"#), "{x}");
    // So updating the fields doesn't eat it.
    ops(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(texts(&doc)[1], "Figure 1 is nice");
    // Typing in the middle of the label is ordinary typing.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":cap,"o":3},"text":"!"}));
    assert_eq!(texts(&doc)[1], "Fig!ure 1 is nice");
    // Deleting a field's whole result takes the field with it; there is no empty shell to refill.
    let len = doc.find_any_para(cap).unwrap().len16();
    ops(&mut doc, json!({"op":"delete","from":{"p":cap,"o":0},"to":{"p":cap,"o":len}}));
    assert_eq!(texts(&doc)[1], "");
    assert!(!para_xml(&doc, cap).contains("SEQ") && !para_xml(&doc, cap).contains("fldChar"));
    ops(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(texts(&doc)[1], "");
    // Deleting part of the result keeps the field.
    let r = ops(&mut doc, json!({"op":"insert_caption","after":intro,"label":"Table","text":"Prices"}));
    let cap2 = r[0].created[0];
    ops(&mut doc, json!({"op":"delete","from":{"p":cap2,"o":0},"to":{"p":cap2,"o":4}}));
    assert_eq!(texts(&doc)[1], "e 1: Prices");
    assert!(para_xml(&doc, cap2).contains("SEQ Table"));
}

pub(super) fn doc_with_headings() -> Document {
    let mut doc = doc_with("# Introduction\n\nSee below.\n\n# Methods\n\nWe measured things.\n\n# Results\n\nNumbers.\n");
    let ids: Vec<u32> = ["Introduction", "Methods", "Results"].iter().map(|t| find_p(&doc, t)).collect();
    // Number the headings 1, 2, 3.
    ops(&mut doc, json!({"op":"toggle_list","ids":ids,"kind":"decimal"}));
    doc
}

#[test]
fn cross_reference_to_a_heading_text_number_and_page() {
    let mut doc = doc_with_headings();
    let (intro, methods, results) = (find_p(&doc, "Introduction"), find_p(&doc, "Methods"), find_p(&doc, "Results"));
    let host = find_p(&doc, "See below");
    let pg = pages(&[(intro, 1), (methods, 3), (results, 7)]);
    let r = ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":host,"o":4},"target":{"type":"heading","p":methods},"show":"text"}));
    // The caret is after the field's result.
    assert_eq!(r[0].caret.unwrap(), Pos::new(host, 4 + "Methods".len()));
    ops(&mut doc, json!({"op":"insert_text","at":r[0].caret.unwrap(),"text":" / "}));
    let r = ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":host,"o":14},"target":{"type":"heading","p":methods},"show":"number"}));
    ops(&mut doc, json!({"op":"insert_text","at":r[0].caret.unwrap(),"text":" / "}));
    let r = ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":host,"o":18},"target":{"type":"heading","p":results},"show":"page","pages":pg}));
    ops(&mut doc, json!({"op":"insert_text","at":r[0].caret.unwrap(),"text":" "}));
    assert_eq!(texts(&doc)[1], "See Methods / 2 / 7 below.");
    let xml = para_xml(&doc, host);
    // Word's field markup: a hidden _Ref bookmark around the heading, REF with \h, \r for the number, PAGEREF.
    let bm = {
        let x = para_xml(&doc, methods);
        let i = x.find("w:name=\"_Ref").unwrap();
        x[i + 8..i + 8 + 13].to_string()
    };
    assert!(bm.starts_with("_Ref") && bm[4..].chars().all(|c| c.is_ascii_digit()) && bm.len() == 13, "{bm}");
    assert!(xml.contains(&format!(" REF {bm} \\h ")), "{xml}");
    assert!(xml.contains(&format!(" REF {bm} \\r \\h ")), "{xml}");
    assert!(xml.contains(" PAGEREF _Ref"), "{xml}");
    // The two references to Methods share one bookmark; Results has its own, with a different name and id.
    let m = para_xml(&doc, methods);
    assert_eq!(m.matches("<w:bookmarkStart").count(), 1);
    assert!(m.starts_with("<w:p>"));
    let r_xml = para_xml(&doc, results);
    assert_eq!(r_xml.matches("<w:bookmarkStart").count(), 1);
    assert!(!r_xml.contains(&bm));
    let ids: Vec<String> = [&m, &r_xml].iter().map(|x| x.split("w:id=\"").nth(1).unwrap().split('"').next().unwrap().to_string()).collect();
    assert_ne!(ids[0], ids[1]);
    // The page is what the layout said; saved output is valid and reopens with the same text.
    assert!(xml.contains("<w:t>7</w:t>"));
    let mut re = reopen(&mut doc);
    assert_eq!(texts(&re)[1], "See Methods / 2 / 7 below.");
    // A numbered-heading error, and a paragraph that isn't a heading.
    assert!(err(&mut re, json!({"op":"insert_cross_reference","at":{"p":host,"o":0},"target":{"type":"heading","p":host},"show":"text"})).contains("isn't a heading"));
    let plain = find_p(&re, "We measured");
    assert!(err(&mut re, json!({"op":"insert_cross_reference","at":{"p":host,"o":0},"target":{"type":"heading","p":plain},"show":"number"})).contains("isn't a heading"));
}

#[test]
fn cross_reference_unnumbered_heading_and_unsupported_show() {
    let mut doc = doc_with("# Plain heading\n\nBody text.");
    let h = find_p(&doc, "Plain heading");
    let b = find_p(&doc, "Body");
    assert!(err(&mut doc, json!({"op":"insert_cross_reference","at":{"p":b,"o":0},"target":{"type":"heading","p":h},"show":"number"})).contains("isn't numbered"));
    assert!(err(&mut doc, json!({"op":"insert_cross_reference","at":{"p":b,"o":0},"target":{"type":"heading","p":h},"show":"label_number"})).contains("can't be shown"));
    assert!(err(&mut doc, json!({"op":"insert_cross_reference","at":{"p":b,"o":0},"target":{"type":"bookmark","name":"Nope"},"show":"text"})).contains("no bookmark called Nope"));
    // Nothing was changed by the failures.
    assert!(!body_xml(&doc).contains("_Ref"));
}

#[test]
fn cross_reference_to_a_caption() {
    let mut doc = doc_with("Intro text.\n\nImage here.\n\nConclusion.");
    let (intro, image, end) = (find_p(&doc, "Intro"), find_p(&doc, "Image"), find_p(&doc, "Conclusion"));
    let r = ops(&mut doc, json!({"op":"insert_caption","after":image,"label":"Figure","text":"Revenue by region"}));
    let cap = r[0].created[0];
    let pg = pages(&[(cap, 4)]);
    let mut at = 11;
    for show in ["label_number", "text", "caption_text", "number", "page"] {
        ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":intro,"o":at},"target":{"type":"caption","p":cap},"show":show,"pages":pg}));
        let t = texts(&doc)[0].clone();
        ops(&mut doc, json!({"op":"insert_text","at":{"p":intro,"o":t.encode_utf16().count()},"text":" | "}));
        at = texts(&doc)[0].encode_utf16().count();
    }
    assert_eq!(texts(&doc)[0], "Intro text.Figure 1 | Figure 1: Revenue by region | : Revenue by region | 1 | 4 | ");
    let x = para_xml(&doc, cap);
    // Distinct hidden bookmarks for each part of the caption; each closes after the field it covers.
    assert_eq!(x.matches("<w:bookmarkStart").count(), 4, "{x}");
    // No bookmark ends inside the field's result: they close after the field's end.
    let field_end = x.find(r#"w:fldCharType="end""#).unwrap();
    let num_text = x.find("<w:t>1</w:t>").unwrap();
    assert_eq!(x.matches("<w:bookmarkEnd").count(), 4);
    for m in x.match_indices("<w:bookmarkEnd") {
        assert!(m.0 < num_text || m.0 > field_end, "a bookmark ends inside the field result: {x}");
    }
    let host = para_xml(&doc, intro);
    assert_eq!(host.matches(" REF _Ref").count(), 4);
    assert_eq!(host.matches(" PAGEREF _Ref").count(), 1);
    // Referencing the same part twice reuses its bookmark.
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":end,"o":0},"target":{"type":"caption","p":cap},"show":"label_number"}));
    assert_eq!(para_xml(&doc, cap).matches("<w:bookmarkStart").count(), 4);
    assert_eq!(texts(&doc).last().unwrap(), "Figure 1Conclusion.");
    assert_valid(&doc.save().unwrap());
    assert!(err(&mut doc, json!({"op":"insert_cross_reference","at":{"p":end,"o":0},"target":{"type":"caption","p":end},"show":"text"})).contains("isn't a caption"));
}

#[test]
fn cross_reference_to_a_bookmark_and_notes() {
    let mut doc = doc_with("Alpha beta gamma delta.\n\nElsewhere.");
    let (p, other) = (find_p(&doc, "Alpha"), find_p(&doc, "Elsewhere"));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":p,"o":6},"to":{"p":p,"o":10},"name":"Mark"}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":p,"o":16},"text":"A footnote."}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":p,"o":22},"text":"An endnote."}));
    let foot = doc.page_setup().notes[0].id.clone();
    let end = doc.page_setup().endnotes[0].id.clone();
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":other,"o":0},"target":{"type":"bookmark","name":"mark"},"show":"text","link":false}));
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":other,"o":4},"target":{"type":"footnote","id":foot},"show":"number"}));
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":other,"o":5},"target":{"type":"endnote","id":end},"show":"number"}));
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":other,"o":6},"target":{"type":"footnote","id":foot.parse::<u32>().unwrap()},"show":"number_formatted"}));
    assert_eq!(texts(&doc)[1], "beta1i1Elsewhere.");
    let x = para_xml(&doc, other);
    // REF without \h when it isn't a link; NOTEREF with \h; the formatted number looks like the mark.
    assert!(x.contains(" REF Mark "), "{x}");
    assert!(x.contains(" NOTEREF _Ref") && x.contains(r#"<w:rStyle w:val="FootnoteReference"/>"#), "{x}");
    // The notes' marks sit inside hidden bookmarks.
    let host = para_xml(&doc, p);
    assert_eq!(host.matches("<w:bookmarkStart").count(), 3, "{host}");
    assert!(host.contains(r#"<w:bookmarkStart w:id="#) && host.contains("<w:footnoteReference"));
    // Page references to a note and to a bookmark.
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":other,"o":0},"target":{"type":"endnote","id":end},"show":"page","pages":pages(&[(p, 2)])}));
    assert!(texts(&doc)[1].starts_with("2beta"));
    assert!(err(&mut doc, json!({"op":"insert_cross_reference","at":{"p":other,"o":0},"target":{"type":"footnote","id":"99"},"show":"number"})).contains("no footnote"));
    assert!(err(&mut doc, json!({"op":"insert_cross_reference","at":{"p":other,"o":0},"target":{"type":"footnote","id":foot},"show":"text"})).contains("can't be shown"));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
}

#[test]
fn cross_reference_above_below() {
    let mut doc = doc_with("# Target heading\n\nAfter the heading.");
    let (h, after) = (find_p(&doc, "Target"), find_p(&doc, "After"));
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":after,"o":5},"target":{"type":"heading","p":h},"show":"text","above_below":true}));
    // The heading comes first in the text, so the reference to it says "above".
    assert_eq!(texts(&doc)[1], "AfterTarget heading above the heading.");
    let x = para_xml(&doc, after);
    assert!(x.contains(" \\p \\h "), "{x}");
    // The other way round says "below".
    let mut hb = doc_with("Intro.\n\n# Later\n");
    let (i, l) = (find_p(&hb, "Intro"), find_p(&hb, "Later"));
    ops(&mut hb, json!({"op":"insert_cross_reference","at":{"p":i,"o":0},"target":{"type":"heading","p":l},"show":"text","above_below":true}));
    assert_eq!(texts(&hb)[0], "Later belowIntro.");
}

#[test]
fn ref_fields_update_after_the_target_changes_and_link_to_their_bookmark() {
    let mut doc = doc_with_headings();
    let (methods, results) = (find_p(&doc, "Methods"), find_p(&doc, "Results"));
    let host = find_p(&doc, "See below");
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":host,"o":4},"target":{"type":"heading","p":methods},"show":"text"}));
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":host,"o":0},"target":{"type":"heading","p":results},"show":"number"}));
    assert_eq!(texts(&doc)[1], "3See Methodsbelow.");
    // The result renders as an internal link to its bookmark, so Ctrl-click can follow it.
    let s = segs(&doc, host);
    let linked: Vec<_> = s.iter().filter(|x| x["link"].is_string()).collect();
    assert_eq!(linked.len(), 2, "{s:?}");
    for l in &linked {
        let link = l["link"].as_str().unwrap();
        assert!(link.starts_with("#_Ref") && l["fld"].is_string(), "{link}");
    }
    // The text it links to is a bookmark that starts in the heading paragraph (the render lists it).
    let target = linked[1]["link"].as_str().unwrap().trim_start_matches('#').to_string();
    let rendered = serde_json::to_value(doc.render_snapshot()).unwrap();
    assert!(rendered.to_string().contains(&format!("\"{target}\"")));
    // Rename the heading and add one before it: the number and text are stale until the fields are updated.
    ops(&mut doc, json!({"op":"replace_text","find":"Methods","replace":"Approach","ids":[methods]}));
    ops(&mut doc, json!({"op":"insert_markdown","before":methods,"markdown":"Extra paragraph, not numbered."}));
    assert_eq!(texts(&doc)[1], "3See Methodsbelow.");
    let intro = find_p(&doc, "Introduction");
    // Without the first heading the others are numbered 1 and 2.
    ops(&mut doc, json!({"op":"delete_blocks","ids":[intro]}));
    let r = ops(&mut doc, json!({"op":"update_fields"}));
    assert!(r[0].count.unwrap() >= 1);
    assert!(texts(&doc).contains(&"2See Approachbelow.".to_string()), "{:?}", texts(&doc));
    // Undo puts the old results back.
    assert!(doc.undo());
    assert!(texts(&doc).contains(&"3See Methodsbelow.".to_string()));
    // Lines to an unknown bookmark are left as they were.
    let mut w = doc_from_body(r#"<w:p><w:r><w:t xml:space="preserve">See </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> REF _Ref999999999 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>old result</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#);
    ops(&mut w, json!({"op":"update_fields"}));
    assert_eq!(texts(&w)[0], "See old result");
}

#[test]
fn xref_targets_lists_what_can_be_pointed_at() {
    let mut doc = doc_with_headings();
    let (methods, body) = (find_p(&doc, "Methods"), find_p(&doc, "We measured"));
    ops(&mut doc, json!({"op":"insert_bookmark","from":{"p":body,"o":3},"to":{"p":body,"o":11},"name":"Measured"}));
    ops(&mut doc, json!({"op":"insert_caption","after":body,"label":"Figure","text":"A chart"}));
    ops(&mut doc, json!({"op":"insert_footnote","at":{"p":body,"o":2},"text":"The footnote text."}));
    ops(&mut doc, json!({"op":"insert_endnote","at":{"p":body,"o":1},"text":"The endnote text."}));
    // Another reference makes a hidden bookmark: it isn't listed.
    ops(&mut doc, json!({"op":"insert_cross_reference","at":{"p":body,"o":0},"target":{"type":"heading","p":methods},"show":"text"}));
    let t = serde_json::to_value(doc.xref_targets()).unwrap();
    let heads: Vec<_> = t["headings"].as_array().unwrap().iter().map(|h| (h["text"].as_str().unwrap(), h["level"].as_u64().unwrap(), h["number"].as_str().map(String::from))).collect();
    assert_eq!(heads, [("Introduction", 1, Some("1".to_string())), ("Methods", 1, Some("2".to_string())), ("Results", 1, Some("3".to_string()))]);
    assert_eq!(t["bookmarks"].as_array().unwrap().len(), 1);
    assert_eq!(t["bookmarks"][0]["name"], "Measured");
    assert_eq!(t["bookmarks"][0]["text"], "measured");
    assert_eq!(t["captions"][0]["label"], "Figure");
    assert_eq!(t["captions"][0]["number"], "1");
    assert_eq!(t["captions"][0]["text"], "Figure 1: A chart");
    assert_eq!(t["footnotes"][0]["label"], "1");
    assert_eq!(t["footnotes"][0]["text"], "The footnote text.");
    assert_eq!(t["endnotes"][0]["label"], "i");
    assert_eq!(t["endnotes"][0]["text"], "The endnote text.");
    assert!(t["footnotes"][0]["id"].is_string());
}
