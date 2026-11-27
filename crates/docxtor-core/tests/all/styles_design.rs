//! Editing styles, the document theme, style sets and paragraph spacing: what is written to styles.xml and
//! theme1.xml, what shows on the rendered paragraphs, and that each change is one undoable step.

use docxtor_core::model::{walk_paragraphs, Inline};
use docxtor_core::package::Package;
use docxtor_core::render::RBlock;
use docxtor_core::Document;
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

fn ops(doc: &mut Document, json: Value) {
    doc.apply_json(&json.to_string(), false).unwrap_or_else(|e| panic!("{e}: {json}"));
}

fn fails(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).err().unwrap_or_else(|| panic!("expected an error: {json}")).to_string()
}

fn doc_with(md: &str) -> Document {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_markdown","before":first,"markdown":md}));
    ops(&mut doc, json!({"op":"delete_blocks","ids":[first]}));
    doc
}

fn part(doc: &mut Document, name: &str) -> String {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    // A part written again after an undo has a different line ending after its declaration; the content is the same.
    String::from_utf8(Package::open(&bytes).unwrap().get(name).unwrap_or_default().to_vec()).unwrap().replace("\r\n", "\n")
}

fn styles(doc: &mut Document) -> String {
    part(doc, "word/styles.xml")
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

/// The style element for an id, as XML text.
fn style_xml(doc: &mut Document, id: &str) -> String {
    let s = styles(doc);
    let at = s.find(&format!(r#"w:styleId="{id}""#)).unwrap_or_else(|| panic!("no style {id} in {s}"));
    let start = s[..at].rfind("<w:style ").unwrap();
    let end = s[at..].find("</w:style>").map(|e| at + e + 10).unwrap();
    s[start..end].to_string()
}

/// The first text segment's formatting of a rendered paragraph.
fn run_of(doc: &Document, id: u32) -> Value {
    for b in doc.render_snapshot().blocks {
        if let RBlock::Para(p) = b {
            if p.id == id {
                return serde_json::to_value(&p.segs[0].f).unwrap();
            }
        }
    }
    panic!("no paragraph {id}");
}

fn para_of(doc: &Document, id: u32) -> Value {
    for b in doc.render_snapshot().blocks {
        if let RBlock::Para(p) = b {
            if p.id == id {
                return serde_json::to_value(&p.fmt).unwrap();
            }
        }
    }
    panic!("no paragraph {id}");
}

// ---------------------------------------------------------------------------
// Editing a style
// ---------------------------------------------------------------------------

#[test]
fn modifying_a_style_changes_every_paragraph_that_uses_it_in_one_step() {
    let mut doc = doc_with("# Intro\n\nBody\n\n# Next");
    let h1 = find_p(&doc, "Intro");
    let h2 = find_p(&doc, "Next");
    let before = styles(&mut doc);
    ops(
        &mut doc,
        json!({"op":"set_style_definition","style":"Heading 1","def":{
            "ppr":{"spaceBefore":480,"spaceAfter":120,"align":"center","keepNext":true},
            "rpr":{"size":20,"bold":true,"color":"#C00000","font":"Georgia"}}}),
    );
    for h in [h1, h2] {
        let r = run_of(&doc, h);
        assert_eq!((r["size"].as_f64(), r["bold"].as_bool(), r["color"].as_str(), r["font"].as_str()), (Some(20.0), Some(true), Some("C00000"), Some("Georgia")));
        let p = para_of(&doc, h);
        assert_eq!((p["spaceBefore"].as_i64(), p["spaceAfter"].as_i64(), p["align"].as_str()), (Some(480), Some(120), Some("center")));
    }
    let xml = style_xml(&mut doc, "Heading1");
    // Word's order: pPr before rPr, and the properties in schema order inside each.
    assert!(xml.find("<w:pPr>").unwrap() < xml.find("<w:rPr>").unwrap(), "{xml}");
    assert!(xml.contains(r#"<w:jc w:val="center"/>"#) && xml.contains(r#"<w:sz w:val="40"/>"#) && xml.contains(r#"<w:color w:val="C00000"/>"#), "{xml}");
    // The text itself carries no direct formatting.
    assert!(!part(&mut doc, "word/document.xml").contains("<w:sz "), "the paragraphs are left alone");
    assert!(doc.undo());
    assert_eq!(styles(&mut doc), before);
    assert!(doc.redo());
    assert_eq!(run_of(&doc, h1)["size"], 20.0);
}

#[test]
fn a_built_in_style_the_file_lacks_is_added_as_word_writes_it() {
    let mut doc = doc_with("Plain");
    assert!(!styles(&mut doc).contains("Heading3"));
    ops(&mut doc, json!({"op":"set_style_definition","style":"Heading 3","def":{"rpr":{"italic":true}}}));
    let xml = style_xml(&mut doc, "Heading3");
    assert!(xml.contains(r#"<w:name w:val="heading 3"/>"#) && xml.contains(r#"<w:outlineLvl w:val="2"/>"#), "{xml}");
    assert!(xml.contains("<w:i/>"), "{xml}");
    // Its colour is the theme's accent, so it follows the theme.
    assert!(xml.contains(r#"w:themeColor="accent1""#), "{xml}");
}

#[test]
fn a_character_style_of_word_s_the_file_lacks_is_added_when_text_gets_it() {
    let mut doc = doc_with("Hello wonderful world");
    let p = find_p(&doc, "Hello");
    assert!(!styles(&mut doc).contains("Emphasis"));
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":6},"to":{"p":p,"o":15},"format":{"style":"Emphasis"}}));
    assert!(style_xml(&mut doc, "Emphasis").contains("<w:i/>"));
    assert_eq!(doc.style_uses("Emphasis").len(), 1);
    // Typed text can take it too; a style that does not exist at all is an error and changes nothing.
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":0},"text":"A ","format":{"style":"Strong"}}));
    assert!(styles(&mut doc).contains(r#"w:styleId="Strong""#));
    assert!(fails(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":3},"format":{"style":"No such style"}})).contains("unknown style"));
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":40},"format":{"style":""}}));
    assert!(doc.style_uses("Emphasis").is_empty() && doc.style_uses("Strong").is_empty());
}

#[test]
fn style_properties_and_the_gallery_flag() {
    let mut doc = doc_with("Plain");
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Callout","kind":"paragraph","next":"Normal","quick":true,"autoUpdate":true,"priority":5,
        "ppr":{"indLeft":720},"rpr":{"italic":true},"lang":"es-ES"}}));
    let xml = style_xml(&mut doc, "Callout");
    assert!(xml.contains(r#"w:customStyle="1""#) && xml.contains(r#"<w:basedOn w:val="Normal"/>"#) && xml.contains(r#"<w:next w:val="Normal"/>"#), "{xml}");
    assert!(xml.contains("<w:qFormat/>") && xml.contains("<w:autoRedefine/>") && xml.contains(r#"<w:uiPriority w:val="5"/>"#), "{xml}");
    assert!(xml.contains(r#"<w:lang w:val="es-ES"/>"#) && xml.contains(r#"<w:ind w:left="720"/>"#), "{xml}");
    // The children are in the order the schema asks for.
    let order = ["<w:name", "<w:basedOn", "<w:next", "<w:autoRedefine", "<w:uiPriority", "<w:qFormat", "<w:pPr", "<w:rPr"];
    let at: Vec<usize> = order.iter().map(|t| xml.find(t).unwrap_or_else(|| panic!("{t} in {xml}"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "{xml}");
    // Taking the flags off removes the elements.
    ops(&mut doc, json!({"op":"set_style_definition","style":"Callout","def":{"quick":false,"autoUpdate":false,"next":""}}));
    let xml = style_xml(&mut doc, "Callout");
    assert!(!xml.contains("qFormat") && !xml.contains("autoRedefine") && !xml.contains("<w:next"), "{xml}");
}

#[test]
fn linked_styles_come_in_pairs_and_stay_alike() {
    let mut doc = doc_with("Plain");
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Note","kind":"linked","rpr":{"bold":true}}}));
    let p = style_xml(&mut doc, "Note");
    let c = style_xml(&mut doc, "NoteChar");
    assert!(p.contains(r#"<w:link w:val="NoteChar"/>"#) && c.contains(r#"<w:link w:val="Note"/>"#), "{p} {c}");
    assert!(c.contains(r#"w:type="character""#) && c.contains(r#"<w:name w:val="Note Char"/>"#), "{c}");
    // A change to the paragraph style reaches the character style.
    ops(&mut doc, json!({"op":"set_style_definition","style":"Note","def":{"rpr":{"color":"#336699"}}}));
    assert!(style_xml(&mut doc, "NoteChar").contains(r#"<w:color w:val="336699"/>"#));
}

#[test]
fn names_bases_and_cycles_are_checked() {
    let mut doc = doc_with("Plain");
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Alpha"}}));
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Beta","basedOn":"Alpha"}}));
    assert!(fails(&mut doc, json!({"op":"set_style_definition","style":"Alpha","def":{"basedOn":"Beta"}})).contains("based on"));
    assert!(fails(&mut doc, json!({"op":"set_style_definition","style":"Alpha","def":{"basedOn":"Alpha"}})).contains("based on"));
    assert!(fails(&mut doc, json!({"op":"set_style_definition","def":{"name":"alpha"}})).contains("already"));
    assert!(fails(&mut doc, json!({"op":"set_style_definition","style":"Beta","def":{"name":"Alpha"}})).contains("already"));
    assert!(fails(&mut doc, json!({"op":"set_style_definition","style":"Normal","def":{"name":"Plain text"}})).contains("keeps its name"));
    assert!(fails(&mut doc, json!({"op":"set_style_definition","style":"Nope","def":{}})).contains("no style"));
    assert!(fails(&mut doc, json!({"op":"set_style_definition","def":{"name":"Chars","kind":"character","basedOn":"Alpha"}})).contains("character"));
    // Renaming a custom style keeps its id, so paragraphs keep it.
    let p = find_p(&doc, "Plain");
    ops(&mut doc, json!({"op":"set_style","ids":[p],"style":"Beta"}));
    ops(&mut doc, json!({"op":"set_style_definition","style":"Beta","def":{"name":"Gamma"}}));
    let xml = styles(&mut doc);
    assert!(xml.contains(r#"w:styleId="Beta""#) && xml.contains(r#"<w:name w:val="Gamma"/>"#), "{xml}");
    assert!(part(&mut doc, "word/document.xml").contains(r#"<w:pStyle w:val="Beta"/>"#));
}

#[test]
fn deleting_a_style_sends_its_paragraphs_to_the_style_it_was_based_on() {
    let mut doc = doc_with("One\n\nTwo\n\nThree");
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Base","rpr":{"italic":true}}}));
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Child","basedOn":"Base","rpr":{"bold":true}}}));
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Grandchild","basedOn":"Child"}}));
    let (one, two) = (find_p(&doc, "One"), find_p(&doc, "Two"));
    ops(&mut doc, json!({"op":"set_style","ids":[one],"style":"Child"}));
    ops(&mut doc, json!({"op":"set_style","ids":[two],"style":"Grandchild"}));
    let before = styles(&mut doc);
    ops(&mut doc, json!({"op":"delete_style","style":"Child"}));
    let xml = styles(&mut doc);
    assert!(!xml.contains(r#"w:styleId="Child""#), "{xml}");
    // The style built on it is built on its base now; the paragraph that used it uses the base.
    assert!(style_xml(&mut doc, "Grandchild").contains(r#"<w:basedOn w:val="Base"/>"#));
    let body = part(&mut doc, "word/document.xml");
    assert!(!body.contains(r#"w:val="Child""#) && body.contains(r#"<w:pStyle w:val="Base"/>"#), "{body}");
    assert!(doc.undo());
    assert_eq!(styles(&mut doc), before);
    assert!(part(&mut doc, "word/document.xml").contains(r#"<w:pStyle w:val="Child"/>"#));
    // Word's own styles stay.
    assert!(fails(&mut doc, json!({"op":"delete_style","style":"Normal"})).contains("can't be deleted"));
    ops(&mut doc, json!({"op":"set_style_definition","style":"Heading 2","def":{}}));
    assert!(fails(&mut doc, json!({"op":"delete_style","style":"Heading 2"})).contains("can't be deleted"));
}

#[test]
fn a_style_from_the_selection_takes_the_formatting_and_the_text_takes_the_style() {
    let mut doc = doc_with("Alpha beta\n\nSecond");
    let p = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[p],"format":{"align":"center","spaceAfter":400,"indLeft":360}}));
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":0},"to":{"p":p,"o":10},"format":{"bold":true,"color":"#336699","size":16}}));
    let before = styles(&mut doc);
    ops(&mut doc, json!({"op":"create_style","name":"Feature","from":{"p":p,"o":0},"to":{"p":p,"o":10}}));
    let xml = style_xml(&mut doc, "Feature");
    assert!(xml.contains(r#"<w:basedOn w:val="Normal"/>"#) && xml.contains(r#"<w:jc w:val="center"/>"#) && xml.contains(r#"w:after="400""#), "{xml}");
    assert!(xml.contains("<w:b/>") && xml.contains(r#"<w:color w:val="336699"/>"#) && xml.contains(r#"<w:sz w:val="32"/>"#), "{xml}");
    // It is in the gallery, and the paragraph has it without direct formatting.
    assert!(xml.contains("<w:qFormat/>"), "{xml}");
    let body = part(&mut doc, "word/document.xml");
    let pxml = &body[body.find("Alpha").unwrap() - 400..body.find("Alpha").unwrap()];
    assert!(pxml.contains(r#"<w:pStyle w:val="Feature"/>"#) && !pxml.contains("<w:jc") && !pxml.contains("<w:b/>") && !pxml.contains("<w:color"), "{pxml}");
    assert_eq!(para_of(&doc, p)["align"], "center");
    assert_eq!(run_of(&doc, p)["bold"], true);
    assert!(doc.undo());
    assert_eq!(styles(&mut doc), before);
}

#[test]
fn a_character_style_from_the_selection_marks_just_that_text() {
    let mut doc = doc_with("Alpha beta gamma");
    let p = find_p(&doc, "Alpha");
    ops(&mut doc, json!({"op":"format_text","from":{"p":p,"o":6},"to":{"p":p,"o":10},"format":{"italic":true,"color":"#CC0000"}}));
    ops(&mut doc, json!({"op":"create_style","name":"Warn","kind":"character","from":{"p":p,"o":6},"to":{"p":p,"o":10}}));
    let xml = style_xml(&mut doc, "Warn");
    assert!(xml.contains(r#"w:type="character""#) && xml.contains("<w:i/>") && xml.contains(r#"<w:color w:val="CC0000"/>"#), "{xml}");
    let uses = doc.style_uses("Warn");
    assert_eq!(uses.len(), 1);
    assert_eq!((uses[0].p, uses[0].from, uses[0].to), (p, 6, 10));
    assert!(fails(&mut doc, json!({"op":"create_style","name":"Other","kind":"character","from":{"p":p,"o":2},"to":{"p":p,"o":2}})).contains("select"));
    let mut inline_styles = 0;
    walk_paragraphs(&doc.body, &mut |q| {
        for i in &q.inlines {
            if let Inline::Run(r) = i {
                if r.rpr.as_ref().is_some_and(|e| e.child("w:rStyle").is_some()) {
                    inline_styles += 1;
                }
            }
        }
    });
    assert_eq!(inline_styles, 1);
}

#[test]
fn update_style_to_match_moves_what_the_selection_has_by_hand_into_the_style() {
    let mut doc = doc_with("# One\n\n# Two");
    let (one, two) = (find_p(&doc, "One"), find_p(&doc, "Two"));
    ops(&mut doc, json!({"op":"format_text","from":{"p":one,"o":0},"to":{"p":one,"o":3},"format":{"color":"#00AA00","size":30}}));
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[one],"format":{"align":"right"}}));
    ops(&mut doc, json!({"op":"update_style","style":"Heading 1","from":{"p":one,"o":0},"to":{"p":one,"o":3}}));
    for h in [one, two] {
        let r = run_of(&doc, h);
        assert_eq!((r["color"].as_str(), r["size"].as_f64()), (Some("00AA00"), Some(30.0)), "{h}");
        assert_eq!(para_of(&doc, h)["align"], "right");
    }
    let body = part(&mut doc, "word/document.xml");
    assert!(!body.contains("<w:jc") && !body.contains("00AA00"), "the paragraphs no longer carry it by hand: {body}");
    assert!(style_xml(&mut doc, "Heading1").contains(r#"<w:color w:val="00AA00"/>"#));
}

#[test]
fn updating_a_style_from_a_header_redraws_the_body_that_uses_it() {
    let mut doc = doc_with("# One");
    let one = find_p(&doc, "One");
    docxtor_core::ai::call_tool(&mut doc, "set_header_footer", &json!({"kind":"header","text":"Head"})).unwrap();
    let head = {
        let s = doc.stories.iter().find(|s| s.part.contains("header")).unwrap();
        docxtor_core::model::paragraph_order(&s.blocks)[0]
    };
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[head],"format":{"style":"Heading 1"}}));
    ops(&mut doc, json!({"op":"format_text","from":{"p":head,"o":0},"to":{"p":head,"o":4},"format":{"color":"#FF0000"}}));
    doc.render_all();
    ops(&mut doc, json!({"op":"update_style","style":"Heading 1","from":{"p":head,"o":0},"to":{"p":head,"o":4}}));
    let out = doc.render_changed();
    assert!(out.blocks.iter().any(|b| matches!(b, RBlock::Para(p) if p.id == one)), "the body paragraph is drawn again");
    assert_eq!(run_of(&doc, one)["color"], "FF0000");
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

#[test]
fn a_new_document_has_office_s_theme_and_its_text_uses_the_theme_fonts() {
    let mut doc = doc_with("# Head\n\nBody");
    let info = doc.theme_info();
    assert_eq!((info.name.as_str(), info.fonts.major.as_str(), info.fonts.minor.as_str()), ("Office", "Calibri Light", "Calibri"));
    assert_eq!(info.colors.colors["accent1"], "4472C4");
    assert_eq!(run_of(&doc, find_p(&doc, "Body"))["font"], "Calibri");
    assert_eq!(run_of(&doc, find_p(&doc, "Head"))["font"], "Calibri Light");
    let bytes = doc.save().unwrap();
    assert!(Package::open(&bytes).unwrap().has("word/theme/theme1.xml"));
    assert_valid(&bytes);
    // East Asian text takes the theme's East Asian font, as Word writes it.
    let styles = String::from_utf8(Package::open(&bytes).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap();
    assert!(styles.contains(r#"w:eastAsiaTheme="minorEastAsia""#) && !styles.contains(r#"w:eastAsiaTheme="minorHAnsi""#));
}

#[test]
fn changing_the_theme_recolours_and_refonts_everything_that_follows_it() {
    let mut doc = doc_with("# Head\n\nBody");
    let (h, b) = (find_p(&doc, "Head"), find_p(&doc, "Body"));
    // The heading colour is the accent in the Office theme (shaded).
    assert_eq!(run_of(&doc, h)["color"], "2F5496");
    let before_theme = part(&mut doc, "word/theme/theme1.xml");
    ops(&mut doc, json!({"op":"set_theme","theme":"Ion Boardroom"}));
    let info = doc.theme_info();
    assert_eq!((info.name.as_str(), info.fonts.major.as_str()), ("Ion Boardroom", "Century Gothic"));
    assert_eq!(run_of(&doc, b)["font"], "Century Gothic");
    assert_eq!(run_of(&doc, h)["font"], "Century Gothic");
    assert_ne!(run_of(&doc, h)["color"], "2F5496");
    // B31166 shaded by 0xBF in lightness.
    let c = run_of(&doc, h)["color"].as_str().unwrap().to_string();
    assert!(c.starts_with(|ch: char| ch.is_ascii_hexdigit()) && c.len() == 6, "{c}");
    let after = part(&mut doc, "word/theme/theme1.xml");
    assert!(after.contains(r#"name="Ion Boardroom""#) && after.contains("B31166"), "{after}");
    // The rest of the theme part stays.
    assert!(after.contains("<a:fmtScheme"), "{after}");
    assert!(doc.undo());
    assert_eq!(part(&mut doc, "word/theme/theme1.xml"), before_theme);
    assert_eq!(run_of(&doc, h)["color"], "2F5496");
    assert_eq!(run_of(&doc, b)["font"], "Calibri");
    assert!(doc.redo());
    assert_eq!(run_of(&doc, b)["font"], "Century Gothic");
}

#[test]
fn colours_and_fonts_change_on_their_own() {
    let mut doc = doc_with("# Head\n\nBody");
    let b = find_p(&doc, "Body");
    ops(&mut doc, json!({"op":"set_theme","colors":{"name":"Blue Warm"}}));
    let info = doc.theme_info();
    assert_eq!((info.colors.name.as_str(), info.colors.colors["accent1"].as_str(), info.name.as_str()), ("Blue Warm", "4A66AC", "Office"));
    assert_eq!(info.fonts.minor, "Calibri");
    ops(&mut doc, json!({"op":"set_theme","fonts":{"name":"Mine","major":"Georgia","minor":"Verdana"}}));
    assert_eq!(run_of(&doc, b)["font"], "Verdana");
    assert_eq!(doc.theme_info().colors.name, "Blue Warm");
    // Customised colours: the slots given change, the rest stay.
    ops(&mut doc, json!({"op":"set_theme","colors":{"name":"Mine","colors":{"accent2":"#123456"}}}));
    let info = doc.theme_info();
    assert_eq!((info.colors.colors["accent2"].as_str(), info.colors.colors["accent1"].as_str()), ("123456", "4A66AC"));
    assert!(fails(&mut doc, json!({"op":"set_theme","colors":{"name":"Bad","colors":{"accent9":"112233"}}})).contains("theme colour"));
    assert!(fails(&mut doc, json!({"op":"set_theme","colors":{"name":"Bad","colors":{"accent1":"nope"}}})).contains("not a colour"));
    assert!(fails(&mut doc, json!({"op":"set_theme","theme":"Nowhere"})).contains("no theme"));
    assert!(fails(&mut doc, json!({"op":"set_theme"})).contains("name a theme"));
}

#[test]
fn text_with_a_theme_colour_follows_the_theme() {
    let mut doc = doc_with("Plain");
    // A run coloured the way Word colours text with a theme colour: the hex is a fallback.
    let mut bytes = Package::open(&doc.save().unwrap()).unwrap();
    let xml = String::from_utf8(bytes.get("word/document.xml").unwrap().to_vec()).unwrap();
    let xml = xml.replace("<w:r><w:t>Plain</w:t></w:r>", r#"<w:r><w:rPr><w:color w:val="4472C4" w:themeColor="accent1"/></w:rPr><w:t>Plain</w:t></w:r>"#);
    bytes.set("word/document.xml", xml.into_bytes());
    let mut doc = Document::open(&bytes.to_bytes().unwrap()).unwrap();
    let p = find_p(&doc, "Plain");
    assert_eq!(run_of(&doc, p)["color"], "4472C4");
    ops(&mut doc, json!({"op":"set_theme","theme":"Facet"}));
    assert_eq!(run_of(&doc, p)["color"], "90C226");
    // The file keeps the hex fallback and the theme name; Word recomputes it the same way.
    let body = part(&mut doc, "word/document.xml");
    assert!(body.contains(r#"w:themeColor="accent1""#), "{body}");
}

#[test]
fn table_designs_and_paragraph_borders_follow_the_theme() {
    let mut doc = doc_with("Intro");
    let intro = find_p(&doc, "Intro");
    ops(&mut doc, json!({"op":"insert_table","after":intro,"rows":3,"cols":2}));
    let cell = {
        let mut id = None;
        walk_paragraphs(&doc.body, &mut |p| {
            if id.is_none() && p.plain_text().is_empty() {
                id = Some(p.id);
            }
        });
        id.unwrap()
    };
    ops(&mut doc, json!({"op":"table_design","at":cell,"style":"Grid Table 4 Accent 1"}));
    let table_fill = |doc: &mut Document| {
        doc.render_all().blocks.into_iter().find_map(|b| if let RBlock::Table(t) = b { Some(t) } else { None }).map(|t| (t.rows[0].cells[0].shading.clone(), t.rows[1].cells[0].borders.top.clone().map(|b| b.color)))
    };
    let (fill, border) = table_fill(&mut doc).unwrap();
    assert_eq!(fill.as_deref(), Some("4472C4"));
    ops(&mut doc, json!({"op":"set_theme","theme":"Facet"}));
    let (fill, border2) = table_fill(&mut doc).unwrap();
    assert_eq!(fill.as_deref(), Some("90C226"), "the header row takes the new accent");
    assert_ne!(border, border2, "so do the borders");
    // The paragraph borders of the Intense Quote style follow it too.
    ops(&mut doc, json!({"op":"set_style","ids":[intro],"style":"Intense Quote"}));
    let color = |doc: &Document| {
        doc.render_snapshot().blocks.iter().find_map(|b| if let RBlock::Para(p) = b { Some(p.clone()) } else { None }).and_then(|p| p.fmt.borders.and_then(|b| b.top).map(|t| t.color))
    };
    let facet = color(&doc).unwrap();
    ops(&mut doc, json!({"op":"set_theme","theme":"Wisp"}));
    assert_ne!(color(&doc).unwrap(), facet);
    assert_valid(&doc.save().unwrap());
}

#[test]
fn charts_use_the_theme_s_accents() {
    let mut doc = doc_with("Intro\n\nBody");
    let p = find_p(&doc, "Body");
    ops(&mut doc, json!({"op":"insert_chart","at":{"p":p,"o":0},"kind":"column","title":"Sample","categories":["A","B"],"series":[{"name":"One","values":[1,2]},{"name":"Two","values":[2,1]}]}));
    doc.render_all();
    let accents = |doc: &Document| doc.chart_at(docxtor_core::Pos::new(p, 0)).expect("a chart").accents;
    assert_eq!(accents(&doc).first().map(String::as_str), Some("4472C4"));
    ops(&mut doc, json!({"op":"set_theme","theme":"Facet"}));
    doc.render_all();
    assert_eq!(accents(&doc).first().map(String::as_str), Some("90C226"));
    assert!(doc.undo());
    doc.render_all();
    assert_eq!(accents(&doc).first().map(String::as_str), Some("4472C4"));
}

#[test]
fn shapes_read_the_theme_the_document_has_now() {
    use docxtor_core::shapes::theme::ShapeTheme;
    let mut doc = doc_with("Intro");
    assert_eq!(ShapeTheme::load(&doc.pkg).scheme("accent1"), Some("4472C4"));
    ops(&mut doc, json!({"op":"set_theme","theme":"Wisp"}));
    // Shapes (and anything else drawn from the theme part) resolve scheme colours against the new part at once.
    assert_eq!(ShapeTheme::load(&doc.pkg).scheme("accent1"), Some("A53010"));
    assert!(doc.undo());
    assert_eq!(ShapeTheme::load(&doc.pkg).scheme("accent1"), Some("4472C4"));
}

#[test]
fn a_file_with_no_theme_gets_one_when_a_theme_is_chosen_and_loses_it_on_undo() {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    pkg.remove_rels_to("word/document.xml", "word/theme/theme1.xml");
    pkg.remove_override("word/theme/theme1.xml");
    pkg.remove("word/theme/theme1.xml");
    let mut doc = Document::open(&pkg.to_bytes().unwrap()).unwrap();
    assert!(!Package::open(&doc.save().unwrap()).unwrap().has("word/theme/theme1.xml"));
    ops(&mut doc, json!({"op":"set_theme","theme":"wisp"}));
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    assert!(pkg.has("word/theme/theme1.xml"));
    assert!(pkg.rels_for("word/document.xml").unwrap().iter().any(|r| r.rel_type.ends_with("/theme")));
    assert!(String::from_utf8(pkg.get("[Content_Types].xml").unwrap().to_vec()).unwrap().contains("theme+xml"));
    assert_eq!(doc.theme_info().name, "Wisp");
    assert!(doc.undo());
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    let pkg = Package::open(&bytes).unwrap();
    assert!(!pkg.has("word/theme/theme1.xml"));
    assert!(!pkg.rels_for("word/document.xml").unwrap().iter().any(|r| r.rel_type.ends_with("/theme")));
}

#[test]
fn an_untouched_theme_is_saved_byte_for_byte() {
    let mut a = Document::new_blank();
    let original = part(&mut a, "word/theme/theme1.xml");
    let mut doc = Document::open(&a.save().unwrap()).unwrap();
    let first = doc.render_all().order[0];
    ops(&mut doc, json!({"op":"insert_text","at":{"p":first,"o":0},"text":"hi"}));
    assert_eq!(part(&mut doc, "word/theme/theme1.xml"), original);
}

#[test]
fn every_built_in_theme_applies_and_saves_a_valid_file() {
    for t in docxtor_core::themes::themes() {
        let mut doc = doc_with("# Head\n\nBody");
        ops(&mut doc, json!({"op":"set_theme","theme":t.id}));
        assert_eq!(doc.theme_info().name, t.name);
        assert_valid(&doc.save().unwrap());
    }
}

// ---------------------------------------------------------------------------
// Style sets and spacing
// ---------------------------------------------------------------------------

#[test]
fn a_style_set_rewrites_the_styles_and_the_default_spacing() {
    let mut doc = doc_with("# Head\n\n## Sub\n\nBody");
    let (h, b) = (find_p(&doc, "Head"), find_p(&doc, "Body"));
    let before = styles(&mut doc);
    ops(&mut doc, json!({"op":"set_style_set","style_set":"Word 2003"}));
    let r = run_of(&doc, h);
    assert_eq!((r["font"].as_str(), r["size"].as_f64(), r["bold"].as_bool()), (Some("Arial"), Some(16.0), Some(true)));
    let r = run_of(&doc, b);
    assert_eq!((r["font"].as_str(), r["size"].as_f64()), (Some("Times New Roman"), Some(12.0)));
    // Single spacing and no room after a paragraph, from the document defaults.
    let p = para_of(&doc, b);
    assert_eq!((p["line"].as_i64(), p["spaceAfter"].as_i64()), (Some(240), Some(0)));
    let xml = styles(&mut doc);
    for id in ["Title", "Subtitle", "Heading1", "Heading9", "Quote"] {
        assert!(xml.contains(&format!(r#"w:styleId="{id}""#)), "{id}");
    }
    assert!(doc.undo());
    assert_eq!(styles(&mut doc), before);
    assert_eq!(run_of(&doc, b)["font"], "Calibri");
}

#[test]
fn style_set_colours_name_theme_colours() {
    let mut doc = doc_with("# Head");
    let h = find_p(&doc, "Head");
    ops(&mut doc, json!({"op":"set_style_set","style_set":"word-2010"}));
    let blue = run_of(&doc, h)["color"].as_str().unwrap().to_string();
    ops(&mut doc, json!({"op":"set_theme","theme":"Retrospect"}));
    assert_ne!(run_of(&doc, h)["color"].as_str().unwrap(), blue, "a theme change recolours the set's headings");
}

#[test]
fn a_style_set_from_an_extension_is_applied_from_its_xml() {
    let mut doc = doc_with("# Head");
    let h = find_p(&doc, "Head");
    let xml = r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:rPr><w:i/><w:sz w:val="50"/></w:rPr></w:style>"#;
    ops(&mut doc, json!({"op":"set_style_set","definition":{"name":"Mine","styles":xml,"spacing":{"after":40}}}));
    let r = run_of(&doc, h);
    assert_eq!((r["italic"].as_bool(), r["size"].as_f64()), (Some(true), Some(25.0)));
    assert_eq!(para_of(&doc, h)["spaceAfter"], 40);
    let msg = fails(&mut doc, json!({"op":"set_style_set","definition":{"name":"Bad","styles":"<w:style"}}));
    assert!(msg.contains("style"), "{msg}");
    assert!(fails(&mut doc, json!({"op":"set_style_set","definition":{"name":"Empty","styles":""}})).contains("no styles"));
    assert!(fails(&mut doc, json!({"op":"set_style_set","style_set":"nope"})).contains("no style set"));
}

#[test]
fn spacing_presets_write_the_document_defaults_and_leave_normal_following_them() {
    let mut doc = doc_with("Body");
    let b = find_p(&doc, "Body");
    // A Normal that has spacing of its own would hide the defaults.
    ops(&mut doc, json!({"op":"set_style_definition","style":"Normal","def":{"ppr":{"spaceAfter":500}}}));
    assert_eq!(para_of(&doc, b)["spaceAfter"], 500);
    for (preset, after, line) in [("No paragraph space", 0, 240), ("compact", 80, 240), ("double", 160, 480)] {
        ops(&mut doc, json!({"op":"set_paragraph_spacing","preset":preset}));
        let p = para_of(&doc, b);
        assert_eq!((p["spaceAfter"].as_i64(), p["line"].as_i64()), (Some(after), Some(line)), "{preset}");
    }
    let xml = styles(&mut doc);
    assert!(xml.contains(r#"<w:spacing w:before="0" w:after="160" w:line="480" w:lineRule="auto"/>"#) || xml.contains(r#"w:line="480""#), "{xml}");
    ops(&mut doc, json!({"op":"set_paragraph_spacing","spacing":{"after":300,"line":300}}));
    assert_eq!(para_of(&doc, b)["spaceAfter"], 300);
    assert!(fails(&mut doc, json!({"op":"set_paragraph_spacing","preset":"enormous"})).contains("no spacing preset"));
    assert!(doc.undo());
    assert_eq!(para_of(&doc, b)["spaceAfter"], 160);
}

#[test]
fn the_design_snapshot_carries_theme_styles_and_spacing_to_a_new_document() {
    let mut doc = doc_with("# Head");
    ops(&mut doc, json!({"op":"set_theme","theme":"Facet"}));
    ops(&mut doc, json!({"op":"set_style_set","style_set":"Lines (Simple)"}));
    ops(&mut doc, json!({"op":"set_paragraph_spacing","preset":"open"}));
    let (theme, set) = doc.design_snapshot();
    let mut fresh = doc_with("# Head");
    fresh.apply_json(&json!([{"op":"set_theme","definition":theme},{"op":"set_style_set","definition":set}]).to_string(), false).unwrap();
    assert_eq!(fresh.theme_info().name, "Facet");
    let h = find_p(&fresh, "Head");
    assert_eq!(run_of(&fresh, h), run_of(&doc, find_p(&doc, "Head")));
    assert_eq!(para_of(&fresh, h)["spaceBefore"], para_of(&doc, find_p(&doc, "Head"))["spaceBefore"]);
}

#[test]
fn a_blank_document_can_start_with_a_design() {
    let mut doc = doc_with("# Head");
    ops(&mut doc, json!({"op":"set_theme","theme":"Wisp"}));
    ops(&mut doc, json!({"op":"set_style_set","style_set":"Casual"}));
    ops(&mut doc, json!({"op":"set_paragraph_spacing","preset":"tight"}));
    let (theme, set) = doc.design_snapshot();
    let mut fresh = Document::new_blank_with_design(Some(&theme), Some(&set)).unwrap();
    assert!(!fresh.is_modified(), "a design that is there from the start is not a change");
    assert!(!fresh.undo(), "and not something to undo");
    assert_eq!(fresh.theme_info().name, "Wisp");
    let bytes = fresh.save().unwrap();
    assert_valid(&bytes);
    let mut reopened = Document::open(&bytes).unwrap();
    assert_eq!(reopened.theme_info().name, "Wisp");
    assert!(styles(&mut reopened).contains("Heading1"));
    assert!(styles(&mut reopened).contains(r#"w:line="276""#));
}

// ---------------------------------------------------------------------------
// What the gallery and the panels read
// ---------------------------------------------------------------------------

#[test]
fn the_gallery_shows_the_real_look_of_each_style_and_offers_the_latent_ones() {
    let mut doc = doc_with("# Head\n\nBody\n\nMore");
    ops(&mut doc, json!({"op":"set_style_definition","style":"Heading 1","def":{"rpr":{"size":30,"color":"#336699"}}}));
    let looks = doc.style_gallery();
    let h1 = looks.iter().find(|l| l.id == "Heading1").unwrap();
    assert_eq!((h1.run.size, h1.run.color.as_deref(), h1.quick, h1.latent, h1.uses), (Some(30.0), Some("336699"), true, false, 1));
    // Heading 1's font is the theme's heading font, resolved.
    assert_eq!(h1.run.font.as_deref(), Some("Calibri Light"));
    let normal = looks.iter().find(|l| l.id == "Normal").unwrap();
    assert_eq!((normal.uses, normal.run.size, normal.run.font.as_deref()), (2, Some(11.0), Some("Calibri")));
    // Built-in styles the file lacks are listed, drawn as Word writes them.
    let quote = looks.iter().find(|l| l.id == "IntenseQuote").unwrap();
    assert!(quote.latent && quote.run.italic == Some(true) && quote.para.align.as_deref() == Some("center"));
    let names: Vec<&str> = looks.iter().filter(|l| l.latent).map(|l| l.name.as_str()).collect();
    for want in ["Title", "Subtitle", "Heading 9", "Quote", "Emphasis", "Strong", "No Spacing", "List Paragraph"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    // Choosing a latent style adds it.
    let p = find_p(&doc, "Body");
    ops(&mut doc, json!({"op":"set_style","ids":[p],"style":"Intense Quote"}));
    assert!(doc.style_gallery().iter().find(|l| l.id == "IntenseQuote").is_some_and(|l| !l.latent && l.uses == 1));
}

#[test]
fn style_details_describe_a_style_and_what_it_builds_on() {
    let mut doc = doc_with("Body");
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Deep","basedOn":"Heading 2","rpr":{"italic":true},"ppr":{"spaceAfter":300}}}));
    let d = doc.style_details("Deep").unwrap();
    assert_eq!((d.name.as_str(), d.kind.as_str(), d.based_on.as_deref(), d.custom, d.latent), ("Deep", "paragraph", Some("Heading2"), true, false));
    assert_eq!(d.own_run.italic, Some(true));
    assert_eq!(d.own_para.space_after, Some(300));
    assert_eq!(d.own_run.size, None);
    // The resolved look includes what it builds on.
    assert_eq!(d.run.size, Some(16.0));
    let latent = doc.style_details("Subtitle").unwrap();
    assert!(latent.latent && !latent.custom && latent.run.size == Some(14.0));
    assert!(doc.style_details("Not a style").is_none());
}

#[test]
fn paragraph_style_uses_list_every_paragraph() {
    let doc = doc_with("# A\n\nx\n\n# B");
    let uses = doc.style_uses("Heading 1");
    assert_eq!(uses.len(), 2);
    assert!(uses.iter().all(|u| u.from == 0 && u.to == 1));
    assert_eq!(doc.style_uses("Normal").len(), 1);
    assert!(doc.style_uses("Missing").is_empty());
}

// ---------------------------------------------------------------------------
// The assistant's tools
// ---------------------------------------------------------------------------

#[test]
fn the_assistant_can_restyle_themes_and_spacing() {
    use docxtor_core::ai::call_tool;
    let mut doc = doc_with("# Head\n\nBody");
    let (h, b) = (find_p(&doc, "Head"), find_p(&doc, "Body"));
    call_tool(&mut doc, "define_style", &json!({"style":"Heading 1","size_pt":22,"bold":true,"color":"1F4E79","space_after_pt":12,"align":"center"})).unwrap();
    let r = run_of(&doc, h);
    assert_eq!((r["size"].as_f64(), r["bold"].as_bool(), r["color"].as_str()), (Some(22.0), Some(true), Some("1F4E79")));
    assert_eq!(para_of(&doc, h)["spaceAfter"], 240);
    let made = call_tool(&mut doc, "define_style", &json!({"style":"Pull quote","kind":"paragraph","italic":true,"indent_left_pt":36,"in_gallery":true})).unwrap();
    assert!(made.contains("made"), "{made}");
    assert!(doc.style_details("Pull quote").is_some_and(|d| d.custom && d.quick));
    call_tool(&mut doc, "set_theme", &json!({"theme":"Frame"})).unwrap();
    assert_eq!(doc.theme_info().name, "Frame");
    call_tool(&mut doc, "set_theme", &json!({"colors":{"accent1":"C00000"}})).unwrap();
    assert_eq!(doc.theme_info().colors.colors["accent1"], "C00000");
    call_tool(&mut doc, "set_theme", &json!({"heading_font":"Georgia","body_font":"Verdana"})).unwrap();
    assert_eq!(run_of(&doc, b)["font"], "Verdana");
    assert!(call_tool(&mut doc, "set_theme", &json!({"heading_font":"Georgia"})).is_err());
    call_tool(&mut doc, "set_style_set", &json!({"name":"Minimalist"})).unwrap();
    call_tool(&mut doc, "set_paragraph_spacing", &json!({"preset":"compact"})).unwrap();
    assert_eq!(para_of(&doc, b)["spaceAfter"], 80);
    let list = call_tool(&mut doc, "list_design", &json!({})).unwrap();
    assert!(list.contains("Theme: Frame") && list.contains("Style sets:") && list.contains("Lines (Simple)") && list.contains("accent1 C00000"), "{list}");
    let id = find_p(&doc, "Body");
    call_tool(&mut doc, "create_style_from_selection", &json!({"name":"Plain note","id":id})).unwrap();
    call_tool(&mut doc, "delete_style", &json!({"style":"Plain note"})).unwrap();
    assert!(call_tool(&mut doc, "delete_style", &json!({"style":"Normal"})).is_err());
    assert_valid(&doc.save().unwrap());
}

// ---------------------------------------------------------------------------
// Reveal formatting
// ---------------------------------------------------------------------------

#[test]
fn reveal_formatting_says_where_each_setting_comes_from() {
    let mut doc = doc_with("# Head\n\nBody text");
    let h = find_p(&doc, "Head");
    ops(&mut doc, json!({"op":"set_style_definition","def":{"name":"Marker","kind":"character","rpr":{"italic":true,"color":"#CC0000"}}}));
    ops(&mut doc, json!({"op":"format_text","from":{"p":h,"o":0},"to":{"p":h,"o":4},"format":{"style":"Marker","size":30}}));
    ops(&mut doc, json!({"op":"format_paragraphs","ids":[h],"format":{"align":"center"}}));
    let r = doc.reveal_formatting(docxtor_core::Pos::new(h, 2)).unwrap();
    assert_eq!((r.paragraph_style.name.as_str(), r.character_style.as_ref().map(|c| c.name.as_str())), ("Heading 1", Some("Marker")));
    assert!(r.direct_paragraph && r.direct_run);
    let row = |key: &str| r.rows.iter().find(|x| x.key == key).unwrap_or_else(|| panic!("no row {key}"));
    // Size by hand, colour from the character style, font from Heading 1 (the theme's heading font), line spacing from the defaults.
    assert_eq!((row("size").source, row("size").value.as_f64()), ("direct", Some(30.0)));
    assert_eq!((row("color").source, row("color").style.as_deref()), ("characterStyle", Some("Marker")));
    // A style of the person's own says so and Word's does not, so the panel keeps the one's case and not the other's.
    assert!(row("color").custom && r.character_style.as_ref().unwrap().custom);
    assert!(!row("font").custom && !r.paragraph_style.custom);
    assert_eq!((row("font").source, row("font").value.as_str(), row("font").style.as_deref()), ("paragraphStyle", Some("Calibri Light"), Some("Heading 1")));
    assert_eq!((row("alignment").source, row("alignment").value.as_str()), ("direct", Some("center")));
    assert_eq!(row("spaceBefore").source, "paragraphStyle");
    assert_eq!(row("lineSpacing").source, "default");
    assert_eq!(row("keepWithNext").style.as_deref(), Some("Heading 1"));
    // Plain text has nothing by hand and no character style.
    let b = doc.reveal_formatting(docxtor_core::Pos::new(find_p(&doc, "Body"), 1)).unwrap();
    assert!(!b.direct_paragraph && !b.direct_run && b.character_style.is_none());
    assert_eq!(b.paragraph_style.name, "Normal");
    assert!(b.rows.iter().all(|x| x.source == "default" || x.source == "paragraphStyle"));
}
