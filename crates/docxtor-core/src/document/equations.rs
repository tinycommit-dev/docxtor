//! The ops for equations. An equation stays the XML element Word wrote (`m:oMath` in the paragraph, or
//! `m:oMathPara` for one on its own line); editing replaces its formula and keeps the rest.

use super::*;
use crate::math;

/// An equation as the editor needs it to open the equation box.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EquationAt {
    pub latex: String,
    pub display: bool,
}

impl Document {
    /// The equation at a position (the one character `[o, o + 1)`), in any story.
    pub fn equation_at(&self, at: Pos) -> Option<EquationAt> {
        let p = self.find_any_para(at.p.0)?;
        let e = math::at(&p.inlines, at.o)?;
        Some(EquationAt { latex: math::omml_to_latex(e), display: e.name == "m:oMathPara" })
    }

    pub(super) fn op_insert_equation(&mut self, at: Pos, latex: &str, display: bool) -> Result<OpResult, EditError> {
        let omath = math::latex_to_omml(latex).map_err(EditError::Invalid)?;
        self.ensure_ns("m", math::NS_M);
        if display {
            return self.insert_display_equation(at, omath);
        }
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let off = at.o.min(p.len16());
        edit::insert_inline_at(&mut p.inlines, off, Inline::Opaque(omath));
        Ok(OpResult { caret: Some(Pos::new(at.p.0, off + 1)), ..Default::default() })
    }

    /// An equation on its own line: the paragraph is split around the position so the equation has one to itself.
    fn insert_display_equation(&mut self, at: Pos, omath: Element) -> Result<OpResult, EditError> {
        let len = self.para_ref(at.p.0)?.len16();
        let off = at.o.min(len);
        let (target, created) = if len == 0 {
            (at.p.0, vec![])
        } else if off == 0 {
            // The paragraph's own text moves to a new paragraph below; this one is left for the equation.
            let rest = self.split_paragraph(Pos::new(at.p.0, 0))?;
            (at.p.0, rest.created)
        } else {
            let first = self.split_paragraph(Pos::new(at.p.0, off))?;
            if off < len {
                // Text follows: an empty paragraph goes between the two halves.
                let mid = self.split_paragraph(Pos::new(at.p.0, off))?;
                (mid.created[0], vec![first.created[0], mid.created[0]])
            } else {
                (first.created[0], first.created)
            }
        };
        self.fill_display(target, omath)?;
        Ok(OpResult { caret: Some(Pos::new(target, 1)), created, count: None })
    }

    fn fill_display(&mut self, id: NodeId, omath: Element) -> Result<(), EditError> {
        let wrapped = math::display_wrap(omath);
        self.touch(id);
        let p = self.para_mut(id)?;
        p.inlines = vec![Inline::Opaque(wrapped)];
        if let Some(pp) = p.ppr.as_mut() {
            // A numbered or bulleted paragraph made by splitting is not what an equation line wants.
            pp.remove_children("w:numPr");
            pp.remove_children("w:sectPr");
        }
        Ok(())
    }

    pub(super) fn op_set_equation(&mut self, at: Pos, latex: &str, display: Option<bool>) -> Result<OpResult, EditError> {
        let new = math::latex_to_omml(latex).map_err(EditError::Invalid)?;
        self.ensure_ns("m", math::NS_M);
        self.touch(at.p.0);
        let p = self.para_mut(at.p.0)?;
        let el = math::at_mut(&mut p.inlines, at.o).ok_or_else(|| EditError::Invalid("there is no equation at that position".into()))?;
        let is_display = el.name == "m:oMathPara";
        match display {
            Some(true) if !is_display => {
                // Inline to a line of its own: the equation leaves its place and goes in a paragraph of its own.
                edit::delete_range(&mut p.inlines, at.o, at.o + 1);
                return self.insert_display_equation(at, new);
            }
            Some(false) if is_display => *el = new,
            _ => math::replace_inner(el, new),
        }
        Ok(OpResult { caret: Some(Pos::new(at.p.0, at.o + 1)), ..Default::default() })
    }
}
