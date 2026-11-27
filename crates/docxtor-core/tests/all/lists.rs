//! Lists as Word shows them (list styles, restarts, formats, picture bullets, indents), lists a person
//! defines, setting a list item's number, and sorting paragraphs.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::package::Package;
use docxtor_core::render::RBlock;
use docxtor_core::Document;
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

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

/// A blank document with the given body, numbering part and extra styles (inside `w:styles`).
fn docx(body: &str, numbering: &str, styles: &str, lang: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    if !numbering.is_empty() {
        pkg.set("word/numbering.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:numbering xmlns:w="{W}">{numbering}</w:numbering>"#).into_bytes());
    }
    pkg.set(
        "word/styles.xml",
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:sz w:val="22"/><w:lang w:val="{lang}"/></w:rPr></w:rPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>{styles}</w:styles>"#
        )
        .into_bytes(),
    );
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn para(text: &str, ppr: &str) -> String {
    format!(r#"<w:p><w:pPr>{ppr}</w:pPr><w:r><w:t>{text}</w:t></w:r></w:p>"#)
}

fn item(text: &str, num: i64, lvl: i64) -> String {
    para(text, &format!(r#"<w:numPr><w:ilvl w:val="{lvl}"/><w:numId w:val="{num}"/></w:numPr>"#))
}

fn level(ilvl: i64, fmt: &str, text: &str, extra: &str) -> String {
    format!(r#"<w:lvl w:ilvl="{ilvl}"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/>{extra}<w:lvlText w:val="{text}"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="{}" w:hanging="360"/></w:pPr></w:lvl>"#, 720 * (ilvl + 1))
}

/// (text, label text) of every list paragraph.
fn labels(doc: &Document) -> Vec<(String, String)> {
    let labels = doc.numbering.labels(&doc.body, &doc.styles);
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| {
        if let Some(l) = labels.get(&p.id) {
            v.push((p.plain_text(), l.text.clone()));
        }
    });
    v
}

fn label_texts(doc: &Document) -> Vec<String> {
    labels(doc).into_iter().map(|(_, l)| l).collect()
}

fn id_of(doc: &Document, text: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if p.plain_text() == text {
            id = Some(p.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph says {text:?}"))
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn rendered(doc: &Document, text: &str) -> docxtor_core::render::RPara {
    let mut found = None;
    for b in doc.render_snapshot().blocks {
        if let RBlock::Para(p) = b {
            if p.segs.iter().any(|s| s.x.as_deref() == Some(text)) {
                found = Some(p);
            }
        }
    }
    found.unwrap_or_else(|| panic!("no rendered paragraph says {text:?}"))
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

#[test]
fn a_list_that_takes_its_numbering_from_a_list_style_shows_numbers() {
    // Definition 1 says "use the list style MyList"; definition 0 is that style's list.
    let numbering = format!(
        r#"<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/><w:styleLink w:val="MyList"/>{}{}</w:abstractNum><w:abstractNum w:abstractNumId="1"><w:multiLevelType w:val="multilevel"/><w:numStyleLink w:val="MyList"/></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="0"/></w:num>"#,
        level(0, "upperRoman", "%1.", ""),
        level(1, "lowerLetter", "%2)", "")
    );
    let styles = r#"<w:style w:type="numbering" w:styleId="MyList"><w:name w:val="MyList"/><w:pPr><w:numPr><w:numId w:val="2"/></w:numPr></w:pPr></w:style>"#;
    let body = [item("One", 1, 0), item("Sub", 1, 1), item("Two", 1, 0), item("Direct", 2, 0)].concat();
    let mut doc = docx(&body, &numbering, styles, "en-US");
    assert_eq!(label_texts(&doc), ["I.", "a)", "II.", "III."], "the style's list counts together with its users");
    // And it survives a save.
    let bytes = doc.save().unwrap();
    let doc = Document::open(&bytes).unwrap();
    assert_eq!(label_texts(&doc), ["I.", "a)", "II.", "III."]);
}

#[test]
fn a_level_can_restart_after_another_level_or_never() {
    // Level 1 restarts normally, level 2 never, level 3 only after level 1 items.
    let numbering = format!(
        r#"<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/>{}{}{}{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
        level(0, "decimal", "%1.", ""),
        level(1, "decimal", "%2.", r#"<w:lvlRestart w:val="0"/>"#),
        level(2, "decimal", "%3.", r#"<w:lvlRestart w:val="1"/>"#),
        level(3, "decimal", "%4.", "")
    );
    let body = [
        item("a", 1, 0),
        item("b", 1, 1),
        item("c", 1, 2),
        item("d", 1, 1),
        item("e", 1, 2),
        item("f", 1, 3),
        item("g", 1, 3),
        item("h", 1, 0),
        item("i", 1, 1),
        item("j", 1, 2),
        item("k", 1, 3),
    ]
    .concat();
    let doc = docx(&body, &numbering, "", "en-US");
    let l = label_texts(&doc);
    // b and d count on across the item between them and the next top-level item.
    assert_eq!(l[1], "1.");
    assert_eq!(l[3], "2.");
    assert_eq!(l[8], "3.", "level 2 never restarts, not even after a level 1 item");
    // c and e: level 3 restarts only after level 1 items, so level 2 items don't restart it.
    assert_eq!((l[2].as_str(), l[4].as_str()), ("1.", "2."));
    assert_eq!(l[9], "1.", "a new level 1 item restarts level 3");
    // Level 4 restarts after any higher level.
    assert_eq!((l[5].as_str(), l[6].as_str(), l[10].as_str()), ("1.", "2.", "1."));
}

#[test]
fn formats_spell_numbers_in_the_documents_language() {
    let numbering = format!(
        r#"<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/>{}{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
        level(0, "cardinalText", "%1", ""),
        level(1, "ordinalText", "%1 %2", "")
    );
    let body = [item("x", 1, 0), item("y", 1, 0), item("z", 1, 1)].concat();
    assert_eq!(label_texts(&docx(&body, &numbering, "", "en-US")), ["One", "Two", "Two First"]);
    assert_eq!(label_texts(&docx(&body, &numbering, "", "es-ES")), ["Uno", "Dos", "Dos Primero"]);
    assert_eq!(label_texts(&docx(&body, &numbering, "", "de-DE")), ["Eins", "Zwei", "Zwei Erste"]);
    assert_eq!(label_texts(&docx(&body, &numbering, "", "ja-JP")), ["One", "Two", "Two First"]);
}

#[test]
fn label_formatting_alignment_and_picture_bullets_reach_the_view_model() {
    let numbering = r##"<w:numPicBullet w:numPicBulletId="3"><w:pict><v:shape id="_x0000_i1025" type="#_x0000_t75" style="width:12pt;height:6pt" o:bullet="t" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><v:imagedata r:id="rId1" o:title="star" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/></v:shape></w:pict></w:numPicBullet><w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="right"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr><w:rPr><w:i/><w:u w:val="double"/><w:strike/><w:highlight w:val="yellow"/><w:caps/><w:b/><w:color w:val="FF0000"/></w:rPr></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="&#xF0B7;"/><w:lvlPicBulletId w:val="3"/><w:lvlJc w:val="center"/><w:pPr><w:ind w:left="1440" w:hanging="360"/></w:pPr><w:rPr><w:rFonts w:ascii="Symbol" w:hAnsi="Symbol" w:hint="default"/></w:rPr></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"##;
    let body = [item("styled", 1, 0), item("pictured", 1, 1)].concat();
    let mut doc = docx(&body, numbering, "", "en-US");
    // Without a relationship for the picture the label is still a bullet.
    assert!(rendered(&doc, "pictured").list.unwrap().pic.is_none());
    let mut pkg = Package::open(&doc.save().unwrap()).unwrap();
    pkg.set(
        "word/_rels/numbering.xml.rels",
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/star.png"/></Relationships>"#.to_vec(),
    );
    let doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    let s = rendered(&doc, "styled").list.unwrap();
    assert_eq!((s.italic, s.strike, s.caps, s.bold), (Some(true), Some(true), Some(true), Some(true)));
    assert_eq!((s.underline.as_deref(), s.highlight.as_deref(), s.color.as_deref(), s.align.as_deref()), (Some("double"), Some("yellow"), Some("FF0000"), Some("right")));
    let p = rendered(&doc, "pictured").list.unwrap();
    let pic = p.pic.expect("a picture bullet");
    assert_eq!((pic.src.as_str(), pic.w, pic.h), ("word/media/star.png", 12.0, 6.0));
    assert_eq!(p.align.as_deref(), Some("center"));
    // The symbol-font character stays a bullet for readers that can't draw the picture.
    assert_eq!(p.text, "\u{2022}");
}

#[test]
fn indents_follow_word_direct_numbering_over_style_and_a_list_styles_own_indent_over_the_level() {
    let numbering = format!(r#"<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="hybridMultilevel"/>{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#, level(0, "decimal", "%1.", ""));
    let styles = r#"<w:style w:type="paragraph" w:styleId="Wide"><w:name w:val="Wide"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="2000"/></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="WideList"><w:name w:val="WideList"/><w:basedOn w:val="Normal"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr><w:ind w:left="2000" w:hanging="500"/></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="PlainList"><w:name w:val="PlainList"/><w:basedOn w:val="Wide"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr></w:style>"#;
    let body = [
        para("direct", r#"<w:pStyle w:val="Wide"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>"#),
        para("styled", r#"<w:pStyle w:val="WideList"/>"#),
        para("inherited", r#"<w:pStyle w:val="PlainList"/>"#),
        para("both", r#"<w:pStyle w:val="WideList"/><w:ind w:left="300"/>"#),
    ]
    .concat();
    let doc = docx(&body, &numbering, styles, "en-US");
    let ind = |t: &str| {
        let f = rendered(&doc, t).fmt;
        (f.ind_left, f.ind_hanging)
    };
    // numPr in the paragraph: the level's indent wins over the style's.
    assert_eq!(ind("direct"), (Some(720), Some(360)));
    // numPr in the style that has its own indent: the style wins.
    assert_eq!(ind("styled"), (Some(2000), Some(500)));
    // numPr in a style that sets no indent: the level wins over the style it is based on.
    assert_eq!(ind("inherited"), (Some(720), Some(360)));
    // A direct indent beats everything.
    assert_eq!(ind("both").0, Some(300));
}

#[test]
fn a_style_linked_to_a_level_without_a_level_number_takes_that_level() {
    let numbering = format!(
        r#"<w:abstractNum w:abstractNumId="0"><w:multiLevelType w:val="multilevel"/>{}{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#,
        level(0, "decimal", "%1.", r#"<w:pStyle w:val="H1"/>"#),
        level(1, "decimal", "%1.%2.", r#"<w:pStyle w:val="H2"/>"#)
    );
    // Neither style writes an ilvl; the level that names the style says which level it is.
    let styles = r#"<w:style w:type="paragraph" w:styleId="H1"><w:name w:val="H1"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr></w:style><w:style w:type="paragraph" w:styleId="H2"><w:name w:val="H2"/><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr></w:style>"#;
    let body = [para("A", r#"<w:pStyle w:val="H1"/>"#), para("B", r#"<w:pStyle w:val="H2"/>"#), para("C", r#"<w:pStyle w:val="H2"/>"#), para("D", r#"<w:pStyle w:val="H1"/>"#)].concat();
    let doc = docx(&body, &numbering, styles, "en-US");
    assert_eq!(label_texts(&doc), ["1.", "1.1.", "1.2.", "2."]);
}

// ---------------------------------------------------------------------------
// Defining lists
// ---------------------------------------------------------------------------

fn three_paragraphs() -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"Alpha\n\nBeta\n\nGamma"}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

#[test]
fn define_a_bullet_a_number_format_and_a_multilevel_list() {
    let mut doc = three_paragraphs();
    let (a, b, c) = (id_of(&doc, "Alpha"), id_of(&doc, "Beta"), id_of(&doc, "Gamma"));
    // A bullet with its own symbol, font, alignment and indents.
    let r = ops(&mut doc, json!({"op":"define_list","ids":[a],"def":{"levels":[{"format":"bullet","text":"\u{2713}","font":"Arial","align":"center","left":1000,"hanging":400,"suffix":"space"}]}}));
    let bullet = r[0].count.unwrap() as i64;
    assert_eq!(labels(&doc)[0], ("Alpha".into(), "\u{2713}".into()));
    let l = doc.numbering.level(bullet, 0).unwrap();
    assert_eq!((l.fmt.as_str(), l.jc.as_str(), l.suff.as_str(), l.ppr.ind_left, l.ppr.ind_hanging, l.rpr.font.as_deref()), ("bullet", "center", "space", Some(1000), Some(400), Some("Arial")));
    // A number format: "Step 3)" style, starting at 3.
    ops(&mut doc, json!({"op":"define_list","ids":[b,c],"def":{"levels":[{"format":"decimal","text":"Step %1)","start":3,"bold":true}]}}));
    assert_eq!(label_texts(&doc), ["\u{2713}", "Step 3)", "Step 4)"]);
    // A multilevel list with the earlier level's count in the text and legal numbering.
    ops(&mut doc, json!({"op":"define_list","ids":[a,b,c],"def":{"name":"Outline","levels":[{"format":"upperRoman","text":"%1."},{"format":"lowerRoman","text":"%1.%2.","legal":true,"restart":1,"suffix":"nothing"}]}}));
    assert_eq!(label_texts(&doc), ["I.", "II.", "III."]);
    ops(&mut doc, json!({"op":"indent","ids":[b,c],"delta":1}));
    assert_eq!(label_texts(&doc), ["I.", "1.1.", "1.2."], "legal numbering shows the counts as numerals");
    // Word's markup: one abstractNum per definition, the nums after all of them, nine levels each.
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let numbering = docxtor_core::xml::parse(pkg.get("word/numbering.xml").unwrap()).unwrap();
    let names: Vec<_> = numbering.root.elements().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["w:abstractNum", "w:abstractNum", "w:abstractNum", "w:num", "w:num", "w:num"]);
    for a in numbering.root.children_named("w:abstractNum") {
        assert_eq!(a.children_named("w:lvl").count(), 9);
        assert!(a.child("w:nsid").is_some() && a.child("w:tmpl").is_some());
    }
    // And it opens again the same.
    let doc = Document::open(&bytes).unwrap();
    assert_eq!(label_texts(&doc), ["I.", "1.1.", "1.2."]);
}

#[test]
fn defining_a_list_is_one_undo_step() {
    let mut doc = three_paragraphs();
    let a = id_of(&doc, "Alpha");
    let before = doc.numbering.doc.clone().map(|d| d.to_bytes());
    ops(&mut doc, json!({"op":"define_list","ids":[a],"def":{"levels":[{"format":"decimal","text":"%1."}]}}));
    assert_eq!(label_texts(&doc), ["1."]);
    assert!(doc.undo());
    assert!(label_texts(&doc).is_empty());
    assert_eq!(doc.numbering.doc.clone().map(|d| d.to_bytes()), before);
    assert!(doc.redo());
    assert_eq!(label_texts(&doc), ["1."]);
}

#[test]
fn headings_are_linked_to_levels_the_way_word_links_them() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":"# Intro\n\n## Scope\n\n## Aim\n\n# Method\n\nBody text"}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    let intro = id_of(&doc, "Intro");
    ops(&mut doc, json!({"op":"define_list","def":{"levels":[
        {"format":"decimal","text":"%1.","style":"Heading 1"},
        {"format":"decimal","text":"%1.%2.","style":"Heading 2"},
    ]}}));
    // No paragraph joined directly, yet the headings are numbered through their styles.
    assert_eq!(labels(&doc), [("Intro".into(), "1.".into()), ("Scope".into(), "1.1.".into()), ("Aim".into(), "1.2.".into()), ("Method".into(), "2.".into())]);
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let styles = docxtor_core::xml::parse(pkg.get("word/styles.xml").unwrap()).unwrap();
    let style = |id: &str| styles.root.children_named("w:style").find(|s| s.attr("w:styleId") == Some(id)).unwrap().clone();
    let np1 = style("Heading1").child("w:pPr").unwrap().child("w:numPr").unwrap().clone();
    assert!(np1.child("w:ilvl").is_none(), "Word leaves the level out for the first");
    assert!(np1.child("w:numId").is_some());
    let np2 = style("Heading2").child("w:pPr").unwrap().child("w:numPr").unwrap().clone();
    assert_eq!(np2.child("w:ilvl").unwrap().attr("w:val"), Some("1"));
    let numbering = docxtor_core::xml::parse(pkg.get("word/numbering.xml").unwrap()).unwrap();
    let lvls: Vec<_> = numbering.root.child("w:abstractNum").unwrap().children_named("w:lvl").take(2).map(|l| l.child("w:pStyle").map(|p| p.attr("w:val").unwrap().to_string())).collect();
    assert_eq!(lvls, [Some("Heading1".to_string()), Some("Heading2".to_string())]);
    // The next list to take Heading 1 takes it over from this one.
    ops(&mut doc, json!({"op":"define_list","ids":[intro],"def":{"levels":[{"format":"upperLetter","text":"%1.","style":"Heading 1"}]}}));
    assert_eq!(labels(&doc)[0].1, "A.");
    assert_eq!(labels(&doc)[3].1, "B.");
    assert_eq!(labels(&doc)[1].1, "1.1.", "Heading 2 stays with the first list, which no longer has a Heading 1: its count shows %1 as it stands");
}

#[test]
fn a_picture_bullet_from_a_file() {
    let mut doc = three_paragraphs();
    let a = id_of(&doc, "Alpha");
    let png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 0];
    let media = doc.add_media("png", png.clone());
    let r = ops(&mut doc, json!({"op":"define_list","ids":[a],"def":{"levels":[{"format":"bullet","text":"","picture":media,"picture_size":[10.0,10.0]}]}}));
    let num = r[0].count.unwrap() as i64;
    let label = doc.numbering.labels(&doc.body, &doc.styles).get(&a).cloned().unwrap();
    let pic = label.pic.expect("the label is a picture");
    assert_eq!((pic.src.as_str(), pic.w, pic.h), (media.as_str(), 10.0, 10.0));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    let rels = String::from_utf8(pkg.get("word/_rels/numbering.xml.rels").unwrap().to_vec()).unwrap();
    assert!(rels.contains("relationships/image") && rels.contains("media/docxtor1.png"), "{rels}");
    assert_eq!(pkg.get(&media), Some(&png[..]));
    let numbering = String::from_utf8(pkg.get("word/numbering.xml").unwrap().to_vec()).unwrap();
    assert!(numbering.contains("w:numPicBullet") && numbering.contains("w:lvlPicBulletId") && numbering.contains("o:bullet=\"t\""), "{numbering}");
    // Opened again, it is still a picture, and the dialog can read the definition back.
    let doc = Document::open(&bytes).unwrap();
    let a = id_of(&doc, "Alpha");
    assert_eq!(doc.numbering.labels(&doc.body, &doc.styles)[&a].pic.as_ref().map(|p| p.src.clone()), Some(media.clone()));
    assert_eq!(doc.list_definition(num).unwrap().levels[0].picture.as_deref(), Some(media.as_str()));
    // A picture that isn't in the file is refused.
    let mut doc = three_paragraphs();
    let a = id_of(&doc, "Alpha");
    let err = doc.apply_json(&json!({"op":"define_list","ids":[a],"def":{"levels":[{"format":"bullet","text":"","picture":"word/media/none.png"}]}}).to_string(), false).unwrap_err().to_string();
    assert!(err.contains("isn't in the document"), "{err}");
}

#[test]
fn bad_definitions_are_refused_and_change_nothing() {
    let mut doc = three_paragraphs();
    let a = id_of(&doc, "Alpha");
    for def in [json!({"levels":[]}), json!({"levels":[{"format":"decimal","text":"no count"}]}), json!({"levels":vec![json!({"format":"decimal","text":"%1."}); 10]})] {
        assert!(doc.apply_json(&json!({"op":"define_list","ids":[a],"def":def}).to_string(), false).is_err());
    }
    assert!(label_texts(&doc).is_empty());
    assert!(doc.numbering.doc.is_none() || doc.numbering.doc.as_ref().unwrap().root.elements().count() == 0);
}

#[test]
fn definitions_in_use_fill_the_gallery_and_can_be_applied_to_other_paragraphs() {
    let mut doc = three_paragraphs();
    let (a, b, c) = (id_of(&doc, "Alpha"), id_of(&doc, "Beta"), id_of(&doc, "Gamma"));
    // Presets are not listed; a defined list is, with a preview of its first levels.
    ops(&mut doc, json!({"op":"set_list_style","ids":[c],"style":"disc"}));
    assert!(doc.list_definitions().is_empty());
    let r = ops(&mut doc, json!({"op":"define_list","ids":[a],"def":{"name":"Steps","levels":[{"format":"decimal","text":"Step %1:"},{"format":"decimal","text":"%1.%2"}]}}));
    let num = r[0].count.unwrap() as i64;
    let defs = doc.list_definitions();
    assert_eq!(defs.len(), 1);
    assert_eq!((defs[0].num_id, defs[0].name.as_deref(), defs[0].bullet), (num, Some("Steps"), false));
    assert_eq!(defs[0].levels.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["Step 1:", "1.1", "1.1.1"]);
    // Applying it to another paragraph puts that paragraph in a list of its own on the same definition.
    ops(&mut doc, json!({"op":"apply_list","ids":[b],"num_id":num}));
    let l = label_texts(&doc);
    assert_eq!(l[..2], ["Step 1:", "Step 1:"]);
    assert_ne!(l[2], "Step 1:");
    // Applying to a paragraph in another list restyles that whole list.
    ops(&mut doc, json!({"op":"apply_list","ids":[c],"num_id":num}));
    assert_eq!(label_texts(&doc), ["Step 1:", "Step 1:", "Step 1:"]);
    assert!(doc.apply_json(&json!({"op":"apply_list","ids":[a],"num_id":999}).to_string(), false).is_err());
}

// ---------------------------------------------------------------------------
// Set numbering value
// ---------------------------------------------------------------------------

fn five_items() -> Document {
    let mut doc = three_paragraphs();
    let first = id_of(&doc, "Alpha");
    let after = id_of(&doc, "Gamma");
    ops(&mut doc, json!({"op":"insert_paragraph","after":after,"text":"Delta"}));
    let after = id_of(&doc, "Delta");
    ops(&mut doc, json!({"op":"insert_paragraph","after":after,"text":"Epsilon"}));
    let ids: Vec<u32> = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"].iter().map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"set_list_style","ids":ids,"style":"decimal"}));
    let _ = first;
    doc
}

#[test]
fn set_numbering_value_starts_a_new_list_or_skips_numbers() {
    let mut doc = five_items();
    assert_eq!(label_texts(&doc), ["1.", "2.", "3.", "4.", "5."]);
    // Start a new list at 10 from Gamma on.
    let gamma = id_of(&doc, "Gamma");
    ops(&mut doc, json!({"op":"set_numbering_value","id":gamma,"value":10}));
    assert_eq!(label_texts(&doc), ["1.", "2.", "10.", "11.", "12."]);
    // Continue the previous list at 4: Gamma is the next number, 3, so 4 skips one.
    let gamma = id_of(&doc, "Gamma");
    ops(&mut doc, json!({"op":"set_numbering_value","id":gamma,"value":4,"continue_previous":true}));
    assert_eq!(label_texts(&doc), ["1.", "2.", "4.", "5.", "6."]);
    // Continue at the number it would have anyway: the lists join again.
    let gamma = id_of(&doc, "Gamma");
    ops(&mut doc, json!({"op":"set_numbering_value","id":gamma,"value":3,"continue_previous":true}));
    assert_eq!(label_texts(&doc), ["1.", "2.", "3.", "4.", "5."]);
    assert!(doc.undo());
    assert_eq!(label_texts(&doc), ["1.", "2.", "4.", "5.", "6."]);
    // Not a list item.
    let mut plain = three_paragraphs();
    let a = id_of(&plain, "Alpha");
    assert!(plain.apply_json(&json!({"op":"set_numbering_value","id":a,"value":2}).to_string(), false).is_err());
}

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

fn sorted_doc(lines: &[&str]) -> Document {
    let body: String = lines.iter().map(|l| para(l, "")).collect();
    docx(&body, "", "", "en-US")
}

#[test]
fn sort_paragraphs_as_text_numbers_and_dates_ascending_and_descending() {
    let mut doc = sorted_doc(&["pear", "Apple", "banana", "cherry"]);
    let ids: Vec<u32> = texts(&doc).iter().map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids}));
    assert_eq!(texts(&doc)[..4], ["Apple", "banana", "cherry", "pear"]);
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids,"keys":[{"descending":true}]}));
    assert_eq!(texts(&doc)[..4], ["pear", "cherry", "banana", "Apple"]);
    assert!(doc.undo());
    assert_eq!(texts(&doc)[..4], ["Apple", "banana", "cherry", "pear"]);
    // The paragraphs are the same ones: they moved.
    assert_eq!(id_of(&doc, "pear"), ids[0]);

    let mut doc = sorted_doc(&["10 eggs", "9 eggs", "no number", "100 eggs"]);
    let ids: Vec<u32> = texts(&doc).iter().take(4).map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids,"keys":[{"type":"number"}]}));
    assert_eq!(texts(&doc)[..4], ["no number", "9 eggs", "10 eggs", "100 eggs"]);

    let mut doc = sorted_doc(&["Due 5 June 2024", "Due 1 May 2023", "Due 2024-01-31"]);
    let ids: Vec<u32> = texts(&doc).iter().take(3).map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids,"keys":[{"type":"date","descending":true}]}));
    assert_eq!(texts(&doc)[..3], ["Due 5 June 2024", "Due 2024-01-31", "Due 1 May 2023"]);
}

#[test]
fn sort_can_keep_a_header_be_case_sensitive_and_sort_by_a_field() {
    let mut doc = sorted_doc(&["Name\tScore", "b\t2", "A\t30", "a\t4"]);
    let ids: Vec<u32> = texts(&doc).iter().take(4).map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids,"header":true,"case_sensitive":true,"keys":[{"field":1}]}));
    assert_eq!(texts(&doc)[..4], ["Name\tScore", "a\t4", "A\t30", "b\t2"]);
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids,"header":true,"keys":[{"type":"number","field":2,"descending":true}]}));
    assert_eq!(texts(&doc)[..4], ["Name\tScore", "A\t30", "a\t4", "b\t2"]);
}

#[test]
fn sorting_list_items_keeps_the_list_and_the_numbers_follow_the_new_order() {
    let mut doc = five_items();
    let ids: Vec<u32> = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"].iter().map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids,"keys":[{"descending":true}]}));
    assert_eq!(labels(&doc).iter().map(|(t, l)| format!("{l} {t}")).collect::<Vec<_>>(), ["1. Gamma", "2. Epsilon", "3. Delta", "4. Beta", "5. Alpha"]);
}

#[test]
fn a_section_break_stays_where_it_is() {
    let body = format!("{}{}{}", para("b", ""), para("a", r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr>"#), para("c", ""));
    let mut doc = docx(&body, "", "", "en-US");
    let ids: Vec<u32> = ["a", "b", "c"].iter().map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":ids}));
    assert_eq!(texts(&doc)[..3], ["a", "b", "c"]);
    // The break is after the second paragraph still, not after the one that carried it.
    let mut with_break = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| with_break.push(p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some())));
    assert_eq!(with_break[..3], [false, true, false]);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn sorting_paragraphs_in_a_table_cell_and_across_places() {
    let body = format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="4000" w:type="dxa"/></w:tcPr>{}{}{}</w:tc></w:tr></w:tbl>{}"#,
        para("z", ""),
        para("x", ""),
        para("y", ""),
        para("outside", "")
    );
    let mut doc = docx(&body, "", "", "en-US");
    let cell: Vec<u32> = ["z", "x", "y"].iter().map(|t| id_of(&doc, t)).collect();
    ops(&mut doc, json!({"op":"sort_paragraphs","ids":cell}));
    assert_eq!(texts(&doc)[..3], ["x", "y", "z"]);
    // Paragraphs in different places can't be sorted together.
    let mixed = vec![id_of(&doc, "x"), id_of(&doc, "outside")];
    assert!(doc.apply_json(&json!({"op":"sort_paragraphs","ids":mixed}).to_string(), false).is_err());
}

#[test]
fn a_picture_bullets_relationship_is_given_back_when_the_op_is_refused_or_undone() {
    let rels = |doc: &Document| doc.pkg.rels_for("word/numbering.xml").unwrap_or_default().len();
    let mut doc = three_paragraphs();
    let a = id_of(&doc, "Alpha");
    let media = doc.add_media("png", vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 0]);
    let before = rels(&doc);
    // A picture level followed by one the definition check refuses: the picture's relationship was already added.
    let bad = json!({"op":"define_list","ids":[a],"def":{"levels":[{"format":"bullet","text":"","picture":media},{"format":"decimal","text":"no count"}]}});
    assert!(doc.apply_json(&bad.to_string(), false).is_err());
    assert_eq!(rels(&doc), before, "a refused op left a relationship");
    let ok = json!({"op":"define_list","ids":[a],"def":{"levels":[{"format":"bullet","text":"","picture":media}]}});
    ops(&mut doc, ok);
    assert_eq!(rels(&doc), before + 1);
    assert!(doc.undo());
    assert_eq!(rels(&doc), before, "undo left the relationship");
    assert!(doc.redo());
    assert_eq!(rels(&doc), before + 1);
    assert_valid(&doc.save().unwrap());
}

/// A label that ends in a character of more than one byte (`第 1 章`, Chinese chapter numbering) once cut the label in the
/// middle of the character in the Markdown export and crashed it (found on the corpus file chtoutline.docx).
#[test]
fn markdown_export_takes_a_label_that_ends_in_a_wide_character() {
    let abs = format!(r#"<w:abstractNum w:abstractNumId="0">{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#, level(0, "decimal", "第 %1 章", ""));
    let doc = docx(&(item("Start", 1, 0) + &item("Next", 1, 0)), &abs, "", "zh-CN");
    let md = docxtor_core::export::to_markdown(&doc);
    assert!(md.contains("Start") && md.contains("Next"), "{md}");
    // A plain number label is still kept as it is.
    let abs = format!(r#"<w:abstractNum w:abstractNumId="0">{}</w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>"#, level(0, "decimal", "%1)", ""));
    let doc = docx(&item("One", 1, 0), &abs, "", "en-US");
    assert!(docxtor_core::export::to_markdown(&doc).contains("1) One"));
}

/// An outline level outside Word's 0 to 8 (a damaged or hand-made file) is not a heading in the AI's outline; a negative
/// one once asked for billions of spaces and crashed it (found on the corpus file tdf143692_outlineLevelTortureTest.docx).
#[test]
fn the_outline_skips_an_outline_level_out_of_range() {
    let mut doc = docx(&(para("Odd", r#"<w:outlineLvl w:val="-3"/>"#) + &para("Big", r#"<w:outlineLvl w:val="4000000000"/>"#) + &para("Real", r#"<w:outlineLvl w:val="0"/>"#)), "", "", "en-US");
    let out = docxtor_core::ai::outline(&mut doc);
    assert!(out.contains("Real") && !out.contains("Odd") && !out.contains("Big"), "{out}");
}
