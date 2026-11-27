//! Table properties (size, alignment, indent, wrapping, autofit, margins, row height, header rows, column and cell
//! widths, alignment, text direction, alt text), borders per cell and per edge, splitting cells and tables, inserting
//! and deleting single cells, sorting rows, converting text to a table and back, and the table formula suggestion.
//! Each edit is checked on the rendering, written, read back and undone.

use docxtor_core::model::walk_paragraphs;
use docxtor_core::render::{RBlock, RTable};
use docxtor_core::{package::Package, Document};
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

macro_rules! ops {
    ($d:expr, $j:expr) => {{
        let j = $j;
        ops(&mut $d, j)
    }};
}

macro_rules! fails {
    ($d:expr, $j:expr) => {{
        let j = $j;
        fails(&mut $d, j)
    }};
}

fn fails(doc: &mut Document, json: Value) -> String {
    doc.apply_json(&json.to_string(), false).err().unwrap_or_else(|| panic!("expected an error: {json}")).to_string()
}

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

fn docx(body: &str) -> Document {
    let mut pkg = Package::open(&Document::new_blank().save().unwrap()).unwrap();
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{W}"><w:body>{body}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    pkg.set("word/document.xml", xml.into_bytes());
    Document::open(&pkg.to_bytes().unwrap()).unwrap()
}

/// A table of text `r0c0`, `r0c1`... with a paragraph after it.
fn grid(rows: usize, cols: usize) -> String {
    let mut s = String::from(r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid>"#);
    for _ in 0..cols {
        s.push_str(r#"<w:gridCol w:w="2000"/>"#);
    }
    s.push_str("</w:tblGrid>");
    for r in 0..rows {
        s.push_str("<w:tr>");
        for c in 0..cols {
            s.push_str(&format!(r#"<w:tc><w:tcPr><w:tcW w:w="2000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>r{r}c{c}</w:t></w:r></w:p></w:tc>"#));
        }
        s.push_str("</w:tr>");
    }
    s.push_str("</w:tbl><w:p><w:r><w:t>after</w:t></w:r></w:p>");
    s
}

fn tables(doc: &mut Document) -> Vec<RTable> {
    doc.render_all().blocks.into_iter().filter_map(|b| if let RBlock::Table(t) = b { Some(t) } else { None }).collect()
}

/// The id of the paragraph with this text.
fn p(doc: &Document, text: &str) -> u32 {
    let mut id = None;
    walk_paragraphs(&doc.body, &mut |q| {
        if id.is_none() && q.plain_text() == text {
            id = Some(q.id);
        }
    });
    id.unwrap_or_else(|| panic!("no paragraph {text}"))
}

fn cell_texts(t: &RTable) -> Vec<Vec<String>> {
    t.rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| c.blocks.iter().filter_map(|b| if let RBlock::Para(p) = b { Some(p.segs.iter().filter_map(|s| s.x.clone()).collect::<String>()) } else { None }).collect::<Vec<_>>().join(" "))
                .collect()
        })
        .collect()
}

fn body_xml(doc: &mut Document) -> String {
    let bytes = doc.save().unwrap();
    String::from_utf8(Package::open(&bytes).unwrap().get("word/document.xml").unwrap().to_vec()).unwrap().replace("\r\n", "")
}

/// Save, check the file, read it back.
fn reopen(doc: &mut Document) -> Document {
    let bytes = doc.save().unwrap();
    assert_valid(&bytes);
    Document::open(&bytes).unwrap()
}

#[test]
fn table_properties_are_written_shown_read_back_and_undone() {
    let mut doc = docx(&grid(2, 2));
    let at = p(&doc, "r0c0");
    let before = body_xml(&mut doc);
    ops!(doc, json!({"op":"table_format","at":at,"width":{"type":"pct","value":5000},"align":"center","indent":0,"autofit":false,"cellMargins":[30,150,30,150],"spacing":20,"title":"Sales","description":"Sales by region"}));
    let t = &tables(&mut doc)[0];
    assert_eq!((t.width, t.width_pct, t.align.as_deref(), t.fixed, t.cell_mar, t.spacing), (Some(5000), true, Some("center"), true, [30, 150, 30, 150], Some(20)));
    let xml = body_xml(&mut doc);
    assert!(xml.contains(r#"<w:tblCaption w:val="Sales"/>"#) && xml.contains(r#"<w:tblDescription w:val="Sales by region"/>"#), "{xml}");
    // What the dialog reads back.
    let d = reopen(&mut doc);
    let info = d.table_details(p(&d, "r0c0")).unwrap();
    assert_eq!((info.table.title.as_str(), info.table.align.as_str(), info.table.autofit, info.table.cell_margins), ("Sales", "center", false, [30, 150, 30, 150]));
    assert_eq!(info.grid, [2000, 2000]);
    assert!(doc.undo());
    assert_eq!(body_xml(&mut doc), before);
    // A bad alignment changes nothing.
    assert!(fails!(doc, json!({"op":"table_format","at":at,"align":"middle"})).contains("alignment"));
    assert_eq!(body_xml(&mut doc), before);
}

#[test]
fn text_wrapping_around_a_table_makes_it_float() {
    let mut doc = docx(&grid(1, 2));
    let at = p(&doc, "r0c0");
    ops!(doc, json!({"op":"table_format","at":at,"wrap":"around","align":"right"}));
    let t = &tables(&mut doc)[0];
    let f = t.float.as_ref().expect("the table floats");
    assert_eq!(f.x_align.as_deref(), Some("right"));
    assert_eq!((f.left, f.right), (180, 180));
    assert!(reopen(&mut doc).table_details(at).unwrap().table.wrap);
    ops!(doc, json!({"op":"table_format","at":at,"wrap":"none"}));
    assert!(tables(&mut doc)[0].float.is_none());
}

#[test]
fn row_height_may_not_break_and_header_rows() {
    let mut doc = docx(&grid(3, 2));
    let (a, b) = (p(&doc, "r0c0"), p(&doc, "r1c1"));
    ops!(doc, json!({"op":"table_rows","from":a,"to":b,"height":{"value":600,"rule":"exact"},"cantSplit":true,"header":true}));
    let t = &tables(&mut doc)[0];
    for r in 0..2 {
        assert_eq!((t.rows[r].height, t.rows[r].height_rule.as_deref(), t.rows[r].cant_split, t.rows[r].header), (Some(600), Some("exact"), true, true));
    }
    assert_eq!((t.rows[2].height, t.rows[2].header), (None, false));
    let d = reopen(&mut doc);
    let info = d.table_details(p(&d, "r1c0")).unwrap();
    assert_eq!((info.rows[1].height, info.rows[1].rule.as_str(), info.rows[1].header, info.row), (600, "exact", true, 1));
    // Rows that repeat must start at the top.
    let c = p(&doc, "r2c0");
    let mut other = docx(&grid(3, 2));
    let o = p(&other, "r2c0");
    assert!(fails!(other, json!({"op":"table_rows","from":o,"header":true})).contains("top of the table"));
    // `all` reaches every row; auto takes the height off.
    ops!(doc, json!({"op":"table_rows","from":c,"all":true,"height":{"rule":"auto"},"header":false}));
    let t = &tables(&mut doc)[0];
    assert!(t.rows.iter().all(|r| r.height.is_none() && !r.header));
    assert!(doc.undo());
    assert_eq!(tables(&mut doc)[0].rows[0].height, Some(600));
}

#[test]
fn column_and_cell_widths_alignment_margins_and_direction() {
    let mut doc = docx(&grid(2, 3));
    let at = p(&doc, "r0c1");
    ops!(doc, json!({"op":"table_column","at":at,"width":{"type":"dxa","value":3000}}));
    assert_eq!(tables(&mut doc)[0].grid, [2000, 3000, 2000]);
    ops!(doc, json!({"op":"table_cells","from":at,"to":at,"action":{"type":"format","valign":"center","margins":[10,20,30,40],"noWrap":true,"textDirection":"btLr","width":{"type":"dxa","value":3000}}}));
    let t = &tables(&mut doc)[0];
    let c = &t.rows[0].cells[1];
    assert_eq!((c.valign.as_deref(), c.mar, c.width), (Some("center"), Some([10, 20, 30, 40]), Some(3000)));
    // Only that cell changed.
    assert!(t.rows[1].cells[1].mar.is_none());
    let d = reopen(&mut doc);
    let info = d.table_details(p(&d, "r0c1")).unwrap();
    assert_eq!((info.cell.valign.as_str(), info.cell.text_direction.as_str(), info.cell.no_wrap, info.col), ("center", "btLr", true, 1));
    // A percentage column leaves the grid alone.
    ops!(doc, json!({"op":"table_column","at":at,"width":{"type":"pct","value":1500}}));
    let info = doc.table_details(at).unwrap();
    assert_eq!(info.column_width, docxtor_core::table_format::Width { kind: docxtor_core::table_format::WidthKind::Pct, value: 1500 });
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(tables(&mut doc)[0].grid, [2000, 3000, 2000]);
    assert!(fails!(doc, json!({"op":"table_cells","from":at,"to":at,"action":{"type":"format","textDirection":"sideways"}})).contains("text direction"));
}

#[test]
fn borders_per_edge_on_a_cell_and_on_the_table() {
    let mut doc = docx(&grid(3, 3));
    let (a, b) = (p(&doc, "r0c0"), p(&doc, "r1c1"));
    // The outside of the top left 2 by 2 block thick red, the line between its cells dotted.
    ops!(doc, json!({"op":"table_cells","from":a,"to":b,"action":{"type":"borders",
        "top":{"val":"single","sz":18,"color":"FF0000"},"bottom":{"val":"single","sz":18,"color":"FF0000"},
        "left":{"val":"single","sz":18,"color":"FF0000"},"right":{"val":"single","sz":18,"color":"FF0000"},
        "insideH":{"val":"dotted","sz":4,"color":"auto"},"insideV":{"val":"dotted","sz":4,"color":"auto"}}}));
    let t = &tables(&mut doc)[0];
    let red = |c: &docxtor_core::render::RCell, side: &str| {
        let b = match side {
            "top" => &c.borders.top,
            "bottom" => &c.borders.bottom,
            "left" => &c.borders.left,
            _ => &c.borders.right,
        };
        b.as_ref().map(|b| (b.val.clone(), b.sz, b.color.clone()))
    };
    let thick = Some(("single".to_string(), 18, "FF0000".to_string()));
    assert_eq!(red(&t.rows[0].cells[0], "top"), thick);
    assert_eq!(red(&t.rows[0].cells[0], "left"), thick);
    assert_eq!(red(&t.rows[1].cells[1], "bottom"), thick);
    assert_eq!(red(&t.rows[1].cells[1], "right"), thick);
    assert_eq!(red(&t.rows[0].cells[0], "bottom").map(|b| b.0), Some("dotted".into()));
    assert_eq!(red(&t.rows[1].cells[0], "top").map(|b| b.0), Some("dotted".into()));
    // The neighbours across the block agree about the shared line.
    assert_eq!(red(&t.rows[2].cells[0], "top"), thick);
    assert_eq!(red(&t.rows[0].cells[2], "left"), thick);
    // A cell outside is untouched.
    assert!(t.rows[2].cells[2].borders.top.is_none());
    let d = reopen(&mut doc);
    let info = d.table_details(p(&d, "r0c0")).unwrap();
    assert_eq!(info.cell_borders.top.as_ref().map(|b| b.sz), Some(18));
    assert!(doc.undo());
    assert!(tables(&mut doc)[0].rows[0].cells[0].borders.top.is_none());

    // The whole table: all lines, and they replace what cells said.
    ops!(doc, json!({"op":"table_cells","from":a,"to":a,"action":{"type":"borders","top":{"val":"single","sz":24,"color":"00FF00"}}}));
    ops!(doc, json!({"op":"table_format","at":a,"borders":{"top":{"val":"single","sz":8,"color":"0000FF"},"bottom":{"val":"single","sz":8,"color":"0000FF"},"left":{"val":"single","sz":8,"color":"0000FF"},"right":{"val":"single","sz":8,"color":"0000FF"},"insideH":{"val":"single","sz":4,"color":"auto"},"insideV":{"val":"none"}}}));
    let t = &tables(&mut doc)[0];
    assert_eq!(t.borders.top.as_ref().map(|b| (b.sz, b.color.clone())), Some((8, "0000FF".into())));
    assert!(t.borders.inside_v.is_none());
    assert!(t.rows[0].cells[0].borders.top.as_ref().is_none_or(|b| b.color != "00FF00"), "the cell's own top line is gone");
    assert_valid(&doc.save().unwrap());
}

#[test]
fn a_border_set_to_none_hides_the_line_and_clear_takes_the_setting_off() {
    let mut doc = docx(&grid(2, 2));
    let a = p(&doc, "r0c0");
    ops!(doc, json!({"op":"table_format","at":a,"borders":{"top":{"val":"single","sz":4,"color":"auto"}}}));
    assert!(tables(&mut doc)[0].rows[0].cells[0].borders.top.is_none() || tables(&mut doc)[0].borders.top.is_some());
    ops!(doc, json!({"op":"table_cells","from":a,"to":a,"action":{"type":"borders","top":{"val":"none"}}}));
    let xml = body_xml(&mut doc);
    assert!(xml.contains(r#"<w:top w:val="nil"/>"#), "{xml}");
    ops!(doc, json!({"op":"table_cells","from":a,"to":a,"action":{"type":"borders","top":{"val":"clear"}}}));
    assert!(!body_xml(&mut doc).contains("<w:tcBorders>"));
}

#[test]
fn a_cell_splits_into_columns_and_rows() {
    let mut doc = docx(&grid(2, 2));
    let a = p(&doc, "r0c0");
    ops!(doc, json!({"op":"table_cells","from":a,"to":a,"action":{"type":"split","cols":2,"rows":2}}));
    let t = &tables(&mut doc)[0];
    // Three rows (the cell's two, and the row below it carries on), three grid columns.
    assert_eq!(t.grid.len(), 3);
    assert_eq!(t.rows.len(), 3);
    assert_eq!(cell_texts(t)[0][0], "r0c0");
    assert_eq!(cell_texts(t)[0].len(), 3);
    let mut d = reopen(&mut doc);
    assert_eq!(tables(&mut d)[0].grid.len(), 3);
    assert!(doc.undo());
    assert_eq!(tables(&mut doc)[0].grid.len(), 2);
    // Asking for nothing different is an error.
    assert!(fails!(doc, json!({"op":"table_cells","from":a,"to":a,"action":{"type":"split","cols":1,"rows":1}})).contains("more than one"));
    // Unmerging with no counts still works.
    ops!(doc, json!({"op":"table_cells","from":a,"to":p(&doc, "r0c1"),"action":{"type":"merge"}}));
    ops!(doc, json!({"op":"table_cells","from":a,"to":a,"action":{"type":"split"}}));
    assert_eq!(tables(&mut doc)[0].rows[0].cells.len(), 2);
}

#[test]
fn a_table_splits_in_two_with_a_paragraph_between() {
    let mut doc = docx(&grid(3, 2));
    let at = p(&doc, "r1c0");
    ops!(doc, json!({"op":"split_table","at":at}));
    let ts = tables(&mut doc);
    assert_eq!(ts.len(), 2);
    assert_eq!((ts[0].rows.len(), ts[1].rows.len()), (1, 2));
    assert_eq!(ts[0].grid, ts[1].grid);
    let order: Vec<&str> = doc.render_all().blocks.iter().map(|b| match b { RBlock::Table(_) => "t", RBlock::Para(_) => "p", _ => "?" }).collect();
    assert_eq!(order, ["t", "p", "t", "p"]);
    assert_valid(&doc.save().unwrap());
    assert!(doc.undo());
    assert_eq!(tables(&mut doc).len(), 1);
    // Before the first row a paragraph goes above the table.
    ops!(doc, json!({"op":"split_table","at":p(&doc, "r0c0")}));
    let order: Vec<&str> = doc.render_all().blocks.iter().map(|b| match b { RBlock::Table(_) => "t", RBlock::Para(_) => "p", _ => "?" }).collect();
    assert_eq!(order, ["p", "t", "p"]);
}

#[test]
fn cells_are_inserted_and_deleted_with_the_others_shifting() {
    let mut doc = docx(&grid(2, 2));
    let at = p(&doc, "r0c1");
    let made = doc.apply_json(&json!({"op":"table_cell","at":at,"action":"insert","shift":"right"}).to_string(), false).unwrap();
    let new_cell = made[0].caret.unwrap().p.0;
    let t = &tables(&mut doc)[0];
    assert_eq!(cell_texts(t)[0], ["r0c0", "", "r0c1"]);
    assert_eq!(t.grid.len(), 3);
    assert_eq!(t.rows[1].grid_after, 1);
    assert_valid(&doc.save().unwrap());
    ops!(doc, json!({"op":"table_cell","at":new_cell,"action":"delete","shift":"left"}));
    assert_eq!(cell_texts(&tables(&mut doc)[0])[0], ["r0c0", "r0c1"]);
    ops!(doc, json!({"op":"table_cell","at":p(&doc, "r0c0"),"action":"insert","shift":"down"}));
    let t = &tables(&mut doc)[0];
    assert_eq!(cell_texts(t)[0], ["", "r0c1"]);
    assert_eq!(cell_texts(t)[1], ["r0c0", "r1c1"]);
    assert_eq!(cell_texts(t)[2], ["r1c0", ""]);
    ops!(doc, json!({"op":"table_cell","at":p(&doc, "r0c1"),"action":"delete","shift":"up"}));
    assert_eq!(cell_texts(&tables(&mut doc)[0])[0], ["", "r1c1"]);
    assert!(doc.undo());
    assert_eq!(cell_texts(&tables(&mut doc)[0])[0], ["", "r0c1"]);
}

#[test]
fn rows_are_sorted_by_a_column_and_the_header_stays() {
    let mut doc = docx(
        r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Qty</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>pear</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>10</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>apple</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>9</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>fig</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#,
    );
    let at = p(&doc, "Name");
    ops!(doc, json!({"op":"sort_table","from":at,"keys":[{"type":"number","field":2,"descending":true}],"header":true}));
    let t = &tables(&mut doc)[0];
    assert_eq!(cell_texts(t).iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), ["Name", "fig", "pear", "apple"]);
    assert_valid(&doc.save().unwrap());
    // Only some rows: sort the last two by name.
    ops!(doc, json!({"op":"sort_table","from":p(&doc, "pear"),"to":p(&doc, "apple"),"keys":[{"field":1}]}));
    assert_eq!(cell_texts(&tables(&mut doc)[0]).iter().map(|r| r[0].as_str()).collect::<Vec<_>>(), ["Name", "fig", "apple", "pear"]);
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(cell_texts(&tables(&mut doc)[0])[1][0], "pear");
}

#[test]
fn text_becomes_a_table_and_a_table_becomes_text() {
    let mut doc = docx(r#"<w:p><w:r><w:t>Name,Qty</w:t></w:r></w:p><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>pear,10</w:t></w:r></w:p><w:p><w:r><w:t>fig,100</w:t></w:r></w:p>"#);
    let ids = vec![p(&doc, "Name,Qty"), p(&doc, "pear,10"), p(&doc, "fig,100")];
    ops!(doc, json!({"op":"convert_to_table","ids":ids,"separator":"commas"}));
    let t = &tables(&mut doc)[0];
    assert_eq!(cell_texts(t), [["Name", "Qty"], ["pear", "10"], ["fig", "100"]]);
    // Paragraph formatting went along into the cells.
    assert!(body_xml(&mut doc).contains(r#"<w:jc w:val="center"/>"#));
    reopen(&mut doc);
    // Back to text with tabs: a paragraph per row.
    ops!(doc, json!({"op":"convert_to_text","at":p(&doc, "Name"),"separator":"tabs"}));
    assert!(tables(&mut doc).is_empty());
    let mut texts = Vec::new();
    walk_paragraphs(&doc.body, &mut |q| texts.push(q.text()));
    assert_eq!(texts[..3], ["Name\tQty", "pear\t10", "fig\t100"]);
    assert_valid(&doc.save().unwrap());
    // And undo brings the table back.
    assert!(doc.undo());
    assert_eq!(tables(&mut doc).len(), 1);
    assert!(doc.undo());
    assert!(tables(&mut doc).is_empty());
}

#[test]
fn paragraphs_fill_the_cells_row_by_row() {
    let mut doc = docx(r#"<w:p><w:r><w:t>a</w:t></w:r></w:p><w:p><w:r><w:t>b</w:t></w:r></w:p><w:p><w:r><w:t>c</w:t></w:r></w:p>"#);
    let ids = vec![p(&doc, "a"), p(&doc, "b"), p(&doc, "c")];
    ops!(doc, json!({"op":"convert_to_table","ids":ids,"separator":"paragraphs","columns":2,"autofit":"window"}));
    let t = &tables(&mut doc)[0];
    assert_eq!(cell_texts(t), [["a", "b"], ["c", ""]]);
    assert_eq!((t.width, t.width_pct), (Some(5000), true));
    assert!(fails!(doc, json!({"op":"convert_to_table","ids":[p(&doc, "a")],"separator":"tabs","autofit":"sideways"})).contains("autofit"));
}

#[test]
fn paragraphs_that_do_not_follow_each_other_are_refused() {
    let mut doc = docx(r#"<w:p><w:r><w:t>a</w:t></w:r></w:p><w:p><w:r><w:t>b</w:t></w:r></w:p><w:p><w:r><w:t>c</w:t></w:r></w:p>"#);
    let ids = vec![p(&doc, "a"), p(&doc, "c")];
    assert!(fails!(doc, json!({"op":"convert_to_table","ids":ids})).contains("follow each other"));
}

#[test]
fn insert_table_sizes_its_columns() {
    let mut doc = docx(r#"<w:p><w:r><w:t>here</w:t></w:r></w:p>"#);
    let at = p(&doc, "here");
    ops!(doc, json!({"op":"insert_table","after":at,"rows":2,"cols":3,"autofit":"fixed","column_width":1500}));
    ops!(doc, json!({"op":"insert_table","after":at,"rows":1,"cols":2,"autofit":"window"}));
    ops!(doc, json!({"op":"insert_table","after":at,"rows":1,"cols":2,"autofit":"contents"}));
    let ts = tables(&mut doc);
    // contents, window, fixed (the last inserted comes first)
    assert_eq!((ts[0].fixed, ts[0].width.unwrap_or(0)), (false, 0));
    assert_eq!((ts[1].fixed, ts[1].width, ts[1].width_pct), (false, Some(5000), true));
    assert_eq!((ts[2].fixed, ts[2].grid.clone()), (true, vec![1500, 1500, 1500]));
    assert_valid(&doc.save().unwrap());
}

#[test]
fn a_table_inside_a_cell_is_a_nested_table() {
    let mut doc = docx(&grid(1, 2));
    let at = p(&doc, "r0c0");
    ops!(doc, json!({"op":"insert_table","after":at,"rows":2,"cols":2}));
    let t = &tables(&mut doc)[0];
    assert!(t.rows[0].cells[0].blocks.iter().any(|b| matches!(b, RBlock::Table(_))));
    assert_valid(&doc.save().unwrap());
    // The dialogs address the inner table from a paragraph in it.
    let mut inner = None;
    walk_paragraphs(&doc.body, &mut |q| {
        if q.plain_text().is_empty() && inner.is_none() {
            inner = Some(q.id);
        }
    });
    let info = doc.table_details(inner.unwrap()).unwrap();
    assert_eq!(info.grid.len(), 2);
    ops!(doc, json!({"op":"table_format","at":inner.unwrap(),"align":"center"}));
    assert_eq!(doc.table_details(inner.unwrap()).unwrap().table.align, "center");
    assert_eq!(doc.table_details(at).unwrap().table.align, "left");
}

#[test]
fn the_suggested_formula_follows_the_numbers_around_the_cell() {
    let mut doc = docx(
        r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid>
<w:tr><w:tc><w:p><w:r><w:t>Item</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Jan</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Feb</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Rent</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>100</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>200</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Food</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>6</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Total</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>x1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>x2</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#,
    );
    // Under numbers: the sum above. Beside numbers and with none above: the sum to the left.
    assert_eq!(doc.suggest_formula(p(&doc, "x1")), "=SUM(ABOVE)");
    assert_eq!(doc.suggest_formula(p(&doc, "Rent")), "=SUM(ABOVE)");
    assert_eq!(doc.suggest_formula(p(&doc, "Jan")), "=SUM(ABOVE)");
    let _ = &mut doc;
    let row = docx(r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>4</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>total</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#);
    assert_eq!(row.suggest_formula(p(&row, "total")), "=SUM(LEFT)");
}

#[test]
fn a_formula_field_inserted_in_a_cell_shows_its_value_at_once() {
    let mut doc = docx(r#"<w:tbl><w:tblPr/><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>10</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>5.5</w:t></w:r></w:p></w:tc><w:tc><w:p/></w:tc></w:tr></w:tbl><w:p/>"#);
    let cell = p(&doc, "");
    ops!(doc, json!({"op":"insert_field","at":{"p":cell,"o":0},"field":"= SUM(LEFT) \\# \"0.00\""}));
    let t = &tables(&mut doc)[0];
    assert_eq!(cell_texts(t)[0][2], "15.50");
    assert_valid(&doc.save().unwrap());
    assert!(doc.undo());
    assert_eq!(cell_texts(&tables(&mut doc)[0])[0][2], "");
}

#[test]
fn table_details_read_the_right_cell_in_a_row_with_old_style_horizontal_merges() {
    // wide (hMerge restart), (hMerge continue), x, y
    let row = |t: &str, tcpr: &str| format!(r#"<w:tc><w:tcPr>{tcpr}</w:tcPr><w:p><w:r><w:t>{t}</w:t></w:r></w:p></w:tc>"#);
    let shd = |c: &str| format!(r#"<w:shd w:val="clear" w:color="auto" w:fill="{c}"/>"#);
    let body = format!(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/><w:gridCol w:w="1000"/></w:tblGrid><w:tr>{}{}{}{}</w:tr></w:tbl><w:p/>"#,
        row("wide", r#"<w:hMerge w:val="restart"/>"#),
        row("gone", r#"<w:hMerge/>"#),
        row("x", &shd("00FF00")),
        row("y", &shd("0000FF")),
    );
    let doc = docx(&body);
    let fill = |t: &str| doc.table_details(p(&doc, t)).unwrap().shading;
    assert_eq!(fill("x").as_deref(), Some("00FF00"));
    assert_eq!(fill("y").as_deref(), Some("0000FF"));
    assert_eq!(fill("wide"), None);
}

/// Three grid columns; the second row starts one column in (`w:gridBefore`) and has two cells.
const SHORT_START: &str = r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/><w:gridCol w:w="2000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>a0</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>a1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>a2</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:trPr><w:gridBefore w:val="1"/></w:trPr><w:tc><w:p><w:r><w:t>b1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b2</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/>"#;

#[test]
fn a_row_that_starts_in_the_grid_gets_its_cell_borders_and_split_in_the_right_place() {
    let mut doc = docx(SHORT_START);
    let b1 = p(&doc, "b1");
    ops!(doc, json!({"op":"table_cells","from":b1,"to":b1,"action":{"type":"borders","left":{"val":"single","sz":12,"color":"FF0000"}}}));
    let t = &tables(&mut doc)[0];
    let left = |r: usize, c: usize| t.rows[r].cells[c].borders.left.as_ref().map(|b| b.sz);
    assert_eq!(left(1, 0), Some(12), "the cell the caret is in gets the line");
    assert_eq!(left(1, 1), None);
    assert_eq!(left(0, 0), None);
    // The same cell splits in two.
    ops!(doc, json!({"op":"table_cells","from":b1,"to":b1,"action":{"type":"split","cols":2,"rows":1}}));
    let t = &tables(&mut doc)[0];
    assert_eq!(cell_texts(t)[1].len(), 3);
    assert_eq!(cell_texts(t)[1][0], "b1");
    assert_valid(&doc.save().unwrap());
}
