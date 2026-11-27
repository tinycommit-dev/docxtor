//! Sheets of labels. A sheet is a table, as Word makes it: a column for each label across, and between them a narrow
//! column for the gap; every row is as tall as the labels are far apart (`w:trHeight` exact); the page margins put the
//! first label where the sheet has it. The products are listed with their sizes so a person picks "Avery 5160" rather
//! than typing numbers; any extension can add more (they reach the engine as a `LabelSpec`).

use crate::model::*;
use crate::props::{put_ordered, put_rpr};
use crate::xml::{Element, Node};
use serde::{Deserialize, Serialize};

/// A kind of label sheet. Sizes are in twips (a twentieth of a point; 1440 to the inch).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabelSpec {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub name: String,
    pub page_w: i64,
    pub page_h: i64,
    /// From the top of the page to the top of the first label.
    pub top: i64,
    /// From the left of the page to the left of the first label.
    pub left: i64,
    pub label_w: i64,
    pub label_h: i64,
    /// From the left of one label to the left of the next.
    pub pitch_h: i64,
    /// From the top of one label to the top of the next.
    pub pitch_v: i64,
    pub across: u32,
    pub down: u32,
}

/// What goes on the labels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LabelFill {
    /// Every label has the same text.
    Same { lines: Vec<String> },
    /// One label has the text (counted from 1).
    Single { lines: Vec<String>, row: u32, col: u32 },
    /// A mail merge: the first label is empty, the others start with Next Record. Insert the address block in the
    /// first label and use Update labels to copy it.
    Merge,
}

/// How the text on a label looks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TextLook {
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub size_pt: Option<f64>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
}

pub(crate) fn rpr_of(look: &TextLook) -> Option<Element> {
    let mut rpr = Element::new("w:rPr");
    if let Some(f) = look.font.as_deref().filter(|f| !f.is_empty()) {
        let mut e = Element::new("w:rFonts");
        for a in ["w:ascii", "w:hAnsi", "w:cs"] {
            e.set_attr(a, f);
        }
        put_rpr(&mut rpr, e);
    }
    if look.bold {
        put_rpr(&mut rpr, Element::new("w:b"));
    }
    if look.italic {
        put_rpr(&mut rpr, Element::new("w:i"));
    }
    if let Some(sz) = look.size_pt.filter(|s| *s > 0.0) {
        let half = (sz * 2.0).round().to_string();
        put_rpr(&mut rpr, Element::new("w:sz").with_attr("w:val", half.clone()));
        put_rpr(&mut rpr, Element::new("w:szCs").with_attr("w:val", half));
    }
    (!rpr.children.is_empty()).then_some(rpr)
}

/// A paragraph with no space around it, holding some lines of text (one paragraph for each).
pub(crate) fn text_paragraph(line: &str, look: &TextLook, indent: Option<i64>, before: Option<i64>, exact: Option<i64>) -> Element {
    let mut sp = Element::new("w:spacing").with_attr("w:before", before.unwrap_or(0).to_string()).with_attr("w:after", "0");
    match exact {
        Some(h) => {
            sp.set_attr("w:line", h.to_string());
            sp.set_attr("w:lineRule", "exact");
        }
        None => {
            sp.set_attr("w:line", "240");
            sp.set_attr("w:lineRule", "auto");
        }
    }
    let mut ppr = Element::new("w:pPr").with_child(sp);
    if let Some(i) = indent.filter(|i| *i > 0) {
        ppr.children.push(Node::Element(Element::new("w:ind").with_attr("w:left", i.to_string())));
    }
    // The paragraph mark has the size too: an empty paragraph is as tall as its mark.
    if let Some(rpr) = rpr_of(look) {
        ppr.children.push(Node::Element(rpr));
    }
    let mut p = Element::new("w:p").with_child(ppr);
    if !line.is_empty() {
        let mut r = Element::new("w:r");
        if let Some(rpr) = rpr_of(look) {
            r.children.push(Node::Element(rpr));
        }
        r.children.push(Node::Element(Element::new("w:t").with_attr("xml:space", "preserve").with_text(line)));
        p.children.push(Node::Element(r));
    }
    p
}

fn inch(v: f64) -> i64 {
    (v * 1440.0).round() as i64
}

fn mm(v: f64) -> i64 {
    (v * 1440.0 / 25.4).round() as i64
}

const LETTER: (i64, i64) = (12240, 15840);
const A4: (i64, i64) = (11906, 16838);

#[allow(clippy::too_many_arguments)]
fn spec(id: &str, vendor: &str, name: &str, page: (i64, i64), top: i64, left: i64, label_w: i64, label_h: i64, pitch_h: i64, pitch_v: i64, across: u32, down: u32) -> LabelSpec {
    LabelSpec { id: id.into(), vendor: vendor.into(), name: name.into(), page_w: page.0, page_h: page.1, top, left, label_w, label_h, pitch_h, pitch_v, across, down }
}

/// Docxtor's own list of label products.
pub fn products() -> Vec<LabelSpec> {
    let mut out = Vec::new();
    let us = "Avery US Letter";
    // Address labels, 1 inch high, three across.
    let five160 = |id: &str, name: &str| spec(id, us, name, LETTER, inch(0.5), inch(0.1875), inch(2.625), inch(1.0), inch(2.75), inch(1.0), 3, 10);
    // The two-across sheets: 4 inches wide, a gap of 3/16 inch, 5/32 inch from the edge.
    let two = |id: &str, name: &str, h: f64, top: f64, down: u32| spec(id, us, name, LETTER, inch(top), inch(0.15625), inch(4.0), inch(h), inch(4.1875), inch(h), 2, down);
    out.push(five160("avery-5160", "5160 Address labels"));
    out.push(two("avery-5161", "5161 Address labels", 1.0, 0.5, 10));
    out.push(two("avery-5162", "5162 Address labels", 4.0 / 3.0, 5.0 / 6.0, 7));
    out.push(two("avery-5163", "5163 Shipping labels", 2.0, 0.5, 5));
    out.push(two("avery-5164", "5164 Shipping labels", 10.0 / 3.0, 0.5, 3));
    out.push(spec("avery-5167", us, "5167 Return address labels", LETTER, inch(0.5), inch(0.28125), inch(1.75), inch(0.5), inch(2.0625), inch(0.5), 4, 20));
    out.push(spec("avery-5371", us, "5371 Business cards", LETTER, inch(0.5), inch(0.75), inch(3.5), inch(2.0), inch(3.5), inch(2.0), 2, 5));
    out.push(five160("avery-5260", "5260 Address labels"));
    out.push(two("avery-5261", "5261 Address labels", 1.0, 0.5, 10));
    out.push(two("avery-5262", "5262 Address labels", 4.0 / 3.0, 5.0 / 6.0, 7));
    out.push(two("avery-5263", "5263 Shipping labels", 2.0, 0.5, 5));
    out.push(two("avery-5264", "5264 Shipping labels", 10.0 / 3.0, 0.5, 3));
    out.push(five160("avery-8160", "8160 Address labels"));
    out.push(two("avery-8161", "8161 Address labels", 1.0, 0.5, 10));
    out.push(two("avery-8162", "8162 Address labels", 4.0 / 3.0, 5.0 / 6.0, 7));
    out.push(two("avery-8163", "8163 Shipping labels", 2.0, 0.5, 5));
    out.push(two("avery-8164", "8164 Shipping labels", 10.0 / 3.0, 0.5, 3));
    let eu = "Avery A4 and A5";
    // Three across, 63.5 mm wide with 2.5 mm between; two across, 99.1 mm wide.
    let three = |id: &str, name: &str, h: f64, down: u32| {
        let top = (297.0 - h * down as f64) / 2.0;
        spec(id, eu, name, A4, mm(top), mm(7.2), mm(63.5), mm(h), mm(66.0), mm(h), 3, down)
    };
    let twoa = |id: &str, name: &str, h: f64, down: u32| {
        let top = (297.0 - h * down as f64) / 2.0;
        spec(id, eu, name, A4, mm(top), mm(4.65), mm(99.1), mm(h), mm(101.6), mm(h), 2, down)
    };
    out.push(three("avery-l7160", "L7160 Address labels", 38.1, 7));
    out.push(three("avery-l7161", "L7161 Address labels", 46.6, 6));
    out.push(twoa("avery-l7162", "L7162 Address labels", 33.9, 8));
    out.push(twoa("avery-l7163", "L7163 Address labels", 38.1, 7));
    out.push(twoa("avery-l7173", "L7173 Shipping labels", 57.0, 5));
    out.push(three("avery-j8160", "J8160 Address labels", 38.1, 7));
    out.push(three("avery-l7159", "L7159 Address labels", 33.9, 8));
    out.push(three("avery-l7164", "L7164 Shipping labels", 72.0, 4));
    out.push(twoa("avery-l7165", "L7165 Shipping labels", 67.7, 4));
    out.push(twoa("avery-l7166", "L7166 Shipping labels", 93.1, 3));
    out.push(spec("avery-l7651", eu, "L7651 Small labels", A4, mm(10.7), mm(4.7), mm(38.1), mm(21.2), mm(40.6), mm(21.2), 5, 13));
    out.push(spec("avery-l7167", eu, "L7167 Large label", A4, mm(3.95), mm(5.2), mm(199.6), mm(289.1), mm(199.6), mm(289.1), 1, 1));
    out.push(spec("letter-full", "Plain paper", "Letter, one label a page", LETTER, inch(0.5), inch(0.5), inch(7.5), inch(10.0), inch(7.5), inch(10.0), 1, 1));
    out.push(spec("a4-full", "Plain paper", "A4, one label a page", A4, mm(12.7), mm(12.7), mm(184.6), mm(271.6), mm(184.6), mm(271.6), 1, 1));
    out
}

/// What is wrong with a label sheet's numbers, in words, or nothing.
pub fn check(s: &LabelSpec) -> Result<(), String> {
    if s.across == 0 || s.down == 0 || s.across > 20 || s.down > 60 {
        return Err("A sheet needs between 1 and 20 labels across and between 1 and 60 down.".into());
    }
    if s.label_w <= 0 || s.label_h <= 0 || s.page_w <= 0 || s.page_h <= 0 {
        return Err("The page and the labels need a width and a height.".into());
    }
    if s.pitch_h < s.label_w || s.pitch_v < s.label_h {
        return Err("The labels can't overlap: the pitch is at least as big as the label.".into());
    }
    let wide = s.left + (s.across as i64 - 1) * s.pitch_h + s.label_w;
    let high = s.top + (s.down as i64 - 1) * s.pitch_v + s.label_h;
    if wide > s.page_w + 20 || high > s.page_h + 20 || s.left < 0 || s.top < 0 {
        return Err("The labels don't fit on the page.".into());
    }
    Ok(())
}

/// Where the cells are: the labels' columns and the gaps between them.
pub fn grid(s: &LabelSpec) -> Vec<i64> {
    let gap = s.pitch_h - s.label_w;
    let mut cols = Vec::new();
    for i in 0..s.across {
        if i > 0 && gap > 0 {
            cols.push(gap);
        }
        cols.push(s.label_w);
    }
    cols
}

/// The section properties for the sheet.
pub fn section(s: &LabelSpec) -> Element {
    let right = (s.page_w - s.left - (s.across as i64 - 1) * s.pitch_h - s.label_w).max(0);
    // A little room under the last row for the paragraph Word needs after a table, so the sheet stays one page.
    let bottom = (s.page_h - s.top - s.down as i64 * s.pitch_v - 60).max(0);
    Element::new("w:sectPr")
        .with_child(Element::new("w:pgSz").with_attr("w:w", s.page_w.to_string()).with_attr("w:h", s.page_h.to_string()))
        .with_child(
            Element::new("w:pgMar")
                .with_attr("w:top", s.top.to_string())
                .with_attr("w:right", right.to_string())
                .with_attr("w:bottom", bottom.to_string())
                .with_attr("w:left", s.left.to_string())
                .with_attr("w:header", "0")
                .with_attr("w:footer", "0")
                .with_attr("w:gutter", "0"),
        )
        .with_child(Element::new("w:cols").with_attr("w:space", "720"))
}

fn cell(width: i64, paras: Vec<Element>) -> Element {
    let mut c = Element::new("w:tc").with_child(Element::new("w:tcPr").with_child(Element::new("w:tcW").with_attr("w:w", width.to_string()).with_attr("w:type", "dxa")));
    for p in paras {
        c.children.push(Node::Element(p));
    }
    c
}

/// The runs of a Next Record field, which a merge puts in front of the text of every label but the first.
pub fn next_record_runs() -> Vec<Element> {
    let run = |inner: Element| Element::new("w:r").with_child(inner);
    vec![
        run(Element::new("w:fldChar").with_attr("w:fldCharType", "begin")),
        run(Element::new("w:instrText").with_attr("xml:space", "preserve").with_text(" NEXT ")),
        run(Element::new("w:fldChar").with_attr("w:fldCharType", "separate")),
        Element::new("w:r").with_child(Element::new("w:rPr").with_child(Element::new("w:noProof"))).with_child(Element::new("w:t").with_text("«Next_Record»")),
        run(Element::new("w:fldChar").with_attr("w:fldCharType", "end")),
    ]
}

/// The table for a sheet. `fill` says what each label holds.
pub fn table(s: &LabelSpec, fill: &LabelFill, look: &TextLook) -> Element {
    let cols = grid(s);
    let mut tbl = Element::new("w:tbl");
    let mut pr = Element::new("w:tblPr")
        .with_child(Element::new("w:tblW").with_attr("w:w", cols.iter().sum::<i64>().to_string()).with_attr("w:type", "dxa"))
        .with_child(Element::new("w:tblInd").with_attr("w:w", "0").with_attr("w:type", "dxa"))
        .with_child(Element::new("w:tblLayout").with_attr("w:type", "fixed"))
        .with_child(
            Element::new("w:tblCellMar")
                .with_child(Element::new("w:left").with_attr("w:w", "115").with_attr("w:type", "dxa"))
                .with_child(Element::new("w:right").with_attr("w:w", "115").with_attr("w:type", "dxa")),
        );
    put_ordered(&mut pr, Element::new("w:tblLook").with_attr("w:val", "0000"), &["w:tblStyle", "w:tblpPr", "w:tblOverlap", "w:bidiVisual", "w:tblStyleRowBandSize", "w:tblStyleColBandSize", "w:tblW", "w:jc", "w:tblCellSpacing", "w:tblInd", "w:tblBorders", "w:shd", "w:tblLayout", "w:tblCellMar", "w:tblLook"]);
    tbl.children.push(Node::Element(pr));
    let mut g = Element::new("w:tblGrid");
    for w in &cols {
        g.children.push(Node::Element(Element::new("w:gridCol").with_attr("w:w", w.to_string())));
    }
    tbl.children.push(Node::Element(g));
    let gap_col = |i: usize| s.pitch_h > s.label_w && i % 2 == 1;
    let mut label_no = 0usize;
    for row in 0..s.down {
        let mut tr = Element::new("w:tr").with_child(
            Element::new("w:trPr")
                .with_child(Element::new("w:cantSplit"))
                .with_child(Element::new("w:trHeight").with_attr("w:val", s.pitch_v.to_string()).with_attr("w:hRule", "exact")),
        );
        for (i, w) in cols.iter().enumerate() {
            if gap_col(i) {
                tr.children.push(Node::Element(cell(*w, vec![text_paragraph("", &TextLook::default(), None, None, Some(20))])));
                continue;
            }
            let col = if s.pitch_h > s.label_w { i / 2 } else { i } as u32;
            let first = label_no == 0;
            label_no += 1;
            let lines: Vec<String> = match fill {
                LabelFill::Same { lines } => lines.clone(),
                LabelFill::Single { lines, row: r, col: c } if *r == row + 1 && *c == col + 1 => lines.clone(),
                _ => Vec::new(),
            };
            let mut paras: Vec<Element> = lines.iter().map(|l| text_paragraph(l, look, None, None, None)).collect();
            if paras.is_empty() {
                paras.push(text_paragraph("", look, None, None, None));
            }
            if matches!(fill, LabelFill::Merge) && !first {
                // Next Record goes at the start of the first paragraph, after its properties.
                let mut first_p = paras.remove(0);
                for (k, r) in next_record_runs().into_iter().enumerate() {
                    first_p.children.insert(1 + k, Node::Element(r));
                }
                paras.insert(0, first_p);
            }
            tr.children.push(Node::Element(cell(*w, paras)));
        }
        tbl.children.push(Node::Element(tr));
    }
    tbl
}

/// A paragraph so small it fits on a full page, which Word needs after a table.
pub fn closing_paragraph() -> Element {
    text_paragraph("", &TextLook { size_pt: Some(1.0), ..Default::default() }, None, None, Some(20))
}

/// The label cells of a table (not the gaps), in reading order, as (row, column, width).
pub fn label_cells(t: &Table) -> Vec<(usize, usize)> {
    let widths: Vec<i64> = t
        .rows()
        .next()
        .map(|r| r.cells().map(|c| c.tcpr.as_ref().and_then(|p| p.child("w:tcW")).and_then(|w| w.attr("w:w")).and_then(|v| v.parse().ok()).unwrap_or(0)).collect())
        .unwrap_or_default();
    let widest = widths.iter().copied().max().unwrap_or(0);
    let mut out = Vec::new();
    for (r, _) in t.rows().enumerate() {
        for (c, w) in widths.iter().enumerate() {
            // The gaps are far narrower than the labels.
            if *w * 2 >= widest {
                out.push((r, c));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_sound() {
        let all = products();
        assert!(all.len() >= 30);
        let mut ids: Vec<&str> = all.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        for p in &all {
            assert!(check(p).is_ok(), "{} should fit on its page: {:?}", p.id, check(p));
            // Margins left over are the same on both sides for the sheets Avery centres.
            let right = p.page_w - p.left - (p.across as i64 - 1) * p.pitch_h - p.label_w;
            assert!((right - p.left).abs() <= 30, "{}: left {} right {}", p.id, p.left, right);
        }
        let g = |id: &str| all.iter().find(|p| p.id == id).unwrap().clone();
        let a = g("avery-5160");
        assert_eq!((a.across * a.down, a.label_w, a.label_h, a.pitch_h, a.top, a.left), (30, 3780, 1440, 3960, 720, 270));
        assert_eq!(grid(&a), [3780, 180, 3780, 180, 3780]);
        let b = g("avery-l7160");
        assert_eq!((b.across * b.down, b.page_w, b.page_h), (21, 11906, 16838));
        assert_eq!(grid(&g("letter-full")), [10800]);
        assert!(all.iter().any(|p| p.id == "avery-j8160") && all.iter().any(|p| p.id == "avery-5167"));
    }

    #[test]
    fn section_margins_put_the_labels_on_the_sheet() {
        let a = products().into_iter().find(|p| p.id == "avery-5160").unwrap();
        let s = section(&a);
        let m = s.child("w:pgMar").unwrap();
        assert_eq!((m.attr("w:top"), m.attr("w:left"), m.attr("w:right"), m.attr("w:bottom")), (Some("720"), Some("270"), Some("270"), Some("660")));
        let mut bad = a.clone();
        bad.pitch_h = 100;
        assert!(check(&bad).is_err());
        bad = a.clone();
        bad.down = 11;
        assert!(check(&bad).is_err());
    }

    #[test]
    fn the_table_has_a_cell_for_every_label() {
        let a = products().into_iter().find(|p| p.id == "avery-5160").unwrap();
        let t = table(&a, &LabelFill::Same { lines: vec!["Ada".into(), "1 Crumb Rd".into()] }, &TextLook::default());
        assert_eq!(t.children_named("w:tr").count(), 10);
        let row = t.children_named("w:tr").next().unwrap();
        assert_eq!(row.children_named("w:tc").count(), 5);
        let h = row.child("w:trPr").unwrap().child("w:trHeight").unwrap();
        assert_eq!((h.attr("w:val"), h.attr("w:hRule")), (Some("1440"), Some("exact")));
        let ids = &mut IdGen(0);
        let Block::Table(model) = parse_block(&t, ids) else { panic!() };
        // 30 labels, and the gaps are not among them.
        assert_eq!(label_cells(&model).len(), 30);
        // One label.
        let one = table(&a, &LabelFill::Single { lines: vec!["Only".into()], row: 2, col: 3 }, &TextLook::default());
        let texts: Vec<String> = one.children_named("w:tr").flat_map(|r| r.children_named("w:tc")).map(|c| c.text()).collect();
        assert_eq!(texts.iter().filter(|t| !t.is_empty()).count(), 1);
        assert_eq!(one.children_named("w:tr").nth(1).unwrap().children_named("w:tc").nth(4).unwrap().text(), "Only");
    }
}
