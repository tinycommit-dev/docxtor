//! Word 2003 XML (WordprocessingML before Word 2007, `w:wordDocument`) into a fragment.
//!
//! The 2003 format is close to today's markup: paragraphs, runs, tables and styles keep their names, so
//! the reader copies them across, renames what changed (`w:hlink` is now `w:hyperlink`, `w:listPr` is
//! `w:numPr`, `wx:font` is `w:rFonts`) and drops Word's own layout hints (`wx:` elements). Pictures are
//! base64 `w:binData` blocks named by `wordml://` addresses. Lost: section breaks (the last section's
//! page setup is used), headers and footers, comments, text boxes and drawings.

use super::text::{Converted, Meta, Page};
use super::{Note, OpenError};
use crate::fragment::{b64_decode, inline_drawing, sniff_image, Builder, FragDefaults, LevelSpec};
use crate::props::{put_ordered, put_ppr, put_rpr};
use crate::xml::{self, Element, Node};
use std::collections::HashMap;

const TCPR_ORDER: &[&str] = &["w:cnfStyle", "w:tcW", "w:gridSpan", "w:hMerge", "w:vMerge", "w:tcBorders", "w:shd", "w:noWrap", "w:tcMar", "w:textDirection", "w:tcFitText", "w:vAlign", "w:hideMark"];
const TBLPR_ORDER: &[&str] = &[
    "w:tblStyle", "w:tblpPr", "w:tblOverlap", "w:bidiVisual", "w:tblStyleRowBandSize", "w:tblStyleColBandSize", "w:tblW", "w:jc", "w:tblCellSpacing", "w:tblInd", "w:tblBorders", "w:shd", "w:tblLayout",
    "w:tblCellMar", "w:tblLook",
];

const MAX_DEPTH: usize = 100;

struct Ctx {
    b: Builder,
    media: HashMap<String, Vec<u8>>,
    lists: HashMap<i64, i64>,
    pictures_lost: usize,
    objects_lost: usize,
    /// How deep the markup being read is nested; deeper than any real file goes is left out.
    depth: usize,
}

pub fn read(bytes: &[u8]) -> Result<Converted, OpenError> {
    let doc = match xml::parse(bytes) {
        Ok(d) => d,
        Err(_) => {
            let r = super::xmlfix::recover(bytes).ok_or_else(|| OpenError::new("damaged", "The XML file can't be read."))?;
            xml::parse(&r.bytes).map_err(|e| OpenError::new("damaged", e.to_string()))?
        }
    };
    let root = doc.root;
    if root.name != "w:wordDocument" {
        return Err(OpenError::new("unknown", "This XML file is not a Word 2003 document."));
    }
    let mut cx = Ctx { b: Builder::new(), media: HashMap::new(), lists: HashMap::new(), pictures_lost: 0, objects_lost: 0, depth: 0 };
    collect_media(&root, &mut cx.media);
    let mut defaults = None;
    if let Some(styles) = root.child("w:styles") {
        defaults = read_styles(styles, &mut cx);
    }
    if let Some(lists) = root.child("w:lists") {
        read_lists(lists, &mut cx);
    }
    let body = root.child("w:body").ok_or_else(|| OpenError::new("no-text", "The document has no body."))?;
    let mut sect: Option<&Element> = None;
    blocks(body, &mut cx, &mut sect);
    let page = sect.and_then(page_of);
    let mut notes = Vec::new();
    if cx.pictures_lost > 0 {
        notes.push(Note::count("picture-unreadable", cx.pictures_lost));
    }
    if cx.objects_lost > 0 {
        notes.push(Note::count("object-dropped", cx.objects_lost));
    }
    if body.find("w:hdr").is_some() || body.find("w:ftr").is_some() || root.find("w:hdr").is_some() || root.find("w:ftr").is_some() {
        notes.push(Note::new("lost-headers"));
    }
    cx.b.set_block(true);
    let mut fragment = cx.b.finish();
    fragment.defaults = defaults;
    Ok(Converted { fragment, meta: meta_of(&root), page, notes })
}

fn meta_of(root: &Element) -> Meta {
    let mut m = Meta::default();
    if let Some(p) = root.child("o:DocumentProperties") {
        let t = |n: &str| p.child(n).map(|e| e.text().trim().to_string()).filter(|s| !s.is_empty());
        m.title = t("o:Title");
        m.author = t("o:Author");
        m.subject = t("o:Subject");
        m.keywords = t("o:Keywords");
        m.description = t("o:Description");
    }
    m
}

fn page_of(sect: &Element) -> Option<Page> {
    let n = |e: Option<&Element>, a: &str| e.and_then(|e| e.attr(a)).and_then(|v| v.parse::<f64>().ok()).map(|v| v as i64);
    let sz = sect.child("w:pgSz");
    let mar = sect.child("w:pgMar");
    let (w, h) = (n(sz, "w:w")?, n(sz, "w:h")?);
    Some(Page { width: w, height: h, margins: [n(mar, "w:top").unwrap_or(1440), n(mar, "w:right").unwrap_or(1440), n(mar, "w:bottom").unwrap_or(1440), n(mar, "w:left").unwrap_or(1440)] })
}

fn collect_media(el: &Element, out: &mut HashMap<String, Vec<u8>>) {
    if el.name == "w:binData" {
        if let Some(name) = el.attr("w:name") {
            if let Some(data) = b64_decode(&el.text()) {
                out.insert(name.to_string(), data);
            }
        }
        return;
    }
    for c in el.elements() {
        collect_media(c, out);
    }
}

// ---------------------------------------------------------------------------
// Styles and lists
// ---------------------------------------------------------------------------

fn read_styles(styles: &Element, cx: &mut Ctx) -> Option<FragDefaults> {
    let mut defaults = None;
    for s in styles.children_named("w:style") {
        let Some(id) = s.attr("w:styleId") else { continue };
        let mut out = Element::new("w:style").with_attr("w:type", s.attr("w:type").unwrap_or("paragraph")).with_attr("w:styleId", id);
        if matches!(s.attr("w:default"), Some("on") | Some("1") | Some("true")) {
            out.set_attr("w:default", "1");
        }
        for tag in ["w:name", "w:basedOn", "w:next", "w:link"] {
            if let Some(c) = s.child(tag) {
                out = out.with_child(c.clone());
            }
        }
        if let Some(p) = s.child("w:pPr") {
            out = out.with_child(convert_ppr(p, cx));
        }
        if let Some(r) = s.child("w:rPr") {
            let r = convert_rpr(r);
            if id == "Normal" || s.attr("w:default").is_some() && s.attr("w:type") == Some("paragraph") {
                let font = r.child("w:rFonts").and_then(|f| f.attr("w:ascii")).map(String::from);
                let size = r.child("w:sz").and_then(|z| z.attr("w:val")).and_then(|v| v.parse::<f64>().ok()).map(|v| v / 2.0);
                if font.is_some() || size.is_some() {
                    defaults = Some(FragDefaults { font, size });
                }
            }
            out = out.with_child(r);
        }
        if let Some(t) = s.child("w:tblPr") {
            out = out.with_child(sorted(t, TBLPR_ORDER));
        }
        cx.b.style(out);
    }
    defaults
}

fn read_lists(lists: &Element, cx: &mut Ctx) {
    let defs: HashMap<String, &Element> = lists.children_named("w:listDef").filter_map(|d| Some((d.attr("w:listDefId")?.to_string(), d))).collect();
    for l in lists.children_named("w:list") {
        let (Some(ilfo), Some(def)) = (l.attr("w:ilfo").and_then(|v| v.parse::<i64>().ok()), l.child("w:ilst").and_then(|i| i.attr("w:val")).and_then(|v| defs.get(v))) else { continue };
        let levels: Vec<LevelSpec> = def.children_named("w:lvl").map(level_spec).collect();
        if levels.is_empty() {
            continue;
        }
        let num = cx.b.list(&levels);
        cx.lists.insert(ilfo, num);
    }
}

fn level_spec(l: &Element) -> LevelSpec {
    let n = |e: Option<&Element>, a: &str| e.and_then(|e| e.attr(a)).and_then(|v| v.parse::<i64>().ok());
    let fmt = match n(l.child("w:nfc"), "w:val").unwrap_or(0) {
        1 => "upperRoman",
        2 => "lowerRoman",
        3 => "upperLetter",
        4 => "lowerLetter",
        23 | 22 => "bullet",
        255 => "none",
        _ => "decimal",
    };
    let ind = l.child("w:pPr").and_then(|p| p.child("w:ind"));
    let font = l.child("w:rPr").and_then(|r| r.child("wx:font")).and_then(|f| f.attr("wx:val")).map(String::from);
    LevelSpec {
        fmt: fmt.into(),
        text: l.child("w:lvlText").and_then(|t| t.attr("w:val")).map(String::from).unwrap_or_else(|| if fmt == "bullet" { "\u{2022}".into() } else { "%1.".into() }),
        start: n(l.child("w:start"), "w:val").unwrap_or(1),
        left: n(ind, "w:left").unwrap_or(720),
        hanging: n(ind, "w:hanging").unwrap_or(360),
        font: if fmt == "bullet" { font } else { None },
    }
}

// ---------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------

fn blocks<'a>(parent: &'a Element, cx: &mut Ctx, sect: &mut Option<&'a Element>) {
    if cx.depth > MAX_DEPTH {
        return;
    }
    cx.depth += 1;
    for c in parent.elements() {
        match c.name.as_str() {
            "w:p" => {
                let p = convert_p(c, cx);
                cx.b.block(p);
            }
            "w:tbl" => {
                let t = convert_tbl(c, cx);
                cx.b.block(t);
            }
            "w:sectPr" => *sect = Some(c),
            "wx:sect" | "wx:sub-section" | "wx:pBdrGroup" => blocks(c, cx, sect),
            _ => {}
        }
    }
    cx.depth -= 1;
}

fn convert_tbl(t: &Element, cx: &mut Ctx) -> Element {
    let mut out = Element::new("w:tbl");
    if cx.depth > MAX_DEPTH {
        return out;
    }
    cx.depth += 1;
    for c in t.elements() {
        match c.name.as_str() {
            "w:tblPr" => out.children.push(Node::Element(sorted(&strip_wx(c), TBLPR_ORDER))),
            "w:tblGrid" => out.children.push(Node::Element(c.clone())),
            "w:tr" => {
                let mut tr = Element::new("w:tr");
                for rc in c.elements() {
                    match rc.name.as_str() {
                        "w:trPr" => tr.children.push(Node::Element(strip_wx(rc))),
                        "w:tc" => {
                            let mut tc = Element::new("w:tc");
                            let mut paras = 0;
                            for cc in rc.elements() {
                                match cc.name.as_str() {
                                    "w:tcPr" => tc.children.push(Node::Element(sorted(&strip_wx(cc), TCPR_ORDER))),
                                    "w:p" => {
                                        tc.children.push(Node::Element(convert_p(cc, cx)));
                                        paras += 1;
                                    }
                                    "w:tbl" => {
                                        tc.children.push(Node::Element(convert_tbl(cc, cx)));
                                        paras += 1;
                                    }
                                    _ => {}
                                }
                            }
                            if paras == 0 || matches!(tc.children.last(), Some(Node::Element(e)) if e.name == "w:tbl") {
                                tc.children.push(Node::Element(Element::new("w:p")));
                            }
                            tr.children.push(Node::Element(tc));
                        }
                        _ => {}
                    }
                }
                out.children.push(Node::Element(tr));
            }
            _ => {}
        }
    }
    cx.depth -= 1;
    out
}

/// Word 2003 XML spells some names with hyphens (`w:sz-cs`, `w:first-line`, `w:line-rule`) where the Open XML of Word 2007 on
/// has camel case (`w:szCs`, `w:firstLine`, `w:lineRule`), and `w:fareast` where it has `w:eastAsia`. Found by the Open XML
/// validator on the converted sample.
fn ooxml_name(name: &str) -> String {
    if name == "w:fareast" {
        return "w:eastAsia".into();
    }
    if !name.starts_with("w:") || !name.contains('-') {
        return name.to_string();
    }
    let mut out = String::new();
    let mut up = false;
    for c in name.chars() {
        if c == '-' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn strip_wx(el: &Element) -> Element {
    let mut out = Element { name: ooxml_name(&el.name).into(), attrs: el.attrs.iter().filter(|(k, _)| !k.starts_with("wx:")).map(|(k, v)| (ooxml_name(k).into(), v.clone())).collect(), children: Vec::new() };
    for n in &el.children {
        match n {
            Node::Element(c) if c.name.starts_with("wx:") => {}
            Node::Element(c) => out.children.push(Node::Element(strip_wx(c))),
            other => out.children.push(other.clone()),
        }
    }
    out
}

fn sorted(el: &Element, order: &[&str]) -> Element {
    let mut out = Element { name: el.name.clone(), attrs: el.attrs.clone(), children: Vec::new() };
    for c in el.elements() {
        put_ordered(&mut out, c.clone(), order);
    }
    out
}

fn convert_rpr(r: &Element) -> Element {
    let mut out = Element::new("w:rPr");
    for c in r.elements() {
        match c.name.as_str() {
            "wx:font" => {
                if let Some(f) = c.attr("wx:val") {
                    put_rpr(&mut out, Element::new("w:rFonts").with_attr("w:ascii", f).with_attr("w:hAnsi", f).with_attr("w:cs", f));
                }
            }
            n if n.starts_with("wx:") => {}
            "w:rFonts" => put_rpr(&mut out, strip_wx(c)),
            _ => put_rpr(&mut out, strip_wx(c)),
        }
    }
    out
}

fn convert_ppr(p: &Element, cx: &mut Ctx) -> Element {
    let mut out = Element::new("w:pPr");
    for c in p.elements() {
        match c.name.as_str() {
            "w:listPr" => {
                let ilvl = c.child("w:ilvl").and_then(|e| e.attr("w:val")).unwrap_or("0");
                if let Some(num) = c.child("w:ilfo").and_then(|e| e.attr("w:val")).and_then(|v| v.parse::<i64>().ok()).and_then(|i| cx.lists.get(&i)) {
                    put_ppr(&mut out, Element::new("w:numPr").with_child(Element::new("w:ilvl").with_attr("w:val", ilvl)).with_child(Element::new("w:numId").with_attr("w:val", num.to_string())));
                }
            }
            "w:rPr" => put_ppr(&mut out, convert_rpr(c)),
            "w:sectPr" => {}
            n if n.starts_with("wx:") => {}
            "w:ind" => {
                // Word 2003 gives a hanging indent as a negative `first-line`; Open XML has `w:hanging` and no negative `w:firstLine`.
                let mut ind = strip_wx(c);
                if let Some(v) = ind.attr("w:firstLine").and_then(|v| v.trim().parse::<i64>().ok()).filter(|v| *v < 0) {
                    ind.attrs.retain(|(k, _)| k != "w:firstLine");
                    ind.set_attr("w:hanging", (-v).to_string());
                }
                put_ppr(&mut out, ind);
            }
            _ => put_ppr(&mut out, strip_wx(c)),
        }
    }
    out
}

fn convert_p(p: &Element, cx: &mut Ctx) -> Element {
    let mut out = Element::new("w:p");
    for c in p.elements() {
        match c.name.as_str() {
            "w:pPr" => out.children.push(Node::Element(convert_ppr(c, cx))),
            _ => inline(c, cx, &mut out),
        }
    }
    out
}

/// Convert something inside a paragraph and add it to `out`.
fn inline(el: &Element, cx: &mut Ctx, out: &mut Element) {
    if cx.depth > MAX_DEPTH {
        return;
    }
    cx.depth += 1;
    inline_in(el, cx, out);
    cx.depth -= 1;
}

fn inline_in(el: &Element, cx: &mut Ctx, out: &mut Element) {
    match el.name.as_str() {
        "w:r" => {
            let mut r = Element::new("w:r");
            for c in el.elements() {
                match c.name.as_str() {
                    "w:rPr" => r.children.push(Node::Element(convert_rpr(c))),
                    "w:pict" => match picture(c, cx) {
                        Some(d) => r.children.push(Node::Element(d)),
                        None => cx.pictures_lost += 1,
                    },
                    "w:object" | "w:binData" => cx.objects_lost += (c.name == "w:object") as usize,
                    n if n.starts_with("wx:") => {}
                    _ => r.children.push(Node::Element(strip_wx(c))),
                }
            }
            out.children.push(Node::Element(r));
        }
        "w:hlink" => {
            let mut h = Element::new("w:hyperlink");
            if let Some(dest) = el.attr("w:dest") {
                let key = cx.b.link(dest);
                h.set_attr("r:id", key);
            }
            if let Some(a) = el.attr("w:bookmark") {
                h.set_attr("w:anchor", a);
            }
            for c in el.elements() {
                inline(c, cx, &mut h);
            }
            out.children.push(Node::Element(h));
        }
        "w:fldSimple" | "w:smartTag" | "w:ins" | "w:subDoc" => {
            // Keep a field's result and the text inside smart tags and insertions.
            let mut inner = Element::new(if el.name == "w:fldSimple" { "w:fldSimple" } else { "w:r" });
            if el.name == "w:fldSimple" {
                if let Some(i) = el.attr("w:instr") {
                    inner.set_attr("w:instr", i);
                }
                for c in el.elements() {
                    inline(c, cx, &mut inner);
                }
                out.children.push(Node::Element(inner));
            } else {
                for c in el.elements() {
                    inline(c, cx, out);
                }
            }
        }
        "w:pict" => match picture(el, cx) {
            Some(d) => out.children.push(Node::Element(Element::new("w:r").with_child(d))),
            None => cx.pictures_lost += 1,
        },
        "w:bookmarkStart" | "w:bookmarkEnd" | "w:proofErr" | "w:del" | "w:permStart" | "w:permEnd" => {}
        n if n.starts_with("wx:") || n == "w:pPr" => {}
        _ => out.children.push(Node::Element(strip_wx(el))),
    }
}

/// A `w:pict` with an image as an inline drawing.
fn picture(pict: &Element, cx: &mut Ctx) -> Option<Element> {
    let shape = pict.find("v:shape").or_else(|| pict.find("v:rect"))?;
    let data = shape.find("v:imagedata")?;
    let src = data.attr("src")?;
    let bytes = cx.media.get(src)?.clone();
    let key = cx.b.media(&bytes)?;
    let (_, pw, ph) = sniff_image(&bytes)?;
    let style = shape.attr("style").unwrap_or_default();
    let dim = |name: &str| -> Option<f64> {
        let v = style.split(';').find_map(|kv| kv.trim().strip_prefix(&format!("{name}:")))?;
        let n: f64 = v.trim_end_matches(|c: char| c.is_alphabetic()).parse().ok()?;
        Some(match v.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.') {
            "in" => n * 72.0,
            "cm" => n * 28.346,
            "mm" => n * 2.8346,
            "px" => n * 0.75,
            _ => n,
        })
    };
    let (w, h) = match (dim("width"), dim("height")) {
        (Some(w), Some(h)) => (w, h),
        _ => (pw as f64 * 0.75, ph as f64 * 0.75),
    };
    let id = cx.b.drawing_id();
    Some(inline_drawing(&key, w.max(1.0), h.max(1.0), id, data.attr("o:title").unwrap_or("")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::walk_paragraphs;
    use crate::open::{text, Format};

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    pub(crate) fn sample() -> String {
        format!(
            r#"<?xml version="1.0" standalone="yes"?><?mso-application progid="Word.Document"?>
<w:wordDocument xmlns:w="http://schemas.microsoft.com/office/word/2003/wordml" xmlns:wx="http://schemas.microsoft.com/office/word/2003/auxHint" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xml:space="preserve">
<o:DocumentProperties><o:Title>Old report</o:Title><o:Author>Grace</o:Author></o:DocumentProperties>
<w:lists><w:listDef w:listDefId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:nfc w:val="23"/><w:lvlText w:val="&#xF0B7;"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr><w:rPr><wx:font wx:val="Symbol"/></w:rPr></w:lvl></w:listDef><w:list w:ilfo="1"><w:ilst w:val="0"/></w:list></w:lists>
<w:styles><w:style w:type="paragraph" w:default="on" w:styleId="Normal"><w:name w:val="Normal"/><wx:uiName wx:val="Normal"/><w:rPr><wx:font wx:val="Times New Roman"/><w:sz w:val="24"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:keepNext/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style></w:styles>
<w:body><wx:sect><wx:sub-section>
<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter one</w:t></w:r></w:p>
<w:p><w:pPr><w:listPr><w:ilvl w:val="0"/><w:ilfo w:val="1"/><wx:t wx:val="&#xF0B7;"/></w:listPr></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Bold item</w:t></w:r></w:p>
<w:p><w:r><w:t>See </w:t></w:r><w:hlink w:dest="https://example.com/x"><w:r><w:rPr><w:rStyle w:val="Hyperlink"/></w:rPr><w:t>the site</w:t></w:r></w:hlink><w:r><w:pict><w:binData w:name="wordml://01.png">{PNG}</w:binData><v:shape style="width:30pt;height:20pt"><v:imagedata src="wordml://01.png" o:title="dot"/></v:shape></w:pict></w:r></w:p>
<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1000" w:right="1100" w:bottom="1200" w:left="1300"/></w:sectPr>
</wx:sub-section></wx:sect></w:body></w:wordDocument>"#
        )
    }

    #[test]
    fn a_word_2003_document_opens() {
        let conv = read(sample().as_bytes()).unwrap();
        assert_eq!(conv.fragment.defaults.as_ref().and_then(|d| d.font.as_deref()), Some("Times New Roman"), "{:?}", conv.fragment.defaults);
        let o = text::finish(Format::Word2003, conv).unwrap();
        let mut doc = o.doc;
        let mut texts = Vec::new();
        walk_paragraphs(&doc.body, &mut |p| texts.push(p.plain_text()));
        assert_eq!(&texts[..3], ["Chapter one", "Bold item", "See the site"]);
        assert!(texts.iter().any(|t| t == "A") && texts.iter().any(|t| t == "B"));
        let bytes = doc.save().unwrap();
        let pkg = crate::package::Package::open(&bytes).unwrap();
        let body = String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap();
        assert!(body.contains("w:hyperlink") && body.contains("w:numId") && body.contains("<w:drawing>"), "{body}");
        assert!(body.contains("Heading1"));
        // The Normal style's font and size (Times New Roman, 12 points) are the document's.
        let styles = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
        assert!(styles.contains("Times New Roman") && styles.contains(r#"<w:sz w:val="24"/>"#), "{styles}");
        let setup = doc.page_setup();
        assert_eq!(setup.margin, [1000, 1100, 1200, 1300]);
        let core = String::from_utf8(pkg.get("docProps/core.xml").unwrap().to_vec()).unwrap();
        assert!(core.contains("Old report") && core.contains("Grace"), "{core}");
    }
}
