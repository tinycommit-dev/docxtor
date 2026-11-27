//! What applying an op gives back, and its errors.

use super::*;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpResult {
    /// Suggested caret position after the op.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caret: Option<Pos>,
    /// Blocks created by the op.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<NodeId>,
    /// Number of matches for `replace_text`; for `define_list`, the numId of the new list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("block {0} not found")]
    NotFound(NodeId),
    #[error("block {0} is not a paragraph")]
    NotParagraph(NodeId),
    #[error("{0}")]
    Invalid(String),
    #[error("invalid op JSON: {0}")]
    Json(String),
}

impl EditError {
    /// The error with any text in it cut to a length a person can read: an op from outside may carry megabytes (a
    /// name, a value), and an error that quotes it back would carry them too.
    pub fn shortened(self) -> EditError {
        const MAX: usize = 300;
        let cut = |s: String| {
            if s.chars().count() <= MAX {
                s
            } else {
                let mut t: String = s.chars().take(MAX).collect();
                t.push('…');
                t
            }
        };
        match self {
            EditError::Invalid(s) => EditError::Invalid(cut(s)),
            EditError::Json(s) => EditError::Json(cut(s)),
            other => other,
        }
    }
}
