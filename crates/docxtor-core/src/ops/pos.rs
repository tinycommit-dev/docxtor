//! Positions, ids and the small action types that table ops carry.

use super::*;

/// A block id. Accepts `12`, `"12"`, `"p12"` or `"t12"` so that ids copied from
/// DocScript work verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Id(pub NodeId);

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        let n = match &v {
            serde_json::Value::Number(n) => n.as_u64(),
            serde_json::Value::String(s) => s.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == '#').parse().ok(),
            _ => None,
        };
        n.map(|n| Id(n as NodeId)).ok_or_else(|| serde::de::Error::custom(format!("invalid block id: {v}")))
    }
}

/// A caret position: paragraph id + UTF-16 offset into its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pos {
    pub p: Id,
    pub o: usize,
}

impl Pos {
    pub fn new(p: NodeId, o: usize) -> Self {
        Pos { p: Id(p), o }
    }
}

/// Operations on a rectangle of cells (given by paragraphs in two corner cells).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CellAction {
    Merge,
    /// Split into `cols` by `rows` cells. Left out, a merged cell is unmerged.
    Split {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cols: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rows: Option<usize>,
    },
    /// Width, vertical alignment, margins, wrapping and text direction of the cells.
    Format(crate::table_format::CellFormat),
    /// Borders by edge: the outside edges of the selected cells, the lines between them, and the diagonals.
    Borders(Box<crate::table_format::BorderEdges>),
    /// Background fill (hex), or none to clear.
    Shading {
        #[serde(default)]
        color: Option<String>,
    },
    /// `top`, `middle` or `bottom`.
    Valign { value: String },
}

/// The switches of a table design (Word's "Table style options"); a field left out stays as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableLookPatch {
    /// Header row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_row: Option<bool>,
    /// Total row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_row: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_column: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_column: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banded_rows: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banded_columns: Option<bool>,
}

/// Whether `table_cell` inserts a cell or deletes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableCellAction {
    Insert,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableAction {
    RowAbove,
    RowBelow,
    ColLeft,
    ColRight,
    DeleteRow,
    DeleteCol,
    DeleteTable,
}
