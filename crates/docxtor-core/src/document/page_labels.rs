//! Page numbers as a section prints them. The editor's layout reports each paragraph's page number (restarts
//! included); a section that numbers its pages i, ii, iii or a, b, c prints that, so a table of contents and
//! `PAGEREF` fields must show it too.

use super::*;

/// The number format of the section holding each paragraph of the main text.
pub(super) struct PageLabels {
    section_of: HashMap<NodeId, usize>,
    /// `lowerRoman`, `upperRoman`, `lowerLetter`, `upperLetter`; None for 1, 2, 3.
    formats: Vec<Option<String>>,
}

impl PageLabels {
    /// The label page number `n` has where paragraph `pid` is.
    pub(super) fn label(&self, pid: NodeId, n: u32) -> String {
        match self.section_of.get(&pid).and_then(|s| self.formats.get(*s)).and_then(|f| f.as_deref()) {
            Some(fmt) => numbering::format_number(i64::from(n), fmt),
            None => n.to_string(),
        }
    }
}

impl Document {
    pub(super) fn page_labels(&self) -> PageLabels {
        let refs = self.section_refs();
        let formats: Vec<Option<String>> = refs
            .iter()
            .map(|(_, e)| e.and_then(|e| e.child("w:pgNumType")).and_then(|n| n.attr("w:fmt")).filter(|f| *f != "decimal").map(String::from))
            .collect();
        let mut section_of = HashMap::new();
        if formats.iter().any(Option::is_some) {
            let mut n = 0;
            walk_paragraphs(self.main_body(), &mut |p| {
                section_of.insert(p.id, n);
                if p.ppr.as_ref().is_some_and(|pp| pp.child("w:sectPr").is_some()) {
                    n += 1;
                }
            });
        }
        PageLabels { section_of, formats }
    }
}
