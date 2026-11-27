//! Mail merge, as in Word's Mailings tab: a letter, a sheet of labels, an envelope or a directory with merge fields in
//! it, and a list of recipients to fill them from.
//!
//! The pieces:
//! - `csv` and `xlsx` read a list of recipients from a file.
//! - `instr` writes and reads the merge fields (`MERGEFIELD`, `ADDRESSBLOCK`, `GREETINGLINE`, the rules).
//! - `address` lays out an address block and a greeting line from a recipient's columns.
//! - `run` merges a document with recipients. It is a pure function of the document's blocks and the records.
//! - `state` keeps the recipients in the file: `w:mailMerge` in the settings, as Word writes it, and a part of
//!   Docxtor's own with the recipients themselves (Word keeps them in the file the data source points at).
//! - `labels` and `envelope` build a sheet of labels and an envelope page.

pub mod address;
pub mod csv;
pub mod envelope;
pub mod instr;
pub mod labels;
pub mod run;
pub mod state;
pub mod xlsx;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A list of recipients as a file holds it: a header row and the records.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// A table from a header row and records. Column names are made usable: an empty one becomes `Column 3`, a
    /// repeated one gets a number (`Name 2`). Every record has one value for each column.
    ///
    /// This is where every recipient list (a text file, a workbook, an extension's source) becomes a table, so the
    /// limits for a list from outside are kept here: columns, rows and cells past them are left out.
    pub fn from_rows(mut header: Vec<String>, mut rows: Vec<Vec<String>>) -> Table {
        use crate::budget::caps::{LIST_CELLS, LIST_COLS, LIST_ROWS};
        header.truncate(LIST_COLS);
        let width = header.len();
        rows.truncate(LIST_ROWS.min(LIST_CELLS / width.max(1)));
        let mut columns: Vec<String> = Vec::with_capacity(width);
        // Names taken so far, compared as `norm` does, and the next number to try for each name.
        let mut taken: std::collections::HashSet<String> = std::collections::HashSet::with_capacity(width);
        let mut next: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (i, h) in header.into_iter().enumerate() {
            let base = h.trim().to_string();
            let base = if base.is_empty() { format!("Column {}", i + 1) } else { base };
            let mut name = base.clone();
            let n = next.entry(norm(&base)).or_insert(2);
            while taken.contains(&norm(&name)) {
                name = format!("{base} {n}");
                *n += 1;
            }
            taken.insert(norm(&name));
            columns.push(name);
        }
        let rows = rows
            .into_iter()
            .map(|mut r| {
                r.resize(width, String::new());
                r.into_iter().map(clean).collect()
            })
            .collect();
        Table { columns, rows }
    }
}

/// A value as the merge shows it: no control characters (an XML part can't hold them), no trailing blank space.
pub fn clean(v: String) -> String {
    let v: String = v.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t')).collect();
    v.trim_end().to_string()
}

/// A column or field name compared without regard to case, with an underscore standing for a space (Word writes
/// `MERGEFIELD First_Name` for the column `First Name`).
pub fn norm(name: &str) -> String {
    name.replace('_', " ").split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Where the recipients came from. Word links to the file; Docxtor also keeps the recipients in the document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceInfo {
    /// `csv`, `xlsx`, or the id of a data source an extension added.
    #[serde(default)]
    pub kind: String,
    /// The file's name (`people.csv`), or a name for a source that isn't a file.
    #[serde(default)]
    pub name: String,
    /// The sheet of a workbook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<String>,
    /// The separator of a text file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<char>,
}

/// One rule of a filter (Word's Filter and Sort, Filter records).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterRule {
    pub field: String,
    /// `eq`, `ne`, `lt`, `gt`, `le`, `ge`, `empty`, `not_empty`, `contains` or `not_contains`.
    pub op: String,
    #[serde(default)]
    pub value: String,
    /// How this rule joins the one before it: `and` (the default) or `or`. `and` binds tighter, as in a query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
}

/// One key of a sort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortKey {
    pub field: String,
    #[serde(default)]
    pub desc: bool,
}

/// The recipients of a mail merge, as the document keeps them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MailMerge {
    pub source: SourceInfo,
    pub table: Arc<Table>,
    /// Which records are in the merge (the check boxes of Edit recipient list). Empty means all of them.
    pub active: Vec<bool>,
    pub filter: Vec<FilterRule>,
    pub sort: Vec<SortKey>,
    /// Which column stands for one of Word's address fields (`First Name` is the column `Given`).
    pub mapping: BTreeMap<String, String>,
}

fn parse_number(s: &str) -> Option<f64> {
    crate::field_fmt::parse_number(s)
}

fn compare_text(a: &str, b: &str) -> std::cmp::Ordering {
    match (parse_number(a), parse_number(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    }
}

impl FilterRule {
    fn test(&self, value: &str) -> bool {
        let v = value.trim();
        let w = self.value.trim();
        match self.op.as_str() {
            "empty" => v.is_empty(),
            "not_empty" => !v.is_empty(),
            "contains" => v.to_lowercase().contains(&w.to_lowercase()),
            "not_contains" => !v.to_lowercase().contains(&w.to_lowercase()),
            "eq" => compare_text(v, w).is_eq(),
            "ne" => !compare_text(v, w).is_eq(),
            "lt" => compare_text(v, w).is_lt(),
            "gt" => compare_text(v, w).is_gt(),
            "le" => compare_text(v, w).is_le(),
            "ge" => compare_text(v, w).is_ge(),
            _ => true,
        }
    }
}

impl MailMerge {
    pub fn column(&self, name: &str) -> Option<usize> {
        let want = norm(name);
        self.table.columns.iter().position(|c| norm(c) == want)
    }

    pub fn is_active(&self, row: usize) -> bool {
        self.active.get(row).copied().unwrap_or(true)
    }

    /// The records in the merge, as indexes into the table, in the order they are merged: the ones that are ticked and
    /// pass the filter, sorted.
    pub fn order(&self) -> Vec<usize> {
        let rules: Vec<(usize, &FilterRule)> = self.filter.iter().filter_map(|r| self.column(&r.field).map(|c| (c, r))).collect();
        let mut rows: Vec<usize> = (0..self.table.rows.len())
            .filter(|&i| {
                if !self.is_active(i) {
                    return false;
                }
                if rules.is_empty() {
                    return true;
                }
                // An or of ands: a rule joined with `or` starts a new group.
                let mut any = false;
                let mut group = true;
                for (k, (c, r)) in rules.iter().enumerate() {
                    if k > 0 && r.join.as_deref() == Some("or") {
                        any |= group;
                        group = true;
                    }
                    group &= r.test(&self.table.rows[i][*c]);
                }
                any | group
            })
            .collect();
        let keys: Vec<(usize, bool)> = self.sort.iter().filter_map(|k| self.column(&k.field).map(|c| (c, k.desc))).collect();
        if !keys.is_empty() {
            rows.sort_by(|&a, &b| {
                for (c, desc) in &keys {
                    let o = compare_text(&self.table.rows[a][*c], &self.table.rows[b][*c]);
                    let o = if *desc { o.reverse() } else { o };
                    if !o.is_eq() {
                        return o;
                    }
                }
                std::cmp::Ordering::Equal
            });
        }
        rows
    }
}

/// What an op gives a document to merge with.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MailMergeDef {
    /// Word's `w:mainDocumentType`: `formLetters`, `mailingLabels`, `envelopes`, `catalog` (a directory), `email` or `fax`.
    pub main_type: String,
    #[serde(default)]
    pub source: Option<SourceInfo>,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub active: Option<Vec<bool>>,
    #[serde(default)]
    pub filter: Vec<FilterRule>,
    #[serde(default)]
    pub sort: Vec<SortKey>,
    #[serde(default)]
    pub mapping: BTreeMap<String, String>,
}

/// A list of recipients read from a file, with where it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipients {
    pub source: SourceInfo,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// The sheets of a workbook (empty for a text file).
    pub sheets: Vec<String>,
    /// Records past the limit were left out.
    pub truncated: bool,
}

/// Read a list of recipients from a file: a text file with comma, semicolon, tab or bar separated values, or an Excel
/// workbook (.xlsx, first sheet unless `sheet` names another). The name says which by its extension.
pub fn read_recipients(bytes: &[u8], name: &str, sheet: Option<&str>) -> Result<Recipients, String> {
    let lower = name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "xlsx" | "xlsm" => {
            let wb = xlsx::read(bytes, sheet)?;
            Ok(Recipients { source: SourceInfo { kind: "xlsx".into(), name: name.into(), table: Some(wb.sheet.clone()), delimiter: None }, columns: wb.table.columns, rows: wb.table.rows, sheets: wb.sheets, truncated: wb.truncated })
        }
        "xls" | "ods" | "numbers" => Err("Docxtor reads .xlsx and .csv files. Save the list as one of those first.".into()),
        _ => {
            let text = csv::decode_text(bytes);
            let delim = if ext == "tsv" { Some('\t') } else { None };
            let (t, d) = csv::parse(&text, delim)?;
            let truncated = t.rows.len() > xlsx::MAX_ROWS;
            let rows = if truncated { t.rows[..xlsx::MAX_ROWS].to_vec() } else { t.rows };
            Ok(Recipients { source: SourceInfo { kind: "csv".into(), name: name.into(), table: None, delimiter: Some(d) }, columns: t.columns, rows, sheets: Vec::new(), truncated })
        }
    }
}

/// The kinds of main document Word has.
pub const MAIN_TYPES: [&str; 6] = ["formLetters", "mailingLabels", "envelopes", "catalog", "email", "fax"];

#[cfg(test)]
mod tests {
    use super::*;

    fn people() -> MailMerge {
        let rows = [["Ada", "Lovelace", "CA", "30"], ["Grace", "Hopper", "NY", "9"], ["Alan", "Turing", "CA", "41"], ["Edsger", "Dijkstra", "", "100"]];
        MailMerge {
            table: Arc::new(Table::from_rows(
                ["First Name", "Last Name", "State", "Age"].map(String::from).to_vec(),
                rows.iter().map(|r| r.map(String::from).to_vec()).collect(),
            )),
            ..Default::default()
        }
    }

    fn rule(field: &str, op: &str, value: &str, join: Option<&str>) -> FilterRule {
        FilterRule { field: field.into(), op: op.into(), value: value.into(), join: join.map(String::from) }
    }

    #[test]
    fn order_applies_ticks_filter_and_sort() {
        let mut m = people();
        assert_eq!(m.order(), [0, 1, 2, 3]);
        m.active = vec![true, true, true, false];
        assert_eq!(m.order(), [0, 1, 2]);
        m.filter = vec![rule("state", "eq", "ca", None)];
        assert_eq!(m.order(), [0, 2]);
        m.filter = vec![rule("State", "eq", "CA", None), rule("Age", "gt", "35", Some("and")), rule("Last Name", "contains", "hop", Some("or"))];
        assert_eq!(m.order(), [1, 2]);
        m.filter = vec![rule("State", "empty", "", None)];
        m.active.clear();
        assert_eq!(m.order(), [3]);
        m.filter.clear();
        // Numbers sort as numbers.
        m.sort = vec![SortKey { field: "Age".into(), desc: true }];
        assert_eq!(m.order(), [3, 2, 0, 1]);
        m.sort = vec![SortKey { field: "State".into(), desc: false }, SortKey { field: "Last Name".into(), desc: true }];
        assert_eq!(m.order(), [3, 2, 0, 1]);
    }

    #[test]
    fn names_match_loosely() {
        let m = people();
        assert_eq!(m.column("first_name"), Some(0));
        assert_eq!(m.column("  LAST   name "), Some(1));
        assert_eq!(m.column("Nope"), None);
    }
}
