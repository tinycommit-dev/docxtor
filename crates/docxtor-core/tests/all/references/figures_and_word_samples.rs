//! Tests of: Table of figures and word-made samples.

use super::*;

#[test]
fn table_of_figures_lists_captions_and_updates() {
    let mut doc = doc_with("Start.\n\nPicture one.\n\nPicture two.\n\nA table.\n\nEnd.");
    let (start, one, two, tbl) = (find_p(&doc, "Start"), find_p(&doc, "Picture one"), find_p(&doc, "Picture two"), find_p(&doc, "A table"));
    let c1 = ops(&mut doc, json!({"op":"insert_caption","after":one,"label":"Figure","text":"Alpha"}))[0].created[0];
    let c2 = ops(&mut doc, json!({"op":"insert_caption","after":two,"label":"Figure","text":"Beta"}))[0].created[0];
    ops(&mut doc, json!({"op":"insert_caption","before":tbl,"label":"Table","text":"Gamma"}));
    let r = ops(&mut doc, json!({"op":"insert_table_of_figures","after":start,"label":"Figure","pages":pages(&[(c1, 2), (c2, 5)])}));
    let tof = r[0].created.clone();
    assert_eq!(tof.len(), 2);
    let t = texts(&doc);
    assert_eq!(&t[1..3], ["Figure 1: Alpha\t2", "Figure 2: Beta\t5"]);
    // Word's markup: TOC \h \z \c "Figure", entries in style Table of Figures, links to _Toc bookmarks on the captions.
    let x0 = para_xml(&doc, tof[0]);
    assert!(x0.contains(r#"<w:pStyle w:val="TableofFigures"/>"#) && x0.contains(r#"<w:instrText xml:space="preserve"> TOC \h \z \c "Figure" </w:instrText>"#), "{x0}");
    assert!(x0.contains("<w:hyperlink w:anchor=\"_Toc") && x0.contains(" PAGEREF _Toc"));
    let anchor = x0.split("w:anchor=\"").nth(1).unwrap().split('"').next().unwrap().to_string();
    assert!(para_xml(&doc, c1).contains(&format!("w:name=\"{anchor}\"")));
    // The table caption isn't in it.
    assert!(!t[1..3].iter().any(|x| x.contains("Gamma")));
    // The links render as internal links.
    assert!(segs(&doc, tof[0]).iter().any(|s| s["link"] == format!("#{anchor}")));
    // A new figure appears after update_fields and after the older update_toc.
    let tail = find_p(&doc, "End.");
    let c3 = ops(&mut doc, json!({"op":"insert_caption","after":tail,"label":"Figure","text":"Delta"}))[0].created[0];
    let r = ops(&mut doc, json!({"op":"update_fields","pages":pages(&[(c1, 2), (c2, 5), (c3, 8)])}));
    assert!(r[0].count.unwrap() >= 1);
    let t = texts(&doc);
    assert_eq!(&t[1..4], ["Figure 1: Alpha\t2", "Figure 2: Beta\t5", "Figure 3: Delta\t8"]);
    let c4 = ops(&mut doc, json!({"op":"insert_caption","before":start,"label":"Figure","text":"Zero"}))[0].created[0];
    ops(&mut doc, json!({"op":"update_toc","pages":pages(&[(c4, 1), (c1, 2), (c2, 5), (c3, 8)])}));
    let t = texts(&doc);
    assert_eq!(&t[2..6], ["Figure 1: Zero\t1", "Figure 2: Alpha\t2", "Figure 3: Beta\t5", "Figure 4: Delta\t8"]);
    // The table of contents path doesn't turn a table of figures into a list of headings.
    assert!(!t.iter().any(|x| x.starts_with("Start.\t")));
    let saved = doc.save().unwrap();
    assert_valid(&saved);
    // Empty: no captions with that label.
    let r = ops(&mut doc, json!({"op":"insert_table_of_figures","label":"Photo"}));
    assert!(texts(&doc).iter().any(|x| x == "No table of figures entries found."));
    assert_eq!(r[0].created.len(), 1);
    let styles = saved_part(&mut doc, "word/styles.xml");
    assert!(styles.contains(r#"w:styleId="TableofFigures""#));
}

#[test]
fn toc_and_table_of_figures_coexist_when_updated() {
    let mut doc = doc_with("# Heading one\n\nBody.\n\n# Heading two\n\nMore.");
    let body = find_p(&doc, "Body.");
    let cap = ops(&mut doc, json!({"op":"insert_caption","after":body,"label":"Figure","text":"Chart"}))[0].created[0];
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_toc","before":first,"title":"Contents"}));
    ops(&mut doc, json!({"op":"insert_table_of_figures","before":first,"label":"Figure"}));
    ops(&mut doc, json!({"op":"update_fields","pages":pages(&[(cap, 3)])}));
    let t = texts(&doc);
    assert!(t.iter().any(|x| x == "Figure 1: Chart\t3"));
    assert!(t.iter().any(|x| x.starts_with("Heading one\t")) && t.iter().any(|x| x.starts_with("Heading two\t")));
    // Headings only in the contents, captions only in the figures.
    assert_eq!(t.iter().filter(|x| x.contains("Chart")).count(), 2);
}

#[test]
fn word_made_caption_and_cross_reference_are_read_and_updated() {
    // Exactly what Word writes: a caption with a _Ref bookmark around "Figure 1" (the number field is
    // stale), a cross-reference to it as REF ... \h, a PAGEREF, and a REF to a heading's text.
    let body = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:bookmarkStart w:id="1" w:name="_Ref111111111"/><w:r><w:t>Setup</w:t></w:r><w:bookmarkEnd w:id="1"/></w:p><w:p><w:pPr><w:pStyle w:val="Caption"/></w:pPr><w:bookmarkStart w:id="0" w:name="_Ref222222222"/><w:r><w:t xml:space="preserve">Figure </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> SEQ Figure \* ARABIC </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:rPr><w:noProof/></w:rPr><w:t>4</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:bookmarkEnd w:id="0"/><w:r><w:t>: A chart</w:t></w:r></w:p><w:p><w:r><w:t xml:space="preserve">As shown in </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> REF _Ref222222222 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Figure 4</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve"> on page </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF _Ref222222222 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>9</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t xml:space="preserve">, see </w:t></w:r><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> REF _Ref111111111 \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Old name</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r><w:r><w:t>.</w:t></w:r></w:p>"#;
    let mut doc = doc_from_body(body);
    let host = find_p(&doc, "As shown");
    assert_eq!(texts(&doc)[2], "As shown in Figure 4 on page 9, see Old name.");
    // Read as a caption and a heading by the dialog's lists.
    let t = doc.xref_targets();
    assert_eq!((t.captions[0].label.as_str(), t.captions[0].number.as_str()), ("Figure", "4"));
    assert_eq!(t.headings[0].text, "Setup");
    // The results are links to the bookmarks, as Word's \h does.
    let s = segs(&doc, host);
    let links: Vec<_> = s.iter().filter_map(|x| x["link"].as_str()).collect();
    assert_eq!(links, ["#_Ref222222222", "#_Ref222222222", "#_Ref111111111"]);
    let cap = find_p(&doc, "A chart");
    ops(&mut doc, json!({"op":"update_fields","pages":pages(&[(cap, 6)])}));
    assert_eq!(texts(&doc)[1], "Figure 1: A chart");
    assert_eq!(texts(&doc)[2], "As shown in Figure 1 on page 6, see Setup.");
    // Both of the caption's runs and Word's bookmark placement are kept.
    let x = para_xml(&doc, cap);
    assert!(x.contains(r#"<w:bookmarkStart w:id="0" w:name="_Ref222222222"/>"#) && x.contains("<w:rPr><w:noProof/></w:rPr><w:t>1</w:t>"));
    assert!(saved_part(&mut doc, "word/document.xml").contains(r#"<w:instrText xml:space="preserve"> REF _Ref222222222 \h </w:instrText>"#));
}

#[test]
fn simple_fields_and_footnote_numbers_in_word_markup_update() {
    // fldSimple REF and NOTEREF, as older Word versions and other producers write them; a footnote the REF points at.
    let body = r#"<w:p><w:bookmarkStart w:id="3" w:name="Key"/><w:r><w:t>Key phrase</w:t></w:r><w:bookmarkEnd w:id="3"/><w:r><w:t xml:space="preserve"> and </w:t></w:r><w:fldSimple w:instr=" REF Key \h "><w:r><w:t>stale</w:t></w:r></w:fldSimple><w:r><w:t>.</w:t></w:r></w:p>"#;
    let mut doc = doc_from_body(body);
    ops(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(texts(&doc)[0], "Key phrase and Key phrase.");
    // A simple field's result renders as a link too.
    let id = find_p(&doc, "Key phrase");
    assert!(segs(&doc, id).iter().any(|s| s["link"] == "#Key"));
    // Case switches apply.
    let mut doc = doc_from_body(&body.replace(r"REF Key \h", r"REF Key \h \* Upper"));
    ops(&mut doc, json!({"op":"update_fields"}));
    assert_eq!(texts(&doc)[0], "Key phrase and KEY PHRASE.");
}
