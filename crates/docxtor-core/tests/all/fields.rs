//! Fields as in Word: links written as fields, updating dates, properties, formulas and page numbers, locked
//! and dirty fields, update on open, the field ops, symbol fonts, page and date elements, soft hyphens, custom
//! footnote marks, legacy check boxes and EQ fields.

use docxtor_core::model::{walk_paragraphs, write_block};
use docxtor_core::xml;
use docxtor_core::{package::Package, Document};
use serde_json::{json, Value};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NOW: &str = "2026-10-13T09:05:00";

fn assert_valid(bytes: &[u8]) {
    let pkg = Package::open(bytes).unwrap();
    for part in &pkg.parts {
        if part.name.ends_with(".xml") || part.name.ends_with(".rels") {
            let doc = xml::parse(&part.data).unwrap_or_else(|e| panic!("{}: {e}", part.name));
            xml::check_namespaces(&doc).unwrap_or_else(|e| panic!("{}: {e}", part.name));
        }
    }
}

fn ops(doc: &mut Document, json: Value) -> Vec<docxtor_core::OpResult> {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"))
}

/// A document whose body is the given XML.
fn docx(body: &str) -> Document {
    docx_with(body, "")
}

/// The same, with extra XML inside `w:settings`.
fn docx_with(body: &str, settings: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    if !settings.is_empty() {
        pkg.set("word/settings.xml", format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:settings xmlns:w="{W}">{settings}</w:settings>"#).into_bytes());
    }
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn complex(instr: &str, result: &str) -> String {
    format!(r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve">{instr}</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>{result}</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#)
}

fn para(inner: &str) -> String {
    format!("<w:p>{inner}</w:p>")
}

fn texts(doc: &Document) -> Vec<String> {
    let mut v = Vec::new();
    walk_paragraphs(&doc.body, &mut |p| v.push(p.plain_text()));
    v
}

fn first(doc: &Document) -> u32 {
    let mut id = 0;
    walk_paragraphs(&doc.body, &mut |p| {
        if id == 0 {
            id = p.id;
        }
    });
    id
}

fn segs(doc: &Document, id: u32) -> Vec<Value> {
    let r = serde_json::to_value(doc.render_snapshot()).unwrap();
    fn find(blocks: &Value, id: u32) -> Option<Value> {
        for b in blocks.as_array()? {
            match b["t"].as_str() {
                Some("p") if b["id"] == id => return Some(b["segs"].clone()),
                Some("c") => {
                    if let Some(s) = find(&b["blocks"], id) {
                        return Some(s);
                    }
                }
                Some("tbl") => {
                    for row in b["rows"].as_array().unwrap() {
                        for cell in row["cells"].as_array().unwrap() {
                            if let Some(s) = find(&cell["blocks"], id) {
                                return Some(s);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
    find(&r["blocks"], id).expect("rendered paragraph").as_array().unwrap().clone()
}

fn saved(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    String::from_utf8(Package::open(&bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap()
}

fn update(doc: &mut Document, extra: Value) -> usize {
    let mut op = json!({"op": "update_fields", "input": {"now": NOW, "pages": 7}});
    for (k, v) in extra.as_object().unwrap() {
        op[k] = v.clone();
    }
    ops(doc, op)[0].count.unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------

#[test]
fn a_hyperlink_field_is_a_link() {
    let body = para(&complex(r#" HYPERLINK "https://example.com/a" \o "Tip" "#, "Example"));
    let doc = docx(&body);
    let p = first(&doc);
    let s = segs(&doc, p);
    assert!(s.iter().any(|x| x["x"] == "Example" && x["link"] == "https://example.com/a"), "{s:?}");
    // A place in the document, and an address with a place.
    let doc = docx(&para(&complex(r#" HYPERLINK \l "Chapter1" "#, "Go")));
    assert!(segs(&doc, first(&doc)).iter().any(|x| x["link"] == "#Chapter1"));
    let doc = docx(&para(&complex(r#" HYPERLINK "https://x.org/p" \l "top" "#, "Go")));
    assert!(segs(&doc, first(&doc)).iter().any(|x| x["link"] == "https://x.org/p#top"));
    // A simple field too.
    let doc = docx(&para(r#"<w:fldSimple w:instr=" HYPERLINK &quot;https://e.org&quot; "><w:r><w:t>Simple</w:t></w:r></w:fldSimple>"#));
    assert!(segs(&doc, first(&doc)).iter().any(|x| x["link"] == "https://e.org"));
}

#[test]
fn editing_a_hyperlink_field_makes_it_an_ordinary_link() {
    let mut doc = docx(&para(&format!("<w:r><w:t>See </w:t></w:r>{}", complex(r#" HYPERLINK "https://old.example" "#, "here"))));
    let p = first(&doc);
    ops(&mut doc, json!({"op": "set_link", "from": {"p": p, "o": 4}, "to": {"p": p, "o": 8}, "url": "https://new.example"}));
    let xml = saved(&mut doc);
    assert!(xml.contains("<w:hyperlink"), "{xml}");
    assert!(!xml.contains("HYPERLINK"), "{xml}");
    assert_eq!(texts(&doc)[0], "See here");
    assert!(segs(&doc, p).iter().any(|x| x["link"] == "https://new.example"));
    // Taking the link off leaves plain text.
    let mut doc = docx(&para(&complex(r#" HYPERLINK "https://old.example" "#, "here")));
    let p = first(&doc);
    ops(&mut doc, json!({"op": "set_link", "from": {"p": p, "o": 0}, "to": {"p": p, "o": 4}}));
    assert!(!saved(&mut doc).contains("HYPERLINK"));
    assert!(segs(&doc, p).iter().all(|x| x["link"].is_null()));
    assert!(doc.undo());
    assert!(segs(&doc, p).iter().any(|x| x["link"] == "https://old.example"));
}

// ---------------------------------------------------------------------------
// Updating
// ---------------------------------------------------------------------------

#[test]
fn dates_and_times_update_with_their_pictures() {
    let body = para(&format!(
        "{}<w:r><w:t xml:space=\"preserve\"> | </w:t></w:r>{}",
        complex(r#" DATE \@ "dddd, MMMM d, yyyy" "#, "Monday, January 1, 2001"),
        complex(r#" TIME \@ "h:mm am/pm" "#, "1:00 pm")
    ));
    let mut doc = docx(&body);
    assert_eq!(update(&mut doc, json!({})), 1);
    assert_eq!(texts(&doc)[0], "Tuesday, October 13, 2026 | 9:05 am");
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "Monday, January 1, 2001 | 1:00 pm");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn switches_apply_to_results() {
    let mut doc = docx(&para(&format!(
        "{}<w:r><w:t xml:space=\"preserve\"> {}</w:t></w:r>",
        complex(r#" DATE \@ "MMMM" \* Upper "#, "x"),
        "z"
    )));
    update(&mut doc, json!({}));
    assert!(texts(&doc)[0].starts_with("OCTOBER"), "{:?}", texts(&doc));
    let mut doc = docx(&para(&complex(r#" NUMPAGES \* ROMAN "#, "1")));
    update(&mut doc, json!({}));
    assert_eq!(texts(&doc)[0], "VII");
    let mut doc = docx(&para(&complex(r#" NUMPAGES \* CardText \* FirstCap "#, "1")));
    update(&mut doc, json!({}));
    assert_eq!(texts(&doc)[0], "Seven");
}

#[test]
fn locked_fields_stay_and_dirty_ones_update_on_open() {
    let locked = r#"<w:r><w:fldChar w:fldCharType="begin" w:fldLock="1"/></w:r><w:r><w:instrText> DATE \@ "yyyy" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>1999</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#;
    let dirty = r#"<w:r><w:fldChar w:fldCharType="begin" w:dirty="true"/></w:r><w:r><w:instrText> TITLE </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>old</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#;
    let mut doc = docx(&para(&format!("{locked}<w:r><w:t> </w:t></w:r>{dirty}")));
    ops(&mut doc, json!({"op": "set_doc_props", "title": "New title"}));
    // A full update leaves the locked one alone.
    update(&mut doc, json!({}));
    assert_eq!(texts(&doc)[0], "1999 New title");
    assert!(saved(&mut doc).contains("w:fldLock"));
    assert!(!saved(&mut doc).contains("w:dirty"), "the update clears the dirty mark");
}

#[test]
fn opening_updates_dates_dirty_fields_and_all_fields_when_the_file_asks() {
    let body = para(&format!("{}<w:r><w:t xml:space=\"preserve\"> </w:t></w:r>{}", complex(r#" DATE \@ "yyyy" "#, "1999"), complex(" NUMPAGES ", "1")));
    let mut doc = docx(&body);
    let before = doc.can_undo();
    assert_eq!(update(&mut doc, json!({"open": true})), 1);
    // Only the date; and opening leaves nothing to undo.
    assert_eq!(texts(&doc)[0], "2026 1");
    assert_eq!(doc.can_undo(), before);
    // With the file's own request, everything is updated and the request is removed.
    let mut doc = docx_with(&body, "<w:updateFields w:val=\"true\"/>");
    update(&mut doc, json!({"open": true}));
    assert_eq!(texts(&doc)[0], "2026 7");
    let bytes = doc.save().unwrap();
    let settings = String::from_utf8(Package::open(&bytes).unwrap().get("word/settings.xml").unwrap().to_vec()).unwrap();
    assert!(!settings.contains("updateFields"), "{settings}");
}

#[test]
fn updating_before_printing_leaves_no_step_when_nothing_changes() {
    let mut doc = docx(&para(&complex(r#" DATE \@ "yyyy" "#, "2026")));
    assert!(!doc.can_undo());
    assert_eq!(update(&mut doc, json!({"quiet": true})), 0);
    assert!(!doc.can_undo(), "nothing changed, so nothing to undo");
    let mut doc = docx(&para(&complex(r#" DATE \@ "yyyy" "#, "1999")));
    assert_eq!(update(&mut doc, json!({"quiet": true})), 1);
    assert!(doc.can_undo());
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "1999");
    assert!(!doc.can_undo(), "one step");
}

#[test]
fn properties_and_if_and_set_work() {
    let body = format!(
        "{}{}{}{}",
        para(&complex(" AUTHOR ", "x")),
        para(&complex(r#" SET who "Ada" "#, "")),
        para(&complex(" REF who ", "?")),
        para(&complex(r#" IF 3 > 2 "yes" "no" "#, "no")),
    );
    let mut doc = docx(&body);
    ops(&mut doc, json!({"op": "set_doc_props", "author": "Grace"}));
    update(&mut doc, json!({}));
    let t = texts(&doc);
    assert_eq!(t[0], "Grace");
    assert_eq!(t[2], "Ada");
    assert_eq!(t[3], "yes");
}

#[test]
fn nested_fields_are_updated_inside_out() {
    // IF { DATE \@ "yyyy" } = 2026 "current" "old": the date inside is worked out first.
    let inner = complex(r#" DATE \@ "yyyy" "#, "1999");
    let body = para(&format!(
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> IF </w:instrText></w:r>{inner}<w:r><w:instrText xml:space="preserve"> = 2026 "current" "old" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>old</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
    ));
    let mut doc = docx(&body);
    update(&mut doc, json!({}));
    // The date inside the code is in the text but not shown.
    let shown: String = segs(&doc, first(&doc)).iter().filter(|x| x["hid"] != true).filter_map(|x| x["x"].as_str()).collect();
    assert_eq!(shown, "current");
    assert_eq!(texts(&doc)[0], "2026current");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn formulas_read_the_table_they_are_in() {
    let cell = |inner: &str| format!("<w:tc><w:tcPr><w:tcW w:w=\"2000\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>", para(inner));
    let run = |t: &str| format!("<w:r><w:t>{t}</w:t></w:r>");
    let table = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/></w:tblPr><w:tblGrid><w:gridCol w:w=\"2000\"/></w:tblGrid><w:tr>{}</w:tr><w:tr>{}</w:tr><w:tr>{}</w:tr></w:tbl>",
        cell(&run("10")),
        cell(&run("32.5")),
        cell(&complex(r#" =SUM(ABOVE) \# "0.00" "#, "0"))
    );
    let mut doc = docx(&format!("{table}{}", para("")));
    assert_eq!(update(&mut doc, json!({})), 1);
    assert!(texts(&doc).contains(&"42.50".to_string()), "{:?}", texts(&doc));
    // The evaluator is available for the table feature.
    let mut id = 0;
    walk_paragraphs(&doc.body, &mut |p| {
        if p.plain_text() == "42.50" {
            id = p.id;
        }
    });
    assert_eq!(doc.evaluate_formula("SUM(ABOVE)*2", Some(id)).unwrap(), 85.0);
    assert!(doc.evaluate_formula("1/0", None).is_err());
}

#[test]
fn page_fields_in_the_text_follow_the_layout() {
    let body = format!("{}{}", para(&format!("<w:r><w:t xml:space=\"preserve\">Page </w:t></w:r>{}", complex(" PAGE ", "1"))), para(&complex(" SECTION ", "9")));
    let mut doc = docx(&body);
    let p = first(&doc);
    // Without a layout the page number is left alone.
    update(&mut doc, json!({}));
    assert_eq!(texts(&doc)[0], "Page 1");
    let pages: Value = json!({ p.to_string(): 3 });
    update(&mut doc, json!({"pages": pages}));
    assert_eq!(texts(&doc)[0], "Page 3");
    assert_eq!(texts(&doc)[1], "1");
    // A page-dependent field's instruction is on its result for the layout.
    let s = segs(&doc, p);
    assert!(s.iter().any(|x| x["fld"] == "PAGE" && x["fi"].as_str().is_some_and(|i| i.contains("PAGE"))), "{s:?}");
}

#[test]
fn symbol_fields_use_the_font() {
    let mut doc = docx(&para(&complex(r#" SYMBOL 169 \f "Wingdings" \s 14 "#, "?")));
    update(&mut doc, json!({}));
    let xml = saved(&mut doc);
    assert!(xml.contains("\u{F0A9}"), "{xml}");
    assert!(xml.contains(r#"w:ascii="Wingdings""#) && xml.contains(r#"<w:sz w:val="28"/>"#), "{xml}");
    // It is drawn as the look-alike.
    let s = segs(&doc, first(&doc));
    assert!(s.iter().any(|x| x["k"] == "sym"), "{s:?}");
}

// ---------------------------------------------------------------------------
// Ops
// ---------------------------------------------------------------------------

#[test]
fn insert_field_works_out_the_result_and_keeps_the_picture_case() {
    let mut doc = docx(&para("<w:r><w:t>Today: </w:t></w:r>"));
    let p = first(&doc);
    ops(&mut doc, json!({"op": "insert_field", "at": {"p": p, "o": 7}, "field": r#"DATE \@ "yyyy-MM-dd""#, "input": {"now": NOW}}));
    assert_eq!(texts(&doc)[0], "Today: 2026-10-13");
    let xml = saved(&mut doc);
    assert!(xml.contains(r#"w:instr=" DATE \@ &quot;yyyy-MM-dd&quot; ""#) || xml.contains(r#"DATE \@ "yyyy-MM-dd""#), "{xml}");
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "Today: ");
}

#[test]
fn editing_a_field_code_works_out_the_result_again() {
    let mut doc = docx(&para(&complex(r#" DATE \@ "yyyy" "#, "2026")));
    let p = first(&doc);
    let fields = doc.list_fields();
    assert_eq!((fields.len(), fields[0].code.as_str(), fields[0].o), (1, "DATE", 0));
    ops(&mut doc, json!({"op": "set_field_code", "at": {"p": p, "o": 0}, "instr": r#"DATE \@ "MMMM yyyy""#, "input": {"now": NOW}}));
    assert_eq!(texts(&doc)[0], "October 2026");
    assert!(doc.list_fields()[0].instr.contains("MMMM yyyy"));
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "2026");
    // A position with no field is an error, and changes nothing.
    assert!(doc.apply_json(&json!({"op": "set_field_code", "at": {"p": p, "o": 3}, "instr": "DATE"}).to_string(), false).is_err());
}

#[test]
fn set_field_result_replaces_a_plain_result() {
    let mut doc = docx(&para(&complex(" MYFIELD ", "old")));
    let p = first(&doc);
    ops(&mut doc, json!({"op": "set_field_result", "at": {"p": p, "o": 0}, "text": "new"}));
    assert_eq!(texts(&doc)[0], "new");
    assert!(doc.undo());
    assert_eq!(texts(&doc)[0], "old");
}

// ---------------------------------------------------------------------------
// Drawing odd elements
// ---------------------------------------------------------------------------

#[test]
fn symbol_fonts_are_drawn_as_look_alikes() {
    let body = para(r#"<w:r><w:rPr><w:rFonts w:ascii="Wingdings" w:hAnsi="Wingdings"/></w:rPr><w:t>JKl</w:t></w:r><w:r><w:sym w:font="Webdings" w:char="F048"/></w:r><w:r><w:rPr><w:rFonts w:ascii="Symbol" w:hAnsi="Symbol"/></w:rPr><w:t>a</w:t></w:r>"#);
    let doc = docx(&body);
    let s = segs(&doc, first(&doc));
    let xs: Vec<String> = s.iter().filter(|x| x["k"] == "sym").map(|x| x["x"].as_str().unwrap().to_string()).collect();
    assert_eq!(xs, ["\u{263A}", "\u{1F610}", "\u{25CF}", "\u{1F3E0}", "\u{03B1}"], "{s:?}");
    // Each takes one place, as the character it replaces did.
    let os: Vec<u64> = s.iter().filter(|x| x["k"] == "sym").map(|x| x["o"].as_u64().unwrap()).collect();
    assert_eq!(os, [0, 1, 2, 3, 4]);
    // The saved file is the original.
    let xml = saved(&mut { docx(&body) });
    assert!(xml.contains("JKl") && xml.contains("F048"), "{xml}");
}

#[test]
fn page_number_and_date_elements_have_values() {
    let doc = docx(&para(r#"<w:r><w:pgNum/></w:r><w:r><w:dayLong/></w:r><w:r><w:monthShort/></w:r><w:r><w:yearLong/></w:r>"#));
    let s = segs(&doc, first(&doc));
    let flds: Vec<&str> = s.iter().filter(|x| x["k"] == "val").map(|x| x["fld"].as_str().unwrap()).collect();
    assert_eq!(flds, ["pgNum", "dayLong", "monthShort", "yearLong"], "{s:?}");
    assert!(s.iter().all(|x| x["label"].is_null()), "no grey chip");
}

#[test]
fn soft_hyphens_are_characters_that_survive_a_round_trip() {
    let body = para(r#"<w:r><w:t>inter</w:t><w:softHyphen/><w:t>national</w:t></w:r>"#);
    let mut doc = docx(&body);
    assert_eq!(texts(&doc)[0], "inter\u{AD}national");
    let s = segs(&doc, first(&doc));
    assert_eq!(s[0]["x"], "inter\u{AD}national");
    let xml = saved(&mut doc);
    assert!(xml.contains("<w:softHyphen/>"), "{xml}");
    assert!(!xml.contains('\u{AD}'));
}

#[test]
fn a_footnote_with_a_custom_mark_shows_only_the_mark() {
    let body = para(r#"<w:r><w:t>Text</w:t></w:r><w:r><w:footnoteReference w:customMarkFollows="1" w:id="5"/><w:t>*</w:t></w:r>"#);
    let doc = docx(&body);
    let s = segs(&doc, first(&doc));
    let note = s.iter().find(|x| x["k"] == "note").expect("note seg");
    assert_eq!(note["label"], "");
    assert!(s.iter().any(|x| x["k"] == "t" && x["x"] == "*"), "{s:?}");
}

#[test]
fn legacy_check_boxes_and_eq_fields_are_drawn() {
    let check = |checked: bool| {
        format!(
            r#"<w:r><w:fldChar w:fldCharType="begin"><w:ffData><w:name w:val="Check1"/><w:enabled/><w:calcOnExit w:val="0"/><w:checkBox><w:sizeAuto/><w:default w:val="0"/>{}</w:checkBox></w:ffData></w:fldChar></w:r><w:r><w:instrText xml:space="preserve"> FORMCHECKBOX </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#,
            if checked { "<w:checked/>" } else { "" }
        )
    };
    let doc = docx(&para(&format!("{}<w:r><w:t> A </w:t></w:r>{}", check(false), check(true))));
    let s = segs(&doc, first(&doc));
    let boxes: Vec<&str> = s.iter().filter(|x| x["k"] == "ff").map(|x| x["x"].as_str().unwrap()).collect();
    assert_eq!(boxes, ["0", "1"], "{s:?}");
    // The length of the paragraph is still the text.
    assert_eq!(texts(&doc)[0], " A ");

    let doc = docx(&para(r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> EQ \f(1,2) </w:instrText></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#));
    let s = segs(&doc, first(&doc));
    let eq = s.iter().find(|x| x["k"] == "eq").expect("an eq seg");
    assert!(eq["math"]["mathml"].as_str().unwrap().contains("<mfrac>"), "{eq:?}");
    assert_eq!(texts(&doc)[0], "");
}

#[test]
fn unchanged_documents_stay_byte_identical() {
    let body = para(&complex(r#" DATE \@ "yyyy" "#, "2026"));
    let mut doc = docx(&body);
    let before = doc.save().unwrap();
    // An update that finds nothing to change writes nothing.
    update(&mut doc, json!({"quiet": true}));
    let after = doc.save().unwrap();
    let a = Package::open(&before).unwrap();
    let b = Package::open(&after).unwrap();
    assert_eq!(a.get("word/document.xml"), b.get("word/document.xml"));
    let _ = write_block;
}

#[test]
fn the_assistant_can_insert_list_and_change_fields() {
    use docxtor_core::ai::call_tool;
    let mut doc = docx(&para("<w:r><w:t>Words: </w:t></w:r>"));
    ops(&mut doc, json!({"op": "set_doc_props", "author": "Grace"}));
    let id = first(&doc);
    let out = call_tool(&mut doc, "insert_field", &json!({"id": format!("p{id}"), "field": "AUTHOR"})).unwrap();
    assert!(out.contains("Grace"), "{out}");
    let list = call_tool(&mut doc, "list_fields", &json!({})).unwrap();
    assert!(list.contains("AUTHOR") && list.contains("Grace"), "{list}");
    call_tool(&mut doc, "set_field_code", &json!({"id": format!("p{id}"), "field": "TITLE"})).unwrap();
    assert!(call_tool(&mut doc, "list_fields", &json!({})).unwrap().contains("TITLE"));
    assert!(call_tool(&mut doc, "update_fields", &json!({})).is_ok());
    assert!(call_tool(&mut doc, "set_field_code", &json!({"id": format!("p{id}"), "index": 4, "field": "TITLE"})).is_err());
}

#[test]
fn an_inserted_equation_is_a_complex_field_that_is_drawn_and_a_toc_is_refused() {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op": "insert_field", "at": {"p": first, "o": 0}, "field": r"EQ \f(1,2)"}));
    let s = segs(&doc, first);
    let eq = s.iter().find(|x| x["k"] == "eq").expect("the equation is drawn");
    assert!(eq["math"]["mathml"].as_str().unwrap().contains("<mfrac>"), "{eq:?}");
    let err = doc.apply_json(&json!({"op": "insert_field", "at": {"p": first, "o": 0}, "field": r#"TOC \o "1-3""#}).to_string(), false).unwrap_err().to_string();
    assert!(err.contains("insert_toc"), "{err}");
}
