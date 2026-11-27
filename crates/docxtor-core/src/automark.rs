//! Word's AutoMark file: a document with one table of two columns. The first column holds text to find, the
//! second the index entry for it (`Fruit:Apple` is the sub-entry Apple under Fruit; empty means the text itself).

use crate::document::Document;
use crate::model::{Block, RowChild, TableChild};
use crate::ops::AutoMarkEntry;

fn cell_text(blocks: &[Block]) -> String {
    let mut parts = Vec::new();
    crate::model::walk_paragraphs(blocks, &mut |p| parts.push(p.plain_text().trim().to_string()));
    parts.join(" ").trim().to_string()
}

/// The entries of an AutoMark file: the rows of its first table that have text in the first column.
pub fn read(bytes: &[u8]) -> Result<Vec<AutoMarkEntry>, String> {
    let doc = Document::open(bytes).map_err(|e| format!("That file can't be read: {e}"))?;
    let table = doc.body.iter().find_map(|b| if let Block::Table(t) = b { Some(t) } else { None }).ok_or("The file has no table. An AutoMark file is a table of two columns: the text to find, then its index entry.")?;
    let mut out = Vec::new();
    for child in &table.children {
        let TableChild::Row(row) = child else { continue };
        let cells: Vec<String> = row
            .children
            .iter()
            .filter_map(|c| if let RowChild::Cell(c) = c { Some(cell_text(&c.blocks)) } else { None })
            .collect();
        let Some(text) = cells.first().filter(|t| !t.is_empty()) else { continue };
        out.push(AutoMarkEntry { text: text.clone(), entry: cells.get(1).cloned().unwrap_or_default(), bold: false, italic: false });
    }
    if out.is_empty() {
        return Err("The table has no text in its first column.".into());
    }
    Ok(out)
}

/// A new AutoMark file as a .docx: a table of two columns, with the entries given (or two examples).
pub fn template(entries: &[AutoMarkEntry]) -> Result<Vec<u8>, String> {
    let mut doc = Document::new_blank();
    let first = doc.render_all().order[0];
    let rows: Vec<Vec<String>> = if entries.is_empty() {
        vec![vec!["apple".into(), "Fruit:Apple".into()], vec!["pear".into(), "Fruit:Pear".into()]]
    } else {
        entries.iter().map(|e| vec![e.text.clone(), e.entry.clone()]).collect()
    };
    let op = crate::ops::Op::InsertTable { after: crate::ops::Id(first), rows: rows.len(), cols: 2, data: rows, autofit: None, column_width: None };
    doc.apply_batch(&[op], false).map_err(|e| e.to_string())?;
    doc.save().map_err(|e| e.to_string())
}
