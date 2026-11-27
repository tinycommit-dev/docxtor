//! Outline: the blocks of the main text with their outline levels (for the Outline view and the Navigation pane),
//! promoting and demoting headings (`Op::SetOutlineLevel`) and moving blocks (`Op::MoveBlocks`).

use super::*;
use serde::Serialize;

/// One top-level block of the main text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutlineEntry {
    pub id: NodeId,
    /// 0 for body text, tables and anything that is not a heading; 1 to 9 for the outline levels.
    pub level: u8,
    /// `p` (a paragraph), `t` (a table) or `o` (a wrapper such as a content control).
    pub kind: &'static str,
    /// The start of the paragraph's text (up to 160 characters).
    pub text: String,
}

/// The level of a `Heading 1` to `Heading 9` style: the id (`Heading3`) or, in a file from a localized Word, the
/// style's name (`heading 3` under an id such as `Titre3`). Those change level by switching styles.
fn heading_style_level(styles: &crate::styles::StyleSheet, style: Option<&str>) -> Option<u8> {
    let style = style?;
    let number = |s: &str, prefix: &str| -> Option<u8> {
        let n: u8 = s.strip_prefix(prefix)?.trim().parse().ok()?;
        (1..=9).contains(&n).then_some(n)
    };
    number(style, "Heading").or_else(|| styles.get(style).and_then(|i| number(&i.name.to_lowercase(), "heading")))
}

impl Document {
    /// Every block of the main text, in order, with its outline level.
    pub fn outline(&self) -> Vec<OutlineEntry> {
        self.body
            .iter()
            .filter_map(|b| match b {
                Block::Para(p) => {
                    let text = p.plain_text();
                    let text: String = text.chars().take(160).collect();
                    Some(OutlineEntry { id: p.id, level: self.heading_level(p).unwrap_or(0), kind: "p", text })
                }
                Block::Table(t) => Some(OutlineEntry { id: t.id, level: 0, kind: "t", text: String::new() }),
                Block::Container(c) => Some(OutlineEntry { id: c.id, level: 0, kind: "o", text: String::new() }),
                Block::Opaque(_) => None,
            })
            .collect()
    }

    /// A block and everything under it: for a heading, the blocks after it down to the next heading of the same or a higher
    /// level; for any other block, just that block. Empty when `id` is not a block of the main text.
    pub fn section_ids(&self, id: NodeId) -> Vec<NodeId> {
        let outline = self.outline();
        let Some(i) = outline.iter().position(|e| e.id == id) else { return Vec::new() };
        let level = outline[i].level;
        let end = if level == 0 { i + 1 } else { outline[i + 1..].iter().position(|e| e.level > 0 && e.level <= level).map(|n| i + 1 + n).unwrap_or(outline.len()) };
        outline[i..end].iter().map(|e| e.id).collect()
    }

    /// The level of the nearest heading before the block `id` (0 when there is none), as the headings are now.
    fn level_before(&self, id: NodeId) -> u8 {
        let mut last = 0;
        for b in &self.body {
            if b.id() == id {
                break;
            }
            if let Block::Para(p) = b {
                if let Some(l) = self.heading_level(p) {
                    last = l;
                }
            }
        }
        last
    }

    /// Set the outline level of paragraphs to `level` (0 is body text, 1 to 9 are heading levels), or move each by
    /// `delta` levels (negative promotes). Headings keep to 1..=9; demoting a level 9 makes body text, and promoting
    /// body text makes a heading one level below the nearest heading above it (level 1 when there is none).
    pub(super) fn op_set_outline_level(&mut self, ids: &[Id], level: Option<u8>, delta: Option<i8>) -> Result<OpResult, EditError> {
        if level.is_none() && delta.is_none() {
            return Err(EditError::Invalid("set_outline_level needs a level or a delta".into()));
        }
        if level.is_some_and(|l| l > 9) {
            return Err(EditError::Invalid("outline levels are 0 to 9".into()));
        }
        for id in ids {
            let p = find_para(&self.body, id.0).ok_or(EditError::NotParagraph(id.0))?;
            let style = p.ppr.as_ref().and_then(|pp| pp.child("w:pStyle")).and_then(|s| s.attr("w:val")).map(String::from);
            let cur = self.heading_level(p).unwrap_or(0);
            let target: u8 = match (level, delta) {
                (Some(l), _) => l,
                (None, Some(d)) if cur == 0 => {
                    if d < 0 {
                        (self.level_before(id.0) + 1).min(9)
                    } else {
                        0
                    }
                }
                (None, Some(d)) => {
                    let n = cur as i16 + d as i16;
                    if n > 9 {
                        0
                    } else {
                        n.max(1) as u8
                    }
                }
                _ => cur,
            };
            if target == cur {
                continue;
            }
            self.touch(id.0);
            let by_style = cur == 0 || heading_style_level(&self.styles, style.as_deref()).is_some();
            if target == 0 {
                if by_style {
                    self.apply_inner(&Op::SetStyle { ids: vec![*id], style: String::new() })?;
                }
                // A direct level, or one a custom style brings, is switched off by writing body text.
                let still = find_para(&self.body, id.0).and_then(|p| self.heading_level(p)).is_some();
                if still {
                    self.apply_inner(&Op::FormatParagraphs { ids: vec![*id], format: ParaPatch { outline_lvl: Some(9), ..Default::default() } })?;
                }
            } else if by_style {
                self.apply_inner(&Op::SetStyle { ids: vec![*id], style: format!("Heading {target}") })?;
                // The style carries the level: a direct one would only hide it.
                if find_para(&self.body, id.0).and_then(|p| p.ppr.as_ref()).is_some_and(|pp| pp.child("w:outlineLvl").is_some()) {
                    self.apply_inner(&Op::FormatParagraphs { ids: vec![*id], format: ParaPatch { outline_lvl: Some(-1), ..Default::default() } })?;
                }
            } else {
                self.apply_inner(&Op::FormatParagraphs { ids: vec![*id], format: ParaPatch { outline_lvl: Some(target as i64 - 1), ..Default::default() } })?;
            }
        }
        Ok(OpResult::default())
    }

    /// Move top-level blocks, keeping their order, to just after `after` or just before `before`. List a heading and
    /// the text under it to move a section. A paragraph that holds a section break takes it along.
    pub(super) fn op_move_blocks(&mut self, ids: &[Id], after: Option<Id>, before: Option<Id>) -> Result<OpResult, EditError> {
        let target = match (after, before) {
            (Some(a), None) => a.0,
            (None, Some(b)) => b.0,
            _ => return Err(EditError::Invalid("move_blocks needs `after` or `before`".into())),
        };
        if ids.is_empty() {
            return Err(EditError::Invalid("move_blocks needs at least one block".into()));
        }
        if ids.iter().any(|i| i.0 == target) {
            return Err(EditError::Invalid("a block can't be moved next to itself".into()));
        }
        let mut idx = Vec::new();
        for id in ids {
            let at = self
                .body
                .iter()
                .position(|b| b.id() == id.0 && !matches!(b, Block::Opaque(_)))
                .ok_or_else(|| EditError::Invalid(format!("block {} is not a block of the main text", id.0)))?;
            if !idx.contains(&at) {
                idx.push(at);
            }
        }
        if !self.body.iter().any(|b| b.id() == target && !matches!(b, Block::Opaque(_))) {
            return Err(EditError::Invalid(format!("block {target} is not a block of the main text")));
        }
        idx.sort_unstable();
        let mut moved = Vec::new();
        for &i in idx.iter().rev() {
            moved.push(self.body.remove(i));
        }
        moved.reverse();
        let touched: Vec<NodeId> = moved.iter().map(|b| b.id()).collect();
        let at = self.body.iter().position(|b| b.id() == target).unwrap_or(0);
        let at = if after.is_some() { at + 1 } else { at };
        for (k, b) in moved.into_iter().enumerate() {
            self.body.insert(at + k, b);
        }
        for id in touched {
            self.touch(id);
        }
        self.touch(target);
        Ok(OpResult::default())
    }
}
