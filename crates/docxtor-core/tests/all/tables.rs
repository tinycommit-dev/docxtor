//! Table styles (conditional formatting, `tblLook`, `cnfStyle`), table details (floating tables, cell margins,
//! cell spacing, short rows, old-style merges, row heights, diagonals), content controls around rows and cells,
//! and the table design gallery.

use docxtor_core::ai::call_tool;
use docxtor_core::model::walk_paragraphs;
use docxtor_core::render::{RBlock, RTable};
use docxtor_core::shapes::theme::ShapeTheme;
use docxtor_core::{package::Package, table_designs, Document};
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

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// A document whose body is the given XML and whose style sheet also holds `styles`.
fn docx(body: &str, styles: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    if !styles.is_empty() {
        let old = String::from_utf8(pkg.get("word/styles.xml").unwrap().to_vec()).unwrap();
        let at = old.rfind("</w:styles>").unwrap();
        pkg.set("word/styles.xml", format!("{}{}{}", &old[..at], styles, &old[at..]).into_bytes());
    }
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

fn tables(doc: &mut Document) -> Vec<RTable> {
    doc.render_all().blocks.into_iter().filter_map(|b| if let RBlock::Table(t) = b { Some(t) } else { None }).collect()
}

fn grid(rows: usize, cols: usize, tblpr: &str, trpr: &str) -> String {
    let mut s = format!(r#"<w:tbl><w:tblPr>{tblpr}</w:tblPr><w:tblGrid>"#);
    for _ in 0..cols {
        s.push_str(r#"<w:gridCol w:w="2000"/>"#);
    }
    s.push_str("</w:tblGrid>");
    for r in 0..rows {
        s.push_str(&format!("<w:tr>{trpr}"));
        for c in 0..cols {
            s.push_str(&format!(r#"<w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>r{r}c{c}</w:t></w:r></w:p></w:tc>"#));
        }
        s.push_str("</w:tr>");
    }
    s.push_str("</w:tbl><w:p/>");
    s
}

fn theme() -> ShapeTheme {
    ShapeTheme::default().with_scheme(&[("accent1", "4472C4"), ("accent5", "5B9BD5")])
}

fn style(id: &str) -> String {
    table_designs::style_xml(id, &theme()).unwrap_or_else(|| panic!("no design {id}"))
}

fn fill(t: &RTable, r: usize, c: usize) -> Option<&str> {
    t.rows[r].cells[c].shading.as_deref()
}

// ---------------------------------------------------------------------------
// Table styles
// ---------------------------------------------------------------------------

#[test]
fn grid_table_4_accent_1_shows_its_header_bands_and_first_column() {
    let look = r#"<w:tblStyle w:val="GridTable4-Accent1"/><w:tblW w:w="0" w:type="auto"/><w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#;
    let mut doc = docx(&grid(5, 3, look, ""), &format!("{}{}", table_designs_normal(), style("GridTable4-Accent1")));
    let t = &tables(&mut doc)[0];
    // The header row is the accent colour and the first column cell above it too.
    assert_eq!(fill(t, 0, 0), Some("4472C4"));
    assert_eq!(fill(t, 0, 2), Some("4472C4"));
    // The body: the first banded row is tinted, the next is plain, the next tinted again.
    assert_eq!(fill(t, 1, 1), Some("D9E2F3"));
    assert_eq!(fill(t, 2, 1), None);
    assert_eq!(fill(t, 3, 1), Some("D9E2F3"));
    assert_eq!(fill(t, 4, 1), None);
    // The first column of a banded row keeps the band too.
    assert_eq!(fill(t, 1, 0), Some("D9E2F3"));
    // Light blue lines everywhere in the body.
    let b = &t.rows[2].cells[1].borders;
    for side in [&b.top, &b.bottom, &b.left, &b.right] {
        let side = side.as_ref().unwrap();
        assert_eq!(side.color, "8EAADB");
        assert_eq!(side.sz, 4);
    }
    // Text: the header is bold and white, the first column bold, the rest plain. Checked through the rendered runs.
    let RBlock::Table(rt) = doc.render_all().blocks.remove(0) else { panic!() };
    let first_run = |r: usize, c: usize| match &rt.rows[r].cells[c].blocks[0] {
        RBlock::Para(p) => p.segs.iter().find(|s| s.k == "t").map(|s| s.f.clone()).unwrap_or_default(),
        _ => panic!(),
    };
    assert_eq!(first_run(0, 1).bold, Some(true));
    assert_eq!(first_run(0, 1).color.as_deref(), Some("FFFFFF"));
    assert_eq!(first_run(2, 0).bold, Some(true));
    assert_eq!(first_run(2, 1).bold, None);
}

fn table_designs_normal() -> String {
    r#"<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:unhideWhenUsed/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>"#.into()
}

#[test]
fn the_look_decides_what_is_drawn() {
    let styles = format!("{}{}", table_designs_normal(), style("GridTable4-Accent1"));
    let body = |look: &str| grid(4, 3, &format!(r#"<w:tblStyle w:val="GridTable4-Accent1"/>{look}"#), "");
    let mut doc = docx(&body(r#"<w:tblLook w:val="0060"/>"#), &styles);
    let t = &tables(&mut doc)[0];
    // Header row on: the accent colour. Total row on: bold with a double line above, no fill of its own.
    assert_eq!(fill(t, 0, 1), Some("4472C4"));
    let total_top = t.rows[3].cells[1].borders.top.as_ref().unwrap();
    assert_eq!(total_top.val, "double");
    assert_eq!(t.look.as_ref().map(|l| (l.first_row, l.last_row, l.first_column, l.banded_rows, l.banded_columns)), Some((true, true, false, true, true)));
    // Header row and first column off, banded rows on: the first row is a band.
    let mut doc = docx(&body(r#"<w:tblLook w:val="0400" w:firstRow="0" w:firstColumn="0" w:noHBand="0" w:noVBand="1"/>"#), &styles);
    let t = &tables(&mut doc)[0];
    assert_eq!(fill(t, 0, 1), Some("D9E2F3"));
    assert_eq!(fill(t, 1, 1), None);
    // No look at all: the style's whole-table formatting only.
    let mut doc = docx(&body(""), &styles);
    let t = &tables(&mut doc)[0];
    assert!(t.look.is_none());
    assert_eq!(fill(t, 0, 1), None);
    assert_eq!(fill(t, 1, 1), None);
}

#[test]
fn cnf_style_is_used_when_there_is_no_look() {
    let styles = format!("{}{}", table_designs_normal(), style("GridTable4-Accent1"));
    let tbl = r#"<w:tbl><w:tblPr><w:tblStyle w:val="GridTable4-Accent1"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:trPr><w:cnfStyle w:val="100000000000" w:firstRow="1"/></w:trPr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Head</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Body</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#;
    let mut doc = docx(tbl, &styles);
    let t = &tables(&mut doc)[0];
    assert_eq!(fill(t, 0, 0), Some("4472C4"));
    assert_eq!(fill(t, 1, 0), None);
}

#[test]
fn direct_formatting_wins_over_the_style() {
    let styles = format!("{}{}", table_designs_normal(), style("GridTable4-Accent1"));
    let tbl = r#"<w:tbl><w:tblPr><w:tblStyle w:val="GridTable4-Accent1"/><w:tblLook w:val="04A0"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/><w:tcBorders><w:bottom w:val="nil"/></w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="FFFF00"/></w:tcPr><w:p><w:r><w:t>Head</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#;
    let mut doc = docx(tbl, &styles);
    let t = &tables(&mut doc)[0];
    assert_eq!(fill(t, 0, 0), Some("FFFF00"));
    assert_eq!(t.rows[0].cells[0].borders.bottom.as_ref().unwrap().val, "nil");
}

#[test]
fn list_table_3_plain_table_1_and_grid_table_5_draw_their_designs() {
    let styles = format!("{}{}{}{}", table_designs_normal(), style("ListTable3"), style("PlainTable1"), style("GridTable5Dark"));
    let look = r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#;
    let body = format!(
        "{}{}{}",
        grid(4, 3, &format!(r#"<w:tblStyle w:val="ListTable3"/>{look}"#), ""),
        grid(4, 3, &format!(r#"<w:tblStyle w:val="PlainTable1"/>{look}"#), ""),
        grid(4, 3, &format!(r#"<w:tblStyle w:val="GridTable5Dark"/>{look}"#), "")
    );
    let mut doc = docx(&body, &styles);
    let ts = tables(&mut doc);
    // List Table 3: a black header with an outline around the table.
    assert_eq!(fill(&ts[0], 0, 1), Some("000000"));
    assert_eq!(ts[0].rows[2].cells[0].borders.left.as_ref().unwrap().color, "000000");
    // Plain Table 1: grey bands (F2F2F2) and a line under the header.
    assert_eq!(fill(&ts[1], 1, 1), Some("F2F2F2"));
    assert_eq!(fill(&ts[1], 2, 1), None);
    assert_eq!(ts[1].rows[0].cells[1].borders.bottom.as_ref().unwrap().color, "BFBFBF");
    // Grid Table 5 Dark: black header and first column, tinted body.
    assert_eq!(fill(&ts[2], 0, 1), Some("000000"));
    assert_eq!(fill(&ts[2], 2, 0), Some("000000"));
    assert_eq!(fill(&ts[2], 1, 1), Some("999999"));
    assert_eq!(fill(&ts[2], 2, 1), Some("CCCCCC"));
}

// ---------------------------------------------------------------------------
// Table details
// ---------------------------------------------------------------------------

#[test]
fn a_floating_table_reports_its_place() {
    let tblpr = r#"<w:tblpPr w:leftFromText="180" w:rightFromText="180" w:topFromText="90" w:bottomFromText="90" w:vertAnchor="text" w:horzAnchor="margin" w:tblpXSpec="right" w:tblpY="144"/><w:tblOverlap w:val="never"/><w:tblW w:w="3000" w:type="dxa"/>"#;
    let mut doc = docx(&grid(2, 2, tblpr, ""), "");
    let f = tables(&mut doc)[0].float.clone().expect("floating");
    assert_eq!((f.horz_anchor.as_str(), f.vert_anchor.as_str()), ("margin", "text"));
    assert_eq!(f.x_align.as_deref(), Some("right"));
    assert_eq!((f.y, f.left, f.right, f.top, f.bottom), (Some(144), 180, 180, 90, 90));
    assert!(f.never_overlap);
    // An ordinary table is not floating.
    let mut doc = docx(&grid(2, 2, "", ""), "");
    assert!(tables(&mut doc)[0].float.is_none());
}

#[test]
fn cell_margins_table_shading_and_cell_spacing() {
    let tblpr = r#"<w:tblW w:w="4000" w:type="dxa"/><w:tblCellSpacing w:w="30" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="EEEEEE"/><w:tblLayout w:type="fixed"/>"#;
    let tbl = grid(2, 2, tblpr, "").replacen(
        r#"<w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr>"#,
        r#"<w:tcPr><w:tcW w:w="2000" w:type="dxa"/><w:tcMar><w:top w:w="40" w:type="dxa"/><w:left w:w="200" w:type="dxa"/></w:tcMar></w:tcPr>"#,
        1,
    );
    let mut doc = docx(&tbl, "");
    let t = &tables(&mut doc)[0];
    assert_eq!(t.spacing, Some(30));
    assert!(t.fixed);
    // The table's shading is each cell's fill.
    assert_eq!(fill(t, 1, 1), Some("EEEEEE"));
    // The cell with margins has them (the other sides come from the table); the others have none of their own.
    assert_eq!(t.rows[0].cells[0].mar, Some([40, 108, 0, 200]));
    assert_eq!(t.rows[0].cells[1].mar, None);
}

#[test]
fn rows_that_start_or_end_short_and_old_style_merges() {
    let tbl = r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/></w:tblGrid>
<w:tr><w:trPr><w:gridBefore w:val="1"/><w:wBefore w:w="1000" w:type="dxa"/><w:gridAfter w:val="1"/><w:wAfter w:w="1000" w:type="dxa"/><w:cantSplit/><w:trHeight w:val="600" w:hRule="exact"/></w:trPr>
<w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>mid1</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>mid2</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/><w:hMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>wide</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/><w:hMerge w:val="continue"/></w:tcPr><w:p/></w:tc>
<w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/><w:tcBorders><w:tl2br w:val="single" w:sz="8" w:color="FF0000"/><w:tr2bl w:val="single" w:sz="4" w:color="0000FF"/></w:tcBorders></w:tcPr><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="1000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>y</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#;
    let mut doc = docx(tbl, "");
    let t = &tables(&mut doc)[0];
    let r0 = &t.rows[0];
    assert_eq!((r0.grid_before, r0.grid_after), (1, 1));
    assert!(r0.cant_split);
    assert_eq!((r0.height, r0.height_rule.as_deref()), (Some(600), Some("exact")));
    let r1 = &t.rows[1];
    // The first cell of a horizontal merge takes in the span; the cell it joined is not in the view.
    assert_eq!(r1.cells.len(), 3);
    assert_eq!(r1.cells[0].span, 2);
    assert_eq!(r1.cells[1].diag_down.as_ref().map(|b| (b.sz, b.color.as_str())), Some((8, "FF0000")));
    assert_eq!(r1.cells[1].diag_up.as_ref().map(|b| b.color.as_str()), Some("0000FF"));
    assert!(r1.cells[2].diag_down.is_none());
}

// ---------------------------------------------------------------------------
// Content controls around rows and cells
// ---------------------------------------------------------------------------

const SDT_ROW_TABLE: &str = r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Qty</w:t></w:r></w:p></w:tc></w:tr>
<w:sdt><w:sdtPr><w:alias w:val="Items"/><w:id w:val="101"/><w15:repeatingSection xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml"/></w:sdtPr><w:sdtContent>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Apples</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>3</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Pears</w:t></w:r></w:p></w:tc><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc></w:tr>
</w:sdtContent></w:sdt>
<w:tr><w:sdt><w:sdtPr><w:alias w:val="Total"/><w:id w:val="202"/></w:sdtPr><w:sdtContent><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Sum</w:t></w:r></w:p></w:tc></w:sdtContent></w:sdt><w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>8</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#;

fn body_xml(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    let pkg = Package::open(&bytes).unwrap();
    String::from_utf8(pkg.get("word/document.xml").unwrap().to_vec()).unwrap()
}

fn texts(t: &RTable) -> Vec<Vec<String>> {
    t.rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| {
                    c.blocks
                        .iter()
                        .filter_map(|b| if let RBlock::Para(p) = b { Some(p.segs.iter().filter_map(|s| s.x.clone()).collect::<String>()) } else { None })
                        .collect::<Vec<_>>()
                        .join("")
                })
                .collect()
        })
        .collect()
}

#[test]
fn rows_and_cells_in_content_controls_are_shown() {
    let mut doc = docx(SDT_ROW_TABLE, "");
    let t = &tables(&mut doc)[0];
    assert_eq!(texts(t), [["Name", "Qty"], ["Apples", "3"], ["Pears", "5"], ["Sum", "8"]]);
    assert_eq!(t.rows[0].sdt, None);
    assert_eq!(t.rows[1].sdt.as_deref(), Some("Items"));
    assert_eq!(t.rows[2].sdt.as_deref(), Some("Items"));
    assert_eq!(t.rows[3].cells[0].sdt.as_deref(), Some("Total"));
    assert_eq!(t.rows[3].cells[1].sdt, None);
}

#[test]
fn content_controls_around_rows_and_cells_are_kept_on_save() {
    let mut doc = docx(SDT_ROW_TABLE, "");
    let before = body_xml(&mut doc);
    assert!(before.contains("<w:sdt><w:sdtPr><w:alias w:val=\"Items\"/>"), "{before}");
    // Typing in a wrapped row changes its text and leaves both controls where they were.
    let p = {
        let mut id = None;
        walk_paragraphs(&doc.body, &mut |p| {
            if p.plain_text() == "Pears" {
                id = Some(p.id);
            }
        });
        id.unwrap()
    };
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":5},"text":" and plums"}));
    let after = body_xml(&mut doc);
    assert!(after.contains("Pears and plums"));
    assert_eq!(after.matches("<w:sdt>").count(), before.matches("<w:sdt>").count());
    let flat = |x: &str| x.replace(['\r', '\n'], "");
    assert_eq!(flat(&after.replace("Pears and plums", "Pears")), flat(&before), "only the typed text differs");
    // A new row below a row in the control joins the control.
    ops(&mut doc, json!({"op":"table_edit","at":p,"action":"row_below"}));
    let t = &tables(&mut doc)[0];
    assert_eq!(t.rows.len(), 5);
    assert_eq!(t.rows[3].sdt.as_deref(), Some("Items"));
    let saved = body_xml(&mut doc);
    assert_eq!(saved.matches("<w:alias w:val=\"Items\"/>").count(), 1, "one control holds the three rows");
    assert_valid(&doc.save().unwrap());
    // And undo takes it back.
    assert!(doc.undo());
    assert_eq!(tables(&mut doc)[0].rows.len(), 4);
}

#[test]
fn a_table_with_controls_is_written_back_as_it_was_when_the_body_is_rewritten() {
    let mut doc = docx(SDT_ROW_TABLE, "");
    // Typing elsewhere makes the body be written from the model.
    let last = *doc.render_all().order.last().unwrap();
    ops(&mut doc, json!({"op":"insert_text","at":{"p":last,"o":0},"text":"after"}));
    let xml = body_xml(&mut doc);
    let flat = |x: &str| x.replace(['\r', '\n'], "");
    let table = |x: &str| x[x.find("<w:tbl>").unwrap()..x.find("</w:tbl>").unwrap()].to_string();
    assert_eq!(table(&flat(&xml)), table(&flat(SDT_ROW_TABLE)));
    assert!(xml.contains("<w:sdtContent><w:tr>"));
}

const MARKED_TABLE: &str = r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:sdt><w:sdtPr><w:id w:val="9"/></w:sdtPr><w:sdtContent><w:bookmarkStart w:id="0" w:name="b"/><w:tr><w:tc><w:p><w:r><w:t>kept</w:t></w:r></w:p></w:tc><w:sdt><w:sdtPr><w:id w:val="10"/></w:sdtPr><w:sdtContent><w:permStart w:id="5" w:edGrp="everyone"/><w:tc><w:p><w:r><w:t>one</w:t></w:r></w:p></w:tc><w:permEnd w:id="5"/></w:sdtContent></w:sdt><w:tc><w:p><w:r><w:t>two</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>next</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>row</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>here</w:t></w:r></w:p></w:tc></w:tr><w:bookmarkEnd w:id="0"/></w:sdtContent></w:sdt></w:tbl><w:p/>"#;

#[test]
fn a_control_with_range_markers_around_rows_and_cells_is_still_unwrapped() {
    let mut doc = docx(MARKED_TABLE, "");
    let t = &tables(&mut doc)[0];
    // Both rows show, and the cells after the wrapped one sit in their own columns.
    assert_eq!(texts(t), [["kept", "one", "two"], ["next", "row", "here"]]);
}

#[test]
fn a_column_inserted_between_cells_of_one_control_stays_in_that_control() {
    let tbl = r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:sdt><w:sdtPr><w:id w:val="7"/><w:tag w:val="pair"/></w:sdtPr><w:sdtContent><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:sdtContent></w:sdt></w:tr></w:tbl><w:p/>"#;
    let mut doc = docx(tbl, "");
    let b = {
        let mut id = None;
        walk_paragraphs(&doc.body, &mut |p| {
            if p.plain_text() == "B" {
                id = Some(p.id);
            }
        });
        id.unwrap()
    };
    ops(&mut doc, json!({"op":"table_edit","at":b,"action":"col_left"}));
    assert_eq!(texts(&tables(&mut doc)[0])[0].len(), 3);
    let xml = body_xml(&mut doc);
    assert_eq!(xml.matches("<w:tag w:val=\"pair\"/>").count(), 1, "the control is not split in two: {xml}");
    assert_valid(&doc.save().unwrap());
    // Added at the end of the row, next to only one cell of the control, the new cell stays outside it.
    ops(&mut doc, json!({"op":"table_edit","at":b,"action":"col_right"}));
    let xml = body_xml(&mut doc);
    assert_eq!(xml.matches("<w:tag w:val=\"pair\"/>").count(), 1, "{xml}");
    assert!(xml.contains("</w:sdt><w:tc>"), "{xml}");
}

#[test]
fn range_markers_stay_inside_their_control_when_the_table_is_written_again() {
    let mut doc = docx(MARKED_TABLE, "");
    // Typing in the table makes the body be written from the model.
    let p = {
        let mut id = None;
        walk_paragraphs(&doc.body, &mut |p| {
            if p.plain_text() == "row" {
                id = Some(p.id);
            }
        });
        id.unwrap()
    };
    ops(&mut doc, json!({"op":"insert_text","at":{"p":p,"o":3},"text":"s"}));
    let xml = body_xml(&mut doc);
    let flat = |x: &str| x.replace(['\r', '\n'], "");
    let table = |x: &str| x[x.find("<w:tbl>").unwrap()..x.find("</w:tbl>").unwrap()].to_string();
    assert_eq!(table(&flat(&xml)), table(&flat(MARKED_TABLE)).replace(">row<", ">rows<"));
    assert_valid(&doc.save().unwrap());
}

// ---------------------------------------------------------------------------
// The design gallery
// ---------------------------------------------------------------------------

fn first_para_of_table(doc: &Document) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |p| {
        if id.is_none() && p.plain_text().starts_with("r0c0") {
            id = Some(p.id);
        }
    });
    id.unwrap()
}

fn styles_xml(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    String::from_utf8(Package::open(&bytes).unwrap().get("word/styles.xml").unwrap().to_vec()).unwrap()
}

#[test]
fn picking_a_design_adds_its_definition_and_sets_the_table() {
    let mut doc = docx(&grid(4, 3, r#"<w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/>"#, ""), "");
    let p = first_para_of_table(&doc);
    assert!(!styles_xml(&mut doc).contains("GridTable4-Accent1"));
    ops(&mut doc, json!({"op":"table_design","at":p,"style":"Grid Table 4 Accent 1"}));
    let styles = styles_xml(&mut doc);
    assert!(styles.contains(r#"w:styleId="GridTable4-Accent1""#));
    assert!(styles.contains(r#"<w:name w:val="Grid Table 4 Accent 1"/>"#));
    // It is based on the Normal Table style, which the document has.
    assert!(styles.contains(r#"<w:basedOn w:val="TableNormal"/>"#));
    let body = body_xml(&mut doc);
    assert!(body.contains(r#"<w:tblStyle w:val="GridTable4-Accent1"/>"#));
    // A new table gets Word's default look: header row, first column, banded rows.
    assert!(body.contains(r#"<w:tblLook w:val="04A0" w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0" w:noHBand="0" w:noVBand="1"/>"#), "{body}");
    let t = &tables(&mut doc)[0];
    assert_eq!(t.style.as_deref(), Some("GridTable4-Accent1"));
    assert_eq!(t.style_name.as_deref(), Some("Grid Table 4 Accent 1"));
    // The blank document's theme is Office's: its first accent is the header colour.
    assert_eq!(fill(t, 0, 0), Some("4472C4"));
    assert_valid(&doc.save().unwrap());
    // Picking it again adds nothing: the style is written once.
    ops(&mut doc, json!({"op":"table_design","at":p,"style":"GridTable4-Accent1"}));
    assert_eq!(styles_xml(&mut doc).matches(r#"w:styleId="GridTable4-Accent1""#).count(), 1);
    // Undo takes the table back to Table Grid and the style sheet back to how it was.
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(body_xml(&mut doc).contains(r#"<w:tblStyle w:val="TableGrid"/>"#));
    assert!(!styles_xml(&mut doc).contains("GridTable4-Accent1"));
    assert!(doc.redo());
    assert!(styles_xml(&mut doc).contains("GridTable4-Accent1"));
}

#[test]
fn style_options_write_tbl_look() {
    let mut doc = docx(&grid(4, 3, r#"<w:tblStyle w:val="TableGrid"/>"#, ""), "");
    let p = first_para_of_table(&doc);
    ops(&mut doc, json!({"op":"table_design","at":p,"style":"PlainTable4","look":{"lastRow":true,"bandedColumns":true,"firstColumn":false}}));
    let body = body_xml(&mut doc);
    // Header row (default) and total row on, first column off, both bandings on: 0x0020 + 0x0040.
    assert!(body.contains(r#"<w:tblLook w:val="0060" w:firstRow="1" w:lastRow="1" w:firstColumn="0" w:lastColumn="0" w:noHBand="0" w:noVBand="0"/>"#), "{body}");
    // Options alone keep the style.
    ops(&mut doc, json!({"op":"table_design","at":p,"look":{"firstRow":false}}));
    let body = body_xml(&mut doc);
    assert!(body.contains(r#"<w:tblStyle w:val="PlainTable4"/>"#));
    assert!(body.contains(r#"w:val="0040""#));
    let t = &tables(&mut doc)[0];
    assert_eq!(t.look.as_ref().map(|l| l.first_row), Some(false));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn a_design_replaces_the_tables_own_borders() {
    let mut doc = docx(&grid(3, 2, r#"<w:tblStyle w:val="TableGrid"/><w:tblBorders><w:top w:val="nil"/></w:tblBorders>"#, ""), "");
    let p = first_para_of_table(&doc);
    ops(&mut doc, json!({"op":"table_design","at":p,"style":"List Table 3"}));
    assert!(!body_xml(&mut doc).contains("w:tblBorders"));
}

#[test]
fn an_unknown_design_is_an_error_and_changes_nothing() {
    let mut doc = docx(&grid(2, 2, r#"<w:tblStyle w:val="TableGrid"/>"#, ""), "");
    let p = first_para_of_table(&doc);
    let err = doc.apply_json(&json!({"op":"table_design","at":p,"style":"Sparkly Table 9"}).to_string(), false).unwrap_err();
    assert!(err.to_string().contains("Sparkly Table 9"));
    // Outside a table too.
    let mut blank = Document::new_blank();
    let first = blank.render_all().order[0];
    assert!(blank.apply_json(&json!({"op":"table_design","at":first,"style":"Plain Table 1"}).to_string(), false).is_err());
}

const ACME: &str = r#"<w:style w:type="table" w:customStyle="1" w:styleId="AcmeTable"><w:name w:val="Acme table"/><w:basedOn w:val="TableNormal"/><w:tblPr><w:tblBorders><w:bottom w:val="single" w:sz="8" w:space="0" w:color="C00000"/></w:tblBorders></w:tblPr><w:tblStylePr w:type="firstRow"><w:rPr><w:b/><w:color w:val="C00000"/></w:rPr><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="FCE4D6"/></w:tcPr></w:tblStylePr></w:style>"#;

#[test]
fn a_design_an_extension_brings_is_added_to_the_document_and_drawn() {
    let mut doc = docx(&grid(3, 2, r#"<w:tblStyle w:val="TableGrid"/>"#, ""), "");
    let p = first_para_of_table(&doc);
    // Drawn small before it is used.
    let look = docxtor_core::table_style::TableLook::default();
    let preview = doc.table_design_preview(ACME, &look).unwrap();
    assert_eq!((preview.id.as_str(), preview.name.as_str(), preview.group), ("AcmeTable", "Acme table", "custom"));
    assert_eq!(preview.cells[0][0].fill.as_deref(), Some("FCE4D6"));
    assert!(preview.cells[0][1].bold);
    ops(&mut doc, json!({"op":"table_design","at":p,"definition":ACME}));
    let styles = styles_xml(&mut doc);
    assert!(styles.contains(r#"w:styleId="AcmeTable""#));
    assert!(styles.contains(r#"w:styleId="TableNormal""#), "the style it is based on comes with it");
    assert_eq!(tables(&mut doc)[0].style.as_deref(), Some("AcmeTable"));
    assert_eq!(fill(&tables(&mut doc)[0], 0, 0), Some("FCE4D6"));
    assert_valid(&doc.save().unwrap());
    // Once the document has the style, its own copy stays.
    ops(&mut doc, json!({"op":"table_design","at":p,"definition":ACME.replace("FCE4D6", "000000")}));
    assert_eq!(fill(&tables(&mut doc)[0], 0, 0), Some("FCE4D6"));
    assert_eq!(styles_xml(&mut doc).matches(r#"w:styleId="AcmeTable""#).count(), 1);
}

#[test]
fn a_design_that_is_not_one_table_style_is_refused() {
    let mut doc = docx(&grid(2, 2, r#"<w:tblStyle w:val="TableGrid"/>"#, ""), "");
    let p = first_para_of_table(&doc);
    let look = docxtor_core::table_style::TableLook::default();
    for bad in [
        "not xml <",
        r#"<w:style w:type="paragraph" w:styleId="X"><w:name w:val="X"/></w:style>"#,
        r#"<w:style w:type="table"><w:name w:val="No id"/></w:style>"#,
        &format!("{ACME}{ACME}"),
        r#"<w:style w:type="table" w:styleId="X"><q:name w:val="X"/></w:style>"#,
    ] {
        assert!(doc.table_design_preview(bad, &look).is_err(), "{bad}");
        assert!(doc.apply_json(&json!({"op":"table_design","at":p,"definition":bad}).to_string(), false).is_err(), "{bad}");
    }
    // Nothing changed.
    assert_eq!(tables(&mut doc)[0].style.as_deref(), Some("TableGrid"));
}

#[test]
fn the_ai_tool_sets_a_design() {
    let mut doc = docx(&grid(3, 2, r#"<w:tblStyle w:val="TableGrid"/>"#, ""), "");
    let p = first_para_of_table(&doc);
    let out = call_tool(&mut doc, "set_table_design", &json!({"cell": format!("p{p}"), "design": "list table 3 accent 2", "total_row": true, "banded_rows": false})).unwrap();
    assert!(out.contains("list table 3 accent 2"), "{out}");
    let t = &tables(&mut doc)[0];
    assert_eq!(t.style.as_deref(), Some("ListTable3-Accent2"));
    assert_eq!(t.look.as_ref().map(|l| (l.last_row, l.banded_rows)), Some((true, false)));
    assert!(call_tool(&mut doc, "set_table_design", &json!({"cell": format!("p{p}")})).is_err());
}

#[test]
fn previews_draw_every_design_with_the_documents_theme() {
    let mut doc = Document::new_blank();
    let previews = doc.table_design_previews(&docxtor_core::table_style::TableLook::default());
    assert_eq!(previews.len(), table_designs::designs().len());
    let gt4 = previews.iter().find(|p| p.id == "GridTable4-Accent2").unwrap();
    assert!(gt4.cells[0][0].bold);
    assert_eq!(gt4.cells[0][0].color, "FFFFFF");
    assert_eq!(gt4.cells.len(), 5);
    assert_eq!(gt4.cells[0].len(), 4);
    let _ = &mut doc;
}
